//! P2-07c: селекторы mentions и бейджей (RW-17 / W17c).
//!
//! Все выборки — только чтение и без паники; контракт заморожен
//! `waves/RW-17/DESIGN-P2-07.md` §5. Отсутствие записи эквивалентно нулю, отрицательные
//! значения clamp-ятся в 0. Порядок `mention_channels`/`badge_entries` — по возрастанию id,
//! независимо от порядка провода. Гильдия учитывает индексы `channel_guild` и `thread_guild`;
//! `private_mention_count` — только id из `private_channels`.

use super::types::State;

impl State {
    /// Число упоминаний канала/записи; отсутствие и отрицательные — 0.
    #[must_use]
    pub fn mention_count(&self, id: u64) -> i64 {
        non_negative(
            self.read_state_entry(id)
                .and_then(|entry| entry.mention_count),
        )
    }

    /// Есть ли у канала/записи непрочитанные упоминания.
    #[must_use]
    pub fn has_mentions(&self, id: u64) -> bool {
        self.mention_count(id) > 0
    }

    /// Id записей с положительным `mention_count`, по возрастанию.
    #[must_use]
    pub fn mention_channels(&self) -> Vec<u64> {
        sorted_positive(
            self.read_state
                .entries
                .iter()
                .map(|entry| (entry.id, entry.mention_count)),
        )
    }

    /// Сумма положительных `mention_count` всех записей.
    #[must_use]
    pub fn total_mentions(&self) -> i64 {
        sum_clamped(
            self.read_state
                .entries
                .iter()
                .map(|entry| entry.mention_count),
        )
    }

    /// Сумма упоминаний каналов и тредов гильдии; ЛС, неизвестные и чужие — не входят.
    #[must_use]
    pub fn guild_mention_count(&self, guild_id: u64) -> i64 {
        sum_clamped(
            self.read_state
                .entries
                .iter()
                .filter(|entry| self.entry_guild(entry.id) == Some(guild_id))
                .map(|entry| entry.mention_count),
        )
    }

    /// Сумма упоминаний только приватных каналов (`private_channels`).
    #[must_use]
    pub fn private_mention_count(&self) -> i64 {
        sum_clamped(
            self.read_state
                .entries
                .iter()
                .filter(|entry| self.private_channels.contains_key(&entry.id))
                .map(|entry| entry.mention_count),
        )
    }

    /// Число бейджей записи; отсутствие и отрицательные — 0.
    #[must_use]
    pub fn badge_count(&self, id: u64) -> i64 {
        non_negative(
            self.read_state_entry(id)
                .and_then(|entry| entry.badge_count),
        )
    }

    /// Id записей с положительным `badge_count`, по возрастанию.
    #[must_use]
    pub fn badge_entries(&self) -> Vec<u64> {
        sorted_positive(
            self.read_state
                .entries
                .iter()
                .map(|entry| (entry.id, entry.badge_count)),
        )
    }

    /// Сумма положительных `badge_count` всех записей.
    #[must_use]
    pub fn total_badges(&self) -> i64 {
        sum_clamped(
            self.read_state
                .entries
                .iter()
                .map(|entry| entry.badge_count),
        )
    }

    /// Ack-точка записи: для канала (`read_state_type` `None`/0) — только
    /// `last_message_id`; для non-channel — `last_acked_id` с fallback на
    /// `last_message_id`. Неизвестная запись — `None`.
    #[must_use]
    pub fn acknowledged_message_id(&self, id: u64) -> Option<u64> {
        let entry = self.read_state_entry(id)?;
        match entry.read_state_type {
            None | Some(0) => entry.last_message_id,
            Some(_) => entry.last_acked_id.or(entry.last_message_id),
        }
    }

    /// Прочитано ли сообщение: ack-точка записи `>= message_id`; неизвестная запись — `false`.
    #[must_use]
    pub fn is_message_read(&self, id: u64, message_id: u64) -> bool {
        self.acknowledged_message_id(id)
            .is_some_and(|acked| acked >= message_id)
    }

    /// Глобальная версия read-state.
    #[must_use]
    pub fn read_state_version(&self) -> u64 {
        self.read_state.version
    }

    /// Гильдия записи: сначала каналы, затем треды; `None` для ЛС/неизвестных.
    fn entry_guild(&self, id: u64) -> Option<u64> {
        self.channel_guild
            .get(&id)
            .or_else(|| self.thread_guild.get(&id))
            .copied()
    }
}

/// Отсутствие — 0, отрицательное — 0.
fn non_negative(value: Option<i64>) -> i64 {
    value.unwrap_or(0).max(0)
}

/// Id с положительным счётчиком: возрастание, без дубликатов.
fn sorted_positive(entries: impl Iterator<Item = (u64, Option<i64>)>) -> Vec<u64> {
    let mut ids: Vec<u64> = entries
        .filter_map(|(id, count)| (non_negative(count) > 0).then_some(id))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// Сумма clamp-нутых счётчиков без переполнения.
fn sum_clamped(counts: impl Iterator<Item = Option<i64>>) -> i64 {
    counts.fold(0i64, |total, count| {
        total.saturating_add(non_negative(count))
    })
}
