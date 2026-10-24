use discord_gui_spike::messages::{
    ack_message, delete_message, edit_message, reply_message, send_message, DiscordMessageSender,
    MessageActions, MessageError, MessageReceipt, MessageSender, ReadStateAck,
};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct FakeSender {
    result: Result<MessageReceipt, MessageError>,
    calls: Arc<Mutex<Vec<(String, String, String)>>>,
}

impl MessageSender for FakeSender {
    fn send_message<'a>(
        &'a self,
        channel_id: &'a str,
        content: &'a str,
        nonce: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<MessageReceipt, MessageError>> + Send + 'a>> {
        self.calls.lock().unwrap().push((
            channel_id.to_string(),
            content.to_string(),
            nonce.to_string(),
        ));
        let result = self.result.clone();
        Box::pin(async move { result })
    }
}

fn receipt() -> MessageReceipt {
    MessageReceipt {
        id: "m1".to_string(),
        channel_id: "c1".to_string(),
        content: "hello".to_string(),
        nonce: "n1".to_string(),
    }
}

fn run<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(future)
}

fn fake(result: Result<MessageReceipt, MessageError>) -> FakeSender {
    FakeSender {
        result,
        calls: Arc::new(Mutex::new(Vec::new())),
    }
}

#[test]
fn send_message_delegates_and_returns_receipt() {
    let sender = fake(Ok(receipt()));
    assert_eq!(
        run(send_message(&sender, "c1", "hello", "n1")),
        Ok(receipt())
    );
    assert_eq!(
        *sender.calls.lock().unwrap(),
        vec![("c1".to_string(), "hello".to_string(), "n1".to_string())]
    );
}

#[test]
fn send_message_preserves_typed_http_errors() {
    for error in [
        MessageError::Unauthorized,
        MessageError::Forbidden,
        MessageError::NotFound,
        MessageError::RateLimited {
            retry_after_ms: 250,
        },
        MessageError::Client { status: 400 },
        MessageError::Server { status: 503 },
    ] {
        let sender = fake(Err(error.clone()));
        assert_eq!(run(send_message(&sender, "c1", "hello", "n1")), Err(error));
    }
}

#[test]
fn send_message_rejects_empty_or_whitespace_content_before_sender() {
    for content in ["", " \t\n"] {
        let sender = fake(Ok(receipt()));
        assert_eq!(
            run(send_message(&sender, "c1", content, "n1")),
            Err(MessageError::InvalidArg)
        );
        assert!(sender.calls.lock().unwrap().is_empty());
    }
}

#[test]
fn send_message_rejects_content_over_2000_before_sender() {
    let sender = fake(Ok(receipt()));
    let content = "x".repeat(2001);
    assert_eq!(
        run(send_message(&sender, "c1", &content, "n1")),
        Err(MessageError::InvalidArg)
    );
    assert!(sender.calls.lock().unwrap().is_empty());
}

#[test]
fn sender_debug_does_not_reveal_token() {
    let sender = DiscordMessageSender::new("secret-token").expect("sender config");
    assert!(!format!("{sender:?}").contains("secret-token"));
}

#[test]
fn tauri_registers_send_message_command() {
    let source = include_str!("../src/main.rs");
    let start = source.find("generate_handler![").expect("handler") + "generate_handler![".len();
    let end = source[start..].find(']').expect("handler close");
    let commands = &source[start..start + end];
    assert!(commands
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .any(|name| name == "send_message"));
}

#[derive(Clone)]
struct FakeActions {
    calls: Arc<Mutex<Vec<String>>>,
    result: Result<(), MessageError>,
}

impl MessageActions for FakeActions {
    fn edit_message<'a>(
        &'a self,
        channel: &'a str,
        message: &'a str,
        content: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), MessageError>> + Send + 'a>> {
        self.calls.lock().unwrap().push(format!(
            "PATCH /channels/{channel}/messages/{message} {content}"
        ));
        let result = self.result.clone();
        Box::pin(async move { result })
    }

    fn delete_message<'a>(
        &'a self,
        channel: &'a str,
        message: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), MessageError>> + Send + 'a>> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("DELETE /channels/{channel}/messages/{message}"));
        let result = self.result.clone();
        Box::pin(async move { result })
    }

    fn reply_message<'a>(
        &'a self,
        channel: &'a str,
        content: &'a str,
        reply_to: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), MessageError>> + Send + 'a>> {
        self.calls.lock().unwrap().push(format!(
            "POST /channels/{channel}/messages {content} reference={reply_to}"
        ));
        let result = self.result.clone();
        Box::pin(async move { result })
    }
}

fn actions(result: Result<(), MessageError>) -> FakeActions {
    FakeActions {
        calls: Arc::new(Mutex::new(Vec::new())),
        result,
    }
}

#[test]
fn message_actions_delegate_to_expected_paths_and_payloads() {
    let fake = actions(Ok(()));
    assert_eq!(run(edit_message(&fake, "10", "20", "updated")), Ok(()));
    assert_eq!(run(delete_message(&fake, "10", "20")), Ok(()));
    assert_eq!(run(reply_message(&fake, "10", "hello", "20")), Ok(()));
    assert_eq!(
        *fake.calls.lock().unwrap(),
        vec![
            "PATCH /channels/10/messages/20 updated",
            "DELETE /channels/10/messages/20",
            "POST /channels/10/messages hello reference=20",
        ]
    );
}

#[test]
fn message_actions_reject_invalid_ids_and_content_before_io() {
    for (channel, message) in [("x", "20"), ("10", "x"), ("", "20")] {
        let fake = actions(Ok(()));
        assert_eq!(
            run(delete_message(&fake, channel, message)),
            Err(MessageError::InvalidArg)
        );
        assert!(fake.calls.lock().unwrap().is_empty());
    }
    for content in ["", " \t\n", &"x".repeat(2001)] {
        let fake = actions(Ok(()));
        assert_eq!(
            run(edit_message(&fake, "10", "20", content)),
            Err(MessageError::InvalidArg)
        );
        assert_eq!(
            run(reply_message(&fake, "10", content, "20")),
            Err(MessageError::InvalidArg)
        );
        assert!(fake.calls.lock().unwrap().is_empty());
    }
}

#[test]
fn actions_preserve_http_errors_and_debug_hides_token() {
    for error in [
        MessageError::Unauthorized,
        MessageError::Forbidden,
        MessageError::NotFound,
        MessageError::RateLimited {
            retry_after_ms: 250,
        },
        MessageError::Client { status: 400 },
        MessageError::Server { status: 500 },
        MessageError::Network,
        MessageError::Storage,
    ] {
        let fake = actions(Err(error.clone()));
        assert_eq!(run(delete_message(&fake, "10", "20")), Err(error));
    }
    // SEC-02: отказ хранилища — собственный kind, а не «сеть».
    let value = serde_json::to_value(MessageError::Storage).expect("storage");
    assert_eq!(value["kind"], "storage", "{value}");
    assert!(!MessageError::Storage.to_string().is_empty());
    let sender = DiscordMessageSender::new("secret-token").expect("sender config");
    assert!(!format!("{sender:?}").contains("secret-token"));
}

#[test]
fn tauri_registers_message_action_commands() {
    let source = include_str!("../src/main.rs");
    let start = source.find("generate_handler![").expect("handler") + "generate_handler![".len();
    let end = source[start..].find(']').expect("handler close");
    let commands = &source[start..start + end];
    for expected in [
        "edit_message",
        "delete_message",
        "reply_message",
        "ack_message",
    ] {
        assert!(commands
            .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .any(|name| name == expected));
    }
    for command in [
        "edit_message",
        "delete_message",
        "reply_message",
        "ack_message",
    ] {
        let start = source
            .find(&format!("async fn {command}("))
            .expect("command body");
        let body = &source[start..source[start..].find("\n}\n").expect("command close") + start];
        assert!(
            body.contains("backend.token()"),
            "{command} must obtain the token from AuthBackend"
        );
    }
}

// ---------------------------------------------------------------------------
// M-07: модульный контракт ack (тело/транспорт — в `tests/rest_ack.rs`).
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct FakeAck {
    calls: Arc<Mutex<Vec<AckCall>>>,
    result: Result<(), MessageError>,
}

/// (channel_id, message_id, last_viewed, nullable body token)
type AckCall = (String, String, u64, Option<String>);

impl ReadStateAck for FakeAck {
    fn ack_message<'a>(
        &'a self,
        channel_id: &'a str,
        message_id: &'a str,
        last_viewed: u64,
        token: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<(), MessageError>> + Send + 'a>> {
        self.calls.lock().unwrap().push((
            channel_id.to_string(),
            message_id.to_string(),
            last_viewed,
            token.clone(),
        ));
        let result = self.result.clone();
        Box::pin(async move { result })
    }
}

fn ack(result: Result<(), MessageError>) -> FakeAck {
    FakeAck {
        calls: Arc::new(Mutex::new(Vec::new())),
        result,
    }
}

#[test]
fn ack_message_delegates_counter_and_nullable_token_verbatim() {
    let fake = ack(Ok(()));
    let message_id = "1561348877573947515";
    assert_eq!(
        run(ack_message(
            &fake,
            "1293192721237938187",
            message_id,
            4310,
            None
        )),
        Ok(())
    );
    assert_eq!(
        run(ack_message(
            &fake,
            "1293192721237938187",
            message_id,
            0,
            Some("read-state".to_string())
        )),
        Ok(())
    );
    assert_eq!(
        *fake.calls.lock().unwrap(),
        vec![
            (
                "1293192721237938187".to_string(),
                message_id.to_string(),
                4310,
                None
            ),
            (
                "1293192721237938187".to_string(),
                message_id.to_string(),
                0,
                Some("read-state".to_string())
            ),
        ]
    );
}

#[test]
fn ack_message_preserves_typed_errors() {
    for error in [
        MessageError::Unauthorized,
        MessageError::Forbidden,
        MessageError::NotFound,
        MessageError::RateLimited {
            retry_after_ms: 900,
        },
        MessageError::Client { status: 400 },
        MessageError::Server { status: 500 },
        MessageError::Network,
    ] {
        let fake = ack(Err(error.clone()));
        assert_eq!(run(ack_message(&fake, "1", "2", 4310, None)), Err(error));
    }
}
