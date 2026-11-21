//! Contract tests for `LIB-SIMPLIFY-02`:
//! - `rusqlite` (bundled SQLite + backup) replacing custom unsafe FFI in `chord-store` (`#![forbid(unsafe_code)]`)
//! - Schema v2 tables (`guilds`, `channels`, `messages`, `read_states`, `kv`, `messages_fts` FTS5 virtual table)
//! - Transactional schema migrations (`PRAGMA user_version` v1 -> v2) with power-loss / fault rollback protection
//! - Account-partitioned database cache (`open_for_account`, isolated per account ID)
//! - Dedicated `std::thread` DB Worker (`DbWorker` / `StoreBridge`) over bounded `mpsc(1024)` command queue and `oneshot` reply channels

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use chord_store::fts5_search::Fts5MessageRecord;
use chord_store::{
    ChannelRow, DEFAULT_WORKER_CHANNEL_CAPACITY, DbCommand, DbWorker, GuildRow, MessageRow,
    ReadStateRow, SCHEMA_VERSION, SCHEMA_VERSION_V2, Store, StoreBridge, StoreError,
    account_db_path,
};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(tag: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let serial = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "chord-store-lib-simplify-02-{}-{tag}-{serial}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn db_path(&self) -> PathBuf {
        self.path.join("store_v2.db")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[test]
fn subtask_201_1_and_201_2_schema_v2_tables_guilds_channels_messages_read_states_and_fts5() {
    let mut store = Store::open_in_memory().expect("open in memory");
    assert_eq!(store.schema_version().expect("v0"), 0);

    // Step 1: migrate v1 first, then upgrade to v2
    store.migrate().expect("migrate v1");
    assert_eq!(store.schema_version().expect("v1"), SCHEMA_VERSION);

    store.migrate_v2().expect("migrate v2");
    assert_eq!(store.schema_version().expect("v2"), SCHEMA_VERSION_V2);
    // Idempotent second call
    store.migrate_v2().expect("migrate v2 idempotent");
    assert_eq!(store.schema_version().expect("v2 again"), SCHEMA_VERSION_V2);

    // Guilds CRUD
    let guild = GuildRow {
        id: "g-100".to_owned(),
        name: "Chord Rust Guild".to_owned(),
        icon: Some("icon_hash_123".to_owned()),
        owner_id: Some("u-owner".to_owned()),
    };
    store.put_guild(&guild).expect("put_guild");
    assert_eq!(
        store.guild("g-100").expect("get guild"),
        Some(guild.clone())
    );
    assert_eq!(store.guilds().expect("list guilds"), vec![guild]);

    // Channels CRUD
    let channel = ChannelRow {
        id: "ch-200".to_owned(),
        guild_id: Some("g-100".to_owned()),
        name: Some("general".to_owned()),
        kind: 0,
        position: 1,
    };
    store.put_channel(&channel).expect("put_channel");
    assert_eq!(store.channel("ch-200").expect("get channel"), Some(channel));

    // Messages CRUD + automatic FTS5 synchronization in v2
    let msg1 = MessageRow {
        id: "m-301".to_owned(),
        channel_id: "ch-200".to_owned(),
        author_id: "u-alice".to_owned(),
        content: "Deploying rusqlite bundled storage with dedicated DB-Worker thread".to_owned(),
        timestamp: 1_700_000_100,
        edited_timestamp: None,
        flags: 0,
        deleted: false,
    };
    let msg2 = MessageRow {
        id: "m-302".to_owned(),
        channel_id: "ch-200".to_owned(),
        author_id: "u-bob".to_owned(),
        content: "Checking messages_fts FTS5 BM25 ranking and unicode61 snippets".to_owned(),
        timestamp: 1_700_000_200,
        edited_timestamp: None,
        flags: 0,
        deleted: false,
    };
    store.put_message(&msg1).expect("put msg1");
    store.put_message(&msg2).expect("put msg2");

    // FTS5 search directly on Store
    let hits = store
        .search_messages_fts("rusqlite", Some("ch-200"), None, 10, 0)
        .expect("search_messages_fts");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].message_id, "m-301");
    assert!(hits[0].snippet.contains("<b>rusqlite</b>"));

    // Read states CRUD
    let read_state = ReadStateRow {
        channel_id: "ch-200".to_owned(),
        last_message_id: Some("m-302".to_owned()),
        mention_count: 3,
        last_pin_timestamp: Some(1_700_000_050),
    };
    store.put_read_state(&read_state).expect("put_read_state");
    assert_eq!(
        store.read_state("ch-200").expect("get read_state"),
        Some(read_state.clone())
    );

    let updated_rs = ReadStateRow {
        channel_id: "ch-200".to_owned(),
        last_message_id: Some("m-302".to_owned()),
        mention_count: 0,
        last_pin_timestamp: Some(1_700_000_050),
    };
    store
        .put_read_state(&updated_rs)
        .expect("update read_state");
    assert_eq!(
        store.read_state("ch-200").expect("get updated read_state"),
        Some(updated_rs.clone())
    );
    assert_eq!(
        store.read_states().expect("all read_states"),
        vec![updated_rs]
    );

    // Soft-deleting a message also removes it from messages_fts
    assert!(store.delete_message("m-301").expect("delete m-301"));
    let hits_after_del = store
        .search_messages_fts("rusqlite", Some("ch-200"), None, 10, 0)
        .expect("search after delete");
    assert!(
        hits_after_del.is_empty(),
        "soft-deleted message must not appear in FTS5 search results"
    );
}

#[test]
fn subtask_203_1_transactional_migrations_rollback_on_power_loss_fault_and_preserve_data() {
    let dir = TempDir::new("migration-power-loss");
    let db_path = dir.db_path();

    let mut store = Store::open(&db_path).expect("open db");
    store.migrate().expect("migrate to v1");
    store
        .put_channel(&ChannelRow {
            id: "ch-safe".to_owned(),
            guild_id: None,
            name: Some("safe-channel".to_owned()),
            kind: 0,
            position: 0,
        })
        .expect("put channel in v1");
    store
        .put_message(&MessageRow {
            id: "m-safe".to_owned(),
            channel_id: "ch-safe".to_owned(),
            author_id: "u-1".to_owned(),
            content: "Critical message before simulated power interruption".to_owned(),
            timestamp: 500,
            edited_timestamp: None,
            flags: 0,
            deleted: false,
        })
        .expect("put message in v1");

    // Simulate interrupted migration (power-loss fault mid-transaction before user_version bump)
    let err = store
        .simulate_interrupted_migration_for_test(2)
        .expect_err("interrupted migration must fail and rollback");
    assert!(matches!(err, StoreError::Migration(_)));

    // Verify user_version remained at 1 and v1 data is 100% intact
    assert_eq!(store.schema_version().expect("still v1"), 1);
    let msgs = store
        .messages("ch-safe", None, 10)
        .expect("messages intact");
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].id, "m-safe");

    // Now run normal v2 migration: must succeed and index existing v1 messages into messages_fts!
    store.migrate_v2().expect("clean migrate_v2 after rollback");
    assert_eq!(store.schema_version().expect("now v2"), 2);

    let fts_hits = store
        .search_messages_fts("simulated", Some("ch-safe"), None, 10, 0)
        .expect("fts search on migrated v1 message");
    assert_eq!(fts_hits.len(), 1);
    assert_eq!(fts_hits[0].message_id, "m-safe");

    // Online backup verification via rusqlite backup feature
    let backup_path = dir.path().join("backup_v2.db");
    store.backup_to(&backup_path).expect("online backup");
    let backup_store = Store::open(&backup_path).expect("open backup");
    assert_eq!(backup_store.schema_version().expect("backup version"), 2);
    assert_eq!(
        backup_store
            .messages("ch-safe", None, 10)
            .expect("backup messages")
            .len(),
        1
    );
}

#[test]
fn account_partitioned_cache_isolates_multiple_accounts() {
    let dir = TempDir::new("accounts");
    let path_a = account_db_path(dir.path(), "111111111111111111").expect("path a");
    let path_b = account_db_path(dir.path(), "222222222222222222").expect("path b");
    assert_ne!(path_a, path_b);

    let mut store_a = Store::open_for_account(dir.path(), "111111111111111111").expect("open a");
    let mut store_b = Store::open_for_account(dir.path(), "222222222222222222").expect("open b");

    store_a.kv_set("active_user", "alice").expect("kv a");
    store_b.kv_set("active_user", "bob").expect("kv b");

    assert_eq!(
        store_a.kv_get("active_user").expect("get a"),
        Some("alice".to_owned())
    );
    assert_eq!(
        store_b.kv_get("active_user").expect("get b"),
        Some("bob".to_owned())
    );

    // Reject path traversal in account_id
    assert!(Store::open_for_account(dir.path(), "../evil").is_err());
    assert!(Store::open_for_account(dir.path(), "").is_err());
}

#[test]
fn subtask_202_1_db_worker_and_store_bridge_over_bounded_mpsc_1024_and_oneshot() {
    assert_eq!(DEFAULT_WORKER_CHANNEL_CAPACITY, 1024);

    let dir = TempDir::new("db-worker");
    let db_path = dir.db_path();

    let worker = DbWorker::spawn(&db_path).expect("spawn DbWorker thread");
    let bridge: StoreBridge = worker.bridge();

    // Verify dedicated worker thread has its own thread ID distinct from caller
    let caller_thread = std::thread::current().id();
    let worker_thread = bridge.worker_thread_id().expect("worker_thread_id");
    assert_ne!(
        caller_thread, worker_thread,
        "DB operations must execute on a dedicated std::thread DB-Worker"
    );

    assert_eq!(
        bridge.schema_version().expect("worker schema version"),
        SCHEMA_VERSION_V2
    );

    // Guilds, Channels, Messages, ReadStates, KV, and FTS5 over mpsc + oneshot
    let guild = GuildRow {
        id: "g-777".to_owned(),
        name: "Async Worker Guild".to_owned(),
        icon: None,
        owner_id: Some("u-admin".to_owned()),
    };
    bridge.put_guild(guild.clone()).expect("bridge put_guild");
    assert_eq!(bridge.guild("g-777").expect("bridge guild"), Some(guild));

    let channel = ChannelRow {
        id: "ch-777".to_owned(),
        guild_id: Some("g-777".to_owned()),
        name: Some("worker-chat".to_owned()),
        kind: 0,
        position: 0,
    };
    bridge
        .put_channel(channel.clone())
        .expect("bridge put_channel");
    assert_eq!(
        bridge.channel("ch-777").expect("bridge channel"),
        Some(channel)
    );

    let msg = MessageRow {
        id: "m-777".to_owned(),
        channel_id: "ch-777".to_owned(),
        author_id: "u-admin".to_owned(),
        content: "StoreBridge dispatches commands over bounded mpsc(1024) to rusqlite worker"
            .to_owned(),
        timestamp: 1_700_500_000,
        edited_timestamp: None,
        flags: 0,
        deleted: false,
    };
    bridge.put_message(msg.clone()).expect("bridge put_message");

    let fetched = bridge
        .messages("ch-777", None, 20)
        .expect("bridge messages");
    assert_eq!(fetched, vec![msg]);

    let rs = ReadStateRow {
        channel_id: "ch-777".to_owned(),
        last_message_id: Some("m-777".to_owned()),
        mention_count: 5,
        last_pin_timestamp: None,
    };
    bridge
        .put_read_state(rs.clone())
        .expect("bridge put_read_state");
    assert_eq!(
        bridge.read_state("ch-777").expect("bridge read_state"),
        Some(rs)
    );

    // FTS5 search over DB-Worker
    let hits = bridge
        .search_messages_fts("StoreBridge", Some("ch-777"), None, 10, 0)
        .expect("bridge search_messages_fts");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].message_id, "m-777");
    assert!(hits[0].snippet.contains("<b>StoreBridge</b>"));

    // Bulk index FTS5 records via DB-Worker
    bridge
        .index_fts_messages(vec![Fts5MessageRecord {
            id: "m-778".to_owned(),
            channel_id: "ch-777".to_owned(),
            author_id: "u-admin".to_owned(),
            content: "Additional indexed FTS5 record via DbCommand".to_owned(),
        }])
        .expect("bridge index_fts_messages");
    let hits2 = bridge
        .search_messages_fts("DbCommand", Some("ch-777"), None, 10, 0)
        .expect("bridge search DbCommand");
    assert_eq!(hits2.len(), 1);
    assert_eq!(hits2[0].message_id, "m-778");

    // Multi-threaded concurrent callers sharing StoreBridge clones
    let mut handles = Vec::new();
    for i in 0..8 {
        let b = bridge.clone();
        handles.push(std::thread::spawn(move || {
            let key = format!("k-{i}");
            let val = format!("v-{i}");
            b.kv_set(&key, &val).expect("concurrent kv_set");
            assert_eq!(b.kv_get(&key).expect("concurrent kv_get"), Some(val));
        }));
    }
    for h in handles {
        h.join().expect("join caller thread");
    }

    // Raw command dispatch over DbCommand
    let (reply_tx, reply_rx) = chord_store::oneshot_channel();
    bridge
        .send_command(DbCommand::KvGet {
            key: "k-0".to_owned(),
            reply: reply_tx,
        })
        .expect("send raw DbCommand");
    assert_eq!(
        reply_rx.recv().expect("oneshot recv").expect("kv result"),
        Some("v-0".to_owned())
    );

    // Graceful shutdown of worker thread
    worker.shutdown().expect("graceful worker shutdown");
}
