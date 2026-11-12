//! Audit Log Parser and Change Decoder (TASK-1008A).
//!
//! Provides protocol parsing for `AuditLogActionType 1..=145`, decoding of old/new change
//! diffs, entity resolution (`users[]`, `roles[]`, `channels[]`), and request building for
//! `GET /guilds/{id}/audit-logs`.

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::collections::BTreeMap;

pub const AUDIT_PROTOCOL_CONTAINER: &str = "Audit Protocol";
pub const AUDIT_PROTOCOL_DIMENSIONS: &str = "Core Module";
pub const AUDIT_PROTOCOL_PADDING: &str = "0px";
pub const AUDIT_PROTOCOL_SPACING: &str = "0px";

pub const TOKEN_BACKGROUND: &str = "#1e1f22";
pub const TOKEN_SURFACE: &str = "#2b2d31";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_BORDER: &str = "#3f4147";

pub const AUDIT_DECODE_ELEMENT: &str = "audit-decode";

pub const AUDIT_ENDPOINT_TEMPLATE: &str = "/guilds/{id}/audit-logs";
pub const DEFAULT_AUDIT_LOG_LIMIT: u8 = 50;
pub const MAX_AUDIT_LOG_LIMIT: u8 = 100;
pub const MIN_AUDIT_LOG_LIMIT: u8 = 1;

pub const PERM_VIEW_AUDIT_LOG: u64 = 1 << 7;
pub const PERM_ADMINISTRATOR: u64 = 1 << 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditDecodePhase {
    Raw,
    Decoding,
    HumanReadable,
}

impl AuditDecodePhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::Decoding => "decoding",
            Self::HumanReadable => "human readable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditActionCategory {
    Create,
    Update,
    Delete,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditTargetKind {
    Guild,
    Channel,
    ChannelOverwrite,
    User,
    Role,
    Invite,
    Webhook,
    Emoji,
    Sticker,
    Thread,
    AutoModerationRule,
    Unknown,
}

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AuditLogActionType {
    GuildUpdate = 1,
    ChannelCreate = 10,
    ChannelUpdate = 11,
    ChannelDelete = 12,
    ChannelOverwriteCreate = 13,
    ChannelOverwriteUpdate = 14,
    ChannelOverwriteDelete = 15,
    MemberKick = 20,
    MemberPrune = 21,
    MemberBanAdd = 22,
    MemberBanRemove = 23,
    MemberUpdate = 24,
    MemberRoleUpdate = 25,
    MemberMove = 26,
    MemberDisconnect = 27,
    BotAdd = 28,
    RoleCreate = 30,
    RoleUpdate = 31,
    RoleDelete = 32,
    InviteCreate = 40,
    InviteUpdate = 41,
    InviteDelete = 42,
    WebhookCreate = 50,
    WebhookUpdate = 51,
    WebhookDelete = 52,
    EmojiCreate = 60,
    EmojiUpdate = 61,
    EmojiDelete = 62,
    MessageDelete = 72,
    MessageBulkDelete = 73,
    MessagePin = 74,
    MessageUnpin = 75,
    IntegrationCreate = 80,
    IntegrationUpdate = 81,
    IntegrationDelete = 82,
    StageInstanceCreate = 83,
    StageInstanceUpdate = 84,
    StageInstanceDelete = 85,
    StickerCreate = 90,
    StickerUpdate = 91,
    StickerDelete = 92,
    GuildScheduledEventCreate = 100,
    GuildScheduledEventUpdate = 101,
    GuildScheduledEventDelete = 102,
    ThreadCreate = 110,
    ThreadUpdate = 111,
    ThreadDelete = 112,
    ApplicationCommandPermissionUpdate = 121,
    AutoModerationRuleCreate = 140,
    AutoModerationRuleUpdate = 141,
    AutoModerationRuleDelete = 142,
    AutoModerationBlockMessage = 143,
    AutoModerationFlagToChannel = 144,
    AutoModerationUserCommunicationDisabled = 145,
    Unknown(u16),
}

impl AuditLogActionType {
    pub fn from_u16(code: u16) -> Self {
        if code <= 42 {
            Self::from_u16_low(code)
        } else {
            Self::from_u16_high(code)
        }
    }

    fn from_u16_low(code: u16) -> Self {
        match code {
            1 => Self::GuildUpdate,
            10 => Self::ChannelCreate,
            11 => Self::ChannelUpdate,
            12 => Self::ChannelDelete,
            13 => Self::ChannelOverwriteCreate,
            14 => Self::ChannelOverwriteUpdate,
            15 => Self::ChannelOverwriteDelete,
            20 => Self::MemberKick,
            21 => Self::MemberPrune,
            22 => Self::MemberBanAdd,
            23 => Self::MemberBanRemove,
            24 => Self::MemberUpdate,
            25 => Self::MemberRoleUpdate,
            26 => Self::MemberMove,
            27 => Self::MemberDisconnect,
            28 => Self::BotAdd,
            30 => Self::RoleCreate,
            31 => Self::RoleUpdate,
            32 => Self::RoleDelete,
            40 => Self::InviteCreate,
            41 => Self::InviteUpdate,
            42 => Self::InviteDelete,
            other => Self::Unknown(other),
        }
    }

    fn from_u16_high(code: u16) -> Self {
        match code {
            50 => Self::WebhookCreate,
            51 => Self::WebhookUpdate,
            52 => Self::WebhookDelete,
            60 => Self::EmojiCreate,
            61 => Self::EmojiUpdate,
            62 => Self::EmojiDelete,
            72 => Self::MessageDelete,
            73 => Self::MessageBulkDelete,
            74 => Self::MessagePin,
            75 => Self::MessageUnpin,
            80 => Self::IntegrationCreate,
            81 => Self::IntegrationUpdate,
            82 => Self::IntegrationDelete,
            83 => Self::StageInstanceCreate,
            84 => Self::StageInstanceUpdate,
            85 => Self::StageInstanceDelete,
            90 => Self::StickerCreate,
            91 => Self::StickerUpdate,
            92 => Self::StickerDelete,
            100 => Self::GuildScheduledEventCreate,
            101 => Self::GuildScheduledEventUpdate,
            102 => Self::GuildScheduledEventDelete,
            110 => Self::ThreadCreate,
            111 => Self::ThreadUpdate,
            112 => Self::ThreadDelete,
            121 => Self::ApplicationCommandPermissionUpdate,
            140 => Self::AutoModerationRuleCreate,
            141 => Self::AutoModerationRuleUpdate,
            142 => Self::AutoModerationRuleDelete,
            143 => Self::AutoModerationBlockMessage,
            144 => Self::AutoModerationFlagToChannel,
            145 => Self::AutoModerationUserCommunicationDisabled,
            other => Self::Unknown(other),
        }
    }

    pub fn category(&self) -> AuditActionCategory {
        match self {
            Self::ChannelCreate
            | Self::RoleCreate
            | Self::InviteCreate
            | Self::WebhookCreate
            | Self::EmojiCreate
            | Self::StickerCreate
            | Self::ThreadCreate
            | Self::MemberBanAdd
            | Self::ChannelOverwriteCreate
            | Self::StageInstanceCreate
            | Self::IntegrationCreate
            | Self::GuildScheduledEventCreate
            | Self::AutoModerationRuleCreate => AuditActionCategory::Create,
            Self::ChannelDelete
            | Self::RoleDelete
            | Self::InviteDelete
            | Self::WebhookDelete
            | Self::EmojiDelete
            | Self::StickerDelete
            | Self::ThreadDelete
            | Self::MemberKick
            | Self::MemberPrune
            | Self::MemberBanRemove
            | Self::MessageDelete
            | Self::MessageBulkDelete
            | Self::ChannelOverwriteDelete
            | Self::StageInstanceDelete
            | Self::IntegrationDelete
            | Self::GuildScheduledEventDelete
            | Self::AutoModerationRuleDelete => AuditActionCategory::Delete,
            Self::Unknown(_)
            | Self::MessagePin
            | Self::MessageUnpin
            | Self::MemberMove
            | Self::MemberDisconnect
            | Self::BotAdd
            | Self::AutoModerationBlockMessage
            | Self::AutoModerationFlagToChannel => AuditActionCategory::Other,
            _ => AuditActionCategory::Update,
        }
    }

    pub fn target_kind(&self) -> AuditTargetKind {
        match self {
            Self::GuildUpdate => AuditTargetKind::Guild,
            Self::ChannelCreate | Self::ChannelUpdate | Self::ChannelDelete => {
                AuditTargetKind::Channel
            }
            Self::ChannelOverwriteCreate
            | Self::ChannelOverwriteUpdate
            | Self::ChannelOverwriteDelete => AuditTargetKind::ChannelOverwrite,
            Self::RoleCreate | Self::RoleUpdate | Self::RoleDelete => AuditTargetKind::Role,
            Self::InviteCreate | Self::InviteUpdate | Self::InviteDelete => AuditTargetKind::Invite,
            Self::WebhookCreate | Self::WebhookUpdate | Self::WebhookDelete => {
                AuditTargetKind::Webhook
            }
            Self::EmojiCreate | Self::EmojiUpdate | Self::EmojiDelete => AuditTargetKind::Emoji,
            Self::StickerCreate | Self::StickerUpdate | Self::StickerDelete => {
                AuditTargetKind::Sticker
            }
            Self::ThreadCreate | Self::ThreadUpdate | Self::ThreadDelete => AuditTargetKind::Thread,
            Self::AutoModerationRuleCreate
            | Self::AutoModerationRuleUpdate
            | Self::AutoModerationRuleDelete
            | Self::AutoModerationBlockMessage
            | Self::AutoModerationFlagToChannel => AuditTargetKind::AutoModerationRule,
            Self::MemberKick
            | Self::MemberPrune
            | Self::MemberBanAdd
            | Self::MemberBanRemove
            | Self::MemberUpdate
            | Self::MemberRoleUpdate
            | Self::MemberMove
            | Self::MemberDisconnect
            | Self::BotAdd
            | Self::AutoModerationUserCommunicationDisabled => AuditTargetKind::User,
            _ => AuditTargetKind::Unknown,
        }
    }
}

pub fn audit_action_label_ru(action: AuditLogActionType) -> &'static str {
    match action {
        AuditLogActionType::GuildUpdate => "Обновление сервера",
        AuditLogActionType::ChannelCreate => "Создание канала",
        AuditLogActionType::ChannelUpdate => "Изменение канала",
        AuditLogActionType::ChannelDelete => "Удаление канала",
        AuditLogActionType::MemberKick => "Исключение участника",
        AuditLogActionType::MemberBanAdd => "Бан участника",
        AuditLogActionType::MemberBanRemove => "Разбан участника",
        AuditLogActionType::RoleCreate => "Создание роли",
        AuditLogActionType::RoleUpdate => "Изменение роли",
        AuditLogActionType::RoleDelete => "Удаление роли",
        AuditLogActionType::MessageDelete => "Удаление сообщения",
        _ => "Действие аудита",
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GuildAuditLogQuery {
    pub user_id: Option<u64>,
    pub action_type: Option<u16>,
    pub before: Option<u64>,
    pub after: Option<u64>,
    pub limit: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestAuditLogRequest {
    pub method: &'static str,
    pub path: String,
    pub query_string: String,
    pub limit: u8,
}

pub fn build_guild_audit_logs_request(
    guild_id: u64,
    query: &GuildAuditLogQuery,
) -> RestAuditLogRequest {
    let limit = query
        .limit
        .unwrap_or(DEFAULT_AUDIT_LOG_LIMIT)
        .clamp(MIN_AUDIT_LOG_LIMIT, MAX_AUDIT_LOG_LIMIT);

    let mut parts = Vec::new();
    parts.push(format!("limit={limit}"));
    if let Some(uid) = query.user_id {
        parts.push(format!("user_id={uid}"));
    }
    if let Some(act) = query.action_type {
        parts.push(format!("action_type={act}"));
    }
    if let Some(before) = query.before {
        parts.push(format!("before={before}"));
    }
    if let Some(after) = query.after {
        parts.push(format!("after={after}"));
    }

    RestAuditLogRequest {
        method: "GET",
        path: format!("/guilds/{guild_id}/audit-logs"),
        query_string: parts.join("&"),
        limit,
    }
}

pub fn validate_audit_reason_header(reason: &str) -> Result<(), &'static str> {
    if reason.is_empty() {
        return Err("Reason cannot be empty");
    }
    if reason.len() > 512 {
        return Err("Reason exceeds 512 bytes limit");
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditChangeValue {
    String(String),
    Integer(i64),
    Boolean(bool),
    RoleList(Vec<u64>),
    Null,
}

impl AuditChangeValue {
    pub fn display_value(&self) -> String {
        match self {
            Self::String(s) => s.clone(),
            Self::Integer(i) => i.to_string(),
            Self::Boolean(b) => b.to_string(),
            Self::RoleList(roles) => {
                let items: Vec<String> = roles.iter().map(|r| r.to_string()).collect();
                items.join(", ")
            }
            Self::Null => "нет".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditLogChange {
    pub key: String,
    pub old_value: AuditChangeValue,
    pub new_value: AuditChangeValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedAuditChange {
    pub key: String,
    pub old_display: String,
    pub new_display: String,
    pub formatted_ru: String,
}

pub fn is_sensitive_change_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    lower.contains("token")
        || lower.contains("secret")
        || lower.contains("password")
        || lower.contains("mfa")
}

pub fn decode_audit_log_change(change: &AuditLogChange) -> DecodedAuditChange {
    let key = change.key.as_str();
    let (old_display, new_display) = if is_sensitive_change_key(key) {
        ("[REDACTED]".to_string(), "[REDACTED]".to_string())
    } else {
        (
            change.old_value.display_value(),
            change.new_value.display_value(),
        )
    };

    let formatted_ru = format!("{}: {old_display} → {new_display}", change.key);
    DecodedAuditChange {
        key: change.key.clone(),
        old_display,
        new_display,
        formatted_ru,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditLogUser {
    pub id: u64,
    pub username: String,
    pub global_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditLogRole {
    pub id: u64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditLogChannel {
    pub id: u64,
    pub name: String,
}

#[derive(Debug, Clone, Default)]
pub struct AuditLogContext {
    pub users: BTreeMap<u64, AuditLogUser>,
    pub roles: BTreeMap<u64, AuditLogRole>,
    pub channels: BTreeMap<u64, AuditLogChannel>,
}

#[derive(Debug, Clone)]
pub struct RawAuditLogEntry {
    pub id: u64,
    pub action_type: u16,
    pub user_id: Option<u64>,
    pub target_id: Option<u64>,
    pub reason: Option<String>,
    pub changes: Vec<AuditLogChange>,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone)]
pub struct DecodedAuditLogEntry {
    pub id: u64,
    pub action_type: AuditLogActionType,
    pub moderator_name: String,
    pub target_name: String,
    pub reason_display: Option<String>,
    pub changes: Vec<DecodedAuditChange>,
    pub summary_ru: String,
}

#[allow(clippy::needless_lifetimes, clippy::needless_match)]
pub fn resolve_moderator_user<'a>(
    uid: Option<u64>,
    ctx: &'a AuditLogContext,
) -> Option<&'a AuditLogUser> {
    if let Some(user) = uid.and_then(|id| ctx.users.get(&id)) {
        Some(user)
    } else {
        None
    }
}

pub fn resolve_target_entity(
    target_id: Option<u64>,
    kind: AuditTargetKind,
    ctx: &AuditLogContext,
) -> String {
    let tid = match target_id {
        Some(id) => id,
        None => return "нет цели".to_string(),
    };
    match kind {
        AuditTargetKind::User => ctx
            .users
            .get(&tid)
            .map(|u| u.global_name.clone().unwrap_or_else(|| u.username.clone()))
            .unwrap_or_else(|| tid.to_string()),
        AuditTargetKind::Role => ctx
            .roles
            .get(&tid)
            .map(|r| r.name.clone())
            .unwrap_or_else(|| tid.to_string()),
        AuditTargetKind::Channel | AuditTargetKind::Thread => ctx
            .channels
            .get(&tid)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| tid.to_string()),
        _ => tid.to_string(),
    }
}

pub fn decode_audit_log_entry(
    raw: &RawAuditLogEntry,
    ctx: &AuditLogContext,
) -> DecodedAuditLogEntry {
    let action = AuditLogActionType::from_u16(raw.action_type);
    let mod_user = resolve_moderator_user(raw.user_id, ctx);
    let moderator_name = match mod_user {
        Some(u) => u.global_name.clone().unwrap_or_else(|| u.username.clone()),
        None => "Неизвестный".to_string(),
    };

    let target_name = resolve_target_entity(raw.target_id, action.target_kind(), ctx);
    let decoded_changes: Vec<DecodedAuditChange> =
        raw.changes.iter().map(decode_audit_log_change).collect();

    let label_ru = audit_action_label_ru(action);
    let reason_part = match &raw.reason {
        Some(r) if !r.trim().is_empty() => format!(" — Причина: {r}"),
        _ => String::new(),
    };
    let summary_ru = format!("{moderator_name}: {label_ru} ({target_name}){reason_part}");

    DecodedAuditLogEntry {
        id: raw.id,
        action_type: action,
        moderator_name,
        target_name,
        reason_display: raw.reason.clone(),
        changes: decoded_changes,
        summary_ru,
    }
}

pub fn check_audit_log_access(permissions_bitset: u64) -> bool {
    (permissions_bitset & PERM_ADMINISTRATOR != 0)
        || (permissions_bitset & PERM_VIEW_AUDIT_LOG != 0)
}

pub fn classify_audit_http_status(status_code: u16) -> &'static str {
    match status_code {
        200 => "OK",
        202 => "Accepted/Skeleton",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        413 => "Payload Too Large",
        429 => "Rate Limited",
        500..=599 => "Server Error",
        _ => "Unknown",
    }
}

pub fn validate_audit_workload_boundaries(
    message_len: usize,
    files_count: usize,
    total_bytes: usize,
) -> Result<(), &'static str> {
    if message_len > 2000 {
        return Err("Message text exceeds 2000 characters limit");
    }
    if files_count > 10 {
        return Err("Files count exceeds 10 attachments limit");
    }
    if total_bytes > 25 * 1024 * 1024 {
        return Err("Payload exceeds 25 MB boundary");
    }
    Ok(())
}

pub fn can_group_consecutive_entries(
    first_time_ms: u64,
    second_time_ms: u64,
    max_window_ms: u64,
) -> bool {
    let diff = second_time_ms.abs_diff(first_time_ms);
    diff <= max_window_ms
}

pub fn format_audit_badge(unread_count: u32) -> String {
    if unread_count > 99 {
        "99+".to_string()
    } else {
        unread_count.to_string()
    }
}

pub fn redact_audit_telemetry(raw: &str) -> String {
    let mut s = raw.to_string();
    if let Some(idx) = s.find("mfa.") {
        let end = s[idx..]
            .find(|c: char| c.is_whitespace() || c == '"' || c == ',' || c == '}')
            .map(|i| idx + i)
            .unwrap_or(s.len());
        s.replace_range(idx..end, "mfa.***");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sub_1008a_1_layout_tokens_and_phase() {
        assert_eq!(AUDIT_PROTOCOL_CONTAINER, "Audit Protocol");
        assert_eq!(AUDIT_PROTOCOL_DIMENSIONS, "Core Module");
        assert_eq!(TOKEN_BACKGROUND, "#1e1f22");
        assert_eq!(TOKEN_SURFACE, "#2b2d31");
        assert_eq!(TOKEN_TEXT_PRIMARY, "#ffffff");
        assert_eq!(TOKEN_ACCENT, "#5865f2");
        assert_eq!(TOKEN_BORDER, "#3f4147");
        assert_eq!(AuditDecodePhase::Raw.as_str(), "raw");
        assert_eq!(AuditDecodePhase::Decoding.as_str(), "decoding");
        assert_eq!(AuditDecodePhase::HumanReadable.as_str(), "human readable");
        assert_eq!(AUDIT_ENDPOINT_TEMPLATE, "/guilds/{id}/audit-logs");
    }

    #[test]
    fn test_sub_1008a_2_action_types_categories_and_rest() {
        let act = AuditLogActionType::from_u16(1);
        assert_eq!(act, AuditLogActionType::GuildUpdate);
        assert_eq!(act.category(), AuditActionCategory::Update);
        assert_eq!(act.target_kind(), AuditTargetKind::Guild);
        assert_eq!(audit_action_label_ru(act), "Обновление сервера");

        let m_ban = AuditLogActionType::from_u16(22);
        assert_eq!(m_ban, AuditLogActionType::MemberBanAdd);
        assert_eq!(m_ban.category(), AuditActionCategory::Create);
        assert_eq!(m_ban.target_kind(), AuditTargetKind::User);
        assert_eq!(audit_action_label_ru(m_ban), "Бан участника");

        let r_del = AuditLogActionType::from_u16(32);
        assert_eq!(r_del, AuditLogActionType::RoleDelete);
        assert_eq!(r_del.category(), AuditActionCategory::Delete);
        assert_eq!(r_del.target_kind(), AuditTargetKind::Role);
        assert_eq!(audit_action_label_ru(r_del), "Удаление роли");

        let auto_mod = AuditLogActionType::from_u16(145);
        assert_eq!(
            auto_mod,
            AuditLogActionType::AutoModerationUserCommunicationDisabled
        );

        let query = GuildAuditLogQuery {
            user_id: Some(12345),
            action_type: Some(22),
            before: None,
            after: None,
            limit: None,
        };
        let req = build_guild_audit_logs_request(999, &query);
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/guilds/999/audit-logs");
        assert_eq!(req.limit, 50);
        assert!(req.query_string.contains("limit=50"));
        assert!(req.query_string.contains("user_id=12345"));
        assert!(req.query_string.contains("action_type=22"));

        assert!(validate_audit_reason_header("Spam in general").is_ok());
        assert!(validate_audit_reason_header("").is_err());
        assert!(validate_audit_reason_header(&"a".repeat(513)).is_err());
    }

    #[test]
    fn test_sub_1008a_3_decode_changes() {
        let change = AuditLogChange {
            key: "nick".to_string(),
            old_value: AuditChangeValue::String("OldNick".to_string()),
            new_value: AuditChangeValue::String("NewNick".to_string()),
        };
        let decoded_ch = decode_audit_log_change(&change);
        assert_eq!(decoded_ch.old_display, "OldNick");
        assert_eq!(decoded_ch.new_display, "NewNick");
        assert_eq!(decoded_ch.formatted_ru, "nick: OldNick → NewNick");

        let secret_ch = AuditLogChange {
            key: "mfa_token".to_string(),
            old_value: AuditChangeValue::String("secret123".to_string()),
            new_value: AuditChangeValue::String("secret456".to_string()),
        };
        let decoded_secret = decode_audit_log_change(&secret_ch);
        assert_eq!(decoded_secret.old_display, "[REDACTED]");
        assert_eq!(decoded_secret.new_display, "[REDACTED]");
    }

    #[test]
    fn test_sub_1008a_3_decode_entries() {
        let mut ctx = AuditLogContext::default();
        ctx.users.insert(
            100,
            AuditLogUser {
                id: 100,
                username: "admin".to_string(),
                global_name: Some("GlobalAdmin".to_string()),
            },
        );
        ctx.users.insert(
            200,
            AuditLogUser {
                id: 200,
                username: "spammer".to_string(),
                global_name: None,
            },
        );

        let resolved_mod = resolve_moderator_user(Some(100), &ctx);
        assert!(resolved_mod.is_some());
        assert_eq!(resolved_mod.map(|u| u.username.as_str()), Some("admin"));

        let change = AuditLogChange {
            key: "nick".to_string(),
            old_value: AuditChangeValue::String("OldNick".to_string()),
            new_value: AuditChangeValue::String("NewNick".to_string()),
        };
        let raw = RawAuditLogEntry {
            id: 888,
            action_type: 22,
            user_id: Some(100),
            target_id: Some(200),
            reason: Some("Bad behavior".to_string()),
            changes: vec![change],
            timestamp_ms: 1_700_000_000_000,
        };
        let decoded_entry = decode_audit_log_entry(&raw, &ctx);
        assert_eq!(decoded_entry.moderator_name, "GlobalAdmin");
        assert_eq!(decoded_entry.target_name, "spammer");
        assert_eq!(
            decoded_entry.summary_ru,
            "GlobalAdmin: Бан участника (spammer) — Причина: Bad behavior"
        );
    }

    #[test]
    fn test_sub_1008a_4_nine_dimensions_and_policies() {
        assert!(check_audit_log_access(PERM_VIEW_AUDIT_LOG));
        assert!(check_audit_log_access(PERM_ADMINISTRATOR));
        assert!(!check_audit_log_access(0));

        assert_eq!(classify_audit_http_status(200), "OK");
        assert_eq!(classify_audit_http_status(202), "Accepted/Skeleton");
        assert_eq!(classify_audit_http_status(403), "Forbidden");
        assert_eq!(classify_audit_http_status(429), "Rate Limited");

        assert!(validate_audit_workload_boundaries(2000, 10, 25 * 1024 * 1024).is_ok());
        assert!(validate_audit_workload_boundaries(2001, 10, 100).is_err());
        assert!(validate_audit_workload_boundaries(100, 11, 100).is_err());
        assert!(validate_audit_workload_boundaries(100, 1, 26 * 1024 * 1024).is_err());

        assert!(can_group_consecutive_entries(1000, 2000, 30 * 60 * 1000));
        assert!(!can_group_consecutive_entries(
            1000,
            1000 + 31 * 60 * 1000,
            30 * 60 * 1000
        ));

        assert_eq!(format_audit_badge(5), "5");
        assert_eq!(format_audit_badge(99), "99");
        assert_eq!(format_audit_badge(100), "99+");

        assert_eq!(
            redact_audit_telemetry("token: mfa.abc123xyz payload"),
            "token: mfa.*** payload"
        );
    }
}
