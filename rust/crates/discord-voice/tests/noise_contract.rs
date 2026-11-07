//! Контрактные тесты модуля шумоподавления (`Krisp` / `RNNoise` / `StandardWebRtc`) и VAD-гейтинга.
//!
//! Оракулы RE (`/home/sanya/re-deep/`):
//! - `voice-deep.md` (§3, строка 110): `[VOICE_DISCONNECT] krisp_nc_model: krisp-nc-o-med-v7.kef`.
//! - `native-re.md` (§4.1 `discord_krisp.node`): модели `.kef` (`krisp-nc-o-med-v7.kef`,
//!   `krisp-nc-o-lite-v1.kef`, `krisp-nc-o-nb-v2.kef`, `krisp-bvc-o-pro-v3.kef`, `krisp-vad-o-v2.kef`),
//!   уровень подавления `setSuppressionLevel` (`0..=100`).
//! - `native-re/raw/discord_voice/rabin2-strings.txt` (`src/krisp/noise_canceller.cpp`,
//!   `src/krisp/voice_activity_detector.cpp`, `src/media/captured_audio_processor.cpp`):
//!   - Внутренние модели: `"small_NC_8k"`, `"small_NC"`, `"full_NC"`.
//!   - Состояния `NoiseCanceller`: `NotInitialized` -> `Initialize` -> `Setup` -> `Process`.
//!   - Ошибки: `"Attempting to initialize while not in NotInitialized state"`,
//!     `"Failed to load Krisp"`, `"Attempting to setup while not initialized"`,
//!     `"Unable to configure Krisp: invalid number of channels"`,
//!     `"Attempting to process while not initialized"`.
//!   - Защита от перегрузки CPU (`"Noise canceller CPU overuse"`) и рассинхрона частоты
//!     дискретизации (`sampleRateMismatchPercent`), пропуск при mute (`skipNoiseCancellationIfMuted`).
//!   - Метрики сессии (`NoiseCanceller::GetSessionStats` / `getNoiseCancellationStats`):
//!     `talkTimeMs`, `noNoiseMs`, `lowNoiseMs`, `mediumNoiseMs`, `highNoiseMs`,
//!     `noiseCancellerIsEnabled`, `noiseCancellerProcessTime`, `noiseCancellerFrames`,
//!     `apmProcessTimeMs`, `apmFrameCount`, `echo_delay_ms`, `sampleRateMismatchPercent`,
//!     `currentSampleRate`, `captureProcessingDelayMs`, `krisp_nc_model`.

use discord_voice::noise::{
    AGC_KRISP_VAD_THRESHOLD, CPU_OVERUSE_CONSECUTIVE_FRAMES_LIMIT, DEFAULT_KRISP_SUPPRESSION_DB,
    DEFAULT_KRISP_VAD_THRESHOLD, DEFAULT_RNNOISE_SUPPRESSION_DB, DEFAULT_SUPPRESSION_LEVEL,
    DEFAULT_VAD_LEADING_FRAMES, DEFAULT_VAD_THRESHOLD_DB, DEFAULT_VAD_TRAILING_FRAMES,
    KRISP_MODEL_BVC_PRO_V3, KRISP_MODEL_LITE_V1, KRISP_MODEL_MED_V7, KRISP_MODEL_NB_V2,
    KRISP_MODEL_VAD_V2, KrispModelTier, MAX_CPU_PROCESS_TIME_US_PER_20MS, MAX_NOISE_SUPPRESSION_DB,
    MAX_SAMPLE_RATE_MISMATCH_PERCENT, MAX_SUPPRESSION_LEVEL, MAX_VAD_HANGOVER_FRAMES,
    MAX_VAD_THRESHOLD_DB, MIN_NOISE_SUPPRESSION_DB, MIN_SUPPRESSION_LEVEL, MIN_VAD_THRESHOLD_DB,
    NoiseBand, NoiseCancellerState, NoiseSuppressionConfig, NoiseSuppressionError,
    NoiseSuppressionPipeline, SUPPORTED_FRAME_DURATIONS_MS, SUPPORTED_SAMPLE_RATES_HZ,
    SuppressionBackend, attenuate_pcm_s16_in_place, classify_noise_band, compute_frame_rms_dbfs,
};

#[test]
fn t_noise_01_constants_and_krisp_model_tier_oracle() {
    assert_eq!(MIN_VAD_THRESHOLD_DB, -100);
    assert_eq!(MAX_VAD_THRESHOLD_DB, 0);
    assert_eq!(DEFAULT_VAD_THRESHOLD_DB, -40);
    assert!((DEFAULT_KRISP_VAD_THRESHOLD - 0.5).abs() < f32::EPSILON);
    assert!((AGC_KRISP_VAD_THRESHOLD - 0.8).abs() < f32::EPSILON);
    assert_eq!(DEFAULT_VAD_LEADING_FRAMES, 5);
    assert_eq!(DEFAULT_VAD_TRAILING_FRAMES, 25);
    assert_eq!(MAX_VAD_HANGOVER_FRAMES, 100);
    assert_eq!(MIN_SUPPRESSION_LEVEL, 0);
    assert_eq!(MAX_SUPPRESSION_LEVEL, 100);
    assert_eq!(DEFAULT_SUPPRESSION_LEVEL, 100);
    assert_eq!(MIN_NOISE_SUPPRESSION_DB, 0);
    assert_eq!(MAX_NOISE_SUPPRESSION_DB, 60);
    assert_eq!(DEFAULT_RNNOISE_SUPPRESSION_DB, 30);
    assert_eq!(DEFAULT_KRISP_SUPPRESSION_DB, 45);

    // Имена моделей `.kef` из `voice-deep.md` и `native-re.md` §4.1
    assert_eq!(KRISP_MODEL_MED_V7, "krisp-nc-o-med-v7.kef");
    assert_eq!(KRISP_MODEL_LITE_V1, "krisp-nc-o-lite-v1.kef");
    assert_eq!(KRISP_MODEL_NB_V2, "krisp-nc-o-nb-v2.kef");
    assert_eq!(KRISP_MODEL_BVC_PRO_V3, "krisp-bvc-o-pro-v3.kef");
    assert_eq!(KRISP_MODEL_VAD_V2, "krisp-vad-o-v2.kef");

    assert_eq!(SUPPORTED_SAMPLE_RATES_HZ, [8_000, 16_000, 32_000, 48_000]);
    assert_eq!(SUPPORTED_FRAME_DURATIONS_MS, [10, 20, 30]);

    // Выбор модели по частоте дискретизации и флагу `hasFullbandPerformance_` (`rabin2-strings.txt:641-643`)
    assert_eq!(
        KrispModelTier::select(8_000, true),
        KrispModelTier::SmallNc8k
    );
    assert_eq!(KrispModelTier::SmallNc8k.engine_name(), "small_NC_8k");
    assert_eq!(KrispModelTier::SmallNc8k.kef_file_name(), KRISP_MODEL_NB_V2);

    assert_eq!(
        KrispModelTier::select(48_000, false),
        KrispModelTier::SmallNc
    );
    assert_eq!(KrispModelTier::SmallNc.engine_name(), "small_NC");
    assert_eq!(KrispModelTier::SmallNc.kef_file_name(), KRISP_MODEL_LITE_V1);

    assert_eq!(KrispModelTier::select(48_000, true), KrispModelTier::FullNc);
    assert_eq!(KrispModelTier::FullNc.engine_name(), "full_NC");
    assert_eq!(KrispModelTier::FullNc.kef_file_name(), KRISP_MODEL_MED_V7);
}

#[test]
fn t_noise_02_lifecycle_state_machine_and_error_oracles() {
    let mut pipeline = NoiseSuppressionPipeline::new(NoiseSuppressionConfig::default_krisp());
    assert_eq!(pipeline.state(), NoiseCancellerState::NotInitialized);

    // 1. Попытка вызвать `setup` или `process_frame` до `initialize` возвращает `NotInitialized`
    // со строками из `src/krisp/noise_canceller.cpp`.
    assert_eq!(
        pipeline.setup(48_000, 1, 20),
        Err(NoiseSuppressionError::NotInitialized)
    );
    assert_eq!(
        NoiseSuppressionError::NotInitialized.engine_message(),
        "Attempting to setup while not initialized"
    );

    let mut dummy_frame = vec![0i16; 960];
    assert_eq!(
        pipeline.process_frame(&mut dummy_frame, false, -50.0, 0.9, 500),
        Err(NoiseSuppressionError::NotInitialized)
    );

    // 2. Ошибка загрузки весов модели Krisp (`model_available = false`) -> `ModelLoadFailed`
    // и автоматический откат бэкенда на `RnNoise`.
    assert_eq!(
        pipeline.initialize(false),
        Err(NoiseSuppressionError::ModelLoadFailed)
    );
    assert_eq!(
        NoiseSuppressionError::ModelLoadFailed.engine_message(),
        "Failed to load Krisp"
    );
    assert_eq!(
        pipeline.active_backend(),
        SuppressionBackend::RnNoise,
        "при недоступности весов Krisp пайплайн обязан откатиться на RNNoise"
    );

    // 3. Успешная инициализация и запрет повторной инициализации (`AlreadyInitialized`).
    let mut pipeline = NoiseSuppressionPipeline::new(NoiseSuppressionConfig::default_krisp());
    assert_eq!(pipeline.initialize(true), Ok(()));
    assert_eq!(pipeline.state(), NoiseCancellerState::Initialized);
    assert_eq!(
        pipeline.initialize(true),
        Err(NoiseSuppressionError::AlreadyInitialized)
    );
    assert_eq!(
        NoiseSuppressionError::AlreadyInitialized.engine_message(),
        "Attempting to initialize while not in NotInitialized state"
    );

    // 4. Валидация параметров `setup(sample_rate_hz, channels, frame_duration_ms)`.
    assert_eq!(
        pipeline.setup(44_100, 1, 20),
        Err(NoiseSuppressionError::UnsupportedSampleRate {
            sample_rate_hz: 44_100
        })
    );
    assert_eq!(
        pipeline.setup(48_000, 0, 20),
        Err(NoiseSuppressionError::UnsupportedChannels { channels: 0 })
    );
    assert_eq!(
        pipeline.setup(48_000, 3, 20),
        Err(NoiseSuppressionError::UnsupportedChannels { channels: 3 })
    );
    assert_eq!(
        NoiseSuppressionError::UnsupportedChannels { channels: 0 }.engine_message(),
        "Unable to configure Krisp: invalid number of channels"
    );
    assert_eq!(
        pipeline.setup(48_000, 1, 15),
        Err(NoiseSuppressionError::UnsupportedFrameDuration { duration_ms: 15 })
    );

    // 48 кГц, 1 канал, 20 мс -> 960 сэмплов на кадр.
    let expected_samples = pipeline.setup(48_000, 1, 20).expect("валидный setup 48kHz");
    assert_eq!(expected_samples, 960);
    assert_eq!(pipeline.state(), NoiseCancellerState::Configured);
    assert_eq!(
        pipeline.selected_krisp_model(),
        Some(KrispModelTier::FullNc)
    );

    // Неверная длина кадра PCM отклоняется.
    let mut short_frame = vec![100i16; 480];
    assert_eq!(
        pipeline.process_frame(&mut short_frame, false, -50.0, 0.9, 500),
        Err(NoiseSuppressionError::FrameLengthMismatch {
            expected_samples: 960,
            actual_samples: 480,
        })
    );
}

#[test]
fn t_noise_03_pcm_attenuation_in_db_and_vad_hangover_gating() {
    // 1. Проверка расчёта уровня RMS dBFS и подавления шума в дБ.
    assert!((compute_frame_rms_dbfs(&[]) - (-100.0)).abs() < f32::EPSILON);
    assert!((compute_frame_rms_dbfs(&[0i16; 960]) - (-100.0)).abs() < f32::EPSILON);

    let mut full_scale = vec![10_000i16; 960];
    let before_db = compute_frame_rms_dbfs(&full_scale);
    // Ослабление на 20 дБ уменьшает амплитуду ровно в 10 раз (10_000 -> 1_000).
    attenuate_pcm_s16_in_place(&mut full_scale, 20.0);
    assert_eq!(full_scale[0], 1_000);
    let after_db = compute_frame_rms_dbfs(&full_scale);
    assert!(
        ((before_db - after_db) - 20.0).abs() < 0.25,
        "ослабление на 20 дБ должно снижать уровень сигнала на ~20 дБ: до={before_db}, после={after_db}"
    );

    // 2. Классификация полос фонового шума (`noNoiseMs`, `lowNoiseMs`, `mediumNoiseMs`, `highNoiseMs`).
    assert_eq!(classify_noise_band(-70.0), NoiseBand::NoNoise);
    assert_eq!(classify_noise_band(-50.0), NoiseBand::LowNoise);
    assert_eq!(classify_noise_band(-30.0), NoiseBand::MediumNoise);
    assert_eq!(classify_noise_band(-15.0), NoiseBand::HighNoise);

    // 3. Проверка VAD leading/trailing hangover гейтинга и подавления фонового шума в дБ.
    let mut cfg = NoiseSuppressionConfig::default_krisp();
    cfg.vad_leading_frames = 2;
    cfg.vad_trailing_frames = 3;
    cfg.vad_threshold_db = -40;
    cfg.max_attenuation_db = 40;
    cfg.suppression_level = 50; // эффективное подавление фонового шума = 20 дБ
    assert!((cfg.effective_attenuation_db() - 20.0).abs() < f32::EPSILON);

    let mut pipeline = NoiseSuppressionPipeline::new(cfg);
    pipeline.initialize(true).unwrap();
    pipeline.setup(48_000, 1, 20).unwrap();

    // Кадр 1 с речью (уровень ~ -10 dBFS, confidence 0.9 >= 0.5):
    // из-за `vad_leading_frames = 2` гейт ещё закрыт на 1-м кадре, но `raw_voice_detected = true`.
    let mut speech_frame = vec![10_000i16; 960];
    let r1 = pipeline
        .process_frame(&mut speech_frame, false, -50.0, 0.9, 800)
        .unwrap();
    assert!(r1.raw_voice_detected);
    assert!(
        !r1.gate_open,
        "на 1-м кадре при vad_leading_frames=2 гейт ещё закрыт"
    );

    // Кадр 2 с речью: достигнут `vad_leading_frames = 2` -> гейт открывается (`gate_open = true`)!
    let mut speech_frame_2 = vec![10_000i16; 960];
    let r2 = pipeline
        .process_frame(&mut speech_frame_2, false, -50.0, 0.9, 800)
        .unwrap();
    assert!(r2.raw_voice_detected);
    assert!(r2.gate_open, "на 2-м кадре речи гейт обязан открыться");
    assert_eq!(
        speech_frame_2[0], 10_000,
        "активная речь не глушится гейтом"
    );

    // Кадры 3, 4, 5 — фоновый шум (сэмплы 1_000 => ~ -30 dBFS, но `krisp_confidence = 0.1 < 0.5`):
    // `raw_voice_detected = false`, однако гейт удерживается открытым в течение `vad_trailing_frames = 3` кадров!
    for trailing_step in 1..=3 {
        let mut noise_frame = vec![1_000i16; 960];
        let res = pipeline
            .process_frame(&mut noise_frame, false, -30.0, 0.1, 800)
            .unwrap();
        assert!(!res.raw_voice_detected);
        assert!(
            res.gate_open,
            "внутри окна vad_trailing_frames (шаг {trailing_step}/3) гейт обязан удерживаться открытым"
        );
    }

    // Кадр 6 — хвост `vad_trailing_frames` истёк -> гейт закрывается (`gate_open = false`),
    // а фоновый шум ослабляется на `effective_attenuation_db = 20.0` дБ (10_000 -> 1_000)!
    let mut background_noise = vec![10_000i16; 960];
    let r6 = pipeline
        .process_frame(&mut background_noise, false, -20.0, 0.05, 800)
        .unwrap();
    assert!(!r6.raw_voice_detected);
    assert!(!r6.gate_open);
    assert!((r6.applied_suppression_db - 20.0).abs() < f32::EPSILON);
    assert_eq!(background_noise[0], 1_000);
}

#[test]
fn t_noise_04_skip_when_muted_cpu_overuse_fallback_and_session_stats_json() {
    let mut pipeline = NoiseSuppressionPipeline::new(NoiseSuppressionConfig::default_krisp());
    pipeline.initialize(true).unwrap();
    pipeline.setup(48_000, 1, 20).unwrap();

    // 1. При `is_muted == true` и `skip_noise_cancellation_if_muted == true`
    // инференс Krisp пропускается (`skipped_because_muted = true`), сэмплы зануляются,
    // а счётчик `noise_canceller_frames` не растёт.
    let mut muted_frame = vec![5_000i16; 960];
    let muted_res = pipeline
        .process_frame(&mut muted_frame, true, -40.0, 0.95, 100)
        .unwrap();
    assert!(muted_res.skipped_because_muted);
    assert!(!muted_res.gate_open);
    assert!(muted_frame.iter().all(|&s| s == 0));
    assert_eq!(pipeline.stats().noise_canceller_frames, 0);

    // 2. Защита от перегрузки CPU (`Noise canceller CPU overuse`):
    // после `CPU_OVERUSE_CONSECUTIVE_FRAMES_LIMIT` (5) кадров с `process_time_us > MAX_CPU_PROCESS_TIME_US_PER_20MS`
    // Krisp автоматически отключается (`DisabledByCpuOveruse`) и переключается на `RnNoise`.
    for _ in 0..CPU_OVERUSE_CONSECUTIVE_FRAMES_LIMIT {
        let mut frame = vec![4_000i16; 960];
        let _ = pipeline
            .process_frame(
                &mut frame,
                false,
                -60.0,
                0.8,
                MAX_CPU_PROCESS_TIME_US_PER_20MS + 1_000,
            )
            .unwrap();
    }
    assert_eq!(
        pipeline.state(),
        NoiseCancellerState::DisabledByCpuOveruse,
        "после серии кадров с превышением бюджета CPU Krisp обязан отключиться"
    );
    assert_eq!(pipeline.active_backend(), SuppressionBackend::RnNoise);
    assert!(!pipeline.stats().noise_canceller_is_enabled);

    // 3. Проверка телеметрии `NoiseCancellationStats::to_json()` (`getNoiseCancellationStats`).
    pipeline.report_sample_rate_mismatch(MAX_SAMPLE_RATE_MISMATCH_PERCENT + 5);
    let json = pipeline.stats().to_json();
    for required_key in [
        "\"talkTimeMs\":",
        "\"noNoiseMs\":",
        "\"lowNoiseMs\":",
        "\"mediumNoiseMs\":",
        "\"highNoiseMs\":",
        "\"noiseCancellerIsEnabled\":false",
        "\"noiseCancellerProcessTime\":",
        "\"noiseCancellerFrames\":5",
        "\"apmProcessTimeMs\":",
        "\"apmFrameCount\":",
        "\"echo_delay_ms\":",
        "\"sampleRateMismatchPercent\":20",
        "\"currentSampleRate\":48000",
        "\"captureProcessingDelayMs\":",
        "\"krisp_nc_model\":\"krisp-nc-o-med-v7.kef\"",
    ] {
        assert!(
            json.contains(required_key),
            "JSON статистики сессии обязан содержать {required_key}, получено: {json}"
        );
    }

    // Сброс статистики `reset_stats` обнуляет счётчики кадров и времени.
    pipeline.reset_stats();
    assert_eq!(pipeline.stats().noise_canceller_frames, 0);
    assert_eq!(pipeline.stats().talk_time_ms, 0);
}
