//! Модуль бенчмаркинга 120 FPS, профилирования кучи (jemalloc/heaptrack),
//! контроля VmRSS (< 60 МБ) и замера холодного старта (< 150 мс) для `discord-gpui` (TASK-906).

pub mod ci_artifacts;
pub mod cold_start;
pub mod frame_budget;
pub mod memory_profiler;

use serde::{Deserialize, Serialize};

/// Целевая частота кадров графического конвейера GPUI (120 FPS).
pub const TARGET_FPS: u32 = 120;

/// Бюджет одного кадра при 120 FPS в наносекундах (8.333 мс = 8_333_333 нс).
pub const FRAME_BUDGET_120FPS_NS: u64 = 8_333_333;

/// Бюджет одного кадра при 120 FPS в микросекундах (8_333 мкс).
pub const FRAME_BUDGET_120FPS_US: u64 = 8_333;

/// Целевой бюджет перцентиля p95 времени кадра в наносекундах (< 2.0 мс).
pub const FRAME_P95_BUDGET_NS: u64 = 2_000_000;

/// Максимально допустимая доля кадров с превышением 8.33 мс (0.1% = 0.001).
pub const MAX_FRAME_OVERSHOOT_RATIO: f64 = 0.001;

/// Жёсткий верхний предел VmRSS всего приложения в килобайтах (60 МБ = 61_440 КБ).
pub const MAX_VMRSS_KB: u64 = 60 * 1024;

/// Жёсткий верхний предел VmRSS всего приложения в мегабайтах (60.0 МБ).
pub const MAX_VMRSS_MB: f64 = 60.0;

/// Бюджет холодного старта до First Meaningful Paint при прогретом кэше SQLite (< 150 мс).
pub const WARM_COLD_START_BUDGET_MS: f64 = 150.0;

/// Бюджет холодного старта до First Meaningful Paint при чистом запуске (< 300 мс).
pub const CLEAN_COLD_START_BUDGET_MS: f64 = 300.0;

/// Целевое количество сообщений в стресс-тесте виртуального скролла (100 000).
pub const SCROLL_WORKLOAD_MESSAGES: usize = 100_000;

/// Количество одновременно рендерящихся элементов в видимом окне VirtualList (~30).
pub const VIRTUAL_VIEWPORT_ROWS: usize = 30;

/// Ошибки бенчмаркинга производительности и профилирования памяти.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PerfBenchmarkError {
    FrameBudgetExceeded {
        p95_us: u64,
        p99_us: u64,
        overshoot_ratio: f64,
        max_allowed_ratio: f64,
    },
    EmptyFrameSamples,
    InvalidViewportConfig {
        total_messages: usize,
        window_size: usize,
    },
    MemoryBudgetExceeded {
        vmrss_kb: u64,
        limit_kb: u64,
    },
    MemoryLeakDetected {
        leaked_bytes: u64,
        slope_bytes_per_cycle: f64,
        tag: String,
    },
    VramBudgetExceeded {
        live_vram_bytes: usize,
        budget_bytes: usize,
    },
    ColdStartBudgetExceeded {
        mode: String,
        elapsed_ms: f64,
        budget_ms: f64,
    },
    ProcParseError(String),
    ProfileFormatError(String),
    IoError(String),
    JsonError(String),
}

impl std::fmt::Display for PerfBenchmarkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for PerfBenchmarkError {}
