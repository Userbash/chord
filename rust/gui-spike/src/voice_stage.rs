//! RW-40 / task-205: BFF-модуль голосовой сцены и запуска демонстрации экрана (`voice_stage`).
//!
//! Источники контракта:
//! - `waves/RW-40/VOICE/STREAM-GATEWAY-CONTRACT.md` (§0.1 `A-OPCODES`, `A-SENDERS`, `A-KEY-PARSE`, `A-KEY-BUILD`, `D-27-2`);
//! - `waves/RW-40/VOICE/stream-opcodes-fixtures.json`;
//! - `waves/RW-40/DESIGN/VOICE-STREAM-UI-SPEC.md`;
//! - `rust/gui-spike/ui/features/voice-grid.js` (`MAX_VISIBLE_VOICE_TILES = 25`);
//! - `rust/gui-spike/ui/features/voice-stage-ui.js`.

use core::fmt;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Value};

/// Максимум одновременно видимых плиток в голосовой сцене (сетка до `5x5`).
pub const MAX_STAGE_VISIBLE_TILES: usize = 25;

/// Разрешение видеопотока демонстрации экрана (`720p`, `1080p`, `1440p`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StreamResolution {
    /// `1280x720` (`720p`).
    #[serde(rename = "720p", alias = "P720", alias = "p720")]
    P720,
    /// `1920x1080` (`1080p`).
    #[serde(rename = "1080p", alias = "P1080", alias = "p1080")]
    P1080,
    /// `2560x1440` (`1440p`).
    #[serde(rename = "1440p", alias = "P1440", alias = "p1440")]
    P1440,
}

impl StreamResolution {
    /// Горизонтальное разрешение кадра в пикселях.
    #[must_use]
    pub const fn width(self) -> u32 {
        match self {
            Self::P720 => 1280,
            Self::P1080 => 1920,
            Self::P1440 => 2560,
        }
    }

    /// Вертикальное разрешение кадра в пикселях.
    #[must_use]
    pub const fn height(self) -> u32 {
        match self {
            Self::P720 => 720,
            Self::P1080 => 1080,
            Self::P1440 => 1440,
        }
    }

    /// Пара `(width, height)` в пикселях.
    #[must_use]
    pub const fn dimensions(self) -> (u32, u32) {
        (self.width(), self.height())
    }

    /// Строковая метка разрешения (`"720p"`, `"1080p"`, `"1440p"`).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::P720 => "720p",
            Self::P1080 => "1080p",
            Self::P1440 => "1440p",
        }
    }
}

/// Частота кадров демонстрации экрана (`30` или `60` FPS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum StreamFps {
    /// `30` кадров в секунду.
    #[serde(rename = "30")]
    Fps30,
    /// `60` кадров в секунду.
    #[serde(rename = "60")]
    Fps60,
}

impl<'de> Deserialize<'de> for StreamFps {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct FpsVisitor;

        impl<'de> Visitor<'de> for FpsVisitor {
            type Value = StreamFps;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("целое число 30 или 60, либо строка '30', '60', 'Fps30', 'Fps60'")
            }

            fn visit_i64<E>(self, value: i64) -> Result<StreamFps, E>
            where
                E: de::Error,
            {
                match value {
                    30 => Ok(StreamFps::Fps30),
                    60 => Ok(StreamFps::Fps60),
                    other => Err(E::custom(format!("неподдерживаемое значение FPS: {other}"))),
                }
            }

            fn visit_u64<E>(self, value: u64) -> Result<StreamFps, E>
            where
                E: de::Error,
            {
                match value {
                    30 => Ok(StreamFps::Fps30),
                    60 => Ok(StreamFps::Fps60),
                    other => Err(E::custom(format!("неподдерживаемое значение FPS: {other}"))),
                }
            }

            fn visit_str<E>(self, value: &str) -> Result<StreamFps, E>
            where
                E: de::Error,
            {
                match value {
                    "30" | "Fps30" | "fps30" => Ok(StreamFps::Fps30),
                    "60" | "Fps60" | "fps60" => Ok(StreamFps::Fps60),
                    other => Err(E::custom(format!("неподдерживаемая строка FPS: {other}"))),
                }
            }
        }

        deserializer.deserialize_any(FpsVisitor)
    }
}

impl StreamFps {
    /// Числовое значение частоты кадров (`30` или `60`).
    #[must_use]
    pub const fn fps(self) -> u32 {
        match self {
            Self::Fps30 => 30,
            Self::Fps60 => 60,
        }
    }

    /// Синоним [`Self::fps`].
    #[must_use]
    pub const fn value(self) -> u32 {
        self.fps()
    }
}

/// Спецификация запуска демонстрации экрана (Go Live / Screen Share).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenShareSpec {
    /// Snowflake ID сервера (`Some` для голосового канала сервера, `None` для звонка/ЛС).
    #[serde(default, alias = "guildId")]
    pub guild_id: Option<String>,
    /// Snowflake ID голосового канала.
    #[serde(alias = "channelId")]
    pub channel_id: String,
    /// Идентификатор источника захвата экрана или окна ОС (например, `"screen:0:0"`).
    #[serde(alias = "sourceId")]
    pub source_id: String,
    /// Выбранное разрешение видеопотока.
    pub resolution: StreamResolution,
    /// Выбранная частота кадров.
    pub fps: StreamFps,
    /// Флаг трансляции системного звука.
    #[serde(default, alias = "shareAudio")]
    pub share_audio: bool,
}

/// Проверяет, что строка является валидным положительным Snowflake ID
/// (непустая, только ASCII-цифры, без ведущих нулей, парсится в `u64 > 0`).
fn validate_snowflake(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty() || value.starts_with('0') || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("некорректный Snowflake ID в поле {field}"));
    }
    match value.parse::<u64>() {
        Ok(n) if n > 0 => Ok(()),
        _ => Err(format!("некорректный Snowflake ID в поле {field}")),
    }
}

impl ScreenShareSpec {
    /// Проверяет корректность спецификации запуска демонстрации экрана:
    /// - `channel_id` (и `guild_id`, если задан) — ненулевые числовые Snowflake ID (`1..=u64::MAX`);
    /// - `source_id` не пустой после обрезки пробелов.
    pub fn validate(&self) -> Result<(), String> {
        if let Some(gid) = self.guild_id.as_deref() {
            validate_snowflake("guild_id", gid)?;
        }
        validate_snowflake("channel_id", &self.channel_id)?;
        if self.source_id.trim().is_empty() {
            return Err("source_id не должен быть пустым".to_string());
        }
        Ok(())
    }

    /// Строит канонический ключ стрима (`guild:{gid}:{cid}:{uid}` или `call:{cid}:{uid}`)
    /// по эталонному контракту `A-KEY-BUILD` (`@5089954`).
    #[must_use]
    pub fn to_stream_key(&self, user_id: &str) -> String {
        match self.guild_id.as_deref() {
            Some(gid) => format!("guild:{gid}:{}:{user_id}", self.channel_id),
            None => format!("call:{}:{user_id}", self.channel_id),
        }
    }

    /// Строит канонический ключ стрима (`guild:{gid}:{cid}:{uid}` или `call:{cid}:{uid}`).
    /// Имя метода сохранено в точном соответствии с ТЗ (`to_stream_Key`).
    #[must_use]
    #[allow(non_snake_case)]
    pub fn to_stream_Key(&self, user_id: &str) -> String {
        self.to_stream_key(user_id)
    }
}

/// Строит полный JSON-кадр шлюза для `op 18 STREAM_CREATE` (`A-OPCODES`, `A-SENDERS`, `D-27-2`).
///
/// Формат:
/// - для гильдии: `{"op": 18, "d": {"type": "guild", "guild_id": "<gid>", "channel_id": "<cid>", "preferred_region": null}}`;
/// - для звонка: `{"op": 18, "d": {"type": "call", "guild_id": null, "channel_id": "<cid>", "preferred_region": null}}`.
pub fn build_gateway_stream_create_op18(spec: &ScreenShareSpec) -> Result<Value, String> {
    spec.validate()?;
    let (stream_type, guild_val) = match spec.guild_id.as_deref() {
        Some(gid) => ("guild", Value::String(gid.to_string())),
        None => ("call", Value::Null),
    };
    Ok(json!({
        "op": 18,
        "d": {
            "type": stream_type,
            "guild_id": guild_val,
            "channel_id": spec.channel_id,
            "preferred_region": null
        }
    }))
}

/// Валидирует каноническую строку `stream_key` (`guild:{gid}:{cid}:{uid}` или `call:{cid}:{uid}`)
/// по правилам `A-KEY-PARSE` (`@5089698`).
fn validate_stream_key(stream_key: &str) -> Result<(), String> {
    if stream_key.is_empty() {
        return Err("stream_key не должен быть пустым".to_string());
    }
    let segments: Vec<&str> = stream_key.split(':').collect();
    match segments.first().copied() {
        Some("guild") => {
            if segments.len() != 4 {
                return Err("stream_key типа guild должен содержать 4 сегмента".to_string());
            }
            validate_snowflake("guild_id", segments[1])?;
            validate_snowflake("channel_id", segments[2])?;
            validate_snowflake("user_id", segments[3])?;
            Ok(())
        }
        Some("call") => {
            if segments.len() != 3 {
                return Err("stream_key типа call должен содержать 3 сегмента".to_string());
            }
            validate_snowflake("channel_id", segments[1])?;
            validate_snowflake("user_id", segments[2])?;
            Ok(())
        }
        _ => Err("неизвестный тип stream_key (ожидается guild или call)".to_string()),
    }
}

/// Строит полный JSON-кадр шлюза для `op 19 STREAM_DELETE` (`A-OPCODES`, `A-SENDERS`, `A-STOP-HELPER`).
///
/// Формат: `{"op": 19, "d": {"stream_key": "<stream_key>"}}`.
pub fn build_gateway_stream_delete_op19(stream_key: &str) -> Result<Value, String> {
    validate_stream_key(stream_key)?;
    Ok(json!({
        "op": 19,
        "d": {
            "stream_key": stream_key
        }
    }))
}

/// Строит полный JSON-кадр шлюза для `op 20 STREAM_WATCH` (`A-OPCODES`, `A-SENDERS`).
///
/// Формат: `{"op": 20, "d": {"stream_key": "<stream_key>"}}`.
pub fn build_gateway_stream_watch_op20(stream_key: &str) -> Result<Value, String> {
    validate_stream_key(stream_key)?;
    Ok(json!({
        "op": 20,
        "d": {
            "stream_key": stream_key
        }
    }))
}

/// Строит полный JSON-кадр шлюза для `op 21 STREAM_PING` (`A-OPCODES`, `A-SENDERS`, `A-PING-CALLSITE`).
///
/// Формат: `{"op": 21, "d": {"stream_key": "<stream_key>"}}`.
pub fn build_gateway_stream_ping_op21(stream_key: &str) -> Result<Value, String> {
    validate_stream_key(stream_key)?;
    Ok(json!({
        "op": 21,
        "d": {
            "stream_key": stream_key
        }
    }))
}

/// Строит полный JSON-кадр шлюза для `op 22 STREAM_SET_PAUSED` (`A-OPCODES`, `A-SENDERS`, `A-PAUSE-UI`).
///
/// Формат: `{"op": 22, "d": {"stream_key": "<stream_key>", "paused": <paused>}}`.
pub fn build_gateway_stream_set_paused_op22(
    stream_key: &str,
    paused: bool,
) -> Result<Value, String> {
    validate_stream_key(stream_key)?;
    Ok(json!({
        "op": 22,
        "d": {
            "stream_key": stream_key,
            "paused": paused
        }
    }))
}
