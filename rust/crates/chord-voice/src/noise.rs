//! Шумоподавление (`Krisp` / `RNNoise` / `StandardWebRtc`) и Voice Activity Detection (VAD) гейтинг.
//!
//! Оракулы RE (`/home/sanya/re-deep/`):
//! - `voice-deep.md` (§3, строка 110): `[VOICE_DISCONNECT] krisp_nc_model: krisp-nc-o-med-v7.kef`.
//! - `native-re.md` (§4.1 `discord_krisp.node`): модели `.kef` (`krisp-nc-o-med-v7.kef`,
//!   `krisp-nc-o-lite-v1.kef`, `krisp-nc-o-nb-v2.kef`, `krisp-bvc-o-pro-v3.kef`, `krisp-vad-o-v2.kef`),
//!   уровень подавления `setSuppressionLevel` (`0..=100`).
//! - `native-re/raw/chord_voice/rabin2-strings.txt` (`src/krisp/noise_canceller.cpp`,
//!   `src/krisp/voice_activity_detector.cpp`, `src/media/captured_audio_processor.cpp`):
//!   - Внутренние модели: `"small_NC_8k"`, `"small_NC"`, `"full_NC"`.
//!   - Состояния `NoiseCanceller`: `NotInitialized` -> `Initialize` -> `Setup` -> `Process`.
//!   - Защита от перегрузки CPU (`"Noise canceller CPU overuse"`), контроль `sampleRateMismatchPercent`,
//!     пропуск при mute (`skipNoiseCancellationIfMuted`) и телеметрия `NoiseCanceller::GetSessionStats`.

/// Минимальный порог чувствительности VAD (дБ полной шкалы).
pub const MIN_VAD_THRESHOLD_DB: i8 = -100;

/// Максимальный порог чувствительности VAD (дБ полной шкалы).
pub const MAX_VAD_THRESHOLD_DB: i8 = 0;

/// Порог чувствительности VAD по умолчанию (`vadThreshold` в `rabin2-strings.txt`).
pub const DEFAULT_VAD_THRESHOLD_DB: i8 = -40;

/// Порог уверенности Krisp VAD по умолчанию (`vadKrispActivationThreshold` = `0.5`).
pub const DEFAULT_KRISP_VAD_THRESHOLD: f32 = 0.5;

/// Порог уверенности Krisp VAD при включённом AGC (`0.8` в `decoded/3eefa7dae1e2c234_0`).
pub const AGC_KRISP_VAD_THRESHOLD: f32 = 0.8;

/// Число входных кадров речи до открытия VAD-гейта по умолчанию (`vadLeading` = 5 кадров = 100 мс).
pub const DEFAULT_VAD_LEADING_FRAMES: u16 = 5;

/// Число кадров удержания VAD-гейта после окончания речи (`vadTrailing` = 25 кадров = 500 мс).
pub const DEFAULT_VAD_TRAILING_FRAMES: u16 = 25;

/// Максимальное число кадров leading/trailing hangover.
pub const MAX_VAD_HANGOVER_FRAMES: u16 = 100;

/// Минимальный уровень подавления шума (`discord_krisp.node` `setSuppressionLevel`).
pub const MIN_SUPPRESSION_LEVEL: u8 = 0;

/// Максимальный уровень подавления шума (`discord_krisp.node` `setSuppressionLevel`).
pub const MAX_SUPPRESSION_LEVEL: u8 = 100;

/// Уровень подавления шума по умолчанию (`100%`).
pub const DEFAULT_SUPPRESSION_LEVEL: u8 = 100;

/// Минимальное ослабление фонового шума в дБ.
pub const MIN_NOISE_SUPPRESSION_DB: u8 = 0;

/// Максимальное ослабление фонового шума в дБ.
pub const MAX_NOISE_SUPPRESSION_DB: u8 = 60;

/// Ослабление фонового шума по умолчанию для RNNoise (30 дБ).
pub const DEFAULT_RNNOISE_SUPPRESSION_DB: u8 = 30;

/// Ослабление фонового шума по умолчанию для Krisp (45 дБ).
pub const DEFAULT_KRISP_SUPPRESSION_DB: u8 = 45;

/// Файл весов полнополосной модели Krisp NC (`voice-deep.md` §3).
pub const KRISP_MODEL_MED_V7: &str = "krisp-nc-o-med-v7.kef";

/// Файл весов облегчённой модели Krisp NC (`native-re.md` §4.1).
pub const KRISP_MODEL_LITE_V1: &str = "krisp-nc-o-lite-v1.kef";

/// Файл весов узкополосной (8 кГц) модели Krisp NC (`native-re.md` §4.1).
pub const KRISP_MODEL_NB_V2: &str = "krisp-nc-o-nb-v2.kef";

/// Файл весов BVC Pro модели Krisp (`native-re.md` §4.1).
pub const KRISP_MODEL_BVC_PRO_V3: &str = "krisp-bvc-o-pro-v3.kef";

/// Файл весов VAD-детектора Krisp (`native-re.md` §4.1).
pub const KRISP_MODEL_VAD_V2: &str = "krisp-vad-o-v2.kef";

/// Поддерживаемые частоты дискретизации аудиозахвата (Гц).
pub const SUPPORTED_SAMPLE_RATES_HZ: [u32; 4] = [8_000, 16_000, 32_000, 48_000];

/// Поддерживаемые длительности аудиокадра (мс).
pub const SUPPORTED_FRAME_DURATIONS_MS: [u16; 3] = [10, 20, 30];

/// Максимально допустимое время обработки одного 20-мс кадра (мкс) до фиксации перегрузки CPU.
pub const MAX_CPU_PROCESS_TIME_US_PER_20MS: u64 = 12_000;

/// Число подряд кадров с перегрузкой CPU до автоматического отключения Krisp (`MaybeToggleNoiseCanceller`).
pub const CPU_OVERUSE_CONSECUTIVE_FRAMES_LIMIT: u32 = 5;

/// Максимально допустимый процент рассинхрона частоты дискретизации (`sampleRateMismatchPercent`).
pub const MAX_SAMPLE_RATE_MISMATCH_PERCENT: u8 = 15;

/// Бэкенд шумоподавления голосового тракта.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuppressionBackend {
    Disabled,
    StandardWebRtc,
    RnNoise,
    Krisp,
}

impl SuppressionBackend {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::StandardWebRtc => "webrtc_ns",
            Self::RnNoise => "rnnoise",
            Self::Krisp => "krisp",
        }
    }
}

/// Уровень модели Krisp (`rabin2-strings.txt:641-643`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KrispModelTier {
    SmallNc8k,
    SmallNc,
    FullNc,
}

impl KrispModelTier {
    #[must_use]
    pub const fn engine_name(self) -> &'static str {
        match self {
            Self::SmallNc8k => "small_NC_8k",
            Self::SmallNc => "small_NC",
            Self::FullNc => "full_NC",
        }
    }

    #[must_use]
    pub const fn kef_file_name(self) -> &'static str {
        match self {
            Self::SmallNc8k => KRISP_MODEL_NB_V2,
            Self::SmallNc => KRISP_MODEL_LITE_V1,
            Self::FullNc => KRISP_MODEL_MED_V7,
        }
    }

    #[must_use]
    pub const fn select(sample_rate_hz: u32, fullband_performance: bool) -> Self {
        if sample_rate_hz == 8_000 {
            Self::SmallNc8k
        } else if fullband_performance {
            Self::FullNc
        } else {
            Self::SmallNc
        }
    }
}

/// Состояние жизненного цикла `discord::NoiseCanceller` (`src/krisp/noise_canceller.cpp`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseCancellerState {
    NotInitialized,
    Initialized,
    Configured,
    DisabledByCpuOveruse,
}

/// Ошибки инициализации, настройки и выполнения шумоподавления (`rabin2-strings.txt:648-663`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoiseSuppressionError {
    AlreadyInitialized,
    NotInitialized,
    NotConfigured,
    ModelLoadFailed,
    UnsupportedSampleRate {
        sample_rate_hz: u32,
    },
    UnsupportedChannels {
        channels: u8,
    },
    UnsupportedFrameDuration {
        duration_ms: u16,
    },
    FrameLengthMismatch {
        expected_samples: usize,
        actual_samples: usize,
    },
    InvalidThreshold,
}

impl NoiseSuppressionError {
    #[must_use]
    pub const fn engine_message(&self) -> &'static str {
        match self {
            Self::AlreadyInitialized => {
                "Attempting to initialize while not in NotInitialized state"
            }
            Self::NotInitialized => "Attempting to setup while not initialized",
            Self::NotConfigured => "Attempting to process while not initialized",
            Self::ModelLoadFailed => "Failed to load Krisp",
            Self::UnsupportedSampleRate { .. } => "Unable to configure Krisp: invalid sample rate",
            Self::UnsupportedChannels { .. } => {
                "Unable to configure Krisp: invalid number of channels"
            }
            Self::UnsupportedFrameDuration { .. } => {
                "Unable to configure Krisp: invalid frame duration"
            }
            Self::FrameLengthMismatch { .. } => "Invalid audio frame sample count",
            Self::InvalidThreshold => "Invalid VAD threshold configuration",
        }
    }
}

/// Конфигурация фильтров шумоподавления и VAD-гейта (`rabin2-strings.txt:19-39`).
#[derive(Debug, Clone, PartialEq)]
pub struct NoiseSuppressionConfig {
    pub backend: SuppressionBackend,
    pub echo_cancellation: bool,
    pub noise_suppression: bool,
    pub noise_cancellation: bool,
    pub noise_cancellation_during_processing: bool,
    pub skip_noise_cancellation_if_muted: bool,
    pub automatic_gain_control: bool,
    pub bypass_system_processing: bool,
    pub suppression_level: u8,
    pub max_attenuation_db: u8,
    pub vad_threshold_db: i8,
    pub vad_auto_threshold: bool,
    pub vad_use_krisp: bool,
    pub vad_krisp_activation_threshold: f32,
    pub vad_leading_frames: u16,
    pub vad_trailing_frames: u16,
    pub fullband_performance: bool,
}

impl NoiseSuppressionConfig {
    #[must_use]
    pub fn default_krisp() -> Self {
        Self {
            backend: SuppressionBackend::Krisp,
            echo_cancellation: true,
            noise_suppression: true,
            noise_cancellation: true,
            noise_cancellation_during_processing: false,
            skip_noise_cancellation_if_muted: true,
            automatic_gain_control: true,
            bypass_system_processing: false,
            suppression_level: DEFAULT_SUPPRESSION_LEVEL,
            max_attenuation_db: DEFAULT_KRISP_SUPPRESSION_DB,
            vad_threshold_db: DEFAULT_VAD_THRESHOLD_DB,
            vad_auto_threshold: true,
            vad_use_krisp: true,
            vad_krisp_activation_threshold: DEFAULT_KRISP_VAD_THRESHOLD,
            vad_leading_frames: DEFAULT_VAD_LEADING_FRAMES,
            vad_trailing_frames: DEFAULT_VAD_TRAILING_FRAMES,
            fullband_performance: true,
        }
    }

    #[must_use]
    pub fn default_rnnoise() -> Self {
        Self {
            backend: SuppressionBackend::RnNoise,
            noise_cancellation: false,
            vad_use_krisp: false,
            max_attenuation_db: DEFAULT_RNNOISE_SUPPRESSION_DB,
            ..Self::default_krisp()
        }
    }

    #[must_use]
    pub fn normalized(&self) -> Self {
        let mut out = self.clone();
        out.suppression_level = out
            .suppression_level
            .clamp(MIN_SUPPRESSION_LEVEL, MAX_SUPPRESSION_LEVEL);
        out.max_attenuation_db = out
            .max_attenuation_db
            .clamp(MIN_NOISE_SUPPRESSION_DB, MAX_NOISE_SUPPRESSION_DB);
        out.vad_threshold_db = out
            .vad_threshold_db
            .clamp(MIN_VAD_THRESHOLD_DB, MAX_VAD_THRESHOLD_DB);
        out.vad_krisp_activation_threshold = if out.vad_krisp_activation_threshold.is_finite() {
            out.vad_krisp_activation_threshold.clamp(0.0, 1.0)
        } else if out.automatic_gain_control {
            AGC_KRISP_VAD_THRESHOLD
        } else {
            DEFAULT_KRISP_VAD_THRESHOLD
        };
        out.vad_leading_frames = out.vad_leading_frames.min(MAX_VAD_HANGOVER_FRAMES);
        out.vad_trailing_frames = out.vad_trailing_frames.min(MAX_VAD_HANGOVER_FRAMES);
        out
    }

    #[must_use]
    pub fn effective_attenuation_db(&self) -> f32 {
        let norm = self.normalized();
        f32::from(norm.max_attenuation_db) * (f32::from(norm.suppression_level) / 100.0)
    }
}

/// Диапазон интенсивности фонового шума для гистограммы сессии.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseBand {
    NoNoise,
    LowNoise,
    MediumNoise,
    HighNoise,
}

/// Статистика сессии шумоподавления (`NoiseCanceller::GetSessionStats` / `getNoiseCancellationStats`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NoiseCancellationStats {
    pub talk_time_ms: u64,
    pub no_noise_ms: u64,
    pub low_noise_ms: u64,
    pub medium_noise_ms: u64,
    pub high_noise_ms: u64,
    pub noise_canceller_is_enabled: bool,
    pub noise_canceller_process_time_us: u64,
    pub noise_canceller_frames: u64,
    pub apm_process_time_ms: u64,
    pub apm_frame_count: u64,
    pub echo_delay_ms: u32,
    pub sample_rate_mismatch_percent: u8,
    pub current_sample_rate: u32,
    pub capture_processing_delay_ms: u32,
    pub krisp_nc_model: String,
}

impl NoiseCancellationStats {
    #[must_use]
    pub fn to_json(&self) -> String {
        format!(
            "{{\"talkTimeMs\":{},\"noNoiseMs\":{},\"lowNoiseMs\":{},\"mediumNoiseMs\":{},\"highNoiseMs\":{},\"noiseCancellerIsEnabled\":{},\"noiseCancellerProcessTime\":{},\"noiseCancellerFrames\":{},\"apmProcessTimeMs\":{},\"apmFrameCount\":{},\"echo_delay_ms\":{},\"sampleRateMismatchPercent\":{},\"currentSampleRate\":{},\"captureProcessingDelayMs\":{},\"krisp_nc_model\":\"{}\"}}",
            self.talk_time_ms,
            self.no_noise_ms,
            self.low_noise_ms,
            self.medium_noise_ms,
            self.high_noise_ms,
            self.noise_canceller_is_enabled,
            self.noise_canceller_process_time_us,
            self.noise_canceller_frames,
            self.apm_process_time_ms,
            self.apm_frame_count,
            self.echo_delay_ms,
            self.sample_rate_mismatch_percent,
            self.current_sample_rate,
            self.capture_processing_delay_ms,
            self.krisp_nc_model,
        )
    }
}

/// Результат обработки аудиокадра фильтром шумоподавления и VAD-гейтом.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VadGateResult {
    pub input_dbfs: f32,
    pub output_dbfs: f32,
    pub applied_suppression_db: f32,
    pub krisp_confidence: f32,
    pub raw_voice_detected: bool,
    pub gate_open: bool,
    pub noise_band: NoiseBand,
    pub active_backend: SuppressionBackend,
    pub skipped_because_muted: bool,
}

/// Вычисляет уровень сигнала PCM S16LE в дБ относительно полной шкалы (`[-100.0..=0.0]` dBFS).
#[must_use]
pub fn compute_frame_rms_dbfs(samples: &[i16]) -> f32 {
    if samples.is_empty() {
        return -100.0;
    }
    let mut sum_sq = 0.0_f64;
    for &sample in samples {
        let s = f64::from(sample);
        sum_sq += s * s;
    }
    let mean_sq = sum_sq / (samples.len() as f64);
    if mean_sq <= 0.0 {
        return -100.0;
    }
    let rms = mean_sq.sqrt() / f64::from(i16::MAX);
    if rms <= 1e-5 {
        return -100.0;
    }
    let db = 20.0 * rms.log10();
    (db as f32).clamp(-100.0, 0.0)
}

/// Ослабляет кадр PCM S16LE на заданное число децибел (`attenuation_db >= 0.0`).
pub fn attenuate_pcm_s16_in_place(samples: &mut [i16], attenuation_db: f32) {
    if !attenuation_db.is_finite() || attenuation_db <= 0.0 {
        return;
    }
    let gain = 10.0_f32.powf(-attenuation_db / 20.0);
    for sample in samples.iter_mut() {
        let scaled = (f32::from(*sample) * gain).round();
        *sample = scaled.clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16;
    }
}

/// Классифицирует уровень фонового шума по корзинам телеметрии `NoiseCanceller::GetSessionStats`.
#[must_use]
pub fn classify_noise_band(noise_dbfs: f32) -> NoiseBand {
    if noise_dbfs <= -65.0 {
        NoiseBand::NoNoise
    } else if noise_dbfs <= -45.0 {
        NoiseBand::LowNoise
    } else if noise_dbfs <= -25.0 {
        NoiseBand::MediumNoise
    } else {
        NoiseBand::HighNoise
    }
}

/// Конвейер шумоподавления (`NoiseCanceller` + `CapturedAudioProcessor` + `VoiceActivityDetector`).
#[derive(Debug, Clone)]
pub struct NoiseSuppressionPipeline {
    config: NoiseSuppressionConfig,
    state: NoiseCancellerState,
    active_backend: SuppressionBackend,
    selected_model: Option<KrispModelTier>,
    sample_rate_hz: u32,
    channels: u8,
    frame_duration_ms: u16,
    expected_samples: usize,
    leading_streak: u16,
    trailing_remaining: u16,
    gate_open: bool,
    cpu_overuse_streak: u32,
    stats_enabled: bool,
    stats: NoiseCancellationStats,
}

impl NoiseSuppressionPipeline {
    #[must_use]
    pub fn new(config: NoiseSuppressionConfig) -> Self {
        let normalized = config.normalized();
        let active_backend = normalized.backend;
        Self {
            config: normalized,
            state: NoiseCancellerState::NotInitialized,
            active_backend,
            selected_model: None,
            sample_rate_hz: 0,
            channels: 0,
            frame_duration_ms: 0,
            expected_samples: 0,
            leading_streak: 0,
            trailing_remaining: 0,
            gate_open: false,
            cpu_overuse_streak: 0,
            stats_enabled: true,
            stats: NoiseCancellationStats::default(),
        }
    }

    #[must_use]
    pub const fn state(&self) -> NoiseCancellerState {
        self.state
    }

    #[must_use]
    pub const fn config(&self) -> &NoiseSuppressionConfig {
        &self.config
    }

    pub fn set_config(&mut self, config: NoiseSuppressionConfig) {
        self.config = config.normalized();
        if self.state != NoiseCancellerState::DisabledByCpuOveruse {
            self.active_backend = self.config.backend;
        }
    }

    pub fn set_suppression_level(&mut self, level: u8) -> u8 {
        self.config.suppression_level = level.clamp(MIN_SUPPRESSION_LEVEL, MAX_SUPPRESSION_LEVEL);
        self.config.suppression_level
    }

    #[must_use]
    pub const fn suppression_level(&self) -> u8 {
        self.config.suppression_level
    }

    pub fn set_vad_threshold_db(&mut self, threshold_db: i8) -> i8 {
        self.config.vad_threshold_db =
            threshold_db.clamp(MIN_VAD_THRESHOLD_DB, MAX_VAD_THRESHOLD_DB);
        self.config.vad_threshold_db
    }

    #[must_use]
    pub const fn vad_threshold_db(&self) -> i8 {
        self.config.vad_threshold_db
    }

    #[must_use]
    pub const fn active_backend(&self) -> SuppressionBackend {
        self.active_backend
    }

    #[must_use]
    pub const fn selected_krisp_model(&self) -> Option<KrispModelTier> {
        self.selected_model
    }

    pub fn set_enable_stats(&mut self, enabled: bool) {
        self.stats_enabled = enabled;
    }

    #[must_use]
    pub const fn is_stats_enabled(&self) -> bool {
        self.stats_enabled
    }

    #[must_use]
    pub const fn stats(&self) -> &NoiseCancellationStats {
        &self.stats
    }

    pub fn reset_stats(&mut self) {
        let model = self.stats.krisp_nc_model.clone();
        let enabled = self.stats.noise_canceller_is_enabled;
        let sr = self.stats.current_sample_rate;
        self.stats = NoiseCancellationStats {
            noise_canceller_is_enabled: enabled,
            current_sample_rate: sr,
            krisp_nc_model: model,
            ..NoiseCancellationStats::default()
        };
    }

    pub fn initialize(&mut self, model_available: bool) -> Result<(), NoiseSuppressionError> {
        if self.state != NoiseCancellerState::NotInitialized {
            return Err(NoiseSuppressionError::AlreadyInitialized);
        }
        if self.config.backend == SuppressionBackend::Krisp && !model_available {
            self.active_backend = if self.config.noise_suppression {
                SuppressionBackend::RnNoise
            } else {
                SuppressionBackend::Disabled
            };
            self.stats.noise_canceller_is_enabled = false;
            return Err(NoiseSuppressionError::ModelLoadFailed);
        }
        self.state = NoiseCancellerState::Initialized;
        self.active_backend = self.config.backend;
        self.stats.noise_canceller_is_enabled = self.active_backend != SuppressionBackend::Disabled;
        Ok(())
    }

    pub fn setup(
        &mut self,
        sample_rate_hz: u32,
        channels: u8,
        frame_duration_ms: u16,
    ) -> Result<usize, NoiseSuppressionError> {
        if self.state == NoiseCancellerState::NotInitialized {
            return Err(NoiseSuppressionError::NotInitialized);
        }
        if !SUPPORTED_SAMPLE_RATES_HZ.contains(&sample_rate_hz) {
            return Err(NoiseSuppressionError::UnsupportedSampleRate { sample_rate_hz });
        }
        if channels == 0 || channels > 2 {
            return Err(NoiseSuppressionError::UnsupportedChannels { channels });
        }
        if !SUPPORTED_FRAME_DURATIONS_MS.contains(&frame_duration_ms) {
            return Err(NoiseSuppressionError::UnsupportedFrameDuration {
                duration_ms: frame_duration_ms,
            });
        }
        let samples_per_channel = (sample_rate_hz as usize) * (frame_duration_ms as usize) / 1_000;
        let expected_samples = samples_per_channel * (channels as usize);
        let tier = KrispModelTier::select(sample_rate_hz, self.config.fullband_performance);
        self.selected_model = Some(tier);
        self.sample_rate_hz = sample_rate_hz;
        self.channels = channels;
        self.frame_duration_ms = frame_duration_ms;
        self.expected_samples = expected_samples;
        self.state = NoiseCancellerState::Configured;
        self.stats.current_sample_rate = sample_rate_hz;
        self.stats.krisp_nc_model = tier.kef_file_name().to_owned();
        self.stats.noise_canceller_is_enabled = self.active_backend != SuppressionBackend::Disabled;
        Ok(expected_samples)
    }

    pub fn reset(&mut self) {
        self.leading_streak = 0;
        self.trailing_remaining = 0;
        self.gate_open = false;
        self.cpu_overuse_streak = 0;
    }

    pub fn report_sample_rate_mismatch(&mut self, mismatch_percent: u8) {
        self.stats.sample_rate_mismatch_percent = mismatch_percent;
        if mismatch_percent > MAX_SAMPLE_RATE_MISMATCH_PERCENT
            && self.active_backend == SuppressionBackend::Krisp
        {
            self.active_backend = if self.config.noise_suppression {
                SuppressionBackend::RnNoise
            } else {
                SuppressionBackend::Disabled
            };
            self.stats.noise_canceller_is_enabled = false;
        }
    }

    pub fn process_frame(
        &mut self,
        samples: &mut [i16],
        is_muted: bool,
        ambient_noise_dbfs: f32,
        krisp_confidence: f32,
        process_time_us: u64,
    ) -> Result<VadGateResult, NoiseSuppressionError> {
        match self.state {
            NoiseCancellerState::NotInitialized => {
                return Err(NoiseSuppressionError::NotInitialized);
            }
            NoiseCancellerState::Initialized => {
                return Err(NoiseSuppressionError::NotConfigured);
            }
            NoiseCancellerState::Configured | NoiseCancellerState::DisabledByCpuOveruse => {}
        }

        if samples.len() != self.expected_samples {
            return Err(NoiseSuppressionError::FrameLengthMismatch {
                expected_samples: self.expected_samples,
                actual_samples: samples.len(),
            });
        }

        let input_dbfs = compute_frame_rms_dbfs(samples);
        let noise_band = classify_noise_band(ambient_noise_dbfs);
        let clamped_confidence = if krisp_confidence.is_finite() {
            krisp_confidence.clamp(0.0, 1.0)
        } else {
            0.0
        };

        // Оптимизация `skipNoiseCancellationIfMuted` (`rabin2-strings.txt:30`).
        if is_muted && self.config.skip_noise_cancellation_if_muted {
            samples.fill(0);
            self.leading_streak = 0;
            self.trailing_remaining = 0;
            self.gate_open = false;
            return Ok(VadGateResult {
                input_dbfs,
                output_dbfs: -100.0,
                applied_suppression_db: self.config.effective_attenuation_db(),
                krisp_confidence: 0.0,
                raw_voice_detected: false,
                gate_open: false,
                noise_band,
                active_backend: self.active_backend,
                skipped_because_muted: true,
            });
        }

        // Контроль перегрузки CPU (`Noise canceller CPU overuse` в `captured_audio_processor.cpp`).
        if self.active_backend == SuppressionBackend::Krisp {
            if process_time_us > MAX_CPU_PROCESS_TIME_US_PER_20MS {
                self.cpu_overuse_streak = self.cpu_overuse_streak.saturating_add(1);
                if self.cpu_overuse_streak >= CPU_OVERUSE_CONSECUTIVE_FRAMES_LIMIT {
                    self.state = NoiseCancellerState::DisabledByCpuOveruse;
                    self.active_backend = if self.config.noise_suppression {
                        SuppressionBackend::RnNoise
                    } else {
                        SuppressionBackend::StandardWebRtc
                    };
                    self.stats.noise_canceller_is_enabled = false;
                }
            } else {
                self.cpu_overuse_streak = 0;
            }
        }

        // Детектирование речи по порогу dBFS + порогу уверенности модели Krisp VAD.
        let db_above_threshold = input_dbfs >= f32::from(self.config.vad_threshold_db);
        let krisp_above_threshold = !self.config.vad_use_krisp
            || self.active_backend != SuppressionBackend::Krisp
            || clamped_confidence >= self.config.vad_krisp_activation_threshold;
        let raw_voice_detected = db_above_threshold && krisp_above_threshold;

        if raw_voice_detected {
            self.leading_streak = self.leading_streak.saturating_add(1);
            if self.leading_streak >= self.config.vad_leading_frames.max(1) {
                self.gate_open = true;
                self.trailing_remaining = self.config.vad_trailing_frames;
            }
        } else {
            self.leading_streak = 0;
            if self.gate_open {
                if self.trailing_remaining > 0 {
                    self.trailing_remaining -= 1;
                } else {
                    self.gate_open = false;
                }
            }
        }

        let applied_suppression_db =
            if self.active_backend == SuppressionBackend::Disabled || self.gate_open {
                0.0
            } else {
                self.config.effective_attenuation_db()
            };

        if applied_suppression_db > 0.0 {
            attenuate_pcm_s16_in_place(samples, applied_suppression_db);
        }
        let output_dbfs = compute_frame_rms_dbfs(samples);

        if self.stats_enabled {
            let frame_ms = u64::from(self.frame_duration_ms);
            self.stats.noise_canceller_frames = self.stats.noise_canceller_frames.saturating_add(1);
            self.stats.noise_canceller_process_time_us = self
                .stats
                .noise_canceller_process_time_us
                .saturating_add(process_time_us);
            self.stats.apm_frame_count = self.stats.apm_frame_count.saturating_add(1);
            self.stats.apm_process_time_ms = self
                .stats
                .apm_process_time_ms
                .saturating_add(process_time_us / 1_000);
            if self.gate_open {
                self.stats.talk_time_ms = self.stats.talk_time_ms.saturating_add(frame_ms);
            }
            match noise_band {
                NoiseBand::NoNoise => {
                    self.stats.no_noise_ms = self.stats.no_noise_ms.saturating_add(frame_ms);
                }
                NoiseBand::LowNoise => {
                    self.stats.low_noise_ms = self.stats.low_noise_ms.saturating_add(frame_ms);
                }
                NoiseBand::MediumNoise => {
                    self.stats.medium_noise_ms =
                        self.stats.medium_noise_ms.saturating_add(frame_ms);
                }
                NoiseBand::HighNoise => {
                    self.stats.high_noise_ms = self.stats.high_noise_ms.saturating_add(frame_ms);
                }
            }
        }

        Ok(VadGateResult {
            input_dbfs,
            output_dbfs,
            applied_suppression_db,
            krisp_confidence: clamped_confidence,
            raw_voice_detected,
            gate_open: self.gate_open,
            noise_band,
            active_backend: self.active_backend,
            skipped_because_muted: false,
        })
    }
}
