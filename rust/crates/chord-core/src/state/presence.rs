//! P2-09a: presence-типы и разбор (RW-18 / W18a).
//!
//! Контракт заморожен `waves/RW-18/DESIGN-P2-09.md` §2–§4. Модель memory-only:
//! scope `guild|private`, статусы, client statuses и безопасное подмножество activities.
//! `secrets`, `session_id`, `sync_id`, `restricted_application` и
//! `content_classification` не сохраняются.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use chord_codec::etf::EtfValue;

use crate::ready::social::parse_user;
use crate::ready::{ReadyDiagnostics, User, value};

use super::types::{EventError, StateDiagnostics, raw_display};

/// Статус присутствия.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum PresenceStatus {
    /// В сети.
    Online,
    /// Неактивен.
    Idle,
    /// Не беспокоить.
    Dnd,
    /// Не в сети (дефолт для отсутствующей записи).
    #[default]
    Offline,
    /// Невидим (в capture не встречался, но значение известно клиенту).
    Invisible,
    /// Неизвестный проводной статус; сохраняется без потери.
    Unknown(String),
}

impl PresenceStatus {
    /// Разобрать статус провода.
    #[must_use]
    pub fn from_wire(value: &str) -> Self {
        match value {
            "online" => Self::Online,
            "idle" => Self::Idle,
            "dnd" => Self::Dnd,
            "offline" => Self::Offline,
            "invisible" => Self::Invisible,
            other => Self::Unknown(other.to_string()),
        }
    }

    /// Проводное/исходное имя.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Online => "online",
            Self::Idle => "idle",
            Self::Dnd => "dnd",
            Self::Offline => "offline",
            Self::Invisible => "invisible",
            Self::Unknown(value) => value,
        }
    }

    /// Считается ли статус присутствием в сети (online/idle/dnd).
    #[must_use]
    pub fn is_online(&self) -> bool {
        matches!(self, Self::Online | Self::Idle | Self::Dnd)
    }

    /// Статус `offline`.
    #[must_use]
    pub fn is_offline(&self) -> bool {
        matches!(self, Self::Offline)
    }
}

/// Тип activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActivityType {
    /// Играет.
    Playing,
    /// Стримит.
    Streaming,
    /// Слушает.
    Listening,
    /// Смотрит.
    Watching,
    /// Custom status.
    Custom,
    /// Соревнуется.
    Competing,
    /// Неизвестный тип; сохраняется как число.
    Unknown(i64),
}

impl Default for ActivityType {
    fn default() -> Self {
        Self::Unknown(-1)
    }
}

impl ActivityType {
    /// Разобрать числовой тип.
    #[must_use]
    pub fn from_i64(value: i64) -> Self {
        match value {
            0 => Self::Playing,
            1 => Self::Streaming,
            2 => Self::Listening,
            3 => Self::Watching,
            4 => Self::Custom,
            5 => Self::Competing,
            other => Self::Unknown(other),
        }
    }

    /// Числовое значение провода.
    #[must_use]
    pub fn as_i64(self) -> i64 {
        match self {
            Self::Playing => 0,
            Self::Streaming => 1,
            Self::Listening => 2,
            Self::Watching => 3,
            Self::Custom => 4,
            Self::Competing => 5,
            Self::Unknown(value) => value,
        }
    }

    /// Приоритет официального клиента: custom > competing > streaming > playing > остальные.
    #[must_use]
    pub fn priority(self) -> u8 {
        match self {
            Self::Custom => 4,
            Self::Competing => 3,
            Self::Streaming => 2,
            Self::Playing => 1,
            _ => 0,
        }
    }

    /// Custom status.
    #[must_use]
    pub fn is_custom(self) -> bool {
        matches!(self, Self::Custom)
    }

    /// Playing-активность.
    #[must_use]
    pub fn is_playing(self) -> bool {
        matches!(self, Self::Playing)
    }
}

/// Времена activity.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActivityTimestamps {
    /// Начало (Unix ms).
    pub start: Option<i64>,
    /// Конец (Unix ms).
    pub end: Option<i64>,
}

/// Эмодзи custom status/activity.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActivityEmoji {
    /// Имя.
    pub name: Option<String>,
    /// Id эмодзи строкой.
    pub id: Option<String>,
    /// Анимировано.
    pub animated: Option<bool>,
}

/// Assets activity.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActivityAssets {
    /// Большая картинка.
    pub large_image: Option<String>,
    /// Текст большой картинки.
    pub large_text: Option<String>,
    /// Малая картинка.
    pub small_image: Option<String>,
    /// Текст малой картинки.
    pub small_text: Option<String>,
}

/// Party activity.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActivityParty {
    /// Идентификатор пати.
    pub id: Option<String>,
    /// Текущий размер.
    pub current_size: Option<u32>,
    /// Максимальный размер.
    pub max_size: Option<u32>,
}

/// Безопасное подмножество activity.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Activity {
    /// Тип.
    pub activity_type: ActivityType,
    /// Имя (обязательно на проводе).
    pub name: String,
    /// Детали.
    pub details: Option<String>,
    /// Состояние.
    pub state: Option<String>,
    /// URL.
    pub url: Option<String>,
    /// Id activity.
    pub id: Option<String>,
    /// Платформа.
    pub platform: Option<String>,
    /// Время создания (Unix ms).
    pub created_at: Option<i64>,
    /// Application id строкой (в capture бывает отрицательным).
    pub application_id: Option<String>,
    /// Parent application id строкой.
    pub parent_application_id: Option<String>,
    /// Времена.
    pub timestamps: Option<ActivityTimestamps>,
    /// Эмодзи.
    pub emoji: Option<ActivityEmoji>,
    /// Assets.
    pub assets: Option<ActivityAssets>,
    /// Party.
    pub party: Option<ActivityParty>,
    /// Кнопки.
    pub buttons: Vec<String>,
    /// Поддерживаемые платформы.
    pub supported_platforms: Vec<String>,
    /// Флаги.
    pub flags: Option<i64>,
    /// `status_display_type`.
    pub status_display_type: Option<i64>,
}

impl Activity {
    /// Есть ли у activity отображаемое содержимое (официальное правило сортировки).
    #[must_use]
    pub(crate) fn has_displayable_content(&self) -> bool {
        !self.activity_type.is_custom()
            && (self.details.is_some()
                || self.assets.as_ref().is_some_and(|assets| {
                    assets.large_image.is_some() || assets.small_text.is_some()
                })
                || self.party.is_some()
                || self.state.is_some())
    }
}

/// Scope presence: гильдия или приватный/друзья.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PresenceScope {
    /// Гильдия.
    Guild(u64),
    /// Приватный scope (`ME` в официальном клиенте, merged friends).
    Private,
}

/// Ключ presence-store.
pub type PresenceKey = (u64, PresenceScope);

/// Presence одного пользователя в одном scope.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Presence {
    /// Пользователь.
    pub user_id: u64,
    /// Гильдия; `None` — приватный scope.
    pub guild_id: Option<u64>,
    /// Статус.
    pub status: PresenceStatus,
    /// Клиенты.
    pub client_status: BTreeMap<String, PresenceStatus>,
    /// Нормализованные activities.
    pub activities: Vec<Activity>,
    /// Скрытые activities (фильтр контента не включён; сохраняем отдельно).
    pub hidden_activities: Vec<Activity>,
    /// Время обработки сервером.
    pub processed_at_timestamp: Option<i64>,
    /// `since`.
    pub since: Option<i64>,
    /// `broadcast`.
    pub broadcast: Option<bool>,
}

impl Presence {
    /// Scope записи.
    #[must_use]
    pub fn scope(&self) -> PresenceScope {
        self.guild_id
            .map_or(PresenceScope::Private, PresenceScope::Guild)
    }

    /// Видима ли запись: не offline либо есть hidden activities.
    #[must_use]
    pub fn is_visible(&self) -> bool {
        !self.status.is_offline() || !self.hidden_activities.is_empty()
    }
}

/// `PRESENCE_UPDATE` после разбора.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresenceUpdate {
    /// Presence.
    pub presence: Presence,
    /// Пользователь из live-события (для merge в `State.users`).
    pub user: Option<User>,
}

/// Начальные presence-снимки `READY_SUPPLEMENTAL`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergedPresences {
    /// Друзья/приватные.
    pub friends: Vec<Presence>,
    /// Группы, выровненные по `ReadySupplemental.guild_ids`.
    pub guilds: Vec<Vec<Presence>>,
}

/// Разобрать `PRESENCE_UPDATE`; фатален только пользователь.
pub(crate) fn parse_presence_event(
    data: &EtfValue,
) -> Result<(PresenceUpdate, StateDiagnostics), EventError> {
    let map = data.as_map().ok_or_else(|| EventError::WrongType {
        path: "d".to_string(),
        expected: "map",
        found: value::kind(data),
    })?;
    let user_raw = map_get(map, "user").ok_or_else(|| EventError::Missing {
        path: "presence.user".to_string(),
    })?;
    if value::object(user_raw).is_none() {
        return Err(EventError::WrongType {
            path: "presence.user".to_string(),
            expected: "map",
            found: value::kind(user_raw),
        });
    }
    let mut ready = ReadyDiagnostics::default();
    let user_id_display = value::object(user_raw)
        .and_then(|user| map_get(user, "id"))
        .map_or_else(|| raw_display(user_raw), raw_display);
    let user =
        parse_user(user_raw, "presence.user", &mut ready).ok_or_else(|| EventError::InvalidId {
            path: "presence.user.id".to_string(),
            value: user_id_display,
        })?;
    let mut diagnostics = StateDiagnostics::default();
    diagnostics.merge(&StateDiagnostics::from_ready(&ready));

    let guild_id = optional_guild_id(map, "presence.guild_id", &mut diagnostics);
    let presence = parse_presence_fields(map, "presence", user.id, guild_id, &mut diagnostics);
    Ok((
        PresenceUpdate {
            presence,
            user: Some(user),
        },
        diagnostics,
    ))
}

/// Разобрать `merged_presences`; `guilds` всегда выровнен по `guild_ids`.
pub(crate) fn parse_merged_presences(
    raw: Option<&EtfValue>,
    guild_ids: &[Option<u64>],
    diagnostics: &mut StateDiagnostics,
) -> MergedPresences {
    let Some(raw) = raw else {
        return MergedPresences {
            friends: Vec::new(),
            guilds: vec![Vec::new(); guild_ids.len()],
        };
    };
    if value::is_null(raw) {
        return MergedPresences {
            friends: Vec::new(),
            guilds: vec![Vec::new(); guild_ids.len()],
        };
    }
    let Some(map) = raw.as_map() else {
        diagnostics.ignored.push("merged_presences".to_string());
        return MergedPresences {
            friends: Vec::new(),
            guilds: vec![Vec::new(); guild_ids.len()],
        };
    };

    let friends = parse_presence_list(
        map_get(map, "friends"),
        "merged_presences.friends",
        None,
        diagnostics,
    );

    let mut guilds = Vec::with_capacity(guild_ids.len());
    let groups = match map_get(map, "guilds") {
        None => None,
        Some(value) if value::is_null(value) => None,
        Some(value) => match value::items(value) {
            Some(items) => Some(items),
            None => {
                diagnostics
                    .ignored
                    .push("merged_presences.guilds".to_string());
                None
            }
        },
    };

    if let Some(groups) = groups {
        if groups.len() != guild_ids.len() {
            diagnostics.malformed.push(format!(
                "merged_presences.guilds:{} vs guilds:{}",
                groups.len(),
                guild_ids.len()
            ));
        }
        for (index, group) in groups.iter().enumerate() {
            let Some(guild_id) = guild_ids.get(index).copied().flatten() else {
                if index < guild_ids.len() {
                    if !is_empty_presence_group(group) {
                        diagnostics
                            .inconsistencies
                            .push(format!("merged_presences.guilds[{index}]"));
                    }
                    guilds.push(Vec::new());
                }
                // Лишняя группа уже отмечена malformed о несовпадении длины.
                continue;
            };
            if index >= guild_ids.len() {
                // Лишняя группа не влезает в выравнивание; факт уже отмечен malformed.
                continue;
            }
            guilds.push(parse_presence_list(
                Some(group),
                &format!("merged_presences.guilds[{index}]"),
                Some(guild_id),
                diagnostics,
            ));
        }
    }

    while guilds.len() < guild_ids.len() {
        guilds.push(Vec::new());
    }
    guilds.truncate(guild_ids.len());

    MergedPresences { friends, guilds }
}

fn is_empty_presence_group(value: &EtfValue) -> bool {
    value::is_null(value) || value::items(value).is_some_and(|items| items.is_empty())
}

fn parse_presence_list(
    raw: Option<&EtfValue>,
    path: &str,
    guild_id: Option<u64>,
    diagnostics: &mut StateDiagnostics,
) -> Vec<Presence> {
    let Some(raw) = raw else {
        return Vec::new();
    };
    if value::is_null(raw) {
        return Vec::new();
    }
    let Some(items) = value::items(raw) else {
        diagnostics.ignored.push(path.to_string());
        return Vec::new();
    };
    let mut presences = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let item_path = format!("{path}[{index}]");
        let Some(map) = item.as_map() else {
            diagnostics.malformed.push(item_path);
            continue;
        };
        let Some(user_id) = map_get(map, "user_id").and_then(value::unsigned) else {
            diagnostics.malformed.push(item_path);
            continue;
        };
        presences.push(parse_presence_fields(
            map,
            &item_path,
            user_id,
            guild_id,
            diagnostics,
        ));
    }
    presences
}

fn parse_presence_fields(
    map: &[(EtfValue, EtfValue)],
    path: &str,
    user_id: u64,
    guild_id: Option<u64>,
    diagnostics: &mut StateDiagnostics,
) -> Presence {
    let status = parse_status(
        map_get(map, "status"),
        &format!("{path}.status"),
        diagnostics,
    );
    let client_status = parse_client_status(
        map_get(map, "client_status"),
        &format!("{path}.client_status"),
        diagnostics,
    );
    let mut activities = normalize_activities(parse_activities(
        map_get(map, "activities"),
        &format!("{path}.activities"),
        diagnostics,
    ));
    let hidden_activities = parse_activities(
        map_get(map, "hidden_activities"),
        &format!("{path}.hidden_activities"),
        diagnostics,
    );
    if status.is_offline() && hidden_activities.is_empty() {
        activities.clear();
    }
    Presence {
        user_id,
        guild_id,
        status,
        client_status,
        activities,
        hidden_activities,
        processed_at_timestamp: parse_i64_field(
            map_get(map, "processed_at_timestamp"),
            &format!("{path}.processed_at_timestamp"),
            diagnostics,
        ),
        since: parse_i64_field(map_get(map, "since"), &format!("{path}.since"), diagnostics),
        broadcast: parse_bool_field(
            map_get(map, "broadcast"),
            &format!("{path}.broadcast"),
            diagnostics,
        ),
    }
}

fn parse_status(
    raw: Option<&EtfValue>,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> PresenceStatus {
    let Some(raw) = raw else {
        diagnostics.ignored.push(path.to_string());
        return PresenceStatus::Offline;
    };
    if value::is_null(raw) {
        diagnostics.ignored.push(path.to_string());
        return PresenceStatus::Offline;
    }
    match value::text(raw) {
        Some(text) if !text.is_empty() => PresenceStatus::from_wire(text),
        _ => {
            diagnostics.ignored.push(path.to_string());
            PresenceStatus::Offline
        }
    }
}

fn parse_client_status(
    raw: Option<&EtfValue>,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> BTreeMap<String, PresenceStatus> {
    let Some(raw) = raw else {
        return BTreeMap::new();
    };
    if value::is_null(raw) {
        return BTreeMap::new();
    }
    let Some(entries) = value::object(raw) else {
        diagnostics.ignored.push(path.to_string());
        return BTreeMap::new();
    };
    let mut statuses = BTreeMap::new();
    for (key, value) in entries {
        let Some(key_text) = value::text(key) else {
            diagnostics.ignored.push(format!("{path}.empty-key"));
            continue;
        };
        if key_text.is_empty() {
            diagnostics.ignored.push(format!("{path}.empty-key"));
            continue;
        }
        match value::text(value) {
            Some(text) if !text.is_empty() => {
                statuses.insert(key_text.to_string(), PresenceStatus::from_wire(text));
            }
            _ => diagnostics.ignored.push(format!("{path}.{key_text}")),
        }
    }
    statuses
}

pub(crate) fn parse_activities(
    raw: Option<&EtfValue>,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Vec<Activity> {
    let Some(raw) = raw else {
        return Vec::new();
    };
    if value::is_null(raw) {
        return Vec::new();
    }
    let Some(items) = value::items(raw) else {
        diagnostics.ignored.push(path.to_string());
        return Vec::new();
    };
    let mut activities = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let item_path = format!("{path}[{index}]");
        if item.as_map().is_none() {
            diagnostics.ignored.push(item_path);
            continue;
        }
        if let Some(activity) = parse_activity(item, &item_path, diagnostics) {
            activities.push(activity);
        }
    }
    activities
}

fn parse_activity(
    raw: &EtfValue,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Option<Activity> {
    let map = raw.as_map()?;
    let activity_type = match map_get(map, "type").and_then(value::integer) {
        Some(value) => ActivityType::from_i64(value),
        None => {
            diagnostics.ignored.push(format!("{path}.type"));
            return None;
        }
    };
    let name = map_get(map, "name")
        .and_then(value::text)
        .filter(|name| !name.is_empty())
        .map(str::to_string);
    let Some(name) = name else {
        diagnostics.ignored.push(format!("{path}.name"));
        return None;
    };

    Some(Activity {
        activity_type,
        name,
        details: optional_text(map, "details", path, diagnostics),
        state: optional_text(map, "state", path, diagnostics),
        url: optional_text(map, "url", path, diagnostics),
        id: optional_text(map, "id", path, diagnostics),
        platform: optional_text(map, "platform", path, diagnostics),
        created_at: parse_i64_field(
            map_get(map, "created_at"),
            &format!("{path}.created_at"),
            diagnostics,
        ),
        application_id: text_or_int(
            map_get(map, "application_id"),
            &format!("{path}.application_id"),
            diagnostics,
        ),
        parent_application_id: text_or_int(
            map_get(map, "parent_application_id"),
            &format!("{path}.parent_application_id"),
            diagnostics,
        ),
        timestamps: parse_timestamps(
            map_get(map, "timestamps"),
            &format!("{path}.timestamps"),
            diagnostics,
        ),
        emoji: parse_emoji(map_get(map, "emoji"), &format!("{path}.emoji"), diagnostics),
        assets: parse_assets(
            map_get(map, "assets"),
            &format!("{path}.assets"),
            diagnostics,
        ),
        party: parse_party(map_get(map, "party"), &format!("{path}.party"), diagnostics),
        buttons: parse_text_list(
            map_get(map, "buttons"),
            &format!("{path}.buttons"),
            diagnostics,
        ),
        supported_platforms: parse_text_list(
            map_get(map, "supported_platforms"),
            &format!("{path}.supported_platforms"),
            diagnostics,
        ),
        flags: parse_i64_field(map_get(map, "flags"), &format!("{path}.flags"), diagnostics),
        status_display_type: parse_i64_field(
            map_get(map, "status_display_type"),
            &format!("{path}.status_display_type"),
            diagnostics,
        ),
    })
}

fn parse_timestamps(
    raw: Option<&EtfValue>,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Option<ActivityTimestamps> {
    let raw = raw?;
    if value::is_null(raw) {
        return None;
    }
    let Some(map) = raw.as_map() else {
        diagnostics.ignored.push(path.to_string());
        return None;
    };
    Some(ActivityTimestamps {
        start: parse_i64_field(map_get(map, "start"), &format!("{path}.start"), diagnostics),
        end: parse_i64_field(map_get(map, "end"), &format!("{path}.end"), diagnostics),
    })
}

fn parse_emoji(
    raw: Option<&EtfValue>,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Option<ActivityEmoji> {
    let raw = raw?;
    if value::is_null(raw) {
        return None;
    }
    let Some(map) = raw.as_map() else {
        diagnostics.ignored.push(path.to_string());
        return None;
    };
    Some(ActivityEmoji {
        name: optional_text(map, "name", path, diagnostics),
        id: optional_text(map, "id", path, diagnostics),
        animated: parse_bool_field(
            map_get(map, "animated"),
            &format!("{path}.animated"),
            diagnostics,
        ),
    })
}

fn parse_assets(
    raw: Option<&EtfValue>,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Option<ActivityAssets> {
    let raw = raw?;
    if value::is_null(raw) {
        return None;
    }
    let Some(map) = raw.as_map() else {
        diagnostics.ignored.push(path.to_string());
        return None;
    };
    Some(ActivityAssets {
        large_image: optional_text(map, "large_image", path, diagnostics),
        large_text: optional_text(map, "large_text", path, diagnostics),
        small_image: optional_text(map, "small_image", path, diagnostics),
        small_text: optional_text(map, "small_text", path, diagnostics),
    })
}

fn parse_party(
    raw: Option<&EtfValue>,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Option<ActivityParty> {
    let raw = raw?;
    if value::is_null(raw) {
        return None;
    }
    let Some(map) = raw.as_map() else {
        diagnostics.ignored.push(path.to_string());
        return None;
    };
    let (current_size, max_size) =
        parse_party_size(map_get(map, "size"), &format!("{path}.size"), diagnostics);
    Some(ActivityParty {
        id: optional_text(map, "id", path, diagnostics),
        current_size,
        max_size,
    })
}

fn parse_party_size(
    raw: Option<&EtfValue>,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> (Option<u32>, Option<u32>) {
    let Some(raw) = raw else {
        return (None, None);
    };
    if value::is_null(raw) {
        return (None, None);
    }
    if let Some(text) = value::text(raw) {
        let bytes = text.as_bytes();
        if bytes.len() == 2 {
            return (Some(u32::from(bytes[0])), Some(u32::from(bytes[1])));
        }
        diagnostics.ignored.push(path.to_string());
        return (None, None);
    }
    if let Some(items) = value::items(raw)
        && items.len() == 2
    {
        let current = value::unsigned(&items[0]).and_then(|value| u32::try_from(value).ok());
        let max = value::unsigned(&items[1]).and_then(|value| u32::try_from(value).ok());
        if let (Some(current), Some(max)) = (current, max) {
            return (Some(current), Some(max));
        }
    }
    diagnostics.ignored.push(path.to_string());
    (None, None)
}

fn parse_text_list(
    raw: Option<&EtfValue>,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Vec<String> {
    let Some(raw) = raw else {
        return Vec::new();
    };
    if value::is_null(raw) {
        return Vec::new();
    }
    let Some(items) = value::items(raw) else {
        diagnostics.ignored.push(path.to_string());
        return Vec::new();
    };
    let mut values = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        match value::text(item) {
            Some(text) if !text.is_empty() => values.push(text.to_string()),
            _ => diagnostics.ignored.push(format!("{path}[{index}]")),
        }
    }
    values
}

fn optional_text(
    map: &[(EtfValue, EtfValue)],
    key: &str,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Option<String> {
    let raw = map_get(map, key)?;
    if value::is_null(raw) {
        return None;
    }
    match value::text(raw) {
        Some(text) => Some(text.to_string()),
        None => {
            diagnostics.ignored.push(format!("{path}.{key}"));
            None
        }
    }
}

fn text_or_int(
    raw: Option<&EtfValue>,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Option<String> {
    let raw = raw?;
    if value::is_null(raw) {
        return None;
    }
    if let Some(text) = value::text(raw) {
        return Some(text.to_string());
    }
    if let Some(value) = value::integer(raw) {
        return Some(value.to_string());
    }
    diagnostics.ignored.push(path.to_string());
    None
}

pub(crate) fn parse_i64_field(
    raw: Option<&EtfValue>,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Option<i64> {
    let raw = raw?;
    if value::is_null(raw) {
        return None;
    }
    if let Some(value) = value::integer(raw) {
        return Some(value);
    }
    if let Some(text) = value::text(raw)
        && let Ok(value) = text.parse::<i64>()
    {
        return Some(value);
    }
    diagnostics.ignored.push(path.to_string());
    None
}

fn parse_bool_field(
    raw: Option<&EtfValue>,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Option<bool> {
    let raw = raw?;
    if value::is_null(raw) {
        return None;
    }
    match value::boolean(raw) {
        Some(value) => Some(value),
        None => {
            diagnostics.ignored.push(path.to_string());
            None
        }
    }
}

fn optional_guild_id(
    map: &[(EtfValue, EtfValue)],
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Option<u64> {
    let raw = map_get(map, "guild_id")?;
    if value::is_null(raw) {
        return None;
    }
    match value::unsigned(raw) {
        Some(id) if id > 0 => Some(id),
        Some(_) | None => {
            diagnostics.ignored.push(path.to_string());
            None
        }
    }
}

fn map_get<'a>(map: &'a [(EtfValue, EtfValue)], key: &str) -> Option<&'a EtfValue> {
    map.iter()
        .find(|(entry_key, _)| {
            entry_key.as_atom() == Some(key) || value::text(entry_key) == Some(key)
        })
        .map(|(_, value)| value)
}

pub(crate) fn normalize_activities(mut activities: Vec<Activity>) -> Vec<Activity> {
    if activities.len() <= 1 {
        return activities;
    }
    let mut playing = Vec::new();
    let mut other = Vec::new();
    for activity in activities.drain(..) {
        if activity.activity_type.is_playing() {
            playing.push(activity);
        } else {
            other.push(activity);
        }
    }
    if playing.len() > 1 {
        playing.sort_by(compare_activity);
        playing.truncate(1);
    }
    other.extend(playing);
    other.sort_by(compare_activity);
    other
}

fn compare_activity(left: &Activity, right: &Activity) -> Ordering {
    right
        .activity_type
        .priority()
        .cmp(&left.activity_type.priority())
        .then_with(|| {
            right
                .has_displayable_content()
                .cmp(&left.has_displayable_content())
        })
        .then_with(|| right.created_at.cmp(&left.created_at))
}

#[cfg(test)]
mod tests {
    use super::{Activity, ActivityType, PresenceStatus};

    #[test]
    fn status_and_custom_activity_helpers() {
        assert_eq!(PresenceStatus::from_wire("online"), PresenceStatus::Online);
        assert_eq!(
            PresenceStatus::from_wire("away"),
            PresenceStatus::Unknown("away".to_string())
        );
        assert!(PresenceStatus::Idle.is_online());
        assert!(!PresenceStatus::Offline.is_online());
        assert_eq!(ActivityType::from_i64(4), ActivityType::Custom);
        assert_eq!(ActivityType::Unknown(9).as_i64(), 9);
        assert!(
            Activity {
                activity_type: ActivityType::Custom,
                name: "custom".to_string(),
                ..Activity::default()
            }
            .activity_type
            .is_custom()
        );
    }
}
