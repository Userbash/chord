//! READY: корневой тип и разбор конверта (RW-12 / P2-01a).
//!
//! Контракт заморожен `waves/RW-12/DESIGN-P2-01.md` §2–§3. `Ready::parse` заполняет W12a;
//! `Debug` не печатает `session_id`, `user_settings_proto` и `raw` (секреты).

use std::fmt;

use chord_codec::etf::EtfValue;

use super::error::ReadyError;
use super::guild::Guild;
use super::read_state::{ReadState, UserGuildSettings};
use super::social::{PrivateChannel, Relationship, Session, User};
use super::{ReadyDiagnostics, guild, read_state, social, value};

/// 40 top-level ключей READY из захвата (`guilds`/`users`/…; `_trace` входит).
pub const TOP_LEVEL_KEYS: [&str; 40] = [
    "_trace",
    "ad_personalization_toggles_disabled",
    "analytics_token",
    "apex_experiments",
    "api_code_version",
    "auth",
    "auth_session_id_hash",
    "broadcaster_user_ids",
    "connected_accounts",
    "consents",
    "country_code",
    "experiments",
    "explicit_content_scan_version",
    "friend_suggestion_count",
    "game_relationships",
    "geo_ordered_rtc_regions",
    "guild_experiments",
    "guild_join_requests",
    "guilds",
    "linked_users",
    "merged_members",
    "notification_settings",
    "pending_payments",
    "private_channels",
    "qos_token",
    "read_state",
    "regional_feature_config",
    "relationships",
    "resume_gateway_url",
    "session_id",
    "session_type",
    "sessions",
    "static_client_session_id",
    "subdivision_code",
    "tutorial",
    "user",
    "user_guild_settings",
    "user_settings_proto",
    "users",
    "v",
];

/// Типизированный READY.
#[derive(Clone, PartialEq)]
pub struct Ready {
    /// `v`.
    pub v: Option<i64>,
    /// Собственный пользователь (fatal).
    pub user: User,
    /// `session_id` (fatal, redacted в Debug).
    pub session_id: String,
    /// `session_type`.
    pub session_type: Option<String>,
    /// `resume_gateway_url`.
    pub resume_gateway_url: Option<String>,
    /// Гильдии.
    pub guilds: Vec<Guild>,
    /// Приватные каналы.
    pub private_channels: Vec<PrivateChannel>,
    /// Пользователи.
    pub users: Vec<User>,
    /// Связи.
    pub relationships: Vec<Relationship>,
    /// Read-state.
    pub read_state: ReadState,
    /// Настройки гильдий.
    pub user_guild_settings: UserGuildSettings,
    /// Сессии.
    pub sessions: Vec<Session>,
    /// RTC-регионы.
    pub geo_ordered_rtc_regions: Vec<String>,
    /// Страна.
    pub country_code: Option<String>,
    /// Субдивизион.
    pub subdivision_code: Option<String>,
    /// Друзья-предложения.
    pub friend_suggestion_count: Option<i64>,
    /// API code version.
    pub api_code_version: Option<i64>,
    /// Explicit content scan version.
    pub explicit_content_scan_version: Option<i64>,
    /// Ad personalization toggles disabled.
    pub ad_personalization_toggles_disabled: bool,
    /// Base64 protobuf (opaque; декод — P2-08).
    pub user_settings_proto: Option<String>,
    /// Число top-level ключей (в capture — 40).
    pub top_level_keys: usize,
    /// Нефатальные расхождения.
    pub diagnostics: ReadyDiagnostics,
    raw: EtfValue,
}

impl fmt::Debug for Ready {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Ready")
            .field("v", &self.v)
            .field("user", &self.user)
            .field("session_id", &"<redacted>")
            .field("session_type", &self.session_type)
            .field("resume_gateway_url", &self.resume_gateway_url)
            .field("guilds", &self.guilds)
            .field("private_channels", &self.private_channels)
            .field("users", &self.users)
            .field("relationships", &self.relationships)
            .field("read_state", &self.read_state)
            .field("user_guild_settings", &self.user_guild_settings)
            .field("sessions", &self.sessions)
            .field("geo_ordered_rtc_regions", &self.geo_ordered_rtc_regions)
            .field("country_code", &self.country_code)
            .field("subdivision_code", &self.subdivision_code)
            .field("friend_suggestion_count", &self.friend_suggestion_count)
            .field("api_code_version", &self.api_code_version)
            .field(
                "explicit_content_scan_version",
                &self.explicit_content_scan_version,
            )
            .field(
                "ad_personalization_toggles_disabled",
                &self.ad_personalization_toggles_disabled,
            )
            .field("user_settings_proto", &"<redacted>")
            .field("top_level_keys", &self.top_level_keys)
            .field("diagnostics", &self.diagnostics)
            .finish_non_exhaustive()
    }
}

impl Ready {
    /// Полный op0-кадр READY: `op == 0`, `t == "READY"`, `d` — map.
    pub fn from_dispatch(value: &EtfValue) -> Result<Self, ReadyError> {
        if value.get("op").and_then(EtfValue::as_int) != Some(0) {
            return Err(ReadyError::NotReady);
        }
        let is_ready = matches!(
            value.get("t"),
            Some(EtfValue::Atom(name) | EtfValue::Str(name)) if name == "READY"
        );
        if !is_ready {
            return Err(ReadyError::NotReady);
        }
        let d = value.get("d").ok_or_else(|| ReadyError::Missing {
            path: "d".to_string(),
        })?;
        // Контракт §2.1: отсутствие `d` — Missing, не-map `d` — NotReady.
        if d.as_map().is_none() {
            return Err(ReadyError::NotReady);
        }
        Self::parse(d)
    }

    /// Только payload `d`.
    ///
    /// Композиция заморожена дизайном A1 §3.5; наполнение парсеров — воркеры W12a/W12b/W12c.
    /// Заглушки возвращают пустые значения, чтобы каждый воркер мог заменить только свой файл.
    pub fn parse(d: &EtfValue) -> Result<Self, ReadyError> {
        let map = d.as_map().ok_or_else(|| ReadyError::WrongType {
            path: "d".to_string(),
            expected: "map",
            found: value::kind(d),
        })?;

        let guilds_value = d.get("guilds").ok_or_else(|| ReadyError::Missing {
            path: "d.guilds".to_string(),
        })?;
        if value::items(guilds_value).is_none() {
            return Err(ReadyError::WrongType {
                path: "d.guilds".to_string(),
                expected: "list",
                found: value::kind(guilds_value),
            });
        }

        let user_value = d.get("user").ok_or_else(|| ReadyError::Missing {
            path: "d.user".to_string(),
        })?;
        if value::object(user_value).is_none() {
            return Err(ReadyError::WrongType {
                path: "d.user".to_string(),
                expected: "map",
                found: value::kind(user_value),
            });
        }

        let session_value = d.get("session_id").ok_or_else(|| ReadyError::Missing {
            path: "d.session_id".to_string(),
        })?;
        let session_id = value::text(session_value).ok_or_else(|| ReadyError::WrongType {
            path: "d.session_id".to_string(),
            expected: "str",
            found: value::kind(session_value),
        })?;

        let mut diagnostics = ReadyDiagnostics::default();
        for (key, _) in map {
            let name = match key {
                EtfValue::Atom(name) | EtfValue::Str(name) => name.as_str(),
                _ => continue,
            };
            if !TOP_LEVEL_KEYS.contains(&name) {
                diagnostics.unknown_top_level.push(name.to_string());
            }
        }

        let parsed_guilds = guild::parse_guilds(guilds_value, &mut diagnostics);
        let mut guilds = parsed_guilds.guilds;
        let raw_guild_positions = parsed_guilds.raw_positions;
        let user = social::parse_user(user_value, "d.user", &mut diagnostics).ok_or_else(|| {
            ReadyError::InvalidId {
                path: "d.user.id".to_string(),
                value: user_value.get("id").map_or_else(
                    || "<missing>".to_string(),
                    |id| match id {
                        EtfValue::Str(text) | EtfValue::Atom(text) => text.clone(),
                        EtfValue::Int(number) => number.to_string(),
                        other => value::kind(other).to_string(),
                    },
                ),
            }
        })?;
        let users = social::parse_users(d.get("users"), &mut diagnostics);
        let private_channels =
            social::parse_private_channels(d.get("private_channels"), &mut diagnostics);
        let relationships = social::parse_relationships(d.get("relationships"), &mut diagnostics);
        let sessions = social::parse_sessions(d.get("sessions"), &mut diagnostics);
        let merged_value = d.get("merged_members");
        let merged_members = social::parse_merged_members(merged_value, &mut diagnostics);
        let read_state = read_state::parse_read_state(d.get("read_state"), &mut diagnostics);
        let user_guild_settings =
            read_state::parse_user_guild_settings(d.get("user_guild_settings"), &mut diagnostics);

        // `merged_members` привязан к сырым индексам гильдий. Явный пустой список — это
        // расхождение длин; отсутствие/`nil` — отсутствие данных (DESIGN §2.3, §3.5).
        if let Some(raw) = merged_value
            && !value::is_null(raw)
            && value::items(raw).is_some()
        {
            if merged_members.len() != raw_guild_positions.len() {
                diagnostics.malformed.push(format!(
                    "merged_members:{} vs guilds:{}",
                    merged_members.len(),
                    raw_guild_positions.len()
                ));
            }
            for (raw_index, members) in merged_members.iter().enumerate() {
                match raw_guild_positions.get(raw_index).copied().flatten() {
                    Some(guild_index) => guilds[guild_index].members = members.clone(),
                    None if !members.is_empty() => diagnostics
                        .malformed
                        .push(format!("merged_members[{raw_index}]")),
                    None => {}
                }
            }
        }

        Ok(Self {
            v: optional_integer(d, "v", &mut diagnostics),
            user,
            session_id: session_id.to_string(),
            session_type: optional_text(d, "session_type", &mut diagnostics),
            resume_gateway_url: optional_text(d, "resume_gateway_url", &mut diagnostics),
            guilds,
            private_channels,
            users,
            relationships,
            read_state,
            user_guild_settings,
            sessions,
            geo_ordered_rtc_regions: optional_text_list(
                d,
                "geo_ordered_rtc_regions",
                &mut diagnostics,
            ),
            country_code: optional_text(d, "country_code", &mut diagnostics),
            subdivision_code: optional_text(d, "subdivision_code", &mut diagnostics),
            friend_suggestion_count: optional_integer(
                d,
                "friend_suggestion_count",
                &mut diagnostics,
            ),
            api_code_version: optional_integer(d, "api_code_version", &mut diagnostics),
            explicit_content_scan_version: optional_integer(
                d,
                "explicit_content_scan_version",
                &mut diagnostics,
            ),
            ad_personalization_toggles_disabled: optional_boolean(
                d,
                "ad_personalization_toggles_disabled",
                &mut diagnostics,
            ),
            user_settings_proto: optional_text(d, "user_settings_proto", &mut diagnostics),
            top_level_keys: map.len(),
            diagnostics,
            raw: d.clone(),
        })
    }

    /// Сырой `d`; содержит секреты — не логировать.
    #[must_use]
    pub fn raw(&self) -> &EtfValue {
        &self.raw
    }
}

/// Необязательная строка: отсутствие/`nil` → `None`; неверный тип → `None` + `ignored`.
fn optional_text(d: &EtfValue, key: &str, diagnostics: &mut ReadyDiagnostics) -> Option<String> {
    let raw = d.get(key)?;
    if value::is_null(raw) {
        return None;
    }
    match value::text(raw) {
        Some(text) => Some(text.to_string()),
        None => {
            diagnostics.ignored.push(format!("d.{key}"));
            None
        }
    }
}

/// Необязательное целое: отсутствие/`nil` → `None`; неверный тип → `None` + `ignored`.
fn optional_integer(d: &EtfValue, key: &str, diagnostics: &mut ReadyDiagnostics) -> Option<i64> {
    let raw = d.get(key)?;
    if value::is_null(raw) {
        return None;
    }
    match value::integer(raw) {
        Some(number) => Some(number),
        None => {
            diagnostics.ignored.push(format!("d.{key}"));
            None
        }
    }
}

/// Необязательный bool: отсутствие/`nil` → `false`; неверный тип → `false` + `ignored`.
fn optional_boolean(d: &EtfValue, key: &str, diagnostics: &mut ReadyDiagnostics) -> bool {
    let Some(raw) = d.get(key) else {
        return false;
    };
    if value::is_null(raw) {
        return false;
    }
    match value::boolean(raw) {
        Some(flag) => flag,
        None => {
            diagnostics.ignored.push(format!("d.{key}"));
            false
        }
    }
}

/// Необязательный список строк: отсутствие/`nil` → пусто; иной тип/элемент → `ignored`.
fn optional_text_list(d: &EtfValue, key: &str, diagnostics: &mut ReadyDiagnostics) -> Vec<String> {
    let Some(raw) = d.get(key) else {
        return Vec::new();
    };
    if value::is_null(raw) {
        return Vec::new();
    }
    let Some(items) = value::items(raw) else {
        diagnostics.ignored.push(format!("d.{key}"));
        return Vec::new();
    };
    let mut texts = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        match value::text(item) {
            Some(text) => texts.push(text.to_string()),
            None => diagnostics.ignored.push(format!("d.{key}[{index}]")),
        }
    }
    texts
}

// Типы ре-экспортируются корнем `ready.rs`; здесь — только чтобы rustdoc видел связи.
#[allow(unused_imports)]
use super::guild::Channel as _Channel;
#[allow(unused_imports)]
use super::guild::Emoji as _Emoji;
#[allow(unused_imports)]
use super::guild::PermissionOverwrite as _PermissionOverwrite;
#[allow(unused_imports)]
use super::guild::Role as _Role;
#[allow(unused_imports)]
use super::guild::Sticker as _Sticker;
#[allow(unused_imports)]
use super::read_state::ChannelOverride as _ChannelOverride;
#[allow(unused_imports)]
use super::read_state::MuteConfig as _MuteConfig;
#[allow(unused_imports)]
use super::read_state::ReadStateEntry as _ReadStateEntry;
#[allow(unused_imports)]
use super::read_state::UserGuildSettingsEntry as _UserGuildSettingsEntry;
#[allow(unused_imports)]
use super::social::ClientInfo as _ClientInfo;
#[allow(unused_imports)]
use super::social::GuildMember as _GuildMember;
