//! Форумные каналы `GUILD_FORUM` (`type: 15`), фильтрация по тегам, сетка карточек постов
//! `ForumGridView` и счётчики ответов/реакций (`TASK-1001`).
//! Оракулы: `docs/re/gateway-schema.json`, `docs/re/gateway-schema.md`, `rust/gui-spike/src/forum.rs`.

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fmt;

pub const CHANNEL_TYPE_GUILD_FORUM: u8 = 15;
pub const CHANNEL_TYPE_GUILD_MEDIA: u8 = 16;
pub const CHANNEL_TYPE_PUBLIC_THREAD: u8 = 11;
pub const CHANNEL_FLAG_REQUIRE_TAG: u64 = 1 << 4;

pub const FORUM_GRID_CONTAINER: &str = "Forum Grid";
pub const FORUM_GRID_DIMENSIONS: &str = "100% x 100%";
pub const FORUM_GRID_PADDING_PX: u32 = 16;
pub const FORUM_GRID_SPACING_PX: u32 = 12;
pub const FORUM_CARD_HEIGHT_PX: u32 = 96;
pub const FORUM_AVATAR_SIZE_PX: u32 = 20;
pub const FORUM_TITLE_FONT_PX: u32 = 15;
pub const FORUM_PREVIEW_THUMB_PX: u32 = 72;
pub const FORUM_GALLERY_MIN_WIDTH_PX: u32 = 600;

pub const TOKEN_BG: &str = "#313338";
pub const TOKEN_SURFACE: &str = "#2b2d31";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_BORDER: &str = "#3f4147";

pub const MAX_FORUM_AVAILABLE_TAGS: usize = 20;
pub const MAX_FORUM_APPLIED_TAGS: usize = 5;
pub const MAX_CARD_VISIBLE_TAGS: usize = 3;
pub const MAX_TAG_NAME_CHARS: usize = 50;
pub const MAX_FORUM_TITLE_CHARS: usize = 100;
pub const MAX_FORUM_CONTENT_CHARS: usize = 2_000;
pub const FORUM_PAGE_SIZE: usize = 25;
pub const MAX_FORUM_CACHED_POSTS: usize = 200;
pub const ALLOWED_AUTO_ARCHIVE_MINUTES: [u32; 4] = [60, 1440, 4320, 10080];

pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_BADGE_CAP: u32 = 99;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForumError {
    UnsupportedChannelType(u8),
    InvalidSnowflake(&'static str),
    InvalidAutoArchiveDuration(u32),
    InvalidTitleLength(usize),
    InvalidContentLength(usize),
    TooManyAppliedTags(usize),
    TagRequiredByChannelFlag,
    UnknownTagId(u64),
    ModeratedTagForbidden(u64),
    ArchivedThreadInActivePage(u64),
    ParentChannelMismatch {
        expected: u64,
        actual: u64,
    },
    PaginationLoopDetected(u64),
    HttpError {
        status: u16,
        retry_after_ms: Option<u64>,
    },
    AccessDenied(&'static str),
    BoundaryViolation(&'static str),
}

impl fmt::Display for ForumError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ForumError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForumSortOrder {
    LatestActivity = 0,
    CreationDate = 1,
}

impl ForumSortOrder {
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn from_u8(code: u8) -> Self {
        match code {
            1 => Self::CreationDate,
            _ => Self::LatestActivity,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForumLayoutView {
    List = 1,
    Gallery = 2,
}

impl ForumLayoutView {
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn from_u8(code: u8) -> Self {
        match code {
            2 => Self::Gallery,
            _ => Self::List,
        }
    }

    #[must_use]
    pub const fn columns_for_width(self, viewport_width_px: u32) -> usize {
        match self {
            Self::Gallery if viewport_width_px >= FORUM_GALLERY_MIN_WIDTH_PX => 2,
            _ => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagMatchMode {
    Any,
    All,
}

impl TagMatchMode {
    #[must_use]
    pub fn from_setting(setting: Option<&str>) -> Self {
        match setting {
            Some("match_all" | "all") => Self::All,
            _ => Self::Any,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForumPostCardState {
    DefaultSurface,
    HoverBorderAccent,
    ActiveOpened,
    SkeletonLoading,
}

impl ForumPostCardState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DefaultSurface => "surface",
            Self::HoverBorderAccent => "border-accent",
            Self::ActiveOpened => "opened",
            Self::SkeletonLoading => "skeleton",
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
    pub fn is_valid(&self) -> bool {
        self.emoji_id.is_some_and(|id| id > 0)
            || self
                .emoji_name
                .as_ref()
                .is_some_and(|n| !n.trim().is_empty())
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
pub struct AttachmentPreview {
    pub attachment_id: u64,
    pub url: String,
    pub width: u32,
    pub height: u32,
    pub is_image: bool,
    pub spoiler: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumPostReaction {
    pub emoji: ForumReactionEmoji,
    pub count: u32,
    pub me: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumPost {
    pub id: u64,
    pub parent_channel_id: u64,
    pub title: String,
    pub author_id: u64,
    pub author_name: String,
    pub created_at_ms: u64,
    pub last_activity_ms: u64,
    pub applied_tags: Vec<u64>,
    pub pinned: bool,
    pub archived: bool,
    pub locked: bool,
    pub message_count: u32,
    pub reactions: Vec<ForumPostReaction>,
    pub first_attachment: Option<AttachmentPreview>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumChannelConfig {
    pub channel_id: u64,
    pub guild_id: u64,
    pub name: String,
    pub channel_type: u8,
    pub flags: u64,
    pub available_tags: Vec<ForumTag>,
    pub default_reaction_emoji: Option<ForumReactionEmoji>,
    pub default_sort_order: ForumSortOrder,
    pub default_forum_layout: ForumLayoutView,
    pub default_tag_setting: TagMatchMode,
    pub default_auto_archive_duration: u32,
    pub nsfw: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumCardRenderSpec {
    pub post_id: u64,
    pub title: String,
    pub title_font_px: u32,
    pub author_name: String,
    pub avatar_size_px: u32,
    pub card_height_px: u32,
    pub visible_tag_ids: Vec<u64>,
    pub overflow_tag_count: usize,
    pub overflow_badge: Option<String>,
    pub reply_count: u32,
    pub reply_badge_text: String,
    pub reaction_count: u32,
    pub has_thumbnail_preview: bool,
    pub thumbnail_size_px: u32,
    pub state: ForumPostCardState,
    pub background_token: &'static str,
    pub border_token: &'static str,
}

pub fn normalize_forum_tags(raw_tags: Vec<ForumTag>) -> Vec<ForumTag> {
    let mut normalized = Vec::new();
    for tag in raw_tags {
        let duplicate = normalized
            .iter()
            .any(|existing: &ForumTag| existing.id == tag.id);
        if tag.id == 0 || duplicate {
            continue;
        }
        let trimmed_name: String = tag.name.trim().chars().take(MAX_TAG_NAME_CHARS).collect();
        if trimmed_name.is_empty() {
            continue;
        }
        normalized.push(ForumTag {
            id: tag.id,
            name: trimmed_name,
            emoji_id: tag.emoji_id.filter(|id| *id > 0),
            emoji_name: tag.emoji_name.filter(|name| !name.trim().is_empty()),
            moderated: tag.moderated,
        });
        if normalized.len() >= MAX_FORUM_AVAILABLE_TAGS {
            break;
        }
    }
    normalized
}

pub fn parse_forum_channel(
    channel_id: u64,
    guild_id: u64,
    name: &str,
    channel_type: u8,
    flags: u64,
    tags: Vec<ForumTag>,
    sort_code: Option<u8>,
    layout_code: Option<u8>,
    tag_setting: Option<&str>,
    auto_archive: Option<u32>,
) -> Result<ForumChannelConfig, ForumError> {
    if channel_type != CHANNEL_TYPE_GUILD_FORUM {
        return Err(ForumError::UnsupportedChannelType(channel_type));
    }
    if channel_id == 0 || guild_id == 0 {
        return Err(ForumError::InvalidSnowflake("channel_id/guild_id"));
    }
    let archive_mins = auto_archive.unwrap_or(10_080);
    if !ALLOWED_AUTO_ARCHIVE_MINUTES.contains(&archive_mins) {
        return Err(ForumError::InvalidAutoArchiveDuration(archive_mins));
    }
    Ok(ForumChannelConfig {
        channel_id,
        guild_id,
        name: name.trim().to_string(),
        channel_type,
        flags,
        available_tags: normalize_forum_tags(tags),
        default_reaction_emoji: None,
        default_sort_order: ForumSortOrder::from_u8(sort_code.unwrap_or(0)),
        default_forum_layout: ForumLayoutView::from_u8(layout_code.unwrap_or(1)),
        default_tag_setting: TagMatchMode::from_setting(tag_setting),
        default_auto_archive_duration: archive_mins,
        nsfw: false,
    })
}

#[must_use]
pub fn post_matches_tags(
    post: &ForumPost,
    available_tag_ids: &[u64],
    selected_tag_ids: &[u64],
    match_mode: TagMatchMode,
) -> bool {
    if selected_tag_ids.is_empty() {
        return true;
    }
    if selected_tag_ids
        .iter()
        .any(|id| !available_tag_ids.contains(id))
    {
        return false;
    }
    match match_mode {
        TagMatchMode::All => selected_tag_ids
            .iter()
            .all(|id| post.applied_tags.contains(id)),
        TagMatchMode::Any => selected_tag_ids
            .iter()
            .any(|id| post.applied_tags.contains(id)),
    }
}

#[must_use]
pub fn filter_and_sort_posts(
    posts: &[ForumPost],
    available_tag_ids: &[u64],
    selected_tag_ids: &[u64],
    match_mode: TagMatchMode,
    search_query: &str,
    sort_order: ForumSortOrder,
) -> Vec<ForumPost> {
    let needle = search_query.trim().to_lowercase();
    let mut filtered: Vec<ForumPost> = posts
        .iter()
        .filter(|post| !post.archived)
        .filter(|post| post_matches_tags(post, available_tag_ids, selected_tag_ids, match_mode))
        .filter(|post| needle.is_empty() || post.title.to_lowercase().contains(&needle))
        .cloned()
        .collect();

    filtered.sort_by(|a, b| {
        let pinned_ord = b.pinned.cmp(&a.pinned);
        pinned_ord
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
    filtered
}

#[must_use]
pub fn count_posts_by_tag(posts: &[ForumPost]) -> BTreeMap<u64, usize> {
    let mut counts = BTreeMap::new();
    for post in posts.iter().filter(|p| !p.archived) {
        let mut seen = Vec::new();
        for &tag_id in &post.applied_tags {
            if tag_id != 0 && !seen.contains(&tag_id) {
                seen.push(tag_id);
                *counts.entry(tag_id).or_insert(0) += 1;
            }
        }
    }
    counts
}

#[must_use]
pub fn format_reply_badge(reply_count: u32) -> String {
    if reply_count > MAX_BADGE_CAP {
        format!("{MAX_BADGE_CAP}+")
    } else {
        reply_count.to_string()
    }
}

#[must_use]
pub fn build_card_render_spec(post: &ForumPost, state: ForumPostCardState) -> ForumCardRenderSpec {
    let visible_tag_ids: Vec<u64> = post
        .applied_tags
        .iter()
        .copied()
        .take(MAX_CARD_VISIBLE_TAGS)
        .collect();
    let overflow_tag_count = post
        .applied_tags
        .len()
        .saturating_sub(MAX_CARD_VISIBLE_TAGS);
    let overflow_badge = (overflow_tag_count > 0).then(|| format!("+{overflow_tag_count}"));
    let reaction_count: u32 = post.reactions.iter().map(|r| r.count).sum();
    let has_thumbnail_preview = post.first_attachment.as_ref().is_some_and(|att| {
        att.is_image
            && !att.spoiler
            && !att.url.trim().is_empty()
            && att.width > 0
            && att.height > 0
    });
    let border_token = match state {
        ForumPostCardState::HoverBorderAccent | ForumPostCardState::ActiveOpened => TOKEN_ACCENT,
        ForumPostCardState::DefaultSurface | ForumPostCardState::SkeletonLoading => TOKEN_BORDER,
    };
    ForumCardRenderSpec {
        post_id: post.id,
        title: post.title.clone(),
        title_font_px: FORUM_TITLE_FONT_PX,
        author_name: post.author_name.clone(),
        avatar_size_px: FORUM_AVATAR_SIZE_PX,
        card_height_px: FORUM_CARD_HEIGHT_PX,
        visible_tag_ids,
        overflow_tag_count,
        overflow_badge,
        reply_count: post.message_count,
        reply_badge_text: format_reply_badge(post.message_count),
        reaction_count,
        has_thumbnail_preview,
        thumbnail_size_px: FORUM_PREVIEW_THUMB_PX,
        state,
        background_token: TOKEN_SURFACE,
        border_token,
    }
}

pub fn validate_new_forum_post(
    config: &ForumChannelConfig,
    title: &str,
    content: &str,
    applied_tags: &[u64],
    can_create_posts: bool,
    can_manage_threads: bool,
    age_verified: bool,
) -> Result<(), ForumError> {
    if !can_create_posts {
        return Err(ForumError::AccessDenied(
            "SEND_MESSAGES_IN_THREADS_REQUIRED",
        ));
    }
    if config.nsfw && !age_verified {
        return Err(ForumError::AccessDenied("AGE_GATED_SPACES"));
    }
    let title_len = title.trim().chars().count();
    if !(1..=MAX_FORUM_TITLE_CHARS).contains(&title_len) {
        return Err(ForumError::InvalidTitleLength(title_len));
    }
    let content_len = content.trim().chars().count();
    if !(1..=MAX_FORUM_CONTENT_CHARS).contains(&content_len) {
        return Err(ForumError::InvalidContentLength(content_len));
    }
    if applied_tags.len() > MAX_FORUM_APPLIED_TAGS {
        return Err(ForumError::TooManyAppliedTags(applied_tags.len()));
    }
    if (config.flags & CHANNEL_FLAG_REQUIRE_TAG) != 0 && applied_tags.is_empty() {
        return Err(ForumError::TagRequiredByChannelFlag);
    }
    for &tag_id in applied_tags {
        let Some(tag) = config.available_tags.iter().find(|t| t.id == tag_id) else {
            return Err(ForumError::UnknownTagId(tag_id));
        };
        if tag.moderated && !can_manage_threads {
            return Err(ForumError::ModeratedTagForbidden(tag_id));
        }
    }
    Ok(())
}

pub fn paginate_active_threads(
    page_threads: Vec<ForumPost>,
    parent_channel_id: u64,
    has_more: bool,
    seen_cursors: &mut Vec<u64>,
    current_total: usize,
) -> Result<(Vec<ForumPost>, bool, String), ForumError> {
    if parent_channel_id == 0 {
        return Err(ForumError::InvalidSnowflake("parent_channel_id"));
    }
    let route = format!("/channels/{parent_channel_id}/threads/active");
    if let Some(last) = page_threads.last() {
        if seen_cursors.contains(&last.id) {
            return Err(ForumError::PaginationLoopDetected(last.id));
        }
        seen_cursors.push(last.id);
    }
    let mut accepted = Vec::new();
    for post in page_threads.into_iter().take(FORUM_PAGE_SIZE) {
        if post.archived {
            return Err(ForumError::ArchivedThreadInActivePage(post.id));
        }
        if post.parent_channel_id != parent_channel_id {
            return Err(ForumError::ParentChannelMismatch {
                expected: parent_channel_id,
                actual: post.parent_channel_id,
            });
        }
        if current_total + accepted.len() < MAX_FORUM_CACHED_POSTS {
            accepted.push(post);
        }
    }
    let more_allowed = has_more && (current_total + accepted.len() < MAX_FORUM_CACHED_POSTS);
    Ok((accepted, more_allowed, route))
}

#[must_use]
pub fn navigate_grid_focus(
    focused_index: usize,
    total_cards: usize,
    columns: usize,
    key: &str,
) -> (usize, Option<usize>) {
    if total_cards == 0 {
        return (0, None);
    }
    let cols = columns.max(1);
    let current = focused_index.min(total_cards - 1);
    match key {
        "ArrowRight" => ((current + 1).min(total_cards - 1), None),
        "ArrowLeft" => (current.saturating_sub(1), None),
        "ArrowDown" => ((current + cols).min(total_cards - 1), None),
        "ArrowUp" => (current.saturating_sub(cols), None),
        "Home" => (0, None),
        "End" => (total_cards - 1, None),
        "Enter" | "Space" => (current, Some(current)),
        _ => (current, None),
    }
}

pub fn validate_forum_boundary_dimensions(
    snowflake_id: u64,
    message_chars: usize,
    attachments: usize,
    attachment_bytes: u64,
    search_offset: u32,
    volume_pct: u16,
    http_status: u16,
    has_access: bool,
) -> Result<&'static str, ForumError> {
    if !has_access {
        return Err(ForumError::AccessDenied("FORUM_VIEW_CHANNEL_REQUIRED"));
    }
    if http_status == 202 {
        return Ok("skeleton");
    }
    if matches!(http_status, 401 | 403 | 404 | 413 | 429) || http_status >= 500 {
        return Err(ForumError::HttpError {
            status: http_status,
            retry_after_ms: (http_status == 429).then_some(1_500),
        });
    }
    if message_chars > MAX_FORUM_CONTENT_CHARS
        || attachments > MAX_ATTACHMENTS_COUNT
        || attachment_bytes > MAX_ATTACHMENT_BYTES
        || search_offset > MAX_SEARCH_OFFSET
        || volume_pct > MAX_VOLUME_PERCENT
    {
        return Err(ForumError::BoundaryViolation("FORUM_BOUNDARY_EXCEEDED"));
    }
    let _ = snowflake_id;
    Ok("ok")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_post(
        id: u64,
        title: &str,
        tags: Vec<u64>,
        pinned: bool,
        created: u64,
        active: u64,
    ) -> ForumPost {
        ForumPost {
            id,
            parent_channel_id: 110,
            title: title.to_string(),
            author_id: 7000 + id,
            author_name: format!("user_{id}"),
            created_at_ms: created,
            last_activity_ms: active,
            applied_tags: tags,
            pinned,
            archived: false,
            locked: false,
            message_count: 12,
            reactions: vec![ForumPostReaction {
                emoji: ForumReactionEmoji {
                    emoji_id: None,
                    emoji_name: Some("🔥".to_string()),
                },
                count: 5,
                me: false,
            }],
            first_attachment: None,
        }
    }

    #[test]
    fn test_sub_1001_1_forum_channel_parser_and_tokens() {
        assert_eq!(CHANNEL_TYPE_GUILD_FORUM, 15);
        assert_eq!(FORUM_GRID_CONTAINER, "Forum Grid");
        assert_eq!(FORUM_GRID_DIMENSIONS, "100% x 100%");
        assert_eq!(FORUM_GRID_PADDING_PX, 16);
        assert_eq!(FORUM_GRID_SPACING_PX, 12);
        assert_eq!(FORUM_CARD_HEIGHT_PX, 96);
        assert_eq!(TOKEN_BG, "#313338");
        assert_eq!(TOKEN_SURFACE, "#2b2d31");
        assert_eq!(TOKEN_TEXT_PRIMARY, "#ffffff");
        assert_eq!(TOKEN_ACCENT, "#5865f2");
        assert_eq!(TOKEN_BORDER, "#3f4147");

        let tag = ForumTag {
            id: 102,
            name: "Announcement".to_string(),
            emoji_id: Some(9001),
            emoji_name: Some("shipit".to_string()),
            moderated: true,
        };
        let cfg = parse_forum_channel(
            110,
            220,
            "forum",
            15,
            16,
            vec![tag],
            Some(0),
            Some(2),
            Some("all"),
            Some(4320),
        );
        assert!(cfg.is_ok());
        let bad = parse_forum_channel(110, 220, "text", 0, 0, Vec::new(), None, None, None, None);
        assert_eq!(bad, Err(ForumError::UnsupportedChannelType(0)));
    }

    #[test]
    fn test_sub_1001_2_tag_filtering_and_sorting() {
        let p1 = sample_post(1, "Shader bug on Wayland", vec![101], false, 100, 500);
        let p2 = sample_post(2, "Forum rules pinned", vec![101, 102], true, 50, 200);
        let p3 = sample_post(3, "Release v0.9.0", vec![102], false, 400, 900);
        let mut p4 = sample_post(4, "Archived thread", vec![101, 102], false, 800, 999);
        p4.archived = true;
        let posts = vec![p1, p2, p3, p4];

        let mode_any = TagMatchMode::Any;
        let sort_act = ForumSortOrder::LatestActivity;
        let sorted = filter_and_sort_posts(&posts, &[101, 102], &[], mode_any, "", sort_act);
        let ids: Vec<u64> = sorted.iter().map(|p| p.id).collect();
        assert_eq!(ids, vec![2, 3, 1]);

        let match_all = filter_and_sort_posts(
            &posts,
            &[101, 102],
            &[101, 102],
            TagMatchMode::All,
            "",
            sort_act,
        );
        assert_eq!(match_all.len(), 1);
        assert_eq!(match_all[0].id, 2);

        let counts = count_posts_by_tag(&posts);
        assert_eq!(counts.get(&101), Some(&2));
        assert_eq!(counts.get(&102), Some(&2));
    }

    #[test]
    fn test_sub_1001_3_post_grid_cards_and_keyboard_nav() {
        assert_eq!(ForumLayoutView::Gallery.columns_for_width(960), 2);
        assert_eq!(ForumLayoutView::Gallery.columns_for_width(480), 1);
        assert_eq!(ForumLayoutView::List.columns_for_width(1280), 1);

        let mut post = sample_post(10, "GPUI Grid Card", vec![1, 2, 3, 4, 5], true, 100, 200);
        post.message_count = 150;
        post.first_attachment = Some(AttachmentPreview {
            attachment_id: 77,
            url: "https://cdn.discordapp.com/attachments/1/77/img.png".to_string(),
            width: 320,
            height: 180,
            is_image: true,
            spoiler: false,
        });
        let spec = build_card_render_spec(&post, ForumPostCardState::ActiveOpened);
        assert_eq!(spec.card_height_px, 96);
        assert_eq!(spec.title_font_px, 15);
        assert_eq!(spec.avatar_size_px, 20);
        assert_eq!(spec.visible_tag_ids, vec![1, 2, 3]);
        assert_eq!(spec.overflow_tag_count, 2);
        assert_eq!(spec.overflow_badge.as_deref(), Some("+2"));
        assert_eq!(spec.reply_badge_text, "99+");
        assert!(spec.has_thumbnail_preview);
        assert_eq!(spec.border_token, "#5865f2");

        let (next_idx, opened) = navigate_grid_focus(0, 4, 2, "ArrowDown");
        assert_eq!(next_idx, 2);
        assert_eq!(opened, None);
        let (_, opened_enter) = navigate_grid_focus(next_idx, 4, 2, "Enter");
        assert_eq!(opened_enter, Some(2));
    }

    #[test]
    fn test_sub_1001_4_pagination_and_nine_dimensions() {
        let tag_mod = ForumTag {
            id: 102,
            name: "ModOnly".to_string(),
            emoji_id: None,
            emoji_name: None,
            moderated: true,
        };
        let Ok(cfg) = parse_forum_channel(
            110,
            220,
            "forum",
            15,
            16,
            vec![tag_mod],
            Some(0),
            Some(2),
            None,
            Some(1440),
        ) else {
            panic!("valid forum config");
        };
        let forbidden = validate_new_forum_post(&cfg, "Title", "Body", &[102], true, false, true);
        assert_eq!(forbidden, Err(ForumError::ModeratedTagForbidden(102)));
        assert!(validate_new_forum_post(&cfg, "Title", "Body", &[102], true, true, true).is_ok());

        let mut cursors = Vec::new();
        let p1 = sample_post(501, "Thread 1", vec![102], false, 10, 20);
        assert!(paginate_active_threads(vec![p1.clone()], 110, true, &mut cursors, 0).is_ok());
        let dup = paginate_active_threads(vec![p1], 110, true, &mut cursors, 1);
        assert_eq!(dup, Err(ForumError::PaginationLoopDetected(501)));

        let max_bytes = 25 * 1024 * 1024;
        assert_eq!(
            validate_forum_boundary_dimensions(0, 2_000, 10, max_bytes, 9_975, 200, 200, true),
            Ok("ok")
        );
        assert_eq!(
            validate_forum_boundary_dimensions(u64::MAX, 100, 1, 1024, 0, 100, 202, true),
            Ok("skeleton")
        );
    }
}
