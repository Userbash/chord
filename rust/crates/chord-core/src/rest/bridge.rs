//! Transparent HTTP/REST bridge (`DiscordHttpClient`, `DiscordRestBridge`) powered by
//! `reqwest`, `url::Url`, and typed `serde` DTOs (LIB-SIMPLIFY-01).
//!
//! Preserves all Discord REST API v9 invariants:
//! - 6 required auth headers (`Authorization`, `X-Super-Properties`, `X-Discord-Locale`,
//!   `X-Discord-Timezone`, `X-Debug-Options`, `Accept-Language`) + optional headers +
//!   `BrowserImpersonationProfile`;
//! - telemetry blocking (`guard_target` before any network request);
//! - Snowflake validation (`1..=u64::MAX`, ASCII digits only, non-zero);
//! - URL & RFC 3986 query string resolution via `url::Url` + `percent_encoding`;
//! - Concurrency control via `tokio::sync::Semaphore`, `X-RateLimit-Bucket` tracking, and
//!   `429 Too Many Requests` / `Retry-After` + idempotent `5xx` exponential backoff.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::Semaphore;
use url::Url;

use super::client::{
    ApiAuth, ApiConfig, ApiError, ApiRetryPolicy, BrowserImpersonationProfile, HeaderConfig,
};
use super::http1::DEFAULT_USER_AGENT;
use super::rate_limit::{
    RateLimitInfo, RestFailure, classify_failure, rate_limit_info, retry_delay, should_retry,
};
use super::routes::HttpMethod;
use super::telemetry::guard_target;
use super::transport::{HttpClientConfig, HttpError, HttpResponse};
use crate::API_V9_BASE;

/// Maximum concurrent in-flight REST requests per [`DiscordHttpClient`] pool.
pub const DEFAULT_MAX_CONCURRENT_REQUESTS: usize = 16;

/// Keep-alive idle timeout for the `reqwest` connection pool (`90` seconds).
pub const DEFAULT_POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(90);

/// RFC 3986 unreserved character set for query parameter percent-encoding.
const RFC3986_UNRESERVED: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

/// Tracked state of a Discord `X-RateLimit-Bucket`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitBucketState {
    /// Bucket identifier from `X-RateLimit-Bucket`.
    pub bucket: String,
    /// Remaining requests in the current window (`X-RateLimit-Remaining`).
    pub remaining: Option<u32>,
    /// Total requests allowed in the bucket (`X-RateLimit-Limit`).
    pub limit: Option<u32>,
    /// Reset delay reported by the server (`X-RateLimit-Reset-After`).
    pub reset_after: Option<Duration>,
    /// Scope (`user`, `global`, `shared`) if reported (`X-RateLimit-Scope`).
    pub scope: Option<String>,
    /// Monotonic timestamp when this bucket state was last updated.
    pub updated_at: Instant,
}

/// Typed error returned by [`DiscordHttpClient`] / [`DiscordRestBridge`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscordRestBridgeError {
    /// Invalid Snowflake ID (`1..=u64::MAX`, ASCII digits only).
    InvalidSnowflake {
        /// Field name that failed validation.
        field: String,
        /// Invalid value provided.
        value: String,
    },
    /// Invalid URL or query parameter.
    InvalidUrl(String),
    /// Serde JSON serialization or deserialization failure.
    Serialization(String),
    /// Underlying REST API error (401, 429, 4xx, 5xx, timeout, telemetry block).
    Api(ApiError),
}

impl fmt::Display for DiscordRestBridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSnowflake { field, value } => {
                write!(f, "некорректный Snowflake ID в поле {field}: {value:?}")
            }
            Self::InvalidUrl(msg) => write!(f, "некорректный URL: {msg}"),
            Self::Serialization(msg) => write!(f, "ошибка сериализации JSON: {msg}"),
            Self::Api(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for DiscordRestBridgeError {}

impl From<ApiError> for DiscordRestBridgeError {
    fn from(err: ApiError) -> Self {
        Self::Api(err)
    }
}

/// Validates that `value` is a non-empty positive 64-bit Discord Snowflake ID
/// (`1..=u64::MAX`, ASCII digits only).
pub fn validate_snowflake_id(field: &str, value: &str) -> Result<u64, DiscordRestBridgeError> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(DiscordRestBridgeError::InvalidSnowflake {
            field: field.to_string(),
            value: value.to_string(),
        });
    }
    match value.parse::<u64>() {
        Ok(n) if n > 0 => Ok(n),
        _ => Err(DiscordRestBridgeError::InvalidSnowflake {
            field: field.to_string(),
            value: value.to_string(),
        }),
    }
}

/// Encodes a query parameter key or value according to RFC 3986 unreserved rules
/// using `percent_encoding`.
#[must_use]
pub fn encode_rfc3986_query_value(value: &str) -> String {
    percent_encoding::utf8_percent_encode(value, RFC3986_UNRESERVED).to_string()
}

/// Builds a relative route + query string using `url::Url` validation and RFC 3986 percent-encoding.
pub fn build_relative_url_with_query(
    route_path: &str,
    query_pairs: &[(&str, String)],
) -> Result<String, DiscordRestBridgeError> {
    let base = Url::parse(API_V9_BASE)
        .map_err(|e| DiscordRestBridgeError::InvalidUrl(format!("base url: {e}")))?;
    let resolved = resolve_url_against_base(&base, route_path, query_pairs)?;
    Ok(DiscordHttpClient::relative_path_and_query(&resolved))
}

fn resolve_url_against_base<K: AsRef<str>, V: AsRef<str>>(
    base_url: &Url,
    route_path: &str,
    query_pairs: &[(K, V)],
) -> Result<Url, DiscordRestBridgeError> {
    if !route_path.starts_with('/') || route_path.contains('#') {
        return Err(DiscordRestBridgeError::InvalidUrl(route_path.to_string()));
    }

    let (raw_path, existing_query) = match route_path.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (route_path, None),
    };

    if raw_path
        .bytes()
        .any(|byte| !(0x21..=0x7e).contains(&byte) || byte == b'#')
    {
        return Err(DiscordRestBridgeError::InvalidUrl(route_path.to_string()));
    }

    let base_str = base_url.as_str().trim_end_matches('/');
    let full_raw = format!("{base_str}{raw_path}");
    let mut url = Url::parse(&full_raw)
        .map_err(|e| DiscordRestBridgeError::InvalidUrl(format!("{full_raw}: {e}")))?;

    let mut encoded_pairs: Vec<String> = Vec::new();
    if let Some(eq) = existing_query
        && !eq.is_empty()
    {
        encoded_pairs.push(eq.to_string());
    }
    for (k, v) in query_pairs {
        let key_enc = encode_rfc3986_query_value(k.as_ref());
        let val_enc = encode_rfc3986_query_value(v.as_ref());
        encoded_pairs.push(format!("{key_enc}={val_enc}"));
    }

    if encoded_pairs.is_empty() {
        url.set_query(None);
    } else {
        url.set_query(Some(&encoded_pairs.join("&")));
    }

    Ok(url)
}

/// Reusable connection-pooled HTTP client bridging `chord-core` and `reqwest` + `url::Url` + `serde`.
#[derive(Clone)]
pub struct DiscordHttpClient {
    client: reqwest::Client,
    base_url: Url,
    auth: ApiAuth,
    http_config: HttpClientConfig,
    retry: ApiRetryPolicy,
    headers: HeaderConfig,
    browser_impersonation: Option<BrowserImpersonationProfile>,
    semaphore: Arc<Semaphore>,
    buckets: Arc<Mutex<HashMap<String, RateLimitBucketState>>>,
}

impl fmt::Debug for DiscordHttpClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DiscordHttpClient")
            .field("base_url", &self.base_url.as_str())
            .field("auth", &self.auth)
            .field("retry", &self.retry)
            .field("headers", &self.headers)
            .finish_non_exhaustive()
    }
}

impl DiscordHttpClient {
    /// Creates a new `DiscordHttpClient` with a pooled `reqwest::Client`, `tokio::sync::Semaphore`
    /// concurrency limiter, and `X-RateLimit-Bucket` tracker.
    pub fn new(config: ApiConfig) -> Result<Self, ApiError> {
        Self::with_concurrency(config, DEFAULT_MAX_CONCURRENT_REQUESTS)
    }

    /// Creates a new `DiscordHttpClient` with an explicit maximum concurrency limit.
    pub fn with_concurrency(config: ApiConfig, max_concurrency: usize) -> Result<Self, ApiError> {
        let parsed_url = Url::parse(&config.base)
            .map_err(|e| ApiError::InvalidBase(format!("{}: {e}", config.base)))?;
        let scheme = parsed_url.scheme();
        if scheme != "http" && scheme != "https" {
            return Err(ApiError::InvalidBase(format!(
                "unsupported scheme: {scheme}"
            )));
        }
        if parsed_url.host_str().is_none()
            || !parsed_url.username().is_empty()
            || parsed_url.password().is_some()
            || parsed_url.query().is_some()
            || parsed_url.fragment().is_some()
        {
            return Err(ApiError::InvalidBase(config.base));
        }

        let client = reqwest::Client::builder()
            .connect_timeout(config.http.connect_timeout)
            .pool_idle_timeout(DEFAULT_POOL_IDLE_TIMEOUT)
            .redirect_policy(reqwest::redirect::Policy::none())
            .user_agent(DEFAULT_USER_AGENT)
            .build()
            .map_err(|e| ApiError::Transport(HttpError::Connect(e.to_string())))?;

        Ok(Self {
            client,
            base_url: parsed_url,
            auth: config.auth,
            http_config: config.http,
            retry: config.retry,
            headers: HeaderConfig::default(),
            browser_impersonation: Some(BrowserImpersonationProfile::desktop_default()),
            semaphore: Arc::new(Semaphore::new(max_concurrency.max(1))),
            buckets: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Creates an anonymous `DiscordHttpClient` for the given base URL.
    pub fn with_anonymous_base(base: &str) -> Result<Self, ApiError> {
        Self::new(ApiConfig {
            base: base.to_string(),
            ..ApiConfig::default()
        })
    }

    /// Configures optional Discord headers (`X-Fingerprint`, `X-Installation-ID`, `X-Routing-Key`).
    #[must_use]
    pub fn with_headers(mut self, headers: HeaderConfig) -> Self {
        self.headers = headers;
        self
    }

    /// Returns the base [`Url`] of this client.
    #[must_use]
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// Returns the underlying pooled [`reqwest::Client`].
    #[must_use]
    pub fn reqwest_client(&self) -> &reqwest::Client {
        &self.client
    }

    /// Returns the tracked state of a rate-limit bucket by its `X-RateLimit-Bucket` ID.
    #[must_use]
    pub fn bucket_state(&self, bucket_id: &str) -> Option<RateLimitBucketState> {
        self.buckets
            .lock()
            .ok()
            .and_then(|guard| guard.get(bucket_id).cloned())
    }

    /// Resolves a relative API route path (e.g. `/users/123/profile`) and query parameters
    /// against the client's base URL using `url::Url`.
    pub fn resolve_url<K: AsRef<str>, V: AsRef<str>>(
        &self,
        route_path: &str,
        query_pairs: &[(K, V)],
    ) -> Result<Url, DiscordRestBridgeError> {
        guard_target(route_path).map_err(ApiError::from)?;
        resolve_url_against_base(&self.base_url, route_path, query_pairs)
    }

    /// Extracts the relative route path (stripping `/api/v9` prefix if present) and query string
    /// from a resolved [`Url`].
    #[must_use]
    pub fn relative_path_and_query(url: &Url) -> String {
        let path = url
            .path()
            .strip_prefix("/api/v9")
            .unwrap_or_else(|| url.path());
        let normalized_path = if path.is_empty() { "/" } else { path };
        match url.query() {
            Some(q) if !q.is_empty() => format!("{normalized_path}?{q}"),
            _ => normalized_path.to_string(),
        }
    }

    /// Builds a signed [`reqwest::Request`] with all configured Discord headers and optional JSON body.
    pub fn build_reqwest_request_with_body(
        &self,
        method: HttpMethod,
        route_path: &str,
        query_pairs: &[(&str, &str)],
        body: Option<Vec<u8>>,
    ) -> Result<reqwest::Request, DiscordRestBridgeError> {
        let url = self.resolve_url(route_path, query_pairs)?;
        let reqwest_method = to_reqwest_method(method);
        let mut builder = self.client.request(reqwest_method, url);

        for (name, value) in self.auth.pairs() {
            if !value.is_empty() {
                builder = builder.header(name, value);
            }
        }
        for (name, value) in self.headers.extra_headers() {
            builder = builder.header(name, value);
        }
        if let Some(ref profile) = self.browser_impersonation {
            for (name, value) in profile.header_pairs() {
                if !value.trim().is_empty() {
                    builder = builder.header(name, value);
                }
            }
        }
        if let Some(bytes) = body
            && !bytes.is_empty()
        {
            builder = builder
                .header("Content-Type", "application/json")
                .body(bytes);
        }

        builder
            .build()
            .map_err(|e| DiscordRestBridgeError::InvalidUrl(e.to_string()))
    }

    /// Executes an HTTP request through the `reqwest` connection pool with semaphore rate-limit gating,
    /// `X-RateLimit-Bucket` header tracking, and Discord `429 Retry-After` / idempotent `5xx` retries.
    pub async fn execute_raw(
        &self,
        method: HttpMethod,
        route_path: &str,
        query_pairs: &[(&str, &str)],
        body: Option<&[u8]>,
    ) -> Result<HttpResponse, DiscordRestBridgeError> {
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| ApiError::Transport(HttpError::Io("semaphore closed".to_string())))?;

        let max_attempts = self.retry.max_attempts.max(1);
        let idempotent = matches!(
            method,
            HttpMethod::Get | HttpMethod::Put | HttpMethod::Delete
        );
        let mut attempt = 0u32;

        loop {
            attempt += 1;
            let req = self.build_reqwest_request_with_body(
                method,
                route_path,
                query_pairs,
                body.map(<[u8]>::to_vec),
            )?;

            let send_fut = async {
                let resp = self.client.execute(req).await.map_err(map_reqwest_error)?;
                let status = resp.status().as_u16();
                let mut headers = Vec::with_capacity(resp.headers().len());
                for (name, val) in resp.headers() {
                    if let Ok(v_str) = val.to_str() {
                        headers.push((name.as_str().to_string(), v_str.to_string()));
                    }
                }
                let bytes = resp.bytes().await.map_err(map_reqwest_error)?;
                if bytes.len() > self.http_config.max_body_bytes {
                    return Err(HttpError::TooLarge(format!(
                        "body {} > {}",
                        bytes.len(),
                        self.http_config.max_body_bytes
                    )));
                }
                Ok(HttpResponse {
                    status,
                    headers,
                    body: bytes.to_vec(),
                })
            };

            let result = tokio::time::timeout(self.retry.attempt_timeout, send_fut).await;
            let response = match result {
                Err(_elapsed) => {
                    if idempotent && attempt < max_attempts {
                        self.sleep_backoff(attempt).await;
                        continue;
                    }
                    return Err(ApiError::Timeout { attempts: attempt }.into());
                }
                Ok(Err(http_err)) => {
                    if idempotent && attempt < max_attempts && is_retryable_http_error(&http_err) {
                        self.sleep_backoff(attempt).await;
                        continue;
                    }
                    return Err(ApiError::from(http_err).into());
                }
                Ok(Ok(resp)) => resp,
            };

            self.update_bucket_from_headers(&response.headers, &response.body, response.status);

            let failure = match classify_failure(response.status, &response.headers, &response.body)
            {
                None => return Ok(response),
                Some(failure) => failure,
            };

            if should_retry(method, &failure)
                && attempt < max_attempts
                && let Some(delay) = retry_delay(
                    attempt,
                    &failure,
                    self.retry.backoff,
                    self.retry.max_retry_after,
                )
            {
                tokio::time::sleep(delay).await;
                continue;
            }

            return Err(match failure {
                RestFailure::RateLimited(info) => ApiError::RateLimited {
                    retry_after: info.retry_after,
                    attempts: attempt,
                }
                .into(),
                RestFailure::Server { status } => ApiError::Server {
                    status,
                    attempts: attempt,
                }
                .into(),
                RestFailure::Unauthorized => ApiError::Unauthorized.into(),
                RestFailure::Client { status } => ApiError::Client { status }.into(),
            });
        }
    }

    /// Executes a typed JSON request using `serde` serialization for the request body and
    /// `serde` deserialization for the response payload.
    pub async fn execute_json<Req, Resp>(
        &self,
        method: HttpMethod,
        route_path: &str,
        query_pairs: &[(&str, &str)],
        body: Option<&Req>,
    ) -> Result<Resp, DiscordRestBridgeError>
    where
        Req: Serialize + ?Sized,
        Resp: DeserializeOwned,
    {
        let serialized_body = match body {
            Some(payload) => Some(
                serde_json::to_vec(payload)
                    .map_err(|e| DiscordRestBridgeError::Serialization(e.to_string()))?,
            ),
            None => None,
        };

        let response = self
            .execute_raw(method, route_path, query_pairs, serialized_body.as_deref())
            .await?;

        serde_json::from_slice(&response.body)
            .map_err(|e| DiscordRestBridgeError::Serialization(e.to_string()))
    }

    fn update_bucket_from_headers(&self, headers: &[(String, String)], body: &[u8], status: u16) {
        let info = rate_limit_info(429, headers, body).unwrap_or_else(|| RateLimitInfo {
            retry_after: Duration::ZERO,
            global: false,
            remaining: None,
            reset_after: None,
            bucket: None,
            limit: None,
            scope: None,
        });

        let _ = status;
        if let Some(bucket_id) = info.bucket
            && let Ok(mut guard) = self.buckets.lock()
        {
            guard.insert(
                bucket_id.clone(),
                RateLimitBucketState {
                    bucket: bucket_id,
                    remaining: info.remaining,
                    limit: info.limit,
                    reset_after: info.reset_after,
                    scope: info.scope,
                    updated_at: Instant::now(),
                },
            );
        }
    }

    async fn sleep_backoff(&self, attempt: u32) {
        let exponent = attempt.saturating_sub(1);
        let factor = 1u32.checked_shl(exponent).unwrap_or(u32::MAX);
        let delay = self.retry.backoff.saturating_mul(factor);
        tokio::time::sleep(delay.min(self.retry.max_retry_after)).await;
    }
}

/// Alias for [`DiscordHttpClient`] acting as the transparent bridge between IPC commands and Discord REST v9.
pub type DiscordRestBridge = DiscordHttpClient;

fn to_reqwest_method(method: HttpMethod) -> reqwest::Method {
    match method {
        HttpMethod::Get => reqwest::Method::GET,
        HttpMethod::Post => reqwest::Method::POST,
        HttpMethod::Put => reqwest::Method::PUT,
        HttpMethod::Patch => reqwest::Method::PATCH,
        HttpMethod::Delete => reqwest::Method::DELETE,
    }
}

fn map_reqwest_error(err: reqwest::Error) -> HttpError {
    if err.is_timeout() {
        HttpError::Timeout(err.to_string())
    } else if err.is_connect() {
        HttpError::Connect(err.to_string())
    } else {
        HttpError::Io(err.to_string())
    }
}

fn is_retryable_http_error(error: &HttpError) -> bool {
    matches!(
        error,
        HttpError::Connect(_) | HttpError::Io(_) | HttpError::Timeout(_)
    )
}
