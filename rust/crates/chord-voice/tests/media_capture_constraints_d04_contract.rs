#![forbid(unsafe_code)]
//! Contract tests for Task D-04: Media Capture Constraints Protocol.
//!
//! Verifies the complete specification and implementation of Discord's media device selection,
//! AGC algorithms, noise suppression flags, echo cancellation options, sample rate requirements,
//! and stream renegotiation against the clean-room oracle documented in `docs/re/voice-deep.md`
//! (§5) and `docs/re/voice-deep.json` (`media_capture_constraints_d04`), as well as `docs/re/native-re.md`:
//! 1. Audio capture parameters & sample rate requirements (`8000`, `16000`, `32000`, `48000` Hz;
//!    canonical WebRTC Opus capture `48000` Hz, `2` stereo channels, `10`/`20`/`30` ms frames,
//!    960 samples/ch @ 20 ms, `setVoiceSampleRateCap`, `setVoiceChannelCountCap`,
//!    `sampleRateMismatchPercent <= 15%`).
//! 2. Media device selection protocol (`deviceId: { exact: "<id>" }`, `setInputDevice`,
//!    `setOutputDevice`, `setVideoInputDevice`, virtual audio loopback injection for PipeWire
//!    `chord-screen-share-audio` & WASAPI `chord-wasapi-loopback-audio`, hot-plug fallback and
//!    preferred exact device auto-restoration, `OwnApplicationFilter` anti-echo PID/binary isolation).
//! 3. Echo cancellation (AEC) options (`echoCancellation`, `echoCancellationPreEcho`,
//!    `echoReferenceMode`, `builtInEchoCancellation`, `bypassSystemProcessing`,
//!    `enforce_high_pass_filtering`, `echo_delay_ms`, `NoopEchoControl` vs WebRTC AEC3).
//! 4. Automatic Gain Control (AGC) algorithms (`AdaptiveDigital` with `target_level_dbfs: -3`,
//!    `compression_gain_db: 9`, `limiter_enabled: true`, `vadKrispActivationThreshold: 0.8`;
//!    `FixedDigital` with `auto_gain_control: false`, `compression_gain_db: 0`;
//!    `BypassedForStream` with full DSP passthrough for screen-share system audio).
//! 5. Noise suppression flags & Krisp/RNNoise/WebRTC pipeline (`noiseSuppression`,
//!    `noiseCancellation`, `noiseCancellationDuringProcessing`, `skipNoiseCancellationIfMuted`,
//!    `vadThreshold: -40 dBFS`, `vadLeading: 5`, `vadTrailing: 25`, `vadAutoThreshold`,
//!    `vadUseKrisp`, 5 `.kef` models with SHA-256 hashes, CPU overuse fallback `> 12000 us`
//!    for `5` consecutive frames).
//! 6. Stream renegotiation & video resolution/bitrate bounds (`480p`..`2160p`, `160x90`..`3840x2160`,
//!    even YUV420 alignment, dynamic mid-stream virtual audio injection, `renegotiate_video_bounds`,
//!    `Voice Opcode 12 Video`, `Voice Opcode 5 SPEAKING` `SOUNDSHARE = 2`, `Voice Opcode 15 MediaSinkWants`,
//!    `ConnectionEventFramerateReducer` 15,000 ms -> 20 FPS).
//! 7. 9 mandatory testing dimensions (`TEST-POLICY.md` §12): positive, negative, boundary,
//!    error handling, empty states, lifecycle/renegotiation states, capabilities/privacy redaction,
//!    recovery/degradation, and observable telemetry.

use std::fs;
use std::path::{Path, PathBuf};

use chord_json::JsonValue;
use chord_voice::devices::{
    AudioDevice, AudioDeviceKind, DEFAULT_INPUT_VOLUME, DEFAULT_OUTPUT_VOLUME, MAX_INPUT_VOLUME,
    MAX_OUTPUT_VOLUME, OwnApplicationFilter, VoiceDevicesState,
};
use chord_voice::media_constraints::{
    AgcMode, AgcPolicyConfig, AudioTrackRole, DEFAULT_AUDIO_CHANNELS, DEFAULT_AUDIO_SAMPLE_RATE_HZ,
    DEFAULT_WASAPI_VIRTUAL_AUDIO_DEVICE_ID, DeviceIdMatchConstraint, EncoderFrameDecision,
    MediaConstraintsEngine, ResolutionBounds, ResolutionPreset, SOUNDSHARE_SPEAKING_BIT,
    VideoEncoderFrameInput, VirtualAudioTrackDescriptor,
};
use chord_voice::noise::{
    AGC_KRISP_VAD_THRESHOLD, CPU_OVERUSE_CONSECUTIVE_FRAMES_LIMIT, DEFAULT_KRISP_VAD_THRESHOLD,
    DEFAULT_VAD_LEADING_FRAMES, DEFAULT_VAD_THRESHOLD_DB, DEFAULT_VAD_TRAILING_FRAMES,
    KrispModelTier, MAX_CPU_PROCESS_TIME_US_PER_20MS, MAX_SAMPLE_RATE_MISMATCH_PERCENT,
    NoiseCancellerState, NoiseSuppressionConfig, NoiseSuppressionError, NoiseSuppressionPipeline,
    SUPPORTED_FRAME_DURATIONS_MS, SUPPORTED_SAMPLE_RATES_HZ, SuppressionBackend,
};
use chord_voice::session::MediaSinkWants;
use chord_voice::stream_quality::{
    CONNECTION_IDLE_FRAMERATE_REDUCE_MS, DEFAULT_720P30_TARGET_BITRATE_BPS,
    DEFAULT_720P60_TARGET_BITRATE_BPS, GoLiveQualityPatchPolicy, IDLE_REDUCED_MAX_FRAMERATE,
    StreamAdaptationState, StreamProfilePreset, StreamQualityController, StreamQualityProfile,
};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn load_voice_deep_md() -> String {
    let path = repo_root().join("docs/re/voice-deep.md");
    fs::read_to_string(&path).expect("docs/re/voice-deep.md must exist")
}

fn load_voice_deep_json() -> JsonValue {
    let path = repo_root().join("docs/re/voice-deep.json");
    let raw = fs::read_to_string(&path).expect("docs/re/voice-deep.json must exist");
    chord_json::parse(&raw).expect("docs/re/voice-deep.json must be valid JSON")
}

fn sample_devices() -> Vec<AudioDevice> {
    vec![
        AudioDevice {
            id: "mic-default-alsa".to_owned(),
            label: "Built-in Analog Stereo Microphone".to_owned(),
            kind: AudioDeviceKind::Input,
            is_default: true,
        },
        AudioDevice {
            id: "mic-usb-focusrite".to_owned(),
            label: "Focusrite Scarlett 2i2 USB".to_owned(),
            kind: AudioDeviceKind::Input,
            is_default: false,
        },
        AudioDevice {
            id: "spk-default-alsa".to_owned(),
            label: "Built-in Analog Stereo Output".to_owned(),
            kind: AudioDeviceKind::Output,
            is_default: true,
        },
        AudioDevice {
            id: "spk-usb-dac".to_owned(),
            label: "External USB Studio Headphones".to_owned(),
            kind: AudioDeviceKind::Output,
            is_default: false,
        },
    ]
}

/// Dimension 1 (Positive & Oracle Parity): `docs/re/voice-deep.md` (§5) and
/// `docs/re/voice-deep.json` (`media_capture_constraints_d04`) document the complete
/// D-04 Media Capture Constraints Protocol and match the Voice Engine constants.
#[test]
fn d04_dim1_clean_room_oracle_documentation_and_schema_parity() {
    let md = load_voice_deep_md();
    let json = load_voice_deep_json();

    // 1. Verify markdown specification sections in docs/re/voice-deep.md
    let required_md_markers = [
        "## 5. Media Capture Constraints Protocol (D-04)",
        "### 5.1 Audio Capture Parameters & Sample Rate Requirements",
        "### 5.2 Media Device Selection & Virtual Audio Loopback Injection",
        "### 5.3 Echo Cancellation (AEC) Options & Acoustic Pipeline",
        "### 5.4 Automatic Gain Control (AGC) Algorithms & Profiles",
        "### 5.5 Noise Suppression Flags, Krisp Models & VAD Gating",
        "### 5.6 Stream Renegotiation & Video Encoder Resolution Bounds",
        "setVoiceSampleRateCap",
        "setVoiceChannelCountCap",
        "sampleRateMismatchPercent",
        "echoCancellationPreEcho",
        "echoReferenceMode",
        "builtInEchoCancellation",
        "bypassSystemProcessing",
        "automaticGainControlConfig",
        "noiseCancellationDuringProcessing",
        "skipNoiseCancellationIfMuted",
        "vadKrispActivationThreshold",
        "AdaptiveDigital",
        "FixedDigital",
        "BypassedForStream",
        "chord-screen-share-audio",
        "chord-wasapi-loopback-audio",
        "ConnectionEventFramerateReducer",
    ];
    for marker in required_md_markers {
        assert!(
            md.contains(marker),
            "docs/re/voice-deep.md must document D-04 contract marker: {marker}"
        );
    }

    // 2. Verify machine-readable specification in docs/re/voice-deep.json
    let root_obj = json.as_object().expect("voice-deep.json root object");
    let d04 = root_obj
        .get("media_capture_constraints_d04")
        .and_then(JsonValue::as_object)
        .expect("voice-deep.json must contain media_capture_constraints_d04 object");

    // 2a. Audio capture parameters & sample rate requirements
    let audio_cap = d04
        .get("audio_capture_parameters")
        .and_then(JsonValue::as_object)
        .expect("audio_capture_parameters section must exist");
    assert_eq!(
        audio_cap
            .get("canonical_sample_rate_hz")
            .and_then(JsonValue::as_f64),
        Some(f64::from(DEFAULT_AUDIO_SAMPLE_RATE_HZ))
    );
    assert_eq!(
        audio_cap
            .get("canonical_channels")
            .and_then(JsonValue::as_f64),
        Some(f64::from(DEFAULT_AUDIO_CHANNELS))
    );
    assert_eq!(
        audio_cap
            .get("max_sample_rate_mismatch_percent")
            .and_then(JsonValue::as_f64),
        Some(f64::from(MAX_SAMPLE_RATE_MISMATCH_PERCENT))
    );
    let supported_rates: Vec<u32> = audio_cap
        .get("supported_sample_rates_hz")
        .and_then(JsonValue::as_array)
        .expect("supported_sample_rates_hz array")
        .iter()
        .map(|v| v.as_f64().expect("rate number") as u32)
        .collect();
    assert_eq!(supported_rates, SUPPORTED_SAMPLE_RATES_HZ);

    let supported_durations: Vec<u16> = audio_cap
        .get("supported_frame_durations_ms")
        .and_then(JsonValue::as_array)
        .expect("supported_frame_durations_ms array")
        .iter()
        .map(|v| v.as_f64().expect("duration number") as u16)
        .collect();
    assert_eq!(supported_durations, SUPPORTED_FRAME_DURATIONS_MS);

    // 2b. Echo cancellation options
    let aec = d04
        .get("echo_cancellation_options")
        .and_then(JsonValue::as_object)
        .expect("echo_cancellation_options section must exist");
    assert_eq!(
        aec.get("engine").and_then(JsonValue::as_str),
        Some("WebRTC_AEC3")
    );
    assert_eq!(
        aec.get("enforce_high_pass_filtering")
            .and_then(JsonValue::as_bool),
        Some(true)
    );
    assert_eq!(
        aec.get("voice_default_enabled")
            .and_then(JsonValue::as_bool),
        Some(true)
    );
    assert_eq!(
        aec.get("screen_share_enabled").and_then(JsonValue::as_bool),
        Some(false)
    );

    // 2c. AGC algorithms
    let agc = d04
        .get("agc_algorithms")
        .and_then(JsonValue::as_object)
        .expect("agc_algorithms section must exist");
    let adaptive = agc
        .get("AdaptiveDigital")
        .and_then(JsonValue::as_object)
        .expect("AdaptiveDigital profile");
    let voice_default = AgcPolicyConfig::voice_adaptive_default();
    assert_eq!(
        adaptive
            .get("auto_gain_control")
            .and_then(JsonValue::as_bool),
        Some(voice_default.auto_gain_control)
    );
    assert_eq!(
        adaptive
            .get("target_level_dbfs")
            .and_then(JsonValue::as_f64),
        Some(f64::from(voice_default.target_level_dbfs))
    );
    assert_eq!(
        adaptive
            .get("compression_gain_db")
            .and_then(JsonValue::as_f64),
        Some(f64::from(voice_default.compression_gain_db))
    );
    assert_eq!(
        adaptive.get("limiter_enabled").and_then(JsonValue::as_bool),
        Some(voice_default.limiter_enabled)
    );

    let fixed = agc
        .get("FixedDigital")
        .and_then(JsonValue::as_object)
        .expect("FixedDigital profile");
    let voice_fixed = AgcPolicyConfig::voice_fixed_manual();
    assert_eq!(
        fixed.get("auto_gain_control").and_then(JsonValue::as_bool),
        Some(voice_fixed.auto_gain_control)
    );
    assert_eq!(
        fixed.get("compression_gain_db").and_then(JsonValue::as_f64),
        Some(f64::from(voice_fixed.compression_gain_db))
    );

    let bypassed = agc
        .get("BypassedForStream")
        .and_then(JsonValue::as_object)
        .expect("BypassedForStream profile");
    let stream_passthrough = AgcPolicyConfig::screen_share_passthrough();
    assert_eq!(
        bypassed
            .get("auto_gain_control")
            .and_then(JsonValue::as_bool),
        Some(stream_passthrough.auto_gain_control)
    );
    assert_eq!(
        bypassed
            .get("echo_cancellation")
            .and_then(JsonValue::as_bool),
        Some(stream_passthrough.echo_cancellation)
    );
    assert_eq!(
        bypassed
            .get("noise_suppression")
            .and_then(JsonValue::as_bool),
        Some(stream_passthrough.noise_suppression)
    );

    // 2d. Noise suppression flags & stream renegotiation
    let ns = d04
        .get("noise_suppression_flags")
        .and_then(JsonValue::as_object)
        .expect("noise_suppression_flags section must exist");
    assert_eq!(
        ns.get("default_vad_threshold_db")
            .and_then(JsonValue::as_f64),
        Some(f64::from(DEFAULT_VAD_THRESHOLD_DB))
    );
    assert_eq!(
        ns.get("vad_leading_frames").and_then(JsonValue::as_f64),
        Some(f64::from(DEFAULT_VAD_LEADING_FRAMES))
    );
    assert_eq!(
        ns.get("vad_trailing_frames").and_then(JsonValue::as_f64),
        Some(f64::from(DEFAULT_VAD_TRAILING_FRAMES))
    );
    assert_eq!(
        ns.get("cpu_overuse_threshold_us_per_20ms")
            .and_then(JsonValue::as_f64),
        Some(MAX_CPU_PROCESS_TIME_US_PER_20MS as f64)
    );
    assert_eq!(
        ns.get("cpu_overuse_consecutive_frames")
            .and_then(JsonValue::as_f64),
        Some(f64::from(CPU_OVERUSE_CONSECUTIVE_FRAMES_LIMIT))
    );

    let reneg = d04
        .get("stream_renegotiation")
        .and_then(JsonValue::as_object)
        .expect("stream_renegotiation section must exist");
    assert_eq!(
        reneg
            .get("soundshare_speaking_bit")
            .and_then(JsonValue::as_f64),
        Some(f64::from(SOUNDSHARE_SPEAKING_BIT))
    );
    assert_eq!(
        reneg
            .get("idle_framerate_reduce_after_ms")
            .and_then(JsonValue::as_f64),
        Some(CONNECTION_IDLE_FRAMERATE_REDUCE_MS as f64)
    );
    assert_eq!(
        reneg
            .get("idle_reduced_max_fps")
            .and_then(JsonValue::as_f64),
        Some(f64::from(IDLE_REDUCED_MAX_FRAMERATE))
    );
}

/// Dimension 2 & 3 (Negative & Boundary Conditions): Audio capture sample rate requirements
/// (`8000`, `16000`, `32000`, `48000` Hz), channel counts (`1..=2` for APM/Krisp, `2` for WebRTC Opus),
/// frame durations (`10`, `20`, `30` ms), and `sampleRateMismatchPercent` (`<= 15%` vs `> 15%`).
#[test]
fn d04_dim2_and_dim3_sample_rate_requirements_frame_boundaries_and_mismatch_fallback() {
    // 1. Verify all 4 supported APM/Krisp sample rates and 3 frame durations
    for &rate_hz in &SUPPORTED_SAMPLE_RATES_HZ {
        for &duration_ms in &SUPPORTED_FRAME_DURATIONS_MS {
            for channels in 1..=2u8 {
                let mut pipeline =
                    NoiseSuppressionPipeline::new(NoiseSuppressionConfig::default_krisp());
                pipeline.initialize(true).expect("init krisp");
                let expected_samples =
                    (rate_hz as usize) * (duration_ms as usize) / 1000 * (channels as usize);
                let actual_samples = pipeline
                    .setup(rate_hz, channels, duration_ms)
                    .expect("valid sample rate, channel count, and frame duration");
                assert_eq!(actual_samples, expected_samples);
                assert_eq!(pipeline.stats().current_sample_rate, rate_hz);

                // Verify automatic Krisp model tier selection (`small_NC_8k` at 8kHz, `full_NC` at >=16kHz)
                let expected_tier = if rate_hz == 8_000 {
                    KrispModelTier::SmallNc8k
                } else {
                    KrispModelTier::FullNc
                };
                assert_eq!(pipeline.selected_krisp_model(), Some(expected_tier));
                assert_eq!(
                    pipeline.stats().krisp_nc_model,
                    expected_tier.kef_file_name()
                );
            }
        }
    }

    // 2. Reject unsupported sample rates (e.g., 22_050 Hz, 44_100 Hz, 96_000 Hz, 0 Hz)
    for bad_rate in [0u32, 11_025, 22_050, 44_100, 96_000] {
        let mut pipeline = NoiseSuppressionPipeline::new(NoiseSuppressionConfig::default_krisp());
        pipeline.initialize(true).unwrap();
        assert_eq!(
            pipeline.setup(bad_rate, 1, 20),
            Err(NoiseSuppressionError::UnsupportedSampleRate {
                sample_rate_hz: bad_rate
            })
        );
    }

    // 3. Reject unsupported channel counts (0 or > 2) and frame durations (5 ms, 15 ms, 40 ms)
    let mut pipeline = NoiseSuppressionPipeline::new(NoiseSuppressionConfig::default_krisp());
    pipeline.initialize(true).unwrap();
    assert_eq!(
        pipeline.setup(48_000, 0, 20),
        Err(NoiseSuppressionError::UnsupportedChannels { channels: 0 })
    );
    assert_eq!(
        pipeline.setup(48_000, 3, 20),
        Err(NoiseSuppressionError::UnsupportedChannels { channels: 3 })
    );
    for bad_duration in [0u16, 5, 15, 25, 40, 60] {
        assert_eq!(
            pipeline.setup(48_000, 1, bad_duration),
            Err(NoiseSuppressionError::UnsupportedFrameDuration {
                duration_ms: bad_duration
            })
        );
    }

    // 4. Boundary check for `sampleRateMismatchPercent`:
    // At exact boundary 15% (`MAX_SAMPLE_RATE_MISMATCH_PERCENT`), Krisp remains active.
    // At 16% (`> 15%`), Krisp automatically degrades to RNNoise to prevent pitch/timing distortion.
    pipeline.setup(48_000, 2, 20).unwrap();
    pipeline.report_sample_rate_mismatch(MAX_SAMPLE_RATE_MISMATCH_PERCENT);
    assert_eq!(pipeline.active_backend(), SuppressionBackend::Krisp);
    assert!(pipeline.stats().noise_canceller_is_enabled);

    pipeline.report_sample_rate_mismatch(MAX_SAMPLE_RATE_MISMATCH_PERCENT + 1);
    assert_eq!(pipeline.active_backend(), SuppressionBackend::RnNoise);
    assert!(!pipeline.stats().noise_canceller_is_enabled);
    assert_eq!(
        pipeline.stats().sample_rate_mismatch_percent,
        MAX_SAMPLE_RATE_MISMATCH_PERCENT + 1
    );
}

/// Dimension 1, 4 & 7: AGC Algorithms (`AdaptiveDigital`, `FixedDigital`, `BypassedForStream`),
/// Echo Cancellation Options, and Noise Suppression Flags Contract.
#[test]
fn d04_dim1_dim4_dim7_agc_algorithms_echo_cancellation_and_noise_suppression_flags() {
    let mut engine = MediaConstraintsEngine::with_devices(sample_devices());

    // 1. AdaptiveDigital profile (standard voice mic: AGC on, AEC on, NS on, -3 dBFS target, 9 dB compression, limiter on)
    let adaptive = AgcPolicyConfig::voice_adaptive_default();
    assert_eq!(adaptive.mode, AgcMode::AdaptiveDigital);
    assert!(adaptive.auto_gain_control);
    assert!(adaptive.echo_cancellation);
    assert!(adaptive.noise_suppression);
    assert!(adaptive.typing_noise_detection);
    assert_eq!(adaptive.target_level_dbfs, -3);
    assert_eq!(adaptive.compression_gain_db, 9);
    assert!(adaptive.limiter_enabled);

    let mic_adaptive = engine
        .negotiate_voice_input_constraints("mic-default-alsa", adaptive)
        .expect("AdaptiveDigital voice mic negotiation");
    assert_eq!(mic_adaptive.role, AudioTrackRole::VoiceMicrophone);
    assert_eq!(mic_adaptive.channels, DEFAULT_AUDIO_CHANNELS);
    assert_eq!(mic_adaptive.sample_rate_hz, DEFAULT_AUDIO_SAMPLE_RATE_HZ);
    assert_eq!(mic_adaptive.agc_policy, adaptive);

    // 2. FixedDigital profile (external USB audio interface: AGC off, compression 0 dB, AEC on, NS on, limiter on)
    let fixed = AgcPolicyConfig::voice_fixed_manual();
    assert_eq!(fixed.mode, AgcMode::FixedDigital);
    assert!(!fixed.auto_gain_control);
    assert!(fixed.echo_cancellation);
    assert!(fixed.noise_suppression);
    assert!(fixed.typing_noise_detection);
    assert_eq!(fixed.target_level_dbfs, -3);
    assert_eq!(fixed.compression_gain_db, 0);
    assert!(fixed.limiter_enabled);

    let mic_fixed = engine
        .negotiate_voice_input_constraints("mic-usb-focusrite", fixed)
        .expect("FixedDigital studio USB interface negotiation");
    assert_eq!(
        mic_fixed.device_id_constraint,
        DeviceIdMatchConstraint::Exact("mic-usb-focusrite".to_owned())
    );
    assert_eq!(mic_fixed.agc_policy, fixed);

    // 3. BypassedForStream profile (screen-share desktop loopback: AGC off, AEC off, NS off, typing detection off, limiter off)
    let passthrough = AgcPolicyConfig::screen_share_passthrough();
    assert_eq!(passthrough.mode, AgcMode::BypassedForStream);
    assert!(!passthrough.auto_gain_control);
    assert!(!passthrough.echo_cancellation);
    assert!(!passthrough.noise_suppression);
    assert!(!passthrough.typing_noise_detection);
    assert_eq!(passthrough.target_level_dbfs, 0);
    assert_eq!(passthrough.compression_gain_db, 0);
    assert!(!passthrough.limiter_enabled);

    let negotiated_stream = engine
        .negotiate_screen_share_constraints(
            Some(VirtualAudioTrackDescriptor::new_pipewire_default()),
            ResolutionBounds::from_preset(ResolutionPreset::P1080, 60),
        )
        .expect("screen-share negotiation with PipeWire virtual audio");
    let stream_audio = negotiated_stream.audio.as_ref().unwrap();
    assert_eq!(stream_audio.agc_policy, passthrough);
    assert_eq!(negotiated_stream.speaking_flags(), SOUNDSHARE_SPEAKING_BIT);

    // 4. Interaction between AGC flag and Krisp VAD confidence activation threshold:
    // When `automatic_gain_control == true`, non-finite/default fallback threshold elevates to `0.8` (`AGC_KRISP_VAD_THRESHOLD`);
    // when `automatic_gain_control == false`, fallback threshold is `0.5` (`DEFAULT_KRISP_VAD_THRESHOLD`).
    let mut cfg_with_agc = NoiseSuppressionConfig::default_krisp();
    cfg_with_agc.automatic_gain_control = true;
    cfg_with_agc.vad_krisp_activation_threshold = f32::NAN;
    assert!(
        (cfg_with_agc.normalized().vad_krisp_activation_threshold - AGC_KRISP_VAD_THRESHOLD).abs()
            < f32::EPSILON
    );

    let mut cfg_without_agc = NoiseSuppressionConfig::default_krisp();
    cfg_without_agc.automatic_gain_control = false;
    cfg_without_agc.vad_krisp_activation_threshold = f32::NAN;
    assert!(
        (cfg_without_agc.normalized().vad_krisp_activation_threshold - DEFAULT_KRISP_VAD_THRESHOLD)
            .abs()
            < f32::EPSILON
    );

    // 5. Verify `skipNoiseCancellationIfMuted` flag and CPU overuse degradation (`> 12_000 us` x 5 frames)
    let mut ns_pipeline = NoiseSuppressionPipeline::new(NoiseSuppressionConfig::default_krisp());
    ns_pipeline.initialize(true).unwrap();
    ns_pipeline.setup(48_000, 1, 20).unwrap();

    let mut muted_frame = vec![4_000i16; 960];
    let muted_res = ns_pipeline
        .process_frame(&mut muted_frame, true, -50.0, 0.95, 500)
        .unwrap();
    assert!(muted_res.skipped_because_muted);
    assert!(!muted_res.gate_open);
    assert!(muted_frame.iter().all(|&s| s == 0));

    for frame_idx in 1..=CPU_OVERUSE_CONSECUTIVE_FRAMES_LIMIT {
        let mut active_frame = vec![3_000i16; 960];
        let _ = ns_pipeline
            .process_frame(
                &mut active_frame,
                false,
                -55.0,
                0.95,
                MAX_CPU_PROCESS_TIME_US_PER_20MS + 500,
            )
            .unwrap();
        if frame_idx < CPU_OVERUSE_CONSECUTIVE_FRAMES_LIMIT {
            assert_eq!(ns_pipeline.state(), NoiseCancellerState::Configured);
            assert_eq!(ns_pipeline.active_backend(), SuppressionBackend::Krisp);
        } else {
            assert_eq!(
                ns_pipeline.state(),
                NoiseCancellerState::DisabledByCpuOveruse
            );
            assert_eq!(ns_pipeline.active_backend(), SuppressionBackend::RnNoise);
        }
    }
}

/// Dimension 5, 6, 8 & 9: Device Selection, Anti-Echo Loopback Isolation (`OwnApplicationFilter`),
/// Stream Renegotiation (`renegotiate_video_bounds`, `inject_virtual_audio_track`, `MediaSinkWants`,
/// `ConnectionEventFramerateReducer`), and Observable Telemetry.
#[test]
fn d04_dim5_dim6_dim8_dim9_device_selection_stream_renegotiation_and_telemetry() {
    // 1. VoiceDevicesState input/output device selection, volume bounds (0..=100% mic, 0..=200% output),
    // and mute/deafen effective_mute invariant.
    let mut dev_state = VoiceDevicesState::with_devices(sample_devices());
    assert_eq!(dev_state.selected_input(), Some("mic-default-alsa"));
    assert_eq!(dev_state.selected_output(), Some("spk-default-alsa"));
    assert_eq!(dev_state.input_volume(), DEFAULT_INPUT_VOLUME);
    assert_eq!(dev_state.output_volume(), DEFAULT_OUTPUT_VOLUME);

    dev_state.select_input("mic-usb-focusrite").unwrap();
    dev_state.select_output("spk-usb-dac").unwrap();
    assert_eq!(dev_state.set_input_volume(150), MAX_INPUT_VOLUME);
    assert_eq!(dev_state.set_output_volume(250), MAX_OUTPUT_VOLUME);

    // Self-deaf implies effective mute, and un-deafening restores explicit user_mute
    dev_state.set_self_deaf(true);
    assert_eq!(dev_state.gateway_mute_deaf(), (true, true));
    dev_state.set_self_deaf(false);
    assert_eq!(dev_state.gateway_mute_deaf(), (false, false));

    // 2. Anti-echo OwnApplicationFilter prevents capturing own client PID/binary in desktop loopback
    let filter = OwnApplicationFilter::with_binary(4242, "chord");
    assert!(!filter.allows_pid(4242));
    assert!(!filter.allows_pid(0));
    assert!(filter.allows_pid(9001));
    assert!(!filter.allows_proplist(Some("4242"), Some("firefox"), Some("Firefox"), None));
    assert!(!filter.allows_proplist(Some("9001"), Some("chord"), Some("Chord"), None));
    assert!(filter.allows_proplist(
        Some("9001"),
        Some("vlc"),
        Some("VLC Media Player"),
        Some("org.videolan.VLC")
    ));

    // 3. Stream renegotiation workflow:
    // Start 720p@30 video-only -> dynamically inject WASAPI virtual audio -> renegotiate to 1440p@60 ->
    // verify WebRTC constraints JSON and encoder bounds enforcement -> adapt under RTCP/MediaSinkWants/Idle.
    let mut engine = MediaConstraintsEngine::with_devices(sample_devices());
    engine
        .negotiate_voice_input_constraints(
            "mic-usb-focusrite",
            AgcPolicyConfig::voice_adaptive_default(),
        )
        .unwrap();

    let initial_stream = engine
        .negotiate_screen_share_constraints(
            None,
            ResolutionBounds::from_preset(ResolutionPreset::P720, 30),
        )
        .unwrap();
    assert_eq!(initial_stream.speaking_flags(), 0);
    assert!(
        initial_stream
            .to_webrtc_constraints_json()
            .contains("\"audio\":false")
    );

    // Dynamically inject virtual audio mid-stream
    let injected_audio = engine
        .inject_virtual_audio_track(VirtualAudioTrackDescriptor::new_wasapi_default())
        .unwrap();
    assert_eq!(
        injected_audio.effective_device_id(),
        DEFAULT_WASAPI_VIRTUAL_AUDIO_DEVICE_ID
    );
    assert_eq!(
        injected_audio.source_kind_or_role(),
        AudioTrackRole::ScreenShareSystemAudio
    );

    // Renegotiate video resolution bounds mid-stream from 720p@30 to 1440p@60
    let renegotiated = engine
        .renegotiate_video_bounds(ResolutionBounds::from_preset(ResolutionPreset::P1440, 60))
        .unwrap();
    assert_eq!(renegotiated.speaking_flags(), SOUNDSHARE_SPEAKING_BIT);
    assert_eq!(renegotiated.video.bounds.max_width, 2560);
    assert_eq!(renegotiated.video.bounds.max_height, 1440);
    assert_eq!(renegotiated.video.bounds.max_fps, 60);
    let webrtc_json = renegotiated.to_webrtc_constraints_json();
    assert!(
        webrtc_json.contains("\"deviceId\":{\"exact\":\"chord-wasapi-loopback-audio\"}")
    );
    assert!(webrtc_json.contains("\"autoGainControl\":false"));
    assert!(webrtc_json.contains("\"echoCancellation\":false"));
    assert!(webrtc_json.contains("\"noiseSuppression\":false"));
    assert!(webrtc_json.contains("\"sampleRate\":48000"));
    assert!(webrtc_json.contains("\"channelCount\":2"));

    // Verify 4K frame downscales to 1440p@60 bounds
    let scaled_decision = engine
        .enforce_video_encoder_bounds(VideoEncoderFrameInput {
            width: 3840,
            height: 2160,
            fps: 120,
            timestamp_us: 16_666,
        })
        .unwrap();
    assert_eq!(
        scaled_decision,
        EncoderFrameDecision::ScaledToBounds {
            original_width: 3840,
            original_height: 2160,
            encoded_width: 2560,
            encoded_height: 1440,
            encoded_fps: 60,
        }
    );

    // Verify Voice Engine StreamQualityController renegotiation & ConnectionEventFramerateReducer (15,000 ms -> 20 FPS)
    let mut sq_ctrl = StreamQualityController::new(
        9208,
        StreamQualityProfile::from_preset(StreamProfilePreset::P720Fps30),
        GoLiveQualityPatchPolicy::PatchedUnrestricted,
    )
    .unwrap();
    let op12_initial = sq_ctrl.handle_go_live(1280, 720);
    assert_eq!(op12_initial.audio_ssrc, 9208);
    assert_eq!(op12_initial.video_ssrc, 9209);
    assert_eq!(op12_initial.rtx_ssrc, 9210);
    assert_eq!(
        sq_ctrl.current_target_bitrate_bps(),
        DEFAULT_720P30_TARGET_BITRATE_BPS
    );

    // Renegotiate quality profile to 720p@60 (`4.0 Mbps`) and trigger 15,000 ms static screen idle reduction
    let op12_upgraded = sq_ctrl
        .set_quality_profile(StreamQualityProfile::from_preset(
            StreamProfilePreset::P720Fps60,
        ))
        .unwrap();
    assert_eq!(op12_upgraded.streams[0].max_framerate, 60);
    assert_eq!(
        sq_ctrl.current_target_bitrate_bps(),
        DEFAULT_720P60_TARGET_BITRATE_BPS
    );

    sq_ctrl.update_static_screen_idle_ms(CONNECTION_IDLE_FRAMERATE_REDUCE_MS);
    assert_eq!(
        sq_ctrl.evaluate_encoder_Params(1280, 720).encoded_fps,
        IDLE_REDUCED_MAX_FRAMERATE
    );

    // MediaSinkWants quality 0 pauses stream; quality 100 resumes stream
    sq_ctrl.apply_media_sink_wants(&MediaSinkWants::any(0));
    assert_eq!(sq_ctrl.state(), StreamAdaptationState::PausedBySink);
    sq_ctrl.apply_media_sink_wants(&MediaSinkWants::any(100));
    assert_eq!(sq_ctrl.state(), StreamAdaptationState::Active);

    // Verify telemetry counters on MediaConstraintsEngine
    let telemetry = engine.telemetry();
    assert_eq!(telemetry.negotiations_total, 3);
    assert_eq!(telemetry.virtual_tracks_injected, 1);
    assert_eq!(telemetry.exact_device_matches, 2);
    assert_eq!(telemetry.agc_bypassed_tracks, 1);
    assert_eq!(telemetry.frames_clamped_to_bounds, 1);
}

trait AudioTrackConstraintExt {
    fn source_kind_or_role(&self) -> AudioTrackRole;
}

impl AudioTrackConstraintExt for chord_voice::media_constraints::AudioTrackConstraint {
    fn source_kind_or_role(&self) -> AudioTrackRole {
        self.role
    }
}
