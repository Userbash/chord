//! Серверный поиск сообщений в гильдии через подтверждённый REST-маршрут.

use std::collections::VecDeque;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use discord_core::rest::client::{ApiAuth, ApiClient, ApiConfig, ApiRetryPolicy};
use discord_core::rest::identity::SystemSources;
use discord_core::rest::routes::{self, HttpMethod};
use discord_core::rest::search::compute_429_exponential_backoff_ms;
use discord_core::rest::transport::{HttpClientConfig, HttpResponse};
use discord_core::API_V9_BASE;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::rest_failure::RestFailure;

/// Максимальный размер страницы поиска (RE-DEEP-REA §2.4: `iG = 25`, константа `T_y`).
pub const MAX_SEARCH_LIMIT: i64 = 25;

/// Максимальное смещение пагинации поиска (M-13: `MAX_SEARCH_OFFSET = 9975`, 400 страниц по 25).
pub const MAX_SEARCH_OFFSET: i64 = 9975;

/// Максимальное число страниц поиска (`MAX_SEARCH_PAGES = 400`, `E+2azY`).
pub const MAX_SEARCH_PAGES: i64 = 400;

/// Максимальное число повторов при HTTP 202 (`SEARCH-202-NSFW-RE.md` §5.2: 5 повторов, всего ≤ 6 запросов).
pub const MAX_SEARCH_202_RETRIES: u32 = 5;

/// Максимальное число повторов индексации (алиас `MAX_SEARCH_202_RETRIES`).
pub const MAX_INDEXING_RETRIES: u32 = MAX_SEARCH_202_RETRIES;

/// Максимальное число повторов при `HTTP 429 Too Many Requests` с экспоненциальным бэкоффом (TASK-617).
pub const MAX_SEARCH_429_RETRIES: u32 = 3;

/// Ёмкость BFF LRU-кэша недавних поисковых запросов (TASK-617).
pub const MAX_SEARCH_CACHE_ENTRIES: usize = 64;

/// Задержка повтора при HTTP 202 по умолчанию (`SEARCH-202-NSFW-RE.md` §5.2: 5000 мс).
pub const DEFAULT_SEARCH_202_RETRY_AFTER_MS: u64 = 5000;

/// Генерирует новый UUID v4 (RFC 4122).
#[must_use]
pub fn generate_uuid_v4() -> String {
    discord_core::rest::identity::uuid_v4(&SystemSources)
}

/// Детерминированный генератор UUID v4 для воспроизводимых тестов.
#[must_use]
pub fn deterministic_uuid_v4(seed: u64) -> String {
    struct SeedSources(u64);
    impl discord_core::rest::identity::IdentitySources for SeedSources {
        fn env_var(&self, _: &str) -> Option<String> {
            None
        }
        fn read_text(&self, _: &str) -> Option<String> {
            None
        }
        fn random_bytes(&self, out: &mut [u8]) {
            let mut state = self.0;
            for byte in out.iter_mut() {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                *byte = (state >> 33) as u8;
            }
        }
        fn os(&self) -> &'static str {
            "linux"
        }
        fn arch(&self) -> &'static str {
            "x86_64"
        }
    }
    discord_core::rest::identity::uuid_v4(&SeedSources(seed))
}

/// Сессия поиска сообщений: держит `session_id`, обновляет `query_id` на новый запрос
/// и переиспользует оба UUID при пагинации (`SEARCH-202-NSFW-RE.md` §2.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchSession {
    #[serde(alias = "search_session_id")]
    pub session_id: String,
    #[serde(alias = "search_query_id")]
    pub query_id: String,
}

impl SearchSession {
    #[must_use]
    pub fn new() -> Self {
        Self {
            session_id: generate_uuid_v4(),
            query_id: generate_uuid_v4(),
        }
    }

    #[must_use]
    pub fn with_ids(session_id: impl Into<String>, query_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            query_id: query_id.into(),
        }
    }

    /// Применяет `session_id` и `query_id` к параметрам поиска, если они ещё не заданы.
    pub fn apply_to(&self, params: &mut SearchParams) {
        if params.search_session_id.is_none() {
            params.search_session_id = Some(self.session_id.clone());
        }
        if params.search_query_id.is_none() {
            params.search_query_id = Some(self.query_id.clone());
        }
    }

    /// Обновляет `query_id` для нового запроса в рамках текущей сессии (`effc40ce4f2aa27c_0@5395: refreshQueryId`).
    pub fn refresh_query_id(&mut self) -> String {
        let new_id = generate_uuid_v4();
        self.query_id = new_id.clone();
        new_id
    }
}

impl Default for SearchSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Подтверждённые значения фильтра `has` (RE-DEEP-REA §2.3).
pub const SEARCH_HAS_VALUES: [&str; 9] = [
    "link", "embed", "poll", "snapshot", "file", "video", "image", "sound", "sticker",
];

/// Подтверждённые значения фильтра `author_type` (RE-DEEP-REA §2.3).
pub const SEARCH_AUTHOR_TYPES: [&str; 3] = ["user", "bot", "webhook"];

/// Параметры GET `/guilds/{id}/messages/search` (RE-DEEP-REA §2.3–2.4).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchParams {
    pub content: Option<String>,
    pub channel_id: Option<String>,
    pub author_id: Option<String>,
    pub mentions: Option<String>,
    pub has: Option<String>,
    pub pinned: Option<bool>,
    pub author_type: Option<String>,
    pub min_id: Option<String>,
    pub max_id: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    /// Допустимый диапазон `1..=25` (RE-DEEP-REA §2.4); прочие значения отвергаются до сети.
    pub limit: Option<i64>,
    /// Допустимый диапазон `0..=9975` (`MAX_SEARCH_OFFSET`); прочие значения отвергаются до сети.
    pub offset: Option<i64>,
    /// Включать ли NSFW-результаты. Query-строка собирается вручную в `execute()` и добавляет
    /// `include_nsfw=true` только при `Some(true)` (`None` и `Some(false)` опускаются из URL).
    pub include_nsfw: Option<bool>,
    /// Номер повторной попытки при поллинге HTTP 202 (`1..=5`, `SEARCH-202-NSFW-RE.md` §5.4).
    /// При `None` или `Some(0)` параметр в URL не добавляется; при `> 5` возвращается `InvalidArg`.
    pub attempts: Option<u32>,
    /// Идентификатор сессии поиска (UUID v4 на контекст/окно поиска, `capture-run2.jsonl`, `SEARCH-202-NSFW-RE.md` §2.5).
    pub search_session_id: Option<String>,
    /// Идентификатор поискового запроса (UUID v4 на запрос, переиспользуется при пагинации, `SEARCH-202-NSFW-RE.md` §2.5).
    pub search_query_id: Option<String>,
    /// Фильтр по домену ссылок (`link_hostname`, TASK-617).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_hostname: Option<Vec<String>>,
    /// Фильтр по расширению вложений (`attachment_extension`, TASK-617).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachment_extension: Option<Vec<String>>,
    /// Фильтр по имени файла вложения (`attachment_filename`, TASK-617).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachment_filename: Option<Vec<String>>,
}

impl SearchParams {
    /// Разобрать сырую поисковую строку с инлайн-токенами (`from:`, `mentions:`, `has:`, `in:`,
    /// `before:`, `after:`, `pinned:`, `author_type:`, `link_hostname:`, `attachment_extension:`,
    /// `attachment_filename:`) и вернуть нормализованные [`SearchParams`] (TASK-617).
    #[must_use]
    pub fn from_raw_query(raw_query: &str) -> Self {
        let mut params = Self {
            content: Some(raw_query.to_string()),
            ..Self::default()
        };
        params.normalize_in_place();
        params
    }

    /// Вернуть копию [`SearchParams`] с извлечёнными из `content` инлайн-фасетами
    /// и нормализованными списками фильтров.
    #[must_use]
    pub fn normalized(&self) -> Self {
        let mut copy = self.clone();
        copy.normalize_in_place();
        copy
    }

    /// Извлечь инлайн-токены из `content` и нормализовать поля на месте.
    pub fn normalize_in_place(&mut self) {
        let mut author_ids = split_csv_owned(self.author_id.as_deref());
        let mut mentions = split_csv_owned(self.mentions.as_deref());
        let mut has_list = split_csv_owned(self.has.as_deref());
        let mut channel_ids = split_csv_owned(self.channel_id.as_deref());
        let mut link_hosts = self.link_hostname.take().unwrap_or_default();
        let mut att_exts = self.attachment_extension.take().unwrap_or_default();
        let mut att_names = self.attachment_filename.take().unwrap_or_default();

        if let Some(raw_content) = self.content.take() {
            let mut free_words = Vec::new();
            for token in raw_content.split_whitespace() {
                if let Some((raw_key, raw_val)) = token.split_once(':') {
                    let key = raw_key.to_ascii_lowercase();
                    let val = raw_val.trim();
                    if val.is_empty() {
                        free_words.push(token.to_string());
                        continue;
                    }
                    match key.as_str() {
                        "from" | "author_id" => push_unique(&mut author_ids, val.to_string()),
                        "mentions" => push_unique(&mut mentions, val.to_string()),
                        "has" => push_unique(&mut has_list, val.to_ascii_lowercase()),
                        "in" | "channel_id" => push_unique(&mut channel_ids, val.to_string()),
                        "before" | "max_id" => self.max_id = Some(val.to_string()),
                        "after" | "min_id" => self.min_id = Some(val.to_string()),
                        "pinned" => match val.to_ascii_lowercase().as_str() {
                            "true" => self.pinned = Some(true),
                            "false" => self.pinned = Some(false),
                            _ => free_words.push(token.to_string()),
                        },
                        "author_type" => self.author_type = Some(val.to_ascii_lowercase()),
                        "link_hostname" | "site" => {
                            push_unique(&mut link_hosts, val.to_ascii_lowercase());
                        }
                        "attachment_extension" | "ext" => {
                            let cleaned = val.trim_start_matches('.').to_ascii_lowercase();
                            if !cleaned.is_empty() {
                                push_unique(&mut att_exts, cleaned);
                            }
                        }
                        "attachment_filename" | "filename" => {
                            push_unique(&mut att_names, val.to_string());
                        }
                        _ => free_words.push(token.to_string()),
                    }
                } else {
                    free_words.push(token.to_string());
                }
            }
            let joined = free_words.join(" ");
            if !joined.is_empty() {
                self.content = Some(joined);
            }
        }

        if !author_ids.is_empty() {
            self.author_id = Some(author_ids.join(","));
        }
        if !mentions.is_empty() {
            self.mentions = Some(mentions.join(","));
        }
        if !has_list.is_empty() {
            self.has = Some(has_list.join(","));
        }
        if !channel_ids.is_empty() {
            self.channel_id = Some(channel_ids.join(","));
        }
        if !link_hosts.is_empty() {
            for h in &mut link_hosts {
                *h = h.trim().to_ascii_lowercase();
            }
            self.link_hostname = Some(link_hosts);
        }
        if !att_exts.is_empty() {
            for ext in &mut att_exts {
                *ext = ext.trim().trim_start_matches('.').to_ascii_lowercase();
            }
            self.attachment_extension = Some(att_exts);
        }
        if !att_names.is_empty() {
            for name in &mut att_names {
                *name = name.trim().to_string();
            }
            self.attachment_filename = Some(att_names);
        }
    }

    /// Построить канонический ключ кэша (без транзиентных `search_session_id`, `search_query_id`, `attempts`).
    #[must_use]
    pub fn canonical_cache_key(&self) -> String {
        let mut norm = self.normalized();
        norm.search_session_id = None;
        norm.search_query_id = None;
        norm.attempts = None;
        serde_json::to_string(&norm).unwrap_or_default()
    }
    /// Генерирует `search_session_id` и `search_query_id` (UUID v4), если они отсутствуют.
    pub fn ensure_session_ids(&mut self) {
        if self.search_session_id.is_none() {
            self.search_session_id = Some(generate_uuid_v4());
        }
        if self.search_query_id.is_none() {
            self.search_query_id = Some(generate_uuid_v4());
        }
    }

    /// Создаёт копию параметров со сгенерированными UUID v4, если они отсутствовали.
    #[must_use]
    pub fn with_generated_ids(mut self) -> Self {
        self.ensure_session_ids();
        self
    }

    /// Устанавливает идентификаторы сессии и запроса.
    #[must_use]
    pub fn with_session_ids(
        mut self,
        session_id: impl Into<String>,
        query_id: impl Into<String>,
    ) -> Self {
        self.search_session_id = Some(session_id.into());
        self.search_query_id = Some(query_id.into());
        self
    }

    /// Создаёт параметры для смещения пагинации с сохранением `search_session_id` и `search_query_id`.
    #[must_use]
    pub fn with_offset(&self, offset: i64) -> Self {
        let mut next = self.clone();
        next.offset = Some(offset);
        next
    }

    /// Переходит на следующую страницу пагинации (шаг `step`, по умолчанию 25).
    #[must_use]
    pub fn next_page(&self, step: i64) -> Self {
        let current = self.offset.unwrap_or(0);
        let step = if step > 0 { step } else { MAX_SEARCH_LIMIT };
        self.with_offset(current.saturating_add(step))
    }

    /// Обновляет запрос в рамках той же сессии (сохраняет `search_session_id`, сбрасывает `offset` и генерирует новый `search_query_id`).
    #[must_use]
    pub fn new_query_in_session(&self, new_content: Option<String>) -> Self {
        let mut next = self.clone();
        next.content = new_content;
        next.offset = Some(0);
        next.attempts = None;
        if next.search_session_id.is_none() {
            next.search_session_id = Some(generate_uuid_v4());
        }
        next.search_query_id = Some(generate_uuid_v4());
        next
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StringOrVec {
    One(String),
    Many(Vec<String>),
}

impl StringOrVec {
    fn into_joined(self) -> Option<String> {
        match self {
            Self::One(value) => Some(value),
            Self::Many(values) => {
                if values.is_empty() {
                    None
                } else {
                    Some(values.join(","))
                }
            }
        }
    }

    fn into_vec(self) -> Option<Vec<String>> {
        match self {
            Self::One(value) => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(vec![trimmed.to_string()])
                }
            }
            Self::Many(values) => {
                let cleaned: Vec<String> = values
                    .into_iter()
                    .map(|v| v.trim().to_string())
                    .filter(|v| !v.is_empty())
                    .collect();
                if cleaned.is_empty() {
                    None
                } else {
                    Some(cleaned)
                }
            }
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSearchFields {
    content: Option<String>,
    #[serde(alias = "channel_id")]
    channel_id: Option<StringOrVec>,
    #[serde(alias = "author_id")]
    author_id: Option<StringOrVec>,
    mentions: Option<StringOrVec>,
    has: Option<StringOrVec>,
    pinned: Option<bool>,
    #[serde(alias = "author_type")]
    author_type: Option<String>,
    #[serde(alias = "min_id")]
    min_id: Option<String>,
    #[serde(alias = "max_id")]
    max_id: Option<String>,
    #[serde(alias = "sort_by")]
    sort_by: Option<String>,
    #[serde(alias = "sort_order")]
    sort_order: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
    #[serde(alias = "include_nsfw")]
    include_nsfw: Option<bool>,
    attempts: Option<u32>,
    #[serde(alias = "search_session_id")]
    search_session_id: Option<String>,
    #[serde(alias = "search_query_id")]
    search_query_id: Option<String>,
    #[serde(alias = "link_hostname")]
    link_hostname: Option<StringOrVec>,
    #[serde(alias = "attachment_extension")]
    attachment_extension: Option<StringOrVec>,
    #[serde(alias = "attachment_filename")]
    attachment_filename: Option<StringOrVec>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSearchParams {
    #[serde(flatten)]
    top: RawSearchFields,
    query: Option<RawSearchFields>,
}

impl<'de> Deserialize<'de> for SearchParams {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawSearchParams::deserialize(deserializer)?;
        let nested = raw.query.unwrap_or_default();
        Ok(Self {
            content: raw.top.content.or(nested.content),
            channel_id: raw
                .top
                .channel_id
                .and_then(StringOrVec::into_joined)
                .or_else(|| nested.channel_id.and_then(StringOrVec::into_joined)),
            author_id: raw
                .top
                .author_id
                .and_then(StringOrVec::into_joined)
                .or_else(|| nested.author_id.and_then(StringOrVec::into_joined)),
            mentions: raw
                .top
                .mentions
                .and_then(StringOrVec::into_joined)
                .or_else(|| nested.mentions.and_then(StringOrVec::into_joined)),
            has: raw
                .top
                .has
                .and_then(StringOrVec::into_joined)
                .or_else(|| nested.has.and_then(StringOrVec::into_joined)),
            pinned: raw.top.pinned.or(nested.pinned),
            author_type: raw.top.author_type.or(nested.author_type),
            min_id: raw.top.min_id.or(nested.min_id),
            max_id: raw.top.max_id.or(nested.max_id),
            sort_by: raw.top.sort_by.or(nested.sort_by),
            sort_order: raw.top.sort_order.or(nested.sort_order),
            limit: raw.top.limit.or(nested.limit),
            offset: raw.top.offset.or(nested.offset),
            include_nsfw: raw.top.include_nsfw.or(nested.include_nsfw),
            attempts: raw.top.attempts.or(nested.attempts),
            search_session_id: raw.top.search_session_id.or(nested.search_session_id),
            search_query_id: raw.top.search_query_id.or(nested.search_query_id),
            link_hostname: raw
                .top
                .link_hostname
                .and_then(StringOrVec::into_vec)
                .or_else(|| nested.link_hostname.and_then(StringOrVec::into_vec)),
            attachment_extension: raw
                .top
                .attachment_extension
                .and_then(StringOrVec::into_vec)
                .or_else(|| nested.attachment_extension.and_then(StringOrVec::into_vec)),
            attachment_filename: raw
                .top
                .attachment_filename
                .and_then(StringOrVec::into_vec)
                .or_else(|| nested.attachment_filename.and_then(StringOrVec::into_vec)),
        })
    }
}

/// Структурированное сообщение внутри контекстной группы поисковой выдачи (TASK-617).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHitMessage {
    pub id: String,
    pub channel_id: String,
    pub author_id: Option<String>,
    pub author_username: Option<String>,
    pub content: String,
    pub timestamp: String,
    pub hit: bool,
    pub pinned: bool,
    pub attachments_count: usize,
    pub embeds_count: usize,
}

/// Контекстная группа сообщений из вложенного массива `messages[i]` ответа `200 OK` (TASK-617).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHitGroup {
    pub messages: Vec<SearchHitMessage>,
}

impl SearchHitGroup {
    /// Возвращает целевое сообщение с `hit == true` (или единственное сообщение в группе).
    #[must_use]
    pub fn primary_hit(&self) -> Option<&SearchHitMessage> {
        if let Some(hit) = self.messages.iter().find(|m| m.hit) {
            return Some(hit);
        }
        if self.messages.len() == 1 {
            return self.messages.first();
        }
        None
    }
}

/// Подтверждённый ответ серверного поиска (`SEARCH-202-NSFW-SPEC.md` §3.1).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub messages: Vec<Value>,
    #[serde(rename = "total_results")]
    pub total_results: u64,
    #[serde(rename = "doing_deep_historical_index")]
    pub doing_deep_historical_index: bool,
    /// Количество проиндексированных документов (`SEARCH-202-NSFW-RE.md` §4.1, §6: опционально в 200 OK, по умолчанию 0).
    #[serde(default, rename = "documents_indexed")]
    pub documents_indexed: u64,
    /// Адаптация Tauri IPC конверта: `true`, когда HTTP-ответ вернул статус `202 Accepted`.
    #[serde(default)]
    pub indexing: bool,
    /// Адаптация Tauri IPC конверта: задержка в мс из заголовка `Retry-After` при статусе `202`.
    #[serde(
        default,
        rename = "retry_after_ms",
        skip_serializing_if = "Option::is_none"
    )]
    pub retry_after_ms: Option<u64>,
}

impl SearchResponse {
    /// Разобрать вложенный массив `messages` (`Vec<Vec<Message>>` или плоский `Vec<Message>`)
    /// в типизированные контекстные группы [`SearchHitGroup`] (TASK-617).
    #[must_use]
    pub fn hit_groups(&self) -> Vec<SearchHitGroup> {
        let mut groups = Vec::with_capacity(self.messages.len());
        for entry in &self.messages {
            if let Some(arr) = entry.as_array() {
                let mut msgs = Vec::with_capacity(arr.len());
                for item in arr {
                    if let Some(m) = parse_hit_message_value(item) {
                        msgs.push(m);
                    }
                }
                if !msgs.is_empty() {
                    groups.push(SearchHitGroup { messages: msgs });
                }
            } else if let Some(mut single) = parse_hit_message_value(entry) {
                single.hit = true;
                groups.push(SearchHitGroup {
                    messages: vec![single],
                });
            }
        }
        groups
    }

    /// Извлечь список главных совпадений (`hit == true`) из всех контекстных групп.
    #[must_use]
    pub fn primary_hits(&self) -> Vec<SearchHitMessage> {
        self.hit_groups()
            .into_iter()
            .filter_map(|g| g.primary_hit().cloned())
            .collect()
    }
}

/// Ошибки поиска без утечки токена или тела запроса.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SearchError {
    InvalidArg,
    MissingToken,
    Unauthorized,
    /// 403: нет доступа к истории гильдии.
    Forbidden,
    /// 404: гильдия не найдена.
    NotFound,
    /// 202: глубокая индексация поиска не завершилась за `MAX_SEARCH_202_RETRIES` повторов.
    Indexing {
        retry_after_ms: u64,
        attempts: u32,
    },
    RateLimited {
        retry_after_ms: u64,
    },
    /// Прочие 4xx (кроме 401/403/404/429).
    Client {
        status: u16,
    },
    /// 5xx: ошибка сервера Discord.
    Server {
        status: u16,
    },
    Network,
    /// SEC-02: keyring/blocking недоступны — это не сеть; причина не утекает.
    Storage,
    InvalidResponse,
}

impl fmt::Display for SearchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArg => formatter.write_str("некорректные параметры поиска"),
            Self::MissingToken => formatter.write_str("нет сохранённого токена"),
            Self::Unauthorized => formatter.write_str("сессия истекла"),
            Self::Forbidden => formatter.write_str("нет доступа к серверу"),
            Self::NotFound => formatter.write_str("сервер не найден"),
            Self::Indexing {
                retry_after_ms,
                attempts,
            } => write!(
                formatter,
                "индексация поиска не завершена после {attempts} попыток; повтор через {retry_after_ms} мс"
            ),
            Self::RateLimited { retry_after_ms } => {
                write!(
                    formatter,
                    "ограничение частоты; повтор через {retry_after_ms} мс"
                )
            }
            Self::Client { status } => write!(formatter, "запрос отклонён (HTTP {status})"),
            Self::Server { status } => write!(formatter, "ошибка сервера Discord (HTTP {status})"),
            Self::Network => formatter.write_str("сеть недоступна"),
            Self::Storage => formatter.write_str("хранилище токена недоступно"),
            Self::InvalidResponse => formatter.write_str("некорректный ответ поиска"),
        }
    }
}

impl std::error::Error for SearchError {}

impl From<crate::rest_failure::RestFailure> for SearchError {
    fn from(failure: crate::rest_failure::RestFailure) -> Self {
        use crate::rest_failure::RestFailure;
        match failure {
            RestFailure::Unauthorized => Self::Unauthorized,
            RestFailure::Forbidden => Self::Forbidden,
            RestFailure::NotFound => Self::NotFound,
            RestFailure::RateLimited { retry_after_ms } => Self::RateLimited { retry_after_ms },
            RestFailure::ClientOther { status } => Self::Client { status },
            RestFailure::Server { status } => Self::Server { status },
            RestFailure::Network => Self::Network,
            RestFailure::InvalidResponse => Self::InvalidResponse,
        }
    }
}

/// Асинхронный контракт удалённого поиска, удобный для fake-реализаций.
pub trait RemoteSearch: Send + Sync {
    fn search<'a>(
        &'a self,
        guild_id: &'a str,
        params: &'a SearchParams,
    ) -> Pin<Box<dyn Future<Output = Result<SearchResponse, SearchError>> + Send + 'a>>;
}

pub trait SearchTransport: Send + Sync {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, SearchError>> + Send + 'a>>;

    fn sleep<'a>(&'a self, delay: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(tokio::time::sleep(delay))
    }
}

struct ApiSearchTransport(ApiClient);

impl SearchTransport for ApiSearchTransport {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, SearchError>> + Send + 'a>> {
        Box::pin(async move {
            self.0
                .get(target)
                .await
                .map_err(|error| SearchError::from(RestFailure::from_api_error(&error)))
        })
    }
}

/// Реализация поиска через `discord-core` REST-клиент с LRU-кэшем недавних ответов.
pub struct DiscordSearch {
    transport: Arc<dyn SearchTransport>,
    cache: Mutex<VecDeque<(String, String, SearchResponse)>>,
}

impl fmt::Debug for DiscordSearch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DiscordSearch")
            .finish_non_exhaustive()
    }
}

impl DiscordSearch {
    pub fn new(token: &str) -> Result<Self, SearchError> {
        Self::with_base(token, API_V9_BASE)
    }

    pub fn with_base(token: &str, base: &str) -> Result<Self, SearchError> {
        if token.trim().is_empty() {
            return Err(SearchError::MissingToken);
        }
        let identity_ctx = crate::auth::IdentityContext::detect();
        let auth = ApiAuth::from_token(
            token,
            &identity_ctx.super_properties,
            &identity_ctx.locale,
            &identity_ctx.timezone,
        )
        .map_err(|_| SearchError::InvalidArg)?;
        let client = ApiClient::new(ApiConfig {
            base: base.to_string(),
            auth,
            http: HttpClientConfig::default(),
            retry: ApiRetryPolicy {
                max_attempts: 1,
                ..ApiRetryPolicy::default()
            },
        })
        .map_err(|_| SearchError::Network)?;
        Ok(Self {
            transport: Arc::new(ApiSearchTransport(client)),
            cache: Mutex::new(VecDeque::new()),
        })
    }

    #[doc(hidden)]
    pub fn with_transport(transport: Arc<dyn SearchTransport>) -> Self {
        Self {
            transport,
            cache: Mutex::new(VecDeque::new()),
        }
    }

    /// Текущее число записей в BFF-кэше недавних поисковых результатов.
    #[must_use]
    pub fn cache_len(&self) -> usize {
        self.cache.lock().map_or(0, |guard| guard.len())
    }

    /// Очистить кэш результатов поиска для заданной гильдии.
    pub fn invalidate_guild_cache(&self, guild_id: &str) {
        let trimmed = guild_id.trim();
        if let Ok(mut guard) = self.cache.lock() {
            guard.retain(|(gid, _, _)| gid != trimmed);
        }
    }

    /// Полностью очистить BFF-кэш результатов поиска.
    pub fn clear_cache(&self) {
        if let Ok(mut guard) = self.cache.lock() {
            guard.clear();
        }
    }

    /// Выполнить поиск с использованием LRU-кэша для подтверждённых ответов `200 OK`
    /// (`!indexing && !doing_deep_historical_index`). Ответы `202 Accepted` и ошибки не кэшируются.
    pub async fn search_cached(
        &self,
        guild_id: &str,
        params: &SearchParams,
    ) -> Result<SearchResponse, SearchError> {
        let normalized = params.normalized();
        let gid = guild_id.trim().to_string();
        let key = normalized.canonical_cache_key();

        if let Ok(mut guard) = self.cache.lock() {
            if let Some(pos) = guard
                .iter()
                .position(|(cached_gid, cached_key, _)| cached_gid == &gid && cached_key == &key)
            {
                if let Some(entry) = guard.remove(pos) {
                    let response = entry.2.clone();
                    guard.push_back(entry);
                    return Ok(response);
                }
            }
        }

        let response = self.execute(guild_id, &normalized).await?;
        if !response.indexing && !response.doing_deep_historical_index {
            if let Ok(mut guard) = self.cache.lock() {
                if let Some(pos) = guard.iter().position(|(cached_gid, cached_key, _)| {
                    cached_gid == &gid && cached_key == &key
                }) {
                    guard.remove(pos);
                } else if guard.len() >= MAX_SEARCH_CACHE_ENTRIES {
                    guard.pop_front();
                }
                guard.push_back((gid, key, response.clone()));
            }
        }
        Ok(response)
    }

    /// Выполняет поиск с автоматическим retry-циклом:
    /// - при `HTTP 202 Accepted`: до `MAX_SEARCH_202_RETRIES = 5` повторов с пробросом `attempts = Some(1..=5)`;
    /// - при `HTTP 429 Too Many Requests`: до `MAX_SEARCH_429_RETRIES = 3` повторов с экспоненциальным бэкоффом.
    pub async fn execute_with_retry<P: std::borrow::Borrow<SearchParams>>(
        &self,
        guild_id: &str,
        params: P,
    ) -> Result<SearchResponse, SearchError> {
        let mut attempt_params = params.borrow().clone();
        let mut retries_202 = 0u32;
        let mut retries_429 = 0u32;
        loop {
            match self.execute(guild_id, &attempt_params).await {
                Ok(response) => {
                    if response.indexing {
                        let retry_after_ms = response
                            .retry_after_ms
                            .unwrap_or(DEFAULT_SEARCH_202_RETRY_AFTER_MS);
                        if retries_202 >= MAX_SEARCH_202_RETRIES {
                            return Err(SearchError::Indexing {
                                retry_after_ms,
                                attempts: retries_202,
                            });
                        }
                        retries_202 += 1;
                        attempt_params.attempts = Some(retries_202);
                        self.transport
                            .sleep(Duration::from_millis(retry_after_ms))
                            .await;
                        continue;
                    }
                    return Ok(response);
                }
                Err(SearchError::RateLimited { retry_after_ms }) => {
                    if retries_429 >= MAX_SEARCH_429_RETRIES {
                        return Err(SearchError::RateLimited { retry_after_ms });
                    }
                    let base_ms = if retry_after_ms > 0 {
                        retry_after_ms
                    } else {
                        1000
                    };
                    let backoff_ms = compute_429_exponential_backoff_ms(base_ms, retries_429);
                    retries_429 += 1;
                    self.transport
                        .sleep(Duration::from_millis(backoff_ms))
                        .await;
                }
                Err(other_err) => return Err(other_err),
            }
        }
    }

    pub async fn execute(
        &self,
        guild_id: &str,
        params: &SearchParams,
    ) -> Result<SearchResponse, SearchError> {
        if guild_id.trim().is_empty() || guild_id.contains(['{', '}', '\r', '\n']) {
            return Err(SearchError::InvalidArg);
        }
        if params
            .offset
            .is_some_and(|offset| !(0..=MAX_SEARCH_OFFSET).contains(&offset))
        {
            return Err(SearchError::InvalidArg);
        }
        if params
            .limit
            .is_some_and(|limit| !(1..=MAX_SEARCH_LIMIT).contains(&limit))
        {
            return Err(SearchError::InvalidArg);
        }
        if params
            .attempts
            .is_some_and(|attempts| attempts > MAX_SEARCH_202_RETRIES)
        {
            return Err(SearchError::InvalidArg);
        }
        let author_ids = split_non_empty_list(params.author_id.as_deref())?;
        let mentions = split_non_empty_list(params.mentions.as_deref())?;
        let has_tokens = validate_has_tokens(params.has.as_deref())?;
        if let Some(author_type) = params.author_type.as_deref() {
            let bare = author_type.strip_prefix('-').unwrap_or(author_type);
            if !SEARCH_AUTHOR_TYPES.contains(&bare) {
                return Err(SearchError::InvalidArg);
            }
        }
        let min_snowflake = params.min_id.as_deref().map(parse_snowflake).transpose()?;
        let max_snowflake = params.max_id.as_deref().map(parse_snowflake).transpose()?;
        if let (Some(min_val), Some(max_val)) = (min_snowflake, max_snowflake) {
            if min_val > max_val {
                return Err(SearchError::InvalidArg);
            }
        }
        if let Some(session_id) = params.search_session_id.as_deref() {
            if session_id.trim().is_empty() || session_id.contains(['{', '}', '\r', '\n']) {
                return Err(SearchError::InvalidArg);
            }
        }
        if let Some(query_id) = params.search_query_id.as_deref() {
            if query_id.trim().is_empty() || query_id.contains(['{', '}', '\r', '\n']) {
                return Err(SearchError::InvalidArg);
            }
        }

        let route = routes::find(HttpMethod::Get, "/guilds/{id}/messages/search")
            .ok_or(SearchError::Network)?;
        let encoded_guild = percent_encode(guild_id);
        let path =
            routes::render_path(route, &[&encoded_guild]).map_err(|_| SearchError::InvalidArg)?;
        let mut query = Vec::new();
        if let Some(content) = params.content.as_deref() {
            query.push(format!("content={}", percent_encode(content)));
        }
        if let Some(channel_id) = params.channel_id.as_deref() {
            for item in channel_id.split(',') {
                query.push(format!("channel_id={}", percent_encode(item)));
            }
        }
        for author_id in author_ids {
            query.push(format!("author_id={}", percent_encode(author_id)));
        }
        for mention in mentions {
            query.push(format!("mentions={}", percent_encode(mention)));
        }
        for token in has_tokens {
            query.push(format!("has={}", percent_encode(token)));
        }
        if let Some(pinned) = params.pinned {
            query.push(format!("pinned={pinned}"));
        }
        if let Some(author_type) = params.author_type.as_deref() {
            query.push(format!("author_type={}", percent_encode(author_type)));
        }
        if let Some(min_id) = params.min_id.as_deref() {
            query.push(format!("min_id={}", percent_encode(min_id)));
        }
        if let Some(max_id) = params.max_id.as_deref() {
            query.push(format!("max_id={}", percent_encode(max_id)));
        }
        if let Some(hosts) = params.link_hostname.as_ref() {
            for host in hosts {
                let trimmed = host.trim();
                if trimmed.is_empty() {
                    return Err(SearchError::InvalidArg);
                }
                query.push(format!("link_hostname={}", percent_encode(trimmed)));
            }
        }
        if let Some(exts) = params.attachment_extension.as_ref() {
            for ext in exts {
                let cleaned = ext.trim().trim_start_matches('.');
                if cleaned.is_empty() {
                    return Err(SearchError::InvalidArg);
                }
                query.push(format!("attachment_extension={}", percent_encode(cleaned)));
            }
        }
        if let Some(names) = params.attachment_filename.as_ref() {
            for name in names {
                let trimmed = name.trim();
                if trimmed.is_empty() {
                    return Err(SearchError::InvalidArg);
                }
                query.push(format!("attachment_filename={}", percent_encode(trimmed)));
            }
        }
        for (name, value) in [
            ("sort_by", params.sort_by.as_deref()),
            ("sort_order", params.sort_order.as_deref()),
        ] {
            if let Some(value) = value {
                query.push(format!("{name}={}", percent_encode(value)));
            }
        }
        if let Some(search_session_id) = params.search_session_id.as_deref() {
            query.push(format!(
                "search_session_id={}",
                percent_encode(search_session_id)
            ));
        }
        if let Some(search_query_id) = params.search_query_id.as_deref() {
            query.push(format!(
                "search_query_id={}",
                percent_encode(search_query_id)
            ));
        }
        if let Some(limit) = params.limit {
            query.push(format!("limit={limit}"));
        }
        if let Some(offset) = params.offset {
            query.push(format!("offset={offset}"));
        }
        if params.include_nsfw == Some(true) {
            query.push("include_nsfw=true".to_string());
        }
        if let Some(attempts) = params.attempts.filter(|&n| n > 0) {
            query.push(format!("attempts={attempts}"));
        }
        let target = if query.is_empty() {
            path
        } else {
            format!("{path}?{}", query.join("&"))
        };
        let response = self.transport.get(&target).await?;
        if response.status == 202 {
            let retry_after_ms = parse_retry_after_ms(&response.headers, &response.body);
            return Ok(SearchResponse {
                messages: Vec::new(),
                total_results: 0,
                doing_deep_historical_index: false,
                documents_indexed: 0,
                indexing: true,
                retry_after_ms: Some(retry_after_ms),
            });
        }
        if response.status == 429 {
            let retry_after_ms =
                parse_429_retry_after_ms(&response.headers, &response.body).unwrap_or(0);
            return Err(SearchError::RateLimited { retry_after_ms });
        }
        if let Some(failure) = RestFailure::from_status(response.status) {
            return Err(failure.into());
        }
        serde_json::from_slice(&response.body).map_err(|_| SearchError::InvalidResponse)
    }
}

impl RemoteSearch for DiscordSearch {
    fn search<'a>(
        &'a self,
        guild_id: &'a str,
        params: &'a SearchParams,
    ) -> Pin<Box<dyn Future<Output = Result<SearchResponse, SearchError>> + Send + 'a>> {
        Box::pin(self.execute(guild_id, params))
    }
}

fn parse_retry_after_ms(headers: &[(String, String)], body: &[u8]) -> u64 {
    if let Some((_, value)) = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("retry-after"))
    {
        if let Ok(secs) = value.trim().parse::<i64>() {
            if secs > 0 {
                return (secs as u64).saturating_mul(1000);
            }
        }
        return DEFAULT_SEARCH_202_RETRY_AFTER_MS;
    }
    // Если заголовок Retry-After отсутствует, разбираем retry_after из JSON-тела
    if let Ok(Value::Object(map)) = serde_json::from_slice::<Value>(body) {
        if let Some(val) = map.get("retry_after") {
            let secs_opt = val
                .as_f64()
                .or_else(|| val.as_str().and_then(|s| s.trim().parse::<f64>().ok()));
            if let Some(secs) = secs_opt {
                if secs > 0.0 {
                    return (secs * 1000.0).round() as u64;
                }
            }
        }
    }
    DEFAULT_SEARCH_202_RETRY_AFTER_MS
}

fn parse_429_retry_after_ms(headers: &[(String, String)], body: &[u8]) -> Option<u64> {
    if let Ok(Value::Object(map)) = serde_json::from_slice::<Value>(body) {
        if let Some(val) = map.get("retry_after") {
            let secs_opt = val
                .as_f64()
                .or_else(|| val.as_str().and_then(|s| s.trim().parse::<f64>().ok()));
            if let Some(secs) = secs_opt {
                if secs.is_finite() && secs > 0.0 {
                    return Some((secs * 1000.0).round() as u64);
                }
            }
        }
    }
    if let Some((_, value)) = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("retry-after"))
    {
        if let Ok(secs) = value.trim().parse::<f64>() {
            if secs.is_finite() && secs > 0.0 {
                return Some((secs * 1000.0).round() as u64);
            }
        }
    }
    None
}

fn parse_hit_message_value(value: &Value) -> Option<SearchHitMessage> {
    let obj = value.as_object()?;
    let id = obj.get("id")?.as_str()?.to_string();
    let channel_id = obj
        .get("channel_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let author_obj = obj.get("author").and_then(Value::as_object);
    let author_id = author_obj
        .and_then(|a| a.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let author_username = author_obj
        .and_then(|a| a.get("username"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let content = obj
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let timestamp = obj
        .get("timestamp")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let hit = obj.get("hit").and_then(Value::as_bool).unwrap_or(false);
    let pinned = obj.get("pinned").and_then(Value::as_bool).unwrap_or(false);
    let attachments_count = obj
        .get("attachments")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let embeds_count = obj
        .get("embeds")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);

    Some(SearchHitMessage {
        id,
        channel_id,
        author_id,
        author_username,
        content,
        timestamp,
        hit,
        pinned,
        attachments_count,
        embeds_count,
    })
}

fn split_csv_owned(raw: Option<&str>) -> Vec<String> {
    let Some(s) = raw else {
        return Vec::new();
    };
    s.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn push_unique(vec: &mut Vec<String>, item: String) {
    if !item.is_empty() && !vec.contains(&item) {
        vec.push(item);
    }
}

fn split_non_empty_list(value: Option<&str>) -> Result<Vec<&str>, SearchError> {
    let Some(raw) = value else {
        return Ok(Vec::new());
    };
    let items: Vec<&str> = raw.split(',').map(str::trim).collect();
    if items.is_empty() || items.iter().any(|item| item.is_empty()) {
        return Err(SearchError::InvalidArg);
    }
    Ok(items)
}

fn validate_has_tokens(value: Option<&str>) -> Result<Vec<&str>, SearchError> {
    let tokens = split_non_empty_list(value)?;
    for token in &tokens {
        let bare = token.strip_prefix('-').unwrap_or(token);
        if !SEARCH_HAS_VALUES.contains(&bare) {
            return Err(SearchError::InvalidArg);
        }
    }
    Ok(tokens)
}

fn parse_snowflake(value: &str) -> Result<u64, SearchError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(SearchError::InvalidArg);
    }
    value.parse::<u64>().map_err(|_| SearchError::InvalidArg)
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(*byte as char);
        } else {
            encoded.push('%');
            encoded.push(char::from(b"0123456789ABCDEF"[(byte >> 4) as usize]));
            encoded.push(char::from(b"0123456789ABCDEF"[(byte & 0x0f) as usize]));
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_after_header_parses_positive_integer_seconds_with_5000ms_fallback() {
        assert_eq!(
            parse_retry_after_ms(&[("Retry-After".into(), "2".into())], b""),
            2000
        );
        assert_eq!(
            parse_retry_after_ms(&[("retry-after".into(), " 7 ".into())], b""),
            7000
        );
        assert_eq!(
            parse_retry_after_ms(&[("Retry-After".into(), "0".into())], b""),
            DEFAULT_SEARCH_202_RETRY_AFTER_MS
        );
        assert_eq!(
            parse_retry_after_ms(&[("Retry-After".into(), "-1".into())], b""),
            DEFAULT_SEARCH_202_RETRY_AFTER_MS
        );
        assert_eq!(
            parse_retry_after_ms(&[("Retry-After".into(), "0.25".into())], b""),
            DEFAULT_SEARCH_202_RETRY_AFTER_MS
        );
        assert_eq!(
            parse_retry_after_ms(&[], b""),
            DEFAULT_SEARCH_202_RETRY_AFTER_MS
        );
    }

    #[test]
    fn retry_after_body_parses_float_seconds_when_header_absent() {
        assert_eq!(
            parse_retry_after_ms(&[], br#"{"retry_after": 2.0, "message": "indexing"}"#),
            2000
        );
        assert_eq!(parse_retry_after_ms(&[], br#"{"retry_after": 1.5}"#), 1500);
        assert_eq!(parse_retry_after_ms(&[], br#"{"retry_after": 3}"#), 3000);
        assert_eq!(
            parse_retry_after_ms(&[], br#"{"retry_after": 0.0}"#),
            DEFAULT_SEARCH_202_RETRY_AFTER_MS
        );
        assert_eq!(
            parse_retry_after_ms(&[], br#"{"retry_after": -2.0}"#),
            DEFAULT_SEARCH_202_RETRY_AFTER_MS
        );
        // Header takes precedence over body if header is present
        assert_eq!(
            parse_retry_after_ms(
                &[("Retry-After".into(), "2".into())],
                br#"{"retry_after": 99}"#
            ),
            2000
        );
    }

    #[test]
    fn m13_constants_and_indexing_error_format_match_spec() {
        assert_eq!(MAX_SEARCH_LIMIT, 25);
        assert_eq!(MAX_SEARCH_OFFSET, 9975);
        assert_eq!(MAX_SEARCH_PAGES, 400);
        assert_eq!(MAX_SEARCH_202_RETRIES, 5);
        assert_eq!(MAX_INDEXING_RETRIES, 5);
        assert_eq!(DEFAULT_SEARCH_202_RETRY_AFTER_MS, 5000);
        let err = SearchError::Indexing {
            retry_after_ms: 2000,
            attempts: 5,
        };
        assert!(format!("{err}").contains("после 5 попыток; повтор через 2000 мс"));
    }

    #[test]
    fn uuid_v4_generation_matches_rfc4122_shape() {
        let uuid = generate_uuid_v4();
        assert_eq!(uuid.len(), 36);
        assert_eq!(&uuid[14..15], "4");
        let variant = uuid.chars().nth(19).unwrap();
        assert!(matches!(variant, '8' | '9' | 'a' | 'b'));

        let det = deterministic_uuid_v4(12345);
        assert_eq!(det.len(), 36);
        assert_eq!(&det[14..15], "4");
        assert_eq!(det, deterministic_uuid_v4(12345));
    }
}
