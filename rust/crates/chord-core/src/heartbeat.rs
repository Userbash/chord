#![forbid(unsafe_code)]

//! Dedicated independent `heartbeat_loop` task with `heartbeat_interval_ms` (STAGE-05.2.1)
//! and Op 1 / Op 11 `HEARTBEAT_ACK` tracking with Zombie Socket Detector (STAGE-05.2.2).
//!
//! Provides an isolated pulse timer that runs independently of UI event processing delays
//! or heavy incoming Gateway dispatch parsing. Communicates via lock-free atomic sequence
//! numbers (`AtomicU64`), atomic ACK state (`AtomicBool`), and a non-blocking bounded channel
//! (`try_send`) with `MissedTickBehavior::Skip` so slow downstream consumers never stall
//! heartbeat scheduling.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{Notify, mpsc};
use tokio::task::JoinHandle;
use tokio::time::{Instant, MissedTickBehavior};
use tokio_util::sync::CancellationToken;

use crate::gateway::{HeartbeatMode, Hello, QosPayload};
use crate::ws::{encode_heartbeat, encode_qos_heartbeat};

/// Default Discord Gateway heartbeat interval from OP 10 HELLO: 41,250 ms (`docs/re/gateway-protocol.md` §4).
pub const DEFAULT_HEARTBEAT_INTERVAL_MS: u64 = 41_250;
/// Discord Gateway Opcode 1: `HEARTBEAT` (bidirectional ping / immediate server heartbeat request).
pub const HEARTBEAT_OP: i64 = 1;
/// Discord Gateway Opcode 11: `HEARTBEAT_ACK` (server acknowledgement of client heartbeat).
pub const HEARTBEAT_ACK_OP: i64 = 11;
/// Sentinel stored in [`AtomicU64`] when no OP 0 dispatch sequence (`s`) has been received yet (`null` on wire).
pub const NO_SEQUENCE_SENTINEL: u64 = u64::MAX;
const NO_VALUE_SENTINEL: u64 = u64::MAX;

/// Signal emitted when a zombie socket is detected due to a missed Op 11 `HEARTBEAT_ACK`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZombieReconnectSignal {
    pub missed_tick_index: u64,
    pub last_seq: Option<u64>,
    pub elapsed_without_ack_ms: u64,
    pub timeout_ms: u64,
    pub close_immediately: bool,
    pub reconnect_immediately: bool,
}

/// Outcome of processing an inbound Op 11 `HEARTBEAT_ACK` in [`ZombieSocketDetector`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeartbeatAckOutcome {
    Acknowledged {
        tick_index: u64,
        seq: Option<u64>,
        rtt_ms: u64,
    },
    UnexpectedAck,
}

/// Deterministic state machine for tracking outbound heartbeats (Op 1 / Op 40),
/// inbound acknowledgements (Op 11), RTT measurement, and zombie socket detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZombieSocketDetector {
    interval_ms: u64,
    awaiting_ack: bool,
    zombie: bool,
    pending_tick_index: Option<u64>,
    pending_seq: Option<u64>,
    sent_at_ms: Option<u64>,
    acks_received: u64,
    missed_acks: u64,
    last_rtt_ms: Option<u64>,
}

impl ZombieSocketDetector {
    #[must_use]
    pub const fn new(interval_ms: u64) -> Self {
        Self {
            interval_ms,
            awaiting_ack: false,
            zombie: false,
            pending_tick_index: None,
            pending_seq: None,
            sent_at_ms: None,
            acks_received: 0,
            missed_acks: 0,
            last_rtt_ms: None,
        }
    }

    #[must_use]
    pub const fn interval_ms(&self) -> u64 {
        self.interval_ms
    }

    #[must_use]
    pub const fn is_awaiting_ack(&self) -> bool {
        self.awaiting_ack
    }

    #[must_use]
    pub const fn is_zombie(&self) -> bool {
        self.zombie
    }

    #[must_use]
    pub const fn acks_received(&self) -> u64 {
        self.acks_received
    }

    #[must_use]
    pub const fn missed_acks(&self) -> u64 {
        self.missed_acks
    }

    #[must_use]
    pub const fn last_rtt_ms(&self) -> Option<u64> {
        self.last_rtt_ms
    }

    #[must_use]
    pub const fn pending_tick_index(&self) -> Option<u64> {
        self.pending_tick_index
    }

    #[must_use]
    pub const fn pending_seq(&self) -> Option<u64> {
        self.pending_seq
    }

    #[must_use]
    pub const fn ack_deadline_ms(&self) -> Option<u64> {
        match self.sent_at_ms {
            Some(sent_ms) if self.awaiting_ack => Some(sent_ms.saturating_add(self.interval_ms)),
            _ => None,
        }
    }

    pub fn on_heartbeat_sent(&mut self, tick_index: u64, seq: Option<u64>, now_ms: u64) {
        self.awaiting_ack = true;
        self.pending_tick_index = Some(tick_index);
        self.pending_seq = seq;
        self.sent_at_ms = Some(now_ms);
    }

    pub fn on_heartbeat_ack(&mut self, now_ms: u64) -> HeartbeatAckOutcome {
        if !self.awaiting_ack {
            return HeartbeatAckOutcome::UnexpectedAck;
        }
        let tick_index = self.pending_tick_index.unwrap_or(0);
        let seq = self.pending_seq;
        let sent_ms = self.sent_at_ms.unwrap_or(now_ms);
        let rtt_ms = now_ms.saturating_sub(sent_ms);

        self.awaiting_ack = false;
        self.zombie = false;
        self.pending_tick_index = None;
        self.pending_seq = None;
        self.sent_at_ms = None;
        self.acks_received = self.acks_received.saturating_add(1);
        self.last_rtt_ms = Some(rtt_ms);

        HeartbeatAckOutcome::Acknowledged {
            tick_index,
            seq,
            rtt_ms,
        }
    }

    pub fn check_timeout(&mut self, now_ms: u64) -> Option<ZombieReconnectSignal> {
        let deadline_ms = self.ack_deadline_ms()?;
        if now_ms < deadline_ms {
            return None;
        }
        Some(self.trigger_zombie(now_ms))
    }

    pub fn on_next_tick_due(&mut self, now_ms: u64) -> Option<ZombieReconnectSignal> {
        if !self.awaiting_ack {
            return None;
        }
        Some(self.trigger_zombie(now_ms))
    }

    fn trigger_zombie(&mut self, now_ms: u64) -> ZombieReconnectSignal {
        let sent_ms = self.sent_at_ms.unwrap_or(now_ms);
        let elapsed_without_ack_ms = now_ms.saturating_sub(sent_ms);
        let missed_tick_index = self.pending_tick_index.unwrap_or(1);
        let last_seq = self.pending_seq;

        self.zombie = true;
        self.missed_acks = self.missed_acks.saturating_add(1);

        ZombieReconnectSignal {
            missed_tick_index,
            last_seq,
            elapsed_without_ack_ms,
            timeout_ms: self.interval_ms,
            close_immediately: true,
            reconnect_immediately: true,
        }
    }
}

/// Errors returned by [`HeartbeatConfig`] validation or [`heartbeat_loop`] execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeartbeatLoopError {
    ZeroInterval,
    ZeroCapacity,
    Join(String),
}

impl fmt::Display for HeartbeatLoopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroInterval => write!(f, "heartbeat_interval_ms must be greater than zero"),
            Self::ZeroCapacity => write!(f, "heartbeat pulse channel capacity must be at least 1"),
            Self::Join(msg) => write!(f, "heartbeat_loop task join error: {msg}"),
        }
    }
}

impl std::error::Error for HeartbeatLoopError {}

/// Reason why [`heartbeat_loop`] terminated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeartbeatLoopOutcome {
    Cancelled,
    ChannelClosed,
    ZombieDetected(ZombieReconnectSignal),
}

/// Configuration for the isolated [`heartbeat_loop`] task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatConfig {
    pub heartbeat_interval_ms: u64,
    pub jitter_seed: u64,
    pub use_initial_jitter: bool,
    pub mode: HeartbeatMode,
    pub qos: Option<QosPayload>,
    pub enforce_ack_timeout: bool,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval_ms: DEFAULT_HEARTBEAT_INTERVAL_MS,
            jitter_seed: 0,
            use_initial_jitter: false,
            mode: HeartbeatMode::LegacyOp1,
            qos: None,
            enforce_ack_timeout: false,
        }
    }
}

impl HeartbeatConfig {
    #[must_use]
    pub fn new(heartbeat_interval_ms: u64) -> Self {
        Self {
            heartbeat_interval_ms,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn from_hello(hello: Hello) -> Self {
        Self::new(hello.heartbeat_interval_ms)
    }

    #[must_use]
    pub fn with_jitter_seed(mut self, jitter_seed: u64) -> Self {
        self.jitter_seed = jitter_seed;
        self.use_initial_jitter = true;
        self
    }

    #[must_use]
    pub fn with_initial_jitter(mut self, enabled: bool) -> Self {
        self.use_initial_jitter = enabled;
        self
    }

    #[must_use]
    pub fn with_mode(mut self, mode: HeartbeatMode) -> Self {
        self.mode = mode;
        self
    }

    #[must_use]
    pub fn with_qos(mut self, qos: Option<QosPayload>) -> Self {
        self.qos = qos;
        self
    }

    #[must_use]
    pub fn with_ack_timeout(mut self, enabled: bool) -> Self {
        self.enforce_ack_timeout = enabled;
        self
    }

    pub fn validate(&self) -> Result<(), HeartbeatLoopError> {
        if self.heartbeat_interval_ms == 0 {
            return Err(HeartbeatLoopError::ZeroInterval);
        }
        Ok(())
    }

    #[must_use]
    pub fn initial_delay_ms(&self) -> u64 {
        if self.heartbeat_interval_ms == 0 {
            return 0;
        }
        if self.use_initial_jitter {
            self.jitter_seed % self.heartbeat_interval_ms
        } else {
            self.heartbeat_interval_ms
        }
    }

    #[must_use]
    pub fn encode_frame(&self, seq: Option<u64>) -> Vec<u8> {
        match self.mode {
            HeartbeatMode::LegacyOp1 => encode_heartbeat(seq),
            HeartbeatMode::Qos => encode_qos_heartbeat(seq.unwrap_or(0), self.qos.as_ref()),
        }
    }
}

/// A single heartbeat pulse emitted by [`heartbeat_loop`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatPulse {
    pub tick_index: u64,
    pub seq: Option<u64>,
    pub elapsed_ms: u64,
    pub interval_ms: u64,
    pub immediate: bool,
    pub frame: Vec<u8>,
}

/// Shared lock-free telemetry and ACK control state for an active [`heartbeat_loop`].
#[derive(Debug)]
pub struct HeartbeatSharedState {
    last_seq: Arc<AtomicU64>,
    ticks_emitted: AtomicU64,
    dropped_pulses: AtomicU64,
    awaiting_ack: AtomicBool,
    zombie_detected: AtomicBool,
    pending_tick_index: AtomicU64,
    pending_seq: AtomicU64,
    last_sent_elapsed_ms: AtomicU64,
    acks_received: AtomicU64,
    missed_acks: AtomicU64,
    last_rtt_ms: AtomicU64,
    start_instant: Instant,
    immediate_notify: Notify,
}

impl HeartbeatSharedState {
    #[must_use]
    pub fn new(last_seq: Arc<AtomicU64>) -> Self {
        Self {
            last_seq,
            ticks_emitted: AtomicU64::new(0),
            dropped_pulses: AtomicU64::new(0),
            awaiting_ack: AtomicBool::new(false),
            zombie_detected: AtomicBool::new(false),
            pending_tick_index: AtomicU64::new(0),
            pending_seq: AtomicU64::new(NO_SEQUENCE_SENTINEL),
            last_sent_elapsed_ms: AtomicU64::new(0),
            acks_received: AtomicU64::new(0),
            missed_acks: AtomicU64::new(0),
            last_rtt_ms: AtomicU64::new(NO_VALUE_SENTINEL),
            start_instant: Instant::now(),
            immediate_notify: Notify::new(),
        }
    }

    #[must_use]
    pub fn current_seq(&self) -> Option<u64> {
        decode_seq(self.last_seq.load(Ordering::SeqCst))
    }

    pub fn set_last_seq(&self, seq: Option<u64>) {
        self.last_seq.store(encode_seq(seq), Ordering::SeqCst);
    }

    #[must_use]
    pub fn ticks_emitted(&self) -> u64 {
        self.ticks_emitted.load(Ordering::SeqCst)
    }

    #[must_use]
    pub fn dropped_pulses(&self) -> u64 {
        self.dropped_pulses.load(Ordering::SeqCst)
    }

    #[must_use]
    pub fn is_awaiting_ack(&self) -> bool {
        self.awaiting_ack.load(Ordering::SeqCst)
    }

    #[must_use]
    pub fn is_zombie(&self) -> bool {
        self.zombie_detected.load(Ordering::SeqCst)
    }

    #[must_use]
    pub fn acks_received(&self) -> u64 {
        self.acks_received.load(Ordering::SeqCst)
    }

    #[must_use]
    pub fn missed_acks(&self) -> u64 {
        self.missed_acks.load(Ordering::SeqCst)
    }

    #[must_use]
    pub fn last_rtt_ms(&self) -> Option<u64> {
        let raw = self.last_rtt_ms.load(Ordering::SeqCst);
        if raw == NO_VALUE_SENTINEL {
            None
        } else {
            Some(raw)
        }
    }

    pub fn record_heartbeat_ack(&self) -> bool {
        if !self.awaiting_ack.swap(false, Ordering::SeqCst) {
            return false;
        }
        let now_ms = self.elapsed_ms();
        let sent_ms = self.last_sent_elapsed_ms.load(Ordering::SeqCst);
        let rtt_ms = now_ms.saturating_sub(sent_ms);
        self.last_rtt_ms.store(rtt_ms, Ordering::SeqCst);
        self.acks_received.fetch_add(1, Ordering::SeqCst);
        true
    }

    pub fn trigger_immediate(&self) {
        self.immediate_notify.notify_one();
    }

    pub fn on_server_op1_heartbeat_request(&self) {
        self.trigger_immediate();
    }

    fn elapsed_ms(&self) -> u64 {
        u64::try_from(self.start_instant.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    fn mark_pulse_sent(&self, tick_index: u64, seq: Option<u64>, elapsed_ms: u64) {
        self.pending_tick_index.store(tick_index, Ordering::SeqCst);
        self.pending_seq.store(encode_seq(seq), Ordering::SeqCst);
        self.last_sent_elapsed_ms
            .store(elapsed_ms, Ordering::SeqCst);
        self.awaiting_ack.store(true, Ordering::SeqCst);
    }

    fn record_zombie_timeout(&self, interval_ms: u64) -> ZombieReconnectSignal {
        self.zombie_detected.store(true, Ordering::SeqCst);
        self.missed_acks.fetch_add(1, Ordering::SeqCst);

        let now_ms = self.elapsed_ms();
        let sent_ms = self.last_sent_elapsed_ms.load(Ordering::SeqCst);
        let elapsed_without_ack_ms = now_ms.saturating_sub(sent_ms).max(interval_ms);
        let missed_tick_index = self.pending_tick_index.load(Ordering::SeqCst).max(1);
        let last_seq = decode_seq(self.pending_seq.load(Ordering::SeqCst));

        ZombieReconnectSignal {
            missed_tick_index,
            last_seq,
            elapsed_without_ack_ms,
            timeout_ms: interval_ms,
            close_immediately: true,
            reconnect_immediately: true,
        }
    }
}

const fn encode_seq(seq: Option<u64>) -> u64 {
    match seq {
        Some(val) => val,
        None => NO_SEQUENCE_SENTINEL,
    }
}

const fn decode_seq(raw: u64) -> Option<u64> {
    if raw == NO_SEQUENCE_SENTINEL {
        None
    } else {
        Some(raw)
    }
}

/// Standalone asynchronous `heartbeat_loop` task with interval `heartbeat_interval_ms`.
pub async fn heartbeat_loop(
    config: HeartbeatConfig,
    last_seq: Arc<AtomicU64>,
    pulse_tx: mpsc::Sender<HeartbeatPulse>,
    cancel_token: CancellationToken,
) -> Result<HeartbeatLoopOutcome, HeartbeatLoopError> {
    let state = Arc::new(HeartbeatSharedState::new(last_seq));
    heartbeat_loop_with_state(config, state, pulse_tx, cancel_token).await
}

/// Runs [`heartbeat_loop`] attached to an explicit [`HeartbeatSharedState`] for telemetry and immediate triggers.
pub async fn heartbeat_loop_with_state(
    config: HeartbeatConfig,
    state: Arc<HeartbeatSharedState>,
    pulse_tx: mpsc::Sender<HeartbeatPulse>,
    cancel_token: CancellationToken,
) -> Result<HeartbeatLoopOutcome, HeartbeatLoopError> {
    config.validate()?;

    let start = state.start_instant;
    let period = Duration::from_millis(config.heartbeat_interval_ms);
    let first_instant = start + Duration::from_millis(config.initial_delay_ms());
    let mut timer = tokio::time::interval_at(first_instant, period);
    timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        let immediate = tokio::select! {
            biased;
            () = cancel_token.cancelled() => {
                return Ok(HeartbeatLoopOutcome::Cancelled);
            }
            () = state.immediate_notify.notified() => {
                timer.reset();
                true
            }
            _ = timer.tick() => {
                false
            }
        };

        if !immediate && config.enforce_ack_timeout && state.is_awaiting_ack() {
            let signal = state.record_zombie_timeout(config.heartbeat_interval_ms);
            cancel_token.cancel();
            return Ok(HeartbeatLoopOutcome::ZombieDetected(signal));
        }

        if emit_pulse(&config, &state, &pulse_tx, immediate).is_err() {
            return Ok(HeartbeatLoopOutcome::ChannelClosed);
        }
    }
}

fn emit_pulse(
    config: &HeartbeatConfig,
    state: &HeartbeatSharedState,
    pulse_tx: &mpsc::Sender<HeartbeatPulse>,
    immediate: bool,
) -> Result<(), ()> {
    let tick_index = state.ticks_emitted.fetch_add(1, Ordering::SeqCst) + 1;
    let seq = state.current_seq();
    let elapsed_ms = state.elapsed_ms();
    let frame = config.encode_frame(seq);

    state.mark_pulse_sent(tick_index, seq, elapsed_ms);

    let pulse = HeartbeatPulse {
        tick_index,
        seq,
        elapsed_ms,
        interval_ms: config.heartbeat_interval_ms,
        immediate,
        frame,
    };

    match pulse_tx.try_send(pulse) {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(_)) => {
            state.dropped_pulses.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        Err(TrySendError::Closed(_)) => Err(()),
    }
}

/// Controller handle for interacting with a spawned [`heartbeat_loop`] task.
#[derive(Debug)]
pub struct HeartbeatLoopControl {
    state: Arc<HeartbeatSharedState>,
    cancel_token: CancellationToken,
    join_handle: JoinHandle<Result<HeartbeatLoopOutcome, HeartbeatLoopError>>,
}

impl HeartbeatLoopControl {
    pub fn set_last_seq(&self, seq: Option<u64>) {
        self.state.set_last_seq(seq);
    }

    #[must_use]
    pub fn last_seq(&self) -> Option<u64> {
        self.state.current_seq()
    }

    #[must_use]
    pub fn ticks_emitted(&self) -> u64 {
        self.state.ticks_emitted()
    }

    #[must_use]
    pub fn dropped_pulses(&self) -> u64 {
        self.state.dropped_pulses()
    }

    #[must_use]
    pub fn is_awaiting_ack(&self) -> bool {
        self.state.is_awaiting_ack()
    }

    #[must_use]
    pub fn is_zombie(&self) -> bool {
        self.state.is_zombie()
    }

    #[must_use]
    pub fn acks_received(&self) -> u64 {
        self.state.acks_received()
    }

    #[must_use]
    pub fn missed_acks(&self) -> u64 {
        self.state.missed_acks()
    }

    #[must_use]
    pub fn last_rtt_ms(&self) -> Option<u64> {
        self.state.last_rtt_ms()
    }

    pub fn record_heartbeat_ack(&self) -> bool {
        self.state.record_heartbeat_ack()
    }

    pub fn trigger_immediate(&self) {
        self.state.trigger_immediate();
    }

    pub fn on_server_op1_heartbeat_request(&self) {
        self.state.on_server_op1_heartbeat_request();
    }

    #[must_use]
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }

    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }

    pub async fn join(self) -> Result<HeartbeatLoopOutcome, HeartbeatLoopError> {
        self.join_handle
            .await
            .map_err(|err| HeartbeatLoopError::Join(err.to_string()))?
    }

    pub async fn stop_and_join(self) -> Result<HeartbeatLoopOutcome, HeartbeatLoopError> {
        self.cancel_token.cancel();
        self.join().await
    }
}

/// Split components of a [`HeartbeatLoopHandle`]: control handle and pulse receiver channel.
#[derive(Debug)]
pub struct HeartbeatLoopSplit {
    pub control: HeartbeatLoopControl,
    pub pulse_rx: mpsc::Receiver<HeartbeatPulse>,
}

/// Handle returned by [`spawn_heartbeat_loop`] owning both the task controller and pulse receiver.
#[derive(Debug)]
pub struct HeartbeatLoopHandle {
    control: HeartbeatLoopControl,
    pulse_rx: mpsc::Receiver<HeartbeatPulse>,
}

impl HeartbeatLoopHandle {
    #[must_use]
    pub fn into_split(self) -> HeartbeatLoopSplit {
        HeartbeatLoopSplit {
            control: self.control,
            pulse_rx: self.pulse_rx,
        }
    }

    #[must_use]
    pub fn control(&self) -> &HeartbeatLoopControl {
        &self.control
    }

    pub fn pulse_rx_mut(&mut self) -> &mut mpsc::Receiver<HeartbeatPulse> {
        &mut self.pulse_rx
    }
}

/// Validates `config` and spawns a dedicated independent [`heartbeat_loop`] task on the Tokio runtime.
pub fn spawn_heartbeat_loop(
    config: HeartbeatConfig,
    channel_capacity: usize,
) -> Result<HeartbeatLoopHandle, HeartbeatLoopError> {
    config.validate()?;
    if channel_capacity == 0 {
        return Err(HeartbeatLoopError::ZeroCapacity);
    }

    let last_seq = Arc::new(AtomicU64::new(NO_SEQUENCE_SENTINEL));
    let state = Arc::new(HeartbeatSharedState::new(last_seq));
    let cancel_token = CancellationToken::new();
    let (pulse_tx, pulse_rx) = mpsc::channel(channel_capacity);

    let loop_state = Arc::clone(&state);
    let loop_token = cancel_token.clone();
    let join_handle = tokio::spawn(async move {
        heartbeat_loop_with_state(config, loop_state, pulse_tx, loop_token).await
    });

    Ok(HeartbeatLoopHandle {
        control: HeartbeatLoopControl {
            state,
            cancel_token,
            join_handle,
        },
        pulse_rx,
    })
}
