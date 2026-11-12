//! Контракт RTCP-парсера (P4-07, B07/RW-26).
//!
//! Провенанс реальных байтов:
//! - `/home/sanya/re-deep/voice/voice-udp4.pcap`
//!   sha256 `0e6e2f8ce16602b3af567e9e033d7721f194688cd3d0cc4aaae8b47d9d40e1e2`;
//! - разбор: `python3 waves/RW-26/research/rtcp-extract.py <pcap> <out> --fixture 20`;
//! - `waves/RW-26/research/fixtures/rtcp-voice-udp4-server.bin`
//!   md5 `795d0af01973d875cb7810abdd066122`.
//!
//! Реальный RR: 32 Б открытого RTCP + 16 Б SRTCP tag. Тело report block зашифровано,
//! поэтому значения block'а проверяются только на синтетическом векторе по RFC 3550.

use chord_voice::rtcp::{
    Compound, MAX_COMPOUND_PACKETS, RTCP_APP, RTCP_BYE, RTCP_RR, RTCP_SDES, RTCP_SR, ReportBlock,
    RtcpError, RtcpPacket, SenderInfo, parse_compound,
};

fn decode_hex(value: &str) -> Vec<u8> {
    let bytes = value.as_bytes();
    assert_eq!(bytes.len() % 2, 0, "hex fixture must have an even length");
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let digit = |byte| match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                _ => panic!("bad test hex"),
            };
            (digit(pair[0]) << 4) | digit(pair[1])
        })
        .collect()
}

/// Первый RR сервера из capture: `81 c9 00 07` + SSRC + 24 Б шифроблока + tag 16 Б.
const REAL_RR_WITH_TAG: &str = "81c90007000023f8\
b58acbc1675a4d35a0ac12ccf8f151bc0f002888166d180d\
33c307bba4d3e9d3fc94880e9397111d";

/// RR из сессии 2 (`voice-udp3.pcap`, sha256 `b655c13a…495926f3`): 32 Б RTCP +
/// 20 Б SRTCP-трейлера (tag 16 + counter 4). Вторая наблюдаемая форма трейлера.
const SESSION2_RR_WITH_TRAILER: &str = "81c90007000045b3\
709ae0b097fff15a517444cc624b7475293767ee9d509856\
18972aff7ae3ee312c988a68ed1ec20100000000";

/// Первый SR клиента из capture: 28 Б открытого RTCP + tag 16 Б + counter 4 Б LE.
/// Поле sender info зашифровано (SRTCP), поэтому его значения — структурные, не
/// семантические; тест проверяет порядок байт и NTP-склейку, а не реальные часы.
const REAL_SR_WITH_TAG: &str = "80c80006000023f8\
8abb34e79b75ca2c3872445b001d6ca27b9c0b49\
20730f08afde72c4e657d80674ed1322\
3fc1383a";

fn synthetic_rr() -> Vec<u8> {
    let mut bytes = vec![0x81, RTCP_RR, 0x00, 0x07];
    bytes.extend_from_slice(&0xdead_beefu32.to_be_bytes());
    bytes.extend_from_slice(&0x1122_3344u32.to_be_bytes());
    bytes.push(64);
    bytes.extend_from_slice(&[0xff, 0xff, 0xff]); // cumulative_lost = -1 (24-bit signed)
    bytes.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    bytes.extend_from_slice(&0x0000_1234u32.to_be_bytes());
    bytes.extend_from_slice(&0x1111_2222u32.to_be_bytes());
    bytes.extend_from_slice(&0x0000_8000u32.to_be_bytes());
    bytes
}

#[test]
fn real_receiver_report_keeps_encrypted_block_and_detects_trailer() {
    let bytes = decode_hex(REAL_RR_WITH_TAG);
    let compound = parse_compound(&bytes).expect("real RR must parse");
    assert_eq!(
        compound,
        Compound {
            packets: vec![RtcpPacket::ReceiverReport {
                sender_ssrc: 0x0000_23f8,
                reports: vec![ReportBlock {
                    ssrc: 0xb58a_cbc1,
                    fraction_lost: 0x67,
                    cumulative_lost: 0x5a4d35,
                    extended_highest_seq: 0xa0ac_12cc,
                    jitter: 0xf8f1_51bc,
                    last_sr: 0x0f00_2888,
                    delay_since_last_sr: 0x166d_180d,
                }],
            }],
            trailer_len: 16,
        }
    );
}

#[test]
fn session2_receiver_report_has_20_byte_trailer() {
    let bytes = decode_hex(SESSION2_RR_WITH_TRAILER);
    let compound = parse_compound(&bytes).expect("session-2 RR must parse");
    assert_eq!(compound.trailer_len, 20);
    let [
        RtcpPacket::ReceiverReport {
            sender_ssrc,
            reports,
        },
    ] = compound.packets.as_slice()
    else {
        panic!("expected one RR, got {:?}", compound.packets);
    };
    assert_eq!(*sender_ssrc, 0x0000_45b3);
    assert_eq!(reports.len(), 1);
}

#[test]
fn real_sender_report_parses_sender_info_and_20_byte_trailer() {
    let bytes = decode_hex(REAL_SR_WITH_TAG);
    let compound = parse_compound(&bytes).expect("real SR must parse");
    assert_eq!(compound.trailer_len, 20);
    let [
        RtcpPacket::SenderReport {
            sender_ssrc,
            info,
            reports,
        },
    ] = compound.packets.as_slice()
    else {
        panic!("expected exactly one SR, got {:?}", compound.packets);
    };
    assert_eq!(*sender_ssrc, 0x0000_23f8);
    assert_eq!(
        *info,
        SenderInfo {
            ntp_timestamp: 0x8abb_34e7_9b75_ca2c,
            rtp_timestamp: 0x3872_445b,
            packet_count: 0x001d_6ca2,
            octet_count: 0x7b9c_0b49,
        }
    );
    assert!(reports.is_empty());
}

#[test]
fn synthetic_report_block_uses_24_bit_signed_cumulative_loss() {
    let bytes = synthetic_rr();
    let compound = parse_compound(&bytes).expect("synthetic RR must parse");
    assert_eq!(compound.trailer_len, 0);
    let [
        RtcpPacket::ReceiverReport {
            sender_ssrc,
            reports,
        },
    ] = compound.packets.as_slice()
    else {
        panic!("expected RR, got {:?}", compound.packets);
    };
    assert_eq!(*sender_ssrc, 0xdead_beef);
    assert_eq!(reports.len(), 1);
    assert_eq!(
        reports[0],
        ReportBlock {
            ssrc: 0x1122_3344,
            fraction_lost: 64,
            cumulative_lost: -1,
            extended_highest_seq: 0x0001_0000,
            jitter: 0x0000_1234,
            last_sr: 0x1111_2222,
            delay_since_last_sr: 0x0000_8000,
        }
    );
}

#[test]
fn cumulative_loss_extremes_are_sign_extended() {
    for (raw, expected) in [
        ([0x00, 0x00, 0x01], 1),
        ([0x7f, 0xff, 0xff], 8_388_607),
        ([0xff, 0xff, 0xfe], -2),
        ([0x80, 0x00, 0x00], -8_388_608),
    ] {
        let mut bytes = vec![0x81, RTCP_RR, 0x00, 0x07];
        bytes.extend_from_slice(&1u32.to_be_bytes());
        bytes.extend_from_slice(&2u32.to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&raw);
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&[0; 4]);
        let compound = parse_compound(&bytes).unwrap();
        let RtcpPacket::ReceiverReport { reports, .. } = &compound.packets[0] else {
            panic!("expected RR");
        };
        assert_eq!(reports[0].cumulative_lost, expected);
    }
}

#[test]
fn compound_report_and_sdes_are_both_parsed() {
    let mut bytes = synthetic_rr();
    // SDES: V=2, RC=1, PT=202, length=2 (12 Б), SSRC + CNAME "a".
    bytes.extend_from_slice(&[0x81, RTCP_SDES, 0x00, 0x02]);
    bytes.extend_from_slice(&0x0000_00aau32.to_be_bytes());
    bytes.extend_from_slice(&[0x01, 0x01, b'a', 0x00]);
    let compound = parse_compound(&bytes).expect("compound must parse");
    assert_eq!(compound.trailer_len, 0);
    assert_eq!(compound.packets.len(), 2);
    assert!(matches!(
        &compound.packets[1],
        RtcpPacket::Other {
            payload_type: RTCP_SDES,
            report_count: 1,
            body,
        } if body.len() == 8
    ));
}

#[test]
fn unknown_payload_types_are_opaque_not_errors() {
    let bytes = [0x80, RTCP_APP, 0x00, 0x01, 1, 2, 3, 4];
    let compound = parse_compound(&bytes).expect("APP must parse");
    assert!(matches!(
        &compound.packets[0],
        RtcpPacket::Other {
            payload_type: RTCP_APP,
            ..
        }
    ));
    let bye = [0x80, RTCP_BYE, 0x00, 0x01, 1, 2, 3, 4];
    assert!(matches!(
        &parse_compound(&bye).unwrap().packets[0],
        RtcpPacket::Other {
            payload_type: RTCP_BYE,
            ..
        }
    ));
}

#[test]
fn empty_input_is_an_empty_compound() {
    assert_eq!(
        parse_compound(&[]).unwrap(),
        Compound {
            packets: vec![],
            trailer_len: 0
        }
    );
}

#[test]
fn wrong_version_on_first_packet_is_an_error() {
    assert_eq!(
        parse_compound(&[0x40, RTCP_RR, 0, 1]),
        Err(RtcpError::InvalidVersion(1))
    );
}

#[test]
fn truncated_packet_reports_required_size() {
    let mut bytes = synthetic_rr();
    bytes.truncate(20);
    assert_eq!(
        parse_compound(&bytes),
        Err(RtcpError::Truncated {
            needed: 32,
            available: 20
        })
    );
}

#[test]
fn padding_bit_is_rejected_explicitly() {
    let mut bytes = synthetic_rr();
    bytes[0] |= 0x20;
    assert_eq!(parse_compound(&bytes), Err(RtcpError::UnsupportedPadding));
}

#[test]
fn impossible_length_for_receiver_report_is_rejected() {
    // RR с length=0: 4 Б < минимальных 8.
    let bytes = [0x80, RTCP_RR, 0x00, 0x00];
    assert_eq!(
        parse_compound(&bytes),
        Err(RtcpError::InvalidLength {
            payload_type: RTCP_RR,
            length_words: 0
        })
    );
}

#[test]
fn compound_packet_limit_is_enforced() {
    let one = synthetic_rr();
    let mut bytes = Vec::new();
    for _ in 0..=MAX_COMPOUND_PACKETS {
        bytes.extend_from_slice(&one);
    }
    assert_eq!(parse_compound(&bytes), Err(RtcpError::TooManyPackets));
}

#[test]
fn compound_at_the_limit_is_accepted() {
    let one = synthetic_rr();
    let mut bytes = Vec::new();
    for _ in 0..MAX_COMPOUND_PACKETS {
        bytes.extend_from_slice(&one);
    }
    let compound = parse_compound(&bytes).expect("exactly the limit must be accepted");
    assert_eq!(compound.packets.len(), MAX_COMPOUND_PACKETS);
}

#[test]
fn trailer_that_starts_with_version_two_is_not_mistaken_for_rtcp() {
    // 32 Б RR + 16 Б tag, где первый байт tag случайно 0x80: длина «пакета» выходит
    // за остаток, но остаток ровно 16 — это трейлер, а не Truncated.
    let mut bytes = synthetic_rr();
    bytes.extend_from_slice(&[0x80, 0xff, 0xff, 0xff]);
    bytes.extend_from_slice(&[0; 12]);
    let compound = parse_compound(&bytes).expect("tag must be treated as trailer");
    assert_eq!(compound.packets.len(), 1);
    assert_eq!(compound.trailer_len, 16);
}

#[test]
fn sender_report_without_sender_info_is_invalid() {
    let bytes = [0x80, RTCP_SR, 0x00, 0x01, 1, 2, 3, 4];
    assert_eq!(
        parse_compound(&bytes),
        Err(RtcpError::InvalidLength {
            payload_type: RTCP_SR,
            length_words: 1
        })
    );
}

#[test]
fn multi_block_receiver_reports_and_compound_rr_pli_fir_roundtrip() {
    use chord_voice::rtcp::{
        FirEntry, KeyframeRequestKind, RTCP_EMPTY_RR_LEN, RTCP_FMT_FIR, RTCP_FMT_PLI,
        RTCP_MAX_REPORT_BLOCKS, RTCP_PSFB, RtcpFir, RtcpKeyframeController, RtcpPli,
        build_compound_rr_fir, build_compound_rr_pli, build_receiver_reports,
        build_srtcp_compound_rr_fir, build_srtcp_compound_rr_pli, parse_receiver_reports,
    };
    use chord_voice::srtp::{EncryptionMode, SrtpContext};

    // 1. Пустой Receiver Report (RC=0, 8 байт)
    let empty_rr = build_receiver_reports(9208, &[]).expect("empty RR (RC=0)");
    assert_eq!(empty_rr.len(), RTCP_EMPTY_RR_LEN);
    assert_eq!(
        empty_rr,
        [0x80, RTCP_RR, 0x00, 0x01, 0x00, 0x00, 0x23, 0xf8]
    );
    let (sender, blocks) = parse_receiver_reports(&empty_rr).expect("parse empty RR");
    assert_eq!(sender, 9208);
    assert!(blocks.is_empty());

    // 2. Мульти-блочный Receiver Report (RC=2: аудио SSRC + видео SSRC)
    let rb_audio = ReportBlock {
        ssrc: 40_000,
        fraction_lost: 0,
        cumulative_lost: 0,
        extended_highest_seq: 1_200,
        jitter: 45,
        last_sr: 0x1111_2222,
        delay_since_last_sr: 65_536,
    };
    let rb_video = ReportBlock {
        ssrc: 40_001,
        fraction_lost: 32,
        cumulative_lost: 4,
        extended_highest_seq: 9_876,
        jitter: 180,
        last_sr: 0x3333_4444,
        delay_since_last_sr: 32_768,
    };
    let multi_rr = build_receiver_reports(9208, &[rb_audio, rb_video]).expect("RC=2 RR");
    assert_eq!(multi_rr.len(), 8 + 2 * 24);
    assert_eq!(multi_rr[0], 0x82); // V=2, P=0, RC=2
    assert_eq!(multi_rr[1], RTCP_RR);
    let (parsed_sender, parsed_blocks) =
        parse_receiver_reports(&multi_rr).expect("parse multi-block RR");
    assert_eq!(parsed_sender, 9208);
    assert_eq!(parsed_blocks, vec![rb_audio, rb_video]);

    // 3. Граница RC=31 проходит, а RC=32 отклоняется
    let max_blocks = vec![rb_audio; RTCP_MAX_REPORT_BLOCKS];
    let rr_31 = build_receiver_reports(9208, &max_blocks).expect("RC=31 valid");
    assert_eq!(rr_31[0], 0x80 | 31);
    let over_blocks = vec![rb_audio; RTCP_MAX_REPORT_BLOCKS + 1];
    assert!(build_receiver_reports(9208, &over_blocks).is_err());

    // 4. Составной пакет RR + PLI (PT 201 + PT 206 FMT 1) и RR + FIR (PT 206, FMT 4)
    let comp_pli = build_compound_rr_pli(9208, Some(&rb_video), 40_001);
    assert_eq!(comp_pli.len(), 32 + 12);
    assert_eq!(comp_pli[32], 0x80 | RTCP_FMT_PLI);
    assert_eq!(comp_pli[33], RTCP_PSFB);
    let parsed_comp_pli = parse_compound(&comp_pli).expect("parse compound RR+PLI");
    assert_eq!(
        parsed_comp_pli.packets,
        vec![
            RtcpPacket::ReceiverReport {
                sender_ssrc: 9208,
                reports: vec![rb_video],
            },
            RtcpPacket::Pli(RtcpPli {
                sender_ssrc: 9208,
                media_ssrc: 40_001,
            }),
        ]
    );

    let comp_fir = build_compound_rr_fir(9208, Some(&rb_video), 40_001, 7);
    assert_eq!(comp_fir.len(), 32 + 20);
    assert_eq!(comp_fir[32], 0x80 | RTCP_FMT_FIR);
    assert_eq!(comp_fir[33], RTCP_PSFB);
    let parsed_comp_fir = parse_compound(&comp_fir).expect("parse compound RR+FIR");
    assert_eq!(
        parsed_comp_fir.packets,
        vec![
            RtcpPacket::ReceiverReport {
                sender_ssrc: 9208,
                reports: vec![rb_video],
            },
            RtcpPacket::Fir(RtcpFir {
                sender_ssrc: 9208,
                media_ssrc: 0,
                entries: vec![FirEntry {
                    ssrc: 40_001,
                    sequence_number: 7,
                }],
            }),
        ]
    );

    // 5. Шифрование и расшифровка SRTCP для составных RR+PLI и RR+FIR
    let key = [0x42u8; 32];
    let mut tx_srtp = SrtpContext::from_secret_key(&key, EncryptionMode::XChaCha20Poly1305RtpSize);
    let mut rx_srtp = SrtpContext::from_secret_key(&key, EncryptionMode::XChaCha20Poly1305RtpSize);
    let srtcp_pli = build_srtcp_compound_rr_pli(&mut tx_srtp, 9208, Some(&rb_video), 40_001)
        .expect("srtcp pli");
    let clear_pli = rx_srtp
        .decrypt_srtcp(&srtcp_pli)
        .expect("decrypt srtcp pli");
    assert_eq!(clear_pli, comp_pli);

    let srtcp_fir = build_srtcp_compound_rr_fir(&mut tx_srtp, 9208, Some(&rb_video), 40_001, 7)
        .expect("srtcp fir");
    let clear_fir = rx_srtp
        .decrypt_srtcp(&srtcp_fir)
        .expect("decrypt srtcp fir");
    assert_eq!(clear_fir, comp_fir);

    // 6. RtcpKeyframeController: авто-инкремент FIR sequence_number (0..=255 -> 0) и дедупликация RFC 5104 §4.3.1.2
    let mut ctrl = RtcpKeyframeController::new(9208, 9209);
    let fir_pkt_0 = ctrl.build_fir_packet(40_001);
    assert_eq!(fir_pkt_0[16], 0);
    let fir_pkt_1 = ctrl.build_fir_packet(40_001);
    assert_eq!(fir_pkt_1[16], 1);

    // На стороне отправителя (local_video_ssrc = 9209):
    // Входящий PLI для чужого SSRC игнорируется, а для 9209 выставляет запрос ключевого кадра
    assert!(!ctrl.on_rtcp_packet(&RtcpPacket::Pli(RtcpPli {
        sender_ssrc: 55_000,
        media_ssrc: 77_777,
    })));
    assert!(!ctrl.is_keyframe_requested());

    assert!(ctrl.on_rtcp_packet(&RtcpPacket::Pli(RtcpPli {
        sender_ssrc: 55_000,
        media_ssrc: 9209,
    })));
    assert!(ctrl.is_keyframe_requested());
    assert_eq!(ctrl.inbound_pli_count(), 1);
    assert_eq!(ctrl.last_request_kind(), Some(KeyframeRequestKind::Pli));
    assert!(ctrl.take_keyframe_request());
    assert!(!ctrl.is_keyframe_requested());

    // Входящий FIR (PT=206, FMT=4) с seq=10 для SSRC 9209 запрашивает ключевой кадр,
    // а повторный FIR с тем же seq=10 от того же отправителя дедуплицируется (RFC 5104 §4.3.1.2)
    let fir_in_10 = RtcpPacket::Fir(RtcpFir {
        sender_ssrc: 55_000,
        media_ssrc: 0,
        entries: vec![FirEntry {
            ssrc: 9209,
            sequence_number: 10,
        }],
    });
    assert!(ctrl.on_rtcp_packet(&fir_in_10));
    assert_eq!(ctrl.inbound_fir_count(), 1);
    assert_eq!(
        ctrl.last_request_kind(),
        Some(KeyframeRequestKind::Fir {
            sequence_number: 10
        })
    );
    assert!(ctrl.take_keyframe_request());

    // Повтор того же seq=10 не вызывает повторную генерацию IDR
    assert!(!ctrl.on_rtcp_packet(&fir_in_10));
    assert!(!ctrl.is_keyframe_requested());
    assert_eq!(ctrl.inbound_fir_count(), 1);

    // Новый seq=11 вызывает генерацию ключевого кадра
    let fir_in_11 = RtcpPacket::Fir(RtcpFir {
        sender_ssrc: 55_000,
        media_ssrc: 0,
        entries: vec![FirEntry {
            ssrc: 9209,
            sequence_number: 11,
        }],
    });
    assert!(ctrl.on_rtcp_packet(&fir_in_11));
    assert_eq!(ctrl.inbound_fir_count(), 2);
    assert!(ctrl.take_keyframe_request());
}

#[test]
fn discord_webrtc_error_codes_2001_2002_2003_2012_2013_oracle_and_remediation() {
    use chord_voice::rtcp::{
        DISCORD_CAMERA_LOW_FPS_MIN_WANTS_MS, DISCORD_CAMERA_MIN_ENCODE_FPS,
        DISCORD_HIGH_PACKET_LOSS_THRESHOLD_PCT, DISCORD_MIN_DATAPOINTS_FOR_FPS_LOSS_EVAL,
        DISCORD_VIEW_LOW_FPS_MIN_ENABLED_MS, WebRtcAvErrorCode, WebRtcMitigationAction,
        WebRtcStreamHealthInput, discord_stream_low_fps_threshold, evaluate_webrtc_stream_health,
    };

    // Сверка точных констант и метаданных из RE-оракула `/home/sanya/re-deep/out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js` (модуль 487329)
    assert_eq!(DISCORD_HIGH_PACKET_LOSS_THRESHOLD_PCT, 10.0);
    assert_eq!(DISCORD_CAMERA_MIN_ENCODE_FPS, 10);
    assert_eq!(DISCORD_VIEW_LOW_FPS_MIN_ENABLED_MS, 10_000);
    assert_eq!(DISCORD_CAMERA_LOW_FPS_MIN_WANTS_MS, 20_000);
    assert_eq!(DISCORD_MIN_DATAPOINTS_FOR_FPS_LOSS_EVAL, 15);

    // Точная кусочно-постоянная функция порога низкого FPS B(maxFrameRate) для кода 2002:
    assert_eq!(discord_stream_low_fps_threshold(5), 3);
    assert_eq!(discord_stream_low_fps_threshold(15), 8);
    assert_eq!(discord_stream_low_fps_threshold(30), 15);
    assert_eq!(discord_stream_low_fps_threshold(60), 30);

    // Проверка всех 5 целевых кодов 2001, 2002, 2003, 2012, 2013
    for (code_u16, expected_enum, expected_slug, expected_sev, expected_outbound) in [
        (
            2001,
            WebRtcAvErrorCode::StreamFailedToStart,
            "stream-failed-to-start",
            "critical",
            true,
        ),
        (
            2002,
            WebRtcAvErrorCode::StreamViewLowFps,
            "stream-view-low-fps",
            "warning",
            false,
        ),
        (
            2003,
            WebRtcAvErrorCode::StreamViewHighPacketLoss,
            "stream-view-high-packet-loss",
            "warning",
            false,
        ),
        (
            2012,
            WebRtcAvErrorCode::VideoStreamReceiverReadyTimeout,
            "video-stream-receiver-ready-timeout",
            "critical",
            false,
        ),
        (
            2013,
            WebRtcAvErrorCode::CameraSendLowEncodeFps,
            "camera-send-low-encode-fps",
            "warning",
            true,
        ),
    ] {
        assert_eq!(WebRtcAvErrorCode::from_code(code_u16), Some(expected_enum));
        assert_eq!(expected_enum.code(), code_u16);
        assert_eq!(expected_enum.slug(), expected_slug);
        assert_eq!(expected_enum.severity(), expected_sev);
        assert_eq!(expected_enum.category(), "video");
        assert_eq!(expected_enum.is_error_outbound(), expected_outbound);
    }
    assert_eq!(WebRtcAvErrorCode::from_code(9999), None);

    // Проверка диагностики и автоматического подавления всех 5 кодов ошибок:
    let diag = evaluate_webrtc_stream_health(&WebRtcStreamHealthInput {
        is_go_live: true,
        identify_stream_type_is_screen: false,
        stream_failed_without_error_code: true,
        incoming_video_enabled_ms: 12_000,
        datapoints: 16,
        max_framerate: 60,
        view_short_fps: 12.0,
        view_long_fps: 24.0,
        packet_loss_rate: 0.14,
        remote_video_announced_without_keyframe: true,
        awaiting_first_keyframe_ms: 500,
        camera_enabled: true,
        active_remote_wants_ms: 25_000,
        camera_encode_short_fps: 7.0,
        camera_encode_long_fps: 15.0,
    });

    assert!(
        diag.active_codes
            .contains(&WebRtcAvErrorCode::StreamFailedToStart)
    );
    assert!(
        diag.active_codes
            .contains(&WebRtcAvErrorCode::StreamViewLowFps)
    );
    assert!(
        diag.active_codes
            .contains(&WebRtcAvErrorCode::StreamViewHighPacketLoss)
    );
    assert!(
        diag.active_codes
            .contains(&WebRtcAvErrorCode::VideoStreamReceiverReadyTimeout)
    );
    assert!(
        diag.active_codes
            .contains(&WebRtcAvErrorCode::CameraSendLowEncodeFps)
    );

    assert!(
        diag.mitigations
            .contains(&WebRtcMitigationAction::UseScreenStreamTypeInIdentify)
    );
    assert!(
        diag.mitigations
            .contains(&WebRtcMitigationAction::SendRtcpPli)
    );
    assert!(
        diag.mitigations
            .contains(&WebRtcMitigationAction::SendRtcpFir)
    );
    assert!(
        diag.mitigations
            .contains(&WebRtcMitigationAction::SendRtcpReceiverReport)
    );
    assert!(
        diag.mitigations
            .contains(&WebRtcMitigationAction::EmitKeepaliveVideoFrame { min_fps: 30 })
    );
}

#[test]
fn rtp_video_packetizer_pli_fir_keyframe_generation_for_h264_and_av1() {
    use chord_voice::media::rtp::{
        ANNEX_B_START_CODE, VideoFrameAssembler, av1_is_keyframe, h264_has_parameter_sets,
        h264_is_keyframe, parse_rtp,
    };
    use chord_voice::rtcp::{build_fir, build_pli};
    use chord_voice::rtp::video::{
        AV1_AGGR_N_NEW_CODED_VIDEO_SEQ, AV1_OBU_FRAME, AV1_OBU_SEQUENCE_HEADER, H264_NAL_STAP_A,
        RtpVideoPacketizer, VIDEO_TICKS_30FPS, VIDEO_TICKS_60FPS,
    };

    let make_annex_b = |nalus: &[&[u8]]| -> Vec<u8> {
        let mut out = Vec::new();
        for nalu in nalus {
            out.extend_from_slice(&ANNEX_B_START_CODE);
            out.extend_from_slice(nalu);
        }
        out
    };

    // 1. H.264: кэширование SPS/PPS и автоматическая вставка STAP-A (SPS+PPS) + IDR при получении PLI/FIR
    let sps = vec![0x67, 0x64, 0x00, 0x28, 0xAC, 0xD9];
    let pps = vec![0x68, 0xEB, 0xE3, 0xCB];
    let idr = vec![0x65, 0x88, 0x84, 0x21, 0xA0];
    let non_idr = vec![0x41, 0x9A, 0x22, 0x33];

    let mut h264_pkt = RtpVideoPacketizer::new_h264(40_001).with_initial_state(100, 90_000);
    // Первый ключевой кадр кэширует SPS и PPS внутри пакетизатора
    let initial_kf = make_annex_b(&[&sps, &pps, &idr]);
    let pkts_0 = h264_pkt
        .packetize_h264_frame(&initial_kf, VIDEO_TICKS_30FPS, None)
        .expect("initial H264 keyframe");
    assert_eq!(pkts_0.len(), 1); // STAP-A (SPS + PPS + IDR)
    assert_eq!(pkts_0[0].payload[0] & 0x1F, H264_NAL_STAP_A);
    assert_eq!(
        h264_pkt.cached_h264_parameter_sets(),
        Some((&sps[..], &pps[..]))
    );
    assert!(!h264_pkt.is_keyframe_requested());

    // Приходит RTCP PLI (PT 206, FMT 1) для SSRC 40001
    let pli_wire = build_pli(9208, 40_001);
    assert!(
        h264_pkt
            .on_rtcp_compound(&pli_wire)
            .expect("handle RTCP PLI")
    );
    assert!(h264_pkt.is_keyframe_requested());
    assert_eq!(h264_pkt.pli_received_count(), 1);

    // Если кодер прислал голый IDR без SPS/PPS во время активного keyframe_requested,
    // пакетизатор автоматически предваряет его кэшированными SPS+PPS (STAP-A), чтобы новый зритель не получил 2012!
    let bare_idr = make_annex_b(&[&idr]);
    let pkts_pli_response = h264_pkt
        .packetize_h264_frame(&bare_idr, VIDEO_TICKS_30FPS, None)
        .expect("bare IDR upgraded with cached SPS/PPS on PLI");
    assert_eq!(pkts_pli_response.len(), 1);
    assert_eq!(pkts_pli_response[0].payload[0] & 0x1F, H264_NAL_STAP_A);
    assert!(!h264_pkt.is_keyframe_requested());

    // Приходит RTCP FIR (PT 206, FMT 4, seq=1) -> прямая генерация ключевого кадра `generate_keyframe_packets`
    // даже когда кодер прислал только P-срез или экран статичен (защита от 2012, 2002, 2013)
    let _ = h264_pkt
        .packetize_h264_frame(&make_annex_b(&[&non_idr]), VIDEO_TICKS_30FPS, None)
        .expect("P-slice");
    let fir_wire = build_fir(9208, 40_001, 1);
    assert!(
        h264_pkt
            .on_rtcp_compound(&fir_wire)
            .expect("handle RTCP FIR seq=1")
    );
    assert!(h264_pkt.is_keyframe_requested());
    assert_eq!(h264_pkt.fir_received_count(), 1);

    // Повтор того же FIR seq=1 дедуплицируется
    assert!(
        !h264_pkt
            .on_rtcp_compound(&fir_wire)
            .expect("duplicate FIR seq=1 ignored")
    );
    assert_eq!(h264_pkt.fir_received_count(), 1);

    let generated_h264_kf = h264_pkt
        .generate_keyframe_packets(VIDEO_TICKS_30FPS, None)
        .expect("generate H264 keyframe packets on FIR");
    assert!(!h264_pkt.is_keyframe_requested());
    let mut rx_h264 = VideoFrameAssembler::new_h264();
    let mut assembled_h264 = None;
    for p in &generated_h264_kf {
        let parsed = parse_rtp(&p.raw).unwrap();
        if let Some(frame) = rx_h264.push_packet(&parsed) {
            assembled_h264 = Some(frame);
        }
    }
    let assembled_h264 = assembled_h264.expect("generated H264 keyframe must assemble");
    assert!(h264_has_parameter_sets(&assembled_h264));
    assert!(h264_is_keyframe(&assembled_h264));

    // 2. AV1: кэширование OBU_SEQUENCE_HEADER и генерация ключевого кадра с битом N=1 (0x08) по PLI/FIR
    let mut av1_pkt = RtpVideoPacketizer::new_av1(40_001).with_initial_state(500, 180_000);
    let obu_seq = [
        (AV1_OBU_SEQUENCE_HEADER << 3) | 0x02,
        0x04,
        0x01,
        0x02,
        0x03,
        0x04,
    ];
    let obu_frame = [(AV1_OBU_FRAME << 3) | 0x02, 0x03, 0xAA, 0xBB, 0xCC];
    let _ = av1_pkt
        .packetize_av1_obus(&[&obu_seq, &obu_frame], VIDEO_TICKS_60FPS, None)
        .expect("initial AV1 keyframe caches OBU_SEQUENCE_HEADER");
    assert_eq!(av1_pkt.cached_av1_sequence_header(), Some(&obu_seq[..]));

    let fir_av1 = build_fir(9208, 40_001, 2);
    assert!(av1_pkt.on_rtcp_compound(&fir_av1).expect("AV1 FIR seq=2"));
    assert!(av1_pkt.is_keyframe_requested());

    let generated_av1_kf = av1_pkt
        .generate_keyframe_packets(VIDEO_TICKS_60FPS, None)
        .expect("generate AV1 keyframe packets on FIR");
    assert_eq!(generated_av1_kf.len(), 1);
    assert_ne!(
        generated_av1_kf[0].payload[0] & AV1_AGGR_N_NEW_CODED_VIDEO_SEQ,
        0,
        "AV1 keyframe generated on PLI/FIR must set N=1 bit"
    );
    let mut rx_av1 = VideoFrameAssembler::new_av1();
    let assembled_av1 = rx_av1
        .push_packet(&parse_rtp(&generated_av1_kf[0].raw).unwrap())
        .expect("AV1 keyframe assembled");
    assert!(av1_is_keyframe(&assembled_av1));
}

#[test]
fn voice_session_driver_defeats_2001_2002_2003_2012_2013_with_srtcp_rr_pli_fir() {
    use chord_voice::rtp::video::{RTP_H264_PAYLOAD_TYPE, VIDEO_TICKS_30FPS};
    use chord_voice::session::{StreamServerUpdate, compute_go_live_rtc_channel_id};
    use chord_voice::{
        Identify, VoiceOp, VoicePayload, VoiceServerUpdate, VoiceSessionDriver, VoiceStateUpdate,
        parse_frame,
    };

    const SAMPLE_SRTP_SECRET_KEY: [u8; 32] = [
        0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f,
        0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x3b, 0x3c, 0x3d, 0x3e,
        0x3f, 0x40,
    ];

    // 1. Подавление кода 2001 (`stream-failed-to-start`):
    // - Вычисление `rtc_channel_id = (rtc_server_id - 1)` для Go Live сессий
    // - В `OP 0 IDENTIFY` для Go Live (`is_go_live() == true`) обязательно уходит `streams: [{ type: "screen", rid: "100", quality: 100 }]`
    assert_eq!(
        compute_go_live_rtc_channel_id("1176785373989445704"),
        Some("1176785373989445703".to_owned())
    );

    let stream_update = StreamServerUpdate::parse(
        r#"{"stream_key":"guild:1014537851418980372:1176785373989445703:384299970632876032","endpoint":"c-fra19-stream.discord.media:2053","token":"stream-secret-token","session_id":"gw-session-secret-id","server_id":"1176785373989445704"}"#,
    )
    .expect("parse StreamServerUpdate");

    let mut streamer = VoiceSessionDriver::new_go_live(
        stream_update,
        "1014537851418980372",
        "",
        "384299970632876032",
        "gw-session-secret-id",
    )
    .expect("new_go_live");
    assert!(streamer.is_go_live());
    let streamer_ws = streamer.take_outgoing_ws();
    assert_eq!(streamer_ws.len(), 1);
    let parsed_id = parse_frame(&streamer_ws[0]).expect("parse Go Live OP 0 IDENTIFY");
    assert_eq!(parsed_id.op, VoiceOp::Identify);
    match parsed_id.payload {
        VoicePayload::Identify(Identify { streams, .. }) => {
            assert_eq!(streams.len(), 1);
            assert_eq!(
                streams[0].kind, "screen",
                "Go Live OP 0 IDENTIFY must use stream type 'screen' to defeat error 2001"
            );
        }
        other => panic!("expected Identify, got {other:?}"),
    }

    // Поднимаем SRTP-сессию стримера (SSRC 40000 audio, 40001 video, 40002 rtx) и зрителя (SSRC 9208)
    let viewer_server = VoiceServerUpdate::parse(
        r#"{"token":"viewer-token","guild_id":"1014537851418980372","endpoint":"c-fra19-stream.discord.media:2053"}"#,
    )
    .expect("viewer server");
    let viewer_state = VoiceStateUpdate::parse(
        r#"{"guild_id":"1014537851418980372","channel_id":"1176785373989445703","user_id":"222222222222222222","session_id":"viewer-sess","self_mute":false,"self_deaf":false,"self_video":true}"#,
    )
    .expect("viewer state");
    let mut viewer = VoiceSessionDriver::new(viewer_server, viewer_state).expect("viewer driver");
    let _ = viewer.take_outgoing_ws();

    let key_json_array = SAMPLE_SRTP_SECRET_KEY
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let op4_json = format!(
        r#"{{"op":4,"d":{{"mode":"aead_xchacha20_poly1305_rtpsize","secret_key":[{key_json_array}],"dave_protocol_version":1,"secure_frames_version":1,"media_session_id":"media-sess-w2","audio_codec":"opus","video_codec":"H264"}}}}"#
    );

    streamer
        .handle_ws_frame(r#"{"op":8,"d":{"v":8,"heartbeat_interval":13750}}"#)
        .unwrap();
    streamer
        .handle_ws_frame(
            r#"{"op":2,"d":{"streams":[],"ssrc":40000,"port":19400,"modes":["aead_xchacha20_poly1305_rtpsize"],"ip":"104.29.146.60","experiments":[]}}"#,
        )
        .unwrap();
    streamer.handle_ws_frame(&op4_json).unwrap();
    // Сбрасываем начальный запрос ключевого кадра, если он был выставлен на OP 4
    let _ = streamer.take_keyframe_request();

    viewer
        .handle_ws_frame(r#"{"op":8,"d":{"v":8,"heartbeat_interval":13750}}"#)
        .unwrap();
    viewer
        .handle_ws_frame(
            r#"{"op":2,"d":{"streams":[],"ssrc":9208,"port":19400,"modes":["aead_xchacha20_poly1305_rtpsize"],"ip":"104.29.146.60","experiments":[]}}"#,
        )
        .unwrap();
    viewer.handle_ws_frame(&op4_json).unwrap();

    // 2. Подавление кода 2012 (`video-stream-receiver-ready-timeout`):
    // Зритель отправляет зашифрованный SRTCP PLI (PT 206, FMT 1) и SRTCP FIR (PT 206, FMT 4) для SSRC 40001,
    // а стример расшифровывает их и выставляет флаг генерации ключевого кадра!
    let srtcp_pli = viewer
        .encrypt_rtcp_pli(40_001)
        .expect("viewer encrypts SRTCP PLI");
    let decoded_pli = streamer
        .decrypt_rtcp_packet(&srtcp_pli)
        .expect("streamer decrypts SRTCP PLI");
    assert!(streamer.is_keyframe_requested());
    assert_eq!(streamer.inbound_pli_count(), 1);
    assert!(
        decoded_pli
            .compound
            .packets
            .iter()
            .any(|p| matches!(p, RtcpPacket::Pli(pli) if pli.media_ssrc == 40_001))
    );
    assert!(streamer.take_keyframe_request());
    assert!(!streamer.is_keyframe_requested());

    // Зритель отправляет два подряд SRTCP FIR (seq=0, затем seq=1) в составном пакете с RR
    let srtcp_fir_0 = viewer
        .encrypt_compound_rr_fir(40_001, 1_000)
        .expect("viewer encrypts compound RR+FIR #0");
    let _ = streamer
        .decrypt_rtcp_packet(&srtcp_fir_0)
        .expect("streamer decrypts RR+FIR #0");
    assert!(streamer.is_keyframe_requested());
    assert_eq!(streamer.inbound_fir_count(), 1);
    assert!(streamer.take_keyframe_request());

    let srtcp_fir_1 = viewer
        .encrypt_compound_rr_fir(40_001, 1_200)
        .expect("viewer encrypts compound RR+FIR #1");
    let _ = streamer
        .decrypt_rtcp_packet(&srtcp_fir_1)
        .expect("streamer decrypts RR+FIR #1");
    assert!(streamer.is_keyframe_requested());
    assert_eq!(streamer.inbound_fir_count(), 2);
    assert!(streamer.take_keyframe_request());

    // 3. Подавление кода 2003 (`stream-view-high-packet-loss`):
    // Стример отправляет видео-пакеты RTP (SSRC 40001), часть пакетов теряется в сети,
    // зритель автоматически считает `ReceptionStats` при `decrypt_rtp_packet` и формирует SRTCP Receiver Report (PT 201)!
    for seq_idx in 0..10u32 {
        let encrypted_video = streamer
            .encrypt_video_frame(
                &[0x65, 0x01, 0x02, seq_idx as u8],
                RTP_H264_PAYLOAD_TYPE,
                VIDEO_TICKS_30FPS,
                true,
                None,
            )
            .expect("streamer encrypts video RTP");
        // Теряем пакеты 2 и 6 (2 из 10 = 20% потерь -> fraction_lost = floor(2 * 256 / 10) = 51)
        if seq_idx != 2 && seq_idx != 6 {
            let _ = viewer
                .decrypt_rtp_packet(&encrypted_video)
                .expect("viewer decrypts video RTP");
        }
    }

    let srtcp_rr = viewer
        .encrypt_rtcp_receiver_report(40_001, 2_000)
        .expect("viewer builds SRTCP Receiver Report for 40001");
    let decoded_rr = streamer
        .decrypt_rtcp_packet(&srtcp_rr)
        .expect("streamer decrypts SRTCP Receiver Report");
    match &decoded_rr.compound.packets[0] {
        RtcpPacket::ReceiverReport {
            sender_ssrc,
            reports,
        } => {
            assert_eq!(*sender_ssrc, 9208);
            assert_eq!(reports.len(), 1);
            assert_eq!(reports[0].ssrc, 40_001);
            assert_eq!(reports[0].fraction_lost, 51);
            assert_eq!(reports[0].cumulative_lost, 2);
            assert_eq!(reports[0].extended_highest_seq, 9);
        }
        other => panic!("expected ReceiverReport, got {other:?}"),
    }
    // Поскольку у удалённого зрителя зафиксирована высокая потеря пакетов (> 10%, fraction_lost = 51 > 25),
    // стример автоматически помечает необходимость ключевого кадра для восстановления видеопотока!
    assert!(streamer.is_keyframe_requested());
}
