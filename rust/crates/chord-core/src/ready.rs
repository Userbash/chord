//! # READY: сводка диспатча (RW-05, P1-04 / T-GW-04)
//!
//! Разбор op0 `t: "READY"` в компактную сводку (числа для наблюдения вызывающим).
//! Оракул — снимок потока `gw-proto/zstd-frames-95974_1389.hexl` (второй терм — READY,
//! s=1) и `gw-proto/msgs-run2-95974_1389.jsonl` (контрольные числа — в тестах).

use chord_codec::etf::EtfValue;

// P2-01 (RW-12): типизированная модель READY по контракту A1. Модули — рабочие площадки
// W12a/W12b/W12c; общие помощники и диагностика — координатора. `ReadySummary` (RW-05)
// остаётся для обратной совместимости P1-04.
pub mod error;
pub mod guild;
pub mod read_state;
pub mod snapshot;
pub mod social;
pub mod value;

pub use error::{ReadyDiagnostics, ReadyError};
pub use guild::{Channel, Emoji, Guild, PermissionOverwrite, Role, Sticker};
pub use read_state::{
    ChannelOverride, MuteConfig, ReadState, ReadStateEntry, UserGuildSettings,
    UserGuildSettingsEntry,
};
pub use snapshot::{Ready, TOP_LEVEL_KEYS};
pub use social::{ClientInfo, GuildMember, PrivateChannel, Relationship, Session, User};

/// Сводка READY: числа, достаточные для сверки с оракулом без хранения всего `d`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadySummary {
    /// Число ключей верхнего уровня в `d` (в захвате — 40).
    pub top_keys: usize,
    /// Число гильдий (`d.guilds`).
    pub guilds: usize,
    /// Число приватных каналов (`d.private_channels`).
    pub private_channels: usize,
    /// Число пользователей (`d.users`).
    pub users: usize,
    /// Число связей (`d.relationships`).
    pub relationships: usize,
    /// Число записей `d.read_state.entries`.
    pub read_state_entries: usize,
    /// `d.read_state.partial` (атом `"true"`/`"false"` либо int 0/1).
    pub read_state_partial: Option<bool>,
    /// `d.read_state.version`.
    pub read_state_version: Option<i64>,
    /// Число записей `d.user_guild_settings.entries`.
    pub user_guild_settings_entries: usize,
    /// Число ключей `d.user` (нет `user` — `None`).
    pub user_keys: Option<usize>,
    /// `d.user.id` (big-int либо строка — как в артефакте).
    pub user_id: Option<String>,
    /// `d.session_id`.
    pub session_id: Option<String>,
    /// `d.resume_gateway_url`.
    pub resume_gateway_url: Option<String>,
    /// Число сессий (`d.sessions`).
    pub sessions: usize,
    /// Число экспериментов (`d.experiments`).
    pub experiments: usize,
    /// Число записей `d.merged_members`.
    pub merged_members: usize,
}

impl ReadySummary {
    /// Сводка диспатча READY: `op == 0`, `t == "READY"`, `d` — map с `guilds`; иначе `None`.
    pub fn from_dispatch(value: &EtfValue) -> Option<ReadySummary> {
        if value.get("op").and_then(EtfValue::as_int)? != 0 {
            return None;
        }
        match value.get("t")? {
            EtfValue::Atom(name) | EtfValue::Str(name) if name == "READY" => {}
            _ => return None,
        }
        let d = value.get("d")?;
        d.as_map()?;
        // READY обязан содержать массив гильдий; прочие счётчики при отсутствии — 0.
        let guilds = array_len(d.get("guilds")?)?;
        let read_state = d.get("read_state");
        let user = d.get("user");
        Some(Self {
            top_keys: d.as_map().map_or(0, <[_]>::len),
            guilds,
            private_channels: d.get("private_channels").and_then(array_len).unwrap_or(0),
            users: d.get("users").and_then(array_len).unwrap_or(0),
            relationships: d.get("relationships").and_then(array_len).unwrap_or(0),
            read_state_entries: read_state
                .and_then(|state| state.get("entries"))
                .and_then(array_len)
                .unwrap_or(0),
            read_state_partial: read_state
                .and_then(|state| state.get("partial"))
                .and_then(partial_bool),
            read_state_version: read_state
                .and_then(|state| state.get("version"))
                .and_then(EtfValue::as_int),
            user_guild_settings_entries: d
                .get("user_guild_settings")
                .and_then(|settings| settings.get("entries"))
                .and_then(array_len)
                .unwrap_or(0),
            user_keys: user.and_then(EtfValue::as_map).map(<[_]>::len),
            user_id: user
                .and_then(|user| user.get("id"))
                .and_then(|id| match id {
                    EtfValue::Int(number) => Some(number.to_string()),
                    EtfValue::Str(text) => Some(text.clone()),
                    _ => None,
                }),
            session_id: d
                .get("session_id")
                .and_then(EtfValue::as_str)
                .map(str::to_string),
            resume_gateway_url: d
                .get("resume_gateway_url")
                .and_then(EtfValue::as_str)
                .map(str::to_string),
            sessions: d.get("sessions").and_then(array_len).unwrap_or(0),
            experiments: d.get("experiments").and_then(array_len).unwrap_or(0),
            merged_members: d.get("merged_members").and_then(array_len).unwrap_or(0),
        })
    }
}

/// Длина массива (`LIST_EXT` либо `TUPLE_EXT`); не массив — `None`.
fn array_len(value: &EtfValue) -> Option<usize> {
    match value {
        EtfValue::List(items) | EtfValue::Tuple(items) => Some(items.len()),
        EtfValue::Nil => Some(0),
        _ => None,
    }
}

/// `partial` из `read_state`: атом `"true"`/`"false"` либо int 1/0; иное — `None`.
fn partial_bool(value: &EtfValue) -> Option<bool> {
    match value {
        EtfValue::Atom(name) if name == "true" => Some(true),
        EtfValue::Atom(name) if name == "false" => Some(false),
        EtfValue::Int(1) => Some(true),
        EtfValue::Int(0) => Some(false),
        _ => None,
    }
}
