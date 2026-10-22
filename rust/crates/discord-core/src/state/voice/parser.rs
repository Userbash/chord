//! P2-10: разбор voice-событий (RW-19 / W19).
//!
//! Контракт заморожен `waves/RW-19/DESIGN-P2-10.md` §4. Все функции чистые и не
//! паникуют на входных данных: обязательные поля дают [`EventError`], optional —
//! диагностику (`ignored`/`malformed`/`inconsistencies`). `data` — уже декодированный
//! ETF; `session_id` не читается и не сохраняется.

use discord_codec::etf::EtfValue;

use crate::ready::social::parse_user;
use crate::ready::{ReadyDiagnostics, User, value};

use super::types::{VoiceChannelStartTime, VoiceChannelStatus, VoiceState, VoiceStateUpdate};
use crate::state::types::{EventError, StateDiagnostics, raw_display};

/// Разобрать `VOICE_STATE_UPDATE`; фатальны только `guild_id`, `user_id`, `channel_id`
/// и форма `d`.
pub fn parse_voice_state_update(
    data: &EtfValue,
) -> Result<(VoiceStateUpdate, StateDiagnostics), EventError> {
    if data.as_map().is_none() {
        return Err(EventError::WrongType {
            path: "voice".to_string(),
            expected: "map",
            found: value::kind(data),
        });
    }
    let guild_id = required_positive(data, "guild_id", "voice.guild_id")?;
    let user_id = required_positive(data, "user_id", "voice.user_id")?;
    let channel_id = required_channel(data, "channel_id", "voice.channel_id")?;

    let mut diagnostics = StateDiagnostics::default();
    let state = parse_voice_fields(
        data,
        "voice",
        guild_id,
        user_id,
        channel_id,
        &mut diagnostics,
    );
    let member_user = parse_member_user(data, "voice", guild_id, user_id, &mut diagnostics);
    Ok((VoiceStateUpdate { state, member_user }, diagnostics))
}

/// Разобрать `voice_states` из `READY_SUPPLEMENTAL`.
///
/// `guild_id` — id родительского `guilds[i].id`; если он отсутствует, используется
/// `guild_id` элемента. Битые элементы пропускаются с диагностикой; дубликаты
/// `(guild,user)` не диагностируются — reducer вставляет их по порядку (last wins).
pub fn parse_voice_states(
    value: Option<&EtfValue>,
    guild_id: Option<u64>,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Vec<VoiceState> {
    let Some(value) = value else {
        return Vec::new();
    };
    if value::is_null(value) {
        return Vec::new();
    }
    let Some(items) = value::items(value) else {
        diagnostics.malformed.push(path.to_string());
        return Vec::new();
    };

    let parent_guild_id = guild_id.filter(|id| *id > 0);
    let mut states = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let item_path = format!("{path}[{index}]");
        if item.as_map().is_none() {
            diagnostics.malformed.push(item_path);
            continue;
        }
        let Some(user_id) = item
            .get("user_id")
            .and_then(value::unsigned)
            .filter(|id| *id > 0)
        else {
            diagnostics.malformed.push(item_path);
            continue;
        };

        let element_guild_id = item
            .get("guild_id")
            .and_then(value::unsigned)
            .filter(|id| *id > 0);
        let guild_id = match (parent_guild_id, element_guild_id) {
            (Some(parent), Some(element)) => {
                if element != parent {
                    diagnostics
                        .inconsistencies
                        .push(format!("voice:{parent}:{user_id}:guild:{element}"));
                }
                parent
            }
            (Some(parent), None) => parent,
            (None, Some(element)) => element,
            (None, None) => {
                diagnostics
                    .inconsistencies
                    .push(format!("{item_path}:guild"));
                continue;
            }
        };

        let channel_id = match item.get("channel_id") {
            Some(raw) if value::is_null(raw) => {
                diagnostics
                    .inconsistencies
                    .push(format!("{item_path}:channel:nil"));
                continue;
            }
            Some(raw) => match value::unsigned(raw).filter(|id| *id > 0) {
                Some(id) => Some(id),
                None => {
                    diagnostics.malformed.push(item_path.clone());
                    continue;
                }
            },
            None => {
                diagnostics.malformed.push(item_path.clone());
                continue;
            }
        };

        let state =
            parse_voice_fields(item, &item_path, guild_id, user_id, channel_id, diagnostics);
        // У снимка `VoiceState` не хранит member; разбор нужен только для диагностики.
        let _ = parse_member_user(item, &item_path, guild_id, user_id, diagnostics);
        states.push(state);
    }
    states
}

/// Разобрать `VOICE_CHANNEL_START_TIME_UPDATE`.
pub fn parse_voice_channel_start_time(
    data: &EtfValue,
) -> Result<(VoiceChannelStartTime, StateDiagnostics), EventError> {
    if data.as_map().is_none() {
        return Err(EventError::WrongType {
            path: "voice_start_time".to_string(),
            expected: "map",
            found: value::kind(data),
        });
    }
    let guild_id = required_positive(data, "guild_id", "voice_start_time.guild_id")?;
    let channel_id = required_positive(data, "id", "voice_start_time.channel_id")?;
    let voice_start_time = match data.get("voice_start_time") {
        None => None,
        Some(raw) if value::is_null(raw) => None,
        Some(raw) => match integer_or_text(raw) {
            Some(value) if value >= 0 => Some(value),
            _ => {
                return Err(EventError::WrongType {
                    path: "voice_start_time.voice_start_time".to_string(),
                    expected: "int|null",
                    found: value::kind(raw),
                });
            }
        },
    };
    Ok((
        VoiceChannelStartTime {
            guild_id,
            channel_id,
            voice_start_time,
        },
        StateDiagnostics::default(),
    ))
}

/// Разобрать `VOICE_CHANNEL_STATUS_UPDATE`.
pub fn parse_voice_channel_status(
    data: &EtfValue,
) -> Result<(VoiceChannelStatus, StateDiagnostics), EventError> {
    if data.as_map().is_none() {
        return Err(EventError::WrongType {
            path: "voice_status".to_string(),
            expected: "map",
            found: value::kind(data),
        });
    }
    let guild_id = required_positive(data, "guild_id", "voice_status.guild_id")?;
    let channel_id = required_positive(data, "id", "voice_status.channel_id")?;
    let status = match data.get("status") {
        None => None,
        Some(raw) if value::is_null(raw) => None,
        Some(raw) => match value::text(raw) {
            Some("") => None,
            Some(text) => Some(text.to_string()),
            None => {
                return Err(EventError::WrongType {
                    path: "voice_status.status".to_string(),
                    expected: "str|null",
                    found: value::kind(raw),
                });
            }
        },
    };
    Ok((
        VoiceChannelStatus {
            guild_id,
            channel_id,
            status,
        },
        StateDiagnostics::default(),
    ))
}

fn parse_voice_fields(
    map: &EtfValue,
    path: &str,
    guild_id: u64,
    user_id: u64,
    channel_id: Option<u64>,
    diagnostics: &mut StateDiagnostics,
) -> VoiceState {
    VoiceState {
        guild_id,
        user_id,
        channel_id,
        connected_at: optional_i64(
            map,
            "connected_at",
            &format!("{path}.connected_at"),
            diagnostics,
        ),
        mute: optional_bool(map, "mute", false, &format!("{path}.mute"), diagnostics),
        deaf: optional_bool(map, "deaf", false, &format!("{path}.deaf"), diagnostics),
        self_mute: optional_bool(
            map,
            "self_mute",
            false,
            &format!("{path}.self_mute"),
            diagnostics,
        ),
        self_deaf: optional_bool(
            map,
            "self_deaf",
            false,
            &format!("{path}.self_deaf"),
            diagnostics,
        ),
        self_video: optional_bool(
            map,
            "self_video",
            false,
            &format!("{path}.self_video"),
            diagnostics,
        ),
        self_stream: optional_bool(
            map,
            "self_stream",
            false,
            &format!("{path}.self_stream"),
            diagnostics,
        ),
        suppress: optional_bool(
            map,
            "suppress",
            false,
            &format!("{path}.suppress"),
            diagnostics,
        ),
        discoverable: optional_bool(
            map,
            "discoverable",
            true,
            &format!("{path}.discoverable"),
            diagnostics,
        ),
        request_to_speak_timestamp: optional_i64(
            map,
            "request_to_speak_timestamp",
            &format!("{path}.request_to_speak_timestamp"),
            diagnostics,
        ),
    }
}

fn parse_member_user(
    map: &EtfValue,
    path: &str,
    guild_id: u64,
    user_id: u64,
    diagnostics: &mut StateDiagnostics,
) -> Option<User> {
    let raw_member = map.get("member")?;
    if value::is_null(raw_member) {
        return None;
    }
    if raw_member.as_map().is_none() {
        diagnostics.ignored.push(format!("{path}.member"));
        return None;
    }
    let Some(raw_user) = raw_member.get("user") else {
        diagnostics.malformed.push(format!("{path}.member.user"));
        return None;
    };
    if raw_user.as_map().is_none() {
        diagnostics.malformed.push(format!("{path}.member.user"));
        return None;
    }

    let mut ready = ReadyDiagnostics::default();
    let user = parse_user(raw_user, &format!("{path}.member.user"), &mut ready);
    diagnostics.merge(&StateDiagnostics::from_ready(&ready));
    let Some(user) = user else {
        diagnostics.malformed.push(format!("{path}.member.user"));
        return None;
    };
    if user.id != user_id {
        diagnostics.inconsistencies.push(format!(
            "voice:{guild_id}:{user_id}:member-user:{}",
            user.id
        ));
        return None;
    }
    Some(user)
}

fn required_positive(map: &EtfValue, key: &str, path: &str) -> Result<u64, EventError> {
    let raw = map.get(key).ok_or_else(|| EventError::Missing {
        path: path.to_string(),
    })?;
    value::unsigned(raw)
        .filter(|id| *id > 0)
        .ok_or_else(|| EventError::InvalidId {
            path: path.to_string(),
            value: raw_display(raw),
        })
}

fn required_channel(map: &EtfValue, key: &str, path: &str) -> Result<Option<u64>, EventError> {
    let raw = map.get(key).ok_or_else(|| EventError::Missing {
        path: path.to_string(),
    })?;
    if value::is_null(raw) {
        return Ok(None);
    }
    value::unsigned(raw)
        .filter(|id| *id > 0)
        .map(Some)
        .ok_or_else(|| EventError::InvalidId {
            path: path.to_string(),
            value: raw_display(raw),
        })
}

fn optional_i64(
    map: &EtfValue,
    key: &str,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Option<i64> {
    let raw = map.get(key)?;
    if value::is_null(raw) {
        return None;
    }
    if let Some(number) = value::integer(raw) {
        return Some(number);
    }
    if let Some(text) = value::text(raw)
        && let Ok(number) = text.parse::<i64>()
    {
        return Some(number);
    }
    diagnostics.ignored.push(path.to_string());
    None
}

fn optional_bool(
    map: &EtfValue,
    key: &str,
    default: bool,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> bool {
    let Some(raw) = map.get(key) else {
        return default;
    };
    if value::is_null(raw) {
        return default;
    }
    match value::boolean(raw) {
        Some(value) => value,
        None => {
            diagnostics.ignored.push(path.to_string());
            default
        }
    }
}

fn integer_or_text(raw: &EtfValue) -> Option<i64> {
    value::integer(raw).or_else(|| value::text(raw).and_then(|text| text.parse::<i64>().ok()))
}
