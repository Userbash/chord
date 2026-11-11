use discord_core::rest::transport::HttpResponse;
use discord_gui_spike::reactions::{
    add_reaction, build_reaction_path, encode_reaction_emoji, flip_reaction, parse_retry_after_ms,
    percent_encode_emoji, remove_reaction, toggle_reaction, toggle_reaction_from_active,
    DiscordReactionApi, ReactionApi, ReactionError, ReactionTransport, REACTION_PUT_EMPTY_BODY,
};
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

type ReactionCall = (String, String, String, bool);

#[derive(Clone)]
struct FakeReactionApi {
    result: Result<(), ReactionError>,
    calls: Arc<Mutex<Vec<ReactionCall>>>,
}

impl ReactionApi for FakeReactionApi {
    fn toggle_reaction<'a>(
        &'a self,
        channel_id: &'a str,
        message_id: &'a str,
        emoji: &'a str,
        active: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(), ReactionError>> + Send + 'a>> {
        self.calls.lock().unwrap().push((
            channel_id.to_string(),
            message_id.to_string(),
            emoji.to_string(),
            active,
        ));
        let result = self.result.clone();
        Box::pin(async move { result })
    }
}

fn run<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(future)
}

fn fake(result: Result<(), ReactionError>) -> FakeReactionApi {
    FakeReactionApi {
        result,
        calls: Arc::new(Mutex::new(Vec::new())),
    }
}

#[test]
fn active_selects_put_and_inactive_selects_delete() {
    let api = fake(Ok(()));
    assert_eq!(run(toggle_reaction(&api, "123", "456", "👍", true)), Ok(()));
    assert_eq!(
        run(toggle_reaction(&api, "123", "456", "👍", false)),
        Ok(())
    );
    assert_eq!(
        *api.calls.lock().unwrap(),
        vec![
            ("123".into(), "456".into(), "👍".into(), true),
            ("123".into(), "456".into(), "👍".into(), false),
        ]
    );
}

#[test]
fn invalid_ids_and_emoji_are_rejected_before_api() {
    let api = fake(Ok(()));
    for (channel, message, emoji) in [
        ("", "1", "👍"),
        ("abc", "1", "👍"),
        ("1", "", "👍"),
        ("1", "abc", "👍"),
        ("1", "2", ""),
        ("1", "2", "x".repeat(257).as_str()),
    ] {
        assert_eq!(
            run(toggle_reaction(&api, channel, message, emoji, true)),
            Err(ReactionError::InvalidArg)
        );
    }
    assert!(api.calls.lock().unwrap().is_empty());
}

#[test]
fn emoji_path_uses_percent_encoding_without_plain_unicode() {
    assert_eq!(percent_encode_emoji("👍"), "%F0%9F%91%8D");
    assert_eq!(percent_encode_emoji("party:123"), "party%3A123");
    assert_eq!(percent_encode_emoji("a/b"), "a%2Fb");
}

#[test]
fn typed_failures_are_preserved() {
    for error in [
        ReactionError::Unauthorized,
        ReactionError::Forbidden,
        ReactionError::NotFound,
        ReactionError::RateLimited {
            retry_after_ms: 250,
        },
        ReactionError::Client { status: 400 },
        ReactionError::Server { status: 500 },
        ReactionError::Network,
        ReactionError::Storage,
        ReactionError::InvalidResponse,
    ] {
        let api = fake(Err(error.clone()));
        assert_eq!(run(toggle_reaction(&api, "1", "2", "👍", true)), Err(error));
    }
    // SEC-02: отказ хранилища — собственный kind, а не «сеть».
    let value = serde_json::to_value(ReactionError::Storage).expect("storage");
    assert_eq!(value["kind"], "storage", "{value}");
    assert!(!ReactionError::Storage.to_string().is_empty());
}

#[test]
fn sender_debug_and_empty_token_do_not_expose_or_accept_token() {
    assert!(matches!(
        DiscordReactionApi::new(""),
        Err(ReactionError::InvalidArg)
    ));
    let sender = DiscordReactionApi::new("secret-token").expect("sender config");
    assert!(!format!("{sender:?}").contains("secret-token"));
}

#[test]
fn tauri_command_reads_token_from_backend_and_is_registered() {
    let source = include_str!("../src/main.rs");
    assert!(source.contains("backend.token()"));
    let start = source.find("generate_handler![").expect("handler") + "generate_handler![".len();
    let end = source[start..].find(']').expect("handler close");
    let commands = &source[start..start + end];
    assert!(commands
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .any(|name| name == "toggle_reaction"));
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordedHttpCall {
    method: &'static str,
    target: String,
    body: Vec<u8>,
}

struct MockReactionTransport {
    calls: Mutex<Vec<RecordedHttpCall>>,
    responses: Mutex<VecDeque<Result<HttpResponse, ReactionError>>>,
}

impl MockReactionTransport {
    fn new(responses: Vec<Result<HttpResponse, ReactionError>>) -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            responses: Mutex::new(VecDeque::from(responses)),
        })
    }

    fn calls(&self) -> Vec<RecordedHttpCall> {
        self.calls.lock().unwrap().clone()
    }

    fn next_response(&self) -> Result<HttpResponse, ReactionError> {
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Ok(HttpResponse {
                status: 204,
                headers: Vec::new(),
                body: Vec::new(),
            }))
    }
}

impl ReactionTransport for MockReactionTransport {
    fn put<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, ReactionError>> + Send + 'a>> {
        self.calls.lock().unwrap().push(RecordedHttpCall {
            method: "PUT",
            target: target.to_string(),
            body: body.to_vec(),
        });
        let res = self.next_response();
        Box::pin(async move { res })
    }

    fn delete<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, ReactionError>> + Send + 'a>> {
        self.calls.lock().unwrap().push(RecordedHttpCall {
            method: "DELETE",
            target: target.to_string(),
            body: Vec::new(),
        });
        let res = self.next_response();
        Box::pin(async move { res })
    }
}

fn http_response(status: u16, headers: &[(&str, &str)], body: &[u8]) -> HttpResponse {
    HttpResponse {
        status,
        headers: headers
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect(),
        body: body.to_vec(),
    }
}

#[test]
fn custom_and_unicode_emoji_encoding_and_route_construction() {
    // 1. Unicode-эмодзи (одиночный глиф и последовательность с U+FE0F)
    assert_eq!(encode_reaction_emoji("👍"), Ok("%F0%9F%91%8D".to_string()));
    assert_eq!(encode_reaction_emoji("👋"), Ok("%F0%9F%91%8B".to_string()));
    assert_eq!(
        encode_reaction_emoji("❤️"),
        Ok("%E2%9D%A4%EF%B8%8F".to_string())
    );

    // 2. Кастомные эмодзи формата name:id и a:name:id (двоеточие кодируется в %3A)
    assert_eq!(
        encode_reaction_emoji("party:123456789012345678"),
        Ok("party%3A123456789012345678".to_string())
    );
    assert_eq!(
        encode_reaction_emoji("a:party_blob:987654321098765432"),
        Ok("a%3Aparty_blob%3A987654321098765432".to_string())
    );

    // 3. Обёртка Discord `<:name:id>` и `<a:name:id>` нормализуется к `name%3Aid` и `a%3Aname%3Aid`
    assert_eq!(
        encode_reaction_emoji("<:party:123456789012345678>"),
        Ok("party%3A123456789012345678".to_string())
    );
    assert_eq!(
        encode_reaction_emoji("<a:party_blob:987654321098765432>"),
        Ok("a%3Aparty_blob%3A987654321098765432".to_string())
    );

    // 4. Формирование полного REST-пути /channels/{channel_id}/messages/{message_id}/reactions/{emoji}/@me
    assert_eq!(
        build_reaction_path("1100000000000000001", "1200000000000000001", "👍"),
        Ok(
            "/channels/1100000000000000001/messages/1200000000000000001/reactions/%F0%9F%91%8D/@me"
                .to_string()
        )
    );
    assert_eq!(
        build_reaction_path(
            "1100000000000000001",
            "1200000000000000001",
            "party:987654321098765432"
        ),
        Ok(
            "/channels/1100000000000000001/messages/1200000000000000001/reactions/party%3A987654321098765432/@me"
                .to_string()
        )
    );
    assert_eq!(
        build_reaction_path(
            "1100000000000000001",
            "1200000000000000001",
            "a:party:987654321098765432"
        ),
        Ok(
            "/channels/1100000000000000001/messages/1200000000000000001/reactions/a%3Aparty%3A987654321098765432/@me"
                .to_string()
        )
    );
}

#[test]
fn strict_validation_rejects_control_chars_slashes_whitespace_and_malformed_custom_emoji() {
    let transport = MockReactionTransport::new(Vec::new());
    let api = DiscordReactionApi::with_transport(transport.clone());

    let invalid_cases = [
        // Пустые или нечисловые snowflake channel_id / message_id
        ("", "1200000000000000001", "👍"),
        ("   ", "1200000000000000001", "👍"),
        ("110/120", "1200000000000000001", "👍"),
        ("99999999999999999999999", "1200000000000000001", "👍"),
        ("1100000000000000001", "", "👍"),
        ("1100000000000000001", "   ", "👍"),
        ("1100000000000000001", "120/130", "👍"),
        ("1100000000000000001", "99999999999999999999999", "👍"),
        // Пустой или пробельный emoji
        ("1100000000000000001", "1200000000000000001", ""),
        ("1100000000000000001", "1200000000000000001", "   "),
        // Слеши и обход пути в emoji
        ("1100000000000000001", "1200000000000000001", "a/b"),
        ("1100000000000000001", "1200000000000000001", "👍/👎"),
        ("1100000000000000001", "1200000000000000001", "a\\b"),
        ("1100000000000000001", "1200000000000000001", "../@me"),
        ("1100000000000000001", "1200000000000000001", "."),
        ("1100000000000000001", "1200000000000000001", ".."),
        // Управляющие символы (включая \0, \r, \n, \t, DEL)
        ("1100000000000000001", "1200000000000000001", "👍\n"),
        ("1100000000000000001", "1200000000000000001", "👍\r"),
        ("1100000000000000001", "1200000000000000001", "\t👍"),
        ("1100000000000000001", "1200000000000000001", "👍\0"),
        (
            "1100000000000000001",
            "1200000000000000001",
            "party\x07:123",
        ),
        (
            "1100000000000000001",
            "1200000000000000001",
            "party\x7f:123",
        ),
        // Битые кастомные эмодзи с двоеточием
        ("1100000000000000001", "1200000000000000001", ":123456"),
        ("1100000000000000001", "1200000000000000001", "party:"),
        (
            "1100000000000000001",
            "1200000000000000001",
            "party:not_a_number",
        ),
        (
            "1100000000000000001",
            "1200000000000000001",
            "b:party:123456",
        ),
        ("1100000000000000001", "1200000000000000001", "a::123456"),
        ("1100000000000000001", "1200000000000000001", "a:b:c:123456"),
    ];

    for (channel_id, message_id, emoji) in invalid_cases {
        assert_eq!(
            build_reaction_path(channel_id, message_id, emoji),
            Err(ReactionError::InvalidArg),
            "build_reaction_path should reject ({channel_id:?}, {message_id:?}, {emoji:?})"
        );
        assert_eq!(
            run(toggle_reaction(&api, channel_id, message_id, emoji, true)),
            Err(ReactionError::InvalidArg),
            "toggle_reaction should reject ({channel_id:?}, {message_id:?}, {emoji:?})"
        );
    }

    assert!(
        transport.calls().is_empty(),
        "невалидные входные параметры не должны приводить к HTTP-вызовам"
    );
}

#[test]
fn discord_reaction_api_transport_handles_204_no_content_and_put_delete_routing() {
    let transport = MockReactionTransport::new(vec![
        Ok(http_response(204, &[], b"")),
        Ok(http_response(204, &[], b"")),
        Ok(http_response(200, &[], b"")),
    ]);
    let api = DiscordReactionApi::with_transport(transport.clone());

    assert_eq!(
        run(toggle_reaction(
            &api,
            "1100000000000000001",
            "1200000000000000001",
            "👍",
            true
        )),
        Ok(())
    );
    assert_eq!(
        run(toggle_reaction(
            &api,
            "1100000000000000001",
            "1200000000000000001",
            "a:party:987654321098765432",
            false
        )),
        Ok(())
    );
    assert_eq!(
        run(api.add_reaction(
            "1100000000000000001",
            "1200000000000000001",
            "<:custom_ok:123456789012345678>"
        )),
        Ok(())
    );

    assert_eq!(
        transport.calls(),
        vec![
            RecordedHttpCall {
                method: "PUT",
                target: "/channels/1100000000000000001/messages/1200000000000000001/reactions/%F0%9F%91%8D/@me"
                    .to_string(),
                body: Vec::new(),
            },
            RecordedHttpCall {
                method: "DELETE",
                target: "/channels/1100000000000000001/messages/1200000000000000001/reactions/a%3Aparty%3A987654321098765432/@me"
                    .to_string(),
                body: Vec::new(),
            },
            RecordedHttpCall {
                method: "PUT",
                target: "/channels/1100000000000000001/messages/1200000000000000001/reactions/custom_ok%3A123456789012345678/@me"
                    .to_string(),
                body: Vec::new(),
            },
        ]
    );
}

#[test]
fn http_429_parses_retry_after_float_seconds_from_json_and_header_and_maps_401_403_404_network() {
    let transport = MockReactionTransport::new(vec![
        // 1. 429 с float retry_after в JSON-теле (1.5 с -> 1500 мс)
        Ok(http_response(
            429,
            &[],
            br#"{"message":"You are being rate limited.","retry_after":1.5,"global":false}"#,
        )),
        // 2. 429 с дробным retry_after в JSON-теле (0.25 с -> 250 мс)
        Ok(http_response(429, &[], br#"{"retry_after":0.25}"#)),
        // 3. 429 с заголовком Retry-After (2.5 с -> 2500 мс)
        Ok(http_response(429, &[("Retry-After", "2.5")], b"")),
        // 4. 429 с заголовком retry-after в нижнем регистре (3 с -> 3000 мс)
        Ok(http_response(429, &[("retry-after", " 3 ")], b"")),
        // 5. 429 с заголовком x-ratelimit-reset-after (0.75 с -> 750 мс)
        Ok(http_response(
            429,
            &[("x-ratelimit-reset-after", "0.75")],
            b"",
        )),
        // 6. 429 с субмиллисекундным положительным retry_after (0.0004 с -> минимум 1 мс)
        Ok(http_response(429, &[], br#"{"retry_after":0.0004}"#)),
        // 7. 401 / 403 / 404 / 502 / Network
        Ok(http_response(
            401,
            &[],
            br#"{"message":"401: Unauthorized"}"#,
        )),
        Ok(http_response(
            403,
            &[],
            br#"{"message":"Missing Permissions"}"#,
        )),
        Ok(http_response(404, &[], br#"{"message":"Unknown Message"}"#)),
        Ok(http_response(502, &[], b"Bad Gateway")),
        Err(ReactionError::Network),
    ]);
    let api = DiscordReactionApi::with_transport(transport);

    let err_json_1500 = run(toggle_reaction(&api, "110", "120", "👍", true)).unwrap_err();
    assert_eq!(
        err_json_1500,
        ReactionError::RateLimited {
            retry_after_ms: 1500
        }
    );
    let serialized = serde_json::to_value(&err_json_1500).expect("serialize 429");
    assert_eq!(serialized["kind"], "rateLimited");
    assert_eq!(serialized["retry_after_ms"], 1500);
    assert!(err_json_1500.to_string().contains("1500"));

    assert_eq!(
        run(toggle_reaction(&api, "110", "120", "👍", false)),
        Err(ReactionError::RateLimited {
            retry_after_ms: 250
        })
    );
    assert_eq!(
        run(toggle_reaction(&api, "110", "120", "👍", true)),
        Err(ReactionError::RateLimited {
            retry_after_ms: 2500
        })
    );
    assert_eq!(
        run(toggle_reaction(&api, "110", "120", "👍", false)),
        Err(ReactionError::RateLimited {
            retry_after_ms: 3000
        })
    );
    assert_eq!(
        run(toggle_reaction(&api, "110", "120", "👍", true)),
        Err(ReactionError::RateLimited {
            retry_after_ms: 750
        })
    );
    assert_eq!(
        run(toggle_reaction(&api, "110", "120", "👍", true)),
        Err(ReactionError::RateLimited { retry_after_ms: 1 })
    );
    assert_eq!(
        run(toggle_reaction(&api, "110", "120", "👍", true)),
        Err(ReactionError::Unauthorized)
    );
    assert_eq!(
        run(toggle_reaction(&api, "110", "120", "👍", true)),
        Err(ReactionError::Forbidden)
    );
    assert_eq!(
        run(toggle_reaction(&api, "110", "120", "👍", true)),
        Err(ReactionError::NotFound)
    );
    assert_eq!(
        run(toggle_reaction(&api, "110", "120", "👍", true)),
        Err(ReactionError::Server { status: 502 })
    );
    assert_eq!(
        run(toggle_reaction(&api, "110", "120", "👍", true)),
        Err(ReactionError::Network)
    );

    // Прямая проверка parse_retry_after_ms на некорректных значениях
    assert_eq!(
        parse_retry_after_ms(&http_response(429, &[("Retry-After", "-1")], b"{}")),
        0
    );
    assert_eq!(
        parse_retry_after_ms(&http_response(
            429,
            &[("Retry-After", "invalid")],
            b"not-json"
        )),
        0
    );

    // Отсутствие утечки токена в Debug/Display
    let secret = "mfa.super-secret-reaction-token-999";
    let real_api = DiscordReactionApi::new(secret).expect("valid token");
    assert!(!format!("{real_api:?}").contains(secret));
    for err in [
        ReactionError::Unauthorized,
        ReactionError::Forbidden,
        ReactionError::NotFound,
        ReactionError::RateLimited {
            retry_after_ms: 1500,
        },
        ReactionError::Network,
    ] {
        assert!(!format!("{err:?}").contains(secret));
        assert!(!format!("{err}").contains(secret));
    }
}

#[test]
fn idempotent_toggle_helper_adds_when_inactive_and_removes_when_active() {
    let transport = MockReactionTransport::new(vec![
        Ok(http_response(204, &[], b"")),
        Ok(http_response(204, &[], b"")),
        Ok(http_response(204, &[], b"")),
        Ok(http_response(204, &[], b"")),
    ]);
    let api = DiscordReactionApi::with_transport(transport.clone());

    // 1. При active = false (реакция ещё не стоит) переключение вызывает PUT (add) и возвращает новое состояние true
    let next_state = run(toggle_reaction_from_active(
        &api,
        "1100000000000000001",
        "1200000000000000001",
        "🔥",
        false,
    ))
    .expect("toggle from inactive should add");
    assert!(
        next_state,
        "после add из active=false новое состояние должно быть true"
    );

    // 2. При active = true (реакция уже стоит) переключение вызывает DELETE (remove) и возвращает новое состояние false
    let after_second_toggle = run(api.toggle_from_active(
        "1100000000000000001",
        "1200000000000000001",
        "🔥",
        next_state,
    ))
    .expect("toggle from active should remove");
    assert!(
        !after_second_toggle,
        "после remove из active=true новое состояние должно быть false"
    );

    // 3. Алиас flip_reaction и явные идемпотентные хелперы add_reaction / remove_reaction
    assert_eq!(
        run(add_reaction(
            &api,
            "1100000000000000001",
            "1200000000000000001",
            "party:123456789012345678"
        )),
        Ok(())
    );
    assert_eq!(
        run(flip_reaction(
            &api,
            "1100000000000000001",
            "1200000000000000001",
            "party:123456789012345678",
            false
        )),
        Ok(true)
    );
    assert_eq!(
        run(remove_reaction(
            &api,
            "1100000000000000001",
            "1200000000000000001",
            "party:123456789012345678"
        )),
        Ok(())
    );

    let calls = transport.calls();
    assert_eq!(calls.len(), 5);
    assert_eq!(calls[0].method, "PUT");
    assert!(
        calls[0].body.is_empty(),
        "PUT /reactions обязан быть без тела (0 байт)"
    );
    assert_eq!(calls[1].method, "DELETE");
    assert!(calls[1].body.is_empty());
    assert_eq!(calls[2].method, "PUT");
    assert!(
        calls[2].body.is_empty(),
        "PUT /reactions обязан быть без тела (0 байт)"
    );
    assert_eq!(calls[3].method, "PUT");
    assert!(
        calls[3].body.is_empty(),
        "PUT /reactions обязан быть без тела (0 байт)"
    );
    assert_eq!(calls[4].method, "DELETE");
    assert!(calls[4].body.is_empty());
}

// --- BE-P3-01: PUT реакции без тела (оракул RE/rest-d02.json endpoint 38) ---
//
// Оракул: `PUT /api/v9/channels/{id}/messages/{id}/reactions/%F0%9F%91%8B/%40me`,
// `reqBodyKeys: []`, `reqBodyRequests: 0`, `requestsWithContentType: 0`,
// `statuses: [204]`. Поэтому отдельный вызов шлёт 0 байт и не ставит Content-Type
// (клиент добавляет `Content-Type: application/json` только для непустого тела,
// `client.rs`: `if !prepared.body.is_empty() ...`).
// Существующий `add_reaction`/`toggle_reaction` остаётся с телом `{}` — это
// зафиксировано тестами выше и регрессионным тестом ниже.

const EMPTY_BODY_PATH: &str =
    "/channels/1100000000000000001/messages/1200000000000000001/reactions/%F0%9F%91%8D/@me";

#[test]
fn empty_body_constant_is_zero_bytes_and_default_transport_method_puts_it() {
    assert_eq!(
        REACTION_PUT_EMPTY_BODY, b"",
        "оракул rest-d02.json: reqBodyKeys: [] — тело пустое"
    );
    assert!(REACTION_PUT_EMPTY_BODY.is_empty());

    // Транспорт-заглушка реализует только `put`; `put_empty_body` — метод по умолчанию,
    // поэтому проверяем именно дефолтную реализацию трейта.
    let transport = MockReactionTransport::new(vec![Ok(http_response(204, &[], b""))]);
    let api = DiscordReactionApi::with_transport(transport.clone());

    let response = run(ReactionTransport::put_empty_body(&api, EMPTY_BODY_PATH))
        .expect("PUT без тела должен принимать 204 No Content");
    assert_eq!(response.status, 204);
    assert_eq!(
        transport.calls(),
        vec![RecordedHttpCall {
            method: "PUT",
            target: EMPTY_BODY_PATH.to_string(),
            body: Vec::new(),
        }],
        "put_empty_body обязан отправить 0-байтовое тело на маршрут .../reactions/{{emoji}}/@me"
    );
}

#[test]
fn add_reaction_empty_body_sends_zero_byte_put_and_accepts_204() {
    let transport = MockReactionTransport::new(vec![
        // 204 No Content с заголовком content-length: 0 — типичный ответ Discord.
        Ok(http_response(204, &[("content-length", "0")], b"")),
    ]);
    let api = DiscordReactionApi::with_transport(transport.clone());

    assert_eq!(
        run(api.add_reaction_empty_body("1100000000000000001", "1200000000000000001", "👍")),
        Ok(())
    );

    let calls = transport.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].method, "PUT");
    assert_eq!(calls[0].target, EMPTY_BODY_PATH);
    assert!(
        calls[0].body.is_empty(),
        "тело PUT должно быть 0 байт, получено {:?}",
        calls[0].body
    );
    assert_ne!(calls[0].body, b"{}".to_vec());
}

#[test]
fn add_reaction_empty_body_encodes_custom_emoji_and_validates_before_transport() {
    let transport = MockReactionTransport::new(vec![
        Ok(http_response(204, &[], b"")),
        Ok(http_response(204, &[], b"")),
    ]);
    let api = DiscordReactionApi::with_transport(transport.clone());

    assert_eq!(
        run(api.add_reaction_empty_body(
            "1100000000000000001",
            "1200000000000000001",
            "<a:party_blob:987654321098765432>"
        )),
        Ok(())
    );
    assert_eq!(
        run(api.add_reaction_empty_body("1100000000000000001", "1200000000000000001", "❤️")),
        Ok(())
    );

    let calls = transport.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(
        calls[0].target,
        "/channels/1100000000000000001/messages/1200000000000000001/reactions/a%3Aparty_blob%3A987654321098765432/@me"
    );
    assert_eq!(
        calls[1].target,
        "/channels/1100000000000000001/messages/1200000000000000001/reactions/%E2%9D%A4%EF%B8%8F/@me"
    );
    assert!(calls
        .iter()
        .all(|call| call.method == "PUT" && call.body.is_empty()));

    // Невалидные входы отсекаются до сети — как у add_reaction/toggle_reaction.
    let guarded = MockReactionTransport::new(Vec::new());
    let guarded_api = DiscordReactionApi::with_transport(guarded.clone());
    for (channel_id, message_id, emoji) in [
        ("", "1200000000000000001", "👍"),
        ("abc", "1200000000000000001", "👍"),
        ("1100000000000000001", "", "👍"),
        ("1100000000000000001", "120/130", "👍"),
        ("1100000000000000001", "1200000000000000001", ""),
        ("1100000000000000001", "1200000000000000001", "a/b"),
        ("1100000000000000001", "1200000000000000001", "👍\n"),
        (
            "1100000000000000001",
            "1200000000000000001",
            "party:not_a_number",
        ),
    ] {
        assert_eq!(
            run(guarded_api.add_reaction_empty_body(channel_id, message_id, emoji)),
            Err(ReactionError::InvalidArg),
            "должно отклоняться до HTTP: ({channel_id:?}, {message_id:?}, {emoji:?})"
        );
    }
    assert!(
        guarded.calls().is_empty(),
        "невалидные параметры не должны приводить к HTTP-вызовам"
    );
}

#[test]
fn add_reaction_empty_body_handles_204_and_maps_429_403_404_401_5xx_network() {
    let transport = MockReactionTransport::new(vec![
        Ok(http_response(204, &[], b"")),
        Ok(http_response(
            429,
            &[],
            br#"{"message":"You are being rate limited.","retry_after":1.5,"global":false}"#,
        )),
        Ok(http_response(429, &[("Retry-After", "2.5")], b"")),
        Ok(http_response(
            403,
            &[],
            br#"{"message":"Missing Permissions","code":50013}"#,
        )),
        Ok(http_response(
            404,
            &[],
            br#"{"message":"Unknown Message","code":10008}"#,
        )),
        Ok(http_response(
            401,
            &[],
            br#"{"message":"401: Unauthorized","code":0}"#,
        )),
        Ok(http_response(503, &[], b"Service Unavailable")),
        Err(ReactionError::Network),
    ]);
    let api = DiscordReactionApi::with_transport(transport);

    assert_eq!(
        run(api.add_reaction_empty_body("110", "120", "👍")),
        Ok(()),
        "204 No Content — успех"
    );
    assert_eq!(
        run(api.add_reaction_empty_body("110", "120", "👍")),
        Err(ReactionError::RateLimited {
            retry_after_ms: 1500
        })
    );
    assert_eq!(
        run(api.add_reaction_empty_body("110", "120", "👍")),
        Err(ReactionError::RateLimited {
            retry_after_ms: 2500
        })
    );
    assert_eq!(
        run(api.add_reaction_empty_body("110", "120", "👍")),
        Err(ReactionError::Forbidden)
    );
    assert_eq!(
        run(api.add_reaction_empty_body("110", "120", "👍")),
        Err(ReactionError::NotFound)
    );
    assert_eq!(
        run(api.add_reaction_empty_body("110", "120", "👍")),
        Err(ReactionError::Unauthorized)
    );
    assert_eq!(
        run(api.add_reaction_empty_body("110", "120", "👍")),
        Err(ReactionError::Server { status: 503 })
    );
    assert_eq!(
        run(api.add_reaction_empty_body("110", "120", "👍")),
        Err(ReactionError::Network)
    );
}

#[test]
fn add_reaction_and_toggle_send_empty_body_per_re_oracle_endpoint_38() {
    let transport = MockReactionTransport::new(vec![Ok(http_response(204, &[], b""))]);
    let api = DiscordReactionApi::with_transport(transport.clone());

    assert_eq!(
        run(api.add_reaction("1100000000000000001", "1200000000000000001", "👍")),
        Ok(())
    );
    let calls = transport.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].method, "PUT");
    assert_eq!(
        calls[0].body,
        Vec::<u8>::new(),
        "PUT /reactions/{{emoji}}/@me обязан слать 0 байт per rest-d02.md#38"
    );
}
