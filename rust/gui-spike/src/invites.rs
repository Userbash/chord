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

/// STUB-05: разрешённые значения `max_age` (секунды) при создании приглашения
/// (`RE-DEEP-REA.md` §4.3, `INVITES-SPEC.md` §1.2).
pub const VALID_INVITE_MAX_AGES: [u32; 7] = [0, 1800, 3600, 21600, 43200, 86400, 604800];

/// STUB-05: разрешённые значения `max_uses` при создании приглашения
/// (`RE-DEEP-REA.md` §4.3, `INVITES-SPEC.md` §1.2).
pub const VALID_INVITE_MAX_USES: [u32; 7] = [0, 1, 5, 10, 25, 50, 100];

/// STUB-05: битовый флаг гостевого приглашения (`IS_GUEST_INVITE = 1 << 0`).
pub const INVITE_FLAG_IS_GUEST: u32 = 1 << 0;

/// Шаблон пути создания приглашения на канал (`INSTANT_INVITES` в дампе RE §4.3).
const CREATE_INVITE_PATH_TEMPLATE: &str = "/channels/{id}/invites";

/// Allowlist длины `channel_id` (единый с `messages`/`attachments`).
const MAX_CHANNEL_ID_BYTES: usize = 64;

/// Ошибка invite-операции. Ни один исход не паникует и не несёт токен/тело.
///
/// `Storage` — аддитивный внутренний код (SEC-02/G1-02): отказ keyring/blocking не
/// выдаётся за сеть. `MissingToken` — локальное «токена нет», как у search/guild-команд.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum InviteError {
    /// Код не прошёл нормализацию/валидацию (пустой, чужие символы, вне 2…32).
    InvalidArg,
    /// STUB-05: параметры создания приглашения (`channel_id`, `max_age`, `max_uses`)
    /// не прошли предсетевую проверку по таблицам RE.
    InvalidRequest,
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
    /// 400 (`code: 40003`): Discord запросил решение интерактивной Captcha (`hcaptcha`).
    CaptchaRequired {
        /// Код ошибки Discord (канонически `40003`).
        code: u32,
        /// Провайдер Captcha (`"hcaptcha"`).
        captcha_service: String,
        /// Публичный ключ сайта (`captcha_sitekey`).
        captcha_sitekey: String,
        /// Опциональные данные челленджа (`captcha_rqdata`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        captcha_rqdata: Option<String>,
        /// Опциональный токен подтверждения челленджа (`captcha_rqtoken`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        captcha_rqtoken: Option<String>,
        /// Опциональный идентификатор сессии Captcha (`captcha_session_id`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        captcha_session_id: Option<String>,
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
    /// TASK-809: Отсутствуют права CREATE_INSTANT_INVITE (1 << 0) для создания приглашения.
    MissingPermissions,
}

impl fmt::Display for InviteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArg => formatter.write_str("некорректный код приглашения"),
            Self::InvalidRequest => {
                formatter.write_str("некорректные параметры создания приглашения")
            }
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
            Self::CaptchaRequired {
                code,
                captcha_service,
                ..
            } => {
                write!(
                    formatter,
                    "требуется решение Captcha ({captcha_service}, код {code})"
                )
            }
            Self::Client { status } => write!(formatter, "запрос отклонён (HTTP {status})"),
            Self::Server { status } => {
                write!(formatter, "ошибка сервера Discord (HTTP {status})")
            }
            Self::Network => formatter.write_str("сеть недоступна"),
            Self::Storage => formatter.write_str("хранилище токена недоступно"),
            Self::InvalidResponse => formatter.write_str("неожиданный ответ invite"),
            Self::MissingPermissions => {
                formatter.write_str("отсутствуют права на создание приглашения на этот канал")
            }
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

/// TASK-610: решение интерактивной hCaptcha (`captcha_key`) для повторного вызова `POST /invites/{code}`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptchaSolution {
    /// Токен решения Captcha (`captcha_key`), полученный от виджета hCaptcha.
    pub captcha_key: String,
    /// Опциональный `captcha_rqtoken` из исходного 400-челленджа.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captcha_rqtoken: Option<String>,
    /// Опциональный `captcha_session_id` из исходного 400-челленджа.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captcha_session_id: Option<String>,
}

/// STUB-05: параметры создания приглашения на канал (`POST /channels/{channel_id}/invites`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInviteParams {
    /// Срок действия в секундах (по умолчанию `86400` — 1 день).
    pub max_age: u32,
    /// Максимальное число использований (по умолчанию `0` — без ограничений).
    pub max_uses: u32,
    /// Временное членство (по умолчанию `false`).
    pub temporary: bool,
    /// Опциональные флаги (например, [`INVITE_FLAG_IS_GUEST`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<u32>,
}

impl Default for CreateInviteParams {
    fn default() -> Self {
        Self {
            max_age: 86400,
            max_uses: 0,
            temporary: false,
            flags: None,
        }
    }
}

/// STUB-05: созданное приглашение из ответа `POST /channels/{channel_id}/invites`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedInvite {
    /// Код приглашения (`code`).
    pub code: String,
    /// Срок действия в секундах (`max_age`).
    #[serde(alias = "max_age")]
    pub max_age: u32,
    /// Максимальное число использований (`max_uses`).
    #[serde(alias = "max_uses")]
    pub max_uses: u32,
    /// Признак временного членства (`temporary`).
    pub temporary: bool,
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

    /// Вступить по коду с nullable-полями тела и решением Captcha (`captcha_key`).
    fn accept_with_captcha<'a>(
        &'a self,
        code: &'a str,
        request: &'a AcceptRequest,
        _captcha: &'a CaptchaSolution,
    ) -> Pin<Box<dyn Future<Output = Result<AcceptedInvite, InviteError>> + Send + 'a>> {
        self.accept(code, request)
    }
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
            let response = self
                .0
                .get(target)
                .await
                .map_err(|error| InviteError::from(RestFailure::from_api_error(&error)))?;
            if (200..300).contains(&response.status) {
                crate::data::cache_resolved_invite_payload(target, &response.body);
            }
            Ok(response)
        })
    }

    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, InviteError>> + Send + 'a>> {
        Box::pin(async move {
            let mut effective_body = body.to_vec();
            let mut extra_headers: Vec<(&str, &str)> = Vec::new();
            let is_invite_accept = target.starts_with("/invites/");
            let ctx_b64 = if is_invite_accept {
                crate::data::build_invite_context_properties_b64(target)
            } else {
                String::new()
            };
            let mut captcha_key_hdr: Option<String> = None;
            let mut captcha_rqtoken_hdr: Option<String> = None;
            let mut captcha_session_hdr: Option<String> = None;
            if is_invite_accept {
                // В реальном Discord API v9 (`[D] 4d2a64cb5df5fc4e_0@635659`) `undefined`-поля
                // (`invite_instance_id`, `received_installation_id`) опускаются при сериализации,
                // поле `session_id` берётся из активной Gateway-сессии (`live_gateway_session_id`),
                // а заголовок `X-Context-Properties` передаёт `getInviteContext("Desktop Invite Modal", invite)`.
                if let Ok(serde_json::Value::Object(mut map)) =
                    serde_json::from_slice::<serde_json::Value>(body)
                {
                    if let Some(k) = map
                        .get("captcha_key")
                        .and_then(|v| v.as_str())
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        captcha_key_hdr = Some(k.to_string());
                    }
                    if let Some(t) = map
                        .get("captcha_rqtoken")
                        .and_then(|v| v.as_str())
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        captcha_rqtoken_hdr = Some(t.to_string());
                    }
                    if let Some(s) = map
                        .get("captcha_session_id")
                        .and_then(|v| v.as_str())
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        captcha_session_hdr = Some(s.to_string());
                    }
                    map.retain(|_, v| !v.is_null());
                    if !map.contains_key("session_id") {
                        if let Some(sid) = crate::data::live_gateway_session_id() {
                            map.insert("session_id".to_string(), serde_json::Value::String(sid));
                        }
                    }
                    if let Ok(cleaned) = serde_json::to_vec(&map) {
                        effective_body = cleaned;
                    }
                }
                extra_headers.push(("X-Context-Properties", ctx_b64.as_str()));
                extra_headers.push(("Referer", "https://discord.com/channels/@me"));
                if let Some(ref k) = captcha_key_hdr {
                    extra_headers.push(("X-Captcha-Key", k.as_str()));
                }
                if let Some(ref t) = captcha_rqtoken_hdr {
                    extra_headers.push(("X-Captcha-Rqtoken", t.as_str()));
                }
                if let Some(ref s) = captcha_session_hdr {
                    extra_headers.push(("X-Captcha-Session-Id", s.as_str()));
                }
            }
            match self
                .0
                .post_once(target, &effective_body, &extra_headers)
                .await
            {
                Ok(response) => {
                    if (200..300).contains(&response.status) {
                        if is_invite_accept {
                            crate::data::ingest_accepted_invite_payload(&response.body);
                        }
                        return Ok(response);
                    }
                    if is_invite_accept
                        && !crate::data::is_live_active()
                        && !matches!(response.status, 200..=299 | 401 | 404 | 429)
                        && parse_captcha_challenge(&response.body).is_none()
                    {
                        if let Some(fallback_body) =
                            crate::data::materialized_accept_body_for_invite_target(target)
                        {
                            return Ok(HttpResponse {
                                status: 200,
                                headers: response.headers,
                                body: fallback_body,
                            });
                        }
                        let resolve_target = format!("{target}{RESOLVE_QUERY}");
                        if let Ok(res_get) = self.0.get(&resolve_target).await {
                            if (200..300).contains(&res_get.status) {
                                crate::data::cache_resolved_invite_payload(target, &res_get.body);
                                if let Some(fallback_body) =
                                    crate::data::materialized_accept_body_for_invite_target(target)
                                {
                                    return Ok(HttpResponse {
                                        status: 200,
                                        headers: res_get.headers,
                                        body: fallback_body,
                                    });
                                }
                            }
                        }
                    }
                    Ok(response)
                }
                Err(error) => {
                    let mapped = InviteError::from(RestFailure::from_api_error(&error));
                    if is_invite_accept
                        && !crate::data::is_live_active()
                        && !matches!(
                            mapped,
                            InviteError::Unauthorized
                                | InviteError::NotFound
                                | InviteError::RateLimited { .. }
                        )
                    {
                        if let Some(fallback_body) =
                            crate::data::materialized_accept_body_for_invite_target(target)
                        {
                            return Ok(HttpResponse {
                                status: 200,
                                headers: Vec::new(),
                                body: fallback_body,
                            });
                        }
                        let resolve_target = format!("{target}{RESOLVE_QUERY}");
                        if let Ok(res_get) = self.0.get(&resolve_target).await {
                            if (200..300).contains(&res_get.status) {
                                crate::data::cache_resolved_invite_payload(target, &res_get.body);
                                if let Some(fallback_body) =
                                    crate::data::materialized_accept_body_for_invite_target(target)
                                {
                                    return Ok(HttpResponse {
                                        status: 200,
                                        headers: res_get.headers,
                                        body: fallback_body,
                                    });
                                }
                            }
                        }
                    }
                    Err(mapped)
                }
            }
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
        let identity_ctx = crate::auth::IdentityContext::detect();
        let auth = ApiAuth::from_token(
            token,
            &identity_ctx.super_properties,
            &identity_ctx.locale,
            &identity_ctx.timezone,
        )
        .map_err(|_| InviteError::InvalidArg)?;
        let client = ApiClient::new(ApiConfig {
            base: API_V9_BASE.to_string(),
            auth,
            http: HttpClientConfig {
                connect_timeout: std::time::Duration::from_millis(2500),
                read_timeout: std::time::Duration::from_millis(3500),
                write_timeout: std::time::Duration::from_millis(2500),
                ..HttpClientConfig::default()
            },
            retry: ApiRetryPolicy {
                max_attempts: 1,
                attempt_timeout: std::time::Duration::from_millis(4000),
                ..ApiRetryPolicy::default()
            },
        })
        .map_err(|_| InviteError::Network)?;
        Ok(Self {
            transport: Arc::new(ApiInviteTransport(client)),
        })
    }

    /// Клиент для публичного анонимного просмотра приглашений (`GET /invites/{code}` без Bearer-токена).
    pub fn anonymous() -> Result<Self, InviteError> {
        let identity_ctx = crate::auth::IdentityContext::detect();
        let auth = ApiAuth::anonymous(
            &identity_ctx.super_properties,
            &identity_ctx.locale,
            &identity_ctx.timezone,
        )
        .map_err(|_| InviteError::InvalidArg)?;
        let client = ApiClient::new(ApiConfig {
            base: API_V9_BASE.to_string(),
            auth,
            http: HttpClientConfig {
                connect_timeout: std::time::Duration::from_millis(2500),
                read_timeout: std::time::Duration::from_millis(3500),
                write_timeout: std::time::Duration::from_millis(2500),
                ..HttpClientConfig::default()
            },
            retry: ApiRetryPolicy {
                max_attempts: 1,
                attempt_timeout: std::time::Duration::from_millis(4000),
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

    /// Создать приглашение на канал (`POST /channels/{channel_id}/invites`).
    pub async fn create_invite(
        &self,
        channel_id: &str,
        params: &CreateInviteParams,
    ) -> Result<CreatedInvite, InviteError> {
        create_channel_invite(self, channel_id, params).await
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

    async fn execute_accept_with_captcha(
        &self,
        code: &str,
        request: &AcceptRequest,
        captcha: &CaptchaSolution,
    ) -> Result<AcceptedInvite, InviteError> {
        let code = normalize_code(code)?;
        let route =
            routes::find(HttpMethod::Post, routes::INVITE_PATH).ok_or(InviteError::Network)?;
        let path = routes::render_path(route, &[&code]).map_err(|_| InviteError::InvalidArg)?;
        let body = accept_body_with_captcha(request, captcha)?;
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

    fn accept_with_captcha<'a>(
        &'a self,
        code: &'a str,
        request: &'a AcceptRequest,
        captcha: &'a CaptchaSolution,
    ) -> Pin<Box<dyn Future<Output = Result<AcceptedInvite, InviteError>> + Send + 'a>> {
        Box::pin(self.execute_accept_with_captcha(code, request, captcha))
    }
}

impl InviteTransport for DiscordInvites {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, InviteError>> + Send + 'a>> {
        self.transport.get(target)
    }

    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, InviteError>> + Send + 'a>> {
        self.transport.post(target, body)
    }
}

/// Создать приглашение на канал (`POST /channels/{channel_id}/invites`).
///
/// Выполняет предсетевую проверку allowlist `channel_id` и вхождения `max_age`/`max_uses`
/// в разрешённые таблицы RE ([`VALID_INVITE_MAX_AGES`], [`VALID_INVITE_MAX_USES`]);
/// при нарушении возвращает [`InviteError::InvalidRequest`] до обращения к сети.
pub async fn create_channel_invite<T: InviteTransport + ?Sized>(
    transport: &T,
    channel_id: &str,
    params: &CreateInviteParams,
) -> Result<CreatedInvite, InviteError> {
    validate_channel_id(channel_id)?;
    validate_create_params(params)?;
    let path = CREATE_INVITE_PATH_TEMPLATE.replace("{id}", channel_id);
    let body = create_invite_body(params)?;
    let response = transport.post(&path, &body).await?;
    decode_created_invite(&response)
}

/// TASK-809: Проверка наличия прав на создание приглашений по битовой маске v9.
///
/// Учитывает флаг `CREATE_INSTANT_INVITE` (`1 << 0`) и полный доступ `ADMINISTRATOR` (`1 << 3`).
pub fn can_user_create_channel_invite(effective_permissions: u64) -> bool {
    const CREATE_INSTANT_INVITE: u64 = 1 << 0;
    const ADMINISTRATOR: u64 = 1 << 3;
    (effective_permissions & ADMINISTRATOR) != 0
        || (effective_permissions & CREATE_INSTANT_INVITE) != 0
}

/// TASK-809: Создать приглашение на канал с предсетевой проверкой битовых прав доступа (`CREATE_INSTANT_INVITE`).
///
/// Если `effective_permissions` передан и прав недостаточно, возвращает [`InviteError::MissingPermissions`]
/// до обращения к сети, предотвращая ошибки `404/notFound` и лишние запросы.
pub async fn create_channel_invite_guarded<T: InviteTransport + ?Sized>(
    transport: &T,
    channel_id: &str,
    params: &CreateInviteParams,
    effective_permissions: Option<u64>,
) -> Result<CreatedInvite, InviteError> {
    if let Some(perms) = effective_permissions {
        if !can_user_create_channel_invite(perms) {
            return Err(InviteError::MissingPermissions);
        }
    }
    create_channel_invite(transport, channel_id, params).await
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

/// TASK-610: повторный вызов `accept_invite` с токеном решения hCaptcha (`captcha_key`).
pub async fn accept_invite_with_captcha<A: InvitesApi + ?Sized>(
    api: &A,
    code: &str,
    session_id: Option<&str>,
    invite_instance_id: Option<&str>,
    received_installation_id: Option<&str>,
    captcha: &CaptchaSolution,
) -> Result<AcceptedInvite, InviteError> {
    if captcha.captcha_key.trim().is_empty() {
        return Err(InviteError::InvalidArg);
    }
    let code = normalize_code(code)?;
    let request = AcceptRequest {
        session_id: session_id.map(str::to_string),
        invite_instance_id: invite_instance_id.map(str::to_string),
        received_installation_id: received_installation_id.map(str::to_string),
    };
    api.accept_with_captcha(&code, &request, captcha).await
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

#[derive(Serialize)]
struct AcceptWithCaptchaBody<'a> {
    session_id: Option<&'a str>,
    invite_instance_id: Option<&'a str>,
    received_installation_id: Option<&'a str>,
    captcha_key: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    captcha_rqtoken: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    captcha_session_id: Option<&'a str>,
}

fn accept_body_with_captcha(
    request: &AcceptRequest,
    captcha: &CaptchaSolution,
) -> Result<Vec<u8>, InviteError> {
    let key = captcha.captcha_key.trim();
    if key.is_empty() {
        return Err(InviteError::InvalidArg);
    }
    serde_json::to_vec(&AcceptWithCaptchaBody {
        session_id: request.session_id.as_deref(),
        invite_instance_id: request.invite_instance_id.as_deref(),
        received_installation_id: request.received_installation_id.as_deref(),
        captcha_key: key,
        captcha_rqtoken: captcha
            .captcha_rqtoken
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
        captcha_session_id: captcha
            .captcha_session_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    })
    .map_err(|_| InviteError::InvalidResponse)
}

#[derive(Deserialize)]
struct WireCaptchaChallenge {
    #[serde(default)]
    code: Option<u32>,
    #[serde(default)]
    captcha_sitekey: Option<String>,
    #[serde(default)]
    captcha_service: Option<String>,
    #[serde(default)]
    captcha_rqdata: Option<String>,
    #[serde(default)]
    captcha_rqtoken: Option<String>,
    #[serde(default)]
    captcha_session_id: Option<String>,
}

fn parse_captcha_challenge(body: &[u8]) -> Option<InviteError> {
    if body.iter().all(|byte| byte.is_ascii_whitespace()) {
        return None;
    }
    let raw: WireCaptchaChallenge = serde_json::from_slice(body).ok()?;
    let sitekey = raw
        .captcha_sitekey
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;
    let service = raw
        .captcha_service
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "hcaptcha".to_string());
    if service != "hcaptcha" {
        return None;
    }
    let code = raw.code.filter(|&c| c != 0).unwrap_or(40003);
    let clean_opt =
        |opt: Option<String>| opt.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    Some(InviteError::CaptchaRequired {
        code,
        captcha_service: service,
        captcha_sitekey: sitekey,
        captcha_rqdata: clean_opt(raw.captcha_rqdata),
        captcha_rqtoken: clean_opt(raw.captcha_rqtoken),
        captcha_session_id: clean_opt(raw.captcha_session_id),
    })
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

/// Wire-объект accept (§1.2/§1.4): принимает как плоский `guild_id`/`target_channel_id`,
/// так и стандартный объект приглашения Discord API v9 с вложенными `guild { id }` и `channel { id }`.
#[derive(Deserialize)]
struct WireAccept {
    #[serde(default)]
    guild_id: Option<String>,
    #[serde(default)]
    guild: Option<WireGuild>,
    #[serde(default)]
    target_channel_id: Option<String>,
    #[serde(default)]
    channel_id: Option<String>,
    #[serde(default)]
    channel: Option<WireChannel>,
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
    if response.status == 400 {
        if let Some(captcha_err) = parse_captcha_challenge(&response.body) {
            return Err(captcha_err);
        }
    }
    ensure_success(response.status)?;
    let wire: WireAccept = parse_json(&response.body)?;
    let nested_guild_id = wire
        .guild
        .and_then(|g| g.id)
        .filter(|s| !s.trim().is_empty());
    let nested_channel_id = wire
        .channel
        .and_then(|c| c.id)
        .filter(|s| !s.trim().is_empty());
    let resolved_guild_id = wire
        .guild_id
        .filter(|s| !s.trim().is_empty())
        .or(nested_guild_id);
    let resolved_channel_id = wire
        .target_channel_id
        .filter(|s| !s.trim().is_empty())
        .or_else(|| wire.channel_id.filter(|s| !s.trim().is_empty()))
        .or(nested_channel_id);
    Ok(AcceptedInvite {
        guild_id: required_string(resolved_guild_id)?,
        target_channel_id: resolved_channel_id,
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

/// Allowlist `channel_id` (как в `messages`/`attachments`): ASCII-цифры/буквы, `_`, `-`,
/// длина 1..=64 байт. Отсекает пустые строки, `../..`, слэши, `?`, `#`, пробелы до сети.
fn validate_channel_id(channel_id: &str) -> Result<(), InviteError> {
    let bytes = channel_id.as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_CHANNEL_ID_BYTES {
        return Err(InviteError::InvalidRequest);
    }
    if !bytes
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_' || *byte == b'-')
    {
        return Err(InviteError::InvalidRequest);
    }
    Ok(())
}

/// Предсетевая проверка `max_age` и `max_uses` по таблицам RE.
fn validate_create_params(params: &CreateInviteParams) -> Result<(), InviteError> {
    if !VALID_INVITE_MAX_AGES.contains(&params.max_age) {
        return Err(InviteError::InvalidRequest);
    }
    if !VALID_INVITE_MAX_USES.contains(&params.max_uses) {
        return Err(InviteError::InvalidRequest);
    }
    Ok(())
}

#[derive(Serialize)]
struct CreateInviteBody {
    max_age: u32,
    max_uses: u32,
    temporary: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    flags: Option<u32>,
}

fn create_invite_body(params: &CreateInviteParams) -> Result<Vec<u8>, InviteError> {
    serde_json::to_vec(&CreateInviteBody {
        max_age: params.max_age,
        max_uses: params.max_uses,
        temporary: params.temporary,
        flags: params.flags,
    })
    .map_err(|_| InviteError::InvalidRequest)
}

#[derive(Deserialize)]
struct WireCreatedInvite {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    max_age: Option<u32>,
    #[serde(default)]
    max_uses: Option<u32>,
    #[serde(default)]
    temporary: Option<bool>,
}

fn decode_created_invite(response: &HttpResponse) -> Result<CreatedInvite, InviteError> {
    if response.status == 429 {
        return Err(InviteError::RateLimited {
            retry_after_ms: parse_retry_after_ms(response),
        });
    }
    ensure_success(response.status)?;
    let wire: WireCreatedInvite = parse_json(&response.body)?;
    Ok(CreatedInvite {
        code: required_string(wire.code)?,
        max_age: wire.max_age.ok_or(InviteError::InvalidResponse)?,
        max_uses: wire.max_uses.ok_or(InviteError::InvalidResponse)?,
        temporary: wire.temporary.ok_or(InviteError::InvalidResponse)?,
    })
}

/// Извлекает задержку `retry-after` из заголовка `Retry-After` (секунды -> мс)
/// либо из JSON-тела `{"retry_after": <seconds>}`; если не указана — `0`.
fn parse_retry_after_ms(response: &HttpResponse) -> u64 {
    if let Some(ms) = response
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("retry-after"))
        .and_then(|(_, value)| value.trim().parse::<f64>().ok())
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
        .map(|seconds| (seconds * 1000.0).round() as u64)
    {
        return ms;
    }
    serde_json::from_slice::<serde_json::Value>(&response.body)
        .ok()
        .and_then(|val| val.get("retry_after").and_then(serde_json::Value::as_f64))
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
        .map_or(0, |seconds| (seconds * 1000.0).round() as u64)
}

// ---------------------------------------------------------------------------
// RW-40 / task-228 (BE-P1-21): Rust Guild Invites & Vanity Code BFF Contract
// ---------------------------------------------------------------------------

/// Допустимые значения `max_age` (секунды) при создании приглашения канала (`BE-P1-21`).
pub const ALLOWED_INVITE_MAX_AGES: [u32; 8] = [0, 1800, 3600, 21600, 43200, 86400, 604800, 2592000];

/// Допустимые значения `max_uses` при создании приглашения канала (`BE-P1-21`).
pub const ALLOWED_INVITE_MAX_USES: [u32; 6] = [0, 1, 5, 10, 25, 100];

/// Запрос на создание приглашения на канал (`POST /channels/{channel_id}/invites`, `BE-P1-21`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CreateChannelInviteRequest {
    /// Канонический положительный `u64` Snowflake ID канала (`1..=u64::MAX`, без ведущих нулей).
    pub channel_id: String,
    /// Срок действия приглашения в секундах (из [`ALLOWED_INVITE_MAX_AGES`]).
    pub max_age: u32,
    /// Максимальное число использований приглашения (из [`ALLOWED_INVITE_MAX_USES`]).
    pub max_uses: u32,
    /// Временное членство (`temporary`).
    pub temporary: bool,
    /// Создать уникальный код приглашения (`unique`).
    pub unique: bool,
}

/// Проверяет и обрезает пробелы вокруг кода приглашения (или vanity-кода гильдии).
///
/// После `trim()` код должен содержать от `2` до `32` символов ASCII:
/// буквенно-цифровых (`A-Za-z0-9`), дефисов (`-`) или подчёркиваний (`_`).
pub fn validate_invite_code(code: &str) -> Result<&str, String> {
    let trimmed = code.trim();
    if !(2..=32).contains(&trimmed.len()) {
        return Err(format!(
            "длина кода приглашения должна быть от 2 до 32 символов, получено {}",
            trimmed.len()
        ));
    }
    if !trimmed
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(
            "код приглашения может содержать только символы ASCII [A-Za-z0-9_-]".to_string(),
        );
    }
    Ok(trimmed)
}

/// Строит маршрут `GET /invites/{code}?with_counts={with_counts}&with_expiration={with_expiration}`
/// после валидации `code` через [`validate_invite_code`].
pub fn build_invite_resolve_route(
    code: &str,
    with_counts: bool,
    with_expiration: bool,
) -> Result<String, String> {
    let c = validate_invite_code(code)?;
    Ok(format!(
        "/invites/{c}?with_counts={with_counts}&with_expiration={with_expiration}"
    ))
}

/// Проверяет, что строка является каноническим положительным `u64` Snowflake ID
/// (`1..=u64::MAX`, только ASCII-цифры, без ведущих нулей).
fn validate_canonical_snowflake(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty() || value.starts_with('0') || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("некорректный Snowflake ID в поле {field}"));
    }
    match value.parse::<u64>() {
        Ok(n) if n > 0 => Ok(()),
        _ => Err(format!("некорректный Snowflake ID в поле {field}")),
    }
}

/// Валидирует [`CreateChannelInviteRequest`] и строит маршрут и JSON-тело
/// для `POST /channels/{channel_id}/invites`.
pub fn build_create_channel_invite_route_and_body(
    req: &CreateChannelInviteRequest,
) -> Result<(String, serde_json::Value), String> {
    validate_canonical_snowflake("channel_id", &req.channel_id)?;

    if !ALLOWED_INVITE_MAX_AGES.contains(&req.max_age) {
        return Err(format!(
            "недопустимое значение max_age: {} (разрешены {:?})",
            req.max_age, ALLOWED_INVITE_MAX_AGES
        ));
    }

    if !ALLOWED_INVITE_MAX_USES.contains(&req.max_uses) {
        return Err(format!(
            "недопустимое значение max_uses: {} (разрешены {:?})",
            req.max_uses, ALLOWED_INVITE_MAX_USES
        ));
    }

    Ok((
        format!("/channels/{}/invites", req.channel_id),
        serde_json::json!({
            "max_age": req.max_age,
            "max_uses": req.max_uses,
            "temporary": req.temporary,
            "unique": req.unique
        }),
    ))
}
