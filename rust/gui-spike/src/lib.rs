//! Чистая логика GUI-спайка: без Tauri-рантайма, чтобы контракт IPC проверялся
//! обычными тестами (`tests/ipc_contract.rs`), а не только живым окном.
//!
//! Здесь три вещи:
//! - разбор `/proc/self/status` в метрики процесса (RSS — именно VmRSS, не VmHWM);
//! - проверка `UI_SNAPSHOT` по порогам (4 экрана / 100k чата / 24 229 участников /
//!   10 категорий / 38 тумблеров / rendered ≥10);
//! - проверка `BENCH`-снимка как JSON с объектами chat/members.
//!
//! BFF фазы 3 — в модуле [`data`]: `ready`, `read_state`, `channels`,
//! `messages_page`, `members_page`, `settings` читают только локальные
//! `fixtures/` (корень — `DL_FIXTURES`).
//!
//! W39j — пилот [`core_bridge`]: та же страница сообщений, но собранная из
//! `discord-core` (`state::Message`/`history::History`), с тем же курсором.
//!
//! RW-39/NET-C2 — [`auth`]: импорт токена владельца, `GET /users/@me`, keyring в
//! потоке-владельце, типизированные ошибки. Сеть — только в `DiscordValidator`,
//! тесты используют фейки.
//!
//! RW-40/B8a — [`settings_store`]: UI-настройки (тумблеры, плотность, тема,
//! язык) в одном JSON-файле; путь — `DL_SETTINGS_PATH` или `<repo>/.local/`,
//! атомарная запись tmp+rename, типизированные `Io`/`Parse`/`Invalid`.
//!
//! RW-40/G-03a — [`guilds`]: вход в сервер из preview/lurker
//! (`PUT /guilds/{id}/members/@me?lurker=true`, тело `{}`, 204) с типизированными
//! ошибками; fake-транспорт в тестах, реальный — `ApiClient` ядра.
//!
//! RW-40/G-03 — [`invites`]: вход по приглашению — resolve (`GET /invites/{code}`)
//! и accept (`POST /invites/{code}`, ровно три nullable-поля) с типизированными
//! ошибками; `[S]`-расширение каталога маршрутов, fake-транспорт в тестах.
//!
//! Команды Tauri (`#[tauri::command]`) и печать маркеров остаются в `main.rs`.

pub mod auth;
pub mod core_bridge;
pub mod data;
pub mod guilds;
pub mod invites;
pub mod messages;
pub mod net;
pub mod reactions;
/// RW-40/G1-01: единый классификатор REST-ошибок для сетевых модулей.
pub mod rest_failure;
pub mod search;
pub mod settings_store;
/// RW-40/P-03 (D-108): исходящий typing `POST /channels/{id}/typing`.
pub mod typing;

use serde::Serialize;

/// Метрики процесса, которые команда `process_metrics` отдаёт фронтенду.
#[derive(Debug, PartialEq, Serialize)]
pub struct ProcessMetrics {
    /// Resident set size, КиБ (из /proc/self/status).
    pub rss_kib: u64,
    /// Пиковый RSS, КиБ.
    pub hwm_kib: u64,
    /// Число потоков процесса.
    pub threads: u64,
}

/// Читает целое значение поля status-файла по началу строки.
/// `None` — поля нет или значение не число (никакого молчаливого 0).
pub fn read_status_field(status: &str, key: &str) -> Option<u64> {
    let line = status.lines().find(|line| line.starts_with(key))?;
    let value = line.split_whitespace().nth(1)?;
    value.parse().ok()
}

/// Разбор status-текста в метрики. Вынесено из команды, чтобы тестировать
/// маппинг полей без чтения живого `/proc` (R9: RSS — именно VmRSS, не VmHWM).
/// Отсутствие/нечитаемость поля — ошибка, а не молчаливый 0 (иначе метрика врёт).
pub fn parse_process_metrics(status: &str) -> Result<ProcessMetrics, String> {
    let field = |key: &str| {
        read_status_field(status, key)
            .ok_or_else(|| format!("в /proc/self/status нет читаемого поля {key}"))
    };
    Ok(ProcessMetrics {
        rss_kib: field("VmRSS:")?,
        hwm_kib: field("VmHWM:")?,
        threads: field("Threads:")?,
    })
}

/// Минимальные значения доказывают, что спайк действительно отрисовал обещанное:
/// 4 экрана, 100k чата, 24 229 участников, 10 категорий, 38 тумблеров.
/// Rendered-строки ≥10 отсекают регрессию, когда виртуализация рисует только overscan.
pub const SNAPSHOT_FLOORS: [(&str, u64); 7] = [
    ("screenCount", 4),
    ("chatRows", 100_000),
    ("memberRows", 24_229),
    ("emojiCategories", 10),
    ("settingsToggles", 38),
    ("chatRenderedRows", 10),
    ("memberRenderedRows", 10),
];

/// Проверка UI-снимка перед печатью маркера `UI_SNAPSHOT`.
pub fn validate_ui_snapshot(payload: &str) -> Result<serde_json::Value, String> {
    let value: serde_json::Value =
        serde_json::from_str(payload).map_err(|err| format!("некорректный UI-снимок: {err}"))?;
    for (key, min) in SNAPSHOT_FLOORS {
        let actual = value
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| format!("UI-снимок не содержит целое неотрицательное поле {key}"))?;
        if actual < min {
            return Err(format!("UI-снимок: {key}={actual} меньше ожидаемого {min}"));
        }
    }
    Ok(value)
}

/// Проверка BENCH-снимка: JSON с объектами chat/members. Иначе перевод строки
/// внутри строки подделал бы маркер `BENCH` в логе.
pub fn validate_bench_payload(payload: &str) -> Result<serde_json::Value, String> {
    let value: serde_json::Value =
        serde_json::from_str(payload).map_err(|err| format!("некорректный BENCH-снимок: {err}"))?;
    for key in ["chat", "members"] {
        if !value.get(key).is_some_and(serde_json::Value::is_object) {
            return Err(format!("BENCH-снимок не содержит объект {key}"));
        }
    }
    Ok(value)
}
