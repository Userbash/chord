//! P6-02: уведомления рабочего стола через `org.freedesktop.Notifications`.
//!
//! Discord на Linux **не загружает** нативный модуль уведомлений (`RE-NOTIFICATIONS.md` §1),
//! поэтому здесь самостоятельный D-Bus-клиент по спецификации freedesktop — не перенос
//! нативного модуля и не wire-совместимость с Electron (`RE-NOTIFICATIONS.md` §3).
//!
//! Поддержано: `Notify`, `CloseNotification`, `GetCapabilities`, `GetServerInformation`,
//! `AddMatch` + события `ActionInvoked`/`NotificationClosed`.
//! `hints` — `a{sv}` с восемью freedesktop-хинтами (`urgency`, `category`,
//! `desktop-entry`, `image-path`, `sound-name`, `resident`, `transient`, `suppress-sound`)
//! через [`Hints`]/[`Notifier::notify_with_hints`]; `notify` оставляет dict пустым.
//! DND/группировка/история — уровень приложения (P6-01/P3), не этого модуля.

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;

use crate::dbus::{BusAddress, Connection, DbusError, Value};

/// Well-known имя сервиса уведомлений.
pub const DESTINATION: &str = "org.freedesktop.Notifications";
/// Путь объекта сервиса уведомлений.
pub const PATH: &str = "/org/freedesktop/Notifications";
/// Интерфейс сервиса уведомлений.
pub const INTERFACE: &str = "org.freedesktop.Notifications";
/// Сигнатура тела `Notify` по спецификации freedesktop.
///
/// `actions` на проводе — `as` (плоский список парами id/label), хотя в модели это
/// `Vec<(String, String)>`; `hints` — `a{sv}` (см. [`Hints`]).
pub const NOTIFY_SIGNATURE: &str = "susssasa{sv}i";
/// Таймаут ожидания события (`next_event`).
pub const EVENT_TIMEOUT: Duration = Duration::from_secs(5);

/// Уведомление к отправке.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Notification {
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub replaces_id: u32,
    /// Пары (id действия, подпись) — на проводе разворачиваются в `as`.
    pub actions: Vec<(String, String)>,
    /// -1 — таймаут сервера по умолчанию.
    pub timeout_ms: i32,
}

/// Важность уведомления — hint `urgency`, на проводе вариант `y`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Urgency {
    Low,
    Normal,
    Critical,
}

impl Urgency {
    /// Значение на проводе: `Low` = 0, `Normal` = 1, `Critical` = 2.
    pub fn code(self) -> u8 {
        match self {
            Self::Low => 0,
            Self::Normal => 1,
            Self::Critical => 2,
        }
    }
}

/// Хинты `Notify` — минимум freedesktop, передаётся как `a{sv}`.
///
/// `Hints::default()` — пустой dict, поэтому
/// `notify(&notification)` эквивалентен
/// `notify_with_hints(&notification, &Hints::default())`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hints {
    /// `urgency` (вариант `y`).
    pub urgency: Option<Urgency>,
    /// `category` (вариант `s`); пустая строка не отправляется.
    pub category: Option<String>,
    /// `desktop-entry` (вариант `s`); пустая строка не отправляется.
    pub desktop_entry: Option<String>,
    /// `image-path` (вариант `s`) — URI или путь к иконке; пустая строка не отправляется.
    pub image_path: Option<String>,
    /// `sound-name` (вариант `s`) — имя звука темы; пустая строка не отправляется.
    pub sound_name: Option<String>,
    /// `resident` (вариант `b`).
    pub resident: Option<bool>,
    /// `transient` (вариант `b`); `Some(false)` — запись `b:0`, а не пропуск.
    pub transient: Option<bool>,
    /// `suppress-sound` (вариант `b`); `Some(false)` — запись `b:0`, а не пропуск.
    pub suppress_sound: Option<bool>,
}

impl Hints {
    /// Собирает `a{sv}`: только заданные непустые значения, порядок фиксирован —
    /// `urgency`, `category`, `desktop-entry`, `image-path`, `sound-name`, `resident`,
    /// `transient`, `suppress-sound`.
    pub fn to_dbus(&self) -> Value {
        let mut entries: Vec<(Value, Value)> = Vec::new();
        if let Some(urgency) = self.urgency {
            entries.push(variant_entry("urgency", Value::Byte(urgency.code())));
        }
        if let Some(category) = non_empty(&self.category) {
            entries.push(variant_entry(
                "category",
                Value::String(category.to_owned()),
            ));
        }
        if let Some(desktop_entry) = non_empty(&self.desktop_entry) {
            entries.push(variant_entry(
                "desktop-entry",
                Value::String(desktop_entry.to_owned()),
            ));
        }
        if let Some(image_path) = non_empty(&self.image_path) {
            entries.push(variant_entry(
                "image-path",
                Value::String(image_path.to_owned()),
            ));
        }
        if let Some(sound_name) = non_empty(&self.sound_name) {
            entries.push(variant_entry(
                "sound-name",
                Value::String(sound_name.to_owned()),
            ));
        }
        if let Some(resident) = self.resident {
            entries.push(variant_entry("resident", Value::Bool(resident)));
        }
        if let Some(transient) = self.transient {
            entries.push(variant_entry("transient", Value::Bool(transient)));
        }
        if let Some(suppress_sound) = self.suppress_sound {
            entries.push(variant_entry("suppress-sound", Value::Bool(suppress_sound)));
        }
        Value::Dict(entries)
    }
}

fn non_empty(value: &Option<String>) -> Option<&str> {
    match value.as_deref() {
        Some(text) if !text.is_empty() => Some(text),
        _ => None,
    }
}

fn variant_entry(key: &str, value: Value) -> (Value, Value) {
    (
        Value::String(key.to_owned()),
        Value::Variant(Box::new(value)),
    )
}

/// Событие уведомления, пришедшее сигналом.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum NotificationEvent {
    Action { id: u32, action: String },
    Closed { id: u32, reason: u32 },
    ActionInvoked { id: u32, action_key: String },
    NotificationClosed { id: u32, reason: u32 },
}

/// Псевдоним для совместимости с интерфейсом обработки событий.
pub type Event = NotificationEvent;

/// Входящее сообщение чата для оценки отправки системного уведомления.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingMessageNotification {
    pub message_id: String,
    pub guild_id: Option<String>,
    pub channel_id: String,
    pub author_id: String,
    pub author_name: String,
    pub content: String,
    pub mentions_self: bool,
    pub is_dm: bool,
}

/// Решение о диспетчеризации уведомления.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationDispatchDecision {
    Send {
        notification: Notification,
        deep_link_url: String,
    },
    SuppressedSelfAuthor,
    SuppressedDnd,
    SuppressedActiveChannel,
    SuppressedNotMentionOrDm,
    SuppressedDuplicate,
}

/// Емкость очереди дедупликации сообщений по умолчанию (FIFO).
pub const DEFAULT_DEDUP_CAPACITY: usize = 256;

/// Проверяет, является ли строка трёхчастным base64/base64url Discord токеном.
fn is_three_part_token(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    let (p0, p1, p2) = (parts[0], parts[1], parts[2]);
    if p0.len() < 4 || p1.len() < 3 || p2.len() < 4 || s.len() < 14 {
        return false;
    }
    let lower_p2 = p2.to_ascii_lowercase();
    if matches!(
        lower_p2.as_str(),
        "com" | "org" | "net" | "edu" | "gov" | "io" | "app" | "dev" | "html" | "json" | "txt"
    ) {
        return false;
    }
    p0.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && p1
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && p2
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Проверяет, является ли строка MFA токеном (например, `mfa.xxxx`).
fn is_mfa_token(s: &str) -> bool {
    if let Some(tail) = s.strip_prefix("mfa.").or_else(|| s.strip_prefix("MFA.")) {
        !tail.is_empty()
            && tail
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    } else {
        false
    }
}

/// Маскирует секреты (MFA токены, 3-частные токены и Bearer токены) в тексте.
pub fn redact_secrets(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut is_after_bearer = false;

    let chars: Vec<(usize, char)> = input.char_indices().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i].1.is_whitespace() {
            result.push(chars[i].1);
            i += 1;
            continue;
        }

        let word_start = chars[i].0;
        while i < chars.len() && !chars[i].1.is_whitespace() {
            i += 1;
        }
        let word_end = if i < chars.len() {
            chars[i].0
        } else {
            input.len()
        };
        let word = &input[word_start..word_end];

        let trimmed_start = word
            .find(|c: char| c.is_alphanumeric() || c == '_' || c == '-')
            .unwrap_or(0);
        let trimmed_end = word
            .rfind(|c: char| c.is_alphanumeric() || c == '_' || c == '-')
            .map(|idx| idx + 1)
            .unwrap_or(word.len());

        let prefix = &word[..trimmed_start];
        let core = if trimmed_start <= trimmed_end {
            &word[trimmed_start..trimmed_end]
        } else {
            word
        };
        let suffix = &word[trimmed_end..];

        let core_lower = core.to_ascii_lowercase();
        let is_bearer_keyword = core_lower == "bearer";

        if (is_after_bearer && !is_bearer_keyword && !core.is_empty())
            || is_mfa_token(core)
            || is_three_part_token(core)
        {
            result.push_str(prefix);
            result.push_str("<redacted>");
            result.push_str(suffix);
            is_after_bearer = false;
        } else if let Some((before_eq, after_eq)) = core.split_once('=') {
            if before_eq.eq_ignore_ascii_case("bearer") && !after_eq.is_empty() {
                result.push_str(prefix);
                result.push_str(before_eq);
                result.push_str("=<redacted>");
                result.push_str(suffix);
                is_after_bearer = false;
            } else {
                result.push_str(word);
                is_after_bearer = is_bearer_keyword;
            }
        } else {
            result.push_str(word);
            is_after_bearer = is_bearer_keyword;
        }
    }

    result
}

/// Маршрутизатор десктопных уведомлений.
///
/// Управляет DND, активным каналом, дедупликацией сообщений и связыванием действий D-Bus.
#[derive(Debug, Clone)]
pub struct NotificationRouter {
    pub app_name: String,
    pub active_channel_id: Option<String>,
    pub self_user_id: Option<String>,
    pub dnd_enabled: bool,
    pub window_focused: bool,
    dedup_capacity: usize,
    seen_messages: HashSet<String>,
    seen_order: VecDeque<String>,
    pending_urls: HashMap<u32, String>,
}

impl NotificationRouter {
    /// Создает новый маршрутизатор с дефолтной емкостью дедупликации (256 сообщений).
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_DEDUP_CAPACITY)
    }

    /// Создает маршрутизатор с заданной емкостью дедупликации FIFO.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            app_name: "Discord".to_string(),
            active_channel_id: None,
            self_user_id: None,
            dnd_enabled: false,
            window_focused: false,
            dedup_capacity: capacity.max(1),
            seen_messages: HashSet::new(),
            seen_order: VecDeque::new(),
            pending_urls: HashMap::new(),
        }
    }

    pub fn set_active_channel(&mut self, channel_id: Option<String>) {
        self.active_channel_id = channel_id;
    }

    pub fn set_self_user_id(&mut self, user_id: Option<String>) {
        self.self_user_id = user_id;
    }

    pub fn set_dnd(&mut self, enabled: bool) {
        self.dnd_enabled = enabled;
    }

    pub fn set_window_focused(&mut self, focused: bool) {
        self.window_focused = focused;
    }

    pub fn pending_urls_count(&self) -> usize {
        self.pending_urls.len()
    }

    pub fn seen_messages_count(&self) -> usize {
        self.seen_messages.len()
    }

    pub fn contains_message_id(&self, message_id: &str) -> bool {
        self.seen_messages.contains(message_id)
    }

    pub fn get_registered_url(&self, notification_id: u32) -> Option<&str> {
        self.pending_urls.get(&notification_id).map(String::as_str)
    }

    /// Оценивает входящее сообщение и принимает решение о диспетчеризации.
    pub fn evaluate(&mut self, msg: &IncomingMessageNotification) -> NotificationDispatchDecision {
        if self.self_user_id.as_deref() == Some(&msg.author_id) {
            return NotificationDispatchDecision::SuppressedSelfAuthor;
        }

        if self.dnd_enabled {
            return NotificationDispatchDecision::SuppressedDnd;
        }

        if self.window_focused && self.active_channel_id.as_deref() == Some(&msg.channel_id) {
            return NotificationDispatchDecision::SuppressedActiveChannel;
        }

        if !msg.mentions_self && !msg.is_dm {
            return NotificationDispatchDecision::SuppressedNotMentionOrDm;
        }

        if self.seen_messages.contains(&msg.message_id) {
            return NotificationDispatchDecision::SuppressedDuplicate;
        }

        if self.seen_messages.insert(msg.message_id.clone()) {
            self.seen_order.push_back(msg.message_id.clone());
            while self.seen_order.len() > self.dedup_capacity {
                if let Some(oldest) = self.seen_order.pop_front() {
                    self.seen_messages.remove(&oldest);
                }
            }
        }

        let deep_link_url = if msg.is_dm {
            format!("discord://-/channels/@me/{}", msg.channel_id)
        } else if let Some(ref gid) = msg.guild_id {
            format!("discord://-/channels/{}/{}", gid, msg.channel_id)
        } else {
            format!("discord://-/channels/@me/{}", msg.channel_id)
        };

        let notification = Notification {
            app_name: self.app_name.clone(),
            summary: msg.author_name.clone(),
            body: redact_secrets(&msg.content),
            replaces_id: 0,
            actions: vec![("default".to_string(), "Open".to_string())],
            timeout_ms: -1,
        };

        NotificationDispatchDecision::Send {
            notification,
            deep_link_url,
        }
    }

    /// Регистрирует URL для отправленного уведомления с его серверным id.
    pub fn register_sent(&mut self, notification_id: u32, deep_link_url: String) {
        self.pending_urls.insert(notification_id, deep_link_url);
    }

    /// Обрабатывает событие D-Bus (`ActionInvoked` / `NotificationClosed`).
    ///
    /// При `Action` возвращает зарегистрированный deep_link_url (если есть).
    /// При `NotificationClosed` удаляет запись и возвращает `None`.
    pub fn handle_event(&mut self, event: &NotificationEvent) -> Option<String> {
        match event {
            NotificationEvent::Action { id, .. } | NotificationEvent::ActionInvoked { id, .. } => {
                self.pending_urls.get(id).cloned()
            }
            NotificationEvent::Closed { id, .. }
            | NotificationEvent::NotificationClosed { id, .. } => {
                self.pending_urls.remove(id);
                None
            }
        }
    }
}

impl Default for NotificationRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Клиент сервиса уведомлений.
#[derive(Debug)]
pub struct Notifier {
    connection: Connection,
}

impl Notifier {
    /// Подключается к шине (AUTH EXTERNAL + `Hello`), но ничего не отправляет.
    pub fn connect(address: &BusAddress) -> Result<Self, DbusError> {
        Ok(Self {
            connection: Connection::connect(address)?,
        })
    }

    /// `GetCapabilities() -> as`.
    pub fn capabilities(&mut self) -> Result<Vec<String>, DbusError> {
        let body = self.call("GetCapabilities", "", &[])?;
        let Some(Value::Array(items)) = body.into_iter().next() else {
            return Err(DbusError::Protocol("GetCapabilities reply is not an array"));
        };
        let mut capabilities = Vec::with_capacity(items.len());
        for item in items {
            match item {
                Value::String(capability) => capabilities.push(capability),
                _ => {
                    return Err(DbusError::Protocol("GetCapabilities item is not a string"));
                }
            }
        }
        Ok(capabilities)
    }

    /// `GetServerInformation() -> ssss`: имя, вендор, версия, версия спецификации.
    ///
    /// По D-Bus несколько возвращаемых значений — это последовательность типов тела
    /// (`ssss`), а не struct: struct-обёртки в теле ответа нет.
    pub fn server_information(&mut self) -> Result<(String, String, String, String), DbusError> {
        let body = self.call("GetServerInformation", "", &[])?;
        let mut values: [String; 4] = Default::default();
        if body.len() != 4 {
            return Err(DbusError::Protocol(
                "GetServerInformation reply does not have 4 values",
            ));
        }
        for (slot, item) in values.iter_mut().zip(body) {
            match item {
                Value::String(text) => *slot = text,
                _ => {
                    return Err(DbusError::Protocol(
                        "GetServerInformation field is not a string",
                    ));
                }
            }
        }
        let [name, vendor, version, specification] = values;
        Ok((name, vendor, version, specification))
    }

    /// `Notify(...) -> u`: серверный id уведомления, hints пустые (`a{sv}` с 0 записей).
    ///
    /// Эквивалент `notify_with_hints(notification, &Hints::default())`.
    pub fn notify(&mut self, notification: &Notification) -> Result<u32, DbusError> {
        self.notify_with_hints(notification, &Hints::default())
    }

    /// `Notify(...) -> u` с hints (`a{sv}`): `urgency`, `category`, `desktop-entry`,
    /// `image-path`, `sound-name`, `resident`, `transient`, `suppress-sound`.
    /// Порядок записей фиксирован, пустые строки не отправляются.
    ///
    /// `app_name` и `summary` обязаны быть непустыми (иначе `DbusError::Protocol`):
    /// пустой заголовок серверы обычно отбрасывают, и пользователь не увидит уведомление.
    pub fn notify_with_hints(
        &mut self,
        notification: &Notification,
        hints: &Hints,
    ) -> Result<u32, DbusError> {
        if notification.app_name.is_empty() {
            return Err(DbusError::Protocol("empty app_name"));
        }
        if notification.summary.is_empty() {
            return Err(DbusError::Protocol("empty summary"));
        }

        let mut actions = Vec::with_capacity(notification.actions.len() * 2);
        for (action_id, label) in &notification.actions {
            if action_id.is_empty() {
                return Err(DbusError::Protocol("empty action id"));
            }
            actions.push(Value::String(action_id.clone()));
            actions.push(Value::String(label.clone()));
        }

        let body = vec![
            Value::String(notification.app_name.clone()),
            Value::U32(notification.replaces_id),
            Value::String(String::new()), // app_icon: иконки в этой карточке не рисуются
            Value::String(notification.summary.clone()),
            Value::String(notification.body.clone()),
            Value::Array(actions),
            hints.to_dbus(), // a{sv}: пустой Hints → 0 записей, не null
            Value::I32(notification.timeout_ms),
        ];

        let reply = self.call("Notify", NOTIFY_SIGNATURE, &body)?;
        match reply.into_iter().next() {
            Some(Value::U32(id)) => Ok(id),
            _ => Err(DbusError::Protocol("Notify reply is not a u32 id")),
        }
    }

    /// `CloseNotification(u)`.
    pub fn close(&mut self, id: u32) -> Result<(), DbusError> {
        self.call("CloseNotification", "u", &[Value::U32(id)])?;
        Ok(())
    }

    /// Подписка на `ActionInvoked` и `NotificationClosed`.
    pub fn listen_actions(&mut self) -> Result<(), DbusError> {
        for member in ["ActionInvoked", "NotificationClosed"] {
            let rule = format!("type='signal',interface='{INTERFACE}',member='{member}'");
            self.connection.add_match(&rule)?;
        }
        Ok(())
    }

    /// Следующее событие уведомления; чужие сигналы и сигналы с неожиданным телом
    /// пропускаются (не ошибка).
    pub fn next_event(&mut self) -> Result<NotificationEvent, DbusError> {
        loop {
            let signal = self.connection.next_signal(EVENT_TIMEOUT)?;
            if signal.interface != INTERFACE {
                continue;
            }
            let event = match signal.member.as_str() {
                "ActionInvoked" => action_event(&signal.body),
                "NotificationClosed" => closed_event(&signal.body),
                _ => None,
            };
            if let Some(event) = event {
                return Ok(event);
            }
        }
    }

    fn call(
        &mut self,
        member: &str,
        signature: &str,
        body: &[Value],
    ) -> Result<Vec<Value>, DbusError> {
        self.connection
            .call_typed(DESTINATION, PATH, INTERFACE, member, signature, body)
    }
}

fn action_event(body: &[Value]) -> Option<NotificationEvent> {
    // Сигнатура `us`: тело — два значения подряд (без struct-обёртки).
    match body {
        [Value::U32(id), Value::String(action)] => Some(NotificationEvent::Action {
            id: *id,
            action: action.clone(),
        }),
        _ => None,
    }
}

fn closed_event(body: &[Value]) -> Option<NotificationEvent> {
    // Сигнатура `uu`.
    match body {
        [Value::U32(id), Value::U32(reason)] => Some(NotificationEvent::Closed {
            id: *id,
            reason: *reason,
        }),
        _ => None,
    }
}
