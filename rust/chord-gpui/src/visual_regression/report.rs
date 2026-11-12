use std::path::Path;

use serde::{Deserialize, Serialize};

use super::framebuffer::GpuiFrameBuffer;
use super::masks::DynamicMaskSet;
use super::ssim::SsimComparator;
use super::{GoldenSnapshotId, LlvmPipeCaptureConfig, VisualRegressionError};
use crate::state::AppState;

/// Запись аудита одного эталонного снимка Golden Snapshot
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotAuditEntry {
    pub slug: String,
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub ssim_score: f64,
    pub threshold: f64,
    pub passed: bool,
    pub masked_pixels: usize,
    pub unmasked_diff_pixels: usize,
}

/// Генератор золотых эталонов (Golden Snapshots) и интерактивного HTML-отчёта Visual Diff
pub struct VisualDiffReport;

impl VisualDiffReport {
    pub fn materialize_golden_suite(
        goldens_dir: &Path,
    ) -> Result<Vec<SnapshotAuditEntry>, VisualRegressionError> {
        std::fs::create_dir_all(goldens_dir)
            .map_err(|e| VisualRegressionError::IoError(e.to_string()))?;

        Self::write_mask_files(goldens_dir)?;

        let state = AppState::new();
        let cfg = LlvmPipeCaptureConfig::deterministic();
        let comparator = SsimComparator::new(SsimComparator::DEFAULT_THRESHOLD);
        let mut entries = Vec::with_capacity(GoldenSnapshotId::ALL.len());

        for id in GoldenSnapshotId::ALL {
            let entry = Self::verify_or_write_snapshot(goldens_dir, id, &state, &cfg, &comparator)?;
            entries.push(entry);
        }

        let manifest_json = serde_json::to_string_pretty(&entries)
            .map_err(|e| VisualRegressionError::JsonError(e.to_string()))?;
        std::fs::write(goldens_dir.join("golden_manifest.json"), manifest_json)
            .map_err(|e| VisualRegressionError::IoError(e.to_string()))?;

        let html = Self::generate_html(&entries);
        std::fs::write(goldens_dir.join("visual_diff_report.html"), html)
            .map_err(|e| VisualRegressionError::IoError(e.to_string()))?;

        Ok(entries)
    }

    fn write_mask_files(goldens_dir: &Path) -> Result<(), VisualRegressionError> {
        let masks_720 = DynamicMaskSet::default_for_resolution(1280, 720);
        let masks_1080 = DynamicMaskSet::default_for_resolution(1920, 1080);

        std::fs::write(
            goldens_dir.join("dynamic_masks_1280x720.json"),
            masks_720.to_json_pretty()?,
        )
        .map_err(|e| VisualRegressionError::IoError(e.to_string()))?;

        std::fs::write(
            goldens_dir.join("dynamic_masks_1920x1080.json"),
            masks_1080.to_json_pretty()?,
        )
        .map_err(|e| VisualRegressionError::IoError(e.to_string()))?;

        Ok(())
    }

    fn verify_or_write_snapshot(
        goldens_dir: &Path,
        id: GoldenSnapshotId,
        state: &AppState,
        cfg: &LlvmPipeCaptureConfig,
        comparator: &SsimComparator,
    ) -> Result<SnapshotAuditEntry, VisualRegressionError> {
        let candidate = GpuiFrameBuffer::capture_snapshot(id, state, cfg)?;
        let png_path = goldens_dir.join(format!("{}.png", id.slug()));
        if !png_path.exists() {
            candidate.save_png(&png_path)?;
        }

        let loaded_golden = GpuiFrameBuffer::load_png(&png_path)?;
        let masks = DynamicMaskSet::default_for_resolution(candidate.width, candidate.height);
        let (res, _diff) = comparator.compare(&loaded_golden, &candidate, &masks)?;

        Ok(SnapshotAuditEntry {
            slug: id.slug().to_string(),
            title: id.title().to_string(),
            width: candidate.width,
            height: candidate.height,
            ssim_score: res.score,
            threshold: res.threshold,
            passed: res.passed,
            masked_pixels: res.masked_pixels,
            unmasked_diff_pixels: res.unmasked_diff_pixels,
        })
    }

    pub fn generate_html(entries: &[SnapshotAuditEntry]) -> String {
        let total = entries.len();
        let passed_count = entries.iter().filter(|e| e.passed).count();
        let rows_html = Self::render_rows_html(entries);

        format!(
            "<!DOCTYPE html>\n\
<html lang=\"ru\">\n\
<head>\n\
<meta charset=\"utf-8\">\n\
<title>GPUI Visual Regression Report (SSIM &gt; 0.99)</title>\n\
<style>\n\
body {{ background:#1e1f22; color:#ffffff; font-family:sans-serif; margin:24px; }}\n\
.card {{ background:#2b2d31; border:1px solid #3f4147; border-radius:8px; padding:16px; margin-bottom:16px; }}\n\
.badge-pass {{ background:#23a55a; color:#fff; padding:4px 8px; border-radius:4px; font-weight:bold; }}\n\
.badge-fail {{ background:#f23f43; color:#fff; padding:4px 8px; border-radius:4px; font-weight:bold; }}\n\
.controls {{ display:flex; gap:12px; align-items:center; margin:16px 0; background:#313338; padding:12px; border-radius:8px; }}\n\
button {{ background:#eb459e; color:#fff; border:none; padding:8px 14px; border-radius:4px; cursor:pointer; font-weight:bold; }}\n\
table {{ width:100%; border-collapse:collapse; margin-top:12px; }}\n\
th, td {{ padding:10px; border-bottom:1px solid #3f4147; text-align:left; }}\n\
</style>\n\
</head>\n\
<body>\n\
<h1>GPUI Visual Regression &amp; Perceptual Diff (SSIM &gt; 0.99)</h1>\n\
<p>Сводка: <strong>{passed_count}/{total} PASS</strong> | Renderer: <code>llvmpipe</code> | Frozen Clock: <code>2026-11-11T12:00:00Z</code></p>\n\
<div class=\"controls\" data-mode=\"slider\">\n\
  <span>Режим сравнения:</span>\n\
  <button type=\"button\" data-view-mode=\"slider\">Slider</button>\n\
  <button type=\"button\" data-view-mode=\"onion-skin\">Onion Skin</button>\n\
  <button type=\"button\" data-view-mode=\"red-diff\">Red Diff Overlay</button>\n\
  <label>Slider / Onion Opacity: <input type=\"range\" min=\"0\" max=\"100\" value=\"50\"></label>\n\
</div>\n\
<div class=\"card\">\n\
  <table>\n\
    <thead><tr><th>Snapshot</th><th>Описание</th><th>Разрешение</th><th>SSIM</th><th>Маска (px)</th><th>Diff (px)</th><th>Статус</th></tr></thead>\n\
    <tbody>\n{rows_html}    </tbody>\n\
  </table>\n\
</div>\n\
</body>\n\
</html>\n"
        )
    }

    fn render_rows_html(entries: &[SnapshotAuditEntry]) -> String {
        let mut out = String::new();
        for e in entries {
            let status_badge = if e.passed {
                "<span class=\"badge-pass\">PASS</span>"
            } else {
                "<span class=\"badge-fail\">FAIL</span>"
            };
            out.push_str(&format!(
                "      <tr><td><code>{}</code></td><td>{}</td><td>{}x{}</td><td>{:.6}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                e.slug,
                e.title,
                e.width,
                e.height,
                e.ssim_score,
                e.masked_pixels,
                e.unmasked_diff_pixels,
                status_badge
            ));
        }
        out
    }
}
