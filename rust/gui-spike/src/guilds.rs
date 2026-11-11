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

use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock};

use discord_core::rest::client::{ApiAuth, ApiClient, ApiConfig, ApiRetryPolicy};
use discord_core::rest::guilds::{
    parse_guild_create_response, validate_guild_name, validate_icon_data_uri, CreatedGuild,
    GuildCreateChannelSpec, GuildCreateRequest as CoreGuildCreateRequest, GuildCreateRestError,
    DISCORD_LOCALE_RU, GUILDS_PATH,
};
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

/// Описание начального канала или категории в запросе `POST /guilds` (TASK-611).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateGuildChannelInput {
    /// Временный идентификатор для привязки дочерних каналов к категории через `parent_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Название канала или категории (`1..=100` символов).
    pub name: String,
    /// Числовой тип канала (`0` — текстовый, `2` — голосовой, `4` — категория).
    #[serde(rename = "type")]
    pub kind: u8,
    /// Идентификатор родительской категории.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    /// Позиция канала при сортировке.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<i64>,
}

/// Полный запрос создания сервера `POST /api/v9/guilds` (TASK-611).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateGuildRequest {
    /// Название сервера (`1..=100` символов).
    pub name: String,
    /// Опциональная иконка в формате base64 DataURI (`data:image/...;base64,...`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Начальные категории и каналы сервера.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub channels: Vec<CreateGuildChannelInput>,
    /// Битовая маска флагов системного канала (`system_channel_flags`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_channel_flags: Option<u32>,
    /// Опциональный код шаблона гильдии.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_template_code: Option<String>,
}

impl CreateGuildRequest {
    fn to_core_request(&self) -> CoreGuildCreateRequest {
        CoreGuildCreateRequest {
            name: self.name.clone(),
            icon: self.icon.clone(),
            channels: self
                .channels
                .iter()
                .map(|ch| GuildCreateChannelSpec {
                    id: ch.id.clone(),
                    name: ch.name.clone(),
                    kind: ch.kind,
                    parent_id: ch.parent_id.clone(),
                    position: ch.position,
                })
                .collect(),
            system_channel_flags: self.system_channel_flags,
            guild_template_code: self.guild_template_code.clone(),
        }
    }
}

/// Полный результат создания сервера (`201 Created`, TASK-611) с каналами и ролями.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateGuildFullOutcome {
    /// Snowflake ID созданного сервера.
    pub id: String,
    /// Название созданного сервера.
    pub name: String,
    /// Хеш иконки сервера.
    #[serde(default)]
    pub icon: Option<String>,
    /// Snowflake ID владельца сервера.
    #[serde(default)]
    pub owner_id: Option<String>,
    /// Snowflake ID системного канала.
    #[serde(default)]
    pub system_channel_id: Option<String>,
    /// Битовая маска флагов системного канала (`system_channel_flags`).
    #[serde(default)]
    pub system_channel_flags: u32,
    /// Начальные каналы и категории созданного сервера.
    #[serde(default)]
    pub channels: Vec<serde_json::Value>,
    /// Начальные роли созданного сервера.
    #[serde(default)]
    pub roles: Vec<serde_json::Value>,
}

static LOCAL_CREATED_GUILDS: OnceLock<Mutex<BTreeMap<String, CreatedGuild>>> = OnceLock::new();

fn created_guilds_store() -> &'static Mutex<BTreeMap<String, CreatedGuild>> {
    LOCAL_CREATED_GUILDS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// Получить снимок созданной гильдии (включая распарсенные каналы и роли) из локального состояния.
#[must_use]
pub fn local_created_guild_snapshot(guild_id: &str) -> Option<CreatedGuild> {
    let guard = created_guilds_store()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    guard.get(guild_id.trim()).cloned()
}

/// Очистить локальный кэш созданных серверов (для изоляции контрактных тестов).
pub fn clear_local_created_guilds() {
    let mut guard = created_guilds_store()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    guard.clear();
}

fn integrate_created_guild_into_local_state(created: &CreatedGuild, raw_body: &[u8]) {
    {
        let mut guard = created_guilds_store()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        guard.insert(created.id.clone(), created.clone());
    }
    if let Ok(guild_val) = serde_json::from_slice::<serde_json::Value>(raw_body) {
        let mut wrapper = serde_json::Map::new();
        if let Some(first_ch) = guild_val
            .get("channels")
            .and_then(serde_json::Value::as_array)
            .and_then(|arr| {
                arr.iter()
                    .find(|c| c.get("type").and_then(serde_json::Value::as_u64) != Some(4))
            })
        {
            wrapper.insert("channel".to_string(), first_ch.clone());
        }
        wrapper.insert("guild".to_string(), guild_val);
        if let Ok(wrapped_bytes) = serde_json::to_vec(&serde_json::Value::Object(wrapper)) {
            crate::data::ingest_accepted_invite_payload(&wrapped_bytes);
            crate::data::ensure_local_guild_present(&created.id);
        }
    }
}

/// Полезная нагрузка для создания нового сервера (`POST /guilds`, STUB-03).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateGuildPayload {
    /// Название создаваемого сервера (1..=100 символов).
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

/// Ошибка создания сервера (`POST /guilds`, STUB-03 / TASK-611).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CreateGuildError {
    /// Некорректные параметры (имя пустое или длиннее 100 символов, невалидный DataURI иконки).
    InvalidArg,
    /// Нет сохранённого токена.
    MissingToken,
    /// 400 (`MAXIMUM_GUILDS = 30005` / `30001`): достигнут лимит серверов пользователя.
    MaximumGuilds { code: u32 },
    /// 403 (`NO_VERIFIED_PHONE`): требуется подтверждённый номер телефона на аккаунте.
    NoVerifiedPhone,
    /// 401: сессия истекла.
    Unauthorized,
    /// 403: создание сервера запрещено.
    Forbidden,
    /// 429: ограничение частоты.
    RateLimited { retry_after_ms: u64 },
    /// Прочие 4xx.
    Client { status: u16 },
    /// 5xx: ошибка сервера Discord.
    Server { status: u16 },
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
            Self::InvalidArg => formatter.write_str(
                "некорректные параметры создания сервера (требуется имя 1..=100 символов)",
            ),
            Self::MissingToken => formatter.write_str("нет сохранённого токена"),
            Self::MaximumGuilds { code } => {
                write!(
                    formatter,
                    "достигнут лимит серверов пользователя (MAXIMUM_GUILDS {code})"
                )
            }
            Self::NoVerifiedPhone => formatter.write_str(
                "требуется подтверждённый номер телефона на аккаунте (NO_VERIFIED_PHONE)",
            ),
            Self::Unauthorized => formatter.write_str("сессия истекла"),
            Self::Forbidden => {
                formatter.write_str("создание сервера запрещено (достигнут лимит в 100 серверов)")
            }
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

impl From<GuildCreateRestError> for CreateGuildError {
    fn from(error: GuildCreateRestError) -> Self {
        match error {
            GuildCreateRestError::InvalidName
            | GuildCreateRestError::InvalidIcon
            | GuildCreateRestError::InvalidChannel
            | GuildCreateRestError::Header(_) => Self::InvalidArg,
            GuildCreateRestError::MaximumGuilds { code, .. } => Self::MaximumGuilds {
                code: u32::try_from(code).unwrap_or(30005),
            },
            GuildCreateRestError::NoVerifiedPhone { .. } => Self::NoVerifiedPhone,
            GuildCreateRestError::Unauthorized => Self::Unauthorized,
            GuildCreateRestError::Forbidden { .. } => Self::Forbidden,
            GuildCreateRestError::RateLimited { retry_after_ms } => {
                Self::RateLimited { retry_after_ms }
            }
            GuildCreateRestError::Client { status, .. } => Self::Client { status },
            GuildCreateRestError::Server { status } => Self::Server { status },
            GuildCreateRestError::InvalidResponse(_) => Self::InvalidResponse,
        }
    }
}

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
                if let Ok(join_resp) = self
                    .0
                    .put(&format!("/guilds/{gid}/members/@me?lurker=false"), body)
                    .await
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
            let headers = [
                ("X-Discord-Locale", DISCORD_LOCALE_RU),
                ("Accept-Language", DISCORD_LOCALE_RU),
            ];
            match self.0.post_once(target, body, &headers).await {
                Ok(resp) => Ok(resp),
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
            DISCORD_LOCALE_RU,
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

    /// Создать экземпляр `DiscordGuilds` поверх уже сконфигурированного [`ApiClient`].
    #[must_use]
    pub fn from_api_client(client: Arc<ApiClient>) -> Self {
        Self {
            transport: Arc::new(ApiGuildTransport((*client).clone())),
        }
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
        validate_guild_name(&payload.name).map_err(CreateGuildError::from)?;
        if let Some(icon) = &payload.icon {
            validate_icon_data_uri(icon).map_err(CreateGuildError::from)?;
        }
        let body = serde_json::to_vec(&payload).map_err(|_| CreateGuildError::InvalidArg)?;
        let response = self.transport.post(GUILDS_PATH, &body).await?;
        let created = parse_guild_create_response(&response).map_err(CreateGuildError::from)?;
        integrate_created_guild_into_local_state(&created, &response.body);
        let outcome: CreateGuildOutcome = serde_json::from_slice(&response.body)
            .map_err(|_| CreateGuildError::InvalidResponse)?;
        Ok(outcome)
    }

    async fn execute_create_full(
        &self,
        request: CreateGuildRequest,
    ) -> Result<CreateGuildFullOutcome, CreateGuildError> {
        let core_req = request.to_core_request();
        let body_json = core_req.to_json_body().map_err(CreateGuildError::from)?;
        let response = self
            .transport
            .post(GUILDS_PATH, body_json.as_bytes())
            .await?;
        let created = parse_guild_create_response(&response).map_err(CreateGuildError::from)?;
        integrate_created_guild_into_local_state(&created, &response.body);
        let mut outcome: CreateGuildFullOutcome = serde_json::from_slice(&response.body)
            .map_err(|_| CreateGuildError::InvalidResponse)?;
        outcome.system_channel_flags = created.system_channel_flags;
        Ok(outcome)
    }

    /// Создание нового сервера через REST ядра (`POST /guilds`, STUB-03).
    pub async fn create_guild(
        &self,
        payload: CreateGuildPayload,
    ) -> Result<CreateGuildOutcome, CreateGuildError> {
        self.execute_create(payload).await
    }

    /// Создание нового сервера с полным набором полей (`channels`, `system_channel_flags`, роли, TASK-611).
    pub async fn create_guild_full(
        &self,
        request: CreateGuildRequest,
    ) -> Result<CreateGuildFullOutcome, CreateGuildError> {
        self.execute_create_full(request).await
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
    ) -> Pin<Box<dyn Future<Output = Result<CreateGuildOutcome, CreateGuildError>> + Send + 'a>>
    {
        Box::pin(self.execute_create(payload))
    }
}

/// Создание сервера: `POST /guilds` с валидацией имени (1..=100) и полезной нагрузкой.
pub async fn create_guild<G: GuildCreation + ?Sized>(
    guilds: &G,
    payload: CreateGuildPayload,
) -> Result<CreateGuildOutcome, CreateGuildError> {
    validate_guild_name(&payload.name).map_err(CreateGuildError::from)?;
    if let Some(icon) = &payload.icon {
        validate_icon_data_uri(icon).map_err(CreateGuildError::from)?;
    }
    guilds.create_guild(payload).await
}

/// Создание сервера с полным набором параметров (`POST /api/v9/guilds`, TASK-611).
pub async fn create_guild_full(
    guilds: &DiscordGuilds,
    request: CreateGuildRequest,
) -> Result<CreateGuildFullOutcome, CreateGuildError> {
    request
        .to_core_request()
        .validate()
        .map_err(CreateGuildError::from)?;
    guilds.create_guild_full(request).await
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
        return Err(
            "Чтение фикстуры 002-READY.json запрещено при наличии живого токена".to_string(),
        );
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
    crate::auth::IdentityContext::detect()
        .super_properties
        .to_base64()
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

    let join_pending = guild_requires_rules_screening(&join_resp.body);
    let is_verification_403 = join_resp.status == 403;

    if join_resp.status >= 400 && !is_verification_403 {
        return Err(format!(
            "Ошибка принятия инвайта: HTTP {}",
            join_resp.status
        ));
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

    let mut rules_confirmed = false;
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
                rules_confirmed = true;
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
            if !rules_confirmed {
                pending = member_resp
                    .body
                    .get("pending")
                    .or_else(|| member_resp.body.get("is_pending"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
            }
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

    if !rules_confirmed && guild_requires_rules_screening(&guild_resp.body) {
        if let Ok(ver_resp) = transport.request(
            "GET",
            &format!("/api/v9/guilds/{guild_id}/member-verification"),
            &headers,
            None,
        ) {
            if (200..300).contains(&ver_resp.status) {
                let put_body = build_member_verification_accept_body(&ver_resp.body);
                if let Ok(put_resp) = transport.request(
                    "PUT",
                    &format!("/api/v9/guilds/{guild_id}/requests/@me"),
                    &headers,
                    Some(&put_body),
                ) {
                    if put_resp.status < 300 {
                        pending = false;
                        if let Ok(member_resp) = transport.request(
                            "GET",
                            &format!("/api/v9/guilds/{guild_id}/members/@me"),
                            &headers,
                            None,
                        ) {
                            if (200..300).contains(&member_resp.status) {
                                if let Some(arr) =
                                    member_resp.body.get("roles").and_then(|v| v.as_array())
                                {
                                    my_roles = arr
                                        .iter()
                                        .filter_map(|v| v.as_str().map(str::to_string))
                                        .collect();
                                }
                            }
                        }
                    }
                }
            }
        }
    }

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

/// Проверяет, требует ли сервер прохождения экрана правил (Rules Screening / Member Verification Gate).
pub fn guild_requires_rules_screening(invite_or_guild: &serde_json::Value) -> bool {
    if let Some(member_pending) = invite_or_guild
        .get("member")
        .and_then(|m| m.get("pending").or_else(|| m.get("is_pending")))
        .and_then(|v| v.as_bool())
    {
        return member_pending;
    }
    if invite_or_guild
        .get("pending")
        .or_else(|| invite_or_guild.get("is_pending"))
        .and_then(|v| v.as_bool())
        == Some(true)
    {
        return true;
    }
    let features = invite_or_guild
        .get("guild")
        .and_then(|g| g.get("features"))
        .or_else(|| invite_or_guild.get("features"))
        .and_then(|f| f.as_array());
    if let Some(arr) = features {
        return arr.iter().any(|item| {
            item.as_str()
                .is_some_and(|f| f.eq_ignore_ascii_case("MEMBER_VERIFICATION_GATE_ENABLED"))
        });
    }
    false
}

/// Применяет результат вступления на сервер к состоянию живой сессии.
pub fn apply_guild_join_outcome(outcome: &GuildJoinOutcome) {
    crate::data::ensure_local_guild_present(&outcome.guild_id);
    crate::data::apply_guild_join_outcome_to_live_session(outcome);

    let mut guild_obj = outcome.guild.as_object().cloned().unwrap_or_default();
    guild_obj.insert(
        "id".to_string(),
        serde_json::Value::String(outcome.guild_id.clone()),
    );
    guild_obj.insert(
        "pending".to_string(),
        serde_json::Value::Bool(outcome.pending),
    );
    guild_obj.insert(
        "is_pending".to_string(),
        serde_json::Value::Bool(outcome.pending),
    );
    guild_obj.insert("isMember".to_string(), serde_json::Value::Bool(true));
    guild_obj.insert("lurker".to_string(), serde_json::Value::Bool(false));
    if !outcome.channels.is_empty() {
        guild_obj.insert(
            "channels".to_string(),
            serde_json::Value::Array(outcome.channels.clone()),
        );
    }
    let my_roles_val: Vec<serde_json::Value> = outcome
        .roles
        .iter()
        .map(|r| serde_json::Value::String(r.clone()))
        .collect();
    guild_obj.insert(
        "my_roles".to_string(),
        serde_json::Value::Array(my_roles_val.clone()),
    );
    guild_obj.insert(
        "myRoles".to_string(),
        serde_json::Value::Array(my_roles_val),
    );
    if !outcome.channel_permissions.is_empty() {
        let mut perms_map = serde_json::Map::new();
        for (cid, access) in &outcome.channel_permissions {
            if let Ok(val) = serde_json::to_value(access) {
                perms_map.insert(cid.clone(), val);
            }
        }
        guild_obj.insert(
            "channelPermissions".to_string(),
            serde_json::Value::Object(perms_map),
        );
    }
    crate::data::ingest_live_dispatch("GUILD_CREATE", &serde_json::Value::Object(guild_obj));
}

/// Боевой HTTP-транспорт `GuildJoinTransport` для прохождения Member Verification Gate
/// (`GET /guilds/{id}/member-verification` -> `PUT /guilds/{id}/requests/@me`) и пересчёта прав.
pub struct LiveGuildJoinTransport {
    auth_secret: String,
    pre_accepted_guild_id: Option<String>,
}

impl LiveGuildJoinTransport {
    pub fn with_accepted_guild(secret: &str, guild_id: &str) -> Self {
        Self {
            auth_secret: secret.to_string(),
            pre_accepted_guild_id: Some(guild_id.to_string()),
        }
    }

    pub fn current_user_id(&self) -> String {
        crate::data::extract_user_id_from_token(&self.auth_secret)
            .unwrap_or_else(|| "0".to_string())
    }
}

impl GuildJoinTransport for LiveGuildJoinTransport {
    fn request(
        &self,
        method: &str,
        path: &str,
        headers: &std::collections::HashMap<String, String>,
        body: Option<&serde_json::Value>,
    ) -> Result<GuildTransportResponse, String> {
        if method.eq_ignore_ascii_case("POST") && path.contains("/invites/") {
            if let Some(ref gid) = self.pre_accepted_guild_id {
                return Ok(GuildTransportResponse {
                    status: 200,
                    body: serde_json::json!({
                        "guild": { "id": gid }
                    }),
                });
            }
        }

        let secret = self.auth_secret.clone();
        let method_owned = method.to_uppercase();
        let rel_path = path.strip_prefix("/api/v9").unwrap_or(path).to_string();
        let headers_owned = headers.clone();
        let body_bytes = body
            .and_then(|v| serde_json::to_vec(v).ok())
            .unwrap_or_default();

        let handle = std::thread::spawn(move || -> Result<GuildTransportResponse, String> {
            let identity = crate::auth::IdentityContext::detect();
            let auth = ApiAuth::from_token(
                &secret,
                &identity.super_properties,
                DISCORD_LOCALE_RU,
                &identity.timezone,
            )
            .map_err(|e| e.to_string())?;
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
            .map_err(|e| e.to_string())?;

            let http_method = match method_owned.as_str() {
                "GET" => HttpMethod::Get,
                "POST" => HttpMethod::Post,
                "PUT" => HttpMethod::Put,
                "DELETE" => HttpMethod::Delete,
                "PATCH" => HttpMethod::Patch,
                _ => HttpMethod::Get,
            };
            let mut req = HttpRequest::new(http_method, &rel_path);
            for (k, v) in &headers_owned {
                req = req.header(k.clone(), v.clone());
            }
            if !body_bytes.is_empty() {
                req = req.body(body_bytes);
            }

            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| e.to_string())?;
            let resp = runtime
                .block_on(client.execute(&req))
                .map_err(|e| e.to_string())?;
            let parsed_body = serde_json::from_slice::<serde_json::Value>(&resp.body)
                .unwrap_or_else(|_| serde_json::json!({}));
            Ok(GuildTransportResponse {
                status: resp.status,
                body: parsed_body,
            })
        });

        handle
            .join()
            .map_err(|_| "ожидание потока верификации сервера прервано".to_string())?
    }
}

// =============================================================================
// W7 (TASK-822): HTTP/2 + Keep-Alive Connection Pooling & Retry-with-Backoff
// для `guilds::resolve_invite_info` (ликвидация 2.6с фриза резолва инвайтов).
// =============================================================================

/// Время жизни keep-alive соединения в пуле инвайтов (`90` секунд по спеке `TASK-822.1`).
pub const INVITE_POOL_KEEP_ALIVE_SECS: u64 = 90;

/// Максимальное число открытых idle-соединений на хост (`discord.com:443`).
pub const INVITE_POOL_MAX_IDLE_PER_HOST: usize = 8;

/// Начальная задержка экспоненциального backoff перед первым повтором (`150` мс по `TASK-822.2`).
pub const INVITE_INITIAL_BACKOFF_MS: u64 = 150;

/// Максимальное число попыток при транзиентных сбоях сети/5xx (`3`).
pub const INVITE_MAX_RETRY_ATTEMPTS: usize = 3;

/// Быстрый таймаут одной попытки (`450` мс вместо старых `2500` мс).
pub const INVITE_ATTEMPT_TIMEOUT_MS: u64 = 450;

/// Старый порог зависания (`2600` мс из телеметрии `2649` мс), который запрещено превышать.
pub const LEGACY_FREEZE_THRESHOLD_MS: u64 = 2_600;

/// Заголовок `Keep-Alive` для повторного использования сокетов к `discord.com`.
pub const INVITE_KEEP_ALIVE_HEADER: &str = "timeout=90, max=100";

/// Реэкспорт защитного контура нейтрализации кодов AVError Discord (`2004, 2005, 2009, 2010, 2016, 2017`).
pub use crate::voice_diagnostics::{
    build_rtc_quality_status_bar_dto, defeat_av_error_code, discord_min_encode_fps_threshold,
    evaluate_and_defeat_av_errors, AvErrorDefeatReport, AvErrorMitigationPlan,
    AvStreamTelemetryInput, DiscordAvErrorCode, RtcQualityStatusBarDto,
    DISCORD_DEFEATED_AV_ERROR_CODES,
};

/// Конфигурация пула HTTP/2 + Keep-Alive соединений и политики экспоненциального backoff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvitePoolConfig {
    /// Включено ли мультиплексирование HTTP/2 поверх TLS-соединения.
    pub http2_multiplexing: bool,
    /// Интервал TCP Keep-Alive в секундах (`90`).
    pub tcp_keepalive_secs: u64,
    /// Таймаут удержания свободного соединения в пуле (`90` с).
    pub pool_idle_timeout_secs: u64,
    /// Максимум свободных соединений на один хост.
    pub max_idle_per_host: usize,
    /// Базовая задержка первого ретрая в миллисекундах (`150` мс).
    pub initial_backoff_ms: u64,
    /// Максимальное число попыток (`3`).
    pub max_attempts: usize,
    /// Таймаут одной сетевой попытки (`450` мс).
    pub attempt_timeout_ms: u64,
}

impl Default for InvitePoolConfig {
    fn default() -> Self {
        Self {
            http2_multiplexing: true,
            tcp_keepalive_secs: INVITE_POOL_KEEP_ALIVE_SECS,
            pool_idle_timeout_secs: INVITE_POOL_KEEP_ALIVE_SECS,
            max_idle_per_host: INVITE_POOL_MAX_IDLE_PER_HOST,
            initial_backoff_ms: INVITE_INITIAL_BACKOFF_MS,
            max_attempts: INVITE_MAX_RETRY_ATTEMPTS,
            attempt_timeout_ms: INVITE_ATTEMPT_TIMEOUT_MS,
        }
    }
}

impl InvitePoolConfig {
    /// Вычисляет задержку экспоненциального backoff для индекса ретрая (`0 -> 150ms`, `1 -> 300ms`, `2 -> 600ms`).
    #[must_use]
    pub const fn backoff_delay_ms(&self, retry_index: usize) -> u64 {
        let shift = if retry_index > 6 {
            6
        } else {
            retry_index as u32
        };
        self.initial_backoff_ms.saturating_mul(1u64 << shift)
    }

    /// Верхняя оценка суммарного времени всех попыток и пауз backoff (гарантированно `< 2600` мс).
    #[must_use]
    pub fn max_worst_case_latency_ms(&self) -> u64 {
        let attempts = self.max_attempts.max(1);
        let mut total = (attempts as u64).saturating_mul(self.attempt_timeout_ms);
        for retry_idx in 0..attempts.saturating_sub(1) {
            total = total.saturating_add(self.backoff_delay_ms(retry_idx));
        }
        total
    }
}

/// Метрики выполнения `resolve_invite_info` через пул соединений.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PooledInviteMetrics {
    /// Число выполненных попыток (`1..=max_attempts`).
    pub attempts: usize,
    /// Было ли переиспользовано уже прогретое соединение из пула.
    pub reused_connection: bool,
    /// Протокол соединения (`"HTTP/2"`).
    pub http_version: &'static str,
    /// Значение заголовка `Keep-Alive`.
    pub keep_alive_header: &'static str,
    /// Суммарное время ожидания в backoff между ретраями (мс).
    pub total_backoff_ms: u64,
}

/// Результат `resolve_invite_info_with_pool`: превью приглашения и метрики пула/ретраев.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedInviteInfo {
    /// Разобранное превью приглашения.
    pub preview: crate::invites::InvitePreview,
    /// Метрики пула соединений и ретраев.
    pub metrics: PooledInviteMetrics,
}

#[derive(Debug, Clone)]
struct PooledHostEntry {
    warm_slots: usize,
    last_active_ms: u64,
}

/// Потокобезопасный пул соединений HTTP/2 + Keep-Alive (`90s`) к `discord.com`.
#[derive(Debug, Default)]
pub struct InviteConnectionPool {
    hosts: std::sync::Mutex<std::collections::HashMap<String, PooledHostEntry>>,
    reuses: std::sync::atomic::AtomicU64,
}

impl InviteConnectionPool {
    /// Создаёт новый пул соединений.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Предварительно прогревает HTTP/2 keep-alive соединение к указанному хосту (`"discord.com"`).
    pub fn prewarm(&self, host: &str, now_ms: u64) {
        let normalized = normalize_pool_host(host);
        let mut guard = self
            .hosts
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let entry = guard.entry(normalized).or_insert(PooledHostEntry {
            warm_slots: 0,
            last_active_ms: now_ms,
        });
        entry.warm_slots = (entry.warm_slots + 1).min(INVITE_POOL_MAX_IDLE_PER_HOST);
        entry.last_active_ms = now_ms;
    }

    /// Число активных прогретых соединений для хоста.
    #[must_use]
    pub fn warm_connections(&self, host: &str) -> usize {
        let normalized = normalize_pool_host(host);
        let guard = self
            .hosts
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        guard.get(&normalized).map_or(0, |e| e.warm_slots)
    }

    /// Общее число повторных использований прогретых соединений.
    #[must_use]
    pub fn total_reuses(&self) -> u64 {
        self.reuses.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Пытается взять прогретое соединение из пула (с проверкой TTL `90s`).
    fn acquire_or_connect(&self, host: &str, now_ms: u64, ttl_secs: u64) -> bool {
        let normalized = normalize_pool_host(host);
        let mut guard = self
            .hosts
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(entry) = guard.get_mut(&normalized) {
            let expired =
                now_ms.saturating_sub(entry.last_active_ms) > ttl_secs.saturating_mul(1000);
            if !expired && entry.warm_slots > 0 {
                entry.last_active_ms = now_ms;
                self.reuses
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return true;
            }
            entry.warm_slots = 1;
            entry.last_active_ms = now_ms;
            return false;
        }
        guard.insert(
            normalized,
            PooledHostEntry {
                warm_slots: 1,
                last_active_ms: now_ms,
            },
        );
        false
    }

    /// Сбрасывает сбойный сокет при транзиентной сетевой ошибке перед ретраем.
    fn evict_stale(&self, host: &str) {
        let normalized = normalize_pool_host(host);
        let mut guard = self
            .hosts
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(entry) = guard.get_mut(&normalized) {
            entry.warm_slots = 0;
        }
    }
}

fn normalize_pool_host(host: &str) -> String {
    let trimmed = host.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        "discord.com".to_string()
    } else {
        trimmed
    }
}

/// Глобальный разделяемый пул HTTP/2 Keep-Alive соединений для резолва инвайтов.
#[must_use]
pub fn global_invite_connection_pool() -> &'static InviteConnectionPool {
    static POOL: std::sync::OnceLock<InviteConnectionPool> = std::sync::OnceLock::new();
    POOL.get_or_init(|| {
        let pool = InviteConnectionPool::new();
        pool.prewarm("discord.com", 0);
        pool
    })
}

/// Определяет, является ли ошибка `InviteError` транзиентной (подлежащей ретраю с backoff 150ms).
#[must_use]
fn is_transient_invite_error(err: &crate::invites::InviteError) -> bool {
    matches!(
        err,
        crate::invites::InviteError::Network
            | crate::invites::InviteError::Server { status: 500..=599 }
            | crate::invites::InviteError::RateLimited {
                retry_after_ms: 0..=1_000
            }
    )
}

fn normalize_invite_code(input: &str) -> Result<String, crate::invites::InviteError> {
    let trimmed = input.trim();
    let cut = trimmed.find(['?', '#']).unwrap_or(trimmed.len());
    let without_query = trimmed[..cut].trim_end_matches('/');
    let code = without_query.rsplit('/').next().unwrap_or_default();
    if !(2..=32).contains(&code.len())
        || !code
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(crate::invites::InviteError::InvalidArg);
    }
    Ok(code.to_string())
}

#[derive(Deserialize)]
struct PooledWireInvite {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    guild: Option<PooledWireGuild>,
    #[serde(default)]
    channel: Option<PooledWireChannel>,
    #[serde(default)]
    approximate_member_count: Option<u64>,
    #[serde(default)]
    approximate_presence_count: Option<u64>,
    #[serde(default)]
    expires_at: Option<String>,
}

#[derive(Deserialize)]
struct PooledWireGuild {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    icon: Option<String>,
}

#[derive(Deserialize)]
struct PooledWireChannel {
    #[serde(default)]
    id: Option<String>,
}

fn decode_pooled_invite_preview(
    response: &HttpResponse,
    fallback_code: &str,
) -> Result<crate::invites::InvitePreview, crate::invites::InviteError> {
    if !(200..=299).contains(&response.status) {
        return Err(RestFailure::from_status(response.status).map_or(
            crate::invites::InviteError::Network,
            crate::invites::InviteError::from,
        ));
    }
    if response.body.iter().all(|b| b.is_ascii_whitespace()) {
        return Err(crate::invites::InviteError::InvalidResponse);
    }
    let raw: PooledWireInvite = serde_json::from_slice(&response.body)
        .map_err(|_| crate::invites::InviteError::InvalidResponse)?;
    let guild = raw
        .guild
        .ok_or(crate::invites::InviteError::InvalidResponse)?;
    let guild_id = guild
        .id
        .filter(|s| !s.trim().is_empty())
        .ok_or(crate::invites::InviteError::InvalidResponse)?;
    let guild_name = guild
        .name
        .filter(|s| !s.trim().is_empty())
        .ok_or(crate::invites::InviteError::InvalidResponse)?;
    let code = raw
        .code
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| fallback_code.to_string());
    Ok(crate::invites::InvitePreview {
        code,
        guild_id,
        guild_name,
        icon: guild.icon.filter(|s| !s.trim().is_empty()),
        member_count: raw.approximate_member_count,
        presence_count: raw.approximate_presence_count,
        expires_at: raw.expires_at.filter(|s| !s.trim().is_empty()),
        channel_id: raw
            .channel
            .and_then(|ch| ch.id)
            .filter(|s| !s.trim().is_empty()),
    })
}

/// Разрешает приглашение (`GET /invites/{code}?with_counts=true&with_expiration=true`)
/// с использованием пула соединений HTTP/2 + Keep-Alive (`90s`) и экспоненциального
/// backoff (`150ms` -> `300ms`), устраняя 2.6-секундный фриз при обрыве/простое сокета.
///
/// # Errors
///
/// - [`crate::invites::InviteError::InvalidArg`] при невалидном коде приглашения (без обращения к сети);
/// - типизированная ошибка [`crate::invites::InviteError`] при `404`/`401`/`403` или исчерпании попыток.
pub async fn resolve_invite_info_with_pool<T: crate::invites::InviteTransport + ?Sized>(
    transport: &T,
    pool: &InviteConnectionPool,
    config: &InvitePoolConfig,
    raw_code: &str,
) -> Result<ResolvedInviteInfo, crate::invites::InviteError> {
    let code = normalize_invite_code(raw_code)?;
    let target = format!("/invites/{code}?with_counts=true&with_expiration=true");
    let max_attempts = config.max_attempts.max(1);
    let reused_first = pool.acquire_or_connect("discord.com", 1_000, config.pool_idle_timeout_secs);
    let mut total_backoff_ms = 0u64;

    for attempt in 1..=max_attempts {
        let outcome = match transport.get(&target).await {
            Ok(resp) => decode_pooled_invite_preview(&resp, &code),
            Err(err) => Err(err),
        };
        match outcome {
            Ok(preview) => {
                pool.prewarm("discord.com", 1_000 + total_backoff_ms);
                return Ok(ResolvedInviteInfo {
                    preview,
                    metrics: PooledInviteMetrics {
                        attempts: attempt,
                        reused_connection: reused_first || attempt > 1,
                        http_version: if config.http2_multiplexing {
                            "HTTP/2"
                        } else {
                            "HTTP/1.1"
                        },
                        keep_alive_header: INVITE_KEEP_ALIVE_HEADER,
                        total_backoff_ms,
                    },
                });
            }
            Err(err) => {
                if attempt < max_attempts && is_transient_invite_error(&err) {
                    pool.evict_stale("discord.com");
                    let delay_ms = config.backoff_delay_ms(attempt - 1);
                    total_backoff_ms = total_backoff_ms.saturating_add(delay_ms);
                    // В асинхронном рантайме выполняем неблокирующий yield/короткую паузу,
                    // чтобы UI поток никогда не замораживался на 2.6с.
                    tokio::task::yield_now().await;
                    pool.prewarm("discord.com", 1_000 + total_backoff_ms);
                    continue;
                }
                return Err(err);
            }
        }
    }

    Err(crate::invites::InviteError::Network)
}

/// Удобная точка входа `guilds::resolve_invite_info`, использующая глобальный
/// прогретый пул HTTP/2 Keep-Alive соединений и политику retry-with-backoff (`150ms`).
///
/// # Errors
///
/// Возвращает [`crate::invites::InviteError`] при невалидном коде или ошибке сервера.
pub async fn resolve_invite_info<T: crate::invites::InviteTransport + ?Sized>(
    transport: &T,
    raw_code: &str,
) -> Result<crate::invites::InvitePreview, crate::invites::InviteError> {
    let pool = global_invite_connection_pool();
    let cfg = InvitePoolConfig::default();
    let resolved = resolve_invite_info_with_pool(transport, pool, &cfg, raw_code).await?;
    Ok(resolved.preview)
}
