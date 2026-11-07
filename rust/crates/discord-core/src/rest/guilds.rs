//! REST v9: `POST /guilds` — создание сервера пользователем (TASK-611).
//!
//! Оракулы:
//! - `docs/re/rest-d02.md` §6.1–§6.2: полный набор из 6 auth-заголовков Discord
//!   (`Authorization`, `X-Super-Properties`, `X-Discord-Locale: ru`,
//!   `X-Discord-Timezone`, `X-Debug-Options: bugReporterEnabled`, `Accept-Language: ru`)
//!   плюс `Content-Type: application/json`.
//! - `/home/sanya/re-deep/rest-static.json`: `Endpoints.GUILDS = "/guilds"`.
//! - `/home/sanya/re-deep/out/bundles/web.4d4b2f75fdea5470.js`:
//!   - поля тела `POST /guilds`: `name` (`1..=100` символов), `icon` (base64 DataURI),
//!     `channels` (начальные категории и каналы), `system_channel_flags`, `guild_template_code`;
//!   - флаги `system_channel_flags`: `1 << 0` (`SUPPRESS_JOIN_NOTIFICATIONS`),
//!     `1 << 1` (`SUPPRESS_PREMIUM_SUBSCRIPTIONS`),
//!     `1 << 2` (`SUPPRESS_GUILD_REMINDER_NOTIFICATIONS`),
//!     `1 << 3` (`SUPPRESS_JOIN_NOTIFICATION_REPLIES`);
//!   - ошибки `400` (`MAXIMUM_GUILDS = 30005` / `TOO_MANY_USER_GUILDS = 30001`)
//!     и `403` (`NO_VERIFIED_PHONE` / `40002`).

use std::fmt;

use discord_json::{JsonValue, parse};

use super::base64;
use super::client::{ApiAuth, ApiAuthError};
use super::routes::HttpMethod;
use super::super_properties::SuperProperties;
use super::transport::{HttpRequest, HttpResponse};

/// Относительный маршрут создания сервера в REST v9 (`Endpoints.GUILDS`).
pub const GUILDS_PATH: &str = "/guilds";

/// Полный путь создания сервера в API v9.
pub const GUILDS_API_V9_PATH: &str = "/api/v9/guilds";

/// Локаль Discord по умолчанию для русскоязычного клиента (`X-Discord-Locale: ru`).
pub const DISCORD_LOCALE_RU: &str = "ru";

/// Минимальная длина имени сервера (в символах Unicode после `trim`).
pub const MIN_GUILD_NAME_CHARS: usize = 1;

/// Максимальная длина имени сервера (в символах Unicode после `trim`).
pub const MAX_GUILD_NAME_CHARS: usize = 100;

/// Код ошибки Discord API при превышении лимита серверов (`MAXIMUM_GUILDS`).
pub const DISCORD_CODE_MAXIMUM_GUILDS: i64 = 30005;

/// Альтернативный код Discord API при превышении лимита серверов пользователя (`TOO_MANY_USER_GUILDS`).
pub const DISCORD_CODE_TOO_MANY_USER_GUILDS: i64 = 30001;

/// Числовой код ошибки отсутствия подтверждённого телефона (`NO_VERIFIED_PHONE`).
pub const DISCORD_CODE_NO_VERIFIED_PHONE_NUM: i64 = 40002;

/// Строковый код ошибки отсутствия подтверждённого телефона (`NO_VERIFIED_PHONE`).
pub const DISCORD_CODE_NO_VERIFIED_PHONE: &str = "NO_VERIFIED_PHONE";

/// Тип канала Discord: текстовый канал гильдии (`GUILD_TEXT = 0`).
pub const CHANNEL_TYPE_GUILD_TEXT: u8 = 0;

/// Тип канала Discord: голосовой канал гильдии (`GUILD_VOICE = 2`).
pub const CHANNEL_TYPE_GUILD_VOICE: u8 = 2;

/// Тип канала Discord: категория гильдии (`GUILD_CATEGORY = 4`).
pub const CHANNEL_TYPE_GUILD_CATEGORY: u8 = 4;

/// Флаг системного канала: отключить системные сообщения о вступлении участников (`1 << 0`).
pub const SYSTEM_CHANNEL_FLAG_SUPPRESS_JOIN_NOTIFICATIONS: u32 = 1 << 0;

/// Флаг системного канала: отключить сообщения о бустах сервера (`1 << 1`).
pub const SYSTEM_CHANNEL_FLAG_SUPPRESS_PREMIUM_SUBSCRIPTIONS: u32 = 1 << 1;

/// Флаг системного канала: отключить подсказки по настройке сервера (`1 << 2`).
pub const SYSTEM_CHANNEL_FLAG_SUPPRESS_GUILD_REMINDER_NOTIFICATIONS: u32 = 1 << 2;

/// Флаг системного канала: отключить кнопки быстрого ответа со стикером на приветствия (`1 << 3`).
pub const SYSTEM_CHANNEL_FLAG_SUPPRESS_JOIN_NOTIFICATION_REPLIES: u32 = 1 << 3;

/// Поддерживаемые MIME-префиксы base64 DataURI для иконки сервера.
const ALLOWED_ICON_DATA_URI_PREFIXES: [&str; 5] = [
    "data:image/png;base64,",
    "data:image/jpeg;base64,",
    "data:image/jpg;base64,",
    "data:image/gif;base64,",
    "data:image/webp;base64,",
];

/// Описание начальной категории или канала в теле запроса `POST /guilds`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildCreateChannelSpec {
    /// Временный идентификатор канала/категории для привязки `parent_id` внутри одного запроса.
    pub id: Option<String>,
    /// Имя канала или категории (`1..=100` символов).
    pub name: String,
    /// Числовой тип канала (`0` — текстовый, `2` — голосовой, `4` — категория).
    pub kind: u8,
    /// Идентификатор родительской категории (`id` категории из этого же списка).
    pub parent_id: Option<String>,
    /// Позиция канала при сортировке.
    pub position: Option<i64>,
}

impl GuildCreateChannelSpec {
    /// Создать спецификацию начальной категории (`type = 4`).
    #[must_use]
    pub fn category(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: Some(id.into()),
            name: name.into(),
            kind: CHANNEL_TYPE_GUILD_CATEGORY,
            parent_id: None,
            position: None,
        }
    }

    /// Создать спецификацию начального текстового канала (`type = 0`).
    #[must_use]
    pub fn text(name: impl Into<String>, parent_id: Option<&str>) -> Self {
        Self {
            id: None,
            name: name.into(),
            kind: CHANNEL_TYPE_GUILD_TEXT,
            parent_id: parent_id.map(str::to_string),
            position: None,
        }
    }

    /// Создать спецификацию начального голосового канала (`type = 2`).
    #[must_use]
    pub fn voice(name: impl Into<String>, parent_id: Option<&str>) -> Self {
        Self {
            id: None,
            name: name.into(),
            kind: CHANNEL_TYPE_GUILD_VOICE,
            parent_id: parent_id.map(str::to_string),
            position: None,
        }
    }
}

/// Запрос создания сервера (`POST /api/v9/guilds`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildCreateRequest {
    /// Название сервера (`1..=100` символов после `trim`).
    pub name: String,
    /// Опциональная иконка в формате `data:image/<ext>;base64,<payload>`.
    pub icon: Option<String>,
    /// Начальные категории и каналы сервера.
    pub channels: Vec<GuildCreateChannelSpec>,
    /// Битовая маска флагов системного канала (`system_channel_flags`).
    pub system_channel_flags: Option<u32>,
    /// Опциональный код шаблона гильдии.
    pub guild_template_code: Option<String>,
}

impl GuildCreateRequest {
    /// Проверить корректность всех полей запроса до отправки по сети.
    pub fn validate(&self) -> Result<(), GuildCreateRestError> {
        validate_guild_name(&self.name)?;
        if let Some(icon) = &self.icon {
            validate_icon_data_uri(icon)?;
        }
        validate_channels(&self.channels)?;
        if let Some(code) = &self.guild_template_code
            && code.trim().is_empty()
        {
            return Err(GuildCreateRestError::InvalidName);
        }
        Ok(())
    }

    /// Собрать каноническое JSON-тело запроса `POST /guilds` по RFC 8259.
    pub fn to_json_body(&self) -> Result<String, GuildCreateRestError> {
        self.validate()?;
        let trimmed_name = self.name.trim();
        let mut out = String::with_capacity(128);
        out.push_str("{\"name\":");
        push_json_string(&mut out, trimmed_name);

        if let Some(icon) = &self.icon {
            out.push_str(",\"icon\":");
            push_json_string(&mut out, icon.trim());
        }

        if !self.channels.is_empty() {
            out.push_str(",\"channels\":[");
            for (index, ch) in self.channels.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push('{');
                let mut first = true;
                if let Some(id) = &ch.id {
                    out.push_str("\"id\":");
                    push_json_string(&mut out, id.trim());
                    first = false;
                }
                if !first {
                    out.push(',');
                }
                out.push_str("\"name\":");
                push_json_string(&mut out, ch.name.trim());
                out.push_str(",\"type\":");
                out.push_str(&ch.kind.to_string());
                if let Some(parent_id) = &ch.parent_id {
                    out.push_str(",\"parent_id\":");
                    push_json_string(&mut out, parent_id.trim());
                }
                if let Some(position) = ch.position {
                    out.push_str(",\"position\":");
                    out.push_str(&position.to_string());
                }
                out.push('}');
            }
            out.push(']');
        }

        if let Some(flags) = self.system_channel_flags {
            out.push_str(",\"system_channel_flags\":");
            out.push_str(&flags.to_string());
        }

        if let Some(template_code) = &self.guild_template_code {
            out.push_str(",\"guild_template_code\":");
            push_json_string(&mut out, template_code.trim());
        }

        out.push('}');
        Ok(out)
    }
}

/// Роль созданной гильдии из ответа `201 Created`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedGuildRole {
    /// Snowflake ID роли.
    pub id: String,
    /// Название роли (`@everyone` и др.).
    pub name: String,
    /// Строка битовой маски прав (`permissions`).
    pub permissions: String,
    /// Позиция роли в иерархии.
    pub position: i64,
    /// RGB-цвет роли.
    pub color: u32,
    /// Отображать ли участников роли отдельно (`hoist`).
    pub hoist: bool,
    /// Разрешено ли упоминание роли (`mentionable`).
    pub mentionable: bool,
}

/// Канал или категория созданной гильдии из ответа `201 Created`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedGuildChannel {
    /// Snowflake ID канала.
    pub id: String,
    /// Snowflake ID гильдии (если присутствует в объекте канала).
    pub guild_id: Option<String>,
    /// Имя канала или категории.
    pub name: String,
    /// Тип канала (`0` — текстовый, `2` — голосовой, `4` — категория и т.д.).
    pub kind: u8,
    /// Snowflake ID родительской категории.
    pub parent_id: Option<String>,
    /// Позиция канала.
    pub position: i64,
}

/// Полная модель созданного сервера из ответа `201 Created` (`POST /api/v9/guilds`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedGuild {
    /// Snowflake ID созданной гильдии.
    pub id: String,
    /// Название гильдии.
    pub name: String,
    /// Хеш иконки гильдии (или `None`).
    pub icon: Option<String>,
    /// Snowflake ID владельца гильдии.
    pub owner_id: Option<String>,
    /// Snowflake ID системного канала.
    pub system_channel_id: Option<String>,
    /// Флаги системного канала (`system_channel_flags`).
    pub system_channel_flags: u32,
    /// Распарсенные начальные каналы и категории.
    pub channels: Vec<CreatedGuildChannel>,
    /// Распарсенные начальные роли гильдии.
    pub roles: Vec<CreatedGuildRole>,
}

/// Типизированная ошибка вызова `POST /api/v9/guilds`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuildCreateRestError {
    /// Название пустое или длиннее 100 символов.
    InvalidName,
    /// Некорректный base64 DataURI иконки.
    InvalidIcon,
    /// Некорректное описание начального канала или категории.
    InvalidChannel,
    /// Ошибка формирования обязательных заголовков Discord.
    Header(ApiAuthError),
    /// Достигнут лимит серверов пользователя (`HTTP 400`, `code = 30005` / `30001`).
    MaximumGuilds {
        /// Числовой код ошибки Discord (`30005` или `30001`).
        code: i64,
        /// Сообщение об ошибке от сервера.
        message: String,
    },
    /// Требуется подтверждённый номер телефона (`HTTP 403`, `NO_VERIFIED_PHONE`).
    NoVerifiedPhone {
        /// Сообщение об ошибке от сервера.
        message: String,
    },
    /// Токен недействителен (`HTTP 401`).
    Unauthorized,
    /// Доступ запрещён по иной причине (`HTTP 403`).
    Forbidden {
        /// Опциональный числовой код ошибки Discord.
        code: Option<i64>,
        /// Опциональное сообщение об ошибке.
        message: Option<String>,
    },
    /// Превышен лимит запросов (`HTTP 429`).
    RateLimited {
        /// Задержка до повтора в миллисекундах.
        retry_after_ms: u64,
    },
    /// Прочая клиентская ошибка `4xx`.
    Client {
        /// HTTP-статус ответа.
        status: u16,
        /// Опциональный код ошибки Discord.
        code: Option<i64>,
        /// Опциональное сообщение об ошибке.
        message: Option<String>,
    },
    /// Ошибка сервера Discord (`5xx`).
    Server {
        /// HTTP-статус ответа.
        status: u16,
    },
    /// Некорректный или неполный JSON ответа `201 Created`.
    InvalidResponse(&'static str),
}

impl fmt::Display for GuildCreateRestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName => {
                f.write_str("некорректное название сервера (допустимо 1..=100 символов)")
            }
            Self::InvalidIcon => {
                f.write_str("некорректная иконка сервера (ожидается base64 DataURI)")
            }
            Self::InvalidChannel => {
                f.write_str("некорректная спецификация начального канала/категории")
            }
            Self::Header(err) => write!(f, "ошибка заголовков Discord: {err:?}"),
            Self::MaximumGuilds { code, message } => {
                write!(
                    f,
                    "достигнут лимит серверов (MAXIMUM_GUILDS {code}): {message}"
                )
            }
            Self::NoVerifiedPhone { message } => {
                write!(
                    f,
                    "требуется подтверждённый телефон (NO_VERIFIED_PHONE): {message}"
                )
            }
            Self::Unauthorized => f.write_str("сессия недействительна (401)"),
            Self::Forbidden { message, .. } => match message {
                Some(msg) => write!(f, "доступ запрещён (403): {msg}"),
                None => f.write_str("доступ запрещён (403)"),
            },
            Self::RateLimited { retry_after_ms } => {
                write!(
                    f,
                    "превышен лимит запросов, повтор через {retry_after_ms} мс"
                )
            }
            Self::Client { status, .. } => write!(f, "клиентская ошибка HTTP {status}"),
            Self::Server { status } => write!(f, "ошибка сервера Discord HTTP {status}"),
            Self::InvalidResponse(reason) => write!(f, "некорректный ответ POST /guilds: {reason}"),
        }
    }
}

impl std::error::Error for GuildCreateRestError {}

impl From<ApiAuthError> for GuildCreateRestError {
    fn from(err: ApiAuthError) -> Self {
        Self::Header(err)
    }
}

/// Проверить название сервера (`1..=100` символов Unicode после `trim`).
pub fn validate_guild_name(name: &str) -> Result<&str, GuildCreateRestError> {
    let trimmed = name.trim();
    let count = trimmed.chars().count();
    if !(MIN_GUILD_NAME_CHARS..=MAX_GUILD_NAME_CHARS).contains(&count) {
        return Err(GuildCreateRestError::InvalidName);
    }
    Ok(trimmed)
}

/// Проверить, что иконка передана как валидный `data:image/(png|jpeg|jpg|gif|webp);base64,<payload>`.
pub fn validate_icon_data_uri(icon: &str) -> Result<(), GuildCreateRestError> {
    let trimmed = icon.trim();
    let Some(base64_part) = ALLOWED_ICON_DATA_URI_PREFIXES
        .iter()
        .find_map(|prefix| trimmed.strip_prefix(prefix))
    else {
        return Err(GuildCreateRestError::InvalidIcon);
    };
    if base64_part.is_empty() {
        return Err(GuildCreateRestError::InvalidIcon);
    }
    let decoded = base64::decode(base64_part).map_err(|_| GuildCreateRestError::InvalidIcon)?;
    if decoded.is_empty() {
        return Err(GuildCreateRestError::InvalidIcon);
    }
    Ok(())
}

/// Проверить массив начальных категорий и каналов.
pub fn validate_channels(channels: &[GuildCreateChannelSpec]) -> Result<(), GuildCreateRestError> {
    for ch in channels {
        let name_len = ch.name.trim().chars().count();
        if !(MIN_GUILD_NAME_CHARS..=MAX_GUILD_NAME_CHARS).contains(&name_len) {
            return Err(GuildCreateRestError::InvalidChannel);
        }
        if let Some(id) = &ch.id
            && id.trim().is_empty()
        {
            return Err(GuildCreateRestError::InvalidChannel);
        }
        if let Some(parent_id) = &ch.parent_id
            && parent_id.trim().is_empty()
        {
            return Err(GuildCreateRestError::InvalidChannel);
        }
    }
    Ok(())
}

/// Собрать обязательные 6 заголовков Discord для `POST /api/v9/guilds`
/// с локалью `X-Discord-Locale: ru` (`docs/re/rest-d02.md` §6.1).
pub fn build_guild_create_headers(
    token: &str,
    super_properties: &SuperProperties,
    timezone: &str,
) -> Result<ApiAuth, ApiAuthError> {
    ApiAuth::from_token(token, super_properties, DISCORD_LOCALE_RU, timezone)
}

/// Собрать готовый [`HttpRequest`] `POST /guilds` с полным набором заголовков Discord
/// (`X-Super-Properties`, `X-Discord-Locale: ru`, `Authorization`, `X-Discord-Timezone`,
/// `X-Debug-Options: bugReporterEnabled`, `Accept-Language: ru`, `Content-Type: application/json`)
/// и проверенным JSON-телом.
pub fn build_guild_create_http_request(
    token: &str,
    super_properties: &SuperProperties,
    timezone: &str,
    request: &GuildCreateRequest,
) -> Result<HttpRequest, GuildCreateRestError> {
    let body_json = request.to_json_body()?;
    let auth_headers = build_guild_create_headers(token, super_properties, timezone)?;
    let mut http_req = HttpRequest::new(HttpMethod::Post, GUILDS_PATH).body(body_json.into_bytes());
    for (name, value) in auth_headers.pairs() {
        http_req = http_req.header(name, value);
    }
    http_req = http_req.header("Content-Type", "application/json");
    Ok(http_req)
}

/// Разобрать HTTP-ответ `POST /api/v9/guilds`:
/// - `201 Created` (и `200..300`) → парсинг созданной гильдии, начальных каналов и ролей;
/// - `400 Bad Request` с `code == 30005` (`MAXIMUM_GUILDS`) или `30001` → [`GuildCreateRestError::MaximumGuilds`];
/// - `403 Forbidden` с `code == "NO_VERIFIED_PHONE"` или `40002` → [`GuildCreateRestError::NoVerifiedPhone`];
/// - `401` / `403` / `429` / `5xx` → соответствующие типизированные варианты [`GuildCreateRestError`].
pub fn parse_guild_create_response(
    response: &HttpResponse,
) -> Result<CreatedGuild, GuildCreateRestError> {
    let status = response.status;
    if (200..300).contains(&status) {
        return parse_created_guild_body(&response.body);
    }
    Err(classify_error_response(
        status,
        &response.headers,
        &response.body,
    ))
}

fn parse_created_guild_body(body: &[u8]) -> Result<CreatedGuild, GuildCreateRestError> {
    let text = std::str::from_utf8(body)
        .map_err(|_| GuildCreateRestError::InvalidResponse("ответ не является валидным UTF-8"))?;
    let root = parse(text)
        .map_err(|_| GuildCreateRestError::InvalidResponse("ответ не является валидным JSON"))?;
    let obj = root
        .as_object()
        .ok_or(GuildCreateRestError::InvalidResponse(
            "ожидался JSON-объект гильдии",
        ))?;

    let id =
        obj.get("id")
            .and_then(JsonValue::as_str)
            .ok_or(GuildCreateRestError::InvalidResponse(
                "отсутствует поле id гильдии",
            ))?;
    if !is_valid_snowflake(id) {
        return Err(GuildCreateRestError::InvalidResponse(
            "id гильдии не является валидным Snowflake",
        ));
    }

    let name = obj
        .get("name")
        .and_then(JsonValue::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or(GuildCreateRestError::InvalidResponse(
            "отсутствует непустое поле name гильдии",
        ))?
        .to_string();

    let icon = obj
        .get("icon")
        .and_then(JsonValue::as_str)
        .map(str::to_string);

    let owner_id = obj
        .get("owner_id")
        .and_then(JsonValue::as_str)
        .filter(|s| is_valid_snowflake(s))
        .map(str::to_string);

    let system_channel_id = obj
        .get("system_channel_id")
        .and_then(JsonValue::as_str)
        .filter(|s| is_valid_snowflake(s))
        .map(str::to_string);

    let system_channel_flags = obj
        .get("system_channel_flags")
        .and_then(JsonValue::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(0);

    let mut channels = Vec::new();
    if let Some(raw_channels) = obj.get("channels").and_then(JsonValue::as_array) {
        for item in raw_channels {
            if let Some(ch) = parse_channel_item(item, id) {
                channels.push(ch);
            }
        }
    }

    let mut roles = Vec::new();
    if let Some(raw_roles) = obj.get("roles").and_then(JsonValue::as_array) {
        for item in raw_roles {
            if let Some(role) = parse_role_item(item) {
                roles.push(role);
            }
        }
    }

    Ok(CreatedGuild {
        id: id.to_string(),
        name,
        icon,
        owner_id,
        system_channel_id,
        system_channel_flags,
        channels,
        roles,
    })
}

fn parse_channel_item(value: &JsonValue, default_guild_id: &str) -> Option<CreatedGuildChannel> {
    let obj = value.as_object()?;
    let id = obj.get("id").and_then(JsonValue::as_str)?;
    if !is_valid_snowflake(id) {
        return None;
    }
    let name = obj
        .get("name")
        .and_then(JsonValue::as_str)
        .unwrap_or("")
        .to_string();
    let kind = obj
        .get("type")
        .and_then(JsonValue::as_u64)
        .and_then(|v| u8::try_from(v).ok())
        .unwrap_or(CHANNEL_TYPE_GUILD_TEXT);
    let guild_id = obj
        .get("guild_id")
        .and_then(JsonValue::as_str)
        .filter(|s| is_valid_snowflake(s))
        .map(str::to_string)
        .or_else(|| Some(default_guild_id.to_string()));
    let parent_id = obj
        .get("parent_id")
        .and_then(JsonValue::as_str)
        .filter(|s| is_valid_snowflake(s))
        .map(str::to_string);
    let position = obj.get("position").and_then(JsonValue::as_i64).unwrap_or(0);

    Some(CreatedGuildChannel {
        id: id.to_string(),
        guild_id,
        name,
        kind,
        parent_id,
        position,
    })
}

fn parse_role_item(value: &JsonValue) -> Option<CreatedGuildRole> {
    let obj = value.as_object()?;
    let id = obj.get("id").and_then(JsonValue::as_str)?;
    if !is_valid_snowflake(id) {
        return None;
    }
    let name = obj
        .get("name")
        .and_then(JsonValue::as_str)
        .unwrap_or("")
        .to_string();
    let permissions = match obj.get("permissions") {
        Some(JsonValue::String(s)) => s.clone(),
        Some(JsonValue::Number(n)) => n.as_str().to_string(),
        _ => "0".to_string(),
    };
    let position = obj.get("position").and_then(JsonValue::as_i64).unwrap_or(0);
    let color = obj
        .get("color")
        .and_then(JsonValue::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(0);
    let hoist = obj
        .get("hoist")
        .and_then(JsonValue::as_bool)
        .unwrap_or(false);
    let mentionable = obj
        .get("mentionable")
        .and_then(JsonValue::as_bool)
        .unwrap_or(false);

    Some(CreatedGuildRole {
        id: id.to_string(),
        name,
        permissions,
        position,
        color,
        hoist,
        mentionable,
    })
}

fn classify_error_response(
    status: u16,
    headers: &[(String, String)],
    body: &[u8],
) -> GuildCreateRestError {
    if status == 401 {
        return GuildCreateRestError::Unauthorized;
    }
    if status == 429 {
        let retry_after_ms = extract_retry_after_ms(headers, body);
        return GuildCreateRestError::RateLimited { retry_after_ms };
    }
    if (500..=599).contains(&status) {
        return GuildCreateRestError::Server { status };
    }

    let parsed = std::str::from_utf8(body)
        .ok()
        .and_then(|text| parse(text).ok());
    let num_code = parsed
        .as_ref()
        .and_then(|v| v.get("code"))
        .and_then(JsonValue::as_i64);
    let str_code = parsed
        .as_ref()
        .and_then(|v| v.get("code"))
        .and_then(JsonValue::as_str);
    let message = parsed
        .as_ref()
        .and_then(|v| v.get("message"))
        .and_then(JsonValue::as_str)
        .map(str::to_string);

    if status == 400 {
        if let Some(code) = num_code
            && (code == DISCORD_CODE_MAXIMUM_GUILDS || code == DISCORD_CODE_TOO_MANY_USER_GUILDS)
        {
            return GuildCreateRestError::MaximumGuilds {
                code,
                message: message.unwrap_or_else(|| "Maximum number of guilds reached".to_string()),
            };
        }
        if matches!(
            str_code,
            Some("MAXIMUM_GUILDS" | "TOO_MANY_USER_GUILDS" | "30005")
        ) {
            return GuildCreateRestError::MaximumGuilds {
                code: DISCORD_CODE_MAXIMUM_GUILDS,
                message: message.unwrap_or_else(|| "Maximum number of guilds reached".to_string()),
            };
        }
        return GuildCreateRestError::Client {
            status,
            code: num_code,
            message,
        };
    }

    if status == 403 {
        let is_no_verified_phone = matches!(str_code, Some(DISCORD_CODE_NO_VERIFIED_PHONE))
            || num_code == Some(DISCORD_CODE_NO_VERIFIED_PHONE_NUM)
            || message
                .as_deref()
                .is_some_and(|m| m.contains(DISCORD_CODE_NO_VERIFIED_PHONE));
        if is_no_verified_phone {
            return GuildCreateRestError::NoVerifiedPhone {
                message: message
                    .unwrap_or_else(|| "Must have a verified phone on account".to_string()),
            };
        }
        return GuildCreateRestError::Forbidden {
            code: num_code,
            message,
        };
    }

    GuildCreateRestError::Client {
        status,
        code: num_code,
        message,
    }
}

fn extract_retry_after_ms(headers: &[(String, String)], body: &[u8]) -> u64 {
    if let Ok(text) = std::str::from_utf8(body)
        && let Ok(val) = parse(text)
        && let Some(secs) = val.get("retry_after").and_then(JsonValue::as_f64)
        && secs >= 0.0
    {
        return (secs * 1000.0).round() as u64;
    }
    for (name, value) in headers {
        if name.eq_ignore_ascii_case("retry-after")
            && let Ok(secs) = value.trim().parse::<f64>()
            && secs.is_finite()
            && secs >= 0.0
        {
            return (secs * 1000.0).round() as u64;
        }
    }
    1000
}

fn is_valid_snowflake(id: &str) -> bool {
    let trimmed = id.trim();
    if trimmed.is_empty() || trimmed.len() > 20 || !trimmed.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    trimmed.parse::<u64>().is_ok_and(|value| value > 0)
}

const HEX_LOWER: [char; 16] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
];

fn push_json_string(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0000}'..='\u{001f}' => {
                let code = ch as u32;
                out.push_str("\\u00");
                out.push(HEX_LOWER[(code >> 4) as usize]);
                out.push(HEX_LOWER[(code & 0xf) as usize]);
            }
            other => out.push(other),
        }
    }
    out.push('"');
}
