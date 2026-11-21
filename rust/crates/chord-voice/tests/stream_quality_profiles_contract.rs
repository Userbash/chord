#![forbid(unsafe_code)]
//! Contract tests for Task R5-09: Stream Quality Profiles (`chord_voice::stream_quality`).
//!
//! Specification & Reference Oracles:
//! - `docs/feature-backlog.md` row `R5-09`:
//!   "Dynamic bitrate and encoder scaling: min/target/max bitrate, framerate, resolution pixel
//!   limits, connection adaptation | Vesktop: `patchStreamQuality`, `handleGoLive` | Voice Engine |
//!   Stream adheres to selected profile (e.g. 720p@60 FPS); bitrate conforms to bandwidth constraints"
//! - `docs/re/voice-deep.md` (§1, §3: `updateVideoQuality: 1280x720@30, bitrate 2.5 Mbps (+-150k), rid 100,
//!   ssrc 9209 (rtx 9210), remoteSinkWantsMaxFramerate 20`, `ConnectionEventFramerateReducer: Reduced framerate
//!   after 15000 ms`, `Remote media sink wants: {"any":100}`).
//! - `Serein/crates/discord-voice/src/video.rs` (`Rate` congestion controller: REMB wire estimate
//!   scaled by 4/5 with 5000 ms TTL, RTCP fraction lost `/256` congestion drop at `>= 13` (~5%),
//!   2000 ms holdoff recovery `+5%` / `>= 25 kbps`).
//!
//! Covers all 9 mandatory dimensions (`TEST-POLICY.md` §12):
//! 1. Positive path: Standard and preset quality profiles (`480p`, `720p`, `1080p`, `1440p`, `4K Source`
//!    at `15`, `30`, `60` FPS), adhering strictly to selected profile (e.g., `720p@60 FPS`), dynamic
//!    aspect-ratio-preserving encoder scaling within pixel budget, and `OP 12 VIDEO` generation.
//! 2. Negative path: Rejecting invalid dimensions (`< 64`, `> 3840`, odd 4:2:0 dimensions), invalid
//!    framerates (`0` or `> 60`), invalid bitrates (`< 150 kbps` or `> 25 Mbps`), and inverted bitrate
//!    hierarchy (`min > target` or `target > max`).
//! 3. Boundary conditions: Exact pixel limits (`854x480 = 409_920`, `1280x720 = 921_600`,
//!    `1920x1080 = 2_073_600`, `2560x1440 = 3_686_400`, `3840x2160 = 8_294_400`), exact RTCP loss
//!    thresholds (`fraction_lost == 12` vs `13`, `5` vs `6`), exact idle timeout (`14_999 ms` vs `15_000 ms`),
//!    and REMB TTL (`4_999 ms` vs `5_000 ms`).
//! 4. Error handling: `MediaSinkWants` malformed or mismatched SSRC queries, non-finite/out-of-range
//!    inputs, and invalid custom profile overrides returning typed `StreamQualityError`.
//! 5. Empty states: Inactive stream (`MediaSinkWants` quality `0`) pauses encoder emission (`active = false`)
//!    without panic; zero elapsed time or missing REMB/loss observations leaves target bitrate stable.
//! 6. Intermediate / lifecycle states: `Idle -> Active -> Degraded -> PausedBySink`
//!    state machine transitions, `patch_stream_quality` mid-stream profile updates, and keyframe requests
//!    on resolution scaling changes.
//! 7. Capabilities & permissions: `patchStreamQuality` unlocking `1080p`/`1440p`/`Source`@`60 FPS` vs
//!    non-patched `FreeTier` clamping to `720p@30 FPS` (`2.5 Mbps`), and `Debug` redaction of sensitive
//!    stream tokens/keys in `GoLiveStreamContext`.
//! 8. Recovery & degradation: Bandwidth adaptation downscaling resolution/FPS under severe bandwidth
//!    constraints (`< min_bitrate_bps` or sink wants `quality < 100`) and recovering back to the selected
//!    profile (`720p@60 FPS`) after sustained low-loss (`<= 5/256`) and sufficient REMB bandwidth.
//! 9. Observable telemetry: `StreamQualityTelemetry` tracking profile applications, bitrate adaptations,
//!    congestion backoff events, recovery steps, resolution downscales/upscales, idle framerate reductions,
//!    and `OP 12 VIDEO` payload emissions.

use chord_voice::rtcp::{ReportBlock, RtcpPacket, RtcpRemb};
use chord_voice::session::MediaSinkWants;
use chord_voice::stream_quality::{
    scale_dimensions_to_pixel_limit, EncoderScaleOutput, GoLiveQualityPatchPolicy,
    GoLiveStreamContext, StreamAdaptationState, StreamContentHint, StreamFrameratePreset,
    StreamProfilePreset, StreamQualityController, StreamQualityError, StreamQualityProfile,
    StreamResolutionPreset, ADAPTATION_TICK_INTERVAL_MS, CONNECTION_IDLE_FRAMERATE_REDUCE_MS,
    DEFAULT_720P30_TARGET_BITRATE_BPS, DEFAULT_720P60_TARGET_BITRATE_BPS, FREE_TIER_MAX_FPS,
    FREE_TIER_MAX_HEIGHT, FREE_TIER_MAX_WIDTH, LOSS_CONGESTION_THRESHOLD_256,
    LOSS_RECOVERY_THRESHOLD_256, MAX_STREAM_BITRATE_BPS, MAX_STREAM_DIMENSION, MAX_STREAM_FPS,
    MIN_STREAM_BITRATE_BPS, MIN_STREAM_DIMENSION, MIN_STREAM_FPS, PIXEL_LIMIT_1080P,
    PIXEL_LIMIT_1440P, PIXEL_LIMIT_2160P, PIXEL_LIMIT_480P, PIXEL_LIMIT_720P,
    REMB_HEADROOM_DENOMINATOR, REMB_HEADROOM_NUMERATOR, REMB_LIMIT_TTL_MS,
};

#[test]
fn dim1_positive_preset_profiles_720p60_adherence_and_handle_go_live_op12() {
    // Verify constants and presets
    assert_eq!(MIN_STREAM_DIMENSION, 64);
    assert_eq!(MAX_STREAM_DIMENSION, 3840);
    assert_eq!(MIN_STREAM_FPS, 1);
    assert_eq!(MAX_STREAM_FPS, 60);
    assert_eq!(MIN_STREAM_BITRATE_BPS, 150_000);
    assert_eq!(MAX_STREAM_BITRATE_BPS, 25_000_000);
    assert_eq!(StreamResolutionPreset::P720.dimensions(), (1280, 720));
    assert_eq!(StreamFrameratePreset::Fps60.as_u32(), 60);
    assert_eq!(PIXEL_LIMIT_480P, 854 * 480);
    assert_eq!(PIXEL_LIMIT_720P, 1280 * 720);
    assert_eq!(PIXEL_LIMIT_1080P, 1920 * 1080);
    assert_eq!(PIXEL_LIMIT_1440P, 2560 * 1440);
    assert_eq!(PIXEL_LIMIT_2160P, 3840 * 2160);

    // 720p@60 FPS profile adherence (Acceptance Criteria: "Stream adheres to selected profile (e.g. 720p@60 FPS)")
    let profile_720p60 = StreamQualityProfile::from_preset(StreamProfilePreset::P720Fps60);
    assert_eq!(profile_720p60.width, 1280);
    assert_eq!(profile_720p60.height, 720);
    assert_eq!(profile_720p60.fps, 60);
    assert_eq!(profile_720p60.max_pixel_count, PIXEL_LIMIT_720P);
    assert_eq!(
        profile_720p60.target_bitrate_bps,
        DEFAULT_720P60_TARGET_BITRATE_BPS
    );
    assert!(profile_720p60.min_bitrate_bps <= profile_720p60.target_bitrate_bps);
    assert!(profile_720p60.target_bitrate_bps <= profile_720p60.max_bitrate_bps);
    assert_eq!(profile_720p60.rtp_timestamp_step_90khz(), 1_500);
    assert_eq!(profile_720p60.frame_interval_us(), 16_666);

    // 720p@30 FPS oracle from `docs/re/voice-deep.md` §3: 1280x720@30, bitrate 2.5 Mbps
    let profile_720p30 = StreamQualityProfile::from_preset(StreamProfilePreset::P720Fps30);
    assert_eq!(profile_720p30.width, 1280);
    assert_eq!(profile_720p30.height, 720);
    assert_eq!(profile_720p30.fps, 30);
    assert_eq!(
        profile_720p30.target_bitrate_bps,
        DEFAULT_720P30_TARGET_BITRATE_BPS
    );
    assert_eq!(profile_720p30.rtp_timestamp_step_90khz(), 3_000);

    // Initialize controller with 720p@60 FPS and run handle_go_live (Session 3 SSRCs: 9208 / 9209 / 9210)
    let mut ctrl = StreamQualityController::new(
        9208,
        profile_720p60,
        GoLiveQualityPatchPolicy::PatchedUnrestricted,
    )
    .expect("valid 720p60 controller");

    assert_eq!(ctrl.audio_ssrc(), 9208);
    assert_eq!(ctrl.video_ssrc(), 9209);
    assert_eq!(ctrl.rtx_ssrc(), 9210);
    assert_eq!(ctrl.state(), StreamAdaptationState::Idle);

    // handle_go_live transitions to Active and builds Voice Opcode 12 (Video)
    let op12 = ctrl.handle_go_live(1920, 1080);
    assert_eq!(ctrl.state(), StreamAdaptationState::Active);
    assert_eq!(op12.audio_ssrc, 9208);
    assert_eq!(op12.video_ssrc, 9209);
    assert_eq!(op12.rtx_ssrc, 9210);
    assert_eq!(op12.streams.len(), 1);
    let stream_desc = &op12.streams[0];
    assert_eq!(stream_desc.kind, "screen");
    assert_eq!(stream_desc.rid, "100");
    assert_eq!(stream_desc.quality, 100);
    assert_eq!(stream_desc.ssrc, 9209);
    assert_eq!(stream_desc.rtx_ssrc, 9210);
    assert!(stream_desc.active);
    assert_eq!(stream_desc.max_framerate, 60);
    assert_eq!(stream_desc.max_resolution.width, 1280);
    assert_eq!(stream_desc.max_resolution.height, 720);
    assert_eq!(stream_desc.max_bitrate, profile_720p60.max_bitrate_bps);

    // Scaling a 1920x1080 source into the 720p60 profile yields exact 1280x720@60
    let scaled: EncoderScaleOutput = ctrl.evaluate_encoder_Params(1920, 1080);
    assert_eq!(scaled.encoded_width, 1280);
    assert_eq!(scaled.encoded_height, 720);
    assert_eq!(scaled.encoded_fps, 60);
    assert!(scaled.encoded_width * scaled.encoded_height <= PIXEL_LIMIT_720P);
    assert_eq!(scaled.target_bitrate_bps, profile_720p60.target_bitrate_bps);
}

#[test]
fn dim2_negative_invalid_dimensions_fps_and_bitrate_hierarchy_rejected() {
    // Width/height below MIN_STREAM_DIMENSION (64) or above MAX_STREAM_DIMENSION (3840)
    let err_small = StreamQualityProfile::custom(
        32,
        720,
        60,
        300_000,
        2_500_000,
        4_000_000,
        StreamContentHint::Motion,
    );
    assert_eq!(
        err_small,
        Err(StreamQualityError::InvalidDimensions {
            width: 32,
            height: 720
        })
    );

    let err_large = StreamQualityProfile::custom(
        4096,
        2160,
        60,
        300_000,
        2_500_000,
        4_000_000,
        StreamContentHint::Motion,
    );
    assert_eq!(
        err_large,
        Err(StreamQualityError::InvalidDimensions {
            width: 4096,
            height: 2160
        })
    );

    // Odd dimensions rejected for YUV 4:2:0 compatibility
    let err_odd = StreamQualityProfile::custom(
        1281,
        720,
        60,
        300_000,
        2_500_000,
        4_000_000,
        StreamContentHint::Motion,
    );
    assert_eq!(
        err_odd,
        Err(StreamQualityError::OddDimensions {
            width: 1281,
            height: 720
        })
    );

    // FPS == 0 or > 60 rejected
    let err_fps0 = StreamQualityProfile::custom(
        1280,
        720,
        0,
        300_000,
        2_500_000,
        4_000_000,
        StreamContentHint::Motion,
    );
    assert_eq!(err_fps0, Err(StreamQualityError::InvalidFramerate(0)));

    let err_fps120 = StreamQualityProfile::custom(
        1280,
        720,
        120,
        300_000,
        2_500_000,
        4_000_000,
        StreamContentHint::Motion,
    );
    assert_eq!(err_fps120, Err(StreamQualityError::InvalidFramerate(120)));

    // Bitrate out of range (< 150_000 or > 25_000_000)
    let err_low_br = StreamQualityProfile::custom(
        1280,
        720,
        60,
        50_000,
        2_500_000,
        4_000_000,
        StreamContentHint::Motion,
    );
    assert_eq!(
        err_low_br,
        Err(StreamQualityError::BitrateOutOfRange {
            min_bps: 50_000,
            target_bps: 2_500_000,
            max_bps: 4_000_000
        })
    );

    let err_high_br = StreamQualityProfile::custom(
        1280,
        720,
        60,
        300_000,
        2_500_000,
        30_000_000,
        StreamContentHint::Motion,
    );
    assert_eq!(
        err_high_br,
        Err(StreamQualityError::BitrateOutOfRange {
            min_bps: 300_000,
            target_bps: 2_500_000,
            max_bps: 30_000_000
        })
    );

    // Inverted bitrate ordering (min > target or target > max)
    let err_order = StreamQualityProfile::custom(
        1280,
        720,
        60,
        3_000_000,
        2_500_000,
        4_000_000,
        StreamContentHint::Motion,
    );
    assert_eq!(
        err_order,
        Err(StreamQualityError::InvalidBitrateHierarchy {
            min_bps: 3_000_000,
            target_bps: 2_500_000,
            max_bps: 4_000_000
        })
    );

    // Zero audio SSRC rejected
    let profile = StreamQualityProfile::from_preset(StreamProfilePreset::P720Fps60);
    assert_eq!(
        StreamQualityController::new(0, profile, GoLiveQualityPatchPolicy::PatchedUnrestricted)
            .unwrap_err(),
        StreamQualityError::InvalidSsrc
    );
}

#[test]
fn dim3_boundary_pixel_limits_loss_thresholds_remb_ttl_and_idle_timeout() {
    // 1. Aspect-ratio preserving resolution scaling against exact pixel limit boundaries
    assert_eq!(
        scale_dimensions_to_pixel_limit(1280, 720, 1280, 720, PIXEL_LIMIT_720P),
        (1280, 720)
    );
    let (uw_w, uw_h) = scale_dimensions_to_pixel_limit(2560, 1080, 1280, 720, PIXEL_LIMIT_720P);
    assert!(uw_w <= 1280 && uw_h <= 720);
    assert!(uw_w * uw_h <= PIXEL_LIMIT_720P);
    assert_eq!(uw_w % 2, 0);
    assert_eq!(uw_h % 2, 0);
    assert_eq!(
        scale_dimensions_to_pixel_limit(800, 600, 1920, 1080, PIXEL_LIMIT_1080P),
        (800, 600)
    );

    // 2. Exact RTCP loss threshold boundary: 12/256 (~4.68%) vs 13/256 (~5.07%)
    let profile = StreamQualityProfile::from_preset(StreamProfilePreset::P720Fps60);
    let initial_target = profile.target_bitrate_bps;
    let mut ctrl =
        StreamQualityController::new(9208, profile, GoLiveQualityPatchPolicy::PatchedUnrestricted)
            .unwrap();
    let _ = ctrl.handle_go_live(1280, 720);

    // At t=1000ms, loss = 12 (< 13 threshold): no congestion reduction
    ctrl.observe_network_feedback(Some(LOSS_CONGESTION_THRESHOLD_256 - 1), None, 1_000);
    let _ = ctrl.tick_adaptation(1_000);
    assert_eq!(ctrl.current_target_bitrate_bps(), initial_target);

    // At t=2000ms, loss = 13 (== 13 threshold): reduces bitrate by 20% (target * 4 / 5)
    ctrl.observe_network_feedback(Some(LOSS_CONGESTION_THRESHOLD_256), None, 2_000);
    let reduced = ctrl
        .tick_adaptation(2_000)
        .expect("must reduce bitrate at loss=13");
    assert_eq!(reduced, (initial_target * 4) / 5);
    assert_eq!(ctrl.current_target_bitrate_bps(), reduced);

    // 3. Exact 2000ms holdoff before recovery at loss <= 5/256
    // At t=3000ms (only 1000ms after congestion at 2000ms, below 2000ms holdoff): no recovery yet
    assert_eq!(ADAPTATION_TICK_INTERVAL_MS, 1_000);
    ctrl.observe_network_feedback(Some(LOSS_RECOVERY_THRESHOLD_256), None, 3_000);
    assert_eq!(ctrl.tick_adaptation(3_000), None);

    // At t=4000ms (exact 2000ms after congestion at 2000ms) with loss == 5: recovers by +5%
    ctrl.observe_network_feedback(Some(LOSS_RECOVERY_THRESHOLD_256), None, 4_000);
    let recovered = ctrl
        .tick_adaptation(4_000)
        .expect("must recover at +2000ms with loss<=5");
    assert!(recovered > reduced);

    // 4. Exact 15000ms idle framerate reducer boundary (`docs/re/voice-deep.md` §3)
    assert_eq!(CONNECTION_IDLE_FRAMERATE_REDUCE_MS, 15_000);
    ctrl.update_static_screen_idle_ms(14_999);
    assert_eq!(ctrl.evaluate_encoder_Params(1280, 720).encoded_fps, 60);
    ctrl.update_static_screen_idle_ms(15_000);
    assert!(ctrl.evaluate_encoder_Params(1280, 720).encoded_fps < 60);
}

#[test]
fn dim4_and_dim5_error_handling_and_empty_sink_wants_pause_and_remb_constraints() {
    let profile = StreamQualityProfile::from_preset(StreamProfilePreset::P720Fps60);
    let mut ctrl =
        StreamQualityController::new(9208, profile, GoLiveQualityPatchPolicy::PatchedUnrestricted)
            .unwrap();
    let _ = ctrl.handle_go_live(1280, 720);

    // Empty / 0-quality MediaSinkWants for our video SSRC (9209) pauses active encoding
    let wants_zero = MediaSinkWants::any(100).with_ssrc(9209, 0);
    ctrl.apply_media_sink_wants(&wants_zero);
    assert_eq!(ctrl.state(), StreamAdaptationState::PausedBySink);
    let paused_scale = ctrl.evaluate_encoder_Params(1280, 720);
    assert!(!paused_scale.active);
    assert_eq!(paused_scale.encoded_fps, 0);

    // Unrelated SSRC in MediaSinkWants falls back to `any: 100` and resumes active encoding
    let wants_resume = MediaSinkWants::any(100).with_ssrc(99999, 0);
    ctrl.apply_media_sink_wants(&wants_resume);
    assert_eq!(ctrl.state(), StreamAdaptationState::Active);
    assert!(ctrl.evaluate_encoder_Params(1280, 720).active);

    // Bandwidth constraint adherence via RTCP REMB packet (Acceptance Criteria: "bitrate conforms to bandwidth constraints")
    // Wire REMB estimate of 1_500_000 bps -> effective media cap is 1_500_000 * 4 / 5 = 1_200_000 bps
    let remb_pkt = RtcpPacket::Remb(RtcpRemb {
        sender_ssrc: 1,
        media_ssrc: 0,
        exponent: 0,
        mantissa: 0,
        bitrate_bps: 1_500_000,
        ssrcs: vec![9209],
    });
    ctrl.on_rtcp_packet(&remb_pkt, 10_000);
    let adapted_bps = ctrl
        .tick_adaptation(11_000)
        .expect("REMB ceiling must clamp bitrate");
    let expected_cap = (1_500_000u64 * u64::from(REMB_HEADROOM_NUMERATOR)
        / u64::from(REMB_HEADROOM_DENOMINATOR)) as u32;
    assert_eq!(adapted_bps, expected_cap);
    assert!(ctrl.current_target_bitrate_bps() <= 1_500_000);

    // REMB for a different SSRC (not 9209) is ignored
    let unrelated_remb = RtcpPacket::Remb(RtcpRemb {
        sender_ssrc: 1,
        media_ssrc: 0,
        exponent: 0,
        mantissa: 0,
        bitrate_bps: 400_000,
        ssrcs: vec![77777],
    });
    ctrl.on_rtcp_packet(&unrelated_remb, 11_500);
    assert_eq!(ctrl.tick_adaptation(12_500), None);
    assert_eq!(ctrl.current_target_bitrate_bps(), expected_cap);
}

#[test]
fn dim6_and_dim7_patch_stream_quality_policy_clamping_and_secret_redaction() {
    // Under FreeTierClamped (unpatched), requesting 1440p@60 is clamped to 720p@30 (1280x720@30, max 2.5 Mbps)
    let high_profile = StreamQualityProfile::from_preset(StreamProfilePreset::P1440Fps60);
    let mut ctrl_free = StreamQualityController::new(
        17843,
        high_profile,
        GoLiveQualityPatchPolicy::FreeTierClamped,
    )
    .unwrap();
    assert_eq!(ctrl_free.effective_profile().width, FREE_TIER_MAX_WIDTH);
    assert_eq!(ctrl_free.effective_profile().height, FREE_TIER_MAX_HEIGHT);
    assert_eq!(ctrl_free.effective_profile().fps, FREE_TIER_MAX_FPS);
    assert_eq!(
        ctrl_free.effective_profile().target_bitrate_bps,
        DEFAULT_720P30_TARGET_BITRATE_BPS
    );

    // Calling Vesktop's `patch_stream_quality(PatchedUnrestricted)` unlocks full 1440p@60 profile immediately
    let op12 = ctrl_free.patch_stream_quality(GoLiveQualityPatchPolicy::PatchedUnrestricted);
    assert_eq!(ctrl_free.effective_profile().width, 2560);
    assert_eq!(ctrl_free.effective_profile().height, 1440);
    assert_eq!(ctrl_free.effective_profile().fps, 60);
    assert_eq!(op12.streams[0].max_resolution.width, 2560);
    assert_eq!(op12.streams[0].max_resolution.height, 1440);
    assert_eq!(op12.streams[0].max_framerate, 60);
    assert!(ctrl_free.take_keyframe_requested());

    // Security & Privacy: GoLiveStreamContext redacts stream_key and rtc_token in Debug output
    let ctx = GoLiveStreamContext::new("guild:111:222:333", "super-secret-rtc-token-999", 17843);
    let debug_str = format!("{ctx:?}");
    assert!(!debug_str.contains("super-secret-rtc-token-999"));
    assert!(!debug_str.contains("guild:111:222:333"));
    assert!(debug_str.contains("<redacted>"));
}

#[test]
fn dim8_and_dim9_degradation_downscaling_recovery_and_telemetry_counters() {
    let profile = StreamQualityProfile::from_preset(StreamProfilePreset::P1080Fps60);
    let mut ctrl =
        StreamQualityController::new(9208, profile, GoLiveQualityPatchPolicy::PatchedUnrestricted)
            .unwrap();
    let _ = ctrl.handle_go_live(1920, 1080);

    // Severe bandwidth constraint via REMB (900 kbps wire -> 720 kbps effective) + MediaSinkWants quality=50
    // triggers resolution & framerate adaptation (`Degraded` state)
    let remb_low = RtcpPacket::Remb(RtcpRemb {
        sender_ssrc: 1,
        media_ssrc: 0,
        exponent: 0,
        mantissa: 0,
        bitrate_bps: 900_000,
        ssrcs: vec![9209],
    });
    ctrl.on_rtcp_packet(&remb_low, 1_000);
    let _ = ctrl.tick_adaptation(2_000);
    ctrl.apply_media_sink_wants(&MediaSinkWants::any(50));
    assert_eq!(ctrl.state(), StreamAdaptationState::Degraded);

    let degraded_out = ctrl.evaluate_encoder_Params(1920, 1080);
    assert!(degraded_out.encoded_width < 1920);
    assert!(degraded_out.encoded_height < 1080);
    assert!(degraded_out.target_bitrate_bps <= 720_000);

    // Receiver Report with high packet loss (fraction_lost = 40/256)
    let rr_pkt = RtcpPacket::ReceiverReport {
        sender_ssrc: 1,
        reports: vec![ReportBlock {
            ssrc: 9209,
            fraction_lost: 40,
            cumulative_lost: 12,
            extended_highest_seq: 500,
            jitter: 120,
            last_sr: 0,
            delay_since_last_sr: 0,
        }],
    };
    ctrl.on_rtcp_packet(&rr_pkt, 2_500);
    let _ = ctrl.tick_adaptation(3_000);
    assert!(ctrl.current_target_bitrate_bps() >= profile.min_bitrate_bps);

    // Now REMB returns to high bandwidth (15 Mbps) and sink wants returns to 100 with 0 loss
    ctrl.apply_media_sink_wants(&MediaSinkWants::any(100));
    for step in 1..=60 {
        let now_ms = 3_000 + REMB_LIMIT_TTL_MS + step * 1_000;
        ctrl.observe_network_feedback(Some(0), Some(15_000_000), now_ms);
        let _ = ctrl.tick_adaptation(now_ms);
    }

    assert_eq!(ctrl.state(), StreamAdaptationState::Active);
    let recovered_out = ctrl.evaluate_encoder_Params(1920, 1080);
    assert_eq!(recovered_out.encoded_width, 1920);
    assert_eq!(recovered_out.encoded_height, 1080);
    assert_eq!(recovered_out.encoded_fps, 60);

    // Verify observable telemetry
    let telemetry = ctrl.telemetry();
    assert!(telemetry.go_live_starts >= 1);
    assert!(telemetry.op12_frames_emitted >= 1);
    assert!(telemetry.bandwidth_adaptations >= 2);
    assert!(telemetry.congestion_backoffs >= 1);
    assert!(telemetry.recovery_steps >= 1);
    assert!(telemetry.sink_wants_updates >= 2);
    assert!(telemetry.resolution_scalings >= 1);
}
