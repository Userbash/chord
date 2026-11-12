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

use chord_voice::crypto::CryptoError;
use chord_voice::srtp::{EncryptionMode, SrtpContext, SrtpError};

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

#[test]
fn dave_mls_key_ratchet_forward_securing_out_of_order_cache_and_generation_gap_bounds() {
    use chord_voice::dave::{
        DAVE_MAX_GENERATION_GAP, DAVE_MAX_LIFETIME_GENERATIONS, DAVE_RATCHET_GENERATION_SHIFT_BITS,
        DaveMediaCipher, MlsKeyRatchet, compute_wrapped_big_nonce, compute_wrapped_generation,
    };

    let epoch_secret = [0x42u8; 32];
    let mut ratchet_a = MlsKeyRatchet::new(1, 1001, &epoch_secret);
    let mut ratchet_b = MlsKeyRatchet::new(1, 1001, &epoch_secret);
    let mut ratchet_other_user = MlsKeyRatchet::new(1, 2002, &epoch_secret);
    let mut ratchet_other_epoch = MlsKeyRatchet::new(2, 1001, &epoch_secret);

    assert_eq!(ratchet_a.epoch(), 1);
    assert_eq!(ratchet_a.user_id(), 1001);
    assert_eq!(ratchet_a.next_generation(), 0);
    assert_eq!(ratchet_a.oldest_generation(), 0);

    // Детерминизм по (epoch, user_id, epoch_secret) и изоляция пользователей/эпох.
    let k0_a = ratchet_a.get_key_and_nonce(0).expect("generation 0");
    let k0_b = ratchet_b.get_key_and_nonce(0).expect("generation 0 copy");
    let k0_other_user = ratchet_other_user
        .get_key_and_nonce(0)
        .expect("other user generation 0");
    let k0_other_epoch = ratchet_other_epoch
        .get_key_and_nonce(0)
        .expect("other epoch generation 0");
    assert_eq!(k0_a.key, k0_b.key);
    assert_eq!(k0_a.nonce_base, k0_b.nonce_base);
    assert_ne!(k0_a.key, k0_other_user.key);
    assert_ne!(k0_a.key, k0_other_epoch.key);
    assert_eq!(ratchet_a.next_generation(), 1);
    assert!(ratchet_a.has_cached_key(0));

    // Прыжок вперёд (out-of-order пакеты): промежуточные поколения 1..=4 кэшируются,
    // а секрет цепочки продвигается до поколения 6.
    let k5 = ratchet_a.get_key(5).expect("generation 5");
    assert_eq!(ratchet_a.next_generation(), 6);
    for g in 0..=5 {
        assert!(ratchet_a.has_cached_key(g), "generation {g} must be cached");
    }
    let k3_late = ratchet_a.get_key(3).expect("cached generation 3");
    assert_eq!(k3_late, ratchet_b.get_key(3).expect("sequential gen 3"));
    assert_eq!(k5, ratchet_b.get_key(5).expect("sequential gen 5"));

    // Forward Secrecy: после delete_key(0) и delete_keys_up_to(3) старые ключи
    // невозможно вычислить повторно или достать из кэша.
    assert!(ratchet_a.delete_key(0));
    assert!(!ratchet_a.delete_key(0));
    assert!(!ratchet_a.has_cached_key(0));
    assert_eq!(ratchet_a.oldest_generation(), 1);
    assert_eq!(
        ratchet_a.get_key(0).expect_err("deleted generation 0"),
        SrtpError::CounterRewind {
            current: 1,
            requested: 0,
        }
    );
    let pruned = ratchet_a.delete_keys_up_to(3);
    assert_eq!(pruned, 2); // удалены поколения 1 и 2
    assert_eq!(ratchet_a.oldest_generation(), 3);
    assert!(ratchet_a.get_key(2).is_err());
    assert_eq!(ratchet_a.get_key(3).expect("gen 3 still alive"), k3_late);

    // Граничное условие DAVE_MAX_GENERATION_GAP (250):
    // На свежем рэтчете при next_generation == 0 прыжок на 250 допустим, на 251 — отказ.
    let mut gap_ratchet = MlsKeyRatchet::new(1, 3003, &epoch_secret);
    assert!(matches!(
        gap_ratchet
            .get_key(DAVE_MAX_GENERATION_GAP + 1)
            .expect_err("gap 251 must fail"),
        SrtpError::MalformedHeader { .. }
    ));
    assert!(
        gap_ratchet.get_key(DAVE_MAX_GENERATION_GAP).is_ok(),
        "exact DAVE_MAX_GENERATION_GAP (250) must succeed"
    );
    assert_eq!(
        gap_ratchet
            .get_key(DAVE_MAX_LIFETIME_GENERATIONS)
            .expect_err("generation 256 exceeds max lifetime"),
        SrtpError::CounterOverflow
    );

    // Проверка обёртки поколений и 64-битного nonce (compute_wrapped_generation / compute_wrapped_big_nonce).
    assert_eq!(compute_wrapped_generation(0, 0), 0);
    assert_eq!(compute_wrapped_generation(0, 5), 5);
    assert_eq!(compute_wrapped_generation(250, 255), 255);
    assert_eq!(compute_wrapped_generation(250, 2), 258);
    assert_eq!(compute_wrapped_big_nonce(0, 1), 1);
    assert_eq!(
        compute_wrapped_big_nonce(0x0000_0001_FFFF_FFF0, 0x0000_0005),
        0x0000_0002_0000_0005
    );

    // Сквозная работа DaveMediaCipher через смену поколений (generation = nonce >> 24).
    let mut sender_cipher =
        DaveMediaCipher::with_ratchet(MlsKeyRatchet::new(1, 1001, &epoch_secret));
    let mut receiver_cipher =
        DaveMediaCipher::with_ratchet(MlsKeyRatchet::new(1, 1001, &epoch_secret));

    // Поколение 0 (counter = 1)
    let frame_gen0 = sender_cipher.encrypt(b"opus-gen-0").expect("encrypt gen 0");
    // Переводим счётчик отправителя в поколение 2 (2 << 24 | 7)
    sender_cipher.set_counter((2 << DAVE_RATCHET_GENERATION_SHIFT_BITS) | 7);
    let frame_gen2 = sender_cipher.encrypt(b"opus-gen-2").expect("encrypt gen 2");
    // На стороне отправителя старые поколения < 2 автоматически очищены (forward secrecy)
    assert_eq!(sender_cipher.ratchet().oldest_generation(), 2);
    assert!(!sender_cipher.ratchet().has_cached_key(0));

    // Получатель принимает сначала пакет поколения 2, затем задержанный пакет поколения 0
    assert_eq!(
        receiver_cipher
            .decrypt(&frame_gen2)
            .expect("decrypt gen 2 first"),
        b"opus-gen-2"
    );
    assert_eq!(
        receiver_cipher
            .decrypt(&frame_gen0)
            .expect("decrypt late gen 0 from cache"),
        b"opus-gen-0"
    );

    // После явного удаления поколения 0 на получателе повторная расшифровка невозможна
    assert!(receiver_cipher.ratchet_mut().delete_key(0));
    assert!(receiver_cipher.decrypt(&frame_gen0).is_err());

    // Наблюдаемость: Debug-представление MlsKeyRatchet и MlsRatchetKeyAndNonce маскирует секреты
    let dbg_ratchet = format!("{:?}", receiver_cipher.ratchet());
    let dbg_key = format!("{:?}", k0_a);
    assert!(dbg_ratchet.contains("[REDACTED]"));
    assert!(dbg_key.contains("[REDACTED]"));
}

#[test]
fn dave_mls_structured_proposal_commit_and_welcome_verification_and_epoch_authenticator() {
    use chord_voice::dave::{
        DAVE_MAX_RESETS, DaveMlsEngine, DaveTransitionOutcome, MlsProposalType,
        MlsVerificationError, build_structured_mls_commit, build_structured_mls_proposal,
        build_structured_mls_revoke, build_structured_mls_welcome,
    };

    let channel_id = 0x0102_0304_0506_0708u64;
    let self_user_id = 1001u64;
    let peer_user_id = 2002u64;
    let intruder_user_id = 9999u64;
    let secret_key = [0x37u8; 32];

    let (mut engine, _op26) = DaveMlsEngine::init_on_op4(
        self_user_id,
        channel_id,
        1,
        &secret_key,
        EncryptionMode::Aes256GcmRtpSize,
    )
    .expect("init_on_op4");
    engine
        .on_clients_connect_op11(&[self_user_id, peer_user_id])
        .expect("on_clients_connect_op11");

    // До установки external_sender (op:25) структурированный proposal фиксирует MissingExternalSender и возвращает None
    let ext_sender_bytes = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
    let (valid_add_prop, prop_ref_1) = build_structured_mls_proposal(
        1,
        channel_id,
        0,
        MlsProposalType::Add,
        peer_user_id,
        &ext_sender_bytes,
    );
    assert!(
        engine
            .on_proposals_op27(&valid_add_prop)
            .expect("proposal before external sender is ignored")
            .is_none()
    );
    assert_eq!(
        engine.last_verification_error(),
        Some(&MlsVerificationError::MissingExternalSender)
    );

    // Устанавливаем external_sender (op:25 = [seq_be16][25][ext_sender_bytes])
    let mut op25 = vec![0x00, 0x01, 25];
    op25.extend_from_slice(&ext_sender_bytes);
    engine.on_external_sender_op25(&op25).expect("op25");

    // 1. Негативные сценарии валидации Proposal (op:27):
    // 1a. Неверный group_id (channel_id)
    let (wrong_group_prop, _) = build_structured_mls_proposal(
        2,
        channel_id ^ 1,
        0,
        MlsProposalType::Add,
        peer_user_id,
        &ext_sender_bytes,
    );
    assert!(engine.on_proposals_op27(&wrong_group_prop).is_err());
    assert_eq!(
        engine.last_verification_error(),
        Some(&MlsVerificationError::UnexpectedGroupId {
            expected: channel_id,
            actual: channel_id ^ 1,
        })
    );

    // 1b. Неверная эпоха (expected 0, actual 3)
    let (wrong_epoch_prop, _) = build_structured_mls_proposal(
        3,
        channel_id,
        3,
        MlsProposalType::Add,
        peer_user_id,
        &ext_sender_bytes,
    );
    assert!(engine.on_proposals_op27(&wrong_epoch_prop).is_err());
    assert_eq!(
        engine.last_verification_error(),
        Some(&MlsVerificationError::EpochMismatch {
            expected: 0,
            actual: 3,
        })
    );

    // 1c. Подпись от чужого external_sender
    let (bad_sig_prop, _) = build_structured_mls_proposal(
        4,
        channel_id,
        0,
        MlsProposalType::Add,
        peer_user_id,
        &[0xFF; 8],
    );
    assert!(engine.on_proposals_op27(&bad_sig_prop).is_err());
    assert_eq!(
        engine.last_verification_error(),
        Some(&MlsVerificationError::InvalidExternalSenderSignature)
    );

    // 1d. Add proposal для нераспознанного user_id (нет в recognized_users)
    let (unrecognized_user_prop, _) = build_structured_mls_proposal(
        5,
        channel_id,
        0,
        MlsProposalType::Add,
        intruder_user_id,
        &ext_sender_bytes,
    );
    assert!(engine.on_proposals_op27(&unrecognized_user_prop).is_err());
    assert_eq!(
        engine.last_verification_error(),
        Some(&MlsVerificationError::UnrecognizedUserId(intruder_user_id))
    );

    // 2. Позитивный сценарий Proposal append + revoke (op:27):
    let op28 = engine
        .on_proposals_op27(&valid_add_prop)
        .expect("valid add proposal")
        .expect("op28 generated");
    assert_eq!(op28[0], 28);
    assert_eq!(engine.pending_proposal_refs(), vec![prop_ref_1]);

    // Отзыв несуществующего proposal_ref отклоняется
    let bad_revoke = build_structured_mls_revoke(6, &[[0xEE; 16]]);
    assert!(engine.on_proposals_op27(&bad_revoke).is_err());
    assert_eq!(
        engine.last_verification_error(),
        Some(&MlsVerificationError::UnrecognizedProposalRef([0xEE; 16]))
    );

    // Отзыв реального prop_ref_1 очищает очередь
    let good_revoke = build_structured_mls_revoke(7, &[prop_ref_1]);
    assert!(
        engine
            .on_proposals_op27(&good_revoke)
            .expect("revoke proposal")
            .is_none()
    );
    assert!(engine.pending_proposal_refs().is_empty());

    // Commit без поставленных в очередь proposals отклоняется через op:31 InvalidCommitWelcome
    let premature_commit = build_structured_mls_commit(
        8,
        10,
        channel_id,
        1,
        &ext_sender_bytes,
        &[valid_add_prop[4..].to_vec()],
        &[self_user_id, peer_user_id],
    );
    match engine
        .on_commit_or_welcome_op29_op30(&premature_commit)
        .expect("commit without queued proposals triggers op31")
    {
        DaveTransitionOutcome::InvalidCommitWelcome { op31_json, .. } => {
            assert!(op31_json.contains("\"op\":31"));
        }
        other => panic!("expected InvalidCommitWelcome, got {other:?}"),
    }
    assert_eq!(
        engine.last_verification_error(),
        Some(&MlsVerificationError::CommitWithoutQueuedProposals)
    );

    // Снова добавляем валидный proposal в очередь
    engine
        .on_proposals_op27(&valid_add_prop)
        .expect("re-queue valid proposal");

    // 3. Негативные сценарии Commit (op:29):
    // 3a. Commit с неверной эпохой (target_epoch = 2 вместо 1)
    let bad_epoch_commit = build_structured_mls_commit(
        9,
        11,
        channel_id,
        2,
        &ext_sender_bytes,
        &[valid_add_prop[4..].to_vec()],
        &[self_user_id, peer_user_id],
    );
    assert!(matches!(
        engine
            .on_commit_or_welcome_op29_op30(&bad_epoch_commit)
            .expect("bad epoch commit"),
        DaveTransitionOutcome::InvalidCommitWelcome { .. }
    ));
    assert_eq!(
        engine.last_verification_error(),
        Some(&MlsVerificationError::EpochMismatch {
            expected: 1,
            actual: 2,
        })
    );

    // 3b. Commit с дубликатом в roster
    engine
        .on_proposals_op27(&valid_add_prop)
        .expect("re-queue after reset 2");
    let dup_roster_commit = build_structured_mls_commit(
        10,
        12,
        channel_id,
        1,
        &ext_sender_bytes,
        &[valid_add_prop[4..].to_vec()],
        &[self_user_id, self_user_id],
    );
    assert!(matches!(
        engine
            .on_commit_or_welcome_op29_op30(&dup_roster_commit)
            .expect("duplicate roster commit"),
        DaveTransitionOutcome::InvalidCommitWelcome { .. }
    ));
    assert_eq!(
        engine.last_verification_error(),
        Some(&MlsVerificationError::DuplicateIdentityInRoster(
            self_user_id
        ))
    );
    assert_eq!(engine.resets(), DAVE_MAX_RESETS);

    // 3c. Превышение лимита сбросов DAVE_MAX_RESETS (4-й невалидный коммит подряд)
    engine
        .on_proposals_op27(&valid_add_prop)
        .expect("re-queue after reset 3");
    assert!(matches!(
        engine
            .on_commit_or_welcome_op29_op30(&dup_roster_commit)
            .expect_err("4th invalid commit exceeds DAVE_MAX_RESETS"),
        SrtpError::MalformedHeader { .. }
    ));

    // 4. Позитивный сценарий структурированного Commit (op:29) и Welcome (op:30):
    let (mut sender_engine, _) = DaveMlsEngine::init_on_op4(
        self_user_id,
        channel_id,
        1,
        &secret_key,
        EncryptionMode::Aes256GcmRtpSize,
    )
    .expect("init sender");
    sender_engine
        .on_clients_connect_op11(&[self_user_id, peer_user_id])
        .expect("connect peers");
    sender_engine.on_external_sender_op25(&op25).expect("op25");
    sender_engine
        .on_proposals_op27(&valid_add_prop)
        .expect("sender proposal");

    let valid_commit = build_structured_mls_commit(
        11,
        15,
        channel_id,
        1,
        &ext_sender_bytes,
        &[valid_add_prop[4..].to_vec()],
        &[self_user_id, peer_user_id],
    );
    match sender_engine
        .on_commit_or_welcome_op29_op30(&valid_commit)
        .expect("valid structured commit")
    {
        DaveTransitionOutcome::SendTransitionReadyOp23 {
            transition_id,
            op23_json,
        } => {
            assert_eq!(transition_id, 15);
            assert!(op23_json.contains("\"transition_id\":15"));
        }
        other => panic!("expected SendTransitionReadyOp23, got {other:?}"),
    }
    sender_engine
        .on_prepare_transition_op21(15, 1)
        .expect("prepare 15");
    assert_eq!(
        sender_engine
            .on_execute_transition_op22(15)
            .expect("execute 15"),
        1
    );

    // Второй участник присоединяется через структурированный Welcome (op:30) с transition_id = 0 (мгновенная активация)
    let (mut joiner_engine, _) = DaveMlsEngine::init_on_op4(
        peer_user_id,
        channel_id,
        1,
        &secret_key,
        EncryptionMode::Aes256GcmRtpSize,
    )
    .expect("init joiner");
    joiner_engine
        .on_clients_connect_op11(&[self_user_id, peer_user_id])
        .expect("connect joiner peers");
    joiner_engine.on_external_sender_op25(&op25).expect("op25");

    // Проверим негативный Welcome без external_sender расширения (ext_sender_count = 0)
    let bad_welcome_no_ext = build_structured_mls_welcome(
        12,
        0,
        channel_id,
        1,
        &ext_sender_bytes,
        0,
        &[valid_add_prop[4..].to_vec()],
        &[self_user_id, peer_user_id],
    );
    assert!(matches!(
        joiner_engine
            .on_commit_or_welcome_op29_op30(&bad_welcome_no_ext)
            .expect("welcome with 0 ext senders"),
        DaveTransitionOutcome::InvalidCommitWelcome { .. }
    ));
    assert_eq!(
        joiner_engine.last_verification_error(),
        Some(&MlsVerificationError::UnexpectedExternalSenderCount(0))
    );

    // Валидный Welcome (op:30)
    let valid_welcome = build_structured_mls_welcome(
        13,
        0,
        channel_id,
        1,
        &ext_sender_bytes,
        1,
        &[valid_add_prop[4..].to_vec()],
        &[self_user_id, peer_user_id],
    );
    assert_eq!(
        joiner_engine
            .on_commit_or_welcome_op29_op30(&valid_welcome)
            .expect("valid welcome"),
        DaveTransitionOutcome::ExecutedImmediate { epoch: 1 }
    );

    // Оба участника получили идентичный epoch_authenticator, 30-значный voice_privacy_code и совместимые MlsKeyRatchet!
    let auth_sender = sender_engine
        .epoch_authenticator()
        .expect("sender epoch_authenticator");
    let auth_joiner = joiner_engine
        .epoch_authenticator()
        .expect("joiner epoch_authenticator");
    assert_eq!(auth_sender, auth_joiner);
    assert_ne!(auth_sender, [0u8; 32]);

    let code_sender = sender_engine
        .voice_privacy_code()
        .expect("sender privacy code");
    let code_joiner = joiner_engine
        .voice_privacy_code()
        .expect("joiner privacy code");
    assert_eq!(code_sender, code_joiner);
    let digits_only: String = code_sender.chars().filter(|c| c.is_ascii_digit()).collect();
    assert_eq!(digits_only.len(), 30);

    // Сквозная проверка пользовательских рэтчетов между sender_engine и joiner_engine
    let mut r_sender_1001 = sender_engine
        .user_key_ratchet(self_user_id)
        .expect("ratchet for 1001 on sender");
    let mut r_joiner_1001 = joiner_engine
        .user_key_ratchet(self_user_id)
        .expect("ratchet for 1001 on joiner");
    assert_eq!(
        r_sender_1001.get_key(0).expect("gen 0"),
        r_joiner_1001.get_key(0).expect("gen 0")
    );
    assert_eq!(
        r_sender_1001.get_key(4).expect("gen 4"),
        r_joiner_1001.get_key(4).expect("gen 4")
    );

    // Нераспознанный пользователь не может получить рэтчет
    assert!(sender_engine.user_key_ratchet(intruder_user_id).is_err());
}
