//! Форумные каналы `GUILD_FORUM` (`type: 15`), фильтрация по тегам, сетка карточек постов
//! `ForumGridView` и счётчики ответов/реакций (`TASK-1001`, `TASK-1001B`).
//! Оракулы: `docs/re/gateway-schema.json`, `docs/re/frontend-d03.json`, `rust/gui-spike/src/forum.rs`.

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
pub const POST_CARD_ELEMENT: &str = "post-card";
pub const POST_CARD_STATE_DEFAULT: &str = "card";
pub const POST_CARD_STATE_HOVER: &str = "border-accent";
pub const POST_CARD_STATE_ACTIVE: &str = "open thread";
pub const BADGE_TAG_CONTAINER_CLASS: &str = "badge_e7c728";
pub const NUMBER_BADGE_CLASS: &str = "numberBadge__463b7";
pub const AVATAR_WRAPPER_CLASS: &str = "avatarWrapper_d8a370";
pub const SCROLLER_BASE_CLASS: &str = "scrollerBase_d125d2";
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
    ModeratedTagForbidden(u64),
    PaginationLoopDetected(u64),
    HttpError(u16),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForumLayoutView {
    List = 1,
    Gallery = 2,
}

impl ForumLayoutView {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForumPostCardState {
    DefaultSurface,
    HoverBorderAccent,
    ActiveOpened,
    SkeletonLoading,
}

impl ForumPostCardState {
    pub const fn as_post_card_state_str(self) -> &'static str {
        match self {
            Self::DefaultSurface | Self::SkeletonLoading => POST_CARD_STATE_DEFAULT,
            Self::HoverBorderAccent => POST_CARD_STATE_HOVER,
            Self::ActiveOpened => POST_CARD_STATE_ACTIVE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumReactionEmoji {
    pub emoji_id: Option<u64>,
    pub emoji_name: Option<String>,
}

impl ForumReactionEmoji {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumTagChipSpec {
    pub tag_id: u64,
    pub label: String,
    pub selected: bool,
    pub post_count: usize,
    pub background_token: &'static str,
    pub border_token: &'static str,
    pub text_token: &'static str,
    pub class_hint: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumGridCellSpec {
    pub row: usize,
    pub col: usize,
    pub card: ForumCardRenderSpec,
    pub post_card_state: &'static str,
    pub reaction_emoji: String,
    pub reacted_by_me: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumGridViewSpec {
    pub container: &'static str,
    pub dimensions: &'static str,
    pub padding_px: u32,
    pub spacing_px: u32,
    pub columns: usize,
    pub rows: usize,
    pub tag_chips: Vec<ForumTagChipSpec>,
    pub cells: Vec<ForumGridCellSpec>,
}

pub fn normalize_forum_tags(raw_tags: Vec<ForumTag>) -> Vec<ForumTag> {
    let mut normalized = Vec::new();
    for tag in raw_tags {
        if tag.id == 0 || normalized.iter().any(|t: &ForumTag| t.id == tag.id) {
            continue;
        }
        let name: String = tag.name.trim().chars().take(MAX_TAG_NAME_CHARS).collect();
        if !name.is_empty() && normalized.len() < MAX_FORUM_AVAILABLE_TAGS {
            normalized.push(ForumTag { name, ..tag });
        }
    }
    normalized
}

pub fn parse_forum_channel(
    channel_id: u64,
    guild_id: u64,
    channel_type: u8,
    flags: u64,
    tags: Vec<ForumTag>,
) -> Result<ForumChannelConfig, ForumError> {
    if channel_type != CHANNEL_TYPE_GUILD_FORUM {
        return Err(ForumError::UnsupportedChannelType(channel_type));
    }
    if channel_id == 0 || guild_id == 0 {
        return Err(ForumError::InvalidSnowflake("channel_id/guild_id"));
    }
    Ok(ForumChannelConfig {
        channel_id,
        guild_id,
        name: "forum".to_string(),
        channel_type,
        flags,
        available_tags: normalize_forum_tags(tags),
        default_reaction_emoji: None,
        default_sort_order: ForumSortOrder::LatestActivity,
        default_forum_layout: ForumLayoutView::Gallery,
        default_tag_setting: TagMatchMode::Any,
        default_auto_archive_duration: 4320,
        nsfw: false,
    })
}

pub fn post_matches_tags(
    post: &ForumPost,
    avail: &[u64],
    selected: &[u64],
    mode: TagMatchMode,
) -> bool {
    if selected.is_empty() {
        return true;
    }
    if selected.iter().any(|id| !avail.contains(id)) {
        return false;
    }
    match mode {
        TagMatchMode::All => selected.iter().all(|id| post.applied_tags.contains(id)),
        TagMatchMode::Any => selected.iter().any(|id| post.applied_tags.contains(id)),
    }
}

pub fn filter_and_sort_posts(
    posts: &[ForumPost],
    avail: &[u64],
    selected: &[u64],
    mode: TagMatchMode,
    query: &str,
    sort_order: ForumSortOrder,
) -> Vec<ForumPost> {
    let needle = query.trim().to_lowercase();
    let mut out: Vec<ForumPost> = posts
        .iter()
        .filter(|p| !p.archived && post_matches_tags(p, avail, selected, mode))
        .filter(|p| needle.is_empty() || p.title.to_lowercase().contains(&needle))
        .cloned()
        .collect();
    out.sort_by(|a, b| {
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
    out
}

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

pub fn toggle_tag_selection(selected: &[u64], tag_id: u64, avail: &[u64]) -> Vec<u64> {
    let mut next: Vec<u64> = Vec::new();
    for &id in selected {
        if id != 0 && avail.contains(&id) && !next.contains(&id) {
            next.push(id);
        }
    }
    if tag_id == 0 || !avail.contains(&tag_id) {
        return next;
    }
    if let Some(pos) = next.iter().position(|&id| id == tag_id) {
        next.remove(pos);
    } else if next.len() < MAX_FORUM_AVAILABLE_TAGS {
        next.push(tag_id);
    }
    next
}

pub fn build_tag_filter_bar(
    available_tags: &[ForumTag],
    selected_tags: &[u64],
    posts: &[ForumPost],
) -> Vec<ForumTagChipSpec> {
    let counts = count_posts_by_tag(posts);
    available_tags
        .iter()
        .map(|tag| {
            let selected = selected_tags.contains(&tag.id);
            let background_token = if selected {
                TOKEN_ACCENT
            } else {
                TOKEN_SURFACE
            };
            let border_token = if selected { TOKEN_ACCENT } else { TOKEN_BORDER };
            let label = match &tag.emoji_name {
                Some(e) if !e.trim().is_empty() => format!("{e} {}", tag.name),
                _ => tag.name.clone(),
            };
            ForumTagChipSpec {
                tag_id: tag.id,
                label,
                selected,
                post_count: counts.get(&tag.id).copied().unwrap_or(0),
                background_token,
                border_token,
                text_token: TOKEN_TEXT_PRIMARY,
                class_hint: BADGE_TAG_CONTAINER_CLASS,
            }
        })
        .collect()
}

pub fn compute_post_reaction_summary(
    post: &ForumPost,
    default_emoji: Option<&ForumReactionEmoji>,
) -> (String, u32, bool) {
    let mut total = 0_u32;
    let mut by_me = false;
    let mut top: Option<(u32, String)> = None;
    for r in &post.reactions {
        if !r.emoji.is_valid() || r.count == 0 {
            continue;
        }
        total = total.saturating_add(r.count);
        by_me |= r.me;
        let name = r
            .emoji
            .emoji_name
            .clone()
            .unwrap_or_else(|| format!("emoji:{}", r.emoji.emoji_id.unwrap_or(0)));
        if top.as_ref().is_none_or(|(best, _)| r.count > *best) {
            top = Some((r.count, name));
        }
    }
    let display = top
        .map(|(_, e)| e)
        .or_else(|| default_emoji.and_then(|e| e.emoji_name.clone()))
        .unwrap_or_else(|| "💬".to_string());
    (display, total, by_me)
}

pub fn format_reply_badge(reply_count: u32) -> String {
    if reply_count > MAX_BADGE_CAP {
        format!("{MAX_BADGE_CAP}+")
    } else {
        reply_count.to_string()
    }
}

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
    let has_thumbnail_preview = post.first_attachment.as_ref().is_some_and(|a| {
        a.is_image && !a.spoiler && !a.url.trim().is_empty() && a.width > 0 && a.height > 0
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

fn resolve_card_state(
    post_id: u64,
    hovered_post_id: Option<u64>,
    opened_post_id: Option<u64>,
) -> ForumPostCardState {
    if opened_post_id == Some(post_id) {
        ForumPostCardState::ActiveOpened
    } else if hovered_post_id == Some(post_id) {
        ForumPostCardState::HoverBorderAccent
    } else {
        ForumPostCardState::DefaultSurface
    }
}

pub fn build_forum_grid_view(
    config: &ForumChannelConfig,
    posts: &[ForumPost],
    selected_tags: &[u64],
    viewport_width_px: u32,
    hovered_post_id: Option<u64>,
    opened_post_id: Option<u64>,
) -> ForumGridViewSpec {
    let avail: Vec<u64> = config.available_tags.iter().map(|t| t.id).collect();
    let mode = config.default_tag_setting;
    let visible = filter_and_sort_posts(
        posts,
        &avail,
        selected_tags,
        mode,
        "",
        config.default_sort_order,
    );
    let columns = config
        .default_forum_layout
        .columns_for_width(viewport_width_px);
    let def_emoji = config.default_reaction_emoji.as_ref();
    let cells: Vec<ForumGridCellSpec> = visible
        .iter()
        .enumerate()
        .map(|(idx, post)| {
            let state = resolve_card_state(post.id, hovered_post_id, opened_post_id);
            let (emoji, _, by_me) = compute_post_reaction_summary(post, def_emoji);
            ForumGridCellSpec {
                row: idx / columns,
                col: idx % columns,
                card: build_card_render_spec(post, state),
                post_card_state: state.as_post_card_state_str(),
                reaction_emoji: emoji,
                reacted_by_me: by_me,
            }
        })
        .collect();
    ForumGridViewSpec {
        container: FORUM_GRID_CONTAINER,
        dimensions: FORUM_GRID_DIMENSIONS,
        padding_px: FORUM_GRID_PADDING_PX,
        spacing_px: FORUM_GRID_SPACING_PX,
        columns,
        rows: cells.len().div_ceil(columns),
        tag_chips: build_tag_filter_bar(&config.available_tags, selected_tags, posts),
        cells,
    }
}

pub fn validate_new_forum_post(
    config: &ForumChannelConfig,
    applied_tags: &[u64],
    can_manage_threads: bool,
) -> Result<(), ForumError> {
    for &tag_id in applied_tags {
        if let Some(tag) = config.available_tags.iter().find(|t| t.id == tag_id) {
            if tag.moderated && !can_manage_threads {
                return Err(ForumError::ModeratedTagForbidden(tag_id));
            }
        }
    }
    Ok(())
}

pub fn paginate_active_threads(
    page_threads: Vec<ForumPost>,
    parent_channel_id: u64,
    seen_cursors: &mut Vec<u64>,
) -> Result<(Vec<ForumPost>, String), ForumError> {
    if let Some(last) = page_threads.last() {
        if seen_cursors.contains(&last.id) {
            return Err(ForumError::PaginationLoopDetected(last.id));
        }
        seen_cursors.push(last.id);
    }
    Ok((
        page_threads.into_iter().take(FORUM_PAGE_SIZE).collect(),
        format!("/channels/{parent_channel_id}/threads/active"),
    ))
}

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
    message_chars: usize,
    attachment_bytes: u64,
    http_status: u16,
    has_access: bool,
) -> Result<&'static str, ForumError> {
    if !has_access {
        return Err(ForumError::AccessDenied("FORUM_VIEW_CHANNEL_REQUIRED"));
    }
    if http_status == 202 {
        return Ok("skeleton");
    }
    if http_status >= 400 {
        return Err(ForumError::HttpError(http_status));
    }
    if message_chars > MAX_FORUM_CONTENT_CHARS || attachment_bytes > MAX_ATTACHMENT_BYTES {
        return Err(ForumError::BoundaryViolation("FORUM_BOUNDARY_EXCEEDED"));
    }
    Ok("ok")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(id: u64, name: &str, emoji: Option<&str>, moderated: bool) -> ForumTag {
        ForumTag {
            id,
            name: name.to_string(),
            emoji_id: None,
            emoji_name: emoji.map(str::to_string),
            moderated,
        }
    }

    fn sample_post(id: u64, title: &str, tags: Vec<u64>, pinned: bool, a: u64) -> ForumPost {
        let emoji = ForumReactionEmoji {
            emoji_id: None,
            emoji_name: Some("🔥".to_string()),
        };
        ForumPost {
            id,
            parent_channel_id: 110,
            title: title.to_string(),
            author_id: 7000 + id,
            author_name: format!("user_{id}"),
            created_at_ms: 10,
            last_activity_ms: a,
            applied_tags: tags,
            pinned,
            archived: false,
            locked: false,
            message_count: 12,
            reactions: vec![ForumPostReaction {
                emoji,
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
        let t = tag(102, "News", Some("shipit"), true);
        assert!(parse_forum_channel(110, 220, 15, 16, vec![t]).is_ok());
        let bad = parse_forum_channel(110, 220, 0, 0, Vec::new());
        assert_eq!(bad, Err(ForumError::UnsupportedChannelType(0)));
    }

    #[test]
    fn test_sub_1001_2_tag_filtering_and_sorting() {
        let p1 = sample_post(1, "Shader bug on Wayland", vec![101], false, 500);
        let p2 = sample_post(2, "Forum rules pinned", vec![101, 102], true, 200);
        let p3 = sample_post(3, "Release v0.9.0", vec![102], false, 900);
        let mut p4 = sample_post(4, "Archived thread", vec![101, 102], false, 999);
        p4.archived = true;
        let posts = vec![p1, p2, p3, p4];
        let act = ForumSortOrder::LatestActivity;
        let sorted = filter_and_sort_posts(&posts, &[101, 102], &[], TagMatchMode::Any, "", act);
        let ids: Vec<u64> = sorted.iter().map(|p| p.id).collect();
        assert_eq!(ids, vec![2, 3, 1]);
        let all =
            filter_and_sort_posts(&posts, &[101, 102], &[101, 102], TagMatchMode::All, "", act);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, 2);
        let counts = count_posts_by_tag(&posts);
        assert_eq!(counts.get(&101), Some(&2));
        assert_eq!(counts.get(&102), Some(&2));
    }

    #[test]
    fn test_sub_1001_3_post_grid_cards_and_keyboard_nav() {
        assert_eq!(ForumLayoutView::Gallery.columns_for_width(960), 2);
        assert_eq!(ForumLayoutView::Gallery.columns_for_width(480), 1);
        assert_eq!(ForumLayoutView::List.columns_for_width(1280), 1);
        let mut post = sample_post(10, "GPUI Grid Card", vec![1, 2, 3, 4, 5], true, 200);
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
        assert!(spec.has_thumbnail_preview && spec.border_token == "#5865f2");
        assert_eq!(navigate_grid_focus(0, 4, 2, "ArrowDown"), (2, None));
        assert_eq!(navigate_grid_focus(2, 4, 2, "Enter"), (2, Some(2)));
    }

    #[test]
    fn test_sub_1001_4_pagination_and_nine_dimensions() {
        let t = tag(102, "ModOnly", None, true);
        let Ok(cfg) = parse_forum_channel(110, 220, 15, 16, vec![t]) else {
            panic!("valid forum config");
        };
        let err = validate_new_forum_post(&cfg, &[102], false);
        assert_eq!(err, Err(ForumError::ModeratedTagForbidden(102)));
        assert!(validate_new_forum_post(&cfg, &[102], true).is_ok());
        let mut cursors = Vec::new();
        let p1 = sample_post(501, "Thread 1", vec![102], false, 20);
        assert!(paginate_active_threads(vec![p1.clone()], 110, &mut cursors).is_ok());
        let dup = paginate_active_threads(vec![p1], 110, &mut cursors);
        assert_eq!(dup, Err(ForumError::PaginationLoopDetected(501)));
        let ok_res = validate_forum_boundary_dimensions(2_000, MAX_ATTACHMENT_BYTES, 200, true);
        assert_eq!(ok_res, Ok("ok"));
        assert_eq!(
            validate_forum_boundary_dimensions(100, 1024, 202, true),
            Ok("skeleton")
        );
    }

    #[test]
    fn test_sub_1001b_forum_grid_view_tags_and_reactions() {
        let tags = vec![
            tag(101, "Bug", Some("🐛"), false),
            tag(102, "Rel", None, false),
        ];
        let Ok(cfg) = parse_forum_channel(110, 220, 15, 0, tags) else {
            panic!("valid forum config");
        };
        let sel1 = toggle_tag_selection(&[], 101, &[101, 102]);
        let sel2 = toggle_tag_selection(&sel1, 102, &[101, 102]);
        assert_eq!(sel1, vec![101]);
        assert_eq!(sel2, vec![101, 102]);
        assert_eq!(toggle_tag_selection(&sel2, 101, &[101, 102]), vec![102]);
        let p1 = sample_post(1, "Post A", vec![101], true, 500);
        let p2 = sample_post(2, "Post B", vec![101, 102], false, 400);
        let p3 = sample_post(3, "Post C", vec![101], false, 300);
        let view = build_forum_grid_view(&cfg, &[p1, p2, p3], &[101], 960, Some(2), Some(1));
        assert_eq!(view.columns, 2);
        assert_eq!(view.rows, 2);
        assert_eq!(view.cells.len(), 3);
        assert_eq!(view.tag_chips[0].background_token, "#5865f2");
        assert_eq!(view.tag_chips[1].background_token, "#2b2d31");
        assert_eq!((view.cells[0].row, view.cells[0].col), (0, 0));
        assert_eq!((view.cells[1].row, view.cells[1].col), (0, 1));
        assert_eq!((view.cells[2].row, view.cells[2].col), (1, 0));
        assert_eq!(view.cells[0].post_card_state, "open thread");
        assert_eq!(view.cells[1].post_card_state, "border-accent");
        assert_eq!(view.cells[2].post_card_state, "card");
        assert_eq!(view.cells[0].card.card_height_px, 96);
    }
}
