//! P2-06: memory-only история сообщений и пагинация (RW-14).
//!
//! Ограниченное окно на канал, слияние live-событий и уже типизированных страниц,
//! generation-токены и read-only селекторы. Без REST-JSON, сети, SQLite, UI и новых
//! зависимостей. Контракт — `waves/RW-14/DESIGN-P2-06.md`.

pub mod pagination;
pub mod selectors;
pub mod types;

pub use pagination::{Cursor, MessagePage, PageApply, PageError, PageRequest, PageToken};
pub use types::{History, HistoryConfig, HistoryDiagnostics, HistoryError, LiveOutcome};
