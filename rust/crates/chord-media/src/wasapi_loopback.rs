#![forbid(unsafe_code)]
//! Windows WASAPI Desktop Audio Loopback Capture Service (`R5-07`).
//!
//! Captures system desktop audio on Windows 10 and Windows 11 via the native WASAPI
//! loopback interface out-of-the-box without requiring third-party virtual audio cables
//! (such as VB-Cable or Virtual Audio Cable), and emits clear, actionable diagnostics
//! on legacy Windows versions (`< Windows 10 build 10240`, e.g., Windows 7 / 8 / 8.1)
//! or when per-process tree exclusion (`build < 20348`) falls back to shared endpoint loopback.
//!
//! Specification & Reference Oracles:
//! - `docs/feature-backlog.md` row `R5-07`:
//!   "Capture system desktop audio via WASAPI loopback interface without requiring third-party
//!   virtual audio cables. Target: Capture Service: Windows WASAPI. Criteria: Audio stream
//!   captures system output on Windows 10/11 out-of-the-box; clear diagnostics on legacy OS versions"
//! - `Serein/crates/discord-voice/src/screen/audio_windows.rs`:
//!   - Process Loopback (`VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK`,
//!     `AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK`,
//!     `PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE`) on Windows 10 21H2+ / Server 2022 /
//!     Windows 11 (`build >= 20348`), excluding the client's own process tree to prevent echo.
//!   - Classic Shared Endpoint Render Loopback (`AUDCLNT_SHAREMODE_SHARED`,
//!     `AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM`)
//!     on Windows 10 builds `10240..20347`, capturing the default `eRender` endpoint out-of-the-box.
//!   - Canonical WebRTC format: `WAVE_FORMAT_IEEE_FLOAT` (`wFormatTag = 3`), 2 channels stereo,
//!     48,000 Hz, 32-bit float (`nBlockAlign = 8`, `nAvgBytesPerSec = 384_000`), sanitized (`NaN`/`±Inf` -> `0.0`,
//!     clamped `[-1.0, 1.0]`) and converted to 20 ms (`960` frames = `1920` stereo samples = `3840` bytes `s16le`)
//!     WebRTC frames.

use std::collections::VecDeque;
use std::fmt;

use crate::pipewire_audio::{
    DEFAULT_GAIN_PERCENT, DEFAULT_MAX_BUFFERED_FRAMES, MAX_GAIN_PERCENT, S16LE_BYTES_PER_SAMPLE,
    WebRtcAudioFrameS16le, f32_to_s16le_sample,
};

/// Canonical channel count for WASAPI loopback WebRTC streaming (`2` stereo channels).
pub const WASAPI_CHANNELS: u16 = 2;
/// Canonical sample rate for WASAPI loopback WebRTC streaming (`48_000` Hz).
pub const WASAPI_SAMPLE_RATE_HZ: u32 = 48_000;
/// Standard WebRTC / Opus audio frame duration in milliseconds (`20` ms).
pub const WASAPI_FRAME_DURATION_MS: u32 = 20;
/// Number of PCM frames (samples per channel) in a 20 ms quantum at 48 kHz (`960` frames).
pub const WASAPI_FRAME_SAMPLES_PER_CHANNEL: usize = 960;
/// Total interleaved stereo samples in a 20 ms quantum at 48 kHz (`960 * 2 = 1920` samples).
pub const WASAPI_FRAME_TOTAL_SAMPLES: usize =
    WASAPI_FRAME_SAMPLES_PER_CHANNEL * (WASAPI_CHANNELS as usize);
/// Exact byte length of a 20 ms stereo 48 kHz `s16le` WebRTC frame (`1920 * 2 = 3840` bytes).
pub const WASAPI_FRAME_BYTES_20MS: usize = WASAPI_FRAME_TOTAL_SAMPLES * S16LE_BYTES_PER_SAMPLE;
/// Maximum number of samples accepted in a single WASAPI capture packet (`48_000 * 2 = 96_000`, 1 second stereo).
pub const MAX_WASAPI_PACKET_SAMPLES: usize = 96_000;

/// Windows `mmreg.h` format tag for 32-bit IEEE floating-point PCM (`WAVE_FORMAT_IEEE_FLOAT = 3`).
pub const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;
/// Windows `Audioclient.h` shared mode constant (`AUDCLNT_SHAREMODE_SHARED = 0`).
pub const AUDCLNT_SHAREMODE_SHARED: u32 = 0;
/// Windows `Audioclient.h` stream flag for render endpoint loopback (`AUDCLNT_STREAMFLAGS_LOOPBACK = 0x00020000`).
pub const AUDCLNT_STREAMFLAGS_LOOPBACK: u32 = 0x0002_0000;
/// Windows `Audioclient.h` stream flag for event-driven buffer readiness (`AUDCLNT_STREAMFLAGS_EVENTCALLBACK = 0x00040000`).
pub const AUDCLNT_STREAMFLAGS_EVENTCALLBACK: u32 = 0x0004_0000;
/// Windows `Audioclient.h` stream flag for automatic kernel mixer sample-rate conversion (`AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM = 0x80000000`).
pub const AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM: u32 = 0x8000_0000;
/// Windows `Audioclient.h` buffer flag indicating a glitch/gap in packet continuity (`AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY = 0x1`).
pub const AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY: u32 = 0x1;
/// Windows `Audioclient.h` buffer flag indicating that the capture packet must be treated as digital silence (`AUDCLNT_BUFFERFLAGS_SILENT = 0x2`).
pub const AUDCLNT_BUFFERFLAGS_SILENT: u32 = 0x2;
/// Windows `Audioclient.h` HRESULT when the audio endpoint is unplugged, disabled, or reconfigured (`AUDCLNT_E_DEVICE_INVALIDATED = 0x88890004`).
pub const AUDCLNT_E_DEVICE_INVALIDATED: u32 = 0x8889_0004;
/// Windows `Audioclient.h` HRESULT when the Windows Audio service (`Audiosrv`) is not running (`AUDCLNT_E_SERVICE_NOT_RUNNING = 0x88890010`).
pub const AUDCLNT_E_SERVICE_NOT_RUNNING: u32 = 0x8889_0010;
/// Windows `mmdeviceapi.h` virtual device activation path for per-process loopback (`VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK`).
pub const VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK: &str = r"VAD\Process_Loopback";

/// Minimum Windows 10 build supporting modern low-latency shared-mode `AUTOCONVERTPCM` WASAPI loopback out-of-the-box (`10240`, Windows 10 RTM 1507).
pub const MIN_WINDOWS_10_LOOPBACK_BUILD: u32 = 10_240;
/// Minimum Windows build supporting `AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK` with `PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE` (`20348`, Windows 10 21H2 / Server 2022 / Windows 11).
pub const MIN_WINDOWS_PROCESS_LOOPBACK_BUILD: u32 = 20_348;

/// Representation of a Windows operating system version (`major.minor.build`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct WindowsOsVersion {
    /// Major NT version (`10` for Windows 10/11, `6` for Vista/7/8/8.1).
    pub major: u32,
    /// Minor NT version (`0` for Windows 10/11, `1` for Win7, `2` for Win8, `3` for Win8.1).
    pub minor: u32,
    /// OS build number (`7601` Win7 SP1, `19045` Win10 22H2, `22631` Win11 23H2).
    pub build: u32,
}

impl WindowsOsVersion {
    /// Creates a new [`WindowsOsVersion`].
    #[must_use]
    pub const fn new(major: u32, minor: u32, build: u32) -> Self {
        Self {
            major,
            minor,
            build,
        }
    }

    /// Canonical Windows 11 23H2 version (`10.0.22631`).
    #[must_use]
    pub const fn windows_11_23h2() -> Self {
        Self::new(10, 0, 22_631)
    }

    /// Canonical Windows 10 22H2 version (`10.0.19045`).
    #[must_use]
    pub const fn windows_10_22h2() -> Self {
        Self::new(10, 0, 19_045)
    }

    /// Returns a human-readable marketing release label for diagnostics.
    #[must_use]
    pub const fn release_label(self) -> &'static str {
        if self.major > 10 || (self.major == 10 && self.build >= 22_000) {
            "Windows 11"
        } else if self.major == 10 && self.build >= MIN_WINDOWS_10_LOOPBACK_BUILD {
            "Windows 10"
        } else if self.major == 10 {
            "Pre-release Windows 10"
        } else if self.major == 6 && self.minor == 3 {
            "Windows 8.1"
        } else if self.major == 6 && self.minor == 2 {
            "Windows 8"
        } else if self.major == 6 && self.minor == 1 {
            "Windows 7"
        } else if self.major == 6 && self.minor == 0 {
            "Windows Vista"
        } else {
            "Legacy Windows"
        }
    }

    /// Returns `true` if this OS supports out-of-the-box WASAPI desktop loopback (`Windows 10 build >= 10240` or `Windows 11`).
    #[must_use]
    pub const fn supports_wasapi_loopback(self) -> bool {
        self.major > 10 || (self.major == 10 && self.build >= MIN_WINDOWS_10_LOOPBACK_BUILD)
    }

    /// Returns `true` if this OS supports `AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK` (`build >= 20348`).
    #[must_use]
    pub const fn supports_process_loopback(self) -> bool {
        self.major > 10 || (self.major == 10 && self.build >= MIN_WINDOWS_PROCESS_LOOPBACK_BUILD)
    }
}

impl fmt::Display for WindowsOsVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {}.{} (build {})",
            self.release_label(),
            self.major,
            self.minor,
            self.build
        )
    }
}

/// WASAPI loopback activation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasapiLoopbackMode {
    /// Modern process-tree exclusion loopback (`VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK` +
    /// `PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE`, Windows build `>= 20348`).
    ProcessLoopbackExcludeTree,
    /// Classic shared render endpoint loopback (`IMMDeviceEnumerator::GetDefaultAudioEndpoint(eRender)` +
    /// `AUDCLNT_STREAMFLAGS_LOOPBACK`, Windows 10 build `>= 10240`).
    SharedEndpointLoopback,
}

/// Compatibility and diagnostic report for WASAPI loopback on a given [`WindowsOsVersion`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasapiCompatibilityReport {
    /// Evaluated Windows OS version.
    pub os_version: WindowsOsVersion,
    /// True if out-of-the-box WASAPI loopback is supported (`Windows 10 build >= 10240` or `Windows 11`).
    pub is_supported: bool,
    /// True if echo-free per-process tree exclusion (`build >= 20348`) is available.
    pub supports_process_loopback: bool,
    /// Always `false` on supported Windows 10/11 builds (no third-party virtual audio cable required).
    pub requires_virtual_audio_cable: bool,
    /// Automatically selected capture mode, or `None` on unsupported legacy OS versions.
    pub recommended_mode: Option<WasapiLoopbackMode>,
    /// Clear, user- and operator-facing diagnostic message explaining the OS capability state.
    pub diagnostic_message: String,
}

/// Evaluates a [`WindowsOsVersion`] against the `R5-07` WASAPI loopback contract and produces
/// clear diagnostics for both supported Windows 10/11 systems and legacy OS versions.
#[must_use]
pub fn evaluate_windows_wasapi_compatibility(
    os_version: WindowsOsVersion,
) -> WasapiCompatibilityReport {
    if os_version.supports_process_loopback() {
        WasapiCompatibilityReport {
            os_version,
            is_supported: true,
            supports_process_loopback: true,
            requires_virtual_audio_cable: false,
            recommended_mode: Some(WasapiLoopbackMode::ProcessLoopbackExcludeTree),
            diagnostic_message: format!(
                "WASAPI process loopback enabled out-of-the-box on {os_version} \
                 (PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE; no third-party virtual audio cable required)."
            ),
        }
    } else if os_version.supports_wasapi_loopback() {
        WasapiCompatibilityReport {
            os_version,
            is_supported: true,
            supports_process_loopback: false,
            requires_virtual_audio_cable: false,
            recommended_mode: Some(WasapiLoopbackMode::SharedEndpointLoopback),
            diagnostic_message: format!(
                "WASAPI shared endpoint loopback enabled out-of-the-box on {os_version} \
                 (no third-party virtual audio cable required; upgrade to Windows build {MIN_WINDOWS_PROCESS_LOOPBACK_BUILD} \
                 or newer for per-process tree echo exclusion)."
            ),
        }
    } else {
        WasapiCompatibilityReport {
            os_version,
            is_supported: false,
            supports_process_loopback: false,
            requires_virtual_audio_cable: false,
            recommended_mode: None,
            diagnostic_message: format!(
                "WASAPI desktop audio loopback is not supported on legacy OS {os_version}: \
                 Windows 10 (build {MIN_WINDOWS_10_LOOPBACK_BUILD} or newer) or Windows 11 is required \
                 for native out-of-the-box system audio capture (Windows build {MIN_WINDOWS_PROCESS_LOOPBACK_BUILD}+ \
                 recommended for echo-free process loopback)."
            ),
        }
    }
}

/// Pure-Rust representation of Windows `WAVEFORMATEX` (`mmreg.h`) used to initialize `IAudioClient`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WasapiWaveFormatEx {
    /// Waveform-audio format type (`WAVE_FORMAT_IEEE_FLOAT = 3`).
    pub w_format_tag: u16,
    /// Number of channels (`2`).
    pub n_channels: u16,
    /// Sample rate in samples per second (`48_000`).
    pub n_samples_per_sec: u32,
    /// Required average data-transfer rate in bytes per second (`48_000 * 8 = 384_000`).
    pub n_avg_bytes_per_sec: u32,
    /// Block alignment in bytes (`n_channels * (w_bits_per_sample / 8) = 8`).
    pub n_block_align: u16,
    /// Bits per sample (`32` for IEEE float).
    pub w_bits_per_sample: u16,
    /// Size in bytes of extra format information (`0`).
    pub cb_size: u16,
}

impl WasapiWaveFormatEx {
    /// Returns the canonical 48 kHz stereo 32-bit IEEE float `WAVEFORMATEX` matching Serein's `audio_windows.rs`.
    #[must_use]
    pub const fn ieee_float_stereo_48khz() -> Self {
        let channels = WASAPI_CHANNELS;
        let bits_per_sample: u16 = 32;
        let block_align = channels * (bits_per_sample / 8);
        let avg_bytes_per_sec = WASAPI_SAMPLE_RATE_HZ * (block_align as u32);
        Self {
            w_format_tag: WAVE_FORMAT_IEEE_FLOAT,
            n_channels: channels,
            n_samples_per_sec: WASAPI_SAMPLE_RATE_HZ,
            n_avg_bytes_per_sec: avg_bytes_per_sec,
            n_block_align: block_align,
            w_bits_per_sample: bits_per_sample,
            cb_size: 0,
        }
    }
}

/// Metadata describing a Windows `IMMDevice` render audio endpoint (`eRender`).
///
/// Redacts `friendly_name` in `Debug` output to comply with `TEST-POLICY.md` log privacy rules.
#[derive(Clone, PartialEq, Eq)]
pub struct WasapiAudioEndpoint {
    /// Windows MMDevice endpoint ID string (e.g. `"{0.0.0.00000000}.{guid}"`).
    pub endpoint_id: String,
    /// Human-readable device friendly name (`PKEY_Device_FriendlyName`).
    pub friendly_name: String,
    /// Whether this endpoint is the default system render output (`eConsole` / `eMultimedia`).
    pub is_default_render: bool,
    /// Whether the endpoint appears to be a third-party virtual cable (`VB-Audio`, `Virtual Audio Cable`).
    pub is_virtual_cable: bool,
    /// Native channel count (`2`).
    pub channels: u16,
    /// Native sample rate in Hz (`48_000`).
    pub sample_rate_hz: u32,
}

impl fmt::Debug for WasapiAudioEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WasapiAudioEndpoint")
            .field("endpoint_id", &self.endpoint_id)
            .field("friendly_name", &"<redacted>")
            .field("is_default_render", &self.is_default_render)
            .field("is_virtual_cable", &self.is_virtual_cable)
            .field("channels", &self.channels)
            .field("sample_rate_hz", &self.sample_rate_hz)
            .finish()
    }
}

/// Parameters used to activate `IAudioClient` for WASAPI loopback capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasapiActivationParams {
    /// Selected loopback mode (`ProcessLoopbackExcludeTree` or `SharedEndpointLoopback`).
    pub mode: WasapiLoopbackMode,
    /// Device activation path (`"VAD\\Process_Loopback"` or the `IMMDevice` endpoint ID).
    pub activation_device_id: String,
    /// Target process ID excluded from capture when `mode == ProcessLoopbackExcludeTree`.
    pub target_process_id: Option<u32>,
}

/// A single raw capture packet read from `IAudioCaptureClient::GetBuffer`.
#[derive(Debug, Clone, PartialEq)]
pub struct WasapiCapturePacket {
    /// Number of stereo audio frames in this packet (`1` frame = `2` interleaved `f32` samples).
    pub frames: u32,
    /// Bitmask of `AUDCLNT_BUFFERFLAGS_*` (`AUDCLNT_BUFFERFLAGS_SILENT`, `AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY`).
    pub flags: u32,
    /// Interleaved 32-bit IEEE floating-point stereo samples (`frames * 2` elements).
    pub samples_f32: Vec<f32>,
}

/// Lifecycle state of the Windows WASAPI loopback capture service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasapiStreamState {
    /// Service created, `IAudioClient` not yet activated.
    Uninitialized,
    /// `IAudioClient` and `IAudioCaptureClient` activated and capturing desktop audio.
    Capturing,
    /// Capture temporarily paused; incoming packets are discarded.
    Paused,
    /// Capture stopped and COM resources released.
    Stopped,
}

/// Configuration for [`WasapiLoopbackService`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasapiLoopbackConfig {
    /// Client process ID to exclude in `ProcessLoopbackExcludeTree` mode (defaults to `std::process::id()`).
    pub target_process_id: u32,
    /// Preferred loopback mode (automatically degrades to `SharedEndpointLoopback` on Windows 10 `< 20348`).
    pub preferred_mode: WasapiLoopbackMode,
    /// Optional explicit render endpoint ID for `SharedEndpointLoopback` (`None` = system default render device).
    pub explicit_endpoint_id: Option<String>,
    /// Number of channels (`2`).
    pub channels: u16,
    /// Sample rate in Hz (`48_000`).
    pub sample_rate_hz: u32,
    /// Maximum number of 20 ms frames held in the PCM ring buffer (`25` frames = `500` ms).
    pub max_buffered_frames: usize,
    /// Initial capture gain percentage (`0..=200`, default `100`).
    pub gain_percent: u16,
}

impl Default for WasapiLoopbackConfig {
    fn default() -> Self {
        Self {
            target_process_id: std::process::id().max(1),
            preferred_mode: WasapiLoopbackMode::ProcessLoopbackExcludeTree,
            explicit_endpoint_id: None,
            channels: WASAPI_CHANNELS,
            sample_rate_hz: WASAPI_SAMPLE_RATE_HZ,
            max_buffered_frames: DEFAULT_MAX_BUFFERED_FRAMES,
            gain_percent: DEFAULT_GAIN_PERCENT,
        }
    }
}

impl WasapiLoopbackConfig {
    /// Validates the WASAPI loopback configuration against the `R5-07` contract.
    pub fn validate(&self) -> Result<(), WasapiLoopbackError> {
        if self.channels != WASAPI_CHANNELS {
            return Err(WasapiLoopbackError::UnsupportedChannels(self.channels));
        }
        if self.sample_rate_hz != WASAPI_SAMPLE_RATE_HZ {
            return Err(WasapiLoopbackError::UnsupportedSampleRate(
                self.sample_rate_hz,
            ));
        }
        if self.preferred_mode == WasapiLoopbackMode::ProcessLoopbackExcludeTree
            && self.target_process_id == 0
        {
            return Err(WasapiLoopbackError::InvalidProcessId(0));
        }
        if self.max_buffered_frames == 0 {
            return Err(WasapiLoopbackError::InvalidBufferCapacity(0));
        }
        Ok(())
    }
}

/// Errors produced by the Windows WASAPI loopback capture service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WasapiLoopbackError {
    /// Current Windows OS version is older than Windows 10 build 10240 (e.g. Windows 7 / 8 / 8.1).
    LegacyOsUnsupported {
        /// Detected legacy OS version.
        version: WindowsOsVersion,
        /// Clear diagnostic message explaining minimum OS build requirements.
        diagnostic: String,
    },
    /// Channel count is not stereo (`2`).
    UnsupportedChannels(u16),
    /// Sample rate is not `48_000` Hz.
    UnsupportedSampleRate(u32),
    /// Target process ID for process loopback exclusion cannot be `0`.
    InvalidProcessId(u32),
    /// Ring buffer capacity in frames must be `> 0`.
    InvalidBufferCapacity(usize),
    /// No active Windows render audio endpoint (`eRender`) is available.
    NoRenderEndpointAvailable,
    /// Provided `f32` sample slice length is odd (not stereo-aligned) or exceeds [`MAX_WASAPI_PACKET_SAMPLES`].
    UnalignedStereoSamples(usize),
    /// Active render endpoint was invalidated (`AUDCLNT_E_DEVICE_INVALIDATED = 0x88890004`).
    DeviceInvalidated(String),
    /// Windows Audio service (`Audiosrv`) is not running (`AUDCLNT_E_SERVICE_NOT_RUNNING = 0x88890010`).
    AudioServiceNotRunning(String),
    /// Operation is invalid in the current [`WasapiStreamState`].
    InvalidState {
        /// Expected state description.
        expected: &'static str,
        /// Actual state.
        actual: WasapiStreamState,
    },
}

impl fmt::Display for WasapiLoopbackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LegacyOsUnsupported { diagnostic, .. } => write!(f, "{diagnostic}"),
            Self::UnsupportedChannels(ch) => write!(
                f,
                "unsupported WASAPI channel count {ch} (expected {WASAPI_CHANNELS} stereo channels)"
            ),
            Self::UnsupportedSampleRate(rate) => write!(
                f,
                "unsupported WASAPI sample rate {rate} Hz (expected {WASAPI_SAMPLE_RATE_HZ} Hz)"
            ),
            Self::InvalidProcessId(pid) => write!(
                f,
                "invalid target process ID {pid} for WASAPI process loopback exclusion"
            ),
            Self::InvalidBufferCapacity(cap) => {
                write!(f, "invalid WASAPI ring buffer capacity: {cap} frames")
            }
            Self::NoRenderEndpointAvailable => write!(
                f,
                "no active Windows WASAPI render audio endpoint (eRender) available for loopback capture"
            ),
            Self::UnalignedStereoSamples(len) => write!(
                f,
                "unaligned or oversized WASAPI stereo f32 sample buffer ({len} samples)"
            ),
            Self::DeviceInvalidated(msg) => {
                write!(f, "WASAPI audio endpoint invalidated: {msg}")
            }
            Self::AudioServiceNotRunning(msg) => {
                write!(f, "Windows Audio service is not running: {msg}")
            }
            Self::InvalidState { expected, actual } => write!(
                f,
                "invalid WASAPI loopback stream state: expected {expected}, actual {actual:?}"
            ),
        }
    }
}

impl std::error::Error for WasapiLoopbackError {}

/// Telemetry counters for the Windows WASAPI loopback capture service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WasapiLoopbackTelemetry {
    /// Total 20 ms WebRTC frames drained.
    pub captured_frames: u64,
    /// Total `s16le` bytes drained across all WebRTC frames.
    pub captured_bytes: u64,
    /// Total WASAPI packets marked with `AUDCLNT_BUFFERFLAGS_SILENT`.
    pub silent_packets: u64,
    /// Total WASAPI packets marked with `AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY`.
    pub discontinuity_packets: u64,
    /// Number of non-finite (`NaN` or `±Infinity`) `f32` samples sanitized to `0.0`.
    pub sanitized_non_finite_samples: u64,
    /// Number of oldest `i16` samples evicted due to ring buffer overflow.
    pub overrun_samples: u64,
    /// Number of silence fallback frames synthesized when polling an empty buffer.
    pub underrun_reads: u64,
    /// Number of successful recoveries after `AUDCLNT_E_DEVICE_INVALIDATED` / default device switch.
    pub device_recoveries: u64,
    /// Number of capture client recreations across DAVE E2EE audio epoch transitions.
    pub epoch_rotations: u64,
}

/// Abstraction over the Windows WASAPI COM backend (`IAudioClient` / `IAudioCaptureClient` or test double).
pub trait WasapiLoopbackBackend {
    /// Queries the host Windows OS version (`RtlGetVersion` / platform oracle).
    fn query_os_version(&self) -> WindowsOsVersion;
    /// Enumerates active `eRender` audio output endpoints via `IMMDeviceEnumerator`.
    fn enumerate_render_endpoints(
        &mut self,
    ) -> Result<Vec<WasapiAudioEndpoint>, WasapiLoopbackError>;
    /// Activates and starts `IAudioClient` in shared loopback mode (`AUDCLNT_SHAREMODE_SHARED`)
    /// with the specified `WAVEFORMATEX` and `AUDCLNT_STREAMFLAGS_*`.
    fn activate_loopback_client(
        &mut self,
        params: &WasapiActivationParams,
        format: &WasapiWaveFormatEx,
        stream_flags: u32,
    ) -> Result<(), WasapiLoopbackError>;
    /// Polls the next available capture packet from `IAudioCaptureClient::GetBuffer` / `ReleaseBuffer`.
    fn poll_next_packet(&mut self) -> Result<Option<WasapiCapturePacket>, WasapiLoopbackError>;
    /// Stops `IAudioClient::Stop()` and releases the active capture client.
    fn stop_loopback_client(&mut self) -> Result<(), WasapiLoopbackError>;
}

/// Sanitizes a raw 32-bit IEEE float WASAPI sample:
/// - Non-finite values (`NaN`, `+Infinity`, `-Infinity`) become `(0.0, true)` (matching Serein's `audio_windows.rs`).
/// - Finite values are clamped to `[-1.0, 1.0]` with `(clamped, false)`.
#[must_use]
pub fn sanitize_wasapi_f32_sample(sample: f32) -> (f32, bool) {
    if sample.is_finite() {
        (sample.clamp(-1.0, 1.0), false)
    } else {
        (0.0, true)
    }
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

/// Windows WASAPI Desktop Audio Loopback Capture Service (`R5-07`).
pub struct WasapiLoopbackService<B: WasapiLoopbackBackend> {
    backend: B,
    config: WasapiLoopbackConfig,
    state: WasapiStreamState,
    active_activation: Option<WasapiActivationParams>,
    pcm_ring: VecDeque<i16>,
    gain_percent: u16,
    audio_epoch: u64,
    next_sequence: u64,
    next_timestamp_samples: u64,
    telemetry: WasapiLoopbackTelemetry,
}

impl<B: WasapiLoopbackBackend> WasapiLoopbackService<B> {
    /// Creates a new WASAPI loopback capture service after validating `config`.
    pub fn new(backend: B, config: WasapiLoopbackConfig) -> Result<Self, WasapiLoopbackError> {
        config.validate()?;
        let capacity_samples = config
            .max_buffered_frames
            .saturating_mul(WASAPI_FRAME_TOTAL_SAMPLES);
        let gain_percent = config.gain_percent.min(MAX_GAIN_PERCENT);
        Ok(Self {
            backend,
            config,
            state: WasapiStreamState::Uninitialized,
            active_activation: None,
            pcm_ring: VecDeque::with_capacity(capacity_samples),
            gain_percent,
            audio_epoch: 1,
            next_sequence: 1,
            next_timestamp_samples: 0,
            telemetry: WasapiLoopbackTelemetry::default(),
        })
    }

    /// Returns the current lifecycle state of the WASAPI loopback stream.
    #[must_use]
    pub const fn state(&self) -> WasapiStreamState {
        self.state
    }

    /// Returns the active [`WasapiActivationParams`], if capture is active.
    #[must_use]
    pub const fn active_activation(&self) -> Option<&WasapiActivationParams> {
        self.active_activation.as_ref()
    }

    /// Returns the current DAVE E2EE audio epoch generation counter.
    #[must_use]
    pub const fn audio_epoch(&self) -> u64 {
        self.audio_epoch
    }

    /// Returns the number of `i16` samples currently buffered in the ring buffer.
    #[must_use]
    pub fn buffered_samples(&self) -> usize {
        self.pcm_ring.len()
    }

    /// Returns the current capture gain percentage (`0..=200`).
    #[must_use]
    pub const fn gain_percent(&self) -> u16 {
        self.gain_percent
    }

    /// Updates the capture gain percentage, clamped to `0..=200`.
    pub fn set_gain_percent(&mut self, gain_percent: u16) {
        self.gain_percent = gain_percent.min(MAX_GAIN_PERCENT);
    }

    /// Returns a snapshot of service telemetry counters.
    #[must_use]
    pub const fn telemetry(&self) -> WasapiLoopbackTelemetry {
        self.telemetry
    }

    /// Evaluates the host Windows OS version and returns a [`WasapiCompatibilityReport`]
    /// with clear diagnostics for Windows 10/11 and legacy OS builds.
    #[must_use]
    pub fn check_os_compatibility(&self) -> WasapiCompatibilityReport {
        evaluate_windows_wasapi_compatibility(self.backend.query_os_version())
    }

    /// Enumerates available Windows render endpoints (`eRender`).
    pub fn enumerate_endpoints(&mut self) -> Result<Vec<WasapiAudioEndpoint>, WasapiLoopbackError> {
        self.backend.enumerate_render_endpoints()
    }

    /// Resolves the [`WasapiActivationParams`] for the current OS version and configuration.
    fn resolve_activation_params(
        &mut self,
        supports_process_loopback: bool,
    ) -> Result<WasapiActivationParams, WasapiLoopbackError> {
        if self.config.preferred_mode == WasapiLoopbackMode::ProcessLoopbackExcludeTree
            && supports_process_loopback
        {
            return Ok(WasapiActivationParams {
                mode: WasapiLoopbackMode::ProcessLoopbackExcludeTree,
                activation_device_id: VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK.to_string(),
                target_process_id: Some(self.config.target_process_id),
            });
        }
        let endpoint_id = if let Some(explicit) = &self.config.explicit_endpoint_id {
            explicit.clone()
        } else {
            let endpoints = self.backend.enumerate_render_endpoints()?;
            endpoints
                .iter()
                .find(|e| e.is_default_render && !e.is_virtual_cable)
                .or_else(|| endpoints.iter().find(|e| e.is_default_render))
                .or_else(|| endpoints.first())
                .ok_or(WasapiLoopbackError::NoRenderEndpointAvailable)?
                .endpoint_id
                .clone()
        };
        Ok(WasapiActivationParams {
            mode: WasapiLoopbackMode::SharedEndpointLoopback,
            activation_device_id: endpoint_id,
            target_process_id: None,
        })
    }

    /// Activates and starts WASAPI desktop audio loopback capture out-of-the-box without virtual audio cables.
    pub fn start_capture(&mut self) -> Result<WasapiActivationParams, WasapiLoopbackError> {
        let compat = self.check_os_compatibility();
        if !compat.is_supported {
            return Err(WasapiLoopbackError::LegacyOsUnsupported {
                version: compat.os_version,
                diagnostic: compat.diagnostic_message,
            });
        }
        let activation = self.resolve_activation_params(compat.supports_process_loopback)?;
        let format = WasapiWaveFormatEx::ieee_float_stereo_48khz();
        let stream_flags = AUDCLNT_STREAMFLAGS_LOOPBACK
            | AUDCLNT_STREAMFLAGS_EVENTCALLBACK
            | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM;

        self.backend
            .activate_loopback_client(&activation, &format, stream_flags)?;
        self.active_activation = Some(activation.clone());
        self.state = WasapiStreamState::Capturing;
        Ok(activation)
    }

    /// Polls up to 32 pending packets from `IAudioCaptureClient`, sanitizing `f32` samples
    /// and honoring `AUDCLNT_BUFFERFLAGS_SILENT`.
    pub fn pump_backend_packets(&mut self) -> Result<usize, WasapiLoopbackError> {
        if !matches!(
            self.state,
            WasapiStreamState::Capturing | WasapiStreamState::Paused
        ) {
            return Err(WasapiLoopbackError::InvalidState {
                expected: "Capturing or Paused",
                actual: self.state,
            });
        }

        let mut total_frames = 0usize;
        for _ in 0..32 {
            let Some(packet) = self.backend.poll_next_packet()? else {
                break;
            };
            if (packet.flags & AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY) != 0 {
                self.telemetry.discontinuity_packets =
                    self.telemetry.discontinuity_packets.saturating_add(1);
            }
            let sample_count = (packet.frames as usize).saturating_mul(WASAPI_CHANNELS as usize);
            if sample_count == 0 {
                continue;
            }
            if sample_count > MAX_WASAPI_PACKET_SAMPLES {
                return Err(WasapiLoopbackError::UnalignedStereoSamples(sample_count));
            }

            if (packet.flags & AUDCLNT_BUFFERFLAGS_SILENT) != 0 {
                self.telemetry.silent_packets = self.telemetry.silent_packets.saturating_add(1);
                let zeros = vec![0.0f32; sample_count];
                self.ingest_f32_samples(&zeros)?;
            } else {
                self.ingest_f32_samples(&packet.samples_f32)?;
            }
            total_frames = total_frames.saturating_add(packet.frames as usize);
        }
        Ok(total_frames)
    }

    /// Ingests interleaved stereo 32-bit IEEE float samples at the current `audio_epoch`.
    pub fn ingest_f32_samples(&mut self, samples: &[f32]) -> Result<usize, WasapiLoopbackError> {
        let epoch = self.audio_epoch;
        self.ingest_f32_samples_at_epoch(samples, epoch)
    }

    /// Ingests interleaved stereo 32-bit IEEE float samples tagged with `epoch`.
    pub fn ingest_f32_samples_at_epoch(
        &mut self,
        samples: &[f32],
        epoch: u64,
    ) -> Result<usize, WasapiLoopbackError> {
        if self.state == WasapiStreamState::Paused || epoch != self.audio_epoch {
            return Ok(0);
        }
        if self.state != WasapiStreamState::Capturing {
            return Err(WasapiLoopbackError::InvalidState {
                expected: "Capturing",
                actual: self.state,
            });
        }
        if samples.len() > MAX_WASAPI_PACKET_SAMPLES
            || !samples.len().is_multiple_of(WASAPI_CHANNELS as usize)
        {
            return Err(WasapiLoopbackError::UnalignedStereoSamples(samples.len()));
        }

        let max_samples = self
            .config
            .max_buffered_frames
            .saturating_mul(WASAPI_FRAME_TOTAL_SAMPLES);
        let gain = self.gain_percent;

        for &raw in samples {
            let (sanitized, was_non_finite) = sanitize_wasapi_f32_sample(raw);
            if was_non_finite {
                self.telemetry.sanitized_non_finite_samples = self
                    .telemetry
                    .sanitized_non_finite_samples
                    .saturating_add(1);
            }
            let s16 = scale_s16le_with_gain(f32_to_s16le_sample(sanitized), gain);
            if self.pcm_ring.len() >= max_samples {
                self.pcm_ring.pop_front();
                self.telemetry.overrun_samples = self.telemetry.overrun_samples.saturating_add(1);
            }
            self.pcm_ring.push_back(s16);
        }

        Ok(samples.len())
    }

    /// Rotates the DAVE E2EE audio epoch generation counter by stopping and recreating the
    /// WASAPI loopback client while purging any buffered samples from the old epoch.
    pub fn rotate_audio_epoch(
        &mut self,
        new_epoch: u64,
    ) -> Result<WasapiActivationParams, WasapiLoopbackError> {
        self.pcm_ring.clear();
        self.audio_epoch = new_epoch;
        let _ = self.backend.stop_loopback_client();
        self.active_activation = None;
        self.telemetry.epoch_rotations = self.telemetry.epoch_rotations.saturating_add(1);
        self.start_capture()
    }

    /// Recovers the WASAPI loopback stream after `AUDCLNT_E_DEVICE_INVALIDATED` or a default
    /// output endpoint change.
    pub fn recover_after_device_change(
        &mut self,
    ) -> Result<WasapiActivationParams, WasapiLoopbackError> {
        let _ = self.backend.stop_loopback_client();
        self.active_activation = None;
        let activation = self.start_capture()?;
        self.telemetry.device_recoveries = self.telemetry.device_recoveries.saturating_add(1);
        Ok(activation)
    }

    /// Pauses loopback audio capture and clears partial buffered samples.
    pub fn pause(&mut self) -> Result<(), WasapiLoopbackError> {
        if self.state != WasapiStreamState::Capturing {
            return Err(WasapiLoopbackError::InvalidState {
                expected: "Capturing",
                actual: self.state,
            });
        }
        self.pcm_ring.clear();
        self.state = WasapiStreamState::Paused;
        Ok(())
    }

    /// Resumes loopback audio capture from [`WasapiStreamState::Paused`].
    pub fn resume(&mut self) -> Result<(), WasapiLoopbackError> {
        if self.state != WasapiStreamState::Paused {
            return Err(WasapiLoopbackError::InvalidState {
                expected: "Paused",
                actual: self.state,
            });
        }
        self.state = WasapiStreamState::Capturing;
        Ok(())
    }

    /// Drains all complete 20 ms stereo 48 kHz `s16le` WebRTC frames (`1920` samples = `3840` bytes each).
    #[must_use]
    pub fn drain_webrtc_frames(&mut self) -> Vec<WebRtcAudioFrameS16le> {
        let mut frames = Vec::new();
        while self.pcm_ring.len() >= WASAPI_FRAME_TOTAL_SAMPLES {
            frames.push(self.pop_single_frame());
        }
        frames
    }

    /// Pops the next 20 ms `s16le` WebRTC frame, or synthesizes a zero-filled 20 ms silence frame
    /// if fewer than `1920` stereo samples are currently buffered.
    #[must_use]
    pub fn poll_next_webrtc_frame_or_silence(&mut self) -> WebRtcAudioFrameS16le {
        if self.pcm_ring.len() >= WASAPI_FRAME_TOTAL_SAMPLES {
            self.pop_single_frame()
        } else {
            self.telemetry.underrun_reads = self.telemetry.underrun_reads.saturating_add(1);
            let seq = self.next_sequence;
            self.next_sequence = self.next_sequence.saturating_add(1);
            let ts = self.next_timestamp_samples;
            self.next_timestamp_samples = self
                .next_timestamp_samples
                .saturating_add(WASAPI_FRAME_SAMPLES_PER_CHANNEL as u64);
            WebRtcAudioFrameS16le {
                sequence: seq,
                timestamp_samples: ts,
                channels: self.config.channels,
                sample_rate_hz: self.config.sample_rate_hz,
                samples: vec![0i16; WASAPI_FRAME_TOTAL_SAMPLES],
                is_silence: true,
            }
        }
    }

    fn pop_single_frame(&mut self) -> WebRtcAudioFrameS16le {
        let mut samples = Vec::with_capacity(WASAPI_FRAME_TOTAL_SAMPLES);
        let mut is_silence = true;
        for _ in 0..WASAPI_FRAME_TOTAL_SAMPLES {
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
            .saturating_add(WASAPI_FRAME_SAMPLES_PER_CHANNEL as u64);

        self.telemetry.captured_frames = self.telemetry.captured_frames.saturating_add(1);
        self.telemetry.captured_bytes = self
            .telemetry
            .captured_bytes
            .saturating_add(WASAPI_FRAME_BYTES_20MS as u64);

        WebRtcAudioFrameS16le {
            sequence: seq,
            timestamp_samples: ts,
            channels: self.config.channels,
            sample_rate_hz: self.config.sample_rate_hz,
            samples,
            is_silence,
        }
    }

    /// Cleanly stops the WASAPI loopback capture client and clears buffered audio.
    pub fn stop(&mut self) -> Result<(), WasapiLoopbackError> {
        if self.state == WasapiStreamState::Stopped {
            return Ok(());
        }
        self.pcm_ring.clear();
        self.active_activation = None;
        let res = self.backend.stop_loopback_client();
        self.state = WasapiStreamState::Stopped;
        res
    }
}

impl<B: WasapiLoopbackBackend> Drop for WasapiLoopbackService<B> {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
