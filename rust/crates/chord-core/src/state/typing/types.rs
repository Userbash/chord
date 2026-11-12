//! P2-11a: типы memory-only typing (RW-20 / W20).
//!
//! Контракт заморожен `waves/RW-20/DESIGN-P2-11.md` §3. TTL — константа, системные
//! часы в reducer не читаются: время приходит извне через `TypingStart.received_at_ms`.
//! В `TypingEntry` нет `User`, поэтому `State::Debug` не печатает member payload.

use crate::ready::User;

/// TTL typing-индикатора в миллисекундах (официальный `TypingStore`: `10 * Millis.SECOND`).
pub const TYPING_TTL_MS: u64 = 10_000;

/// Ключ typing-store: `(channel_id, user_id)`.
pub type TypingKey = (u64, u64);

/// `TYPING_START` после разбора.
#[derive(Debug, Clone, PartialEq)]
pub struct TypingStart {
    /// Канал.
    pub channel_id: u64,
    /// Пользователь.
    pub user_id: u64,
    /// Гильдия (для ЛС/приватного канала — `None`).
    pub guild_id: Option<u64>,
    /// Проводной timestamp в секундах; только для диагностики, не для TTL.
    pub wire_timestamp: Option<i64>,
    /// `member.user`, если пришёл и совпал с `user_id`.
    pub member_user: Option<User>,
    /// Момент получения события вызывающим, мс; единственный источник TTL.
    pub received_at_ms: Option<u64>,
}

/// Запись typing-store: только id/время, без member payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypingEntry {
    /// Канал.
    pub channel_id: u64,
    /// Пользователь.
    pub user_id: u64,
    /// Гильдия.
    pub guild_id: Option<u64>,
    /// Проводной timestamp (секунды).
    pub wire_timestamp: Option<i64>,
    /// Начало TTL, мс.
    pub started_at_ms: u64,
    /// Конец TTL (`started_at_ms + TYPING_TTL_MS`, saturating), мс.
    pub expires_at_ms: u64,
}

impl TypingEntry {
    /// Активна ли запись: `now_ms < expires_at_ms` (граница — уже неактивна).
    #[must_use]
    pub fn is_active(&self, now_ms: u64) -> bool {
        now_ms < self.expires_at_ms
    }

    /// Остаток TTL; для истёкшей записи — `0`, без underflow.
    #[must_use]
    pub fn remaining_ms(&self, now_ms: u64) -> u64 {
        self.expires_at_ms.saturating_sub(now_ms)
    }
}
