//! Wave 4 Contract Tests: W4-2 (`P0-3.4`) — Live REST v9 Message Send (`POST /channels/{id}/messages`),
//! Channel History Fetch (`GET /channels/{id}/messages?limit=50`), & `ChatView` Wiring.
//!
//! Covers all 9 mandatory dimensions (`AGENTS.md` §5):
//! 1. Positive: REST route helpers, `ApiClient` live JSON & multipart message send (`content`, `nonce`,
//!    `tts`, `message_reference`, `attachments`), history fetch (`limit=50`, `before` cursor),
//!    chronological ordering & deduplication, `ChatView` channel switch (`Loading -> Loaded`) and
//!    optimistic send (`Pending -> Confirmed`).
//! 2. Negative: Empty message + 0 attachments rejected, >2000 chars rejected, invalid `limit` (`0`, `101`),
//!    invalid `nonce`, invalid `message_reference`, >10 attachments rejected.
//! 3. Edge boundaries: Snowflake `0` rejected, `1` and `u64::MAX` (`18446744073709551615`) accepted,
//!    `u64::MAX + 1` rejected; exact 2000 UTF-8 chars boundary; `limit=1`, `50`, `100`.
//! 4. Error handling: HTTP `400`, `401`, `403`, `404`, `413`, `429 Retry-After`, `500`/`502`/`503` mapped
//!    deterministically to `ChatRestError`.
//! 5. Empty states: `GET /channels/{id}/messages?limit=50` returning `[]` transitions `ChatView` to
//!    `ChannelHistoryLoadState::Empty` and marks `reached_history_end`; empty pagination page stops.
//! 6. Loading skeletons / phases: `ChannelHistoryLoadState::Loading -> Loaded/Empty/Error` and
//!    `OptimisticSendPhase::Pending -> Confirmed/Failed`.
//! 7. Capability permissions & security: `403 Missing Permissions` surfaced in `ChatView`; attachment
//!    filename path traversal & CRLF injection sanitized on live send.
//! 8. Recovery / fallback & nonce deduplication: Failed send (`429`/`5xx`) preserves optimistic entry,
//!    `nonce`, and staged attachments; retry reuses the same `nonce` and upgrades in-place without
//!    duplicating messages; Gateway echo with matching `nonce` reconciles in-place.
//! 9. Observable telemetry & headers: All 6 `ApiAuth` headers + `BrowserImpersonationProfile` headers
//!    verified on history GET and message POST; `PooledRequestMetrics` recorded; telemetry blocked.

#![recursion_limit = "512"]
#![allow(dead_code)]

#[path = "../src/audio/mod.rs"]
mod audio;
#[path = "../src/dispatch/mod.rs"]
mod dispatch;
#[path = "../src/media/mod.rs"]
mod media;
#[path = "../src/net/mod.rs"]
mod net;
#[path = "../src/platform/mod.rs"]
mod platform;
#[path = "../src/state/mod.rs"]
mod state;
#[path = "../src/storage/mod.rs"]
mod storage;
#[path = "../src/theme/mod.rs"]
mod theme;
#[path = "../src/ui/mod.rs"]
mod ui;
#[path = "../src/video/mod.rs"]
mod video;

use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;
use std::time::Duration;

use chord_core::rest::client::{
    ApiAuth, ApiClient, ApiConfig, ChatRestError, HttpTransport, MAX_ATTACHMENT_SIZE_BYTES,
    MAX_ATTACHMENTS_PER_MESSAGE, MAX_MESSAGE_CONTENT_CHARS, MessageReferenceSpec,
    MultipartUploadFile, SendMessageAttachmentRef, SendMessageParams,
};
use chord_core::rest::routes::{
    CAPTURED_D02_ROUTE_COUNT, CHANNEL_MESSAGES_PATH, DEFAULT_MESSAGES_HISTORY_LIMIT, HttpMethod,
    MAX_MESSAGES_HISTORY_LIMIT, QueryError, ROUTES, STATIC_EXTENSION_ROUTE_COUNT,
    build_channel_history_path, build_channel_messages_send_path, channel_messages_get_route_spec,
    channel_messages_post_route_spec, find,
};
use chord_core::rest::super_properties::SuperProperties;
use chord_core::rest::transport::{HttpError, HttpRequest, HttpResponse};
use net::{
    Http2ConnectionPool, Http2PoolConfig, LiveMessageSendOptions, RestTransport,
    classify_rest_http_error, parse_channel_messages_json, send_message_options_with_transport,
};
use state::AppState;
use ui::chat_view::{
    AttachmentUploadPhase, ChannelHistoryLoadState, ChatView, OptimisticSendPhase,
};

struct MockHttpTransport {
    requests: Mutex<Vec<HttpRequest>>,
    responses: Mutex<Vec<Result<HttpResponse, HttpError>>>,
}

impl MockHttpTransport {
    fn new(responses: Vec<Result<HttpResponse, HttpError>>) -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            responses: Mutex::new(responses),
        }
    }

    fn take_requests(&self) -> Vec<HttpRequest> {
        std::mem::take(&mut *self.requests.lock().expect("requests lock"))
    }
}

impl HttpTransport for MockHttpTransport {
    async fn send(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError> {
        self.requests
            .lock()
            .expect("requests lock")
            .push(request.clone());
        let mut guard = self.responses.lock().expect("responses lock");
        if guard.is_empty() {
            Ok(HttpResponse {
                status: 500,
                headers: Vec::new(),
                body: br#"{"message":"no mock response queued"}"#.to_vec(),
            })
        } else {
            guard.remove(0)
        }
    }
}

#[derive(Default)]
struct MockRestPoolTransport {
    requests: Mutex<Vec<(String, String, Vec<(String, String)>, Option<Vec<u8>>)>>,
    responses: Mutex<Vec<Result<HttpResponse, String>>>,
}

impl MockRestPoolTransport {
    fn new(responses: Vec<Result<HttpResponse, String>>) -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            responses: Mutex::new(responses),
        }
    }

    fn take_requests(&self) -> Vec<(String, String, Vec<(String, String)>, Option<Vec<u8>>)> {
        std::mem::take(&mut *self.requests.lock().expect("requests lock"))
    }
}

impl RestTransport for MockRestPoolTransport {
    fn send_request<'a>(
        &'a self,
        method: &'a str,
        target: &'a str,
        headers: &'a [(String, String)],
        body: Option<&'a [u8]>,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, String>> + Send + 'a>> {
        self.requests.lock().expect("requests lock").push((
            method.to_string(),
            target.to_string(),
            headers.to_vec(),
            body.map(|b| b.to_vec()),
        ));
        let next = {
            let mut guard = self.responses.lock().expect("responses lock");
            if guard.is_empty() {
                Err("no queued response".to_string())
            } else {
                guard.remove(0)
            }
        };
        Box::pin(async move { next })
    }
}

fn test_api_client() -> ApiClient {
    let sp = SuperProperties {
        os: "Linux".to_string(),
        browser: "Discord Client".to_string(),
        release_channel: "stable".to_string(),
        client_version: "1.0.162".to_string(),
        ..SuperProperties::default()
    };
    let auth = ApiAuth::from_token("user-live-token-w4-2-xyz", &sp, "ru", "Europe/Moscow")
        .expect("valid auth");
    ApiClient::new(ApiConfig {
        base: "https://discord.com/api/v9".to_string(),
        auth,
        ..ApiConfig::default()
    })
    .expect("valid client")
}

fn find_header<'a>(req: &'a HttpRequest, name: &str) -> Option<&'a str> {
    req.headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

// ============================================================================
// 1. Route Helpers & Snowflake / Pagination Boundary Validation
// ============================================================================

#[test]
fn t01_routes_catalog_and_message_send_history_helpers() {
    assert_eq!(CHANNEL_MESSAGES_PATH, "/channels/{id}/messages");
    assert_eq!(DEFAULT_MESSAGES_HISTORY_LIMIT, 50);
    assert_eq!(MAX_MESSAGES_HISTORY_LIMIT, 100);
    assert_eq!(MAX_MESSAGE_CONTENT_CHARS, 2000);
    assert_eq!(MAX_ATTACHMENTS_PER_MESSAGE, 10);
    assert_eq!(MAX_ATTACHMENT_SIZE_BYTES, 26_214_400);

    // Catalog invariants must remain strictly untouched
    assert_eq!(ROUTES.len(), 49);
    assert_eq!(CAPTURED_D02_ROUTE_COUNT, 47);
    assert_eq!(STATIC_EXTENSION_ROUTE_COUNT, 2);

    let post_spec = channel_messages_post_route_spec();
    assert_eq!(post_spec.method, HttpMethod::Post);
    assert_eq!(post_spec.path, CHANNEL_MESSAGES_PATH);
    assert_eq!(post_spec.body_keys, &["content", "nonce", "tts"]);
    assert!(post_spec.is_static_extension());
    assert_eq!(
        find(HttpMethod::Post, CHANNEL_MESSAGES_PATH),
        Some(post_spec)
    );

    let get_spec = channel_messages_get_route_spec();
    assert_eq!(get_spec.method, HttpMethod::Get);
    assert_eq!(get_spec.path, CHANNEL_MESSAGES_PATH);
    assert!(!get_spec.is_static_extension());
    assert_eq!(find(HttpMethod::Get, CHANNEL_MESSAGES_PATH), Some(get_spec));

    // Send path builder: valid Snowflakes 1..=u64::MAX
    assert_eq!(
        build_channel_messages_send_path("1056563701764857896"),
        Ok("/channels/1056563701764857896/messages".to_string())
    );
    assert_eq!(
        build_channel_messages_send_path("1"),
        Ok("/channels/1/messages".to_string())
    );
    assert_eq!(
        build_channel_messages_send_path("18446744073709551615"),
        Ok("/channels/18446744073709551615/messages".to_string())
    );

    // History path builder: default limit=50, explicit limit=1..=100, optional before cursor
    assert_eq!(
        build_channel_history_path("1056563701764857896", None, None),
        Ok("/channels/1056563701764857896/messages?limit=50".to_string())
    );
    assert_eq!(
        build_channel_history_path("1056563701764857896", Some("1056563701764857800"), Some(50)),
        Ok(
            "/channels/1056563701764857896/messages?limit=50&before=1056563701764857800"
                .to_string()
        )
    );
    assert_eq!(
        build_channel_history_path("18446744073709551615", Some("1"), Some(1)),
        Ok("/channels/18446744073709551615/messages?limit=1&before=1".to_string())
    );
    assert_eq!(
        build_channel_history_path(
            "1056563701764857896",
            Some("18446744073709551615"),
            Some(100)
        ),
        Ok(
            "/channels/1056563701764857896/messages?limit=100&before=18446744073709551615"
                .to_string()
        )
    );
}

#[test]
fn t02_routes_negative_and_edge_boundaries() {
    for bad_channel in [
        "",
        "0",
        "000",
        "-1",
        "18446744073709551616", // u64::MAX + 1
        "ch-abc",
        "../../etc/passwd",
        "123{id}",
    ] {
        assert!(
            build_channel_messages_send_path(bad_channel).is_err(),
            "send path must reject channel_id={bad_channel:?}"
        );
        assert!(
            build_channel_history_path(bad_channel, None, Some(50)).is_err(),
            "history path must reject channel_id={bad_channel:?}"
        );
    }

    for bad_cursor in ["", "0", "-5", "18446744073709551616", "not-a-snowflake"] {
        match build_channel_history_path("1056563701764857896", Some(bad_cursor), Some(50)) {
            Err(QueryError::InvalidParamValue { param, .. }) => assert_eq!(param, "before"),
            other => panic!("expected InvalidParamValue(before) for {bad_cursor:?}, got {other:?}"),
        }
    }

    for bad_limit in [0u32, 101, 500] {
        match build_channel_history_path("1056563701764857896", None, Some(bad_limit)) {
            Err(QueryError::InvalidParamValue { param, .. }) => assert_eq!(param, "limit"),
            other => panic!("expected InvalidParamValue(limit) for {bad_limit}, got {other:?}"),
        }
    }
}

// ============================================================================
// 2. ApiClient Live Message Send & Channel History Fetch Requests
// ============================================================================

#[test]
fn t03_api_client_build_send_message_request_positive_and_2000_char_boundary() {
    let client = test_api_client();

    // 1. Standard message with nonce, reply reference, and pre-uploaded attachment metadata
    let params = SendMessageParams {
        content: "Привет, мир! Проверка отправки REST v9.".to_string(),
        nonce: Some("1099887766554433221".to_string()),
        tts: false,
        message_reference: Some(MessageReferenceSpec {
            message_id: "1056563701764857800".to_string(),
            channel_id: Some("1056563701764857896".to_string()),
            guild_id: Some("1014537851418980372".to_string()),
            fail_if_not_exists: Some(false),
        }),
        attachments: vec![SendMessageAttachmentRef {
            id: 0,
            filename: "../../screenshots/diagram.png".to_string(),
            uploaded_filename: Some("a1b2/diagram.png".to_string()),
            description: Some("Architecture diagram".to_string()),
        }],
    };

    let req = client
        .build_send_message_request("1056563701764857896", &params)
        .expect("valid send message request");

    assert_eq!(req.method, HttpMethod::Post);
    assert_eq!(req.target, "/channels/1056563701764857896/messages");
    assert_eq!(find_header(&req, "Content-Type"), Some("application/json"));
    assert_eq!(
        find_header(&req, "Authorization"),
        Some("user-live-token-w4-2-xyz")
    );
    assert_eq!(find_header(&req, "X-Discord-Locale"), Some("ru"));
    assert_eq!(
        find_header(&req, "X-Discord-Timezone"),
        Some("Europe/Moscow")
    );
    assert_eq!(
        find_header(&req, "X-Debug-Options"),
        Some("bugReporterEnabled")
    );
    assert!(find_header(&req, "X-Super-Properties").is_some());
    assert!(find_header(&req, "Accept-Language").is_some());
    assert!(
        find_header(&req, "User-Agent").is_some(),
        "BrowserImpersonationProfile User-Agent must be attached"
    );

    let json: serde_json::Value =
        serde_json::from_slice(&req.body).expect("request body must be valid JSON");
    assert_eq!(json["content"], "Привет, мир! Проверка отправки REST v9.");
    assert_eq!(json["nonce"], "1099887766554433221");
    assert_eq!(json["tts"], false);
    assert_eq!(
        json["message_reference"]["message_id"],
        "1056563701764857800"
    );
    assert_eq!(
        json["message_reference"]["channel_id"],
        "1056563701764857896"
    );
    assert_eq!(json["message_reference"]["guild_id"], "1014537851418980372");
    assert_eq!(json["message_reference"]["fail_if_not_exists"], false);
    assert_eq!(json["attachments"][0]["id"], 0);
    assert_eq!(
        json["attachments"][0]["filename"], "diagram.png",
        "attachment filename traversal must be sanitized"
    );
    assert_eq!(
        json["attachments"][0]["uploaded_filename"],
        "a1b2/diagram.png"
    );
    assert_eq!(
        json["attachments"][0]["description"],
        "Architecture diagram"
    );

    // 2. Exact 2000 Unicode characters boundary accepted
    let exact_2000 = "я".repeat(MAX_MESSAGE_CONTENT_CHARS);
    assert_eq!(exact_2000.chars().count(), 2000);
    let params_2000 = SendMessageParams::text(exact_2000).with_nonce("999888777");
    assert!(
        client
            .build_send_message_request("1056563701764857896", &params_2000)
            .is_ok(),
        "exact 2000 UTF-8 characters must be accepted"
    );
}

#[test]
fn t04_api_client_build_send_message_request_negative_validation() {
    let client = test_api_client();

    // Empty content and 0 attachments rejected
    let empty_params = SendMessageParams::text("   \n\t  ");
    assert_eq!(
        client.build_send_message_request("1056563701764857896", &empty_params),
        Err(ChatRestError::EmptyMessage)
    );

    // 2001 chars rejected
    let over_2000 = "a".repeat(MAX_MESSAGE_CONTENT_CHARS + 1);
    assert_eq!(
        client
            .build_send_message_request("1056563701764857896", &SendMessageParams::text(over_2000)),
        Err(ChatRestError::ContentTooLong {
            len: 2001,
            max: 2000,
        })
    );

    // Invalid channel_id rejected
    assert_eq!(
        client.build_send_message_request("0", &SendMessageParams::text("hello")),
        Err(ChatRestError::InvalidChannelId("0".to_string()))
    );

    // Invalid nonce rejected (empty, >64 chars, control chars)
    for bad_nonce in ["", "   ", &"n".repeat(65), "123\r\n456"] {
        let p = SendMessageParams::text("hello").with_nonce(bad_nonce);
        assert!(
            matches!(
                client.build_send_message_request("1056563701764857896", &p),
                Err(ChatRestError::InvalidNonce(_))
            ),
            "expected InvalidNonce for {bad_nonce:?}"
        );
    }

    // Invalid message_reference.message_id rejected
    for bad_ref in ["", "0", "18446744073709551616", "msg-abc"] {
        let mut p = SendMessageParams::text("reply");
        p.message_reference = Some(MessageReferenceSpec {
            message_id: bad_ref.to_string(),
            channel_id: None,
            guild_id: None,
            fail_if_not_exists: None,
        });
        assert!(
            matches!(
                client.build_send_message_request("1056563701764857896", &p),
                Err(ChatRestError::InvalidMessageReference(_))
            ),
            "expected InvalidMessageReference for {bad_ref:?}"
        );
    }

    // >10 attachments rejected
    let mut p_11 = SendMessageParams::text("too many");
    p_11.attachments = (0..11)
        .map(|i| SendMessageAttachmentRef {
            id: i,
            filename: format!("file_{i}.png"),
            uploaded_filename: None,
            description: None,
        })
        .collect();
    assert!(
        client
            .build_send_message_request("1056563701764857896", &p_11)
            .is_err()
    );
}

#[tokio::test]
async fn t05_api_client_send_and_fetch_history_async_roundtrip_and_dedup() {
    let client = test_api_client();
    let history_json = r#"[
        {
            "id": "1056563701764857903",
            "channel_id": "1056563701764857896",
            "content": "Третье сообщение (самое новое)",
            "timestamp": "2026-11-14T12:03:00.000000+00:00",
            "edited_timestamp": "2026-11-14T12:03:30.000000+00:00",
            "tts": false,
            "pinned": true,
            "type": 0,
            "author": {"id": "2001", "username": "alice", "global_name": "Alice", "bot": false},
            "attachments": [
                {"id": "5001", "filename": "spec.pdf", "size": 4096, "url": "https://cdn.discordapp.com/attachments/1056563701764857896/5001/spec.pdf", "content_type": "application/pdf"}
            ]
        },
        {
            "id": "1056563701764857902",
            "channel_id": "1056563701764857896",
            "content": "Второе сообщение (дубль для проверки dedup)",
            "timestamp": "2026-11-14T12:02:00.000000+00:00",
            "tts": false,
            "pinned": false,
            "type": 0,
            "author": {"id": "2002", "username": "bob", "global_name": "Bob", "bot": false}
        },
        {
            "id": "1056563701764857902",
            "channel_id": "1056563701764857896",
            "content": "Второе сообщение (дубль для проверки dedup)",
            "timestamp": "2026-11-14T12:02:00.000000+00:00",
            "tts": false,
            "pinned": false,
            "type": 0,
            "author": {"id": "2002", "username": "bob", "global_name": "Bob", "bot": false}
        },
        {
            "id": "1056563701764857901",
            "channel_id": "1056563701764857896",
            "content": "Первое сообщение (самое старое)",
            "timestamp": "2026-11-14T12:01:00.000000+00:00",
            "tts": false,
            "pinned": false,
            "type": 0,
            "author": {"id": "2001", "username": "alice", "global_name": "Alice", "bot": false}
        }
    ]"#;

    let sent_response_json = r#"{
        "id": "1056563701764857950",
        "channel_id": "1056563701764857896",
        "content": "Ответ с цитированием",
        "nonce": "nonce-777888999",
        "timestamp": "2026-11-14T12:05:00.000000+00:00",
        "tts": false,
        "pinned": false,
        "type": 19,
        "author": {"id": "1001", "username": "rustery", "global_name": "Rustery", "bot": false},
        "referenced_message": {"id": "1056563701764857903"},
        "attachments": []
    }"#;

    let transport = MockHttpTransport::new(vec![
        Ok(HttpResponse {
            status: 200,
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            body: history_json.as_bytes().to_vec(),
        }),
        Ok(HttpResponse {
            status: 200,
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            body: sent_response_json.as_bytes().to_vec(),
        }),
    ]);

    // 1. Fetch history page (default limit=50)
    let page = client
        .fetch_channel_messages_page(&transport, "1056563701764857896", None, None)
        .await
        .expect("fetch history 200 OK");
    assert_eq!(page.len(), 3, "duplicate message id must be deduplicated");
    assert_eq!(page[0].id, "1056563701764857901", "oldest first");
    assert_eq!(page[1].id, "1056563701764857902");
    assert_eq!(page[2].id, "1056563701764857903", "newest last");
    assert!(page[2].pinned);
    assert_eq!(page[2].attachments.len(), 1);
    assert_eq!(page[2].attachments[0].filename, "spec.pdf");

    // 2. Send live message with reply & nonce
    let mut send_params = SendMessageParams::text("Ответ с цитированием")
        .with_nonce("nonce-777888999")
        .with_reply("1056563701764857903");
    send_params.tts = false;
    let sent = client
        .send_channel_message(&transport, "1056563701764857896", &send_params)
        .await
        .expect("send message 200 OK");
    assert_eq!(sent.id, "1056563701764857950");
    assert_eq!(sent.nonce.as_deref(), Some("nonce-777888999"));
    assert_eq!(
        sent.referenced_message_id.as_deref(),
        Some("1056563701764857903")
    );

    let recorded = transport.take_requests();
    assert_eq!(recorded.len(), 2);
    assert_eq!(recorded[0].method, HttpMethod::Get);
    assert_eq!(
        recorded[0].target,
        "/channels/1056563701764857896/messages?limit=50"
    );
    assert_eq!(recorded[1].method, HttpMethod::Post);
    assert_eq!(recorded[1].target, "/channels/1056563701764857896/messages");
}

// ============================================================================
// 3. HTTP Error Mapping (400, 401, 403, 404, 413, 429 Retry-After, 5xx)
// ============================================================================

#[tokio::test]
async fn t06_http_error_mapping_400_401_403_404_413_429_5xx() {
    let client = test_api_client();
    let params = SendMessageParams::text("hello").with_nonce("111222333");

    // 400 Bad Request
    let t_400 = MockHttpTransport::new(vec![Ok(HttpResponse {
        status: 400,
        headers: Vec::new(),
        body: br#"{"code":50006,"message":"Cannot send an empty message"}"#.to_vec(),
    })]);
    assert_eq!(
        client
            .send_channel_message(&t_400, "1056563701764857896", &params)
            .await,
        Err(ChatRestError::BadRequest {
            code: Some(50006),
            message: Some("Cannot send an empty message".to_string()),
        })
    );

    // 401 Unauthorized
    let t_401 = MockHttpTransport::new(vec![Ok(HttpResponse {
        status: 401,
        headers: Vec::new(),
        body: br#"{"message":"401: Unauthorized","code":0}"#.to_vec(),
    })]);
    assert_eq!(
        client
            .fetch_channel_messages_page(&t_401, "1056563701764857896", None, Some(50))
            .await,
        Err(ChatRestError::Unauthorized)
    );

    // 403 Forbidden
    let t_403 = MockHttpTransport::new(vec![Ok(HttpResponse {
        status: 403,
        headers: Vec::new(),
        body: br#"{"code":50013,"message":"Missing Permissions"}"#.to_vec(),
    })]);
    assert_eq!(
        client
            .send_channel_message(&t_403, "1056563701764857896", &params)
            .await,
        Err(ChatRestError::Forbidden {
            message: Some("Missing Permissions".to_string()),
        })
    );

    // 404 Not Found
    let t_404 = MockHttpTransport::new(vec![Ok(HttpResponse {
        status: 404,
        headers: Vec::new(),
        body: br#"{"code":10003,"message":"Unknown Channel"}"#.to_vec(),
    })]);
    assert_eq!(
        client
            .fetch_channel_messages_page(&t_404, "1056563701764857896", None, Some(50))
            .await,
        Err(ChatRestError::NotFound {
            message: Some("Unknown Channel".to_string()),
        })
    );

    // 413 Payload Too Large
    let t_413 = MockHttpTransport::new(vec![Ok(HttpResponse {
        status: 413,
        headers: Vec::new(),
        body: br#"{"code":40005,"message":"Request entity too large"}"#.to_vec(),
    })]);
    assert_eq!(
        client
            .send_channel_message(&t_413, "1056563701764857896", &params)
            .await,
        Err(ChatRestError::PayloadTooLarge)
    );

    // 429 Too Many Requests (Retry-After header & JSON body)
    let t_429 = MockHttpTransport::new(vec![Ok(HttpResponse {
        status: 429,
        headers: vec![("Retry-After".to_string(), "1.75".to_string())],
        body: br#"{"message":"You are being rate limited.","retry_after":1.75,"global":false}"#
            .to_vec(),
    })]);
    assert_eq!(
        client
            .send_channel_message(&t_429, "1056563701764857896", &params)
            .await,
        Err(ChatRestError::RateLimited {
            retry_after: Duration::from_millis(1750),
        })
    );

    // 502 Bad Gateway (5xx)
    let t_502 = MockHttpTransport::new(vec![Ok(HttpResponse {
        status: 502,
        headers: Vec::new(),
        body: b"Bad Gateway".to_vec(),
    })]);
    assert_eq!(
        client
            .fetch_channel_messages_page(&t_502, "1056563701764857896", None, Some(50))
            .await,
        Err(ChatRestError::Server { status: 502 })
    );

    // Direct classify_rest_http_error check in chord-gpui::net
    assert_eq!(
        classify_rest_http_error(404, &[], br#"{"message":"Unknown Channel"}"#),
        ChatRestError::NotFound {
            message: Some("Unknown Channel".to_string()),
        }
    );
}

// ============================================================================
// 4. chord-gpui::net Pooled History Fetch, Pagination & Message Send
// ============================================================================

#[tokio::test]
async fn t07_net_pooled_history_and_send_with_attachments_and_reply() {
    let pool = Http2ConnectionPool::new();
    let mut cfg = Http2PoolConfig::default();
    cfg.initial_backoff = Duration::from_millis(1);

    let history_body = r#"[
        {
            "id": "1056563701764857820",
            "channel_id": "1056563701764857896",
            "content": "Новое сообщение с вложением",
            "timestamp": "2026-11-14T15:00:00.000000+00:00",
            "pinned": false,
            "author": {"id": "u-1", "username": "alice", "global_name": "Alice"},
            "attachments": [
                {"id": "att-900", "filename": "trace.json", "size": 512, "url": "https://cdn.discordapp.com/attachments/1056563701764857896/att-900/trace.json"}
            ],
            "reactions": [
                {"count": 3, "me": true, "emoji": {"name": "🔥"}}
            ]
        },
        {
            "id": "1056563701764857810",
            "channel_id": "1056563701764857896",
            "content": "Старое сообщение",
            "timestamp": "2026-11-14T14:55:00.000000+00:00",
            "pinned": true,
            "author": {"id": "u-2", "username": "bob", "global_name": "Bob"}
        }
    ]"#;

    let parsed = parse_channel_messages_json("1056563701764857896", history_body.as_bytes())
        .expect("parse channel messages json");
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].id, "1056563701764857810");
    assert!(parsed[0].is_pinned);
    assert_eq!(parsed[1].id, "1056563701764857820");
    assert_eq!(parsed[1].attachments.len(), 1);
    assert_eq!(parsed[1].attachments[0].filename, "trace.json");
    assert_eq!(parsed[1].attachments[0].size_bytes, 512);
    assert_eq!(parsed[1].reactions.len(), 1);
    assert_eq!(parsed[1].reactions[0].emoji, "🔥");
    assert_eq!(parsed[1].reactions[0].count, 3);
    assert!(parsed[1].reactions[0].me);

    // Pooled send with options (reply + multipart attachment + nonce)
    let sent_body = r#"{
        "id": "1056563701764857899",
        "channel_id": "1056563701764857896",
        "content": "Отправка с файлом и ответом",
        "nonce": "opt-nonce-42",
        "timestamp": "2026-11-14T15:01:00.000000+00:00",
        "author": {"id": "u-me", "username": "rustery", "global_name": "Rustery"},
        "attachments": [
            {"id": "att-901", "filename": "screenshot.png", "size": 4, "url": "https://cdn.discordapp.com/attachments/1056563701764857896/att-901/screenshot.png"}
        ]
    }"#;

    let transport = MockRestPoolTransport::new(vec![Ok(HttpResponse {
        status: 200,
        headers: Vec::new(),
        body: sent_body.as_bytes().to_vec(),
    })]);

    let options = LiveMessageSendOptions {
        content: "Отправка с файлом и ответом".to_string(),
        nonce: Some("opt-nonce-42".to_string()),
        tts: false,
        reply_to_message_id: Some("1056563701764857820".to_string()),
        attachments: vec![MultipartUploadFile {
            field_index: 0,
            filename: "../../screenshot.png".to_string(),
            content_type: "image/png".to_string(),
            bytes: vec![0x89, b'P', b'N', b'G'],
        }],
    };

    let (msg, echoed_nonce, metrics) = send_message_options_with_transport(
        &transport,
        &pool,
        &cfg,
        "live-token",
        "1056563701764857896",
        &options,
    )
    .await
    .expect("send_message_options_with_transport 200 OK");

    assert_eq!(metrics.attempts, 1);
    assert_eq!(msg.id, "1056563701764857899");
    assert_eq!(echoed_nonce.as_deref(), Some("opt-nonce-42"));
    assert_eq!(msg.attachments.len(), 1);
    assert_eq!(msg.attachments[0].filename, "screenshot.png");

    let recorded = transport.take_requests();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].0, "POST");
    assert_eq!(recorded[0].1, "/channels/1056563701764857896/messages");
    let body_text = String::from_utf8_lossy(recorded[0].3.as_ref().expect("body"));
    assert!(body_text.contains("name=\"payload_json\""));
    assert!(body_text.contains("\"nonce\":\"opt-nonce-42\""));
    assert!(body_text.contains("\"message_reference\":{\"message_id\":\"1056563701764857820\""));
    assert!(body_text.contains("filename=\"screenshot.png\""));
}

// ============================================================================
// 5. ChatView Channel Switch History Load State Machine (Loading -> Loaded / Empty / Error)
// ============================================================================

fn controller_test_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

#[tokio::test]
async fn t08_chat_view_channel_switch_history_states_loading_loaded_empty_and_error() {
    let _serial_guard = controller_test_lock().lock().await;
    let mut state = AppState::empty();
    ChatView::reset_controller_state();

    let pool = Http2ConnectionPool::new();
    let mut cfg = Http2PoolConfig::default();
    cfg.initial_backoff = Duration::from_millis(1);
    cfg.max_attempts = 1;

    let channel_loaded = "1056563701764857896";
    let channel_empty = "1056563701764857897";
    let channel_forbidden = "1056563701764857898";

    // 1. Begin channel switch transitions immediately to Loading
    ChatView::begin_channel_switch(&mut state, channel_loaded);
    assert_eq!(state.selected_channel_id.as_deref(), Some(channel_loaded));
    assert_eq!(
        ChatView::channel_history_state(channel_loaded),
        ChannelHistoryLoadState::Loading {
            channel_id: channel_loaded.to_string(),
            before: None,
            limit: 50,
        }
    );

    // 2. Complete history load with 2 messages -> Loaded
    let msgs_json = r#"[
        {
            "id": "1056563701764857002",
            "channel_id": "1056563701764857896",
            "content": "Второе сообщение",
            "timestamp": "2026-11-14T10:02:00.000000+00:00",
            "author": {"id": "u-10", "username": "dev_one", "global_name": "Dev One"}
        },
        {
            "id": "1056563701764857001",
            "channel_id": "1056563701764857896",
            "content": "Первое сообщение",
            "timestamp": "2026-11-14T10:01:00.000000+00:00",
            "author": {"id": "u-11", "username": "dev_two", "global_name": "Dev Two"}
        }
    ]"#;
    let t_loaded = MockRestPoolTransport::new(vec![Ok(HttpResponse {
        status: 200,
        headers: Vec::new(),
        body: msgs_json.as_bytes().to_vec(),
    })]);

    let loaded_state = ChatView::switch_channel_with_transport(
        &mut state,
        &t_loaded,
        &pool,
        &cfg,
        "live-token",
        channel_loaded,
    )
    .await
    .expect("switch channel loaded");

    assert_eq!(
        loaded_state,
        ChannelHistoryLoadState::Loaded {
            channel_id: channel_loaded.to_string(),
            count: 2,
            has_more: false,
        }
    );
    assert_eq!(
        ChatView::channel_history_state(channel_loaded),
        loaded_state
    );
    let feed = state.messages.get(channel_loaded).expect("channel feed");
    assert_eq!(feed.len(), 2);
    assert_eq!(feed[0].id, "1056563701764857001");
    assert_eq!(feed[1].id, "1056563701764857002");
    assert!(
        state.has_reached_history_top(channel_loaded),
        "fewer than 50 messages means top of history reached"
    );

    // 3. Switch to empty channel (0 messages) -> Empty
    let t_empty = MockRestPoolTransport::new(vec![Ok(HttpResponse {
        status: 200,
        headers: Vec::new(),
        body: b"[]".to_vec(),
    })]);
    let empty_state = ChatView::switch_channel_with_transport(
        &mut state,
        &t_empty,
        &pool,
        &cfg,
        "live-token",
        channel_empty,
    )
    .await
    .expect("switch channel empty");
    assert_eq!(
        empty_state,
        ChannelHistoryLoadState::Empty {
            channel_id: channel_empty.to_string(),
        }
    );
    assert!(state.has_reached_history_top(channel_empty));
    assert!(
        state
            .messages
            .get(channel_empty)
            .is_some_and(|m| m.is_empty())
    );

    // 4. Switch to forbidden channel (HTTP 403) -> Error state recorded
    let t_403 = MockRestPoolTransport::new(vec![Ok(HttpResponse {
        status: 403,
        headers: Vec::new(),
        body: br#"{"code":50013,"message":"Missing Permissions"}"#.to_vec(),
    })]);
    let err = ChatView::switch_channel_with_transport(
        &mut state,
        &t_403,
        &pool,
        &cfg,
        "live-token",
        channel_forbidden,
    )
    .await
    .expect_err("403 must return Err");
    assert_eq!(
        err,
        ChatRestError::Forbidden {
            message: Some("Missing Permissions".to_string()),
        }
    );
    match ChatView::channel_history_state(channel_forbidden) {
        ChannelHistoryLoadState::Error {
            channel_id,
            status_code,
            error,
            ..
        } => {
            assert_eq!(channel_id, channel_forbidden);
            assert_eq!(status_code, Some(403));
            assert!(error.contains("Missing Permissions"));
        }
        other => panic!("expected ChannelHistoryLoadState::Error, got {other:?}"),
    }
}

// ============================================================================
// 6. ChatView Pagination with Transport & Scroll Position Preservation
// ============================================================================

#[tokio::test]
async fn t09_chat_view_pagination_before_cursor_and_scroll_preservation() {
    let _serial_guard = controller_test_lock().lock().await;
    let mut state = AppState::empty();
    ChatView::reset_controller_state();

    let channel_id = "1056563701764857896";
    state.selected_channel_id = Some(channel_id.to_string());
    state.messages.insert(
        channel_id.to_string(),
        vec![
            state::Message {
                id: "1056563701764857500".to_string(),
                channel_id: channel_id.to_string(),
                author: state::UserSummary {
                    id: "u-1".to_string(),
                    username: "alice".to_string(),
                    global_name: Some("Alice".to_string()),
                    avatar: None,
                    is_bot: false,
                },
                content: "Existing message 500".to_string(),
                timestamp: "Сегодня в 12:00".to_string(),
                edited_timestamp: None,
                attachments: Vec::new(),
                reactions: Vec::new(),
                is_pinned: false,
                message_kind: state::MessageKind::Default,
            },
            state::Message {
                id: "1056563701764857501".to_string(),
                channel_id: channel_id.to_string(),
                author: state::UserSummary {
                    id: "u-2".to_string(),
                    username: "bob".to_string(),
                    global_name: Some("Bob".to_string()),
                    avatar: None,
                    is_bot: false,
                },
                content: "Existing message 501".to_string(),
                timestamp: "Сегодня в 12:01".to_string(),
                edited_timestamp: None,
                attachments: Vec::new(),
                reactions: Vec::new(),
                is_pinned: false,
                message_kind: state::MessageKind::Default,
            },
        ],
    );

    let older_page_json = r#"[
        {
            "id": "1056563701764857499",
            "channel_id": "1056563701764857896",
            "content": "Older message 499",
            "timestamp": "2026-11-14T11:59:00.000000+00:00",
            "author": {"id": "u-1", "username": "alice"}
        },
        {
            "id": "1056563701764857498",
            "channel_id": "1056563701764857896",
            "content": "Older message 498",
            "timestamp": "2026-11-14T11:58:00.000000+00:00",
            "author": {"id": "u-2", "username": "bob"}
        }
    ]"#;

    let pool = Http2ConnectionPool::new();
    let cfg = Http2PoolConfig::default();
    let transport = MockRestPoolTransport::new(vec![Ok(HttpResponse {
        status: 200,
        headers: Vec::new(),
        body: older_page_json.as_bytes().to_vec(),
    })]);

    let prepended = ChatView::paginate_channel_history_with_transport(
        &mut state,
        &transport,
        &pool,
        &cfg,
        "live-token",
        channel_id,
    )
    .await
    .expect("paginate older messages");

    assert_eq!(prepended, 2);
    let feed = state.messages.get(channel_id).expect("feed");
    assert_eq!(feed.len(), 4);
    assert_eq!(feed[0].id, "1056563701764857498");
    assert_eq!(feed[1].id, "1056563701764857499");
    assert_eq!(feed[2].id, "1056563701764857500");
    assert_eq!(feed[3].id, "1056563701764857501");
    assert_eq!(state.chat_scroll_handle.target_scroll_item(), Some(2));
    assert!(
        state.has_reached_history_top(channel_id),
        "2 < 50 messages means history top reached"
    );

    let reqs = transport.take_requests();
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].0, "GET");
    assert_eq!(
        reqs[0].1,
        "/channels/1056563701764857896/messages?before=1056563701764857500&limit=50"
    );
}

// ============================================================================
// 7. Optimistic Send Lifecycle (Pending -> Confirmed / Failed Retry & Nonce Dedup)
// ============================================================================

#[tokio::test]
async fn t10_chat_view_optimistic_send_pending_to_confirmed_and_gateway_nonce_dedup() {
    let _serial_guard = controller_test_lock().lock().await;
    let mut state = AppState::empty();
    ChatView::reset_controller_state();

    let channel_id = "1056563701764857896";
    state.selected_channel_id = Some(channel_id.to_string());
    state.draft_text = "Оптимистичное сообщение".to_string();

    // Set reply target
    ChatView::set_reply_target(channel_id, "1056563701764857001", "Alice");
    assert_eq!(
        ChatView::active_reply_target(channel_id).map(|r| r.message_id),
        Some("1056563701764857001".to_string())
    );

    // Stage 1 attachment
    ChatView::stage_attachment("report.txt", "text/plain", b"hello report".to_vec())
        .expect("stage attachment");

    // Step 1: Begin optimistic send -> draft cleared, optimistic message inserted with Pending phase
    let pending = ChatView::begin_optimistic_send(&mut state, Some("nonce-opt-100".to_string()))
        .expect("begin optimistic send");
    assert_eq!(pending.nonce, "nonce-opt-100");
    assert_eq!(pending.channel_id, channel_id);
    assert_eq!(pending.content, "Оптимистичное сообщение");
    assert_eq!(
        pending.reply_to_message_id.as_deref(),
        Some("1056563701764857001")
    );
    assert_eq!(pending.phase, OptimisticSendPhase::Pending);
    assert!(
        state.draft_text.is_empty(),
        "draft text must be cleared on optimistic send"
    );

    let feed_pending = state.messages.get(channel_id).expect("feed while pending");
    assert_eq!(feed_pending.len(), 1);
    assert_eq!(feed_pending[0].id, "opt-nonce-opt-100");
    assert_eq!(feed_pending[0].content, "Оптимистичное сообщение");
    assert_eq!(feed_pending[0].attachments.len(), 1);

    // Step 2: Simulate Gateway MESSAGE_CREATE arriving with the same nonce BEFORE or AFTER REST response
    let server_msg_json = r#"{
        "id": "1056563701764857999",
        "channel_id": "1056563701764857896",
        "content": "Оптимистичное сообщение",
        "nonce": "nonce-opt-100",
        "timestamp": "2026-11-14T15:10:00.000000+00:00",
        "author": {"id": "123456789012345678", "username": "Rustery", "global_name": "Rustery Developer"},
        "attachments": [
            {"id": "att-real-1", "filename": "report.txt", "size": 12, "url": "https://cdn.discordapp.com/attachments/1056563701764857896/att-real-1/report.txt"}
        ]
    }"#;

    let pool = Http2ConnectionPool::new();
    let cfg = Http2PoolConfig::default();
    let transport = MockRestPoolTransport::new(vec![Ok(HttpResponse {
        status: 200,
        headers: Vec::new(),
        body: server_msg_json.as_bytes().to_vec(),
    })]);

    let confirmed = ChatView::complete_optimistic_send_with_transport(
        &mut state,
        &transport,
        &pool,
        &cfg,
        "live-token",
        &pending.nonce,
    )
    .await
    .expect("complete optimistic send 200 OK");

    assert_eq!(confirmed.id, "1056563701764857999");
    assert_eq!(
        ChatView::optimistic_message_phase(&pending.nonce),
        Some(OptimisticSendPhase::Confirmed {
            message_id: "1056563701764857999".to_string(),
        })
    );
    assert_eq!(
        ChatView::staged_attachment_count(),
        0,
        "staged attachments cleared on confirmation"
    );
    assert!(
        ChatView::active_reply_target(channel_id).is_none(),
        "reply target cleared on send"
    );

    // Feed must contain exactly 1 message upgraded in-place to the server ID
    let feed_after = state.messages.get(channel_id).expect("feed after confirm");
    assert_eq!(
        feed_after.len(),
        1,
        "optimistic message must be replaced in-place, not duplicated"
    );
    assert_eq!(feed_after[0].id, "1056563701764857999");
    assert_eq!(feed_after[0].attachments[0].id, "att-real-1");

    // Step 3: Reconciling a duplicate Gateway MESSAGE_CREATE with the same nonce or id must NOT duplicate
    let reconciled = ChatView::reconcile_incoming_message(
        &mut state,
        channel_id,
        confirmed.clone(),
        Some("nonce-opt-100"),
    );
    assert!(reconciled, "reconcile should recognize existing nonce/id");
    assert_eq!(
        state.messages.get(channel_id).unwrap().len(),
        1,
        "duplicate Gateway MESSAGE_CREATE with same nonce must not create a second message"
    );
}

#[tokio::test]
async fn t11_chat_view_optimistic_send_failure_preserves_state_and_retries_with_same_nonce() {
    let _serial_guard = controller_test_lock().lock().await;
    let mut state = AppState::empty();
    ChatView::reset_controller_state();

    let channel_id = "1056563701764857896";
    state.selected_channel_id = Some(channel_id.to_string());
    state.draft_text = "Сообщение при 429/503 сбое".to_string();

    ChatView::stage_attachment("retry_doc.pdf", "application/pdf", b"%PDF-1.7".to_vec())
        .expect("stage attachment");

    let pool = Http2ConnectionPool::new();
    let mut cfg = Http2PoolConfig::default();
    cfg.max_attempts = 1; // 1 attempt so first call fails immediately with 429

    let transport = MockRestPoolTransport::new(vec![
        Ok(HttpResponse {
            status: 429,
            headers: vec![("Retry-After".to_string(), "1.5".to_string())],
            body: br#"{"message":"You are being rate limited.","retry_after":1.5}"#.to_vec(),
        }),
        Ok(HttpResponse {
            status: 200,
            headers: Vec::new(),
            body: r#"{
                "id": "1056563701764858000",
                "channel_id": "1056563701764857896",
                "content": "Сообщение при 429/503 сбое",
                "nonce": "nonce-retry-555",
                "timestamp": "2026-11-14T15:12:00.000000+00:00",
                "author": {"id": "123456789012345678", "username": "Rustery"},
                "attachments": [
                    {"id": "att-555", "filename": "retry_doc.pdf", "size": 8, "url": "https://cdn.discordapp.com/attachments/1056563701764857896/att-555/retry_doc.pdf"}
                ]
            }"#
            .as_bytes()
            .to_vec(),
        }),
    ]);

    // First attempt fails with 429 RateLimited
    let err = ChatView::submit_live_message_with_transport(
        &mut state,
        &transport,
        &pool,
        &cfg,
        "live-token",
        Some("nonce-retry-555".to_string()),
    )
    .await
    .expect_err("first send must fail with 429");

    assert_eq!(
        err,
        ChatRestError::RateLimited {
            retry_after: Duration::from_millis(1500),
        }
    );

    // Optimistic message is in Failed phase, staged attachment is preserved in Failed phase
    match ChatView::optimistic_message_phase("nonce-retry-555") {
        Some(OptimisticSendPhase::Failed {
            status_code,
            retry_after_ms,
            ..
        }) => {
            assert_eq!(status_code, Some(429));
            assert_eq!(retry_after_ms, Some(1500));
        }
        other => panic!("expected OptimisticSendPhase::Failed, got {other:?}"),
    }
    let snap = ChatView::composer_snapshot();
    assert_eq!(
        snap.count(),
        1,
        "staged attachment must be preserved on failed send"
    );
    assert!(matches!(
        snap.staged[0].phase,
        AttachmentUploadPhase::Failed(_)
    ));

    // Retry failed message -> reuses same nonce "nonce-retry-555" and succeeds
    let retried_msg = ChatView::retry_failed_message_with_transport(
        &mut state,
        &transport,
        &pool,
        &cfg,
        "live-token",
        "nonce-retry-555",
    )
    .await
    .expect("retry failed message 200 OK");

    assert_eq!(retried_msg.id, "1056563701764858000");
    assert_eq!(
        ChatView::optimistic_message_phase("nonce-retry-555"),
        Some(OptimisticSendPhase::Confirmed {
            message_id: "1056563701764858000".to_string(),
        })
    );
    assert_eq!(ChatView::staged_attachment_count(), 0);

    let feed = state.messages.get(channel_id).expect("feed");
    assert_eq!(
        feed.len(),
        1,
        "retry must upgrade in-place without duplication"
    );
    assert_eq!(feed[0].id, "1056563701764858000");

    // Verify both requests sent identical nonce "nonce-retry-555"
    let reqs = transport.take_requests();
    assert_eq!(reqs.len(), 2);
    let body1 = String::from_utf8_lossy(reqs[0].3.as_ref().unwrap());
    let body2 = String::from_utf8_lossy(reqs[1].3.as_ref().unwrap());
    assert!(body1.contains("\"nonce\":\"nonce-retry-555\""));
    assert!(body2.contains("\"nonce\":\"nonce-retry-555\""));
}
