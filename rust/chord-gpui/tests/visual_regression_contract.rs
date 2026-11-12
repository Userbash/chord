#![allow(dead_code, unused_imports)]
#![recursion_limit = "512"]

#[path = "../src/audio/mod.rs"]
mod audio;
#[path = "../src/dispatch/mod.rs"]
mod dispatch;
#[path = "../src/media/mod.rs"]
mod media;
#[path = "../src/net/mod.rs"]
mod net;
#[path = "../src/state/mod.rs"]
mod state;
#[path = "../src/storage/mod.rs"]
mod storage;
#[path = "../src/theme/mod.rs"]
mod theme;
#[path = "../src/ui/mod.rs"]
mod ui;
#[path = "../src/video/mod.rs"]
mod video;
#[path = "../src/visual_regression/mod.rs"]
mod visual_regression;

use std::path::PathBuf;

use state::{AppState, SettingsTab};
use visual_regression::{
    DynamicMaskSet, FrozenClock, GoldenSnapshotId, GpuiFrameBuffer, LlvmPipeCaptureConfig,
    MaskCategory, MaskRegion, SnapshotAuditEntry, SsimComparator, SsimResult,
    SubpixelAntialiasingMode, VisualDiffReport, VisualRegressionError,
};

fn goldens_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/goldens")
}

#[test]
fn test_sub_904_1_llvmpipe_config_frozen_clock_and_disabled_subpixel_aa() {
    let cfg = LlvmPipeCaptureConfig::deterministic();
    assert!(cfg.software_gl, "LIBGL_ALWAYS_SOFTWARE должен быть включен");
    assert_eq!(cfg.wgpu_backend, "gl");
    assert_eq!(cfg.mesa_driver, "llvmpipe");
    assert_eq!(
        cfg.subpixel_aa,
        SubpixelAntialiasingMode::Disabled,
        "Субпиксельное сглаживание шрифтов обязано быть отключено в тестовом режиме"
    );

    let clock: &FrozenClock = &cfg.frozen_clock;
    assert_eq!(clock.iso_timestamp, "2026-11-11T12:00:00Z");
    assert_eq!(clock.display_hh_mm, "12:00");
    assert_eq!(clock.unix_epoch_secs, 1_794_398_400);

    let env_vars = cfg.env_pairs();
    assert!(
        env_vars
            .iter()
            .any(|(k, v)| *k == "LIBGL_ALWAYS_SOFTWARE" && v == "1")
    );
    assert!(
        env_vars
            .iter()
            .any(|(k, v)| *k == "MESA_LOADER_DRIVER_OVERRIDE" && v == "llvmpipe")
    );
    assert!(
        env_vars
            .iter()
            .any(|(k, v)| *k == "GPUI_SUBPIXEL_AA" && v == "0")
    );
    assert!(
        env_vars
            .iter()
            .any(|(k, v)| *k == "DISCORD_GPUI_FROZEN_CLOCK" && v == "2026-11-11T12:00:00Z")
    );
}

#[test]
fn test_sub_904_1_framebuffer_rgba_capture_determinism_and_theme_colors() {
    let state = AppState::new();
    let cfg = LlvmPipeCaptureConfig::deterministic();

    let fb_a = GpuiFrameBuffer::capture_snapshot(GoldenSnapshotId::MainChat1280x720, &state, &cfg)
        .expect("Снятие кадра 1280x720 из виртуального FrameBuffer");
    let fb_b = GpuiFrameBuffer::capture_snapshot(GoldenSnapshotId::MainChat1280x720, &state, &cfg)
        .expect("Повторное снятие кадра 1280x720");

    assert_eq!(fb_a.width, 1280);
    assert_eq!(fb_a.height, 720);
    assert_eq!(fb_a.pixels.len(), 1280 * 720 * 4);
    assert_eq!(
        fb_a.pixels, fb_b.pixels,
        "Два последовательных кадра при FrozenClock и отключенном AA должны быть побитово идентичны"
    );

    // Проверка ключевых зон Discord Dark Theme:
    // 1. ServerRail (x=10, y=360) -> #1e1f22 (30, 31, 34, 255)
    assert_eq!(fb_a.pixel_at(10, 360), [0x1e, 0x1f, 0x22, 0xff]);
    // 2. ChannelSidebar (x=100, y=360) -> #2b2d31 (43, 45, 49, 255)
    assert_eq!(fb_a.pixel_at(100, 360), [0x2b, 0x2d, 0x31, 0xff]);
    // 3. ChatView (x=600, y=360) -> #313338 (49, 51, 56, 255)
    assert_eq!(fb_a.pixel_at(600, 360), [0x31, 0x33, 0x38, 0xff]);
}

#[test]
fn test_sub_904_2_json_dynamic_masks_serialization_and_categories() {
    let masks = DynamicMaskSet::default_for_resolution(1280, 720);
    assert_eq!(masks.viewport_width, 1280);
    assert_eq!(masks.viewport_height, 720);
    assert!(masks.validate(1280, 720).is_ok());

    // Обязательные категории динамических зон: часы, FPS, аватары, пинг
    assert!(
        masks
            .regions
            .iter()
            .any(|r| r.category == MaskCategory::Clock)
    );
    assert!(
        masks
            .regions
            .iter()
            .any(|r| r.category == MaskCategory::FpsCounter)
    );
    assert!(
        masks
            .regions
            .iter()
            .any(|r| r.category == MaskCategory::Avatar)
    );
    assert!(
        masks
            .regions
            .iter()
            .any(|r| r.category == MaskCategory::PingIndicator)
    );

    let json = masks.to_json_pretty().expect("Сериализация масок в JSON");
    let parsed = DynamicMaskSet::from_json(&json).expect("Десериализация масок из JSON");
    assert_eq!(parsed, masks);
}

#[test]
fn test_sub_904_2_ssim_ignores_dynamic_masked_regions_without_false_positives() {
    let state = AppState::new();
    let cfg = LlvmPipeCaptureConfig::deterministic();
    let masks = DynamicMaskSet::default_for_resolution(1280, 720);

    let baseline =
        GpuiFrameBuffer::capture_snapshot(GoldenSnapshotId::MainChat1280x720, &state, &cfg)
            .expect("Baseline кадр");
    let mut candidate = baseline.clone();

    // Симулируем динамическое изменение часов, FPS, аватарок и пинга строго внутри масок
    for region in &masks.regions {
        candidate.fill_rect(
            region.x,
            region.y,
            region.width,
            region.height,
            [0xff, 0x00, 0xff, 0xff],
        );
    }

    let comparator = SsimComparator::new(SsimComparator::DEFAULT_THRESHOLD);
    let (res, _diff): (SsimResult, GpuiFrameBuffer) = comparator
        .compare(&baseline, &candidate, &masks)
        .expect("Расчёт SSIM с масками");

    assert!(
        res.passed,
        "Динамические изменения внутри JSON-масок не должны вызывать ложных срабатываний"
    );
    assert!(
        res.score > 0.999,
        "SSIM при шуме только в маскированных зонах должен быть > 0.999, получено {}",
        res.score
    );
    assert_eq!(res.unmasked_diff_pixels, 0);
    assert!(res.masked_pixels > 0);
}

#[test]
fn test_sub_904_2_ssim_detects_unmasked_visual_regression_and_marks_red_diff() {
    let state = AppState::new();
    let cfg = LlvmPipeCaptureConfig::deterministic();
    let masks = DynamicMaskSet::default_for_resolution(1280, 720);

    let baseline =
        GpuiFrameBuffer::capture_snapshot(GoldenSnapshotId::MainChat1280x720, &state, &cfg)
            .expect("Baseline кадр");
    let mut corrupted = baseline.clone();

    // Вносим визуальную регрессию в незамаскированную центральную зону чата (x=560..880, y=220..460)
    corrupted.fill_rect(560, 220, 320, 240, [0xf2, 0x3f, 0x43, 0xff]);

    let comparator = SsimComparator::new(SsimComparator::DEFAULT_THRESHOLD);
    let (res, diff_frame) = comparator
        .compare(&baseline, &corrupted, &masks)
        .expect("Расчёт SSIM при регрессии");

    assert!(
        !res.passed,
        "SSIM обязан упасть при повреждении незамаскированной области чата (score={})",
        res.score
    );
    assert!(res.score < 0.99);
    assert!(res.unmasked_diff_pixels >= 320 * 240);

    // В дифф-буфере расходящиеся пиксели подсвечены красным (R=255, G<=40, B<=70)
    let diff_px = diff_frame.pixel_at(640, 300);
    assert_eq!(diff_px[0], 0xff);
    assert!(diff_px[1] < 0x40);
}

#[test]
fn test_sub_904_3_golden_snapshots_for_8_settings_tabs_modals_and_chat_resolutions() {
    let state = AppState::new();
    let cfg = LlvmPipeCaptureConfig::deterministic();
    let comparator = SsimComparator::new(SsimComparator::DEFAULT_THRESHOLD);

    // 1. Все 8 вкладок SettingsDialog должны иметь уникальные эталоны и давать SSIM > 0.99
    let tabs = [
        SettingsTab::MyAccount,
        SettingsTab::Profile,
        SettingsTab::VoiceAndVideo,
        SettingsTab::Appearance,
        SettingsTab::Keybinds,
        SettingsTab::Notifications,
        SettingsTab::TextAndImages,
        SettingsTab::Language,
    ];
    let mut tab_frames = Vec::new();
    for tab in tabs {
        let id = GoldenSnapshotId::for_settings_tab(tab);
        let frame =
            GpuiFrameBuffer::capture_snapshot(id, &state, &cfg).expect("Снимок вкладки настроек");
        assert_eq!((frame.width, frame.height), (1280, 720));
        tab_frames.push((tab, frame));
    }

    // Каждая из 8 вкладок настроек визуально отличается от остальных
    for i in 0..tab_frames.len() {
        for j in (i + 1)..tab_frames.len() {
            assert_ne!(
                tab_frames[i].1.pixels, tab_frames[j].1.pixels,
                "Вкладки {:?} и {:?} должны иметь различный рендер",
                tab_frames[i].0, tab_frames[j].0
            );
        }
    }

    // 2. Главный чат в 1280x720 и 1920x1080
    let chat_720 =
        GpuiFrameBuffer::capture_snapshot(GoldenSnapshotId::MainChat1280x720, &state, &cfg)
            .unwrap();
    let chat_1080 =
        GpuiFrameBuffer::capture_snapshot(GoldenSnapshotId::MainChat1920x1080, &state, &cfg)
            .unwrap();
    assert_eq!((chat_720.width, chat_720.height), (1280, 720));
    assert_eq!((chat_1080.width, chat_1080.height), (1920, 1080));

    // 3. Модалки (CaptchaModal, CreateGuildModal, ScreenShareModal) и оверлеи (ContextMenu, ProfileTooltip)
    for id in GoldenSnapshotId::ALL {
        let frame = GpuiFrameBuffer::capture_snapshot(id, &state, &cfg)
            .unwrap_or_else(|e| panic!("Ошибка захвата {:?}: {}", id, e));
        let (w, h) = id.dimensions();
        assert_eq!((frame.width, frame.height), (w, h));
        let masks = DynamicMaskSet::default_for_resolution(w, h);
        let (res, _) = comparator.compare(&frame, &frame, &masks).unwrap();
        assert!(res.passed && res.score > 0.999);
    }
}

#[test]
fn test_sub_904_3_materialize_golden_suite_and_interactive_html_diff_report() {
    let dir = goldens_dir();
    let entries: Vec<SnapshotAuditEntry> = VisualDiffReport::materialize_golden_suite(&dir)
        .expect("Генерация и проверка полного набора из 15 Golden Snapshots");

    assert_eq!(entries.len(), 15, "Должно быть ровно 15 золотых эталонов");
    for entry in &entries {
        assert!(
            entry.passed && entry.ssim_score > 0.99,
            "Эталон {} не прошёл проверку SSIM: {}",
            entry.slug,
            entry.ssim_score
        );
        let png_path = dir.join(format!("{}.png", entry.slug));
        assert!(
            png_path.exists(),
            "PNG файл эталона должен существовать: {:?}",
            png_path
        );
    }

    // Проверка интерактивного HTML отчёта с режимами Slider / Onion Skin / Red Diff Overlay
    let html = VisualDiffReport::generate_html(&entries);
    assert!(html.contains("Slider"));
    assert!(html.contains("Onion Skin"));
    assert!(html.contains("Red Diff Overlay"));
    assert!(html.contains("SSIM"));
    assert!(dir.join("visual_diff_report.html").exists());
    assert!(dir.join("dynamic_masks_1280x720.json").exists());
    assert!(dir.join("dynamic_masks_1920x1080.json").exists());
    assert!(dir.join("golden_manifest.json").exists());
}

#[test]
fn test_sub_904_negative_and_boundary_dimensions() {
    // 1. Нулевые и сверхбольшие размеры отклоняются
    assert!(matches!(
        GpuiFrameBuffer::new(0, 720),
        Err(VisualRegressionError::InvalidDimensions { .. })
    ));
    assert!(matches!(
        GpuiFrameBuffer::new(8000, 5000),
        Err(VisualRegressionError::InvalidDimensions { .. })
    ));

    // 2. Граничное минимальное разрешение окна GPUI (900x600) поддерживается
    let min_fb = GpuiFrameBuffer::new(900, 600).expect("900x600 допустимо");
    assert_eq!((min_fb.width, min_fb.height), (900, 600));

    // 3. Несовпадение длины RGBA буфера отклоняется
    assert!(matches!(
        GpuiFrameBuffer::from_rgba(1280, 720, vec![0u8; 100]),
        Err(VisualRegressionError::BufferLengthMismatch { .. })
    ));

    // 4. Несовпадение размеров кадров при сравнении SSIM отклоняется
    let fb_720 = GpuiFrameBuffer::new(1280, 720).unwrap();
    let fb_1080 = GpuiFrameBuffer::new(1920, 1080).unwrap();
    let masks = DynamicMaskSet::default_for_resolution(1280, 720);
    let comp = SsimComparator::new(0.995);
    assert!(matches!(
        comp.compare(&fb_720, &fb_1080, &masks),
        Err(VisualRegressionError::DimensionMismatch { .. })
    ));

    // 5. Выход маски за пределы экрана отклоняется валидатором
    let mut bad_masks = DynamicMaskSet::default_for_resolution(1280, 720);
    bad_masks.regions.push(MaskRegion {
        id: "overflow-mask".into(),
        category: MaskCategory::Clock,
        x: 1200,
        y: 700,
        width: 100,
        height: 50,
        description: "Выходит за границу 1280x720".into(),
    });
    assert!(matches!(
        bad_masks.validate(1280, 720),
        Err(VisualRegressionError::MaskOutOfBounds { .. })
    ));

    // 6. Битый JSON масок возвращает JsonError
    assert!(matches!(
        DynamicMaskSet::from_json("{ invalid_json "),
        Err(VisualRegressionError::JsonError(_))
    ));
}

#[test]
fn test_sub_904_empty_masks_edge_boundaries_and_recovery() {
    // Пустой список масок на граничном разрешении 900x600
    let empty_masks = DynamicMaskSet {
        viewport_width: 900,
        viewport_height: 600,
        regions: Vec::new(),
    };
    assert!(empty_masks.validate(900, 600).is_ok());
    assert!(!empty_masks.is_masked(450, 300));

    let fb_1 = GpuiFrameBuffer::new(900, 600).unwrap();
    let fb_2 = fb_1.clone();
    let comp = SsimComparator::new(0.995);
    let (res, _) = comp.compare(&fb_1, &fb_2, &empty_masks).unwrap();
    assert!(res.passed);
    assert_eq!(res.masked_pixels, 0);
    assert_eq!(res.unmasked_diff_pixels, 0);
    assert!((res.score - 1.0).abs() < 1e-9);
}

#[test]
fn test_sub_904_mutation_gate_5_of_5_killed() {
    let state = AppState::new();
    let cfg = LlvmPipeCaptureConfig::deterministic();
    let comp = SsimComparator::new(SsimComparator::DEFAULT_THRESHOLD);
    let masks_720 = DynamicMaskSet::default_for_resolution(1280, 720);
    let base = GpuiFrameBuffer::capture_snapshot(GoldenSnapshotId::MainChat1280x720, &state, &cfg)
        .unwrap();

    // Мутация 1: Отключение масок при динамическом изменении часов/FPS/аватарок обязано ронять SSIM
    let mut dynamic_noisy = base.clone();
    for r in &masks_720.regions {
        dynamic_noisy.fill_rect(r.x, r.y, r.width, r.height, [0xff, 0xff, 0x00, 0xff]);
    }
    let no_masks = DynamicMaskSet {
        viewport_width: 1280,
        viewport_height: 720,
        regions: Vec::new(),
    };
    let (res_mut1, _) = comp.compare(&base, &dynamic_noisy, &no_masks).unwrap();
    assert!(!res_mut1.passed, "Мутация 1 (отказ масок) убита");

    // Мутация 2: Подмена цвета ServerRail (#1e1f22 -> #ff0000) обязана ронять SSIM
    let mut rail_mutated = base.clone();
    rail_mutated.fill_rect(0, 0, 72, 720, [0xff, 0x00, 0x00, 0xff]);
    let (res_mut2, _) = comp.compare(&base, &rail_mutated, &masks_720).unwrap();
    assert!(!res_mut2.passed, "Мутация 2 (порча темы ServerRail) убита");

    // Мутация 3: Подмена вкладки настроек (MyAccount vs Language) обязана ронять SSIM против эталона
    let acc_fb =
        GpuiFrameBuffer::capture_snapshot(GoldenSnapshotId::SettingsMyAccount, &state, &cfg)
            .unwrap();
    let lang_fb =
        GpuiFrameBuffer::capture_snapshot(GoldenSnapshotId::SettingsLanguage, &state, &cfg)
            .unwrap();
    let (res_mut3, _) = comp.compare(&acc_fb, &lang_fb, &masks_720).unwrap();
    assert!(
        !res_mut3.passed,
        "Мутация 3 (коллизия вкладок настроек) убита"
    );

    // Мутация 4: Пропуск рендера модалки CaptchaModal против базового чата обязан ронять SSIM
    let captcha_fb =
        GpuiFrameBuffer::capture_snapshot(GoldenSnapshotId::ModalCaptcha, &state, &cfg).unwrap();
    let (res_mut4, _) = comp.compare(&captcha_fb, &base, &masks_720).unwrap();
    assert!(
        !res_mut4.passed,
        "Мутация 4 (исчезновение CaptchaModal) убита"
    );

    // Мутация 5: Нарушение FrozenClock (сдвиг времени в незамаскированной зоне) обязано обнаруживаться
    let mut clock_drift = base.clone();
    clock_drift.draw_text_deterministic(
        600,
        500,
        "DRIFT 23:59",
        [0xff, 0xff, 0xff, 0xff],
        SubpixelAntialiasingMode::Disabled,
    );
    let (res_mut5, _) = comp.compare(&base, &clock_drift, &masks_720).unwrap();
    assert!(
        res_mut5.unmasked_diff_pixels > 0 && res_mut5.min_block_score < 0.95,
        "Мутация 5 (рассинхрон часов вне маски) убита"
    );
}
