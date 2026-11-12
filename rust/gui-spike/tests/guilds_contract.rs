//! Контракт входа/выхода сервера: G-03a (`PUT /guilds/{id}/members/@me`) и
//! G-04 (`DELETE /users/@me/guilds/{id}`).
//!
//! Оракул входа: `waves/RW-40/NETWORK-CONTRACTS.md` §1.1 #2 (`D02:101,138,225`) —
//! `PUT /guilds/{id}/members/@me?lurker=true`, тело ровно `{}`, ответ **204**.
//!
//! Оракул выхода: §1.1 #4 (`B` anchor `async leaveGuild(e)`) —
//! `DELETE /users/@me/guilds/{id}`, тело `{lurking: bool}` из call-site
//! (`isLurking(id) || isCurrentUserGuest(id)`), ответ не читается; HTTP-коды там
//! **[A]**, поэтому тесты фиксируют только решённое контрактом: 2xx — успех,
//! 401/403/404/429 — типизированы, тело ошибки не выдумывается.
//!
//! Тесты не ходят в сеть: транспорт подменяется фейком, который записывает
//! target и тело. Токен в тестах фиктивный и не должен попадать в `Debug`.

use discord_core::rest::transport::HttpResponse;
use chord_client::guilds::{
    create_guild, enter_guild, join_guild, leave_guild, CreateGuildError, CreateGuildPayload,
    DiscordGuilds, EnterGuildError, GuildLeaving, GuildMembership, GuildTransport, LeaveGuildError,
};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

const GUILD_ID: &str = "123456789012345678";
const EXPECTED_TARGET: &str = "/guilds/123456789012345678/members/@me?lurker=true";
const LEAVE_TARGET: &str = "/users/@me/guilds/123456789012345678";

/// Транспорт-фейк: фиксирует вызовы и отдаёт заранее заданный ответ/ошибку.
#[derive(Default)]
struct FakeTransport {
    status: u16,
    error: Option<EnterGuildError>,
    leave_error: Option<LeaveGuildError>,
    post_error: Option<CreateGuildError>,
    post_body: Option<Vec<u8>>,
    calls: Mutex<Vec<(String, Vec<u8>)>>,
    deletes: Mutex<Vec<(String, Vec<u8>)>>,
    posts: Mutex<Vec<(String, Vec<u8>)>>,
}

impl FakeTransport {
    fn answering(status: u16) -> Arc<Self> {
        Arc::new(Self {
            status,
            error: None,
            leave_error: None,
            post_error: None,
            post_body: None,
            calls: Mutex::new(Vec::new()),
            deletes: Mutex::new(Vec::new()),
            posts: Mutex::new(Vec::new()),
        })
    }

    fn answering_post(status: u16, body: &[u8]) -> Arc<Self> {
        Arc::new(Self {
            status,
            error: None,
            leave_error: None,
            post_error: None,
            post_body: Some(body.to_vec()),
            calls: Mutex::new(Vec::new()),
            deletes: Mutex::new(Vec::new()),
            posts: Mutex::new(Vec::new()),
        })
    }

    fn failing(error: EnterGuildError) -> Arc<Self> {
        Arc::new(Self {
            status: 0,
            error: Some(error),
            leave_error: None,
            post_error: None,
            post_body: None,
            calls: Mutex::new(Vec::new()),
            deletes: Mutex::new(Vec::new()),
            posts: Mutex::new(Vec::new()),
        })
    }

    fn failing_leave(error: LeaveGuildError) -> Arc<Self> {
        Arc::new(Self {
            status: 0,
            error: None,
            leave_error: Some(error),
            post_error: None,
            post_body: None,
            calls: Mutex::new(Vec::new()),
            deletes: Mutex::new(Vec::new()),
            posts: Mutex::new(Vec::new()),
        })
    }

    fn failing_post(error: CreateGuildError) -> Arc<Self> {
        Arc::new(Self {
            status: 0,
            error: None,
            leave_error: None,
            post_error: Some(error),
            post_body: None,
            calls: Mutex::new(Vec::new()),
            deletes: Mutex::new(Vec::new()),
            posts: Mutex::new(Vec::new()),
        })
    }

    fn calls(&self) -> Vec<(String, Vec<u8>)> {
        self.calls.lock().unwrap().clone()
    }

    fn deletes(&self) -> Vec<(String, Vec<u8>)> {
        self.deletes.lock().unwrap().clone()
    }

    fn posts(&self) -> Vec<(String, Vec<u8>)> {
        self.posts.lock().unwrap().clone()
    }
}

impl GuildTransport for FakeTransport {
    fn put<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, EnterGuildError>> + Send + 'a>> {
        self.calls
            .lock()
            .unwrap()
            .push((target.to_string(), body.to_vec()));
        let answer = match &self.error {
            Some(error) => Err(error.clone()),
            None => Ok(HttpResponse {
                status: self.status,
                headers: Vec::new(),
                body: Vec::new(),
            }),
        };
        Box::pin(async move { answer })
    }

    fn delete<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, LeaveGuildError>> + Send + 'a>> {
        self.deletes
            .lock()
            .unwrap()
            .push((target.to_string(), body.to_vec()));
        let answer = match &self.leave_error {
            Some(error) => Err(error.clone()),
            None => Ok(HttpResponse {
                status: self.status,
                headers: Vec::new(),
                body: Vec::new(),
            }),
        };
        Box::pin(async move { answer })
    }

    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, CreateGuildError>> + Send + 'a>> {
        self.posts
            .lock()
            .unwrap()
            .push((target.to_string(), body.to_vec()));
        let answer = match &self.post_error {
            Some(error) => Err(error.clone()),
            None => Ok(HttpResponse {
                status: self.status,
                headers: Vec::new(),
                body: self.post_body.clone().unwrap_or_default(),
            }),
        };
        Box::pin(async move { answer })
    }
}

fn api(transport: Arc<dyn GuildTransport>) -> DiscordGuilds {
    DiscordGuilds::with_transport(transport)
}

fn run<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(future)
}

#[test]
fn enter_guild_puts_lurker_query_with_empty_json_object() {
    let transport = FakeTransport::answering(204);
    let guilds = api(transport.clone());

    assert_eq!(run(guilds.enter_guild(GUILD_ID)), Ok(()));

    let calls = transport.calls();
    assert_eq!(calls.len(), 1, "ровно один запрос");
    let (target, body) = &calls[0];
    assert_eq!(
        target, EXPECTED_TARGET,
        "метод PUT, путь members/@me и единственный query lurker=true"
    );
    assert_eq!(body.as_slice(), b"{}", "тело — ровно пустой JSON-объект");
    assert!(
        !target.contains("invites"),
        "это не join по инвайту (POST /invites/{{code}})"
    );
    assert!(
        !target.contains("session_id"),
        "session_id в query не подтверждён для lurker-входа"
    );
}

#[test]
fn enter_guild_accepts_204_without_body() {
    // 204 No Content: тело не читается и не парсится, поэтому пустое тело — успех.
    let transport = FakeTransport::answering(204);
    let guilds = api(transport);
    assert_eq!(run(enter_guild(&guilds, GUILD_ID)), Ok(()));
}

#[test]
fn http_statuses_map_to_typed_errors() {
    let cases = [
        (401, EnterGuildError::Unauthorized),
        (403, EnterGuildError::Forbidden),
        (404, EnterGuildError::NotFound),
        (429, EnterGuildError::RateLimited { retry_after_ms: 0 }),
        // G1-01: старый оракул требовал `Network` для 5xx/прочих 4xx — это была
        // потеря статуса («сеть недоступна» вместо ошибки сервера/запроса).
        // Единый классификатор: 5xx → Server, прочие 4xx (кроме 401/403/404/429) → Client.
        (500, EnterGuildError::Server { status: 500 }),
        (503, EnterGuildError::Server { status: 503 }),
        (400, EnterGuildError::Client { status: 400 }),
        // 204 — единственный подтверждённый live успех; иной 2xx = нарушение контракта.
        (200, EnterGuildError::Parse),
        (201, EnterGuildError::Parse),
    ];
    for (status, expected) in cases {
        let guilds = api(FakeTransport::answering(status));
        assert_eq!(
            run(guilds.enter_guild(GUILD_ID)),
            Err(expected),
            "HTTP {status}"
        );
    }
}

#[test]
fn transport_failure_is_typed_network_error() {
    let guilds = api(FakeTransport::failing(EnterGuildError::Network));
    assert_eq!(
        run(guilds.enter_guild(GUILD_ID)),
        Err(EnterGuildError::Network)
    );
}

#[test]
fn invalid_guild_id_is_rejected_before_transport() {
    for guild_id in [
        "",
        "   ",
        "abc",
        "12 34",
        "123\r\n",
        "{guild}",
        "@me",
        "-123",
        "12.5",
        // 21 цифра: за пределы u64 (снежинка Discord — до 20 цифр).
        "123456789012345678901",
    ] {
        let transport = FakeTransport::answering(204);
        let guilds = api(transport.clone());
        assert_eq!(
            run(guilds.enter_guild(guild_id)),
            Err(EnterGuildError::InvalidArg),
            "guild_id={guild_id:?}"
        );
        assert!(
            transport.calls().is_empty(),
            "некорректный id не должен доходить до сети: {guild_id:?}"
        );
    }
}

#[test]
fn empty_or_blank_token_is_missing_token() {
    for token in ["", "   ", "\t\n"] {
        assert_eq!(
            DiscordGuilds::new(token).expect_err("пустой токен — ошибка"),
            EnterGuildError::MissingToken
        );
    }
}

#[test]
fn client_debug_does_not_reveal_token() {
    let guilds = DiscordGuilds::new("secret-token").expect("клиент");
    let text = format!("{guilds:?}");
    assert!(!text.contains("secret-token"), "{text}");
}

#[test]
fn errors_serialize_with_camel_case_kind_only() {
    let cases = [
        (EnterGuildError::Unauthorized, "unauthorized"),
        (EnterGuildError::Forbidden, "forbidden"),
        (EnterGuildError::NotFound, "notFound"),
        (
            EnterGuildError::RateLimited {
                retry_after_ms: 250,
            },
            "rateLimited",
        ),
        (EnterGuildError::Network, "network"),
        (EnterGuildError::Storage, "storage"),
        (EnterGuildError::Parse, "parse"),
        (EnterGuildError::MissingToken, "missingToken"),
        (EnterGuildError::InvalidArg, "invalidArg"),
    ];
    for (error, kind) in cases {
        let value = serde_json::to_value(&error).expect("ошибка сериализуется");
        assert_eq!(value["kind"], kind, "{value}");
        assert!(!error.to_string().is_empty(), "Display непустой: {error}");
    }
}

/// Фейковая реализация контракта: проверяет, что обёртки валидируют до вызова.
#[derive(Default)]
struct FakeMembership {
    seen: Mutex<Vec<String>>,
    result: Option<EnterGuildError>,
}

impl GuildMembership for FakeMembership {
    fn enter_guild<'a>(
        &'a self,
        guild_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), EnterGuildError>> + Send + 'a>> {
        self.seen.lock().unwrap().push(guild_id.to_string());
        let answer = match &self.result {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        };
        Box::pin(async move { answer })
    }
}

#[test]
fn free_functions_validate_before_delegating() {
    let fake = FakeMembership::default();
    assert_eq!(run(enter_guild(&fake, GUILD_ID)), Ok(()));
    assert_eq!(run(join_guild(&fake, GUILD_ID)), Ok(()));
    assert_eq!(
        *fake.seen.lock().unwrap(),
        vec![GUILD_ID.to_string(), GUILD_ID.to_string()]
    );

    let fake = FakeMembership::default();
    assert_eq!(
        run(enter_guild(&fake, "not-a-snowflake")),
        Err(EnterGuildError::InvalidArg)
    );
    assert_eq!(
        run(join_guild(&fake, "not-a-snowflake")),
        Err(EnterGuildError::InvalidArg)
    );
    assert!(fake.seen.lock().unwrap().is_empty());
}

#[test]
fn free_functions_preserve_typed_error() {
    let fake = FakeMembership {
        seen: Mutex::new(Vec::new()),
        result: Some(EnterGuildError::Forbidden),
    };
    assert_eq!(
        run(enter_guild(&fake, GUILD_ID)),
        Err(EnterGuildError::Forbidden)
    );
    assert_eq!(
        run(join_guild(&fake, GUILD_ID)),
        Err(EnterGuildError::Forbidden)
    );
}

/// Тело `#[tauri::command]`-функции по имени: от `fn name` до следующей `#[tauri::command]`
/// или до конца файла. Нужен, чтобы проверить источник команды без окна.
fn command_body<'a>(source: &'a str, name: &str) -> &'a str {
    let marker = format!("fn {name}(");
    let start = source
        .find(&marker)
        .unwrap_or_else(|| panic!("main.rs должен объявлять команду {name}"));
    let rest = &source[start..];
    let end = rest.find("#[tauri::command]").unwrap_or(rest.len());
    &rest[..end]
}

/// Тело списка команд внутри `invoke_handler!` и его имена как отдельные токены.
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
fn tauri_registers_exactly_one_guild_command() {
    // Q-01: одна операция (`PUT /guilds/{id}/members/@me?lurker=true`) — одна
    // IPC-поверхность. `join_guild` — UI-глагол; `enter_guild` был алиасом и удалён.
    let source = include_str!("../src/main.rs");
    let commands = registered_commands(source);
    assert_eq!(
        commands
            .iter()
            .filter(|name| **name == "join_guild")
            .count(),
        1,
        "join_guild регистрируется ровно один раз"
    );
    assert!(
        !commands.contains(&"enter_guild"),
        "алиас enter_guild не должен попадать в invoke_handler!"
    );
    assert!(
        !source.contains("async fn enter_guild("),
        "в бинаре не должно остаться второй Tauri-команды enter_guild"
    );
}

#[test]
fn tauri_commands_take_no_token_and_read_backend() {
    let source = include_str!("../src/main.rs");
    // Единственная точка чтения токена: helper `token_or` (G1-02), без копий.
    assert!(
        source.contains("async fn token_or<"),
        "все команды должны читать токен через общий helper token_or"
    );
    assert!(
        source.contains("backend.token()"),
        "токен — только из AuthBackend::token()"
    );
    let body = command_body(source, "join_guild");
    assert!(
        body.contains("token_or::<"),
        "join_guild: токен берётся только общим helper'ом"
    );
    assert!(
        !body.contains("token: String"),
        "join_guild: токен не должен приходить из фронтенда"
    );
    assert!(
        !body.contains("println!") && !body.contains("eprintln!"),
        "join_guild: токен/ответ не логируются"
    );
}

#[test]
fn module_enter_and_join_are_equivalent() {
    // Модульные имена остаются двумя глаголами одной операции: одинаковый
    // target/тело/результат на всех исходах — `join_guild` лишь alias.
    for status in [204u16, 200, 401, 403, 404, 429, 500] {
        let enter_transport = FakeTransport::answering(status);
        let join_transport = FakeTransport::answering(status);
        let enter = run(enter_guild(&api(enter_transport.clone()), GUILD_ID));
        let join = run(join_guild(&api(join_transport.clone()), GUILD_ID));
        assert_eq!(enter, join, "HTTP {status}: исходы расходятся");
        assert_eq!(
            enter_transport.calls(),
            join_transport.calls(),
            "HTTP {status}: запросы расходятся"
        );
        assert_eq!(enter_transport.calls().len(), 1, "HTTP {status}");
    }

    for guild_id in ["", "abc", "123456789012345678901"] {
        let enter = run(enter_guild(&api(FakeTransport::answering(204)), guild_id));
        let join = run(join_guild(&api(FakeTransport::answering(204)), guild_id));
        assert_eq!(
            enter, join,
            "guild_id={guild_id:?}: валидация имён расходится"
        );
        assert_eq!(enter, Err(EnterGuildError::InvalidArg));
    }
}

// ---------------------------------------------------------------------------
// RW-40/G-04: выход с сервера.
//
// Оракул — `NETWORK-CONTRACTS.md` §1.1 #4 (`B` anchor `async leaveGuild(e)`):
// `DELETE /users/@me/guilds/{id}`, тело `{lurking: bool}` (call-site:
// `isLurking(id) || isCurrentUserGuest(id)`), ответ не читается. Коды `[A]`:
// тесты проверяют только решённое контрактом — 2xx успех, 401/403/404/429
// типизированы; тело ошибки не выдумывается.
// ---------------------------------------------------------------------------

#[test]
fn leave_guild_deletes_users_me_guilds_route_with_lurking_true() {
    let transport = FakeTransport::answering(204);
    let guilds = api(transport.clone());

    assert_eq!(run(leave_guild(&guilds, GUILD_ID, true)), Ok(()));

    let calls = transport.deletes();
    assert_eq!(calls.len(), 1, "ровно один DELETE");
    let (target, body) = &calls[0];
    assert_eq!(
        target, LEAVE_TARGET,
        "путь — users/@me/guilds/{{id}}, без query"
    );
    assert_eq!(
        body.as_slice(),
        br#"{"lurking":true}"#,
        "тело — ровно {{\"lurking\":true}}"
    );
    // Не путать с delete guild (владелец): `POST /guilds/{id}/delete`.
    assert!(!target.ends_with("/delete"), "это не delete guild");
    assert!(!target.contains("members/@me"), "это не join/preview");
}

#[test]
fn leave_guild_sends_explicit_false_lurking() {
    // `lurking` — явный bool от caller (UI решает isLurking/isGuest);
    // модуль не вычисляет его сам и не опускает поле.
    let transport = FakeTransport::answering(204);
    let guilds = api(transport.clone());

    assert_eq!(run(leave_guild(&guilds, GUILD_ID, false)), Ok(()));

    let calls = transport.deletes();
    assert_eq!(calls.len(), 1);
    let (target, body) = &calls[0];
    assert_eq!(target, LEAVE_TARGET);
    assert_eq!(body.as_slice(), br#"{"lurking":false}"#);
    let parsed: serde_json::Value = serde_json::from_slice(body).expect("тело — JSON");
    assert_eq!(parsed, serde_json::json!({ "lurking": false }));
    assert_eq!(
        parsed.as_object().expect("объект").len(),
        1,
        "ровно одно поле"
    );
}

#[test]
fn leave_guild_accepts_2xx_without_reading_body() {
    // Контракт: успех — 2xx; ответ не читается, поэтому 204 без тела и 200
    // с (даже неожиданным) телом одинаково успешны.
    for status in [200u16, 201, 204, 299] {
        let guilds = api(FakeTransport::answering(status));
        assert_eq!(
            run(leave_guild(&guilds, GUILD_ID, true)),
            Ok(()),
            "HTTP {status}"
        );
    }
}

#[test]
fn leave_guild_maps_http_statuses_to_typed_errors() {
    let cases = [
        (401u16, LeaveGuildError::Unauthorized),
        (403, LeaveGuildError::Forbidden),
        (404, LeaveGuildError::NotFound),
        (429, LeaveGuildError::RateLimited { retry_after_ms: 0 }),
        // Не подтверждённые контрактом статусы не теряются: 4xx → Client,
        // 5xx → Server (политика G1-01 в `rest_failure.rs`).
        (400, LeaveGuildError::Client { status: 400 }),
        (409, LeaveGuildError::Client { status: 409 }),
        (500, LeaveGuildError::Server { status: 500 }),
        (502, LeaveGuildError::Server { status: 502 }),
    ];
    for (status, expected) in cases {
        let guilds = api(FakeTransport::answering(status));
        assert_eq!(
            run(leave_guild(&guilds, GUILD_ID, false)),
            Err(expected),
            "HTTP {status}"
        );
    }
}

#[test]
fn leave_guild_transport_failure_is_typed_network_error() {
    let guilds = api(FakeTransport::failing_leave(LeaveGuildError::Network));
    assert_eq!(
        run(leave_guild(&guilds, GUILD_ID, true)),
        Err(LeaveGuildError::Network)
    );
}

#[test]
fn leave_guild_invalid_guild_id_is_rejected_before_transport() {
    for guild_id in [
        "",
        "   ",
        "abc",
        "12 34",
        "123\r\n",
        "{guild}",
        "@me",
        "-123",
        "12.5",
        // 21 цифра: за пределы u64.
        "123456789012345678901",
    ] {
        let transport = FakeTransport::answering(204);
        let guilds = api(transport.clone());
        assert_eq!(
            run(leave_guild(&guilds, guild_id, true)),
            Err(LeaveGuildError::InvalidArg),
            "guild_id={guild_id:?}"
        );
        assert!(
            transport.deletes().is_empty(),
            "некорректный id не должен доходить до сети: {guild_id:?}"
        );
    }
}

#[test]
fn leave_guild_without_token_is_missing_token() {
    for token in ["", "   ", "\t\n"] {
        let error = DiscordGuilds::new(token).expect_err("пустой токен — ошибка");
        assert_eq!(
            LeaveGuildError::from(error),
            LeaveGuildError::MissingToken,
            "token={token:?}"
        );
    }
}

#[test]
fn leave_errors_serialize_with_camel_case_kind_only() {
    let cases = [
        (LeaveGuildError::InvalidArg, "invalidArg"),
        (LeaveGuildError::MissingToken, "missingToken"),
        (LeaveGuildError::Unauthorized, "unauthorized"),
        (LeaveGuildError::Forbidden, "forbidden"),
        (LeaveGuildError::NotFound, "notFound"),
        (
            LeaveGuildError::RateLimited {
                retry_after_ms: 250,
            },
            "rateLimited",
        ),
        (LeaveGuildError::Client { status: 400 }, "client"),
        (LeaveGuildError::Server { status: 500 }, "server"),
        (LeaveGuildError::Network, "network"),
        (LeaveGuildError::Storage, "storage"),
        (LeaveGuildError::InvalidResponse, "invalidResponse"),
    ];
    for (error, kind) in cases {
        let value = serde_json::to_value(&error).expect("ошибка сериализуется");
        assert_eq!(value["kind"], kind, "{value}");
        assert!(!error.to_string().is_empty(), "Display непустой: {error}");
    }
}

/// Фейковая реализация контракта выхода: проверяет валидацию до делегирования.
#[derive(Default)]
struct FakeLeaving {
    seen: Mutex<Vec<(String, bool)>>,
    result: Option<LeaveGuildError>,
}

impl GuildLeaving for FakeLeaving {
    fn leave_guild<'a>(
        &'a self,
        guild_id: &'a str,
        lurking: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(), LeaveGuildError>> + Send + 'a>> {
        self.seen
            .lock()
            .unwrap()
            .push((guild_id.to_string(), lurking));
        let answer = match &self.result {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        };
        Box::pin(async move { answer })
    }
}

#[test]
fn leave_guild_free_function_validates_before_delegating() {
    let fake = FakeLeaving::default();
    assert_eq!(run(leave_guild(&fake, GUILD_ID, true)), Ok(()));
    assert_eq!(
        *fake.seen.lock().unwrap(),
        vec![(GUILD_ID.to_string(), true)]
    );

    let fake = FakeLeaving::default();
    assert_eq!(
        run(leave_guild(&fake, "not-a-snowflake", false)),
        Err(LeaveGuildError::InvalidArg)
    );
    assert!(fake.seen.lock().unwrap().is_empty());
}

#[test]
fn leave_guild_free_function_preserves_typed_error() {
    let fake = FakeLeaving {
        seen: Mutex::new(Vec::new()),
        result: Some(LeaveGuildError::Forbidden),
    };
    assert_eq!(
        run(leave_guild(&fake, GUILD_ID, false)),
        Err(LeaveGuildError::Forbidden)
    );
}

#[test]
fn tauri_registers_leave_guild_once_and_reads_token_via_helper() {
    let source = include_str!("../src/main.rs");
    let commands = registered_commands(source);
    assert_eq!(
        commands
            .iter()
            .filter(|name| **name == "leave_guild")
            .count(),
        1,
        "leave_guild регистрируется ровно один раз"
    );

    let body = command_body(source, "leave_guild");
    assert!(
        body.contains("token_or::<"),
        "leave_guild: токен берётся только общим helper'ом token_or"
    );
    assert!(
        !body.contains("token: String"),
        "leave_guild: токен не должен приходить из фронтенда"
    );
    assert!(
        !body.contains("println!") && !body.contains("eprintln!"),
        "leave_guild: токен/ответ не логируются"
    );
    assert!(
        body.contains("lurking: bool"),
        "leave_guild: lurking — явный bool из caller, не вычисляется в команде"
    );
    assert!(
        body.contains("LeaveGuildError::MissingToken"),
        "leave_guild: отсутствие токена — типизированный код домена"
    );
}

#[test]
fn create_guild_posts_to_guilds_endpoint_with_valid_name() {
    let response_body =
        r#"{"id":"999888777666555444","name":"Новый сервер","channels":[]}"#.as_bytes();
    let transport = FakeTransport::answering_post(201, response_body);
    let guilds = api(transport.clone());

    let payload = CreateGuildPayload {
        name: "Новый сервер".to_string(),
        icon: None,
        channels: None,
        guild_template_code: None,
    };

    let outcome = run(guilds.create_guild(payload)).expect("успешное создание сервера");
    assert_eq!(outcome.id, "999888777666555444");
    assert_eq!(outcome.name, "Новый сервер");

    let posts = transport.posts();
    assert_eq!(posts.len(), 1, "ровно один POST запрос");
    let (target, body) = &posts[0];
    assert_eq!(target, "/guilds", "маршрут POST /guilds");
    let parsed: serde_json::Value = serde_json::from_slice(body).expect("валидный JSON");
    assert_eq!(parsed["name"], "Новый сервер");
    assert!(parsed.get("icon").is_none() || parsed["icon"].is_null());
}

#[test]
fn create_guild_rejects_empty_or_too_long_name_before_transport() {
    let transport = FakeTransport::answering_post(201, b"{}");
    let guilds = api(transport.clone());

    // Пустое имя или пробелы
    let payload = CreateGuildPayload {
        name: "   ".to_string(),
        icon: None,
        channels: None,
        guild_template_code: None,
    };
    assert_eq!(
        run(guilds.create_guild(payload)),
        Err(CreateGuildError::InvalidArg)
    );
    assert!(transport.posts().is_empty(), "в сеть не уходим");

    // Имя длинее 100 символов
    let long_name = "a".repeat(101);
    let payload = CreateGuildPayload {
        name: long_name,
        icon: None,
        channels: None,
        guild_template_code: None,
    };
    assert_eq!(
        run(guilds.create_guild(payload)),
        Err(CreateGuildError::InvalidArg)
    );
    assert!(transport.posts().is_empty(), "в сеть не уходим");
}

#[test]
fn create_guild_sends_icon_when_provided() {
    let response_body =
        r#"{"id":"111222333444555666","name":"Сервер с иконкой","channels":[]}"#.as_bytes();
    let transport = FakeTransport::answering_post(201, response_body);
    let guilds = api(transport.clone());

    let payload = CreateGuildPayload {
        name: "Сервер с иконкой".to_string(),
        icon: Some("data:image/png;base64,iVBORw0KGgoAAAANSUhEUg==".to_string()),
        channels: None,
        guild_template_code: None,
    };

    let outcome = run(guilds.create_guild(payload)).expect("успех");
    assert_eq!(outcome.id, "111222333444555666");

    let posts = transport.posts();
    assert_eq!(posts.len(), 1);
    let (_, body) = &posts[0];
    let parsed: serde_json::Value = serde_json::from_slice(body).expect("валидный JSON");
    assert_eq!(
        parsed["icon"],
        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUg=="
    );
}

#[test]
fn create_guild_maps_http_statuses_to_typed_errors() {
    let cases = [
        (401, CreateGuildError::Unauthorized),
        (403, CreateGuildError::Forbidden),
        (
            429,
            CreateGuildError::RateLimited {
                retry_after_ms: 500,
            },
        ),
        (400, CreateGuildError::Client { status: 400 }),
        (500, CreateGuildError::Server { status: 500 }),
    ];

    for (status, expected) in cases {
        let transport = FakeTransport::failing_post(expected.clone());
        let guilds = api(transport);
        let payload = CreateGuildPayload {
            name: format!("Сервер-{status}"),
            icon: None,
            channels: None,
            guild_template_code: None,
        };
        assert_eq!(
            run(guilds.create_guild(payload)),
            Err(expected),
            "HTTP {status} -> typed error"
        );
    }
}

#[test]
fn create_guild_errors_serialize_with_camel_case_kind_only() {
    let cases = [
        (CreateGuildError::InvalidArg, "invalidArg"),
        (CreateGuildError::MissingToken, "missingToken"),
        (CreateGuildError::Unauthorized, "unauthorized"),
        (CreateGuildError::Forbidden, "forbidden"),
        (
            CreateGuildError::RateLimited {
                retry_after_ms: 250,
            },
            "rateLimited",
        ),
        (CreateGuildError::Client { status: 400 }, "client"),
        (CreateGuildError::Server { status: 500 }, "server"),
        (CreateGuildError::Network, "network"),
        (CreateGuildError::Storage, "storage"),
        (CreateGuildError::InvalidResponse, "invalidResponse"),
    ];
    for (error, kind) in cases {
        let value = serde_json::to_value(&error).expect("ошибка сериализуется");
        assert_eq!(value["kind"], kind, "{value}");
        assert!(!error.to_string().is_empty(), "Display непустой: {error}");
    }
}

#[derive(Default)]
struct FakeCreation {
    seen: Mutex<Vec<CreateGuildPayload>>,
    result: Option<CreateGuildError>,
}

impl chord_client::guilds::GuildCreation for FakeCreation {
    fn create_guild<'a>(
        &'a self,
        payload: CreateGuildPayload,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<
                        chord_client::guilds::CreateGuildOutcome,
                        CreateGuildError,
                    >,
                > + Send
                + 'a,
        >,
    > {
        self.seen.lock().unwrap().push(payload.clone());
        let answer = match &self.result {
            Some(err) => Err(err.clone()),
            None => Ok(chord_client::guilds::CreateGuildOutcome {
                id: "123".to_string(),
                name: payload.name,
                icon: payload.icon,
                owner_id: None,
                channels: Vec::new(),
            }),
        };
        Box::pin(async move { answer })
    }
}

#[test]
fn create_guild_free_function_validates_and_delegates() {
    let fake = FakeCreation::default();
    let payload = CreateGuildPayload {
        name: "Test Guild".to_string(),
        icon: None,
        channels: None,
        guild_template_code: None,
    };
    let outcome = run(create_guild(&fake, payload.clone())).expect("успех");
    assert_eq!(outcome.name, "Test Guild");
    assert_eq!(fake.seen.lock().unwrap().len(), 1);

    // Валидация до вызова: пустое имя
    let invalid_payload = CreateGuildPayload {
        name: "   ".to_string(),
        icon: None,
        channels: None,
        guild_template_code: None,
    };
    assert_eq!(
        run(create_guild(&fake, invalid_payload)),
        Err(CreateGuildError::InvalidArg)
    );
}

#[test]
fn tauri_registers_create_guild_once() {
    let source = include_str!("../src/main.rs");
    let commands = registered_commands(source);
    assert_eq!(
        commands
            .iter()
            .filter(|name| **name == "create_guild")
            .count(),
        1,
        "create_guild регистрируется ровно один раз"
    );
}
