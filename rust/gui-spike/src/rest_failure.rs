//! Единая классификация отказов REST (G1-01).
//!
//! До G1-01 каждый сетевой модуль (`messages`, `reactions`, `search`, `guilds`)
//! держал свою копию `map_api_error` и status-декодера, а хвост `_ => Network`
//! превращал 403/404/5xx в «сеть недоступна». Теперь политика одна:
//!
//! | Вход | [`RestFailure`] |
//! |---|---|
//! | 401 | `Unauthorized` |
//! | 403 | `Forbidden` |
//! | 404 | `NotFound` |
//! | 429 | `RateLimited { retry_after_ms }` |
//! | прочие 4xx | `ClientOther { status }` |
//! | 5xx | `Server { status }` |
//! | транспорт/таймаут | `Network` |
//!
//! Доменные enum'ы конвертируют [`RestFailure`] через `From`, поэтому новый
//! вариант политики добавляется в одном месте, а не в четырёх.
//!
//! 429 из «сырого» статуса несёт `retry_after_ms: 0`: тело/заголовки эти модули
//! не разбирают, задержку не выдумываем (контракт G-03a). Если ядро уже
//! выполнило ретраи и вернуло [`ApiError::RateLimited`] с `retry_after`, значение
//! сохраняется.

use std::time::Duration;

use chord_core::rest::client::ApiError;

/// Классифицированный отказ REST — единый источник политики ошибок.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestFailure {
    /// 401: сессия истекла.
    Unauthorized,
    /// 403: нет доступа к ресурсу.
    Forbidden,
    /// 404: ресурс не найден.
    NotFound,
    /// 429: ограничение частоты; задержка из ответа, если она известна.
    RateLimited {
        /// Задержка до повтора; 0 — статус без разбора тела.
        retry_after_ms: u64,
    },
    /// Прочие 4xx (кроме 401/403/404/429).
    ClientOther {
        /// HTTP-статус.
        status: u16,
    },
    /// 5xx: ошибка сервера.
    Server {
        /// HTTP-статус.
        status: u16,
    },
    /// Транспорт/таймаут: сеть недоступна.
    Network,
    /// 2xx, но тело не разобрано (домен конвертирует в свой `InvalidResponse`).
    InvalidResponse,
}

impl RestFailure {
    /// HTTP-статус → отказ; `None` — 2xx (успех).
    ///
    /// 1xx/3xx не являются подтверждённым ответом API и трактуются как
    /// [`RestFailure::Network`] — как и до G1-01.
    #[must_use]
    pub fn from_status(status: u16) -> Option<Self> {
        match status {
            200..=299 => None,
            401 => Some(Self::Unauthorized),
            403 => Some(Self::Forbidden),
            404 => Some(Self::NotFound),
            429 => Some(Self::RateLimited { retry_after_ms: 0 }),
            400..=499 => Some(Self::ClientOther { status }),
            500..=599 => Some(Self::Server { status }),
            _ => Some(Self::Network),
        }
    }

    /// Ошибка фасада ядра → отказ; HTTP-статус не теряется.
    ///
    /// Локальные отказы без HTTP-статуса (`InvalidBase`/`InvalidRequest`/
    /// `TelemetryBlocked`) остаются `Network`: у них нет статуса, а добавлять
    /// ради них новый `kind` в доменные контракты — отдельное решение (G1-01
    /// аддитивен только для 403/404/5xx).
    #[must_use]
    pub fn from_api_error(error: &ApiError) -> Self {
        match error {
            ApiError::Unauthorized => Self::Unauthorized,
            ApiError::RateLimited { retry_after, .. } => Self::RateLimited {
                retry_after_ms: duration_ms(*retry_after),
            },
            ApiError::Server { status, .. } => Self::Server { status: *status },
            ApiError::Client { status } => match *status {
                401 => Self::Unauthorized,
                403 => Self::Forbidden,
                404 => Self::NotFound,
                429 => Self::RateLimited { retry_after_ms: 0 },
                other => Self::ClientOther { status: other },
            },
            ApiError::InvalidBase(_)
            | ApiError::InvalidRequest(_)
            | ApiError::TelemetryBlocked
            | ApiError::Timeout { .. }
            | ApiError::Transport(_) => Self::Network,
        }
    }
}

/// Миллисекунды без паники: слишком большая задержка сатурируется.
fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
