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
/// Transport-layer Feedback payload type (RFC 4585, e.g. Generic NACK).
pub const RTCP_RTPFB: u8 = 205;
/// Payload-specific Feedback payload type (RFC 4585 / RFC 5104, e.g. PLI, FIR, REMB).
pub const RTCP_PSFB: u8 = 206;

/// Feedback message type (FMT) for Generic NACK (PT 205, FMT 1, RFC 4585 §6.2.1).
pub const RTCP_FMT_NACK: u8 = 1;
/// Feedback message type (FMT) for Picture Loss Indication (PT 206, FMT 1, RFC 4585 §6.3.1).
pub const RTCP_FMT_PLI: u8 = 1;
/// Feedback message type (FMT) for Full Intra Request (PT 206, FMT 4, RFC 5104 §4.3.1).
pub const RTCP_FMT_FIR: u8 = 4;
/// Feedback message type (FMT) for Application-layer FB / REMB (PT 206, FMT 15).
pub const RTCP_FMT_REMB: u8 = 15;

/// 4-byte ASCII identifier for REMB packets (`"REMB"`).
pub const RTCP_REMB_IDENTIFIER: [u8; 4] = *b"REMB";

/// Wire length of an unencrypted RTCP Picture Loss Indication (PLI, 12 bytes).
pub const RTCP_PLI_LEN: usize = 12;
/// Wire length of a single-entry RTCP Full Intra Request (FIR, 20 bytes).
pub const RTCP_FIR_LEN: usize = 20;
/// Maximum number of expanded NACK sequence numbers accepted from one packet.
pub const MAX_NACK_SEQUENCES: usize = 128;

/// Target interval between outgoing RTCP Sender Reports (~5 000 ms).
pub const RTCP_SR_INTERVAL_MS: u64 = 5_000;

/// Wire length of a standard Discord 60-byte SRTCP Sender Report.
pub const SRTCP_SR_WIRE_LEN: usize = 60;
/// Unencrypted length of an RTCP Sender Report with 0 report blocks (28 bytes).
pub const RTCP_SR_CLEAR_LEN: usize = 28;
/// Wire length of a server 52-byte SRTCP Receiver Report.
pub const SRTCP_RR_WIRE_LEN: usize = 52;
/// Unencrypted length of an RTCP Receiver Report with 1 report block (32 bytes).
pub const RTCP_RR_CLEAR_LEN: usize = 32;

/// Encryption flag (E-bit) set on client SRTCP trailer index (bit 31).
pub const SRTCP_E_BIT: u32 = 0x8000_0000;

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

/// State for scheduling periodic RTCP Sender Reports (~5 000 ms).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtcpSenderScheduler {
    interval_ms: u64,
    last_sent_at_ms: Option<u64>,
}

impl Default for RtcpSenderScheduler {
    fn default() -> Self {
        Self::new(RTCP_SR_INTERVAL_MS)
    }
}

impl RtcpSenderScheduler {
    /// Create a scheduler with the specified interval.
    #[must_use]
    pub const fn new(interval_ms: u64) -> Self {
        Self {
            interval_ms,
            last_sent_at_ms: None,
        }
    }

    /// Configured interval in milliseconds.
    #[must_use]
    pub const fn interval_ms(&self) -> u64 {
        self.interval_ms
    }

    /// Timestamp in milliseconds when the last SR was sent.
    #[must_use]
    pub const fn last_sent_at_ms(&self) -> Option<u64> {
        self.last_sent_at_ms
    }

    /// Check if a Sender Report is due to be sent at `now_ms`.
    #[must_use]
    pub fn is_due(&self, now_ms: u64) -> bool {
        match self.last_sent_at_ms {
            None => true,
            Some(last) => now_ms.saturating_sub(last) >= self.interval_ms,
        }
    }

    /// Time until the next Sender Report is due, in milliseconds.
    #[must_use]
    pub fn time_until_due_ms(&self, now_ms: u64) -> u64 {
        match self.last_sent_at_ms {
            None => 0,
            Some(last) => {
                let elapsed = now_ms.saturating_sub(last);
                self.interval_ms.saturating_sub(elapsed)
            }
        }
    }

    /// Record that a Sender Report was sent at `now_ms`.
    pub fn note_sent(&mut self, now_ms: u64) {
        self.last_sent_at_ms = Some(now_ms);
    }

    /// Reset the scheduler.
    pub fn reset(&mut self) {
        self.last_sent_at_ms = None;
    }
}

/// One 4-byte Generic NACK FCI entry (RFC 4585 §6.2.1: PID + BLP bitmask).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NackEntry {
    /// Base lost RTP sequence number (Packet ID).
    pub pid: u16,
    /// 16-bit bitmask of losses for `pid + 1 ..= pid + 16` (bit `i` = `pid.wrapping_add(i + 1)`).
    pub blp: u16,
}

/// Parsed RTCP Generic NACK packet (`PT = 205, FMT = 1`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtcpNack {
    /// SSRC of the feedback sender.
    pub sender_ssrc: u32,
    /// SSRC of the media source experiencing loss.
    pub media_ssrc: u32,
    /// Raw `(PID, BLP)` entries carried by the packet.
    pub entries: Vec<NackEntry>,
    /// Deduplicated list of lost RTP sequence numbers expanded from `entries` (bounded to [`MAX_NACK_SEQUENCES`]).
    pub lost_sequences: Vec<u16>,
}

/// Parsed RTCP Picture Loss Indication packet (`PT = 206, FMT = 1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RtcpPli {
    /// SSRC of the feedback sender.
    pub sender_ssrc: u32,
    /// SSRC of the media source asked for a keyframe.
    pub media_ssrc: u32,
}

/// One 8-byte Full Intra Request FCI entry (`RFC 5104 §4.3.1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FirEntry {
    /// Target media sender SSRC.
    pub ssrc: u32,
    /// Command sequence number (incremented modulo 256 per command).
    pub sequence_number: u8,
}

/// Parsed RTCP Full Intra Request packet (`PT = 206, FMT = 4`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtcpFir {
    /// SSRC of the feedback sender.
    pub sender_ssrc: u32,
    /// Common header media SSRC (`0` per RFC 5104 §4.3.1.1, or target SSRC).
    pub media_ssrc: u32,
    /// FIR control entries (`ssrc`, `sequence_number`).
    pub entries: Vec<FirEntry>,
}

/// Parsed RTCP Receiver Estimated Max Bitrate packet (`PT = 206, FMT = 15, "REMB"`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtcpRemb {
    /// SSRC of the feedback sender.
    pub sender_ssrc: u32,
    /// Common header media SSRC (always `0` in REMB per spec).
    pub media_ssrc: u32,
    /// 6-bit bitrate exponent (`0..=63`).
    pub exponent: u8,
    /// 18-bit bitrate mantissa (`0..=0x3ffff`).
    pub mantissa: u32,
    /// Reconstructed maximum bitrate estimate in bits per second (`mantissa << exponent`).
    pub bitrate_bps: u64,
    /// List of media SSRCs to which this bitrate estimate applies.
    pub ssrcs: Vec<u32>,
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
    /// Transport-layer Feedback Generic NACK (`PT = 205, FMT = 1`).
    Nack(RtcpNack),
    /// Payload-specific Feedback Picture Loss Indication (`PT = 206, FMT = 1`).
    Pli(RtcpPli),
    /// Payload-specific Feedback Full Intra Request (`PT = 206, FMT = 4`).
    Fir(RtcpFir),
    /// Payload-specific Feedback Receiver Estimated Max Bitrate (`PT = 206, FMT = 15, "REMB"`).
    Remb(RtcpRemb),
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
        let available = bytes.len() - offset;

        // If we already parsed at least one packet and the remaining bytes match
        // a known SRTCP trailer shape (16, 20, 24, 32 bytes), check if they form
        // a valid subsequent RTCP packet or are ciphertext/tag/trailer.
        if offset > 0 && is_srtcp_trailer_len(available) {
            let padding = first & 0x20 != 0;
            let payload_type = bytes[offset + 1];
            let length_words = u16::from_be_bytes([bytes[offset + 2], bytes[offset + 3]]);
            let total = (usize::from(length_words) + 1) * 4;
            let minimum = minimum_len(payload_type, first & 0x1f);
            let valid =
                version == 2 && !padding && total >= minimum && offset + total <= bytes.len();
            if !valid {
                break;
            }
        }

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
            if offset > 0 && is_srtcp_trailer_len(available) {
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
            RTCP_RTPFB if report_count == RTCP_FMT_NACK => {
                RtcpPacket::Nack(parse_nack(&bytes[offset..offset + total])?)
            }
            RTCP_PSFB if report_count == RTCP_FMT_PLI => {
                RtcpPacket::Pli(parse_pli(&bytes[offset..offset + total])?)
            }
            RTCP_PSFB if report_count == RTCP_FMT_FIR => {
                RtcpPacket::Fir(parse_fir(&bytes[offset..offset + total])?)
            }
            RTCP_PSFB
                if report_count == RTCP_FMT_REMB
                    && body.get(8..12) == Some(&RTCP_REMB_IDENTIFIER) =>
            {
                RtcpPacket::Remb(parse_remb(&bytes[offset..offset + total])?)
            }
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

fn is_srtcp_trailer_len(len: usize) -> bool {
    len == 16 || len == 20 || len == 24 || len == 32
}

/// Build a 28-byte unencrypted RTCP Sender Report with 0 report blocks (RC=0).
#[must_use]
pub fn build_sender_report(ssrc: u32, info: &SenderInfo) -> [u8; RTCP_SR_CLEAR_LEN] {
    let mut bytes = [0u8; RTCP_SR_CLEAR_LEN];
    bytes[0] = 0x80; // V=2, P=0, RC=0
    bytes[1] = RTCP_SR; // 200
    bytes[2..4].copy_from_slice(&6u16.to_be_bytes()); // (28/4) - 1 = 6 words
    bytes[4..8].copy_from_slice(&ssrc.to_be_bytes());
    bytes[8..16].copy_from_slice(&info.ntp_timestamp.to_be_bytes());
    bytes[16..20].copy_from_slice(&info.rtp_timestamp.to_be_bytes());
    bytes[20..24].copy_from_slice(&info.packet_count.to_be_bytes());
    bytes[24..28].copy_from_slice(&info.octet_count.to_be_bytes());
    bytes
}

/// Build a 60-byte SRTCP Sender Report wire packet matching the Discord oracle:
/// `[8B RTCP header + SSRC][20B sender info / cipher][12B extra / trailer][16B AEAD tag][4B LE SRTCP index with E-bit]`.
#[must_use]
pub fn build_srtcp_sender_report(
    ssrc: u32,
    info: &SenderInfo,
    extra: Option<&[u8; 12]>,
    tag: [u8; 16],
    srtcp_index: u32,
) -> [u8; SRTCP_SR_WIRE_LEN] {
    let mut bytes = [0u8; SRTCP_SR_WIRE_LEN];
    bytes[0] = 0x80; // V=2, P=0, RC=0
    bytes[1] = RTCP_SR; // 200
    bytes[2..4].copy_from_slice(&6u16.to_be_bytes());
    bytes[4..8].copy_from_slice(&ssrc.to_be_bytes());
    bytes[8..16].copy_from_slice(&info.ntp_timestamp.to_be_bytes());
    bytes[16..20].copy_from_slice(&info.rtp_timestamp.to_be_bytes());
    bytes[20..24].copy_from_slice(&info.packet_count.to_be_bytes());
    bytes[24..28].copy_from_slice(&info.octet_count.to_be_bytes());
    if let Some(extra_bytes) = extra {
        bytes[28..40].copy_from_slice(extra_bytes);
    }
    bytes[40..56].copy_from_slice(&tag);
    let trailer = (srtcp_index | SRTCP_E_BIT).to_le_bytes();
    bytes[56..60].copy_from_slice(&trailer);
    bytes
}

/// Build a 32-byte unencrypted RTCP Receiver Report with 1 report block (RC=1).
#[must_use]
pub fn build_receiver_report(sender_ssrc: u32, report: &ReportBlock) -> [u8; RTCP_RR_CLEAR_LEN] {
    let mut bytes = [0u8; RTCP_RR_CLEAR_LEN];
    bytes[0] = 0x81; // V=2, P=0, RC=1
    bytes[1] = RTCP_RR; // 201
    bytes[2..4].copy_from_slice(&7u16.to_be_bytes()); // (32/4) - 1 = 7 words
    bytes[4..8].copy_from_slice(&sender_ssrc.to_be_bytes());
    bytes[8..12].copy_from_slice(&report.ssrc.to_be_bytes());
    bytes[12] = report.fraction_lost;
    let cum_bytes = report.cumulative_lost.to_be_bytes();
    bytes[13..16].copy_from_slice(&cum_bytes[1..4]);
    bytes[16..20].copy_from_slice(&report.extended_highest_seq.to_be_bytes());
    bytes[20..24].copy_from_slice(&report.jitter.to_be_bytes());
    bytes[24..28].copy_from_slice(&report.last_sr.to_be_bytes());
    bytes[28..32].copy_from_slice(&report.delay_since_last_sr.to_be_bytes());
    bytes
}

/// Build a 52-byte SRTCP Receiver Report wire packet with 1 report block (RC=1):
/// `[8B RTCP header + sender SSRC][24B report block / cipher][16B AEAD tag][4B LE counter]`.
#[must_use]
pub fn build_srtcp_receiver_report(
    sender_ssrc: u32,
    report_block_or_cipher: &[u8; 24],
    tag: [u8; 16],
    counter: u32,
) -> [u8; SRTCP_RR_WIRE_LEN] {
    let mut bytes = [0u8; SRTCP_RR_WIRE_LEN];
    bytes[0] = 0x81; // V=2, P=0, RC=1
    bytes[1] = RTCP_RR; // 201
    bytes[2..4].copy_from_slice(&7u16.to_be_bytes());
    bytes[4..8].copy_from_slice(&sender_ssrc.to_be_bytes());
    bytes[8..32].copy_from_slice(report_block_or_cipher);
    bytes[32..48].copy_from_slice(&tag);
    bytes[48..52].copy_from_slice(&counter.to_le_bytes());
    bytes
}

/// Check if bytes match the 52-byte server Receiver Report wire format.
#[must_use]
pub fn is_server_receiver_report(bytes: &[u8]) -> bool {
    bytes.len() == SRTCP_RR_WIRE_LEN
        && bytes[0] == 0x81
        && bytes[1] == RTCP_RR
        && bytes[2] == 0x00
        && bytes[3] == 0x07
}

/// Parse a 52-byte incoming server RTCP/SRTCP Receiver Report.
pub fn parse_server_receiver_report(
    bytes: &[u8],
) -> Result<(u32, ReportBlock, [u8; 16], u32), RtcpError> {
    if bytes.len() < SRTCP_RR_WIRE_LEN {
        return Err(RtcpError::Truncated {
            needed: SRTCP_RR_WIRE_LEN,
            available: bytes.len(),
        });
    }
    let first = bytes[0];
    let version = first >> 6;
    if version != 2 {
        return Err(RtcpError::InvalidVersion(version));
    }
    if first & 0x20 != 0 {
        return Err(RtcpError::UnsupportedPadding);
    }
    let report_count = first & 0x1f;
    let payload_type = bytes[1];
    let length_words = u16::from_be_bytes([bytes[2], bytes[3]]);
    if payload_type != RTCP_RR || report_count != 1 || length_words != 7 {
        return Err(RtcpError::InvalidLength {
            payload_type,
            length_words,
        });
    }
    let sender_ssrc = read_u32(bytes, 4);
    let report = ReportBlock {
        ssrc: read_u32(bytes, 8),
        fraction_lost: bytes[12],
        cumulative_lost: read_i24(bytes, 13),
        extended_highest_seq: read_u32(bytes, 16),
        jitter: read_u32(bytes, 20),
        last_sr: read_u32(bytes, 24),
        delay_since_last_sr: read_u32(bytes, 28),
    };
    let mut tag = [0u8; 16];
    tag.copy_from_slice(&bytes[32..48]);
    let counter = u32::from_le_bytes([bytes[48], bytes[49], bytes[50], bytes[51]]);
    Ok((sender_ssrc, report, tag, counter))
}

fn minimum_len(payload_type: u8, report_count: u8) -> usize {
    let reports = usize::from(report_count) * REPORT_BLOCK_LEN;
    match payload_type {
        RTCP_SR => HEADER_LEN + 24 + reports,
        RTCP_RR => HEADER_LEN + 4 + reports,
        RTCP_RTPFB if report_count == RTCP_FMT_NACK => 16,
        RTCP_PSFB if report_count == RTCP_FMT_PLI => RTCP_PLI_LEN,
        RTCP_PSFB if report_count == RTCP_FMT_FIR => RTCP_FIR_LEN,
        RTCP_PSFB if report_count == RTCP_FMT_REMB => 20,
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

/// Packs a list of lost RTP sequence numbers into compact RFC 4585 `(PID, BLP)` entries.
#[must_use]
pub fn encode_nack_entries(sequences: &[u16]) -> Vec<NackEntry> {
    if sequences.is_empty() {
        return Vec::new();
    }
    let mut unique = Vec::with_capacity(sequences.len().min(MAX_NACK_SEQUENCES));
    for &seq in sequences {
        if !unique.contains(&seq) {
            unique.push(seq);
            if unique.len() == MAX_NACK_SEQUENCES {
                break;
            }
        }
    }
    let base = unique[0];
    unique.sort_by_key(|seq| seq.wrapping_sub(base));

    let mut entries = Vec::new();
    let mut current: Option<NackEntry> = None;
    for seq in unique {
        if let Some(ref mut entry) = current {
            let delta = seq.wrapping_sub(entry.pid);
            if (1..=16).contains(&delta) {
                entry.blp |= 1u16 << (delta - 1);
                continue;
            }
            entries.push(*entry);
        }
        current = Some(NackEntry { pid: seq, blp: 0 });
    }
    if let Some(entry) = current {
        entries.push(entry);
    }
    entries
}

/// Expands `(PID, BLP)` entries into deduplicated RTP sequence numbers (bounded to [`MAX_NACK_SEQUENCES`]).
#[must_use]
pub fn decode_nack_entries(entries: &[NackEntry]) -> Vec<u16> {
    let mut out = Vec::new();
    for entry in entries {
        for offset in 0..=16u16 {
            if offset == 0 || (entry.blp & (1u16 << (offset - 1))) != 0 {
                let seq = entry.pid.wrapping_add(offset);
                if out.len() < MAX_NACK_SEQUENCES && !out.contains(&seq) {
                    out.push(seq);
                }
            }
        }
    }
    out
}

/// Builds an unencrypted RTCP Generic NACK (`PT = 205, FMT = 1`) from pre-packed [`NackEntry`] items.
#[must_use]
pub fn build_nack(sender_ssrc: u32, media_ssrc: u32, entries: &[NackEntry]) -> Vec<u8> {
    let count = entries.len().min(MAX_NACK_SEQUENCES);
    let length_words = 2u16.saturating_add(count as u16);
    let mut bytes = Vec::with_capacity(12 + count * 4);
    bytes.push(0x80 | RTCP_FMT_NACK);
    bytes.push(RTCP_RTPFB);
    bytes.extend_from_slice(&length_words.to_be_bytes());
    bytes.extend_from_slice(&sender_ssrc.to_be_bytes());
    bytes.extend_from_slice(&media_ssrc.to_be_bytes());
    for entry in entries.iter().take(count) {
        bytes.extend_from_slice(&entry.pid.to_be_bytes());
        bytes.extend_from_slice(&entry.blp.to_be_bytes());
    }
    bytes
}

/// Builds an unencrypted RTCP Generic NACK (`PT = 205, FMT = 1`) directly from lost RTP sequence numbers.
#[must_use]
pub fn build_nack_from_sequences(sender_ssrc: u32, media_ssrc: u32, sequences: &[u16]) -> Vec<u8> {
    let entries = encode_nack_entries(sequences);
    build_nack(sender_ssrc, media_ssrc, &entries)
}

fn validate_feedback_header(
    bytes: &[u8],
    expected_pt: u8,
    expected_fmt: u8,
    min_bytes: usize,
) -> Result<usize, RtcpError> {
    if bytes.len() < HEADER_LEN {
        return Err(RtcpError::Truncated {
            needed: HEADER_LEN,
            available: bytes.len(),
        });
    }
    let first = bytes[0];
    let version = first >> 6;
    if version != 2 {
        return Err(RtcpError::InvalidVersion(version));
    }
    if first & 0x20 != 0 {
        return Err(RtcpError::UnsupportedPadding);
    }
    let fmt = first & 0x1f;
    let payload_type = bytes[1];
    let length_words = u16::from_be_bytes([bytes[2], bytes[3]]);
    let total = (usize::from(length_words) + 1) * 4;
    if payload_type != expected_pt || fmt != expected_fmt || total < min_bytes {
        return Err(RtcpError::InvalidLength {
            payload_type,
            length_words,
        });
    }
    if bytes.len() < total {
        return Err(RtcpError::Truncated {
            needed: total,
            available: bytes.len(),
        });
    }
    Ok(total)
}

/// Parses an RTCP Generic NACK (`PT = 205, FMT = 1`) packet.
pub fn parse_nack(bytes: &[u8]) -> Result<RtcpNack, RtcpError> {
    let total = validate_feedback_header(bytes, RTCP_RTPFB, RTCP_FMT_NACK, 16)?;
    let sender_ssrc = read_u32(bytes, 4);
    let media_ssrc = read_u32(bytes, 8);
    let mut entries = Vec::with_capacity((total - 12) / 4);
    for chunk in bytes[12..total].as_chunks::<4>().0 {
        entries.push(NackEntry {
            pid: u16::from_be_bytes([chunk[0], chunk[1]]),
            blp: u16::from_be_bytes([chunk[2], chunk[3]]),
        });
    }
    let lost_sequences = decode_nack_entries(&entries);
    Ok(RtcpNack {
        sender_ssrc,
        media_ssrc,
        entries,
        lost_sequences,
    })
}

/// Builds a 12-byte unencrypted RTCP Picture Loss Indication (`PT = 206, FMT = 1`).
#[must_use]
pub fn build_pli(sender_ssrc: u32, media_ssrc: u32) -> [u8; RTCP_PLI_LEN] {
    let mut bytes = [0u8; RTCP_PLI_LEN];
    bytes[0] = 0x80 | RTCP_FMT_PLI;
    bytes[1] = RTCP_PSFB;
    bytes[2..4].copy_from_slice(&2u16.to_be_bytes());
    bytes[4..8].copy_from_slice(&sender_ssrc.to_be_bytes());
    bytes[8..12].copy_from_slice(&media_ssrc.to_be_bytes());
    bytes
}

/// Parses an RTCP Picture Loss Indication (`PT = 206, FMT = 1`) packet.
pub fn parse_pli(bytes: &[u8]) -> Result<RtcpPli, RtcpError> {
    let total = validate_feedback_header(bytes, RTCP_PSFB, RTCP_FMT_PLI, RTCP_PLI_LEN)?;
    if total != RTCP_PLI_LEN {
        return Err(RtcpError::InvalidLength {
            payload_type: bytes[1],
            length_words: u16::from_be_bytes([bytes[2], bytes[3]]),
        });
    }
    Ok(RtcpPli {
        sender_ssrc: read_u32(bytes, 4),
        media_ssrc: read_u32(bytes, 8),
    })
}

/// Builds a 20-byte unencrypted RTCP Full Intra Request (`PT = 206, FMT = 4`) for one media SSRC.
#[must_use]
pub fn build_fir(sender_ssrc: u32, media_ssrc: u32, sequence_number: u8) -> [u8; RTCP_FIR_LEN] {
    let mut bytes = [0u8; RTCP_FIR_LEN];
    bytes[0] = 0x80 | RTCP_FMT_FIR;
    bytes[1] = RTCP_PSFB;
    bytes[2..4].copy_from_slice(&4u16.to_be_bytes());
    bytes[4..8].copy_from_slice(&sender_ssrc.to_be_bytes());
    // Common header media_ssrc = 0 per RFC 5104 §4.3.1.1; target SSRC goes into FCI.
    bytes[12..16].copy_from_slice(&media_ssrc.to_be_bytes());
    bytes[16] = sequence_number;
    bytes
}

/// Builds an unencrypted RTCP Full Intra Request (`PT = 206, FMT = 4`) for one or more media SSRCs.
#[must_use]
pub fn build_fir_multi(sender_ssrc: u32, entries: &[FirEntry]) -> Vec<u8> {
    let count = entries.len().min(MAX_COMPOUND_PACKETS);
    let length_words = 2u16.saturating_add((count as u16).saturating_mul(2));
    let mut bytes = Vec::with_capacity(12 + count * 8);
    bytes.push(0x80 | RTCP_FMT_FIR);
    bytes.push(RTCP_PSFB);
    bytes.extend_from_slice(&length_words.to_be_bytes());
    bytes.extend_from_slice(&sender_ssrc.to_be_bytes());
    bytes.extend_from_slice(&0u32.to_be_bytes());
    for entry in entries.iter().take(count) {
        bytes.extend_from_slice(&entry.ssrc.to_be_bytes());
        bytes.extend_from_slice(&[entry.sequence_number, 0, 0, 0]);
    }
    bytes
}

/// Parses an RTCP Full Intra Request (`PT = 206, FMT = 4`) packet.
pub fn parse_fir(bytes: &[u8]) -> Result<RtcpFir, RtcpError> {
    let total = validate_feedback_header(bytes, RTCP_PSFB, RTCP_FMT_FIR, RTCP_FIR_LEN)?;
    if !(total - 12).is_multiple_of(8) {
        return Err(RtcpError::InvalidLength {
            payload_type: bytes[1],
            length_words: u16::from_be_bytes([bytes[2], bytes[3]]),
        });
    }
    let sender_ssrc = read_u32(bytes, 4);
    let media_ssrc = read_u32(bytes, 8);
    let mut entries = Vec::with_capacity((total - 12) / 8);
    for chunk in bytes[12..total].as_chunks::<8>().0 {
        entries.push(FirEntry {
            ssrc: u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]),
            sequence_number: chunk[4],
        });
    }
    Ok(RtcpFir {
        sender_ssrc,
        media_ssrc,
        entries,
    })
}

/// Splits a bitrate in bps into a 6-bit exponent (`0..=63`) and 18-bit mantissa (`0..=0x3ffff`).
#[must_use]
pub fn encode_remb_bitrate(bitrate_bps: u64) -> (u8, u32) {
    let mut mantissa = bitrate_bps;
    let mut exponent = 0u8;
    while mantissa > 0x3_ffff && exponent < 63 {
        mantissa >>= 1;
        exponent += 1;
    }
    (exponent, (mantissa & 0x3_ffff) as u32)
}

/// Reconstructs a bitrate in bps from a 6-bit exponent and 18-bit mantissa (`mantissa << exponent`).
#[must_use]
pub fn decode_remb_bitrate(exponent: u8, mantissa: u32) -> Option<u64> {
    if exponent > 63 || mantissa > 0x3_ffff {
        return None;
    }
    if mantissa == 0 {
        return Some(0);
    }
    if exponent >= 64 || u64::from(mantissa) > (u64::MAX >> exponent) {
        return None;
    }
    Some(u64::from(mantissa) << exponent)
}

/// Builds an unencrypted RTCP REMB (`PT = 206, FMT = 15, "REMB"`) packet from `bitrate_bps`.
#[must_use]
pub fn build_remb(sender_ssrc: u32, bitrate_bps: u64, ssrcs: &[u32]) -> Vec<u8> {
    let (exponent, mantissa) = encode_remb_bitrate(bitrate_bps);
    build_remb_with_parts(sender_ssrc, exponent, mantissa, ssrcs)
}

/// Builds an unencrypted RTCP REMB (`PT = 206, FMT = 15, "REMB"`) packet with explicit exponent and mantissa.
#[must_use]
pub fn build_remb_with_parts(
    sender_ssrc: u32,
    exponent: u8,
    mantissa: u32,
    ssrcs: &[u32],
) -> Vec<u8> {
    let count = ssrcs.len().min(255);
    let length_words = 4u16.saturating_add(count as u16);
    let exp = exponent & 0x3f;
    let mant = mantissa & 0x3_ffff;
    let mut bytes = Vec::with_capacity(20 + count * 4);
    bytes.push(0x80 | RTCP_FMT_REMB);
    bytes.push(RTCP_PSFB);
    bytes.extend_from_slice(&length_words.to_be_bytes());
    bytes.extend_from_slice(&sender_ssrc.to_be_bytes());
    bytes.extend_from_slice(&0u32.to_be_bytes());
    bytes.extend_from_slice(&RTCP_REMB_IDENTIFIER);
    bytes.push(count as u8);
    bytes.push((exp << 2) | ((mant >> 16) as u8 & 0x03));
    bytes.push(((mant >> 8) & 0xff) as u8);
    bytes.push((mant & 0xff) as u8);
    for &ssrc in ssrcs.iter().take(count) {
        bytes.extend_from_slice(&ssrc.to_be_bytes());
    }
    bytes
}

/// Parses an RTCP REMB (`PT = 206, FMT = 15, "REMB"`) packet.
pub fn parse_remb(bytes: &[u8]) -> Result<RtcpRemb, RtcpError> {
    let total = validate_feedback_header(bytes, RTCP_PSFB, RTCP_FMT_REMB, 20)?;
    let length_words = u16::from_be_bytes([bytes[2], bytes[3]]);
    if bytes.get(12..16) != Some(&RTCP_REMB_IDENTIFIER) {
        return Err(RtcpError::InvalidLength {
            payload_type: bytes[1],
            length_words,
        });
    }
    let num_ssrc = usize::from(bytes[16]);
    if total != 20 + num_ssrc * 4 {
        return Err(RtcpError::InvalidLength {
            payload_type: bytes[1],
            length_words,
        });
    }
    let exponent = bytes[17] >> 2;
    let mantissa =
        (u32::from(bytes[17] & 0x03) << 16) | (u32::from(bytes[18]) << 8) | u32::from(bytes[19]);
    let bitrate_bps = decode_remb_bitrate(exponent, mantissa).ok_or(RtcpError::InvalidLength {
        payload_type: bytes[1],
        length_words,
    })?;
    let mut ssrcs = Vec::with_capacity(num_ssrc);
    for chunk in bytes[20..total].as_chunks::<4>().0 {
        ssrcs.push(u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(RtcpRemb {
        sender_ssrc: read_u32(bytes, 4),
        media_ssrc: read_u32(bytes, 8),
        exponent,
        mantissa,
        bitrate_bps,
        ssrcs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i24_sign_extension() {
        assert_eq!(read_i24(&[0xff, 0xff, 0xff], 0), -1);
        assert_eq!(read_i24(&[0x7f, 0xff, 0xff], 0), 8_388_607);
    }

    #[test]
    fn rtcp_nack_builder_and_parser_roundtrip_pid_blp_and_wraparound() {
        // Serein test vector: [65535, 0, 1] -> PID=0xffff, BLP=0x0003
        let entries = encode_nack_entries(&[65535, 0, 1, 20]);
        assert_eq!(
            entries,
            vec![
                NackEntry {
                    pid: 65535,
                    blp: 0b0000_0000_0000_0011,
                },
                NackEntry { pid: 20, blp: 0 },
            ]
        );

        let raw = build_nack_from_sequences(9, 42, &[65535, 0, 1, 20]);
        assert_eq!(raw.len(), 20); // 12B header + 2 * 4B FCI
        assert_eq!(&raw[0..4], &[0x81, RTCP_RTPFB, 0x00, 0x04]);
        assert_eq!(&raw[4..8], &9u32.to_be_bytes());
        assert_eq!(&raw[8..12], &42u32.to_be_bytes());
        assert_eq!(&raw[12..16], &[0xff, 0xff, 0x00, 0x03]);
        assert_eq!(&raw[16..20], &[0x00, 20, 0x00, 0x00]);

        let parsed = parse_nack(&raw).expect("valid RTCP NACK must parse");
        assert_eq!(parsed.sender_ssrc, 9);
        assert_eq!(parsed.media_ssrc, 42);
        assert_eq!(parsed.entries, entries);
        assert_eq!(parsed.lost_sequences, vec![65535, 0, 1, 20]);
    }

    #[test]
    fn rtcp_pli_builder_and_parser_match_rfc4585_and_serein_oracle() {
        // Serein oracle (video_receive.rs:518, crypto.rs:710): [0x81, 206, 0, 2, sender_be, media_be]
        let raw = build_pli(9, 42);
        assert_eq!(raw, [0x81, 206, 0, 2, 0, 0, 0, 9, 0, 0, 0, 42]);

        let parsed = parse_pli(&raw).expect("valid RTCP PLI must parse");
        assert_eq!(
            parsed,
            RtcpPli {
                sender_ssrc: 9,
                media_ssrc: 42
            }
        );

        // PLI с неверной длиной (length_words != 2) отклоняется
        let bad_len = [0x81, 206, 0, 1, 0, 0, 0, 9];
        assert!(matches!(
            parse_pli(&bad_len),
            Err(RtcpError::InvalidLength {
                payload_type: 206,
                length_words: 1
            })
        ));
    }

    #[test]
    fn rtcp_fir_builder_and_parser_match_rfc5104() {
        let raw = build_fir(0x1111_2222, 0x3333_4444, 7);
        assert_eq!(raw.len(), RTCP_FIR_LEN);
        assert_eq!(&raw[0..4], &[0x84, RTCP_PSFB, 0x00, 0x04]); // V=2, P=0, FMT=4, PT=206, len=4
        assert_eq!(&raw[4..8], &0x1111_2222u32.to_be_bytes());
        assert_eq!(&raw[8..12], &0u32.to_be_bytes()); // media_ssrc = 0 per RFC 5104 §4.3.1.1
        assert_eq!(&raw[12..16], &0x3333_4444u32.to_be_bytes());
        assert_eq!(&raw[16..20], &[7, 0, 0, 0]);

        let parsed = parse_fir(&raw).expect("valid RTCP FIR must parse");
        assert_eq!(
            parsed,
            RtcpFir {
                sender_ssrc: 0x1111_2222,
                media_ssrc: 0,
                entries: vec![FirEntry {
                    ssrc: 0x3333_4444,
                    sequence_number: 7,
                }],
            }
        );
    }

    #[test]
    fn rtcp_remb_builder_and_parser_match_serein_mantissa_exponent_oracle() {
        // Serein vector (crypto.rs:782-785): exp=2, mantissa=250 -> 250 << 2 = 1000 bps, SSRC=42
        let raw = build_remb_with_parts(9, 2, 250, &[42]);
        assert_eq!(raw.len(), 24);
        assert_eq!(&raw[0..4], &[0x8f, RTCP_PSFB, 0x00, 0x05]);
        assert_eq!(&raw[4..8], &9u32.to_be_bytes());
        assert_eq!(&raw[8..12], &0u32.to_be_bytes());
        assert_eq!(&raw[12..16], b"REMB");
        assert_eq!(&raw[16..20], &[1, 8, 0, 250]);
        assert_eq!(&raw[20..24], &42u32.to_be_bytes());

        let parsed = parse_remb(&raw).expect("valid RTCP REMB must parse");
        assert_eq!(
            parsed,
            RtcpRemb {
                sender_ssrc: 9,
                media_ssrc: 0,
                exponent: 2,
                mantissa: 250,
                bitrate_bps: 1000,
                ssrcs: vec![42],
            }
        );

        // Проверка автоматического кодирования высокого битрейта (например, 2_500_000 bps = 2.5 Mbps)
        let high_raw = build_remb(9, 2_500_000, &[42, 43]);
        let high_parsed = parse_remb(&high_raw).expect("high bitrate REMB must parse");
        assert!(high_parsed.mantissa <= 0x3ffff);
        assert_eq!(high_parsed.ssrcs, vec![42, 43]);
        assert_eq!(
            high_parsed.bitrate_bps,
            u64::from(high_parsed.mantissa) << high_parsed.exponent
        );
        // Погрешность квантования 18-битной мантиссы < 0.002%
        assert!(high_parsed.bitrate_bps.abs_diff(2_500_000) < 64);
    }

    #[test]
    fn compound_parses_rr_pli_nack_fir_and_remb_together() {
        let mut compound_bytes = Vec::new();
        let report = ReportBlock {
            ssrc: 42,
            fraction_lost: 32,
            cumulative_lost: 1,
            extended_highest_seq: 100,
            jitter: 10,
            last_sr: 0,
            delay_since_last_sr: 0,
        };
        compound_bytes.extend_from_slice(&build_receiver_report(9, &report));
        compound_bytes.extend_from_slice(&build_pli(9, 42));
        compound_bytes.extend_from_slice(&build_nack_from_sequences(9, 42, &[65535, 0, 1]));
        compound_bytes.extend_from_slice(&build_fir(9, 42, 3));
        compound_bytes.extend_from_slice(&build_remb_with_parts(9, 2, 250, &[42]));

        let compound = parse_compound(&compound_bytes).expect("compound feedback must parse");
        assert_eq!(compound.trailer_len, 0);
        assert_eq!(compound.packets.len(), 5);
        assert!(matches!(
            compound.packets[0],
            RtcpPacket::ReceiverReport { sender_ssrc: 9, .. }
        ));
        assert_eq!(
            compound.packets[1],
            RtcpPacket::Pli(RtcpPli {
                sender_ssrc: 9,
                media_ssrc: 42
            })
        );
        assert!(matches!(
            &compound.packets[2],
            RtcpPacket::Nack(nack) if nack.lost_sequences == vec![65535, 0, 1]
        ));
        assert!(matches!(
            &compound.packets[3],
            RtcpPacket::Fir(fir) if fir.entries.len() == 1 && fir.entries[0].ssrc == 42 && fir.entries[0].sequence_number == 3
        ));
        assert!(matches!(
            &compound.packets[4],
            RtcpPacket::Remb(remb) if remb.bitrate_bps == 1000 && remb.ssrcs == vec![42]
        ));
    }

    #[test]
    fn rtcp_feedback_negative_and_boundary_validation() {
        // 1. NACK: проверка всех 16 бит BLP (pid + 1 ..= pid + 16) и начала нового блока на pid + 17
        let seqs: Vec<u16> = (100..=117).collect();
        let entries = encode_nack_entries(&seqs);
        assert_eq!(
            entries,
            vec![
                NackEntry {
                    pid: 100,
                    blp: 0xffff
                },
                NackEntry { pid: 117, blp: 0 },
            ]
        );
        assert_eq!(decode_nack_entries(&entries), seqs);

        // NACK без FCI (length_words = 2 -> 12 байт) отклоняется
        let empty_nack = [0x81, RTCP_RTPFB, 0, 2, 0, 0, 0, 1, 0, 0, 0, 2];
        assert!(matches!(
            parse_nack(&empty_nack),
            Err(RtcpError::InvalidLength {
                payload_type: RTCP_RTPFB,
                length_words: 2
            })
        ));

        // 2. FIR multi-entry и отклонение пакета с некратным 8 байтам FCI
        let fir_multi = build_fir_multi(
            10,
            &[
                FirEntry {
                    ssrc: 100,
                    sequence_number: 1,
                },
                FirEntry {
                    ssrc: 200,
                    sequence_number: 2,
                },
            ],
        );
        assert_eq!(fir_multi.len(), 28);
        let parsed_fir = parse_fir(&fir_multi).unwrap();
        assert_eq!(parsed_fir.entries.len(), 2);
        assert_eq!(parsed_fir.entries[1].ssrc, 200);
        assert_eq!(parsed_fir.entries[1].sequence_number, 2);

        let bad_fir = [
            0x84, RTCP_PSFB, 0, 3, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0,
            2, // 16 байт (FCI = 4B != 8B)
        ];
        assert!(matches!(
            parse_fir(&bad_fir),
            Err(RtcpError::InvalidLength {
                payload_type: RTCP_PSFB,
                length_words: 3
            })
        ));

        // 3. REMB: проверка верхних 2 бит 18-битной мантиссы (0x3_ffff) и отклонение чужой сигнатуры
        let max_mant_raw = build_remb_with_parts(9, 3, 0x3_ffff, &[77]);
        let max_mant_parsed = parse_remb(&max_mant_raw).unwrap();
        assert_eq!(max_mant_parsed.exponent, 3);
        assert_eq!(max_mant_parsed.mantissa, 0x3_ffff);
        assert_eq!(max_mant_parsed.bitrate_bps, 0x3_ffffu64 << 3);

        let mut bad_magic = max_mant_raw.clone();
        bad_magic[12..16].copy_from_slice(b"NOPE");
        assert!(parse_remb(&bad_magic).is_err());

        // REMB с неверным num_ssrc относительно длины пакета отклоняется
        let mut bad_count = max_mant_raw;
        bad_count[16] = 2; // заявлено 2 SSRC, но в пакете только 1
        assert!(parse_remb(&bad_count).is_err());
    }
}
