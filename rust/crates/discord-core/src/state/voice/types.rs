//! P2-10: публичные типы memory-only voice states (RW-19 / W19).
//!
//! Контракт заморожен `waves/RW-19/DESIGN-P2-10.md` §3. `session_id` намеренно
//! отсутствует: он не нужен для отображения статуса и не сохраняется. Времена —
//! секунды; `connected_at` не заменяется текущим временем при отсутствии.

use crate::ready::User;

/// Ключ voice-записи: `(guild_id, user_id)`.
pub type VoiceKey = (u64, u64);

/// Ключ таймера/статуса канала: `(guild_id, channel_id)`.
pub type VoiceChannelKey = (u64, u64);

/// Voice-состояние пользователя в гильдии.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceState {
    /// Гильдия.
    pub guild_id: u64,
    /// Пользователь.
    pub user_id: u64,
    /// Голосовой канал; `None` — выход.
    pub channel_id: Option<u64>,
    /// Время входа (секунды).
    pub connected_at: Option<i64>,
    /// Серверный mute.
    pub mute: bool,
    /// Серверный deaf.
    pub deaf: bool,
    /// Self mute.
    pub self_mute: bool,
    /// Self deaf.
    pub self_deaf: bool,
    /// Видео включено.
    pub self_video: bool,
    /// Стрим включён.
    pub self_stream: bool,
    /// Подавлен.
    pub suppress: bool,
    /// Обнаруживаем (default `true`).
    pub discoverable: bool,
    /// Запрос слова (секунды).
    pub request_to_speak_timestamp: Option<i64>,
}

impl VoiceState {
    /// Пользователь находится в канале.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.channel_id.is_some()
    }

    /// Время входа в миллисекундах; `None` для отсутствия, отрицательного значения
    /// и переполнения `u64`.
    #[must_use]
    pub fn connected_at_ms(&self) -> Option<u64> {
        let seconds = u64::try_from(self.connected_at?).ok()?;
        seconds.checked_mul(1000)
    }
}

/// `VOICE_STATE_UPDATE` после разбора.
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceStateUpdate {
    /// Состояние.
    pub state: VoiceState,
    /// Пользователь из `member.user`, только если его id совпал с `state.user_id`.
    pub member_user: Option<User>,
}

/// `VOICE_CHANNEL_START_TIME_UPDATE` после разбора.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceChannelStartTime {
    /// Гильдия.
    pub guild_id: u64,
    /// Канал.
    pub channel_id: u64,
    /// Стартовое время (секунды); `None` — nil/очистка.
    pub voice_start_time: Option<i64>,
}

/// `VOICE_CHANNEL_STATUS_UPDATE` после разбора.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceChannelStatus {
    /// Гильдия.
    pub guild_id: u64,
    /// Канал.
    pub channel_id: u64,
    /// Статус; `None` — nil/очистка, пустая строка тоже очистка.
    pub status: Option<String>,
}
