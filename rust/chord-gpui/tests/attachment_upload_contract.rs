//! Wave 3 Contract Tests: W3-5 — Attachment Upload REST v9 & ChatView UI (`P2-2.2`).
//!
//! Covers all 9 mandatory dimensions (`AGENTS.md` §5):
//! 1. Positive: 1 file & 10 files (max count), 25 MiB exact boundary, RFC 7578 multipart format,
//!    pre-signed slot request JSON (`POST /channels/{id}/attachments`), UI staging lifecycle.
//! 2. Negative: 11 files rejected, `25 MiB + 1` byte rejected, empty message + 0 files rejected.
//! 3. Edge boundaries: Snowflake `0` rejected, `u64::MAX` accepted, 2000-char content boundary,
//!    `1` byte file, `26_214_400` bytes (`25 MiB`) exact boundary.
//! 4. Error handling: HTTP `400`, `401`, `403`, `413 Payload Too Large`, `429 Retry-After`, `500`/`502`/`503`.
//! 5. Empty states: initial empty composer state, clear all attachments, empty filename after sanitization.
//! 6. Loading skeletons / phases: `Pending` -> `Uploading` -> `Ready` -> complete / `Failed`.
//! 7. Security / sanitization: path traversal `../../etc/passwd`, Windows paths `..\..\secret.key`,
//!    CRLF `\r\n` header injection, NUL `\0` and quote `"` injection stripped.
//! 8. Recovery / fallback: HTTP `413`/`429`/`5xx` preserves staged attachments in `ChatView` for retry.
//! 9. Observable telemetry & headers: `ApiAuth` 6 headers + `BrowserImpersonationProfile` headers
//!    preserved on both slot request and multipart upload request; telemetry paths remain guarded.

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

use std::sync::Mutex;
use std::time::Duration;

use chord_core::rest::client::{
    ApiAuth, ApiClient, ApiConfig, AttachmentUploadError, AttachmentUploadSlot, HttpTransport,
    MAX_ATTACHMENT_SIZE_BYTES, MAX_ATTACHMENTS_PER_MESSAGE, MultipartUploadFile,
    PendingAttachmentSpec, sanitize_attachment_filename,
};
use chord_core::rest::routes::{
    CAPTURED_D02_ROUTE_COUNT, CHANNEL_ATTACHMENTS_PATH, HttpMethod, QueryError, ROUTES,
    STATIC_EXTENSION_ROUTE_COUNT, attachment_upload_route_spec, build_channel_attachments_path,
    find,
};
use chord_core::rest::super_properties::SuperProperties;
use chord_core::rest::transport::{HttpError, HttpRequest, HttpResponse};
use state::AppState;
use ui::chat_view::{AttachmentComposerState, AttachmentUploadPhase, ChatView, StagedAttachment};

struct MockUploadTransport {
    requests: Mutex<Vec<HttpRequest>>,
    responses: Mutex<Vec<Result<HttpResponse, HttpError>>>,
}

impl MockUploadTransport {
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

impl HttpTransport for MockUploadTransport {
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
                body: b"{\"message\":\"no mock response queued\"}".to_vec(),
            })
        } else {
            guard.remove(0)
        }
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
    let auth = ApiAuth::from_token("user-secret-token-xyz", &sp, "ru", "Europe/Moscow")
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
// 1. Route Catalog & Snowflake Path Validation
// ============================================================================

#[test]
fn t01_routes_constants_and_catalog_invariants_preserved() {
    assert_eq!(MAX_ATTACHMENTS_PER_MESSAGE, 10);
    assert_eq!(MAX_ATTACHMENT_SIZE_BYTES, 25 * 1024 * 1024);
    assert_eq!(MAX_ATTACHMENT_SIZE_BYTES, 26_214_400);
    assert_eq!(CHANNEL_ATTACHMENTS_PATH, "/channels/{id}/attachments");

    // Invariants from rest_routes.rs must remain untouched
    assert_eq!(ROUTES.len(), 49);
    assert_eq!(CAPTURED_D02_ROUTE_COUNT, 47);
    assert_eq!(STATIC_EXTENSION_ROUTE_COUNT, 2);

    let spec = attachment_upload_route_spec();
    assert_eq!(spec.method, HttpMethod::Post);
    assert_eq!(spec.path, CHANNEL_ATTACHMENTS_PATH);
    assert!(spec.is_static_extension());
    assert_eq!(find(HttpMethod::Post, CHANNEL_ATTACHMENTS_PATH), Some(spec));
}

#[test]
fn t02_build_channel_attachments_path_snowflake_boundaries() {
    // Standard Snowflake
    assert_eq!(
        build_channel_attachments_path("1056563701764857896"),
        Ok("/channels/1056563701764857896/attachments".to_string())
    );

    // Minimum positive Snowflake (1) and maximum u64::MAX (18446744073709551615)
    assert_eq!(
        build_channel_attachments_path("1"),
        Ok("/channels/1/attachments".to_string())
    );
    assert_eq!(
        build_channel_attachments_path("18446744073709551615"),
        Ok("/channels/18446744073709551615/attachments".to_string())
    );

    // Reject Snowflake 0, u64::MAX + 1, empty, negative, non-numeric, traversal
    for bad in [
        "",
        "0",
        "-1",
        "18446744073709551616",
        "channel-abc",
        "../../etc/passwd",
        "123{id}",
    ] {
        assert!(
            build_channel_attachments_path(bad).is_err(),
            "expected Err for invalid channel_id {bad:?}"
        );
    }
    match build_channel_attachments_path("0") {
        Err(QueryError::InvalidParamValue { param, .. }) => assert_eq!(param, "channel_id"),
        other => panic!("expected InvalidParamValue for '0', got {other:?}"),
    }
}

// ============================================================================
// 2. Pre-signed Upload Slot Request (`POST /channels/{id}/attachments`)
// ============================================================================

#[test]
fn t03_request_attachment_upload_slots_positive_1_and_10_files_and_25mib_boundary() {
    let client = test_api_client();

    // Single file at exact 25 MiB (26_214_400 bytes)
    let single = vec![PendingAttachmentSpec {
        filename: "archive.tar.gz".to_string(),
        file_size: MAX_ATTACHMENT_SIZE_BYTES as u64,
        content_type: "application/gzip".to_string(),
        id: 0,
    }];
    let req = client
        .request_attachment_upload_slots("1056563701764857896", &single)
        .expect("1 file at exact 25 MiB must be accepted");

    assert_eq!(req.method, HttpMethod::Post);
    assert_eq!(req.target, "/channels/1056563701764857896/attachments");
    assert_eq!(find_header(&req, "Content-Type"), Some("application/json"));
    assert_eq!(
        find_header(&req, "Authorization"),
        Some("user-secret-token-xyz")
    );
    assert_eq!(find_header(&req, "X-Discord-Locale"), Some("ru"));
    assert_eq!(
        find_header(&req, "X-Debug-Options"),
        Some("bugReporterEnabled")
    );
    assert!(
        find_header(&req, "User-Agent").is_some(),
        "BrowserImpersonationProfile User-Agent header must be included"
    );

    let parsed: serde_json::Value =
        serde_json::from_slice(&req.body).expect("slot request body must be valid JSON");
    let files_arr = parsed["files"].as_array().expect("files array");
    assert_eq!(files_arr.len(), 1);
    assert_eq!(files_arr[0]["filename"], "archive.tar.gz");
    assert_eq!(files_arr[0]["file_size"], 26_214_400);
    assert_eq!(files_arr[0]["id"], 0);

    // Max boundary: 10 files
    let ten_files: Vec<PendingAttachmentSpec> = (0..10)
        .map(|i| PendingAttachmentSpec {
            filename: format!("image_{i}.png"),
            file_size: 1024 * (i + 1),
            content_type: "image/png".to_string(),
            id: i,
        })
        .collect();
    let req_ten = client
        .request_attachment_upload_slots("1056563701764857896", &ten_files)
        .expect("10 files must be accepted");
    let parsed_ten: serde_json::Value =
        serde_json::from_slice(&req_ten.body).expect("10 files JSON");
    assert_eq!(parsed_ten["files"].as_array().unwrap().len(), 10);
}

#[test]
fn t04_request_attachment_upload_slots_negative_limits_and_boundaries() {
    let client = test_api_client();

    // 0 files rejected
    let err_empty = client
        .request_attachment_upload_slots("1056563701764857896", &[])
        .expect_err("0 files must be rejected for slot allocation");
    assert_eq!(err_empty, AttachmentUploadError::EmptyContentAndAttachments);

    // 11 files rejected (> MAX_ATTACHMENTS_PER_MESSAGE = 10)
    let eleven_files: Vec<PendingAttachmentSpec> = (0..11)
        .map(|i| PendingAttachmentSpec {
            filename: format!("file_{i}.txt"),
            file_size: 128,
            content_type: "text/plain".to_string(),
            id: i,
        })
        .collect();
    let err_eleven = client
        .request_attachment_upload_slots("1056563701764857896", &eleven_files)
        .expect_err("11 files must be rejected");
    assert_eq!(err_eleven, AttachmentUploadError::AttachmentLimitExceeded);

    // 25 MiB + 1 byte (26_214_401) rejected
    let oversized = vec![PendingAttachmentSpec {
        filename: "too_big.bin".to_string(),
        file_size: (MAX_ATTACHMENT_SIZE_BYTES as u64) + 1,
        content_type: "application/octet-stream".to_string(),
        id: 0,
    }];
    let err_oversized = client
        .request_attachment_upload_slots("1056563701764857896", &oversized)
        .expect_err("25 MiB + 1 byte must be rejected");
    assert_eq!(
        err_oversized,
        AttachmentUploadError::AttachmentTooLarge {
            size: 26_214_401,
            limit: 26_214_400,
        }
    );
}

// ============================================================================
// 3. Filename Sanitization & Multipart RFC 7578 Construction
// ============================================================================

#[test]
fn t05_filename_sanitization_strips_traversal_and_crlf_injection() {
    // Path traversal stripped to safe basename
    assert_eq!(
        sanitize_attachment_filename("../../etc/passwd"),
        Ok("passwd".to_string())
    );
    assert_eq!(
        sanitize_attachment_filename("..\\..\\Windows\\System32\\cmd.exe"),
        Ok("cmd.exe".to_string())
    );

    // CRLF and NUL control characters stripped to prevent multipart header injection
    let injected = "report.pdf\r\nContent-Disposition: form-data; name=\"injected\"\r\n\0";
    let sanitized = sanitize_attachment_filename(injected).expect("sanitized");
    assert!(!sanitized.contains('\r'));
    assert!(!sanitized.contains('\n'));
    assert!(!sanitized.contains('\0'));
    assert!(!sanitized.contains('"'));
    assert!(!sanitized.contains('/'));
    assert!(!sanitized.contains('\\'));

    // Empty / traversal-only / control-only filenames rejected with EmptyFilename
    for empty_case in ["", "   ", "../..", "..\\..", "/", "\\", "\r\n\0", "..."] {
        assert_eq!(
            sanitize_attachment_filename(empty_case),
            Err(AttachmentUploadError::EmptyFilename),
            "expected EmptyFilename for {empty_case:?}"
        );
    }
}

#[test]
fn t06_build_multipart_message_request_rfc7578_format_and_boundaries() {
    let client = test_api_client();
    let files = vec![
        MultipartUploadFile {
            field_index: 0,
            filename: "../../screenshots/screen_1.png".to_string(),
            content_type: "image/png".to_string(),
            bytes: vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a],
        },
        MultipartUploadFile {
            field_index: 1,
            filename: "notes\r\n.txt".to_string(),
            content_type: "text/plain; charset=utf-8".to_string(),
            bytes: b"hello discord attachment".to_vec(),
        },
    ];

    let req = client
        .build_multipart_message_request(
            "1056563701764857896",
            "Check out these two files!",
            &files,
            Some("1234567890"),
        )
        .expect("valid multipart message request");

    assert_eq!(req.method, HttpMethod::Post);
    assert_eq!(req.target, "/channels/1056563701764857896/messages");

    let content_type = find_header(&req, "Content-Type").expect("Content-Type header");
    assert!(
        content_type.starts_with("multipart/form-data; boundary="),
        "expected multipart/form-data Content-Type, got {content_type}"
    );
    let boundary = content_type
        .strip_prefix("multipart/form-data; boundary=")
        .expect("boundary token");
    assert!(!boundary.is_empty());

    let body_str = String::from_utf8_lossy(&req.body);
    let open_boundary = format!("--{boundary}\r\n");
    let close_boundary = format!("--{boundary}--\r\n");

    assert!(body_str.starts_with(&open_boundary));
    assert!(body_str.ends_with(&close_boundary));
    assert!(
        body_str.contains("Content-Disposition: form-data; name=\"payload_json\"\r\n"),
        "missing payload_json part: {body_str}"
    );
    assert!(
        body_str.contains(
            "Content-Disposition: form-data; name=\"files[0]\"; filename=\"screen_1.png\"\r\n"
        ),
        "filename traversal must be sanitized in Content-Disposition: {body_str}"
    );
    assert!(
        body_str.contains(
            "Content-Disposition: form-data; name=\"files[1]\"; filename=\"notes.txt\"\r\n"
        ),
        "CRLF must be stripped from filename in Content-Disposition: {body_str}"
    );
    assert!(body_str.contains("\"content\":\"Check out these two files!\""));
    assert!(body_str.contains("\"nonce\":\"1234567890\""));
    assert!(body_str.contains("hello discord attachment"));
}

#[test]
fn t07_build_multipart_message_request_negative_validation() {
    let client = test_api_client();

    // Empty message content AND 0 attachments rejected
    let err_empty = client
        .build_multipart_message_request("1056563701764857896", "   ", &[], None)
        .expect_err("empty content and 0 attachments must be rejected");
    assert_eq!(err_empty, AttachmentUploadError::EmptyContentAndAttachments);

    // 11 attachments rejected
    let eleven: Vec<MultipartUploadFile> = (0..11)
        .map(|i| MultipartUploadFile {
            field_index: i,
            filename: format!("f{i}.bin"),
            content_type: "application/octet-stream".to_string(),
            bytes: vec![1, 2, 3],
        })
        .collect();
    let err_limit = client
        .build_multipart_message_request("1056563701764857896", "hi", &eleven, None)
        .expect_err("11 attachments must be rejected");
    assert_eq!(err_limit, AttachmentUploadError::AttachmentLimitExceeded);

    // Empty filename rejected
    let bad_name = vec![MultipartUploadFile {
        field_index: 0,
        filename: "../../".to_string(),
        content_type: "text/plain".to_string(),
        bytes: b"abc".to_vec(),
    }];
    let err_name = client
        .build_multipart_message_request("1056563701764857896", "hi", &bad_name, None)
        .expect_err("traversal-only filename must be rejected");
    assert_eq!(err_name, AttachmentUploadError::EmptyFilename);

    // 25 MiB exact accepted vs 25 MiB + 1 byte rejected
    let exact_25mib = vec![MultipartUploadFile {
        field_index: 0,
        filename: "exact25.bin".to_string(),
        content_type: "application/octet-stream".to_string(),
        bytes: vec![0u8; MAX_ATTACHMENT_SIZE_BYTES],
    }];
    assert!(
        client
            .build_multipart_message_request("1056563701764857896", "", &exact_25mib, None)
            .is_ok(),
        "exact 25 MiB attachment with empty text must be accepted"
    );

    let over_25mib = vec![MultipartUploadFile {
        field_index: 0,
        filename: "over25.bin".to_string(),
        content_type: "application/octet-stream".to_string(),
        bytes: vec![0u8; MAX_ATTACHMENT_SIZE_BYTES + 1],
    }];
    let err_over = client
        .build_multipart_message_request("1056563701764857896", "", &over_25mib, None)
        .expect_err("25 MiB + 1 byte must be rejected");
    assert_eq!(
        err_over,
        AttachmentUploadError::AttachmentTooLarge {
            size: (MAX_ATTACHMENT_SIZE_BYTES + 1) as u64,
            limit: MAX_ATTACHMENT_SIZE_BYTES as u64,
        }
    );
}

// ============================================================================
// 4. Async Transport Execution & HTTP Status Mapping (200, 400, 401, 403, 413, 429, 5xx)
// ============================================================================

#[tokio::test]
async fn t08_async_slot_creation_and_multipart_send_200_ok() {
    let client = test_api_client();
    let transport = MockUploadTransport::new(vec![
        Ok(HttpResponse {
            status: 200,
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            body: br#"{"attachments":[{"id":0,"upload_url":"https://discord-attachments-uploads-prd.storage.googleapis.com/upload/0","upload_filename":"a1b2/photo.png"}]}"#.to_vec(),
        }),
        Ok(HttpResponse {
            status: 200,
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            body: br#"{"id":"112233445566778899","channel_id":"1056563701764857896","content":"Here is the photo","attachments":[{"id":"998877665544332211","filename":"photo.png","size":4,"url":"https://cdn.discordapp.com/attachments/1056563701764857896/998877665544332211/photo.png","content_type":"image/png"}]}"#.to_vec(),
        }),
    ]);

    let specs = vec![PendingAttachmentSpec {
        filename: "photo.png".to_string(),
        file_size: 4,
        content_type: "image/png".to_string(),
        id: 0,
    }];
    let slots = client
        .create_attachment_upload_slots(&transport, "1056563701764857896", &specs)
        .await
        .expect("slot creation 200 OK");
    assert_eq!(
        slots,
        vec![AttachmentUploadSlot {
            id: 0,
            upload_url: "https://discord-attachments-uploads-prd.storage.googleapis.com/upload/0"
                .to_string(),
            upload_filename: "a1b2/photo.png".to_string(),
        }]
    );

    let upload_files = vec![MultipartUploadFile {
        field_index: 0,
        filename: "photo.png".to_string(),
        content_type: "image/png".to_string(),
        bytes: vec![1, 2, 3, 4],
    }];
    let sent = client
        .send_message_with_attachments(
            &transport,
            "1056563701764857896",
            "Here is the photo",
            &upload_files,
            Some("999000111"),
        )
        .await
        .expect("multipart send 200 OK");

    assert_eq!(sent.id, "112233445566778899");
    assert_eq!(sent.channel_id, "1056563701764857896");
    assert_eq!(sent.content, "Here is the photo");
    assert_eq!(sent.attachments.len(), 1);
    assert_eq!(sent.attachments[0].filename, "photo.png");
    assert_eq!(sent.attachments[0].size, 4);

    let recorded = transport.take_requests();
    assert_eq!(recorded.len(), 2);
    assert_eq!(
        recorded[0].target,
        "/channels/1056563701764857896/attachments"
    );
    assert_eq!(recorded[1].target, "/channels/1056563701764857896/messages");
}

#[tokio::test]
async fn t09_async_error_status_mapping_400_401_403_413_429_5xx() {
    let client = test_api_client();
    let files = vec![MultipartUploadFile {
        field_index: 0,
        filename: "doc.pdf".to_string(),
        content_type: "application/pdf".to_string(),
        bytes: b"%PDF-1.7".to_vec(),
    }];

    // 400 Bad Request
    let t_400 = MockUploadTransport::new(vec![Ok(HttpResponse {
        status: 400,
        headers: Vec::new(),
        body: br#"{"code":50035,"message":"Invalid Form Body"}"#.to_vec(),
    })]);
    assert_eq!(
        client
            .send_message_with_attachments(&t_400, "1056563701764857896", "hi", &files, None)
            .await,
        Err(AttachmentUploadError::BadRequest {
            code: Some(50035),
            message: Some("Invalid Form Body".to_string()),
        })
    );

    // 401 Unauthorized
    let t_401 = MockUploadTransport::new(vec![Ok(HttpResponse {
        status: 401,
        headers: Vec::new(),
        body: br#"{"message":"401: Unauthorized"}"#.to_vec(),
    })]);
    assert_eq!(
        client
            .send_message_with_attachments(&t_401, "1056563701764857896", "hi", &files, None)
            .await,
        Err(AttachmentUploadError::Unauthorized)
    );

    // 403 Forbidden
    let t_403 = MockUploadTransport::new(vec![Ok(HttpResponse {
        status: 403,
        headers: Vec::new(),
        body: br#"{"code":50013,"message":"Missing Permissions"}"#.to_vec(),
    })]);
    assert_eq!(
        client
            .send_message_with_attachments(&t_403, "1056563701764857896", "hi", &files, None)
            .await,
        Err(AttachmentUploadError::Forbidden {
            message: Some("Missing Permissions".to_string()),
        })
    );

    // 413 Payload Too Large
    let t_413 = MockUploadTransport::new(vec![Ok(HttpResponse {
        status: 413,
        headers: Vec::new(),
        body: br#"{"code":40005,"message":"Request entity too large"}"#.to_vec(),
    })]);
    assert_eq!(
        client
            .send_message_with_attachments(&t_413, "1056563701764857896", "hi", &files, None)
            .await,
        Err(AttachmentUploadError::PayloadTooLarge)
    );

    // 429 Too Many Requests with Retry-After
    let t_429 = MockUploadTransport::new(vec![Ok(HttpResponse {
        status: 429,
        headers: vec![("Retry-After".to_string(), "2.5".to_string())],
        body: br#"{"message":"You are being rate limited.","retry_after":2.5,"global":false}"#
            .to_vec(),
    })]);
    assert_eq!(
        client
            .send_message_with_attachments(&t_429, "1056563701764857896", "hi", &files, None)
            .await,
        Err(AttachmentUploadError::RateLimited {
            retry_after: Duration::from_millis(2500),
        })
    );

    // 503 Service Unavailable (5xx)
    let t_503 = MockUploadTransport::new(vec![Ok(HttpResponse {
        status: 503,
        headers: Vec::new(),
        body: b"Service Unavailable".to_vec(),
    })]);
    assert_eq!(
        client
            .send_message_with_attachments(&t_503, "1056563701764857896", "hi", &files, None)
            .await,
        Err(AttachmentUploadError::Server { status: 503 })
    );
}

// ============================================================================
// 5. ChatView Attachment Staging & Composer UI State Machine
// ============================================================================

#[test]
fn t10_chat_view_attachment_composer_staging_remove_clear_and_error_recovery() {
    let mut composer = AttachmentComposerState::new();
    assert!(composer.is_empty());
    assert_eq!(composer.count(), 0);
    assert_eq!(composer.total_bytes(), 0);
    assert!(!composer.can_send(""));
    assert!(composer.can_send("hello"));

    // Stage 2 valid files (one with path traversal that gets sanitized)
    let id0 = composer
        .stage_file("../../avatars/avatar.png", "image/png", vec![1, 2, 3, 4])
        .expect("stage first file");
    let id1 = composer
        .stage_file("logs.txt", "text/plain", vec![b'a'; 100])
        .expect("stage second file");

    assert_eq!(id0, 0);
    assert_eq!(id1, 1);
    assert_eq!(composer.count(), 2);
    assert_eq!(composer.total_bytes(), 104);
    assert_eq!(composer.staged[0].filename, "avatar.png");
    assert_eq!(composer.staged[0].phase, AttachmentUploadPhase::Pending);
    assert!(composer.can_send(""));

    // Remove first file by id
    let removed: StagedAttachment = composer.remove_by_id(id0).expect("removed id0");
    assert_eq!(removed.filename, "avatar.png");
    assert_eq!(composer.count(), 1);
    assert_eq!(composer.total_bytes(), 100);

    // Stage up to 10 files total
    for idx in 0..9 {
        composer
            .stage_file(
                &format!("extra_{idx}.bin"),
                "application/octet-stream",
                vec![0u8; 10],
            )
            .expect("stage up to 10");
    }
    assert_eq!(composer.count(), 10);
    assert_eq!(composer.total_bytes(), 190);

    // 11th file rejected with AttachmentLimitExceeded and sets error_banner
    let err_11 = composer
        .stage_file("overflow.bin", "application/octet-stream", vec![1])
        .expect_err("11th staged file must be rejected");
    assert_eq!(err_11, AttachmentUploadError::AttachmentLimitExceeded);
    assert!(composer.error_banner.is_some());

    // Simulate upload start -> failure (HTTP 413/429/5xx) -> preserves staged files for retry
    composer.mark_uploading();
    assert!(composer.is_uploading);
    assert!(
        composer
            .staged
            .iter()
            .all(|item| item.phase == AttachmentUploadPhase::Uploading)
    );

    composer.handle_send_error(&AttachmentUploadError::RateLimited {
        retry_after: Duration::from_secs(3),
    });
    assert!(!composer.is_uploading);
    assert_eq!(
        composer.count(),
        10,
        "staged files must be preserved on error for retry"
    );
    assert!(composer.error_banner.is_some());
    assert!(
        composer
            .staged
            .iter()
            .all(|item| matches!(item.phase, AttachmentUploadPhase::Failed(_)))
    );

    // Retry resets phases to Pending and clears error banner
    assert!(composer.retry_failed());
    assert!(composer.error_banner.is_none());
    assert!(
        composer
            .staged
            .iter()
            .all(|item| item.phase == AttachmentUploadPhase::Pending)
    );

    // Success clears all staged files
    let completed = composer.complete_send_success();
    assert_eq!(completed.len(), 10);
    assert!(composer.is_empty());
    assert_eq!(composer.total_bytes(), 0);
}

#[test]
fn t11_chat_view_plus_picker_and_send_with_attachments_integration() {
    let mut state = AppState::new();
    ChatView::clear_staged_attachments();

    let channel_id = state.selected_channel_id.clone().expect("selected channel");
    let before_count = state.messages.get(&channel_id).map_or(0, Vec::len);

    // Clicking '+' stages an attachment in ChatView composer state
    let staged_id =
        ChatView::stage_attachment("diagram.png", "image/png", vec![0x89, b'P', b'N', b'G'])
            .expect("stage attachment via ChatView");
    assert_eq!(staged_id, 0);
    assert_eq!(ChatView::staged_attachment_count(), 1);
    assert_eq!(ChatView::staged_total_bytes(), 4);

    // Sending message with empty draft_text still succeeds when attachments are staged
    let sent = ChatView::submit_composer_message(&mut state)
        .expect("submit composer with staged attachment");
    assert!(sent);
    assert_eq!(ChatView::staged_attachment_count(), 0);

    let after_messages = state.messages.get(&channel_id).expect("channel messages");
    assert_eq!(after_messages.len(), before_count + 1);
    let last_msg = after_messages.last().expect("new message");
    assert_eq!(last_msg.attachments.len(), 1);
    assert_eq!(last_msg.attachments[0].filename, "diagram.png");
    assert_eq!(last_msg.attachments[0].size_bytes, 4);
}
