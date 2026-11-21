use chord_core::rest::transport::HttpResponse;
use chord_client::messages::{
    ack_message, delete_message, edit_message, reply_message, send_message, AckBulkEntry,
    AckMessageOptions, AckTransport, DiscordMessageSender, DiscordReadState, MessageActions,
    MessageError, MessageReceipt, MessageSender, NonBlockingAckDispatcher, OptimisticMessagesCache,
    ReadStateAck, ACK_BULK_PATH, MAX_ACK_BULK_ENTRIES,
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

/// PLAT-11-B (T2): `channelId` уходит в URL-путь, поэтому path-significant
/// значения отвергаются до любого I/O. Оракул — инвариант §2.5 аудита
/// PLAT-11 (нет `/`, `\`, `..`, `%2f`, `?`, `#`, NUL, CR/LF в id-строках),
/// а не текущая реализация `render_path`.
#[test]
fn send_message_rejects_path_traversal_in_channel_id_before_sender() {
    for id in [
        "../../users/@me",
        "..%2f..%2fusers/@me",
        "/etc/passwd",
        "10/../../users/@me",
        "10?x=1",
        "10\\..\\x",
        "10\x00",
        "10\n",
        "",
        "   ",
    ] {
        let sender = fake(Ok(receipt()));
        assert_eq!(
            run(send_message(&sender, id, "hi", "n1")),
            Err(MessageError::InvalidArg),
            "id={id:?}"
        );
        assert!(
            sender.calls.lock().unwrap().is_empty(),
            "id={id:?}: I/O произошёл до валидации"
        );
    }
}

/// Регресс UI-контракта: фикстурные id (`news`, `c1`) и снежинки проходят,
/// allowlist ограничен 64 байтами (аудит §4.2 T2).
#[test]
fn send_message_accepts_safe_channel_ids_and_enforces_64_byte_bound() {
    for id in ["news", "c1", "10", "1293192721237938187", "a_b-C9"] {
        let sender = fake(Ok(receipt()));
        assert_eq!(
            run(send_message(&sender, id, "hi", "n1")),
            Ok(receipt()),
            "id={id:?}"
        );
        assert_eq!(sender.calls.lock().unwrap().len(), 1, "id={id:?}");
    }
    let at_limit = "a".repeat(64);
    let sender = fake(Ok(receipt()));
    assert_eq!(
        run(send_message(&sender, &at_limit, "hi", "n1")),
        Ok(receipt())
    );
    for id in ["a".repeat(65), "é".repeat(8)] {
        let sender = fake(Ok(receipt()));
        assert_eq!(
            run(send_message(&sender, &id, "hi", "n1")),
            Err(MessageError::InvalidArg),
            "id={id:?}"
        );
        assert!(sender.calls.lock().unwrap().is_empty(), "id={id:?}");
    }
}

/// Defense-in-depth: прямой вызов трейта (в обход free-функции) тоже отвергает
/// path-significant id до HTTP. Таймаут держит тест офлайн: если валидация
/// исчезнет, future отменяется через 500 мс, а не уходит в сеть.
#[test]
fn sender_rejects_path_traversal_channel_id_before_http() {
    let sender = DiscordMessageSender::new("secret-token").expect("sender config");
    for id in ["../../users/@me", "10?x=1", "", "10\x00"] {
        let outcome = run(async {
            tokio::time::timeout(
                std::time::Duration::from_millis(500),
                sender.send_message(id, "hi", "n1"),
            )
            .await
        });
        assert_eq!(outcome, Ok(Err(MessageError::InvalidArg)), "id={id:?}");
    }
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

// ---------------------------------------------------------------------------
// task-180 (BE-P2-12): bulk ack + optional flags/manual/mention_count.
// Оракул — `/home/sanya/re-deep/rest-static.json` (endpoints, read-only):
//   `BULK_ACK`    : POST `/read-states/ack-bulk`, bodyKeys `["{read_states}"]`;
//   `MESSAGE_ACK` : POST `/channels/{arg}/messages/{arg}/ack`,
//                   bodyKeys `["{manual,mention_count}", "{token,last_viewed,flags}"]`.
// Живой клиент (`web.4d4b2f75fdea5470.js`): bulk — `body:{read_states:t}` после
// `ep.splice(0,100)` (пачка максимум 100), элемент `{channel_id,message_id,
// read_state_type}`; auto-ack — `body:{token,last_viewed,flags}` (`flags`
// отсутствует, когда не изменился); mark-unread — `body:{manual,mention_count}`.
// ---------------------------------------------------------------------------

type AckHttpCall = (String, Vec<u8>);

struct FakeAckTransport {
    responses: Mutex<Vec<Result<HttpResponse, MessageError>>>,
    calls: Arc<Mutex<Vec<AckHttpCall>>>,
}

impl AckTransport for FakeAckTransport {
    fn post_ack<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, MessageError>> + Send + 'a>> {
        self.calls
            .lock()
            .unwrap()
            .push((target.to_string(), body.to_vec()));
        let next = self.responses.lock().unwrap().remove(0);
        Box::pin(async move { next })
    }
}

fn http_response(status: u16) -> HttpResponse {
    HttpResponse {
        status,
        headers: Vec::new(),
        body: Vec::new(),
    }
}

/// API поверх фейкового транспорта: каждый вызов съедает следующий ответ.
fn read_state_with(
    responses: Vec<Result<HttpResponse, MessageError>>,
) -> (DiscordReadState, Arc<Mutex<Vec<AckHttpCall>>>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let fake = FakeAckTransport {
        responses: Mutex::new(responses),
        calls: Arc::clone(&calls),
    };
    (DiscordReadState::with_transport(Arc::new(fake)), calls)
}

fn body_of(body: &[u8]) -> serde_json::Value {
    serde_json::from_slice(body).expect("тело запроса — JSON")
}

fn sorted_keys(value: &serde_json::Value) -> Vec<String> {
    let mut keys: Vec<String> = value
        .as_object()
        .expect("тело — JSON-объект")
        .keys()
        .cloned()
        .collect();
    keys.sort();
    keys
}

fn bulk_entry(channel: &str, message: &str, read_state_type: Option<u8>) -> AckBulkEntry {
    AckBulkEntry {
        channel_id: channel.to_string(),
        message_id: message.to_string(),
        read_state_type,
    }
}

const BULK_CHANNEL: &str = "1293192721237938187";
const BULK_MESSAGE: &str = "1561348877573947515";

#[test]
fn ack_bulk_constants_match_the_rest_static_oracle() {
    assert_eq!(ACK_BULK_PATH, "/read-states/ack-bulk");
    assert_eq!(MAX_ACK_BULK_ENTRIES, 100);
}

#[test]
fn ack_message_with_options_default_sends_exactly_token_and_last_viewed() {
    let (api, calls) = read_state_with(vec![Ok(http_response(200))]);
    assert_eq!(
        run(api.ack_message_with_options(BULK_CHANNEL, BULK_MESSAGE, AckMessageOptions::default())),
        Ok(())
    );

    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 1, "ровно один POST ack");
    assert_eq!(
        calls[0].0,
        format!("/channels/{BULK_CHANNEL}/messages/{BULK_MESSAGE}/ack"),
        "target ack — маршрут каталога"
    );
    let body = body_of(&calls[0].1);
    assert_eq!(
        body,
        serde_json::json!({"token": null, "last_viewed": null}),
        "базовые ключи оракула: token (nullable) и last_viewed"
    );
    assert_eq!(
        sorted_keys(&body),
        vec!["last_viewed".to_string(), "token".to_string()]
    );
}

#[test]
fn ack_message_with_options_sends_optionals_only_when_set_and_keeps_zero() {
    let (api, calls) = read_state_with(vec![Ok(http_response(200)), Ok(http_response(200))]);

    assert_eq!(
        run(api.ack_message_with_options(
            BULK_CHANNEL,
            BULK_MESSAGE,
            AckMessageOptions {
                token: Some("read-state-token".to_string()),
                last_viewed: Some(4310),
                flags: Some(2),
                manual: Some(true),
                mention_count: Some(3),
            }
        )),
        Ok(())
    );
    // `mention_count: Some(0)` обязан остаться в теле: 0 — валидное значение,
    // а не «нет данных» (оракул mark-unread считает упоминания).
    assert_eq!(
        run(api.ack_message_with_options(
            BULK_CHANNEL,
            BULK_MESSAGE,
            AckMessageOptions {
                token: None,
                last_viewed: Some(0),
                flags: None,
                manual: Some(true),
                mention_count: Some(0),
            }
        )),
        Ok(())
    );

    let calls = calls.lock().unwrap();
    let full = body_of(&calls[0].1);
    assert_eq!(
        full,
        serde_json::json!({
            "token": "read-state-token",
            "last_viewed": 4310,
            "flags": 2,
            "manual": true,
            "mention_count": 3,
        })
    );
    assert_eq!(
        sorted_keys(&full),
        vec![
            "flags".to_string(),
            "last_viewed".to_string(),
            "manual".to_string(),
            "mention_count".to_string(),
            "token".to_string(),
        ]
    );

    let partial = body_of(&calls[1].1);
    assert_eq!(
        partial,
        serde_json::json!({
            "token": null,
            "last_viewed": 0,
            "manual": true,
            "mention_count": 0,
        }),
        "незаданный flags не появляется, mention_count=0 сохраняется"
    );
    assert!(
        !partial.as_object().unwrap().contains_key("flags"),
        "flags пропускается, когда не задан: {partial}"
    );
    assert!(partial["last_viewed"].is_u64(), "{partial}");
    assert!(partial["mention_count"].is_u64(), "{partial}");
}

#[test]
fn ack_message_with_options_rejects_invalid_ids_before_transport() {
    let (api, calls) = read_state_with(Vec::new());
    for (channel, message) in [
        ("", BULK_MESSAGE),
        ("abc", BULK_MESSAGE),
        ("10/../users/@me", BULK_MESSAGE),
        ("10?x=1", BULK_MESSAGE),
        ("10\n", BULK_MESSAGE),
        (BULK_CHANNEL, ""),
        (BULK_CHANNEL, "abc"),
        (BULK_CHANNEL, "{id}"),
        (BULK_CHANNEL, "１２３"),
    ] {
        assert_eq!(
            run(api.ack_message_with_options(channel, message, AckMessageOptions::default())),
            Err(MessageError::InvalidArg),
            "channel={channel:?} message={message:?}"
        );
    }
    assert!(
        calls.lock().unwrap().is_empty(),
        "валидация обязана отсекать id до транспорта"
    );
}

#[test]
fn ack_message_with_options_maps_http_statuses_to_typed_errors() {
    let cases = [
        (401u16, MessageError::Unauthorized),
        (403, MessageError::Forbidden),
        (404, MessageError::NotFound),
        (429, MessageError::RateLimited { retry_after_ms: 0 }),
        (400, MessageError::Client { status: 400 }),
        (408, MessageError::Client { status: 408 }),
        (500, MessageError::Server { status: 500 }),
        (503, MessageError::Server { status: 503 }),
    ];
    let mut responses = vec![Ok(http_response(200)), Ok(http_response(204))];
    responses.extend(cases.iter().map(|(status, _)| Ok(http_response(*status))));
    let (api, calls) = read_state_with(responses);

    for expected_status in [200u16, 204] {
        assert_eq!(
            run(api.ack_message_with_options(
                BULK_CHANNEL,
                BULK_MESSAGE,
                AckMessageOptions::default()
            )),
            Ok(()),
            "HTTP {expected_status} — успех ack"
        );
    }
    for (status, expected) in cases {
        assert_eq!(
            run(api.ack_message_with_options(
                BULK_CHANNEL,
                BULK_MESSAGE,
                AckMessageOptions::default()
            )),
            Err(expected),
            "HTTP {status}"
        );
    }
    assert_eq!(calls.lock().unwrap().len(), 10);
}

#[test]
fn ack_messages_bulk_posts_oracle_body_to_bulk_path() {
    let (api, calls) = read_state_with(vec![Ok(http_response(200)), Ok(http_response(204))]);
    let entries = vec![
        bulk_entry(BULK_CHANNEL, BULK_MESSAGE, None),
        bulk_entry("42", "43", Some(2)),
    ];

    assert_eq!(run(api.ack_messages_bulk(&entries)), Ok(()));
    assert_eq!(
        run(api.ack_messages_bulk(&entries[..1])),
        Ok(()),
        "204 — тоже подтверждённый успех bulk ack"
    );

    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].0, ACK_BULK_PATH);
    assert_eq!(calls[1].0, "/read-states/ack-bulk");

    let body = body_of(&calls[0].1);
    assert_eq!(sorted_keys(&body), vec!["read_states".to_string()]);
    assert_eq!(
        body,
        serde_json::json!({
            "read_states": [
                {"channel_id": BULK_CHANNEL, "message_id": BULK_MESSAGE},
                {"channel_id": "42", "message_id": "43", "read_state_type": 2},
            ]
        }),
        "тело bulk — ровно {{read_states:[...]}} с omit-none read_state_type"
    );
    for item in body["read_states"].as_array().unwrap() {
        assert!(
            !item.as_object().unwrap().contains_key("read_state_type")
                || !item["read_state_type"].is_null(),
            "read_state_type не сериализуется как null: {item}"
        );
    }

    let single = body_of(&calls[1].1);
    assert_eq!(
        single,
        serde_json::json!({
            "read_states": [
                {"channel_id": BULK_CHANNEL, "message_id": BULK_MESSAGE},
            ]
        })
    );
}

#[test]
fn ack_messages_bulk_rejects_empty_and_over_limit_before_transport() {
    let (api, calls) = read_state_with(Vec::new());
    assert_eq!(
        run(api.ack_messages_bulk(&[])),
        Err(MessageError::InvalidArg),
        "пустой список — не запрос"
    );

    let too_many: Vec<AckBulkEntry> = (0..=MAX_ACK_BULK_ENTRIES)
        .map(|index| bulk_entry(&(index + 1).to_string(), &(index + 2).to_string(), None))
        .collect();
    assert_eq!(too_many.len(), 101);
    assert_eq!(
        run(api.ack_messages_bulk(&too_many)),
        Err(MessageError::InvalidArg),
        "101 элемент — больше лимита оракула (пачка 100)"
    );
    assert!(
        calls.lock().unwrap().is_empty(),
        "оба отказа — до транспорта"
    );

    // Граница включительно: ровно 100 элементов уходят одним POST.
    let exactly: Vec<AckBulkEntry> = (0..MAX_ACK_BULK_ENTRIES)
        .map(|index| bulk_entry(&(index + 1).to_string(), &(index + 2).to_string(), None))
        .collect();
    let (api, calls) = read_state_with(vec![Ok(http_response(200))]);
    assert_eq!(run(api.ack_messages_bulk(&exactly)), Ok(()));
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        body_of(&calls[0].1)["read_states"]
            .as_array()
            .unwrap()
            .len(),
        100
    );
}

#[test]
fn ack_messages_bulk_rejects_any_invalid_entry_id_before_transport() {
    let (api, calls) = read_state_with(Vec::new());
    let invalid_sets = [
        vec![bulk_entry("", BULK_MESSAGE, None)],
        vec![bulk_entry("news", BULK_MESSAGE, None)],
        vec![bulk_entry(BULK_CHANNEL, "", None)],
        vec![bulk_entry(BULK_CHANNEL, "abc", Some(0))],
        vec![bulk_entry(BULK_CHANNEL, "10/../x", None)],
        vec![
            bulk_entry(BULK_CHANNEL, BULK_MESSAGE, None),
            bulk_entry("42", "10?x=1", None),
        ],
        vec![
            bulk_entry(BULK_CHANNEL, BULK_MESSAGE, None),
            bulk_entry("42\n", "43", Some(1)),
        ],
    ];
    for entries in invalid_sets {
        assert_eq!(
            run(api.ack_messages_bulk(&entries)),
            Err(MessageError::InvalidArg),
            "entries={entries:?}"
        );
    }
    assert!(
        calls.lock().unwrap().is_empty(),
        "невалидная пачка не доходит до транспорта"
    );
}

#[test]
fn ack_messages_bulk_maps_http_statuses_to_typed_errors() {
    let cases = [
        (401u16, MessageError::Unauthorized),
        (403, MessageError::Forbidden),
        (404, MessageError::NotFound),
        (429, MessageError::RateLimited { retry_after_ms: 0 }),
        (400, MessageError::Client { status: 400 }),
        (413, MessageError::Client { status: 413 }),
        (500, MessageError::Server { status: 500 }),
        (503, MessageError::Server { status: 503 }),
    ];
    let responses = cases
        .iter()
        .map(|(status, _)| Ok(http_response(*status)))
        .collect();
    let (api, calls) = read_state_with(responses);
    let entries = vec![bulk_entry(BULK_CHANNEL, BULK_MESSAGE, None)];

    for (status, expected) in cases {
        assert_eq!(
            run(api.ack_messages_bulk(&entries)),
            Err(expected),
            "HTTP {status}"
        );
    }
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 8);
    assert!(calls.iter().all(|(target, _)| target == ACK_BULK_PATH));
}

#[test]
fn ack_bulk_entry_deserializes_with_and_without_read_state_type() {
    let without: AckBulkEntry =
        serde_json::from_str(r#"{"channel_id":"1","message_id":"2"}"#).expect("без типа");
    assert_eq!(without, bulk_entry("1", "2", None));
    let with: AckBulkEntry =
        serde_json::from_str(r#"{"channel_id":"1","message_id":"2","read_state_type":2}"#)
            .expect("с типом");
    assert_eq!(with, bulk_entry("1", "2", Some(2)));
    assert_eq!(
        serde_json::to_value(&without).unwrap(),
        serde_json::json!({"channel_id": "1", "message_id": "2"})
    );
}

/// TASK-808: Устранение задержек messages_page (<15 мс) и ack_message (неблокирующий ACK).
#[test]
fn be_p2_task_808_nonblocking_ack_dispatch_and_optimistic_cache() {
    use std::time::Instant;

    // 1. Неблокирующий ACK-диспетчер
    let dispatcher = NonBlockingAckDispatcher::new();
    assert_eq!(dispatcher.pending_count(), 0);

    let start = Instant::now();
    let res = dispatcher.enqueue_ack(
        "123456789012345678",
        "987654321098765432",
        1700000000,
        Some("token_ack".to_string()),
    );
    let elapsed = start.elapsed();

    assert!(res.is_ok());
    assert!(
        elapsed.as_millis() < 15,
        "ACK enqueue обязан отрабатывать быстрее 15 мс (факт: {} мс)",
        elapsed.as_millis()
    );
    assert_eq!(dispatcher.pending_count(), 1);

    let drained = dispatcher.drain_all();
    assert_eq!(drained.len(), 1);
    assert_eq!(drained[0].channel_id, "123456789012345678");
    assert_eq!(drained[0].message_id, "987654321098765432");
    assert_eq!(dispatcher.pending_count(), 0);

    // 2. Оптимистичный кэш страниц сообщений
    let cache = OptimisticMessagesCache::new();
    assert!(cache.get_cached_messages("c_opt_1").is_none());

    let msgs = vec![
        serde_json::json!({"id": "m1", "content": "hello world"}),
        serde_json::json!({"id": "m2", "content": "second msg"}),
    ];
    cache.store_messages("c_opt_1", msgs.clone());

    let start_cache = Instant::now();
    let cached = cache.get_cached_messages("c_opt_1");
    let cache_elapsed = start_cache.elapsed();

    assert!(cached.is_some());
    assert_eq!(cached.unwrap().len(), 2);
    assert!(
        cache_elapsed.as_millis() < 15,
        "Чтение из кэша сообщений обязано быть < 15 мс (факт: {} мс)",
        cache_elapsed.as_millis()
    );
}
