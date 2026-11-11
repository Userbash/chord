//! RW-40 / task-253 (`BE-P1-29`) + RW-41 / TASK-814 (`G1 Backend`): BFF-контракт slash-команд
//! приложений и взаимодействий (`slash_commands`: `APPLICATION_COMMAND` type 2 и
//! `MESSAGE_COMPONENT` type 3).
//!
//! Источники контракта:
//! - `/home/sanya/re-deep/rest-static.json:7720-7731` (`Endpoints.INTERACTIONS: "/interactions"`,
//!   поля тела `{type,application_id,guild_id,channel_id,session_id,data,nonce}`);
//! - Discord API v9: типы взаимодействий (`2 = APPLICATION_COMMAND`, `3 = MESSAGE_COMPONENT`,
//!   `4 = APPLICATION_COMMAND_AUTOCOMPLETE`, `5 = MODAL_SUBMIT`), типы команд приложений
//!   (`1 = CHAT_INPUT`, `2 = USER`, `3 = MESSAGE`), типы компонентов
//!   (`1 = ACTION_ROW`, `2 = BUTTON`, `3 = STRING_SELECT`, `4 = TEXT_INPUT`,
//!    `5 = USER_SELECT`, `6 = ROLE_SELECT`, `7 = MENTIONABLE_SELECT`, `8 = CHANNEL_SELECT`),
//!   ограничения имени slash-команды (`1..=32` символов `[-_a-z0-9]`), числа опций (`<= 25`)
//!   и custom_id компонентов сообщений (`1..=100` символов).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Тип взаимодействия `2` — вызов команды приложения (`APPLICATION_COMMAND`).
pub const INTERACTION_TYPE_APPLICATION_COMMAND: u8 = 2;

/// Тип взаимодействия `3` — нажатие/выбор компонента сообщения (`MESSAGE_COMPONENT`).
pub const INTERACTION_TYPE_MESSAGE_COMPONENT: u8 = 3;

/// Тип взаимодействия `4` — автодополнение аргументов команды (`APPLICATION_COMMAND_AUTOCOMPLETE`).
pub const INTERACTION_TYPE_AUTOCOMPLETE: u8 = 4;

/// Тип взаимодействия `5` — отправка модального окна (`MODAL_SUBMIT`).
pub const INTERACTION_TYPE_MODAL_SUBMIT: u8 = 5;

/// Перечисление типов взаимодействий Discord API v9.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InteractionType {
    /// Тип `2`: `APPLICATION_COMMAND`
    ApplicationCommand = 2,
    /// Тип `3`: `MESSAGE_COMPONENT`
    MessageComponent = 3,
    /// Тип `4`: `APPLICATION_COMMAND_AUTOCOMPLETE`
    Autocomplete = 4,
    /// Тип `5`: `MODAL_SUBMIT`
    ModalSubmit = 5,
}

impl InteractionType {
    /// Числовой код взаимодействия (`2..=5`).
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Преобразует числовой код (`2..=5`) в [`InteractionType`].
    pub const fn from_u8(code: u8) -> Option<Self> {
        match code {
            2 => Some(Self::ApplicationCommand),
            3 => Some(Self::MessageComponent),
            4 => Some(Self::Autocomplete),
            5 => Some(Self::ModalSubmit),
            _ => None,
        }
    }
}

impl Serialize for InteractionType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u8(self.as_u8())
    }
}

impl<'de> Deserialize<'de> for InteractionType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = InteractionType;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("целое число (2..=5) или имя типа взаимодействия")
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v <= u8::MAX as u64 {
                    if let Some(it) = InteractionType::from_u8(v as u8) {
                        return Ok(it);
                    }
                }
                Err(E::custom(format!("неизвестный код взаимодействия: {v}")))
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v >= 0 && v <= u8::MAX as i64 {
                    if let Some(it) = InteractionType::from_u8(v as u8) {
                        return Ok(it);
                    }
                }
                Err(E::custom(format!("неизвестный код взаимодействия: {v}")))
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match v {
                    "ApplicationCommand" | "APPLICATION_COMMAND" => {
                        Ok(InteractionType::ApplicationCommand)
                    }
                    "MessageComponent" | "MESSAGE_COMPONENT" => {
                        Ok(InteractionType::MessageComponent)
                    }
                    "Autocomplete" | "APPLICATION_COMMAND_AUTOCOMPLETE" => {
                        Ok(InteractionType::Autocomplete)
                    }
                    "ModalSubmit" | "MODAL_SUBMIT" => Ok(InteractionType::ModalSubmit),
                    _ => Err(E::custom(format!("неизвестный тип взаимодействия: {v}"))),
                }
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

/// Тип команды приложения `1` — текстовая slash-команда (`CHAT_INPUT`).
pub const COMMAND_TYPE_CHAT_INPUT: u8 = 1;

/// Тип команды приложения `2` — контекстная команда пользователя (`USER`).
pub const COMMAND_TYPE_USER: u8 = 2;

/// Тип команды приложения `3` — контекстная команда сообщения (`MESSAGE`).
pub const COMMAND_TYPE_MESSAGE: u8 = 3;

/// Тип компонента сообщения `1` — строка действий (`ACTION_ROW`).
pub const COMPONENT_TYPE_ACTION_ROW: u8 = 1;

/// Тип компонента сообщения `2` — кнопка (`BUTTON`).
pub const COMPONENT_TYPE_BUTTON: u8 = 2;

/// Тип компонента сообщения `3` — выпадающий список строк (`STRING_SELECT`).
pub const COMPONENT_TYPE_STRING_SELECT: u8 = 3;

/// Тип компонента сообщения `4` — поле текстового ввода модалки (`TEXT_INPUT`).
pub const COMPONENT_TYPE_TEXT_INPUT: u8 = 4;

/// Тип компонента сообщения `5` — селектор пользователей (`USER_SELECT`).
pub const COMPONENT_TYPE_USER_SELECT: u8 = 5;

/// Тип компонента сообщения `6` — селектор ролей (`ROLE_SELECT`).
pub const COMPONENT_TYPE_ROLE_SELECT: u8 = 6;

/// Тип компонента сообщения `7` — селектор упоминаемых (`MENTIONABLE_SELECT`).
pub const COMPONENT_TYPE_MENTIONABLE_SELECT: u8 = 7;

/// Тип компонента сообщения `8` — селектор каналов (`CHANNEL_SELECT`).
pub const COMPONENT_TYPE_CHANNEL_SELECT: u8 = 8;

/// Максимальная длина имени slash-команды в символах (`32`).
pub const MAX_COMMAND_NAME_CHARS: usize = 32;

/// Максимальное число опций при вызове slash-команды (`25`).
pub const MAX_COMMAND_OPTIONS: usize = 25;

/// Максимальная длина `custom_id` компонента в символах (`100`).
pub const MAX_COMPONENT_CUSTOM_ID_CHARS: usize = 100;

/// Дефолтное значение типа компонента для клика (`BUTTON` = 2).
fn default_button_component_type() -> u8 {
    COMPONENT_TYPE_BUTTON
}

/// Запрос на вызов slash-команды приложения (`POST /api/v9/interactions`).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct InvokeSlashCommandRequest {
    /// Snowflake ID приложения (`application_id`).
    pub application_id: String,
    /// Опциональный Snowflake ID гильдии (`guild_id`, `None` в личных сообщениях).
    pub guild_id: Option<String>,
    /// Snowflake ID канала (`channel_id`).
    pub channel_id: String,
    /// Идентификатор активной Gateway-сессии (`session_id`).
    pub session_id: String,
    /// Snowflake ID зарегистрированной команды (`data.id`).
    pub command_id: String,
    /// Snowflake ID версии команды (`data.version`).
    pub command_version: String,
    /// Имя вызываемой slash-команды (`1..=32` символов `[-_a-z0-9]`).
    pub command_name: String,
    /// Аргументы/опции вызова команды (`0..=25` элементов).
    pub options: Vec<Value>,
    /// Положительный числовой Snowflake-токен идемпотентности (`nonce`).
    pub nonce: String,
}

/// TASK-814: Запрос на нажатие компонента сообщения (`MESSAGE_COMPONENT`, `type = 3`).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ClickMessageComponentRequest {
    /// Snowflake ID приложения / бота (`application_id`).
    pub application_id: String,
    /// Опциональный Snowflake ID гильдии (`guild_id`, `None` в личных сообщениях).
    #[serde(default)]
    pub guild_id: Option<String>,
    /// Snowflake ID канала (`channel_id`).
    pub channel_id: String,
    /// Snowflake ID сообщения с кнопкой (`message_id`).
    pub message_id: String,
    /// Идентификатор активной Gateway-сессии (`session_id`).
    pub session_id: String,
    /// Пользовательский идентификатор кнопки (`data.custom_id`, `1..=100` символов).
    pub custom_id: String,
    /// Тип компонента (`data.component_type`, по умолчанию `2 = BUTTON`).
    #[serde(default = "default_button_component_type")]
    pub component_type: u8,
    /// Положительный числовой Snowflake-токен идемпотентности (`nonce`).
    pub nonce: String,
}

/// Проверяет, что строка является положительным числовым Snowflake ID
/// (`1..=20` ASCII-цифр, не начинается с `'0'`).
fn validate_snowflake(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 20
        || value.starts_with('0')
        || !value.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(format!("некорректный Snowflake ID в поле {field}"));
    }
    Ok(())
}

/// Валидирует имя slash-команды:
/// - длина `1..=MAX_COMMAND_NAME_CHARS` (`1..=32`) символов;
/// - все символы — строчные ASCII-буквы, ASCII-цифры, `'-'` или `'_'`
///   (`c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_'`).
pub fn validate_slash_command_name(name: &str) -> Result<(), String> {
    let char_count = name.chars().count();
    if !(1..=MAX_COMMAND_NAME_CHARS).contains(&char_count) {
        return Err(format!(
            "длина имени slash-команды должна быть от 1 до {MAX_COMMAND_NAME_CHARS} символов (получено {char_count})"
        ));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        return Err(
            "имя slash-команды может содержать только строчные ASCII-буквы, цифры, '-' или '_'"
                .to_string(),
        );
    }
    Ok(())
}

/// Валидирует `custom_id` компонента сообщения:
/// - непустая строка без ведущих/хвостовых пробелов;
/// - длина `1..=MAX_COMPONENT_CUSTOM_ID_CHARS` (`1..=100`) символов;
/// - отсутствие управляющих символов (`!c.is_control()`).
pub fn validate_component_custom_id(custom_id: &str) -> Result<(), String> {
    if custom_id.trim().is_empty() {
        return Err("custom_id не может быть пустым".to_string());
    }
    if custom_id.starts_with(' ') || custom_id.ends_with(' ') {
        return Err("custom_id не может начинаться или заканчиваться пробелом".to_string());
    }
    let char_count = custom_id.chars().count();
    if !(1..=MAX_COMPONENT_CUSTOM_ID_CHARS).contains(&char_count) {
        return Err(format!(
            "длина custom_id должна быть от 1 до {MAX_COMPONENT_CUSTOM_ID_CHARS} символов (получено {char_count})"
        ));
    }
    if custom_id.chars().any(|c| c.is_control()) {
        return Err("custom_id не может содержать управляющие символы".to_string());
    }
    Ok(())
}

/// Валидирует тип компонента при клике по кнопке (`component_type == 2 = BUTTON`).
pub fn validate_message_component_type(component_type: u8) -> Result<(), String> {
    if component_type != COMPONENT_TYPE_BUTTON {
        return Err(format!(
            "неподдерживаемый тип компонента для клика по кнопке: {component_type} (ожидается {COMPONENT_TYPE_BUTTON} = BUTTON)"
        ));
    }
    Ok(())
}

/// Валидирует [`InvokeSlashCommandRequest`] перед отправкой в Discord REST API v9:
/// - `application_id`, `channel_id`, `command_id`, `command_version`, `nonce` — положительные
///   числовые Snowflake (`1..=20` ASCII-цифр, не начинаются с `'0'`);
/// - `guild_id` — если `Some`, также положительный числовой Snowflake;
/// - `session_id.trim()` — непустая строка;
/// - `command_name` — проходит [`validate_slash_command_name`];
/// - `options.len() <= MAX_COMMAND_OPTIONS` (`25`).
pub fn validate_invoke_slash_command(req: &InvokeSlashCommandRequest) -> Result<(), String> {
    validate_snowflake("application_id", &req.application_id)?;
    if let Some(guild_id) = &req.guild_id {
        validate_snowflake("guild_id", guild_id)?;
    }
    validate_snowflake("channel_id", &req.channel_id)?;
    validate_snowflake("command_id", &req.command_id)?;
    validate_snowflake("command_version", &req.command_version)?;
    validate_snowflake("nonce", &req.nonce)?;

    if req.session_id.trim().is_empty() {
        return Err("поле session_id не может быть пустым".to_string());
    }

    validate_slash_command_name(&req.command_name)?;

    if req.options.len() > MAX_COMMAND_OPTIONS {
        return Err(format!(
            "число опций slash-команды не может превышать {MAX_COMMAND_OPTIONS} (получено {})",
            req.options.len()
        ));
    }

    Ok(())
}

/// TASK-814: Валидирует [`ClickMessageComponentRequest`] перед отправкой в Discord REST API v9:
/// - `application_id`, `channel_id`, `message_id`, `nonce` — валидные Snowflake;
/// - `guild_id` — если `Some`, валидный Snowflake;
/// - `session_id.trim()` — непустая строка;
/// - `custom_id` — проходит [`validate_component_custom_id`];
/// - `component_type` — проходит [`validate_message_component_type`].
pub fn validate_click_message_component(req: &ClickMessageComponentRequest) -> Result<(), String> {
    validate_snowflake("application_id", &req.application_id)?;
    if let Some(guild_id) = &req.guild_id {
        validate_snowflake("guild_id", guild_id)?;
    }
    validate_snowflake("channel_id", &req.channel_id)?;
    validate_snowflake("message_id", &req.message_id)?;
    validate_snowflake("nonce", &req.nonce)?;

    if req.session_id.trim().is_empty() {
        return Err("поле session_id не может быть пустым".to_string());
    }

    validate_component_custom_id(&req.custom_id)?;
    validate_message_component_type(req.component_type)?;

    Ok(())
}

/// Валидирует [`InvokeSlashCommandRequest`] и строит маршрут и JSON-тело для
/// `POST /api/v9/interactions`.
pub fn build_interactions_route_and_body(
    req: &InvokeSlashCommandRequest,
) -> Result<(String, Value), String> {
    validate_invoke_slash_command(req)?;

    let route = "/api/v9/interactions".to_string();
    let body = json!({
        "type": INTERACTION_TYPE_APPLICATION_COMMAND,
        "application_id": req.application_id,
        "guild_id": req.guild_id,
        "channel_id": req.channel_id,
        "session_id": req.session_id.trim(),
        "nonce": req.nonce,
        "data": {
            "version": req.command_version,
            "id": req.command_id,
            "name": req.command_name,
            "type": COMMAND_TYPE_CHAT_INPUT,
            "options": req.options,
        }
    });

    Ok((route, body))
}

/// TASK-814: Валидирует [`ClickMessageComponentRequest`] и строит маршрут и JSON-тело
/// для `POST /api/v9/interactions` (`type = 3 = MESSAGE_COMPONENT`).
pub fn build_message_component_interactions_route_and_body(
    req: &ClickMessageComponentRequest,
) -> Result<(String, Value), String> {
    validate_click_message_component(req)?;

    let route = "/api/v9/interactions".to_string();
    let body = json!({
        "type": INTERACTION_TYPE_MESSAGE_COMPONENT,
        "application_id": req.application_id,
        "guild_id": req.guild_id,
        "channel_id": req.channel_id,
        "message_id": req.message_id,
        "session_id": req.session_id.trim(),
        "nonce": req.nonce,
        "data": {
            "component_type": req.component_type,
            "custom_id": req.custom_id,
        }
    });

    Ok((route, body))
}
