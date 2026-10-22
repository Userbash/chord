//! AES-GCM (NIST SP 800-38D) — карточка P4-03.
//!
//! Реализация без внешних зависимостей: режим CTR + GHASH над GF(2^128).
//! Проверяется KAT-векторами NIST SP 800-38D (test cases 13–15) — см.
//! `tests/crypto_vectors.rs`.

use super::aes::{Aes256, Aes256Key};
use super::{CryptoError, GCM_MAX_MESSAGE_LEN, GCM_NONCE_SIZE, GCM_TAG_SIZE, check_message_len};

/// Размер блока GCM (CTR) в байтах.
const GCM_BLOCK_SIZE: usize = 16;

/// Шифр AEAD AES-256-GCM с 96-битным nonce и 128-битным тегом.
pub struct Gcm {
    cipher: Aes256,
    /// Хеш-подключ `H = E_K(0^128)`.
    h: u128,
}

impl Gcm {
    /// Готовит шифр по ключу AES-256.
    pub fn new(key: &Aes256Key) -> Self {
        let cipher = Aes256::new(key);
        let h = u128::from_be_bytes(cipher.encrypt_block(&[0u8; 16]));
        Self { cipher, h }
    }

    /// Проверяет длину сообщения **до** аллокации буферов.
    ///
    /// Счётчик CTR 32-битный: длиннее `GCM_MAX_MESSAGE_LEN` байт keystream
    /// повторился бы под тем же ключом и nonce.
    pub fn check_message_len(len: usize) -> Result<(), CryptoError> {
        check_message_len(len, GCM_BLOCK_SIZE, GCM_MAX_MESSAGE_LEN)
    }

    /// Шифрует открытый текст; возвращает `ciphertext || tag` (длина = len + 16).
    pub fn encrypt(
        &self,
        nonce: &[u8; GCM_NONCE_SIZE],
        aad: &[u8],
        plaintext: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        Self::check_message_len(plaintext.len())?;
        let j0 = j0_from_nonce(nonce);
        let ciphertext = gctr(&self.cipher, j0, plaintext);
        let tag = self.tag(&j0, aad, &ciphertext);
        let mut out = Vec::with_capacity(ciphertext.len() + GCM_TAG_SIZE);
        out.extend_from_slice(&ciphertext);
        out.extend_from_slice(&tag);
        Ok(out)
    }

    /// Расшифровывает `ciphertext || tag`; при несовпадении тега — `AuthenticationFailed`.
    pub fn decrypt(
        &self,
        nonce: &[u8; GCM_NONCE_SIZE],
        aad: &[u8],
        ciphertext_and_tag: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        if ciphertext_and_tag.len() < GCM_TAG_SIZE {
            return Err(CryptoError::InvalidTagLength {
                expected: GCM_TAG_SIZE,
                actual: ciphertext_and_tag.len(),
            });
        }
        Self::check_message_len(ciphertext_and_tag.len() - GCM_TAG_SIZE)?;
        let split = ciphertext_and_tag.len() - GCM_TAG_SIZE;
        let (ciphertext, tag) = ciphertext_and_tag.split_at(split);
        let j0 = j0_from_nonce(nonce);
        let expected = self.tag(&j0, aad, ciphertext);
        if !constant_time_eq(&expected, tag) {
            return Err(CryptoError::AuthenticationFailed);
        }
        Ok(gctr(&self.cipher, j0, ciphertext))
    }

    /// Тег: `MSB_128(GCTR(K, J0, GHASH(...)))` (NIST SP 800-38D §6.2).
    fn tag(&self, j0: &u128, aad: &[u8], ciphertext: &[u8]) -> [u8; GCM_TAG_SIZE] {
        let s = ghash(self.h, aad, ciphertext).to_be_bytes();
        let mask = self.cipher.encrypt_block(&j0.to_be_bytes());
        let mut tag = [0u8; GCM_TAG_SIZE];
        for (i, byte) in tag.iter_mut().enumerate() {
            *byte = s[i] ^ mask[i];
        }
        tag
    }
}

/// Блок `J0 = nonce || 0^31 || 1` для 96-битного nonce (NIST SP 800-38D §7.1).
fn j0_from_nonce(nonce: &[u8; GCM_NONCE_SIZE]) -> u128 {
    let mut block = [0u8; 16];
    block[..GCM_NONCE_SIZE].copy_from_slice(nonce);
    block[15] = 1;
    u128::from_be_bytes(block)
}

/// Режим CTR: `Y_i = E_K(CB_i) XOR X_i`, `CB_1 = ICB + 1` (NIST SP 800-38D §6.2).
fn gctr(cipher: &Aes256, icb: u128, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut counter = icb;
    for chunk in data.chunks(16) {
        counter = inc32(counter);
        let stream = cipher.encrypt_block(&counter.to_be_bytes());
        for (i, byte) in chunk.iter().enumerate() {
            out.push(byte ^ stream[i]);
        }
    }
    out
}

/// Инкремент младших 32 бит 128-битного блока.
fn inc32(value: u128) -> u128 {
    let low = (value & 0xffff_ffff) as u32;
    (value & !0xffff_ffffu128) | u128::from(low.wrapping_add(1))
}

/// GHASH над AAD и шифротекстом (NIST SP 800-38D §6.3).
fn ghash(h: u128, aad: &[u8], ciphertext: &[u8]) -> u128 {
    let mut data = Vec::with_capacity(aad.len() + ciphertext.len() + 32);
    data.extend_from_slice(aad);
    data.resize(data.len() + pad16_len(aad.len()), 0);
    data.extend_from_slice(ciphertext);
    data.resize(data.len() + pad16_len(ciphertext.len()), 0);
    data.extend_from_slice(&((aad.len() as u64) * 8).to_be_bytes());
    data.extend_from_slice(&((ciphertext.len() as u64) * 8).to_be_bytes());

    let mut y: u128 = 0;
    for chunk in data.chunks(16) {
        let mut block = [0u8; 16];
        block[..chunk.len()].copy_from_slice(chunk);
        y = gf_mul128(y ^ u128::from_be_bytes(block), h);
    }
    y
}

/// Длина дополнения до кратности 16 байтам.
fn pad16_len(len: usize) -> usize {
    (16 - len % 16) % 16
}

/// Умножение в GF(2^128) по NIST SP 800-38D (алгоритм 1).
fn gf_mul128(x: u128, y: u128) -> u128 {
    let mut z: u128 = 0;
    let mut v = y;
    for i in 0..128 {
        if (x >> (127 - i)) & 1 == 1 {
            z ^= v;
        }
        if v & 1 == 1 {
            v = (v >> 1) ^ (0xe1u128 << 120);
        } else {
            v >>= 1;
        }
    }
    z
}

/// Сравнение без раннего выхода (защита от timing-атак по тегу).
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
