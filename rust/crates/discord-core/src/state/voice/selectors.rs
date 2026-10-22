//! P2-10: селекторы voice states (RW-19 / W19).
//!
//! Контракт заморожен `waves/RW-19/DESIGN-P2-10.md` §6. Только чтение, без паник:
//! отсутствующий ключ даёт `None`, отрицательные/переполняющие timestamps — `None`,
//! вычитание времени насыщается нулём. Таймеры не заводят потоков — `now_ms`
//! передаёт вызывающий.

use std::collections::BTreeSet;

use crate::state::types::State;

use super::types::VoiceState;

impl State {
    /// Voice-запись пользователя в гильдии.
    #[must_use]
    pub fn voice_state(&self, guild_id: u64, user_id: u64) -> Option<&VoiceState> {
        self.voice_states.get(&(guild_id, user_id))
    }

    /// Канал пользователя, если он в голосе.
    #[must_use]
    pub fn voice_channel_of(&self, guild_id: u64, user_id: u64) -> Option<u64> {
        self.voice_state(guild_id, user_id)
            .and_then(|voice| voice.channel_id)
    }

    /// Пользователи канала по возрастанию `user_id`.
    #[must_use]
    pub fn voice_users_in_channel(&self, guild_id: u64, channel_id: u64) -> Vec<&VoiceState> {
        self.voice_states
            .values()
            .filter(|voice| voice.guild_id == guild_id && voice.channel_id == Some(channel_id))
            .collect()
    }

    /// Число пользователей канала.
    #[must_use]
    pub fn voice_channel_user_count(&self, guild_id: u64, channel_id: u64) -> usize {
        self.voice_states
            .values()
            .filter(|voice| voice.guild_id == guild_id && voice.channel_id == Some(channel_id))
            .count()
    }

    /// Число voice-записей.
    #[must_use]
    pub fn voice_state_count(&self) -> usize {
        self.voice_states.len()
    }

    /// Число уникальных гильдий в voice-store.
    #[must_use]
    pub fn voice_guild_count(&self) -> usize {
        self.voice_states
            .keys()
            .map(|(guild_id, _)| *guild_id)
            .collect::<BTreeSet<_>>()
            .len()
    }

    /// Пользователь находится в голосе.
    #[must_use]
    pub fn is_user_in_voice(&self, guild_id: u64, user_id: u64) -> bool {
        self.voice_state(guild_id, user_id)
            .is_some_and(VoiceState::is_connected)
    }

    /// Первая voice-запись собственного пользователя в порядке `(guild,user)`.
    #[must_use]
    pub fn self_voice_state(&self) -> Option<&VoiceState> {
        self.voice_states
            .values()
            .find(|voice| voice.user_id == self.user.id)
    }

    /// Собственный пользователь находится в голосе.
    #[must_use]
    pub fn is_self_in_voice(&self) -> bool {
        self.self_voice_state()
            .is_some_and(VoiceState::is_connected)
    }

    /// Сколько миллисекунд пользователь в голосе; `connected_at` — секунды.
    ///
    /// Отрицательное значение и переполнение умножения дают `None`; вычитание
    /// насыщается нулём, если `now_ms` раньше старта.
    #[must_use]
    pub fn voice_connected_for_ms(&self, guild_id: u64, user_id: u64, now_ms: u64) -> Option<u64> {
        let start_ms = self.voice_state(guild_id, user_id)?.connected_at_ms()?;
        Some(now_ms.saturating_sub(start_ms))
    }

    /// Стартовое время канала (секунды).
    #[must_use]
    pub fn voice_channel_start_time(&self, guild_id: u64, channel_id: u64) -> Option<i64> {
        self.voice_channel_start_times
            .get(&(guild_id, channel_id))
            .copied()
    }

    /// Сколько миллисекунд таймер канала идёт; `voice_start_time` — секунды.
    #[must_use]
    pub fn voice_channel_elapsed_ms(
        &self,
        guild_id: u64,
        channel_id: u64,
        now_ms: u64,
    ) -> Option<u64> {
        let start = self.voice_channel_start_time(guild_id, channel_id)?;
        if start < 0 {
            return None;
        }
        let start_ms = u64::try_from(start).ok()?.checked_mul(1000)?;
        Some(now_ms.saturating_sub(start_ms))
    }

    /// Статус канала; пустая строка всегда `None`.
    #[must_use]
    pub fn voice_channel_status(&self, guild_id: u64, channel_id: u64) -> Option<&str> {
        self.voice_channel_statuses
            .get(&(guild_id, channel_id))
            .map(String::as_str)
            .filter(|status| !status.is_empty())
    }
}
