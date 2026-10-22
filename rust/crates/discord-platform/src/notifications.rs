//! P6-02: уведомления рабочего стола через `org.freedesktop.Notifications`.
//!
//! Discord на Linux **не загружает** нативный модуль уведомлений (`RE-NOTIFICATIONS.md` §1),
//! поэтому здесь самостоятельный D-Bus-клиент по спецификации freedesktop — не перенос
//! нативного модуля и не wire-совместимость с Electron (`RE-NOTIFICATIONS.md` §3).
//!
//! Поддержано: `Notify`, `CloseNotification`, `GetCapabilities`, `GetServerInformation`,
//! `AddMatch` + события `ActionInvoked`/`NotificationClosed`.
//! `hints` намеренно пустой `a{sv}`: полный набор хинтов (urgency, image-path, …) —
//! отдельное расширение, когда появится UI-потребитель.
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
/// `Vec<(String, String)>`; `hints` — пустой `a{sv}`.
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

    /// `Notify(...) -> u`: серверный id уведомления.
    ///
    /// `app_name` и `summary` обязаны быть непустыми (иначе `DbusError::Protocol`):
    /// пустой заголовок серверы обычно отбрасывают, и пользователь не увидит уведомление.
    pub fn notify(&mut self, notification: &Notification) -> Result<u32, DbusError> {
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
            Value::Dict(Vec::new()), // hints: пустой a{sv}
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
