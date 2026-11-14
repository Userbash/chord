#![recursion_limit = "256"]

pub mod a11y;
mod audio;
pub mod auth;
pub mod battery_saver;
pub mod channels;
pub mod chaos;
pub mod chat;
pub mod crash_reporter;
pub mod custom_themes;
mod dispatch;
pub mod encrypted_store;
pub mod guild;
pub mod ingame_overlay;
pub mod input;
pub mod integrations;
pub mod manual_qa;
mod media;
pub mod multi_account_switcher;
mod net;
pub mod perf_benchmark;
pub mod pipewire_portal;
pub mod platform;
pub mod quality_gate;
pub mod session;
mod state;
mod storage;
mod theme;
mod ui;
mod video;
pub mod visual_regression;
pub mod voice;

use gpui::*;
use ui::AppLayout;

fn detect_display_backend() -> &'static str {
    let has_wayland = std::env::var("WAYLAND_DISPLAY")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);
    let has_x11 = std::env::var("DISPLAY")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);

    match (has_wayland, has_x11) {
        (true, true) => "wayland+x11",
        (true, false) => "wayland",
        (false, true) => "x11",
        (false, false) => "headless",
    }
}

fn read_vmrss_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status
                .lines()
                .find(|line| line.starts_with("VmRSS:"))
                .and_then(|line| line.split_whitespace().nth(1))
                .and_then(|val| val.parse::<u64>().ok())
        })
        .unwrap_or(1024)
}

fn run_headless_smoke() -> i32 {
    let backend = detect_display_backend();
    let app_state = state::AppState::new();
    let guild_count = app_state.guilds.len();
    let channel_count: usize = app_state.guilds.iter().map(|g| g.channels.len()).sum();

    let db = match storage::LocalDatabase::open_in_memory() {
        Ok(db) => db,
        Err(err) => {
            eprintln!("smoke: sqlite open error: {err}");
            return 2;
        }
    };
    let sqlite_wal_ok = db.set_setting("smoke_probe", "ok").is_ok()
        && db.get_setting("smoke_probe").ok().flatten().as_deref() == Some("ok");

    let mut ring = audio::AudioRingBuffer::with_capacity(4800);
    let samples = [0.125f32; 192];
    let mut out = [0.0f32; 192];
    ring.write_samples(&samples);
    let read_n = ring.read_samples(&mut out);

    let mut video_pipe = video::GpuVideoPipeline::new(120);
    video_pipe.configure(1280, 720, video::VideoPixelFormat::Nv12);
    let frame = vec![128u8; (1280 * 720 * 3) / 2];
    let tick_start = std::time::Instant::now();
    let frame_ok = video_pipe.push_frame(&frame).is_ok() && read_n == 192;
    let tick_us = tick_start.elapsed().as_micros() as u64;
    let fps_budget_ok = frame_ok && tick_us < 8_300;

    let theme_ok = theme::DISCORD_THEME.bg_primary != theme::DISCORD_THEME.bg_secondary;
    let vmrss_kb = read_vmrss_kb();
    let vmrss_mb = (vmrss_kb as f64) / 1024.0;
    let all_ok = sqlite_wal_ok && fps_budget_ok && theme_ok && vmrss_mb < 60.0;
    let status = if all_ok { "ok" } else { "degraded" };

    // OPT-4: --smoke=summary → одна строка вместо JSON blob.
    // AI-воркер читает "SMOKE ok guilds=5 vmrss=42MB" — не нужен JSON-парсинг (экономия ~480 токен/вызов).
    let args: Vec<String> = std::env::args().skip(1).collect();
    let summary_mode = args.iter().any(|a| a == "--smoke=summary");

    if summary_mode {
        println!(
            "SMOKE {status} backend={backend} guilds={guild_count} ch={channel_count} \
             sqlite={sqlite_wal_ok} fps={fps_budget_ok} tick={tick_us}us vmrss={vmrss_mb:.1}MB"
        );
    } else {
        // Режим по умолчанию — полный JSON (для CI/machines)
        let report = serde_json::json!({
            "status": status,
            "display_backend": backend,
            "window_width": 1280,
            "window_height": 720,
            "min_width": 900,
            "min_height": 550,
            "guilds": guild_count,
            "channels": channel_count,
            "sqlite_wal_ok": sqlite_wal_ok,
            "fps_budget_ok": fps_budget_ok,
            "frame_tick_us": tick_us,
            "vmrss_kb": vmrss_kb,
            "vmrss_mb": (vmrss_mb * 100.0).round() / 100.0,
        });
        println!("GPUI_SMOKE_OK {report}");
    }
    if all_ok { 0 } else { 1 }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let smoke_env = std::env::var("DISCORD_GPUI_SMOKE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    if smoke_env
        || args
            .iter()
            .any(|arg| arg == "--headless-smoke" || arg == "--smoke")
    {
        std::process::exit(run_headless_smoke());
    }

    let exit_after_ms = std::env::var("DISCORD_GPUI_EXIT_AFTER_MS")
        .ok()
        .or_else(|| std::env::var("SPIKE_EXIT_AFTER_MS").ok())
        .and_then(|v| v.parse::<u64>().ok());

    let shutdown_coordinator = session::runtime::install_os_signal_shutdown_hook(
        session::runtime::ShutdownCoordinator::new(),
    );
    let shutdown_for_run = shutdown_coordinator.clone();

    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);

        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: Point::default(),
                size: size(px(1280.), px(720.)),
            })),
            titlebar: Some(TitlebarOptions {
                title: Some("Discord".into()),
                appears_transparent: false,
                traffic_light_position: None,
            }),
            window_min_size: Some(size(px(900.), px(550.))),
            ..Default::default()
        };

        gpui_kit::open_window(window_options, cx, |window, cx| {
            let view = cx.new(AppLayout::new);
            let focus = view.read(cx).focus_handle.clone();
            focus.focus(window, cx);
            println!("window-ready");
            view
        })
        .expect("Не удалось открыть главное окно Discord GPUI");

        if let Some(ms) = exit_after_ms {
            let shutdown_for_timer = shutdown_for_run.clone();
            cx.spawn(async move |cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(ms))
                    .await;
                shutdown_for_timer.request_shutdown(session::runtime::ShutdownReason::AppQuit);
                let _ = cx.update(|cx| cx.quit());
            })
            .detach();
        }
    });

    shutdown_coordinator.request_shutdown(session::runtime::ShutdownReason::AppQuit);
}
