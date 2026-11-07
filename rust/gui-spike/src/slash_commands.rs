//! RW-40 / task-253 (`BE-P1-29`): BFF-контракт slash-команд приложений и взаимодействий (`slash_commands`).
//!
//! Источники контракта:
//! - `/home/sanya/re-deep/rest-static.json:7720-7731` (`Endpoints.INTERACTIONS: "/interactions"`,
//!   поля тела `{type,application_id,guild_id,channel_id,session_id,data,nonce}`);
//! - Discord API v9: типы взаимодействий (`2 = APPLICATION_COMMAND`, `3 = MESSAGE_COMPONENT`,
//!   `4 = APPLICATION_COMMAND_AUTOCOMPLETE`, `5 = MODAL_SUBMIT`), типы команд приложений
//!   (`1 = CHAT_INPUT`, `2 = USER`, `3 = MESSAGE`), ограничения имени slash-команды
//!   (`1..=32` символов `[-_a-z0-9]`) и числа опций (`<= 25`).

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

/// Тип команды приложения `1` — текстовая slash-команда (`CHAT_INPUT`).
pub const COMMAND_TYPE_CHAT_INPUT: u8 = 1;

/// Тип команды приложения `2` — контекстная команда пользователя (`USER`).
pub const COMMAND_TYPE_USER: u8 = 2;

/// Тип команды приложения `3` — контекстная команда сообщения (`MESSAGE`).
pub const COMMAND_TYPE_MESSAGE: u8 = 3;

/// Максимальная длина имени slash-команды в символах (`32`).
pub const MAX_COMMAND_NAME_CHARS: usize = 32;

/// Максимальное число опций при вызове slash-команды (`25`).
pub const MAX_COMMAND_OPTIONS: usize = 25;

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
