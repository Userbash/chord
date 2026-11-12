//! Контракт перф-харнеста T-HARN-04 (карточка P7-02).
//!
//! Оракулы тестов независимы от реализации:
//! - детерминизм и digest — собственный FNV-1a 64 по документированному порядку обхода;
//! - форма отчёта — применение событий к состоянию через публичный `chord_core::state::apply`;
//! - регрессия — арифметика процентов от базового значения;
//! - JSON — независимый разбор `serde_json::Value`.
//!
//! Тайминги сравниваются только в skip-aware тесте бюджета холодного старта
//! (`budget_cold_start_k`): на не-референсной машине и в debug-сборке проверка
//! помечается пропуском с причиной, а не падением.

use std::path::Path;

use chord_core::state::{EventPayload, MemberListEntry, MemberListId, MemberListOp, apply};
use chord_perf::workload::{
    Metric, PERF_GUILD_ID, PERF_MEMBER_LIST_ID, PerfReport, REGRESSION_THRESHOLD_PCT, Regression,
    Workload, WorkloadSpec, base_state, build_workload, compare, measure, report_from_json,
    report_to_json,
};

/// Смещение FNV-1a 64.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// Простой множитель FNV-1a 64.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Независимый FNV-1a 64 по байтам.
fn fnv_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// FNV-1a 64 по `u64` в little-endian.
fn fnv_u64(hash: u64, value: u64) -> u64 {
    fnv_bytes(hash, &value.to_le_bytes())
}

/// Независимый оракул digest: порядок обхода зафиксирован контрактом.
///
/// 1. события в порядке `Workload::events`;
/// 2. `GuildMemberListUpdate` — по `ops`, по элементам op'а; для участника — 8 байт
///    `member.user_id` (little-endian);
/// 3. `MessageCreate` — 8 байт `id`, 8 байт `channel_id`, затем байты `content`.
fn oracle_digest(workload: &Workload) -> u64 {
    let mut hash = FNV_OFFSET;
    for event in &workload.events {
        match &event.payload {
            EventPayload::GuildMemberListUpdate(update) => {
                for op in &update.ops {
                    match op {
                        MemberListOp::Sync { items, .. } => {
                            for entry in items {
                                if let MemberListEntry::Member { member, .. } = entry {
                                    hash = fnv_u64(hash, member.user_id);
                                }
                            }
                        }
                        MemberListOp::Insert { item, .. } | MemberListOp::Update { item, .. } => {
                            if let MemberListEntry::Member { member, .. } = item {
                                hash = fnv_u64(hash, member.user_id);
                            }
                        }
                        MemberListOp::Delete { .. } => {}
                    }
                }
            }
            EventPayload::MessageCreate(message) => {
                hash = fnv_u64(hash, message.id);
                hash = fnv_u64(hash, message.channel_id);
                hash = fnv_bytes(hash, message.content.as_bytes());
            }
            _ => {}
        }
    }
    hash
}

/// Малая нагрузка: тесты контракта не должны жечь CPU референсного масштаба.
fn small_spec(seed: u64) -> WorkloadSpec {
    WorkloadSpec {
        members: 64,
        messages: 128,
        channels: 8,
        seed,
    }
}

/// Отчёт с заданными метриками (остальные поля не влияют на сравнение).
fn report_with(cold_start_ms: f64, ingest_ms: f64, selector_us: f64) -> PerfReport {
    PerfReport {
        spec: WorkloadSpec::reference(),
        cold_start_ms,
        ingest_ms,
        selector_us,
        members_loaded: 0,
        messages_ingested: 0,
        digest: 0,
    }
}

/// Один и тот же seed ⇒ побайтово та же нагрузка.
#[test]
fn determinism_k() {
    let spec = small_spec(42);
    let first = build_workload(&spec);
    let second = build_workload(&spec);

    assert_eq!(
        first.events.len(),
        second.events.len(),
        "одинаковый seed ⇒ одинаковое число событий"
    );
    assert_eq!(
        first.events, second.events,
        "одинаковый seed ⇒ события совпадают побайтово"
    );

    let first_report = measure(&first);
    let second_report = measure(&second);
    assert_eq!(
        first_report.digest, second_report.digest,
        "одинаковый seed ⇒ одинаковый digest"
    );
    assert_eq!(
        first_report.digest,
        oracle_digest(&first),
        "digest совпадает с независимым оракулом"
    );
}

/// Другой seed ⇒ другая нагрузка.
#[test]
fn different_seed_k() {
    let first = build_workload(&small_spec(1));
    let second = build_workload(&small_spec(2));

    assert_ne!(
        oracle_digest(&first),
        oracle_digest(&second),
        "разные seed ⇒ разное содержимое нагрузки"
    );
    assert_ne!(
        measure(&first).digest,
        measure(&second).digest,
        "разные seed ⇒ разный digest отчёта"
    );
}

/// Референсная нагрузка P7-02.
#[test]
fn spec_reference_k() {
    let spec = WorkloadSpec::reference();
    assert_eq!(spec.members, 24_229, "участников в референсе");
    assert_eq!(spec.messages, 100_000, "сообщений в референсе");
    assert_eq!(spec.channels, 512, "каналов в референсе");
    assert_eq!(
        spec,
        WorkloadSpec::reference(),
        "референсная нагрузка детерминирована"
    );
}

/// Отчёт заполнен полностью, счётчики — из фактического состояния.
#[test]
fn report_shape_k() {
    let spec = small_spec(7);
    let workload = build_workload(&spec);
    let report = measure(&workload);

    assert_eq!(report.spec, spec, "в отчёте та же нагрузка");
    assert_eq!(
        report.members_loaded, spec.members,
        "все участники попали в состояние"
    );
    assert_eq!(
        report.messages_ingested, spec.messages,
        "все сообщения применены"
    );
    assert!(
        report.cold_start_ms > 0.0,
        "холодный старт измерен (нули ⇒ базовое состояние не собралось)"
    );
    assert!(report.ingest_ms >= 0.0, "ingest неотрицателен");
    assert!(report.selector_us >= 0.0, "селектор неотрицателен");
    assert_eq!(
        report.digest,
        oracle_digest(&workload),
        "digest совпадает с независимым оракулом"
    );
}

/// События реально применяются редьюсером: проверка через публичный API.
#[test]
fn ingest_applied_k() {
    let spec = small_spec(11);
    let workload = build_workload(&spec);

    let mut state = base_state(&spec).expect("базовое состояние строится");
    for event in &workload.events {
        let (next, _effects) = apply(state, event.clone());
        state = next;
    }

    assert_eq!(
        state.message_count(),
        spec.messages,
        "редьюсер применил все сообщения"
    );
    let list_id = MemberListId::new(PERF_MEMBER_LIST_ID);
    let members = state
        .member_list_items(PERF_GUILD_ID, &list_id)
        .iter()
        .filter(|entry| matches!(entry, MemberListEntry::Member { .. }))
        .count();
    assert_eq!(members, spec.members, "редьюсер сохранил всех участников");
}

/// Одинаковые отчёты ⇒ регрессии нет.
#[test]
fn regression_none_k() {
    let baseline = report_with(100.0, 100.0, 100.0);
    let current = report_with(100.0, 100.0, 100.0);
    assert_eq!(
        compare(&current, &baseline, REGRESSION_THRESHOLD_PCT),
        Regression::None
    );

    let faster = report_with(50.0, 50.0, 50.0);
    assert_eq!(
        compare(&faster, &baseline, REGRESSION_THRESHOLD_PCT),
        Regression::None,
        "ускорение — не регрессия"
    );
}

/// Ухудшение больше порога ⇒ регрессия.
#[test]
fn regression_detected_k() {
    let baseline = report_with(100.0, 100.0, 100.0);
    let current = report_with(111.0, 100.0, 100.0);

    match compare(&current, &baseline, REGRESSION_THRESHOLD_PCT) {
        Regression::Regressed {
            metric,
            previous,
            current: value,
            delta_pct,
        } => {
            assert_eq!(metric, Metric::ColdStart, "ухудшился холодный старт");
            assert_eq!(previous, 100.0, "базовое значение");
            assert_eq!(value, 111.0, "текущее значение");
            assert!(
                (delta_pct - 11.0).abs() < 1e-6,
                "дельта регрессии ≈ 11%, получено {delta_pct}"
            );
        }
        other => panic!("ожидалась регрессия, получено {other:?}"),
    }
}

/// Ухудшение меньше порога ⇒ регрессии нет.
#[test]
fn regression_below_threshold_k() {
    let baseline = report_with(100.0, 100.0, 100.0);
    let current = report_with(109.0, 100.0, 100.0);
    assert_eq!(
        compare(&current, &baseline, REGRESSION_THRESHOLD_PCT),
        Regression::None,
        "9% меньше порога 10%"
    );
}

/// Нулевая базовая метрика и нулевая текущая ⇒ регрессии нет.
#[test]
fn regression_zero_baseline_k() {
    let baseline = report_with(0.0, 100.0, 100.0);
    let current = report_with(0.0, 100.0, 100.0);
    assert_eq!(
        compare(&current, &baseline, REGRESSION_THRESHOLD_PCT),
        Regression::None,
        "нулевая база и нулевой текущий замер — не регрессия"
    );
}

/// Нулевая базовая метрика и ненулевая текущая ⇒ регрессия (рост с нуля).
///
/// Процент от нуля не определён, поэтому `delta_pct` = `f64::INFINITY`.
#[test]
fn regression_zero_baseline_to_nonzero_k() {
    let baseline = report_with(0.0, 100.0, 100.0);
    let current = report_with(500.0, 100.0, 100.0);

    match compare(&current, &baseline, REGRESSION_THRESHOLD_PCT) {
        Regression::Regressed {
            metric,
            previous,
            current: value,
            delta_pct,
        } => {
            assert_eq!(metric, Metric::ColdStart, "ухудшился холодный старт");
            assert_eq!(previous, 0.0, "базовое значение — ноль");
            assert_eq!(value, 500.0, "текущее значение");
            assert!(
                delta_pct.is_infinite() && delta_pct.is_sign_positive(),
                "процент от нуля не определён ⇒ бесконечность, получено {delta_pct}"
            );
        }
        other => panic!("ожидалась регрессия, получено {other:?}"),
    }
}

/// Отчёт переживает JSON round-trip без потерь.
#[test]
fn json_roundtrip_k() {
    let spec = small_spec(3);
    let report = measure(&build_workload(&spec));

    let text = report_to_json(&report).expect("отчёт сериализуется в JSON");
    let parsed = report_from_json(&text).expect("отчёт разбирается из JSON");
    assert_eq!(parsed, report, "round-trip без потерь");

    let value: serde_json::Value =
        serde_json::from_str(&text).expect("JSON разбирается независимым парсером");
    assert_eq!(value["spec"]["members"], serde_json::json!(spec.members));
    assert_eq!(value["spec"]["messages"], serde_json::json!(spec.messages));
    assert_eq!(value["members_loaded"], serde_json::json!(spec.members));
    assert_eq!(value["messages_ingested"], serde_json::json!(spec.messages));
    assert_eq!(value["digest"], serde_json::json!(report.digest));

    // Метрики времени — JSON-числа; с feature `float_roundtrip` разбор точен.
    let metric = &value["cold_start_ms"];
    assert!(
        metric.is_number(),
        "метрика времени сериализуется числом, получено {metric}"
    );

    // Регрессия: число, которое JSON-парсер не разбирает точно.
    let tricky = report_with(0.048010000000000004, 1.0e-7, 1.0e20);
    let text = report_to_json(&tricky).expect("отчёт сериализуется в JSON");
    let parsed = report_from_json(&text).expect("отчёт разбирается из JSON");
    assert_eq!(
        parsed, tricky,
        "точные значения f64 переживают round-trip без потерь"
    );
}

/// Метрики времени — JSON-числа, и round-trip побитово точный.
///
/// `serde_json` с feature `float_roundtrip` разбирает числа корректно, поэтому
/// значение, не представимое точно в десятичной записи, переживает
/// `to_string` → `from_str` без потери младшего бита.
#[test]
fn json_float_roundtrip_k() {
    let tricky = report_with(0.048010000000000004, 1.0e-7, 1.0e20);
    let text = report_to_json(&tricky).expect("отчёт сериализуется в JSON");

    let value: serde_json::Value =
        serde_json::from_str(&text).expect("JSON разбирается независимым парсером");
    let metric = &value["cold_start_ms"];
    assert!(
        metric.is_number(),
        "метрика времени — JSON-число, получено {metric}"
    );

    let parsed = report_from_json(&text).expect("отчёт разбирается из JSON");
    assert_eq!(
        parsed.cold_start_ms.to_bits(),
        tricky.cold_start_ms.to_bits(),
        "cold_start_ms переживает round-trip побитово"
    );
    assert_eq!(
        parsed.ingest_ms.to_bits(),
        tricky.ingest_ms.to_bits(),
        "ingest_ms переживает round-trip побитово"
    );
    assert_eq!(
        parsed.selector_us.to_bits(),
        tricky.selector_us.to_bits(),
        "selector_us переживает round-trip побитово"
    );
    assert_eq!(parsed, tricky, "отчёт совпадает целиком");
}

/// Presence есть у каждого участника нагрузки, а не у выборки.
///
/// Проверка через публичное поле состояния `presences` и публичные селекторы списка.
#[test]
fn presence_for_every_member_k() {
    let spec = WorkloadSpec {
        members: 200,
        messages: 16,
        channels: 4,
        seed: 5,
    };
    let workload = build_workload(&spec);

    let mut state = base_state(&spec).expect("базовое состояние строится");
    for event in &workload.events {
        let (next, _effects) = apply(state, event.clone());
        state = next;
    }

    let list_id = MemberListId::new(PERF_MEMBER_LIST_ID);
    let members: Vec<u64> = state
        .member_list_items(PERF_GUILD_ID, &list_id)
        .iter()
        .filter_map(|entry| match entry {
            MemberListEntry::Member { member, .. } => Some(member.user_id),
            MemberListEntry::Group { .. } => None,
        })
        .collect();
    assert_eq!(members.len(), spec.members, "в списке все участники");

    assert_eq!(
        state.presences.len(),
        spec.members,
        "presence-store содержит запись для каждого участника"
    );
    for user_id in &members {
        assert!(
            state.presences.keys().any(|(id, _)| id == user_id),
            "presence есть для участника {user_id}"
        );
    }
}

/// Имена метрик стабильны.
#[test]
fn metric_names_k() {
    assert_eq!(Metric::ColdStart.as_str(), "cold_start");
    assert_eq!(Metric::Ingest.as_str(), "ingest");
    assert_eq!(Metric::Selector.as_str(), "selector");
}

/// Порог регрессии T-HARN-04 — 10%.
#[test]
fn threshold_constant_k() {
    assert_eq!(REGRESSION_THRESHOLD_PCT, 10.0);
}

/// Бюджет холодного старта ≤ 1,5 с — только на референсной машине и в release.
///
/// Референс: ≥ 8 логических CPU. Замер — медиана из 3 прогонов, чтобы не ловить флейк.
#[test]
fn budget_cold_start_k() {
    if cfg!(debug_assertions) {
        eprintln!(
            "SKIP budget_cold_start_k: бюджет 1500 мс — характеристика release-сборки; \
             в debug-сборке редьюсер в разы медленнее, проверка была бы ложным флейком"
        );
        return;
    }
    let parallelism = std::thread::available_parallelism().map_or(0, |value| value.get());
    if parallelism < 8 {
        eprintln!(
            "SKIP budget_cold_start_k: референс требует ≥ 8 логических CPU, на машине {parallelism}"
        );
        return;
    }

    let spec = WorkloadSpec::reference();
    let workload = build_workload(&spec);
    let mut samples = Vec::with_capacity(3);
    for _ in 0..3 {
        samples.push(measure(&workload).cold_start_ms);
    }
    samples.sort_by(|left, right| left.total_cmp(right));
    let median = samples[1];
    eprintln!(
        "budget_cold_start_k: холодный старт, мс (прогоны {samples:?}), медиана {median}, \
         референс ≤ 1500"
    );
    assert!(
        median <= 1500.0,
        "холодный старт {median} мс превышает бюджет 1500 мс"
    );
}

/// Референсный прогон: масштаб 24 229 участников и 100 000 сообщений.
///
/// Печатает отчёт — это база CI-истории T-HARN-04. Эталонный файл
/// `tests/data/baseline-reference.json` читается, если он есть; отсутствие файла —
/// не ошибка (сообщается причина).
#[test]
fn reference_report_k() {
    let spec = WorkloadSpec::reference();
    let workload = build_workload(&spec);
    let report = measure(&workload);

    assert_eq!(
        report.members_loaded, spec.members,
        "все участники в состоянии"
    );
    assert_eq!(
        report.messages_ingested, spec.messages,
        "все сообщения применены"
    );
    assert_eq!(
        report.digest,
        oracle_digest(&workload),
        "digest референса совпадает с независимым оракулом"
    );

    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/baseline-reference.json");
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            let baseline = report_from_json(&text).expect("эталон — валидный JSON отчёта");
            assert_eq!(baseline.spec, spec, "эталон описывает референсную нагрузку");
            assert_eq!(baseline.members_loaded, spec.members);
            assert_eq!(baseline.messages_ingested, spec.messages);
            assert_eq!(
                baseline.digest, report.digest,
                "digest эталона совпадает с текущим референсом"
            );
        }
        Err(error) => {
            eprintln!(
                "SKIP baseline-reference.json: эталона нет ({error}); проверка его полей пропущена"
            );
        }
    }

    eprintln!(
        "reference_report_k: {}",
        report_to_json(&report).unwrap_or_else(|error| format!("<JSON не собран: {error}>"))
    );
}
