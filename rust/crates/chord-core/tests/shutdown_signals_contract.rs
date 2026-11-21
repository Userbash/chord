#![forbid(unsafe_code)]

use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use chord_core::shutdown::{
    DEFAULT_SHUTDOWN_TIMEOUT, RedactingWriter, ShutdownCoordinator, ShutdownError, ShutdownSignal,
    redact_secrets,
};

#[test]
fn test_shutdown_coordinator_default_timeout() {
    let coordinator = ShutdownCoordinator::new();
    assert_eq!(coordinator.timeout(), DEFAULT_SHUTDOWN_TIMEOUT);
    assert_eq!(DEFAULT_SHUTDOWN_TIMEOUT, Duration::from_secs(5));

    let custom = ShutdownCoordinator::new().with_timeout(Duration::from_millis(150));
    assert_eq!(custom.timeout(), Duration::from_millis(150));
}

#[tokio::test]
async fn test_shutdown_coordinator_joinset_tasks_graceful_completion() {
    let coordinator = ShutdownCoordinator::new().with_timeout(Duration::from_secs(2));
    let completed = Arc::new(AtomicUsize::new(0));

    for _ in 0..5 {
        let counter = Arc::clone(&completed);
        let token = coordinator.child_token();
        coordinator.spawn_task(async move {
            token.cancelled().await;
            counter.fetch_add(1, Ordering::SeqCst);
        });
    }

    assert_eq!(coordinator.task_count(), 5);
    tokio::time::sleep(Duration::from_millis(15)).await;
    assert_eq!(completed.load(Ordering::SeqCst), 0);

    let res = coordinator.shutdown_and_join().await;
    assert!(res.is_ok(), "Graceful shutdown should succeed: {:?}", res);
    assert_eq!(completed.load(Ordering::SeqCst), 5);
    assert_eq!(coordinator.task_count(), 0);
}

#[tokio::test]
async fn test_shutdown_coordinator_joinset_timeout_aborts_hanging_tasks() {
    let coordinator = ShutdownCoordinator::new().with_timeout(Duration::from_millis(50));
    let unstopped = Arc::new(AtomicBool::new(true));
    let unstopped_clone = Arc::clone(&unstopped);

    // This task deliberately ignores cancellation and hangs
    coordinator.spawn_task(async move {
        tokio::time::sleep(Duration::from_secs(10)).await;
        unstopped_clone.store(false, Ordering::SeqCst);
    });

    assert_eq!(coordinator.task_count(), 1);

    let start = std::time::Instant::now();
    let res = coordinator.shutdown_and_join().await;
    let elapsed = start.elapsed();

    assert!(res.is_err(), "Hanging task must trigger timeout error");
    match res {
        Err(ShutdownError::Timeout { .. }) => {}
        other => panic!("Expected ShutdownError::Timeout, got {:?}", other),
    }

    assert!(
        elapsed < Duration::from_millis(500),
        "Shutdown must not hang beyond timeout, took {:?}",
        elapsed
    );
    assert_eq!(
        coordinator.task_count(),
        0,
        "Tasks must be aborted on timeout"
    );
}

#[test]
fn test_shutdown_signals_sigint_and_sigterm_types() {
    assert_eq!(ShutdownSignal::Sigint.as_str(), "SIGINT");
    assert_eq!(ShutdownSignal::Sigterm.as_str(), "SIGTERM");
    assert_eq!(ShutdownSignal::Manual.as_str(), "MANUAL");

    assert!(ShutdownSignal::Sigint.is_signal());
    assert!(ShutdownSignal::Sigterm.is_signal());
    assert!(!ShutdownSignal::Manual.is_signal());
}

#[test]
fn test_secret_redaction_bot_and_bearer_tokens() {
    let dummy_bot = format!(
        "Connecting with header: Bot {}.{}.{}",
        "MTIzNDU2Nzg5MDEyMzQ1Njc4", "GaBcDe", "abcdefghijklmnopqrstuvwxyz0123456789"
    );
    let redacted_bot = redact_secrets(&dummy_bot);
    assert!(
        !redacted_bot.contains("MTIzNDU2Nzg5MDEyMzQ1Njc4"),
        "Bot token must not leak in stdout/logs"
    );
    assert!(
        redacted_bot.contains("[REDACTED]"),
        "Bot token must be masked with [REDACTED]"
    );

    let sample_bearer = "OAuth token Bearer 1234567890abcdefghijklmnopqrstuvwxyz1234567890abcdefghijklmnopqrstuvwxyz";
    let redacted_bearer = redact_secrets(sample_bearer);
    assert!(
        !redacted_bearer.contains("1234567890abcdefghijklmnopqrstuvwxyz1234567890"),
        "Bearer token must not leak in stdout/logs"
    );
    assert!(
        redacted_bearer.contains("[REDACTED]"),
        "Bearer token must be masked with [REDACTED]"
    );
}

#[test]
fn test_secret_redaction_mfa_and_private_keys() {
    let sample_mfa =
        "mfa.abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789abcdefghij";
    let redacted_mfa = redact_secrets(sample_mfa);
    assert!(
        !redacted_mfa.contains("abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
        "MFA token must not leak"
    );
    assert!(
        redacted_mfa.contains("[REDACTED]"),
        "MFA token must be masked with [REDACTED]"
    );

    let pem = "-----BEGIN PRIVATE KEY-----\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQD\n-----END PRIVATE KEY-----";
    let redacted_pem = redact_secrets(pem);
    assert!(
        !redacted_pem.contains("MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQD"),
        "Private key PEM must not leak"
    );
    assert!(
        redacted_pem.contains("[REDACTED]"),
        "Private key PEM must be masked with [REDACTED]"
    );

    let hex_key = "private_key = a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90";
    let redacted_hex = redact_secrets(hex_key);
    assert!(
        !redacted_hex.contains("a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90"),
        "Hex private key must not leak"
    );
    assert!(
        redacted_hex.contains("[REDACTED]"),
        "Hex private key must be masked with [REDACTED]"
    );
}

#[test]
fn test_redacting_writer_stdout_stream() {
    let mut buffer = Vec::new();
    {
        let dummy_bot = format!(
            "INFO: authorized with Bot {}.{}.{}",
            "MTIzNDU2Nzg5MDEyMzQ1Njc4", "GaBcDe", "abcdefghijklmnopqrstuvwxyz0123456789"
        );
        let mut writer = RedactingWriter::new(&mut buffer);
        writeln!(writer, "{dummy_bot}").expect("write must succeed");
        writer.flush().expect("flush must succeed");
    }

    let output = String::from_utf8(buffer).expect("valid utf8");
    assert!(
        !output.contains("MTIzNDU2Nzg5MDEyMzQ1Njc4"),
        "Writer must redact Bot token from stream"
    );
    assert!(
        output.contains("[REDACTED]"),
        "Writer must output [REDACTED] in stream"
    );
}
