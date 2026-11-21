#![forbid(unsafe_code)]

//! Contract tests for dedicated independent `heartbeat_loop` task with `heartbeat_interval_ms` (STAGE-05.2.1).
//!
//! Verifies:
//! - Validation of `HeartbeatConfig` (`heartbeat_interval_ms == 0` rejected with `ZeroInterval`).
//! - Default interval matches Discord Gateway HELLO oracle (`41_250` ms).
//! - Deterministic first-tick jitter calculation (`jitter_seed % heartbeat_interval_ms`).
//! - Periodic pulse emission at `heartbeat_interval_ms` cadence with lock-free sequence updates (`None` -> `Some(seq)`).
//! - ETF wire payload encoding for both `HeartbeatMode::LegacyOp1` (`op: 1`) and `HeartbeatMode::Qos` (`op: 40`).
//! - Complete isolation of the timer loop from UI / consumer event processing delays.
//! - Immediate manual heartbeat trigger (`trigger_immediate` for server `Op 1` request).
//! - Graceful shutdown via `CancellationToken` and receiver drop handling.

use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::{Duration, Instant};

use chord_core::codec::etf::{self, EtfValue};
use chord_core::gateway::{HeartbeatMode, QOS_HEARTBEAT_OP, QosPayload};
use chord_core::heartbeat::{
    DEFAULT_HEARTBEAT_INTERVAL_MS, HeartbeatConfig, HeartbeatLoopError, HeartbeatLoopOutcome,
    HeartbeatPulse, NO_SEQUENCE_SENTINEL, heartbeat_loop, spawn_heartbeat_loop,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[test]
fn test_heartbeat_config_defaults_and_zero_interval_validation() {
    let default_cfg = HeartbeatConfig::default();
    assert_eq!(
        default_cfg.heartbeat_interval_ms,
        DEFAULT_HEARTBEAT_INTERVAL_MS
    );
    assert_eq!(DEFAULT_HEARTBEAT_INTERVAL_MS, 41_250);
    assert!(default_cfg.validate().is_ok());

    let zero_cfg = HeartbeatConfig::new(0);
    assert_eq!(zero_cfg.validate(), Err(HeartbeatLoopError::ZeroInterval));
}

#[test]
fn test_heartbeat_config_initial_delay_jitter_calculation() {
    let cfg = HeartbeatConfig::new(41_250).with_jitter_seed(10_000);
    assert_eq!(cfg.initial_delay_ms(), 10_000);

    let cfg_wrapped = HeartbeatConfig::new(1_000).with_jitter_seed(2_345);
    assert_eq!(cfg_wrapped.initial_delay_ms(), 345);

    let cfg_no_jitter = HeartbeatConfig::new(500).with_initial_jitter(false);
    assert_eq!(cfg_no_jitter.initial_delay_ms(), 500);
}

#[tokio::test]
async fn test_heartbeat_loop_rejects_zero_interval() {
    let cfg = HeartbeatConfig::new(0);
    let seq = Arc::new(AtomicU64::new(NO_SEQUENCE_SENTINEL));
    let (tx, _rx) = mpsc::channel(8);
    let cancel = CancellationToken::new();

    let res = heartbeat_loop(cfg, seq, tx, cancel).await;
    assert_eq!(res, Err(HeartbeatLoopError::ZeroInterval));
}

#[tokio::test]
async fn test_heartbeat_loop_emits_pulses_at_interval_and_tracks_atomic_seq() {
    let cfg = HeartbeatConfig::new(25)
        .with_initial_jitter(false)
        .with_mode(HeartbeatMode::LegacyOp1);

    let handle = spawn_heartbeat_loop(cfg, 16).expect("spawn_heartbeat_loop must succeed");
    let mut rx = handle.into_split();

    // First pulse has no sequence yet (None)
    let pulse1 = tokio::time::timeout(Duration::from_millis(200), rx.pulse_rx.recv())
        .await
        .expect("pulse 1 timeout")
        .expect("pulse 1 some");
    assert_eq!(pulse1.tick_index, 1);
    assert_eq!(pulse1.seq, None);
    assert_eq!(pulse1.interval_ms, 25);

    // Update sequence dynamically from dispatch thread without locking
    rx.control.set_last_seq(Some(42));

    let pulse2 = tokio::time::timeout(Duration::from_millis(200), rx.pulse_rx.recv())
        .await
        .expect("pulse 2 timeout")
        .expect("pulse 2 some");
    assert_eq!(pulse2.tick_index, 2);
    assert_eq!(pulse2.seq, Some(42));

    rx.control.set_last_seq(Some(99));

    let pulse3 = tokio::time::timeout(Duration::from_millis(200), rx.pulse_rx.recv())
        .await
        .expect("pulse 3 timeout")
        .expect("pulse 3 some");
    assert_eq!(pulse3.tick_index, 3);
    assert_eq!(pulse3.seq, Some(99));
    assert!(rx.control.ticks_emitted() >= 3);

    let outcome = rx.control.stop_and_join().await.expect("join must succeed");
    assert_eq!(outcome, HeartbeatLoopOutcome::Cancelled);
}

#[tokio::test]
async fn test_heartbeat_loop_etf_frame_encoding_legacy_op1_and_qos_op40() {
    // Test LegacyOp1 frame encoding
    let cfg_op1 = HeartbeatConfig::new(20)
        .with_initial_jitter(false)
        .with_mode(HeartbeatMode::LegacyOp1);
    let handle1 = spawn_heartbeat_loop(cfg_op1, 8).expect("spawn op1");
    let mut split1 = handle1.into_split();
    split1.control.set_last_seq(Some(77));

    let pulse_op1 = tokio::time::timeout(Duration::from_millis(200), split1.pulse_rx.recv())
        .await
        .expect("op1 timeout")
        .expect("op1 pulse");
    assert_eq!(pulse_op1.frame.first(), Some(&0x83));
    let (decoded1, len1) = etf::decode(&pulse_op1.frame).expect("decode op1 etf");
    assert_eq!(len1, pulse_op1.frame.len());
    assert_eq!(decoded1.get("op").and_then(EtfValue::as_int), Some(1));
    assert_eq!(decoded1.get("d").and_then(EtfValue::as_int), Some(77));
    split1.control.cancel();

    // Test QoS Op40 frame encoding
    let qos = QosPayload {
        active: true,
        reasons: vec!["foreground".to_string()],
    };
    let cfg_op40 = HeartbeatConfig::new(20)
        .with_initial_jitter(false)
        .with_mode(HeartbeatMode::Qos)
        .with_qos(Some(qos));
    let handle40 = spawn_heartbeat_loop(cfg_op40, 8).expect("spawn op40");
    let mut split40 = handle40.into_split();
    split40.control.set_last_seq(Some(123));

    let pulse_op40 = tokio::time::timeout(Duration::from_millis(200), split40.pulse_rx.recv())
        .await
        .expect("op40 timeout")
        .expect("op40 pulse");
    let (decoded40, len40) = etf::decode(&pulse_op40.frame).expect("decode op40 etf");
    assert_eq!(len40, pulse_op40.frame.len());
    assert_eq!(
        decoded40.get("op").and_then(EtfValue::as_int),
        Some(QOS_HEARTBEAT_OP)
    );
    let d40 = decoded40.get("d").expect("d field in op40");
    assert_eq!(d40.get("seq").and_then(EtfValue::as_int), Some(123));
    let qos_map = d40.get("qos").expect("qos map");
    assert_eq!(
        qos_map.get("active").and_then(EtfValue::as_atom),
        Some("true")
    );
    split40.control.cancel();
}

#[tokio::test]
async fn test_heartbeat_loop_isolated_from_slow_ui_event_processing() {
    // Channel capacity 2, interval 20ms.
    // Simulate UI/consumer thread stalled for 110ms without reading from channel.
    // The isolated heartbeat_loop must continue ticking on schedule without blocking!
    let cfg = HeartbeatConfig::new(20).with_initial_jitter(false);
    let handle = spawn_heartbeat_loop(cfg, 2).expect("spawn isolated loop");
    let mut split = handle.into_split();

    let start = Instant::now();
    // Simulate heavy UI freeze / slow event processing delay (110ms)
    tokio::time::sleep(Duration::from_millis(110)).await;

    // Even though channel capacity was only 2 and nobody read from `pulse_rx`,
    // `ticks_emitted()` must have advanced to at least 4 ticks without stalling!
    let emitted = split.control.ticks_emitted();
    let elapsed = start.elapsed();
    assert!(
        emitted >= 4,
        "heartbeat_loop must continue ticking independently during 110ms UI stall (emitted={emitted}, elapsed={elapsed:?})"
    );
    assert!(
        split.control.dropped_pulses() >= 2,
        "non-blocking try_send must record dropped pulses when consumer buffer is full instead of blocking timer"
    );

    split.control.cancel();
}

#[tokio::test]
async fn test_heartbeat_loop_immediate_trigger_on_server_heartbeat_request() {
    // Interval is long (5000ms), but immediate trigger wakes the loop right away.
    let cfg = HeartbeatConfig::new(5_000).with_initial_jitter(false);
    let handle = spawn_heartbeat_loop(cfg, 8).expect("spawn loop");
    let mut split = handle.into_split();

    split.control.set_last_seq(Some(555));
    split.control.trigger_immediate();

    let pulse: HeartbeatPulse =
        tokio::time::timeout(Duration::from_millis(150), split.pulse_rx.recv())
            .await
            .expect("immediate trigger must emit pulse within 150ms despite 5000ms interval")
            .expect("pulse received");
    assert_eq!(pulse.tick_index, 1);
    assert_eq!(pulse.seq, Some(555));
    assert!(pulse.immediate, "pulse must be marked as immediate");

    split.control.cancel();
}

#[tokio::test]
async fn test_heartbeat_loop_stops_cleanly_when_receiver_dropped() {
    let cfg = HeartbeatConfig::new(15).with_initial_jitter(false);
    let handle = spawn_heartbeat_loop(cfg, 4).expect("spawn loop");
    let split = handle.into_split();

    // Drop the receiver immediately
    drop(split.pulse_rx);

    // Next tick should detect closed channel and exit cleanly with ChannelClosed
    let outcome = tokio::time::timeout(Duration::from_millis(200), split.control.join())
        .await
        .expect("loop must terminate promptly after receiver drop")
        .expect("join result");
    assert_eq!(outcome, HeartbeatLoopOutcome::ChannelClosed);
}
