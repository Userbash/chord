#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

/// Default Discord CDN base URL.
pub const CDN_BASE_URL: &str = "https://cdn.discordapp.com";

/// Minimum allowed Discord CDN asset size (power of two).
pub const MIN_CDN_SIZE: u32 = 16;

/// Maximum allowed Discord CDN asset size (power of two).
pub const MAX_CDN_SIZE: u32 = 4096;

/// Maximum length for a Discord asset hash.
pub const MAX_ASSET_HASH_LEN: usize = 128;

/// Errors returned by Discord CDN URL builders and bounded memory cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CdnError {
    /// Asset hash is empty, exceeds 128 characters, or contains path traversal / injection characters.
    InvalidHash,
    /// Snowflake ID string is empty, contains non-ASCII digits, or overflows `u64`.
    InvalidSnowflake,
    /// Single asset byte length exceeds the cache's total `max_bytes` budget.
    AssetTooLarge,
}

impl fmt::Display for CdnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHash => write!(f, "invalid Discord CDN asset hash"),
            Self::InvalidSnowflake => write!(f, "invalid Discord Snowflake ID"),
            Self::AssetTooLarge => write!(f, "asset byte size exceeds cache max_bytes budget"),
        }
    }
}

impl std::error::Error for CdnError {}

/// Clamps a requested pixel dimension to a valid Discord CDN power-of-two size in `16..=4096`.
///
/// - Any `requested <= 16` clamps to `16`.
/// - Any `requested >= 4096` clamps to `4096`.
/// - Intermediate values round up to `.next_power_of_two()`.
#[must_use]
pub const fn clamp_cdn_size(requested: u32) -> u32 {
    if requested <= MIN_CDN_SIZE {
        MIN_CDN_SIZE
    } else if requested >= MAX_CDN_SIZE {
        MAX_CDN_SIZE
    } else {
        requested.next_power_of_two()
    }
}

/// Validates a Discord CDN asset hash against empty input, length limits (`<= 128`),
/// and path traversal / URL injection characters (`..`, `/`, `\`, `?`, `#`, whitespace, control chars).
pub fn validate_asset_hash(hash: &str) -> Result<&str, CdnError> {
    if hash.is_empty() || hash.len() > MAX_ASSET_HASH_LEN {
        return Err(CdnError::InvalidHash);
    }
    if hash.contains("..") {
        return Err(CdnError::InvalidHash);
    }
    if hash.chars().any(|c| {
        c.is_whitespace() || c.is_control() || matches!(c, '/' | '\\' | '?' | '#' | '%' | ':' | '@')
    }) {
        return Err(CdnError::InvalidHash);
    }
    Ok(hash)
}

/// Validates and parses a Discord Snowflake ID string (`0..=u64::MAX` ASCII digits).
pub fn parse_snowflake(raw: &str) -> Result<u64, CdnError> {
    if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return Err(CdnError::InvalidSnowflake);
    }
    u64::from_str(raw).map_err(|_| CdnError::InvalidSnowflake)
}

/// Computes the default Discord embed avatar index:
/// - Pomelo accounts (`discriminator` is `None` or `Some(0)`): `((user_id >> 22) % 6) as u8`
/// - Legacy 4-digit discriminator (`Some(d)` with `d > 0`): `(d % 5) as u8`
#[must_use]
pub const fn default_avatar_index(user_id: u64, discriminator: Option<u16>) -> u8 {
    match discriminator {
        None | Some(0) => ((user_id >> 22) % 6) as u8,
        Some(d) => (d % 5) as u8,
    }
}

#[inline]
fn select_dynamic_ext(hash: &str, prefer_animated: bool) -> &'static str {
    if prefer_animated && hash.starts_with("a_") {
        "gif"
    } else {
        "webp"
    }
}

/// Configurable Discord CDN URL builder client (defaults to [`CDN_BASE_URL`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdnClient {
    base_url: String,
}

impl Default for CdnClient {
    fn default() -> Self {
        Self::new()
    }
}

impl CdnClient {
    /// Creates a CDN client using [`CDN_BASE_URL`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            base_url: CDN_BASE_URL.to_string(),
        }
    }

    /// Creates a CDN client with a custom `base_url` (trailing `/` is stripped).
    #[must_use]
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        let raw = base_url.into();
        let trimmed = raw.trim_end_matches('/').to_string();
        Self {
            base_url: if trimmed.is_empty() {
                CDN_BASE_URL.to_string()
            } else {
                trimmed
            },
        }
    }

    /// Returns the configured base URL without a trailing slash.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Builds a user avatar URL or falls back to the default embed avatar PNG.
    pub fn user_avatar_url(
        &self,
        user_id: u64,
        avatar_hash: Option<&str>,
        discriminator: Option<u16>,
        size: u32,
        prefer_animated: bool,
    ) -> Result<String, CdnError> {
        match avatar_hash {
            Some(hash) => {
                let valid_hash = validate_asset_hash(hash)?;
                let clamped = clamp_cdn_size(size);
                let ext = select_dynamic_ext(valid_hash, prefer_animated);
                Ok(format!(
                    "{}/avatars/{user_id}/{valid_hash}.{ext}?size={clamped}",
                    self.base_url
                ))
            }
            None => {
                let index = default_avatar_index(user_id, discriminator);
                Ok(format!("{}/embed/avatars/{index}.png", self.base_url))
            }
        }
    }

    /// Builds a static `.png` user avatar URL for callers that require PNG over WebP.
    pub fn user_avatar_png_url(
        &self,
        user_id: u64,
        avatar_hash: &str,
        size: u32,
    ) -> Result<String, CdnError> {
        let valid_hash = validate_asset_hash(avatar_hash)?;
        let clamped = clamp_cdn_size(size);
        Ok(format!(
            "{}/avatars/{user_id}/{valid_hash}.png?size={clamped}",
            self.base_url
        ))
    }

    /// Builds a guild icon URL (`{base}/icons/{guild_id}/{icon_hash}.{gif|webp}?size={clamped}`).
    pub fn guild_icon_url(
        &self,
        guild_id: u64,
        icon_hash: &str,
        size: u32,
        prefer_animated: bool,
    ) -> Result<String, CdnError> {
        let valid_hash = validate_asset_hash(icon_hash)?;
        let clamped = clamp_cdn_size(size);
        let ext = select_dynamic_ext(valid_hash, prefer_animated);
        Ok(format!(
            "{}/icons/{guild_id}/{valid_hash}.{ext}?size={clamped}",
            self.base_url
        ))
    }

    /// Builds a guild banner URL (`{base}/banners/{guild_id}/{banner_hash}.{gif|webp}?size={clamped}`).
    pub fn guild_banner_url(
        &self,
        guild_id: u64,
        banner_hash: &str,
        size: u32,
        prefer_animated: bool,
    ) -> Result<String, CdnError> {
        let valid_hash = validate_asset_hash(banner_hash)?;
        let clamped = clamp_cdn_size(size);
        let ext = select_dynamic_ext(valid_hash, prefer_animated);
        Ok(format!(
            "{}/banners/{guild_id}/{valid_hash}.{ext}?size={clamped}",
            self.base_url
        ))
    }

    /// Builds a custom emoji URL (`{base}/emojis/{emoji_id}.{gif|webp}?size={clamped}&quality=lossless`).
    #[must_use]
    pub fn custom_emoji_url(&self, emoji_id: u64, animated: bool, size: u32) -> String {
        let clamped = clamp_cdn_size(size);
        let ext = if animated { "gif" } else { "webp" };
        format!(
            "{}/emojis/{emoji_id}.{ext}?size={clamped}&quality=lossless",
            self.base_url
        )
    }
}

/// Builds a user avatar URL using [`CDN_BASE_URL`].
pub fn user_avatar_url(
    user_id: u64,
    avatar_hash: Option<&str>,
    discriminator: Option<u16>,
    size: u32,
    prefer_animated: bool,
) -> Result<String, CdnError> {
    CdnClient::new().user_avatar_url(user_id, avatar_hash, discriminator, size, prefer_animated)
}

/// Builds a static `.png` user avatar URL using [`CDN_BASE_URL`].
pub fn user_avatar_png_url(user_id: u64, avatar_hash: &str, size: u32) -> Result<String, CdnError> {
    CdnClient::new().user_avatar_png_url(user_id, avatar_hash, size)
}

/// Builds a guild icon URL using [`CDN_BASE_URL`].
pub fn guild_icon_url(
    guild_id: u64,
    icon_hash: &str,
    size: u32,
    prefer_animated: bool,
) -> Result<String, CdnError> {
    CdnClient::new().guild_icon_url(guild_id, icon_hash, size, prefer_animated)
}

/// Builds a guild banner URL using [`CDN_BASE_URL`].
pub fn guild_banner_url(
    guild_id: u64,
    banner_hash: &str,
    size: u32,
    prefer_animated: bool,
) -> Result<String, CdnError> {
    CdnClient::new().guild_banner_url(guild_id, banner_hash, size, prefer_animated)
}

/// Builds a custom emoji URL using [`CDN_BASE_URL`].
#[must_use]
pub fn custom_emoji_url(emoji_id: u64, animated: bool, size: u32) -> String {
    CdnClient::new().custom_emoji_url(emoji_id, animated, size)
}

/// Decoded or fetched CDN asset stored in [`CdnMemoryCache`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedAsset {
    pub url: String,
    pub mime: String,
    pub width: u32,
    pub height: u32,
    pub bytes: Arc<[u8]>,
}

impl CachedAsset {
    /// Returns the byte size of the cached payload.
    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.bytes.len()
    }
}

/// Bounded byte-budget and entry-count LRU memory cache for CDN avatars and media assets.
#[derive(Debug, Clone)]
pub struct CdnMemoryCache {
    max_entries: usize,
    max_bytes: usize,
    current_bytes: usize,
    hits: u64,
    misses: u64,
    evictions: u64,
    /// Front (`0`) is least-recently-used; back is most-recently-used.
    entries: VecDeque<CachedAsset>,
}

impl CdnMemoryCache {
    /// Creates a bounded LRU cache enforcing both `max_entries` and `max_bytes`.
    #[must_use]
    pub fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            max_entries,
            max_bytes,
            current_bytes: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
            entries: VecDeque::new(),
        }
    }

    /// Maximum number of entries allowed in the cache.
    #[must_use]
    pub const fn max_entries(&self) -> usize {
        self.max_entries
    }

    /// Maximum total bytes allowed in the cache.
    #[must_use]
    pub const fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    /// Current total byte footprint of all cached assets.
    #[must_use]
    pub const fn current_bytes(&self) -> usize {
        self.current_bytes
    }

    /// Total cache hit count.
    #[must_use]
    pub const fn hits(&self) -> u64 {
        self.hits
    }

    /// Total cache miss count.
    #[must_use]
    pub const fn misses(&self) -> u64 {
        self.misses
    }

    /// Total number of LRU entries evicted to satisfy entry or byte budgets.
    #[must_use]
    pub const fn evictions(&self) -> u64 {
        self.evictions
    }

    /// Number of assets currently stored in the cache.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the cache contains no assets.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Looks up an asset by URL, promoting it to most-recently-used on hit and updating telemetry.
    pub fn get(&mut self, url: &str) -> Option<CachedAsset> {
        if let Some(pos) = self.entries.iter().position(|entry| entry.url == url) {
            self.hits += 1;
            let entry = self.entries.remove(pos)?;
            let cloned = entry.clone();
            self.entries.push_back(entry);
            Some(cloned)
        } else {
            self.misses += 1;
            None
        }
    }

    /// Inserts or updates a [`CachedAsset`] in the LRU cache.
    ///
    /// - If `asset.bytes.len() > self.max_bytes` (or `max_entries == 0`), returns
    ///   `Err(CdnError::AssetTooLarge)` **without** evicting any existing entries.
    /// - Otherwise, removes any existing entry with the same `url`, evicts least-recently-used
    ///   entries from the front until both `max_entries` and `max_bytes` invariants hold, and
    ///   pushes the new asset as most-recently-used.
    pub fn insert(&mut self, asset: CachedAsset) -> Result<(), CdnError> {
        let asset_bytes = asset.byte_len();
        if self.max_entries == 0 || asset_bytes > self.max_bytes {
            return Err(CdnError::AssetTooLarge);
        }

        // If updating an existing entry, remove the old version first without counting as eviction.
        if let Some(pos) = self.entries.iter().position(|e| e.url == asset.url) {
            if let Some(old) = self.entries.remove(pos) {
                self.current_bytes = self.current_bytes.saturating_sub(old.byte_len());
            }
        }

        while !self.entries.is_empty()
            && (self.entries.len() >= self.max_entries
                || self.current_bytes.saturating_add(asset_bytes) > self.max_bytes)
        {
            if let Some(evicted) = self.entries.pop_front() {
                self.current_bytes = self.current_bytes.saturating_sub(evicted.byte_len());
                self.evictions += 1;
            }
        }

        self.current_bytes += asset_bytes;
        self.entries.push_back(asset);
        Ok(())
    }

    /// Clears all entries from the cache and resets `current_bytes` to `0`.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_bytes = 0;
    }
}
