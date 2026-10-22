//! REST v9: запрет телеметрии science/beaker/track (RW-11 / P1-11, T-SEC-04).
//!
//! Контракт заморожен координатором до старта W11b; реализация — `is_telemetry_path` +
//! `guard_target` (4/4 green), без сети и без отправки телеметрии.
//!
//! Оракул: `docs/re/rest-d02.md` §3/§7 — `POST /api/v9/science` (88 запросов с
//! `Authorization`), `POST /beaker` (1), `POST /track` не наблюдался; в API-таблицу они
//! осознанно не включены. Политика клиента — телеметрия по умолчанию **не отправляется**.

/// Точные первые сегменты путей телеметрии (после `/api/v9`).
pub const TELEMETRY_SEGMENTS: [&str; 3] = ["science", "beaker", "track"];

/// Отказ политики телеметрии.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryError {
    /// Путь относится к телеметрии и не должен уходить в сеть.
    Blocked,
}

/// Первый сегмент пути: между ведущим `/` и следующим `/`, без query и fragment.
///
/// `None` — target не начинается с `/` (относительный путь не является путём REST-запроса,
/// поэтому, например, `science` без слэша — не телеметрия).
fn first_path_segment(target: &str) -> Option<&str> {
    // Query и fragment отбрасываются целиком: режем по первому `?` или `#`.
    let path = match target.find(['?', '#']) {
        Some(end) => &target[..end],
        None => target,
    };
    let rest = path.strip_prefix('/')?;
    Some(
        rest.split_once('/')
            .map_or(rest, |(segment, _tail)| segment),
    )
}

/// Является ли target телеметрическим: первый сегмент пути точно `science`/`beaker`/`track`.
///
/// Query и fragment отбрасываются; `/scientific` и `/users/@me/science` — **не** телеметрия.
#[must_use]
pub fn is_telemetry_path(target: &str) -> bool {
    first_path_segment(target).is_some_and(|segment| TELEMETRY_SEGMENTS.contains(&segment))
}

/// Пропускает target или отказывает [`TelemetryError::Blocked`].
///
/// При пропуске возвращается исходный срез без копирования и нормализации.
pub fn guard_target(target: &str) -> Result<&str, TelemetryError> {
    if is_telemetry_path(target) {
        Err(TelemetryError::Blocked)
    } else {
        Ok(target)
    }
}
