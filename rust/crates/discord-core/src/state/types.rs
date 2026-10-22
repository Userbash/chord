//! P2-02a: модель состояния и разбор событий (RW-13 / W13a).
//!
//! Контракт заморожен `waves/RW-13/DESIGN-P2-02.md` §1–§7. Здесь нет I/O: только типы,
//! `BTreeMap`-индексы и чистый разбор уже декодированных ETF-значений. `State::Debug`
//! редактирует `session_id` и `user_settings_proto`; сырой READY (`raw`) не сохраняется.

use std::collections::BTreeMap;
use std::fmt;

use discord_codec::etf::EtfValue;

use crate::dispatch::{DispatchEvent, DispatchKind, route_dispatch};
use crate::ready::guild::parse_channel_value;
use crate::ready::social::{parse_merged_members, parse_sessions, parse_user};
use crate::ready::{
    Channel, Guild, GuildMember, PrivateChannel, Ready, ReadyDiagnostics, ReadyError, Relationship,
    Session, User, value,
};

use super::member_chunk::{MemberChunk, MemberChunkAssembly, parse_member_chunk};
use super::member_list::{MemberList, MemberListId, MemberListUpdate, parse_member_list_update};
use super::presence::{
    MergedPresences, Presence, PresenceKey, PresenceUpdate, parse_merged_presences,
    parse_presence_event,
};
use super::read_state::MessageAck;
use super::typing::{TypingEntry, TypingKey, TypingStart};
use super::voice::{
    VoiceChannelKey, VoiceChannelStartTime, VoiceChannelStatus, VoiceKey, VoiceState,
    VoiceStateUpdate,
};

/// Диагностика reducer и разбора событий.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateDiagnostics {
    /// Новые top-level ключи READY.
    pub unknown_top_level: Vec<String>,
    /// Пропущенные/битые элементы.
    pub malformed: Vec<String>,
    /// Дубликаты id (last wins).
    pub duplicates: Vec<String>,
    /// Поля неверного типа, заменённые дефолтом.
    pub ignored: Vec<String>,
    /// Неизвестные имена `t`.
    pub unknown_events: Vec<String>,
    /// Известные, но не поддержанные в P2-02 типы.
    pub unsupported_events: Vec<String>,
    /// Отброшенные `s`-последовательности.
    pub stale_sequences: Vec<u64>,
    /// Ссылочные несоответствия (неизвестная гильдия, смена канала и т.п.).
    pub inconsistencies: Vec<String>,
}

impl StateDiagnostics {
    /// Перенести диагностику READY (P2-01) в пространство состояния.
    #[must_use]
    pub fn from_ready(diagnostics: &ReadyDiagnostics) -> Self {
        Self {
            unknown_top_level: diagnostics.unknown_top_level.clone(),
            malformed: diagnostics.malformed.clone(),
            duplicates: diagnostics.duplicates.clone(),
            ignored: diagnostics.ignored.clone(),
            ..Self::default()
        }
    }

    /// Дописать диагностику другого события/разбора, сохраняя порядок.
    pub fn merge(&mut self, other: &Self) {
        self.unknown_top_level
            .extend_from_slice(&other.unknown_top_level);
        self.malformed.extend_from_slice(&other.malformed);
        self.duplicates.extend_from_slice(&other.duplicates);
        self.ignored.extend_from_slice(&other.ignored);
        self.unknown_events.extend_from_slice(&other.unknown_events);
        self.unsupported_events
            .extend_from_slice(&other.unsupported_events);
        self.stale_sequences
            .extend_from_slice(&other.stale_sequences);
        self.inconsistencies
            .extend_from_slice(&other.inconsistencies);
    }
}

/// Модель состояния Discord после READY и применённых событий.
///
/// Индексы `channel_guild`/`thread_guild` приватны: снаружи — только селекторы.
#[derive(Clone, PartialEq, Eq)]
pub struct State {
    /// Собственный пользователь READY.
    pub user: User,
    /// ID сессии (не печатается в `Debug`).
    pub session_id: String,
    /// `session_type`.
    pub session_type: Option<String>,
    /// `resume_gateway_url`.
    pub resume_gateway_url: Option<String>,
    /// Гильдии по id.
    pub guilds: BTreeMap<u64, Guild>,
    /// Приватные каналы по id.
    pub private_channels: BTreeMap<u64, PrivateChannel>,
    /// Известные пользователи по id (включая `user`).
    pub users: BTreeMap<u64, User>,
    /// Связи по id.
    pub relationships: BTreeMap<u64, Relationship>,
    /// Read-state снимок.
    pub read_state: crate::ready::ReadState,
    /// Настройки гильдий.
    pub user_guild_settings: crate::ready::UserGuildSettings,
    /// Сессии в порядке события/снимка.
    pub sessions: Vec<Session>,
    /// Сообщения по id.
    pub messages: BTreeMap<u64, Message>,
    /// Ленивые списки участников по `(guild_id, list_id)`.
    pub member_lists: BTreeMap<(u64, MemberListId), MemberList>,
    /// Сборки `GUILD_MEMBERS_CHUNK` по `(guild_id, nonce)`.
    pub member_chunks: BTreeMap<(u64, Option<String>), MemberChunkAssembly>,
    /// Presence по `(user_id, scope)`; offline-only записи удаляются.
    pub presences: BTreeMap<PresenceKey, Presence>,
    /// Voice-состояния по `(guild_id, user_id)`.
    pub voice_states: BTreeMap<VoiceKey, VoiceState>,
    /// Стартовое время голосовых каналов по `(guild_id, channel_id)`.
    pub voice_channel_start_times: BTreeMap<VoiceChannelKey, i64>,
    /// Статусы голосовых каналов по `(guild_id, channel_id)`.
    pub voice_channel_statuses: BTreeMap<VoiceChannelKey, String>,
    /// Typing по `(channel_id, user_id)`; memory-only TTL.
    pub typing: BTreeMap<TypingKey, TypingEntry>,
    /// Треды по id.
    pub threads: BTreeMap<u64, Thread>,
    /// RTC-регионы READY.
    pub geo_ordered_rtc_regions: Vec<String>,
    /// Страна.
    pub country_code: Option<String>,
    /// Субдивизион.
    pub subdivision_code: Option<String>,
    /// Opaque protobuf (не печатается в `Debug`).
    pub user_settings_proto: Option<String>,
    /// Нефатальные расхождения.
    pub diagnostics: StateDiagnostics,
    /// Последняя применённая `s`.
    pub last_seq: Option<u64>,
    /// `channel_id -> guild_id`.
    pub(crate) channel_guild: BTreeMap<u64, u64>,
    /// `thread_id -> guild_id`.
    pub(crate) thread_guild: BTreeMap<u64, u64>,
}

impl fmt::Debug for State {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("State")
            .field("user_id", &self.user.id)
            .field("session_id", &"<redacted>")
            .field("session_type", &self.session_type)
            .field("guilds", &self.guilds.len())
            .field("channels", &self.channel_guild.len())
            .field("private_channels", &self.private_channels.len())
            .field("users", &self.users.len())
            .field("relationships", &self.relationships.len())
            .field("read_state_entries", &self.read_state.entries.len())
            .field("sessions", &self.sessions.len())
            .field("messages", &self.messages.len())
            .field("member_lists", &self.member_lists.len())
            .field("member_chunks", &self.member_chunks.len())
            .field("presences", &self.presences.len())
            .field("voice_states", &self.voice_states.len())
            .field(
                "voice_channel_start_times",
                &self.voice_channel_start_times.len(),
            )
            .field("voice_channel_statuses", &self.voice_channel_statuses.len())
            .field("typing", &self.typing.len())
            .field("threads", &self.threads.len())
            .field(
                "user_settings_proto",
                &self.user_settings_proto.as_ref().map(|_| "<redacted>"),
            )
            .field("last_seq", &self.last_seq)
            .field("diagnostics", &self.diagnostics)
            .finish_non_exhaustive()
    }
}

impl State {
    /// Построить состояние из разобранного READY (P2-01).
    #[must_use]
    pub fn from_ready(ready: &Ready) -> Self {
        let mut diagnostics = StateDiagnostics::from_ready(&ready.diagnostics);

        let guilds: BTreeMap<u64, Guild> = ready
            .guilds
            .iter()
            .cloned()
            .map(|guild| (guild.id, guild))
            .collect();
        let private_channels = ready
            .private_channels
            .iter()
            .cloned()
            .map(|channel| (channel.id, channel))
            .collect();
        let mut users: BTreeMap<u64, User> = ready
            .users
            .iter()
            .cloned()
            .map(|user| (user.id, user))
            .collect();
        users.insert(ready.user.id, ready.user.clone());
        let relationships = ready
            .relationships
            .iter()
            .cloned()
            .map(|relationship| (relationship.id, relationship))
            .collect();

        let mut channel_guild = BTreeMap::new();
        for guild in &ready.guilds {
            for channel in &guild.channels {
                if let Some(previous) = channel_guild.insert(channel.id, guild.id)
                    && previous != guild.id
                {
                    diagnostics.inconsistencies.push(format!(
                        "channels:{}:guilds:{previous}->{}",
                        channel.id, guild.id
                    ));
                }
            }
        }

        let mut state = Self {
            user: ready.user.clone(),
            session_id: ready.session_id.clone(),
            session_type: ready.session_type.clone(),
            resume_gateway_url: ready.resume_gateway_url.clone(),
            guilds,
            private_channels,
            users,
            relationships,
            read_state: ready.read_state.clone(),
            user_guild_settings: ready.user_guild_settings.clone(),
            sessions: Vec::new(),
            messages: BTreeMap::new(),
            member_lists: BTreeMap::new(),
            member_chunks: BTreeMap::new(),
            presences: BTreeMap::new(),
            voice_states: BTreeMap::new(),
            voice_channel_start_times: BTreeMap::new(),
            voice_channel_statuses: BTreeMap::new(),
            typing: BTreeMap::new(),
            threads: BTreeMap::new(),
            geo_ordered_rtc_regions: ready.geo_ordered_rtc_regions.clone(),
            country_code: ready.country_code.clone(),
            subdivision_code: ready.subdivision_code.clone(),
            user_settings_proto: ready.user_settings_proto.clone(),
            diagnostics,
            last_seq: None,
            channel_guild,
            thread_guild: BTreeMap::new(),
        };
        state.replace_sessions(ready.sessions.clone());
        state
    }

    /// Заменить сессии; дубликаты `session_id` — last wins, позиция первого.
    ///
    /// Идентификатор сессии — секрет, поэтому в диагностику он не попадает.
    pub(crate) fn replace_sessions(&mut self, sessions: Vec<Session>) {
        let mut positions: BTreeMap<String, usize> = BTreeMap::new();
        let mut unique = Vec::with_capacity(sessions.len());
        for session in sessions {
            if let Some(&position) = positions.get(&session.session_id) {
                unique[position] = session;
                self.diagnostics
                    .duplicates
                    .push("sessions:duplicate".to_string());
            } else {
                positions.insert(session.session_id.clone(), unique.len());
                unique.push(session);
            }
        }
        self.sessions = unique;
    }
}

/// Сообщение в состоянии.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Message {
    /// Снежинка.
    pub id: u64,
    /// Канал.
    pub channel_id: u64,
    /// Гильдия (для ЛС — `None`).
    pub guild_id: Option<u64>,
    /// Автор.
    pub author: Option<User>,
    /// Текст.
    pub content: String,
    /// Время создания.
    pub timestamp: Option<String>,
    /// Время правки.
    pub edited_timestamp: Option<String>,
    /// `type`.
    pub message_type: i64,
    /// Флаги.
    pub flags: i64,
    /// Закреплено.
    pub pinned: bool,
    /// TTS.
    pub tts: bool,
    /// `@everyone`.
    pub mention_everyone: bool,
    /// Упомянутые пользователи.
    pub mention_user_ids: Vec<u64>,
    /// Упомянутые роли.
    pub mention_role_ids: Vec<u64>,
    /// Число вложений (сами вложения — P3-04).
    pub attachment_count: usize,
    /// Число embeds (контент — P3-02).
    pub embed_count: usize,
}

/// Частичное обновление сообщения: `None` — «не менять».
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MessagePatch {
    /// Снежинка.
    pub id: u64,
    /// Канал (если пришёл).
    pub channel_id: Option<u64>,
    /// Гильдия.
    pub guild_id: Option<u64>,
    /// Автор.
    pub author: Option<User>,
    /// Текст.
    pub content: Option<String>,
    /// Время правки.
    pub edited_timestamp: Option<String>,
    /// `type`.
    pub message_type: Option<i64>,
    /// Флаги.
    pub flags: Option<i64>,
    /// Закреплено.
    pub pinned: Option<bool>,
    /// TTS.
    pub tts: Option<bool>,
    /// `@everyone`.
    pub mention_everyone: Option<bool>,
    /// Упомянутые пользователи.
    pub mention_user_ids: Option<Vec<u64>>,
    /// Упомянутые роли.
    pub mention_role_ids: Option<Vec<u64>>,
    /// Число вложений.
    pub attachment_count: Option<usize>,
    /// Число embeds.
    pub embed_count: Option<usize>,
}

/// Тред из `THREAD_LIST_SYNC`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Thread {
    /// Снежинка.
    pub id: u64,
    /// Гильдия из конверта события.
    pub guild_id: u64,
    /// Родительский канал.
    pub parent_id: Option<u64>,
    /// Имя.
    pub name: Option<String>,
    /// `type` (11 — public thread).
    pub thread_type: i64,
    /// Владелец.
    pub owner_id: Option<u64>,
    /// Последнее сообщение.
    pub last_message_id: Option<u64>,
    /// Число сообщений.
    pub message_count: i64,
    /// Число участников.
    pub member_count: i64,
    /// Всего отправлено.
    pub total_message_sent: i64,
    /// Rate limit.
    pub rate_limit_per_user: Option<i64>,
    /// Флаги.
    pub flags: i64,
    /// Архивирован.
    pub archived: bool,
    /// Закрыт.
    pub locked: bool,
    /// Приглашения разрешены.
    pub invitable: bool,
    /// Автоархивация (минуты).
    pub auto_archive_duration: Option<i64>,
    /// Время архивации.
    pub archive_timestamp: Option<String>,
    /// Время создания.
    pub create_timestamp: Option<String>,
    /// Превью участников.
    pub member_ids_preview: Vec<u64>,
}

/// `THREAD_LIST_SYNC`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadListSync {
    /// Гильдия.
    pub guild_id: u64,
    /// Треды в порядке события.
    pub threads: Vec<Thread>,
}

/// `CHANNEL_INFO`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelInfo {
    /// Гильдия.
    pub guild_id: u64,
    /// Каналы в порядке события.
    pub channels: Vec<Channel>,
}

/// `READY_SUPPLEMENTAL`: группы участников по сырому индексу `d.guilds`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadySupplemental {
    /// Для каждого сырого элемента `d.guilds[i]` — id или `None` (битый элемент).
    pub guild_ids: Vec<Option<u64>>,
    /// `merged_members[i]` — участники гильдии с сырым индексом `i`.
    pub merged_members: Vec<Vec<GuildMember>>,
    /// `merged_presences` — друзья и группы, выровненные по `guild_ids`.
    pub merged_presences: MergedPresences,
    /// Голосовые состояния из `d.guilds[i].voice_states`.
    pub voice_states: Vec<VoiceState>,
}

/// Типизированное событие состояния.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    /// `s`-последовательность.
    pub seq: Option<u64>,
    /// Полезная нагрузка.
    pub payload: EventPayload,
    /// Диагностика разбора события.
    pub diagnostics: StateDiagnostics,
}

/// Полезная нагрузка события.
#[derive(Debug, Clone, PartialEq)]
pub enum EventPayload {
    /// READY.
    Ready(Box<Ready>),
    /// READY_SUPPLEMENTAL.
    ReadySupplemental(ReadySupplemental),
    /// RESUMED.
    Resumed,
    /// SESSIONS_REPLACE.
    SessionsReplace(Vec<Session>),
    /// THREAD_LIST_SYNC.
    ThreadListSync(ThreadListSync),
    /// CHANNEL_INFO.
    ChannelInfo(ChannelInfo),
    /// MESSAGE_CREATE.
    MessageCreate(Message),
    /// MESSAGE_UPDATE.
    MessageUpdate(MessagePatch),
    /// GUILD_MEMBER_LIST_UPDATE.
    GuildMemberListUpdate(MemberListUpdate),
    /// GUILD_MEMBERS_CHUNK.
    GuildMembersChunk(MemberChunk),
    /// MESSAGE_ACK.
    MessageAck(MessageAck),
    /// PRESENCE_UPDATE.
    PresenceUpdate(PresenceUpdate),
    /// VOICE_STATE_UPDATE.
    VoiceStateUpdate(VoiceStateUpdate),
    /// VOICE_CHANNEL_START_TIME_UPDATE.
    VoiceChannelStartTime(VoiceChannelStartTime),
    /// VOICE_CHANNEL_STATUS_UPDATE.
    VoiceChannelStatus(VoiceChannelStatus),
    /// TYPING_START.
    TypingStart(TypingStart),
    /// Известный тип, не входящий в P2-02.
    Unsupported(DispatchKind),
    /// Неизвестный `t`.
    Unknown {
        /// Имя провода.
        name: String,
    },
}

impl Event {
    /// Событие из уже маршрутизированного dispatch.
    pub fn from_routed(dispatch: &DispatchEvent) -> Result<Self, EventError> {
        Self::from_routed_at(dispatch, None)
    }

    fn from_routed_at(
        dispatch: &DispatchEvent,
        received_at_ms: Option<u64>,
    ) -> Result<Self, EventError> {
        let seq = dispatch.seq;
        let (payload, diagnostics) = match dispatch.kind {
            Some(DispatchKind::Ready) => {
                let ready = Ready::parse(&dispatch.data)?;
                (
                    EventPayload::Ready(Box::new(ready)),
                    StateDiagnostics::default(),
                )
            }
            Some(DispatchKind::ReadySupplemental) => {
                let (supplemental, diagnostics) = parse_ready_supplemental(&dispatch.data)?;
                (EventPayload::ReadySupplemental(supplemental), diagnostics)
            }
            Some(DispatchKind::Resumed) => {
                if !value::is_null(&dispatch.data) && dispatch.data.as_map().is_none() {
                    return Err(EventError::WrongType {
                        path: "resumed".to_string(),
                        expected: "map|null",
                        found: value::kind(&dispatch.data),
                    });
                }
                (EventPayload::Resumed, StateDiagnostics::default())
            }
            Some(DispatchKind::SessionsReplace) => {
                if value::items(&dispatch.data).is_none() {
                    return Err(EventError::WrongType {
                        path: "sessions_replace".to_string(),
                        expected: "list",
                        found: value::kind(&dispatch.data),
                    });
                }
                let mut ready = ReadyDiagnostics::default();
                let sessions = parse_sessions(Some(&dispatch.data), &mut ready);
                (
                    EventPayload::SessionsReplace(sessions),
                    StateDiagnostics::from_ready(&ready),
                )
            }
            Some(DispatchKind::ThreadListSync) => {
                let (sync, diagnostics) = parse_thread_list_sync(&dispatch.data)?;
                (EventPayload::ThreadListSync(sync), diagnostics)
            }
            Some(DispatchKind::ChannelInfo) => {
                let (info, diagnostics) = parse_channel_info(&dispatch.data)?;
                (EventPayload::ChannelInfo(info), diagnostics)
            }
            Some(DispatchKind::MessageCreate) => {
                let (message, diagnostics) = parse_message(&dispatch.data)?;
                (EventPayload::MessageCreate(message), diagnostics)
            }
            Some(DispatchKind::MessageUpdate) => {
                let (patch, diagnostics) = parse_message_patch(&dispatch.data)?;
                (EventPayload::MessageUpdate(patch), diagnostics)
            }
            Some(DispatchKind::GuildMemberListUpdate) => {
                let (update, diagnostics) = parse_member_list_update(&dispatch.data)?;
                (EventPayload::GuildMemberListUpdate(update), diagnostics)
            }
            Some(DispatchKind::GuildMembersChunk) => {
                let (chunk, diagnostics) = parse_member_chunk(&dispatch.data)?;
                (EventPayload::GuildMembersChunk(chunk), diagnostics)
            }
            Some(DispatchKind::MessageAck) => {
                let (ack, diagnostics) = MessageAck::parse(&dispatch.data)?;
                (EventPayload::MessageAck(ack), diagnostics)
            }
            Some(DispatchKind::PresenceUpdate) => {
                let (update, diagnostics) = parse_presence_event(&dispatch.data)?;
                (EventPayload::PresenceUpdate(update), diagnostics)
            }
            Some(DispatchKind::VoiceStateUpdate) => {
                let (update, diagnostics) = super::voice::parse_voice_state_update(&dispatch.data)?;
                (EventPayload::VoiceStateUpdate(update), diagnostics)
            }
            Some(DispatchKind::VoiceChannelStartTimeUpdate) => {
                let (update, diagnostics) =
                    super::voice::parse_voice_channel_start_time(&dispatch.data)?;
                (EventPayload::VoiceChannelStartTime(update), diagnostics)
            }
            Some(DispatchKind::VoiceChannelStatusUpdate) => {
                let (update, diagnostics) =
                    super::voice::parse_voice_channel_status(&dispatch.data)?;
                (EventPayload::VoiceChannelStatus(update), diagnostics)
            }
            Some(DispatchKind::TypingStart) => {
                let (start, diagnostics) =
                    super::typing::parser::parse_typing_start(&dispatch.data, received_at_ms)?;
                (EventPayload::TypingStart(start), diagnostics)
            }
            Some(kind) => (EventPayload::Unsupported(kind), StateDiagnostics::default()),
            None => (
                EventPayload::Unknown {
                    name: dispatch.name.clone(),
                },
                StateDiagnostics::default(),
            ),
        };
        Ok(Self {
            seq,
            payload,
            diagnostics,
        })
    }

    /// Полный op0-кадр → событие; не-dispatch — [`EventError::NotDispatch`].
    pub fn from_frame(frame: &EtfValue) -> Result<Self, EventError> {
        let dispatch = route_dispatch(frame).ok_or(EventError::NotDispatch)?;
        Self::from_routed(&dispatch)
    }

    /// Полный op0-кадр с детерминированными часами (`received_at_ms`).
    pub fn from_frame_at(frame: &EtfValue, now_ms: u64) -> Result<Self, EventError> {
        let dispatch = route_dispatch(frame).ok_or(EventError::NotDispatch)?;
        Self::from_routed_at(&dispatch, Some(now_ms))
    }
}

/// Ошибка разбора события.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventError {
    /// Не op0-диспатч.
    NotDispatch,
    /// Обязательное поле отсутствует.
    Missing {
        /// Путь.
        path: String,
    },
    /// Поле не того типа.
    WrongType {
        /// Путь.
        path: String,
        /// Ожидаемый тип.
        expected: &'static str,
        /// Фактический вид.
        found: &'static str,
    },
    /// Невалидная снежинка.
    InvalidId {
        /// Путь.
        path: String,
        /// Исходное значение.
        value: String,
    },
    /// Ошибка READY-парсера.
    Ready(ReadyError),
}

impl fmt::Display for EventError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotDispatch => write!(formatter, "не op0 dispatch"),
            Self::Missing { path } => write!(formatter, "нет обязательного поля {path}"),
            Self::WrongType {
                path,
                expected,
                found,
            } => write!(
                formatter,
                "поле {path}: ожидался {expected}, найден {found}"
            ),
            Self::InvalidId { path, value } => {
                write!(formatter, "поле {path}: невалидная снежинка {value:?}")
            }
            Self::Ready(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for EventError {}

impl From<ReadyError> for EventError {
    fn from(error: ReadyError) -> Self {
        Self::Ready(error)
    }
}

/// Эффект-декларация; исполнение — вне P2-02.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Загрузить сообщения канала (P2-06).
    FetchMessages {
        /// Канал.
        channel_id: u64,
        /// До какого id.
        before: Option<u64>,
        /// Лимит.
        limit: u16,
    },
    /// Подписаться на ленивый список участников (P2-04).
    SubscribeMembers {
        /// Гильдия.
        guild_id: u64,
        /// Диапазоны (включительно).
        ranges: Vec<MemberRange>,
    },
    /// Подтвердить прочтение (P2-07).
    AckMessage {
        /// Канал.
        channel_id: u64,
        /// Сообщение.
        message_id: u64,
    },
    /// Запросить присутствие (P2-09).
    RequestPresence {
        /// Гильдия.
        guild_id: u64,
        /// Пользователи.
        user_ids: Vec<u64>,
    },
}

/// Включительный диапазон списка участников.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemberRange {
    /// Начало.
    pub start: u32,
    /// Конец (включительно).
    pub end: u32,
}

/// Детерминированный контейнер эффектов.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Effects(Vec<Effect>);

impl Effects {
    /// Пустой список.
    #[must_use]
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Добавить эффект в конец.
    pub fn push(&mut self, effect: Effect) {
        self.0.push(effect);
    }

    /// Срез в порядке добавления.
    #[must_use]
    pub fn as_slice(&self) -> &[Effect] {
        &self.0
    }

    /// Число эффектов.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Пусто?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Владеющий вектор.
    #[must_use]
    pub fn into_vec(self) -> Vec<Effect> {
        self.0
    }
}

impl IntoIterator for Effects {
    type Item = Effect;
    type IntoIter = std::vec::IntoIter<Effect>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

fn event_map<'a>(data: &'a EtfValue, path: &str) -> Result<&'a EtfValue, EventError> {
    if data.as_map().is_none() {
        return Err(EventError::WrongType {
            path: path.to_string(),
            expected: "map",
            found: value::kind(data),
        });
    }
    Ok(data)
}

pub(crate) fn raw_display(raw: &EtfValue) -> String {
    match raw {
        EtfValue::Str(text) | EtfValue::Atom(text) => text.clone(),
        EtfValue::Int(number) => number.to_string(),
        EtfValue::Float(number) => number.to_string(),
        EtfValue::Nil => "null".to_string(),
        other => value::kind(other).to_string(),
    }
}

fn required_unsigned(map: &EtfValue, key: &str, path: &str) -> Result<u64, EventError> {
    let raw = map.get(key).ok_or_else(|| EventError::Missing {
        path: path.to_string(),
    })?;
    value::unsigned(raw).ok_or_else(|| EventError::InvalidId {
        path: path.to_string(),
        value: raw_display(raw),
    })
}

fn optional_unsigned(map: &EtfValue, key: &str, path: &str) -> Result<Option<u64>, EventError> {
    let Some(raw) = map.get(key) else {
        return Ok(None);
    };
    if value::is_null(raw) {
        return Ok(None);
    }
    value::unsigned(raw)
        .map(Some)
        .ok_or_else(|| EventError::InvalidId {
            path: path.to_string(),
            value: raw_display(raw),
        })
}

fn lenient_unsigned(
    map: &EtfValue,
    key: &str,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Option<u64> {
    let raw = map.get(key)?;
    if value::is_null(raw) {
        return None;
    }
    match value::unsigned(raw) {
        Some(value) => Some(value),
        None => {
            diagnostics.ignored.push(format!("{path}.{key}"));
            None
        }
    }
}

fn field_text(
    map: &EtfValue,
    key: &str,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Option<String> {
    let raw = map.get(key)?;
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

fn field_int(
    map: &EtfValue,
    key: &str,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Option<i64> {
    let raw = map.get(key)?;
    if value::is_null(raw) {
        return None;
    }
    match value::integer(raw) {
        Some(value) => Some(value),
        None => {
            diagnostics.ignored.push(format!("{path}.{key}"));
            None
        }
    }
}

fn field_bool(
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
        Some(value) => Some(value),
        None => {
            diagnostics.ignored.push(format!("{path}.{key}"));
            None
        }
    }
}

fn field_ids(
    map: &EtfValue,
    key: &str,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Option<Vec<u64>> {
    let raw = map.get(key)?;
    if value::is_null(raw) {
        return None;
    }
    let Some(items) = value::items(raw) else {
        diagnostics.ignored.push(format!("{path}.{key}"));
        return None;
    };
    let mut ids = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let id = if let Some(id) = value::unsigned(item) {
            Some(id)
        } else {
            item.get("id").and_then(value::unsigned)
        };
        match id {
            Some(id) => ids.push(id),
            None => diagnostics.ignored.push(format!("{path}.{key}[{index}]")),
        }
    }
    Some(ids)
}

fn field_count(
    map: &EtfValue,
    key: &str,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Option<usize> {
    let raw = map.get(key)?;
    if value::is_null(raw) {
        return None;
    }
    match value::items(raw) {
        Some(items) => Some(items.len()),
        None => {
            diagnostics.ignored.push(format!("{path}.{key}"));
            None
        }
    }
}

fn parse_author(value: Option<&EtfValue>, diagnostics: &mut StateDiagnostics) -> Option<User> {
    let raw = value?;
    if value::is_null(raw) {
        return None;
    }
    if value::object(raw).is_none() {
        diagnostics.ignored.push("message.author".to_string());
        return None;
    }
    let mut ready = ReadyDiagnostics::default();
    let user = parse_user(raw, "message.author", &mut ready);
    diagnostics.merge(&StateDiagnostics::from_ready(&ready));
    if user.is_none() {
        diagnostics.malformed.push("message.author".to_string());
    }
    user
}

fn parse_message(data: &EtfValue) -> Result<(Message, StateDiagnostics), EventError> {
    let map = event_map(data, "d")?;
    let id = required_unsigned(map, "id", "message.id")?;
    let channel_id = required_unsigned(map, "channel_id", "message.channel_id")?;
    let guild_id = optional_unsigned(map, "guild_id", "message.guild_id")?;
    let mut diagnostics = StateDiagnostics::default();
    let author = parse_author(map.get("author"), &mut diagnostics);

    let message = Message {
        id,
        channel_id,
        guild_id,
        author,
        content: field_text(map, "content", "message", &mut diagnostics).unwrap_or_default(),
        timestamp: field_text(map, "timestamp", "message", &mut diagnostics),
        edited_timestamp: field_text(map, "edited_timestamp", "message", &mut diagnostics),
        message_type: field_int(map, "type", "message", &mut diagnostics).unwrap_or(0),
        flags: field_int(map, "flags", "message", &mut diagnostics).unwrap_or(0),
        pinned: field_bool(map, "pinned", "message", &mut diagnostics).unwrap_or(false),
        tts: field_bool(map, "tts", "message", &mut diagnostics).unwrap_or(false),
        mention_everyone: field_bool(map, "mention_everyone", "message", &mut diagnostics)
            .unwrap_or(false),
        mention_user_ids: field_ids(map, "mentions", "message", &mut diagnostics)
            .unwrap_or_default(),
        mention_role_ids: field_ids(map, "mention_roles", "message", &mut diagnostics)
            .unwrap_or_default(),
        attachment_count: field_count(map, "attachments", "message", &mut diagnostics).unwrap_or(0),
        embed_count: field_count(map, "embeds", "message", &mut diagnostics).unwrap_or(0),
    };
    Ok((message, diagnostics))
}

fn parse_message_patch(data: &EtfValue) -> Result<(MessagePatch, StateDiagnostics), EventError> {
    let map = event_map(data, "d")?;
    let id = required_unsigned(map, "id", "message.id")?;
    let channel_id = optional_unsigned(map, "channel_id", "message.channel_id")?;
    let guild_id = optional_unsigned(map, "guild_id", "message.guild_id")?;
    let mut diagnostics = StateDiagnostics::default();
    let author = parse_author(map.get("author"), &mut diagnostics);

    let patch = MessagePatch {
        id,
        channel_id,
        guild_id,
        author,
        content: field_text(map, "content", "message", &mut diagnostics),
        edited_timestamp: field_text(map, "edited_timestamp", "message", &mut diagnostics),
        message_type: field_int(map, "type", "message", &mut diagnostics),
        flags: field_int(map, "flags", "message", &mut diagnostics),
        pinned: field_bool(map, "pinned", "message", &mut diagnostics),
        tts: field_bool(map, "tts", "message", &mut diagnostics),
        mention_everyone: field_bool(map, "mention_everyone", "message", &mut diagnostics),
        mention_user_ids: field_ids(map, "mentions", "message", &mut diagnostics),
        mention_role_ids: field_ids(map, "mention_roles", "message", &mut diagnostics),
        attachment_count: field_count(map, "attachments", "message", &mut diagnostics),
        embed_count: field_count(map, "embeds", "message", &mut diagnostics),
    };
    Ok((patch, diagnostics))
}

fn parse_channel_info(data: &EtfValue) -> Result<(ChannelInfo, StateDiagnostics), EventError> {
    let map = event_map(data, "d")?;
    let guild_id = required_unsigned(map, "guild_id", "channel_info.guild_id")?;
    let raw_channels = map.get("channels").ok_or_else(|| EventError::Missing {
        path: "channel_info.channels".to_string(),
    })?;
    let items = value::items(raw_channels).ok_or_else(|| EventError::WrongType {
        path: "channel_info.channels".to_string(),
        expected: "list",
        found: value::kind(raw_channels),
    })?;

    let mut ready = ReadyDiagnostics::default();
    let mut channels = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        if let Some(channel) = parse_channel_value(item, &format!("channels[{index}]"), &mut ready)
        {
            channels.push(channel);
        }
    }
    let channels = value::dedup_last_by(
        channels,
        |channel| channel.id,
        |channel| channel.id.to_string(),
        "channels",
        &mut ready,
    );
    Ok((
        ChannelInfo { guild_id, channels },
        StateDiagnostics::from_ready(&ready),
    ))
}

fn parse_thread(
    item: &EtfValue,
    guild_id: u64,
    index: usize,
    diagnostics: &mut StateDiagnostics,
) -> Option<Thread> {
    let path = format!("threads[{index}]");
    if value::object(item).is_none() {
        diagnostics.malformed.push(path);
        return None;
    }
    let Some(id) = item.get("id").and_then(value::unsigned) else {
        diagnostics.malformed.push(path);
        return None;
    };
    if let Some(thread_guild) = item.get("guild_id").and_then(value::unsigned)
        && thread_guild != guild_id
    {
        diagnostics
            .inconsistencies
            .push(format!("{path}.guild_id:{thread_guild}!={guild_id}"));
    }

    let mut thread = Thread {
        id,
        guild_id,
        parent_id: lenient_unsigned(item, "parent_id", &path, diagnostics),
        name: field_text(item, "name", &path, diagnostics),
        thread_type: field_int(item, "type", &path, diagnostics).unwrap_or(0),
        owner_id: lenient_unsigned(item, "owner_id", &path, diagnostics),
        last_message_id: lenient_unsigned(item, "last_message_id", &path, diagnostics),
        message_count: field_int(item, "message_count", &path, diagnostics).unwrap_or(0),
        member_count: field_int(item, "member_count", &path, diagnostics).unwrap_or(0),
        total_message_sent: field_int(item, "total_message_sent", &path, diagnostics).unwrap_or(0),
        rate_limit_per_user: field_int(item, "rate_limit_per_user", &path, diagnostics),
        flags: field_int(item, "flags", &path, diagnostics).unwrap_or(0),
        member_ids_preview: field_ids(item, "member_ids_preview", &path, diagnostics)
            .unwrap_or_default(),
        ..Thread::default()
    };

    if let Some(metadata) = item.get("thread_metadata") {
        if value::is_null(metadata) {
            // nil — метаданных нет.
        } else if value::object(metadata).is_some() {
            let meta_path = format!("{path}.thread_metadata");
            thread.archived =
                field_bool(metadata, "archived", &meta_path, diagnostics).unwrap_or(false);
            thread.locked =
                field_bool(metadata, "locked", &meta_path, diagnostics).unwrap_or(false);
            thread.invitable =
                field_bool(metadata, "invitable", &meta_path, diagnostics).unwrap_or(false);
            thread.auto_archive_duration =
                field_int(metadata, "auto_archive_duration", &meta_path, diagnostics);
            thread.archive_timestamp =
                field_text(metadata, "archive_timestamp", &meta_path, diagnostics);
            thread.create_timestamp =
                field_text(metadata, "create_timestamp", &meta_path, diagnostics);
        } else {
            diagnostics.ignored.push(format!("{path}.thread_metadata"));
        }
    }
    Some(thread)
}

fn parse_thread_list_sync(
    data: &EtfValue,
) -> Result<(ThreadListSync, StateDiagnostics), EventError> {
    let map = event_map(data, "d")?;
    let guild_id = required_unsigned(map, "guild_id", "thread_list_sync.guild_id")?;
    let mut diagnostics = StateDiagnostics::default();
    let mut threads = Vec::new();

    if let Some(raw_threads) = map.get("threads")
        && !value::is_null(raw_threads)
    {
        let items = value::items(raw_threads).ok_or_else(|| EventError::WrongType {
            path: "thread_list_sync.threads".to_string(),
            expected: "list",
            found: value::kind(raw_threads),
        })?;
        for (index, item) in items.iter().enumerate() {
            if let Some(thread) = parse_thread(item, guild_id, index, &mut diagnostics) {
                threads.push(thread);
            }
        }
    }

    let mut positions: BTreeMap<u64, usize> = BTreeMap::new();
    let mut unique = Vec::with_capacity(threads.len());
    for thread in threads {
        if let Some(&position) = positions.get(&thread.id) {
            unique[position] = thread;
            diagnostics
                .duplicates
                .push(format!("threads:{}", unique[position].id));
        } else {
            positions.insert(thread.id, unique.len());
            unique.push(thread);
        }
    }

    Ok((
        ThreadListSync {
            guild_id,
            threads: unique,
        },
        diagnostics,
    ))
}

fn parse_ready_supplemental(
    data: &EtfValue,
) -> Result<(ReadySupplemental, StateDiagnostics), EventError> {
    let map = event_map(data, "d")?;
    let mut diagnostics = StateDiagnostics::default();
    let mut guild_ids = Vec::new();
    let mut voice_states = Vec::new();

    if let Some(raw_guilds) = map.get("guilds")
        && !value::is_null(raw_guilds)
    {
        let items = value::items(raw_guilds).ok_or_else(|| EventError::WrongType {
            path: "ready_supplemental.guilds".to_string(),
            expected: "list",
            found: value::kind(raw_guilds),
        })?;
        for (index, item) in items.iter().enumerate() {
            let id = item.get("id").and_then(value::unsigned);
            if id.is_none() {
                diagnostics.malformed.push(format!("guilds[{index}]"));
            }
            if let Some(raw_voice_states) = item.get("voice_states") {
                voice_states.extend(super::voice::parse_voice_states(
                    Some(raw_voice_states),
                    id,
                    &format!("ready_supplemental.guilds[{index}].voice_states"),
                    &mut diagnostics,
                ));
            }
            guild_ids.push(id);
        }
    }

    let merged_value = map.get("merged_members");
    let merged_is_list = merged_value.is_some_and(|raw| value::items(raw).is_some());
    let mut ready = ReadyDiagnostics::default();
    let merged_members = parse_merged_members(merged_value, &mut ready);
    diagnostics.merge(&StateDiagnostics::from_ready(&ready));
    if merged_is_list && merged_members.len() != guild_ids.len() {
        diagnostics.malformed.push(format!(
            "merged_members:{} vs guilds:{}",
            merged_members.len(),
            guild_ids.len()
        ));
    }
    let merged_presences =
        parse_merged_presences(map.get("merged_presences"), &guild_ids, &mut diagnostics);

    Ok((
        ReadySupplemental {
            guild_ids,
            merged_members,
            merged_presences,
            voice_states,
        },
        diagnostics,
    ))
}
