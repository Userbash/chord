//! Жизненный цикл тредов: REST-создание `POST /channels/{id}/messages/{msg_id}/threads`,
//! модель `ThreadMetadata`, синхронизация `THREAD_MEMBERS_UPDATE` и таймеры
//! автоархивации `60..=10080` минут (`TASK-1002A`).

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub const CHANNEL_TYPE_PUBLIC_THREAD: u8 = 11;
pub const CHANNEL_TYPE_PRIVATE_THREAD: u8 = 12;
pub const MIN_AUTO_ARCHIVE_MINUTES: u32 = 60;
pub const MAX_AUTO_ARCHIVE_MINUTES: u32 = 10_080;
pub const ALLOWED_AUTO_ARCHIVE_DURATIONS: [u32; 4] = [60, 1440, 4320, 10080];
pub const ARCHIVING_WARNING_WINDOW_MINUTES: u64 = 5;
pub const MAX_THREAD_NAME_CHARS: usize = 100;
pub const REST_CREATE_MESSAGE_THREAD_TEMPLATE: &str = "/channels/{id}/messages/{msg_id}/threads";
pub const REST_CREATE_CHANNEL_THREAD_TEMPLATE: &str = "/channels/{id}/threads";
pub const REST_THREAD_SEARCH_PATH_D02: &str = "/api/v9/channels/{id}/threads/search";
pub const REST_THREAD_SEARCH_STATIC_KEY: &str = "THREAD_SEARCH";
pub const REST_MESSAGE_THREADS_STATIC_KEY: &str = "CHANNEL_MESSAGE_THREADS";
pub const D02_THREAD_SEARCH_REQUEST_ID: &str = "run2|95974.7947";
pub const THREADS_ENGINE_CONTAINER: &str = "Threads Engine";
pub const THREADS_ENGINE_DIMENSIONS: &str = "Backend Module";
pub const THREADS_ENGINE_PADDING: &str = "0px";
pub const THREADS_ENGINE_SPACING: &str = "0px";
pub const TOKEN_BG: &str = "#1e1f22";
pub const TOKEN_SURFACE: &str = "#2b2d31";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_BORDER: &str = "#3f4147";
pub const PERM_VIEW_CHANNEL: u64 = 1 << 10;
pub const PERM_MANAGE_THREADS: u64 = 1 << 34;
pub const PERM_CREATE_PUBLIC_THREADS: u64 = 1 << 35;
pub const PERM_CREATE_PRIVATE_THREADS: u64 = 1 << 36;
pub const PERM_SEND_MESSAGES_IN_THREADS: u64 = 1 << 38;
pub const MAX_MESSAGE_CHARS: usize = 2_000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_BADGE_CAP: u32 = 99;
pub const EMPTY_THREADS_PLACEHOLDER_RU: &str = "Ветки не найдены";
pub const ENCRYPTED_TOKEN_PREFIX: &str = "dQw4w9WgXcQ:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadChannelKind {
    PublicThread,
    PrivateThread,
}

impl ThreadChannelKind {
    pub fn from_channel_type(code: u8) -> Result<Self, ThreadLifecycleError> {
        match code {
            CHANNEL_TYPE_PUBLIC_THREAD => Ok(Self::PublicThread),
            CHANNEL_TYPE_PRIVATE_THREAD => Ok(Self::PrivateThread),
            _ => Err(ThreadLifecycleError::UnsupportedChannelType(code)),
        }
    }

    pub const fn code(self) -> u8 {
        match self {
            Self::PublicThread => CHANNEL_TYPE_PUBLIC_THREAD,
            Self::PrivateThread => CHANNEL_TYPE_PRIVATE_THREAD,
        }
    }

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::PublicThread => "PUBLIC_THREAD",
            Self::PrivateThread => "PRIVATE_THREAD",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoArchiveDuration {
    OneHour = 60,
    OneDay = 1440,
    ThreeDays = 4320,
    OneWeek = 10080,
}

impl AutoArchiveDuration {
    pub fn from_minutes(minutes: u32) -> Result<Self, ThreadLifecycleError> {
        if !(MIN_AUTO_ARCHIVE_MINUTES..=MAX_AUTO_ARCHIVE_MINUTES).contains(&minutes) {
            return Err(ThreadLifecycleError::InvalidAutoArchiveDuration(minutes));
        }
        match minutes {
            60 => Ok(Self::OneHour),
            1440 => Ok(Self::OneDay),
            4320 => Ok(Self::ThreeDays),
            10080 => Ok(Self::OneWeek),
            other => Err(ThreadLifecycleError::InvalidAutoArchiveDuration(other)),
        }
    }

    pub const fn minutes(self) -> u32 {
        self as u32
    }

    pub const fn label_ru(self) -> &'static str {
        match self {
            Self::OneHour => "1 ч.",
            Self::OneDay => "24 ч.",
            Self::ThreeDays => "3 дн.",
            Self::OneWeek => "1 нед.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadLifecycleState {
    Active,
    Archiving,
    Archived,
}

impl ThreadLifecycleState {
    pub const fn element_name() -> &'static str {
        "thread-lifecycle"
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archiving => "archiving",
            Self::Archived => "archived",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThreadLifecycleError {
    InvalidSnowflakeId(String),
    InvalidThreadName,
    UnsupportedChannelType(u8),
    InvalidAutoArchiveDuration(u32),
    MissingMessageIdForPublicStarter,
    ThreadNotFound(String),
    LockedThreadRequiresManageThreads,
    MissingPermission(&'static str),
    BoundaryViolation(&'static str),
    AccessDenied(&'static str),
    RestBadRequest,
    RestUnauthorized,
    RestForbidden,
    RestNotFound,
    RestPayloadTooLarge,
}

impl fmt::Display for ThreadLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ThreadLifecycleError {}

pub fn validate_snowflake_id(raw: &str) -> Result<u64, ThreadLifecycleError> {
    if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ThreadLifecycleError::InvalidSnowflakeId(raw.to_string()));
    }
    let parsed = raw
        .parse::<u64>()
        .map_err(|_| ThreadLifecycleError::InvalidSnowflakeId(raw.to_string()))?;
    if parsed == 0 {
        return Err(ThreadLifecycleError::InvalidSnowflakeId(raw.to_string()));
    }
    Ok(parsed)
}

pub fn validate_thread_name(name: &str) -> Result<String, ThreadLifecycleError> {
    let trimmed = name.trim();
    let count = trimmed.chars().count();
    if count == 0 || count > MAX_THREAD_NAME_CHARS {
        return Err(ThreadLifecycleError::InvalidThreadName);
    }
    Ok(trimmed.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThreadMetadata {
    pub archived: bool,
    pub auto_archive_duration: AutoArchiveDuration,
    pub archive_timestamp_ms: u64,
    pub locked: bool,
    pub invitable: bool,
    pub create_timestamp_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateThreadFromMessageRequest {
    pub channel_id: String,
    pub message_id: Option<String>,
    pub name: String,
    pub kind: ThreadChannelKind,
    pub auto_archive_duration: u32,
}

impl CreateThreadFromMessageRequest {
    pub fn new(
        ch: &str,
        msg: Option<&str>,
        name: &str,
        kind: ThreadChannelKind,
        auto_archive_duration: u32,
    ) -> Self {
        Self {
            channel_id: ch.to_string(),
            message_id: msg.map(str::to_string),
            name: name.to_string(),
            kind,
            auto_archive_duration,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestThreadRequestEnvelope {
    pub method: &'static str,
    pub path: String,
    pub json_body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadChannelRecord {
    pub id: String,
    pub guild_id: String,
    pub parent_id: String,
    pub starter_message_id: Option<String>,
    pub owner_id: String,
    pub name: String,
    pub kind: ThreadChannelKind,
    pub metadata: ThreadMetadata,
    pub message_count: u32,
    pub member_count: u32,
    pub unread_count: u32,
    pub last_activity_ms: u64,
    pub member_ids: Vec<String>,
    pub starter_message_deleted: bool,
}

impl ThreadChannelRecord {
    pub fn formatted_unread_badge(&self) -> String {
        if self.unread_count > MAX_BADGE_CAP {
            format!("{MAX_BADGE_CAP}+")
        } else {
            self.unread_count.to_string()
        }
    }

    pub fn indicator_summary_ru(&self, now_ms: u64) -> String {
        let elapsed_min = now_ms.saturating_sub(self.last_activity_ms) / 60_000;
        let pfx = if self.starter_message_deleted {
            "Исходное сообщение удалено • "
        } else {
            ""
        };
        format!(
            "{pfx}{} ответов • Последний {elapsed_min} мин назад",
            self.message_count
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThreadRestOutcome {
    Ok,
    LoadingSkeletonAccepted,
    RateLimited { retry_after_ms: u64 },
    ServerTransient { status_code: u16 },
}

#[derive(Default)]
pub struct ThreadLifecycleManager {
    threads: BTreeMap<String, ThreadChannelRecord>,
}

impl ThreadLifecycleManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn format_channel_threads_list_ru(&self, parent_id: &str, archived: bool) -> String {
        let mut items = Vec::new();
        for t in self.threads.values() {
            if t.parent_id == parent_id && t.metadata.archived == archived {
                let dur = t.metadata.auto_archive_duration.label_ru();
                items.push(format!("{} ({dur})", t.name));
            }
        }
        if items.is_empty() {
            EMPTY_THREADS_PLACEHOLDER_RU.to_string()
        } else {
            items.join(", ")
        }
    }

    pub fn check_create_permission(
        kind: ThreadChannelKind,
        permissions_bits: u64,
    ) -> Result<(), ThreadLifecycleError> {
        if (permissions_bits & PERM_VIEW_CHANNEL) == 0 {
            return Err(ThreadLifecycleError::MissingPermission("VIEW_CHANNEL"));
        }
        let has_manage = (permissions_bits & PERM_MANAGE_THREADS) != 0;
        let pub_ok = (permissions_bits & PERM_CREATE_PUBLIC_THREADS) != 0;
        let priv_ok = (permissions_bits & PERM_CREATE_PRIVATE_THREADS) != 0;
        let kind_ok = if kind == ThreadChannelKind::PublicThread {
            pub_ok
        } else {
            priv_ok
        };
        if !(has_manage || kind_ok) {
            return Err(ThreadLifecycleError::MissingPermission("CREATE_THREAD"));
        }
        Ok(())
    }

    pub fn build_create_thread_from_message_request(
        req: &CreateThreadFromMessageRequest,
    ) -> Result<RestThreadRequestEnvelope, ThreadLifecycleError> {
        let _ = validate_snowflake_id(&req.channel_id)?;
        let clean_name = validate_thread_name(&req.name)?;
        let duration = AutoArchiveDuration::from_minutes(req.auto_archive_duration)?;
        let path = if let Some(msg_id) = &req.message_id {
            let _ = validate_snowflake_id(msg_id)?;
            REST_CREATE_MESSAGE_THREAD_TEMPLATE
                .replace("{id}", &req.channel_id)
                .replace("{msg_id}", msg_id)
        } else if req.kind == ThreadChannelKind::PrivateThread {
            REST_CREATE_CHANNEL_THREAD_TEMPLATE.replace("{id}", &req.channel_id)
        } else {
            return Err(ThreadLifecycleError::MissingMessageIdForPublicStarter);
        };
        let json_body = format!(
            "{{\"name\":\"{clean_name}\",\"type\":{},\"auto_archive_duration\":{},\"location\":\"Message\"}}",
            req.kind.code(),
            duration.minutes()
        );
        Ok(RestThreadRequestEnvelope {
            method: "POST",
            path,
            json_body,
        })
    }

    pub fn create_thread_from_message(
        &mut self,
        thread_id: &str,
        guild_id: &str,
        owner_id: &str,
        req: &CreateThreadFromMessageRequest,
        permissions_bits: u64,
        now_ms: u64,
    ) -> Result<(ThreadChannelRecord, RestThreadRequestEnvelope), ThreadLifecycleError> {
        for id in [thread_id, guild_id, owner_id] {
            let _ = validate_snowflake_id(id)?;
        }
        Self::check_create_permission(req.kind, permissions_bits)?;
        let envelope = Self::build_create_thread_from_message_request(req)?;
        let duration = AutoArchiveDuration::from_minutes(req.auto_archive_duration)?;
        let record = ThreadChannelRecord {
            id: thread_id.to_string(),
            guild_id: guild_id.to_string(),
            parent_id: req.channel_id.clone(),
            starter_message_id: req.message_id.clone(),
            owner_id: owner_id.to_string(),
            name: validate_thread_name(&req.name)?,
            kind: req.kind,
            metadata: ThreadMetadata {
                archived: false,
                auto_archive_duration: duration,
                archive_timestamp_ms: now_ms,
                locked: false,
                invitable: req.kind == ThreadChannelKind::PublicThread,
                create_timestamp_ms: now_ms,
            },
            message_count: 0,
            member_count: 1,
            unread_count: 0,
            last_activity_ms: now_ms,
            member_ids: vec![owner_id.to_string()],
            starter_message_deleted: false,
        };
        self.threads.insert(thread_id.to_string(), record.clone());
        Ok((record, envelope))
    }

    pub fn set_thread_archived(
        &mut self,
        id: &str,
        archived: bool,
        locked: Option<bool>,
        permissions_bits: u64,
        now_ms: u64,
    ) -> Result<ThreadMetadata, ThreadLifecycleError> {
        let Some(t) = self.threads.get_mut(id) else {
            return Err(ThreadLifecycleError::ThreadNotFound(id.to_string()));
        };
        let has_manage = (permissions_bits & PERM_MANAGE_THREADS) != 0;
        if !archived {
            if t.metadata.locked && !has_manage {
                return Err(ThreadLifecycleError::LockedThreadRequiresManageThreads);
            }
            t.last_activity_ms = now_ms;
        }
        if let Some(next_locked) = locked {
            if !has_manage {
                return Err(ThreadLifecycleError::LockedThreadRequiresManageThreads);
            }
            t.metadata.locked = next_locked;
        }
        t.metadata.archived = archived;
        t.metadata.archive_timestamp_ms = now_ms;
        Ok(t.metadata)
    }

    pub fn evaluate_auto_archive_timeouts(&mut self, now_ms: u64) -> Vec<String> {
        let mut newly_archived = Vec::new();
        for t in self.threads.values_mut() {
            if t.metadata.archived {
                continue;
            }
            let elapsed_minutes = now_ms.saturating_sub(t.last_activity_ms) / 60_000;
            let duration_minutes = u64::from(t.metadata.auto_archive_duration.minutes());
            if elapsed_minutes >= duration_minutes {
                t.metadata.archived = true;
                t.metadata.archive_timestamp_ms = now_ms;
                newly_archived.push(t.id.clone());
            }
        }
        newly_archived
    }

    pub fn remaining_auto_archive_minutes(
        &self,
        id: &str,
        now_ms: u64,
    ) -> Result<u64, ThreadLifecycleError> {
        let Some(t) = self.threads.get(id) else {
            return Err(ThreadLifecycleError::ThreadNotFound(id.to_string()));
        };
        if t.metadata.archived {
            return Ok(0);
        }
        let elapsed_minutes = now_ms.saturating_sub(t.last_activity_ms) / 60_000;
        let duration_minutes = u64::from(t.metadata.auto_archive_duration.minutes());
        Ok(duration_minutes.saturating_sub(elapsed_minutes))
    }

    pub fn lifecycle_state_at(
        &self,
        id: &str,
        now_ms: u64,
    ) -> Result<ThreadLifecycleState, ThreadLifecycleError> {
        let Some(t) = self.threads.get(id) else {
            return Err(ThreadLifecycleError::ThreadNotFound(id.to_string()));
        };
        if t.metadata.archived {
            return Ok(ThreadLifecycleState::Archived);
        }
        let remaining = self.remaining_auto_archive_minutes(id, now_ms)?;
        if remaining == 0 {
            Ok(ThreadLifecycleState::Archived)
        } else if remaining <= ARCHIVING_WARNING_WINDOW_MINUTES {
            Ok(ThreadLifecycleState::Archiving)
        } else {
            Ok(ThreadLifecycleState::Active)
        }
    }

    pub fn append_thread_message(
        &mut self,
        id: &str,
        msg_id: &str,
        content: &str,
        ts_ms: u64,
        perms: u64,
    ) -> Result<u32, ThreadLifecycleError> {
        let _ = validate_snowflake_id(msg_id)?;
        if content.trim().is_empty() || content.chars().count() > MAX_MESSAGE_CHARS {
            return Err(ThreadLifecycleError::BoundaryViolation("message_length"));
        }
        if (perms & PERM_SEND_MESSAGES_IN_THREADS) == 0 {
            return Err(ThreadLifecycleError::MissingPermission("SEND_IN_THREADS"));
        }
        let Some(t) = self.threads.get_mut(id) else {
            return Err(ThreadLifecycleError::ThreadNotFound(id.to_string()));
        };
        let has_manage = (perms & PERM_MANAGE_THREADS) != 0;
        if t.metadata.locked && !has_manage {
            return Err(ThreadLifecycleError::LockedThreadRequiresManageThreads);
        }
        t.metadata.archived = false;
        t.last_activity_ms = ts_ms;
        t.message_count = t.message_count.saturating_add(1);
        Ok(t.message_count)
    }

    pub fn apply_thread_members_update(
        &mut self,
        id: &str,
        added: &[&str],
        removed: &[&str],
    ) -> Result<u32, ThreadLifecycleError> {
        let Some(t) = self.threads.get_mut(id) else {
            return Err(ThreadLifecycleError::ThreadNotFound(id.to_string()));
        };
        t.member_ids.retain(|uid| !removed.contains(&uid.as_str()));
        for &uid in added {
            let _ = validate_snowflake_id(uid)?;
            if !t.member_ids.iter().any(|existing| existing == uid) {
                t.member_ids.push(uid.to_string());
            }
        }
        t.member_count = t.member_ids.len() as u32;
        Ok(t.member_count)
    }

    pub fn apply_thread_list_sync(&mut self, parents: &[String], synced: Vec<ThreadChannelRecord>) {
        let scoped: BTreeSet<&str> = parents.iter().map(String::as_str).collect();
        let incoming: BTreeSet<String> = synced.iter().map(|t| t.id.clone()).collect();
        self.threads
            .retain(|id, t| !scoped.contains(t.parent_id.as_str()) || incoming.contains(id));
        for thread in synced {
            if scoped.contains(thread.parent_id.as_str()) {
                self.threads.insert(thread.id.clone(), thread);
            }
        }
    }
}

pub fn handle_rest_response_status(
    http_status: u16,
    retry_after_ms: Option<u64>,
) -> Result<ThreadRestOutcome, ThreadLifecycleError> {
    let retry_ms = retry_after_ms.unwrap_or(1_000).max(100);
    match http_status {
        200 | 201 | 204 => Ok(ThreadRestOutcome::Ok),
        202 => Ok(ThreadRestOutcome::LoadingSkeletonAccepted),
        400 => Err(ThreadLifecycleError::RestBadRequest),
        401 => Err(ThreadLifecycleError::RestUnauthorized),
        403 => Err(ThreadLifecycleError::RestForbidden),
        404 => Err(ThreadLifecycleError::RestNotFound),
        413 => Err(ThreadLifecycleError::RestPayloadTooLarge),
        429 => Ok(ThreadRestOutcome::RateLimited {
            retry_after_ms: retry_ms,
        }),
        500..=599 => Ok(ThreadRestOutcome::ServerTransient {
            status_code: http_status,
        }),
        _ => Err(ThreadLifecycleError::RestBadRequest),
    }
}

pub fn validate_workload_boundaries(
    sf: u64,
    msg_len: usize,
    att: (usize, u64),
    offset: u32,
    vol: u16,
    perms: u64,
    age: (bool, bool),
) -> Result<(), ThreadLifecycleError> {
    if sf == 0 || msg_len > MAX_MESSAGE_CHARS {
        return Err(ThreadLifecycleError::BoundaryViolation("snowflake_or_msg"));
    }
    if att.0 > MAX_ATTACHMENTS_COUNT || att.1 > MAX_ATTACHMENT_BYTES {
        return Err(ThreadLifecycleError::BoundaryViolation("attachment_limit"));
    }
    if offset > MAX_SEARCH_OFFSET || vol > MAX_VOLUME_PERCENT {
        return Err(ThreadLifecycleError::BoundaryViolation("offset_or_volume"));
    }
    if (perms & PERM_VIEW_CHANNEL) == 0 {
        return Err(ThreadLifecycleError::AccessDenied("missing_view_channel"));
    }
    if age.0 && !age.1 {
        return Err(ThreadLifecycleError::AccessDenied("age_gated_restricted"));
    }
    Ok(())
}

pub fn redact_thread_secrets(input: &str) -> String {
    let mut out = Vec::new();
    for token in input.split_whitespace() {
        let masked = if token.starts_with("mfa.") && token.len() > 8 {
            "mfa.***".to_string()
        } else if token.starts_with(ENCRYPTED_TOKEN_PREFIX) {
            format!("{ENCRYPTED_TOKEN_PREFIX}[REDACTED]")
        } else if token.starts_with("ghp_") && token.len() > 8 {
            "ghp_***".to_string()
        } else if token.len() >= 50 && token.matches('.').count() == 2 {
            "[REDACTED_TOKEN]".to_string()
        } else {
            token.to_string()
        };
        out.push(masked);
    }
    let s = out
        .join(" ")
        .replace("Bearer [REDACTED_TOKEN]", "Bearer ***");
    s.replace("Bot [REDACTED_TOKEN]", "Bot ***")
}

pub fn export_threads_telemetry_json(mgr: &ThreadLifecycleManager, note: &str) -> String {
    let safe_note = redact_thread_secrets(note);
    let total = mgr.threads.len();
    let archived_cnt = mgr.threads.values().filter(|t| t.metadata.archived).count();
    format!(
        "{{\"container\":\"{THREADS_ENGINE_CONTAINER}\",\"dimensions\":\"{THREADS_ENGINE_DIMENSIONS}\",\"threads\":{total},\"archived\":{archived_cnt},\"note\":\"{safe_note}\"}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ThreadChannelKind::{PrivateThread, PublicThread};
    use ThreadLifecycleError::*;
    type Req = CreateThreadFromMessageRequest;

    fn req(ch: &str, msg: Option<&str>, k: ThreadChannelKind, m: u32) -> Req {
        Req::new(ch, msg, "Thread", k, m)
    }

    #[test]
    fn test_sub_1002a_1_oracles_tokens_and_channel_types_contract() {
        assert_eq!(CHANNEL_TYPE_PUBLIC_THREAD, 11);
        assert_eq!(CHANNEL_TYPE_PRIVATE_THREAD, 12);
        assert_eq!(MIN_AUTO_ARCHIVE_MINUTES, 60);
        assert_eq!(MAX_AUTO_ARCHIVE_MINUTES, 10_080);
        assert_eq!(ALLOWED_AUTO_ARCHIVE_DURATIONS, [60, 1440, 4320, 10080]);
        assert_eq!(THREADS_ENGINE_CONTAINER, "Threads Engine");
        assert_eq!(THREADS_ENGINE_DIMENSIONS, "Backend Module");
        assert_eq!(THREADS_ENGINE_PADDING, "0px");
        assert_eq!(THREADS_ENGINE_SPACING, "0px");
        assert_eq!(TOKEN_BG, "#1e1f22");
        assert_eq!(TOKEN_SURFACE, "#2b2d31");
        assert_eq!(TOKEN_TEXT_PRIMARY, "#ffffff");
        assert_eq!(TOKEN_ACCENT, "#5865f2");
        assert_eq!(TOKEN_BORDER, "#3f4147");
        assert_eq!(
            REST_THREAD_SEARCH_PATH_D02,
            "/api/v9/channels/{id}/threads/search"
        );
        assert_eq!(REST_THREAD_SEARCH_STATIC_KEY, "THREAD_SEARCH");
        assert_eq!(REST_MESSAGE_THREADS_STATIC_KEY, "CHANNEL_MESSAGE_THREADS");
        assert_eq!(D02_THREAD_SEARCH_REQUEST_ID, "run2|95974.7947");
        let pub_k = ThreadChannelKind::from_channel_type(11).unwrap();
        let priv_k = ThreadChannelKind::from_channel_type(12).unwrap();
        assert_eq!(pub_k.wire_name(), "PUBLIC_THREAD");
        assert_eq!(priv_k.wire_name(), "PRIVATE_THREAD");
        assert!(ThreadChannelKind::from_channel_type(10).is_err());
        assert_eq!(ThreadLifecycleState::element_name(), "thread-lifecycle");
        assert_eq!(ThreadLifecycleState::Active.as_str(), "active");
        assert_eq!(ThreadLifecycleState::Archiving.as_str(), "archiving");
        assert_eq!(ThreadLifecycleState::Archived.as_str(), "archived");
    }

    #[test]
    fn test_sub_1002a_2_create_public_and_private_threads_and_members_sync() {
        let mut mgr = ThreadLifecycleManager::new();
        let (cid, mid) = ("1058348665342808104", "1100000000000000001");
        let pub_req = req(cid, Some(mid), PublicThread, 1440);
        let pub_p = PERM_VIEW_CHANNEL | PERM_CREATE_PUBLIC_THREADS;
        let r11 = mgr.create_thread_from_message("2001", "3001", "4001", &pub_req, pub_p, 1000);
        let (rec11, env11) = r11.unwrap();
        assert_eq!(rec11.kind.code(), 11);
        assert!(!rec11.metadata.archived);
        assert_eq!(rec11.metadata.auto_archive_duration.minutes(), 1440);
        assert_eq!(rec11.metadata.create_timestamp_ms, 1000);
        assert!(rec11.metadata.invitable);
        assert_eq!(
            env11.path,
            format!("/channels/{cid}/messages/{mid}/threads")
        );
        assert!(env11.json_body.contains("\"type\":11") && env11.json_body.contains("1440"));
        let priv_req = req(cid, None, PrivateThread, 4320);
        let denied = mgr.create_thread_from_message("2002", "3001", "4001", &priv_req, pub_p, 1000);
        assert!(denied.is_err());
        let priv_p = PERM_VIEW_CHANNEL | PERM_CREATE_PRIVATE_THREADS;
        let r12 = mgr.create_thread_from_message("2002", "3001", "4001", &priv_req, priv_p, 1000);
        let (rec12, env12) = r12.unwrap();
        assert_eq!(rec12.kind.code(), 12);
        assert!(!rec12.metadata.invitable);
        assert_eq!(env12.path, format!("/channels/{cid}/threads"));
        let members = mgr
            .apply_thread_members_update("2001", &["4002", "4002", "4003"], &["4001"])
            .unwrap();
        assert_eq!(members, 2);
    }

    #[test]
    fn test_sub_1002a_3_auto_archive_timers_and_lifecycle_states() {
        let mut mgr = ThreadLifecycleManager::new();
        let perms = PERM_VIEW_CHANNEL | PERM_CREATE_PUBLIC_THREADS;
        for (idx, mins) in ALLOWED_AUTO_ARCHIVE_DURATIONS.iter().enumerate() {
            let (tid, mid) = (format!("{}", 5000 + idx), format!("{}", 6000 + idx));
            let r = req("100", Some(&mid), PublicThread, *mins);
            let _ = mgr.create_thread_from_message(&tid, "1", "2", &r, perms, 0);
        }
        assert_eq!(
            mgr.lifecycle_state_at("5000", 10 * 60_000).unwrap(),
            ThreadLifecycleState::Active
        );
        assert_eq!(
            mgr.lifecycle_state_at("5000", 56 * 60_000).unwrap(),
            ThreadLifecycleState::Archiving
        );
        assert_eq!(
            mgr.remaining_auto_archive_minutes("5000", 56 * 60_000)
                .unwrap(),
            4
        );
        assert!(mgr.evaluate_auto_archive_timeouts(59 * 60_000).is_empty());
        for (i, m) in ALLOWED_AUTO_ARCHIVE_DURATIONS.iter().enumerate() {
            let exp = vec![format!("{}", 5000 + i)];
            assert_eq!(
                mgr.evaluate_auto_archive_timeouts(u64::from(*m) * 60_000),
                exp
            );
        }
        assert_eq!(
            mgr.lifecycle_state_at("5000", 60 * 60_000).unwrap(),
            ThreadLifecycleState::Archived
        );
        let _ = mgr.set_thread_archived("5000", true, Some(true), PERM_MANAGE_THREADS, 100);
        let unarch_err = mgr.set_thread_archived("5000", false, None, perms, 200);
        assert_eq!(unarch_err, Err(LockedThreadRequiresManageThreads));
    }

    #[test]
    fn test_sub_1002a_4_nine_dimensions_and_boundary_cases() {
        use ThreadRestOutcome::{LoadingSkeletonAccepted, RateLimited, ServerTransient};
        assert!(validate_snowflake_id("0").is_err());
        let max_sf = u64::MAX.to_string();
        assert_eq!(validate_snowflake_id(&max_sf).unwrap(), u64::MAX);
        assert!(validate_thread_name("").is_err());
        assert!(validate_thread_name(&"a".repeat(100)).is_ok());
        assert!(validate_thread_name(&"a".repeat(101)).is_err());
        for bad_min in [0, 30, 59, 61, 1439, 4319, 10079, 10081] {
            assert!(AutoArchiveDuration::from_minutes(bad_min).is_err());
        }
        assert_eq!(AutoArchiveDuration::OneHour.label_ru(), "1 ч.");
        assert_eq!(AutoArchiveDuration::OneDay.label_ru(), "24 ч.");
        assert_eq!(AutoArchiveDuration::ThreeDays.label_ru(), "3 дн.");
        assert_eq!(AutoArchiveDuration::OneWeek.label_ru(), "1 нед.");
        let f = handle_rest_response_status;
        assert_eq!(f(200, None), Ok(ThreadRestOutcome::Ok));
        assert_eq!(f(202, None), Ok(LoadingSkeletonAccepted));
        let rl = RateLimited {
            retry_after_ms: 2500,
        };
        assert_eq!(f(429, Some(2500)), Ok(rl));
        assert_eq!(f(502, None), Ok(ServerTransient { status_code: 502 }));
        assert_eq!(f(400, None), Err(RestBadRequest));
        assert_eq!(f(401, None), Err(RestUnauthorized));
        assert_eq!(f(403, None), Err(RestForbidden));
        assert_eq!(f(404, None), Err(RestNotFound));
        assert_eq!(f(413, None), Err(RestPayloadTooLarge));
    }

    #[test]
    fn test_sub_1002a_4_sync_empty_badges_and_secret_redaction() {
        let mut mgr = ThreadLifecycleManager::new();
        let empty = mgr.format_channel_threads_list_ru("999", false);
        assert_eq!(empty, EMPTY_THREADS_PLACEHOLDER_RU);
        let perms = PERM_VIEW_CHANNEL | PERM_CREATE_PUBLIC_THREADS | PERM_SEND_MESSAGES_IN_THREADS;
        let r = req("999", Some("1001"), PublicThread, 60);
        let (mut rec, _) = mgr
            .create_thread_from_message("7001", "1", "2", &r, perms, 0)
            .unwrap();
        rec.unread_count = 120;
        rec.starter_message_deleted = true;
        assert_eq!(rec.formatted_unread_badge(), "99+");
        assert!(rec.indicator_summary_ru(300_000).contains("5 мин назад"));
        mgr.apply_thread_list_sync(&["999".to_string()], vec![rec]);
        let cnt = mgr.append_thread_message("7001", "8001", &"x".repeat(2000), 120_000, perms);
        assert_eq!(cnt.unwrap(), 1);
        let (v, vp) = (validate_workload_boundaries, PERM_VIEW_CHANNEL);
        assert!(v(1, 2000, (10, 25 * 1024 * 1024), 9975, 200, vp, (true, true)).is_ok());
        assert!(v(0, 10, (1, 10), 0, 100, vp, (false, false)).is_err());
        assert!(v(1, 10, (1, 10), 0, 100, vp, (true, false)).is_err());
        let dyn_mfa = format!("mfa.{}", "b".repeat(84));
        let telemetry = export_threads_telemetry_json(&mgr, &format!("tok {dyn_mfa}"));
        assert!(telemetry.contains("mfa.***") && !telemetry.contains(&dyn_mfa));
    }
}
