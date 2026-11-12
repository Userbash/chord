//! # Роутер диспатчей шлюза (RW-07, P1-06 / T-GW-08)
//!
//! op0-диспатчи → типизированные события по 21 наблюдаемому типу
//! ([`DispatchKind`], F-GW-06, `gateway-protocol.md` §4); неизвестный тип — не ошибка:
//! событие с `kind: None` (raw) и сохранённым именем, а накопительный счётчик ведёт пайп
//! ([`crate::ws::GatewayPipe::unknown_dispatches`]). Payload `d` не разбирается — это волна
//! P2-01 (T-DATA-01) — и передаётся вызывающему целиком.
//!
//! Оракул — union семи снимков `gw-proto/msgs-*.jsonl`: 1340 op0, 21 тип.

use std::fmt;

use chord_codec::etf::EtfValue;
use chord_json::{JsonError, JsonValue};

/// 21 наблюдаемый тип op0 (F-GW-06, `gateway-protocol.md` §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DispatchKind {
    /// `CHANNEL_INFO`.
    ChannelInfo,
    /// `GUILD_MEMBERS_CHUNK`.
    GuildMembersChunk,
    /// `GUILD_MEMBER_LIST_UPDATE`.
    GuildMemberListUpdate,
    /// `MESSAGE_ACK`.
    MessageAck,
    /// `MESSAGE_CREATE`.
    MessageCreate,
    /// `MESSAGE_POLL_VOTE_ADD`.
    MessagePollVoteAdd,
    /// `MESSAGE_REACTION_ADD`.
    MessageReactionAdd,
    /// `MESSAGE_UPDATE`.
    MessageUpdate,
    /// `PRESENCE_UPDATE`.
    PresenceUpdate,
    /// `READY`.
    Ready,
    /// `READY_SUPPLEMENTAL`.
    ReadySupplemental,
    /// `RESUMED`.
    Resumed,
    /// `SESSIONS_REPLACE`.
    SessionsReplace,
    /// `SOUNDBOARD_SOUNDS`.
    SoundboardSounds,
    /// `THREAD_LIST_SYNC`.
    ThreadListSync,
    /// `TYPING_START`.
    TypingStart,
    /// `USER_SETTINGS_PROTO_UPDATE`.
    UserSettingsProtoUpdate,
    /// `VOICE_CHANNEL_START_TIME_UPDATE`.
    VoiceChannelStartTimeUpdate,
    /// `VOICE_CHANNEL_STATUS_UPDATE`.
    VoiceChannelStatusUpdate,
    /// `VOICE_SERVER_UPDATE`.
    VoiceServerUpdate,
    /// `VOICE_STATE_UPDATE`.
    VoiceStateUpdate,
}

impl DispatchKind {
    /// Все 21; каждый ровно один раз (порядок — по имени провода, как в таблице протокола).
    pub const ALL: [DispatchKind; 21] = [
        DispatchKind::ChannelInfo,
        DispatchKind::GuildMembersChunk,
        DispatchKind::GuildMemberListUpdate,
        DispatchKind::MessageAck,
        DispatchKind::MessageCreate,
        DispatchKind::MessagePollVoteAdd,
        DispatchKind::MessageReactionAdd,
        DispatchKind::MessageUpdate,
        DispatchKind::PresenceUpdate,
        DispatchKind::Ready,
        DispatchKind::ReadySupplemental,
        DispatchKind::Resumed,
        DispatchKind::SessionsReplace,
        DispatchKind::SoundboardSounds,
        DispatchKind::ThreadListSync,
        DispatchKind::TypingStart,
        DispatchKind::UserSettingsProtoUpdate,
        DispatchKind::VoiceChannelStartTimeUpdate,
        DispatchKind::VoiceChannelStatusUpdate,
        DispatchKind::VoiceServerUpdate,
        DispatchKind::VoiceStateUpdate,
    ];

    /// Точное совпадение (регистрозависимо), `name` — имя провода.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|kind| kind.as_str() == name)
    }

    /// Имя типа в проводе (`"MESSAGE_CREATE"`, `"READY"`, …).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ChannelInfo => "CHANNEL_INFO",
            Self::GuildMembersChunk => "GUILD_MEMBERS_CHUNK",
            Self::GuildMemberListUpdate => "GUILD_MEMBER_LIST_UPDATE",
            Self::MessageAck => "MESSAGE_ACK",
            Self::MessageCreate => "MESSAGE_CREATE",
            Self::MessagePollVoteAdd => "MESSAGE_POLL_VOTE_ADD",
            Self::MessageReactionAdd => "MESSAGE_REACTION_ADD",
            Self::MessageUpdate => "MESSAGE_UPDATE",
            Self::PresenceUpdate => "PRESENCE_UPDATE",
            Self::Ready => "READY",
            Self::ReadySupplemental => "READY_SUPPLEMENTAL",
            Self::Resumed => "RESUMED",
            Self::SessionsReplace => "SESSIONS_REPLACE",
            Self::SoundboardSounds => "SOUNDBOARD_SOUNDS",
            Self::ThreadListSync => "THREAD_LIST_SYNC",
            Self::TypingStart => "TYPING_START",
            Self::UserSettingsProtoUpdate => "USER_SETTINGS_PROTO_UPDATE",
            Self::VoiceChannelStartTimeUpdate => "VOICE_CHANNEL_START_TIME_UPDATE",
            Self::VoiceChannelStatusUpdate => "VOICE_CHANNEL_STATUS_UPDATE",
            Self::VoiceServerUpdate => "VOICE_SERVER_UPDATE",
            Self::VoiceStateUpdate => "VOICE_STATE_UPDATE",
        }
    }
}

/// 8 событий удаления сообщений и жизненного цикла каналов/веток (task-146, F-GW-03/F-GW-04).
///
/// В снимке R7 (1340 op0, 21 тип [`DispatchKind::ALL`]) эти события не встречались,
/// поэтому [`DispatchKind::ALL`] сохраняет ровно 21 элемент для обратной совместимости,
/// а типизированный разбор `MESSAGE_DELETE*`, `CHANNEL_*`, `THREAD_*` выделен в
/// [`ExtendedDispatchKind`] и [`TypedDispatch`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExtendedDispatchKind {
    /// `MESSAGE_DELETE`.
    MessageDelete,
    /// `MESSAGE_DELETE_BULK`.
    MessageDeleteBulk,
    /// `CHANNEL_CREATE`.
    ChannelCreate,
    /// `CHANNEL_UPDATE`.
    ChannelUpdate,
    /// `CHANNEL_DELETE`.
    ChannelDelete,
    /// `THREAD_CREATE`.
    ThreadCreate,
    /// `THREAD_UPDATE`.
    ThreadUpdate,
    /// `THREAD_DELETE`.
    ThreadDelete,
}

/// Алиас для [`ExtendedDispatchKind`].
pub type LifecycleDispatchKind = ExtendedDispatchKind;

impl ExtendedDispatchKind {
    /// Все 8 типов удаления сообщений и жизненного цикла каналов/веток.
    pub const ALL: [ExtendedDispatchKind; 8] = [
        Self::MessageDelete,
        Self::MessageDeleteBulk,
        Self::ChannelCreate,
        Self::ChannelUpdate,
        Self::ChannelDelete,
        Self::ThreadCreate,
        Self::ThreadUpdate,
        Self::ThreadDelete,
    ];

    /// Канонические имена провода для всех 8 типов.
    pub const NAMES: [&'static str; 8] = [
        "MESSAGE_DELETE",
        "MESSAGE_DELETE_BULK",
        "CHANNEL_CREATE",
        "CHANNEL_UPDATE",
        "CHANNEL_DELETE",
        "THREAD_CREATE",
        "THREAD_UPDATE",
        "THREAD_DELETE",
    ];

    /// Точное регистрозависимое сопоставление имени провода `t`.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "MESSAGE_DELETE" => Some(Self::MessageDelete),
            "MESSAGE_DELETE_BULK" => Some(Self::MessageDeleteBulk),
            "CHANNEL_CREATE" => Some(Self::ChannelCreate),
            "CHANNEL_UPDATE" => Some(Self::ChannelUpdate),
            "CHANNEL_DELETE" => Some(Self::ChannelDelete),
            "THREAD_CREATE" => Some(Self::ThreadCreate),
            "THREAD_UPDATE" => Some(Self::ThreadUpdate),
            "THREAD_DELETE" => Some(Self::ThreadDelete),
            _ => None,
        }
    }

    /// Имя события в проводе (`"MESSAGE_DELETE"`, `"CHANNEL_CREATE"`, …).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MessageDelete => "MESSAGE_DELETE",
            Self::MessageDeleteBulk => "MESSAGE_DELETE_BULK",
            Self::ChannelCreate => "CHANNEL_CREATE",
            Self::ChannelUpdate => "CHANNEL_UPDATE",
            Self::ChannelDelete => "CHANNEL_DELETE",
            Self::ThreadCreate => "THREAD_CREATE",
            Self::ThreadUpdate => "THREAD_UPDATE",
            Self::ThreadDelete => "THREAD_DELETE",
        }
    }

    /// Относится ли событие к удалению сообщений (`MESSAGE_DELETE`, `MESSAGE_DELETE_BULK`).
    #[must_use]
    pub fn is_message_delete(self) -> bool {
        matches!(self, Self::MessageDelete | Self::MessageDeleteBulk)
    }

    /// Относится ли событие к жизненному циклу каналов (`CHANNEL_CREATE`, `CHANNEL_UPDATE`, `CHANNEL_DELETE`).
    #[must_use]
    pub fn is_channel_lifecycle(self) -> bool {
        matches!(
            self,
            Self::ChannelCreate | Self::ChannelUpdate | Self::ChannelDelete
        )
    }

    /// Относится ли событие к жизненному циклу веток (`THREAD_CREATE`, `THREAD_UPDATE`, `THREAD_DELETE`).
    #[must_use]
    pub fn is_thread_lifecycle(self) -> bool {
        matches!(
            self,
            Self::ThreadCreate | Self::ThreadUpdate | Self::ThreadDelete
        )
    }
}

/// Типизированная дельта `MESSAGE_DELETE` (`@4923794` в бандле Discord 1.0.162).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageDeleteDispatch {
    /// Snowflake ID удалённого сообщения (`d.id`).
    pub id: u64,
    /// Snowflake ID канала (`d.channel_id`).
    pub channel_id: u64,
    /// Snowflake ID гильдии (`d.guild_id`; `None` для личных сообщений).
    pub guild_id: Option<u64>,
}

/// Алиас для [`MessageDeleteDispatch`].
pub type MessageDeleteDelta = MessageDeleteDispatch;

/// Типизированная дельта `MESSAGE_DELETE_BULK` (`@4923864` в бандле Discord 1.0.162).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageDeleteBulkDispatch {
    /// Непустой список Snowflake ID удалённых сообщений (`d.ids`) в порядке провода.
    pub ids: Vec<u64>,
    /// Snowflake ID канала (`d.channel_id`).
    pub channel_id: u64,
    /// Snowflake ID гильдии (`d.guild_id`; `None` для личных сообщений).
    pub guild_id: Option<u64>,
}

/// Алиас для [`MessageDeleteBulkDispatch`].
pub type MessageDeleteBulkDelta = MessageDeleteBulkDispatch;

/// Типизированная дельта `CHANNEL_CREATE`, `CHANNEL_UPDATE`, `CHANNEL_DELETE`
/// (`@4925216`, `@4925551`, `ed.UE` `@8897000..8903000` в бандле Discord 1.0.162).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelLifecycleDispatch {
    /// Snowflake ID канала (`d.id`).
    pub id: u64,
    /// Snowflake ID гильдии (`d.guild_id`; `None` для DM / Group DM).
    pub guild_id: Option<u64>,
    /// Числовой тип канала (`d.type` или `d.kind`: `0` = GUILD_TEXT, `1` = DM, `2` = GUILD_VOICE и т.д.).
    pub kind: Option<i64>,
    /// Имя канала (`d.name`; `None`, если отсутствует или `null`).
    pub name: Option<String>,
    /// Snowflake ID родительской категории (`d.parent_id`).
    pub parent_id: Option<u64>,
}

/// Алиас для [`ChannelLifecycleDispatch`].
pub type ChannelDispatchDelta = ChannelLifecycleDispatch;

impl ChannelLifecycleDispatch {
    /// Числовой тип канала (`d.type` / `d.kind`).
    #[must_use]
    pub fn channel_type(&self) -> Option<i64> {
        self.kind
    }
}

/// Типизированная дельта `THREAD_CREATE`, `THREAD_UPDATE`, `THREAD_DELETE`
/// (`@4925596`, `ed.UE` `@8902000` в бандле Discord 1.0.162).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadLifecycleDispatch {
    /// Snowflake ID ветки (`d.id`).
    pub id: u64,
    /// Snowflake ID гильдии (`d.guild_id`).
    pub guild_id: Option<u64>,
    /// Числовой тип ветки (`d.type` или `d.kind`: `10` = ANNOUNCEMENT_THREAD, `11` = PUBLIC_THREAD, `12` = PRIVATE_THREAD).
    pub kind: Option<i64>,
    /// Имя ветки (`d.name`; `None`, если отсутствует или `null`).
    pub name: Option<String>,
    /// Snowflake ID родительского канала (`d.parent_id`).
    pub parent_id: Option<u64>,
    /// Флаг только что созданной ветки (`d.newly_created` в `THREAD_CREATE`, `@4925596`).
    pub newly_created: Option<bool>,
}

/// Алиас для [`ThreadLifecycleDispatch`].
pub type ThreadDispatchDelta = ThreadLifecycleDispatch;

impl ThreadLifecycleDispatch {
    /// Числовой тип ветки (`d.type` / `d.kind`).
    #[must_use]
    pub fn thread_type(&self) -> Option<i64> {
        self.kind
    }

    /// Числовой тип канала/ветки (`d.type` / `d.kind`).
    #[must_use]
    pub fn channel_type(&self) -> Option<i64> {
        self.kind
    }
}

/// Типизированная полезная нагрузка одного из 8 событий [`ExtendedDispatchKind`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedDispatch {
    /// `MESSAGE_DELETE`.
    MessageDelete(MessageDeleteDispatch),
    /// `MESSAGE_DELETE_BULK`.
    MessageDeleteBulk(MessageDeleteBulkDispatch),
    /// `CHANNEL_CREATE`.
    ChannelCreate(ChannelLifecycleDispatch),
    /// `CHANNEL_UPDATE`.
    ChannelUpdate(ChannelLifecycleDispatch),
    /// `CHANNEL_DELETE`.
    ChannelDelete(ChannelLifecycleDispatch),
    /// `THREAD_CREATE`.
    ThreadCreate(ThreadLifecycleDispatch),
    /// `THREAD_UPDATE`.
    ThreadUpdate(ThreadLifecycleDispatch),
    /// `THREAD_DELETE`.
    ThreadDelete(ThreadLifecycleDispatch),
}

impl TypedDispatch {
    /// Вид типизированного события.
    #[must_use]
    pub fn kind(&self) -> ExtendedDispatchKind {
        match self {
            Self::MessageDelete(_) => ExtendedDispatchKind::MessageDelete,
            Self::MessageDeleteBulk(_) => ExtendedDispatchKind::MessageDeleteBulk,
            Self::ChannelCreate(_) => ExtendedDispatchKind::ChannelCreate,
            Self::ChannelUpdate(_) => ExtendedDispatchKind::ChannelUpdate,
            Self::ChannelDelete(_) => ExtendedDispatchKind::ChannelDelete,
            Self::ThreadCreate(_) => ExtendedDispatchKind::ThreadCreate,
            Self::ThreadUpdate(_) => ExtendedDispatchKind::ThreadUpdate,
            Self::ThreadDelete(_) => ExtendedDispatchKind::ThreadDelete,
        }
    }

    /// Каноническое имя события в проводе (`"MESSAGE_DELETE"`…`"THREAD_DELETE"`).
    #[must_use]
    pub fn event_name(&self) -> &'static str {
        self.kind().as_str()
    }

    /// Snowflake ID гильдии (если присутствует в дельте).
    #[must_use]
    pub fn guild_id(&self) -> Option<u64> {
        match self {
            Self::MessageDelete(d) => d.guild_id,
            Self::MessageDeleteBulk(d) => d.guild_id,
            Self::ChannelCreate(d) | Self::ChannelUpdate(d) | Self::ChannelDelete(d) => d.guild_id,
            Self::ThreadCreate(d) | Self::ThreadUpdate(d) | Self::ThreadDelete(d) => d.guild_id,
        }
    }

    /// Snowflake ID целевого канала (для `MESSAGE_DELETE*` и `CHANNEL_*`) или ветки (для `THREAD_*`).
    #[must_use]
    pub fn target_channel_id(&self) -> u64 {
        match self {
            Self::MessageDelete(d) => d.channel_id,
            Self::MessageDeleteBulk(d) => d.channel_id,
            Self::ChannelCreate(d) | Self::ChannelUpdate(d) | Self::ChannelDelete(d) => d.id,
            Self::ThreadCreate(d) | Self::ThreadUpdate(d) | Self::ThreadDelete(d) => d.id,
        }
    }
}

/// Маршрутизированное типизированное событие op0 из [`ExtendedDispatchKind::ALL`] с сохранённым `seq` (`s`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutedTypedDispatch {
    /// Вид события (`MESSAGE_DELETE`…`THREAD_DELETE`).
    pub kind: ExtendedDispatchKind,
    /// Номер последовательности `s` (`None`, если поля нет либо не `u64`).
    pub seq: Option<u64>,
    /// Разобранная полезная нагрузка [`TypedDispatch`].
    pub payload: TypedDispatch,
}

/// Ошибка типизированного разбора конвертов `MESSAGE_DELETE*`, `CHANNEL_*`, `THREAD_*`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchParseError {
    /// Синтаксическая ошибка входного JSON-конверта.
    InvalidJson(JsonError),
    /// Конверт или полезная нагрузка `d` не является объектом/картой.
    InvalidEnvelope {
        /// Описание нарушения структуры конверта.
        reason: &'static str,
    },
    /// Отсутствует обязательное поле в `d`.
    MissingField {
        /// Имя события в проводе (`"MESSAGE_DELETE"` и т.д.).
        event: &'static str,
        /// Имя обязательного поля (`"id"`, `"channel_id"`, `"ids"`).
        field: &'static str,
    },
    /// Поле имеет неподходящий тип.
    InvalidFieldType {
        /// Имя события в проводе.
        event: &'static str,
        /// Путь к полю.
        field: String,
        /// Ожидаемый тип.
        expected: &'static str,
        /// Фактический тип.
        found: &'static str,
    },
    /// Поле Snowflake ID не является точным 64-битным беззнаковым целым (`u64`).
    InvalidSnowflake {
        /// Имя события в проводе.
        event: &'static str,
        /// Путь к полю (`"id"`, `"channel_id"`, `"guild_id"`, `"parent_id"`, `"ids[0]"`).
        field: String,
        /// Текстовое представление отвергнутого значения.
        value: String,
    },
    /// Список `d.ids` в `MESSAGE_DELETE_BULK` пуст.
    EmptyBulkIds {
        /// Имя события (`"MESSAGE_DELETE_BULK"`).
        event: &'static str,
    },
}

impl fmt::Display for DispatchParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(err) => write!(f, "{err}"),
            Self::InvalidEnvelope { reason } => write!(f, "невалидный конверт dispatch: {reason}"),
            Self::MissingField { event, field } => {
                write!(f, "{event}: отсутствует обязательное поле {field}")
            }
            Self::InvalidFieldType {
                event,
                field,
                expected,
                found,
            } => write!(f, "{event}.{field}: ожидался {expected}, получен {found}"),
            Self::InvalidSnowflake {
                event,
                field,
                value,
            } => write!(f, "{event}.{field}: невалидный Snowflake ID {value:?}"),
            Self::EmptyBulkIds { event } => write!(f, "{event}.ids: список не может быть пустым"),
        }
    }
}

impl std::error::Error for DispatchParseError {}

impl From<JsonError> for DispatchParseError {
    fn from(err: JsonError) -> Self {
        Self::InvalidJson(err)
    }
}

/// Событие op0 после роутинга. Payload не парсится — в этом волна P1-06.
#[derive(Debug, Clone, PartialEq)]
pub struct DispatchEvent {
    /// `Some(kind)` — известный тип; `None` — неизвестный (raw-лог).
    pub kind: Option<DispatchKind>,
    /// Имя типа как пришло (для известных — `kind.as_str()`); не теряется и для raw.
    pub name: String,
    /// `s` диспатча; `None` — поля нет либо не Int ≥ 0.
    pub seq: Option<u64>,
    /// `d` целиком (клонирование — плата за владение; оптимизация не нужна).
    /// `d` отсутствует → `EtfValue::Nil`.
    pub data: EtfValue,
}

impl DispatchEvent {
    /// Если `name` относится к 8 типам удаления сообщений и жизненного цикла каналов/веток,
    /// возвращает [`ExtendedDispatchKind`].
    #[must_use]
    pub fn extended_kind(&self) -> Option<ExtendedDispatchKind> {
        ExtendedDispatchKind::from_name(&self.name)
    }

    /// Относится ли событие к 8 расширенным типам (`MESSAGE_DELETE*`, `CHANNEL_*`, `THREAD_*`).
    #[must_use]
    pub fn is_extended(&self) -> bool {
        self.extended_kind().is_some()
    }

    /// Является ли событие известным роутеру (21 живой тип [`DispatchKind`] либо 8 типов [`ExtendedDispatchKind`]).
    #[must_use]
    pub fn is_known(&self) -> bool {
        self.kind.is_some() || self.is_extended()
    }

    /// Разобрать типизированную дельту [`TypedDispatch`] из `self.data`, если `self.name` входит в [`ExtendedDispatchKind::ALL`].
    #[must_use]
    pub fn parse_typed(&self) -> Option<Result<TypedDispatch, DispatchParseError>> {
        parse_typed_dispatch_etf(&self.name, &self.data)
    }
}

/// op0 + строковый `t` → событие; иначе `None` (не диспатч).
///
/// Порядок шагов (контракт P1-06): `op == 0` строго целым → `t` только атом/строка →
/// вид по точному имени → `s` как `u64` (отрицательный/`"nil"`/отсутствие — `None`) →
/// `d` целиком (`Nil`, если поля нет). Неизвестный `t` — не ошибка: событие с `kind: None`.
pub fn route_dispatch(value: &EtfValue) -> Option<DispatchEvent> {
    if value.get("op").and_then(EtfValue::as_int) != Some(0) {
        return None;
    }
    let name = match value.get("t")? {
        EtfValue::Atom(name) | EtfValue::Str(name) => name.clone(),
        _ => return None,
    };
    let kind = DispatchKind::from_name(&name);
    let seq = value
        .get("s")
        .and_then(EtfValue::as_int)
        .and_then(|raw| u64::try_from(raw).ok());
    let data = value.get("d").cloned().unwrap_or(EtfValue::Nil);
    Some(DispatchEvent {
        kind,
        name,
        seq,
        data,
    })
}

fn etf_is_null(value: &EtfValue) -> bool {
    matches!(value, EtfValue::Nil) || matches!(value, EtfValue::Atom(name) if name == "nil")
}

fn etf_kind_name(value: &EtfValue) -> &'static str {
    match value {
        EtfValue::Nil => "null",
        EtfValue::Atom(_) => "atom",
        EtfValue::Int(_) => "int",
        EtfValue::Float(_) => "float",
        EtfValue::Str(_) => "str",
        EtfValue::Tuple(_) => "tuple",
        EtfValue::List(_) => "list",
        EtfValue::Map(_) => "map",
    }
}

fn etf_raw_display(value: &EtfValue) -> String {
    match value {
        EtfValue::Str(text) | EtfValue::Atom(text) => text.clone(),
        EtfValue::Int(number) => number.to_string(),
        EtfValue::Float(number) => number.to_string(),
        EtfValue::Nil => "null".to_string(),
        other => etf_kind_name(other).to_string(),
    }
}

fn etf_parse_snowflake(value: &EtfValue) -> Option<u64> {
    match value {
        EtfValue::Int(number) if *number >= 0 => u64::try_from(*number).ok(),
        EtfValue::Str(text) | EtfValue::Atom(text) => (!text.is_empty()
            && text.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| text.parse::<u64>().ok())
        .flatten(),
        _ => None,
    }
}

fn etf_require_map<'a>(
    event: &'static str,
    data: &'a EtfValue,
) -> Result<&'a EtfValue, DispatchParseError> {
    if data.as_map().is_none() {
        return Err(DispatchParseError::InvalidFieldType {
            event,
            field: "d".to_string(),
            expected: "map",
            found: etf_kind_name(data),
        });
    }
    Ok(data)
}

fn etf_required_snowflake(
    map: &EtfValue,
    event: &'static str,
    field: &'static str,
) -> Result<u64, DispatchParseError> {
    let raw = map
        .get(field)
        .ok_or(DispatchParseError::MissingField { event, field })?;
    if etf_is_null(raw) {
        return Err(DispatchParseError::MissingField { event, field });
    }
    etf_parse_snowflake(raw).ok_or_else(|| DispatchParseError::InvalidSnowflake {
        event,
        field: field.to_string(),
        value: etf_raw_display(raw),
    })
}

fn etf_optional_snowflake(
    map: &EtfValue,
    event: &'static str,
    field: &'static str,
) -> Result<Option<u64>, DispatchParseError> {
    let Some(raw) = map.get(field) else {
        return Ok(None);
    };
    if etf_is_null(raw) {
        return Ok(None);
    }
    etf_parse_snowflake(raw)
        .map(Some)
        .ok_or_else(|| DispatchParseError::InvalidSnowflake {
            event,
            field: field.to_string(),
            value: etf_raw_display(raw),
        })
}

fn etf_optional_int(
    map: &EtfValue,
    event: &'static str,
    field: &'static str,
) -> Result<Option<i64>, DispatchParseError> {
    let Some(raw) = map.get(field) else {
        return Ok(None);
    };
    if etf_is_null(raw) {
        return Ok(None);
    }
    match raw {
        EtfValue::Int(number) => Ok(Some(*number)),
        _ => Err(DispatchParseError::InvalidFieldType {
            event,
            field: field.to_string(),
            expected: "int",
            found: etf_kind_name(raw),
        }),
    }
}

fn etf_optional_kind(
    map: &EtfValue,
    event: &'static str,
) -> Result<Option<i64>, DispatchParseError> {
    if let Some(kind) = etf_optional_int(map, event, "type")? {
        return Ok(Some(kind));
    }
    etf_optional_int(map, event, "kind")
}

fn etf_optional_string(
    map: &EtfValue,
    event: &'static str,
    field: &'static str,
) -> Result<Option<String>, DispatchParseError> {
    let Some(raw) = map.get(field) else {
        return Ok(None);
    };
    if etf_is_null(raw) {
        return Ok(None);
    }
    match raw {
        EtfValue::Str(text) => Ok(Some(text.clone())),
        EtfValue::Atom(name) if !matches!(name.as_str(), "nil" | "true" | "false") => {
            Ok(Some(name.clone()))
        }
        _ => Err(DispatchParseError::InvalidFieldType {
            event,
            field: field.to_string(),
            expected: "str",
            found: etf_kind_name(raw),
        }),
    }
}

fn etf_optional_bool(
    map: &EtfValue,
    event: &'static str,
    field: &'static str,
) -> Result<Option<bool>, DispatchParseError> {
    let Some(raw) = map.get(field) else {
        return Ok(None);
    };
    if etf_is_null(raw) {
        return Ok(None);
    }
    match raw {
        EtfValue::Atom(name) | EtfValue::Str(name) if name == "true" => Ok(Some(true)),
        EtfValue::Atom(name) | EtfValue::Str(name) if name == "false" => Ok(Some(false)),
        EtfValue::Int(1) => Ok(Some(true)),
        EtfValue::Int(0) => Ok(Some(false)),
        _ => Err(DispatchParseError::InvalidFieldType {
            event,
            field: field.to_string(),
            expected: "bool",
            found: etf_kind_name(raw),
        }),
    }
}

fn parse_message_delete_etf(data: &EtfValue) -> Result<MessageDeleteDispatch, DispatchParseError> {
    const EVENT: &str = "MESSAGE_DELETE";
    let map = etf_require_map(EVENT, data)?;
    let id = etf_required_snowflake(map, EVENT, "id")?;
    let channel_id = etf_required_snowflake(map, EVENT, "channel_id")?;
    let guild_id = etf_optional_snowflake(map, EVENT, "guild_id")?;
    Ok(MessageDeleteDispatch {
        id,
        channel_id,
        guild_id,
    })
}

fn parse_message_delete_bulk_etf(
    data: &EtfValue,
) -> Result<MessageDeleteBulkDispatch, DispatchParseError> {
    const EVENT: &str = "MESSAGE_DELETE_BULK";
    let map = etf_require_map(EVENT, data)?;
    let raw_ids = map.get("ids").ok_or(DispatchParseError::MissingField {
        event: EVENT,
        field: "ids",
    })?;
    let items = match raw_ids {
        EtfValue::List(items) | EtfValue::Tuple(items) => items.as_slice(),
        _ => {
            return Err(DispatchParseError::InvalidFieldType {
                event: EVENT,
                field: "ids".to_string(),
                expected: "list",
                found: etf_kind_name(raw_ids),
            });
        }
    };
    if items.is_empty() {
        return Err(DispatchParseError::EmptyBulkIds { event: EVENT });
    }
    let mut ids = Vec::with_capacity(items.len());
    for (idx, item) in items.iter().enumerate() {
        let id = etf_parse_snowflake(item).ok_or_else(|| DispatchParseError::InvalidSnowflake {
            event: EVENT,
            field: format!("ids[{idx}]"),
            value: etf_raw_display(item),
        })?;
        ids.push(id);
    }
    let channel_id = etf_required_snowflake(map, EVENT, "channel_id")?;
    let guild_id = etf_optional_snowflake(map, EVENT, "guild_id")?;
    Ok(MessageDeleteBulkDispatch {
        ids,
        channel_id,
        guild_id,
    })
}

fn parse_channel_lifecycle_etf(
    event: &'static str,
    data: &EtfValue,
) -> Result<ChannelLifecycleDispatch, DispatchParseError> {
    let map = etf_require_map(event, data)?;
    let id = etf_required_snowflake(map, event, "id")?;
    let guild_id = etf_optional_snowflake(map, event, "guild_id")?;
    let kind = etf_optional_kind(map, event)?;
    let name = etf_optional_string(map, event, "name")?;
    let parent_id = etf_optional_snowflake(map, event, "parent_id")?;
    Ok(ChannelLifecycleDispatch {
        id,
        guild_id,
        kind,
        name,
        parent_id,
    })
}

fn parse_thread_lifecycle_etf(
    event: &'static str,
    data: &EtfValue,
) -> Result<ThreadLifecycleDispatch, DispatchParseError> {
    let map = etf_require_map(event, data)?;
    let id = etf_required_snowflake(map, event, "id")?;
    let guild_id = etf_optional_snowflake(map, event, "guild_id")?;
    let kind = etf_optional_kind(map, event)?;
    let name = etf_optional_string(map, event, "name")?;
    let parent_id = etf_optional_snowflake(map, event, "parent_id")?;
    let newly_created = etf_optional_bool(map, event, "newly_created")?;
    Ok(ThreadLifecycleDispatch {
        id,
        guild_id,
        kind,
        name,
        parent_id,
        newly_created,
    })
}

fn parse_typed_by_kind_etf(
    kind: ExtendedDispatchKind,
    data: &EtfValue,
) -> Result<TypedDispatch, DispatchParseError> {
    let event = kind.as_str();
    match kind {
        ExtendedDispatchKind::MessageDelete => {
            parse_message_delete_etf(data).map(TypedDispatch::MessageDelete)
        }
        ExtendedDispatchKind::MessageDeleteBulk => {
            parse_message_delete_bulk_etf(data).map(TypedDispatch::MessageDeleteBulk)
        }
        ExtendedDispatchKind::ChannelCreate => {
            parse_channel_lifecycle_etf(event, data).map(TypedDispatch::ChannelCreate)
        }
        ExtendedDispatchKind::ChannelUpdate => {
            parse_channel_lifecycle_etf(event, data).map(TypedDispatch::ChannelUpdate)
        }
        ExtendedDispatchKind::ChannelDelete => {
            parse_channel_lifecycle_etf(event, data).map(TypedDispatch::ChannelDelete)
        }
        ExtendedDispatchKind::ThreadCreate => {
            parse_thread_lifecycle_etf(event, data).map(TypedDispatch::ThreadCreate)
        }
        ExtendedDispatchKind::ThreadUpdate => {
            parse_thread_lifecycle_etf(event, data).map(TypedDispatch::ThreadUpdate)
        }
        ExtendedDispatchKind::ThreadDelete => {
            parse_thread_lifecycle_etf(event, data).map(TypedDispatch::ThreadDelete)
        }
    }
}

fn json_kind_name(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "bool",
        JsonValue::Number(_) => "number",
        JsonValue::String(_) => "str",
        JsonValue::Array(_) => "list",
        JsonValue::Object(_) => "map",
    }
}

fn json_raw_display(value: &JsonValue) -> String {
    match value {
        JsonValue::String(text) => text.clone(),
        JsonValue::Number(number) => number.as_str().to_string(),
        JsonValue::Bool(flag) => flag.to_string(),
        JsonValue::Null => "null".to_string(),
        other => json_kind_name(other).to_string(),
    }
}

fn json_parse_snowflake(value: &JsonValue) -> Option<u64> {
    match value {
        JsonValue::Number(number) => number.as_u64(),
        JsonValue::String(text) => (!text.is_empty()
            && text.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| text.parse::<u64>().ok())
        .flatten(),
        _ => None,
    }
}

fn json_require_object<'a>(
    event: &'static str,
    data: &'a JsonValue,
) -> Result<&'a JsonValue, DispatchParseError> {
    if data.as_object().is_none() {
        return Err(DispatchParseError::InvalidFieldType {
            event,
            field: "d".to_string(),
            expected: "map",
            found: json_kind_name(data),
        });
    }
    Ok(data)
}

fn json_required_snowflake(
    obj: &JsonValue,
    event: &'static str,
    field: &'static str,
) -> Result<u64, DispatchParseError> {
    let raw = obj
        .get(field)
        .ok_or(DispatchParseError::MissingField { event, field })?;
    if raw.is_null() {
        return Err(DispatchParseError::MissingField { event, field });
    }
    json_parse_snowflake(raw).ok_or_else(|| DispatchParseError::InvalidSnowflake {
        event,
        field: field.to_string(),
        value: json_raw_display(raw),
    })
}

fn json_optional_snowflake(
    obj: &JsonValue,
    event: &'static str,
    field: &'static str,
) -> Result<Option<u64>, DispatchParseError> {
    let Some(raw) = obj.get(field) else {
        return Ok(None);
    };
    if raw.is_null() {
        return Ok(None);
    }
    json_parse_snowflake(raw)
        .map(Some)
        .ok_or_else(|| DispatchParseError::InvalidSnowflake {
            event,
            field: field.to_string(),
            value: json_raw_display(raw),
        })
}

fn json_optional_int(
    obj: &JsonValue,
    event: &'static str,
    field: &'static str,
) -> Result<Option<i64>, DispatchParseError> {
    let Some(raw) = obj.get(field) else {
        return Ok(None);
    };
    if raw.is_null() {
        return Ok(None);
    }
    raw.as_i64()
        .map(Some)
        .ok_or_else(|| DispatchParseError::InvalidFieldType {
            event,
            field: field.to_string(),
            expected: "int",
            found: json_kind_name(raw),
        })
}

fn json_optional_kind(
    obj: &JsonValue,
    event: &'static str,
) -> Result<Option<i64>, DispatchParseError> {
    if let Some(kind) = json_optional_int(obj, event, "type")? {
        return Ok(Some(kind));
    }
    json_optional_int(obj, event, "kind")
}

fn json_optional_string(
    obj: &JsonValue,
    event: &'static str,
    field: &'static str,
) -> Result<Option<String>, DispatchParseError> {
    let Some(raw) = obj.get(field) else {
        return Ok(None);
    };
    if raw.is_null() {
        return Ok(None);
    }
    raw.as_str()
        .map(|s| Some(s.to_string()))
        .ok_or_else(|| DispatchParseError::InvalidFieldType {
            event,
            field: field.to_string(),
            expected: "str",
            found: json_kind_name(raw),
        })
}

fn json_optional_bool(
    obj: &JsonValue,
    event: &'static str,
    field: &'static str,
) -> Result<Option<bool>, DispatchParseError> {
    let Some(raw) = obj.get(field) else {
        return Ok(None);
    };
    if raw.is_null() {
        return Ok(None);
    }
    raw.as_bool()
        .map(Some)
        .ok_or_else(|| DispatchParseError::InvalidFieldType {
            event,
            field: field.to_string(),
            expected: "bool",
            found: json_kind_name(raw),
        })
}

fn parse_message_delete_json(
    data: &JsonValue,
) -> Result<MessageDeleteDispatch, DispatchParseError> {
    const EVENT: &str = "MESSAGE_DELETE";
    let obj = json_require_object(EVENT, data)?;
    let id = json_required_snowflake(obj, EVENT, "id")?;
    let channel_id = json_required_snowflake(obj, EVENT, "channel_id")?;
    let guild_id = json_optional_snowflake(obj, EVENT, "guild_id")?;
    Ok(MessageDeleteDispatch {
        id,
        channel_id,
        guild_id,
    })
}

fn parse_message_delete_bulk_json(
    data: &JsonValue,
) -> Result<MessageDeleteBulkDispatch, DispatchParseError> {
    const EVENT: &str = "MESSAGE_DELETE_BULK";
    let obj = json_require_object(EVENT, data)?;
    let raw_ids = obj.get("ids").ok_or(DispatchParseError::MissingField {
        event: EVENT,
        field: "ids",
    })?;
    let items = raw_ids
        .as_array()
        .ok_or_else(|| DispatchParseError::InvalidFieldType {
            event: EVENT,
            field: "ids".to_string(),
            expected: "list",
            found: json_kind_name(raw_ids),
        })?;
    if items.is_empty() {
        return Err(DispatchParseError::EmptyBulkIds { event: EVENT });
    }
    let mut ids = Vec::with_capacity(items.len());
    for (idx, item) in items.iter().enumerate() {
        let id =
            json_parse_snowflake(item).ok_or_else(|| DispatchParseError::InvalidSnowflake {
                event: EVENT,
                field: format!("ids[{idx}]"),
                value: json_raw_display(item),
            })?;
        ids.push(id);
    }
    let channel_id = json_required_snowflake(obj, EVENT, "channel_id")?;
    let guild_id = json_optional_snowflake(obj, EVENT, "guild_id")?;
    Ok(MessageDeleteBulkDispatch {
        ids,
        channel_id,
        guild_id,
    })
}

fn parse_channel_lifecycle_json(
    event: &'static str,
    data: &JsonValue,
) -> Result<ChannelLifecycleDispatch, DispatchParseError> {
    let obj = json_require_object(event, data)?;
    let id = json_required_snowflake(obj, event, "id")?;
    let guild_id = json_optional_snowflake(obj, event, "guild_id")?;
    let kind = json_optional_kind(obj, event)?;
    let name = json_optional_string(obj, event, "name")?;
    let parent_id = json_optional_snowflake(obj, event, "parent_id")?;
    Ok(ChannelLifecycleDispatch {
        id,
        guild_id,
        kind,
        name,
        parent_id,
    })
}

fn parse_thread_lifecycle_json(
    event: &'static str,
    data: &JsonValue,
) -> Result<ThreadLifecycleDispatch, DispatchParseError> {
    let obj = json_require_object(event, data)?;
    let id = json_required_snowflake(obj, event, "id")?;
    let guild_id = json_optional_snowflake(obj, event, "guild_id")?;
    let kind = json_optional_kind(obj, event)?;
    let name = json_optional_string(obj, event, "name")?;
    let parent_id = json_optional_snowflake(obj, event, "parent_id")?;
    let newly_created = json_optional_bool(obj, event, "newly_created")?;
    Ok(ThreadLifecycleDispatch {
        id,
        guild_id,
        kind,
        name,
        parent_id,
        newly_created,
    })
}

fn parse_typed_by_kind_json(
    kind: ExtendedDispatchKind,
    data: &JsonValue,
) -> Result<TypedDispatch, DispatchParseError> {
    let event = kind.as_str();
    match kind {
        ExtendedDispatchKind::MessageDelete => {
            parse_message_delete_json(data).map(TypedDispatch::MessageDelete)
        }
        ExtendedDispatchKind::MessageDeleteBulk => {
            parse_message_delete_bulk_json(data).map(TypedDispatch::MessageDeleteBulk)
        }
        ExtendedDispatchKind::ChannelCreate => {
            parse_channel_lifecycle_json(event, data).map(TypedDispatch::ChannelCreate)
        }
        ExtendedDispatchKind::ChannelUpdate => {
            parse_channel_lifecycle_json(event, data).map(TypedDispatch::ChannelUpdate)
        }
        ExtendedDispatchKind::ChannelDelete => {
            parse_channel_lifecycle_json(event, data).map(TypedDispatch::ChannelDelete)
        }
        ExtendedDispatchKind::ThreadCreate => {
            parse_thread_lifecycle_json(event, data).map(TypedDispatch::ThreadCreate)
        }
        ExtendedDispatchKind::ThreadUpdate => {
            parse_thread_lifecycle_json(event, data).map(TypedDispatch::ThreadUpdate)
        }
        ExtendedDispatchKind::ThreadDelete => {
            parse_thread_lifecycle_json(event, data).map(TypedDispatch::ThreadDelete)
        }
    }
}

/// Разобрать полезную нагрузку `d` (ETF) по имени события `name` из [`ExtendedDispatchKind::ALL`].
///
/// Возвращает `None`, если `name` не относится к 8 типам удаления сообщений и жизненного цикла каналов/веток.
#[must_use]
pub fn parse_typed_dispatch_etf(
    name: &str,
    data: &EtfValue,
) -> Option<Result<TypedDispatch, DispatchParseError>> {
    let kind = ExtendedDispatchKind::from_name(name)?;
    Some(parse_typed_by_kind_etf(kind, data))
}

/// Разобрать полезную нагрузку `d` (JSON) по имени события `name` из [`ExtendedDispatchKind::ALL`].
///
/// Возвращает `None`, если `name` не относится к 8 типам удаления сообщений и жизненного цикла каналов/веток.
#[must_use]
pub fn parse_typed_dispatch_json(
    name: &str,
    data: &JsonValue,
) -> Option<Result<TypedDispatch, DispatchParseError>> {
    let kind = ExtendedDispatchKind::from_name(name)?;
    Some(parse_typed_by_kind_json(kind, data))
}

/// Маршрутизировать `op == 0` ETF-конверт и разобрать его в [`RoutedTypedDispatch`],
/// если `t` принадлежит [`ExtendedDispatchKind::ALL`].
#[must_use]
pub fn route_typed_dispatch(
    value: &EtfValue,
) -> Option<Result<RoutedTypedDispatch, DispatchParseError>> {
    let event = route_dispatch(value)?;
    let kind = event.extended_kind()?;
    Some(
        parse_typed_by_kind_etf(kind, &event.data).map(|payload| RoutedTypedDispatch {
            kind,
            seq: event.seq,
            payload,
        }),
    )
}

/// Маршрутизировать JSON-конверт (`{"op":0,"t":...,"s":...,"d":...}` или `{"kind":...,"data":...}`)
/// и разобрать его в [`RoutedTypedDispatch`], если тип принадлежит [`ExtendedDispatchKind::ALL`].
#[must_use]
pub fn route_typed_dispatch_json_value(
    value: &JsonValue,
) -> Option<Result<RoutedTypedDispatch, DispatchParseError>> {
    let obj = value.as_object()?;
    let (name, seq, data) = if let Some(op_val) = obj.get("op") {
        if op_val.as_i64() != Some(0) {
            return None;
        }
        let name = obj.get("t")?.as_str()?;
        let seq = obj.get("s").and_then(JsonValue::as_u64);
        let data = obj.get("d").unwrap_or(&JsonValue::Null);
        (name, seq, data)
    } else {
        let name = obj.get("kind")?.as_str()?;
        let seq = obj
            .get("s")
            .or_else(|| obj.get("seq"))
            .and_then(JsonValue::as_u64);
        let data = obj.get("data").unwrap_or(&JsonValue::Null);
        (name, seq, data)
    };
    let kind = ExtendedDispatchKind::from_name(name)?;
    Some(
        parse_typed_by_kind_json(kind, data).map(|payload| RoutedTypedDispatch {
            kind,
            seq,
            payload,
        }),
    )
}

/// Разобрать строковый JSON-конверт (`{"op":0,"t":...,"s":...,"d":...}` или `{"kind":...,"data":...}`)
/// в [`RoutedTypedDispatch`] без потери точности 64-битных Snowflake ID.
pub fn route_typed_dispatch_json(
    input: &str,
) -> Result<Option<RoutedTypedDispatch>, DispatchParseError> {
    let value = chord_json::parse(input)?;
    match route_typed_dispatch_json_value(&value) {
        None => Ok(None),
        Some(Ok(routed)) => Ok(Some(routed)),
        Some(Err(err)) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chord_codec::etf;
    use chord_json::parse as parse_json;
    use std::collections::BTreeSet;
    use std::fs;
    use std::io::ErrorKind;
    use std::path::PathBuf;

    fn lab_root() -> PathBuf {
        PathBuf::from(
            std::env::var("RE_LAB_ROOT").unwrap_or_else(|_| "/home/sanya/re-deep".to_string()),
        )
    }

    fn unhex(hex: &str) -> Vec<u8> {
        let compact: String = hex.chars().filter(|c| !c.is_whitespace()).collect();
        assert_eq!(compact.len() % 2, 0, "нечётная длина hex: {hex}");
        (0..compact.len() / 2)
            .map(|i| u8::from_str_radix(&compact[2 * i..2 * i + 2], 16).expect("валидный hex"))
            .collect()
    }

    /// Декодирует `SMALL_BIG_EXT`-фрагмент из `gateway-protocol.json` `idChecks` (с префиксом `0x83`).
    fn decode_etf_term_body(hex_without_version: &str) -> EtfValue {
        let mut bytes = vec![0x83];
        bytes.extend(unhex(hex_without_version));
        etf::decode(&bytes).expect("валидный ETF-терм из оракула").0
    }

    fn atom(s: &str) -> EtfValue {
        EtfValue::Atom(s.to_string())
    }

    fn str_val(s: &str) -> EtfValue {
        EtfValue::Str(s.to_string())
    }

    fn etf_map(pairs: Vec<(&str, EtfValue)>) -> EtfValue {
        EtfValue::Map(pairs.into_iter().map(|(k, v)| (atom(k), v)).collect())
    }

    fn etf_op0(t: &str, seq: Option<i64>, d: EtfValue) -> EtfValue {
        let mut pairs = vec![("op", EtfValue::Int(0)), ("t", atom(t))];
        if let Some(s) = seq {
            pairs.push(("s", EtfValue::Int(s)));
        }
        pairs.push(("d", d));
        etf_map(pairs)
    }

    #[test]
    fn canonical_21_dispatch_kinds_preserved_and_disjoint_from_8_extended_kinds() {
        // 1. Обратная совместимость: ровно 21 наблюдаемый тип R7 в DispatchKind::ALL.
        assert_eq!(DispatchKind::ALL.len(), 21);

        // 2. Все 8 типов удаления сообщений и жизненного цикла каналов/веток зарегистрированы.
        assert_eq!(ExtendedDispatchKind::ALL.len(), 8);
        assert_eq!(ExtendedDispatchKind::NAMES.len(), 8);

        let canonical_names: BTreeSet<&str> =
            DispatchKind::ALL.iter().map(|k| k.as_str()).collect();
        let extended_names: BTreeSet<&str> = ExtendedDispatchKind::ALL
            .iter()
            .map(|k| k.as_str())
            .collect();
        assert_eq!(canonical_names.len(), 21);
        assert_eq!(extended_names.len(), 8);
        assert!(
            canonical_names.is_disjoint(&extended_names),
            "21 канонический тип R7 и 8 расширенных типов не должны пересекаться"
        );

        for (idx, kind) in ExtendedDispatchKind::ALL.into_iter().enumerate() {
            let wire_name = kind.as_str();
            assert_eq!(ExtendedDispatchKind::NAMES[idx], wire_name);
            assert_eq!(
                ExtendedDispatchKind::from_name(wire_name),
                Some(kind),
                "from_name должен находить {wire_name}"
            );
            assert_eq!(
                DispatchKind::from_name(wire_name),
                None,
                "{wire_name} не входит в 21 тип R7"
            );
        }

        // Регистрозависимость и отказ на мусор.
        for invalid in [
            "message_delete",
            "message_delete_bulk",
            "channel_create",
            "thread_create",
            "MESSAGE_CREATE",
            "READY",
            "",
            "nil",
        ] {
            assert_eq!(ExtendedDispatchKind::from_name(invalid), None);
        }

        // 3. Независимый оракул: сверка с бандлом Discord 1.0.162 (skip-aware).
        let bundle_path = lab_root().join("out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js");
        match fs::read(&bundle_path) {
            Ok(bytes) => {
                for expected_anchor in [
                    &b"eH([\"MESSAGE_DELETE\"]"[..],
                    &b"eH([\"MESSAGE_DELETE_BULK\"]"[..],
                    &b"eH([\"CHANNEL_CREATE\",\"CHANNEL_DELETE\"]"[..],
                    &b"eH([\"CHANNEL_UPDATE\"]"[..],
                    &b"eH([\"THREAD_CREATE\",\"THREAD_UPDATE\",\"THREAD_DELETE\"]"[..],
                ] {
                    assert!(
                        bytes
                            .windows(expected_anchor.len())
                            .any(|w| w == expected_anchor),
                        "в бандле должен присутствовать якорь {:?}",
                        std::str::from_utf8(expected_anchor).unwrap_or("")
                    );
                }
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => panic!("ошибка чтения бандла {}: {err}", bundle_path.display()),
        }
    }

    #[test]
    fn message_delete_and_bulk_etf_and_json_preserve_64bit_snowflakes() {
        // Оракул: реальные Snowflake ID > 2^53 из `/home/sanya/re-deep/gateway-protocol.json` `idChecks`.
        // `1014537851418980372 as f64 as u64 == 1014537851418980352` (потеря младших бит при float).
        let guild_etf = decode_etf_term_body("6e0800143004f5cc5c140e");
        let channel_etf = decode_etf_term_body("6e08004720841110c85410");
        let guild_id: u64 = 1_014_537_851_418_980_372;
        let channel_id: u64 = 1_176_785_373_989_445_703;
        let msg_id_1: u64 = 1_561_632_596_763_353_163;
        let msg_id_2: u64 = 1_561_624_484_056_793_151;
        let msg_id_3: u64 = 1_561_379_697_990_959_244;
        let max_snowflake: u64 = u64::MAX; // 18_446_744_073_709_551_615

        // Доказываем, что все эти ID теряют точность при проходе через IEEE-754 f64.
        for id in [guild_id, channel_id, msg_id_1, msg_id_2, msg_id_3] {
            assert_ne!(
                (id as f64) as u64,
                id,
                "оракульный snowflake {id} обязан быть > 2^53 и чувствительным к потере точности f64"
            );
        }

        // 1. MESSAGE_DELETE из ETF (SMALL_BIG_EXT + roundtrip через etf::encode/decode).
        let delete_frame = etf_op0(
            "MESSAGE_DELETE",
            Some(77),
            etf_map(vec![
                ("id", EtfValue::Int(msg_id_1 as i64)),
                ("channel_id", channel_etf.clone()),
                ("guild_id", guild_etf.clone()),
            ]),
        );
        let wire_bytes = etf::encode(&delete_frame);
        let (decoded_frame, _) = etf::decode(&wire_bytes).expect("ETF roundtrip");

        let routed_event = route_dispatch(&decoded_frame).expect("op0 dispatch");
        assert_eq!(routed_event.kind, None, "21 канонический тип не изменён");
        assert_eq!(
            routed_event.extended_kind(),
            Some(ExtendedDispatchKind::MessageDelete)
        );
        assert!(routed_event.is_extended());
        assert!(routed_event.is_known());

        let routed = route_typed_dispatch(&decoded_frame)
            .expect("MESSAGE_DELETE должен маршрутизироваться")
            .expect("валидный MESSAGE_DELETE");
        assert_eq!(routed.kind, ExtendedDispatchKind::MessageDelete);
        assert_eq!(routed.seq, Some(77));
        assert_eq!(
            routed.payload,
            TypedDispatch::MessageDelete(MessageDeleteDispatch {
                id: msg_id_1,
                channel_id,
                guild_id: Some(guild_id),
            })
        );
        assert_eq!(routed.payload.guild_id(), Some(guild_id));
        assert_eq!(routed.payload.target_channel_id(), channel_id);

        // 2. MESSAGE_DELETE в ЛС (без guild_id или с guild_id = nil) из JSON-конвертов (op0 и {kind, data}).
        let dm_json_op0 = format!(
            r#"{{"op":0,"t":"MESSAGE_DELETE","s":78,"d":{{"id":"{max_snowflake}","channel_id":{channel_id},"guild_id":null}}}}"#
        );
        let dm_routed = route_typed_dispatch_json(&dm_json_op0)
            .expect("валидный JSON")
            .expect("MESSAGE_DELETE распознан");
        assert_eq!(dm_routed.seq, Some(78));
        assert_eq!(
            dm_routed.payload,
            TypedDispatch::MessageDelete(MessageDeleteDispatch {
                id: max_snowflake,
                channel_id,
                guild_id: None,
            })
        );

        // 3. MESSAGE_DELETE_BULK из ETF (смешанные представления Snowflake: Int, Str, Atom).
        let bulk_frame = etf_op0(
            "MESSAGE_DELETE_BULK",
            Some(91),
            etf_map(vec![
                (
                    "ids",
                    EtfValue::List(vec![
                        EtfValue::Int(msg_id_1 as i64),
                        str_val(&msg_id_2.to_string()),
                        atom(&msg_id_3.to_string()),
                        str_val(&max_snowflake.to_string()),
                    ]),
                ),
                ("channel_id", channel_etf),
                ("guild_id", guild_etf),
            ]),
        );
        let bulk_routed = route_typed_dispatch(&bulk_frame)
            .expect("MESSAGE_DELETE_BULK должен маршрутизироваться")
            .expect("валидный MESSAGE_DELETE_BULK");
        assert_eq!(bulk_routed.kind, ExtendedDispatchKind::MessageDeleteBulk);
        assert_eq!(bulk_routed.seq, Some(91));
        assert_eq!(
            bulk_routed.payload,
            TypedDispatch::MessageDeleteBulk(MessageDeleteBulkDispatch {
                ids: vec![msg_id_1, msg_id_2, msg_id_3, max_snowflake],
                channel_id,
                guild_id: Some(guild_id),
            })
        );

        // 4. MESSAGE_DELETE_BULK из JSON-конверта моста `{kind, data}` (строковые и числовые 64-битные ID).
        let bulk_bridge_json = format!(
            r#"{{"kind":"MESSAGE_DELETE_BULK","data":{{"ids":["{msg_id_1}",{msg_id_2},"{msg_id_3}","{max_snowflake}"],"channel_id":"{channel_id}","guild_id":"{guild_id}"}}}}"#
        );
        let bulk_from_json = route_typed_dispatch_json(&bulk_bridge_json)
            .expect("валидный JSON")
            .expect("MESSAGE_DELETE_BULK распознан");
        assert_eq!(bulk_from_json.seq, None);
        assert_eq!(
            bulk_from_json.payload,
            TypedDispatch::MessageDeleteBulk(MessageDeleteBulkDispatch {
                ids: vec![msg_id_1, msg_id_2, msg_id_3, max_snowflake],
                channel_id,
                guild_id: Some(guild_id),
            })
        );
    }

    #[test]
    fn channel_create_update_delete_etf_and_json_typed_parsing() {
        let guild_id: u64 = 1_014_537_851_418_980_372;
        let category_id: u64 = 1_163_408_591_315_812_392;
        let channel_id: u64 = 1_176_785_373_989_445_703;
        let dm_channel_id: u64 = 1_293_192_721_237_938_187;

        // 1. CHANNEL_CREATE (текстовый канал гильдии в ETF).
        let create_etf = etf_op0(
            "CHANNEL_CREATE",
            Some(105),
            etf_map(vec![
                ("id", EtfValue::Int(channel_id as i64)),
                ("guild_id", str_val(&guild_id.to_string())),
                ("type", EtfValue::Int(0)),
                ("name", str_val("general-chat")),
                ("parent_id", EtfValue::Int(category_id as i64)),
            ]),
        );
        let created = route_typed_dispatch(&create_etf)
            .expect("CHANNEL_CREATE маршрутизируется")
            .expect("валидный CHANNEL_CREATE");
        assert_eq!(created.kind, ExtendedDispatchKind::ChannelCreate);
        assert_eq!(created.seq, Some(105));
        assert_eq!(
            created.payload,
            TypedDispatch::ChannelCreate(ChannelLifecycleDispatch {
                id: channel_id,
                guild_id: Some(guild_id),
                kind: Some(0),
                name: Some("general-chat".to_string()),
                parent_id: Some(category_id),
            })
        );

        // 2. CHANNEL_UPDATE (голосовой канал гильдии в JSON op0).
        let update_json = format!(
            r#"{{"op":0,"t":"CHANNEL_UPDATE","s":106,"d":{{"id":"{channel_id}","guild_id":{guild_id},"type":2,"name":"Voice Lounge","parent_id":"{category_id}"}}}}"#
        );
        let updated = route_typed_dispatch_json(&update_json)
            .expect("валидный JSON")
            .expect("CHANNEL_UPDATE распознан");
        assert_eq!(updated.kind, ExtendedDispatchKind::ChannelUpdate);
        assert_eq!(updated.seq, Some(106));
        assert_eq!(
            updated.payload,
            TypedDispatch::ChannelUpdate(ChannelLifecycleDispatch {
                id: channel_id,
                guild_id: Some(guild_id),
                kind: Some(2),
                name: Some("Voice Lounge".to_string()),
                parent_id: Some(category_id),
            })
        );

        // 3. CHANNEL_DELETE (удаление/закрытие DM-канала без guild_id, parent_id, name — как в бандле @569957).
        let delete_dm_json = format!(
            r#"{{"kind":"CHANNEL_DELETE","data":{{"id":"{dm_channel_id}","type":1,"guild_id":null,"parent_id":null,"name":null}}}}"#
        );
        let deleted = route_typed_dispatch_json(&delete_dm_json)
            .expect("валидный JSON")
            .expect("CHANNEL_DELETE распознан");
        assert_eq!(deleted.kind, ExtendedDispatchKind::ChannelDelete);
        assert_eq!(
            deleted.payload,
            TypedDispatch::ChannelDelete(ChannelLifecycleDispatch {
                id: dm_channel_id,
                guild_id: None,
                kind: Some(1),
                name: None,
                parent_id: None,
            })
        );
    }

    #[test]
    fn thread_create_update_delete_etf_and_json_typed_parsing() {
        let guild_id: u64 = 1_014_537_851_418_980_372;
        let parent_channel_id: u64 = 1_014_537_851_867_775_060;
        let thread_id: u64 = 1_561_632_596_763_353_163;

        // 1. THREAD_CREATE с newly_created = true (`@4925596` в бандле) из ETF.
        let thread_create_etf = etf_op0(
            "THREAD_CREATE",
            Some(201),
            etf_map(vec![
                ("id", str_val(&thread_id.to_string())),
                ("guild_id", EtfValue::Int(guild_id as i64)),
                ("parent_id", EtfValue::Int(parent_channel_id as i64)),
                ("type", EtfValue::Int(11)),
                ("name", str_val("Разбор бага zstd")),
                ("newly_created", atom("true")),
            ]),
        );
        let created = route_typed_dispatch(&thread_create_etf)
            .expect("THREAD_CREATE маршрутизируется")
            .expect("валидный THREAD_CREATE");
        assert_eq!(created.kind, ExtendedDispatchKind::ThreadCreate);
        assert_eq!(created.seq, Some(201));
        assert_eq!(
            created.payload,
            TypedDispatch::ThreadCreate(ThreadLifecycleDispatch {
                id: thread_id,
                guild_id: Some(guild_id),
                kind: Some(11),
                name: Some("Разбор бага zstd".to_string()),
                parent_id: Some(parent_channel_id),
                newly_created: Some(true),
            })
        );

        // 2. THREAD_UPDATE из JSON op0.
        let thread_update_json = format!(
            r#"{{"op":0,"t":"THREAD_UPDATE","s":202,"d":{{"id":{thread_id},"guild_id":"{guild_id}","parent_id":"{parent_channel_id}","type":12,"name":"Архивная ветка"}}}}"#
        );
        let updated = route_typed_dispatch_json(&thread_update_json)
            .expect("валидный JSON")
            .expect("THREAD_UPDATE распознан");
        assert_eq!(updated.kind, ExtendedDispatchKind::ThreadUpdate);
        assert_eq!(updated.seq, Some(202));
        assert_eq!(
            updated.payload,
            TypedDispatch::ThreadUpdate(ThreadLifecycleDispatch {
                id: thread_id,
                guild_id: Some(guild_id),
                kind: Some(12),
                name: Some("Архивная ветка".to_string()),
                parent_id: Some(parent_channel_id),
                newly_created: None,
            })
        );

        // 3. THREAD_DELETE из JSON `{kind, data}` и ETF (в THREAD_DELETE приходят id, guild_id, parent_id, type).
        let thread_delete_json = format!(
            r#"{{"kind":"THREAD_DELETE","data":{{"id":"{thread_id}","guild_id":"{guild_id}","parent_id":"{parent_channel_id}","type":11}}}}"#
        );
        let deleted = route_typed_dispatch_json(&thread_delete_json)
            .expect("валидный JSON")
            .expect("THREAD_DELETE распознан");
        assert_eq!(deleted.kind, ExtendedDispatchKind::ThreadDelete);
        assert_eq!(
            deleted.payload,
            TypedDispatch::ThreadDelete(ThreadLifecycleDispatch {
                id: thread_id,
                guild_id: Some(guild_id),
                kind: Some(11),
                name: None,
                parent_id: Some(parent_channel_id),
                newly_created: None,
            })
        );
    }

    #[test]
    fn rejects_invalid_snowflakes_missing_fields_and_non_dispatch_envelopes() {
        // 1. Отрицательный big-int из старого сломанного парсера (`-1467272931325252608` в gateway-schema.json) отвергается.
        let neg_guild_etf = etf_op0(
            "MESSAGE_DELETE",
            Some(1),
            etf_map(vec![
                ("id", EtfValue::Int(1_561_632_596_763_353_163)),
                ("channel_id", EtfValue::Int(1_176_785_373_989_445_703)),
                ("guild_id", EtfValue::Int(-1_467_272_931_325_252_608)),
            ]),
        );
        let err = route_typed_dispatch(&neg_guild_etf)
            .expect("MESSAGE_DELETE распознан")
            .expect_err("отрицательный snowflake должен отвергаться");
        assert!(matches!(
            err,
            DispatchParseError::InvalidSnowflake {
                event: "MESSAGE_DELETE",
                ref field,
                ..
            } if field == "guild_id"
        ));

        // 2. Float-представление Snowflake (потеря точности > 2^53) отвергается и в ETF, и в JSON.
        let float_id_etf = etf_op0(
            "CHANNEL_CREATE",
            Some(2),
            etf_map(vec![("id", EtfValue::Float(1014537851418980372.0))]),
        );
        assert!(matches!(
            route_typed_dispatch(&float_id_etf),
            Some(Err(DispatchParseError::InvalidSnowflake {
                event: "CHANNEL_CREATE",
                ..
            }))
        ));
        let float_id_json =
            r#"{"op":0,"t":"THREAD_DELETE","s":3,"d":{"id":1014537851418980372.0}}"#;
        assert!(matches!(
            route_typed_dispatch_json(float_id_json),
            Err(DispatchParseError::InvalidSnowflake {
                event: "THREAD_DELETE",
                ..
            })
        ));

        // 3. Переполнение u64 (`18446744073709551616` = 2^64) и нечисловые строки отвергаются.
        let overflow_json = r#"{"op":0,"t":"MESSAGE_DELETE","s":4,"d":{"id":"18446744073709551616","channel_id":"123"}}"#;
        assert!(matches!(
            route_typed_dispatch_json(overflow_json),
            Err(DispatchParseError::InvalidSnowflake {
                event: "MESSAGE_DELETE",
                ..
            })
        ));

        // 4. Пустой массив `ids` или битый элемент в `MESSAGE_DELETE_BULK` отвергаются.
        let empty_bulk_json =
            r#"{"op":0,"t":"MESSAGE_DELETE_BULK","s":5,"d":{"ids":[],"channel_id":"123"}}"#;
        assert!(matches!(
            route_typed_dispatch_json(empty_bulk_json),
            Err(DispatchParseError::EmptyBulkIds {
                event: "MESSAGE_DELETE_BULK"
            })
        ));
        let bad_item_bulk_json = r#"{"op":0,"t":"MESSAGE_DELETE_BULK","s":6,"d":{"ids":["123","not_a_snowflake"],"channel_id":"456"}}"#;
        assert!(matches!(
            route_typed_dispatch_json(bad_item_bulk_json),
            Err(DispatchParseError::InvalidSnowflake {
                event: "MESSAGE_DELETE_BULK",
                ref field,
                ..
            }) if field == "ids[1]"
        ));

        // 5. Отсутствие обязательных полей (`id`, `channel_id`).
        let missing_channel_etf = etf_op0(
            "MESSAGE_DELETE",
            Some(7),
            etf_map(vec![("id", str_val("12345"))]),
        );
        assert!(matches!(
            route_typed_dispatch(&missing_channel_etf),
            Some(Err(DispatchParseError::MissingField {
                event: "MESSAGE_DELETE",
                field: "channel_id"
            }))
        ));

        // 6. Не-op0 кадры и события вне 8 расширенных типов возвращают `None` (а не ошибку).
        let hello_etf = etf_map(vec![
            ("op", EtfValue::Int(10)),
            ("t", atom("MESSAGE_DELETE")),
            ("d", etf_map(vec![])),
        ]);
        assert_eq!(route_typed_dispatch(&hello_etf), None);
        assert_eq!(
            route_typed_dispatch_json(r#"{"op":10,"t":"MESSAGE_DELETE","d":{}}"#),
            Ok(None)
        );
        assert_eq!(
            route_typed_dispatch_json(r#"{"op":0,"t":"MESSAGE_CREATE","s":1,"d":{"id":"1"}}"#),
            Ok(None)
        );
        let parsed_msg_create =
            parse_json(r#"{"id":"1"}"#).expect("валидный JSON для MESSAGE_CREATE");
        assert_eq!(
            parse_typed_dispatch_json("MESSAGE_CREATE", &parsed_msg_create),
            None
        );
    }
}
