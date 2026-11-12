//! Сквозное шифрование голоса Discord DAVE MLS v1 (`TASK-903`).
//!
//! Разбито на тематические подмодули без `unsafe` (`#![forbid(unsafe_code)]`):
//! - `replay` — скользящее битовое окно `AntiReplayWindow` (1024 позиции на стеке, `O(1)`)
//!   и защищённая от переполнений развёртка поколений/счётчиков;
//! - `mls` — примитивы RFC 9420 MLS 1.0 (`VLBytes`, `GroupContext`, `KeyScheduleEpoch`, `DaveHashRatchet`);
//! - `session` — кадровый шифр Opus (`0xFAFA`, `Aes128GcmTruncated8`) и многопользовательская
//!   сессия `DaveVoiceSession` с верификацией `Proposal` (op:27) и `Commit` (op:29).

pub mod mls;
pub mod replay;
pub mod session;

pub use mls::{
    DaveHashRatchet, DaveMlsEpochSecrets, DaveMlsGroupContext, DaveMlsKeySchedule,
    dave_hkdf_expand_sha256, dave_hkdf_extract_sha256, dave_hmac_sha256, dave_sha256,
    mls_decode_vlbytes, mls_encode_vlbytes, mls_expand_with_label,
};
pub use replay::{
    AntiReplayWindow, DAVE_GENERATION_WRAP, DAVE_RATCHET_GENERATION_SHIFT_BITS,
    DAVE_REPLAY_WINDOW_SIZE, ReplayError, dave_compute_wrapped_big_nonce,
    dave_compute_wrapped_generation,
};
pub use session::{
    DaveFrameDecryptor, DaveFrameEncryptor, DaveMlsProposal, DaveVoiceSession,
    build_mls_commit_frame, build_mls_proposal_append_frame, build_mls_proposal_revoke_frame,
};

use std::fmt;
use std::hint::black_box;

/// Версия протокола сквозного голосового шифрования Discord DAVE E2EE (`voice-deep.md` §3).
pub const DAVE_PROTOCOL_VERSION_V1: u16 = 1;
/// Идентификатор MLS Ciphersuite для Discord DAVE v1 (`MLS_128_DHKEMP256_AES128GCM_SHA256_P256`).
pub const DAVE_MLS_CIPHERSUITE_ID: u16 = 0x0001;
/// Метка MLS-экспортёра базового секрета отправителя медиа (`chord_voice.node` / `libdave`).
pub const DAVE_USER_MEDIA_KEY_BASE_LABEL: &str = "Discord Secure Frames v0";
/// Магический маркер конца зашифрованного кадра DAVE (`0xFAFA`).
pub const DAVE_MARKER_BYTES: [u8; 2] = [0xFA, 0xFA];
/// Пакет тишины Opus DTX (`[0xF8, 0xFF, 0xFE]`), пропускаемый DAVE прозрачно (`voice-deep.md` §2.3).
pub const DAVE_OPUS_SILENCE_PACKET: [u8; 3] = [0xF8, 0xFF, 0xFE];
/// Максимально допустимый скачок поколения вперёд (`libdave`: `MAX_GENERATION_GAP = 250`).
pub const DAVE_MAX_GENERATION_GAP: u32 = 250;

/// Ошибки криптографического конвейера и MLS-парсера Discord DAVE v1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaveError {
    InvalidKeyLength {
        expected: usize,
        actual: usize,
    },
    InvalidOutputLength(usize),
    VlBytesTooLarge(usize),
    GenerationGapExceeded {
        requested: u32,
        current: u32,
        max_gap: u32,
    },
    KeyExpired {
        generation: u32,
        oldest_available: u32,
    },
    ConfirmationTagMismatch,
    DowngradeRejected {
        current_version: u16,
        requested_version: u16,
    },
    TransitionNotPrepared {
        transition_id: u16,
    },
    MissingKeyRatchet {
        user_id: u64,
    },
    UnknownProposalRef,
    MalformedFrame(&'static str),
    UnencryptedFrameRejected,
    ReplayOrExpiredNonce {
        generation: u32,
        truncated_nonce: u32,
    },
    AeadAuthenticationFailed,
}

impl fmt::Display for DaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for DaveError {}

pub(crate) fn zeroize_slice(buf: &mut [u8]) {
    for byte in buf.iter_mut() {
        *byte = black_box(0);
    }
}

pub(crate) fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
