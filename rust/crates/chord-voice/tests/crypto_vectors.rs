//! KAT-векторы крипто-примитивов карточки P4-03 (`discord-voice`).
//!
//! Оракул — опубликованные тест-векторы, вшитые в тест как константы:
//! * FIPS-197 (App. C.3) — AES-256;
//! * NIST SP 800-38D (GCM test cases 13–15) — AES-256-GCM;
//! * RFC 8439 §2.8.2 — ChaCha20-Poly1305 (AEAD);
//! * draft-irtf-cfrg-xchacha (A.2, A.3.1) — HChaCha20 и XChaCha20-Poly1305 (AEAD);
//! * RFC 5869 (A.2, A.3) — HKDF-SHA1.
//!
//! Значения сверены независимыми реализациями (OpenSSL/Node.js `crypto`, Python
//! `cryptography`), а не кодом крейта: тест не «зеленится» под реализацию.

use chord_voice::crypto::aes::{self, Aes256, Aes256Key};
use chord_voice::crypto::chacha::{ChaCha20Poly1305, XChaCha20Poly1305};
use chord_voice::crypto::gcm::Gcm;
use chord_voice::crypto::kdf::hkdf_sha1;
use chord_voice::crypto::{
    AES256_KEY_SIZE, CHACHA_MAX_MESSAGE_LEN, CryptoError, GCM_MAX_MESSAGE_LEN, GCM_NONCE_SIZE,
};

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

/// Ключ AES-256 из FIPS-197 App. C.3.
const FIPS197_KEY: &str = "603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4";
/// Открытый блок FIPS-197 App. C.3.
const FIPS197_PT: &str = "6bc1bee22e409f96e93d7e117393172a";
/// Шифротекст FIPS-197 App. C.3.
const FIPS197_CT: &str = "f3eed1bdb5d2a03c064b5a7e3db181f8";

/// Таблица S-box FIPS-197 (Fig. 7) — эталон для проверки алгебраического вычисления.
///
/// Источник: опубликованная таблица FIPS-197, перепроверенная по независимой
/// реализации (aws-lc / BoringSSL, `aes_ref_sub_byte`) и по KAT FIPS-197 App. C.3.
const FIPS197_SBOX: [u8; 256] = [
    0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab, 0x76,
    0xca, 0x82, 0xc9, 0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4, 0x72, 0xc0,
    0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f, 0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71, 0xd8, 0x31, 0x15,
    0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07, 0x12, 0x80, 0xe2, 0xeb, 0x27, 0xb2, 0x75,
    0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6, 0xb3, 0x29, 0xe3, 0x2f, 0x84,
    0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb, 0xbe, 0x39, 0x4a, 0x4c, 0x58, 0xcf,
    0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45, 0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8,
    0x51, 0xa3, 0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5, 0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2,
    0xcd, 0x0c, 0x13, 0xec, 0x5f, 0x97, 0x44, 0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73,
    0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a, 0x90, 0x88, 0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb,
    0xe0, 0x32, 0x3a, 0x0a, 0x49, 0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac, 0x62, 0x91, 0x95, 0xe4, 0x79,
    0xe7, 0xc8, 0x37, 0x6d, 0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a, 0xae, 0x08,
    0xba, 0x78, 0x25, 0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a,
    0x70, 0x3e, 0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e,
    0xe1, 0xf8, 0x98, 0x11, 0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf,
    0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42, 0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb, 0x16,
];

/// NIST SP 800-38D, GCM test case 13: K = 0^256, IV = 0^96, пустые AAD и P.
const GCM_TC13_TAG: &str = "530f8afbc74536b9a963b4f1c4cb738b";
/// NIST SP 800-38D, GCM test case 14: K = 0^256, IV = 0^96, P = 0^128, A = пусто.
const GCM_TC14_CT: &str = "cea7403d4d606b6e074ec5d3baf39d18";
const GCM_TC14_TAG: &str = "d0d1c8a799996bf0265b98b5d48ab919";
/// NIST SP 800-38D, GCM test case 15: K/IV/AAD/P — по спецификации.
const GCM_TC15_KEY: &str = "feffe9928665731c6d6a8f9467308308feffe9928665731c6d6a8f9467308308";
const GCM_TC15_IV: &str = "cafebabefacedbaddecaf888";
const GCM_TC15_AAD: &str = "feedfacedeadbeeffeedfacedeadbeefabaddad2";
const GCM_TC15_PT: &str = "d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a721\
                          c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b39";
const GCM_TC15_CT: &str = "522dc1f099567d07f47f37a32a84427d643a8cdcbfe5c0c97598a2bd2555d1aa\
                          8cb08e48590dbb3da7b08b1056828838c5f61e6393ba7a0abcc9f662";
const GCM_TC15_TAG: &str = "76fc6ece0f4e1768cddf8853bb2d551b";

/// RFC 8439 §2.8.2: ChaCha20-Poly1305 AEAD.
const RFC8439_KEY: &str = "808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f";
const RFC8439_NONCE: &str = "070000004041424344454647";
const RFC8439_AAD: &str = "50515253c0c1c2c3c4c5c6c7";
const RFC8439_PT: &str = "4c616469657320616e642047656e746c656d656e206f662074686520636c6173\
                          73206f66202739393a204966204920636f756c64206f6666657220796f75206f\
                          6e6c79206f6e652074697020666f7220746865206675747572652c2073756e73\
                          637265656e20776f756c642062652069742e";
const RFC8439_CT: &str = "d31a8d34648e60db7b86afbc53ef7ec2a4aded51296e08fea9e2b5a736ee62d6\
                          3dbea45e8ca9671282fafb69da92728b1a71de0a9e060b2905d6a5b67ecd3b369\
                          2ddbd7f2d778b8c9803aee328091b58fab324e4fad675945585808b4831d7bc3ff\
                          4def08e4b7a9de576d26586cec64b6116";
const RFC8439_TAG: &str = "1ae10b594f09e26a7e902ecbd0600691";

/// draft-irtf-cfrg-xchacha A.3.1: XChaCha20-Poly1305 AEAD (nonce 24 Б).
const XCHACHA_KEY: &str = "808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f";
const XCHACHA_NONCE: &str = "404142434445464748494a4b4c4d4e4f5051525354555657";
const XCHACHA_AAD: &str = "50515253c0c1c2c3c4c5c6c7";
const XCHACHA_CT: &str = "bd6d179d3e83d43b9576579493c0e939572a1700252bfaccbed2902c21396cbb\
                          731c7f1b0b4aa6440bf3a82f4eda7e39ae64c6708c54c216cb96b72e1213b4522f8\
                          c9ba40db5d945b11b69b982c1bb9e3f3fac2bc369488f76b2383565d3fff921f9\
                          664c97637da9768812f615c68b13b52e";
const XCHACHA_TAG: &str = "c0875924c1c7987947deafd8780acf49";

/// RFC 5869 A.2: базовый тест HKDF-SHA1.
const HKDF_IKM: &str = "0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b";
const HKDF_SALT: &str = "000102030405060708090a0b0c";
const HKDF_INFO: &str = "f0f1f2f3f4f5f6f7f8f9";
const HKDF_OKM_42: &str = "d6000ffb5b50bd3970b260017798fb9c8df9ce2e2c16b6cd709cca07dc3cf9cf\
                          26d6c6d750d0aaf5ac94";
/// RFC 5869 A.3: нулевая соль и пустой info (вариант SHA-1).
const HKDF_OKM_42_ZERO_SALT: &str =
    "0ac1af7002b3d761d1e55298da9d0506b9ae52057220a306e07b6b87e8df21d0ea00033de03984d34918";

/// FIPS-197 App. C.3: шифрование одного блока AES-256.
#[test]
fn aes256_k() {
    let key = Aes256Key::from_bytes(&hex(FIPS197_KEY)).expect("ключ 32 Б");
    let cipher = Aes256::new(&key);
    let pt: [u8; 16] = hex(FIPS197_PT).try_into().expect("блок 16 Б");
    let ct = cipher.encrypt_block(&pt);
    assert_eq!(ct.to_vec(), hex(FIPS197_CT));
}

/// FIPS-197 App. C.3: обратное преобразование воспроизводит исходный блок.
#[test]
fn aes256_decrypt_k() {
    let key = Aes256Key::from_bytes(&hex(FIPS197_KEY)).expect("ключ 32 Б");
    let cipher = Aes256::new(&key);
    let ct: [u8; 16] = hex(FIPS197_CT).try_into().expect("блок 16 Б");
    let pt = cipher.decrypt_block(&ct);
    assert_eq!(pt.to_vec(), hex(FIPS197_PT));
}

/// Ключ AES-256 не принимает длину 31/33 Б.
#[test]
fn aes256_wrong_key_len() {
    assert_eq!(
        Aes256Key::from_bytes(&[0u8; 31]),
        Err(CryptoError::InvalidKeyLength {
            expected: AES256_KEY_SIZE,
            actual: 31,
        })
    );
    assert_eq!(
        Aes256Key::from_bytes(&[0u8; 33]),
        Err(CryptoError::InvalidKeyLength {
            expected: AES256_KEY_SIZE,
            actual: 33,
        })
    );
    assert!(Aes256Key::from_bytes(&[0u8; AES256_KEY_SIZE]).is_ok());
}

/// `Debug` ключа не раскрывает байты ключевого материала.
#[test]
fn aes256_key_debug_redacted_k() {
    let key = Aes256Key::from_bytes(&[0xABu8; AES256_KEY_SIZE]).expect("ключ 32 Б");
    let dumped = format!("{key:?}");
    assert!(
        !dumped.contains("ab"),
        "Debug ключа не должен печатать байты: {dumped}"
    );
    assert!(
        !dumped.contains("171"),
        "Debug ключа не должен печатать байты: {dumped}"
    );
}

/// NIST SP 800-38D, GCM test case 14: AES-256-GCM, K = 0^256, IV = 0^96.
#[test]
fn gcm_k() {
    let key = Aes256Key::from_bytes(&[0u8; AES256_KEY_SIZE]).expect("ключ 32 Б");
    let gcm = Gcm::new(&key);
    let nonce = [0u8; GCM_NONCE_SIZE];
    let out = gcm.encrypt(&nonce, &[], &[0u8; 16]).expect("шифрование");
    assert_eq!(out.len(), 16 + 16, "шифротекст 16 Б + тег 16 Б");
    assert_eq!(&out[..16], &hex(GCM_TC14_CT)[..]);
    assert_eq!(&out[16..], &hex(GCM_TC14_TAG)[..]);
    let back = gcm.decrypt(&nonce, &[], &out).expect("расшифрование");
    assert_eq!(back, vec![0u8; 16]);
}

/// NIST SP 800-38D, GCM test case 15: AAD привязан к шифротексту.
#[test]
fn gcm_aad_k() {
    let key = Aes256Key::from_bytes(&hex(GCM_TC15_KEY)).expect("ключ 32 Б");
    let gcm = Gcm::new(&key);
    let nonce: [u8; GCM_NONCE_SIZE] = hex(GCM_TC15_IV).try_into().expect("nonce 12 Б");
    let aad = hex(GCM_TC15_AAD);
    let pt = hex(GCM_TC15_PT);
    let out = gcm.encrypt(&nonce, &aad, &pt).expect("шифрование");
    assert_eq!(&out[..out.len() - 16], &hex(GCM_TC15_CT)[..]);
    assert_eq!(&out[out.len() - 16..], &hex(GCM_TC15_TAG)[..]);
    let back = gcm.decrypt(&nonce, &aad, &out).expect("расшифрование");
    assert_eq!(back, pt);
    // Изменение AAD ломает аутентификацию.
    let mut bad_aad = aad.clone();
    bad_aad[0] ^= 0x01;
    assert_eq!(
        gcm.decrypt(&nonce, &bad_aad, &out),
        Err(CryptoError::AuthenticationFailed)
    );
}

/// NIST SP 800-38D, GCM test case 13: пустой plaintext ⇒ на выходе только тег.
#[test]
fn gcm_empty_plaintext_k() {
    let key = Aes256Key::from_bytes(&[0u8; AES256_KEY_SIZE]).expect("ключ 32 Б");
    let gcm = Gcm::new(&key);
    let nonce = [0u8; GCM_NONCE_SIZE];
    let out = gcm.encrypt(&nonce, &[], &[]).expect("шифрование");
    assert_eq!(out.len(), 16, "пустой plaintext ⇒ только тег 16 Б");
    assert_eq!(out, hex(GCM_TC13_TAG));
    assert_eq!(
        gcm.decrypt(&nonce, &[], &out).expect("расшифрование"),
        Vec::new()
    );
}

/// Правка байта шифротекста ⇒ `AuthenticationFailed`.
#[test]
fn gcm_tamper_k() {
    let key = Aes256Key::from_bytes(&hex(GCM_TC15_KEY)).expect("ключ 32 Б");
    let gcm = Gcm::new(&key);
    let nonce: [u8; GCM_NONCE_SIZE] = hex(GCM_TC15_IV).try_into().expect("nonce 12 Б");
    let aad = hex(GCM_TC15_AAD);
    let mut out = gcm
        .encrypt(&nonce, &aad, &hex(GCM_TC15_PT))
        .expect("шифрование");
    out[0] ^= 0x01;
    assert_eq!(
        gcm.decrypt(&nonce, &aad, &out),
        Err(CryptoError::AuthenticationFailed)
    );
    // Правка тега — тоже.
    let mut out = gcm
        .encrypt(&nonce, &aad, &hex(GCM_TC15_PT))
        .expect("шифрование");
    let last = out.len() - 1;
    out[last] ^= 0x80;
    assert_eq!(
        gcm.decrypt(&nonce, &aad, &out),
        Err(CryptoError::AuthenticationFailed)
    );
}

/// Вход короче тега (16 Б) ⇒ `InvalidTagLength`.
#[test]
fn gcm_short_input_k() {
    let key = Aes256Key::from_bytes(&[0u8; AES256_KEY_SIZE]).expect("ключ 32 Б");
    let gcm = Gcm::new(&key);
    let nonce = [0u8; GCM_NONCE_SIZE];
    for len in [0usize, 1, 15] {
        assert_eq!(
            gcm.decrypt(&nonce, &[], &vec![0u8; len]),
            Err(CryptoError::InvalidTagLength {
                expected: 16,
                actual: len,
            })
        );
    }
}

/// RFC 8439 §2.8.2: ChaCha20-Poly1305 AEAD, полный вектор.
#[test]
fn chacha20poly1305_k() {
    let key: [u8; 32] = hex(RFC8439_KEY).try_into().expect("ключ 32 Б");
    let nonce: [u8; 12] = hex(RFC8439_NONCE).try_into().expect("nonce 12 Б");
    let aad = hex(RFC8439_AAD);
    let pt = hex(RFC8439_PT);
    let cipher = ChaCha20Poly1305::new(&key);
    let out = cipher.encrypt(&nonce, &aad, &pt).expect("шифрование");
    assert_eq!(out.len(), pt.len() + 16);
    assert_eq!(&out[..pt.len()], &hex(RFC8439_CT)[..]);
    assert_eq!(&out[pt.len()..], &hex(RFC8439_TAG)[..]);
    let back = cipher.decrypt(&nonce, &aad, &out).expect("расшифрование");
    assert_eq!(back, pt);
}

/// draft-irtf-cfrg-xchacha A.3.1: XChaCha20-Poly1305 AEAD, nonce 24 Б.
#[test]
fn xchacha20poly1305_k() {
    let key: [u8; 32] = hex(XCHACHA_KEY).try_into().expect("ключ 32 Б");
    let nonce: [u8; 24] = hex(XCHACHA_NONCE).try_into().expect("nonce 24 Б");
    let aad = hex(XCHACHA_AAD);
    let pt = hex(RFC8439_PT);
    let cipher = XChaCha20Poly1305::new(&key);
    let out = cipher.encrypt(&nonce, &aad, &pt).expect("шифрование");
    assert_eq!(out.len(), pt.len() + 16);
    assert_eq!(&out[..pt.len()], &hex(XCHACHA_CT)[..]);
    assert_eq!(&out[pt.len()..], &hex(XCHACHA_TAG)[..]);
    let back = cipher.decrypt(&nonce, &aad, &out).expect("расшифрование");
    assert_eq!(back, pt);
}

/// RFC 5869 A.2: базовый тест HKDF-SHA1 (L = 42).
#[test]
fn hkdf_sha1_k() {
    let okm = hkdf_sha1(&hex(HKDF_IKM), &hex(HKDF_SALT), &hex(HKDF_INFO), 42).expect("HKDF");
    assert_eq!(okm, hex(HKDF_OKM_42));
}

/// RFC 5869 A.3: нулевая соль и пустой info (вариант SHA-1).
#[test]
fn hkdf_sha1_zero_salt_k() {
    let okm = hkdf_sha1(&hex(HKDF_IKM), &[], &[], 42).expect("HKDF");
    assert_eq!(okm, hex(HKDF_OKM_42_ZERO_SALT));
}

/// Границы вывода HKDF-SHA1: L = 0 и L > 255*20 ⇒ `InvalidOutputLength`.
#[test]
fn hkdf_sha1_limits_k() {
    assert_eq!(
        hkdf_sha1(&hex(HKDF_IKM), &hex(HKDF_SALT), &hex(HKDF_INFO), 0),
        Err(CryptoError::InvalidOutputLength)
    );
    assert_eq!(
        hkdf_sha1(
            &hex(HKDF_IKM),
            &hex(HKDF_SALT),
            &hex(HKDF_INFO),
            255 * 20 + 1
        ),
        Err(CryptoError::InvalidOutputLength)
    );
    let max = hkdf_sha1(&hex(HKDF_IKM), &hex(HKDF_SALT), &hex(HKDF_INFO), 255 * 20)
        .expect("максимальная длина");
    assert_eq!(max.len(), 255 * 20);
}

/// S-box FIPS-197 совпадает с опубликованной таблицей для всех 256 значений,
/// обратный S-box — обратная перестановка.
#[test]
fn sbox_matches_fips197_k() {
    // Обратная перестановка, построенная из эталонной таблицы.
    let mut inverse = [0u8; 256];
    for (x, y) in FIPS197_SBOX.iter().enumerate() {
        inverse[*y as usize] = x as u8;
    }
    for value in 0u16..256 {
        let x = value as u8;
        assert_eq!(
            aes::sbox(x),
            FIPS197_SBOX[value as usize],
            "S-box для {x:#04x}"
        );
        assert_eq!(
            aes::inv_sbox(FIPS197_SBOX[value as usize]),
            x,
            "обратный S-box для {x:#04x}"
        );
        assert_eq!(
            aes::inv_sbox(x),
            inverse[value as usize],
            "обратный S-box для {x:#04x}"
        );
        assert_eq!(
            aes::inv_sbox(aes::sbox(x)),
            x,
            "inv(sbox(x)) == x для {x:#04x}"
        );
    }
}

/// AEAD-примитивы отклоняют сообщение длиннее предела 32-битного счётчика:
/// иначе keystream повторяется под тем же ключом и nonce.
#[test]
fn aead_rejects_overlong_message_k() {
    // Граница проверяется по длине, до аллокации: можно передать usize::MAX.
    assert_eq!(
        Gcm::check_message_len(usize::MAX),
        Err(CryptoError::MessageTooLong {
            limit: GCM_MAX_MESSAGE_LEN,
        })
    );
    assert_eq!(
        Gcm::check_message_len(GCM_MAX_MESSAGE_LEN + 1),
        Err(CryptoError::MessageTooLong {
            limit: GCM_MAX_MESSAGE_LEN,
        })
    );
    assert!(Gcm::check_message_len(1024).is_ok());
    assert!(Gcm::check_message_len(GCM_MAX_MESSAGE_LEN).is_ok());

    assert_eq!(
        ChaCha20Poly1305::check_message_len(usize::MAX),
        Err(CryptoError::MessageTooLong {
            limit: CHACHA_MAX_MESSAGE_LEN,
        })
    );
    assert_eq!(
        ChaCha20Poly1305::check_message_len(CHACHA_MAX_MESSAGE_LEN + 1),
        Err(CryptoError::MessageTooLong {
            limit: CHACHA_MAX_MESSAGE_LEN,
        })
    );
    assert!(ChaCha20Poly1305::check_message_len(1024).is_ok());
    assert!(ChaCha20Poly1305::check_message_len(CHACHA_MAX_MESSAGE_LEN).is_ok());

    assert_eq!(
        XChaCha20Poly1305::check_message_len(usize::MAX),
        Err(CryptoError::MessageTooLong {
            limit: CHACHA_MAX_MESSAGE_LEN,
        })
    );
    assert!(XChaCha20Poly1305::check_message_len(1024).is_ok());
}
