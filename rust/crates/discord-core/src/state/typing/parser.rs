//! P2-11a: разбор `TYPING_START` (RW-20 / W20).
//!
//! Контракт заморожен `waves/RW-20/DESIGN-P2-11.md` §4. Путь диагностики — `typing.*`.
//! `received_at_ms` не читается из провода: его передаёт вызывающий. Nil/не-map `d` —
//! фатально; битые optional-поля — `ignored`/`malformed`, обязательные id — `Missing`/
//! `InvalidId`. Паник нет.

use discord_codec::etf::EtfValue;

use crate::ready::social::parse_user;
use crate::ready::value;
use crate::ready::{ReadyDiagnostics, User};
use crate::state::types::{EventError, StateDiagnostics, raw_display};

use super::types::TypingStart;

/// Разобрать `d` события `TYPING_START`.
pub fn parse_typing_start(
    data: &EtfValue,
    received_at_ms: Option<u64>,
) -> Result<(TypingStart, StateDiagnostics), EventError> {
    let map = data.as_map().ok_or_else(|| EventError::WrongType {
        path: "typing".to_string(),
        expected: "map",
        found: value::kind(data),
    })?;

    let channel_id = required_id(map, "channel_id", "typing.channel_id")?;
    let user_id = required_id(map, "user_id", "typing.user_id")?;

    let mut diagnostics = StateDiagnostics::default();
    let guild_id = optional_id(map, "guild_id", "typing.guild_id", &mut diagnostics);
    let wire_timestamp = optional_timestamp(map, "timestamp", "typing.timestamp", &mut diagnostics);
    let member_user = parse_member(map, channel_id, user_id, &mut diagnostics);
    if map_get(map, "typing_indicator_style").is_some() {
        // Поле известно проводу, но в модели P2-11 не сохраняется.
        diagnostics
            .ignored
            .push("typing.typing_indicator_style".to_string());
    }

    Ok((
        TypingStart {
            channel_id,
            user_id,
            guild_id,
            wire_timestamp,
            member_user,
            received_at_ms,
        },
        diagnostics,
    ))
}

fn required_id(map: &[(EtfValue, EtfValue)], key: &str, path: &str) -> Result<u64, EventError> {
    let raw = map_get(map, key).ok_or_else(|| EventError::Missing {
        path: path.to_string(),
    })?;
    match value::unsigned(raw) {
        Some(id) if id > 0 => Ok(id),
        _ => Err(EventError::InvalidId {
            path: path.to_string(),
            value: raw_display(raw),
        }),
    }
}

fn optional_id(
    map: &[(EtfValue, EtfValue)],
    key: &str,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Option<u64> {
    let raw = map_get(map, key)?;
    if value::is_null(raw) {
        return None;
    }
    match value::unsigned(raw) {
        Some(id) if id > 0 => Some(id),
        _ => {
            diagnostics.ignored.push(path.to_string());
            None
        }
    }
}

fn optional_timestamp(
    map: &[(EtfValue, EtfValue)],
    key: &str,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Option<i64> {
    let raw = map_get(map, key)?;
    if value::is_null(raw) {
        return None;
    }
    if let Some(timestamp) = value::integer(raw) {
        return Some(timestamp);
    }
    if let Some(timestamp) = value::text(raw).and_then(|text| text.parse::<i64>().ok()) {
        return Some(timestamp);
    }
    diagnostics.ignored.push(path.to_string());
    None
}

fn parse_member(
    map: &[(EtfValue, EtfValue)],
    channel_id: u64,
    user_id: u64,
    diagnostics: &mut StateDiagnostics,
) -> Option<User> {
    let raw = map_get(map, "member")?;
    if value::is_null(raw) {
        return None;
    }
    let Some(member) = raw.as_map() else {
        diagnostics.ignored.push("typing.member".to_string());
        return None;
    };
    let Some(user_raw) = map_get(member, "user") else {
        diagnostics.malformed.push("typing.member.user".to_string());
        return None;
    };
    if value::is_null(user_raw) || user_raw.as_map().is_none() {
        diagnostics.malformed.push("typing.member.user".to_string());
        return None;
    }

    let mut ready = ReadyDiagnostics::default();
    let Some(user) = parse_user(user_raw, "typing.member.user", &mut ready) else {
        diagnostics.malformed.push("typing.member.user".to_string());
        return None;
    };
    diagnostics.merge(&StateDiagnostics::from_ready(&ready));

    if user.id != user_id {
        // Не отравляем `State.users` записью, не совпадающей с `user_id` события.
        diagnostics.inconsistencies.push(format!(
            "typing:{channel_id}:{user_id}:member-user:{}",
            user.id
        ));
        return None;
    }
    Some(user)
}

fn map_get<'a>(map: &'a [(EtfValue, EtfValue)], key: &str) -> Option<&'a EtfValue> {
    map.iter()
        .find(|(entry_key, _)| {
            entry_key.as_atom() == Some(key) || value::text(entry_key) == Some(key)
        })
        .map(|(_, value)| value)
}
