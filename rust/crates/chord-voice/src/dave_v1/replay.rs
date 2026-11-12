//! Скользящее битовое окно `AntiReplayWindow` и арифметика поколений/счётчиков DAVE v1.

use std::fmt;

/// Размер скользящего битового окна защиты от повторов (`16 * 64 = 1024` пакета).
pub const DAVE_REPLAY_WINDOW_SIZE: u64 = 1024;
/// Сдвиг 8-битного номера поколения внутри 32-битного `truncated_nonce` (`truncated_nonce >> 24`).
pub const DAVE_RATCHET_GENERATION_SHIFT_BITS: u32 = 24;
/// Период оборачивания младших 8 бит поколения (`256`).
pub const DAVE_GENERATION_WRAP: u32 = 256;

/// Ошибка проверки монотонности и уникальности 64-битного счётчика в `AntiReplayWindow`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayError {
    /// Пакет с таким `nonce` уже был принят ранее.
    DuplicateNonce(u64),
    /// Пакет отстал от вершины окна более чем на `DAVE_REPLAY_WINDOW_SIZE - 1`.
    ExpiredNonce { nonce: u64, oldest_allowed: u64 },
}

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateNonce(n) => write!(f, "повтор пакета с nonce {n}"),
            Self::ExpiredNonce {
                nonce,
                oldest_allowed,
            } => write!(
                f,
                "устаревший nonce {nonce} (минимально допустимый {oldest_allowed})"
            ),
        }
    }
}

impl std::error::Error for ReplayError {}

/// Скользящее битовое окно защиты от повторов на 1024 позиции без аллокаций в куче.
///
/// Бит `offset = highest_nonce - nonce` хранится в слове `words[offset / 64]` в позиции
/// `offset % 64`. Арифметика полностью устойчива к переполнению около `0` и `u64::MAX`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AntiReplayWindow {
    words: [u64; 16],
    highest_nonce: Option<u64>,
}

impl Default for AntiReplayWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl AntiReplayWindow {
    /// Создаёт пустое окно защиты от повторов.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            words: [0u64; 16],
            highest_nonce: None,
        }
    }

    /// Возвращает максимальный принятый `nonce`, если хотя бы один пакет прошёл проверку.
    #[must_use]
    pub const fn highest_nonce(&self) -> Option<u64> {
        self.highest_nonce
    }

    /// Проверяет, допустим ли `nonce`, **не** изменяя состояние окна (вызывается до AEAD).
    #[must_use]
    pub fn can_accept(&self, nonce: u64) -> bool {
        let Some(highest) = self.highest_nonce else {
            return true;
        };
        if nonce > highest {
            return true;
        }
        let offset = highest - nonce;
        if offset >= DAVE_REPLAY_WINDOW_SIZE {
            return false;
        }
        let word_idx = (offset / 64) as usize;
        let bit_idx = (offset % 64) as u32;
        ((self.words[word_idx] >> bit_idx) & 1) == 0
    }

    /// Регистрирует `nonce` в скользящем окне или отклоняет повтор/просроченный счётчик.
    pub fn accept(&mut self, nonce: u64) -> Result<(), ReplayError> {
        let Some(highest) = self.highest_nonce else {
            self.words[0] = 1;
            self.highest_nonce = Some(nonce);
            return Ok(());
        };

        if nonce > highest {
            let delta = nonce - highest;
            if delta >= DAVE_REPLAY_WINDOW_SIZE {
                self.words = [0u64; 16];
                self.words[0] = 1;
            } else {
                let word_shift = (delta / 64) as usize;
                let bit_shift = (delta % 64) as u32;
                let mut next = [0u64; 16];
                for (src_w, &val) in self.words.iter().enumerate() {
                    if val == 0 {
                        continue;
                    }
                    let dst_w = src_w + word_shift;
                    if dst_w < 16 {
                        next[dst_w] |= val << bit_shift;
                    }
                    if bit_shift > 0 && dst_w + 1 < 16 {
                        next[dst_w + 1] |= val >> (64 - bit_shift);
                    }
                }
                next[0] |= 1;
                self.words = next;
            }
            self.highest_nonce = Some(nonce);
            return Ok(());
        }

        let offset = highest - nonce;
        if offset >= DAVE_REPLAY_WINDOW_SIZE {
            return Err(ReplayError::ExpiredNonce {
                nonce,
                oldest_allowed: highest.saturating_sub(DAVE_REPLAY_WINDOW_SIZE - 1),
            });
        }

        let word_idx = (offset / 64) as usize;
        let bit_idx = (offset % 64) as u32;
        let mask = 1u64 << bit_idx;
        if (self.words[word_idx] & mask) != 0 {
            return Err(ReplayError::DuplicateNonce(nonce));
        }
        self.words[word_idx] |= mask;
        Ok(())
    }
}

/// Восстанавливает 32-битное поколение из младших 8 бит относительно `oldest` без переполнения `u32`.
#[must_use]
pub const fn dave_compute_wrapped_generation(oldest: u32, generation_u8: u32) -> u32 {
    let delta = ((generation_u8 & 0xFF) as u8).wrapping_sub((oldest & 0xFF) as u8);
    oldest.saturating_add(delta as u32)
}

/// Формирует 64-битный монотонный счётчик пакета из поколения и 24 младших бит `truncated_nonce`.
#[must_use]
pub const fn dave_compute_wrapped_big_nonce(generation: u32, truncated_nonce: u32) -> u64 {
    let low_mask = (1u64 << DAVE_RATCHET_GENERATION_SHIFT_BITS) - 1;
    ((generation as u64) << DAVE_RATCHET_GENERATION_SHIFT_BITS)
        | ((truncated_nonce as u64) & low_mask)
}
