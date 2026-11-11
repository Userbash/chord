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
    accept_invite, can_user_create_channel_invite, create_channel_invite,
    create_channel_invite_guarded, resolve_invite, AcceptRequest, AcceptedInvite,
    CreateInviteParams, CreatedInvite, DiscordInvites, InviteError, InvitePreview, InviteTransport,
    InvitesApi, INVITE_FLAG_IS_GUEST, VALID_INVITE_MAX_AGES, VALID_INVITE_MAX_USES,
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
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    error: Option<InviteError>,
    gets: Arc<Mutex<Vec<String>>>,
    posts: Posts,
}

impl FakeTransport {
    fn answering(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
            error: None,
            ..Self::default()
        }
    }

    fn with_body(status: u16, body: &[u8]) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.to_vec(),
            error: None,
            ..Self::default()
        }
    }

    fn with_headers(mut self, headers: &[(&str, &str)]) -> Self {
        self.headers = headers
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect();
        self
    }

    fn failing(error: InviteError) -> Self {
        Self {
            status: 0,
            headers: Vec::new(),
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
                headers: self.headers.clone(),
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
                headers: self.headers.clone(),
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
        (InviteError::InvalidRequest, "invalidRequest"),
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

// ---------------------------------------------------------------------------
// STUB-05-INVITES-BACKEND: создание приглашения на канал
// `POST /channels/{channel_id}/invites` (RE-DEEP-REA §4.3, INVITES-SPEC §1.2).
// ---------------------------------------------------------------------------

const CREATE_CHANNEL_ID: &str = "1014537851418980375";

const CREATE_RESPONSE_BODY: &str = r#"{
    "code": "aBcD123XyZ",
    "type": 0,
    "expires_at": "2026-10-31T12:00:00.000Z",
    "guild": { "id": "1014537851418980372", "name": "ITSKIDON PC BOOST", "icon": "icon_hash" },
    "channel": { "id": "1014537851418980375", "name": "новости", "type": 0 },
    "max_age": 86400,
    "max_uses": 0,
    "temporary": false,
    "uses": 0,
    "flags": 0
}"#;

#[test]
fn create_invite_oracle_constants_and_defaults_match_re() {
    assert_eq!(
        VALID_INVITE_MAX_AGES,
        [0, 1800, 3600, 21600, 43200, 86400, 604800]
    );
    assert_eq!(VALID_INVITE_MAX_USES, [0, 1, 5, 10, 25, 50, 100]);
    assert_eq!(INVITE_FLAG_IS_GUEST, 1 << 0);

    let defaults = CreateInviteParams::default();
    assert_eq!(
        defaults,
        CreateInviteParams {
            max_age: 86400,
            max_uses: 0,
            temporary: false,
            flags: None,
        }
    );
}

#[test]
fn create_invite_posts_to_channel_invites_with_params_and_parses_created_invite() {
    let transport = FakeTransport::with_body(200, CREATE_RESPONSE_BODY.as_bytes());
    let params = CreateInviteParams::default();

    let created = run(create_channel_invite(
        &transport,
        CREATE_CHANNEL_ID,
        &params,
    ))
    .expect("create_invite");
    assert_eq!(
        created,
        CreatedInvite {
            code: "aBcD123XyZ".to_string(),
            max_age: 86400,
            max_uses: 0,
            temporary: false,
        }
    );
    assert_eq!(
        serde_json::to_value(&created).expect("IPC"),
        serde_json::json!({
            "code": "aBcD123XyZ",
            "maxAge": 86400,
            "maxUses": 0,
            "temporary": false
        })
    );

    let posts = transport.posts();
    assert_eq!(posts.len(), 1, "ровно один POST");
    let (target, body) = &posts[0];
    assert_eq!(
        target,
        &format!("/channels/{CREATE_CHANNEL_ID}/invites"),
        "маршрут INSTANT_INVITES"
    );
    assert!(transport.gets().is_empty(), "только POST");

    let value: serde_json::Value = serde_json::from_slice(body).expect("JSON тело");
    assert_eq!(
        value,
        serde_json::json!({
            "max_age": 86400,
            "max_uses": 0,
            "temporary": false
        }),
        "без flags при flags=None"
    );

    // С гостевым флагом и временным членством:
    let guest_response = br#"{
        "code": "Guest99",
        "max_age": 1800,
        "max_uses": 5,
        "temporary": true
    }"#;
    let transport_guest = FakeTransport::with_body(201, guest_response);
    let client = api(&transport_guest);
    let guest_params = CreateInviteParams {
        max_age: 1800,
        max_uses: 5,
        temporary: true,
        flags: Some(INVITE_FLAG_IS_GUEST),
    };
    let created_guest =
        run(client.create_invite(CREATE_CHANNEL_ID, &guest_params)).expect("create_invite method");
    assert_eq!(
        created_guest,
        CreatedInvite {
            code: "Guest99".to_string(),
            max_age: 1800,
            max_uses: 5,
            temporary: true,
        }
    );
    let posts_guest = transport_guest.posts();
    assert_eq!(posts_guest.len(), 1);
    let guest_json: serde_json::Value =
        serde_json::from_slice(&posts_guest[0].1).expect("JSON тело с flags");
    assert_eq!(
        guest_json,
        serde_json::json!({
            "max_age": 1800,
            "max_uses": 5,
            "temporary": true,
            "flags": 1
        })
    );
}

#[test]
fn create_invite_accepts_every_table_entry_for_max_age_and_max_uses() {
    for max_age in VALID_INVITE_MAX_AGES {
        let body = format!(
            r#"{{"code":"age{max_age}","max_age":{max_age},"max_uses":0,"temporary":false}}"#
        );
        let transport = FakeTransport::with_body(200, body.as_bytes());
        let params = CreateInviteParams {
            max_age,
            ..CreateInviteParams::default()
        };
        let created =
            run(create_channel_invite(&transport, "news", &params)).unwrap_or_else(|err| {
                panic!("max_age={max_age} из таблицы RE обязан проходить: {err}")
            });
        assert_eq!(created.max_age, max_age);
        assert_eq!(transport.posts().len(), 1);
    }

    for max_uses in VALID_INVITE_MAX_USES {
        let body = format!(
            r#"{{"code":"uses{max_uses}","max_age":86400,"max_uses":{max_uses},"temporary":false}}"#
        );
        let transport = FakeTransport::with_body(200, body.as_bytes());
        let params = CreateInviteParams {
            max_uses,
            ..CreateInviteParams::default()
        };
        let created = run(create_channel_invite(&transport, "c1", &params)).unwrap_or_else(|err| {
            panic!("max_uses={max_uses} из таблицы RE обязан проходить: {err}")
        });
        assert_eq!(created.max_uses, max_uses);
        assert_eq!(transport.posts().len(), 1);
    }
}

#[test]
fn create_invite_rejects_unsafe_channel_id_before_network() {
    use discord_gui_spike::messages::{send_message, MessageError, MessageReceipt, MessageSender};

    #[derive(Default)]
    struct DummyMessageSender;
    impl MessageSender for DummyMessageSender {
        fn send_message<'a>(
            &'a self,
            channel_id: &'a str,
            content: &'a str,
            nonce: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<MessageReceipt, MessageError>> + Send + 'a>>
        {
            let receipt = MessageReceipt {
                id: "1".to_string(),
                channel_id: channel_id.to_string(),
                content: content.to_string(),
                nonce: nonce.to_string(),
            };
            Box::pin(async move { Ok(receipt) })
        }
    }

    let transport = FakeTransport::with_body(200, CREATE_RESPONSE_BODY.as_bytes());
    let client = api(&transport);
    let params = CreateInviteParams::default();
    let too_long = "1".repeat(65);

    for bad_channel in [
        "",
        "   ",
        "../..",
        "../../channels/1/invites",
        "123/456",
        "123?x=1",
        "123#frag",
        "123 456",
        "123\n456",
        "%2e%2e",
        "chan.1",
        "канал",
        too_long.as_str(),
    ] {
        assert_eq!(
            run(create_channel_invite(&transport, bad_channel, &params)),
            Err(InviteError::InvalidRequest),
            "free fn: bad_channel={bad_channel:?}"
        );
        assert_eq!(
            run(client.create_invite(bad_channel, &params)),
            Err(InviteError::InvalidRequest),
            "method: bad_channel={bad_channel:?}"
        );
    }
    assert!(
        transport.posts().is_empty(),
        "невалидный channel_id не должен доходить до транспорта"
    );

    // Сверка allowlist с независимым модулем messages::send_message:
    let sender = DummyMessageSender;
    for sample in [
        CREATE_CHANNEL_ID,
        "news",
        "c1",
        "chan_1-A",
        "",
        "../..",
        "a.b",
        "a/b",
        "a b",
    ] {
        let invite_err = run(create_channel_invite(&transport, sample, &params)).is_err();
        let msg_err = run(send_message(&sender, sample, "hi", "n")).is_err();
        assert_eq!(
            invite_err, msg_err,
            "allowlist channel_id должен совпадать с messages для {sample:?}"
        );
    }
}

#[test]
fn create_invite_rejects_unlisted_max_age_and_max_uses_with_invalid_request_before_network() {
    let transport = FakeTransport::with_body(200, CREATE_RESPONSE_BODY.as_bytes());
    let client = api(&transport);

    for bad_age in [
        1,
        60,
        1799,
        1801,
        7200,
        28800,
        86401,
        604801,
        1209600,
        u32::MAX,
    ] {
        let params = CreateInviteParams {
            max_age: bad_age,
            ..CreateInviteParams::default()
        };
        assert_eq!(
            run(create_channel_invite(
                &transport,
                CREATE_CHANNEL_ID,
                &params
            )),
            Err(InviteError::InvalidRequest),
            "free fn: bad max_age={bad_age}"
        );
        assert_eq!(
            run(client.create_invite(CREATE_CHANNEL_ID, &params)),
            Err(InviteError::InvalidRequest),
            "method: bad max_age={bad_age}"
        );
    }

    for bad_uses in [2, 3, 4, 6, 9, 11, 24, 26, 49, 51, 99, 101, 1000, u32::MAX] {
        let params = CreateInviteParams {
            max_uses: bad_uses,
            ..CreateInviteParams::default()
        };
        assert_eq!(
            run(create_channel_invite(
                &transport,
                CREATE_CHANNEL_ID,
                &params
            )),
            Err(InviteError::InvalidRequest),
            "free fn: bad max_uses={bad_uses}"
        );
        assert_eq!(
            run(client.create_invite(CREATE_CHANNEL_ID, &params)),
            Err(InviteError::InvalidRequest),
            "method: bad max_uses={bad_uses}"
        );
    }

    assert!(
        transport.posts().is_empty(),
        "предсетевая проверка обязана отсекать невалидные max_age/max_uses до сети"
    );
}

#[test]
fn create_invite_maps_429_retry_after_and_403_401_errors() {
    let params = CreateInviteParams::default();

    // 429 с заголовком Retry-After (секунды -> мс)
    for (header_name, header_val, expected_ms) in [
        ("retry-after", "4", 4000_u64),
        ("Retry-After", "1.5", 1500_u64),
    ] {
        let transport = FakeTransport::answering(429).with_headers(&[(header_name, header_val)]);
        let client = api(&transport);
        assert_eq!(
            run(client.create_invite(CREATE_CHANNEL_ID, &params)),
            Err(InviteError::RateLimited {
                retry_after_ms: expected_ms
            }),
            "429 header {header_name}={header_val}"
        );
    }

    // 429 с телом {"retry_after": ...} при отсутствии заголовка
    let transport_body_429 = FakeTransport::with_body(
        429,
        br#"{"message":"You are being rate limited.","retry_after":2.5}"#,
    );
    assert_eq!(
        run(create_channel_invite(
            &transport_body_429,
            CREATE_CHANNEL_ID,
            &params
        )),
        Err(InviteError::RateLimited {
            retry_after_ms: 2500
        })
    );

    // 429 без заголовка и тела -> retry_after_ms: 0
    let transport_bare_429 = FakeTransport::answering(429);
    assert_eq!(
        run(create_channel_invite(
            &transport_bare_429,
            CREATE_CHANNEL_ID,
            &params
        )),
        Err(InviteError::RateLimited { retry_after_ms: 0 })
    );

    // 403 / 401 / 404 / 500
    for (status, expected) in [
        (401, InviteError::Unauthorized),
        (403, InviteError::Forbidden),
        (404, InviteError::NotFound),
        (400, InviteError::Client { status: 400 }),
        (500, InviteError::Server { status: 500 }),
    ] {
        let transport = FakeTransport::answering(status);
        let client = api(&transport);
        assert_eq!(
            run(client.create_invite(CREATE_CHANNEL_ID, &params)),
            Err(expected),
            "HTTP {status}"
        );
    }

    // Malformed 200 ответы -> InvalidResponse
    for bad_body in [
        b"".as_slice(),
        br#"{}"#.as_slice(),
        br#"{"code":"","max_age":86400,"max_uses":0,"temporary":false}"#.as_slice(),
        br#"{"code":"abc","max_uses":0,"temporary":false}"#.as_slice(),
        br#"{"code":"abc","max_age":86400,"temporary":false}"#.as_slice(),
        br#"{"code":"abc","max_age":86400,"max_uses":0}"#.as_slice(),
    ] {
        let transport = FakeTransport::with_body(200, bad_body);
        assert_eq!(
            run(create_channel_invite(
                &transport,
                CREATE_CHANNEL_ID,
                &params
            )),
            Err(InviteError::InvalidResponse),
            "bad_body={bad_body:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// RW-40 / task-228 (BE-P1-21): Rust Guild Invites & Vanity Code BFF Contract
// ---------------------------------------------------------------------------

use discord_gui_spike::invites::{
    build_create_channel_invite_route_and_body, build_invite_resolve_route, validate_invite_code,
    CreateChannelInviteRequest, ALLOWED_INVITE_MAX_AGES, ALLOWED_INVITE_MAX_USES,
};

#[test]
fn be_p1_21_allowed_invite_constants_match_contract() {
    assert_eq!(
        ALLOWED_INVITE_MAX_AGES,
        [0, 1800, 3600, 21600, 43200, 86400, 604800, 2592000]
    );
    assert_eq!(ALLOWED_INVITE_MAX_USES, [0, 1, 5, 10, 25, 100]);
}

#[test]
fn be_p1_21_create_channel_invite_request_serde_roundtrip() {
    let req = CreateChannelInviteRequest {
        channel_id: "1014537851418980375".to_string(),
        max_age: 86400,
        max_uses: 10,
        temporary: true,
        unique: false,
    };
    let json = serde_json::to_value(&req).expect("serialize CreateChannelInviteRequest");
    assert_eq!(
        json,
        serde_json::json!({
            "channel_id": "1014537851418980375",
            "max_age": 86400,
            "max_uses": 10,
            "temporary": true,
            "unique": false
        })
    );
    let decoded: CreateChannelInviteRequest =
        serde_json::from_value(json).expect("deserialize CreateChannelInviteRequest");
    assert_eq!(decoded, req);
}

#[test]
fn be_p1_21_validate_invite_code_trims_and_enforces_2_to_32_ascii_alnum_dash_underscore() {
    let min_code = "ab";
    let max_code = "A1_b2-C3_d4-E5_f6-G7_h8-I9_j0-K1";
    assert_eq!(max_code.len(), 32);

    for (raw, expected) in [
        ("ab", "ab"),
        ("  discord-lab_2026  ", "discord-lab_2026"),
        ("\t\nvanity_CODE-99\r\n", "vanity_CODE-99"),
        ("__", "__"),
        ("--", "--"),
        ("_-", "_-"),
        (min_code, min_code),
        (max_code, max_code),
    ] {
        assert_eq!(
            validate_invite_code(raw),
            Ok(expected),
            "validate_invite_code({raw:?})"
        );
    }

    let len_33 = "a".repeat(33);
    for invalid in [
        "",
        "   ",
        "a",
        " a ",
        len_33.as_str(),
        "ab cd",
        "abc/def",
        "abc?x=1",
        "abc#frag",
        "abc.def",
        "abc!",
        "инвайт",
        "https://discord.gg/abc",
    ] {
        assert!(
            validate_invite_code(invalid).is_err(),
            "validate_invite_code({invalid:?}) должен возвращать Err"
        );
    }
}

#[test]
fn be_p1_21_build_invite_resolve_route_formats_query_and_propagates_code_validation() {
    assert_eq!(
        build_invite_resolve_route("  rust_guild-1  ", true, true),
        Ok("/invites/rust_guild-1?with_counts=true&with_expiration=true".to_string())
    );
    assert_eq!(
        build_invite_resolve_route("ab", false, true),
        Ok("/invites/ab?with_counts=false&with_expiration=true".to_string())
    );
    assert_eq!(
        build_invite_resolve_route("VANITY_99", true, false),
        Ok("/invites/VANITY_99?with_counts=true&with_expiration=false".to_string())
    );
    assert_eq!(
        build_invite_resolve_route("a_b-c", false, false),
        Ok("/invites/a_b-c?with_counts=false&with_expiration=false".to_string())
    );

    for bad_code in ["", "x", "bad/code", "bad?query", &"z".repeat(33)] {
        assert!(
            build_invite_resolve_route(bad_code, true, true).is_err(),
            "build_invite_resolve_route({bad_code:?}) должен отклонять невалидный код"
        );
    }
}

#[test]
fn be_p1_21_build_create_channel_invite_route_and_body_validates_snowflake_and_tables() {
    let u64_max_str = u64::MAX.to_string();

    for channel_id in ["1", "1014537851418980375", u64_max_str.as_str()] {
        for max_age in ALLOWED_INVITE_MAX_AGES {
            for max_uses in ALLOWED_INVITE_MAX_USES {
                let req = CreateChannelInviteRequest {
                    channel_id: channel_id.to_string(),
                    max_age,
                    max_uses,
                    temporary: max_age == 1800,
                    unique: max_uses == 1,
                };
                let (route, body) = build_create_channel_invite_route_and_body(&req)
                    .unwrap_or_else(|e| panic!("ожидался Ok для {req:?}: {e}"));
                assert_eq!(route, format!("/channels/{channel_id}/invites"));
                assert_eq!(
                    body,
                    serde_json::json!({
                        "max_age": max_age,
                        "max_uses": max_uses,
                        "temporary": req.temporary,
                        "unique": req.unique
                    })
                );
            }
        }
    }

    // Невалидные Snowflake channel_id (0, ведущие нули, не-цифры, > u64::MAX, пустой)
    for bad_channel_id in [
        "",
        "0",
        "00",
        "01",
        "01014537851418980375",
        "-1",
        "123abc",
        "123 456",
        " 123456 ",
        "123/456",
        "18446744073709551616", // u64::MAX + 1
    ] {
        let req = CreateChannelInviteRequest {
            channel_id: bad_channel_id.to_string(),
            max_age: 86400,
            max_uses: 0,
            temporary: false,
            unique: false,
        };
        assert!(
            build_create_channel_invite_route_and_body(&req).is_err(),
            "bad_channel_id={bad_channel_id:?} должен отклоняться"
        );
    }

    // Невалидные max_age (вне ALLOWED_INVITE_MAX_AGES)
    for bad_age in [
        1,
        60,
        1799,
        1801,
        3599,
        7200,
        86399,
        604801,
        2591999,
        2592001,
        u32::MAX,
    ] {
        let req = CreateChannelInviteRequest {
            channel_id: "1014537851418980375".to_string(),
            max_age: bad_age,
            max_uses: 0,
            temporary: false,
            unique: false,
        };
        assert!(
            build_create_channel_invite_route_and_body(&req).is_err(),
            "bad_age={bad_age} должен отклоняться"
        );
    }

    // Невалидные max_uses (включая 50, которого нет в ALLOWED_INVITE_MAX_USES = [0, 1, 5, 10, 25, 100])
    for bad_uses in [2, 3, 4, 6, 9, 11, 24, 26, 50, 99, 101, u32::MAX] {
        let req = CreateChannelInviteRequest {
            channel_id: "1014537851418980375".to_string(),
            max_age: 86400,
            max_uses: bad_uses,
            temporary: false,
            unique: false,
        };
        assert!(
            build_create_channel_invite_invite_check_bad_uses(&req),
            "bad_uses={bad_uses} должен отклоняться"
        );
    }
}

fn build_create_channel_invite_invite_check_bad_uses(
    req: &discord_gui_spike::invites::CreateChannelInviteRequest,
) -> bool {
    discord_gui_spike::invites::build_create_channel_invite_route_and_body(req).is_err()
}

#[test]
fn accept_decodes_nested_guild_and_channel_when_top_level_guild_id_is_absent() {
    let real_discord_accept_body = br#"{
        "type": 0,
        "code": "eSrxp2YGUC",
        "guild": {
            "id": "274093052673392641",
            "name": "ITSKIDON PC BOOST",
            "icon": "a_123456"
        },
        "channel": {
            "id": "274093052673392642",
            "name": "welcome",
            "type": 0
        },
        "new_member": true
    }"#;
    let transport = FakeTransport::with_body(200, real_discord_accept_body);
    let client = api(&transport);
    let accepted = run(accept_invite(&client, "eSrxp2YGUC", None, None, None))
        .expect("POST /invites/{code} с вложенными guild.id и channel.id должен декодироваться");
    assert_eq!(accepted.guild_id, "274093052673392641");
    assert_eq!(
        accepted.target_channel_id.as_deref(),
        Some("274093052673392642")
    );
}

#[test]
fn accept_maps_http_400_hcaptcha_challenge_to_captcha_required_error() {
    let challenge_body = br#"{
        "code": 40003,
        "captcha_key": ["You need to update your app to join this server."],
        "captcha_sitekey": "a9b5fb07-92ff-493f-86fe-352a2803b3df",
        "captcha_service": "hcaptcha",
        "captcha_rqdata": "rqdata_payload_610",
        "captcha_rqtoken": "rqtoken_payload_610",
        "captcha_session_id": "sess_payload_610"
    }"#;
    let transport = FakeTransport::with_body(400, challenge_body);
    let client = api(&transport);
    let err = run(accept_invite(&client, CODE, None, None, None)).expect_err(
        "HTTP 400 с hCaptcha-челленджем должен возвращать InviteError::CaptchaRequired",
    );

    assert_eq!(
        err,
        InviteError::CaptchaRequired {
            code: 40003,
            captcha_service: "hcaptcha".to_string(),
            captcha_sitekey: "a9b5fb07-92ff-493f-86fe-352a2803b3df".to_string(),
            captcha_rqdata: Some("rqdata_payload_610".to_string()),
            captcha_rqtoken: Some("rqtoken_payload_610".to_string()),
            captcha_session_id: Some("sess_payload_610".to_string()),
        }
    );

    let serialized = serde_json::to_value(&err).expect("сериализация CaptchaRequired");
    assert_eq!(serialized["kind"], "captchaRequired");
    assert_eq!(serialized["code"], 40003);
    assert_eq!(serialized["captcha_service"], "hcaptcha");
    assert_eq!(
        serialized["captcha_sitekey"],
        "a9b5fb07-92ff-493f-86fe-352a2803b3df"
    );
    assert_eq!(serialized["captcha_rqdata"], "rqdata_payload_610");
    assert_eq!(serialized["captcha_rqtoken"], "rqtoken_payload_610");
    assert_eq!(serialized["captcha_session_id"], "sess_payload_610");
}

#[test]
fn accept_invite_with_captcha_sends_captcha_key_and_decodes_accepted_guild() {
    use discord_gui_spike::invites::{accept_invite_with_captcha, CaptchaSolution};

    let transport = FakeTransport::with_body(200, ACCEPT_BODY);
    let client = api(&transport);
    let solution = CaptchaSolution {
        captcha_key: "P1_eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.solution_610".to_string(),
        captcha_rqtoken: Some("rqtoken_payload_610".to_string()),
        captcha_session_id: Some("sess_payload_610".to_string()),
    };

    let accepted = run(accept_invite_with_captcha(
        &client,
        CODE,
        Some("sess_gw_1"),
        None,
        None,
        &solution,
    ))
    .expect("повторный вызов accept_invite_with_captcha с токеном решения должен пройти");

    assert_eq!(accepted.guild_id, "111111111111111111");
    let posts = transport.posts();
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].0, format!("/invites/{CODE}"));

    let sent_json: serde_json::Value =
        serde_json::from_slice(&posts[0].1).expect("JSON тело POST /invites/{code}");
    assert_eq!(
        sent_json["captcha_key"],
        "P1_eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.solution_610"
    );
    assert_eq!(sent_json["captcha_rqtoken"], "rqtoken_payload_610");
    assert_eq!(sent_json["captcha_session_id"], "sess_payload_610");
    assert_eq!(sent_json["session_id"], "sess_gw_1");

    // Пустой captcha_key отклоняется до обращения к сети
    let empty_solution = CaptchaSolution {
        captcha_key: "   ".to_string(),
        captcha_rqtoken: None,
        captcha_session_id: None,
    };
    assert_eq!(
        run(accept_invite_with_captcha(
            &client,
            CODE,
            None,
            None,
            None,
            &empty_solution,
        )),
        Err(InviteError::InvalidArg)
    );
    assert_eq!(
        transport.posts().len(),
        1,
        "пустой captcha_key не должен отправлять HTTP-запрос"
    );
}

/// TASK-809: Проверка битовой маски прав CREATE_INSTANT_INVITE перед созданием инвайта.
/// Предотвращает ошибки 404/notFound и исключает лишние сетевые запросы при отсутствии прав.
#[test]
fn be_p2_task_809_create_invite_permission_guards_and_missing_permissions_error() {
    use discord_core::permissions::{ADMINISTRATOR, CREATE_INSTANT_INVITE, VIEW_CHANNEL};

    // 1. Проверка чистой функции can_user_create_channel_invite
    assert!(can_user_create_channel_invite(CREATE_INSTANT_INVITE));
    assert!(can_user_create_channel_invite(ADMINISTRATOR));
    assert!(can_user_create_channel_invite(
        CREATE_INSTANT_INVITE | VIEW_CHANNEL
    ));
    assert!(can_user_create_channel_invite(u64::MAX));
    assert!(!can_user_create_channel_invite(0));
    assert!(!can_user_create_channel_invite(VIEW_CHANNEL));

    let params = CreateInviteParams::default();

    // 2. Без прав: запрос отклоняется до обращения к сети с InviteError::MissingPermissions
    let transport = FakeTransport::with_body(200, CREATE_RESPONSE_BODY.as_bytes());
    let res = run(create_channel_invite_guarded(
        &transport,
        CREATE_CHANNEL_ID,
        &params,
        Some(VIEW_CHANNEL),
    ));
    assert_eq!(res, Err(InviteError::MissingPermissions));
    assert_eq!(
        transport.posts.lock().unwrap().len(),
        0,
        "сеть не должна вызываться без прав"
    );

    // 3. С правами CREATE_INSTANT_INVITE: запрос доходит до транспорта
    let res = run(create_channel_invite_guarded(
        &transport,
        CREATE_CHANNEL_ID,
        &params,
        Some(CREATE_INSTANT_INVITE | VIEW_CHANNEL),
    ));
    assert!(res.is_ok());
    assert_eq!(res.unwrap().code, "aBcD123XyZ");
    assert_eq!(transport.posts.lock().unwrap().len(), 1);
}
