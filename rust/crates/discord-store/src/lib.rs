#![deny(unsafe_op_in_unsafe_fn)]
//! # discord-store
//!
//! Локальное хранилище клиента (SQLite): каналы, сообщения, служебный key-value.
//! Офлайн-старт, история канала и keyset-пагинация (F-DATA-02/F-DATA-06 в
//! `docs/rust-client/FEATURES.md`).
//!
//! Реализация — P2-03 (RW-31, вариант A из `waves/RW-31/SQLITE-DECISION.md`): безопасный API
//! поверх системного `libsqlite3` (3.54). Весь `unsafe` изолирован в приватном модуле [`ffi`];
//! здесь нет `unsafe`, `unwrap`, `expect` и `panic!`. Значения всегда биндятся
//! (`sqlite3_bind_*`), SQL со значениями не конкатенируется.
//!
//! Ограничения (см. отчёт `waves/RW-31/REPORT-W31b.md`):
//!
//! - `Store` однопоточный: одно соединение, один писатель; конкурентный доступ не заявляется.
//! - `journal_mode = WAL` действует только для файловой БД; для `:memory:` SQLite оставляет
//!   `memory` — это допустимый фолбэк, а не ошибка.
//! - Строки с внутренним NUL не сохраняются (C-строка): возвращается [`StoreError::Sqlite`].
//! - Wire-совместимость с внутренней схемой Discord не заявляется: таблицы — наши (`v1`).
//! - `flags: u64` хранится в SQLite `INTEGER` (i64); значения > `i64::MAX` — `Constraint`.

use std::fmt;
use std::path::{Path, PathBuf};

#[allow(unsafe_code)]
mod ffi;

/// Стартовый набор таблиц ядра данных (по F-DATA-02). В схеме v1 реализованы `channels`.
pub const CORE_TABLES: [&str; 5] = ["guilds", "channels", "roles", "users", "emoji"];

/// Версия схемы хранилища (`PRAGMA user_version`).
pub const SCHEMA_VERSION: u32 = 1;

/// Верхняя граница `limit` для [`Store::messages`] (TZ §1).
const MAX_MESSAGES_LIMIT: usize = 1000;

// --- схема v1 -------------------------------------------------------------------

const SQL_SCHEMA_V1: &str = "
CREATE TABLE IF NOT EXISTS channels (
    id       TEXT PRIMARY KEY NOT NULL,
    guild_id TEXT,
    name     TEXT,
    kind     INTEGER NOT NULL,
    position INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS messages (
    id               TEXT PRIMARY KEY NOT NULL,
    channel_id       TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    author_id        TEXT NOT NULL,
    content          TEXT NOT NULL,
    timestamp        INTEGER NOT NULL,
    edited_timestamp INTEGER,
    flags            INTEGER NOT NULL,
    deleted          INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_messages_channel_timestamp ON messages(channel_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_messages_channel_id ON messages(channel_id, id DESC);
CREATE TABLE IF NOT EXISTS kv (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);
";

const SQL_UPSERT_CHANNEL: &str = "
INSERT INTO channels (id, guild_id, name, kind, position)
VALUES (?1, ?2, ?3, ?4, ?5)
ON CONFLICT(id) DO UPDATE SET
    guild_id = excluded.guild_id,
    name = excluded.name,
    kind = excluded.kind,
    position = excluded.position
";

const SQL_UPSERT_MESSAGE: &str = "
INSERT INTO messages (id, channel_id, author_id, content, timestamp, edited_timestamp, flags, deleted)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
ON CONFLICT(id) DO UPDATE SET
    channel_id = excluded.channel_id,
    author_id = excluded.author_id,
    content = excluded.content,
    timestamp = excluded.timestamp,
    edited_timestamp = excluded.edited_timestamp,
    flags = excluded.flags,
    deleted = excluded.deleted
";

const SQL_SELECT_CHANNEL: &str = "
SELECT id, guild_id, name, kind, position FROM channels WHERE id = ?1
";

/// Первая страница истории канала (самые новые сообщения).
const SQL_MESSAGES_HEAD: &str = "
SELECT id, channel_id, author_id, content, timestamp, edited_timestamp, flags, deleted
FROM messages
WHERE channel_id = ?1 AND deleted = 0
ORDER BY timestamp DESC, id DESC
LIMIT ?2
";

/// Страница строго старше курсора `(timestamp, id)` — keyset-пагинация.
const SQL_MESSAGES_BEFORE: &str = "
SELECT id, channel_id, author_id, content, timestamp, edited_timestamp, flags, deleted
FROM messages
WHERE channel_id = ?1 AND deleted = 0 AND (timestamp < ?2 OR (timestamp = ?2 AND id < ?3))
ORDER BY timestamp DESC, id DESC
LIMIT ?4
";

const SQL_SELECT_MESSAGE_CURSOR: &str = "SELECT timestamp FROM messages WHERE id = ?1";

const SQL_UPSERT_KV: &str = "
INSERT INTO kv (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value
";

const SQL_SELECT_KV: &str = "SELECT value FROM kv WHERE key = ?1";

// --- модель ---------------------------------------------------------------------

/// Строка сообщения (замороженный контракт W31b §1).
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Строка канала (замороженный контракт W31b §1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelRow {
    pub id: String,
    pub guild_id: Option<String>,
    pub name: Option<String>,
    pub kind: u8,
    pub position: i64,
}

/// Ошибки хранилища. Ни один путь не паникует: всё возвращается значением.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// Не удалось открыть БД (нет каталога, путь не UTF-8, отказ SQLite).
    Open(String),
    /// Ошибка SQLite: `code` — расширенный код (например 787 — FK), `message` — текст.
    Sqlite { code: i32, message: String },
    /// Нарушено ограничение схемы (FK, диапазон значения).
    Constraint(String),
    /// Схема/миграция: неподдерживаемая версия или битая метаинформация.
    Migration(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open(message) => write!(formatter, "не удалось открыть хранилище: {message}"),
            Self::Sqlite { code, message } => write!(formatter, "ошибка SQLite {code}: {message}"),
            Self::Constraint(message) => write!(formatter, "нарушено ограничение: {message}"),
            Self::Migration(message) => write!(formatter, "ошибка схемы: {message}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<ffi::FfiError> for StoreError {
    fn from(error: ffi::FfiError) -> Self {
        if error.primary_code() == ffi::SQLITE_CONSTRAINT {
            Self::Constraint(error.message)
        } else {
            Self::Sqlite {
                code: error.code,
                message: error.message,
            }
        }
    }
}

// --- хранилище ------------------------------------------------------------------

/// Соединение с локальной БД. Однопоточный: один писатель, без пула соединений.
pub struct Store {
    db: ffi::Db,
    path: PathBuf,
}

impl fmt::Debug for Store {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Store")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl Store {
    /// Открывает (создаёт при отсутствии) файловую БД. Схема — после [`Store::migrate`].
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref();
        let text = path.to_str().ok_or_else(|| {
            StoreError::Open(format!("путь БД не является UTF-8: {}", path.display()))
        })?;
        let db = ffi::Db::open(text).map_err(|error| {
            StoreError::Open(format!(
                "sqlite3_open_v2({}) код {}: {}",
                path.display(),
                error.code,
                error.message
            ))
        })?;
        Ok(Self {
            db,
            path: path.to_path_buf(),
        })
    }

    /// БД в памяти (тесты, эфемерные кэши). `journal_mode = WAL` неприменим — фолбэк `memory`.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::open(":memory:")
    }

    /// Создаёт схему v1 и выставляет `user_version = 1`. Идемпотентно.
    ///
    /// Транзакция `BEGIN IMMEDIATE`; `foreign_keys = ON` и `journal_mode = WAL` выставляются
    /// до неё (внутри транзакции эти PRAGMA не действуют).
    pub fn migrate(&mut self) -> Result<(), StoreError> {
        self.db.busy_timeout(5_000)?;
        self.db.exec("PRAGMA foreign_keys = ON")?;
        // Для файла — WAL; для `:memory:` SQLite вернёт "memory", ошибки нет.
        self.db.exec("PRAGMA journal_mode = WAL")?;

        let version = self.schema_version()?;
        if version > SCHEMA_VERSION {
            return Err(StoreError::Migration(format!(
                "user_version {version} новее поддерживаемой {SCHEMA_VERSION}: обновите клиент"
            )));
        }

        self.db.exec("BEGIN IMMEDIATE")?;
        if let Err(error) = self.apply_schema_v1() {
            // best-effort: транзакцию надо снять, иначе соединение останется в ней;
            // исходная ошибка важнее ошибки отката.
            if let Err(rollback) = self.db.exec("ROLLBACK") {
                return Err(StoreError::Migration(format!(
                    "{error}; откат транзакции не удался: {rollback}"
                )));
            }
            return Err(error);
        }
        self.db.exec("COMMIT")?;
        Ok(())
    }

    fn apply_schema_v1(&self) -> Result<(), StoreError> {
        self.db.exec(SQL_SCHEMA_V1)?;
        // Литерал версии, а не параметр: PRAGMA не принимает bind-параметры.
        self.db.exec("PRAGMA user_version = 1")?;
        Ok(())
    }

    /// Текущая версия схемы из `PRAGMA user_version` (0 — схема ещё не создана).
    pub fn schema_version(&self) -> Result<u32, StoreError> {
        let mut statement = self.db.prepare("PRAGMA user_version")?;
        if !step_row(&mut statement)? {
            return Err(StoreError::Migration(
                "PRAGMA user_version не вернул строку".to_owned(),
            ));
        }
        let version = statement.column_i64(0)?;
        u32::try_from(version)
            .map_err(|_| StoreError::Migration(format!("user_version {version} вне u32")))
    }

    /// Вставляет канал или обновляет существующий по `id`.
    pub fn put_channel(&mut self, row: &ChannelRow) -> Result<(), StoreError> {
        let mut statement = self.db.prepare(SQL_UPSERT_CHANNEL)?;
        statement.bind_text(1, &row.id)?;
        bind_optional_text(&mut statement, 2, row.guild_id.as_deref())?;
        bind_optional_text(&mut statement, 3, row.name.as_deref())?;
        statement.bind_i64(4, i64::from(row.kind))?;
        statement.bind_i64(5, row.position)?;
        step_done(&mut statement)
    }

    /// Читает канал по `id`; `None`, если строки нет.
    pub fn channel(&self, id: &str) -> Result<Option<ChannelRow>, StoreError> {
        let mut statement = self.db.prepare(SQL_SELECT_CHANNEL)?;
        statement.bind_text(1, id)?;
        if !step_row(&mut statement)? {
            return Ok(None);
        }
        let kind = statement.column_i64(3)?;
        let kind = u8::try_from(kind).map_err(|_| corrupt("channels.kind вне диапазона u8"))?;
        Ok(Some(ChannelRow {
            id: required_text(&statement, 0)?,
            guild_id: statement.column_text(1)?,
            name: statement.column_text(2)?,
            kind,
            position: statement.column_i64(4)?,
        }))
    }

    /// Вставляет сообщение или обновляет существующее по `id` (канал должен существовать).
    pub fn put_message(&mut self, row: &MessageRow) -> Result<(), StoreError> {
        let flags = i64::try_from(row.flags).map_err(|_| {
            StoreError::Constraint("messages.flags не влезает в SQLite INTEGER (i64)".to_owned())
        })?;
        let mut statement = self.db.prepare(SQL_UPSERT_MESSAGE)?;
        statement.bind_text(1, &row.id)?;
        statement.bind_text(2, &row.channel_id)?;
        statement.bind_text(3, &row.author_id)?;
        statement.bind_text(4, &row.content)?;
        statement.bind_i64(5, row.timestamp)?;
        match row.edited_timestamp {
            Some(value) => statement.bind_i64(6, value)?,
            None => statement.bind_null(6)?,
        }
        statement.bind_i64(7, flags)?;
        statement.bind_i64(8, if row.deleted { 1 } else { 0 })?;
        step_done(&mut statement)
    }

    /// История канала: `ORDER BY timestamp DESC, id DESC`.
    ///
    /// `before` — id сообщения-курсора: вернутся только строки строго старше `(timestamp, id)`.
    /// `limit == 0` → пусто, `limit > 1000` → 1000. Неизвестный курсор → пустая страница.
    pub fn messages(
        &self,
        channel_id: &str,
        before: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MessageRow>, StoreError> {
        let limit = limit.min(MAX_MESSAGES_LIMIT);
        if limit == 0 {
            return Ok(Vec::new());
        }
        let limit = i64::try_from(limit).map_err(|_| corrupt("limit не влезает в i64"))?;

        let cursor = match before {
            None => None,
            Some(id) => match self.message_timestamp(id)? {
                Some(timestamp) => Some((timestamp, id)),
                // Продолжать не с чего: курсор вне этой БД (или уже вычищен).
                None => return Ok(Vec::new()),
            },
        };

        let mut rows = Vec::new();
        match cursor {
            None => {
                let mut statement = self.db.prepare(SQL_MESSAGES_HEAD)?;
                statement.bind_text(1, channel_id)?;
                statement.bind_i64(2, limit)?;
                while step_row(&mut statement)? {
                    rows.push(read_message(&statement)?);
                }
            }
            Some((timestamp, id)) => {
                let mut statement = self.db.prepare(SQL_MESSAGES_BEFORE)?;
                statement.bind_text(1, channel_id)?;
                statement.bind_i64(2, timestamp)?;
                statement.bind_text(3, id)?;
                statement.bind_i64(4, limit)?;
                while step_row(&mut statement)? {
                    rows.push(read_message(&statement)?);
                }
            }
        }
        Ok(rows)
    }

    /// `(timestamp)` сообщения-курсора; удалённые тоже годятся — курсор не должен «прыгать».
    fn message_timestamp(&self, id: &str) -> Result<Option<i64>, StoreError> {
        let mut statement = self.db.prepare(SQL_SELECT_MESSAGE_CURSOR)?;
        statement.bind_text(1, id)?;
        if !step_row(&mut statement)? {
            return Ok(None);
        }
        Ok(Some(statement.column_i64(0)?))
    }

    /// Soft delete: помечает строку `deleted = 1`, физически не удаляя.
    ///
    /// `true` — строка существует (помечена сейчас или была помечена раньше), `false` — нет.
    pub fn delete_message(&mut self, id: &str) -> Result<bool, StoreError> {
        let mut statement = self
            .db
            .prepare("UPDATE messages SET deleted = 1 WHERE id = ?1")?;
        statement.bind_text(1, id)?;
        step_done(&mut statement)?;
        Ok(self.db.changes() > 0)
    }

    /// Пишет значение в служебный key-value (upsert по ключу).
    pub fn kv_set(&mut self, key: &str, value: &str) -> Result<(), StoreError> {
        let mut statement = self.db.prepare(SQL_UPSERT_KV)?;
        statement.bind_text(1, key)?;
        statement.bind_text(2, value)?;
        step_done(&mut statement)
    }

    /// Читает значение из key-value; `None`, если ключа нет.
    pub fn kv_get(&self, key: &str) -> Result<Option<String>, StoreError> {
        let mut statement = self.db.prepare(SQL_SELECT_KV)?;
        statement.bind_text(1, key)?;
        if !step_row(&mut statement)? {
            return Ok(None);
        }
        Ok(statement.column_text(0)?)
    }
}

// --- помощники ------------------------------------------------------------------

fn corrupt(message: impl Into<String>) -> StoreError {
    StoreError::Sqlite {
        code: ffi::SQLITE_CORRUPT,
        message: message.into(),
    }
}

/// `true` — пришла строка; `false` — результат закончился.
fn step_row(statement: &mut ffi::Stmt<'_>) -> Result<bool, StoreError> {
    match statement.step()? {
        ffi::Step::Row => Ok(true),
        ffi::Step::Done => Ok(false),
    }
}

/// Для INSERT/UPDATE ждём `Done`: строка в ответе означала бы не тот SQL.
fn step_done(statement: &mut ffi::Stmt<'_>) -> Result<(), StoreError> {
    match statement.step()? {
        ffi::Step::Done => Ok(()),
        ffi::Step::Row => Err(corrupt("INSERT/UPDATE неожиданно вернул строку")),
    }
}

fn bind_optional_text(
    statement: &mut ffi::Stmt<'_>,
    index: usize,
    value: Option<&str>,
) -> Result<(), StoreError> {
    match value {
        Some(text) => {
            statement.bind_text(index, text)?;
            Ok(())
        }
        None => {
            statement.bind_null(index)?;
            Ok(())
        }
    }
}

/// NOT NULL-колонка: NULL здесь означает повреждённую строку, а не «пустое значение».
fn required_text(statement: &ffi::Stmt<'_>, column: usize) -> Result<String, StoreError> {
    statement
        .column_text(column)?
        .ok_or_else(|| corrupt(format!("NOT NULL колонка {column} содержит NULL")))
}

fn read_message(statement: &ffi::Stmt<'_>) -> Result<MessageRow, StoreError> {
    let edited_timestamp = if statement.column_is_null(5)? {
        None
    } else {
        Some(statement.column_i64(5)?)
    };
    let flags = statement.column_i64(6)?;
    let flags = u64::try_from(flags)
        .map_err(|_| corrupt("messages.flags отрицателен — данные не из этой схемы"))?;
    Ok(MessageRow {
        id: required_text(statement, 0)?,
        channel_id: required_text(statement, 1)?,
        author_id: required_text(statement, 2)?,
        content: required_text(statement, 3)?,
        timestamp: statement.column_i64(4)?,
        edited_timestamp,
        flags,
        deleted: statement.column_i64(7)? != 0,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn core_tables_are_set() {
        assert_eq!(super::CORE_TABLES.len(), 5);
        assert!(super::CORE_TABLES.contains(&"guilds"));
        assert!(super::CORE_TABLES.contains(&"channels"));
    }
}
