//! P1-2.3: VP8/VP9 Video Encoder & Keyframe/Bitrate Controller.
//!
//! Pure-Rust (`#![forbid(unsafe_code)]`) software video frame encoder and bitstream packer
//! producing RFC 6386 VP8 and VP9 uncompressed headers, quantized downsampled luma/chroma
//! payloads bounded by target bitrate budgets, 90 kHz RTP media timestamps, monotonic 15-bit
//! `picture_id` counters, and RTCP PLI/FIR keyframe forcing.
//!
//! Clean-room references:
//! - RFC 6386 §9.1 (VP8 Uncompressed Data Chunk: 3-byte frame tag + keyframe start code
//!   `[0x9d, 0x01, 0x2a]` + 14-bit little-endian width/height).
//! - VP9 Bitstream Specification (2-bit frame marker `0b10`, `key_frame == 0` intra bit,
//!   and sync code `[0x49, 0x83, 0x42]`).
//! - `docs/re/voice-deep.md` (§1, §5: 90 kHz RTP clock, dynamic video payload types, PLI/FIR).

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::fmt;

use serde::{Deserialize, Serialize};

/// Standard 90 kHz RTP clock rate for video streams (`docs/re/voice-deep.md`, RFC 3551).
pub const RTP_VIDEO_CLOCK_HZ: u32 = 90_000;

/// Minimum supported video frame width or height in pixels (`64`).
pub const MIN_ENCODER_DIMENSION: u32 = 64;

/// Maximum supported video frame width or height in pixels (`3840`, 4K UHD).
pub const MAX_ENCODER_DIMENSION: u32 = 3840;

/// Minimum supported frame rate (`1` FPS).
pub const MIN_ENCODER_FPS: u32 = 1;

/// Maximum supported frame rate (`120` FPS).
pub const MAX_ENCODER_FPS: u32 = 120;

/// Minimum allowed encoder bitrate in bits per second (`100 kbps`).
pub const MIN_ENCODER_BITRATE_BPS: u32 = 100_000;

/// Maximum allowed encoder bitrate in bits per second (`25 Mbps`).
pub const MAX_ENCODER_BITRATE_BPS: u32 = 25_000_000;

/// Default minimum encoder bitrate (`300 kbps`).
pub const DEFAULT_MIN_BITRATE_BPS: u32 = 300_000;

/// Default target encoder bitrate (`2.5 Mbps`).
pub const DEFAULT_TARGET_BITRATE_BPS: u32 = 2_500_000;

/// Default maximum encoder bitrate (`8.0 Mbps`).
pub const DEFAULT_MAX_BITRATE_BPS: u32 = 8_000_000;

/// Default automatic keyframe cadence interval in frames (`120` frames, 2s at 60 FPS).
pub const DEFAULT_KEYFRAME_INTERVAL_FRAMES: u32 = 120;

/// 15-bit `picture_id` wraparound modulus (`0x8000 = 32768`, valid IDs `0..=0x7FFF`).
pub const PICTURE_ID_MODULO: u16 = 0x8000;

/// Bitmask for 15-bit `picture_id` (`0x7FFF`).
pub const PICTURE_ID_MASK: u16 = 0x7FFF;

/// RFC 6386 §9.1 VP8 keyframe 3-byte start code (`[0x9d, 0x01, 0x2a]`).
pub const VP8_START_CODE: [u8; 3] = [0x9d, 0x01, 0x2a];

/// VP9 keyframe uncompressed header 3-byte sync code (`[0x49, 0x83, 0x42]`).
pub const VP9_SYNC_CODE: [u8; 3] = [0x49, 0x83, 0x42];

/// Minimum encoded packet size in bytes (codec header + metadata + 4-byte CRC32).
const MIN_ENCODED_PACKET_BYTES: usize = 24;

/// Supported software video codecs for `P1-2.3`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoCodec {
    /// RFC 6386 VP8 codec.
    #[default]
    Vp8,
    /// VP9 codec (Profile 0, 8-bit 4:2:0).
    Vp9,
}

impl VideoCodec {
    /// Canonical codec name for SDP/Gateway negotiation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Vp8 => "VP8",
            Self::Vp9 => "VP9",
        }
    }
}

/// Supported input pixel formats for [`RawVideoFrame`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoFramePixelFormat {
    /// Planar YUV 4:2:0 (`Y` plane followed by `U` plane and `V` plane).
    #[default]
    I420,
    /// Semi-planar YUV 4:2:0 (`Y` plane followed by interleaved `UV` plane).
    Nv12,
    /// Packed 32-bit RGBA (`4` bytes per pixel).
    Rgba8,
}

impl VideoFramePixelFormat {
    /// Returns `true` if 4:2:0 chroma subsampling requires even width and height.
    #[must_use]
    pub const fn requires_even_dimensions(self) -> bool {
        matches!(self, Self::I420 | Self::Nv12)
    }

    /// Returns the minimum valid row stride in bytes for `width`.
    #[must_use]
    pub const fn min_stride_bytes(self, width: u32) -> u32 {
        match self {
            Self::I420 | Self::Nv12 => width,
            Self::Rgba8 => width.saturating_mul(4),
        }
    }

    /// Computes the minimum required frame buffer size in bytes for `stride_bytes` and `height`.
    #[must_use]
    pub const fn expected_buffer_bytes(self, stride_bytes: u32, height: u32) -> usize {
        let luma_bytes = (stride_bytes as usize).saturating_mul(height as usize);
        match self {
            Self::Rgba8 => luma_bytes,
            Self::I420 | Self::Nv12 => (luma_bytes.saturating_mul(3)) / 2,
        }
    }

    #[must_use]
    const fn tag_byte(self) -> u8 {
        match self {
            Self::I420 => 1,
            Self::Nv12 => 2,
            Self::Rgba8 => 3,
        }
    }
}

/// Validation and encoding errors for [`VideoEncoder`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VideoEncoderError {
    /// Width or height is outside `64..=3840`.
    InvalidDimensions { width: u32, height: u32 },
    /// Subsampled 4:2:0 format (`I420` or `NV12`) has odd width or height.
    OddSubsampledDimensions {
        width: u32,
        height: u32,
        format: VideoFramePixelFormat,
    },
    /// Frame rate is `0` or exceeds `120` FPS.
    InvalidFps(u32),
    /// One or more bitrates are outside `100_000..=25_000_000` bps.
    BitrateOutOfRange {
        min_bps: u32,
        target_bps: u32,
        max_bps: u32,
    },
    /// Bitrate hierarchy violates `min_bps <= target_bps <= max_bps`.
    InvalidBitrateOrder {
        min_bps: u32,
        target_bps: u32,
        max_bps: u32,
    },
    /// Keyframe cadence interval is `0`.
    InvalidKeyframeInterval(u32),
    /// Raw video frame buffer is empty (`0` bytes).
    EmptyFrameBuffer,
    /// Raw video frame dimensions do not match configured encoder dimensions.
    FrameDimensionMismatch {
        expected_width: u32,
        expected_height: u32,
        actual_width: u32,
        actual_height: u32,
    },
    /// Raw video frame pixel format does not match configured pixel format.
    PixelFormatMismatch {
        expected: VideoFramePixelFormat,
        actual: VideoFramePixelFormat,
    },
    /// Row stride is smaller than the minimum required for the frame width and pixel format.
    InvalidStride {
        stride_bytes: u32,
        minimum_stride_bytes: u32,
    },
    /// Frame buffer length is smaller than the required byte count for the planes.
    TruncatedFrameBuffer {
        actual_bytes: usize,
        expected_bytes: usize,
    },
}

impl fmt::Display for VideoEncoderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDimensions { width, height } => write!(
                f,
                "invalid video dimensions {width}x{height} (expected {MIN_ENCODER_DIMENSION}..={MAX_ENCODER_DIMENSION})"
            ),
            Self::OddSubsampledDimensions {
                width,
                height,
                format,
            } => write!(
                f,
                "subsampled format {format:?} requires even dimensions, got {width}x{height}"
            ),
            Self::InvalidFps(fps) => write!(
                f,
                "invalid frame rate {fps} FPS (expected {MIN_ENCODER_FPS}..={MAX_ENCODER_FPS})"
            ),
            Self::BitrateOutOfRange {
                min_bps,
                target_bps,
                max_bps,
            } => write!(
                f,
                "bitrate ({min_bps}/{target_bps}/{max_bps} bps) outside allowed range ({MIN_ENCODER_BITRATE_BPS}..={MAX_ENCODER_BITRATE_BPS})"
            ),
            Self::InvalidBitrateOrder {
                min_bps,
                target_bps,
                max_bps,
            } => write!(
                f,
                "invalid bitrate ordering: min ({min_bps}) <= target ({target_bps}) <= max ({max_bps}) required"
            ),
            Self::InvalidKeyframeInterval(interval) => {
                write!(f, "invalid keyframe interval {interval} (must be >= 1)")
            }
            Self::EmptyFrameBuffer => write!(f, "raw video frame buffer cannot be empty"),
            Self::FrameDimensionMismatch {
                expected_width,
                expected_height,
                actual_width,
                actual_height,
            } => write!(
                f,
                "frame dimensions {actual_width}x{actual_height} do not match configured {expected_width}x{expected_height}"
            ),
            Self::PixelFormatMismatch { expected, actual } => write!(
                f,
                "frame pixel format {actual:?} does not match configured {expected:?}"
            ),
            Self::InvalidStride {
                stride_bytes,
                minimum_stride_bytes,
            } => write!(
                f,
                "frame stride {stride_bytes} bytes is below minimum {minimum_stride_bytes} bytes"
            ),
            Self::TruncatedFrameBuffer {
                actual_bytes,
                expected_bytes,
            } => write!(
                f,
                "truncated frame buffer: got {actual_bytes} bytes, expected at least {expected_bytes} bytes"
            ),
        }
    }
}

impl std::error::Error for VideoEncoderError {}

/// Configuration for [`VideoEncoder`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoEncoderConfig {
    /// Target codec (`VP8` or `VP9`).
    pub codec: VideoCodec,
    /// Frame width in pixels (`64..=3840`).
    pub width: u32,
    /// Frame height in pixels (`64..=3840`).
    pub height: u32,
    /// Target frame rate (`1..=120`).
    pub fps: u32,
    /// Expected input pixel format (`I420`, `Nv12`, or `Rgba8`).
    pub pixel_format: VideoFramePixelFormat,
    /// Minimum target bitrate in bits per second (`>= 100_000`).
    pub min_bitrate_bps: u32,
    /// Initial/current target bitrate in bits per second.
    pub target_bitrate_bps: u32,
    /// Maximum target bitrate in bits per second (`<= 25_000_000`).
    pub max_bitrate_bps: u32,
    /// Automatic keyframe cadence in frames (`>= 1`).
    pub keyframe_interval_frames: u32,
}

impl Default for VideoEncoderConfig {
    fn default() -> Self {
        Self::new(VideoCodec::Vp8, 1280, 720, 60)
    }
}

impl VideoEncoderConfig {
    /// Creates a new configuration with default bitrates (`300 kbps .. 2.5 Mbps .. 8.0 Mbps`),
    /// `I420` pixel format, and `120`-frame keyframe interval.
    #[must_use]
    pub const fn new(codec: VideoCodec, width: u32, height: u32, fps: u32) -> Self {
        Self {
            codec,
            width,
            height,
            fps,
            pixel_format: VideoFramePixelFormat::I420,
            min_bitrate_bps: DEFAULT_MIN_BITRATE_BPS,
            target_bitrate_bps: DEFAULT_TARGET_BITRATE_BPS,
            max_bitrate_bps: DEFAULT_MAX_BITRATE_BPS,
            keyframe_interval_frames: DEFAULT_KEYFRAME_INTERVAL_FRAMES,
        }
    }

    /// Sets the input [`VideoFramePixelFormat`] (builder style).
    #[must_use]
    pub const fn with_pixel_format(mut self, pixel_format: VideoFramePixelFormat) -> Self {
        self.pixel_format = pixel_format;
        self
    }

    /// Sets `min_bitrate_bps`, `target_bitrate_bps`, and `max_bitrate_bps` (builder style).
    #[must_use]
    pub const fn with_bitrates(mut self, min_bps: u32, target_bps: u32, max_bps: u32) -> Self {
        self.min_bitrate_bps = min_bps;
        self.target_bitrate_bps = target_bps;
        self.max_bitrate_bps = max_bps;
        self
    }

    /// Sets `keyframe_interval_frames` (builder style).
    #[must_use]
    pub const fn with_keyframe_interval(mut self, interval_frames: u32) -> Self {
        self.keyframe_interval_frames = interval_frames;
        self
    }

    /// Validates dimensions, subsampling parity, FPS bounds, bitrate bounds/ordering, and keyframe cadence.
    pub fn validate(&self) -> Result<(), VideoEncoderError> {
        validate_dimensions(self.width, self.height, self.pixel_format)?;

        if !(MIN_ENCODER_FPS..=MAX_ENCODER_FPS).contains(&self.fps) {
            return Err(VideoEncoderError::InvalidFps(self.fps));
        }

        if self.min_bitrate_bps < MIN_ENCODER_BITRATE_BPS
            || self.min_bitrate_bps > MAX_ENCODER_BITRATE_BPS
            || self.target_bitrate_bps < MIN_ENCODER_BITRATE_BPS
            || self.target_bitrate_bps > MAX_ENCODER_BITRATE_BPS
            || self.max_bitrate_bps < MIN_ENCODER_BITRATE_BPS
            || self.max_bitrate_bps > MAX_ENCODER_BITRATE_BPS
        {
            return Err(VideoEncoderError::BitrateOutOfRange {
                min_bps: self.min_bitrate_bps,
                target_bps: self.target_bitrate_bps,
                max_bps: self.max_bitrate_bps,
            });
        }

        if self.min_bitrate_bps > self.target_bitrate_bps
            || self.target_bitrate_bps > self.max_bitrate_bps
        {
            return Err(VideoEncoderError::InvalidBitrateOrder {
                min_bps: self.min_bitrate_bps,
                target_bps: self.target_bitrate_bps,
                max_bps: self.max_bitrate_bps,
            });
        }

        if self.keyframe_interval_frames == 0 {
            return Err(VideoEncoderError::InvalidKeyframeInterval(0));
        }

        Ok(())
    }
}

fn validate_dimensions(
    width: u32,
    height: u32,
    pixel_format: VideoFramePixelFormat,
) -> Result<(), VideoEncoderError> {
    if !(MIN_ENCODER_DIMENSION..=MAX_ENCODER_DIMENSION).contains(&width)
        || !(MIN_ENCODER_DIMENSION..=MAX_ENCODER_DIMENSION).contains(&height)
    {
        return Err(VideoEncoderError::InvalidDimensions { width, height });
    }
    if pixel_format.requires_even_dimensions() && (width % 2 != 0 || height % 2 != 0) {
        return Err(VideoEncoderError::OddSubsampledDimensions {
            width,
            height,
            format: pixel_format,
        });
    }
    Ok(())
}

/// Raw uncompressed video frame submitted to [`VideoEncoder`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawVideoFrame {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Luma/packed row stride in bytes.
    pub stride_bytes: u32,
    /// Pixel format (`I420`, `Nv12`, or `Rgba8`).
    pub pixel_format: VideoFramePixelFormat,
    /// Presentation timestamp in microseconds (`pts_us`).
    pub pts_us: u64,
    /// Raw pixel plane buffer.
    pub data: Vec<u8>,
}

impl RawVideoFrame {
    /// Constructs a [`RawVideoFrame`] with an explicit row `stride_bytes`.
    #[must_use]
    pub const fn new(
        width: u32,
        height: u32,
        stride_bytes: u32,
        pixel_format: VideoFramePixelFormat,
        pts_us: u64,
        data: Vec<u8>,
    ) -> Self {
        Self {
            width,
            height,
            stride_bytes,
            pixel_format,
            pts_us,
            data,
        }
    }

    /// Constructs a tightly packed [`RawVideoFrame`] where `stride_bytes == pixel_format.min_stride_bytes(width)`.
    #[must_use]
    pub const fn new_tight(
        width: u32,
        height: u32,
        pixel_format: VideoFramePixelFormat,
        pts_us: u64,
        data: Vec<u8>,
    ) -> Self {
        let stride_bytes = pixel_format.min_stride_bytes(width);
        Self::new(width, height, stride_bytes, pixel_format, pts_us, data)
    }

    /// Returns the minimum required byte length of `data` for this frame's `stride_bytes`, `height`, and `pixel_format`.
    #[must_use]
    pub const fn expected_byte_len(&self) -> usize {
        self.pixel_format
            .expected_buffer_bytes(self.stride_bytes, self.height)
    }

    /// Validates that the buffer is non-empty, dimensions are valid, stride is sufficient,
    /// and buffer length is not truncated (`>= expected_byte_len()`).
    pub fn validate(&self) -> Result<(), VideoEncoderError> {
        if self.data.is_empty() {
            return Err(VideoEncoderError::EmptyFrameBuffer);
        }
        validate_dimensions(self.width, self.height, self.pixel_format)?;

        let min_stride = self.pixel_format.min_stride_bytes(self.width);
        if self.stride_bytes < min_stride {
            return Err(VideoEncoderError::InvalidStride {
                stride_bytes: self.stride_bytes,
                minimum_stride_bytes: min_stride,
            });
        }

        let expected_bytes = self.expected_byte_len();
        if self.data.len() < expected_bytes {
            return Err(VideoEncoderError::TruncatedFrameBuffer {
                actual_bytes: self.data.len(),
                expected_bytes,
            });
        }

        Ok(())
    }
}

/// Encoded VP8/VP9 video frame packet ready for RTP packetization via [`super::sender::VideoRtpSender`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedVideoPacket {
    /// Codec used (`VP8` or `VP9`).
    pub codec: VideoCodec,
    /// Encoded frame width in pixels.
    pub width: u32,
    /// Encoded frame height in pixels.
    pub height: u32,
    /// `true` if this packet is an independent intra keyframe.
    pub is_keyframe: bool,
    /// Zero-based sequential index of encoded frames emitted by the encoder.
    pub frame_index: u64,
    /// Monotonic 15-bit picture identifier (`0..0x8000`).
    pub picture_id: u16,
    /// Source presentation timestamp in microseconds.
    pub pts_us: u64,
    /// 90 kHz RTP media timestamp derived from `pts_us` (`(pts_us * 90) / 1000` mod `2^32`).
    pub timestamp_90khz: u32,
    /// Active target bitrate (bps) when this frame was encoded.
    pub target_bitrate_bps: u32,
    /// Maximum byte budget allocated to this frame (`target_bitrate_bps / (8 * fps)`).
    pub byte_budget: usize,
    /// Quantization step index (`1..=63`, lower = higher quality).
    pub quantizer_index: u8,
    /// 32-bit CRC32 checksum appended to the end of `payload`.
    pub checksum_crc32: u32,
    /// Complete encoded bitstream frame (RFC 6386 VP8 or VP9 uncompressed header + quantized payload + CRC32).
    pub payload: Vec<u8>,
}

/// Telemetry counters for [`VideoEncoder`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VideoEncoderStats {
    /// Total number of frames encoded.
    pub frames_encoded: u64,
    /// Total number of keyframes encoded (initial, cadence, PLI-forced, or resolution change).
    pub keyframes_encoded: u64,
    /// Total number of inter-frames encoded.
    pub inter_frames_encoded: u64,
    /// Number of keyframes explicitly forced by [`VideoEncoder::request_keyframe`] (RTCP PLI/FIR).
    pub forced_keyframes: u64,
    /// Number of resolution changes applied via `reconfigure` or dynamic resolution adaptation.
    pub resolution_changes: u64,
    /// Number of bandwidth estimates processed via [`VideoEncoder::on_bandwidth_estimate`].
    pub bandwidth_updates: u64,
    /// Cumulative encoded payload bytes emitted across all frames.
    pub bytes_encoded: u64,
    /// Current clamped target bitrate in bits per second.
    pub current_target_bitrate_bps: u32,
    /// Encoded byte length of the most recently emitted frame.
    pub last_frame_bytes: usize,
    /// Per-frame byte budget of the most recently emitted frame.
    pub last_frame_budget_bytes: usize,
    /// Most recently assigned 15-bit `picture_id`.
    pub last_picture_id: u16,
    /// Most recently computed 90 kHz RTP timestamp.
    pub last_timestamp_90khz: u32,
}

/// Converts a microsecond presentation timestamp (`pts_us`) to a 90 kHz RTP timestamp
/// (`(pts_us * 90) / 1000` modulo `2^32`), wrapping safely across `u32::MAX`.
#[must_use]
pub const fn pts_us_to_rtp_90khz(pts_us: u64) -> u32 {
    let ticks = (pts_us as u128 * 90) / 1000;
    (ticks & 0xFFFF_FFFF) as u32
}

/// Computes an IEEE 802.3 CRC32 checksum over `data` in pure safe Rust.
#[must_use]
pub fn compute_packet_crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Verifies the trailing 4-byte little-endian CRC32 checksum of an encoded frame payload.
#[must_use]
pub fn verify_packet_checksum(payload: &[u8]) -> bool {
    if payload.len() < MIN_ENCODED_PACKET_BYTES {
        return false;
    }
    let split = payload.len() - 4;
    let expected = compute_packet_crc32(&payload[..split]);
    let actual = u32::from_le_bytes([
        payload[split],
        payload[split + 1],
        payload[split + 2],
        payload[split + 3],
    ]);
    expected == actual
}

/// Parses the RFC 6386 §9.1 VP8 keyframe uncompressed header and returns `Some((width, height))`
/// if `payload` is a valid VP8 keyframe containing start code `[0x9d, 0x01, 0x2a]`.
#[must_use]
pub fn parse_vp8_keyframe_dimensions(payload: &[u8]) -> Option<(u16, u16)> {
    if payload.len() < 10 {
        return None;
    }
    // Bit 0 of byte 0 is frame_type (0 = keyframe, 1 = interframe); bit 4 is show_frame (1)
    if (payload[0] & 0x01) != 0 || (payload[0] & 0x10) == 0 {
        return None;
    }
    if payload[3..6] != VP8_START_CODE {
        return None;
    }
    let width = u16::from_le_bytes([payload[6], payload[7]]) & 0x3FFF;
    let height = u16::from_le_bytes([payload[8], payload[9]]) & 0x3FFF;
    Some((width, height))
}

/// Parses the VP9 keyframe uncompressed header and returns `Some((width, height))`
/// if `payload` starts with frame marker `0b10`, `key_frame == 0`, and sync code `[0x49, 0x83, 0x42]`.
#[must_use]
pub fn parse_vp9_keyframe_header(payload: &[u8]) -> Option<(u16, u16)> {
    if payload.len() < 9 {
        return None;
    }
    let b0 = payload[0];
    // Top 2 bits: frame_marker == 0b10 (2)
    if (b0 >> 6) != 0b10 {
        return None;
    }
    // Bit 2: key_frame == 0 (intra frame); Bit 1: show_frame == 1
    if ((b0 >> 2) & 0x01) != 0 || ((b0 >> 1) & 0x01) == 0 {
        return None;
    }
    if payload[1..4] != VP9_SYNC_CODE {
        return None;
    }
    let width_minus_1 = u16::from_be_bytes([payload[5], payload[6]]);
    let height_minus_1 = u16::from_be_bytes([payload[7], payload[8]]);
    Some((
        width_minus_1.saturating_add(1),
        height_minus_1.saturating_add(1),
    ))
}

/// Stateful VP8/VP9 software video encoder, keyframe cadence controller, and bitrate estimator adapter.
#[derive(Debug, Clone)]
pub struct VideoEncoder {
    config: VideoEncoderConfig,
    frame_index: u64,
    cadence_index: u64,
    next_picture_id: u16,
    force_next_keyframe: bool,
    pli_keyframe_pending: bool,
    reference_digest: Vec<u8>,
    stats: VideoEncoderStats,
}

impl VideoEncoder {
    /// Creates a new [`VideoEncoder`] after validating `config`.
    pub fn new(config: VideoEncoderConfig) -> Result<Self, VideoEncoderError> {
        config.validate()?;
        let stats = VideoEncoderStats {
            current_target_bitrate_bps: config.target_bitrate_bps,
            ..VideoEncoderStats::default()
        };
        Ok(Self {
            config,
            frame_index: 0,
            cadence_index: 0,
            next_picture_id: 0,
            force_next_keyframe: true,
            pli_keyframe_pending: false,
            reference_digest: Vec::new(),
            stats,
        })
    }

    /// Sets the initial 15-bit `picture_id` (masked to `0..0x8000`, builder style).
    #[must_use]
    pub const fn with_initial_picture_id(mut self, picture_id: u16) -> Self {
        self.next_picture_id = picture_id & PICTURE_ID_MASK;
        self
    }

    /// Returns a reference to the active [`VideoEncoderConfig`].
    #[must_use]
    pub const fn config(&self) -> &VideoEncoderConfig {
        &self.config
    }

    /// Returns the current clamped target bitrate in bits per second.
    #[must_use]
    pub const fn target_bitrate_bps(&self) -> u32 {
        self.config.target_bitrate_bps
    }

    /// Returns the next 15-bit `picture_id` (`0..0x8000`) to be assigned.
    #[must_use]
    pub const fn next_picture_id(&self) -> u16 {
        self.next_picture_id
    }

    /// Returns `true` if the next encoded frame is scheduled to be forced as a keyframe.
    #[must_use]
    pub const fn is_keyframe_requested(&self) -> bool {
        self.force_next_keyframe
    }

    /// Returns a reference to the cumulative [`VideoEncoderStats`].
    #[must_use]
    pub const fn stats(&self) -> &VideoEncoderStats {
        &self.stats
    }

    /// Computes the per-frame maximum byte budget proportional to `target_bitrate_bps / (8 * fps)`.
    #[must_use]
    pub fn frame_byte_budget(&self) -> usize {
        let bps = self.config.target_bitrate_bps as usize;
        let fps = (self.config.fps.max(1)) as usize;
        (bps / (8 * fps)).max(MIN_ENCODED_PACKET_BYTES)
    }

    /// Requests that the very next encoded frame be emitted as an intra keyframe
    /// (e.g. in response to incoming RTCP PLI or FIR feedback).
    pub fn request_keyframe(&mut self) {
        self.force_next_keyframe = true;
        self.pli_keyframe_pending = true;
    }

    /// Updates `target_bitrate_bps` from an incoming REMB / TWCC bandwidth estimate,
    /// clamping to `[min_bitrate_bps, max_bitrate_bps]` and updating [`VideoEncoderStats`].
    pub fn on_bandwidth_estimate(&mut self, estimated_bps: u32) -> u32 {
        let clamped = estimated_bps.clamp(self.config.min_bitrate_bps, self.config.max_bitrate_bps);
        self.config.target_bitrate_bps = clamped;
        self.stats.current_target_bitrate_bps = clamped;
        self.stats.bandwidth_updates += 1;
        clamped
    }

    /// Reconfigures the encoder. If resolution (`width` or `height`) or `codec` changes,
    /// resets the keyframe cadence and forces the next encoded frame to be a keyframe.
    pub fn reconfigure(&mut self, new_config: VideoEncoderConfig) -> Result<(), VideoEncoderError> {
        new_config.validate()?;
        let resolution_changed =
            new_config.width != self.config.width || new_config.height != self.config.height;
        let codec_changed = new_config.codec != self.config.codec;

        self.config = new_config;
        self.stats.current_target_bitrate_bps = new_config.target_bitrate_bps;

        if resolution_changed {
            self.stats.resolution_changes += 1;
        }
        if resolution_changed || codec_changed {
            self.cadence_index = 0;
            self.force_next_keyframe = true;
            self.reference_digest.clear();
        }
        Ok(())
    }

    /// Encodes a single [`RawVideoFrame`] whose dimensions and pixel format match [`Self::config`].
    pub fn encode_frame(
        &mut self,
        frame: &RawVideoFrame,
    ) -> Result<EncodedVideoPacket, VideoEncoderError> {
        frame.validate()?;

        if frame.width != self.config.width || frame.height != self.config.height {
            return Err(VideoEncoderError::FrameDimensionMismatch {
                expected_width: self.config.width,
                expected_height: self.config.height,
                actual_width: frame.width,
                actual_height: frame.height,
            });
        }
        if frame.pixel_format != self.config.pixel_format {
            return Err(VideoEncoderError::PixelFormatMismatch {
                expected: self.config.pixel_format,
                actual: frame.pixel_format,
            });
        }

        Ok(self.encode_validated_frame(frame))
    }

    /// Encodes a [`RawVideoFrame`], automatically updating configured dimensions, resetting cadence,
    /// and forcing a keyframe if the incoming frame's valid resolution differs from [`Self::config`].
    pub fn encode_frame_auto_reconfigure(
        &mut self,
        frame: &RawVideoFrame,
    ) -> Result<EncodedVideoPacket, VideoEncoderError> {
        frame.validate()?;

        if frame.pixel_format != self.config.pixel_format {
            return Err(VideoEncoderError::PixelFormatMismatch {
                expected: self.config.pixel_format,
                actual: frame.pixel_format,
            });
        }

        if frame.width != self.config.width || frame.height != self.config.height {
            self.config.width = frame.width;
            self.config.height = frame.height;
            self.cadence_index = 0;
            self.force_next_keyframe = true;
            self.reference_digest.clear();
            self.stats.resolution_changes += 1;
        }

        Ok(self.encode_validated_frame(frame))
    }

    fn encode_validated_frame(&mut self, frame: &RawVideoFrame) -> EncodedVideoPacket {
        let interval = u64::from(self.config.keyframe_interval_frames.max(1));
        let cadence_due = self.cadence_index == 0 || (self.cadence_index % interval == 0);
        let is_keyframe = self.force_next_keyframe || cadence_due;

        if self.pli_keyframe_pending {
            self.stats.forced_keyframes += 1;
            self.pli_keyframe_pending = false;
        }
        self.force_next_keyframe = false;

        let picture_id = self.next_picture_id;
        self.next_picture_id = (self.next_picture_id + 1) & PICTURE_ID_MASK;

        let timestamp_90khz = pts_us_to_rtp_90khz(frame.pts_us);
        let byte_budget = self.frame_byte_budget();
        let quantizer_index = self.compute_quantizer_index(is_keyframe);

        let header_len = match (self.config.codec, is_keyframe) {
            (VideoCodec::Vp8, true) => 10,
            (VideoCodec::Vp8, false) => 3,
            (VideoCodec::Vp9, true) => 9,
            (VideoCodec::Vp9, false) => 4,
        };
        // 4 bytes picture metadata + 4 bytes trailing CRC32
        let overhead_bytes = header_len + 4 + 4;
        let max_coeff_bytes = byte_budget.saturating_sub(overhead_bytes).max(4);
        let target_coeff_bytes = if is_keyframe {
            max_coeff_bytes.min(4096)
        } else {
            (max_coeff_bytes / 2).max(4).min(2048)
        };

        let sampled_blocks = sample_quantized_luma_chroma(
            frame,
            target_coeff_bytes,
            quantizer_index,
            is_keyframe,
            &self.reference_digest,
        );
        self.reference_digest.clone_from(&sampled_blocks);

        let partition_len = 4 + sampled_blocks.len() + 4;
        let mut payload = Vec::with_capacity(header_len + partition_len);

        match (self.config.codec, is_keyframe) {
            (VideoCodec::Vp8, true) => {
                // RFC 6386 §9.1: 3-byte frame tag (bit 0 = 0 keyframe, bit 4 = 1 show_frame, bits 5..23 = first_part_size)
                let tag: u32 = (1 << 4) | (((partition_len as u32) & 0x7FFFF) << 5);
                payload.push((tag & 0xFF) as u8);
                payload.push(((tag >> 8) & 0xFF) as u8);
                payload.push(((tag >> 16) & 0xFF) as u8);
                payload.extend_from_slice(&VP8_START_CODE);
                let w_le = ((frame.width as u16) & 0x3FFF).to_le_bytes();
                let h_le = ((frame.height as u16) & 0x3FFF).to_le_bytes();
                payload.extend_from_slice(&w_le);
                payload.extend_from_slice(&h_le);
            }
            (VideoCodec::Vp8, false) => {
                // RFC 6386 §9.1: 3-byte frame tag (bit 0 = 1 inter-frame, bit 4 = 1 show_frame)
                let tag: u32 = 1 | (1 << 4) | (((partition_len as u32) & 0x7FFFF) << 5);
                payload.push((tag & 0xFF) as u8);
                payload.push(((tag >> 8) & 0xFF) as u8);
                payload.push(((tag >> 16) & 0xFF) as u8);
            }
            (VideoCodec::Vp9, true) => {
                // VP9 uncompressed keyframe header:
                // bits 7..6 = 0b10 (frame_marker), bits 5..4 = 00 (Profile 0),
                // bit 3 = 0 (show_existing_frame), bit 2 = 0 (key_frame == 0 intra),
                // bit 1 = 1 (show_frame), bit 0 = 1 (error_resilient_mode) -> 0x83
                payload.push(0x83);
                payload.extend_from_slice(&VP9_SYNC_CODE);
                payload.push(0x00); // Profile 0 8-bit CS_BT_601 YUV420
                let w_minus_1 = ((frame.width - 1) as u16).to_be_bytes();
                let h_minus_1 = ((frame.height - 1) as u16).to_be_bytes();
                payload.extend_from_slice(&w_minus_1);
                payload.extend_from_slice(&h_minus_1);
            }
            (VideoCodec::Vp9, false) => {
                // VP9 uncompressed inter-frame header:
                // bits 7..6 = 0b10 (frame_marker), bit 2 = 1 (inter-frame), bit 1 = 1 (show_frame) -> 0x86
                payload.push(0x86);
                payload.push(quantizer_index);
                payload.extend_from_slice(&(picture_id & PICTURE_ID_MASK).to_be_bytes());
            }
        }

        // 4-byte picture metadata: 15-bit picture_id (with M=1 bit), quantizer_index, pixel_format tag
        let tagged_pic_id = (picture_id & PICTURE_ID_MASK) | 0x8000;
        payload.extend_from_slice(&tagged_pic_id.to_be_bytes());
        payload.push(quantizer_index);
        payload.push(frame.pixel_format.tag_byte());

        // Quantized luma/chroma payload bytes
        payload.extend_from_slice(&sampled_blocks);

        // Trailing 4-byte CRC32 integrity checksum
        let checksum_crc32 = compute_packet_crc32(&payload);
        payload.extend_from_slice(&checksum_crc32.to_le_bytes());

        let current_frame_index = self.frame_index;
        self.frame_index += 1;
        self.cadence_index += 1;

        self.stats.frames_encoded += 1;
        if is_keyframe {
            self.stats.keyframes_encoded += 1;
        } else {
            self.stats.inter_frames_encoded += 1;
        }
        self.stats.bytes_encoded += payload.len() as u64;
        self.stats.last_frame_bytes = payload.len();
        self.stats.last_frame_budget_bytes = byte_budget;
        self.stats.last_picture_id = picture_id;
        self.stats.last_timestamp_90khz = timestamp_90khz;

        EncodedVideoPacket {
            codec: self.config.codec,
            width: frame.width,
            height: frame.height,
            is_keyframe,
            frame_index: current_frame_index,
            picture_id,
            pts_us: frame.pts_us,
            timestamp_90khz,
            target_bitrate_bps: self.config.target_bitrate_bps,
            byte_budget,
            quantizer_index,
            checksum_crc32,
            payload,
        }
    }

    fn compute_quantizer_index(&self, is_keyframe: bool) -> u8 {
        let span = (self.config.max_bitrate_bps - self.config.min_bitrate_bps).max(1);
        let offset = self
            .config
            .target_bitrate_bps
            .saturating_sub(self.config.min_bitrate_bps);
        // Map higher bitrate -> lower quantizer step (finer detail), range 4..=56
        let ratio_permille = ((u64::from(offset) * 1000) / u64::from(span)).min(1000) as u16;
        let base_q = 56u8.saturating_sub(((ratio_permille * 52) / 1000) as u8);
        if is_keyframe {
            base_q.saturating_sub(2).max(1)
        } else {
            base_q.max(1)
        }
    }
}

fn sample_quantized_luma_chroma(
    frame: &RawVideoFrame,
    coeff_count: usize,
    quantizer_index: u8,
    is_keyframe: bool,
    reference: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(coeff_count);
    let q_step = u16::from(quantizer_index.max(1));
    let width = frame.width as usize;
    let height = frame.height as usize;
    let stride = frame.stride_bytes as usize;
    let total_pixels = (width * height).max(1);

    for idx in 0..coeff_count {
        let pixel_idx = (idx * total_pixels) / coeff_count.max(1);
        let y_row = (pixel_idx / width).min(height - 1);
        let x_col = (pixel_idx % width).min(width - 1);

        let (y_val, u_val, v_val) = sample_yuv_pixel(frame, x_col, y_row, stride, width, height);
        let raw_sample = match idx % 3 {
            0 => y_val,
            1 => u_val,
            _ => v_val,
        };

        let quantized = (((u16::from(raw_sample) + q_step / 2) / q_step) * q_step).min(255) as u8;
        let encoded_byte = if is_keyframe {
            quantized
        } else {
            let ref_byte = reference.get(idx).copied().unwrap_or(0);
            quantized.wrapping_sub(ref_byte)
        };
        out.push(encoded_byte);
    }

    out
}

fn sample_yuv_pixel(
    frame: &RawVideoFrame,
    x: usize,
    y: usize,
    stride: usize,
    width: usize,
    height: usize,
) -> (u8, u8, u8) {
    match frame.pixel_format {
        VideoFramePixelFormat::I420 => {
            let y_offset = y * stride + x;
            let y_plane_len = stride * height;
            let uv_stride = stride / 2;
            let uv_height = height / 2;
            let u_plane_len = uv_stride * uv_height;
            let uv_x = x / 2;
            let uv_y = y / 2;
            let u_offset = y_plane_len + uv_y * uv_stride + uv_x;
            let v_offset = y_plane_len + u_plane_len + uv_y * uv_stride + uv_x;

            let y_val = frame.data.get(y_offset).copied().unwrap_or(16);
            let u_val = frame.data.get(u_offset).copied().unwrap_or(128);
            let v_val = frame.data.get(v_offset).copied().unwrap_or(128);
            (y_val, u_val, v_val)
        }
        VideoFramePixelFormat::Nv12 => {
            let y_offset = y * stride + x;
            let y_plane_len = stride * height;
            let uv_base = y_plane_len + (y / 2) * stride + (x & !1);

            let y_val = frame.data.get(y_offset).copied().unwrap_or(16);
            let u_val = frame.data.get(uv_base).copied().unwrap_or(128);
            let v_val = frame.data.get(uv_base + 1).copied().unwrap_or(128);
            (y_val, u_val, v_val)
        }
        VideoFramePixelFormat::Rgba8 => {
            let base = y * stride + x * 4;
            let r = i32::from(frame.data.get(base).copied().unwrap_or(0));
            let g = i32::from(frame.data.get(base + 1).copied().unwrap_or(0));
            let b = i32::from(frame.data.get(base + 2).copied().unwrap_or(0));
            let _ = width;

            // BT.601 studio swing integer conversion
            let y_val = (((66 * r + 129 * g + 25 * b + 128) >> 8) + 16).clamp(0, 255) as u8;
            let u_val = (((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128).clamp(0, 255) as u8;
            let v_val = (((112 * r - 94 * g - 18 * b + 128) >> 8) + 128).clamp(0, 255) as u8;
            (y_val, u_val, v_val)
        }
    }
}
