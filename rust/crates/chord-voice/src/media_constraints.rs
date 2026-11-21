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
/// `Debug` redacts the human-readable `label` field (`<redacted>`) per `G5-VOICE-PLAN.md` §7.4.
#[derive(Clone, PartialEq, Eq)]
pub struct VirtualAudioTrackDescriptor {
    pub track_id: String,
    pub device_id: String,
    pub label: String,
    pub source_kind: VirtualAudioSourceKind,
    pub channels: u16,
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
    /// Canonical Linux PipeWire virtual audio track descriptor (`48 kHz`, `2` channels).
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

    /// Canonical Windows WASAPI virtual audio track descriptor (`48 kHz`, `2` channels).
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
    Exact(String),
    /// Preferred device with fallback to an active device when the preferred device is unplugged.
    ExactWithFallback {
        preferred_exact_id: String,
        effective_active_id: String,
    },
}

impl DeviceIdMatchConstraint {
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
    /// Complete DSP bypass (`autoGainControl: false`, `echoCancellation: false`, `noiseSuppression: false`).
    BypassedForStream,
}

/// AGC and voice DSP constraint policy applied to an audio track (`R5-08`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgcPolicyConfig {
    pub mode: AgcMode,
    pub auto_gain_control: bool,
    pub echo_cancellation: bool,
    pub noise_suppression: bool,
    pub typing_noise_detection: bool,
    pub target_level_dbfs: i8,
    pub compression_gain_db: u8,
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

    /// Fixed Digital policy for external audio interfaces with manual hardware gain (`D-04`).
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

    /// Studio passthrough policy for virtual screen-share audio (`screenShareFixes.ts` / `fixStreamConstraints.ts`).
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
    VoiceMicrophone,
    ScreenShareSystemAudio,
}

/// Negotiated constraint specification for an active audio track (`R5-08`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioTrackConstraint {
    pub role: AudioTrackRole,
    pub device_id_constraint: DeviceIdMatchConstraint,
    pub is_virtual_injected: bool,
    pub channels: u16,
    pub sample_rate_hz: u32,
    pub agc_policy: AgcPolicyConfig,
    pub track_generation: u64,
}

impl AudioTrackConstraint {
    #[must_use]
    pub fn effective_device_id(&self) -> &str {
        self.device_id_constraint.effective_id()
    }

    #[must_use]
    pub fn preferred_exact_device_id(&self) -> &str {
        self.device_id_constraint.preferred_exact_id()
    }
}

/// Canonical screen-share and video resolution presets (`480p`..`2160p`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResolutionPreset {
    P480,
    P720,
    P1080,
    P1440,
    P2160,
}

impl ResolutionPreset {
    pub const ALL: [Self; 5] = [
        Self::P480,
        Self::P720,
        Self::P1080,
        Self::P1440,
        Self::P2160,
    ];

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
    pub min_width: u32,
    pub min_height: u32,
    pub target_width: u32,
    pub target_height: u32,
    pub max_width: u32,
    pub max_height: u32,
    pub min_fps: u32,
    pub target_fps: u32,
    pub max_fps: u32,
    pub strict_lock: bool,
}

impl ResolutionBounds {
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
        self.validate_dimensions()?;
        self.validate_ordering()?;
        self.validate_fps()
    }

    fn validate_dimensions(&self) -> Result<(), MediaConstraintError> {
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
        Ok(())
    }

    fn validate_ordering(&self) -> Result<(), MediaConstraintError> {
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
        Ok(())
    }

    fn validate_fps(&self) -> Result<(), MediaConstraintError> {
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
    pub bounds: ResolutionBounds,
}

/// Input frame metadata submitted to the video encoder bounds enforcer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoEncoderFrameInput {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub timestamp_us: u64,
}

/// Decision produced by [`MediaConstraintsEngine::enforce_video_encoder_bounds`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoderFrameDecision {
    PassThrough { width: u32, height: u32, fps: u32 },
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
    pub audio: Option<AudioTrackConstraint>,
    pub video: VideoResolutionConstraint,
}

impl NegotiatedStreamConstraints {
    #[must_use]
    pub const fn speaking_flags(&self) -> u8 {
        if self.audio.is_some() {
            SOUNDSHARE_SPEAKING_BIT
        } else {
            0
        }
    }

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
    Idle,
    Active,
    Stopped,
}

/// Outcome report after handling an OS audio device topology change (`hot-plug` / `unplug`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceTopologyStabilityReport {
    pub voice_track_unchanged: bool,
    pub virtual_stream_track_unchanged: bool,
    pub voice_fallback_device_id: Option<String>,
    pub voice_restored_preferred_exact: bool,
}

/// Observable telemetry counters for the [`MediaConstraintsEngine`] (`TEST-POLICY.md` §12 dim 9).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MediaConstraintsTelemetry {
    pub negotiations_total: u64,
    pub virtual_tracks_injected: u64,
    pub exact_device_matches: u64,
    pub agc_bypassed_tracks: u64,
    pub frames_passed_within_bounds: u64,
    pub frames_clamped_to_bounds: u64,
    pub frames_dropped_out_of_bounds: u64,
    pub device_change_events_handled: u64,
    pub track_stability_preserved_count: u64,
}

/// Errors returned by the [`MediaConstraintsEngine`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaConstraintError {
    EmptyDeviceId,
    NoInputDevicesAvailable,
    ExactDeviceNotFound { device_id: String },
    WrongDeviceKind {
        device_id: String,
        expected: AudioDeviceKind,
        actual: AudioDeviceKind,
    },
    UnsupportedAudioFormat { channels: u16, sample_rate_hz: u32 },
    ResolutionOutOfRange {
        width: u32,
        height: u32,
        min_width: u32,
        min_height: u32,
        max_width: u32,
        max_height: u32,
    },
    InvertedResolutionBounds {
        min_width: u32,
        min_height: u32,
        max_width: u32,
        max_height: u32,
    },
    OddResolutionDimension { width: u32, height: u32 },
    FpsOutOfRange {
        fps: u32,
        min_fps: u32,
        max_fps: u32,
    },
    InvalidFrameDimensions { width: u32, height: u32 },
    EngineNotActive,
}

impl MediaConstraintError {
    fn write_msg(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDeviceId => f.write_str("media constraint deviceId must not be empty"),
            Self::NoInputDevicesAvailable => f.write_str("no audio input devices available"),
            Self::ExactDeviceNotFound { device_id } => {
                write!(f, "exact audio input device '{device_id}' not found")
            }
            Self::WrongDeviceKind {
                device_id,
                expected,
                actual,
            } => write!(f, "device '{device_id}' kind {actual:?} != {expected:?}"),
            Self::UnsupportedAudioFormat {
                channels,
                sample_rate_hz,
            } => write!(f, "unsupported format: {channels}ch @ {sample_rate_hz} Hz"),
            Self::ResolutionOutOfRange { width, height, .. } => {
                write!(f, "resolution {width}x{height} out of allowed bounds")
            }
            Self::InvertedResolutionBounds {
                min_width,
                min_height,
                max_width,
                max_height,
            } => write!(f, "inverted bounds: {min_width}x{min_height} > {max_width}x{max_height}"),
            Self::OddResolutionDimension { width, height } => {
                write!(f, "odd resolution {width}x{height} invalid for YUV420")
            }
            Self::FpsOutOfRange { fps, min_fps, max_fps } => {
                write!(f, "fps {fps} outside [{min_fps}..={max_fps}]")
            }
            Self::InvalidFrameDimensions { width, height } => {
                write!(f, "invalid frame dimensions {width}x{height}")
            }
            Self::EngineNotActive => f.write_str("media constraints engine is not active"),
        }
    }
}

impl fmt::Display for MediaConstraintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_msg(f)
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

    #[must_use]
    pub fn with_devices(devices: Vec<AudioDevice>) -> Self {
        let mut engine = Self::new();
        engine.devices = devices;
        engine
    }

    #[must_use]
    pub const fn lifecycle_state(&self) -> MediaConstraintsLifecycleState {
        self.lifecycle_state
    }

    #[must_use]
    pub fn available_devices(&self) -> &[AudioDevice] {
        &self.devices
    }

    #[must_use]
    pub const fn active_voice_input_constraint(&self) -> Option<&AudioTrackConstraint> {
        self.active_voice_input.as_ref()
    }

    #[must_use]
    pub const fn active_stream_constraints(&self) -> Option<&NegotiatedStreamConstraints> {
        self.active_stream.as_ref()
    }

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
        let trimmed = self.verify_exact_input_device(exact_device_id)?;
        let generation = self.allocate_generation();
        let constraint = AudioTrackConstraint {
            role: AudioTrackRole::VoiceMicrophone,
            device_id_constraint: DeviceIdMatchConstraint::Exact(trimmed),
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

    fn verify_exact_input_device(&self, raw_id: &str) -> Result<String, MediaConstraintError> {
        let trimmed = raw_id.trim();
        if trimmed.is_empty() {
            return Err(MediaConstraintError::EmptyDeviceId);
        }
        if !self.devices.iter().any(|d| d.kind == AudioDeviceKind::Input) {
            return Err(MediaConstraintError::NoInputDevicesAvailable);
        }
        let Some(device) = self.devices.iter().find(|d| d.id == trimmed) else {
            return Err(MediaConstraintError::ExactDeviceNotFound {
                device_id: trimmed.to_owned(),
            });
        };
        if device.kind != AudioDeviceKind::Input {
            return Err(MediaConstraintError::WrongDeviceKind {
                device_id: trimmed.to_owned(),
                expected: AudioDeviceKind::Input,
                actual: device.kind,
            });
        }
        Ok(trimmed.to_owned())
    }

    /// Negotiates custom screen-share constraints: optional virtual audio track injection
    /// (with exact `deviceId` matching and AGC/AEC/NS bypass) and strict video resolution bounds.
    pub fn negotiate_screen_share_constraints(
        &mut self,
        virtual_audio: Option<VirtualAudioTrackDescriptor>,
        bounds: ResolutionBounds,
    ) -> Result<NegotiatedStreamConstraints, MediaConstraintError> {
        bounds.validate()?;
        let audio_constraint = match virtual_audio {
            Some(vtrack) => Some(self.apply_virtual_audio_track(vtrack)?),
            None => {
                self.injected_virtual_track = None;
                None
            }
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

    /// Dynamically injects or replaces a virtual audio track on an active screen-share stream.
    pub fn inject_virtual_audio_track(
        &mut self,
        virtual_audio: VirtualAudioTrackDescriptor,
    ) -> Result<AudioTrackConstraint, MediaConstraintError> {
        if self.lifecycle_state != MediaConstraintsLifecycleState::Active {
            return Err(MediaConstraintError::EngineNotActive);
        }
        let constraint = self.apply_virtual_audio_track(virtual_audio)?;
        if let Some(stream) = self.active_stream.as_mut() {
            stream.audio = Some(constraint.clone());
        }
        Ok(constraint)
    }

    fn apply_virtual_audio_track(
        &mut self,
        vtrack: VirtualAudioTrackDescriptor,
    ) -> Result<AudioTrackConstraint, MediaConstraintError> {
        vtrack.validate()?;
        let generation = self.allocate_generation();
        let constraint = Self::build_virtual_audio_constraint(&vtrack, generation);
        self.ensure_virtual_device_registered(&vtrack);
        self.injected_virtual_track = Some(vtrack);
        self.telemetry.virtual_tracks_injected =
            self.telemetry.virtual_tracks_injected.saturating_add(1);
        self.telemetry.exact_device_matches = self.telemetry.exact_device_matches.saturating_add(1);
        self.telemetry.agc_bypassed_tracks = self.telemetry.agc_bypassed_tracks.saturating_add(1);
        Ok(constraint)
    }

    /// Renegotiates the video encoder resolution bounds on an active stream.
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
        let (encoded_w, encoded_h) =
            Self::clamp_dimensions_to_bounds(frame.width, frame.height, &bounds);
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
        let (voice_track_unchanged, voice_fallback_device_id, voice_restored_preferred_exact) =
            self.reconcile_voice_input_on_topology_change();
        if voice_track_unchanged {
            self.telemetry.track_stability_preserved_count = self
                .telemetry
                .track_stability_preserved_count
                .saturating_add(1);
        }
        DeviceTopologyStabilityReport {
            voice_track_unchanged,
            virtual_stream_track_unchanged: true,
            voice_fallback_device_id,
            voice_restored_preferred_exact,
        }
    }

    fn reconcile_voice_input_on_topology_change(&mut self) -> (bool, Option<String>, bool) {
        let next_gen = self.next_generation;
        let Some(voice) = self.active_voice_input.as_mut() else {
            return (true, None, false);
        };
        let pref = voice.preferred_exact_device_id().to_owned();
        let pref_ok = self.devices.iter().any(|d| d.kind == AudioDeviceKind::Input && d.id == pref);
        let fb_id = Self::find_fallback_input(&self.devices).map(|d| d.id.clone());
        match &voice.device_id_constraint {
            DeviceIdMatchConstraint::Exact(_) if !pref_ok => {
                if let Some(fb) = fb_id {
                    Self::bind_fallback_constraint(voice, pref, fb.clone(), next_gen, &mut self.next_generation);
                    return (false, Some(fb), false);
                }
                (false, None, false)
            }
            DeviceIdMatchConstraint::ExactWithFallback { effective_active_id, .. } => {
                if pref_ok {
                    voice.device_id_constraint = DeviceIdMatchConstraint::Exact(pref);
                    voice.track_generation = next_gen;
                    self.next_generation = next_gen.saturating_add(1);
                    return (false, None, true);
                }
                let act_ok = self.devices.iter().any(|d| d.kind == AudioDeviceKind::Input && &d.id == effective_active_id);
                if !act_ok && let Some(fb) = fb_id {
                    Self::bind_fallback_constraint(voice, pref, fb.clone(), next_gen, &mut self.next_generation);
                    return (false, Some(fb), false);
                }
                (true, None, false)
            }
            _ => (true, None, false),
        }
    }

    fn bind_fallback_constraint(
        voice: &mut AudioTrackConstraint,
        preferred_exact_id: String,
        effective_active_id: String,
        next_gen: u64,
        gen_counter: &mut u64,
    ) {
        voice.device_id_constraint = DeviceIdMatchConstraint::ExactWithFallback {
            preferred_exact_id,
            effective_active_id,
        };
        voice.track_generation = next_gen;
        *gen_counter = next_gen.saturating_add(1);
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
        let (mut w, mut h) = (width, height);
        if w > bounds.max_width || h > bounds.max_height {
            let (w64, h64) = (u64::from(w), u64::from(h));
            let (mw64, mh64) = (u64::from(bounds.max_width), u64::from(bounds.max_height));
            if w64 * mh64 >= h64 * mw64 {
                w = bounds.max_width;
                h = ((h64 * mw64) / w64) as u32;
            } else {
                h = bounds.max_height;
                w = ((w64 * mh64) / h64) as u32;
            }
        }
        let even_w = (w.max(bounds.min_width) & !1)
            .max(bounds.min_width)
            .min(bounds.max_width);
        let even_h = (h.max(bounds.min_height) & !1)
            .max(bounds.min_height)
            .min(bounds.max_height);
        (even_w, even_h)
    }
}
