//! `GET /users/@me`: тип текущего пользователя и строгий разбор JSON-тела (RW-39 / NET-C1).
//!
//! Зачем: `discord-core` умел отправить запрос и вернуть байты, но не превратить их в
//! пользователя (аудит NET-B, P0-4), поэтому проверить токен и показать сессию было нечем.
//! Разбор — production-парсер [`discord_json`] (без serde): те же правила RFC 8259,
//! duplicate keys и лимиты глубины/узлов, что и в тестах.
//!
//! Оракул формы: `docs/re/rest-d02.md` §2.1 (маршрут `GET /users/@me`), схема user-объекта —
//! `docs/re/gateway-schema.md` (READY `user`) и `docs/re/rest-d02.md` §6.2. Тело ошибки —
//! форма Discord `{"message": "...", "code": N}`; 401 отдаётся как [`MeError::Unauthorized`].

use std::fmt;

use discord_json::{JsonError, JsonValue};

/// Текущий пользователь из `GET /users/@me` (только поля, нужные клиенту).
///
/// `id` — снежинка строкой: число не влезает в безопасный JSON-диапазон и не должно терять
/// точность. `global_name`/`avatar` — nullable в API; `bot` отсутствует у обычных аккаунтов.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Me {
    /// ID пользователя (снежинка, строка).
    pub id: String,
    /// Имя пользователя (без дискриминатора).
    pub username: String,
    /// Дискриминатор (`"0"` у новых аккаунтов).
    pub discriminator: String,
    /// Отображаемое имя (nullable).
    pub global_name: Option<String>,
    /// Хеш аватара (nullable).
    pub avatar: Option<String>,
    /// Бот ли аккаунт (в API поле может отсутствовать — тогда `false`).
    pub bot: bool,
}

/// Ошибка разбора `/users/@me`: каждый исход типизирован, секретов в сообщениях нет.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeError {
    /// Тело не строгий UTF-8.
    NotUtf8,
    /// Тело не JSON (позиция — в [`JsonError`]).
    Json(JsonError),
    /// Корень JSON — не объект.
    NotObject,
    /// Обязательное поле отсутствует.
    MissingField(&'static str),
    /// Поле есть, но другого типа.
    InvalidField(&'static str),
    /// Тело ошибки Discord — 401 (токен истёк/недействителен).
    Unauthorized,
    /// Прочее тело ошибки Discord (`{"message", "code", …}`).
    Discord {
        /// Код ошибки Discord (`code`), если есть.
        code: Option<i64>,
        /// Текст `message` без секретов (сервер не присылает токен в ошибке).
        message: String,
    },
}

impl fmt::Display for MeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotUtf8 => write!(formatter, "тело /users/@me не UTF-8"),
            Self::Json(error) => write!(formatter, "тело /users/@me не JSON: {error}"),
            Self::NotObject => write!(formatter, "тело /users/@me — не объект"),
            Self::MissingField(field) => {
                write!(formatter, "в /users/@me нет обязательного поля {field}")
            }
            Self::InvalidField(field) => {
                write!(formatter, "поле {field} в /users/@me другого типа")
            }
            Self::Unauthorized => write!(formatter, "401: токен недействителен или истёк"),
            Self::Discord { code, message } => match code {
                Some(code) => write!(formatter, "ошибка Discord {code}: {message}"),
                None => write!(formatter, "ошибка Discord: {message}"),
            },
        }
    }
}

impl std::error::Error for MeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::NotUtf8
            | Self::NotObject
            | Self::MissingField(_)
            | Self::InvalidField(_)
            | Self::Unauthorized
            | Self::Discord { .. } => None,
        }
    }
}

/// Разобрать тело ответа `GET /users/@me`.
///
/// Тело ошибки Discord (`message`/`code` без `id`) не выдаётся за пользователя: 401 —
/// [`MeError::Unauthorized`], остальные — [`MeError::Discord`].
///
/// # Errors
///
/// [`MeError`]: не UTF-8/не JSON, корень не объект, нет обязательного поля, неверный тип
/// поля, тело ошибки API.
pub fn parse_me(body: &[u8]) -> Result<Me, MeError> {
    let text = std::str::from_utf8(body).map_err(|_| MeError::NotUtf8)?;
    let value = discord_json::parse(text).map_err(MeError::Json)?;
    if value.as_object().is_none() {
        return Err(MeError::NotObject);
    }
    if value.get("id").is_none()
        && let Some((code, message)) = discord_error(&value)
    {
        return Err(if message.starts_with("401") {
            MeError::Unauthorized
        } else {
            MeError::Discord {
                code,
                message: message.to_string(),
            }
        });
    }

    Ok(Me {
        id: required_string(&value, "id")?,
        username: required_string(&value, "username")?,
        discriminator: required_string(&value, "discriminator")?,
        global_name: optional_string(&value, "global_name")?,
        avatar: optional_string(&value, "avatar")?,
        bot: optional_bool(&value, "bot")?,
    })
}

/// Тело ошибки Discord: `message` строкой и `code` числом (если есть).
fn discord_error(value: &JsonValue) -> Option<(Option<i64>, &str)> {
    let message = value.get("message")?.as_str()?;
    Some((value.get("code").and_then(JsonValue::as_i64), message))
}

/// Обязательное строковое поле.
fn required_string(value: &JsonValue, field: &'static str) -> Result<String, MeError> {
    value
        .get(field)
        .ok_or(MeError::MissingField(field))?
        .as_str()
        .map(str::to_string)
        .ok_or(MeError::InvalidField(field))
}

/// Nullable строковое поле: отсутствие и `null` — `None`.
fn optional_string(value: &JsonValue, field: &'static str) -> Result<Option<String>, MeError> {
    match value.get(field) {
        None | Some(JsonValue::Null) => Ok(None),
        Some(JsonValue::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(MeError::InvalidField(field)),
    }
}

/// Булево поле с дефолтом `false` (в API `bot` есть только у ботов).
fn optional_bool(value: &JsonValue, field: &'static str) -> Result<bool, MeError> {
    match value.get(field) {
        None => Ok(false),
        Some(JsonValue::Bool(flag)) => Ok(*flag),
        Some(_) => Err(MeError::InvalidField(field)),
    }
}
