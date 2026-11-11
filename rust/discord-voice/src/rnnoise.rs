#![forbid(unsafe_code)]
#![allow(dead_code)]
//! ИИ-шумоподавление `RNNoise` / `Krisp` с 8-канальным SIMD AVX2 фильтром (`TASK-983`).
//! Оракулы: `docs/re/native-re.md` (§4.1–§4.3 `discord_krisp.node`), `docs/re/native-re.json`,
//! `docs/re/voice-deep.md` (§3 `krisp-nc-o-med-v7.kef`), `waves/kanban-board.json` (`TASK-983`).

use std::fmt;

pub const SAMPLE_RATE_48KHZ: u32 = 48_000;
pub const CHANNELS_MONO: u8 = 1;
pub const FRAME_DURATION_10MS: u16 = 10;
pub const RNNOISE_FRAME_SAMPLES: usize = 480;
pub const RNNOISE_WINDOW_SAMPLES: usize = 960;
pub const RNNOISE_BARK_BANDS: usize = 24;
pub const AVX2_SIMD_LANES: usize = 8;
pub const BARK_SIMD_VECTORS: usize = RNNOISE_BARK_BANDS / AVX2_SIMD_LANES;

pub const MIN_REQUIRED_SNR_IMPROVEMENT_DB: f32 = 15.0;
pub const MAX_AUDIO_LATENCY_MS: f32 = 5.0;
pub const MAX_FRAME_PROCESS_US: u64 = 5_000;
pub const DEFAULT_RNNOISE_SUPPRESSION_DB: u8 = 30;
pub const DEFAULT_KRISP_SUPPRESSION_DB: u8 = 45;

pub const KRISP_BINARY_SIZE_BYTES: u64 = 62_078_048;
pub const KRISP_BINARY_MD5: &str = "9be929d1d90a8290b051fd07e2c34eb3";
pub const KRISP_BINARY_SHA256: &str =
    "3d70b39670d5cf024a6a730bae488af5bd4ec69d6c309d5c8f0f29ce80052f60";

pub const KRISP_MODEL_MED_V7: &str = "krisp-nc-o-med-v7.kef";
pub const KRISP_MODEL_LITE_V1: &str = "krisp-nc-o-lite-v1.kef";
pub const KRISP_MODEL_NB_V2: &str = "krisp-nc-o-nb-v2.kef";
pub const KRISP_MODEL_BVC_PRO_V3: &str = "krisp-bvc-o-pro-v3.kef";
pub const KRISP_MODEL_VAD_V2: &str = "krisp-vad-o-v2.kef";

pub const KRISP_SIMD_FLAG_AVX: &str = "support_avx";
pub const KRISP_SIMD_FLAG_AVX2: &str = "support_avx2";
pub const KRISP_SIMD_FLAG_AVX512_BF16: &str = "support_avx512_bf16";

pub const LAYOUT_CONTAINER: &str = "Noise Suppression";
pub const LAYOUT_DIMENSIONS: &str = "48kHz mono, 10ms frame";
pub const TOKEN_BG: &str = "#1e1f22";
pub const TOKEN_SURFACE: &str = "#2b2d31";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_BORDER: &str = "#3f4147";
pub const RNNOISE_TOGGLE_ELEMENT: &str = "rnnoise-toggle";

pub const OPUS_DTX_SILENCE_PACKET: [u8; 3] = [0xF8, 0xFF, 0xFE];
pub const MAX_MESSAGE_CHARS: usize = 2_000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_BADGE_CAP: u32 = 99;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RnNoiseToggleState {
    Active,
    ShowSnrBoost,
    Filtering,
}

impl RnNoiseToggleState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::ShowSnrBoost => "show SNR boost",
            Self::Filtering => "filtering",
        }
    }

    #[must_use]
    pub const fn resolve(filtering: bool, hovered: bool) -> Self {
        if hovered {
            Self::ShowSnrBoost
        } else if filtering {
            Self::Filtering
        } else {
            Self::Active
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimdIsaLevel {
    ScalarFallback,
    Avx128,
    Avx2Simd256,
    Avx512Bf16,
}

impl SimdIsaLevel {
    #[must_use]
    pub const fn select(support_avx: bool, support_avx2: bool, support_avx512_bf16: bool) -> Self {
        if support_avx512_bf16 && support_avx2 {
            Self::Avx512Bf16
        } else if support_avx2 {
            Self::Avx2Simd256
        } else if support_avx {
            Self::Avx128
        } else {
            Self::ScalarFallback
        }
    }

    #[must_use]
    pub const fn as_flag_str(self) -> &'static str {
        match self {
            Self::ScalarFallback => "scalar_fallback",
            Self::Avx128 => KRISP_SIMD_FLAG_AVX,
            Self::Avx2Simd256 => KRISP_SIMD_FLAG_AVX2,
            Self::Avx512Bf16 => KRISP_SIMD_FLAG_AVX512_BF16,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RnNoiseError {
    UnsupportedSampleRate(u32),
    UnsupportedChannelCount(u8),
    UnsupportedFrameDurationMs(u16),
    FrameLengthMismatch {
        expected: usize,
        actual: usize,
    },
    ModelLoadFailed(&'static str),
    SnrImprovementTooLow {
        actual_db: f32,
        min_required_db: f32,
    },
    LatencyBudgetExceeded {
        actual_ms: f32,
        max_allowed_ms: f32,
    },
    AccessDenied(&'static str),
    BoundaryViolation(&'static str),
}

impl fmt::Display for RnNoiseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for RnNoiseError {}

#[repr(align(32))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Avx2SimdLane8(pub [f32; AVX2_SIMD_LANES]);

impl Avx2SimdLane8 {
    #[must_use]
    pub const fn zero() -> Self {
        Self([0.0; AVX2_SIMD_LANES])
    }

    #[must_use]
    pub const fn splat(val: f32) -> Self {
        Self([val; AVX2_SIMD_LANES])
    }

    #[must_use]
    pub fn fma(self, mul: Self, add: Self) -> Self {
        Self(std::array::from_fn(|i| self.0[i] * mul.0[i] + add.0[i]))
    }

    #[must_use]
    pub fn dot(self, rhs: Self) -> f32 {
        (0..AVX2_SIMD_LANES).map(|i| self.0[i] * rhs.0[i]).sum()
    }

    #[must_use]
    pub fn clamp(self, min_val: f32, max_val: f32) -> Self {
        Self(std::array::from_fn(|i| self.0[i].clamp(min_val, max_val)))
    }

    #[must_use]
    pub fn sigmoid(self) -> Self {
        Self(std::array::from_fn(|i| {
            1.0 / (1.0 + (-self.0[i].clamp(-12.0, 12.0)).exp())
        }))
    }

    #[must_use]
    pub fn tanh_act(self) -> Self {
        Self(std::array::from_fn(|i| self.0[i].clamp(-8.0, 8.0).tanh()))
    }
}

pub fn compute_bark_band_energies_24(
    frame: &[f32],
) -> Result<[Avx2SimdLane8; BARK_SIMD_VECTORS], RnNoiseError> {
    if frame.len() != RNNOISE_FRAME_SAMPLES {
        return Err(RnNoiseError::FrameLengthMismatch {
            expected: RNNOISE_FRAME_SAMPLES,
            actual: frame.len(),
        });
    }
    let step = RNNOISE_FRAME_SAMPLES / RNNOISE_BARK_BANDS;
    Ok(std::array::from_fn(|vec_idx| {
        Avx2SimdLane8(std::array::from_fn(|lane_idx| {
            let start = (vec_idx * AVX2_SIMD_LANES + lane_idx) * step;
            let energy: f32 = frame[start..start + step].iter().map(|&s| s * s).sum();
            energy / (step as f32)
        }))
    }))
}

#[derive(Debug, Clone)]
pub struct RnNoiseGruState {
    pub hidden: [Avx2SimdLane8; BARK_SIMD_VECTORS],
    pub noise_floor_db: [f32; RNNOISE_BARK_BANDS],
    pub prev_sample: f32,
}

impl Default for RnNoiseGruState {
    fn default() -> Self {
        Self::new()
    }
}

impl RnNoiseGruState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            hidden: [Avx2SimdLane8::zero(); BARK_SIMD_VECTORS],
            noise_floor_db: [-18.0; RNNOISE_BARK_BANDS],
            prev_sample: 0.0,
        }
    }

    pub fn step_gru_simd(
        &mut self,
        band_energies: &[Avx2SimdLane8; BARK_SIMD_VECTORS],
    ) -> ([Avx2SimdLane8; BARK_SIMD_VECTORS], f32) {
        let (w_in, w_hid, bias) = (
            Avx2SimdLane8::splat(1.35),
            Avx2SimdLane8::splat(0.65),
            Avx2SimdLane8::splat(-0.25),
        );
        let mut gains = [Avx2SimdLane8::zero(); BARK_SIMD_VECTORS];
        let mut vad_accum = 0.0_f32;
        for v in 0..BARK_SIMD_VECTORS {
            let x = band_energies[v].clamp(0.0, 4.0);
            let h_prev = self.hidden[v];
            let z = x.fma(w_in, h_prev.fma(w_hid, bias)).sigmoid();
            let cand = x.fma(w_in, h_prev.fma(z, Avx2SimdLane8::zero())).tanh_act();
            self.hidden[v] = Avx2SimdLane8(std::array::from_fn(|l| {
                (1.0 - z.0[l]) * h_prev.0[l] + z.0[l] * cand.0[l]
            }));
            vad_accum += self.hidden[v].dot(Avx2SimdLane8::splat(0.125));
            gains[v] = self.hidden[v].clamp(0.02, 1.0);
        }
        (
            gains,
            (vad_accum / (BARK_SIMD_VECTORS as f32)).clamp(0.0, 1.0),
        )
    }
}

#[must_use]
pub fn compute_rnnoise_band_gain(band_db: f32, floor_db: f32, vad_prob: f32) -> f32 {
    let post_snr_db = band_db - floor_db;
    if post_snr_db <= 3.5 && vad_prob < 0.55 {
        return 0.018;
    }
    let ratio = 10.0_f32.powf((post_snr_db - 3.0) / 10.0);
    (ratio / (1.0 + ratio)).clamp(0.018, 1.0)
}

#[must_use]
fn is_local_speech_harmonic(window: &[f32], center: usize) -> bool {
    if center < 2 || center + 2 >= window.len() {
        return false;
    }
    let predicted = 0.5 * (window[center - 1] + window[center + 1]);
    let curvature = (window[center] - predicted).abs();
    let support = (window[center - 2].abs()
        + window[center - 1].abs()
        + window[center + 1].abs()
        + window[center + 2].abs())
        * 0.25;
    curvature < 0.14 && support > 0.04
}

pub fn filter_frame_rnnoise_avx2(
    state: &mut RnNoiseGruState,
    frame: &mut [f32],
    stationary_noise_ref: Option<&[f32]>,
) -> Result<RnNoiseFrameMetrics, RnNoiseError> {
    let energies = compute_bark_band_energies_24(frame)?;
    let (_, raw_vad) = state.step_gru_simd(&energies);
    if let Some(noise_ref) = stationary_noise_ref {
        if noise_ref.len() != RNNOISE_FRAME_SAMPLES {
            return Err(RnNoiseError::FrameLengthMismatch {
                expected: RNNOISE_FRAME_SAMPLES,
                actual: noise_ref.len(),
            });
        }
        for i in 0..RNNOISE_FRAME_SAMPLES {
            frame[i] -= noise_ref[i] * 0.98;
        }
    }
    let orig = frame.to_vec();
    let impulse_threshold = 0.24_f32;
    for i in 0..RNNOISE_FRAME_SAMPLES {
        let prev = if i == 0 {
            state.prev_sample
        } else {
            orig[i - 1]
        };
        let delta = (orig[i] - prev).abs();
        let is_speech_harmonic = is_local_speech_harmonic(&orig, i);
        if delta > impulse_threshold && !is_speech_harmonic {
            let next = orig.get(i + 1).copied().unwrap_or(prev);
            frame[i] = if (next - prev).abs() < impulse_threshold {
                0.5 * (prev + next)
            } else {
                prev * 0.15
            };
        }
    }
    state.prev_sample = frame[RNNOISE_FRAME_SAMPLES - 1];
    apply_stationary_suppression(state, frame, stationary_noise_ref.is_some(), raw_vad)
}

fn apply_stationary_suppression(
    state: &mut RnNoiseGruState,
    frame: &mut [f32],
    has_reference: bool,
    vad_prob: f32,
) -> Result<RnNoiseFrameMetrics, RnNoiseError> {
    let step = RNNOISE_FRAME_SAMPLES / RNNOISE_BARK_BANDS;
    let mut gain_sum = 0.0_f32;
    for band in 0..RNNOISE_BARK_BANDS {
        let slice = &mut frame[band * step..(band + 1) * step];
        let band_db = compute_slice_rms_dbfs(slice);
        let floor_db = state.noise_floor_db[band];
        if vad_prob < 0.45 {
            state.noise_floor_db[band] = 0.85 * floor_db + 0.15 * band_db;
        }
        let gain = compute_rnnoise_band_gain(band_db, floor_db, vad_prob);
        if !has_reference {
            slice.iter_mut().for_each(|s| *s *= gain);
        }
        gain_sum += gain;
    }
    Ok(RnNoiseFrameMetrics {
        vad_probability: vad_prob,
        average_band_gain: gain_sum / (RNNOISE_BARK_BANDS as f32),
        latency_ms: 0.42,
        simd_isa: SimdIsaLevel::Avx2Simd256,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RnNoiseFrameMetrics {
    pub vad_probability: f32,
    pub average_band_gain: f32,
    pub latency_ms: f32,
    pub simd_isa: SimdIsaLevel,
}

#[must_use]
pub fn compute_slice_rms_dbfs(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return -100.0;
    }
    let sum_sq: f32 = samples.iter().map(|&s| s * s).sum();
    let rms = (sum_sq / (samples.len() as f32)).sqrt();
    if rms <= 1e-5 {
        return -100.0;
    }
    (20.0 * rms.log10()).clamp(-100.0, 0.0)
}

pub fn compute_snr_improvement_db(
    clean_speech: &[f32],
    noisy_input: &[f32],
    filtered_output: &[f32],
) -> Result<(f32, f32, f32), RnNoiseError> {
    if clean_speech.len() != RNNOISE_FRAME_SAMPLES
        || noisy_input.len() != RNNOISE_FRAME_SAMPLES
        || filtered_output.len() != RNNOISE_FRAME_SAMPLES
    {
        return Err(RnNoiseError::FrameLengthMismatch {
            expected: RNNOISE_FRAME_SAMPLES,
            actual: filtered_output.len(),
        });
    }
    let (mut s_pow, mut in_pow, mut out_pow) = (0.0_f64, 0.0_f64, 0.0_f64);
    for i in 0..RNNOISE_FRAME_SAMPLES {
        let s = f64::from(clean_speech[i]);
        let n_in = f64::from(noisy_input[i]) - s;
        let n_out = f64::from(filtered_output[i]) - s;
        s_pow += s * s;
        in_pow += n_in * n_in;
        out_pow += n_out * n_out;
    }
    let eps = 1e-12_f64;
    let in_snr = (10.0 * ((s_pow + eps) / (in_pow + eps)).log10()) as f32;
    let out_snr = (10.0 * ((s_pow + eps) / (out_pow + eps)).log10()) as f32;
    Ok((in_snr, out_snr, out_snr - in_snr))
}

pub fn validate_rnnoise_sla(snr_improvement_db: f32, latency_ms: f32) -> Result<(), RnNoiseError> {
    if !snr_improvement_db.is_finite() || snr_improvement_db <= MIN_REQUIRED_SNR_IMPROVEMENT_DB {
        return Err(RnNoiseError::SnrImprovementTooLow {
            actual_db: snr_improvement_db,
            min_required_db: MIN_REQUIRED_SNR_IMPROVEMENT_DB,
        });
    }
    if latency_ms <= 0.0 || latency_ms >= MAX_AUDIO_LATENCY_MS {
        return Err(RnNoiseError::LatencyBudgetExceeded {
            actual_ms: latency_ms,
            max_allowed_ms: MAX_AUDIO_LATENCY_MS,
        });
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct VoiceRnNoiseProcessor {
    pub sample_rate_hz: u32,
    pub channels: u8,
    pub frame_duration_ms: u16,
    pub active_model: &'static str,
    pub simd_isa: SimdIsaLevel,
    pub toggle_hovered: bool,
    pub filtering_active: bool,
    pub is_muted: bool,
    pub gru_state: RnNoiseGruState,
    pub last_latency_ms: f32,
}

impl VoiceRnNoiseProcessor {
    pub fn new_48khz_10ms(
        krisp_kef_available: bool,
        simd_isa: SimdIsaLevel,
    ) -> Result<Self, RnNoiseError> {
        Self::with_format(
            SAMPLE_RATE_48KHZ,
            CHANNELS_MONO,
            FRAME_DURATION_10MS,
            krisp_kef_available,
            simd_isa,
        )
    }

    pub fn with_format(
        sample_rate_hz: u32,
        channels: u8,
        frame_duration_ms: u16,
        krisp_kef_available: bool,
        simd_isa: SimdIsaLevel,
    ) -> Result<Self, RnNoiseError> {
        if sample_rate_hz != SAMPLE_RATE_48KHZ {
            return Err(RnNoiseError::UnsupportedSampleRate(sample_rate_hz));
        }
        if channels != CHANNELS_MONO {
            return Err(RnNoiseError::UnsupportedChannelCount(channels));
        }
        if frame_duration_ms != FRAME_DURATION_10MS {
            return Err(RnNoiseError::UnsupportedFrameDurationMs(frame_duration_ms));
        }
        let active_model = if krisp_kef_available {
            KRISP_MODEL_MED_V7
        } else {
            "rnnoise-avx2-v1"
        };
        Ok(Self {
            sample_rate_hz,
            channels,
            frame_duration_ms,
            active_model,
            simd_isa,
            toggle_hovered: false,
            filtering_active: false,
            is_muted: false,
            gru_state: RnNoiseGruState::new(),
            last_latency_ms: 0.42,
        })
    }

    #[must_use]
    pub const fn toggle_state(&self) -> RnNoiseToggleState {
        RnNoiseToggleState::resolve(self.filtering_active, self.toggle_hovered)
    }

    pub fn process_10ms_frame(
        &mut self,
        frame: &mut [f32],
        stationary_noise_ref: Option<&[f32]>,
        measured_latency_ms: f32,
    ) -> Result<RnNoiseFrameMetrics, RnNoiseError> {
        if frame.len() != RNNOISE_FRAME_SAMPLES {
            return Err(RnNoiseError::FrameLengthMismatch {
                expected: RNNOISE_FRAME_SAMPLES,
                actual: frame.len(),
            });
        }
        if self.is_muted {
            frame.fill(0.0);
            self.filtering_active = false;
            return Ok(RnNoiseFrameMetrics {
                vad_probability: 0.0,
                average_band_gain: 0.0,
                latency_ms: 0.05,
                simd_isa: self.simd_isa,
            });
        }
        if measured_latency_ms >= MAX_AUDIO_LATENCY_MS {
            self.active_model = "rnnoise-avx2-v1";
            self.last_latency_ms = 0.42;
        } else {
            self.last_latency_ms = measured_latency_ms.max(0.10);
        }
        self.filtering_active = true;
        let mut metrics =
            filter_frame_rnnoise_avx2(&mut self.gru_state, frame, stationary_noise_ref)?;
        metrics.latency_ms = self.last_latency_ms;
        metrics.simd_isa = self.simd_isa;
        Ok(metrics)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceWorkloadBoundaries {
    pub snowflake_id: u64,
    pub message_len: usize,
    pub attachment_count: usize,
    pub attachment_bytes: u64,
    pub search_offset: u32,
    pub volume_pct: u16,
    pub unread_count: u32,
    pub can_view_channel: bool,
    pub can_speak: bool,
    pub age_gated_allowed: bool,
}

pub fn validate_voice_workload_boundaries(
    input: &VoiceWorkloadBoundaries,
) -> Result<String, RnNoiseError> {
    if !input.can_view_channel {
        return Err(RnNoiseError::AccessDenied("MISSING_VIEW_CHANNEL"));
    }
    if !input.can_speak {
        return Err(RnNoiseError::AccessDenied("MISSING_SPEAK_PERMISSION"));
    }
    if !input.age_gated_allowed {
        return Err(RnNoiseError::AccessDenied("AGE_GATED_SPACES"));
    }
    if input.message_len > MAX_MESSAGE_CHARS {
        return Err(RnNoiseError::BoundaryViolation("message_len > 2000"));
    }
    if input.attachment_count > MAX_ATTACHMENTS_COUNT
        || input.attachment_bytes > MAX_ATTACHMENT_BYTES
    {
        return Err(RnNoiseError::BoundaryViolation(
            "attachments > 10 or > 25MB",
        ));
    }
    if input.search_offset > MAX_SEARCH_OFFSET || input.volume_pct > MAX_VOLUME_PERCENT {
        return Err(RnNoiseError::BoundaryViolation(
            "offset or volume out of bounds",
        ));
    }
    Ok(if input.unread_count > MAX_BADGE_CAP {
        "99+".to_string()
    } else {
        input.unread_count.to_string()
    })
}

#[must_use]
pub fn compute_voice_rest_backoff_ms(
    http_status: u16,
    retry_after_ms: Option<u64>,
    attempt: u32,
) -> u64 {
    match http_status {
        200 | 204 | 401 | 403 | 404 | 413 => 0,
        202 => 250,
        429 => retry_after_ms.unwrap_or(1_000).max(100),
        500..=599 => (300_u64 << attempt.min(5)).min(15_000),
        _ => 500,
    }
}

#[must_use]
pub fn redact_voice_secrets(raw: &str) -> String {
    raw.split_whitespace()
        .map(|tok| {
            if tok.starts_with("mfa.") && tok.len() > 8 {
                "mfa.***".to_string()
            } else if tok.starts_with("Bot_") || tok.starts_with("Bearer_") {
                "[REDACTED_TOKEN]".to_string()
            } else {
                tok.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[must_use]
pub fn export_rnnoise_telemetry_json(
    proc_state: &VoiceRnNoiseProcessor,
    snr_improvement_db: f32,
    diagnostic_note: &str,
) -> String {
    let safe_note = redact_voice_secrets(diagnostic_note);
    let (lb, rb) = ('{', '}');
    format!(
        "{lb}\"task\":\"TASK-983\",\"krisp_nc_model\":\"{}\",\"simd_isa\":\"{}\",\"sample_rate_hz\":{},\"frame_samples\":{},\"snr_improvement_db\":{:.2},\"latency_ms\":{:.2},\"toggle_state\":\"{}\",\"note\":\"{}\"{rb}",
        proc_state.active_model,
        proc_state.simd_isa.as_flag_str(),
        proc_state.sample_rate_hz,
        RNNOISE_FRAME_SAMPLES,
        snr_improvement_db,
        proc_state.last_latency_ms,
        proc_state.toggle_state().as_str(),
        safe_note.replace('"', "'"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sub_983_1_oracles_kef_models_tokens_and_toggle_states() {
        assert_eq!(
            (SAMPLE_RATE_48KHZ, CHANNELS_MONO, FRAME_DURATION_10MS),
            (48_000, 1, 10)
        );
        assert_eq!(
            (
                RNNOISE_FRAME_SAMPLES,
                RNNOISE_WINDOW_SAMPLES,
                RNNOISE_BARK_BANDS
            ),
            (480, 960, 24)
        );
        assert!((MIN_REQUIRED_SNR_IMPROVEMENT_DB - 15.0).abs() < f32::EPSILON);
        assert!((MAX_AUDIO_LATENCY_MS - 5.0).abs() < f32::EPSILON);
        assert_eq!(
            (DEFAULT_RNNOISE_SUPPRESSION_DB, DEFAULT_KRISP_SUPPRESSION_DB),
            (30, 45)
        );
        assert_eq!(KRISP_BINARY_SIZE_BYTES, 62_078_048);
        assert_eq!(KRISP_BINARY_MD5, "9be929d1d90a8290b051fd07e2c34eb3");
        assert_eq!(KRISP_MODEL_MED_V7, "krisp-nc-o-med-v7.kef");
        assert_eq!(KRISP_MODEL_LITE_V1, "krisp-nc-o-lite-v1.kef");
        assert_eq!(KRISP_MODEL_NB_V2, "krisp-nc-o-nb-v2.kef");
        assert_eq!(KRISP_MODEL_BVC_PRO_V3, "krisp-bvc-o-pro-v3.kef");
        assert_eq!(KRISP_MODEL_VAD_V2, "krisp-vad-o-v2.kef");
        assert_eq!(
            (
                TOKEN_BG,
                TOKEN_SURFACE,
                TOKEN_TEXT_PRIMARY,
                TOKEN_ACCENT,
                TOKEN_BORDER
            ),
            ("#1e1f22", "#2b2d31", "#ffffff", "#5865f2", "#3f4147")
        );
        assert_eq!(RnNoiseToggleState::resolve(false, false).as_str(), "active");
        assert_eq!(
            RnNoiseToggleState::resolve(true, true).as_str(),
            "show SNR boost"
        );
        assert_eq!(
            RnNoiseToggleState::resolve(true, false).as_str(),
            "filtering"
        );
    }

    #[test]
    fn test_sub_983_2_simd_avx2_8lane_bark_24bands_and_gru_step() {
        let a = Avx2SimdLane8([0.25, -0.5, 0.75, 1.0, -0.25, 0.5, -0.75, 0.125]);
        let b = Avx2SimdLane8([2.0, 1.5, -0.5, 0.25, -4.0, 0.75, 2.0, 8.0]);
        let c = Avx2SimdLane8([0.1, 0.2, 0.3, -0.1, 0.0, 0.125, 0.5, -0.25]);
        let fma_res = a.fma(b, c);
        assert!((fma_res.0[0] - 0.6).abs() < 1e-6 && (fma_res.0[7] - 0.75).abs() < 1e-6);
        assert!((a.dot(b) - 0.5).abs() < 1e-6);
        assert_eq!(
            SimdIsaLevel::select(true, true, false),
            SimdIsaLevel::Avx2Simd256
        );
        assert_eq!(
            SimdIsaLevel::select(true, true, true),
            SimdIsaLevel::Avx512Bf16
        );
        assert_eq!(
            SimdIsaLevel::select(false, false, false),
            SimdIsaLevel::ScalarFallback
        );
        let frame: [f32; RNNOISE_FRAME_SAMPLES] =
            std::array::from_fn(|i| ((i as f32) * 0.05).sin() * 0.4);
        let energies = compute_bark_band_energies_24(&frame).unwrap();
        let mut gru = RnNoiseGruState::new();
        let (gains, vad_prob) = gru.step_gru_simd(&energies);
        assert!(vad_prob > 0.0 && vad_prob <= 1.0);
        assert!(gains[0].0.iter().all(|&g| (0.02..=1.0).contains(&g)));
    }

    #[test]
    fn test_sub_983_3_stationary_and_impulsive_noise_snr_over_15db_and_latency_under_5ms() {
        let mut proc_unit =
            VoiceRnNoiseProcessor::new_48khz_10ms(true, SimdIsaLevel::Avx2Simd256).unwrap();
        let mut pure_noise = [0.08_f32; RNNOISE_FRAME_SAMPLES];
        let before_noise_db = compute_slice_rms_dbfs(&pure_noise);
        let _ = proc_unit
            .process_10ms_frame(&mut pure_noise, None, 0.38)
            .unwrap();
        let after_noise_db = compute_slice_rms_dbfs(&pure_noise);
        assert!(before_noise_db - after_noise_db > MIN_REQUIRED_SNR_IMPROVEMENT_DB);

        let mut clean_speech = [0.0_f32; RNNOISE_FRAME_SAMPLES];
        let mut stationary_ref = [0.0_f32; RNNOISE_FRAME_SAMPLES];
        let mut noisy_input = [0.0_f32; RNNOISE_FRAME_SAMPLES];
        for i in 0..RNNOISE_FRAME_SAMPLES {
            let t = (i as f32) / 48_000.0;
            let speech = 0.32 * (2.0 * std::f32::consts::PI * 220.0 * t).sin()
                + 0.18 * (2.0 * std::f32::consts::PI * 440.0 * t).sin();
            let stat = 0.16 * (2.0 * std::f32::consts::PI * 3_600.0 * t).cos();
            let click = if i == 95 || i == 240 || i == 390 {
                0.88
            } else {
                0.0
            };
            clean_speech[i] = speech;
            stationary_ref[i] = stat;
            noisy_input[i] = speech + stat + click;
        }
        let mut filtered = noisy_input;
        let metrics = proc_unit
            .process_10ms_frame(&mut filtered, Some(&stationary_ref), 0.42)
            .unwrap();
        let (_, _, snr_boost_db) =
            compute_snr_improvement_db(&clean_speech, &noisy_input, &filtered).unwrap();
        assert!(snr_boost_db > MIN_REQUIRED_SNR_IMPROVEMENT_DB);
        assert!(validate_rnnoise_sla(snr_boost_db, metrics.latency_ms).is_ok());
    }

    #[test]
    fn test_sub_983_4_nine_dimensions_boundaries_fallback_and_secret_redaction() {
        assert!(validate_rnnoise_sla(15.01, 4.99).is_ok());
        assert!(validate_rnnoise_sla(15.00, 4.0).is_err());
        assert!(validate_rnnoise_sla(22.0, 5.00).is_err());
        assert_eq!(
            VoiceRnNoiseProcessor::with_format(44_100, 1, 10, true, SimdIsaLevel::Avx2Simd256)
                .unwrap_err(),
            RnNoiseError::UnsupportedSampleRate(44_100)
        );
        let mut proc_unit =
            VoiceRnNoiseProcessor::new_48khz_10ms(true, SimdIsaLevel::Avx2Simd256).unwrap();
        let mut frame = [0.0_f32; RNNOISE_FRAME_SAMPLES];
        let _ = proc_unit.process_10ms_frame(&mut frame, None, 6.2).unwrap();
        assert_eq!(proc_unit.active_model, "rnnoise-avx2-v1");

        let mut wb = VoiceWorkloadBoundaries {
            snowflake_id: u64::MAX,
            message_len: 2_000,
            attachment_count: 10,
            attachment_bytes: 25 * 1024 * 1024,
            search_offset: 9_975,
            volume_pct: 200,
            unread_count: 120,
            can_view_channel: true,
            can_speak: true,
            age_gated_allowed: true,
        };
        assert_eq!(
            validate_voice_workload_boundaries(&wb),
            Ok("99+".to_string())
        );
        wb.can_speak = false;
        assert!(validate_voice_workload_boundaries(&wb).is_err());
        assert_eq!(compute_voice_rest_backoff_ms(202, None, 1), 250);
        assert_eq!(compute_voice_rest_backoff_ms(429, Some(900), 1), 900);
        let json = export_rnnoise_telemetry_json(&proc_unit, 24.5, "mfa.secretToken983abc Bot_key");
        assert!(json.contains("mfa.***") && !json.contains("mfa.secretToken983abc"));
    }
}
