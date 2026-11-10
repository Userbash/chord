//! Отправка сообщений через REST ядра. Содержимое и токены не попадают в ошибки/Debug.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use discord_core::rest::client::{ApiAuth, ApiClient, ApiConfig, ApiRetryPolicy};
use discord_core::rest::transport::{HttpClientConfig, HttpResponse};
use discord_core::rest::routes;
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

fn deserialize_nonce<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        _ => Ok(String::new()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
pub struct MessageReceipt {
    pub id: String,
    pub channel_id: String,
    pub content: String,
    #[serde(default, deserialize_with = "deserialize_nonce")]
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
        let identity = crate::auth::IdentityContext::detect();
        let auth = ApiAuth::from_token(
            token,
            &identity.super_properties,
            &identity.locale,
            &identity.timezone,
        )
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

    pub async fn send_with_attachments(
        &self,
        channel_id: &str,
        content: &str,
        nonce: &str,
        attachments: &[crate::attachments::UploadedAttachment],
    ) -> Result<MessageReceipt, MessageError> {
        validate_channel_id(channel_id)?;
        if (content.trim().is_empty() && attachments.is_empty())
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
        let payload = if attachments.is_empty() {
            serde_json::json!({
                "content": content,
                "nonce": nonce,
                "tts": false,
            })
        } else {
            serde_json::json!({
                "content": content,
                "nonce": nonce,
                "tts": false,
                "attachments": attachments,
            })
        };
        let body = serde_json::to_vec(&payload).map_err(|_| MessageError::InvalidArg)?;
        match self.client.post_once(&path, &body, &[]).await {
            Ok(response) => {
                if (200..300).contains(&response.status) {
                    crate::data::ingest_live_sent_message_json(&response.body);
                }
                decode_receipt(response)
            }
            Err(error) => Err(MessageError::from(RestFailure::from_api_error(&error))),
        }
    }

    async fn send(
        &self,
        channel_id: &str,
        content: &str,
        nonce: &str,
    ) -> Result<MessageReceipt, MessageError> {
        self.send_with_attachments(channel_id, content, nonce, &[])
            .await
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
            let res = self
                .action(
                    discord_core::rest::routes::HttpMethod::Patch,
                    &path,
                    Some(body),
                )
                .await;
            if res.is_ok() {
                crate::data::update_edited_message(channel_id, message_id, content);
            }
            res
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
            eprintln!("[IPC-DELETE] Sending DELETE to Discord: {path}");
            let res = self
                .action(discord_core::rest::routes::HttpMethod::Delete, &path, None)
                .await;
            eprintln!("[IPC-DELETE] Response for {path}: {res:?}");
            if res.is_ok() {
                crate::data::remove_deleted_message(channel_id, message_id);
            }
            res
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

// ---------------------------------------------------------------------------
// task-180 (BE-P2-12): bulk ack и необязательные поля ack.
// Оракул `/home/sanya/re-deep/rest-static.json`:
//   `BULK_ACK`    — POST `/read-states/ack-bulk`, bodyKeys `["{read_states}"]`;
//   `MESSAGE_ACK` — bodyKeys `["{manual,mention_count}", "{token,last_viewed,flags}"]`.
// Живой клиент (`web.4d4b2f75fdea5470.js`): `ep.splice(0,100)` →
// `body:{read_states:[{channel_id,message_id,read_state_type}]}`; auto-ack —
// `{token,last_viewed,flags}` (`flags` опускается, если не изменился);
// mark-unread — `{manual,mention_count}`.
// ---------------------------------------------------------------------------

/// Bulk-подтверждение прочитанного (`Endpoints.BULK_ACK`).
pub const ACK_BULK_PATH: &str = "/read-states/ack-bulk";

/// Предел пачки bulk ack: живой клиент режет очередь `splice(0,100)`.
pub const MAX_ACK_BULK_ENTRIES: usize = 100;

/// Элемент пачки `{read_states:[...]}`: `read_state_type` не отправляется,
/// когда тип не задан (оракул шлёт `undefined` → ключа в JSON нет).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AckBulkEntry {
    pub channel_id: String,
    pub message_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_state_type: Option<u8>,
}

/// Необязательные поля `MESSAGE_ACK`. Базовые `token`/`last_viewed` остаются
/// в теле всегда (`null` при `None`); остальные ключи появляются только при
/// `Some(...)` — ровно как в живом клиенте.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AckMessageOptions {
    pub token: Option<String>,
    pub last_viewed: Option<u64>,
    pub flags: Option<u32>,
    pub manual: Option<bool>,
    pub mention_count: Option<u32>,
}

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
            match self.0.post_once(target, body, &[]).await {
                Ok(mut resp) => {
                    if matches!(resp.status, 400 | 401 | 403 | 404) {
                        resp.status = 204;
                        resp.body.clear();
                    }
                    Ok(resp)
                }
                Err(error) => {
                    let mapped = MessageError::from(RestFailure::from_api_error(&error));
                    if matches!(
                        mapped,
                        MessageError::Forbidden
                            | MessageError::NotFound
                            | MessageError::Client { .. }
                    ) {
                        return Ok(HttpResponse {
                            status: 204,
                            headers: Vec::new(),
                            body: Vec::new(),
                        });
                    }
                    Err(mapped)
                }
            }
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
        let identity_ctx = crate::auth::IdentityContext::detect();
        let auth = ApiAuth::from_token(
            token,
            &identity_ctx.super_properties,
            &identity_ctx.locale,
            &identity_ctx.timezone,
        )
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

    /// task-180: ack с необязательными `flags`/`manual`/`mention_count`.
    /// Базовые ключи оракула — `token` (nullable read-state токен) и
    /// `last_viewed`; optionals сериализуются только при `Some(...)`.
    pub async fn ack_message_with_options(
        &self,
        channel_id: &str,
        message_id: &str,
        options: AckMessageOptions,
    ) -> Result<(), MessageError> {
        validate_ids(&[channel_id, message_id])?;
        let spec = routes::find(routes::HttpMethod::Post, ACK_PATH).ok_or(MessageError::Network)?;
        let path = routes::render_path(spec, &[channel_id, message_id])
            .map_err(|_| MessageError::InvalidArg)?;
        let body = ack_options_body(&options)?;
        let response = self.transport.post_ack(&path, &body).await?;
        decode_action(response)
    }

    /// task-180: bulk ack одной пачкой `1..=MAX_ACK_BULK_ENTRIES`
    /// (пустая или слишком большая пачка отвергается до транспорта).
    pub async fn ack_messages_bulk(&self, entries: &[AckBulkEntry]) -> Result<(), MessageError> {
        validate_bulk_entries(entries)?;
        let body = bulk_ack_body(entries)?;
        let response = self.transport.post_ack(ACK_BULK_PATH, &body).await?;
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

/// Тело ack с опциями: базовые `token`/`last_viewed` всегда (None → `null`),
/// `flags`/`manual`/`mention_count` — только когда заданы. `mention_count: 0`
/// остаётся в теле: ноль упоминаний — валидное значение.
fn ack_options_body(options: &AckMessageOptions) -> Result<Vec<u8>, MessageError> {
    let mut body = serde_json::Map::new();
    body.insert(
        "token".to_string(),
        options
            .token
            .as_ref()
            .map_or(serde_json::Value::Null, |token| {
                serde_json::Value::String(token.clone())
            }),
    );
    body.insert(
        "last_viewed".to_string(),
        options
            .last_viewed
            .map_or(serde_json::Value::Null, serde_json::Value::from),
    );
    if let Some(flags) = options.flags {
        body.insert("flags".to_string(), serde_json::Value::from(flags));
    }
    if let Some(manual) = options.manual {
        body.insert("manual".to_string(), serde_json::Value::from(manual));
    }
    if let Some(mention_count) = options.mention_count {
        body.insert(
            "mention_count".to_string(),
            serde_json::Value::from(mention_count),
        );
    }
    serde_json::to_vec(&serde_json::Value::Object(body)).map_err(|_| MessageError::InvalidArg)
}

/// Тело bulk ack — ровно `{read_states:[...]}` (оракул `BULK_ACK`).
#[derive(serde::Serialize)]
struct BulkAckBody<'a> {
    read_states: &'a [AckBulkEntry],
}

fn bulk_ack_body(entries: &[AckBulkEntry]) -> Result<Vec<u8>, MessageError> {
    serde_json::to_vec(&BulkAckBody {
        read_states: entries,
    })
    .map_err(|_| MessageError::InvalidArg)
}

/// `1..=MAX_ACK_BULK_ENTRIES` и ASCII-цифровые id в каждом элементе — до I/O.
fn validate_bulk_entries(entries: &[AckBulkEntry]) -> Result<(), MessageError> {
    if entries.is_empty() || entries.len() > MAX_ACK_BULK_ENTRIES {
        return Err(MessageError::InvalidArg);
    }
    for entry in entries {
        validate_ids(&[entry.channel_id.as_str(), entry.message_id.as_str()])?;
    }
    Ok(())
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

/// PLAT-11-B: `channel_id` подставляется в URL-путь (`/channels/{id}/messages`),
/// поэтому допускается только allowlist: ASCII-цифры/буквы, `_`, `-`, 1..=64 байт.
/// Снежинки проходят как цифры, фикстуры UI — как `news`/`c1`. Точки, слэши,
/// `%`, `?`, `#`, NUL, CR/LF и прочие непечатаемые отвергаются до I/O.
const MAX_CHANNEL_ID_BYTES: usize = 64;

fn validate_channel_id(channel_id: &str) -> Result<(), MessageError> {
    let bytes = channel_id.as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_CHANNEL_ID_BYTES {
        return Err(MessageError::InvalidArg);
    }
    if !bytes
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_' || *byte == b'-')
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
    validate_channel_id(channel_id)?;
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

/// task-180: free-helper над [`DiscordReadState::ack_message_with_options`].
pub async fn ack_message_with_options(
    api: &DiscordReadState,
    channel_id: &str,
    message_id: &str,
    options: AckMessageOptions,
) -> Result<(), MessageError> {
    api.ack_message_with_options(channel_id, message_id, options)
        .await
}

/// task-180: free-helper над [`DiscordReadState::ack_messages_bulk`].
pub async fn ack_messages_bulk(
    api: &DiscordReadState,
    entries: &[AckBulkEntry],
) -> Result<(), MessageError> {
    api.ack_messages_bulk(entries).await
}

fn decode_receipt(response: HttpResponse) -> Result<MessageReceipt, MessageError> {
    if let Some(failure) = RestFailure::from_status(response.status) {
        return Err(failure.into());
    }
    if let Ok(full_msg) = serde_json::from_slice::<serde_json::Value>(&response.body) {
        if let Some(cid) = full_msg.get("channel_id").and_then(serde_json::Value::as_str) {
            crate::data::record_sent_message(cid, full_msg.clone());
        }
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
