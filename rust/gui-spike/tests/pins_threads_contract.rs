//! RW-40 / M-10 & M-11: контракт закреплённых сообщений (`pins`) и веток канала (`threads`).
//!
//! Оракул и спецификация:
//! - `waves/RW-40/PARITY/PARITY-MATRIX.md` (строки M-10 и M-11);
//! - `waves/RW-40/DESIGN/PINS-THREADS-PROFILE-SPEC.md` (§2, §3);
//! - `rust/gui-spike/src/messages.rs` (независимый оракул allowlist `[A-Za-z0-9_-]`, `1..=64` байта).
//!
//! Контракт:
//! - `list_channel_pins(&transport, channel_id)` -> `GET /channels/{channel_id}/pins` -> `Vec<PinnedMessage>`
//! - `pin_channel_message(&transport, channel_id, message_id)` -> `PUT /channels/{channel_id}/pins/{message_id}` (204 No Content / 200 OK)
//! - `unpin_channel_message(&transport, channel_id, message_id)` -> `DELETE /channels/{channel_id}/pins/{message_id}` (204 No Content / 200 OK)
//! - `list_channel_threads(&transport, channel_id, archived)` ->
//!   `GET /channels/{channel_id}/threads/active` (при `archived == false`) или
//!   `GET /channels/{channel_id}/threads/archived/public` (при `archived == true`) -> `Vec<ThreadSummary>`
//! - предсетевая валидация `channel_id` и `message_id` по allowlist (`[A-Za-z0-9_-]`, `1..=64` байта,
//!   в паритете с `messages::send_message`) — отказ `InvalidChannelId` / `InvalidMessageId` до любого вызова транспорта!
//! - маппинг HTTP `429` с извлечением `retry-after` в миллисекунды (`RateLimited { retry_after_ms }`),
//!   `401 Unauthorized`, `403 Forbidden`, `404 NotFound`, `4xx Client`, `5xx Server`.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use discord_core::rest::transport::HttpResponse;
use chord_client::messages::{send_message, MessageError, MessageReceipt, MessageSender};
use chord_client::pins_threads::{
    list_channel_pins, list_channel_threads, pin_channel_message, unpin_channel_message,
    DiscordPinsThreads, PinnedMessage, PinsThreadsError, PinsThreadsTransport, ThreadSummary,
    MAX_ID_BYTES,
};

const CHANNEL_ID: &str = "1014537851418980375";
const MESSAGE_ID: &str = "1293192721237938187";

#[derive(Debug, Clone, PartialEq, Eq)]
enum CallRecord {
    Get(String),
    Put(String, Vec<u8>),
    Delete(String),
}

#[derive(Clone, Default)]
struct FakeTransport {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    error: Option<PinsThreadsError>,
    calls: Arc<Mutex<Vec<CallRecord>>>,
}

impl FakeTransport {
    fn answering(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
            error: None,
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn with_body(status: u16, body: &[u8]) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.to_vec(),
            error: None,
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn with_headers(mut self, headers: &[(&str, &str)]) -> Self {
        self.headers = headers
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect();
        self
    }

    fn failing(error: PinsThreadsError) -> Self {
        Self {
            status: 0,
            headers: Vec::new(),
            body: Vec::new(),
            error: Some(error),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn calls(&self) -> Vec<CallRecord> {
        self.calls.lock().unwrap().clone()
    }

    fn response(&self) -> Result<HttpResponse, PinsThreadsError> {
        match self.error.clone() {
            Some(error) => Err(error),
            None => Ok(HttpResponse {
                status: self.status,
                headers: self.headers.clone(),
                body: self.body.clone(),
            }),
        }
    }
}

impl PinsThreadsTransport for FakeTransport {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, PinsThreadsError>> + Send + 'a>> {
        self.calls
            .lock()
            .unwrap()
            .push(CallRecord::Get(target.to_string()));
        let answer = self.response();
        Box::pin(async move { answer })
    }

    fn put<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, PinsThreadsError>> + Send + 'a>> {
        self.calls
            .lock()
            .unwrap()
            .push(CallRecord::Put(target.to_string(), body.to_vec()));
        let answer = self.response();
        Box::pin(async move { answer })
    }

    fn delete<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, PinsThreadsError>> + Send + 'a>> {
        self.calls
            .lock()
            .unwrap()
            .push(CallRecord::Delete(target.to_string()));
        let answer = self.response();
        Box::pin(async move { answer })
    }
}

fn api(transport: &FakeTransport) -> DiscordPinsThreads {
    DiscordPinsThreads::with_transport(Arc::new(transport.clone()))
}

fn run<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(future)
}

#[derive(Default)]
struct DummyMessageSender {
    calls: Mutex<Vec<String>>,
}

impl MessageSender for DummyMessageSender {
    fn send_message<'a>(
        &'a self,
        channel_id: &'a str,
        content: &'a str,
        nonce: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<MessageReceipt, MessageError>> + Send + 'a>> {
        self.calls.lock().unwrap().push(channel_id.to_string());
        let receipt = MessageReceipt {
            id: "1".to_string(),
            channel_id: channel_id.to_string(),
            content: content.to_string(),
            nonce: nonce.to_string(),
        };
        Box::pin(async move { Ok(receipt) })
    }
}

const PINS_JSON: &str = r#"[
    {
        "id": "9001",
        "channel_id": "1014537851418980375",
        "author": {"id": "u1", "username": "alice", "global_name": "Alice Wonder"},
        "content": "Релиз RW-40 сегодня в 18:00",
        "timestamp": "2026-10-30T12:00:00.000Z",
        "pinned": true
    },
    {
        "id": "9002",
        "channel_id": "1014537851418980375",
        "author": "Bob",
        "content": "Правила канала",
        "timestamp": "2026-10-29T09:30:00.000Z",
        "pinned": true
    }
]"#;

const ACTIVE_THREADS_JSON: &str = r#"{
    "threads": [
        {
            "id": "th_active_1",
            "parent_id": "1014537851418980375",
            "name": "Обсуждение архитектуры Rust",
            "message_count": 42,
            "member_count": 7,
            "thread_metadata": {
                "archived": false,
                "locked": false
            },
            "last_message_id": "999001"
        },
        {
            "id": "th_active_2",
            "parent_id": "1014537851418980375",
            "name": "Сбор логов",
            "message_count": 5,
            "member_count": 2,
            "archived": false,
            "locked": true,
            "last_message_id": null
        }
    ],
    "has_more": false
}"#;

const ARCHIVED_THREADS_JSON: &str = r#"{
    "threads": [
        {
            "id": "th_arch_1",
            "parent_id": "1014537851418980375",
            "name": "Старый релиз RW-39",
            "message_count": 128,
            "member_count": 15,
            "thread_metadata": {
                "archived": true,
                "locked": true
            },
            "last_message_id": "888777"
        }
    ],
    "has_more": false
}"#;

#[test]
fn list_channel_pins_gets_pins_endpoint_and_parses_pinned_messages() {
    let transport = FakeTransport::with_body(200, PINS_JSON.as_bytes());
    let client = api(&transport);

    let pins = run(list_channel_pins(&transport, CHANNEL_ID)).expect("list_channel_pins free fn");
    assert_eq!(
        pins,
        vec![
            PinnedMessage {
                id: "9001".to_string(),
                channel_id: CHANNEL_ID.to_string(),
                author: "Alice Wonder".to_string(),
                content: "Релиз RW-40 сегодня в 18:00".to_string(),
                timestamp: "2026-10-30T12:00:00.000Z".to_string(),
                pinned: true,
            },
            PinnedMessage {
                id: "9002".to_string(),
                channel_id: CHANNEL_ID.to_string(),
                author: "Bob".to_string(),
                content: "Правила канала".to_string(),
                timestamp: "2026-10-29T09:30:00.000Z".to_string(),
                pinned: true,
            },
        ]
    );

    let via_method = run(client.list_channel_pins(CHANNEL_ID)).expect("list_channel_pins method");
    assert_eq!(via_method, pins);

    assert_eq!(
        transport.calls(),
        vec![
            CallRecord::Get(format!("/channels/{CHANNEL_ID}/pins")),
            CallRecord::Get(format!("/channels/{CHANNEL_ID}/pins")),
        ],
        "ровно GET /channels/{{channel_id}}/pins"
    );

    // Проверка пустого списка закреплённых сообщений `[]` (empty state §2.3):
    let empty_transport = FakeTransport::with_body(200, b"[]");
    let empty_pins =
        run(list_channel_pins(&empty_transport, "news")).expect("пустой массив пинов валиден");
    assert!(empty_pins.is_empty());
    assert_eq!(
        empty_transport.calls(),
        vec![CallRecord::Get("/channels/news/pins".to_string())]
    );
}

#[test]
fn pin_and_unpin_channel_message_use_put_and_delete_on_pins_endpoint_and_accept_204_or_200() {
    for status in [204_u16, 200_u16] {
        let put_transport = FakeTransport::answering(status);
        let put_client = api(&put_transport);

        assert_eq!(
            run(pin_channel_message(&put_transport, CHANNEL_ID, MESSAGE_ID)),
            Ok(()),
            "pin free fn HTTP {status}"
        );
        assert_eq!(
            run(put_client.pin_channel_message("news", "msg_42")),
            Ok(()),
            "pin method HTTP {status}"
        );
        assert_eq!(
            put_transport.calls(),
            vec![
                CallRecord::Put(
                    format!("/channels/{CHANNEL_ID}/pins/{MESSAGE_ID}"),
                    Vec::new()
                ),
                CallRecord::Put("/channels/news/pins/msg_42".to_string(), Vec::new()),
            ],
            "PUT /channels/{{channel_id}}/pins/{{message_id}}"
        );

        let del_transport = FakeTransport::answering(status);
        let del_client = api(&del_transport);

        assert_eq!(
            run(unpin_channel_message(
                &del_transport,
                CHANNEL_ID,
                MESSAGE_ID
            )),
            Ok(()),
            "unpin free fn HTTP {status}"
        );
        assert_eq!(
            run(del_client.unpin_channel_message("news", "msg_42")),
            Ok(()),
            "unpin method HTTP {status}"
        );
        assert_eq!(
            del_transport.calls(),
            vec![
                CallRecord::Delete(format!("/channels/{CHANNEL_ID}/pins/{MESSAGE_ID}")),
                CallRecord::Delete("/channels/news/pins/msg_42".to_string()),
            ],
            "DELETE /channels/{{channel_id}}/pins/{{message_id}}"
        );
    }
}

#[test]
fn list_channel_threads_routes_active_and_archived_public_and_parses_thread_summaries() {
    // 1. archived == false -> GET /channels/{channel_id}/threads/active
    let active_transport = FakeTransport::with_body(200, ACTIVE_THREADS_JSON.as_bytes());
    let active_client = api(&active_transport);

    let active = run(list_channel_threads(&active_transport, CHANNEL_ID, false))
        .expect("list_channel_threads active");
    assert_eq!(
        active,
        vec![
            ThreadSummary {
                id: "th_active_1".to_string(),
                parent_id: CHANNEL_ID.to_string(),
                name: "Обсуждение архитектуры Rust".to_string(),
                message_count: 42,
                member_count: 7,
                archived: false,
                locked: false,
                last_message_id: Some("999001".to_string()),
            },
            ThreadSummary {
                id: "th_active_2".to_string(),
                parent_id: CHANNEL_ID.to_string(),
                name: "Сбор логов".to_string(),
                message_count: 5,
                member_count: 2,
                archived: false,
                locked: true,
                last_message_id: None,
            },
        ]
    );
    assert_eq!(
        run(active_client.list_channel_threads(CHANNEL_ID, false)),
        Ok(active)
    );
    assert_eq!(
        active_transport.calls(),
        vec![
            CallRecord::Get(format!("/channels/{CHANNEL_ID}/threads/active")),
            CallRecord::Get(format!("/channels/{CHANNEL_ID}/threads/active")),
        ]
    );

    // 2. archived == true -> GET /channels/{channel_id}/threads/archived/public
    let archived_transport = FakeTransport::with_body(200, ARCHIVED_THREADS_JSON.as_bytes());
    let archived_client = api(&archived_transport);

    let archived = run(list_channel_threads(&archived_transport, CHANNEL_ID, true))
        .expect("list_channel_threads archived");
    assert_eq!(
        archived,
        vec![ThreadSummary {
            id: "th_arch_1".to_string(),
            parent_id: CHANNEL_ID.to_string(),
            name: "Старый релиз RW-39".to_string(),
            message_count: 128,
            member_count: 15,
            archived: true,
            locked: true,
            last_message_id: Some("888777".to_string()),
        }]
    );
    assert_eq!(
        run(archived_client.list_channel_threads(CHANNEL_ID, true)),
        Ok(archived)
    );
    assert_eq!(
        archived_transport.calls(),
        vec![
            CallRecord::Get(format!("/channels/{CHANNEL_ID}/threads/archived/public")),
            CallRecord::Get(format!("/channels/{CHANNEL_ID}/threads/archived/public")),
        ]
    );

    // Поддержка прямого JSON-массива веток `[...]` помимо обёртки `{"threads":[...]}`:
    let array_body = r#"[
        {
            "id": "th_arr_1",
            "parent_id": "c1",
            "name": "Прямой массив",
            "message_count": 3,
            "member_count": 1,
            "archived": false,
            "locked": false,
            "last_message_id": "100"
        }
    ]"#
    .as_bytes();
    let array_transport = FakeTransport::with_body(200, array_body);
    let parsed_array =
        run(list_channel_threads(&array_transport, "c1", false)).expect("массив веток");
    assert_eq!(parsed_array.len(), 1);
    assert_eq!(parsed_array[0].id, "th_arr_1");
}

#[test]
fn rejects_invalid_channel_id_and_message_id_before_any_transport_call_in_parity_with_messages() {
    assert_eq!(MAX_ID_BYTES, 64);

    let transport = FakeTransport::with_body(200, b"[]");
    let client = api(&transport);
    let over_64 = "a".repeat(65);

    let bad_ids = [
        "",
        "   ",
        "../..",
        "../../users/@me",
        "123/456",
        "123\\456",
        "123?x=1",
        "123#frag",
        "123 456",
        "123\n456",
        "123\r456",
        "123\x00",
        "%2e%2e",
        "a.b",
        "канал",
        over_64.as_str(),
    ];

    for bad_channel in bad_ids {
        assert_eq!(
            run(list_channel_pins(&transport, bad_channel)),
            Err(PinsThreadsError::InvalidChannelId),
            "list_channel_pins bad_channel={bad_channel:?}"
        );
        assert_eq!(
            run(pin_channel_message(&transport, bad_channel, MESSAGE_ID)),
            Err(PinsThreadsError::InvalidChannelId),
            "pin_channel_message bad_channel={bad_channel:?}"
        );
        assert_eq!(
            run(unpin_channel_message(&transport, bad_channel, MESSAGE_ID)),
            Err(PinsThreadsError::InvalidChannelId),
            "unpin_channel_message bad_channel={bad_channel:?}"
        );
        assert_eq!(
            run(list_channel_threads(&transport, bad_channel, false)),
            Err(PinsThreadsError::InvalidChannelId),
            "list_channel_threads(active) bad_channel={bad_channel:?}"
        );
        assert_eq!(
            run(list_channel_threads(&transport, bad_channel, true)),
            Err(PinsThreadsError::InvalidChannelId),
            "list_channel_threads(archived) bad_channel={bad_channel:?}"
        );
        assert_eq!(
            run(client.pin_channel_message(bad_channel, MESSAGE_ID)),
            Err(PinsThreadsError::InvalidChannelId),
            "client.pin_channel_message bad_channel={bad_channel:?}"
        );
    }

    for bad_message in bad_ids {
        assert_eq!(
            run(pin_channel_message(&transport, CHANNEL_ID, bad_message)),
            Err(PinsThreadsError::InvalidMessageId),
            "pin_channel_message bad_message={bad_message:?}"
        );
        assert_eq!(
            run(unpin_channel_message(&transport, CHANNEL_ID, bad_message)),
            Err(PinsThreadsError::InvalidMessageId),
            "unpin_channel_message bad_message={bad_message:?}"
        );
        assert_eq!(
            run(client.unpin_channel_message(CHANNEL_ID, bad_message)),
            Err(PinsThreadsError::InvalidMessageId),
            "client.unpin_channel_message bad_message={bad_message:?}"
        );
    }

    assert!(
        transport.calls().is_empty(),
        "предсетевая валидация обязана отсекать невалидные id до любого вызова транспорта"
    );

    // Независимая сверка паритета allowlist с `messages::send_message`:
    let sender = DummyMessageSender::default();
    let exact_64 = "a".repeat(64);
    let samples = [
        CHANNEL_ID,
        MESSAGE_ID,
        "news",
        "c1",
        "chan_1-A",
        exact_64.as_str(),
        "",
        "../..",
        "a.b",
        "a/b",
        "a b",
        "10?x=1",
        over_64.as_str(),
    ];

    for sample in samples {
        let ok_transport = FakeTransport::answering(204);
        let pin_err = run(pin_channel_message(&ok_transport, sample, MESSAGE_ID)).is_err();
        let msg_id_err = run(pin_channel_message(&ok_transport, CHANNEL_ID, sample)).is_err();
        let send_err = run(send_message(&sender, sample, "hi", "n1")).is_err();
        assert_eq!(
            pin_err, send_err,
            "allowlist channel_id должен совпадать с messages::send_message для {sample:?}"
        );
        assert_eq!(
            msg_id_err, send_err,
            "allowlist message_id должен совпадать с messages::send_message для {sample:?}"
        );
    }
}

#[test]
fn maps_429_retry_after_ms_and_401_403_404_errors_across_all_operations() {
    // 1. 429 с заголовком Retry-After (целые и дробные секунды -> мс)
    for (header_name, header_val, expected_ms) in [
        ("retry-after", "3", 3000_u64),
        ("Retry-After", "1.5", 1500_u64),
        ("RETRY-AFTER", "0.25", 250_u64),
    ] {
        let transport = FakeTransport::answering(429).with_headers(&[(header_name, header_val)]);
        let expected_err = PinsThreadsError::RateLimited {
            retry_after_ms: expected_ms,
        };
        assert_eq!(
            run(list_channel_pins(&transport, CHANNEL_ID)),
            Err(expected_err.clone()),
            "list_channel_pins 429 {header_name}={header_val}"
        );
        assert_eq!(
            run(pin_channel_message(&transport, CHANNEL_ID, MESSAGE_ID)),
            Err(expected_err.clone()),
            "pin_channel_message 429 {header_name}={header_val}"
        );
        assert_eq!(
            run(unpin_channel_message(&transport, CHANNEL_ID, MESSAGE_ID)),
            Err(expected_err.clone()),
            "unpin_channel_message 429 {header_name}={header_val}"
        );
        assert_eq!(
            run(list_channel_threads(&transport, CHANNEL_ID, false)),
            Err(expected_err),
            "list_channel_threads 429 {header_name}={header_val}"
        );
    }

    // 2. 429 с JSON-телом {"retry_after": ...} при отсутствии заголовка
    let body_429 = FakeTransport::with_body(
        429,
        br#"{"message":"You are being rate limited.","retry_after":2.25}"#,
    );
    assert_eq!(
        run(pin_channel_message(&body_429, CHANNEL_ID, MESSAGE_ID)),
        Err(PinsThreadsError::RateLimited {
            retry_after_ms: 2250
        })
    );
    assert_eq!(
        run(list_channel_pins(&body_429, CHANNEL_ID)),
        Err(PinsThreadsError::RateLimited {
            retry_after_ms: 2250
        })
    );

    // 3. 429 без заголовка и тела -> retry_after_ms: 0
    let bare_429 = FakeTransport::answering(429);
    assert_eq!(
        run(unpin_channel_message(&bare_429, CHANNEL_ID, MESSAGE_ID)),
        Err(PinsThreadsError::RateLimited { retry_after_ms: 0 })
    );

    // 4. 401 Unauthorized, 403 Forbidden, 404 NotFound, 400 Client, 500/503 Server
    for (status, expected_err) in [
        (401, PinsThreadsError::Unauthorized),
        (403, PinsThreadsError::Forbidden),
        (404, PinsThreadsError::NotFound),
        (400, PinsThreadsError::Client { status: 400 }),
        (500, PinsThreadsError::Server { status: 500 }),
        (503, PinsThreadsError::Server { status: 503 }),
    ] {
        let transport = FakeTransport::answering(status);
        assert_eq!(
            run(list_channel_pins(&transport, CHANNEL_ID)),
            Err(expected_err.clone()),
            "list_channel_pins HTTP {status}"
        );
        assert_eq!(
            run(pin_channel_message(&transport, CHANNEL_ID, MESSAGE_ID)),
            Err(expected_err.clone()),
            "pin_channel_message HTTP {status}"
        );
        assert_eq!(
            run(unpin_channel_message(&transport, CHANNEL_ID, MESSAGE_ID)),
            Err(expected_err.clone()),
            "unpin_channel_message HTTP {status}"
        );
        assert_eq!(
            run(list_channel_threads(&transport, CHANNEL_ID, false)),
            Err(expected_err.clone()),
            "list_channel_threads(active) HTTP {status}"
        );
        assert_eq!(
            run(list_channel_threads(&transport, CHANNEL_ID, true)),
            Err(expected_err),
            "list_channel_threads(archived) HTTP {status}"
        );
    }

    // 5. Ошибка транспорта (сеть недоступна)
    let offline = FakeTransport::failing(PinsThreadsError::Network);
    assert_eq!(
        run(list_channel_pins(&offline, CHANNEL_ID)),
        Err(PinsThreadsError::Network)
    );
}

#[test]
fn rejects_malformed_200_payloads_and_protects_token_in_debug() {
    for bad_pins in [
        b"".as_slice(),
        b"not-json".as_slice(),
        br#"{}"#.as_slice(),
        br#"[{"id":"","channel_id":"c1","author":"a","content":"x","timestamp":"t","pinned":true}]"#
            .as_slice(),
        br#"[{"id":"1","channel_id":"","author":"a","content":"x","timestamp":"t","pinned":true}]"#
            .as_slice(),
    ] {
        let transport = FakeTransport::with_body(200, bad_pins);
        assert_eq!(
            run(list_channel_pins(&transport, CHANNEL_ID)),
            Err(PinsThreadsError::InvalidResponse),
            "bad_pins={bad_pins:?}"
        );
    }

    for bad_threads in [
        b"".as_slice(),
        b"not-json".as_slice(),
        br#"{"threads":{}}"#.as_slice(),
        br#"[{"id":"","parent_id":"c1","name":"n","message_count":1,"member_count":1,"archived":false,"locked":false}]"#
            .as_slice(),
        br#"[{"id":"t1","parent_id":"c1","name":"   ","message_count":1,"member_count":1,"archived":false,"locked":false}]"#
            .as_slice(),
    ] {
        let transport = FakeTransport::with_body(200, bad_threads);
        assert_eq!(
            run(list_channel_threads(&transport, CHANNEL_ID, false)),
            Err(PinsThreadsError::InvalidResponse),
            "bad_threads={bad_threads:?}"
        );
    }

    // Пустой токен отвергается, а Debug клиента не раскрывает токен:
    assert_eq!(
        DiscordPinsThreads::new("   ").expect_err("пустой токен"),
        PinsThreadsError::MissingToken
    );
    let client = DiscordPinsThreads::new("secret-token-value").expect("клиент");
    assert!(!format!("{client:?}").contains("secret-token-value"));
}
