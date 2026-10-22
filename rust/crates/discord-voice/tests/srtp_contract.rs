//! Контракт SRTP/SRTCP-кадрирования карточки P4-03.
//!
//! Оракул раскладки — capture медиа-пакетов движка (`strace-media-s3.jsonl`,
//! разбор `waves/RW-25/research/media-packet-layout.mjs`): байты эталонных кадров
//! взяты из реального трафика, раскладка —
//! `[clear][ciphertext][tag 16][counter 4 LE]`, исходящий счётчик `0x80000000 | n`,
//! где `n` общий для RTP и SRTCP одного сокета.
//!
//! Ключевого материала в capture нет, поэтому совместимость с wire Discord не
//! проверяется: тесты фиксируют самосогласованный формат и привязку AAD.

use discord_voice::crypto::CryptoError;
use discord_voice::srtp::{EncryptionMode, SrtpContext, SrtpError};

/// Разбор hex-строки в байты (в тесте — без внешних зависимостей).
fn hex(s: &str) -> Vec<u8> {
    assert!(
        s.len().is_multiple_of(2),
        "hex-строка должна иметь чётную длину"
    );
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("валидный hex"))
        .collect()
}

/// Эталонный мастер-ключ (32 Б) и мастер-соль (14 Б) для тестов.
const MASTER_KEY: [u8; 32] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
];
const MASTER_SALT: [u8; 14] = [
    0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xab, 0xac, 0xad,
];

/// Capture s3, исходящий RTP-кадр 47 Б: clear-область 24 Б (12 Б фикс. заголовка +
/// ext-заголовок `0xBEDE`/2 слова + 8 Б данных расширения), 3 Б payload, tag 16 Б,
/// счётчик 4 Б LE = `0x80000000`.
const RTP_CAPTURE_47: &str = "9078443a1099214c000023f8bede0002a62bf0cc7fc0b826489648de2a8560f6\
                              c08127ba388baea809dfee00000080";
/// Clear-область того же кадра (24 Б) — она должна уйти в кадр байт-в-байт.
const RTP_CLEAR_24: &str = "9078443a1099214c000023f8bede0002a62bf0cc7fc0b826";
/// Payload того же кадра (3 Б) — в тесте это открытый текст для шифрования.
const RTP_PAYLOAD_3: &str = "489648";

/// Capture s3, исходящий SRTCP-кадр 60 Б: clear 8 Б (заголовок RTCP + SSRC),
/// ciphertext 32 Б, tag 16 Б, счётчик `0x80000008`.
const SRTCP_CLEAR_8: &str = "80c80006000023f8";
const SRTCP_PAYLOAD_32: &str = "8abb34e79b75ca2c3872445b001d6ca27b9c764920730f08afde72c4e657d806";
/// Capture s3, исходящий пустой RR 28 Б: clear 8 Б, ciphertext 0 Б, tag 16 Б,
/// счётчик `0x80000016`.
const RR_CLEAR_8: &str = "80c90001000023f8";

/// Capture s3, два первых пакета talkspurt (43 Б): расширение с невалидным
/// профилем/длиной и без трейлера-счётчика ⇒ это не SRTP-кадр нашего формата.
const TALKSPURT_43_A: &str = "90f84438109919cc000023f819f5e987418252362f4e0076ca9d80de6f75\
                              10489d85d21e50a5615b017af3";
const TALKSPURT_43_B: &str = "9078443910991d8c000023f82d6d6f134451bcebb4dd8580830ab192bc4c2\
                              07cf7a2b2128bb13cd5e4276e";

/// Capture s3, входящий SRTCP 52 Б: clear 8 Б, ciphertext 24 Б, tag 16 Б,
/// счётчик `0x00000000` (входящие счётчики не монотонны).
const INCOMING_SRTCP_52: &str = "81c90007000023f8b58acbc1675a4d35a0ac12ccf8f151bc0f002888166d180d33c307bba4d3e9d3fc\
     94880e9397111d00000000";

/// Открытый RTP-кадр для шифрования: clear-область capture + открытый payload.
fn rtp_plaintext() -> Vec<u8> {
    let mut frame = hex(RTP_CLEAR_24);
    frame.extend_from_slice(&hex(RTP_PAYLOAD_3));
    frame
}

/// Трейлер (счётчик) кадра: последние 4 Б, little endian.
fn trailer(frame: &[u8]) -> u32 {
    let tail = &frame[frame.len() - 4..];
    u32::from_le_bytes([tail[0], tail[1], tail[2], tail[3]])
}

/// Контекст в режиме `aead_aes256_gcm_rtpsize`.
fn context() -> SrtpContext {
    SrtpContext::new(&MASTER_KEY, &MASTER_SALT, EncryptionMode::Aes256GcmRtpSize)
}

/// Раскладка RTP-кадра: 47 Б, clear-область сохранена, tag и трейлер на местах.
#[test]
fn rtp_layout_k() {
    let mut ctx = context();
    let plain = rtp_plaintext();
    let frame = ctx.encrypt_rtp(&plain).expect("шифрование RTP");

    assert_eq!(frame.len(), 47, "раскладка capture: 24 + 3 + 16 + 4 = 47 Б");
    assert_eq!(
        &frame[..24],
        &hex(RTP_CLEAR_24)[..],
        "clear-область байт-в-байт"
    );
    // Эталонный кадр capture: та же clear-область и тот же трейлер.
    let capture = hex(RTP_CAPTURE_47);
    assert_eq!(capture.len(), 47);
    assert_eq!(&capture[..24], &hex(RTP_CLEAR_24)[..]);
    assert_eq!(
        &capture[43..],
        &[0x00, 0x00, 0x00, 0x80],
        "трейлер capture = 0x80000000"
    );
    assert_eq!(frame[frame.len() - 20..frame.len() - 4].len(), 16);
    assert_eq!(
        trailer(&frame),
        0x8000_0000,
        "исходящий счётчик = 0x80000000 | n"
    );
    assert_ne!(
        &frame[24..27],
        &hex(RTP_PAYLOAD_3)[..],
        "payload должен быть зашифрован"
    );
}

/// Счётчик общий для RTP и SRTCP: 8 RTP-кадров, затем SRTCP ⇒ `0x80000000..0x80000008`.
#[test]
fn rtp_counter_shared_k() {
    let mut ctx = context();
    let plain = rtp_plaintext();
    for n in 0..8u32 {
        let frame = ctx.encrypt_rtp(&plain).expect("шифрование RTP");
        assert_eq!(trailer(&frame), 0x8000_0000 | n, "n = {n}");
    }
    let srtcp = ctx
        .encrypt_srtcp(&hex(SRTCP_CLEAR_8))
        .expect("шифрование SRTCP");
    assert_eq!(
        trailer(&srtcp),
        0x8000_0008,
        "SRTCP продолжает сквозной счётчик RTP"
    );
    assert_eq!(ctx.counter(), 9, "счётчик вырос на 9 кадров");
}

/// Раскладка SRTCP: 60 Б (clear 8 + ct 32 + tag 16 + трейлер) и пустой RR 28 Б.
#[test]
fn srtcp_layout_k() {
    let mut ctx = context();
    let mut plain = hex(SRTCP_CLEAR_8);
    plain.extend_from_slice(&hex(SRTCP_PAYLOAD_32));
    let frame = ctx.encrypt_srtcp(&plain).expect("шифрование SRTCP");

    assert_eq!(frame.len(), 60, "раскладка capture: 8 + 32 + 16 + 4 = 60 Б");
    assert_eq!(
        &frame[..8],
        &hex(SRTCP_CLEAR_8)[..],
        "clear-область байт-в-байт"
    );
    assert_eq!(
        trailer(&frame),
        0x8000_0000,
        "первый SRTCP в этом контексте"
    );
    assert_ne!(
        &frame[8..40],
        &hex(SRTCP_PAYLOAD_32)[..],
        "payload зашифрован"
    );

    // Пустой RR: clear 8 Б, ciphertext 0 Б, tag 16 Б, трейлер 4 Б.
    let rr = ctx
        .encrypt_srtcp(&hex(RR_CLEAR_8))
        .expect("шифрование пустого RR");
    assert_eq!(rr.len(), 28, "раскладка capture: 8 + 0 + 16 + 4 = 28 Б");
    assert_eq!(&rr[..8], &hex(RR_CLEAR_8)[..]);
    assert_eq!(trailer(&rr), 0x8000_0001, "счётчик продолжает расти");
}

/// Round-trip RTP: `decrypt_rtp(encrypt_rtp(x)) == x`.
#[test]
fn rtp_roundtrip_k() {
    let mut ctx = context();
    let plain = rtp_plaintext();
    let frame = ctx.encrypt_rtp(&plain).expect("шифрование RTP");
    let back = ctx.decrypt_rtp(&frame).expect("расшифрование RTP");
    assert_eq!(back, plain);
}

/// Round-trip SRTCP: `decrypt_srtcp(encrypt_srtcp(x)) == x`.
#[test]
fn srtcp_roundtrip_k() {
    let mut ctx = context();
    let mut plain = hex(SRTCP_CLEAR_8);
    plain.extend_from_slice(&hex(SRTCP_PAYLOAD_32));
    let frame = ctx.encrypt_srtcp(&plain).expect("шифрование SRTCP");
    let back = ctx.decrypt_srtcp(&frame).expect("расшифрование SRTCP");
    assert_eq!(back, plain);
}

/// Правка байта в ciphertext ⇒ `AuthenticationFailed`.
#[test]
fn rtp_tamper_k() {
    let mut ctx = context();
    let plain = rtp_plaintext();
    let frame = ctx.encrypt_rtp(&plain).expect("шифрование RTP");

    let mut bad = frame.clone();
    bad[24] ^= 0x01; // первый байт ciphertext
    assert_eq!(
        ctx.decrypt_rtp(&bad),
        Err(SrtpError::AuthenticationFailed),
        "правка ciphertext ломает аутентификацию"
    );

    let mut bad = frame.clone();
    bad[2] ^= 0x01; // байт в clear-области (AAD)
    assert_eq!(
        ctx.decrypt_rtp(&bad),
        Err(SrtpError::AuthenticationFailed),
        "правка clear-области (AAD) ломает аутентификацию"
    );
}

/// Правка байта в SRTCP-кадре ⇒ `AuthenticationFailed`.
#[test]
fn srtcp_tamper_k() {
    let mut ctx = context();
    let mut plain = hex(SRTCP_CLEAR_8);
    plain.extend_from_slice(&hex(SRTCP_PAYLOAD_32));
    let frame = ctx.encrypt_srtcp(&plain).expect("шифрование SRTCP");

    let mut bad = frame.clone();
    bad[10] ^= 0x01;
    assert_eq!(
        ctx.decrypt_srtcp(&bad),
        Err(SrtpError::AuthenticationFailed)
    );

    let mut bad = frame.clone();
    bad[1] ^= 0x01; // PT в clear-области RTCP
    assert_eq!(
        ctx.decrypt_srtcp(&bad),
        Err(SrtpError::AuthenticationFailed)
    );
}

/// 43-байтовые пакеты talkspurt ⇒ явная ошибка, без паники.
#[test]
fn malformed_43b_k() {
    let mut ctx = context();
    for (name, hex_str) in [("A", TALKSPURT_43_A), ("B", TALKSPURT_43_B)] {
        let packet = hex(hex_str);
        assert_eq!(packet.len(), 43);
        let err = ctx
            .encrypt_rtp(&packet)
            .expect_err("пакет talkspurt не является кадром нашего формата");
        assert!(
            matches!(err, SrtpError::MalformedHeader { .. }),
            "кадр {name}: ожидалась MalformedHeader, получено {err:?}"
        );
    }
}

/// Короткие и обрезанные пакеты ⇒ `PacketTooShort`, без паники.
#[test]
fn short_packet_k() {
    let mut ctx = context();
    assert_eq!(
        ctx.encrypt_rtp(&[0u8; 5]),
        Err(SrtpError::PacketTooShort { len: 5, min: 12 })
    );
    assert_eq!(
        ctx.encrypt_srtcp(&[0u8; 4]),
        Err(SrtpError::PacketTooShort { len: 4, min: 8 })
    );
    // RTP-заголовок без хвоста (tag + счётчик) — не SRTP-кадр.
    let mut header = hex(RTP_CLEAR_24);
    header.truncate(12);
    header[0] &= !0x10; // без расширения: clear-область ровно 12 Б
    assert_eq!(
        ctx.decrypt_rtp(&header),
        Err(SrtpError::PacketTooShort { len: 12, min: 32 })
    );
    // Обрезанный заголовок расширения — тоже явная ошибка, не паника.
    let mut ext_header = hex(RTP_CLEAR_24);
    ext_header.truncate(12);
    assert!(matches!(
        ctx.decrypt_rtp(&ext_header),
        Err(SrtpError::PacketTooShort { .. })
    ));
}

/// Имена режимов движка: разбор и обратное преобразование.
#[test]
fn mode_names_k() {
    assert_eq!(
        EncryptionMode::from_engine_name("aead_aes256_gcm_rtpsize"),
        Some(EncryptionMode::Aes256GcmRtpSize)
    );
    assert_eq!(
        EncryptionMode::from_engine_name("aead_aes256_gcm"),
        Some(EncryptionMode::Aes256Gcm)
    );
    assert_eq!(
        EncryptionMode::from_engine_name("aead_xchacha20_poly1305_rtpsize"),
        Some(EncryptionMode::XChaCha20Poly1305RtpSize)
    );
    assert_eq!(
        EncryptionMode::from_engine_name("xsalsa20_poly1305_lite"),
        None,
        "режим вне карточки не разбирается"
    );

    for mode in [
        EncryptionMode::Aes256GcmRtpSize,
        EncryptionMode::Aes256Gcm,
        EncryptionMode::XChaCha20Poly1305RtpSize,
    ] {
        assert_eq!(
            EncryptionMode::from_engine_name(mode.as_engine_name()),
            Some(mode)
        );
    }

    assert_eq!(
        EncryptionMode::Aes256GcmRtpSize.as_engine_name(),
        "aead_aes256_gcm_rtpsize"
    );
    assert_eq!(
        EncryptionMode::Aes256Gcm.as_engine_name(),
        "aead_aes256_gcm"
    );
    assert_eq!(
        EncryptionMode::XChaCha20Poly1305RtpSize.as_engine_name(),
        "aead_xchacha20_poly1305_rtpsize"
    );

    assert!(EncryptionMode::Aes256GcmRtpSize.covers_header());
    assert!(EncryptionMode::XChaCha20Poly1305RtpSize.covers_header());
    assert!(!EncryptionMode::Aes256Gcm.covers_header());
    assert_eq!(EncryptionMode::Aes256Gcm.tag_size(), 16);
    assert_eq!(EncryptionMode::Aes256GcmRtpSize.tag_size(), 16);
}

/// Режим вне карточки (`xsalsa20_poly1305*`) ⇒ `UnsupportedMode`.
#[test]
fn unsupported_mode_k() {
    let err = SrtpContext::from_engine_name(&MASTER_KEY, &MASTER_SALT, "xsalsa20_poly1305_lite")
        .expect_err("режим вне карточки");
    assert_eq!(
        err,
        SrtpError::UnsupportedMode {
            name: "xsalsa20_poly1305_lite"
        }
    );
}

/// Режим `aead_aes256_gcm` (не `rtpsize`): AAD пустой, clear-области в выводе нет.
#[test]
fn non_rtpsize_mode_k() {
    let mut ctx = SrtpContext::new(&MASTER_KEY, &MASTER_SALT, EncryptionMode::Aes256Gcm);
    let plain = rtp_plaintext();
    let frame = ctx.encrypt_rtp(&plain).expect("шифрование RTP");

    assert_eq!(
        frame.len(),
        plain.len() + 20,
        "шифротекст + tag 16 + счётчик 4"
    );
    assert_ne!(
        &frame[..12],
        &plain[..12],
        "заголовок тоже зашифрован — clear-области нет"
    );
    assert_eq!(trailer(&frame), 0x8000_0000);
    assert_eq!(ctx.decrypt_rtp(&frame).expect("расшифрование"), plain);
}

/// Переполнение 32-битного счётчика ⇒ `CounterOverflow`, без паники.
#[test]
fn counter_overflow_k() {
    let mut ctx = context();
    ctx.set_counter(0x7FFF_FFFF).expect("максимальный счётчик");
    let frame = ctx.encrypt_rtp(&rtp_plaintext()).expect("шифрование");
    assert_eq!(
        trailer(&frame),
        0xFFFF_FFFF,
        "последний представимый счётчик"
    );

    let before = ctx.counter();
    assert_eq!(
        ctx.encrypt_rtp(&rtp_plaintext()),
        Err(SrtpError::CounterOverflow)
    );
    assert_eq!(ctx.counter(), before, "счётчик не растёт при ошибке");
    assert!(matches!(
        ctx.set_counter(u64::MAX),
        Err(SrtpError::CounterOverflow)
    ));
}

/// Версия RTP ≠ 2 ⇒ `MalformedHeader`.
#[test]
fn rtp_version_k() {
    let mut ctx = context();
    let mut frame = rtp_plaintext();
    frame[0] &= 0x3F; // V = 1
    assert!(matches!(
        ctx.encrypt_rtp(&frame),
        Err(SrtpError::MalformedHeader { .. })
    ));
}

/// Длина RTP-расширения не влезает в пакет ⇒ `MalformedHeader`.
#[test]
fn rtp_ext_len_k() {
    let mut ctx = context();
    let mut frame = vec![0x90u8; 12]; // V = 2, X = 1, CC = 0
    frame.extend_from_slice(&[0xbe, 0xde, 0x00, 0x64]); // профиль 0xBEDE, 100 слов
    assert!(matches!(
        ctx.encrypt_rtp(&frame),
        Err(SrtpError::MalformedHeader { .. })
    ));
}

/// Версия RTCP ≠ 2 ⇒ `MalformedHeader`.
#[test]
fn srtcp_version_k() {
    let mut ctx = context();
    let mut frame = hex(SRTCP_CLEAR_8);
    frame[0] &= 0x3F; // V = 1
    assert!(matches!(
        ctx.encrypt_srtcp(&frame),
        Err(SrtpError::MalformedHeader { .. })
    ));
}

/// Входящие счётчики не монотонны — расшифровка монотонность не проверяет.
#[test]
fn incoming_counter_non_monotonic_k() {
    // Отправители с разными значениями счётчика (в capture: 0x00..0x0a, затем 0x76, 0x66).
    let mut sender_high = context();
    sender_high.set_counter(0x10).expect("счётчик отправителя");
    let high = sender_high
        .encrypt_rtp(&rtp_plaintext())
        .expect("шифрование");
    assert_eq!(trailer(&high), 0x8000_0010);

    let mut sender_low = context();
    sender_low.set_counter(0x02).expect("счётчик отправителя");
    let low = sender_low
        .encrypt_rtp(&rtp_plaintext())
        .expect("шифрование");
    assert_eq!(
        trailer(&low),
        0x8000_0002,
        "счётчик уменьшился — это не ошибка"
    );

    // Приёмник со своим счётчиком (0) принимает оба кадра: монотонность не проверяется.
    let mut receiver = context();
    assert_eq!(
        receiver.decrypt_rtp(&high).expect("расшифрование"),
        rtp_plaintext()
    );
    assert_eq!(
        receiver.decrypt_rtp(&low).expect("расшифрование"),
        rtp_plaintext()
    );
}

/// Режим `aead_xchacha20_poly1305_rtpsize`: раскладка и round-trip.
#[test]
fn xchacha_rtpsize_k() {
    let mut ctx = SrtpContext::new(
        &MASTER_KEY,
        &MASTER_SALT,
        EncryptionMode::XChaCha20Poly1305RtpSize,
    );
    let plain = rtp_plaintext();
    let frame = ctx.encrypt_rtp(&plain).expect("шифрование RTP");
    assert_eq!(frame.len(), 47);
    assert_eq!(&frame[..24], &hex(RTP_CLEAR_24)[..]);
    assert_eq!(trailer(&frame), 0x8000_0000);
    assert_eq!(ctx.decrypt_rtp(&frame).expect("расшифрование"), plain);
}

/// Входящий SRTCP из capture (52 Б, счётчик `0x00000000`) разбирается без паники.
#[test]
fn incoming_srtcp_k() {
    let mut ctx = context();
    let frame = hex(INCOMING_SRTCP_52);
    assert_eq!(frame.len(), 52);
    // Ключ другой ⇒ аутентификация не проходит, но ошибка — ожидаемый тип.
    let err = ctx.decrypt_srtcp(&frame).expect_err("чужой ключ");
    assert!(
        matches!(err, SrtpError::AuthenticationFailed | SrtpError::Crypto(_)),
        "неожиданная ошибка: {err:?}"
    );
}

/// Ошибка примитива пробрасывается как `Crypto(CryptoError)`.
#[test]
fn crypto_error_k() {
    let err = SrtpError::from(CryptoError::InvalidTagLength {
        expected: 16,
        actual: 4,
    });
    assert_eq!(
        err,
        SrtpError::Crypto(CryptoError::InvalidTagLength {
            expected: 16,
            actual: 4,
        })
    );
}

/// Перемотка счётчика назад запрещена: повтор nonce GCM под тем же ключом
/// раскрывает XOR открытых текстов и позволяет подделать тег.
#[test]
fn counter_rewind_rejected_k() {
    let mut ctx = context();
    ctx.set_counter(0x10).expect("счётчик вперёд");
    let err = ctx
        .set_counter(0x02)
        .expect_err("перемотка назад запрещена");
    assert_eq!(
        err,
        SrtpError::CounterRewind {
            current: 0x10,
            requested: 0x02,
        }
    );
    assert_eq!(ctx.counter(), 0x10, "счётчик не изменился");

    // Движение вперёд — разрешено.
    ctx.set_counter(0x20).expect("счётчик вперёд");
    assert_eq!(ctx.counter(), 0x20);
}

/// Повтор трейлера (nonce) на приёме отклоняется: иначе GCM под тем же ключом
/// раскрывает XOR открытых текстов и позволяет подделать тег.
#[test]
fn decrypt_nonce_reuse_rejected_k() {
    let mut sender = context();
    let frame = sender.encrypt_rtp(&rtp_plaintext()).expect("шифрование");

    let mut receiver = context();
    assert_eq!(
        receiver.decrypt_rtp(&frame).expect("первая расшифровка"),
        rtp_plaintext()
    );
    let err = receiver
        .decrypt_rtp(&frame)
        .expect_err("повтор nonce отклоняется");
    assert_eq!(
        err,
        SrtpError::NonceReuse {
            trailer: 0x8000_0000
        }
    );

    // Кадр с другим трейлером (другой nonce) принимается.
    let mut sender_other = context();
    sender_other.set_counter(0x10).expect("счётчик отправителя");
    let other = sender_other
        .encrypt_rtp(&rtp_plaintext())
        .expect("шифрование");
    assert_eq!(
        receiver.decrypt_rtp(&other).expect("другой трейлер"),
        rtp_plaintext()
    );

    // SRTCP: тот же контракт.
    let mut srtcp_sender = context();
    let srtcp = srtcp_sender
        .encrypt_srtcp(&hex(SRTCP_CLEAR_8))
        .expect("шифрование SRTCP");
    let mut srtcp_receiver = context();
    assert!(srtcp_receiver.decrypt_srtcp(&srtcp).is_ok());
    assert_eq!(
        srtcp_receiver
            .decrypt_srtcp(&srtcp)
            .expect_err("повтор nonce отклоняется"),
        SrtpError::NonceReuse {
            trailer: 0x8000_0000
        }
    );
}
