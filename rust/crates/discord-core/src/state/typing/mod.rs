//! P2-11: memory-only typing TTL (RW-20 / W20).
//!
//! Контракт заморожен `waves/RW-20/DESIGN-P2-11.md` §3–§7. `TYPING_START` хранится
//! по `(channel_id, user_id)` с TTL 10 000 мс; время передаёт вызывающий через
//! `TypingStart.received_at_ms` / `now_ms`. Исходящего typing, `TYPING_STOP`,
//! `TYPING_START_LOCAL`, unread-count и `typing_indicator_style` в модели нет.

pub mod parser;
pub mod reducer;
pub mod selectors;
pub mod types;

pub use types::{TYPING_TTL_MS, TypingEntry, TypingKey, TypingStart};
