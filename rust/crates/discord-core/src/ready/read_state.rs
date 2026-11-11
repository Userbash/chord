//! READY: read_state и user_guild_settings (RW-12 / P2-01c).
//!
//! Типы и сигнатуры заморожены `waves/RW-12/DESIGN-P2-01.md` §3.3–3.4. Политика контракта
//! A1 §2.3: отсутствие/`nil` → дефолт без диагностики; неверный тип необязательного поля →
//! дефолт + `diagnostics.ignored`; битый обязательный id элемента → skip +
//! `diagnostics.malformed`; дубликаты id — last wins, позиция первого, `diagnostics.duplicates`.
//! `read_state.entries[].id` — не только каналы (может быть пользователем/гильдией; P2-01 не
//! сверяет id со списком каналов, интерпретация — P2-07).

use discord_codec::etf::EtfValue;

use super::ReadyDiagnostics;
use super::value;

/// `d.read_state`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadState {
    /// Записи.
    pub entries: Vec<ReadStateEntry>,
    /// Partial.
    pub partial: bool,
    /// Версия.
    pub version: u64,
}

/// Запись read_state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadStateEntry {
    /// Снежинка (канал/пользователь/гильдия).
    pub id: u64,
    /// Флаги.
    pub flags: Option<i64>,
    /// Последнее сообщение.
    pub last_message_id: Option<u64>,
    /// Последний пин.
    pub last_pin_timestamp: Option<String>,
    /// Упоминания.
    pub mention_count: Option<i64>,
    /// Последний просмотр.
    pub last_viewed: Option<i64>,
    /// Бейджи.
    pub badge_count: Option<i64>,
    /// Последний ack.
    pub last_acked_id: Option<u64>,
    /// Тип read_state.
    pub read_state_type: Option<i64>,
}

/// `d.user_guild_settings`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserGuildSettings {
    /// Записи.
    pub entries: Vec<UserGuildSettingsEntry>,
    /// Partial.
    pub partial: bool,
    /// Версия.
    pub version: u64,
}

/// Настройки гильдии.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserGuildSettingsEntry {
    /// Гильдия (`None` для DM).
    pub guild_id: Option<u64>,
    /// Перезаписи каналов.
    pub channel_overrides: Vec<ChannelOverride>,
    /// Флаги.
    pub flags: i64,
    /// Скрывать mute-каналы.
    pub hide_muted_channels: bool,
    /// Уведомления.
    pub message_notifications: i64,
    /// Mobile push.
    pub mobile_push: bool,
    /// Mute config.
    pub mute_config: Option<MuteConfig>,
    /// Mute scheduled events.
    pub mute_scheduled_events: bool,
    /// Muted.
    pub muted: bool,
    /// Notify highlights.
    pub notify_highlights: i64,
    /// Suppress everyone.
    pub suppress_everyone: bool,
    /// Suppress roles.
    pub suppress_roles: bool,
    /// Версия.
    pub version: u64,
}

/// Перезапись настроек канала.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChannelOverride {
    /// Канал.
    pub channel_id: u64,
    /// Свёрнут.
    pub collapsed: bool,
    /// Флаги (отсутствуют 75/1328).
    pub flags: Option<i64>,
    /// Уведомления.
    pub message_notifications: i64,
    /// Mute config.
    pub mute_config: Option<MuteConfig>,
    /// Muted.
    pub muted: bool,
}

/// Mute config.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MuteConfig {
    /// Конец mute.
    pub end_time: Option<String>,
    /// Окно.
    pub selected_time_window: Option<i64>,
}

/// `d.read_state`: отсутствие/`nil` → `Default`; не map → `Default` + `ignored`; `entries`
/// неверного типа → пусто + `ignored`; дубликаты id — last wins (позиция первого).
pub(crate) fn parse_read_state(
    value: Option<&EtfValue>,
    diagnostics: &mut ReadyDiagnostics,
) -> ReadState {
    let Some(value) = value else {
        return ReadState::default();
    };
    if value::is_null(value) {
        return ReadState::default();
    }
    if value::object(value).is_none() {
        diagnostics.ignored.push("d.read_state".to_string());
        return ReadState::default();
    }

    let mut entries = Vec::new();
    match value.get("entries") {
        None => {}
        Some(entries_value) if value::is_null(entries_value) => {}
        Some(entries_value) => match value::items(entries_value) {
            Some(items) => {
                for (index, item) in items.iter().enumerate() {
                    if let Some(entry) = parse_read_state_entry(item, index, diagnostics) {
                        entries.push(entry);
                    }
                }
            }
            None => diagnostics.ignored.push("d.read_state.entries".to_string()),
        },
    }
    let entries = value::dedup_last_by(
        entries,
        |entry| entry.id,
        |entry| entry.id.to_string(),
        "read_state.entries",
        diagnostics,
    );

    ReadState {
        entries,
        partial: field_bool(value, "partial", "d.read_state", diagnostics).unwrap_or(false),
        version: field_unsigned(value, "version", "d.read_state", diagnostics).unwrap_or(0),
    }
}

/// Запись `read_state`: обязательный `id`; без валидного id элемент пропускается.
fn parse_read_state_entry(
    item: &EtfValue,
    index: usize,
    diagnostics: &mut ReadyDiagnostics,
) -> Option<ReadStateEntry> {
    let Some(id) = item.get("id").and_then(value::unsigned) else {
        diagnostics
            .malformed
            .push(format!("read_state.entries[{index}]"));
        return None;
    };
    let path = format!("d.read_state.entries[{index}]");
    Some(ReadStateEntry {
        id,
        flags: field_integer(item, "flags", &path, diagnostics),
        last_message_id: field_unsigned(item, "last_message_id", &path, diagnostics),
        last_pin_timestamp: field_text(item, "last_pin_timestamp", &path, diagnostics),
        mention_count: field_integer(item, "mention_count", &path, diagnostics),
        last_viewed: field_integer(item, "last_viewed", &path, diagnostics),
        badge_count: field_integer(item, "badge_count", &path, diagnostics),
        last_acked_id: field_unsigned(item, "last_acked_id", &path, diagnostics),
        read_state_type: field_integer(item, "read_state_type", &path, diagnostics),
    })
}

/// `d.user_guild_settings`: та же политика, что у [`parse_read_state`]; ключ дубликата —
/// `guild_id` (`None` для DM — отдельный ключ `nil`).
pub(crate) fn parse_user_guild_settings(
    value: Option<&EtfValue>,
    diagnostics: &mut ReadyDiagnostics,
) -> UserGuildSettings {
    let Some(value) = value else {
        return UserGuildSettings::default();
    };
    if value::is_null(value) {
        return UserGuildSettings::default();
    }
    if value::object(value).is_none() {
        diagnostics
            .ignored
            .push("d.user_guild_settings".to_string());
        return UserGuildSettings::default();
    }

    let mut entries = Vec::new();
    match value.get("entries") {
        None => {}
        Some(entries_value) if value::is_null(entries_value) => {}
        Some(entries_value) => match value::items(entries_value) {
            Some(items) => {
                for (index, item) in items.iter().enumerate() {
                    if let Some(entry) = parse_user_guild_settings_entry(item, index, diagnostics) {
                        entries.push(entry);
                    }
                }
            }
            None => diagnostics
                .ignored
                .push("d.user_guild_settings.entries".to_string()),
        },
    }
    let entries = value::dedup_last_by(
        entries,
        |entry| entry.guild_id,
        |entry| {
            entry
                .guild_id
                .map_or_else(|| "nil".to_string(), |id| id.to_string())
        },
        "user_guild_settings.entries",
        diagnostics,
    );

    UserGuildSettings {
        entries,
        partial: field_bool(value, "partial", "d.user_guild_settings", diagnostics)
            .unwrap_or(false),
        version: field_unsigned(value, "version", "d.user_guild_settings", diagnostics)
            .unwrap_or(0),
    }
}

/// Настройки гильдии: обязательных полей нет (даже `guild_id` может быть `nil`).
fn parse_user_guild_settings_entry(
    item: &EtfValue,
    index: usize,
    diagnostics: &mut ReadyDiagnostics,
) -> Option<UserGuildSettingsEntry> {
    if value::object(item).is_none() {
        diagnostics
            .malformed
            .push(format!("user_guild_settings.entries[{index}]"));
        return None;
    }
    let path = format!("d.user_guild_settings.entries[{index}]");
    Some(UserGuildSettingsEntry {
        guild_id: field_unsigned(item, "guild_id", &path, diagnostics),
        channel_overrides: parse_channel_overrides(
            item.get("channel_overrides"),
            &path,
            diagnostics,
        ),
        flags: field_integer(item, "flags", &path, diagnostics).unwrap_or(0),
        hide_muted_channels: field_bool(item, "hide_muted_channels", &path, diagnostics)
            .unwrap_or(false),
        message_notifications: field_integer(item, "message_notifications", &path, diagnostics)
            .unwrap_or(0),
        mobile_push: field_bool(item, "mobile_push", &path, diagnostics).unwrap_or(false),
        mute_config: parse_mute_config(item.get("mute_config"), &path, diagnostics),
        mute_scheduled_events: field_bool(item, "mute_scheduled_events", &path, diagnostics)
            .unwrap_or(false),
        muted: field_bool(item, "muted", &path, diagnostics).unwrap_or(false),
        notify_highlights: field_integer(item, "notify_highlights", &path, diagnostics)
            .unwrap_or(0),
        suppress_everyone: field_bool(item, "suppress_everyone", &path, diagnostics)
            .unwrap_or(false),
        suppress_roles: field_bool(item, "suppress_roles", &path, diagnostics).unwrap_or(false),
        version: field_unsigned(item, "version", &path, diagnostics).unwrap_or(0),
    })
}

/// `channel_overrides`: отсутствие/`nil` → пусто; не list → пусто + `ignored`; элемент без
/// валидного `channel_id` пропускается в `malformed` (дубликатов в контракте нет).
fn parse_channel_overrides(
    value: Option<&EtfValue>,
    parent_path: &str,
    diagnostics: &mut ReadyDiagnostics,
) -> Vec<ChannelOverride> {
    let Some(value) = value else {
        return Vec::new();
    };
    if value::is_null(value) {
        return Vec::new();
    }
    let Some(items) = value::items(value) else {
        diagnostics
            .ignored
            .push(format!("{parent_path}.channel_overrides"));
        return Vec::new();
    };

    // `ignored`-пути идут от `d` (пример дизайна §3.3: `d.read_state.version`), `malformed` —
    // от ключа коллекции (`guilds[2]`, `...channel_overrides[1]`).
    let malformed_path = parent_path.strip_prefix("d.").unwrap_or(parent_path);
    let mut overrides = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let Some(channel_id) = item.get("channel_id").and_then(value::unsigned) else {
            diagnostics
                .malformed
                .push(format!("{malformed_path}.channel_overrides[{index}]"));
            continue;
        };
        let path = format!("{parent_path}.channel_overrides[{index}]");
        overrides.push(ChannelOverride {
            channel_id,
            collapsed: field_bool(item, "collapsed", &path, diagnostics).unwrap_or(false),
            flags: field_integer(item, "flags", &path, diagnostics),
            message_notifications: field_integer(item, "message_notifications", &path, diagnostics)
                .unwrap_or(0),
            mute_config: parse_mute_config(item.get("mute_config"), &path, diagnostics),
            muted: field_bool(item, "muted", &path, diagnostics).unwrap_or(false),
        });
    }
    overrides
}

/// `mute_config`: отсутствие/`nil` → `None`; не map → `None` + `ignored`.
fn parse_mute_config(
    value: Option<&EtfValue>,
    parent_path: &str,
    diagnostics: &mut ReadyDiagnostics,
) -> Option<MuteConfig> {
    let value = value?;
    if value::is_null(value) {
        return None;
    }
    if value::object(value).is_none() {
        diagnostics
            .ignored
            .push(format!("{parent_path}.mute_config"));
        return None;
    }
    let path = format!("{parent_path}.mute_config");
    Some(MuteConfig {
        end_time: field_text(value, "end_time", &path, diagnostics),
        selected_time_window: field_integer(value, "selected_time_window", &path, diagnostics),
    })
}

/// Поле объекта: отсутствие/`nil`/пустая строка → `None` без диагностики; неверный тип → `None` + `ignored`.
fn present_field<T>(
    object: &EtfValue,
    key: &str,
    path: &str,
    diagnostics: &mut ReadyDiagnostics,
    parse: impl FnOnce(&EtfValue) -> Option<T>,
) -> Option<T> {
    let value = object.get(key)?;
    if value::is_null(value)
        || matches!(
            value,
            EtfValue::Str(text) | EtfValue::Atom(text)
                if text == "nil" || text == "null" || text.trim().is_empty()
        )
    {
        return None;
    }
    let parsed = parse(value);
    if parsed.is_none() {
        diagnostics.ignored.push(format!("{path}.{key}"));
    }
    parsed
}

fn field_unsigned(
    object: &EtfValue,
    key: &str,
    path: &str,
    diagnostics: &mut ReadyDiagnostics,
) -> Option<u64> {
    present_field(object, key, path, diagnostics, value::unsigned)
}

fn field_integer(
    object: &EtfValue,
    key: &str,
    path: &str,
    diagnostics: &mut ReadyDiagnostics,
) -> Option<i64> {
    present_field(object, key, path, diagnostics, |raw| {
        value::integer(raw).or_else(|| value::text(raw).and_then(|text| text.trim().parse().ok()))
    })
}

fn field_bool(
    object: &EtfValue,
    key: &str,
    path: &str,
    diagnostics: &mut ReadyDiagnostics,
) -> Option<bool> {
    present_field(object, key, path, diagnostics, value::boolean)
}

fn field_text(
    object: &EtfValue,
    key: &str,
    path: &str,
    diagnostics: &mut ReadyDiagnostics,
) -> Option<String> {
    present_field(object, key, path, diagnostics, |raw| {
        value::text(raw).map(str::to_string)
    })
}
