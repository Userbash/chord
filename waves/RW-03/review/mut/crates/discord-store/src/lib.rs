//! # discord-store
//!
//! Локальное хранилище клиента (SQLite): гильдии, каналы, роли, пользователи, эмодзи;
//! офлайн-старт и быстрые сортировки/папки.
//!
//! Спека: `docs/rust-client/FEATURES.md` F-DATA-02 (`docs/feature-backlog.md`, D-серия).
//! План: P2; тесты: T-STORE-* (`docs/rust-client/TEST-PLAN.md`).
//!
//! Статус: каркас P0 — состав таблиц из RE-анализа; миграции появятся в P2.

/// Стартовый набор таблиц ядра данных (по F-DATA-02).
pub const CORE_TABLES: [&str; 5] = ["guilds", "channels", "roles", "users", "emoji"];

/// Версия схемы хранилища (миграции — P2-04 плана).
pub const SCHEMA_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    #[test]
    fn core_tables_are_set() {
        assert_eq!(super::CORE_TABLES.len(), 5);
        assert!(super::CORE_TABLES.contains(&"guilds"));
        assert!(super::CORE_TABLES.contains(&"channels"));
    }
}
