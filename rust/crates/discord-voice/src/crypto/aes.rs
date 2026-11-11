//! AES-128 и AES-256 в режиме одного блока (FIPS-197) — карточки P4-03, TASK-903.
//!
//! Реализация без внешних зависимостей: S-box и обратный S-box вычисляются
//! константными функциями (инверсия в GF(2^8) + аффинное преобразование FIPS-197),
//! поэтому таблицы не «перепечатаны» и проверяются KAT-векторами FIPS-197 App. C.1 и C.3
//! (см. `tests/crypto_vectors.rs` и `tests/fuzz_protocol_dave_contract.rs`).

use super::{AES_BLOCK_SIZE, AES128_KEY_SIZE, AES256_KEY_SIZE, CryptoError};
use std::fmt;
use std::hint::black_box;

/// Ключ AES-128: 16 байт (используется в DAVE MLS v1 media cipher).
///
/// `Debug` намеренно не показывает байты ключа; при `Drop` буфер затирается нулями.
#[derive(Clone, PartialEq, Eq)]
pub struct Aes128Key([u8; AES128_KEY_SIZE]);

impl fmt::Debug for Aes128Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Aes128Key(<скрыто>)")
    }
}

impl Drop for Aes128Key {
    fn drop(&mut self) {
        for byte in self.0.iter_mut() {
            *byte = black_box(0);
        }
    }
}

impl Aes128Key {
    /// Создаёт ключ из 16 байт; иная длина ⇒ `InvalidKeyLength`.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        if bytes.len() != AES128_KEY_SIZE {
            return Err(CryptoError::InvalidKeyLength {
                expected: AES128_KEY_SIZE,
                actual: bytes.len(),
            });
        }
        let mut key = [0u8; AES128_KEY_SIZE];
        key.copy_from_slice(bytes);
        Ok(Self(key))
    }

    /// Байты ключа.
    pub fn as_bytes(&self) -> &[u8; AES128_KEY_SIZE] {
        &self.0
    }

    /// Ключ из массива фиксированного размера (внутрикрейтовое использование).
    #[allow(dead_code)]
    pub(crate) fn from_array(bytes: [u8; AES128_KEY_SIZE]) -> Self {
        Self(bytes)
    }
}

/// Расширенный ключ AES-128 (11 раундовых ключей по 16 Б = 176 Б).
///
/// `Debug` не показывает раундовые ключи; при `Drop` раундовые ключи затираются нулями.
#[derive(Clone)]
pub struct Aes128 {
    round_keys: [u8; 176],
}

impl fmt::Debug for Aes128 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Aes128(<скрыто>)")
    }
}

impl Drop for Aes128 {
    fn drop(&mut self) {
        for byte in self.round_keys.iter_mut() {
            *byte = black_box(0);
        }
    }
}

impl Aes128 {
    /// Расширяет ключ AES-128 (Nk = 4, Nr = 10) и готовит шифр.
    pub fn new(key: &Aes128Key) -> Self {
        Self {
            round_keys: expand_key_128(key.as_bytes()),
        }
    }

    /// Шифрует один блок 16 Б (FIPS-197 §5.1).
    pub fn encrypt_block(&self, block: &[u8; AES_BLOCK_SIZE]) -> [u8; AES_BLOCK_SIZE] {
        let mut state = *block;
        add_round_key(&mut state, &self.round_keys, 0);
        for round in 1..10 {
            sub_bytes(&mut state);
            shift_rows(&mut state);
            mix_columns(&mut state);
            add_round_key(&mut state, &self.round_keys, round);
        }
        sub_bytes(&mut state);
        shift_rows(&mut state);
        add_round_key(&mut state, &self.round_keys, 10);
        state
    }

    /// Расшифровывает один блок 16 Б (FIPS-197 §5.3).
    pub fn decrypt_block(&self, block: &[u8; AES_BLOCK_SIZE]) -> [u8; AES_BLOCK_SIZE] {
        let mut state = *block;
        add_round_key(&mut state, &self.round_keys, 10);
        for round in (1..10).rev() {
            inv_shift_rows(&mut state);
            inv_sub_bytes(&mut state);
            add_round_key(&mut state, &self.round_keys, round);
            inv_mix_columns(&mut state);
        }
        inv_shift_rows(&mut state);
        inv_sub_bytes(&mut state);
        add_round_key(&mut state, &self.round_keys, 0);
        state
    }
}

/// Ключ AES-256: 32 байта.
///
/// `Debug` намеренно не показывает байты ключа.
#[derive(Clone, PartialEq, Eq)]
pub struct Aes256Key([u8; AES256_KEY_SIZE]);

impl fmt::Debug for Aes256Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Aes256Key(<скрыто>)")
    }
}

impl Aes256Key {
    /// Создаёт ключ из 32 байт; иная длина ⇒ `InvalidKeyLength`.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        if bytes.len() != AES256_KEY_SIZE {
            return Err(CryptoError::InvalidKeyLength {
                expected: AES256_KEY_SIZE,
                actual: bytes.len(),
            });
        }
        let mut key = [0u8; AES256_KEY_SIZE];
        key.copy_from_slice(bytes);
        Ok(Self(key))
    }

    /// Байты ключа.
    pub fn as_bytes(&self) -> &[u8; AES256_KEY_SIZE] {
        &self.0
    }

    /// Ключ из массива фиксированного размера (внутрикрейтовое использование:
    /// длина заведомо корректна, ошибка `InvalidKeyLength` тут невозможна).
    pub(crate) fn from_array(bytes: [u8; AES256_KEY_SIZE]) -> Self {
        Self(bytes)
    }
}

/// Расширенный ключ AES-256 (15 раундовых ключей по 16 Б = 240 Б).
///
/// `Debug` не показывает раундовые ключи.
pub struct Aes256 {
    round_keys: [u8; 240],
}

impl fmt::Debug for Aes256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Aes256(<скрыто>)")
    }
}

impl Aes256 {
    /// Расширяет ключ AES-256 (Nk = 8, Nr = 14) и готовит шифр.
    pub fn new(key: &Aes256Key) -> Self {
        Self {
            round_keys: expand_key(key.as_bytes()),
        }
    }

    /// Шифрует один блок 16 Б (FIPS-197 §5.1).
    pub fn encrypt_block(&self, block: &[u8; AES_BLOCK_SIZE]) -> [u8; AES_BLOCK_SIZE] {
        let mut state = *block;
        add_round_key(&mut state, &self.round_keys, 0);
        for round in 1..14 {
            sub_bytes(&mut state);
            shift_rows(&mut state);
            mix_columns(&mut state);
            add_round_key(&mut state, &self.round_keys, round);
        }
        sub_bytes(&mut state);
        shift_rows(&mut state);
        add_round_key(&mut state, &self.round_keys, 14);
        state
    }

    /// Расшифровывает один блок 16 Б (FIPS-197 §5.3).
    pub fn decrypt_block(&self, block: &[u8; AES_BLOCK_SIZE]) -> [u8; AES_BLOCK_SIZE] {
        let mut state = *block;
        add_round_key(&mut state, &self.round_keys, 14);
        for round in (1..14).rev() {
            inv_shift_rows(&mut state);
            inv_sub_bytes(&mut state);
            add_round_key(&mut state, &self.round_keys, round);
            inv_mix_columns(&mut state);
        }
        inv_shift_rows(&mut state);
        inv_sub_bytes(&mut state);
        add_round_key(&mut state, &self.round_keys, 0);
        state
    }
}

/// Один константно-временный шаг умножения в GF(2^8) без ветвлений и таблиц.
#[inline(always)]
const fn gf_mul_step(product: u8, a: u8, b: u8) -> (u8, u8, u8) {
    let mask = (b & 1).wrapping_neg();
    let next_product = product ^ (a & mask);
    // Старший бит сдвигаемого байта: 0x00 или 0xFF (маска от бита, не от 0x80).
    let high = (a >> 7).wrapping_neg();
    let next_a = (a << 1) ^ (0x1b & high);
    (next_product, next_a, b >> 1)
}

/// Умножение в GF(2^8) по многочлену FIPS-197 `x^8 + x^4 + x^3 + x + 1`.
///
/// «Русский крестьянин» без ветвлений по данным: выборка по младшему биту
/// делается маской, поэтому по времени выполнения не зависит от значения
/// операндов (важно: операнды могут быть секретными).
#[inline(always)]
const fn gf_mul(a: u8, b: u8) -> u8 {
    let (p, a, b) = gf_mul_step(0, a, b);
    let (p, a, b) = gf_mul_step(p, a, b);
    let (p, a, b) = gf_mul_step(p, a, b);
    let (p, a, b) = gf_mul_step(p, a, b);
    let (p, a, b) = gf_mul_step(p, a, b);
    let (p, a, b) = gf_mul_step(p, a, b);
    let (p, a, b) = gf_mul_step(p, a, b);
    let (p, _, _) = gf_mul_step(p, a, b);
    p
}

/// Обратный элемент в GF(2^8): `x^254` (0^254 = 0, отдельная ветка не нужна).
#[inline(always)]
const fn gf_inv(x: u8) -> u8 {
    let x2 = gf_mul(x, x);
    let x4 = gf_mul(x2, x2);
    let x8 = gf_mul(x4, x4);
    let x16 = gf_mul(x8, x8);
    let x32 = gf_mul(x16, x16);
    let x64 = gf_mul(x32, x32);
    let x128 = gf_mul(x64, x64);
    // x^254 = x^(2+4+8+16+32+64+128)
    gf_mul(
        gf_mul(gf_mul(gf_mul(gf_mul(gf_mul(x2, x4), x8), x16), x32), x64),
        x128,
    )
}

/// Аффинное преобразование FIPS-197 §5.1.1.
#[inline(always)]
const fn affine(x: u8) -> u8 {
    x ^ x.rotate_left(1) ^ x.rotate_left(2) ^ x.rotate_left(3) ^ x.rotate_left(4) ^ 0x63
}

/// Обратное аффинное преобразование FIPS-197.
#[inline(always)]
const fn inv_affine(y: u8) -> u8 {
    y.rotate_left(1) ^ y.rotate_left(3) ^ y.rotate_left(6) ^ 0x05
}

/// S-box FIPS-197 без таблиц: `affine(x^254)`.
///
/// Табличный S-box не используется намеренно: обращение к памяти по индексу,
/// зависящему от секрета, — канал утечки ключевого материала через кэш.
/// Здесь индекс отсутствует: значение считается регистровыми операциями.
#[inline(always)]
pub const fn sbox(x: u8) -> u8 {
    affine(gf_inv(x))
}

/// Обратный S-box FIPS-197 без таблиц: `(A^-1(y))^254`.
pub const fn inv_sbox(y: u8) -> u8 {
    gf_inv(inv_affine(y))
}

/// Расширение ключа AES-128: 44 слова (176 Б) по FIPS-197 §5.2 (`Nk = 4, Nr = 10`).
fn expand_key_128(key: &[u8; AES128_KEY_SIZE]) -> [u8; 176] {
    let mut round_keys = [0u8; 176];
    round_keys[..AES128_KEY_SIZE].copy_from_slice(key);
    let mut rcon: u8 = 0x01;
    let mut word = 4usize;
    while word < 44 {
        let prev = 4 * (word - 1);
        let mut temp = [
            round_keys[prev],
            round_keys[prev + 1],
            round_keys[prev + 2],
            round_keys[prev + 3],
        ];
        if word.is_multiple_of(4) {
            temp = [temp[1], temp[2], temp[3], temp[0]];
            for byte in temp.iter_mut() {
                *byte = sbox(*byte);
            }
            temp[0] ^= rcon;
            rcon = xtime(rcon);
        }
        let base = 4 * word;
        let prev_base = 4 * (word - 4);
        for i in 0..4 {
            round_keys[base + i] = round_keys[prev_base + i] ^ temp[i];
        }
        word += 1;
    }
    round_keys
}

/// Расширение ключа AES-256: 60 слов (240 Б) по FIPS-197 §5.2.
fn expand_key(key: &[u8; AES256_KEY_SIZE]) -> [u8; 240] {
    let mut round_keys = [0u8; 240];
    round_keys[..AES256_KEY_SIZE].copy_from_slice(key);
    let mut rcon: u8 = 0x01;
    let mut word = 8usize;
    while word < 60 {
        let prev = 4 * (word - 1);
        let mut temp = [
            round_keys[prev],
            round_keys[prev + 1],
            round_keys[prev + 2],
            round_keys[prev + 3],
        ];
        if word.is_multiple_of(8) {
            temp = [temp[1], temp[2], temp[3], temp[0]];
            for byte in temp.iter_mut() {
                *byte = sbox(*byte);
            }
            temp[0] ^= rcon;
            rcon = xtime(rcon);
        } else if word % 8 == 4 {
            for byte in temp.iter_mut() {
                *byte = sbox(*byte);
            }
        }
        let base = 4 * word;
        let prev_base = 4 * (word - 8);
        for i in 0..4 {
            round_keys[base + i] = round_keys[prev_base + i] ^ temp[i];
        }
        word += 1;
    }
    round_keys
}

/// Умножение байта на `x` в GF(2^8).
#[inline(always)]
const fn xtime(value: u8) -> u8 {
    (value << 1) ^ (0x1b & (value >> 7).wrapping_neg())
}

/// Добавляет раундовый ключ (байт за байтом).
#[inline(always)]
fn add_round_key(state: &mut [u8; AES_BLOCK_SIZE], round_keys: &[u8], round: usize) {
    let offset = round * AES_BLOCK_SIZE;
    for (i, byte) in state.iter_mut().enumerate() {
        *byte ^= round_keys[offset + i];
    }
}

/// Замена байтов по S-box (вычисляется, без таблицы).
#[inline(always)]
fn sub_bytes(state: &mut [u8; AES_BLOCK_SIZE]) {
    for byte in state.iter_mut() {
        *byte = sbox(*byte);
    }
}

/// Замена байтов по обратному S-box (вычисляется, без таблицы).
fn inv_sub_bytes(state: &mut [u8; AES_BLOCK_SIZE]) {
    for byte in state.iter_mut() {
        *byte = inv_sbox(*byte);
    }
}

/// Сдвиг строк состояния влево (FIPS-197 §5.1.1): строка `r` сдвигается на `r`.
#[inline(always)]
fn shift_rows(state: &mut [u8; AES_BLOCK_SIZE]) {
    let source = *state;
    state[1] = source[5];
    state[5] = source[9];
    state[9] = source[13];
    state[13] = source[1];
    state[2] = source[10];
    state[6] = source[14];
    state[10] = source[2];
    state[14] = source[6];
    state[3] = source[15];
    state[7] = source[3];
    state[11] = source[7];
    state[15] = source[11];
}

/// Обратный сдвиг строк состояния вправо.
fn inv_shift_rows(state: &mut [u8; AES_BLOCK_SIZE]) {
    let source = *state;
    state[1] = source[13];
    state[5] = source[1];
    state[9] = source[5];
    state[13] = source[9];
    state[2] = source[10];
    state[6] = source[14];
    state[10] = source[2];
    state[14] = source[6];
    state[3] = source[7];
    state[7] = source[11];
    state[11] = source[15];
    state[15] = source[3];
}

/// Смешивание столбцов состояния (FIPS-197 §5.1.2).
#[inline(always)]
fn mix_columns(state: &mut [u8; AES_BLOCK_SIZE]) {
    for col in 0..4usize {
        let i = col * 4;
        let a0 = state[i];
        let a1 = state[i + 1];
        let a2 = state[i + 2];
        let a3 = state[i + 3];
        let x0 = xtime(a0);
        let x1 = xtime(a1);
        let x2 = xtime(a2);
        let x3 = xtime(a3);
        state[i] = x0 ^ (x1 ^ a1) ^ a2 ^ a3;
        state[i + 1] = a0 ^ x1 ^ (x2 ^ a2) ^ a3;
        state[i + 2] = a0 ^ a1 ^ x2 ^ (x3 ^ a3);
        state[i + 3] = (x0 ^ a0) ^ a1 ^ a2 ^ x3;
    }
}

/// Обратное смешивание столбцов состояния (FIPS-197 §5.3.2).
fn inv_mix_columns(state: &mut [u8; AES_BLOCK_SIZE]) {
    for col in 0..4usize {
        let i = col * 4;
        let a0 = state[i];
        let a1 = state[i + 1];
        let a2 = state[i + 2];
        let a3 = state[i + 3];
        state[i] = gf_mul(a0, 14) ^ gf_mul(a1, 11) ^ gf_mul(a2, 13) ^ gf_mul(a3, 9);
        state[i + 1] = gf_mul(a0, 9) ^ gf_mul(a1, 14) ^ gf_mul(a2, 11) ^ gf_mul(a3, 13);
        state[i + 2] = gf_mul(a0, 13) ^ gf_mul(a1, 9) ^ gf_mul(a2, 14) ^ gf_mul(a3, 11);
        state[i + 3] = gf_mul(a0, 11) ^ gf_mul(a1, 13) ^ gf_mul(a2, 9) ^ gf_mul(a3, 14);
    }
}
