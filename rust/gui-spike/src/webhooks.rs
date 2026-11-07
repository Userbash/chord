//! RW-40 / task-245 (`BE-P1-27`): BFF-контракт вебхуков и подписки на новостные каналы (`webhooks`).
//!
//! Источники контракта:
//! - `/home/sanya/re-deep/rest-static.json:4910-4916` (`Endpoints.CHANNEL_WEBHOOKS: "/channels/{arg}/webhooks"`);
//! - `/home/sanya/re-deep/rest-static.json:2712-2720` (`Endpoints.CHANNEL_FOLLOWERS: "/channels/{arg}/followers"`, поле `webhook_channel_id`);
//! - Discord API v9: типы вебхуков (`1 = INCOMING`, `2 = CHANNEL_FOLLOWER`, `3 = APPLICATION`),
//!   ограничение имени вебхука (`1..=80` символов Unicode после `trim()`, запрет имени `"clyde"` и
//!   подстроки `"discord"` без учёта регистра).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Тип вебхука `1` — входящий вебхук (`INCOMING`).
pub const WEBHOOK_TYPE_INCOMING: u8 = 1;

/// Тип вебхука `2` — вебхук подписки на новостной канал (`CHANNEL_FOLLOWER`).
pub const WEBHOOK_TYPE_CHANNEL_FOLLOWER: u8 = 2;

/// Тип вебхука `3` — вебхук приложения (`APPLICATION`).
pub const WEBHOOK_TYPE_APPLICATION: u8 = 3;

/// Минимальная длина имени вебхука в символах Unicode после `trim()` (`1`).
pub const MIN_WEBHOOK_NAME_CHARS: usize = 1;

/// Максимальная длина имени вебхука в символах Unicode после `trim()` (`80`).
pub const MAX_WEBHOOK_NAME_CHARS: usize = 80;

/// Запрос на создание вебхука канала (`POST /api/v9/channels/{channel_id}/webhooks`).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CreateWebhookRequest {
    /// Имя вебхука (`1..=80` символов Unicode после `trim()`, без `"clyde"` и подстроки `"discord"`).
    pub name: String,
    /// Опциональный Data URI аватара вебхука (`data:image/...;base64,...`).
    pub avatar: Option<String>,
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

/// Валидирует и нормализует имя вебхука:
/// - обрезает внешние пробельные символы (`trim()`);
/// - проверяет длину в диапазоне [`MIN_WEBHOOK_NAME_CHARS`]..=[`MAX_WEBHOOK_NAME_CHARS`] (`1..=80`) символов Unicode;
/// - проверяет, что имя в нижнем регистре не равно `"clyde"` и не содержит подстроку `"discord"`;
/// - возвращает `Ok(trimmed.to_string())`.
pub fn validate_webhook_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    let char_count = trimmed.chars().count();
    if !(MIN_WEBHOOK_NAME_CHARS..=MAX_WEBHOOK_NAME_CHARS).contains(&char_count) {
        return Err(format!(
            "длина имени вебхука после trim должна быть от {MIN_WEBHOOK_NAME_CHARS} до {MAX_WEBHOOK_NAME_CHARS} символов (получено {char_count})"
        ));
    }

    let lowered = trimmed.to_lowercase();
    if lowered == "clyde" {
        return Err("имя вебхука не может быть 'clyde'".to_string());
    }
    if lowered.contains("discord") {
        return Err("имя вебхука не может содержать подстроку 'discord'".to_string());
    }

    Ok(trimmed.to_string())
}

/// Валидирует `channel_id` и [`CreateWebhookRequest`], затем строит маршрут и JSON-тело
/// для создания вебхука (`POST /api/v9/channels/{channel_id}/webhooks`):
/// - проверяет `channel_id` как положительный числовой Snowflake (`1..=20` ASCII-цифр, не начинается с `'0'`);
/// - валидирует `req.name` через [`validate_webhook_name`];
/// - возвращает `("/api/v9/channels/{channel_id}/webhooks", json!({ "name": clean_name, "avatar": req.avatar }))`.
pub fn build_create_webhook_route_and_body(
    channel_id: &str,
    req: &CreateWebhookRequest,
) -> Result<(String, Value), String> {
    validate_snowflake("channel_id", channel_id)?;
    let clean_name = validate_webhook_name(&req.name)?;
    let route = format!("/api/v9/channels/{channel_id}/webhooks");
    let body = json!({
        "name": clean_name,
        "avatar": req.avatar,
    });
    Ok((route, body))
}

/// Валидирует `news_channel_id` и `target_channel_id` и строит маршрут и JSON-тело
/// для подписки на новостной канал (`POST /api/v9/channels/{news_channel_id}/followers`):
/// - проверяет оба идентификатора как положительные числовые Snowflake (`1..=20` ASCII-цифр, не начинаются с `'0'`);
/// - отклоняет совпадение `news_channel_id == target_channel_id`;
/// - возвращает `("/api/v9/channels/{news_channel_id}/followers", json!({ "webhook_channel_id": target_channel_id }))`.
pub fn build_follow_channel_route_and_body(
    news_channel_id: &str,
    target_channel_id: &str,
) -> Result<(String, Value), String> {
    validate_snowflake("news_channel_id", news_channel_id)?;
    validate_snowflake("target_channel_id", target_channel_id)?;
    if news_channel_id == target_channel_id {
        return Err(
            "news_channel_id и target_channel_id не должны совпадать при подписке на канал"
                .to_string(),
        );
    }
    let route = format!("/api/v9/channels/{news_channel_id}/followers");
    let body = json!({
        "webhook_channel_id": target_channel_id,
    });
    Ok((route, body))
}
