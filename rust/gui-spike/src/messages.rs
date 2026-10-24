//! Отправка сообщений через REST ядра. Содержимое и токены не попадают в ошибки/Debug.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use discord_core::rest::client::{ApiAuth, ApiClient, ApiConfig, ApiRetryPolicy};
use discord_core::rest::identity::{IdentityConfig, SystemSources};
use discord_core::rest::transport::{HttpClientConfig, HttpResponse};
use discord_core::rest::{identity, routes};
use discord_core::API_V9_BASE;
use serde::Deserialize;

use crate::rest_failure::RestFailure;

pub const MAX_MESSAGE_CHARS: usize = 2000;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MessageError {
    InvalidArg,
    Unauthorized,
    /// 403: нет права писать/менять в канале.
    Forbidden,
    /// 404: сообщение или канал не найдены.
    NotFound,
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

impl fmt::Display for MessageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArg => f.write_str("некорректный текст сообщения"),
            Self::Unauthorized => f.write_str("сессия истекла"),
            Self::Forbidden => f.write_str("нет доступа к каналу"),
            Self::NotFound => f.write_str("сообщение или канал не найдены"),
            Self::RateLimited { retry_after_ms } => {
                write!(f, "ограничение частоты; повтор через {retry_after_ms} мс")
            }
            Self::Client { status } => write!(f, "запрос отклонён (HTTP {status})"),
            Self::Server { status } => write!(f, "ошибка сервера Discord (HTTP {status})"),
            Self::Network => f.write_str("сеть недоступна"),
            Self::Storage => f.write_str("хранилище токена недоступно"),
            Self::InvalidResponse => f.write_str("не удалось разобрать ответ отправки сообщения"),
        }
    }
}

impl std::error::Error for MessageError {}

impl From<crate::rest_failure::RestFailure> for MessageError {
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
pub struct MessageReceipt {
    pub id: String,
    pub channel_id: String,
    pub content: String,
    pub nonce: String,
}

pub trait MessageSender: Send + Sync {
    fn send_message<'a>(
        &'a self,
        channel_id: &'a str,
        content: &'a str,
        nonce: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<MessageReceipt, MessageError>> + Send + 'a>>;
}

pub trait MessageActions: Send + Sync {
    fn edit_message<'a>(
        &'a self,
        channel_id: &'a str,
        message_id: &'a str,
        content: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), MessageError>> + Send + 'a>>;
    fn delete_message<'a>(
        &'a self,
        channel_id: &'a str,
        message_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), MessageError>> + Send + 'a>>;
    fn reply_message<'a>(
        &'a self,
        channel_id: &'a str,
        content: &'a str,
        reply_to: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), MessageError>> + Send + 'a>>;
}

pub struct DiscordMessageSender {
    client: ApiClient,
}

impl fmt::Debug for DiscordMessageSender {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DiscordMessageSender")
            .finish_non_exhaustive()
    }
}

impl DiscordMessageSender {
    pub fn new(token: &str) -> Result<Self, MessageError> {
        let identity = identity::build(&SystemSources, &IdentityConfig::default());
        let auth = ApiAuth::from_token(token, &identity, "en-US", "UTC")
            .map_err(|_| MessageError::InvalidArg)?;
        let config = ApiConfig {
            base: API_V9_BASE.to_string(),
            auth,
            http: HttpClientConfig::default(),
            retry: ApiRetryPolicy::default(),
        };
        let client = ApiClient::new(config).map_err(|_| MessageError::Network)?;
        Ok(Self { client })
    }

    async fn send(
        &self,
        channel_id: &str,
        content: &str,
        nonce: &str,
    ) -> Result<MessageReceipt, MessageError> {
        if channel_id.is_empty()
            || channel_id.contains(['{', '}', '\r', '\n'])
            || content.trim().is_empty()
            || content.chars().count() > MAX_MESSAGE_CHARS
        {
            return Err(MessageError::InvalidArg);
        }
        let path = routes::render_path(
            routes::find(routes::HttpMethod::Post, "/channels/{id}/messages")
                .ok_or(MessageError::Network)?,
            &[channel_id],
        )
        .map_err(|_| MessageError::InvalidArg)?;
        let body = serde_json::to_vec(&serde_json::json!({
            "content": content,
            "nonce": nonce,
            "tts": false,
        }))
        .map_err(|_| MessageError::InvalidArg)?;
        let response = self
            .client
            .post_once(&path, &body, &[])
            .await
            .map_err(|error| MessageError::from(RestFailure::from_api_error(&error)))?;
        decode_receipt(response)
    }
}

impl MessageSender for DiscordMessageSender {
    fn send_message<'a>(
        &'a self,
        channel_id: &'a str,
        content: &'a str,
        nonce: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<MessageReceipt, MessageError>> + Send + 'a>> {
        Box::pin(self.send(channel_id, content, nonce))
    }
}

impl DiscordMessageSender {
    async fn action(
        &self,
        method: discord_core::rest::routes::HttpMethod,
        target: &str,
        body: Option<Vec<u8>>,
    ) -> Result<(), MessageError> {
        let response = match method {
            discord_core::rest::routes::HttpMethod::Patch => {
                self.client
                    .patch(target, body.as_deref().unwrap_or_default())
                    .await
            }
            discord_core::rest::routes::HttpMethod::Delete => self.client.delete(target).await,
            _ => return Err(MessageError::Network),
        }
        .map_err(|error| MessageError::from(RestFailure::from_api_error(&error)))?;
        decode_action(response)
    }
}

impl MessageActions for DiscordMessageSender {
    fn edit_message<'a>(
        &'a self,
        channel_id: &'a str,
        message_id: &'a str,
        content: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), MessageError>> + Send + 'a>> {
        Box::pin(async move {
            validate_ids(&[channel_id, message_id])?;
            validate_content(content)?;
            let path = format!("/channels/{channel_id}/messages/{message_id}");
            let body = serde_json::to_vec(&serde_json::json!({"content": content}))
                .map_err(|_| MessageError::InvalidArg)?;
            self.action(
                discord_core::rest::routes::HttpMethod::Patch,
                &path,
                Some(body),
            )
            .await
        })
    }

    fn delete_message<'a>(
        &'a self,
        channel_id: &'a str,
        message_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), MessageError>> + Send + 'a>> {
        Box::pin(async move {
            validate_ids(&[channel_id, message_id])?;
            let path = format!("/channels/{channel_id}/messages/{message_id}");
            self.action(discord_core::rest::routes::HttpMethod::Delete, &path, None)
                .await
        })
    }

    fn reply_message<'a>(
        &'a self,
        channel_id: &'a str,
        content: &'a str,
        reply_to: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), MessageError>> + Send + 'a>> {
        Box::pin(async move {
            validate_ids(&[channel_id, reply_to])?;
            validate_content(content)?;
            let path = format!("/channels/{channel_id}/messages");
            let body = serde_json::to_vec(&serde_json::json!({
                "content": content,
                "message_reference": {"message_id": reply_to, "channel_id": channel_id}
            }))
            .map_err(|_| MessageError::InvalidArg)?;
            let response = self
                .client
                .post(&path, &body)
                .await
                .map_err(|error| MessageError::from(RestFailure::from_api_error(&error)))?;
            decode_action(response)
        })
    }
}

// ---------------------------------------------------------------------------
// M-07: read state / ack. POST `/channels/{id}/messages/{id}/ack`,
// тело ровно `{token, last_viewed}` (оракул: `docs/re/rest-d02.md` §4,
// requestId `ready|57992.7577` — `{"token":null,"last_viewed":4310}`).
// `last_viewed` — счётчик read-state (`MESSAGE_ACK.last_viewed`), не snowflake;
// `token` в теле — nullable read-state токен, а не bearer (bearer живёт только
// в auth-заголовке `ApiClient`).
// ---------------------------------------------------------------------------

const ACK_PATH: &str = "/channels/{id}/messages/{id}/ack";

/// Транспорт ack: POST с JSON-телом; фейк в тестах записывает target и тело.
pub trait AckTransport: Send + Sync {
    fn post_ack<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, MessageError>> + Send + 'a>>;
}

/// Контракт отметки канала прочитанным: `last_viewed` — число read-state,
/// `token` — nullable значение тела (не bearer).
pub trait ReadStateAck: Send + Sync {
    fn ack_message<'a>(
        &'a self,
        channel_id: &'a str,
        message_id: &'a str,
        last_viewed: u64,
        token: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<(), MessageError>> + Send + 'a>>;
}

struct ApiAckTransport(ApiClient);

impl AckTransport for ApiAckTransport {
    fn post_ack<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, MessageError>> + Send + 'a>> {
        Box::pin(async move {
            // Ack — изменяющая операция: как отправка сообщения, без повторов.
            self.0
                .post_once(target, body, &[])
                .await
                .map_err(|error| MessageError::from(RestFailure::from_api_error(&error)))
        })
    }
}

/// Отметка канала прочитанным через REST ядра.
pub struct DiscordReadState {
    transport: Arc<dyn AckTransport>,
}

impl fmt::Debug for DiscordReadState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DiscordReadState").finish_non_exhaustive()
    }
}

impl DiscordReadState {
    /// Клиент с боевым транспортом; `token` уходит только в auth-заголовок.
    pub fn new(token: &str) -> Result<Self, MessageError> {
        let identity = identity::build(&SystemSources, &IdentityConfig::default());
        let auth = ApiAuth::from_token(token, &identity, "en-US", "UTC")
            .map_err(|_| MessageError::InvalidArg)?;
        let config = ApiConfig {
            base: API_V9_BASE.to_string(),
            auth,
            http: HttpClientConfig::default(),
            retry: ApiRetryPolicy::default(),
        };
        let client = ApiClient::new(config).map_err(|_| MessageError::Network)?;
        Ok(Self {
            transport: Arc::new(ApiAckTransport(client)),
        })
    }

    /// Транспорт подменяется в тестах (сеть не нужна).
    #[doc(hidden)]
    pub fn with_transport(transport: Arc<dyn AckTransport>) -> Self {
        Self { transport }
    }

    async fn ack(
        &self,
        channel_id: &str,
        message_id: &str,
        last_viewed: u64,
        token: Option<String>,
    ) -> Result<(), MessageError> {
        validate_ids(&[channel_id, message_id])?;
        let spec = routes::find(routes::HttpMethod::Post, ACK_PATH).ok_or(MessageError::Network)?;
        let path = routes::render_path(spec, &[channel_id, message_id])
            .map_err(|_| MessageError::InvalidArg)?;
        let body = ack_body(last_viewed, token)?;
        let response = self.transport.post_ack(&path, &body).await?;
        decode_action(response)
    }
}

impl ReadStateAck for DiscordReadState {
    fn ack_message<'a>(
        &'a self,
        channel_id: &'a str,
        message_id: &'a str,
        last_viewed: u64,
        token: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<(), MessageError>> + Send + 'a>> {
        Box::pin(self.ack(channel_id, message_id, last_viewed, token))
    }
}

/// Тело ack дословно по оракулу: `{token, last_viewed}`; `None` → `null`.
/// Bearer сюда не попадает: единственный источник `token` — аргумент вызова.
fn ack_body(last_viewed: u64, token: Option<String>) -> Result<Vec<u8>, MessageError> {
    serde_json::to_vec(&serde_json::json!({
        "token": token,
        "last_viewed": last_viewed,
    }))
    .map_err(|_| MessageError::InvalidArg)
}

fn validate_ids(ids: &[&str]) -> Result<(), MessageError> {
    if ids
        .iter()
        .any(|id| id.is_empty() || !id.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err(MessageError::InvalidArg);
    }
    Ok(())
}

fn validate_content(content: &str) -> Result<(), MessageError> {
    if content.trim().is_empty() || content.chars().count() > MAX_MESSAGE_CHARS {
        return Err(MessageError::InvalidArg);
    }
    Ok(())
}

fn decode_action(response: HttpResponse) -> Result<(), MessageError> {
    RestFailure::from_status(response.status).map_or(Ok(()), |failure| Err(failure.into()))
}

pub async fn send_message<S: MessageSender + ?Sized>(
    sender: &S,
    channel_id: &str,
    content: &str,
    nonce: &str,
) -> Result<MessageReceipt, MessageError> {
    if content.trim().is_empty() || content.chars().count() > MAX_MESSAGE_CHARS {
        return Err(MessageError::InvalidArg);
    }
    sender.send_message(channel_id, content, nonce).await
}

pub async fn edit_message<A: MessageActions + ?Sized>(
    actions: &A,
    channel: &str,
    message: &str,
    content: &str,
) -> Result<(), MessageError> {
    validate_ids(&[channel, message])?;
    validate_content(content)?;
    actions.edit_message(channel, message, content).await
}

pub async fn delete_message<A: MessageActions + ?Sized>(
    actions: &A,
    channel: &str,
    message: &str,
) -> Result<(), MessageError> {
    validate_ids(&[channel, message])?;
    actions.delete_message(channel, message).await
}

pub async fn reply_message<A: MessageActions + ?Sized>(
    actions: &A,
    channel: &str,
    content: &str,
    reply_to: &str,
) -> Result<(), MessageError> {
    validate_ids(&[channel, reply_to])?;
    validate_content(content)?;
    actions.reply_message(channel, content, reply_to).await
}

/// Отметка канала прочитанным (M-07); `token` — nullable read-state токен.
pub async fn ack_message<A: ReadStateAck + ?Sized>(
    api: &A,
    channel_id: &str,
    message_id: &str,
    last_viewed: u64,
    token: Option<String>,
) -> Result<(), MessageError> {
    api.ack_message(channel_id, message_id, last_viewed, token)
        .await
}

fn decode_receipt(response: HttpResponse) -> Result<MessageReceipt, MessageError> {
    if let Some(failure) = RestFailure::from_status(response.status) {
        return Err(failure.into());
    }
    serde_json::from_slice(&response.body).map_err(|_| MessageError::InvalidResponse)
}

#[cfg(test)]
mod tests {
    use super::*;
    use discord_core::rest::client::ApiError;

    fn response(status: u16) -> HttpResponse {
        HttpResponse {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    #[test]
    fn decode_actions_map_2xx_to_ok_and_other_statuses_to_typed_errors() {
        for status in [200u16, 204, 299] {
            assert_eq!(decode_action(response(status)), Ok(()), "HTTP {status}");
        }
        for (status, expected) in [
            (401, MessageError::Unauthorized),
            (403, MessageError::Forbidden),
            (404, MessageError::NotFound),
            (429, MessageError::RateLimited { retry_after_ms: 0 }),
            (400, MessageError::Client { status: 400 }),
            (500, MessageError::Server { status: 500 }),
            (503, MessageError::Server { status: 503 }),
        ] {
            assert_eq!(
                decode_action(response(status)),
                Err(expected.clone()),
                "HTTP {status}"
            );
            assert_eq!(
                decode_receipt(response(status)),
                Err(expected),
                "HTTP {status}"
            );
        }
    }

    #[test]
    fn api_error_is_classified_by_the_shared_module() {
        let forbidden: MessageError =
            RestFailure::from_api_error(&ApiError::Client { status: 403 }).into();
        assert_eq!(forbidden, MessageError::Forbidden);
        let server: MessageError = RestFailure::from_api_error(&ApiError::Server {
            status: 503,
            attempts: 2,
        })
        .into();
        assert_eq!(server, MessageError::Server { status: 503 });
        assert_eq!(
            RestFailure::from_api_error(&ApiError::Unauthorized),
            RestFailure::Unauthorized
        );
    }
}
