//! Членство в сервере: вход из preview/lurker (G-03a) и выход (G-04).
//!
//! Вход — `PUT /guilds/{id}/members/@me?lurker=true`. Контракт подтверждён живым
//! захватом (`waves/RW-40/NETWORK-CONTRACTS.md` §1.1 #2, `D02:101,138,225`):
//! query ровно `lurker=true`, тело — пустой JSON-объект `{}`, успех — **204** без
//! тела. Это **не** join по инвайту (`POST /invites/{code}`): тот flow требует
//! `session_id`/`invite_instance_id` и здесь не реализуется.
//!
//! Выход — `DELETE /users/@me/guilds/{id}` с телом `{lurking: bool}` (§1.1 #4,
//! `B` anchor `async leaveGuild(e)`; ответ не читается). `lurking` — явный bool
//! от caller'а (UI решает `isLurking`/`isGuest`), модуль его не вычисляет. HTTP-коды
//! выхода в оракуле помечены `[A]`; реализация не выдумывает их: 2xx — успех,
//! 401/403/404/429 типизированы, тело ошибки не разбирается.
//! Это **не** delete guild владельцем (`POST /guilds/{id}/delete`) — другой flow.
//!
//! Токен живёт только в auth-заголовке `ApiClient`; в ошибки и `Debug` не попадает.
//! Сеть — только через [`GuildTransport`]; тесты подменяют транспорт фейком.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use discord_core::rest::client::{ApiAuth, ApiClient, ApiConfig, ApiRetryPolicy};
use discord_core::rest::identity::{IdentityConfig, SystemSources};
use discord_core::rest::routes::{self, HttpMethod};
use discord_core::rest::transport::{HttpClientConfig, HttpRequest, HttpResponse};
use discord_core::API_V9_BASE;
use serde::Serialize;

use crate::rest_failure::RestFailure;

/// Шаблон маршрута в каталоге `discord-core` (там же — оракул `D02`).
const MEMBERS_ME_PATH: &str = "/guilds/{id}/members/@me";

/// Путь выхода из сервера (§1.1 #4). В каталоге `routes.rs` его нет: каталог
/// фиксирует 47 live-маршрутов `D02`, а leave подтверждён только кодом клиента.
const LEAVE_PATH_PREFIX: &str = "/users/@me/guilds/";

/// Единственный подтверждённый query lurker-входа.
pub const LURKER_QUERY: &str = "lurker=true";

/// Тело запроса — ровно `{}` (в захвате `body_keys = ["(empty-object)"]`).
const EMPTY_BODY: &[u8] = b"{}";

/// Ошибка входа в сервер. Ни один исход не паникует и не несёт токен/тело.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EnterGuildError {
    /// `guild_id` не снежинка (пустой, не цифры, длиннее u64).
    InvalidArg,
    /// Нет сохранённого токена — в сеть не уходим.
    MissingToken,
    /// 401: сессия истекла.
    Unauthorized,
    /// 403: нет доступа к серверу (закрытый preview/бан).
    Forbidden,
    /// 404: сервер не найден.
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
    /// 2xx, кроме подтверждённого live 204: тело/статус нарушают контракт.
    Parse,
}

impl fmt::Display for EnterGuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArg => formatter.write_str("некорректный id сервера"),
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
            Self::Server { status } => {
                write!(formatter, "ошибка сервера Discord (HTTP {status})")
            }
            Self::Network => formatter.write_str("сеть недоступна"),
            Self::Storage => formatter.write_str("хранилище токена недоступно"),
            Self::Parse => formatter.write_str("неожиданный ответ входа в сервер"),
        }
    }
}

impl std::error::Error for EnterGuildError {}

impl From<crate::rest_failure::RestFailure> for EnterGuildError {
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
            // Вход в сервер не разбирает тело ошибки; 2xx с иным статусом — `Parse`
            // (см. `decode`), поэтому `InvalidResponse` сюда не приходит.
            RestFailure::InvalidResponse => Self::Parse,
        }
    }
}

/// Ошибка выхода из сервера (G-04). Ни один исход не паникует и не несёт
/// токен/тело; коды HTTP, не подтверждённые контрактом, не выдумываются.
///
/// `Client`/`Server` — аддитивные варианты политики G1-01 (`rest_failure.rs`):
/// иначе 400/5xx схлопнулись бы в `Network` и потеряли статус.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LeaveGuildError {
    /// `guild_id` не снежинка (пустой, не цифры, длиннее u64).
    InvalidArg,
    /// Нет сохранённого токена — в сеть не уходим.
    MissingToken,
    /// 401: сессия истекла.
    Unauthorized,
    /// 403: нет прав на выход (например, владелец сервера).
    Forbidden,
    /// 404: сервер/участие не найдены.
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
    /// 2xx, но ответ нарушает контракт (не должно случаться: тело не читается).
    InvalidResponse,
}

impl fmt::Display for LeaveGuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArg => formatter.write_str("некорректный id сервера"),
            Self::MissingToken => formatter.write_str("нет сохранённого токена"),
            Self::Unauthorized => formatter.write_str("сессия истекла"),
            Self::Forbidden => formatter.write_str("выход с сервера запрещён"),
            Self::NotFound => formatter.write_str("сервер не найден"),
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
            Self::InvalidResponse => formatter.write_str("неожиданный ответ выхода с сервера"),
        }
    }
}

impl std::error::Error for LeaveGuildError {}

impl From<RestFailure> for LeaveGuildError {
    fn from(failure: RestFailure) -> Self {
        match failure {
            RestFailure::Unauthorized => Self::Unauthorized,
            RestFailure::Forbidden => Self::Forbidden,
            RestFailure::NotFound => Self::NotFound,
            RestFailure::RateLimited { retry_after_ms } => Self::RateLimited { retry_after_ms },
            RestFailure::ClientOther { status } => Self::Client { status },
            RestFailure::Server { status } => Self::Server { status },
            RestFailure::Network => Self::Network,
            // Тело ответа не читается, поэтому `InvalidResponse` рождается только
            // из 2xx-ветки `decode_leave` (сейчас недостижимой), не из классификатора.
            RestFailure::InvalidResponse => Self::InvalidResponse,
        }
    }
}

/// `DiscordGuilds::new` возвращает ошибку входа; для команды выхода нужен тот же
/// типизированный код домена, а не второй конструктор с копией проверки токена.
impl From<EnterGuildError> for LeaveGuildError {
    fn from(error: EnterGuildError) -> Self {
        match error {
            EnterGuildError::InvalidArg => Self::InvalidArg,
            EnterGuildError::MissingToken => Self::MissingToken,
            EnterGuildError::Unauthorized => Self::Unauthorized,
            EnterGuildError::Forbidden => Self::Forbidden,
            EnterGuildError::NotFound => Self::NotFound,
            EnterGuildError::RateLimited { retry_after_ms } => Self::RateLimited { retry_after_ms },
            EnterGuildError::Client { status } => Self::Client { status },
            EnterGuildError::Server { status } => Self::Server { status },
            EnterGuildError::Network => Self::Network,
            EnterGuildError::Storage => Self::Storage,
            EnterGuildError::Parse => Self::InvalidResponse,
        }
    }
}

/// Асинхронный контракт входа в сервер; удобен для фейков в тестах.
pub trait GuildMembership: Send + Sync {
    fn enter_guild<'a>(
        &'a self,
        guild_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), EnterGuildError>> + Send + 'a>>;
}

/// Асинхронный контракт выхода из сервера (G-04); отдельный трейт, чтобы вход и
/// выход не навязывали друг другу реализацию.
pub trait GuildLeaving: Send + Sync {
    fn leave_guild<'a>(
        &'a self,
        guild_id: &'a str,
        lurking: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(), LeaveGuildError>> + Send + 'a>>;
}

/// Транспорт guild-операций с JSON-телом; фейк в тестах записывает target и тело.
pub trait GuildTransport: Send + Sync {
    fn put<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, EnterGuildError>> + Send + 'a>>;

    /// DELETE с телом: `{lurking: bool}` — часть контракта leave (§1.1 #4).
    fn delete<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, LeaveGuildError>> + Send + 'a>>;
}

struct ApiGuildTransport(ApiClient);

impl GuildTransport for ApiGuildTransport {
    fn put<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, EnterGuildError>> + Send + 'a>> {
        Box::pin(async move {
            self.0
                .put(target, body)
                .await
                .map_err(|error| EnterGuildError::from(RestFailure::from_api_error(&error)))
        })
    }

    fn delete<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, LeaveGuildError>> + Send + 'a>> {
        Box::pin(async move {
            // В ядре нет `delete` с телом: `ApiClient::execute` — тот же путь,
            // что у `put`/`post` (auth, телеметрия, политика повторов).
            let request = HttpRequest::new(HttpMethod::Delete, target).body(body.to_vec());
            self.0
                .execute(&request)
                .await
                .map_err(|error| LeaveGuildError::from(RestFailure::from_api_error(&error)))
        })
    }
}

/// Вход в сервер через REST ядра.
pub struct DiscordGuilds {
    transport: Arc<dyn GuildTransport>,
}

impl fmt::Debug for DiscordGuilds {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DiscordGuilds")
            .finish_non_exhaustive()
    }
}

impl DiscordGuilds {
    /// Клиент с боевым транспортом; токен — только в auth-заголовке.
    pub fn new(token: &str) -> Result<Self, EnterGuildError> {
        if token.trim().is_empty() {
            return Err(EnterGuildError::MissingToken);
        }
        let identity =
            discord_core::rest::identity::build(&SystemSources, &IdentityConfig::default());
        let auth = ApiAuth::from_token(token, &identity, "en-US", "UTC")
            .map_err(|_| EnterGuildError::InvalidArg)?;
        let client = ApiClient::new(ApiConfig {
            base: API_V9_BASE.to_string(),
            auth,
            http: HttpClientConfig::default(),
            // Вход в сервер — изменяющая операция: повторов нет (как у логина).
            retry: ApiRetryPolicy {
                max_attempts: 1,
                ..ApiRetryPolicy::default()
            },
        })
        .map_err(|_| EnterGuildError::Network)?;
        Ok(Self {
            transport: Arc::new(ApiGuildTransport(client)),
        })
    }

    #[doc(hidden)]
    pub fn with_transport(transport: Arc<dyn GuildTransport>) -> Self {
        Self { transport }
    }

    async fn execute(&self, guild_id: &str) -> Result<(), EnterGuildError> {
        validate_guild_id(guild_id)?;
        let route =
            routes::find(HttpMethod::Put, MEMBERS_ME_PATH).ok_or(EnterGuildError::Network)?;
        let path =
            routes::render_path(route, &[guild_id]).map_err(|_| EnterGuildError::InvalidArg)?;
        let target = format!("{path}?{LURKER_QUERY}");
        let response = self.transport.put(&target, EMPTY_BODY).await?;
        decode(response)
    }

    async fn execute_leave(&self, guild_id: &str, lurking: bool) -> Result<(), LeaveGuildError> {
        validate_leave_guild_id(guild_id)?;
        // `guild_id` уже проверен как снежинка: подстановка безопасна и без каталога.
        let target = format!("{LEAVE_PATH_PREFIX}{guild_id}");
        let body = leave_body(lurking);
        let response = self.transport.delete(&target, &body).await?;
        decode_leave(response)
    }
}

impl GuildMembership for DiscordGuilds {
    fn enter_guild<'a>(
        &'a self,
        guild_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), EnterGuildError>> + Send + 'a>> {
        Box::pin(self.execute(guild_id))
    }
}

impl GuildLeaving for DiscordGuilds {
    fn leave_guild<'a>(
        &'a self,
        guild_id: &'a str,
        lurking: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(), LeaveGuildError>> + Send + 'a>> {
        Box::pin(self.execute_leave(guild_id, lurking))
    }
}

/// Вход в сервер из preview/lurker; валидация — до обращения к сети.
pub async fn enter_guild<G: GuildMembership + ?Sized>(
    guilds: &G,
    guild_id: &str,
) -> Result<(), EnterGuildError> {
    validate_guild_id(guild_id)?;
    guilds.enter_guild(guild_id).await
}

/// UI-глагол «войти в сервер»; семантика и валидация те же, что у [`enter_guild`].
pub async fn join_guild<G: GuildMembership + ?Sized>(
    guilds: &G,
    guild_id: &str,
) -> Result<(), EnterGuildError> {
    enter_guild(guilds, guild_id).await
}

/// Выход из сервера: `DELETE /users/@me/guilds/{id}`, тело `{lurking}`.
///
/// `lurking` передаёт caller (UI решает `isLurking`/`isGuest`); валидация id — до
/// сети. Успех — любой 2xx, тело ответа не читается.
pub async fn leave_guild<G: GuildLeaving + ?Sized>(
    guilds: &G,
    guild_id: &str,
    lurking: bool,
) -> Result<(), LeaveGuildError> {
    validate_leave_guild_id(guild_id)?;
    guilds.leave_guild(guild_id, lurking).await
}

/// Тело выхода: ровно одно логическое поле `lurking` (не строка, не 0/1).
fn leave_body(lurking: bool) -> Vec<u8> {
    serde_json::json!({ "lurking": lurking })
        .to_string()
        .into_bytes()
}

/// Снежинка Discord: только цифры и влезает в `u64` (≤20 цифр).
fn is_snowflake(guild_id: &str) -> bool {
    !guild_id.is_empty()
        && guild_id.bytes().all(|byte| byte.is_ascii_digit())
        && guild_id.parse::<u64>().is_ok()
}

fn validate_guild_id(guild_id: &str) -> Result<(), EnterGuildError> {
    if !is_snowflake(guild_id) {
        return Err(EnterGuildError::InvalidArg);
    }
    Ok(())
}

fn validate_leave_guild_id(guild_id: &str) -> Result<(), LeaveGuildError> {
    if !is_snowflake(guild_id) {
        return Err(LeaveGuildError::InvalidArg);
    }
    Ok(())
}

/// Статус → типизированная ошибка. Тело не разбирается: у 204 его нет.
fn decode(response: HttpResponse) -> Result<(), EnterGuildError> {
    match response.status {
        // Единственный успех, подтверждённый live (204 ×2).
        204 => Ok(()),
        // Другой 2xx означает нарушение зафиксированного контракта, а не успех.
        status if (200..300).contains(&status) => Err(EnterGuildError::Parse),
        // 429: тело не подтверждено для этого маршрута — задержку не выдумываем.
        status => {
            Err(RestFailure::from_status(status).map_or(EnterGuildError::Network, Into::into))
        }
    }
}

/// Выход: успех — любой 2xx (контракт §1.1 #4: ответ не читается), остальное —
/// по единой политике [`RestFailure`]; тело ошибки не разбираем.
fn decode_leave(response: HttpResponse) -> Result<(), LeaveGuildError> {
    if (200..300).contains(&response.status) {
        return Ok(());
    }
    Err(RestFailure::from_status(response.status)
        .map_or(LeaveGuildError::Network, LeaveGuildError::from))
}
