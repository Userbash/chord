//! M-07: read-state ack — REST-срез и Tauri-wiring.
//!
//! Оракул тела: `docs/re/rest-d02.md` §4 (`POST /channels/{id}/messages/{id}/ack`
//! ×5, requestId `ready|57992.7577`) — ровно `{"token":null,"last_viewed":4310}`;
//! `PARITY/G6-REPORT-RW40-02.md` §3.5; `fixtures/gateway/by-type/046-MESSAGE_ACK.json`
//! (`last_viewed: 4310` — счётчик read-state, не snowflake сообщения).
//! Транспорт — фейк, сети/окна нет.

use discord_core::rest::routes::{find, HttpMethod};
use discord_core::rest::transport::HttpResponse;
use chord_client::messages::{
    ack_message, AckTransport, DiscordReadState, MessageError, ReadStateAck,
};
use serde_json::{json, Value};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

const CHANNEL: &str = "1293192721237938187";
const MESSAGE: &str = "1561348877573947515";
const ACK_ROUTE: &str = "/channels/{id}/messages/{id}/ack";

type AckCall = (String, Vec<u8>);

struct FakeAckTransport {
    responses: Mutex<Vec<Result<HttpResponse, MessageError>>>,
    calls: Arc<Mutex<Vec<AckCall>>>,
}

impl AckTransport for FakeAckTransport {
    fn post_ack<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, MessageError>> + Send + 'a>> {
        self.calls
            .lock()
            .expect("журнал вызовов")
            .push((target.to_string(), body.to_vec()));
        let next = self.responses.lock().expect("очередь ответов").remove(0);
        Box::pin(async move { next })
    }
}

fn response(status: u16) -> HttpResponse {
    HttpResponse {
        status,
        headers: Vec::new(),
        body: Vec::new(),
    }
}

/// API поверх фейка; каждый вызов съедает следующий ответ из очереди.
fn api_with(
    responses: Vec<Result<HttpResponse, MessageError>>,
) -> (DiscordReadState, Arc<Mutex<Vec<AckCall>>>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let fake = FakeAckTransport {
        responses: Mutex::new(responses),
        calls: Arc::clone(&calls),
    };
    (DiscordReadState::with_transport(Arc::new(fake)), calls)
}

fn run<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("runtime")
        .block_on(future)
}

fn body_of(body: &[u8]) -> Value {
    serde_json::from_slice(body).expect("тело ack — JSON")
}

/// Тело оракула дословно из захвата (`rest-d02.md` §4) и порядок ключей каталога
/// не подменяются: сверяем JSON-равенство и точный набор ключей маршрута.
#[test]
fn ack_posts_oracle_body_to_catalog_route() {
    let (api, calls) = api_with(vec![Ok(response(200))]);
    assert_eq!(run(ack_message(&api, CHANNEL, MESSAGE, 4310, None)), Ok(()));

    let calls = calls.lock().expect("журнал");
    assert_eq!(calls.len(), 1, "ack — ровно один POST");
    let (target, body) = &calls[0];
    assert_eq!(
        target,
        &format!("/channels/{CHANNEL}/messages/{MESSAGE}/ack"),
        "target берётся из каталога маршрутов"
    );

    let oracle: Value =
        serde_json::from_str(r#"{"token":null,"last_viewed":4310}"#).expect("тело оракула");
    let actual = body_of(body);
    assert_eq!(actual, oracle, "тело ack обязано совпасть с захватом D-02");

    let mut keys: Vec<String> = actual
        .as_object()
        .expect("тело — объект")
        .keys()
        .cloned()
        .collect();
    keys.sort();
    let route = find(HttpMethod::Post, ACK_ROUTE).expect("маршрут ack уже в каталоге");
    let mut expected = route.body_keys.to_vec();
    expected.sort_unstable();
    assert_eq!(keys, expected, "набор ключей — ровно body_keys каталога");
    assert_eq!(keys, vec!["last_viewed".to_string(), "token".to_string()]);
}

/// `token` в теле — nullable read-state токен, не bearer: `None` уходит `null`,
/// значение из аргумента уходит как есть, подстановки auth-токена нет.
#[test]
fn nullable_body_token_is_sent_as_null_and_never_substituted() {
    let (api, calls) = api_with(vec![Ok(response(200)), Ok(response(200))]);

    assert_eq!(run(ack_message(&api, CHANNEL, MESSAGE, 4310, None)), Ok(()));
    assert_eq!(
        run(ack_message(
            &api,
            CHANNEL,
            MESSAGE,
            4310,
            Some("read-state-token".to_string())
        )),
        Ok(())
    );

    let calls = calls.lock().expect("журнал");
    let null_body = body_of(&calls[0].1);
    assert!(
        null_body["token"].is_null(),
        "при неизвестном значении token в теле ровно null: {null_body}"
    );
    let value_body = body_of(&calls[1].1);
    assert_eq!(value_body["token"], json!("read-state-token"));
    assert_eq!(value_body["last_viewed"], json!(4310));
}

/// `last_viewed` — счётчик read-state (`MESSAGE_ACK.last_viewed`), а не snowflake:
/// число, ноль остаётся нулём (не null и не id сообщения).
#[test]
fn last_viewed_is_a_number_and_zero_is_not_null() {
    let (api, calls) = api_with(vec![Ok(response(200)), Ok(response(200))]);
    assert_eq!(run(ack_message(&api, CHANNEL, MESSAGE, 0, None)), Ok(()));
    assert_eq!(run(ack_message(&api, CHANNEL, MESSAGE, 4310, None)), Ok(()));

    let calls = calls.lock().expect("журнал");
    let zero = body_of(&calls[0].1);
    assert_eq!(zero["last_viewed"], json!(0));
    assert!(zero["last_viewed"].is_number(), "{zero}");
    assert_ne!(
        zero["last_viewed"],
        json!(MESSAGE),
        "last_viewed не подменяется id сообщения"
    );

    let counter = body_of(&calls[1].1);
    assert_eq!(counter["last_viewed"], json!(4310));
    assert!(counter["last_viewed"].is_u64(), "{counter}");
    assert!(!counter["last_viewed"].is_string(), "{counter}");
}

/// 200 → `Ok`; 401/403/404/429, прочие 4xx и 5xx — типизированные ошибки.
#[test]
fn ack_maps_http_statuses_to_typed_errors() {
    let cases = [
        (401u16, MessageError::Unauthorized),
        (403, MessageError::Forbidden),
        (404, MessageError::NotFound),
        (429, MessageError::RateLimited { retry_after_ms: 0 }),
        (400, MessageError::Client { status: 400 }),
        (500, MessageError::Server { status: 500 }),
        (503, MessageError::Server { status: 503 }),
    ];
    let mut responses = vec![Ok(response(200))];
    responses.extend(cases.iter().map(|(status, _)| Ok(response(*status))));
    let (api, calls) = api_with(responses);

    assert_eq!(
        run(ack_message(&api, CHANNEL, MESSAGE, 4310, None)),
        Ok(()),
        "200 — подтверждённый успех ack"
    );
    for (status, expected) in cases {
        assert_eq!(
            run(ack_message(&api, CHANNEL, MESSAGE, 4310, None)),
            Err(expected),
            "HTTP {status}"
        );
    }
    assert_eq!(calls.lock().expect("журнал").len(), 8);
}

/// Сетевой отказ и отказ клиента из транспорта не маскируются под успех.
#[test]
fn transport_failures_are_typed_and_not_masked() {
    let (api, _) = api_with(vec![Err(MessageError::Network)]);
    assert_eq!(
        run(ack_message(&api, CHANNEL, MESSAGE, 4310, None)),
        Err(MessageError::Network)
    );

    let (api, _) = api_with(vec![Err(MessageError::RateLimited {
        retry_after_ms: 1500,
    })]);
    assert_eq!(
        run(ack_message(&api, CHANNEL, MESSAGE, 4310, None)),
        Err(MessageError::RateLimited {
            retry_after_ms: 1500
        })
    );
}

/// Невалидные id отсекаются до транспорта (пустая очередь ответов = вызов был бы паникой).
#[test]
fn invalid_ids_are_rejected_before_transport() {
    let (api, calls) = api_with(Vec::new());
    for (channel, message) in [
        ("", MESSAGE),
        ("abc", MESSAGE),
        ("{id}", MESSAGE),
        (CHANNEL, ""),
        (CHANNEL, "abc"),
    ] {
        assert_eq!(
            run(ack_message(&api, channel, message, 4310, None)),
            Err(MessageError::InvalidArg),
            "channel={channel:?} message={message:?}"
        );
    }
    assert!(
        calls.lock().expect("журнал").is_empty(),
        "до транспорта дело не доходит"
    );
}

/// Bearer не печатается в `Debug`, пустой bearer не принимается.
#[test]
fn read_state_debug_and_empty_bearer_do_not_expose_token() {
    assert_eq!(
        DiscordReadState::new("").map(|_| ()),
        Err(MessageError::InvalidArg)
    );
    let api = DiscordReadState::new("secret-bearer").expect("конфиг клиента");
    assert!(!format!("{api:?}").contains("secret-bearer"));
}

/// Трейт объектно-безопасен: команда может держать `&dyn ReadStateAck`.
#[test]
fn ack_is_available_through_a_trait_object() {
    let (api, calls) = api_with(vec![Ok(response(200))]);
    let dynamic: &dyn ReadStateAck = &api;
    assert_eq!(
        run(ack_message(dynamic, CHANNEL, MESSAGE, 4310, None)),
        Ok(())
    );
    assert_eq!(calls.lock().expect("журнал").len(), 1);
}

/// Tauri-команда `ack_message`: bearer — через `token_or` из backend/keyring,
/// `token` в теле — отдельный nullable аргумент фронтенда, не bearer.
#[test]
fn tauri_ack_command_separates_bearer_from_nullable_body_token() {
    let source = include_str!("../src/main.rs");

    let open = "generate_handler![";
    let start = source.find(open).expect("invoke_handler!") + open.len();
    let handler = &source[start..start + source[start..].find(']').expect("закрытие списка")];
    assert!(
        handler
            .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .any(|name| name == "ack_message"),
        "ack_message не зарегистрирован в invoke_handler!"
    );

    let marker = "async fn ack_message(";
    let body_start = source
        .find(marker)
        .expect("main.rs объявляет команду ack_message");
    let rest = &source[body_start..];
    let end = rest.find("#[tauri::command]").unwrap_or(rest.len());
    let body = &rest[..end];

    assert!(
        body.contains("token_or::<MessageError"),
        "bearer читается только helper'ом token_or: {body}"
    );
    assert_eq!(
        body.matches("backend.token()").count(),
        1,
        "ровно одно чтение bearer из backend"
    );
    assert!(
        body.contains("token: Option<String>"),
        "token в теле — nullable аргумент, не bearer: {body}"
    );
    assert!(
        body.contains("last_viewed: u64"),
        "last_viewed — число read-state, не id: {body}"
    );
    assert!(
        body.contains("DiscordReadState::new(&bearer)"),
        "bearer уходит в auth клиента, а не в тело: {body}"
    );
    assert!(
        !body.contains("println!") && !body.contains("eprintln!"),
        "токены не логируются: {body}"
    );
}
