//! Публикация графиков и артефактов производительности в CI (Шаг 3.3).

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::PerfBenchmarkError;
use super::cold_start::ColdStartPhaseBreakdown;
use super::frame_budget::FramePercentileStats;
use super::memory_profiler::{HeapAllocationProfiler, ProcMemorySnapshot, StressLeakReport};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CiPerfReport {
    pub task_id: String,
    pub frame_stats: FramePercentileStats,
    pub memory_snapshot: ProcMemorySnapshot,
    pub stress_report: StressLeakReport,
    pub warm_cold_start: ColdStartPhaseBreakdown,
    pub clean_cold_start: ColdStartPhaseBreakdown,
    pub all_gates_passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiPublishedPaths {
    pub json_report_path: PathBuf,
    pub html_dashboard_path: PathBuf,
    pub jemalloc_profile_path: PathBuf,
    pub heaptrack_log_path: PathBuf,
}

pub struct CiPerfArtifactPublisher;

impl CiPerfArtifactPublisher {
    pub fn publish(
        output_dir: &Path,
        report: &CiPerfReport,
        profiler: &HeapAllocationProfiler,
    ) -> Result<CiPublishedPaths, PerfBenchmarkError> {
        fs::create_dir_all(output_dir)
            .map_err(|e| PerfBenchmarkError::IoError(format!("create_dir_all: {e}")))?;

        let json_report_path = output_dir.join("perf-benchmark-report.json");
        let html_dashboard_path = output_dir.join("perf-dashboard.html");
        let jemalloc_profile_path = output_dir.join("jemalloc-heap.prof");
        let heaptrack_log_path = output_dir.join("heaptrack-stress.txt");

        let json_str = serde_json::to_string_pretty(report)
            .map_err(|e| PerfBenchmarkError::JsonError(format!("serde_json: {e}")))?;
        fs::write(&json_report_path, json_str)
            .map_err(|e| PerfBenchmarkError::IoError(format!("write json: {e}")))?;
        fs::write(&html_dashboard_path, Self::render_svg_dashboard(report))
            .map_err(|e| PerfBenchmarkError::IoError(format!("write html: {e}")))?;
        fs::write(&jemalloc_profile_path, profiler.export_jemalloc_profile())
            .map_err(|e| PerfBenchmarkError::IoError(format!("write jemalloc: {e}")))?;
        fs::write(&heaptrack_log_path, profiler.export_heaptrack_log())
            .map_err(|e| PerfBenchmarkError::IoError(format!("write heaptrack: {e}")))?;

        Ok(CiPublishedPaths {
            json_report_path,
            html_dashboard_path,
            jemalloc_profile_path,
            heaptrack_log_path,
        })
    }

    #[must_use]
    pub fn render_svg_dashboard(report: &CiPerfReport) -> String {
        let frame_svg = render_frame_budget_svg(&report.frame_stats);
        let mem_card = render_memory_card_html(report);
        let cold_card = render_cold_start_card_html(report);
        format!(
            "<!DOCTYPE html>\n<html lang=\"ru\">\n<head><meta charset=\"utf-8\" /><title>{id}</title></head>\n<body style=\"background:#1e1f22;color:#ffffff;font-family:sans-serif\">\n<h1>Discord GPUI Performance Suite ({id})</h1>\n{frame_svg}\n{mem_card}\n{cold_card}\n</body>\n</html>",
            id = report.task_id
        )
    }
}

fn render_frame_budget_svg(stats: &FramePercentileStats) -> String {
    format!(
        "<section><h2>120 FPS Frame Budget (&lt; 8.33 ms, 100k messages)</h2><p>p50: {:.3} ms | p95: {:.3} ms | p99: {:.3} ms</p><svg width=\"600\" height=\"90\" viewBox=\"0 0 600 90\"><rect x=\"0\" y=\"0\" width=\"600\" height=\"90\" fill=\"#2b2d31\"/><line x1=\"500\" y1=\"0\" x2=\"500\" y2=\"90\" stroke=\"#f23f43\" stroke-width=\"2\"/><text x=\"506\" y=\"20\" fill=\"#f23f43\">8.33 ms SLA</text></svg></section>",
        stats.p50_ms(),
        stats.p95_ms(),
        stats.p99_ms()
    )
}

fn render_memory_card_html(report: &CiPerfReport) -> String {
    format!(
        "<section><h2>VmRSS &amp; Heap Profiling (jemalloc / heaptrack)</h2><p>VmRSS: {:.2} MB / 60.00 MB | Leaked: {} B | Slope: {:.4} B/cycle</p></section>",
        report.memory_snapshot.vmrss_mb(),
        report.stress_report.leaked_bytes,
        report.stress_report.slope_bytes_per_cycle
    )
}

fn render_cold_start_card_html(report: &CiPerfReport) -> String {
    format!(
        "<section><h2>Cold Start Time (First Meaningful Paint)</h2><p>Warm SQLite: {:.2} ms (&lt; 150 ms) | Clean Boot: {:.2} ms (&lt; 300 ms)</p></section>",
        report.warm_cold_start.total_ms(),
        report.clean_cold_start.total_ms()
    )
}
