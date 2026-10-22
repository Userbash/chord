# TZ W34a — D-Bus abstract-адреса (headless)

Волна: RW-34. Дата: 2026-10-23. Крейт: `discord-platform`. Без новых зависимостей и `unsafe`.

## 1. Замороженный контракт

```rust
pub enum BusAddress {
    Path(std::path::PathBuf),
    Abstract(Vec<u8>),
}

impl BusAddress {
    pub fn parse(value: &str) -> Option<Self>;
    pub fn from_env() -> Option<Self>;
    pub fn path(&self) -> Option<&std::path::Path>;      // None для abstract
    pub fn abstract_name(&self) -> Option<&[u8]>;
}

impl Connection {
    // существующая сигнатура сохраняется:
    pub fn connect(address: &BusAddress) -> Result<Self, DbusError>;
}
```

Инварианты:

- `parse`: `unix:path=/abs/path` → `Path`; `unix:abstract=name` → `Abstract` (байты, до 107);
  список `;` — берётся первый поддерживаемый; пустое имя/путь, NUL, `tcp:` → `None`.
- `path()`/`abstract_name()` — ровно один из них `Some`.
- `Connection::connect`: для `Path` — `UnixStream::connect`; для `Abstract` —
  `std::os::linux::net::SocketAddrExt::from_abstract_name` + `UnixStream::connect_addr`.
  Ошибки — `DbusError::Io`/`Address`, без паник и `unsafe`.
- Аутентификация/Hello/вызовы не меняются; `notifications`/`power` продолжают работать.

## 2. Оракул

Приватный `dbus-daemon --session --address=unix:abstract=discord-lab-<pid>-<n>` +
существующий libdbus-мок; тест делает Hello, `GetCapabilities` и проверяет журнал. Если
abstract-сокеты в окружении запрещены — skip с причиной (не падение).

## 3. Кейсы (red-first)

| ID | Проверка | Ожидание |
|---|---|---|
| a01 | parse path | Path, `path()` Some |
| a02 | parse abstract | Abstract + байты |
| a03 | список `;`, мусор, `tcp:` | первый валидный / None |
| a04 | connect abstract к приватному daemon | Hello, unique name |
| a05 | вызов через abstract-шину | ответ мока |
| a06 | несуществующий abstract | `DbusError`, без паники |
| a07 | path-ветка не сломана | старые кейсы dbus_contract зелёные |

Бюджет: 6–8 тестов; мутации 4–6 (parse-ветки, connect_addr, abstract-байты).

## 4. Команды

```sh
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w34a cargo test --offline -p discord-platform --test dbus_contract
```

## 5. DoD

red → green → мутации → отчёт `waves/RW-34/REPORT-W34a.md`.
