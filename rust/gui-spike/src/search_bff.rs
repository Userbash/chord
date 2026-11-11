#![forbid(unsafe_code)]
//! BFF полнотекстового REST-поиска сообщений (`search_bff`).

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use discord_core::rest::client::{ApiAuth, ApiClient, ApiConfig, ApiRetryPolicy};
use discord_core::rest::routes::HttpMethod;
use discord_core::rest::super_properties::SuperProperties;
use discord_core::rest::transport::{HttpClientConfig, HttpRequest, HttpResponse};
use discord_core::API_V9_BASE;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const MAX_SEARCH_LIMIT: u8 = 25;
pub const DEFAULT_SEARCH_LIMIT: u8 = 25;
pub const MAX_SEARCH_OFFSET: u32 = 9975;
pub const MAX_SEARCH_PAGES: u32 = 400;
pub const MAX_SEARCH_CONTENT_CHARS: usize = 512;
pub const MAX_SEARCH_202_RETRIES: u32 = 5;
pub const MAX_SEARCH_429_RETRIES: u32 = 3;
pub const DEFAULT_SEARCH_202_RETRY_AFTER_MS: u64 = 5_000;
pub const DEFAULT_SEARCH_429_RETRY_AFTER_MS: u64 = 1_000;
pub const MIN_SEARCH_429_BASE_MS: u64 = 100;
pub const MAX_SEARCH_429_BACKOFF_MS: u64 = 30_000;
pub const MAX_SEARCH_CACHE_ENTRIES: usize = 64;
pub const SEARCH_HAS_FACETS: [&str; 9] = [
    "link", "embed", "poll", "snapshot", "file", "video", "image", "sound", "sticker",
];
pub const SEARCH_AUTHOR_TYPES: [&str; 3] = ["user", "bot", "webhook"];
pub const DISCORD_LOCALE_RU: &str = "ru";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchSortBy {
    Timestamp,
    Relevance,
}

impl SearchSortBy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Timestamp => "timestamp",
            Self::Relevance => "relevance",
        }
    }
    pub fn parse_str(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "timestamp" => Some(Self::Timestamp),
            "relevance" => Some(Self::Relevance),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchSortOrder {
    Desc,
    Asc,
}

impl SearchSortOrder {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Desc => "desc",
            Self::Asc => "asc",
        }
    }
    pub fn parse_str(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "desc" => Some(Self::Desc),
            "asc" => Some(Self::Asc),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum SearchBffScope {
    Guild(String),
    Channel(String),
}

impl SearchBffScope {
    pub fn validate(&self) -> Result<&str, SearchBffError> {
        let raw = match self {
            Self::Guild(id) | Self::Channel(id) => id.trim(),
        };
        if parse_valid_snowflake(raw).is_some() {
            Ok(raw)
        } else {
            Err(SearchBffError::InvalidScopeId)
        }
    }
    pub fn scope_key(&self) -> Result<String, SearchBffError> {
        let id = self.validate()?;
        match self {
            Self::Guild(_) => Ok(format!("guild:{id}")),
            Self::Channel(_) => Ok(format!("channel:{id}")),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SearchBffQuery {
    pub content: Option<String>,
    pub from: Vec<String>,
    pub mentions: Vec<String>,
    pub has: Vec<String>,
    pub pinned: Option<bool>,
    pub channel_id: Vec<String>,
    pub author_type: Vec<String>,
    pub min_id: Option<String>,
    pub max_id: Option<String>,
    pub link_hostname: Vec<String>,
    pub attachment_extension: Vec<String>,
    pub attachment_filename: Vec<String>,
    pub sort_by: Option<SearchSortBy>,
    pub sort_order: Option<SearchSortOrder>,
    pub limit: Option<u8>,
    pub offset: Option<u32>,
    pub include_nsfw: Option<bool>,
    pub search_session_id: Option<String>,
    pub search_query_id: Option<String>,
    pub attempts: Option<u32>,
}

impl SearchBffQuery {
    pub fn parse(raw_query: &str) -> Result<Self, SearchBffError> {
        let mut q = Self::default();
        q.apply_inline_tokens(raw_query)?;
        q.validate_and_normalize()
    }
    pub fn apply_inline_tokens(&mut self, raw_query: &str) -> Result<(), SearchBffError> {
        let mut words = Vec::new();
        for tok in raw_query.split_whitespace() {
            if let Some((k, v)) = tok.split_once(':') {
                if !apply_inline_pair(self, &k.to_ascii_lowercase(), v.trim())? {
                    words.push(tok);
                }
            } else {
                words.push(tok);
            }
        }
        if !words.is_empty() {
            let joined = words.join(" ");
            match &mut self.content {
                Some(ex) if !ex.trim().is_empty() => *ex = format!("{} {}", ex.trim(), joined),
                slot => *slot = Some(joined),
            }
        }
        Ok(())
    }
    pub fn validate_and_normalize(&self) -> Result<Self, SearchBffError> {
        let mut out = self.clone();
        if let Some(raw) = out.content.take() {
            out.apply_inline_tokens(&raw)?;
        }
        if let Some(raw) = out.content.take() {
            let col = raw.split_whitespace().collect::<Vec<_>>().join(" ");
            if !col.is_empty() {
                let len = col.chars().count();
                if len > MAX_SEARCH_CONTENT_CHARS {
                    return Err(SearchBffError::ContentTooLong {
                        len,
                        max: MAX_SEARCH_CONTENT_CHARS,
                    });
                }
                out.content = Some(col);
            }
        }
        out.from = norm_list(&out.from, |s| check_sf(s, "from"))?;
        out.mentions = norm_list(&out.mentions, |s| check_sf(s, "mentions"))?;
        out.channel_id = norm_list(&out.channel_id, |s| check_sf(s, "channel_id"))?;
        out.has = norm_list(&out.has, validate_has_facet)?;
        out.author_type = norm_list(&out.author_type, validate_author_type)?;
        let min_v = out
            .min_id
            .as_deref()
            .map(|s| validate_sf(s, "min_id"))
            .transpose()?;
        let max_v = out
            .max_id
            .as_deref()
            .map(|s| validate_sf(s, "max_id"))
            .transpose()?;
        if min_v.zip(max_v).is_some_and(|(lo, hi)| lo > hi) {
            return Err(SearchBffError::MinIdGreaterThanMaxId);
        }
        out.min_id = out.min_id.map(|s| s.trim().to_string());
        out.max_id = out.max_id.map(|s| s.trim().to_string());
        out.link_hostname = norm_list(&out.link_hostname, norm_host)?;
        out.attachment_extension = norm_list(&out.attachment_extension, norm_ext)?;
        out.attachment_filename = norm_list(&out.attachment_filename, norm_file)?;
        validate_limits_and_session(&out)?;
        if has_criteria(&out) {
            Ok(out)
        } else {
            Err(SearchBffError::EmptyQuery)
        }
    }
    pub fn to_query_string(&self) -> Result<String, SearchBffError> {
        let n = self.validate_and_normalize()?;
        let mut p = Vec::new();
        push_opt(&mut p, "content", n.content.as_deref());
        push_all(&mut p, "channel_id", &n.channel_id);
        push_all(&mut p, "author_id", &n.from);
        push_all(&mut p, "mentions", &n.mentions);
        push_all(&mut p, "has", &n.has);
        if let Some(pin) = n.pinned {
            p.push(("pinned", pin.to_string()));
        }
        push_all(&mut p, "author_type", &n.author_type);
        push_opt(&mut p, "min_id", n.min_id.as_deref());
        push_opt(&mut p, "max_id", n.max_id.as_deref());
        push_all(&mut p, "link_hostname", &n.link_hostname);
        push_all(&mut p, "attachment_extension", &n.attachment_extension);
        push_all(&mut p, "attachment_filename", &n.attachment_filename);
        push_opt(&mut p, "sort_by", n.sort_by.map(SearchSortBy::as_str));
        push_opt(
            &mut p,
            "sort_order",
            n.sort_order.map(SearchSortOrder::as_str),
        );
        if let Some(l) = n.limit {
            p.push(("limit", l.to_string()));
        }
        if let Some(o) = n.offset {
            p.push(("offset", o.to_string()));
        }
        if n.include_nsfw == Some(true) {
            p.push(("include_nsfw", "true".into()));
        }
        push_opt(
            &mut p,
            "search_session_id",
            n.search_session_id.as_deref().map(str::trim),
        );
        push_opt(
            &mut p,
            "search_query_id",
            n.search_query_id.as_deref().map(str::trim),
        );
        if let Some(att) = n.attempts.filter(|&a| a > 0) {
            p.push(("attempts", att.to_string()));
        }
        Ok(p.into_iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&"))
    }
    pub fn build_endpoint(&self, scope: &SearchBffScope) -> Result<String, SearchBffError> {
        let (id, qs) = (scope.validate()?, self.to_query_string()?);
        match scope {
            SearchBffScope::Guild(_) => Ok(format!("/guilds/{id}/messages/search?{qs}")),
            SearchBffScope::Channel(_) => Ok(format!("/channels/{id}/messages/search?{qs}")),
        }
    }
    pub fn build_api_v9_path(&self, scope: &SearchBffScope) -> Result<String, SearchBffError> {
        Ok(format!("/api/v9{}", self.build_endpoint(scope)?))
    }
    pub fn canonical_cache_key(&self, scope: &SearchBffScope) -> Result<String, SearchBffError> {
        let (prefix, mut n) = (scope.scope_key()?, self.validate_and_normalize()?);
        (n.search_session_id, n.search_query_id, n.attempts) = (None, None, None);
        Ok(format!("{prefix}?{}", n.to_query_string()?))
    }
    pub fn with_offset(&self, offset: u32) -> Self {
        let mut next = self.clone();
        next.offset = Some(offset);
        next
    }
    pub fn next_page(&self, step: u32) -> Self {
        let s = if step > 0 {
            step
        } else {
            u32::from(DEFAULT_SEARCH_LIMIT)
        };
        self.with_offset(self.offset.unwrap_or(0).saturating_add(s))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SearchAttachmentDto {
    pub id: String,
    pub filename: String,
    pub size: u64,
    pub url: Option<String>,
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SearchMessageDto {
    pub id: String,
    pub channel_id: String,
    pub author_id: Option<String>,
    pub author_username: Option<String>,
    pub content: String,
    pub timestamp: String,
    pub hit: bool,
    pub pinned: bool,
    pub attachments: Vec<SearchAttachmentDto>,
    pub embeds_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SearchContextGroupDto {
    pub messages: Vec<SearchMessageDto>,
}

impl SearchContextGroupDto {
    pub fn hit_message(&self) -> Option<&SearchMessageDto> {
        self.messages
            .iter()
            .find(|m| m.hit)
            .or_else(|| (self.messages.len() == 1).then(|| &self.messages[0]))
    }
    pub fn context_before(&self) -> &[SearchMessageDto] {
        self.messages
            .iter()
            .position(|m| m.hit)
            .map_or(&[], |i| &self.messages[..i])
    }
    pub fn context_after(&self) -> &[SearchMessageDto] {
        self.messages
            .iter()
            .position(|m| m.hit)
            .map_or(&[], |i| &self.messages[i + 1..])
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SearchBffResponse {
    pub analytics_id: Option<String>,
    pub total_results: u64,
    pub doing_deep_historical_index: bool,
    pub documents_indexed: u64,
    pub indexing: bool,
    pub retry_after_ms: Option<u64>,
    pub groups: Vec<SearchContextGroupDto>,
}

impl SearchBffResponse {
    pub fn hits(&self) -> Vec<&SearchMessageDto> {
        self.groups
            .iter()
            .filter_map(SearchContextGroupDto::hit_message)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SearchBffError {
    InvalidScopeId,
    EmptyQuery,
    ContentTooLong {
        len: usize,
        max: usize,
    },
    OffsetOutOfRange {
        offset: u32,
        max: u32,
    },
    InvalidLimit {
        limit: u8,
    },
    InvalidSnowflake {
        field: String,
        value: String,
    },
    InvalidHasFacet {
        value: String,
    },
    InvalidPinnedValue {
        value: String,
    },
    InvalidAuthorType {
        value: String,
    },
    MinIdGreaterThanMaxId,
    InvalidFilterValue {
        field: String,
    },
    MissingToken,
    Unauthorized,
    Forbidden,
    NotFound,
    IndexingTimeout {
        retry_after_ms: u64,
        attempts: u32,
    },
    RateLimited {
        retry_after_ms: u64,
        is_global: bool,
        attempts: u32,
    },
    Client {
        status: u16,
    },
    Server {
        status: u16,
    },
    Network,
    InvalidResponse,
}

impl fmt::Display for SearchBffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for SearchBffError {}

#[derive(Debug, Clone, Default)]
pub struct SearchRateLimiter {
    global_until_ms: u64,
    global_attempts: u32,
    buckets: HashMap<String, (u64, u32)>,
}

impl SearchRateLimiter {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn record_429(
        &mut self,
        scope_key: &str,
        now_ms: u64,
        retry_after_ms: u64,
        is_global: bool,
    ) -> u64 {
        let key = scope_key.trim().to_string();
        let cur = self.buckets.get(&key).map_or(0, |&(_, a)| a);
        let att = if is_global {
            self.global_attempts.max(cur)
        } else {
            cur
        };
        let backoff = compute_429_backoff_ms(retry_after_ms, att);
        let until = now_ms.saturating_add(backoff);
        self.buckets.insert(key, (until, cur.saturating_add(1)));
        if is_global {
            self.global_until_ms = self.global_until_ms.max(until);
            self.global_attempts = self.global_attempts.saturating_add(1);
        }
        backoff
    }
    pub fn record_success(&mut self, scope_key: &str) {
        self.buckets.remove(scope_key.trim());
    }
    pub fn remaining_cooldown_ms(&self, scope_key: &str, now_ms: u64) -> Option<u64> {
        let g = self.global_until_ms.saturating_sub(now_ms);
        let b = self
            .buckets
            .get(scope_key.trim())
            .map_or(0, |&(u, _)| u.saturating_sub(now_ms));
        (g.max(b) > 0).then_some(g.max(b))
    }
    pub fn consecutive_429(&self, scope_key: &str) -> u32 {
        self.buckets.get(scope_key.trim()).map_or(0, |&(_, a)| a)
    }
}

pub trait SearchBffTransport: Send + Sync {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, SearchBffError>> + Send + 'a>>;
    fn sleep<'a>(&'a self, delay: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(tokio::time::sleep(delay))
    }
}

impl SearchBffTransport for ApiClient {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, SearchBffError>> + Send + 'a>> {
        Box::pin(async move {
            ApiClient::get(self, target)
                .await
                .map_err(|_| SearchBffError::Network)
        })
    }
}

pub struct SearchBffClient {
    transport: Arc<dyn SearchBffTransport>,
    cache: Mutex<VecDeque<(String, SearchBffResponse)>>,
}

impl fmt::Debug for SearchBffClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SearchBffClient").finish_non_exhaustive()
    }
}

impl SearchBffClient {
    pub fn new(token: &str) -> Result<Self, SearchBffError> {
        let t = token.trim();
        if t.is_empty() {
            return Err(SearchBffError::MissingToken);
        }
        let sp = SuperProperties::default();
        let auth = ApiAuth::from_token(t, &sp, DISCORD_LOCALE_RU, "UTC")
            .map_err(|_| bad_field("token"))?;
        let retry = ApiRetryPolicy {
            max_attempts: 1,
            ..ApiRetryPolicy::default()
        };
        let cfg = ApiConfig {
            base: API_V9_BASE.into(),
            auth,
            http: HttpClientConfig::default(),
            retry,
        };
        Ok(Self::with_transport(Arc::new(
            ApiClient::new(cfg).map_err(|_| SearchBffError::Network)?,
        )))
    }
    pub fn with_transport(transport: Arc<dyn SearchBffTransport>) -> Self {
        Self {
            transport,
            cache: Mutex::new(VecDeque::new()),
        }
    }
    pub fn cache_len(&self) -> usize {
        self.cache.lock().map_or(0, |g| g.len())
    }
    pub fn invalidate_scope_cache(&self, scope: &SearchBffScope) {
        if let (Ok(pfx), Ok(mut g)) = (scope.scope_key(), self.cache.lock()) {
            let p = format!("{pfx}?");
            g.retain(|(k, _)| !k.starts_with(&p));
        }
    }
    pub fn clear_cache(&self) {
        if let Ok(mut g) = self.cache.lock() {
            g.clear();
        }
    }
    pub async fn execute(
        &self,
        scope: &SearchBffScope,
        query: &SearchBffQuery,
    ) -> Result<SearchBffResponse, SearchBffError> {
        let ep = query.build_endpoint(scope)?;
        parse_search_http_response(&self.transport.get(&ep).await?)
    }
    pub async fn execute_with_retry(
        &self,
        scope: &SearchBffScope,
        query: &SearchBffQuery,
    ) -> Result<SearchBffResponse, SearchBffError> {
        let (mut cur, mut r202, mut r429) = (query.validate_and_normalize()?, 0u32, 0u32);
        loop {
            match self.execute(scope, &cur).await {
                Ok(r) if r.indexing => {
                    let w = r
                        .retry_after_ms
                        .unwrap_or(DEFAULT_SEARCH_202_RETRY_AFTER_MS);
                    if r202 >= MAX_SEARCH_202_RETRIES {
                        return Err(SearchBffError::IndexingTimeout {
                            retry_after_ms: w,
                            attempts: r202,
                        });
                    }
                    r202 += 1;
                    cur.attempts = Some(r202);
                    self.transport.sleep(Duration::from_millis(w)).await;
                }
                Ok(r) => return Ok(r),
                Err(SearchBffError::RateLimited {
                    retry_after_ms,
                    is_global,
                    ..
                }) => {
                    if r429 >= MAX_SEARCH_429_RETRIES {
                        return Err(SearchBffError::RateLimited {
                            retry_after_ms,
                            is_global,
                            attempts: r429,
                        });
                    }
                    let w = compute_429_backoff_ms(retry_after_ms, r429);
                    r429 += 1;
                    self.transport.sleep(Duration::from_millis(w)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }
    pub async fn search_cached(
        &self,
        scope: &SearchBffScope,
        query: &SearchBffQuery,
    ) -> Result<SearchBffResponse, SearchBffError> {
        let key = query.canonical_cache_key(scope)?;
        if let Ok(mut g) = self.cache.lock() {
            if let Some(hit) = lru_get(&mut g, &key) {
                return Ok(hit);
            }
        }
        let resp = self.execute_with_retry(scope, query).await?;
        if !resp.indexing && !resp.doing_deep_historical_index {
            if let Ok(mut g) = self.cache.lock() {
                lru_put(&mut g, key, resp.clone());
            }
        }
        Ok(resp)
    }
}

pub fn parse_search_query(raw_query: &str) -> Result<SearchBffQuery, SearchBffError> {
    SearchBffQuery::parse(raw_query)
}
pub fn compute_429_backoff_ms(base_retry_after_ms: u64, attempt: u32) -> u64 {
    let b = if base_retry_after_ms == 0 {
        DEFAULT_SEARCH_429_RETRY_AFTER_MS
    } else {
        base_retry_after_ms.max(MIN_SEARCH_429_BASE_MS)
    };
    b.saturating_mul(1_u64 << attempt.min(10))
        .min(MAX_SEARCH_429_BACKOFF_MS)
}
pub fn parse_429_retry_after(headers: &[(String, String)], body: &[u8]) -> (u64, bool) {
    let j = serde_json::from_slice::<Value>(body).ok();
    let global = j
        .as_ref()
        .and_then(|v| v.get("global"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let ms = retry_ms_json(j.as_ref())
        .or_else(|| retry_ms_headers(headers))
        .unwrap_or(DEFAULT_SEARCH_429_RETRY_AFTER_MS);
    (ms, global)
}
pub fn parse_202_retry_after_ms(headers: &[(String, String)], body: &[u8]) -> u64 {
    let j = serde_json::from_slice::<Value>(body).ok();
    retry_ms_headers(headers)
        .or_else(|| retry_ms_json(j.as_ref()))
        .unwrap_or(DEFAULT_SEARCH_202_RETRY_AFTER_MS)
}
pub fn parse_search_http_response(
    response: &HttpResponse,
) -> Result<SearchBffResponse, SearchBffError> {
    match response.status {
        200 => parse_200_payload(&response.body),
        202 => Ok(parse_202_payload(&response.headers, &response.body)),
        401 => Err(SearchBffError::Unauthorized),
        403 => Err(SearchBffError::Forbidden),
        404 => Err(SearchBffError::NotFound),
        429 => {
            let (retry_after_ms, is_global) =
                parse_429_retry_after(&response.headers, &response.body);
            Err(SearchBffError::RateLimited {
                retry_after_ms,
                is_global,
                attempts: 0,
            })
        }
        status if (500..=599).contains(&status) => Err(SearchBffError::Server { status }),
        status => Err(SearchBffError::Client { status }),
    }
}
pub fn build_search_http_request(
    token: &str,
    super_properties: &SuperProperties,
    timezone: &str,
    scope: &SearchBffScope,
    query: &SearchBffQuery,
) -> Result<HttpRequest, SearchBffError> {
    let t = token.trim();
    if t.is_empty() {
        return Err(SearchBffError::MissingToken);
    }
    let path = query.build_api_v9_path(scope)?;
    let auth = ApiAuth::from_token(t, super_properties, DISCORD_LOCALE_RU, timezone)
        .map_err(|_| bad_field("headers"))?;
    let mut req = HttpRequest::new(HttpMethod::Get, path);
    for (k, v) in auth.pairs() {
        req = req.header(k, v);
    }
    Ok(req)
}

fn push_opt(p: &mut Vec<(&'static str, String)>, k: &'static str, v: Option<&str>) {
    if let Some(val) = v {
        p.push((k, percent_encode(val)));
    }
}
fn push_all(p: &mut Vec<(&'static str, String)>, k: &'static str, list: &[String]) {
    for v in list {
        p.push((k, percent_encode(v)));
    }
}
fn lru_get(g: &mut VecDeque<(String, SearchBffResponse)>, key: &str) -> Option<SearchBffResponse> {
    let pos = g.iter().position(|(k, _)| k == key)?;
    let entry = g.remove(pos)?;
    let hit = entry.1.clone();
    g.push_back(entry);
    Some(hit)
}
fn lru_put(g: &mut VecDeque<(String, SearchBffResponse)>, key: String, resp: SearchBffResponse) {
    if let Some(pos) = g.iter().position(|(k, _)| k == &key) {
        g.remove(pos);
    } else if g.len() >= MAX_SEARCH_CACHE_ENTRIES {
        g.pop_front();
    }
    g.push_back((key, resp));
}
fn bad_field(field: &str) -> SearchBffError {
    SearchBffError::InvalidFilterValue {
        field: field.into(),
    }
}
fn apply_inline_pair(q: &mut SearchBffQuery, key: &str, val: &str) -> Result<bool, SearchBffError> {
    match key {
        "from" | "author_id" => push_unique(&mut q.from, check_sf(val, "from")?),
        "mentions" => push_unique(&mut q.mentions, check_sf(val, "mentions")?),
        "has" => push_unique(&mut q.has, validate_has_facet(val)?),
        "pinned" => q.pinned = Some(parse_pinned_bool(val)?),
        "in" | "channel_id" => push_unique(&mut q.channel_id, check_sf(val, "channel_id")?),
        "before" | "max_id" => q.max_id = Some(check_sf(val, "max_id")?),
        "after" | "min_id" => q.min_id = Some(check_sf(val, "min_id")?),
        "author_type" => push_unique(&mut q.author_type, validate_author_type(val)?),
        "link_hostname" | "site" => push_unique(&mut q.link_hostname, norm_host(val)?),
        "attachment_extension" | "ext" => push_unique(&mut q.attachment_extension, norm_ext(val)?),
        "attachment_filename" | "filename" => {
            push_unique(&mut q.attachment_filename, norm_file(val)?)
        }
        _ => return Ok(false),
    }
    Ok(true)
}
fn validate_sf(raw: &str, field: &str) -> Result<u64, SearchBffError> {
    if let Some(v) = parse_valid_snowflake(raw) {
        Ok(v)
    } else {
        Err(SearchBffError::InvalidSnowflake {
            field: field.into(),
            value: raw.trim().into(),
        })
    }
}
fn check_sf(raw: &str, field: &str) -> Result<String, SearchBffError> {
    validate_sf(raw, field).map(|_| raw.trim().into())
}
fn parse_valid_snowflake(raw: &str) -> Option<u64> {
    let t = raw.trim();
    if t.is_empty() || t.len() > 20 || !t.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    t.parse::<u64>().ok().filter(|&v| v > 0)
}
fn norm_list(
    items: &[String],
    f: impl Fn(&str) -> Result<String, SearchBffError>,
) -> Result<Vec<String>, SearchBffError> {
    let mut out = Vec::with_capacity(items.len());
    for raw in items {
        push_unique(&mut out, f(raw)?);
    }
    Ok(out)
}
fn validate_has_facet(raw: &str) -> Result<String, SearchBffError> {
    let low = raw.trim().to_ascii_lowercase();
    let core = low.strip_prefix('-').unwrap_or(&low);
    if !core.is_empty() && SEARCH_HAS_FACETS.contains(&core) {
        Ok(low)
    } else {
        Err(SearchBffError::InvalidHasFacet {
            value: raw.trim().into(),
        })
    }
}
fn parse_pinned_bool(raw: &str) -> Result<bool, SearchBffError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(SearchBffError::InvalidPinnedValue {
            value: raw.trim().into(),
        }),
    }
}
fn validate_author_type(raw: &str) -> Result<String, SearchBffError> {
    let low = raw.trim().to_ascii_lowercase();
    let core = low.strip_prefix('-').unwrap_or(&low);
    if !core.is_empty() && SEARCH_AUTHOR_TYPES.contains(&core) {
        Ok(low)
    } else {
        Err(SearchBffError::InvalidAuthorType {
            value: raw.trim().into(),
        })
    }
}
fn norm_host(raw: &str) -> Result<String, SearchBffError> {
    let t = raw.trim();
    if !t.is_empty() && !t.chars().any(char::is_whitespace) {
        Ok(t.to_ascii_lowercase())
    } else {
        Err(bad_field("link_hostname"))
    }
}
fn norm_ext(raw: &str) -> Result<String, SearchBffError> {
    let c = raw.trim().trim_start_matches('.');
    if !c.is_empty() && !c.chars().any(char::is_whitespace) {
        Ok(c.to_ascii_lowercase())
    } else {
        Err(bad_field("attachment_extension"))
    }
}
fn norm_file(raw: &str) -> Result<String, SearchBffError> {
    let t = raw.trim();
    if !t.is_empty() {
        Ok(t.to_string())
    } else {
        Err(bad_field("attachment_filename"))
    }
}
fn validate_limits_and_session(q: &SearchBffQuery) -> Result<(), SearchBffError> {
    if let Some(limit) = q.limit.filter(|l| !(1..=MAX_SEARCH_LIMIT).contains(l)) {
        return Err(SearchBffError::InvalidLimit { limit });
    }
    if let Some(offset) = q.offset.filter(|&o| o > MAX_SEARCH_OFFSET) {
        return Err(SearchBffError::OffsetOutOfRange {
            offset,
            max: MAX_SEARCH_OFFSET,
        });
    }
    if q.attempts.is_some_and(|a| a > MAX_SEARCH_202_RETRIES) {
        return Err(bad_field("attempts"));
    }
    for (f, o) in [
        ("search_session_id", &q.search_session_id),
        ("search_query_id", &q.search_query_id),
    ] {
        if o.as_deref()
            .is_some_and(|v| v.trim().is_empty() || v.contains(['{', '}', '\r', '\n']))
        {
            return Err(bad_field(f));
        }
    }
    Ok(())
}
fn has_criteria(q: &SearchBffQuery) -> bool {
    q.content.is_some()
        || !q.from.is_empty()
        || !q.mentions.is_empty()
        || !q.has.is_empty()
        || q.pinned.is_some()
        || !q.channel_id.is_empty()
        || !q.author_type.is_empty()
        || q.min_id.is_some()
        || q.max_id.is_some()
        || !q.link_hostname.is_empty()
        || !q.attachment_extension.is_empty()
        || !q.attachment_filename.is_empty()
}
fn parse_200_payload(body: &[u8]) -> Result<SearchBffResponse, SearchBffError> {
    let root: Value = serde_json::from_slice(body).map_err(|_| SearchBffError::InvalidResponse)?;
    let obj = root.as_object().ok_or(SearchBffError::InvalidResponse)?;
    let raw_groups = obj
        .get("messages")
        .and_then(Value::as_array)
        .ok_or(SearchBffError::InvalidResponse)?;
    let mut groups = Vec::with_capacity(raw_groups.len());
    for entry in raw_groups {
        if let Some(arr) = entry.as_array() {
            let msgs: Vec<_> = arr.iter().filter_map(parse_msg).collect();
            if !msgs.is_empty() {
                groups.push(SearchContextGroupDto { messages: msgs });
            }
        } else if let Some(mut m) = parse_msg(entry) {
            m.hit = true;
            groups.push(SearchContextGroupDto { messages: vec![m] });
        }
    }
    Ok(SearchBffResponse {
        analytics_id: str_opt(obj, "analytics_id"),
        total_results: u64_or(obj, "total_results"),
        doing_deep_historical_index: bool_or(obj, "doing_deep_historical_index"),
        documents_indexed: u64_or(obj, "documents_indexed"),
        indexing: false,
        retry_after_ms: None,
        groups,
    })
}
fn parse_msg(val: &Value) -> Option<SearchMessageDto> {
    let o = val.as_object()?;
    let id = str_opt(o, "id")?;
    let au = o.get("author").and_then(Value::as_object);
    let att = o.get("attachments").and_then(Value::as_array);
    Some(SearchMessageDto {
        id,
        channel_id: str_or(o, "channel_id"),
        author_id: au.and_then(|a| str_opt(a, "id")),
        author_username: au.and_then(|a| str_opt(a, "username")),
        content: str_or(o, "content"),
        timestamp: str_or(o, "timestamp"),
        hit: bool_or(o, "hit"),
        pinned: bool_or(o, "pinned"),
        attachments: att
            .map(|a| a.iter().filter_map(parse_att).collect())
            .unwrap_or_default(),
        embeds_count: o
            .get("embeds")
            .and_then(Value::as_array)
            .map_or(0, Vec::len),
    })
}
fn parse_att(val: &Value) -> Option<SearchAttachmentDto> {
    let o = val.as_object()?;
    Some(SearchAttachmentDto {
        id: str_or(o, "id"),
        filename: str_or(o, "filename"),
        size: u64_or(o, "size"),
        url: str_opt(o, "url"),
        content_type: str_opt(o, "content_type"),
    })
}
fn str_opt(o: &Map<String, Value>, k: &str) -> Option<String> {
    o.get(k)?.as_str().map(str::to_string)
}
fn str_or(o: &Map<String, Value>, k: &str) -> String {
    str_opt(o, k).unwrap_or_default()
}
fn u64_or(o: &Map<String, Value>, k: &str) -> u64 {
    o.get(k).and_then(Value::as_u64).unwrap_or(0)
}
fn bool_or(o: &Map<String, Value>, k: &str) -> bool {
    o.get(k).and_then(Value::as_bool).unwrap_or(false)
}
fn parse_202_payload(headers: &[(String, String)], body: &[u8]) -> SearchBffResponse {
    let p = serde_json::from_slice::<Value>(body).ok();
    let o = p.as_ref().and_then(Value::as_object);
    SearchBffResponse {
        doing_deep_historical_index: o.is_some_and(|m| bool_or(m, "doing_deep_historical_index")),
        documents_indexed: o.map_or(0, |m| u64_or(m, "documents_indexed")),
        indexing: true,
        retry_after_ms: Some(parse_202_retry_after_ms(headers, body)),
        ..SearchBffResponse::default()
    }
}
fn retry_ms_json(p: Option<&Value>) -> Option<u64> {
    let v = p?.get("retry_after")?;
    secs_to_ms(
        v.as_f64()
            .or_else(|| v.as_str()?.trim().parse::<f64>().ok())?,
    )
}
fn retry_ms_headers(headers: &[(String, String)]) -> Option<u64> {
    let (_, raw) = headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case("retry-after"))?;
    secs_to_ms(raw.trim().parse::<f64>().ok()?)
}
fn secs_to_ms(s: f64) -> Option<u64> {
    (s.is_finite() && s > 0.0).then(|| (s * 1000.0).round() as u64)
}
fn push_unique(list: &mut Vec<String>, item: String) {
    if !item.is_empty() && !list.contains(&item) {
        list.push(item);
    }
}
fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for b in input.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(char::from(b"0123456789ABCDEF"[(b >> 4) as usize]));
            out.push(char::from(b"0123456789ABCDEF"[(b & 0x0f) as usize]));
        }
    }
    out
}
