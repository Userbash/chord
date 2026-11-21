#![forbid(unsafe_code)]
//! Прозрачное страничное шифрование SQLite (AES-256-GCM, страницы 4096 Б) и WAL-журнал.

use std::collections::BTreeMap;
use std::fmt;

/// Размер страницы SQLite по умолчанию (байт).
pub const SQLITE_PAGE_SIZE: usize = 4096;
/// Длина 96-битного IV (nonce) для AES-256-GCM.
pub const GCM_NONCE_LEN: usize = 12;
/// Длина 128-битного тега аутентификации AES-256-GCM.
pub const GCM_TAG_LEN: usize = 16;
/// Размер зашифрованного фрейма страницы на диске: `[nonce | ciphertext | tag]`.
pub const ENCRYPTED_PAGE_SIZE: usize = GCM_NONCE_LEN + SQLITE_PAGE_SIZE + GCM_TAG_LEN;

const AES_BLOCK_LEN: usize = 16;
const AES256_KEY_LEN: usize = 32;
const AES256_ROUNDS: usize = 14;
const AES256_ROUND_KEYS: usize = (AES256_ROUNDS + 1) * 4;

#[rustfmt::skip]
const SBOX: [u8; 256] = [
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

const RCON: [u32; 8] = [
    0x0100_0000,
    0x0200_0000,
    0x0400_0000,
    0x0800_0000,
    0x1000_0000,
    0x2000_0000,
    0x4000_0000,
    0x8000_0000,
];

/// Ошибка страничного шифрования SQLite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncryptedSqliteError {
    /// Неверный размер страницы или зашифрованного фрейма.
    InvalidPageSize { expected: usize, actual: usize },
    /// Ошибка проверки тега аутентификации AES-256-GCM.
    AuthenticationFailed,
    /// Переполнение счётчика IV.
    NonceExhausted,
}

impl fmt::Display for EncryptedSqliteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPageSize { expected, actual } => {
                write!(
                    f,
                    "неверный размер страницы: ожидалось {expected}, получено {actual}"
                )
            }
            Self::AuthenticationFailed => write!(f, "ошибка аутентификации AES-256-GCM"),
            Self::NonceExhausted => write!(f, "исчерпан счётчик IV"),
        }
    }
}

impl std::error::Error for EncryptedSqliteError {}

fn xtime(x: u8) -> u8 {
    let shifted = x << 1;
    if (x & 0x80) != 0 {
        shifted ^ 0x1b
    } else {
        shifted
    }
}

fn gf_mul(mut a: u8, mut b: u8) -> u8 {
    let mut acc = 0u8;
    for _ in 0..8 {
        if (b & 1) != 0 {
            acc ^= a;
        }
        a = xtime(a);
        b >>= 1;
    }
    acc
}

fn sub_word(w: u32) -> u32 {
    let b = w.to_be_bytes();
    u32::from_be_bytes([
        SBOX[b[0] as usize],
        SBOX[b[1] as usize],
        SBOX[b[2] as usize],
        SBOX[b[3] as usize],
    ])
}

fn expand_key_256(key: &[u8; AES256_KEY_LEN]) -> [u32; AES256_ROUND_KEYS] {
    let mut rk = [0u32; AES256_ROUND_KEYS];
    for i in 0..8 {
        let o = i * 4;
        rk[i] = u32::from_be_bytes([key[o], key[o + 1], key[o + 2], key[o + 3]]);
    }
    for i in 8..AES256_ROUND_KEYS {
        let mut temp = rk[i - 1];
        if i % 8 == 0 {
            temp = sub_word(temp.rotate_left(8)) ^ RCON[(i / 8) - 1];
        } else if i % 8 == 4 {
            temp = sub_word(temp);
        }
        rk[i] = rk[i - 8] ^ temp;
    }
    rk
}

fn add_round_key(state: &mut [u8; AES_BLOCK_LEN], rk: &[u32]) {
    for col in 0..4 {
        let bytes = rk[col].to_be_bytes();
        for row in 0..4 {
            state[col * 4 + row] ^= bytes[row];
        }
    }
}

fn sub_bytes_and_shift_rows(state: &mut [u8; AES_BLOCK_LEN]) {
    for byte in state.iter_mut() {
        *byte = SBOX[*byte as usize];
    }
    state.swap(1, 5);
    state.swap(5, 9);
    state.swap(9, 13);
    state.swap(2, 10);
    state.swap(6, 14);
    state.swap(15, 11);
    state.swap(11, 7);
    state.swap(7, 3);
}

fn mix_columns(state: &mut [u8; AES_BLOCK_LEN]) {
    for col in 0..4 {
        let i = col * 4;
        let (s0, s1, s2, s3) = (state[i], state[i + 1], state[i + 2], state[i + 3]);
        state[i] = gf_mul(s0, 2) ^ gf_mul(s1, 3) ^ s2 ^ s3;
        state[i + 1] = s0 ^ gf_mul(s1, 2) ^ gf_mul(s2, 3) ^ s3;
        state[i + 2] = s0 ^ s1 ^ gf_mul(s2, 2) ^ gf_mul(s3, 3);
        state[i + 3] = gf_mul(s0, 3) ^ s1 ^ s2 ^ gf_mul(s3, 2);
    }
}

fn aes256_encrypt_block(
    rk: &[u32; AES256_ROUND_KEYS],
    input: &[u8; AES_BLOCK_LEN],
) -> [u8; AES_BLOCK_LEN] {
    let mut state = *input;
    add_round_key(&mut state, &rk[0..4]);
    for round in 1..AES256_ROUNDS {
        sub_bytes_and_shift_rows(&mut state);
        mix_columns(&mut state);
        add_round_key(&mut state, &rk[round * 4..(round + 1) * 4]);
    }
    sub_bytes_and_shift_rows(&mut state);
    add_round_key(&mut state, &rk[AES256_ROUNDS * 4..AES256_ROUND_KEYS]);
    state
}

fn gf128_mul(x: u128, y: u128) -> u128 {
    let mut z = 0u128;
    let mut v = y;
    for i in 0..128 {
        if ((x >> (127 - i)) & 1) != 0 {
            z ^= v;
        }
        let lsb = v & 1;
        v >>= 1;
        if lsb != 0 {
            v ^= 0xe100_0000_0000_0000_0000_0000_0000_0000;
        }
    }
    z
}

fn ghash_update(h: u128, acc: &mut u128, data: &[u8]) {
    for chunk in data.chunks(AES_BLOCK_LEN) {
        let mut block = [0u8; AES_BLOCK_LEN];
        block[..chunk.len()].copy_from_slice(chunk);
        *acc = gf128_mul(*acc ^ u128::from_be_bytes(block), h);
    }
}

fn ghash_tag(
    rk: &[u32; AES256_ROUND_KEYS],
    j0: &[u8; AES_BLOCK_LEN],
    aad: &[u8],
    ciphertext: &[u8],
) -> [u8; GCM_TAG_LEN] {
    let h = u128::from_be_bytes(aes256_encrypt_block(rk, &[0u8; AES_BLOCK_LEN]));
    let mut acc = 0u128;
    ghash_update(h, &mut acc, aad);
    ghash_update(h, &mut acc, ciphertext);
    let mut lengths = [0u8; AES_BLOCK_LEN];
    lengths[..8].copy_from_slice(&((aad.len() as u64) * 8).to_be_bytes());
    lengths[8..].copy_from_slice(&((ciphertext.len() as u64) * 8).to_be_bytes());
    acc = gf128_mul(acc ^ u128::from_be_bytes(lengths), h);
    let s = u128::from_be_bytes(aes256_encrypt_block(rk, j0));
    (acc ^ s).to_be_bytes()
}

fn gctr_xor(rk: &[u32; AES256_ROUND_KEYS], j0: &[u8; AES_BLOCK_LEN], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut counter = u32::from_be_bytes([j0[12], j0[13], j0[14], j0[15]]);
    let mut cb = *j0;
    for chunk in data.chunks(AES_BLOCK_LEN) {
        counter = counter.wrapping_add(1);
        cb[12..16].copy_from_slice(&counter.to_be_bytes());
        let keystream = aes256_encrypt_block(rk, &cb);
        for (b, k) in chunk.iter().zip(keystream.iter()) {
            out.push(b ^ k);
        }
    }
    out
}

fn make_j0(nonce: &[u8; GCM_NONCE_LEN]) -> [u8; AES_BLOCK_LEN] {
    let mut j0 = [0u8; AES_BLOCK_LEN];
    j0[..GCM_NONCE_LEN].copy_from_slice(nonce);
    j0[15] = 1;
    j0
}

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

/// Шифрует произвольный буфер через AES-256-GCM.
pub fn aes_256_gcm_encrypt(
    key: &[u8; AES256_KEY_LEN],
    nonce: &[u8; GCM_NONCE_LEN],
    aad: &[u8],
    plaintext: &[u8],
) -> (Vec<u8>, [u8; GCM_TAG_LEN]) {
    let rk = expand_key_256(key);
    let j0 = make_j0(nonce);
    let ciphertext = gctr_xor(&rk, &j0, plaintext);
    let tag = ghash_tag(&rk, &j0, aad, &ciphertext);
    (ciphertext, tag)
}

/// Расшифровывает буфер и проверяет тег аутентификации AES-256-GCM.
pub fn aes_256_gcm_decrypt(
    key: &[u8; AES256_KEY_LEN],
    nonce: &[u8; GCM_NONCE_LEN],
    aad: &[u8],
    ciphertext: &[u8],
    tag: &[u8; GCM_TAG_LEN],
) -> Result<Vec<u8>, EncryptedSqliteError> {
    let rk = expand_key_256(key);
    let j0 = make_j0(nonce);
    let expected_tag = ghash_tag(&rk, &j0, aad, ciphertext);
    if !constant_time_eq(&expected_tag, tag) {
        return Err(EncryptedSqliteError::AuthenticationFailed);
    }
    Ok(gctr_xor(&rk, &j0, ciphertext))
}

fn page_aad(page_no: u32) -> [u8; 8] {
    let mut aad = [0u8; 8];
    aad[..4].copy_from_slice(&page_no.to_le_bytes());
    aad[4..8].copy_from_slice(&(SQLITE_PAGE_SIZE as u32).to_le_bytes());
    aad
}

/// Виртуальная файловая система страничного шифрования SQLite (4096 Б).
#[derive(Debug, Clone)]
pub struct EncryptedSqliteVfs {
    key: [u8; AES256_KEY_LEN],
    write_counter: u64,
}

impl EncryptedSqliteVfs {
    /// Создаёт новый экземпляр VFS с 256-битным ключом.
    pub fn new(key: [u8; AES256_KEY_LEN]) -> Self {
        Self {
            key,
            write_counter: 1,
        }
    }

    /// Шифрует 4096-байтную страницу SQLite с уникальным per-page IV.
    pub fn encrypt_page(
        &mut self,
        page_no: u32,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, EncryptedSqliteError> {
        if plaintext.len() != SQLITE_PAGE_SIZE {
            return Err(EncryptedSqliteError::InvalidPageSize {
                expected: SQLITE_PAGE_SIZE,
                actual: plaintext.len(),
            });
        }
        let write_counter = self.write_counter;
        self.write_counter = self
            .write_counter
            .checked_add(1)
            .ok_or(EncryptedSqliteError::NonceExhausted)?;
        let mut nonce = [0u8; GCM_NONCE_LEN];
        nonce[..4].copy_from_slice(&page_no.to_le_bytes());
        nonce[4..12].copy_from_slice(&write_counter.to_le_bytes());
        let aad = page_aad(page_no);
        let (ciphertext, tag) = aes_256_gcm_encrypt(&self.key, &nonce, &aad, plaintext);
        let mut frame = Vec::with_capacity(ENCRYPTED_PAGE_SIZE);
        frame.extend_from_slice(&nonce);
        frame.extend_from_slice(&ciphertext);
        frame.extend_from_slice(&tag);
        Ok(frame)
    }

    /// Расшифровывает зашифрованный фрейм страницы и проверяет тег и AAD.
    pub fn decrypt_page(
        &self,
        page_no: u32,
        frame: &[u8],
    ) -> Result<Vec<u8>, EncryptedSqliteError> {
        if frame.len() != ENCRYPTED_PAGE_SIZE {
            return Err(EncryptedSqliteError::InvalidPageSize {
                expected: ENCRYPTED_PAGE_SIZE,
                actual: frame.len(),
            });
        }
        let mut nonce = [0u8; GCM_NONCE_LEN];
        nonce.copy_from_slice(&frame[..GCM_NONCE_LEN]);
        let ciphertext = &frame[GCM_NONCE_LEN..GCM_NONCE_LEN + SQLITE_PAGE_SIZE];
        let mut tag = [0u8; GCM_TAG_LEN];
        tag.copy_from_slice(&frame[GCM_NONCE_LEN + SQLITE_PAGE_SIZE..]);
        let aad = page_aad(page_no);
        aes_256_gcm_decrypt(&self.key, &nonce, &aad, ciphertext, &tag)
    }
}

/// Запись зашифрованного WAL-журнала.
#[derive(Debug, Clone)]
pub struct WalFrame {
    pub page_no: u32,
    pub encrypted_frame: Vec<u8>,
    pub committed: bool,
}

/// Зашифрованный WAL-журнал с поддержкой восстановления после сбоя питания.
#[derive(Debug, Default, Clone)]
pub struct EncryptedWal {
    frames: Vec<WalFrame>,
}

impl EncryptedWal {
    /// Создаёт пустой WAL-журнал.
    pub fn new() -> Self {
        Self { frames: Vec::new() }
    }

    /// Добавляет зашифрованный фрейм страницы в WAL (без утечки открытого текста).
    pub fn append_frame(
        &mut self,
        vfs: &mut EncryptedSqliteVfs,
        page_no: u32,
        plaintext_page: &[u8],
    ) -> Result<(), EncryptedSqliteError> {
        let encrypted_frame = vfs.encrypt_page(page_no, plaintext_page)?;
        self.frames.push(WalFrame {
            page_no,
            encrypted_frame,
            committed: false,
        });
        Ok(())
    }

    /// Фиксирует все текущие фреймы транзакции.
    pub fn commit(&mut self) {
        for frame in &mut self.frames {
            frame.committed = true;
        }
    }

    /// Восстановление после сбоя питания: удаляет незафиксированные фреймы.
    pub fn recover_after_crash(&mut self) -> usize {
        let before = self.frames.len();
        self.frames.retain(|f| f.committed);
        before - self.frames.len()
    }

    /// Переносит зафиксированные страницы в основное хранилище и очищает WAL.
    pub fn checkpoint(
        &mut self,
        vfs: &EncryptedSqliteVfs,
        db_pages: &mut BTreeMap<u32, Vec<u8>>,
    ) -> Result<usize, EncryptedSqliteError> {
        let mut applied = 0usize;
        for frame in &self.frames {
            if frame.committed {
                let _ = vfs.decrypt_page(frame.page_no, &frame.encrypted_frame)?;
                db_pages.insert(frame.page_no, frame.encrypted_frame.clone());
                applied += 1;
            }
        }
        self.frames.retain(|f| !f.committed);
        Ok(applied)
    }

    /// Возвращает срез всех фреймов WAL для инспекции.
    pub fn frames(&self) -> &[WalFrame] {
        &self.frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; AES256_KEY_LEN] {
        let mut k = [0u8; AES256_KEY_LEN];
        for (i, b) in k.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(7).wrapping_add(3);
        }
        k
    }

    fn make_page_with_secret(secret: &[u8]) -> Vec<u8> {
        let mut page = vec![0u8; SQLITE_PAGE_SIZE];
        page[..secret.len()].copy_from_slice(secret);
        page
    }

    #[test]
    fn zero_plaintext_on_disk_and_roundtrip() {
        let mut vfs = EncryptedSqliteVfs::new(test_key());
        let secret = b"TOP_SECRET_DISCORD_TOKEN_PLAINTEXT_MARKER";
        let page = make_page_with_secret(secret);
        let frame = vfs.encrypt_page(1, &page).expect("encrypt_page");
        assert_eq!(frame.len(), ENCRYPTED_PAGE_SIZE);
        assert!(
            !frame.windows(secret.len()).any(|w| w == secret),
            "plaintext secret leaked into encrypted page"
        );
        let decrypted = vfs.decrypt_page(1, &frame).expect("decrypt_page");
        assert_eq!(decrypted, page);
    }

    #[test]
    fn unique_iv_per_write_even_for_same_page() {
        let mut vfs = EncryptedSqliteVfs::new(test_key());
        let page = make_page_with_secret(b"SAME_PAGE_CONTENT");
        let f1 = vfs.encrypt_page(5, &page).expect("f1");
        let f2 = vfs.encrypt_page(5, &page).expect("f2");
        assert_ne!(&f1[..GCM_NONCE_LEN], &f2[..GCM_NONCE_LEN]);
        assert_ne!(f1, f2);
    }

    #[test]
    fn rejects_corrupted_gcm_tag_wrong_key_and_page_swap() {
        let mut vfs = EncryptedSqliteVfs::new(test_key());
        let page = make_page_with_secret(b"AUTH_TAG_CHECK");
        let mut frame = vfs.encrypt_page(2, &page).expect("encrypt");

        let last = frame.len() - 1;
        frame[last] ^= 0x01;
        assert_eq!(
            vfs.decrypt_page(2, &frame),
            Err(EncryptedSqliteError::AuthenticationFailed)
        );
        frame[last] ^= 0x01;

        assert_eq!(
            vfs.decrypt_page(3, &frame),
            Err(EncryptedSqliteError::AuthenticationFailed)
        );

        let wrong_vfs = EncryptedSqliteVfs::new([0xffu8; AES256_KEY_LEN]);
        assert_eq!(
            wrong_vfs.decrypt_page(2, &frame),
            Err(EncryptedSqliteError::AuthenticationFailed)
        );
    }

    #[test]
    fn wal_integrity_and_crash_recovery() {
        let mut vfs = EncryptedSqliteVfs::new(test_key());
        let mut wal = EncryptedWal::new();
        let committed_secret = b"COMMITTED_WAL_PAYLOAD";
        let uncommitted_secret = b"UNCOMMITTED_CRASH_PAYLOAD";

        wal.append_frame(&mut vfs, 1, &make_page_with_secret(committed_secret))
            .expect("append committed");
        wal.commit();
        wal.append_frame(&mut vfs, 2, &make_page_with_secret(uncommitted_secret))
            .expect("append uncommitted");

        for f in wal.frames() {
            assert!(
                !f.encrypted_frame
                    .windows(committed_secret.len())
                    .any(|w| w == committed_secret)
            );
            assert!(
                !f.encrypted_frame
                    .windows(uncommitted_secret.len())
                    .any(|w| w == uncommitted_secret)
            );
        }

        let dropped = wal.recover_after_crash();
        assert_eq!(dropped, 1);
        assert_eq!(wal.frames().len(), 1);

        let mut db_pages = BTreeMap::new();
        let applied = wal.checkpoint(&vfs, &mut db_pages).expect("checkpoint");
        assert_eq!(applied, 1);
        assert!(wal.frames().is_empty());
        let recovered = vfs
            .decrypt_page(1, db_pages.get(&1).expect("page 1"))
            .expect("decrypt page 1");
        assert_eq!(&recovered[..committed_secret.len()], committed_secret);
    }
}
