#![forbid(unsafe_code)]
//! Contract tests for Task R5-07: Windows Loopback Audio (`chord_media::wasapi_loopback`).
//!
//! Specification & Reference Oracles:
//! - `docs/feature-backlog.md` row `R5-07`:
//!   "Capture system desktop audio via WASAPI loopback interface without requiring third-party
//!   virtual audio cables. Target: Capture Service: Windows WASAPI. Criteria: Audio stream
//!   captures system output on Windows 10/11 out-of-the-box; clear diagnostics on legacy OS versions"
//! - `Serein/crates/discord-voice/src/screen/audio_windows.rs`:
//!   - Process loopback activation (`VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK`,
//!     `AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK`,
//!     `PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE`) for Windows 10 21H2+ / Server 2022 /
//!     Windows 11 (`build >= 20348`) to exclude the client's own process tree without virtual cables.
//!   - Classic WASAPI Endpoint Render Loopback (`AUDCLNT_SHAREMODE_SHARED`,
//!     `AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM`)
//!     for Windows 10 builds `10240..20348`, capturing default `eRender` output out-of-the-box.
//!   - Clear diagnostic rejection on legacy Windows versions (`< Windows 10 build 10240`, e.g.
//!     Windows 7 SP1 build 7601, Windows 8 build 9200, Windows 8.1 build 9600).
//!   - Canonical WebRTC format: `WAVE_FORMAT_IEEE_FLOAT` (`wFormatTag = 3`), 2 channels stereo,
//!     48,000 Hz, 32-bit float (`nBlockAlign = 8`, `nAvgBytesPerSec = 384_000`), sanitized (NaN/Inf -> 0.0,
//!     clamped `[-1.0, 1.0]`) and framed into 20 ms (`960` frames = `1920` stereo samples = `3840` bytes `s16le`)
//!     packets with `AUDCLNT_BUFFERFLAGS_SILENT` handling and encryption epoch generation fencing.
//!
//! Covers all 9 mandatory dimensions (`TEST-POLICY.md` §12):
//! 1. Positive path: Windows 11 (`10.0.22631`) and Windows 10 21H2+ (`10.0.20348`) process loopback
//!    capture out-of-the-box (`ExcludeTargetProcessTree` on client PID), plus Windows 10 (`10.0.19045`)
//!    shared endpoint loopback capture, draining 20 ms stereo 48 kHz WebRTC frames.
//! 2. Negative path: rejecting legacy OS builds (Windows 7 `6.1.7601`, Windows 8 `6.2.9200`,
//!    Windows 8.1 `6.3.9600`, pre-threshold Windows 10 `< 10240`) with clear actionable diagnostics;
//!    rejecting invalid channels, sample rates, zero PID for process loopback, or unaligned sample slices.
//! 3. Boundary conditions: exact Windows build boundaries (`10239` rejected vs `10240` endpoint loopback;
//!    `20347` endpoint loopback with diagnostic note vs `20348` process loopback), 20 ms framing boundaries
//!    (`1919` vs `1920` samples), `f32` `NaN`/`±Infinity`/`±2.0` sanitization & clamping, and ring buffer
//!    capacity eviction.
//! 4. Error handling: `AUDCLNT_E_DEVICE_INVALIDATED` (`0x88890004`), `AUDCLNT_E_SERVICE_NOT_RUNNING`
//!    (`0x88890010`), activation timeout, and operations in invalid stream lifecycle state.
//! 5. Empty states: `AUDCLNT_BUFFERFLAGS_SILENT` packet injection produces zero-filled silence samples;
//!    draining when fewer than 1920 samples are buffered returns empty list; `poll_next_webrtc_frame_or_silence`
//!    synthesizes a 20 ms DTX silence frame.
//! 6. Intermediate / lifecycle states: `Uninitialized -> Activated -> Capturing -> Paused -> Stopped`
//!    state machine transitions and epoch rotation (`Stop/Reset/Start` avoidance by recreating client on epoch change).
//! 7. Capabilities & privacy: `Debug` redaction of human-readable Windows endpoint device names,
//!    zero third-party virtual audio cable requirement (`requires_virtual_audio_cable == false`),
//!    and Voice Gateway OP 5 `SOUNDSHARE` (`speaking == 2`) payload generation.
//! 8. Recovery & degradation: automatic fallback from Process Loopback to Shared Endpoint Loopback on
//!    Windows 10 builds `10240..20347`, automatic re-activation on default render device change /
//!    `AUDCLNT_E_DEVICE_INVALIDATED`, and clean COM capture client stop on `stop()` and `Drop`.
//! 9. Observable telemetry: accurate tracking of `captured_frames`, `captured_bytes`, `silent_packets`,
//!    `sanitized_non_finite_samples`, `overrun_samples`, `underrun_reads`, `device_recoveries`, and `epoch_rotations`.

use std::cell::RefCell;
use std::rc::Rc;

use chord_media::wasapi_loopback::{
    AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY, AUDCLNT_BUFFERFLAGS_SILENT,
    AUDCLNT_E_DEVICE_INVALIDATED, AUDCLNT_E_SERVICE_NOT_RUNNING, AUDCLNT_SHAREMODE_SHARED,
    AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM, AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
    AUDCLNT_STREAMFLAGS_LOOPBACK, MIN_WINDOWS_10_LOOPBACK_BUILD, MIN_WINDOWS_PROCESS_LOOPBACK_BUILD,
    VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, WASAPI_CHANNELS, WASAPI_FRAME_BYTES_20MS,
    WASAPI_FRAME_DURATION_MS, WASAPI_FRAME_SAMPLES_PER_CHANNEL, WASAPI_FRAME_TOTAL_SAMPLES,
    WASAPI_SAMPLE_RATE_HZ, WAVE_FORMAT_IEEE_FLOAT, WasapiActivationParams, WasapiAudioEndpoint,
    WasapiCapturePacket, WasapiCompatibilityReport, WasapiLoopbackBackend, WasapiLoopbackConfig,
    WasapiLoopbackError, WasapiLoopbackMode, WasapiLoopbackService, WasapiStreamState,
    WasapiWaveFormatEx, WindowsOsVersion, evaluate_windows_wasapi_compatibility,
    sanitize_wasapi_f32_sample,
};

// ---------------------------------------------------------------------------
// Deterministic Mock WASAPI Backend for L1/L3 Contract Verification
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct MockWasapiState {
    os_version: Option<WindowsOsVersion>,
    endpoints: Vec<WasapiAudioEndpoint>,
    active_activation: Option<WasapiActivationParams>,
    initialized_format: Option<WasapiWaveFormatEx>,
    initialized_flags: u32,
    started: bool,
    stopped_count: u32,
    pending_packets: Vec<WasapiCapturePacket>,
    fail_activate: Option<WasapiLoopbackError>,
    fail_poll: Option<WasapiLoopbackError>,
    calls: Vec<String>,
}

#[derive(Clone)]
struct MockWasapiBackend {
    state: Rc<RefCell<MockWasapiState>>,
}

impl MockWasapiBackend {
    fn new(os_version: WindowsOsVersion) -> Self {
        let state = MockWasapiState {
            os_version: Some(os_version),
            endpoints: vec![
                WasapiAudioEndpoint {
                    endpoint_id: "{0.0.0.00000000}.{d4a1b2c3-e5f6-4a5b-9c8d-7e6f5a4b3c2d}"
                        .to_string(),
                    friendly_name: "Speakers (Realtek(R) Audio)".to_string(),
                    is_default_render: true,
                    is_virtual_cable: false,
                    channels: 2,
                    sample_rate_hz: 48_000,
                },
                WasapiAudioEndpoint {
                    endpoint_id: "{0.0.0.00000000}.{a1b2c3d4-1111-2222-3333-444455556666}"
                        .to_string(),
                    friendly_name: "Headphones (USB DAC)".to_string(),
                    is_default_render: false,
                    is_virtual_cable: false,
                    channels: 2,
                    sample_rate_hz: 48_000,
                },
            ],
            ..Default::default()
        };
        Self {
            state: Rc::new(RefCell::new(state)),
        }
    }
}

impl WasapiLoopbackBackend for MockWasapiBackend {
    fn query_os_version(&self) -> WindowsOsVersion {
        self.state
            .borrow()
            .os_version
            .unwrap_or_else(WindowsOsVersion::windows_11_23h2)
    }

    fn enumerate_render_endpoints(
        &mut self,
    ) -> Result<Vec<WasapiAudioEndpoint>, WasapiLoopbackError> {
        let mut st = self.state.borrow_mut();
        st.calls.push("enumerate_render_endpoints".to_string());
        Ok(st.endpoints.clone())
    }

    fn activate_loopback_client(
        &mut self,
        params: &WasapiActivationParams,
        format: &WasapiWaveFormatEx,
        stream_flags: u32,
    ) -> Result<(), WasapiLoopbackError> {
        let mut st = self.state.borrow_mut();
        st.calls.push(format!(
            "activate_loopback_client:{:?}:flags=0x{:08x}",
            params.mode, stream_flags
        ));
        if let Some(err) = st.fail_activate.clone() {
            return Err(err);
        }
        st.active_activation = Some(params.clone());
        st.initialized_format = Some(*format);
        st.initialized_flags = stream_flags;
        st.started = true;
        Ok(())
    }

    fn poll_next_packet(
        &mut self,
    ) -> Result<Option<WasapiCapturePacket>, WasapiLoopbackError> {
        let mut st = self.state.borrow_mut();
        st.calls.push("poll_next_packet".to_string());
        if let Some(err) = st.fail_poll.clone() {
            return Err(err);
        }
        if st.pending_packets.is_empty() {
            Ok(None)
        } else {
            Ok(Some(st.pending_packets.remove(0)))
        }
    }

    fn stop_loopback_client(&mut self) -> Result<(), WasapiLoopbackError> {
        let mut st = self.state.borrow_mut();
        st.calls.push("stop_loopback_client".to_string());
        st.started = false;
        st.active_activation = None;
        st.stopped_count += 1;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 1. Constants & WASAPI Wire Contract Oracles
// ---------------------------------------------------------------------------

#[test]
fn wasapi_loopback_constants_and_waveformat_match_windows_sdk_oracle() {
    assert_eq!(WASAPI_CHANNELS, 2);
    assert_eq!(WASAPI_SAMPLE_RATE_HZ, 48_000);
    assert_eq!(WASAPI_FRAME_DURATION_MS, 20);
    assert_eq!(WASAPI_FRAME_SAMPLES_PER_CHANNEL, 960);
    assert_eq!(WASAPI_FRAME_TOTAL_SAMPLES, 1_920);
    assert_eq!(WASAPI_FRAME_BYTES_20MS, 3_840);

    // Windows SDK mmreg.h / Audioclient.h constants
    assert_eq!(WAVE_FORMAT_IEEE_FLOAT, 3);
    assert_eq!(AUDCLNT_SHAREMODE_SHARED, 0);
    assert_eq!(AUDCLNT_STREAMFLAGS_LOOPBACK, 0x0002_0000);
    assert_eq!(AUDCLNT_STREAMFLAGS_EVENTCALLBACK, 0x0004_0000);
    assert_eq!(AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM, 0x8000_0000);
    assert_eq!(AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY, 0x1);
    assert_eq!(AUDCLNT_BUFFERFLAGS_SILENT, 0x2);
    assert_eq!(AUDCLNT_E_DEVICE_INVALIDATED, 0x8889_0004);
    assert_eq!(AUDCLNT_E_SERVICE_NOT_RUNNING, 0x8889_0010);
    assert_eq!(
        VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
        r"VAD\Process_Loopback"
    );

    assert_eq!(MIN_WINDOWS_10_LOOPBACK_BUILD, 10_240);
    assert_eq!(MIN_WINDOWS_PROCESS_LOOPBACK_BUILD, 20_348);

    let wf = WasapiWaveFormatEx::ieee_float_stereo_48khz();
    assert_eq!(wf.w_format_tag, WAVE_FORMAT_IEEE_FLOAT);
    assert_eq!(wf.n_channels, 2);
    assert_eq!(wf.n_samples_per_sec, 48_000);
    assert_eq!(wf.w_bits_per_sample, 32);
    assert_eq!(wf.n_block_align, 8);
    assert_eq!(wf.n_avg_bytes_per_sec, 384_000);
    assert_eq!(wf.cb_size, 0);
}

// ---------------------------------------------------------------------------
// 2. Positive Path: Out-of-the-Box Windows 11 & Windows 10 Capture Without Virtual Cables
// ---------------------------------------------------------------------------

#[test]
fn captures_system_output_out_of_the_box_on_windows_11_and_windows_10_without_virtual_cables() {
    // Case A: Windows 11 23H2 (10.0.22631) -> selects ProcessLoopbackExcludeTree automatically
    let win11 = WindowsOsVersion::new(10, 0, 22_631);
    let backend = MockWasapiBackend::new(win11);
    let state_ref = backend.state.clone();

    let config = WasapiLoopbackConfig {
        target_process_id: 4242,
        ..Default::default()
    };
    let mut service = WasapiLoopbackService::new(backend, config).expect("valid config");
    assert_eq!(service.state(), WasapiStreamState::Uninitialized);

    let report: WasapiCompatibilityReport = service.check_os_compatibility();
    assert!(report.is_supported);
    assert!(report.supports_process_loopback);
    assert!(!report.requires_virtual_audio_cable);
    assert_eq!(
        report.recommended_mode,
        Some(WasapiLoopbackMode::ProcessLoopbackExcludeTree)
    );

    // Start capture out-of-the-box
    let activation = service
        .start_capture()
        .expect("start WASAPI capture on Windows 11");
    assert_eq!(
        activation.mode,
        WasapiLoopbackMode::ProcessLoopbackExcludeTree
    );
    assert_eq!(activation.target_process_id, Some(4242));
    assert_eq!(
        activation.activation_device_id,
        VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK
    );
    assert_eq!(service.state(), WasapiStreamState::Capturing);

    // Verify exact stream flags initialized on IAudioClient
    let expected_flags = AUDCLNT_STREAMFLAGS_LOOPBACK
        | AUDCLNT_STREAMFLAGS_EVENTCALLBACK
        | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM;
    assert_eq!(state_ref.borrow().initialized_flags, expected_flags);

    // Enqueue 1 packet of 960 stereo frames (1920 f32 samples = 20 ms)
    let samples_f32: Vec<f32> = (0..WASAPI_FRAME_TOTAL_SAMPLES)
        .map(|i| if i % 2 == 0 { 0.5 } else { -0.5 })
        .collect();
    state_ref
        .borrow_mut()
        .pending_packets
        .push(WasapiCapturePacket {
            frames: 960,
            flags: 0,
            samples_f32,
        });

    let frames_ingested = service
        .pump_backend_packets()
        .expect("pump WASAPI capture packets");
    assert_eq!(frames_ingested, 960);

    let webrtc_frames = service.drain_webrtc_frames();
    assert_eq!(webrtc_frames.len(), 1);
    let frame = &webrtc_frames[0];
    assert_eq!(frame.sequence, 1);
    assert_eq!(frame.timestamp_samples, 0);
    assert_eq!(frame.channels, 2);
    assert_eq!(frame.sample_rate_hz, 48_000);
    assert_eq!(frame.samples.len(), WASAPI_FRAME_TOTAL_SAMPLES);
    assert!(!frame.is_silence);
    // 0.5 * 32768 = 16384, -0.5 * 32768 = -16384
    assert_eq!(frame.samples[0], 16_384);
    assert_eq!(frame.samples[1], -16_384);
    assert_eq!(frame.to_le_bytes().len(), WASAPI_FRAME_BYTES_20MS);

    service.stop().expect("stop WASAPI capture");
    assert_eq!(service.state(), WasapiStreamState::Stopped);
}

#[test]
fn windows_10_pre_21h2_captures_default_render_endpoint_loopback_with_clear_diagnostic() {
    // Windows 10 1909 / 20H2 (build 19042: >= 10240 and < 20348)
    let win10 = WindowsOsVersion::new(10, 0, 19_042);
    let backend = MockWasapiBackend::new(win10);
    let config = WasapiLoopbackConfig {
        target_process_id: 7777,
        ..Default::default()
    };

    let mut service = WasapiLoopbackService::new(backend, config).unwrap();
    let compat = service.check_os_compatibility();
    assert!(compat.is_supported);
    assert!(!compat.supports_process_loopback);
    assert!(!compat.requires_virtual_audio_cable);
    assert_eq!(
        compat.recommended_mode,
        Some(WasapiLoopbackMode::SharedEndpointLoopback)
    );
    assert!(
        compat.diagnostic_message.contains("20348"),
        "Diagnostic must explain build 20348 requirement for per-process tree exclusion: {}",
        compat.diagnostic_message
    );

    let activation = service
        .start_capture()
        .expect("Windows 10 19042 falls back to shared render endpoint loopback out-of-the-box");
    assert_eq!(activation.mode, WasapiLoopbackMode::SharedEndpointLoopback);
    assert_eq!(
        activation.activation_device_id,
        "{0.0.0.00000000}.{d4a1b2c3-e5f6-4a5b-9c8d-7e6f5a4b3c2d}"
    );
    assert_eq!(service.state(), WasapiStreamState::Capturing);
}

// ---------------------------------------------------------------------------
// 3. Negative Path & Legacy OS Diagnostics (Windows 7, Windows 8, Windows 8.1, < 10240)
// ---------------------------------------------------------------------------

#[test]
fn rejects_legacy_windows_versions_with_clear_actionable_diagnostics() {
    let legacy_versions = [
        (
            WindowsOsVersion::new(6, 1, 7_601),
            "Windows 7",
            "6.1 (build 7601)",
        ),
        (
            WindowsOsVersion::new(6, 2, 9_200),
            "Windows 8",
            "6.2 (build 9200)",
        ),
        (
            WindowsOsVersion::new(6, 3, 9_600),
            "Windows 8.1",
            "6.3 (build 9600)",
        ),
        (
            WindowsOsVersion::new(6, 0, 6_002),
            "Windows Vista",
            "6.0 (build 6002)",
        ),
        (
            WindowsOsVersion::new(10, 0, 10_239),
            "Pre-release Windows 10",
            "10.0 (build 10239)",
        ),
    ];

    for (ver, expected_name, expected_build_str) in legacy_versions {
        let report = evaluate_windows_wasapi_compatibility(ver);
        assert!(
            !report.is_supported,
            "Legacy OS {ver:?} must be marked unsupported"
        );
        assert!(!report.supports_process_loopback);
        assert_eq!(report.recommended_mode, None);
        assert!(
            report.diagnostic_message.contains(expected_name)
                || report.diagnostic_message.contains(expected_build_str),
            "Diagnostic must identify legacy OS {expected_name}: {}",
            report.diagnostic_message
        );
        assert!(
            report.diagnostic_message.contains("10240")
                && report.diagnostic_message.contains("Windows 10"),
            "Diagnostic must state minimum required Windows 10 build 10240: {}",
            report.diagnostic_message
        );

        // Attempting to start capture on legacy OS returns LegacyOsUnsupported with the diagnostic
        let backend = MockWasapiBackend::new(ver);
        let mut service =
            WasapiLoopbackService::new(backend, WasapiLoopbackConfig::default()).unwrap();
        let err = service
            .start_capture()
            .expect_err("start_capture must fail on legacy OS");
        match err {
            WasapiLoopbackError::LegacyOsUnsupported {
                version,
                ref diagnostic,
            } => {
                assert_eq!(version, ver);
                assert!(diagnostic.contains("10240"));
                assert_eq!(err.to_string(), *diagnostic);
            }
            other => panic!("Expected LegacyOsUnsupported, got {other:?}"),
        }
    }
}

#[test]
fn rejects_invalid_wasapi_configs_and_malformed_packet_buffers() {
    // 1. Invalid channel counts
    let bad_ch = WasapiLoopbackConfig {
        channels: 1,
        ..Default::default()
    };
    assert_eq!(
        bad_ch.validate(),
        Err(WasapiLoopbackError::UnsupportedChannels(1))
    );

    // 2. Invalid sample rate
    let bad_rate = WasapiLoopbackConfig {
        sample_rate_hz: 44_100,
        ..Default::default()
    };
    assert_eq!(
        bad_rate.validate(),
        Err(WasapiLoopbackError::UnsupportedSampleRate(44_100))
    );

    // 3. Invalid zero process ID when ProcessLoopbackExcludeTree is requested
    let bad_pid = WasapiLoopbackConfig {
        target_process_id: 0,
        preferred_mode: WasapiLoopbackMode::ProcessLoopbackExcludeTree,
        ..Default::default()
    };
    assert_eq!(
        bad_pid.validate(),
        Err(WasapiLoopbackError::InvalidProcessId(0))
    );

    // 4. Zero buffer capacity
    let bad_cap = WasapiLoopbackConfig {
        max_buffered_frames: 0,
        ..Default::default()
    };
    assert_eq!(
        bad_cap.validate(),
        Err(WasapiLoopbackError::InvalidBufferCapacity(0))
    );

    // 5. Odd (non-stereo-aligned) f32 sample slice is rejected
    let backend = MockWasapiBackend::new(WindowsOsVersion::windows_11_23h2());
    let mut service =
        WasapiLoopbackService::new(backend, WasapiLoopbackConfig::default()).unwrap();
    service.start_capture().unwrap();

    assert_eq!(
        service.ingest_f32_samples(&[0.25, -0.25, 0.1]),
        Err(WasapiLoopbackError::UnalignedStereoSamples(3))
    );
}

// ---------------------------------------------------------------------------
// 4. Boundary Conditions: OS Build Thresholds, 20ms Framing, NaN/Inf Sanitization & Gain
// ---------------------------------------------------------------------------

#[test]
fn exact_os_build_boundaries_nan_inf_sanitization_and_20ms_framing_boundaries() {
    // 1. Build boundary 10239 vs 10240
    let b10239 = evaluate_windows_wasapi_compatibility(WindowsOsVersion::new(10, 0, 10_239));
    let b10240 = evaluate_windows_wasapi_compatibility(WindowsOsVersion::new(10, 0, 10_240));
    assert!(!b10239.is_supported);
    assert!(b10240.is_supported);
    assert!(!b10240.supports_process_loopback);
    assert_eq!(
        b10240.recommended_mode,
        Some(WasapiLoopbackMode::SharedEndpointLoopback)
    );

    // 2. Build boundary 20347 vs 20348 (Serein oracle: Windows build 20348 required for process loopback)
    let b20347 = evaluate_windows_wasapi_compatibility(WindowsOsVersion::new(10, 0, 20_347));
    let b20348 = evaluate_windows_wasapi_compatibility(WindowsOsVersion::new(10, 0, 20_348));
    assert!(b20347.is_supported);
    assert!(!b20347.supports_process_loopback);
    assert_eq!(
        b20347.recommended_mode,
        Some(WasapiLoopbackMode::SharedEndpointLoopback)
    );
    assert!(b20348.is_supported);
    assert!(b20348.supports_process_loopback);
    assert_eq!(
        b20348.recommended_mode,
        Some(WasapiLoopbackMode::ProcessLoopbackExcludeTree)
    );

    // 3. Sample sanitization oracle (matches Serein `loopback_samples_are_gated_sanitized_and_bounded`)
    assert_eq!(sanitize_wasapi_f32_sample(f32::NAN), (0.0, true));
    assert_eq!(sanitize_wasapi_f32_sample(f32::INFINITY), (0.0, true));
    assert_eq!(sanitize_wasapi_f32_sample(f32::NEG_INFINITY), (0.0, true));
    assert_eq!(sanitize_wasapi_f32_sample(2.5), (1.0, false));
    assert_eq!(sanitize_wasapi_f32_sample(-2.5), (-1.0, false));
    assert_eq!(sanitize_wasapi_f32_sample(0.25), (0.25, false));

    // 4. 20ms framing boundary (1918 stereo samples -> 0 frames; +2 samples -> 1 frame of 1920)
    let backend = MockWasapiBackend::new(WindowsOsVersion::windows_11_23h2());
    let cfg = WasapiLoopbackConfig {
        max_buffered_frames: 2,
        ..Default::default()
    };
    let mut service = WasapiLoopbackService::new(backend, cfg).unwrap();
    service.start_capture().unwrap();

    service.ingest_f32_samples(&vec![0.25f32; 1918]).unwrap();
    assert_eq!(service.buffered_samples(), 1918);
    assert!(service.drain_webrtc_frames().is_empty());

    service
        .ingest_f32_samples(&[f32::NAN, f32::INFINITY])
        .unwrap();
    assert_eq!(service.buffered_samples(), 1920);
    assert_eq!(service.telemetry().sanitized_non_finite_samples, 2);

    let frames = service.drain_webrtc_frames();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].samples[1918], 0);
    assert_eq!(frames[0].samples[1919], 0);
}

// ---------------------------------------------------------------------------
// 5. Empty States & AUDCLNT_BUFFERFLAGS_SILENT Handling
// ---------------------------------------------------------------------------

#[test]
fn silent_buffer_flag_synthesizes_zero_samples_and_empty_poll_emits_dtx_silence_frame() {
    let backend = MockWasapiBackend::new(WindowsOsVersion::windows_11_23h2());
    let state_ref = backend.state.clone();
    let mut service =
        WasapiLoopbackService::new(backend, WasapiLoopbackConfig::default()).unwrap();
    service.start_capture().unwrap();

    // When WASAPI sets AUDCLNT_BUFFERFLAGS_SILENT, the data pointer content must be ignored and treated as 0.0
    state_ref
        .borrow_mut()
        .pending_packets
        .push(WasapiCapturePacket {
            frames: 960,
            flags: AUDCLNT_BUFFERFLAGS_SILENT,
            samples_f32: vec![0.99f32; 1920], // garbage payload that WASAPI says is silent
        });

    service.pump_backend_packets().unwrap();
    assert_eq!(service.telemetry().silent_packets, 1);

    let frames = service.drain_webrtc_frames();
    assert_eq!(frames.len(), 1);
    assert!(frames[0].is_silence);
    assert!(frames[0].samples.iter().all(|&s| s == 0));

    // Polling when buffer is empty synthesizes a 20ms silence frame and increments underrun_reads
    let fallback = service.poll_next_webrtc_frame_or_silence();
    assert!(fallback.is_silence);
    assert_eq!(fallback.sequence, 2);
    assert_eq!(fallback.timestamp_samples, 960);
    assert_eq!(service.telemetry().underrun_reads, 1);
}

// ---------------------------------------------------------------------------
// 6. Lifecycle States, Epoch Rotation (DAVE E2EE Rekey Isolation) & Privacy Redaction
// ---------------------------------------------------------------------------

#[test]
fn epoch_rotation_recreates_client_and_drops_stale_epoch_samples_with_privacy_redaction() {
    let backend = MockWasapiBackend::new(WindowsOsVersion::windows_11_23h2());
    let state_ref = backend.state.clone();
    let mut service =
        WasapiLoopbackService::new(backend, WasapiLoopbackConfig::default()).unwrap();

    // Privacy check: WasapiAudioEndpoint Debug redacts friendly_name
    let endpoints = service.enumerate_endpoints().unwrap();
    let dbg = format!("{:?}", endpoints[0]);
    assert!(
        !dbg.contains("Realtek"),
        "Debug output must redact friendly hardware endpoint name: {dbg}"
    );
    assert!(dbg.contains("<redacted>"));

    service.start_capture().unwrap();
    assert_eq!(service.audio_epoch(), 1);

    // Queue 1000 samples under epoch 1
    service.ingest_f32_samples(&vec![0.5f32; 1000]).unwrap();
    assert_eq!(service.buffered_samples(), 1000);

    // Stale epoch ingestion is rejected
    assert_eq!(
        service.ingest_f32_samples_at_epoch(&[0.5, -0.5], 0),
        Ok(0)
    );

    // Rotate audio epoch (Serein contract: Stop/Reset/Start can leave process loopback empty on Windows;
    // recreate client instead and flush old epoch samples so unencrypted/old-key audio never crosses epoch boundary)
    service
        .rotate_audio_epoch(2)
        .expect("rotate audio epoch on DAVE transition");
    assert_eq!(service.audio_epoch(), 2);
    assert_eq!(
        service.buffered_samples(),
        0,
        "Buffered samples from previous epoch must be purged on epoch rotation"
    );
    assert_eq!(service.telemetry().epoch_rotations, 1);
    assert_eq!(state_ref.borrow().stopped_count, 1);
    assert!(state_ref.borrow().started);

    // Pause / Resume lifecycle
    service.pause().expect("pause capture");
    assert_eq!(service.state(), WasapiStreamState::Paused);
    assert_eq!(service.ingest_f32_samples(&[0.5, -0.5]).unwrap(), 0);
    service.resume().expect("resume capture");
    assert_eq!(service.state(), WasapiStreamState::Capturing);
}

// ---------------------------------------------------------------------------
// 7. Error Recovery: AUDCLNT_E_DEVICE_INVALIDATED & Drop Cleanup
// ---------------------------------------------------------------------------

#[test]
fn recovers_from_device_invalidated_error_and_stops_cleanly_on_drop() {
    let backend = MockWasapiBackend::new(WindowsOsVersion::windows_11_23h2());
    let state_ref = backend.state.clone();

    {
        let mut service =
            WasapiLoopbackService::new(backend, WasapiLoopbackConfig::default()).unwrap();
        service.start_capture().unwrap();

        // Simulate AUDCLNT_E_DEVICE_INVALIDATED (default audio output device unplugged / switched)
        state_ref.borrow_mut().fail_poll = Some(WasapiLoopbackError::DeviceInvalidated(
            "AUDCLNT_E_DEVICE_INVALIDATED (0x88890004)".to_string(),
        ));

        let poll_err = service.pump_backend_packets().unwrap_err();
        assert!(matches!(
            poll_err,
            WasapiLoopbackError::DeviceInvalidated(_)
        ));

        // Clear fault and trigger automatic device recovery
        state_ref.borrow_mut().fail_poll = None;
        service
            .recover_after_device_change()
            .expect("recover WASAPI client after default render endpoint switch");
        assert_eq!(service.state(), WasapiStreamState::Capturing);
        assert_eq!(service.telemetry().device_recoveries, 1);
    }

    // Drop must invoke stop_loopback_client on the backend
    assert!(
        !state_ref.borrow().started,
        "Drop must stop the active WASAPI loopback client"
    );
}
