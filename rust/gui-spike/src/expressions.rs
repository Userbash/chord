//! RW-40 / task-242 (`BE-P1-26`): BFF-контракт кастомных эмодзи и стикеров гильдии (`expressions`).
//!
//! Источники контракта:
//! - `/home/sanya/re-deep/rest-static.json:1197-1208` (`Endpoints.GUILD_EMOJIS: "/guilds/{arg}/emojis"`, bodyKeys `{image,name,roles}`);
//! - `/home/sanya/re-deep/rest-static.json:1225-1236` (`Endpoints.GUILD_EMOJI: "/guilds/{arg}/emojis/{arg}"`);
//! - Discord API v9:
//!   - длина имени эмодзи `2..=32` символов (`[A-Za-z0-9_]`), размер файла до `256 KiB` (`262_144` байт);
//!   - длина имени стикера `2..=30` символов, описания — до `100` символов;
//!   - форматы стикеров: `1 = PNG`, `2 = APNG`, `3 = LOTTIE`, `4 = GIF`.

use serde::{Deserialize, Serialize};

/// Минимальная длина имени кастомного эмодзи гильдии (`2` символа).
pub const MIN_EMOJI_NAME_CHARS: usize = 2;

/// Максимальная длина имени кастомного эмодзи гильдии (`32` символа).
pub const MAX_EMOJI_NAME_CHARS: usize = 32;

/// Максимальный размер изображения кастомного эмодзи в байтах (`256 KiB = 262_144`).
pub const MAX_EMOJI_BYTES: u64 = 256 * 1024;

/// Минимальная длина имени кастомного стикера гильдии (`2` символа).
pub const MIN_STICKER_NAME_CHARS: usize = 2;

/// Максимальная длина имени кастомного стикера гильдии (`30` символов).
pub const MAX_STICKER_NAME_CHARS: usize = 30;

/// Максимальная длина описания кастомного стикера гильдии (`100` символов).
pub const MAX_STICKER_DESC_CHARS: usize = 100;

/// Формат стикера `1` — статический `PNG`.
pub const STICKER_FORMAT_PNG: u8 = 1;

/// Формат стикера `2` — анимированный `APNG`.
pub const STICKER_FORMAT_APNG: u8 = 2;

/// Формат стикера `3` — векторная анимация `LOTTIE` (JSON).
pub const STICKER_FORMAT_LOTTIE: u8 = 3;

/// Формат стикера `4` — анимированный `GIF`.
pub const STICKER_FORMAT_GIF: u8 = 4;

/// Разрешённые префиксы Data URI для загрузки изображения кастомного эмодзи.
const ALLOWED_IMAGE_DATA_URI_PREFIXES: [&str; 4] = [
    "data:image/png;base64,",
    "data:image/gif;base64,",
    "data:image/jpeg;base64,",
    "data:image/webp;base64,",
];

/// Запрос на создание кастомного эмодзи гильдии (`POST /api/v9/guilds/{guild_id}/emojis`).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CreateGuildEmojiRequest {
    /// Имя эмодзи (`2..=32` ASCII-буквенно-цифровых символов или `'_'`).
    pub name: String,
    /// Data URI изображения (`data:image/{png,gif,jpeg,webp};base64,<payload>`).
    pub image_data_uri: String,
    /// Список Snowflake ID ролей, которым разрешено использовать эмодзи.
    pub roles: Vec<String>,
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

/// Валидирует имя кастомного эмодзи гильдии:
/// - длина `name.trim()` должна быть в диапазоне [`MIN_EMOJI_NAME_CHARS`]..=[`MAX_EMOJI_NAME_CHARS`] (`2..=32`);
/// - каждый символ в `name.trim()` должен быть ASCII-буквенно-цифровым или `'_'`
///   (`c.is_ascii_alphanumeric() || c == '_'`).
pub fn validate_emoji_name(name: &str) -> Result<(), String> {
    let trimmed = name.trim();
    let char_count = trimmed.chars().count();
    if !(MIN_EMOJI_NAME_CHARS..=MAX_EMOJI_NAME_CHARS).contains(&char_count) {
        return Err(format!(
            "длина имени эмодзи после trim должна быть от {MIN_EMOJI_NAME_CHARS} до {MAX_EMOJI_NAME_CHARS} символов (получено {char_count})"
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(
            "имя эмодзи может содержать только латинские буквы, цифры и символ подчёркивания ('_')"
                .to_string(),
        );
    }
    Ok(())
}

/// Валидирует запрос на создание кастомного эмодзи гильдии [`CreateGuildEmojiRequest`]:
/// - вызывает [`validate_emoji_name`] для `req.name`;
/// - проверяет, что `byte_len` находится в диапазоне `1..=MAX_EMOJI_BYTES` (`1..=262_144`);
/// - проверяет, что `req.image_data_uri` начинается с одного из допустимых префиксов
///   (`"data:image/png;base64,"`, `"data:image/gif;base64,"`, `"data:image/jpeg;base64,"`, `"data:image/webp;base64,"`)
///   и имеет непустой base64-пейлоад после запятой;
/// - проверяет, что каждая роль в `req.roles` является валидным положительным числовым Snowflake
///   (`1..=20` ASCII-цифр, без ведущего `'0'`).
pub fn validate_create_guild_emoji(
    req: &CreateGuildEmojiRequest,
    byte_len: u64,
) -> Result<(), String> {
    validate_emoji_name(&req.name)?;

    if !(1..=MAX_EMOJI_BYTES).contains(&byte_len) {
        return Err(format!(
            "размер файла эмодзи должен быть от 1 до {MAX_EMOJI_BYTES} байт (получено {byte_len})"
        ));
    }

    let payload = ALLOWED_IMAGE_DATA_URI_PREFIXES
        .iter()
        .find_map(|prefix| req.image_data_uri.strip_prefix(prefix))
        .ok_or_else(|| {
            "image_data_uri должен начинаться с data:image/{png,gif,jpeg,webp};base64,".to_string()
        })?;

    if payload.trim().is_empty() {
        return Err(
            "image_data_uri должен содержать непустой base64-пейлоад после запятой".to_string(),
        );
    }

    for role_id in &req.roles {
        validate_snowflake("roles[]", role_id)?;
    }

    Ok(())
}

/// Валидирует `guild_id` (и `emoji_id`, если передан `Some`) как положительные числовые Snowflake
/// (`1..=20` ASCII-цифр, без ведущего `'0'`) и возвращает REST-маршрут:
/// - `"/api/v9/guilds/{guild_id}/emojis"` при `emoji_id == None`;
/// - `"/api/v9/guilds/{guild_id}/emojis/{emoji_id}"` при `emoji_id == Some(id)`.
pub fn build_guild_emojis_route(guild_id: &str, emoji_id: Option<&str>) -> Result<String, String> {
    validate_snowflake("guild_id", guild_id)?;
    match emoji_id {
        Some(eid) => {
            validate_snowflake("emoji_id", eid)?;
            Ok(format!("/api/v9/guilds/{guild_id}/emojis/{eid}"))
        }
        None => Ok(format!("/api/v9/guilds/{guild_id}/emojis")),
    }
}
