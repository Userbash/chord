//! Чтение профиля пользователя: `GET /users/{id}/profile`.
//!
//! Оракул — `/home/sanya/re-deep/rest-static.json`, endpoint `USER_PROFILE`
//! (`GET /users/{arg}/profile`, 3 call-site, `confirmed: false`). Живой захват
//! (`cacheEvidence.example`) фиксирует query целиком:
//! `?type=popout&with_mutual_guilds=true&with_mutual_friends=true&with_mutual_friends_count=false&guild_id=1014537851418980372`.
//! Отсюда — [`DEFAULT_POPOUT_QUERY`] и порядок параметров в
//! [`build_user_profile_path`]: `guild_id` идёт последним и только если задан.
//!
//! Тело ответа — `UserProfileResponse`; обязательны только `user.id` и
//! `user.username`, остальные поля и коллекции берут значения по умолчанию
//! (в захвате профиля без mutual-данных этих ключей нет). Пустая идентичность пользователя —
//! [`ProfileError::InvalidResponse`], а не «профиль без имени».
//!
//! Сеть — только через [`ProfileTransport`]; тесты подменяют транспорт фейком.
//! Токен живёт в `ApiClient` и в ошибки/`Debug` не попадает.

use std::fmt;
use std::future::Future;
use std::pin::Pin;

use chord_core::rest::client::{ApiAuth, ApiClient, ApiConfig, ApiRetryPolicy};
use chord_core::rest::transport::{HttpClientConfig, HttpResponse};
use chord_core::API_V9_BASE;

use crate::rest_failure::RestFailure;

/// Шаблон маршрута профиля пользователя (`Endpoints.USER_PROFILE`).
pub const USER_PROFILE_PATH_TEMPLATE: &str = "/users/{id}/profile";

/// Query всплывающего профиля по умолчанию — ровно как в захвате
/// (`cacheEvidence.example`), без `guild_id`.
pub const DEFAULT_POPOUT_QUERY: &str =
    "type=popout&with_mutual_guilds=true&with_mutual_friends=true&with_mutual_friends_count=false";

/// Параметры запроса профиля.
///
/// `Default` повторяет захват: `type=popout`, mutual guilds/friends включены,
/// счётчик друзей — выключен, `guild_id` не задан.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ProfileQuery {
    /// `type`; `None` — параметр не отправляется (сервер применит свой дефолт).
    pub profile_type: Option<String>,
    /// `with_mutual_guilds`.
    pub with_mutual_guilds: bool,
    /// `with_mutual_friends`.
    pub with_mutual_friends: bool,
    /// `with_mutual_friends_count`.
    pub with_mutual_friends_count: bool,
    /// `guild_id` — контекст сервера (снежинка), если профиль открыт из гильдии.
    pub guild_id: Option<String>,
}

impl Default for ProfileQuery {
    fn default() -> Self {
        Self {
            profile_type: Some("popout".to_string()),
            with_mutual_guilds: true,
            with_mutual_friends: true,
            with_mutual_friends_count: false,
            guild_id: None,
        }
    }
}

/// Пользователь в ответе профиля (`user`).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ProfileUser {
    /// Снежинка пользователя.
    pub id: String,
    /// Уникальное имя (Pomelo), не пустое в валидном ответе.
    pub username: String,
    /// Отображаемое имя.
    pub global_name: Option<String>,
    /// Хеш аватара.
    pub avatar: Option<String>,
    /// Описание профиля.
    pub bio: Option<String>,
    /// Местоимения.
    pub pronouns: Option<String>,
    /// Хеш баннера.
    pub banner: Option<String>,
    /// Цвет акцента (RGB, 24 бита).
    pub accent_color: Option<u32>,
}

/// Значок профиля (`badges[]`).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ProfileBadge {
    /// Идентификатор значка.
    pub id: String,
    /// Описание значка.
    pub description: String,
    /// Имя иконки в клиенте.
    pub icon: Option<String>,
    /// Ссылка значка.
    pub link: Option<String>,
}

/// Подключённый аккаунт (`connected_accounts[]`).
///
/// В JSON поле называется `type` — переименовано в `account_type`, чтобы не
/// конфликтовать с ключевым словом Rust.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ConnectedAccount {
    /// Тип сервиса (`github`, `steam`, ...).
    #[serde(rename = "type")]
    pub account_type: String,
    /// Идентификатор аккаунта в сервисе.
    pub id: String,
    /// Отображаемое имя в сервисе.
    pub name: String,
    /// Аккаунт подтверждён.
    pub verified: bool,
}

/// Профиль участника в контексте сервера (`guild_member`).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct GuildMemberProfile {
    /// Снежинка сервера.
    pub guild_id: Option<String>,
    /// Ник на сервере.
    pub nick: Option<String>,
    /// Описание профиля сервера.
    pub bio: Option<String>,
    /// Местоимения на сервере.
    pub pronouns: Option<String>,
    /// Роли участника на сервере.
    pub roles: Vec<String>,
}

/// Общий сервер с пользователем (`mutual_guilds[]`).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct MutualGuild {
    /// Снежинка сервера.
    pub id: String,
    /// Ник пользователя на этом сервере.
    pub nick: Option<String>,
}

/// Ответ `GET /users/{id}/profile`.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct UserProfileResponse {
    /// Пользователь.
    pub user: ProfileUser,
    /// Значки профиля.
    pub badges: Vec<ProfileBadge>,
    /// Профиль в контексте сервера, если запрос был с `guild_id`.
    pub guild_member: Option<GuildMemberProfile>,
    /// Подключённые аккаунты.
    pub connected_accounts: Vec<ConnectedAccount>,
    /// Общие серверы.
    pub mutual_guilds: Vec<MutualGuild>,
    /// Число общих друзей (запрошено `with_mutual_friends_count=true`).
    pub mutual_friends_count: Option<u32>,
}

/// Ошибка чтения профиля. Ни один исход не паникует и не несёт токен/тело.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProfileError {
    /// `user_id`/`guild_id`/`type` не прошли предсетевую валидацию.
    InvalidArg,
    /// 401: сессия истекла.
    Unauthorized,
    /// 403: профиль закрыт (заблокирован, нет доступа).
    Forbidden,
    /// 404: пользователь не найден.
    NotFound,
    /// 429: ограничение частоты.
    RateLimited {
        /// Задержка до повтора из тела/заголовков; 0 — если не разобрана.
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
    /// Транспорт/таймаут: сеть недоступна.
    Network,
    /// SEC-02: keyring/blocking недоступны — это не сеть; причина не утекает.
    Storage,
    /// 2xx, но тело не является профилем (или пустая идентичность пользователя).
    InvalidResponse,
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArg => f.write_str("некорректный id пользователя или сервера"),
            Self::Unauthorized => f.write_str("сессия истекла"),
            Self::Forbidden => f.write_str("нет доступа к профилю"),
            Self::NotFound => f.write_str("пользователь не найден"),
            Self::RateLimited { retry_after_ms } => {
                write!(f, "ограничение частоты; повтор через {retry_after_ms} мс")
            }
            Self::Client { status } => write!(f, "запрос отклонён (HTTP {status})"),
            Self::Server { status } => write!(f, "ошибка сервера Discord (HTTP {status})"),
            Self::Network => f.write_str("сеть недоступна"),
            Self::Storage => f.write_str("хранилище токена недоступно"),
            Self::InvalidResponse => f.write_str("некорректный ответ профиля"),
        }
    }
}

impl std::error::Error for ProfileError {}

impl From<RestFailure> for ProfileError {
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

/// Низкоуровневый HTTP-транспорт чтения профиля (`GET`).
pub trait ProfileTransport: Send + Sync {
    /// `GET target`; ошибка уже типизирована доменом профиля.
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, ProfileError>> + Send + 'a>>;
}

/// Транспорт поверх [`ApiClient`] ядра (реальная сеть).
pub struct ApiProfileTransport {
    client: ApiClient,
}

impl ApiProfileTransport {
    /// Создать транспорт с токеном владельца; токен в ошибки не попадает.
    pub fn new(token: &str) -> Result<Self, ProfileError> {
        let identity_ctx = crate::auth::IdentityContext::detect();
        let auth = ApiAuth::from_token(
            token,
            &identity_ctx.super_properties,
            &identity_ctx.locale,
            &identity_ctx.timezone,
        )
        .map_err(|_| ProfileError::InvalidArg)?;
        let config = ApiConfig {
            base: API_V9_BASE.to_string(),
            auth,
            http: HttpClientConfig::default(),
            retry: ApiRetryPolicy::default(),
        };
        let client = ApiClient::new(config).map_err(|_| ProfileError::Network)?;
        Ok(Self { client })
    }
}

impl ProfileTransport for ApiProfileTransport {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, ProfileError>> + Send + 'a>> {
        Box::pin(async move {
            self.client
                .get(target)
                .await
                .map_err(|error| ProfileError::from(RestFailure::from_api_error(&error)))
        })
    }
}

/// Клиент профиля поверх произвольного [`ProfileTransport`].
pub struct DiscordProfileApi<T> {
    transport: T,
}

impl<T> fmt::Debug for DiscordProfileApi<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DiscordProfileApi").finish_non_exhaustive()
    }
}

impl<T: ProfileTransport> DiscordProfileApi<T> {
    /// Клиент с готовым транспортом (в тестах — фейк).
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    /// Загрузить профиль пользователя (`GET /users/{id}/profile?...`).
    pub async fn fetch_user_profile(
        &self,
        user_id: &str,
        query: &ProfileQuery,
    ) -> Result<UserProfileResponse, ProfileError> {
        fetch_user_profile(&self.transport, user_id, query).await
    }
}

impl<T: ProfileTransport> ProfileTransport for DiscordProfileApi<T> {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, ProfileError>> + Send + 'a>> {
        self.transport.get(target)
    }
}

/// Загрузить профиль пользователя через транспорт.
pub async fn fetch_user_profile<T: ProfileTransport + ?Sized>(
    transport: &T,
    user_id: &str,
    query: &ProfileQuery,
) -> Result<UserProfileResponse, ProfileError> {
    let target = build_user_profile_path(user_id, query)?;
    let response = transport.get(&target).await?;
    decode_user_profile(&response)
}

/// Валидирует идентификаторы и собирает путь с query в порядке захвата:
/// `/users/{user_id}/profile?type=...&with_mutual_guilds=...&with_mutual_friends=...&with_mutual_friends_count=...[&guild_id=...]`.
pub fn build_user_profile_path(
    user_id: &str,
    query: &ProfileQuery,
) -> Result<String, ProfileError> {
    if !is_snowflake(user_id) {
        return Err(ProfileError::InvalidArg);
    }
    if let Some(guild_id) = query.guild_id.as_deref() {
        if !is_snowflake(guild_id) {
            return Err(ProfileError::InvalidArg);
        }
    }

    let mut params: Vec<String> = Vec::with_capacity(5);
    if let Some(profile_type) = query.profile_type.as_deref() {
        // `type` попадает в query без percent-encoding, поэтому `&`, `#`, `?`,
        // `/`, пробелы и control-символы отвергаем: иначе caller подменит
        // остальные параметры запроса.
        if !is_safe_query_token(profile_type) {
            return Err(ProfileError::InvalidArg);
        }
        params.push(format!("type={profile_type}"));
    }
    params.push(format!("with_mutual_guilds={}", query.with_mutual_guilds));
    params.push(format!("with_mutual_friends={}", query.with_mutual_friends));
    params.push(format!(
        "with_mutual_friends_count={}",
        query.with_mutual_friends_count
    ));
    if let Some(guild_id) = query.guild_id.as_deref() {
        params.push(format!("guild_id={guild_id}"));
    }

    Ok(format!("/users/{user_id}/profile?{}", params.join("&")))
}

/// Разобрать тело ответа профиля.
///
/// Требует непустые `user.id` и `user.username`: профиль без идентичности — не
/// профиль, и отдавать его фронтенду как валидный нельзя.
pub fn parse_user_profile(body: &[u8]) -> Result<UserProfileResponse, ProfileError> {
    let profile: UserProfileResponse =
        serde_json::from_slice(body).map_err(|_| ProfileError::InvalidResponse)?;
    if profile.user.id.trim().is_empty() || profile.user.username.trim().is_empty() {
        return Err(ProfileError::InvalidResponse);
    }
    Ok(profile)
}

/// Извлекает задержку `retry_after` (секунды) из JSON-тела `{"retry_after": ...}`
/// или заголовков `Retry-After` / `x-ratelimit-reset-after` и переводит в
/// миллисекунды (`u64`). Положительные субмиллисекундные значения — минимум `1`;
/// непригодное/отсутствующее значение — `0` (задержку не выдумываем).
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

fn decode_user_profile(response: &HttpResponse) -> Result<UserProfileResponse, ProfileError> {
    if response.status == 429 {
        return Err(ProfileError::RateLimited {
            retry_after_ms: parse_retry_after_ms(response),
        });
    }
    if let Some(failure) = RestFailure::from_status(response.status) {
        return Err(failure.into());
    }
    parse_user_profile(&response.body)
}

/// Снежинка: непустые ASCII-цифры, влезающие в `u64` (как в `reactions.rs`).
fn is_snowflake(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && value.parse::<u64>().is_ok()
}

/// Токен query без символов, способных переопределить остальные параметры.
fn is_safe_query_token(value: &str) -> bool {
    !value.is_empty()
        && !value
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace() || matches!(ch, '&' | '#' | '?' | '/'))
}
