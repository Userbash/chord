# TZ W30b — P6-02 D-Bus-уведомления на Linux (headless, без новых зависимостей)

Волна: RW-30. Дата: 2026-10-22. Крейт: `discord-platform`. Сеть не используется; D-Bus —
локальный unix-сокет.

## 1. Замороженный контракт

```rust
pub mod dbus {
    pub struct BusAddress(/* path */);
    impl BusAddress {
        pub fn parse(value: &str) -> Option<Self>;      // unix:path=…, unix:abstract=… (abstract → None)
        pub fn from_env() -> Option<Self>;              // DBUS_SESSION_BUS_ADDRESS
        pub fn path(&self) -> &std::path::Path;
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum Value {
        Byte(u8), Bool(bool), U32(u32), I32(i32), String(String),
        Array(Vec<Value>), Struct(Vec<Value>), Dict(Vec<(Value, Value)>), Variant(Box<Value>),
    }

    #[derive(Debug)]
    pub enum DbusError { Address, Io(std::io::Error), Auth(&'static str), Protocol(&'static str),
                         Remote { name: String, message: String }, Timeout }

    pub struct Connection { /* std::os::unix::net::UnixStream */ }
    impl Connection {
        pub fn connect(address: &BusAddress) -> Result<Self, DbusError>; // AUTH EXTERNAL + HELLO
        pub fn unique_name(&self) -> &str;
        pub fn call(&mut self, destination: &str, path: &str, interface: &str, member: &str,
                    body: &[(char, Value)]) -> Result<Vec<Value>, DbusError>; // (signature char, value)
        pub fn add_match(&mut self, rule: &str) -> Result<(), DbusError>;
        pub fn next_signal(&mut self, timeout: std::time::Duration) -> Result<Signal, DbusError>;
    }

    pub struct Signal { pub path: String, pub interface: String, pub member: String, pub body: Vec<Value> }
}

pub mod notifications {
    pub const DESTINATION: &str = "org.freedesktop.Notifications";
    pub const PATH: &str = "/org/freedesktop/Notifications";

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct Notification {
        pub app_name: String,
        pub summary: String,
        pub body: String,
        pub replaces_id: u32,
        pub actions: Vec<(String, String)>,   // (action id, label)
        pub timeout_ms: i32,                 // -1 = server default
    }

    pub struct Notifier { /* Connection */ }
    impl Notifier {
        pub fn connect(address: &dbus::BusAddress) -> Result<Self, dbus::DbusError>;
        pub fn capabilities(&mut self) -> Result<Vec<String>, dbus::DbusError>;
        pub fn server_information(&mut self) -> Result<(String, String, String, String), dbus::DbusError>;
        pub fn notify(&mut self, notification: &Notification) -> Result<u32, dbus::DbusError>;
        pub fn close(&mut self, id: u32) -> Result<(), dbus::DbusError>;
        pub fn listen_actions(&mut self) -> Result<(), dbus::DbusError>; // AddMatch ActionInvoked/NotificationClosed
        pub fn next_event(&mut self) -> Result<NotificationEvent, dbus::DbusError>;
    }

    pub enum NotificationEvent {
        Action { id: u32, action: String },
        Closed { id: u32, reason: u32 },
    }
}
```

Инварианты:

- Только `std` (`UnixStream`, `std::process` в тестах), без `unsafe`, паник и новых зависимостей.
- D-Bus: little-endian; заголовок 16 Б (`l`/`yyyy`), поля заголовка в порядке спецификации,
  выравнивание по типу (4 для `u/i`, 8 для `x/t/d`, 4 для `a`/`struct`, 4 для строки `u32`+байты+NUL);
  `AUTH EXTERNAL <hex ASCII uid>` (uid из `geteuid`), ожидание `OK <guid>`, `BEGIN`.
- `call`: монотонный serial (wrapping), ответ сопоставляется по `reply_serial`, `ERROR` →
  `DbusError::Remote`; входящие сигналы во время `call` складываются в очередь, а не теряются.
- `notify`: `Notify(susssasa{sv}i)`; `actions` — `a(ss)`; `hints` — пустой `a{sv}`
  (обосновано: `RE-NOTIFICATIONS.md` §3, минимум для Linux-реализации); `app_name`/`summary`
  непустые (иначе `DbusError::Protocol`).
- `listen_actions`: `AddMatch` на `type='signal',interface='org.freedesktop.Notifications',
  member='ActionInvoked'` и `NotificationClosed`; `next_event` разбирает `(us)`/`(uu)`,
  чужие сигналы пропускает (не ошибка).
- `BusAddress::parse`: поддержать `unix:path=…` и список адресов через `;` (первый
  поддерживаемый); `abstract=` → `None` (документировано).

## 2. Оракул и доказательная база

- `waves/RW-30/RE-NOTIFICATIONS.md` — RE-выжимка: Discord на Linux **не загружает**
  `discord_notifications` (`main-arch.md` §3.2), значит реализация своя; IPC-каналы
  `DISCORD_NOTIFICATIONS_*`; допустимые действия (открыть/ответить) и DND.
- Независимый оракул: приватный `dbus-daemon --session --print-address` + мок-сервис
  `waves/RW-30/dbus/mock_service.py` на **python3-dbus (libdbus)**: регистрирует
  `org.freedesktop.Notifications`, отвечает `Notify/CloseNotification/GetCapabilities/
  GetServerInformation`, эмитит `ActionInvoked`/`NotificationClosed`, пишет журнал вызовов.
  Тесты Rust поднимают демон и мок, проверяют вызовы и байтовый round-trip.
- `waves/RW-30/dbus/gen-golden.py` — фиксирует «золотые» ответы/сигнатуры в
  `fixtures/dbus/*.json` (signature, args, reply) для unit-тестов без запуска демона.
- Live-сервис (`org.freedesktop.Notifications` на сессионной шине) — опциональный
  skip-aware тест; уведомление не отправляется без `DISCORD_LAB_LIVE_NOTIFY=1`.

## 3. Кейсы (red-first)

| ID | Проверка | Ожидание |
|---|---|---|
| n01 | `BusAddress::parse` path/abstract/мусор/`;` | path/None/None/первый валидный |
| n02 | auth EXTERNAL на приватном демоне | `OK`, уникальное имя `:1.x` |
| n03 | маршалинг `s`, `u`, `i`, `b`, `as` | байты по спецификации |
| n04 | выравнивание `a(ss)`, `a{sv}` | корректные padding |
| n05 | header round-trip (path/iface/member/signature/reply_serial) | поля совпадают |
| n06 | `Hello` → `unique_name` | `:1.x`, не пусто |
| n07 | `GetCapabilities` через мок | список из мока |
| n08 | `Notify` 4 аргумента → id; мок видит summary/body/app_name | id, журнал |
| n09 | `actions` `a(ss)` | пары доходят в мок |
| n10 | `CloseNotification` | мок фиксирует id |
| n11 | ERROR-ответ (мок бросает) | `DbusError::Remote{name,message}` |
| n12 | `ActionInvoked`/`NotificationClosed` | `NotificationEvent` |
| n13 | чужие сигналы в `next_event` | пропускаются |
| n14 | обрезанный/битый пакет | `Protocol`, без паники |
| n15 | повторный `close` и закрытая сессия | ошибка/`Ok`, без паники |
| n16 | live-сервис при `DISCORD_LAB_LIVE_NOTIFY=1` | skip иначе |

Бюджет: 14–18 тестов; мутации 8–10 (auth hex, alignment, reply_serial, error-ветка,
actions/struct, match-rule, event-фильтр, serial, app_name guard).

## 4. Команды

```sh
node waves/RW-30/dbus/gen-golden.py     # или python3 — по реализации генератора
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w30b cargo test --offline -p discord-platform --test dbus_contract
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w30b cargo test --offline -p discord-platform --test notifications_contract
```

Тесты skip-aware: нет `dbus-daemon`/`python3-dbus` → skip с причиной (не падение).

## 5. DoD

red-лог с exit code → green → мутации (restore по md5) → отчёт `waves/RW-30/REPORT-W30b.md`.
