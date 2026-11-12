//! P6-06: питание и idle через freedesktop D-Bus (headless).
//!
//! Три независимых части, каждая — самостоятельная freedesktop-реализация, а не перенос
//! нативного `discord_utils` (wire-совместимость не заявляется, `RE-POWER.md` §3):
//!
//! - [`ScreenSaver`] (`org.freedesktop.ScreenSaver`, сессионная шина) — не гасить экран
//!   во время звонка: `Inhibit`/`UnInhibit`/`IsActive`, плюс KDE-метод
//!   `GetSessionIdleTime` (на других окружениях может отсутствовать — `Ok(None)`).
//! - [`PowerMonitor`] (`org.freedesktop.login1.Manager`, системная шина) — события
//!   `PrepareForSleep(b)`: `true` → [`PowerEvent::GoingToSleep`], `false` →
//!   [`PowerEvent::WakingUp`].
//! - [`IdlePolicy`] — авто-статус `online/idle/dnd` по времени бездействия. Точные пороги
//!   в RE клиента не подтверждены, поэтому [`IdlePolicy::DEFAULT`] — конфигурация с явной
//!   пометкой, а не «факт» Discord.
//! - [`PowerMonitor::inhibit_sleep`] — fd-ингибитор сна: `login1.Manager.Inhibit(ssss) -> h`
//!   через SCM_RIGHTS, владение fd — [`SleepInhibitor`] (`Drop` → `close`).
//!
//! Ограничения (осознанные, в бэклоге): Wayland `ext-idle-notify-v1` и X11 `XScreenSaver`
//! без клиентских библиотек недоступны; live-проверка только читает состояние и по флагу
//! `DISCORD_LAB_POWER_LIVE=1` ставит ингибитор на время теста (в CI флаг выключен).
//! Только `std` + [`crate::dbus`] + `libc` (изоляция `unsafe` — `dbus/fd.rs`).

use std::os::fd::RawFd;
use std::time::Duration;

use crate::dbus::{BusAddress, Connection, DbusError, Signal, Value, fd};

/// Well-known имя сервиса скринсейвера (сессионная шина).
pub const SCREENSAVER_DESTINATION: &str = "org.freedesktop.ScreenSaver";
/// Путь объекта скринсейвера.
pub const SCREENSAVER_PATH: &str = "/org/freedesktop/ScreenSaver";
/// Интерфейс скринсейвера.
pub const SCREENSAVER_INTERFACE: &str = "org.freedesktop.ScreenSaver";
/// `Inhibit(application_name: s, reason_for_inhibit: s) -> cookie: u`.
pub const INHIBIT_METHOD: &str = "Inhibit";
/// Сигнатура `Inhibit`.
pub const INHIBIT_SIGNATURE: &str = "ss";
/// Каноническое имя снятия ингибитора.
pub const UNINHIBIT_METHOD: &str = "UnInhibit";
/// Вариант имени, который встречается у части сервисов; используется только как fallback.
pub const UNINHIBIT_FALLBACK_METHOD: &str = "Uninhibit";
/// `IsActive() -> b`.
pub const IS_ACTIVE_METHOD: &str = "IsActive";
/// `GetSessionIdleTime() -> u` (KDE; метод не входит в базовую спеку freedesktop).
pub const IDLE_METHOD: &str = "GetSessionIdleTime";

/// Well-known имя systemd-logind (системная шина).
pub const LOGIN1_DESTINATION: &str = "org.freedesktop.login1";
/// Путь объекта менеджера logind.
pub const LOGIN1_PATH: &str = "/org/freedesktop/login1";
/// Интерфейс менеджера logind.
pub const LOGIN1_INTERFACE: &str = "org.freedesktop.login1.Manager";
/// Сигнал `PrepareForSleep(b)`: `true` — уходим в сон, `false` — проснулись.
pub const SLEEP_SIGNAL: &str = "PrepareForSleep";
/// Метод `Inhibit` менеджера logind.
pub const SLEEP_INHIBIT_METHOD: &str = "Inhibit";
/// Что ингибируем: только сон.
pub const SLEEP_INHIBIT_WHAT: &str = "sleep";
/// `who` для журнала logind.
pub const SLEEP_INHIBIT_WHO: &str = "discord-lab";
/// Режим ингибитора: `block` (не `delay`/`prepare`).
pub const SLEEP_INHIBIT_MODE: &str = "block";

/// `org.freedesktop.ScreenSaver`: ингибитор гашения экрана в звонке.
#[derive(Debug)]
pub struct ScreenSaver {
    connection: Connection,
}

impl ScreenSaver {
    /// Подключается к шине (AUTH EXTERNAL + `Hello`), ничего не вызывая.
    pub fn connect(address: &BusAddress) -> Result<Self, DbusError> {
        Ok(Self {
            connection: Connection::connect(address)?,
        })
    }

    /// `Inhibit(app_name, reason) -> cookie`: серверный cookie для снятия ингибитора.
    ///
    /// Пустые `app_name`/`reason` отвергаются локально (`DbusError::Protocol`): такие
    /// вызовы бессмысленны для сервиса и не должны уходить на шину.
    pub fn inhibit(&mut self, app_name: &str, reason: &str) -> Result<u32, DbusError> {
        if app_name.is_empty() {
            return Err(DbusError::Protocol("empty app_name"));
        }
        if reason.is_empty() {
            return Err(DbusError::Protocol("empty reason"));
        }
        let body = [
            Value::String(app_name.to_owned()),
            Value::String(reason.to_owned()),
        ];
        let reply = self.call(INHIBIT_METHOD, INHIBIT_SIGNATURE, &body)?;
        match reply.into_iter().next() {
            Some(Value::U32(cookie)) => Ok(cookie),
            _ => Err(DbusError::Protocol("Inhibit reply is not a u32 cookie")),
        }
    }

    /// `UnInhibit(cookie)`: снимает ингибитор.
    ///
    /// Если сервис не знает `UnInhibit` (`UnknownMethod`/`UnknownInterface`), пробуется
    /// вариант `Uninhibit` — регистр в имени метода различается между реализациями.
    /// Неизвестный cookie возвращается сервером как `DbusError::Remote`, не паника.
    pub fn uninhibit(&mut self, cookie: u32) -> Result<(), DbusError> {
        let body = [Value::U32(cookie)];
        match self.call(UNINHIBIT_METHOD, "u", &body) {
            Ok(_) => Ok(()),
            Err(DbusError::Remote { name, .. }) if is_unknown_member(&name) => {
                self.call(UNINHIBIT_FALLBACK_METHOD, "u", &body)?;
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    /// `IsActive() -> b`: активен ли сейчас скринсейвер/блокировка.
    pub fn is_active(&mut self) -> Result<bool, DbusError> {
        let reply = self.call(IS_ACTIVE_METHOD, "", &[])?;
        match reply.into_iter().next() {
            Some(Value::Bool(active)) => Ok(active),
            _ => Err(DbusError::Protocol("IsActive reply is not a bool")),
        }
    }

    /// `GetSessionIdleTime() -> u` в секундах.
    ///
    /// `Ok(None)` — метод или интерфейс не поддержан (`UnknownMethod`/`UnknownInterface`):
    /// это штатная ситуация вне KDE, а не ошибка. Прочие ошибки остаются `Err`.
    pub fn idle_seconds(&mut self) -> Result<Option<u64>, DbusError> {
        match self.call(IDLE_METHOD, "", &[]) {
            Ok(reply) => match reply.into_iter().next() {
                Some(Value::U32(seconds)) => Ok(Some(u64::from(seconds))),
                _ => Err(DbusError::Protocol("GetSessionIdleTime reply is not a u32")),
            },
            Err(DbusError::Remote { name, .. }) if is_unknown_member(&name) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn call(
        &mut self,
        member: &str,
        signature: &str,
        body: &[Value],
    ) -> Result<Vec<Value>, DbusError> {
        self.connection.call_typed(
            SCREENSAVER_DESTINATION,
            SCREENSAVER_PATH,
            SCREENSAVER_INTERFACE,
            member,
            signature,
            body,
        )
    }
}

/// Событие питания от `org.freedesktop.login1.Manager`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerEvent {
    /// `PrepareForSleep(true)`.
    GoingToSleep,
    /// `PrepareForSleep(false)`.
    WakingUp,
}

/// Владелец fd ингибитора сна: пока fd открыт, logind не усыпляет систему.
///
/// fd не клонируется (`Clone` не реализован); `Drop` закрывает его ровно один раз, после
/// чего ингибитор снят. Переживание `fork` не гарантируется: fd принадлежит процессу.
#[derive(Debug)]
pub struct SleepInhibitor {
    fd: RawFd,
}

impl SleepInhibitor {
    /// Тестовый хук: сырой fd ингибитора (владелец — структура).
    pub fn fd(&self) -> RawFd {
        self.fd
    }
}

impl Drop for SleepInhibitor {
    fn drop(&mut self) {
        fd::close_fd(self.fd);
    }
}

/// Берёт первый `h` из ответа `Inhibit`, закрывает лишние fd; без fd — ошибка.
fn take_inhibitor(values: &[Value], fds: Vec<RawFd>) -> Result<SleepInhibitor, DbusError> {
    let chosen = values.iter().find_map(|value| match value {
        Value::UnixFd(raw) => Some(*raw),
        _ => None,
    });
    let Some(chosen) = chosen else {
        fd::close_all(&fds);
        return Err(DbusError::Protocol("Inhibit reply without unix fd"));
    };
    let mut kept = false;
    for raw in fds {
        if !kept && raw == chosen {
            kept = true;
        } else {
            fd::close_fd(raw);
        }
    }
    if !kept {
        // Ответ ссылался на fd, которого нет в очереди: закрывать больше нечего.
        return Err(DbusError::Protocol("Inhibit fd is not in the message"));
    }
    Ok(SleepInhibitor { fd: chosen })
}

/// `org.freedesktop.login1.Manager`: подписка на сон/пробуждение.
#[derive(Debug)]
pub struct PowerMonitor {
    connection: Connection,
}

impl PowerMonitor {
    /// Подключается к шине, ничего не подписывая.
    pub fn connect(address: &BusAddress) -> Result<Self, DbusError> {
        Ok(Self {
            connection: Connection::connect(address)?,
        })
    }

    /// `Inhibit("sleep", "discord-lab", why, "block") -> h`: блокировать сон, пока жив fd.
    ///
    /// fd приходит ancillary-данными (`SCM_RIGHTS`) и хранится в [`SleepInhibitor`]:
    /// `Drop` закрывает его, что и снимает ингибитор. Пустой `why` отвергается локально
    /// (`Protocol`) — такой вызов не уходит на шину. Любая ошибка (`ERROR`, нет fd,
    /// обрыв) возвращает `Err`, уже полученные fd закрываются.
    pub fn inhibit_sleep(&mut self, why: &str) -> Result<SleepInhibitor, DbusError> {
        if why.is_empty() {
            return Err(DbusError::Protocol("empty why"));
        }
        let body = [
            ('s', Value::String(SLEEP_INHIBIT_WHAT.to_owned())),
            ('s', Value::String(SLEEP_INHIBIT_WHO.to_owned())),
            ('s', Value::String(why.to_owned())),
            ('s', Value::String(SLEEP_INHIBIT_MODE.to_owned())),
        ];
        let (values, fds) = self.connection.call_with_fds(
            LOGIN1_DESTINATION,
            LOGIN1_PATH,
            LOGIN1_INTERFACE,
            SLEEP_INHIBIT_METHOD,
            &body,
        )?;
        take_inhibitor(&values, fds)
    }

    /// `AddMatch` на `PrepareForSleep` менеджера logind.
    pub fn subscribe(&mut self) -> Result<(), DbusError> {
        // sender не фиксируется: шина приписывает сигналы уникальному имени отправителя,
        // а path/interface/member клиент проверяет сам в `sleep_event`.
        let rule = format!(
            "type='signal',path='{LOGIN1_PATH}',interface='{LOGIN1_INTERFACE}',\
             member='{SLEEP_SIGNAL}'"
        );
        self.connection.add_match(&rule)
    }

    /// Следующее событие сна/пробуждения.
    ///
    /// Чужие сигналы и `PrepareForSleep` с неожиданным телом пропускаются (не ошибка);
    /// таймаут ожидания — `Ok(None)`. Битое/закрытое соединение — `DbusError`.
    pub fn next_event(&mut self, timeout: Duration) -> Result<Option<PowerEvent>, DbusError> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Ok(None);
            }
            match self.connection.next_signal(remaining) {
                Ok(mut signal) => {
                    if let Some(event) = sleep_event(&signal) {
                        return Ok(Some(event));
                    }
                    // Не наш сигнал: если в теле пришли fd, их нужно закрыть.
                    signal.close_fds();
                }
                Err(DbusError::Timeout) => return Ok(None),
                Err(error) => return Err(error),
            }
        }
    }
}

fn sleep_event(signal: &Signal) -> Option<PowerEvent> {
    if signal.path != LOGIN1_PATH
        || signal.interface != LOGIN1_INTERFACE
        || signal.member != SLEEP_SIGNAL
    {
        return None;
    }
    // Сигнатура `b`: тело — одно значение, без struct-обёртки.
    match signal.body.as_slice() {
        [Value::Bool(true)] => Some(PowerEvent::GoingToSleep),
        [Value::Bool(false)] => Some(PowerEvent::WakingUp),
        _ => None,
    }
}

fn is_unknown_member(name: &str) -> bool {
    matches!(
        name,
        "org.freedesktop.DBus.Error.UnknownMethod" | "org.freedesktop.DBus.Error.UnknownInterface"
    )
}

/// Авто-статус присутствия по времени бездействия.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresenceStatus {
    /// Пользователь активен.
    Online,
    /// Бездействие достигло `idle_after_secs`.
    Idle,
    /// Бездействие достигло `dnd_after_secs`.
    Dnd,
}

/// Пороги `idle → status` — конфигурация продукта, а не RE-факт клиента Discord.
///
/// [`IdlePolicy::DEFAULT`] (600/1800 с) — продуктовые значения из ТЗ P6-06; точные пороги
/// нативного клиента в RE не зафиксированы (`RE-POWER.md` §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdlePolicy {
    /// С какого времени бездействия статус становится `Idle`.
    pub idle_after_secs: u64,
    /// С какого времени бездействия статус становится `Dnd`.
    pub dnd_after_secs: u64,
}

impl IdlePolicy {
    /// 10 минут до `Idle`, 30 минут до `Dnd` — конфигурация, не подтверждённый RE-факт.
    pub const DEFAULT: Self = Self {
        idle_after_secs: 600,
        dnd_after_secs: 1800,
    };

    /// Статус по времени бездействия.
    ///
    /// `idle_after == dnd_after` → `Dnd` уже при `>=`; нулевые пороги срабатывают сразу;
    /// сравнения только `u64`, без арифметики — переполнение невозможно.
    pub fn status(&self, idle_secs: u64) -> PresenceStatus {
        if idle_secs >= self.dnd_after_secs {
            PresenceStatus::Dnd
        } else if idle_secs >= self.idle_after_secs {
            PresenceStatus::Idle
        } else {
            PresenceStatus::Online
        }
    }
}
