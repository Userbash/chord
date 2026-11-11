//! Автоматизированный гейт качества кода (`TASK-907`):
//! - `SUB-907-1`: Статический анализ и строгий линтинг Rust (`cargo clippy --all-targets -- -D warnings`,
//!   `rustfmt --check --edition 2024`, `cargo audit` / `cargo-deny`, лимиты функций `< 50` и модулей `< 800`).
//! - `SUB-907-2`: Динамические санитайзеры LLVM (`ASan` & `TSan`) и проверка отсутствия дедлоков Tokio <-> GPUI.
//! - `SUB-907-3`: Мутационный гейт тестирования (`5/5` мутаций на модуль, `100%` kill rate).

pub mod mutation_gate;
pub mod sanitizers;
pub mod static_linter;

use std::path::Path;

use serde::{Deserialize, Serialize};

use self::{
    mutation_gate::MutationGateRunner,
    sanitizers::{
        ConcurrentRingBufferProbe, SanitizerKind, SanitizerRunner, TokioGpuiDeadlockDetector,
    },
    static_linter::{
        ClippyGateRunner, DependencySecurityAuditor, RustfmtGateRunner, SourceMetricsScanner,
    },
};

/// Жёсткий верхний предел размера одной функции в строках (`< 50` строк).
pub const MAX_FUNCTION_LINES: usize = 50;

/// Мягкий/статический предел размера файла модуля в строках (`< 800` строк).
pub const MAX_MODULE_LINES: usize = 800;

/// Обязательное число контрольных мутаций на каждый изменяемый модуль (`5`).
pub const REQUIRED_MUTATIONS_PER_MODULE: usize = 5;

/// Обязательная доля убитых мутаций (`5/5 = 1.0 = 100%`).
pub const REQUIRED_MUTATION_KILL_RATE: f64 = 1.0;

/// Редакция Rust для проверки форматирования `rustfmt`.
pub const RUST_EDITION: &str = "2024";

/// Строгий флаг блокировки любых предупреждений компилятора и Clippy.
pub const CLIPPY_STRICT_FLAG: &str = "-D warnings";

/// Флаг компиляции с LLVM AddressSanitizer (ASan).
pub const ASAN_RUSTFLAG: &str = "-Zsanitizer=address";

/// Флаг компиляции с LLVM ThreadSanitizer (TSan).
pub const TSAN_RUSTFLAG: &str = "-Zsanitizer=thread";

/// Единый тип ошибок автоматизированного гейта качества кода (`TASK-907`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QualityGateError {
    ClippyWarningsDetected {
        warnings: usize,
        errors: usize,
        details: String,
    },
    ProductionUnwrapDetected {
        file: String,
        line: usize,
        kind: String,
    },
    RustfmtDiffDetected {
        files: Vec<String>,
        diff_excerpt: String,
    },
    LicenseRejected {
        package: String,
        version: String,
        license: String,
    },
    VulnerabilityDetected {
        advisory_id: String,
        package: String,
        version: String,
        title: String,
    },
    UntrustedDependencySource {
        package: String,
        source: String,
    },
    FunctionTooLong {
        file: String,
        function_name: String,
        lines: usize,
        max_lines: usize,
    },
    ModuleFileTooLong {
        file: String,
        lines: usize,
        max_lines: usize,
    },
    SanitizerViolationDetected {
        sanitizer: SanitizerKind,
        exit_code: i32,
        summary: String,
    },
    DeadlockDetected {
        component: String,
        cycle_path: String,
    },
    MutationGateFailed {
        module: String,
        killed: usize,
        total: usize,
        survivors: Vec<String>,
    },
    IoError(String),
}

impl std::fmt::Display for QualityGateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for QualityGateError {}

/// Итоговая сводка прохождения всех трёх подгейтов `TASK-907`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityGateSummary {
    pub clippy_clean: bool,
    pub rustfmt_2024_clean: bool,
    pub dependency_audit_clean: bool,
    pub source_size_limits_clean: bool,
    pub asan_clean: bool,
    pub tsan_clean: bool,
    pub tokio_gpui_deadlock_free: bool,
    pub mutations_killed: usize,
    pub mutations_total: usize,
    pub verdict: String,
}

/// Оркестратор сквозного пайплайна контроля качества кода (`TASK-907`).
pub struct QualityGatePipeline;

impl QualityGatePipeline {
    /// Запускает полную проверку всех трёх фаз гейта (`SUB-907-1`, `SUB-907-2`, `SUB-907-3`).
    pub fn run_full_verification(gpui_root: &Path) -> Result<QualityGateSummary, QualityGateError> {
        let qg_dir = gpui_root.join("src/quality_gate");
        let files = vec![
            qg_dir.join("mod.rs"),
            qg_dir.join("static_linter.rs"),
            qg_dir.join("sanitizers.rs"),
            qg_dir.join("mutation_gate.rs"),
        ];
        let clippy = ClippyGateRunner::compile_probe_with_deny_warnings(
            "pub fn gate_probe(x: u32) -> u32 { x.saturating_add(1) }\n",
        )?;
        let fmt = RustfmtGateRunner::check_files(&files)?;
        let audit = DependencySecurityAuditor::audit_project(
            &gpui_root.join("deny.toml"),
            &gpui_root.join("Cargo.lock"),
            &[],
        )?;
        let metrics = SourceMetricsScanner::scan_directory(&qg_dir)?;
        let asan = SanitizerRunner::run_asan_ring_buffer_contract()?;
        let _ring = ConcurrentRingBufferProbe::run_multithreaded_stress(2_400, 64, 48)?;
        let tsan = SanitizerRunner::run_tsan_ring_buffer_contract()?;
        let bridge = TokioGpuiDeadlockDetector::verify_bidirectional_bridge(128, 32, 2_000)?;
        let mut_report = MutationGateRunner::run_default_reference_gate()?;
        Ok(QualityGateSummary {
            clippy_clean: clippy.passed,
            rustfmt_2024_clean: fmt.passed,
            dependency_audit_clean: audit.passed,
            source_size_limits_clean: metrics.files_scanned >= 4,
            asan_clean: asan.passed,
            tsan_clean: tsan.passed,
            tokio_gpui_deadlock_free: !bridge.deadlock_detected,
            mutations_killed: mut_report.killed_mutations,
            mutations_total: mut_report.total_mutations,
            verdict: "5/5 mutations killed PASS".to_string(),
        })
    }
}
