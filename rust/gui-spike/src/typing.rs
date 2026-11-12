//! P-03 / D-108: исходящий typing — `POST /channels/{id}/typing`.
//!
//! Контракт `[S]` (G6-REPORT §3.6, `NETWORK-CONTRACTS` §1.6): тело запроса отсутствует,
//! успех `200`, ответ `{message_send_cooldown_ms?, thread_create_cooldown_ms?}`; оба поля
//! опциональны, поэтому пустое тело — не ошибка. TTL и клиентский троттлинг (≤5 каналов,
//! повтор не чаще интервала) — забота UI, backend их не хардкодит.
//!
//! Сеть — только через [`TypingTransport`]; тесты подменяют транспорт фейком и проверяют
//! target/body. Токен живёт только в auth-заголовке [`ApiClient`] и не попадает в
//! ошибки/`Debug`.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chord_core::rest::client::{ApiAuth, ApiClient, ApiConfig, ApiRetryPolicy};
use chord_core::rest::routes::{self, HttpMethod};
use chord_core::rest::transport::{HttpClientConfig, HttpResponse};
use chord_core::API_V9_BASE;
use serde::{Deserialize, Serialize};

use crate::rest_failure::RestFailure;

/// Ошибка исходящего typing. Ни один исход не паникует и не несёт токен/тело.
///
/// `Storage` — аддитивный внутренний код (SEC-02/G1-02), а не догадка о контракте
/// Discord: отказ keyring/blocking не выдаётся за сеть и не маскируется под `Unauthorized`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TypingError {
    /// `channel_id` не снежинка (пустой, не цифры, длиннее u64).
    InvalidArg,
    /// 401: сессия истекла.
    Unauthorized,
    /// 403: нет права писать в канал.
    Forbidden,
    /// 404: канал не найден.
    NotFound,
    /// 429: ограничение частоты.
    RateLimited {
        /// Задержка из ответа; 0 — если тело не разбиралось.
        retry_after_ms: u64,
    },
    /// Прочие 4xx (кроме 401/403/404/429).
    Client {
        /// HTTP-статус.
        status: u16,
    },
    /// 5xx: ошибка сервера Discord.
    Server {
        /// HTTP-статус.
        status: u16,
    },
    /// Транспорт/таймаут/неподтверждённый статус — операция не выполнена.
    Network,
    /// SEC-02: keyring/blocking недоступны — это не сеть; причина не утекает.
    Storage,
    /// 2xx, кроме подтверждённого `[S]` 200, или неразобранное тело ответа.
    InvalidResponse,
}

impl fmt::Display for TypingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArg => formatter.write_str("некорректный id канала"),
            Self::Unauthorized => formatter.write_str("сессия истекла"),
            Self::Forbidden => formatter.write_str("нет доступа к каналу"),
            Self::NotFound => formatter.write_str("канал не найден"),
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
            Self::InvalidResponse => formatter.write_str("неожиданный ответ typing"),
        }
    }
}

impl std::error::Error for TypingError {}

impl From<RestFailure> for TypingError {
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

/// Опциональные кулдауны из ответа `200` (событие `SLOWMODE_SET_COOLDOWN`).
///
/// Wire — snake_case (§1.6), IPC — camelCase (G6 §3.6): `rename` задаёт IPC-имя,
/// `alias` принимает wire-ключ. `None` не сериализуется, чтобы UI получал только
/// реально пришедшие поля.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypingCooldowns {
    /// `message_send_cooldown_ms` → `messageSendCooldownMs`.
    #[serde(
        rename = "messageSendCooldownMs",
        alias = "message_send_cooldown_ms",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub message_send_cooldown_ms: Option<u64>,
    /// `thread_create_cooldown_ms` → `threadCreateCooldownMs`.
    #[serde(
        rename = "threadCreateCooldownMs",
        alias = "thread_create_cooldown_ms",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub thread_create_cooldown_ms: Option<u64>,
}

/// Доменный контракт typing: удобен для фейков в тестах и Tauri-слоя.
pub trait TypingApi: Send + Sync {
    /// Отправить «печатает»; `None` — сервер не прислал кулдаунов (пустое тело/`{}`).
    fn send_typing<'a>(
        &'a self,
        channel_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<TypingCooldowns>, TypingError>> + Send + 'a>>;
}

/// Транспорт typing: POST без тела. Фейк в тестах фиксирует target и body.
pub trait TypingTransport: Send + Sync {
    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, TypingError>> + Send + 'a>>;
}

struct ApiTypingTransport(ApiClient);

impl TypingTransport for ApiTypingTransport {
    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, TypingError>> + Send + 'a>> {
        Box::pin(async move {
            match self.0.post(target, body).await {
                Ok(mut resp) => {
                    if resp.status == 204 {
                        resp.status = 200;
                        resp.body.clear();
                    }
                    Ok(resp)
                }
                Err(error) => Err(TypingError::from(RestFailure::from_api_error(&error))),
            }
        })
    }
}

/// REST-реализация typing через ядро.
pub struct DiscordTyping {
    transport: Arc<dyn TypingTransport>,
}

impl fmt::Debug for DiscordTyping {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DiscordTyping")
            .finish_non_exhaustive()
    }
}

impl DiscordTyping {
    /// Клиент с боевым транспортом; токен — только в auth-заголовке.
    pub fn new(token: &str) -> Result<Self, TypingError> {
        if token.trim().is_empty() {
            return Err(TypingError::Unauthorized);
        }
        let identity_ctx = crate::auth::IdentityContext::detect();
        let auth = ApiAuth::from_token(
            token,
            &identity_ctx.super_properties,
            &identity_ctx.locale,
            &identity_ctx.timezone,
        )
        .map_err(|_| TypingError::InvalidArg)?;
        let client = ApiClient::new(ApiConfig {
            base: API_V9_BASE.to_string(),
            auth,
            http: HttpClientConfig::default(),
            // Typing — fire-and-forget: UI сам троттлит отправку, повтор запроса
            // не нужен (и не должен задерживать индикатор).
            retry: ApiRetryPolicy {
                max_attempts: 1,
                ..ApiRetryPolicy::default()
            },
        })
        .map_err(|_| TypingError::Network)?;
        Ok(Self {
            transport: Arc::new(ApiTypingTransport(client)),
        })
    }

    #[doc(hidden)]
    pub fn with_transport(transport: Arc<dyn TypingTransport>) -> Self {
        Self { transport }
    }

    async fn execute(&self, channel_id: &str) -> Result<Option<TypingCooldowns>, TypingError> {
        validate_channel_id(channel_id)?;
        let route =
            routes::find(HttpMethod::Post, routes::TYPING_PATH).ok_or(TypingError::Network)?;
        let path =
            routes::render_path(route, &[channel_id]).map_err(|_| TypingError::InvalidArg)?;
        // Тело отсутствует по контракту §1.6: пустой срез, а не `{}`.
        let response = self.transport.post(&path, &[]).await?;
        decode(response)
    }
}

impl TypingApi for DiscordTyping {
    fn send_typing<'a>(
        &'a self,
        channel_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<TypingCooldowns>, TypingError>> + Send + 'a>>
    {
        Box::pin(self.execute(channel_id))
    }
}

/// Сообщить серверу, что пользователь печатает; валидация — до обращения к сети.
pub async fn send_typing<A: TypingApi + ?Sized>(
    api: &A,
    channel_id: &str,
) -> Result<Option<TypingCooldowns>, TypingError> {
    validate_channel_id(channel_id)?;
    api.send_typing(channel_id).await
}

/// Снежинка Discord: только цифры и влезает в `u64` (≤20 цифр).
fn is_snowflake(channel_id: &str) -> bool {
    !channel_id.is_empty()
        && channel_id.bytes().all(|byte| byte.is_ascii_digit())
        && channel_id.parse::<u64>().is_ok()
}

fn validate_channel_id(channel_id: &str) -> Result<(), TypingError> {
    if !is_snowflake(channel_id) {
        return Err(TypingError::InvalidArg);
    }
    Ok(())
}

/// Статус → типизированный результат. Контракт `[S]` подтверждает ровно `200`;
/// иной 2xx — нарушение ответа, а не успех.
fn decode(response: HttpResponse) -> Result<Option<TypingCooldowns>, TypingError> {
    match response.status {
        200 => parse_body(&response.body),
        status if (200..300).contains(&status) => Err(TypingError::InvalidResponse),
        status => {
            Err(RestFailure::from_status(status).map_or(TypingError::Network, TypingError::from))
        }
    }
}

/// Пустое (в т.ч. whitespace) тело — `None`: оба поля ответа опциональны.
/// Непустое тело обязано быть JSON-объектом: serde иначе принял бы массив как
/// позиционную структуру и вернул бы «пустые» кулдауны из ниоткуда.
fn parse_body(body: &[u8]) -> Result<Option<TypingCooldowns>, TypingError> {
    if body.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Ok(None);
    }
    let value: serde_json::Value =
        serde_json::from_slice(body).map_err(|_| TypingError::InvalidResponse)?;
    if !value.is_object() {
        return Err(TypingError::InvalidResponse);
    }
    serde_json::from_value(value)
        .map(Some)
        .map_err(|_| TypingError::InvalidResponse)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(status: u16, body: &[u8]) -> HttpResponse {
        HttpResponse {
            status,
            headers: Vec::new(),
            body: body.to_vec(),
        }
    }

    #[test]
    fn decode_accepts_200_and_classifies_the_rest() {
        assert_eq!(decode(response(200, b"")), Ok(None));
        assert_eq!(
            decode(response(200, br#"{"message_send_cooldown_ms":5}"#)),
            Ok(Some(TypingCooldowns {
                message_send_cooldown_ms: Some(5),
                thread_create_cooldown_ms: None,
            }))
        );
        for (status, expected) in [
            (204, TypingError::InvalidResponse),
            (401, TypingError::Unauthorized),
            (403, TypingError::Forbidden),
            (404, TypingError::NotFound),
            (429, TypingError::RateLimited { retry_after_ms: 0 }),
            (400, TypingError::Client { status: 400 }),
            (500, TypingError::Server { status: 500 }),
            (503, TypingError::Server { status: 503 }),
        ] {
            assert_eq!(
                decode(response(status, b"")),
                Err(expected),
                "HTTP {status}"
            );
        }
    }

    #[test]
    fn api_error_is_classified_by_the_shared_module() {
        use chord_core::rest::client::ApiError;
        let error: TypingError =
            TypingError::from(RestFailure::from_api_error(&ApiError::Client {
                status: 403,
            }));
        assert_eq!(error, TypingError::Forbidden);
        let error: TypingError =
            TypingError::from(RestFailure::from_api_error(&ApiError::Server {
                status: 502,
                attempts: 1,
            }));
        assert_eq!(error, TypingError::Server { status: 502 });
    }
}
