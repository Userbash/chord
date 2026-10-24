//! G-03 (D-108): вход по приглашению — `GET /invites/{code}` и
//! `POST /invites/{code}`.
//!
//! Контракт `[S]` (`NETWORK-CONTRACTS.md` §1.2/§1.4, `G6-REPORT-RW40-02.md` §3.3):
//!
//! - **resolve** — `GET /invites/{code}` с подтверждёнными якорем флагами
//!   `with_counts=true&with_expiration=true`; ответ — invite-объект
//!   (`code`, `guild{id,name,icon}`, `channel{id}`, `approximate_member_count`,
//!   `approximate_presence_count`, `expires_at`). Остальные query-параметры
//!   (`inputValue`, `with_games`, …) — analytics/опциональные, их не выдумываем.
//! - **accept** — `POST /invites/{code}`, тело ровно
//!   `{session_id, invite_instance_id, received_installation_id}`, **все три nullable**
//!   (`getSessionId()` в бандле отдаёт `null`, `invite_instance_id` может отсутствовать,
//!   `received_installation_id` чистится после успеха). `context` — analytics-объект
//!   запроса, **не** поле тела. Ответ — тот же invite-объект, из него берутся
//!   `guild_id`, `target_channel_id?`, `target_message_id?`.
//!
//! HTTP-коды и TTL live не наблюдались (`[A]`, в `D02` invite-ручек 0). Поэтому:
//! любой 2xx обязан принести разбираемый invite-объект (2xx без объекта — не успех,
//! а [`InviteError::InvalidResponse`]), 401/403/404/429/4xx/5xx/сеть — типизированы.
//!
//! Сеть — только через [`InviteTransport`]; тесты подменяют транспорт фейком и
//! проверяют target/тело. Токен живёт только в auth-заголовке [`ApiClient`] и не
//! попадает в ошибки/`Debug`.

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

use crate::rest_failure::RestFailure;

/// Подтверждённый якорем запрос resolve (§1.1 #12: `with_counts:!0,with_expiration:!0`).
///
/// Прочие параметры (`inputValue`, `with_permissions`, `with_games`, …) зависят от
/// контекста UI и не подтверждены live — не отправляем и не выдумываем.
const RESOLVE_QUERY: &str = "?with_counts=true&with_expiration=true";

/// Ошибка invite-операции. Ни один исход не паникует и не несёт токен/тело.
///
/// `Storage` — аддитивный внутренний код (SEC-02/G1-02): отказ keyring/blocking не
/// выдаётся за сеть. `MissingToken` — локальное «токена нет», как у search/guild-команд.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum InviteError {
    /// Код не прошёл нормализацию/валидацию (пустой, чужие символы, вне 2…32).
    InvalidArg,
    /// В хранилище нет токена — сеть не вызывалась.
    MissingToken,
    /// 401: сессия истекла.
    Unauthorized,
    /// 403: нет доступа к приглашению (в т.ч. бан — тело ошибки `[A]`, не разбираем).
    Forbidden,
    /// 404: приглашение не найдено/истекло.
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
    /// 2xx без разбираемого invite-объекта или неожиданные типы полей.
    InvalidResponse,
}

impl fmt::Display for InviteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArg => formatter.write_str("некорректный код приглашения"),
            Self::MissingToken => formatter.write_str("токен не найден"),
            Self::Unauthorized => formatter.write_str("сессия истекла"),
            Self::Forbidden => formatter.write_str("нет доступа к приглашению"),
            Self::NotFound => formatter.write_str("приглашение не найдено или истекло"),
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
            Self::InvalidResponse => formatter.write_str("неожиданный ответ invite"),
        }
    }
}

impl std::error::Error for InviteError {}

impl From<RestFailure> for InviteError {
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

/// Превью приглашения для UI (`G6 §3.3`), поля — camelCase в IPC.
///
/// `code`, `guildId`, `guildName` обязательны: серверный invite без гильдии (групповой
/// DM) не является целью этого flow и трактуется как [`InviteError::InvalidResponse`].
/// Опциональные поля не сериализуются, если их нет в ответе, — UI не получает
/// выдуманных значений.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvitePreview {
    /// Код приглашения (эхо сервера, не нормализованный вход).
    pub code: String,
    /// `guild.id`.
    pub guild_id: String,
    /// `guild.name`.
    pub guild_name: String,
    /// `guild.icon` — хеш CDN, не URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// `approximate_member_count`.
    #[serde(
        rename = "memberCount",
        alias = "approximate_member_count",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub member_count: Option<u64>,
    /// `approximate_presence_count`.
    #[serde(
        rename = "presenceCount",
        alias = "approximate_presence_count",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub presence_count: Option<u64>,
    /// `expires_at` (ISO-8601); `null` у бессрочного инвайта — `None`.
    #[serde(
        rename = "expiresAt",
        alias = "expires_at",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub expires_at: Option<String>,
    /// `channel.id` — целевой канал приглашения.
    #[serde(rename = "channelId", default, skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<String>,
}

/// Результат accept: что клиенту нужно, чтобы перейти в сервер/канал (§3.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptedInvite {
    /// `guild_id` — сервер, в который вступили.
    pub guild_id: String,
    /// `target_channel_id` — канал, куда ведёт инвайт.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_channel_id: Option<String>,
    /// `target_message_id` — сообщение, куда ведёт инвайт.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_message_id: Option<String>,
}

/// Nullable-поля тела accept (§1.2). `Default` — все три `null`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AcceptRequest {
    /// `session_id` = `getSessionId()`; в бандле `null`.
    pub session_id: Option<String>,
    /// `invite_instance_id`; для вставленного кода может отсутствовать.
    pub invite_instance_id: Option<String>,
    /// `received_installation_id`; `null` допустим.
    pub received_installation_id: Option<String>,
}

/// Доменный контракт invite: удобен для фейков в тестах и Tauri-слоя.
pub trait InvitesApi: Send + Sync {
    /// Превью по коду; код уже нормализован вызывающим.
    fn resolve<'a>(
        &'a self,
        code: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<InvitePreview, InviteError>> + Send + 'a>>;

    /// Вступить по коду с nullable-полями тела.
    fn accept<'a>(
        &'a self,
        code: &'a str,
        request: &'a AcceptRequest,
    ) -> Pin<Box<dyn Future<Output = Result<AcceptedInvite, InviteError>> + Send + 'a>>;
}

/// Транспорт invite: GET без тела, POST с JSON-телом. Фейк фиксирует target/тело.
pub trait InviteTransport: Send + Sync {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, InviteError>> + Send + 'a>>;

    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, InviteError>> + Send + 'a>>;
}

struct ApiInviteTransport(ApiClient);

impl InviteTransport for ApiInviteTransport {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, InviteError>> + Send + 'a>> {
        Box::pin(async move {
            self.0
                .get(target)
                .await
                .map_err(|error| InviteError::from(RestFailure::from_api_error(&error)))
        })
    }

    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, InviteError>> + Send + 'a>> {
        Box::pin(async move {
            self.0
                .post(target, body)
                .await
                .map_err(|error| InviteError::from(RestFailure::from_api_error(&error)))
        })
    }
}

/// REST-реализация invite через ядро.
pub struct DiscordInvites {
    transport: Arc<dyn InviteTransport>,
}

impl fmt::Debug for DiscordInvites {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DiscordInvites")
            .finish_non_exhaustive()
    }
}

impl DiscordInvites {
    /// Клиент с боевым транспортом; токен — только в auth-заголовке.
    ///
    /// Повторов нет (`max_attempts: 1`): accept — изменяющая операция, повторный
    /// POST недопустим; resolve при этом остаётся читающим и от повтора не страдает.
    pub fn new(token: &str) -> Result<Self, InviteError> {
        if token.trim().is_empty() {
            return Err(InviteError::MissingToken);
        }
        let identity =
            discord_core::rest::identity::build(&SystemSources, &IdentityConfig::default());
        let auth = ApiAuth::from_token(token, &identity, "en-US", "UTC")
            .map_err(|_| InviteError::InvalidArg)?;
        let client = ApiClient::new(ApiConfig {
            base: API_V9_BASE.to_string(),
            auth,
            http: HttpClientConfig::default(),
            retry: ApiRetryPolicy {
                max_attempts: 1,
                ..ApiRetryPolicy::default()
            },
        })
        .map_err(|_| InviteError::Network)?;
        Ok(Self {
            transport: Arc::new(ApiInviteTransport(client)),
        })
    }

    #[doc(hidden)]
    pub fn with_transport(transport: Arc<dyn InviteTransport>) -> Self {
        Self { transport }
    }

    async fn execute_resolve(&self, code: &str) -> Result<InvitePreview, InviteError> {
        let code = normalize_code(code)?;
        let route =
            routes::find(HttpMethod::Get, routes::INVITE_PATH).ok_or(InviteError::Network)?;
        let path = routes::render_path(route, &[&code]).map_err(|_| InviteError::InvalidArg)?;
        let target = format!("{path}{RESOLVE_QUERY}");
        let response = self.transport.get(&target).await?;
        decode_preview(&response)
    }

    async fn execute_accept(
        &self,
        code: &str,
        request: &AcceptRequest,
    ) -> Result<AcceptedInvite, InviteError> {
        let code = normalize_code(code)?;
        let route =
            routes::find(HttpMethod::Post, routes::INVITE_PATH).ok_or(InviteError::Network)?;
        let path = routes::render_path(route, &[&code]).map_err(|_| InviteError::InvalidArg)?;
        let body = accept_body(request)?;
        let response = self.transport.post(&path, &body).await?;
        decode_accept(&response)
    }
}

impl InvitesApi for DiscordInvites {
    fn resolve<'a>(
        &'a self,
        code: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<InvitePreview, InviteError>> + Send + 'a>> {
        Box::pin(self.execute_resolve(code))
    }

    fn accept<'a>(
        &'a self,
        code: &'a str,
        request: &'a AcceptRequest,
    ) -> Pin<Box<dyn Future<Output = Result<AcceptedInvite, InviteError>> + Send + 'a>> {
        Box::pin(self.execute_accept(code, request))
    }
}

/// Превью по коду: нормализация — до обращения к сети.
pub async fn resolve_invite<A: InvitesApi + ?Sized>(
    api: &A,
    code: &str,
) -> Result<InvitePreview, InviteError> {
    let code = normalize_code(code)?;
    api.resolve(&code).await
}

/// Вступить по коду: три nullable-поля передаются как есть (без подстановок).
pub async fn accept_invite<A: InvitesApi + ?Sized>(
    api: &A,
    code: &str,
    session_id: Option<&str>,
    invite_instance_id: Option<&str>,
    received_installation_id: Option<&str>,
) -> Result<AcceptedInvite, InviteError> {
    let code = normalize_code(code)?;
    let request = AcceptRequest {
        session_id: session_id.map(str::to_string),
        invite_instance_id: invite_instance_id.map(str::to_string),
        received_installation_id: received_installation_id.map(str::to_string),
    };
    api.accept(&code, &request).await
}

/// Нормализация введённого кода (`G6 §3.3`): trim, снять query/fragment и хвостовой
/// `/`, взять последний сегмент (`…/invite/<code>`), допустимые символы —
/// `[A-Za-z0-9-]`, длина 2…32.
///
/// Границы и набор символов — наша валидация (`[A]`), а не утверждение о Discord:
/// так код нельзя использовать как произвольный путь. Снятие query/fragment — тоже
/// наша нормализация пользовательского ввода (ссылка из буфера обмена).
fn normalize_code(input: &str) -> Result<String, InviteError> {
    let trimmed = input.trim();
    let cut = trimmed.find(['?', '#']).unwrap_or(trimmed.len());
    let without_query = trimmed[..cut].trim_end_matches('/');
    let code = without_query.rsplit('/').next().unwrap_or_default();
    if !(2..=32).contains(&code.len())
        || !code
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(InviteError::InvalidArg);
    }
    Ok(code.to_string())
}

/// Тело accept — дословно три поля (§1.2), `null` вместо отсутствующих ключей.
#[derive(Serialize)]
struct AcceptBody<'a> {
    session_id: Option<&'a str>,
    invite_instance_id: Option<&'a str>,
    received_installation_id: Option<&'a str>,
}

fn accept_body(request: &AcceptRequest) -> Result<Vec<u8>, InviteError> {
    serde_json::to_vec(&AcceptBody {
        session_id: request.session_id.as_deref(),
        invite_instance_id: request.invite_instance_id.as_deref(),
        received_installation_id: request.received_installation_id.as_deref(),
    })
    .map_err(|_| InviteError::InvalidResponse)
}

/// HTTP-статус → типизированный отказ. Любой 2xx допустим как «ответ пришёл», но
/// тело всё равно обязано быть invite-объектом (`[A]`-коды, живой захват — позже).
fn ensure_success(status: u16) -> Result<(), InviteError> {
    match status {
        200..=299 => Ok(()),
        other => {
            Err(RestFailure::from_status(other).map_or(InviteError::Network, InviteError::from))
        }
    }
}

/// Wire-объект resolve (§1.4). Поля объявлены `Option`, чтобы отличить
/// «сервер прислал `null`» от «сервер не прислал поля»; обязательность проверяется
/// после разбора.
#[derive(Deserialize)]
struct WireInvite {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    guild: Option<WireGuild>,
    #[serde(default)]
    channel: Option<WireChannel>,
    #[serde(default)]
    approximate_member_count: Option<u64>,
    #[serde(default)]
    approximate_presence_count: Option<u64>,
    #[serde(default)]
    expires_at: Option<String>,
}

#[derive(Deserialize)]
struct WireGuild {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    icon: Option<String>,
}

#[derive(Deserialize)]
struct WireChannel {
    #[serde(default)]
    id: Option<String>,
}

/// Wire-объект accept (§1.2/§1.4): обязателен только `guild_id`.
#[derive(Deserialize)]
struct WireAccept {
    #[serde(default)]
    guild_id: Option<String>,
    #[serde(default)]
    target_channel_id: Option<String>,
    #[serde(default)]
    target_message_id: Option<String>,
}

/// `Option<String>` → непустая строка; пустое/отсутствующее значение — ошибка.
fn required_string(value: Option<String>) -> Result<String, InviteError> {
    match value {
        Some(text) if !text.trim().is_empty() => Ok(text),
        _ => Err(InviteError::InvalidResponse),
    }
}

fn decode_preview(response: &HttpResponse) -> Result<InvitePreview, InviteError> {
    ensure_success(response.status)?;
    let wire: WireInvite = parse_json(&response.body)?;
    let guild = wire.guild.ok_or(InviteError::InvalidResponse)?;
    let channel_id = match wire.channel {
        Some(channel) => Some(required_string(channel.id)?),
        None => None,
    };
    Ok(InvitePreview {
        code: required_string(wire.code)?,
        guild_id: required_string(guild.id)?,
        guild_name: required_string(guild.name)?,
        icon: guild.icon,
        member_count: wire.approximate_member_count,
        presence_count: wire.approximate_presence_count,
        expires_at: wire.expires_at,
        channel_id,
    })
}

fn decode_accept(response: &HttpResponse) -> Result<AcceptedInvite, InviteError> {
    ensure_success(response.status)?;
    let wire: WireAccept = parse_json(&response.body)?;
    Ok(AcceptedInvite {
        guild_id: required_string(wire.guild_id)?,
        target_channel_id: wire.target_channel_id,
        target_message_id: wire.target_message_id,
    })
}

/// Пустое тело — не invite-объект; не-JSON/несовпадение типов — `InvalidResponse`.
fn parse_json<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, InviteError> {
    if body.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Err(InviteError::InvalidResponse);
    }
    serde_json::from_slice(body).map_err(|_| InviteError::InvalidResponse)
}
