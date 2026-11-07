//! Установка и снятие реакции через Discord REST API.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use discord_core::rest::client::{ApiAuth, ApiClient, ApiConfig, ApiRetryPolicy};
use discord_core::rest::transport::HttpResponse;
use discord_core::API_V9_BASE;

use crate::rest_failure::RestFailure;

/// Ограничение длины emoji до percent-encoding (достаточно для unicode/custom emoji).
pub const MAX_EMOJI_CHARS: usize = 256;

/// Тело `PUT /channels/{id}/messages/{id}/reactions/{emoji}/@me` — 0 байт.
///
/// Оракул: `RE/rest-d02.json` endpoint 38 (`PUT .../reactions/%F0%9F%91%8B/%40me`):
/// `reqBodyKeys: []`, `reqBodyRequests: 0`, `requestsWithContentType: 0`,
/// `statuses: [204]`. Клиент добавляет `Content-Type: application/json` только
/// для непустого тела, поэтому с этим телом заголовка нет.
pub const REACTION_PUT_EMPTY_BODY: &[u8] = b"";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ReactionError {
    InvalidArg,
    Unauthorized,
    /// 403: нет права ставить реакцию в канале.
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

impl fmt::Display for ReactionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArg => f.write_str("некорректные параметры реакции"),
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
            Self::InvalidResponse => f.write_str("некорректный ответ реакции"),
        }
    }
}

impl std::error::Error for ReactionError {}

impl From<crate::rest_failure::RestFailure> for ReactionError {
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

/// Низкоуровневый HTTP-транспорт для операций с реакциями (`PUT` / `DELETE .../@me`).
pub trait ReactionTransport: Send + Sync {
    fn put<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, ReactionError>> + Send + 'a>>;

    fn delete<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, ReactionError>> + Send + 'a>>;

    /// `PUT` с пустым телом (0 байт), как в эталонном клиенте для реакций.
    ///
    /// По умолчанию делегирует в [`ReactionTransport::put`] с
    /// [`REACTION_PUT_EMPTY_BODY`]; реализации транспорта могут переопределить
    /// метод, чтобы не собирать тело вовсе.
    fn put_empty_body<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, ReactionError>> + Send + 'a>> {
        self.put(target, REACTION_PUT_EMPTY_BODY)
    }
}

/// Абстракция реакции для Tauri слоя и тестов без сети.
pub trait ReactionApi: Send + Sync {
    fn toggle_reaction<'a>(
        &'a self,
        channel_id: &'a str,
        message_id: &'a str,
        emoji: &'a str,
        active: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(), ReactionError>> + Send + 'a>>;
}

struct ApiReactionTransport(ApiClient);

impl ReactionTransport for ApiReactionTransport {
    fn put<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, ReactionError>> + Send + 'a>> {
        Box::pin(async move {
            self.0
                .put(target, body)
                .await
                .map_err(|error| ReactionError::from(RestFailure::from_api_error(&error)))
        })
    }

    fn delete<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, ReactionError>> + Send + 'a>> {
        Box::pin(async move {
            self.0
                .delete(target)
                .await
                .map_err(|error| ReactionError::from(RestFailure::from_api_error(&error)))
        })
    }
}

/// REST реализация реакций Discord.
pub struct DiscordReactionApi {
    transport: Arc<dyn ReactionTransport>,
}

impl fmt::Debug for DiscordReactionApi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DiscordReactionApi").finish_non_exhaustive()
    }
}

impl DiscordReactionApi {
    pub fn new(token: &str) -> Result<Self, ReactionError> {
        let identity_ctx = crate::auth::IdentityContext::detect();
        let auth = ApiAuth::from_token(
            token,
            &identity_ctx.super_properties,
            &identity_ctx.locale,
            &identity_ctx.timezone,
        )
        .map_err(|_| ReactionError::InvalidArg)?;
        let config = ApiConfig {
            base: API_V9_BASE.to_string(),
            auth,
            http: discord_core::rest::transport::HttpClientConfig::default(),
            retry: ApiRetryPolicy::default(),
        };
        let client = ApiClient::new(config).map_err(|_| ReactionError::Network)?;
        Ok(Self {
            transport: Arc::new(ApiReactionTransport(client)),
        })
    }

    #[doc(hidden)]
    pub fn with_transport(transport: Arc<dyn ReactionTransport>) -> Self {
        Self { transport }
    }

    pub async fn add_reaction(
        &self,
        channel_id: &str,
        message_id: &str,
        emoji: &str,
    ) -> Result<(), ReactionError> {
        self.toggle(channel_id, message_id, emoji, true).await
    }

    /// Идемпотентная установка реакции **без тела запроса** (0 байт).
    ///
    /// Соответствует эталонному клиенту: `PUT .../reactions/{emoji}/@me` с
    /// `reqBodyKeys: []` и ответом `204 No Content` (`RE/rest-d02.json` endpoint 38).
    /// Существующий [`DiscordReactionApi::add_reaction`] продолжает слать `{}` —
    /// обратная совместимость сохраняется.
    pub async fn add_reaction_empty_body(
        &self,
        channel_id: &str,
        message_id: &str,
        emoji: &str,
    ) -> Result<(), ReactionError> {
        let path = build_reaction_path(channel_id, message_id, emoji)?;
        let response = self.transport.put_empty_body(&path).await?;
        decode_response(response)
    }

    pub async fn remove_reaction(
        &self,
        channel_id: &str,
        message_id: &str,
        emoji: &str,
    ) -> Result<(), ReactionError> {
        self.toggle(channel_id, message_id, emoji, false).await
    }

    pub async fn toggle_from_active(
        &self,
        channel_id: &str,
        message_id: &str,
        emoji: &str,
        active: bool,
    ) -> Result<bool, ReactionError> {
        toggle_reaction_from_active(self, channel_id, message_id, emoji, active).await
    }

    pub async fn flip_reaction(
        &self,
        channel_id: &str,
        message_id: &str,
        emoji: &str,
        active: bool,
    ) -> Result<bool, ReactionError> {
        self.toggle_from_active(channel_id, message_id, emoji, active)
            .await
    }

    async fn toggle(
        &self,
        channel_id: &str,
        message_id: &str,
        emoji: &str,
        active: bool,
    ) -> Result<(), ReactionError> {
        let path = build_reaction_path(channel_id, message_id, emoji)?;
        let response = if active {
            self.transport.put_empty_body(&path).await?
        } else {
            self.transport.delete(&path).await?
        };
        decode_response(response)
    }
}

impl ReactionTransport for DiscordReactionApi {
    fn put<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, ReactionError>> + Send + 'a>> {
        self.transport.put(target, body)
    }

    fn delete<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, ReactionError>> + Send + 'a>> {
        self.transport.delete(target)
    }
}

impl ReactionApi for DiscordReactionApi {
    fn toggle_reaction<'a>(
        &'a self,
        channel_id: &'a str,
        message_id: &'a str,
        emoji: &'a str,
        active: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(), ReactionError>> + Send + 'a>> {
        Box::pin(self.toggle(channel_id, message_id, emoji, active))
    }
}

pub async fn toggle_reaction<A: ReactionApi + ?Sized>(
    api: &A,
    channel_id: &str,
    message_id: &str,
    emoji: &str,
    active: bool,
) -> Result<(), ReactionError> {
    validate(channel_id, message_id, emoji)?;
    api.toggle_reaction(channel_id, message_id, emoji, active)
        .await
}

/// Идемпотентная установка реакции (`PUT /channels/{channel_id}/messages/{message_id}/reactions/{emoji}/@me`, тело `{}`).
pub async fn add_reaction<A: ReactionApi + ?Sized>(
    api: &A,
    channel_id: &str,
    message_id: &str,
    emoji: &str,
) -> Result<(), ReactionError> {
    toggle_reaction(api, channel_id, message_id, emoji, true).await
}

/// Идемпотентное снятие собственной реакции (`DELETE /channels/{channel_id}/messages/{message_id}/reactions/{emoji}/@me`).
pub async fn remove_reaction<A: ReactionApi + ?Sized>(
    api: &A,
    channel_id: &str,
    message_id: &str,
    emoji: &str,
) -> Result<(), ReactionError> {
    toggle_reaction(api, channel_id, message_id, emoji, false).await
}

/// Переключает реакцию от её текущего состояния `active`:
/// - при `active == false` вызывает `add_reaction` (`PUT .../@me`) и возвращает `Ok(true)`;
/// - при `active == true` вызывает `remove_reaction` (`DELETE .../@me`) и возвращает `Ok(false)`.
pub async fn toggle_reaction_from_active<A: ReactionApi + ?Sized>(
    api: &A,
    channel_id: &str,
    message_id: &str,
    emoji: &str,
    active: bool,
) -> Result<bool, ReactionError> {
    let next_active = !active;
    toggle_reaction(api, channel_id, message_id, emoji, next_active).await?;
    Ok(next_active)
}

/// Алиас [`toggle_reaction_from_active`]: инвертирует текущее состояние реакции (`false -> PUT`, `true -> DELETE`).
pub async fn flip_reaction<A: ReactionApi + ?Sized>(
    api: &A,
    channel_id: &str,
    message_id: &str,
    emoji: &str,
    active: bool,
) -> Result<bool, ReactionError> {
    toggle_reaction_from_active(api, channel_id, message_id, emoji, active).await
}

/// Валидирует и кодирует эмодзи (Unicode либо кастомный `name:id` / `a:name:id` / `<:name:id>` / `<a:name:id>`)
/// для сегмента пути `/reactions/{emoji}/@me`.
pub fn encode_reaction_emoji(emoji: &str) -> Result<String, ReactionError> {
    let normalized = normalize_and_validate_emoji(emoji)?;
    Ok(percent_encode_emoji(normalized))
}

/// Валидирует `channel_id`, `message_id` и `emoji` и строит маршрут
/// `/channels/{channel_id}/messages/{message_id}/reactions/{encoded_emoji}/@me`.
pub fn build_reaction_path(
    channel_id: &str,
    message_id: &str,
    emoji: &str,
) -> Result<String, ReactionError> {
    if !is_numeric_id(channel_id) || !is_numeric_id(message_id) {
        return Err(ReactionError::InvalidArg);
    }
    let encoded = encode_reaction_emoji(emoji)?;
    Ok(format!(
        "/channels/{channel_id}/messages/{message_id}/reactions/{encoded}/@me"
    ))
}

/// Извлекает задержку `retry_after` (в секундах float) из JSON-тела `{"retry_after": ...}`
/// или заголовков `Retry-After` / `x-ratelimit-reset-after` и переводит в миллисекунды (`u64`).
/// Для положительных субмиллисекундных значений возвращает минимум `1` мс; при отсутствии поля — `0`.
#[must_use]
pub fn parse_retry_after_ms(response: &HttpResponse) -> u64 {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&response.body) {
        if let Some(seconds) = value.get("retry_after").and_then(|field| {
            field.as_f64().or_else(|| {
                field
                    .as_str()
                    .and_then(|raw| raw.trim().parse::<f64>().ok())
            })
        }) {
            if let Some(ms) = seconds_to_ms(seconds) {
                return ms;
            }
        }
    }
    for header_name in ["retry-after", "x-ratelimit-reset-after"] {
        if let Some(ms) = response
            .headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(header_name))
            .and_then(|(_, value)| value.trim().parse::<f64>().ok())
            .and_then(seconds_to_ms)
        {
            return ms;
        }
    }
    0
}

fn seconds_to_ms(seconds: f64) -> Option<u64> {
    if !seconds.is_finite() || seconds <= 0.0 {
        return None;
    }
    let scaled = (seconds * 1000.0).round();
    if scaled >= u64::MAX as f64 {
        Some(u64::MAX)
    } else {
        Some((scaled as u64).max(1))
    }
}

fn validate(channel_id: &str, message_id: &str, emoji: &str) -> Result<(), ReactionError> {
    if !is_numeric_id(channel_id) || !is_numeric_id(message_id) {
        return Err(ReactionError::InvalidArg);
    }
    normalize_and_validate_emoji(emoji)?;
    Ok(())
}

fn is_numeric_id(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && value.parse::<u64>().is_ok()
}

fn is_custom_emoji_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

fn normalize_and_validate_emoji(emoji: &str) -> Result<&str, ReactionError> {
    if emoji
        .chars()
        .any(|ch| ch.is_control() || ch == '/' || ch == '\\')
    {
        return Err(ReactionError::InvalidArg);
    }
    let trimmed = emoji.trim();
    if trimmed.is_empty()
        || matches!(trimmed, "." | "..")
        || trimmed.chars().count() > MAX_EMOJI_CHARS
    {
        return Err(ReactionError::InvalidArg);
    }
    let candidate = if trimmed.starts_with('<') || trimmed.ends_with('>') {
        if let Some(rest) = trimmed.strip_prefix("<:").and_then(|s| s.strip_suffix('>')) {
            rest
        } else if trimmed
            .strip_prefix("<a:")
            .and_then(|s| s.strip_suffix('>'))
            .is_some()
        {
            // Сохраняем префикс `a:` из `<a:name:id>`
            &trimmed[1..trimmed.len() - 1]
        } else {
            return Err(ReactionError::InvalidArg);
        }
    } else {
        trimmed
    };

    if candidate.contains(':') {
        let parts: Vec<&str> = candidate.split(':').collect();
        match parts.as_slice() {
            [name, id] if is_custom_emoji_name(name) && is_numeric_id(id) => Ok(candidate),
            ["a", name, id] if is_custom_emoji_name(name) && is_numeric_id(id) => Ok(candidate),
            _ => Err(ReactionError::InvalidArg),
        }
    } else if candidate.chars().any(char::is_whitespace) {
        Err(ReactionError::InvalidArg)
    } else {
        Ok(candidate)
    }
}

/// Percent-encode every byte outside RFC 3986 unreserved characters.
pub fn percent_encode_emoji(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(*byte as char);
        } else {
            encoded.push('%');
            encoded.push(HEX[(byte >> 4) as usize] as char);
            encoded.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }
    encoded
}

fn decode_response(response: HttpResponse) -> Result<(), ReactionError> {
    if response.status == 429 {
        return Err(ReactionError::RateLimited {
            retry_after_ms: parse_retry_after_ms(&response),
        });
    }
    RestFailure::from_status(response.status).map_or(Ok(()), |failure| Err(failure.into()))
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
    fn decode_maps_2xx_to_ok_and_statuses_to_typed_errors() {
        for status in [200u16, 204, 299] {
            assert_eq!(decode_response(response(status)), Ok(()), "HTTP {status}");
        }
        for (status, expected) in [
            (401, ReactionError::Unauthorized),
            (403, ReactionError::Forbidden),
            (404, ReactionError::NotFound),
            (429, ReactionError::RateLimited { retry_after_ms: 0 }),
            (400, ReactionError::Client { status: 400 }),
            (500, ReactionError::Server { status: 500 }),
            (503, ReactionError::Server { status: 503 }),
        ] {
            assert_eq!(
                decode_response(response(status)),
                Err(expected),
                "HTTP {status}"
            );
        }
    }

    #[test]
    fn api_error_is_classified_by_the_shared_module() {
        let not_found: ReactionError =
            RestFailure::from_api_error(&ApiError::Client { status: 404 }).into();
        assert_eq!(not_found, ReactionError::NotFound);
        assert_eq!(
            RestFailure::from_api_error(&ApiError::Server {
                status: 500,
                attempts: 1,
            }),
            RestFailure::Server { status: 500 }
        );
    }

    #[test]
    fn decode_429_extracts_retry_after_from_json_and_headers() {
        let with_json = HttpResponse {
            status: 429,
            headers: Vec::new(),
            body: br#"{"retry_after": 1.5}"#.to_vec(),
        };
        assert_eq!(
            decode_response(with_json),
            Err(ReactionError::RateLimited {
                retry_after_ms: 1500
            })
        );

        let with_header = HttpResponse {
            status: 429,
            headers: vec![("Retry-After".to_string(), "2.25".to_string())],
            body: Vec::new(),
        };
        assert_eq!(
            decode_response(with_header),
            Err(ReactionError::RateLimited {
                retry_after_ms: 2250
            })
        );
    }

    #[test]
    fn custom_emoji_and_invalid_inputs_are_validated() {
        assert_eq!(
            encode_reaction_emoji("party:123456"),
            Ok("party%3A123456".to_string())
        );
        assert_eq!(
            encode_reaction_emoji("a:party:123456"),
            Ok("a%3Aparty%3A123456".to_string())
        );
        assert_eq!(encode_reaction_emoji("a/b"), Err(ReactionError::InvalidArg));
        assert_eq!(
            encode_reaction_emoji("👍\n"),
            Err(ReactionError::InvalidArg)
        );
    }
}
