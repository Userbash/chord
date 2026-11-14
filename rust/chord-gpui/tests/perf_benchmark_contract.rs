//! Контрактные и интеграционные тесты `TASK-906`:
//! Бенчмаркинг 120 FPS (< 8.33 мс, p50/p95/p99) при скролле 100 000 сообщений,
//! профилирование аллокаций кучи (`jemalloc` / `heaptrack`), контроль `VmRSS < 60 МБ`,
//! выгрузка неиспользуемых текстур GPU VRAM и замер холодного старта (`< 150 мс` / `< 300 мс`).

#![recursion_limit = "512"]
#![allow(dead_code)]

#[path = "../src/audio/mod.rs"]
mod audio;
#[path = "../src/dispatch/mod.rs"]
mod dispatch;
#[path = "../src/media/mod.rs"]
mod media;
#[path = "../src/net/mod.rs"]
mod net;
#[path = "../src/platform/mod.rs"]
mod platform;
#[path = "../src/perf_benchmark/mod.rs"]
mod perf_benchmark;
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

use std::path::PathBuf;
use std::process::Command;

use media::MediaCache;
use perf_benchmark::{
    CLEAN_COLD_START_BUDGET_MS, FRAME_BUDGET_120FPS_NS, FRAME_BUDGET_120FPS_US,
    FRAME_P95_BUDGET_NS, MAX_FRAME_OVERSHOOT_RATIO, MAX_VMRSS_KB, MAX_VMRSS_MB, PerfBenchmarkError,
    SCROLL_WORKLOAD_MESSAGES, TARGET_FPS, VIRTUAL_VIEWPORT_ROWS, WARM_COLD_START_BUDGET_MS,
    ci_artifacts::{CiPerfArtifactPublisher, CiPerfReport},
    cold_start::{ColdStartBenchmark, ColdStartMode},
    frame_budget::{FramePercentileStats, FramePhaseSample, VirtualChatScrollBench},
    memory_profiler::{
        AllocationTag, HeapAllocationProfiler, ProcMemorySnapshot, StressSessionSimulator,
        TextureKind, VramTexturePool,
    },
};

fn perf_artifacts_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/perf-artifacts")
}

/// ═══════════════════════════════════════════════════════════════════════════
/// SUB-906-1: Непрерывный замер бюджета кадра 120 FPS (< 8.33 мс) на 100k сообщений
/// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_sub_906_1_high_res_frame_timers_and_100k_virtual_scroll_percentiles() {
    assert_eq!(TARGET_FPS, 120);
    assert_eq!(FRAME_BUDGET_120FPS_US, 8_333);
    assert_eq!(FRAME_BUDGET_120FPS_NS, 8_333_333);

    let bench = VirtualChatScrollBench::new(SCROLL_WORKLOAD_MESSAGES, VIRTUAL_VIEWPORT_ROWS)
        .expect("Инициализация виртуального скролла на 100 000 сообщений");
    assert_eq!(bench.total_messages, 100_000);
    assert_eq!(bench.viewport_rows, 30);

    // Прогоняем 1000 кадров непрерывного скролла по ленте 100 000 сообщений
    let (samples, stats) = bench
        .run_scroll_benchmark(1_000)
        .expect("Замер 1000 кадров скролла по 100 000 сообщениям");

    assert_eq!(samples.len(), 1_000);
    assert_eq!(stats.total_frames, 1_000);

    // Шаг 1.1: Таймеры высокой точности на каждом шаге layout & render
    for sample in &samples {
        let sum_phases = sample.state_sync_ns
            + sample.virtual_window_slice_ns
            + sample.layout_compute_ns
            + sample.gpu_scene_submit_ns;
        assert!(
            sample.total_frame_ns >= sum_phases,
            "Полное время кадра ({}) должно быть >= суммы фаз ({})",
            sample.total_frame_ns,
            sum_phases
        );
        assert!(
            sample.rendered_rows > 0 && sample.rendered_rows <= VIRTUAL_VIEWPORT_ROWS,
            "Виртуализация обязана рендерить только видимое окно (<= 30 строк), получено {}",
            sample.rendered_rows
        );
    }

    // Шаг 1.2: Перцентили p50, p95, p99 при скроллинге чата на 100 000 сообщений
    assert!(stats.min_ns <= stats.p50_ns);
    assert!(stats.p50_ns <= stats.p95_ns);
    assert!(stats.p95_ns <= stats.p99_ns);
    assert!(stats.p99_ns <= stats.p99_9_ns);
    assert!(stats.p99_9_ns <= stats.max_ns);
    assert!(
        stats.p95_ns < FRAME_P95_BUDGET_NS,
        "p95 ({:.3} мс) должен укладываться в целевой бюджет < 2.0 мс",
        stats.p95_ms()
    );
    assert!(
        stats.p99_ns <= FRAME_BUDGET_120FPS_NS,
        "p99 ({:.3} мс) обязан быть <= 8.33 мс (120 FPS)",
        stats.p99_ms()
    );
    assert!(stats.scroll_anchor_preserved);
    assert_eq!(stats.max_rendered_rows_per_frame, VIRTUAL_VIEWPORT_ROWS);

    // Шаг 1.3: Проверка 99.9% кадров в бюджете 8.33 мс (<= 0.1% превышений)
    assert!(
        stats.overshoot_ratio <= MAX_FRAME_OVERSHOOT_RATIO,
        "Доля кадров > 8.33 мс ({:.4}) превысила порог 0.1%",
        stats.overshoot_ratio
    );
    assert!(stats.within_budget_ratio >= 0.999);
    stats
        .validate_120fps_sla()
        .expect("Гейт 120 FPS должен успешно пройти");
}

#[test]
fn test_sub_906_1_frame_budget_boundary_and_negative_overshoot_failure() {
    // 1. Пустая выборка кадров -> EmptyFrameSamples
    assert_eq!(
        FramePercentileStats::from_samples(&[], true),
        Err(PerfBenchmarkError::EmptyFrameSamples)
    );

    // 2. Некорректная конфигурация вьюпорта (0 сообщений или окно > 36)
    assert!(VirtualChatScrollBench::new(0, 30).is_err());
    assert!(VirtualChatScrollBench::new(100_000, 0).is_err());
    assert!(VirtualChatScrollBench::new(100_000, 500).is_err());

    // 3. Граничный случай: ровно 1 кадр из 1000 превысил 8.33 мс (0.1% == MAX_FRAME_OVERSHOOT_RATIO) -> PASS
    let mut boundary_samples: Vec<FramePhaseSample> = (0..1_000)
        .map(|idx| FramePhaseSample {
            frame_index: idx,
            scroll_offset_row: idx * 30,
            rendered_rows: 30,
            state_sync_ns: 150_000,
            virtual_window_slice_ns: 100_000,
            layout_compute_ns: 450_000,
            gpu_scene_submit_ns: 300_000,
            total_frame_ns: 1_000_000, // 1.0 мс
        })
        .collect();

    // Проверка точной границы 8_333_333 нс (в бюджете) vs 8_333_334 нс (превышение)
    boundary_samples[998].total_frame_ns = FRAME_BUDGET_120FPS_NS;
    assert!(boundary_samples[998].is_within_120fps_budget());
    boundary_samples[999].total_frame_ns = FRAME_BUDGET_120FPS_NS + 1;
    assert!(!boundary_samples[999].is_within_120fps_budget());

    let stats_pass = FramePercentileStats::from_samples(&boundary_samples, true)
        .expect("Расчёт перцентилей на граничной выборке 1/1000");
    assert_eq!(stats_pass.overshoot_frames, 1);
    assert!((stats_pass.overshoot_ratio - 0.001).abs() < 1e-9);
    assert!(
        stats_pass.validate_120fps_sla().is_ok(),
        "Ровно 0.1% (1 из 1000) кадров > 8.33 мс находится на допустимой границе"
    );

    // 4. Негативный случай: 2 кадра из 1000 превысили 8.33 мс (0.2% > 0.1%) -> автоматический FAIL
    boundary_samples[998].total_frame_ns = FRAME_BUDGET_120FPS_NS + 500_000;
    let stats_fail = FramePercentileStats::from_samples(&boundary_samples, true)
        .expect("Расчёт перцентилей при 2/1000 дропах");
    assert_eq!(stats_fail.overshoot_frames, 2);
    assert!(
        matches!(
            stats_fail.validate_120fps_sla(),
            Err(PerfBenchmarkError::FrameBudgetExceeded { .. })
        ),
        "Превышение > 8.33 мс более чем в 0.1% кадров обязано ронять тест"
    );

    // 5. Проверка якорения позиции скролла при подгрузке старых сообщений вверх (prepend)
    let bench = VirtualChatScrollBench::new(100_000, 30).unwrap();
    let anchored_row = bench
        .verify_scroll_anchor_on_prepend(420, 50)
        .expect("Проверка якорения скролла при пагинации 50 сообщений вверх");
    assert_eq!(anchored_row, 470);
}

/// ═══════════════════════════════════════════════════════════════════════════
/// SUB-906-2: Профилирование кучи (jemalloc/heaptrack), VmRSS < 60 MB и GC VRAM
/// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_sub_906_2_jemalloc_and_heaptrack_profiling_and_24h_leak_free_stress() {
    let mut profiler = HeapAllocationProfiler::new();
    let id1 = profiler.record_alloc(AllocationTag::ChatMessages, 4096);
    let id2 = profiler.record_alloc(AllocationTag::ModalDialog, 2048);
    let id3 = profiler.record_alloc(AllocationTag::VramTexture, 8192);

    assert_eq!(profiler.active_allocations, 3);
    assert_eq!(profiler.live_bytes, 4096 + 2048 + 8192);
    assert_eq!(
        profiler.live_bytes_for_tag(AllocationTag::ModalDialog),
        2048
    );

    // Освобождаем буфер модалки
    let freed = profiler
        .record_free(id2)
        .expect("Освобождение аллокации модалки");
    assert_eq!(freed, 2048);
    assert_eq!(profiler.active_allocations, 2);
    assert_eq!(profiler.live_bytes, 4096 + 8192);
    assert_eq!(profiler.live_bytes_for_tag(AllocationTag::ModalDialog), 0);
    assert_eq!(profiler.peak_live_bytes, 4096 + 2048 + 8192);

    // Повторное освобождение того же ID -> ошибка double-free
    assert!(profiler.record_free(id2).is_err());

    // Экспорт и парсинг профиля jemalloc (heap_v2/1)
    let jemalloc_text = profiler.export_jemalloc_profile();
    assert!(jemalloc_text.starts_with("heap_v2/1\n"));
    let parsed_je = HeapAllocationProfiler::parse_jemalloc_profile(&jemalloc_text)
        .expect("Парсинг профиля jemalloc heap_v2/1");
    assert_eq!(parsed_je.header_version, "heap_v2/1");
    assert_eq!(parsed_je.total_inuse_objects, 2);
    assert_eq!(parsed_je.total_inuse_bytes, 4096 + 8192);
    assert_eq!(parsed_je.total_alloc_objects, 3);
    assert_eq!(parsed_je.total_alloc_bytes, 4096 + 2048 + 8192);

    // Экспорт и парсинг лога heaptrack
    let heaptrack_text = profiler.export_heaptrack_log();
    let parsed_ht = HeapAllocationProfiler::parse_heaptrack_log(&heaptrack_text)
        .expect("Парсинг лога heaptrack");
    assert_eq!(parsed_ht.version, 3);
    assert_eq!(parsed_ht.total_allocations, 3);
    assert_eq!(parsed_ht.total_frees, 1);
    assert_eq!(parsed_ht.peak_heap_bytes, 4096 + 2048 + 8192);
    assert_eq!(parsed_ht.leaked_bytes, 4096 + 8192);

    // Доосвобождаем оставшиеся блоки -> leaked_bytes == 0
    profiler.record_free(id1).unwrap();
    profiler.record_free(id3).unwrap();
    let clean_ht =
        HeapAllocationProfiler::parse_heaptrack_log(&profiler.export_heaptrack_log()).unwrap();
    assert_eq!(clean_ht.leaked_bytes, 0);

    // Симуляция 24-часового стресс-теста (240 циклов открытия/закрытия модалок и смены каналов)
    let (stress_report, _stress_prof) = StressSessionSimulator::run_24h_cycle_simulation(240, 0)
        .expect("24-часовой стресс-тест без утечек памяти");
    assert_eq!(stress_report.simulated_hours, 24);
    assert_eq!(stress_report.total_cycles, 240);
    assert!(stress_report.modals_toggled >= 240 * 4);
    assert!(stress_report.channels_switched >= 240);
    assert_eq!(stress_report.leaked_bytes, 0);
    assert!(
        stress_report.slope_bytes_per_cycle.abs() < 1e-6,
        "Наклон тренда памяти должен быть 0.0 Б/цикл, получено {}",
        stress_report.slope_bytes_per_cycle
    );
    assert!(stress_report.vmrss_kb < MAX_VMRSS_KB);
    stress_report
        .validate_zero_leaks()
        .expect("Валидация отсутствия утечек должна пройти");

    // Негативный тест: инжекция утечки 512 байт на цикл открытия модалки -> детектор обязан упасть
    let leak_res = StressSessionSimulator::run_24h_cycle_simulation(240, 512);
    assert!(
        matches!(leak_res, Err(PerfBenchmarkError::MemoryLeakDetected { .. })),
        "При растущей утечке памяти в цикле модалок/каналов тест обязан падать"
    );
}

#[test]
fn test_sub_906_2_vmrss_proc_status_oracle_and_60mb_hard_limit() {
    // 1. Чтение реального /proc/self/status и /proc/self/statm текущего процесса
    let live_snap = ProcMemorySnapshot::read_current_process()
        .expect("Чтение /proc/self/status и /proc/self/statm");
    assert!(live_snap.vmrss_kb > 0);
    assert!(live_snap.vmsize_kb >= live_snap.vmrss_kb);
    assert!(live_snap.statm_resident_kb > 0);

    // 2. Граничные тесты лимита VmRSS < 60 МБ (61_440 КБ)
    let sample_ok = ProcMemorySnapshot::from_proc_texts(
        "Name:\tdiscord-gpui\nVmPeak:\t120000 kB\nVmSize:\t98000 kB\nVmHWM:\t22000 kB\nVmRSS:\t61439 kB\nVmData:\t14000 kB\n",
        "24500 15359 4000 1000 0 3500 0\n",
    )
    .expect("Парсинг валидного /proc/self/status на границе 61439 КБ");
    assert_eq!(sample_ok.vmrss_kb, 61_439);
    assert!(sample_ok.vmrss_mb() < MAX_VMRSS_MB);
    assert!(
        sample_ok.validate_rss_budget().is_ok(),
        "61 439 КБ (59.999 МБ) строго меньше 60 МБ и должно проходить"
    );

    // Ровно 60 МБ (61_440 КБ) нарушает строгий бюджет `< 60 МБ`
    let sample_at_limit = ProcMemorySnapshot::from_proc_texts(
        "Name:\tdiscord-gpui\nVmSize:\t98000 kB\nVmHWM:\t61440 kB\nVmRSS:\t61440 kB\nVmData:\t14000 kB\n",
        "24500 15360 4000 1000 0 3500 0\n",
    )
    .unwrap();
    assert_eq!(
        sample_at_limit.validate_rss_budget(),
        Err(PerfBenchmarkError::MemoryBudgetExceeded {
            vmrss_kb: 61_440,
            limit_kb: 61_440,
        })
    );

    // Битый текст /proc/self/status без поля VmRSS -> ProcParseError
    assert!(matches!(
        ProcMemorySnapshot::from_proc_texts("Name:\tbroken\n", "0 0 0 0 0 0 0"),
        Err(PerfBenchmarkError::ProcParseError(_))
    ));
}

#[test]
fn test_sub_906_2_gpu_vram_texture_gc_and_media_cache_eviction() {
    let mut media_cache = MediaCache::new();
    // Бюджет пула текстур VRAM: 4 МБ (4 * 1024 * 1024 = 4_194_304 байт)
    let mut vram_pool = VramTexturePool::new(4 * 1024 * 1024);

    // Загружаем аватарки и вложения для канала #general (frame = 10)
    let avatar_bytes = vram_pool
        .upload_rgba_texture(
            "avatar-alice",
            TextureKind::Avatar,
            "c-general",
            128,
            128,
            10,
            Some(&media_cache),
        )
        .expect("Загрузка текстуры аватарки 128x128");
    assert_eq!(avatar_bytes, 128 * 128 * 4);

    let attach_bytes = vram_pool
        .upload_rgba_texture(
            "attach-banner",
            TextureKind::Attachment,
            "c-general",
            512,
            512,
            10,
            Some(&media_cache),
        )
        .expect("Загрузка текстуры вложения 512x512");
    assert_eq!(attach_bytes, 512 * 512 * 4);

    // Загружаем текстуры для другого канала #memes (frame = 50)
    vram_pool
        .upload_rgba_texture(
            "attach-meme-1",
            TextureKind::Attachment,
            "c-memes",
            512,
            512,
            50,
            Some(&media_cache),
        )
        .expect("Загрузка текстуры для #memes");

    assert_eq!(vram_pool.texture_count(), 3);
    assert_eq!(media_cache.len(), 3);
    assert_eq!(
        vram_pool.live_vram_bytes(),
        avatar_bytes + attach_bytes + attach_bytes
    );

    // Пользователь переключился в канал #memes на кадре 120;
    // текстуры неактивного канала #general старше max_idle_frames=30 должны быть собраны GC
    let evicted_bytes =
        vram_pool.collect_unused_textures("c-memes", 120, 30, Some(&mut media_cache));
    assert_eq!(evicted_bytes, avatar_bytes + attach_bytes);
    assert_eq!(vram_pool.texture_count(), 1);
    assert_eq!(vram_pool.live_vram_bytes(), attach_bytes);
    assert!(
        media_cache.get("avatar-alice").is_none(),
        "Выгруженная текстура должна удаляться и из MediaCache"
    );
    assert!(media_cache.get("attach-meme-1").is_some());
}

/// ═══════════════════════════════════════════════════════════════════════════
/// SUB-906-3: Бенчмарк холодного старта (< 150 мс Warm / < 300 мс Clean) и CI артефакты
/// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_sub_906_3_cold_start_fmp_warm_cache_150ms_and_clean_boot_300ms() {
    assert_eq!(WARM_COLD_START_BUDGET_MS, 150.0);
    assert_eq!(CLEAN_COLD_START_BUDGET_MS, 300.0);

    // 1. Замер с прогретым кэшем SQLite (< 150 мс до First Meaningful Paint)
    let warm = ColdStartBenchmark::measure_warm_cache_boot()
        .expect("Холодный старт при прогретом кэше SQLite");
    assert_eq!(warm.mode, ColdStartMode::WarmSqliteCache);
    assert!(warm.hydrated_guilds >= 1);
    assert!(warm.hydrated_messages >= 50);
    assert!(warm.first_frame_pixels_verified);
    assert!(
        warm.total_ms() < WARM_COLD_START_BUDGET_MS,
        "Холодный старт (Warm SQLite) занял {:.2} мс (бюджет < 150 мс)",
        warm.total_ms()
    );
    warm.validate_budget()
        .expect("Бюджет < 150 мс должен выполняться");

    // 2. Замер чистого запуска (< 300 мс до First Meaningful Paint)
    let clean =
        ColdStartBenchmark::measure_clean_boot().expect("Холодный старт при чистом запуске");
    assert_eq!(clean.mode, ColdStartMode::CleanBoot);
    assert!(clean.first_frame_pixels_verified);
    assert!(
        clean.total_ms() < CLEAN_COLD_START_BUDGET_MS,
        "Чистый холодный старт занял {:.2} мс (бюджет < 300 мс)",
        clean.total_ms()
    );
    clean
        .validate_budget()
        .expect("Бюджет < 300 мс должен выполняться");

    // 3. Проверка реального бинарника discord-gpui --headless-smoke (VmRSS < 60 МБ и старт < 300 мс)
    let bin = env!("CARGO_BIN_EXE_discord-gpui");
    let proc_start = std::time::Instant::now();
    let out = Command::new(bin)
        .arg("--headless-smoke")
        .env("WAYLAND_DISPLAY", "wayland-1")
        .env("DISPLAY", ":0")
        .output()
        .expect("Запуск бинарника discord-gpui --headless-smoke");
    let proc_elapsed_ms = proc_start.elapsed().as_secs_f64() * 1_000.0;
    assert!(out.status.success());
    assert!(
        proc_elapsed_ms < CLEAN_COLD_START_BUDGET_MS,
        "Полный старт процесса discord-gpui занял {:.2} мс (бюджет < 300 мс)",
        proc_elapsed_ms
    );

    // 4. Негативный тест превышения бюджета холодного старта (150.1 мс > 150.0 мс)
    let mut slow_warm = warm.clone();
    slow_warm.total_us = 150_100;
    assert!(matches!(
        slow_warm.validate_budget(),
        Err(PerfBenchmarkError::ColdStartBudgetExceeded { .. })
    ));
}

#[test]
fn test_sub_906_3_publish_ci_perf_charts_and_reports() {
    let bench = VirtualChatScrollBench::new(100_000, 30).unwrap();
    let (_samples, frame_stats) = bench.run_scroll_benchmark(500).unwrap();
    let memory_snapshot = ProcMemorySnapshot::from_proc_texts(
        "Name:\tdiscord-gpui\nVmSize:\t84000 kB\nVmHWM:\t19200 kB\nVmRSS:\t18432 kB\nVmData:\t11200 kB\n",
        "21000 4608 3000 800 0 2800 0\n",
    )
    .unwrap();
    let (stress_report, profiler) =
        StressSessionSimulator::run_24h_cycle_simulation(240, 0).unwrap();
    let warm_cold_start = ColdStartBenchmark::measure_warm_cache_boot().unwrap();
    let clean_cold_start = ColdStartBenchmark::measure_clean_boot().unwrap();

    let report = CiPerfReport {
        task_id: "TASK-906".into(),
        frame_stats,
        memory_snapshot,
        stress_report,
        warm_cold_start,
        clean_cold_start,
        all_gates_passed: true,
    };

    let out_dir = perf_artifacts_dir();
    let published = CiPerfArtifactPublisher::publish(&out_dir, &report, &profiler)
        .expect("Публикация графиков и отчётов перформанса в артефакты CI");

    assert!(published.json_report_path.exists());
    assert!(published.html_dashboard_path.exists());
    assert!(published.jemalloc_profile_path.exists());
    assert!(published.heaptrack_log_path.exists());

    let html = std::fs::read_to_string(&published.html_dashboard_path).unwrap();
    assert!(html.contains("<svg"));
    assert!(html.contains("120 FPS"));
    assert!(html.contains("8.33 ms"));
    assert!(html.contains("VmRSS"));
    assert!(html.contains("Cold Start"));

    let json_raw = std::fs::read_to_string(&published.json_report_path).unwrap();
    let decoded: CiPerfReport =
        serde_json::from_str(&json_raw).expect("Десериализация CiPerfReport");
    assert_eq!(decoded, report);
}

#[test]
fn test_sub_906_comprehensive_edge_cases_vram_lru_and_allocation_tags() {
    // 1. Все 6 тегов аллокаций имеют уникальные символьные имена для jemalloc
    for tag in AllocationTag::ALL {
        assert!(tag.symbol_name().starts_with("discord_gpui::"));
    }

    // 2. Проверка touch() и защиты от выгрузки недавно использованной текстуры
    let mut pool = VramTexturePool::new(512 * 1024);
    pool.upload_rgba_texture("avatar-1", TextureKind::Avatar, "c-old", 64, 64, 10, None)
        .unwrap();
    assert!(pool.touch("avatar-1", 95));
    assert!(!pool.touch("non-existent", 95));

    // На кадре 100 возраст avatar-1 равен 5 <= max_idle_frames(10) -> текстура сохраняется
    let evicted = pool.collect_unused_textures("c-new", 100, 10, None);
    assert_eq!(evicted, 0);
    assert_eq!(pool.texture_count(), 1);

    // 3. Превышение бюджета VRAM при загрузке слишком большой текстуры -> VramBudgetExceeded
    let overflow = pool.upload_rgba_texture(
        "huge-4k",
        TextureKind::Attachment,
        "c-new",
        1024,
        1024,
        100,
        None,
    );
    assert!(matches!(
        overflow,
        Err(PerfBenchmarkError::VramBudgetExceeded { .. })
    ));
}
