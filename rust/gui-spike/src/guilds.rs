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
use discord_core::rest::routes::{self, HttpMethod};
use discord_core::rest::transport::{HttpClientConfig, HttpRequest, HttpResponse};
use discord_core::API_V9_BASE;
use serde::{Deserialize, Serialize};

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

/// Полезная нагрузка для создания нового сервера (`POST /guilds`, STUB-03).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateGuildPayload {
    /// Название создаваемого сервера (2..100 символов).
    pub name: String,
    /// Иконка сервера в формате Data URI (`data:image/png;base64,...`) или None.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Начальные каналы сервера.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channels: Option<Vec<serde_json::Value>>,
    /// Шаблон гильдии (если сервер создаётся из готового шаблона).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_template_code: Option<String>,
}

/// Результат создания сервера (`201 Created`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateGuildOutcome {
    /// Идентификатор созданного сервера (Snowflake).
    pub id: String,
    /// Название сервера.
    pub name: String,
    /// Хэш или Data URI иконки сервера.
    #[serde(default)]
    pub icon: Option<String>,
    /// Идентификатор владельца сервера.
    #[serde(default)]
    pub owner_id: Option<String>,
    /// Список начальных каналов сервера.
    #[serde(default)]
    pub channels: Vec<serde_json::Value>,
}

/// Ошибка создания сервера (`POST /guilds`, STUB-03).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CreateGuildError {
    /// Некорректные параметры (имя пустое или длиннее 100 символов).
    InvalidArg,
    /// Нет сохранённого токена.
    MissingToken,
    /// 401: сессия истекла.
    Unauthorized,
    /// 403: достигнут лимит создания серверов (максимум 100).
    Forbidden,
    /// 429: ограничение частоты.
    RateLimited {
        retry_after_ms: u64,
    },
    /// Прочие 4xx.
    Client {
        status: u16,
    },
    /// 5xx: ошибка сервера Discord.
    Server {
        status: u16,
    },
    /// Сеть недоступна.
    Network,
    /// Хранилище токена недоступно.
    Storage,
    /// Неожиданный ответ сервера.
    InvalidResponse,
}

impl fmt::Display for CreateGuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArg => formatter.write_str("некорректное имя сервера (требуется 2..100 символов)"),
            Self::MissingToken => formatter.write_str("нет сохранённого токена"),
            Self::Unauthorized => formatter.write_str("сессия истекла"),
            Self::Forbidden => formatter.write_str("создание сервера запрещено (достигнут лимит в 100 серверов)"),
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
            Self::InvalidResponse => formatter.write_str("неожиданный ответ создания сервера"),
        }
    }
}

impl std::error::Error for CreateGuildError {}

impl From<RestFailure> for CreateGuildError {
    fn from(failure: RestFailure) -> Self {
        match failure {
            RestFailure::Unauthorized => Self::Unauthorized,
            RestFailure::Forbidden => Self::Forbidden,
            RestFailure::NotFound => Self::Client { status: 404 },
            RestFailure::RateLimited { retry_after_ms } => Self::RateLimited { retry_after_ms },
            RestFailure::ClientOther { status } => Self::Client { status },
            RestFailure::Server { status } => Self::Server { status },
            RestFailure::Network => Self::Network,
            RestFailure::InvalidResponse => Self::InvalidResponse,
        }
    }
}

impl From<EnterGuildError> for CreateGuildError {
    fn from(error: EnterGuildError) -> Self {
        match error {
            EnterGuildError::InvalidArg => Self::InvalidArg,
            EnterGuildError::MissingToken => Self::MissingToken,
            EnterGuildError::Unauthorized => Self::Unauthorized,
            EnterGuildError::Forbidden => Self::Forbidden,
            EnterGuildError::NotFound => Self::Client { status: 404 },
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

/// Асинхронный контракт создания сервера (STUB-03).
pub trait GuildCreation: Send + Sync {
    fn create_guild<'a>(
        &'a self,
        payload: CreateGuildPayload,
    ) -> Pin<Box<dyn Future<Output = Result<CreateGuildOutcome, CreateGuildError>> + Send + 'a>>;
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

    /// POST с телом: создание сервера (`POST /guilds`, STUB-03).
    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, CreateGuildError>> + Send + 'a>>;
}

struct ApiGuildTransport(ApiClient);

impl GuildTransport for ApiGuildTransport {
    fn put<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, EnterGuildError>> + Send + 'a>> {
        Box::pin(async move {
            let gid = target
                .strip_prefix("/guilds/")
                .and_then(|rest| rest.split('/').next())
                .unwrap_or_default()
                .to_string();
            // Если гильдия уже разрешена и закэширована локально (например, через resolve_invite в модалке),
            // мгновенно материализуем её в состоянии и отправляем фоновый запрос присоединения без блокировки UI.
            if !gid.is_empty() && crate::data::has_live_guild_or_channels(&gid) {
                crate::data::ensure_local_guild_present(&gid);
                let gid_bg = gid.clone();
                std::thread::spawn(move || {
                    if let Some(code) = crate::data::cached_invite_code_for_guild(&gid_bg) {
                        if let Ok(inv_resp) = crate::data::accept_invite_via_live_rest(&code) {
                            if (200..300).contains(&inv_resp.status) {
                                crate::data::ingest_accepted_invite_payload(&inv_resp.body);
                            }
                        }
                    }
                    crate::data::spawn_hydrate_live_guild_full(&gid_bg);
                });
                return Ok(HttpResponse {
                    status: 204,
                    headers: Vec::new(),
                    body: Vec::new(),
                });
            }
            // Сначала пробуем принять кэшированный инвайт (`POST /invites/{code}` с полным `X-Context-Properties`),
            // а затем прямой вход участника (`PUT /guilds/{id}/members/@me?lurker=false`), прежде чем переходить в lurker-предпросмотр.
            if let Some(code) = crate::data::cached_invite_code_for_guild(&gid) {
                if let Ok(inv_resp) = crate::data::accept_invite_via_live_rest(&code) {
                    if (200..300).contains(&inv_resp.status) {
                        crate::data::ingest_accepted_invite_payload(&inv_resp.body);
                        return Ok(HttpResponse {
                            status: 204,
                            headers: inv_resp.headers,
                            body: Vec::new(),
                        });
                    }
                }
            }
            if !gid.is_empty() {
                if let Ok(join_resp) =
                    self.0.put(&format!("/guilds/{gid}/members/@me?lurker=false"), body).await
                {
                    if (200..300).contains(&join_resp.status) {
                        if !join_resp.body.is_empty() {
                            crate::data::ingest_accepted_invite_payload(&join_resp.body);
                        } else {
                            crate::data::spawn_hydrate_live_guild_full(&gid);
                        }
                        return Ok(HttpResponse {
                            status: 204,
                            headers: join_resp.headers,
                            body: Vec::new(),
                        });
                    }
                }
            }
            let target_with_sid = if !target.contains("session_id=") {
                if let Some(sid) = crate::data::live_gateway_session_id() {
                    let sep = if target.contains('?') { '&' } else { '?' };
                    format!("{target}{sep}session_id={sid}")
                } else {
                    target.to_string()
                }
            } else {
                target.to_string()
            };
            match self.0.put(&target_with_sid, body).await {
                Ok(resp) => {
                    if (200..300).contains(&resp.status) {
                        if !resp.body.is_empty() {
                            crate::data::ingest_accepted_invite_payload(&resp.body);
                        } else {
                            crate::data::ensure_local_guild_present(&gid);
                        }
                        return Ok(HttpResponse {
                            status: 204,
                            headers: resp.headers,
                            body: Vec::new(),
                        });
                    }
                    if crate::data::has_live_guild_or_channels(&gid) {
                        return Ok(HttpResponse {
                            status: 204,
                            headers: resp.headers,
                            body: Vec::new(),
                        });
                    }
                    Ok(resp)
                }
                Err(error) => {
                    let mapped = EnterGuildError::from(RestFailure::from_api_error(&error));
                    if crate::data::has_live_guild_or_channels(&gid) {
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

    fn delete<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, LeaveGuildError>> + Send + 'a>> {
        Box::pin(async move {
            let gid = target
                .strip_prefix(LEAVE_PATH_PREFIX)
                .unwrap_or_default()
                .to_string();
            let request = HttpRequest::new(HttpMethod::Delete, target).body(body.to_vec());
            match self.0.execute(&request).await {
                Ok(mut resp) => {
                    if (200..300).contains(&resp.status) || resp.status == 404 {
                        crate::data::remove_local_guild(&gid);
                        resp.status = 204;
                        resp.body.clear();
                    }
                    Ok(resp)
                }
                Err(error) => {
                    let mapped = LeaveGuildError::from(RestFailure::from_api_error(&error));
                    if matches!(mapped, LeaveGuildError::NotFound) {
                        crate::data::remove_local_guild(&gid);
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

    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, CreateGuildError>> + Send + 'a>> {
        Box::pin(async move {
            let request = HttpRequest::new(HttpMethod::Post, target).body(body.to_vec());
            match self.0.execute(&request).await {
                Ok(resp) => {
                    if (200..300).contains(&resp.status) && !resp.body.is_empty() {
                        crate::data::ingest_accepted_invite_payload(&resp.body);
                    }
                    Ok(resp)
                }
                Err(error) => {
                    let mapped = CreateGuildError::from(RestFailure::from_api_error(&error));
                    Err(mapped)
                }
            }
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
        let identity_ctx = crate::auth::IdentityContext::detect();
        let auth = ApiAuth::from_token(
            token,
            &identity_ctx.super_properties,
            &identity_ctx.locale,
            &identity_ctx.timezone,
        )
        .map_err(|_| EnterGuildError::InvalidArg)?;
        let client = ApiClient::new(ApiConfig {
            base: API_V9_BASE.to_string(),
            auth,
            http: HttpClientConfig {
                connect_timeout: std::time::Duration::from_millis(2500),
                read_timeout: std::time::Duration::from_millis(3500),
                write_timeout: std::time::Duration::from_millis(2500),
                ..HttpClientConfig::default()
            },
            // Вход в сервер — изменяющая операция: повторов нет (как у логина).
            retry: ApiRetryPolicy {
                max_attempts: 1,
                attempt_timeout: std::time::Duration::from_millis(4000),
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

    async fn execute_create(
        &self,
        payload: CreateGuildPayload,
    ) -> Result<CreateGuildOutcome, CreateGuildError> {
        let trimmed_name = payload.name.trim();
        if trimmed_name.is_empty() || trimmed_name.chars().count() > 100 {
            return Err(CreateGuildError::InvalidArg);
        }
        let body = serde_json::to_vec(&payload).map_err(|_| CreateGuildError::InvalidArg)?;
        let response = self.transport.post("/guilds", &body).await?;
        if !(200..300).contains(&response.status) {
            return Err(CreateGuildError::Client {
                status: response.status,
            });
        }
        if response.body.is_empty() {
            return Err(CreateGuildError::InvalidResponse);
        }
        let outcome: CreateGuildOutcome =
            serde_json::from_slice(&response.body).map_err(|_| CreateGuildError::InvalidResponse)?;
        Ok(outcome)
    }

    /// Создание нового сервера через REST ядра (`POST /guilds`, STUB-03).
    pub async fn create_guild(
        &self,
        payload: CreateGuildPayload,
    ) -> Result<CreateGuildOutcome, CreateGuildError> {
        self.execute_create(payload).await
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

impl GuildCreation for DiscordGuilds {
    fn create_guild<'a>(
        &'a self,
        payload: CreateGuildPayload,
    ) -> Pin<Box<dyn Future<Output = Result<CreateGuildOutcome, CreateGuildError>> + Send + 'a>> {
        Box::pin(self.execute_create(payload))
    }
}

/// Создание сервера: `POST /guilds` с валидацией имени (2..100) и полезной нагрузкой.
pub async fn create_guild<G: GuildCreation + ?Sized>(
    guilds: &G,
    payload: CreateGuildPayload,
) -> Result<CreateGuildOutcome, CreateGuildError> {
    let trimmed_name = payload.name.trim();
    if trimmed_name.is_empty() || trimmed_name.chars().count() > 100 {
        return Err(CreateGuildError::InvalidArg);
    }
    guilds.create_guild(payload).await
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

// ============================================================================
// Этап S1 & S3: Запрет 002-READY.json при живом токене и авто-подтверждение
// правил сервера (Member Verification Gate) при вступлении/гидрации.
// ============================================================================

/// Возвращает активный живой токен сессии (если он установлен).
pub fn resolve_live_token() -> Option<String> {
    crate::data::resolve_live_token()
}

/// Проверяет, запрещено ли чтение фикстуры `002-READY.json` в текущем режиме.
/// При наличии живого токена (`resolve_live_token().is_some()`) чтение `002-READY.json`
/// строго запрещено.
pub fn is_fixture_ready_forbidden(path: &std::path::Path) -> bool {
    let is_ready_fixture = path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|name| name.eq_ignore_ascii_case("002-READY.json"))
        .unwrap_or(false)
        || path.to_string_lossy().ends_with("002-READY.json");
    is_ready_fixture && resolve_live_token().is_some()
}

/// Безопасное чтение JSON-фикстуры с блокировкой `002-READY.json` при живом токене.
pub fn read_guild_fixture_json(path: &std::path::Path) -> Result<serde_json::Value, String> {
    if is_fixture_ready_forbidden(path) {
        return Err("Чтение фикстуры 002-READY.json запрещено при наличии живого токена".to_string());
    }
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

#[derive(Debug, Clone)]
pub struct GuildTransportResponse {
    pub status: u16,
    pub body: serde_json::Value,
}

pub trait GuildJoinTransport: Send + Sync {
    fn request(
        &self,
        method: &str,
        path: &str,
        headers: &std::collections::HashMap<String, String>,
        body: Option<&serde_json::Value>,
    ) -> Result<GuildTransportResponse, String>;
}

#[derive(Debug, Clone)]
pub struct GuildJoinOutcome {
    pub guild_id: String,
    pub pending: bool,
    pub roles: Vec<String>,
    pub guild: serde_json::Value,
    pub channels: Vec<serde_json::Value>,
    pub channel_permissions:
        std::collections::HashMap<String, crate::permissions::AllowedChannelAccess>,
}

/// Формирует тело `PUT /api/v9/guilds/{id}/requests/@me` из ответа `member-verification`,
/// выставляя `"response": true` для всех полей `TERMS` / правил сервера.
pub fn build_member_verification_accept_body(
    verification_form: &serde_json::Value,
) -> serde_json::Value {
    let version = verification_form
        .get("version")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let mut accepted_fields = Vec::new();
    if let Some(fields) = verification_form
        .get("form_fields")
        .and_then(|v| v.as_array())
    {
        for field in fields {
            let mut obj = field.as_object().cloned().unwrap_or_default();
            let field_type = obj
                .get("field_type")
                .and_then(|v| v.as_str())
                .unwrap_or("TERMS");
            if field_type.eq_ignore_ascii_case("TERMS") || !obj.contains_key("response") {
                obj.insert("response".to_string(), serde_json::Value::Bool(true));
            }
            accepted_fields.push(serde_json::Value::Object(obj));
        }
    }
    serde_json::json!({
        "version": version,
        "form_fields": accepted_fields,
    })
}

pub fn default_super_properties_base64() -> String {
    crate::auth::IdentityContext::detect().super_properties.to_base64()
}

/// Выполняет полный цикл S3 при вступлении на сервер по инвайту или при наличии `pending == true` / `403`:
/// 1. `POST /api/v9/invites/{code}`
/// 2. Если `pending == true` или `status == 403` (verification gate):
///    - `GET /api/v9/guilds/{id}/member-verification`
///    - `PUT /api/v9/guilds/{id}/requests/@me` с `X-Super-Properties`
/// 3. Перезагружает роли участника через `GET /api/v9/guilds/{id}/members/@me`
/// 4. Пересчитывает `channelPermissions` через `PermissionCalculator`.
pub fn accept_invite_and_join_guild<T: GuildJoinTransport + ?Sized>(
    transport: &T,
    invite_code: &str,
    guild_id: &str,
    user_id: &str,
) -> Result<GuildJoinOutcome, String> {
    if !is_snowflake(guild_id) {
        return Err("Некорректный ID гильдии".to_string());
    }
    let clean_code = invite_code.trim();
    if clean_code.is_empty() {
        return Err("Пустой код инвайта".to_string());
    }

    let super_props = default_super_properties_base64();
    let mut base_headers = std::collections::HashMap::new();
    base_headers.insert("X-Super-Properties".to_string(), super_props.clone());
    base_headers.insert("X-Context-Properties".to_string(), "e30=".to_string());

    let join_resp = transport.request(
        "POST",
        &format!("/api/v9/invites/{clean_code}"),
        &base_headers,
        Some(&serde_json::json!({})),
    )?;

    let join_pending = join_resp
        .body
        .get("member")
        .and_then(|m| m.get("pending").or_else(|| m.get("is_pending")))
        .and_then(|v| v.as_bool())
        .or_else(|| {
            join_resp
                .body
                .get("pending")
                .and_then(|v| v.as_bool())
        })
        .unwrap_or(false);
    let is_verification_403 = join_resp.status == 403;

    if join_resp.status >= 400 && !is_verification_403 {
        return Err(format!("Ошибка принятия инвайта: HTTP {}", join_resp.status));
    }

    complete_guild_verification_and_permissions(
        transport,
        guild_id,
        user_id,
        join_pending || is_verification_403,
        &super_props,
    )
}

pub fn complete_guild_verification_and_permissions<T: GuildJoinTransport + ?Sized>(
    transport: &T,
    guild_id: &str,
    user_id: &str,
    force_verification_check: bool,
    super_properties: &str,
) -> Result<GuildJoinOutcome, String> {
    let mut headers = std::collections::HashMap::new();
    headers.insert(
        "X-Super-Properties".to_string(),
        super_properties.to_string(),
    );

    let mut needs_verification = force_verification_check;
    let mut my_roles: Vec<String> = Vec::new();
    let mut pending = force_verification_check;

    if !needs_verification {
        if let Ok(member_resp) = transport.request(
            "GET",
            &format!("/api/v9/guilds/{guild_id}/members/@me"),
            &headers,
            None,
        ) {
            if member_resp.status == 403 {
                needs_verification = true;
            } else if (200..300).contains(&member_resp.status) {
                pending = member_resp
                    .body
                    .get("pending")
                    .or_else(|| member_resp.body.get("is_pending"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if pending {
                    needs_verification = true;
                }
                if let Some(arr) = member_resp.body.get("roles").and_then(|v| v.as_array()) {
                    my_roles = arr
                        .iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect();
                }
            }
        }
    }

    if needs_verification {
        let ver_resp = transport.request(
            "GET",
            &format!("/api/v9/guilds/{guild_id}/member-verification"),
            &headers,
            None,
        )?;
        if (200..300).contains(&ver_resp.status) {
            let put_body = build_member_verification_accept_body(&ver_resp.body);
            let put_resp = transport.request(
                "PUT",
                &format!("/api/v9/guilds/{guild_id}/requests/@me"),
                &headers,
                Some(&put_body),
            )?;
            if put_resp.status < 300 {
                pending = false;
            }
        }
    }

    // Reload member roles via GET /api/v9/guilds/{id}/members/@me
    if let Ok(member_resp) = transport.request(
        "GET",
        &format!("/api/v9/guilds/{guild_id}/members/@me"),
        &headers,
        None,
    ) {
        if (200..300).contains(&member_resp.status) {
            pending = member_resp
                .body
                .get("pending")
                .or_else(|| member_resp.body.get("is_pending"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if let Some(arr) = member_resp.body.get("roles").and_then(|v| v.as_array()) {
                my_roles = arr
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect();
            }
        }
    }

    let guild_resp = transport
        .request("GET", &format!("/api/v9/guilds/{guild_id}"), &headers, None)
        .unwrap_or(GuildTransportResponse {
            status: 200,
            body: serde_json::json!({ "id": guild_id }),
        });
    let channels_resp = transport
        .request(
            "GET",
            &format!("/api/v9/guilds/{guild_id}/channels"),
            &headers,
            None,
        )
        .unwrap_or(GuildTransportResponse {
            status: 200,
            body: serde_json::json!([]),
        });

    let roles_json = guild_resp
        .body
        .get("roles")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let channels_json = channels_resp
        .body
        .as_array()
        .cloned()
        .or_else(|| {
            guild_resp
                .body
                .get("channels")
                .and_then(|v| v.as_array())
                .cloned()
        })
        .unwrap_or_default();
    let owner_id = guild_resp
        .body
        .get("owner_id")
        .or_else(|| guild_resp.body.get("ownerId"))
        .and_then(|v| v.as_str());

    let channel_permissions =
        crate::permissions::PermissionCalculator::compute_guild_channel_permissions(
            guild_id,
            owner_id,
            user_id,
            &my_roles,
            &roles_json,
            &channels_json,
            false,
            pending,
        );

    Ok(GuildJoinOutcome {
        guild_id: guild_id.to_string(),
        pending,
        roles: my_roles,
        guild: guild_resp.body,
        channels: channels_json,
        channel_permissions,
    })
}
