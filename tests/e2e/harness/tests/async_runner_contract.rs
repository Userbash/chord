//! H-FIX-11: Контрактный тест асинхронного запуска дочерних бинарников с kill-on-timeout и reap без зомби.

use std::path::Path;
use std::time::{Duration, Instant};

use chord_gpui_e2e_harness::{E2eRunner, E2eRunnerConfig, HarnessError};
use tokio::sync::Mutex;

static PROCESS_HYGIENE_LOCK: Mutex<()> = Mutex::const_new(());

#[tokio::test]
async fn h_fix_11_fast_command_succeeds_captures_output_and_masks_secrets() {
    let _guard = PROCESS_HYGIENE_LOCK.lock().await;
    let mut runner = E2eRunner::boot(E2eRunnerConfig::default())
        .await
        .expect("boot E2eRunner");

    let sh_bin = Path::new("/bin/sh");
    let res = runner
        .run_binary(
            sh_bin,
            &["-c", "printf 'hello %s' \"$SECRET_TOKEN\""],
            &[("SECRET_TOKEN", "top-secret-token-42")],
        )
        .await
        .expect("fast command should succeed");

    assert_eq!(res.exit_code, Some(0));
    assert_eq!(res.stdout, "hello [REDACTED_TOKEN]");
    assert!(
        !res.stdout.contains("top-secret-token-42"),
        "secret must be masked in stdout: {}",
        res.stdout
    );
    assert!(res.panic_report.is_none());

    let panic_res = runner
        .run_binary(
            sh_bin,
            &[
                "-c",
                "printf \"thread 'main' panicked at src/main.rs:10:5:\\nboom %s\\nstack backtrace:\\n   0: main\\n\" \"$SECRET_TOKEN\" >&2; exit 101",
            ],
            &[("SECRET_TOKEN", "top-secret-token-42")],
        )
        .await
        .expect("panic command should return E2eRunResult");

    assert_eq!(panic_res.exit_code, Some(101));
    assert!(!panic_res.stderr.contains("top-secret-token-42"));
    let report = panic_res.panic_report.expect("panic_report must be extracted");
    assert_eq!(report.location.as_deref(), Some("src/main.rs:10:5"));
    assert_eq!(report.message, "boom [REDACTED_TOKEN]");

    let hygiene = runner.shutdown().await.expect("shutdown E2eRunner");
    assert_eq!(hygiene.zombie_processes, 0);
    assert_eq!(hygiene.lingering_children, 0);
}

#[tokio::test]
async fn h_fix_11_missing_binary_returns_io_error() {
    let _guard = PROCESS_HYGIENE_LOCK.lock().await;
    let mut runner = E2eRunner::boot(E2eRunnerConfig::default())
        .await
        .expect("boot E2eRunner");

    let missing_bin = Path::new("/nonexistent/path/to/missing_binary_hfix11");
    let err = runner
        .run_binary(missing_bin, &[], &[])
        .await
        .expect_err("missing binary must return Err");

    assert!(
        matches!(err, HarnessError::Io(_)),
        "expected HarnessError::Io, got {err:?}"
    );

    let hygiene = runner.shutdown().await.expect("shutdown E2eRunner");
    assert_eq!(hygiene.zombie_processes, 0);
    assert_eq!(hygiene.lingering_children, 0);
}

#[tokio::test]
async fn h_fix_11_hanging_binary_times_out_quickly_and_kills_and_reaps_child() {
    let _guard = PROCESS_HYGIENE_LOCK.lock().await;
    let config = E2eRunnerConfig {
        command_timeout: Duration::from_millis(150),
        ..E2eRunnerConfig::default()
    };
    let mut runner = E2eRunner::boot(config).await.expect("boot E2eRunner");

    let sleep_bin = Path::new("/bin/sleep");
    let start = Instant::now();
    let err = runner
        .run_binary(sleep_bin, &["30"], &[])
        .await
        .expect_err("hanging binary must time out");
    let elapsed = start.elapsed();

    assert!(
        matches!(err, HarnessError::Timeout(_)),
        "expected HarnessError::Timeout, got {err:?}"
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "timeout took too long: {elapsed:?} (must be < 2s)"
    );

    let hygiene = runner.shutdown().await.expect("shutdown E2eRunner");
    assert_eq!(
        hygiene.zombie_processes, 0,
        "timed-out child must be reaped with zero zombie processes"
    );
    assert_eq!(
        hygiene.lingering_children, 0,
        "timed-out child must be killed with zero lingering child processes"
    );
}
