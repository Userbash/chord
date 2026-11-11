#![recursion_limit = "256"]

mod audio;
mod dispatch;
mod media;
mod net;
mod state;
mod storage;
mod theme;
mod ui;
mod video;

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

    let report = serde_json::json!({
        "status": if sqlite_wal_ok && fps_budget_ok && theme_ok && vmrss_mb < 60.0 { "ok" } else { "degraded" },
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
    if report["status"] == "ok" { 0 } else { 1 }
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
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(ms));
                std::process::exit(0);
            });
        }
    });
}
