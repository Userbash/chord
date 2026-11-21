//! Мастер-ключ из Linux Keyring (`org.freedesktop.secrets`) и деривация ключа БД через Argon2id.
//!
//! Реализация без `unsafe` (`#![forbid(unsafe_code)]`):
//! - Чистый Rust RFC 9106 Argon2id (Blake2b-512 / H' + гибридная индексация Argon2i/Argon2d).
//! - Автоматическая зачистка буферов (`zeroize_now` + `Drop` с `compiler_fence`).
//! - Редакция секретов в `Debug` (`[REDACTED]`).

use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::sync::Mutex;
use std::sync::atomic::{Ordering, compiler_fence};

/// Ошибки работы с Keyring и деривацией ключей Argon2id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyringError {
    /// Некорректные параметры Argon2id или соли.
    InvalidParams(String),
    /// Секрет не найден в Keyring.
    NotFound,
    /// Ошибка бэкенда Keyring (`org.freedesktop.secrets`).
    Backend(String),
    /// Ошибка ввода-вывода (например, при чтении CSPRNG).
    Io(String),
}

impl fmt::Display for KeyringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidParams(msg) => write!(f, "invalid argon2id params: {msg}"),
            Self::NotFound => write!(f, "secret not found in keyring"),
            Self::Backend(msg) => write!(f, "keyring backend error: {msg}"),
            Self::Io(msg) => write!(f, "io error: {msg}"),
        }
    }
}

impl std::error::Error for KeyringError {}

/// Параметры деривации Argon2id (RFC 9106).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Argon2idParams {
    /// Объём памяти в КиБ (минимум `8 * parallelism`).
    pub memory_kib: u32,
    /// Число проходов по памяти (`>= 1`).
    pub iterations: u32,
    /// Число параллельных полос (`>= 1`).
    pub parallelism: u32,
    /// Длина выходного ключа в байтах (`>= 4`, обычно 32).
    pub output_len: usize,
}

impl Default for Argon2idParams {
    fn default() -> Self {
        Self {
            memory_kib: 65_536,
            iterations: 3,
            parallelism: 1,
            output_len: 32,
        }
    }
}

impl Argon2idParams {
    fn validate(&self, salt: &[u8]) -> Result<(), KeyringError> {
        if self.parallelism == 0 {
            return Err(KeyringError::InvalidParams(
                "parallelism must be >= 1".to_owned(),
            ));
        }
        if self.iterations == 0 {
            return Err(KeyringError::InvalidParams(
                "iterations must be >= 1".to_owned(),
            ));
        }
        if self.memory_kib < 8 * self.parallelism {
            return Err(KeyringError::InvalidParams(format!(
                "memory_kib ({}) must be >= 8 * parallelism ({})",
                self.memory_kib,
                8 * self.parallelism
            )));
        }
        if self.output_len < 4 {
            return Err(KeyringError::InvalidParams(
                "output_len must be >= 4".to_owned(),
            ));
        }
        if salt.len() < 8 {
            return Err(KeyringError::InvalidParams(format!(
                "salt length ({}) must be >= 8 bytes",
                salt.len()
            )));
        }
        Ok(())
    }
}

/// Производный ключ шифрования с зачисткой памяти при освобождении.
#[derive(Clone, PartialEq, Eq)]
pub struct DerivedKey {
    bytes: Vec<u8>,
}

impl fmt::Debug for DerivedKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("DerivedKey").field(&"[REDACTED]").finish()
    }
}

impl DerivedKey {
    /// Создаёт `DerivedKey` из вектора байт.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    /// Возвращает срез байт ключа.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Принудительно затирает буфер ключа нулями.
    pub fn zeroize_now(&mut self) {
        zeroize_slice(&mut self.bytes);
    }
}

impl Drop for DerivedKey {
    fn drop(&mut self) {
        self.zeroize_now();
    }
}

fn zeroize_slice(buf: &mut [u8]) {
    for b in buf.iter_mut() {
        *b = 0;
    }
    compiler_fence(Ordering::SeqCst);
}

/// Провайдер секретов Linux Secret Service API (`org.freedesktop.secrets`).
pub trait SecretServiceProvider {
    /// Читает секрет по `(service, account)`.
    fn get_secret(&self, service: &str, account: &str) -> Result<Vec<u8>, KeyringError>;
    /// Сохраняет секрет по `(service, account)`.
    fn set_secret(&self, service: &str, account: &str, secret: &[u8]) -> Result<(), KeyringError>;
}

/// Абстракция бэкенда Keyring (`org.freedesktop.secrets`).
pub trait KeyringBackend: SecretServiceProvider {}
impl<T: SecretServiceProvider> KeyringBackend for T {}

/// Потокобезопасный in-memory Keyring для модульных и контрактных тестов.
#[derive(Debug, Default)]
pub struct InMemoryKeyring {
    entries: Mutex<HashMap<(String, String), Vec<u8>>>,
}

impl InMemoryKeyring {
    /// Создаёт пустой in-memory Keyring.
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretServiceProvider for InMemoryKeyring {
    fn get_secret(&self, service: &str, account: &str) -> Result<Vec<u8>, KeyringError> {
        let guard = self
            .entries
            .lock()
            .map_err(|_| KeyringError::Backend("keyring mutex poisoned".to_owned()))?;
        guard
            .get(&(service.to_owned(), account.to_owned()))
            .cloned()
            .ok_or(KeyringError::NotFound)
    }

    fn set_secret(&self, service: &str, account: &str, secret: &[u8]) -> Result<(), KeyringError> {
        if secret.is_empty() || secret.iter().all(|&b| b == 0) {
            return Err(KeyringError::InvalidParams(
                "secret must be non-empty and non-zero".to_owned(),
            ));
        }
        let mut guard = self
            .entries
            .lock()
            .map_err(|_| KeyringError::Backend("keyring mutex poisoned".to_owned()))?;
        guard.insert((service.to_owned(), account.to_owned()), secret.to_vec());
        Ok(())
    }
}

/// Менеджер мастер-ключа БД поверх `SecretServiceProvider` и Argon2id.
#[derive(Debug)]
pub struct MasterKeyManager<P: SecretServiceProvider> {
    provider: P,
    params: Argon2idParams,
}

impl<P: SecretServiceProvider> MasterKeyManager<P> {
    /// Создаёт менеджер с параметрами Argon2id по умолчанию.
    pub fn new(provider: P) -> Self {
        Self {
            provider,
            params: Argon2idParams::default(),
        }
    }

    /// Создаёт менеджер с явными параметрами Argon2id.
    pub fn with_params(provider: P, params: Argon2idParams) -> Self {
        Self { provider, params }
    }

    /// Загружает мастер-секрет из Keyring (или генерирует 256-битный CSPRNG-секрет при отсутствии)
    /// и деривирует ключ БД через Argon2id с аппаратной солью `hardware_salt`.
    pub fn derive_or_create_master_key(
        &self,
        service: &str,
        account: &str,
        hardware_salt: &[u8],
    ) -> Result<DerivedKey, KeyringError> {
        let mut secret = match self.provider.get_secret(service, account) {
            Ok(existing) => {
                if existing.len() < 32 || existing.iter().all(|&b| b == 0) {
                    return Err(KeyringError::Backend(
                        "stored keyring secret is weak or corrupted".to_owned(),
                    ));
                }
                existing
            }
            Err(KeyringError::NotFound) => {
                let generated = generate_csprng_key_256()?;
                self.provider.set_secret(service, account, &generated)?;
                generated
            }
            Err(err) => return Err(err),
        };

        let derived = argon2id_derive(&secret, hardware_salt, &self.params);
        zeroize_slice(&mut secret);
        derived
    }
}

fn generate_csprng_key_256() -> Result<Vec<u8>, KeyringError> {
    let mut file = File::open("/dev/urandom")
        .map_err(|err| KeyringError::Io(format!("open /dev/urandom: {err}")))?;
    let mut buf = vec![0u8; 32];
    file.read_exact(&mut buf)
        .map_err(|err| KeyringError::Io(format!("read /dev/urandom: {err}")))?;
    if buf.iter().all(|&b| b == 0) {
        return Err(KeyringError::Backend(
            "CSPRNG returned all-zero key".to_owned(),
        ));
    }
    Ok(buf)
}

/// Деривирует ключ заданной длины по RFC 9106 Argon2id.
pub fn argon2id_derive(
    password: &[u8],
    salt: &[u8],
    params: &Argon2idParams,
) -> Result<DerivedKey, KeyringError> {
    params.validate(salt)?;

    let lanes = params.parallelism;
    let lane_len = (params.memory_kib / (4 * lanes)) * 4;
    let lane_len = lane_len.max(8) as usize;
    let mem_blocks = lane_len * (lanes as usize);
    let seg_len = lane_len / 4;

    let h0 = argon2_h0(
        lanes,
        params.output_len as u32,
        params.memory_kib,
        params.iterations,
        2, // Argon2id
        password,
        salt,
    );

    let mut memory = vec![[0u64; 128]; mem_blocks];

    for lane in 0..(lanes as usize) {
        for idx in 0..2 {
            let mut input = Vec::with_capacity(72);
            input.extend_from_slice(&h0);
            input.extend_from_slice(&(idx as u32).to_le_bytes());
            input.extend_from_slice(&(lane as u32).to_le_bytes());
            let block_bytes = blake2b_long(&input, 1024);
            memory[lane * lane_len + idx] = bytes_to_block(&block_bytes);
        }
    }

    for pass in 0..params.iterations {
        for slice in 0..4u32 {
            for lane in 0..(lanes as usize) {
                let start_idx = if pass == 0 && slice == 0 { 2 } else { 0 };
                for idx in start_idx..seg_len {
                    let cur_offset = (slice as usize) * seg_len + idx;
                    let cur_pos = lane * lane_len + cur_offset;
                    let prev_offset = if cur_offset == 0 {
                        lane_len - 1
                    } else {
                        cur_offset - 1
                    };
                    let prev_pos = lane * lane_len + prev_offset;

                    let (j1, j2) = if pass == 0 && slice < 2 {
                        let r = argon2i_index(
                            pass,
                            lane as u32,
                            slice,
                            mem_blocks as u32,
                            params.iterations,
                            idx,
                        );
                        (r as u32, (r >> 32) as u32)
                    } else {
                        let w = memory[prev_pos][0];
                        (w as u32, (w >> 32) as u32)
                    };

                    let ref_lane = if pass == 0 && slice == 0 {
                        lane
                    } else {
                        (j2 as usize) % (lanes as usize)
                    };
                    let ref_index = index_alpha(
                        pass,
                        slice,
                        idx,
                        lane_len,
                        seg_len,
                        ref_lane == lane,
                        j1,
                    );
                    let ref_pos = ref_lane * lane_len + ref_index;

                    let next = compress_blocks(&memory[prev_pos], &memory[ref_pos]);
                    if pass == 0 {
                        memory[cur_pos] = next;
                    } else {
                        for k in 0..128 {
                            memory[cur_pos][k] ^= next[k];
                        }
                    }
                }
            }
        }
    }

    let mut final_block = memory[lane_len - 1];
    for lane in 1..(lanes as usize) {
        let last = &memory[lane * lane_len + (lane_len - 1)];
        for k in 0..128 {
            final_block[k] ^= last[k];
        }
    }

    let final_bytes = block_to_bytes(&final_block);
    let out = blake2b_long(&final_bytes, params.output_len);

    for block in memory.iter_mut() {
        *block = [0u64; 128];
    }
    compiler_fence(Ordering::SeqCst);

    Ok(DerivedKey::from_bytes(out))
}

fn argon2_h0(
    lanes: u32,
    tag_len: u32,
    memory_kib: u32,
    iterations: u32,
    argon_type: u32,
    password: &[u8],
    salt: &[u8],
) -> [u8; 64] {
    let mut buf = Vec::with_capacity(40 + password.len() + salt.len());
    buf.extend_from_slice(&lanes.to_le_bytes());
    buf.extend_from_slice(&tag_len.to_le_bytes());
    buf.extend_from_slice(&memory_kib.to_le_bytes());
    buf.extend_from_slice(&iterations.to_le_bytes());
    buf.extend_from_slice(&0x13u32.to_le_bytes()); // Argon2 v1.3
    buf.extend_from_slice(&argon_type.to_le_bytes());
    buf.extend_from_slice(&(password.len() as u32).to_le_bytes());
    buf.extend_from_slice(password);
    buf.extend_from_slice(&(salt.len() as u32).to_le_bytes());
    buf.extend_from_slice(salt);
    buf.extend_from_slice(&0u32.to_le_bytes()); // secret len
    buf.extend_from_slice(&0u32.to_le_bytes()); // ad len
    blake2b(&buf, 64).try_into().expect("64 bytes")
}

fn blake2b_long(input: &[u8], out_len: usize) -> Vec<u8> {
    let mut prefix = Vec::with_capacity(4 + input.len());
    prefix.extend_from_slice(&(out_len as u32).to_le_bytes());
    prefix.extend_from_slice(input);

    if out_len <= 64 {
        return blake2b(&prefix, out_len);
    }

    let mut out = Vec::with_capacity(out_len);
    let mut v = blake2b(&prefix, 64);
    out.extend_from_slice(&v[..32]);

    let mut remaining = out_len - 32;
    while remaining > 64 {
        v = blake2b(&v, 64);
        out.extend_from_slice(&v[..32]);
        remaining -= 32;
    }
    let tail = blake2b(&v, remaining);
    out.extend_from_slice(&tail);
    out
}

fn argon2i_index(
    pass: u32,
    lane: u32,
    slice: u32,
    mem_blocks: u32,
    iterations: u32,
    idx: usize,
) -> u64 {
    let counter = ((idx / 128) + 1) as u64;
    let mut input = [0u64; 128];
    input[0] = pass as u64;
    input[1] = lane as u64;
    input[2] = slice as u64;
    input[3] = mem_blocks as u64;
    input[4] = iterations as u64;
    input[5] = 2; // Argon2id
    input[6] = counter;

    let zero = [0u64; 128];
    let tmp = compress_blocks(&zero, &input);
    let out = compress_blocks(&zero, &tmp);
    out[idx % 128]
}

fn index_alpha(
    pass: u32,
    slice: u32,
    idx: usize,
    lane_len: usize,
    seg_len: usize,
    same_lane: bool,
    j1: u32,
) -> usize {
    let ref_area_size = if pass == 0 {
        if slice == 0 {
            idx.saturating_sub(1)
        } else if same_lane {
            (slice as usize) * seg_len + idx - 1
        } else {
            (slice as usize) * seg_len - usize::from(idx == 0)
        }
    } else if same_lane {
        lane_len - seg_len + idx - 1
    } else {
        lane_len - seg_len - usize::from(idx == 0)
    };

    if ref_area_size == 0 {
        return 0;
    }

    let mut rel = (j1 as u64).wrapping_mul(j1 as u64) >> 32;
    rel = (ref_area_size as u64) - 1 - (((ref_area_size as u64) * rel) >> 32);

    let start_pos = if pass != 0 && slice != 3 {
        ((slice as usize) + 1) * seg_len
    } else {
        0
    };

    (start_pos + (rel as usize)) % lane_len
}

fn bytes_to_block(bytes: &[u8]) -> [u64; 128] {
    let mut block = [0u64; 128];
    for (i, chunk) in bytes.as_chunks::<8>().0.iter().take(128).enumerate() {
        block[i] = u64::from_le_bytes(*chunk);
    }
    block
}

fn block_to_bytes(block: &[u64; 128]) -> [u8; 1024] {
    let mut out = [0u8; 1024];
    for (i, &w) in block.iter().enumerate() {
        out[i * 8..(i + 1) * 8].copy_from_slice(&w.to_le_bytes());
    }
    out
}

fn compress_blocks(x: &[u64; 128], y: &[u64; 128]) -> [u64; 128] {
    let mut r = [0u64; 128];
    for i in 0..128 {
        r[i] = x[i] ^ y[i];
    }
    let mut z = r;

    for i in 0..8 {
        let b = i * 16;
        blake2_round(
            &mut z[b..b + 16],
            [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        );
    }

    for i in 0..8 {
        let mut idx = [0usize; 16];
        for j in 0..8 {
            idx[j * 2] = j * 16 + i * 2;
            idx[j * 2 + 1] = j * 16 + i * 2 + 1;
        }
        blake2_round(&mut z, idx);
    }

    for i in 0..128 {
        z[i] ^= r[i];
    }
    z
}

#[inline]
fn f_bla_mka(x: u64, y: u64) -> u64 {
    let m = 0xFFFF_FFFFu64;
    let xy = (x & m).wrapping_mul(y & m);
    x.wrapping_add(y).wrapping_add(xy.wrapping_mul(2))
}

#[inline]
fn g_argon(v: &mut [u64], a: usize, b: usize, c: usize, d: usize) {
    v[a] = f_bla_mka(v[a], v[b]);
    v[d] = (v[d] ^ v[a]).rotate_right(32);
    v[c] = f_bla_mka(v[c], v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(24);
    v[a] = f_bla_mka(v[a], v[b]);
    v[d] = (v[d] ^ v[a]).rotate_right(16);
    v[c] = f_bla_mka(v[c], v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(63);
}

fn blake2_round(v: &mut [u64], idx: [usize; 16]) {
    g_argon(v, idx[0], idx[4], idx[8], idx[12]);
    g_argon(v, idx[1], idx[5], idx[9], idx[13]);
    g_argon(v, idx[2], idx[6], idx[10], idx[14]);
    g_argon(v, idx[3], idx[7], idx[11], idx[15]);
    g_argon(v, idx[0], idx[5], idx[10], idx[15]);
    g_argon(v, idx[1], idx[6], idx[11], idx[12]);
    g_argon(v, idx[2], idx[7], idx[8], idx[13]);
    g_argon(v, idx[3], idx[4], idx[9], idx[14]);
}

// Минимальная реализация Blake2b (RFC 7693) без unsafe.
const BLAKE2B_IV: [u64; 8] = [
    0x6a09e667f3bcc908,
    0xbb67ae8584caa73b,
    0x3c6ef372fe94f82b,
    0xa54ff53a5f1d36f1,
    0x510e527fade682d1,
    0x9b05688c2b3e6c1f,
    0x1f83d9abfb41bd6b,
    0x5be0cd19137e2179,
];

const BLAKE2B_SIGMA: [[usize; 16]; 12] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
    [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
    [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
    [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
    [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
    [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
    [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
    [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
    [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0],
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
];

fn blake2b(input: &[u8], out_len: usize) -> Vec<u8> {
    let mut h = BLAKE2B_IV;
    h[0] ^= 0x0101_0000 ^ (out_len as u64);

    let mut offset = 0usize;
    let mut t = 0u128;

    while input.len().saturating_sub(offset) > 128 {
        t += 128;
        blake2b_compress(&mut h, &input[offset..offset + 128], t, false);
        offset += 128;
    }

    let rem = input.len() - offset;
    t += rem as u128;
    let mut last = [0u8; 128];
    last[..rem].copy_from_slice(&input[offset..]);
    blake2b_compress(&mut h, &last, t, true);

    let mut out = Vec::with_capacity(out_len);
    for &word in &h {
        out.extend_from_slice(&word.to_le_bytes());
    }
    out.truncate(out_len);
    out
}

fn blake2b_compress(h: &mut [u64; 8], block: &[u8], t: u128, last: bool) {
    let mut m = [0u64; 16];
    for (i, chunk) in block.as_chunks::<8>().0.iter().take(16).enumerate() {
        m[i] = u64::from_le_bytes(*chunk);
    }

    let mut v = [0u64; 16];
    v[..8].copy_from_slice(h);
    v[8..].copy_from_slice(&BLAKE2B_IV);
    v[12] ^= t as u64;
    v[13] ^= (t >> 64) as u64;
    if last {
        v[14] = !v[14];
    }

    for sigma in &BLAKE2B_SIGMA {
        blake2b_g(&mut v, 0, 4, 8, 12, m[sigma[0]], m[sigma[1]]);
        blake2b_g(&mut v, 1, 5, 9, 13, m[sigma[2]], m[sigma[3]]);
        blake2b_g(&mut v, 2, 6, 10, 14, m[sigma[4]], m[sigma[5]]);
        blake2b_g(&mut v, 3, 7, 11, 15, m[sigma[6]], m[sigma[7]]);
        blake2b_g(&mut v, 0, 5, 10, 15, m[sigma[8]], m[sigma[9]]);
        blake2b_g(&mut v, 1, 6, 11, 12, m[sigma[10]], m[sigma[11]]);
        blake2b_g(&mut v, 2, 7, 8, 13, m[sigma[12]], m[sigma[13]]);
        blake2b_g(&mut v, 3, 4, 9, 14, m[sigma[14]], m[sigma[15]]);
    }

    for i in 0..8 {
        h[i] ^= v[i] ^ v[i + 8];
    }
}

#[inline]
fn blake2b_g(v: &mut [u64; 16], a: usize, b: usize, c: usize, d: usize, x: u64, y: u64) {
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(x);
    v[d] = (v[d] ^ v[a]).rotate_right(32);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(24);
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(y);
    v[d] = (v[d] ^ v[a]).rotate_right(16);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(63);
}
