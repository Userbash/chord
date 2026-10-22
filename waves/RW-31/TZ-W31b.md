# TZ W31b — P2-03 SQLite-store через FFI к системному libsqlite3

Волна: RW-31. Дата: 2026-10-22. Крейт: `discord-store`. Решение владельца: вариант **A**
(`waves/RW-31/SQLITE-DECISION.md`). Сеть не используется.

## 1. Замороженный контракт

```rust
pub struct MessageRow {
    pub id: String,
    pub channel_id: String,
    pub author_id: String,
    pub content: String,
    pub timestamp: i64,
    pub edited_timestamp: Option<i64>,
    pub flags: u64,
    pub deleted: bool,
}

pub struct ChannelRow {
    pub id: String,
    pub guild_id: Option<String>,
    pub name: Option<String>,
    pub kind: u8,
    pub position: i64,
}

pub enum StoreError {
    Open(String),
    Sqlite { code: i32, message: String },
    Constraint(String),
    Migration(String),
}

pub struct Store { /* *mut sqlite3 + path */ }
impl Store {
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, StoreError>;
    pub fn open_in_memory() -> Result<Self, StoreError>;
    pub fn migrate(&mut self) -> Result<(), StoreError>;        // идемпотентно, user_version = 1
    pub fn schema_version(&self) -> Result<u32, StoreError>;
    pub fn put_channel(&mut self, row: &ChannelRow) -> Result<(), StoreError>;   // upsert по id
    pub fn channel(&self, id: &str) -> Result<Option<ChannelRow>, StoreError>;
    pub fn put_message(&mut self, row: &MessageRow) -> Result<(), StoreError>;   // upsert по id
    pub fn messages(&self, channel_id: &str, before: Option<&str>, limit: usize)
        -> Result<Vec<MessageRow>, StoreError>;                 // ORDER BY timestamp DESC, id DESC
    pub fn delete_message(&mut self, id: &str) -> Result<bool, StoreError>;     // soft delete
    pub fn kv_set(&mut self, key: &str, value: &str) -> Result<(), StoreError>;
    pub fn kv_get(&self, key: &str) -> Result<Option<String>, StoreError>;
}
```

Инварианты:

- **Безопасность:** весь `unsafe` — только в `src/ffi.rs`; в `Cargo.toml` крейта
  `unsafe_code = "deny"` (было `forbid`) + `#[allow(unsafe_code)]` на модуль `ffi`; каждый
  `unsafe`-блок с `// SAFETY:`-комментарием; `#![deny(unsafe_op_in_unsafe_fn)]`.
  Вне `ffi.rs` нет `unsafe`, `unwrap`, `expect`, `panic!` в продакшн-пути.
- Линковка: `#[link(name = "sqlite3")]` (системная библиотека, без новых крейтов и правок
  workspace `Cargo.toml`).
- `migrate`: транзакция, `PRAGMA foreign_keys = ON`, `journal_mode = WAL` (для файла; для
  `:memory:` — допустимый фолбэк), таблицы `channels`, `messages`, `kv`; индексы
  `messages(channel_id, timestamp DESC)`, `messages(channel_id, id DESC)`; `user_version = 1`.
  Повторный вызов — no-op; `schema_version()` читает `PRAGMA user_version`.
- `messages`: `limit == 0` → пустой вектор; limit > 1000 клампится до 1000; `before` —
  keyset-пагинация по `(timestamp, id)` строго меньше курсора; `deleted = 0`; параметры
  биндятся (`sqlite3_bind_*`), конкатенации SQL со значениями нет.
- `put_message` без существующего канала → `StoreError::Constraint` (FK).
- `put_*` — upsert (`ON CONFLICT(id) DO UPDATE`), повторный вызов не дублирует.
- Строки с Unicode/`'`/`;`/NUL-подобным мусором сохраняются дословно (NUL в C-строке
  недопустим — такое значение → `StoreError::Sqlite`, без паники).
- `open` несуществующего каталога → `Open`, без паники; БД переоткрывается после `drop`
  с сохранением данных.
- `delete_message` возвращает `true`, если строка была помечена (или уже удалена — `false`
  при отсутствии).

## 2. Оракул и проверка

- Независимый оракул — Python 3 `sqlite3` (stdlib): тест открывает созданный Rust-ом файл,
  читает `PRAGMA user_version`, схему, данные, проверяет отсутствие дублей и корректность
  keyset-пагинации. Нет `python3` → skip с причиной.
- Схема/поведение сверяются с `docs/rust-client/FEATURES.md` F-DATA-02 и `docs/feature-backlog.md`
  (D-серия); RE-оракул — только имена/типы таблиц, без выдуманных полей Discord.

## 3. Кейсы (red-first)

| ID | Проверка | Ожидание |
|---|---|---|
| s01 | `open_in_memory` + `migrate` | `schema_version == 1`, повторный migrate — no-op |
| s02 | channel upsert/read | данные и Unicode дословно |
| s03 | message upsert | без дублей, обновление content |
| s04 | messages: порядок/limit=0/limit>1000 | DESC, клампинг |
| s05 | keyset `before` | строго старше курсора, без пропусков |
| s06 | FK: message без канала | `Constraint` |
| s07 | soft delete | `messages` не отдаёт, `delete_message` → true/false |
| s08 | kv set/get/overwrite | значение перезаписано |
| s09 | reopen файла | данные и версия сохранены |
| s10 | инъекция `'; DROP TABLE` в id/content | таблица цела, значение дословно |
| s11 | несуществующий каталог | `Open`, без паники |
| s12 | Python-оракул файла | user_version/схема/данные совпадают |
| s13 | NUL/огромная строка | ошибка или корректное сохранение, без паники |

Бюджет: 12–15 тестов; мутации 6–8 (FK off, ORDER BY, клампинг, upsert, soft-delete, bind vs
конкатенация, user_version).

## 4. Команды

```sh
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w31b cargo test --offline -p discord-store --test store_contract
```

Полный gate — один раз в integration-лейне.

## 5. DoD

red-лог с exit code → green → мутации (restore по md5) → отчёт `waves/RW-31/REPORT-W31b.md`.
