//! AES-256 в режиме одного блока (FIPS-197) — карточка P4-03.
//!
//! Реализация без внешних зависимостей: S-box и обратный S-box вычисляются
//! константными функциями (инверсия в GF(2^8) + аффинное преобразование FIPS-197),
//! поэтому таблицы не «перепечатаны» и проверяются KAT-вектором FIPS-197 App. C.3
//! (см. `tests/crypto_vectors.rs`).

use super::{AES_BLOCK_SIZE, AES256_KEY_SIZE, CryptoError};
use std::fmt;

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

/// Умножение в GF(2^8) по многочлену FIPS-197 `x^8 + x^4 + x^3 + x + 1`.
///
/// «Русский крестьянин» без ветвлений по данным: выборка по младшему биту
/// делается маской, поэтому по времени выполнения не зависит от значения
/// операндов (важно: операнды могут быть секретными).
const fn gf_mul(mut a: u8, mut b: u8) -> u8 {
    let mut product: u8 = 0;
    let mut i = 0;
    while i < 8 {
        let mask = (b & 1).wrapping_neg();
        product ^= a & mask;
        // Старший бит сдвигаемого байта: 0x00 или 0xFF (маска от бита, не от 0x80).
        let high = (a >> 7).wrapping_neg();
        a <<= 1;
        a ^= 0x1b & high;
        b >>= 1;
        i += 1;
    }
    product
}

/// Обратный элемент в GF(2^8): `x^254` (0^254 = 0, отдельная ветка не нужна).
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
const fn affine(x: u8) -> u8 {
    x ^ x.rotate_left(1) ^ x.rotate_left(2) ^ x.rotate_left(3) ^ x.rotate_left(4) ^ 0x63
}

/// Обратное аффинное преобразование FIPS-197.
const fn inv_affine(y: u8) -> u8 {
    y.rotate_left(1) ^ y.rotate_left(3) ^ y.rotate_left(6) ^ 0x05
}

/// S-box FIPS-197 без таблиц: `affine(x^254)`.
///
/// Табличный S-box не используется намеренно: обращение к памяти по индексу,
/// зависящему от секрета, — канал утечки ключевого материала через кэш.
/// Здесь индекс отсутствует: значение считается регистровыми операциями.
pub const fn sbox(x: u8) -> u8 {
    affine(gf_inv(x))
}

/// Обратный S-box FIPS-197 без таблиц: `(A^-1(y))^254`.
pub const fn inv_sbox(y: u8) -> u8 {
    gf_inv(inv_affine(y))
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
const fn xtime(value: u8) -> u8 {
    (value << 1) ^ (((value >> 7) & 1) * 0x1b)
}

/// Добавляет раундовый ключ (байт за байтом).
fn add_round_key(state: &mut [u8; AES_BLOCK_SIZE], round_keys: &[u8; 240], round: usize) {
    let offset = round * AES_BLOCK_SIZE;
    for (i, byte) in state.iter_mut().enumerate() {
        *byte ^= round_keys[offset + i];
    }
}

/// Замена байтов по S-box (вычисляется, без таблицы).
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
fn mix_columns(state: &mut [u8; AES_BLOCK_SIZE]) {
    for col in 0..4usize {
        let i = col * 4;
        let a0 = state[i];
        let a1 = state[i + 1];
        let a2 = state[i + 2];
        let a3 = state[i + 3];
        state[i] = gf_mul(a0, 2) ^ gf_mul(a1, 3) ^ a2 ^ a3;
        state[i + 1] = a0 ^ gf_mul(a1, 2) ^ gf_mul(a2, 3) ^ a3;
        state[i + 2] = a0 ^ a1 ^ gf_mul(a2, 2) ^ gf_mul(a3, 3);
        state[i + 3] = gf_mul(a0, 3) ^ a1 ^ a2 ^ gf_mul(a3, 2);
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
