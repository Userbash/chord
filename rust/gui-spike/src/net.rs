// @ts-check
//! Сетевой слой спайка: токен → gateway-драйвер → статус → dispatch в UI.
//!
//! Контракт (`tests/net_contract.rs`): токен читается только через `TokenStore`,
//! статус никогда не содержит токен, close-коды 4004/4013/4014 классифицируются,
//! `disconnect` идемпотентен. Реальный драйвер (core supervisor) подключается
//! отдельно; в тестах — `FakeGateway`, без сети.
//!
//! L-02 (backend-срез dispatch → UI): `GatewayEvent::Dispatch` несёт
//! `discord_core::DispatchEvent`; [`map_dispatch`] пропускает канонические типы
//! ядра, вырезает секреты READY и конвертирует `d` в JSON-конверт
//! `{kind, data}`; [`DispatchChannel`] отдаёт его в UI-канал
//! [`DISPATCH_EVENT`] на отдельном потоке (try_send: источник не блокируется).
//!
//! Граница возможного (не фейк): core supervisor отдаёт жизненный цикл
//! (`Ready`/`Reconnecting`/`Stopped`) и — с L-02-CORE-FINAL — dispatch-кадры через
//! `SupervisorConfig::with_dispatch_handler` (`RunOptions.on_dispatch`, см.
//! `CONTRACTS/dispatch.md` §6). Поэтому [`CORE_DISPATCH_AVAILABLE`] = true: источник
//! живой, а не тестовый; `FakeGateway` в тестах идёт тем же путём.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use discord_core::codec::etf::EtfValue;
use discord_core::dispatch::{DispatchEvent, DispatchKind};
use discord_core::gateway::supervisor::{
    StopReason, Supervisor, SupervisorConfig, SupervisorEvent,
};
use discord_core::gateway::{
    encode_guild_subscriptions_bulk_chunked, encode_presence_update, encode_voice_state_update,
    GuildSubscriptionsBulk, OutboundGuildMembersRequest, OutboundGuildSubscription,
    OutboundPresenceUpdate, OutboundVoiceStateUpdate,
};
use discord_core::state::PresenceStatus;
use discord_core::ws::{encode_request_guild_members_frames, GatewayUrl};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Ошибка сетевого слоя. Токен в сообщениях/`Debug` не появляется.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetError {
    /// В keyring нет токена — сначала вход.
    MissingToken,
    /// Ошибка чтения хранилища.
    TokenStore(String),
    /// Ошибка подключения/драйвера.
    Gateway(String),
}

impl fmt::Display for NetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingToken => write!(formatter, "нет сохранённого токена"),
            Self::TokenStore(message) => write!(formatter, "хранилище токена: {message}"),
            Self::Gateway(message) => write!(formatter, "подключение к шлюзу: {message}"),
        }
    }
}

impl std::error::Error for NetError {}

/// Состояние соединения (без секретов).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatewayStatus {
    Offline,
    Connecting,
    Ready,
    Reconnecting,
    Error,
}

impl fmt::Display for GatewayStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::Offline => "offline",
            Self::Connecting => "connecting",
            Self::Ready => "ready",
            Self::Reconnecting => "reconnecting",
            Self::Error => "error",
        };
        write!(formatter, "{text}")
    }
}

/// Событие драйвера. `Ready` — сессия вошла; `Closed(code)` — шлюз закрыл соединение.
///
/// `Dispatch` — сырой op0 из ядра ([`DispatchEvent`]): фильтрацию, вырезание
/// секретов и конвертацию делает контроллер, драйвер ничего не переписывает.
#[derive(Debug, Clone, PartialEq)]
pub enum GatewayEvent {
    Ready,
    Reconnecting,
    Closed(u16),
    Error(String),
    Dispatch(DispatchEvent),
}

// ---------------------------------------------------------------------------
// L-02: dispatch → UI. Конверт `{kind, data}` — единственный контракт с UI:
// `kind` — имя провода (без выдуманных вариантов), `data` — `d` без
// переписывания полей (кроме вырезанных секретов READY).
// ---------------------------------------------------------------------------

/// Имя Tauri-события для диспатчей шлюза.
pub const DISPATCH_EVENT: &str = "gateway://dispatch";

/// Сколько событий ждёт медленный UI, прежде чем источник получает отказ.
pub const DISPATCH_QUEUE_CAPACITY: usize = 256;

/// 8 расширенных событий жизненного цикла каналов/тредов и удаления сообщений из PR-32
/// (`MESSAGE_DELETE`, `MESSAGE_DELETE_BULK`, `CHANNEL_CREATE`, `CHANNEL_UPDATE`, `CHANNEL_DELETE`,
/// `THREAD_CREATE`, `THREAD_UPDATE`, `THREAD_DELETE`).
const EXTENDED_DISPATCHES: [&str; 8] = [
    "CHANNEL_CREATE",
    "CHANNEL_UPDATE",
    "CHANNEL_DELETE",
    "THREAD_CREATE",
    "THREAD_UPDATE",
    "THREAD_DELETE",
    "MESSAGE_DELETE",
    "MESSAGE_DELETE_BULK",
];

/// Типы op0, доходящие до UI: все 21 канонических имени ядра ([`DispatchKind`])
/// плюс 8 расширенных событий жизненного цикла каналов/тредов и удаления сообщений из PR-32
/// и `MESSAGE_REACTION_REMOVE`.
pub const FORWARDED_DISPATCHES: [&str; 30] = [
    "CHANNEL_INFO",
    "GUILD_MEMBERS_CHUNK",
    "GUILD_MEMBER_LIST_UPDATE",
    "MESSAGE_ACK",
    "MESSAGE_CREATE",
    "MESSAGE_POLL_VOTE_ADD",
    "MESSAGE_REACTION_ADD",
    "MESSAGE_REACTION_REMOVE",
    "MESSAGE_UPDATE",
    "PRESENCE_UPDATE",
    "READY",
    "READY_SUPPLEMENTAL",
    "RESUMED",
    "SESSIONS_REPLACE",
    "SOUNDBOARD_SOUNDS",
    "THREAD_LIST_SYNC",
    "TYPING_START",
    "USER_SETTINGS_PROTO_UPDATE",
    "VOICE_CHANNEL_START_TIME_UPDATE",
    "VOICE_CHANNEL_STATUS_UPDATE",
    "VOICE_SERVER_UPDATE",
    "VOICE_STATE_UPDATE",
    EXTENDED_DISPATCHES[0],
    EXTENDED_DISPATCHES[1],
    EXTENDED_DISPATCHES[2],
    EXTENDED_DISPATCHES[3],
    EXTENDED_DISPATCHES[4],
    EXTENDED_DISPATCHES[5],
    EXTENDED_DISPATCHES[6],
    EXTENDED_DISPATCHES[7],
];

/// Отдаёт ли ядро диспатчи наружу (см. `CONTRACTS/dispatch.md` §6).
///
/// Да: `SupervisorConfig::with_dispatch_handler` дренирует `GatewayPipe::take_events`
/// и отдаёт op0 в `CoreGatewayDriver`, откуда они идут в контроллер.
pub const CORE_DISPATCH_AVAILABLE: bool = true;

/// Причина/механизм источника для диагностики/`net_dispatch_stats`.
pub const CORE_DISPATCH_DETAIL: &str =
    "supervisor ядра дренирует GatewayPipe::take_events и отдаёт op0 через RunOptions.on_dispatch";

/// Конверт события для UI: ровно `{kind, data}`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DispatchPayload {
    /// Имя типа в проводе: `READY`, `PRESENCE_UPDATE`, …
    pub kind: String,
    /// `d` без секретов: поля не добавляются и не переименовываются.
    pub data: Value,
}

/// Ошибка конвертации `d` в JSON (не паника: событие считается и отбрасывается).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchError {
    /// Ключ map нельзя представить строкой JSON-объекта.
    UnsupportedKey,
}

impl fmt::Display for DispatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedKey => {
                write!(formatter, "dispatch: ключ объекта не атом/строка/целое")
            }
        }
    }
}

impl std::error::Error for DispatchError {}

/// Сконвертированное событие и число вырезанных секретных ключей.
#[derive(Debug, Clone, PartialEq)]
pub struct MappedDispatch {
    /// Готовый конверт для UI.
    pub payload: DispatchPayload,
    /// Сколько ключей удалено политикой секретов (READY: session/qos/…).
    pub redacted: usize,
}

/// Ключ не должен покидать backend: токены, session/resume-возможности.
///
/// Сравнение по точному имени либо суффиксу `_token`; регистр — как в проводе.
pub fn is_secret_key(key: &str) -> bool {
    matches!(
        key,
        "session_id"
            | "resume_gateway_url"
            | "auth_session_id_hash"
            | "client_secret"
            | "password"
            | "token"
            | "access_token"
            | "refresh_token"
    ) || key.ends_with("_token")
}

/// ETF → JSON без падений: атом остаётся строкой (как в дампе `by-type`),
/// `Nil` → `null`, нечисловые ключи map — типизированная ошибка.
///
/// `Infinity`/`NaN` не представимы в JSON → `null` (значение не выдумывается:
/// в наблюдаемых payload'ах таких чисел нет).
pub fn etf_to_json(value: &EtfValue) -> Result<Value, DispatchError> {
    Ok(match value {
        EtfValue::Nil => Value::Null,
        EtfValue::Atom(text) | EtfValue::Str(text) => Value::String(text.clone()),
        EtfValue::Int(int) => {
            if int.unsigned_abs() > 9_007_199_254_740_991 {
                Value::String(int.to_string())
            } else {
                Value::Number((*int).into())
            }
        }
        EtfValue::Float(float) => serde_json::Number::from_f64(*float)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        EtfValue::Tuple(items) | EtfValue::List(items) => Value::Array(
            items
                .iter()
                .map(etf_to_json)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        EtfValue::Map(pairs) => {
            let mut object = serde_json::Map::new();
            for (key, nested) in pairs {
                let key = match key {
                    EtfValue::Atom(text) | EtfValue::Str(text) => text.clone(),
                    EtfValue::Int(int) => int.to_string(),
                    _ => return Err(DispatchError::UnsupportedKey),
                };
                object.insert(key, etf_to_json(nested)?);
            }
            Value::Object(object)
        }
    })
}

/// Рекурсивно вырезает секретные ключи; возвращает значение и счётчик.
fn redact_secrets(value: Value, redacted: &mut usize) -> Value {
    match value {
        Value::Object(map) => {
            let mut clean = serde_json::Map::new();
            for (key, nested) in map {
                if is_secret_key(&key) {
                    *redacted = redacted.saturating_add(1);
                    continue;
                }
                clean.insert(key, redact_secrets(nested, redacted));
            }
            Value::Object(clean)
        }
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| redact_secrets(item, redacted))
                .collect(),
        ),
        other => other,
    }
}

/// op0 ядра → конверт UI; `Ok(None)` — тип вне списка проброса (включая raw).
///
/// Порядок: фильтр по имени провода → `d` в JSON → вырезание секретов. Ни
/// одно поле не добавляется: UI получает ровно то, что пришло.
pub fn map_dispatch(event: &DispatchEvent) -> Result<Option<MappedDispatch>, DispatchError> {
    if !FORWARDED_DISPATCHES.contains(&event.name.as_str()) {
        return Ok(None);
    }
    let data = etf_to_json(&event.data)?;
    let mut redacted = 0;
    let data = redact_secrets(data, &mut redacted);
    Ok(Some(MappedDispatch {
        payload: DispatchPayload {
            kind: event.name.clone(),
            data,
        },
        redacted,
    }))
}

/// Псевдоним [`map_dispatch`] для единообразия именования BFF-обработчиков событий шлюза.
pub fn map_dispatch_event(event: &DispatchEvent) -> Result<Option<MappedDispatch>, DispatchError> {
    map_dispatch(event)
}

/// JSON → ETF для исходящих команд и тестовых фикстур.
pub fn json_to_etf(value: &Value) -> EtfValue {
    match value {
        Value::Null => EtfValue::Nil,
        Value::Bool(b) => EtfValue::Atom(b.to_string()),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                EtfValue::Int(i)
            } else if let Some(f) = n.as_f64() {
                EtfValue::Float(f)
            } else {
                EtfValue::Nil
            }
        }
        Value::String(s) => EtfValue::Str(s.clone()),
        Value::Array(arr) => EtfValue::List(arr.iter().map(json_to_etf).collect()),
        Value::Object(map) => EtfValue::Map(
            map.iter()
                .map(|(k, v)| (EtfValue::Str(k.clone()), json_to_etf(v)))
                .collect(),
        ),
    }
}

/// Куда уходит конверт: Tauri-событие в проде, fake-канал в тестах.
pub trait DispatchEmitter: Send + Sync {
    /// Ошибка не глотается: счётчик `emitFailed` + `lastError` в статистике.
    fn emit(&self, payload: &DispatchPayload) -> Result<(), String>;
}

/// Наблюдаемая статистика UI-канала (поток-эмиттер).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelStats {
    /// Принято в очередь (источник не ждал UI).
    pub queued: u64,
    /// Эмиттер отработал без ошибки.
    pub delivered: u64,
    /// Эмиттер вернул ошибку (не проглочена).
    pub emit_failed: u64,
    /// Очередь полна: событие отброшено, источник не блокировался.
    pub overflow: u64,
    /// Последняя причина отказа (без payload и секретов).
    pub last_error: Option<String>,
}

/// Очередь `gateway://dispatch` на выделенном потоке.
///
/// Источник (поток супервайзера) делает только `try_send`: медленный UI или
/// зависший emit не останавливает heartbeat и не блокирует runtime. Порядок
/// сохраняется (один потребитель). Поток завершается, когда сервис отпускает
/// канал.
pub struct DispatchChannel {
    sender: Option<SyncSender<DispatchPayload>>,
    stats: Arc<Mutex<ChannelStats>>,
}

impl DispatchChannel {
    /// `capacity` clamp-ится до ≥ 1; ошибка запуска потока не глотается
    /// (попадает в `lastError`, доставки считаются отказом).
    pub fn start(emitter: Arc<dyn DispatchEmitter>, capacity: usize) -> Self {
        let (sender, receiver) = sync_channel::<DispatchPayload>(capacity.max(1));
        let stats = Arc::new(Mutex::new(ChannelStats::default()));
        let worker_stats = Arc::clone(&stats);
        let spawned = thread::Builder::new()
            .name("gateway-dispatch".to_string())
            .spawn(move || {
                for payload in receiver {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        emitter.emit(&payload)
                    }))
                    .unwrap_or_else(|_| {
                        Err("паника при отправке события в UI-канал".to_string())
                    });
                    let mut stats = worker_stats
                        .lock()
                        .unwrap_or_else(|poison| poison.into_inner());
                    match result {
                        Ok(()) => stats.delivered = stats.delivered.saturating_add(1),
                        Err(message) => {
                            stats.emit_failed = stats.emit_failed.saturating_add(1);
                            stats.last_error = Some(message);
                        }
                    }
                }
            });
        if spawned.is_err() {
            stats
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .last_error = Some("не удалось запустить поток UI-канала".to_string());
        }
        Self {
            sender: Some(sender),
            stats,
        }
    }

    /// Отдать событие в UI. `false` — очередь полна/канал закрыт; вызов не
    /// блокируется и причина видна в [`Self::stats`].
    pub fn deliver(&self, payload: DispatchPayload) -> bool {
        let Some(sender) = self.sender.as_ref() else {
            return false;
        };
        let mut stats = self
            .stats
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        match sender.try_send(payload) {
            Ok(()) => {
                stats.queued = stats.queued.saturating_add(1);
                true
            }
            Err(TrySendError::Full(_)) => {
                stats.overflow = stats.overflow.saturating_add(1);
                stats.last_error = Some("очередь UI-канала переполнена".to_string());
                false
            }
            Err(TrySendError::Disconnected(_)) => {
                stats.emit_failed = stats.emit_failed.saturating_add(1);
                stats.last_error = Some("поток UI-канала завершён".to_string());
                false
            }
        }
    }

    /// Снимок статистики канала.
    pub fn stats(&self) -> ChannelStats {
        self.stats
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone()
    }
}

/// Счётчики контроллера: что пришло от драйвера и что с этим стало.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct DispatchMetrics {
    seen: u64,
    forwarded: u64,
    filtered: u64,
    duplicates: u64,
    unsupported: u64,
    redacted: u64,
}

#[derive(Debug, Clone, Default)]
struct GateState {
    /// Последний `s` текущего соединения (для отсева повторов).
    last_seq: Option<u64>,
    metrics: DispatchMetrics,
}

/// Приём диспатчей контроллером: дедуп по `s`, фильтр, конверт, UI-канал.
struct DispatchSink {
    channel: Option<Arc<DispatchChannel>>,
    state: Mutex<GateState>,
}

impl DispatchSink {
    /// Новое соединение: `s` начинается заново, дедуп не переносится.
    fn reset(&self) {
        self.state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .last_seq = None;
    }

    fn on_dispatch(&self, event: &DispatchEvent) {
        {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            state.metrics.seen = state.metrics.seen.saturating_add(1);
            if let Some(seq) = event.seq {
                if state.last_seq.is_some_and(|last| seq <= last) {
                    state.metrics.duplicates = state.metrics.duplicates.saturating_add(1);
                    return;
                }
                state.last_seq = Some(seq);
            }
        }
        if matches!(
            event.name.as_str(),
            "READY"
                | "RESUMED"
                | "READY_SUPPLEMENTAL"
                | "VOICE_STATE_UPDATE"
                | "VOICE_SERVER_UPDATE"
                | "STREAM_CREATE"
                | "STREAM_SERVER_UPDATE"
                | "STREAM_UPDATE"
                | "STREAM_DELETE"
        ) {
            if let Ok(raw_json) = etf_to_json(&event.data) {
                crate::data::ingest_raw_gateway_dispatch(&event.name, &raw_json);
            }
        }
        if matches!(
            event.name.as_str(),
            "CALL_CREATE"
                | "CALL_UPDATE"
                | "CALL_DELETE"
                | "GUILD_CREATE"
                | "GUILD_DELETE"
                | "RELATIONSHIP_ADD"
                | "RELATIONSHIP_REMOVE"
                | "STREAM_CREATE"
                | "STREAM_UPDATE"
                | "STREAM_DELETE"
        ) {
            if let Ok(raw_json) = etf_to_json(&event.data) {
                crate::data::ingest_live_dispatch(&event.name, &raw_json);
                if let Some(channel) = &self.channel {
                    channel.deliver(DispatchPayload {
                        kind: event.name.clone(),
                        data: raw_json,
                    });
                }
            }
        }
        match map_dispatch(event) {
            Ok(Some(mapped)) => {
                {
                    let mut state = self
                        .state
                        .lock()
                        .unwrap_or_else(|poison| poison.into_inner());
                    state.metrics.forwarded = state.metrics.forwarded.saturating_add(1);
                    state.metrics.redacted = state
                        .metrics
                        .redacted
                        .saturating_add(u64::try_from(mapped.redacted).unwrap_or(u64::MAX));
                }
                crate::data::ingest_live_dispatch(&mapped.payload.kind, &mapped.payload.data);
                if let Some(channel) = &self.channel {
                    channel.deliver(mapped.payload);
                }
            }
            Ok(None) => {
                let mut state = self
                    .state
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner());
                state.metrics.filtered = state.metrics.filtered.saturating_add(1);
            }
            Err(_) => {
                // Не паника и не тишина: битый `d` виден счётчиком.
                let mut state = self
                    .state
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner());
                state.metrics.unsupported = state.metrics.unsupported.saturating_add(1);
            }
        }
    }
}

/// Полная статистика L-02 для UI/диагностики (`net_dispatch_stats`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchStats {
    /// Подключён ли живой источник диспатчей в ядре (hook супервайзера).
    pub source_available: bool,
    /// Причина недоступности источника (не секрет).
    pub source_detail: &'static str,
    /// Диспатчей пришло от драйвера.
    pub seen: u64,
    /// Прошло фильтр, дедуп и конвертацию.
    pub forwarded: u64,
    /// Тип вне [`FORWARDED_DISPATCHES`] (включая raw).
    pub filtered: u64,
    /// Повтор `s` в текущем соединении.
    pub duplicates: u64,
    /// `d` не конвертируется (событие отброшено, не паника).
    pub unsupported: u64,
    /// Вырезано секретных ключей (READY).
    pub redacted: u64,
    /// Принято очередью UI.
    pub queued: u64,
    /// Эмиттер отработал без ошибки.
    pub delivered: u64,
    /// Эмиттер вернул ошибку (не проглочена).
    pub emit_failed: u64,
    /// Очередь переполнена: UI не успевает, источник не ждал.
    pub overflow: u64,
    /// Последняя ошибка канала UI (без payload/секретов).
    pub last_error: Option<String>,
}

/// Чтение токена: keyring в проде, статическое значение в тестах.
pub trait TokenStore: Send + Sync {
    fn load(&self) -> Result<Option<String>, NetError>;
    /// Удалить сохранённый токен (logout/close 4004).
    fn clear(&self) -> Result<(), NetError>;
}

/// Статическое хранилище для контрактных тестов (не пишет на диск).
#[derive(Clone)]
pub struct StaticTokenStore {
    token: Arc<Mutex<Option<String>>>,
}

impl StaticTokenStore {
    pub fn new(token: Option<&str>) -> Self {
        Self {
            token: Arc::new(Mutex::new(token.map(str::to_string))),
        }
    }
}

impl StaticTokenStore {
    /// Сменить токен в тесте (после 4004 хранилище очищено).
    pub fn set(&self, token: &str) {
        *self
            .token
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(token.to_string());
    }
}

impl TokenStore for StaticTokenStore {
    fn load(&self) -> Result<Option<String>, NetError> {
        Ok(self
            .token
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone())
    }

    fn clear(&self) -> Result<(), NetError> {
        *self
            .token
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = None;
        Ok(())
    }
}

/// TokenStore adapter backed only by AuthBackend's keyring read API.
pub struct AuthTokenStore(pub Arc<crate::auth::AuthBackend>);

impl TokenStore for AuthTokenStore {
    fn load(&self) -> Result<Option<String>, NetError> {
        self.0
            .token()
            .map_err(|_| NetError::TokenStore("не удалось прочитать токен".to_string()))
    }

    fn clear(&self) -> Result<(), NetError> {
        let state = self.0.logout();
        if matches!(state.status, crate::auth::AuthStatus::Error) {
            return Err(NetError::TokenStore(
                state
                    .error
                    .map(|error| error.to_string())
                    .unwrap_or_else(|| "не удалось очистить keyring".to_string()),
            ));
        }
        Ok(())
    }
}

/// Shared network service exposed to Tauri commands.
pub struct NetService {
    controller: NetController,
}

impl NetService {
    /// Без UI-канала: события фильтруются и считаются, но никуда не уходят.
    pub fn new(auth: Arc<crate::auth::AuthBackend>) -> Self {
        Self {
            controller: NetController::new(AuthTokenStore(auth), CoreGatewayDriver::new()),
        }
    }

    /// С каналом `gateway://dispatch`: эмиттер вызывается на своём потоке.
    pub fn with_emitter(
        auth: Arc<crate::auth::AuthBackend>,
        emitter: Arc<dyn DispatchEmitter>,
    ) -> Self {
        Self {
            controller: NetController::with_emitter(
                AuthTokenStore(auth),
                CoreGatewayDriver::new(),
                emitter,
                DISPATCH_QUEUE_CAPACITY,
            ),
        }
    }

    pub fn connect(&self) -> Result<GatewayStatus, NetError> {
        self.controller.connect()?;
        Ok(self.controller.status())
    }

    pub fn status(&self) -> GatewayStatus {
        self.controller.status()
    }

    pub fn disconnect(&self) -> Result<GatewayStatus, NetError> {
        self.controller.disconnect()?;
        Ok(self.controller.status())
    }

    /// Счётчики L-02 (флаг источника, фильтр, дедуп, UI-канал).
    pub fn dispatch_stats(&self) -> DispatchStats {
        self.controller.dispatch_stats()
    }
}

/// Драйвер шлюза: в проде — core supervisor, в тестах — `FakeGateway`.
pub trait GatewayDriver: Send + Sync {
    fn connect(
        &self,
        token: &str,
        events: Arc<dyn Fn(GatewayEvent) + Send + Sync>,
    ) -> Result<(), NetError>;
    fn disconnect(&self);
}

/// Production driver around the core supervisor. The runtime lives on its own thread.
#[derive(Default)]
pub struct CoreGatewayDriver {
    state: Mutex<Option<DriverTask>>,
}

/// Боевой конфиг супервайзера: жизненный цикл и dispatch-кадры в один callback.
///
/// Хук ядра (`SupervisorConfig::with_dispatch_handler` → `RunOptions.on_dispatch`)
/// отдаёт сырой `DispatchEvent` без переписывания имени и `d`. Вынесено из
/// [`CoreGatewayDriver::connect`], чтобы проводку можно было проверить без сети.
pub fn core_supervisor_config(
    token: &str,
    events: Arc<dyn Fn(GatewayEvent) + Send + Sync>,
) -> SupervisorConfig {
    let lifecycle = Arc::clone(&events);
    let dispatches = Arc::clone(&events);
    let identity = crate::auth::IdentityContext::detect();
    let mut cfg = SupervisorConfig::with_super_properties(
        GatewayUrl::default(),
        token.to_string(),
        &identity.super_properties,
    )
    .with_event_handler(move |event| lifecycle(map_supervisor_event(event)))
    .with_dispatch_handler(move |event| {
        // Сырой op0 ядра → контроллер: без переписывания имени/`d`.
        dispatches(GatewayEvent::Dispatch(event.clone()));
    });
    cfg.jitter_seed = 15_000;
    cfg
}

struct DriverTask {
    shutdown: tokio::sync::oneshot::Sender<()>,
    thread: JoinHandle<()>,
}

impl CoreGatewayDriver {
    pub fn new() -> Self {
        Self::default()
    }
}

impl GatewayDriver for CoreGatewayDriver {
    fn connect(
        &self,
        token: &str,
        events: Arc<dyn Fn(GatewayEvent) + Send + Sync>,
    ) -> Result<(), NetError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if state.is_some() {
            return Ok(());
        }
        let config = core_supervisor_config(token, Arc::clone(&events));
        let (shutdown, receiver) = tokio::sync::oneshot::channel();
        let thread = thread::Builder::new()
            .name("gateway-supervisor".to_string())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(_) => {
                        events(GatewayEvent::Error(
                            "не удалось запустить runtime шлюза".to_string(),
                        ));
                        return;
                    }
                };
                let run_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    runtime.block_on(async move {
                        Supervisor::new(config)
                            .run_until(async move {
                                let _ = receiver.await;
                            })
                            .await;
                    });
                }));
                if run_res.is_err() {
                    events(GatewayEvent::Error(
                        "поток шлюза перехватил внутреннюю панику".to_string(),
                    ));
                }
            })
            .map_err(|_| NetError::Gateway("не удалось запустить поток шлюза".to_string()))?;
        *state = Some(DriverTask { shutdown, thread });
        Ok(())
    }

    fn disconnect(&self) {
        let task = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .take();
        if let Some(task) = task {
            let _ = task.shutdown.send(());
            let _ = task.thread.join();
        }
    }
}

impl Drop for CoreGatewayDriver {
    fn drop(&mut self) {
        self.disconnect();
    }
}

pub fn map_supervisor_event(event: SupervisorEvent) -> GatewayEvent {
    match event {
        SupervisorEvent::Ready => GatewayEvent::Ready,
        SupervisorEvent::Reconnecting { .. } => GatewayEvent::Reconnecting,
        SupervisorEvent::Stopped(StopReason::Unauthorized) => GatewayEvent::Closed(4004),
        SupervisorEvent::Stopped(StopReason::InvalidClient { code }) => GatewayEvent::Closed(code),
        SupervisorEvent::Stopped(_) => GatewayEvent::Closed(4013),
    }
}

/// Контроллер сети: единая точка статуса и диспатчей для UI/команд.
pub struct NetController {
    store: Arc<dyn TokenStore>,
    driver: Arc<dyn GatewayDriver>,
    status: Arc<Mutex<GatewayStatus>>,
    sink: Arc<DispatchSink>,
}

impl NetController {
    /// Без UI-канала (статус-only): события считаются, но не эмитятся.
    pub fn new<S, G>(store: S, driver: G) -> Self
    where
        S: TokenStore + 'static,
        G: GatewayDriver + 'static,
    {
        Self::build(Arc::new(store), Arc::new(driver), None)
    }

    /// С UI-каналом `gateway://dispatch`; `capacity` — очередь до backpressure.
    pub fn with_emitter<S, G>(
        store: S,
        driver: G,
        emitter: Arc<dyn DispatchEmitter>,
        capacity: usize,
    ) -> Self
    where
        S: TokenStore + 'static,
        G: GatewayDriver + 'static,
    {
        Self::build(
            Arc::new(store),
            Arc::new(driver),
            Some(Arc::new(DispatchChannel::start(emitter, capacity))),
        )
    }

    fn build(
        store: Arc<dyn TokenStore>,
        driver: Arc<dyn GatewayDriver>,
        channel: Option<Arc<DispatchChannel>>,
    ) -> Self {
        Self {
            store,
            driver,
            status: Arc::new(Mutex::new(GatewayStatus::Offline)),
            sink: Arc::new(DispatchSink {
                channel,
                state: Mutex::new(GateState::default()),
            }),
        }
    }

    /// Текущий статус. Никогда не содержит токен.
    pub fn status(&self) -> GatewayStatus {
        *self
            .status
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    /// Счётчики L-02: транспорт, фильтр, дедуп, UI-канал.
    pub fn dispatch_stats(&self) -> DispatchStats {
        let (metrics, channel) = {
            let state = self
                .sink
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            let channel = self
                .sink
                .channel
                .as_ref()
                .map(|channel| channel.stats())
                .unwrap_or_default();
            (state.metrics.clone(), channel)
        };
        DispatchStats {
            source_available: CORE_DISPATCH_AVAILABLE,
            source_detail: CORE_DISPATCH_DETAIL,
            seen: metrics.seen,
            forwarded: metrics.forwarded,
            filtered: metrics.filtered,
            duplicates: metrics.duplicates,
            unsupported: metrics.unsupported,
            redacted: metrics.redacted,
            queued: channel.queued,
            delivered: channel.delivered,
            emit_failed: channel.emit_failed,
            overflow: channel.overflow,
            last_error: channel.last_error,
        }
    }

    /// Подключиться: токен из store → driver.connect.
    pub fn connect(&self) -> Result<(), NetError> {
        let token = self.store.load()?.ok_or(NetError::MissingToken)?;
        // SEC-L02-01: сброс last_seq на границе нового соединения до прихода Ready.
        self.sink.reset();
        self.set_status(GatewayStatus::Connecting);
        let status = Arc::clone(&self.status);
        let store = Arc::clone(&self.store);
        let sink = Arc::clone(&self.sink);
        let events = Arc::new(move |event: GatewayEvent| {
            let next = match event {
                // Диспатч — не статус: конверт уходит в UI-канал, состояние сети
                // не трогается (порядок событий сохраняет поток супервайзера).
                GatewayEvent::Dispatch(event) => {
                    if matches!(event.kind, Some(DispatchKind::Ready | DispatchKind::Resumed)) {
                        *status.lock().unwrap_or_else(|poison| poison.into_inner()) =
                            GatewayStatus::Ready;
                    }
                    sink.on_dispatch(&event);
                    return;
                }
                GatewayEvent::Ready => {
                    sink.reset();
                    GatewayStatus::Ready
                }
                GatewayEvent::Reconnecting => {
                    // Новое соединение: `s` начинается заново, дедуп сбрасывается.
                    sink.reset();
                    GatewayStatus::Reconnecting
                }
                GatewayEvent::Closed(4004) => {
                    sink.reset();
                    if store.clear().is_err() {
                        GatewayStatus::Error
                    } else {
                        GatewayStatus::Offline
                    }
                }
                GatewayEvent::Closed(4013 | 4014) => {
                    sink.reset();
                    GatewayStatus::Error
                }
                GatewayEvent::Closed(_) => {
                    sink.reset();
                    GatewayStatus::Reconnecting
                }
                GatewayEvent::Error(_) => GatewayStatus::Error,
            };
            *status.lock().unwrap_or_else(|poison| poison.into_inner()) = next;
        });
        if let Err(error) = self.driver.connect(&token, events) {
            self.set_status(GatewayStatus::Error);
            return Err(error);
        }
        Ok(())
    }

    /// Остановить соединение; повторный вызов — no-op.
    pub fn disconnect(&self) -> Result<(), NetError> {
        self.driver.disconnect();
        self.sink.reset();
        self.set_status(GatewayStatus::Offline);
        Ok(())
    }

    fn set_status(&self, next: GatewayStatus) {
        *self
            .status
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = next;
    }
}

#[derive(Default)]
struct FakeState {
    tokens: Vec<String>,
    logout: u32,
    stop: u32,
    fail: bool,
    callback: Option<Arc<dyn Fn(GatewayEvent) + Send + Sync>>,
    disconnects: u32,
}

/// Тестовый драйвер: без сети, с журналом identify/close.
#[derive(Clone, Default)]
pub struct FakeGateway {
    state: Arc<Mutex<FakeState>>,
}

impl FakeGateway {
    pub fn new() -> Self {
        Self::default()
    }

    /// Драйвер, который всегда падает на connect (ошибка без токена в тексте).
    pub fn failing() -> Self {
        let gateway = Self::default();
        gateway
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .fail = true;
        gateway
    }

    pub fn identify_tokens(&self) -> Vec<String> {
        self.state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .tokens
            .clone()
    }

    pub fn logout_count(&self) -> u32 {
        self.state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .logout
    }

    pub fn stop_count(&self) -> u32 {
        self.state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .stop
    }

    pub fn disconnect_count(&self) -> u32 {
        self.state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .disconnects
    }

    /// Сымитировать close-код от шлюза.
    pub fn close(&self, code: u16) {
        let callback = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            if code == 4004 {
                state.logout = state.logout.saturating_add(1);
            } else if code == 4013 || code == 4014 {
                state.stop = state.stop.saturating_add(1);
            }
            state.callback.clone()
        };
        if let Some(callback) = callback {
            callback(GatewayEvent::Closed(code));
        }
    }

    /// Сымитировать reconnect-событие.
    pub fn reconnecting(&self) {
        let callback = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .callback
            .clone();
        if let Some(callback) = callback {
            callback(GatewayEvent::Reconnecting);
        }
    }

    /// Сырой op0 без сети: драйвер не переписывает событие (как и боевой).
    ///
    /// `kind` выводится тем же роутером ядра (`DispatchKind::from_name`),
    /// поэтому неизвестное имя остаётся raw (`kind: None`, имя сохранено).
    pub fn dispatch(&self, name: &str, seq: u64, data: EtfValue) {
        let callback = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .callback
            .clone();
        if let Some(callback) = callback {
            let event = DispatchEvent {
                kind: DispatchKind::from_name(name),
                name: name.to_string(),
                seq: Some(seq),
                data,
            };
            callback(GatewayEvent::Dispatch(event));
        }
    }
}

impl GatewayDriver for FakeGateway {
    fn connect(
        &self,
        token: &str,
        events: Arc<dyn Fn(GatewayEvent) + Send + Sync>,
    ) -> Result<(), NetError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if state.fail {
            return Err(NetError::Gateway("тестовый шлюз недоступен".to_string()));
        }
        state.tokens.push(token.to_string());
        state.callback = Some(Arc::clone(&events));
        drop(state);
        events(GatewayEvent::Ready);
        Ok(())
    }

    fn disconnect(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        state.disconnects = state.disconnects.saturating_add(1);
        state.callback = None;
    }
}

// ---------------------------------------------------------------------------
// BFF: построение и валидация клиентских исходящих команд шлюза
// OP 3 (presence), OP 4 (voice state), OP 8 (request members), OP 37 (subscriptions)
// ---------------------------------------------------------------------------

/// Проверяет, что строка является валидной снежинкой Discord (1..20 десятичных цифр, влезает в u64).
pub fn is_snowflake(id: &str) -> bool {
    !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit()) && id.parse::<u64>().is_ok()
}

/// Ошибка построения/валидации клиентских исходящих команд шлюза BFF.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
pub enum GatewayCommandError {
    /// Поле содержит невалидный snowflake Discord.
    InvalidSnowflake { field: String, value: String },
    /// Невалидный статус presence.
    InvalidStatus { status: String },
    /// Обязательное поле пустое.
    EmptyField { field: String },
    /// Ошибка кодирования кадра wetf/etf.
    Encode(String),
}

impl fmt::Display for GatewayCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSnowflake { field, value } => {
                write!(
                    formatter,
                    "поле {field} содержит невалидный snowflake: '{value}'"
                )
            }
            Self::InvalidStatus { status } => {
                write!(formatter, "недопустимый статус presence: '{status}'")
            }
            Self::EmptyField { field } => {
                write!(formatter, "обязательное поле {field} не может быть пустым")
            }
            Self::Encode(msg) => write!(formatter, "ошибка кодирования кадра шлюза: {msg}"),
        }
    }
}

impl std::error::Error for GatewayCommandError {}

/// Команда обновления статуса и активности (OP 3 PRESENCE_UPDATE).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresenceCommand {
    /// Статус ("online", "idle", "dnd", "invisible", "offline").
    pub status: String,
    /// Unix-время миллисекунд активности (`0` по умолчанию).
    #[serde(default)]
    pub since: Option<u64>,
    /// Массив активностей (сырые JSON-значения, конвертируемые в ETF).
    #[serde(default)]
    pub activities: Option<Vec<Value>>,
    /// AFK-флаг.
    #[serde(default)]
    pub afk: Option<bool>,
}

impl PresenceCommand {
    /// Валидирует статус команды; допустимы: "online", "idle", "dnd", "invisible", "offline".
    pub fn validate(&self) -> Result<(), GatewayCommandError> {
        match self.status.as_str() {
            "online" | "idle" | "dnd" | "invisible" | "offline" => Ok(()),
            other => Err(GatewayCommandError::InvalidStatus {
                status: other.to_string(),
            }),
        }
    }

    /// Преобразует команду в структуру ядра [`OutboundPresenceUpdate`].
    pub fn build(&self) -> Result<OutboundPresenceUpdate, GatewayCommandError> {
        self.validate()?;
        let status = PresenceStatus::from_wire(&self.status);
        let activities = if let Some(items) = &self.activities {
            items.iter().map(json_to_etf).collect()
        } else {
            Vec::new()
        };
        Ok(OutboundPresenceUpdate {
            status,
            since: self.since.unwrap_or(0),
            activities,
            afk: self.afk.unwrap_or(false),
        })
    }

    /// Кодирует команду в ETF-кадр OP 3.
    pub fn encode(&self) -> Result<Vec<u8>, GatewayCommandError> {
        let update = self.build()?;
        encode_presence_update(&update).map_err(|e| GatewayCommandError::Encode(e.to_string()))
    }
}

/// Команда изменения голосового состояния (OP 4 VOICE_STATE_UPDATE).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceStateCommand {
    /// Snowflake id гильдии; `None` для выхода из голосового канала в ЛС.
    #[serde(default)]
    pub guild_id: Option<String>,
    /// Snowflake id целевого голосового канала; `None` для отключения.
    #[serde(default)]
    pub channel_id: Option<String>,
    /// Флаг локального отключения микрофона.
    #[serde(default)]
    pub self_mute: Option<bool>,
    /// Флаг локального отключения звука.
    #[serde(default)]
    pub self_deaf: Option<bool>,
    /// Флаг локальной трансляции камеры.
    #[serde(default)]
    pub self_video: Option<bool>,
    /// Битовая маска флагов голосового состояния (`0` по умолчанию).
    #[serde(default)]
    pub flags: Option<u64>,
}

impl VoiceStateCommand {
    /// Валидирует команду; если `guild_id` или `channel_id` заданы, они обязаны быть валидными snowflake.
    pub fn validate(&self) -> Result<(), GatewayCommandError> {
        if let Some(ref gid) = self.guild_id {
            if !is_snowflake(gid) {
                return Err(GatewayCommandError::InvalidSnowflake {
                    field: "guild_id".to_string(),
                    value: gid.clone(),
                });
            }
        }
        if let Some(ref cid) = self.channel_id {
            if !is_snowflake(cid) {
                return Err(GatewayCommandError::InvalidSnowflake {
                    field: "channel_id".to_string(),
                    value: cid.clone(),
                });
            }
        }
        Ok(())
    }

    /// Преобразует команду в структуру ядра [`OutboundVoiceStateUpdate`].
    pub fn build(&self) -> Result<OutboundVoiceStateUpdate, GatewayCommandError> {
        self.validate()?;
        Ok(OutboundVoiceStateUpdate {
            guild_id: self.guild_id.clone(),
            channel_id: self.channel_id.clone(),
            self_mute: self.self_mute.unwrap_or(false),
            self_deaf: self.self_deaf.unwrap_or(false),
            self_video: self.self_video.unwrap_or(false),
            flags: self.flags.unwrap_or(0),
        })
    }

    /// Кодирует команду в ETF-кадр OP 4.
    pub fn encode(&self) -> Result<Vec<u8>, GatewayCommandError> {
        let update = self.build()?;
        encode_voice_state_update(&update).map_err(|e| GatewayCommandError::Encode(e.to_string()))
    }
}

/// Команда запроса участников гильдии (OP 8 REQUEST_GUILD_MEMBERS).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestGuildMembersCommand {
    /// Snowflake id целевой гильдии (обязателен).
    pub guild_id: String,
    /// Префикс имени участников для поиска; взаимно дополняет `user_ids`.
    #[serde(default)]
    pub query: Option<String>,
    /// Максимальное число возвращаемых участников (`0` = без лимита на сервере).
    #[serde(default)]
    pub limit: Option<u64>,
    /// Точечный список snowflake id участников.
    #[serde(default)]
    pub user_ids: Option<Vec<String>>,
    /// Запрашивать ли presences участников (`false` по умолчанию).
    #[serde(default)]
    pub presences: Option<bool>,
    /// Необязательный nonce для сопоставления чанка.
    #[serde(default)]
    pub nonce: Option<String>,
}

impl RequestGuildMembersCommand {
    /// Валидирует команду: `guild_id` обязан быть непустым snowflake,
    /// а каждый id в `user_ids` (при наличии) — валидным snowflake.
    pub fn validate(&self) -> Result<(), GatewayCommandError> {
        if self.guild_id.trim().is_empty() {
            return Err(GatewayCommandError::EmptyField {
                field: "guild_id".to_string(),
            });
        }
        if !is_snowflake(&self.guild_id) {
            return Err(GatewayCommandError::InvalidSnowflake {
                field: "guild_id".to_string(),
                value: self.guild_id.clone(),
            });
        }
        if let Some(ref ids) = self.user_ids {
            for id in ids {
                if !is_snowflake(id) {
                    return Err(GatewayCommandError::InvalidSnowflake {
                        field: "user_ids".to_string(),
                        value: id.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Преобразует команду в структуру ядра [`OutboundGuildMembersRequest`].
    pub fn build(&self) -> Result<OutboundGuildMembersRequest, GatewayCommandError> {
        self.validate()?;
        Ok(OutboundGuildMembersRequest {
            guild_id: self.guild_id.clone(),
            query: self.query.clone(),
            limit: self.limit,
            user_ids: self.user_ids.clone().unwrap_or_default(),
            presences: self.presences.unwrap_or(false),
            nonce: self.nonce.clone(),
        })
    }

    /// Кодирует команду в ETF-кадры OP 8 с чанкованием `user_ids` по 100 id на кадр.
    pub fn encode_frames(&self) -> Result<Vec<Vec<u8>>, GatewayCommandError> {
        let request = self.build()?;
        encode_request_guild_members_frames(&request)
            .map_err(|e| GatewayCommandError::Encode(e.to_string()))
    }
}

/// Элемент подписки на одну гильдию (`SubscriptionManager.get(guild)` @9 112 505).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionEntry {
    /// Подписка на typing.
    #[serde(default)]
    pub typing: Option<bool>,
    /// Подписка на threads.
    #[serde(default)]
    pub threads: Option<bool>,
    /// Подписка на activities.
    #[serde(default)]
    pub activities: Option<bool>,
    /// Точечные id участников (ключи `_members`).
    #[serde(default)]
    pub members: Option<Vec<String>>,
    /// Подписка на member updates.
    #[serde(default)]
    pub member_updates: Option<bool>,
    /// id каналов (ключи `_channels`).
    #[serde(default)]
    pub channels: Option<Vec<String>>,
}

impl SubscriptionEntry {
    /// Валидирует все id участников и каналов на соответствие формату snowflake.
    pub fn validate(&self) -> Result<(), GatewayCommandError> {
        if let Some(ref members) = self.members {
            for mid in members {
                if !is_snowflake(mid) {
                    return Err(GatewayCommandError::InvalidSnowflake {
                        field: "members".to_string(),
                        value: mid.clone(),
                    });
                }
            }
        }
        if let Some(ref channels) = self.channels {
            for cid in channels {
                if !is_snowflake(cid) {
                    return Err(GatewayCommandError::InvalidSnowflake {
                        field: "channels".to_string(),
                        value: cid.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Преобразует запись подписки в структуру ядра [`OutboundGuildSubscription`].
    pub fn build(&self) -> Result<OutboundGuildSubscription, GatewayCommandError> {
        self.validate()?;
        Ok(OutboundGuildSubscription {
            typing: self.typing.unwrap_or(false),
            threads: self.threads.unwrap_or(false),
            activities: self.activities.unwrap_or(false),
            members: self.members.clone().unwrap_or_default(),
            member_updates: self.member_updates.unwrap_or(false),
            channels: self.channels.clone().unwrap_or_default(),
        })
    }
}

/// Команда обновления подписок гильдий (OP 37 GUILD_SUBSCRIPTIONS_BULK).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildSubscriptionsBulkCommand {
    /// Карта `guild_id -> SubscriptionEntry`.
    pub subscriptions: BTreeMap<String, SubscriptionEntry>,
}

impl GuildSubscriptionsBulkCommand {
    /// Валидирует команду: каждый ключ гильдии обязан быть snowflake, а внутренние id каналов/участников валидными.
    pub fn validate(&self) -> Result<(), GatewayCommandError> {
        for (gid, sub) in &self.subscriptions {
            if !is_snowflake(gid) {
                return Err(GatewayCommandError::InvalidSnowflake {
                    field: "subscriptions.key".to_string(),
                    value: gid.clone(),
                });
            }
            sub.validate()?;
        }
        Ok(())
    }

    /// Преобразует команду в словарь [`GuildSubscriptionsBulk`] ядра.
    pub fn build(&self) -> Result<GuildSubscriptionsBulk, GatewayCommandError> {
        self.validate()?;
        let mut bulk = GuildSubscriptionsBulk::new();
        for (gid, sub) in &self.subscriptions {
            bulk.insert(gid.clone(), sub.build()?);
        }
        Ok(bulk)
    }

    /// Кодирует команду в серию ETF-кадров OP 37 с автоматическим разбиением по лимиту 15 360 байт.
    pub fn encode_chunks(&self) -> Result<Vec<Vec<u8>>, GatewayCommandError> {
        let bulk = self.build()?;
        encode_guild_subscriptions_bulk_chunked(&bulk)
            .map_err(|e| GatewayCommandError::Encode(e.to_string()))
    }
}

/// Обобщённая клиентская исходящая команда шлюза (OP 3 / OP 4 / OP 8 / OP 37).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", content = "d")]
pub enum ClientGatewayCommand {
    /// OP 3 `PRESENCE_UPDATE`.
    #[serde(rename = "3")]
    PresenceUpdate(PresenceCommand),
    /// OP 4 `VOICE_STATE_UPDATE`.
    #[serde(rename = "4")]
    VoiceStateUpdate(VoiceStateCommand),
    /// OP 8 `REQUEST_GUILD_MEMBERS`.
    #[serde(rename = "8")]
    RequestGuildMembers(RequestGuildMembersCommand),
    /// OP 37 `GUILD_SUBSCRIPTIONS_BULK`.
    #[serde(rename = "37")]
    GuildSubscriptionsBulk(GuildSubscriptionsBulkCommand),
}

/// BFF-хелпер построения команды OP 3.
pub fn build_presence_command(
    cmd: &PresenceCommand,
) -> Result<OutboundPresenceUpdate, GatewayCommandError> {
    cmd.build()
}

/// BFF-хелпер валидации команды OP 3.
pub fn validate_presence_command(cmd: &PresenceCommand) -> Result<(), GatewayCommandError> {
    cmd.validate()
}

/// BFF-хелпер построения команды OP 4.
pub fn build_voice_state_command(
    cmd: &VoiceStateCommand,
) -> Result<OutboundVoiceStateUpdate, GatewayCommandError> {
    cmd.build()
}

/// BFF-хелпер валидации команды OP 4.
pub fn validate_voice_state_command(cmd: &VoiceStateCommand) -> Result<(), GatewayCommandError> {
    cmd.validate()
}

/// BFF-хелпер построения команды OP 8.
pub fn build_request_members_command(
    cmd: &RequestGuildMembersCommand,
) -> Result<OutboundGuildMembersRequest, GatewayCommandError> {
    cmd.build()
}

/// BFF-хелпер валидации команды OP 8.
pub fn validate_request_members_command(
    cmd: &RequestGuildMembersCommand,
) -> Result<(), GatewayCommandError> {
    cmd.validate()
}

/// BFF-хелпер построения команды OP 37.
pub fn build_guild_subscriptions_command(
    cmd: &GuildSubscriptionsBulkCommand,
) -> Result<GuildSubscriptionsBulk, GatewayCommandError> {
    cmd.build()
}

/// BFF-хелпер валидации команды OP 37.
pub fn validate_guild_subscriptions_command(
    cmd: &GuildSubscriptionsBulkCommand,
) -> Result<(), GatewayCommandError> {
    cmd.validate()
}
