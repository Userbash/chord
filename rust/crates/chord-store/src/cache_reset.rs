//! Client cache purge and local preferences reset (R6-06).
//!
//! Provides settings actions to purge the local SQLite message cache (freeing
//! disk space via `VACUUM` + WAL checkpoint), purge temporary media thumbnails,
//! and reset local preferences while keeping login tokens / credentials intact.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use rusqlite::params;

use crate::Store;
use crate::schema::{StoreError, map_rusqlite_error, validate_no_nul};

/// Result of purging the local SQLite message cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PurgeCacheReport {
    pub messages_deleted: usize,
    pub db_bytes_before: u64,
    pub db_bytes_after: u64,
    pub bytes_freed: u64,
}

/// Result of purging temporary media thumbnails from disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThumbnailPurgeReport {
    pub files_deleted: usize,
    pub bytes_freed: u64,
}

/// Result of resetting local preferences in `kv` while preserving credentials.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreferencesResetReport {
    pub keys_removed: usize,
    pub defaults_restored: usize,
    pub credentials_preserved: usize,
}

impl Store {
    /// Purges all cached messages (and FTS5 index rows if present) and runs
    /// `PRAGMA wal_checkpoint(TRUNCATE)` + `VACUUM` to reclaim disk space.
    ///
    /// Credentials in `kv`, `guilds`, `channels`, and `read_states` are preserved.
    pub fn purge_message_cache(&mut self) -> Result<PurgeCacheReport, StoreError> {
        let is_memory = self.path.as_os_str() == ":memory:";
        if !is_memory {
            let _ = self.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        }
        let db_bytes_before = if is_memory {
            0
        } else {
            db_disk_usage(&self.path)
        };

        let tx = self.conn.transaction().map_err(map_rusqlite_error)?;
        let messages_deleted = tx
            .execute("DELETE FROM messages", [])
            .map_err(map_rusqlite_error)?;

        let has_fts: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='messages_fts')",
                [],
                |row| row.get(0),
            )
            .map_err(map_rusqlite_error)?;
        if has_fts {
            tx.execute("DELETE FROM messages_fts", [])
                .map_err(map_rusqlite_error)?;
        }
        tx.commit().map_err(map_rusqlite_error)?;

        self.conn
            .execute_batch("VACUUM; PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(map_rusqlite_error)?;

        let db_bytes_after = if is_memory {
            0
        } else {
            db_disk_usage(&self.path)
        };
        let bytes_freed = db_bytes_before.saturating_sub(db_bytes_after);

        Ok(PurgeCacheReport {
            messages_deleted,
            db_bytes_before,
            db_bytes_after,
            bytes_freed,
        })
    }

    /// Resets local preferences in `kv` while keeping `protected_keys`
    /// (login tokens / credentials) intact, and restores `defaults`.
    pub fn reset_local_preferences_with_defaults(
        &mut self,
        protected_keys: &[&str],
        defaults: &[(&str, &str)],
    ) -> Result<PreferencesResetReport, StoreError> {
        for key in protected_keys {
            validate_no_nul(key)?;
        }
        for (key, val) in defaults {
            validate_no_nul(key)?;
            validate_no_nul(val)?;
        }

        let protected: HashSet<&str> = protected_keys.iter().copied().collect();
        let tx = self.conn.transaction().map_err(map_rusqlite_error)?;

        let all_keys: Vec<String> = {
            let mut stmt = tx
                .prepare("SELECT key FROM kv")
                .map_err(map_rusqlite_error)?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(map_rusqlite_error)?;
            let mut collected = Vec::new();
            for row in rows {
                collected.push(row.map_err(map_rusqlite_error)?);
            }
            collected
        };

        let mut keys_removed = 0usize;
        let mut credentials_preserved = 0usize;

        for key in &all_keys {
            if protected.contains(key.as_str()) {
                credentials_preserved += 1;
            } else {
                tx.execute("DELETE FROM kv WHERE key = ?1", params![key])
                    .map_err(map_rusqlite_error)?;
                keys_removed += 1;
            }
        }

        let mut defaults_restored = 0usize;
        for (key, val) in defaults {
            if !protected.contains(key) {
                tx.execute(
                    "INSERT INTO kv(key, value) VALUES (?1, ?2)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    params![key, val],
                )
                .map_err(map_rusqlite_error)?;
                defaults_restored += 1;
            }
        }

        tx.commit().map_err(map_rusqlite_error)?;

        Ok(PreferencesResetReport {
            keys_removed,
            defaults_restored,
            credentials_preserved,
        })
    }

    /// Resets all non-credential local preferences in `kv` without restoring defaults.
    pub fn reset_local_preferences(
        &mut self,
        protected_keys: &[&str],
    ) -> Result<PreferencesResetReport, StoreError> {
        self.reset_local_preferences_with_defaults(protected_keys, &[])
    }
}

/// Purges all temporary media thumbnail files inside `cache_dir` while
/// leaving `cache_dir` itself intact (if it exists).
///
/// Missing `cache_dir` is treated as a no-op returning zeroed counts.
pub fn purge_media_thumbnails(cache_dir: &Path) -> Result<ThumbnailPurgeReport, StoreError> {
    if !cache_dir.exists() {
        return Ok(ThumbnailPurgeReport {
            files_deleted: 0,
            bytes_freed: 0,
        });
    }

    let mut files_deleted = 0usize;
    let mut bytes_freed = 0u64;
    purge_dir_contents(cache_dir, &mut files_deleted, &mut bytes_freed)?;

    Ok(ThumbnailPurgeReport {
        files_deleted,
        bytes_freed,
    })
}

fn purge_dir_contents(
    dir: &Path,
    files_deleted: &mut usize,
    bytes_freed: &mut u64,
) -> Result<(), StoreError> {
    let entries = fs::read_dir(dir)
        .map_err(|err| StoreError::Open(format!("read_dir {}: {err}", dir.display())))?;

    for entry in entries {
        let entry = entry.map_err(|err| StoreError::Open(format!("dir entry: {err}")))?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path)
            .map_err(|err| StoreError::Open(format!("metadata {}: {err}", path.display())))?;

        if meta.is_dir() {
            purge_dir_contents(&path, files_deleted, bytes_freed)?;
            fs::remove_dir(&path)
                .map_err(|err| StoreError::Open(format!("remove_dir {}: {err}", path.display())))?;
        } else {
            let len = meta.len();
            fs::remove_file(&path).map_err(|err| {
                StoreError::Open(format!("remove_file {}: {err}", path.display()))
            })?;
            *files_deleted += 1;
            *bytes_freed += len;
        }
    }
    Ok(())
}

fn db_disk_usage(db_path: &Path) -> u64 {
    let main_bytes = fs::metadata(db_path).map(|m| m.len()).unwrap_or(0);
    let mut wal_os = db_path.as_os_str().to_os_string();
    wal_os.push("-wal");
    let wal_bytes = fs::metadata(Path::new(&wal_os)).map(|m| m.len()).unwrap_or(0);
    main_bytes.saturating_add(wal_bytes)
}
