//! RW-40 / BE-P2-10: контракт чтения профиля пользователя
//! (`GET /users/{id}/profile`).
//!
//! Оракул: `/home/sanya/re-deep/rest-static.json`, endpoint `USER_PROFILE`
//! (`GET /users/{arg}/profile`, 3 call-site) и `cacheEvidence.example`:
//! `https://discord.com/api/v9/users/1168176063113875531/profile?type=popout&with_mutual_guilds=true&with_mutual_friends=true&with_mutual_friends_count=false&guild_id=1014537851418980372`.
//! Шаблон маршрута, порядок query-параметров и значения по умолчанию берутся из
//! этого захвата, а не придумываются.
//!
//! Контракт:
//! - `build_user_profile_path(user_id, query)` собирает путь с query **в порядке
//!   захвата**: `type`, `with_mutual_guilds`, `with_mutual_friends`,
//!   `with_mutual_friends_count`, затем `guild_id` (если задан).
//! - `user_id`/`guild_id` — снежинки (непустые ASCII-цифры, влезающие в `u64`);
//!   иначе `ProfileError::InvalidArg` **до** любого обращения к транспорту.
//! - `fetch_user_profile` делает ровно один `GET` по построенному пути, мапит
//!   401/403/404/429/4xx/5xx в типизированные ошибки и разбирает тело в
//!   [`UserProfileResponse`], требуя непустые `user.id` и `user.username`.
//! - `429` несёт `retry_after_ms` из JSON-тела либо заголовков
//!   `Retry-After`/`x-ratelimit-reset-after` (секунды -> миллисекунды).

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use chord_core::rest::transport::HttpResponse;
use chord_client::profile::{
    build_user_profile_path, parse_retry_after_ms, parse_user_profile, ConnectedAccount,
    DiscordProfileApi, GuildMemberProfile, MutualGuild, ProfileBadge, ProfileError, ProfileQuery,
    ProfileTransport, ProfileUser, UserProfileResponse, DEFAULT_POPOUT_QUERY,
    USER_PROFILE_PATH_TEMPLATE,
};
use chord_client::rest_failure::RestFailure;

const USER_ID: &str = "1168176063113875531";
const GUILD_ID: &str = "1014537851418980372";

/// Полный путь из `cacheEvidence.example` (без хоста и `/api/v9`).
const EXPECTED_DEFAULT_PATH: &str = "/users/1168176063113875531/profile?type=popout&with_mutual_guilds=true&with_mutual_friends=true&with_mutual_friends_count=false";
const EXPECTED_GUILD_PATH: &str = "/users/1168176063113875531/profile?type=popout&with_mutual_guilds=true&with_mutual_friends=true&with_mutual_friends_count=false&guild_id=1014537851418980372";

#[derive(Clone, Default)]
struct FakeTransport {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    error: Option<ProfileError>,
    calls: Arc<Mutex<Vec<String>>>,
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

    fn failing(error: ProfileError) -> Self {
        Self {
            status: 0,
            headers: Vec::new(),
            body: Vec::new(),
            error: Some(error),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl ProfileTransport for FakeTransport {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, ProfileError>> + Send + 'a>> {
        self.calls.lock().unwrap().push(target.to_string());
        let answer = match &self.error {
            Some(error) => Err(error.clone()),
            None => Ok(HttpResponse {
                status: self.status,
                headers: self.headers.clone(),
                body: self.body.clone(),
            }),
        };
        Box::pin(async move { answer })
    }
}

fn api(transport: FakeTransport) -> DiscordProfileApi<FakeTransport> {
    DiscordProfileApi::new(transport)
}

fn run<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(future)
}

fn full_profile_body() -> &'static str {
    r#"{
        "user": {
            "id": "1168176063113875531",
            "username": "testuser",
            "global_name": "TestUser",
            "avatar": "a_abc123",
            "bio": "привет",
            "pronouns": "он/его",
            "banner": "def456",
            "accent_color": 16711680,
            "premium_type": 2
        },
        "badges": [
            {"id": "staff", "description": "Discord Staff", "icon": "staff", "link": "https://discord.com/staff"},
            {"id": "nitro", "description": "Nitro"}
        ],
        "guild_member": {
            "guild_id": "1014537851418980372",
            "nick": "TestUser",
            "bio": "guild bio",
            "pronouns": "он",
            "roles": ["1", "2"]
        },
        "connected_accounts": [
            {"type": "github", "id": "42", "name": "testuser", "verified": true},
            {"type": "steam", "id": "7", "name": "testuser", "verified": false}
        ],
        "mutual_guilds": [
            {"id": "1014537851418980372", "nick": "TestUser"},
            {"id": "2"}
        ],
        "mutual_friends_count": 3
    }"#
}

// ---------------------------------------------------------------------------
// Маршрут и query
// ---------------------------------------------------------------------------

#[test]
fn path_template_and_default_query_match_the_static_oracle() {
    assert_eq!(USER_PROFILE_PATH_TEMPLATE, "/users/{id}/profile");
    assert_eq!(
        DEFAULT_POPOUT_QUERY,
        "type=popout&with_mutual_guilds=true&with_mutual_friends=true&with_mutual_friends_count=false"
    );

    let path = build_user_profile_path(USER_ID, &ProfileQuery::default()).unwrap();
    assert_eq!(path, EXPECTED_DEFAULT_PATH);
    let query = path
        .strip_prefix(&format!("/users/{USER_ID}/profile?"))
        .expect("путь профиля с query");
    assert_eq!(
        query, DEFAULT_POPOUT_QUERY,
        "query строится из DEFAULT_POPOUT_QUERY без перестановки параметров"
    );
}

#[test]
fn default_query_is_popout_with_mutuals_but_without_guild() {
    let query = ProfileQuery::default();
    assert_eq!(query.profile_type.as_deref(), Some("popout"));
    assert!(query.with_mutual_guilds);
    assert!(query.with_mutual_friends);
    assert!(!query.with_mutual_friends_count);
    assert_eq!(query.guild_id, None);
}

#[test]
fn guild_id_is_appended_last_exactly_as_in_the_capture() {
    let query = ProfileQuery {
        guild_id: Some(GUILD_ID.to_string()),
        ..ProfileQuery::default()
    };
    assert_eq!(
        build_user_profile_path(USER_ID, &query),
        Ok(EXPECTED_GUILD_PATH.to_string())
    );
}

#[test]
fn every_query_flag_is_written_as_the_literal_true_or_false() {
    let query = ProfileQuery {
        profile_type: Some("popout".to_string()),
        with_mutual_guilds: false,
        with_mutual_friends: true,
        with_mutual_friends_count: true,
        guild_id: None,
    };
    assert_eq!(
        build_user_profile_path(USER_ID, &query),
        Ok(format!(
            "/users/{USER_ID}/profile?type=popout&with_mutual_guilds=false&with_mutual_friends=true&with_mutual_friends_count=true"
        ))
    );
}

#[test]
fn absent_profile_type_is_omitted_instead_of_written_empty() {
    let query = ProfileQuery {
        profile_type: None,
        ..ProfileQuery::default()
    };
    assert_eq!(
        build_user_profile_path(USER_ID, &query),
        Ok(format!(
            "/users/{USER_ID}/profile?with_mutual_guilds=true&with_mutual_friends=true&with_mutual_friends_count=false"
        ))
    );
}

#[test]
fn user_id_must_be_a_non_empty_ascii_digit_snowflake() {
    for invalid in [
        "",
        " ",
        "abc",
        "12a",
        "a12",
        "-1",
        "+1",
        "1 2",
        "１２３",
        "1.0",
        "18446744073709551616", // u64::MAX + 1
    ] {
        assert_eq!(
            build_user_profile_path(invalid, &ProfileQuery::default()),
            Err(ProfileError::InvalidArg),
            "user_id={invalid:?}"
        );
    }
    for valid in ["0", "1", "18446744073709551615"] {
        assert!(
            build_user_profile_path(valid, &ProfileQuery::default()).is_ok(),
            "user_id={valid:?}"
        );
    }
}

#[test]
fn guild_id_must_be_a_snowflake_when_present() {
    for invalid in ["", " ", "abc", "12a", "18446744073709551616"] {
        let query = ProfileQuery {
            guild_id: Some(invalid.to_string()),
            ..ProfileQuery::default()
        };
        assert_eq!(
            build_user_profile_path(USER_ID, &query),
            Err(ProfileError::InvalidArg),
            "guild_id={invalid:?}"
        );
    }
    let query = ProfileQuery {
        guild_id: Some("0".to_string()),
        ..ProfileQuery::default()
    };
    assert!(build_user_profile_path(USER_ID, &query).is_ok());
}

#[test]
fn profile_type_cannot_inject_extra_query_parameters() {
    for invalid in [
        "",
        "popout&guild_id=1",
        "pop out",
        "popout#frag",
        "popout?x=1",
    ] {
        let query = ProfileQuery {
            profile_type: Some(invalid.to_string()),
            ..ProfileQuery::default()
        };
        assert_eq!(
            build_user_profile_path(USER_ID, &query),
            Err(ProfileError::InvalidArg),
            "profile_type={invalid:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Разбор ответа
// ---------------------------------------------------------------------------

#[test]
fn full_response_body_maps_every_documented_field() {
    assert_eq!(
        parse_user_profile(full_profile_body().as_bytes()),
        Ok(UserProfileResponse {
            user: ProfileUser {
                id: USER_ID.to_string(),
                username: "testuser".to_string(),
                global_name: Some("TestUser".to_string()),
                avatar: Some("a_abc123".to_string()),
                bio: Some("привет".to_string()),
                pronouns: Some("он/его".to_string()),
                banner: Some("def456".to_string()),
                accent_color: Some(16_711_680),
            },
            badges: vec![
                ProfileBadge {
                    id: "staff".to_string(),
                    description: "Discord Staff".to_string(),
                    icon: Some("staff".to_string()),
                    link: Some("https://discord.com/staff".to_string()),
                },
                ProfileBadge {
                    id: "nitro".to_string(),
                    description: "Nitro".to_string(),
                    icon: None,
                    link: None,
                },
            ],
            guild_member: Some(GuildMemberProfile {
                guild_id: Some(GUILD_ID.to_string()),
                nick: Some("TestUser".to_string()),
                bio: Some("guild bio".to_string()),
                pronouns: Some("он".to_string()),
                roles: vec!["1".to_string(), "2".to_string()],
            }),
            connected_accounts: vec![
                ConnectedAccount {
                    account_type: "github".to_string(),
                    id: "42".to_string(),
                    name: "testuser".to_string(),
                    verified: true,
                },
                ConnectedAccount {
                    account_type: "steam".to_string(),
                    id: "7".to_string(),
                    name: "testuser".to_string(),
                    verified: false,
                },
            ],
            mutual_guilds: vec![
                MutualGuild {
                    id: GUILD_ID.to_string(),
                    nick: Some("TestUser".to_string()),
                },
                MutualGuild {
                    id: "2".to_string(),
                    nick: None,
                },
            ],
            mutual_friends_count: Some(3),
        })
    );
}

#[test]
fn minimal_response_body_defaults_absent_collections_and_optionals() {
    let parsed = parse_user_profile(br#"{"user": {"id": "1", "username": "x"}}"#).unwrap();
    assert_eq!(parsed.user.id, "1");
    assert_eq!(parsed.user.username, "x");
    assert_eq!(parsed.user.global_name, None);
    assert_eq!(parsed.user.accent_color, None);
    assert!(parsed.badges.is_empty());
    assert_eq!(parsed.guild_member, None);
    assert!(parsed.connected_accounts.is_empty());
    assert!(parsed.mutual_guilds.is_empty());
    assert_eq!(parsed.mutual_friends_count, None);
}

#[test]
fn guild_member_null_is_treated_as_absence() {
    let parsed =
        parse_user_profile(br#"{"user": {"id": "1", "username": "x"}, "guild_member": null}"#)
            .unwrap();
    assert_eq!(parsed.guild_member, None);
}

#[test]
fn empty_or_missing_user_identity_is_invalid_response() {
    for body in [
        &b"{}"[..],
        br#"{"user": {}}"#,
        br#"{"user": {"id": "", "username": "x"}}"#,
        br#"{"user": {"id": "1", "username": ""}}"#,
        br#"{"user": {"id": "1"}}"#,
        br#"{"user": {"username": "x"}}"#,
    ] {
        assert_eq!(
            parse_user_profile(body),
            Err(ProfileError::InvalidResponse),
            "body={}",
            String::from_utf8_lossy(body)
        );
    }
}

#[test]
fn malformed_bodies_are_invalid_response() {
    for body in [
        &b""[..],
        b"not json",
        b"[]",
        b"null",
        br#"{"user": "not-an-object"}"#,
        br#"{"user": {"id": "1", "username": "x"}, "badges": "no"}"#,
        br#"{"user": {"id": "1", "username": "x"}, "mutual_friends_count": -1}"#,
    ] {
        assert_eq!(
            parse_user_profile(body),
            Err(ProfileError::InvalidResponse),
            "body={}",
            String::from_utf8_lossy(body)
        );
    }
}

#[test]
fn connected_account_type_rename_is_reversible_on_serialize() {
    let account = ConnectedAccount {
        account_type: "twitch".to_string(),
        id: "9".to_string(),
        name: "streamer".to_string(),
        verified: true,
    };
    let json = serde_json::to_string(&account).unwrap();
    assert!(json.contains(r#""type":"twitch""#), "{json}");
    assert!(!json.contains("account_type"), "{json}");
    assert_eq!(
        serde_json::from_str::<ConnectedAccount>(&json).unwrap(),
        account
    );
}

// ---------------------------------------------------------------------------
// Транспорт и ошибки
// ---------------------------------------------------------------------------

#[test]
fn fetch_performs_one_get_on_the_built_path_and_decodes_the_profile() {
    let transport = FakeTransport::with_body(200, full_profile_body().as_bytes());
    let api = api(transport.clone());

    let profile = run(api.fetch_user_profile(USER_ID, &ProfileQuery::default())).unwrap();

    assert_eq!(profile.user.id, USER_ID);
    assert_eq!(profile.mutual_friends_count, Some(3));
    assert_eq!(transport.calls(), vec![EXPECTED_DEFAULT_PATH.to_string()]);
}

#[test]
fn fetch_forwards_the_guild_scoped_path() {
    let transport = FakeTransport::with_body(200, full_profile_body().as_bytes());
    let api = api(transport.clone());
    let query = ProfileQuery {
        guild_id: Some(GUILD_ID.to_string()),
        ..ProfileQuery::default()
    };

    run(api.fetch_user_profile(USER_ID, &query)).unwrap();

    assert_eq!(transport.calls(), vec![EXPECTED_GUILD_PATH.to_string()]);
}

#[test]
fn invalid_ids_are_rejected_before_any_transport_call() {
    let transport = FakeTransport::with_body(200, full_profile_body().as_bytes());
    let api = api(transport.clone());

    assert_eq!(
        run(api.fetch_user_profile("not-a-snowflake", &ProfileQuery::default())),
        Err(ProfileError::InvalidArg)
    );
    let bad_guild = ProfileQuery {
        guild_id: Some("oops".to_string()),
        ..ProfileQuery::default()
    };
    assert_eq!(
        run(api.fetch_user_profile(USER_ID, &bad_guild)),
        Err(ProfileError::InvalidArg)
    );
    assert!(transport.calls().is_empty(), "сеть не трогаем");
}

#[test]
fn http_statuses_map_to_typed_errors() {
    let cases = [
        (401, ProfileError::Unauthorized),
        (403, ProfileError::Forbidden),
        (404, ProfileError::NotFound),
        (429, ProfileError::RateLimited { retry_after_ms: 0 }),
        (400, ProfileError::Client { status: 400 }),
        (405, ProfileError::Client { status: 405 }),
        (500, ProfileError::Server { status: 500 }),
        (503, ProfileError::Server { status: 503 }),
    ];
    for (status, expected) in cases {
        let api = api(FakeTransport::answering(status));
        assert_eq!(
            run(api.fetch_user_profile(USER_ID, &ProfileQuery::default())),
            Err(expected),
            "HTTP {status}"
        );
    }
}

#[test]
fn non_2xx_bodies_are_not_parsed_as_profiles() {
    // 200 с мусором в теле — InvalidResponse, а не паника/пустой профиль.
    let api = api(FakeTransport::with_body(200, b"<html>"));
    assert_eq!(
        run(api.fetch_user_profile(USER_ID, &ProfileQuery::default())),
        Err(ProfileError::InvalidResponse)
    );
}

#[test]
fn ok_response_with_broken_identity_is_invalid_response() {
    let api = api(FakeTransport::with_body(
        200,
        br#"{"user": {"id": "", "username": ""}}"#,
    ));
    assert_eq!(
        run(api.fetch_user_profile(USER_ID, &ProfileQuery::default())),
        Err(ProfileError::InvalidResponse)
    );
}

#[test]
fn fetch_429_extracts_retry_after_from_json_body() {
    let api = api(FakeTransport::with_body(
        429,
        br#"{"retry_after": 1.5, "global": false}"#,
    ));
    assert_eq!(
        run(api.fetch_user_profile(USER_ID, &ProfileQuery::default())),
        Err(ProfileError::RateLimited {
            retry_after_ms: 1500
        })
    );
}

#[test]
fn fetch_429_extracts_retry_after_from_headers() {
    for header in ["Retry-After", "retry-after", "x-ratelimit-reset-after"] {
        let api = api(FakeTransport::answering(429).with_headers(&[(header, " 2.25 ")]));
        assert_eq!(
            run(api.fetch_user_profile(USER_ID, &ProfileQuery::default())),
            Err(ProfileError::RateLimited {
                retry_after_ms: 2250
            }),
            "header={header}"
        );
    }
}

#[test]
fn parse_retry_after_supports_strings_headers_and_unusable_values() {
    let json_number = HttpResponse {
        status: 429,
        headers: Vec::new(),
        body: br#"{"retry_after": 0.5}"#.to_vec(),
    };
    assert_eq!(parse_retry_after_ms(&json_number), 500);

    let json_string = HttpResponse {
        status: 429,
        headers: Vec::new(),
        body: br#"{"retry_after": "1.25"}"#.to_vec(),
    };
    assert_eq!(parse_retry_after_ms(&json_string), 1250);

    for body in [
        &b""[..],
        b"{}",
        br#"{"retry_after": null}"#,
        br#"{"retry_after": 0}"#,
        br#"{"retry_after": -1}"#,
        br#"{"retry_after": "soon"}"#,
    ] {
        let response = HttpResponse {
            status: 429,
            headers: Vec::new(),
            body: body.to_vec(),
        };
        assert_eq!(
            parse_retry_after_ms(&response),
            0,
            "body={}",
            String::from_utf8_lossy(body)
        );
    }

    let header_fallback = HttpResponse {
        status: 429,
        headers: vec![("x-ratelimit-reset-after".to_string(), "3".to_string())],
        body: br#"{"retry_after": 0}"#.to_vec(),
    };
    assert_eq!(parse_retry_after_ms(&header_fallback), 3000);

    let huge = HttpResponse {
        status: 429,
        headers: Vec::new(),
        body: br#"{"retry_after": 1e300}"#.to_vec(),
    };
    assert_eq!(parse_retry_after_ms(&huge), u64::MAX);
}

#[test]
fn transport_failure_propagates_and_skips_body_decoding() {
    for error in [
        ProfileError::Network,
        ProfileError::Storage,
        ProfileError::Unauthorized,
    ] {
        let api = api(FakeTransport::failing(error.clone()));
        assert_eq!(
            run(api.fetch_user_profile(USER_ID, &ProfileQuery::default())),
            Err(error)
        );
    }
}

#[test]
fn rest_failure_classifier_maps_every_variant() {
    let cases = [
        (RestFailure::Unauthorized, ProfileError::Unauthorized),
        (RestFailure::Forbidden, ProfileError::Forbidden),
        (RestFailure::NotFound, ProfileError::NotFound),
        (
            RestFailure::RateLimited { retry_after_ms: 42 },
            ProfileError::RateLimited { retry_after_ms: 42 },
        ),
        (
            RestFailure::ClientOther { status: 409 },
            ProfileError::Client { status: 409 },
        ),
        (
            RestFailure::Server { status: 502 },
            ProfileError::Server { status: 502 },
        ),
        (RestFailure::Network, ProfileError::Network),
        (RestFailure::InvalidResponse, ProfileError::InvalidResponse),
    ];
    for (failure, expected) in cases {
        assert_eq!(ProfileError::from(failure.clone()), expected, "{failure:?}");
    }
}

#[test]
fn display_is_human_readable_and_keeps_status_and_delay() {
    assert!(ProfileError::InvalidArg.to_string().contains("id"));
    assert!(ProfileError::Unauthorized.to_string().contains("сессия"));
    assert!(ProfileError::Forbidden.to_string().contains("доступ"));
    assert!(ProfileError::NotFound.to_string().contains("не найден"));
    assert!(ProfileError::Client { status: 409 }
        .to_string()
        .contains("409"));
    assert!(ProfileError::Server { status: 503 }
        .to_string()
        .contains("503"));
    assert!(ProfileError::RateLimited {
        retry_after_ms: 750
    }
    .to_string()
    .contains("750"));
    assert!(ProfileError::Network.to_string().contains("сеть"));
    assert!(ProfileError::Storage.to_string().contains("хранилищ"));
    assert!(ProfileError::InvalidResponse.to_string().contains("ответ"));
}
