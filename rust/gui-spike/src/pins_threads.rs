//! RW-40 / M-10 & M-11: закреплённые сообщения (`pins`) и ветки канала (`threads`).
//!
//! Контракт:
//! - `list_channel_pins(&transport, channel_id)` -> `GET /channels/{channel_id}/pins` -> `Vec<PinnedMessage>`
//! - `pin_channel_message(&transport, channel_id, message_id)` -> `PUT /channels/{channel_id}/pins/{message_id}` (204 No Content / 200 OK)
//! - `unpin_channel_message(&transport, channel_id, message_id)` -> `DELETE /channels/{channel_id}/pins/{message_id}` (204 No Content / 200 OK)
//! - `list_channel_threads(&transport, channel_id, archived)` ->
//!   `GET /channels/{channel_id}/threads/active` (при `archived == false`) или
//!   `GET /channels/{channel_id}/threads/archived/public` (при `archived == true`) -> `Vec<ThreadSummary>`
//! - предсетевая валидация `channel_id` и `message_id` по allowlist (`[A-Za-z0-9_-]`, `1..=64` байта,
//!   в паритете с `messages::send_message`) — отказ [`PinsThreadsError::InvalidChannelId`] /
//!   [`PinsThreadsError::InvalidMessageId`] до любого вызова транспорта.
//! - маппинг HTTP `429` с извлечением `retry-after` в миллисекунды (`RateLimited { retry_after_ms }`),
//!   `401 Unauthorized`, `403 Forbidden`, `404 NotFound`, `4xx Client`, `5xx Server`.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use discord_core::rest::client::{ApiAuth, ApiClient, ApiConfig, ApiRetryPolicy};
use discord_core::rest::transport::{HttpClientConfig, HttpResponse};
use discord_core::API_V9_BASE;
use serde::{Deserialize, Serialize};

use crate::rest_failure::RestFailure;

/// Максимальная длина `channel_id` и `message_id` в байтах (паритет с `messages::send_message`).
pub const MAX_ID_BYTES: usize = 64;

/// M-10: закреплённое сообщение в канале (`GET /channels/{channel_id}/pins`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinnedMessage {
    pub id: String,
    #[serde(alias = "channel_id")]
    pub channel_id: String,
    pub author: String,
    pub content: String,
    pub timestamp: String,
    pub pinned: bool,
}

/// M-11: сводка ветки канала (`GET /channels/{channel_id}/threads/active` или `/threads/archived/public`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSummary {
    pub id: String,
    #[serde(alias = "parent_id")]
    pub parent_id: String,
    pub name: String,
    #[serde(alias = "message_count")]
    pub message_count: u64,
    #[serde(alias = "member_count")]
    pub member_count: u64,
    pub archived: bool,
    pub locked: bool,
    #[serde(
        alias = "last_message_id",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub last_message_id: Option<String>,
}

/// Типизированная ошибка операций с пинами (M-10) и ветками (M-11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PinsThreadsError {
    /// Некорректный `channel_id` (не прошёл allowlist `[A-Za-z0-9_-]`, 1..=64 байта).
    InvalidChannelId,
    /// Некорректный `message_id` (не прошёл allowlist `[A-Za-z0-9_-]`, 1..=64 байта).
    InvalidMessageId,
    /// Отсутствует или пустой токен авторизации.
    MissingToken,
    /// 401: сессия истекла.
    Unauthorized,
    /// 403: нет доступа к каналу или права закреплять/управлять сообщениями.
    Forbidden,
    /// 404: канал или сообщение не найдены.
    NotFound,
    /// 429: ограничение частоты запросов; задержка `retry-after` в миллисекундах.
    RateLimited { retry_after_ms: u64 },
    /// Прочие 4xx (кроме 401/403/404/429).
    Client { status: u16 },
    /// 5xx: ошибка сервера Discord.
    Server { status: u16 },
    /// Транспорт/таймаут — сеть недоступна.
    Network,
    /// Отказ хранилища токена (SEC-02).
    Storage,
    /// Ответ 2xx не соответствует ожидаемой JSON-схеме.
    InvalidResponse,
}

impl fmt::Display for PinsThreadsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidChannelId => formatter.write_str("некорректный идентификатор канала"),
            Self::InvalidMessageId => formatter.write_str("некорректный идентификатор сообщения"),
            Self::MissingToken => formatter.write_str("токен не найден"),
            Self::Unauthorized => formatter.write_str("сессия истекла"),
            Self::Forbidden => formatter.write_str("нет доступа к каналу"),
            Self::NotFound => formatter.write_str("канал или сообщение не найдены"),
            Self::RateLimited { retry_after_ms } => {
                write!(
                    formatter,
                    "ограничение частоты; повтор через {retry_after_ms} мс"
                )
            }
            Self::Client { status } => write!(formatter, "запрос отклонён (HTTP {status})"),
            Self::Server { status } => {
                write!(formatter, "ошибка сервера Discord (HTTP {status})")
            }
            Self::Network => formatter.write_str("сеть недоступна"),
            Self::Storage => formatter.write_str("хранилище токена недоступно"),
            Self::InvalidResponse => formatter.write_str("некорректный ответ пинов или веток"),
        }
    }
}

impl std::error::Error for PinsThreadsError {}

impl From<RestFailure> for PinsThreadsError {
    fn from(failure: RestFailure) -> Self {
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

/// Транспорт для операций с пинами (M-10) и ветками (M-11).
pub trait PinsThreadsTransport: Send + Sync {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, PinsThreadsError>> + Send + 'a>>;

    fn put<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, PinsThreadsError>> + Send + 'a>>;

    fn delete<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, PinsThreadsError>> + Send + 'a>>;
}

struct ApiPinsThreadsTransport(ApiClient);

impl PinsThreadsTransport for ApiPinsThreadsTransport {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, PinsThreadsError>> + Send + 'a>> {
        Box::pin(async move {
            self.0
                .get(target)
                .await
                .map_err(|error| PinsThreadsError::from(RestFailure::from_api_error(&error)))
        })
    }

    fn put<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, PinsThreadsError>> + Send + 'a>> {
        Box::pin(async move {
            self.0
                .put(target, body)
                .await
                .map_err(|error| PinsThreadsError::from(RestFailure::from_api_error(&error)))
        })
    }

    fn delete<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, PinsThreadsError>> + Send + 'a>> {
        Box::pin(async move {
            self.0
                .delete(target)
                .await
                .map_err(|error| PinsThreadsError::from(RestFailure::from_api_error(&error)))
        })
    }
}

/// REST-клиент пинов и веток канала.
pub struct DiscordPinsThreads {
    transport: Arc<dyn PinsThreadsTransport>,
}

impl fmt::Debug for DiscordPinsThreads {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DiscordPinsThreads")
            .finish_non_exhaustive()
    }
}

impl DiscordPinsThreads {
    /// Создаёт клиент с боевым транспортом ядра; токен уходит только в auth-заголовок.
    pub fn new(token: &str) -> Result<Self, PinsThreadsError> {
        if token.trim().is_empty() {
            return Err(PinsThreadsError::MissingToken);
        }
        let identity_ctx = crate::auth::IdentityContext::detect();
        let auth = ApiAuth::from_token(
            token,
            &identity_ctx.super_properties,
            &identity_ctx.locale,
            &identity_ctx.timezone,
        )
        .map_err(|_| PinsThreadsError::MissingToken)?;
        let client = ApiClient::new(ApiConfig {
            base: API_V9_BASE.to_string(),
            auth,
            http: HttpClientConfig::default(),
            retry: ApiRetryPolicy {
                max_attempts: 1,
                ..ApiRetryPolicy::default()
            },
        })
        .map_err(|_| PinsThreadsError::Network)?;
        Ok(Self {
            transport: Arc::new(ApiPinsThreadsTransport(client)),
        })
    }

    #[doc(hidden)]
    pub fn with_transport(transport: Arc<dyn PinsThreadsTransport>) -> Self {
        Self { transport }
    }

    pub async fn list_channel_pins(
        &self,
        channel_id: &str,
    ) -> Result<Vec<PinnedMessage>, PinsThreadsError> {
        list_channel_pins(self, channel_id).await
    }

    pub async fn pin_channel_message(
        &self,
        channel_id: &str,
        message_id: &str,
    ) -> Result<(), PinsThreadsError> {
        pin_channel_message(self, channel_id, message_id).await
    }

    pub async fn unpin_channel_message(
        &self,
        channel_id: &str,
        message_id: &str,
    ) -> Result<(), PinsThreadsError> {
        unpin_channel_message(self, channel_id, message_id).await
    }

    pub async fn list_channel_threads(
        &self,
        channel_id: &str,
        archived: bool,
    ) -> Result<Vec<ThreadSummary>, PinsThreadsError> {
        list_channel_threads(self, channel_id, archived).await
    }
}

impl PinsThreadsTransport for DiscordPinsThreads {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, PinsThreadsError>> + Send + 'a>> {
        self.transport.get(target)
    }

    fn put<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, PinsThreadsError>> + Send + 'a>> {
        self.transport.put(target, body)
    }

    fn delete<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, PinsThreadsError>> + Send + 'a>> {
        self.transport.delete(target)
    }
}

/// Загрузить список закреплённых сообщений канала (`GET /channels/{channel_id}/pins`).
pub async fn list_channel_pins<T: PinsThreadsTransport + ?Sized>(
    transport: &T,
    channel_id: &str,
) -> Result<Vec<PinnedMessage>, PinsThreadsError> {
    validate_channel_id(channel_id)?;
    let target = format!("/channels/{channel_id}/pins");
    let response = transport.get(&target).await?;
    ensure_2xx(&response)?;
    decode_pins(&response.body)
}

/// Закрепить сообщение в канале (`PUT /channels/{channel_id}/pins/{message_id}`).
pub async fn pin_channel_message<T: PinsThreadsTransport + ?Sized>(
    transport: &T,
    channel_id: &str,
    message_id: &str,
) -> Result<(), PinsThreadsError> {
    validate_channel_id(channel_id)?;
    validate_message_id(message_id)?;
    let target = format!("/channels/{channel_id}/pins/{message_id}");
    let response = transport.put(&target, &[]).await?;
    decode_pin_action(&response)
}

/// Открепить сообщение в канале (`DELETE /channels/{channel_id}/pins/{message_id}`).
pub async fn unpin_channel_message<T: PinsThreadsTransport + ?Sized>(
    transport: &T,
    channel_id: &str,
    message_id: &str,
) -> Result<(), PinsThreadsError> {
    validate_channel_id(channel_id)?;
    validate_message_id(message_id)?;
    let target = format!("/channels/{channel_id}/pins/{message_id}");
    let response = transport.delete(&target).await?;
    decode_pin_action(&response)
}

/// Загрузить список веток канала:
/// - `archived == false` -> `GET /channels/{channel_id}/threads/active`
/// - `archived == true` -> `GET /channels/{channel_id}/threads/archived/public`
pub async fn list_channel_threads<T: PinsThreadsTransport + ?Sized>(
    transport: &T,
    channel_id: &str,
    archived: bool,
) -> Result<Vec<ThreadSummary>, PinsThreadsError> {
    validate_channel_id(channel_id)?;
    let target = if archived {
        format!("/channels/{channel_id}/threads/archived/public")
    } else {
        format!("/channels/{channel_id}/threads/active")
    };
    let response = transport.get(&target).await?;
    ensure_2xx(&response)?;
    decode_threads(&response.body)
}

fn is_safe_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= MAX_ID_BYTES
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_' || *byte == b'-')
}

fn validate_channel_id(channel_id: &str) -> Result<(), PinsThreadsError> {
    if !is_safe_id(channel_id) {
        return Err(PinsThreadsError::InvalidChannelId);
    }
    Ok(())
}

fn validate_message_id(message_id: &str) -> Result<(), PinsThreadsError> {
    if !is_safe_id(message_id) {
        return Err(PinsThreadsError::InvalidMessageId);
    }
    Ok(())
}

fn ensure_2xx(response: &HttpResponse) -> Result<(), PinsThreadsError> {
    if response.status == 429 {
        return Err(PinsThreadsError::RateLimited {
            retry_after_ms: parse_retry_after_ms(response),
        });
    }
    match response.status {
        200..=299 => Ok(()),
        other => Err(RestFailure::from_status(other)
            .map_or(PinsThreadsError::Network, PinsThreadsError::from)),
    }
}

fn decode_pin_action(response: &HttpResponse) -> Result<(), PinsThreadsError> {
    if response.status == 429 {
        return Err(PinsThreadsError::RateLimited {
            retry_after_ms: parse_retry_after_ms(response),
        });
    }
    match response.status {
        200 | 204 => Ok(()),
        status if (200..300).contains(&status) => Err(PinsThreadsError::InvalidResponse),
        other => Err(RestFailure::from_status(other)
            .map_or(PinsThreadsError::Network, PinsThreadsError::from)),
    }
}

/// Извлекает задержку `retry-after` из заголовка `Retry-After` (секунды -> мс)
/// либо из JSON-тела `{"retry_after": <seconds>}`; если не указана — `0`.
fn parse_retry_after_ms(response: &HttpResponse) -> u64 {
    if let Some(ms) = response
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("retry-after"))
        .and_then(|(_, value)| value.trim().parse::<f64>().ok())
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
        .map(|seconds| (seconds * 1000.0).round() as u64)
    {
        return ms;
    }
    serde_json::from_slice::<serde_json::Value>(&response.body)
        .ok()
        .and_then(|val| val.get("retry_after").and_then(serde_json::Value::as_f64))
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
        .map_or(0, |seconds| (seconds * 1000.0).round() as u64)
}

#[derive(Deserialize)]
#[serde(untagged)]
enum WireAuthor {
    Name(String),
    User {
        #[serde(default)]
        global_name: Option<String>,
        #[serde(default)]
        username: Option<String>,
    },
}

impl WireAuthor {
    fn into_string(self) -> Option<String> {
        match self {
            Self::Name(name) if !name.trim().is_empty() => Some(name),
            Self::Name(_) => None,
            Self::User {
                global_name,
                username,
            } => global_name
                .filter(|s| !s.trim().is_empty())
                .or_else(|| username.filter(|s| !s.trim().is_empty())),
        }
    }
}

#[derive(Deserialize)]
struct WirePin {
    id: String,
    #[serde(alias = "channelId")]
    channel_id: String,
    author: WireAuthor,
    #[serde(default)]
    content: String,
    timestamp: String,
    #[serde(default = "default_true")]
    pinned: bool,
}

const fn default_true() -> bool {
    true
}

fn decode_pins(body: &[u8]) -> Result<Vec<PinnedMessage>, PinsThreadsError> {
    if body.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Err(PinsThreadsError::InvalidResponse);
    }
    let raw: Vec<WirePin> =
        serde_json::from_slice(body).map_err(|_| PinsThreadsError::InvalidResponse)?;
    let mut pins = Vec::with_capacity(raw.len());
    for item in raw {
        if item.id.trim().is_empty()
            || item.channel_id.trim().is_empty()
            || item.timestamp.trim().is_empty()
        {
            return Err(PinsThreadsError::InvalidResponse);
        }
        let author = item
            .author
            .into_string()
            .ok_or(PinsThreadsError::InvalidResponse)?;
        pins.push(PinnedMessage {
            id: item.id,
            channel_id: item.channel_id,
            author,
            content: item.content,
            timestamp: item.timestamp,
            pinned: item.pinned,
        });
    }
    Ok(pins)
}

#[derive(Deserialize)]
struct WireThreadMetadata {
    #[serde(default)]
    archived: bool,
    #[serde(default)]
    locked: bool,
}

#[derive(Deserialize)]
struct WireThread {
    id: String,
    #[serde(alias = "parentId")]
    parent_id: String,
    name: String,
    #[serde(alias = "messageCount", default)]
    message_count: u64,
    #[serde(alias = "memberCount", default)]
    member_count: u64,
    #[serde(default)]
    archived: Option<bool>,
    #[serde(default)]
    locked: Option<bool>,
    #[serde(default)]
    thread_metadata: Option<WireThreadMetadata>,
    #[serde(alias = "lastMessageId", default)]
    last_message_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum WireThreadsPayload {
    Wrapped { threads: Vec<WireThread> },
    List(Vec<WireThread>),
}

fn decode_threads(body: &[u8]) -> Result<Vec<ThreadSummary>, PinsThreadsError> {
    if body.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Err(PinsThreadsError::InvalidResponse);
    }
    let payload: WireThreadsPayload =
        serde_json::from_slice(body).map_err(|_| PinsThreadsError::InvalidResponse)?;
    let list = match payload {
        WireThreadsPayload::Wrapped { threads } | WireThreadsPayload::List(threads) => threads,
    };
    let mut result = Vec::with_capacity(list.len());
    for item in list {
        if item.id.trim().is_empty()
            || item.parent_id.trim().is_empty()
            || item.name.trim().is_empty()
        {
            return Err(PinsThreadsError::InvalidResponse);
        }
        let meta = item.thread_metadata.as_ref();
        let archived = item
            .archived
            .or_else(|| meta.map(|m| m.archived))
            .unwrap_or(false);
        let locked = item
            .locked
            .or_else(|| meta.map(|m| m.locked))
            .unwrap_or(false);
        let last_message_id = item.last_message_id.filter(|s| !s.trim().is_empty());
        result.push(ThreadSummary {
            id: item.id,
            parent_id: item.parent_id,
            name: item.name,
            message_count: item.message_count,
            member_count: item.member_count,
            archived,
            locked,
            last_message_id,
        });
    }
    Ok(result)
}
