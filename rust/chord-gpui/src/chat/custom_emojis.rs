//! Серверные эмодзи гильдий `<:name:id>`, анимированные GIF, векторные стикеров Lottie
//! и дисковый кэш `~/.cache/discord-gpui/emojis/` (`TASK-1005`).
//!
//! Оракулы:
//! - `waves/kanban-board.json` (`TASK-1005`): `Emoji & Sticker Grid` (`320px x 400px`, `padding: 8px`,
//!   `spacing: 4px`), дизайн-токены (`#2b2d31`, `#1e1f22`, `#ffffff`, `#5865f2`, `#3f4147`),
//!   интерактивные состояния `emoji-item` (`normal -> scale-110 -> inserted`).
//! - `docs/re/frontend-d03.json`: экран `emoji` (`tabCount=3`, `navCategories=10`,
//!   `visibleGridChannel=50`, `lockedChannel=17`, `visibleGridDM=42`, `reactionPickerItems=60`).
//! - `docs/re/i18n-deep.json`: синтаксис `<a:name:123> / <:name:123>` и `EMOJI_NAME_RE`.
//! - `docs/re/gateway-schema.json`: `emojisPerGuild` (`363` эмодзи, `CoP`, `ClearSky`).

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

pub const EMOJI_GRID_CONTAINER: &str = "Emoji & Sticker Grid";
pub const EMOJI_GRID_DIMENSIONS: &str = "320px x 400px";
pub const EMOJI_GRID_PADDING_PX: u32 = 8;
pub const EMOJI_GRID_SPACING_PX: u32 = 4;

pub const TOKEN_BACKGROUND: &str = "#2b2d31";
pub const TOKEN_SURFACE: &str = "#1e1f22";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_BORDER: &str = "#3f4147";

pub const DEFAULT_EMOJI_CACHE_DIR: &str = "~/.cache/discord-gpui/emojis/";
pub const FULL_PICKER_ROOT_CLASS: &str =
    "positionLayer__08434 expressionPickerPositionLayer__74017 theme-dark images-dark layer__59d0d";
pub const EMOJI_ITEM_CLASS: &str = "emojiItem_fc7141";
pub const LOCKED_EMOJI_CLASS: &str = "lockedEmoji_d982c2";

pub const MIN_EMOJI_NAME_CHARS: usize = 2;
pub const MAX_EMOJI_NAME_CHARS: usize = 32;
pub const MAX_EMOJI_UPLOAD_BYTES: u64 = 256 * 1024;
pub const MAX_JUMBO_EMOJI_COUNT: usize = 30;
pub const INLINE_EMOJI_SIZE_PX: u32 = 22;
pub const JUMBO_EMOJI_SIZE_PX: u32 = 48;

pub const STICKER_FORMAT_PNG: u8 = 1;
pub const STICKER_FORMAT_APNG: u8 = 2;
pub const STICKER_FORMAT_LOTTIE: u8 = 3;
pub const STICKER_FORMAT_GIF: u8 = 4;

pub const MAX_LOTTIE_JSON_BYTES: usize = 512 * 1024;
pub const MAX_LOTTIE_CANVAS_PX: u32 = 1_024;
pub const LOTTIE_RENDER_EDGE_PX: u32 = 160;
pub const MIN_GIF_FRAME_DELAY_MS: u32 = 20;
pub const DEFAULT_GIF_FALLBACK_DELAY_MS: u32 = 100;

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
pub enum EmojiError {
    InvalidSnowflake(u64),
    InvalidEmojiName(String),
    InvalidStickerFormat(u8),
    EmojiUploadTooLarge(u64),
    LottiePayloadTooLarge(usize),
    LottieInvalidCanvas(u32, u32),
    LottieInvalidFrameRange(u32, u32, u32),
    LottieEmptyLayers,
    GifEmptyFrames,
    CachePathTraversal(String),
    CacheEntryExceedsBudget(usize, usize),
    CacheIoError(String),
    CacheChecksumMismatch(String),
    MessageTooLong(usize),
    TooManyAttachments(usize),
    AttachmentTooLarge(u64),
    SearchOffsetOutOfBounds(u32),
    VolumeOutOfBounds(u16),
    AccessDenied(&'static str),
    AgeGatedSpaceRestricted,
    HttpFailure(u16, u64),
}

impl fmt::Display for EmojiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for EmojiError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmojiItemTrigger {
    Default,
    Hover,
    Active,
}

impl EmojiItemTrigger {
    #[must_use]
    pub const fn state_label(self) -> &'static str {
        match self {
            Self::Default => "normal",
            Self::Hover => "scale-110",
            Self::Active => "inserted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedEmojiToken {
    pub raw: String,
    pub name: String,
    pub id: u64,
    pub animated: bool,
    pub start_byte: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageEmojiPresentation {
    pub tokens: Vec<ParsedEmojiToken>,
    pub jumboable: bool,
    pub render_size_px: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmojiAccessVerdict {
    pub allowed: bool,
    pub css_class: &'static str,
    pub reason: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GifPlaybackStep {
    pub playing: bool,
    pub frame_index: usize,
    pub total_duration_ms: u32,
    pub normalized_delays_ms: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LottieStickerSpec {
    pub version: String,
    pub name: String,
    pub fps: u32,
    pub in_point: u32,
    pub out_point: u32,
    pub width: u32,
    pub height: u32,
    pub layer_count: usize,
    pub json_bytes: usize,
    pub sample_premul_rgba: [u8; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct LottieRenderPreview {
    pub target_width: u32,
    pub target_height: u32,
    pub scale_factor: f64,
    pub current_frame: u32,
    pub unpremultiplied_rgba: [u8; 4],
}

pub fn validate_emoji_name(name: &str) -> Result<(), EmojiError> {
    let trimmed = name.trim();
    let count = trimmed.chars().count();
    let valid_chars = trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !(MIN_EMOJI_NAME_CHARS..=MAX_EMOJI_NAME_CHARS).contains(&count) || !valid_chars {
        return Err(EmojiError::InvalidEmojiName(name.to_string()));
    }
    Ok(())
}

fn parse_single_tag(slice: &str, start_byte: usize) -> Option<(ParsedEmojiToken, usize)> {
    let prefix = if slice.starts_with("<a:") {
        "<a:"
    } else if slice.starts_with("<:") {
        "<:"
    } else {
        return None;
    };
    let animated = prefix == "<a:";
    let body = &slice[prefix.len()..];
    let close_idx = body.find('>')?;
    let (name, id_str) = body[..close_idx].split_once(':')?;
    if validate_emoji_name(name).is_err() || id_str.starts_with('0') || id_str.len() > 20 {
        return None;
    }
    let id = id_str.parse::<u64>().ok().filter(|&v| v > 0)?;
    let consumed = prefix.len() + close_idx + 1;
    Some((
        ParsedEmojiToken {
            raw: slice[..consumed].to_string(),
            name: name.to_string(),
            id,
            animated,
            start_byte,
        },
        consumed,
    ))
}

#[must_use]
pub fn parse_custom_emoji_tokens(text: &str) -> Vec<ParsedEmojiToken> {
    let mut tokens = Vec::new();
    let bytes = text.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        if bytes[idx] == b'\\' && idx + 1 < bytes.len() && bytes[idx + 1] == b'<' {
            idx += 2;
            continue;
        }
        if bytes[idx] == b'<'
            && let Some(tail) = text.get(idx..)
            && let Some((token, consumed)) = parse_single_tag(tail, idx)
        {
            tokens.push(token);
            idx += consumed;
            continue;
        }
        idx += 1;
    }
    tokens
}

#[must_use]
pub fn compute_message_emoji_presentation(text: &str) -> MessageEmojiPresentation {
    let tokens = parse_custom_emoji_tokens(text);
    let mut stripped = text.to_string();
    for tok in &tokens {
        stripped = stripped.replacen(&tok.raw, "", 1);
    }
    let only_emojis = !tokens.is_empty() && stripped.chars().all(char::is_whitespace);
    let jumboable = only_emojis && tokens.len() <= MAX_JUMBO_EMOJI_COUNT;
    let render_size_px = if jumboable {
        JUMBO_EMOJI_SIZE_PX
    } else {
        INLINE_EMOJI_SIZE_PX
    };
    MessageEmojiPresentation {
        tokens,
        jumboable,
        render_size_px,
    }
}

pub fn build_emoji_cdn_url(id: u64, animated: bool, size: u32) -> Result<String, EmojiError> {
    if id == 0 {
        return Err(EmojiError::InvalidSnowflake(0));
    }
    let ext = if animated { "gif" } else { "webp" };
    Ok(format!(
        "https://cdn.discordapp.com/emojis/{id}.{ext}?size={size}"
    ))
}

pub fn build_sticker_cdn_url(id: u64, format_type: u8) -> Result<String, EmojiError> {
    if id == 0 {
        return Err(EmojiError::InvalidSnowflake(0));
    }
    match format_type {
        STICKER_FORMAT_PNG | STICKER_FORMAT_APNG => {
            Ok(format!("https://cdn.discordapp.com/stickers/{id}.png"))
        }
        STICKER_FORMAT_LOTTIE => Ok(format!("https://cdn.discordapp.com/stickers/{id}.json")),
        STICKER_FORMAT_GIF => Ok(format!("https://media.discordapp.net/stickers/{id}.gif")),
        other => Err(EmojiError::InvalidStickerFormat(other)),
    }
}

#[must_use]
pub fn evaluate_custom_emoji_access(
    emoji_guild_id: u64,
    current_guild_id: Option<u64>,
    animated: bool,
    available: bool,
    has_nitro: bool,
    channel_permissions: u64,
) -> EmojiAccessVerdict {
    if !available {
        return EmojiAccessVerdict {
            allowed: false,
            css_class: LOCKED_EMOJI_CLASS,
            reason: "emoji_unavailable",
        };
    }
    let is_external = current_guild_id != Some(emoji_guild_id);
    if is_external && (channel_permissions & PERM_USE_EXTERNAL_EMOJIS) == 0 {
        return EmojiAccessVerdict {
            allowed: false,
            css_class: LOCKED_EMOJI_CLASS,
            reason: "missing_use_external_emojis_permission",
        };
    }
    if (animated || is_external) && !has_nitro {
        return EmojiAccessVerdict {
            allowed: false,
            css_class: LOCKED_EMOJI_CLASS,
            reason: "nitro_required",
        };
    }
    EmojiAccessVerdict {
        allowed: true,
        css_class: EMOJI_ITEM_CLASS,
        reason: "allowed",
    }
}

pub fn step_animated_gif_playback(
    frame_delays_ms: &[u32],
    elapsed_ms: u64,
    window_focused: bool,
    reduced_motion: bool,
    hovering: bool,
) -> Result<GifPlaybackStep, EmojiError> {
    if frame_delays_ms.is_empty() {
        return Err(EmojiError::GifEmptyFrames);
    }
    let normalized_delays_ms: Vec<u32> = frame_delays_ms
        .iter()
        .map(|&d| {
            if d < MIN_GIF_FRAME_DELAY_MS {
                DEFAULT_GIF_FALLBACK_DELAY_MS
            } else {
                d
            }
        })
        .collect();
    let total_duration_ms: u32 = normalized_delays_ms.iter().sum();
    let playing = window_focused && (!reduced_motion || hovering) && normalized_delays_ms.len() > 1;
    if !playing || total_duration_ms == 0 {
        return Ok(GifPlaybackStep {
            playing: false,
            frame_index: 0,
            total_duration_ms,
            normalized_delays_ms,
        });
    }
    let cursor = (elapsed_ms % u64::from(total_duration_ms)) as u32;
    let mut acc = 0u32;
    let mut frame_index = 0usize;
    for (idx, &delay) in normalized_delays_ms.iter().enumerate() {
        acc = acc.saturating_add(delay);
        if cursor < acc {
            frame_index = idx;
            break;
        }
    }
    Ok(GifPlaybackStep {
        playing: true,
        frame_index,
        total_duration_ms,
        normalized_delays_ms,
    })
}

#[must_use]
pub fn unpremultiply_rgba_pixel(premul: [u8; 4]) -> [u8; 4] {
    let alpha = u16::from(premul[3]);
    if alpha == 0 {
        return [0, 0, 0, 0];
    }
    if alpha == 255 {
        return premul;
    }
    let unpremul = |ch: u8| -> u8 { ((u16::from(ch) * 255 + alpha / 2) / alpha).min(255) as u8 };
    [
        unpremul(premul[0]),
        unpremul(premul[1]),
        unpremul(premul[2]),
        premul[3],
    ]
}

#[must_use]
const fn is_valid_lottie_canvas(width: u32, height: u32) -> bool {
    width > 0 && height > 0 && width <= MAX_LOTTIE_CANVAS_PX && height <= MAX_LOTTIE_CANVAS_PX
}

pub fn validate_and_render_lottie_sticker(
    spec: &LottieStickerSpec,
    elapsed_ms: u64,
) -> Result<LottieRenderPreview, EmojiError> {
    if spec.json_bytes == 0 || spec.json_bytes > MAX_LOTTIE_JSON_BYTES {
        return Err(EmojiError::LottiePayloadTooLarge(spec.json_bytes));
    }
    if !is_valid_lottie_canvas(spec.width, spec.height) {
        return Err(EmojiError::LottieInvalidCanvas(spec.width, spec.height));
    }
    if spec.fps == 0 || spec.fps > 240 || spec.out_point <= spec.in_point {
        return Err(EmojiError::LottieInvalidFrameRange(
            spec.in_point,
            spec.out_point,
            spec.fps,
        ));
    }
    if spec.layer_count == 0 {
        return Err(EmojiError::LottieEmptyLayers);
    }
    let max_edge = spec.width.max(spec.height);
    let scale_factor = f64::from(LOTTIE_RENDER_EDGE_PX) / f64::from(max_edge);
    let target_width = ((f64::from(spec.width) * scale_factor).round() as u32).max(1);
    let target_height = ((f64::from(spec.height) * scale_factor).round() as u32).max(1);
    let span = spec.out_point - spec.in_point;
    let current_frame =
        spec.in_point + (((elapsed_ms * u64::from(spec.fps)) / 1_000) as u32 % span);
    Ok(LottieRenderPreview {
        target_width,
        target_height,
        scale_factor,
        current_frame,
        unpremultiplied_rgba: unpremultiply_rgba_pixel(spec.sample_premul_rgba),
    })
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

fn validate_cache_key(key: &str) -> Result<(), EmojiError> {
    let valid = !key.is_empty()
        && !key.contains("..")
        && !key.contains('/')
        && !key.contains('\\')
        && !key.contains('\0')
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
    if !valid {
        return Err(EmojiError::CachePathTraversal(key.to_string()));
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct DiskCacheRecord {
    file_path: PathBuf,
    size_bytes: usize,
    checksum: u64,
}

#[derive(Debug)]
pub struct EmojiDiskCache {
    root_dir: PathBuf,
    max_entries: usize,
    max_bytes: usize,
    total_bytes: usize,
    lru_order: VecDeque<String>,
    records: BTreeMap<String, DiskCacheRecord>,
    hits: u64,
    misses: u64,
}

impl EmojiDiskCache {
    pub fn new(
        root_dir: PathBuf,
        max_entries: usize,
        max_bytes: usize,
    ) -> Result<Self, EmojiError> {
        fs::create_dir_all(&root_dir).map_err(|e| EmojiError::CacheIoError(e.to_string()))?;
        Ok(Self {
            root_dir,
            max_entries: max_entries.max(1),
            max_bytes: max_bytes.max(1),
            total_bytes: 0,
            lru_order: VecDeque::new(),
            records: BTreeMap::new(),
            hits: 0,
            misses: 0,
        })
    }

    #[must_use]
    pub fn resolve_default_dir(home_env: Option<&str>) -> String {
        match home_env.map(str::trim).filter(|s| !s.is_empty()) {
            Some(home) => format!("{}/.cache/discord-gpui/emojis/", home.trim_end_matches('/')),
            None => DEFAULT_EMOJI_CACHE_DIR.to_string(),
        }
    }

    fn touch_lru(&mut self, key: &str) {
        self.lru_order.retain(|k| k != key);
        self.lru_order.push_back(key.to_string());
    }

    fn remove_key(&mut self, key: &str) {
        self.lru_order.retain(|k| k != key);
        if let Some(rec) = self.records.remove(key) {
            self.total_bytes = self.total_bytes.saturating_sub(rec.size_bytes);
            let _ = fs::remove_file(&rec.file_path);
        }
    }

    fn evict_until_within_budget(&mut self, incoming_bytes: usize) {
        while (!self.records.is_empty())
            && (self.records.len() >= self.max_entries
                || self.total_bytes.saturating_add(incoming_bytes) > self.max_bytes)
        {
            let Some(oldest) = self.lru_order.pop_front() else {
                break;
            };
            self.remove_key(&oldest);
        }
    }

    pub fn put(&mut self, key: &str, bytes: &[u8]) -> Result<PathBuf, EmojiError> {
        validate_cache_key(key)?;
        if bytes.is_empty() || bytes.len() > self.max_bytes {
            return Err(EmojiError::CacheEntryExceedsBudget(
                bytes.len(),
                self.max_bytes,
            ));
        }
        self.remove_key(key);
        self.evict_until_within_budget(bytes.len());
        let file_path = self.root_dir.join(key);
        fs::write(&file_path, bytes).map_err(|e| EmojiError::CacheIoError(e.to_string()))?;
        self.total_bytes = self.total_bytes.saturating_add(bytes.len());
        self.records.insert(
            key.to_string(),
            DiskCacheRecord {
                file_path: file_path.clone(),
                size_bytes: bytes.len(),
                checksum: fnv1a64(bytes),
            },
        );
        self.touch_lru(key);
        Ok(file_path)
    }

    pub fn get(&mut self, key: &str) -> Result<Option<Vec<u8>>, EmojiError> {
        validate_cache_key(key)?;
        let Some(rec) = self.records.get(key).cloned() else {
            self.misses += 1;
            return Ok(None);
        };
        let Ok(bytes) = fs::read(&rec.file_path) else {
            self.remove_key(key);
            self.misses += 1;
            return Ok(None);
        };
        if fnv1a64(&bytes) != rec.checksum {
            self.remove_key(key);
            self.misses += 1;
            return Err(EmojiError::CacheChecksumMismatch(key.to_string()));
        }
        self.touch_lru(key);
        self.hits += 1;
        Ok(Some(bytes))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    #[must_use]
    pub fn root_dir(&self) -> &Path {
        self.root_dir.as_path()
    }
}

pub fn validate_policy_boundaries(
    snowflake: u64,
    message_chars: usize,
    attachments_count: usize,
    attachment_bytes: u64,
    search_offset: u32,
    volume_percent: u16,
) -> Result<(), EmojiError> {
    if snowflake == 0 {
        return Err(EmojiError::InvalidSnowflake(0));
    }
    if message_chars > MAX_MESSAGE_CHARS {
        return Err(EmojiError::MessageTooLong(message_chars));
    }
    if attachments_count > MAX_ATTACHMENTS_COUNT {
        return Err(EmojiError::TooManyAttachments(attachments_count));
    }
    if attachment_bytes > MAX_ATTACHMENT_BYTES {
        return Err(EmojiError::AttachmentTooLarge(attachment_bytes));
    }
    if search_offset > MAX_SEARCH_OFFSET {
        return Err(EmojiError::SearchOffsetOutOfBounds(search_offset));
    }
    if volume_percent > MAX_VOLUME_PERCENT {
        return Err(EmojiError::VolumeOutOfBounds(volume_percent));
    }
    Ok(())
}

#[must_use]
pub fn format_unread_badge(count: u32) -> String {
    if count > MAX_BADGE_CAP {
        "99+".to_string()
    } else {
        count.to_string()
    }
}

pub fn classify_emoji_http_status(
    status: u16,
    retry_after_ms: u64,
) -> Result<&'static str, EmojiError> {
    match status {
        200 | 201 | 204 => Ok("ready"),
        202 => Ok("skeleton"),
        401 | 403 | 404 | 413 | 429 | 500..=599 => {
            Err(EmojiError::HttpFailure(status, retry_after_ms))
        }
        _ => Err(EmojiError::HttpFailure(status, 0)),
    }
}

#[must_use]
pub fn redact_emoji_telemetry(log_line: &str) -> String {
    let mut output = String::new();
    for token in log_line.split_whitespace() {
        if !output.is_empty() {
            output.push(' ');
        }
        let redacted = token.contains("mfa.")
            || token.contains("Bearer")
            || token.starts_with("token=")
            || token.starts_with("auth=");
        output.push_str(if redacted { "[REDACTED]" } else { token });
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sub_1005_1_tokens_layout_and_interactive_states() {
        assert_eq!(EMOJI_GRID_CONTAINER, "Emoji & Sticker Grid");
        assert_eq!(EMOJI_GRID_DIMENSIONS, "320px x 400px");
        assert_eq!(EMOJI_GRID_PADDING_PX, 8);
        assert_eq!(EMOJI_GRID_SPACING_PX, 4);
        assert_eq!(TOKEN_BACKGROUND, "#2b2d31");
        assert_eq!(TOKEN_SURFACE, "#1e1f22");
        assert_eq!(TOKEN_TEXT_PRIMARY, "#ffffff");
        assert_eq!(TOKEN_ACCENT, "#5865f2");
        assert_eq!(TOKEN_BORDER, "#3f4147");
        assert_eq!(DEFAULT_EMOJI_CACHE_DIR, "~/.cache/discord-gpui/emojis/");
        assert_eq!(EmojiItemTrigger::Default.state_label(), "normal");
        assert_eq!(EmojiItemTrigger::Hover.state_label(), "scale-110");
        assert_eq!(EmojiItemTrigger::Active.state_label(), "inserted");
    }

    #[test]
    fn test_sub_1005_2_custom_emoji_parser_jumbo_cdn_and_permissions() {
        let tokens = parse_custom_emoji_tokens(
            "Hello <:CoP:3527262836281581568> and <a:wave:1056141433670742056>!",
        );
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].name, "CoP");
        assert_eq!(tokens[0].id, 3_527_262_836_281_581_568);
        assert!(!tokens[0].animated);
        assert_eq!(tokens[1].name, "wave");
        assert_eq!(tokens[1].id, 1_056_141_433_670_742_056);
        assert!(tokens[1].animated);

        let jumbo = compute_message_emoji_presentation(
            "<:CoP:3527262836281581568> <a:wave:1056141433670742056>",
        );
        assert!(jumbo.jumboable);
        assert_eq!(jumbo.render_size_px, 48);

        let inline = compute_message_emoji_presentation("Text <:CoP:3527262836281581568>");
        assert!(!inline.jumboable);
        assert_eq!(inline.render_size_px, 22);

        let url_anim = build_emoji_cdn_url(1056141433670742056, true, 64).unwrap();
        assert_eq!(
            url_anim,
            "https://cdn.discordapp.com/emojis/1056141433670742056.gif?size=64"
        );
        let url_lottie = build_sticker_cdn_url(875058193758289990, STICKER_FORMAT_LOTTIE).unwrap();
        assert_eq!(
            url_lottie,
            "https://cdn.discordapp.com/stickers/875058193758289990.json"
        );

        let verdict_locked = evaluate_custom_emoji_access(
            101,
            Some(101),
            true,
            true,
            false,
            PERM_USE_EXTERNAL_EMOJIS,
        );
        assert!(!verdict_locked.allowed);
        assert_eq!(verdict_locked.css_class, LOCKED_EMOJI_CLASS);
    }

    #[test]
    fn test_sub_1005_3_animated_gif_and_lottie_sticker_rendering() {
        let gif = step_animated_gif_playback(&[40, 40, 10, 80], 95, true, false, false).unwrap();
        assert!(gif.playing);
        assert_eq!(gif.total_duration_ms, 260);
        assert_eq!(gif.frame_index, 2);

        let valid_lottie = LottieStickerSpec {
            version: "5.7.4".to_string(),
            name: "wumpus_wave".to_string(),
            fps: 60,
            in_point: 0,
            out_point: 120,
            width: 512,
            height: 512,
            layer_count: 2,
            json_bytes: 4096,
            sample_premul_rgba: [64, 32, 96, 128],
        };
        let preview = validate_and_render_lottie_sticker(&valid_lottie, 500).unwrap();
        assert_eq!(preview.target_width, 160);
        assert_eq!(preview.target_height, 160);
        assert_eq!(preview.current_frame, 30);
        assert_eq!(preview.unpremultiplied_rgba, [128, 64, 191, 128]);

        let oversized_lottie = LottieStickerSpec {
            width: 2048,
            height: 2048,
            ..valid_lottie
        };
        assert!(matches!(
            validate_and_render_lottie_sticker(&oversized_lottie, 0),
            Err(EmojiError::LottieInvalidCanvas(..))
        ));
    }

    #[test]
    fn test_sub_1005_4_disk_cache_lru_checksum_and_policy_dimensions() {
        assert_eq!(
            EmojiDiskCache::resolve_default_dir(Some("/home/user")),
            "/home/user/.cache/discord-gpui/emojis/"
        );
        let temp_dir =
            std::env::temp_dir().join(format!("gpui_emoji_cache_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        let mut cache = EmojiDiskCache::new(temp_dir.clone(), 2, 64).unwrap();

        let p1 = cache.put("e1.webp", b"1111111111").unwrap();
        assert!(p1.exists());
        let _ = cache.put("e2.gif", b"2222222222").unwrap();
        let _ = cache.get("e1.webp").unwrap();
        let _ = cache.put("e3.png", b"3333333333").unwrap();

        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get("e2.gif").unwrap(), None);
        assert_eq!(cache.get("e1.webp").unwrap(), Some(b"1111111111".to_vec()));
        assert!(cache.put("../traversal", b"bad").is_err());

        assert!(
            validate_policy_boundaries(u64::MAX, 2_000, 10, MAX_ATTACHMENT_BYTES, 9_975, 200)
                .is_ok()
        );
        assert!(validate_policy_boundaries(0, 10, 1, 100, 0, 100).is_err());
        assert_eq!(format_unread_badge(150), "99+");
        assert_eq!(classify_emoji_http_status(202, 0).unwrap(), "skeleton");
        assert!(classify_emoji_http_status(429, 1500).is_err());

        let redacted = redact_emoji_telemetry("auth=mfa.secret123456 token=abc");
        assert!(!redacted.contains("mfa.secret123456"));
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
