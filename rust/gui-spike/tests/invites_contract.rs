//! G-03: контракт invite — `GET /invites/{code}` (resolve) и
//! `POST /invites/{code}` (accept).
//!
//! **Оракул — отдельный `[S]`** (в frozen D-02 каталоге invite-ручек нет вообще):
//! - `waves/RW-40/PARITY/G6-REPORT-RW40-02.md` §3.3 — resolve/accept, тело
//!   `{session_id, invite_instance_id, received_installation_id}` (все nullable),
//!   ответ — invite-объект (`guild_id`, `target_channel_id`, `target_message_id`);
//! - `waves/RW-40/NETWORK-CONTRACTS.md` §1.2 — дословный call-site
//!   (`session_id` = `getSessionId()`, `received_installation_id` может быть `null`,
//!   `context` — analytics, **не** поле тела) и §1.4 — форма invite-объекта
//!   (`code`, `guild{id,name,icon}`, `channel{id}`, `approximate_member_count`,
//!   `approximate_presence_count`, `expires_at` nullable).
//!
//! HTTP-коды resolve/accept live не наблюдались (`[A]`), поэтому тесты фиксируют
//! только решённое: любой 2xx обязан принести разбираемый invite-объект, а
//! 401/403/404/429/прочие 4xx/5xx/сеть — типизированы. Тело и вид ошибки не
//! выдумываются.
//!
//! Тесты не ходят в сеть: транспорт подменён [`InviteTransport`], который
//! фиксирует target/тело. `routes::ROUTES` остаётся frozen D-02 (49 строк с POST
//! сообщения): invites подключены явным `[S]`-расширением каталога (D-108).

use discord_core::rest::routes::{self, HttpMethod};
use discord_core::rest::transport::HttpResponse;
use discord_gui_spike::invites::{
    accept_invite, resolve_invite, AcceptRequest, AcceptedInvite, DiscordInvites, InviteError,
    InvitePreview, InviteTransport, InvitesApi,
};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

const CODE: &str = "abc123";

/// Дословный `[S]`-оракул: путь шаблона (без `/api/v9`).
const ORACLE_PATH: &str = "/invites/{code}";

/// Дословные ключи тела accept (§1.2) — ровно три, в порядке call-site.
const ORACLE_ACCEPT_KEYS: [&str; 3] = [
    "session_id",
    "invite_instance_id",
    "received_installation_id",
];

/// Выдержка frozen D-02: 47 live-маршрутов. Invite-ручек в ней нет — это
/// доказывает, что контракт не подогнан под захват.
const D02_TSV: &str = include_str!("../../crates/discord-core/tests/data/rest-routes-47.tsv");

/// Исходный invite-объект (`[S]`, §1.4): вложенные `guild`/`channel`, snake_case.
const RESOLVE_BODY: &str = r#"{
    "code": "abc123",
    "type": 0,
    "guild": {"id": "111111111111111111", "name": "Тестовый сервер", "icon": "a1b2c3"},
    "channel": {"id": "222222222222222222", "name": "general", "type": 0},
    "approximate_member_count": 1500,
    "approximate_presence_count": 42,
    "expires_at": "2026-11-01T00:00:00+00:00"
}"#;

const ACCEPT_BODY: &[u8] = br#"{
    "code": "abc123",
    "guild_id": "111111111111111111",
    "target_channel_id": "222222222222222222",
    "target_message_id": "333333333333333333",
    "guild_scheduled_event": null
}"#;

/// Журнал фейкового транспорта: target и тело запроса.
type Posts = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

/// Транспорт-фейк: фиксирует GET/POST и отдаёт заранее заданный ответ/ошибку.
#[derive(Clone, Default)]
struct FakeTransport {
    status: u16,
    body: Vec<u8>,
    error: Option<InviteError>,
    gets: Arc<Mutex<Vec<String>>>,
    posts: Posts,
}

impl FakeTransport {
    fn answering(status: u16) -> Self {
        Self {
            status,
            body: Vec::new(),
            error: None,
            ..Self::default()
        }
    }

    fn with_body(status: u16, body: &[u8]) -> Self {
        Self {
            status,
            body: body.to_vec(),
            error: None,
            ..Self::default()
        }
    }

    fn failing(error: InviteError) -> Self {
        Self {
            status: 0,
            body: Vec::new(),
            error: Some(error),
            ..Self::default()
        }
    }

    fn gets(&self) -> Vec<String> {
        self.gets.lock().unwrap().clone()
    }

    fn posts(&self) -> Vec<(String, Vec<u8>)> {
        self.posts.lock().unwrap().clone()
    }
}

impl InviteTransport for FakeTransport {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, InviteError>> + Send + 'a>> {
        self.gets.lock().unwrap().push(target.to_string());
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

    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, InviteError>> + Send + 'a>> {
        self.posts
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

fn api(transport: &FakeTransport) -> DiscordInvites {
    DiscordInvites::with_transport(Arc::new(transport.clone()))
}

fn run<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(future)
}

fn expected_preview() -> InvitePreview {
    InvitePreview {
        code: "abc123".to_string(),
        guild_id: "111111111111111111".to_string(),
        guild_name: "Тестовый сервер".to_string(),
        icon: Some("a1b2c3".to_string()),
        member_count: Some(1500),
        presence_count: Some(42),
        expires_at: Some("2026-11-01T00:00:00+00:00".to_string()),
        channel_id: Some("222222222222222222".to_string()),
    }
}

#[test]
fn resolve_gets_invite_target_with_counts_and_expiration() {
    let transport = FakeTransport::with_body(200, RESOLVE_BODY.as_bytes());
    let client = api(&transport);

    assert_eq!(run(client.resolve(CODE)), Ok(expected_preview()));

    let gets = transport.gets();
    assert_eq!(
        gets,
        vec![format!(
            "/invites/{CODE}?with_counts=true&with_expiration=true"
        )],
        "target подтверждён якорем with_counts:!0,with_expiration:!0"
    );
    assert!(transport.posts().is_empty(), "resolve — только GET");
    assert!(
        !gets[0].contains("inputValue"),
        "inputValue — analytics-поле, не выдумываем: {}",
        gets[0]
    );
}

#[test]
fn resolve_maps_wire_object_to_camel_case_preview() {
    let transport = FakeTransport::with_body(200, RESOLVE_BODY.as_bytes());
    let client = api(&transport);

    let preview = run(resolve_invite(&client, CODE)).expect("resolve");
    assert_eq!(preview, expected_preview());
    assert_eq!(
        serde_json::to_value(&preview).expect("IPC"),
        serde_json::json!({
            "code": "abc123",
            "guildId": "111111111111111111",
            "guildName": "Тестовый сервер",
            "icon": "a1b2c3",
            "memberCount": 1500,
            "presenceCount": 42,
            "expiresAt": "2026-11-01T00:00:00+00:00",
            "channelId": "222222222222222222"
        }),
        "IPC-контракт §3.3 — camelCase"
    );

    // Минимальный объект: опциональные поля отсутствуют/null — не ошибка и не
    // сериализуются в IPC (UI не получает выдуманных значений).
    for body in [
        br#"{"code":"abc","guild":{"id":"1","name":"G"}}"#.as_slice(),
        br#"{"code":"abc","guild":{"id":"1","name":"G","icon":null},"channel":null,
             "approximate_member_count":null,"approximate_presence_count":null,
             "expires_at":null}"#
            .as_slice(),
    ] {
        let transport = FakeTransport::with_body(200, body);
        let client = api(&transport);
        let preview = run(client.resolve(CODE)).expect("минимальный invite-объект");
        assert_eq!(
            preview,
            InvitePreview {
                code: "abc".to_string(),
                guild_id: "1".to_string(),
                guild_name: "G".to_string(),
                icon: None,
                member_count: None,
                presence_count: None,
                expires_at: None,
                channel_id: None,
            },
            "{body:?}"
        );
        assert_eq!(
            serde_json::to_value(&preview).expect("IPC"),
            serde_json::json!({"code": "abc", "guildId": "1", "guildName": "G"}),
            "пустые опциональные поля не сериализуются"
        );
    }
}

#[test]
fn codes_are_normalized_and_invalid_ones_never_reach_transport() {
    let valid = [
        ("abc", "abc"),
        ("  abc  ", "abc"),
        ("abc\n\t", "abc"),
        ("abc/", "abc"),
        ("discord.gg/abc", "abc"),
        ("https://discord.gg/abc", "abc"),
        ("https://discord.gg/abc/", "abc"),
        ("https://discord.com/invite/AbC-12", "AbC-12"),
        ("https://discord.gg/abc?event=123", "abc"),
        ("https://discord.gg/abc#fragment", "abc"),
        ("AB", "AB"),
        ("a1-b2", "a1-b2"),
    ];
    for (input, expected) in valid {
        let transport = FakeTransport::with_body(200, RESOLVE_BODY.as_bytes());
        let client = api(&transport);
        assert_eq!(
            run(client.resolve(input)),
            Ok(expected_preview()),
            "вход {input:?} → код {expected:?}"
        );
        assert_eq!(
            transport.gets(),
            vec![format!(
                "/invites/{expected}?with_counts=true&with_expiration=true"
            )],
            "вход {input:?}"
        );
    }

    let too_long = "a".repeat(33);
    for input in [
        "",
        "   ",
        "a",
        too_long.as_str(),
        "ab cd",
        "abc!",
        "abc.def",
        "код",
        "abc{",
        "abc}",
        "?x=1",
        "https://discord.gg/",
        "///",
    ] {
        let transport = FakeTransport::with_body(200, RESOLVE_BODY.as_bytes());
        let client = api(&transport);
        assert_eq!(
            run(resolve_invite(&client, input)),
            Err(InviteError::InvalidArg),
            "resolve: вход {input:?}"
        );
        assert_eq!(
            run(accept_invite(&client, input, None, None, None)),
            Err(InviteError::InvalidArg),
            "accept: вход {input:?}"
        );
        assert!(
            transport.gets().is_empty(),
            "GET не должен уходить: {input:?}"
        );
        assert!(
            transport.posts().is_empty(),
            "POST не должен уходить: {input:?}"
        );
    }
}

#[test]
fn accept_posts_exactly_three_nullable_fields_without_context() {
    let transport = FakeTransport::with_body(200, ACCEPT_BODY);
    let client = api(&transport);

    let accepted = run(accept_invite(&client, CODE, None, None, None)).expect("accept");
    assert_eq!(
        accepted,
        AcceptedInvite {
            guild_id: "111111111111111111".to_string(),
            target_channel_id: Some("222222222222222222".to_string()),
            target_message_id: Some("333333333333333333".to_string()),
        }
    );

    let posts = transport.posts();
    assert_eq!(posts.len(), 1, "ровно один POST");
    let (target, body) = &posts[0];
    assert_eq!(target, &format!("/invites/{CODE}"), "путь без query");
    assert!(transport.gets().is_empty(), "accept — только POST");

    let value: serde_json::Value = serde_json::from_slice(body).expect("тело accept — JSON-объект");
    let object = value.as_object().expect("объект");
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    let mut expected = ORACLE_ACCEPT_KEYS.to_vec();
    expected.sort_unstable();
    assert_eq!(keys, expected, "ровно три поля тела: {value}");
    for key in ORACLE_ACCEPT_KEYS {
        assert!(value[key].is_null(), "{key} nullable: {value}");
    }
    assert!(
        !object.contains_key("context"),
        "context — analytics-объект, не поле тела: {value}"
    );
    assert!(
        !object.contains_key("code"),
        "код — в пути, не в теле: {value}"
    );

    // Заполненные значения уходят как есть (null → строка), состав полей тот же.
    let transport = FakeTransport::with_body(200, ACCEPT_BODY);
    let client = api(&transport);
    run(accept_invite(
        &client,
        CODE,
        Some("s1"),
        Some("i2"),
        Some("r3"),
    ))
    .expect("accept");
    let posts = transport.posts();
    let value: serde_json::Value = serde_json::from_slice(&posts[0].1).expect("JSON");
    assert_eq!(
        value,
        serde_json::json!({
            "session_id": "s1",
            "invite_instance_id": "i2",
            "received_installation_id": "r3"
        }),
        "поля тела дословно из §1.2"
    );
}

#[test]
fn accept_maps_response_to_typed_receipt() {
    let transport = FakeTransport::with_body(200, ACCEPT_BODY);
    let client = api(&transport);
    let accepted = run(client.accept(
        CODE,
        &AcceptRequest {
            session_id: None,
            invite_instance_id: None,
            received_installation_id: None,
        },
    ))
    .expect("accept");
    assert_eq!(
        serde_json::to_value(&accepted).expect("IPC"),
        serde_json::json!({
            "guildId": "111111111111111111",
            "targetChannelId": "222222222222222222",
            "targetMessageId": "333333333333333333"
        }),
        "IPC-контракт §3.3 — camelCase"
    );

    let transport = FakeTransport::with_body(200, br#"{"guild_id":"111111111111111111"}"#);
    let client = api(&transport);
    let minimal = run(accept_invite(&client, CODE, None, None, None)).expect("минимальный ответ");
    assert_eq!(
        minimal,
        AcceptedInvite {
            guild_id: "111111111111111111".to_string(),
            target_channel_id: None,
            target_message_id: None,
        }
    );
    assert_eq!(
        serde_json::to_value(&minimal).expect("IPC"),
        serde_json::json!({"guildId": "111111111111111111"}),
        "отсутствующие target-поля не сериализуются"
    );
}

#[test]
fn http_statuses_map_to_typed_errors() {
    let cases = [
        (401, InviteError::Unauthorized),
        (403, InviteError::Forbidden),
        (404, InviteError::NotFound),
        (429, InviteError::RateLimited { retry_after_ms: 0 }),
        (400, InviteError::Client { status: 400 }),
        (500, InviteError::Server { status: 500 }),
        (503, InviteError::Server { status: 503 }),
    ];
    for (status, expected) in cases {
        let transport = FakeTransport::answering(status);
        let client = api(&transport);
        assert_eq!(
            run(resolve_invite(&client, CODE)),
            Err(expected.clone()),
            "resolve HTTP {status}"
        );
        assert_eq!(
            run(accept_invite(&client, CODE, None, None, None)),
            Err(expected),
            "accept HTTP {status}"
        );
        assert_eq!(transport.gets().len(), 1, "HTTP {status}: GET ровно один");
        assert_eq!(transport.posts().len(), 1, "HTTP {status}: POST ровно один");
    }
}

#[test]
fn success_without_parsable_invite_object_is_invalid_response() {
    for body in [
        b"".as_slice(),
        b"  \r\n".as_slice(),
        br#"[]"#.as_slice(),
        br#"null"#.as_slice(),
        br#""oops""#.as_slice(),
        // guild обязателен: приглашение в группу — не серверный invite.
        br#"{"code":"abc"}"#.as_slice(),
        br#"{"code":"abc","guild":null}"#.as_slice(),
        br#"{"code":"abc","guild":{"id":"1"}}"#.as_slice(),
        br#"{"code":"abc","guild":{"name":"G"}}"#.as_slice(),
        br#"{"code":"abc","guild":{"id":"","name":"G"}}"#.as_slice(),
        br#"{"code":"abc","guild":{"id":"1","name":""}}"#.as_slice(),
        br#"{"code":"","guild":{"id":"1","name":"G"}}"#.as_slice(),
        br#"{"guild":{"id":"1","name":"G"}}"#.as_slice(),
        // channel присутствует, но без id — противоречивый объект.
        br#"{"code":"abc","guild":{"id":"1","name":"G"},"channel":{}}"#.as_slice(),
        // отрицательный счётчик и unix-время вместо ISO-8601 — не наш wire.
        br#"{"code":"abc","guild":{"id":"1","name":"G"},"approximate_member_count":-1}"#.as_slice(),
        br#"{"code":"abc","guild":{"id":"1","name":"G"},"expires_at":42}"#.as_slice(),
        br#"{"code":"abc","guild":{"id":"1","name":"G""#.as_slice(),
    ] {
        let transport = FakeTransport::with_body(200, body);
        let client = api(&transport);
        assert_eq!(
            run(resolve_invite(&client, CODE)),
            Err(InviteError::InvalidResponse),
            "resolve {body:?}"
        );
    }

    for body in [
        b"".as_slice(),
        br#"{}"#.as_slice(),
        br#"[]"#.as_slice(),
        br#"{"guild_id":""}"#.as_slice(),
        br#"{"guild_id":5}"#.as_slice(),
        br#"{"guild_id":null}"#.as_slice(),
        br#"{"guild_id":"1","target_channel_id":42}"#.as_slice(),
    ] {
        let transport = FakeTransport::with_body(200, body);
        let client = api(&transport);
        assert_eq!(
            run(accept_invite(&client, CODE, None, None, None)),
            Err(InviteError::InvalidResponse),
            "accept {body:?}"
        );
    }

    // 204 — 2xx без invite-объекта: успех не подтверждён, но и не сеть/403.
    let transport = FakeTransport::answering(204);
    let client = api(&transport);
    assert_eq!(
        run(resolve_invite(&client, CODE)),
        Err(InviteError::InvalidResponse)
    );
    assert_eq!(
        run(accept_invite(&client, CODE, None, None, None)),
        Err(InviteError::InvalidResponse)
    );
}

#[test]
fn transport_failure_is_typed_network_error() {
    let transport = FakeTransport::failing(InviteError::Network);
    let client = api(&transport);
    assert_eq!(
        run(resolve_invite(&client, CODE)),
        Err(InviteError::Network)
    );
    assert_eq!(
        run(accept_invite(&client, CODE, None, None, None)),
        Err(InviteError::Network)
    );
}

#[test]
fn empty_or_blank_token_is_missing_token() {
    for token in ["", "   ", "\t\n"] {
        assert_eq!(
            DiscordInvites::new(token).expect_err("пустой токен — ошибка"),
            InviteError::MissingToken
        );
    }
}

#[test]
fn client_debug_does_not_reveal_token() {
    let client = DiscordInvites::new("secret-token").expect("клиент");
    let text = format!("{client:?}");
    assert!(!text.contains("secret-token"), "{text}");
}

#[test]
fn errors_serialize_with_camel_case_kind_only() {
    let cases = [
        (InviteError::InvalidArg, "invalidArg"),
        (InviteError::MissingToken, "missingToken"),
        (InviteError::Unauthorized, "unauthorized"),
        (InviteError::Forbidden, "forbidden"),
        (InviteError::NotFound, "notFound"),
        (
            InviteError::RateLimited {
                retry_after_ms: 250,
            },
            "rateLimited",
        ),
        (InviteError::Client { status: 400 }, "client"),
        (InviteError::Server { status: 500 }, "server"),
        (InviteError::Network, "network"),
        (InviteError::Storage, "storage"),
        (InviteError::InvalidResponse, "invalidResponse"),
    ];
    for (error, kind) in cases {
        let value = serde_json::to_value(&error).expect("ошибка сериализуется");
        assert_eq!(value["kind"], kind, "{value}");
        let object = value.as_object().expect("объект ошибки");
        assert!(
            object
                .iter()
                .all(|(key, detail)| key == "kind" || detail.is_number()),
            "наружу уходят только kind и числовая деталь, без текста: {value}"
        );
        assert!(!error.to_string().is_empty(), "Display непустой: {error}");
    }
}

/// Фейковая реализация доменного контракта: проверяет делегирование и то, что
/// нормализация происходит до вызова API.
#[derive(Default)]
struct FakeInvitesApi {
    resolve_result: Option<Result<InvitePreview, InviteError>>,
    accept_result: Option<Result<AcceptedInvite, InviteError>>,
    codes: Mutex<Vec<String>>,
    requests: Mutex<Vec<AcceptRequest>>,
}

impl InvitesApi for FakeInvitesApi {
    fn resolve<'a>(
        &'a self,
        code: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<InvitePreview, InviteError>> + Send + 'a>> {
        self.codes.lock().unwrap().push(code.to_string());
        let result = self
            .resolve_result
            .clone()
            .unwrap_or(Err(InviteError::Network));
        Box::pin(async move { result })
    }

    fn accept<'a>(
        &'a self,
        code: &'a str,
        request: &'a AcceptRequest,
    ) -> Pin<Box<dyn Future<Output = Result<AcceptedInvite, InviteError>> + Send + 'a>> {
        self.codes.lock().unwrap().push(code.to_string());
        self.requests.lock().unwrap().push(request.clone());
        let result = self
            .accept_result
            .clone()
            .unwrap_or(Err(InviteError::Network));
        Box::pin(async move { result })
    }
}

#[test]
fn free_functions_normalize_and_delegate() {
    let fake = FakeInvitesApi {
        resolve_result: Some(Ok(expected_preview())),
        accept_result: Some(Ok(AcceptedInvite {
            guild_id: "111111111111111111".to_string(),
            target_channel_id: None,
            target_message_id: None,
        })),
        ..FakeInvitesApi::default()
    };

    assert_eq!(
        run(resolve_invite(&fake, "https://discord.gg/abc123/")),
        Ok(expected_preview())
    );
    assert_eq!(
        *fake.codes.lock().unwrap(),
        vec!["abc123".to_string()],
        "в API уходит нормализованный код, не ссылка"
    );

    let accepted = run(accept_invite(
        &fake,
        "  abc123 ",
        Some("s1"),
        None,
        Some("r3"),
    ))
    .expect("accept");
    assert_eq!(accepted.guild_id, "111111111111111111");
    assert_eq!(
        *fake.codes.lock().unwrap(),
        vec!["abc123".to_string(), "abc123".to_string()]
    );
    assert_eq!(
        *fake.requests.lock().unwrap(),
        vec![AcceptRequest {
            session_id: Some("s1".to_string()),
            invite_instance_id: None,
            received_installation_id: Some("r3".to_string()),
        }],
        "nullable-поля доходят без подстановок"
    );

    let failing = FakeInvitesApi {
        resolve_result: Some(Err(InviteError::NotFound)),
        accept_result: Some(Err(InviteError::RateLimited {
            retry_after_ms: 700,
        })),
        ..FakeInvitesApi::default()
    };
    assert_eq!(
        run(resolve_invite(&failing, CODE)),
        Err(InviteError::NotFound)
    );
    assert_eq!(
        run(accept_invite(&failing, CODE, None, None, None)),
        Err(InviteError::RateLimited {
            retry_after_ms: 700
        })
    );
}

/// Маршруты — отдельный `[S]`-контракт: в frozen D-02 их нет, но каталог
/// (`routes::find`) резолвит GET и POST, тело accept — дословно три ключа.
#[test]
fn invite_routes_are_s_catalog_extension_not_d02() {
    let resolve = routes::find(HttpMethod::Get, ORACLE_PATH)
        .expect("GET-маршрут invite зарегистрирован в каталоге");
    assert_eq!(resolve.method.as_str(), "GET");
    assert_eq!(resolve.path, ORACLE_PATH);
    assert!(resolve.body_keys.is_empty(), "у resolve тела нет");
    assert_eq!(routes::INVITE_PATH, ORACLE_PATH, "один источник пути");

    let accept = routes::find(HttpMethod::Post, ORACLE_PATH)
        .expect("POST-маршрут invite зарегистрирован в каталоге");
    assert_eq!(accept.method.as_str(), "POST");
    assert_eq!(accept.path, ORACLE_PATH);
    assert_eq!(accept.body_keys, ORACLE_ACCEPT_KEYS.as_slice());
    assert_eq!(routes::INVITE_ACCEPT_BODY_KEYS, ORACLE_ACCEPT_KEYS);

    assert_eq!(
        routes::render_path(resolve, &[CODE]).expect("рендер"),
        format!("/invites/{CODE}")
    );
    assert_eq!(
        routes::render_path(accept, &[CODE]).expect("рендер"),
        format!("/invites/{CODE}")
    );

    assert!(
        !D02_TSV.lines().any(|line| line.contains("/invites/")),
        "D-02 не подтверждает invite-ручки — оракул отдельный [S]"
    );
    assert!(
        !routes::ROUTES
            .iter()
            .any(|route| route.path.contains("/invites/")),
        "frozen каталог D-02 не расширяется"
    );
    assert_eq!(routes::ROUTES.len(), 49, "каталог остаётся 49 строк");

    // Маршрут ищется только по точной паре «метод, шаблон»; `{id}` — другой шаблон.
    assert_eq!(routes::find(HttpMethod::Get, "/invites/{id}"), None);
    assert_eq!(routes::find(HttpMethod::Put, ORACLE_PATH), None);
    assert_eq!(
        routes::find(HttpMethod::Get, "/api/v9/invites/{code}"),
        None
    );

    // Расширение typing (D-108) не потеряно при добавлении invites.
    assert_eq!(
        routes::find(HttpMethod::Post, routes::TYPING_PATH).map(|spec| spec.path),
        Some("/channels/{id}/typing")
    );
}

/// Тело `#[tauri::command]`-функции по имени: от `async fn name(` до следующей
/// `#[tauri::command]` или до конца файла.
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
fn tauri_invite_commands_read_bearer_through_token_or_without_logging() {
    let source = include_str!("../src/main.rs");
    let commands = registered_commands(source);
    for command in ["resolve_invite", "accept_invite"] {
        assert_eq!(
            commands.iter().filter(|name| **name == command).count(),
            1,
            "{command} регистрируется ровно один раз"
        );
        let body = command_body(source, command);
        assert!(
            body.contains("token_or::<InviteError"),
            "{command}: bearer читается только helper'ом token_or: {body}"
        );
        assert_eq!(
            body.matches("backend.token()").count(),
            1,
            "{command}: ровно одно чтение bearer из backend: {body}"
        );
        assert!(
            body.contains("InviteError::Storage"),
            "{command}: отказ keyring остаётся storage, а не сетью: {body}"
        );
        assert!(
            body.contains("InviteError::MissingToken"),
            "{command}: отсутствие токена — missingToken: {body}"
        );
        assert!(
            body.contains("DiscordInvites::new(&token)"),
            "{command}: токен уходит только в auth-заголовок клиента: {body}"
        );
        assert!(
            !body.contains("token: String") && !body.contains("bearer: String"),
            "{command}: токен не приходит из фронтенда: {body}"
        );
        assert!(
            !body.contains("println!") && !body.contains("eprintln!"),
            "{command}: токен и ответ не логируются: {body}"
        );
    }

    let accept = command_body(source, "accept_invite");
    for argument in [
        "session_id: Option<String>",
        "invite_instance_id: Option<String>",
        "received_installation_id: Option<String>",
    ] {
        assert!(
            accept.contains(argument),
            "accept: {argument} — nullable: {accept}"
        );
    }
    assert!(
        accept.contains("invites::accept_invite"),
        "accept: тело и запрос собирает модуль, не команда: {accept}"
    );

    let resolve = command_body(source, "resolve_invite");
    assert!(
        resolve.contains("InvitePreview"),
        "resolve: типизированный preview: {resolve}"
    );
}
