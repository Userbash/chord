#![forbid(unsafe_code)]
//! # chord-store
//!
//! Локальное хранилище клиента на базе `rusqlite` (`bundled` SQLite + `backup`) и
//! выделенного потока [`DbWorker`] с очередью команд `mpsc(1024)` и `oneshot`-ответами (`LIB-SIMPLIFY-02`).

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, params};

pub mod cache_reset;
pub mod fts5_search;
pub mod keyring_crypto;
pub mod schema;
pub mod worker;

pub use cache_reset::{
    PreferencesResetReport, PurgeCacheReport, ThumbnailPurgeReport, purge_media_thumbnails,
};

pub use fts5_search::{Fts5MessageRecord, Fts5SearchEngine, Fts5SearchHit};
pub use schema::{
    ChannelRow, GuildRow, MessageRow, ReadStateRow, SCHEMA_VERSION, SCHEMA_VERSION_V2, StoreError,
    account_db_path,
};
pub use worker::{
    DEFAULT_WORKER_CHANNEL_CAPACITY, DbCommand, DbWorker, OneshotReceiver, OneshotSender,
    StoreBridge, oneshot_channel,
};

use schema::{
    MAX_MESSAGES_LIMIT, SQL_MESSAGES_BEFORE, SQL_MESSAGES_HEAD, SQL_SCHEMA_V1, SQL_SCHEMA_V2_EXT,
    SQL_UPSERT_CHANNEL, SQL_UPSERT_GUILD, SQL_UPSERT_KV, SQL_UPSERT_MESSAGE, SQL_UPSERT_READ_STATE,
    corrupt, map_rusqlite_error, validate_no_nul,
};

/// Стартовый набор таблиц ядра данных (по F-DATA-02).
pub const CORE_TABLES: [&str; 5] = ["guilds", "channels", "roles", "users", "emoji"];

/// Соединение с локальной БД на базе `rusqlite::Connection`.
pub struct Store {
    conn: Connection,
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
    /// Открывает (создаёт при отсутствии) файловую БД через `rusqlite`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref();
        let text = path.to_str().ok_or_else(|| {
            StoreError::Open(format!("путь БД не является UTF-8: {}", path.display()))
        })?;
        if text.contains('\0') {
            return Err(StoreError::Open("путь содержит NUL-байт".to_owned()));
        }
        let conn = if text == ":memory:" {
            Connection::open_in_memory()
        } else {
            Connection::open(path)
        }
        .map_err(|err| StoreError::Open(format!("rusqlite open({}): {err}", path.display())))?;

        Ok(Self {
            conn,
            path: path.to_path_buf(),
        })
    }

    /// БД в памяти (тесты, эфемерные кэши).
    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::open(":memory:")
    }

    /// Открывает изолированную БД для конкретного аккаунта `account_id` и накатывает схему v2.
    pub fn open_for_account(
        base_dir: impl AsRef<Path>,
        account_id: &str,
    ) -> Result<Self, StoreError> {
        let db_path = account_db_path(base_dir, account_id)?;
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| StoreError::Open(format!("каталог аккаунта: {err}")))?;
        }
        let mut store = Self::open(db_path)?;
        store.migrate_v2()?;
        Ok(store)
    }

    fn configure_pragmas(&self) -> Result<(), StoreError> {
        self.conn
            .busy_timeout(Duration::from_millis(5_000))
            .map_err(map_rusqlite_error)?;
        self.conn
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;
                 PRAGMA cache_size = -64000;
                 PRAGMA mmap_size = 268435456;
                 PRAGMA temp_store = MEMORY;",
            )
            .map_err(map_rusqlite_error)
    }

    /// Создаёт схему v1 и выставляет `user_version = 1`. Идемпотентно.
    pub fn migrate(&mut self) -> Result<(), StoreError> {
        self.configure_pragmas()?;
        let version = self.schema_version()?;
        if version > SCHEMA_VERSION_V2 {
            return Err(StoreError::Migration(format!(
                "user_version {version} новее поддерживаемой {SCHEMA_VERSION_V2}: обновите клиент"
            )));
        }
        if version >= SCHEMA_VERSION {
            return Ok(());
        }
        self.run_in_immediate_tx(|conn| {
            conn.execute_batch(SQL_SCHEMA_V1)
                .map_err(map_rusqlite_error)?;
            conn.execute_batch("PRAGMA user_version = 1")
                .map_err(map_rusqlite_error)
        })
    }

    /// Транзакционно мигрирует БД до схемы v2 (`guilds`, `channels`, `messages`, `read_states`, `messages_fts`).
    pub fn migrate_v2(&mut self) -> Result<(), StoreError> {
        self.migrate_to(SCHEMA_VERSION_V2).map(|_| ())
    }

    /// Транзакционно мигрирует схему до `target_version` (`1` или `2`) с откатом при сбое питания/ошибке.
    pub fn migrate_to(&mut self, target_version: u32) -> Result<u32, StoreError> {
        if target_version == 0 || target_version > SCHEMA_VERSION_V2 {
            return Err(StoreError::Migration(format!(
                "целевая версия схемы {target_version} не поддерживается"
            )));
        }
        self.configure_pragmas()?;
        let mut current = self.schema_version()?;
        if current > SCHEMA_VERSION_V2 {
            return Err(StoreError::Migration(format!(
                "user_version {current} новее поддерживаемой {SCHEMA_VERSION_V2}"
            )));
        }
        if current < SCHEMA_VERSION && target_version >= SCHEMA_VERSION {
            self.migrate()?;
            current = SCHEMA_VERSION;
        }
        if current < SCHEMA_VERSION_V2 && target_version >= SCHEMA_VERSION_V2 {
            self.run_in_immediate_tx(|conn| {
                conn.execute_batch(SQL_SCHEMA_V2_EXT)
                    .map_err(map_rusqlite_error)?;
                conn.execute_batch(
                    "DELETE FROM messages_fts;
                     INSERT INTO messages_fts (message_id, channel_id, author_id, content)
                     SELECT id, channel_id, author_id, content FROM messages WHERE deleted = 0;
                     PRAGMA user_version = 2;",
                )
                .map_err(map_rusqlite_error)
            })?;
            current = SCHEMA_VERSION_V2;
        }
        Ok(current)
    }

    /// Имитирует обрыв питания/сбой посреди транзакции миграции на `target_version` (для теста `203.1`),
    /// проверяя атомарный `ROLLBACK` и сохранение предыдущей `PRAGMA user_version`.
    pub fn simulate_interrupted_migration_for_test(
        &mut self,
        target_version: u32,
    ) -> Result<(), StoreError> {
        self.configure_pragmas()?;
        self.run_in_immediate_tx(|conn| {
            conn.execute_batch(SQL_SCHEMA_V2_EXT)
                .map_err(map_rusqlite_error)?;
            Err(StoreError::Migration(format!(
                "simulated power loss before committing user_version = {target_version}"
            )))
        })
    }

    fn run_in_immediate_tx<F>(&self, op: F) -> Result<(), StoreError>
    where
        F: FnOnce(&Connection) -> Result<(), StoreError>,
    {
        self.conn
            .execute_batch("BEGIN IMMEDIATE")
            .map_err(map_rusqlite_error)?;
        if let Err(error) = op(&self.conn) {
            if let Err(rb_err) = self.conn.execute_batch("ROLLBACK") {
                return Err(StoreError::Migration(format!(
                    "{error}; откат транзакции не удался: {rb_err}"
                )));
            }
            return Err(error);
        }
        self.conn
            .execute_batch("COMMIT")
            .map_err(map_rusqlite_error)
    }

    /// Текущая версия схемы из `PRAGMA user_version` (0 — схема ещё не создана).
    pub fn schema_version(&self) -> Result<u32, StoreError> {
        let version: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(map_rusqlite_error)?;
        u32::try_from(version)
            .map_err(|_| StoreError::Migration(format!("user_version {version} вне u32")))
    }

    /// Вставляет или обновляет гильдию в таблице `guilds` (схема v2).
    pub fn put_guild(&mut self, row: &GuildRow) -> Result<(), StoreError> {
        validate_no_nul(&row.id)?;
        validate_no_nul(&row.name)?;
        if let Some(ref icon) = row.icon {
            validate_no_nul(icon)?;
        }
        if let Some(ref owner) = row.owner_id {
            validate_no_nul(owner)?;
        }
        self.conn
            .execute(
                SQL_UPSERT_GUILD,
                params![row.id, row.name, row.icon, row.owner_id],
            )
            .map_err(map_rusqlite_error)?;
        Ok(())
    }

    /// Читает гильдию по `id`; `None`, если строки нет.
    pub fn guild(&self, id: &str) -> Result<Option<GuildRow>, StoreError> {
        validate_no_nul(id)?;
        self.conn
            .query_row(
                "SELECT id, name, icon, owner_id FROM guilds WHERE id = ?1",
                params![id],
                |row| {
                    Ok(GuildRow {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        icon: row.get(2)?,
                        owner_id: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(map_rusqlite_error)
    }

    /// Возвращает все гильдии, отсортированные по `id`.
    pub fn guilds(&self) -> Result<Vec<GuildRow>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, icon, owner_id FROM guilds ORDER BY id ASC")
            .map_err(map_rusqlite_error)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(GuildRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    icon: row.get(2)?,
                    owner_id: row.get(3)?,
                })
            })
            .map_err(map_rusqlite_error)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(map_rusqlite_error)?);
        }
        Ok(out)
    }

    /// Вставляет канал или обновляет существующий по `id`.
    pub fn put_channel(&mut self, row: &ChannelRow) -> Result<(), StoreError> {
        validate_no_nul(&row.id)?;
        if let Some(ref gid) = row.guild_id {
            validate_no_nul(gid)?;
        }
        if let Some(ref name) = row.name {
            validate_no_nul(name)?;
        }
        self.conn
            .execute(
                SQL_UPSERT_CHANNEL,
                params![
                    row.id,
                    row.guild_id,
                    row.name,
                    i64::from(row.kind),
                    row.position
                ],
            )
            .map_err(map_rusqlite_error)?;
        Ok(())
    }

    /// Читает канал по `id`; `None`, если строки нет.
    pub fn channel(&self, id: &str) -> Result<Option<ChannelRow>, StoreError> {
        validate_no_nul(id)?;
        let raw = self
            .conn
            .query_row(
                "SELECT id, guild_id, name, kind, position FROM channels WHERE id = ?1",
                params![id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(map_rusqlite_error)?;
        let Some((cid, guild_id, name, kind_i64, position)) = raw else {
            return Ok(None);
        };
        let kind = u8::try_from(kind_i64).map_err(|_| corrupt("channels.kind вне диапазона u8"))?;
        Ok(Some(ChannelRow {
            id: cid,
            guild_id,
            name,
            kind,
            position,
        }))
    }

    /// Вставляет сообщение или обновляет существующее по `id` (канал должен существовать).
    pub fn put_message(&mut self, row: &MessageRow) -> Result<(), StoreError> {
        validate_no_nul(&row.id)?;
        validate_no_nul(&row.channel_id)?;
        validate_no_nul(&row.author_id)?;
        validate_no_nul(&row.content)?;
        let flags = i64::try_from(row.flags).map_err(|_| {
            StoreError::Constraint("messages.flags не влезает в SQLite INTEGER (i64)".to_owned())
        })?;
        self.conn
            .execute(
                SQL_UPSERT_MESSAGE,
                params![
                    row.id,
                    row.channel_id,
                    row.author_id,
                    row.content,
                    row.timestamp,
                    row.edited_timestamp,
                    flags,
                    i64::from(row.deleted)
                ],
            )
            .map_err(map_rusqlite_error)?;
        self.sync_message_to_fts_if_v2(row)
    }

    fn sync_message_to_fts_if_v2(&self, row: &MessageRow) -> Result<(), StoreError> {
        if self.schema_version()? < SCHEMA_VERSION_V2 {
            return Ok(());
        }
        self.conn
            .execute(fts5_search::SQL_DELETE_FTS_BY_ID, params![row.id])
            .map_err(map_rusqlite_error)?;
        if !row.deleted {
            self.conn
                .execute(
                    fts5_search::SQL_INSERT_FTS,
                    params![row.id, row.channel_id, row.author_id, row.content],
                )
                .map_err(map_rusqlite_error)?;
        }
        Ok(())
    }

    /// История канала: `ORDER BY timestamp DESC, id DESC`.
    pub fn messages(
        &self,
        channel_id: &str,
        before: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MessageRow>, StoreError> {
        validate_no_nul(channel_id)?;
        let limit = limit.min(MAX_MESSAGES_LIMIT);
        if limit == 0 {
            return Ok(Vec::new());
        }
        let limit_i64 = i64::try_from(limit).map_err(|_| corrupt("limit не влезает в i64"))?;

        let cursor = match before {
            None => None,
            Some(id) => {
                validate_no_nul(id)?;
                match self.message_timestamp(id)? {
                    Some(ts) => Some((ts, id)),
                    None => return Ok(Vec::new()),
                }
            }
        };
        self.query_messages_page(channel_id, cursor, limit_i64)
    }

    fn query_messages_page(
        &self,
        channel_id: &str,
        cursor: Option<(i64, &str)>,
        limit_i64: i64,
    ) -> Result<Vec<MessageRow>, StoreError> {
        match cursor {
            None => {
                let mut stmt = self
                    .conn
                    .prepare(SQL_MESSAGES_HEAD)
                    .map_err(map_rusqlite_error)?;
                let rows = stmt
                    .query_map(params![channel_id, limit_i64], read_message_row)
                    .map_err(map_rusqlite_error)?;
                collect_message_rows(rows)
            }
            Some((timestamp, id)) => {
                let mut stmt = self
                    .conn
                    .prepare(SQL_MESSAGES_BEFORE)
                    .map_err(map_rusqlite_error)?;
                let rows = stmt
                    .query_map(
                        params![channel_id, timestamp, id, limit_i64],
                        read_message_row,
                    )
                    .map_err(map_rusqlite_error)?;
                collect_message_rows(rows)
            }
        }
    }

    fn message_timestamp(&self, id: &str) -> Result<Option<i64>, StoreError> {
        self.conn
            .query_row(
                "SELECT timestamp FROM messages WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_rusqlite_error)
    }

    /// Soft delete: помечает строку `deleted = 1`, физически не удаляя, и убирает из `messages_fts`.
    pub fn delete_message(&mut self, id: &str) -> Result<bool, StoreError> {
        validate_no_nul(id)?;
        let changes = self
            .conn
            .execute("UPDATE messages SET deleted = 1 WHERE id = ?1", params![id])
            .map_err(map_rusqlite_error)?;
        if changes > 0 && self.schema_version()? >= SCHEMA_VERSION_V2 {
            self.conn
                .execute(fts5_search::SQL_DELETE_FTS_BY_ID, params![id])
                .map_err(map_rusqlite_error)?;
        }
        Ok(changes > 0)
    }

    /// Вставляет или обновляет состояние прочтения канала (`read_states`, схема v2).
    pub fn put_read_state(&mut self, row: &ReadStateRow) -> Result<(), StoreError> {
        validate_no_nul(&row.channel_id)?;
        if let Some(ref mid) = row.last_message_id {
            validate_no_nul(mid)?;
        }
        self.conn
            .execute(
                SQL_UPSERT_READ_STATE,
                params![
                    row.channel_id,
                    row.last_message_id,
                    row.mention_count,
                    row.last_pin_timestamp
                ],
            )
            .map_err(map_rusqlite_error)?;
        Ok(())
    }

    /// Читает состояние прочтения канала по `channel_id`.
    pub fn read_state(&self, channel_id: &str) -> Result<Option<ReadStateRow>, StoreError> {
        validate_no_nul(channel_id)?;
        self.conn
            .query_row(
                "SELECT channel_id, last_message_id, mention_count, last_pin_timestamp
                 FROM read_states WHERE channel_id = ?1",
                params![channel_id],
                |row| {
                    Ok(ReadStateRow {
                        channel_id: row.get(0)?,
                        last_message_id: row.get(1)?,
                        mention_count: row.get(2)?,
                        last_pin_timestamp: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(map_rusqlite_error)
    }

    /// Возвращает все состояния прочтения каналов (`read_states`), отсортированные по `channel_id`.
    pub fn read_states(&self) -> Result<Vec<ReadStateRow>, StoreError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT channel_id, last_message_id, mention_count, last_pin_timestamp
                 FROM read_states ORDER BY channel_id ASC",
            )
            .map_err(map_rusqlite_error)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(ReadStateRow {
                    channel_id: row.get(0)?,
                    last_message_id: row.get(1)?,
                    mention_count: row.get(2)?,
                    last_pin_timestamp: row.get(3)?,
                })
            })
            .map_err(map_rusqlite_error)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(map_rusqlite_error)?);
        }
        Ok(out)
    }

    /// Пишет значение в служебный key-value (upsert по ключу).
    pub fn kv_set(&mut self, key: &str, value: &str) -> Result<(), StoreError> {
        validate_no_nul(key)?;
        validate_no_nul(value)?;
        self.conn
            .execute(SQL_UPSERT_KV, params![key, value])
            .map_err(map_rusqlite_error)?;
        Ok(())
    }

    /// Читает значение из key-value; `None`, если ключа нет.
    pub fn kv_get(&self, key: &str) -> Result<Option<String>, StoreError> {
        validate_no_nul(key)?;
        self.conn
            .query_row("SELECT value FROM kv WHERE key = ?1", params![key], |row| {
                row.get(0)
            })
            .optional()
            .map_err(map_rusqlite_error)
    }

    /// Индексирует записи в виртуальную таблицу `messages_fts` (требует схему v2).
    pub fn index_fts_messages(&mut self, records: &[Fts5MessageRecord]) -> Result<(), StoreError> {
        fts5_search::index_records_on_conn(&self.conn, records)
    }

    /// Полнотекстовый поиск сообщений через FTS5 `messages_fts` с ранжированием BM25 и сниппетами.
    pub fn search_messages_fts(
        &self,
        query: &str,
        channel_id: Option<&str>,
        author_id: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Fts5SearchHit>, StoreError> {
        fts5_search::search_on_conn(&self.conn, query, channel_id, author_id, limit, offset)
    }

    /// Создаёт консистентную резервную копию БД в `dst_path` через `rusqlite::backup::Backup`.
    pub fn backup_to(&self, dst_path: impl AsRef<Path>) -> Result<(), StoreError> {
        let mut dst = Connection::open(dst_path.as_ref()).map_err(|err| {
            StoreError::Open(format!(
                "backup destination {}: {err}",
                dst_path.as_ref().display()
            ))
        })?;
        let backup =
            rusqlite::backup::Backup::new(&self.conn, &mut dst).map_err(map_rusqlite_error)?;
        backup
            .run_to_completion(128, Duration::from_millis(10), None)
            .map_err(map_rusqlite_error)
    }
}

struct RawMessageColumns {
    id: String,
    channel_id: String,
    author_id: String,
    content: String,
    timestamp: i64,
    edited_timestamp: Option<i64>,
    flags_i64: i64,
    deleted: i64,
}

fn read_message_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawMessageColumns> {
    Ok(RawMessageColumns {
        id: row.get(0)?,
        channel_id: row.get(1)?,
        author_id: row.get(2)?,
        content: row.get(3)?,
        timestamp: row.get(4)?,
        edited_timestamp: row.get(5)?,
        flags_i64: row.get(6)?,
        deleted: row.get(7)?,
    })
}

fn collect_message_rows<F>(rows: rusqlite::MappedRows<'_, F>) -> Result<Vec<MessageRow>, StoreError>
where
    F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<RawMessageColumns>,
{
    let mut out = Vec::new();
    for item in rows {
        let raw = item.map_err(map_rusqlite_error)?;
        let flags = u64::try_from(raw.flags_i64)
            .map_err(|_| corrupt("messages.flags отрицателен — данные не из этой схемы"))?;
        out.push(MessageRow {
            id: raw.id,
            channel_id: raw.channel_id,
            author_id: raw.author_id,
            content: raw.content,
            timestamp: raw.timestamp,
            edited_timestamp: raw.edited_timestamp,
            flags,
            deleted: raw.deleted != 0,
        });
    }
    Ok(out)
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
