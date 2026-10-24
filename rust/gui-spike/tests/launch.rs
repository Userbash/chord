//! Запуск реального Tauri-бинаря: окно создаётся, UI отдаёт проверяемый UI_SNAPSHOT,
//! в stderr нет GBM/Gdk-Message/panic/segfault.
//!
//! Живая проверка: при наличии дисплея запускается всегда (TEST-POLICY §9);
//! без дисплея — skip через ранний return с eprintln (skip-aware, не красный на headless CI).

use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

/// Жёсткий таймаут запуска: спайк сам выходит по SPIKE_EXIT_AFTER_MS=9000,
/// запас нужен на холодный старт WebKit в CI.
const LAUNCH_TIMEOUT: Duration = Duration::from_secs(20);
/// Сколько обязан прожить процесс: окно не должно исчезать раньше своего же срока.
const SPIKE_EXIT_AFTER_MS: u64 = 9_000;
const LIVENESS_MARGIN: Duration = Duration::from_millis(1_000);

struct LaunchOutcome {
    status: Option<ExitStatus>,
    stdout: String,
    stderr: String,
    timed_out: bool,
    lived: Duration,
}

/// `SPIKE_BIN_OVERRIDE` — только для мутационной проверки (P2-3): подмена бинаря
/// сценарием «окно умерло раньше срока». По умолчанию — собранный бинарь спайка.
fn spike_binary() -> PathBuf {
    std::env::var_os("SPIKE_BIN_OVERRIDE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_discord-gui-spike")))
}

/// Запуск с таймаутом и параллельным чтением обоих потоков (иначе пайп переполнится
/// и процесс встанет до нашего wait). По таймауту — kill и ожидание.
fn launch_with_timeout(mut command: Command) -> LaunchOutcome {
    let started = Instant::now();
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("не удалось запустить discord-gui-spike");

    let stdout = child.stdout.take().expect("stdout подключён к пайпу");
    let stderr = child.stderr.take().expect("stderr подключён к пайпу");
    let stdout_reader = std::thread::spawn(move || drain(stdout));
    let stderr_reader = std::thread::spawn(move || drain(stderr));

    let deadline = Instant::now() + LAUNCH_TIMEOUT;
    let mut timed_out = false;
    let status = loop {
        match child.try_wait().expect("try_wait для discord-gui-spike") {
            Some(status) => break Some(status),
            None if Instant::now() >= deadline => {
                timed_out = true;
                let _ = child.kill();
                break child.wait().ok();
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    let lived = started.elapsed();

    LaunchOutcome {
        status,
        stdout: stdout_reader.join().expect("поток чтения stdout"),
        stderr: stderr_reader.join().expect("поток чтения stderr"),
        timed_out,
        lived,
    }
}

fn drain(mut stream: impl Read) -> String {
    let mut buffer = Vec::new();
    // Процесс завершился/убит → EOF; ошибка чтения не должна маскировать вывод.
    let _ = stream.read_to_end(&mut buffer);
    String::from_utf8_lossy(&buffer).into_owned()
}

fn has_display() -> bool {
    ["DISPLAY", "WAYLAND_DISPLAY"]
        .iter()
        .any(|key| std::env::var(key).is_ok_and(|value| !value.trim().is_empty()))
}

/// Поля UI_SNAPSHOT, которые обязана подтвердить живая сессия.
const SNAPSHOT_EXPECTATIONS: [(&str, u64); 5] = [
    ("screenCount", 4),
    ("chatRows", 100_000),
    ("memberRows", 24_229),
    ("emojiCategories", 10),
    ("settingsToggles", 38),
];

#[test]
fn launches_real_window_and_reports_ui_snapshot() {
    // Политика TEST-POLICY §9: на дисплее live-проверка обязательна; skip только headless.
    if !has_display() {
        eprintln!("skip: нет DISPLAY/WAYLAND_DISPLAY — live-запуск Tauri невозможен");
        return;
    }

    let mut command = Command::new(spike_binary());
    command
        .env("SPIKE_EXIT_AFTER_MS", SPIKE_EXIT_AFTER_MS.to_string())
        .env("SPIKE_AUTOBENCH", "0")
        .env("GDK_BACKEND", "x11")
        .env("WEBKIT_DISABLE_DMABUF_RENDERER", "1")
        .env("WEBKIT_DISABLE_COMPOSITING_MODE", "1")
        .env("LIBGL_ALWAYS_SOFTWARE", "1")
        .env("GTK_MODULES", "gail:atk-bridge");

    let outcome = launch_with_timeout(command);
    let LaunchOutcome {
        status,
        stdout,
        stderr,
        timed_out,
        lived,
    } = outcome;

    assert!(
        !timed_out,
        "спайк не завершился за {LAUNCH_TIMEOUT:?}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let status = status.expect("процесс не отдал exit status");
    assert!(
        status.success(),
        "exit={status}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    // P2-3: snapshot не доказывает, что окно живо; ранний exit 0 — провал.
    let min_alive = Duration::from_millis(SPIKE_EXIT_AFTER_MS) - LIVENESS_MARGIN;
    assert!(
        lived >= min_alive,
        "окно прожило {lived:?} < {min_alive:?} (SPIKE_EXIT_AFTER_MS={SPIKE_EXIT_AFTER_MS}): \
         преждевременный exit 0\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("window-ready"),
        "нет маркера window-ready\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    let line = stdout
        .lines()
        .find(|line| line.starts_with("UI_SNAPSHOT "))
        .unwrap_or_else(|| panic!("UI_SNAPSHOT отсутствует\nstdout:\n{stdout}\nstderr:\n{stderr}"));
    let snapshot: serde_json::Value =
        serde_json::from_str(line.trim_start_matches("UI_SNAPSHOT ")).expect("UI_SNAPSHOT — JSON");
    for (field, expected) in SNAPSHOT_EXPECTATIONS {
        assert_eq!(
            snapshot[field], expected,
            "UI_SNAPSHOT.{field} не совпал: {snapshot}\nstderr:\n{stderr}"
        );
    }

    let stderr_lower = stderr.to_lowercase();
    for forbidden in [
        "failed to create gbm buffer",
        "gdk-message:",
        "panic",
        "segmentation fault",
    ] {
        assert!(
            !stderr_lower.contains(forbidden),
            "в stderr запрещённый маркер {forbidden:?}:\n{stderr}"
        );
    }
}
