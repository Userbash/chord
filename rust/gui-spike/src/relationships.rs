//! RW-40 / task-233 (`BE-P1-23`): BFF-контракт связей и заявок в друзья (`relationships`).
//!
//! Источники контракта:
//! - `/home/sanya/re-deep/rest-static.json:19-33` (`Endpoints.USER_RELATIONSHIPS: "/users/@me/relationships"`);
//! - `/home/sanya/re-deep/rest-static.json:34-44` (`Endpoints.USER_RELATIONSHIP: "/users/@me/relationships/{arg}"`);
//! - `waves/RW-40/DM-FRIENDS-RE.md` (§4.2 `sendRequest`, §4.3 `addRelationship` / `blockUser`, §4.7 `RelationshipType`).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Тип связи `1` — друг (`RelationshipType.FRIEND`).
pub const RELATIONSHIP_FRIEND: u8 = 1;

/// Тип связи `2` — заблокированный пользователь (`RelationshipType.BLOCKED`).
pub const RELATIONSHIP_BLOCKED: u8 = 2;

/// Тип связи `3` — входящая заявка в друзья (`RelationshipType.PENDING_INCOMING`).
pub const RELATIONSHIP_PENDING_INCOMING: u8 = 3;

/// Тип связи `4` — исходящая заявка в друзья (`RelationshipType.PENDING_OUTGOING`).
pub const RELATIONSHIP_PENDING_OUTGOING: u8 = 4;

/// Максимальная длина имени пользователя Discord в символах Unicode (`2..=32`).
pub const MAX_USERNAME_CHARS: usize = 32;

/// Запрос на отправку заявки в друзья по имени пользователя (`POST /users/@me/relationships`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendFriendRequest {
    /// Имя пользователя Discord (после `trim()` — от `2` до [`MAX_USERNAME_CHARS`] символов Unicode).
    pub username: String,
}

/// Валидирует имя пользователя для заявки в друзья:
/// - обрезает внешние пробельные символы (`trim()`);
/// - проверяет длину в символах Unicode в диапазоне `2..=MAX_USERNAME_CHARS` (`2..=32`);
/// - отклоняет наличие символов `"@"`, `":"` или `"#"`;
/// - возвращает `Ok(trimmed)` при успехе или `Err` с описанием причины.
pub fn validate_friend_request_username(username: &str) -> Result<&str, String> {
    let trimmed = username.trim();
    let char_count = trimmed.chars().count();
    if !(2..=MAX_USERNAME_CHARS).contains(&char_count) {
        return Err(format!(
            "длина имени пользователя должна быть от 2 до {MAX_USERNAME_CHARS} символов (получено {char_count})"
        ));
    }
    if trimmed.contains('@') || trimmed.contains(':') || trimmed.contains('#') {
        return Err(
            "имя пользователя не должно содержать запрещённые символы '@', ':' или '#'".to_string(),
        );
    }
    Ok(trimmed)
}

/// Валидирует [`SendFriendRequest`] через [`validate_friend_request_username`] и формирует
/// JSON-тело для `POST /users/@me/relationships` (Pomelo: `"discriminator": null`).
pub fn build_send_friend_request_body(req: &SendFriendRequest) -> Result<Value, String> {
    let trimmed = validate_friend_request_username(&req.username)?;
    Ok(json!({
        "username": trimmed,
        "discriminator": Value::Null
    }))
}

/// Валидирует `target_user_id` как канонический положительный `u64` Snowflake ID
/// (`1..=u64::MAX`, только ASCII-цифры, без ведущих нулей) и возвращает маршрут
/// `"/users/@me/relationships/{target_user_id}"`.
pub fn build_relationship_user_route(target_user_id: &str) -> Result<String, String> {
    if target_user_id.is_empty()
        || target_user_id.starts_with('0')
        || !target_user_id.bytes().all(|b| b.is_ascii_digit())
    {
        return Err("некорректный Snowflake ID в поле target_user_id".to_string());
    }
    match target_user_id.parse::<u64>() {
        Ok(n) if n > 0 => Ok(format!("/users/@me/relationships/{target_user_id}")),
        _ => Err("некорректный Snowflake ID в поле target_user_id".to_string()),
    }
}

/// Формирует JSON-тело для блокировки пользователя (`PUT /users/@me/relationships/{user_id}`).
#[must_use]
pub fn build_block_user_body() -> Value {
    json!({
        "type": RELATIONSHIP_BLOCKED
    })
}
