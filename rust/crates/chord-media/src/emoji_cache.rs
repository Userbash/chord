//! Парсер кастомных статических `<:name:id>` и анимированных GIF `<a:name:id>` эмодзи,
//! CDN-загрузчик (`https://cdn.discordapp.com/emojis/{id}.{webp|gif}?size=48&quality=lossless`)
//! и двухуровневый кэш (L1 ОЗУ LRU 256 МБ + L2 диск `~/.cache/discord-gpui/emojis/` с `ETag`) (`TASK-1005A`).

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

pub const MEDIA_CACHE_CONTAINER: &str = "Media Cache";
pub const MEDIA_CACHE_DIMENSIONS: &str = "LRU 256MB";
pub const MEDIA_CACHE_PADDING: &str = "0px";
pub const MEDIA_CACHE_SPACING: &str = "0px";
pub const TOKEN_BACKGROUND: &str = "#1e1f22";
pub const TOKEN_SURFACE: &str = "#2b2d31";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_BORDER: &str = "#3f4147";
pub const CACHE_HIT_ELEMENT: &str = "cache-hit";
pub const CACHE_HIT_STATE_CACHED: &str = "cached";
pub const CACHE_HIT_STATE_REVALIDATE: &str = "revalidate";
pub const CACHE_HIT_STATE_FETCHED: &str = "fetched";
pub const DEFAULT_EMOJI_CACHE_DIR: &str = "~/.cache/discord-gpui/emojis/";
pub const CDN_EMOJI_BASE_URL: &str = "https://cdn.discordapp.com/emojis/";
pub const LRU_RAM_BUDGET_BYTES: usize = 256 * 1024 * 1024;
pub const DEFAULT_DISK_BUDGET_BYTES: usize = 512 * 1024 * 1024;
pub const FRONTEND_D03_TAB_COUNT: usize = 3;
pub const FRONTEND_D03_NAV_CATEGORIES: usize = 10;
pub const FRONTEND_D03_VISIBLE_GRID_CHANNEL: usize = 50;
pub const FRONTEND_D03_LOCKED_CHANNEL: usize = 17;
pub const FRONTEND_D03_VISIBLE_GRID_DM: usize = 42;
pub const FRONTEND_D03_REACTION_PICKER_ITEMS: usize = 60;
pub const FRONTEND_D03_REACTION_PICKER_LOCKED: usize = 43;
pub const EMOJI_ITEM_CLASS: &str = "emojiItem_fc7141";
pub const LOCKED_EMOJI_CLASS: &str = "lockedEmoji_d982c2";
pub const CUSTOM_EMOJI_SYNTAX: &str = "<a:name:123> / <:name:123>";
pub const EMOJI_NAME_RE_PATTERN: &str = "^:([^\\s:]+?(?:::skin-tone-\\d)?):";
pub const READY_TOTAL_EMOJIS: usize = 363;
pub const MIN_EMOJI_NAME_CHARS: usize = 2;
pub const MAX_EMOJI_NAME_CHARS: usize = 32;
pub const MAX_JUMBO_EMOJIS: usize = 30;
pub const INLINE_EMOJI_SIZE_PX: u32 = 22;
pub const JUMBO_EMOJI_SIZE_PX: u32 = 48;
pub const ALLOWED_CDN_EMOJI_SIZES: [u32; 13] =
    [16, 20, 22, 24, 32, 40, 44, 48, 56, 60, 64, 96, 128];
pub const PERM_VIEW_CHANNEL: u64 = 1 << 10;
pub const PERM_SEND_MESSAGES: u64 = 1 << 11;
pub const PERM_USE_EXTERNAL_EMOJIS: u64 = 1 << 18;
pub const MAX_MESSAGE_CHARS: usize = 2_000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_BADGE_CAP: u32 = 99;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmojiCacheError {
    InvalidSnowflake(u64),
    InvalidEmojiName(String),
    InvalidShortcode(String),
    UnsupportedCdnSize(u32),
    PathTraversal(String),
    EmptyPayload,
    EntryExceedsBudget(usize, usize),
    MissingEtag,
    EtagMismatch(String, String),
    DiskCorrupted(String),
    IoFailure(String),
    BoundaryViolation(&'static str),
    AccessDenied(&'static str),
    AgeGatedSpaceRestricted,
    HttpError(u16, Option<u64>),
}

impl fmt::Display for EmojiCacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for EmojiCacheError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheHitState {
    Cached,
    Revalidate,
    Fetched,
}

impl CacheHitState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cached => CACHE_HIT_STATE_CACHED,
            Self::Revalidate => CACHE_HIT_STATE_REVALIDATE,
            Self::Fetched => CACHE_HIT_STATE_FETCHED,
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
pub struct ShortcodeToken {
    pub raw: String,
    pub name: String,
    pub skin_tone: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JumboEmojiLayout {
    pub tokens: Vec<ParsedEmojiToken>,
    pub jumboable: bool,
    pub render_size_px: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmojiCacheEntry {
    pub key: String,
    pub bytes: Vec<u8>,
    pub etag: String,
    pub checksum: u64,
    pub state: CacheHitState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdnFetchOutcome {
    pub entry: EmojiCacheEntry,
    pub state: CacheHitState,
    pub from_ram: bool,
    pub from_disk: bool,
}

pub fn validate_snowflake_id(id: u64) -> Result<u64, EmojiCacheError> {
    if id == 0 {
        return Err(EmojiCacheError::InvalidSnowflake(0));
    }
    Ok(id)
}

pub fn validate_emoji_name(name: &str) -> Result<(), EmojiCacheError> {
    let count = name.chars().count();
    let valid_chars = name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !(MIN_EMOJI_NAME_CHARS..=MAX_EMOJI_NAME_CHARS).contains(&count) || !valid_chars {
        return Err(EmojiCacheError::InvalidEmojiName(name.to_string()));
    }
    Ok(())
}

fn parse_single_emoji_tag(slice: &str, start_byte: usize) -> Option<(ParsedEmojiToken, usize)> {
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
    let id = validate_snowflake_id(id_str.parse::<u64>().ok()?).ok()?;
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
pub fn parse_emoji_tokens(text: &str) -> Vec<ParsedEmojiToken> {
    let (mut tokens, bytes, mut idx) = (Vec::new(), text.as_bytes(), 0);
    while idx < bytes.len() {
        if bytes[idx] == b'\\' && idx + 1 < bytes.len() && bytes[idx + 1] == b'<' {
            idx += 2;
            continue;
        }
        if bytes[idx] == b'<'
            && let Some(tail) = text.get(idx..)
            && let Some((tok, consumed)) = parse_single_emoji_tag(tail, idx)
        {
            tokens.push(tok);
            idx += consumed;
            continue;
        }
        idx += 1;
    }
    tokens
}

pub fn parse_shortcode_token(input: &str) -> Result<ShortcodeToken, EmojiCacheError> {
    if !input.starts_with(':') || !input.ends_with(':') || input.len() < 4 {
        return Err(EmojiCacheError::InvalidShortcode(input.to_string()));
    }
    let inner = &input[1..input.len() - 1];
    if let Some((base, tone_part)) = inner.split_once("::skin-tone-") {
        validate_emoji_name(base)?;
        let tone = tone_part
            .parse::<u8>()
            .ok()
            .filter(|&t| (1..=5).contains(&t))
            .ok_or_else(|| EmojiCacheError::InvalidShortcode(input.to_string()))?;
        return Ok(ShortcodeToken {
            raw: input.to_string(),
            name: base.to_string(),
            skin_tone: Some(tone),
        });
    }
    validate_emoji_name(inner)?;
    Ok(ShortcodeToken {
        raw: input.to_string(),
        name: inner.to_string(),
        skin_tone: None,
    })
}

#[must_use]
pub fn compute_jumbo_layout(text: &str) -> JumboEmojiLayout {
    let tokens = parse_emoji_tokens(text);
    let mut remainder = text.to_string();
    for tok in &tokens {
        remainder = remainder.replacen(&tok.raw, "", 1);
    }
    let jumboable = !tokens.is_empty()
        && remainder.chars().all(char::is_whitespace)
        && tokens.len() <= MAX_JUMBO_EMOJIS;
    let render_size_px = if jumboable {
        JUMBO_EMOJI_SIZE_PX
    } else {
        INLINE_EMOJI_SIZE_PX
    };
    JumboEmojiLayout {
        tokens,
        jumboable,
        render_size_px,
    }
}

pub fn build_emoji_cdn_url(
    id: u64,
    animated: bool,
    size: u32,
    lossless: bool,
) -> Result<String, EmojiCacheError> {
    let valid_id = validate_snowflake_id(id)?;
    if !ALLOWED_CDN_EMOJI_SIZES.contains(&size) {
        return Err(EmojiCacheError::UnsupportedCdnSize(size));
    }
    let ext = if animated { "gif" } else { "webp" };
    let suffix = if lossless { "&quality=lossless" } else { "" };
    Ok(format!(
        "{CDN_EMOJI_BASE_URL}{valid_id}.{ext}?size={size}{suffix}"
    ))
}

pub fn build_emoji_cdn_url_lossless(id: u64, animated: bool) -> Result<String, EmojiCacheError> {
    build_emoji_cdn_url(id, animated, JUMBO_EMOJI_SIZE_PX, true)
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash = (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

pub fn validate_cache_key(key: &str) -> Result<(), EmojiCacheError> {
    let valid_key = !key.is_empty()
        && !key.contains("..")
        && !key.contains('/')
        && !key.contains('\\')
        && !key.contains('\0')
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
    if !valid_key {
        return Err(EmojiCacheError::PathTraversal(key.to_string()));
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct DiskEntryMeta {
    file_path: PathBuf,
    etag_path: PathBuf,
    etag: String,
    checksum: u64,
}

#[derive(Debug)]
pub struct TwoTierEmojiCache {
    root_dir: PathBuf,
    max_ram_bytes: usize,
    max_ram_entries: usize,
    ram_used_bytes: usize,
    ram_lru: VecDeque<String>,
    ram_entries: BTreeMap<String, EmojiCacheEntry>,
    disk_index: BTreeMap<String, DiskEntryMeta>,
}

impl TwoTierEmojiCache {
    pub fn new(
        root_dir: PathBuf,
        max_ram_bytes: usize,
        max_ram_entries: usize,
    ) -> Result<Self, EmojiCacheError> {
        fs::create_dir_all(&root_dir).map_err(|e| EmojiCacheError::IoFailure(e.to_string()))?;
        Ok(Self {
            root_dir,
            max_ram_bytes: max_ram_bytes.max(1),
            max_ram_entries: max_ram_entries.max(1),
            ram_used_bytes: 0,
            ram_lru: VecDeque::new(),
            ram_entries: BTreeMap::new(),
            disk_index: BTreeMap::new(),
        })
    }

    #[must_use]
    pub fn resolve_default_dir(home_env: Option<&str>) -> String {
        match home_env.map(str::trim).filter(|s| !s.is_empty()) {
            Some(home) => format!("{}/.cache/discord-gpui/emojis/", home.trim_end_matches('/')),
            None => DEFAULT_EMOJI_CACHE_DIR.to_string(),
        }
    }

    fn touch_ram_lru(&mut self, key: &str) {
        self.ram_lru.retain(|k| k != key);
        self.ram_lru.push_back(key.to_string());
    }

    fn remove_from_ram(&mut self, key: &str) {
        self.ram_lru.retain(|k| k != key);
        if let Some(removed) = self.ram_entries.remove(key) {
            self.ram_used_bytes = self.ram_used_bytes.saturating_sub(removed.bytes.len());
        }
    }

    fn evict_ram_if_needed(&mut self, incoming_bytes: usize) {
        while self.ram_used_bytes.saturating_add(incoming_bytes) > self.max_ram_bytes
            || self.ram_entries.len() >= self.max_ram_entries
        {
            let Some(oldest) = self.ram_lru.pop_front() else {
                break;
            };
            self.remove_from_ram(&oldest);
        }
    }

    fn write_to_disk(
        &mut self,
        key: &str,
        bytes: &[u8],
        etag: &str,
        checksum: u64,
    ) -> Result<(), EmojiCacheError> {
        let (file_path, etag_path) = (
            self.root_dir.join(key),
            self.root_dir.join(format!("{key}.etag")),
        );
        fs::write(&file_path, bytes).map_err(|e| EmojiCacheError::IoFailure(e.to_string()))?;
        fs::write(&etag_path, etag.as_bytes())
            .map_err(|e| EmojiCacheError::IoFailure(e.to_string()))?;
        self.disk_index.insert(
            key.to_string(),
            DiskEntryMeta {
                file_path,
                etag_path,
                etag: etag.to_string(),
                checksum,
            },
        );
        Ok(())
    }

    fn insert_ram_entry(
        &mut self,
        key: &str,
        bytes: Vec<u8>,
        etag: String,
        checksum: u64,
        state: CacheHitState,
    ) -> EmojiCacheEntry {
        self.evict_ram_if_needed(bytes.len());
        self.ram_used_bytes = self.ram_used_bytes.saturating_add(bytes.len());
        let entry = EmojiCacheEntry {
            key: key.to_string(),
            bytes,
            etag,
            checksum,
            state,
        };
        self.ram_entries.insert(key.to_string(), entry.clone());
        self.touch_ram_lru(key);
        entry
    }

    pub fn put(
        &mut self,
        key: &str,
        bytes: &[u8],
        etag: &str,
    ) -> Result<EmojiCacheEntry, EmojiCacheError> {
        validate_cache_key(key)?;
        if bytes.is_empty() {
            return Err(EmojiCacheError::EmptyPayload);
        }
        if bytes.len() > self.max_ram_bytes {
            return Err(EmojiCacheError::EntryExceedsBudget(
                bytes.len(),
                self.max_ram_bytes,
            ));
        }
        let clean_etag = etag.trim();
        if clean_etag.is_empty() {
            return Err(EmojiCacheError::MissingEtag);
        }
        self.remove_from_ram(key);
        let checksum = fnv1a64(bytes);
        self.write_to_disk(key, bytes, clean_etag, checksum)?;
        Ok(self.insert_ram_entry(
            key,
            bytes.to_vec(),
            clean_etag.to_string(),
            checksum,
            CacheHitState::Fetched,
        ))
    }

    pub fn get(&mut self, key: &str) -> Result<Option<CdnFetchOutcome>, EmojiCacheError> {
        validate_cache_key(key)?;
        if let Some(mut entry) = self.ram_entries.get(key).cloned() {
            entry.state = CacheHitState::Cached;
            self.touch_ram_lru(key);
            return Ok(Some(CdnFetchOutcome {
                entry,
                state: CacheHitState::Cached,
                from_ram: true,
                from_disk: false,
            }));
        }
        self.promote_from_disk(key, CacheHitState::Cached)
    }

    fn promote_from_disk(
        &mut self,
        key: &str,
        target_state: CacheHitState,
    ) -> Result<Option<CdnFetchOutcome>, EmojiCacheError> {
        let Some(meta) = self.disk_index.get(key).cloned() else {
            return Ok(None);
        };
        let Ok(disk_bytes) = fs::read(&meta.file_path) else {
            self.purge_key(key);
            return Ok(None);
        };
        if fnv1a64(&disk_bytes) != meta.checksum {
            self.purge_key(key);
            return Err(EmojiCacheError::DiskCorrupted(key.to_string()));
        }
        let entry = self.insert_ram_entry(key, disk_bytes, meta.etag, meta.checksum, target_state);
        Ok(Some(CdnFetchOutcome {
            entry,
            state: target_state,
            from_ram: false,
            from_disk: true,
        }))
    }

    pub fn prepare_if_none_match(&self, key: &str) -> Result<Option<String>, EmojiCacheError> {
        validate_cache_key(key)?;
        if let Some(ram) = self.ram_entries.get(key) {
            return Ok(Some(ram.etag.clone()));
        }
        Ok(self.disk_index.get(key).map(|m| m.etag.clone()))
    }

    pub fn handle_cdn_revalidation(
        &mut self,
        key: &str,
        status: u16,
        etag: &str,
        body: Option<&[u8]>,
    ) -> Result<CdnFetchOutcome, EmojiCacheError> {
        validate_cache_key(key)?;
        match status {
            200 => {
                let bytes = body.ok_or(EmojiCacheError::EmptyPayload)?;
                let entry = self.put(key, bytes, etag)?;
                Ok(CdnFetchOutcome {
                    entry,
                    state: CacheHitState::Fetched,
                    from_ram: true,
                    from_disk: true,
                })
            }
            304 => self.complete_not_modified(key, etag),
            other => Err(EmojiCacheError::HttpError(other, None)),
        }
    }

    fn complete_not_modified(
        &mut self,
        key: &str,
        etag: &str,
    ) -> Result<CdnFetchOutcome, EmojiCacheError> {
        let expected = self
            .prepare_if_none_match(key)?
            .ok_or(EmojiCacheError::MissingEtag)?;
        if expected != etag.trim() {
            return Err(EmojiCacheError::EtagMismatch(
                expected,
                etag.trim().to_string(),
            ));
        }
        if let Some(mut entry) = self.ram_entries.get(key).cloned() {
            entry.state = CacheHitState::Revalidate;
            self.touch_ram_lru(key);
            return Ok(CdnFetchOutcome {
                entry,
                state: CacheHitState::Revalidate,
                from_ram: true,
                from_disk: false,
            });
        }
        self.promote_from_disk(key, CacheHitState::Revalidate)?
            .ok_or_else(|| EmojiCacheError::DiskCorrupted(key.to_string()))
    }

    pub fn purge_key(&mut self, key: &str) {
        self.remove_from_ram(key);
        if let Some(meta) = self.disk_index.remove(key) {
            let _ = fs::remove_file(meta.file_path);
            let _ = fs::remove_file(meta.etag_path);
        }
    }

    #[must_use]
    pub const fn ram_used_bytes(&self) -> usize {
        self.ram_used_bytes
    }
    #[must_use]
    pub fn ram_len(&self) -> usize {
        self.ram_entries.len()
    }
    #[must_use]
    pub fn disk_len(&self) -> usize {
        self.disk_index.len()
    }
    #[must_use]
    pub fn root_dir(&self) -> &Path {
        self.root_dir.as_path()
    }
}

pub fn validate_emoji_policy_boundaries(
    snowflake: u64,
    message_chars: usize,
    attachments_count: usize,
    attachment_bytes: u64,
    search_offset: u32,
    volume_percent: u16,
) -> Result<(), EmojiCacheError> {
    validate_snowflake_id(snowflake)?;
    if message_chars > MAX_MESSAGE_CHARS {
        return Err(EmojiCacheError::BoundaryViolation("message_chars > 2000"));
    }
    if attachments_count > MAX_ATTACHMENTS_COUNT {
        return Err(EmojiCacheError::BoundaryViolation("attachments_count > 10"));
    }
    if attachment_bytes > MAX_ATTACHMENT_BYTES {
        return Err(EmojiCacheError::BoundaryViolation(
            "attachment_bytes > 25MB",
        ));
    }
    if search_offset > MAX_SEARCH_OFFSET {
        return Err(EmojiCacheError::BoundaryViolation("search_offset > 9975"));
    }
    if volume_percent > MAX_VOLUME_PERCENT {
        return Err(EmojiCacheError::BoundaryViolation("volume_percent > 200"));
    }
    Ok(())
}

pub fn classify_cdn_http_status(
    status: u16,
    retry_after_ms: Option<u64>,
) -> Result<&'static str, EmojiCacheError> {
    match status {
        200 | 201 | 204 => Ok("ready"),
        202 => Ok("skeleton"),
        304 => Ok("revalidate"),
        400 | 401 | 403 | 404 | 413 | 429 | 500..=599 => {
            Err(EmojiCacheError::HttpError(status, retry_after_ms))
        }
        other => Err(EmojiCacheError::HttpError(other, None)),
    }
}

pub fn evaluate_emoji_permissions(
    permissions: u64,
    is_external_guild: bool,
    animated: bool,
    has_nitro: bool,
    age_gated_channel: bool,
    age_verified: bool,
) -> Result<&'static str, EmojiCacheError> {
    if (permissions & PERM_VIEW_CHANNEL) == 0 {
        return Err(EmojiCacheError::AccessDenied("missing VIEW_CHANNEL"));
    }
    if age_gated_channel && !age_verified {
        return Err(EmojiCacheError::AgeGatedSpaceRestricted);
    }
    if is_external_guild && (permissions & PERM_USE_EXTERNAL_EMOJIS) == 0 {
        return Err(EmojiCacheError::AccessDenied("missing USE_EXTERNAL_EMOJIS"));
    }
    if (animated || is_external_guild) && !has_nitro {
        return Ok(LOCKED_EMOJI_CLASS);
    }
    Ok(EMOJI_ITEM_CLASS)
}

#[must_use]
pub fn format_badge_count(count: u32) -> String {
    if count > MAX_BADGE_CAP {
        "99+".to_string()
    } else {
        count.to_string()
    }
}

#[must_use]
pub fn redact_emoji_cache_log(line: &str) -> String {
    let mut out = String::new();
    for tok in line.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        let sensitive = tok.contains("mfa.")
            || tok.contains("Bearer")
            || tok.starts_with("token=")
            || tok.starts_with("auth=");
        out.push_str(if sensitive { "[REDACTED]" } else { tok });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sub_1005a_1_tokens_layout_states_and_re_oracles() {
        assert_eq!(MEDIA_CACHE_CONTAINER, "Media Cache");
        assert_eq!(MEDIA_CACHE_DIMENSIONS, "LRU 256MB");
        assert_eq!(MEDIA_CACHE_PADDING, "0px");
        assert_eq!(MEDIA_CACHE_SPACING, "0px");
        assert_eq!(TOKEN_BACKGROUND, "#1e1f22");
        assert_eq!(TOKEN_SURFACE, "#2b2d31");
        assert_eq!(TOKEN_TEXT_PRIMARY, "#ffffff");
        assert_eq!(TOKEN_ACCENT, "#5865f2");
        assert_eq!(TOKEN_BORDER, "#3f4147");
        assert_eq!(CACHE_HIT_ELEMENT, "cache-hit");
        assert_eq!(CacheHitState::Cached.as_str(), "cached");
        assert_eq!(CacheHitState::Revalidate.as_str(), "revalidate");
        assert_eq!(CacheHitState::Fetched.as_str(), "fetched");
        assert_eq!(LRU_RAM_BUDGET_BYTES, 268_435_456);
        assert_eq!(FRONTEND_D03_TAB_COUNT, 3);
        assert_eq!(FRONTEND_D03_NAV_CATEGORIES, 10);
        assert_eq!(READY_TOTAL_EMOJIS, 363);
    }

    #[test]
    fn test_sub_1005a_2_regex_parser_shortcodes_jumbo_and_cdn_urls() {
        let src = "Hi <:CoP:3527262836281581568> <a:party_blob:1056141433670742056> \\<:skip:123>";
        let parsed = parse_emoji_tokens(src);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].name, "CoP");
        assert_eq!(parsed[0].id, 3_527_262_836_281_581_568);
        assert!(!parsed[0].animated);
        assert_eq!(parsed[1].name, "party_blob");
        assert_eq!(parsed[1].id, 1_056_141_433_670_742_056);
        assert!(parsed[1].animated);
        let sc = parse_shortcode_token(":wave::skin-tone-3:").unwrap();
        assert_eq!(sc.name, "wave");
        assert_eq!(sc.skin_tone, Some(3));
        let jumbo = compute_jumbo_layout("<:CoP:3527262836281581568> <a:wave:1056141433670742056>");
        assert!(jumbo.jumboable);
        assert_eq!(jumbo.render_size_px, 48);
        let u1 = build_emoji_cdn_url_lossless(3_527_262_836_281_581_568, false).unwrap();
        assert_eq!(
            u1,
            "https://cdn.discordapp.com/emojis/3527262836281581568.webp?size=48&quality=lossless"
        );
        let u2 = build_emoji_cdn_url(1_056_141_433_670_742_056, true, 64, true).unwrap();
        assert_eq!(
            u2,
            "https://cdn.discordapp.com/emojis/1056141433670742056.gif?size=64&quality=lossless"
        );
        assert!(build_emoji_cdn_url(0, false, 48, true).is_err());
    }

    #[test]
    fn test_sub_1005a_3_two_tier_lru_cache_etag_and_path_traversal() {
        let def_dir = TwoTierEmojiCache::resolve_default_dir(Some("/home/user"));
        assert_eq!(def_dir, "/home/user/.cache/discord-gpui/emojis/");
        let dir = std::env::temp_dir().join(format!("emoji_cache_1005a_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut cache = TwoTierEmojiCache::new(dir.clone(), 24, 10).unwrap();
        let e1 = cache.put("e1.webp", b"0123456789", "\"etag-v1\"").unwrap();
        assert_eq!(e1.state, CacheHitState::Fetched);
        let _ = cache.put("e2.gif", b"abcdefghij", "\"etag-v2\"").unwrap();
        let hit1 = cache.get("e1.webp").unwrap().unwrap();
        assert_eq!(hit1.state, CacheHitState::Cached);
        assert!(hit1.from_ram);
        let ram_reval = cache
            .handle_cdn_revalidation("e1.webp", 304, "\"etag-v1\"", None)
            .unwrap();
        assert_eq!(ram_reval.state, CacheHitState::Revalidate);
        assert_eq!(ram_reval.entry.state, CacheHitState::Revalidate);
        let _ = cache.put("e3.webp", b"klmnopqrst", "\"etag-v3\"").unwrap();
        assert_eq!(cache.ram_len(), 2);
        assert!(cache.ram_used_bytes() <= 24);
        assert_eq!(cache.disk_len(), 3);
        let disk_reval = cache
            .handle_cdn_revalidation("e2.gif", 304, "\"etag-v2\"", None)
            .unwrap();
        assert_eq!(disk_reval.state, CacheHitState::Revalidate);
        assert!(disk_reval.from_disk);
        assert!(cache.put("../evil.webp", b"bad", "\"e\"").is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_sub_1005a_4_nine_policy_dimensions_and_corruption_recovery() {
        let ok_b =
            validate_emoji_policy_boundaries(u64::MAX, 2_000, 10, MAX_ATTACHMENT_BYTES, 9_975, 200);
        assert!(ok_b.is_ok());
        assert!(validate_emoji_policy_boundaries(0, 10, 1, 100, 0, 100).is_err());
        assert_eq!(classify_cdn_http_status(202, None).unwrap(), "skeleton");
        assert_eq!(classify_cdn_http_status(304, None).unwrap(), "revalidate");
        assert!(classify_cdn_http_status(429, Some(1500)).is_err());
        assert_eq!(format_badge_count(150), "99+");
        let p_locked = evaluate_emoji_permissions(
            PERM_VIEW_CHANNEL | PERM_USE_EXTERNAL_EMOJIS,
            true,
            true,
            false,
            false,
            true,
        )
        .unwrap();
        assert_eq!(p_locked, LOCKED_EMOJI_CLASS);
        assert!(
            evaluate_emoji_permissions(PERM_VIEW_CHANNEL, false, false, true, true, false).is_err()
        );
        let dir = std::env::temp_dir().join(format!("emoji_cache_corrupt_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut cache = TwoTierEmojiCache::new(dir.clone(), 64, 4).unwrap();
        let _ = cache.put("corrupt.webp", b"valid_bytes", "\"e1\"").unwrap();
        cache.remove_from_ram("corrupt.webp");
        fs::write(dir.join("corrupt.webp"), b"tampered").unwrap();
        assert!(matches!(
            cache.get("corrupt.webp"),
            Err(EmojiCacheError::DiskCorrupted(_))
        ));
        assert!(
            !redact_emoji_cache_log("auth=mfa.secret987654 token=xyz").contains("mfa.secret987654")
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
