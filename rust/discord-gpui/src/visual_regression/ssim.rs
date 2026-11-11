use serde::{Deserialize, Serialize};

use super::VisualRegressionError;
use super::framebuffer::GpuiFrameBuffer;
use super::masks::DynamicMaskSet;

/// Результат перцептивного сравнения двух кадров по метрике SSIM (Wang et al., 2004)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SsimResult {
    pub score: f64,
    pub threshold: f64,
    pub passed: bool,
    pub total_pixels: usize,
    pub masked_pixels: usize,
    pub unmasked_diff_pixels: usize,
    pub evaluated_blocks: usize,
    pub min_block_score: f64,
}

#[derive(Debug, Clone, Copy)]
struct BlockAccum {
    n: usize,
    sum_x: f64,
    sum_y: f64,
    sum_xx: f64,
    sum_yy: f64,
    sum_xy: f64,
    diff_pixels: usize,
}

/// Перцептивный компаратор Structural Similarity Index (SSIM) с поддержкой JSON-масок
#[derive(Debug, Clone, Copy)]
pub struct SsimComparator {
    pub threshold: f64,
}

impl SsimComparator {
    pub const DEFAULT_THRESHOLD: f64 = 0.995;
    pub const MIN_CONTRACT_THRESHOLD: f64 = 0.99;
    const BLOCK_SIZE: u32 = 8;
    const C1: f64 = (0.01 * 255.0) * (0.01 * 255.0);
    const C2: f64 = (0.03 * 255.0) * (0.03 * 255.0);

    pub fn new(threshold: f64) -> Self {
        Self {
            threshold: threshold.clamp(0.0, 1.0),
        }
    }

    pub fn compare(
        &self,
        baseline: &GpuiFrameBuffer,
        candidate: &GpuiFrameBuffer,
        masks: &DynamicMaskSet,
    ) -> Result<(SsimResult, GpuiFrameBuffer), VisualRegressionError> {
        if baseline.width != candidate.width || baseline.height != candidate.height {
            return Err(VisualRegressionError::DimensionMismatch {
                expected: (baseline.width, baseline.height),
                actual: (candidate.width, candidate.height),
            });
        }
        masks.validate(baseline.width, baseline.height)?;

        let mask_bitmap = masks.to_bitmap();
        let masked_pixels = mask_bitmap.iter().filter(|&&m| m).count();
        let mut diff_frame = GpuiFrameBuffer::new(baseline.width, baseline.height)?;

        let (score, min_block_score, evaluated_blocks, unmasked_diff_pixels) =
            self.evaluate_blocks(baseline, candidate, &mask_bitmap, &mut diff_frame);

        let passed = score >= self.threshold && score >= Self::MIN_CONTRACT_THRESHOLD;
        let result = SsimResult {
            score,
            threshold: self.threshold,
            passed,
            total_pixels: (baseline.width as usize) * (baseline.height as usize),
            masked_pixels,
            unmasked_diff_pixels,
            evaluated_blocks,
            min_block_score,
        };
        Ok((result, diff_frame))
    }

    fn evaluate_blocks(
        &self,
        baseline: &GpuiFrameBuffer,
        candidate: &GpuiFrameBuffer,
        mask_bitmap: &[bool],
        diff_frame: &mut GpuiFrameBuffer,
    ) -> (f64, f64, usize, usize) {
        let mut ssim_sum = 0.0f64;
        let mut min_block = 1.0f64;
        let mut evaluated_blocks = 0usize;
        let mut total_diff_pixels = 0usize;

        let mut by = 0u32;
        while by < baseline.height {
            let mut bx = 0u32;
            while bx < baseline.width {
                let acc = Self::process_block(bx, by, baseline, candidate, mask_bitmap, diff_frame);
                total_diff_pixels += acc.diff_pixels;
                if acc.n > 0 {
                    let b_score = Self::compute_block_ssim(&acc);
                    ssim_sum += b_score;
                    if b_score < min_block {
                        min_block = b_score;
                    }
                    evaluated_blocks += 1;
                }
                bx += Self::BLOCK_SIZE;
            }
            by += Self::BLOCK_SIZE;
        }

        let final_score = if evaluated_blocks == 0 {
            1.0
        } else {
            (ssim_sum / (evaluated_blocks as f64)).clamp(0.0, 1.0)
        };
        (final_score, min_block, evaluated_blocks, total_diff_pixels)
    }

    fn process_block(
        bx: u32,
        by: u32,
        baseline: &GpuiFrameBuffer,
        candidate: &GpuiFrameBuffer,
        mask_bitmap: &[bool],
        diff_frame: &mut GpuiFrameBuffer,
    ) -> BlockAccum {
        let mut acc = BlockAccum {
            n: 0,
            sum_x: 0.0,
            sum_y: 0.0,
            sum_xx: 0.0,
            sum_yy: 0.0,
            sum_xy: 0.0,
            diff_pixels: 0,
        };
        let x_end = bx.saturating_add(Self::BLOCK_SIZE).min(baseline.width);
        let y_end = by.saturating_add(Self::BLOCK_SIZE).min(baseline.height);
        let w = baseline.width as usize;

        for py in by..y_end {
            for px in bx..x_end {
                let p_idx = (py as usize) * w + (px as usize);
                let b_px = baseline.pixel_at(px, py);
                if mask_bitmap[p_idx] {
                    diff_frame.set_pixel(px, py, [b_px[0] / 3, b_px[1] / 2, b_px[2], 0xff]);
                    continue;
                }
                let c_px = candidate.pixel_at(px, py);
                Self::accumulate_pixel(px, py, b_px, c_px, &mut acc, diff_frame);
            }
        }
        acc
    }

    fn accumulate_pixel(
        px: u32,
        py: u32,
        b_px: [u8; 4],
        c_px: [u8; 4],
        acc: &mut BlockAccum,
        diff_frame: &mut GpuiFrameBuffer,
    ) {
        let lx = rgb_to_luma(b_px);
        let ly = rgb_to_luma(c_px);
        acc.n += 1;
        acc.sum_x += lx;
        acc.sum_y += ly;
        acc.sum_xx += lx * lx;
        acc.sum_yy += ly * ly;
        acc.sum_xy += lx * ly;

        let dr = (b_px[0] as i16 - c_px[0] as i16).unsigned_abs();
        let dg = (b_px[1] as i16 - c_px[1] as i16).unsigned_abs();
        let db = (b_px[2] as i16 - c_px[2] as i16).unsigned_abs();
        if dr > 2 || dg > 2 || db > 2 {
            acc.diff_pixels += 1;
            diff_frame.set_pixel(px, py, [0xff, 0x22, 0x44, 0xff]);
        } else {
            diff_frame.set_pixel(px, py, [b_px[0] / 2, b_px[1] / 2, b_px[2] / 2, 0xff]);
        }
    }

    fn compute_block_ssim(acc: &BlockAccum) -> f64 {
        let nf = acc.n as f64;
        let mu_x = acc.sum_x / nf;
        let mu_y = acc.sum_y / nf;
        let var_x = (acc.sum_xx / nf - mu_x * mu_x).max(0.0);
        let var_y = (acc.sum_yy / nf - mu_y * mu_y).max(0.0);
        let cov_xy = acc.sum_xy / nf - mu_x * mu_y;

        let num = (2.0 * mu_x * mu_y + Self::C1) * (2.0 * cov_xy + Self::C2);
        let den = (mu_x * mu_x + mu_y * mu_y + Self::C1) * (var_x + var_y + Self::C2);
        (num / den).clamp(0.0, 1.0)
    }
}

#[inline]
fn rgb_to_luma(rgba: [u8; 4]) -> f64 {
    0.299 * (rgba[0] as f64) + 0.587 * (rgba[1] as f64) + 0.114 * (rgba[2] as f64)
}
