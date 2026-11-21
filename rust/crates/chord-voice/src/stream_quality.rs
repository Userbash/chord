#![forbid(unsafe_code)]
//! Task R5-09: Stream Quality Profiles (`chord_voice::stream_quality`).
//!
//! Dynamic bitrate and encoder scaling for WebRTC / Go Live video streaming:
//! - Min / target / max bitrate hierarchy and validation (`150 kbps ..= 25 Mbps`).
//! - Framerate presets (`15`, `30`, `60` FPS) and 90 kHz RTP clock step calculation.
//! - Resolution pixel limits (`480p` = `409_920`, `720p` = `921_600`, `1080p` = `2_073_600`,
//!   `1440p` = `3_686_400`, `4K Source` = `8_294_400`) and aspect-ratio-preserving even-dimension
//!   YUV 4:2:0 scaling.
//! - Connection adaptation via RTCP Receiver Reports (`fraction_lost / 256` congestion drop at
//!   `>= 13/256` (~5%), recovery after `2_000 ms` holdoff at `<= 5/256` (~2%)), RTCP REMB
//!   bandwidth estimates (`4/5` media headroom, `5_000 ms` TTL), `Voice Opcode 15 MediaSinkWants`
//!   (`any` / per-SSRC quality `0..=100` and `pixelCounts`), and `ConnectionEventFramerateReducer`
//!   (`15_000 ms` static screen idle framerate drop).
//! - Vesktop `patchStreamQuality` policy (`PatchedUnrestricted` vs `FreeTierClamped` `720p@30 FPS`
//!   at `2.5 Mbps`) and `handleGoLive` `Voice Opcode 12 (Video)` announcement builder.
//!
//! Clean-room references:
//! - `docs/feature-backlog.md` row `R5-09`
//! - `docs/re/voice-deep.md` (§1, §3: `updateVideoQuality: 1280x720@30, bitrate 2.5 Mbps (+-150k),
//!   rid 100, ssrc 9209 (rtx 9210), remoteSinkWantsMaxFramerate 20`,
//!   `ConnectionEventFramerateReducer: Reduced framerate after 15000 ms`,
//!   `Remote media sink wants: {"any":100}`)
//! - `Serein/crates/discord-voice/src/video.rs` (`Rate` loss/REMB congestion controller)

use std::fmt;

use crate::rtcp::RtcpPacket;
use crate::rtp::video::{VIDEO_CLOCK_RATE, video_ticks_for_fps};
use crate::session::{MediaSinkWants, VoiceVideo, compute_stream_ssrcs};
use serde::{Deserialize, Serialize};

/// Minimum allowed video dimension (width or height) in pixels (`64`).
pub const MIN_STREAM_DIMENSION: u32 = 64;
/// Maximum allowed video dimension (width or height) in pixels (`3840`, 4K UHD).
pub const MAX_STREAM_DIMENSION: u32 = 3840;
/// Minimum allowed stream framerate (`1` FPS).
pub const MIN_STREAM_FPS: u32 = 1;
/// Maximum allowed Go Live stream framerate (`60` FPS).
pub const MAX_STREAM_FPS: u32 = 60;
/// Minimum allowed video bitrate in bits per second (`150 kbps`, `docs/re/voice-deep.md` §3).
pub const MIN_STREAM_BITRATE_BPS: u32 = 150_000;
/// Maximum allowed video bitrate in bits per second (`25 Mbps`).
pub const MAX_STREAM_BITRATE_BPS: u32 = 25_000_000;

/// Canonical target bitrate for `720p@30 FPS` (`2.5 Mbps`, `docs/re/voice-deep.md` §3).
pub const DEFAULT_720P30_TARGET_BITRATE_BPS: u32 = 2_500_000;
/// Canonical target bitrate for `720p@60 FPS` (`4.0 Mbps`).
pub const DEFAULT_720P60_TARGET_BITRATE_BPS: u32 = 4_000_000;

/// Unpatched free-tier maximum width (`1280`, `720p`).
pub const FREE_TIER_MAX_WIDTH: u32 = 1280;
/// Unpatched free-tier maximum height (`720`, `720p`).
pub const FREE_TIER_MAX_HEIGHT: u32 = 720;
/// Unpatched free-tier maximum framerate (`30` FPS).
pub const FREE_TIER_MAX_FPS: u32 = 30;
/// Unpatched free-tier maximum target bitrate (`2.5 Mbps`).
pub const FREE_TIER_MAX_TARGET_BITRATE_BPS: u32 = 2_500_000;
/// Unpatched free-tier maximum ceiling bitrate (`3.5 Mbps`).
pub const FREE_TIER_MAX_CEILING_BITRATE_BPS: u32 = 3_500_000;

/// Maximum pixel budget for `480p` (`854 * 480 = 409_920` pixels).
pub const PIXEL_LIMIT_480P: u32 = 854 * 480;
/// Maximum pixel budget for `720p` (`1280 * 720 = 921_600` pixels).
pub const PIXEL_LIMIT_720P: u32 = 1280 * 720;
/// Maximum pixel budget for `1080p` (`1920 * 1080 = 2_073_600` pixels).
pub const PIXEL_LIMIT_1080P: u32 = 1920 * 1080;
/// Maximum pixel budget for `1440p` (`2560 * 1440 = 3_686_400` pixels).
pub const PIXEL_LIMIT_1440P: u32 = 2560 * 1440;
/// Maximum pixel budget for `2160p` / `4K Source` (`3840 * 2160 = 8_294_400` pixels).
pub const PIXEL_LIMIT_2160P: u32 = 3840 * 2160;

/// RTCP `fraction_lost` (`/256`) threshold triggering congestion backoff (`13/256` ≈ `5.07%`).
pub const LOSS_CONGESTION_THRESHOLD_256: u8 = 13;
/// RTCP `fraction_lost` (`/256`) threshold permitting gradual recovery (`5/256` ≈ `1.95%`).
pub const LOSS_RECOVERY_THRESHOLD_256: u8 = 5;
/// Numerator for scaling wire REMB estimates to leave RTP/SRTP/DAVE headroom (`4`).
pub const REMB_HEADROOM_NUMERATOR: u32 = 4;
/// Denominator for scaling wire REMB estimates (`5`, i.e., `80%` media share).
pub const REMB_HEADROOM_DENOMINATOR: u32 = 5;
/// Time-to-live in milliseconds for an RTCP REMB bandwidth cap (`5_000` ms).
pub const REMB_LIMIT_TTL_MS: u64 = 5_000;
/// Holdoff duration in milliseconds after congestion before probing bitrate upward (`2_000` ms).
pub const CONGESTION_RECOVERY_HOLDOFF_MS: u64 = 2_000;
/// Minimum interval in milliseconds between congestion controller ticks (`1_000` ms).
pub const ADAPTATION_TICK_INTERVAL_MS: u64 = 1_000;
/// Static screen duration before `ConnectionEventFramerateReducer` drops FPS (`15_000` ms, `docs/re/voice-deep.md` §3).
pub const CONNECTION_IDLE_FRAMERATE_REDUCE_MS: u64 = 15_000;
/// Reduced framerate cap applied by `ConnectionEventFramerateReducer` during static screen (`20` FPS, `docs/re/voice-deep.md` §3).
pub const IDLE_REDUCED_MAX_FRAMERATE: u32 = 20;

/// Content optimization hint for Go Live video encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum StreamContentHint {
    /// Smooth video motion (games, video playback, up to 60 FPS).
    #[default]
    Motion,
    /// Crisp text readability (code, documents, terminals; favors resolution clarity over frame rate).
    Detail,
}

/// Resolution tier presets for Go Live streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum StreamResolutionPreset {
    /// `854x480` SD (`409_920` px).
    P480,
    /// `1280x720` HD (`921_600` px).
    #[default]
    P720,
    /// `1920x1080` FHD (`2_073_600` px).
    P1080,
    /// `2560x1440` QHD (`3_686_400` px).
    P1440,
    /// `3840x2160` 4K UHD Source (`8_294_400` px).
    Source4K,
}

impl StreamResolutionPreset {
    /// Returns canonical `(width, height)` dimensions for this resolution preset.
    #[must_use]
    pub const fn dimensions(self) -> (u32, u32) {
        match self {
            Self::P480 => (854, 480),
            Self::P720 => (1280, 720),
            Self::P1080 => (1920, 1080),
            Self::P1440 => (2560, 1440),
            Self::Source4K => (3840, 2160),
        }
    }

    /// Returns the maximum pixel count (`width * height`) for this resolution preset.
    #[must_use]
    pub const fn max_pixel_count(self) -> u32 {
        match self {
            Self::P480 => PIXEL_LIMIT_480P,
            Self::P720 => PIXEL_LIMIT_720P,
            Self::P1080 => PIXEL_LIMIT_1080P,
            Self::P1440 => PIXEL_LIMIT_1440P,
            Self::Source4K => PIXEL_LIMIT_2160P,
        }
    }
}

/// Framerate tier presets for Go Live streams (`15`, `30`, `60` FPS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum StreamFrameratePreset {
    /// `15` frames per second.
    Fps15,
    /// `30` frames per second.
    Fps30,
    /// `60` frames per second.
    #[default]
    Fps60,
}

impl StreamFrameratePreset {
    /// Returns the numeric framerate in frames per second.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        match self {
            Self::Fps15 => 15,
            Self::Fps30 => 30,
            Self::Fps60 => 60,
        }
    }
}

/// Combined resolution and framerate quality profile presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum StreamProfilePreset {
    P480Fps15,
    P480Fps30,
    P480Fps60,
    P720Fps15,
    P720Fps30,
    #[default]
    P720Fps60,
    P1080Fps15,
    P1080Fps30,
    P1080Fps60,
    P1440Fps15,
    P1440Fps30,
    P1440Fps60,
    Source4KFps30,
    Source4KFps60,
}

impl StreamProfilePreset {
    /// Returns the resolution preset component.
    #[must_use]
    pub const fn resolution(self) -> StreamResolutionPreset {
        match self {
            Self::P480Fps15 | Self::P480Fps30 | Self::P480Fps60 => StreamResolutionPreset::P480,
            Self::P720Fps15 | Self::P720Fps30 | Self::P720Fps60 => StreamResolutionPreset::P720,
            Self::P1080Fps15 | Self::P1080Fps30 | Self::P1080Fps60 => StreamResolutionPreset::P1080,
            Self::P1440Fps15 | Self::P1440Fps30 | Self::P1440Fps60 => StreamResolutionPreset::P1440,
            Self::Source4KFps30 | Self::Source4KFps60 => StreamResolutionPreset::Source4K,
        }
    }

    /// Returns the framerate preset component.
    #[must_use]
    pub const fn framerate(self) -> StreamFrameratePreset {
        match self {
            Self::P480Fps15 | Self::P720Fps15 | Self::P1080Fps15 | Self::P1440Fps15 => {
                StreamFrameratePreset::Fps15
            }
            Self::P480Fps30
            | Self::P720Fps30
            | Self::P1080Fps30
            | Self::P1440Fps30
            | Self::Source4KFps30 => StreamFrameratePreset::Fps30,
            Self::P480Fps60
            | Self::P720Fps60
            | Self::P1080Fps60
            | Self::P1440Fps60
            | Self::Source4KFps60 => StreamFrameratePreset::Fps60,
        }
    }

    /// Returns canonical `(min_bitrate_bps, target_bitrate_bps, max_bitrate_bps)` for this preset.
    #[must_use]
    pub const fn bitrate_bounds_bps(self) -> (u32, u32, u32) {
        match self {
            Self::P480Fps15 => (200_000, 800_000, 1_200_000),
            Self::P480Fps30 => (250_000, 1_200_000, 2_000_000),
            Self::P480Fps60 => (300_000, 2_000_000, 3_000_000),
            Self::P720Fps15 => (300_000, 1_500_000, 2_500_000),
            Self::P720Fps30 => (350_000, DEFAULT_720P30_TARGET_BITRATE_BPS, 3_500_000),
            Self::P720Fps60 => (400_000, DEFAULT_720P60_TARGET_BITRATE_BPS, 6_000_000),
            Self::P1080Fps15 => (400_000, 3_000_000, 4_500_000),
            Self::P1080Fps30 => (500_000, 5_000_000, 8_000_000),
            Self::P1080Fps60 => (600_000, 8_000_000, 12_000_000),
            Self::P1440Fps15 => (800_000, 5_000_000, 7_000_000),
            Self::P1440Fps30 => (1_000_000, 8_000_000, 12_000_000),
            Self::P1440Fps60 => (1_200_000, 12_000_000, 16_000_000),
            Self::Source4KFps30 => (1_500_000, 14_000_000, 20_000_000),
            Self::Source4KFps60 => (2_000_000, 18_000_000, 25_000_000),
        }
    }
}

/// Vesktop `patchStreamQuality` client entitlement policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GoLiveQualityPatchPolicy {
    /// Standard unpatched free tier: clamps resolution to `<= 1280x720`, FPS to `<= 30`,
    /// and target bitrate to `<= 2.5 Mbps` (`max <= 3.5 Mbps`).
    FreeTierClamped,
    /// Vesktop `patchStreamQuality` active (or Nitro unlocked): allows full selected profile
    /// (`1080p`, `1440p`, `4K Source`, `60 FPS`, up to `25 Mbps`).
    #[default]
    PatchedUnrestricted,
}

/// Validation and adaptation errors for stream quality profiles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamQualityError {
    /// Width or height is outside `MIN_STREAM_DIMENSION..=MAX_STREAM_DIMENSION` (`64..=3840`).
    InvalidDimensions { width: u32, height: u32 },
    /// Width or height is odd (YUV 4:2:0 encoders require even dimensions).
    OddDimensions { width: u32, height: u32 },
    /// Framerate is outside `MIN_STREAM_FPS..=MAX_STREAM_FPS` (`1..=60`).
    InvalidFramerate(u32),
    /// One or more bitrates fall outside `MIN_STREAM_BITRATE_BPS..=MAX_STREAM_BITRATE_BPS` (`150_000..=25_000_000`).
    BitrateOutOfRange {
        min_bps: u32,
        target_bps: u32,
        max_bps: u32,
    },
    /// Bitrate hierarchy violates `min_bps <= target_bps <= max_bps`.
    InvalidBitrateHierarchy {
        min_bps: u32,
        target_bps: u32,
        max_bps: u32,
    },
    /// Base audio SSRC is zero (`0`).
    InvalidSsrc,
}

impl fmt::Display for StreamQualityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDimensions { width, height } => write!(
                f,
                "invalid stream dimensions {width}x{height} (expected {MIN_STREAM_DIMENSION}..={MAX_STREAM_DIMENSION})"
            ),
            Self::OddDimensions { width, height } => write!(
                f,
                "stream dimensions {width}x{height} must be even for YUV 4:2:0 encoding"
            ),
            Self::InvalidFramerate(fps) => write!(
                f,
                "invalid stream framerate {fps} FPS (expected {MIN_STREAM_FPS}..={MAX_STREAM_FPS})"
            ),
            Self::BitrateOutOfRange {
                min_bps,
                target_bps,
                max_bps,
            } => write!(
                f,
                "stream bitrates ({min_bps}/{target_bps}/{max_bps} bps) outside allowed range ({MIN_STREAM_BITRATE_BPS}..={MAX_STREAM_BITRATE_BPS})"
            ),
            Self::InvalidBitrateHierarchy {
                min_bps,
                target_bps,
                max_bps,
            } => write!(
                f,
                "invalid bitrate hierarchy: min ({min_bps}) <= target ({target_bps}) <= max ({max_bps}) required"
            ),
            Self::InvalidSsrc => f.write_str("stream audio SSRC must be non-zero"),
        }
    }
}

impl std::error::Error for StreamQualityError {}

/// Validated stream quality profile defining resolution bounds, pixel budget, framerate, and bitrate envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamQualityProfile {
    /// Target maximum width in pixels (`64..=3840`, even).
    pub width: u32,
    /// Target maximum height in pixels (`64..=3840`, even).
    pub height: u32,
    /// Maximum pixel budget (`width * height`).
    pub max_pixel_count: u32,
    /// Target maximum framerate (`1..=60` FPS).
    pub fps: u32,
    /// Minimum encoder bitrate in bits per second (`>= 150_000`).
    pub min_bitrate_bps: u32,
    /// Nominal target encoder bitrate in bits per second.
    pub target_bitrate_bps: u32,
    /// Maximum ceiling encoder bitrate in bits per second (`<= 25_000_000`).
    pub max_bitrate_bps: u32,
    /// Content optimization hint (`Motion` vs `Detail`).
    pub content_hint: StreamContentHint,
}

impl Default for StreamQualityProfile {
    fn default() -> Self {
        Self::from_preset(StreamProfilePreset::P720Fps60)
    }
}

impl StreamQualityProfile {
    /// Constructs a canonical [`StreamQualityProfile`] from a [`StreamProfilePreset`].
    #[must_use]
    pub const fn from_preset(preset: StreamProfilePreset) -> Self {
        let res = preset.resolution();
        let (width, height) = res.dimensions();
        let max_pixel_count = res.max_pixel_count();
        let fps = preset.framerate().as_u32();
        let (min_bitrate_bps, target_bitrate_bps, max_bitrate_bps) = preset.bitrate_bounds_bps();
        let content_hint = if fps >= 60 {
            StreamContentHint::Motion
        } else {
            StreamContentHint::Detail
        };
        Self {
            width,
            height,
            max_pixel_count,
            fps,
            min_bitrate_bps,
            target_bitrate_bps,
            max_bitrate_bps,
            content_hint,
        }
    }

    /// Constructs and validates a custom [`StreamQualityProfile`].
    pub fn custom(
        width: u32,
        height: u32,
        fps: u32,
        min_bitrate_bps: u32,
        target_bitrate_bps: u32,
        max_bitrate_bps: u32,
        content_hint: StreamContentHint,
    ) -> Result<Self, StreamQualityError> {
        let profile = Self {
            width,
            height,
            max_pixel_count: width.saturating_mul(height),
            fps,
            min_bitrate_bps,
            target_bitrate_bps,
            max_bitrate_bps,
            content_hint,
        };
        profile.validate()?;
        Ok(profile)
    }

    /// Validates all profile constraints (dimensions, parity, FPS, bitrate bounds and hierarchy).
    pub fn validate(&self) -> Result<(), StreamQualityError> {
        if !(MIN_STREAM_DIMENSION..=MAX_STREAM_DIMENSION).contains(&self.width)
            || !(MIN_STREAM_DIMENSION..=MAX_STREAM_DIMENSION).contains(&self.height)
        {
            return Err(StreamQualityError::InvalidDimensions {
                width: self.width,
                height: self.height,
            });
        }
        if self.width % 2 != 0 || self.height % 2 != 0 {
            return Err(StreamQualityError::OddDimensions {
                width: self.width,
                height: self.height,
            });
        }
        if !(MIN_STREAM_FPS..=MAX_STREAM_FPS).contains(&self.fps) {
            return Err(StreamQualityError::InvalidFramerate(self.fps));
        }
        if !(MIN_STREAM_BITRATE_BPS..=MAX_STREAM_BITRATE_BPS).contains(&self.min_bitrate_bps)
            || !(MIN_STREAM_BITRATE_BPS..=MAX_STREAM_BITRATE_BPS).contains(&self.target_bitrate_bps)
            || !(MIN_STREAM_BITRATE_BPS..=MAX_STREAM_BITRATE_BPS).contains(&self.max_bitrate_bps)
        {
            return Err(StreamQualityError::BitrateOutOfRange {
                min_bps: self.min_bitrate_bps,
                target_bps: self.target_bitrate_bps,
                max_bps: self.max_bitrate_bps,
            });
        }
        if self.min_bitrate_bps > self.target_bitrate_bps
            || self.target_bitrate_bps > self.max_bitrate_bps
        {
            return Err(StreamQualityError::InvalidBitrateHierarchy {
                min_bps: self.min_bitrate_bps,
                target_bps: self.target_bitrate_bps,
                max_bps: self.max_bitrate_bps,
            });
        }
        Ok(())
    }

    /// Applies [`GoLiveQualityPatchPolicy`] clamping: when `FreeTierClamped`, caps resolution to
    /// `1280x720`, FPS to `30`, target bitrate to `2.5 Mbps`, and max bitrate to `3.5 Mbps`.
    #[must_use]
    pub fn apply_patch_policy(self, policy: GoLiveQualityPatchPolicy) -> Self {
        match policy {
            GoLiveQualityPatchPolicy::PatchedUnrestricted => self,
            GoLiveQualityPatchPolicy::FreeTierClamped => {
                let (width, height) = scale_dimensions_to_pixel_limit(
                    self.width,
                    self.height,
                    FREE_TIER_MAX_WIDTH,
                    FREE_TIER_MAX_HEIGHT,
                    PIXEL_LIMIT_720P,
                );
                let fps = self.fps.min(FREE_TIER_MAX_FPS);
                let max_bitrate_bps = self.max_bitrate_bps.min(FREE_TIER_MAX_CEILING_BITRATE_BPS);
                let target_bitrate_bps = self
                    .target_bitrate_bps
                    .min(FREE_TIER_MAX_TARGET_BITRATE_BPS)
                    .min(max_bitrate_bps);
                let min_bitrate_bps = self.min_bitrate_bps.min(target_bitrate_bps);
                Self {
                    width,
                    height,
                    max_pixel_count: width * height,
                    fps,
                    min_bitrate_bps,
                    target_bitrate_bps,
                    max_bitrate_bps,
                    content_hint: self.content_hint,
                }
            }
        }
    }

    /// Returns the 90 kHz RTP media timestamp increment per frame for this profile's framerate.
    #[must_use]
    pub const fn rtp_timestamp_step_90khz(&self) -> u32 {
        video_ticks_for_fps(self.fps)
    }

    /// Returns the target frame interval in microseconds (`1_000_000 / fps`).
    #[must_use]
    pub const fn frame_interval_us(&self) -> u64 {
        let fps = if self.fps == 0 { 1 } else { self.fps as u64 };
        1_000_000 / fps
    }
}

/// Scales `(source_width, source_height)` proportionally so that it fits within `(max_width, max_height)`
/// AND total pixels `<= max_pixel_limit`, rounding to even dimensions (`>= 64x64`) for YUV 4:2:0 encoders.
/// Never upscales a source smaller than the target bounds.
#[must_use]
pub fn scale_dimensions_to_pixel_limit(
    source_width: u32,
    source_height: u32,
    max_width: u32,
    max_height: u32,
    max_pixel_limit: u32,
) -> (u32, u32) {
    let src_w = source_width.clamp(MIN_STREAM_DIMENSION, MAX_STREAM_DIMENSION);
    let src_h = source_height.clamp(MIN_STREAM_DIMENSION, MAX_STREAM_DIMENSION);
    let cap_w = max_width.clamp(MIN_STREAM_DIMENSION, MAX_STREAM_DIMENSION);
    let cap_h = max_height.clamp(MIN_STREAM_DIMENSION, MAX_STREAM_DIMENSION);
    let pixel_cap = u64::from(max_pixel_limit.max(MIN_STREAM_DIMENSION * MIN_STREAM_DIMENSION));

    let (mut w, mut h) = fit_bounding_box(u64::from(src_w), u64::from(src_h), u64::from(cap_w), u64::from(cap_h));
    let current_pixels = w * h;
    if current_pixels > pixel_cap {
        let scale_num = integer_sqrt_u64(pixel_cap * 1_000_000 / current_pixels.max(1));
        w = (w * scale_num) / 1_000;
        h = (h * scale_num) / 1_000;
    }

    let mut even_w = ((w as u32) & !1).clamp(MIN_STREAM_DIMENSION, cap_w & !1);
    let mut even_h = ((h as u32) & !1).clamp(MIN_STREAM_DIMENSION, cap_h & !1);
    while u64::from(even_w) * u64::from(even_h) > pixel_cap
        && even_w > MIN_STREAM_DIMENSION
        && even_h > MIN_STREAM_DIMENSION
    {
        even_w = (even_w - 2).max(MIN_STREAM_DIMENSION);
        even_h = (even_h - 2).max(MIN_STREAM_DIMENSION);
    }
    (even_w, even_h)
}

const fn fit_bounding_box(mut w: u64, mut h: u64, cap_w: u64, cap_h: u64) -> (u64, u64) {
    if w > cap_w {
        h = (h * cap_w) / if w == 0 { 1 } else { w };
        w = cap_w;
    }
    if h > cap_h {
        w = (w * cap_h) / if h == 0 { 1 } else { h };
        h = cap_h;
    }
    (w, h)
}

const fn integer_sqrt_u64(n: u64) -> u64 {
    if n < 2 {
        return n;
    }
    let mut x0 = n / 2;
    let mut x1 = (x0 + n / x0) / 2;
    while x1 < x0 {
        x0 = x1;
        x1 = (x0 + n / x0) / 2;
    }
    x0
}

/// Lifecycle state of the adaptive stream quality controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum StreamAdaptationState {
    /// Stream has not started Go Live yet.
    #[default]
    Idle,
    /// Stream is active and operating at its full selected quality profile.
    Active,
    /// Stream resolution, framerate, or bitrate is currently scaled down due to network congestion or sink wants.
    Degraded,
    /// Stream encoding is temporarily paused because all remote sinks requested `quality == 0`.
    PausedBySink,
}

/// Evaluated encoder parameters for the current frame/segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncoderScaleOutput {
    /// Whether video encoding should actively emit frames (`false` when paused by sink wants `0`).
    pub active: bool,
    /// Adapted even width in pixels (`64..=3840`).
    pub encoded_width: u32,
    /// Adapted even height in pixels (`64..=3840`).
    pub encoded_height: u32,
    /// Effective pixel limit applied during scaling.
    pub effective_pixel_limit: u32,
    /// Adapted target framerate (`0` when paused, else `1..=60`).
    pub encoded_fps: u32,
    /// Adapted target bitrate in bits per second (`min_bitrate_bps..=max_bitrate_bps`).
    pub target_bitrate_bps: u32,
    /// Per-frame byte budget (`target_bitrate_bps / (8 * encoded_fps)`).
    pub frame_byte_budget: usize,
    /// 90 kHz RTP timestamp step (`90_000 / encoded_fps`).
    pub rtp_timestamp_step_90khz: u32,
}

/// Sensitive Go Live session context whose `stream_key` and `rtc_token` are redacted in `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct GoLiveStreamContext {
    stream_key: String,
    rtc_token: String,
    audio_ssrc: u32,
    video_ssrc: u32,
    rtx_ssrc: u32,
}

impl GoLiveStreamContext {
    /// Creates a new [`GoLiveStreamContext`] deriving `video_ssrc = audio_ssrc + 1` and `rtx_ssrc = audio_ssrc + 2`.
    #[must_use]
    pub fn new(
        stream_key: impl Into<String>,
        rtc_token: impl Into<String>,
        audio_ssrc: u32,
    ) -> Self {
        let (video_ssrc, rtx_ssrc) = compute_stream_ssrcs(audio_ssrc);
        Self {
            stream_key: stream_key.into(),
            rtc_token: rtc_token.into(),
            audio_ssrc,
            video_ssrc,
            rtx_ssrc,
        }
    }

    #[must_use]
    pub fn stream_key(&self) -> &str {
        &self.stream_key
    }

    #[must_use]
    pub fn rtc_token(&self) -> &str {
        &self.rtc_token
    }

    #[must_use]
    pub const fn audio_ssrc(&self) -> u32 {
        self.audio_ssrc
    }

    #[must_use]
    pub const fn video_ssrc(&self) -> u32 {
        self.video_ssrc
    }

    #[must_use]
    pub const fn rtx_ssrc(&self) -> u32 {
        self.rtx_ssrc
    }
}

impl fmt::Debug for GoLiveStreamContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GoLiveStreamContext")
            .field("stream_key", &"<redacted>")
            .field("rtc_token", &"<redacted>")
            .field("audio_ssrc", &self.audio_ssrc)
            .field("video_ssrc", &self.video_ssrc)
            .field("rtx_ssrc", &self.rtx_ssrc)
            .finish()
    }
}

/// Observable telemetry counters for [`StreamQualityController`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamQualityTelemetry {
    /// Number of `handle_go_live` stream initializations.
    pub go_live_starts: u64,
    /// Number of `Voice Opcode 12 (Video)` frames generated.
    pub op12_frames_emitted: u64,
    /// Number of profile or patch policy updates applied.
    pub profile_updates: u64,
    /// Number of bitrate adaptation ticks that changed `current_target_bitrate_bps`.
    pub bandwidth_adaptations: u64,
    /// Number of congestion backoff events (`fraction_lost >= 13/256` or REMB drop).
    pub congestion_backoffs: u64,
    /// Number of upward bitrate recovery steps after sustained low loss.
    pub recovery_steps: u64,
    /// Number of `Voice Opcode 15 MediaSinkWants` updates processed.
    pub sink_wants_updates: u64,
    /// Number of dynamic resolution scaling transitions.
    pub resolution_scalings: u64,
    /// Number of static screen idle framerate reductions (`>= 15_000 ms`).
    pub idle_framerate_reductions: u64,
}

/// Stateful stream quality profile and bandwidth adaptation controller (`Task R5-09`).
#[derive(Debug, Clone)]
pub struct StreamQualityController {
    audio_ssrc: u32,
    video_ssrc: u32,
    rtx_ssrc: u32,
    selected_profile: StreamQualityProfile,
    effective_profile: StreamQualityProfile,
    patch_policy: GoLiveQualityPatchPolicy,
    state: StreamAdaptationState,
    current_target_bitrate_bps: u32,
    sink_wants_quality: u32,
    sink_wants_pixel_limit: Option<u32>,
    remote_sink_wants_max_fps: Option<u32>,
    static_screen_idle_ms: u64,
    pending_loss_256: Option<u8>,
    pending_remb_bps: Option<u32>,
    active_remb_limit: Option<(u32, u64)>,
    last_tick_at_ms: Option<u64>,
    last_congestion_at_ms: u64,
    last_encoded_dimensions: Option<(u32, u32)>,
    keyframe_requested: bool,
    telemetry: StreamQualityTelemetry,
}

impl StreamQualityController {
    /// Creates a new [`StreamQualityController`] for `audio_ssrc`, validating `selected_profile`
    /// and applying `patch_policy`.
    pub fn new(
        audio_ssrc: u32,
        selected_profile: StreamQualityProfile,
        patch_policy: GoLiveQualityPatchPolicy,
    ) -> Result<Self, StreamQualityError> {
        if audio_ssrc == 0 {
            return Err(StreamQualityError::InvalidSsrc);
        }
        selected_profile.validate()?;
        let (video_ssrc, rtx_ssrc) = compute_stream_ssrcs(audio_ssrc);
        let effective_profile = selected_profile.apply_patch_policy(patch_policy);
        Ok(Self {
            audio_ssrc,
            video_ssrc,
            rtx_ssrc,
            selected_profile,
            effective_profile,
            patch_policy,
            state: StreamAdaptationState::Idle,
            current_target_bitrate_bps: effective_profile.target_bitrate_bps,
            sink_wants_quality: 100,
            sink_wants_pixel_limit: None,
            remote_sink_wants_max_fps: None,
            static_screen_idle_ms: 0,
            pending_loss_256: None,
            pending_remb_bps: None,
            active_remb_limit: None,
            last_tick_at_ms: None,
            last_congestion_at_ms: 0,
            last_encoded_dimensions: None,
            keyframe_requested: true,
            telemetry: StreamQualityTelemetry::default(),
        })
    }

    #[must_use]
    pub const fn audio_ssrc(&self) -> u32 {
        self.audio_ssrc
    }

    #[must_use]
    pub const fn video_ssrc(&self) -> u32 {
        self.video_ssrc
    }

    #[must_use]
    pub const fn rtx_ssrc(&self) -> u32 {
        self.rtx_ssrc
    }

    #[must_use]
    pub const fn state(&self) -> StreamAdaptationState {
        self.state
    }

    #[must_use]
    pub const fn selected_profile(&self) -> &StreamQualityProfile {
        &self.selected_profile
    }

    #[must_use]
    pub const fn effective_profile(&self) -> &StreamQualityProfile {
        &self.effective_profile
    }

    #[must_use]
    pub const fn current_target_bitrate_bps(&self) -> u32 {
        self.current_target_bitrate_bps
    }

    #[must_use]
    pub const fn telemetry(&self) -> &StreamQualityTelemetry {
        &self.telemetry
    }

    /// Takes and clears the pending keyframe request flag.
    pub fn take_keyframe_requested(&mut self) -> bool {
        let req = self.keyframe_requested;
        self.keyframe_requested = false;
        req
    }

    /// Vesktop `handleGoLive`: transitions stream to active state, scales source dimensions to the
    /// effective profile's pixel limit, and emits a `Voice Opcode 12 (Video)` frame.
    pub fn handle_go_live(&mut self, source_width: u32, source_height: u32) -> VoiceVideo {
        self.telemetry.go_live_starts += 1;
        self.keyframe_requested = true;
        self.refresh_adaptation_state();
        let out = self.evaluate_encoder_Params(source_width, source_height);
        self.build_op12_video(out.encoded_width, out.encoded_height, out.encoded_fps.max(1))
    }

    /// Vesktop `patchStreamQuality`: dynamically updates [`GoLiveQualityPatchPolicy`] mid-stream,
    /// recalculates the effective profile, requests an IDR keyframe, and returns an updated `OP 12 VIDEO`.
    pub fn patch_stream_quality(&mut self, policy: GoLiveQualityPatchPolicy) -> VoiceVideo {
        self.patch_policy = policy;
        self.effective_profile = self.selected_profile.apply_patch_policy(policy);
        self.current_target_bitrate_bps = self.effective_profile.target_bitrate_bps;
        self.keyframe_requested = true;
        self.telemetry.profile_updates += 1;
        self.refresh_adaptation_state();
        self.build_op12_video(
            self.effective_profile.width,
            self.effective_profile.height,
            self.effective_profile.fps,
        )
    }

    /// Updates the user-selected [`StreamQualityProfile`] mid-stream.
    pub fn set_quality_profile(
        &mut self,
        new_profile: StreamQualityProfile,
    ) -> Result<VoiceVideo, StreamQualityError> {
        new_profile.validate()?;
        self.selected_profile = new_profile;
        self.effective_profile = new_profile.apply_patch_policy(self.patch_policy);
        self.current_target_bitrate_bps = self.effective_profile.target_bitrate_bps;
        self.keyframe_requested = true;
        self.telemetry.profile_updates += 1;
        self.refresh_adaptation_state();
        Ok(self.build_op12_video(
            self.effective_profile.width,
            self.effective_profile.height,
            self.effective_profile.fps,
        ))
    }

    /// Applies an incoming `Voice Opcode 15 (MediaSinkWants)` frame (`any`, per-SSRC quality, `pixelCounts`).
    pub fn apply_media_sink_wants(&mut self, wants: &MediaSinkWants) {
        self.telemetry.sink_wants_updates += 1;
        let ssrc_q = wants
            .ssrc_qualities
            .get(&self.video_ssrc)
            .copied()
            .or(wants.any)
            .unwrap_or(100);
        self.sink_wants_quality = ssrc_q.min(100);
        self.sink_wants_pixel_limit = wants.pixel_counts.filter(|&pc| pc > 0);
        self.refresh_adaptation_state();
    }

    /// Updates the static screen idle timer for `ConnectionEventFramerateReducer` (`15_000 ms` threshold).
    pub fn update_static_screen_idle_ms(&mut self, idle_ms: u64) {
        let was_reduced = self.static_screen_idle_ms >= CONNECTION_IDLE_FRAMERATE_REDUCE_MS;
        self.static_screen_idle_ms = idle_ms;
        let is_reduced = self.static_screen_idle_ms >= CONNECTION_IDLE_FRAMERATE_REDUCE_MS;
        if !was_reduced && is_reduced {
            self.telemetry.idle_framerate_reductions += 1;
        }
    }

    /// Inspects an incoming [`RtcpPacket`] (`ReceiverReport` or `Remb`) for this stream's `video_ssrc`.
    pub fn on_rtcp_packet(&mut self, packet: &RtcpPacket, now_ms: u64) {
        match packet {
            RtcpPacket::ReceiverReport { reports, .. }
            | RtcpPacket::SenderReport { reports, .. } => {
                for block in reports {
                    if block.ssrc == self.video_ssrc {
                        self.observe_network_feedback(Some(block.fraction_lost), None, now_ms);
                    }
                }
            }
            RtcpPacket::Remb(remb) => {
                if remb.ssrcs.is_empty() || remb.ssrcs.contains(&self.video_ssrc) {
                    let wire_bps = u32::try_from(remb.bitrate_bps).unwrap_or(u32::MAX);
                    self.observe_network_feedback(None, Some(wire_bps), now_ms);
                }
            }
            _ => {}
        }
    }

    /// Records raw loss (`fraction_lost / 256`) and/or wire REMB bitrate estimate (`wire_remb_bps`).
    pub fn observe_network_feedback(
        &mut self,
        fraction_lost_256: Option<u8>,
        wire_remb_bps: Option<u32>,
        _now_ms: u64,
    ) {
        if let Some(loss) = fraction_lost_256 {
            self.pending_loss_256 = Some(self.pending_loss_256.map_or(loss, |old| old.max(loss)));
        }
        if let Some(wire) = wire_remb_bps {
            let headroom_bps = ((u64::from(wire) * u64::from(REMB_HEADROOM_NUMERATOR))
                / u64::from(REMB_HEADROOM_DENOMINATOR)) as u32;
            self.pending_remb_bps = Some(
                self.pending_remb_bps
                    .map_or(headroom_bps, |old| old.min(headroom_bps)),
            );
        }
    }

    /// Advances the 1 Hz congestion/bandwidth adaptation loop at `now_ms`.
    /// Returns `Some(new_target_bitrate_bps)` whenever the target bitrate changes.
    pub fn tick_adaptation(&mut self, now_ms: u64) -> Option<u32> {
        if let Some(last) = self.last_tick_at_ms
            && now_ms.saturating_sub(last) < ADAPTATION_TICK_INTERVAL_MS
        {
            return None;
        }
        self.last_tick_at_ms = Some(now_ms);

        let loss = self.pending_loss_256.take();
        let estimate = self.pending_remb_bps.take();
        if let Some(est) = estimate {
            self.active_remb_limit = Some((est, now_ms));
        }

        let min_bps = self.effective_profile.min_bitrate_bps;
        let max_bps = self.effective_profile.max_bitrate_bps;

        let ceiling = self
            .active_remb_limit
            .filter(|(_, at_ms)| now_ms.saturating_sub(*at_ms) < REMB_LIMIT_TTL_MS)
            .map_or(max_bps, |(lim, _)| lim.min(max_bps))
            .max(min_bps);

        let mut target = self.current_target_bitrate_bps.min(ceiling);
        let mut congested = false;
        let mut recovered = false;

        if loss.is_some_and(|l| l >= LOSS_CONGESTION_THRESHOLD_256) {
            target = (target.saturating_mul(4)) / 5;
            self.last_congestion_at_ms = now_ms;
            congested = true;
        } else if target < self.current_target_bitrate_bps {
            self.last_congestion_at_ms = now_ms;
            congested = true;
        } else if (loss.is_some_and(|l| l <= LOSS_RECOVERY_THRESHOLD_256)
            || (loss.is_none() && estimate.is_some()))
            && now_ms.saturating_sub(self.last_congestion_at_ms) >= CONGESTION_RECOVERY_HOLDOFF_MS
        {
            let step = (target / 20).max(25_000);
            let candidate = target.saturating_add(step).min(ceiling);
            if candidate > target {
                target = candidate;
                recovered = true;
            }
        }

        target = target.clamp(min_bps, max_bps);
        if target == self.current_target_bitrate_bps {
            self.refresh_adaptation_state();
            return None;
        }

        self.current_target_bitrate_bps = target;
        self.telemetry.bandwidth_adaptations += 1;
        if congested {
            self.telemetry.congestion_backoffs += 1;
        }
        if recovered {
            self.telemetry.recovery_steps += 1;
        }
        self.refresh_adaptation_state();
        Some(target)
    }

    /// Computes the active encoder resolution, framerate, and byte budget for `(source_width, source_height)`.
    #[allow(non_snake_case)]
    pub fn evaluate_encoder_Params(
        &mut self,
        source_width: u32,
        source_height: u32,
    ) -> EncoderScaleOutput {
        if self.sink_wants_quality == 0 {
            return EncoderScaleOutput {
                active: false,
                encoded_width: self.effective_profile.width,
                encoded_height: self.effective_profile.height,
                effective_pixel_limit: 0,
                encoded_fps: 0,
                target_bitrate_bps: self.current_target_bitrate_bps,
                frame_byte_budget: 0,
                rtp_timestamp_step_90khz: VIDEO_CLOCK_RATE,
            };
        }

        let pixel_limit = self.compute_effective_pixel_limit();
        let (encoded_width, encoded_height) = scale_dimensions_to_pixel_limit(
            source_width,
            source_height,
            self.effective_profile.width,
            self.effective_profile.height,
            pixel_limit,
        );

        if self.last_encoded_dimensions != Some((encoded_width, encoded_height)) {
            if self.last_encoded_dimensions.is_some() {
                self.telemetry.resolution_scalings += 1;
                self.keyframe_requested = true;
            }
            self.last_encoded_dimensions = Some((encoded_width, encoded_height));
        }

        let encoded_fps = self.compute_effective_fps();
        let frame_byte_budget =
            (self.current_target_bitrate_bps as usize) / (8 * (encoded_fps.max(1) as usize));

        EncoderScaleOutput {
            active: true,
            encoded_width,
            encoded_height,
            effective_pixel_limit: pixel_limit,
            encoded_fps,
            target_bitrate_bps: self.current_target_bitrate_bps,
            frame_byte_budget,
            rtp_timestamp_step_90khz: video_ticks_for_fps(encoded_fps),
        }
    }

    fn compute_effective_pixel_limit(&self) -> u32 {
        let mut pixel_limit = self.effective_profile.max_pixel_count;
        if let Some(sink_px) = self.sink_wants_pixel_limit {
            pixel_limit = pixel_limit.min(sink_px);
        }
        if self.sink_wants_quality < 100 {
            let scaled = (u64::from(pixel_limit) * u64::from(self.sink_wants_quality)) / 100;
            pixel_limit = (scaled as u32).max(PIXEL_LIMIT_480P / 2);
        }
        if self.current_target_bitrate_bps * 2 < self.effective_profile.target_bitrate_bps {
            pixel_limit = pixel_limit.min(PIXEL_LIMIT_480P);
        } else if self.current_target_bitrate_bps * 4
            < self.effective_profile.target_bitrate_bps * 3
        {
            pixel_limit = pixel_limit.min(PIXEL_LIMIT_720P);
        }
        pixel_limit
    }

    fn compute_effective_fps(&self) -> u32 {
        let mut encoded_fps = self.effective_profile.fps;
        if let Some(sink_fps) = self.remote_sink_wants_max_fps {
            encoded_fps = encoded_fps.min(sink_fps.max(MIN_STREAM_FPS));
        }
        if self.static_screen_idle_ms >= CONNECTION_IDLE_FRAMERATE_REDUCE_MS {
            encoded_fps = encoded_fps.min(IDLE_REDUCED_MAX_FRAMERATE);
        }
        if self.current_target_bitrate_bps <= self.effective_profile.min_bitrate_bps
            && self.effective_profile.content_hint == StreamContentHint::Detail
        {
            encoded_fps = encoded_fps.min(15);
        }
        encoded_fps
    }

    fn refresh_adaptation_state(&mut self) {
        if self.sink_wants_quality == 0 {
            self.state = StreamAdaptationState::PausedBySink;
        } else if self.sink_wants_quality < 100
            || self.current_target_bitrate_bps * 4 < self.effective_profile.target_bitrate_bps * 3
        {
            self.state = StreamAdaptationState::Degraded;
        } else {
            self.state = StreamAdaptationState::Active;
        }
    }

    fn build_op12_video(&mut self, width: u32, height: u32, fps: u32) -> VoiceVideo {
        self.telemetry.op12_frames_emitted += 1;
        VoiceVideo::screen_share_with_ssrcs(
            self.audio_ssrc,
            self.video_ssrc,
            self.rtx_ssrc,
            self.effective_profile.max_bitrate_bps,
            fps,
            width,
            height,
        )
    }
}
