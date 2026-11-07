//! RW-40 / task-239 (`BE-P1-25`): BFF-контракт запланированных событий гильдии (`scheduled_events`).
//!
//! Источники контракта:
//! - `/home/sanya/re-deep/rest-static.json:8205-8284` (`Endpoints.GUILD_EVENTS_FOR_GUILD`, `Endpoints.GUILD_EVENT`, `Endpoints.USER_GUILD_EVENT`);
//! - Discord API v9: типы сущностей событий (`1 = STAGE`, `2 = VOICE`, `3 = EXTERNAL`),
//!   статусы событий (`1 = SCHEDULED`, `2 = ACTIVE`, `3 = COMPLETED`, `4 = CANCELED`),
//!   уровень приватности `2 = GUILD_ONLY`, лимиты названия (`1..=100`), описания (`<= 1000`) и локации (`1..=100`).

use serde::{Deserialize, Serialize};

/// Тип сущности запланированного события `1` — трибуна (`STAGE_INSTANCE`).
pub const EVENT_ENTITY_STAGE: u8 = 1;

/// Тип сущности запланированного события `2` — голосовой канал (`VOICE`).
pub const EVENT_ENTITY_VOICE: u8 = 2;

/// Тип сущности запланированного события `3` — внешняя локация (`EXTERNAL`).
pub const EVENT_ENTITY_EXTERNAL: u8 = 3;

/// Статус запланированного события `1` — запланировано (`SCHEDULED`).
pub const EVENT_STATUS_SCHEDULED: u8 = 1;

/// Статус запланированного события `2` — активно (`ACTIVE`).
pub const EVENT_STATUS_ACTIVE: u8 = 2;

/// Статус запланированного события `3` — завершено (`COMPLETED`).
pub const EVENT_STATUS_COMPLETED: u8 = 3;

/// Статус запланированного события `4` — отменено (`CANCELED`).
pub const EVENT_STATUS_CANCELED: u8 = 4;

/// Максимальная длина названия события в символах Unicode (`1..=100`).
pub const MAX_EVENT_NAME_CHARS: usize = 100;

/// Максимальная длина описания события в символах Unicode (`<= 1000`).
pub const MAX_EVENT_DESC_CHARS: usize = 1000;

/// Максимальная длина внешней локации события в символах Unicode (`1..=100`).
pub const MAX_EVENT_LOCATION_CHARS: usize = 100;

/// Уровень приватности события гильдии (`GUILD_ONLY = 2`).
const PRIVACY_LEVEL_GUILD_ONLY: u8 = 2;

/// Запрос на создание запланированного события гильдии (`POST /api/v9/guilds/{guild_id}/scheduled-events`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateScheduledEventRequest {
    /// Название события (`1..=MAX_EVENT_NAME_CHARS` символов Unicode после `trim()`).
    pub name: String,
    /// Опциональное описание события (`<= MAX_EVENT_DESC_CHARS` символов Unicode).
    pub description: Option<String>,
    /// Тип сущности события (`1 = STAGE`, `2 = VOICE`, `3 = EXTERNAL`).
    pub entity_type: u8,
    /// Snowflake ID канала (обязателен для `STAGE` и `VOICE`).
    pub channel_id: Option<String>,
    /// Текстовая локация события (обязательна для `EXTERNAL`, `1..=MAX_EVENT_LOCATION_CHARS` символов после `trim()`).
    pub location: Option<String>,
    /// ISO-8601 время начала события (после `trim()` непустая строка).
    pub scheduled_start_time: String,
    /// ISO-8601 время окончания события (обязательно для `EXTERNAL`, после `trim()` непустая строка).
    pub scheduled_end_time: Option<String>,
    /// Уровень приватности события (должен быть равен `2` — `GUILD_ONLY`).
    pub privacy_level: u8,
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

/// Валидирует запрос на создание запланированного события [`CreateScheduledEventRequest`]:
/// - `req.name.trim()` должен содержать от `1` до [`MAX_EVENT_NAME_CHARS`] (`100`) символов Unicode;
/// - `req.description`, если `Some`, должен содержать не более [`MAX_EVENT_DESC_CHARS`] (`1000`) символов Unicode;
/// - `req.privacy_level` должен быть равен `2` (`GUILD_ONLY`);
/// - `req.scheduled_start_time.trim()` должен быть непустым;
/// - при `entity_type == 1` (`STAGE`) или `2` (`VOICE`): `channel_id` должен быть `Some` с валидным положительным числовым Snowflake (`1..=20` цифр, без ведущего `'0'`);
/// - при `entity_type == 3` (`EXTERNAL`): `location` должен быть `Some` с непустой `trim()`-строкой `<= 100` символов Unicode И `scheduled_end_time` должен быть `Some` с непустой `trim()`-строкой;
/// - иные значения `entity_type` возвращают `Err`.
pub fn validate_scheduled_event_request(req: &CreateScheduledEventRequest) -> Result<(), String> {
    let name_len = req.name.trim().chars().count();
    if !(1..=MAX_EVENT_NAME_CHARS).contains(&name_len) {
        return Err(format!(
            "длина названия события после trim должна быть от 1 до {MAX_EVENT_NAME_CHARS} символов (получено {name_len})"
        ));
    }

    if let Some(desc) = req.description.as_deref() {
        let desc_len = desc.chars().count();
        if desc_len > MAX_EVENT_DESC_CHARS {
            return Err(format!(
                "описание события превышает лимит {MAX_EVENT_DESC_CHARS} символов (получено {desc_len})"
            ));
        }
    }

    if req.privacy_level != PRIVACY_LEVEL_GUILD_ONLY {
        return Err(format!(
            "неподдерживаемый privacy_level={} (ожидается {PRIVACY_LEVEL_GUILD_ONLY} = GUILD_ONLY)",
            req.privacy_level
        ));
    }

    if req.scheduled_start_time.trim().is_empty() {
        return Err("scheduled_start_time не может быть пустым".to_string());
    }

    match req.entity_type {
        EVENT_ENTITY_STAGE | EVENT_ENTITY_VOICE => {
            let cid = req.channel_id.as_deref().ok_or_else(|| {
                "для события STAGE/VOICE обязателен валидный channel_id".to_string()
            })?;
            validate_snowflake("channel_id", cid)?;
        }
        EVENT_ENTITY_EXTERNAL => {
            let loc = req
                .location
                .as_deref()
                .ok_or_else(|| "для события EXTERNAL обязательна локация (location)".to_string())?;
            let loc_len = loc.trim().chars().count();
            if !(1..=MAX_EVENT_LOCATION_CHARS).contains(&loc_len) {
                return Err(format!(
                    "длина location после trim должна быть от 1 до {MAX_EVENT_LOCATION_CHARS} символов (получено {loc_len})"
                ));
            }

            let end_time = req.scheduled_end_time.as_deref().ok_or_else(|| {
                "для события EXTERNAL обязательно время окончания (scheduled_end_time)".to_string()
            })?;
            if end_time.trim().is_empty() {
                return Err(
                    "scheduled_end_time для события EXTERNAL не может быть пустым".to_string(),
                );
            }
        }
        other => {
            return Err(format!("неподдерживаемый entity_type={other}"));
        }
    }

    Ok(())
}

/// Валидирует `guild_id` как положительный числовой Snowflake (`1..=20` цифр, без ведущего `'0'`)
/// и возвращает маршрут списка событий гильдии:
/// `"/api/v9/guilds/{guild_id}/scheduled-events?with_user_count={with_user_count}"`.
pub fn build_guild_events_route(guild_id: &str, with_user_count: bool) -> Result<String, String> {
    validate_snowflake("guild_id", guild_id)?;
    Ok(format!(
        "/api/v9/guilds/{guild_id}/scheduled-events?with_user_count={with_user_count}"
    ))
}

/// Валидирует `guild_id` и `event_id` как положительные числовые Snowflake (`1..=20` цифр, без ведущего `'0'`)
/// и возвращает маршрут подписки (RSVP) текущего пользователя на событие:
/// `"/api/v9/guilds/{guild_id}/scheduled-events/{event_id}/users/@me"`.
pub fn build_event_rsvp_route(guild_id: &str, event_id: &str) -> Result<String, String> {
    validate_snowflake("guild_id", guild_id)?;
    validate_snowflake("event_id", event_id)?;
    Ok(format!(
        "/api/v9/guilds/{guild_id}/scheduled-events/{event_id}/users/@me"
    ))
}
