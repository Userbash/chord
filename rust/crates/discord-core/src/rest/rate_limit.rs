//! REST v9: 429/5xx/401 — разбор rate-limit и политика повторов (RW-11 / P1-09).
//!
//! Чистые функции без I/O: вызывающий применяет задержку и повторяет запрос.
//!
//! Оракул: `docs/re/rest-d02.md` §3 — единственный 429 в захвате
//! (`GET /users/@me/survey`, `run2|95974.8271`), коды 200/204/304/404/429; тела и поля
//! ответов в захвате отсутствуют (§5 — документированный пробел), поэтому семантика полей —
//! RFC 6585 §4 + документированные Discord-поля `retry-after`, `x-ratelimit-*`.

use std::time::Duration;

use super::retry::is_idempotent;
use super::routes::HttpMethod;

/// Разобранный rate-limit ответа.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitInfo {
    /// Задержка до повтора (секунды из заголовка/тела, уже в `Duration`).
    pub retry_after: Duration,
    /// Глобальный лимит (касается всех bucket'ов).
    pub global: bool,
    /// Остаток запросов в bucket'е, если сервер прислал.
    pub remaining: Option<u32>,
    /// `x-ratelimit-reset-after` в `Duration`.
    pub reset_after: Option<Duration>,
    /// Идентификатор bucket'а, если сервер прислал.
    pub bucket: Option<String>,
}

/// Классифицированный отказ REST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestFailure {
    /// 429: ждать `retry_after` и повторить.
    RateLimited(RateLimitInfo),
    /// 401: сессия протухла, повторов нет.
    Unauthorized,
    /// 5xx: серверная ошибка, бэкофф для идемпотентных методов.
    Server {
        /// HTTP-статус.
        status: u16,
    },
    /// Прочие 4xx: повторов нет.
    Client {
        /// HTTP-статус.
        status: u16,
    },
}

/// Значение поля без учёта регистра имени (первое вхождение), с обрезкой OWS.
fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.trim())
}

/// Секунды (целые/дробные) → `Duration`; отрицательные/NaN/∞ → `None`.
fn seconds_to_duration(seconds: f64) -> Option<Duration> {
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }
    Duration::try_from_secs_f64(seconds).ok()
}

/// Разбор значения-секунд из заголовка.
fn parse_seconds(raw: &str) -> Option<Duration> {
    raw.trim().parse::<f64>().ok().and_then(seconds_to_duration)
}

/// Позиция первого байта значения JSON-поля `key` (вне строковых литералов).
///
/// Примитивный сканер достаточен для тел ошибок Discord: ищет `"key"` вне строк, затем
/// двоеточие и OWS. Значения внутри строк (например, текст сообщения) не считаются полями.
fn json_value_start(body: &[u8], key: &str) -> Option<usize> {
    let key = key.as_bytes();
    let mut index = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    while index < body.len() {
        let byte = body[index];
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }

        if byte == b'"' {
            let start = index + 1;
            let end = start + key.len();
            if end < body.len() && &body[start..end] == key && body[end] == b'"' {
                let mut cursor = end + 1;
                while cursor < body.len() && body[cursor].is_ascii_whitespace() {
                    cursor += 1;
                }
                if body.get(cursor) == Some(&b':') {
                    cursor += 1;
                    while cursor < body.len() && body[cursor].is_ascii_whitespace() {
                        cursor += 1;
                    }
                    return Some(cursor);
                }
            }
            in_string = true;
        }
        index += 1;
    }
    None
}

/// Число из JSON-поля верхнего уровня (неотрицательное, конечное).
fn json_number(body: &[u8], key: &str) -> Option<f64> {
    let start = json_value_start(body, key)?;
    let rest = std::str::from_utf8(&body[start..]).ok()?;
    let end = rest
        .find(|ch: char| ch == ',' || ch == '}' || ch.is_whitespace())
        .unwrap_or(rest.len());
    let value = rest[..end].parse::<f64>().ok()?;
    (value.is_finite() && value >= 0.0).then_some(value)
}

/// `true`/`false` из JSON-поля верхнего уровня.
fn json_bool(body: &[u8], key: &str) -> Option<bool> {
    let start = json_value_start(body, key)?;
    let rest = &body[start..];
    if rest.starts_with(b"true") {
        Some(true)
    } else if rest.starts_with(b"false") {
        Some(false)
    } else {
        None
    }
}

/// Разбор rate-limit из статуса, полей и тела.
///
/// `None` — статус не 429. Приоритет задержки: `retry-after` → тело `retry_after` →
/// `x-ratelimit-reset-after`. Отрицательные/NaN/бесконечные значения игнорируются.
#[must_use]
pub fn rate_limit_info(
    status: u16,
    headers: &[(String, String)],
    body: &[u8],
) -> Option<RateLimitInfo> {
    if status != 429 {
        return None;
    }

    let retry_after = header_value(headers, "retry-after")
        .and_then(parse_seconds)
        .or_else(|| json_number(body, "retry_after").and_then(seconds_to_duration))
        .or_else(|| header_value(headers, "x-ratelimit-reset-after").and_then(parse_seconds))
        .unwrap_or(Duration::ZERO);

    let global = header_value(headers, "x-ratelimit-global")
        .is_some_and(|value| value.eq_ignore_ascii_case("true"))
        || json_bool(body, "global").unwrap_or(false);

    let remaining =
        header_value(headers, "x-ratelimit-remaining").and_then(|value| value.parse::<u32>().ok());
    let reset_after = header_value(headers, "x-ratelimit-reset-after").and_then(parse_seconds);
    let bucket = header_value(headers, "x-ratelimit-bucket")
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    Some(RateLimitInfo {
        retry_after,
        global,
        remaining,
        reset_after,
        bucket,
    })
}

/// Классификация отказа: 429/401/5xx/прочие 4xx; `None` — успех/3xx.
#[must_use]
pub fn classify_failure(
    status: u16,
    headers: &[(String, String)],
    body: &[u8],
) -> Option<RestFailure> {
    match status {
        429 => Some(RestFailure::RateLimited(
            rate_limit_info(status, headers, body).unwrap_or(RateLimitInfo {
                retry_after: Duration::ZERO,
                global: false,
                remaining: None,
                reset_after: None,
                bucket: None,
            }),
        )),
        401 => Some(RestFailure::Unauthorized),
        500..=599 => Some(RestFailure::Server { status }),
        400..=499 => Some(RestFailure::Client { status }),
        _ => None,
    }
}

/// Задержка перед повтором; `attempt` — номер уже выполненной попытки (1-based).
///
/// 429 → `retry_after` (не больше `max`); 5xx → `base * 2^(attempt-1)` (не больше `max`);
/// 401/4xx → `None` (повтора нет).
#[must_use]
pub fn retry_delay(
    attempt: u32,
    failure: &RestFailure,
    base: Duration,
    max: Duration,
) -> Option<Duration> {
    match failure {
        RestFailure::RateLimited(info) => Some(info.retry_after.min(max)),
        RestFailure::Server { .. } => {
            let exponent = attempt.saturating_sub(1);
            let factor = 1u32.checked_shl(exponent).unwrap_or(u32::MAX);
            Some(base.saturating_mul(factor).min(max))
        }
        RestFailure::Unauthorized | RestFailure::Client { .. } => None,
    }
}

/// Разрешён ли повтор: 429 — да (после паузы), 5xx — только идемпотентные методы,
/// 401/4xx — нет.
#[must_use]
pub fn should_retry(method: HttpMethod, failure: &RestFailure) -> bool {
    match failure {
        RestFailure::RateLimited(_) => true,
        RestFailure::Server { .. } => is_idempotent(method),
        RestFailure::Unauthorized | RestFailure::Client { .. } => false,
    }
}
