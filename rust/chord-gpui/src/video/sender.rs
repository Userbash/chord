//! P1-2.4: RTP Video Packetizer (90 kHz, MTU 1200, Marker Bit, NACK Retransmit Ring).
//!
//! Clean-room references:
//! - `docs/re/voice-deep.md` (§1, §2.3, §3: RTP v2 fixed header layout per RFC 3550,
//!   90 kHz video clock, dynamic video payload types and SSRCs).
//! - `rust/crates/chord-voice/src/rtp/video.rs` (90 kHz video clock constants and MTU bounds).

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::fmt;

/// RFC 3550 RTP protocol version (2).
pub const RTP_VERSION: u8 = 2;

/// Fixed RFC 3550 RTP v2 header size in bytes (without CSRC list or extensions).
pub const RTP_FIXED_HEADER_LEN: usize = 12;

/// Standard 90 kHz RTP clock rate for video streams (`docs/re/voice-deep.md`, RFC 3551).
pub const RTP_VIDEO_CLOCK_HZ: u32 = 90_000;

/// Default dynamic RTP payload type for Discord video (VP8 / H.264 dynamic range `96..=127`).
pub const DEFAULT_VIDEO_PAYLOAD_TYPE: u8 = 96;

/// Maximum valid 7-bit RTP payload type (`0..=127`).
pub const MAX_RTP_PAYLOAD_TYPE: u8 = 127;

/// Default maximum RTP payload bytes per packet (MTU 1200 -> 1212 bytes with 12-byte RTP header).
pub const DEFAULT_MTU_PAYLOAD_BYTES: usize = 1200;

/// Minimum allowed RTP payload MTU in bytes.
pub const MIN_MTU_PAYLOAD_BYTES: usize = 64;

/// Default maximum encoded video frame size (`2 MiB`).
pub const DEFAULT_MAX_FRAME_BYTES: usize = 2 * 1024 * 1024;

/// Default bounded NACK retransmission ring buffer capacity (512 packets, indexed by `seq % cap`).
pub const DEFAULT_NACK_RING_CAPACITY: usize = 512;

/// Errors returned by the RTP video packetizer and header codec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VideoSenderError {
    /// SSRC cannot be zero (`ssrc == 0`).
    InvalidSsrc,
    /// RTP payload type exceeds 7-bit maximum (`> 127`).
    InvalidPayloadType(u8),
    /// RTP version in header is not `2`.
    InvalidVersion(u8),
    /// Configured MTU payload size is below `MIN_MTU_PAYLOAD_BYTES` (`64`).
    InvalidMtu(usize),
    /// Configured NACK ring capacity is zero.
    InvalidNackCapacity(usize),
    /// Encoded video frame is empty (`0` bytes).
    EmptyFrame,
    /// Encoded video frame exceeds `max_frame_bytes`.
    FrameTooLarge { size: usize, max: usize },
    /// Buffer is shorter than the 12-byte RFC 3550 fixed RTP header.
    PacketTooShort { actual: usize, minimum: usize },
}

impl fmt::Display for VideoSenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSsrc => write!(f, "RTP SSRC must be non-zero"),
            Self::InvalidPayloadType(pt) => {
                write!(f, "RTP payload type {pt} exceeds 7-bit limit (127)")
            }
            Self::InvalidVersion(v) => write!(f, "unsupported RTP version {v} (expected 2)"),
            Self::InvalidMtu(mtu) => write!(
                f,
                "invalid MTU payload size {mtu} bytes (minimum is {MIN_MTU_PAYLOAD_BYTES})"
            ),
            Self::InvalidNackCapacity(cap) => {
                write!(f, "invalid NACK ring capacity {cap} (must be >= 1)")
            }
            Self::EmptyFrame => write!(f, "cannot packetize an empty video frame"),
            Self::FrameTooLarge { size, max } => {
                write!(
                    f,
                    "video frame size {size} bytes exceeds maximum {max} bytes"
                )
            }
            Self::PacketTooShort { actual, minimum } => write!(
                f,
                "RTP packet too short: got {actual} bytes, expected at least {minimum} bytes"
            ),
        }
    }
}

impl std::error::Error for VideoSenderError {}

/// RFC 3550 RTP v2 fixed 12-byte header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RtpHeader {
    /// RTP version (must be `2`).
    pub version: u8,
    /// Padding flag (`P` bit).
    pub padding: bool,
    /// Extension flag (`X` bit).
    pub extension: bool,
    /// CSRC count (`CC`, 4 bits, `0` for fixed 12-byte Discord video headers).
    pub csrc_count: u8,
    /// Marker bit (`M`, `0x80` in byte 1): `true` ONLY on the final fragment of a video frame.
    pub marker: bool,
    /// 7-bit payload type (`PT`, `0..=127`, default `96`).
    pub payload_type: u8,
    /// 16-bit sequence number (big-endian, wraps `65535 -> 0`).
    pub sequence_number: u16,
    /// 32-bit 90 kHz media timestamp (big-endian, wraps around `u32::MAX`).
    pub timestamp: u32,
    /// 32-bit synchronization source identifier (big-endian, non-zero).
    pub ssrc: u32,
}

impl RtpHeader {
    /// Creates a standard RFC 3550 v2 fixed header (`V=2, P=0, X=0, CC=0`).
    pub fn new(
        marker: bool,
        payload_type: u8,
        sequence_number: u16,
        timestamp: u32,
        ssrc: u32,
    ) -> Result<Self, VideoSenderError> {
        if ssrc == 0 {
            return Err(VideoSenderError::InvalidSsrc);
        }
        if payload_type > MAX_RTP_PAYLOAD_TYPE {
            return Err(VideoSenderError::InvalidPayloadType(payload_type));
        }
        Ok(Self {
            version: RTP_VERSION,
            padding: false,
            extension: false,
            csrc_count: 0,
            marker,
            payload_type,
            sequence_number,
            timestamp,
            ssrc,
        })
    }

    /// Serializes the RTP header into its canonical 12-byte big-endian representation.
    #[must_use]
    pub fn encode(&self) -> [u8; RTP_FIXED_HEADER_LEN] {
        let mut out = [0u8; RTP_FIXED_HEADER_LEN];
        let v_bits = (self.version & 0x03) << 6;
        let p_bit = u8::from(self.padding) << 5;
        let x_bit = u8::from(self.extension) << 4;
        let cc_bits = self.csrc_count & 0x0F;
        out[0] = v_bits | p_bit | x_bit | cc_bits;

        let m_bit = u8::from(self.marker) << 7;
        let pt_bits = self.payload_type & 0x7F;
        out[1] = m_bit | pt_bits;

        out[2..4].copy_from_slice(&self.sequence_number.to_be_bytes());
        out[4..8].copy_from_slice(&self.timestamp.to_be_bytes());
        out[8..12].copy_from_slice(&self.ssrc.to_be_bytes());
        out
    }

    /// Parses a 12-byte big-endian RFC 3550 RTP v2 header from the beginning of `buf`.
    pub fn decode(buf: &[u8]) -> Result<Self, VideoSenderError> {
        if buf.len() < RTP_FIXED_HEADER_LEN {
            return Err(VideoSenderError::PacketTooShort {
                actual: buf.len(),
                minimum: RTP_FIXED_HEADER_LEN,
            });
        }
        let b0 = buf[0];
        let version = b0 >> 6;
        if version != RTP_VERSION {
            return Err(VideoSenderError::InvalidVersion(version));
        }
        let padding = (b0 & 0x20) != 0;
        let extension = (b0 & 0x10) != 0;
        let csrc_count = b0 & 0x0F;

        let b1 = buf[1];
        let marker = (b1 & 0x80) != 0;
        let payload_type = b1 & 0x7F;

        let sequence_number = u16::from_be_bytes([buf[2], buf[3]]);
        let timestamp = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let ssrc = u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]);
        if ssrc == 0 {
            return Err(VideoSenderError::InvalidSsrc);
        }

        Ok(Self {
            version,
            padding,
            extension,
            csrc_count,
            marker,
            payload_type,
            sequence_number,
            timestamp,
            ssrc,
        })
    }
}

/// Configuration for [`VideoRtpSender`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoRtpSenderConfig {
    /// Outbound video SSRC (must be non-zero).
    pub ssrc: u32,
    /// 7-bit RTP payload type (default `96`, must be `<= 127`).
    pub payload_type: u8,
    /// Maximum RTP payload size per packet in bytes (default `1200`, minimum `64`).
    pub mtu_payload_bytes: usize,
    /// Maximum allowed encoded frame size in bytes (default `2 MiB`).
    pub max_frame_bytes: usize,
    /// Capacity of the NACK retransmission ring buffer in packets (default `512`).
    pub nack_ring_capacity: usize,
}

impl VideoRtpSenderConfig {
    /// Creates a default sender configuration for the given `ssrc` and `payload_type`.
    #[must_use]
    pub const fn new(ssrc: u32, payload_type: u8) -> Self {
        Self {
            ssrc,
            payload_type,
            mtu_payload_bytes: DEFAULT_MTU_PAYLOAD_BYTES,
            max_frame_bytes: DEFAULT_MAX_FRAME_BYTES,
            nack_ring_capacity: DEFAULT_NACK_RING_CAPACITY,
        }
    }
}

/// Telemetry counters for [`VideoRtpSender`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VideoSenderStats {
    /// Total number of complete video frames packetized.
    pub frames_sent: u64,
    /// Total number of primary RTP packets emitted across all frames.
    pub packets_sent: u64,
    /// Total bytes emitted across primary RTP packets (including 12-byte RTP headers).
    pub bytes_sent: u64,
    /// Number of RTCP NACK requests fulfilled from the retransmission ring buffer.
    pub nack_retransmits: u64,
    /// Number of RTCP NACK requests that missed (never sent or overwritten in ring buffer).
    pub nack_misses: u64,
}

#[derive(Debug, Clone)]
struct NackSlot {
    sequence_number: u16,
    packet: Vec<u8>,
}

/// Stateful 90 kHz RTP video packetizer with MTU fragmentation and bounded NACK retransmit ring.
#[derive(Debug, Clone)]
pub struct VideoRtpSender {
    config: VideoRtpSenderConfig,
    next_seq: u16,
    current_timestamp: u32,
    nack_ring: Vec<Option<NackSlot>>,
    stats: VideoSenderStats,
}

impl VideoRtpSender {
    /// Creates a new [`VideoRtpSender`] with default MTU (`1200`), max frame size (`2 MiB`),
    /// and NACK ring capacity (`512`).
    pub fn new(ssrc: u32, payload_type: u8) -> Result<Self, VideoSenderError> {
        Self::with_config(VideoRtpSenderConfig::new(ssrc, payload_type))
    }

    /// Creates a new [`VideoRtpSender`] with explicit [`VideoRtpSenderConfig`].
    pub fn with_config(config: VideoRtpSenderConfig) -> Result<Self, VideoSenderError> {
        if config.ssrc == 0 {
            return Err(VideoSenderError::InvalidSsrc);
        }
        if config.payload_type > MAX_RTP_PAYLOAD_TYPE {
            return Err(VideoSenderError::InvalidPayloadType(config.payload_type));
        }
        if config.mtu_payload_bytes < MIN_MTU_PAYLOAD_BYTES {
            return Err(VideoSenderError::InvalidMtu(config.mtu_payload_bytes));
        }
        if config.nack_ring_capacity == 0 {
            return Err(VideoSenderError::InvalidNackCapacity(
                config.nack_ring_capacity,
            ));
        }

        Ok(Self {
            config,
            next_seq: 0,
            current_timestamp: 0,
            nack_ring: vec![None; config.nack_ring_capacity],
            stats: VideoSenderStats::default(),
        })
    }

    /// Sets the initial RTP sequence number (builder style).
    #[must_use]
    pub fn with_initial_sequence_number(mut self, seq: u16) -> Self {
        self.next_seq = seq;
        self
    }

    /// Sets the initial 90 kHz RTP timestamp (builder style).
    #[must_use]
    pub fn with_initial_timestamp(mut self, timestamp: u32) -> Self {
        self.current_timestamp = timestamp;
        self
    }

    /// Returns the next RTP sequence number to be assigned.
    #[must_use]
    pub const fn next_sequence_number(&self) -> u16 {
        self.next_seq
    }

    /// Returns the current 90 kHz RTP timestamp state.
    #[must_use]
    pub const fn current_timestamp(&self) -> u32 {
        self.current_timestamp
    }

    /// Returns the configured SSRC.
    #[must_use]
    pub const fn ssrc(&self) -> u32 {
        self.config.ssrc
    }

    /// Returns the configured 7-bit RTP payload type.
    #[must_use]
    pub const fn payload_type(&self) -> u8 {
        self.config.payload_type
    }

    /// Returns the configured maximum RTP payload size per packet.
    #[must_use]
    pub const fn mtu_payload_bytes(&self) -> usize {
        self.config.mtu_payload_bytes
    }

    /// Advances the internal 90 kHz timestamp by one frame interval (`90_000 / fps`)
    /// using wrapping 32-bit arithmetic, returning the new timestamp.
    ///
    /// If `fps == 0`, advances by a full second (`90_000` ticks) to avoid division by zero.
    pub fn advance_timestamp_for_fps(&mut self, fps: u32) -> u32 {
        let delta = if fps == 0 {
            RTP_VIDEO_CLOCK_HZ
        } else {
            RTP_VIDEO_CLOCK_HZ / fps
        };
        self.current_timestamp = self.current_timestamp.wrapping_add(delta);
        self.current_timestamp
    }

    /// Splits an encoded video frame into MTU-sized RTP packets with a 12-byte RFC 3550 header,
    /// sets the marker bit (`M = 1`) ONLY on the last packet of the frame, and stores every
    /// emitted packet in the bounded NACK retransmission ring buffer.
    pub fn packetize_frame(
        &mut self,
        frame_bytes: &[u8],
        pts_90khz: u32,
    ) -> Result<Vec<Vec<u8>>, VideoSenderError> {
        if frame_bytes.is_empty() {
            return Err(VideoSenderError::EmptyFrame);
        }
        if frame_bytes.len() > self.config.max_frame_bytes {
            return Err(VideoSenderError::FrameTooLarge {
                size: frame_bytes.len(),
                max: self.config.max_frame_bytes,
            });
        }

        self.current_timestamp = pts_90khz;
        let chunks: Vec<&[u8]> = frame_bytes.chunks(self.config.mtu_payload_bytes).collect();
        let total_chunks = chunks.len();
        let mut packets = Vec::with_capacity(total_chunks);

        for (idx, chunk) in chunks.into_iter().enumerate() {
            let is_last_chunk = idx + 1 == total_chunks;
            let seq = self.next_seq;
            self.next_seq = self.next_seq.wrapping_add(1);

            let header = RtpHeader {
                version: RTP_VERSION,
                padding: false,
                extension: false,
                csrc_count: 0,
                marker: is_last_chunk,
                payload_type: self.config.payload_type,
                sequence_number: seq,
                timestamp: pts_90khz,
                ssrc: self.config.ssrc,
            };

            let mut pkt = Vec::with_capacity(RTP_FIXED_HEADER_LEN + chunk.len());
            pkt.extend_from_slice(&header.encode());
            pkt.extend_from_slice(chunk);

            // Store in bounded NACK retransmission ring buffer indexed by sequence_number
            let ring_idx = (seq as usize) % self.nack_ring.len();
            self.nack_ring[ring_idx] = Some(NackSlot {
                sequence_number: seq,
                packet: pkt.clone(),
            });

            self.stats.packets_sent += 1;
            self.stats.bytes_sent += pkt.len() as u64;
            packets.push(pkt);
        }

        self.stats.frames_sent += 1;
        Ok(packets)
    }

    /// Looks up requested `missing_seqs` in the NACK retransmission ring buffer.
    ///
    /// Returns clones of all packets still retained in the ring buffer (matching the exact
    /// `sequence_number`, ignoring stale overwritten slots) and updates `nack_retransmits`
    /// and `nack_misses` counters.
    pub fn retransmit_nack(&mut self, missing_seqs: &[u16]) -> Vec<Vec<u8>> {
        let mut out = Vec::with_capacity(missing_seqs.len());
        let cap = self.nack_ring.len();

        for &seq in missing_seqs {
            let ring_idx = (seq as usize) % cap;
            match &self.nack_ring[ring_idx] {
                Some(slot) if slot.sequence_number == seq => {
                    self.stats.nack_retransmits += 1;
                    out.push(slot.packet.clone());
                }
                _ => {
                    self.stats.nack_misses += 1;
                }
            }
        }

        out
    }

    /// Returns a reference to the cumulative sender telemetry counters.
    #[must_use]
    pub const fn stats(&self) -> &VideoSenderStats {
        &self.stats
    }
}
