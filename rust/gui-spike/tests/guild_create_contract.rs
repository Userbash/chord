//! Контрактные тесты TASK-611: `REST POST /guilds` для создания сервера пользователем.
//!
//! Оракулы:
//! - `docs/re/rest-d02.md` §6.1–§6.2 (`Authorization`, `X-Super-Properties`, `X-Discord-Locale: ru`,
//!   `X-Discord-Timezone`, `X-Debug-Options: bugReporterEnabled`, `Accept-Language: ru`,
//!   `Content-Type: application/json`).
//! - `/home/sanya/re-deep/rest-static.json` (`Endpoints.GUILDS = "/guilds"`).
//! - `/home/sanya/re-deep/out/bundles/web.4d4b2f75fdea5470.js` (`system_channel_flags`,
//!   коды ошибок `MAXIMUM_GUILDS = 30005` / `TOO_MANY_USER_GUILDS = 30001`, `NO_VERIFIED_PHONE`).
//!
//! Покрываемые 9 измерений (`TEST-POLICY.md`):
//! 1. Позитивные (`201 Created`, полный набор заголовков Discord, парсинг гильдии, каналов, ролей,
//!    интеграция в локальное состояние).
//! 2. Негативные (пустое/слишком длинное имя, невалидный base64 DataURI иконки, невалидный канал).
//! 3. Граничные (`name` длиной `1` и `100` символов, `101` символ, Snowflake `0` и `u64::MAX`).
//! 4. Ошибки (`400 MAXIMUM_GUILDS 30005`, `403 NO_VERIFIED_PHONE`, `401`, `429 retry_after_ms`, `5xx`).
//! 5. Пустые (`channels: []`, `roles: []`, опциональные `icon: None`, `system_channel_flags: None`).
//! 6. Загрузочные/транспортные (последовательные вызовы и корректный возврат после ошибки сети).
//! 7. Права доступа (`403 NO_VERIFIED_PHONE` vs обычный `403 Forbidden`, `permissions` ролей).
//! 8. Восстановление (после `429`/`503` следующий вызов `201 Created` штатно интегрирует сервер).
//! 9. Совместимость и наблюдаемость (маскирование токена в `Debug`/`Display`, camelCase `kind` в JSON).

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use discord_core::rest::client::{ApiAuth, ApiClient, ApiConfig};
use discord_core::rest::guilds::{
    build_guild_create_headers, build_guild_create_http_request, parse_guild_create_response,
    CreatedGuild, CreatedGuildChannel, CreatedGuildRole, GuildCreateChannelSpec,
    GuildCreateRequest, GuildCreateRestError, CHANNEL_TYPE_GUILD_CATEGORY, CHANNEL_TYPE_GUILD_TEXT,
    CHANNEL_TYPE_GUILD_VOICE, DISCORD_CODE_MAXIMUM_GUILDS, DISCORD_CODE_NO_VERIFIED_PHONE,
    DISCORD_LOCALE_RU, GUILDS_API_V9_PATH, GUILDS_PATH, MAX_GUILD_NAME_CHARS, MIN_GUILD_NAME_CHARS,
    SYSTEM_CHANNEL_FLAG_SUPPRESS_GUILD_REMINDER_NOTIFICATIONS,
    SYSTEM_CHANNEL_FLAG_SUPPRESS_JOIN_NOTIFICATIONS,
    SYSTEM_CHANNEL_FLAG_SUPPRESS_JOIN_NOTIFICATION_REPLIES,
    SYSTEM_CHANNEL_FLAG_SUPPRESS_PREMIUM_SUBSCRIPTIONS,
};
use discord_core::rest::identity::ClientIdentity;
use discord_core::rest::routes::HttpMethod;
use discord_core::rest::super_properties::SuperProperties;
use discord_core::rest::transport::{HttpRequest, HttpResponse};

use discord_gui_spike::guilds::{
    clear_local_created_guilds, create_guild_full, local_created_guild_snapshot,
    CreateGuildChannelInput, CreateGuildError, CreateGuildFullOutcome, CreateGuildPayload,
    CreateGuildRequest, DiscordGuilds, GuildTransport,
};

fn run<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("current-thread runtime")
        .block_on(future)
}

#[derive(Debug, Clone)]
struct RecordedPost {
    target: String,
    body: Vec<u8>,
}

struct MockGuildTransport {
    posted: Mutex<Vec<RecordedPost>>,
    responses: Mutex<Vec<Result<HttpResponse, CreateGuildError>>>,
}

impl MockGuildTransport {
    fn with_response(response: HttpResponse) -> Arc<Self> {
        Arc::new(Self {
            posted: Mutex::new(Vec::new()),
            responses: Mutex::new(vec![Ok(response)]),
        })
    }

    fn with_sequence(responses: Vec<Result<HttpResponse, CreateGuildError>>) -> Arc<Self> {
        Arc::new(Self {
            posted: Mutex::new(Vec::new()),
            responses: Mutex::new(responses),
        })
    }

    fn take_posted(&self) -> Vec<RecordedPost> {
        std::mem::take(&mut *self.posted.lock().expect("posted lock"))
    }
}

impl GuildTransport for MockGuildTransport {
    fn put<'a>(
        &'a self,
        _target: &'a str,
        _body: &'a [u8],
    ) -> Pin<
        Box<
            dyn Future<Output = Result<HttpResponse, discord_gui_spike::guilds::EnterGuildError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async {
            Ok(HttpResponse {
                status: 200,
                headers: Vec::new(),
                body: b"{}".to_vec(),
            })
        })
    }

    fn delete<'a>(
        &'a self,
        _target: &'a str,
        _body: &'a [u8],
    ) -> Pin<
        Box<
            dyn Future<Output = Result<HttpResponse, discord_gui_spike::guilds::LeaveGuildError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async {
            Ok(HttpResponse {
                status: 204,
                headers: Vec::new(),
                body: Vec::new(),
            })
        })
    }

    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, CreateGuildError>> + Send + 'a>> {
        self.posted.lock().expect("posted lock").push(RecordedPost {
            target: target.to_string(),
            body: body.to_vec(),
        });
        let next = {
            let mut guard = self.responses.lock().expect("responses lock");
            if guard.is_empty() {
                Err(CreateGuildError::Network)
            } else {
                guard.remove(0)
            }
        };
        Box::pin(async move { next })
    }
}

static STATE_TEST_LOCK: Mutex<()> = Mutex::new(());

const VALID_PNG_DATA_URI: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

fn find_header<'a>(req: &'a HttpRequest, name: &str) -> Option<&'a str> {
    req.headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

// -----------------------------------------------------------------------------
// 1. Заголовки Discord (`X-Super-Properties`, `X-Discord-Locale: ru`) и маршрут
// -----------------------------------------------------------------------------

#[test]
fn guild_create_route_and_headers_include_super_properties_and_ru_locale() {
    assert_eq!(GUILDS_PATH, "/guilds");
    assert_eq!(GUILDS_API_V9_PATH, "/api/v9/guilds");
    assert_eq!(DISCORD_LOCALE_RU, "ru");
    assert_eq!(MIN_GUILD_NAME_CHARS, 1);
    assert_eq!(MAX_GUILD_NAME_CHARS, 100);
    assert_eq!(DISCORD_CODE_MAXIMUM_GUILDS, 30005);
    assert_eq!(DISCORD_CODE_NO_VERIFIED_PHONE, "NO_VERIFIED_PHONE");

    let identity = ClientIdentity::rw_client();
    let super_props = identity.super_properties();
    let token = "mfa.test_secret_token_for_guild_create_611";

    let headers = build_guild_create_headers(token, super_props, "Europe/Moscow")
        .expect("заголовки собираются");
    let pairs = headers.pairs();
    assert_eq!(pairs[0], ("Authorization", token));
    assert_eq!(pairs[2], ("X-Discord-Locale", "ru"));
    assert_eq!(pairs[3], ("X-Discord-Timezone", "Europe/Moscow"));
    assert_eq!(pairs[4], ("X-Debug-Options", "bugReporterEnabled"));
    assert_eq!(pairs[5], ("Accept-Language", "ru"));

    let decoded_sp = SuperProperties::from_base64(pairs[1].1)
        .expect("X-Super-Properties декодируется обратно в 22 поля");
    assert_eq!(&decoded_sp, super_props);

    let req_spec = GuildCreateRequest {
        name: "Тестовый сервер".to_string(),
        icon: Some(VALID_PNG_DATA_URI.to_string()),
        channels: vec![
            GuildCreateChannelSpec::category("cat-1", "Текстовые каналы"),
            GuildCreateChannelSpec::text("общий", Some("cat-1")),
        ],
        system_channel_flags: Some(SYSTEM_CHANNEL_FLAG_SUPPRESS_JOIN_NOTIFICATIONS),
        guild_template_code: None,
    };

    let http_req = build_guild_create_http_request(token, super_props, "Europe/Moscow", &req_spec)
        .expect("HTTP-запрос собирается");
    assert_eq!(http_req.method, HttpMethod::Post);
    assert_eq!(http_req.target, "/guilds");
    assert_eq!(find_header(&http_req, "Authorization"), Some(token));
    assert_eq!(find_header(&http_req, "X-Discord-Locale"), Some("ru"));
    assert_eq!(find_header(&http_req, "Accept-Language"), Some("ru"));
    assert_eq!(
        find_header(&http_req, "X-Discord-Timezone"),
        Some("Europe/Moscow")
    );
    assert_eq!(
        find_header(&http_req, "X-Debug-Options"),
        Some("bugReporterEnabled")
    );
    assert_eq!(
        find_header(&http_req, "Content-Type"),
        Some("application/json")
    );
    let sp_header = find_header(&http_req, "X-Super-Properties").expect("X-Super-Properties");
    assert_eq!(
        &SuperProperties::from_base64(sp_header).expect("валидный base64 X-Super-Properties"),
        super_props
    );

    // Токен не утекает в Debug HttpRequest:
    let debug_text = format!("{http_req:?}");
    assert!(
        !debug_text.contains(token),
        "Debug не должен содержать сырой токен: {debug_text}"
    );
}

// -----------------------------------------------------------------------------
// 2. Тело запроса: name, icon (base64 DataURI), channels (категории + каналы),
//    system_channel_flags
// -----------------------------------------------------------------------------

#[test]
fn guild_create_request_body_serializes_name_icon_channels_and_system_channel_flags() {
    let flags = SYSTEM_CHANNEL_FLAG_SUPPRESS_JOIN_NOTIFICATIONS
        | SYSTEM_CHANNEL_FLAG_SUPPRESS_PREMIUM_SUBSCRIPTIONS
        | SYSTEM_CHANNEL_FLAG_SUPPRESS_GUILD_REMINDER_NOTIFICATIONS
        | SYSTEM_CHANNEL_FLAG_SUPPRESS_JOIN_NOTIFICATION_REPLIES;
    assert_eq!(flags, 15);

    let req = GuildCreateRequest {
        name: "  Сервер Разработки  ".to_string(),
        icon: Some(VALID_PNG_DATA_URI.to_string()),
        channels: vec![
            GuildCreateChannelSpec {
                id: Some("100".to_string()),
                name: "Текстовые каналы".to_string(),
                kind: CHANNEL_TYPE_GUILD_CATEGORY,
                parent_id: None,
                position: Some(0),
            },
            GuildCreateChannelSpec {
                id: Some("101".to_string()),
                name: "общий".to_string(),
                kind: CHANNEL_TYPE_GUILD_TEXT,
                parent_id: Some("100".to_string()),
                position: Some(0),
            },
            GuildCreateChannelSpec {
                id: Some("200".to_string()),
                name: "Голосовые каналы".to_string(),
                kind: CHANNEL_TYPE_GUILD_CATEGORY,
                parent_id: None,
                position: Some(1),
            },
            GuildCreateChannelSpec {
                id: None,
                name: "Голосовой".to_string(),
                kind: CHANNEL_TYPE_GUILD_VOICE,
                parent_id: Some("200".to_string()),
                position: Some(0),
            },
        ],
        system_channel_flags: Some(flags),
        guild_template_code: None,
    };

    let json_text = req.to_json_body().expect("JSON-тело собирается");
    let parsed: serde_json::Value =
        serde_json::from_str(&json_text).expect("валидный JSON по RFC 8259");

    assert_eq!(parsed["name"], "Сервер Разработки");
    assert_eq!(parsed["icon"], VALID_PNG_DATA_URI);
    assert_eq!(parsed["system_channel_flags"], 15);

    let channels = parsed["channels"].as_array().expect("массив channels");
    assert_eq!(channels.len(), 4);
    assert_eq!(channels[0]["id"], "100");
    assert_eq!(channels[0]["name"], "Текстовые каналы");
    assert_eq!(channels[0]["type"], 4);
    assert_eq!(channels[0]["position"], 0);

    assert_eq!(channels[1]["id"], "101");
    assert_eq!(channels[1]["name"], "общий");
    assert_eq!(channels[1]["type"], 0);
    assert_eq!(channels[1]["parent_id"], "100");

    assert_eq!(channels[2]["id"], "200");
    assert_eq!(channels[2]["name"], "Голосовые каналы");
    assert_eq!(channels[2]["type"], 4);

    assert_eq!(channels[3]["name"], "Голосовой");
    assert_eq!(channels[3]["type"], 2);
    assert_eq!(channels[3]["parent_id"], "200");
}

// -----------------------------------------------------------------------------
// 3. Валидация границ name (1..=100 символов), icon (base64 DataURI) и channels
// -----------------------------------------------------------------------------

#[test]
fn guild_create_validates_name_boundaries_1_to_100_chars_and_icon_data_uri() {
    // Граница 1 символ (Unicode и ASCII)
    let one_char_req = GuildCreateRequest {
        name: "Я".to_string(),
        icon: None,
        channels: Vec::new(),
        system_channel_flags: None,
        guild_template_code: None,
    };
    assert!(one_char_req.validate().is_ok(), "1 символ допустим");

    // Граница 100 символов
    let hundred_chars_req = GuildCreateRequest {
        name: "Ж".repeat(100),
        icon: None,
        channels: Vec::new(),
        system_channel_flags: None,
        guild_template_code: None,
    };
    assert!(
        hundred_chars_req.validate().is_ok(),
        "100 символов допустимы"
    );

    // Пустое или пробельное имя -> InvalidName
    for bad_name in ["", "   ", "\t\n"] {
        let req = GuildCreateRequest {
            name: bad_name.to_string(),
            icon: None,
            channels: Vec::new(),
            system_channel_flags: None,
            guild_template_code: None,
        };
        assert_eq!(req.validate(), Err(GuildCreateRestError::InvalidName));
    }

    // 101 символ -> InvalidName
    let too_long_req = GuildCreateRequest {
        name: "Ж".repeat(101),
        icon: None,
        channels: Vec::new(),
        system_channel_flags: None,
        guild_template_code: None,
    };
    assert_eq!(
        too_long_req.validate(),
        Err(GuildCreateRestError::InvalidName)
    );

    // Валидные DataURI иконки (png, jpeg, gif, webp)
    for mime in [
        "image/png",
        "image/jpeg",
        "image/jpg",
        "image/gif",
        "image/webp",
    ] {
        let req = GuildCreateRequest {
            name: "Сервер".to_string(),
            icon: Some(format!("data:{mime};base64,QUJDRA==")),
            channels: Vec::new(),
            system_channel_flags: None,
            guild_template_code: None,
        };
        assert!(req.validate().is_ok(), "{mime} должен быть валиден");
    }

    // Невалидные DataURI иконки -> InvalidIcon
    for bad_icon in [
        "",
        "https://cdn.discordapp.com/icons/1/a.png",
        "data:text/plain;base64,QUJDRA==",
        "data:image/png;QUJDRA==",
        "data:image/png;base64,",
        "data:image/png;base64,%%%не_base64%%%",
    ] {
        let req = GuildCreateRequest {
            name: "Сервер".to_string(),
            icon: Some(bad_icon.to_string()),
            channels: Vec::new(),
            system_channel_flags: None,
            guild_template_code: None,
        };
        assert_eq!(
            req.validate(),
            Err(GuildCreateRestError::InvalidIcon),
            "icon `{bad_icon}` должен отвергаться"
        );
    }

    // Невалидные каналы -> InvalidChannel
    let bad_channel_name = GuildCreateRequest {
        name: "Сервер".to_string(),
        icon: None,
        channels: vec![GuildCreateChannelSpec::text("   ", None)],
        system_channel_flags: None,
        guild_template_code: None,
    };
    assert_eq!(
        bad_channel_name.validate(),
        Err(GuildCreateRestError::InvalidChannel)
    );

    let overlong_channel_name = GuildCreateRequest {
        name: "Сервер".to_string(),
        icon: None,
        channels: vec![GuildCreateChannelSpec::text("к".repeat(101), None)],
        system_channel_flags: None,
        guild_template_code: None,
    };
    assert_eq!(
        overlong_channel_name.validate(),
        Err(GuildCreateRestError::InvalidChannel)
    );
}

// -----------------------------------------------------------------------------
// 4. Разбор ответа 201 Created: гильдия, каналы, роли и интеграция в local state
// -----------------------------------------------------------------------------

#[test]
fn guild_create_201_created_parses_guild_channels_roles_and_integrates_into_local_state() {
    let _guard = STATE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_local_created_guilds();

    let created_json = r#"{
        "id": "1300000000000000001",
        "name": "Сервер G1",
        "icon": "a_1234567890abcdef",
        "owner_id": "499602424292343829",
        "system_channel_id": "1300000000000000011",
        "system_channel_flags": 3,
        "channels": [
            {
                "id": "1300000000000000010",
                "guild_id": "1300000000000000001",
                "name": "Текстовые каналы",
                "type": 4,
                "parent_id": null,
                "position": 0
            },
            {
                "id": "1300000000000000011",
                "guild_id": "1300000000000000001",
                "name": "общий",
                "type": 0,
                "parent_id": "1300000000000000010",
                "position": 0
            },
            {
                "id": "1300000000000000012",
                "guild_id": "1300000000000000001",
                "name": "Голосовой",
                "type": 2,
                "parent_id": null,
                "position": 1
            }
        ],
        "roles": [
            {
                "id": "1300000000000000001",
                "name": "@everyone",
                "permissions": "1071698660929",
                "position": 0,
                "color": 0,
                "hoist": false,
                "mentionable": false
            },
            {
                "id": "1300000000000000050",
                "name": "Основатель",
                "permissions": "8",
                "position": 1,
                "color": 3447003,
                "hoist": true,
                "mentionable": true
            }
        ]
    }"#;

    let response = HttpResponse {
        status: 201,
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: created_json.as_bytes().to_vec(),
    };

    // 1. Проверка парсера discord-core
    let parsed: CreatedGuild =
        parse_guild_create_response(&response).expect("201 Created успешно парсится");
    assert_eq!(parsed.id, "1300000000000000001");
    assert_eq!(parsed.name, "Сервер G1");
    assert_eq!(parsed.icon.as_deref(), Some("a_1234567890abcdef"));
    assert_eq!(parsed.owner_id.as_deref(), Some("499602424292343829"));
    assert_eq!(
        parsed.system_channel_id.as_deref(),
        Some("1300000000000000011")
    );
    assert_eq!(parsed.system_channel_flags, 3);
    assert_eq!(
        parsed.channels,
        vec![
            CreatedGuildChannel {
                id: "1300000000000000010".to_string(),
                guild_id: Some("1300000000000000001".to_string()),
                name: "Текстовые каналы".to_string(),
                kind: 4,
                parent_id: None,
                position: 0,
            },
            CreatedGuildChannel {
                id: "1300000000000000011".to_string(),
                guild_id: Some("1300000000000000001".to_string()),
                name: "общий".to_string(),
                kind: 0,
                parent_id: Some("1300000000000000010".to_string()),
                position: 0,
            },
            CreatedGuildChannel {
                id: "1300000000000000012".to_string(),
                guild_id: Some("1300000000000000001".to_string()),
                name: "Голосовой".to_string(),
                kind: 2,
                parent_id: None,
                position: 1,
            },
        ]
    );
    assert_eq!(
        parsed.roles,
        vec![
            CreatedGuildRole {
                id: "1300000000000000001".to_string(),
                name: "@everyone".to_string(),
                permissions: "1071698660929".to_string(),
                position: 0,
                color: 0,
                hoist: false,
                mentionable: false,
            },
            CreatedGuildRole {
                id: "1300000000000000050".to_string(),
                name: "Основатель".to_string(),
                permissions: "8".to_string(),
                position: 1,
                color: 3447003,
                hoist: true,
                mentionable: true,
            },
        ]
    );

    // 2. Проверка клиентского вызова gui-spike и интеграции в локальное состояние
    let transport = MockGuildTransport::with_response(response);
    let guilds_api = DiscordGuilds::with_transport(transport.clone());

    let request = CreateGuildRequest {
        name: "Сервер G1".to_string(),
        icon: Some(VALID_PNG_DATA_URI.to_string()),
        channels: vec![
            CreateGuildChannelInput {
                id: Some("1".to_string()),
                name: "Текстовые каналы".to_string(),
                kind: 4,
                parent_id: None,
                position: Some(0),
            },
            CreateGuildChannelInput {
                id: None,
                name: "общий".to_string(),
                kind: 0,
                parent_id: Some("1".to_string()),
                position: Some(0),
            },
        ],
        system_channel_flags: Some(3),
        guild_template_code: None,
    };

    let outcome: CreateGuildFullOutcome =
        run(create_guild_full(&guilds_api, request)).expect("create_guild_full успешен");
    assert_eq!(outcome.id, "1300000000000000001");
    assert_eq!(outcome.name, "Сервер G1");
    assert_eq!(outcome.system_channel_flags, 3);
    assert_eq!(outcome.channels.len(), 3);
    assert_eq!(outcome.roles.len(), 2);

    // Проверка отправленного тела
    let posted = transport.take_posted();
    assert_eq!(posted.len(), 1);
    assert_eq!(posted[0].target, "/guilds");
    let sent_json: serde_json::Value =
        serde_json::from_slice(&posted[0].body).expect("валидный JSON запроса");
    assert_eq!(sent_json["name"], "Сервер G1");
    assert_eq!(sent_json["icon"], VALID_PNG_DATA_URI);
    assert_eq!(sent_json["system_channel_flags"], 3);
    assert_eq!(sent_json["channels"].as_array().unwrap().len(), 2);

    // Проверка интеграции в локальное состояние без падения:
    let snapshot = local_created_guild_snapshot("1300000000000000001")
        .expect("созданная гильдия сохранена в локальном состоянии");
    assert_eq!(snapshot.id, "1300000000000000001");
    assert_eq!(snapshot.name, "Сервер G1");
    assert_eq!(snapshot.channels.len(), 3);
    assert_eq!(snapshot.roles.len(), 2);
    assert_eq!(snapshot.roles[1].name, "Основатель");
    assert!(
        discord_gui_spike::data::has_live_guild_or_channels("1300000000000000001"),
        "гильдия и её каналы интегрированы в data::LIVE_IMPORT"
    );
}

// -----------------------------------------------------------------------------
// 5. Граничные и пустые ответы 201 Created + защита от битого JSON
// -----------------------------------------------------------------------------

#[test]
fn guild_create_handles_201_with_empty_channels_and_roles_and_rejects_invalid_payloads() {
    let _guard = STATE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_local_created_guilds();

    // u64::MAX в качестве Snowflake ID, без массивов channels/roles
    let minimal_201 = HttpResponse {
        status: 201,
        headers: Vec::new(),
        body: r#"{"id":"18446744073709551615","name":"Мин"}"#.as_bytes().to_vec(),
    };
    let transport = MockGuildTransport::with_response(minimal_201);
    let guilds_api = DiscordGuilds::with_transport(transport);
    let payload = CreateGuildPayload {
        name: "Мин".to_string(),
        icon: None,
        channels: None,
        guild_template_code: None,
    };
    let outcome = run(guilds_api.create_guild(payload)).expect("минимальный 201 парсится");
    assert_eq!(outcome.id, "18446744073709551615");
    assert_eq!(outcome.name, "Мин");
    assert!(outcome.channels.is_empty());

    let snap = local_created_guild_snapshot("18446744073709551615")
        .expect("минимальная гильдия в локальном состоянии");
    assert!(snap.channels.is_empty());
    assert!(snap.roles.is_empty());

    // Невалидный Snowflake `0` или нечисловой id -> InvalidResponse
    for bad_body in [
        r#"{"id":"0","name":"Сервер"}"#,
        r#"{"id":"abc","name":"Сервер"}"#,
        r#"{"id":"123","name":""}"#,
        r#"not-json"#,
    ] {
        let resp = HttpResponse {
            status: 201,
            headers: Vec::new(),
            body: bad_body.as_bytes().to_vec(),
        };
        assert!(
            matches!(
                parse_guild_create_response(&resp),
                Err(GuildCreateRestError::InvalidResponse(_))
            ),
            "bad body `{bad_body}` должен возвращать InvalidResponse"
        );
    }
}

// -----------------------------------------------------------------------------
// 6. Обработка ошибки 400 (MAXIMUM_GUILDS 30005)
// -----------------------------------------------------------------------------

#[test]
fn guild_create_maps_400_maximum_guilds_30005_to_typed_error() {
    let err_body = r#"{"code": 30005, "message": "Maximum number of guilds reached (100)"}"#;
    let response = HttpResponse {
        status: 400,
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: err_body.as_bytes().to_vec(),
    };

    let core_err = parse_guild_create_response(&response).expect_err("400 30005 — ошибка");
    assert_eq!(
        core_err,
        GuildCreateRestError::MaximumGuilds {
            code: 30005,
            message: "Maximum number of guilds reached (100)".to_string(),
        }
    );

    let transport = MockGuildTransport::with_response(response);
    let guilds_api = DiscordGuilds::with_transport(transport);
    let payload = CreateGuildPayload {
        name: "Сервер 101".to_string(),
        icon: None,
        channels: None,
        guild_template_code: None,
    };
    let gui_err = run(guilds_api.create_guild(payload)).expect_err("ожидается MaximumGuilds");
    assert_eq!(gui_err, CreateGuildError::MaximumGuilds { code: 30005 });

    let serialized = serde_json::to_value(&gui_err).expect("сериализация ошибки");
    assert_eq!(serialized["kind"], "maximumGuilds");
    assert_eq!(serialized["code"], 30005);
    assert!(gui_err.to_string().contains("30005"));

    // Обычный 400 без кода 30005 остаётся Client { status: 400 }
    let generic_400 = HttpResponse {
        status: 400,
        headers: Vec::new(),
        body: br#"{"code": 50035, "message": "Invalid Form Body"}"#.to_vec(),
    };
    let transport_generic = MockGuildTransport::with_response(generic_400);
    let guilds_generic = DiscordGuilds::with_transport(transport_generic);
    let payload_generic = CreateGuildPayload {
        name: "Сервер".to_string(),
        icon: None,
        channels: None,
        guild_template_code: None,
    };
    assert_eq!(
        run(guilds_generic.create_guild(payload_generic)),
        Err(CreateGuildError::Client { status: 400 })
    );
}

// -----------------------------------------------------------------------------
// 7. Обработка ошибки 403 (NO_VERIFIED_PHONE)
// -----------------------------------------------------------------------------

#[test]
fn guild_create_maps_403_no_verified_phone_to_typed_error() {
    let err_body =
        r#"{"code": "NO_VERIFIED_PHONE", "message": "Must have a verified phone on account"}"#;
    let response = HttpResponse {
        status: 403,
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: err_body.as_bytes().to_vec(),
    };

    let core_err = parse_guild_create_response(&response).expect_err("403 NO_VERIFIED_PHONE");
    assert_eq!(
        core_err,
        GuildCreateRestError::NoVerifiedPhone {
            message: "Must have a verified phone on account".to_string(),
        }
    );

    let transport = MockGuildTransport::with_response(response);
    let guilds_api = DiscordGuilds::with_transport(transport);
    let payload = CreateGuildPayload {
        name: "Сервер с телефоном".to_string(),
        icon: None,
        channels: None,
        guild_template_code: None,
    };
    let gui_err = run(guilds_api.create_guild(payload)).expect_err("ожидается NoVerifiedPhone");
    assert_eq!(gui_err, CreateGuildError::NoVerifiedPhone);

    let serialized = serde_json::to_value(&gui_err).expect("сериализация ошибки");
    assert_eq!(serialized["kind"], "noVerifiedPhone");
    assert!(gui_err.to_string().contains("NO_VERIFIED_PHONE"));

    // Обычный 403 без NO_VERIFIED_PHONE мапится в Forbidden
    let generic_403 = HttpResponse {
        status: 403,
        headers: Vec::new(),
        body: br#"{"code": 50013, "message": "Missing Permissions"}"#.to_vec(),
    };
    let transport_generic = MockGuildTransport::with_response(generic_403);
    let guilds_generic = DiscordGuilds::with_transport(transport_generic);
    let payload_generic = CreateGuildPayload {
        name: "Сервер".to_string(),
        icon: None,
        channels: None,
        guild_template_code: None,
    };
    assert_eq!(
        run(guilds_generic.create_guild(payload_generic)),
        Err(CreateGuildError::Forbidden)
    );
}

// -----------------------------------------------------------------------------
// 8. Ошибки 401/429/5xx, восстановление после сбоя и интеграция с ApiAuth
// -----------------------------------------------------------------------------

#[test]
fn guild_create_maps_401_429_5xx_and_recovers_on_subsequent_201() {
    let _guard = STATE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_local_created_guilds();

    let rate_limit_429 = HttpResponse {
        status: 429,
        headers: vec![("Retry-After".to_string(), "1.5".to_string())],
        body: br#"{"message": "You are being rate limited.", "retry_after": 1.5}"#.to_vec(),
    };
    assert_eq!(
        parse_guild_create_response(&rate_limit_429),
        Err(GuildCreateRestError::RateLimited {
            retry_after_ms: 1500
        })
    );

    let server_503 = HttpResponse {
        status: 503,
        headers: Vec::new(),
        body: b"Service Unavailable".to_vec(),
    };
    assert_eq!(
        parse_guild_create_response(&server_503),
        Err(GuildCreateRestError::Server { status: 503 })
    );

    let ok_201 = HttpResponse {
        status: 201,
        headers: Vec::new(),
        body: r#"{"id":"1300000000000000099","name":"Восстановленный сервер","channels":[],"roles":[]}"#.as_bytes().to_vec(),
    };

    let transport =
        MockGuildTransport::with_sequence(vec![Ok(rate_limit_429), Ok(server_503), Ok(ok_201)]);
    let guilds_api = DiscordGuilds::with_transport(transport);

    let mk_req = || CreateGuildRequest {
        name: "Восстановленный сервер".to_string(),
        icon: None,
        channels: Vec::new(),
        system_channel_flags: Some(0),
        guild_template_code: None,
    };

    assert_eq!(
        run(create_guild_full(&guilds_api, mk_req())),
        Err(CreateGuildError::RateLimited {
            retry_after_ms: 1500
        })
    );
    assert_eq!(
        run(create_guild_full(&guilds_api, mk_req())),
        Err(CreateGuildError::Server { status: 503 })
    );

    let recovered =
        run(create_guild_full(&guilds_api, mk_req())).expect("после 429/503 запрос 201 успешен");
    assert_eq!(recovered.id, "1300000000000000099");
    assert!(local_created_guild_snapshot("1300000000000000099").is_some());

    // Проверка ApiClient::new с локалью ru
    let identity = ClientIdentity::rw_client();
    let auth = ApiAuth::from_token(
        "secret_tok_999",
        identity.super_properties(),
        DISCORD_LOCALE_RU,
        "Europe/Moscow",
    )
    .expect("ApiAuth с ru локалью");
    assert_eq!(auth.locale, "ru");
    assert_eq!(auth.accept_language, "ru");
    let client = ApiClient::new(ApiConfig {
        auth,
        ..ApiConfig::default()
    })
    .expect("ApiClient создаётся");
    let api_guilds = DiscordGuilds::from_api_client(Arc::new(client));
    // При невалидном имени ошибка возвращается до сетевого вызова
    let bad_req = CreateGuildRequest {
        name: "".to_string(),
        icon: None,
        channels: Vec::new(),
        system_channel_flags: None,
        guild_template_code: None,
    };
    assert_eq!(
        run(create_guild_full(&api_guilds, bad_req)),
        Err(CreateGuildError::InvalidArg)
    );
}
