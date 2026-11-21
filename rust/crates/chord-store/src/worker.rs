#![forbid(unsafe_code)]
//! Dedicated `std::thread` DB Worker and transparent `StoreBridge` (`LIB-SIMPLIFY-02`).
//!
//! Architecture:
//! `Async Tokio / UI Caller -> bounded mpsc::SyncSender(1024) -> Dedicated std::thread DB Worker -> rusqlite`
//! Each command carries a lightweight oneshot reply channel so callers receive typed `Result<T, StoreError>`
//! without sharing or locking `rusqlite::Connection` across threads.

use std::path::Path;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::{self, JoinHandle, ThreadId};

use crate::fts5_search::{Fts5MessageRecord, Fts5SearchHit};
use crate::{ChannelRow, GuildRow, MessageRow, ReadStateRow, Store, StoreError};

/// Bounded command queue capacity for [`StoreBridge`] -> [`DbWorker`] (`mpsc(1024)`).
pub const DEFAULT_WORKER_CHANNEL_CAPACITY: usize = 1024;

/// Sender half of a single-value oneshot response channel from [`DbWorker`].
#[derive(Debug)]
pub struct OneshotSender<T> {
    tx: SyncSender<T>,
}

impl<T> OneshotSender<T> {
    /// Sends a single reply value to the waiting caller.
    pub fn send(self, value: T) -> Result<(), T> {
        self.tx.send(value).map_err(|err| err.0)
    }
}

/// Receiver half of a single-value oneshot response channel from [`DbWorker`].
#[derive(Debug)]
pub struct OneshotReceiver<T> {
    rx: Receiver<T>,
}

impl<T> OneshotReceiver<T> {
    /// Blocks until the DB Worker sends the reply or drops the channel.
    pub fn recv(self) -> Result<T, StoreError> {
        self.rx.recv().map_err(|_| StoreError::Sqlite {
            code: 1,
            message: "DB-Worker закрыл канал ответа oneshot".to_owned(),
        })
    }
}

/// Creates a bounded-1 oneshot reply channel (`(OneshotSender<T>, OneshotReceiver<T>)`).
#[must_use]
pub fn oneshot_channel<T>() -> (OneshotSender<T>, OneshotReceiver<T>) {
    let (tx, rx) = mpsc::sync_channel(1);
    (OneshotSender { tx }, OneshotReceiver { rx })
}

/// Typed commands processed sequentially by the dedicated [`DbWorker`] thread.
#[derive(Debug)]
pub enum DbCommand {
    /// Returns the OS/Rust [`ThreadId`] of the dedicated DB Worker thread.
    WorkerThreadId {
        reply: OneshotSender<Result<ThreadId, StoreError>>,
    },
    /// Reads current `PRAGMA user_version`.
    SchemaVersion {
        reply: OneshotSender<Result<u32, StoreError>>,
    },
    /// Runs transactional schema migration up to `target_version`.
    MigrateTo {
        target_version: u32,
        reply: OneshotSender<Result<u32, StoreError>>,
    },
    /// Upserts a guild row into `guilds`.
    PutGuild {
        row: GuildRow,
        reply: OneshotSender<Result<(), StoreError>>,
    },
    /// Fetches a guild row by `id`.
    GetGuild {
        id: String,
        reply: OneshotSender<Result<Option<GuildRow>, StoreError>>,
    },
    /// Lists all guilds ordered by `id`.
    ListGuilds {
        reply: OneshotSender<Result<Vec<GuildRow>, StoreError>>,
    },
    /// Upserts a channel row into `channels`.
    PutChannel {
        row: ChannelRow,
        reply: OneshotSender<Result<(), StoreError>>,
    },
    /// Fetches a channel row by `id`.
    GetChannel {
        id: String,
        reply: OneshotSender<Result<Option<ChannelRow>, StoreError>>,
    },
    /// Upserts a message row into `messages` (and `messages_fts` in v2).
    PutMessage {
        row: MessageRow,
        reply: OneshotSender<Result<(), StoreError>>,
    },
    /// Queries paginated channel message history (`ORDER BY timestamp DESC, id DESC`).
    GetMessages {
        channel_id: String,
        before: Option<String>,
        limit: usize,
        reply: OneshotSender<Result<Vec<MessageRow>, StoreError>>,
    },
    /// Soft-deletes a message (`deleted = 1`) and removes it from `messages_fts`.
    DeleteMessage {
        id: String,
        reply: OneshotSender<Result<bool, StoreError>>,
    },
    /// Upserts a channel read state into `read_states`.
    PutReadState {
        row: ReadStateRow,
        reply: OneshotSender<Result<(), StoreError>>,
    },
    /// Fetches a channel read state by `channel_id`.
    GetReadState {
        channel_id: String,
        reply: OneshotSender<Result<Option<ReadStateRow>, StoreError>>,
    },
    /// Lists all channel read states ordered by `channel_id`.
    ListReadStates {
        reply: OneshotSender<Result<Vec<ReadStateRow>, StoreError>>,
    },
    /// Upserts a key-value pair into `kv`.
    KvSet {
        key: String,
        value: String,
        reply: OneshotSender<Result<(), StoreError>>,
    },
    /// Reads a value from `kv` by `key`.
    KvGet {
        key: String,
        reply: OneshotSender<Result<Option<String>, StoreError>>,
    },
    /// Bulk-indexes records into `messages_fts`.
    IndexFtsMessages {
        records: Vec<Fts5MessageRecord>,
        reply: OneshotSender<Result<(), StoreError>>,
    },
    /// Executes an FTS5 BM25 search with highlighted snippets on `messages_fts`.
    SearchMessagesFts {
        query: String,
        channel_id: Option<String>,
        author_id: Option<String>,
        limit: usize,
        offset: usize,
        reply: OneshotSender<Result<Vec<Fts5SearchHit>, StoreError>>,
    },
    /// Performs an online SQLite backup to `dst_path` via `rusqlite::backup`.
    BackupTo {
        dst_path: std::path::PathBuf,
        reply: OneshotSender<Result<(), StoreError>>,
    },
    /// Flushes and terminates the worker loop cleanly.
    Shutdown {
        reply: OneshotSender<Result<(), StoreError>>,
    },
}

/// Cloneable thread-safe bridge (`StoreBridge`) that sends [`DbCommand`]s over a bounded
/// `mpsc(1024)` channel to the dedicated [`DbWorker`] thread.
#[derive(Clone, Debug)]
pub struct StoreBridge {
    tx: SyncSender<DbCommand>,
}

impl StoreBridge {
    /// Sends a raw [`DbCommand`] to the dedicated DB-Worker queue.
    pub fn send_command(&self, cmd: DbCommand) -> Result<(), StoreError> {
        self.tx.send(cmd).map_err(|_| StoreError::Sqlite {
            code: 1,
            message: "очередь DB-Worker закрыта".to_owned(),
        })
    }

    fn request<T>(
        &self,
        build: impl FnOnce(OneshotSender<Result<T, StoreError>>) -> DbCommand,
    ) -> Result<T, StoreError> {
        let (tx, rx) = oneshot_channel();
        self.send_command(build(tx))?;
        rx.recv()?
    }

    /// Returns the [`ThreadId`] of the dedicated DB-Worker thread.
    pub fn worker_thread_id(&self) -> Result<ThreadId, StoreError> {
        self.request(|reply| DbCommand::WorkerThreadId { reply })
    }

    /// Returns current `PRAGMA user_version` from the DB-Worker.
    pub fn schema_version(&self) -> Result<u32, StoreError> {
        self.request(|reply| DbCommand::SchemaVersion { reply })
    }

    /// Runs transactional schema migration up to `target_version`.
    pub fn migrate_to(&self, target_version: u32) -> Result<u32, StoreError> {
        self.request(|reply| DbCommand::MigrateTo {
            target_version,
            reply,
        })
    }

    /// Upserts a [`GuildRow`] via the DB-Worker.
    pub fn put_guild(&self, row: GuildRow) -> Result<(), StoreError> {
        self.request(|reply| DbCommand::PutGuild { row, reply })
    }

    /// Fetches a [`GuildRow`] by `id` via the DB-Worker.
    pub fn guild(&self, id: impl Into<String>) -> Result<Option<GuildRow>, StoreError> {
        let id = id.into();
        self.request(|reply| DbCommand::GetGuild { id, reply })
    }

    /// Lists all [`GuildRow`]s via the DB-Worker.
    pub fn guilds(&self) -> Result<Vec<GuildRow>, StoreError> {
        self.request(|reply| DbCommand::ListGuilds { reply })
    }

    /// Upserts a [`ChannelRow`] via the DB-Worker.
    pub fn put_channel(&self, row: ChannelRow) -> Result<(), StoreError> {
        self.request(|reply| DbCommand::PutChannel { row, reply })
    }

    /// Fetches a [`ChannelRow`] by `id` via the DB-Worker.
    pub fn channel(&self, id: impl Into<String>) -> Result<Option<ChannelRow>, StoreError> {
        let id = id.into();
        self.request(|reply| DbCommand::GetChannel { id, reply })
    }

    /// Upserts a [`MessageRow`] via the DB-Worker.
    pub fn put_message(&self, row: MessageRow) -> Result<(), StoreError> {
        self.request(|reply| DbCommand::PutMessage { row, reply })
    }

    /// Queries channel history (`ORDER BY timestamp DESC, id DESC`) via the DB-Worker.
    pub fn messages(
        &self,
        channel_id: impl Into<String>,
        before: Option<String>,
        limit: usize,
    ) -> Result<Vec<MessageRow>, StoreError> {
        let channel_id = channel_id.into();
        self.request(|reply| DbCommand::GetMessages {
            channel_id,
            before,
            limit,
            reply,
        })
    }

    /// Soft-deletes a message by `id` via the DB-Worker.
    pub fn delete_message(&self, id: impl Into<String>) -> Result<bool, StoreError> {
        let id = id.into();
        self.request(|reply| DbCommand::DeleteMessage { id, reply })
    }

    /// Upserts a [`ReadStateRow`] via the DB-Worker.
    pub fn put_read_state(&self, row: ReadStateRow) -> Result<(), StoreError> {
        self.request(|reply| DbCommand::PutReadState { row, reply })
    }

    /// Fetches a [`ReadStateRow`] by `channel_id` via the DB-Worker.
    pub fn read_state(
        &self,
        channel_id: impl Into<String>,
    ) -> Result<Option<ReadStateRow>, StoreError> {
        let channel_id = channel_id.into();
        self.request(|reply| DbCommand::GetReadState { channel_id, reply })
    }

    /// Lists all [`ReadStateRow`]s via the DB-Worker.
    pub fn read_states(&self) -> Result<Vec<ReadStateRow>, StoreError> {
        self.request(|reply| DbCommand::ListReadStates { reply })
    }

    /// Sets a key-value entry in `kv` via the DB-Worker.
    pub fn kv_set(
        &self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<(), StoreError> {
        let key = key.into();
        let value = value.into();
        self.request(|reply| DbCommand::KvSet { key, value, reply })
    }

    /// Reads a key-value entry from `kv` via the DB-Worker.
    pub fn kv_get(&self, key: impl Into<String>) -> Result<Option<String>, StoreError> {
        let key = key.into();
        self.request(|reply| DbCommand::KvGet { key, reply })
    }

    /// Bulk-indexes FTS5 records into `messages_fts` via the DB-Worker.
    pub fn index_fts_messages(&self, records: Vec<Fts5MessageRecord>) -> Result<(), StoreError> {
        self.request(|reply| DbCommand::IndexFtsMessages { records, reply })
    }

    /// Searches `messages_fts` via the DB-Worker.
    pub fn search_messages_fts(
        &self,
        query: impl Into<String>,
        channel_id: Option<&str>,
        author_id: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Fts5SearchHit>, StoreError> {
        let query = query.into();
        let channel_id = channel_id.map(str::to_owned);
        let author_id = author_id.map(str::to_owned);
        self.request(|reply| DbCommand::SearchMessagesFts {
            query,
            channel_id,
            author_id,
            limit,
            offset,
            reply,
        })
    }

    /// Backs up the database to `dst_path` via the DB-Worker.
    pub fn backup_to(&self, dst_path: impl AsRef<Path>) -> Result<(), StoreError> {
        let dst_path = dst_path.as_ref().to_path_buf();
        self.request(|reply| DbCommand::BackupTo { dst_path, reply })
    }
}

/// Dedicated `std::thread` owner of [`Store`] (`rusqlite::Connection`), consuming [`DbCommand`]s
/// from a bounded `mpsc(1024)` queue.
pub struct DbWorker {
    bridge: StoreBridge,
    join_handle: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for DbWorker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DbWorker")
            .field("active", &self.join_handle.is_some())
            .finish_non_exhaustive()
    }
}

impl DbWorker {
    /// Spawns a dedicated `std::thread` DB Worker opening `path` and migrating to schema v2.
    pub fn spawn(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        Self::spawn_with_capacity(path, DEFAULT_WORKER_CHANNEL_CAPACITY)
    }

    /// Spawns a dedicated `std::thread` DB Worker backed by an in-memory database (`:memory:`).
    pub fn spawn_in_memory() -> Result<Self, StoreError> {
        Self::spawn(":memory:")
    }

    /// Spawns a dedicated `std::thread` DB Worker for a specific `account_id` under `base_dir`.
    pub fn spawn_for_account(
        base_dir: impl AsRef<Path>,
        account_id: &str,
    ) -> Result<Self, StoreError> {
        let db_path = crate::account_db_path(base_dir, account_id)?;
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| StoreError::Open(format!("create account dir: {err}")))?;
        }
        Self::spawn(db_path)
    }

    /// Spawns a dedicated `std::thread` DB Worker with an explicit bounded channel capacity.
    pub fn spawn_with_capacity(
        path: impl AsRef<Path>,
        capacity: usize,
    ) -> Result<Self, StoreError> {
        let db_path = path.as_ref().to_path_buf();
        let cap = capacity.max(1);
        let (tx, rx) = mpsc::sync_channel::<DbCommand>(cap);
        let (init_tx, init_rx) = oneshot_channel::<Result<(), StoreError>>();

        let handle = thread::Builder::new()
            .name("chord-store-db-worker".to_owned())
            .spawn(move || run_worker_thread(db_path, rx, init_tx))
            .map_err(|err| StoreError::Open(format!("spawn DB-Worker thread: {err}")))?;

        match init_rx.recv()? {
            Ok(()) => Ok(Self {
                bridge: StoreBridge { tx },
                join_handle: Some(handle),
            }),
            Err(err) => {
                let _ = handle.join();
                Err(err)
            }
        }
    }

    /// Returns a cloneable [`StoreBridge`] handle for sending commands to this worker.
    #[must_use]
    pub fn bridge(&self) -> StoreBridge {
        self.bridge.clone()
    }

    /// Signals the worker thread to shut down cleanly and waits for it to exit.
    pub fn shutdown(mut self) -> Result<(), StoreError> {
        self.shutdown_inner()
    }

    fn shutdown_inner(&mut self) -> Result<(), StoreError> {
        let Some(handle) = self.join_handle.take() else {
            return Ok(());
        };
        let (tx, rx) = oneshot_channel();
        if self
            .bridge
            .send_command(DbCommand::Shutdown { reply: tx })
            .is_ok()
        {
            let _ = rx.recv();
        }
        handle.join().map_err(|_| StoreError::Sqlite {
            code: 1,
            message: "поток DB-Worker завершился с паникой".to_owned(),
        })
    }
}

impl Drop for DbWorker {
    fn drop(&mut self) {
        let _ = self.shutdown_inner();
    }
}

fn run_worker_thread(
    db_path: std::path::PathBuf,
    rx: Receiver<DbCommand>,
    init_tx: OneshotSender<Result<(), StoreError>>,
) {
    let mut store = match Store::open(&db_path).and_then(|mut s| {
        s.migrate_v2()?;
        Ok(s)
    }) {
        Ok(store) => {
            let _ = init_tx.send(Ok(()));
            store
        }
        Err(err) => {
            let _ = init_tx.send(Err(err));
            return;
        }
    };

    while let Ok(command) = rx.recv() {
        let should_stop = handle_command(&mut store, command);
        if should_stop {
            break;
        }
    }
}

fn handle_command(store: &mut Store, command: DbCommand) -> bool {
    match command {
        DbCommand::WorkerThreadId { reply } => {
            let _ = reply.send(Ok(thread::current().id()));
        }
        DbCommand::SchemaVersion { reply } => {
            let _ = reply.send(store.schema_version());
        }
        DbCommand::MigrateTo {
            target_version,
            reply,
        } => {
            let _ = reply.send(store.migrate_to(target_version));
        }
        DbCommand::PutGuild { row, reply } => {
            let _ = reply.send(store.put_guild(&row));
        }
        DbCommand::GetGuild { id, reply } => {
            let _ = reply.send(store.guild(&id));
        }
        DbCommand::ListGuilds { reply } => {
            let _ = reply.send(store.guilds());
        }
        DbCommand::PutChannel { row, reply } => {
            let _ = reply.send(store.put_channel(&row));
        }
        DbCommand::GetChannel { id, reply } => {
            let _ = reply.send(store.channel(&id));
        }
        DbCommand::Shutdown { reply } => {
            let _ = reply.send(Ok(()));
            return true;
        }
        other => handle_data_command(store, other),
    }
    false
}

fn handle_data_command(store: &mut Store, command: DbCommand) {
    match command {
        DbCommand::PutMessage { row, reply } => {
            let _ = reply.send(store.put_message(&row));
        }
        DbCommand::GetMessages {
            channel_id,
            before,
            limit,
            reply,
        } => {
            let _ = reply.send(store.messages(&channel_id, before.as_deref(), limit));
        }
        DbCommand::DeleteMessage { id, reply } => {
            let _ = reply.send(store.delete_message(&id));
        }
        DbCommand::PutReadState { row, reply } => {
            let _ = reply.send(store.put_read_state(&row));
        }
        DbCommand::GetReadState { channel_id, reply } => {
            let _ = reply.send(store.read_state(&channel_id));
        }
        DbCommand::ListReadStates { reply } => {
            let _ = reply.send(store.read_states());
        }
        other => handle_aux_command(store, other),
    }
}

fn handle_aux_command(store: &mut Store, command: DbCommand) {
    match command {
        DbCommand::KvSet { key, value, reply } => {
            let _ = reply.send(store.kv_set(&key, &value));
        }
        DbCommand::KvGet { key, reply } => {
            let _ = reply.send(store.kv_get(&key));
        }
        DbCommand::IndexFtsMessages { records, reply } => {
            let _ = reply.send(store.index_fts_messages(&records));
        }
        DbCommand::SearchMessagesFts {
            query,
            channel_id,
            author_id,
            limit,
            offset,
            reply,
        } => {
            let _ = reply.send(store.search_messages_fts(
                &query,
                channel_id.as_deref(),
                author_id.as_deref(),
                limit,
                offset,
            ));
        }
        DbCommand::BackupTo { dst_path, reply } => {
            let _ = reply.send(store.backup_to(&dst_path));
        }
        _ => {}
    }
}
