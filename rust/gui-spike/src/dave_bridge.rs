#![forbid(unsafe_code)]
//! Stateful BFF adapter binding `DaveVoiceBridge<DaveySession>` (`dave-spike`)
//! to the Tauri voice commands (`voice_connect`, `voice_state`, `voice_e2ee_status`).

use chord_voice::MEDIA_ENCRYPTION_MODE;
use dave_spike::bridge::{
    BridgeOutboundFrame, DaveBackendKind, DaveVoiceBridge, StreamSignalingEvent,
};
use dave_spike::davey_session::DaveySession;
use dave_spike::protocol::{
    DaveError, DAVE_PROTOCOL_VERSION_V1, OP_DAVE_MLS_ANNOUNCE_COMMIT, OP_DAVE_MLS_EXTERNAL_SENDER,
    OP_DAVE_MLS_PROPOSALS, OP_DAVE_MLS_WELCOME,
};
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};

/// Canonical MLS 1.0 ciphersuite string for Discord DAVE v1.
pub const DAVE_CIPHER_SUITE_NAME: &str = "MLS_128_DHKEMP256_AES128GCM_SHA256_P256";

/// Fallback local Snowflake ID used in offline/synthetic sessions before READY hydration.
const FALLBACK_LOCAL_USER_ID: u64 = 100_000_000_000_000_001;

/// Strongly-typed E2EE status snapshot exposed to the voice UI (`voice_e2ee_status`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaveE2eeStatusDto {
    pub enabled: bool,
    pub state: String,
    pub is_end_to_end_encrypted: bool,
    pub dave_verified: bool,
    pub waiting_for_peer: bool,
    pub downgrade_blocked: bool,
    pub protocol_version: u16,
    pub configured_version: u16,
    pub epoch: Option<u64>,
    pub backend: String,
    pub cipher_suite: String,
    pub media_encryption_mode: String,
    pub privacy_code: Option<String>,
    pub privacy_code_chunks: Vec<String>,
    pub guild_id: Option<String>,
    pub channel_id: Option<String>,
    pub participant_count: usize,
    pub resets: u8,
    pub ui_badge_label: String,
}

/// Strongly-typed voice session & DAVE E2EE state snapshot (`voice_state`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceStateSnapshotDto {
    pub connected: bool,
    pub state: String,
    pub guild_id: Option<String>,
    pub channel_id: Option<String>,
    pub self_mute: bool,
    pub self_deaf: bool,
    pub effective_mute: bool,
    pub self_video: bool,
    pub self_stream: bool,
    pub speaking_flags: u32,
    pub participant_ids: Vec<String>,
    pub e2ee: DaveE2eeStatusDto,
}

/// Typed Voice Gateway binary MLS opcode selector (`Opcodes 25, 27, 29, 30`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaveGatewayBinaryOp {
    ExternalSender,
    Proposals,
    AnnounceCommit,
    Welcome,
}

impl DaveGatewayBinaryOp {
    #[must_use]
    pub const fn opcode(self) -> u8 {
        match self {
            Self::ExternalSender => OP_DAVE_MLS_EXTERNAL_SENDER,
            Self::Proposals => OP_DAVE_MLS_PROPOSALS,
            Self::AnnounceCommit => OP_DAVE_MLS_ANNOUNCE_COMMIT,
            Self::Welcome => OP_DAVE_MLS_WELCOME,
        }
    }
}

struct ActiveDaveSession {
    guild_id: Option<String>,
    channel_id_str: String,
    bridge: DaveVoiceBridge<DaveySession>,
    downgrade_blocked: bool,
}

fn global_dave_slot() -> &'static Mutex<Option<ActiveDaveSession>> {
    static SLOT: OnceLock<Mutex<Option<ActiveDaveSession>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

fn backend_label(kind: DaveBackendKind) -> &'static str {
    match kind {
        DaveBackendKind::DaveyRustCrypto => "davey_rust_crypto",
        DaveBackendKind::LibdaveFfi => "libdave_ffi",
    }
}

fn ui_badge_for_state(state: &str) -> &'static str {
    match state {
        "encrypted" => "DAVE E2EE • Encrypted",
        "waiting_for_peer" => "DAVE E2EE • Waiting for peer",
        "handshaking" => "DAVE E2EE • MLS key exchange",
        "recovering" => "DAVE E2EE • Recovering epoch",
        "downgrade_blocked" => "DAVE E2EE • Downgrade blocked",
        _ => "Disconnected",
    }
}

fn parse_strict_snowflake(raw: &str, field: &str) -> Result<u64, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || !trimmed.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("invalid {field} snowflake: `{trimmed}`"));
    }
    let parsed = trimmed
        .parse::<u64>()
        .map_err(|_| format!("{field} snowflake overflow: `{trimmed}`"))?;
    if parsed == 0 {
        return Err(format!("{field} snowflake cannot be 0"));
    }
    Ok(parsed)
}

fn resolve_channel_u64(channel_id: &str) -> Result<u64, String> {
    let trimmed = channel_id.trim();
    if trimmed.is_empty() || trimmed == "0" {
        return Err("channel_id cannot be empty or 0".to_string());
    }
    if trimmed.bytes().all(|b| b.is_ascii_digit()) {
        return parse_strict_snowflake(trimmed, "channel_id");
    }
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in trimmed.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    Ok(if hash == 0 { 1 } else { hash })
}

fn resolve_local_user_u64(self_uid_raw: &str) -> u64 {
    parse_strict_snowflake(self_uid_raw, "self_user_id").unwrap_or(FALLBACK_LOCAL_USER_ID)
}

fn split_privacy_code_chunks(code: Option<&str>) -> Vec<String> {
    let Some(raw) = code.map(str::trim).filter(|s| !s.is_empty()) else {
        return Vec::new();
    };
    if raw.len() == 30 && raw.bytes().all(|b| b.is_ascii_digit()) {
        return (0..6)
            .map(|idx| raw[idx * 5..(idx + 1) * 5].to_string())
            .collect();
    }
    let chunks: Vec<String> = raw
        .split_whitespace()
        .filter(|part| part.len() == 5 && part.bytes().all(|b| b.is_ascii_digit()))
        .map(str::to_string)
        .collect();
    if chunks.len() == 6 {
        chunks
    } else {
        Vec::new()
    }
}

/// Builds a disconnected `DaveE2eeStatusDto` baseline.
#[must_use]
pub fn disconnected_status(channel_id: Option<String>) -> DaveE2eeStatusDto {
    DaveE2eeStatusDto {
        enabled: true,
        state: "disconnected".to_string(),
        is_end_to_end_encrypted: false,
        dave_verified: false,
        waiting_for_peer: false,
        downgrade_blocked: false,
        protocol_version: 0,
        configured_version: DAVE_PROTOCOL_VERSION_V1,
        epoch: None,
        backend: backend_label(DaveBackendKind::DaveyRustCrypto).to_string(),
        cipher_suite: DAVE_CIPHER_SUITE_NAME.to_string(),
        media_encryption_mode: MEDIA_ENCRYPTION_MODE.to_string(),
        privacy_code: None,
        privacy_code_chunks: Vec::new(),
        guild_id: None,
        channel_id,
        participant_count: 0,
        resets: 0,
        ui_badge_label: ui_badge_for_state("disconnected").to_string(),
    }
}

fn status_from_active(active: &ActiveDaveSession) -> DaveE2eeStatusDto {
    let ready = active.bridge.is_ready();
    let proto_ver = active.bridge.active_protocol_version();
    let is_e2ee = ready && proto_ver >= DAVE_PROTOCOL_VERSION_V1;
    let state = if is_e2ee {
        "encrypted"
    } else if active.downgrade_blocked {
        "downgrade_blocked"
    } else if active.bridge.resets() > 0 {
        "recovering"
    } else if active.bridge.is_waiting_for_peer() {
        "waiting_for_peer"
    } else {
        "handshaking"
    };
    let privacy_code = if is_e2ee {
        active.bridge.voice_privacy_code()
    } else {
        None
    };
    let chunks = split_privacy_code_chunks(privacy_code.as_deref());
    let dave_verified = is_e2ee && chunks.len() == 6;

    DaveE2eeStatusDto {
        enabled: true,
        state: state.to_string(),
        is_end_to_end_encrypted: is_e2ee,
        dave_verified,
        waiting_for_peer: active.bridge.is_waiting_for_peer(),
        downgrade_blocked: active.downgrade_blocked,
        protocol_version: proto_ver,
        configured_version: active.bridge.configured_protocol_version(),
        epoch: active.bridge.epoch().filter(|&e| e > 0),
        backend: backend_label(active.bridge.backend_kind()).to_string(),
        cipher_suite: DAVE_CIPHER_SUITE_NAME.to_string(),
        media_encryption_mode: MEDIA_ENCRYPTION_MODE.to_string(),
        privacy_code: if dave_verified { privacy_code } else { None },
        privacy_code_chunks: chunks,
        guild_id: active.guild_id.clone(),
        channel_id: Some(active.channel_id_str.clone()),
        participant_count: active.bridge.announced_ids().len(),
        resets: active.bridge.resets(),
        ui_badge_label: ui_badge_for_state(state).to_string(),
    }
}

/// Initializes or refreshes the active `DaveVoiceBridge<DaveySession>` for a voice channel.
pub fn connect_or_refresh_session(
    guild_id: Option<String>,
    channel_id: &str,
    self_uid_raw: &str,
) -> Result<DaveE2eeStatusDto, String> {
    let chan_trimmed = channel_id.trim();
    let chan_u64 = resolve_channel_u64(chan_trimmed)?;
    let self_u64 = resolve_local_user_u64(self_uid_raw);

    let mut guard = global_dave_slot()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());

    if let Some(existing) = guard.as_mut() {
        if existing.channel_id_str == chan_trimmed && existing.bridge.user_id() == self_u64 {
            existing.guild_id = guild_id;
            return Ok(status_from_active(existing));
        }
    }

    let backend = DaveySession::new(DAVE_PROTOCOL_VERSION_V1, self_u64, chan_u64)
        .map_err(|err| format!("failed to initialize DaveySession: {err}"))?;
    let mut bridge = DaveVoiceBridge::new(backend, None)
        .map_err(|err| format!("failed to initialize DaveVoiceBridge: {err}"))?;
    bridge
        .enter_sole_member_waiting()
        .map_err(|err| format!("failed to enter sole-member waiting state: {err}"))?;

    let session = ActiveDaveSession {
        guild_id,
        channel_id_str: chan_trimmed.to_string(),
        bridge,
        downgrade_blocked: false,
    };
    let status = status_from_active(&session);
    *guard = Some(session);
    Ok(status)
}

/// Tears down the active `DaveVoiceBridge` session, zeroizing ephemeral MLS keys via `Drop`.
pub fn disconnect_session() -> DaveE2eeStatusDto {
    let mut guard = global_dave_slot()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    *guard = None;
    disconnected_status(None)
}

fn parse_unique_peer_snowflakes(peer_ids: &[String]) -> Result<Vec<u64>, String> {
    let mut parsed_peers = Vec::with_capacity(peer_ids.len());
    for raw in peer_ids {
        let uid = parse_strict_snowflake(raw, "peer_user_id")?;
        if !parsed_peers.contains(&uid) {
            parsed_peers.push(uid);
        }
    }
    Ok(parsed_peers)
}

/// Synchronizes connected channel peer Snowflakes with the active `DaveVoiceBridge`.
pub fn sync_peer_roster(peer_ids: &[String]) -> Result<DaveE2eeStatusDto, String> {
    let mut parsed_peers = parse_unique_peer_snowflakes(peer_ids)?;
    let mut guard = global_dave_slot()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let Some(active) = guard.as_mut() else {
        return Err("no active DaveVoiceBridge session".to_string());
    };

    let own_uid = active.bridge.user_id();
    parsed_peers.retain(|&uid| uid != own_uid);
    let departed: Vec<u64> = active
        .bridge
        .announced_ids()
        .iter()
        .copied()
        .filter(|&uid| uid != own_uid && !parsed_peers.contains(&uid))
        .collect();

    for uid in departed {
        active
            .bridge
            .disconnect_user(uid)
            .map_err(|err| format!("DaveVoiceBridge disconnect_user failed: {err}"))?;
    }
    if !parsed_peers.is_empty() {
        active
            .bridge
            .connect_users(&parsed_peers)
            .map_err(|err| format!("DaveVoiceBridge connect_users failed: {err}"))?;
    } else if active.bridge.announced_ids().len() == 1 {
        active
            .bridge
            .enter_sole_member_waiting()
            .map_err(|err| format!("DaveVoiceBridge enter_sole_member_waiting failed: {err}"))?;
    }
    Ok(status_from_active(active))
}

/// Queries the current `DaveE2eeStatusDto`, optionally scoped to `filter_channel_id`.
#[must_use]
pub fn e2ee_status_for_channel(filter_channel_id: Option<&str>) -> DaveE2eeStatusDto {
    let clean_filter = filter_channel_id
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "null" && *s != "nil");
    let guard = global_dave_slot()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let Some(active) = guard.as_ref() else {
        return disconnected_status(clean_filter.map(str::to_string));
    };
    if let Some(req_cid) = clean_filter {
        if req_cid != active.channel_id_str {
            return disconnected_status(Some(req_cid.to_string()));
        }
    }
    status_from_active(active)
}

/// Returns the local user Snowflake (`u64`) bound to the active `DaveVoiceBridge`, if connected.
#[must_use]
pub fn active_local_user_id() -> Option<u64> {
    let guard = global_dave_slot()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    guard.as_ref().map(|s| s.bridge.user_id())
}

/// Returns the announced participant Snowflake strings in the active `DaveVoiceBridge`.
#[must_use]
pub fn active_participant_ids_strings() -> Vec<String> {
    let guard = global_dave_slot()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    guard
        .as_ref()
        .map(|s| {
            s.bridge
                .announced_ids()
                .iter()
                .map(u64::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Routes Voice Gateway Opcode `21` (`dave_protocol_prepare_transition`) with downgrade tracking.
pub fn handle_prepare_transition(
    opcode: u8,
    transition_id: u16,
    protocol_version: u16,
) -> Result<Vec<BridgeOutboundFrame>, String> {
    let mut guard = global_dave_slot()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let Some(active) = guard.as_mut() else {
        return Err("no active DaveVoiceBridge session".to_string());
    };
    match active
        .bridge
        .handle_prepare_transition(opcode, transition_id, protocol_version)
    {
        Ok(frames) => Ok(frames),
        Err(err) => {
            if matches!(err, DaveError::DowngradeRejected { .. }) {
                active.downgrade_blocked = true;
            }
            Err(format!("DAVE prepare_transition error: {err}"))
        }
    }
}

/// Routes Voice Gateway Opcode `24` (`dave_protocol_prepare_epoch`).
pub fn handle_prepare_epoch(
    opcode: u8,
    epoch: u64,
    protocol_version: u16,
) -> Result<Vec<BridgeOutboundFrame>, String> {
    let mut guard = global_dave_slot()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let Some(active) = guard.as_mut() else {
        return Err("no active DaveVoiceBridge session".to_string());
    };
    active
        .bridge
        .handle_prepare_epoch(opcode, epoch, protocol_version)
        .map_err(|err| format!("DAVE prepare_epoch error: {err}"))
}

/// Routes Voice Gateway Opcode `22` (`dave_protocol_execute_transition`).
pub fn handle_execute_transition(opcode: u8, transition_id: u16) -> Result<(), String> {
    let mut guard = global_dave_slot()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let Some(active) = guard.as_mut() else {
        return Err("no active DaveVoiceBridge session".to_string());
    };
    active
        .bridge
        .handle_execute_transition(opcode, transition_id)
        .map_err(|err| format!("DAVE execute_transition error: {err}"))
}

/// Routes binary MLS Voice Gateway frames (`Opcodes 25, 27, 29, 30`).
pub fn handle_gateway_binary(
    op: DaveGatewayBinaryOp,
    payload: &[u8],
) -> Result<Vec<BridgeOutboundFrame>, String> {
    let mut guard = global_dave_slot()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let Some(active) = guard.as_mut() else {
        return Err("no active DaveVoiceBridge session".to_string());
    };
    let frames = active
        .bridge
        .handle_binary_opcode(op.opcode(), payload)
        .map_err(|err| format!("DAVE binary opcode {} error: {err}", op.opcode()))?;
    if active.bridge.resets() > 0 {
        active.downgrade_blocked = false;
    }
    Ok(frames)
}

/// Routes Main Gateway Go Live stream signaling (`Opcodes 18..=22`) through `DaveVoiceBridge`.
pub fn handle_stream_signaling(
    opcode: u8,
    json_payload: &str,
) -> Result<StreamSignalingEvent, String> {
    let guard = global_dave_slot()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let Some(active) = guard.as_ref() else {
        return Err("no active DaveVoiceBridge session".to_string());
    };
    active
        .bridge
        .handle_stream_opcode(opcode, json_payload)
        .map_err(|err| format!("DAVE stream opcode {opcode} error: {err}"))
}

/// Encrypts an outbound Opus audio packet via the active `DaveVoiceBridge`.
pub fn encrypt_active_opus(opus_frame: &[u8]) -> Result<Vec<u8>, String> {
    let mut guard = global_dave_slot()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let Some(active) = guard.as_mut() else {
        return Err("no active DaveVoiceBridge session".to_string());
    };
    active
        .bridge
        .encrypt_opus(opus_frame)
        .map_err(|err| format!("DAVE encrypt_opus error: {err}"))
}

/// Decrypts an inbound DAVE-protected Opus audio packet via the active `DaveVoiceBridge`.
pub fn decrypt_active_opus(sender_uid: u64, frame: &[u8]) -> Result<Vec<u8>, String> {
    let mut guard = global_dave_slot()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let Some(active) = guard.as_mut() else {
        return Err("no active DaveVoiceBridge session".to_string());
    };
    active
        .bridge
        .decrypt_opus(sender_uid, frame)
        .map_err(|err| format!("DAVE decrypt_opus error: {err}"))
}
