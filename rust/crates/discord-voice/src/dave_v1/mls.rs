//! Примитивы RFC 9420 MLS 1.0 для DAVE v1: SHA-256, HKDF-SHA256, `VLBytes`, `GroupContext`,
//! `KeyScheduleEpoch` и `DaveHashRatchet`.

use super::{
    DAVE_MAX_GENERATION_GAP, DAVE_MLS_CIPHERSUITE_ID, DAVE_USER_MEDIA_KEY_BASE_LABEL, DaveError,
    ct_eq, zeroize_slice,
};
use crate::crypto::{AES128_KEY_SIZE, GCM_NONCE_SIZE};
use std::collections::BTreeMap;
use std::fmt;

const MAX_VLBYTES_DECODE_LEN: usize = 65_536;

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

#[derive(Clone)]
pub(crate) struct Sha256State {
    state: [u32; 8],
    buffer: [u8; 64],
    buffer_len: usize,
    total_len: u64,
}

impl Sha256State {
    pub(crate) fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buffer: [0u8; 64],
            buffer_len: 0,
            total_len: 0,
        }
    }

    fn compress_block(state: &mut [u32; 8], block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for (i, &chunk) in block.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(chunk);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(SHA256_K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (s, add) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *s = s.wrapping_add(add);
        }
    }

    pub(crate) fn update(&mut self, mut data: &[u8]) {
        self.total_len = self.total_len.wrapping_add(data.len() as u64);
        if self.buffer_len > 0 {
            let needed = 64 - self.buffer_len;
            if data.len() < needed {
                self.buffer[self.buffer_len..self.buffer_len + data.len()].copy_from_slice(data);
                self.buffer_len += data.len();
                return;
            }
            self.buffer[self.buffer_len..64].copy_from_slice(&data[..needed]);
            Self::compress_block(&mut self.state, &self.buffer);
            self.buffer_len = 0;
            data = &data[needed..];
        }
        let (blocks, rem) = data.as_chunks::<64>();
        for block in blocks {
            Self::compress_block(&mut self.state, block);
        }
        if !rem.is_empty() {
            self.buffer[..rem.len()].copy_from_slice(rem);
            self.buffer_len = rem.len();
        }
    }

    pub(crate) fn finalize(mut self) -> [u8; 32] {
        let bit_len = self.total_len.wrapping_mul(8);
        self.buffer[self.buffer_len] = 0x80;
        self.buffer_len += 1;
        if self.buffer_len > 56 {
            self.buffer[self.buffer_len..64].fill(0);
            Self::compress_block(&mut self.state, &self.buffer);
            self.buffer = [0u8; 64];
            self.buffer_len = 0;
        } else {
            self.buffer[self.buffer_len..56].fill(0);
        }
        self.buffer[56..64].copy_from_slice(&bit_len.to_be_bytes());
        Self::compress_block(&mut self.state, &self.buffer);
        let mut out = [0u8; 32];
        for (i, word) in self.state.iter().enumerate() {
            out[i * 4..(i + 1) * 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }
}

/// Вычисляет SHA-256 (FIPS 180-4).
#[must_use]
pub fn dave_sha256(data: &[u8]) -> [u8; 32] {
    let mut h = Sha256State::new();
    h.update(data);
    h.finalize()
}

/// Вычисляет HMAC-SHA256 (RFC 2104).
#[must_use]
pub fn dave_hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut key_block = [0u8; 64];
    if key.len() > 64 {
        let digest = dave_sha256(key);
        key_block[..32].copy_from_slice(&digest);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5cu8; 64];
    for i in 0..64 {
        ipad[i] ^= key_block[i];
        opad[i] ^= key_block[i];
    }
    zeroize_slice(&mut key_block);

    let mut inner = Sha256State::new();
    inner.update(&ipad);
    inner.update(data);
    let inner_digest = inner.finalize();
    zeroize_slice(&mut ipad);

    let mut outer = Sha256State::new();
    outer.update(&opad);
    outer.update(&inner_digest);
    let out = outer.finalize();
    zeroize_slice(&mut opad);
    out
}

/// `HKDF-Extract(salt, IKM)` на SHA-256 (RFC 5869 §2.2).
#[must_use]
pub fn dave_hkdf_extract_sha256(salt: &[u8], ikm: &[u8]) -> [u8; 32] {
    let zero_salt = [0u8; 32];
    dave_hmac_sha256(if salt.is_empty() { &zero_salt } else { salt }, ikm)
}

/// `HKDF-Expand(PRK, info, L)` на SHA-256 (RFC 5869 §2.3).
pub fn dave_hkdf_expand_sha256(
    prk: &[u8],
    info: &[u8],
    out_len: usize,
) -> Result<Vec<u8>, DaveError> {
    if prk.is_empty() {
        return Err(DaveError::InvalidKeyLength {
            expected: AES128_KEY_SIZE,
            actual: 0,
        });
    }
    if out_len > 255 * 32 {
        return Err(DaveError::InvalidOutputLength(out_len));
    }
    if out_len == 0 {
        return Ok(Vec::new());
    }
    let mut okm = Vec::with_capacity(out_len);
    let mut prev_t = [0u8; 32];
    let mut has_prev = false;
    let mut counter = 0u8;
    while okm.len() < out_len {
        counter = counter
            .checked_add(1)
            .ok_or(DaveError::InvalidOutputLength(out_len))?;
        let mut msg = Vec::with_capacity(usize::from(has_prev) * 32 + info.len() + 1);
        if has_prev {
            msg.extend_from_slice(&prev_t);
        }
        msg.extend_from_slice(info);
        msg.push(counter);
        prev_t = dave_hmac_sha256(prk, &msg);
        has_prev = true;
        let take = (out_len - okm.len()).min(32);
        okm.extend_from_slice(&prev_t[..take]);
    }
    zeroize_slice(&mut prev_t);
    Ok(okm)
}

/// Кодирует срез переменной длины MLS `VLBytes` по RFC 9420 §2.1.2.
pub fn mls_encode_vlbytes(data: &[u8], out: &mut Vec<u8>) -> Result<(), DaveError> {
    let len = data.len();
    if len < 64 {
        out.push(len as u8);
    } else if len < 16_384 {
        out.extend_from_slice(&(0x4000u16 | (len as u16)).to_be_bytes());
    } else if len <= 0x3FFF_FFFF {
        out.extend_from_slice(&(0x8000_0000u32 | (len as u32)).to_be_bytes());
    } else {
        return Err(DaveError::VlBytesTooLarge(len));
    }
    out.extend_from_slice(data);
    Ok(())
}

/// Декодирует срез `VLBytes` по RFC 9420 §2.1.2 со строгой проверкой минимальной длины префикса.
pub fn mls_decode_vlbytes(input: &[u8]) -> Result<(&[u8], usize), DaveError> {
    let (&first, _) = input
        .split_first()
        .ok_or(DaveError::MalformedFrame("truncated VLBytes header"))?;
    let (hdr_len, len) = match first >> 6 {
        0 => (1usize, usize::from(first & 0x3F)),
        1 => {
            if input.len() < 2 {
                return Err(DaveError::MalformedFrame("truncated 2-byte VLBytes header"));
            }
            let len = (usize::from(first & 0x3F) << 8) | usize::from(input[1]);
            if len < 64 {
                return Err(DaveError::MalformedFrame("non-minimal 2-byte VLBytes"));
            }
            (2, len)
        }
        2 => {
            if input.len() < 4 {
                return Err(DaveError::MalformedFrame("truncated 4-byte VLBytes header"));
            }
            let raw = u32::from_be_bytes([first & 0x3F, input[1], input[2], input[3]]) as usize;
            if raw < 16_384 {
                return Err(DaveError::MalformedFrame("non-minimal 4-byte VLBytes"));
            }
            if raw > MAX_VLBYTES_DECODE_LEN {
                return Err(DaveError::VlBytesTooLarge(raw));
            }
            (4, raw)
        }
        _ => {
            return Err(DaveError::MalformedFrame(
                "invalid 0b11 VLBytes length prefix",
            ));
        }
    };
    let end = hdr_len
        .checked_add(len)
        .ok_or(DaveError::MalformedFrame("VLBytes length overflow"))?;
    if input.len() < end {
        return Err(DaveError::MalformedFrame("truncated VLBytes payload"));
    }
    Ok((&input[hdr_len..end], end))
}

/// `ExpandWithLabel(Secret, Label, Context, Length)` по RFC 9420 §8.
pub fn mls_expand_with_label(
    secret: &[u8],
    label: &str,
    context: &[u8],
    length: u16,
) -> Result<Vec<u8>, DaveError> {
    let mut full_label = Vec::with_capacity(8 + label.len());
    full_label.extend_from_slice(b"MLS 1.0 ");
    full_label.extend_from_slice(label.as_bytes());

    let mut kdf_label = Vec::with_capacity(6 + full_label.len() + context.len());
    kdf_label.extend_from_slice(&length.to_be_bytes());
    mls_encode_vlbytes(&full_label, &mut kdf_label)?;
    mls_encode_vlbytes(context, &mut kdf_label)?;
    dave_hkdf_expand_sha256(secret, &kdf_label, usize::from(length))
}

fn mls_derive_secret(secret: &[u8], label: &str) -> Result<[u8; 32], DaveError> {
    vec_to_arr32(mls_expand_with_label(secret, label, &[], 32)?)
}

fn mls_derive_tree_secret(
    secret: &[u8],
    label: &str,
    generation: u32,
    length: u16,
) -> Result<Vec<u8>, DaveError> {
    mls_expand_with_label(secret, label, &generation.to_be_bytes(), length)
}

fn vec_to_arr32(v: Vec<u8>) -> Result<[u8; 32], DaveError> {
    let arr: [u8; 32] = v
        .as_slice()
        .try_into()
        .map_err(|_| DaveError::InvalidOutputLength(v.len()))?;
    Ok(arr)
}

/// Контекст группы MLS 1.0 (RFC 9420 §8.1 `GroupContext`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaveMlsGroupContext {
    pub group_id: Vec<u8>,
    pub epoch: u64,
    pub tree_hash: [u8; 32],
    pub confirmed_transcript_hash: [u8; 32],
    pub extensions: Vec<u8>,
}

impl DaveMlsGroupContext {
    /// Сериализует `GroupContext` в проводной формат MLS 1.0.
    pub fn to_mls_bytes(&self) -> Result<Vec<u8>, DaveError> {
        let mut buf = Vec::with_capacity(128 + self.group_id.len() + self.extensions.len());
        buf.extend_from_slice(&1u16.to_be_bytes());
        buf.extend_from_slice(&DAVE_MLS_CIPHERSUITE_ID.to_be_bytes());
        mls_encode_vlbytes(&self.group_id, &mut buf)?;
        buf.extend_from_slice(&self.epoch.to_be_bytes());
        mls_encode_vlbytes(&self.tree_hash, &mut buf)?;
        mls_encode_vlbytes(&self.confirmed_transcript_hash, &mut buf)?;
        mls_encode_vlbytes(&self.extensions, &mut buf)?;
        Ok(buf)
    }

    /// Декодирует `GroupContext` из проводного формата MLS 1.0 с проверкой версии и шифронабора.
    pub fn from_mls_bytes(bytes: &[u8]) -> Result<Self, DaveError> {
        if bytes.len() < 4 {
            return Err(DaveError::MalformedFrame("GroupContext header too short"));
        }
        let version = u16::from_be_bytes([bytes[0], bytes[1]]);
        let suite = u16::from_be_bytes([bytes[2], bytes[3]]);
        if version != 1 || suite != DAVE_MLS_CIPHERSUITE_ID {
            return Err(DaveError::MalformedFrame(
                "unsupported MLS GroupContext version or ciphersuite",
            ));
        }
        let mut cursor = 4usize;
        let (group_id, used) = mls_decode_vlbytes(&bytes[cursor..])?;
        if group_id.is_empty() {
            return Err(DaveError::MalformedFrame("empty MLS group_id"));
        }
        cursor += used;
        if bytes.len() < cursor + 8 {
            return Err(DaveError::MalformedFrame("truncated GroupContext epoch"));
        }
        let mut epoch_bytes = [0u8; 8];
        epoch_bytes.copy_from_slice(&bytes[cursor..cursor + 8]);
        let epoch = u64::from_be_bytes(epoch_bytes);
        cursor += 8;

        let (tree_slice, used) = mls_decode_vlbytes(&bytes[cursor..])?;
        let tree_hash: [u8; 32] = tree_slice
            .try_into()
            .map_err(|_| DaveError::MalformedFrame("GroupContext tree_hash must be 32 bytes"))?;
        cursor += used;

        let (conf_slice, used) = mls_decode_vlbytes(&bytes[cursor..])?;
        let confirmed_transcript_hash: [u8; 32] = conf_slice.try_into().map_err(|_| {
            DaveError::MalformedFrame("GroupContext confirmed_transcript_hash must be 32 bytes")
        })?;
        cursor += used;

        let (extensions, used) = mls_decode_vlbytes(&bytes[cursor..])?;
        cursor += used;
        if cursor != bytes.len() {
            return Err(DaveError::MalformedFrame("trailing bytes in GroupContext"));
        }

        Ok(Self {
            group_id: group_id.to_vec(),
            epoch,
            tree_hash,
            confirmed_transcript_hash,
            extensions: extensions.to_vec(),
        })
    }
}

/// Производные секреты эпохи MLS 1.0 (`KeyScheduleEpoch`, RFC 9420 §8).
#[derive(Clone, PartialEq, Eq)]
pub struct DaveMlsEpochSecrets {
    pub epoch: u64,
    pub joiner_secret: [u8; 32],
    pub welcome_secret: [u8; 32],
    pub epoch_secret: [u8; 32],
    pub exporter_secret: [u8; 32],
    pub epoch_authenticator: [u8; 32],
    pub confirmation_key: [u8; 32],
    pub membership_key: [u8; 32],
    pub init_secret: [u8; 32],
}

impl Drop for DaveMlsEpochSecrets {
    fn drop(&mut self) {
        zeroize_slice(&mut self.joiner_secret);
        zeroize_slice(&mut self.welcome_secret);
        zeroize_slice(&mut self.epoch_secret);
        zeroize_slice(&mut self.exporter_secret);
        zeroize_slice(&mut self.epoch_authenticator);
        zeroize_slice(&mut self.confirmation_key);
        zeroize_slice(&mut self.membership_key);
        zeroize_slice(&mut self.init_secret);
    }
}

impl fmt::Debug for DaveMlsEpochSecrets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DaveMlsEpochSecrets")
            .field("epoch", &self.epoch)
            .field("secrets", &"<скрыто>")
            .finish()
    }
}

impl DaveMlsEpochSecrets {
    /// Деривирует секреты эпохи MLS 1.0 из `joiner_secret` и сериализованного `GroupContext`.
    pub fn from_joiner_secret(
        epoch: u64,
        joiner_secret: [u8; 32],
        group_context_bytes: &[u8],
        psk_secret: Option<&[u8]>,
    ) -> Result<Self, DaveError> {
        let zero_psk = [0u8; 32];
        let mut member_secret =
            dave_hkdf_extract_sha256(&joiner_secret, psk_secret.unwrap_or(&zero_psk));
        let welcome_secret = mls_derive_secret(&member_secret, "welcome")?;
        let epoch_secret = vec_to_arr32(mls_expand_with_label(
            &member_secret,
            "epoch",
            group_context_bytes,
            32,
        )?)?;
        zeroize_slice(&mut member_secret);

        let exporter_secret = mls_derive_secret(&epoch_secret, "exporter")?;
        let epoch_authenticator = mls_derive_secret(&epoch_secret, "authentication")?;
        let confirmation_key = mls_derive_secret(&epoch_secret, "confirm")?;
        let membership_key = mls_derive_secret(&epoch_secret, "membership")?;
        let init_secret = mls_derive_secret(&epoch_secret, "init")?;

        Ok(Self {
            epoch,
            joiner_secret,
            welcome_secret,
            epoch_secret,
            exporter_secret,
            epoch_authenticator,
            confirmation_key,
            membership_key,
            init_secret,
        })
    }

    /// Вычисляет `confirmation_tag = MAC(confirmation_key, confirmed_transcript_hash)` (RFC 9420 §8.2).
    #[must_use]
    pub fn compute_confirmation_tag(&self, confirmed_transcript_hash: &[u8; 32]) -> [u8; 32] {
        dave_hmac_sha256(&self.confirmation_key, confirmed_transcript_hash)
    }

    /// Константно-временная верификация `confirmation_tag`.
    pub fn verify_confirmation_tag(
        &self,
        confirmed_transcript_hash: &[u8; 32],
        candidate_tag: &[u8],
    ) -> Result<(), DaveError> {
        let expected = self.compute_confirmation_tag(confirmed_transcript_hash);
        if ct_eq(&expected, candidate_tag) {
            Ok(())
        } else {
            Err(DaveError::ConfirmationTagMismatch)
        }
    }

    /// `MLS-Exporter(Label, Context, Length)` по RFC 9420 §8.5.
    pub fn mls_export(
        &self,
        label: &str,
        context: &[u8],
        length: u16,
    ) -> Result<Vec<u8>, DaveError> {
        let mut derived = mls_derive_secret(&self.exporter_secret, label)?;
        let ctx_hash = dave_sha256(context);
        let out = mls_expand_with_label(&derived, "exported", &ctx_hash, length)?;
        zeroize_slice(&mut derived);
        Ok(out)
    }

    /// Экспортирует 16-байтный базовый медиа-секрет участника (`"Discord Secure Frames v0"`, LE64 `user_id`).
    pub fn export_user_media_secret(
        &self,
        user_id: u64,
    ) -> Result<[u8; AES128_KEY_SIZE], DaveError> {
        let raw = self.mls_export(
            DAVE_USER_MEDIA_KEY_BASE_LABEL,
            &user_id.to_le_bytes(),
            AES128_KEY_SIZE as u16,
        )?;
        let mut key = [0u8; AES128_KEY_SIZE];
        key.copy_from_slice(&raw);
        Ok(key)
    }
}

/// Машина расписания ключей эпох MLS 1.0 для Discord DAVE v1.
#[derive(Clone)]
pub struct DaveMlsKeySchedule {
    group_context: DaveMlsGroupContext,
    current: DaveMlsEpochSecrets,
}

impl fmt::Debug for DaveMlsKeySchedule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DaveMlsKeySchedule")
            .field("epoch", &self.current.epoch)
            .field("secrets", &"<скрыто>")
            .finish()
    }
}

impl DaveMlsKeySchedule {
    fn derive_epoch(
        init_secret: &[u8; 32],
        commit_secret: &[u8],
        group_context: &DaveMlsGroupContext,
        psk_secret: Option<&[u8]>,
    ) -> Result<DaveMlsEpochSecrets, DaveError> {
        let gc_bytes = group_context.to_mls_bytes()?;
        let mut extracted = dave_hkdf_extract_sha256(init_secret, commit_secret);
        let joiner_secret =
            vec_to_arr32(mls_expand_with_label(&extracted, "joiner", &gc_bytes, 32)?)?;
        zeroize_slice(&mut extracted);
        DaveMlsEpochSecrets::from_joiner_secret(
            group_context.epoch,
            joiner_secret,
            &gc_bytes,
            psk_secret,
        )
    }

    /// Инициализирует расписание ключей начальной эпохи.
    pub fn from_initial_epoch(
        init_secret: [u8; 32],
        commit_secret: &[u8],
        group_context: &DaveMlsGroupContext,
        psk_secret: Option<&[u8]>,
    ) -> Result<Self, DaveError> {
        let current = Self::derive_epoch(&init_secret, commit_secret, group_context, psk_secret)?;
        Ok(Self {
            group_context: group_context.clone(),
            current,
        })
    }

    #[must_use]
    pub fn current_epoch(&self) -> u64 {
        self.current.epoch
    }

    #[must_use]
    pub fn group_context(&self) -> &DaveMlsGroupContext {
        &self.group_context
    }

    #[must_use]
    pub fn current_secrets(&self) -> &DaveMlsEpochSecrets {
        &self.current
    }

    /// Продвигает расписание ключей на следующую эпоху MLS.
    pub fn advance_epoch(
        &mut self,
        commit_secret: &[u8],
        next_group_context: &DaveMlsGroupContext,
        psk_secret: Option<&[u8]>,
    ) -> Result<&DaveMlsEpochSecrets, DaveError> {
        let next_init = self.current.init_secret;
        self.current =
            Self::derive_epoch(&next_init, commit_secret, next_group_context, psk_secret)?;
        self.group_context = next_group_context.clone();
        Ok(&self.current)
    }

    /// Экспортирует персональный хеш-рэтчет отправителя `user_id`.
    pub fn export_sender_ratchet(&self, user_id: u64) -> Result<DaveHashRatchet, DaveError> {
        let mut media_secret = self.current.export_user_media_secret(user_id)?;
        let ratchet = DaveHashRatchet::new(media_secret.to_vec());
        zeroize_slice(&mut media_secret);
        ratchet
    }
}

/// Однонаправленный хеш-рэтчет отправителя MLS 1.0 с защитой от зацикливания на `u32::MAX`.
#[derive(Clone)]
pub struct DaveHashRatchet {
    next_secret: Vec<u8>,
    next_generation: u32,
    oldest_generation: u32,
    exhausted: bool,
    cache: BTreeMap<u32, ([u8; AES128_KEY_SIZE], [u8; GCM_NONCE_SIZE])>,
}

impl Drop for DaveHashRatchet {
    fn drop(&mut self) {
        zeroize_slice(&mut self.next_secret);
        for (k, n) in self.cache.values_mut() {
            zeroize_slice(k);
            zeroize_slice(n);
        }
    }
}

impl fmt::Debug for DaveHashRatchet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DaveHashRatchet")
            .field("next_generation", &self.next_generation)
            .field("oldest_generation", &self.oldest_generation)
            .field("exhausted", &self.exhausted)
            .finish()
    }
}

impl DaveHashRatchet {
    pub fn new(base_secret: impl Into<Vec<u8>>) -> Result<Self, DaveError> {
        let secret = base_secret.into();
        if secret.is_empty() {
            return Err(DaveError::InvalidKeyLength {
                expected: AES128_KEY_SIZE,
                actual: 0,
            });
        }
        Ok(Self {
            next_secret: secret,
            next_generation: 0,
            oldest_generation: 0,
            exhausted: false,
            cache: BTreeMap::new(),
        })
    }

    fn step_once(&mut self) -> Result<(), DaveError> {
        if self.exhausted {
            return Err(DaveError::KeyExpired {
                generation: u32::MAX,
                oldest_available: self.oldest_generation,
            });
        }
        let generation = self.next_generation;
        let key_vec =
            mls_derive_tree_secret(&self.next_secret, "key", generation, AES128_KEY_SIZE as u16)?;
        let nonce_vec = mls_derive_tree_secret(
            &self.next_secret,
            "nonce",
            generation,
            GCM_NONCE_SIZE as u16,
        )?;
        let next_secret = mls_derive_tree_secret(&self.next_secret, "secret", generation, 32)?;

        let mut key = [0u8; AES128_KEY_SIZE];
        key.copy_from_slice(&key_vec);
        let mut nonce = [0u8; GCM_NONCE_SIZE];
        nonce.copy_from_slice(&nonce_vec);

        zeroize_slice(&mut self.next_secret);
        self.next_secret = next_secret;
        if generation == u32::MAX {
            self.exhausted = true;
        } else {
            self.next_generation = generation + 1;
        }
        self.cache.insert(generation, (key, nonce));
        Ok(())
    }

    /// Возвращает ключ и базовый nonce поколения `generation`, продвигая рэтчет не более чем на `250` шагов.
    pub fn get(
        &mut self,
        generation: u32,
    ) -> Result<([u8; AES128_KEY_SIZE], [u8; GCM_NONCE_SIZE]), DaveError> {
        if let Some(&cached) = self.cache.get(&generation) {
            return Ok(cached);
        }
        if self.exhausted || generation < self.next_generation {
            return Err(DaveError::KeyExpired {
                generation,
                oldest_available: self.oldest_generation,
            });
        }
        if generation - self.next_generation > DAVE_MAX_GENERATION_GAP {
            return Err(DaveError::GenerationGapExceeded {
                requested: generation,
                current: self.next_generation,
                max_gap: DAVE_MAX_GENERATION_GAP,
            });
        }
        for _ in self.next_generation..=generation {
            self.step_once()?;
        }
        self.cache
            .get(&generation)
            .copied()
            .ok_or(DaveError::KeyExpired {
                generation,
                oldest_available: self.oldest_generation,
            })
    }

    /// Удаляет ключи поколений строго меньше `up_to_exclusive` (Forward Secrecy).
    pub fn delete_keys_up_to(&mut self, up_to_exclusive: u32) {
        let expired: Vec<u32> = self
            .cache
            .range(..up_to_exclusive)
            .map(|(&g, _)| g)
            .collect();
        for g in expired {
            if let Some((mut k, mut n)) = self.cache.remove(&g) {
                zeroize_slice(&mut k);
                zeroize_slice(&mut n);
            }
        }
        if up_to_exclusive > self.oldest_generation {
            self.oldest_generation = up_to_exclusive;
        }
    }
}
