//! RW-40 / task-265 (`BE-P1-32`): BFF-контракт трибун (Stage Channel Instances) и запроса слова (`stage_instances`).
//!
//! Источники контракта:
//! - `/home/sanya/re-deep/rest-static.json:8088-8099` (`Endpoints.STAGE_INSTANCES: "POST /stage-instances"`);
//! - `/home/sanya/re-deep/rest-static.json:8114-8124` (`Endpoints.STAGE_INSTANCE: "DELETE|PATCH /stage-instances/{arg}"`);
//! - `/home/sanya/re-deep/rest-static.json:7915-7928` (`Endpoints.UPDATE_VOICE_STATE: "PATCH /guilds/{arg}/voice-states/{arg}"`);
//! - Discord API v9:
//!   - длина темы трибуны после `trim()` — `1..=120` символов Unicode;
//!   - уровень приватности трибуны `privacy_level = 2` (`GUILD_ONLY`);
//!   - обновление собственного состояния голоса на трибуне (`PATCH /api/v9/guilds/{guild_id}/voice-states/@me`)
//!     передаёт `channel_id`, `suppress` и `request_to_speak_timestamp`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Минимальная длина темы трибуны после `trim()` (`1` символ Unicode).
pub const MIN_STAGE_TOPIC_CHARS: usize = 1;

/// Максимальная длина темы трибуны после `trim()` (`120` символов Unicode).
pub const MAX_STAGE_TOPIC_CHARS: usize = 120;

/// Уровень приватности трибуны «только для участников сервера» (`GUILD_ONLY = 2`).
pub const STAGE_PRIVACY_GUILD_ONLY: u8 = 2;

/// Запрос на создание экземпляра трибуны (`POST /api/v9/stage-instances`).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CreateStageInstanceRequest {
    /// Snowflake ID трибуны (Stage-канала).
    pub channel_id: String,
    /// Тема трибуны (`1..=120` символов Unicode после `trim()`).
    pub topic: String,
    /// Уровень приватности трибуны (`STAGE_PRIVACY_GUILD_ONLY = 2`).
    pub privacy_level: u8,
    /// Отправить ли уведомление о начале трибуны участникам сервера.
    pub send_start_notification: Option<bool>,
}

/// Запрос на обновление собственного состояния голоса на трибуне
/// (`PATCH /api/v9/guilds/{guild_id}/voice-states/@me`).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct UpdateSelfVoiceStageRequest {
    /// Snowflake ID трибуны (Stage-канала).
    pub channel_id: String,
    /// Находится ли пользователь в аудитории (`Some(true)`) или выступает спикером (`Some(false)`).
    pub suppress: Option<bool>,
    /// ISO-8601 таймстемп запроса слова (`Some(...)` при поднятии руки, `None` при отмене).
    pub request_to_speak_timestamp: Option<String>,
}

/// Проверяет, что строка является валидным положительным числовым Snowflake ID
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

/// Валидирует запрос на создание экземпляра трибуны [`CreateStageInstanceRequest`]:
/// - проверяет `req.channel_id` как положительный числовой Snowflake (`1..=20` ASCII-цифр, без ведущего `'0'`);
/// - проверяет длину `req.topic.trim().chars().count()` в диапазоне [`MIN_STAGE_TOPIC_CHARS`]..=[`MAX_STAGE_TOPIC_CHARS`] (`1..=120`);
/// - проверяет `req.privacy_level == STAGE_PRIVACY_GUILD_ONLY` (`2`).
pub fn validate_create_stage_instance(req: &CreateStageInstanceRequest) -> Result<(), String> {
    validate_snowflake("channel_id", &req.channel_id)?;

    let topic_chars = req.topic.trim().chars().count();
    if !(MIN_STAGE_TOPIC_CHARS..=MAX_STAGE_TOPIC_CHARS).contains(&topic_chars) {
        return Err(format!(
            "длина темы трибуны после trim должна быть от {MIN_STAGE_TOPIC_CHARS} до {MAX_STAGE_TOPIC_CHARS} символов (получено {topic_chars})"
        ));
    }

    if req.privacy_level != STAGE_PRIVACY_GUILD_ONLY {
        return Err(format!(
            "privacy_level трибуны должен быть равен STAGE_PRIVACY_GUILD_ONLY ({STAGE_PRIVACY_GUILD_ONLY}), получено {}",
            req.privacy_level
        ));
    }

    Ok(())
}

/// Валидирует [`CreateStageInstanceRequest`] через [`validate_create_stage_instance`] и строит
/// маршрут и JSON-тело для `POST /api/v9/stage-instances`.
pub fn build_create_stage_instance_route_and_body(
    req: &CreateStageInstanceRequest,
) -> Result<(String, Value), String> {
    validate_create_stage_instance(req)?;

    let route = "/api/v9/stage-instances".to_string();
    let body = json!({
        "channel_id": req.channel_id,
        "topic": req.topic.trim(),
        "privacy_level": req.privacy_level,
        "send_start_notification": req.send_start_notification,
    });
    Ok((route, body))
}

/// Валидирует `guild_id`, `req.channel_id` (положительные числовые Snowflake `1..=20` ASCII-цифр,
/// без ведущего `'0'`) и `req.request_to_speak_timestamp` (если `Some(ts)`, то `!ts.trim().is_empty()`),
/// после чего возвращает маршрут и JSON-тело для `PATCH /api/v9/guilds/{guild_id}/voice-states/@me`.
pub fn build_update_self_stage_voice_state_route_and_body(
    guild_id: &str,
    req: &UpdateSelfVoiceStageRequest,
) -> Result<(String, Value), String> {
    validate_snowflake("guild_id", guild_id)?;
    validate_snowflake("channel_id", &req.channel_id)?;

    if let Some(ref ts) = req.request_to_speak_timestamp {
        if ts.trim().is_empty() {
            return Err(
                "request_to_speak_timestamp не может быть пустой строкой или состоять только из пробелов"
                    .to_string(),
            );
        }
    }

    let route = format!("/api/v9/guilds/{guild_id}/voice-states/@me");
    let body = json!({
        "channel_id": req.channel_id,
        "suppress": req.suppress,
        "request_to_speak_timestamp": req.request_to_speak_timestamp,
    });
    Ok((route, body))
}
