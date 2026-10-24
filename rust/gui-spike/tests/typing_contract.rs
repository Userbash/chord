//! P-03 / D-108: контракт исходящего typing — `POST /channels/{id}/typing`.
//!
//! **Оракул — отдельный `[S]`** (не frozen D-02 каталог, где маршрута нет):
//! - `waves/RW-40/PARITY/G6-REPORT-RW40-02.md` §3.6 — тело отсутствует, успех `200`,
//!   ответ `{message_send_cooldown_ms?, thread_create_cooldown_ms?}`;
//! - `waves/RW-40/NETWORK-CONTRACTS.md` §1.6 — те же поля; TTL/троттлинг — клиентские,
//!   backend их не выдумывает.
//!
//! Тесты не ходят в сеть: HTTP-слой подменён [`TypingTransport`], который фиксирует
//! target и тело. `routes::ROUTES` остаётся frozen D-02 (47 live-строк) + POST сообщения:
//! typing подключён как явное `[S]`-расширение каталога (D-108).

use discord_core::rest::routes::{self, HttpMethod};
use discord_core::rest::transport::HttpResponse;
use discord_gui_spike::typing::{
    send_typing, DiscordTyping, TypingApi, TypingCooldowns, TypingError, TypingTransport,
};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

const CHANNEL_ID: &str = "123456789012345678";

/// Дословный `[S]`-оракул: путь шаблона (без `/api/v9`).
const ORACLE_PATH: &str = "/channels/{id}/typing";

/// Дословные ключи тела ответа из §1.6 (wire — snake_case, не имена Rust-полей).
const ORACLE_RESPONSE_KEYS: [&str; 2] = ["message_send_cooldown_ms", "thread_create_cooldown_ms"];

/// Выдержка frozen D-02: 47 live-маршрутов. `/typing` в ней отсутствует — это
/// доказывает, что контракт не подогнан под захват.
const D02_TSV: &str = include_str!("../../crates/discord-core/tests/data/rest-routes-47.tsv");

/// Журнал фейкового транспорта: (target, body).
type Calls = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

#[derive(Clone)]
struct FakeTransport {
    status: u16,
    body: Vec<u8>,
    error: Option<TypingError>,
    calls: Calls,
}

impl FakeTransport {
    fn answering(status: u16) -> Self {
        Self {
            status,
            body: Vec::new(),
            error: None,
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn with_body(status: u16, body: &[u8]) -> Self {
        Self {
            status,
            body: body.to_vec(),
            error: None,
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn failing(error: TypingError) -> Self {
        Self {
            status: 0,
            body: Vec::new(),
            error: Some(error),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn calls(&self) -> Vec<(String, Vec<u8>)> {
        self.calls.lock().unwrap().clone()
    }
}

impl TypingTransport for FakeTransport {
    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, TypingError>> + Send + 'a>> {
        self.calls
            .lock()
            .unwrap()
            .push((target.to_string(), body.to_vec()));
        let answer = match self.error.clone() {
            Some(error) => Err(error),
            None => Ok(HttpResponse {
                status: self.status,
                headers: Vec::new(),
                body: self.body.clone(),
            }),
        };
        Box::pin(async move { answer })
    }
}

fn api(transport: &FakeTransport) -> DiscordTyping {
    DiscordTyping::with_transport(Arc::new(transport.clone()))
}

fn run<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(future)
}

/// Фейковая реализация доменного контракта: проверяет делегирование и то, что
/// валидация происходит до вызова API.
struct FakeTypingApi {
    result: Result<Option<TypingCooldowns>, TypingError>,
    calls: Mutex<Vec<String>>,
}

impl FakeTypingApi {
    fn new(result: Result<Option<TypingCooldowns>, TypingError>) -> Self {
        Self {
            result,
            calls: Mutex::new(Vec::new()),
        }
    }
}

impl TypingApi for FakeTypingApi {
    fn send_typing<'a>(
        &'a self,
        channel_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<TypingCooldowns>, TypingError>> + Send + 'a>>
    {
        self.calls.lock().unwrap().push(channel_id.to_string());
        let result = self.result.clone();
        Box::pin(async move { result })
    }
}

#[test]
fn posts_empty_body_to_channel_typing_target() {
    let transport = FakeTransport::answering(200);
    let client = api(&transport);

    assert_eq!(run(client.send_typing(CHANNEL_ID)), Ok(None));

    let calls = transport.calls();
    assert_eq!(calls.len(), 1, "ровно один POST");
    let (target, body) = &calls[0];
    assert_eq!(target, &format!("/channels/{CHANNEL_ID}/typing"));
    assert!(body.is_empty(), "тело отсутствует: {body:?}");
    assert!(
        !target.contains("/messages"),
        "typing — не отправка сообщения: {target}"
    );
    assert!(!target.contains('?'), "query не подтверждён: {target}");
}

#[test]
fn free_function_validates_and_delegates() {
    let api = FakeTypingApi::new(Ok(Some(TypingCooldowns {
        message_send_cooldown_ms: Some(1200),
        thread_create_cooldown_ms: None,
    })));
    assert_eq!(
        run(send_typing(&api, CHANNEL_ID)),
        Ok(Some(TypingCooldowns {
            message_send_cooldown_ms: Some(1200),
            thread_create_cooldown_ms: None,
        }))
    );
    assert_eq!(*api.calls.lock().unwrap(), vec![CHANNEL_ID.to_string()]);

    let failing = FakeTypingApi::new(Err(TypingError::RateLimited {
        retry_after_ms: 250,
    }));
    assert_eq!(
        run(send_typing(&failing, CHANNEL_ID)),
        Err(TypingError::RateLimited {
            retry_after_ms: 250
        })
    );
}

#[test]
fn empty_two_hundred_body_means_no_cooldowns() {
    for body in [b"".as_slice(), b" ".as_slice(), b"\r\n\t".as_slice()] {
        let transport = FakeTransport::with_body(200, body);
        let client = api(&transport);
        assert_eq!(run(client.send_typing(CHANNEL_ID)), Ok(None), "{body:?}");
    }
}

#[test]
fn two_hundred_json_cooldowns_are_mapped_and_camel_cased_for_ipc() {
    let transport = FakeTransport::with_body(
        200,
        br#"{"message_send_cooldown_ms":1500,"thread_create_cooldown_ms":0}"#,
    );
    let full = api(&transport);
    let cooldowns = run(full.send_typing(CHANNEL_ID))
        .expect("200")
        .expect("кулдауны есть");
    assert_eq!(
        cooldowns,
        TypingCooldowns {
            message_send_cooldown_ms: Some(1500),
            thread_create_cooldown_ms: Some(0),
        }
    );
    assert_eq!(
        serde_json::to_value(&cooldowns).expect("ipc"),
        serde_json::json!({
            "messageSendCooldownMs": 1500,
            "threadCreateCooldownMs": 0
        }),
        "IPC-контракт G6 §3.6 — camelCase"
    );

    // Оба поля опциональны: частичный объект и пустой объект — не ошибка.
    let cases: [(&[u8], TypingCooldowns); 4] = [
        (
            br#"{"message_send_cooldown_ms":2500}"#,
            TypingCooldowns {
                message_send_cooldown_ms: Some(2500),
                thread_create_cooldown_ms: None,
            },
        ),
        (
            br#"{"thread_create_cooldown_ms":7500}"#,
            TypingCooldowns {
                message_send_cooldown_ms: None,
                thread_create_cooldown_ms: Some(7500),
            },
        ),
        (br#"{}"#, TypingCooldowns::default()),
        (br#"{"unexpected":true}"#, TypingCooldowns::default()),
    ];
    for (body, expected) in cases {
        let transport = FakeTransport::with_body(200, body);
        let client = api(&transport);
        assert_eq!(
            run(client.send_typing(CHANNEL_ID)),
            Ok(Some(expected)),
            "{body:?}"
        );
    }
}

#[test]
fn http_statuses_map_to_typed_errors() {
    let cases = [
        (401, TypingError::Unauthorized),
        (403, TypingError::Forbidden),
        (404, TypingError::NotFound),
        (429, TypingError::RateLimited { retry_after_ms: 0 }),
        (400, TypingError::Client { status: 400 }),
        (500, TypingError::Server { status: 500 }),
        (503, TypingError::Server { status: 503 }),
        // Контракт §1.6 подтверждает только 200; иной 2xx — нарушение ответа.
        (201, TypingError::InvalidResponse),
        (204, TypingError::InvalidResponse),
    ];
    for (status, expected) in cases {
        let transport = FakeTransport::answering(status);
        let client = api(&transport);
        assert_eq!(
            run(client.send_typing(CHANNEL_ID)),
            Err(expected),
            "HTTP {status}"
        );
    }
}

#[test]
fn malformed_two_hundred_body_is_invalid_response() {
    for body in [
        br#"[]"#.as_slice(),
        br#""oops""#.as_slice(),
        br#"null"#.as_slice(),
        br#"{"message_send_cooldown_ms":"soon"}"#.as_slice(),
        br#"{"message_send_cooldown_ms":"#.as_slice(),
    ] {
        let transport = FakeTransport::with_body(200, body);
        let client = api(&transport);
        assert_eq!(
            run(client.send_typing(CHANNEL_ID)),
            Err(TypingError::InvalidResponse),
            "{body:?}"
        );
    }
}

#[test]
fn transport_failure_is_typed_network_error() {
    let transport = FakeTransport::failing(TypingError::Network);
    let client = api(&transport);
    assert_eq!(
        run(client.send_typing(CHANNEL_ID)),
        Err(TypingError::Network)
    );
}

#[test]
fn invalid_channel_id_is_rejected_before_transport() {
    for channel_id in [
        "",
        "   ",
        "abc",
        "12 34",
        "123\r\n",
        "{channel}",
        "@me",
        "-123",
        "12.5",
        // 21 цифра: за пределы u64 (снежинка Discord — до 20 цифр).
        "123456789012345678901",
    ] {
        let transport = FakeTransport::answering(200);
        let client = api(&transport);
        assert_eq!(
            run(send_typing(&client, channel_id)),
            Err(TypingError::InvalidArg),
            "channel_id={channel_id:?}"
        );
        // Прямой вызов метода трейта валидирует так же: в сеть не уходим.
        assert_eq!(
            run(client.send_typing(channel_id)),
            Err(TypingError::InvalidArg),
            "trait channel_id={channel_id:?}"
        );
        assert!(
            transport.calls().is_empty(),
            "некорректный id не должен доходить до сети: {channel_id:?}"
        );
    }
}

#[test]
fn empty_or_blank_token_is_unauthorized() {
    for token in ["", "   ", "\t\n"] {
        assert_eq!(
            DiscordTyping::new(token).expect_err("пустой токен — ошибка"),
            TypingError::Unauthorized
        );
    }
}

#[test]
fn client_debug_does_not_reveal_token() {
    let client = DiscordTyping::new("secret-token").expect("клиент");
    let text = format!("{client:?}");
    assert!(!text.contains("secret-token"), "{text}");
}

#[test]
fn errors_serialize_with_camel_case_kind_only() {
    let cases = [
        (TypingError::InvalidArg, "invalidArg"),
        (TypingError::Unauthorized, "unauthorized"),
        (TypingError::Forbidden, "forbidden"),
        (TypingError::NotFound, "notFound"),
        (
            TypingError::RateLimited {
                retry_after_ms: 250,
            },
            "rateLimited",
        ),
        (TypingError::Client { status: 400 }, "client"),
        (TypingError::Server { status: 500 }, "server"),
        (TypingError::Network, "network"),
        (TypingError::Storage, "storage"),
        (TypingError::InvalidResponse, "invalidResponse"),
    ];
    for (error, kind) in cases {
        let value = serde_json::to_value(&error).expect("ошибка сериализуется");
        assert_eq!(value["kind"], kind, "{value}");
        assert!(!error.to_string().is_empty(), "Display непустой: {error}");
    }
}

/// Маршрут — отдельный `[S]`-контракт: в frozen D-02 его нет, но каталог
/// (`routes::find`) его резолвит, тело пустое, поля ответа — дословно из §1.6.
#[test]
fn typing_route_is_s_catalog_extension_not_d02() {
    let spec = routes::find(HttpMethod::Post, ORACLE_PATH)
        .expect("маршрут typing зарегистрирован в каталоге");
    assert_eq!(spec.method.as_str(), "POST");
    assert_eq!(spec.path, ORACLE_PATH);
    assert!(spec.body_keys.is_empty(), "тело отсутствует");
    assert_eq!(routes::TYPING_PATH, ORACLE_PATH, "один источник пути");

    assert_eq!(
        routes::render_path(spec, &[CHANNEL_ID]).expect("рендер"),
        format!("/channels/{CHANNEL_ID}/typing")
    );

    assert!(
        !D02_TSV.lines().any(|line| line.contains("/typing")),
        "D-02 не подтверждает typing — оракул отдельный [S]"
    );
    assert!(
        !routes::ROUTES.iter().any(|route| route.path == ORACLE_PATH),
        "frozen каталог D-02 не расширяется"
    );

    // Тело ответа строим из дословных wire-ключей оракула, а не из имён полей.
    let wire = format!(
        r#"{{"{}":1200,"{}":0}}"#,
        ORACLE_RESPONSE_KEYS[0], ORACLE_RESPONSE_KEYS[1]
    );
    let cooldowns: TypingCooldowns =
        serde_json::from_str(&wire).expect("wire-ключи §1.6 разбираются");
    assert_eq!(cooldowns.message_send_cooldown_ms, Some(1200));
    assert_eq!(cooldowns.thread_create_cooldown_ms, Some(0));
}

/// Тело `#[tauri::command]`-функции по имени: от `async fn name(` до следующей
/// `#[tauri::command]` или до конца файла. Окно для этой проверки не нужно.
fn command_body<'a>(source: &'a str, name: &str) -> &'a str {
    let marker = format!("async fn {name}(");
    let start = source
        .find(&marker)
        .unwrap_or_else(|| panic!("main.rs должен объявлять команду {name}"));
    let rest = &source[start..];
    let end = rest.find("#[tauri::command]").unwrap_or(rest.len());
    &rest[..end]
}

fn registered_commands(source: &str) -> Vec<&str> {
    let open = "generate_handler![";
    let start = source
        .find(open)
        .expect("main.rs объявляет invoke_handler!")
        + open.len();
    let block = &source[start..start + source[start..].find(']').expect("закрывающая ]")];
    block
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .filter(|name| !name.is_empty())
        .collect()
}

#[test]
fn tauri_send_typing_reads_bearer_through_token_or_without_logging() {
    let source = include_str!("../src/main.rs");
    let commands = registered_commands(source);
    assert_eq!(
        commands
            .iter()
            .filter(|name| **name == "send_typing")
            .count(),
        1,
        "send_typing регистрируется ровно один раз"
    );

    let body = command_body(source, "send_typing");
    assert!(
        body.contains("token_or::<TypingError"),
        "bearer читается только helper'ом token_or: {body}"
    );
    assert_eq!(
        body.matches("backend.token()").count(),
        1,
        "ровно одно чтение bearer из backend: {body}"
    );
    assert!(
        body.contains("TypingError::Storage"),
        "отказ keyring остаётся storage, а не сетью: {body}"
    );
    assert!(
        body.contains("TypingError::Unauthorized"),
        "отсутствие токена — unauthorized: {body}"
    );
    assert!(
        body.contains("DiscordTyping::new(&token)"),
        "токен уходит только в auth-заголовок клиента: {body}"
    );
    assert!(
        !body.contains("token: String") && !body.contains("bearer: String"),
        "токен не приходит из фронтенда: {body}"
    );
    assert!(
        !body.contains("println!") && !body.contains("eprintln!"),
        "токен и ответ не логируются: {body}"
    );
}
