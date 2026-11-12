//! RTT estimation and a bounded statistics window for voice reports (P4-07).
//!
//! RTT is derived from the standard SR/RR echo: we remember when a Sender Report was
//! sent under the middle 32 bits of its NTP timestamp, and a matching report block
//! returns that value in `LSR` plus the remote hold time in `DLSR`
//! (RFC 3550 §6.4.1). The capture contains no plaintext report blocks, so the
//! arithmetic is verified with synthetic vectors; see
//! `waves/RW-26/research/RE-VOICE-STATS.md` §2.

use std::collections::{BTreeMap, VecDeque};

use crate::rtcp::{
    Compound, ReportBlock, RtcpPacket, RtcpSenderScheduler, is_server_receiver_report,
    parse_compound, parse_server_receiver_report,
};

/// Default audio clock rate for Opus codec (48 000 Hz).
pub const DEFAULT_CLOCK_RATE: u32 = 48_000;

/// Maximum number of unmatched sender reports remembered at once.
const MAX_PENDING_SENDER_REPORTS: usize = 64;

/// Middle 32 bits of a 64-bit NTP timestamp (the value carried in `LSR`).
#[must_use]
pub fn ntp_middle_32(ntp_timestamp: u64) -> u32 {
    (ntp_timestamp >> 32) as u32
}

/// Matches report blocks to locally sent Sender Reports.
#[derive(Debug, Clone, Default)]
pub struct RttEstimator {
    sent: BTreeMap<u32, u64>,
}

impl RttEstimator {
    /// Create an empty estimator.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Remember a sender report by its NTP middle bits and local send time.
    pub fn note_sender_report(&mut self, ntp_middle: u32, sent_at_ms: u64) {
        if self.sent.len() >= MAX_PENDING_SENDER_REPORTS
            && let Some(oldest) = self.sent.keys().next().copied()
        {
            self.sent.remove(&oldest);
        }
        self.sent.insert(ntp_middle, sent_at_ms);
    }

    /// Compute RTT for a report block, if it echoes a known sender report.
    ///
    /// Returns `None` when `LSR` is zero or unknown, and clamps clock skew to zero.
    #[must_use]
    pub fn on_report_block(&mut self, block: &ReportBlock, received_at_ms: u64) -> Option<f64> {
        if block.last_sr == 0 {
            return None;
        }
        let sent_at_ms = *self.sent.get(&block.last_sr)?;
        let elapsed_ms = received_at_ms.saturating_sub(sent_at_ms) as f64;
        let hold_ms = f64::from(block.delay_since_last_sr) * 1000.0 / 65_536.0;
        Some((elapsed_ms - hold_ms).max(0.0))
    }
}

/// One sample of voice statistics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatsSample {
    /// Local receive time.
    pub at_ms: u64,
    /// Round-trip time, if a report block matched a sender report.
    pub rtt_ms: Option<f64>,
    /// Lost fraction from the report block (0.0..=255/256).
    pub fraction_lost: f64,
    /// Cumulative lost packets (24-bit signed in the wire format).
    pub cumulative_lost: i64,
    /// Receiver-reported jitter, milliseconds.
    pub jitter_ms: f64,
}

impl StatsSample {
    /// Create a sample from an RTCP report block and estimated RTT using the default Opus 48 kHz clock.
    #[must_use]
    pub fn from_report_block(block: &ReportBlock, at_ms: u64, rtt_ms: Option<f64>) -> Self {
        Self::from_report_block_with_clock(block, at_ms, rtt_ms, DEFAULT_CLOCK_RATE)
    }

    /// Create a sample from an RTCP report block with a specified clock rate (in Hz).
    #[must_use]
    pub fn from_report_block_with_clock(
        block: &ReportBlock,
        at_ms: u64,
        rtt_ms: Option<f64>,
        clock_rate: u32,
    ) -> Self {
        let fraction_lost = f64::from(block.fraction_lost) / 256.0;
        let cumulative_lost = i64::from(block.cumulative_lost);
        let clock = if clock_rate == 0 {
            48_000.0
        } else {
            f64::from(clock_rate)
        };
        let jitter_ms = (f64::from(block.jitter) * 1000.0) / clock;
        Self {
            at_ms,
            rtt_ms,
            fraction_lost,
            cumulative_lost,
            jitter_ms,
        }
    }
}

/// A bounded FIFO of [`StatsSample`] values.
#[derive(Debug, Clone)]
pub struct StatsWindow {
    capacity: usize,
    samples: VecDeque<StatsSample>,
}

impl StatsWindow {
    /// Create a window; zero capacity is treated as one.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            samples: VecDeque::new(),
        }
    }

    /// Push a sample, evicting the oldest when full.
    pub fn push(&mut self, sample: StatsSample) {
        if self.samples.len() == self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
    }

    /// Most recent sample.
    #[must_use]
    pub fn latest(&self) -> Option<&StatsSample> {
        self.samples.back()
    }

    /// Number of retained samples.
    #[must_use]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Whether the window is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Average RTT over samples that have one.
    #[must_use]
    pub fn average_rtt_ms(&self) -> Option<f64> {
        let mut sum = 0.0;
        let mut count = 0u32;
        for sample in &self.samples {
            if let Some(rtt) = sample.rtt_ms {
                sum += rtt;
                count += 1;
            }
        }
        if count == 0 {
            None
        } else {
            Some(sum / f64::from(count))
        }
    }

    /// Maximum reported jitter in the window.
    #[must_use]
    pub fn max_jitter_ms(&self) -> Option<f64> {
        self.samples
            .iter()
            .map(|sample| sample.jitter_ms)
            .reduce(f64::max)
    }

    /// Change of cumulative loss between the oldest and the newest sample.
    ///
    /// `None` until two samples are present. A negative value means the remote
    /// stream restarted, and is deliberately reported as-is.
    #[must_use]
    pub fn loss_delta(&self) -> Option<i64> {
        if self.samples.len() < 2 {
            return None;
        }
        let first = self.samples.front()?;
        let last = self.samples.back()?;
        Some(last.cumulative_lost - first.cumulative_lost)
    }
}

/// Overall session statistics for RTCP reports, RTT tracking, loss, and jitter.
#[derive(Debug, Clone)]
pub struct SessionStats {
    /// Local SSRC.
    local_ssrc: u32,
    /// Remote/server SSRC, if discovered.
    server_ssrc: Option<u32>,
    /// Number of local Sender Reports generated and sent.
    sr_sent_count: u64,
    /// Number of server Receiver Reports received.
    rr_received_count: u64,
    /// Number of undecrypted Receiver Reports counted when key is absent.
    undecrypted_rr_count: u64,
    /// Latest loss fraction reported by remote (0.0..=1.0).
    latest_fraction_lost: Option<f64>,
    /// Latest cumulative packets lost reported by remote.
    latest_cumulative_lost: Option<i64>,
    /// Latest jitter reported by remote in milliseconds.
    latest_jitter_ms: Option<f64>,
    /// Latest round-trip time in milliseconds.
    latest_rtt_ms: Option<f64>,
    /// RTT estimator matching local SR to remote echoed LSR/DLSR.
    rtt_estimator: RttEstimator,
    /// Window of recent stats samples.
    window: StatsWindow,
    /// Scheduler for periodic SR generation (~5 000 ms).
    sr_scheduler: RtcpSenderScheduler,
}

impl SessionStats {
    /// Default capacity for the stats sample window.
    pub const DEFAULT_WINDOW_CAPACITY: usize = 32;

    /// Create a session stats tracker for the given local SSRC with a specified window capacity.
    #[must_use]
    pub fn new(local_ssrc: u32, window_capacity: usize) -> Self {
        Self {
            local_ssrc,
            server_ssrc: None,
            sr_sent_count: 0,
            rr_received_count: 0,
            undecrypted_rr_count: 0,
            latest_fraction_lost: None,
            latest_cumulative_lost: None,
            latest_jitter_ms: None,
            latest_rtt_ms: None,
            rtt_estimator: RttEstimator::new(),
            window: StatsWindow::new(window_capacity),
            sr_scheduler: RtcpSenderScheduler::default(),
        }
    }

    /// Create a session stats tracker with default window capacity.
    #[must_use]
    pub fn with_default_capacity(local_ssrc: u32) -> Self {
        Self::new(local_ssrc, Self::DEFAULT_WINDOW_CAPACITY)
    }

    /// Local SSRC.
    #[must_use]
    pub const fn local_ssrc(&self) -> u32 {
        self.local_ssrc
    }

    /// Remote/server SSRC, if known.
    #[must_use]
    pub const fn server_ssrc(&self) -> Option<u32> {
        self.server_ssrc
    }

    /// Set or update the remote server SSRC.
    pub fn set_server_ssrc(&mut self, ssrc: u32) {
        self.server_ssrc = Some(ssrc);
    }

    /// Number of local Sender Reports sent.
    #[must_use]
    pub const fn sr_sent_count(&self) -> u64 {
        self.sr_sent_count
    }

    /// Number of Receiver Reports received from remote.
    #[must_use]
    pub const fn rr_received_count(&self) -> u64 {
        self.rr_received_count
    }

    /// Number of undecrypted Receiver Reports counted when key is absent.
    #[must_use]
    pub const fn undecrypted_rr_count(&self) -> u64 {
        self.undecrypted_rr_count
    }

    /// Latest fraction lost from remote (0.0..=1.0).
    #[must_use]
    pub const fn latest_fraction_lost(&self) -> Option<f64> {
        self.latest_fraction_lost
    }

    /// Latest cumulative packets lost from remote.
    #[must_use]
    pub const fn latest_cumulative_lost(&self) -> Option<i64> {
        self.latest_cumulative_lost
    }

    /// Latest remote jitter in milliseconds.
    #[must_use]
    pub const fn latest_jitter_ms(&self) -> Option<f64> {
        self.latest_jitter_ms
    }

    /// Latest round-trip time in milliseconds.
    #[must_use]
    pub const fn latest_rtt_ms(&self) -> Option<f64> {
        self.latest_rtt_ms
    }

    /// Reference to the internal RTT estimator.
    #[must_use]
    pub const fn rtt_estimator(&self) -> &RttEstimator {
        &self.rtt_estimator
    }

    /// Mutable reference to the internal RTT estimator.
    pub fn rtt_estimator_mut(&mut self) -> &mut RttEstimator {
        &mut self.rtt_estimator
    }

    /// Reference to the sample window.
    #[must_use]
    pub const fn window(&self) -> &StatsWindow {
        &self.window
    }

    /// Reference to the SR scheduler.
    #[must_use]
    pub const fn sr_scheduler(&self) -> &RtcpSenderScheduler {
        &self.sr_scheduler
    }

    /// Mutable reference to the SR scheduler.
    pub fn sr_scheduler_mut(&mut self) -> &mut RtcpSenderScheduler {
        &mut self.sr_scheduler
    }

    /// Whether an outgoing Sender Report is due at `now_ms`.
    #[must_use]
    pub fn is_sr_due(&self, now_ms: u64) -> bool {
        self.sr_scheduler.is_due(now_ms)
    }

    /// Average RTT over samples that matched an SR in the window.
    #[must_use]
    pub fn average_rtt_ms(&self) -> Option<f64> {
        self.window.average_rtt_ms()
    }

    /// Maximum jitter in the window.
    #[must_use]
    pub fn max_jitter_ms(&self) -> Option<f64> {
        self.window.max_jitter_ms()
    }

    /// Loss delta across the window.
    #[must_use]
    pub fn loss_delta(&self) -> Option<i64> {
        self.window.loss_delta()
    }

    /// Record that a local Sender Report was sent.
    pub fn note_sr_sent(&mut self, ntp_timestamp: u64, sent_at_ms: u64) {
        self.sr_sent_count += 1;
        self.sr_scheduler.note_sent(sent_at_ms);
        self.rtt_estimator
            .note_sender_report(ntp_middle_32(ntp_timestamp), sent_at_ms);
    }

    /// Safely record an undecrypted Receiver Report packet when encryption key is absent.
    pub fn note_undecrypted_rr(&mut self) {
        self.rr_received_count += 1;
        self.undecrypted_rr_count += 1;
    }

    /// Process a decrypted RTCP report block and update loss, jitter, and RTT.
    pub fn on_report_block(&mut self, block: &ReportBlock, received_at_ms: u64) {
        self.rr_received_count += 1;
        let rtt = self.rtt_estimator.on_report_block(block, received_at_ms);
        if let Some(rtt_val) = rtt {
            self.latest_rtt_ms = Some(rtt_val);
        }
        let sample = StatsSample::from_report_block(block, received_at_ms, rtt);
        self.latest_fraction_lost = Some(sample.fraction_lost);
        self.latest_cumulative_lost = Some(sample.cumulative_lost);
        self.latest_jitter_ms = Some(sample.jitter_ms);
        self.window.push(sample);
    }

    /// Process incoming server Receiver Report wire bytes.
    ///
    /// If `has_key` is false (or if decryption failed), verifies the wire structure,
    /// extracts the server SSRC, and records the packet count safely without interpreting
    /// ciphertext as loss/jitter.
    /// If `has_key` is true, parses the report block and updates metrics.
    pub fn on_incoming_rr_packet(
        &mut self,
        packet_bytes: &[u8],
        received_at_ms: u64,
        has_key: bool,
    ) {
        if packet_bytes.len() >= 8 {
            let sender_ssrc = u32::from_be_bytes([
                packet_bytes[4],
                packet_bytes[5],
                packet_bytes[6],
                packet_bytes[7],
            ]);
            self.server_ssrc = Some(sender_ssrc);
        }

        if !has_key {
            self.note_undecrypted_rr();
            return;
        }

        if is_server_receiver_report(packet_bytes)
            && let Ok((_, report, _, _)) = parse_server_receiver_report(packet_bytes)
        {
            self.on_report_block(&report, received_at_ms);
            return;
        }

        if let Ok(compound) = parse_compound(packet_bytes) {
            self.on_rtcp_compound(&compound, received_at_ms);
        } else {
            self.note_undecrypted_rr();
        }
    }

    /// Process an RTCP compound packet.
    pub fn on_rtcp_compound(&mut self, compound: &Compound, received_at_ms: u64) {
        for packet in &compound.packets {
            match packet {
                RtcpPacket::ReceiverReport {
                    sender_ssrc,
                    reports,
                } => {
                    self.server_ssrc = Some(*sender_ssrc);
                    if reports.is_empty() {
                        self.note_undecrypted_rr();
                    } else {
                        for report in reports {
                            self.on_report_block(report, received_at_ms);
                        }
                    }
                }
                RtcpPacket::SenderReport {
                    sender_ssrc,
                    reports,
                    ..
                } => {
                    self.server_ssrc = Some(*sender_ssrc);
                    for report in reports {
                        self.on_report_block(report, received_at_ms);
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ntp_middle_is_high_word() {
        assert_eq!(ntp_middle_32(0x1122_3344_5566_7788), 0x1122_3344);
    }
}
