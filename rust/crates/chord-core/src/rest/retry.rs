//! REST v9: таймауты и ретраи идемпотентных запросов (T-REST-03, волна RW-08 / P1-07).
//!
//! Публичные типы и сигнатуры — замороженный контракт (`waves/RW-08/TZ.md` §6.1).
//!
//! Политика над транспорт-замыканием (без сети: реальный HTTP-клиент/TLS — открытый вопрос
//! владельцу, P1-08/P1-09; тесты волны работают на локальном стабе `127.0.0.1`).
//! Идемпотентность — RFC 9110 §9.2.2: GET/PUT/DELETE повторяемы, POST/PATCH — нет.
//! HTTP-статусы (429/5xx) ретраями здесь не управляют — это P1-09.

use std::future::Future;
use std::time::Duration;

use tokio::time::{sleep, timeout};

use super::routes::HttpMethod;

/// Лимиты политики ретраев.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryLimits {
    /// Максимум попыток идемпотентного запроса; 0 трактуется как 1.
    pub max_attempts: u32,
    /// Таймаут одной попытки.
    pub attempt_timeout: Duration,
    /// Пауза между попытками.
    pub backoff: Duration,
}

/// Ошибка одной попытки.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttemptError {
    /// Будущее не завершилось за `attempt_timeout`.
    Timeout,
    /// Транспортная ошибка (обрыв, отказ соединения, пустой ответ…).
    Transport(String),
}

/// Итоговая ошибка политики ретраев.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryError {
    /// Не-идемпотентный запрос: повтор запрещён (ровно одна попытка).
    NotRetried {
        /// Ошибка единственной попытки.
        error: AttemptError,
    },
    /// Идемпотентный запрос: попытки исчерпаны.
    Exhausted {
        /// Сколько попыток выполнено.
        attempts: u32,
        /// Ошибка последней попытки.
        last: AttemptError,
    },
}

/// Успешный результат с числом выполненных попыток.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Retried<T> {
    /// Значение успешной попытки.
    pub value: T,
    /// Сколько попыток выполнено (включая успешную).
    pub attempts: u32,
}

/// Идемпотентен ли метод (RFC 9110 §9.2.2): GET/PUT/DELETE — да, POST/PATCH — нет.
#[must_use]
pub fn is_idempotent(method: HttpMethod) -> bool {
    matches!(
        method,
        HttpMethod::Get | HttpMethod::Put | HttpMethod::Delete
    )
}

/// Одна попытка под `attempt_timeout`.
///
/// Истечение таймаута → [`AttemptError::Timeout`]; иначе результат попытки как есть
/// (включая [`AttemptError::Transport`]).
pub async fn attempt_once<T, F, Fut>(attempt_timeout: Duration, f: F) -> Result<T, AttemptError>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<T, AttemptError>>,
{
    match timeout(attempt_timeout, f()).await {
        Ok(result) => result,
        Err(_elapsed) => Err(AttemptError::Timeout),
    }
}

/// Выполняет запрос по политике ретраев.
///
/// Не-идемпотентный метод — ровно одна попытка (`Err` → [`RetryError::NotRetried`]).
/// Идемпотентный — до `limits.max_attempts` попыток (значение 0 трактуется как 1) с паузой
/// `limits.backoff` между ними; успех → [`Retried`], исчерпание → [`RetryError::Exhausted`].
/// Каждая попытка обёрнута в [`attempt_once`] с `limits.attempt_timeout`.
pub async fn execute_with_retries<T, F, Fut>(
    method: HttpMethod,
    limits: &RetryLimits,
    mut attempt: F,
) -> Result<Retried<T>, RetryError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, AttemptError>>,
{
    let idempotent = is_idempotent(method);
    let max_attempts = if idempotent {
        limits.max_attempts.max(1)
    } else {
        1
    };
    let mut attempts = 0u32;
    loop {
        attempts += 1;
        match attempt_once(limits.attempt_timeout, &mut attempt).await {
            Ok(value) => return Ok(Retried { value, attempts }),
            Err(error) => {
                if attempts >= max_attempts {
                    return Err(if idempotent {
                        RetryError::Exhausted {
                            attempts,
                            last: error,
                        }
                    } else {
                        RetryError::NotRetried { error }
                    });
                }
                sleep(limits.backoff).await;
            }
        }
    }
}
