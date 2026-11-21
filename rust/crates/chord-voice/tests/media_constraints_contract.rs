#![forbid(unsafe_code)]
//! Contract tests for Task R5-08: Media Constraints Engine (`chord_voice::media_constraints`).
//!
//! Specification & Reference Oracles:
//! - `docs/feature-backlog.md` row `R5-08`:
//!   "Custom media constraint negotiation: virtual audio track injection, exact `deviceId` matching,
//!   AGC policy, resolution bounds | Vesktop: `screenShareFixes.ts`, `fixStreamConstraints.ts` |
//!   Voice Engine | Audio track remains stable through device changes; video encoder adheres
//!   strictly to selected resolution bounds"
//! - `docs/feature-backlog.md` row `D-04`:
//!   "Media Capture Constraints Protocol: Detailed contract of Discord's media device selection,
//!   AGC algorithms, noise suppression flags, and stream renegotiation"
//! - `docs/re/voice-deep.md` (§1, §2.3, §3: 48 kHz stereo 20 ms Opus audio, 90 kHz video clock,
//!   Voice OP 5 `SOUNDSHARE` `speaking == 2`, `updateVideoQuality: 1280x720@30, bitrate 2.5 Mbps`).
//!
//! Covers all 9 mandatory dimensions (`TEST-POLICY.md` §12):
//! 1. Positive path: custom media constraint negotiation with virtual audio track injection,
//!    exact `deviceId` matching (`deviceId: { exact: "<id>" }`), AGC/echo/noise suppression policy
//!    for voice vs desktop screen-share audio, and strict video encoder resolution bounds.
//! 2. Negative path: rejecting empty `deviceId`, mismatched device kind (`Output` as `Input`),
//!    zero/odd/out-of-range video dimensions (`< 160x90` or `> 3840x2160`), invalid frame rates
//!    (`0` or `> 120` FPS), and inverted min/max resolution bounds (`min > max`).
//! 3. Boundary conditions: exact resolution presets (`480p = 854x480`, `720p = 1280x720`,
//!    `1080p = 1920x1080`, `1440p = 2560x1440`, `2160p = 3840x2160`), minimum (`160x90`) and
//!    maximum (`3840x2160`) bounds, aspect-ratio preserving downscaling with even-dimension
//!    YUV420 macroblock alignment, and FPS boundaries (`15`, `30`, `60`, `120`).
//! 4. Error handling: explicit `MediaConstraintError` variants when negotiating prior to device
//!    registration, injecting a missing virtual audio source, or applying invalid bounds.
//! 5. Empty states: device list empty or no virtual audio track attached yields clean error /
//!    graceful video-only constraint without panic.
//! 6. Intermediate / lifecycle states: `Idle -> Negotiating -> Active -> Renegotiating -> Stopped`
//!    state machine transitions and mid-stream constraint renegotiation.
//! 7. Capabilities & privacy: `Debug` redaction of human-readable OS device labels, studio/screen-share
//!    audio isolation (`autoGainControl: false`, `echoCancellation: false`, `noiseSuppression: false`),
//!    and Voice Gateway OP 5 `SOUNDSHARE` (`speaking == 2`) payload generation.
//! 8. Recovery & degradation: audio track stability across physical microphone/headset hot-plug
//!    and removal events (virtual screen-share audio track remains pinned and uninterrupted; voice
//!    track recovers to default device when pinned physical mic disconnects and re-pins on reconnect).
//! 9. Observable telemetry: accurate tracking of `negotiations_total`, `virtual_tracks_injected`,
//!    `exact_device_matches`, `agc_bypassed_tracks`, `frames_clamped_to_bounds`,
//!    `frames_dropped_out_of_bounds`, `device_change_events_handled`, and `track_stability_preserved`.

use chord_voice::devices::{AudioDevice, AudioDeviceKind};
use chord_voice::media_constraints::{
    AgcMode, AgcPolicyConfig, AudioTrackConstraint, AudioTrackRole, DEFAULT_AUDIO_CHANNELS,
    DEFAULT_AUDIO_SAMPLE_RATE_HZ, DEFAULT_VIRTUAL_AUDIO_DEVICE_ID, DeviceIdMatchConstraint,
    EncoderFrameDecision, MAX_STREAM_FPS, MAX_VIDEO_HEIGHT, MAX_VIDEO_WIDTH, MIN_STREAM_FPS,
    MIN_VIDEO_HEIGHT, MIN_VIDEO_WIDTH, MediaConstraintError, MediaConstraintsEngine,
    MediaConstraintsLifecycleState, MediaConstraintsTelemetry, NegotiatedStreamConstraints,
    ResolutionBounds, ResolutionPreset, VideoEncoderFrameInput, VideoResolutionConstraint,
    VirtualAudioSourceKind, VirtualAudioTrackDescriptor,
};

fn sample_audio_devices() -> Vec<AudioDevice> {
    vec![
        AudioDevice {
            id: "mic-default-alsa".to_owned(),
            label: "Built-in Analog Microphone".to_owned(),
            kind: AudioDeviceKind::Input,
            is_default: true,
        },
        AudioDevice {
            id: "mic-usb-shure".to_owned(),
            label: "Shure MV7 USB Microphone".to_owned(),
            kind: AudioDeviceKind::Input,
            is_default: false,
        },
        AudioDevice {
            id: DEFAULT_VIRTUAL_AUDIO_DEVICE_ID.to_owned(),
            label: "Chord Screen Share Audio (Virtual Microphone)".to_owned(),
            kind: AudioDeviceKind::Input,
            is_default: false,
        },
        AudioDevice {
            id: "spk-default-alsa".to_owned(),
            label: "Built-in Analog Stereo Output".to_owned(),
            kind: AudioDeviceKind::Output,
            is_default: true,
        },
    ]
}

// ============================================================================
// Dimension 1: Positive Path — Virtual Audio Track Injection, Exact deviceId
// Matching, AGC Policy, and Resolution Bounds Negotiation
// ============================================================================

#[test]
fn test_r5_08_dim1_negotiate_virtual_audio_injection_exact_device_id_agc_and_resolution_bounds() {
    let mut engine = MediaConstraintsEngine::with_devices(sample_audio_devices());
    assert_eq!(
        engine.lifecycle_state(),
        MediaConstraintsLifecycleState::Idle
    );

    // 1. Register and inject virtual desktop loopback audio track (PipeWire / WASAPI 48 kHz stereo)
    let virtual_track = VirtualAudioTrackDescriptor::new_pipewire_default();
    assert_eq!(virtual_track.device_id, DEFAULT_VIRTUAL_AUDIO_DEVICE_ID);
    assert_eq!(virtual_track.channels, DEFAULT_AUDIO_CHANNELS);
    assert_eq!(virtual_track.sample_rate_hz, DEFAULT_AUDIO_SAMPLE_RATE_HZ);
    assert_eq!(
        virtual_track.source_kind,
        VirtualAudioSourceKind::PipeWireLoopback
    );

    // 2. Negotiate screen share stream with virtual audio track injection + 1080p@60 resolution bounds
    let bounds = ResolutionBounds::from_preset(ResolutionPreset::P1080, 60);
    let negotiated: NegotiatedStreamConstraints = engine
        .negotiate_screen_share_constraints(Some(virtual_track.clone()), bounds)
        .expect("screen share constraint negotiation must succeed");

    assert_eq!(
        engine.lifecycle_state(),
        MediaConstraintsLifecycleState::Active
    );

    // Verify virtual audio track injection & exact deviceId matching
    let audio_constraint: &AudioTrackConstraint = negotiated
        .audio
        .as_ref()
        .expect("virtual audio track must be injected into negotiated stream");
    assert_eq!(
        audio_constraint.role,
        AudioTrackRole::ScreenShareSystemAudio
    );
    assert_eq!(
        audio_constraint.device_id_constraint,
        DeviceIdMatchConstraint::Exact(DEFAULT_VIRTUAL_AUDIO_DEVICE_ID.to_owned())
    );
    assert!(audio_constraint.is_virtual_injected);
    assert_eq!(audio_constraint.channels, 2);
    assert_eq!(audio_constraint.sample_rate_hz, 48_000);

    // Verify AGC policy for screen-share audio: AGC, AEC, and NS MUST be strictly disabled
    // so desktop music/game audio is never ducked or distorted by WebRTC voice processing.
    assert_eq!(audio_constraint.agc_policy.mode, AgcMode::BypassedForStream);
    assert!(!audio_constraint.agc_policy.auto_gain_control);
    assert!(!audio_constraint.agc_policy.echo_cancellation);
    assert!(!audio_constraint.agc_policy.noise_suppression);
    assert!(!audio_constraint.agc_policy.typing_noise_detection);

    // Verify video resolution bounds
    let video_constraint: &VideoResolutionConstraint = &negotiated.video;
    assert_eq!(video_constraint.bounds.max_width, 1920);
    assert_eq!(video_constraint.bounds.max_height, 1080);
    assert_eq!(video_constraint.bounds.target_width, 1920);
    assert_eq!(video_constraint.bounds.target_height, 1080);
    assert_eq!(video_constraint.bounds.max_fps, 60);

    // Verify wire JSON representation (`deviceId: { exact: ... }`, `autoGainControl: false`, `width: { min, ideal, max }`)
    let wire_json = negotiated.to_webrtc_constraints_json();
    assert!(wire_json.contains("\"deviceId\":{\"exact\":\"chord-screen-share-audio\"}"));
    assert!(wire_json.contains("\"autoGainControl\":false"));
    assert!(wire_json.contains("\"echoCancellation\":false"));
    assert!(wire_json.contains("\"noiseSuppression\":false"));
    assert!(wire_json.contains("\"max\":1920"));
    assert!(wire_json.contains("\"max\":1080"));

    // 3. Negotiate standard voice microphone track with exact `deviceId` matching and AGC policy
    let voice_audio = engine
        .negotiate_voice_input_constraints(
            "mic-usb-shure",
            AgcPolicyConfig::voice_adaptive_default(),
        )
        .expect("voice mic constraint negotiation must succeed");
    assert_eq!(voice_audio.role, AudioTrackRole::VoiceMicrophone);
    assert_eq!(
        voice_audio.device_id_constraint,
        DeviceIdMatchConstraint::Exact("mic-usb-shure".to_owned())
    );
    assert!(!voice_audio.is_virtual_injected);
    assert_eq!(voice_audio.agc_policy.mode, AgcMode::AdaptiveDigital);
    assert!(voice_audio.agc_policy.auto_gain_control);
    assert!(voice_audio.agc_policy.echo_cancellation);
    assert!(voice_audio.agc_policy.noise_suppression);
}

// ============================================================================
// Dimension 2: Negative Path — Invalid Device IDs, Wrong Device Kind,
// Out-of-Range or Inverted Resolution Bounds, and Invalid Audio Parameters
// ============================================================================

#[test]
fn test_r5_08_dim2_rejects_invalid_device_ids_wrong_kind_and_bad_resolution_bounds() {
    let mut engine = MediaConstraintsEngine::with_devices(sample_audio_devices());

    // 1. Empty deviceId rejected
    assert_eq!(
        engine.negotiate_voice_input_constraints("", AgcPolicyConfig::voice_adaptive_default()),
        Err(MediaConstraintError::EmptyDeviceId)
    );

    // 2. Output device selected as Input microphone rejected with WrongDeviceKind
    assert_eq!(
        engine.negotiate_voice_input_constraints(
            "spk-default-alsa",
            AgcPolicyConfig::voice_adaptive_default()
        ),
        Err(MediaConstraintError::WrongDeviceKind {
            device_id: "spk-default-alsa".to_owned(),
            expected: AudioDeviceKind::Input,
            actual: AudioDeviceKind::Output,
        })
    );

    // 3. Unknown device under strict exact matching rejected
    assert_eq!(
        engine.negotiate_voice_input_constraints(
            "mic-nonexistent-999",
            AgcPolicyConfig::voice_adaptive_default()
        ),
        Err(MediaConstraintError::ExactDeviceNotFound {
            device_id: "mic-nonexistent-999".to_owned(),
        })
    );

    // 4. Resolution bounds below minimum (160x90) or above maximum (3840x2160) rejected
    let too_small = ResolutionBounds {
        min_width: 120,
        min_height: 60,
        target_width: 120,
        target_height: 60,
        max_width: 120,
        max_height: 60,
        min_fps: 15,
        target_fps: 30,
        max_fps: 30,
        strict_lock: false,
    };
    assert_eq!(
        engine.negotiate_screen_share_constraints(None, too_small),
        Err(MediaConstraintError::ResolutionOutOfRange {
            width: 120,
            height: 60,
            min_width: MIN_VIDEO_WIDTH,
            min_height: MIN_VIDEO_HEIGHT,
            max_width: MAX_VIDEO_WIDTH,
            max_height: MAX_VIDEO_HEIGHT,
        })
    );

    let too_large = ResolutionBounds {
        min_width: 1280,
        min_height: 720,
        target_width: 7680,
        target_height: 4320,
        max_width: 7680,
        max_height: 4320,
        min_fps: 15,
        target_fps: 60,
        max_fps: 60,
        strict_lock: false,
    };
    assert_eq!(
        engine.negotiate_screen_share_constraints(None, too_large),
        Err(MediaConstraintError::ResolutionOutOfRange {
            width: 7680,
            height: 4320,
            min_width: MIN_VIDEO_WIDTH,
            min_height: MIN_VIDEO_HEIGHT,
            max_width: MAX_VIDEO_WIDTH,
            max_height: MAX_VIDEO_HEIGHT,
        })
    );

    // 5. Inverted min > max bounds rejected
    let inverted = ResolutionBounds {
        min_width: 1920,
        min_height: 1080,
        target_width: 1280,
        target_height: 720,
        max_width: 1280,
        max_height: 720,
        min_fps: 15,
        target_fps: 30,
        max_fps: 60,
        strict_lock: false,
    };
    assert_eq!(
        engine.negotiate_screen_share_constraints(None, inverted),
        Err(MediaConstraintError::InvertedResolutionBounds {
            min_width: 1920,
            min_height: 1080,
            max_width: 1280,
            max_height: 720,
        })
    );

    // 6. Odd dimensions rejected (YUV420 chroma subsampling requires even width & height)
    let odd_dims = ResolutionBounds {
        min_width: 640,
        min_height: 360,
        target_width: 1281,
        target_height: 721,
        max_width: 1920,
        max_height: 1080,
        min_fps: 15,
        target_fps: 30,
        max_fps: 60,
        strict_lock: false,
    };
    assert_eq!(
        engine.negotiate_screen_share_constraints(None, odd_dims),
        Err(MediaConstraintError::OddResolutionDimension {
            width: 1281,
            height: 721,
        })
    );

    // 7. Invalid frame rate (0 FPS or > 120 FPS) rejected
    let bad_fps = ResolutionBounds {
        min_width: 640,
        min_height: 360,
        target_width: 1280,
        target_height: 720,
        max_width: 1280,
        max_height: 720,
        min_fps: 0,
        target_fps: 0,
        max_fps: 0,
        strict_lock: false,
    };
    assert_eq!(
        engine.negotiate_screen_share_constraints(None, bad_fps),
        Err(MediaConstraintError::FpsOutOfRange {
            fps: 0,
            min_fps: MIN_STREAM_FPS,
            max_fps: MAX_STREAM_FPS,
        })
    );

    // 8. Invalid virtual audio track format (non-48kHz or 0 channels) rejected
    let bad_virtual = VirtualAudioTrackDescriptor {
        track_id: "vtrack-bad".to_owned(),
        device_id: DEFAULT_VIRTUAL_AUDIO_DEVICE_ID.to_owned(),
        label: "Bad Rate Virtual Mic".to_owned(),
        source_kind: VirtualAudioSourceKind::PipeWireLoopback,
        channels: 2,
        sample_rate_hz: 44_100,
    };
    assert_eq!(
        engine.negotiate_screen_share_constraints(
            Some(bad_virtual),
            ResolutionBounds::from_preset(ResolutionPreset::P720, 30),
        ),
        Err(MediaConstraintError::UnsupportedAudioFormat {
            channels: 2,
            sample_rate_hz: 44_100,
        })
    );
}

// ============================================================================
// Dimension 3: Boundary Conditions — Video Encoder Strict Adherence to
// Selected Resolution Bounds, Aspect Ratio Downscaling & Even Alignment
// ============================================================================

#[test]
fn test_r5_08_dim3_video_encoder_strictly_adheres_to_selected_resolution_bounds() {
    let mut engine = MediaConstraintsEngine::with_devices(sample_audio_devices());

    // Select 720p (1280x720) @ 30 FPS bounds
    let bounds_720p = ResolutionBounds::from_preset(ResolutionPreset::P720, 30);
    assert_eq!(bounds_720p.max_width, 1280);
    assert_eq!(bounds_720p.max_height, 720);
    assert_eq!(bounds_720p.max_fps, 30);

    engine
        .negotiate_screen_share_constraints(None, bounds_720p)
        .expect("negotiate 720p30");

    // Case A: Exact boundary match (1280x720 @ 30 FPS) -> PassThrough unchanged
    let exact_frame = VideoEncoderFrameInput {
        width: 1280,
        height: 720,
        fps: 30,
        timestamp_us: 33_333,
    };
    let decision_exact = engine
        .enforce_video_encoder_bounds(exact_frame)
        .expect("enforce 1280x720");
    assert_eq!(
        decision_exact,
        EncoderFrameDecision::PassThrough {
            width: 1280,
            height: 720,
            fps: 30,
        }
    );

    // Case B: 4K monitor capture (3840x2160 @ 60 FPS) fed into 720p@30 encoder ->
    // MUST be strictly clamped down to 1280x720 @ 30 FPS preserving 16:9 aspect ratio!
    let oversized_4k = VideoEncoderFrameInput {
        width: 3840,
        height: 2160,
        fps: 60,
        timestamp_us: 66_666,
    };
    let decision_4k = engine
        .enforce_video_encoder_bounds(oversized_4k)
        .expect("enforce 4K -> 720p");
    assert_eq!(
        decision_4k,
        EncoderFrameDecision::ScaledToBounds {
            original_width: 3840,
            original_height: 2160,
            encoded_width: 1280,
            encoded_height: 720,
            encoded_fps: 30,
        }
    );

    // Case C: Ultrawide 21:9 capture (3440x1440 @ 60 FPS) fed into 720p (max 1280x720) ->
    // Aspect ratio preserved within 1280x720 bounding box and rounded down to even YUV420 pixels:
    // 1280 / 3440 * 1440 = 535.81 -> even aligned 534 (1280x534 <= 1280x720).
    let ultrawide = VideoEncoderFrameInput {
        width: 3440,
        height: 1440,
        fps: 60,
        timestamp_us: 100_000,
    };
    let decision_ultrawide = engine
        .enforce_video_encoder_bounds(ultrawide)
        .expect("enforce ultrawide -> 720p bounds");
    match decision_ultrawide {
        EncoderFrameDecision::ScaledToBounds {
            encoded_width,
            encoded_height,
            encoded_fps,
            ..
        } => {
            assert_eq!(encoded_width, 1280);
            assert!(encoded_height <= 720);
            assert_eq!(encoded_width % 2, 0, "width must be even for YUV420");
            assert_eq!(encoded_height % 2, 0, "height must be even for YUV420");
            assert_eq!(encoded_height, 534);
            assert_eq!(encoded_fps, 30);
        }
        other => panic!("expected ScaledToBounds for ultrawide input, got {other:?}"),
    }

    // Case D: Strict lock mode (`strict_lock = true` at 1920x1080) locks both min and max to 1920x1080
    let strict_1080p = ResolutionBounds::strict_fixed(1920, 1080, 60);
    assert_eq!(strict_1080p.min_width, 1920);
    assert_eq!(strict_1080p.max_width, 1920);
    assert_eq!(strict_1080p.min_height, 1080);
    assert_eq!(strict_1080p.max_height, 1080);
    engine
        .renegotiate_video_bounds(strict_1080p)
        .expect("renegotiate strict 1080p");

    let sub_1080_input = VideoEncoderFrameInput {
        width: 1280,
        height: 720,
        fps: 60,
        timestamp_us: 120_000,
    };
    let strict_decision = engine
        .enforce_video_encoder_bounds(sub_1080_input)
        .expect("strict lock scales up to exact fixed bounds");
    assert_eq!(
        strict_decision,
        EncoderFrameDecision::ScaledToBounds {
            original_width: 1280,
            original_height: 720,
            encoded_width: 1920,
            encoded_height: 1080,
            encoded_fps: 60,
        }
    );

    // Verify all 5 canonical resolution presets (480p, 720p, 1080p, 1440p, 2160p)
    assert_eq!(ResolutionPreset::P480.dimensions(), (854, 480));
    assert_eq!(ResolutionPreset::P720.dimensions(), (1280, 720));
    assert_eq!(ResolutionPreset::P1080.dimensions(), (1920, 1080));
    assert_eq!(ResolutionPreset::P1440.dimensions(), (2560, 1440));
    assert_eq!(ResolutionPreset::P2160.dimensions(), (3840, 2160));
}

// ============================================================================
// Dimension 4: Error Handling — Unnegotiated Engine, Missing Virtual Source,
// Zero-Dimension Frame Rejection
// ============================================================================

#[test]
fn test_r5_08_dim4_error_handling_unnegotiated_state_and_corrupt_frames() {
    let mut engine = MediaConstraintsEngine::with_devices(sample_audio_devices());

    // Calling `enforce_video_encoder_bounds` before any negotiation returns `EngineNotActive`
    let frame = VideoEncoderFrameInput {
        width: 1280,
        height: 720,
        fps: 30,
        timestamp_us: 0,
    };
    assert_eq!(
        engine.enforce_video_encoder_bounds(frame),
        Err(MediaConstraintError::EngineNotActive)
    );

    // Calling `renegotiate_video_bounds` before initial negotiation returns `EngineNotActive`
    assert_eq!(
        engine.renegotiate_video_bounds(ResolutionBounds::from_preset(ResolutionPreset::P720, 30)),
        Err(MediaConstraintError::EngineNotActive)
    );

    // Start active negotiation, then feed a degenerate 0x0 or 0-FPS frame to encoder
    engine
        .negotiate_screen_share_constraints(
            None,
            ResolutionBounds::from_preset(ResolutionPreset::P720, 30),
        )
        .expect("negotiate");

    let zero_frame = VideoEncoderFrameInput {
        width: 0,
        height: 720,
        fps: 30,
        timestamp_us: 10_000,
    };
    assert_eq!(
        engine.enforce_video_encoder_bounds(zero_frame),
        Err(MediaConstraintError::InvalidFrameDimensions {
            width: 0,
            height: 720,
        })
    );
    assert_eq!(engine.telemetry().frames_dropped_out_of_bounds, 1);
}

// ============================================================================
// Dimension 5: Empty States — Empty Device List & Video-Only Screen Share
// ============================================================================

#[test]
fn test_r5_08_dim5_empty_device_list_and_video_only_constraints() {
    let mut empty_engine = MediaConstraintsEngine::new();
    assert!(empty_engine.available_devices().is_empty());

    // Voice input negotiation on an empty device list returns `NoInputDevicesAvailable`
    assert_eq!(
        empty_engine
            .negotiate_voice_input_constraints("mic-any", AgcPolicyConfig::voice_adaptive_default()),
        Err(MediaConstraintError::NoInputDevicesAvailable)
    );

    // Screen share without audio (`virtual_audio = None`) succeeds cleanly even when no audio devices exist
    let negotiated = empty_engine
        .negotiate_screen_share_constraints(
            None,
            ResolutionBounds::from_preset(ResolutionPreset::P720, 30),
        )
        .expect("video-only screen share negotiation must succeed with empty audio device list");
    assert!(negotiated.audio.is_none());
    assert_eq!(negotiated.video.bounds.max_width, 1280);
    assert_eq!(negotiated.video.bounds.max_height, 720);
}

// ============================================================================
// Dimension 6: Intermediate / Lifecycle States — State Transitions &
// Dynamic Mid-Stream Renegotiation
// ============================================================================

#[test]
fn test_r5_08_dim6_lifecycle_transitions_and_mid_stream_renegotiation() {
    let mut engine = MediaConstraintsEngine::with_devices(sample_audio_devices());
    assert_eq!(
        engine.lifecycle_state(),
        MediaConstraintsLifecycleState::Idle
    );

    // Idle -> Active via initial negotiation (720p@30, video-only)
    engine
        .negotiate_screen_share_constraints(
            None,
            ResolutionBounds::from_preset(ResolutionPreset::P720, 30),
        )
        .expect("initial negotiation");
    assert_eq!(
        engine.lifecycle_state(),
        MediaConstraintsLifecycleState::Active
    );

    // Mid-stream: dynamically inject virtual audio track without dropping video bounds
    let virtual_wasapi = VirtualAudioTrackDescriptor::new_wasapi_default();
    let updated = engine
        .inject_virtual_audio_track(virtual_wasapi)
        .expect("dynamic virtual audio injection must succeed");
    assert!(updated.is_virtual_injected);
    assert_eq!(
        updated.device_id_constraint,
        DeviceIdMatchConstraint::Exact("chord-wasapi-loopback-audio".to_owned())
    );

    // Mid-stream: upgrade resolution bounds from 720p@30 to 1440p@60
    let upgraded = engine
        .renegotiate_video_bounds(ResolutionBounds::from_preset(ResolutionPreset::P1440, 60))
        .expect("renegotiate to 1440p60");
    assert_eq!(upgraded.video.bounds.max_width, 2560);
    assert_eq!(upgraded.video.bounds.max_height, 1440);
    assert_eq!(upgraded.video.bounds.max_fps, 60);
    assert!(
        upgraded.audio.is_some(),
        "virtual audio track must persist across video bounds renegotiation"
    );

    // Active -> Stopped
    engine.stop();
    assert_eq!(
        engine.lifecycle_state(),
        MediaConstraintsLifecycleState::Stopped
    );
    assert!(engine.active_stream_constraints().is_none());
}

// ============================================================================
// Dimension 7: Capabilities & Privacy — Debug Redaction of Device Labels,
// AGC / Studio Mode Policy, and Voice OP 5 SOUNDSHARE Speaking Flag
// ============================================================================

#[test]
fn test_r5_08_dim7_privacy_redaction_agc_policies_and_soundshare_flag() {
    let vtrack = VirtualAudioTrackDescriptor {
        track_id: "track-pw-01".to_owned(),
        device_id: DEFAULT_VIRTUAL_AUDIO_DEVICE_ID.to_owned(),
        label: "Sensitive Host PipeWire Monitor Label".to_owned(),
        source_kind: VirtualAudioSourceKind::PipeWireLoopback,
        channels: 2,
        sample_rate_hz: 48_000,
    };

    // Debug output MUST redact the human-readable `label` field (`<redacted>`)
    let debug_str = format!("{vtrack:?}");
    assert!(
        !debug_str.contains("Sensitive Host PipeWire Monitor Label"),
        "VirtualAudioTrackDescriptor Debug must redact human-readable OS label: {debug_str}"
    );
    assert!(debug_str.contains("<redacted>"));

    // Verify all 3 AGC policy profiles:
    // 1. Voice Adaptive Digital (default for mic: AGC on, AEC on, NS on, Krisp VAD 0.8)
    let voice_agc = AgcPolicyConfig::voice_adaptive_default();
    assert_eq!(voice_agc.mode, AgcMode::AdaptiveDigital);
    assert!(voice_agc.auto_gain_control);
    assert!(voice_agc.echo_cancellation);
    assert!(voice_agc.noise_suppression);
    assert_eq!(voice_agc.target_level_dbfs, -3);
    assert_eq!(voice_agc.compression_gain_db, 9);
    assert!(voice_agc.limiter_enabled);

    // 2. Voice Fixed Digital (manual gain for hardware interfaces: AGC off, AEC on, NS on)
    let fixed_agc = AgcPolicyConfig::voice_fixed_manual();
    assert_eq!(fixed_agc.mode, AgcMode::FixedDigital);
    assert!(!fixed_agc.auto_gain_control);
    assert!(fixed_agc.echo_cancellation);
    assert!(fixed_agc.noise_suppression);

    // 3. Screen Share / Studio Bypass (for virtual audio track: AGC off, AEC off, NS off)
    let stream_agc = AgcPolicyConfig::screen_share_passthrough();
    assert_eq!(stream_agc.mode, AgcMode::BypassedForStream);
    assert!(!stream_agc.auto_gain_control);
    assert!(!stream_agc.echo_cancellation);
    assert!(!stream_agc.noise_suppression);

    // Verify Voice Gateway OP 5 SPEAKING flag (`speaking == 2` for SOUNDSHARE when virtual audio is active)
    let mut engine = MediaConstraintsEngine::with_devices(sample_audio_devices());
    let negotiated = engine
        .negotiate_screen_share_constraints(
            Some(vtrack),
            ResolutionBounds::from_preset(ResolutionPreset::P1080, 60),
        )
        .expect("negotiate with virtual audio");
    assert_eq!(negotiated.speaking_flags(), 2);
}

// ============================================================================
// Dimension 8: Recovery & Degradation — Audio Track Stability Across Physical
// Device Changes (Hot-Plug, Default Switch, Unplug & Reconnect)
// ============================================================================

#[test]
fn test_r5_08_dim8_audio_track_remains_stable_through_device_changes() {
    let mut engine = MediaConstraintsEngine::with_devices(sample_audio_devices());

    // Pin voice microphone to exact device `mic-usb-shure` AND inject virtual screen-share audio track
    let _voice = engine
        .negotiate_voice_input_constraints(
            "mic-usb-shure",
            AgcPolicyConfig::voice_adaptive_default(),
        )
        .expect("pin voice mic to mic-usb-shure");
    let vtrack = VirtualAudioTrackDescriptor::new_pipewire_default();
    let _stream = engine
        .negotiate_screen_share_constraints(
            Some(vtrack),
            ResolutionBounds::from_preset(ResolutionPreset::P1080, 60),
        )
        .expect("negotiate screen share with virtual audio");

    let initial_voice_generation = engine
        .active_voice_input_constraint()
        .expect("active voice")
        .track_generation;
    let initial_stream_generation = engine
        .active_stream_constraints()
        .and_then(|s| s.audio.as_ref())
        .expect("active stream audio")
        .track_generation;

    // Event 1: Hot-plug a Bluetooth headset and change the OS default microphone to Bluetooth.
    // Because `mic-usb-shure` is still connected and matched via exact `deviceId`, and the virtual
    // screen-share track is injected, NEITHER audio track may be interrupted or re-bound!
    let mut hotplug_devices = sample_audio_devices();
    hotplug_devices[0].is_default = false; // Built-in mic no longer default
    hotplug_devices.push(AudioDevice {
        id: "mic-bt-airpods".to_owned(),
        label: "Bluetooth Headset Mic".to_owned(),
        kind: AudioDeviceKind::Input,
        is_default: true, // OS switches default to Bluetooth!
    });

    let stability_report_1 = engine.handle_device_Topology_change(hotplug_devices);
    assert!(
        stability_report_1.voice_track_unchanged,
        "exact deviceId match must prevent OS default-device hijack on hot-plug"
    );
    assert!(
        stability_report_1.virtual_stream_track_unchanged,
        "virtual screen-share audio track must remain 100% stable across physical device changes"
    );
    assert_eq!(
        engine
            .active_voice_input_constraint()
            .unwrap()
            .effective_device_id(),
        "mic-usb-shure"
    );
    assert_eq!(
        engine
            .active_voice_input_constraint()
            .unwrap()
            .track_generation,
        initial_voice_generation
    );
    assert_eq!(
        engine
            .active_stream_constraints()
            .and_then(|s| s.audio.as_ref())
            .unwrap()
            .track_generation,
        initial_stream_generation
    );

    // Event 2: Physical unplug of `mic-usb-shure` while streaming!
    // - The virtual screen-share audio track MUST remain completely stable and untouched!
    // - The voice microphone track gracefully falls back to the system default (`mic-default-alsa`)
    //   while remembering the user's preferred exact device (`mic-usb-shure`) for auto-restoration.
    let unplugged_devices = vec![
        AudioDevice {
            id: "mic-default-alsa".to_owned(),
            label: "Built-in Analog Microphone".to_owned(),
            kind: AudioDeviceKind::Input,
            is_default: true,
        },
        AudioDevice {
            id: "spk-default-alsa".to_owned(),
            label: "Built-in Analog Stereo Output".to_owned(),
            kind: AudioDeviceKind::Output,
            is_default: true,
        },
    ];
    let stability_report_2 = engine.handle_device_Topology_change(unplugged_devices);
    assert!(
        stability_report_2.virtual_stream_track_unchanged,
        "virtual audio track must remain stable even when physical devices are unplugged"
    );
    assert!(!stability_report_2.voice_track_unchanged);
    assert_eq!(
        stability_report_2.voice_fallback_device_id.as_deref(),
        Some("mic-default-alsa")
    );
    assert_eq!(
        engine
            .active_voice_input_constraint()
            .unwrap()
            .preferred_exact_device_id(),
        "mic-usb-shure"
    );
    assert_eq!(
        engine
            .active_voice_input_constraint()
            .unwrap()
            .effective_device_id(),
        "mic-default-alsa"
    );

    // Event 3: `mic-usb-shure` is plugged back in -> voice track automatically restores exact match!
    let restored_devices = sample_audio_devices();
    let stability_report_3 = engine.handle_device_Topology_change(restored_devices);
    assert!(stability_report_3.voice_restored_preferred_exact);
    assert!(stability_report_3.virtual_stream_track_unchanged);
    assert_eq!(
        engine
            .active_voice_input_constraint()
            .unwrap()
            .effective_device_id(),
        "mic-usb-shure"
    );
}

// ============================================================================
// Dimension 9: Observable Telemetry — Counters for Negotiations, Virtual Track
// Injections, Exact Matches, AGC Bypass, Frame Clamping, and Device Stability
// ============================================================================

#[test]
fn test_r5_08_dim9_observable_telemetry_counters() {
    let mut engine = MediaConstraintsEngine::with_devices(sample_audio_devices());

    // 1. Negotiate voice input + screen share with virtual audio track
    engine
        .negotiate_voice_input_constraints(
            "mic-usb-shure",
            AgcPolicyConfig::voice_adaptive_default(),
        )
        .expect("voice negotiation");
    engine
        .negotiate_screen_share_constraints(
            Some(VirtualAudioTrackDescriptor::new_pipewire_default()),
            ResolutionBounds::from_preset(ResolutionPreset::P720, 30),
        )
        .expect("stream negotiation");

    // 2. Pass 1 exact frame and 1 oversized frame through encoder bounds check
    engine
        .enforce_video_encoder_bounds(VideoEncoderFrameInput {
            width: 1280,
            height: 720,
            fps: 30,
            timestamp_us: 10_000,
        })
        .expect("pass frame");
    engine
        .enforce_video_encoder_bounds(VideoEncoderFrameInput {
            width: 1920,
            height: 1080,
            fps: 60,
            timestamp_us: 20_000,
        })
        .expect("clamped frame");

    // 3. Trigger a hot-plug event that preserves both tracks
    engine.handle_device_Topology_change(sample_audio_devices());

    let telemetry: &MediaConstraintsTelemetry = engine.telemetry();
    assert_eq!(telemetry.negotiations_total, 2);
    assert_eq!(telemetry.virtual_tracks_injected, 1);
    assert_eq!(telemetry.exact_device_matches, 2);
    assert_eq!(telemetry.agc_bypassed_tracks, 1);
    assert_eq!(telemetry.frames_passed_within_bounds, 1);
    assert_eq!(telemetry.frames_clamped_to_bounds, 1);
    assert_eq!(telemetry.device_change_events_handled, 1);
    assert_eq!(telemetry.track_stability_preserved_count, 1);
}
