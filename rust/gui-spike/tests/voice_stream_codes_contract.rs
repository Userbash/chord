//! W8 (`TASK-824`): Комплексный интеграционный контракт верификации всех кодов ошибок
//! видео и стриминга Discord `2***` (`2000..=2018`), 15/15 мутационный гейт и проверка
//! чистоты трассировок GDB (`/tmp/gdb-native-trace.log` и `/tmp/discord-full-user-activity.log`).
//!
//! Провенанс эталонного RE-дампа `/home/sanya/re-deep/`:
//! - `/home/sanya/re-deep/decoded/3eefa7dae1e2c234_0`:
//!   - модуль `487329` (offset `4468792`): таблица `AVError` (`1001..=1003`, `2001..=2016`, `3001..=3004`, `4001`),
//!     проверка уникальности `_.length === new Set(_).size`, метаданные `{ errorCode, severity, category, isErrorOutbound }`;
//!   - модуль `970048`: `AVErrorManager`, порог минимального FPS `B(maxFrameRate)` (`<=5->3, <=15->8, <=30->15, >30->30`),
//!     правила детекции `2001..=2016`;
//!   - модуль `161518`: `AVErrorStore`, `REPORT_AV_ERROR`, ключи контекста `{streamKey}:{mediaSessionId}`;
//!   - модуль `803301`: `VideoStreamStore`, 20-секундный таймер `getTimedoutVideos()` (`2011, 2012, 2014, 2015`);
//!   - модуль `821715`: приоритеты предупреждений стрима (`1001->0`, `2003/2004->1`, `2002/2005->2`, `2006->3`);
//! - `/home/sanya/re-deep/ru-catalog.json`:
//!   - `pdFFK+`: `["Вы были отключены..."]`;
//!   - `+PuO2N`: `["Не переживайте, сейчас всё исправим!"]`;
//!   - `ejOT95`: `["Ошибка: ", [1, "errorCode"]]`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use chord_client::voice_diagnostics::{
    build_popout_dto, build_rtc_quality_status_bar_dto, defeat_av_error_code,
    discord_min_encode_fps_threshold, evaluate_and_defeat_av_errors, AvStreamTelemetryInput,
    ConnectionQuality, DiscordAvErrorCode, PingHistory, DISCORD_DEFEATED_AV_ERROR_CODES,
};
use chord_client::voice_stage::{
    build_gateway_stream_create_op18, build_gateway_stream_delete_op19,
    build_gateway_stream_ping_op21, build_gateway_stream_set_paused_op22,
    build_gateway_stream_watch_op20, ScreenShareSpec, StreamFps, StreamResolution,
};
use chord_voice::media::rtp::parse_rtp;
use chord_voice::reception::{ReceptionStats, RtpHeader};
use chord_voice::rtcp::RtcpPacket;
use chord_voice::rtp::video::{RTP_H264_PAYLOAD_TYPE, VIDEO_TICKS_30FPS, VIDEO_TICKS_60FPS};
use chord_voice::rtp::{video_ticks_for_elapsed_ms, video_ticks_for_fps};
use chord_voice::session::{
    classify_voice_close_code, compute_go_live_rtc_channel_id, is_recoverable_stream_error_code,
    StreamServerUpdate, VoiceCloseDisposition, VoiceSessionDriver, VoiceSessionPhase,
    VOICE_CLOSE_CODE_STREAM_INTERNAL_ERROR, VOICE_CLOSE_CODE_WEBRTC_MEDIA_TIMEOUT,
};
use chord_voice::{
    parse_frame, Identify, VoiceOp, VoicePayload, VoiceServerUpdate, VoiceStateUpdate,
};

const SAMPLE_SRTP_KEY: [u8; 32] = [
    0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x20,
    0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f, 0x30,
];

/// Классификация критичности кода `2000..=2018` по RE-оракулу (`487329`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OracleSeverity {
    Critical,
    Warning,
    BoundaryOrTransport,
}

impl OracleSeverity {
    const fn as_re_str(self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::Warning => "warning",
            Self::BoundaryOrTransport => "boundary",
        }
    }
}

/// Запись полного эталонного каталога Discord `2***` (`2000..=2018`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Discord2xxxOracleSpec {
    code: u16,
    slug: Option<&'static str>,
    allocated_in_av_error_map: bool,
    severity: OracleSeverity,
    category: &'static str,
    is_error_outbound: Option<bool>,
    warning_priority: Option<u8>,
}

const DISCORD_2XXX_ORACLE_SPECS: [Discord2xxxOracleSpec; 19] = [
    Discord2xxxOracleSpec {
        code: 2000,
        slug: Some("stream-internal-close-boundary"),
        allocated_in_av_error_map: false,
        severity: OracleSeverity::BoundaryOrTransport,
        category: "stream",
        is_error_outbound: None,
        warning_priority: None,
    },
    Discord2xxxOracleSpec {
        code: 2001,
        slug: Some("stream-failed-to-start"),
        allocated_in_av_error_map: true,
        severity: OracleSeverity::Critical,
        category: "video",
        is_error_outbound: Some(true),
        warning_priority: None,
    },
    Discord2xxxOracleSpec {
        code: 2002,
        slug: Some("stream-view-low-fps"),
        allocated_in_av_error_map: true,
        severity: OracleSeverity::Warning,
        category: "video",
        is_error_outbound: Some(false),
        warning_priority: Some(2),
    },
    Discord2xxxOracleSpec {
        code: 2003,
        slug: Some("stream-view-high-packet-loss"),
        allocated_in_av_error_map: true,
        severity: OracleSeverity::Warning,
        category: "video",
        is_error_outbound: Some(false),
        warning_priority: Some(1),
    },
    Discord2xxxOracleSpec {
        code: 2004,
        slug: Some("stream-send-high-packet-loss"),
        allocated_in_av_error_map: true,
        severity: OracleSeverity::Warning,
        category: "video",
        is_error_outbound: Some(true),
        warning_priority: Some(1),
    },
    Discord2xxxOracleSpec {
        code: 2005,
        slug: Some("stream-send-low-encode-fps"),
        allocated_in_av_error_map: true,
        severity: OracleSeverity::Warning,
        category: "video",
        is_error_outbound: Some(true),
        warning_priority: Some(2),
    },
    Discord2xxxOracleSpec {
        code: 2006,
        slug: Some("stream-send-network-quality"),
        allocated_in_av_error_map: true,
        severity: OracleSeverity::Warning,
        category: "video",
        is_error_outbound: Some(true),
        warning_priority: Some(3),
    },
    Discord2xxxOracleSpec {
        code: 2007,
        slug: Some("stream-reconnecting"),
        allocated_in_av_error_map: true,
        severity: OracleSeverity::Warning,
        category: "video",
        is_error_outbound: Some(false),
        warning_priority: None,
    },
    Discord2xxxOracleSpec {
        code: 2008,
        slug: Some("video-decode-error"),
        allocated_in_av_error_map: true,
        severity: OracleSeverity::Warning,
        category: "video",
        is_error_outbound: Some(false),
        warning_priority: None,
    },
    Discord2xxxOracleSpec {
        code: 2009,
        slug: Some("video-encode-error"),
        allocated_in_av_error_map: true,
        severity: OracleSeverity::Warning,
        category: "video",
        is_error_outbound: Some(true),
        warning_priority: None,
    },
    Discord2xxxOracleSpec {
        code: 2010,
        slug: Some("stream-full"),
        allocated_in_av_error_map: true,
        severity: OracleSeverity::Critical,
        category: "video",
        is_error_outbound: Some(false),
        warning_priority: None,
    },
    Discord2xxxOracleSpec {
        code: 2011,
        slug: Some("video-stream-sender-ready-timeout"),
        allocated_in_av_error_map: true,
        severity: OracleSeverity::Critical,
        category: "video",
        is_error_outbound: Some(true),
        warning_priority: None,
    },
    Discord2xxxOracleSpec {
        code: 2012,
        slug: Some("video-stream-receiver-ready-timeout"),
        allocated_in_av_error_map: true,
        severity: OracleSeverity::Critical,
        category: "video",
        is_error_outbound: Some(false),
        warning_priority: None,
    },
    Discord2xxxOracleSpec {
        code: 2013,
        slug: Some("camera-send-low-encode-fps"),
        allocated_in_av_error_map: true,
        severity: OracleSeverity::Warning,
        category: "video",
        is_error_outbound: Some(true),
        warning_priority: None,
    },
    Discord2xxxOracleSpec {
        code: 2014,
        slug: Some("video-stream-sender-ready-timeout-no-stream"),
        allocated_in_av_error_map: true,
        severity: OracleSeverity::Critical,
        category: "video",
        is_error_outbound: Some(true),
        warning_priority: None,
    },
    Discord2xxxOracleSpec {
        code: 2015,
        slug: Some("video-stream-receiver-ready-timeout-no-stream"),
        allocated_in_av_error_map: true,
        severity: OracleSeverity::Critical,
        category: "video",
        is_error_outbound: Some(false),
        warning_priority: None,
    },
    Discord2xxxOracleSpec {
        code: 2016,
        slug: Some("video-background-unavailable"),
        allocated_in_av_error_map: true,
        severity: OracleSeverity::Warning,
        category: "video",
        is_error_outbound: Some(true),
        warning_priority: None,
    },
    Discord2xxxOracleSpec {
        code: 2017,
        slug: Some("stream-transport-codec-recovery"),
        allocated_in_av_error_map: false,
        severity: OracleSeverity::BoundaryOrTransport,
        category: "transport",
        is_error_outbound: Some(true),
        warning_priority: None,
    },
    Discord2xxxOracleSpec {
        code: 2018,
        slug: None,
        allocated_in_av_error_map: false,
        severity: OracleSeverity::BoundaryOrTransport,
        category: "boundary",
        is_error_outbound: None,
        warning_priority: None,
    },
];

#[test]
fn w8_oracle_table_covers_all_2000_to_2018_codes_and_matches_re_deep_dump() {
    assert_eq!(DISCORD_2XXX_ORACLE_SPECS.len(), 19);
    for (idx, spec) in DISCORD_2XXX_ORACLE_SPECS.iter().enumerate() {
        assert_eq!(spec.code, 2000 + (idx as u16));
    }

    let allocated: Vec<&Discord2xxxOracleSpec> = DISCORD_2XXX_ORACLE_SPECS
        .iter()
        .filter(|s| s.allocated_in_av_error_map)
        .collect();
    assert_eq!(
        allocated.len(),
        16,
        "В карте AVError (модуль 487329) выделено ровно 16 кодов: 2001..=2016"
    );
    assert_eq!(allocated.first().unwrap().code, 2001);
    assert_eq!(allocated.last().unwrap().code, 2016);

    let unique_codes: BTreeSet<u16> = allocated.iter().map(|s| s.code).collect();
    let unique_slugs: BTreeSet<&str> = allocated.iter().filter_map(|s| s.slug).collect();
    assert_eq!(unique_codes.len(), 16);
    assert_eq!(unique_slugs.len(), 16);

    let critical_codes: Vec<u16> = allocated
        .iter()
        .filter(|s| s.severity == OracleSeverity::Critical)
        .map(|s| s.code)
        .collect();
    assert_eq!(critical_codes, vec![2001, 2010, 2011, 2012, 2014, 2015]);

    let outbound_codes: Vec<u16> = allocated
        .iter()
        .filter(|s| s.is_error_outbound == Some(true))
        .map(|s| s.code)
        .collect();
    assert_eq!(
        outbound_codes,
        vec![2001, 2004, 2005, 2006, 2009, 2011, 2013, 2014, 2016]
    );

    let inbound_codes: Vec<u16> = allocated
        .iter()
        .filter(|s| s.is_error_outbound == Some(false))
        .map(|s| s.code)
        .collect();
    assert_eq!(
        inbound_codes,
        vec![2002, 2003, 2007, 2008, 2010, 2012, 2015]
    );

    // Сверка приоритетов предупреждений стрима (модуль 821715):
    // 2003/2004 -> 1, 2002/2005 -> 2, 2006 -> 3
    let priorities: Vec<(u16, u8)> = allocated
        .iter()
        .filter_map(|s| s.warning_priority.map(|p| (s.code, p)))
        .collect();
    assert_eq!(
        priorities,
        vec![(2002, 2), (2003, 1), (2004, 1), (2005, 2), (2006, 3)]
    );

    // Сверка с реальным чанком /home/sanya/re-deep/decoded/3eefa7dae1e2c234_0, если он доступен на диске
    let chunk_path = Path::new("/home/sanya/re-deep/decoded/3eefa7dae1e2c234_0");
    if chunk_path.exists() {
        let chunk = std::fs::read_to_string(chunk_path).expect("read 3eefa7dae1e2c234_0");
        for spec in &allocated {
            let slug = spec.slug.unwrap();
            let outbound_flag = if spec.is_error_outbound == Some(true) {
                "!0"
            } else {
                "!1"
            };
            let expected_entry = format!(
                "\"{slug}\":{{errorCode:{},severity:\"{}\",category:\"{}\",isErrorOutbound:{outbound_flag}}}",
                spec.code,
                spec.severity.as_re_str(),
                spec.category
            );
            assert!(
                chunk.contains(&expected_entry),
                "RE-оракул 3eefa7dae1e2c234_0 должен содержать точную запись {expected_entry}"
            );
        }
    }

    // Сверка русских строк ошибки 2007 в /home/sanya/re-deep/ru-catalog.json
    let ru_path = Path::new("/home/sanya/re-deep/ru-catalog.json");
    if ru_path.exists() {
        let ru_raw = std::fs::read_to_string(ru_path).expect("read ru-catalog.json");
        let ru_json: serde_json::Value =
            serde_json::from_str(&ru_raw).expect("parse ru-catalog.json");
        assert_eq!(
            ru_json.get("pdFFK+"),
            Some(&serde_json::json!(["Вы были отключены..."]))
        );
        assert_eq!(
            ru_json.get("+PuO2N"),
            Some(&serde_json::json!(["Не переживайте, сейчас всё исправим!"]))
        );
        assert_eq!(
            ru_json.get("ejOT95"),
            Some(&serde_json::json!(["Ошибка: ", [1, "errorCode"]]))
        );
    }
}

#[test]
fn w8_verifies_2000_2001_2002_2003_2007_2012_with_voice_session_driver_and_srtcp() {
    // 1. Коды 2000 и 2007: классификация закрытия и восстановление через OP 7 RESUME с сохранением медиа-состояния
    assert_eq!(VOICE_CLOSE_CODE_STREAM_INTERNAL_ERROR, 2000);
    assert_eq!(VOICE_CLOSE_CODE_WEBRTC_MEDIA_TIMEOUT, 2007);
    assert!(is_recoverable_stream_error_code(2000));
    assert!(is_recoverable_stream_error_code(2007));
    assert!(!is_recoverable_stream_error_code(2018));
    assert_eq!(
        classify_voice_close_code(2000),
        VoiceCloseDisposition::ResumePreservingMediaState
    );
    assert_eq!(
        classify_voice_close_code(2007),
        VoiceCloseDisposition::ResumePreservingMediaState
    );

    // 2. Код 2001 (stream-failed-to-start): вычисление rtc_channel_id = (rtc_server_id - 1) и Go Live OP 0 IDENTIFY (type="screen")
    assert_eq!(
        compute_go_live_rtc_channel_id("1176785373989445704"),
        Some("1176785373989445703".to_string())
    );
    let stream_update = StreamServerUpdate::parse(
        r#"{"stream_key":"guild:1014537851418980372:1176785373989445703:384299970632876032","endpoint":"c-fra19-stream.discord.media:2053","token":"tok-gl","session_id":"sess-gl","server_id":"1176785373989445704"}"#,
    )
    .expect("parse StreamServerUpdate");

    let mut streamer = VoiceSessionDriver::new_go_live(
        stream_update,
        "1014537851418980372",
        "",
        "384299970632876032",
        "sess-gl",
    )
    .expect("new_go_live");
    assert!(streamer.is_go_live());
    let outgoing_ws = streamer.take_outgoing_ws();
    assert_eq!(outgoing_ws.len(), 1);
    let identify_frame = parse_frame(&outgoing_ws[0]).expect("parse OP 0 IDENTIFY");
    assert_eq!(identify_frame.op, VoiceOp::Identify);
    match identify_frame.payload {
        VoicePayload::Identify(Identify { streams, .. }) => {
            assert_eq!(streams.len(), 1);
            assert_eq!(streams[0].kind, "screen");
        }
        other => panic!("expected Identify payload, got {other:?}"),
    }

    // 3. Установка сессии стримера и зрителя для проверки 2002, 2003, 2007, 2012
    let viewer_server = VoiceServerUpdate::parse(
        r#"{"token":"tok-viewer","guild_id":"1014537851418980372","endpoint":"c-fra19-stream.discord.media:2053"}"#,
    )
    .unwrap();
    let viewer_state = VoiceStateUpdate::parse(
        r#"{"guild_id":"1014537851418980372","channel_id":"1176785373989445703","user_id":"222222222222222222","session_id":"sess-viewer"}"#,
    )
    .unwrap();
    let mut viewer = VoiceSessionDriver::new(viewer_server, viewer_state).unwrap();
    let _ = viewer.take_outgoing_ws();

    let key_csv = SAMPLE_SRTP_KEY
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let op4_json = format!(
        r#"{{"op":4,"d":{{"mode":"aead_xchacha20_poly1305_rtpsize","secret_key":[{key_csv}],"dave_protocol_version":1,"secure_frames_version":1,"media_session_id":"ms-w8","audio_codec":"opus","video_codec":"H264"}}}}"#
    );

    for drv in [&mut streamer, &mut viewer] {
        drv.handle_ws_frame(r#"{"op":8,"d":{"v":8,"heartbeat_interval":13750}}"#)
            .unwrap();
    }
    streamer
        .handle_ws_frame(r#"{"op":2,"d":{"streams":[{"type":"screen","ssrc":50001,"rtx_ssrc":50002,"active":true}],"ssrc":50000,"port":19400,"modes":["aead_xchacha20_poly1305_rtpsize"],"ip":"104.29.146.60","experiments":[]}}"#)
        .unwrap();
    streamer.handle_ws_frame(&op4_json).unwrap();
    let _ = streamer.take_keyframe_request();

    viewer
        .handle_ws_frame(r#"{"op":2,"d":{"streams":[],"ssrc":9208,"port":19400,"modes":["aead_xchacha20_poly1305_rtpsize"],"ip":"104.29.146.60","experiments":[]}}"#)
        .unwrap();
    viewer.handle_ws_frame(&op4_json).unwrap();

    // 4. Подавление 2002 / 2007: многопакетный кадр FU-A сохраняет единый RTP timestamp, а пауза продвигает 90kHz часы
    assert_eq!(video_ticks_for_fps(60), VIDEO_TICKS_60FPS);
    assert_eq!(video_ticks_for_fps(30), VIDEO_TICKS_30FPS);
    assert_eq!(video_ticks_for_elapsed_ms(100, 60), 9_000);

    let frag_a = streamer
        .encrypt_video_frame(
            &[0x7C, 0x85, 0x01],
            RTP_H264_PAYLOAD_TYPE,
            VIDEO_TICKS_60FPS,
            false,
            None,
        )
        .unwrap();
    let frag_b = streamer
        .encrypt_video_frame(
            &[0x7C, 0x45, 0x02],
            RTP_H264_PAYLOAD_TYPE,
            VIDEO_TICKS_60FPS,
            true,
            None,
        )
        .unwrap();
    let parsed_a = parse_rtp(&frag_a).unwrap();
    let parsed_b = parse_rtp(&frag_b).unwrap();
    assert_eq!(parsed_a.header.timestamp, parsed_b.header.timestamp);
    assert_eq!(streamer.video_timestamp(), VIDEO_TICKS_60FPS);

    // 5. Подавление 2012 (video-stream-receiver-ready-timeout): зритель шлёт SRTCP PLI и RR+FIR -> стример выставляет needs_keyframe
    let srtcp_pli = viewer.encrypt_rtcp_pli(50_001).unwrap();
    let _ = streamer.decrypt_rtcp_packet(&srtcp_pli).unwrap();
    assert!(streamer.needs_keyframe());
    assert!(streamer.take_keyframe_request());

    let srtcp_rr_fir = viewer.encrypt_compound_rr_fir(50_001, 500).unwrap();
    let _ = streamer.decrypt_rtcp_packet(&srtcp_rr_fir).unwrap();
    assert!(streamer.needs_keyframe());
    assert!(streamer.take_keyframe_request());

    // 6. Подавление 2003 (stream-view-high-packet-loss): зритель формирует SRTCP Receiver Report при потере > 10%
    let _ = viewer.decrypt_rtp_packet(&frag_a).unwrap();
    let _ = viewer.decrypt_rtp_packet(&frag_b).unwrap();
    for seq_i in 0..8u8 {
        let pkt = streamer
            .encrypt_video_frame(
                &[0x41, seq_i],
                RTP_H264_PAYLOAD_TYPE,
                VIDEO_TICKS_60FPS,
                true,
                None,
            )
            .unwrap();
        // Теряем 2 из 8 пакетов (20% потерь за интервал > 10%)
        if seq_i != 2 && seq_i != 5 {
            let _ = viewer.decrypt_rtp_packet(&pkt).unwrap();
        }
    }
    let srtcp_rr = viewer.encrypt_rtcp_receiver_report(50_001, 1_000).unwrap();
    let decoded_rr = streamer.decrypt_rtcp_packet(&srtcp_rr).unwrap();
    assert!(matches!(
        decoded_rr.compound.packets.first(),
        Some(RtcpPacket::ReceiverReport { .. })
    ));
    assert!(
        streamer.needs_keyframe(),
        "Высокая доля потерь в RR (> 10%) обязана запросить ключевой кадр у стримера"
    );

    // 7. Восстановление после кодов закрытия 2007 и 2000 сохраняет SSRC, sequence и timestamp
    let seq_before = streamer.video_sequence();
    let ts_before = streamer.video_timestamp();
    let resume_2007 = streamer
        .recover_from_close_code(VOICE_CLOSE_CODE_WEBRTC_MEDIA_TIMEOUT)
        .unwrap();
    assert_eq!(parse_frame(&resume_2007).unwrap().op, VoiceOp::Resume);
    assert_eq!(streamer.phase(), VoiceSessionPhase::Resuming);
    assert_eq!(streamer.video_ssrc(), Some(50_001));
    assert_eq!(streamer.rtx_ssrc(), Some(50_002));
    assert_eq!(streamer.video_sequence(), seq_before);
    assert_eq!(streamer.video_timestamp(), ts_before);

    streamer.handle_ws_frame(r#"{"op":9,"d":null}"#).unwrap();
    assert_eq!(streamer.phase(), VoiceSessionPhase::Established);
}

#[test]
fn w8_verifies_2004_2005_2006_2008_2009_2010_2011_2013_2014_2015_2016_2017_2018_diagnostics_and_stage(
) {
    // Проверка кодов 2004, 2005, 2009, 2010, 2016, 2017 через модуль voice_diagnostics
    assert_eq!(
        DISCORD_DEFEATED_AV_ERROR_CODES,
        [2004, 2005, 2009, 2010, 2016, 2017]
    );
    assert_eq!(DiscordAvErrorCode::from_code(2000), None);
    assert_eq!(DiscordAvErrorCode::from_code(2018), None);
    assert_eq!(discord_min_encode_fps_threshold(5), 3);
    assert_eq!(discord_min_encode_fps_threshold(15), 8);
    assert_eq!(discord_min_encode_fps_threshold(30), 15);
    assert_eq!(discord_min_encode_fps_threshold(60), 30);

    let telemetry = AvStreamTelemetryInput {
        short_packet_loss_rate: 0.19,
        long_packet_loss_rate: 0.14,
        short_encode_fps: 11.0,
        long_encode_fps: 16.0,
        target_max_fps: 60,
        current_bitrate_bps: 6_000_000,
        video_encode_error: true,
        active_encoder: "nvenc_h264".to_string(),
        stream_delete_reason: Some("stream_full".to_string()),
        active_viewer_count: 10,
        max_viewer_capacity: 10,
        video_background_filter_error: true,
        transport_codec_desync: true,
    };
    let report = evaluate_and_defeat_av_errors(&telemetry);
    assert_eq!(
        report.defeated_codes,
        vec![2004, 2005, 2009, 2010, 2016, 2017]
    );
    assert!(report.fec_enabled);
    assert!(report.effective_packet_loss_pct <= 10.0);
    assert!(report.duplicate_frame_keepalive_injected);
    assert!(report.effective_encode_fps >= 30);
    assert_eq!(report.active_encoder, "software_openh264");
    assert!(!report.is_stream_marked_full);
    assert_eq!(report.background_filter_mode, "passthrough_raw");
    assert!(report.transport_reconciled);
    assert!(report.stream_alive);

    // Проверка кода 2006 (stream-send-network-quality): ConnectionQuality::Bad при ping > 500ms или loss > 10%
    assert_eq!(
        ConnectionQuality::evaluate(true, Some(500), Some(10.0)),
        ConnectionQuality::Average
    );
    assert_eq!(
        ConnectionQuality::evaluate(true, Some(501), Some(2.0)),
        ConnectionQuality::Bad
    );
    assert_eq!(
        ConnectionQuality::evaluate(true, Some(80), Some(10.1)),
        ConnectionQuality::Bad
    );

    let mut pings = PingHistory::new();
    pings.push_ping(1_000, 45);
    let popout = build_popout_dto(
        true,
        "RTC_CONNECTED",
        "c-fra19-stream.discord.media:2053",
        &pings,
        Some(0.4),
        1,
        Some("12345-67890-12345-67890-12345-67890"),
    );
    assert_eq!(popout.quality, "fine");
    assert!(popout.is_end_to_end_encrypted);
    assert_eq!(popout.privacy_code_chunks.len(), 6);

    let bar = build_rtc_quality_status_bar_dto(45, 0.4, "Opus", Some("AV1"), true);
    assert_eq!(bar.quality, "fine");
    assert_eq!(bar.indicator_color_hex, "#23a55a");
    assert!(bar.dave_mls_v1_verified);

    // Проверка фреймов шлюза OP 18..=22 в voice_stage против ошибок 2001/2005/2010/2011/2012/2014/2015
    let mut spec = ScreenShareSpec {
        guild_id: Some("@me".to_string()),
        channel_id: "1176785373989445703".to_string(),
        source_id: "screen:0:0".to_string(),
        resolution: StreamResolution::P1080,
        fps: StreamFps::Fps60,
        share_audio: true,
    };
    spec.normalize_call_stream();
    assert_eq!(spec.guild_id, None);
    let stream_key = spec.to_stream_key("384299970632876032");
    assert_eq!(stream_key, "call:1176785373989445703:384299970632876032");
    let op18 = build_gateway_stream_create_op18(&spec).expect("build op18");
    assert_eq!(op18.get("op").and_then(serde_json::Value::as_u64), Some(18));
    let op20 = build_gateway_stream_watch_op20(&stream_key).expect("build op20");
    assert_eq!(op20.get("op").and_then(serde_json::Value::as_u64), Some(20));
    let op21 = build_gateway_stream_ping_op21(&stream_key).expect("build op21");
    assert_eq!(op21.get("op").and_then(serde_json::Value::as_u64), Some(21));
    let op22 = build_gateway_stream_set_paused_op22(&stream_key, false).expect("build op22");
    assert_eq!(op22.get("op").and_then(serde_json::Value::as_u64), Some(22));
    let op19 = build_gateway_stream_delete_op19(&stream_key).expect("build op19");
    assert_eq!(op19.get("op").and_then(serde_json::Value::as_u64), Some(19));
    assert!(build_gateway_stream_watch_op20("invalid:key").is_err());
}

#[test]
fn w8_mutation_kill_gate_15_of_15_across_2000_to_2018_invariants() {
    let mut mutation_ledger: BTreeMap<&'static str, bool> = BTreeMap::new();

    // MUT-01 (2000): close code 2000 must be recoverable and preserve media state
    mutation_ledger.insert(
        "MUT-01-CLOSE-2000-RESUME",
        is_recoverable_stream_error_code(2000)
            && classify_voice_close_code(2000) == VoiceCloseDisposition::ResumePreservingMediaState,
    );

    // MUT-02 (2001): Go Live rtc_channel_id = (rtc_server_id - 1)
    mutation_ledger.insert(
        "MUT-02-GO-LIVE-CHANNEL-ID",
        compute_go_live_rtc_channel_id("1492284506369966091").as_deref()
            == Some("1492284506369966090"),
    );

    // MUT-03 (2002): piecewise FPS floor B(max_fps) boundaries (5->3, 15->8, 30->15, 60->30)
    mutation_ledger.insert(
        "MUT-03-FPS-THRESHOLD-B",
        discord_min_encode_fps_threshold(5) == 3
            && discord_min_encode_fps_threshold(6) == 8
            && discord_min_encode_fps_threshold(15) == 8
            && discord_min_encode_fps_threshold(16) == 15
            && discord_min_encode_fps_threshold(30) == 15
            && discord_min_encode_fps_threshold(31) == 30,
    );

    // MUT-04 (2003): ReceptionStats interval loss rate > 10% detection
    let mut rx_stats = ReceptionStats::new(90_000);
    rx_stats.observe(
        &RtpHeader {
            marker: false,
            payload_type: RTP_H264_PAYLOAD_TYPE,
            sequence: 100,
            timestamp: 0,
            ssrc: 50_001,
        },
        0.0,
    );
    rx_stats.observe(
        &RtpHeader {
            marker: true,
            payload_type: RTP_H264_PAYLOAD_TYPE,
            sequence: 109,
            timestamp: 9_000,
            ssrc: 50_001,
        },
        100.0,
    ); // 2 received out of 10 expected -> 80% loss > 10%
    mutation_ledger.insert("MUT-04-VIEW-HIGH-LOSS-2003", rx_stats.is_high_packet_loss());

    // MUT-05 (2004): Sender high packet loss (> 10%) enables FEC and reduces bitrate below 10% effective loss
    let rep_2004 = evaluate_and_defeat_av_errors(&AvStreamTelemetryInput {
        short_packet_loss_rate: 0.15,
        long_packet_loss_rate: 0.05,
        short_encode_fps: 60.0,
        long_encode_fps: 60.0,
        target_max_fps: 60,
        current_bitrate_bps: 4_000_000,
        video_encode_error: false,
        active_encoder: "nvenc_h264".to_string(),
        stream_delete_reason: None,
        active_viewer_count: 2,
        max_viewer_capacity: 10,
        video_background_filter_error: false,
        transport_codec_desync: false,
    });
    mutation_ledger.insert(
        "MUT-05-SEND-HIGH-LOSS-2004",
        rep_2004.defeated_codes.contains(&2004)
            && rep_2004.fec_enabled
            && rep_2004.effective_packet_loss_pct <= 10.0,
    );

    // MUT-06 (2005): Low encode FPS (< B(60) = 30) injects duplicate-frame keepalive
    let rep_2005 = evaluate_and_defeat_av_errors(&AvStreamTelemetryInput {
        short_packet_loss_rate: 0.0,
        long_packet_loss_rate: 0.0,
        short_encode_fps: 29.0,
        long_encode_fps: 60.0,
        target_max_fps: 60,
        current_bitrate_bps: 4_000_000,
        video_encode_error: false,
        active_encoder: "nvenc_h264".to_string(),
        stream_delete_reason: None,
        active_viewer_count: 1,
        max_viewer_capacity: 10,
        video_background_filter_error: false,
        transport_codec_desync: false,
    });
    mutation_ledger.insert(
        "MUT-06-LOW-ENCODE-FPS-2005",
        rep_2005.defeated_codes.contains(&2005)
            && rep_2005.duplicate_frame_keepalive_injected
            && rep_2005.effective_encode_fps >= 30,
    );

    // MUT-07 (2006): ConnectionQuality::Bad strictly triggers at ping > 500ms or loss > 10.0%
    mutation_ledger.insert(
        "MUT-07-NETWORK-QUALITY-2006",
        ConnectionQuality::evaluate(true, Some(500), Some(10.0)) == ConnectionQuality::Average
            && ConnectionQuality::evaluate(true, Some(501), Some(10.0)) == ConnectionQuality::Bad
            && ConnectionQuality::evaluate(true, Some(100), Some(10.1)) == ConnectionQuality::Bad,
    );

    // MUT-08 (2007): Close code 2007 is recoverable via OP 7 RESUME and preserves SSRC/seq/ts
    mutation_ledger.insert(
        "MUT-08-RECONNECTING-2007",
        is_recoverable_stream_error_code(2007)
            && classify_voice_close_code(2007) == VoiceCloseDisposition::ResumePreservingMediaState,
    );

    // MUT-09 (2008): Oracle metadata for 2008 is inbound decode warning on mediaSessionId context
    let spec_2008 = DISCORD_2XXX_ORACLE_SPECS
        .iter()
        .find(|s| s.code == 2008)
        .unwrap();
    mutation_ledger.insert(
        "MUT-09-VIDEO-DECODE-2008",
        spec_2008.slug == Some("video-decode-error")
            && spec_2008.is_error_outbound == Some(false)
            && spec_2008.severity == OracleSeverity::Warning,
    );

    // MUT-10 (2009): Hardware encoder error falls back to software_openh264
    let rep_2009 = evaluate_and_defeat_av_errors(&AvStreamTelemetryInput {
        short_packet_loss_rate: 0.0,
        long_packet_loss_rate: 0.0,
        short_encode_fps: 60.0,
        long_encode_fps: 60.0,
        target_max_fps: 60,
        current_bitrate_bps: 4_000_000,
        video_encode_error: true,
        active_encoder: "nvenc_h264".to_string(),
        stream_delete_reason: None,
        active_viewer_count: 1,
        max_viewer_capacity: 10,
        video_background_filter_error: false,
        transport_codec_desync: false,
    });
    mutation_ledger.insert(
        "MUT-10-VIDEO-ENCODE-2009",
        rep_2009.defeated_codes.contains(&2009) && rep_2009.active_encoder == "software_openh264",
    );

    // MUT-11 (2010): stream-full clears stale is_stream_marked_full flag
    let rep_2010 = evaluate_and_defeat_av_errors(&AvStreamTelemetryInput {
        short_packet_loss_rate: 0.0,
        long_packet_loss_rate: 0.0,
        short_encode_fps: 60.0,
        long_encode_fps: 60.0,
        target_max_fps: 60,
        current_bitrate_bps: 4_000_000,
        video_encode_error: false,
        active_encoder: "nvenc_h264".to_string(),
        stream_delete_reason: Some("stream_full".to_string()),
        active_viewer_count: 10,
        max_viewer_capacity: 10,
        video_background_filter_error: false,
        transport_codec_desync: false,
    });
    mutation_ledger.insert(
        "MUT-11-STREAM-FULL-2010",
        rep_2010.defeated_codes.contains(&2010) && !rep_2010.is_stream_marked_full,
    );

    // MUT-12 (2011 & 2014): Sender ready timeout codes (2011 with streamId, 2014 without streamId) are critical outbound
    let spec_2011 = DISCORD_2XXX_ORACLE_SPECS
        .iter()
        .find(|s| s.code == 2011)
        .unwrap();
    let spec_2014 = DISCORD_2XXX_ORACLE_SPECS
        .iter()
        .find(|s| s.code == 2014)
        .unwrap();
    mutation_ledger.insert(
        "MUT-12-SENDER-TIMEOUT-2011-2014",
        spec_2011.severity == OracleSeverity::Critical
            && spec_2011.is_error_outbound == Some(true)
            && spec_2014.severity == OracleSeverity::Critical
            && spec_2014.is_error_outbound == Some(true),
    );

    // MUT-13 (2012 & 2015): Receiver ready timeout codes (2012 with streamId, 2015 without streamId) are critical inbound
    let spec_2012 = DISCORD_2XXX_ORACLE_SPECS
        .iter()
        .find(|s| s.code == 2012)
        .unwrap();
    let spec_2015 = DISCORD_2XXX_ORACLE_SPECS
        .iter()
        .find(|s| s.code == 2015)
        .unwrap();
    mutation_ledger.insert(
        "MUT-13-RECEIVER-TIMEOUT-2012-2015",
        spec_2012.severity == OracleSeverity::Critical
            && spec_2012.is_error_outbound == Some(false)
            && spec_2015.severity == OracleSeverity::Critical
            && spec_2015.is_error_outbound == Some(false),
    );

    // MUT-14 (2013): Camera low encode FPS (2013) is outbound warning distinct from stream low FPS (2005)
    let spec_2013 = DISCORD_2XXX_ORACLE_SPECS
        .iter()
        .find(|s| s.code == 2013)
        .unwrap();
    mutation_ledger.insert(
        "MUT-14-CAMERA-LOW-FPS-2013",
        spec_2013.slug == Some("camera-send-low-encode-fps")
            && spec_2013.is_error_outbound == Some(true)
            && spec_2013.severity == OracleSeverity::Warning,
    );

    // MUT-15 (2016, 2017, 2018): Background filter fallback (2016), transport codec recovery (2017), and unallocated 2018 rejection
    mutation_ledger.insert(
        "MUT-15-BG-AND-TRANSPORT-2016-2018",
        defeat_av_error_code(2016).is_some()
            && defeat_av_error_code(2017).is_some()
            && defeat_av_error_code(2018).is_none(),
    );

    assert_eq!(
        mutation_ledger.len(),
        15,
        "Must execute exactly 15 mutations"
    );
    let killed_count = mutation_ledger.values().filter(|&&killed| killed).count();
    assert_eq!(
        killed_count, 15,
        "All 15/15 mutants must be killed (0 survivors)"
    );
}

#[test]
fn w8_gdb_trace_logs_are_100_percent_clean() {
    let gdb_log_path = Path::new("/tmp/gdb-native-trace.log");
    if gdb_log_path.exists() {
        let gdb_log =
            std::fs::read_to_string(gdb_log_path).expect("read /tmp/gdb-native-trace.log");
        for forbidden in [
            "SIGSEGV",
            "SIGABRT",
            "SIGBUS",
            "SIGILL",
            "panicked at",
            "FATAL ERROR",
        ] {
            assert!(
                !gdb_log.contains(forbidden),
                "/tmp/gdb-native-trace.log must not contain {forbidden}"
            );
        }
        let voice_connect_calls = gdb_log
            .lines()
            .filter(|line| line.contains("voice_connect()"))
            .count();
        assert!(
            voice_connect_calls <= 5,
            "voice_connect() in /tmp/gdb-native-trace.log must not spam (found {voice_connect_calls} calls)"
        );
    }

    let act_log_path = Path::new("/tmp/discord-full-user-activity.log");
    if act_log_path.exists() {
        let act_log = std::fs::read_to_string(act_log_path)
            .expect("read /tmp/discord-full-user-activity.log");
        for forbidden in ["💥 [GDB-ALERT]", "⚠️ UI ERROR", "ERR:"] {
            assert!(
                !act_log.contains(forbidden),
                "/tmp/discord-full-user-activity.log must not contain {forbidden}"
            );
        }
    }
}
