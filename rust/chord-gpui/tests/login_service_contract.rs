//! Контрактные тесты Discord REST v9 `LoginService` (`P0-2.1` + `P0-2.4`):
//! - вход по паролю (`POST /api/v9/auth/login`),
//! - двухфакторная аутентификация TOTP (`POST /api/v9/auth/mfa/totp`),
//! - структурированная ошибка капчи (`400` с `captcha_key`, без авто-обхода),
//! - получение текущего пользователя (`GET /api/v9/users/@me` через `chord_core::parse_me`),
//! - маскирование секретов (`token`, `ticket`, `password`) в `Debug` и `Display`.

#![forbid(unsafe_code)]

#[path = "../src/auth/login_service.rs"]
mod login_service;

use chord_core::rest::identity::ClientIdentity;
use chord_core::rest::routes::HttpMethod;
use chord_core::rest::super_properties::SuperProperties;
use chord_core::rest::transport::{HttpRequest, HttpResponse};
use login_service::{
    AUTH_LOGIN_PATH, AUTH_MFA_TOTP_PATH, AuthHttpTransport, CaptchaChallenge, LiveAuthTransport,
    LoginError, LoginOutcome, LoginService, MfaChallenge, MockAuthTransport, USERS_ME_PATH,
};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn sample_super_properties() -> SuperProperties {
    let mut sp = ClientIdentity::rw_client().to_super_properties();
    sp.os = "Linux".to_string();
    sp.browser = "Discord Client".to_string();
    sp.release_channel = "stable".to_string();
    sp.client_version = "1.0.162".to_string();
    sp.system_locale = "en-US".to_string();
    sp.browser_user_agent =
        "Mozilla/5.0 (X11; Linux x86_64) discord/1.0.162 Electron/42.11.12".to_string();
    sp.client_build_number = 641_004;
    sp
}

fn json_response(status: u16, body: &serde_json::Value) -> HttpResponse {
    HttpResponse {
        status,
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: body.to_string().into_bytes(),
    }
}

fn find_header<'a>(req: &'a HttpRequest, name: &str) -> Option<&'a str> {
    req.headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

#[tokio::test]
async fn login_200_direct_token_and_redacts_debug_secrets() {
    let transport = MockAuthTransport::new();
    let secret_token = "mfa.direct_secret_token_9988776655";
    let secret_password = "correct-horse-battery-staple-secret!";

    transport.push_response(json_response(
        200,
        &json!({
            "token": secret_token,
            "user_settings": {
                "locale": "en-US",
                "theme": "dark"
            }
        }),
    ));

    let sp = sample_super_properties();
    let expected_sp_b64 = sp.to_base64();
    let service = LoginService::new("https://discord.com/api/v9", sp, transport);

    let outcome = service
        .login_password("alice@example.com", secret_password)
        .await
        .expect("200 direct token login must succeed");

    // Verify Debug redaction on LoginOutcome::Authenticated
    let debug_str = format!("{outcome:?}");
    assert!(
        !debug_str.contains(secret_token),
        "Debug of LoginOutcome::Authenticated must NEVER leak token: {debug_str}"
    );
    assert!(
        debug_str.contains("[REDACTED]"),
        "Debug of LoginOutcome::Authenticated must show [REDACTED]: {debug_str}"
    );

    match outcome {
        LoginOutcome::Authenticated {
            token,
            user_settings,
        } => {
            assert_eq!(token, secret_token);
            assert_eq!(
                user_settings,
                Some(json!({"locale": "en-US", "theme": "dark"}))
            );
        }
        other => panic!("expected LoginOutcome::Authenticated, got {other:?}"),
    }

    // Verify wire request headers and JSON payload
    let calls = service.transport().recorded_calls();
    assert_eq!(calls.len(), 1);
    let req = &calls[0].request;
    assert_eq!(req.method, HttpMethod::Post);
    assert_eq!(req.target, AUTH_LOGIN_PATH);
    assert_eq!(find_header(req, "Content-Type"), Some("application/json"));
    assert_eq!(
        find_header(req, "X-Super-Properties"),
        Some(expected_sp_b64.as_str())
    );
    assert_eq!(find_header(req, "X-Discord-Locale"), Some("en-US"));
    assert!(
        find_header(req, "User-Agent").is_some_and(|ua| ua.contains("discord/1.0.162")),
        "User-Agent header must be populated"
    );

    let sent_json: serde_json::Value =
        serde_json::from_slice(&req.body).expect("request body must be valid JSON");
    assert_eq!(
        sent_json,
        json!({
            "login": "alice@example.com",
            "password": secret_password,
            "undelete": false
        })
    );
}

#[tokio::test]
async fn login_200_mfa_challenge_then_submit_totp_then_fetch_current_user() {
    let transport = MockAuthTransport::new();
    let secret_ticket = "ticket_mfa_secret_abc123xyz987";
    let secret_mfa_token = "mfa.verified_token_77441122";

    // Step 1: /api/v9/auth/login -> 200 MFA challenge
    transport.push_response(json_response(
        200,
        &json!({
            "token": null,
            "mfa": true,
            "ticket": secret_ticket,
            "totp": true,
            "sms": false,
            "backup": true,
            "webauthn": null
        }),
    ));

    // Step 2: /api/v9/auth/mfa/totp -> 200 with session token
    transport.push_response(json_response(
        200,
        &json!({
            "token": secret_mfa_token
        }),
    ));

    // Step 3: /api/v9/users/@me -> 200 current user profile
    transport.push_response(json_response(
        200,
        &json!({
            "id": "80351110224678912",
            "username": "nelly",
            "discriminator": "0",
            "global_name": "Nelly the Hamster",
            "avatar": "8342729096ea3675442027381ff50dfe",
            "bot": false
        }),
    ));

    let sp = sample_super_properties();
    let expected_sp_b64 = sp.to_base64();
    let service = LoginService::new("https://discord.com/api/v9", sp, transport);

    // 1. Password login -> MfaRequired
    let login_outcome = service
        .login_password("nelly@discord.com", "NellySecretPass42!")
        .await
        .expect("MFA challenge response must succeed");

    let mfa_debug = format!("{login_outcome:?}");
    assert!(
        !mfa_debug.contains(secret_ticket),
        "MfaChallenge Debug must redact ticket: {mfa_debug}"
    );

    let LoginOutcome::MfaRequired(MfaChallenge {
        ticket,
        totp,
        sms,
        backup,
    }) = login_outcome
    else {
        panic!("expected MfaRequired, got {login_outcome:?}");
    };
    assert_eq!(ticket, secret_ticket);
    assert!(totp);
    assert!(!sms);
    assert!(backup);

    // 2. Submit TOTP -> session token
    let token = service
        .submit_totp(&ticket, "482910")
        .await
        .expect("submit_totp must succeed on 200");
    assert_eq!(token, secret_mfa_token);

    // 3. Fetch current user (/api/v9/users/@me) -> Me
    let me = service
        .fetch_current_user(&token)
        .await
        .expect("fetch_current_user must parse @me");
    assert_eq!(me.id, "80351110224678912");
    assert_eq!(me.username, "nelly");
    assert_eq!(me.discriminator, "0");
    assert_eq!(me.global_name.as_deref(), Some("Nelly the Hamster"));
    assert_eq!(
        me.avatar.as_deref(),
        Some("8342729096ea3675442027381ff50dfe")
    );
    assert!(!me.bot);

    // Verify all 3 recorded wire requests
    let calls = service.transport().recorded_calls();
    assert_eq!(calls.len(), 3);

    // Call 2: POST /api/v9/auth/mfa/totp
    assert_eq!(calls[1].request.method, HttpMethod::Post);
    assert_eq!(calls[1].request.target, AUTH_MFA_TOTP_PATH);
    let totp_body: serde_json::Value =
        serde_json::from_slice(&calls[1].request.body).expect("totp body JSON");
    assert_eq!(
        totp_body,
        json!({
            "code": "482910",
            "ticket": secret_ticket
        })
    );

    // Call 3: GET /api/v9/users/@me with Authorization and X-Super-Properties
    assert_eq!(calls[2].request.method, HttpMethod::Get);
    assert_eq!(calls[2].request.target, USERS_ME_PATH);
    assert_eq!(
        find_header(&calls[2].request, "Authorization"),
        Some(secret_mfa_token)
    );
    assert_eq!(
        find_header(&calls[2].request, "X-Super-Properties"),
        Some(expected_sp_b64.as_str())
    );
}

#[tokio::test]
async fn login_400_captcha_challenge_surfaced_as_structured_error() {
    let transport = MockAuthTransport::new();
    transport.push_response(json_response(
        400,
        &json!({
            "captcha_key": ["captcha-required"],
            "captcha_sitekey": "a9b5fb07-92ff-493f-86fe-352a2803b3df",
            "captcha_service": "hcaptcha",
            "captcha_rqdata": "rqdata_payload_token_123",
            "captcha_rqtoken": "rqtoken_signed_456"
        }),
    ));

    let service = LoginService::new(
        "https://discord.com/api/v9",
        sample_super_properties(),
        transport,
    );

    let err = service
        .login_password("user@example.com", "SecretPass123!")
        .await
        .expect_err("400 captcha response must return Err(LoginError::CaptchaRequired)");

    // Ensure password never appears in Debug or Display of error
    assert!(!format!("{err:?}").contains("SecretPass123!"));
    assert!(!format!("{err}").contains("SecretPass123!"));

    assert_eq!(
        err,
        LoginError::CaptchaRequired(CaptchaChallenge {
            captcha_service: "hcaptcha".to_string(),
            captcha_sitekey: "a9b5fb07-92ff-493f-86fe-352a2803b3df".to_string(),
            captcha_rqdata: Some("rqdata_payload_token_123".to_string()),
            captcha_rqtoken: Some("rqtoken_signed_456".to_string()),
        })
    );
}

#[tokio::test]
async fn login_and_totp_400_and_401_invalid_credentials() {
    let transport = MockAuthTransport::new();

    // 1. 400 invalid login/password (code 50035)
    transport.push_response(json_response(
        400,
        &json!({
            "code": 50035,
            "message": "Invalid Form Body",
            "errors": {
                "login": {
                    "_errors": [{"code": "INVALID_LOGIN", "message": "Login or password is invalid."}]
                }
            }
        }),
    ));

    // 2. 401 unauthorized on TOTP (code 60008)
    transport.push_response(json_response(
        401,
        &json!({
            "code": 60008,
            "message": "Invalid two-factor code"
        }),
    ));

    // 3. 401 unauthorized on /users/@me
    transport.push_response(json_response(
        401,
        &json!({
            "code": 0,
            "message": "401: Unauthorized"
        }),
    ));

    let service = LoginService::new(
        "https://discord.com/api/v9",
        sample_super_properties(),
        transport,
    );

    let err_login = service
        .login_password("wrong@example.com", "wrong-password-secret")
        .await
        .expect_err("400 invalid login must fail");
    assert!(!format!("{err_login:?}").contains("wrong-password-secret"));
    assert_eq!(
        err_login,
        LoginError::InvalidCredentials {
            code: Some(50035),
            message: "Invalid Form Body".to_string(),
        }
    );

    let err_totp = service
        .submit_totp("secret-ticket-999", "000000")
        .await
        .expect_err("401 invalid TOTP must fail");
    assert!(!format!("{err_totp:?}").contains("secret-ticket-999"));
    assert_eq!(
        err_totp,
        LoginError::InvalidCredentials {
            code: Some(60008),
            message: "Invalid two-factor code".to_string(),
        }
    );

    let err_me = service
        .fetch_current_user("expired-secret-user-token")
        .await
        .expect_err("401 on @me must fail");
    assert!(!format!("{err_me:?}").contains("expired-secret-user-token"));
    assert_eq!(
        err_me,
        LoginError::InvalidCredentials {
            code: Some(0),
            message: "401: Unauthorized".to_string(),
        }
    );
}

#[tokio::test]
async fn rate_limit_429_parses_retry_after_float_and_header() {
    let transport = MockAuthTransport::new();

    // 1. JSON body retry_after: 1.5 -> 1500 ms
    transport.push_response(json_response(
        429,
        &json!({
            "message": "You are being rate limited.",
            "retry_after": 1.5,
            "global": false
        }),
    ));

    // 2. Retry-After header + global: true
    transport.push_response(HttpResponse {
        status: 429,
        headers: vec![
            ("Retry-After".to_string(), "2.25".to_string()),
            ("X-RateLimit-Global".to_string(), "true".to_string()),
        ],
        body: b"{}".to_vec(),
    });

    let service = LoginService::new(
        "https://discord.com/api/v9",
        sample_super_properties(),
        transport,
    );

    let err1 = service
        .login_password("alice@example.com", "password123")
        .await
        .expect_err("429 must return RateLimited");
    assert_eq!(
        err1,
        LoginError::RateLimited {
            retry_after_ms: 1500,
            global: false,
        }
    );

    let err2 = service
        .submit_totp("ticket-abc", "123456")
        .await
        .expect_err("429 on submit_totp must return RateLimited");
    assert_eq!(
        err2,
        LoginError::RateLimited {
            retry_after_ms: 2250,
            global: true,
        }
    );
}

#[tokio::test]
async fn upstream_5xx_returns_server_error() {
    let transport = MockAuthTransport::new();
    transport.push_response(HttpResponse {
        status: 502,
        headers: vec![],
        body: b"Bad Gateway".to_vec(),
    });
    transport.push_response(HttpResponse {
        status: 503,
        headers: vec![],
        body: b"Service Unavailable".to_vec(),
    });

    let service = LoginService::new(
        "https://discord.com/api/v9",
        sample_super_properties(),
        transport,
    );

    let err_502 = service
        .login_password("alice@example.com", "password123")
        .await
        .expect_err("502 must fail with ServerError");
    assert_eq!(err_502, LoginError::ServerError { status: 502 });

    let err_503 = service
        .fetch_current_user("valid_token_123")
        .await
        .expect_err("503 must fail with ServerError");
    assert_eq!(err_503, LoginError::ServerError { status: 503 });
}

#[tokio::test]
async fn input_validation_and_malformed_json_errors() {
    let transport = MockAuthTransport::new();
    let service = LoginService::new(
        "https://discord.com/api/v9",
        sample_super_properties(),
        transport,
    );

    // Empty login or password must fail validation before hitting transport
    assert!(matches!(
        service.login_password("", "password123").await,
        Err(LoginError::Validation(_))
    ));
    assert!(matches!(
        service.login_password("   ", "password123").await,
        Err(LoginError::Validation(_))
    ));
    assert!(matches!(
        service.login_password("alice@example.com", "").await,
        Err(LoginError::Validation(_))
    ));

    // Empty ticket or invalid TOTP code ("12ab", too short, too long, non-digits)
    assert!(matches!(
        service.submit_totp("", "123456").await,
        Err(LoginError::Validation(_))
    ));
    assert!(matches!(
        service.submit_totp("   ", "123456").await,
        Err(LoginError::Validation(_))
    ));
    assert!(matches!(
        service.submit_totp("ticket-1", "12ab").await,
        Err(LoginError::Validation(_))
    ));
    assert!(matches!(
        service.submit_totp("ticket-1", "12345").await,
        Err(LoginError::Validation(_))
    ));
    assert!(matches!(
        service.submit_totp("ticket-1", "123456789").await,
        Err(LoginError::Validation(_))
    ));

    // Empty token on fetch_current_user
    assert!(matches!(
        service.fetch_current_user("   ").await,
        Err(LoginError::Validation(_))
    ));

    // Zero transport calls should have been made for validation failures
    assert_eq!(service.transport().recorded_calls().len(), 0);

    // Malformed JSON bodies on 200 responses
    service.transport().push_response(HttpResponse {
        status: 200,
        headers: vec![],
        body: b"{not valid json".to_vec(),
    });
    service.transport().push_response(HttpResponse {
        status: 200,
        headers: vec![],
        body: b"{\"unexpected\": true}".to_vec(),
    });
    service.transport().push_response(HttpResponse {
        status: 200,
        headers: vec![],
        body: b"{brokentotp".to_vec(),
    });
    service.transport().push_response(HttpResponse {
        status: 200,
        headers: vec![],
        body: b"{\"id\": 123}".to_vec(),
    });

    assert!(matches!(
        service.login_password("alice@example.com", "pass").await,
        Err(LoginError::MalformedResponse(_))
    ));
    assert!(matches!(
        service.login_password("alice@example.com", "pass").await,
        Err(LoginError::MalformedResponse(_))
    ));
    assert!(matches!(
        service.submit_totp("ticket-1", "123456").await,
        Err(LoginError::MalformedResponse(_))
    ));
    assert!(matches!(
        service.fetch_current_user("valid-token").await,
        Err(LoginError::MalformedResponse(_))
    ));
}

#[tokio::test]
async fn live_auth_transport_executes_against_local_http_mock_gateway() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind local mock server");
    let addr = listener.local_addr().expect("local addr");

    let server_task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept connection");
        let mut buf = vec![0u8; 4096];
        let n = socket.read(&mut buf).await.expect("read request");
        let req_text = String::from_utf8_lossy(&buf[..n]).to_string();

        let body = r#"{"id":"18446744073709551615","username":"max_snowflake","discriminator":"0001","global_name":null,"avatar":null,"bot":true}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        socket
            .write_all(response.as_bytes())
            .await
            .expect("write response");
        req_text
    });

    let base_url = format!("http://{addr}/api/v9");
    let service = LoginService::new(
        base_url,
        sample_super_properties(),
        LiveAuthTransport::new(),
    );

    let me = service
        .fetch_current_user("local_e2e_token_xyz")
        .await
        .expect("LiveAuthTransport must succeed against local HTTP server");

    assert_eq!(me.id, "18446744073709551615");
    assert_eq!(me.username, "max_snowflake");
    assert!(me.bot);

    let raw_request = server_task.await.expect("server task join");
    assert!(
        raw_request.starts_with("GET /api/v9/users/@me HTTP/1.1\r\n"),
        "wire request line must not duplicate /api/v9: {raw_request}"
    );
}

#[tokio::test]
async fn custom_auth_http_transport_and_8_digit_backup_code_boundary() {
    struct CustomStaticTransport;

    impl AuthHttpTransport for CustomStaticTransport {
        async fn execute(
            &self,
            base_url: &str,
            request: HttpRequest,
        ) -> Result<HttpResponse, LoginError> {
            assert_eq!(base_url, "https://discord.com/api/v9");
            assert_eq!(request.target, AUTH_MFA_TOTP_PATH);
            Ok(json_response(
                200,
                &json!({
                    "token": "mfa.backup_8digit_token"
                }),
            ))
        }
    }

    let service = LoginService::new(
        "https://discord.com/api/v9",
        sample_super_properties(),
        CustomStaticTransport,
    )
    .with_locale("ru")
    .with_timezone("Europe/Riga")
    .with_user_agent("discord-lab-test/1.0");

    let token = service
        .submit_totp("ticket-8digit", "12345678")
        .await
        .expect("8-digit numeric code must be accepted by submit_totp");
    assert_eq!(token, "mfa.backup_8digit_token");
}
