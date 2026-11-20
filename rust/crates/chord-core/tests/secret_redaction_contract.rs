#![forbid(unsafe_code)]

use std::sync::{Arc, Mutex};
use chord_core::{
    redact_secrets, RedactedWriter, REDACTED_MARKER,
};

#[test]
fn test_redact_secrets_bot_token() {
    let dummy_bot = format!("Authorization: Bot {}.{}.{}", "MTIzNDU2Nzg5MDEyMzQ1Njc4OQ", "GaBcDe", "AbCdEfGhIjKlMnOpQrStUvWxYz012345678");
    let output = redact_secrets(&dummy_bot);
    assert_eq!(output, format!("Authorization: Bot {REDACTED_MARKER}"));
    assert!(!output.contains("MTIzNDU2Nzg5MDEyMzQ1Njc4OQ"));
}

#[test]
fn test_redact_secrets_bearer_token() {
    let input = "Authorization: Bearer ya29.a0AfH6SM_secret_token_value_here";
    let output = redact_secrets(input);
    assert_eq!(output, format!("Authorization: Bearer {REDACTED_MARKER}"));
    assert!(!output.contains("ya29.a0AfH6SM"));
}

#[test]
fn test_redact_secrets_discord_token_2dots() {
    let dummy_token = format!("{}.{}.{}", "MTIzNDU2Nzg5MDEyMzQ1Njc4OQ", "GaBcDe", "AbCdEfGhIjKlMnOpQrStUvWxYz012345678");
    let input = format!("User token is {dummy_token} in session");
    let output = redact_secrets(&input);
    assert_eq!(output, format!("User token is {REDACTED_MARKER} in session"));
    assert!(!output.contains("MTIzNDU2Nzg5MDEyMzQ1Njc4OQ"));
}

#[test]
fn test_redact_secrets_mfa_token() {
    let input = "Authentication success: mfa.abcdef1234567890abcdef1234567890 confirmed";
    let output = redact_secrets(input);
    assert_eq!(output, format!("Authentication success: {REDACTED_MARKER} confirmed"));
    assert!(!output.contains("abcdef1234567890"));
}

#[test]
fn test_redact_secrets_hex_encryption_key_64() {
    let input = "SRTP master key initialized: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let output = redact_secrets(input);
    assert_eq!(output, format!("SRTP master key initialized: {REDACTED_MARKER}"));
    assert!(!output.contains("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"));
}

#[test]
fn test_redact_secrets_key_value_assignments() {
    let input = "Connecting: secret_key=deadbeef0123456789abcdef password=SuperSecretPassword123! token=mfa.secretTokenVal999";
    let output = redact_secrets(input);
    assert!(output.contains(&format!("secret_key={REDACTED_MARKER}")));
    assert!(output.contains(&format!("password={REDACTED_MARKER}")));
    assert!(output.contains(&format!("token={REDACTED_MARKER}")));
    assert!(!output.contains("deadbeef0123456789abcdef"));
    assert!(!output.contains("SuperSecretPassword123!"));
    assert!(!output.contains("secretTokenVal999"));
}

#[test]
fn test_redact_secrets_json_fields() {
    let input = r#"Payload: {"token": "secret_session_token_123", "secret_key": "4f6a7b8c9d0e", "status": 200}"#;
    let output = redact_secrets(input);
    assert!(output.contains(&format!(r#""token": "{REDACTED_MARKER}""#)));
    assert!(output.contains(&format!(r#""secret_key": "{REDACTED_MARKER}""#)));
    assert!(output.contains(r#""status": 200"#));
    assert!(!output.contains("secret_session_token_123"));
}

#[test]
fn test_redact_secrets_preserves_safe_text() {
    let input = "Connecting to gateway wss://gateway.discord.gg?v=9&encoding=etf with heartbeat 41250ms";
    let output = redact_secrets(input);
    assert_eq!(output, input);
}

#[test]
fn test_redacted_writer_intercepts_and_masks_output() {
    use std::io::Write;

    let buffer = Arc::new(Mutex::new(Vec::new()));

    #[derive(Clone)]
    struct SharedBuffer(Arc<Mutex<Vec<u8>>>);
    impl Write for SharedBuffer {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let shared = SharedBuffer(Arc::clone(&buffer));
    let mut writer = RedactedWriter::new(shared);

    let dummy_bot = format!("Received Bot {}.{}.{}", "MTIzNDU2Nzg5MDEyMzQ1Njc4OQ", "GaBcDe", "AbCdEfGhIjKlMnOpQrStUvWxYz012345678");
    writeln!(writer, "{dummy_bot}").unwrap();
    writeln!(writer, "Using key=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef").unwrap();
    writer.flush().unwrap();

    let captured = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
    assert!(captured.contains(&format!("Bot {REDACTED_MARKER}")));
    assert!(captured.contains(&format!("key={REDACTED_MARKER}")));
    assert!(!captured.contains("MTIzNDU2Nzg5MDEyMzQ1Njc4OQ"));
    assert!(!captured.contains("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"));
}

#[test]
fn test_tracing_subscriber_integration_masks_logged_events() {
    use chord_core::{RedactedMakeWriter, SecretRedactionLayer};
    use tracing_subscriber::prelude::*;

    let buffer = Arc::new(Mutex::new(Vec::new()));
    let make_writer = RedactedMakeWriter::from_shared(Arc::clone(&buffer));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(make_writer)
        .with_ansi(false);

    let subscriber = tracing_subscriber::registry()
        .with(fmt_layer)
        .with(SecretRedactionLayer::new());

    tracing::subscriber::with_default(subscriber, || {
        let msg = format!("Starting voice session with Authorization: Bot {}.{}.{}", "MTIzNDU2Nzg5MDEyMzQ1Njc4OQ", "GaBcDe", "AbCdEfGhIjKlMnOpQrStUvWxYz012345678");
        tracing::info!("{msg}");
        tracing::info!("DAVE MLS key: 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef");
    });

    let captured = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
    assert!(captured.contains(&format!("Bot {REDACTED_MARKER}")));
    assert!(captured.contains(REDACTED_MARKER));
    assert!(!captured.contains("MTIzNDU2Nzg5MDEyMzQ1Njc4OQ"));
    assert!(!captured.contains("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"));
}
