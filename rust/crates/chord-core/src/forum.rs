//! Парсер `GUILD_FORUM` (`type: 15`) из Gateway (`CHANNEL_CREATE` / `GUILD_CREATE`)
//! и REST-пагинация активных постов через `GET /channels/{id}/threads/active` (`TASK-1001A`).
//! Оракулы: `waves/kanban-board.json` (`TASK-1001A`), `docs/re/gateway-schema.json`,
//! `docs/re/gateway-schema.md`, `docs/re/gateway-protocol.json` и `rust/gui-spike/src/forum.rs`.

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::fmt;

pub const FORUM_PROTOCOL_CONTAINER: &str = "Core Protocol";
pub const FORUM_PROTOCOL_DIMENSIONS: &str = "Data Struct";
pub const FORUM_PROTOCOL_PADDING: &str = "0px";
pub const FORUM_PROTOCOL_SPACING: &str = "0px";
pub const TOKEN_BACKGROUND: &str = "#1e1f22";
pub const TOKEN_SURFACE: &str = "#2b2d31";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_BORDER: &str = "#3f4147";
pub const FORUM_PARSER_ELEMENT: &str = "forum-parser";
pub const FORUM_PARSER_STATE_IDLE: &str = "idle";
pub const FORUM_PARSER_STATE_ACTIVE: &str = "active";
pub const FORUM_PARSER_STATE_PARSED: &str = "parsed";
pub const CHANNEL_TYPE_GUILD_FORUM: u8 = 15;
pub const CHANNEL_TYPE_GUILD_MEDIA: u8 = 16;
pub const CHANNEL_TYPE_PUBLIC_THREAD: u8 = 11;
pub const CHANNEL_FLAG_REQUIRE_TAG: u64 = 1 << 4;
pub const THREAD_FLAG_PINNED: u64 = 1 << 1;
pub const PERM_VIEW_CHANNEL: u64 = 1 << 10;
pub const PERM_MANAGE_THREADS: u64 = 1 << 34;
pub const PERM_SEND_MESSAGES_IN_THREADS: u64 = 1 << 38;
pub const REST_ACTIVE_THREADS_ROUTE_TEMPLATE: &str = "/channels/{id}/threads/active";
pub const REST_CREATE_FORUM_POST_ROUTE_TEMPLATE: &str = "/channels/{id}/threads";
pub const GATEWAY_CAPTURE_FORUM_CHANNEL_COUNT: usize = 9;
pub const GATEWAY_SCHEMA_AVAILABLE_TAGS_NIL_COUNT: u32 = 67;
pub const GATEWAY_SCHEMA_CHANNEL_SAMPLE_TOTAL: u32 = 272;
pub const MAX_FORUM_AVAILABLE_TAGS: usize = 20;
pub const MAX_FORUM_APPLIED_TAGS: usize = 5;
pub const MAX_FORUM_TAG_NAME_CHARS: usize = 50;
pub const MAX_FORUM_TITLE_CHARS: usize = 100;
pub const MAX_FORUM_CONTENT_CHARS: usize = 2_000;
pub const FORUM_ACTIVE_PAGE_LIMIT: usize = 25;
pub const MAX_CACHED_ACTIVE_THREADS: usize = 200;
pub const ALLOWED_AUTO_ARCHIVE_MINUTES: [u32; 4] = [60, 1440, 4320, 10080];
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_REPLY_BADGE_CAP: u32 = 99;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForumProtocolError {
    UnsupportedChannelType(u8),
    InvalidSnowflake(&'static str),
    InvalidSortOrder(u8),
    InvalidForumLayout(u8),
    InvalidAutoArchiveDuration(u32),
    InvalidTitleLength(usize),
    InvalidContentLength(usize),
    TooManyAppliedTags(usize),
    TagRequiredByChannelFlag,
    UnknownTagId(u64),
    ModeratedTagForbidden(u64),
    ArchivedThreadInActiveResponse(u64),
    ParentChannelMismatch {
        expected: u64,
        actual: u64,
    },
    PaginationLoopDetected(u64),
    InvalidPageLimit(usize),
    AccessDenied(&'static str),
    BoundaryViolation(&'static str),
    HttpError {
        status: u16,
        retry_after_ms: Option<u64>,
    },
}

impl fmt::Display for ForumProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ForumProtocolError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForumParserState {
    Idle,
    Active,
    Parsed,
}

impl ForumParserState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => FORUM_PARSER_STATE_IDLE,
            Self::Active => FORUM_PARSER_STATE_ACTIVE,
            Self::Parsed => FORUM_PARSER_STATE_PARSED,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForumSortOrder {
    LatestActivity = 0,
    CreationDate = 1,
}

impl ForumSortOrder {
    pub const fn from_optional_code(code: Option<u8>) -> Result<Self, ForumProtocolError> {
        match code {
            None | Some(0) => Ok(Self::LatestActivity),
            Some(1) => Ok(Self::CreationDate),
            Some(other) => Err(ForumProtocolError::InvalidSortOrder(other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForumLayout {
    NotSet = 0,
    ListView = 1,
    GalleryView = 2,
}

impl ForumLayout {
    pub const fn from_optional_code(code: Option<u8>) -> Result<Self, ForumProtocolError> {
        match code {
            None | Some(0) => Ok(Self::NotSet),
            Some(1) => Ok(Self::ListView),
            Some(2) => Ok(Self::GalleryView),
            Some(other) => Err(ForumProtocolError::InvalidForumLayout(other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForumTagMatchSetting {
    MatchSome,
    MatchAll,
}

impl ForumTagMatchSetting {
    #[must_use]
    pub fn from_optional_str(raw: Option<&str>) -> Self {
        match raw {
            Some("match_all" | "all") => Self::MatchAll,
            _ => Self::MatchSome,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumReactionEmoji {
    pub emoji_id: Option<u64>,
    pub emoji_name: Option<String>,
}

impl ForumReactionEmoji {
    #[must_use]
    pub fn normalized(self) -> Option<Self> {
        let emoji_id = self.emoji_id.filter(|id| *id > 0);
        let emoji_name = self
            .emoji_name
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        (emoji_id.is_some() || emoji_name.is_some()).then_some(Self {
            emoji_id,
            emoji_name,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumTag {
    pub id: u64,
    pub name: String,
    pub emoji_id: Option<u64>,
    pub emoji_name: Option<String>,
    pub moderated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayForumChannelPayload {
    pub id: u64,
    pub guild_id: u64,
    pub channel_type: u8,
    pub name: String,
    pub flags: u64,
    pub nsfw: bool,
    pub available_tags: Vec<ForumTag>,
    pub default_reaction_emoji: Option<ForumReactionEmoji>,
    pub default_sort_order: Option<u8>,
    pub default_forum_layout: Option<u8>,
    pub default_tag_setting: Option<String>,
    pub default_auto_archive_duration: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumChannelSpec {
    pub id: u64,
    pub guild_id: u64,
    pub channel_type: u8,
    pub name: String,
    pub flags: u64,
    pub require_tag: bool,
    pub nsfw: bool,
    pub available_tags: Vec<ForumTag>,
    pub default_reaction_emoji: Option<ForumReactionEmoji>,
    pub default_sort_order: ForumSortOrder,
    pub default_forum_layout: ForumLayout,
    pub default_tag_setting: ForumTagMatchSetting,
    pub default_auto_archive_duration: u32,
    pub parser_state: ForumParserState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumActiveThread {
    pub id: u64,
    pub parent_id: u64,
    pub guild_id: u64,
    pub name: String,
    pub owner_id: u64,
    pub message_count: u32,
    pub applied_tags: Vec<u64>,
    pub flags: u64,
    pub archived: bool,
    pub locked: bool,
    pub created_at_ms: u64,
    pub last_activity_ms: u64,
}

impl ForumActiveThread {
    #[must_use]
    pub const fn is_pinned(&self) -> bool {
        (self.flags & THREAD_FLAG_PINNED) != 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveThreadsPageResponse {
    pub threads: Vec<ForumActiveThread>,
    pub has_more: bool,
    pub cursor_snowflake: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestActiveThreadsRequest {
    pub method: &'static str,
    pub path: String,
    pub query: String,
    pub channel_id: u64,
    pub before_cursor: Option<u64>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumRestClient {
    pub channel_id: u64,
    pub sort_order: ForumSortOrder,
    pub parser_state: ForumParserState,
    pub seen_cursors: Vec<u64>,
    pub threads: Vec<ForumActiveThread>,
    pub has_more: bool,
    pub pages_loaded: usize,
}

pub fn parse_snowflake_str(field: &'static str, raw: &str) -> Result<u64, ForumProtocolError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || !trimmed.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ForumProtocolError::InvalidSnowflake(field));
    }
    match trimmed.parse::<u64>() {
        Ok(id) if id > 0 => Ok(id),
        _ => Err(ForumProtocolError::InvalidSnowflake(field)),
    }
}

#[must_use]
pub fn normalize_available_tags(raw_tags: Vec<ForumTag>) -> Vec<ForumTag> {
    let mut normalized = Vec::new();
    for tag in raw_tags {
        if tag.id == 0 || normalized.iter().any(|e: &ForumTag| e.id == tag.id) {
            continue;
        }
        let clean_name: String = tag
            .name
            .trim()
            .chars()
            .take(MAX_FORUM_TAG_NAME_CHARS)
            .collect();
        if clean_name.is_empty() {
            continue;
        }
        normalized.push(ForumTag {
            id: tag.id,
            name: clean_name,
            emoji_id: tag.emoji_id.filter(|id| *id > 0),
            emoji_name: tag
                .emoji_name
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            moderated: tag.moderated,
        });
        if normalized.len() >= MAX_FORUM_AVAILABLE_TAGS {
            break;
        }
    }
    normalized
}

pub fn parse_gateway_forum_channel(
    payload: GatewayForumChannelPayload,
) -> Result<ForumChannelSpec, ForumProtocolError> {
    if payload.channel_type != CHANNEL_TYPE_GUILD_FORUM {
        return Err(ForumProtocolError::UnsupportedChannelType(
            payload.channel_type,
        ));
    }
    if payload.id == 0 || payload.guild_id == 0 {
        return Err(ForumProtocolError::InvalidSnowflake("id/guild_id"));
    }
    let sort_order = ForumSortOrder::from_optional_code(payload.default_sort_order)?;
    let forum_layout = ForumLayout::from_optional_code(payload.default_forum_layout)?;
    let tag_setting =
        ForumTagMatchSetting::from_optional_str(payload.default_tag_setting.as_deref());
    let archive_duration = payload.default_auto_archive_duration.unwrap_or(10_080);
    if !ALLOWED_AUTO_ARCHIVE_MINUTES.contains(&archive_duration) {
        return Err(ForumProtocolError::InvalidAutoArchiveDuration(
            archive_duration,
        ));
    }
    Ok(ForumChannelSpec {
        id: payload.id,
        guild_id: payload.guild_id,
        channel_type: payload.channel_type,
        name: payload.name.trim().to_string(),
        flags: payload.flags,
        require_tag: (payload.flags & CHANNEL_FLAG_REQUIRE_TAG) != 0,
        nsfw: payload.nsfw,
        available_tags: normalize_available_tags(payload.available_tags),
        default_reaction_emoji: payload
            .default_reaction_emoji
            .and_then(ForumReactionEmoji::normalized),
        default_sort_order: sort_order,
        default_forum_layout: forum_layout,
        default_tag_setting: tag_setting,
        default_auto_archive_duration: archive_duration,
        parser_state: ForumParserState::Parsed,
    })
}

pub fn validate_forum_tag_selection(
    channel: &ForumChannelSpec,
    applied_tags: &[u64],
    user_permissions: u64,
    age_verified: bool,
) -> Result<(), ForumProtocolError> {
    if (user_permissions & PERM_VIEW_CHANNEL) == 0 {
        return Err(ForumProtocolError::AccessDenied("VIEW_CHANNEL_REQUIRED"));
    }
    if (user_permissions & PERM_SEND_MESSAGES_IN_THREADS) == 0 {
        return Err(ForumProtocolError::AccessDenied(
            "SEND_MESSAGES_IN_THREADS_REQUIRED",
        ));
    }
    if channel.nsfw && !age_verified {
        return Err(ForumProtocolError::AccessDenied("AGE_GATED_SPACES"));
    }
    if applied_tags.len() > MAX_FORUM_APPLIED_TAGS {
        return Err(ForumProtocolError::TooManyAppliedTags(applied_tags.len()));
    }
    if channel.require_tag && applied_tags.is_empty() {
        return Err(ForumProtocolError::TagRequiredByChannelFlag);
    }
    let can_manage_threads = (user_permissions & PERM_MANAGE_THREADS) != 0;
    for &tag_id in applied_tags {
        let Some(tag) = channel.available_tags.iter().find(|t| t.id == tag_id) else {
            return Err(ForumProtocolError::UnknownTagId(tag_id));
        };
        if tag.moderated && !can_manage_threads {
            return Err(ForumProtocolError::ModeratedTagForbidden(tag_id));
        }
    }
    Ok(())
}

pub fn build_active_threads_request(
    channel_id: u64,
    before_cursor: Option<u64>,
    limit: usize,
) -> Result<RestActiveThreadsRequest, ForumProtocolError> {
    if channel_id == 0 {
        return Err(ForumProtocolError::InvalidSnowflake("channel_id"));
    }
    if limit == 0 || limit > FORUM_ACTIVE_PAGE_LIMIT {
        return Err(ForumProtocolError::InvalidPageLimit(limit));
    }
    if let Some(0) = before_cursor {
        return Err(ForumProtocolError::InvalidSnowflake("before_cursor"));
    }
    let path = format!("/channels/{channel_id}/threads/active");
    let query = match before_cursor {
        Some(cursor) => format!("?limit={limit}&before={cursor}"),
        None => format!("?limit={limit}"),
    };
    Ok(RestActiveThreadsRequest {
        method: "GET",
        path,
        query,
        channel_id,
        before_cursor,
        limit,
    })
}

pub fn sort_active_forum_threads(threads: &mut [ForumActiveThread], sort_order: ForumSortOrder) {
    threads.sort_by(|a, b| {
        b.is_pinned()
            .cmp(&a.is_pinned())
            .then_with(|| {
                let a_ts = match sort_order {
                    ForumSortOrder::CreationDate => a.created_at_ms,
                    ForumSortOrder::LatestActivity => a.last_activity_ms.max(a.created_at_ms),
                };
                let b_ts = match sort_order {
                    ForumSortOrder::CreationDate => b.created_at_ms,
                    ForumSortOrder::LatestActivity => b.last_activity_ms.max(b.created_at_ms),
                };
                b_ts.cmp(&a_ts)
            })
            .then_with(|| b.id.cmp(&a.id))
    });
}

impl ForumRestClient {
    pub fn new(channel_id: u64, sort_order: ForumSortOrder) -> Result<Self, ForumProtocolError> {
        if channel_id == 0 {
            return Err(ForumProtocolError::InvalidSnowflake("channel_id"));
        }
        Ok(Self {
            channel_id,
            sort_order,
            parser_state: ForumParserState::Idle,
            seen_cursors: Vec::new(),
            threads: Vec::new(),
            has_more: true,
            pages_loaded: 0,
        })
    }

    pub fn next_page_request(&mut self) -> Result<RestActiveThreadsRequest, ForumProtocolError> {
        self.parser_state = ForumParserState::Active;
        build_active_threads_request(
            self.channel_id,
            self.seen_cursors.last().copied(),
            FORUM_ACTIVE_PAGE_LIMIT,
        )
    }

    pub fn ingest_active_threads_page(
        &mut self,
        page: ActiveThreadsPageResponse,
    ) -> Result<usize, ForumProtocolError> {
        let cursor = page
            .cursor_snowflake
            .or_else(|| page.threads.last().map(|t| t.id));
        if let Some(cursor) = cursor {
            if cursor == 0 {
                return Err(ForumProtocolError::InvalidSnowflake("cursor_snowflake"));
            }
            if self.seen_cursors.contains(&cursor) {
                return Err(ForumProtocolError::PaginationLoopDetected(cursor));
            }
            self.seen_cursors.push(cursor);
        }
        let mut added = 0usize;
        for thread in page.threads.into_iter().take(FORUM_ACTIVE_PAGE_LIMIT) {
            if thread.id == 0 {
                return Err(ForumProtocolError::InvalidSnowflake("thread.id"));
            }
            if thread.archived {
                return Err(ForumProtocolError::ArchivedThreadInActiveResponse(
                    thread.id,
                ));
            }
            if thread.parent_id != self.channel_id {
                return Err(ForumProtocolError::ParentChannelMismatch {
                    expected: self.channel_id,
                    actual: thread.parent_id,
                });
            }
            if !self.threads.iter().any(|e| e.id == thread.id)
                && self.threads.len() < MAX_CACHED_ACTIVE_THREADS
            {
                self.threads.push(thread);
                added += 1;
            }
        }
        sort_active_forum_threads(&mut self.threads, self.sort_order);
        self.pages_loaded += 1;
        self.has_more = page.has_more && self.threads.len() < MAX_CACHED_ACTIVE_THREADS;
        self.parser_state = ForumParserState::Parsed;
        Ok(added)
    }
}

pub fn classify_forum_http_status(
    http_status: u16,
    retry_after_ms: Option<u64>,
) -> Result<&'static str, ForumProtocolError> {
    if http_status == 202 {
        return Ok("skeleton");
    }
    if (200..300).contains(&http_status) {
        return Ok("ok");
    }
    Err(ForumProtocolError::HttpError {
        status: http_status,
        retry_after_ms: (http_status == 429).then_some(retry_after_ms.unwrap_or(1_500)),
    })
}

#[must_use]
pub fn format_reply_count_badge(reply_count: u32) -> String {
    if reply_count > MAX_REPLY_BADGE_CAP {
        format!("{MAX_REPLY_BADGE_CAP}+")
    } else {
        reply_count.to_string()
    }
}

pub fn validate_forum_protocol_boundaries(
    snowflake_id: u64,
    title_chars: usize,
    content_chars: usize,
    attachments_count: usize,
    attachment_bytes: u64,
    search_offset: u32,
    volume_pct: u16,
) -> Result<(), ForumProtocolError> {
    if snowflake_id == 0 {
        return Err(ForumProtocolError::InvalidSnowflake("snowflake_id"));
    }
    if !(1..=MAX_FORUM_TITLE_CHARS).contains(&title_chars) {
        return Err(ForumProtocolError::InvalidTitleLength(title_chars));
    }
    if !(1..=MAX_FORUM_CONTENT_CHARS).contains(&content_chars) {
        return Err(ForumProtocolError::InvalidContentLength(content_chars));
    }
    if attachments_count > MAX_ATTACHMENTS_COUNT
        || attachment_bytes > MAX_ATTACHMENT_BYTES
        || search_offset > MAX_SEARCH_OFFSET
        || volume_pct > MAX_VOLUME_PERCENT
    {
        return Err(ForumProtocolError::BoundaryViolation(
            "FORUM_PROTOCOL_BOUNDARY_EXCEEDED",
        ));
    }
    Ok(())
}

#[must_use]
pub fn redact_forum_protocol_log(raw_log: &str) -> String {
    raw_log
        .split_whitespace()
        .map(|token| {
            let clean = token.trim_matches(|c: char| c == '"' || c == '\'' || c == ',');
            let is_mfa = clean.starts_with("mfa.") && clean.len() > 8;
            let dot_parts: Vec<&str> = clean.split('.').collect();
            let is_jwt = dot_parts.len() == 3
                && dot_parts[0].len() >= 16
                && dot_parts[1].len() >= 4
                && dot_parts[2].len() >= 16;
            if is_mfa || is_jwt {
                "[REDACTED_TOKEN]"
            } else {
                token
            }
        })
        .collect::<Vec<&str>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tag(id: u64, name: &str, emoji_id: Option<u64>, moderated: bool) -> ForumTag {
        ForumTag {
            id,
            name: name.to_string(),
            emoji_id,
            emoji_name: Some("🔥".to_string()),
            moderated,
        }
    }

    fn sample_gateway_payload() -> GatewayForumChannelPayload {
        GatewayForumChannelPayload {
            id: 1_100_220_033_004_400_55,
            guild_id: 274_093_052_673_392_641,
            channel_type: CHANNEL_TYPE_GUILD_FORUM,
            name: "  rust-core-forum  ".to_string(),
            flags: CHANNEL_FLAG_REQUIRE_TAG,
            nsfw: false,
            available_tags: vec![
                sample_tag(101, "  Bug Report  ", None, false),
                sample_tag(102, "Release Note", Some(900_001), true),
                sample_tag(101, "Duplicate Id", None, false),
            ],
            default_reaction_emoji: Some(ForumReactionEmoji {
                emoji_id: None,
                emoji_name: Some("🔥".to_string()),
            }),
            default_sort_order: Some(1),
            default_forum_layout: Some(2),
            default_tag_setting: Some("match_all".to_string()),
            default_auto_archive_duration: Some(4320),
        }
    }

    fn sample_active_thread(
        id: u64,
        parent_id: u64,
        pinned: bool,
        created: u64,
        active: u64,
    ) -> ForumActiveThread {
        ForumActiveThread {
            id,
            parent_id,
            guild_id: 274_093_052_673_392_641,
            name: format!("Thread #{id}"),
            owner_id: 384_299_970_632_876_032,
            message_count: 14,
            applied_tags: vec![101],
            flags: if pinned { THREAD_FLAG_PINNED } else { 0 },
            archived: false,
            locked: false,
            created_at_ms: created,
            last_activity_ms: active,
        }
    }

    #[test]
    fn test_sub_1001a_1_oracle_constants_and_design_tokens() {
        assert_eq!(FORUM_PROTOCOL_CONTAINER, "Core Protocol");
        assert_eq!(FORUM_PROTOCOL_DIMENSIONS, "Data Struct");
        assert_eq!(FORUM_PROTOCOL_PADDING, "0px");
        assert_eq!(FORUM_PROTOCOL_SPACING, "0px");
        assert_eq!(TOKEN_BACKGROUND, "#1e1f22");
        assert_eq!(TOKEN_SURFACE, "#2b2d31");
        assert_eq!(TOKEN_TEXT_PRIMARY, "#ffffff");
        assert_eq!(TOKEN_ACCENT, "#5865f2");
        assert_eq!(TOKEN_BORDER, "#3f4147");
        assert_eq!(FORUM_PARSER_ELEMENT, "forum-parser");
        assert_eq!(ForumParserState::Idle.as_str(), "idle");
        assert_eq!(ForumParserState::Active.as_str(), "active");
        assert_eq!(ForumParserState::Parsed.as_str(), "parsed");
        assert_eq!(CHANNEL_TYPE_GUILD_FORUM, 15);
        assert_eq!(GATEWAY_CAPTURE_FORUM_CHANNEL_COUNT, 9);
        assert_eq!(GATEWAY_SCHEMA_AVAILABLE_TAGS_NIL_COUNT, 67);
        assert_eq!(GATEWAY_SCHEMA_CHANNEL_SAMPLE_TOTAL, 272);
    }

    #[test]
    fn test_sub_1001a_2_gateway_forum_parser_and_moderated_tags() {
        let Ok(spec) = parse_gateway_forum_channel(sample_gateway_payload()) else {
            panic!("valid GUILD_FORUM payload must parse");
        };
        assert_eq!(spec.channel_type, 15);
        assert_eq!(spec.name, "rust-core-forum");
        assert!(spec.require_tag);
        assert_eq!(spec.default_sort_order, ForumSortOrder::CreationDate);
        assert_eq!(spec.default_forum_layout, ForumLayout::GalleryView);
        assert_eq!(spec.default_tag_setting, ForumTagMatchSetting::MatchAll);
        assert_eq!(spec.default_auto_archive_duration, 4320);
        assert_eq!(spec.parser_state, ForumParserState::Parsed);
        assert_eq!(spec.available_tags.len(), 2);
        assert!(!spec.available_tags[0].moderated);
        assert!(spec.available_tags[1].moderated);
        assert_eq!(spec.available_tags[1].emoji_id, Some(900_001));

        let wrong = GatewayForumChannelPayload {
            channel_type: 0,
            ..sample_gateway_payload()
        };
        assert_eq!(
            parse_gateway_forum_channel(wrong),
            Err(ForumProtocolError::UnsupportedChannelType(0))
        );

        let base_perms = PERM_VIEW_CHANNEL | PERM_SEND_MESSAGES_IN_THREADS;
        assert_eq!(
            validate_forum_tag_selection(&spec, &[102], base_perms, true),
            Err(ForumProtocolError::ModeratedTagForbidden(102))
        );
        assert_eq!(
            validate_forum_tag_selection(&spec, &[102], base_perms | PERM_MANAGE_THREADS, true),
            Ok(())
        );
    }

    #[test]
    fn test_sub_1001a_3_rest_active_threads_pagination_and_loop_guard() {
        let ch_id = 1_100_220_033_004_400_55;
        let Ok(mut client) = ForumRestClient::new(ch_id, ForumSortOrder::LatestActivity) else {
            panic!("valid client");
        };
        assert_eq!(client.parser_state, ForumParserState::Idle);
        let Ok(req1) = client.next_page_request() else {
            panic!("req1")
        };
        assert_eq!(req1.method, "GET");
        assert_eq!(req1.path, format!("/channels/{ch_id}/threads/active"));
        assert_eq!(req1.query, "?limit=25");
        assert_eq!(client.parser_state, ForumParserState::Active);

        let page1 = ActiveThreadsPageResponse {
            threads: vec![
                sample_active_thread(5001, ch_id, false, 100, 400),
                sample_active_thread(5002, ch_id, true, 80, 200),
            ],
            has_more: true,
            cursor_snowflake: Some(5001),
        };
        assert_eq!(client.ingest_active_threads_page(page1), Ok(2));
        assert_eq!(client.parser_state, ForumParserState::Parsed);
        assert_eq!(client.threads[0].id, 5002);
        assert_eq!(client.threads[1].id, 5001);

        let Ok(req2) = client.next_page_request() else {
            panic!("req2")
        };
        assert_eq!(req2.query, "?limit=25&before=5001");

        let dup = ActiveThreadsPageResponse {
            threads: vec![sample_active_thread(5003, ch_id, false, 120, 600)],
            has_more: false,
            cursor_snowflake: Some(5001),
        };
        assert_eq!(
            client.ingest_active_threads_page(dup),
            Err(ForumProtocolError::PaginationLoopDetected(5001))
        );
    }

    #[test]
    fn test_sub_1001a_4_nine_dimensions_and_redaction() {
        assert_eq!(
            parse_snowflake_str("id", "0"),
            Err(ForumProtocolError::InvalidSnowflake("id"))
        );
        assert_eq!(
            parse_snowflake_str("id", "18446744073709551615"),
            Ok(u64::MAX)
        );
        let ok_bounds = validate_forum_protocol_boundaries(
            u64::MAX,
            MAX_FORUM_TITLE_CHARS,
            MAX_FORUM_CONTENT_CHARS,
            MAX_ATTACHMENTS_COUNT,
            MAX_ATTACHMENT_BYTES,
            MAX_SEARCH_OFFSET,
            MAX_VOLUME_PERCENT,
        );
        assert_eq!(ok_bounds, Ok(()));
        assert_eq!(format_reply_count_badge(99), "99");
        assert_eq!(format_reply_count_badge(100), "99+");
        assert_eq!(classify_forum_http_status(200, None), Ok("ok"));
        assert_eq!(classify_forum_http_status(202, None), Ok("skeleton"));
        let err_429 = ForumProtocolError::HttpError {
            status: 429,
            retry_after_ms: Some(2_500),
        };
        assert_eq!(classify_forum_http_status(429, Some(2_500)), Err(err_429));
        let fake_mfa = format!("mfa.{}", "secret_forum_token_1001a");
        let fake_jwt = format!(
            "{}.{}.{}",
            "MTIzNDU2Nzg5MDEyMzQ1Njc4", "GhIjKl", "MnOpQrStUvWxYz0123456789abcdef"
        );
        let raw_log = format!("GET /channels/110/threads/active {fake_mfa} {fake_jwt}");
        let redacted = redact_forum_protocol_log(&raw_log);
        assert!(!redacted.contains("secret_forum_token_1001a"));
        assert!(!redacted.contains("MTIzNDU2Nzg5MDEyMzQ1Njc4"));
        assert!(redacted.contains("[REDACTED_TOKEN]"));
    }
}
