# TZ W35a — fd-передача D-Bus (SCM_RIGHTS) и logind sleep-inhibitor

Волна: RW-35. Дата: 2026-10-23. Крейт: `discord-platform`. Разрешена `libc = "0.2"` (офлайн).

## 1. Замороженный контракт

```rust
// dbus/message.rs
pub enum Value {
    // ... существующие ...
    UnixFd(std::os::fd::RawFd),
}

// dbus/mod.rs
impl Connection {
    /// Вызов с приёмом fd; `body` может содержать `h` (индексы файловых дескрипторов).
    pub fn call_with_fds(
        &mut self,
        destination: &str,
        path: &str,
        interface: &str,
        member: &str,
        body: &[(char, Value)],
    ) -> Result<(Vec<Value>, Vec<std::os::fd::RawFd>), DbusError>;
}

// power.rs
pub struct SleepInhibitor { /* RawFd */ }
impl SleepInhibitor {
    pub fn fd(&self) -> std::os::fd::RawFd;      // тестовый хук
}
impl Drop for SleepInhibitor { /* close(fd) */ }

impl PowerMonitor {
    /// `org.freedesktop.login1.Manager.Inhibit("sleep", "discord-lab", why, "block")` → fd.
    pub fn inhibit_sleep(&mut self, why: &str) -> Result<SleepInhibitor, DbusError>;
}
```

Инварианты:

- **FD приём:** `recvmsg` через `libc` с `SCM_RIGHTS`; fd складываются в очередь сообщения;
  `MSG_CTRUNC` → `DbusError::Protocol`; лишние/битые fd закрываются (`close`), утечек нет.
- Тип `h` (`Value::UnixFd`) читает индекс из очереди fd; индекс вне диапазона → `Protocol`.
- **FD отправка:** `call_with_fds` поддерживает `h` в теле (sendmsg + SCM_RIGHTS), но для
  logind нужен только приём; отправка проверяется unit-тестом (2 fd через pipe).
- `call` остаётся эквивалентом `call_with_fds` с отбрасыванием fd (но закрывает их, если
  метод вернул fd — иначе утечка); документируется.
- `SleepInhibitor::drop` закрывает fd ровно один раз (`close`), повторный drop невозможен
  (владение); fd не клонируется.
- `inhibit_sleep`: пустой `why` → `DbusError::Protocol`; `ERROR`/нет fd → ошибка; при ошибке
  полученные fd закрываются.
- Только `libc` + std; `unsafe` — в `dbus/fd.rs` (`#[allow(unsafe_code)]`, `// SAFETY:`),
  `unsafe_code = "deny"` остаётся; вне ffi нет паник/`unwrap`/`expect`.

## 2. Оракул

Приватный `dbus-daemon` + `waves/RW-35/power/mock_logind.py` (python3-dbus) реализует
`org.freedesktop.login1.Manager.Inhibit(ssss) -> h`, отдаёт реальный fd (pipe/memfd), журналирует
аргументы. Live `systemd-logind` — skip-aware (`DISCORD_LAB_POWER_LIVE=1`), без реального
ингибирования сна в CI.

## 3. Кейсы (red-first)

| ID | Проверка | Ожидание |
|---|---|---|
| f01 | `Value::UnixFd` парсинг/`h` | индекс → fd |
| f02 | mock Inhibit → fd | валидный fd, аргументы в журнале |
| f03 | Drop закрывает fd | `/proc/self/fd` без роста |
| f04 | 2 fd в одном сообщении | оба получены и закрыты |
| f05 | индекс вне диапазона | `Protocol`, без паники |
| f06 | `MSG_CTRUNC`/битый ancillary | `Protocol`, fd закрыты |
| f07 | `call` с fd в ответе | fd закрывается, утечки нет |
| f08 | пустой `why` | `Protocol`, вызова нет |
| f09 | отправка 2 fd (pipe) mock-серверу | получены |
| f10 | live logind | skip-aware |

Бюджет: 9–11 тестов; мутации 5–7 (индекс, close, CTRUNC, sendmsg, пустой why).

## 4. Команды

```sh
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w35a cargo test --offline -p discord-platform --test dbus_contract --test power_contract
```

## 5. DoD

red → green → мутации → отчёт `waves/RW-35/REPORT-W35a.md`.
