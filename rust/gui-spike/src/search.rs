//! Серверный поиск сообщений в гильдии через подтверждённый REST-маршрут.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use discord_core::rest::client::{ApiAuth, ApiClient, ApiConfig, ApiRetryPolicy};
use discord_core::rest::identity::{IdentityConfig, SystemSources};
use discord_core::rest::routes::{self, HttpMethod};
use discord_core::rest::transport::{HttpClientConfig, HttpResponse};
use discord_core::API_V9_BASE;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::rest_failure::RestFailure;

/// Параметры GET `/guilds/{id}/messages/search`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchParams {
    pub content: Option<String>,
    pub channel_id: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    /// Отрицательные значения отвергаются до обращения к сети.
    pub offset: Option<i64>,
}

/// Подтверждённый ответ серверного поиска.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub messages: Vec<Value>,
    #[serde(rename = "total_results")]
    pub total_results: u64,
    #[serde(rename = "doing_deep_historical_index")]
    pub doing_deep_historical_index: bool,
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

/// Реализация поиска через `discord-core` REST-клиент.
pub struct DiscordSearch {
    transport: Arc<dyn SearchTransport>,
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
        let identity =
            discord_core::rest::identity::build(&SystemSources, &IdentityConfig::default());
        let auth = ApiAuth::from_token(token, &identity, "en-US", "UTC")
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
        })
    }

    #[doc(hidden)]
    pub fn with_transport(transport: Arc<dyn SearchTransport>) -> Self {
        Self { transport }
    }

    async fn execute(
        &self,
        guild_id: &str,
        params: &SearchParams,
    ) -> Result<SearchResponse, SearchError> {
        if guild_id.trim().is_empty() || guild_id.contains(['{', '}', '\r', '\n']) {
            return Err(SearchError::InvalidArg);
        }
        if params.offset.is_some_and(|offset| offset < 0) {
            return Err(SearchError::InvalidArg);
        }
        let route = routes::find(HttpMethod::Get, "/guilds/{id}/messages/search")
            .ok_or(SearchError::Network)?;
        let encoded_guild = percent_encode(guild_id);
        let path =
            routes::render_path(route, &[&encoded_guild]).map_err(|_| SearchError::InvalidArg)?;
        let mut query = Vec::new();
        for (name, value) in [
            ("content", params.content.as_deref()),
            ("channel_id", params.channel_id.as_deref()),
            ("sort_by", params.sort_by.as_deref()),
            ("sort_order", params.sort_order.as_deref()),
        ] {
            if let Some(value) = value {
                query.push(format!("{name}={}", percent_encode(value)));
            }
        }
        if let Some(offset) = params.offset {
            query.push(format!("offset={offset}"));
        }
        let target = if query.is_empty() {
            path
        } else {
            format!("{path}?{}", query.join("&"))
        };
        let response = self.transport.get(&target).await?;
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
