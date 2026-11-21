use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use chord_store::{ChannelRow, MessageRow, Store};

fn temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("chord_store_{name}_{nanos}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[test]
fn r6_06_purge_message_cache_frees_disk_space_and_preserves_credentials() {
    let dir = temp_dir("purge_msg");
    let db_path = dir.join("store.db");
    let mut store = Store::open(&db_path).expect("open store");
    store.migrate_v2().expect("migrate v2");

    store
        .put_channel(&ChannelRow {
            id: "c1".to_string(),
            guild_id: Some("g1".to_string()),
            name: Some("general".to_string()),
            kind: 0,
            position: 0,
        })
        .expect("put channel");

    // Insert enough message payload to grow the SQLite file noticeably.
    let big_body = "x".repeat(4096);
    for i in 0..200 {
        store
            .put_message(&MessageRow {
                id: format!("m{i:04}"),
                channel_id: "c1".to_string(),
                author_id: "u1".to_string(),
                content: format!("{i}:{big_body}"),
                timestamp: 1_700_000_000_000 + i as i64,
                edited_timestamp: None,
                flags: 0,
                deleted: false,
            })
            .expect("put message");
    }

    store
        .kv_set("auth.token", "secret-session-token")
        .expect("set token");

    let report = store.purge_message_cache().expect("purge message cache");
    assert_eq!(report.messages_deleted, 200);
    assert!(
        report.db_bytes_after < report.db_bytes_before,
        "expected VACUUM to shrink db file: before={}, after={}",
        report.db_bytes_before,
        report.db_bytes_after
    );
    assert!(report.bytes_freed > 0);

    let remaining = store.messages("c1", None, 50).expect("query messages");
    assert!(remaining.is_empty());
    assert_eq!(
        store.kv_get("auth.token").expect("get token").as_deref(),
        Some("secret-session-token")
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn r6_06_purge_media_thumbnails_frees_disk_space_and_handles_missing_dir() {
    let dir = temp_dir("purge_thumbs");
    let thumb_dir = dir.join("thumbnails");
    fs::create_dir_all(thumb_dir.join("nested")).expect("create nested thumb dir");

    fs::write(thumb_dir.join("a.png"), vec![1u8; 2048]).expect("write a.png");
    fs::write(thumb_dir.join("nested/b.webp"), vec![2u8; 4096]).expect("write b.webp");

    let report = chord_store::purge_media_thumbnails(&thumb_dir).expect("purge thumbnails");
    assert_eq!(report.files_deleted, 2);
    assert_eq!(report.bytes_freed, 6144);
    assert!(thumb_dir.exists(), "cache root dir should remain usable");
    assert_eq!(fs::read_dir(&thumb_dir).expect("read_dir").count(), 0);

    // Missing directory should be a no-op, not an error.
    let missing = dir.join("does_not_exist");
    let empty_report = chord_store::purge_media_thumbnails(&missing).expect("missing dir ok");
    assert_eq!(empty_report.files_deleted, 0);
    assert_eq!(empty_report.bytes_freed, 0);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn r6_06_reset_local_preferences_restores_defaults_and_keeps_login_tokens_intact() {
    let mut store = Store::open_in_memory().expect("open in-memory");
    store.migrate_v2().expect("migrate v2");

    // Credentials
    store.kv_set("auth.token", "tok_live_123").expect("set auth.token");
    store
        .kv_set("session.refresh_token", "ref_456")
        .expect("set refresh_token");

    // Local preferences
    store.kv_set("ui.theme", "amoled").expect("set theme");
    store.kv_set("ui.zoom", "125").expect("set zoom");
    store.kv_set("audio.input_gain", "0.85").expect("set gain");

    let report = store
        .reset_local_preferences_with_defaults(
            &["auth.token", "session.refresh_token"],
            &[("ui.theme", "dark"), ("ui.zoom", "100")],
        )
        .expect("reset preferences");

    assert_eq!(report.keys_removed, 3);
    assert_eq!(report.defaults_restored, 2);
    assert_eq!(report.credentials_preserved, 2);

    // Login tokens intact
    assert_eq!(
        store.kv_get("auth.token").expect("get token").as_deref(),
        Some("tok_live_123")
    );
    assert_eq!(
        store
            .kv_get("session.refresh_token")
            .expect("get refresh")
            .as_deref(),
        Some("ref_456")
    );

    // Defaults restored, custom-only preference cleared
    assert_eq!(
        store.kv_get("ui.theme").expect("get theme").as_deref(),
        Some("dark")
    );
    assert_eq!(
        store.kv_get("ui.zoom").expect("get zoom").as_deref(),
        Some("100")
    );
    assert_eq!(store.kv_get("audio.input_gain").expect("get gain"), None);
}
