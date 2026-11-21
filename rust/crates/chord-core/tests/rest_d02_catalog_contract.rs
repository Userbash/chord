//! D-02 Contract Test: REST API v9 Routes, Request Body Schemas, Authentication Headers & Rate Limits.
//!
//! Independent Oracles:
//! - `docs/re/rest-d02.md` (clean-room empirical specification of 47 `/api/v9` routes, 133 requests,
//!   6 request body schemas, 6 auth headers across 221 requests, 22 `X-Super-Properties` fields, 429 rate limit)
//! - `docs/re/rest-d02.json` (`md5 = 87d7a05d8cd11f3b8ae37aa5c9811c9e`)
//! - `docs/re/rest-d02.stats.json` & `docs/re/rest-d02.crossref.json`

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::time::Duration;

use chord_core::API_V9_BASE;
use chord_core::rest::ack::{build_ack_body, build_ack_target};
use chord_core::rest::guilds::GUILDS_PATH;
use chord_core::rest::headers::{
    AUTH_HEADER_NAMES, AuthHeaderValues, ResponseClass, auth_headers, classify_status,
};
use chord_core::rest::rate_limit::{
    DEFAULT_RETRY_AFTER, MAX_RETRY_AFTER, RETRYABLE_SERVER_STATUSES, RestFailure, classify_failure,
    rate_limit_info, retry_delay, should_retry,
};
use chord_core::rest::resources::{
    RestDomainCatalogSummary, RestRouteDomain, classify_route_domain, d02_domain_catalog_summary,
    routes_by_domain,
};
use chord_core::rest::routes::{
    CAPTURED_D02_ROUTE_COUNT, CAPTURED_QUERY_PARAMS, CHANNEL_ATTACHMENTS_PATH, CHANNEL_SEARCH_PATH,
    ChannelMessagesParams, GUILD_SEARCH_PATH, HttpMethod, INVITE_PATH, MessageSearchParams, ROUTES,
    STATIC_EXTENSION_ROUTE_COUNT, TYPING_PATH, ThreadSearchParams, build_channel_attachments_path,
    build_channel_history_path, build_channel_messages_path, build_channel_messages_send_path,
    build_channel_pins_path, build_channel_search_path, build_guild_search_path,
    build_thread_search_path, find, render_path,
};
use chord_core::rest::super_properties::field_names;
use chord_json::JsonValue;

fn repo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

#[test]
fn d02_domain_catalog_covers_auth_messaging_search_channels_and_guilds() {
    let summary: RestDomainCatalogSummary = d02_domain_catalog_summary();
    assert_eq!(summary.api_base, API_V9_BASE);
    assert_eq!(summary.captured_d02_routes, CAPTURED_D02_ROUTE_COUNT);
    assert_eq!(summary.static_extension_routes, STATIC_EXTENSION_ROUTE_COUNT);
    assert_eq!(summary.auth_header_count, 6);
    assert_eq!(summary.super_properties_field_count, 22);
    assert_eq!(summary.d02_payload_route_count, 6);

    // Every required criteria domain in D-02 must have non-empty route coverage.
    assert!(
        summary.authentication_routes >= 2,
        "Authentication domain must cover /users/@me and MFA webauthn credentials"
    );
    assert!(
        summary.messaging_routes >= 5,
        "Messaging domain must cover message history, send, ack, reactions, and pins"
    );
    assert!(
        summary.search_routes >= 4,
        "Search domain must cover thread search, guild message search, channel message search, and games autocomplete"
    );
    assert!(
        summary.channel_routes >= 9,
        "Channels domain must cover all /channels/{{id}}/* endpoints including extensions"
    );
    assert!(
        summary.guild_routes >= 3,
        "Guilds domain must cover /guilds/{{id}}/* endpoints"
    );

    // Verify specific domain classifications and route lookups.
    assert_eq!(
        classify_route_domain(HttpMethod::Get, "/users/@me"),
        RestRouteDomain::Authentication
    );
    assert_eq!(
        classify_route_domain(HttpMethod::Get, "/users/@me/mfa/webauthn/credentials"),
        RestRouteDomain::Authentication
    );
    assert_eq!(
        classify_route_domain(HttpMethod::Get, "/channels/{id}/messages"),
        RestRouteDomain::Messaging
    );
    assert_eq!(
        classify_route_domain(HttpMethod::Post, "/channels/{id}/messages"),
        RestRouteDomain::Messaging
    );
    assert_eq!(
        classify_route_domain(HttpMethod::Post, "/channels/{id}/messages/{id}/ack"),
        RestRouteDomain::Messaging
    );
    assert_eq!(
        classify_route_domain(HttpMethod::Get, "/channels/{id}/threads/search"),
        RestRouteDomain::Search
    );
    assert_eq!(
        classify_route_domain(HttpMethod::Get, GUILD_SEARCH_PATH),
        RestRouteDomain::Search
    );
    assert_eq!(
        classify_route_domain(HttpMethod::Get, CHANNEL_SEARCH_PATH),
        RestRouteDomain::Search
    );
    assert_eq!(
        classify_route_domain(HttpMethod::Patch, "/channels/{id}/explicit-media"),
        RestRouteDomain::Channels
    );
    assert_eq!(
        classify_route_domain(HttpMethod::Post, TYPING_PATH),
        RestRouteDomain::Channels
    );
    assert_eq!(
        classify_route_domain(HttpMethod::Post, CHANNEL_ATTACHMENTS_PATH),
        RestRouteDomain::Channels
    );
    assert_eq!(
        classify_route_domain(HttpMethod::Get, "/guilds/{id}/application-command-index"),
        RestRouteDomain::Guilds
    );
    assert_eq!(
        classify_route_domain(HttpMethod::Put, "/guilds/{id}/members/@me"),
        RestRouteDomain::Guilds
    );
    assert_eq!(
        classify_route_domain(HttpMethod::Post, "/beaker"),
        RestRouteDomain::Telemetry
    );

    let auth_specs = routes_by_domain(RestRouteDomain::Authentication);
    assert!(
        auth_specs
            .iter()
            .any(|s| s.method == HttpMethod::Get && s.path == "/users/@me")
    );
    assert!(
        auth_specs
            .iter()
            .any(|s| s.method == HttpMethod::Get
                && s.path == "/users/@me/mfa/webauthn/credentials")
    );

    let msg_specs = routes_by_domain(RestRouteDomain::Messaging);
    assert!(
        msg_specs
            .iter()
            .any(|s| s.method == HttpMethod::Get && s.path == "/channels/{id}/messages")
    );
    assert!(
        msg_specs
            .iter()
            .any(|s| s.method == HttpMethod::Post && s.path == "/channels/{id}/messages")
    );
    assert!(
        msg_specs
            .iter()
            .any(|s| s.method == HttpMethod::Post && s.path == "/channels/{id}/messages/{id}/ack")
    );

    let search_specs = routes_by_domain(RestRouteDomain::Search);
    assert!(
        search_specs
            .iter()
            .any(|s| s.method == HttpMethod::Get && s.path == "/channels/{id}/threads/search")
    );
    assert!(
        search_specs
            .iter()
            .any(|s| s.method == HttpMethod::Get && s.path == GUILD_SEARCH_PATH)
    );
    assert!(
        search_specs
            .iter()
            .any(|s| s.method == HttpMethod::Get && s.path == CHANNEL_SEARCH_PATH)
    );

    let channel_specs = routes_by_domain(RestRouteDomain::Channels);
    assert!(
        channel_specs
            .iter()
            .any(|s| s.method == HttpMethod::Patch && s.path == "/channels/{id}/explicit-media")
    );
    assert!(
        channel_specs
            .iter()
            .any(|s| s.method == HttpMethod::Post && s.path == TYPING_PATH)
    );

    let guild_specs = routes_by_domain(RestRouteDomain::Guilds);
    assert!(
        guild_specs
            .iter()
            .any(|s| s.method == HttpMethod::Put && s.path == "/guilds/{id}/members/@me")
    );
    assert_eq!(GUILDS_PATH, "/guilds");
}

#[test]
fn d02_request_body_schemas_and_url_builders_validate_boundaries() {
    // 1. Verify all 6 D-02 payload routes have exact body_keys matching docs/re/rest-d02.md §4.
    let expected_d02_payloads: [(&str, HttpMethod, &[&str]); 6] = [
        (
            "/channels/{id}/messages/{id}/ack",
            HttpMethod::Post,
            &["token", "last_viewed"],
        ),
        (
            "/users/@me/billing/user-offer",
            HttpMethod::Post,
            &["allow_triggers"],
        ),
        (
            "/guilds/{id}/members/@me",
            HttpMethod::Put,
            &["(empty-object)"],
        ),
        ("/beaker", HttpMethod::Post, &["token", "events"]),
        (
            "/channels/{id}/explicit-media",
            HttpMethod::Patch,
            &["message_ids"],
        ),
        (
            "/users/@me/settings-proto/{num}",
            HttpMethod::Patch,
            &["(non-json)"],
        ),
    ];

    for (path, method, expected_keys) in expected_d02_payloads {
        let spec = find(method, path).unwrap_or_else(|| panic!("missing route {method:?} {path}"));
        assert_eq!(
            spec.body_keys, expected_keys,
            "body_keys for {method:?} {path}"
        );
    }

    // 2. Verify extension payload schemas: message send, invite accept, attachment upload.
    assert_eq!(
        find(HttpMethod::Post, "/channels/{id}/messages")
            .expect("POST messages")
            .body_keys,
        &["content", "nonce", "tts"]
    );
    assert_eq!(
        find(HttpMethod::Post, INVITE_PATH)
            .expect("POST invite")
            .body_keys,
        &[
            "session_id",
            "invite_instance_id",
            "received_installation_id"
        ]
    );
    assert_eq!(
        find(HttpMethod::Post, CHANNEL_ATTACHMENTS_PATH)
            .expect("POST attachments")
            .body_keys,
        &["files"]
    );

    // 3. Verify ACK target & body builder against D-02 §4 oracle (`{"token":null,"last_viewed":4310}`).
    assert_eq!(
        build_ack_target("1056563701764857896", "1300000000000000001").unwrap(),
        "/channels/1056563701764857896/messages/1300000000000000001/ack"
    );
    assert_eq!(
        build_ack_body(None, 4310),
        r#"{"token":null,"last_viewed":4310}"#
    );

    // 4. Verify boundary checks on messaging, search, channels, and guilds URL builders.
    assert!(
        build_channel_messages_send_path("0").is_err(),
        "snowflake 0 rejected"
    );
    assert!(
        build_channel_messages_send_path("18446744073709551615").is_ok(),
        "u64::MAX snowflake accepted"
    );
    assert_eq!(
        build_channel_history_path("1056563701764857896", Some("1300000000000000001"), Some(12))
            .unwrap(),
        "/channels/1056563701764857896/messages?limit=12&before=1300000000000000001"
    );
    assert!(
        build_channel_messages_path(
            "1056563701764857896",
            &ChannelMessagesParams {
                limit: Some(101),
                ..Default::default()
            }
        )
        .is_err(),
        "limit > 100 rejected"
    );
    assert!(
        build_channel_pins_path("1058348665342808104", Some(51)).is_err(),
        "pins limit > 50 rejected"
    );
    assert!(
        build_thread_search_path("1058348665342808104", &ThreadSearchParams::empty()).is_err(),
        "missing required archived parameter rejected"
    );
    assert_eq!(
        build_guild_search_path(
            "1014537851418980372",
            &MessageSearchParams {
                content: Some("hello"),
                offset: Some(0),
                ..Default::default()
            }
        )
        .unwrap(),
        "/guilds/1014537851418980372/messages/search?content=hello&offset=0"
    );
    assert_eq!(
        build_channel_search_path(
            "1058348665342808104",
            &MessageSearchParams {
                content: Some("rust"),
                ..Default::default()
            }
        )
        .unwrap(),
        "/channels/1058348665342808104/messages/search?content=rust"
    );
    assert!(build_channel_attachments_path("0").is_err());
    assert_eq!(
        build_channel_attachments_path("1058348665342808104").unwrap(),
        "/channels/1058348665342808104/attachments"
    );
}

#[test]
fn d02_auth_headers_super_properties_and_rate_limits_match_oracle() {
    // 1. 6 mandatory authentication headers (D-02 §6.1: 133/133 API requests, 221 total with science).
    assert_eq!(
        AUTH_HEADER_NAMES,
        [
            "Authorization",
            "X-Super-Properties",
            "X-Discord-Locale",
            "X-Discord-Timezone",
            "X-Debug-Options",
            "Accept-Language",
        ]
    );
    let pairs = auth_headers(&AuthHeaderValues {
        authorization: "REDACTED",
        super_properties: "eyJvcyI6IkxpbnV4In0=",
        locale: "en-US",
        timezone: "UTC",
        debug_options: "bugReporterEnabled",
        accept_language: "en-US,en;q=0.9",
    });
    assert_eq!(pairs.len(), 6);
    assert_eq!(
        field_names().len(),
        22,
        "X-Super-Properties must contain 22 fields (D-02 §6.2)"
    );
    assert_eq!(
        CAPTURED_QUERY_PARAMS.len(),
        45,
        "D-02 §3 captures 45 query parameter names"
    );

    // 2. Observed HTTP status codes in D-02 §2: 200, 204, 304, 404, 429.
    assert_eq!(classify_status(200), ResponseClass::Ok200);
    assert_eq!(classify_status(204), ResponseClass::NoContent204);
    assert_eq!(classify_status(304), ResponseClass::NotModified304);
    assert_eq!(classify_status(404), ResponseClass::NotFound404);
    assert_eq!(classify_status(429), ResponseClass::RateLimited429);

    // 3. Rate-limit parsing and retry backoff contract (D-02 §2 `GET /users/@me/survey` -> 429).
    let rl_headers = vec![
        ("retry-after".to_string(), "2.5".to_string()),
        ("x-ratelimit-global".to_string(), "false".to_string()),
        ("x-ratelimit-remaining".to_string(), "0".to_string()),
        ("x-ratelimit-reset-after".to_string(), "2.5".to_string()),
        ("x-ratelimit-bucket".to_string(), "survey-bucket".to_string()),
    ];
    let body_bytes = br#"{"retry_after":2.5,"global":false}"#;
    let info = rate_limit_info(429, &rl_headers, body_bytes).expect("429 rate limit parsed");
    assert_eq!(info.retry_after, Duration::from_millis(2500));
    assert!(!info.global);
    assert_eq!(info.remaining, Some(0));
    assert_eq!(info.bucket.as_deref(), Some("survey-bucket"));

    let failure_429 = classify_failure(429, &rl_headers, b"").expect("429 classified as failure");
    assert_eq!(
        retry_delay(1, &failure_429, Duration::from_secs(1), MAX_RETRY_AFTER),
        Some(Duration::from_millis(2500))
    );
    assert!(should_retry(HttpMethod::Get, &failure_429));
    assert!(should_retry(HttpMethod::Post, &failure_429));

    let failure_401 = classify_failure(401, &[], b"").expect("401 classified as failure");
    assert_eq!(failure_401, RestFailure::Unauthorized);
    assert_eq!(
        retry_delay(1, &failure_401, Duration::from_secs(1), MAX_RETRY_AFTER),
        None
    );
    assert!(!should_retry(HttpMethod::Get, &failure_401));

    assert_eq!(DEFAULT_RETRY_AFTER, Duration::from_secs(5));
    assert_eq!(MAX_RETRY_AFTER, Duration::from_secs(60));
    assert_eq!(RETRYABLE_SERVER_STATUSES.len(), 8);
}

#[test]
fn d02_markdown_and_json_oracles_are_consistent_and_complete() {
    let root = repo_root();
    let md_path = root.join("docs/re/rest-d02.md");
    let json_path = root.join("docs/re/rest-d02.json");
    let stats_path = root.join("docs/re/rest-d02.stats.json");
    let crossref_path = root.join("docs/re/rest-d02.crossref.json");

    let md_text = fs::read_to_string(&md_path).expect("docs/re/rest-d02.md must exist");
    let json_text = fs::read_to_string(&json_path).expect("docs/re/rest-d02.json must exist");
    let stats_text =
        fs::read_to_string(&stats_path).expect("docs/re/rest-d02.stats.json must exist");
    let crossref_text =
        fs::read_to_string(&crossref_path).expect("docs/re/rest-d02.crossref.json must exist");

    // Verify all mandatory sections in docs/re/rest-d02.md
    for heading in [
        "## 0. Quantitative Summary",
        "## 1. Methodology and Reproducibility",
        "## 2. Endpoints Table (/api/v9)",
        "## 3. Query Parameters",
        "## 4. Request Payloads (postData)",
        "## 5. Response Bodies",
        "## 6. Authentication Flow",
        "## 7. Filtered Non-API Noise",
        "## 8. Static Reconciliation and Unknowns",
        "## 9. Appendix: Reproducibility Guide",
    ] {
        assert!(
            md_text.contains(heading),
            "docs/re/rest-d02.md missing heading: {heading}"
        );
    }

    let oracle = chord_json::parse(&json_text).expect("valid rest-d02.json");
    let stats = chord_json::parse(&stats_text).expect("valid rest-d02.stats.json");
    let crossref = chord_json::parse(&crossref_text).expect("valid rest-d02.crossref.json");

    let endpoints = oracle
        .get("endpoints")
        .and_then(JsonValue::as_array)
        .expect("endpoints array");
    assert_eq!(endpoints.len(), 47, "47 unique endpoints in rest-d02.json");

    // Verify every single one of the 47 endpoints in rest-d02.json is documented in rest-d02.md
    // and present in chord_core::rest::routes::ROUTES.
    let mut total_calls = 0u64;
    let mut routes_with_body = 0usize;
    let mut observed_statuses = BTreeSet::new();

    for ep in endpoints {
        let method_str = ep
            .get("method")
            .and_then(JsonValue::as_str)
            .expect("endpoint method");
        let full_path = ep
            .get("path")
            .and_then(JsonValue::as_str)
            .expect("endpoint path");
        let count = ep
            .get("count")
            .and_then(JsonValue::as_u64)
            .expect("endpoint count");
        total_calls += count;

        let rel_path = full_path
            .strip_prefix("/api/v9")
            .expect("endpoint starts with /api/v9");
        assert!(
            md_text.contains(rel_path),
            "docs/re/rest-d02.md must document route {rel_path}"
        );

        let method = match method_str {
            "GET" => HttpMethod::Get,
            "POST" => HttpMethod::Post,
            "PUT" => HttpMethod::Put,
            "PATCH" => HttpMethod::Patch,
            "DELETE" => HttpMethod::Delete,
            other => panic!("unexpected method {other}"),
        };
        let route_spec = find(method, rel_path)
            .unwrap_or_else(|| panic!("route {method_str} {rel_path} missing from ROUTES"));
        assert!(
            !route_spec.is_static_extension(),
            "D-02 route {method_str} {rel_path} must not be marked as static extension"
        );

        let body_keys = ep
            .get("reqBodyKeys")
            .and_then(JsonValue::as_array)
            .expect("reqBodyKeys");
        if !body_keys.is_empty() {
            routes_with_body += 1;
        }
        assert_eq!(
            route_spec.body_keys.len(),
            body_keys.len(),
            "body_keys count mismatch on {method_str} {rel_path}"
        );

        for status_val in ep
            .get("statuses")
            .and_then(JsonValue::as_array)
            .expect("statuses")
        {
            observed_statuses.insert(status_val.as_u64().expect("status u64") as u16);
        }

        // Verify render_path works on every catalogued D-02 route
        let placeholders = rel_path.matches("{id}").count() + rel_path.matches("{num}").count();
        let sample_args: Vec<&str> = (0..placeholders).map(|_| "123456789012345678").collect();
        let rendered = render_path(route_spec, &sample_args).expect("render_path succeeds");
        assert!(rendered.starts_with('/'));
        assert!(!rendered.contains("{id}") && !rendered.contains("{num}"));
    }

    assert_eq!(total_calls, 133, "133 total API v9 calls in D-02");
    assert_eq!(routes_with_body, 6, "6 routes with request body in D-02");
    assert_eq!(
        observed_statuses.into_iter().collect::<Vec<u16>>(),
        vec![200, 204, 304, 404, 429]
    );

    // Cross-check stats and crossref JSON files
    let provenance = stats
        .get("provenance")
        .and_then(JsonValue::as_array)
        .expect("stats provenance");
    assert_eq!(provenance.len(), 2, "2 snapshot files in provenance");

    let crossref_details = crossref
        .get("details")
        .and_then(JsonValue::as_array)
        .expect("crossref details");
    assert_eq!(crossref_details.len(), 47, "47 crossref records");
    let confirmed_count = ROUTES.iter().filter(|r| !r.is_static_extension()).count();
    assert_eq!(confirmed_count, 47);
}
