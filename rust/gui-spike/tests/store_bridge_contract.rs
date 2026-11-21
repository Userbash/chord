//! Contract tests for STAGE-10.BFF.4: StoreBridge (SQLite worker) <-> Tauri commands `db_query` and `db_messages`.

use chord_client::data::{
    db_messages_via_bridge, db_query_via_bridge, DbMessageDto, DbQueryHitDto,
};
use chord_store::{ChannelRow, DbWorker, Fts5MessageRecord, GuildRow, MessageRow};

#[test]
fn stage_10_bff_4_db_messages_and_db_query_via_store_bridge_mpsc() {
    let worker = DbWorker::spawn_in_memory().expect("spawn in-memory DbWorker");
    let bridge = worker.bridge();

    bridge
        .put_guild(GuildRow {
            id: "g-1".to_owned(),
            name: "Guild One".to_owned(),
            icon: None,
            owner_id: None,
        })
        .expect("put_guild");

    bridge
        .put_channel(ChannelRow {
            id: "ch-1".to_owned(),
            guild_id: Some("g-1".to_owned()),
            kind: 0,
            name: Some("general".to_owned()),
            position: 0,
        })
        .expect("put_channel");

    bridge
        .put_message(MessageRow {
            id: "100".to_owned(),
            channel_id: "ch-1".to_owned(),
            author_id: "u-1".to_owned(),
            content: "Hello StoreBridge world".to_owned(),
            timestamp: 1_700_000_001,
            edited_timestamp: None,
            flags: 0,
            deleted: false,
        })
        .expect("put_message 100");

    bridge
        .put_message(MessageRow {
            id: "101".to_owned(),
            channel_id: "ch-1".to_owned(),
            author_id: "u-2".to_owned(),
            content: "Second message in channel".to_owned(),
            timestamp: 1_700_000_002,
            edited_timestamp: None,
            flags: 0,
            deleted: false,
        })
        .expect("put_message 101");

    bridge
        .index_fts_messages(vec![
            Fts5MessageRecord {
                id: "100".to_owned(),
                channel_id: "ch-1".to_owned(),
                author_id: "u-1".to_owned(),
                content: "Hello StoreBridge world".to_owned(),
            },
            Fts5MessageRecord {
                id: "101".to_owned(),
                channel_id: "ch-1".to_owned(),
                author_id: "u-2".to_owned(),
                content: "Second message in channel".to_owned(),
            },
        ])
        .expect("index_fts_messages");

    // 1. db_messages_via_bridge returns messages ordered newest-first
    let page1: Vec<DbMessageDto> =
        db_messages_via_bridge(&bridge, "ch-1", None, Some(1)).expect("db_messages_via_bridge");
    assert_eq!(page1.len(), 1);
    assert_eq!(page1[0].id, "101");
    assert_eq!(page1[0].channel_id, "ch-1");

    // 2. Pagination with `before` cursor
    let page2: Vec<DbMessageDto> =
        db_messages_via_bridge(&bridge, "ch-1", Some("101".to_owned()), Some(10))
            .expect("db_messages_via_bridge before=101");
    assert_eq!(page2.len(), 1);
    assert_eq!(page2[0].id, "100");

    // 3. db_query_via_bridge performs FTS5 search over mpsc channel
    let hits: Vec<DbQueryHitDto> =
        db_query_via_bridge(&bridge, "StoreBridge", Some("ch-1"), None, Some(10), Some(0))
            .expect("db_query_via_bridge");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].message_id, "100");
    assert!(hits[0].snippet.contains("<b>StoreBridge</b>"));

    worker.shutdown().expect("shutdown DbWorker");
}
