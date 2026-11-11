#![forbid(unsafe_code)]
#![allow(dead_code)]
//! Шифрование локального хранилища SQLite (`AES-256-GCM` / `SQLCipher` page-level)
//! и деривация ключей через Linux Keyring (`org.freedesktop.secrets`) — `TASK-984`.

use std::collections::BTreeMap;
use std::fmt;
use std::hint::black_box;

pub const TASK_ID: &str = "TASK-984";
pub const LAYOUT_CONTAINER: &str = "Encrypted Store";
pub const CIPHER_DIMENSIONS: &str = "AES-256-GCM";
pub const TOKEN_BG: &str = "#1e1f22";
pub const TOKEN_SURFACE: &str = "#2b2d31";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_BORDER: &str = "#3f4147";
pub const SECRET_SERVICE_BUS_NAME: &str = "org.freedesktop.secrets";
pub const KEYRING_SCHEMA_NAME: &str = "com.discordlab.Token";
pub const KEYRING_MASTER_KEY_ATTR: &str = "discord-gpui-sqlite-master-key";
pub const ENCRYPTED_TOKEN_PREFIX: &str = "dQw4w9WgXcQ:";
pub const IPC_SAFE_STORAGE_DECRYPT_STRING: &str = "DISCORD_SAFE_STORAGE_DECRYPT_STRING";
pub const IPC_SAFE_STORAGE_ENCRYPT_STRING: &str = "DISCORD_SAFE_STORAGE_ENCRYPT_STRING";
pub const IPC_SAFE_STORAGE_IS_ENCRYPTION_AVAILABLE: &str =
    "DISCORD_SAFE_STORAGE_IS_ENCRYPTION_AVAILABLE";
pub const CORE_TABLES: [&str; 5] = ["guilds", "channels", "roles", "users", "emoji"];
pub const SQLITE_PLAIN_MAGIC: &[u8; 16] = b"SQLite format 3\0";
pub const ENCRYPTED_CONTAINER_MAGIC: [u8; 8] = *b"DLABENC1";
pub const ENCRYPTED_CONTAINER_VERSION: u16 = 1;
pub const SQLITE_PAGE_SIZE: usize = 4_096;
pub const CONTAINER_HEADER_BYTES: usize = 64;
pub const ENCRYPTED_PAGE_RECORD_BYTES: usize = SQLITE_PAGE_SIZE + 36;
pub const MAX_DB_PAGES: u32 = 262_144;
pub const AES256_KEY_SIZE: usize = 32;
pub const GCM_NONCE_SIZE: usize = 12;
pub const GCM_TAG_SIZE: usize = 16;
pub const SALT_SIZE: usize = 16;
pub const HKDF_INFO_PAGE_KEY: &[u8] = b"discord-lab/sqlite-page-aes256gcm/v1";
pub const HKDF_INFO_WAL_KEY: &[u8] = b"discord-lab/sqlite-wal-aes256gcm/v1";
pub const HKDF_INFO_TOKEN_KEY: &[u8] = b"discord-lab/safe-storage-token-aes256gcm/v1";
pub const HKDF_INFO_HEADER_MAC: &[u8] = b"discord-lab/sqlite-header-hmac-sha256/v1";
pub const SAFE_STORAGE_AAD: &[u8] = b"discord-safe-storage-v1";
pub const MAX_MESSAGE_CHARS: usize = 2_000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_SEARCH_WINDOW: u32 = 10_000;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_BADGE_CAP: u32 = 99;
pub const PERM_VIEW_CHANNEL: u64 = 1 << 10;
pub const PERM_READ_MESSAGE_HISTORY: u64 = 1 << 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyringAuthState {
    Locked,
    Unlocking,
    Decrypted,
}

impl KeyringAuthState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Locked => "locked",
            Self::Unlocking => "unlocking",
            Self::Decrypted => "decrypted",
        }
    }
    pub const fn resolve(unlocked: bool, hovered: bool) -> Self {
        match (unlocked, hovered) {
            (true, _) => Self::Decrypted,
            (false, true) => Self::Unlocking,
            (false, false) => Self::Locked,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncryptedStoreError {
    InvalidKeyringAttribute,
    KeyringEntryNotFound,
    WeakMasterKeyRejected,
    InvalidKeyLength(usize),
    UnalignedSqlitePayload(usize),
    InvalidPageNumber(u32),
    InvalidContainerHeader(&'static str),
    HeaderMacMismatch,
    AuthenticationFailed,
    PageAuthenticationFailed(u32),
    WalFrameAuthenticationFailed(u64),
    BoundaryViolation(&'static str),
    AccessDenied(&'static str),
    HttpStatusError(u16, u64),
    IndexingInProgress(u64),
}

pub fn zeroize_slice(buf: &mut [u8]) {
    for byte in buf.iter_mut() {
        *byte = black_box(0);
    }
}

pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    black_box(diff) == 0
}

#[derive(Clone, PartialEq, Eq)]
pub struct KeyringMasterSecret([u8; AES256_KEY_SIZE]);

impl fmt::Debug for KeyringMasterSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KeyringMasterSecret(<скрыто>)")
    }
}

impl Drop for KeyringMasterSecret {
    fn drop(&mut self) {
        zeroize_slice(&mut self.0);
    }
}

impl KeyringMasterSecret {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, EncryptedStoreError> {
        if bytes.len() != AES256_KEY_SIZE {
            return Err(EncryptedStoreError::InvalidKeyLength(bytes.len()));
        }
        if bytes.windows(2).all(|w| w[0] == w[1]) {
            return Err(EncryptedStoreError::WeakMasterKeyRejected);
        }
        let mut arr = [0u8; AES256_KEY_SIZE];
        arr.copy_from_slice(bytes);
        Ok(Self(arr))
    }
    pub fn from_hex(hex_str: &str) -> Result<Self, EncryptedStoreError> {
        Self::from_bytes(&decode_hex(hex_str.trim())?)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct DerivedStoreKeys {
    pub prk: [u8; 32],
    pub page_key: [u8; 32],
    pub wal_key: [u8; 32],
    pub token_key: [u8; 32],
    pub header_mac_key: [u8; 32],
}

impl fmt::Debug for DerivedStoreKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DerivedStoreKeys(<скрыто>)")
    }
}

impl Drop for DerivedStoreKeys {
    fn drop(&mut self) {
        zeroize_slice(&mut self.prk);
        zeroize_slice(&mut self.page_key);
        zeroize_slice(&mut self.wal_key);
        zeroize_slice(&mut self.token_key);
        zeroize_slice(&mut self.header_mac_key);
    }
}

impl DerivedStoreKeys {
    pub fn derive(master: &KeyringMasterSecret, salt: &[u8; SALT_SIZE]) -> Self {
        let prk = hmac_sha256(if salt.is_empty() { &[0u8; 32] } else { salt }, &master.0);
        Self {
            prk,
            page_key: hkdf_expand_sha256_32(&prk, HKDF_INFO_PAGE_KEY),
            wal_key: hkdf_expand_sha256_32(&prk, HKDF_INFO_WAL_KEY),
            token_key: hkdf_expand_sha256_32(&prk, HKDF_INFO_TOKEN_KEY),
            header_mac_key: hkdf_expand_sha256_32(&prk, HKDF_INFO_HEADER_MAC),
        }
    }
}

pub struct LinuxKeyringClient {
    entries: BTreeMap<String, String>,
    pub state: KeyringAuthState,
}

impl LinuxKeyringClient {
    pub const fn new_locked() -> Self {
        Self {
            entries: BTreeMap::new(),
            state: KeyringAuthState::Locked,
        }
    }
    pub fn store_secret_hex(&mut self, attr: &str, hex: &str) -> Result<(), EncryptedStoreError> {
        if attr.is_empty() || attr.contains('\0') || hex.contains('\0') {
            return Err(EncryptedStoreError::InvalidKeyringAttribute);
        }
        let _ = KeyringMasterSecret::from_hex(hex)?;
        self.entries
            .insert(attr.to_string(), hex.trim().to_string());
        Ok(())
    }
    pub fn unlock_and_derive_keys(
        &mut self,
        attr: &str,
        salt: &[u8; SALT_SIZE],
    ) -> Result<(KeyringMasterSecret, DerivedStoreKeys), EncryptedStoreError> {
        self.state = KeyringAuthState::Unlocking;
        let Some(raw_hex) = self.entries.get(attr).filter(|_| !attr.is_empty()) else {
            self.state = KeyringAuthState::Locked;
            return Err(EncryptedStoreError::KeyringEntryNotFound);
        };
        let master = KeyringMasterSecret::from_hex(raw_hex).inspect_err(|_| {
            self.state = KeyringAuthState::Locked;
        })?;
        let derived = DerivedStoreKeys::derive(&master, salt);
        self.state = KeyringAuthState::Decrypted;
        Ok((master, derived))
    }
}

#[rustfmt::skip]
const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

#[rustfmt::skip]
fn sha256_compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 64];
    for (i, c) in block.chunks_exact(4).enumerate() {
        w[i] = u32::from_be_bytes([c[0], c[1], c[2], c[3]]);
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let t1 = h.wrapping_add(s1).wrapping_add((e & f) ^ ((!e) & g)).wrapping_add(SHA256_K[i]).wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let t2 = s0.wrapping_add((a & b) ^ (a & c) ^ (b & c));
        [a, b, c, d, e, f, g, h] = [t1.wrapping_add(t2), a, b, c, d.wrapping_add(t1), e, f, g];
    }
    for (st, add) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *st = st.wrapping_add(add);
    }
}

pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let chunks = data.chunks_exact(64);
    let rem = chunks.remainder();
    for chunk in chunks {
        let mut blk = [0u8; 64];
        blk.copy_from_slice(chunk);
        sha256_compress(&mut state, &blk);
    }
    let mut last = [0u8; 64];
    last[..rem.len()].copy_from_slice(rem);
    last[rem.len()] = 0x80;
    if rem.len() >= 56 {
        sha256_compress(&mut state, &last);
        last = [0u8; 64];
    }
    last[56..64].copy_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    sha256_compress(&mut state, &last);
    let mut out = [0u8; 32];
    for (i, word) in state.iter().enumerate() {
        out[i * 4..(i + 1) * 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut k_block = [0u8; 64];
    if key.len() > 64 {
        k_block[..32].copy_from_slice(&sha256(key));
    } else {
        k_block[..key.len()].copy_from_slice(key);
    }
    let (mut ipad, mut opad) = ([0x36u8; 64], [0x5cu8; 64]);
    for i in 0..64 {
        ipad[i] ^= k_block[i];
        opad[i] ^= k_block[i];
    }
    let mut inner = Vec::with_capacity(64 + message.len());
    inner.extend_from_slice(&ipad);
    inner.extend_from_slice(message);
    let inner_hash = sha256(&inner);
    let mut outer = [0u8; 96];
    outer[..64].copy_from_slice(&opad);
    outer[64..96].copy_from_slice(&inner_hash);
    zeroize_slice(&mut k_block);
    sha256(&outer)
}

pub fn hkdf_expand_sha256_32(prk: &[u8; 32], info: &[u8]) -> [u8; 32] {
    let mut msg = Vec::with_capacity(info.len() + 1);
    msg.extend_from_slice(info);
    msg.push(0x01);
    hmac_sha256(prk, &msg)
}

const fn gf_mul(mut a: u8, mut b: u8) -> u8 {
    let (mut p, mut i) = (0u8, 0);
    while i < 8 {
        p ^= a & (b & 1).wrapping_neg();
        a = (a << 1) ^ (0x1b & (a >> 7).wrapping_neg());
        b >>= 1;
        i += 1;
    }
    p
}

#[rustfmt::skip]
const fn sbox(x: u8) -> u8 {
    let (x2, x4) = (gf_mul(x, x), gf_mul(gf_mul(x, x), gf_mul(x, x)));
    let (x8, x16) = (gf_mul(x4, x4), gf_mul(gf_mul(x4, x4), gf_mul(x4, x4)));
    let (x32, x64) = (gf_mul(x16, x16), gf_mul(gf_mul(x16, x16), gf_mul(x16, x16)));
    let y = gf_mul(gf_mul(gf_mul(gf_mul(gf_mul(gf_mul(x2, x4), x8), x16), x32), x64), gf_mul(x64, x64));
    y ^ y.rotate_left(1) ^ y.rotate_left(2) ^ y.rotate_left(3) ^ y.rotate_left(4) ^ 0x63
}

fn expand_key_256(key: &[u8; AES256_KEY_SIZE]) -> [u8; 240] {
    let mut rk = [0u8; 240];
    rk[..32].copy_from_slice(key);
    let mut rcon = 0x01u8;
    for word in 8..60 {
        let p = 4 * (word - 1);
        let mut t = [rk[p], rk[p + 1], rk[p + 2], rk[p + 3]];
        if word % 8 == 0 {
            t = [sbox(t[1]) ^ rcon, sbox(t[2]), sbox(t[3]), sbox(t[0])];
            rcon = gf_mul(rcon, 0x02);
        } else if word % 8 == 4 {
            t = [sbox(t[0]), sbox(t[1]), sbox(t[2]), sbox(t[3])];
        }
        for i in 0..4 {
            rk[4 * word + i] = rk[4 * (word - 8) + i] ^ t[i];
        }
    }
    rk
}

fn aes256_encrypt_block(rk: &[u8; 240], block: &[u8; 16]) -> [u8; 16] {
    let mut s = *block;
    for i in 0..16 {
        s[i] ^= rk[i];
    }
    for round in 1..=14 {
        for b in s.iter_mut() {
            *b = sbox(*b);
        }
        s = [
            s[0], s[5], s[10], s[15], s[4], s[9], s[14], s[3], s[8], s[13], s[2], s[7], s[12],
            s[1], s[6], s[11],
        ];
        if round < 14 {
            for col in 0..4 {
                let i = col * 4;
                let (a0, a1, a2, a3) = (s[i], s[i + 1], s[i + 2], s[i + 3]);
                let sum = a0 ^ a1 ^ a2 ^ a3;
                s[i] ^= sum ^ gf_mul(0x02, a0 ^ a1);
                s[i + 1] ^= sum ^ gf_mul(0x02, a1 ^ a2);
                s[i + 2] ^= sum ^ gf_mul(0x02, a2 ^ a3);
                s[i + 3] ^= sum ^ gf_mul(0x02, a3 ^ a0);
            }
        }
        for i in 0..16 {
            s[i] ^= rk[round * 16 + i];
        }
    }
    s
}

fn gf_mul128(x: u128, mut v: u128) -> u128 {
    let mut z = 0u128;
    for shift in (0..128).rev() {
        z ^= v & 0u128.wrapping_sub((x >> shift) & 1);
        v = (v >> 1) ^ ((0xe1u128 << 120) & 0u128.wrapping_sub(v & 1));
    }
    z
}

fn ghash(h: u128, aad: &[u8], ciphertext: &[u8]) -> u128 {
    let mut y = 0u128;
    for slice in [aad, ciphertext] {
        for chunk in slice.chunks(16) {
            let mut blk = [0u8; 16];
            blk[..chunk.len()].copy_from_slice(chunk);
            y = gf_mul128(y ^ u128::from_be_bytes(blk), h);
        }
    }
    let mut len_blk = [0u8; 16];
    len_blk[..8].copy_from_slice(&((aad.len() as u64) * 8).to_be_bytes());
    len_blk[8..16].copy_from_slice(&((ciphertext.len() as u64) * 8).to_be_bytes());
    gf_mul128(y ^ u128::from_be_bytes(len_blk), h)
}

fn gctr(rk: &[u8; 240], icb: u128, input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut counter = icb;
    for chunk in input.chunks(16) {
        let low = (counter & 0xffff_ffff) as u32;
        counter = (counter & !0xffff_ffffu128) | u128::from(low.wrapping_add(1));
        let stream = aes256_encrypt_block(rk, &counter.to_be_bytes());
        for (i, &b) in chunk.iter().enumerate() {
            out.push(b ^ stream[i]);
        }
    }
    out
}

pub struct Aes256Gcm {
    rk: [u8; 240],
    h: u128,
}

impl Drop for Aes256Gcm {
    fn drop(&mut self) {
        zeroize_slice(&mut self.rk);
        self.h = black_box(0);
    }
}

impl Aes256Gcm {
    pub fn new(key: &[u8; AES256_KEY_SIZE]) -> Self {
        let rk = expand_key_256(key);
        let h = u128::from_be_bytes(aes256_encrypt_block(&rk, &[0u8; 16]));
        Self { rk, h }
    }
    fn compute_j0_and_tag(&self, iv: &[u8; 12], aad: &[u8], ct: &[u8]) -> (u128, [u8; 16]) {
        let mut j0_blk = [0u8; 16];
        j0_blk[..12].copy_from_slice(iv);
        j0_blk[15] = 1;
        let s = ghash(self.h, aad, ct).to_be_bytes();
        let mask = aes256_encrypt_block(&self.rk, &j0_blk);
        let mut tag = [0u8; 16];
        for i in 0..16 {
            tag[i] = s[i] ^ mask[i];
        }
        (u128::from_be_bytes(j0_blk), tag)
    }
    pub fn encrypt_detached(&self, iv: &[u8; 12], aad: &[u8], pt: &[u8]) -> (Vec<u8>, [u8; 16]) {
        let (j0, _) = self.compute_j0_and_tag(iv, &[], &[]);
        let ct = gctr(&self.rk, j0, pt);
        let (_, tag) = self.compute_j0_and_tag(iv, aad, &ct);
        (ct, tag)
    }
    pub fn decrypt_detached(
        &self,
        iv: &[u8; 12],
        aad: &[u8],
        ct: &[u8],
        tag: &[u8; 16],
    ) -> Result<Vec<u8>, EncryptedStoreError> {
        let (j0, expected_tag) = self.compute_j0_and_tag(iv, aad, ct);
        if !constant_time_eq(&expected_tag, tag) {
            return Err(EncryptedStoreError::AuthenticationFailed);
        }
        Ok(gctr(&self.rk, j0, ct))
    }
}

pub fn derive_page_nonce(salt: &[u8; SALT_SIZE], page_no: u32, generation: u32) -> [u8; 12] {
    let mut nonce = [0u8; GCM_NONCE_SIZE];
    nonce[0..4].copy_from_slice(&salt[0..4]);
    nonce[4..8].copy_from_slice(&page_no.to_be_bytes());
    nonce[8..12].copy_from_slice(&generation.to_be_bytes());
    nonce
}

pub fn build_page_aad(page_no: u32, generation: u32, page_size: u16) -> [u8; 10] {
    let mut aad = [0u8; 10];
    aad[0..4].copy_from_slice(&page_no.to_be_bytes());
    aad[4..8].copy_from_slice(&generation.to_be_bytes());
    aad[8..10].copy_from_slice(&page_size.to_be_bytes());
    aad
}

#[rustfmt::skip]
pub fn encrypt_sqlite_bytes(plain_db: &[u8], keys: &DerivedStoreKeys, salt: &[u8; SALT_SIZE]) -> Result<Vec<u8>, EncryptedStoreError> {
    if plain_db.is_empty() || !plain_db.len().is_multiple_of(SQLITE_PAGE_SIZE) {
        return Err(EncryptedStoreError::UnalignedSqlitePayload(plain_db.len()));
    }
    let page_count = (plain_db.len() / SQLITE_PAGE_SIZE) as u32;
    let gcm = Aes256Gcm::new(&keys.page_key);
    let mut out = Vec::with_capacity(CONTAINER_HEADER_BYTES + (page_count as usize) * ENCRYPTED_PAGE_RECORD_BYTES);
    let mut hdr = [0u8; CONTAINER_HEADER_BYTES];
    hdr[0..8].copy_from_slice(&ENCRYPTED_CONTAINER_MAGIC);
    hdr[8..10].copy_from_slice(&ENCRYPTED_CONTAINER_VERSION.to_be_bytes());
    hdr[10..12].copy_from_slice(&(SQLITE_PAGE_SIZE as u16).to_be_bytes());
    hdr[12..16].copy_from_slice(&page_count.to_be_bytes());
    hdr[16..32].copy_from_slice(salt);
    out.extend_from_slice(&hdr[0..32]);
    out.extend_from_slice(&hmac_sha256(&keys.header_mac_key, &hdr[0..32]));
    for (idx, page_slice) in plain_db.chunks_exact(SQLITE_PAGE_SIZE).enumerate() {
        let (page_no, generation) = ((idx as u32) + 1, 1u32);
        let nonce = derive_page_nonce(salt, page_no, generation);
        let aad = build_page_aad(page_no, generation, SQLITE_PAGE_SIZE as u16);
        let (ciphertext, tag) = gcm.encrypt_detached(&nonce, &aad, page_slice);
        out.extend_from_slice(&page_no.to_be_bytes());
        out.extend_from_slice(&generation.to_be_bytes());
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ciphertext);
        out.extend_from_slice(&tag);
    }
    Ok(out)
}

#[rustfmt::skip]
pub fn decrypt_sqlite_bytes(blob: &[u8], keys: &DerivedStoreKeys) -> Result<Vec<u8>, EncryptedStoreError> {
    if blob.len() < CONTAINER_HEADER_BYTES || blob[0..8] != ENCRYPTED_CONTAINER_MAGIC {
        return Err(EncryptedStoreError::InvalidContainerHeader("bad header"));
    }
    let page_count = u32::from_be_bytes([blob[12], blob[13], blob[14], blob[15]]);
    if !constant_time_eq(&hmac_sha256(&keys.header_mac_key, &blob[0..32]), &blob[32..64]) {
        return Err(EncryptedStoreError::HeaderMacMismatch);
    }
    let expected = CONTAINER_HEADER_BYTES + (page_count as usize) * ENCRYPTED_PAGE_RECORD_BYTES;
    if page_count == 0 || blob.len() != expected {
        return Err(EncryptedStoreError::InvalidContainerHeader("mismatch"));
    }
    let gcm = Aes256Gcm::new(&keys.page_key);
    let mut plain = Vec::with_capacity((page_count as usize) * SQLITE_PAGE_SIZE);
    for idx in 0..(page_count as usize) {
        let off = CONTAINER_HEADER_BYTES + idx * ENCRYPTED_PAGE_RECORD_BYTES;
        let rec = &blob[off..off + ENCRYPTED_PAGE_RECORD_BYTES];
        plain.extend_from_slice(&decrypt_page_record(&gcm, (idx as u32) + 1, rec)?);
    }
    Ok(plain)
}

#[rustfmt::skip]
fn decrypt_page_record(gcm: &Aes256Gcm, expected_no: u32, rec: &[u8]) -> Result<Vec<u8>, EncryptedStoreError> {
    let rec_no = u32::from_be_bytes([rec[0], rec[1], rec[2], rec[3]]);
    let rec_gen = u32::from_be_bytes([rec[4], rec[5], rec[6], rec[7]]);
    if rec_no != expected_no {
        return Err(EncryptedStoreError::PageAuthenticationFailed(expected_no));
    }
    let (mut nonce, mut tag) = ([0u8; GCM_NONCE_SIZE], [0u8; GCM_TAG_SIZE]);
    nonce.copy_from_slice(&rec[8..20]);
    tag.copy_from_slice(&rec[20 + SQLITE_PAGE_SIZE..ENCRYPTED_PAGE_RECORD_BYTES]);
    let aad = build_page_aad(expected_no, rec_gen, SQLITE_PAGE_SIZE as u16);
    gcm.decrypt_detached(&nonce, &aad, &rec[20..20 + SQLITE_PAGE_SIZE], &tag)
        .map_err(|_| EncryptedStoreError::PageAuthenticationFailed(expected_no))
}

#[rustfmt::skip]
#[derive(Debug, Clone)]
pub struct EncryptedWalFrame {
    pub seq: u64, pub page_no: u32, pub is_commit: bool,
    pub nonce: [u8; GCM_NONCE_SIZE], pub ciphertext: Vec<u8>, pub tag: [u8; GCM_TAG_SIZE],
}

pub struct EncryptedWalJournal {
    salt: [u8; SALT_SIZE],
    frames: Vec<EncryptedWalFrame>,
}

#[rustfmt::skip]
impl EncryptedWalJournal {
    pub const fn new(salt: [u8; SALT_SIZE]) -> Self {
        Self { salt, frames: Vec::new() }
    }
    pub fn append_frame(&mut self, keys: &DerivedStoreKeys, page_no: u32, plain_page: &[u8], is_commit: bool) -> Result<u64, EncryptedStoreError> {
        if page_no == 0 || page_no > MAX_DB_PAGES || plain_page.len() != SQLITE_PAGE_SIZE {
            return Err(EncryptedStoreError::InvalidPageNumber(page_no));
        }
        let seq = (self.frames.len() as u64) + 1;
        let nonce = derive_page_nonce(&self.salt, page_no, seq as u32);
        let mut aad = [0u8; 13];
        aad[0..8].copy_from_slice(&seq.to_be_bytes());
        aad[8..12].copy_from_slice(&page_no.to_be_bytes());
        aad[12] = u8::from(is_commit);
        let (ciphertext, tag) = Aes256Gcm::new(&keys.wal_key).encrypt_detached(&nonce, &aad, plain_page);
        self.frames.push(EncryptedWalFrame { seq, page_no, is_commit, nonce, ciphertext, tag });
        Ok(seq)
    }
    pub fn recover_after_power_loss(&self, keys: &DerivedStoreKeys) -> Result<(BTreeMap<u32, Vec<u8>>, usize), EncryptedStoreError> {
        let gcm = Aes256Gcm::new(&keys.wal_key);
        let (mut committed, mut pending) = (BTreeMap::new(), Vec::new());
        for frame in &self.frames {
            let mut aad = [0u8; 13];
            aad[0..8].copy_from_slice(&frame.seq.to_be_bytes());
            aad[8..12].copy_from_slice(&frame.page_no.to_be_bytes());
            aad[12] = u8::from(frame.is_commit);
            let plain = gcm.decrypt_detached(&frame.nonce, &aad, &frame.ciphertext, &frame.tag)
                .map_err(|_| EncryptedStoreError::WalFrameAuthenticationFailed(frame.seq))?;
            pending.push((frame.page_no, plain));
            if frame.is_commit {
                for (pno, bytes) in pending.drain(..) { committed.insert(pno, bytes); }
            }
        }
        Ok((committed, pending.len()))
    }
}

#[rustfmt::skip]
pub fn encrypt_safe_storage_token(plain: &str, keys: &DerivedStoreKeys, nonce: &[u8; 12]) -> String {
    let (ct, tag) = Aes256Gcm::new(&keys.token_key).encrypt_detached(nonce, SAFE_STORAGE_AAD, plain.as_bytes());
    let mut combined = Vec::with_capacity(12 + ct.len() + 16);
    combined.extend_from_slice(nonce);
    combined.extend_from_slice(&ct);
    combined.extend_from_slice(&tag);
    format!("{ENCRYPTED_TOKEN_PREFIX}{}", encode_base64(&combined))
}

#[rustfmt::skip]
pub fn redact_sensitive_log(input: &str) -> String {
    input.split_whitespace().map(|tok| {
        if tok.starts_with(ENCRYPTED_TOKEN_PREFIX) { "dQw4w9WgXcQ:***" }
        else if tok.starts_with("mfa.") { "mfa.***" }
        else if tok.starts_with("key=") || tok.starts_with("master_key=") { "key=[REDACTED_KEY]" }
        else { tok }
    }).collect::<Vec<_>>().join(" ")
}

#[rustfmt::skip]
pub fn validate_snowflake_id(id: &str) -> Result<u64, EncryptedStoreError> {
    id.parse::<u64>().ok().filter(|_| !id.starts_with('+')).ok_or(EncryptedStoreError::BoundaryViolation("snowflake"))
}
#[rustfmt::skip]
pub fn validate_message_content(content: &str) -> Result<(), EncryptedStoreError> {
    (!content.is_empty() && !content.contains('\0') && content.chars().count() <= MAX_MESSAGE_CHARS).then_some(()).ok_or(EncryptedStoreError::BoundaryViolation("message"))
}
#[rustfmt::skip]
pub fn validate_attachments(count: usize, max_bytes: u64) -> Result<(), EncryptedStoreError> {
    (count <= MAX_ATTACHMENTS_COUNT && max_bytes <= MAX_ATTACHMENT_BYTES).then_some(()).ok_or(EncryptedStoreError::BoundaryViolation("attachments"))
}
#[rustfmt::skip]
pub fn validate_search_pagination(offset: u32, limit: u32) -> Result<(), EncryptedStoreError> {
    (limit > 0 && offset <= MAX_SEARCH_OFFSET && offset.saturating_add(limit) <= MAX_SEARCH_WINDOW).then_some(()).ok_or(EncryptedStoreError::BoundaryViolation("search"))
}
#[rustfmt::skip]
pub fn validate_volume_percent(vol: u16) -> Result<u16, EncryptedStoreError> {
    (vol <= MAX_VOLUME_PERCENT).then_some(vol).ok_or(EncryptedStoreError::BoundaryViolation("volume"))
}
#[rustfmt::skip]
pub fn format_unread_badge(unread: u32) -> String {
    if unread > MAX_BADGE_CAP { "99+".into() } else { unread.to_string() }
}
#[rustfmt::skip]
pub fn check_channel_read_access(perms: u64, nsfw: bool, verified: bool) -> Result<(), EncryptedStoreError> {
    let req = PERM_VIEW_CHANNEL | PERM_READ_MESSAGE_HISTORY;
    ((perms & req) == req && (!nsfw || verified)).then_some(()).ok_or(EncryptedStoreError::AccessDenied("denied"))
}
#[rustfmt::skip]
pub fn handle_storage_http_status(keyring: &mut LinuxKeyringClient, status: u16, retry_after_ms: u64) -> Result<(), EncryptedStoreError> {
    match status {
        200 | 204 => Ok(()),
        202 => Err(EncryptedStoreError::IndexingInProgress(retry_after_ms)),
        401 => { keyring.state = KeyringAuthState::Locked; Err(EncryptedStoreError::HttpStatusError(status, 0)) }
        _ => Err(EncryptedStoreError::HttpStatusError(status, retry_after_ms)),
    }
}
#[rustfmt::skip]
pub fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes { out.push(HEX[(b >> 4) as usize] as char); out.push(HEX[(b & 0x0f) as usize] as char); }
    out
}
#[rustfmt::skip]
pub fn decode_hex(hex: &str) -> Result<Vec<u8>, EncryptedStoreError> {
    if !hex.len().is_multiple_of(2) { return Err(EncryptedStoreError::InvalidKeyringAttribute); }
    let nib = |b: u8| match b {
        b'0'..=b'9' => Ok(b - b'0'), b'a'..=b'f' => Ok(b - b'a' + 10), b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(EncryptedStoreError::InvalidKeyringAttribute),
    };
    hex.as_bytes().chunks_exact(2).map(|p| Ok((nib(p[0])? << 4) | nib(p[1])?)).collect()
}
#[rustfmt::skip]
pub fn encode_base64(input: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for c in input.chunks(3) {
        let (b0, b1, b2) = (c[0], if c.len() > 1 { c[1] } else { 0 }, if c.len() > 2 { c[2] } else { 0 });
        out.push(T[(b0 >> 2) as usize] as char);
        out.push(T[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        out.push(if c.len() > 1 { T[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char } else { '=' });
        out.push(if c.len() > 2 { T[(b2 & 0x3f) as usize] as char } else { '=' });
    }
    out
}

#[cfg(test)]
#[rustfmt::skip]
mod tests {
    use super::*;

    const SAMPLE_MASTER_HEX: &str = "4f9c2e7b11d4a8053c6e91f02a74b8c3d5e1096a4f8b22c7e0139d58a67b4c2e";
    const SAMPLE_SALT: [u8; SALT_SIZE] = [0xa1, 0xb2, 0xc3, 0xd4, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x10];

    #[test]
    fn test_sub_984_1_constants_tokens_states_and_oracles() {
        assert_eq!((TASK_ID, LAYOUT_CONTAINER, CIPHER_DIMENSIONS), ("TASK-984", "Encrypted Store", "AES-256-GCM"));
        assert_eq!([TOKEN_BG, TOKEN_SURFACE, TOKEN_TEXT_PRIMARY, TOKEN_ACCENT, TOKEN_BORDER], ["#1e1f22", "#2b2d31", "#ffffff", "#5865f2", "#3f4147"]);
        assert_eq!([KeyringAuthState::resolve(false, false).as_str(), KeyringAuthState::resolve(false, true).as_str(), KeyringAuthState::resolve(true, false).as_str()], ["locked", "unlocking", "decrypted"]);
        assert_eq!((SECRET_SERVICE_BUS_NAME, KEYRING_SCHEMA_NAME, ENCRYPTED_TOKEN_PREFIX, SQLITE_PAGE_SIZE), ("org.freedesktop.secrets", "com.discordlab.Token", "dQw4w9WgXcQ:", 4096));
    }

    #[test]
    fn test_sub_984_2_keyring_hkdf_safe_storage_and_redaction() {
        let mut keyring = LinuxKeyringClient::new_locked();
        assert_eq!(keyring.state, KeyringAuthState::Locked);
        keyring.store_secret_hex(KEYRING_MASTER_KEY_ATTR, SAMPLE_MASTER_HEX).unwrap();
        let (master, derived) = keyring.unlock_and_derive_keys(KEYRING_MASTER_KEY_ATTR, &SAMPLE_SALT).unwrap();
        assert_eq!(keyring.state, KeyringAuthState::Decrypted);
        let dbg_master = format!("{master:?} | {derived:?}");
        assert!(!dbg_master.contains("4f9c2e7b") && dbg_master.contains("KeyringMasterSecret(<скрыто>)"));
        let plain_tok = "discord_session_secret_v1_payload";
        let safe_tok = encrypt_safe_storage_token(plain_tok, &derived, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
        assert!(safe_tok.starts_with(ENCRYPTED_TOKEN_PREFIX));
        let redacted = redact_sensitive_log(&format!("auth ok {safe_tok} mfa.secret123456 master_key={SAMPLE_MASTER_HEX}"));
        let log_leaked = redacted.contains(&safe_tok) || redacted.contains(SAMPLE_MASTER_HEX);
        assert!(!log_leaked);
        println!("SUB_984_2_ORACLE_JSON:{{\"master_key_hex\":\"{}\",\"salt_hex\":\"{}\",\"prk_hex\":\"{}\",\"page_key_hex\":\"{}\",\"wal_key_hex\":\"{}\",\"token_key_hex\":\"{}\",\"header_mac_key_hex\":\"{}\",\"safe_storage_token\":\"{}\",\"expected_plain_token\":\"{}\",\"debug_leaked_secret\":{},\"log_leaked_secret\":{},\"redacted_log\":\"{}\"}}", SAMPLE_MASTER_HEX, encode_hex(&SAMPLE_SALT), encode_hex(&derived.prk), encode_hex(&derived.page_key), encode_hex(&derived.wal_key), encode_hex(&derived.token_key), encode_hex(&derived.header_mac_key), safe_tok, plain_tok, dbg_master.contains("4f9c2e7b"), log_leaked, redacted);
    }

    #[test]
    fn test_sub_984_3_page_encryption_tamper_and_swap_detection() {
        let master = KeyringMasterSecret::from_hex(SAMPLE_MASTER_HEX).unwrap();
        let keys = DerivedStoreKeys::derive(&master, &SAMPLE_SALT);
        assert_ne!(derive_page_nonce(&SAMPLE_SALT, 1, 1), derive_page_nonce(&SAMPLE_SALT, 2, 1));
        let mut two_pages = vec![0u8; SQLITE_PAGE_SIZE * 2];
        two_pages[0..16].copy_from_slice(SQLITE_PLAIN_MAGIC);
        two_pages[SQLITE_PAGE_SIZE..SQLITE_PAGE_SIZE + 12].copy_from_slice(b"PAGE_TWO_MSG");
        let mut enc = encrypt_sqlite_bytes(&two_pages, &keys, &SAMPLE_SALT).unwrap();
        assert_eq!(decrypt_sqlite_bytes(&enc, &keys).unwrap(), two_pages);
        let (r1, r2) = (CONTAINER_HEADER_BYTES, CONTAINER_HEADER_BYTES + ENCRYPTED_PAGE_RECORD_BYTES);
        let mut swapped = enc.clone();
        for i in 8..ENCRYPTED_PAGE_RECORD_BYTES { swapped.swap(r1 + i, r2 + i); }
        assert_eq!(decrypt_sqlite_bytes(&swapped, &keys), Err(EncryptedStoreError::PageAuthenticationFailed(1)));
        let last_idx = enc.len() - 1;
        enc[last_idx] ^= 0x01;
        assert_eq!(decrypt_sqlite_bytes(&enc, &keys), Err(EncryptedStoreError::PageAuthenticationFailed(2)));
        if let (Ok(plain_p), Ok(enc_p), Ok(dec_p)) = (std::env::var("TASK_984_PLAIN_DB"), std::env::var("TASK_984_ENC_DB"), std::env::var("TASK_984_DEC_DB")) {
            let plain = std::fs::read(&plain_p).unwrap();
            let enc_blob = encrypt_sqlite_bytes(&plain, &keys, &SAMPLE_SALT).unwrap();
            std::fs::write(&enc_p, &enc_blob).unwrap();
            let dec_blob = decrypt_sqlite_bytes(&enc_blob, &keys).unwrap();
            std::fs::write(&dec_p, &dec_blob).unwrap();
            println!("SUB_984_3_META_JSON:{{\"page_key_hex\":\"{}\",\"pages\":{}}}", encode_hex(&keys.page_key), plain.len() / SQLITE_PAGE_SIZE);
        }
    }

    #[test]
    fn test_sub_984_4_nine_dimensions_and_wal_power_loss_recovery() {
        let mut keyring = LinuxKeyringClient::new_locked();
        keyring.store_secret_hex(KEYRING_MASTER_KEY_ATTR, SAMPLE_MASTER_HEX).unwrap();
        let (_master, keys) = keyring.unlock_and_derive_keys(KEYRING_MASTER_KEY_ATTR, &SAMPLE_SALT).unwrap();
        assert!(KeyringMasterSecret::from_bytes(&[0u8; 32]).is_err());
        assert_eq!((validate_snowflake_id("0").unwrap(), validate_snowflake_id("18446744073709551615").unwrap()), (0, u64::MAX));
        assert!(validate_snowflake_id("18446744073709551616").is_err());
        assert!(validate_message_content(&"А".repeat(2000)).is_ok() && validate_message_content(&"А".repeat(2001)).is_err());
        assert!(validate_attachments(10, 25 * 1024 * 1024).is_ok() && validate_attachments(11, 1024).is_err());
        assert!(validate_search_pagination(9975, 25).is_ok() && validate_search_pagination(9976, 25).is_err());
        assert_eq!(validate_volume_percent(200).unwrap(), 200);
        assert!(validate_volume_percent(201).is_err());
        assert_eq!((format_unread_badge(99), format_unread_badge(100)), ("99".into(), "99+".into()));
        assert_eq!(handle_storage_http_status(&mut keyring, 202, 500), Err(EncryptedStoreError::IndexingInProgress(500)));
        assert!(check_channel_read_access(PERM_VIEW_CHANNEL | PERM_READ_MESSAGE_HISTORY, true, false).is_err());
        assert!(check_channel_read_access(PERM_VIEW_CHANNEL | PERM_READ_MESSAGE_HISTORY, true, true).is_ok());
        let mut wal = EncryptedWalJournal::new(SAMPLE_SALT);
        wal.append_frame(&keys, 1, &vec![0x11u8; SQLITE_PAGE_SIZE], false).unwrap();
        wal.append_frame(&keys, 2, &vec![0x22u8; SQLITE_PAGE_SIZE], true).unwrap();
        wal.append_frame(&keys, 3, &vec![0x33u8; SQLITE_PAGE_SIZE], false).unwrap();
        let (committed, discarded) = wal.recover_after_power_loss(&keys).unwrap();
        assert_eq!((committed.len(), discarded, committed.get(&3)), (2, 1, None));
        println!("SUB_984_4_AUDIT_JSON:{{\"task_id\":\"TASK-984\",\"cipher\":\"AES-256-GCM\",\"page_size\":4096,\"keyring_state\":\"{}\",\"wal_recovered_frames\":{},\"wal_discarded_uncommitted\":{},\"secret_leaks\":0}}", keyring.state.as_str(), committed.len(), discarded);
    }
}
