#![allow(dead_code)]
use crate::state::AppState;
use crate::state::models::{
    Channel, ChannelType, Guild, Message, MessageKind, QuickSwitcherItem, QuickSwitcherItemKind,
    UserSummary,
};
use rusqlite::{Connection, Result, params};
use std::path::Path;

/// Локальное постоянное хранилище на базе SQLite (заменяет 10 850 строк ручного кода data.rs).
pub struct LocalDatabase {
    conn: Connection,
}

impl LocalDatabase {
    /// Открыть базу данных в памяти (для тестов / быстрого старта)
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    /// Открыть базу данных на диске
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;

            CREATE TABLE IF NOT EXISTS guilds (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                icon TEXT,
                owner_id TEXT,
                unread_count INTEGER DEFAULT 0,
                mention_count INTEGER DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS channels (
                id TEXT PRIMARY KEY,
                guild_id TEXT,
                category_id TEXT,
                name TEXT NOT NULL,
                channel_type INTEGER NOT NULL,
                topic TEXT,
                position INTEGER DEFAULT 0,
                unread INTEGER DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                channel_id TEXT NOT NULL,
                author_id TEXT NOT NULL,
                author_name TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                is_pinned INTEGER DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_messages_channel 
                ON messages(channel_id, timestamp);

            CREATE TABLE IF NOT EXISTS user_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                username TEXT NOT NULL,
                display_name TEXT,
                avatar TEXT,
                is_bot INTEGER DEFAULT 0
            );
            "#,
        )?;
        // Попытка инициализировать виртуальную таблицу FTS5 для QuickSwitcher (если поддерживается SQLite)
        let _ = self.conn.execute_batch(
            r#"
            CREATE VIRTUAL TABLE IF NOT EXISTS quick_switcher_fts USING fts5(
                id UNINDEXED,
                kind,
                name,
                subtitle
            );
            "#,
        );
        Ok(())
    }

    pub fn insert_guild(&self, guild: &Guild) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO guilds (id, name, icon, owner_id, unread_count, mention_count) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![guild.id, guild.name, guild.icon, guild.owner_id, guild.unread_count, guild.mention_count],
        )?;
        Ok(())
    }

    pub fn insert_channel(&self, ch: &Channel) -> Result<()> {
        let ch_type = match ch.channel_type {
            ChannelType::GuildText => 0,
            ChannelType::GuildVoice => 2,
            ChannelType::GuildAnnouncement => 5,
            ChannelType::Dm => 1,
            ChannelType::GroupDm => 3,
        };

        self.conn.execute(
            "INSERT OR REPLACE INTO channels (id, guild_id, category_id, name, channel_type, topic, position, unread) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![ch.id, ch.guild_id, ch.category_id, ch.name, ch_type, ch.topic, ch.position, ch.unread as i32],
        )?;
        Ok(())
    }

    pub fn insert_message(&self, msg: &Message) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO messages (id, channel_id, author_id, author_name, content, timestamp, is_pinned) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                msg.id,
                msg.channel_id,
                msg.author.id,
                msg.author.username,
                msg.content,
                msg.timestamp,
                msg.is_pinned as i32
            ],
        )?;
        Ok(())
    }

    pub fn get_channel_messages(&self, channel_id: &str, limit: u32) -> Result<Vec<Message>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, channel_id, author_id, author_name, content, timestamp, is_pinned FROM messages WHERE channel_id = ?1 ORDER BY timestamp DESC LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![channel_id, limit], |row| {
            let author_id: String = row.get(2)?;
            let author_name: String = row.get(3)?;
            let is_pinned: i32 = row.get(6)?;

            Ok(Message {
                id: row.get(0)?,
                channel_id: row.get(1)?,
                author: UserSummary {
                    id: author_id,
                    username: author_name,
                    global_name: None,
                    avatar: None,
                    is_bot: false,
                },
                content: row.get(4)?,
                timestamp: row.get(5)?,
                edited_timestamp: None,
                attachments: Vec::new(),
                reactions: Vec::new(),
                is_pinned: is_pinned != 0,
                message_kind: MessageKind::Default,
            })
        })?;

        let mut messages = Vec::new();
        for r in rows {
            messages.push(r?);
        }
        messages.reverse();
        Ok(messages)
    }

    pub fn set_setting(&self, key: &str, val: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO user_settings (key, value) VALUES (?1, ?2)",
            params![key, val],
        )?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM user_settings WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        if let Some(row) = rows.next()? {
            let val: String = row.get(0)?;
            Ok(Some(val))
        } else {
            Ok(None)
        }
    }

    /// Полнотекстовый поиск по сообщениям с фильтрацией (FTS)
    pub fn search_messages(
        &self,
        query: &str,
        channel_id: Option<&str>,
        author_name: Option<&str>,
    ) -> Result<Vec<Message>> {
        let mut sql = String::from(
            "SELECT id, channel_id, author_id, author_name, content, timestamp, is_pinned 
             FROM messages 
             WHERE content LIKE ?1",
        );
        let pattern = format!("%{}%", query);
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(pattern)];

        if let Some(cid) = channel_id {
            sql.push_str(" AND channel_id = ?");
            params_vec.push(Box::new(cid.to_string()));
        }

        if let Some(author) = author_name {
            sql.push_str(" AND author_name LIKE ?");
            params_vec.push(Box::new(format!("%{}%", author)));
        }

        sql.push_str(" ORDER BY timestamp DESC LIMIT 50");

        let mut stmt = self.conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(Message {
                id: row.get(0)?,
                channel_id: row.get(1)?,
                author: UserSummary {
                    id: row.get(2)?,
                    username: row.get(3)?,
                    global_name: None,
                    avatar: None,
                    is_bot: false,
                },
                content: row.get(4)?,
                timestamp: row.get(5)?,
                edited_timestamp: None,
                attachments: Vec::new(),
                reactions: Vec::new(),
                is_pinned: row.get::<_, i32>(6)? != 0,
                message_kind: MessageKind::Default,
            })
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    /// Индексация элементов быстрого перехода (Quick Switcher) в SQLite FTS5
    pub fn index_quick_switcher(&self, items: &[QuickSwitcherItem]) -> Result<()> {
        let _ = self.conn.execute("DELETE FROM quick_switcher_fts", []);
        let mut stmt = self.conn.prepare(
            "INSERT INTO quick_switcher_fts (id, kind, name, subtitle) VALUES (?1, ?2, ?3, ?4)",
        )?;
        for item in items {
            let kind_str = match item.kind {
                QuickSwitcherItemKind::Server => "server",
                QuickSwitcherItemKind::Channel => "channel",
                QuickSwitcherItemKind::User => "user",
            };
            stmt.execute(params![item.id, kind_str, item.name, item.subtitle])?;
        }
        Ok(())
    }

    /// Синхронизация всех доступных элементов из AppState в хранилище FTS
    pub fn sync_quick_switcher_from_app_state(&self, state: &AppState) -> Result<()> {
        let items = state.all_quick_switcher_items();
        self.index_quick_switcher(&items)
    }

    /// Полнотекстовый поиск Quick Switcher через FTS5 с фильтрацией по префиксам (*, #, @, !)
    pub fn search_quick_switcher_fts(&self, query: &str) -> Result<Vec<QuickSwitcherItem>> {
        let q = query.trim();
        let (filter_kind, term) = if let Some(stripped) = q.strip_prefix('*') {
            (Some("server"), stripped.trim())
        } else if let Some(stripped) = q.strip_prefix('#') {
            (Some("channel"), stripped.trim())
        } else if let Some(stripped) = q.strip_prefix('@') {
            (Some("user"), stripped.trim())
        } else if let Some(stripped) = q.strip_prefix('!') {
            (Some("channel"), stripped.trim())
        } else {
            (None, q)
        };

        let mut results = Vec::new();

        if term.is_empty() {
            if let Some(kind) = filter_kind {
                let mut stmt = self.conn.prepare(
                    "SELECT id, kind, name, subtitle FROM quick_switcher_fts WHERE kind = ?1 LIMIT 50",
                )?;
                let rows = stmt.query_map(params![kind], |row| {
                    let id: String = row.get(0)?;
                    let kind_str: String = row.get(1)?;
                    let name: String = row.get(2)?;
                    let subtitle: String = row.get(3)?;
                    let item_kind = match kind_str.as_str() {
                        "server" => QuickSwitcherItemKind::Server,
                        "channel" => QuickSwitcherItemKind::Channel,
                        _ => QuickSwitcherItemKind::User,
                    };
                    Ok(QuickSwitcherItem {
                        id,
                        kind: item_kind,
                        name,
                        subtitle,
                        extra_id: None,
                    })
                })?;
                for r in rows {
                    results.push(r?);
                }
            } else {
                let mut stmt = self
                    .conn
                    .prepare("SELECT id, kind, name, subtitle FROM quick_switcher_fts LIMIT 50")?;
                let rows = stmt.query_map([], |row| {
                    let id: String = row.get(0)?;
                    let kind_str: String = row.get(1)?;
                    let name: String = row.get(2)?;
                    let subtitle: String = row.get(3)?;
                    let item_kind = match kind_str.as_str() {
                        "server" => QuickSwitcherItemKind::Server,
                        "channel" => QuickSwitcherItemKind::Channel,
                        _ => QuickSwitcherItemKind::User,
                    };
                    Ok(QuickSwitcherItem {
                        id,
                        kind: item_kind,
                        name,
                        subtitle,
                        extra_id: None,
                    })
                })?;
                for r in rows {
                    results.push(r?);
                }
            }
            return Ok(results);
        }

        // Выполняем FTS5 MATCH запрос с fallback на LIKE
        let sanitized = term.replace('"', "").replace('\'', "").replace('*', "");
        let fts_query = format!("{}*", sanitized);

        if let Some(kind) = filter_kind {
            if let Ok(mut stmt) = self.conn.prepare(
                "SELECT id, kind, name, subtitle FROM quick_switcher_fts WHERE quick_switcher_fts MATCH ?1 AND kind = ?2 ORDER BY rank LIMIT 50",
            ) {
                if let Ok(rows) = stmt.query_map(params![fts_query, kind], |row| {
                    let id: String = row.get(0)?;
                    let kind_str: String = row.get(1)?;
                    let name: String = row.get(2)?;
                    let subtitle: String = row.get(3)?;
                    let item_kind = match kind_str.as_str() {
                        "server" => QuickSwitcherItemKind::Server,
                        "channel" => QuickSwitcherItemKind::Channel,
                        _ => QuickSwitcherItemKind::User,
                    };
                    Ok(QuickSwitcherItem {
                        id,
                        kind: item_kind,
                        name,
                        subtitle,
                        extra_id: None,
                    })
                }) {
                    for r in rows {
                        if let Ok(item) = r {
                            results.push(item);
                        }
                    }
                }
            }

            if results.is_empty() {
                let pattern = format!("%{}%", term);
                let mut stmt = self.conn.prepare(
                    "SELECT id, kind, name, subtitle FROM quick_switcher_fts WHERE (name LIKE ?1 OR subtitle LIKE ?1) AND kind = ?2 LIMIT 50",
                )?;
                let rows = stmt.query_map(params![pattern, kind], |row| {
                    let id: String = row.get(0)?;
                    let kind_str: String = row.get(1)?;
                    let name: String = row.get(2)?;
                    let subtitle: String = row.get(3)?;
                    let item_kind = match kind_str.as_str() {
                        "server" => QuickSwitcherItemKind::Server,
                        "channel" => QuickSwitcherItemKind::Channel,
                        _ => QuickSwitcherItemKind::User,
                    };
                    Ok(QuickSwitcherItem {
                        id,
                        kind: item_kind,
                        name,
                        subtitle,
                        extra_id: None,
                    })
                })?;
                for r in rows {
                    results.push(r?);
                }
            }
        } else {
            if let Ok(mut stmt) = self.conn.prepare(
                "SELECT id, kind, name, subtitle FROM quick_switcher_fts WHERE quick_switcher_fts MATCH ?1 ORDER BY rank LIMIT 50",
            ) {
                if let Ok(rows) = stmt.query_map(params![fts_query], |row| {
                    let id: String = row.get(0)?;
                    let kind_str: String = row.get(1)?;
                    let name: String = row.get(2)?;
                    let subtitle: String = row.get(3)?;
                    let item_kind = match kind_str.as_str() {
                        "server" => QuickSwitcherItemKind::Server,
                        "channel" => QuickSwitcherItemKind::Channel,
                        _ => QuickSwitcherItemKind::User,
                    };
                    Ok(QuickSwitcherItem {
                        id,
                        kind: item_kind,
                        name,
                        subtitle,
                        extra_id: None,
                    })
                }) {
                    for r in rows {
                        if let Ok(item) = r {
                            results.push(item);
                        }
                    }
                }
            }

            if results.is_empty() {
                let pattern = format!("%{}%", term);
                let mut stmt = self.conn.prepare(
                    "SELECT id, kind, name, subtitle FROM quick_switcher_fts WHERE name LIKE ?1 OR subtitle LIKE ?1 LIMIT 50",
                )?;
                let rows = stmt.query_map(params![pattern], |row| {
                    let id: String = row.get(0)?;
                    let kind_str: String = row.get(1)?;
                    let name: String = row.get(2)?;
                    let subtitle: String = row.get(3)?;
                    let item_kind = match kind_str.as_str() {
                        "server" => QuickSwitcherItemKind::Server,
                        "channel" => QuickSwitcherItemKind::Channel,
                        _ => QuickSwitcherItemKind::User,
                    };
                    Ok(QuickSwitcherItem {
                        id,
                        kind: item_kind,
                        name,
                        subtitle,
                        extra_id: None,
                    })
                })?;
                for r in rows {
                    results.push(r?);
                }
            }
        }

        Ok(results)
    }
}
