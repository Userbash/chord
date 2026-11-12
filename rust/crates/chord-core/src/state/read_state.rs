//! P2-07a: `MESSAGE_ACK` и его разбор (RW-17 / W17a).
//!
//! Контракт заморожен `waves/RW-17/DESIGN-P2-07.md` §2–§3. Здесь только чистое
//! представление подтверждения прочтения: без I/O, reducer-а и сети.
//!
//! `channel_id`/`message_id` обязательны; `flags`, `last_viewed`, `mention_count`,
//! `manual`, `version` — optional. Отсутствие/`nil` optional-поля даёт дефолт без
//! диагностики, неверный тип — дефолт и запись `d.<поле>` в `ignored`.

use chord_codec::etf::EtfValue;

use crate::ready::value;

use super::types::{EventError, StateDiagnostics, raw_display};

/// Подтверждение прочтения `MESSAGE_ACK`.
///
/// `version` — глобальный watermark read-state, а не версия канала. `manual`
/// подтверждён официальным клиентом, но в capture отсутствует; его отсутствие —
/// нормальный дефолт `false`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MessageAck {
    /// Канал (обязателен).
    pub channel_id: u64,
    /// Подтверждённое сообщение (обязательно).
    pub message_id: u64,
    /// Флаги канала.
    pub flags: Option<i64>,
    /// Последний просмотр (дни от Discord epoch).
    pub last_viewed: Option<i64>,
    /// Число упоминаний (имеет смысл при `manual`).
    pub mention_count: Option<i64>,
    /// Ручное подтверждение.
    pub manual: bool,
    /// Глобальная версия read-state.
    pub version: Option<u64>,
}

impl MessageAck {
    /// Разобрать `d` ACK.
    ///
    /// Ошибка обязательных полей (`channel_id`, `message_id`) возвращается как
    /// [`EventError`]; optional-поля не роняют событие — дефолт и диагностика.
    /// `d` не map — [`EventError::WrongType`] с путём `d`.
    pub fn parse(data: &EtfValue) -> Result<(Self, StateDiagnostics), EventError> {
        if data.as_map().is_none() {
            return Err(EventError::WrongType {
                path: "d".to_string(),
                expected: "map",
                found: value::kind(data),
            });
        }

        let channel_id = required_id(data, "channel_id", "message_ack.channel_id")?;
        let message_id = required_id(data, "message_id", "message_ack.message_id")?;

        let mut diagnostics = StateDiagnostics::default();
        let flags = optional_int(data, "flags", "d", &mut diagnostics);
        let last_viewed = optional_int(data, "last_viewed", "d", &mut diagnostics);
        let mention_count = optional_int(data, "mention_count", "d", &mut diagnostics);
        let manual = optional_bool(data, "manual", "d", &mut diagnostics).unwrap_or(false);
        let version = optional_unsigned(data, "version", "d", &mut diagnostics);

        Ok((
            Self {
                channel_id,
                message_id,
                flags,
                last_viewed,
                mention_count,
                manual,
                version,
            },
            diagnostics,
        ))
    }
}

fn required_id(map: &EtfValue, key: &str, path: &str) -> Result<u64, EventError> {
    let raw = map.get(key).ok_or_else(|| EventError::Missing {
        path: path.to_string(),
    })?;
    value::unsigned(raw).ok_or_else(|| EventError::InvalidId {
        path: path.to_string(),
        value: raw_display(raw),
    })
}

fn is_null_or_empty(raw: &EtfValue) -> bool {
    value::is_null(raw)
        || matches!(
            raw,
            EtfValue::Str(text) | EtfValue::Atom(text)
                if text == "nil" || text == "null" || text.trim().is_empty()
        )
}

fn optional_int(
    map: &EtfValue,
    key: &str,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Option<i64> {
    let raw = map.get(key)?;
    if is_null_or_empty(raw) {
        return None;
    }
    match value::integer(raw).or_else(|| value::text(raw).and_then(|text| text.trim().parse().ok()))
    {
        Some(number) => Some(number),
        None => {
            diagnostics.ignored.push(format!("{path}.{key}"));
            None
        }
    }
}

fn optional_unsigned(
    map: &EtfValue,
    key: &str,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Option<u64> {
    let raw = map.get(key)?;
    if is_null_or_empty(raw) {
        return None;
    }
    match value::unsigned(raw) {
        Some(number) => Some(number),
        None => {
            diagnostics.ignored.push(format!("{path}.{key}"));
            None
        }
    }
}

fn optional_bool(
    map: &EtfValue,
    key: &str,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Option<bool> {
    let raw = map.get(key)?;
    if value::is_null(raw) {
        return None;
    }
    match value::boolean(raw) {
        Some(flag) => Some(flag),
        None => {
            diagnostics.ignored.push(format!("{path}.{key}"));
            None
        }
    }
}
