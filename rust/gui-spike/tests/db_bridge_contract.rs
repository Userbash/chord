#![forbid(unsafe_code)]
//! Contract tests for STAGE-10.BFF.4: StoreBridge (SQLite worker) <-> db_query & db_messages.

use chord_client::db_bridge::{db_messages_async, db_query_async, DbMessageDto, DbSearchHitDto};
use chord_store::{ChannelRow, DbWorker, Fts5MessageRecord, GuildRow, MessageRow};

#[tokio::test]
async fn db_messages_returns_channel_history_via_store_bridge() {
    let worker = DbWorker::spawn_in_memory().expect("spawn in-memory DbWorker");
    let bridge = worker.bridge();

    bridge
        .put_guild(GuildRow {
            id: "g-1".to_owned(),
            name: "Guild 1".to_owned(),
            icon: None,
            owner_id: None,
        })
        .expect("put_guild");
    bridge
        .put_channel(ChannelRow {
            id: "ch-100".to_owned(),
            guild_id: Some("g-1".to_owned()),
            name: Some("general".to_owned()),
            kind: 0,
            position: 0,
        })
        .expect("put_channel");

    bridge
        .put_message(MessageRow {
            id: "1001".to_owned(),
            channel_id: "ch-100".to_owned(),
            author_id: "u-1".to_owned(),
            content: "first message".to_owned(),
            timestamp: 1_700_000_001,
            edited_timestamp: None,
            flags: 0,
            deleted: false,
        })
        .expect("put_message 1");
    bridge
        .put_message(MessageRow {
            id: "1002".to_owned(),
            channel_id: "ch-100".to_owned(),
            author_id: "u-2".to_owned(),
            content: "second message".to_owned(),
            timestamp: 1_700_000_002,
            edited_timestamp: None,
            flags: 0,
            deleted: false,
        })
        .expect("put_message 2");

    let rows: Vec<DbMessageDto> = db_messages_async(&bridge, "ch-100".to_owned(), None, Some(10))
        .await
        .expect("db_messages_async");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, "1002");
    assert_eq!(rows[1].id, "1001");
}

#[tokio::test]
async fn db_query_searches_fts5_messages_via_store_bridge() {
    let worker = DbWorker::spawn_in_memory().expect("spawn in-memory DbWorker");
    let bridge = worker.bridge();

    bridge
        .index_fts_messages(vec![Fts5MessageRecord {
            id: "2001".to_owned(),
            channel_id: "ch-200".to_owned(),
            author_id: "u-9".to_owned(),
            content: "StoreBridge async mpsc channel works".to_owned(),
        }])
        .expect("index_fts_messages");

    let hits: Vec<DbSearchHitDto> = db_query_async(
        &bridge,
        "StoreBridge".to_owned(),
        Some("ch-200".to_owned()),
        None,
        Some(10),
        Some(0),
    )
    .await
    .expect("db_query_async");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].message_id, "2001");
    assert!(hits[0].snippet.contains("StoreBridge"));
}
