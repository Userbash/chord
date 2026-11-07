//! Полнотекстовый поиск сообщений по гильдии и каналу (TASK-617):
//! `GET /api/v9/guilds/{guild_id}/messages/search` и
//! `GET /api/v9/channels/{channel_id}/messages/search`.
//!
//! Оракул RE: `/home/sanya/re-deep/decoded/chunk-b929d27a8474c41c.js` (`SEARCH_MESSAGES`),
//! `waves/RW-40/SEARCH-202-NSFW-RE.md` (§2–§6) и `docs/re/rest-d02.md` (§6.1).
//!
//! Контракт модуля:
//! 1. Разбор и нормализация инлайн-фасетов поисковой строки (`from:`, `mentions:`, `has:`, `in:`,
//!    `before:`, `after:`, `pinned:`, `author_type:`, `link_hostname:`, `attachment_extension:`,
//!    `attachment_filename:`) и детерминированная сборка query string.
//! 2. Валидация параметров (`content` до 512 символов, `offset <= 9975`, `limit` `1..=25`,
//!    Snowflake ID, 9 подтверждённых токенов `has`, 3 значения `author_type`, `min_id <= max_id`).
//! 3. Сборка [`HttpRequest`] с полным набором из 6 обязательных заголовков Discord REST v9
//!    (`Authorization`, `X-Super-Properties`, `X-Discord-Locale: ru`, `X-Discord-Timezone`,
//!    `X-Debug-Options: bugReporterEnabled`, `Accept-Language: ru`).
//! 4. Декодирование HTTP-ответов:
//!    - `200 OK` с вложенными контекстными группами `messages: Vec<Vec<SearchedMessage>>`
//!      (сообщение с `"hit": true` + соседние контекстные сообщения до/после),
//!      `total_results`, `doing_deep_historical_index`, `documents_indexed`, `analytics_id`;
//!    - `202 Accepted` (индексация гильдии/канала: `indexing: true`, `retry_after_ms`);
//!    - `429 Too Many Requests` (`RateLimited { retry_after_ms, is_global }` и расчёт
//!      экспоненциального бэкоффа `compute_429_exponential_backoff_ms`);
//!    - `401`, `403`, `404`, `4xx`, `5xx`.

use std::fmt;

use discord_json::{JsonValue, parse};

use super::client::{ApiAuth, ApiAuthError};
use super::routes::HttpMethod;
use super::super_properties::SuperProperties;
use super::transport::{HttpRequest, HttpResponse};
use crate::API_V9_BASE;

/// Максимальное смещение пагинации поиска Discord (`offset <= 9975`, 400 страниц по 25).
pub const MAX_SEARCH_OFFSET: u32 = 9975;

/// Размер страницы поиска по умолчанию (`25`).
pub const DEFAULT_SEARCH_LIMIT: u8 = 25;

/// Максимальный размер страницы поиска (`1..=25`).
pub const MAX_SEARCH_LIMIT: u8 = 25;

/// Максимальная длина текстового фрагмента `content` (RE §2.6: `maxLength: 512`).
pub const MAX_SEARCH_CONTENT_CHARS: usize = 512;

/// Задержка по умолчанию для `HTTP 202 Accepted`, если `Retry-After` отсутствует (`5000` мс).
pub const DEFAULT_SEARCH_202_RETRY_AFTER_MS: u64 = 5000;

/// Базовая задержка по умолчанию для `HTTP 429 Too Many Requests` (`1000` мс).
pub const DEFAULT_SEARCH_429_RETRY_AFTER_MS: u64 = 1000;

/// Верхний потолок экспоненциального бэкоффа при `HTTP 429` (`30_000` мс).
pub const MAX_SEARCH_BACKOFF_MS: u64 = 30_000;

/// Локаль по умолчанию для `X-Discord-Locale` (`ru`).
pub const DISCORD_LOCALE_RU: &str = "ru";

/// Подтверждённые RE фасеты фильтра `has` (`chunk-b929d27a8474c41c.js`).
pub const ALLOWED_HAS_FACETS: &[&str] = &[
    "link", "embed", "poll", "snapshot", "file", "video", "image", "sound", "sticker",
];

/// Подтверждённые RE значения фильтра `author_type`.
pub const ALLOWED_AUTHOR_TYPES: &[&str] = &["user", "bot", "webhook"];

/// Поле сортировки результатов поиска (`sort_by`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchSortBy {
    /// Сортировка по времени создания сообщения (`timestamp`).
    Timestamp,
    /// Сортировка по релевантности (`relevance`).
    Relevance,
}

impl SearchSortBy {
    /// Строковое представление для query string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Timestamp => "timestamp",
            Self::Relevance => "relevance",
        }
    }

    /// Разобрать значение `sort_by` из строки.
    pub fn parse_str(raw: &str) -> Option<Self> {
        match raw.trim() {
            "timestamp" => Some(Self::Timestamp),
            "relevance" => Some(Self::Relevance),
            _ => None,
        }
    }
}

/// Направление сортировки результатов поиска (`sort_order`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchSortOrder {
    /// По убыванию (`desc`).
    Desc,
    /// По возрастанию (`asc`).
    Asc,
}

impl SearchSortOrder {
    /// Строковое представление для query string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Desc => "desc",
            Self::Asc => "asc",
        }
    }

    /// Разобрать значение `sort_order` из строки.
    pub fn parse_str(raw: &str) -> Option<Self> {
        match raw.trim() {
            "desc" => Some(Self::Desc),
            "asc" => Some(Self::Asc),
            _ => None,
        }
    }
}

/// Нормализованный запрос полнотекстового поиска сообщений Discord REST v9.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchQueryRequest {
    /// Свободный текст поискового запроса (`content`).
    pub content: Option<String>,
    /// Фильтр по каналам внутри гильдии (`channel_id` / `in:`).
    pub channel_id: Vec<String>,
    /// Фильтр по авторам сообщений (`author_id` / `from:`).
    pub author_id: Vec<String>,
    /// Фильтр по упоминаниям пользователей (`mentions` / `mentions:`).
    pub mentions: Vec<String>,
    /// Фильтр по типу содержимого (`has` / `has:`, допускается отрицание `-`).
    pub has: Vec<String>,
    /// Фильтр по закреплённым сообщениям (`pinned`).
    pub pinned: Option<bool>,
    /// Фильтр по типу автора (`author_type`: `user`, `bot`, `webhook`, допускается `-`).
    pub author_type: Vec<String>,
    /// Нижняя граница Snowflake ID (`min_id` / `after:`).
    pub min_id: Option<String>,
    /// Верхняя граница Snowflake ID (`max_id` / `before:`).
    pub max_id: Option<String>,
    /// Фильтр по домену ссылок (`link_hostname`).
    pub link_hostname: Vec<String>,
    /// Фильтр по расширению вложений без ведущей точки в нижнем регистре (`attachment_extension`).
    pub attachment_extension: Vec<String>,
    /// Фильтр по имени файла вложения (`attachment_filename`).
    pub attachment_filename: Vec<String>,
    /// Поле сортировки (`sort_by`).
    pub sort_by: Option<SearchSortBy>,
    /// Направление сортировки (`sort_order`).
    pub sort_order: Option<SearchSortOrder>,
    /// Размер страницы (`limit`, `1..=25`).
    pub limit: Option<u8>,
    /// Смещение пагинации (`offset`, `0..=9975`).
    pub offset: Option<u32>,
    /// Включать ли результаты из NSFW-каналов (`include_nsfw`).
    pub include_nsfw: Option<bool>,
    /// Идентификатор поисковой сессии (`search_session_id`, UUID v4).
    pub search_session_id: Option<String>,
    /// Идентификатор конкретного поискового запроса (`search_query_id`, UUID v4).
    pub search_query_id: Option<String>,
    /// Счётчик повторных попыток при индексации `HTTP 202` (`attempts`).
    pub attempts: Option<u32>,
}

impl SearchQueryRequest {
    /// Разобрать сырую поисковую строку с инлайн-фасетами (`from:`, `mentions:`, `has:`, `in:`,
    /// `before:`, `after:`, `pinned:`, `author_type:`, `link_hostname:`, `attachment_extension:`,
    /// `attachment_filename:`) и вернуть нормализованный [`SearchQueryRequest`].
    pub fn parse_and_normalize(raw_query: &str) -> Result<Self, SearchRestError> {
        let mut req = Self::default();
        req.apply_inline_query_tokens(raw_query);
        req.normalized()
    }

    /// Извлечь инлайн-токены из строки и пополнить поля структуры без потери уже заданных фасетов.
    pub fn apply_inline_query_tokens(&mut self, raw_query: &str) {
        let mut free_words = Vec::new();
        for token in raw_query.split_whitespace() {
            if let Some((raw_key, raw_val)) = token.split_once(':') {
                let key = raw_key.to_ascii_lowercase();
                let val = raw_val.trim();
                if val.is_empty() {
                    free_words.push(token);
                    continue;
                }
                match key.as_str() {
                    "from" | "author_id" => {
                        self.author_id.push(val.to_string());
                    }
                    "mentions" => {
                        self.mentions.push(val.to_string());
                    }
                    "has" => {
                        self.has.push(val.to_ascii_lowercase());
                    }
                    "in" | "channel_id" => {
                        self.channel_id.push(val.to_string());
                    }
                    "before" | "max_id" => {
                        self.max_id = Some(val.to_string());
                    }
                    "after" | "min_id" => {
                        self.min_id = Some(val.to_string());
                    }
                    "pinned" => match val.to_ascii_lowercase().as_str() {
                        "true" => self.pinned = Some(true),
                        "false" => self.pinned = Some(false),
                        _ => free_words.push(token),
                    },
                    "author_type" => {
                        self.author_type.push(val.to_ascii_lowercase());
                    }
                    "link_hostname" | "site" => {
                        self.link_hostname.push(val.to_ascii_lowercase());
                    }
                    "attachment_extension" | "ext" => {
                        let cleaned = val.trim_start_matches('.').to_ascii_lowercase();
                        if !cleaned.is_empty() {
                            self.attachment_extension.push(cleaned);
                        }
                    }
                    "attachment_filename" | "filename" => {
                        self.attachment_filename.push(val.to_string());
                    }
                    _ => free_words.push(token),
                }
            } else {
                free_words.push(token);
            }
        }

        let joined = free_words.join(" ");
        if !joined.is_empty() {
            match &mut self.content {
                Some(existing) if !existing.trim().is_empty() => {
                    let combined = format!("{} {}", existing.trim(), joined);
                    *existing = collapse_whitespace(&combined);
                }
                _ => {
                    self.content = Some(joined);
                }
            }
        }
    }

    /// Проверить и нормализовать все поля поискового запроса.
    pub fn normalized(&self) -> Result<Self, SearchRestError> {
        let mut out = self.clone();
        if let Some(raw_content) = out.content.take() {
            out.apply_inline_query_tokens(&raw_content);
        }

        if let Some(text) = &mut out.content {
            let collapsed = collapse_whitespace(text);
            if collapsed.is_empty() {
                out.content = None;
            } else {
                let char_len = collapsed.chars().count();
                if char_len > MAX_SEARCH_CONTENT_CHARS {
                    return Err(SearchRestError::ContentTooLong {
                        len: char_len,
                        max: MAX_SEARCH_CONTENT_CHARS,
                    });
                }
                *text = collapsed;
            }
        }

        out.channel_id = normalize_snowflake_vec(&out.channel_id, "channel_id")?;
        out.author_id = normalize_snowflake_vec(&out.author_id, "author_id")?;
        out.mentions = normalize_snowflake_vec(&out.mentions, "mentions")?;

        let mut normalized_has = Vec::with_capacity(out.has.len());
        for raw_has in &out.has {
            let lowered = raw_has.trim().to_ascii_lowercase();
            let core = lowered.strip_prefix('-').unwrap_or(lowered.as_str());
            if core.is_empty() || !ALLOWED_HAS_FACETS.contains(&core) {
                return Err(SearchRestError::InvalidHasFacet(raw_has.clone()));
            }
            if !normalized_has.contains(&lowered) {
                normalized_has.push(lowered);
            }
        }
        out.has = normalized_has;

        let mut normalized_author_type = Vec::with_capacity(out.author_type.len());
        for raw_at in &out.author_type {
            let lowered = raw_at.trim().to_ascii_lowercase();
            let core = lowered.strip_prefix('-').unwrap_or(lowered.as_str());
            if core.is_empty() || !ALLOWED_AUTHOR_TYPES.contains(&core) {
                return Err(SearchRestError::InvalidAuthorType(raw_at.clone()));
            }
            if !normalized_author_type.contains(&lowered) {
                normalized_author_type.push(lowered);
            }
        }
        out.author_type = normalized_author_type;

        let min_u64 = match &out.min_id {
            Some(raw) => {
                let trimmed = raw.trim();
                let parsed = parse_valid_snowflake(trimmed).ok_or_else(|| {
                    SearchRestError::InvalidSnowflake {
                        field: "min_id",
                        value: raw.clone(),
                    }
                })?;
                out.min_id = Some(trimmed.to_string());
                Some(parsed)
            }
            None => None,
        };

        let max_u64 = match &out.max_id {
            Some(raw) => {
                let trimmed = raw.trim();
                let parsed = parse_valid_snowflake(trimmed).ok_or_else(|| {
                    SearchRestError::InvalidSnowflake {
                        field: "max_id",
                        value: raw.clone(),
                    }
                })?;
                out.max_id = Some(trimmed.to_string());
                Some(parsed)
            }
            None => None,
        };

        if let (Some(min_val), Some(max_val)) = (min_u64, max_u64)
            && min_val > max_val
        {
            return Err(SearchRestError::InvalidFilterValue {
                field: "min_id_gt_max_id",
            });
        }

        out.link_hostname = normalize_lowercase_vec(&out.link_hostname, "link_hostname", false)?;
        out.attachment_extension =
            normalize_lowercase_vec(&out.attachment_extension, "attachment_extension", true)?;
        out.attachment_filename =
            normalize_trimmed_vec(&out.attachment_filename, "attachment_filename")?;

        if let Some(limit) = out.limit
            && !(1..=MAX_SEARCH_LIMIT).contains(&limit)
        {
            return Err(SearchRestError::InvalidLimit { limit });
        }

        if let Some(offset) = out.offset
            && offset > MAX_SEARCH_OFFSET
        {
            return Err(SearchRestError::OffsetOutOfRange {
                offset,
                max: MAX_SEARCH_OFFSET,
            });
        }

        let has_any_criteria = out.content.is_some()
            || !out.channel_id.is_empty()
            || !out.author_id.is_empty()
            || !out.mentions.is_empty()
            || !out.has.is_empty()
            || out.pinned.is_some()
            || !out.author_type.is_empty()
            || out.min_id.is_some()
            || out.max_id.is_some()
            || !out.link_hostname.is_empty()
            || !out.attachment_extension.is_empty()
            || !out.attachment_filename.is_empty();

        if !has_any_criteria {
            return Err(SearchRestError::EmptyQuery);
        }

        Ok(out)
    }

    /// Собрать каноническую строку query-параметров (без ведущего `?`).
    pub fn to_query_string(&self) -> Result<String, SearchRestError> {
        let norm = self.normalized()?;
        let mut pairs: Vec<(&str, String)> = Vec::new();

        if let Some(content) = &norm.content {
            pairs.push(("content", url_encode(content)));
        }
        for ch in &norm.channel_id {
            pairs.push(("channel_id", url_encode(ch)));
        }
        for author in &norm.author_id {
            pairs.push(("author_id", url_encode(author)));
        }
        for mention in &norm.mentions {
            pairs.push(("mentions", url_encode(mention)));
        }
        for h in &norm.has {
            pairs.push(("has", url_encode(h)));
        }
        if let Some(pinned) = norm.pinned {
            pairs.push(("pinned", pinned.to_string()));
        }
        for at in &norm.author_type {
            pairs.push(("author_type", url_encode(at)));
        }
        if let Some(min_id) = &norm.min_id {
            pairs.push(("min_id", url_encode(min_id)));
        }
        if let Some(max_id) = &norm.max_id {
            pairs.push(("max_id", url_encode(max_id)));
        }
        for host in &norm.link_hostname {
            pairs.push(("link_hostname", url_encode(host)));
        }
        for ext in &norm.attachment_extension {
            pairs.push(("attachment_extension", url_encode(ext)));
        }
        for fname in &norm.attachment_filename {
            pairs.push(("attachment_filename", url_encode(fname)));
        }
        if let Some(sort_by) = norm.sort_by {
            pairs.push(("sort_by", sort_by.as_str().to_string()));
        }
        if let Some(sort_order) = norm.sort_order {
            pairs.push(("sort_order", sort_order.as_str().to_string()));
        }
        if let Some(limit) = norm.limit {
            pairs.push(("limit", limit.to_string()));
        }
        if let Some(offset) = norm.offset {
            pairs.push(("offset", offset.to_string()));
        }
        if norm.include_nsfw == Some(true) {
            pairs.push(("include_nsfw", "true".to_string()));
        }
        if let Some(session_id) = &norm.search_session_id
            && !session_id.trim().is_empty()
        {
            pairs.push(("search_session_id", url_encode(session_id.trim())));
        }
        if let Some(query_id) = &norm.search_query_id
            && !query_id.trim().is_empty()
        {
            pairs.push(("search_query_id", url_encode(query_id.trim())));
        }
        if let Some(attempts) = norm.attempts
            && attempts > 0
        {
            pairs.push(("attempts", attempts.to_string()));
        }

        Ok(pairs
            .into_iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&"))
    }

    /// Собрать относительный маршрут гильдии `/guilds/{guild_id}/messages/search?...`.
    pub fn guild_endpoint(&self, guild_id: &str) -> Result<String, SearchRestError> {
        let trimmed = guild_id.trim();
        if parse_valid_snowflake(trimmed).is_none() {
            return Err(SearchRestError::InvalidScopeId);
        }
        let qs = self.to_query_string()?;
        Ok(format!("/guilds/{trimmed}/messages/search?{qs}"))
    }

    /// Собрать полный путь REST v9 гильдии `/api/v9/guilds/{guild_id}/messages/search?...`.
    pub fn guild_api_v9_path(&self, guild_id: &str) -> Result<String, SearchRestError> {
        let endpoint = self.guild_endpoint(guild_id)?;
        Ok(format!("/api/v9{endpoint}"))
    }

    /// Собрать относительный маршрут канала `/channels/{channel_id}/messages/search?...`.
    pub fn channel_endpoint(&self, channel_id: &str) -> Result<String, SearchRestError> {
        let trimmed = channel_id.trim();
        if parse_valid_snowflake(trimmed).is_none() {
            return Err(SearchRestError::InvalidScopeId);
        }
        let qs = self.to_query_string()?;
        Ok(format!("/channels/{trimmed}/messages/search?{qs}"))
    }

    /// Собрать полный путь REST v9 канала `/api/v9/channels/{channel_id}/messages/search?...`.
    pub fn channel_api_v9_path(&self, channel_id: &str) -> Result<String, SearchRestError> {
        let endpoint = self.channel_endpoint(channel_id)?;
        Ok(format!("/api/v9{endpoint}"))
    }

    /// Собрать полный URL Discord API v9 для поиска в гильдии (`https://discord.com/api/v9/guilds/...`).
    pub fn guild_full_url(&self, guild_id: &str) -> Result<String, SearchRestError> {
        let endpoint = self.guild_endpoint(guild_id)?;
        Ok(format!("{API_V9_BASE}{endpoint}"))
    }
}

/// Метаданные вложения внутри найденного сообщения.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchedAttachment {
    /// Snowflake ID вложения.
    pub id: String,
    /// Имя файла вложения.
    pub filename: String,
    /// Размер файла в байтах.
    pub size: u64,
    /// URL вложения на CDN.
    pub url: Option<String>,
    /// MIME-тип вложения (`content_type`).
    pub content_type: Option<String>,
}

/// Одно сообщение внутри поисковой выдачи (как само совпадение `hit: true`, так и окружение).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchedMessage {
    /// Snowflake ID сообщения.
    pub id: String,
    /// Snowflake ID канала.
    pub channel_id: String,
    /// Snowflake ID автора сообщения.
    pub author_id: Option<String>,
    /// Имя пользователя автора (`author.username`).
    pub author_username: Option<String>,
    /// Текст сообщения.
    pub content: String,
    /// ISO-8601 метка времени создания сообщения.
    pub timestamp: String,
    /// Признак целевого совпадения (`"hit": true`).
    pub hit: bool,
    /// Признак закреплённого сообщения (`"pinned": true`).
    pub pinned: bool,
    /// Список вложений сообщения.
    pub attachments: Vec<SearchedAttachment>,
    /// Число embed-блоков сообщения.
    pub embeds_count: usize,
}

/// Контекстная группа сообщений из массива верхнего уровня `messages[i]`.
///
/// Discord возвращает для каждого найденного сообщения группу из 1..=3 элементов
/// (сообщение до, целевое сообщение с `hit == true`, сообщение после).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchContextGroup {
    /// Упорядоченный список сообщений контекстной группы.
    pub messages: Vec<SearchedMessage>,
}

impl SearchContextGroup {
    /// Найти целевое сообщение (`hit == true`), либо единственное сообщение в группе из 1 элемента.
    #[must_use]
    pub fn hit_message(&self) -> Option<&SearchedMessage> {
        if let Some(hit) = self.messages.iter().find(|m| m.hit) {
            return Some(hit);
        }
        if self.messages.len() == 1 {
            return self.messages.first();
        }
        None
    }

    /// Контекстные сообщения, предшествующие целевому совпадению (`hit == true`).
    #[must_use]
    pub fn context_before(&self) -> &[SearchedMessage] {
        if let Some(idx) = self.messages.iter().position(|m| m.hit) {
            &self.messages[..idx]
        } else {
            &[]
        }
    }

    /// Контекстные сообщения, следующие за целевым совпадением (`hit == true`).
    #[must_use]
    pub fn context_after(&self) -> &[SearchedMessage] {
        if let Some(idx) = self.messages.iter().position(|m| m.hit) {
            &self.messages[idx + 1..]
        } else {
            &[]
        }
    }
}

/// Распарсенный результат вызова `GET /guilds/{id}/messages/search` (`200 OK` или `202 Accepted`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildSearchRestResult {
    /// Идентификатор аналитики поиска (`analytics_id`).
    pub analytics_id: Option<String>,
    /// Общее число найденных совпадений (`total_results`).
    pub total_results: u64,
    /// Признак глубокой исторической индексации (`doing_deep_historical_index`).
    pub doing_deep_historical_index: bool,
    /// Число проиндексированных документов (`documents_indexed`).
    pub documents_indexed: u64,
    /// Признак ответа `202 Accepted` (индекс ещё строится).
    pub indexing: bool,
    /// Рекомендованная задержка до повтора в миллисекундах (`202 Accepted`).
    pub retry_after_ms: Option<u64>,
    /// Контекстные группы сообщений (`messages: Vec<Vec<SearchedMessage>>`).
    pub groups: Vec<SearchContextGroup>,
}

impl GuildSearchRestResult {
    /// Извлечь список всех целевых совпадений (`hit_message`) по всем группам.
    #[must_use]
    pub fn hits(&self) -> Vec<&SearchedMessage> {
        self.groups
            .iter()
            .filter_map(SearchContextGroup::hit_message)
            .collect()
    }
}

/// Типизированная ошибка REST-поиска сообщений.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchRestError {
    /// Некорректный Snowflake ID гильдии или канала.
    InvalidScopeId,
    /// Запрос не содержит ни текста, ни одного фильтра.
    EmptyQuery,
    /// Поле `content` превышает `MAX_SEARCH_CONTENT_CHARS` (`512`).
    ContentTooLong {
        /// Фактическая длина в символах.
        len: usize,
        /// Максимально допустимая длина.
        max: usize,
    },
    /// Смещение `offset` превышает `MAX_SEARCH_OFFSET` (`9975`).
    OffsetOutOfRange {
        /// Запрошенное смещение.
        offset: u32,
        /// Максимально допустимое смещение (`9975`).
        max: u32,
    },
    /// Недопустимый размер страницы `limit` (разрешено `1..=25`).
    InvalidLimit {
        /// Запрошенный `limit`.
        limit: u8,
    },
    /// Некорректный Snowflake ID в одном из фильтров (`author_id`, `mentions`, `channel_id`, `min_id`, `max_id`).
    InvalidSnowflake {
        /// Имя поля фильтра.
        field: &'static str,
        /// Переданное невалидное значение.
        value: String,
    },
    /// Неизвестный фасет `has` (разрешены только 9 токенов RE).
    InvalidHasFacet(String),
    /// Неизвестное значение `author_type` (разрешены `user`, `bot`, `webhook`).
    InvalidAuthorType(String),
    /// Прочее невалидное значение фильтра.
    InvalidFilterValue {
        /// Имя поля или правила.
        field: &'static str,
    },
    /// Ошибка формирования обязательных заголовков Discord.
    Header(ApiAuthError),
    /// Недействительный токен (`HTTP 401`).
    Unauthorized,
    /// Доступ к поиску в гильдии/канале запрещён (`HTTP 403`).
    Forbidden {
        /// Числовой код ошибки Discord.
        code: Option<i64>,
        /// Сообщение об ошибке.
        message: Option<String>,
    },
    /// Гильдия или канал не найдены (`HTTP 404`).
    NotFound {
        /// Сообщение об ошибке.
        message: Option<String>,
    },
    /// Превышен лимит запросов (`HTTP 429`).
    RateLimited {
        /// Задержка до повтора в миллисекундах.
        retry_after_ms: u64,
        /// Глобальный лимит или локальный бакет.
        is_global: bool,
    },
    /// Прочая клиентская ошибка `4xx`.
    Client {
        /// HTTP-статус.
        status: u16,
        /// Опциональный код ошибки Discord.
        code: Option<i64>,
        /// Опциональное сообщение.
        message: Option<String>,
    },
    /// Ошибка сервера Discord (`5xx`).
    Server {
        /// HTTP-статус.
        status: u16,
    },
    /// Некорректный JSON-ответ сервера.
    InvalidResponse(&'static str),
}

impl fmt::Display for SearchRestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidScopeId => f.write_str("некорректный Snowflake ID гильдии или канала"),
            Self::EmptyQuery => f.write_str("поисковый запрос не содержит критериев"),
            Self::ContentTooLong { len, max } => {
                write!(f, "длина поискового запроса {len} превышает лимит {max}")
            }
            Self::OffsetOutOfRange { offset, max } => {
                write!(f, "смещение пагинации {offset} превышает потолок {max}")
            }
            Self::InvalidLimit { limit } => {
                write!(f, "недопустимый limit={limit} (ожидается 1..=25)")
            }
            Self::InvalidSnowflake { field, value } => {
                write!(f, "некорректный Snowflake в поле {field}: {value}")
            }
            Self::InvalidHasFacet(val) => write!(f, "недопустимый токен фильтра has: {val}"),
            Self::InvalidAuthorType(val) => {
                write!(f, "недопустимый токен фильтра author_type: {val}")
            }
            Self::InvalidFilterValue { field } => {
                write!(f, "некорректное значение фильтра поиска: {field}")
            }
            Self::Header(err) => write!(f, "ошибка заголовков Discord: {err:?}"),
            Self::Unauthorized => f.write_str("сессия недействительна (401)"),
            Self::Forbidden { message, .. } => match message {
                Some(msg) => write!(f, "доступ к поиску запрещён (403): {msg}"),
                None => f.write_str("доступ к поиску запрещён (403)"),
            },
            Self::NotFound { message } => match message {
                Some(msg) => write!(f, "ресурс поиска не найден (404): {msg}"),
                None => f.write_str("ресурс поиска не найден (404)"),
            },
            Self::RateLimited { retry_after_ms, .. } => {
                write!(
                    f,
                    "превышен лимит запросов поиска (429), повтор через {retry_after_ms} мс"
                )
            }
            Self::Client { status, .. } => write!(f, "клиентская ошибка поиска HTTP {status}"),
            Self::Server { status } => write!(f, "ошибка сервера поиска HTTP {status}"),
            Self::InvalidResponse(reason) => write!(f, "некорректный ответ поиска: {reason}"),
        }
    }
}

impl std::error::Error for SearchRestError {}

impl From<ApiAuthError> for SearchRestError {
    fn from(err: ApiAuthError) -> Self {
        Self::Header(err)
    }
}

/// Собрать обязательные 6 заголовков Discord для запросов поиска с `X-Discord-Locale: ru`.
pub fn build_search_headers(
    token: &str,
    super_properties: &SuperProperties,
    timezone: &str,
) -> Result<ApiAuth, ApiAuthError> {
    ApiAuth::from_token(token, super_properties, DISCORD_LOCALE_RU, timezone)
}

/// Собрать готовый [`HttpRequest`] `GET /api/v9/guilds/{guild_id}/messages/search?...`
/// с полным набором из 6 заголовков авторизации и телеметрии Discord.
pub fn build_guild_search_http_request(
    token: &str,
    super_properties: &SuperProperties,
    timezone: &str,
    guild_id: &str,
    request: &SearchQueryRequest,
) -> Result<HttpRequest, SearchRestError> {
    let path = request.guild_api_v9_path(guild_id)?;
    let auth_headers = build_search_headers(token, super_properties, timezone)?;
    let mut http_req = HttpRequest::new(HttpMethod::Get, path);
    for (name, value) in auth_headers.pairs() {
        http_req = http_req.header(name, value);
    }
    Ok(http_req)
}

/// Собрать готовый [`HttpRequest`] `GET /api/v9/channels/{channel_id}/messages/search?...`
/// с полным набором из 6 заголовков авторизации и телеметрии Discord.
pub fn build_channel_search_http_request(
    token: &str,
    super_properties: &SuperProperties,
    timezone: &str,
    channel_id: &str,
    request: &SearchQueryRequest,
) -> Result<HttpRequest, SearchRestError> {
    let path = request.channel_api_v9_path(channel_id)?;
    let auth_headers = build_search_headers(token, super_properties, timezone)?;
    let mut http_req = HttpRequest::new(HttpMethod::Get, path);
    for (name, value) in auth_headers.pairs() {
        http_req = http_req.header(name, value);
    }
    Ok(http_req)
}

/// Рассчитать задержку экспоненциального бэкоффа при повторных ответах `HTTP 429`:
/// `min(MAX_SEARCH_BACKOFF_MS, max(base_retry_after_ms, 100) * 2^attempt)`.
#[must_use]
pub fn compute_429_exponential_backoff_ms(base_retry_after_ms: u64, attempt: u32) -> u64 {
    let base = base_retry_after_ms.max(100);
    let shift = attempt.min(10);
    let factor = 1_u64 << shift;
    base.saturating_mul(factor).min(MAX_SEARCH_BACKOFF_MS)
}

/// Разобрать HTTP-ответ эндпоинта поиска сообщений:
/// - `200 OK` → [`GuildSearchRestResult`] с `indexing: false` и распарсенными `groups`;
/// - `202 Accepted` → [`GuildSearchRestResult`] с `indexing: true` и `retry_after_ms`;
/// - `429 Too Many Requests` → [`SearchRestError::RateLimited`];
/// - `401` / `403` / `404` / `4xx` / `5xx` → соответствующие варианты [`SearchRestError`].
pub fn parse_search_http_response(
    response: &HttpResponse,
) -> Result<GuildSearchRestResult, SearchRestError> {
    match response.status {
        200 => parse_200_search_body(&response.body),
        202 => Ok(parse_202_indexing_response(
            &response.headers,
            &response.body,
        )),
        status => Err(classify_search_error_response(
            status,
            &response.headers,
            &response.body,
        )),
    }
}

fn parse_200_search_body(body: &[u8]) -> Result<GuildSearchRestResult, SearchRestError> {
    let text = std::str::from_utf8(body)
        .map_err(|_| SearchRestError::InvalidResponse("ответ поиска не является валидным UTF-8"))?;
    let root = parse(text)
        .map_err(|_| SearchRestError::InvalidResponse("ответ поиска не является валидным JSON"))?;
    let obj = root.as_object().ok_or(SearchRestError::InvalidResponse(
        "ожидался JSON-объект ответа поиска",
    ))?;

    let analytics_id = obj
        .get("analytics_id")
        .and_then(JsonValue::as_str)
        .map(str::to_string);
    let total_results = obj
        .get("total_results")
        .and_then(JsonValue::as_u64)
        .unwrap_or(0);
    let doing_deep_historical_index = obj
        .get("doing_deep_historical_index")
        .and_then(JsonValue::as_bool)
        .unwrap_or(false);
    let documents_indexed = obj
        .get("documents_indexed")
        .and_then(JsonValue::as_u64)
        .unwrap_or(0);

    let raw_groups = obj.get("messages").and_then(JsonValue::as_array).ok_or(
        SearchRestError::InvalidResponse("отсутствует массив messages в ответе поиска"),
    )?;

    let mut groups = Vec::with_capacity(raw_groups.len());
    for entry in raw_groups {
        if let Some(inner_arr) = entry.as_array() {
            let mut msgs = Vec::with_capacity(inner_arr.len());
            for item in inner_arr {
                if let Some(msg) = parse_searched_message(item) {
                    msgs.push(msg);
                }
            }
            if !msgs.is_empty() {
                groups.push(SearchContextGroup { messages: msgs });
            }
        } else if let Some(mut single_msg) = parse_searched_message(entry) {
            // Совместимость с плоским списком сообщений в упрощённых стабах.
            single_msg.hit = true;
            groups.push(SearchContextGroup {
                messages: vec![single_msg],
            });
        }
    }

    Ok(GuildSearchRestResult {
        analytics_id,
        total_results,
        doing_deep_historical_index,
        documents_indexed,
        indexing: false,
        retry_after_ms: None,
        groups,
    })
}

fn parse_searched_message(value: &JsonValue) -> Option<SearchedMessage> {
    let obj = value.as_object()?;
    let id = obj.get("id").and_then(JsonValue::as_str)?.to_string();
    let channel_id = obj
        .get("channel_id")
        .and_then(JsonValue::as_str)
        .unwrap_or("")
        .to_string();
    let author_obj = obj.get("author").and_then(JsonValue::as_object);
    let author_id = author_obj
        .and_then(|a| a.get("id"))
        .and_then(JsonValue::as_str)
        .map(str::to_string);
    let author_username = author_obj
        .and_then(|a| a.get("username"))
        .and_then(JsonValue::as_str)
        .map(str::to_string);
    let content = obj
        .get("content")
        .and_then(JsonValue::as_str)
        .unwrap_or("")
        .to_string();
    let timestamp = obj
        .get("timestamp")
        .and_then(JsonValue::as_str)
        .unwrap_or("")
        .to_string();
    let hit = obj.get("hit").and_then(JsonValue::as_bool).unwrap_or(false);
    let pinned = obj
        .get("pinned")
        .and_then(JsonValue::as_bool)
        .unwrap_or(false);

    let mut attachments = Vec::new();
    if let Some(raw_atts) = obj.get("attachments").and_then(JsonValue::as_array) {
        for att in raw_atts {
            if let Some(att_obj) = att.as_object() {
                let att_id = att_obj
                    .get("id")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("")
                    .to_string();
                let filename = att_obj
                    .get("filename")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("")
                    .to_string();
                let size = att_obj.get("size").and_then(JsonValue::as_u64).unwrap_or(0);
                let url = att_obj
                    .get("url")
                    .and_then(JsonValue::as_str)
                    .map(str::to_string);
                let content_type = att_obj
                    .get("content_type")
                    .and_then(JsonValue::as_str)
                    .map(str::to_string);
                attachments.push(SearchedAttachment {
                    id: att_id,
                    filename,
                    size,
                    url,
                    content_type,
                });
            }
        }
    }

    let embeds_count = obj
        .get("embeds")
        .and_then(JsonValue::as_array)
        .map_or(0, <[JsonValue]>::len);

    Some(SearchedMessage {
        id,
        channel_id,
        author_id,
        author_username,
        content,
        timestamp,
        hit,
        pinned,
        attachments,
        embeds_count,
    })
}

fn parse_202_indexing_response(headers: &[(String, String)], body: &[u8]) -> GuildSearchRestResult {
    let parsed = std::str::from_utf8(body)
        .ok()
        .and_then(|text| parse(text).ok());
    let documents_indexed = parsed
        .as_ref()
        .and_then(|v| v.get("documents_indexed"))
        .and_then(JsonValue::as_u64)
        .unwrap_or(0);
    let doing_deep_historical_index = parsed
        .as_ref()
        .and_then(|v| v.get("doing_deep_historical_index"))
        .and_then(JsonValue::as_bool)
        .unwrap_or(false);

    let retry_after_ms = extract_202_retry_after_ms(headers, parsed.as_ref());

    GuildSearchRestResult {
        analytics_id: None,
        total_results: 0,
        doing_deep_historical_index,
        documents_indexed,
        indexing: true,
        retry_after_ms: Some(retry_after_ms),
        groups: Vec::new(),
    }
}

fn classify_search_error_response(
    status: u16,
    headers: &[(String, String)],
    body: &[u8],
) -> SearchRestError {
    if status == 401 {
        return SearchRestError::Unauthorized;
    }
    if (500..=599).contains(&status) {
        return SearchRestError::Server { status };
    }

    let parsed = std::str::from_utf8(body)
        .ok()
        .and_then(|text| parse(text).ok());
    let code = parsed
        .as_ref()
        .and_then(|v| v.get("code"))
        .and_then(JsonValue::as_i64);
    let message = parsed
        .as_ref()
        .and_then(|v| v.get("message"))
        .and_then(JsonValue::as_str)
        .map(str::to_string);

    if status == 429 {
        let retry_after_ms = extract_429_retry_after_ms(headers, parsed.as_ref());
        let is_global = parsed
            .as_ref()
            .and_then(|v| v.get("global"))
            .and_then(JsonValue::as_bool)
            .unwrap_or(false);
        return SearchRestError::RateLimited {
            retry_after_ms,
            is_global,
        };
    }

    if status == 403 {
        return SearchRestError::Forbidden { code, message };
    }
    if status == 404 {
        return SearchRestError::NotFound { message };
    }

    SearchRestError::Client {
        status,
        code,
        message,
    }
}

fn extract_202_retry_after_ms(
    headers: &[(String, String)],
    parsed_body: Option<&JsonValue>,
) -> u64 {
    for (name, value) in headers {
        if name.eq_ignore_ascii_case("retry-after")
            && let Ok(secs) = value.trim().parse::<u64>()
            && secs > 0
        {
            return secs.saturating_mul(1000);
        }
    }
    if let Some(secs) = parsed_body
        .and_then(|v| v.get("retry_after"))
        .and_then(JsonValue::as_f64)
        && secs.is_finite()
        && secs > 0.0
    {
        return (secs * 1000.0).round() as u64;
    }
    DEFAULT_SEARCH_202_RETRY_AFTER_MS
}

fn extract_429_retry_after_ms(
    headers: &[(String, String)],
    parsed_body: Option<&JsonValue>,
) -> u64 {
    if let Some(secs) = parsed_body
        .and_then(|v| v.get("retry_after"))
        .and_then(JsonValue::as_f64)
        && secs.is_finite()
        && secs > 0.0
    {
        return (secs * 1000.0).round() as u64;
    }
    for (name, value) in headers {
        if name.eq_ignore_ascii_case("retry-after")
            && let Ok(secs) = value.trim().parse::<f64>()
            && secs.is_finite()
            && secs > 0.0
        {
            return (secs * 1000.0).round() as u64;
        }
    }
    DEFAULT_SEARCH_429_RETRY_AFTER_MS
}

fn collapse_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parse_valid_snowflake(raw: &str) -> Option<u64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 20 || !trimmed.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    trimmed.parse::<u64>().ok().filter(|&v| v > 0)
}

fn normalize_snowflake_vec(
    items: &[String],
    field: &'static str,
) -> Result<Vec<String>, SearchRestError> {
    let mut out = Vec::with_capacity(items.len());
    for raw in items {
        let trimmed = raw.trim();
        if parse_valid_snowflake(trimmed).is_none() {
            return Err(SearchRestError::InvalidSnowflake {
                field,
                value: raw.clone(),
            });
        }
        let s = trimmed.to_string();
        if !out.contains(&s) {
            out.push(s);
        }
    }
    Ok(out)
}

fn normalize_lowercase_vec(
    items: &[String],
    field: &'static str,
    strip_leading_dot: bool,
) -> Result<Vec<String>, SearchRestError> {
    let mut out = Vec::with_capacity(items.len());
    for raw in items {
        let trimmed = raw.trim();
        let cleaned = if strip_leading_dot {
            trimmed.trim_start_matches('.')
        } else {
            trimmed
        };
        if cleaned.is_empty() || cleaned.chars().any(char::is_whitespace) {
            return Err(SearchRestError::InvalidFilterValue { field });
        }
        let lowered = cleaned.to_ascii_lowercase();
        if !out.contains(&lowered) {
            out.push(lowered);
        }
    }
    Ok(out)
}

fn normalize_trimmed_vec(
    items: &[String],
    field: &'static str,
) -> Result<Vec<String>, SearchRestError> {
    let mut out = Vec::with_capacity(items.len());
    for raw in items {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(SearchRestError::InvalidFilterValue { field });
        }
        let s = trimmed.to_string();
        if !out.contains(&s) {
            out.push(s);
        }
    }
    Ok(out)
}

fn url_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_inline_tokens_and_builds_canonical_guild_endpoint() {
        let req = SearchQueryRequest::parse_and_normalize(
            "from:123456789012345678 has:Link in:987654321098765432 ext:.PDF привет мир",
        )
        .expect("valid inline query");
        assert_eq!(req.content.as_deref(), Some("привет мир"));
        assert_eq!(req.author_id, vec!["123456789012345678"]);
        assert_eq!(req.has, vec!["link"]);
        assert_eq!(req.channel_id, vec!["987654321098765432"]);
        assert_eq!(req.attachment_extension, vec!["pdf"]);

        let ep = req
            .guild_endpoint("1014537851418980372")
            .expect("guild endpoint");
        assert_eq!(
            ep,
            "/guilds/1014537851418980372/messages/search?content=%D0%BF%D1%80%D0%B8%D0%B2%D0%B5%D1%82%20%D0%BC%D0%B8%D1%80&channel_id=987654321098765432&author_id=123456789012345678&has=link&attachment_extension=pdf"
        );
    }

    #[test]
    fn computes_exponential_backoff_capped_at_30s() {
        assert_eq!(compute_429_exponential_backoff_ms(500, 0), 500);
        assert_eq!(compute_429_exponential_backoff_ms(500, 1), 1000);
        assert_eq!(compute_429_exponential_backoff_ms(500, 2), 2000);
        assert_eq!(compute_429_exponential_backoff_ms(5000, 4), 30_000);
    }
}
