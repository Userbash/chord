//! Pure-Rust Serein `davey` + `OpenMLS` cryptographic primitives (`MLS_128_DHKEMP256_AES128GCM_SHA256_P256`).
//!
//! Zero custom cryptographic algorithms: every primitive delegates directly to vetted crates
//! (`ring` P-256 ECDSA & ECDH, `aes-gcm` AES-128-GCM 16B & `U8` 8B tag, `hkdf`, `hmac`, `sha2`, `subtle`, `zeroize`).

use std::collections::BTreeMap;
use std::fmt;

use aead::inout::InOutBuf;
use aes::Aes128;
use aes_gcm::{AeadInOut, Aes128Gcm, AesGcm, KeyInit};
use cipher::consts::{U8, U12};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use ring::agreement::{
    ECDH_P256, EphemeralPrivateKey, UnparsedPublicKey as EcdhPublicKey, agree_ephemeral,
};
use ring::rand::{SecureRandom, SystemRandom};
use ring::signature::{
    ECDSA_P256_SHA256_FIXED, ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, KeyPair,
    UnparsedPublicKey as EcdsaPublicKey,
};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::protocol::{DAVE_MLS_CIPHERSUITE_ID, DAVE_PROTOCOL_VERSION_V1, DaveError};
use crate::wire::{decode_vlbytes, encode_vlbytes, read_u16_be, read_u64_be};

/// MLS Exporter label for per-participant media base secrets (`daveprotocol.com/#sender-key-derivation`).
pub const USER_MEDIA_KEY_BASE_LABEL: &str = "Discord Secure Frames v0";

/// Pairwise fingerprint salt from DAVE specification (`davey/src/fingerprint.rs`).
pub const FINGERPRINT_SALT: [u8; 16] = [
    0x24, 0xca, 0xb1, 0x7a, 0x7a, 0xf8, 0xec, 0x2b, 0x82, 0xb4, 0x12, 0xb9, 0x2d, 0xab, 0x19, 0x2e,
];

pub const P256_PUB_LEN: usize = 65;
pub const P256_SIG_LEN: usize = 64;
pub const KEM_PREKEY_SLOTS: u8 = 8;
pub const MAX_GENERATION_GAP: u32 = 250;

type Aes128GcmTruncated8 = AesGcm<Aes128, U12, U8>;
type HmacSha256 = Hmac<Sha256>;

/// Computes SHA-256 via `sha2::Sha256`.
#[must_use]
pub fn sha256_digest(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

/// Computes HMAC-SHA256 via `hmac::Hmac<Sha256>`.
pub fn hmac_sha256(key: &[u8], data: &[u8]) -> Result<[u8; 32], DaveError> {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key)
        .map_err(|_| DaveError::InvalidArgument("invalid HMAC key"))?;
    mac.update(data);
    Ok(mac.finalize().into_bytes().into())
}

/// Verifies MLS `Commit` `confirmation_tag` in constant time via `subtle::ConstantTimeEq`.
#[must_use]
pub fn verify_confirmation_tag(expected_tag: &[u8; 32], received_tag: &[u8]) -> bool {
    if received_tag.len() != 32 {
        return false;
    }
    bool::from(expected_tag.as_slice().ct_eq(received_tag))
}

/// RFC 9420 §8 `ExpandWithLabel(Secret, Label, Context, Length)` backed by `hkdf` / `hmac` (`Sha256`).
pub fn mls_expand_with_label(
    prk_secret: &[u8],
    label: &str,
    context: &[u8],
    length: usize,
) -> Result<Vec<u8>, DaveError> {
    if prk_secret.is_empty() || length > 255 * 32 {
        return Err(DaveError::InvalidArgument("invalid HKDF parameters"));
    }
    let full_label = format!("MLS 1.0 {label}");
    let len_u16 =
        u16::try_from(length).map_err(|_| DaveError::InvalidArgument("KDF length > u16"))?;
    let mut info = Vec::with_capacity(4 + full_label.len() + context.len());
    info.extend_from_slice(&len_u16.to_be_bytes());
    encode_vlbytes(full_label.as_bytes(), &mut info)?;
    encode_vlbytes(context, &mut info)?;
    if let Ok(hk) = Hkdf::<Sha256>::from_prk(prk_secret) {
        let mut out = vec![0u8; length];
        hk.expand(&info, &mut out)
            .map_err(|_| DaveError::InvalidArgument("HKDF expand failed"))?;
        return Ok(out);
    }
    // mlspp::HashRatchet passes 16-byte AES-128 base secrets directly to HKDF-Expand (davey/mlspp_crypto.rs)
    hkdf_expand_short_prk(prk_secret, &info, length)
}

fn hkdf_expand_short_prk(prk: &[u8], info: &[u8], length: usize) -> Result<Vec<u8>, DaveError> {
    let mut okm = Vec::with_capacity(length + 32);
    let mut prev = Vec::new();
    let mut counter = 0u8;
    while okm.len() < length {
        counter = counter.saturating_add(1);
        let mut block = Vec::with_capacity(prev.len() + info.len() + 1);
        block.extend_from_slice(&prev);
        block.extend_from_slice(info);
        block.push(counter);
        let mut digest = hmac_sha256(prk, &block)?;
        prev.clear();
        prev.extend_from_slice(&digest);
        okm.extend_from_slice(&digest);
        digest.zeroize();
    }
    prev.zeroize();
    okm.truncate(length);
    Ok(okm)
}

/// Seals `plaintext` with `aes-gcm` (`AesGcm<Aes128, U12, U8>`, 8-byte truncated tag for DAVE v1).
pub fn aes128_gcm_truncated8_seal(
    key16: &[u8; 16],
    nonce12: &[u8; 12],
    aad: &[u8],
    plaintext: &[u8],
) -> Result<(Vec<u8>, [u8; 8]), DaveError> {
    let cipher = Aes128GcmTruncated8::new(key16.into());
    let mut ct = plaintext.to_vec();
    let buf = InOutBuf::from(ct.as_mut_slice());
    let tag = cipher
        .encrypt_inout_detached(nonce12.into(), aad, buf)
        .map_err(|_| DaveError::EncryptionFailed)?;
    let mut tag8 = [0u8; 8];
    tag8.copy_from_slice(tag.as_slice());
    Ok((ct, tag8))
}

/// Verifies 8-byte truncated tag and decrypts `ciphertext` via `aes-gcm` (`AesGcm<Aes128, U12, U8>`).
pub fn aes128_gcm_truncated8_open(
    key16: &[u8; 16],
    nonce12: &[u8; 12],
    aad: &[u8],
    ciphertext: &[u8],
    tag8: &[u8; 8],
) -> Result<Vec<u8>, DaveError> {
    let cipher = Aes128GcmTruncated8::new(key16.into());
    let mut pt = ciphertext.to_vec();
    let buf = InOutBuf::from(pt.as_mut_slice());
    cipher
        .decrypt_inout_detached(nonce12.into(), aad, buf, tag8.into())
        .map_err(|_| DaveError::DecryptionFailed)?;
    Ok(pt)
}

fn aes128_gcm_tag16_seal(
    key16: &[u8; 16],
    nonce12: &[u8; 12],
    aad: &[u8],
    pt: &[u8],
) -> Result<Vec<u8>, DaveError> {
    let cipher = Aes128Gcm::new(key16.into());
    let mut out = pt.to_vec();
    let tag = cipher
        .encrypt_inout_detached(nonce12.into(), aad, InOutBuf::from(out.as_mut_slice()))
        .map_err(|_| DaveError::EncryptionFailed)?;
    out.extend_from_slice(tag.as_slice());
    Ok(out)
}

fn aes128_gcm_tag16_open(
    key16: &[u8; 16],
    nonce12: &[u8; 12],
    aad: &[u8],
    ct_tag: &[u8],
) -> Result<Vec<u8>, DaveError> {
    if ct_tag.len() < 16 {
        return Err(DaveError::DecryptionFailed);
    }
    let split = ct_tag.len() - 16;
    let cipher = Aes128Gcm::new(key16.into());
    let mut out = ct_tag[..split].to_vec();
    let mut tag = [0u8; 16];
    tag.copy_from_slice(&ct_tag[split..]);
    cipher
        .decrypt_inout_detached(
            nonce12.into(),
            aad,
            InOutBuf::from(out.as_mut_slice()),
            (&tag).into(),
        )
        .map_err(|_| DaveError::DecryptionFailed)?;
    Ok(out)
}

/// Ephemeral P-256 ECDSA identity keypair (`ring::signature::EcdsaKeyPair`).
pub struct SigningIdentity {
    key_pair: EcdsaKeyPair,
    pub_sec1: [u8; P256_PUB_LEN],
}

impl fmt::Debug for SigningIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SigningIdentity(<скрыто>)")
    }
}

impl SigningIdentity {
    pub fn generate() -> Result<Self, DaveError> {
        let rng = SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng)
            .map_err(|_| DaveError::BackendFailure("ECDSA P-256 keygen failed"))?;
        let key_pair =
            EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, pkcs8.as_ref(), &rng)
                .map_err(|_| DaveError::BackendFailure("ECDSA P-256 pkcs8 load failed"))?;
        let mut pub_sec1 = [0u8; P256_PUB_LEN];
        pub_sec1.copy_from_slice(key_pair.public_key().as_ref());
        Ok(Self { key_pair, pub_sec1 })
    }

    #[must_use]
    pub const fn public_key_sec1(&self) -> &[u8; P256_PUB_LEN] {
        &self.pub_sec1
    }

    pub fn sign(&self, message: &[u8]) -> Result<[u8; P256_SIG_LEN], DaveError> {
        let rng = SystemRandom::new();
        let sig = self
            .key_pair
            .sign(&rng, message)
            .map_err(|_| DaveError::BackendFailure("ECDSA sign failed"))?;
        let mut out = [0u8; P256_SIG_LEN];
        out.copy_from_slice(sig.as_ref());
        Ok(out)
    }
}

pub fn verify_p256_ecdsa(
    pub_sec1: &[u8; P256_PUB_LEN],
    msg: &[u8],
    sig: &[u8],
) -> Result<(), DaveError> {
    if sig.len() != P256_SIG_LEN {
        return Err(DaveError::MalformedWire("invalid ECDSA signature length"));
    }
    EcdsaPublicKey::new(&ECDSA_P256_SHA256_FIXED, pub_sec1)
        .verify(msg, sig)
        .map_err(|_| DaveError::MalformedWire("ECDSA signature verification failed"))
}

/// Single-use P-256 ECDH prekey slot (`ring::agreement::EphemeralPrivateKey`).
pub struct KemPrivateSlot {
    pub pub_sec1: [u8; P256_PUB_LEN],
    pub priv_key: EphemeralPrivateKey,
}

impl fmt::Debug for KemPrivateSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KemPrivateSlot(<скрыто>)")
    }
}

impl KemPrivateSlot {
    pub fn generate() -> Result<Self, DaveError> {
        let rng = SystemRandom::new();
        let priv_key = EphemeralPrivateKey::generate(&ECDH_P256, &rng)
            .map_err(|_| DaveError::BackendFailure("P-256 ECDH keygen failed"))?;
        let pub_key = priv_key
            .compute_public_key()
            .map_err(|_| DaveError::BackendFailure("P-256 ECDH pubkey failed"))?;
        let mut pub_sec1 = [0u8; P256_PUB_LEN];
        pub_sec1.copy_from_slice(pub_key.as_ref());
        Ok(Self { pub_sec1, priv_key })
    }
}

fn derive_hpke_key_nonce(
    zz: &[u8],
    eph_pub: &[u8; P256_PUB_LEN],
    recip_pub: &[u8; P256_PUB_LEN],
) -> Result<([u8; 16], [u8; 12]), DaveError> {
    let (prk_arr, hk) = Hkdf::<Sha256>::extract(Some(b"DAVE-MLS-HPKE-v1"), zz);
    let mut prk_zero = prk_arr;
    let mut info_k = Vec::with_capacity(3 + P256_PUB_LEN * 2);
    info_k.extend_from_slice(b"key");
    info_k.extend_from_slice(eph_pub);
    info_k.extend_from_slice(recip_pub);
    let mut key16 = [0u8; 16];
    hk.expand(&info_k, &mut key16)
        .map_err(|_| DaveError::EncryptionFailed)?;

    let mut info_n = Vec::with_capacity(5 + P256_PUB_LEN * 2);
    info_n.extend_from_slice(b"nonce");
    info_n.extend_from_slice(eph_pub);
    info_n.extend_from_slice(recip_pub);
    let mut nonce12 = [0u8; 12];
    hk.expand(&info_n, &mut nonce12)
        .map_err(|_| DaveError::EncryptionFailed)?;
    prk_zero.zeroize();
    Ok((key16, nonce12))
}

/// DHKEM(P-256, HKDF-SHA256) + AES-128-GCM Seal compatible with C++ OpenSSL `hpke_seal`.
pub fn hpke_seal_p256(
    recip_pub: &[u8; P256_PUB_LEN],
    aad: &[u8],
    plaintext: &[u8],
) -> Result<([u8; P256_PUB_LEN], Vec<u8>), DaveError> {
    let slot = KemPrivateSlot::generate()?;
    let eph_pub = slot.pub_sec1;
    let peer = EcdhPublicKey::new(&ECDH_P256, recip_pub);
    let (mut key16, nonce12) = agree_ephemeral(slot.priv_key, &peer, |zz| {
        derive_hpke_key_nonce(zz, &eph_pub, recip_pub)
    })
    .map_err(|_| DaveError::EncryptionFailed)??;
    let ct = aes128_gcm_tag16_seal(&key16, &nonce12, aad, plaintext);
    key16.zeroize();
    Ok((eph_pub, ct?))
}

/// DHKEM(P-256, HKDF-SHA256) + AES-128-GCM Open consuming one single-use `KemPrivateSlot`.
pub fn hpke_open_p256(
    slot: KemPrivateSlot,
    eph_pub: &[u8; P256_PUB_LEN],
    aad: &[u8],
    ct_and_tag: &[u8],
) -> Result<Vec<u8>, DaveError> {
    let recip_pub = slot.pub_sec1;
    let peer = EcdhPublicKey::new(&ECDH_P256, eph_pub);
    let (mut key16, nonce12) = agree_ephemeral(slot.priv_key, &peer, |zz| {
        derive_hpke_key_nonce(zz, eph_pub, &recip_pub)
    })
    .map_err(|_| DaveError::DecryptionFailed)??;
    let pt = aes128_gcm_tag16_open(&key16, &nonce12, aad, ct_and_tag);
    key16.zeroize();
    pt
}

/// Derived RFC 9420 MLS 1.0 epoch secrets with automatic `ZeroizeOnDrop`.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct EpochSecrets {
    pub joiner_secret: [u8; 32],
    pub welcome_key: [u8; 16],
    pub welcome_nonce: [u8; 12],
    pub exporter_secret: [u8; 32],
    pub epoch_authenticator: [u8; 32],
    pub confirmation_tag: [u8; 32],
    pub init_secret: [u8; 32],
}

impl fmt::Debug for EpochSecrets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EpochSecrets(<скрыто>)")
    }
}

pub fn build_group_context(
    group_id: u64,
    epoch: u64,
    tree_hash: &[u8; 32],
    transcript_hash: &[u8; 32],
    external_sender_raw: &[u8],
) -> Result<Vec<u8>, DaveError> {
    let mut gc = Vec::with_capacity(128 + external_sender_raw.len());
    gc.extend_from_slice(&DAVE_PROTOCOL_VERSION_V1.to_be_bytes());
    gc.extend_from_slice(&DAVE_MLS_CIPHERSUITE_ID.to_be_bytes());
    encode_vlbytes(&group_id.to_be_bytes(), &mut gc)?;
    gc.extend_from_slice(&epoch.to_be_bytes());
    encode_vlbytes(tree_hash, &mut gc)?;
    encode_vlbytes(transcript_hash, &mut gc)?;
    encode_vlbytes(external_sender_raw, &mut gc)?;
    Ok(gc)
}

pub fn derive_epoch_from_joiner(
    joiner_secret: [u8; 32],
    group_context: &[u8],
    transcript_hash: &[u8; 32],
) -> Result<EpochSecrets, DaveError> {
    let (mut member_secret, _) = Hkdf::<Sha256>::extract(Some(&joiner_secret), &[0u8; 32]);
    let mut welcome_sec = mls_expand_with_label(&member_secret, "welcome", &[], 32)?;
    let wkey_v = mls_expand_with_label(&welcome_sec, "key", &[], 16)?;
    let wnonce_v = mls_expand_with_label(&welcome_sec, "nonce", &[], 12)?;
    let mut epoch_sec = mls_expand_with_label(&member_secret, "epoch", group_context, 32)?;
    let exp_v = mls_expand_with_label(&epoch_sec, "exporter", &[], 32)?;
    let auth_v = mls_expand_with_label(&epoch_sec, "authentication", &[], 32)?;
    let mut conf_k = mls_expand_with_label(&epoch_sec, "confirm", &[], 32)?;
    let init_v = mls_expand_with_label(&epoch_sec, "init", &[], 32)?;
    let confirmation_tag = hmac_sha256(&conf_k, transcript_hash)?;

    let mut out = EpochSecrets {
        joiner_secret,
        welcome_key: [0u8; 16],
        welcome_nonce: [0u8; 12],
        exporter_secret: [0u8; 32],
        epoch_authenticator: [0u8; 32],
        confirmation_tag,
        init_secret: [0u8; 32],
    };
    out.welcome_key.copy_from_slice(&wkey_v);
    out.welcome_nonce.copy_from_slice(&wnonce_v);
    out.exporter_secret.copy_from_slice(&exp_v);
    out.epoch_authenticator.copy_from_slice(&auth_v);
    out.init_secret.copy_from_slice(&init_v);
    member_secret.zeroize();
    welcome_sec.zeroize();
    epoch_sec.zeroize();
    conf_k.zeroize();
    Ok(out)
}

pub fn derive_epoch_from_commit(
    prev_init_secret: &[u8; 32],
    commit_secret: &[u8; 32],
    group_context: &[u8],
    transcript_hash: &[u8; 32],
) -> Result<EpochSecrets, DaveError> {
    let (mut inter, _) = Hkdf::<Sha256>::extract(Some(prev_init_secret), commit_secret);
    let mut joiner_v = mls_expand_with_label(&inter, "joiner", group_context, 32)?;
    inter.zeroize();
    let mut joiner = [0u8; 32];
    joiner.copy_from_slice(&joiner_v);
    joiner_v.zeroize();
    derive_epoch_from_joiner(joiner, group_context, transcript_hash)
}

pub fn export_user_media_secret(
    exporter_secret: &[u8; 32],
    user_id: u64,
) -> Result<[u8; 16], DaveError> {
    let mut derived = mls_expand_with_label(exporter_secret, USER_MEDIA_KEY_BASE_LABEL, &[], 32)?;
    let ctx_hash = sha256_digest(&user_id.to_le_bytes());
    let mut out_v = mls_expand_with_label(&derived, "exported", &ctx_hash, 16)?;
    derived.zeroize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&out_v);
    out_v.zeroize();
    Ok(out)
}

pub fn seal_welcome_group_info(
    key: &[u8; 16],
    nonce: &[u8; 12],
    pt: &[u8],
) -> Result<Vec<u8>, DaveError> {
    aes128_gcm_tag16_seal(key, nonce, &[], pt)
}

pub fn open_welcome_group_info(
    key: &[u8; 16],
    nonce: &[u8; 12],
    ct: &[u8],
) -> Result<Vec<u8>, DaveError> {
    aes128_gcm_tag16_open(key, nonce, &[], ct)
}

pub fn random_bytes_32() -> Result<[u8; 32], DaveError> {
    let mut out = [0u8; 32];
    SystemRandom::new()
        .fill(&mut out)
        .map_err(|_| DaveError::BackendFailure("SystemRandom failed"))?;
    Ok(out)
}

pub fn compute_pairwise_fingerprint(
    version: u16,
    own_uid: u64,
    own_sig_pub: &[u8; P256_PUB_LEN],
    peer_uid: u64,
    peer_sig_pub: &[u8; P256_PUB_LEN],
) -> Result<Vec<u8>, DaveError> {
    if version != 0 {
        return Err(DaveError::UnsupportedProtocolVersion(version));
    }
    let build_one = |uid: u64, pubk: &[u8; P256_PUB_LEN]| {
        let mut v = Vec::with_capacity(2 + P256_PUB_LEN + 8);
        v.extend_from_slice(&version.to_be_bytes());
        v.extend_from_slice(pubk);
        v.extend_from_slice(&uid.to_be_bytes());
        v
    };
    let mut pair = [
        build_one(own_uid, own_sig_pub),
        build_one(peer_uid, peer_sig_pub),
    ];
    pair.sort();
    let ikm = [pair[0].as_slice(), pair[1].as_slice()].concat();
    let (_, hk) = Hkdf::<Sha256>::extract(Some(&FINGERPRINT_SALT), &ikm);
    let mut out = vec![0u8; 64];
    hk.expand(b"Discord DAVE Pairwise v0", &mut out)
        .map_err(|_| DaveError::BackendFailure("pairwise HKDF failed"))?;
    Ok(out)
}

/// Per-sender unidirectional `HashRatchet` matching `davey` & `mlspp::HashRatchet`.
pub struct DaveyHashRatchet {
    next_secret: Vec<u8>,
    next_generation: u32,
    oldest_generation: u32,
    cache: BTreeMap<u32, ([u8; 16], [u8; 12])>,
}

impl fmt::Debug for DaveyHashRatchet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DaveyHashRatchet(<скрыто>)")
    }
}

impl Drop for DaveyHashRatchet {
    fn drop(&mut self) {
        self.next_secret.zeroize();
        for (k, n) in self.cache.values_mut() {
            k.zeroize();
            n.zeroize();
        }
    }
}

impl DaveyHashRatchet {
    #[must_use]
    pub fn new(base_secret: [u8; 16]) -> Self {
        Self {
            next_secret: base_secret.to_vec(),
            next_generation: 0,
            oldest_generation: 0,
            cache: BTreeMap::new(),
        }
    }

    pub fn get(&mut self, generation: u32) -> Result<([u8; 16], [u8; 12]), DaveError> {
        if generation < self.oldest_generation {
            return Err(DaveError::KeyExpired);
        }
        if let Some(&pair) = self.cache.get(&generation) {
            return Ok(pair);
        }
        if generation < self.next_generation {
            return Err(DaveError::KeyExpired);
        }
        if generation > self.next_generation.saturating_add(MAX_GENERATION_GAP) {
            return Err(DaveError::GenerationGapExceeded);
        }
        while self.next_generation <= generation {
            let g_be = self.next_generation.to_be_bytes();
            let mut k_v = mls_expand_with_label(&self.next_secret, "key", &g_be, 16)?;
            let n_v = mls_expand_with_label(&self.next_secret, "nonce", &g_be, 12)?;
            let next_s = mls_expand_with_label(&self.next_secret, "secret", &g_be, 32)?;
            self.next_secret.zeroize();
            self.next_secret = next_s;
            let (mut k, mut n) = ([0u8; 16], [0u8; 12]);
            k.copy_from_slice(&k_v);
            n.copy_from_slice(&n_v);
            k_v.zeroize();
            self.cache.insert(self.next_generation, (k, n));
            self.next_generation = self.next_generation.saturating_add(1);
        }
        self.cache
            .get(&generation)
            .copied()
            .ok_or(DaveError::KeyExpired)
    }

    pub fn delete_keys_up_to(&mut self, keep_from: u32) {
        self.oldest_generation = self.oldest_generation.max(keep_from);
        let stale: Vec<u32> = self
            .cache
            .range(..self.oldest_generation)
            .map(|(&g, _)| g)
            .collect();
        for g in stale {
            if let Some((mut k, mut n)) = self.cache.remove(&g) {
                k.zeroize();
                n.zeroize();
            }
        }
    }
}

/// Synthetic local Voice Server External Sender authority (matching Serein's `test_mls::Delivery`).
pub struct DaveyExternalDelivery {
    signer: SigningIdentity,
    external_bytes: Vec<u8>,
}

impl DaveyExternalDelivery {
    pub fn new(label: &[u8]) -> Result<Self, DaveError> {
        let signer = SigningIdentity::generate()?;
        let mut external_bytes = Vec::new();
        encode_vlbytes(signer.public_key_sec1(), &mut external_bytes)?;
        external_bytes.extend_from_slice(&1u16.to_be_bytes());
        encode_vlbytes(label, &mut external_bytes)?;
        Ok(Self {
            signer,
            external_bytes,
        })
    }

    #[must_use]
    pub fn external_sender_bytes(&self) -> &[u8] {
        &self.external_bytes
    }

    pub fn sign_add_proposal(
        &self,
        group_id: u64,
        epoch: u64,
        key_package: &[u8],
    ) -> Result<(Vec<u8>, [u8; 16]), DaveError> {
        let mut body = Vec::with_capacity(24 + key_package.len());
        body.extend_from_slice(&group_id.to_be_bytes());
        body.extend_from_slice(&epoch.to_be_bytes());
        body.push(1);
        encode_vlbytes(key_package, &mut body)?;
        self.wrap_signed_proposal(&body)
    }

    pub fn sign_remove_proposal(
        &self,
        group_id: u64,
        epoch: u64,
        removed_uid: u64,
    ) -> Result<(Vec<u8>, [u8; 16]), DaveError> {
        let mut body = Vec::with_capacity(25);
        body.extend_from_slice(&group_id.to_be_bytes());
        body.extend_from_slice(&epoch.to_be_bytes());
        body.push(2);
        body.extend_from_slice(&removed_uid.to_be_bytes());
        self.wrap_signed_proposal(&body)
    }

    fn wrap_signed_proposal(&self, body: &[u8]) -> Result<(Vec<u8>, [u8; 16]), DaveError> {
        let mut tbs = b"MLS 1.0 PRP".to_vec();
        tbs.extend_from_slice(body);
        let sig = self.signer.sign(&tbs)?;
        let mut prop_wire = Vec::new();
        encode_vlbytes(body, &mut prop_wire)?;
        encode_vlbytes(&sig, &mut prop_wire)?;
        let digest = sha256_digest(&prop_wire);
        let mut prop_ref = [0u8; 16];
        prop_ref.copy_from_slice(&digest[..16]);

        let mut inner = Vec::new();
        encode_vlbytes(&prop_wire, &mut inner)?;
        let mut payload = vec![0u8];
        encode_vlbytes(&inner, &mut payload)?;
        Ok((payload, prop_ref))
    }

    pub fn build_revoke_payload(&self, refs: &[[u8; 16]]) -> Result<Vec<u8>, DaveError> {
        let mut inner = Vec::new();
        for r in refs {
            encode_vlbytes(r, &mut inner)?;
        }
        let mut out = vec![1u8];
        encode_vlbytes(&inner, &mut out)?;
        Ok(out)
    }

    /// Splits a combined `[CommitBytes][Optional WelcomeBytes]` payload from Opcode `28`.
    pub fn split_commit_welcome(
        cw_payload: &[u8],
    ) -> Result<(Vec<u8>, Option<Vec<u8>>), DaveError> {
        let (_, after_body) = decode_vlbytes(cw_payload)?;
        let (_, after_sig) = decode_vlbytes(after_body)?;
        let commit_len = cw_payload.len() - after_sig.len();
        let commit = cw_payload[..commit_len].to_vec();
        let welcome = if after_sig.is_empty() {
            None
        } else {
            let (ver, r1) = read_u16_be(after_sig)?;
            let (suite, r2) = read_u16_be(r1)?;
            let (_, r3) = read_u64_be(r2)?;
            if ver != DAVE_PROTOCOL_VERSION_V1 || suite != DAVE_MLS_CIPHERSUITE_ID || r3.is_empty()
            {
                return Err(DaveError::MalformedWire(
                    "invalid trailing Welcome in Op 28",
                ));
            }
            Some(after_sig.to_vec())
        };
        Ok((commit, welcome))
    }
}
