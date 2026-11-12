//! # discord-core
//!
//! Транспорт и логика Discord: WebSocket-шлюз, REST v9, сессии, heartbeat, реконнект.
//!
//! Спека: `docs/re/gateway-protocol.md` §4–6, `docs/re/gateway-schema.md`.
//! План: P1 (транспорт), P2 (REST/данные); тесты: T-GW-*, T-REST-*.
//!
//! Статус: каркас P0 — константы, подтверждённые захватом; ядро сессии шлюза (RW-03/P1-03,
//! T-GW-01…03), транспорт (RW-04/P1-03: WS-клиент `ws://`/`wss://` с ring, пайп
//! zstd→ETF-термы→сессия, heartbeat/таймеры/исходы, T-GW-01/03) и идентификация (RW-05/P1-04:
//! IDENTIFY op2 по структуре веб-клиента, сводка READY, `connect_and_run_with`/`last_ready`,
//! T-GW-02/04) и роутер диспатчей (RW-07/P1-06: op0 → 21 типизированный вид, raw со счётчиком,
//! `events`/`take_events`, T-GW-08) реализованы и покрыты тестами.

pub mod audit_log;
pub mod dispatch;
pub mod gateway;
pub mod history;
pub mod identify;
pub mod permissions;
pub mod ready;
pub mod rest;
pub mod resume;
pub mod settings;
pub mod state;
pub mod stream;
pub mod ws;

/// Ре-экспорт кодека: потребителям и интеграционным тестам `discord-core` не нужна
/// отдельная зависимость на `discord-codec`.
pub use discord_codec as codec;
pub use dispatch::{DispatchEvent, DispatchKind, route_dispatch};
pub use gateway::supervisor::{
    Backoff, CloseAction, StopReason, Supervisor, SupervisorConfig, SupervisorEvent,
    SupervisorOutcome, classify_close,
};
pub use gateway::{
    Effect, GatewayError, HeartbeatMode, Hello, Op, QOS_HEARTBEAT_OP, QosPayload, ReconnectReason,
    Session,
};
pub use history::{
    Cursor, History, HistoryConfig, HistoryDiagnostics, HistoryError, LiveOutcome, MessagePage,
    PageApply, PageError, PageRequest, PageToken,
};
pub use identify::{
    CAPABILITIES_ACTIVE, CAPABILITIES_CHANNEL_OBFUSCATION, IdentifyError, IdentifyParams,
    IdentifyProperties, PresenceInfo, build_identify, build_identify_frame,
    qos_token_client_provided,
};
pub use permissions::{
    ALL_PERMISSIONS, OVERWRITE_TYPE_MEMBER, OVERWRITE_TYPE_ROLE, PermissionOverwrite, Permissions,
    compute_base_permissions, compute_channel_permissions, has_permission,
};
pub use ready::ReadySummary;
pub use rest::client::{ApiAuth, ApiAuthError, AuthScheme};
pub use rest::user::{Me, MeError, parse_me};
pub use resume::{
    RESUME_MAX_DOWNTIME_MS, ReconnectAction, ResumeError, ResumeParams, ResumeState,
    build_resume_frame, plan_reconnect,
};
pub use settings::{
    FrecencyUserSettings, PreloadedUserSettings, ProtoField, ProtoMessage, ProtoValue,
    SettingsProto, SettingsProtoError, decode_frecency_user_settings,
    decode_preloaded_user_settings, decode_settings,
};
pub use state::{
    Activity, ActivityAssets, ActivityEmoji, ActivityParty, ActivityTimestamps, ActivityType,
    Event, MemberChunk, MemberChunkAssembly, MemberChunkMember, MemberList, MemberListEntry,
    MemberListGroup, MemberListId, MemberListOp, MemberListRange, MemberListUpdate, MemberPresence,
    MergedPresences, MessageAck, Presence, PresenceKey, PresenceScope, PresenceStatus,
    PresenceUpdate, State, TYPING_TTL_MS, TypingEntry, TypingKey, TypingStart, VoiceChannelKey,
    VoiceChannelStartTime, VoiceChannelStatus, VoiceKey, VoiceState, VoiceStateUpdate, apply,
    apply_dispatch, apply_frame, apply_frame_at,
};
pub use ws::{
    GATEWAY_ENDPOINT, GATEWAY_QUERY, GatewayPipe, GatewayUrl, GatewayUrlError, RunOptions,
    RunOutcome, TransportError, connect_and_run, connect_and_run_resume, connect_and_run_with,
    encode_heartbeat, encode_qos_heartbeat,
};

/// База REST API v9 (47 подтверждённых маршрутов, 133 запроса в захвате D-02).
pub const API_V9_BASE: &str = "https://discord.com/api/v9";

/// Интервал heartbeat из op10 HELLO — **41 250 мс** (подтверждён в `gateway`).
pub const HEARTBEAT_INTERVAL_MS: u64 = 41_250;

/// Число ключей в READY (top-level) — 40 (`gateway/by-type/002-READY.json`).
pub const READY_TOP_LEVEL_KEYS: usize = 40;

/// Идентификаторы op-кодов шлюза, наблюдавшиеся в волне R7 (op0…op11).
pub const OBSERVED_OPCODES: [u8; 9] = [0, 1, 2, 4, 6, 7, 9, 10, 11];

/// Всего op0-диспатчей в захвате (union 7 сокетов) — 1 340.
pub const DISPATCH_COUNT_R7: u32 = 1_340;

/// Типов диспатчей (op0) в захвате — 21 (`gateway-protocol.md` §4).
pub const DISPATCH_TYPE_COUNT_R7: usize = 21;

#[cfg(test)]
mod tests {
    #[test]
    fn gateway_constants_match_capture() {
        assert_eq!(super::HEARTBEAT_INTERVAL_MS, 41_250);
        assert_eq!(super::READY_TOP_LEVEL_KEYS, 40);
        assert_eq!(super::DISPATCH_TYPE_COUNT_R7, 21);
    }
}
