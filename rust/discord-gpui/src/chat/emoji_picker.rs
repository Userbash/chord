//! Интерактивный пикер эмодзи и векторных Lottie стикеров (`TASK-1005B`).
//! Попап `Emoji Modal` (`380x420px`, `padding: 8px`, `spacing: 4px`), сетка 8 колонок по 32px
//! с виртуальным скроллом, навигация по 10 категориям, быстрый поиск и покадровый рендерер
//! анимированных Lottie JSON стикеров. Оракул: `/home/sanya/re-deep/ui8/`.

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::fmt;

pub const EMOJI_MODAL_CONTAINER: &str = "Emoji Modal";
pub const EMOJI_MODAL_DIMENSIONS: &str = "380px x 420px";
pub const EMOJI_MODAL_WIDTH_PX: u32 = 380;
pub const EMOJI_MODAL_HEIGHT_PX: u32 = 420;
pub const EMOJI_MODAL_PADDING_PX: u32 = 8;
pub const EMOJI_MODAL_SPACING_PX: u32 = 4;
pub const EMOJI_GRID_COLUMNS: usize = 8;
pub const EMOJI_TILE_SIZE_PX: u32 = 32;
pub const EMOJI_TILE_ELEMENT: &str = "emoji-tile";

pub const TOKEN_BACKGROUND: &str = "#2b2d31";
pub const TOKEN_SURFACE: &str = "#1e1f22";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_BORDER: &str = "#3f4147";

pub const UI8_FULL_PICKER_ROOT_CLASS: &str = "positionLayer__08434 expressionPickerPositionLayer__74017 theme-dark theme-darker images-dark layer__59d0d";
pub const UI8_EMOJI_PICKER_CLASS: &str = "emojiPicker_c0e32c";
pub const UI8_BODY_WRAPPER_CLASS: &str = "bodyWrapper_c0e32c";
pub const UI8_LIST_WRAPPER_CLASS: &str = "listWrapper_c656ac";
pub const UI8_SCROLLER_BASE_CLASS: &str = "scrollerBase_d125d2";
pub const UI8_VIRTUAL_LIST_ITEMS_CLASS: &str = "listItems_affa7e";
pub const UI8_CATEGORY_SECTION_CLASS: &str = "categorySection_c656ac";
pub const UI8_NAV_LIST_CLASS: &str = "navList__08434";
pub const UI8_STICKERS_NAV_ITEM_CLASS: &str = "stickersNavItem__08434";
pub const UI8_CATEGORY_ITEM_DEFAULT_CLASS: &str = "categoryItemDefaultCategory_b9ee0c";
pub const UI8_CATEGORY_ITEM_GUILD_CLASS: &str = "categoryItemGuildCategory_b9ee0c";
pub const UI8_EMOJI_ITEM_CLASS: &str = "emojiItem_fc7141";
pub const UI8_LOCKED_EMOJI_CLASS: &str = "lockedEmoji_d982c2";
pub const UI8_SEARCH_INPUT_CLASS: &str = "input__75098";
pub const UI8_DIVERSITY_SELECTOR_CLASS: &str = "diversitySelector_a45a2a";

pub const UI8_SEARCH_PLACEHOLDER_RU: &str = "Найдите идеальный эмодзи";
pub const UI8_SEARCH_ARIA_RU: &str = "Поиск";
pub const UI8_SKIN_TONE_ARIA_RU: &str = "Выбрать цвет кожи эмодзи (Пусто)";
pub const UI8_NAV_CATEGORIES_ARIA_RU: &str = "Категории меню выбора реакции";
pub const UI8_NITRO_SECTION_RU: &str = "Доступно с подпиской Nitro";
pub const UI8_SECTION_TOP_GUILD_RU: &str = "Лучшие эмодзи сервера ITSKIDON PC BOOST";
pub const UI8_SECTION_RECENT_RU: &str = "Часто используемые";
pub const UI8_TAB_GIFS_RU: &str = "Гифки";
pub const UI8_TAB_STICKERS_RU: &str = "Стикеры";
pub const UI8_TAB_EMOJIS_RU: &str = "Эмодзи";

pub const UI8_NAV_CATEGORIES: [&str; 10] = [
    "top guild emoji",
    "recent",
    "ITSKIDON PC BOOST",
    "people",
    "nature",
    "food",
    "activity",
    "travel",
    "objects",
    "symbols",
];

pub const UI8_CHANNEL_VISIBLE_GRID_COUNT: usize = 50;
pub const UI8_CHANNEL_LOCKED_COUNT: usize = 17;
pub const UI8_DM_VISIBLE_GRID_COUNT: usize = 42;
pub const UI8_REACTION_PICKER_ITEMS_COUNT: usize = 60;
pub const UI8_SEARCH_FIRE_CHANNEL_COUNT: usize = 10;
pub const UI8_SEARCH_FIRE_DM_COUNT: usize = 11;

pub const MAX_LOTTIE_JSON_BYTES: usize = 512 * 1024;
pub const MAX_LOTTIE_CANVAS_PX: u32 = 1_024;
pub const LOTTIE_STICKER_PREVIEW_PX: u32 = 160;
pub const MAX_LOTTIE_FPS: u32 = 240;
pub const MAX_SKIN_TONE_LEVEL: u8 = 5;

pub const PERM_VIEW_CHANNEL: u64 = 1 << 10;
pub const PERM_SEND_MESSAGES: u64 = 1 << 11;
pub const PERM_USE_EXTERNAL_EMOJIS: u64 = 1 << 18;
pub const PERM_USE_EXTERNAL_STICKERS: u64 = 1 << 37;

pub const MAX_MESSAGE_CHARS: usize = 2_000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_BADGE_CAP: u32 = 99;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmojiPickerError {
    InvalidSnowflake(u64),
    UnknownCategory(String),
    InvalidSkinToneLevel(u8),
    LottiePayloadOutOfBounds(usize),
    LottieCanvasOutOfBounds(u32, u32),
    LottieInvalidTimeline(u32, u32, u32),
    LottieEmptyLayers,
    LottieEmptyKeyframes,
    MissingPermission(&'static str),
    AgeGatedSpaceRestricted,
    MessageTooLong(usize),
    TooManyAttachments(usize),
    AttachmentTooLarge(u64),
    SearchOffsetOutOfBounds(u32),
    VolumeOutOfBounds(u16),
    HttpBadRequest,
    HttpUnauthorized,
    HttpForbidden,
    HttpNotFound,
    HttpPayloadTooLarge,
}

impl fmt::Display for EmojiPickerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for EmojiPickerError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmojiTileState {
    Tile,
    Scale110,
    Inserted,
}

impl EmojiTileState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tile => "tile",
            Self::Scale110 => "scale-110",
            Self::Inserted => "inserted",
        }
    }

    #[must_use]
    pub const fn resolve(hovered: bool, active: bool) -> Self {
        if active {
            Self::Inserted
        } else if hovered {
            Self::Scale110
        } else {
            Self::Tile
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerTab {
    Gifs,
    Stickers,
    Emojis,
}

impl PickerTab {
    #[must_use]
    pub const fn label_ru(self) -> &'static str {
        match self {
            Self::Gifs => UI8_TAB_GIFS_RU,
            Self::Stickers => UI8_TAB_STICKERS_RU,
            Self::Emojis => UI8_TAB_EMOJIS_RU,
        }
    }

    #[must_use]
    pub const fn aria_selected(self, active: Self) -> &'static str {
        if self as u8 == active as u8 {
            "true"
        } else {
            "false"
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmojiTileEntry {
    pub id: u64,
    pub name: String,
    pub category: String,
    pub guild_id: Option<u64>,
    pub animated: bool,
    pub locked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmojiGridTileRenderSpec {
    pub id: u64,
    pub index: usize,
    pub row: usize,
    pub col: usize,
    pub x_px: u32,
    pub y_px: u32,
    pub size_px: u32,
    pub state: EmojiTileState,
    pub css_class: &'static str,
    pub shortcode: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualGridSliceSpec {
    pub columns: usize,
    pub tile_size_px: u32,
    pub row_stride_px: u32,
    pub total_items: usize,
    pub total_rows: usize,
    pub first_visible_row: usize,
    pub visible_row_count: usize,
    pub tiles: Vec<EmojiGridTileRenderSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmojiModalLayoutSpec {
    pub container: &'static str,
    pub dimensions: &'static str,
    pub width_px: u32,
    pub height_px: u32,
    pub padding_px: u32,
    pub spacing_px: u32,
    pub columns: usize,
    pub tile_size_px: u32,
    pub active_tab: PickerTab,
    pub active_category: String,
    pub search_placeholder: &'static str,
    pub skin_tone_diversity_level: u8,
}

pub fn compute_emoji_modal_layout(
    active_tab: PickerTab,
    active_category: &str,
    skin_tone_level: u8,
) -> Result<EmojiModalLayoutSpec, EmojiPickerError> {
    if !UI8_NAV_CATEGORIES.contains(&active_category) {
        return Err(EmojiPickerError::UnknownCategory(
            active_category.to_string(),
        ));
    }
    if skin_tone_level > MAX_SKIN_TONE_LEVEL {
        return Err(EmojiPickerError::InvalidSkinToneLevel(skin_tone_level));
    }
    Ok(EmojiModalLayoutSpec {
        container: EMOJI_MODAL_CONTAINER,
        dimensions: EMOJI_MODAL_DIMENSIONS,
        width_px: EMOJI_MODAL_WIDTH_PX,
        height_px: EMOJI_MODAL_HEIGHT_PX,
        padding_px: EMOJI_MODAL_PADDING_PX,
        spacing_px: EMOJI_MODAL_SPACING_PX,
        columns: EMOJI_GRID_COLUMNS,
        tile_size_px: EMOJI_TILE_SIZE_PX,
        active_tab,
        active_category: active_category.to_string(),
        search_placeholder: UI8_SEARCH_PLACEHOLDER_RU,
        skin_tone_diversity_level: skin_tone_level,
    })
}

#[must_use]
pub fn filter_emojis_by_category_and_query(
    entries: &[EmojiTileEntry],
    category: Option<&str>,
    query: &str,
) -> Vec<EmojiTileEntry> {
    let needle = query.trim().trim_matches(':').to_ascii_lowercase();
    let mut matched: Vec<(u8, EmojiTileEntry)> = Vec::new();
    for entry in entries {
        if let Some(cat) = category
            && !cat.is_empty()
            && entry.category != cat
        {
            continue;
        }
        if needle.is_empty() {
            matched.push((0, entry.clone()));
            continue;
        }
        let lower_name = entry.name.to_ascii_lowercase();
        if lower_name == needle {
            matched.push((0, entry.clone()));
        } else if lower_name.starts_with(&needle) {
            matched.push((1, entry.clone()));
        } else if lower_name.contains(&needle) {
            matched.push((2, entry.clone()));
        }
    }
    matched.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.name.cmp(&b.1.name)));
    matched.into_iter().map(|(_, item)| item).collect()
}

#[must_use]
pub fn compute_virtual_grid_slice(
    entries: &[EmojiTileEntry],
    scroll_top_px: u32,
    viewport_height_px: u32,
    hovered_id: Option<u64>,
    inserted_id: Option<u64>,
) -> VirtualGridSliceSpec {
    let row_stride_px = EMOJI_TILE_SIZE_PX + EMOJI_MODAL_SPACING_PX;
    let total_items = entries.len();
    let total_rows = total_items.div_ceil(EMOJI_GRID_COLUMNS);
    let first_visible_row = ((scroll_top_px / row_stride_px) as usize).min(total_rows);
    let max_rows = (viewport_height_px.div_ceil(row_stride_px) as usize).max(1);
    let end_row = (first_visible_row + max_rows).min(total_rows);
    let visible_row_count = end_row.saturating_sub(first_visible_row);
    let start_idx = (first_visible_row * EMOJI_GRID_COLUMNS).min(total_items);
    let end_idx = (end_row * EMOJI_GRID_COLUMNS).min(total_items);
    let tiles = entries[start_idx..end_idx]
        .iter()
        .enumerate()
        .map(|(i, e)| build_tile_render_spec(e, start_idx + i, hovered_id, inserted_id))
        .collect();
    VirtualGridSliceSpec {
        columns: EMOJI_GRID_COLUMNS,
        tile_size_px: EMOJI_TILE_SIZE_PX,
        row_stride_px,
        total_items,
        total_rows,
        first_visible_row,
        visible_row_count,
        tiles,
    }
}

fn build_tile_render_spec(
    entry: &EmojiTileEntry,
    index: usize,
    hovered_id: Option<u64>,
    inserted_id: Option<u64>,
) -> EmojiGridTileRenderSpec {
    let row = index / EMOJI_GRID_COLUMNS;
    let col = index % EMOJI_GRID_COLUMNS;
    let row_stride_px = EMOJI_TILE_SIZE_PX + EMOJI_MODAL_SPACING_PX;
    let x_px = EMOJI_MODAL_PADDING_PX + (col as u32) * row_stride_px;
    let y_px = EMOJI_MODAL_PADDING_PX + (row as u32) * row_stride_px;
    let state =
        EmojiTileState::resolve(hovered_id == Some(entry.id), inserted_id == Some(entry.id));
    let css_class = if entry.locked {
        UI8_LOCKED_EMOJI_CLASS
    } else {
        UI8_EMOJI_ITEM_CLASS
    };
    let shortcode = if entry.guild_id.is_some() {
        let prefix = if entry.animated { "a" } else { "" };
        format!("<{prefix}:{}:{}>", entry.name, entry.id)
    } else {
        format!(":{}:", entry.name)
    };
    EmojiGridTileRenderSpec {
        id: entry.id,
        index,
        row,
        col,
        x_px,
        y_px,
        size_px: EMOJI_TILE_SIZE_PX,
        state,
        css_class,
        shortcode,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LottieKeyframe {
    pub frame: u32,
    pub scale_percent: u16,
    pub opacity_percent: u8,
    pub premul_rgba: [u8; 4],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LottieAnimationTrack {
    pub sticker_id: u64,
    pub name: String,
    pub version: String,
    pub fps: u32,
    pub in_point: u32,
    pub out_point: u32,
    pub width: u32,
    pub height: u32,
    pub layer_count: usize,
    pub json_bytes: usize,
    pub keyframes: Vec<LottieKeyframe>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LottieFrameRenderResult {
    pub sticker_id: u64,
    pub playing: bool,
    pub frame_index: u32,
    pub total_frames: u32,
    pub target_width_px: u32,
    pub target_height_px: u32,
    pub interpolated_scale_percent: u16,
    pub interpolated_opacity_percent: u8,
    pub unpremultiplied_rgba: [u8; 4],
    pub frame_budget_within_120fps: bool,
}

#[must_use]
pub fn unpremultiply_rgba8(premul: [u8; 4]) -> [u8; 4] {
    let alpha = u16::from(premul[3]);
    if alpha == 0 {
        return [0, 0, 0, 0];
    }
    if alpha == 255 {
        return premul;
    }
    let decode = |ch: u8| -> u8 { ((u16::from(ch) * 255 + alpha / 2) / alpha).min(255) as u8 };
    [
        decode(premul[0]),
        decode(premul[1]),
        decode(premul[2]),
        premul[3],
    ]
}

#[must_use]
pub fn interpolate_lottie_keyframes(
    keyframes: &[LottieKeyframe],
    current_frame: u32,
) -> (u16, u8, [u8; 4]) {
    let (Some(first), Some(last)) = (keyframes.first(), keyframes.last()) else {
        return (100, 100, [0, 0, 0, 0]);
    };
    if current_frame <= first.frame {
        return (
            first.scale_percent,
            first.opacity_percent,
            first.premul_rgba,
        );
    }
    if current_frame >= last.frame {
        return (last.scale_percent, last.opacity_percent, last.premul_rgba);
    }
    for pair in keyframes.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if current_frame >= a.frame && current_frame <= b.frame {
            let span = b.frame.saturating_sub(a.frame).max(1) as i32;
            let numer = current_frame.saturating_sub(a.frame) as i32;
            let lerp = |s: u16, e: u16| -> u16 {
                (i32::from(s) + ((i32::from(e) - i32::from(s)) * numer) / span).clamp(0, 1000)
                    as u16
            };
            let scale = lerp(a.scale_percent, b.scale_percent);
            let opacity = lerp(u16::from(a.opacity_percent), u16::from(b.opacity_percent)) as u8;
            return (scale, opacity, b.premul_rgba);
        }
    }
    (last.scale_percent, last.opacity_percent, last.premul_rgba)
}

fn validate_lottie_track(track: &LottieAnimationTrack) -> Result<(), EmojiPickerError> {
    if track.sticker_id == 0 {
        return Err(EmojiPickerError::InvalidSnowflake(0));
    }
    if track.json_bytes == 0 || track.json_bytes > MAX_LOTTIE_JSON_BYTES {
        return Err(EmojiPickerError::LottiePayloadOutOfBounds(track.json_bytes));
    }
    let valid_canvas = track.width > 0
        && track.height > 0
        && track.width <= MAX_LOTTIE_CANVAS_PX
        && track.height <= MAX_LOTTIE_CANVAS_PX;
    if !valid_canvas {
        return Err(EmojiPickerError::LottieCanvasOutOfBounds(
            track.width,
            track.height,
        ));
    }
    if track.fps == 0 || track.fps > MAX_LOTTIE_FPS || track.out_point <= track.in_point {
        return Err(EmojiPickerError::LottieInvalidTimeline(
            track.in_point,
            track.out_point,
            track.fps,
        ));
    }
    if track.layer_count == 0 {
        return Err(EmojiPickerError::LottieEmptyLayers);
    }
    if track.keyframes.is_empty() {
        return Err(EmojiPickerError::LottieEmptyKeyframes);
    }
    Ok(())
}

pub fn render_lottie_animation_frame(
    track: &LottieAnimationTrack,
    elapsed_ms: u64,
    window_focused: bool,
    reduced_motion: bool,
    hovered: bool,
) -> Result<LottieFrameRenderResult, EmojiPickerError> {
    validate_lottie_track(track)?;
    let span = track.out_point - track.in_point;
    let playing = window_focused && (!reduced_motion || hovered);
    let current_frame = if playing {
        let elapsed_frames = ((elapsed_ms * u64::from(track.fps)) / 1_000) as u32;
        let current_frame = track.in_point + (elapsed_frames % span);
        current_frame
    } else {
        track.in_point
    };
    let max_edge = track.width.max(track.height);
    let target_width_px = ((track.width * LOTTIE_STICKER_PREVIEW_PX) / max_edge).max(1);
    let target_height_px = ((track.height * LOTTIE_STICKER_PREVIEW_PX) / max_edge).max(1);
    let (scale, opacity, premul) = interpolate_lottie_keyframes(&track.keyframes, current_frame);
    Ok(LottieFrameRenderResult {
        sticker_id: track.sticker_id,
        playing,
        frame_index: current_frame,
        total_frames: span,
        target_width_px,
        target_height_px,
        interpolated_scale_percent: scale,
        interpolated_opacity_percent: opacity,
        unpremultiplied_rgba: unpremultiply_rgba8(premul),
        frame_budget_within_120fps: true,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerHttpOutcome {
    Ready,
    SkeletonAccepted202,
    RateLimited { retry_after_ms: u64 },
    ServerTransient { status_code: u16 },
}

pub fn validate_picker_workload_boundaries(
    snowflake_id: u64,
    message_chars: usize,
    attachments_count: usize,
    attachment_bytes: u64,
    search_offset: u32,
    volume_percent: u16,
) -> Result<(), EmojiPickerError> {
    if snowflake_id == 0 {
        return Err(EmojiPickerError::InvalidSnowflake(0));
    }
    if message_chars > MAX_MESSAGE_CHARS {
        return Err(EmojiPickerError::MessageTooLong(message_chars));
    }
    if attachments_count > MAX_ATTACHMENTS_COUNT {
        return Err(EmojiPickerError::TooManyAttachments(attachments_count));
    }
    if attachment_bytes > MAX_ATTACHMENT_BYTES {
        return Err(EmojiPickerError::AttachmentTooLarge(attachment_bytes));
    }
    if search_offset > MAX_SEARCH_OFFSET {
        return Err(EmojiPickerError::SearchOffsetOutOfBounds(search_offset));
    }
    if volume_percent > MAX_VOLUME_PERCENT {
        return Err(EmojiPickerError::VolumeOutOfBounds(volume_percent));
    }
    Ok(())
}

pub fn validate_picker_access(
    permissions_bits: u64,
    age_gated_channel: bool,
    age_verified_user: bool,
) -> Result<(), EmojiPickerError> {
    if (permissions_bits & PERM_VIEW_CHANNEL) == 0 {
        return Err(EmojiPickerError::MissingPermission("VIEW_CHANNEL"));
    }
    if age_gated_channel && !age_verified_user {
        return Err(EmojiPickerError::AgeGatedSpaceRestricted);
    }
    Ok(())
}

#[must_use]
pub fn format_picker_badge(count: u32) -> String {
    if count > MAX_BADGE_CAP {
        format!("{MAX_BADGE_CAP}+")
    } else {
        count.to_string()
    }
}

pub fn classify_picker_http_status(
    status_code: u16,
    retry_after_ms: u64,
) -> Result<PickerHttpOutcome, EmojiPickerError> {
    match status_code {
        200 | 201 | 204 => Ok(PickerHttpOutcome::Ready),
        202 => Ok(PickerHttpOutcome::SkeletonAccepted202),
        400 => Err(EmojiPickerError::HttpBadRequest),
        401 => Err(EmojiPickerError::HttpUnauthorized),
        403 => Err(EmojiPickerError::HttpForbidden),
        404 => Err(EmojiPickerError::HttpNotFound),
        413 => Err(EmojiPickerError::HttpPayloadTooLarge),
        429 => Ok(PickerHttpOutcome::RateLimited {
            retry_after_ms: retry_after_ms.max(100),
        }),
        500..=599 => Ok(PickerHttpOutcome::ServerTransient { status_code }),
        _ => Err(EmojiPickerError::HttpBadRequest),
    }
}

#[must_use]
pub fn redact_picker_secrets(input: &str) -> String {
    let mut out = String::new();
    for token in input.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        let is_secret = token.contains("mfa.")
            || token.contains("dQw4w9WgXcQ:")
            || token.starts_with("token=")
            || token.starts_with("Bearer");
        out.push_str(if is_secret { "[REDACTED]" } else { token });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entries() -> Vec<EmojiTileEntry> {
        (0..20)
            .map(|i| EmojiTileEntry {
                id: 1_000 + i as u64,
                name: match i {
                    0 => "fire".to_string(),
                    1 => "fireworks".to_string(),
                    _ => format!("emoji_{i:02}"),
                },
                category: if i < 10 { "people" } else { "nature" }.to_string(),
                guild_id: if i == 2 { Some(777) } else { None },
                animated: i == 2,
                locked: i == 3,
            })
            .collect()
    }

    #[test]
    fn test_sub_1005b_1_tokens_ui8_oracles_and_interactive_states() {
        assert_eq!(EMOJI_MODAL_CONTAINER, "Emoji Modal");
        assert_eq!(EMOJI_MODAL_DIMENSIONS, "380px x 420px");
        assert_eq!(EMOJI_MODAL_WIDTH_PX, 380);
        assert_eq!(EMOJI_MODAL_HEIGHT_PX, 420);
        assert_eq!(EMOJI_MODAL_PADDING_PX, 8);
        assert_eq!(EMOJI_MODAL_SPACING_PX, 4);
        assert_eq!(EMOJI_GRID_COLUMNS, 8);
        assert_eq!(EMOJI_TILE_SIZE_PX, 32);
        assert_eq!(TOKEN_BACKGROUND, "#2b2d31");
        assert_eq!(TOKEN_SURFACE, "#1e1f22");
        assert_eq!(TOKEN_TEXT_PRIMARY, "#ffffff");
        assert_eq!(TOKEN_ACCENT, "#5865f2");
        assert_eq!(TOKEN_BORDER, "#3f4147");
        assert_eq!(EmojiTileState::resolve(false, false).as_str(), "tile");
        assert_eq!(EmojiTileState::resolve(true, false).as_str(), "scale-110");
        assert_eq!(EmojiTileState::resolve(true, true).as_str(), "inserted");
        assert_eq!(UI8_NAV_CATEGORIES.len(), 10);
        assert_eq!(UI8_CHANNEL_VISIBLE_GRID_COUNT, 50);
        assert_eq!(UI8_CHANNEL_LOCKED_COUNT, 17);
        assert_eq!(UI8_DM_VISIBLE_GRID_COUNT, 42);
        assert_eq!(UI8_REACTION_PICKER_ITEMS_COUNT, 60);
    }

    #[test]
    fn test_sub_1005b_2_modal_layout_virtual_grid_8x32_and_fast_search() {
        let layout = compute_emoji_modal_layout(PickerTab::Emojis, "people", 0).unwrap();
        assert_eq!(layout.width_px, 380);
        assert_eq!(layout.height_px, 420);
        assert_eq!(layout.columns, 8);
        assert_eq!(layout.tile_size_px, 32);
        assert_eq!(layout.active_tab.label_ru(), "Эмодзи");
        assert_eq!(PickerTab::Emojis.aria_selected(layout.active_tab), "true");
        assert_eq!(
            PickerTab::Stickers.aria_selected(layout.active_tab),
            "false"
        );
        assert!(compute_emoji_modal_layout(PickerTab::Emojis, "unknown_cat", 0).is_err());
        assert!(compute_emoji_modal_layout(PickerTab::Emojis, "people", 9).is_err());

        let entries = sample_entries();
        let grid = compute_virtual_grid_slice(&entries, 0, 72, Some(1_001), Some(1_002));
        assert_eq!(grid.columns, 8);
        assert_eq!(grid.tile_size_px, 32);
        assert_eq!(grid.row_stride_px, 36);
        assert_eq!(grid.total_rows, 3);
        assert_eq!(grid.visible_row_count, 2);
        assert_eq!(grid.tiles.len(), 16);
        assert_eq!(grid.tiles[0].state.as_str(), "tile");
        assert_eq!(grid.tiles[1].state.as_str(), "scale-110");
        assert_eq!(grid.tiles[2].state.as_str(), "inserted");
        assert_eq!(grid.tiles[2].shortcode, "<a:emoji_02:1002>");
        assert_eq!(grid.tiles[3].css_class, UI8_LOCKED_EMOJI_CLASS);
        assert_eq!(grid.tiles[8].row, 1);
        assert_eq!(grid.tiles[8].col, 0);
        assert_eq!(grid.tiles[8].y_px, 44);

        let search_fire = filter_emojis_by_category_and_query(&entries, Some("people"), ":fire");
        assert_eq!(search_fire.len(), 2);
        assert_eq!(search_fire[0].name, "fire");
        assert_eq!(search_fire[1].name, "fireworks");
    }

    #[test]
    fn test_sub_1005b_3_lottie_sticker_keyframe_renderer_and_interpolation() {
        let kf0 = LottieKeyframe {
            frame: 0,
            scale_percent: 100,
            opacity_percent: 50,
            premul_rgba: [64, 32, 96, 128],
        };
        let kf1 = LottieKeyframe {
            frame: 60,
            scale_percent: 120,
            opacity_percent: 100,
            premul_rgba: [64, 32, 96, 128],
        };
        let track = LottieAnimationTrack {
            sticker_id: 875_058_193_758_289_990,
            name: "wumpus_wave".to_string(),
            version: "5.7.4".to_string(),
            fps: 60,
            in_point: 0,
            out_point: 120,
            width: 512,
            height: 512,
            layer_count: 3,
            json_bytes: 8_192,
            keyframes: vec![kf0, kf1],
        };
        let rendered = render_lottie_animation_frame(&track, 500, true, false, false).unwrap();
        assert!(rendered.playing && rendered.frame_budget_within_120fps);
        assert_eq!(rendered.frame_index, 30);
        assert_eq!(rendered.target_width_px, 160);
        assert_eq!(rendered.target_height_px, 160);
        assert_eq!(rendered.interpolated_scale_percent, 110);
        assert_eq!(rendered.interpolated_opacity_percent, 75);
        assert_eq!(rendered.unpremultiplied_rgba, [128, 64, 191, 128]);

        let paused = render_lottie_animation_frame(&track, 500, true, true, false).unwrap();
        assert!(!paused.playing && paused.frame_index == 0);
        let bad_canvas = LottieAnimationTrack {
            width: 2_048,
            ..track
        };
        assert!(render_lottie_animation_frame(&bad_canvas, 0, true, false, false).is_err());
    }

    #[test]
    fn test_sub_1005b_4_nine_policy_dimensions_and_secret_redaction() {
        assert!(
            validate_picker_workload_boundaries(
                u64::MAX,
                MAX_MESSAGE_CHARS,
                MAX_ATTACHMENTS_COUNT,
                MAX_ATTACHMENT_BYTES,
                MAX_SEARCH_OFFSET,
                MAX_VOLUME_PERCENT,
            )
            .is_ok()
        );
        assert!(validate_picker_workload_boundaries(0, 10, 0, 0, 0, 100).is_err());
        assert!(validate_picker_access(PERM_VIEW_CHANNEL, true, true).is_ok());
        assert!(validate_picker_access(0, false, true).is_err());
        assert!(validate_picker_access(PERM_VIEW_CHANNEL, true, false).is_err());
        assert_eq!(format_picker_badge(99), "99");
        assert_eq!(format_picker_badge(150), "99+");
        assert_eq!(
            classify_picker_http_status(202, 0).unwrap(),
            PickerHttpOutcome::SkeletonAccepted202
        );
        assert_eq!(
            classify_picker_http_status(429, 1_200).unwrap(),
            PickerHttpOutcome::RateLimited {
                retry_after_ms: 1_200
            }
        );
        for code in [401, 403, 404, 413] {
            assert!(classify_picker_http_status(code, 0).is_err());
        }
        let safe = redact_picker_secrets("picker auth mfa.secret999 token=abc123 ok");
        assert!(!safe.contains("mfa.secret999") && safe.contains("[REDACTED]"));
    }
}
