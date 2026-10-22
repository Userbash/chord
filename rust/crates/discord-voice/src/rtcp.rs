//! RTCP compound parser for voice statistics (P4-07).
//!
//! Discord voice uses SRTCP in `*_rtpsize` mode: the RTCP header and the beginning of
//! the packet are clear, while the auth tag follows the RTCP part. A captured
//! server report looks like `[RTCP 32 B][tag 16 B]`; a client sender report is
//! `[RTCP 28 B][tag 16 B][counter 4 B LE]`. The parser walks the self-describing RTCP
//! length chain and reports the remaining bytes as [`Compound::trailer_len`] instead
//! of treating the tag as a malformed packet.
//!
//! Provenance: `waves/RW-26/research/RE-VOICE-STATS.md`.

/// Sender Report payload type.
pub const RTCP_SR: u8 = 200;
/// Receiver Report payload type.
pub const RTCP_RR: u8 = 201;
/// Source Description payload type.
pub const RTCP_SDES: u8 = 202;
/// Goodbye payload type.
pub const RTCP_BYE: u8 = 203;
/// Application-defined payload type.
pub const RTCP_APP: u8 = 204;

/// Upper bound on packets in a single compound to keep untrusted input bounded.
pub const MAX_COMPOUND_PACKETS: usize = 32;

/// Length of one report block, in bytes.
const REPORT_BLOCK_LEN: usize = 24;

/// Length of the common RTCP header.
const HEADER_LEN: usize = 4;

/// One reception report block (RFC 3550 §6.4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReportBlock {
    /// SSRC of the stream this block describes.
    pub ssrc: u32,
    /// Lost fraction in 1/256 units (0..=255).
    pub fraction_lost: u8,
    /// Cumulative lost packets, 24-bit signed.
    pub cumulative_lost: i32,
    /// Extended highest sequence number received.
    pub extended_highest_seq: u32,
    /// Interarrival jitter in RTP timestamp units.
    pub jitter: u32,
    /// Middle 32 bits of the NTP timestamp of the last sent SR.
    pub last_sr: u32,
    /// Delay since the last SR, in 1/65536 seconds.
    pub delay_since_last_sr: u32,
}

/// Sender information from an SR packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SenderInfo {
    /// Full 64-bit NTP timestamp.
    pub ntp_timestamp: u64,
    /// RTP timestamp corresponding to the NTP time.
    pub rtp_timestamp: u32,
    /// Total packets sent.
    pub packet_count: u32,
    /// Total payload octets sent.
    pub octet_count: u32,
}

/// Parsed RTCP packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RtcpPacket {
    /// Sender Report with zero or more report blocks.
    SenderReport {
        /// SSRC of the sender.
        sender_ssrc: u32,
        /// Sender counters.
        info: SenderInfo,
        /// Report blocks carried by the packet.
        reports: Vec<ReportBlock>,
    },
    /// Receiver Report with zero or more report blocks.
    ReceiverReport {
        /// SSRC of the reporter.
        sender_ssrc: u32,
        /// Report blocks carried by the packet.
        reports: Vec<ReportBlock>,
    },
    /// SDES/BYE/APP or an unknown payload type, kept opaque.
    Other {
        /// Raw payload type.
        payload_type: u8,
        /// Report count field, meaningful for SDES/BYE only.
        report_count: u8,
        /// Body after the common header.
        body: Vec<u8>,
    },
}

/// Result of parsing a compound packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compound {
    /// Parsed packets in wire order.
    pub packets: Vec<RtcpPacket>,
    /// Bytes after the last RTCP packet (SRTCP tag and optional counter).
    pub trailer_len: usize,
}

/// Why an RTCP packet could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RtcpError {
    /// The declared packet length exceeds the available bytes.
    Truncated {
        /// Full packet size derived from the length field.
        needed: usize,
        /// Bytes available from the packet start.
        available: usize,
    },
    /// The first packet does not use RTCP version 2.
    InvalidVersion(u8),
    /// The declared length is below the minimum for its payload type.
    InvalidLength {
        /// Raw payload type.
        payload_type: u8,
        /// Length field in 32-bit words minus one.
        length_words: u16,
    },
    /// More packets than [`MAX_COMPOUND_PACKETS`].
    TooManyPackets,
    /// The padding bit is set; padded RTCP is not supported by this card.
    UnsupportedPadding,
}

/// Parse an RTCP compound packet that may carry an SRTCP trailer.
///
/// The function never panics on untrusted input. It stops at the first byte that
/// does not start an RTCP version-2 packet and reports the remaining bytes as the
/// trailer. A version-2 trailer whose declared length runs past the end is also
/// treated as a trailer when exactly 16 or 20 bytes remain (the two SRTCP shapes
/// observed in the capture); otherwise the packet is reported as truncated.
///
/// # Errors
///
/// Returns a typed [`RtcpError`] for malformed headers, impossible lengths,
/// padding, or a compound that exceeds [`MAX_COMPOUND_PACKETS`].
pub fn parse_compound(bytes: &[u8]) -> Result<Compound, RtcpError> {
    if bytes.is_empty() {
        return Ok(Compound {
            packets: Vec::new(),
            trailer_len: 0,
        });
    }
    if bytes.len() < HEADER_LEN {
        return Err(RtcpError::Truncated {
            needed: HEADER_LEN,
            available: bytes.len(),
        });
    }

    let mut packets = Vec::new();
    let mut offset = 0usize;
    while offset + HEADER_LEN <= bytes.len() {
        let first = bytes[offset];
        let version = first >> 6;
        if version != 2 {
            if packets.is_empty() {
                return Err(RtcpError::InvalidVersion(version));
            }
            break;
        }
        if first & 0x20 != 0 {
            return Err(RtcpError::UnsupportedPadding);
        }
        let report_count = first & 0x1f;
        let payload_type = bytes[offset + 1];
        let length_words = u16::from_be_bytes([bytes[offset + 2], bytes[offset + 3]]);
        let total = (usize::from(length_words) + 1) * 4;
        let minimum = minimum_len(payload_type, report_count);
        if total < minimum {
            return Err(RtcpError::InvalidLength {
                payload_type,
                length_words,
            });
        }
        if offset + total > bytes.len() {
            let available = bytes.len() - offset;
            // Only bytes *after* at least one parsed packet may be an SRTCP trailer;
            // a short first packet is a genuine truncation even if it happens to be
            // 16 or 20 bytes long.
            if offset > 0 && (available == 16 || available == 20) {
                break;
            }
            return Err(RtcpError::Truncated {
                needed: total,
                available,
            });
        }
        let body = &bytes[offset + HEADER_LEN..offset + total];
        let packet = match payload_type {
            RTCP_SR => {
                let sender_ssrc = read_u32(body, 0);
                let info = SenderInfo {
                    ntp_timestamp: (u64::from(read_u32(body, 4)) << 32)
                        | u64::from(read_u32(body, 8)),
                    rtp_timestamp: read_u32(body, 12),
                    packet_count: read_u32(body, 16),
                    octet_count: read_u32(body, 20),
                };
                RtcpPacket::SenderReport {
                    sender_ssrc,
                    info,
                    reports: parse_reports(&body[24..], report_count),
                }
            }
            RTCP_RR => RtcpPacket::ReceiverReport {
                sender_ssrc: read_u32(body, 0),
                reports: parse_reports(&body[4..], report_count),
            },
            _ => RtcpPacket::Other {
                payload_type,
                report_count,
                body: body.to_vec(),
            },
        };
        packets.push(packet);
        if packets.len() > MAX_COMPOUND_PACKETS {
            return Err(RtcpError::TooManyPackets);
        }
        offset += total;
    }

    Ok(Compound {
        packets,
        trailer_len: bytes.len() - offset,
    })
}

fn minimum_len(payload_type: u8, report_count: u8) -> usize {
    let reports = usize::from(report_count) * REPORT_BLOCK_LEN;
    match payload_type {
        RTCP_SR => HEADER_LEN + 24 + reports,
        RTCP_RR => HEADER_LEN + 4 + reports,
        _ => HEADER_LEN,
    }
}

fn parse_reports(body: &[u8], report_count: u8) -> Vec<ReportBlock> {
    let count = usize::from(report_count);
    let mut reports = Vec::with_capacity(count);
    for index in 0..count {
        let start = index * REPORT_BLOCK_LEN;
        let block = &body[start..start + REPORT_BLOCK_LEN];
        reports.push(ReportBlock {
            ssrc: read_u32(block, 0),
            fraction_lost: block[4],
            cumulative_lost: read_i24(block, 5),
            extended_highest_seq: read_u32(block, 8),
            jitter: read_u32(block, 12),
            last_sr: read_u32(block, 16),
            delay_since_last_sr: read_u32(block, 20),
        });
    }
    reports
}

fn read_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

fn read_i24(data: &[u8], offset: usize) -> i32 {
    let raw = (i32::from(data[offset]) << 16)
        | (i32::from(data[offset + 1]) << 8)
        | i32::from(data[offset + 2]);
    // Sign-extend bit 23.
    (raw << 8) >> 8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i24_sign_extension() {
        assert_eq!(read_i24(&[0xff, 0xff, 0xff], 0), -1);
        assert_eq!(read_i24(&[0x7f, 0xff, 0xff], 0), 8_388_607);
    }
}
