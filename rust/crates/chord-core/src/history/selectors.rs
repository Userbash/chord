//! P2-06c: read-only селекторы истории (RW-14 / W14c).
//!
//! Контракт — `waves/RW-14/DESIGN-P2-06.md` §6. Все селекторы только читают:
//! неизвестный канал даёт пустой срез/`None`, канал не создаётся, окно не меняется.
//! Порядок в `messages`/`range`/`window_around` — возрастание id, в `recent` — новые первыми.

use crate::state::Message;

use super::types::History;

impl History {
    /// Число каналов с историей (включая каналы только с известными границами).
    #[must_use]
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    /// Суммарное число сообщений во всех каналах.
    #[must_use]
    pub fn total_len(&self) -> usize {
        self.channels
            .values()
            .map(|channel| channel.messages.len())
            .sum()
    }

    /// Число сообщений в канале; неизвестный канал — `0`.
    #[must_use]
    pub fn len(&self, channel_id: u64) -> usize {
        self.channel(channel_id)
            .map_or(0, |channel| channel.messages.len())
    }

    /// Пуст ли канал; неизвестный канал — `true`.
    #[must_use]
    pub fn is_empty(&self, channel_id: u64) -> bool {
        self.len(channel_id) == 0
    }

    /// Сообщения канала по возрастанию id.
    #[must_use]
    pub fn messages(&self, channel_id: u64) -> Vec<&Message> {
        let Some(channel) = self.channel(channel_id) else {
            return Vec::new();
        };
        channel.messages.values().collect()
    }

    /// До `limit` самых новых сообщений, новые первыми; `limit == 0` — пусто.
    #[must_use]
    pub fn recent(&self, channel_id: u64, limit: usize) -> Vec<&Message> {
        let Some(channel) = self.channel(channel_id) else {
            return Vec::new();
        };
        channel.messages.values().rev().take(limit).collect()
    }

    /// Сообщение по id в пределах указанного канала.
    #[must_use]
    pub fn message(&self, channel_id: u64, message_id: u64) -> Option<&Message> {
        self.channel(channel_id)?.messages.get(&message_id)
    }

    /// Самое старое сообщение канала.
    #[must_use]
    pub fn oldest(&self, channel_id: u64) -> Option<&Message> {
        self.channel(channel_id)?.messages.values().next()
    }

    /// Самое новое сообщение канала.
    #[must_use]
    pub fn newest(&self, channel_id: u64) -> Option<&Message> {
        self.channel(channel_id)?.messages.values().next_back()
    }

    /// Известно ли, что есть более старые сообщения; `None` — неизвестно.
    #[must_use]
    pub fn has_older(&self, channel_id: u64) -> Option<bool> {
        self.channel(channel_id)?.has_older
    }

    /// Известно ли, что есть более новые сообщения; `None` — неизвестно.
    #[must_use]
    pub fn has_newer(&self, channel_id: u64) -> Option<bool> {
        self.channel(channel_id)?.has_newer
    }

    /// Сообщения с `from <= id <= to` по возрастанию; `from > to` — пусто.
    #[must_use]
    pub fn range(&self, channel_id: u64, from: u64, to: u64) -> Vec<&Message> {
        if from > to {
            return Vec::new();
        }
        let Some(channel) = self.channel(channel_id) else {
            return Vec::new();
        };
        channel
            .messages
            .range(from..=to)
            .map(|(_, message)| message)
            .collect()
    }

    /// До `limit` сообщений, ближайших к `anchor`, по возрастанию id.
    ///
    /// Близость — `(abs_diff(id, anchor), id)`: при равном расстоянии выигрывает меньший id.
    /// Anchor может отсутствовать в окне. `limit == 0` — пусто.
    #[must_use]
    pub fn window_around(&self, channel_id: u64, anchor: u64, limit: usize) -> Vec<&Message> {
        if limit == 0 {
            return Vec::new();
        }
        let Some(channel) = self.channel(channel_id) else {
            return Vec::new();
        };
        let mut messages: Vec<&Message> = channel.messages.values().collect();
        messages.sort_by_key(|message| (message.id.abs_diff(anchor), message.id));
        messages.truncate(limit);
        messages.sort_by_key(|message| message.id);
        messages
    }
}
