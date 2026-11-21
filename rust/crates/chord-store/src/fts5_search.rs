#![forbid(unsafe_code)]
//! High-performance full-text message search engine backed by SQLite FTS5 (`RW51-EPIC4-T2` / `LIB-SIMPLIFY-02`).
//!
//! Uses `rusqlite` (`bundled` SQLite FTS5 engine) with the `unicode61` tokenizer,
//! BM25 ranking (`bm25(messages_fts)`), and highlighted match snippets via
//! `snippet(messages_fts, -1, '<b>', '</b>', '...', 15)`.

use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, params};

use crate::{StoreError, map_rusqlite_error, validate_no_nul};

/// SQLite FTS5 schema definition for the `messages_fts` virtual table using the `unicode61` tokenizer.
pub const SQL_CREATE_MESSAGES_FTS: &str = "
CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
    message_id UNINDEXED,
    channel_id UNINDEXED,
    author_id UNINDEXED,
    content,
    tokenize = 'unicode61'
);
";

pub(crate) const SQL_INSERT_FTS: &str = "
INSERT INTO messages_fts (message_id, channel_id, author_id, content)
VALUES (?1, ?2, ?3, ?4)
";

pub(crate) const SQL_DELETE_FTS_BY_ID: &str = "
DELETE FROM messages_fts WHERE message_id = ?1
";

/// Ranked FTS5 query with `bm25(messages_fts)` scoring and `snippet(messages_fts, -1, '<b>', '</b>', '...', 15)`.
pub(crate) const SQL_SEARCH_FTS_MATCH: &str = "
SELECT
    message_id,
    channel_id,
    author_id,
    content,
    bm25(messages_fts) AS rank,
    snippet(messages_fts, -1, '<b>', '</b>', '...', 15) AS snippet
FROM messages_fts
WHERE messages_fts MATCH ?1
  AND (?2 IS NULL OR channel_id = ?2)
  AND (?3 IS NULL OR author_id = ?3)
ORDER BY rank ASC, rowid ASC
LIMIT ?4 OFFSET ?5
";

/// Filter-only query when the full-text search query string is empty.
pub(crate) const SQL_SEARCH_FTS_SCAN: &str = "
SELECT
    message_id,
    channel_id,
    author_id,
    content,
    0.0 AS rank,
    substr(content, 1, 120) AS snippet
FROM messages_fts
WHERE (?1 IS NULL OR channel_id = ?1)
  AND (?2 IS NULL OR author_id = ?2)
ORDER BY rowid ASC
LIMIT ?3 OFFSET ?4
";

const MAX_FTS_LIMIT: usize = 100_000;

/// Input message record to be indexed into the `messages_fts` virtual table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fts5MessageRecord {
    /// Unique Discord message Snowflake ID.
    pub id: String,
    /// Channel Snowflake ID where the message was posted.
    pub channel_id: String,
    /// Author user Snowflake ID.
    pub author_id: String,
    /// Message text content indexed by `unicode61`.
    pub content: String,
}

/// Ranked full-text search result returned by [`Fts5SearchEngine::search_messages`].
#[derive(Debug, Clone, PartialEq)]
pub struct Fts5SearchHit {
    /// Matched message Snowflake ID.
    pub message_id: String,
    /// Channel Snowflake ID containing the message.
    pub channel_id: String,
    /// Author user Snowflake ID.
    pub author_id: String,
    /// Original full message content.
    pub content: String,
    /// Highlighted match snippet produced by `snippet(messages_fts, -1, '<b>', '</b>', '...', 15)`.
    pub snippet: String,
    /// BM25 relevance score (more negative values indicate higher relevance).
    pub rank: f64,
}

/// SQLite FTS5 search engine managing the `messages_fts` virtual table (`tokenize = 'unicode61'`).
pub struct Fts5SearchEngine {
    conn: Connection,
    indexed_count: usize,
}

impl std::fmt::Debug for Fts5SearchEngine {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Fts5SearchEngine")
            .field("indexed_count", &self.indexed_count)
            .finish_non_exhaustive()
    }
}

impl Fts5SearchEngine {
    /// Opens or creates a file-backed SQLite FTS5 search index at `path` and initializes `messages_fts`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref();
        let text = path.to_str().ok_or_else(|| {
            StoreError::Open(format!(
                "database path is not valid UTF-8: {}",
                path.display()
            ))
        })?;
        if text.contains('\0') {
            return Err(StoreError::Open(
                "database path contains NUL byte".to_owned(),
            ));
        }
        let conn = if text == ":memory:" {
            Connection::open_in_memory()
        } else {
            Connection::open(path)
        }
        .map_err(|err| StoreError::Open(format!("rusqlite open({}): {err}", path.display())))?;

        let mut engine = Self {
            conn,
            indexed_count: 0,
        };
        engine.init_schema()?;
        Ok(engine)
    }

    /// Opens an in-memory SQLite FTS5 search index (`:memory:`) and initializes `messages_fts`.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::open(":memory:")
    }

    fn init_schema(&mut self) -> Result<(), StoreError> {
        self.conn
            .busy_timeout(Duration::from_millis(5_000))
            .map_err(map_rusqlite_error)?;
        self.conn
            .execute_batch(
                "PRAGMA synchronous = NORMAL;
                 PRAGMA temp_store = MEMORY;
                 PRAGMA cache_size = -32000;",
            )
            .map_err(map_rusqlite_error)?;
        self.conn
            .execute_batch(SQL_CREATE_MESSAGES_FTS)
            .map_err(map_rusqlite_error)?;
        Ok(())
    }

    /// Returns the total number of messages currently indexed in `messages_fts`.
    #[inline]
    #[must_use]
    pub const fn indexed_count(&self) -> usize {
        self.indexed_count
    }

    /// Clears all entries from `messages_fts` and bulk-inserts `records` within a single transaction.
    pub fn index_messages(&mut self, records: &[Fts5MessageRecord]) -> Result<(), StoreError> {
        index_records_on_conn(&self.conn, records)?;
        self.indexed_count = records.len();
        Ok(())
    }

    /// Inserts or updates a single message in the `messages_fts` index.
    pub fn upsert_message(&mut self, record: &Fts5MessageRecord) -> Result<(), StoreError> {
        let removed = upsert_record_on_conn(&self.conn, record)?;
        self.indexed_count = self.indexed_count.saturating_sub(removed) + 1;
        Ok(())
    }

    /// Searches indexed messages using SQLite FTS5 `MATCH`, optional channel/author filters, and pagination.
    pub fn search_messages(
        &self,
        query: &str,
        channel_id: Option<&str>,
        author_id: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Fts5SearchHit>, StoreError> {
        search_on_conn(&self.conn, query, channel_id, author_id, limit, offset)
    }
}

pub(crate) fn index_records_on_conn(
    conn: &Connection,
    records: &[Fts5MessageRecord],
) -> Result<(), StoreError> {
    for rec in records {
        validate_no_nul(&rec.id)?;
        validate_no_nul(&rec.channel_id)?;
        validate_no_nul(&rec.author_id)?;
        validate_no_nul(&rec.content)?;
    }
    conn.execute_batch("BEGIN IMMEDIATE")
        .map_err(map_rusqlite_error)?;
    let result = (|| -> Result<(), StoreError> {
        conn.execute("DELETE FROM messages_fts", [])
            .map_err(map_rusqlite_error)?;
        let mut stmt = conn.prepare(SQL_INSERT_FTS).map_err(map_rusqlite_error)?;
        for rec in records {
            stmt.execute(params![rec.id, rec.channel_id, rec.author_id, rec.content])
                .map_err(map_rusqlite_error)?;
        }
        Ok(())
    })();
    if let Err(err) = result {
        let _ = conn.execute_batch("ROLLBACK");
        return Err(err);
    }
    conn.execute_batch("COMMIT").map_err(map_rusqlite_error)
}

pub(crate) fn upsert_record_on_conn(
    conn: &Connection,
    record: &Fts5MessageRecord,
) -> Result<usize, StoreError> {
    validate_no_nul(&record.id)?;
    validate_no_nul(&record.channel_id)?;
    validate_no_nul(&record.author_id)?;
    validate_no_nul(&record.content)?;
    let removed = conn
        .execute(SQL_DELETE_FTS_BY_ID, params![record.id])
        .map_err(map_rusqlite_error)?;
    conn.execute(
        SQL_INSERT_FTS,
        params![
            record.id,
            record.channel_id,
            record.author_id,
            record.content
        ],
    )
    .map_err(map_rusqlite_error)?;
    Ok(removed)
}

pub(crate) fn search_on_conn(
    conn: &Connection,
    query: &str,
    channel_id: Option<&str>,
    author_id: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<Vec<Fts5SearchHit>, StoreError> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    validate_no_nul(query)?;
    let normalized_channel = normalize_optional_filter(channel_id);
    let normalized_author = normalize_optional_filter(author_id);
    if let Some(ch) = normalized_channel {
        validate_no_nul(ch)?;
    }
    if let Some(au) = normalized_author {
        validate_no_nul(au)?;
    }
    let clamped_limit = i64::try_from(limit.min(MAX_FTS_LIMIT)).unwrap_or(100);
    let clamped_offset = i64::try_from(offset).unwrap_or(0);

    if let Some(fts_query) = build_fts5_match_expression(query) {
        query_fts_match(
            conn,
            &fts_query,
            normalized_channel,
            normalized_author,
            clamped_limit,
            clamped_offset,
        )
    } else if !query.trim().is_empty() {
        Ok(Vec::new())
    } else {
        query_fts_scan(
            conn,
            normalized_channel,
            normalized_author,
            clamped_limit,
            clamped_offset,
        )
    }
}

fn query_fts_match(
    conn: &Connection,
    fts_query: &str,
    channel: Option<&str>,
    author: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<Fts5SearchHit>, StoreError> {
    let mut stmt = conn
        .prepare(SQL_SEARCH_FTS_MATCH)
        .map_err(map_rusqlite_error)?;
    let rows = stmt
        .query_map(
            params![fts_query, channel, author, limit, offset],
            map_hit_row,
        )
        .map_err(map_rusqlite_error)?;
    collect_mapped_hits(rows)
}

fn query_fts_scan(
    conn: &Connection,
    channel: Option<&str>,
    author: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<Fts5SearchHit>, StoreError> {
    let mut stmt = conn
        .prepare(SQL_SEARCH_FTS_SCAN)
        .map_err(map_rusqlite_error)?;
    let rows = stmt
        .query_map(params![channel, author, limit, offset], map_hit_row)
        .map_err(map_rusqlite_error)?;
    collect_mapped_hits(rows)
}

fn map_hit_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Fts5SearchHit> {
    Ok(Fts5SearchHit {
        message_id: row.get(0)?,
        channel_id: row.get(1)?,
        author_id: row.get(2)?,
        content: row.get(3)?,
        rank: row.get(4)?,
        snippet: row.get(5)?,
    })
}

fn collect_mapped_hits<F>(
    rows: rusqlite::MappedRows<'_, F>,
) -> Result<Vec<Fts5SearchHit>, StoreError>
where
    F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<Fts5SearchHit>,
{
    let mut hits = Vec::new();
    for row in rows {
        hits.push(row.map_err(map_rusqlite_error)?);
    }
    Ok(hits)
}

/// Executes an indexed FTS5 search (`search_messages(query, channel_id, author_id, limit, offset)`)
/// on the provided [`Fts5SearchEngine`].
#[inline]
pub fn search_messages(
    engine: &Fts5SearchEngine,
    query: &str,
    channel_id: Option<&str>,
    author_id: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<Vec<Fts5SearchHit>, StoreError> {
    engine.search_messages(query, channel_id, author_id, limit, offset)
}

/// Sanitizes an arbitrary user search string into a safe FTS5 `MATCH` expression compatible
/// with the `unicode61` tokenizer.
#[must_use]
pub fn build_fts5_match_expression(raw_query: &str) -> Option<String> {
    let trimmed = raw_query.trim();
    if trimmed.is_empty() {
        return None;
    }
    let tokens: Vec<String> = trimmed
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
        .filter(|part| !part.is_empty())
        .map(|token| {
            let escaped = token.replace('"', "\"\"");
            format!("\"{escaped}\"*")
        })
        .collect();
    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join(" "))
    }
}

fn normalize_optional_filter(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|text| !text.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_expression_escapes_punctuation_and_builds_prefix_tokens() {
        assert_eq!(build_fts5_match_expression("   "), None);
        assert_eq!(build_fts5_match_expression("!!! ???"), None);
        assert_eq!(
            build_fts5_match_expression("call of duty"),
            Some("\"call\"* \"of\"* \"duty\"*".to_owned())
        );
        assert_eq!(
            build_fts5_match_expression("РАДЫ ВИДЕТЬ!"),
            Some("\"РАДЫ\"* \"ВИДЕТЬ\"*".to_owned())
        );
    }
}
