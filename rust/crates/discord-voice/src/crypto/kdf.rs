//! HKDF-SHA1 (RFC 5869) и label-KDF для сессионных ключей SRTP — карточка P4-03.
//!
//! SHA-1 и HMAC-SHA1 реализованы внутри модуля (внешних зависимостей нет).
//! HKDF-SHA1 проверяется KAT-векторами RFC 5869 (A.2, A.3) — см.
//! `tests/crypto_vectors.rs`.
//!
//! Label-KDF для SRTP — это HKDF-SHA1 от мастер-ключа и мастер-соли с меткой в
//! `info`. Это **не** label-KDF RFC 3711 §4.3.1 (тот текст в офлайн-режиме недоступен),
//! поэтому совместимость с RFC 3711 не заявляется.

use super::CryptoError;

/// Длина хеша SHA-1 в байтах.
const SHA1_LEN: usize = 20;
/// Максимальная длина вывода HKDF (`255 * HashLen`).
const HKDF_MAX_OUT: usize = 255 * SHA1_LEN;

/// HKDF-SHA1 (RFC 5869): `extract(salt, ikm)` + `expand(info, L)`.
///
/// `out_len == 0` либо `out_len > 255 * 20` ⇒ `InvalidOutputLength`.
pub fn hkdf_sha1(
    ikm: &[u8],
    salt: &[u8],
    info: &[u8],
    out_len: usize,
) -> Result<Vec<u8>, CryptoError> {
    if out_len == 0 || out_len > HKDF_MAX_OUT {
        return Err(CryptoError::InvalidOutputLength);
    }
    Ok(hkdf_expand(ikm, salt, info, out_len))
}

/// Label-KDF для SRTP: HKDF-SHA1 от мастер-ключа и мастер-соли, метка идёт в `info`.
///
/// Основа — RFC 5869; отличие от RFC 3711 §4.3.1 зафиксировано в документации модуля.
pub fn srtp_key_derive(
    master_key: &[u8],
    master_salt: &[u8],
    label: &[u8],
    out_len: usize,
) -> Result<Vec<u8>, CryptoError> {
    hkdf_sha1(master_key, master_salt, label, out_len)
}

/// HKDF-SHA1 с фиксированной длиной 32 Б (внутрикрейтовое использование).
pub(crate) fn hkdf_sha1_32(ikm: &[u8], salt: &[u8], info: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&hkdf_expand(ikm, salt, info, 32));
    out
}

/// HKDF-SHA1 с фиксированной длиной 16 Б (внутрикрейтовое использование).
pub(crate) fn hkdf_sha1_16(ikm: &[u8], salt: &[u8], info: &[u8]) -> [u8; 16] {
    let mut out = [0u8; 16];
    out.copy_from_slice(&hkdf_expand(ikm, salt, info, 16));
    out
}

/// Ядро HKDF-SHA1 без проверок длины (длина уже проверена вызывающим кодом).
fn hkdf_expand(ikm: &[u8], salt: &[u8], info: &[u8], out_len: usize) -> Vec<u8> {
    // Extract: пустая соль ⇒ строка нулей длины HashLen (RFC 5869 §2.2).
    let salt_material = if salt.is_empty() {
        vec![0u8; SHA1_LEN]
    } else {
        salt.to_vec()
    };
    let prk = hmac_sha1(&salt_material, ikm);

    // Expand.
    let mut okm = Vec::with_capacity(out_len);
    let mut t: Vec<u8> = Vec::new();
    let mut counter: u8 = 1;
    while okm.len() < out_len {
        let mut input = Vec::with_capacity(t.len() + info.len() + 1);
        input.extend_from_slice(&t);
        input.extend_from_slice(info);
        input.push(counter);
        t = hmac_sha1(&prk, &input).to_vec();
        let take = std::cmp::min(t.len(), out_len - okm.len());
        okm.extend_from_slice(&t[..take]);
        counter = counter.wrapping_add(1);
    }
    okm
}

/// HMAC-SHA1 (RFC 2104).
fn hmac_sha1(key: &[u8], message: &[u8]) -> [u8; SHA1_LEN] {
    let mut key_block = [0u8; 64];
    if key.len() > 64 {
        key_block[..SHA1_LEN].copy_from_slice(&sha1(key));
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }

    let mut inner_pad = [0x36u8; 64];
    let mut outer_pad = [0x5cu8; 64];
    for i in 0..64 {
        inner_pad[i] ^= key_block[i];
        outer_pad[i] ^= key_block[i];
    }

    let mut inner = Vec::with_capacity(64 + message.len());
    inner.extend_from_slice(&inner_pad);
    inner.extend_from_slice(message);
    let inner_hash = sha1(&inner);

    let mut outer = Vec::with_capacity(64 + SHA1_LEN);
    outer.extend_from_slice(&outer_pad);
    outer.extend_from_slice(&inner_hash);
    sha1(&outer)
}

/// SHA-1 (FIPS 180-4 §6.1).
fn sha1(message: &[u8]) -> [u8; SHA1_LEN] {
    let mut state: [u32; 5] = [
        0x6745_2301,
        0xefcd_ab89,
        0x98ba_dcfe,
        0x1032_5476,
        0xc3d2_e1f0,
    ];

    let bit_len = (message.len() as u64) * 8;
    let mut data = message.to_vec();
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bit_len.to_be_bytes());

    let mut offset = 0usize;
    while offset < data.len() {
        let block = &data[offset..offset + 64];
        let mut w = [0u32; 80];
        for (i, word) in w.iter_mut().enumerate().take(16) {
            *word = u32::from_be_bytes([
                block[4 * i],
                block[4 * i + 1],
                block[4 * i + 2],
                block[4 * i + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let (mut a, mut b, mut c, mut d, mut e) =
            (state[0], state[1], state[2], state[3], state[4]);
        for (i, word) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5a82_7999u32),
                20..=39 => (b ^ c ^ d, 0x6ed9_eba1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1b_bcdc),
                _ => (b ^ c ^ d, 0xca62_c1d6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        offset += 64;
    }

    let mut digest = [0u8; SHA1_LEN];
    for (i, word) in state.iter().enumerate() {
        digest[4 * i..4 * i + 4].copy_from_slice(&word.to_be_bytes());
    }
    digest
}
