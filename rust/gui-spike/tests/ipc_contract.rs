//! Контракт IPC-команд спайка: чистые функции из `lib.rs` без Tauri-рантайма.
//!
//! Проверяем ровно то, что команда отдаёт фронтенду/харнессу:
//! - `validate_ui_snapshot` — пороги UI_SNAPSHOT (4/100000/24229/10/38/rendered≥10);
//! - `validate_bench_payload` — JSON с объектами chat/members;
//! - `read_status_field`/`parse_process_metrics` — маппинг `/proc/self/status` без вранья
//!   (RSS именно VmRSS, отсутствие поля — ошибка).

use discord_gui_spike::{
    parse_process_metrics, read_status_field, validate_bench_payload, validate_ui_snapshot,
    ProcessMetrics, SNAPSHOT_FLOORS,
};

const VALID_SNAPSHOT: &str = r#"{"screenCount":4,"chatRows":100000,"memberRows":24229,"emojiCategories":10,"settingsToggles":38,"chatRenderedRows":30,"memberRenderedRows":24,"actions":{"screenClicks":4}}"#;

/// Снимок с подменённым одним полем: остальные — валидные.
fn snapshot_with(key: &str, raw_json_value: &str) -> String {
    let mut object = serde_json::from_str::<serde_json::Value>(VALID_SNAPSHOT)
        .expect("VALID_SNAPSHOT — корректный JSON")
        .as_object()
        .expect("VALID_SNAPSHOT — объект")
        .clone();
    object.insert(
        key.to_string(),
        serde_json::from_str(raw_json_value).expect("сырое значение — корректный JSON"),
    );
    serde_json::Value::Object(object).to_string()
}

// ---------------------------------------------------------------------------
// UI_SNAPSHOT
// ---------------------------------------------------------------------------

#[test]
fn accepts_full_snapshot_and_preserves_extra_fields() {
    let value = validate_ui_snapshot(VALID_SNAPSHOT).expect("валидный снимок");
    assert_eq!(value["screenCount"], 4);
    assert_eq!(value["chatRows"], 100_000);
    assert_eq!(value["memberRows"], 24_229);
    assert_eq!(value["emojiCategories"], 10);
    assert_eq!(value["settingsToggles"], 38);
    // Команда печатает значение целиком: actions должны доехать до лога.
    assert_eq!(value["actions"]["screenClicks"], 4);
}

#[test]
fn snapshot_floors_are_the_documented_ones() {
    // Порог — часть контракта, а не деталь реализации: сверяем независимо от функции.
    assert_eq!(
        SNAPSHOT_FLOORS,
        [
            ("screenCount", 4),
            ("chatRows", 100_000),
            ("memberRows", 24_229),
            ("emojiCategories", 10),
            ("settingsToggles", 38),
            ("chatRenderedRows", 10),
            ("memberRenderedRows", 10),
        ]
    );
}

#[test]
fn accepts_values_exactly_at_the_floors() {
    // Граница включительная: ровно порог — ещё валидно (и 10 rendered строк — минимум).
    let payload = r#"{"screenCount":4,"chatRows":100000,"memberRows":24229,"emojiCategories":10,"settingsToggles":38,"chatRenderedRows":10,"memberRenderedRows":10}"#;
    assert!(validate_ui_snapshot(payload).is_ok());
}

#[test]
fn rejects_missing_each_required_field() {
    for key in [
        "screenCount",
        "chatRows",
        "memberRows",
        "emojiCategories",
        "settingsToggles",
        "chatRenderedRows",
        "memberRenderedRows",
    ] {
        let mut object = serde_json::from_str::<serde_json::Value>(VALID_SNAPSHOT)
            .expect("valid json")
            .as_object()
            .expect("object")
            .clone();
        object.remove(key);
        let payload = serde_json::Value::Object(object).to_string();
        let error = validate_ui_snapshot(&payload).expect_err(key);
        assert!(
            error.contains(key) || error.contains("не содержит"),
            "{key}: {error}"
        );
    }
}

#[test]
fn rejects_wrong_types() {
    // as_u64 отвергает строки, float, отрицательные, null и bool.
    for bad in [r#""4""#, "4.5", "-1", "null", "true", "[]", "{}"] {
        let payload = snapshot_with("screenCount", bad);
        assert!(
            validate_ui_snapshot(&payload).is_err(),
            "принят неверный тип screenCount: {bad}"
        );
    }
}

#[test]
fn rejects_negative_values() {
    let payload = snapshot_with("chatRows", "-100000");
    let error = validate_ui_snapshot(&payload).expect_err("отрицательный chatRows");
    assert!(error.contains("chatRows"), "неожиданная ошибка: {error}");
}

#[test]
fn rejects_degenerate_zero_values() {
    // Нули/единицы не доказывают, что обещанный UI действительно отрисован.
    let payload = r#"{"screenCount":0,"chatRows":1,"memberRows":1,"emojiCategories":1,"settingsToggles":1,"chatRenderedRows":1,"memberRenderedRows":1}"#;
    assert!(validate_ui_snapshot(payload).is_err());
}

#[test]
fn rejects_rendered_rows_below_floor() {
    // Регрессия виртуализации: рисуется только overscan (~8) — это не отрисованный список.
    for (field, payload) in [
        (
            "chatRenderedRows",
            r#"{"screenCount":4,"chatRows":100000,"memberRows":24229,"emojiCategories":10,"settingsToggles":38,"chatRenderedRows":9,"memberRenderedRows":24}"#,
        ),
        (
            "memberRenderedRows",
            r#"{"screenCount":4,"chatRows":100000,"memberRows":24229,"emojiCategories":10,"settingsToggles":38,"chatRenderedRows":30,"memberRenderedRows":9}"#,
        ),
    ] {
        let error = validate_ui_snapshot(payload).expect_err(field);
        assert!(
            error.contains(field),
            "{field}: неожиданная ошибка: {error}"
        );
    }
}

#[test]
fn rejects_invalid_json_with_parser_message() {
    let error = validate_ui_snapshot("{not json").expect_err("битый JSON");
    assert!(
        error.contains("некорректный UI-снимок"),
        "ошибка должна происходить из парсера JSON: {error}"
    );
    assert!(validate_ui_snapshot("").is_err());
    assert!(validate_ui_snapshot("[]").is_err());
}

// ---------------------------------------------------------------------------
// BENCH
// ---------------------------------------------------------------------------

#[test]
fn accepts_bench_payload_with_chat_and_members_objects() {
    let payload = r#"{"frontend_ready_ms":21,"fps":59,"p95_ms":17,"chat":{"frames":300},"members":{"frames":299}}"#;
    let value = validate_bench_payload(payload).expect("валидный BENCH");
    assert_eq!(value["chat"]["frames"], 300);
    assert_eq!(value["members"]["frames"], 299);
}

#[test]
fn rejects_bench_payload_with_non_object_chat_or_members() {
    for payload in [
        r#"{"chat":null,"members":{}}"#,
        r#"{"chat":{},"members":[]}"#,
        r#"{"chat":42,"members":{}}"#,
        r#"{"chat":{},"members":"{}"}"#,
        r#"{"members":{}}"#,
        r#"{"chat":{}}"#,
    ] {
        assert!(
            validate_bench_payload(payload).is_err(),
            "принят BENCH без объектов chat/members: {payload}"
        );
    }
}

#[test]
fn rejects_bench_payload_with_invalid_json() {
    let error = validate_bench_payload("not json").expect_err("битый JSON");
    assert!(
        error.contains("некорректный BENCH-снимок"),
        "ошибка должна происходить из парсера JSON: {error}"
    );
    assert!(validate_bench_payload("").is_err());
}

// ---------------------------------------------------------------------------
// /proc/self/status
// ---------------------------------------------------------------------------

#[test]
fn reads_numeric_fields_from_proc_status() {
    let status = "Name:\tdiscord-gui-spike\nVmRSS:\t  123456 kB\nThreads:\t12\n";
    assert_eq!(read_status_field(status, "VmRSS:"), Some(123_456));
    assert_eq!(read_status_field(status, "Threads:"), Some(12));
}

#[test]
fn missing_status_field_is_none() {
    assert_eq!(read_status_field("Name:\ttest\n", "VmRSS:"), None);
}

#[test]
fn status_field_matches_only_line_start() {
    // Ключ внутри другой строки не должен матчиться (starts_with, не contains).
    let status = "Name:\ttest\nSomethingVmRSS:\t 999 kB\nVmRSS:\t 42 kB\n";
    assert_eq!(read_status_field(status, "VmRSS:"), Some(42));
    assert_eq!(
        read_status_field("Name:\ttest\nPrefixVmRSS:\t 999 kB\n", "VmRSS:"),
        None
    );
}

#[test]
fn non_numeric_status_value_is_none() {
    assert_eq!(read_status_field("VmRSS:\t n/a kB\n", "VmRSS:"), None);
    assert_eq!(read_status_field("VmRSS:\n", "VmRSS:"), None);
}

#[test]
fn parses_all_metrics_and_maps_rss_to_vmrss_not_hwm() {
    // R9: RSS — именно VmRSS, HWM — VmHWM, потоки — Threads. Поля не перепутаны.
    let status = "Name:\ttest\nVmRSS:\t 1000 kB\nVmHWM:\t 2000 kB\nThreads:\t 7\n";
    let metrics = parse_process_metrics(status).expect("валидный status");
    assert_eq!(
        metrics,
        ProcessMetrics {
            rss_kib: 1000,
            hwm_kib: 2000,
            threads: 7,
        }
    );
    assert_ne!(metrics.rss_kib, metrics.hwm_kib);
}

#[test]
fn parse_metrics_rejects_missing_each_field() {
    for (missing, status) in [
        ("VmRSS:", "VmHWM:\t 2000 kB\nThreads:\t 7\n"),
        ("VmHWM:", "VmRSS:\t 1000 kB\nThreads:\t 7\n"),
        ("Threads:", "VmRSS:\t 1000 kB\nVmHWM:\t 2000 kB\n"),
    ] {
        let error = parse_process_metrics(status).expect_err(missing);
        assert!(error.contains(missing), "{missing}: {error}");
    }
}

#[test]
fn parse_metrics_rejects_non_numeric_field() {
    // Нечитаемое поле — ошибка, а не молчаливый 0 (иначе метрика врёт).
    let status = "VmRSS:\t n/a kB\nVmHWM:\t 2000 kB\nThreads:\t 7\n";
    let error = parse_process_metrics(status).expect_err("VmRSS не число");
    assert!(error.contains("VmRSS:"), "неожиданная ошибка: {error}");
}

#[test]
fn metrics_serialize_with_snake_case_fields() {
    // IPC отдаёт JSON: имена полей — часть контракта фронтенда.
    let metrics = ProcessMetrics {
        rss_kib: 1,
        hwm_kib: 2,
        threads: 3,
    };
    let value = serde_json::to_value(metrics).expect("Serialize");
    assert_eq!(value["rss_kib"], 1);
    assert_eq!(value["hwm_kib"], 2);
    assert_eq!(value["threads"], 3);
}
