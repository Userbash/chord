//! Contract tests for `chord_store::fts5_search` (RW51-EPIC4-T2).
//!
//! Verifies SQLite FTS5 `messages_fts` (`unicode61` tokenizer), ranked BM25 results,
//! snippet extraction (`snippet(messages_fts, -1, '<b>', '</b>', '...', 15)`),
//! channel and author filtering, pagination (`limit`, `offset`), and `< 2 ms`
//! search latency across 100,000 indexed messages.

use std::time::Instant;

use chord_store::fts5_search::{
    search_messages, Fts5MessageRecord, Fts5SearchEngine,
};

#[test]
fn fts5_creates_messages_fts_with_unicode61_and_returns_ranked_snippets() {
    let mut engine = Fts5SearchEngine::open_in_memory().expect("open FTS5 engine");
    let records = vec![
        Fts5MessageRecord {
            id: "m-1".to_owned(),
            channel_id: "ch-news".to_owned(),
            author_id: "u-alice".to_owned(),
            content: "Welcome to the server! Call of Duty tournament starts tonight.".to_owned(),
        },
        Fts5MessageRecord {
            id: "m-2".to_owned(),
            channel_id: "ch-news".to_owned(),
            author_id: "u-bob".to_owned(),
            content: "Call of Duty patch notes and Call of Duty map rotation details.".to_owned(),
        },
        Fts5MessageRecord {
            id: "m-3".to_owned(),
            channel_id: "ch-general".to_owned(),
            author_id: "u-alice".to_owned(),
            content: "РАДЫ ВИДЕТЬ всех участников в новом сезоне!".to_owned(),
        },
    ];
    engine.index_messages(&records).expect("index messages");

    let hits = engine
        .search_messages("call of duty", None, None, 10, 0)
        .expect("search call of duty");
    assert_eq!(hits.len(), 2);
    // m-2 mentions "Call of Duty" twice and is shorter, so BM25 ranks it first (more negative score).
    assert_eq!(hits[0].message_id, "m-2");
    assert_eq!(hits[1].message_id, "m-1");
    assert!(hits[0].rank <= hits[1].rank);
    assert!(hits[0].snippet.contains("<b>Call</b>"));
    assert!(hits[0].snippet.contains("<b>Duty</b>"));

    // Filter by channel_id and author_id
    let filtered = engine
        .search_messages("call of duty", Some("ch-news"), Some("u-alice"), 10, 0)
        .expect("filtered search");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].message_id, "m-1");

    // Case-insensitive Cyrillic search via unicode61 tokenizer
    let cyrillic = search_messages(&engine, "рады видеть", Some("ch-general"), None, 10, 0)
        .expect("cyrillic search");
    assert_eq!(cyrillic.len(), 1);
    assert_eq!(cyrillic[0].message_id, "m-3");
    assert!(cyrillic[0].snippet.contains("<b>РАДЫ</b>"));
    assert!(cyrillic[0].snippet.contains("<b>ВИДЕТЬ</b>"));
}

#[test]
fn fts5_search_100k_messages_completes_under_2ms() {
    let mut engine = Fts5SearchEngine::open_in_memory().expect("open FTS5 engine");
    let mut records = Vec::with_capacity(100_000);
    for i in 0..100_000 {
        let content = if i == 42_137 {
            "Unique needle_quantum_telemetry_payload in message stream for FTS5 benchmark".to_owned()
        } else if i % 2_000 == 0 {
            format!("Periodic needle_quantum_telemetry_payload checkpoint at row {i}")
        } else {
            format!("Standard chat message number {i} discussing Rust architecture and audio")
        };
        records.push(Fts5MessageRecord {
            id: format!("msg-{i}"),
            channel_id: format!("ch-{}", i % 20),
            author_id: format!("user-{}", i % 100),
            content,
        });
    }
    engine.index_messages(&records).expect("bulk index 100k messages");

    // Warm-up query
    let warmup = engine
        .search_messages("needle_quantum_telemetry_payload", None, None, 25, 0)
        .expect("warmup search");
    assert!(!warmup.is_empty());

    let start = Instant::now();
    let hits = engine
        .search_messages("needle_quantum_telemetry_payload", None, None, 25, 0)
        .expect("benchmark search");
    let elapsed = start.elapsed();

    assert_eq!(hits.len(), 25);
    assert!(
        hits[0]
            .snippet
            .contains("<b>needle_quantum_telemetry_payload</b>"),
        "unexpected snippet: {}",
        hits[0].snippet
    );
    assert!(
        elapsed.as_micros() < 2_000,
        "Expected FTS5 search over 100,000 messages to take < 2 ms, took {:?}",
        elapsed
    );
}
