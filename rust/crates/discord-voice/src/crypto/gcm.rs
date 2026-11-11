//! AES-GCM (NIST SP 800-38D) и `Aes128GcmTruncated8` для DAVE MLS v1 — карточки P4-03, TASK-903.
//!
//! Реализация без внешних зависимостей: режим CTR + потоковый GHASH над GF(2^128)
//! без кучи-аллокаций. Проверяется KAT-векторами NIST SP 800-38D (test cases 4 и 13–15) —
//! см. `tests/crypto_vectors.rs` и `tests/fuzz_protocol_dave_contract.rs`.

use super::aes::{Aes128, Aes128Key, Aes256, Aes256Key};
use super::{
    CryptoError, GCM_MAX_MESSAGE_LEN, GCM_NONCE_SIZE, GCM_TAG_SIZE, GCM_TRUNCATED8_TAG_SIZE,
    check_message_len,
};
use std::fmt;
use std::hint::black_box;

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

/// Шифр AEAD AES-128-GCM с 96-битным nonce и 64-битным (8-байтным) усечённым тегом DAVE MLS v1.
///
/// Не выделяет динамическую память при работе in-place (`encrypt_in_place_detached` /
/// `decrypt_in_place_detached`) и затирает хеш-подключ `H` при `Drop`.
#[derive(Clone)]
pub struct Aes128GcmTruncated8 {
    cipher: Aes128,
    /// Хеш-подключ `H = E_K(0^128)`.
    h: u128,
}

impl fmt::Debug for Aes128GcmTruncated8 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Aes128GcmTruncated8(<скрыто>)")
    }
}

impl Drop for Aes128GcmTruncated8 {
    fn drop(&mut self) {
        self.h = black_box(0);
    }
}

impl Aes128GcmTruncated8 {
    /// Готовит шифр по 16-байтному ключу AES-128; при иной длине возвращает `InvalidKeyLength`.
    pub fn new(key: &[u8]) -> Result<Self, CryptoError> {
        let aes_key = Aes128Key::from_bytes(key)?;
        let cipher = Aes128::new(&aes_key);
        let h = u128::from_be_bytes(cipher.encrypt_block(&[0u8; 16]));
        Ok(Self { cipher, h })
    }

    /// Проверяет длину сообщения **до** обработки или аллокации буферов.
    pub fn check_message_len(len: usize) -> Result<(), CryptoError> {
        check_message_len(len, GCM_BLOCK_SIZE, GCM_MAX_MESSAGE_LEN)
    }

    /// Шифрует буфер на месте и возвращает 8-байтный усечённый тег `MSB_64(T)` (DAVE v1).
    pub fn encrypt_in_place_detached(
        &self,
        nonce: &[u8; GCM_NONCE_SIZE],
        aad: &[u8],
        buffer: &mut [u8],
    ) -> Result<[u8; GCM_TRUNCATED8_TAG_SIZE], CryptoError> {
        Self::check_message_len(buffer.len())?;
        let j0 = j0_from_nonce(nonce);
        gctr128_in_place(&self.cipher, j0, buffer);
        Ok(self.tag8(&j0, aad, buffer))
    }

    /// Проверяет 8-байтный тег в константное время **до** изменения буфера и расшифровывает на месте.
    pub fn decrypt_in_place_detached(
        &self,
        nonce: &[u8; GCM_NONCE_SIZE],
        aad: &[u8],
        buffer: &mut [u8],
        tag: &[u8; GCM_TRUNCATED8_TAG_SIZE],
    ) -> Result<(), CryptoError> {
        Self::check_message_len(buffer.len())?;
        let j0 = j0_from_nonce(nonce);
        let expected = self.tag8(&j0, aad, buffer);
        if !constant_time_eq(&expected, tag) {
            return Err(CryptoError::AuthenticationFailed);
        }
        gctr128_in_place(&self.cipher, j0, buffer);
        Ok(())
    }

    /// Шифрует открытый текст и возвращает `ciphertext || tag8` (`len + 8` байт).
    pub fn encrypt(
        &self,
        nonce: &[u8; GCM_NONCE_SIZE],
        aad: &[u8],
        plaintext: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        Self::check_message_len(plaintext.len())?;
        let mut out = Vec::with_capacity(plaintext.len() + GCM_TRUNCATED8_TAG_SIZE);
        out.extend_from_slice(plaintext);
        let tag8 = self.encrypt_in_place_detached(nonce, aad, &mut out)?;
        out.extend_from_slice(&tag8);
        Ok(out)
    }

    /// Расшифровывает `ciphertext || tag8`; усечённые входы (`< 8` Б) дают `InvalidTagLength`,
    /// несовпадение тега — `AuthenticationFailed` без выделения буфера открытого текста.
    pub fn decrypt(
        &self,
        nonce: &[u8; GCM_NONCE_SIZE],
        aad: &[u8],
        ciphertext_and_tag: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        if ciphertext_and_tag.len() < GCM_TRUNCATED8_TAG_SIZE {
            return Err(CryptoError::InvalidTagLength {
                expected: GCM_TRUNCATED8_TAG_SIZE,
                actual: ciphertext_and_tag.len(),
            });
        }
        let split = ciphertext_and_tag.len() - GCM_TRUNCATED8_TAG_SIZE;
        Self::check_message_len(split)?;
        let (ciphertext, tag) = ciphertext_and_tag.split_at(split);
        let j0 = j0_from_nonce(nonce);
        let expected = self.tag8(&j0, aad, ciphertext);
        if !constant_time_eq(&expected, tag) {
            return Err(CryptoError::AuthenticationFailed);
        }
        let mut out = ciphertext.to_vec();
        gctr128_in_place(&self.cipher, j0, &mut out);
        Ok(out)
    }

    /// Усечённый 64-битный тег: `MSB_64(GCTR(K, J0, GHASH(H, A, C)))` (NIST SP 800-38D §6.2, §7.1).
    fn tag8(&self, j0: &u128, aad: &[u8], ciphertext: &[u8]) -> [u8; GCM_TRUNCATED8_TAG_SIZE] {
        let s = ghash(self.h, aad, ciphertext).to_be_bytes();
        let mask = self.cipher.encrypt_block(&j0.to_be_bytes());
        let mut tag = [0u8; GCM_TRUNCATED8_TAG_SIZE];
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

/// Режим CTR для AES-128 на месте без аллокаций.
fn gctr128_in_place(cipher: &Aes128, icb: u128, data: &mut [u8]) {
    let mut counter = icb;
    for chunk in data.chunks_mut(16) {
        counter = inc32(counter);
        let stream = cipher.encrypt_block(&counter.to_be_bytes());
        for (dst, &ks) in chunk.iter_mut().zip(stream.iter()) {
            *dst ^= ks;
        }
    }
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

/// Потоковый GHASH над AAD и шифротекстом без кучи-аллокаций (NIST SP 800-38D §6.3).
fn ghash(h: u128, aad: &[u8], ciphertext: &[u8]) -> u128 {
    let mut y: u128 = 0;
    for chunk in aad.chunks(16) {
        let mut block = [0u8; 16];
        block[..chunk.len()].copy_from_slice(chunk);
        y = gf_mul128(y ^ u128::from_be_bytes(block), h);
    }
    for chunk in ciphertext.chunks(16) {
        let mut block = [0u8; 16];
        block[..chunk.len()].copy_from_slice(chunk);
        y = gf_mul128(y ^ u128::from_be_bytes(block), h);
    }
    let mut len_block = [0u8; 16];
    len_block[..8].copy_from_slice(&((aad.len() as u64) * 8).to_be_bytes());
    len_block[8..16].copy_from_slice(&((ciphertext.len() as u64) * 8).to_be_bytes());
    gf_mul128(y ^ u128::from_be_bytes(len_block), h)
}

#[inline(always)]
const fn gf_mul128_bit(z: u128, v: u128, x: u128, shift: u32) -> (u128, u128) {
    let bit = (x >> shift) & 1;
    let mask = 0u128.wrapping_sub(bit);
    let next_z = z ^ (v & mask);
    let lsb = v & 1;
    let r_mask = 0u128.wrapping_sub(lsb);
    let next_v = (v >> 1) ^ ((0xe1u128 << 120) & r_mask);
    (next_z, next_v)
}

#[inline(always)]
const fn gf_mul128_octet(z: u128, v: u128, x: u128, top_shift: u32) -> (u128, u128) {
    let (z, v) = gf_mul128_bit(z, v, x, top_shift);
    let (z, v) = gf_mul128_bit(z, v, x, top_shift - 1);
    let (z, v) = gf_mul128_bit(z, v, x, top_shift - 2);
    let (z, v) = gf_mul128_bit(z, v, x, top_shift - 3);
    let (z, v) = gf_mul128_bit(z, v, x, top_shift - 4);
    let (z, v) = gf_mul128_bit(z, v, x, top_shift - 5);
    let (z, v) = gf_mul128_bit(z, v, x, top_shift - 6);
    gf_mul128_bit(z, v, x, top_shift - 7)
}

/// Константно-временное умножение в GF(2^128) по NIST SP 800-38D (алгоритм 1) без циклов и ветвлений.
fn gf_mul128(x: u128, y: u128) -> u128 {
    let (z, v) = gf_mul128_octet(0, y, x, 127);
    let (z, v) = gf_mul128_octet(z, v, x, 119);
    let (z, v) = gf_mul128_octet(z, v, x, 111);
    let (z, v) = gf_mul128_octet(z, v, x, 103);
    let (z, v) = gf_mul128_octet(z, v, x, 95);
    let (z, v) = gf_mul128_octet(z, v, x, 87);
    let (z, v) = gf_mul128_octet(z, v, x, 79);
    let (z, v) = gf_mul128_octet(z, v, x, 71);
    let (z, v) = gf_mul128_octet(z, v, x, 63);
    let (z, v) = gf_mul128_octet(z, v, x, 55);
    let (z, v) = gf_mul128_octet(z, v, x, 47);
    let (z, v) = gf_mul128_octet(z, v, x, 39);
    let (z, v) = gf_mul128_octet(z, v, x, 31);
    let (z, v) = gf_mul128_octet(z, v, x, 23);
    let (z, v) = gf_mul128_octet(z, v, x, 15);
    let (z, _) = gf_mul128_octet(z, v, x, 7);
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
