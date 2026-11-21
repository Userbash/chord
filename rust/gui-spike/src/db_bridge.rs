#![forbid(unsafe_code)]
//! Async bridge between Tauri commands (`db_query`, `db_messages`) and
//! `chord_store::StoreBridge` (SQLite DB-Worker over bounded `mpsc(1024)`).

use chord_store::{Fts5SearchHit, MessageRow, StoreBridge};
use serde::{Deserialize, Serialize};

const DEFAULT_MESSAGES_LIMIT: usize = 50;
const MAX_MESSAGES_LIMIT: usize = 200;
const DEFAULT_QUERY_LIMIT: usize = 25;
const MAX_QUERY_LIMIT: usize = 100;

/// Serializable message DTO returned by `db_messages`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DbMessageDto {
    pub id: String,
    pub channel_id: String,
    pub author_id: String,
    pub content: String,
    pub timestamp: i64,
    pub edited_timestamp: Option<i64>,
    pub flags: u64,
    pub deleted: bool,
}

impl From<MessageRow> for DbMessageDto {
    fn from(row: MessageRow) -> Self {
        Self {
            id: row.id,
            channel_id: row.channel_id,
            author_id: row.author_id,
            content: row.content,
            timestamp: row.timestamp,
            edited_timestamp: row.edited_timestamp,
            flags: row.flags,
            deleted: row.deleted,
        }
    }
}

/// Serializable FTS5 search hit DTO returned by `db_query`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DbSearchHitDto {
    pub message_id: String,
    pub channel_id: String,
    pub author_id: String,
    pub content: String,
    pub snippet: String,
    pub rank: f64,
}

impl From<Fts5SearchHit> for DbSearchHitDto {
    fn from(hit: Fts5SearchHit) -> Self {
        Self {
            message_id: hit.message_id,
            channel_id: hit.channel_id,
            author_id: hit.author_id,
            content: hit.content,
            snippet: hit.snippet,
            rank: hit.rank,
        }
    }
}

/// Asynchronously fetches channel message history via `StoreBridge` (`mpsc` + `oneshot`).
pub async fn db_messages_async(
    bridge: &StoreBridge,
    channel_id: String,
    before: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<DbMessageDto>, String> {
    let bridge = bridge.clone();
    let limit = limit
        .unwrap_or(DEFAULT_MESSAGES_LIMIT)
        .clamp(1, MAX_MESSAGES_LIMIT);
    tokio::task::spawn_blocking(move || {
        bridge
            .messages(channel_id, before, limit)
            .map(|rows| rows.into_iter().map(DbMessageDto::from).collect())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("db_messages task join error: {e}"))?
}

/// Asynchronously searches FTS5 message index via `StoreBridge` (`mpsc` + `oneshot`).
pub async fn db_query_async(
    bridge: &StoreBridge,
    query: String,
    channel_id: Option<String>,
    author_id: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<DbSearchHitDto>, String> {
    let bridge = bridge.clone();
    let limit = limit
        .unwrap_or(DEFAULT_QUERY_LIMIT)
        .clamp(1, MAX_QUERY_LIMIT);
    let offset = offset.unwrap_or(0);
    tokio::task::spawn_blocking(move || {
        bridge
            .search_messages_fts(
                query,
                channel_id.as_deref(),
                author_id.as_deref(),
                limit,
                offset,
            )
            .map(|hits| hits.into_iter().map(DbSearchHitDto::from).collect())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("db_query task join error: {e}"))?
}
