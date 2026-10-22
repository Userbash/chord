//! RTP reception statistics (RFC 3550 §6.4.1 / Appendix A.3).
//!
//! In Discord voice `*_rtpsize` mode the fixed RTP header is transmitted in the clear,
//! so sequence numbers, timestamps and the SSRC are observable without keys. This
//! module turns a stream of such headers into loss/jitter counters. It does not
//! decrypt or decode media.
//!
//! Provenance and the 8-packet capture oracle: `waves/RW-26/research/RE-VOICE-STATS.md`.

use std::collections::BTreeSet;

/// RTP clock rate for Opus (48 kHz).
pub const OPUS_CLOCK_RATE: u32 = 48_000;

/// Threshold used to detect 16-bit sequence wrap (RFC 3550 A.1).
const SEQ_HALF: i32 = 32_768;

/// Minimum fixed RTP header length.
const FIXED_HEADER_LEN: usize = 12;

/// Fields of the fixed RTP header that matter for reception statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RtpHeader {
    /// Payload type (7 bits).
    pub payload_type: u8,
    /// Marker bit.
    pub marker: bool,
    /// 16-bit sequence number.
    pub sequence: u16,
    /// 32-bit RTP timestamp.
    pub timestamp: u32,
    /// Synchronization source.
    pub ssrc: u32,
}

/// How a packet relates to the stream seen so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observation {
    /// Packet extends the highest sequence seen so far.
    InOrder,
    /// Packet is older than the highest sequence but was not seen before.
    Reordered,
    /// Exact extended sequence was already observed.
    Duplicate,
}

/// Point-in-time view of reception counters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReceptionSnapshot {
    /// Unique packets received.
    pub received: u64,
    /// Packets expected from the first to the highest extended sequence.
    pub expected: u64,
    /// `expected - received`, saturating.
    pub lost: u64,
    /// Duplicate packets observed.
    pub duplicates: u64,
    /// Low 16 bits of the highest extended sequence.
    pub highest_sequence: Option<u16>,
    /// RFC 3550 interarrival jitter in RTP timestamp units.
    pub jitter_units: f64,
    /// The same jitter converted to milliseconds.
    pub jitter_ms: f64,
}

/// Accumulates reception statistics for one RTP stream.
#[derive(Debug, Clone)]
pub struct ReceptionStats {
    clock_rate: u32,
    base_ext: Option<u32>,
    highest_ext: Option<u32>,
    received: u64,
    duplicates: u64,
    seen: BTreeSet<u32>,
    base_arrival_ms: Option<f64>,
    prev_arrival_units: Option<f64>,
    prev_timestamp: Option<u32>,
    jitter: f64,
}

impl ReceptionStats {
    /// Create counters for a stream with the given RTP clock rate.
    ///
    /// A zero clock rate disables jitter accumulation (counters still work).
    #[must_use]
    pub fn new(clock_rate: u32) -> Self {
        Self {
            clock_rate,
            base_ext: None,
            highest_ext: None,
            received: 0,
            duplicates: 0,
            seen: BTreeSet::new(),
            base_arrival_ms: None,
            prev_arrival_units: None,
            prev_timestamp: None,
            jitter: 0.0,
        }
    }

    /// Record one received packet.
    pub fn observe(&mut self, header: &RtpHeader, arrival_ms: f64) -> Observation {
        let arrival_units = match self.base_arrival_ms {
            Some(base) => {
                let scale = if self.clock_rate == 0 {
                    1.0
                } else {
                    f64::from(self.clock_rate) / 1000.0
                };
                (arrival_ms - base) * scale
            }
            None => {
                self.base_arrival_ms = Some(arrival_ms);
                0.0
            }
        };

        let extended = match self.highest_ext {
            None => {
                let extended = u32::from(header.sequence);
                self.base_ext = Some(extended);
                extended
            }
            Some(highest) => extend_sequence(header.sequence, highest),
        };

        if !self.seen.insert(extended) {
            self.duplicates += 1;
            return Observation::Duplicate;
        }
        self.received += 1;
        let reordered = self.highest_ext.is_some_and(|highest| extended < highest);
        if self.highest_ext.is_none_or(|highest| extended > highest) {
            self.highest_ext = Some(extended);
        }

        if self.clock_rate != 0 {
            if let (Some(previous_units), Some(previous_timestamp)) =
                (self.prev_arrival_units, self.prev_timestamp)
            {
                let transit = arrival_units - previous_units;
                let timestamp_delta =
                    f64::from(header.timestamp.wrapping_sub(previous_timestamp) as i32);
                let delta = transit - timestamp_delta;
                self.jitter += (delta.abs() - self.jitter) / 16.0;
            }
            self.prev_arrival_units = Some(arrival_units);
            self.prev_timestamp = Some(header.timestamp);
        }

        if reordered {
            Observation::Reordered
        } else {
            Observation::InOrder
        }
    }

    /// Current counters.
    #[must_use]
    pub fn snapshot(&self) -> ReceptionSnapshot {
        let (expected, highest_sequence) = match (self.base_ext, self.highest_ext) {
            (Some(base), Some(highest)) => (
                u64::from(highest.saturating_sub(base)) + 1,
                Some(highest as u16),
            ),
            _ => (0, None),
        };
        let jitter_ms = if self.clock_rate == 0 {
            0.0
        } else {
            self.jitter / f64::from(self.clock_rate) * 1000.0
        };
        ReceptionSnapshot {
            received: self.received,
            expected,
            lost: expected.saturating_sub(self.received),
            duplicates: self.duplicates,
            highest_sequence,
            jitter_units: self.jitter,
            jitter_ms,
        }
    }
}

/// Parse the fixed part of an RTP header.
///
/// Returns `None` for non-RTP data (version != 2) and for inputs shorter than the
/// fixed 12-byte header. Header extensions are allowed and deliberately ignored.
#[must_use]
pub fn parse_rtp_header(bytes: &[u8]) -> Option<RtpHeader> {
    if bytes.len() < FIXED_HEADER_LEN || bytes[0] >> 6 != 2 {
        return None;
    }
    Some(RtpHeader {
        payload_type: bytes[1] & 0x7f,
        marker: bytes[1] & 0x80 != 0,
        sequence: u16::from_be_bytes([bytes[2], bytes[3]]),
        timestamp: u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
        ssrc: u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
    })
}

/// Extend a 16-bit sequence number relative to the highest extended value.
fn extend_sequence(sequence: u16, highest: u32) -> u32 {
    let cycle = highest & 0xffff_0000;
    let highest_sequence = (highest & 0xffff) as i32;
    let difference = i32::from(sequence) - highest_sequence;
    let extended = if difference > SEQ_HALF {
        cycle.wrapping_add(u32::from(sequence)).wrapping_sub(65_536)
    } else if difference < -SEQ_HALF {
        cycle.wrapping_add(u32::from(sequence)).wrapping_add(65_536)
    } else {
        cycle | u32::from(sequence)
    };
    // A packet far "ahead" can only be an artefact of wrapping below the stream
    // base; clamp it to the highest value so it is treated as a duplicate rather
    // than creating a huge phantom gap.
    if extended > highest.saturating_add(SEQ_HALF as u32) {
        highest
    } else {
        extended
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_sequence_wraps_forward() {
        assert_eq!(extend_sequence(0, 65_535), 65_536);
        assert_eq!(extend_sequence(1, 65_536), 65_537);
        assert_eq!(extend_sequence(65_534, 65_535), 65_534);
    }
}
