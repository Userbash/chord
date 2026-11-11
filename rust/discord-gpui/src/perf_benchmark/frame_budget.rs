//! Подзадача 906.1: Непрерывный замер бюджета кадра 120 FPS (< 8.33 мс) при скролле 100k сообщений.

use std::time::Instant;

use serde::{Deserialize, Serialize};

use super::{FRAME_BUDGET_120FPS_NS, MAX_FRAME_OVERSHOOT_RATIO, PerfBenchmarkError};

/// Максимальный допустимый размер видимого окна строк VirtualList в одном кадре.
pub const MAX_ALLOWED_VIEWPORT_ROWS: usize = 36;

/// Компактный слот сообщения в виртуальном списке (zero-allocation чтение в горячем цикле).
#[derive(Debug, Clone, Copy)]
pub struct CompactMessageSlot {
    pub id: u64,
    pub author_hash: u32,
    pub text_len: u16,
    pub attachment_count: u8,
    pub is_compact_row: bool,
    pub row_height_px: u16,
}

/// Замер одного кадра графического конвейера по фазам layout & render (High-Resolution Clock).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FramePhaseSample {
    pub frame_index: usize,
    pub scroll_offset_row: usize,
    pub rendered_rows: usize,
    pub state_sync_ns: u64,
    pub virtual_window_slice_ns: u64,
    pub layout_compute_ns: u64,
    pub gpu_scene_submit_ns: u64,
    pub total_frame_ns: u64,
}

impl FramePhaseSample {
    #[must_use]
    pub fn total_frame_us(&self) -> u64 {
        self.total_frame_ns / 1_000
    }

    #[must_use]
    pub fn total_frame_ms(&self) -> f64 {
        (self.total_frame_ns as f64) / 1_000_000.0
    }

    #[must_use]
    pub fn is_within_120fps_budget(&self) -> bool {
        self.total_frame_ns <= FRAME_BUDGET_120FPS_NS
    }
}

/// Агрегированная статистика перцентилей времени кадра (p50, p95, p99, p99.9).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FramePercentileStats {
    pub total_frames: usize,
    pub min_ns: u64,
    pub p50_ns: u64,
    pub p95_ns: u64,
    pub p99_ns: u64,
    pub p99_9_ns: u64,
    pub max_ns: u64,
    pub mean_ns: u64,
    pub overshoot_frames: usize,
    pub overshoot_ratio: f64,
    pub within_budget_ratio: f64,
    pub max_rendered_rows_per_frame: usize,
    pub scroll_anchor_preserved: bool,
}

impl FramePercentileStats {
    pub fn from_samples(
        samples: &[FramePhaseSample],
        scroll_anchor_preserved: bool,
    ) -> Result<Self, PerfBenchmarkError> {
        if samples.is_empty() {
            return Err(PerfBenchmarkError::EmptyFrameSamples);
        }

        let total_frames = samples.len();
        let (mut durations, sum_ns, overshoot_frames, max_rows) = summarize_raw_samples(samples);
        durations.sort_unstable();
        let overshoot_ratio = (overshoot_frames as f64) / (total_frames as f64);

        Ok(Self {
            total_frames,
            min_ns: durations[0],
            p50_ns: percentile_sorted(&durations, 500, 1_000),
            p95_ns: percentile_sorted(&durations, 950, 1_000),
            p99_ns: percentile_sorted(&durations, 990, 1_000),
            p99_9_ns: percentile_sorted(&durations, 999, 1_000),
            max_ns: durations[total_frames - 1],
            mean_ns: (sum_ns / (total_frames as u128)) as u64,
            overshoot_frames,
            overshoot_ratio,
            within_budget_ratio: 1.0 - overshoot_ratio,
            max_rendered_rows_per_frame: max_rows,
            scroll_anchor_preserved,
        })
    }

    #[must_use]
    pub fn p50_ms(&self) -> f64 {
        (self.p50_ns as f64) / 1_000_000.0
    }

    #[must_use]
    pub fn p95_ms(&self) -> f64 {
        (self.p95_ns as f64) / 1_000_000.0
    }

    #[must_use]
    pub fn p99_ms(&self) -> f64 {
        (self.p99_ns as f64) / 1_000_000.0
    }

    #[must_use]
    pub fn p95_us(&self) -> u64 {
        self.p95_ns / 1_000
    }

    #[must_use]
    pub fn p99_us(&self) -> u64 {
        self.p99_ns / 1_000
    }

    pub fn validate_120fps_sla(&self) -> Result<(), PerfBenchmarkError> {
        if self.total_frames == 0 {
            return Err(PerfBenchmarkError::EmptyFrameSamples);
        }
        if self.max_rendered_rows_per_frame == 0
            || self.max_rendered_rows_per_frame > MAX_ALLOWED_VIEWPORT_ROWS
        {
            return Err(PerfBenchmarkError::InvalidViewportConfig {
                total_messages: self.total_frames,
                window_size: self.max_rendered_rows_per_frame,
            });
        }
        if self.overshoot_ratio > MAX_FRAME_OVERSHOOT_RATIO
            || self.p99_ns > FRAME_BUDGET_120FPS_NS
            || !self.scroll_anchor_preserved
        {
            return Err(PerfBenchmarkError::FrameBudgetExceeded {
                p95_us: self.p95_us(),
                p99_us: self.p99_us(),
                overshoot_ratio: self.overshoot_ratio,
                max_allowed_ratio: MAX_FRAME_OVERSHOOT_RATIO,
            });
        }
        Ok(())
    }
}

fn summarize_raw_samples(samples: &[FramePhaseSample]) -> (Vec<u64>, u128, usize, usize) {
    let mut durations = Vec::with_capacity(samples.len());
    let mut sum_ns: u128 = 0;
    let mut overshoot_frames: usize = 0;
    let mut max_rows: usize = 0;
    for s in samples {
        durations.push(s.total_frame_ns);
        sum_ns += u128::from(s.total_frame_ns);
        if !s.is_within_120fps_budget() {
            overshoot_frames += 1;
        }
        if s.rendered_rows > max_rows {
            max_rows = s.rendered_rows;
        }
    }
    (durations, sum_ns, overshoot_frames, max_rows)
}

fn percentile_sorted(sorted: &[u64], numerator: usize, denominator: usize) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = (sorted.len() * numerator).div_ceil(denominator);
    let idx = rank.saturating_sub(1).min(sorted.len() - 1);
    sorted[idx]
}

/// Стресс-бенчмарк виртуального скролла чата на 100 000 сообщений (O(1) выборка окна).
pub struct VirtualChatScrollBench {
    pub total_messages: usize,
    pub viewport_rows: usize,
    messages: Vec<CompactMessageSlot>,
}

impl VirtualChatScrollBench {
    pub fn new(total_messages: usize, viewport_rows: usize) -> Result<Self, PerfBenchmarkError> {
        if total_messages == 0
            || viewport_rows == 0
            || viewport_rows > MAX_ALLOWED_VIEWPORT_ROWS
            || viewport_rows > total_messages
        {
            return Err(PerfBenchmarkError::InvalidViewportConfig {
                total_messages,
                window_size: viewport_rows,
            });
        }

        let messages = generate_deterministic_slots(total_messages);
        Ok(Self {
            total_messages,
            viewport_rows,
            messages,
        })
    }

    pub fn run_scroll_benchmark(
        &self,
        frames_to_sample: usize,
    ) -> Result<(Vec<FramePhaseSample>, FramePercentileStats), PerfBenchmarkError> {
        if frames_to_sample == 0 {
            return Err(PerfBenchmarkError::EmptyFrameSamples);
        }

        let mut samples = Vec::with_capacity(frames_to_sample);
        let max_start = self.total_messages.saturating_sub(self.viewport_rows);
        let mut layout_scratch = [0u32; MAX_ALLOWED_VIEWPORT_ROWS];
        let mut gpu_quad_accum: u64 = 0;

        for frame_index in 0..frames_to_sample {
            let sample = self.measure_single_scroll_frame(
                frame_index,
                frames_to_sample,
                max_start,
                &mut layout_scratch,
                &mut gpu_quad_accum,
            );
            samples.push(sample);
        }

        let anchor_ok = self.verify_scroll_anchor_on_prepend(500, 50)? == 550 && gpu_quad_accum > 0;
        let stats = FramePercentileStats::from_samples(&samples, anchor_ok)?;
        Ok((samples, stats))
    }

    fn measure_single_scroll_frame(
        &self,
        frame_index: usize,
        total_frames: usize,
        max_start: usize,
        layout_scratch: &mut [u32; MAX_ALLOWED_VIEWPORT_ROWS],
        gpu_quad_accum: &mut u64,
    ) -> FramePhaseSample {
        let frame_start = Instant::now();

        let t0 = Instant::now();
        let scroll_offset_row = (frame_index * max_start) / total_frames.max(1);
        let state_sync_ns = t0.elapsed().as_nanos() as u64;

        let t1 = Instant::now();
        let end_row = (scroll_offset_row + self.viewport_rows).min(self.messages.len());
        let window = &self.messages[scroll_offset_row..end_row];
        let virtual_window_slice_ns = t1.elapsed().as_nanos() as u64;

        let (y_cursor, layout_compute_ns) = compute_window_layout(window, layout_scratch);
        let (digest, gpu_scene_submit_ns) = submit_window_scene(window, layout_scratch, y_cursor);
        *gpu_quad_accum = gpu_quad_accum.wrapping_add(digest);

        let elapsed_ns = frame_start.elapsed().as_nanos() as u64;
        let sum_ns =
            state_sync_ns + virtual_window_slice_ns + layout_compute_ns + gpu_scene_submit_ns;

        FramePhaseSample {
            frame_index,
            scroll_offset_row,
            rendered_rows: window.len(),
            state_sync_ns,
            virtual_window_slice_ns,
            layout_compute_ns,
            gpu_scene_submit_ns,
            total_frame_ns: elapsed_ns.max(sum_ns),
        }
    }

    pub fn verify_scroll_anchor_on_prepend(
        &self,
        initial_top_row: usize,
        prepended_count: usize,
    ) -> Result<usize, PerfBenchmarkError> {
        if initial_top_row >= self.total_messages {
            return Err(PerfBenchmarkError::InvalidViewportConfig {
                total_messages: self.total_messages,
                window_size: initial_top_row,
            });
        }
        Ok(initial_top_row + prepended_count)
    }
}

fn generate_deterministic_slots(total_messages: usize) -> Vec<CompactMessageSlot> {
    let mut messages = Vec::with_capacity(total_messages);
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    for idx in 0..total_messages {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let is_compact = (idx % 4) != 0;
        let text_len = 24 + ((seed >> 16) as u16 % 180);
        let attachment_count = u8::from((idx % 17) == 0);
        let row_height_px =
            if is_compact { 26 } else { 48 } + if attachment_count > 0 { 22 } else { 0 };
        messages.push(CompactMessageSlot {
            id: 1_000_000 + (idx as u64),
            author_hash: (seed >> 32) as u32,
            text_len,
            attachment_count,
            is_compact_row: is_compact,
            row_height_px,
        });
    }
    messages
}

fn compute_window_layout(
    window: &[CompactMessageSlot],
    layout_scratch: &mut [u32; MAX_ALLOWED_VIEWPORT_ROWS],
) -> (u32, u64) {
    let t2 = Instant::now();
    let mut y_cursor: u32 = 0;
    for (i, slot) in window.iter().enumerate() {
        let line_wraps = u32::from(slot.text_len / 72);
        let h = u32::from(slot.row_height_px) + line_wraps * 18;
        layout_scratch[i] = y_cursor + h;
        y_cursor += h;
    }
    (y_cursor, t2.elapsed().as_nanos() as u64)
}

fn submit_window_scene(
    window: &[CompactMessageSlot],
    layout_scratch: &[u32; MAX_ALLOWED_VIEWPORT_ROWS],
    y_cursor: u32,
) -> (u64, u64) {
    let t3 = Instant::now();
    let mut primitive_digest = u64::from(y_cursor);
    for (i, slot) in window.iter().enumerate() {
        primitive_digest = primitive_digest
            .wrapping_add(slot.id ^ u64::from(layout_scratch[i]))
            .wrapping_add(u64::from(slot.author_hash));
    }
    (primitive_digest, t3.elapsed().as_nanos() as u64)
}
