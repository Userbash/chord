#![forbid(unsafe_code)]
//! SQLite schema definitions, SQL statements, row DTOs, and error types for `chord-store` (`LIB-SIMPLIFY-02`).

use std::fmt;
use std::path::{Path, PathBuf};

/// Стартовый набор таблиц ядра данных (по F-DATA-02).
pub const CORE_TABLES: [&str; 5] = ["guilds", "channels", "roles", "users", "emoji"];

/// Базовая версия схемы хранилища v1 (`PRAGMA user_version = 1`).
pub const SCHEMA_VERSION: u32 = 1;

/// Расширенная версия схемы хранилища v2 (`guilds`, `channels`, `messages`, `read_states`, `messages_fts`).
pub const SCHEMA_VERSION_V2: u32 = 2;

/// Верхняя граница `limit` для [`crate::Store::messages`] (TZ §1).
pub(crate) const MAX_MESSAGES_LIMIT: usize = 1000;

pub(crate) const SQLITE_CORRUPT_CODE: i32 = 11;
pub(crate) const SQLITE_CONSTRAINT_CODE: i32 = 19;
pub(crate) const SQLITE_MISUSE_CODE: i32 = 21;

pub(crate) const SQL_SCHEMA_V1: &str = "
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

pub(crate) const SQL_SCHEMA_V2_EXT: &str = "
CREATE TABLE IF NOT EXISTS guilds (
    id       TEXT PRIMARY KEY NOT NULL,
    name     TEXT NOT NULL,
    icon     TEXT,
    owner_id TEXT
);
CREATE TABLE IF NOT EXISTS read_states (
    channel_id         TEXT PRIMARY KEY NOT NULL,
    last_message_id    TEXT,
    mention_count      INTEGER NOT NULL DEFAULT 0,
    last_pin_timestamp INTEGER
);
CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
    message_id UNINDEXED,
    channel_id UNINDEXED,
    author_id UNINDEXED,
    content,
    tokenize = 'unicode61'
);
";

pub(crate) const SQL_UPSERT_GUILD: &str = "
INSERT INTO guilds (id, name, icon, owner_id)
VALUES (?1, ?2, ?3, ?4)
ON CONFLICT(id) DO UPDATE SET
    name = excluded.name,
    icon = excluded.icon,
    owner_id = excluded.owner_id
";

pub(crate) const SQL_UPSERT_CHANNEL: &str = "
INSERT INTO channels (id, guild_id, name, kind, position)
VALUES (?1, ?2, ?3, ?4, ?5)
ON CONFLICT(id) DO UPDATE SET
    guild_id = excluded.guild_id,
    name = excluded.name,
    kind = excluded.kind,
    position = excluded.position
";

pub(crate) const SQL_UPSERT_MESSAGE: &str = "
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

pub(crate) const SQL_UPSERT_READ_STATE: &str = "
INSERT INTO read_states (channel_id, last_message_id, mention_count, last_pin_timestamp)
VALUES (?1, ?2, ?3, ?4)
ON CONFLICT(channel_id) DO UPDATE SET
    last_message_id = excluded.last_message_id,
    mention_count = excluded.mention_count,
    last_pin_timestamp = excluded.last_pin_timestamp
";

pub(crate) const SQL_MESSAGES_HEAD: &str = "
SELECT id, channel_id, author_id, content, timestamp, edited_timestamp, flags, deleted
FROM messages
WHERE channel_id = ?1 AND deleted = 0
ORDER BY timestamp DESC, id DESC
LIMIT ?2
";

pub(crate) const SQL_MESSAGES_BEFORE: &str = "
SELECT id, channel_id, author_id, content, timestamp, edited_timestamp, flags, deleted
FROM messages
WHERE channel_id = ?1 AND deleted = 0 AND (timestamp < ?2 OR (timestamp = ?2 AND id < ?3))
ORDER BY timestamp DESC, id DESC
LIMIT ?4
";

pub(crate) const SQL_UPSERT_KV: &str = "
INSERT INTO kv (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value
";

/// Строка сервера/гильдии (схема v2, `LIB-SIMPLIFY-02`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildRow {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub owner_id: Option<String>,
}

/// Строка состояния прочтения канала (`read_states`, схема v2, `LIB-SIMPLIFY-02`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadStateRow {
    pub channel_id: String,
    pub last_message_id: Option<String>,
    pub mention_count: i64,
    pub last_pin_timestamp: Option<i64>,
}

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

pub(crate) fn map_rusqlite_error(error: rusqlite::Error) -> StoreError {
    match error {
        rusqlite::Error::SqliteFailure(ffi_err, msg) => {
            let text = msg.unwrap_or_else(|| ffi_err.to_string());
            if ffi_err.code == rusqlite::ErrorCode::ConstraintViolation
                || (ffi_err.extended_code & 0xff) == SQLITE_CONSTRAINT_CODE
            {
                StoreError::Constraint(text)
            } else {
                StoreError::Sqlite {
                    code: ffi_err.extended_code,
                    message: text,
                }
            }
        }
        rusqlite::Error::NulError(_) => StoreError::Sqlite {
            code: SQLITE_MISUSE_CODE,
            message: "значение содержит NUL-байт".to_owned(),
        },
        other => StoreError::Sqlite {
            code: SQLITE_CORRUPT_CODE,
            message: other.to_string(),
        },
    }
}

pub(crate) fn validate_no_nul(value: &str) -> Result<(), StoreError> {
    if value.contains('\0') {
        return Err(StoreError::Sqlite {
            code: SQLITE_MISUSE_CODE,
            message: "значение содержит NUL-байт".to_owned(),
        });
    }
    Ok(())
}

pub(crate) fn corrupt(message: impl Into<String>) -> StoreError {
    StoreError::Sqlite {
        code: SQLITE_CORRUPT_CODE,
        message: message.into(),
    }
}

/// Вычисляет изолированный путь к файлу БД для конкретного `account_id` внутри `base_dir`.
pub fn account_db_path(
    base_dir: impl AsRef<Path>,
    account_id: &str,
) -> Result<PathBuf, StoreError> {
    let trimmed = account_id.trim();
    if trimmed.is_empty()
        || !trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err(StoreError::Open(format!(
            "недопустимый идентификатор аккаунта: {account_id:?}"
        )));
    }
    Ok(base_dir
        .as_ref()
        .join("accounts")
        .join(trimmed)
        .join("chord_store.sqlite3"))
}
