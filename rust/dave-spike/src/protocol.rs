//! Voice Gateway signaling wire constants (`Op 18..22` & `Op 21..31`), codec/media enums,
//! displayable privacy codes, and typed error domain (`what_to_keep` in `LIB-SIMPLIFY-05`).

use std::fmt;

/// Maximum supported Discord DAVE E2EE protocol version (`voice-deep.md` §1, §3).
pub const DAVE_PROTOCOL_VERSION_V1: u16 = 1;

/// MLS 1.0 ciphersuite ID for Discord DAVE v1 (`MLS_128_DHKEMP256_AES128GCM_SHA256_P256`).
pub const DAVE_MLS_CIPHERSUITE_ID: u16 = 0x0001;

/// Magic 2-byte footer marker terminating every DAVE-encrypted media frame (`0xFAFA`).
pub const DAVE_MAGIC_MARKER: [u8; 2] = [0xFA, 0xFA];

/// Opus DTX comfort-noise / silence packet passed through unencrypted by DAVE v1.
pub const DAVE_OPUS_SILENCE_PACKET: [u8; 3] = [0xF8, 0xFF, 0xFE];

/// Maximum participants allowed in an authenticated DAVE voice group.
pub const MAX_VOICE_PARTICIPANTS: usize = 64;

/// Maximum automatic `op 31` MLS recovery resets before requiring a call rejoin.
pub const MAX_DAVE_RESETS: u8 = 3;

/// Gateway Opcode `18` — Stream Create (`Go Live`).
pub const OP_STREAM_CREATE: u8 = 18;
/// Gateway Opcode `19` — Stream Delete.
pub const OP_STREAM_DELETE: u8 = 19;
/// Gateway Opcode `20` — Stream Watch.
pub const OP_STREAM_WATCH: u8 = 20;
/// Gateway Opcode `21` — Stream Ping (Main Gateway) / DAVE Prepare Transition (Voice Gateway).
pub const OP_STREAM_PING: u8 = 21;
/// Gateway Opcode `22` — Stream Set Paused (Main Gateway) / DAVE Execute Transition (Voice Gateway).
pub const OP_STREAM_SET_PAUSED: u8 = 22;

/// Voice Gateway Opcode `21` — DAVE Prepare Protocol Transition (`JSON`).
pub const OP_DAVE_PREPARE_TRANSITION: u8 = 21;
/// Voice Gateway Opcode `22` — DAVE Execute Transition (`JSON`).
pub const OP_DAVE_EXECUTE_TRANSITION: u8 = 22;
/// Voice Gateway Opcode `23` — DAVE Ready For Transition (`JSON`, client → server).
pub const OP_DAVE_READY_FOR_TRANSITION: u8 = 23;
/// Voice Gateway Opcode `24` — DAVE Prepare Epoch (`JSON`, server → client).
pub const OP_DAVE_PREPARE_EPOCH: u8 = 24;
/// Voice Gateway Opcode `25` — MLS External Sender Package (`Binary`, server → client).
pub const OP_DAVE_MLS_EXTERNAL_SENDER: u8 = 25;
/// Voice Gateway Opcode `26` — MLS Key Package (`Binary`, client → server).
pub const OP_DAVE_MLS_KEY_PACKAGE: u8 = 26;
/// Voice Gateway Opcode `27` — MLS Proposals (`Binary`, server → client).
pub const OP_DAVE_MLS_PROPOSALS: u8 = 27;
/// Voice Gateway Opcode `28` — MLS Commit + Optional Welcome (`Binary`, client → server).
pub const OP_DAVE_MLS_COMMIT_WELCOME: u8 = 28;
/// Voice Gateway Opcode `29` — MLS Announce Commit Transition (`Binary`, server → client).
pub const OP_DAVE_MLS_ANNOUNCE_COMMIT: u8 = 29;
/// Voice Gateway Opcode `30` — MLS Welcome (`Binary`, server → client).
pub const OP_DAVE_MLS_WELCOME: u8 = 30;
/// Voice Gateway Opcode `31` — MLS Invalid Commit / Welcome (`JSON`, client → server).
pub const OP_DAVE_MLS_INVALID_COMMIT_WELCOME: u8 = 31;

/// Media stream classification matching `libdave` and `davey`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaType {
    Audio = 0,
    Video = 1,
}

impl MediaType {
    #[must_use]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }
}

/// Media codec identifier matching `libdave` and `davey`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Codec {
    Unknown = 0,
    Opus = 1,
    Vp8 = 2,
    Vp9 = 3,
    H264 = 4,
    H265 = 5,
    Av1 = 6,
}

impl Codec {
    #[must_use]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }
}

/// Operation mode of Voice Gateway Opcode `27` (`dave_mls_proposals`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalsOperation {
    Append = 0,
    Revoke = 1,
}

impl ProposalsOperation {
    pub const fn from_u8(raw: u8) -> Result<Self, DaveError> {
        match raw {
            0 => Ok(Self::Append),
            1 => Ok(Self::Revoke),
            other => Err(DaveError::UnsupportedProposalOperation(other)),
        }
    }
}

/// Unified error type across `DaveVoiceBridge`, `DaveySession`, and `LibdaveFfiBackend`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaveError {
    UnsupportedProtocolVersion(u16),
    UnsupportedOpcode(u8),
    UnsupportedProposalOperation(u8),
    DowngradeRejected { current: u16, requested: u16 },
    InvalidArgument(&'static str),
    InvalidState(&'static str),
    MalformedWire(&'static str),
    ParticipantLimitExceeded(usize),
    UnrecognizedParticipant(u64),
    DuplicateParticipant(u64),
    UnknownSender(u64),
    NotReady,
    KeyExpired,
    GenerationGapExceeded,
    ReplayDetected,
    CommitRejected,
    WelcomeRejected,
    RecoveryLimitReached,
    EncryptionFailed,
    DecryptionFailed,
    BackendFailure(&'static str),
}

impl fmt::Display for DaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for DaveError {}

/// Formats `epoch_authenticator` or pairwise fingerprint bytes into zero-padded decimal groups
/// matching Discord's DAVE whitepaper (`daveprotocol.com/#displayable-codes`).
pub fn generate_displayable_code(
    data: &[u8],
    desired_length: usize,
    group_size: usize,
) -> Result<String, DaveError> {
    if group_size == 0 || group_size > 8 || !desired_length.is_multiple_of(group_size) {
        return Err(DaveError::InvalidArgument(
            "invalid displayable code group parameters",
        ));
    }
    if data.len() < desired_length {
        return Err(DaveError::InvalidArgument(
            "insufficient bytes for displayable code",
        ));
    }
    let modulus = 10u64.pow(group_size as u32);
    let mut out = String::with_capacity(desired_length);
    let mut offset = 0usize;
    while offset < desired_length {
        let mut val = 0u64;
        for &b in &data[offset..offset + group_size] {
            val = (val << 8) | u64::from(b);
        }
        out.push_str(&format!("{:0width$}", val % modulus, width = group_size));
        offset += group_size;
    }
    Ok(out)
}
