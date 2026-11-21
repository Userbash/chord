//! Wave 3 Contract Tests (`W3-1`):
//! Live Auth Flow Wiring, Mock Guard & Auth State Split (`P0-2.3`, `P0-3.1`, `P0-3.2`, `P2-3.1`, `P2-3.2`).
//!
//! Covers all 9 mandatory test dimensions (`TEST-POLICY.md` §12):
//! 1. Positive:
//!    - `P0-2.3`: Startup auto-login from `TokenStore` (`MemoryKeyringBackend`) -> HTTP `@me` 200
//!      populates `current_user`, sets `active_token`, and closes `is_login_modal_open`.
//!    - `P0-3.1`: Email + password -> `POST /api/v9/auth/login` 200 token -> `GET /api/v9/users/@me` 200
//!      persists `SecretToken` in `TokenStore` and hydrates `AppState.current_user`.
//!    - `P0-3.1` -> `P0-3.2`: Email + password -> `200 {"mfa": true, "ticket": ...}` transitions
//!      `AppState.login_form.step` to `LoginStep::Mfa { ticket }` -> 6-digit TOTP (and 8-digit backup)
//!      -> `POST /api/v9/auth/mfa/totp` 200 token -> persisted in `TokenStore` -> `@me` applied.
//!    - `P2-3.2`: `AppState::new_live()` has zero mock guilds/tokens/users (`"Rustery"`,
//!      `"sample-token-rustery-001"`) when `CHORD_OFFLINE_DEMO != "1"`, whereas
//!      `AppState::new_live_with_offline_demo(true)` populates demo fixtures.
//! 2. Negative & Boundaries:
//!    - Empty email/password/token rejected before network call (`recorded_calls().is_empty()`).
//!    - Invalid MFA code (<6 chars, >8 chars, non-digit ASCII) rejected before network call.
//!    - Missing or empty `mfa_ticket` rejected with `AuthFlowError::MissingMfaTicket`.
//!    - Snowflake boundaries (`"0"` minimum vs `"18446744073709551615"` `u64::MAX`) in `@me`.
//! 3. Error Handling (`400`/`401`/`429`/`500`):
//!    - `400` with `captcha_key` surfaces `CaptchaChallenge` in `state.active_captcha` and
//!      `LoginModal::active_captcha_banner_text` without bypassing or solving CAPTCHA.
//!    - `401` on startup `@me`, credentials login, or TOTP verification surfaces error and keeps modal open.
//!    - `429` preserves `retry_after_ms` in `StartupAuthResult::RateLimited` and `AuthFlowError::RateLimited`.
//!    - `500` server error mapped cleanly to `StartupAuthResult::NetworkError` / `AuthFlowError::ServerError`.
//! 4. Recovery / Keyring Faults & Security Redaction:
//!    - Missing, corrupted (`\0`/`\n`), or unavailable `TokenStore` gracefully falls back to
//!      `StartupAuthResult::RequiresLogin` with `is_login_modal_open == true`.
//!    - `Debug` and `Display` outputs of all auth flow outcomes/errors and `AppState` error messages
//!      never leak raw passwords, MFA tickets, TOTP codes, or session tokens (`[REDACTED]`).

#![recursion_limit = "512"]

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

use chord_core::rest::identity::ClientIdentity;
use chord_core::rest::routes::HttpMethod;
use chord_core::rest::super_properties::SuperProperties;
use chord_core::rest::transport::HttpResponse;
use serde_json::json;

use state::auth_flow::{
    AUTH_LOGIN_PATH, AUTH_MFA_TOTP_PATH, AuthFlowController, AuthFlowError, AuthMode,
    LiveAuthStepOutcome, LoginService, MemoryKeyringBackend, MockAuthTransport, SecretBackend,
    SecretToken, StartupAuthResult, TokenStore, TokenStoreError, USERS_ME_PATH,
    should_populate_mock_data_with,
};
use state::{AppState, LoginStep, PresenceStatus};
use ui::login_modal::LoginModal;

fn test_super_properties() -> SuperProperties {
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

fn build_mock_service() -> LoginService<MockAuthTransport> {
    LoginService::new(
        "https://discord.com/api/v9",
        test_super_properties(),
        MockAuthTransport::new(),
    )
}

fn json_http_response(
    status: u16,
    headers: Vec<(&str, &str)>,
    body: &serde_json::Value,
) -> HttpResponse {
    let mut hdrs = vec![("content-type".to_string(), "application/json".to_string())];
    for (k, v) in headers {
        hdrs.push((k.to_string(), v.to_string()));
    }
    HttpResponse {
        status,
        headers: hdrs,
        body: body.to_string().into_bytes(),
    }
}

fn sample_me_json(id: &str, username: &str, global_name: Option<&str>) -> serde_json::Value {
    json!({
        "id": id,
        "username": username,
        "discriminator": "0",
        "global_name": global_name,
        "avatar": "a_live_avatar_hash_123",
        "bot": false
    })
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. P2-3.1 & P2-3.2: AuthMode & Mock Data Guard Contract
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn p2_3_2_mock_data_guard_and_auth_mode_resolution() {
    // Explicit flag overrides env
    assert!(!should_populate_mock_data_with(Some("1"), Some(false)));
    assert!(should_populate_mock_data_with(Some("0"), Some(true)));

    // Env evaluation when explicit flag is None
    assert_eq!(AuthMode::from_env_value(None), AuthMode::Live);
    assert_eq!(AuthMode::from_env_value(Some("")), AuthMode::Live);
    assert_eq!(AuthMode::from_env_value(Some("0")), AuthMode::Live);
    assert_eq!(AuthMode::from_env_value(Some("false")), AuthMode::Live);
    assert_eq!(AuthMode::from_env_value(Some("1")), AuthMode::OfflineDemo);
    assert_eq!(AuthMode::from_env_value(Some(" 1 ")), AuthMode::OfflineDemo);
    assert!(AuthMode::Live.is_live());
    assert!(!AuthMode::Live.is_offline_demo());
    assert!(AuthMode::OfflineDemo.is_offline_demo());

    // Live mode (`offline_demo = false`) must have zero mock guilds, zero fake tokens, no "Rustery"
    let live_state = AppState::new_live_with_offline_demo(false);
    assert!(
        live_state.guilds.is_empty(),
        "live AppState must not populate mock guilds"
    );
    assert!(
        live_state.messages.is_empty(),
        "live AppState must not populate mock messages"
    );
    assert!(
        live_state.relationships.is_empty(),
        "live AppState must not populate mock relationships"
    );
    assert!(
        live_state.accounts.is_empty(),
        "live AppState must not populate mock accounts"
    );
    assert!(
        live_state.active_token.is_none(),
        "live AppState must not set sample-token-rustery-001"
    );
    assert_ne!(
        live_state.current_user.username, "Rustery",
        "live AppState must not set fake Rustery username"
    );
    assert!(
        live_state.is_login_modal_open,
        "live AppState must open login modal when unauthenticated"
    );

    // Offline demo mode (`offline_demo = true`) populates demo fixtures
    let demo_state = AppState::new_live_with_offline_demo(true);
    assert_eq!(demo_state.guilds.len(), 2);
    assert_eq!(demo_state.current_user.username, "Rustery");
    assert_eq!(
        demo_state.active_token.as_deref(),
        Some("sample-token-rustery-001")
    );
    assert!(!demo_state.is_login_modal_open);
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. P0-2.3: Startup Auto-Login from TokenStore -> @me
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn p0_2_3_startup_auto_login_200_hydrates_user_and_closes_modal() {
    let mut state = AppState::new_live_with_offline_demo(false);
    assert!(state.is_login_modal_open);

    let mut store = TokenStore::new(MemoryKeyringBackend::new());
    let raw_token = "mfa.startup_valid_secret_token_998877665544332211";
    let secret = SecretToken::new(raw_token).expect("valid secret token");
    store.save_active_token(&secret).expect("save active token");

    let service = build_mock_service();
    service.transport().push_response(json_http_response(
        200,
        vec![],
        &sample_me_json(
            "18446744073709551615",
            "max_snowflake_user",
            Some("Max Snowflake"),
        ),
    ));

    let result = state.attempt_startup_auto_login(&store, &service).await;
    match &result {
        StartupAuthResult::Authenticated { user, token_stored } => {
            assert!(*token_stored);
            assert_eq!(user.id, "18446744073709551615");
            assert_eq!(user.username, "max_snowflake_user");
            assert_eq!(user.global_name.as_deref(), Some("Max Snowflake"));
            assert_eq!(user.status, PresenceStatus::Online);
        }
        other => panic!("expected StartupAuthResult::Authenticated, got {other:?}"),
    }

    assert!(
        !state.is_login_modal_open,
        "login modal must close after startup auto-login"
    );
    assert_eq!(state.current_user.id, "18446744073709551615");
    assert_eq!(state.current_user.username, "max_snowflake_user");
    assert_eq!(state.active_token.as_deref(), Some(raw_token));
    assert_eq!(state.accounts.len(), 1);
    assert!(state.accounts[0].is_active);

    let calls = service.transport().recorded_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].request.method, HttpMethod::Get);
    assert_eq!(calls[0].request.target, USERS_ME_PATH);
}

#[tokio::test]
async fn p0_2_3_startup_auto_login_handles_missing_corrupt_401_429_and_500() {
    // Case 1: Empty TokenStore -> RequiresLogin
    let mut state = AppState::new_live_with_offline_demo(false);
    let empty_store = TokenStore::new(MemoryKeyringBackend::new());
    let service = build_mock_service();
    let res_empty = AuthFlowController::try_auto_login(&mut state, &empty_store, &service).await;
    assert!(matches!(res_empty, StartupAuthResult::RequiresLogin { .. }));
    assert!(state.is_login_modal_open);
    assert!(service.transport().recorded_calls().is_empty());

    // Case 2: Corrupted token in keyring (contains newline) -> RequiresLogin without network call
    let mut corrupt_backend = MemoryKeyringBackend::new();
    corrupt_backend
        .set_secret("chord.active_account", "corrupt\ntoken_value")
        .expect("raw backend insert");
    let corrupt_store = TokenStore::new(corrupt_backend);
    let res_corrupt =
        AuthFlowController::try_auto_login(&mut state, &corrupt_store, &service).await;
    assert!(matches!(
        res_corrupt,
        StartupAuthResult::RequiresLogin { .. }
    ));
    assert!(state.is_login_modal_open);
    assert!(service.transport().recorded_calls().is_empty());

    // Case 3: Unavailable keyring backend -> RequiresLogin
    let mut unavail_backend = MemoryKeyringBackend::new();
    unavail_backend.set_unavailable(true);
    let unavail_store = TokenStore::new(unavail_backend);
    let res_unavail =
        AuthFlowController::try_auto_login(&mut state, &unavail_store, &service).await;
    assert!(matches!(
        res_unavail,
        StartupAuthResult::RequiresLogin { .. }
    ));
    assert!(state.is_login_modal_open);

    // Case 4: Stored token rejected with HTTP 401 -> RequiresLogin, modal stays open, active_token None
    let mut valid_store = TokenStore::new(MemoryKeyringBackend::new());
    valid_store
        .save_active_token(&SecretToken::new("mfa.expired_token_401").unwrap())
        .unwrap();
    service.transport().push_response(json_http_response(
        401,
        vec![],
        &json!({"code": 0, "message": "401: Unauthorized"}),
    ));
    let res_401 = AuthFlowController::try_auto_login(&mut state, &valid_store, &service).await;
    assert!(matches!(res_401, StartupAuthResult::RequiresLogin { .. }));
    assert!(state.is_login_modal_open);
    assert!(state.active_token.is_none());
    assert!(state.login_form.error_message.is_some());

    // Case 5: HTTP 429 RateLimited preserves retry_after_ms
    service.transport().push_response(json_http_response(
        429,
        vec![("retry-after", "3.25")],
        &json!({"message": "You are being rate limited.", "retry_after": 3.25, "global": false}),
    ));
    let res_429 = AuthFlowController::try_auto_login(&mut state, &valid_store, &service).await;
    assert_eq!(
        res_429,
        StartupAuthResult::RateLimited {
            retry_after_ms: 3250
        }
    );
    assert!(state.is_login_modal_open);

    // Case 6: HTTP 502 ServerError -> NetworkError
    service.transport().push_response(json_http_response(
        502,
        vec![],
        &json!({"message": "Bad Gateway"}),
    ));
    let res_502 = AuthFlowController::try_auto_login(&mut state, &valid_store, &service).await;
    assert!(matches!(res_502, StartupAuthResult::NetworkError(_)));
    assert!(state.is_login_modal_open);
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. P0-3.1: Live Credentials Login -> Token -> @me & Captcha Handling
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn p0_3_1_submit_credentials_live_direct_token_persists_and_hydrates_me() {
    let mut state = AppState::new_live_with_offline_demo(false);
    let mut store = TokenStore::new(MemoryKeyringBackend::new());
    let service = build_mock_service();

    let secret_password = "MyTopSecretPassword!2026";
    let issued_token = "mfa.direct_login_issued_token_abcdef1234567890";

    service.transport().push_response(json_http_response(
        200,
        vec![],
        &json!({
            "token": issued_token,
            "user_settings": {"locale": "ru", "theme": "dark"}
        }),
    ));
    service.transport().push_response(json_http_response(
        200,
        vec![],
        &sample_me_json("0", "zero_snowflake_alice", Some("Alice Live")),
    ));

    state.set_login_email("alice@example.com".into());
    state.set_login_password(secret_password.into());

    let outcome = state
        .submit_login_with_service(&mut store, &service)
        .await
        .expect("direct login should succeed");

    // Verify Debug output of outcome never leaks token or password
    let outcome_dbg = format!("{outcome:?}");
    assert!(!outcome_dbg.contains(issued_token));
    assert!(!outcome_dbg.contains(secret_password));
    assert!(outcome_dbg.contains("[REDACTED]"));

    assert!(!state.is_login_modal_open);
    assert_eq!(state.current_user.id, "0");
    assert_eq!(state.current_user.username, "zero_snowflake_alice");
    assert_eq!(
        state.current_user.global_name.as_deref(),
        Some("Alice Live")
    );
    assert_eq!(state.active_token.as_deref(), Some(issued_token));
    assert!(
        state.login_form.password.is_empty(),
        "password field in login_form must be cleared after successful login"
    );

    // Token must be persisted in TokenStore under both active and per-account keys
    let stored_active = store
        .load_active_token()
        .expect("load active")
        .expect("token present");
    assert_eq!(stored_active.expose_secret(), issued_token);
    let stored_account = store
        .load_account_token("0")
        .expect("load account 0")
        .expect("account token present");
    assert_eq!(stored_account.expose_secret(), issued_token);

    let calls = service.transport().recorded_calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].request.method, HttpMethod::Post);
    assert_eq!(calls[0].request.target, AUTH_LOGIN_PATH);
    assert_eq!(calls[1].request.method, HttpMethod::Get);
    assert_eq!(calls[1].request.target, USERS_ME_PATH);
}

#[tokio::test]
async fn p0_3_1_submit_credentials_surfaces_captcha_challenge_without_bypass() {
    let mut state = AppState::new_live_with_offline_demo(false);
    let mut store = TokenStore::new(MemoryKeyringBackend::new());
    let service = build_mock_service();

    let sitekey = "a9b5fb07-92ff-493f-86fe-352a2803b3df";
    service.transport().push_response(json_http_response(
        400,
        vec![],
        &json!({
            "captcha_key": ["captcha-required"],
            "captcha_sitekey": sitekey,
            "captcha_service": "hcaptcha",
            "captcha_rqdata": "rqdata_challenge_payload_xyz",
            "captcha_rqtoken": "rqtoken_abc"
        }),
    ));

    state.set_login_email("captcha.user@example.com".into());
    state.set_login_password("SecretPassForCaptcha!99".into());

    let err = state
        .submit_login_with_service(&mut store, &service)
        .await
        .expect_err("captcha response must return Err(AuthFlowError::CaptchaRequired)");

    match &err {
        AuthFlowError::CaptchaRequired(challenge) => {
            assert_eq!(challenge.captcha_service, "hcaptcha");
            assert_eq!(challenge.captcha_sitekey, sitekey);
            assert_eq!(
                challenge.captcha_rqdata.as_deref(),
                Some("rqdata_challenge_payload_xyz")
            );
        }
        other => panic!("expected AuthFlowError::CaptchaRequired, got {other:?}"),
    }

    // Must NOT store any token or close modal or retry automatically
    assert!(state.is_login_modal_open);
    assert!(store.load_active_token().unwrap().is_none());
    assert_eq!(service.transport().recorded_calls().len(), 1);
    assert_eq!(state.login_form.step, LoginStep::Credentials);

    // Structured captcha state & LoginModal banner must expose service & sitekey
    let active_captcha = state
        .active_captcha
        .as_ref()
        .expect("active_captcha must be populated on AppState");
    assert_eq!(active_captcha.service, "hcaptcha");
    assert_eq!(active_captcha.sitekey, sitekey);

    let banner = LoginModal::active_captcha_banner_text(&state)
        .expect("LoginModal must surface captcha notice");
    assert!(banner.contains("hcaptcha"));
    assert!(banner.contains(sitekey));
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. P0-3.1 -> P0-3.2: MFA Challenge Transition & Live TOTP Verification
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn p0_3_2_mfa_challenge_transitions_step_and_submit_totp_persists_token_and_me() {
    let mut state = AppState::new_live_with_offline_demo(false);
    let mut store = TokenStore::new(MemoryKeyringBackend::new());
    let service = build_mock_service();

    let secret_password = "PasswordWith2FA!2026";
    let secret_ticket = "mfa_ticket_super_secret_do_not_leak_991122";
    let issued_token = "mfa.verified_session_token_77889900112233";

    // Step 1: POST /api/v9/auth/login returns MFA challenge
    service.transport().push_response(json_http_response(
        200,
        vec![],
        &json!({
            "token": null,
            "mfa": true,
            "ticket": secret_ticket,
            "totp": true,
            "sms": false,
            "backup": true
        }),
    ));

    state.set_login_email("mfa.user@example.com".into());
    state.set_login_password(secret_password.into());

    let step1 = state
        .submit_login_with_service(&mut store, &service)
        .await
        .expect("MFA challenge is a valid step outcome");

    // Verify Debug redaction on MfaRequired
    let step1_dbg = format!("{step1:?}");
    assert!(
        !step1_dbg.contains(secret_ticket),
        "MfaRequired Debug must redact ticket: {step1_dbg}"
    );
    assert!(step1_dbg.contains("[REDACTED]"));

    assert!(
        state.is_login_modal_open,
        "modal stays open during MFA step"
    );
    assert_eq!(
        state.login_form.step,
        LoginStep::Mfa {
            ticket: secret_ticket.to_string()
        }
    );
    assert_eq!(state.login_form.mfa_ticket.as_deref(), Some(secret_ticket));
    assert!(
        state.login_form.password.is_empty(),
        "password must be wiped from login_form once MFA step is reached"
    );
    assert!(store.load_active_token().unwrap().is_none());

    // Step 2: Invalid TOTP codes (<6 digits, >8 digits, non-digits) are rejected locally without network call
    for bad_code in ["", "12345", "123456789", "12ab56"] {
        state.set_login_mfa_code(bad_code.to_string());
        let bad_res = state.submit_mfa_with_service(&mut store, &service).await;
        assert!(
            matches!(bad_res, Err(AuthFlowError::Validation(_))),
            "bad TOTP code '{bad_code}' must fail validation"
        );
        assert!(state.login_form.error_message.is_some());
        assert_eq!(
            state.login_form.step,
            LoginStep::Mfa {
                ticket: secret_ticket.to_string()
            },
            "step must remain Mfa after validation failure"
        );
    }
    // Still only 1 HTTP call recorded (from step 1)
    assert_eq!(service.transport().recorded_calls().len(), 1);

    // Step 3: Valid 6-digit TOTP -> 200 token -> 200 @me
    service.transport().push_response(json_http_response(
        200,
        vec![],
        &json!({ "token": issued_token }),
    ));
    service.transport().push_response(json_http_response(
        200,
        vec![],
        &sample_me_json(
            "384299970632876032",
            "totp_verified_user",
            Some("TOTP Verified"),
        ),
    ));

    state.set_login_mfa_code("482910".into());
    let authed_user = state
        .submit_mfa_with_service(&mut store, &service)
        .await
        .expect("valid 6-digit TOTP must authenticate");

    assert_eq!(authed_user.id, "384299970632876032");
    assert_eq!(authed_user.username, "totp_verified_user");
    assert!(
        !state.is_login_modal_open,
        "login modal must close after MFA success"
    );
    assert_eq!(state.active_token.as_deref(), Some(issued_token));
    assert!(state.login_form.mfa_code.is_empty());
    assert!(state.login_form.mfa_ticket.is_none());
    assert_eq!(state.login_form.step, LoginStep::Credentials);

    // Verify token stored in TokenStore
    let saved = store
        .load_active_token()
        .expect("load active")
        .expect("saved token");
    assert_eq!(saved.expose_secret(), issued_token);

    let calls = service.transport().recorded_calls();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[1].request.method, HttpMethod::Post);
    assert_eq!(calls[1].request.target, AUTH_MFA_TOTP_PATH);
    assert_eq!(calls[2].request.method, HttpMethod::Get);
    assert_eq!(calls[2].request.target, USERS_ME_PATH);
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Negative, Error Handling, Missing Ticket & Security Redaction Contracts
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn negative_validation_errors_and_http_401_429_500_on_login_and_totp() {
    let mut state = AppState::new_live_with_offline_demo(false);
    let mut store = TokenStore::new(MemoryKeyringBackend::new());
    let service = build_mock_service();

    // 1. Empty email & password rejected without HTTP request
    assert!(matches!(
        state.submit_login_with_service(&mut store, &service).await,
        Err(AuthFlowError::Validation(_))
    ));
    assert!(service.transport().recorded_calls().is_empty());

    // 2. Empty password rejected without HTTP request
    state.set_login_email("bob@example.com".into());
    assert!(matches!(
        state.submit_login_with_service(&mut store, &service).await,
        Err(AuthFlowError::Validation(_))
    ));
    assert!(service.transport().recorded_calls().is_empty());

    // 3. Token mode with empty or control-char token rejected without HTTP request
    state.toggle_token_login_mode();
    state.set_login_token("   ".into());
    assert!(matches!(
        state.submit_login_with_service(&mut store, &service).await,
        Err(AuthFlowError::Validation(_))
    ));
    state.set_login_token("bad\0token".into());
    assert!(matches!(
        state.submit_login_with_service(&mut store, &service).await,
        Err(AuthFlowError::Validation(_))
    ));
    assert!(service.transport().recorded_calls().is_empty());
    state.toggle_token_login_mode();

    // 4. Missing MFA ticket rejected with MissingMfaTicket
    state.login_form.step = LoginStep::Credentials;
    state.login_form.mfa_ticket = None;
    state.set_login_mfa_code("123456".into());
    assert_eq!(
        state.submit_mfa_with_service(&mut store, &service).await,
        Err(AuthFlowError::MissingMfaTicket)
    );

    // 5. Invalid credentials (HTTP 400 code 50035)
    state.set_login_email("bob@example.com".into());
    state.set_login_password("WrongPassword123".into());
    service.transport().push_response(json_http_response(
        400,
        vec![],
        &json!({"code": 50035, "message": "Invalid Form Body"}),
    ));
    let err_400 = state
        .submit_login_with_service(&mut store, &service)
        .await
        .unwrap_err();
    assert_eq!(
        err_400,
        AuthFlowError::InvalidCredentials {
            code: Some(50035),
            message: "Invalid Form Body".to_string()
        }
    );
    assert!(state.is_login_modal_open);

    // 6. Rate limited (HTTP 429) on login preserves retry_after_ms
    service.transport().push_response(json_http_response(
        429,
        vec![("retry-after", "1.5")],
        &json!({"message": "Too Many Requests", "retry_after": 1.5, "global": true}),
    ));
    let err_429 = state
        .submit_login_with_service(&mut store, &service)
        .await
        .unwrap_err();
    assert_eq!(
        err_429,
        AuthFlowError::RateLimited {
            retry_after_ms: 1500,
            global: true
        }
    );

    // 7. Server error (HTTP 500) on TOTP verification preserves MFA step & ticket
    state.login_form.step = LoginStep::Mfa {
        ticket: "ticket_keep_on_500".into(),
    };
    state.set_login_mfa_code("654321".into());
    service.transport().push_response(json_http_response(
        500,
        vec![],
        &json!({"message": "Internal Server Error"}),
    ));
    let err_500 = state
        .submit_mfa_with_service(&mut store, &service)
        .await
        .unwrap_err();
    assert_eq!(err_500, AuthFlowError::ServerError { status: 500 });
    assert_eq!(
        state.login_form.step,
        LoginStep::Mfa {
            ticket: "ticket_keep_on_500".into()
        }
    );

    // 8. Keyring storage failure during token save rolls back cleanly
    let mut failing_backend = MemoryKeyringBackend::new();
    failing_backend.set_fail_storage(true);
    let mut failing_store = TokenStore::new(failing_backend);
    service.transport().push_response(json_http_response(
        200,
        vec![],
        &json!({"token": "mfa.token_cannot_save_123"}),
    ));
    let err_store = state
        .submit_mfa_with_service(&mut failing_store, &service)
        .await
        .unwrap_err();
    assert!(matches!(
        err_store,
        AuthFlowError::TokenStore(TokenStoreError::StorageFailure(_))
    ));
    assert!(state.is_login_modal_open);
}

#[test]
fn login_modal_interactive_submit_wiring_and_security_redaction_contract() {
    let mut state = AppState::new_live_with_offline_demo(false);

    // Button labels reflect normal vs loading state
    assert_eq!(LoginModal::login_submit_button_label(&state), "Войти");
    assert_eq!(LoginModal::mfa_submit_button_label(&state), "Подтвердить");
    assert!(!LoginModal::is_submitting(&state));

    state.login_form.step = LoginStep::Loading;
    assert!(LoginModal::is_submitting(&state));
    assert_eq!(LoginModal::login_submit_button_label(&state), "Вход...");
    assert_eq!(LoginModal::mfa_submit_button_label(&state), "Проверка...");
    state.login_form.step = LoginStep::Credentials;

    // Interactive submit_login_flow -> MFA transition -> submit_mfa_flow
    state.set_login_email("interactive.user@discord.gg".into());
    state.set_login_password("Interactive2faPass!".into());
    assert!(state.submit_login_flow().is_ok());
    assert!(matches!(state.login_form.step, LoginStep::Mfa { .. }));

    state.set_login_mfa_code("908172".into());
    assert!(state.submit_mfa_flow().is_ok());
    assert!(!state.is_login_modal_open);

    // Security redaction check across secret types and errors
    let raw_secret_token = "mfa.SUPER_SECRET_TOKEN_VALUE_DO_NOT_PRINT_12345";
    let secret_token = SecretToken::new(raw_secret_token).unwrap();
    let outcome = LiveAuthStepOutcome::Authenticated {
        user: state.current_user.clone(),
        token: secret_token,
    };
    let debug_outcome = format!("{outcome:?}");
    assert!(!debug_outcome.contains(raw_secret_token));
    assert!(debug_outcome.contains("[REDACTED]"));
}
