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
pub enum NotificationEvent {
    Action { id: u32, action: String },
    Closed { id: u32, reason: u32 },
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
