//! P2-11b: reducer typing-store (RW-20 / W20).
//!
//! Контракт заморожен `waves/RW-20/DESIGN-P2-11.md` §5. Store memory-only по
//! `(channel_id, user_id)`; повторный START перезапускает TTL, `MESSAGE_CREATE`
//! удаляет пару автора, `READY` создаёт пустую карту, `prune_expired` — явная
//! очистка. Время приходит параметром, системные часы не читаются.

use crate::state::reducer::upsert_user;
use crate::state::types::State;

use super::types::{TYPING_TTL_MS, TypingEntry, TypingStart};

/// Применить `TYPING_START`.
///
/// Без `received_at_ms` запись не создаётся: диагностика `typing.received_at_ms`.
/// Повторная пара — last wins, без `duplicates`.
pub fn apply_start(state: &mut State, start: TypingStart) {
    let TypingStart {
        channel_id,
        user_id,
        guild_id,
        wire_timestamp,
        member_user,
        received_at_ms,
    } = start;

    let Some(now_ms) = received_at_ms else {
        state
            .diagnostics
            .ignored
            .push("typing.received_at_ms".to_string());
        return;
    };

    if let Some(user) = member_user
        && user.id == user_id
    {
        upsert_user(state, user);
    }

    state.typing.insert(
        (channel_id, user_id),
        TypingEntry {
            channel_id,
            user_id,
            guild_id,
            wire_timestamp,
            started_at_ms: now_ms,
            expires_at_ms: now_ms.saturating_add(TYPING_TTL_MS),
        },
    );
}

/// Удалить typing автора сообщения; `true`, если запись была.
#[must_use]
pub fn remove_for_message(state: &mut State, channel_id: u64, author_id: u64) -> bool {
    state.typing.remove(&(channel_id, author_id)).is_some()
}

/// Явно удалить запись пары; `true`, если запись была.
#[must_use]
pub fn remove_entry(state: &mut State, channel_id: u64, user_id: u64) -> bool {
    state.typing.remove(&(channel_id, user_id)).is_some()
}

/// Удалить все записи с `now_ms >= expires_at_ms`; вернуть число удалённых.
pub fn prune_expired(state: &mut State, now_ms: u64) -> usize {
    let before = state.typing.len();
    state.typing.retain(|_, entry| now_ms < entry.expires_at_ms);
    before - state.typing.len()
}
