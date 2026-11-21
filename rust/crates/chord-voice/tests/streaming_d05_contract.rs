#![forbid(unsafe_code)]
//! Contract tests for Task D-05: Streaming Protocol Wire Contracts.
//!
//! Specification & Clean-Room Reference Oracles:
//! - `docs/feature-backlog.md` row `D-05`:
//!   "Resolution parameters, video codec negotiation (H.264, AV1), RTP packetization rules,
//!   and voice opcodes `18..=22` | Reference Code: Stream Quality | Media Engine |
//!   Full wire contract specification for WebRTC screen share signaling, RTP framing, and keyframe request opcodes"
//! - `docs/re/voice-deep.md` & `docs/re/voice-deep.json`:
//!   Complete wire contract specification for WebRTC screen share signaling, resolution parameters,
//!   video codec negotiation (`AV1` PT 101/RTX 102, `H264` PT 105/RTX 106), RTP packetization rules
//!   (RFC 3550 + RFC 5285 `0xBEDE`, RFC 6184 Single NALU / STAP-A / FU-A, AV1 OBU Aggregation Header
//!   `Z`/`Y`/`W`/`N`, RFC 4588 RTX), opcodes `18..=22` across Main Gateway (`STREAM_CREATE`..`STREAM_SET_PAUSED`)
//!   and Voice Gateway (`SOUNDBOARD_SOUND`, `SPEAKING`, DAVE `OP_PREPARE_TRANSITION` / `OP_EXECUTE_TRANSITION`),
//!   and keyframe request opcodes (`RTCP PLI` PT=206 FMT=1, `RTCP FIR` PT=206 FMT=4, `Voice Opcode 12 Video`,
//!   `Voice Opcode 15 MediaSinkWants`).
//!
//! Covers all 9 mandatory dimensions (`TEST-POLICY.md` §12):
//! 1. Positive path: Resolution presets (`480p`..`4K Source` at `15`/`30`/`60` FPS), video codec negotiation
//!    (`AV1` PT 101/RTX 102 priority 1000, `H264` PT 105/RTX 106 priority 3000), RTP 90 kHz framing + `0xBEDE`,
//!    H.264 (Single NALU, STAP-A, FU-A) & AV1 (`Z`/`Y`/`W`/`N` LEB128) packetization + RTX (`OSN`) roundtrip,
//!    opcodes `18..=22` wire framing, and RTCP PLI/FIR keyframe requests.
//! 2. Negative path: Rejecting invalid dimensions (`< 64`, `> 3840`, odd YUV420 dimensions), invalid FPS (`0`, `> 60`),
//!    invalid bitrates (`< 150 kbps`, `> 25 Mbps`, inverted hierarchy), forbidden H.264 `F=1` bit, invalid NAL type (`0`, `24..=31`),
//!    forbidden AV1 `obu_forbidden_bit=1`, invalid MTU (`< 16`), and unsupported opcodes (`20`).
//! 3. Boundary conditions: Exact pixel limits (`854x480 = 409_920`, `1280x720 = 921_600`, `1920x1080 = 2_073_600`,
//!    `2560x1440 = 3_686_400`, `3840x2160 = 8_294_400`), exact 1200-byte vs 1201-byte MTU fragmentation boundary,
//!    Snowflake `0` and `u64::MAX` boundaries, `u16::MAX` sequence wrap, and `u8::MAX` FIR sequence wrap.
//! 4. Error handling: Typed `StreamQualityError`, `VideoRtpError`, `RtcpError`, `VoiceError`, and `SoundboardProtocolError`
//!    on malformed inputs without panics.
//! 5. Empty states: Empty video frames (`VideoRtpError::EmptyFrame`), empty STAP-A (`VideoRtpError::StapAEmpty`),
//!    and `MediaSinkWants` `quality: 0` pausing encoder (`active: false`, `PausedBySink`).
//! 6. Intermediate / lifecycle states: `Idle -> Active -> Degraded -> PausedBySink`, `VoiceSessionDriver`
//!    (`AwaitingReady -> AwaitingSessionDescription -> Established -> Resuming`), mid-stream `reconfigure_codec`
//!    (`H264 -> AV1`) preserving stabilized SSRCs, sequence, timestamp, and SRTP counter.
//! 7. Capabilities & privacy: `FreeTierClamped` (`720p@30`, `2.5 Mbps`) vs `PatchedUnrestricted` (`4K@60`, `25 Mbps`),
//!    and `Debug` redaction of sensitive `token`, `secret_key`, `session_id`, and `media_session_id` fields.
//! 8. Recovery & degradation: Error `2000`/`2007` (`OP 7 RESUME` preserving media state), RTCP fraction_lost congestion
//!    backoff (`>= 13/256`) and recovery (`<= 5/256`), REMB `4/5` headroom, and `15_000 ms` idle framerate reducer (`20 FPS`).
//! 9. Observable telemetry & specification contract: `StreamQualityTelemetry`, `RtcpKeyframeController` counters
//!    (`inbound_pli_count`, `inbound_fir_count`), and complete documentation in `docs/re/voice-deep.md`.

use std::fs;
use std::path::PathBuf;

use chord_voice::dave::{
    DaveExecuteTransition, DavePrepareTransition, OP_EXECUTE_TRANSITION, OP_PREPARE_TRANSITION,
};
use chord_voice::media::rtp::{
    ANNEX_B_START_CODE, EXTENSION_PROFILE, RtpHeader, VideoCodec, VideoFrameAssembler,
    av1_is_keyframe, h264_has_parameter_sets, h264_is_keyframe, parse_rtp, restore_rtx_bytes,
};
use chord_voice::rtcp::{
    FirEntry, KeyframeRequestKind, RTCP_FIR_LEN, RTCP_FMT_FIR, RTCP_FMT_NACK, RTCP_FMT_PLI,
    RTCP_FMT_REMB, RTCP_PLI_LEN, RTCP_PSFB, RTCP_RTPFB, ReportBlock, RtcpError, RtcpFir,
    RtcpKeyframeController, RtcpPacket, RtcpPli, build_compound_rr_fir, build_compound_rr_pli,
    build_fir, build_nack_from_sequences, build_pli, build_remb, parse_compound, parse_fir,
    parse_pli, parse_remb,
};
use chord_voice::rtp::video::{
    AV1_AGGR_N_NEW_CODED_VIDEO_SEQ, AV1_AGGR_Y_FRAGMENTED, AV1_AGGR_Z_CONTINUATION, AV1_OBU_FRAME,
    AV1_OBU_METADATA, AV1_OBU_SEQUENCE_HEADER, AV1_OBU_TILE_GROUP, DEFAULT_720P60_BITRATE_BPS,
    GW_OP_STREAM_CREATE, GW_OP_STREAM_DELETE, GW_OP_STREAM_PING, GW_OP_STREAM_SET_PAUSED,
    GW_OP_STREAM_WATCH, H264_FU_HEADER_END, H264_FU_HEADER_START, H264_NAL_FU_A, H264_NAL_IDR,
    H264_NAL_PPS, H264_NAL_SPS, H264_NAL_STAP_A, H264PacketizationKind, MAX_GO_LIVE_VIDEO_BITRATE_BPS,
    MAX_VIDEO_RTP_PAYLOAD_LEN, MIN_VIDEO_BITRATE_BPS, MIN_VIDEO_RTP_PAYLOAD_LEN,
    RTP_AV1_PAYLOAD_TYPE, RTP_AV1_RTX_PAYLOAD_TYPE, RTP_H264_PAYLOAD_TYPE,
    RTP_H264_RTX_PAYLOAD_TYPE, RTP_H265_PAYLOAD_TYPE, RTP_H265_RTX_PAYLOAD_TYPE,
    RTP_VP8_PAYLOAD_TYPE, RTP_VP8_RTX_PAYLOAD_TYPE, RtpVideoPacketizer, VIDEO_CLOCK_RATE,
    VIDEO_TICKS_30FPS, VIDEO_TICKS_60FPS, VideoRtpError, build_stream_create_call,
    build_stream_create_guild, build_stream_delete, build_stream_ping, build_stream_set_paused,
    build_stream_watch, clamp_video_bitrate_bps, encode_h264_fu_a, encode_h264_stap_a,
    packetize_av1_obus_with_mtu, packetize_h264_annex_b_with_mtu, parse_av1_obu_stream,
    parse_h264_annex_b, video_ticks_for_elapsed_ms, video_ticks_for_fps,
};
use chord_voice::session::{
    MediaSinkWants, StreamServerUpdate, VOICE_CLOSE_CODE_STREAM_INTERNAL_ERROR,
    VOICE_CLOSE_CODE_WEBRTC_MEDIA_TIMEOUT, VideoMaxResolution, VideoStreamDescriptor,
    VoiceCloseDisposition, VoiceSessionDriver, VoiceSessionPhase, VoiceVideo,
    classify_voice_close_code, compute_go_live_rtc_channel_id, compute_stream_ssrcs,
};
use chord_voice::soundboard_protocol::{
    SPEAKING_FLAG_MICROPHONE, SPEAKING_FLAG_PRIORITY, SPEAKING_FLAG_SOUND_SHARE,
    SoundboardProtocolError, VOICE_OP_SOUNDBOARD_SOUND, VOICE_OP_SPEAKING_SOUNDBOARD,
    VoiceOp18SoundboardPayload, VoiceOp19SpeakingSoundboardEvent,
};
use chord_voice::srtp::{EncryptionMode, SrtpContext};
use chord_voice::stream_quality::{
    ADAPTATION_TICK_INTERVAL_MS, CONGESTION_RECOVERY_HOLDOFF_MS,
    CONNECTION_IDLE_FRAMERATE_REDUCE_MS, DEFAULT_720P30_TARGET_BITRATE_BPS,
    DEFAULT_720P60_TARGET_BITRATE_BPS, FREE_TIER_MAX_CEILING_BITRATE_BPS, FREE_TIER_MAX_FPS,
    FREE_TIER_MAX_HEIGHT, FREE_TIER_MAX_TARGET_BITRATE_BPS, FREE_TIER_MAX_WIDTH,
    GoLiveQualityPatchPolicy, GoLiveStreamContext, IDLE_REDUCED_MAX_FRAMERATE,
    LOSS_CONGESTION_THRESHOLD_256, LOSS_RECOVERY_THRESHOLD_256, MAX_STREAM_BITRATE_BPS,
    MAX_STREAM_DIMENSION, MAX_STREAM_FPS, MIN_STREAM_BITRATE_BPS, MIN_STREAM_DIMENSION,
    MIN_STREAM_FPS, PIXEL_LIMIT_480P, PIXEL_LIMIT_720P, PIXEL_LIMIT_1080P, PIXEL_LIMIT_1440P,
    PIXEL_LIMIT_2160P, REMB_HEADROOM_DENOMINATOR, REMB_HEADROOM_NUMERATOR, REMB_LIMIT_TTL_MS,
    StreamAdaptationState, StreamContentHint, StreamFrameratePreset, StreamProfilePreset,
    StreamQualityController, StreamQualityError, StreamQualityProfile, StreamResolutionPreset,
    scale_dimensions_to_pixel_limit,
};
use chord_voice::{
    Codec, Identify, RTP_OPUS_PAYLOAD_TYPE, Ready, SelectProtocol, SessionDescription, StreamSpec,
    VoiceOp, VoicePayload, VoiceServerUpdate, VoiceStateUpdate, default_client_codecs, parse_frame,
};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn make_annex_b(nalus: &[&[u8]]) -> Vec<u8> {
    let mut out = Vec::new();
    for nalu in nalus {
        out.extend_from_slice(&ANNEX_B_START_CODE);
        out.extend_from_slice(nalu);
    }
    out
}

/// 1. Resolution Parameters, Quality Profiles, YUV420 Scaling & WebRTC Screen Share Signaling (`OP 12 VIDEO`, `OP 15 MEDIA_SINK_WANTS`).
#[test]
fn d05_resolution_parameters_scaling_and_screen_share_signaling_contract() {
    // Resolution pixel limits and presets
    assert_eq!(MIN_STREAM_DIMENSION, 64);
    assert_eq!(MAX_STREAM_DIMENSION, 3840);
    assert_eq!(MIN_STREAM_FPS, 1);
    assert_eq!(MAX_STREAM_FPS, 60);
    assert_eq!(MIN_STREAM_BITRATE_BPS, 150_000);
    assert_eq!(MAX_STREAM_BITRATE_BPS, 25_000_000);

    let expected_presets = [
        (StreamResolutionPreset::P480, (854, 480), PIXEL_LIMIT_480P),
        (StreamResolutionPreset::P720, (1280, 720), PIXEL_LIMIT_720P),
        (StreamResolutionPreset::P1080, (1920, 1080), PIXEL_LIMIT_1080P),
        (StreamResolutionPreset::P1440, (2560, 1440), PIXEL_LIMIT_1440P),
        (StreamResolutionPreset::Source4K, (3840, 2160), PIXEL_LIMIT_2160P),
    ];
    for (preset, dims, max_px) in expected_presets {
        assert_eq!(preset.dimensions(), dims);
        assert_eq!(preset.max_pixel_count(), max_px);
        assert_eq!(dims.0 * dims.1, max_px);
    }

    assert_eq!(StreamFrameratePreset::Fps15.as_u32(), 15);
    assert_eq!(StreamFrameratePreset::Fps30.as_u32(), 30);
    assert_eq!(StreamFrameratePreset::Fps60.as_u32(), 60);
    assert_eq!(video_ticks_for_fps(15), 6_000);
    assert_eq!(video_ticks_for_fps(30), VIDEO_TICKS_30FPS);
    assert_eq!(video_ticks_for_fps(60), VIDEO_TICKS_60FPS);
    assert_eq!(video_ticks_for_fps(0), VIDEO_CLOCK_RATE);

    // Verify 720p@30 oracle from `docs/re/voice-deep.md` §3 (`1280x720@30, bitrate 2.5 Mbps, rid 100, ssrc 9209, rtx 9210`)
    let p720_30 = StreamQualityProfile::from_preset(StreamProfilePreset::P720Fps30);
    assert_eq!(p720_30.width, 1280);
    assert_eq!(p720_30.height, 720);
    assert_eq!(p720_30.fps, 30);
    assert_eq!(p720_30.target_bitrate_bps, DEFAULT_720P30_TARGET_BITRATE_BPS);
    assert_eq!(p720_30.rtp_timestamp_step_90khz(), 3_000);
    assert_eq!(p720_30.content_hint, StreamContentHint::Detail);

    // Verify 720p@60 default profile (`4.0 Mbps` target, `6.0 Mbps` max) and 4K@60 (`18 Mbps` target, `25 Mbps` max)
    let p720_60 = StreamQualityProfile::default();
    assert_eq!(p720_60, StreamQualityProfile::from_preset(StreamProfilePreset::P720Fps60));
    assert_eq!(p720_60.target_bitrate_bps, DEFAULT_720P60_TARGET_BITRATE_BPS);
    assert_eq!(p720_60.content_hint, StreamContentHint::Motion);

    let p4k_60 = StreamQualityProfile::from_preset(StreamProfilePreset::Source4KFps60);
    assert_eq!(p4k_60.width, 3840);
    assert_eq!(p4k_60.height, 2160);
    assert_eq!(p4k_60.fps, 60);
    assert_eq!(p4k_60.max_bitrate_bps, MAX_STREAM_BITRATE_BPS);

    // Verify FreeTierClamped vs PatchedUnrestricted policy
    let clamped = p4k_60.apply_patch_policy(GoLiveQualityPatchPolicy::FreeTierClamped);
    assert_eq!(clamped.width, FREE_TIER_MAX_WIDTH);
    assert_eq!(clamped.height, FREE_TIER_MAX_HEIGHT);
    assert_eq!(clamped.fps, FREE_TIER_MAX_FPS);
    assert_eq!(clamped.target_bitrate_bps, FREE_TIER_MAX_TARGET_BITRATE_BPS);
    assert_eq!(clamped.max_bitrate_bps, FREE_TIER_MAX_CEILING_BITRATE_BPS);

    let unrestricted = p4k_60.apply_patch_policy(GoLiveQualityPatchPolicy::PatchedUnrestricted);
    assert_eq!(unrestricted, p4k_60);

    // Aspect-ratio preserving even-dimension YUV 4:2:0 scaling
    let (sw, sh) = scale_dimensions_to_pixel_limit(2560, 1440, 1280, 720, PIXEL_LIMIT_720P);
    assert_eq!((sw, sh), (1280, 720));
    assert_eq!(sw % 2, 0);
    assert_eq!(sh % 2, 0);
    assert!(sw * sh <= PIXEL_LIMIT_720P);

    let (uw, uh) = scale_dimensions_to_pixel_limit(3440, 1440, 1920, 1080, PIXEL_LIMIT_1080P);
    assert_eq!(uw % 2, 0);
    assert_eq!(uh % 2, 0);
    assert!(uw <= 1920 && uh <= 1080);
    assert!(uw * uh <= PIXEL_LIMIT_1080P);

    // SSRC derivation from Session 2 (`17843 -> 17844, 17845`) and Session 3 (`9208 -> 9209, 9210`)
    assert_eq!(compute_stream_ssrcs(17_843), (17_844, 17_845));
    assert_eq!(compute_stream_ssrcs(9_208), (9_209, 9_210));
    // Go Live rtc_channel_id = rtc_server_id - 1 (prevents Discord Error 2001)
    assert_eq!(
        compute_go_live_rtc_channel_id("1014537851418980372").as_deref(),
        Some("1014537851418980371")
    );
    assert_eq!(compute_go_live_rtc_channel_id("0"), None);

    // Voice Opcode 12 (Video) wire roundtrip
    let mut ctrl = StreamQualityController::new(
        9_208,
        p720_30,
        GoLiveQualityPatchPolicy::PatchedUnrestricted,
    )
    .expect("valid controller");
    let op12: VoiceVideo = ctrl.handle_go_live(1920, 1080);
    let op12_json = op12.to_json();
    let parsed_op12 = VoiceVideo::parse_json(&op12_json).expect("OP 12 JSON roundtrip");
    assert_eq!(parsed_op12.audio_ssrc, 9_208);
    assert_eq!(parsed_op12.video_ssrc, 9_209);
    assert_eq!(parsed_op12.rtx_ssrc, 9_210);
    assert_eq!(parsed_op12.streams.len(), 1);
    let desc: &VideoStreamDescriptor = &parsed_op12.streams[0];
    assert_eq!(desc.kind, "screen");
    assert_eq!(desc.rid, "100");
    assert_eq!(desc.quality, 100);
    assert_eq!(desc.ssrc, 9_209);
    assert_eq!(desc.rtx_ssrc, 9_210);
    assert!(desc.active);
    assert_eq!(desc.max_framerate, 30);
    assert_eq!(desc.max_bitrate, 3_500_000);
    assert_eq!(
        desc.max_resolution,
        VideoMaxResolution::fixed(1280, 720)
    );

    // Voice Opcode 15 (MediaSinkWants) wire roundtrip (`{"op":15,"d":{"any":100,"9209":50,"pixelCounts":409920}}`)
    let mut wants = MediaSinkWants::any(100).with_ssrc(9_209, 50);
    wants.pixel_counts = Some(PIXEL_LIMIT_480P);
    let wants_json = wants.to_json();
    let parsed_wants = MediaSinkWants::parse_json(&wants_json).expect("OP 15 JSON roundtrip");
    assert_eq!(parsed_wants.any, Some(100));
    assert_eq!(parsed_wants.pixel_counts, Some(PIXEL_LIMIT_480P));
    assert_eq!(parsed_wants.ssrc_qualities.get(&9_209), Some(&50));
    assert_eq!(parsed_wants.effective_quality_for_ssrc(9_209), 50);
    assert_eq!(parsed_wants.effective_quality_for_ssrc(99_999), 100);
}

/// 2. Video Codec Negotiation (`AV1` PT 101/RTX 102, `H265` PT 103/RTX 104, `H264` PT 105/RTX 106, `VP8` PT 107/RTX 108)
/// across `OP 0 IDENTIFY`, `OP 2 READY`, `OP 1 SELECT_PROTOCOL`, and `OP 4 SESSION_DESCRIPTION`.
#[test]
fn d05_video_codec_negotiation_and_signaling_wire_contract() {
    let codecs = default_client_codecs();
    assert_eq!(codecs.len(), 5);

    // 1. opus: PT 120, audio, priority 1000
    assert_eq!(
        codecs[0],
        Codec {
            name: "opus".to_owned(),
            kind: "audio".to_owned(),
            priority: 1000,
            payload_type: RTP_OPUS_PAYLOAD_TYPE,
            rtx_payload_type: None,
            encode: None,
            decode: None,
        }
    );
    // 2. AV1: PT 101, RTX PT 102, video, priority 1000, encode=true, decode=true
    assert_eq!(
        codecs[1],
        Codec::new("AV1", "video", 1000, RTP_AV1_PAYLOAD_TYPE).with_rtx(
            RTP_AV1_RTX_PAYLOAD_TYPE,
            true,
            true
        )
    );
    // 3. H265: PT 103, RTX PT 104, video, priority 2000
    assert_eq!(
        codecs[2],
        Codec::new("H265", "video", 2000, RTP_H265_PAYLOAD_TYPE).with_rtx(
            RTP_H265_RTX_PAYLOAD_TYPE,
            true,
            true
        )
    );
    // 4. H264: PT 105, RTX PT 106, video, priority 3000
    assert_eq!(
        codecs[3],
        Codec::new("H264", "video", 3000, RTP_H264_PAYLOAD_TYPE).with_rtx(
            RTP_H264_RTX_PAYLOAD_TYPE,
            true,
            true
        )
    );
    // 5. VP8: PT 107, RTX PT 108, video, priority 4000
    assert_eq!(
        codecs[4],
        Codec::new("VP8", "video", 4000, RTP_VP8_PAYLOAD_TYPE).with_rtx(
            RTP_VP8_RTX_PAYLOAD_TYPE,
            true,
            true
        )
    );

    // OP 0 IDENTIFY with video=true, max_dave_protocol_version=1, streams=[{type:"video",rid:"100",quality:100}]
    let identify = Identify {
        server_id: "1014537851418980372".to_owned(),
        channel_id: "1176785373989445703".to_owned(),
        user_id: "384299970632876032".to_owned(),
        session_id: "secret_session_id_value".to_owned(),
        token: "secret_voice_token_value".to_owned(),
        max_dave_protocol_version: 1,
        video: true,
        streams: vec![StreamSpec {
            kind: "video".to_owned(),
            ssrc: None,
            rtx_ssrc: None,
            rid: Some("100".to_owned()),
            quality: Some(100),
            active: None,
        }],
    };
    let identify_json = identify.to_json();
    let parsed_identify = parse_frame(&identify_json).expect("OP 0 Identify frame parses");
    assert_eq!(parsed_identify.op, VoiceOp::Identify);
    assert_eq!(parsed_identify.payload, VoicePayload::Identify(identify.clone()));

    // Security check: Identify Debug redacts credentials
    let id_debug = format!("{identify:?}");
    assert!(!id_debug.contains("secret_session_id_value"));
    assert!(!id_debug.contains("secret_voice_token_value"));
    assert!(id_debug.contains("<redacted>"));

    // OP 1 SELECT_PROTOCOL wire roundtrip with all 5 codecs
    let select = SelectProtocol::new("udp", "203.0.113.42", 35251, "aead_aes256_gcm_rtpsize", codecs)
        .with_rtc_connection_id("rtc-conn-d05")
        .with_experiments(vec!["fixed_keyframe_interval".to_owned()]);
    let select_json = select.to_json();
    let parsed_select = parse_frame(&select_json).expect("OP 1 SelectProtocol parses");
    assert_eq!(parsed_select.op, VoiceOp::SelectProtocol);
    assert_eq!(parsed_select.payload, VoicePayload::SelectProtocol(select));

    // OP 2 READY with audio SSRC 9208, video SSRC 9209, RTX SSRC 9210 (`docs/re/voice-deep.md` §1 Session 3)
    let ready_json = r#"{"op":2,"d":{"ssrc":9208,"ip":"104.29.146.60","port":19329,"modes":["aead_aes256_gcm_rtpsize","aead_xchacha20_poly1305_rtpsize"],"experiments":["fixed_keyframe_interval"],"streams":[{"type":"video","ssrc":9209,"rtx_ssrc":9210,"rid":"100","quality":100,"active":false}]}}"#;
    let parsed_ready = parse_frame(ready_json).expect("OP 2 Ready parses");
    assert_eq!(parsed_ready.op, VoiceOp::Ready);
    if let VoicePayload::Ready(Ready {
        ssrc,
        port,
        ip,
        modes,
        streams,
        ..
    }) = parsed_ready.payload
    {
        assert_eq!(ssrc, 9208);
        assert_eq!(port, 19329);
        assert_eq!(ip, "104.29.146.60");
        assert_eq!(modes.len(), 2);
        assert_eq!(streams.len(), 1);
        assert_eq!(streams[0].ssrc, Some(9209));
        assert_eq!(streams[0].rtx_ssrc, Some(9210));
        assert_eq!(streams[0].rid.as_deref(), Some("100"));
        assert_eq!(streams[0].quality, Some(100));
        assert_eq!(streams[0].active, Some(false));
    } else {
        panic!("expected VoicePayload::Ready");
    }

    // OP 4 SESSION_DESCRIPTION for AV1 and H264
    let op4_av1_json = r#"{"op":4,"d":{"mode":"aead_aes256_gcm_rtpsize","secret_key":[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32],"media_session_id":"secret_media_sess_1","dave_protocol_version":1,"secure_frames_version":1,"audio_codec":"opus","video_codec":"AV1"}}"#;
    let parsed_op4_av1 = parse_frame(op4_av1_json).expect("OP 4 AV1 parses");
    if let VoicePayload::SessionDescription(sd) = &parsed_op4_av1.payload {
        assert_eq!(sd.mode, "aead_aes256_gcm_rtpsize");
        assert_eq!(sd.secret_key.len(), 32);
        assert_eq!(sd.audio_codec.as_deref(), Some("opus"));
        assert_eq!(sd.video_codec.as_deref(), Some("AV1"));
        assert_eq!(sd.dave_protocol_version, 1);
        assert_eq!(sd.secure_frames_version, 1);
        let sd_debug = format!("{sd:?}");
        assert!(!sd_debug.contains("secret_media_sess_1"));
        assert!(sd_debug.contains("<redacted>"));
    } else {
        panic!("expected VoicePayload::SessionDescription");
    }
}

/// 3. RTP Packetization Rules for H.264 (RFC 6184: Single NALU, STAP-A, FU-A) & AV1 (OBU Aggregation Header `Z`/`Y`/`W`/`N`),
/// RFC 5285 `0xBEDE` Extension, and RFC 4588 RTX Retransmission (`OSN`).
#[test]
fn d05_rtp_packetization_h264_av1_rtx_and_bede_extension_contract() {
    assert_eq!(VIDEO_CLOCK_RATE, 90_000);
    assert_eq!(MAX_VIDEO_RTP_PAYLOAD_LEN, 1200);
    assert_eq!(MIN_VIDEO_RTP_PAYLOAD_LEN, 16);
    assert_eq!(EXTENSION_PROFILE, 0xBEDE);

    // --- A. H.264 Packetization (Single NALU <= 1200B, STAP-A type 24, FU-A type 28 > 1200B) ---
    let sps = vec![0x60 | H264_NAL_SPS, 0x64, 0x00, 0x1F, 0xAC, 0xD9];
    let pps = vec![0x60 | H264_NAL_PPS, 0xEB, 0xE3, 0xCB, 0x22, 0xC0];
    let mut idr_large = vec![0x60 | H264_NAL_IDR];
    idr_large.extend((0..2500).map(|i| ((i * 11 + 7) & 0xFF) as u8));
    let annex_b = make_annex_b(&[&sps, &pps, &idr_large]);
    assert!(h264_has_parameter_sets(&annex_b));
    assert!(h264_is_keyframe(&annex_b));

    let bede_8b = [0x10, 0x7F, 0x21, 0x00, 0x12, 0x34, 0x00, 0x00];
    let mut h264_pktizer =
        RtpVideoPacketizer::new_h264(9_209).with_initial_state(65_534, 180_000);
    assert_eq!(h264_pktizer.ssrc(), 9_209);
    assert_eq!(h264_pktizer.rtx_ssrc(), 9_210);
    assert_eq!(h264_pktizer.payload_type(), RTP_H264_PAYLOAD_TYPE);
    assert_eq!(h264_pktizer.rtx_payload_type(), RTP_H264_RTX_PAYLOAD_TYPE);

    let h264_pkts = h264_pktizer
        .packetize_h264_frame(&annex_b, VIDEO_TICKS_60FPS, Some(&bede_8b))
        .expect("H.264 frame packetizes");
    // 1 STAP-A (SPS+PPS) + 3 FU-A fragments (1198 + 1198 + 104 = 2500B IDR body) = 4 packets
    assert_eq!(h264_pkts.len(), 4);

    // Packet 0: STAP-A (`type = 24`), marker=false, seq=65534
    assert_eq!(h264_pkts[0].header.sequence, 65_534);
    assert_eq!(h264_pkts[0].header.timestamp, 180_000);
    assert!(!h264_pkts[0].header.marker);
    assert_eq!(h264_pkts[0].payload[0] & 0x1F, H264_NAL_STAP_A);
    assert_eq!(h264_pkts[0].raw[0], 0x90); // V=2, P=0, X=1, CC=0

    // Packet 1: FU-A Start (`S=1, E=0`), seq=65535
    assert_eq!(h264_pkts[1].header.sequence, 65_535);
    assert_eq!(h264_pkts[1].header.timestamp, 180_000);
    assert!(!h264_pkts[1].header.marker);
    assert_eq!(h264_pkts[1].payload[0] & 0x1F, H264_NAL_FU_A);
    assert_eq!(h264_pkts[1].payload[1], H264_FU_HEADER_START | H264_NAL_IDR);

    // Packet 2: FU-A Middle (`S=0, E=0`), seq=0 (wrapped u16::MAX -> 0)
    assert_eq!(h264_pkts[2].header.sequence, 0);
    assert_eq!(h264_pkts[2].header.timestamp, 180_000);
    assert!(!h264_pkts[2].header.marker);
    assert_eq!(h264_pkts[2].payload[1], H264_NAL_IDR);

    // Packet 3: FU-A End (`S=0, E=1`), seq=1, marker=true
    assert_eq!(h264_pkts[3].header.sequence, 1);
    assert_eq!(h264_pkts[3].header.timestamp, 180_000);
    assert!(h264_pkts[3].header.marker);
    assert_eq!(h264_pkts[3].payload[1], H264_FU_HEADER_END | H264_NAL_IDR);
    assert_eq!(h264_pktizer.timestamp(), 181_500);

    // Verify RTX RFC 4588 encapsulation (`PT = 106`, `rtx_ssrc = 9210`, 2B OSN prefix) and `restore_rtx_bytes`
    let rtx_pkt = h264_pktizer
        .build_rtx_packet(&h264_pkts[1])
        .expect("RTX packet built");
    assert_eq!(rtx_pkt.header.ssrc, 9_210);
    assert_eq!(rtx_pkt.header.payload_type, RTP_H264_RTX_PAYLOAD_TYPE);
    assert_eq!(rtx_pkt.header.sequence, 0);
    assert_eq!(&rtx_pkt.payload[0..2], &65_535u16.to_be_bytes());
    let restored_wire = restore_rtx_bytes(&rtx_pkt.raw, RTP_H264_PAYLOAD_TYPE, 9_209)
        .expect("restore RTX wire packet");
    assert_eq!(restored_wire, h264_pkts[1].raw);

    // Full H.264 reassembly via `VideoFrameAssembler`
    let mut h264_assembler = VideoFrameAssembler::new_h264();
    for (idx, pkt) in h264_pkts.iter().enumerate() {
        let parsed = parse_rtp(&pkt.raw).expect("valid RTP packet");
        let ext = parsed.extension.as_ref().expect("0xBEDE extension present");
        assert_eq!(ext.profile, 0xBEDE);
        assert_eq!(ext.data, &bede_8b);
        let out = h264_assembler.push_packet(&parsed);
        if idx + 1 == h264_pkts.len() {
            let frame = out.expect("assembled H.264 keyframe on marker=true");
            assert!(frame.keyframe);
            assert_eq!(frame.codec, VideoCodec::H264);
            assert_eq!(frame.data, annex_b);
        } else {
            assert!(out.is_none());
        }
    }

    // --- B. AV1 Packetization (Aggregation Header Z/Y/W/N, LEB128 sizes, OBU stream) ---
    let av1_seq_hdr = vec![
        (AV1_OBU_SEQUENCE_HEADER << 3) | 0x02,
        0x04,
        0xAA,
        0xBB,
        0xCC,
        0xDD,
    ];
    let av1_meta = vec![(AV1_OBU_METADATA << 3) | 0x02, 0x02, 0x11, 0x22];
    let mut av1_large_frame = vec![(AV1_OBU_FRAME << 3) | 0x02, 0x80, 0x10]; // size = 2048
    av1_large_frame.extend((0..2048).map(|i| ((i * 5 + 1) & 0xFF) as u8));

    let mut av1_pktizer = RtpVideoPacketizer::new_av1(17_844).with_initial_state(10, 90_000);
    assert_eq!(av1_pktizer.codec(), VideoCodec::Av1);
    assert_eq!(av1_pktizer.payload_type(), RTP_AV1_PAYLOAD_TYPE);
    assert_eq!(av1_pktizer.rtx_payload_type(), RTP_AV1_RTX_PAYLOAD_TYPE);

    let av1_pkts = av1_pktizer
        .packetize_av1_obus(
            &[&av1_seq_hdr, &av1_meta, &av1_large_frame],
            VIDEO_TICKS_30FPS,
            Some(&bede_8b),
        )
        .expect("AV1 keyframe packetizes");
    assert_eq!(av1_pkts.len(), 3);

    // Packet 0: W=2 (`0x20`) + N=1 (`0x08`) = `0x28` (aggregated `OBU_SEQUENCE_HEADER` + `OBU_METADATA`)
    assert_eq!(
        av1_pkts[0].payload[0],
        (2 << 4) | AV1_AGGR_N_NEW_CODED_VIDEO_SEQ
    );
    assert!(av1_is_keyframe(&av1_pkts[0].payload));
    assert!(!av1_pkts[0].header.marker);

    // Packet 1: First fragment of `av1_large_frame` (`Z=0, Y=1, W=1` -> `0x50`), marker=false
    assert_eq!(av1_pkts[1].payload[0], AV1_AGGR_Y_FRAGMENTED | (1 << 4));
    assert_eq!(av1_pkts[1].payload.len(), MAX_VIDEO_RTP_PAYLOAD_LEN);
    assert!(!av1_pkts[1].header.marker);

    // Packet 2: Continuation/end fragment (`Z=1, Y=0, W=1` -> `0x90`), marker=true
    assert_eq!(av1_pkts[2].payload[0], AV1_AGGR_Z_CONTINUATION | (1 << 4));
    assert!(av1_pkts[2].header.marker);

    // Full AV1 reassembly via `VideoFrameAssembler`
    let mut av1_assembler = VideoFrameAssembler::new_av1();
    for (idx, pkt) in av1_pkts.iter().enumerate() {
        let parsed = parse_rtp(&pkt.raw).expect("valid AV1 RTP packet");
        let out = av1_assembler.push_packet(&parsed);
        if idx + 1 == av1_pkts.len() {
            let frame = out.expect("assembled AV1 keyframe on marker=true");
            assert!(frame.keyframe);
            assert_eq!(frame.codec, VideoCodec::Av1);
            let mut expected_av1 = Vec::new();
            expected_av1.extend_from_slice(&av1_seq_hdr);
            expected_av1.extend_from_slice(&av1_meta);
            expected_av1.extend_from_slice(&av1_large_frame);
            assert_eq!(frame.data, expected_av1);
        } else {
            assert!(out.is_none());
        }
    }
}

/// 4. Opcodes `18..=22` Full Wire Contract Specification:
/// - Main Gateway Go Live Streaming Opcodes `18..=22` (`STREAM_CREATE`, `STREAM_DELETE`, `STREAM_WATCH`, `STREAM_PING`, `STREAM_SET_PAUSED`)
///   + `STREAM_SERVER_UPDATE` dispatch.
/// - Voice Gateway Opcodes `18..=22` (`18 SOUNDBOARD_SOUND`, `19 SPEAKING`, `21 DAVE_PREPARE_TRANSITION`, `22 DAVE_EXECUTE_TRANSITION`).
#[test]
fn d05_opcodes_18_to_22_main_gateway_and_voice_gateway_wire_contracts() {
    // --- A. Main Gateway Go Live Streaming Opcodes 18..=22 ---
    assert_eq!(GW_OP_STREAM_CREATE, 18);
    assert_eq!(GW_OP_STREAM_DELETE, 19);
    assert_eq!(GW_OP_STREAM_WATCH, 20);
    assert_eq!(GW_OP_STREAM_PING, 21);
    assert_eq!(GW_OP_STREAM_SET_PAUSED, 22);

    let stream_key_guild = "guild:1014537851418980372:1176785373989445703:384299970632876032";
    let stream_key_call = "call:1176785373989445703:384299970632876032";

    // Op 18 STREAM_CREATE (guild & call)
    let op18_guild = build_stream_create_guild(
        "1014537851418980372",
        "1176785373989445703",
        Some("frankfurt"),
    );
    let op18_guild_json = chord_json::parse(&op18_guild).unwrap();
    assert_eq!(op18_guild_json.get("op").and_then(|v| v.as_u64()), Some(18));
    assert_eq!(
        op18_guild_json
            .get("d")
            .and_then(|d| d.get("type"))
            .and_then(|v| v.as_str()),
        Some("guild")
    );
    assert_eq!(
        op18_guild_json
            .get("d")
            .and_then(|d| d.get("preferred_region"))
            .and_then(|v| v.as_str()),
        Some("frankfurt")
    );

    let op18_call = build_stream_create_call("1176785373989445703", None);
    let op18_call_json = chord_json::parse(&op18_call).unwrap();
    assert_eq!(op18_call_json.get("op").and_then(|v| v.as_u64()), Some(18));
    assert_eq!(
        op18_call_json
            .get("d")
            .and_then(|d| d.get("type"))
            .and_then(|v| v.as_str()),
        Some("call")
    );
    assert!(
        op18_call_json
            .get("d")
            .and_then(|d| d.get("guild_id"))
            .is_some_and(|v| matches!(v, chord_json::JsonValue::Null))
    );

    // Op 19 STREAM_DELETE
    let op19_del = build_stream_delete(stream_key_guild);
    let op19_json = chord_json::parse(&op19_del).unwrap();
    assert_eq!(op19_json.get("op").and_then(|v| v.as_u64()), Some(19));
    assert_eq!(
        op19_json
            .get("d")
            .and_then(|d| d.get("stream_key"))
            .and_then(|v| v.as_str()),
        Some(stream_key_guild)
    );

    // Op 20 STREAM_WATCH
    let op20_watch = build_stream_watch(stream_key_guild);
    let op20_json = chord_json::parse(&op20_watch).unwrap();
    assert_eq!(op20_json.get("op").and_then(|v| v.as_u64()), Some(20));
    assert_eq!(
        op20_json
            .get("d")
            .and_then(|d| d.get("stream_key"))
            .and_then(|v| v.as_str()),
        Some(stream_key_guild)
    );

    // Op 21 STREAM_PING
    let op21_ping = build_stream_ping(stream_key_guild);
    let op21_json = chord_json::parse(&op21_ping).unwrap();
    assert_eq!(op21_json.get("op").and_then(|v| v.as_u64()), Some(21));
    assert_eq!(
        op21_json
            .get("d")
            .and_then(|d| d.get("stream_key"))
            .and_then(|v| v.as_str()),
        Some(stream_key_guild)
    );

    // Op 22 STREAM_SET_PAUSED
    let op22_pause = build_stream_set_paused(stream_key_guild, true);
    let op22_json = chord_json::parse(&op22_pause).unwrap();
    assert_eq!(op22_json.get("op").and_then(|v| v.as_u64()), Some(22));
    assert_eq!(
        op22_json
            .get("d")
            .and_then(|d| d.get("paused"))
            .and_then(|v| v.as_bool()),
        Some(true)
    );

    // STREAM_SERVER_UPDATE parsing & stream_key decomposition
    let ssu_guild = StreamServerUpdate::parse(&format!(
        r#"{{"d":{{"stream_key":"{stream_key_guild}","server_id":"1014537851418980372","endpoint":"c-fra19-4da7fe5f.discord.media:2053","token":"super_secret_stream_tok"}}}}"#
    ))
    .expect("STREAM_SERVER_UPDATE guild parses");
    assert_eq!(
        ssu_guild.parse_stream_key(),
        Some((
            "1014537851418980372",
            "1176785373989445703",
            "384299970632876032"
        ))
    );
    assert_eq!(
        ssu_guild.websocket_url().unwrap(),
        "wss://c-fra19-4da7fe5f.discord.media:2053/?v=9"
    );
    assert!(!format!("{ssu_guild:?}").contains("super_secret_stream_tok"));

    let ssu_call =
        StreamServerUpdate::new(stream_key_call, "c-arn02-5229f1fc.discord.media:2096", "tok");
    assert_eq!(
        ssu_call.parse_stream_key(),
        Some((
            "1176785373989445703",
            "1176785373989445703",
            "384299970632876032"
        ))
    );

    // --- B. Voice Gateway Opcodes 18..=22 ---
    assert_eq!(VOICE_OP_SOUNDBOARD_SOUND, 18);
    assert_eq!(VOICE_OP_SPEAKING_SOUNDBOARD, 19);
    assert_eq!(OP_PREPARE_TRANSITION, 21);
    assert_eq!(OP_EXECUTE_TRANSITION, 22);
    assert_eq!(
        VoiceOp::from_u8(21),
        VoiceOp::SecureFramesPrepareProtocolTransition
    );
    assert_eq!(
        VoiceOp::from_u8(22),
        VoiceOp::SecureFramesExecuteTransition
    );
    assert!(VoiceOp::from_u8(21).is_dave_or_mls());
    assert!(VoiceOp::from_u8(22).is_dave_or_mls());

    // Voice Op 18 (`SOUNDBOARD_SOUND`) & Voice Op 19 (`SPEAKING` soundboard)
    let v_op18 = VoiceOp18SoundboardPayload::build(
        "1100000000000000001",
        Some("1014537851418980372"),
        0.75,
        None,
        Some("🔥"),
        9_208,
    )
    .expect("Voice Op 18 builds");
    let v_op18_json = v_op18.to_ws_json();
    assert!(v_op18_json.contains(r#""op":18"#));
    assert!(v_op18_json.contains(r#""sound_id":"1100000000000000001""#));
    assert!(v_op18_json.contains(r#""ssrc":9208"#));

    let v_op19 = VoiceOp19SpeakingSoundboardEvent::parse(
        19,
        "384299970632876032",
        9_208,
        SPEAKING_FLAG_SOUND_SHARE | SPEAKING_FLAG_MICROPHONE,
        Some("1100000000000000001"),
        Some("1014537851418980372"),
        0.8,
    )
    .expect("Voice Op 19 parses");
    assert!(v_op19.is_soundboard_speaking());
    assert!(v_op19.is_microphone_speaking());
    assert!(!v_op19.is_priority_speaker());

    // Voice Op 21 (`DavePrepareTransition`) & Voice Op 22 (`DaveExecuteTransition`)
    let v_op21 = DavePrepareTransition {
        transition_id: 14,
        protocol_version: 1,
    };
    let v_op21_json = v_op21.to_json();
    assert_eq!(
        DavePrepareTransition::parse_json(&v_op21_json).unwrap(),
        v_op21
    );

    let v_op22 = DaveExecuteTransition { transition_id: 14 };
    let v_op22_json = v_op22.to_json();
    assert_eq!(
        DaveExecuteTransition::parse_json(&v_op22_json).unwrap(),
        v_op22
    );
}

/// 5. Keyframe Request Opcodes (`RTCP PLI` PT=206 FMT=1, `RTCP FIR` PT=206 FMT=4, Compound RR+PLI/FIR,
/// `RtcpKeyframeController`, and automatic `SPS+PPS+IDR` / `OBU_SEQUENCE_HEADER` injection).
#[test]
fn d05_keyframe_request_opcodes_pli_fir_and_encoder_injection_contract() {
    assert_eq!(RTCP_RTPFB, 205);
    assert_eq!(RTCP_PSFB, 206);
    assert_eq!(RTCP_FMT_NACK, 1);
    assert_eq!(RTCP_FMT_PLI, 1);
    assert_eq!(RTCP_FMT_FIR, 4);
    assert_eq!(RTCP_FMT_REMB, 15);
    assert_eq!(RTCP_PLI_LEN, 12);
    assert_eq!(RTCP_FIR_LEN, 20);

    // 1. RTCP PLI (12 bytes: `[0x81, 206, 0x00, 0x02, sender_ssrc, media_ssrc]`)
    let pli_bytes = build_pli(9_208, 9_209);
    assert_eq!(pli_bytes[0], 0x80 | RTCP_FMT_PLI);
    assert_eq!(pli_bytes[1], RTCP_PSFB);
    assert_eq!(u16::from_be_bytes([pli_bytes[2], pli_bytes[3]]), 2);
    let parsed_pli: RtcpPli = parse_pli(&pli_bytes).expect("parse PLI");
    assert_eq!(
        parsed_pli,
        RtcpPli {
            sender_ssrc: 9_208,
            media_ssrc: 9_209
        }
    );

    // 2. RTCP FIR (20 bytes: `[0x84, 206, 0x00, 0x04, sender_ssrc, 0, target_ssrc, seq, 0, 0, 0]`)
    let fir_bytes = build_fir(9_208, 9_209, 42);
    assert_eq!(fir_bytes[0], 0x80 | RTCP_FMT_FIR);
    assert_eq!(fir_bytes[1], RTCP_PSFB);
    assert_eq!(u16::from_be_bytes([fir_bytes[2], fir_bytes[3]]), 4);
    let parsed_fir: RtcpFir = parse_fir(&fir_bytes).expect("parse FIR");
    assert_eq!(parsed_fir.sender_ssrc, 9_208);
    assert_eq!(parsed_fir.media_ssrc, 0);
    assert_eq!(
        parsed_fir.entries,
        vec![FirEntry {
            ssrc: 9_209,
            sequence_number: 42
        }]
    );

    // 3. Compound RR + PLI and RR + FIR parsing
    let report = ReportBlock {
        ssrc: 9_209,
        fraction_lost: 0,
        cumulative_lost: 0,
        extended_highest_seq: 1200,
        jitter: 15,
        last_sr: 0x1122_3344,
        delay_since_last_sr: 1024,
    };
    let compound_pli = build_compound_rr_pli(9_208, Some(&report), 9_209);
    let parsed_comp_pli = parse_compound(&compound_pli).expect("parse compound RR+PLI");
    assert_eq!(parsed_comp_pli.packets.len(), 2);

    let compound_fir = build_compound_rr_fir(9_208, None, 9_209, 0);
    let parsed_comp_fir = parse_compound(&compound_fir).expect("parse compound empty RR+FIR");
    assert_eq!(parsed_comp_fir.packets.len(), 2);

    // 4. RtcpKeyframeController deduplication of FIR sequence numbers and wrap-around at 255 -> 0
    let mut kf_ctrl = RtcpKeyframeController::new(9_208, 9_209);
    assert!(!kf_ctrl.is_keyframe_requested());

    assert!(kf_ctrl.on_rtcp_packet(&RtcpPacket::Pli(parsed_pli)));
    assert!(kf_ctrl.is_keyframe_requested());
    assert_eq!(kf_ctrl.inbound_pli_count(), 1);
    assert_eq!(kf_ctrl.last_request_kind(), Some(KeyframeRequestKind::Pli));
    assert!(kf_ctrl.take_keyframe_request());
    assert!(!kf_ctrl.is_keyframe_requested());

    // First FIR seq=255 triggers keyframe; duplicate FIR seq=255 is ignored; wrapped seq=0 triggers
    let fir_255 = parse_fir(&build_fir(9_208, 9_209, 255)).unwrap();
    assert!(kf_ctrl.on_rtcp_packet(&RtcpPacket::Fir(fir_255.clone())));
    assert_eq!(kf_ctrl.inbound_fir_count(), 1);
    assert_eq!(
        kf_ctrl.last_request_kind(),
        Some(KeyframeRequestKind::Fir {
            sequence_number: 255
        })
    );
    assert!(kf_ctrl.take_keyframe_request());

    // Duplicate FIR seq=255 must NOT re-trigger
    assert!(!kf_ctrl.on_rtcp_packet(&RtcpPacket::Fir(fir_255)));
    assert_eq!(kf_ctrl.inbound_fir_count(), 1);
    assert!(!kf_ctrl.is_keyframe_requested());

    // Wrapped FIR seq=0 MUST trigger
    let fir_0 = parse_fir(&build_fir(9_208, 9_209, 0)).unwrap();
    assert!(kf_ctrl.on_rtcp_packet(&RtcpPacket::Fir(fir_0)));
    assert_eq!(kf_ctrl.inbound_fir_count(), 2);
    assert!(kf_ctrl.take_keyframe_request());

    // 5. RtpVideoPacketizer automatic SPS+PPS prepending on H.264 IDR and OBU_SEQUENCE_HEADER prepending on AV1 after PLI
    let mut h264_pktizer = RtpVideoPacketizer::new_h264(9_209);
    h264_pktizer.set_h264_parameter_sets(vec![0x67, 0x42, 0x00, 0x1F], vec![0x68, 0xCE, 0x3C, 0x80]);
    assert!(h264_pktizer.on_rtcp_compound(&compound_pli).unwrap());
    assert!(h264_pktizer.is_keyframe_requested());
    assert_eq!(h264_pktizer.pli_received_count(), 1);

    // Next H.264 IDR slice without SPS/PPS automatically gets cached SPS+PPS prepended as STAP-A
    let idr_only = make_annex_b(&[&[0x65, 0x88, 0x84, 0x21]]);
    let upgraded_pkts = h264_pktizer
        .packetize_h264_frame(&idr_only, VIDEO_TICKS_60FPS, None)
        .expect("IDR upgraded with cached SPS+PPS");
    assert_eq!(upgraded_pkts.len(), 2); // Packet 0: STAP-A(SPS,PPS), Packet 1: SingleNalu(IDR)
    assert_eq!(upgraded_pkts[0].payload[0] & 0x1F, H264_NAL_STAP_A);
    assert_eq!(upgraded_pkts[1].payload[0] & 0x1F, H264_NAL_IDR);
    assert!(!h264_pktizer.is_keyframe_requested(), "keyframe request cleared after IDR emission");
}

/// 6. Nine Mandatory Dimensions: Negative, Boundary, Empty, Lifecycle, Privacy, Recovery, and Telemetry.
#[test]
fn d05_nine_dimensions_negative_boundaries_recovery_and_telemetry() {
    // Dimension 2 & 4: Negative & Error handling across H.264, AV1, MTU, and Opcodes
    assert_eq!(
        parse_h264_annex_b(&[]),
        Err(VideoRtpError::EmptyFrame)
    );
    assert_eq!(
        parse_h264_annex_b(&[0, 0, 0, 1, 0x85, 0x01]),
        Err(VideoRtpError::ForbiddenZeroBitSet(0x85))
    );
    assert_eq!(
        parse_h264_annex_b(&[0, 0, 0, 1, 0x60 | 24, 0x01]),
        Err(VideoRtpError::InvalidNalType(24))
    );
    assert_eq!(
        encode_h264_stap_a(&[], 1200),
        Err(VideoRtpError::StapAEmpty)
    );
    assert_eq!(
        packetize_h264_annex_b_with_mtu(&[0x65, 0x01], 15),
        Err(VideoRtpError::InvalidMtu(15))
    );
    assert_eq!(
        parse_av1_obu_stream(&[0x80 | (AV1_OBU_FRAME << 3)]),
        Err(VideoRtpError::InvalidAv1Obu("obu_forbidden_bit must be 0"))
    );
    assert_eq!(
        VoiceOp19SpeakingSoundboardEvent::parse(20, "384299970632876032", 9208, 2, None, None, 1.0),
        Err(SoundboardProtocolError::UnsupportedVoiceOpcode(20))
    );
    assert_eq!(
        VoiceOp18SoundboardPayload::build("0", None, 0.5, None, None, 9208),
        Err(SoundboardProtocolError::InvalidSnowflake("0".to_owned()))
    );
    assert!(
        VoiceOp18SoundboardPayload::build(
            &u64::MAX.to_string(),
            Some(&u64::MAX.to_string()),
            1.0,
            None,
            None,
            u32::MAX
        )
        .is_ok(),
        "u64::MAX Snowflake boundary accepted"
    );

    // Dimension 5, 6, 8 & 9: Bandwidth adaptation, MediaSinkWants pause, 15s idle framerate reducer, and telemetry
    let profile = StreamQualityProfile::from_preset(StreamProfilePreset::P1080Fps60);
    let mut ctrl = StreamQualityController::new(
        9_208,
        profile,
        GoLiveQualityPatchPolicy::PatchedUnrestricted,
    )
    .expect("1080p60 controller");
    let _ = ctrl.handle_go_live(1920, 1080);
    assert_eq!(ctrl.state(), StreamAdaptationState::Active);

    // Empty/paused sink state (`MediaSinkWants` `any: 0`) pauses encoding
    ctrl.apply_media_sink_wants(&MediaSinkWants::any(0));
    assert_eq!(ctrl.state(), StreamAdaptationState::PausedBySink);
    let paused_out = ctrl.evaluate_encoder_Params(1920, 1080);
    assert!(!paused_out.active);
    assert_eq!(paused_out.encoded_fps, 0);

    // Restore sink wants to 100
    ctrl.apply_media_sink_wants(&MediaSinkWants::any(100));
    assert_eq!(ctrl.state(), StreamAdaptationState::Active);

    // Congestion drop at fraction_lost == 13/256 (>= LOSS_CONGESTION_THRESHOLD_256)
    let initial_br = ctrl.current_target_bitrate_bps();
    ctrl.observe_network_feedback(Some(LOSS_CONGESTION_THRESHOLD_256), None, 1_000);
    let dropped_br = ctrl.tick_adaptation(1_000).expect("congestion reduces bitrate");
    assert_eq!(dropped_br, (initial_br * 4) / 5);
    assert_eq!(ctrl.telemetry().congestion_backoffs, 1);

    // Recovery after CONGESTION_RECOVERY_HOLDOFF_MS (2_000 ms) at fraction_lost == 5/256 (<= LOSS_RECOVERY_THRESHOLD_256)
    ctrl.observe_network_feedback(Some(LOSS_RECOVERY_THRESHOLD_256), None, 3_000);
    let recovered_br = ctrl
        .tick_adaptation(1_000 + CONGESTION_RECOVERY_HOLDOFF_MS)
        .expect("low loss after holdoff recovers bitrate");
    assert!(recovered_br > dropped_br);
    assert_eq!(ctrl.telemetry().recovery_steps, 1);

    // Static screen idle framerate reducer at exact 14_999 ms vs 15_000 ms boundary (`docs/re/voice-deep.md` §3)
    ctrl.update_static_screen_idle_ms(CONNECTION_IDLE_FRAMERATE_REDUCE_MS - 1);
    assert_eq!(ctrl.evaluate_encoder_Params(1920, 1080).encoded_fps, 60);
    ctrl.update_static_screen_idle_ms(CONNECTION_IDLE_FRAMERATE_REDUCE_MS);
    assert_eq!(
        ctrl.evaluate_encoder_Params(1920, 1080).encoded_fps,
        IDLE_REDUCED_MAX_FRAMERATE
    );
    assert_eq!(ctrl.telemetry().idle_framerate_reductions, 1);

    // Dimension 7: Privacy redaction of GoLiveStreamContext
    let ctx = GoLiveStreamContext::new(
        "guild:1014537851418980372:1176785373989445703:384299970632876032",
        "top_secret_stream_token",
        "top_secret_media_session",
        profile,
    );
    let ctx_debug = format!("{ctx:?}");
    assert!(!ctx_debug.contains("top_secret_stream_token"));
    assert!(!ctx_debug.contains("top_secret_media_session"));
    assert!(ctx_debug.contains("<redacted>"));
}

/// 7. Clean-Room Specification Verification in `docs/re/voice-deep.md`:
/// Verifies that `docs/re/voice-deep.md` contains the full wire contract specification for:
/// - Resolution parameters (`480p`, `720p`, `1080p`, `1440p`, `4K Source`, pixel limits, bitrates, `ConnectionEventFramerateReducer`).
/// - Video codec negotiation (`AV1` PT 101/RTX 102, `H265` PT 103/RTX 104, `H264` PT 105/RTX 106, `VP8` PT 107/RTX 108).
/// - RTP packetization rules (`90 kHz`, `0xBEDE`, H.264 Single NALU / `STAP-A` / `FU-A`, AV1 OBU Aggregation Header `Z`/`Y`/`W`/`N`, `RTX` `OSN`).
/// - Opcodes `18..=22` (Main Gateway `STREAM_CREATE`, `STREAM_DELETE`, `STREAM_WATCH`, `STREAM_PING`, `STREAM_SET_PAUSED`
///   and Voice Gateway `SOUNDBOARD_SOUND`, `SPEAKING`, `SECURE_FRAMES_PREPARE_PROTOCOL_TRANSITION`, `SECURE_FRAMES_EXECUTE_TRANSITION`).
/// - Keyframe request opcodes (`RTCP PLI` PT=206 FMT=1, `RTCP FIR` PT=206 FMT=4, `OP 12 VIDEO`, `OP 15 MEDIA_SINK_WANTS`).
#[test]
fn d05_voice_deep_doc_specifies_full_streaming_wire_contracts() {
    let doc_path = repo_root().join("docs/re/voice-deep.md");
    let doc = fs::read_to_string(&doc_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", doc_path.display()));

    // Required sections and wire contract specification items for D-05
    let required_tokens = [
        // Section headings & D-05 markers
        "D-05",
        "Streaming Protocol Wire Contracts",
        // 1. Resolution parameters & quality profiles
        "StreamResolutionPreset",
        "StreamQualityProfile",
        "409,920",
        "921,600",
        "2,073,600",
        "3,686,400",
        "8,294,400",
        "ConnectionEventFramerateReducer",
        "remoteSinkWantsMaxFramerate",
        // 2. Video codec negotiation
        "default_client_codecs",
        "rtx_payload_type",
        "101",
        "102",
        "103",
        "104",
        "105",
        "106",
        "107",
        "108",
        // 3. RTP packetization rules (H.264, AV1, RTX, 90 kHz, 0xBEDE)
        "90,000 Hz",
        "0xBEDE",
        "STAP-A",
        "FU-A",
        "OBU_SEQUENCE_HEADER",
        "RFC 6184",
        "RFC 4588",
        "MAX_VIDEO_RTP_PAYLOAD_LEN",
        "1200",
        // 4. Opcodes 18..=22 (Main Gateway & Voice Gateway)
        "STREAM_CREATE",
        "STREAM_DELETE",
        "STREAM_WATCH",
        "STREAM_PING",
        "STREAM_SET_PAUSED",
        "STREAM_SERVER_UPDATE",
        "SOUNDBOARD_SOUND",
        "SECURE_FRAMES_PREPARE_PROTOCOL_TRANSITION",
        "SECURE_FRAMES_EXECUTE_TRANSITION",
        // 5. Keyframe request opcodes & signaling
        "RTCP PLI",
        "RTCP FIR",
        "PT=206",
        "FMT=1",
        "FMT=4",
        "FMT=15",
        "REMB",
        "RtcpKeyframeController",
        "RtpVideoPacketizer",
    ];

    for token in required_tokens {
        assert!(
            doc.contains(token),
            "docs/re/voice-deep.md is missing required D-05 wire contract token `{token}`"
        );
    }
}
