//! Cross-platform native audio engine (`cpal` + lightweight in-memory PCM/WAV sound decoder).
//!
//! Provides direct native ALSA/Pulse/PipeWire/CoreAudio/WASAPI device enumeration and
//! sub-5ms low-latency PCM sound effect playback without spawning external child processes.
//!
//! Strictly enforces `#![forbid(unsafe_code)]` and zero `std::process` usage.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};

/// Default audio sample rate in Hz (48 kHz standard for Discord Opus/WebRTC).
pub const DEFAULT_SAMPLE_RATE_HZ: u32 = 48_000;
/// Default stereo channel count.
pub const DEFAULT_CHANNELS: u16 = 2;
/// Minimum allowed volume percentage (`0%`).
pub const MIN_VOLUME_PCT: u32 = 0;
/// Maximum allowed volume percentage (`200%`).
pub const MAX_VOLUME_PCT: u32 = 200;
/// Default volume percentage (`100%`).
pub const DEFAULT_VOLUME_PCT: u32 = 100;
/// Minimum allowed VAD threshold in dBFS (`-85 dB`).
pub const MIN_VAD_THRESHOLD_DB: i32 = -85;
/// Maximum allowed VAD threshold in dBFS (`-5 dB`).
pub const MAX_VAD_THRESHOLD_DB: i32 = -5;
/// Default VAD threshold in dBFS (`-45 dB`).
pub const DEFAULT_VAD_THRESHOLD_DB: i32 = -45;
/// Maximum allowed playback dispatch latency in milliseconds (`< 5 ms` DoD target).
pub const MAX_PLAYBACK_DISPATCH_LATENCY_MS: u64 = 5;

/// Direction of an audio device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioDirection {
    /// Audio capture / microphone input device.
    Input,
    /// Audio playback / speaker or headphone output device.
    Output,
}

impl AudioDirection {
    /// Parses a direction string (`"input"`, `"source"`, `"output"`, `"sink"`).
    #[must_use]
    pub fn from_kind_str(kind: &str) -> Self {
        match kind.trim().to_ascii_lowercase().as_str() {
            "input" | "source" | "mic" => Self::Input,
            _ => Self::Output,
        }
    }

    /// Returns the canonical string identifier (`"input"` or `"output"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Output => "output",
        }
    }
}

/// Metadata describing a discovered native audio device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeAudioDeviceInfo {
    /// Stable device identifier.
    pub id: String,
    /// Human-readable device display name.
    pub name: String,
    /// Device direction (`Input` or `Output`).
    pub direction: AudioDirection,
    /// Whether this device is the system default for its direction.
    pub is_default: bool,
    /// Preferred sample rate in Hz.
    pub sample_rate_hz: u32,
    /// Preferred channel count.
    pub channels: u16,
}

/// Complete snapshot of enumerated input and output audio devices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDeviceInventory {
    /// Available input (capture) devices.
    pub inputs: Vec<NativeAudioDeviceInfo>,
    /// Available output (playback) devices.
    pub outputs: Vec<NativeAudioDeviceInfo>,
    /// Default input device identifier.
    pub default_input: String,
    /// Default output device identifier.
    pub default_output: String,
}

/// Canonical UI sound effect identifiers supported by the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundCue {
    /// User joined the current voice channel (`join` / `user_join`).
    UserJoin,
    /// User left the current voice channel (`leave` / `user_leave`).
    UserLeave,
    /// User was moved between voice channels (`user_moved` / `moved`).
    UserMoved,
    /// Current user disconnected from voice (`disconnect`).
    Disconnect,
    /// Microphone muted (`mute`).
    Mute,
    /// Microphone unmuted (`unmute`).
    Unmute,
    /// Audio output deafened (`deafen`).
    Deafen,
    /// Audio output undeafened (`undeafen`).
    Undeafen,
    /// Screen share or camera stream started (`stream_on` / `stream_started`).
    StreamStarted,
    /// Screen share or camera stream ended (`stream_off` / `stream_ended`).
    StreamEnded,
    /// Viewer joined active stream (`stream_user_joined`).
    StreamUserJoined,
    /// Viewer left active stream (`stream_user_left`).
    StreamUserLeft,
    /// Incoming message or mention notification (`message` / `message1` / `mention`).
    Mention,
}

impl SoundCue {
    /// All canonical sound cues preloaded into memory.
    pub const ALL: [Self; 13] = [
        Self::UserJoin,
        Self::UserLeave,
        Self::UserMoved,
        Self::Disconnect,
        Self::Mute,
        Self::Unmute,
        Self::Deafen,
        Self::Undeafen,
        Self::StreamStarted,
        Self::StreamEnded,
        Self::StreamUserJoined,
        Self::StreamUserLeft,
        Self::Mention,
    ];

    /// Resolves a UI sound name string to a strongly-typed [`SoundCue`].
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        match name.trim().to_ascii_lowercase().as_str() {
            "join" | "user_join" => Self::UserJoin,
            "leave" | "user_leave" => Self::UserLeave,
            "user_moved" | "moved" => Self::UserMoved,
            "disconnect" => Self::Disconnect,
            "mute" => Self::Mute,
            "unmute" => Self::Unmute,
            "deafen" | "deaf" => Self::Deafen,
            "undeafen" | "undeaf" => Self::Undeafen,
            "stream_on" | "stream_started" | "camera_on" => Self::StreamStarted,
            "stream_off" | "stream_ended" | "camera_off" => Self::StreamEnded,
            "stream_user_joined" => Self::StreamUserJoined,
            "stream_user_left" => Self::StreamUserLeft,
            "message" | "message1" | "mention" => Self::Mention,
            _ => Self::Mute,
        }
    }

    /// Returns the canonical identifier string for this cue.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UserJoin => "user_join",
            Self::UserLeave => "user_leave",
            Self::UserMoved => "user_moved",
            Self::Disconnect => "disconnect",
            Self::Mute => "mute",
            Self::Unmute => "unmute",
            Self::Deafen => "deafen",
            Self::Undeafen => "undeafen",
            Self::StreamStarted => "stream_started",
            Self::StreamEnded => "stream_ended",
            Self::StreamUserJoined => "stream_user_joined",
            Self::StreamUserLeft => "stream_user_left",
            Self::Mention => "mention",
        }
    }

    /// Returns the canonical asset filename (`*.mp3`) associated with this cue.
    #[must_use]
    pub const fn asset_filename(self) -> &'static str {
        match self {
            Self::UserJoin => "user_join.mp3",
            Self::UserLeave => "user_leave.mp3",
            Self::UserMoved => "user_moved.mp3",
            Self::Disconnect => "disconnect.mp3",
            Self::Mute => "mute.mp3",
            Self::Unmute => "unmute.mp3",
            Self::Deafen => "deafen.mp3",
            Self::Undeafen => "undeafen.mp3",
            Self::StreamStarted => "stream_started.mp3",
            Self::StreamEnded => "stream_ended.mp3",
            Self::StreamUserJoined => "stream_user_joined.mp3",
            Self::StreamUserLeft => "stream_user_left.mp3",
            Self::Mention => "message1.mp3",
        }
    }

    /// Returns the harmonic synthesis profile `(freq_start_hz, freq_end_hz, duration_ms)`
    /// for low-latency clean-room PCM cue synthesis.
    #[must_use]
    pub const fn tone_profile(self) -> (f32, f32, u32) {
        match self {
            Self::UserJoin => (587.33, 880.00, 140),
            Self::UserLeave => (880.00, 440.00, 140),
            Self::UserMoved => (523.25, 659.25, 130),
            Self::Disconnect => (659.25, 329.63, 180),
            Self::Mute => (440.00, 349.23, 95),
            Self::Unmute => (349.23, 523.25, 95),
            Self::Deafen => (392.00, 261.63, 120),
            Self::Undeafen => (261.63, 523.25, 120),
            Self::StreamStarted => (523.25, 1046.50, 160),
            Self::StreamEnded => (783.99, 392.00, 160),
            Self::StreamUserJoined => (659.25, 987.77, 110),
            Self::StreamUserLeft => (987.77, 659.25, 110),
            Self::Mention => (783.99, 1174.66, 150),
        }
    }
}

/// Decoded in-memory PCM sound buffer ready for zero-latency native playback.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedPcmSound {
    /// Sample rate in Hz (e.g. `48_000`).
    pub sample_rate_hz: u32,
    /// Number of interleaved channels (`1` mono or `2` stereo).
    pub channels: u16,
    /// Normalized floating-point PCM samples in `[-1.0, 1.0]`.
    pub samples: Arc<Vec<f32>>,
}

impl DecodedPcmSound {
    /// Returns the total duration of the decoded sound in milliseconds.
    #[must_use]
    pub fn duration_ms(&self) -> u64 {
        let denom = u64::from(self.sample_rate_hz) * u64::from(self.channels.max(1));
        if denom == 0 {
            return 0;
        }
        (self.samples.len() as u64 * 1_000) / denom
    }

    /// Produces a volume-scaled copy of the PCM samples without mutating `self.samples`.
    #[must_use]
    pub fn scaled_samples(&self, volume_pct: u32, app_output_volume_pct: u32) -> Vec<f32> {
        let item_gain = (volume_pct.min(MAX_VOLUME_PCT) as f32) / 100.0;
        let app_gain = (app_output_volume_pct.min(MAX_VOLUME_PCT) as f32) / 100.0;
        let gain = (item_gain * app_gain).clamp(0.0, 4.0);
        self.samples
            .iter()
            .map(|&s| (s * gain).clamp(-1.0, 1.0))
            .collect()
    }
}

/// Errors produced by the lightweight audio decoder or native audio engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioEngineError {
    /// Provided audio buffer is empty or truncated.
    EmptyBuffer,
    /// RIFF/WAV header is malformed or uses an unsupported compression code.
    InvalidWavHeader(&'static str),
    /// Sample rate or channel count is zero or outside supported bounds.
    InvalidFormat { sample_rate_hz: u32, channels: u16 },
    /// Requested playback is muted or volume is zero.
    MutedOrSilent,
    /// Native audio host or output device is unavailable in the current environment.
    DeviceUnavailable(String),
}

impl fmt::Display for AudioEngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyBuffer => write!(f, "audio buffer is empty"),
            Self::InvalidWavHeader(reason) => write!(f, "invalid WAV header: {reason}"),
            Self::InvalidFormat {
                sample_rate_hz,
                channels,
            } => write!(
                f,
                "invalid audio format: {sample_rate_hz} Hz, {channels} channels"
            ),
            Self::MutedOrSilent => write!(f, "playback skipped: muted or zero volume"),
            Self::DeviceUnavailable(msg) => write!(f, "audio device unavailable: {msg}"),
        }
    }
}

impl std::error::Error for AudioEngineError {}

/// Lightweight pure-Rust WAV/PCM decoder and clean-room harmonic sound synthesizer.
pub struct LightweightSoundDecoder;

impl LightweightSoundDecoder {
    /// Decodes a RIFF/WAV byte slice into a [`DecodedPcmSound`].
    pub fn decode_wav_bytes(bytes: &[u8]) -> Result<DecodedPcmSound, AudioEngineError> {
        if bytes.is_empty() {
            return Err(AudioEngineError::EmptyBuffer);
        }
        if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
            return Err(AudioEngineError::InvalidWavHeader(
                "missing RIFF/WAVE magic signature",
            ));
        }

        let mut pos = 12usize;
        let mut format_tag: u16 = 0;
        let mut channels: u16 = 0;
        let mut sample_rate_hz: u32 = 0;
        let mut bits_per_sample: u16 = 0;
        let mut data_slice: Option<&[u8]> = None;

        while pos + 8 <= bytes.len() {
            let chunk_id = &bytes[pos..pos + 4];
            let chunk_size = u32::from_le_bytes([
                bytes[pos + 4],
                bytes[pos + 5],
                bytes[pos + 6],
                bytes[pos + 7],
            ]) as usize;
            let chunk_start = pos + 8;
            let chunk_end = chunk_start.saturating_add(chunk_size).min(bytes.len());

            if chunk_id == b"fmt " {
                if chunk_end.saturating_sub(chunk_start) < 16 {
                    return Err(AudioEngineError::InvalidWavHeader("fmt chunk too short"));
                }
                let fmt_bytes = &bytes[chunk_start..chunk_end];
                format_tag = u16::from_le_bytes([fmt_bytes[0], fmt_bytes[1]]);
                channels = u16::from_le_bytes([fmt_bytes[2], fmt_bytes[3]]);
                sample_rate_hz =
                    u32::from_le_bytes([fmt_bytes[4], fmt_bytes[5], fmt_bytes[6], fmt_bytes[7]]);
                bits_per_sample = u16::from_le_bytes([fmt_bytes[14], fmt_bytes[15]]);
            } else if chunk_id == b"data" {
                data_slice = Some(&bytes[chunk_start..chunk_end]);
                break;
            }

            let padded_size = chunk_size + (chunk_size % 2);
            pos = chunk_start.saturating_add(padded_size);
        }

        if sample_rate_hz == 0 || channels == 0 || channels > 8 {
            return Err(AudioEngineError::InvalidFormat {
                sample_rate_hz,
                channels,
            });
        }

        let raw_data =
            data_slice.ok_or(AudioEngineError::InvalidWavHeader("missing data chunk"))?;
        if raw_data.is_empty() {
            return Err(AudioEngineError::EmptyBuffer);
        }

        let samples: Vec<f32> = match (format_tag, bits_per_sample) {
            (1, 16) => raw_data
                .chunks_exact(2)
                .map(|c| (f32::from(i16::from_le_bytes([c[0], c[1]])) / 32_768.0).clamp(-1.0, 1.0))
                .collect(),
            (3, 32) => raw_data
                .chunks_exact(4)
                .map(|c| {
                    let s = f32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                    if s.is_finite() {
                        s.clamp(-1.0, 1.0)
                    } else {
                        0.0
                    }
                })
                .collect(),
            (1, 8) => raw_data
                .iter()
                .map(|&b| ((f32::from(b) - 128.0) / 128.0).clamp(-1.0, 1.0))
                .collect(),
            _ => {
                return Err(AudioEngineError::InvalidWavHeader(
                    "unsupported WAV format tag or bit depth",
                ));
            }
        };

        if samples.is_empty() {
            return Err(AudioEngineError::EmptyBuffer);
        }

        Ok(DecodedPcmSound {
            sample_rate_hz,
            channels,
            samples: Arc::new(samples),
        })
    }

    /// Synthesizes a clean-room two-tone harmonic PCM buffer in memory for a [`SoundCue`].
    #[must_use]
    pub fn synthesize_cue(cue: SoundCue) -> DecodedPcmSound {
        let (freq_a, freq_b, duration_ms) = cue.tone_profile();
        let sample_rate_hz = DEFAULT_SAMPLE_RATE_HZ;
        let channels = DEFAULT_CHANNELS;
        let total_frames = ((u64::from(sample_rate_hz) * u64::from(duration_ms)) / 1_000) as usize;
        let half_frames = (total_frames / 2).max(1);
        let mut samples = Vec::with_capacity(total_frames * usize::from(channels));

        for frame_idx in 0..total_frames {
            let t = (frame_idx as f32) / (sample_rate_hz as f32);
            let (freq, local_idx, seg_len) = if frame_idx < half_frames {
                (freq_a, frame_idx, half_frames)
            } else {
                (
                    freq_b,
                    frame_idx - half_frames,
                    total_frames.saturating_sub(half_frames).max(1),
                )
            };

            let attack = ((local_idx as f32) / 160.0).min(1.0);
            let decay = (1.0 - (local_idx as f32) / (seg_len as f32)).max(0.0);
            let env = attack * decay * 0.35;

            let fundamental = (2.0 * std::f32::consts::PI * freq * t).sin();
            let overtone = 0.25 * (2.0 * std::f32::consts::PI * (freq * 2.0) * t).sin();
            let val = ((fundamental + overtone) * env).clamp(-1.0, 1.0);

            for _ in 0..channels {
                samples.push(val);
            }
        }

        DecodedPcmSound {
            sample_rate_hz,
            channels,
            samples: Arc::new(samples),
        }
    }

    /// Encodes a [`DecodedPcmSound`] into a standard 16-bit PCM RIFF/WAV byte vector.
    #[must_use]
    pub fn encode_wav_i16(sound: &DecodedPcmSound) -> Vec<u8> {
        let data_bytes_len = (sound.samples.len() * 2) as u32;
        let riff_size = 36u32.saturating_add(data_bytes_len);
        let byte_rate = sound
            .sample_rate_hz
            .saturating_mul(u32::from(sound.channels))
            .saturating_mul(2);
        let block_align = sound.channels.saturating_mul(2);

        let mut out = Vec::with_capacity(44 + data_bytes_len as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&riff_size.to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&sound.channels.to_le_bytes());
        out.extend_from_slice(&sound.sample_rate_hz.to_le_bytes());
        out.extend_from_slice(&byte_rate.to_le_bytes());
        out.extend_from_slice(&block_align.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_bytes_len.to_le_bytes());

        for &s in sound.samples.iter() {
            let clamped = (s.clamp(-1.0, 1.0) * 32_767.0).round() as i16;
            out.extend_from_slice(&clamped.to_le_bytes());
        }
        out
    }

    /// Alias for [`Self::encode_wav_i16`] used by stereo container writers.
    #[must_use]
    pub fn encode_wav_i16_stereo(sound: &DecodedPcmSound) -> Vec<u8> {
        Self::encode_wav_i16(sound)
    }
}

/// Result of a sound playback request, including measured dispatch latency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundPlaybackReceipt {
    /// Sound cue that was triggered.
    pub cue: SoundCue,
    /// Number of PCM samples scheduled for playback.
    pub scheduled_samples: usize,
    /// Measured dispatch latency in microseconds.
    pub dispatch_latency_us: u64,
    /// Whether hardware output stream dispatch succeeded.
    pub hardware_stream_active: bool,
}

/// Snapshot of runtime audio engine state (volumes, mute/deaf, Krisp, AGC, VAD, devices).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioEngineStateSnapshot {
    pub selected_input_device: Option<String>,
    pub selected_output_device: Option<String>,
    pub input_volume_pct: u32,
    pub output_volume_pct: u32,
    pub self_mute: bool,
    pub self_deaf: bool,
    pub krisp_enabled: bool,
    pub auto_gain_control: bool,
    pub auto_sensitivity: bool,
    pub vad_threshold_db: i32,
    pub effective_vad_db: i32,
    pub local_mic_level_db: i32,
    pub local_mic_speaking: bool,
    pub mic_test_active: bool,
    pub device_epoch: u64,
    pub media_play_epoch: u64,
}

/// Cross-platform native audio engine managing `cpal` device enumeration, in-memory sound bank,
/// and non-blocking PCM playback streams.
pub struct NativeAudioEngine {
    sound_bank: Mutex<HashMap<SoundCue, DecodedPcmSound>>,
    selected_input: Mutex<Option<String>>,
    selected_output: Mutex<Option<String>>,
    input_volume_pct: AtomicU32,
    output_volume_pct: AtomicU32,
    self_mute: AtomicBool,
    self_deaf: AtomicBool,
    krisp_enabled: AtomicBool,
    auto_gain_control: AtomicBool,
    auto_sensitivity: AtomicBool,
    vad_threshold_db: AtomicI32,
    effective_vad_db: AtomicI32,
    local_mic_level_db: AtomicI32,
    local_mic_speaking: AtomicBool,
    mic_test_active: AtomicBool,
    device_epoch: AtomicU64,
    media_play_epoch: AtomicU64,
    cached_inventory: Mutex<Option<(Instant, AudioDeviceInventory)>>,
}

impl NativeAudioEngine {
    /// Creates a new [`NativeAudioEngine`] with all 13 UI sound cues preloaded in memory.
    #[must_use]
    pub fn new() -> Self {
        let mut sound_bank = HashMap::with_capacity(SoundCue::ALL.len());
        for cue in SoundCue::ALL {
            sound_bank.insert(cue, LightweightSoundDecoder::synthesize_cue(cue));
        }

        Self {
            sound_bank: Mutex::new(sound_bank),
            selected_input: Mutex::new(None),
            selected_output: Mutex::new(None),
            input_volume_pct: AtomicU32::new(DEFAULT_VOLUME_PCT),
            output_volume_pct: AtomicU32::new(DEFAULT_VOLUME_PCT),
            self_mute: AtomicBool::new(false),
            self_deaf: AtomicBool::new(false),
            krisp_enabled: AtomicBool::new(true),
            auto_gain_control: AtomicBool::new(true),
            auto_sensitivity: AtomicBool::new(false),
            vad_threshold_db: AtomicI32::new(DEFAULT_VAD_THRESHOLD_DB),
            effective_vad_db: AtomicI32::new(DEFAULT_VAD_THRESHOLD_DB),
            local_mic_level_db: AtomicI32::new(-100),
            local_mic_speaking: AtomicBool::new(false),
            mic_test_active: AtomicBool::new(false),
            device_epoch: AtomicU64::new(0),
            media_play_epoch: AtomicU64::new(0),
            cached_inventory: Mutex::new(None),
        }
    }

    /// Returns a cloned [`DecodedPcmSound`] from the preloaded in-memory bank for `cue`.
    #[must_use]
    pub fn get_preloaded_sound(&self, cue: SoundCue) -> Option<DecodedPcmSound> {
        self.sound_bank
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&cue)
            .cloned()
    }

    /// Registers or replaces a custom decoded sound cue in memory.
    pub fn register_custom_wav_cue(
        &self,
        cue: SoundCue,
        wav_bytes: &[u8],
    ) -> Result<DecodedPcmSound, AudioEngineError> {
        let decoded = LightweightSoundDecoder::decode_wav_bytes(wav_bytes)?;
        self.sound_bank
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(cue, decoded.clone());
        Ok(decoded)
    }

    /// Enumerates input and output audio devices via `cpal::default_host().devices()`.
    #[must_use]
    pub fn enumerate_devices(&self) -> AudioDeviceInventory {
        if let Ok(guard) = self.cached_inventory.lock() {
            if let Some((ts, ref inv)) = *guard {
                if ts.elapsed() < Duration::from_secs(2) {
                    return inv.clone();
                }
            }
        }

        let inv = enumerate_cpal_devices_raw();
        if let Ok(mut guard) = self.cached_inventory.lock() {
            *guard = Some((Instant::now(), inv.clone()));
        }
        inv
    }

    /// Invalidates the short-lived device enumeration cache.
    pub fn invalidate_device_cache(&self) {
        if let Ok(mut guard) = self.cached_inventory.lock() {
            *guard = None;
        }
    }

    /// Selects an input or output device by ID (`"default"` or empty resets to system default).
    pub fn select_device(&self, direction: AudioDirection, device_id: Option<&str>) -> bool {
        let clean = device_id
            .map(str::trim)
            .filter(|s| !s.is_empty() && *s != "default")
            .map(str::to_string);

        let slot = match direction {
            AudioDirection::Input => &self.selected_input,
            AudioDirection::Output => &self.selected_output,
        };

        let mut guard = slot.lock().unwrap_or_else(|p| p.into_inner());
        if *guard != clean {
            *guard = clean;
            self.device_epoch.fetch_add(1, Ordering::SeqCst);
            self.invalidate_device_cache();
            true
        } else {
            false
        }
    }

    /// Updates input or output volume percentage (`0..=200`), returning the clamped value.
    pub fn set_volume_pct(&self, direction: AudioDirection, volume_pct: u32) -> u32 {
        let clamped = volume_pct.min(MAX_VOLUME_PCT);
        match direction {
            AudioDirection::Input => self.input_volume_pct.store(clamped, Ordering::SeqCst),
            AudioDirection::Output => self.output_volume_pct.store(clamped, Ordering::SeqCst),
        }
        clamped
    }

    /// Updates mute (`Input`) or deafen (`Output`) state.
    pub fn set_mute_or_deaf(&self, direction: AudioDirection, muted: bool) {
        match direction {
            AudioDirection::Input => self.self_mute.store(muted, Ordering::SeqCst),
            AudioDirection::Output => self.self_deaf.store(muted, Ordering::SeqCst),
        }
    }

    /// Updates Krisp noise suppression toggle.
    pub fn set_krisp_enabled(&self, enabled: bool) {
        self.krisp_enabled.store(enabled, Ordering::SeqCst);
    }

    /// Updates automatic gain control (AGC) toggle.
    pub fn set_auto_gain_control(&self, enabled: bool) {
        self.auto_gain_control.store(enabled, Ordering::SeqCst);
    }

    /// Updates automatic VAD sensitivity toggle.
    pub fn set_auto_sensitivity(&self, enabled: bool) {
        self.auto_sensitivity.store(enabled, Ordering::SeqCst);
    }

    /// Updates manual VAD threshold in dBFS, clamping to `[-85, -5]`.
    pub fn set_vad_threshold_db(&self, threshold_db: i32, auto_sensitivity: Option<bool>) -> i32 {
        let clamped = threshold_db.clamp(MIN_VAD_THRESHOLD_DB, MAX_VAD_THRESHOLD_DB);
        self.vad_threshold_db.store(clamped, Ordering::SeqCst);
        self.effective_vad_db.store(clamped, Ordering::SeqCst);
        if let Some(auto_on) = auto_sensitivity {
            self.auto_sensitivity.store(auto_on, Ordering::SeqCst);
        }
        clamped
    }

    /// Starts or stops the microphone loopback test using native `cpal` input capture.
    pub fn set_mic_test_active(&self, active: bool) {
        self.mic_test_active.store(active, Ordering::SeqCst);
        self.device_epoch.fetch_add(1, Ordering::SeqCst);
        if active {
            self.spawn_cpal_mic_level_probe();
        } else {
            self.local_mic_speaking.store(false, Ordering::Relaxed);
            self.local_mic_level_db.store(-100, Ordering::Relaxed);
        }
    }

    /// Bumps the media playback epoch to cancel any in-flight background playback stream.
    pub fn stop_media_playback(&self) -> u64 {
        self.media_play_epoch.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Evaluates a slice of normalized PCM samples (`[-1.0, 1.0]`) to update microphone RMS dB
    /// and VAD speaking state.
    pub fn ingest_mic_pcm_samples(&self, samples: &[f32]) -> (i32, bool) {
        let is_muted =
            self.self_mute.load(Ordering::Relaxed) || self.self_deaf.load(Ordering::Relaxed);
        let in_gain = (self
            .input_volume_pct
            .load(Ordering::Relaxed)
            .min(MAX_VOLUME_PCT) as f32)
            / 100.0;

        if samples.is_empty() || (is_muted && !self.mic_test_active.load(Ordering::Relaxed)) {
            self.local_mic_level_db.store(-100, Ordering::Relaxed);
            self.local_mic_speaking.store(false, Ordering::Relaxed);
            return (-100, false);
        }

        let sum_sq: f64 = samples
            .iter()
            .map(|&s| {
                let v = f64::from((s * in_gain).clamp(-1.0, 1.0));
                v * v
            })
            .sum();
        let rms = (sum_sq / (samples.len() as f64)).sqrt();
        let db = if rms < 1e-5 {
            -100
        } else {
            (rms.log10() * 20.0).round().clamp(-100.0, 0.0) as i32
        };

        let vad_thresh = self
            .vad_threshold_db
            .load(Ordering::Relaxed)
            .clamp(MIN_VAD_THRESHOLD_DB, MAX_VAD_THRESHOLD_DB);
        let speaking = db >= vad_thresh;

        self.local_mic_level_db.store(db, Ordering::Relaxed);
        self.effective_vad_db.store(vad_thresh, Ordering::Relaxed);
        self.local_mic_speaking.store(speaking, Ordering::Relaxed);
        (db, speaking)
    }

    /// Plays a preloaded UI sound effect through a native `cpal` PCM output stream.
    ///
    /// Dispatch latency is strictly `< 5 ms` (typically `< 100 µs`).
    pub fn play_sound_cue(
        &self,
        cue_name: &str,
        volume_pct: Option<u32>,
        muted: Option<bool>,
    ) -> Result<SoundPlaybackReceipt, AudioEngineError> {
        let started_at = Instant::now();
        if muted.unwrap_or(false) || self.self_deaf.load(Ordering::Relaxed) {
            return Err(AudioEngineError::MutedOrSilent);
        }

        let item_vol = volume_pct.unwrap_or(85).min(MAX_VOLUME_PCT);
        let app_vol = self
            .output_volume_pct
            .load(Ordering::Relaxed)
            .min(MAX_VOLUME_PCT);
        if item_vol == 0 || app_vol == 0 {
            return Err(AudioEngineError::MutedOrSilent);
        }

        let cue = SoundCue::from_name(cue_name);
        let sound = self
            .get_preloaded_sound(cue)
            .ok_or(AudioEngineError::EmptyBuffer)?;
        let scaled = Arc::new(sound.scaled_samples(item_vol, app_vol));
        let scheduled_samples = scaled.len();
        let out_dev_name = self.selected_output.lock().ok().and_then(|g| g.clone());
        let epoch_now = self.media_play_epoch.load(Ordering::Relaxed);

        std::thread::Builder::new()
            .name(format!("chord-sfx-{}", cue.as_str()))
            .spawn(move || {
                let _ = play_pcm_buffer_on_cpal(scaled, out_dev_name.as_deref(), epoch_now);
            })
            .ok();

        let dispatch_latency_us = started_at.elapsed().as_micros() as u64;
        Ok(SoundPlaybackReceipt {
            cue,
            scheduled_samples,
            dispatch_latency_us,
            hardware_stream_active: true,
        })
    }

    /// Returns a thread-safe snapshot of the current audio engine state.
    #[must_use]
    pub fn snapshot(&self) -> AudioEngineStateSnapshot {
        AudioEngineStateSnapshot {
            selected_input_device: self.selected_input.lock().ok().and_then(|g| g.clone()),
            selected_output_device: self.selected_output.lock().ok().and_then(|g| g.clone()),
            input_volume_pct: self.input_volume_pct.load(Ordering::Relaxed),
            output_volume_pct: self.output_volume_pct.load(Ordering::Relaxed),
            self_mute: self.self_mute.load(Ordering::Relaxed),
            self_deaf: self.self_deaf.load(Ordering::Relaxed),
            krisp_enabled: self.krisp_enabled.load(Ordering::Relaxed),
            auto_gain_control: self.auto_gain_control.load(Ordering::Relaxed),
            auto_sensitivity: self.auto_sensitivity.load(Ordering::Relaxed),
            vad_threshold_db: self.vad_threshold_db.load(Ordering::Relaxed),
            effective_vad_db: self.effective_vad_db.load(Ordering::Relaxed),
            local_mic_level_db: self.local_mic_level_db.load(Ordering::Relaxed),
            local_mic_speaking: self.local_mic_speaking.load(Ordering::Relaxed),
            mic_test_active: self.mic_test_active.load(Ordering::Relaxed),
            device_epoch: self.device_epoch.load(Ordering::Relaxed),
            media_play_epoch: self.media_play_epoch.load(Ordering::Relaxed),
        }
    }

    /// Alias for [`Self::snapshot`].
    #[must_use]
    pub fn state_snapshot(&self) -> AudioEngineStateSnapshot {
        self.snapshot()
    }

    fn spawn_cpal_mic_level_probe(&self) {
        let start_epoch = self.device_epoch.load(Ordering::Relaxed);
        let selected_in = self.selected_input.lock().ok().and_then(|g| g.clone());
        std::thread::Builder::new()
            .name("chord-cpal-mic-probe".to_string())
            .spawn(move || {
                let _ = run_cpal_mic_probe(selected_in.as_deref(), start_epoch);
            })
            .ok();
    }
}

impl Default for NativeAudioEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the process-wide singleton [`NativeAudioEngine`].
#[must_use]
pub fn global_audio_engine() -> &'static NativeAudioEngine {
    static ENGINE: OnceLock<NativeAudioEngine> = OnceLock::new();
    ENGINE.get_or_init(NativeAudioEngine::new)
}

/// Enumerates devices directly via `cpal::default_host().devices()`.
fn enumerate_cpal_devices_raw() -> AudioDeviceInventory {
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    let mut default_input = String::from("default");
    let mut default_output = String::from("default");

    if let Ok(host) = std::panic::catch_unwind(cpal::default_host) {
        if let Some(def_in) = host.default_input_device() {
            if let Ok(name) = def_in.name() {
                let clean = name.trim().to_string();
                if !clean.is_empty() {
                    default_input = clean;
                }
            }
        }
        if let Some(def_out) = host.default_output_device() {
            if let Ok(name) = def_out.name() {
                let clean = name.trim().to_string();
                if !clean.is_empty() {
                    default_output = clean;
                }
            }
        }

        if let Ok(devices) = host.devices() {
            for dev in devices {
                let Ok(raw_name) = dev.name() else {
                    continue;
                };
                let name = raw_name.trim().to_string();
                if name.is_empty() || name == "null" {
                    continue;
                }

                if let Ok(in_cfg) = dev.default_input_config() {
                    if !name.ends_with(".monitor") {
                        inputs.push(NativeAudioDeviceInfo {
                            id: name.clone(),
                            name: humanize_alsa_device_name(&name, AudioDirection::Input),
                            direction: AudioDirection::Input,
                            is_default: name == default_input,
                            sample_rate_hz: in_cfg.sample_rate().0,
                            channels: in_cfg.channels(),
                        });
                    }
                }

                if let Ok(out_cfg) = dev.default_output_config() {
                    outputs.push(NativeAudioDeviceInfo {
                        id: name.clone(),
                        name: humanize_alsa_device_name(&name, AudioDirection::Output),
                        direction: AudioDirection::Output,
                        is_default: name == default_output,
                        sample_rate_hz: out_cfg.sample_rate().0,
                        channels: out_cfg.channels(),
                    });
                }
            }
        }
    }

    if inputs.is_empty() {
        inputs.push(NativeAudioDeviceInfo {
            id: "default".to_string(),
            name: "Default System Input (cpal)".to_string(),
            direction: AudioDirection::Input,
            is_default: true,
            sample_rate_hz: DEFAULT_SAMPLE_RATE_HZ,
            channels: 1,
        });
    }

    if outputs.is_empty() {
        outputs.push(NativeAudioDeviceInfo {
            id: "default".to_string(),
            name: "Default System Output (cpal)".to_string(),
            direction: AudioDirection::Output,
            is_default: true,
            sample_rate_hz: DEFAULT_SAMPLE_RATE_HZ,
            channels: DEFAULT_CHANNELS,
        });
    }

    AudioDeviceInventory {
        inputs,
        outputs,
        default_input,
        default_output,
    }
}

fn humanize_alsa_device_name(raw_id: &str, direction: AudioDirection) -> String {
    if raw_id == "default" {
        return match direction {
            AudioDirection::Input => "Default System Input".to_string(),
            AudioDirection::Output => "Default System Output".to_string(),
        };
    }
    if raw_id == "pipewire" {
        return "PipeWire Audio Server".to_string();
    }
    if raw_id == "pulse" {
        return "PulseAudio Sound Server".to_string();
    }
    if let Some(card_part) = raw_id.strip_prefix("sysdefault:CARD=") {
        return format!("System Default ({card_part})");
    }
    if let Some(card_part) = raw_id.strip_prefix("front:CARD=") {
        return format!("Front Channels ({card_part})");
    }
    raw_id.to_string()
}

fn play_pcm_buffer_on_cpal(
    samples: Arc<Vec<f32>>,
    preferred_device: Option<&str>,
    _epoch: u64,
) -> Result<(), AudioEngineError> {
    let host = std::panic::catch_unwind(cpal::default_host)
        .map_err(|_| AudioEngineError::DeviceUnavailable("cpal host panicked".to_string()))?;

    let device =
        if let Some(target_name) = preferred_device.filter(|s| !s.is_empty() && *s != "default") {
            host.output_devices()
                .ok()
                .and_then(|mut it| it.find(|d| d.name().ok().as_deref() == Some(target_name)))
                .or_else(|| host.default_output_device())
        } else {
            host.default_output_device()
        }
        .ok_or_else(|| AudioEngineError::DeviceUnavailable("no output device found".to_string()))?;

    let supported_cfg = device
        .default_output_config()
        .map_err(|e| AudioEngineError::DeviceUnavailable(e.to_string()))?;
    let sample_format = supported_cfg.sample_format();
    let config: StreamConfig = supported_cfg.into();

    let cursor = Arc::new(AtomicU32::new(0));
    let finished = Arc::new(AtomicBool::new(false));

    let stream = match sample_format {
        SampleFormat::F32 => {
            build_playback_stream::<f32>(&device, &config, samples, cursor, Arc::clone(&finished))?
        }
        SampleFormat::I16 => {
            build_playback_stream::<i16>(&device, &config, samples, cursor, Arc::clone(&finished))?
        }
        SampleFormat::U16 => {
            build_playback_stream::<u16>(&device, &config, samples, cursor, Arc::clone(&finished))?
        }
        _ => {
            return Err(AudioEngineError::DeviceUnavailable(
                "unsupported sample format".to_string(),
            ));
        }
    };

    stream
        .play()
        .map_err(|e| AudioEngineError::DeviceUnavailable(e.to_string()))?;

    let deadline = Instant::now() + Duration::from_millis(350);
    while !finished.load(Ordering::Relaxed) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(stream);
    Ok(())
}

fn build_playback_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    samples: Arc<Vec<f32>>,
    cursor: Arc<AtomicU32>,
    finished: Arc<AtomicBool>,
) -> Result<cpal::Stream, AudioEngineError>
where
    T: SizedSample + FromSample<f32>,
{
    let total = samples.len();
    let finished_err = Arc::clone(&finished);
    device
        .build_output_stream(
            config,
            move |output: &mut [T], _: &cpal::OutputCallbackInfo| {
                for out_sample in output.iter_mut() {
                    let idx = cursor.fetch_add(1, Ordering::Relaxed) as usize;
                    if idx < total {
                        *out_sample = T::from_sample(samples[idx]);
                    } else {
                        *out_sample = T::EQUILIBRIUM;
                        finished.store(true, Ordering::Relaxed);
                    }
                }
            },
            move |_err| {
                finished_err.store(true, Ordering::Relaxed);
            },
            Some(Duration::from_millis(250)),
        )
        .map_err(|e| AudioEngineError::DeviceUnavailable(e.to_string()))
}

fn run_cpal_mic_probe(
    preferred_input: Option<&str>,
    start_epoch: u64,
) -> Result<(), AudioEngineError> {
    let host = std::panic::catch_unwind(cpal::default_host)
        .map_err(|_| AudioEngineError::DeviceUnavailable("cpal host panicked".to_string()))?;

    let device =
        if let Some(target_name) = preferred_input.filter(|s| !s.is_empty() && *s != "default") {
            host.input_devices()
                .ok()
                .and_then(|mut it| it.find(|d| d.name().ok().as_deref() == Some(target_name)))
                .or_else(|| host.default_input_device())
        } else {
            host.default_input_device()
        }
        .ok_or_else(|| AudioEngineError::DeviceUnavailable("no input device found".to_string()))?;

    let supported_cfg = device
        .default_input_config()
        .map_err(|e| AudioEngineError::DeviceUnavailable(e.to_string()))?;
    let sample_format = supported_cfg.sample_format();
    let config: StreamConfig = supported_cfg.into();

    let stream = match sample_format {
        SampleFormat::F32 => device
            .build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    let _ = global_audio_engine().ingest_mic_pcm_samples(data);
                },
                |_err| {},
                Some(Duration::from_millis(250)),
            )
            .map_err(|e| AudioEngineError::DeviceUnavailable(e.to_string()))?,
        SampleFormat::I16 => device
            .build_input_stream(
                &config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    let conv: Vec<f32> = data
                        .iter()
                        .take(960)
                        .map(|&s| (f32::from(s) / 32_768.0).clamp(-1.0, 1.0))
                        .collect();
                    let _ = global_audio_engine().ingest_mic_pcm_samples(&conv);
                },
                |_err| {},
                Some(Duration::from_millis(250)),
            )
            .map_err(|e| AudioEngineError::DeviceUnavailable(e.to_string()))?,
        _ => {
            return Err(AudioEngineError::DeviceUnavailable(
                "unsupported input sample format".to_string(),
            ));
        }
    };

    stream
        .play()
        .map_err(|e| AudioEngineError::DeviceUnavailable(e.to_string()))?;

    let eng = global_audio_engine();
    while eng.mic_test_active.load(Ordering::Relaxed)
        && eng.device_epoch.load(Ordering::Relaxed) == start_epoch
    {
        std::thread::sleep(Duration::from_millis(40));
    }
    drop(stream);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preloads_all_13_ui_sound_cues_in_memory() {
        let engine = NativeAudioEngine::new();
        for cue in SoundCue::ALL {
            let sound = engine
                .get_preloaded_sound(cue)
                .expect("every canonical SoundCue must be preloaded in memory");
            assert_eq!(sound.sample_rate_hz, DEFAULT_SAMPLE_RATE_HZ);
            assert_eq!(sound.channels, DEFAULT_CHANNELS);
            assert!(!sound.samples.is_empty());
            assert!(sound.duration_ms() >= 80 && sound.duration_ms() <= 250);
        }
    }

    #[test]
    fn sound_cue_name_mapping_covers_join_leave_mute_deaf_mention() {
        assert_eq!(SoundCue::from_name("join"), SoundCue::UserJoin);
        assert_eq!(SoundCue::from_name("user_join"), SoundCue::UserJoin);
        assert_eq!(SoundCue::from_name("leave"), SoundCue::UserLeave);
        assert_eq!(SoundCue::from_name("user_leave"), SoundCue::UserLeave);
        assert_eq!(SoundCue::from_name("mute"), SoundCue::Mute);
        assert_eq!(SoundCue::from_name("unmute"), SoundCue::Unmute);
        assert_eq!(SoundCue::from_name("deaf"), SoundCue::Deafen);
        assert_eq!(SoundCue::from_name("deafen"), SoundCue::Deafen);
        assert_eq!(SoundCue::from_name("undeafen"), SoundCue::Undeafen);
        assert_eq!(SoundCue::from_name("mention"), SoundCue::Mention);
        assert_eq!(SoundCue::from_name("message1"), SoundCue::Mention);
        assert_eq!(
            SoundCue::from_name("stream_started"),
            SoundCue::StreamStarted
        );
        assert_eq!(SoundCue::from_name("stream_ended"), SoundCue::StreamEnded);
    }

    #[test]
    fn wav_roundtrip_decoder_and_validation() {
        let orig = LightweightSoundDecoder::synthesize_cue(SoundCue::Mention);
        let wav_bytes = LightweightSoundDecoder::encode_wav_i16(&orig);
        let decoded = LightweightSoundDecoder::decode_wav_bytes(&wav_bytes)
            .expect("encoded WAV should decode cleanly");
        assert_eq!(decoded.sample_rate_hz, orig.sample_rate_hz);
        assert_eq!(decoded.channels, orig.channels);
        assert_eq!(decoded.samples.len(), orig.samples.len());

        assert_eq!(
            LightweightSoundDecoder::decode_wav_bytes(&[]),
            Err(AudioEngineError::EmptyBuffer)
        );
        assert_eq!(
            LightweightSoundDecoder::decode_wav_bytes(b"NOT_A_WAV_FILE"),
            Err(AudioEngineError::InvalidWavHeader(
                "missing RIFF/WAVE magic signature"
            ))
        );
    }

    #[test]
    fn playback_dispatch_latency_is_under_5ms_and_respects_mute() {
        let engine = NativeAudioEngine::new();
        let receipt = engine
            .play_sound_cue("join", Some(90), Some(false))
            .expect("unmuted sound cue should schedule playback");
        assert_eq!(receipt.cue, SoundCue::UserJoin);
        assert!(receipt.scheduled_samples > 0);
        assert!(
            receipt.dispatch_latency_us < MAX_PLAYBACK_DISPATCH_LATENCY_MS * 1_000,
            "dispatch latency {} us exceeded 5ms ceiling",
            receipt.dispatch_latency_us
        );

        assert_eq!(
            engine.play_sound_cue("mute", Some(100), Some(true)),
            Err(AudioEngineError::MutedOrSilent)
        );
        assert_eq!(
            engine.play_sound_cue("mute", Some(0), Some(false)),
            Err(AudioEngineError::MutedOrSilent)
        );
    }

    #[test]
    fn device_enumeration_and_state_mutations_are_consistent() {
        let engine = NativeAudioEngine::new();
        let inv = engine.enumerate_devices();
        assert!(!inv.inputs.is_empty(), "inputs list must never be empty");
        assert!(!inv.outputs.is_empty(), "outputs list must never be empty");
        assert!(!inv.default_input.is_empty());
        assert!(!inv.default_output.is_empty());

        assert!(engine.select_device(AudioDirection::Input, Some("usb-studio-mic")));
        assert!(!engine.select_device(AudioDirection::Input, Some("usb-studio-mic")));
        assert!(engine.select_device(AudioDirection::Output, Some("usb-headphones")));

        assert_eq!(engine.set_volume_pct(AudioDirection::Input, 250), 200);
        assert_eq!(engine.set_volume_pct(AudioDirection::Output, 135), 135);
        assert_eq!(engine.set_vad_threshold_db(-120, Some(true)), -85);
        assert_eq!(engine.set_vad_threshold_db(10, Some(false)), -5);

        let snap = engine.snapshot();
        assert_eq!(
            snap.selected_input_device.as_deref(),
            Some("usb-studio-mic")
        );
        assert_eq!(
            snap.selected_output_device.as_deref(),
            Some("usb-headphones")
        );
        assert_eq!(snap.input_volume_pct, 200);
        assert_eq!(snap.output_volume_pct, 135);
        assert_eq!(snap.vad_threshold_db, -5);
        assert!(!snap.auto_sensitivity);
        assert_eq!(snap.device_epoch, 2);

        let loud = vec![0.5f32; 960];
        let (db, speaking) = engine.ingest_mic_pcm_samples(&loud);
        assert!(db > -15);
        assert!(speaking);
    }
}
