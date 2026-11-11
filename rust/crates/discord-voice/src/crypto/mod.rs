//! Крипто-примитивы для SRTP/SRTCP и DAVE MLS v1 (карточки P4-03, TASK-903).
//!
//! Реализация без внешних зависимостей: AES-128 / AES-256 (FIPS-197), GCM и
//! AES-128-GCM с 8-байтным усечённым тегом DAVE v1 (NIST SP 800-38D),
//! ChaCha20/Poly1305 (RFC 8439), XChaCha20-Poly1305 (draft-irtf-cfrg-xchacha) и
//! HKDF-SHA1 (RFC 5869).
//!
//! Корректность примитивов подтверждена KAT-векторами NIST/RFC, вшитыми в
//! `tests/crypto_vectors.rs` как константы (значения сверены независимыми
//! реализациями, а не кодом крейта).
//!
//! Совместимость с wire Discord **не заявляется**: в capture движка нет ключевого
//! материала, поэтому формат SRTP-кадра самосогласован (см. модуль `srtp`).

pub mod aes;
pub mod chacha;
pub mod gcm;
pub mod kdf;

use std::fmt;

/// Размер блока AES в байтах.
pub const AES_BLOCK_SIZE: usize = 16;
/// Размер ключа AES-128 в байтах (DAVE MLS v1 media cipher).
pub const AES128_KEY_SIZE: usize = 16;
/// Размер ключа AES-256 в байтах.
pub const AES256_KEY_SIZE: usize = 32;
/// Размер nonce GCM в байтах (96 бит — рекомендованный NIST SP 800-38D размер).
pub const GCM_NONCE_SIZE: usize = 12;
/// Размер тега GCM в байтах.
pub const GCM_TAG_SIZE: usize = 16;
/// Размер усечённого 64-битного тега AES-128-GCM в кадровом шифре DAVE v1.
pub const GCM_TRUNCATED8_TAG_SIZE: usize = 8;
/// Размер ключа ChaCha20 в байтах.
pub const CHACHA_KEY_SIZE: usize = 32;
/// Размер nonce ChaCha20-Poly1305 в байтах.
pub const CHACHA_NONCE_SIZE: usize = 12;
/// Размер nonce XChaCha20-Poly1305 в байтах.
pub const XCHACHA_NONCE_SIZE: usize = 24;

/// Максимальное число блоков AEAD: счётчик CTR/ChaCha20 32-битный.
const MAX_BLOCKS: u64 = u32::MAX as u64;

/// Максимальная длина сообщения AES-GCM в байтах (блок 16 Б).
pub const GCM_MAX_MESSAGE_LEN: usize = max_message_len(16);
/// Максимальная длина сообщения ChaCha20-Poly1305 в байтах (блок 64 Б).
pub const CHACHA_MAX_MESSAGE_LEN: usize = max_message_len(64);

/// Предел длины сообщения по числу блоков (32-битный счётчик ⇒ 2^32-1 блоков).
const fn max_message_len(block_size: usize) -> usize {
    let max = MAX_BLOCKS.saturating_mul(block_size as u64);
    if max > usize::MAX as u64 {
        usize::MAX
    } else {
        max as usize
    }
}

/// Проверяет, что длина сообщения укладывается в 32-битный счётчик блоков.
///
/// Вызывается **до** аллокации буферов: проверяется только длина, данные не нужны.
pub(crate) fn check_message_len(
    len: usize,
    block_size: usize,
    limit: usize,
) -> Result<(), CryptoError> {
    let blocks = (len as u64).div_ceil(block_size as u64);
    if blocks > MAX_BLOCKS {
        return Err(CryptoError::MessageTooLong { limit });
    }
    Ok(())
}

/// Ошибка крипто-примитива.
///
/// Ключевой материал в вариантах ошибок не переносится: утечки через `Debug`
/// или `Display` нет.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CryptoError {
    /// Длина ключа не соответствует ожидаемой.
    InvalidKeyLength {
        /// Ожидаемая длина ключа в байтах.
        expected: usize,
        /// Фактическая длина ключа в байтах.
        actual: usize,
    },
    /// Длина nonce не соответствует ожидаемой.
    InvalidNonceLength {
        /// Ожидаемая длина nonce в байтах.
        expected: usize,
        /// Фактическая длина nonce в байтах.
        actual: usize,
    },
    /// Длина тега (или входа с тегом) не соответствует ожидаемой.
    InvalidTagLength {
        /// Ожидаемая длина тега в байтах.
        expected: usize,
        /// Фактическая длина в байтах.
        actual: usize,
    },
    /// Запрошенная длина вывода KDF вне допустимого диапазона.
    InvalidOutputLength,
    /// Сообщение длиннее, чем допускает 32-битный счётчик блоков AEAD.
    MessageTooLong {
        /// Предельная длина сообщения в байтах.
        limit: usize,
    },
    /// Проверка аутентификации (тега) не прошла.
    AuthenticationFailed,
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKeyLength { expected, actual } => {
                write!(
                    f,
                    "неверная длина ключа: ожидалось {expected} Б, получено {actual} Б"
                )
            }
            Self::InvalidNonceLength { expected, actual } => {
                write!(
                    f,
                    "неверная длина nonce: ожидалось {expected} Б, получено {actual} Б"
                )
            }
            Self::InvalidTagLength { expected, actual } => {
                write!(
                    f,
                    "неверная длина тега: ожидалось {expected} Б, получено {actual} Б"
                )
            }
            Self::InvalidOutputLength => write!(f, "недопустимая длина вывода KDF"),
            Self::MessageTooLong { limit } => write!(
                f,
                "сообщение длиннее предела 32-битного счётчика блоков (максимум {limit} Б)"
            ),
            Self::AuthenticationFailed => write!(f, "проверка аутентификации не пройдена"),
        }
    }
}

impl std::error::Error for CryptoError {}
