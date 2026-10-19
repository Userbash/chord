//! # Роутер диспатчей шлюза (RW-07, P1-06 / T-GW-08)
//!
//! op0-диспатчи → типизированные события по 21 наблюдаемому типу
//! ([`DispatchKind`], F-GW-06, `gateway-protocol.md` §4); неизвестный тип — не ошибка:
//! событие с `kind: None` (raw) и сохранённым именем, а накопительный счётчик ведёт пайп
//! ([`crate::ws::GatewayPipe::unknown_dispatches`]). Payload `d` не разбирается — это волна
//! P2-01 (T-DATA-01) — и передаётся вызывающему целиком.
//!
//! Оракул — union семи снимков `gw-proto/msgs-*.jsonl`: 1340 op0, 21 тип.

use discord_codec::etf::EtfValue;

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
