//! Политика reconnect/resume для сессии шлюза Discord (P4-12).
//!
//! После разрыва сокета клиент сначала пробует RESUME — сессия и `s` продолжаются;
//! если сервер ответил `op9 INVALID_SESSION`, нужен свежий IDENTIFY. В захвате
//! (`docs/re/gateway-protocol.md` §4) виден один `op7 RECONNECT` (после `s=331`) и три
//! `RESUME → RESUMED` с продолжением `s` (95, 664, 332); `op9` не наблюдался.
//! Задержки переподключений в захвате — секунды (~2.9 с), но точные клиентские значения
//! Discord не измерены (jitter не наблюдался), поэтому бэкофф детерминированный.
//!
//! Модуль чистый: он возвращает решение и задержку, а таймер и I/O остаются снаружи.
//! Случайности, времени, паник и `unsafe` здесь нет.

/// Детерминированный бэкофф перед попыткой переподключения.
pub const BACKOFF_MS: [u64; 6] = [1_000, 2_000, 4_000, 8_000, 16_000, 30_000];

/// Задержка перед IDENTIFY после `op9 INVALID_SESSION`.
pub const INVALID_SESSION_DELAY_MS: u64 = 1_000;

/// Решение по текущему разрыву соединения.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconnectAction {
    /// Продолжить сессию (RESUME): `s` не разрывается.
    Resume,
    /// Открыть новую сессию (IDENTIFY), например после `op9 INVALID_SESSION`.
    Identify,
    /// Лимит попыток исчерпан — дальше переподключаться не пытаемся.
    GiveUp,
}

/// Один шаг политики: что делать и через сколько.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconnectPlan {
    /// Действие, которое нужно выполнить.
    pub action: ReconnectAction,
    /// Задержка перед действием; для [`ReconnectAction::GiveUp`] всегда 0.
    pub delay_ms: u64,
    /// Номер попытки после учёта события (0 — соединение ещё не рвалось).
    pub attempt: u32,
}

/// Детерминированная политика переподключения с лимитом попыток.
#[derive(Debug, Clone)]
pub struct ReconnectPolicy {
    attempt: u32,
    max_attempts: u32,
}

impl ReconnectPolicy {
    /// Создать политику; `max_attempts == 0` трактуется как одна попытка.
    #[must_use]
    pub fn new(max_attempts: u32) -> Self {
        Self {
            attempt: 0,
            max_attempts: max_attempts.max(1),
        }
    }

    /// Сколько попыток израсходовано с последнего [`Self::on_established`].
    #[must_use]
    pub fn attempts(&self) -> u32 {
        self.attempt
    }

    /// Разрыв сокета; `resumable` показывает, можно ли продолжить текущую сессию.
    ///
    /// При исчерпанном лимите возвращает [`ReconnectAction::GiveUp`] с нулевой задержкой
    /// и не наращивает счётчик.
    #[must_use]
    pub fn on_disconnect(&mut self, resumable: bool) -> ReconnectPlan {
        let action = if resumable {
            ReconnectAction::Resume
        } else {
            ReconnectAction::Identify
        };
        self.advance(action, backoff_ms(self.attempt))
    }

    /// `op9 INVALID_SESSION`: сессия не продолжается, нужен IDENTIFY с фиксированной задержкой.
    ///
    /// Лимит попыток общий с [`Self::on_disconnect`].
    #[must_use]
    pub fn on_invalid_session(&mut self) -> ReconnectPlan {
        self.advance(ReconnectAction::Identify, INVALID_SESSION_DELAY_MS)
    }

    /// READY/RESUMED: соединение установлено, счётчик попыток сбрасывается.
    ///
    /// Сброс после [`ReconnectAction::GiveUp`] тоже возвращает политику в работу.
    pub fn on_established(&mut self) {
        self.attempt = 0;
    }

    /// Общий шаг: проверка лимита, задержка, насыщающий инкремент счётчика.
    fn advance(&mut self, action: ReconnectAction, delay_ms: u64) -> ReconnectPlan {
        if self.attempt >= self.max_attempts {
            return ReconnectPlan {
                action: ReconnectAction::GiveUp,
                delay_ms: 0,
                attempt: self.attempt,
            };
        }
        // Насыщение на `u32::MAX`: переполнения нет даже при неограниченном лимите.
        self.attempt = self.attempt.saturating_add(1);
        ReconnectPlan {
            action,
            delay_ms,
            attempt: self.attempt,
        }
    }
}

/// Задержка для `attempt`-й попытки (0-based) с потолком на последнем элементе таблицы.
fn backoff_ms(attempt: u32) -> u64 {
    let last = BACKOFF_MS.len() - 1;
    let index = usize::try_from(attempt).map_or(last, |index| index.min(last));
    BACKOFF_MS[index]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attempt_saturates_at_u32_max_without_overflow() {
        let mut policy = ReconnectPolicy {
            attempt: u32::MAX - 1,
            max_attempts: u32::MAX,
        };
        let step = policy.on_disconnect(true);
        assert_eq!(
            step,
            ReconnectPlan {
                action: ReconnectAction::Resume,
                delay_ms: 30_000,
                attempt: u32::MAX,
            }
        );
        assert_eq!(policy.attempts(), u32::MAX);
        // На пределе лимит уже исчерпан — GiveUp без переполнения счётчика.
        let step = policy.on_disconnect(true);
        assert_eq!(step.action, ReconnectAction::GiveUp);
        assert_eq!(step.delay_ms, 0);
        assert_eq!(step.attempt, u32::MAX);
        assert_eq!(policy.attempts(), u32::MAX);
    }

    #[test]
    fn backoff_index_is_clamped_to_last_entry() {
        assert_eq!(backoff_ms(0), 1_000);
        assert_eq!(backoff_ms(5), 30_000);
        assert_eq!(backoff_ms(6), 30_000);
        assert_eq!(backoff_ms(u32::MAX), 30_000);
    }
}
