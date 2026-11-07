//! RW-40 / task-259 (`BE-P1-31`): BFF-контракт голосового саундборда и кастомных звуков гильдии (`soundboard`).
//!
//! Источники контракта:
//! - `/home/sanya/re-deep/rest-static.json:9268-9281` (`Endpoints.GUILD_SOUNDBOARD_SOUNDS: "/guilds/{arg}/soundboard-sounds"`);
//! - `/home/sanya/re-deep/rest-static.json:9348-9358` (`Endpoints.SEND_SOUNDBOARD_SOUND: "/channels/{arg}/send-soundboard-sound"`);
//! - Discord API v9:
//!   - длина имени звука после `trim()` — `2..=32` символов Unicode;
//!   - размер аудиофайла — `1..=512 KiB` (`524_288` байт);
//!   - допустимые форматы аудио Data URI — `ogg`, `mpeg`, `mp3`, `wav` (`base64`);
//!   - громкость звука — конечное число `f64` в диапазоне `0.0..=1.0`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Минимальная длина имени звука саундборда после `trim()` (`2` символа Unicode).
pub const MIN_SOUND_NAME_CHARS: usize = 2;

/// Максимальная длина имени звука саундборда после `trim()` (`32` символа Unicode).
pub const MAX_SOUND_NAME_CHARS: usize = 32;

/// Максимальный размер аудиофайла звука саундборда в байтах (`512 KiB = 524_288`).
pub const MAX_SOUND_BYTES: u64 = 512 * 1024;

/// Минимальная допустимая громкость звука саундборда (`0.0`).
pub const MIN_SOUND_VOLUME: f64 = 0.0;

/// Максимальная допустимая громкость звука саундборда (`1.0`).
pub const MAX_SOUND_VOLUME: f64 = 1.0;

/// Разрешённые префиксы Data URI для загрузки аудиофайла кастомного звука гильдии.
const ALLOWED_SOUND_DATA_URI_PREFIXES: [&str; 4] = [
    "data:audio/ogg;base64,",
    "data:audio/mpeg;base64,",
    "data:audio/mp3;base64,",
    "data:audio/wav;base64,",
];

/// Запрос на воспроизведение звука саундборда в голосовом канале
/// (`POST /api/v9/channels/{channel_id}/send-soundboard-sound`).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SendSoundboardSoundRequest {
    /// Snowflake ID звука саундборда.
    pub sound_id: String,
    /// Опциональный Snowflake ID исходной гильдии (если звук принадлежит другой гильдии).
    pub source_guild_id: Option<String>,
}

/// Запрос на создание кастомного звука саундборда в гильдии
/// (`POST /api/v9/guilds/{guild_id}/soundboard-sounds`).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CreateGuildSoundRequest {
    /// Имя звука (`2..=32` символов Unicode после `trim()`).
    pub name: String,
    /// Data URI аудиофайла (`data:audio/{ogg,mpeg,mp3,wav};base64,<payload>`).
    pub sound_data_uri: String,
    /// Опциональная громкость звука (`0.0..=1.0`).
    pub volume: Option<f64>,
    /// Опциональный Snowflake ID кастомного эмодзи.
    pub emoji_id: Option<String>,
    /// Опциональное Unicode-имя стандартного эмодзи.
    pub emoji_name: Option<String>,
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

/// Валидирует запрос на создание кастомного звука гильдии [`CreateGuildSoundRequest`]:
/// - обрезает внешние пробельные символы `req.name.trim()` и проверяет длину в диапазоне
///   [`MIN_SOUND_NAME_CHARS`]..=[`MAX_SOUND_NAME_CHARS`] (`2..=32` символов Unicode);
/// - проверяет, что `byte_len` находится в диапазоне `1..=MAX_SOUND_BYTES` (`1..=524_288`);
/// - проверяет, что `req.sound_data_uri` начинается с одного из допустимых префиксов
///   (`"data:audio/ogg;base64,"`, `"data:audio/mpeg;base64,"`, `"data:audio/mp3;base64,"`, `"data:audio/wav;base64,"`)
///   и содержит непустой пейлоад после запятой;
/// - если `req.volume` задан (`Some(v)`), проверяет `v.is_finite()` и `(0.0..=1.0).contains(&v)`;
/// - если `req.emoji_id` задан (`Some(ref eid)`), проверяет, что он является валидным положительным
///   числовым Snowflake (`1..=20` ASCII-цифр, не начинается с `'0'`).
pub fn validate_create_guild_sound(
    req: &CreateGuildSoundRequest,
    byte_len: u64,
) -> Result<(), String> {
    let trimmed_name = req.name.trim();
    let char_count = trimmed_name.chars().count();
    if !(MIN_SOUND_NAME_CHARS..=MAX_SOUND_NAME_CHARS).contains(&char_count) {
        return Err(format!(
            "длина имени звука после trim должна быть от {MIN_SOUND_NAME_CHARS} до {MAX_SOUND_NAME_CHARS} символов (получено {char_count})"
        ));
    }

    if !(1..=MAX_SOUND_BYTES).contains(&byte_len) {
        return Err(format!(
            "размер аудиофайла звука должен быть от 1 до {MAX_SOUND_BYTES} байт (получено {byte_len})"
        ));
    }

    let payload = ALLOWED_SOUND_DATA_URI_PREFIXES
        .iter()
        .find_map(|prefix| req.sound_data_uri.strip_prefix(prefix))
        .ok_or_else(|| {
            "sound_data_uri должен начинаться с data:audio/{ogg,mpeg,mp3,wav};base64,".to_string()
        })?;

    if payload.trim().is_empty() {
        return Err(
            "sound_data_uri должен содержать непустой base64-пейлоад после запятой".to_string(),
        );
    }

    if let Some(v) = req.volume {
        if !v.is_finite() || !(MIN_SOUND_VOLUME..=MAX_SOUND_VOLUME).contains(&v) {
            return Err(format!(
                "громкость звука должна быть конечным числом в диапазоне {MIN_SOUND_VOLUME}..={MAX_SOUND_VOLUME} (получено {v})"
            ));
        }
    }

    if let Some(ref eid) = req.emoji_id {
        validate_snowflake("emoji_id", eid)?;
    }

    Ok(())
}

/// Валидирует `channel_id`, `req.sound_id` и `req.source_guild_id` (если `Some`) как положительные
/// числовые Snowflake (`1..=20` ASCII-цифр, не начинаются с `'0'`) и строит маршрут и JSON-тело
/// для отправки звука саундборда в голосовой канал:
/// `("/api/v9/channels/{channel_id}/send-soundboard-sound", json!({ "sound_id": req.sound_id, "source_guild_id": req.source_guild_id }))`.
pub fn build_send_soundboard_route_and_body(
    channel_id: &str,
    req: &SendSoundboardSoundRequest,
) -> Result<(String, Value), String> {
    validate_snowflake("channel_id", channel_id)?;
    validate_snowflake("sound_id", &req.sound_id)?;
    if let Some(ref gid) = req.source_guild_id {
        validate_snowflake("source_guild_id", gid)?;
    }

    let route = format!("/api/v9/channels/{channel_id}/send-soundboard-sound");
    let body = json!({
        "sound_id": req.sound_id,
        "source_guild_id": req.source_guild_id,
    });
    Ok((route, body))
}
