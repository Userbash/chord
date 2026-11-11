//! Журнал аудита сервера (`Server Audit Log & REST API v9` — `TASK-1008` / `TASK-1008A` / `TASK-1008B`).
//! Оракулы: `waves/kanban-board.json`, `/home/sanya/re-deep/rest-static.json` (`GUILD_AUDIT_LOG`),
//! `/home/sanya/re-deep/modules-index.json` (`117497`, `165259`, `411153`, `523599`, `555067`),
//! `rust/crates/discord-core/src/permissions.rs` (`VIEW_AUDIT_LOG = 1 << 7`) и `audit-log.js`.

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fmt;

pub const AUDIT_LOG_VIEWER_CONTAINER: &str = "Audit Log Viewer";
pub const AUDIT_LOG_VIEWER_DIMENSIONS: &str = "100% x 100%";
pub const AUDIT_LOG_VIEWER_PADDING_PX: u32 = 16;
pub const AUDIT_LOG_VIEWER_SPACING_PX: u32 = 8;

pub const TOKEN_BACKGROUND: &str = "#313338";
pub const TOKEN_SURFACE: &str = "#2b2d31";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_BORDER: &str = "#3f4147";

pub const AUDIT_ENTRY_ELEMENT: &str = "audit-entry";
pub const AUDIT_DECODE_ELEMENT: &str = "audit-decode";
pub const AUDIT_ROW_ELEMENT: &str = "audit-row";
pub const HEADER_AUDIT_LOG_REASON: &str = "X-Audit-Log-Reason";

pub const RE_MODULE_AUDIT_SECTION_CSS: &str = "117497";
pub const RE_MODULE_AUDIT_HEADER_CSS: &str = "165259";
pub const RE_MODULE_AUDIT_MODEL: &str = "411153";
pub const RE_MODULE_AUDIT_STORE: &str = "523599";
pub const RE_MODULE_AUDIT_UTILS: &str = "555067";

pub const MIN_AUDIT_LOG_LIMIT: u8 = 1;
pub const DEFAULT_AUDIT_LOG_LIMIT: u8 = 50;
pub const MAX_AUDIT_LOG_LIMIT: u8 = 100;
pub const MAX_AUDIT_REASON_CHARS: usize = 512;
pub const MERGE_WINDOW_MS: u64 = 30 * 60 * 1_000;
pub const MAX_GROUPED_CHANGES: usize = 50;
pub const DISCORD_EPOCH_MS: u64 = 1_420_070_400_000;

pub const PERM_ADMINISTRATOR: u64 = 1 << 3;
pub const PERM_VIEW_AUDIT_LOG: u64 = 1 << 7;

pub const MAX_MESSAGE_CHARS: usize = 2_000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_BADGE_CAP: u32 = 99;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditEntryInteractiveState {
    EntryRow,
    ExpandDiff,
    ExpandedDetails,
}

impl AuditEntryInteractiveState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EntryRow => "entry row",
            Self::ExpandDiff => "expand diff",
            Self::ExpandedDetails => "expanded details",
        }
    }

    #[must_use]
    pub const fn resolve(expanded: bool, hovered: bool) -> Self {
        if expanded {
            Self::ExpandedDetails
        } else if hovered {
            Self::ExpandDiff
        } else {
            Self::EntryRow
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditDecodeState {
    Raw,
    Decoding,
    HumanReadable,
}

impl AuditDecodeState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
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
    All,
}

impl AuditActionCategory {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "CREATE",
            Self::Update => "UPDATE",
            Self::Delete => "DELETE",
            Self::All => "ALL",
        }
    }

    #[must_use]
    pub const fn from_action_code(code: u16) -> Self {
        match code {
            10 | 13 | 23 | 28 | 30 | 40 | 50 | 60 | 74 => Self::Create,
            80 | 83 | 90 | 100 | 110 | 130 | 140 | 150 | 163 | 166 | 171 | 190 | 192 => {
                Self::Create
            }
            12 | 15 | 20..=22 | 27 | 32 | 42 | 52 | 62 | 72 | 73 | 75 => Self::Delete,
            82 | 85 | 92 | 102 | 112 | 132 | 142 | 143 | 165 | 172 | 193 | 202 => Self::Delete,
            1 | 11 | 14 | 16 | 24..=26 | 31 | 33 | 41 | 51 | 61 => Self::Update,
            81 | 84 | 91 | 101 | 111 | 121 | 131 | 141 | 144..=146 => Self::Update,
            151 | 164 | 167 | 191 | 200 | 201 | 210..=213 => Self::Update,
            _ => Self::All,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditTargetType {
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

impl AuditTargetType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Guild => "GUILD",
            Self::Channel => "CHANNEL",
            Self::ChannelOverwrite => "CHANNEL_OVERWRITE",
            Self::User => "USER",
            Self::Role => "ROLE",
            Self::Invite => "INVITE",
            Self::Webhook => "WEBHOOK",
            Self::Emoji => "EMOJI",
            Self::Sticker => "STICKER",
            Self::Thread => "THREAD",
            Self::AutoModerationRule => "AUTO_MODERATION_RULE",
            Self::Unknown => "UNKNOWN",
        }
    }

    #[must_use]
    pub const fn from_action_code(code: u16) -> Self {
        match code {
            1 | 150 | 151 | 180 | 210..=213 => Self::Guild,
            10..=12 | 16 | 73 => Self::Channel,
            13..=15 => Self::ChannelOverwrite,
            20..=28 | 72 | 74 | 75 | 143..=146 => Self::User,
            30..=33 => Self::Role,
            40..=42 => Self::Invite,
            50..=52 => Self::Webhook,
            60..=62 => Self::Emoji,
            90..=92 => Self::Sticker,
            110..=112 => Self::Thread,
            140..=142 => Self::AutoModerationRule,
            _ => Self::Unknown,
        }
    }
}

#[must_use]
pub fn audit_action_label_ru(action_type: u16) -> String {
    match action_type {
        1 => "Обновление сервера".to_string(),
        10 => "Создание канала".to_string(),
        11 => "Изменение канала".to_string(),
        12 => "Удаление канала".to_string(),
        13 => "Создание прав доступа канала".to_string(),
        14 => "Изменение прав доступа канала".to_string(),
        15 => "Удаление прав доступа канала".to_string(),
        20 => "Исключение участника".to_string(),
        21 => "Очистка неактивных участников".to_string(),
        22 => "Бан участника".to_string(),
        23 => "Разбан участника".to_string(),
        24 => "Изменение участника".to_string(),
        25 => "Изменение ролей участника".to_string(),
        30 => "Создание роли".to_string(),
        31 => "Изменение роли".to_string(),
        32 => "Удаление роли".to_string(),
        40 => "Создание приглашения".to_string(),
        50 => "Создание вебхука".to_string(),
        72 => "Удаление сообщения".to_string(),
        73 => "Массовое удаление сообщений".to_string(),
        110 => "Создание ветки".to_string(),
        140 => "Создание правила автомодерации".to_string(),
        143 => "Блокировка сообщения автомодерацией".to_string(),
        145 => "Тайм-аут участника автомодерацией".to_string(),
        other => format!("Действие {other}"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditLogError {
    InvalidSnowflake(String),
    InvalidLimit(u8),
    EmptyReason,
    ReasonTooLong(usize),
    ReasonContainsControlChars,
    MissingPermission(&'static str),
    AgeGatedRestricted,
    MessageTooLong(usize),
    TooManyAttachments(usize),
    AttachmentTooLarge(u64),
    SearchOffsetOutOfBounds(u32),
    VolumeOutOfBounds(u16),
}

impl fmt::Display for AuditLogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for AuditLogError {}

pub fn validate_snowflake(raw: &str) -> Result<u64, AuditLogError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || !trimmed.bytes().all(|b| b.is_ascii_digit()) {
        return Err(AuditLogError::InvalidSnowflake(raw.to_string()));
    }
    let parsed = trimmed
        .parse::<u64>()
        .map_err(|_| AuditLogError::InvalidSnowflake(raw.to_string()))?;
    if parsed == 0 {
        return Err(AuditLogError::InvalidSnowflake(raw.to_string()));
    }
    Ok(parsed)
}

#[must_use]
pub const fn extract_snowflake_timestamp_ms(snowflake: u64) -> u64 {
    (snowflake >> 22).saturating_add(DISCORD_EPOCH_MS)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GuildAuditLogQuery {
    pub guild_id: String,
    pub user_id: Option<String>,
    pub action_type: Option<u16>,
    pub before: Option<String>,
    pub after: Option<String>,
    pub limit: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditLogRestRequest {
    pub method: &'static str,
    pub path: String,
    pub api_v9_path: String,
    pub limit: u8,
}

pub fn build_guild_audit_log_request(
    query: &GuildAuditLogQuery,
) -> Result<AuditLogRestRequest, AuditLogError> {
    validate_snowflake(&query.guild_id)?;
    let limit = query.limit.unwrap_or(DEFAULT_AUDIT_LOG_LIMIT);
    if !(MIN_AUDIT_LOG_LIMIT..=MAX_AUDIT_LOG_LIMIT).contains(&limit) {
        return Err(AuditLogError::InvalidLimit(limit));
    }
    let mut params = vec![format!("limit={limit}")];
    if let Some(uid) = &query.user_id {
        validate_snowflake(uid)?;
        params.push(format!("user_id={uid}"));
    }
    if let Some(act) = query.action_type {
        params.push(format!("action_type={act}"));
    }
    if let Some(before) = &query.before {
        validate_snowflake(before)?;
        params.push(format!("before={before}"));
    }
    if let Some(after) = &query.after {
        validate_snowflake(after)?;
        params.push(format!("after={after}"));
    }
    let path = format!("/guilds/{}/audit-logs?{}", query.guild_id, params.join("&"));
    Ok(AuditLogRestRequest {
        method: "GET",
        api_v9_path: format!("/api/v9{path}"),
        path,
        limit,
    })
}

pub fn validate_audit_log_reason_header(
    reason: &str,
) -> Result<(&'static str, String), AuditLogError> {
    let trimmed = reason.trim();
    if trimmed.is_empty() {
        return Err(AuditLogError::EmptyReason);
    }
    let char_count = trimmed.chars().count();
    if char_count > MAX_AUDIT_REASON_CHARS {
        return Err(AuditLogError::ReasonTooLong(char_count));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(AuditLogError::ReasonContainsControlChars);
    }
    Ok((HEADER_AUDIT_LOG_REASON, trimmed.to_string()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditChangeValue {
    Absent,
    Null,
    Bool(bool),
    Number(i64),
    Text(String),
    RoleList(Vec<(String, String)>),
    PermissionMask(u64),
}

#[must_use]
pub fn is_sensitive_change_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "token"
            | "access_token"
            | "refresh_token"
            | "authorization"
            | "secret"
            | "client_secret"
            | "password"
            | "webhook_token"
    )
}

#[must_use]
pub fn render_audit_change_value(key: &str, val: &AuditChangeValue) -> String {
    if is_sensitive_change_key(key) {
        return match val {
            AuditChangeValue::Absent | AuditChangeValue::Null => "нет".to_string(),
            _ => "[REDACTED]".to_string(),
        };
    }
    match val {
        AuditChangeValue::Absent | AuditChangeValue::Null => "нет".to_string(),
        AuditChangeValue::Bool(b) => b.to_string(),
        AuditChangeValue::Number(n) => n.to_string(),
        AuditChangeValue::Text(s) => {
            if s.contains("/webhooks/") || s.contains("mfa.") {
                "[REDACTED]".to_string()
            } else {
                s.clone()
            }
        }
        AuditChangeValue::RoleList(roles) => {
            if roles.is_empty() {
                "нет".to_string()
            } else {
                roles
                    .iter()
                    .map(|(id, name)| format!("{name} ({id})"))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        }
        AuditChangeValue::PermissionMask(bits) => format!("0x{bits:08x}"),
    }
}

#[must_use]
pub fn format_audit_change_diff_ru(
    key: &str,
    old_val: &AuditChangeValue,
    new_val: &AuditChangeValue,
) -> String {
    let old_str = render_audit_change_value(key, old_val);
    let new_str = render_audit_change_value(key, new_val);
    format!("{key}: {old_str} → {new_str}")
}

#[must_use]
pub fn format_audit_entry_summary_ru(
    action_type: u16,
    actor_name: Option<&str>,
    target_name: Option<&str>,
    reason: Option<&str>,
) -> String {
    let actor = actor_name
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("Неизвестный");
    let label = audit_action_label_ru(action_type);
    let target_part = target_name
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map_or_else(String::new, |t| format!(" ({t})"));
    let reason_part = reason
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map_or_else(String::new, |r| format!(" — Причина: {r}"));
    format!("{actor}: {label}{target_part}{reason_part}")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditChangeItem {
    pub key: String,
    pub old_value: AuditChangeValue,
    pub new_value: AuditChangeValue,
}

impl AuditChangeItem {
    #[must_use]
    pub fn formatted_ru(&self) -> String {
        format_audit_change_diff_ru(&self.key, &self.old_value, &self.new_value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditLogEntryRecord {
    pub id: u64,
    pub user_id: Option<String>,
    pub actor_name: String,
    pub target_id: Option<String>,
    pub target_name: Option<String>,
    pub action_type: u16,
    pub reason: Option<String>,
    pub timestamp_ms: u64,
    pub changes: Vec<AuditChangeItem>,
}

impl AuditLogEntryRecord {
    #[must_use]
    pub fn summary_ru(&self) -> String {
        format_audit_entry_summary_ru(
            self.action_type,
            Some(&self.actor_name),
            self.target_name.as_deref(),
            self.reason.as_deref(),
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuditLogFilterSpec {
    pub action_type: Option<u16>,
    pub category: Option<AuditActionCategory>,
    pub user_id: Option<String>,
    pub target_id: Option<String>,
    pub time_range_ms: Option<(u64, u64)>,
    pub search_query: Option<String>,
}

#[must_use]
pub fn matches_audit_filter(entry: &AuditLogEntryRecord, filter: &AuditLogFilterSpec) -> bool {
    if filter
        .action_type
        .is_some_and(|act| entry.action_type != act)
    {
        return false;
    }
    if filter
        .category
        .is_some_and(|cat| AuditActionCategory::from_action_code(entry.action_type) != cat)
    {
        return false;
    }
    if filter
        .user_id
        .as_deref()
        .is_some_and(|uid| entry.user_id.as_deref() != Some(uid))
    {
        return false;
    }
    if filter
        .target_id
        .as_deref()
        .is_some_and(|tid| entry.target_id.as_deref() != Some(tid))
    {
        return false;
    }
    if filter
        .time_range_ms
        .is_some_and(|(s, e)| entry.timestamp_ms < s || entry.timestamp_ms > e)
    {
        return false;
    }
    if let Some(q) = &filter.search_query {
        let needle = q.trim().to_lowercase();
        if !needle.is_empty() && !entry.summary_ru().to_lowercase().contains(&needle) {
            return false;
        }
    }
    true
}

#[must_use]
pub fn filter_audit_entries(
    entries: &[AuditLogEntryRecord],
    filter: &AuditLogFilterSpec,
) -> Vec<AuditLogEntryRecord> {
    entries
        .iter()
        .filter(|e| matches_audit_filter(e, filter))
        .cloned()
        .collect()
}

#[must_use]
pub const fn is_non_mergeable_action(action_type: u16) -> bool {
    matches!(
        action_type,
        16 | 21 | 26 | 27 | 28 | 33 | 40..=42 | 72..=75 | 121
    )
}

#[must_use]
pub fn can_merge_audit_entries(
    prev: &AuditLogEntryRecord,
    next: &AuditLogEntryRecord,
    merged_count: usize,
) -> bool {
    if is_non_mergeable_action(prev.action_type) || merged_count >= MAX_GROUPED_CHANGES {
        return false;
    }
    let same_keys = prev.action_type == next.action_type
        && prev.user_id == next.user_id
        && prev.target_id == next.target_id;
    let delta_ms = prev.timestamp_ms.abs_diff(next.timestamp_ms);
    same_keys && delta_ms < MERGE_WINDOW_MS
}

#[must_use]
pub fn resolve_actor_name(user_id: Option<&str>, users: &BTreeMap<String, String>) -> String {
    user_id
        .and_then(|uid| users.get(uid))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Неизвестный".to_string())
}

pub fn check_audit_log_view_permission(
    permissions: u64,
    is_owner: bool,
    age_gated_restricted: bool,
) -> Result<(), AuditLogError> {
    if age_gated_restricted {
        return Err(AuditLogError::AgeGatedRestricted);
    }
    if is_owner || (permissions & PERM_ADMINISTRATOR) != 0 {
        return Ok(());
    }
    if (permissions & PERM_VIEW_AUDIT_LOG) == 0 {
        return Err(AuditLogError::MissingPermission("VIEW_AUDIT_LOG"));
    }
    Ok(())
}

pub fn validate_system_boundaries(
    message_chars: usize,
    attachments_count: usize,
    attachment_bytes: u64,
    search_offset: u32,
    volume_pct: u16,
) -> Result<(), AuditLogError> {
    if message_chars > MAX_MESSAGE_CHARS {
        return Err(AuditLogError::MessageTooLong(message_chars));
    }
    if attachments_count > MAX_ATTACHMENTS_COUNT {
        return Err(AuditLogError::TooManyAttachments(attachments_count));
    }
    if attachment_bytes > MAX_ATTACHMENT_BYTES {
        return Err(AuditLogError::AttachmentTooLarge(attachment_bytes));
    }
    if search_offset > MAX_SEARCH_OFFSET {
        return Err(AuditLogError::SearchOffsetOutOfBounds(search_offset));
    }
    if volume_pct > MAX_VOLUME_PERCENT {
        return Err(AuditLogError::VolumeOutOfBounds(volume_pct));
    }
    Ok(())
}

#[must_use]
pub fn format_badge_cap(count: u32) -> String {
    if count > MAX_BADGE_CAP {
        "99+".to_string()
    } else {
        count.to_string()
    }
}

#[must_use]
pub fn redact_audit_diagnostic(raw: &str) -> String {
    let mut out = raw.to_string();
    for marker in ["mfa.", "Bearer "] {
        if let Some(pos) = out.find(marker) {
            let start = pos + marker.len();
            let end = out[start..]
                .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
                .map_or(out.len(), |i| start + i);
            out.replace_range(start..end, "[REDACTED]");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(id: u64, uid: &str, actor: &str, act: u16, target: &str) -> AuditLogEntryRecord {
        AuditLogEntryRecord {
            id,
            user_id: Some(uid.to_string()),
            actor_name: actor.to_string(),
            target_id: Some("1294815650186788916".to_string()),
            target_name: Some(target.to_string()),
            action_type: act,
            reason: Some("Проверка".to_string()),
            timestamp_ms: extract_snowflake_timestamp_ms(id),
            changes: Vec::new(),
        }
    }

    #[test]
    fn test_sub_1008_1_oracle_constants_and_interactive_states() {
        assert_eq!(AUDIT_LOG_VIEWER_CONTAINER, "Audit Log Viewer");
        assert_eq!(AUDIT_LOG_VIEWER_DIMENSIONS, "100% x 100%");
        assert_eq!(
            (AUDIT_LOG_VIEWER_PADDING_PX, AUDIT_LOG_VIEWER_SPACING_PX),
            (16, 8)
        );
        assert_eq!(
            (TOKEN_BACKGROUND, TOKEN_SURFACE, TOKEN_TEXT_PRIMARY),
            ("#313338", "#2b2d31", "#ffffff")
        );
        assert_eq!((TOKEN_ACCENT, TOKEN_BORDER), ("#5865f2", "#3f4147"));
        assert_eq!((PERM_VIEW_AUDIT_LOG, PERM_ADMINISTRATOR), (1 << 7, 1 << 3));
        assert_eq!(DEFAULT_AUDIT_LOG_LIMIT, 50);
        assert_eq!(
            AuditEntryInteractiveState::resolve(false, false).as_str(),
            "entry row"
        );
        assert_eq!(
            AuditEntryInteractiveState::resolve(false, true).as_str(),
            "expand diff"
        );
        assert_eq!(
            AuditEntryInteractiveState::resolve(true, false).as_str(),
            "expanded details"
        );
        assert_eq!(AuditDecodeState::Raw.as_str(), "raw");
        assert_eq!(AuditDecodeState::Decoding.as_str(), "decoding");
        assert_eq!(AuditDecodeState::HumanReadable.as_str(), "human readable");
    }

    #[test]
    fn test_sub_1008_2_rest_query_builder_and_action_classification() {
        let req_default = build_guild_audit_log_request(&GuildAuditLogQuery {
            guild_id: "942355007271997450".to_string(),
            ..Default::default()
        })
        .unwrap();
        assert_eq!((req_default.method, req_default.limit), ("GET", 50));
        assert_eq!(
            req_default.path,
            "/guilds/942355007271997450/audit-logs?limit=50"
        );

        let req_filtered = build_guild_audit_log_request(&GuildAuditLogQuery {
            guild_id: "942355007271997450".to_string(),
            user_id: Some("479659568322969611".to_string()),
            action_type: Some(22),
            before: Some("1558237752917758108".to_string()),
            limit: Some(25),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(
            req_filtered.path,
            "/guilds/942355007271997450/audit-logs?limit=25&user_id=479659568322969611&action_type=22&before=1558237752917758108"
        );
        assert_eq!(
            AuditActionCategory::from_action_code(10),
            AuditActionCategory::Create
        );
        assert_eq!(
            AuditActionCategory::from_action_code(22),
            AuditActionCategory::Delete
        );
        assert_eq!(
            AuditActionCategory::from_action_code(31),
            AuditActionCategory::Update
        );
        assert_eq!(AuditTargetType::from_action_code(22), AuditTargetType::User);
        assert_eq!(
            AuditTargetType::from_action_code(140),
            AuditTargetType::AutoModerationRule
        );
    }

    #[test]
    fn test_sub_1008_3_changes_diff_decoding_and_redaction() {
        let txt = |s: &str| AuditChangeValue::Text(s.to_string());
        assert_eq!(
            format_audit_change_diff_ru("name", &txt("старое"), &txt("новое")),
            "name: старое → новое"
        );
        assert_eq!(
            format_audit_change_diff_ru("topic", &AuditChangeValue::Null, &txt("тема")),
            "topic: нет → тема"
        );
        assert_eq!(
            format_audit_change_diff_ru("token", &AuditChangeValue::Absent, &txt("secret_tok")),
            "token: нет → [REDACTED]"
        );
        assert_eq!(
            format_audit_entry_summary_ru(22, Some("Модератор"), Some("Боб"), Some("спам")),
            "Модератор: Бан участника (Боб) — Причина: спам"
        );
        assert_eq!(
            format_audit_entry_summary_ru(30, None, Some("role"), None),
            "Неизвестный: Создание роли (role)"
        );
    }

    #[test]
    fn test_sub_1008_4_filtering_grouping_and_nine_dimensions() {
        let entries = vec![
            make_entry(1001, "479659568322969611", "Алиса", 11, "#общий"),
            make_entry(1000, "606781270382346250", "Модератор", 22, "Боб"),
        ];
        let by_action = filter_audit_entries(
            &entries,
            &AuditLogFilterSpec {
                action_type: Some(22),
                ..Default::default()
            },
        );
        assert_eq!(by_action.len(), 1);
        assert_eq!(by_action[0].action_type, 22);

        let by_user = filter_audit_entries(
            &entries,
            &AuditLogFilterSpec {
                user_id: Some("479659568322969611".to_string()),
                ..Default::default()
            },
        );
        assert_eq!(by_user.len(), 1);
        assert_eq!(by_user[0].actor_name, "Алиса");

        assert!(check_audit_log_view_permission(PERM_VIEW_AUDIT_LOG, false, false).is_ok());
        assert!(check_audit_log_view_permission(0, false, false).is_err());
        assert!(check_audit_log_view_permission(PERM_VIEW_AUDIT_LOG, false, true).is_err());
        assert!(validate_snowflake("0").is_err());
        assert!(validate_snowflake("18446744073709551615").is_ok());
        assert!(validate_system_boundaries(2_000, 10, 25 * 1024 * 1024, 9_975, 200).is_ok());
        assert!(validate_system_boundaries(2_001, 10, 25 * 1024 * 1024, 9_975, 200).is_err());
        assert_eq!(
            (format_badge_cap(99), format_badge_cap(100)),
            ("99".into(), "99+".into())
        );
        assert!(!redact_audit_diagnostic("Bearer mfa.secret99").contains("secret99"));
    }
}
