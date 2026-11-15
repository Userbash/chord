//! High-performance full-text message search engine backed by SQLite FTS5 (`RW51-EPIC4-T2`).
//!
//! Replaces O(N) linear vector scanning in `data.rs` with an O(log N) inverted index
//! using the `fts5` virtual table module (`rusqlite = "0.40.2"` / `libsqlite3` FTS5 engine),
//! `unicode61` tokenizer, BM25 ranking, and highlighted match snippets via
//! `snippet(messages_fts, -1, '<b>', '</b>', '...', 15)`.

use std::path::Path;

use crate::ffi::{self, Db, Step, Stmt};
use crate::StoreError;

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

const SQL_INSERT_FTS: &str = "
INSERT INTO messages_fts (message_id, channel_id, author_id, content)
VALUES (?1, ?2, ?3, ?4)
";

const SQL_DELETE_FTS_BY_ID: &str = "
DELETE FROM messages_fts WHERE message_id = ?1
";

/// Ranked FTS5 query with `bm25(messages_fts)` scoring and `snippet(messages_fts, -1, '<b>', '</b>', '...', 15)`.
const SQL_SEARCH_FTS_MATCH: &str = "
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
const SQL_SEARCH_FTS_SCAN: &str = "
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
    db: Db,
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
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the database cannot be opened or the `fts5` schema fails to initialize.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref();
        let text = path.to_str().ok_or_else(|| {
            StoreError::Open(format!("database path is not valid UTF-8: {}", path.display()))
        })?;
        let db = Db::open(text).map_err(|error| {
            StoreError::Open(format!(
                "sqlite3_open_v2({}) code {}: {}",
                path.display(),
                error.code,
                error.message
            ))
        })?;
        let mut engine = Self {
            db,
            indexed_count: 0,
        };
        engine.init_schema()?;
        Ok(engine)
    }

    /// Opens an in-memory SQLite FTS5 search index (`:memory:`) and initializes `messages_fts`.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the in-memory database or `fts5` virtual table cannot be created.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::open(":memory:")
    }

    /// Initializes pragmatic performance settings and creates the `messages_fts` table with `unicode61`.
    fn init_schema(&mut self) -> Result<(), StoreError> {
        self.db.busy_timeout(5_000)?;
        self.db.exec("PRAGMA synchronous = NORMAL")?;
        self.db.exec("PRAGMA temp_store = MEMORY")?;
        self.db.exec("PRAGMA cache_size = -32000")?;
        self.db.exec(SQL_CREATE_MESSAGES_FTS)?;
        Ok(())
    }

    /// Returns the total number of messages currently indexed in `messages_fts`.
    #[inline]
    #[must_use]
    pub const fn indexed_count(&self) -> usize {
        self.indexed_count
    }

    /// Clears all entries from `messages_fts` and bulk-inserts `records` within a single transaction.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if any record contains a NUL byte or the SQLite transaction fails.
    pub fn index_messages(&mut self, records: &[Fts5MessageRecord]) -> Result<(), StoreError> {
        self.db.exec("BEGIN IMMEDIATE")?;
        if let Err(error) = self.replace_all_records(records) {
            let _ = self.db.exec("ROLLBACK");
            return Err(error);
        }
        self.db.exec("COMMIT")?;
        self.indexed_count = records.len();
        Ok(())
    }

    fn replace_all_records(&mut self, records: &[Fts5MessageRecord]) -> Result<(), StoreError> {
        self.db.exec("DELETE FROM messages_fts")?;
        let mut statement = self.db.prepare(SQL_INSERT_FTS)?;
        for record in records {
            statement.bind_text(1, &record.id)?;
            statement.bind_text(2, &record.channel_id)?;
            statement.bind_text(3, &record.author_id)?;
            statement.bind_text(4, &record.content)?;
            step_done(&mut statement)?;
            statement.reset()?;
            statement.clear_bindings()?;
        }
        Ok(())
    }

    /// Inserts or updates a single message in the `messages_fts` index.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the SQLite operation fails.
    pub fn upsert_message(&mut self, record: &Fts5MessageRecord) -> Result<(), StoreError> {
        let mut del_stmt = self.db.prepare(SQL_DELETE_FTS_BY_ID)?;
        del_stmt.bind_text(1, &record.id)?;
        step_done(&mut del_stmt)?;
        let removed = self.db.changes() as usize;

        let mut ins_stmt = self.db.prepare(SQL_INSERT_FTS)?;
        ins_stmt.bind_text(1, &record.id)?;
        ins_stmt.bind_text(2, &record.channel_id)?;
        ins_stmt.bind_text(3, &record.author_id)?;
        ins_stmt.bind_text(4, &record.content)?;
        step_done(&mut ins_stmt)?;

        self.indexed_count = self.indexed_count.saturating_sub(removed) + 1;
        Ok(())
    }

    /// Searches indexed messages using SQLite FTS5 `MATCH`, optional channel/author filters, and pagination.
    ///
    /// Returns BM25-ranked results with highlighted match snippets generated via
    /// `snippet(messages_fts, -1, '<b>', '</b>', '...', 15)`.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if parameter binding or query execution fails.
    pub fn search_messages(
        &self,
        query: &str,
        channel_id: Option<&str>,
        author_id: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Fts5SearchHit>, StoreError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let clamped_limit = i64::try_from(limit.min(MAX_FTS_LIMIT)).unwrap_or(100);
        let clamped_offset = i64::try_from(offset).unwrap_or(0);
        let normalized_channel = normalize_optional_filter(channel_id);
        let normalized_author = normalize_optional_filter(author_id);

        if let Some(fts_query) = build_fts5_match_expression(query) {
            let mut statement = self.db.prepare(SQL_SEARCH_FTS_MATCH)?;
            statement.bind_text(1, &fts_query)?;
            bind_optional_text(&mut statement, 2, normalized_channel)?;
            bind_optional_text(&mut statement, 3, normalized_author)?;
            statement.bind_i64(4, clamped_limit)?;
            statement.bind_i64(5, clamped_offset)?;
            collect_hits(&mut statement)
        } else if !query.trim().is_empty() {
            // Query had only punctuation/symbols with no tokenizable characters.
            Ok(Vec::new())
        } else {
            let mut statement = self.db.prepare(SQL_SEARCH_FTS_SCAN)?;
            bind_optional_text(&mut statement, 1, normalized_channel)?;
            bind_optional_text(&mut statement, 2, normalized_author)?;
            statement.bind_i64(3, clamped_limit)?;
            statement.bind_i64(4, clamped_offset)?;
            collect_hits(&mut statement)
        }
    }
}

/// Executes an indexed FTS5 search (`search_messages(query, channel_id, author_id, limit, offset)`)
/// on the provided [`Fts5SearchEngine`].
///
/// # Errors
///
/// Returns [`StoreError`] if the underlying SQLite FTS5 query fails.
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
/// with the `unicode61` tokenizer. Each alphanumeric token is quoted and given a prefix wildcard
/// (`"token"*`) so both exact words and prefix/substring token matches resolve in O(log N) time
/// without risking FTS5 syntax errors on special characters.
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

fn bind_optional_text(
    statement: &mut Stmt<'_>,
    index: usize,
    value: Option<&str>,
) -> Result<(), StoreError> {
    match value {
        Some(text) => {
            statement.bind_text(index, text)?;
            Ok(())
        }
        None => {
            statement.bind_null(index)?;
            Ok(())
        }
    }
}

fn step_done(statement: &mut Stmt<'_>) -> Result<(), StoreError> {
    match statement.step()? {
        Step::Done => Ok(()),
        Step::Row => Err(StoreError::Sqlite {
            code: ffi::SQLITE_CORRUPT,
            message: "FTS5 write statement unexpectedly returned a row".to_owned(),
        }),
    }
}

fn required_text(statement: &Stmt<'_>, column: usize) -> Result<String, StoreError> {
    statement
        .column_text(column)?
        .ok_or_else(|| StoreError::Sqlite {
            code: ffi::SQLITE_CORRUPT,
            message: format!("FTS5 column {column} unexpectedly contained NULL"),
        })
}

fn collect_hits(statement: &mut Stmt<'_>) -> Result<Vec<Fts5SearchHit>, StoreError> {
    let mut hits = Vec::new();
    while matches!(statement.step()?, Step::Row) {
        hits.push(Fts5SearchHit {
            message_id: required_text(statement, 0)?,
            channel_id: required_text(statement, 1)?,
            author_id: required_text(statement, 2)?,
            content: required_text(statement, 3)?,
            rank: statement.column_f64(4)?,
            snippet: required_text(statement, 5)?,
        });
    }
    Ok(hits)
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
