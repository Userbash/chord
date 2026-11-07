//! RW-40 / task-237 (`BE-P1-24`): BFF-контракт журнала аудита, кика, бана и таймаута
//! модерации гильдии (`PUT /guilds/{guild_id}/bans/{user_id}`, `GET /guilds/{guild_id}/audit-logs`).
//!
//! Источники контракта:
//! - `/home/sanya/re-deep/rest-static.json:871-877` (`Endpoints.GUILD_BAN: "/guilds/{arg}/bans/{arg}"`);
//! - `/home/sanya/re-deep/rest-static.json:1240-1246` (`Endpoints.GUILD_AUDIT_LOG: "/guilds/{arg}/audit-logs"`);
//! - Discord API v9: лимит длины причины `X-Audit-Log-Reason` (`512` символов Unicode),
//!   удаление сообщений при бане `delete_message_seconds` (`0..=604_800`, 7 дней),
//!   максимальная длительность таймаута (`2_419_200` секунд, 28 дней),
//!   лимит записей журнала аудита `limit` (`1..=100`).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Максимальная длина причины в заголовке `X-Audit-Log-Reason` в символах Unicode (`512`).
pub const MAX_AUDIT_REASON_CHARS: usize = 512;

/// Максимальное окно удаления сообщений при бане участника в секундах (`604_800` = 7 суток).
pub const MAX_BAN_DELETE_MESSAGE_SECONDS: u32 = 604_800;

/// Максимальная длительность таймаута (`communication_disabled_until`) в секундах (`2_419_200` = 28 суток).
pub const MAX_TIMEOUT_SECONDS: u64 = 2_419_200;

/// Максимальное число записей за один запрос `GET /guilds/{guild_id}/audit-logs` (`1..=100`).
pub const MAX_AUDIT_LOG_LIMIT: u8 = 100;

/// Запрос на бан участника гильдии (`PUT /guilds/{guild_id}/bans/{user_id}`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BanMemberRequest {
    /// Snowflake ID гильдии (`1..=u64::MAX`, без ведущих нулей).
    pub guild_id: String,
    /// Snowflake ID банимого пользователя (`1..=u64::MAX`, без ведущих нулей).
    pub user_id: String,
    /// Окно удаления сообщений в секундах (`0..=MAX_BAN_DELETE_MESSAGE_SECONDS`).
    pub delete_message_seconds: u32,
    /// Опциональная причина для журнала аудита (`X-Audit-Log-Reason`, до `512` символов).
    pub reason: Option<String>,
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

/// Валидирует и нормализует причину для журнала аудита (`X-Audit-Log-Reason`):
/// - если `None`, возвращает `Ok(None)`;
/// - если `Some(r)`: выполняет `r.trim()`; если строка пуста, возвращает `Ok(None)`;
/// - если `r.chars().count() > MAX_AUDIT_REASON_CHARS`, возвращает `Err`;
/// - иначе возвращает `Ok(Some(trimmed.to_string()))`.
pub fn validate_audit_reason(reason: Option<&str>) -> Result<Option<String>, String> {
    let Some(r) = reason else {
        return Ok(None);
    };
    let trimmed = r.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let char_count = r.chars().count();
    if char_count > MAX_AUDIT_REASON_CHARS {
        return Err(format!(
            "длина причины аудита ({char_count}) превышает максимум {MAX_AUDIT_REASON_CHARS} символов"
        ));
    }
    Ok(Some(trimmed.to_string()))
}

/// Валидирует запрос бана участника [`BanMemberRequest`] и строит REST-маршрут, JSON-тело и
/// нормализованную причину аудита:
/// - проверяет `guild_id` и `user_id` как канонические положительные `u64` Snowflake ID (`1..=u64::MAX`, без ведущих нулей);
/// - проверяет `req.delete_message_seconds <= MAX_BAN_DELETE_MESSAGE_SECONDS`;
/// - валидирует `req.reason` через [`validate_audit_reason`];
/// - возвращает `Ok((format!("/guilds/{}/bans/{}", req.guild_id, req.user_id), json!({"delete_message_seconds": req.delete_message_seconds}), clean_reason))`.
pub fn build_ban_member_route_and_body(
    req: &BanMemberRequest,
) -> Result<(String, Value, Option<String>), String> {
    validate_snowflake("guild_id", &req.guild_id)?;
    validate_snowflake("user_id", &req.user_id)?;

    if req.delete_message_seconds > MAX_BAN_DELETE_MESSAGE_SECONDS {
        return Err(format!(
            "delete_message_seconds ({}) превышает максимум {MAX_BAN_DELETE_MESSAGE_SECONDS}",
            req.delete_message_seconds
        ));
    }

    let clean_reason = validate_audit_reason(req.reason.as_deref())?;
    let route = format!("/guilds/{}/bans/{}", req.guild_id, req.user_id);
    let body = json!({ "delete_message_seconds": req.delete_message_seconds });
    Ok((route, body, clean_reason))
}

/// Валидирует параметры запроса журнала аудита гильдии и формирует REST-маршрут:
/// - проверяет `guild_id` как канонический положительный `u64` Snowflake ID (`1..=u64::MAX`, без ведущих нулей);
/// - проверяет `1 <= limit && limit <= MAX_AUDIT_LOG_LIMIT`;
/// - возвращает `"/guilds/{guild_id}/audit-logs?limit={limit}"` (и добавляет `"&action_type={at}"`, если `action_type` равен `Some(at)`).
pub fn build_audit_log_query_route(
    guild_id: &str,
    action_type: Option<u16>,
    limit: u8,
) -> Result<String, String> {
    validate_snowflake("guild_id", guild_id)?;
    if !(1..=MAX_AUDIT_LOG_LIMIT).contains(&limit) {
        return Err(format!(
            "limit ({limit}) должен быть в диапазоне 1..={MAX_AUDIT_LOG_LIMIT}"
        ));
    }
    match action_type {
        Some(at) => Ok(format!(
            "/guilds/{guild_id}/audit-logs?limit={limit}&action_type={at}"
        )),
        None => Ok(format!("/guilds/{guild_id}/audit-logs?limit={limit}")),
    }
}
