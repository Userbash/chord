# TZ W31a — P6-06 Питание и idle (headless)

Волна: RW-31. Дата: 2026-10-22. Крейт: `discord-platform`. Сеть не используется.
Опирается на D-Bus-клиент P6-02 (`discord-platform/src/dbus/*`, RW-30).

## 1. Замороженный контракт

```rust
use crate::dbus::{BusAddress, Connection, DbusError, Signal};

/// org.freedesktop.ScreenSaver (сессионная шина): не гасить экран в звонке.
pub struct ScreenSaver { /* Connection */ }
impl ScreenSaver {
    pub fn connect(address: &BusAddress) -> Result<Self, DbusError>;
    pub fn inhibit(&mut self, app_name: &str, reason: &str) -> Result<u32, DbusError>; // cookie
    pub fn uninhibit(&mut self, cookie: u32) -> Result<(), DbusError>;
    pub fn is_active(&mut self) -> Result<bool, DbusError>;
    /// `org.freedesktop.ScreenSaver.GetSessionIdleTime` (KDE); `None` — метод не поддержан.
    pub fn idle_seconds(&mut self) -> Result<Option<u64>, DbusError>;
}

/// org.freedesktop.login1.Manager (системная шина): события сна/пробуждения.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerEvent { GoingToSleep, WakingUp }

pub struct PowerMonitor { /* Connection */ }
impl PowerMonitor {
    pub fn connect(address: &BusAddress) -> Result<Self, DbusError>;
    pub fn subscribe(&mut self) -> Result<(), DbusError>; // AddMatch PrepareForSleep
    pub fn next_event(&mut self, timeout: std::time::Duration) -> Result<Option<PowerEvent>, DbusError>;
}

/// Авто-статус по бездействию (порог — конфигурация, не RE-факт).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresenceStatus { Online, Idle, Dnd }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdlePolicy { pub idle_after_secs: u64, pub dnd_after_secs: u64 }
impl IdlePolicy {
    pub const DEFAULT: Self; // 600 / 1800 — продуктовые значения, не подтверждены RE
    pub fn status(&self, idle_secs: u64) -> PresenceStatus;
}
```

Инварианты:

- `status`: `idle_secs < idle_after` → `Online`; `idle_after <= idle_secs < dnd_after` → `Idle`;
  `>= dnd_after` → `Dnd`; `idle_after == dnd_after` → `Dnd` при `>=`; нулевые пороги
  трактуются как «сразу idle/dnd»; `u64::MAX` без переполнения.
- `ScreenSaver::inhibit`: `Inhibit(s,s) -> u`; пустые `app_name`/`reason` → `DbusError::Protocol`.
  `uninhibit`: `UnInhibit(u)` (имя метода сервиса — `UnInhibit`; проверить и при расхождении
  принимать `Uninhibit`, зафиксировать в отчёте); неизвестный cookie — ошибка сервера, не паника.
- `idle_seconds`: если сервер вернул `UnknownMethod`/`UnknownInterface` — `Ok(None)`; прочие
  ошибки — `Err`.
- `PowerMonitor`: путь `/org/freedesktop/login1`, интерфейс `org.freedesktop.login1.Manager`,
  сигнал `PrepareForSleep(b)`; `true` → `GoingToSleep`, `false` → `WakingUp`; чужие сигналы
  пропускаются; таймаут → `Ok(None)`.
- Никаких паник/`unsafe`/новых зависимостей; только `std` + `crate::dbus`.
- Соединение закрыто/битый пакет → `DbusError`, без паники.

## 2. Доказательная база

`waves/RW-31/RE-POWER.md`: `POWER_MONITOR_*` IPC-каналы и `POWER_SAVE_BLOCKER_*`
(`/home/sanya/re-deep/backend-ipc.md`), `discord_utils` (Wayland-idle, X11) в
`docs/re/architecture.md`, `native-re.md` §WaylandIdleManager. Точные пороги idle→status в RE
не зафиксированы — вынесены в конфигурацию с явной пометкой.

## 3. Кейсы (red-first)

| ID | Проверка | Ожидание |
|---|---|---|
| p01 | `IdlePolicy::status` границы 0/599/600/601/1799/1800 | Online/Idle/Dnd |
| p02 | `inhibit` на мок-сервисе | cookie, мок видит app/reason |
| p03 | `uninhibit(cookie)` | мок получил вызов |
| p04 | `is_active` true/false | значение мока |
| p05 | `idle_seconds` Some/UnknownMethod | `Some(n)` / `None` |
| p06 | `PrepareForSleep(true/false)` | GoingToSleep/WakingUp |
| p07 | чужие сигналы между событиями | пропущены |
| p08 | таймаут `next_event` | `Ok(None)` |
| p09 | пустые app_name/reason | `Protocol` |
| p10 | закрытое соединение | ошибка, без паники |
| p11 | live-сервис сессии | skip-aware |
| p12 | нет `dbus-daemon`/python-dbus | skip с причиной |

Бюджет: 10–12 тестов; мутации 5–7 (границы policy, sleep-флаг, таймаут, UnknownMethod,
cookie, фильтр сигналов).

## 4. Команды

```sh
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w31a cargo test --offline -p discord-platform --test power_contract
```

Полный gate — один раз в integration-лейне.

## 5. DoD

red-лог с exit code → green → мутации (restore по md5) → отчёт `waves/RW-31/REPORT-W31a.md`.
