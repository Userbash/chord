//! Высокопроизводительный асинхронный и мультипоточный диспетчер задач.
//! Обеспечивает неблокирующий I/O, фоновые воркеры на базе `tokio` и `async-channel`.
#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use chord_codec::etf::EtfValue;
use chord_core::dispatch::{DispatchEvent, DispatchKind, TypedDispatch, parse_typed_dispatch_etf};
use chord_json::JsonValue;

/// Обёртка для голосового токена `VOICE_SERVER_UPDATE`, скрывающая секрет в `Debug`/`Display`.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct RedactedVoiceToken(String);

impl RedactedVoiceToken {
    /// Создать обёртку над сырым голосовым токеном.
    #[must_use]
    pub fn new(token: impl Into<String>) -> Self {
        Self(token.into())
    }

    /// Явный доступ к секретному голосовому токену для передачи в голосовой транспорт.
    #[must_use]
    pub fn expose(&self) -> &str {
        self.0.as_str()
    }

    /// Пуст ли токен.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for RedactedVoiceToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl fmt::Display for RedactedVoiceToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

/// Тип события, поступающего из фонового асинхронного воркера в GPUI AppState
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackgroundEvent {
    GatewayMessage(String, String), // channel_id, content
    PresenceUpdate(String, String), // user_id, status
    VoicePing(u32),                 // ping_ms
    ImageDecoded(String, u32, u32), // cache_key, width, height
    TaskCompleted(String),          // task_id
    GatewayReady {
        session_id: String,
        resume_gateway_url: Option<String>,
        user_id: String,
        username: String,
        guild_count: usize,
    },
    GatewayResumed {
        seq: u64,
    },
    GatewayMessageCreate {
        id: String,
        channel_id: String,
        guild_id: Option<String>,
        author_id: String,
        author_name: String,
        content: String,
        timestamp: Option<String>,
    },
    GatewayMessageUpdate {
        id: String,
        channel_id: String,
        guild_id: Option<String>,
        content: Option<String>,
        edited_timestamp: Option<String>,
    },
    GatewayMessageDelete {
        ids: Vec<String>,
        channel_id: String,
        guild_id: Option<String>,
    },
    GatewayChannelLifecycle {
        event_name: String,
        id: String,
        guild_id: Option<String>,
        kind: Option<i64>,
        name: Option<String>,
    },
    GatewayTypingStart {
        channel_id: String,
        guild_id: Option<String>,
        user_id: String,
        timestamp: u64,
    },
    GatewayVoiceStateUpdate {
        guild_id: Option<String>,
        channel_id: Option<String>,
        user_id: String,
        session_id: String,
        self_mute: bool,
        self_deaf: bool,
        self_video: bool,
    },
    GatewayVoiceServerUpdate {
        guild_id: Option<String>,
        endpoint: Option<String>,
        token_present: bool,
    },
    GatewayDisconnected {
        close_code: Option<u16>,
        resumable: bool,
        reason: String,
    },
}

/// Исход неблокирующей пересылки `DispatchEvent -> BackgroundEvent` в канал GPUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForwardOutcome {
    /// Событие распознано и отправлено в канал `BackgroundEvent`.
    Sent,
    /// Событие не подлежит пробросу или имеет неполную/некорректную нагрузку (`map_dispatch_event == None`).
    Ignored,
    /// Очередь `BackgroundEvent` переполнена (backpressure): событие отброшено без блокировки цикла шлюза.
    ChannelFull,
    /// Канал `BackgroundEvent` закрыт получателем.
    ChannelClosed,
}

impl ForwardOutcome {
    #[must_use]
    pub fn is_sent(self) -> bool {
        matches!(self, Self::Sent)
    }

    #[must_use]
    pub fn is_ignored(self) -> bool {
        matches!(self, Self::Ignored)
    }

    #[must_use]
    pub fn is_dropped(self) -> bool {
        matches!(self, Self::ChannelFull | Self::ChannelClosed)
    }
}

/// Наблюдаемые метрики моста `DispatchEvent -> BackgroundEvent`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DispatchBridgeMetrics {
    /// Успешно доставлено в очередь `BackgroundEvent`.
    pub sent: u64,
    /// Отфильтровано (`map_dispatch_event == None`).
    pub ignored: u64,
    /// Суммарно отброшено из-за `ChannelFull` или `ChannelClosed`.
    pub dropped: u64,
    /// Отброшено из-за переполнения ограниченного канала (`ChannelFull`).
    pub channel_full: u64,
    /// Отброшено из-за закрытия канала (`ChannelClosed`).
    pub channel_closed: u64,
}

impl DispatchBridgeMetrics {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Учесть результат одной попытки пересылки.
    pub fn record(&mut self, outcome: ForwardOutcome) {
        match outcome {
            ForwardOutcome::Sent => self.sent = self.sent.saturating_add(1),
            ForwardOutcome::Ignored => self.ignored = self.ignored.saturating_add(1),
            ForwardOutcome::ChannelFull => {
                self.dropped = self.dropped.saturating_add(1);
                self.channel_full = self.channel_full.saturating_add(1);
            }
            ForwardOutcome::ChannelClosed => {
                self.dropped = self.dropped.saturating_add(1);
                self.channel_closed = self.channel_closed.saturating_add(1);
            }
        }
    }

    /// Переслать событие в `tx` и обновить счётчики метрик.
    pub fn forward(
        &mut self,
        tx: &async_channel::Sender<BackgroundEvent>,
        event: &DispatchEvent,
    ) -> ForwardOutcome {
        forward_dispatch_event_with_metrics(tx, event, self)
    }

    #[must_use]
    pub fn sent_count(&self) -> u64 {
        self.sent
    }

    #[must_use]
    pub fn ignored_count(&self) -> u64 {
        self.ignored
    }

    #[must_use]
    pub fn dropped_count(&self) -> u64 {
        self.dropped
    }
}

#[derive(Default)]
struct AtomicBridgeMetrics {
    sent: AtomicU64,
    ignored: AtomicU64,
    channel_full: AtomicU64,
    channel_closed: AtomicU64,
}

impl AtomicBridgeMetrics {
    fn record(&self, outcome: ForwardOutcome) {
        match outcome {
            ForwardOutcome::Sent => {
                self.sent.fetch_add(1, Ordering::Relaxed);
            }
            ForwardOutcome::Ignored => {
                self.ignored.fetch_add(1, Ordering::Relaxed);
            }
            ForwardOutcome::ChannelFull => {
                self.channel_full.fetch_add(1, Ordering::Relaxed);
            }
            ForwardOutcome::ChannelClosed => {
                self.channel_closed.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn snapshot(&self) -> DispatchBridgeMetrics {
        let sent = self.sent.load(Ordering::Relaxed);
        let ignored = self.ignored.load(Ordering::Relaxed);
        let channel_full = self.channel_full.load(Ordering::Relaxed);
        let channel_closed = self.channel_closed.load(Ordering::Relaxed);
        DispatchBridgeMetrics {
            sent,
            ignored,
            dropped: channel_full.saturating_add(channel_closed),
            channel_full,
            channel_closed,
        }
    }

    fn reset(&self) {
        self.sent.store(0, Ordering::Relaxed);
        self.ignored.store(0, Ordering::Relaxed);
        self.channel_full.store(0, Ordering::Relaxed);
        self.channel_closed.store(0, Ordering::Relaxed);
    }
}

/// Асинхронный диспетчер задач с пулом воркеров
pub struct AsyncTaskDispatcher {
    tx: async_channel::Sender<BackgroundEvent>,
    rx: async_channel::Receiver<BackgroundEvent>,
    bridge_metrics: Arc<AtomicBridgeMetrics>,
}

impl Default for AsyncTaskDispatcher {
    fn default() -> Self {
        Self::new(1024)
    }
}

impl AsyncTaskDispatcher {
    pub fn new(capacity: usize) -> Self {
        let (tx, rx) = async_channel::bounded(capacity);
        Self {
            tx,
            rx,
            bridge_metrics: Arc::new(AtomicBridgeMetrics::default()),
        }
    }

    /// Получить отправителя для фоновых задач
    pub fn sender(&self) -> async_channel::Sender<BackgroundEvent> {
        self.tx.clone()
    }

    /// Получить получателя для интеграции с циклом GPUI
    pub fn receiver(&self) -> async_channel::Receiver<BackgroundEvent> {
        self.rx.clone()
    }

    /// Неблокирующая отправка события
    pub fn try_send(
        &self,
        event: BackgroundEvent,
    ) -> Result<(), async_channel::TrySendError<BackgroundEvent>> {
        self.tx.try_send(event)
    }

    /// Неблокирующее чтение событий для основного потока отрисовки (drain batch)
    pub fn drain_events(&self) -> Vec<BackgroundEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.rx.try_recv() {
            events.push(event);
        }
        events
    }

    /// Неблокирующая маршрутизация и отправка живого `chord_core::DispatchEvent` в очередь GPUI
    /// с обновлением наблюдаемых метрик [`DispatchBridgeMetrics`].
    pub fn forward_dispatch_event(&self, event: &DispatchEvent) -> ForwardOutcome {
        let outcome = forward_dispatch_event(&self.tx, event);
        self.bridge_metrics.record(outcome);
        outcome
    }

    /// Псевдоним [`Self::forward_dispatch_event`].
    pub fn forward_dispatch(&self, event: &DispatchEvent) -> ForwardOutcome {
        self.forward_dispatch_event(event)
    }

    /// Снимок метрик моста `DispatchEvent -> BackgroundEvent` (включая счётчик `dropped`).
    #[must_use]
    pub fn bridge_metrics(&self) -> DispatchBridgeMetrics {
        self.bridge_metrics.snapshot()
    }

    /// Число отброшенных из-за backpressure/закрытия канала событий шлюза.
    #[must_use]
    pub fn dropped_count(&self) -> u64 {
        self.bridge_metrics.snapshot().dropped
    }

    /// Сброс наблюдаемых счётчиков моста.
    pub fn reset_bridge_metrics(&self) {
        self.bridge_metrics.reset();
    }

    /// Создать неблокирующий [`chord_core::ws::DispatchHandler`] для подключения к циклу шлюза.
    #[must_use]
    pub fn dispatch_handler(&self) -> chord_core::ws::DispatchHandler {
        let tx = self.tx.clone();
        let metrics = Arc::clone(&self.bridge_metrics);
        Arc::new(move |event: &DispatchEvent| {
            let outcome = forward_dispatch_event(&tx, event);
            metrics.record(outcome);
        })
    }
}

fn effective_dispatch_name(event: &DispatchEvent) -> Option<&str> {
    if !event.name.is_empty() {
        if let Some(kind) = event.kind
            && kind.as_str() != event.name.as_str()
        {
            return None;
        }
        Some(event.name.as_str())
    } else {
        event.kind.map(DispatchKind::as_str)
    }
}

fn etf_is_null(value: &EtfValue) -> bool {
    matches!(value, EtfValue::Nil) || matches!(value, EtfValue::Atom(name) if name == "nil")
}

fn etf_text(value: &EtfValue) -> Option<&str> {
    match value {
        EtfValue::Str(text) => Some(text.as_str()),
        EtfValue::Atom(name) if !matches!(name.as_str(), "nil" | "true" | "false") => {
            Some(name.as_str())
        }
        _ => None,
    }
}

fn etf_parse_u64(value: &EtfValue) -> Option<u64> {
    match value {
        EtfValue::Int(number) if *number >= 0 => u64::try_from(*number).ok(),
        EtfValue::Str(text) | EtfValue::Atom(text) => (!text.is_empty()
            && text.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| text.parse::<u64>().ok())
        .flatten(),
        _ => None,
    }
}

fn etf_parse_snowflake(value: &EtfValue) -> Option<String> {
    etf_parse_u64(value).map(|id| id.to_string())
}

fn etf_required_snowflake(map: &EtfValue, field: &str) -> Option<String> {
    let raw = map.get(field)?;
    if etf_is_null(raw) {
        return None;
    }
    etf_parse_snowflake(raw)
}

fn etf_optional_snowflake(map: &EtfValue, field: &str) -> Option<Option<String>> {
    let Some(raw) = map.get(field) else {
        return Some(None);
    };
    if etf_is_null(raw) {
        return Some(None);
    }
    etf_parse_snowflake(raw).map(Some)
}

fn etf_required_non_empty_string(map: &EtfValue, field: &str) -> Option<String> {
    let raw = map.get(field)?;
    if etf_is_null(raw) {
        return None;
    }
    let text = etf_text(raw)?;
    if text.is_empty() {
        return None;
    }
    Some(text.to_string())
}

fn etf_required_string_allow_empty(map: &EtfValue, field: &str) -> Option<String> {
    let raw = map.get(field)?;
    if etf_is_null(raw) {
        return None;
    }
    etf_text(raw).map(str::to_string)
}

fn etf_optional_non_empty_string(map: &EtfValue, field: &str) -> Option<Option<String>> {
    let Some(raw) = map.get(field) else {
        return Some(None);
    };
    if etf_is_null(raw) {
        return Some(None);
    }
    let text = etf_text(raw)?;
    if text.is_empty() {
        return Some(None);
    }
    Some(Some(text.to_string()))
}

fn etf_optional_string_allow_empty(map: &EtfValue, field: &str) -> Option<Option<String>> {
    let Some(raw) = map.get(field) else {
        return Some(None);
    };
    if etf_is_null(raw) {
        return Some(None);
    }
    etf_text(raw).map(|text| Some(text.to_string()))
}

fn etf_optional_bool(map: &EtfValue, field: &str) -> Option<Option<bool>> {
    let Some(raw) = map.get(field) else {
        return Some(None);
    };
    if etf_is_null(raw) {
        return Some(None);
    }
    match raw {
        EtfValue::Atom(name) | EtfValue::Str(name) if name == "true" => Some(Some(true)),
        EtfValue::Atom(name) | EtfValue::Str(name) if name == "false" => Some(Some(false)),
        EtfValue::Int(1) => Some(Some(true)),
        EtfValue::Int(0) => Some(Some(false)),
        _ => None,
    }
}

fn json_value_to_etf(value: &JsonValue) -> EtfValue {
    match value {
        JsonValue::Null => EtfValue::Nil,
        JsonValue::Bool(flag) => EtfValue::Atom(flag.to_string()),
        JsonValue::Number(num) => {
            if let Some(i) = num.as_i64() {
                EtfValue::Int(i)
            } else if let Some(u) = num.as_u64() {
                EtfValue::Str(u.to_string())
            } else if let Some(f) = num.as_f64() {
                EtfValue::Float(f)
            } else {
                EtfValue::Str(num.as_str().to_string())
            }
        }
        JsonValue::String(text) => EtfValue::Str(text.clone()),
        JsonValue::Array(items) => EtfValue::List(items.iter().map(json_value_to_etf).collect()),
        JsonValue::Object(entries) => EtfValue::Map(
            entries
                .iter()
                .map(|(k, v)| (EtfValue::Str(k.clone()), json_value_to_etf(v)))
                .collect(),
        ),
    }
}

fn normalize_payload_data(data: &EtfValue) -> Cow<'_, EtfValue> {
    if let EtfValue::Str(raw_json) = data
        && raw_json.trim_start().starts_with('{')
        && let Ok(parsed) = chord_json::parse(raw_json)
    {
        return Cow::Owned(json_value_to_etf(&parsed));
    }
    Cow::Borrowed(data)
}

/// Извлечь защищённый голосовой токен [`RedactedVoiceToken`] из `VOICE_SERVER_UPDATE`,
/// если событие является валидным `VOICE_SERVER_UPDATE` и содержит непустой токен.
#[must_use]
pub fn extract_voice_server_token(event: &DispatchEvent) -> Option<RedactedVoiceToken> {
    let name = effective_dispatch_name(event)?;
    if name != "VOICE_SERVER_UPDATE" {
        return None;
    }
    let data = normalize_payload_data(&event.data);
    data.as_map()?;
    let raw_token = data.get("token")?;
    if etf_is_null(raw_token) {
        return None;
    }
    let token = etf_text(raw_token)?;
    if token.is_empty() {
        return None;
    }
    Some(RedactedVoiceToken::new(token))
}

/// Сформировать `BackgroundEvent::GatewayDisconnected` с автоматической классификацией `resumable`
/// по коду закрытия (`chord_core::classify_close`).
#[must_use]
pub fn map_gateway_disconnect(
    close_code: Option<u16>,
    reason: impl Into<String>,
) -> BackgroundEvent {
    let resumable = matches!(
        chord_core::classify_close(close_code),
        chord_core::CloseAction::Reconnect
    );
    BackgroundEvent::GatewayDisconnected {
        close_code,
        resumable,
        reason: reason.into(),
    }
}

/// Чистый маппер события шлюза `chord_core::DispatchEvent` в типизированный `BackgroundEvent` для GPUI.
///
/// Возвращает `None` (никогда не паникует) для неизвестных событий шлюза или неполных/повреждённых полезных нагрузок.
#[must_use]
pub fn map_dispatch_event(event: &DispatchEvent) -> Option<BackgroundEvent> {
    let event_name = effective_dispatch_name(event)?;
    let data_cow = normalize_payload_data(&event.data);
    let data = data_cow.as_ref();

    match event_name {
        "READY" => {
            data.as_map()?;
            let session_id = etf_required_non_empty_string(data, "session_id")?;
            let resume_gateway_url = etf_optional_non_empty_string(data, "resume_gateway_url")?;
            let user = data.get("user")?;
            user.as_map()?;
            let user_id = etf_required_snowflake(user, "id")?;
            let username = etf_required_non_empty_string(user, "username")
                .or_else(|| etf_required_non_empty_string(user, "global_name"))?;
            let guild_count = match data.get("guilds") {
                None => 0,
                Some(raw) if etf_is_null(raw) => 0,
                Some(EtfValue::List(items) | EtfValue::Tuple(items)) => items.len(),
                Some(_) => return None,
            };
            Some(BackgroundEvent::GatewayReady {
                session_id,
                resume_gateway_url,
                user_id,
                username,
                guild_count,
            })
        }
        "RESUMED" => {
            if !etf_is_null(data) && data.as_map().is_none() {
                return None;
            }
            let seq = event
                .seq
                .or_else(|| data.get("seq").and_then(etf_parse_u64))
                .or_else(|| data.get("s").and_then(etf_parse_u64))
                .unwrap_or(0);
            Some(BackgroundEvent::GatewayResumed { seq })
        }
        "MESSAGE_CREATE" => {
            data.as_map()?;
            let id = etf_required_snowflake(data, "id")?;
            let channel_id = etf_required_snowflake(data, "channel_id")?;
            let guild_id = etf_optional_snowflake(data, "guild_id")?;
            let (author_id, author_name) = if let Some(author) = data.get("author")
                && !etf_is_null(author)
            {
                author.as_map()?;
                let aid = etf_required_snowflake(author, "id")?;
                let aname = etf_required_non_empty_string(author, "username")
                    .or_else(|| etf_required_non_empty_string(author, "global_name"))?;
                (aid, aname)
            } else {
                let aid = etf_required_snowflake(data, "author_id")?;
                let aname = etf_required_non_empty_string(data, "author_name")?;
                (aid, aname)
            };
            let content = etf_required_string_allow_empty(data, "content")?;
            let timestamp = etf_optional_non_empty_string(data, "timestamp")?;
            Some(BackgroundEvent::GatewayMessageCreate {
                id,
                channel_id,
                guild_id,
                author_id,
                author_name,
                content,
                timestamp,
            })
        }
        "MESSAGE_UPDATE" => {
            data.as_map()?;
            let id = etf_required_snowflake(data, "id")?;
            let channel_id = etf_required_snowflake(data, "channel_id")?;
            let guild_id = etf_optional_snowflake(data, "guild_id")?;
            let content = etf_optional_string_allow_empty(data, "content")?;
            let edited_timestamp = etf_optional_non_empty_string(data, "edited_timestamp")?;
            Some(BackgroundEvent::GatewayMessageUpdate {
                id,
                channel_id,
                guild_id,
                content,
                edited_timestamp,
            })
        }
        "MESSAGE_DELETE" => match parse_typed_dispatch_etf("MESSAGE_DELETE", data)? {
            Ok(TypedDispatch::MessageDelete(del)) => Some(BackgroundEvent::GatewayMessageDelete {
                ids: vec![del.id.to_string()],
                channel_id: del.channel_id.to_string(),
                guild_id: del.guild_id.map(|g| g.to_string()),
            }),
            _ => None,
        },
        "MESSAGE_DELETE_BULK" => match parse_typed_dispatch_etf("MESSAGE_DELETE_BULK", data)? {
            Ok(TypedDispatch::MessageDeleteBulk(bulk)) => {
                Some(BackgroundEvent::GatewayMessageDelete {
                    ids: bulk.ids.into_iter().map(|id| id.to_string()).collect(),
                    channel_id: bulk.channel_id.to_string(),
                    guild_id: bulk.guild_id.map(|g| g.to_string()),
                })
            }
            _ => None,
        },
        "CHANNEL_CREATE" | "CHANNEL_UPDATE" | "CHANNEL_DELETE" => {
            match parse_typed_dispatch_etf(event_name, data)? {
                Ok(
                    TypedDispatch::ChannelCreate(ch)
                    | TypedDispatch::ChannelUpdate(ch)
                    | TypedDispatch::ChannelDelete(ch),
                ) => Some(BackgroundEvent::GatewayChannelLifecycle {
                    event_name: event_name.to_string(),
                    id: ch.id.to_string(),
                    guild_id: ch.guild_id.map(|g| g.to_string()),
                    kind: ch.kind,
                    name: ch.name,
                }),
                _ => None,
            }
        }
        "PRESENCE_UPDATE" => {
            data.as_map()?;
            let user_id = if let Some(user) = data.get("user")
                && !etf_is_null(user)
            {
                user.as_map()?;
                etf_required_snowflake(user, "id")?
            } else {
                etf_required_snowflake(data, "user_id")?
            };
            let status = etf_required_non_empty_string(data, "status")?;
            Some(BackgroundEvent::PresenceUpdate(user_id, status))
        }
        "TYPING_START" => {
            data.as_map()?;
            let channel_id = etf_required_snowflake(data, "channel_id")?;
            let guild_id = etf_optional_snowflake(data, "guild_id")?;
            let user_id = etf_required_snowflake(data, "user_id")?;
            let timestamp = data.get("timestamp").and_then(etf_parse_u64)?;
            Some(BackgroundEvent::GatewayTypingStart {
                channel_id,
                guild_id,
                user_id,
                timestamp,
            })
        }
        "VOICE_STATE_UPDATE" => {
            data.as_map()?;
            let guild_id = etf_optional_snowflake(data, "guild_id")?;
            let channel_id = etf_optional_snowflake(data, "channel_id")?;
            let user_id = etf_required_snowflake(data, "user_id")?;
            let session_id = etf_required_non_empty_string(data, "session_id")?;
            let self_mute = etf_optional_bool(data, "self_mute")?.unwrap_or(false);
            let self_deaf = etf_optional_bool(data, "self_deaf")?.unwrap_or(false);
            let self_video = etf_optional_bool(data, "self_video")?.unwrap_or(false);
            Some(BackgroundEvent::GatewayVoiceStateUpdate {
                guild_id,
                channel_id,
                user_id,
                session_id,
                self_mute,
                self_deaf,
                self_video,
            })
        }
        "VOICE_SERVER_UPDATE" => {
            data.as_map()?;
            let guild_id = etf_optional_snowflake(data, "guild_id")?;
            let endpoint = etf_optional_non_empty_string(data, "endpoint")?;
            let token_present = match data.get("token") {
                None => {
                    if guild_id.is_none() && endpoint.is_none() {
                        return None;
                    }
                    false
                }
                Some(raw) if etf_is_null(raw) => {
                    if guild_id.is_none() && endpoint.is_none() {
                        return None;
                    }
                    false
                }
                Some(raw) => {
                    let token = etf_text(raw)?;
                    if token.is_empty() && guild_id.is_none() && endpoint.is_none() {
                        return None;
                    }
                    !token.is_empty()
                }
            };
            Some(BackgroundEvent::GatewayVoiceServerUpdate {
                guild_id,
                endpoint,
                token_present,
            })
        }
        _ => None,
    }
}

/// Неблокирующая пересылка события `chord_core::DispatchEvent` в канал `BackgroundEvent`.
pub fn forward_dispatch_event(
    tx: &async_channel::Sender<BackgroundEvent>,
    event: &DispatchEvent,
) -> ForwardOutcome {
    let Some(bg_event) = map_dispatch_event(event) else {
        return ForwardOutcome::Ignored;
    };
    match tx.try_send(bg_event) {
        Ok(()) => ForwardOutcome::Sent,
        Err(async_channel::TrySendError::Full(_)) => ForwardOutcome::ChannelFull,
        Err(async_channel::TrySendError::Closed(_)) => ForwardOutcome::ChannelClosed,
    }
}

/// Неблокирующая пересылка события `chord_core::DispatchEvent` с записью результата в [`DispatchBridgeMetrics`].
pub fn forward_dispatch_event_with_metrics(
    tx: &async_channel::Sender<BackgroundEvent>,
    event: &DispatchEvent,
    metrics: &mut DispatchBridgeMetrics,
) -> ForwardOutcome {
    let outcome = forward_dispatch_event(tx, event);
    metrics.record(outcome);
    outcome
}

/// Пул параллельных потоков ОС для тяжелых CPU-bound вычислений
/// (декодирование медиа, шифрование RTP, парсинг больших JSON/ETF)
pub struct ParallelWorkerPool {
    worker_threads: usize,
}

impl Default for ParallelWorkerPool {
    fn default() -> Self {
        let cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        Self::new(cpus)
    }
}

impl ParallelWorkerPool {
    pub fn new(threads: usize) -> Self {
        Self {
            worker_threads: threads.clamp(2, 32),
        }
    }

    pub fn worker_count(&self) -> usize {
        self.worker_threads
    }

    /// Параллельное выполнение пакетных задач с разделением на потоки
    pub fn execute_parallel_chunks<T, F, R>(&self, items: Vec<T>, f: F) -> Vec<R>
    where
        T: Send + 'static,
        R: Send + 'static,
        F: Fn(T) -> R + Sync + Send + 'static,
    {
        let f = Arc::new(f);
        let mut handles = Vec::new();

        for item in items {
            let f_clone = Arc::clone(&f);
            let handle = std::thread::spawn(move || f_clone(item));
            handles.push(handle);
        }

        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            if let Ok(res) = handle.join() {
                results.push(res);
            }
        }
        results
    }
}

use chord_platform::dbus::BusAddress;
use chord_platform::notifications::{Notification, Notifier};
use chord_platform::rpc_ipc::{
    IpcFrame, RpcCommand, RpcOpCode, SetActivityPayload, decode_ipc_frame, encode_ipc_frame,
};
use chord_platform::tray::{
    BadgeState, CloseDisposition, MinimizeToTrayPolicy, MockTrayBackend, TrayBackend,
    TrayController, TrayError, TrayIconState, TrayMenu, TrayMenuAction, compute_tray_icon_state,
    format_badge_count,
};

/// Менеджер нативных уведомлений рабочего стола через freedesktop D-Bus (org.freedesktop.Notifications)
pub struct DesktopNotificationManager;

impl DesktopNotificationManager {
    /// Построение уведомления о сообщении по спецификации Freedesktop
    pub fn build_message_notification(
        author: &str,
        content: &str,
        channel_name: &str,
    ) -> Notification {
        Notification {
            app_name: "Discord".to_string(),
            summary: format!("{} (#{})", author, channel_name),
            body: content.to_string(),
            replaces_id: 0,
            actions: vec![
                ("default".to_string(), "Открыть".to_string()),
                ("mark_read".to_string(), "Прочитано".to_string()),
            ],
            timeout_ms: 5000,
        }
    }

    /// Построение уведомления об упоминании (mention) пользователя
    pub fn build_mention_notification(
        author: &str,
        content: &str,
        guild_name: &str,
        channel_name: &str,
    ) -> Notification {
        Notification {
            app_name: "Discord".to_string(),
            summary: format!(
                "{} упомянул(а) вас в {} (#{})",
                author, guild_name, channel_name
            ),
            body: content.to_string(),
            replaces_id: 0,
            actions: vec![
                ("default".to_string(), "Открыть".to_string()),
                ("mark_read".to_string(), "Прочитано".to_string()),
            ],
            timeout_ms: 7000,
        }
    }

    /// Построение уведомления о личном сообщении (Direct Message)
    pub fn build_dm_notification(author: &str, content: &str) -> Notification {
        Notification {
            app_name: "Discord".to_string(),
            summary: format!("Личное сообщение от {}", author),
            body: content.to_string(),
            replaces_id: 0,
            actions: vec![
                ("default".to_string(), "Открыть".to_string()),
                ("mark_read".to_string(), "Прочитано".to_string()),
            ],
            timeout_ms: 5000,
        }
    }

    /// Отправка нативного уведомления по объекту `Notification`
    pub fn send_notification(notification: &Notification) -> Result<u32, String> {
        let addr = BusAddress::from_env()
            .ok_or_else(|| "DBUS_SESSION_BUS_ADDRESS не задан в окружении".to_string())?;
        let mut notifier =
            Notifier::connect(&addr).map_err(|e| format!("D-Bus Notifier недоступен: {:?}", e))?;
        notifier
            .notify(notification)
            .map_err(|e| format!("Ошибка отправки D-Bus уведомления: {:?}", e))
    }

    /// Отправка нативного уведомления о новом входящем сообщении
    pub fn send_message_notification(
        author: &str,
        content: &str,
        channel_name: &str,
    ) -> Result<u32, String> {
        let notif = Self::build_message_notification(author, content, channel_name);
        Self::send_notification(&notif)
    }

    /// Отправка уведомления об упоминании
    pub fn send_mention_notification(
        author: &str,
        content: &str,
        guild_name: &str,
        channel_name: &str,
    ) -> Result<u32, String> {
        let notif = Self::build_mention_notification(author, content, guild_name, channel_name);
        Self::send_notification(&notif)
    }

    /// Закрытие уведомления по идентификатору
    pub fn close_notification(id: u32) -> Result<(), String> {
        let addr = BusAddress::from_env()
            .ok_or_else(|| "DBUS_SESSION_BUS_ADDRESS не задан в окружении".to_string())?;
        let mut notifier =
            Notifier::connect(&addr).map_err(|e| format!("D-Bus Notifier недоступен: {:?}", e))?;
        notifier
            .close(id)
            .map_err(|e| format!("Ошибка закрытия уведомления: {:?}", e))
    }
}

/// Менеджер системного трея Linux с поддержкой 6-пунктового меню и бейджей
pub struct SystemTrayManager;

impl SystemTrayManager {
    /// Создание контроллера системного трея с 6-пунктовым меню Discord
    pub fn create_default_controller() -> TrayController<MockTrayBackend> {
        let backend = MockTrayBackend::default();
        TrayController::new(Some(backend), MinimizeToTrayPolicy::new(true))
    }

    /// Создание headless-контроллера системного трея
    pub fn create_headless_controller(minimize_to_tray: bool) -> TrayController<MockTrayBackend> {
        TrayController::new_headless(MinimizeToTrayPolicy::new(minimize_to_tray))
    }

    /// Создание контроллера с произвольным бэкендом
    pub fn create_controller<B: TrayBackend>(
        backend: Option<B>,
        minimize_to_tray: bool,
    ) -> TrayController<B> {
        TrayController::new(backend, MinimizeToTrayPolicy::new(minimize_to_tray))
    }

    /// Создание 6-пунктового меню Discord
    pub fn create_menu(in_call: bool, self_mute: bool, self_deaf: bool) -> TrayMenu {
        TrayMenu::new(in_call, self_mute, self_deaf)
    }

    /// Проверка соответствия меню 6-пунктовой спецификации Discord:
    /// 1. Open, 2. Mute, 3. Deafen, 4. VoiceSettings, 5. CheckForUpdates, 6. Quit
    pub fn verify_6_item_menu(menu: &TrayMenu) -> bool {
        if menu.len() != 6 {
            return false;
        }
        let expected_actions = [
            TrayMenuAction::Open,
            TrayMenuAction::Mute,
            TrayMenuAction::Deafen,
            TrayMenuAction::VoiceSettings,
            TrayMenuAction::CheckForUpdates,
            TrayMenuAction::Quit,
        ];
        menu.items
            .iter()
            .zip(expected_actions.iter())
            .all(|(item, &expected)| item.action == expected)
    }

    /// Вычисление состояния иконки трея (Normal, Speaking, Muted, Deafened, Unread)
    pub fn compute_icon_state(
        in_call: bool,
        is_muted: bool,
        is_deafened: bool,
        unread_count: u32,
    ) -> TrayIconState {
        compute_tray_icon_state(in_call, is_muted, is_deafened, unread_count)
    }

    /// Форматирование бейджа лаунчера (1..99, 99+)
    pub fn format_badge(count: u32) -> Option<String> {
        format_badge_count(count)
    }
}

/// Высокоуровневый контроллер иконки системного трея Linux и D-Bus уведомлений.
/// Интегрирует 6-пунктовое меню Discord, бейджи непрочитанных,
/// состояния микрофона/звука и отправку уведомлений через D-Bus (org.freedesktop.Notifications).
pub struct LinuxSystemTrayIconController<B: TrayBackend = MockTrayBackend> {
    controller: TrayController<B>,
}

impl LinuxSystemTrayIconController<MockTrayBackend> {
    /// Создание контроллера по умолчанию с mock-бэкендом трея и политикой сворачивания в трей
    pub fn default_linux() -> Self {
        Self {
            controller: SystemTrayManager::create_default_controller(),
        }
    }

    /// Создание headless-контроллера без UI-бэкенда
    pub fn headless(minimize_to_tray: bool) -> Self {
        Self {
            controller: SystemTrayManager::create_headless_controller(minimize_to_tray),
        }
    }
}

impl<B: TrayBackend> LinuxSystemTrayIconController<B> {
    /// Создание контроллера с заданным бэкендом и политикой
    pub fn new(backend: Option<B>, minimize_to_tray: bool) -> Self {
        Self {
            controller: TrayController::new(backend, MinimizeToTrayPolicy::new(minimize_to_tray)),
        }
    }

    /// Доступ к внутреннему TrayController
    pub fn controller(&self) -> &TrayController<B> {
        &self.controller
    }

    /// Мутабельный доступ к внутреннему TrayController
    pub fn controller_mut(&mut self) -> &mut TrayController<B> {
        &mut self.controller
    }

    /// 6-пунктовое контекстное меню Discord
    pub fn menu(&self) -> &TrayMenu {
        self.controller.menu()
    }

    /// Проверка, что меню строго соответствует 6 пунктам Discord
    pub fn has_valid_6_item_menu(&self) -> bool {
        SystemTrayManager::verify_6_item_menu(self.menu())
    }

    /// Текущее состояние иконки (Normal, Speaking, Muted, Deafened, Unread)
    pub fn icon_state(&self) -> TrayIconState {
        self.controller.icon_state()
    }

    /// Текущий бейдж непрочитанных сообщений
    pub fn badge(&self) -> &BadgeState {
        self.controller.badge()
    }

    /// Видимо ли окно приложения
    pub fn is_window_visible(&self) -> bool {
        self.controller.is_window_visible()
    }

    /// Обновление состояния голосового звонка (активность, mute, deafen)
    pub fn update_voice_state(
        &mut self,
        in_call: bool,
        self_mute: bool,
        self_deaf: bool,
    ) -> Result<(), TrayError> {
        self.controller
            .update_voice_state(in_call, self_mute, self_deaf)
    }

    /// Обновление счетчика непрочитанных сообщений
    pub fn set_unread_count(&mut self, count: u32) -> Result<(), TrayError> {
        self.controller.set_unread_count(count)
    }

    /// Обработка клика по иконке трея (переключение видимости окна)
    pub fn on_tray_clicked(&mut self) {
        self.controller.on_tray_clicked();
    }

    /// Обработка действия из контекстного меню
    pub fn on_menu_action(&mut self, action: TrayMenuAction) -> CloseDisposition {
        self.controller.on_menu_action(action)
    }

    /// Обработка запроса закрытия главного окна
    pub fn handle_window_close(&mut self) -> CloseDisposition {
        self.controller.handle_window_close()
    }

    /// Отправка нативного D-Bus уведомления о входящем сообщении
    pub fn send_message_notification(
        &self,
        author: &str,
        content: &str,
        channel_name: &str,
    ) -> Result<u32, String> {
        DesktopNotificationManager::send_message_notification(author, content, channel_name)
    }

    /// Отправка нативного D-Bus уведомления об упоминании
    pub fn send_mention_notification(
        &self,
        author: &str,
        content: &str,
        guild_name: &str,
        channel_name: &str,
    ) -> Result<u32, String> {
        DesktopNotificationManager::send_mention_notification(
            author,
            content,
            guild_name,
            channel_name,
        )
    }
}

/// Сервер локального бинарного протокола Discord Local RPC IPC (discord-ipc-0..9)
/// Полная совместимость с rust-discord-activity и Serein
pub struct LocalRpcIpcServer;

impl LocalRpcIpcServer {
    /// Кодирование фрейма рукопожатия Handshake (Opcode = 0)
    pub fn create_handshake_frame(v: u64, client_id: &str) -> Result<Vec<u8>, String> {
        let payload = serde_json::json!({
            "v": v,
            "client_id": client_id
        });
        encode_ipc_frame(RpcOpCode::Handshake, &payload.to_string())
            .map_err(|e| format!("Ошибка кодирования IPC фрейма: {:?}", e))
    }

    /// Декодирование входящего бинарного фрейма (Header 8B + JSON payload)
    pub fn parse_ipc_frame(bytes: &[u8]) -> Result<IpcFrame, String> {
        decode_ipc_frame(bytes).map_err(|e| format!("Ошибка декодирования IPC фрейма: {:?}", e))
    }

    /// Разбор полезной нагрузки SET_ACTIVITY из JSON
    pub fn parse_set_activity(payload_json: &str) -> Result<SetActivityPayload, String> {
        match RpcCommand::from_json(payload_json) {
            Ok(RpcCommand::SetActivity(payload)) => Ok(payload),
            Ok(_) => Err("Команда не является SET_ACTIVITY".to_string()),
            Err(e) => Err(format!("Ошибка разбора RPC команды: {:?}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn etf_map(pairs: Vec<(&str, EtfValue)>) -> EtfValue {
        EtfValue::Map(
            pairs
                .into_iter()
                .map(|(k, v)| (EtfValue::Atom(k.to_string()), v))
                .collect(),
        )
    }

    #[test]
    fn dispatch_bridge_unit_smoke() {
        let dispatcher = AsyncTaskDispatcher::new(2);
        let ready = DispatchEvent {
            kind: Some(DispatchKind::Ready),
            name: "READY".into(),
            seq: Some(1),
            data: etf_map(vec![
                ("session_id", EtfValue::Str("sess-1".into())),
                (
                    "user",
                    etf_map(vec![
                        ("id", EtfValue::Int(42)),
                        ("username", EtfValue::Str("tester".into())),
                    ]),
                ),
                ("guilds", EtfValue::List(vec![])),
            ]),
        };
        assert_eq!(
            dispatcher.forward_dispatch_event(&ready),
            ForwardOutcome::Sent
        );
        assert_eq!(dispatcher.bridge_metrics().sent, 1);
        assert_eq!(dispatcher.dropped_count(), 0);
        let events = dispatcher.drain_events();
        assert_eq!(
            events,
            vec![BackgroundEvent::GatewayReady {
                session_id: "sess-1".into(),
                resume_gateway_url: None,
                user_id: "42".into(),
                username: "tester".into(),
                guild_count: 0,
            }]
        );
    }
}
