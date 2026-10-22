//! P2-11c: селекторы typing-store (RW-20 / W20).
//!
//! Контракт заморожен `waves/RW-20/DESIGN-P2-11.md` §7. Только чтение, без паник.
//! Просроченные записи (`now_ms >= expires_at_ms`) скрываются, но не удаляются;
//! явная очистка — `reducer::prune_expired`.

use std::collections::BTreeSet;

use crate::ready::User;
use crate::state::types::State;

use super::types::TypingEntry;

impl State {
    /// Активная запись пары; просроченная/отсутствующая — `None`.
    #[must_use]
    pub fn typing_entry(&self, channel_id: u64, user_id: u64, now_ms: u64) -> Option<&TypingEntry> {
        self.typing
            .get(&(channel_id, user_id))
            .filter(|entry| entry.is_active(now_ms))
    }

    /// Печатает ли пользователь в канале на момент `now_ms`.
    #[must_use]
    pub fn is_typing(&self, channel_id: u64, user_id: u64, now_ms: u64) -> bool {
        self.typing_entry(channel_id, user_id, now_ms).is_some()
    }

    /// Активные записи канала по возрастанию `user_id`.
    #[must_use]
    pub fn typing_entries(&self, channel_id: u64, now_ms: u64) -> Vec<&TypingEntry> {
        let mut entries: Vec<&TypingEntry> = self
            .typing
            .values()
            .filter(|entry| entry.channel_id == channel_id && entry.is_active(now_ms))
            .collect();
        entries.sort_by_key(|entry| entry.user_id);
        entries
    }

    /// Id активных пользователей канала по возрастанию.
    #[must_use]
    pub fn typing_user_ids(&self, channel_id: u64, now_ms: u64) -> Vec<u64> {
        self.typing_entries(channel_id, now_ms)
            .into_iter()
            .map(|entry| entry.user_id)
            .collect()
    }

    /// Известные `State.users` среди активных пользователей канала, по возрастанию id.
    ///
    /// Неизвестные id не выдумываются: запись остаётся доступной через
    /// [`Self::typing_entries`], но `User` для неё не создаётся.
    #[must_use]
    pub fn typing_users(&self, channel_id: u64, now_ms: u64) -> Vec<&User> {
        self.typing_entries(channel_id, now_ms)
            .into_iter()
            .filter_map(|entry| self.users.get(&entry.user_id))
            .collect()
    }

    /// Активные записи точной гильдии, сортировка `(channel_id, user_id)`.
    #[must_use]
    pub fn typing_entries_by_guild(&self, guild_id: u64, now_ms: u64) -> Vec<&TypingEntry> {
        let mut entries: Vec<&TypingEntry> = self
            .typing
            .values()
            .filter(|entry| entry.guild_id == Some(guild_id) && entry.is_active(now_ms))
            .collect();
        entries.sort_by_key(|entry| (entry.channel_id, entry.user_id));
        entries
    }

    /// Число активных записей во всех каналах.
    #[must_use]
    pub fn typing_entry_count(&self, now_ms: u64) -> usize {
        self.typing
            .values()
            .filter(|entry| entry.is_active(now_ms))
            .count()
    }

    /// Число уникальных каналов с активными записями.
    #[must_use]
    pub fn typing_channel_count(&self, now_ms: u64) -> usize {
        self.typing
            .values()
            .filter(|entry| entry.is_active(now_ms))
            .map(|entry| entry.channel_id)
            .collect::<BTreeSet<_>>()
            .len()
    }

    /// Остаток TTL пары; `None`, если запись отсутствует или просрочена.
    #[must_use]
    pub fn typing_remaining_ms(&self, channel_id: u64, user_id: u64, now_ms: u64) -> Option<u64> {
        self.typing_entry(channel_id, user_id, now_ms)
            .map(|entry| entry.remaining_ms(now_ms))
    }
}
