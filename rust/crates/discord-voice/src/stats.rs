//! RTT estimation and a bounded statistics window for voice reports (P4-07).
//!
//! RTT is derived from the standard SR/RR echo: we remember when a Sender Report was
//! sent under the middle 32 bits of its NTP timestamp, and a matching report block
//! returns that value in `LSR` plus the remote hold time in `DLSR`
//! (RFC 3550 §6.4.1). The capture contains no plaintext report blocks, so the
//! arithmetic is verified with synthetic vectors; see
//! `waves/RW-26/research/RE-VOICE-STATS.md` §2.

use std::collections::{BTreeMap, VecDeque};

use crate::rtcp::ReportBlock;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ntp_middle_is_high_word() {
        assert_eq!(ntp_middle_32(0x1122_3344_5566_7788), 0x1122_3344);
    }
}
