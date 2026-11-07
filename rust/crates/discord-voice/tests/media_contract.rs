//! Контракт RTP-медиа P4-04: заголовок/расширение, DTX/CN, джиттер-буфер.
//!
//! Оракул — `fixtures/voice/rtp-pt120-oracle.json` (golden pcap `voice-udp4.pcap`,
//! 10 пакетов PT120, только чтение). Тесты сверяют байтовую раскладку с capture,
//! а не с реализацией: 8 валидных пакетов (`0xBEDE`, 2 слова, 23 Б остатка) и
//! 2 аномальных по 43 Б.
//!
//! Трактовка неоднозначного места контракта (TZ-W29b §1 vs §3 m02). У аномальных
//! пакетов профиль расширения не `0xBEDE` (`0x19f5`/`0x2d6d`), а поле words
//! (59783/28435) заведомо не является длиной: в 43-байтовом пакете под данные
//! остаётся 27 Б. Поэтому для нестандартного профиля при `words*4 > остатка`
//! парсер не падает: хвост отдаётся как opaque-данные расширения
//! (`standard = false`), payload пуст — ровно то, что зафиксировал генератор
//! оракула (`data_hex` 27 Б, `remainder_hex` пуст). Для `0xBEDE` тот же случай —
//! `TruncatedExtension` (m06).

use discord_json::JsonValue;
use discord_voice::media::{
    AudioFrameKind, EXTENSION_PROFILE, FRAME_TICKS, JitterBuffer, OPUS_PAYLOAD_TYPE, PopOutcome,
    PushOutcome, RtpError, RtpHeader, RtpSender, classify_opus_payload, parse_rtp,
};

/// Golden-оракул из корня репозитория (путь от `CARGO_MANIFEST_DIR` крейта).
const ORACLE_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../fixtures/voice/rtp-pt120-oracle.json"
));

/// SSRC аудио-потока сессии s3 (`docs/re/voice-deep.md` §1).
const AUDIO_SSRC: u32 = 9208;

/// Разбор hex-строки (в тесте без внешних зависимостей).
fn hex(text: &str) -> Vec<u8> {
    assert!(
        text.len().is_multiple_of(2),
        "hex-строка должна иметь чётную длину"
    );
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).expect("валидный hex"))
        .collect()
}

/// Пакеты оракула в порядке захвата.
fn oracle_packets() -> Vec<JsonValue> {
    let root = discord_json::parse(ORACLE_JSON).expect("оракул — валидный JSON");
    root.get("packets")
        .and_then(JsonValue::as_array)
        .expect("в оракуле есть packets")
        .to_vec()
}

fn integer(packet: &JsonValue, key: &str) -> u64 {
    packet
        .get(key)
        .and_then(JsonValue::as_u64)
        .unwrap_or_else(|| panic!("в оракуле нет целого поля {key}"))
}

fn small(packet: &JsonValue, key: &str) -> u16 {
    u16::try_from(integer(packet, key)).expect("поле помещается в u16")
}

fn text<'a>(packet: &'a JsonValue, key: &str) -> &'a str {
    packet
        .get(key)
        .and_then(JsonValue::as_str)
        .unwrap_or_else(|| panic!("в оракуле нет строки {key}"))
}

fn boolean(packet: &JsonValue, key: &str) -> bool {
    packet
        .get(key)
        .and_then(JsonValue::as_bool)
        .unwrap_or_else(|| panic!("в оракуле нет булева поля {key}"))
}

fn extension(packet: &JsonValue) -> &JsonValue {
    packet
        .get("extension")
        .expect("в оракуле есть описание расширения")
}

fn is_null(packet: &JsonValue, key: &str) -> bool {
    packet.get(key).is_some_and(JsonValue::is_null)
}

/// Валидные (не аномальные) пакеты оракула.
fn valid_packets(packets: &[JsonValue]) -> Vec<&JsonValue> {
    packets
        .iter()
        .filter(|packet| is_null(packet, "anomaly"))
        .collect()
}

#[test]
fn m01_parse_captured_valid_packets() {
    let packets = oracle_packets();
    assert_eq!(packets.len(), 10, "в оракуле 10 пакетов PT120");
    let valid = valid_packets(&packets);
    assert_eq!(valid.len(), 8, "8 валидных пакетов с 0xBEDE");

    let mut previous_timestamp: Option<u32> = None;
    for packet in valid {
        let raw = hex(text(packet, "raw_hex"));
        let parsed = parse_rtp(&raw).expect("валидный пакет парсится");
        assert_eq!(parsed.header.sequence, small(packet, "sequence"));
        assert_eq!(
            parsed.header.timestamp,
            u32::try_from(integer(packet, "timestamp")).unwrap()
        );
        assert_eq!(
            parsed.header.ssrc,
            u32::try_from(integer(packet, "ssrc")).unwrap()
        );
        assert_eq!(parsed.header.ssrc, AUDIO_SSRC);
        assert_eq!(parsed.header.payload_type, OPUS_PAYLOAD_TYPE);
        assert_eq!(parsed.header.marker, boolean(packet, "marker"));
        assert!(
            !parsed.header.marker,
            "marker только на первом пакете talkspurt"
        );

        let ext = extension(packet);
        let parsed_ext = parsed.extension.as_ref().expect("расширение присутствует");
        assert_eq!(parsed_ext.profile, EXTENSION_PROFILE);
        assert!(parsed_ext.standard);
        assert_eq!(small(ext, "words"), 2);
        assert_eq!(parsed_ext.data, hex(text(ext, "data_hex")).as_slice());
        assert_eq!(parsed_ext.data.len(), 8);

        assert_eq!(
            parsed.payload,
            hex(text(packet, "remainder_hex")).as_slice()
        );
        assert_eq!(parsed.payload.len(), 23);
        assert_eq!(
            parsed.payload.len(),
            usize::try_from(integer(packet, "remainder_len")).unwrap()
        );

        // Счётчик трейлера `0x80000000 | n` — последние 4 Б LE (docs/re §2.3).
        let tail = &parsed.payload[parsed.payload.len() - 4..];
        let counter = u32::from_le_bytes([tail[0], tail[1], tail[2], tail[3]]);
        assert_eq!(u64::from(counter), integer(packet, "counter"));

        if let Some(previous) = previous_timestamp {
            assert_eq!(
                parsed.header.timestamp.wrapping_sub(previous),
                FRAME_TICKS,
                "каденция 20 мс = 960 тиков"
            );
        }
        previous_timestamp = Some(parsed.header.timestamp);
    }

    // Первый пакет talkspurt — аномальный, и именно он несёт marker.
    let first_raw = hex(text(&packets[0], "raw_hex"));
    let first = parse_rtp(&first_raw).expect("парсится");
    assert!(first.header.marker);
    assert!(packets[1..].iter().all(|packet| !boolean(packet, "marker")));
}

#[test]
fn m02_parse_captured_anomalies_without_standard_extension() {
    let packets = oracle_packets();
    let anomalies: Vec<&JsonValue> = packets
        .iter()
        .filter(|packet| {
            packet.get("anomaly").and_then(JsonValue::as_str) == Some("no-valid-extension")
        })
        .collect();
    assert_eq!(anomalies.len(), 2);
    assert_eq!(integer(anomalies[0], "length"), 43);
    assert_eq!(integer(anomalies[1], "length"), 43);

    for packet in anomalies {
        let raw = hex(text(packet, "raw_hex"));
        assert_eq!(raw.len(), 43);
        let parsed = parse_rtp(&raw).expect("нестандартный профиль — не ошибка парсинга");
        let ext = parsed.extension.as_ref().expect("расширение присутствует");

        let expected_profile = u16::from_str_radix(
            text(extension(packet), "profile").trim_start_matches("0x"),
            16,
        )
        .expect("профиль в hex");
        assert_eq!(ext.profile, expected_profile);
        assert_ne!(ext.profile, EXTENSION_PROFILE);
        assert!(!ext.standard);

        // words (59783/28435) не является длиной: слова * 4 >> остатка пакета.
        let words = usize::try_from(integer(extension(packet), "words")).unwrap();
        let available = raw.len() - 16;
        assert!(words * 4 > available);
        assert_eq!(
            ext.data,
            hex(text(extension(packet), "data_hex")).as_slice()
        );
        assert_eq!(ext.data.len(), available);
        assert!(
            parsed.payload.is_empty(),
            "opaque-хвост не считается payload"
        );
        assert_eq!(parsed.header.payload_type, OPUS_PAYLOAD_TYPE);
        assert_eq!(parsed.header.ssrc, AUDIO_SSRC);
    }
}

#[test]
fn m03_encode_header_without_extension() {
    let header = RtpHeader {
        marker: true,
        payload_type: OPUS_PAYLOAD_TYPE,
        sequence: 0x4438,
        timestamp: 0x1099_214c,
        ssrc: 0x0000_23f8,
    };
    let bytes = header.encode(None).expect("заголовок без расширения");
    assert_eq!(bytes.len(), 12);
    assert_eq!(bytes[0], 0x80, "V=2, P=0, X=0, CC=0");
    assert_eq!(bytes[1], 0x80 | OPUS_PAYLOAD_TYPE, "marker<<7 | PT");
    assert_eq!(&bytes[2..4], &0x4438u16.to_be_bytes());
    assert_eq!(&bytes[4..8], &0x1099_214cu32.to_be_bytes());
    assert_eq!(&bytes[8..12], &0x0000_23f8u32.to_be_bytes());

    let header = RtpHeader {
        marker: false,
        ..header
    };
    let bytes = header.encode(None).expect("заголовок без расширения");
    assert_eq!(
        bytes,
        [
            0x80, 0x78, 0x44, 0x38, 0x10, 0x99, 0x21, 0x4c, 0x00, 0x00, 0x23, 0xf8
        ]
    );
}

#[test]
fn m04_encode_header_with_extension() {
    let packets = oracle_packets();
    let valid = valid_packets(&packets);
    let captured = hex(text(valid[0], "raw_hex"));
    let ext_data = hex(text(extension(valid[0]), "data_hex"));
    assert_eq!(ext_data.len(), 8);

    let header = RtpHeader {
        marker: boolean(valid[0], "marker"),
        payload_type: OPUS_PAYLOAD_TYPE,
        sequence: small(valid[0], "sequence"),
        timestamp: u32::try_from(integer(valid[0], "timestamp")).unwrap(),
        ssrc: u32::try_from(integer(valid[0], "ssrc")).unwrap(),
    };
    let bytes = header.encode(Some(&ext_data)).expect("8 Б выровнены");
    assert_eq!(bytes.len(), 24);
    assert_eq!(bytes[0], 0x90, "V=2, P=0, X=1, CC=0");
    assert_eq!(bytes[1], OPUS_PAYLOAD_TYPE);
    assert_eq!(&bytes[12..14], &EXTENSION_PROFILE.to_be_bytes());
    assert_eq!(&bytes[14..16], &2u16.to_be_bytes(), "words = len / 4");
    assert_eq!(&bytes[16..24], ext_data.as_slice());
    assert_eq!(
        bytes,
        captured[..24],
        "префикс совпадает с capture побайтно"
    );
}

#[test]
fn m05_encode_rejects_misaligned_or_oversized_extension() {
    let header = RtpHeader {
        marker: false,
        payload_type: OPUS_PAYLOAD_TYPE,
        sequence: 0,
        timestamp: 0,
        ssrc: AUDIO_SSRC,
    };
    assert_eq!(
        header.encode(Some(&[0; 2])),
        Err(RtpError::ExtensionNotAligned)
    );
    assert_eq!(
        header.encode(Some(&[0; 6])),
        Err(RtpError::ExtensionNotAligned)
    );
    assert_eq!(
        header.encode(Some(&[0; 64])),
        Err(RtpError::ExtensionNotAligned)
    );
    assert_eq!(
        header.encode(Some(&[0; 61])),
        Err(RtpError::ExtensionNotAligned)
    );

    // Границы 0 и 60 Б допустимы: длина 0 даёт ext-заголовок с words = 0.
    let empty = header.encode(Some(&[])).expect("0 Б кратно 4");
    assert_eq!(empty.len(), 16);
    assert_eq!(&empty[14..16], &0u16.to_be_bytes());
    let full = header.encode(Some(&[0x5a; 60])).expect("60 Б — предел");
    assert_eq!(full.len(), 76);
    assert_eq!(&full[14..16], &15u16.to_be_bytes());
}

#[test]
fn m06_parse_rejects_short_bad_version_and_truncated_extension() {
    assert_eq!(parse_rtp(&[]).unwrap_err(), RtpError::TooShort);
    assert_eq!(parse_rtp(&[0x80; 11]).unwrap_err(), RtpError::TooShort);
    assert_eq!(parse_rtp(&[0x40; 12]).unwrap_err(), RtpError::BadVersion);

    let header = RtpHeader {
        marker: false,
        payload_type: OPUS_PAYLOAD_TYPE,
        sequence: 7,
        timestamp: 960,
        ssrc: AUDIO_SSRC,
    };
    // X=1, но 4 Б заголовка расширения отсутствуют.
    let mut missing_header = header.encode(None).unwrap();
    missing_header[0] |= 0x10;
    assert_eq!(
        parse_rtp(&missing_header).unwrap_err(),
        RtpError::TruncatedExtension
    );

    // 0xBEDE заявляет 2 слова (8 Б), а в пакете только 4 Б данных.
    let mut truncated = header.encode(None).unwrap();
    truncated[0] |= 0x10;
    truncated.extend_from_slice(&[0xbe, 0xde, 0x00, 0x02, 1, 2, 3, 4]);
    assert_eq!(
        parse_rtp(&truncated).unwrap_err(),
        RtpError::TruncatedExtension
    );

    // as_str различает варианты (диагностика в логах).
    let messages = [
        RtpError::TooShort.as_str(),
        RtpError::BadVersion.as_str(),
        RtpError::TruncatedExtension.as_str(),
        RtpError::PaddingOutOfRange.as_str(),
        RtpError::ExtensionNotAligned.as_str(),
    ];
    assert!(messages.iter().all(|message| !message.is_empty()));
    assert_eq!(
        messages
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        messages.len()
    );
}

#[test]
fn m07_parse_strips_padding_and_rejects_out_of_range() {
    let header = RtpHeader {
        marker: false,
        payload_type: OPUS_PAYLOAD_TYPE,
        sequence: 9,
        timestamp: 1920,
        ssrc: AUDIO_SSRC,
    };

    let mut padded = header.encode(None).unwrap();
    padded[0] |= 0x20; // P=1
    padded.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0x01]);
    let parsed = parse_rtp(&padded).expect("корректный паддинг");
    assert_eq!(parsed.payload, &[0xaa, 0xbb, 0xcc]);

    // Последний байт — число октетов паддинга вместе с самим собой.
    let mut double = header.encode(None).unwrap();
    double[0] |= 0x20;
    double.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd, 0x02]);
    let parsed = parse_rtp(&double).expect("два октета паддинга");
    assert_eq!(parsed.payload, &[0xaa, 0xbb, 0xcc]);

    let mut too_much = header.encode(None).unwrap();
    too_much[0] |= 0x20;
    too_much.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0x05]);
    assert_eq!(
        parse_rtp(&too_much).unwrap_err(),
        RtpError::PaddingOutOfRange
    );

    let mut zero = header.encode(None).unwrap();
    zero[0] |= 0x20;
    zero.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0x00]);
    assert_eq!(parse_rtp(&zero).unwrap_err(), RtpError::PaddingOutOfRange);

    let mut empty = header.encode(None).unwrap();
    empty[0] |= 0x20;
    assert_eq!(parse_rtp(&empty).unwrap_err(), RtpError::PaddingOutOfRange);
}

#[test]
fn m08_sender_increments_sequence_and_timestamp_with_one_marker() {
    let mut sender = RtpSender::new(AUDIO_SSRC);
    assert_eq!(sender.sequence(), 0);
    assert_eq!(sender.timestamp(), 0);

    let first = sender.next_packet(&[1, 2, 3], None).expect("первый пакет");
    assert_eq!(first.len(), 15);
    let parsed = parse_rtp(&first).expect("пакет самосогласован");
    assert!(parsed.header.marker, "marker на первом пакете talkspurt");
    assert_eq!(parsed.header.payload_type, OPUS_PAYLOAD_TYPE);
    assert_eq!(parsed.header.sequence, 0);
    assert_eq!(parsed.header.timestamp, 0);
    assert_eq!(parsed.header.ssrc, AUDIO_SSRC);
    assert!(parsed.extension.is_none());
    assert_eq!(parsed.payload, &[1, 2, 3]);
    assert_eq!(sender.sequence(), 1);
    assert_eq!(sender.timestamp(), FRAME_TICKS);

    let second = sender
        .next_packet(&[], Some(&[0xa5; 4]))
        .expect("второй пакет");
    let parsed = parse_rtp(&second).expect("пакет самосогласован");
    assert!(!parsed.header.marker);
    assert_eq!(parsed.header.sequence, 1);
    assert_eq!(parsed.header.timestamp, FRAME_TICKS);
    assert_eq!(
        parsed.extension.expect("расширение есть").profile,
        EXTENSION_PROFILE
    );
    assert!(parsed.payload.is_empty());
    assert_eq!(sender.sequence(), 2);
    assert_eq!(sender.timestamp(), 2 * FRAME_TICKS);

    // Ошибка кодирования не сдвигает состояние отправителя.
    assert_eq!(
        sender.next_packet(b"x", Some(&[0; 3])).unwrap_err(),
        RtpError::ExtensionNotAligned
    );
    assert_eq!(sender.sequence(), 2);
    assert_eq!(sender.timestamp(), 2 * FRAME_TICKS);
}

#[test]
fn m09_sender_silence_moves_only_timestamp_and_rearms_marker() {
    let mut sender = RtpSender::new(AUDIO_SSRC);
    let first = sender.next_packet(b"a", None).expect("первый пакет");
    assert!(parse_rtp(&first).unwrap().header.marker);

    sender.silence(3);
    assert_eq!(sender.sequence(), 1, "DTX не двигает sequence");
    assert_eq!(
        sender.timestamp(),
        4 * FRAME_TICKS,
        "0 + 960 после пакета + 3 кадра"
    );

    let after_silence = sender.next_packet(b"b", None).expect("после тишины");
    let parsed = parse_rtp(&after_silence).unwrap();
    assert!(parsed.header.marker, "marker взводится снова");
    assert_eq!(parsed.header.sequence, 1, "seq растёт на один пакет");
    assert_eq!(parsed.header.timestamp, 4 * FRAME_TICKS);
    assert_eq!(sender.timestamp(), 5 * FRAME_TICKS);

    sender.silence(0);
    assert_eq!(sender.timestamp(), 5 * FRAME_TICKS);
    assert_eq!(sender.sequence(), 2);
}

#[test]
fn m10_classify_opus_payload_by_comfort_noise_threshold() {
    assert_eq!(classify_opus_payload(0), AudioFrameKind::Empty);
    assert_eq!(classify_opus_payload(1), AudioFrameKind::ComfortNoise);
    assert_eq!(classify_opus_payload(2), AudioFrameKind::ComfortNoise);
    assert_eq!(classify_opus_payload(3), AudioFrameKind::ComfortNoise);
    assert_eq!(classify_opus_payload(4), AudioFrameKind::Speech);
    // В capture шифротекст CN — 3 Б (docs/re/voice-deep.md §2.3).
    assert_eq!(classify_opus_payload(23), AudioFrameKind::Speech);
}

#[test]
fn m11_jitter_reorders_three_one_two() {
    // Трактовка §1: первый push задаёт ожидаемый seq, но пока ничего не отдано,
    // пакет позади окна не является «опоздавшим» (он ещё не отдан и не пропущен):
    // окно расширяется назад, поэтому реордер 3→1→2 выдаётся как 1,2,3.
    let mut buffer = JitterBuffer::new(4);
    assert_eq!(buffer.capacity(), 4);
    assert_eq!(buffer.push(3, b"three".to_vec()), PushOutcome::Queued);
    assert_eq!(buffer.push(1, b"one".to_vec()), PushOutcome::Queued);
    assert_eq!(buffer.push(2, b"two".to_vec()), PushOutcome::Queued);
    assert_eq!(buffer.buffered(), 3);

    assert_eq!(
        buffer.pop(),
        PopOutcome::Frame {
            sequence: 1,
            payload: b"one".to_vec(),
        }
    );
    assert_eq!(
        buffer.pop(),
        PopOutcome::Frame {
            sequence: 2,
            payload: b"two".to_vec(),
        }
    );
    assert_eq!(
        buffer.pop(),
        PopOutcome::Frame {
            sequence: 3,
            payload: b"three".to_vec(),
        }
    );
    assert_eq!(buffer.pop(), PopOutcome::Empty);

    let stats = buffer.stats();
    assert_eq!(stats.received, 3);
    assert_eq!(
        (stats.duplicates, stats.late, stats.missing, stats.overflow),
        (0, 0, 0, 0)
    );
}

#[test]
fn m12_jitter_counts_duplicate_late_and_missing_once() {
    let mut buffer = JitterBuffer::new(8);
    assert_eq!(buffer.push(1, b"one".to_vec()), PushOutcome::Queued);
    assert_eq!(buffer.push(3, b"three".to_vec()), PushOutcome::Queued);
    assert_eq!(buffer.push(4, b"four".to_vec()), PushOutcome::Queued);
    // Дубль ещё не отданного пакета.
    assert_eq!(buffer.push(3, b"three".to_vec()), PushOutcome::Duplicate);

    assert_eq!(
        buffer.pop(),
        PopOutcome::Frame {
            sequence: 1,
            payload: b"one".to_vec(),
        }
    );
    // §1: Duplicate — «уже в буфере или уже отдан», поэтому повтор отданного
    // seq — дубль, а Late достаётся seq позади фронта, который был пропущен.
    assert_eq!(buffer.push(1, b"one".to_vec()), PushOutcome::Duplicate);
    // Потеря 2: ровно один Missing, затем выдача продолжается.
    assert_eq!(buffer.pop(), PopOutcome::Missing { sequence: 2 });
    // Пропущенный seq не был отдан — он опоздавший.
    assert_eq!(buffer.push(2, b"two".to_vec()), PushOutcome::Late);
    assert_eq!(
        buffer.pop(),
        PopOutcome::Frame {
            sequence: 3,
            payload: b"three".to_vec(),
        }
    );
    assert_eq!(
        buffer.pop(),
        PopOutcome::Frame {
            sequence: 4,
            payload: b"four".to_vec(),
        }
    );
    assert_eq!(buffer.pop(), PopOutcome::Empty);

    let stats = buffer.stats();
    assert_eq!(stats.received, 3, "приняты только 1, 3, 4");
    assert_eq!(stats.duplicates, 2, "3 (в буфере) и 1 (уже отдан)");
    assert_eq!(stats.late, 1, "seq 2 был пропущен, но не отдан");
    assert_eq!(stats.missing, 1);
    assert_eq!(stats.overflow, 0);
}

#[test]
fn m13_jitter_handles_wraparound_and_capacity_one() {
    let mut buffer = JitterBuffer::new(0);
    assert_eq!(buffer.capacity(), 1, "capacity 0 трактуется как 1");

    assert_eq!(buffer.push(65_534, b"a".to_vec()), PushOutcome::Queued);
    assert_eq!(buffer.push(65_535, b"b".to_vec()), PushOutcome::Overflow);
    assert_eq!(buffer.stats().overflow, 1);
    assert_eq!(buffer.buffered(), 1);
    assert_eq!(
        buffer.pop(),
        PopOutcome::Frame {
            sequence: 65_534,
            payload: b"a".to_vec(),
        }
    );

    assert_eq!(buffer.push(65_535, b"b".to_vec()), PushOutcome::Queued);
    assert_eq!(
        buffer.pop(),
        PopOutcome::Frame {
            sequence: 65_535,
            payload: b"b".to_vec(),
        }
    );

    // 65535 + 1 = 0: окно по модулю 2^16, порядок сохраняется.
    assert_eq!(buffer.push(0, b"c".to_vec()), PushOutcome::Queued);
    assert_eq!(
        buffer.pop(),
        PopOutcome::Frame {
            sequence: 0,
            payload: b"c".to_vec(),
        }
    );
    assert_eq!(buffer.pop(), PopOutcome::Empty);

    let stats = buffer.stats();
    assert_eq!(stats.received, 3);
    assert_eq!(
        (stats.duplicates, stats.late, stats.missing, stats.overflow),
        (0, 0, 0, 1)
    );
}

#[test]
fn m14_srtcp_empty_rr_exact_byte_layout_oracle_28b() {
    use discord_voice::udp::{SRTCP_EMPTY_RR_LEN, empty_srtcp_receiver_report};

    let ssrc = 0x0000_23f8; // 9208 из оракула s3
    let tag = [
        0x4c, 0x43, 0x9e, 0x13, 0x57, 0xc7, 0xd7, 0xe3, 0x85, 0xee, 0x66, 0xd1, 0x20, 0x58, 0xbf,
        0xe0,
    ];
    let trailer_counter = 0x8000_0016; // 0x80000000 | 22

    let packet = empty_srtcp_receiver_report(ssrc, tag, trailer_counter);
    assert_eq!(packet.len(), SRTCP_EMPTY_RR_LEN);
    assert_eq!(
        packet.len(),
        28,
        "ровно 28 байт по спецификации RE/voice-deep.json"
    );

    // 8B RTCP header: V=2, P=0, RC=0 (0x80), PT=201 (0xc9), len=1 (0x00, 0x01), ssrc (4B BE)
    assert_eq!(&packet[0..4], &[0x80, 0xc9, 0x00, 0x01]);
    assert_eq!(&packet[4..8], &ssrc.to_be_bytes());

    // 16B AEAD tag
    assert_eq!(&packet[8..24], &tag);

    // 4B LE SRTCP index with E-bit (0x80000016 in LE = [0x16, 0x00, 0x00, 0x80])
    assert_eq!(&packet[24..28], &[0x16, 0x00, 0x00, 0x80]);

    // Проверка E-bit (старший бит трейлера = 1)
    assert_ne!(packet[27] & 0x80, 0, "E-bit (bit 31) обязан быть выставлен");
}

#[test]
fn m15_udp_socket_rotation_due_at_60080ms_sends_discovery_and_28b_srtcp_rr() {
    use discord_voice::udp::{
        DISCOVERY_LEN, DiscoveryPacket, RotationSchedule, SRTCP_EMPTY_RR_LEN, UdpMediaSocket,
    };

    let server = match std::net::UdpSocket::bind("127.0.0.1:0") {
        Ok(socket) => socket,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skip: loopback UDP unavailable: {err}");
            return;
        }
        Err(err) => panic!("server bind failed: {err}"),
    };
    let server_addr = server.local_addr().unwrap();

    let server_handle = std::thread::spawn(move || {
        let mut packets = Vec::new();
        let mut buf = [0u8; 128];
        // 1 initial discovery + 1 rotated discovery + 1 rotated 28B SRTCP RR
        for _ in 0..3 {
            let (size, peer) = server.recv_from(&mut buf).unwrap();
            packets.push((buf[..size].to_vec(), peer));
            if size == DISCOVERY_LEN {
                server.send_to(&buf[..size], peer).unwrap();
            }
        }
        packets
    });

    let mut client = match UdpMediaSocket::bind("127.0.0.1:0") {
        Ok(client) => client,
        Err(err) => {
            eprintln!("skip: loopback UDP client bind: {err}");
            return;
        }
    };
    client.send_discovery(server_addr, 9208).unwrap();
    let initial_addr = client.local_addr().unwrap();

    let mut schedule = RotationSchedule::new(0);
    let tag = [0xabu8; 16];
    let mut packet_counter = 100u32;

    // До наступления 60 080 мс ротация не срабатывает, счётчик не инкрементируется
    let early = client
        .rotate_if_due_srtcp(60_079, &mut schedule, tag, &mut packet_counter)
        .unwrap();
    assert!(early.is_none());
    assert_eq!(packet_counter, 100);
    assert_eq!(client.local_addr().unwrap(), initial_addr);

    // Ровно в 60 080 мс — ротация
    let action = client
        .rotate_if_due_srtcp(60_080, &mut schedule, tag, &mut packet_counter)
        .unwrap()
        .expect("rotation must be due at 60_080 ms");

    // Проверяем инкремент сквозного счётчика пакетов (100 -> 101)
    assert_eq!(packet_counter, 101, "счётчик пакетов инкрементирован");

    // Проверяем сформированные стартовые пакеты: 74B Discovery + 28B SRTCP RR
    assert_eq!(action.discovery.len(), DISCOVERY_LEN);
    assert_eq!(action.empty_rr.len(), SRTCP_EMPTY_RR_LEN);
    assert_eq!(action.empty_rr.len(), 28);

    // Discovery содержит актуальный порт нового сокета
    let new_addr = client.local_addr().unwrap();
    assert_ne!(new_addr.port(), initial_addr.port());
    assert_eq!(
        DiscoveryPacket::parse(&action.discovery),
        Ok(DiscoveryPacket::Request {
            ssrc: 9208,
            port: new_addr.port(),
        })
    );

    // 28B SRTCP RR содержит заголовок RTCP, SSRC, tag и счётчик 100 с E-bit
    assert_eq!(&action.empty_rr[0..4], &[0x80, 0xc9, 0x00, 0x01]);
    assert_eq!(&action.empty_rr[4..8], &9208u32.to_be_bytes());
    assert_eq!(&action.empty_rr[8..24], &tag);
    let expected_trailer = (0x8000_0000u32 | 100).to_le_bytes();
    assert_eq!(&action.empty_rr[24..28], &expected_trailer);

    // Повторный вызов в ту же миллисекунду не триггерит повторную ротацию
    let repeated = client
        .rotate_if_due_srtcp(60_080, &mut schedule, tag, &mut packet_counter)
        .unwrap();
    assert!(repeated.is_none());
    assert_eq!(packet_counter, 101);

    // Сервер получил discovery + 28B RR от нового порта
    let packets = server_handle.join().unwrap();
    assert_eq!(packets.len(), 3);
    assert_eq!(packets[0].0.len(), 74);
    assert_eq!(packets[0].1, initial_addr);

    assert_eq!(packets[1].0.len(), 74);
    assert_eq!(packets[1].1, new_addr);

    assert_eq!(packets[2].0.len(), 28);
    assert_eq!(packets[2].1, new_addr);
    assert_eq!(packets[2].0, action.empty_rr.to_vec());
}

#[test]
fn m16_udp_socket_rotation_closes_old_socket_and_increments_shared_counter() {
    use discord_voice::udp::{DISCOVERY_LEN, RotationSchedule, UdpMediaSocket};

    let server = match std::net::UdpSocket::bind("127.0.0.1:0") {
        Ok(socket) => socket,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skip: loopback UDP unavailable: {err}");
            return;
        }
        Err(err) => panic!("server bind failed: {err}"),
    };
    let server_addr = server.local_addr().unwrap();

    let server_handle = std::thread::spawn(move || {
        let mut buf = [0u8; 128];
        // 1 initial discovery + 2 x (discovery + 28B RR) for two rotations = 5 packets
        for _ in 0..5 {
            let (size, peer) = server.recv_from(&mut buf).unwrap();
            if size == DISCOVERY_LEN {
                server.send_to(&buf[..size], peer).unwrap();
            }
        }
    });

    let mut client = match UdpMediaSocket::bind("127.0.0.1:0") {
        Ok(client) => client,
        Err(err) => {
            eprintln!("skip: loopback UDP client bind: {err}");
            return;
        }
    };
    client.send_discovery(server_addr, 9208).unwrap();
    let old_addr = client.local_addr().unwrap();

    let mut schedule = RotationSchedule::new(10);
    let tag = [0x55u8; 16];
    let mut counter = 42u32;

    // Ротация #1 на 60_090 мс (10 + 60_080)
    let action1 = client
        .rotate_if_due_srtcp(60_090, &mut schedule, tag, &mut counter)
        .unwrap()
        .expect("rotation #1 due");

    assert_eq!(counter, 43, "счётчик увеличился с 42 до 43");
    let intermediate_addr = client.local_addr().unwrap();
    assert_ne!(intermediate_addr.port(), old_addr.port());

    // КРИТИЧЕСКИЙ ТЕСТ: старый сокет закрыт, его локальный адрес и порт освобождены
    let old_bind_probe = std::net::UdpSocket::bind(old_addr);
    assert!(
        old_bind_probe.is_ok(),
        "старый сокет закрыт, перепривязка к old_addr успешна"
    );

    // Ротация #2 на 120_170 мс (10 + 2 * 60_080) — продолжение сквозного счётчика
    let action2 = client
        .rotate_if_due_srtcp(120_170, &mut schedule, tag, &mut counter)
        .unwrap()
        .expect("rotation #2 due");

    assert_eq!(counter, 44, "счётчик увеличился с 43 до 44");
    let final_addr = client.local_addr().unwrap();
    assert_ne!(final_addr.port(), intermediate_addr.port());

    // Трейлер первого RR имел counter=42, трейлер второго counter=43
    let trailer1 = u32::from_le_bytes(action1.empty_rr[24..28].try_into().unwrap());
    let trailer2 = u32::from_le_bytes(action2.empty_rr[24..28].try_into().unwrap());
    assert_eq!(trailer1, 0x8000_0000 | 42);
    assert_eq!(trailer2, 0x8000_0000 | 43);

    // Старый intermediate_addr также закрыт
    let inter_probe = std::net::UdpSocket::bind(intermediate_addr);
    assert!(
        inter_probe.is_ok(),
        "промежуточный сокет закрыт, его порт освобождён"
    );

    server_handle.join().unwrap();
}
