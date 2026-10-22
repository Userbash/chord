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

use discord_voice::rtcp::{
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
