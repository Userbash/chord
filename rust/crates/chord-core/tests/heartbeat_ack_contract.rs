#![forbid(unsafe_code)]

//! Contract tests for ACK heartbeat tracking (Op 1 / Op 11) and zombie socket detector (STAGE-05.2.2).
//!
//! Verifies:
//! - Outbound heartbeat (Op 1 / Op 40) marks pending ACK (`is_awaiting_ack() == true`) and tracks sequence.
//! - Receiving Op 11 (`HEARTBEAT_ACK`) clears pending ACK, updates RTT (`last_rtt_ms()`), and increments `acks_received()`.
//! - Receiving server Op 1 (`HEARTBEAT` request) triggers an immediate heartbeat pulse and marks pending ACK.
//! - Zombie socket detection: when next scheduled tick arrives while still awaiting Op 11 ACK from the previous pulse,
//!   `heartbeat_loop` immediately detects a zombie socket (`HeartbeatLoopOutcome::ZombieDetected`), closes/terminates
//!   the connection loop, and emits a `ZombieReconnectSignal` so the transport reconnects without waiting.
//! - `ZombieSocketDetector` standalone state machine verifies deterministic timeout calculation, Op 1 / Op 11 lifecycle,
//!   and immediate close + reconnect action.

use std::time::Duration;

use chord_core::gateway::HeartbeatMode;
use chord_core::heartbeat::{
    HEARTBEAT_ACK_OP, HEARTBEAT_OP, HeartbeatAckOutcome, HeartbeatConfig, HeartbeatLoopOutcome,
    ZombieReconnectSignal, ZombieSocketDetector, spawn_heartbeat_loop,
};

#[test]
fn test_heartbeat_opcodes_and_zombie_detector_initial_state() {
    assert_eq!(HEARTBEAT_OP, 1);
    assert_eq!(HEARTBEAT_ACK_OP, 11);

    let detector = ZombieSocketDetector::new(41_250);
    assert_eq!(detector.interval_ms(), 41_250);
    assert!(!detector.is_awaiting_ack());
    assert!(!detector.is_zombie());
    assert_eq!(detector.acks_received(), 0);
    assert_eq!(detector.missed_acks(), 0);
    assert_eq!(detector.last_rtt_ms(), None);
    assert_eq!(detector.ack_deadline_ms(), None);
}

#[test]
fn test_zombie_socket_detector_ack_lifecycle_and_rtt_tracking() {
    let mut detector = ZombieSocketDetector::new(41_250);

    // 1. Outbound Op 1 heartbeat sent at t = 1000ms with seq = Some(42)
    detector.on_heartbeat_sent(1, Some(42), 1_000);
    assert!(detector.is_awaiting_ack());
    assert!(!detector.is_zombie());
    assert_eq!(detector.pending_tick_index(), Some(1));
    assert_eq!(detector.pending_seq(), Some(42));
    assert_eq!(detector.ack_deadline_ms(), Some(42_250));

    // Before deadline: socket is healthy
    assert_eq!(detector.check_timeout(42_249), None);
    assert!(!detector.is_zombie());

    // 2. Inbound Op 11 HEARTBEAT_ACK received at t = 1085ms (RTT = 85ms)
    let ack = detector.on_heartbeat_ack(1_085);
    assert_eq!(
        ack,
        HeartbeatAckOutcome::Acknowledged {
            tick_index: 1,
            seq: Some(42),
            rtt_ms: 85,
        }
    );
    assert!(!detector.is_awaiting_ack());
    assert!(!detector.is_zombie());
    assert_eq!(detector.acks_received(), 1);
    assert_eq!(detector.last_rtt_ms(), Some(85));
    assert_eq!(detector.ack_deadline_ms(), None);

    // Extra Op 11 ACK when no beat is pending is safely ignored
    assert_eq!(
        detector.on_heartbeat_ack(1_200),
        HeartbeatAckOutcome::UnexpectedAck
    );
    assert_eq!(detector.acks_received(), 1);
}

#[test]
fn test_zombie_socket_detector_triggers_immediate_close_and_reconnect_on_ack_timeout() {
    let mut detector = ZombieSocketDetector::new(41_250);

    // Send heartbeat #1 at t = 5000ms with seq = 105
    detector.on_heartbeat_sent(1, Some(105), 5_000);
    assert!(detector.is_awaiting_ack());
    assert_eq!(detector.ack_deadline_ms(), Some(46_250));

    // At t = 46_250ms (deadline reached without Op 11 ACK), detector flags zombie socket
    let signal = detector
        .check_timeout(46_250)
        .expect("must emit ZombieReconnectSignal on ACK timeout");
    assert_eq!(
        signal,
        ZombieReconnectSignal {
            missed_tick_index: 1,
            last_seq: Some(105),
            elapsed_without_ack_ms: 41_250,
            timeout_ms: 41_250,
            close_immediately: true,
            reconnect_immediately: true,
        }
    );
    assert!(detector.is_zombie());
    assert_eq!(detector.missed_acks(), 1);
}

#[test]
fn test_zombie_socket_detector_fails_if_second_heartbeat_due_before_ack() {
    let mut detector = ZombieSocketDetector::new(1_000);

    // Heartbeat #1 sent at t = 100ms
    detector.on_heartbeat_sent(1, Some(7), 100);
    assert!(detector.is_awaiting_ack());

    // Next scheduled tick arrives at t = 1100ms while still awaiting ACK
    let signal = detector
        .on_next_tick_due(1_100)
        .expect("unacknowledged previous beat on next tick must trigger zombie disconnect");
    assert_eq!(signal.missed_tick_index, 1);
    assert_eq!(signal.last_seq, Some(7));
    assert_eq!(signal.elapsed_without_ack_ms, 1_000);
    assert!(signal.close_immediately);
    assert!(signal.reconnect_immediately);
    assert!(detector.is_zombie());
}

#[tokio::test]
async fn test_heartbeat_loop_acknowledges_op11_and_continues_healthy_pulses() {
    let cfg = HeartbeatConfig::new(30)
        .with_initial_jitter(false)
        .with_mode(HeartbeatMode::LegacyOp1)
        .with_ack_timeout(true);

    let handle = spawn_heartbeat_loop(cfg, 16).expect("spawn heartbeat loop");
    let mut split = handle.into_split();
    split.control.set_last_seq(Some(10));

    // Receive pulse 1
    let pulse1 = tokio::time::timeout(Duration::from_millis(200), split.pulse_rx.recv())
        .await
        .expect("pulse 1 timeout")
        .expect("pulse 1 some");
    assert_eq!(pulse1.tick_index, 1);
    assert!(
        split.control.is_awaiting_ack(),
        "must await Op 11 ACK after pulse 1"
    );

    // Record Op 11 HEARTBEAT_ACK before next pulse
    assert!(split.control.record_heartbeat_ack());
    assert!(!split.control.is_awaiting_ack());
    assert_eq!(split.control.acks_received(), 1);
    assert!(split.control.last_rtt_ms().is_some());

    // Receive pulse 2 (loop stays alive because ACK was received)
    split.control.set_last_seq(Some(11));
    let pulse2 = tokio::time::timeout(Duration::from_millis(200), split.pulse_rx.recv())
        .await
        .expect("pulse 2 timeout")
        .expect("pulse 2 some");
    assert_eq!(pulse2.tick_index, 2);
    assert_eq!(pulse2.seq, Some(11));
    assert!(split.control.is_awaiting_ack());

    // Record Op 11 HEARTBEAT_ACK for pulse 2
    assert!(split.control.record_heartbeat_ack());
    assert_eq!(split.control.acks_received(), 2);

    let outcome = split.control.stop_and_join().await.expect("stop and join");
    assert_eq!(outcome, HeartbeatLoopOutcome::Cancelled);
}

#[tokio::test]
async fn test_heartbeat_loop_detects_zombie_socket_on_missed_op11_ack_and_closes_immediately() {
    let cfg = HeartbeatConfig::new(25)
        .with_initial_jitter(false)
        .with_mode(HeartbeatMode::LegacyOp1)
        .with_ack_timeout(true);

    let handle = spawn_heartbeat_loop(cfg, 16).expect("spawn heartbeat loop");
    let mut split = handle.into_split();
    split.control.set_last_seq(Some(88));

    // Pulse 1 is emitted at t ~ 25ms
    let pulse1 = tokio::time::timeout(Duration::from_millis(200), split.pulse_rx.recv())
        .await
        .expect("pulse 1 timeout")
        .expect("pulse 1");
    assert_eq!(pulse1.tick_index, 1);
    assert_eq!(pulse1.seq, Some(88));
    assert!(split.control.is_awaiting_ack());

    // Simulate dead/frozen socket: DO NOT call record_heartbeat_ack() (missed Op 11).
    // When tick 2 timer fires (~25ms later), heartbeat_loop must immediately detect the zombie socket,
    // cancel the connection token, and terminate with ZombieDetected(ZombieReconnectSignal).
    let outcome = tokio::time::timeout(Duration::from_millis(250), split.control.join())
        .await
        .expect("loop must terminate immediately on missed ACK timeout")
        .expect("join ok");

    match outcome {
        HeartbeatLoopOutcome::ZombieDetected(signal) => {
            assert_eq!(signal.missed_tick_index, 1);
            assert_eq!(signal.last_seq, Some(88));
            assert_eq!(signal.timeout_ms, 25);
            assert!(signal.close_immediately);
            assert!(signal.reconnect_immediately);
        }
        other => panic!("expected ZombieDetected outcome, got {other:?}"),
    }
}

#[tokio::test]
async fn test_heartbeat_loop_handles_inbound_op1_server_heartbeat_request() {
    // Long scheduled interval (5000ms), but server sends inbound Op 1 HEARTBEAT request
    let cfg = HeartbeatConfig::new(5_000)
        .with_initial_jitter(false)
        .with_ack_timeout(true);

    let handle = spawn_heartbeat_loop(cfg, 8).expect("spawn heartbeat loop");
    let mut split = handle.into_split();
    split.control.set_last_seq(Some(321));

    // Server sends Op 1 -> client must immediately emit Op 1 / Op 40 heartbeat response
    split.control.on_server_op1_heartbeat_request();

    let pulse = tokio::time::timeout(Duration::from_millis(150), split.pulse_rx.recv())
        .await
        .expect("inbound Op 1 must trigger immediate pulse")
        .expect("pulse");
    assert_eq!(pulse.tick_index, 1);
    assert_eq!(pulse.seq, Some(321));
    assert!(pulse.immediate);
    assert!(
        split.control.is_awaiting_ack(),
        "immediate heartbeat triggered by Op 1 must also track Op 11 ACK"
    );

    assert!(split.control.record_heartbeat_ack());
    assert!(!split.control.is_awaiting_ack());
    assert_eq!(split.control.acks_received(), 1);

    split.control.cancel();
}
