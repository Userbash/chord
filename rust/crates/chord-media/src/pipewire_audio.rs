#![forbid(unsafe_code)]
//! Desktop Audio Capture & Virtual Microphone Loopback Service via PipeWire (`R5-01` / `F-MEDIA-04`).
//!
//! Captures desktop audio via PipeWire loopback and publishes a virtual microphone source
//! (`Audio/Source/Virtual`, 2 channels stereo, 48 kHz, `s16le`) for WebRTC streaming.
//!
//! Specification & Reference Oracles:
//! - `docs/feature-backlog.md` row `R5-01`: "Capture desktop audio via PipeWire loopback and
//!   publish a virtual microphone source (2 channels, 48 kHz, s16le) for WebRTC streaming.
//!   Target: Capture Service (`pipewire-rs`). Criteria: Desktop sound routes into WebRTC voice
//!   stream; virtual loopback device cleanly registered"
//! - `docs/vesktop-analysis.md` §2.1: native Rust implementation in `chord-media` replacing
//!   Vesktop's `venmic` virtual microphone addon.
//! - `docs/re/voice-deep.md` §2.3 & `docs/specs/STREAM-WAYLAND-GUIDE.md` §2.2:
//!   48 kHz stereo (`2` channels), 20 ms frame quantum (`960` samples/channel = `1920` interleaved
//!   `i16` samples = `3840` bytes `s16le`), Voice Gateway OP 5 SPEAKING flag `speaking: 2` (`SOUNDSHARE`).

use std::collections::VecDeque;
use std::fmt;

/// Canonical channel count for PipeWire virtual microphone loopback (2 channels / Stereo).
pub const PIPEWIRE_AUDIO_CHANNELS: u16 = 2;

/// Canonical audio sample rate for PipeWire WebRTC streaming (`48_000` Hz).
pub const PIPEWIRE_AUDIO_SAMPLE_RATE_HZ: u32 = 48_000;

/// Standard WebRTC / Opus audio frame duration in milliseconds (`20` ms).
pub const PIPEWIRE_AUDIO_FRAME_DURATION_MS: u32 = 20;

/// Number of PCM samples per channel in a 20 ms frame at 48 kHz (`960` samples).
pub const PIPEWIRE_AUDIO_FRAME_SAMPLES_PER_CHANNEL: usize = 960;

/// Total interleaved stereo samples in a 20 ms frame at 48 kHz (`960 * 2 = 1920` samples).
pub const PIPEWIRE_AUDIO_FRAME_TOTAL_SAMPLES: usize =
    PIPEWIRE_AUDIO_FRAME_SAMPLES_PER_CHANNEL * (PIPEWIRE_AUDIO_CHANNELS as usize);

/// Byte width of a single signed 16-bit little-endian (`s16le`) PCM sample (`2` bytes).
pub const S16LE_BYTES_PER_SAMPLE: usize = 2;

/// Exact byte length of a 20 ms stereo 48 kHz `s16le` audio frame (`1920 * 2 = 3840` bytes).
pub const PIPEWIRE_AUDIO_FRAME_BYTES_20MS: usize =
    PIPEWIRE_AUDIO_FRAME_TOTAL_SAMPLES * S16LE_BYTES_PER_SAMPLE;

/// Default capacity of the loopback ring buffer in 20 ms frames (`25` frames = `500` ms).
pub const DEFAULT_MAX_BUFFERED_FRAMES: usize = 25;

/// Default gain percentage (`100%` unity gain).
pub const DEFAULT_GAIN_PERCENT: u16 = 100;

/// Maximum allowed gain percentage (`200%` amplification with hard `i16` saturation).
pub const MAX_GAIN_PERCENT: u16 = 200;

/// PipeWire media class for a virtual microphone source node (`Audio/Source/Virtual`).
pub const PW_MEDIA_CLASS_AUDIO_SOURCE_VIRTUAL: &str = "Audio/Source/Virtual";

/// PipeWire media class for a hardware or virtual audio output sink (`Audio/Sink`).
pub const PW_MEDIA_CLASS_AUDIO_SINK: &str = "Audio/Sink";

/// PipeWire media class for an application audio playback stream (`Stream/Output/Audio`).
pub const PW_MEDIA_CLASS_STREAM_OUTPUT_AUDIO: &str = "Stream/Output/Audio";

/// Default PipeWire node name registered for the virtual microphone source.
pub const DEFAULT_VIRTUAL_MIC_NODE_NAME: &str = "chord-screen-share-audio";

/// Default human-readable description for the PipeWire virtual microphone node.
pub const DEFAULT_VIRTUAL_MIC_DESCRIPTION: &str = "Chord Screen Share Audio (Virtual Microphone)";

/// Discord Voice Gateway OP 5 SPEAKING bitflag for desktop soundshare (`1 << 1 = 2`).
pub const SOUNDSHARE_SPEAKING_FLAG: u8 = 2;

/// Builds the Discord Voice Gateway `OP 5 SPEAKING` JSON payload with `speaking: 2` (`SOUNDSHARE`).
#[must_use]
pub fn build_soundshare_speaking_payload(ssrc: u32) -> String {
    format!(
        "{{\"op\":5,\"d\":{{\"speaking\":{SOUNDSHARE_SPEAKING_FLAG},\"delay\":0,\"ssrc\":{ssrc}}}}}"
    )
}

/// PCM sample format negotiated on the PipeWire loopback stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeWireAudioFormat {
    /// Signed 16-bit Little-Endian PCM (`S16LE`).
    S16le,
}

impl PipeWireAudioFormat {
    /// Returns the canonical PipeWire SPA audio format identifier (`"S16LE"`).
    #[must_use]
    pub const fn spa_format_str(self) -> &'static str {
        match self {
            Self::S16le => "S16LE",
        }
    }

    /// Returns the number of bytes per single-channel sample (`2` for `S16le`).
    #[must_use]
    pub const fn bytes_per_sample(self) -> usize {
        match self {
            Self::S16le => S16LE_BYTES_PER_SAMPLE,
        }
    }
}

/// Stereo audio port channel in the PipeWire graph (`FL` or `FR`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PipeWireChannelPort {
    /// Front Left channel (`FL`).
    FrontLeft,
    /// Front Right channel (`FR`).
    FrontRight,
}

impl PipeWireChannelPort {
    /// Short channel name (`"FL"` or `"FR"`).
    #[must_use]
    pub const fn port_name(self) -> &'static str {
        match self {
            Self::FrontLeft => "FL",
            Self::FrontRight => "FR",
        }
    }

    /// Virtual microphone capture input port name (`"input_FL"` or `"input_FR"`).
    #[must_use]
    pub const fn input_port_name(self) -> &'static str {
        match self {
            Self::FrontLeft => "input_FL",
            Self::FrontRight => "input_FR",
        }
    }

    /// Sink monitor output port name (`"monitor_FL"` or `"monitor_FR"`).
    #[must_use]
    pub const fn monitor_port_name(self) -> &'static str {
        match self {
            Self::FrontLeft => "monitor_FL",
            Self::FrontRight => "monitor_FR",
        }
    }
}

/// Errors produced by the PipeWire desktop audio capture and virtual microphone service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipeWireAudioError {
    /// Channel count is not supported (R5-01 requires 2 stereo channels).
    UnsupportedChannels(u16),
    /// Sample rate is not supported (R5-01 requires 48,000 Hz).
    UnsupportedSampleRate(u32),
    /// Virtual microphone node name is empty or contains invalid characters.
    InvalidNodeName(String),
    /// Ring buffer capacity in frames must be greater than zero.
    InvalidBufferCapacity(usize),
    /// PipeWire node ID `0` is reserved/invalid.
    InvalidNodeId(u32),
    /// Target PipeWire node was not found in the audio graph.
    NodeNotFound(u32),
    /// Attempted to link the virtual microphone node to itself (acoustic feedback loop).
    SelfLoopbackForbidden(u32),
    /// Raw `s16le` byte slice length is not a multiple of 2 bytes (or stereo 4-byte frame).
    UnalignedS16leBytes(usize),
    /// Operation is invalid in the current [`PipeWireAudioStreamState`].
    InvalidState {
        /// Expected state description.
        expected: &'static str,
        /// Actual state.
        actual: PipeWireAudioStreamState,
    },
    /// PipeWire daemon socket disconnected or `pipewire.service` is unavailable.
    DaemonDisconnected(String),
}

impl fmt::Display for PipeWireAudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedChannels(ch) => write!(
                f,
                "unsupported PipeWire audio channel count {ch} (expected {PIPEWIRE_AUDIO_CHANNELS})"
            ),
            Self::UnsupportedSampleRate(rate) => write!(
                f,
                "unsupported PipeWire sample rate {rate} Hz (expected {PIPEWIRE_AUDIO_SAMPLE_RATE_HZ} Hz)"
            ),
            Self::InvalidNodeName(name) => {
                write!(f, "invalid PipeWire virtual mic node name: {name:?}")
            }
            Self::InvalidBufferCapacity(cap) => {
                write!(f, "invalid PipeWire audio buffer capacity: {cap} frames")
            }
            Self::InvalidNodeId(id) => write!(f, "invalid PipeWire node id: {id}"),
            Self::NodeNotFound(id) => write!(f, "PipeWire audio node #{id} not found"),
            Self::SelfLoopbackForbidden(id) => write!(
                f,
                "cannot link PipeWire virtual microphone node #{id} to itself"
            ),
            Self::UnalignedS16leBytes(len) => write!(
                f,
                "unaligned s16le PCM byte buffer length ({len} bytes is not a multiple of 2)"
            ),
            Self::InvalidState { expected, actual } => write!(
                f,
                "invalid PipeWire audio stream state: expected {expected}, actual {actual:?}"
            ),
            Self::DaemonDisconnected(reason) => {
                write!(f, "PipeWire daemon disconnected: {reason}")
            }
        }
    }
}

impl std::error::Error for PipeWireAudioError {}

/// Lifecycle state of the PipeWire virtual microphone & desktop audio loopback service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeWireAudioStreamState {
    /// Virtual microphone has not been registered yet.
    Unregistered,
    /// Virtual microphone node (`Audio/Source/Virtual`) is registered in PipeWire.
    Registered,
    /// One or more desktop audio nodes are linked and routing `s16le` audio into WebRTC.
    Streaming,
    /// Audio capture routing is temporarily paused.
    Paused,
    /// Virtual microphone has been unregistered and all links torn down.
    Stopped,
}

/// Configuration for registering the PipeWire virtual microphone and loopback capture buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipeWireLoopbackConfig {
    /// Technical node name (`node.name` in PipeWire properties).
    pub node_name: String,
    /// User-visible node description (`node.description` in PipeWire properties).
    pub node_description: String,
    /// PCM sample format (`S16le`).
    pub format: PipeWireAudioFormat,
    /// Number of channels (`2` for stereo).
    pub channels: u16,
    /// Sample rate in Hz (`48_000`).
    pub sample_rate_hz: u32,
    /// Maximum number of 20 ms frames held in the loopback ring buffer before evicting oldest.
    pub max_buffered_frames: usize,
    /// Initial capture gain percentage (`0..=200`, default `100`).
    pub gain_percent: u16,
}

impl Default for PipeWireLoopbackConfig {
    fn default() -> Self {
        Self {
            node_name: DEFAULT_VIRTUAL_MIC_NODE_NAME.to_string(),
            node_description: DEFAULT_VIRTUAL_MIC_DESCRIPTION.to_string(),
            format: PipeWireAudioFormat::S16le,
            channels: PIPEWIRE_AUDIO_CHANNELS,
            sample_rate_hz: PIPEWIRE_AUDIO_SAMPLE_RATE_HZ,
            max_buffered_frames: DEFAULT_MAX_BUFFERED_FRAMES,
            gain_percent: DEFAULT_GAIN_PERCENT,
        }
    }
}

impl PipeWireLoopbackConfig {
    /// Validates all configuration parameters against the R5-01 WebRTC contract.
    pub fn validate(&self) -> Result<(), PipeWireAudioError> {
        if self.channels != PIPEWIRE_AUDIO_CHANNELS {
            return Err(PipeWireAudioError::UnsupportedChannels(self.channels));
        }
        if self.sample_rate_hz != PIPEWIRE_AUDIO_SAMPLE_RATE_HZ {
            return Err(PipeWireAudioError::UnsupportedSampleRate(
                self.sample_rate_hz,
            ));
        }
        let trimmed = self.node_name.trim();
        if trimmed.is_empty()
            || self.node_name.chars().any(char::is_control)
            || self.node_description.chars().any(char::is_control)
        {
            return Err(PipeWireAudioError::InvalidNodeName(self.node_name.clone()));
        }
        if self.max_buffered_frames == 0 {
            return Err(PipeWireAudioError::InvalidBufferCapacity(0));
        }
        Ok(())
    }
}

/// Metadata of an audio node in the PipeWire graph (`Audio/Sink` or `Stream/Output/Audio`).
///
/// Enforces log privacy (`TEST-POLICY.md` / `G5-VOICE-PLAN.md` §7.4): `Debug` redacts
/// human-readable window/application titles in `description` and `application_name`.
#[derive(Clone, PartialEq, Eq)]
pub struct PipeWireAudioNode {
    /// PipeWire object `node.id`.
    pub node_id: u32,
    /// Technical node name (`node.name`).
    pub name: String,
    /// Human-readable description (`node.description` / media title).
    pub description: String,
    /// PipeWire media class (`Audio/Sink`, `Stream/Output/Audio`, etc.).
    pub media_class: String,
    /// Optional application name (`application.name`).
    pub application_name: Option<String>,
    /// Optional OS process ID (`application.process.id`).
    pub process_id: Option<u32>,
    /// Channel count (`2`).
    pub channels: u16,
    /// Sample rate in Hz (`48_000`).
    pub sample_rate_hz: u32,
}

impl fmt::Debug for PipeWireAudioNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PipeWireAudioNode")
            .field("node_id", &self.node_id)
            .field("name", &"<redacted>")
            .field("description", &"<redacted>")
            .field("media_class", &self.media_class)
            .field("application_name", &"<redacted>")
            .field("process_id", &self.process_id)
            .field("channels", &self.channels)
            .field("sample_rate_hz", &self.sample_rate_hz)
            .finish()
    }
}

/// Descriptor of a registered PipeWire virtual microphone node (`Audio/Source/Virtual`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualMicDescriptor {
    /// Assigned PipeWire `node_id` (`> 0`).
    pub node_id: u32,
    /// Registered `node.name`.
    pub node_name: String,
    /// Registered `node.description`.
    pub description: String,
    /// Media class (`"Audio/Source/Virtual"`).
    pub media_class: String,
    /// Sample format (`PipeWireAudioFormat::S16le`).
    pub format: PipeWireAudioFormat,
    /// Channel count (`2`).
    pub channels: u16,
    /// Sample rate in Hz (`48_000`).
    pub sample_rate_hz: u32,
}

/// Active link between an output/monitor port of a desktop audio node and an input port of the virtual mic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipeWirePortLink {
    /// PipeWire link object ID.
    pub link_id: u32,
    /// Source desktop audio node ID.
    pub source_node_id: u32,
    /// Source stereo channel port (`FL` or `FR`).
    pub source_port: PipeWireChannelPort,
    /// Target virtual microphone node ID.
    pub target_virtual_mic_id: u32,
    /// Target virtual microphone stereo port (`FL` or `FR`).
    pub target_port: PipeWireChannelPort,
}

/// A 20 ms stereo 48 kHz `s16le` audio frame ready for WebRTC / Opus voice transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebRtcAudioFrameS16le {
    /// Monotonic 1-based frame sequence number.
    pub sequence: u64,
    /// RTP/WebRTC timestamp in 48 kHz per-channel sample ticks (`0, 960, 1920, ...`).
    pub timestamp_samples: u64,
    /// Number of channels (`2`).
    pub channels: u16,
    /// Sample rate in Hz (`48_000`).
    pub sample_rate_hz: u32,
    /// Interleaved signed 16-bit PCM samples (`1920` samples for 20 ms stereo).
    pub samples: Vec<i16>,
    /// True if all samples in the frame are digital silence (`0`).
    pub is_silence: bool,
}

impl WebRtcAudioFrameS16le {
    /// Encodes the interleaved `i16` samples into little-endian `s16le` wire bytes (`3840` bytes).
    #[must_use]
    pub fn to_le_bytes(&self) -> Vec<u8> {
        samples_to_s16le_bytes(&self.samples)
    }
}

/// Telemetry counters for the PipeWire desktop audio capture service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PipeWireAudioTelemetry {
    /// Total 20 ms WebRTC frames drained.
    pub captured_frames: u64,
    /// Total `s16le` bytes drained across all frames.
    pub captured_bytes: u64,
    /// Number of oldest `i16` samples evicted due to ring buffer overflow.
    pub overrun_samples: u64,
    /// Number of silence fallback frames synthesized when polling an empty buffer.
    pub underrun_reads: u64,
    /// Number of active PipeWire port links (`2` per stereo source node).
    pub active_links: usize,
    /// Number of successful recoveries after `pipewire.service` daemon restarts.
    pub daemon_restarts: u64,
}

/// Abstraction over the PipeWire graph / session backend (`pipewire-rs` or test double).
pub trait PipeWireAudioBackend {
    /// Enumerates available audio sinks and application playback streams in the PipeWire graph.
    fn enumerate_nodes(&mut self) -> Result<Vec<PipeWireAudioNode>, PipeWireAudioError>;

    /// Registers a virtual microphone node (`Audio/Source/Virtual`, 2ch, 48 kHz, `s16le`).
    fn register_virtual_mic(
        &mut self,
        config: &PipeWireLoopbackConfig,
    ) -> Result<VirtualMicDescriptor, PipeWireAudioError>;

    /// Unregisters a previously created virtual microphone node by its `node_id`.
    fn unregister_virtual_mic(&mut self, node_id: u32) -> Result<(), PipeWireAudioError>;

    /// Creates a stereo pair of port links (`FL -> input_FL`, `FR -> input_FR`) from `source_node_id`
    /// into `target_virtual_mic_id`.
    fn create_stereo_link(
        &mut self,
        source_node_id: u32,
        target_virtual_mic_id: u32,
    ) -> Result<[PipeWirePortLink; 2], PipeWireAudioError>;

    /// Destroys a previously created port link by its `link_id`.
    fn destroy_link(&mut self, link_id: u32) -> Result<(), PipeWireAudioError>;
}

/// Converts a single signed 16-bit PCM sample (`i16`) to normalized floating-point `[-1.0, 1.0)`.
#[must_use]
pub fn s16le_to_f32_sample(sample: i16) -> f32 {
    (sample as f32) / 32_768.0
}

/// Converts a normalized floating-point sample `[-1.0, 1.0]` to signed 16-bit PCM (`i16`)
/// with NaN/Infinity sanitization and saturation clamping.
#[must_use]
pub fn f32_to_s16le_sample(sample: f32) -> i16 {
    if sample.is_nan() {
        return 0;
    }
    if sample >= 1.0 {
        return i16::MAX;
    }
    if sample <= -1.0 {
        return i16::MIN;
    }
    let scaled = (sample * 32_768.0).round();
    scaled.clamp(i16::MIN as f32, i16::MAX as f32) as i16
}

/// Encodes a slice of `i16` PCM samples into little-endian (`s16le`) bytes.
#[must_use]
pub fn samples_to_s16le_bytes(samples: &[i16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * S16LE_BYTES_PER_SAMPLE);
    for &sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

/// Decodes a little-endian (`s16le`) byte slice into `i16` PCM samples.
/// Returns [`PipeWireAudioError::UnalignedS16leBytes`] if `bytes.len()` is odd.
pub fn s16le_bytes_to_samples(bytes: &[u8]) -> Result<Vec<i16>, PipeWireAudioError> {
    if !bytes.len().is_multiple_of(S16LE_BYTES_PER_SAMPLE) {
        return Err(PipeWireAudioError::UnalignedS16leBytes(bytes.len()));
    }
    let mut samples = Vec::with_capacity(bytes.len() / S16LE_BYTES_PER_SAMPLE);
    for &chunk in bytes.as_chunks::<S16LE_BYTES_PER_SAMPLE>().0 {
        samples.push(i16::from_le_bytes(chunk));
    }
    Ok(samples)
}

#[inline]
fn scale_s16le_with_gain(sample: i16, gain_percent: u16) -> i16 {
    if gain_percent == 0 {
        return 0;
    }
    if gain_percent == 100 {
        return sample;
    }
    let scaled = (i32::from(sample) * i32::from(gain_percent)) / 100;
    scaled.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

/// PipeWire Desktop Audio Capture & Virtual Microphone Service (`R5-01`).
///
/// Manages:
/// - Virtual microphone node registration (`Audio/Source/Virtual`, 2 channels, 48 kHz, `s16le`);
/// - Dynamic stereo port linking from desktop sinks or application streams;
/// - Bounded `s16le` PCM ring buffering with 20 ms (`1920` stereo samples = `3840` bytes) WebRTC framing;
/// - Multi-stream saturating mixer;
/// - Automatic re-registration and link restoration when `pipewire.service` restarts;
/// - Clean RAII unregistration on `stop()` or `Drop`.
pub struct PipeWireAudioCaptureService<B: PipeWireAudioBackend> {
    backend: B,
    config: PipeWireLoopbackConfig,
    state: PipeWireAudioStreamState,
    virtual_mic: Option<VirtualMicDescriptor>,
    active_links: Vec<PipeWirePortLink>,
    linked_source_nodes: Vec<u32>,
    pcm_ring: VecDeque<i16>,
    gain_percent: u16,
    next_sequence: u64,
    next_timestamp_samples: u64,
    telemetry: PipeWireAudioTelemetry,
}

impl<B: PipeWireAudioBackend> PipeWireAudioCaptureService<B> {
    /// Creates a new PipeWire audio capture service after validating `config`.
    pub fn new(backend: B, config: PipeWireLoopbackConfig) -> Result<Self, PipeWireAudioError> {
        config.validate()?;
        let capacity_samples = config
            .max_buffered_frames
            .saturating_mul(PIPEWIRE_AUDIO_FRAME_TOTAL_SAMPLES);
        let gain_percent = config.gain_percent.min(MAX_GAIN_PERCENT);
        Ok(Self {
            backend,
            config,
            state: PipeWireAudioStreamState::Unregistered,
            virtual_mic: None,
            active_links: Vec::new(),
            linked_source_nodes: Vec::new(),
            pcm_ring: VecDeque::with_capacity(capacity_samples),
            gain_percent,
            next_sequence: 1,
            next_timestamp_samples: 0,
            telemetry: PipeWireAudioTelemetry::default(),
        })
    }

    /// Returns the current stream lifecycle state.
    #[must_use]
    pub const fn state(&self) -> PipeWireAudioStreamState {
        self.state
    }

    /// Returns the registered virtual microphone descriptor, if active.
    #[must_use]
    pub const fn virtual_mic(&self) -> Option<&VirtualMicDescriptor> {
        self.virtual_mic.as_ref()
    }

    /// Returns all active stereo port links feeding the virtual microphone.
    #[must_use]
    pub fn active_links(&self) -> &[PipeWirePortLink] {
        &self.active_links
    }

    /// Returns the current capture gain percentage (`0..=200`).
    #[must_use]
    pub const fn gain_percent(&self) -> u16 {
        self.gain_percent
    }

    /// Sets the capture gain percentage, clamping to `0..=200`.
    pub fn set_gain_percent(&mut self, gain_percent: u16) {
        self.gain_percent = gain_percent.min(MAX_GAIN_PERCENT);
    }

    /// Returns the number of `i16` samples currently queued in the ring buffer.
    #[must_use]
    pub fn buffered_samples(&self) -> usize {
        self.pcm_ring.len()
    }

    /// Returns a snapshot of service telemetry counters.
    #[must_use]
    pub const fn telemetry(&self) -> PipeWireAudioTelemetry {
        self.telemetry
    }

    /// Enumerates capturable audio nodes in the PipeWire graph.
    pub fn discover_capturable_nodes(
        &mut self,
    ) -> Result<Vec<PipeWireAudioNode>, PipeWireAudioError> {
        self.backend.enumerate_nodes()
    }

    /// Registers the virtual microphone source (`Audio/Source/Virtual`, 2ch, 48 kHz, `s16le`) in PipeWire.
    pub fn register_virtual_microphone(
        &mut self,
    ) -> Result<VirtualMicDescriptor, PipeWireAudioError> {
        if let Some(existing) = &self.virtual_mic {
            return Ok(existing.clone());
        }
        let descriptor = self.backend.register_virtual_mic(&self.config)?;
        if descriptor.node_id == 0 {
            return Err(PipeWireAudioError::InvalidNodeId(0));
        }
        self.virtual_mic = Some(descriptor.clone());
        self.state = PipeWireAudioStreamState::Registered;
        Ok(descriptor)
    }

    /// Links a desktop audio sink or application stream (`source_node_id`) into the virtual microphone.
    pub fn attach_desktop_source_node(
        &mut self,
        source_node_id: u32,
    ) -> Result<[PipeWirePortLink; 2], PipeWireAudioError> {
        if !matches!(
            self.state,
            PipeWireAudioStreamState::Registered | PipeWireAudioStreamState::Streaming
        ) {
            return Err(PipeWireAudioError::InvalidState {
                expected: "Registered or Streaming",
                actual: self.state,
            });
        }
        if source_node_id == 0 {
            return Err(PipeWireAudioError::InvalidNodeId(0));
        }
        let vmic_id = self
            .virtual_mic
            .as_ref()
            .ok_or(PipeWireAudioError::InvalidState {
                expected: "Registered or Streaming",
                actual: self.state,
            })?
            .node_id;

        if source_node_id == vmic_id {
            return Err(PipeWireAudioError::SelfLoopbackForbidden(source_node_id));
        }

        // Idempotent check: if already linked, return the existing stereo links
        let existing: Vec<PipeWirePortLink> = self
            .active_links
            .iter()
            .filter(|l| l.source_node_id == source_node_id)
            .cloned()
            .collect();
        if existing.len() == 2 {
            return Ok([existing[0].clone(), existing[1].clone()]);
        }

        let links = self.backend.create_stereo_link(source_node_id, vmic_id)?;
        self.active_links.push(links[0].clone());
        self.active_links.push(links[1].clone());
        if !self.linked_source_nodes.contains(&source_node_id) {
            self.linked_source_nodes.push(source_node_id);
        }
        self.telemetry.active_links = self.active_links.len();
        self.state = PipeWireAudioStreamState::Streaming;
        Ok(links)
    }

    /// Detaches a previously linked desktop audio source node from the virtual microphone.
    pub fn detach_desktop_source_node(
        &mut self,
        source_node_id: u32,
    ) -> Result<(), PipeWireAudioError> {
        let to_remove: Vec<u32> = self
            .active_links
            .iter()
            .filter(|l| l.source_node_id == source_node_id)
            .map(|l| l.link_id)
            .collect();

        for link_id in to_remove {
            self.backend.destroy_link(link_id)?;
        }
        self.active_links
            .retain(|l| l.source_node_id != source_node_id);
        self.linked_source_nodes.retain(|&id| id != source_node_id);
        self.telemetry.active_links = self.active_links.len();
        if self.active_links.is_empty() && self.state == PipeWireAudioStreamState::Streaming {
            self.state = PipeWireAudioStreamState::Registered;
        }
        Ok(())
    }

    /// Pauses loopback audio capture and clears any half-buffered samples.
    pub fn pause(&mut self) -> Result<(), PipeWireAudioError> {
        if self.state != PipeWireAudioStreamState::Streaming {
            return Err(PipeWireAudioError::InvalidState {
                expected: "Streaming",
                actual: self.state,
            });
        }
        self.pcm_ring.clear();
        self.state = PipeWireAudioStreamState::Paused;
        Ok(())
    }

    /// Resumes loopback audio capture from the [`PipeWireAudioStreamState::Paused`] state.
    pub fn resume(&mut self) -> Result<(), PipeWireAudioError> {
        if self.state != PipeWireAudioStreamState::Paused {
            return Err(PipeWireAudioError::InvalidState {
                expected: "Paused",
                actual: self.state,
            });
        }
        self.state = PipeWireAudioStreamState::Streaming;
        Ok(())
    }

    /// Ingests raw little-endian `s16le` PCM bytes captured from the PipeWire loopback stream.
    pub fn ingest_loopback_s16le_bytes(
        &mut self,
        bytes: &[u8],
    ) -> Result<usize, PipeWireAudioError> {
        let samples = s16le_bytes_to_samples(bytes)?;
        self.ingest_loopback_samples(&samples)
    }

    /// Ingests interleaved stereo `i16` PCM samples into the bounded loopback ring buffer.
    pub fn ingest_loopback_samples(
        &mut self,
        samples: &[i16],
    ) -> Result<usize, PipeWireAudioError> {
        if self.state == PipeWireAudioStreamState::Paused {
            return Ok(0);
        }
        if !matches!(
            self.state,
            PipeWireAudioStreamState::Registered | PipeWireAudioStreamState::Streaming
        ) {
            return Err(PipeWireAudioError::InvalidState {
                expected: "Registered or Streaming",
                actual: self.state,
            });
        }

        let max_samples = self
            .config
            .max_buffered_frames
            .saturating_mul(PIPEWIRE_AUDIO_FRAME_TOTAL_SAMPLES);
        let gain = self.gain_percent;

        for &raw in samples {
            if self.pcm_ring.len() >= max_samples {
                self.pcm_ring.pop_front();
                self.telemetry.overrun_samples = self.telemetry.overrun_samples.saturating_add(1);
            }
            self.pcm_ring.push_back(scale_s16le_with_gain(raw, gain));
        }

        Ok(samples.len())
    }

    /// Mixes multiple concurrent desktop audio streams (sample-by-sample with `i16` saturation)
    /// and ingests the combined stereo stream into the loopback ring buffer.
    pub fn ingest_mixed_loopback_streams(
        &mut self,
        streams: &[&[i16]],
    ) -> Result<usize, PipeWireAudioError> {
        let Some(max_len) = streams.iter().map(|s| s.len()).max() else {
            return Ok(0);
        };
        let mut mixed = vec![0i16; max_len];
        for (idx, out_sample) in mixed.iter_mut().enumerate() {
            let mut acc: i32 = 0;
            for stream in streams {
                if let Some(&s) = stream.get(idx) {
                    acc = acc.saturating_add(i32::from(s));
                }
            }
            *out_sample = acc.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
        }
        self.ingest_loopback_samples(&mixed)
    }

    /// Drains all complete 20 ms stereo 48 kHz `s16le` frames (`1920` samples / `3840` bytes each)
    /// currently buffered for WebRTC transmission.
    #[must_use]
    pub fn drain_webrtc_frames(&mut self) -> Vec<WebRtcAudioFrameS16le> {
        let mut frames = Vec::new();
        while self.pcm_ring.len() >= PIPEWIRE_AUDIO_FRAME_TOTAL_SAMPLES {
            frames.push(self.pop_single_frame());
        }
        frames
    }

    /// Pops the next 20 ms `s16le` frame for WebRTC, or synthesizes a zero-filled 20 ms silence frame
    /// if fewer than `1920` stereo samples are currently buffered.
    #[must_use]
    pub fn poll_next_webrtc_frame_or_silence(&mut self) -> WebRtcAudioFrameS16le {
        if self.pcm_ring.len() >= PIPEWIRE_AUDIO_FRAME_TOTAL_SAMPLES {
            self.pop_single_frame()
        } else {
            self.telemetry.underrun_reads = self.telemetry.underrun_reads.saturating_add(1);
            let seq = self.next_sequence;
            self.next_sequence = self.next_sequence.saturating_add(1);
            let ts = self.next_timestamp_samples;
            self.next_timestamp_samples = self
                .next_timestamp_samples
                .saturating_add(PIPEWIRE_AUDIO_FRAME_SAMPLES_PER_CHANNEL as u64);
            WebRtcAudioFrameS16le {
                sequence: seq,
                timestamp_samples: ts,
                channels: self.config.channels,
                sample_rate_hz: self.config.sample_rate_hz,
                samples: vec![0i16; PIPEWIRE_AUDIO_FRAME_TOTAL_SAMPLES],
                is_silence: true,
            }
        }
    }

    fn pop_single_frame(&mut self) -> WebRtcAudioFrameS16le {
        let mut samples = Vec::with_capacity(PIPEWIRE_AUDIO_FRAME_TOTAL_SAMPLES);
        let mut is_silence = true;
        for _ in 0..PIPEWIRE_AUDIO_FRAME_TOTAL_SAMPLES {
            let s = self.pcm_ring.pop_front().unwrap_or(0);
            if s != 0 {
                is_silence = false;
            }
            samples.push(s);
        }
        let seq = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(1);
        let ts = self.next_timestamp_samples;
        self.next_timestamp_samples = self
            .next_timestamp_samples
            .saturating_add(PIPEWIRE_AUDIO_FRAME_SAMPLES_PER_CHANNEL as u64);

        self.telemetry.captured_frames = self.telemetry.captured_frames.saturating_add(1);
        self.telemetry.captured_bytes = self
            .telemetry
            .captured_bytes
            .saturating_add(PIPEWIRE_AUDIO_FRAME_BYTES_20MS as u64);

        WebRtcAudioFrameS16le {
            sequence: seq,
            timestamp_samples: ts,
            channels: self.config.channels,
            sample_rate_hz: self.config.sample_rate_hz,
            samples,
            is_silence,
        }
    }

    /// Re-registers the virtual microphone node and restores all previously attached desktop
    /// audio loopback links after a `pipewire.service` daemon restart.
    pub fn recover_after_daemon_restart(
        &mut self,
    ) -> Result<VirtualMicDescriptor, PipeWireAudioError> {
        let saved_sources = self.linked_source_nodes.clone();
        self.virtual_mic = None;
        self.active_links.clear();
        self.telemetry.active_links = 0;

        let new_vmic = self.backend.register_virtual_mic(&self.config)?;
        self.virtual_mic = Some(new_vmic.clone());
        self.state = PipeWireAudioStreamState::Registered;

        for source_id in saved_sources {
            let links = self
                .backend
                .create_stereo_link(source_id, new_vmic.node_id)?;
            self.active_links.push(links[0].clone());
            self.active_links.push(links[1].clone());
            self.state = PipeWireAudioStreamState::Streaming;
        }

        self.telemetry.active_links = self.active_links.len();
        self.telemetry.daemon_restarts = self.telemetry.daemon_restarts.saturating_add(1);
        Ok(new_vmic)
    }

    /// Cleanly tears down all active PipeWire port links and unregisters the virtual microphone device.
    pub fn stop(&mut self) -> Result<(), PipeWireAudioError> {
        if self.state == PipeWireAudioStreamState::Stopped {
            return Ok(());
        }
        let link_ids: Vec<u32> = self.active_links.iter().map(|l| l.link_id).collect();
        for link_id in link_ids {
            let _ = self.backend.destroy_link(link_id);
        }
        self.active_links.clear();
        self.linked_source_nodes.clear();
        self.telemetry.active_links = 0;
        self.pcm_ring.clear();

        if let Some(vmic) = self.virtual_mic.take() {
            self.backend.unregister_virtual_mic(vmic.node_id)?;
        }
        self.state = PipeWireAudioStreamState::Stopped;
        Ok(())
    }
}

impl<B: PipeWireAudioBackend> Drop for PipeWireAudioCaptureService<B> {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
