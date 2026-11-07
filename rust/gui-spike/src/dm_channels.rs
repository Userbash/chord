//! RW-40 / task-223 (`BE-P1-20`): BFF-контракт создания DM / Group DM каналов
//! (`POST /users/@me/channels`) и управления участниками Group DM
//! (`PUT/DELETE /channels/{channel_id}/recipients/{user_id}`).
//!
//! Источники контракта:
//! - `/home/sanya/re-deep/rest-static.json:255-261` (`Endpoints.USER_CHANNELS: "/users/@me/channels"`);
//! - `/home/sanya/re-deep/rest-static.json:2594-2600` (`Endpoints.CHANNEL_RECIPIENT: "/channels/{arg}/recipients/{arg}"`);
//! - `rust/gui-spike/src/dm_friends.rs` (`MAX_DM_RECIPIENTS = 9`).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Максимальное количество получателей при создании Group DM (`1..=9`, до 10 участников включая автора).
pub const MAX_GROUP_DM_RECIPIENTS: usize = 9;

/// Максимальная длина названия Group DM канала в символах Unicode (`<= 100`).
pub const MAX_GROUP_DM_NAME_CHARS: usize = 100;

/// Запрос на создание/открытие одиночного DM (`1` получатель) или Group DM (`2..=9` получателей).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateDmRequest {
    /// Список Snowflake ID получателей (`1..=MAX_GROUP_DM_RECIPIENTS`, без дубликатов).
    pub recipient_ids: Vec<String>,
}

/// Проверяет, что строка является каноническим положительным `u64` Snowflake ID
/// (`1..=u64::MAX`, только ASCII-цифры, без ведущих нулей).
fn validate_snowflake(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty() || value.starts_with('0') || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("некорректный Snowflake ID в поле {field}"));
    }
    match value.parse::<u64>() {
        Ok(n) if n > 0 => Ok(()),
        _ => Err(format!("некорректный Snowflake ID в поле {field}")),
    }
}

/// Валидирует запрос создания DM / Group DM [`CreateDmRequest`]:
/// - отклоняет пустой `recipient_ids` и `recipient_ids.len() > MAX_GROUP_DM_RECIPIENTS`;
/// - проверяет, что каждый `recipient_id` — каноническая положительная `u64` Snowflake строка
///   (`1..=u64::MAX`, без ведущих нулей);
/// - проверяет отсутствие дубликатов в `recipient_ids`.
pub fn validate_create_dm_request(req: &CreateDmRequest) -> Result<(), String> {
    if req.recipient_ids.is_empty() {
        return Err("список recipient_ids не может быть пустым".to_string());
    }
    if req.recipient_ids.len() > MAX_GROUP_DM_RECIPIENTS {
        return Err(format!(
            "количество получателей ({}) превышает максимум {MAX_GROUP_DM_RECIPIENTS}",
            req.recipient_ids.len()
        ));
    }

    for (idx, recipient_id) in req.recipient_ids.iter().enumerate() {
        validate_snowflake("recipient_ids", recipient_id)?;
        if req.recipient_ids[..idx].contains(recipient_id) {
            return Err(format!(
                "обнаружен дублирующийся recipient_id: {recipient_id}"
            ));
        }
    }

    Ok(())
}

/// Валидирует [`CreateDmRequest`] и строит JSON-тело для `POST /users/@me/channels`:
/// - если `req.recipient_ids.len() == 1` — возвращает `Ok(json!({"recipient_id": req.recipient_ids[0]}))`;
/// - иначе (`2..=9`) — возвращает `Ok(json!({"recipients": req.recipient_ids}))`.
pub fn build_create_dm_body(req: &CreateDmRequest) -> Result<Value, String> {
    validate_create_dm_request(req)?;
    if req.recipient_ids.len() == 1 {
        Ok(json!({ "recipient_id": req.recipient_ids[0] }))
    } else {
        Ok(json!({ "recipients": req.recipient_ids }))
    }
}

/// Валидирует `channel_id` и `user_id` как канонические положительные `u64` Snowflake ID
/// и возвращает маршрут управления участником Group DM:
/// `"/channels/{channel_id}/recipients/{user_id}"`.
pub fn build_group_dm_recipient_route(channel_id: &str, user_id: &str) -> Result<String, String> {
    validate_snowflake("channel_id", channel_id)?;
    validate_snowflake("user_id", user_id)?;
    Ok(format!("/channels/{channel_id}/recipients/{user_id}"))
}
