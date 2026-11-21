#![forbid(unsafe_code)]

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
pub mod cache;
pub mod dispatch;
pub mod gateway;
pub mod gateway_events;
pub mod heartbeat;
pub mod history;
pub mod identify;
pub mod logging;
pub mod permissions;
pub mod ready;
pub mod rest;
pub mod resume;
pub mod runtime;
pub mod sessions_manager;
pub mod settings;
pub mod shutdown;
pub mod soundshare;
pub mod state;
pub mod stream;
pub mod threads;
pub mod totp_view;
pub mod ws;
pub mod ws_transport;

/// Ре-экспорт кодека: потребителям и интеграционным тестам `discord-core` не нужна
/// отдельная зависимость на `discord-codec`.
pub use chord_codec as codec;
pub use cache::*;
pub use runtime::{
    MediaThreadPool, MediaThreadPoolError, RegisteredThread, RuntimeConfig, RuntimeConfigError,
    RuntimeError, RuntimeManager, ThreadRegistry,
};
pub use shutdown::ShutdownCoordinator;
pub use gateway_events::*;
pub use twilight_model;
pub use dispatch::{DispatchEvent, DispatchKind, route_dispatch};
pub use gateway::supervisor::{
    Backoff, CloseAction, StopReason, Supervisor, SupervisorConfig, SupervisorEvent,
    SupervisorOutcome, classify_close,
};
pub use gateway::{
    Effect, GatewayError, HeartbeatMode, Hello, Op, QOS_HEARTBEAT_OP, QosPayload, ReconnectReason,
    Session,
};
pub use heartbeat::{
    DEFAULT_HEARTBEAT_INTERVAL_MS, HEARTBEAT_ACK_OP, HEARTBEAT_OP, HeartbeatAckOutcome,
    HeartbeatConfig, HeartbeatLoopControl, HeartbeatLoopError, HeartbeatLoopHandle,
    HeartbeatLoopOutcome, HeartbeatLoopSplit, HeartbeatPulse, HeartbeatSharedState,
    NO_SEQUENCE_SENTINEL, ZombieReconnectSignal, ZombieSocketDetector, heartbeat_loop,
    heartbeat_loop_with_state, spawn_heartbeat_loop,
};
pub use history::{
    Cursor, History, HistoryConfig, HistoryDiagnostics, HistoryError, LiveOutcome, MessagePage,
    PageApply, PageError, PageRequest, PageToken,
};
pub use identify::{
    CAPABILITIES_ACTIVE, CAPABILITIES_CHANNEL_OBFUSCATION, IDENTIFY_OP, IdentifyClientState,
    IdentifyError, IdentifyHandshakeFrame, IdentifyHandshakeGenerator, IdentifyParams,
    IdentifyProperties, IdentifySystemHeaders, PresenceInfo, build_identify, build_identify_frame,
    qos_token_client_provided,
};
pub use logging::{
    redact_secrets, RedactedMakeWriter, RedactedWriter, SecretRedactionLayer, REDACTED_MARKER,
};
pub use permissions::{
    ALL_PERMISSIONS, OVERWRITE_TYPE_MEMBER, OVERWRITE_TYPE_ROLE, PermissionCalculator,
    PermissionOverwrite, Permissions, compute_base_permissions, compute_channel_permissions,
    has_permission,
};
pub use twilight_util;
pub use ready::ReadySummary;
pub use rest::bridge::{
    DiscordHttpClient, DiscordRestBridge, DiscordRestBridgeError, RateLimitBucketState,
    build_relative_url_with_query, encode_rfc3986_query_value, validate_snowflake_id,
};
pub use rest::client::{ApiAuth, ApiAuthError, AuthScheme};
pub use rest::user::{Me, MeError, parse_me};
pub use resume::{
    RESUME_BACKOFF_MAX_MS, RESUME_BACKOFF_MIN_MS, RESUME_MAX_DOWNTIME_MS,
    RESUME_MAX_FAILURE_ATTEMPTS, RESUME_OP, RESUME_RESET_AFTER_FAILURES, ReconnectAction,
    ResumeError, ResumeExponentialBackoff, ResumeHandshakeFrame, ResumeParams,
    ResumeRecoveryOutcome, ResumeRecoveryReport, ResumeSessionCoordinator, ResumeSessionManager,
    ResumeState, build_resume_frame, build_resume_json_payload, plan_reconnect,
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
pub use ws_transport::{
    DEFAULT_CONNECT_TIMEOUT, DEFAULT_MAX_GATEWAY_WIRE_BYTES, DEFAULT_MAX_WRITE_BUFFER_BYTES,
    DEFAULT_SEND_TIMEOUT, GatewayInboundFrame, GatewayTransport, GatewayTransportConfig,
    GatewayTransportError, GatewayTransportMetrics, build_gateway_tls_connector,
    connect_gateway_stream, connect_gateway_transport, ensure_gateway_crypto_provider,
    validate_gateway_endpoint,
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
