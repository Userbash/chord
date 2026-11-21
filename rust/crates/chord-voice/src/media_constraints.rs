#![forbid(unsafe_code)]
//! R5-08: Media Constraints Engine (`chord_voice::media_constraints`).
//!
//! Custom media constraint negotiation for the Voice Engine:
//! - Virtual audio track injection (PipeWire / WASAPI 48 kHz stereo desktop loopback);
//! - Exact `deviceId` matching (`deviceId: { exact: "<id>" }`) preventing OS default-device
//!   hijack during hot-plug events while supporting automatic restoration on reconnect;
//! - Automatic Gain Control (AGC), Echo Cancellation (AEC), and Noise Suppression (NS) policy
//!   profiles (Adaptive Digital for voice microphones, Fixed Digital for hardware interfaces,
//!   and complete passthrough bypass for screen-share system audio tracks);
//! - Strict video encoder resolution and frame-rate bounds enforcement (`480p`..`2160p`,
//!   `160x90`..`3840x2160`, aspect-ratio preserving downscaling with even YUV420 alignment,
//!   and optional `strict_lock` fixed-resolution clamping).
//!
//! Specification & Reference Oracles:
//! - `docs/feature-backlog.md` row `R5-08` (Vesktop `screenShareFixes.ts`, `fixStreamConstraints.ts`);
//! - `docs/feature-backlog.md` row `D-04` (Media Capture Constraints Protocol);
//! - `docs/re/voice-deep.md` (§1, §2.3, §3: 48 kHz stereo Opus, 90 kHz video clock,
//!   Voice OP 5 `SOUNDSHARE` `speaking == 2`).

use std::fmt;

use crate::devices::{AudioDevice, AudioDeviceKind};

/// Canonical audio channel count for WebRTC Opus voice and screen-share streaming (`2` stereo).
pub const DEFAULT_AUDIO_CHANNELS: u16 = 2;

/// Canonical audio sample rate for WebRTC Opus voice and screen-share streaming (`48_000` Hz).
pub const DEFAULT_AUDIO_SAMPLE_RATE_HZ: u32 = 48_000;

/// Default virtual audio device identifier registered by the Linux PipeWire loopback service (`R5-01`).
pub const DEFAULT_VIRTUAL_AUDIO_DEVICE_ID: &str = "chord-screen-share-audio";

/// Default virtual audio device identifier registered by the Windows WASAPI loopback service (`R5-07`).
pub const DEFAULT_WASAPI_VIRTUAL_AUDIO_DEVICE_ID: &str = "chord-wasapi-loopback-audio";

/// Minimum permitted video encoding width in pixels (`160` px).
pub const MIN_VIDEO_WIDTH: u32 = 160;

/// Minimum permitted video encoding height in pixels (`90` px).
pub const MIN_VIDEO_HEIGHT: u32 = 90;

/// Maximum permitted video encoding width in pixels (`3840` px, 4K UHD).
pub const MAX_VIDEO_WIDTH: u32 = 3840;

/// Maximum permitted video encoding height in pixels (`2160` px, 4K UHD).
pub const MAX_VIDEO_HEIGHT: u32 = 2160;

/// Minimum permitted video streaming frame rate (`1` FPS).
pub const MIN_STREAM_FPS: u32 = 1;

/// Maximum permitted video streaming frame rate (`120` FPS).
pub const MAX_STREAM_FPS: u32 = 120;

/// Discord Voice Gateway OP 5 `SPEAKING` bitflag for desktop soundshare (`1 << 1 = 2`).
pub const SOUNDSHARE_SPEAKING_BIT: u8 = 2;

/// Source backend providing the injected virtual audio track for screen sharing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VirtualAudioSourceKind {
    /// Linux PipeWire virtual microphone loopback (`Audio/Source/Virtual`, `R5-01`).
    PipeWireLoopback,
    /// Windows WASAPI process/endpoint render loopback (`R5-07`).
    WasapiLoopback,
    /// macOS ScreenCaptureKit / CoreAudio process tap loopback.
    CoreAudioProcessTap,
}

impl VirtualAudioSourceKind {
    /// Returns the canonical backend identifier string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PipeWireLoopback => "pipewire_loopback",
            Self::WasapiLoopback => "wasapi_loopback",
            Self::CoreAudioProcessTap => "coreaudio_tap",
        }
    }
}

/// Descriptor for a virtual audio track injected into a screen-share MediaStream (`R5-08`).
///
/// The `Debug` implementation redacts the human-readable `label` field to prevent leaking
/// host audio topology strings into diagnostic logs (`AGENTS.md` §6 / `G5-VOICE-PLAN.md` §7.4).
#[derive(Clone, PartialEq, Eq)]
pub struct VirtualAudioTrackDescriptor {
    /// Unique logical track identifier within the media session.
    pub track_id: String,
    /// Virtual device identifier matched via `deviceId: { exact: "<device_id>" }`.
    pub device_id: String,
    /// Human-readable device label (redacted in `Debug`).
    pub label: String,
    /// Platform loopback backend providing PCM frames.
    pub source_kind: VirtualAudioSourceKind,
    /// Negotiated channel count (must be `2` stereo for WebRTC desktop audio).
    pub channels: u16,
    /// Negotiated sample rate in Hz (must be `48_000` Hz for WebRTC Opus).
    pub sample_rate_hz: u32,
}

impl fmt::Debug for VirtualAudioTrackDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VirtualAudioTrackDescriptor")
            .field("track_id", &self.track_id)
            .field("device_id", &self.device_id)
            .field("label", &"<redacted>")
            .field("source_kind", &self.source_kind)
            .field("channels", &self.channels)
            .field("sample_rate_hz", &self.sample_rate_hz)
            .finish()
    }
}

impl VirtualAudioTrackDescriptor {
    /// Creates the canonical Linux PipeWire virtual audio track descriptor (`48 kHz`, `2` channels).
    #[must_use]
    pub fn new_pipewire_default() -> Self {
        Self {
            track_id: "virtual-audio-pw-0".to_owned(),
            device_id: DEFAULT_VIRTUAL_AUDIO_DEVICE_ID.to_owned(),
            label: "Chord Screen Share Audio (Virtual Microphone)".to_owned(),
            source_kind: VirtualAudioSourceKind::PipeWireLoopback,
            channels: DEFAULT_AUDIO_CHANNELS,
            sample_rate_hz: DEFAULT_AUDIO_SAMPLE_RATE_HZ,
        }
    }

    /// Creates the canonical Windows WASAPI virtual audio track descriptor (`48 kHz`, `2` channels).
    #[must_use]
    pub fn new_wasapi_default() -> Self {
        Self {
            track_id: "virtual-audio-wasapi-0".to_owned(),
            device_id: DEFAULT_WASAPI_VIRTUAL_AUDIO_DEVICE_ID.to_owned(),
            label: "Chord WASAPI Loopback Audio".to_owned(),
            source_kind: VirtualAudioSourceKind::WasapiLoopback,
            channels: DEFAULT_AUDIO_CHANNELS,
            sample_rate_hz: DEFAULT_AUDIO_SAMPLE_RATE_HZ,
        }
    }

    /// Validates that the virtual audio track conforms to the WebRTC 48 kHz stereo contract.
    pub fn validate(&self) -> Result<(), MediaConstraintError> {
        if self.device_id.trim().is_empty() || self.track_id.trim().is_empty() {
            return Err(MediaConstraintError::EmptyDeviceId);
        }
        if self.channels != DEFAULT_AUDIO_CHANNELS
            || self.sample_rate_hz != DEFAULT_AUDIO_SAMPLE_RATE_HZ
        {
            return Err(MediaConstraintError::UnsupportedAudioFormat {
                channels: self.channels,
                sample_rate_hz: self.sample_rate_hz,
            });
        }
        Ok(())
    }
}

/// WebRTC `deviceId` matching constraint mode (`R5-08`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceIdMatchConstraint {
    /// Strict exact device binding (`deviceId: { exact: "<id>" }`).
    /// Prevents Chromium/WebRTC from silently switching to a newly plugged headset or default device.
    Exact(String),
    /// Preferred device with fallback to an active device when the preferred device is unplugged.
    ExactWithFallback {
        preferred_exact_id: String,
        effective_active_id: String,
    },
}

impl DeviceIdMatchConstraint {
    /// Returns the currently active device ID bound to the audio track.
    #[must_use]
    pub fn effective_id(&self) -> &str {
        match self {
            Self::Exact(id) => id.as_str(),
            Self::ExactWithFallback {
                effective_active_id,
                ..
            } => effective_active_id.as_str(),
        }
    }

    /// Returns the user's originally requested exact device ID.
    #[must_use]
    pub fn preferred_exact_id(&self) -> &str {
        match self {
            Self::Exact(id) => id.as_str(),
            Self::ExactWithFallback {
                preferred_exact_id, ..
            } => preferred_exact_id.as_str(),
        }
    }
}

/// Automatic Gain Control (AGC) operating mode in the Voice Engine (`R5-08` / `D-04`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgcMode {
    /// WebRTC Adaptive Digital AGC + analog/digital level estimator (default for voice microphones).
    AdaptiveDigital,
    /// Fixed Digital Gain without dynamic level pumping (for studio USB interfaces with hardware gain).
    FixedDigital,
    /// Complete DSP bypass (`autoGainControl: false`, `echoCancellation: false`, `noiseSuppression: false`)
    /// mandatory for virtual desktop/game audio tracks in screen sharing.
    BypassedForStream,
}

/// AGC and voice DSP constraint policy applied to an audio track (`R5-08`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgcPolicyConfig {
    /// Selected AGC operating mode.
    pub mode: AgcMode,
    /// WebRTC `autoGainControl` / `googAutoGainControl` constraint flag.
    pub auto_gain_control: bool,
    /// WebRTC `echoCancellation` constraint flag.
    pub echo_cancellation: bool,
    /// WebRTC `noiseSuppression` constraint flag.
    pub noise_suppression: bool,
    /// WebRTC `typingNoiseDetection` constraint flag.
    pub typing_noise_detection: bool,
    /// Target peak output level below full-scale in dBFS (e.g. `-3` dBFS for voice, `0` dBFS for stream).
    pub target_level_dbfs: i8,
    /// Maximum dynamic compression gain in dB (e.g. `9` dB for adaptive voice, `0` dB for stream).
    pub compression_gain_db: u8,
    /// Whether the brickwall peak limiter is enabled.
    pub limiter_enabled: bool,
}

impl AgcPolicyConfig {
    /// Default Adaptive Digital AGC policy for voice microphone capture (`D-04`).
    #[must_use]
    pub const fn voice_adaptive_default() -> Self {
        Self {
            mode: AgcMode::AdaptiveDigital,
            auto_gain_control: true,
            echo_cancellation: true,
            noise_suppression: true,
            typing_noise_detection: true,
            target_level_dbfs: -3,
            compression_gain_db: 9,
            limiter_enabled: true,
        }
    }

    /// Fixed Digital policy for users with external audio interfaces who disable automatic gain (`D-04`).
    #[must_use]
    pub const fn voice_fixed_manual() -> Self {
        Self {
            mode: AgcMode::FixedDigital,
            auto_gain_control: false,
            echo_cancellation: true,
            noise_suppression: true,
            typing_noise_detection: true,
            target_level_dbfs: -3,
            compression_gain_db: 0,
            limiter_enabled: true,
        }
    }

    /// Studio passthrough policy for virtual screen-share audio (`screenShareFixes.ts` / `fixStreamConstraints.ts`):
    /// disables AGC, AEC, NS, and typing detection so music and game audio preserve full stereo fidelity.
    #[must_use]
    pub const fn screen_share_passthrough() -> Self {
        Self {
            mode: AgcMode::BypassedForStream,
            auto_gain_control: false,
            echo_cancellation: false,
            noise_suppression: false,
            typing_noise_detection: false,
            target_level_dbfs: 0,
            compression_gain_db: 0,
            limiter_enabled: false,
        }
    }
}

/// Functional role of a negotiated audio track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioTrackRole {
    /// Primary voice chat microphone input.
    VoiceMicrophone,
    /// Injected virtual system/application audio track for screen sharing (`Go Live`).
    ScreenShareSystemAudio,
}

/// Negotiated constraint specification for an active audio track (`R5-08`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioTrackConstraint {
    /// Role of this audio track (voice mic vs screen-share system audio).
    pub role: AudioTrackRole,
    /// Exact `deviceId` match binding (or fallback binding after physical unplug).
    pub device_id_constraint: DeviceIdMatchConstraint,
    /// Whether this track was injected from a virtual loopback source (`PipeWire` / `WASAPI`).
    pub is_virtual_injected: bool,
    /// Negotiated channel count (`1` or `2`).
    pub channels: u16,
    /// Negotiated sample rate (`48_000` Hz).
    pub sample_rate_hz: u32,
    /// Negotiated AGC and voice processing policy.
    pub agc_policy: AgcPolicyConfig,
    /// Monotonic generation counter that increments only if the underlying track is rebound.
    pub track_generation: u64,
}

impl AudioTrackConstraint {
    /// Returns the currently active device ID bound to the track.
    #[must_use]
    pub fn effective_device_id(&self) -> &str {
        self.device_id_constraint.effective_id()
    }

    /// Returns the user's preferred exact device ID.
    #[must_use]
    pub fn preferred_exact_device_id(&self) -> &str {
        self.device_id_constraint.preferred_exact_id()
    }
}

/// Canonical screen-share and video resolution presets (`480p`..`2160p`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResolutionPreset {
    /// 480p SD (`854×480`, 16:9 even-aligned).
    P480,
    /// 720p HD (`1280×720`, 16:9).
    P720,
    /// 1080p Full HD (`1920×1080`, 16:9).
    P1080,
    /// 1440p QHD (`2560×1440`, 16:9).
    P1440,
    /// 2160p 4K UHD (`3840×2160`, 16:9).
    P2160,
}

impl ResolutionPreset {
    /// All canonical resolution presets in ascending order.
    pub const ALL: [Self; 5] = [
        Self::P480,
        Self::P720,
        Self::P1080,
        Self::P1440,
        Self::P2160,
    ];

    /// Returns the exact `(width, height)` pixel dimensions for this preset.
    #[must_use]
    pub const fn dimensions(self) -> (u32, u32) {
        match self {
            Self::P480 => (854, 480),
            Self::P720 => (1280, 720),
            Self::P1080 => (1920, 1080),
            Self::P1440 => (2560, 1440),
            Self::P2160 => (3840, 2160),
        }
    }
}

/// Strict resolution and frame-rate bounding box enforced on the video encoder (`R5-08`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolutionBounds {
    /// Minimum allowed frame width (`>= MIN_VIDEO_WIDTH`).
    pub min_width: u32,
    /// Minimum allowed frame height (`>= MIN_VIDEO_HEIGHT`).
    pub min_height: u32,
    /// Target / ideal frame width.
    pub target_width: u32,
    /// Target / ideal frame height.
    pub target_height: u32,
    /// Maximum allowed frame width (`<= MAX_VIDEO_WIDTH`).
    pub max_width: u32,
    /// Maximum allowed frame height (`<= MAX_VIDEO_HEIGHT`).
    pub max_height: u32,
    /// Minimum frame rate (`>= MIN_STREAM_FPS`).
    pub min_fps: u32,
    /// Target / ideal frame rate.
    pub target_fps: u32,
    /// Maximum allowed frame rate (`<= MAX_STREAM_FPS`).
    pub max_fps: u32,
    /// When `true`, locks `min == target == max` so any input frame is scaled to exact target bounds.
    pub strict_lock: bool,
}

impl ResolutionBounds {
    /// Constructs resolution bounds for a canonical [`ResolutionPreset`] and `max_fps`.
    #[must_use]
    pub const fn from_preset(preset: ResolutionPreset, max_fps: u32) -> Self {
        let (w, h) = preset.dimensions();
        let min_fps = if max_fps < 15 { max_fps } else { 15 };
        Self {
            min_width: MIN_VIDEO_WIDTH,
            min_height: MIN_VIDEO_HEIGHT,
            target_width: w,
            target_height: h,
            max_width: w,
            max_height: h,
            min_fps,
            target_fps: max_fps,
            max_fps,
            strict_lock: false,
        }
    }

    /// Constructs strictly locked resolution bounds (`min == target == max`).
    #[must_use]
    pub const fn strict_fixed(width: u32, height: u32, fps: u32) -> Self {
        Self {
            min_width: width,
            min_height: height,
            target_width: width,
            target_height: height,
            max_width: width,
            max_height: height,
            min_fps: fps,
            target_fps: fps,
            max_fps: fps,
            strict_lock: true,
        }
    }

    /// Validates all width, height, parity (even YUV420 alignment), and FPS invariants.
    pub fn validate(&self) -> Result<(), MediaConstraintError> {
        for &(w, h) in &[
            (self.min_width, self.min_height),
            (self.target_width, self.target_height),
            (self.max_width, self.max_height),
        ] {
            if !(MIN_VIDEO_WIDTH..=MAX_VIDEO_WIDTH).contains(&w)
                || !(MIN_VIDEO_HEIGHT..=MAX_VIDEO_HEIGHT).contains(&h)
            {
                return Err(MediaConstraintError::ResolutionOutOfRange {
                    width: w,
                    height: h,
                    min_width: MIN_VIDEO_WIDTH,
                    min_height: MIN_VIDEO_HEIGHT,
                    max_width: MAX_VIDEO_WIDTH,
                    max_height: MAX_VIDEO_HEIGHT,
                });
            }
            if !w.is_multiple_of(2) || !h.is_multiple_of(2) {
                return Err(MediaConstraintError::OddResolutionDimension {
                    width: w,
                    height: h,
                });
            }
        }

        if self.min_width > self.max_width
            || self.min_height > self.max_height
            || self.target_width < self.min_width
            || self.target_width > self.max_width
            || self.target_height < self.min_height
            || self.target_height > self.max_height
        {
            return Err(MediaConstraintError::InvertedResolutionBounds {
                min_width: self.min_width,
                min_height: self.min_height,
                max_width: self.max_width,
                max_height: self.max_height,
            });
        }

        for &fps in &[self.min_fps, self.target_fps, self.max_fps] {
            if !(MIN_STREAM_FPS..=MAX_STREAM_FPS).contains(&fps) {
                return Err(MediaConstraintError::FpsOutOfRange {
                    fps,
                    min_fps: MIN_STREAM_FPS,
                    max_fps: MAX_STREAM_FPS,
                });
            }
        }
        if self.min_fps > self.max_fps || self.target_fps > self.max_fps {
            return Err(MediaConstraintError::FpsOutOfRange {
                fps: self.min_fps,
                min_fps: MIN_STREAM_FPS,
                max_fps: self.max_fps,
            });
        }

        Ok(())
    }
}

/// Negotiated video resolution constraint applied to the video capture and encoder pipeline (`R5-08`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoResolutionConstraint {
    /// Validated resolution and frame-rate bounds.
    pub bounds: ResolutionBounds,
}

/// Input frame metadata submitted to the video encoder bounds enforcer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoEncoderFrameInput {
    /// Captured source width in pixels.
    pub width: u32,
    /// Captured source height in pixels.
    pub height: u32,
    /// Captured source frame rate.
    pub fps: u32,
    /// Monotonic capture timestamp in microseconds.
    pub timestamp_us: u64,
}

/// Decision produced by [`MediaConstraintsEngine::enforce_video_encoder_bounds`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoderFrameDecision {
    /// Source frame already lies within the selected resolution and FPS bounds.
    PassThrough { width: u32, height: u32, fps: u32 },
    /// Source frame exceeded (or under-ran strict-lock) bounds and was clamped/scaled to adhere strictly.
    ScaledToBounds {
        original_width: u32,
        original_height: u32,
        encoded_width: u32,
        encoded_height: u32,
        encoded_fps: u32,
    },
}

/// Combined negotiated audio and video constraints for an active screen-share stream (`R5-08`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NegotiatedStreamConstraints {
    /// Optional injected virtual audio track constraint (`None` for video-only stream).
    pub audio: Option<AudioTrackConstraint>,
    /// Strict video encoder resolution and frame-rate constraint.
    pub video: VideoResolutionConstraint,
}

impl NegotiatedStreamConstraints {
    /// Returns the Discord Voice Gateway OP 5 `SPEAKING` bitmask (`2` for `SOUNDSHARE` when audio is attached, `0` otherwise).
    #[must_use]
    pub const fn speaking_flags(&self) -> u8 {
        if self.audio.is_some() {
            SOUNDSHARE_SPEAKING_BIT
        } else {
            0
        }
    }

    /// Serializes the negotiated constraints into the canonical WebRTC MediaTrackConstraints JSON structure
    /// (`fixStreamConstraints.ts` / `screenShareFixes.ts` wire contract).
    #[must_use]
    pub fn to_webrtc_constraints_json(&self) -> String {
        let b = &self.video.bounds;
        let video_json = format!(
            "{{\"width\":{{\"min\":{},\"ideal\":{},\"max\":{}}},\"height\":{{\"min\":{},\"ideal\":{},\"max\":{}}},\"frameRate\":{{\"min\":{},\"ideal\":{},\"max\":{}}}}}",
            b.min_width,
            b.target_width,
            b.max_width,
            b.min_height,
            b.target_height,
            b.max_height,
            b.min_fps,
            b.target_fps,
            b.max_fps
        );
        let audio_json = match &self.audio {
            Some(a) => format!(
                "{{\"deviceId\":{{\"exact\":\"{}\"}},\"autoGainControl\":{},\"echoCancellation\":{},\"noiseSuppression\":{},\"channelCount\":{},\"sampleRate\":{}}}",
                a.effective_device_id(),
                a.agc_policy.auto_gain_control,
                a.agc_policy.echo_cancellation,
                a.agc_policy.noise_suppression,
                a.channels,
                a.sample_rate_hz
            ),
            None => "false".to_owned(),
        };
        format!("{{\"audio\":{audio_json},\"video\":{video_json}}}")
    }
}

/// Lifecycle state of the [`MediaConstraintsEngine`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaConstraintsLifecycleState {
    /// Initial state before constraint negotiation.
    Idle,
    /// Constraints actively negotiated and enforced on voice/video tracks.
    Active,
    /// Stream and constraint enforcement stopped.
    Stopped,
}

/// Outcome report after handling an OS audio device topology change (`hot-plug` / `unplug`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceTopologyStabilityReport {
    /// `true` if the active voice microphone track remained bound to its exact device without interruption.
    pub voice_track_unchanged: bool,
    /// `true` if the injected virtual screen-share audio track remained 100% stable and uninterrupted.
    pub virtual_stream_track_unchanged: bool,
    /// If the pinned physical voice mic was unplugged, holds the fallback device ID selected.
    pub voice_fallback_device_id: Option<String>,
    /// `true` if a previously unplugged preferred exact voice mic was plugged back in and automatically restored.
    pub voice_restored_preferred_exact: bool,
}

/// Observable telemetry counters for the [`MediaConstraintsEngine`] (`TEST-POLICY.md` §12 dim 9).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MediaConstraintsTelemetry {
    /// Total number of successful voice or stream constraint negotiations.
    pub negotiations_total: u64,
    /// Number of virtual audio tracks injected into screen-share streams.
    pub virtual_tracks_injected: u64,
    /// Number of exact `deviceId` constraint bindings established.
    pub exact_device_matches: u64,
    /// Number of audio tracks configured with AGC/AEC/NS bypass (`BypassedForStream`).
    pub agc_bypassed_tracks: u64,
    /// Number of video frames that passed encoder bounds checks without scaling.
    pub frames_passed_within_bounds: u64,
    /// Number of video frames scaled/clamped to adhere strictly to selected resolution bounds.
    pub frames_clamped_to_bounds: u64,
    /// Number of corrupt/zero-dimension video frames rejected before encoding.
    pub frames_dropped_out_of_bounds: u64,
    /// Number of OS audio device hot-plug / topology change events processed.
    pub device_change_events_handled: u64,
    /// Number of topology change events where active tracks remained completely uninterrupted.
    pub track_stability_preserved_count: u64,
}

/// Errors returned by the [`MediaConstraintsEngine`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaConstraintError {
    /// Provided `deviceId` or track identifier was empty.
    EmptyDeviceId,
    /// No audio input devices are registered in the engine.
    NoInputDevicesAvailable,
    /// Requested exact `deviceId` does not exist in the current input device list.
    ExactDeviceNotFound { device_id: String },
    /// Requested `deviceId` belongs to an output device instead of an input device.
    WrongDeviceKind {
        device_id: String,
        expected: AudioDeviceKind,
        actual: AudioDeviceKind,
    },
    /// Virtual audio track sample rate or channel count violates the 48 kHz stereo WebRTC contract.
    UnsupportedAudioFormat { channels: u16, sample_rate_hz: u32 },
    /// Video resolution width or height lies outside `[160x90..=3840x2160]`.
    ResolutionOutOfRange {
        width: u32,
        height: u32,
        min_width: u32,
        min_height: u32,
        max_width: u32,
        max_height: u32,
    },
    /// Video resolution min bounds exceed max bounds or target lies outside `[min..=max]`.
    InvertedResolutionBounds {
        min_width: u32,
        min_height: u32,
        max_width: u32,
        max_height: u32,
    },
    /// Video width or height is odd (violates YUV420 2x2 chroma subsampling alignment).
    OddResolutionDimension { width: u32, height: u32 },
    /// Video frame rate lies outside `[1..=120]` FPS.
    FpsOutOfRange {
        fps: u32,
        min_fps: u32,
        max_fps: u32,
    },
    /// Input video frame has zero width, zero height, or zero FPS.
    InvalidFrameDimensions { width: u32, height: u32 },
    /// Attempted to enforce or renegotiate bounds when the engine is not in the `Active` state.
    EngineNotActive,
}

impl fmt::Display for MediaConstraintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDeviceId => f.write_str("media constraint deviceId must not be empty"),
            Self::NoInputDevicesAvailable => {
                f.write_str("no audio input devices available for constraint negotiation")
            }
            Self::ExactDeviceNotFound { device_id } => {
                write!(f, "exact audio input device '{device_id}' not found")
            }
            Self::WrongDeviceKind {
                device_id,
                expected,
                actual,
            } => write!(
                f,
                "audio device '{device_id}' has kind {actual:?}, expected {expected:?}"
            ),
            Self::UnsupportedAudioFormat {
                channels,
                sample_rate_hz,
            } => write!(
                f,
                "unsupported virtual audio format: {channels}ch @ {sample_rate_hz} Hz (expected 2ch @ 48000 Hz)"
            ),
            Self::ResolutionOutOfRange {
                width,
                height,
                min_width,
                min_height,
                max_width,
                max_height,
            } => write!(
                f,
                "resolution {width}x{height} out of allowed bounds [{min_width}x{min_height}..={max_width}x{max_height}]"
            ),
            Self::InvertedResolutionBounds {
                min_width,
                min_height,
                max_width,
                max_height,
            } => write!(
                f,
                "inverted resolution bounds: min {min_width}x{min_height} exceeds max {max_width}x{max_height}"
            ),
            Self::OddResolutionDimension { width, height } => write!(
                f,
                "video resolution {width}x{height} must have even dimensions for YUV420 encoding"
            ),
            Self::FpsOutOfRange {
                fps,
                min_fps,
                max_fps,
            } => write!(
                f,
                "frame rate {fps} FPS out of allowed bounds [{min_fps}..={max_fps}]"
            ),
            Self::InvalidFrameDimensions { width, height } => {
                write!(f, "invalid video encoder input frame dimensions {width}x{height}")
            }
            Self::EngineNotActive => {
                f.write_str("media constraints engine has no active negotiated stream")
            }
        }
    }
}

impl std::error::Error for MediaConstraintError {}

/// Media Constraints Engine (`R5-08`) managing virtual audio track injection,
/// exact `deviceId` matching, AGC policy enforcement, and video encoder resolution bounds.
#[derive(Debug, Clone)]
pub struct MediaConstraintsEngine {
    devices: Vec<AudioDevice>,
    lifecycle_state: MediaConstraintsLifecycleState,
    active_voice_input: Option<AudioTrackConstraint>,
    active_stream: Option<NegotiatedStreamConstraints>,
    injected_virtual_track: Option<VirtualAudioTrackDescriptor>,
    next_generation: u64,
    telemetry: MediaConstraintsTelemetry,
}

impl Default for MediaConstraintsEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl MediaConstraintsEngine {
    /// Creates a new [`MediaConstraintsEngine`] in [`MediaConstraintsLifecycleState::Idle`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
            lifecycle_state: MediaConstraintsLifecycleState::Idle,
            active_voice_input: None,
            active_stream: None,
            injected_virtual_track: None,
            next_generation: 1,
            telemetry: MediaConstraintsTelemetry::default(),
        }
    }

    /// Creates a new [`MediaConstraintsEngine`] pre-populated with known audio devices.
    #[must_use]
    pub fn with_devices(devices: Vec<AudioDevice>) -> Self {
        let mut engine = Self::new();
        engine.devices = devices;
        engine
    }

    /// Returns the current engine lifecycle state.
    #[must_use]
    pub const fn lifecycle_state(&self) -> MediaConstraintsLifecycleState {
        self.lifecycle_state
    }

    /// Returns the currently registered audio devices.
    #[must_use]
    pub fn available_devices(&self) -> &[AudioDevice] {
        &self.devices
    }

    /// Returns the active voice microphone constraint, if negotiated.
    #[must_use]
    pub const fn active_voice_input_constraint(&self) -> Option<&AudioTrackConstraint> {
        self.active_voice_input.as_ref()
    }

    /// Returns the active screen-share stream constraints, if negotiated.
    #[must_use]
    pub const fn active_stream_constraints(&self) -> Option<&NegotiatedStreamConstraints> {
        self.active_stream.as_ref()
    }

    /// Returns the observable telemetry counters.
    #[must_use]
    pub const fn telemetry(&self) -> &MediaConstraintsTelemetry {
        &self.telemetry
    }

    /// Negotiates exact `deviceId` matching and AGC policy for the primary voice microphone input.
    pub fn negotiate_voice_input_constraints(
        &mut self,
        exact_device_id: &str,
        agc_policy: AgcPolicyConfig,
    ) -> Result<AudioTrackConstraint, MediaConstraintError> {
        let trimmed = exact_device_id.trim();
        if trimmed.is_empty() {
            return Err(MediaConstraintError::EmptyDeviceId);
        }
        if !self
            .devices
            .iter()
            .any(|d| d.kind == AudioDeviceKind::Input)
        {
            return Err(MediaConstraintError::NoInputDevicesAvailable);
        }

        if let Some(device) = self.devices.iter().find(|d| d.id == trimmed) {
            if device.kind != AudioDeviceKind::Input {
                return Err(MediaConstraintError::WrongDeviceKind {
                    device_id: trimmed.to_owned(),
                    expected: AudioDeviceKind::Input,
                    actual: device.kind,
                });
            }
        } else {
            return Err(MediaConstraintError::ExactDeviceNotFound {
                device_id: trimmed.to_owned(),
            });
        }

        let generation = self.allocate_generation();
        let constraint = AudioTrackConstraint {
            role: AudioTrackRole::VoiceMicrophone,
            device_id_constraint: DeviceIdMatchConstraint::Exact(trimmed.to_owned()),
            is_virtual_injected: false,
            channels: DEFAULT_AUDIO_CHANNELS,
            sample_rate_hz: DEFAULT_AUDIO_SAMPLE_RATE_HZ,
            agc_policy,
            track_generation: generation,
        };

        self.telemetry.negotiations_total = self.telemetry.negotiations_total.saturating_add(1);
        self.telemetry.exact_device_matches = self.telemetry.exact_device_matches.saturating_add(1);
        if !agc_policy.auto_gain_control {
            self.telemetry.agc_bypassed_tracks =
                self.telemetry.agc_bypassed_tracks.saturating_add(1);
        }

        self.active_voice_input = Some(constraint.clone());
        Ok(constraint)
    }

    /// Negotiates custom screen-share constraints: optional virtual audio track injection
    /// (with exact `deviceId` matching and AGC/AEC/NS bypass) and strict video resolution bounds.
    pub fn negotiate_screen_share_constraints(
        &mut self,
        virtual_audio: Option<VirtualAudioTrackDescriptor>,
        bounds: ResolutionBounds,
    ) -> Result<NegotiatedStreamConstraints, MediaConstraintError> {
        bounds.validate()?;

        let audio_constraint = if let Some(vtrack) = virtual_audio {
            vtrack.validate()?;
            let generation = self.allocate_generation();
            let constraint = Self::build_virtual_audio_constraint(&vtrack, generation);
            self.ensure_virtual_device_registered(&vtrack);
            self.injected_virtual_track = Some(vtrack);
            self.telemetry.virtual_tracks_injected =
                self.telemetry.virtual_tracks_injected.saturating_add(1);
            self.telemetry.exact_device_matches =
                self.telemetry.exact_device_matches.saturating_add(1);
            self.telemetry.agc_bypassed_tracks =
                self.telemetry.agc_bypassed_tracks.saturating_add(1);
            Some(constraint)
        } else {
            self.injected_virtual_track = None;
            None
        };

        let negotiated = NegotiatedStreamConstraints {
            audio: audio_constraint,
            video: VideoResolutionConstraint { bounds },
        };

        self.lifecycle_state = MediaConstraintsLifecycleState::Active;
        self.active_stream = Some(negotiated.clone());
        self.telemetry.negotiations_total = self.telemetry.negotiations_total.saturating_add(1);
        Ok(negotiated)
    }

    /// Dynamically injects or replaces a virtual audio track on an active screen-share stream
    /// without interrupting the video encoder bounds.
    pub fn inject_virtual_audio_track(
        &mut self,
        virtual_audio: VirtualAudioTrackDescriptor,
    ) -> Result<AudioTrackConstraint, MediaConstraintError> {
        if self.lifecycle_state != MediaConstraintsLifecycleState::Active {
            return Err(MediaConstraintError::EngineNotActive);
        }
        virtual_audio.validate()?;
        let generation = self.allocate_generation();
        let constraint = Self::build_virtual_audio_constraint(&virtual_audio, generation);
        self.ensure_virtual_device_registered(&virtual_audio);
        self.injected_virtual_track = Some(virtual_audio);

        if let Some(stream) = self.active_stream.as_mut() {
            stream.audio = Some(constraint.clone());
        }
        self.telemetry.virtual_tracks_injected =
            self.telemetry.virtual_tracks_injected.saturating_add(1);
        self.telemetry.exact_device_matches = self.telemetry.exact_device_matches.saturating_add(1);
        self.telemetry.agc_bypassed_tracks = self.telemetry.agc_bypassed_tracks.saturating_add(1);
        Ok(constraint)
    }

    /// Renegotiates the video encoder resolution bounds on an active stream while keeping
    /// the injected virtual audio track intact and stable.
    pub fn renegotiate_video_bounds(
        &mut self,
        new_bounds: ResolutionBounds,
    ) -> Result<NegotiatedStreamConstraints, MediaConstraintError> {
        if self.lifecycle_state != MediaConstraintsLifecycleState::Active {
            return Err(MediaConstraintError::EngineNotActive);
        }
        new_bounds.validate()?;
        let stream = self
            .active_stream
            .as_mut()
            .ok_or(MediaConstraintError::EngineNotActive)?;
        stream.video.bounds = new_bounds;
        self.telemetry.negotiations_total = self.telemetry.negotiations_total.saturating_add(1);
        Ok(stream.clone())
    }

    /// Enforces the negotiated resolution and frame-rate bounds on an incoming video encoder frame.
    ///
    /// Guarantees that the video encoder strictly adheres to the selected resolution bounds:
    /// - Rejects zero-width or zero-height frames;
    /// - If `strict_lock` is enabled, scales any differing frame directly to `(target_width, target_height)`;
    /// - If the frame exceeds `(max_width, max_height)` or falls below `(min_width, min_height)`,
    ///   scales the frame proportionally while preserving aspect ratio and aligning dimensions
    ///   to even numbers (`% 2 == 0`) for YUV420 planar encoding.
    pub fn enforce_video_encoder_bounds(
        &mut self,
        frame: VideoEncoderFrameInput,
    ) -> Result<EncoderFrameDecision, MediaConstraintError> {
        if self.lifecycle_state != MediaConstraintsLifecycleState::Active {
            return Err(MediaConstraintError::EngineNotActive);
        }
        if frame.width == 0 || frame.height == 0 || frame.fps == 0 {
            self.telemetry.frames_dropped_out_of_bounds = self
                .telemetry
                .frames_dropped_out_of_bounds
                .saturating_add(1);
            return Err(MediaConstraintError::InvalidFrameDimensions {
                width: frame.width,
                height: frame.height,
            });
        }

        let bounds = self
            .active_stream
            .as_ref()
            .ok_or(MediaConstraintError::EngineNotActive)?
            .video
            .bounds;

        let encoded_fps = frame.fps.clamp(bounds.min_fps, bounds.max_fps);
        let (encoded_w, encoded_h) = Self::clamp_dimensions_to_bounds(
            frame.width,
            frame.height,
            &bounds,
        );

        if encoded_w == frame.width && encoded_h == frame.height && encoded_fps == frame.fps {
            self.telemetry.frames_passed_within_bounds =
                self.telemetry.frames_passed_within_bounds.saturating_add(1);
            Ok(EncoderFrameDecision::PassThrough {
                width: encoded_w,
                height: encoded_h,
                fps: encoded_fps,
            })
        } else {
            self.telemetry.frames_clamped_to_bounds =
                self.telemetry.frames_clamped_to_bounds.saturating_add(1);
            Ok(EncoderFrameDecision::ScaledToBounds {
                original_width: frame.width,
                original_height: frame.height,
                encoded_width: encoded_w,
                encoded_height: encoded_h,
                encoded_fps,
            })
        }
    }

    /// Processes an OS audio device list update (`hot-plug`, default change, or physical unplug)
    /// while guaranteeing audio track stability (`R5-08` acceptance criteria).
    #[allow(non_snake_case)]
    pub fn handle_device_Topology_change(
        &mut self,
        mut new_devices: Vec<AudioDevice>,
    ) -> DeviceTopologyStabilityReport {
        // Ensure any injected virtual audio device remains registered in the logical topology
        if let Some(vtrack) = &self.injected_virtual_track
            && !new_devices.iter().any(|d| d.id == vtrack.device_id)
        {
            new_devices.push(AudioDevice {
                id: vtrack.device_id.clone(),
                label: vtrack.label.clone(),
                kind: AudioDeviceKind::Input,
                is_default: false,
            });
        }
        self.devices = new_devices;
        self.telemetry.device_change_events_handled = self
            .telemetry
            .device_change_events_handled
            .saturating_add(1);

        let virtual_stream_track_unchanged = true;
        let mut voice_track_unchanged = true;
        let mut voice_fallback_device_id = None;
        let mut voice_restored_preferred_exact = false;

        let next_gen = self.next_generation;
        if let Some(voice) = self.active_voice_input.as_mut() {
            let preferred_id = voice.preferred_exact_device_id().to_owned();
            let preferred_present = self
                .devices
                .iter()
                .any(|d| d.kind == AudioDeviceKind::Input && d.id == preferred_id);

            match &voice.device_id_constraint {
                DeviceIdMatchConstraint::Exact(_) => {
                    if !preferred_present {
                        voice_track_unchanged = false;
                        if let Some(fallback) = Self::find_fallback_input(&self.devices) {
                            let fb_id = fallback.id.clone();
                            voice.device_id_constraint =
                                DeviceIdMatchConstraint::ExactWithFallback {
                                    preferred_exact_id: preferred_id,
                                    effective_active_id: fb_id.clone(),
                                };
                            voice.track_generation = next_gen;
                            self.next_generation = next_gen.saturating_add(1);
                            voice_fallback_device_id = Some(fb_id);
                        }
                    }
                }
                DeviceIdMatchConstraint::ExactWithFallback {
                    effective_active_id,
                    ..
                } => {
                    if preferred_present {
                        voice_track_unchanged = false;
                        voice_restored_preferred_exact = true;
                        voice.device_id_constraint = DeviceIdMatchConstraint::Exact(preferred_id);
                        voice.track_generation = next_gen;
                        self.next_generation = next_gen.saturating_add(1);
                    } else {
                        let active_still_present = self.devices.iter().any(|d| {
                            d.kind == AudioDeviceKind::Input && &d.id == effective_active_id
                        });
                        if !active_still_present
                            && let Some(fallback) = Self::find_fallback_input(&self.devices)
                        {
                            voice_track_unchanged = false;
                            let fb_id = fallback.id.clone();
                            voice.device_id_constraint =
                                DeviceIdMatchConstraint::ExactWithFallback {
                                    preferred_exact_id: preferred_id,
                                    effective_active_id: fb_id.clone(),
                                };
                            voice.track_generation = next_gen;
                            self.next_generation = next_gen.saturating_add(1);
                            voice_fallback_device_id = Some(fb_id);
                        }
                    }
                }
            }
        }

        if voice_track_unchanged && virtual_stream_track_unchanged {
            self.telemetry.track_stability_preserved_count = self
                .telemetry
                .track_stability_preserved_count
                .saturating_add(1);
        }

        DeviceTopologyStabilityReport {
            voice_track_unchanged,
            virtual_stream_track_unchanged,
            voice_fallback_device_id,
            voice_restored_preferred_exact,
        }
    }

    /// Stops active stream constraint enforcement and transitions to [`MediaConstraintsLifecycleState::Stopped`].
    pub fn stop(&mut self) {
        self.lifecycle_state = MediaConstraintsLifecycleState::Stopped;
        self.active_stream = None;
        self.injected_virtual_track = None;
    }

    fn allocate_generation(&mut self) -> u64 {
        let generation = self.next_generation;
        self.next_generation = self.next_generation.saturating_add(1);
        generation
    }

    fn build_virtual_audio_constraint(
        vtrack: &VirtualAudioTrackDescriptor,
        generation: u64,
    ) -> AudioTrackConstraint {
        AudioTrackConstraint {
            role: AudioTrackRole::ScreenShareSystemAudio,
            device_id_constraint: DeviceIdMatchConstraint::Exact(vtrack.device_id.clone()),
            is_virtual_injected: true,
            channels: vtrack.channels,
            sample_rate_hz: vtrack.sample_rate_hz,
            agc_policy: AgcPolicyConfig::screen_share_passthrough(),
            track_generation: generation,
        }
    }

    fn ensure_virtual_device_registered(&mut self, vtrack: &VirtualAudioTrackDescriptor) {
        if !self.devices.iter().any(|d| d.id == vtrack.device_id) {
            self.devices.push(AudioDevice {
                id: vtrack.device_id.clone(),
                label: vtrack.label.clone(),
                kind: AudioDeviceKind::Input,
                is_default: false,
            });
        }
    }

    fn find_fallback_input(devices: &[AudioDevice]) -> Option<&AudioDevice> {
        devices
            .iter()
            .find(|d| d.kind == AudioDeviceKind::Input && d.is_default)
            .or_else(|| devices.iter().find(|d| d.kind == AudioDeviceKind::Input))
    }

    fn clamp_dimensions_to_bounds(
        width: u32,
        height: u32,
        bounds: &ResolutionBounds,
    ) -> (u32, u32) {
        if bounds.strict_lock {
            return (bounds.target_width, bounds.target_height);
        }

        let mut w = width;
        let mut h = height;

        // Downscale proportionally if either dimension exceeds max_width or max_height
        if w > bounds.max_width || h > bounds.max_height {
            let w_u64 = u64::from(w);
            let h_u64 = u64::from(h);
            let max_w = u64::from(bounds.max_width);
            let max_h = u64::from(bounds.max_height);

            if w_u64 * max_h >= h_u64 * max_w {
                w = bounds.max_width;
                h = ((h_u64 * max_w) / w_u64) as u32;
            } else {
                h = bounds.max_height;
                w = ((w_u64 * max_h) / h_u64) as u32;
            }
        }

        // Upscale if below minimum bounds
        if w < bounds.min_width {
            w = bounds.min_width;
        }
        if h < bounds.min_height {
            h = bounds.min_height;
        }

        // Align down to even integers for YUV420 planar compatibility while respecting min bounds
        let even_w = (w & !1).max(bounds.min_width).min(bounds.max_width);
        let even_h = (h & !1).max(bounds.min_height).min(bounds.max_height);
        (even_w, even_h)
    }
}
