//! Контрактные и интеграционные тесты `TASK-907`:
//! Автоматизированный гейт качества кода:
//! - `SUB-907-1`: Статический анализ и строгий линтинг Rust (`cargo clippy --all-targets -- -D warnings`,
//!   `rustfmt --check --edition 2024`, `cargo audit` / `cargo-deny`, функции `< 50` строк, модули `< 800` строк).
//! - `SUB-907-2`: Динамические санитайзеры LLVM (`RUSTFLAGS="-Zsanitizer=address"` ASan,
//!   `RUSTFLAGS="-Zsanitizer=thread"` TSan на многопоточных кольцевых буферах, детектор дедлоков Tokio <-> GPUI).
//! - `SUB-907-3`: Автоматический `5/5` мутационный гейт тестирования (`100%` kill rate, 5 контрольных мутаций
//!   на каждый изменяемый модуль, прогон таргетированных тестов и проверка восстановления исходника).

#![recursion_limit = "512"]

#[path = "../src/quality_gate/mod.rs"]
mod quality_gate;

use std::path::PathBuf;

use quality_gate::{
    ASAN_RUSTFLAG, CLIPPY_STRICT_FLAG, MAX_FUNCTION_LINES, MAX_MODULE_LINES, QualityGateError,
    QualityGatePipeline, REQUIRED_MUTATION_KILL_RATE, REQUIRED_MUTATIONS_PER_MODULE, RUST_EDITION,
    TSAN_RUSTFLAG,
    mutation_gate::{MutantStatus, MutationGateRunner, MutationGenerator, MutationOperator},
    sanitizers::{
        AsanViolationKind, ConcurrentRingBufferProbe, LockOrderGraph, SanitizerKind,
        SanitizerRunner, TokioGpuiDeadlockDetector, TsanViolationKind,
    },
    static_linter::{
        ClippyGateConfig, ClippyGateRunner, DependencySecurityAuditor, RustSecAdvisory,
        RustfmtGateRunner, SourceMetricsScanner,
    },
};

fn gpui_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

// ═══════════════════════════════════════════════════════════════════════════
// SUB-907-1: Статический анализ и строгий линтинг Rust
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_sub_907_1_step_1_1_clippy_strict_all_targets_deny_warnings_and_unwrap_guard() {
    let cfg = ClippyGateConfig::strict_default();
    assert_eq!(CLIPPY_STRICT_FLAG, "-D warnings");
    assert!(cfg.all_targets);
    assert!(cfg.deny_warnings);
    assert!(cfg.forbid_production_unwrap);

    let args = cfg.cargo_args();
    assert_eq!(
        args,
        vec![
            "clippy".to_string(),
            "--offline".to_string(),
            "--all-targets".to_string(),
            "--".to_string(),
            "-D".to_string(),
            "warnings".to_string(),
        ]
    );

    // 1. Позитивный сценарий: чистый вывод Clippy (0 warnings, 0 errors, exit 0) проходит гейт
    let clean_output = "    Checking discord-gpui v0.1.0\n    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.42s\n";
    let clean_report = ClippyGateRunner::evaluate_output(0, clean_output)
        .expect("Чистый прогон Clippy без предупреждений должен проходить гейт");
    assert_eq!(clean_report.warning_count, 0);
    assert_eq!(clean_report.error_count, 0);
    assert!(clean_report.passed);

    // 2. Негативный сценарий: любой коммит с warning блокируется (-D warnings)
    let warning_output = concat!(
        "warning: unused variable: `x`\n",
        " --> src/net/mod.rs:14:9\n",
        "error: could not compile `discord-gpui` due to 1 previous error\n"
    );
    let err = ClippyGateRunner::evaluate_output(101, warning_output)
        .expect_err("Коммит с предупреждением Clippy обязан блокироваться");
    assert!(matches!(
        err,
        QualityGateError::ClippyWarningsDetected {
            warnings: 1,
            errors: 1,
            ..
        }
    ));

    // 3. Реальный прогон компилятора с `-D warnings` на чистом и варнинговом Rust-модуле
    let real_clean = ClippyGateRunner::compile_probe_with_deny_warnings(
        "pub fn add_checked(a: u32, b: u32) -> Option<u32> { a.checked_add(b) }\n",
    )
    .expect("Чистый код без предупреждений проходит реальную проверку -D warnings");
    assert!(real_clean.passed);

    let real_warn = ClippyGateRunner::compile_probe_with_deny_warnings(
        "pub fn unused_var() -> u32 { let unused_x = 42u32; 7 }\n",
    );
    assert!(
        matches!(
            real_warn,
            Err(QualityGateError::ClippyWarningsDetected { .. })
        ),
        "Неиспользуемая переменная при -D warnings обязана блокировать сборку"
    );

    // 4. Запрет .unwrap() / .expect() в продакшн-путях (разрешено только в #[cfg(test)])
    let prod_with_unwrap = r#"
pub fn parse_packet(raw: &[u8]) -> u8 {
    raw.first().copied().unwrap()
}
#[cfg(test)]
mod tests {
    #[test]
    fn ok_in_test() {
        let v = Some(1).unwrap();
        assert_eq!(v, 1);
    }
}
"#;
    let unwrap_violations = ClippyGateRunner::scan_production_unwraps(prod_with_unwrap);
    assert_eq!(unwrap_violations.len(), 1);
    assert_eq!(unwrap_violations[0].line, 3);
    assert_eq!(unwrap_violations[0].kind, "unwrap()");

    // 5. Проверка отсутствия .unwrap() / .expect() во всех продуктовых файлах src/quality_gate/*.rs
    let qg_dir = gpui_root().join("src/quality_gate");
    for file_name in [
        "mod.rs",
        "static_linter.rs",
        "sanitizers.rs",
        "mutation_gate.rs",
    ] {
        let content = std::fs::read_to_string(qg_dir.join(file_name)).unwrap();
        let found = ClippyGateRunner::scan_production_unwraps(&content);
        assert!(
            found.is_empty(),
            "В продуктовом файле src/quality_gate/{file_name} обнаружены запрещённые unwrap/expect: {found:?}"
        );
    }
}

#[test]
fn test_sub_907_1_step_1_2_rustfmt_check_edition_2024_clean_and_diff_rejection() {
    assert_eq!(RUST_EDITION, "2024");
    let args = RustfmtGateRunner::command_args(&["src/quality_gate/mod.rs"]);
    assert_eq!(
        args,
        vec![
            "--check".to_string(),
            "--edition".to_string(),
            "2024".to_string(),
            "src/quality_gate/mod.rs".to_string(),
        ]
    );

    // 1. Реальная проверка форматирования всех файлов модуля quality_gate через rustfmt --check --edition 2024
    let qg_dir = gpui_root().join("src/quality_gate");
    let qg_files = vec![
        qg_dir.join("mod.rs"),
        qg_dir.join("static_linter.rs"),
        qg_dir.join("sanitizers.rs"),
        qg_dir.join("mutation_gate.rs"),
    ];
    let fmt_report = RustfmtGateRunner::check_files(&qg_files).expect(
        "Все файлы модуля quality_gate обязаны быть отформатированы по rustfmt --edition 2024",
    );
    assert!(fmt_report.passed);
    assert_eq!(fmt_report.edition, "2024");
    assert_eq!(fmt_report.files_checked, 4);
    assert!(fmt_report.unformatted_files.is_empty());

    // 2. Негативный сценарий: неотформатированный код отклоняется реальным rustfmt --check --edition 2024
    let badly_formatted =
        "pub fn   bad_fmt(  a:u32,b:   u32 )->u32{if a>b{return a;}else{return b;}}\n";
    let err = RustfmtGateRunner::check_snippet_edition_2024(badly_formatted).expect_err(
        "Неотформатированный Rust-код обязан отклоняться rustfmt --check --edition 2024",
    );
    assert!(matches!(err, QualityGateError::RustfmtDiffDetected { .. }));
}

#[test]
fn test_sub_907_1_step_1_3_cargo_audit_and_cargo_deny_licenses_bans_sources() {
    let deny_toml_path = gpui_root().join("deny.toml");
    let lock_path = gpui_root().join("Cargo.lock");

    // 1. Проверка конфигурации deny.toml и всех пакетов Cargo.lock в discord-gpui
    let audit_report = DependencySecurityAuditor::audit_project(&deny_toml_path, &lock_path, &[])
        .expect("Аудит лицензий, источников и зависимостей discord-gpui должен проходить");
    assert!(audit_report.passed);
    assert!(audit_report.packages_audited >= 20);
    assert!(audit_report.license_violations.is_empty());
    assert!(audit_report.vulnerability_hits.is_empty());
    assert!(audit_report.source_violations.is_empty());

    // 2. Проверка отбраковки запрещённых лицензий (GPL-3.0-only, AGPL-3.0, Proprietary)
    assert!(DependencySecurityAuditor::is_license_expression_allowed(
        "MIT OR Apache-2.0",
        &audit_report.allowed_licenses
    ));
    assert!(DependencySecurityAuditor::is_license_expression_allowed(
        "Apache-2.0 WITH LLVM-exception OR MIT",
        &audit_report.allowed_licenses
    ));
    assert!(!DependencySecurityAuditor::is_license_expression_allowed(
        "GPL-3.0-only",
        &audit_report.allowed_licenses
    ));
    assert!(!DependencySecurityAuditor::is_license_expression_allowed(
        "(MIT OR Apache-2.0) AND AGPL-3.0-or-later",
        &audit_report.allowed_licenses
    ));

    // 3. Проверка обнаружения уязвимостей cargo audit (RustSec Advisory match)
    let synthetic_advisory = RustSecAdvisory {
        id: "RUSTSEC-2026-9999".to_string(),
        package: "tokio".to_string(),
        affected_version: "1.53.0".to_string(),
        Patched: ">=1.54.0".to_string(),
        title: "Synthetic Tokio vulnerability test".to_string(),
    };
    let synthetic_lock = r#"
[[package]]
name = "tokio"
version = "1.53.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
"#;
    let vuln_err = DependencySecurityAuditor::audit_lock_text(
        &std::fs::read_to_string(&deny_toml_path).unwrap(),
        synthetic_lock,
        &[synthetic_advisory],
    )
    .expect_err("Уязвимая зависимость из базы RustSec обязана блокировать гейт");
    assert!(matches!(
        vuln_err,
        QualityGateError::VulnerabilityDetected { ref advisory_id, ref package, .. }
        if advisory_id == "RUSTSEC-2026-9999" && package == "tokio"
    ));

    // 4. Проверка блокировки недоверенного git-источника (unknown-git = "deny")
    let untrusted_source_lock = r#"
[[package]]
name = "rogue-crate"
version = "0.1.0"
source = "git+https://evil.example.com/rogue.git#deadbeef"
"#;
    let src_err = DependencySecurityAuditor::audit_lock_text(
        &std::fs::read_to_string(&deny_toml_path).unwrap(),
        untrusted_source_lock,
        &[],
    )
    .expect_err("Зависимость из неизвестного git-репозитория обязана блокироваться");
    assert!(matches!(
        src_err,
        QualityGateError::UntrustedDependencySource { .. }
    ));
}

#[test]
fn test_sub_907_1_step_1_4_static_source_size_limits_functions_under_50_files_under_800() {
    assert_eq!(MAX_FUNCTION_LINES, 50);
    assert_eq!(MAX_MODULE_LINES, 800);

    // 1. Все собственные модули quality_gate обязаны соблюдать лимиты: функции < 50 строк, файлы < 800 строк
    let qg_dir = gpui_root().join("src/quality_gate");
    let metrics = SourceMetricsScanner::scan_directory(&qg_dir).expect(
        "Все файлы src/quality_gate обязаны укладываться в < 800 строк и функции < 50 строк",
    );
    assert!(metrics.files_scanned >= 4);
    assert!(metrics.total_functions >= 15);
    for file_m in &metrics.file_metrics {
        assert!(
            file_m.line_count < MAX_MODULE_LINES,
            "Модуль {} имеет {} строк (лимит < 800)",
            file_m.path,
            file_m.line_count
        );
        for fn_m in &file_m.functions {
            assert!(
                fn_m.line_count < MAX_FUNCTION_LINES,
                "Функция {}::{} имеет {} строк (лимит < 50)",
                file_m.path,
                fn_m.name,
                fn_m.line_count
            );
        }
    }

    // 2. Негативный сценарий: функция из 52 строк отклоняется
    let mut long_fn_lines = vec!["pub fn oversized_function(mut x: u32) -> u32 {".to_string()];
    for i in 0..50 {
        long_fn_lines.push(format!("    x = x.wrapping_add({i});"));
    }
    long_fn_lines.push("    x".to_string());
    long_fn_lines.push("}".to_string());
    let long_fn_src = long_fn_lines.join("\n");
    let fn_err = SourceMetricsScanner::scan_source("src/oversized_fn.rs", &long_fn_src)
        .expect_err("Функция >= 50 строк обязана вызывать ошибку гейта");
    assert!(matches!(
        fn_err,
        QualityGateError::FunctionTooLong {
            ref function_name,
            lines,
            max_lines: 50,
            ..
        } if function_name == "oversized_function" && lines >= 50
    ));

    // 3. Негативный сценарий: файл модуля из 805 строк отклоняется
    let long_file_src = (0..805)
        .map(|i| format!("pub const CONST_{i}: u32 = {i};"))
        .collect::<Vec<_>>()
        .join("\n");
    let file_err = SourceMetricsScanner::scan_source("src/giant_module.rs", &long_file_src)
        .expect_err("Модуль >= 800 строк обязан вызывать ошибку гейта");
    assert!(matches!(
        file_err,
        QualityGateError::ModuleFileTooLong {
            lines: 805,
            max_lines: 800,
            ..
        }
    ));
}

// ═══════════════════════════════════════════════════════════════════════════
// SUB-907-2: Динамические санитайзеры LLVM (ASan & TSan) и дедлоки Tokio <-> GPUI
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_sub_907_2_step_2_1_address_sanitizer_detects_uaf_and_oob_and_passes_clean_buffers() {
    assert_eq!(ASAN_RUSTFLAG, "-Zsanitizer=address");
    let env_cfg = SanitizerRunner::env_for(SanitizerKind::Address);
    assert!(
        env_cfg
            .iter()
            .any(|(k, v)| k == "RUSTFLAGS" && v.contains("-Zsanitizer=address"))
    );

    // 1. Позитивный прогон реального LLVM AddressSanitizer (-Zsanitizer=address) на кольцевом буфере
    let clean_asan = SanitizerRunner::run_asan_ring_buffer_contract().expect(
        "Чистый кольцевой буфер должен проходить под реальным LLVM AddressSanitizer без ошибок",
    );
    assert_eq!(clean_asan.sanitizer, SanitizerKind::Address);
    assert!(clean_asan.passed);
    assert!(clean_asan.violations.is_empty());

    // 2. Реальный перехват heap-use-after-free через компиляцию с -Zsanitizer=address
    let uaf_probe = r#"
fn main() {
    let ptr = Box::into_raw(Box::new([0xAAu8; 64]));
    unsafe {
        drop(Box::from_raw(ptr));
        let _leaked_byte = std::ptr::read_volatile(ptr as *const u8);
    }
}
"#;
    let uaf_err =
        SanitizerRunner::compile_and_run_with_sanitizer(SanitizerKind::Address, uaf_probe)
            .expect_err("LLVM AddressSanitizer обязан перехватывать heap-use-after-free");
    assert!(matches!(
        uaf_err,
        QualityGateError::SanitizerViolationDetected {
            sanitizer: SanitizerKind::Address,
            ref summary,
            ..
        } if summary.contains("heap-use-after-free")
    ));

    // 3. Парсер отчётов ASan распознаёт heap-buffer-overflow и stack-buffer-overflow
    let oob_log = "==9911==ERROR: AddressSanitizer: heap-buffer-overflow on address 0x602000000018 at pc 0x555555801122";
    let parsed = SanitizerRunner::parse_asan_output(oob_log);
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].kind, AsanViolationKind::HeapBufferOverflow);
}

#[test]
fn test_sub_907_2_step_2_2_thread_sanitizer_ring_buffer_concurrency_and_data_race_detection() {
    assert_eq!(TSAN_RUSTFLAG, "-Zsanitizer=thread");
    let env_cfg = SanitizerRunner::env_for(SanitizerKind::Thread);
    assert!(
        env_cfg
            .iter()
            .any(|(k, v)| k == "RUSTFLAGS" && v.contains("-Zsanitizer=thread"))
    );

    // 1. Многопоточный стресс-тест кольцевого буфера AudioRingBuffer + реальный прогон под LLVM TSan
    let ring_probe = ConcurrentRingBufferProbe::run_multithreaded_stress(4_800, 200, 96).expect(
        "Многопоточная передача аудио-фреймов через кольцевой буфер не должна иметь гонок и потерь",
    );
    assert_eq!(ring_probe.frames_produced, 200);
    assert_eq!(ring_probe.frames_consumed, 200);
    assert_eq!(ring_probe.total_samples_verified, 200 * 96);
    assert_eq!(ring_probe.data_races_detected, 0);

    let clean_tsan = SanitizerRunner::run_tsan_ring_buffer_contract()
        .expect("Синхронизированный многопоточный кольцевой буфер должен проходить LLVM TSan");
    assert_eq!(clean_tsan.sanitizer, SanitizerKind::Thread);
    assert!(clean_tsan.passed);
    assert!(clean_tsan.violations.is_empty());

    // 2. Реальный перехват гонки данных (Data Race) через компиляцию с -Zsanitizer=thread
    let race_probe = r#"
static mut UNSYNC_SLOT: u64 = 0;
fn main() {
    let t1 = std::thread::spawn(|| {
        for _ in 0..20_000 {
            unsafe { UNSYNC_SLOT += 1; }
        }
    });
    let t2 = std::thread::spawn(|| {
        for _ in 0..20_000 {
            unsafe { UNSYNC_SLOT += 1; }
        }
    });
    t1.join().unwrap();
    t2.join().unwrap();
}
"#;
    let race_err =
        SanitizerRunner::compile_and_run_with_sanitizer(SanitizerKind::Thread, race_probe)
            .expect_err(
                "LLVM ThreadSanitizer обязан перехватывать несинхронизированную гонку данных",
            );
    assert!(matches!(
        race_err,
        QualityGateError::SanitizerViolationDetected {
            sanitizer: SanitizerKind::Thread,
            ref summary,
            ..
        } if summary.contains("ThreadSanitizer: data race")
    ));

    // 3. Парсер отчётов TSan распознаёт data race и lock-order-inversion
    let tsan_log = "WARNING: ThreadSanitizer: data race (pid=12345)\n  Write of size 8 at 0x555555 by thread T1:";
    let parsed = SanitizerRunner::parse_tsan_output(tsan_log);
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].kind, TsanViolationKind::DataRace);
}

#[test]
fn test_sub_907_2_step_2_3_tokio_gpui_event_loop_deadlock_detector_and_lock_order_graph() {
    // 1. Позитивный сценарий: двунаправленный мост Tokio <-> GPUI Event Loop обрабатывает 500 событий без дедлоков
    let bridge_report = TokioGpuiDeadlockDetector::verify_bidirectional_bridge(500, 64, 2_000)
        .expect("Связка Tokio <-> GPUI Event Loop должна обмениваться командами и событиями без дедлоков");
    assert_eq!(bridge_report.commands_dispatched, 500);
    assert_eq!(bridge_report.ui_events_drained, 500);
    assert!(!bridge_report.deadlock_detected);
    assert!(bridge_report.elapsed_ms < 2_000);

    // 2. Граф порядка захвата мьютексов (Wait-For / Lock-Order DAG):
    // Строгий порядок: UiFrameLock -> AppStateLock -> AudioRingLock (ациклический граф)
    let mut clean_graph = LockOrderGraph::new();
    clean_graph
        .record_acquisition(
            "gpui_main",
            &["UiFrameLock", "AppStateLock", "AudioRingLock"],
        )
        .expect("Ациклический порядок захвата локов");
    clean_graph
        .record_acquisition("tokio_worker", &["AppStateLock", "AudioRingLock"])
        .expect("Согласованный порядок захвата локов в воркере Tokio");
    assert!(clean_graph.verify_acyclic().is_ok());

    // 3. Негативный сценарий: инверсия порядка блокировок (ABBA: AppStateLock -> NetSessionLock и NetSessionLock -> AppStateLock)
    let mut cyclic_graph = LockOrderGraph::new();
    cyclic_graph
        .record_acquisition("gpui_ui_thread", &["AppStateLock", "NetSessionLock"])
        .expect("Первая цепочка локов");
    let cycle_err = cyclic_graph
        .record_acquisition("tokio_net_task", &["NetSessionLock", "AppStateLock"])
        .expect_err(
            "Цикл ожидания ABBA между Tokio и GPUI обязан детектироваться как потенциальный дедлок",
        );
    assert!(matches!(
        cycle_err,
        QualityGateError::DeadlockDetected { ref cycle_path, .. }
        if cycle_path.contains("AppStateLock") && cycle_path.contains("NetSessionLock")
    ));
}

// ═══════════════════════════════════════════════════════════════════════════
// SUB-907-3: Мутационный гейт тестирования (5/5 мутаций на модуль, 100% kill rate)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_sub_907_3_step_3_1_and_3_2_five_control_mutations_generated_and_killed_with_restore() {
    assert_eq!(REQUIRED_MUTATIONS_PER_MODULE, 5);
    assert!((REQUIRED_MUTATION_KILL_RATE - 1.0).abs() < f64::EPSILON);

    let target_module_src = r#"
pub const MAX_MESSAGE_CHARS: usize = 2000;

pub fn validate_and_route_message(content: &str, is_authenticated: bool, seq: u64) -> Result<u64, &'static str> {
    if !is_authenticated {
        return Err("unauthenticated");
    }
    if content.is_empty() || content.chars().count() > MAX_MESSAGE_CHARS {
        return Err("invalid_length");
    }
    let next_seq = seq + 1;
    Ok(next_seq)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unauthenticated() {
        assert_eq!(validate_and_route_message("привет", false, 10), Err("unauthenticated"));
    }

    #[test]
    fn enforces_max_2000_chars_and_non_empty() {
        assert_eq!(validate_and_route_message("", true, 10), Err("invalid_length"));
        let exact_2000 = "a".repeat(2000);
        assert_eq!(validate_and_route_message(&exact_2000, true, 10), Ok(11));
        let over_2001 = "a".repeat(2001);
        assert_eq!(validate_and_route_message(&over_2001, true, 10), Err("invalid_length"));
    }

    #[test]
    fn increments_sequence_monotonically() {
        assert_eq!(validate_and_route_message("ok", true, 41), Ok(42));
    }
}
"#;

    // 1. Генерация ровно 5 контрольных мутаций разных классов на изменяемый модуль
    let mutations = MutationGenerator::generate_five_for_module("chat_router", target_module_src)
        .expect("Генератор обязан создать ровно 5 контрольных мутаций");
    assert_eq!(mutations.len(), 5);
    assert_eq!(mutations[0].operator, MutationOperator::ConditionInversion);
    assert_eq!(mutations[1].operator, MutationOperator::ConstantReplacement);
    assert_eq!(mutations[2].operator, MutationOperator::ValidationBypass);
    assert_eq!(
        mutations[3].operator,
        MutationOperator::ArithmeticBoundaryShift
    );
    assert_eq!(
        mutations[4].operator,
        MutationOperator::StateTransitionOmission
    );

    // 2. Реальный прогон таргетированных тестов против каждого из 5 мутантов в песочнице с проверкой восстановления хеша
    let report = MutationGateRunner::run_module_mutation_gate("chat_router", target_module_src)
        .expect("Все 5/5 мутаций должны быть пойманы и убиты таргетированными тестами");
    assert_eq!(report.module_name, "chat_router");
    assert_eq!(report.total_mutations, 5);
    assert_eq!(report.killed_mutations, 5);
    assert_eq!(report.survived_mutations, 0);
    assert!((report.kill_rate - 1.0).abs() < f64::EPSILON);
    assert!(report.source_restored_cleanly);
    assert_eq!(report.original_sha256, report.restored_sha256);
    assert!(
        report
            .outcomes
            .iter()
            .all(|o| o.status == MutantStatus::Killed)
    );
}

#[test]
fn test_sub_907_3_step_3_3_mutation_gate_rejects_surviving_mutant_below_5_of_5() {
    // Слабый тестовый набор, который НЕ проверяет верхнюю границу 2000 символов и инкремент seq:
    let weak_module_src = r#"
pub const MAX_MESSAGE_CHARS: usize = 2000;

pub fn validate_and_route_message(content: &str, is_authenticated: bool, seq: u64) -> Result<u64, &'static str> {
    if !is_authenticated {
        return Err("unauthenticated");
    }
    if content.is_empty() || content.chars().count() > MAX_MESSAGE_CHARS {
        return Err("invalid_length");
    }
    let next_seq = seq + 1;
    Ok(next_seq)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weak_test_only_checks_happy_path_is_ok() {
        assert!(validate_and_route_message("hi", true, 1).is_ok());
    }
}
"#;

    let err = MutationGateRunner::run_module_mutation_gate("weak_router", weak_module_src)
        .expect_err(
            "Если хотя бы 1 мутант из 5 выжил, мутационный гейт обязан завершиться ошибкой",
        );
    match err {
        QualityGateError::MutationGateFailed {
            module,
            killed,
            total,
            survivors,
        } => {
            assert_eq!(module, "weak_router");
            assert_eq!(total, 5);
            assert!(
                killed < 5,
                "Не все 5 мутаций должны быть убиты слабым тестом (убито {killed}/5)"
            );
            assert!(!survivors.is_empty());
        }
        other => panic!("Ожидалась ошибка MutationGateFailed, получено: {other:?}"),
    }
}

#[test]
fn test_task_907_full_automated_quality_gate_pipeline_end_to_end() {
    let root = gpui_root();
    let summary = QualityGatePipeline::run_full_verification(&root).expect(
        "Сквозной гейт качества TASK-907 (Static + ASan/TSan + 5/5 Mutations) должен проходить",
    );

    assert!(summary.clippy_clean);
    assert!(summary.rustfmt_2024_clean);
    assert!(summary.dependency_audit_clean);
    assert!(summary.source_size_limits_clean);
    assert!(summary.asan_clean);
    assert!(summary.tsan_clean);
    assert!(summary.tokio_gpui_deadlock_free);
    assert_eq!(summary.mutations_killed, 5);
    assert_eq!(summary.mutations_total, 5);
    assert_eq!(summary.verdict, "5/5 mutations killed PASS");
}
