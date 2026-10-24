//! Установка и снятие реакции через Discord REST API.

use std::fmt;
use std::future::Future;
use std::pin::Pin;

use discord_core::rest::client::{ApiAuth, ApiClient, ApiConfig, ApiRetryPolicy};
use discord_core::rest::identity;
use discord_core::rest::identity::{IdentityConfig, SystemSources};
use discord_core::rest::transport::HttpResponse;
use discord_core::API_V9_BASE;

use crate::rest_failure::RestFailure;

/// Ограничение длины emoji до percent-encoding (достаточно для unicode/custom emoji).
pub const MAX_EMOJI_CHARS: usize = 256;

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

/// REST реализация реакций Discord.
pub struct DiscordReactionApi {
    client: ApiClient,
}

impl fmt::Debug for DiscordReactionApi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DiscordReactionApi").finish_non_exhaustive()
    }
}

impl DiscordReactionApi {
    pub fn new(token: &str) -> Result<Self, ReactionError> {
        let identity = identity::build(&SystemSources, &IdentityConfig::default());
        let auth = ApiAuth::from_token(token, &identity, "en-US", "UTC")
            .map_err(|_| ReactionError::InvalidArg)?;
        let config = ApiConfig {
            base: API_V9_BASE.to_string(),
            auth,
            http: discord_core::rest::transport::HttpClientConfig::default(),
            retry: ApiRetryPolicy::default(),
        };
        let client = ApiClient::new(config).map_err(|_| ReactionError::Network)?;
        Ok(Self { client })
    }

    async fn toggle(
        &self,
        channel_id: &str,
        message_id: &str,
        emoji: &str,
        active: bool,
    ) -> Result<(), ReactionError> {
        validate(channel_id, message_id, emoji)?;
        let path = format!(
            "/channels/{channel_id}/messages/{message_id}/reactions/{}/@me",
            percent_encode_emoji(emoji)
        );
        let response = if active {
            self.client
                .put(&path, b"{}")
                .await
                .map_err(|error| ReactionError::from(RestFailure::from_api_error(&error)))?
        } else {
            self.client
                .delete(&path)
                .await
                .map_err(|error| ReactionError::from(RestFailure::from_api_error(&error)))?
        };
        decode_response(response)
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

fn validate(channel_id: &str, message_id: &str, emoji: &str) -> Result<(), ReactionError> {
    if !is_numeric_id(channel_id)
        || !is_numeric_id(message_id)
        || emoji.is_empty()
        || emoji.chars().count() > MAX_EMOJI_CHARS
    {
        return Err(ReactionError::InvalidArg);
    }
    Ok(())
}

fn is_numeric_id(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
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
}
