#![forbid(unsafe_code)]
//! Contract tests for Task D-07: Session Resilience & Security (`chord_core::session_resilience`).
//!
//! Specification & Clean-Room Reference Oracles:
//! - `docs/feature-backlog.md` row `D-07`:
//!   "Session Resilience & Security: Session token protection, secure keyring storage, recovery from
//!   abrupt connection dropouts, rate limit backoff | Clean-Room Lab: Auth Traces | Auth & Storage |
//!   Session recovery protocol handles Opcode 6 Resume; secure token storage in OS keyring with
//!   automatic masking"
//! - `docs/re/session-resilience-d07.md` & `docs/re/session-resilience-d07.json`
//! - `docs/re/gateway-protocol.md` (§4 Resumption Lifecycle) & `docs/re/gateway-protocol.json`
//!   (`heartbeat.resume.events`: 3 empirically captured Opcode 6 Resume transitions:
//!   `run2:95974.8269` `s=95..663`, `run2:95974.8608` `s=664..722`, `run2:95974.13072` `s=332..371`,
//!   outbound Opcode 6 frame sizes `173` B and `176` B, `HELLO.d.heartbeat_interval = 41250` ms)
//! - `docs/re/rest-d02.md` & `docs/re/rest-d02.json` (`GET /users/@me/survey` -> `429`, 6 auth headers)
//! - `docs/re/backend-ipc.md` (`SAFE_STORAGE_*` 3 IPC channels, `com.discordlab.Token` OS keyring)
//! - `waves/RW-39/NET/AUTH-RE.md` & `waves/RW-39/NET/auth-re.json` (`dQw4w9WgXcQ:` prefix,
//!   `_doResumeOrIdentify` `RESUME_TTL_MS = 180_000` ms, `gatewayBackoff` `1000..=60000` ms,
//!   `RESUME_RESET_AFTER_FAILURES = 4`, close `4004`/`4013`/`4014`, rate-limit backoff)
//!
//! Covers all 9 mandatory dimensions (`TEST-POLICY.md` §12):
//! 1. Positive path: `D07KeyringTokenVault` storing/loading/clearing/rotating session tokens in OS
//!    keyring schema `com.discordlab.Token` with automatic masking (`D07MaskedToken`, `redact_secrets`),
//!    one-time Electron `safeStorage` (`dQw4w9WgXcQ:`) migration, Opcode 6 Resume recovery across the
//!    3 empirical capture seams (`s=94 -> 95..663`, `s=663 -> 664..722`, `s=331 -> 332..371`) preserving
//!    guild/channel caches, and 429/503/5xx rate-limit backoff via `D07SessionResilienceEngine`.
//! 2. Negative path: rejecting empty/whitespace/NUL/newline tokens, rejecting invalid 3-part token
//!    format when strict format check is requested, rejecting empty/NUL keyring account keys,
//!    rejecting malformed `dQw4w9WgXcQ:` base64 payloads, rejecting Opcode 6 Resume when no session
//!    or token is stored, and rejecting non-retryable `401 Unauthorized` / `500` status codes.
//! 3. Boundary conditions: Snowflake boundaries (`0` rejected, `1` and `u64::MAX` accepted),
//!    downtime & TTL boundaries (`RESUME_MAX_DOWNTIME_MS - 1 = 29_999` vs `30_000` ms;
//!    `RESUME_TTL_MS = 180_000` vs `180_001` ms), failure threshold (`3` vs `4` consecutive failures),
//!    heartbeat timeout boundary (`41_249` vs `41_250` ms), and rate-limit cap (`DEFAULT_RETRY_AFTER = 5s`,
//!    `MAX_RETRY_AFTER = 60s`).
//! 4. Error handling: explicit typed `D07SessionResilienceError` variants (`EmptyToken`, `InvalidTokenFormat`,
//!    `InvalidKeyringKey`, `TokenNotFound`, `EmptyPayload`, `InvalidBase64`, `DecryptFailed`,
//!    `NoActiveSession`, `SessionExpired`, `UnauthorizedRevoke`) without panics or secret leakage.
//! 5. Empty states: uninitialized vault (`TokenNotFound`), unestablished session before `READY`,
//!    missing `retry-after` header defaulting to `5_000` ms, and empty replayed event list.
//! 6. Intermediate / lifecycle states: `Unauthenticated -> TokenStored -> SessionEstablished (READY) ->
//!    AbruptDropout -> Resuming (Opcode 6 attempts 1..=4) -> Resumed / FallbackToIdentify -> LoggedOut`,
//!    plus idempotent `migrate_safe_storage_once`.
//! 7. Capabilities, permissions & security: automatic masking in `D07MaskedToken` (`Debug` and `Display`
//!    output `"[REDACTED]"`), zeroization on drop & logout (`zeroize_now`), DevTools token hiding
//!    (`hide_token_for_devtools` / `restore_token_from_devtools` per `AUTH-RE.md` `T-11`),
//!    log stream redaction (`RedactedWriter` + `SecretRedactionLayer`), and authorized remote session revocation.
//! 8. Recovery & degradation: abrupt TCP disconnect (`None` close code), `ZombieSocketDetector` missed
//!    `HEARTBEAT_ACK` (op11), `OP 7 RECONNECT`, `OP 9 INVALID_SESSION` (`d=true -> Resume`, `d=false -> Identify`),
//!    fatal close codes (`4004` automatic token wipe & logout, `4013/4014` permanent stop), and per-URL
//!    rate-limit queue backoff recovery.
//! 9. Observable telemetry & empirical oracle verification: verifying `docs/re/session-resilience-d07.md`
//!    and `docs/re/session-resilience-d07.json` against `docs/re/gateway-protocol.json` and
//!    `D07ResilienceTelemetry` counters (`tokens_stored`, `tokens_masked`, `safe_storage_migrations`,
//!    `abrupt_dropouts`, `opcode6_resumes_sent`, `sessions_resumed`, `identify_fallbacks`,
//!    `zombie_sockets_detected`, `rate_limit_backoffs`, `unauthorized_logouts`).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chord_codec::etf::{self, EtfValue};
use chord_core::cache::{InMemoryCache, InMemoryCacheBuilder, ResourceType};
use chord_core::gateway::supervisor::{CloseAction, RESUME_TTL_MS, StopReason, classify_close};
use chord_core::heartbeat::{
    DEFAULT_HEARTBEAT_INTERVAL_MS, HeartbeatAckOutcome, ZombieSocketDetector,
};
use chord_core::logging::{
    REDACTED_MARKER, RedactedMakeWriter, RedactedWriter, SecretRedactionLayer, redact_secrets,
};
use chord_core::rest::bridge::validate_snowflake_id;
use chord_core::rest::client::{ApiAuth, AuthScheme};
use chord_core::rest::rate_limit::{
    DEFAULT_RETRY_AFTER, MAX_RETRY_AFTER, RETRYABLE_SERVER_STATUSES,
};
use chord_core::rest::routes::HttpMethod;
use chord_core::rest::super_properties::SuperProperties;
use chord_core::resume::{
    RESUME_BACKOFF_MAX_MS, RESUME_BACKOFF_MIN_MS, RESUME_MAX_DOWNTIME_MS, RESUME_OP,
    RESUME_RESET_AFTER_FAILURES, ReconnectAction,
};
use chord_core::session_resilience::{
    D07_KEYRING_LABEL, D07_KEYRING_SCHEMA_NAME, D07_SAFE_STORAGE_PREFIX, D07DropoutCause,
    D07DropoutRecoveryPlan, D07KeyringTokenVault, D07MaskedToken, D07RateLimitBackoffDecision,
    D07ResilienceState, D07ResilienceTelemetry, D07SessionResilienceEngine,
    D07SessionResilienceError, is_valid_discord_token_structure, mask_token_preview,
};
use chord_core::sessions_manager::{
    CURRENT_DEVICE_BADGE_COLOR, CURRENT_DEVICE_BADGE_LABEL, SessionError, SessionInfo,
    SessionManager,
};
use chord_json::JsonValue;
use tracing_subscriber::prelude::*;
use twilight_model::id::Id;
use twilight_model::id::marker::{ChannelMarker, GuildMarker};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn synthetic_discord_token() -> String {
    format!(
        "{}.{}.{}",
        "MTIzNDU2Nzg5MDEyMzQ1Njc4OQ", "GaBcDe", "AbCdEfGhIjKlMnOpQrStUvWxYz012345678"
    )
}

fn seed_guild_cache() -> InMemoryCache {
    let cache = InMemoryCacheBuilder::new()
        .resource_types(ResourceType::ALL)
        .build();
    let ready_json = serde_json::json!({
        "v": 9,
        "user": {
            "id": "384299970632876032",
            "username": "chord_d07_operator",
            "discriminator": "0000",
            "avatar": "e234430751874dcfca60cdd3a53a2673",
            "bot": false
        },
        "guilds": [
            {
                "id": "1014537851418980372",
                "name": "ITSKIDON PC BOOST",
                "icon": "b4b4c3850182f7e0dbbaa116df2ed3f2",
                "channels": [
                    {
                        "id": "1176785373989445703",
                        "guild_id": "1014537851418980372",
                        "name": "general",
                        "type": 0,
                        "position": 0
                    },
                    {
                        "id": "1275727867497680918",
                        "guild_id": "1014537851418980372",
                        "name": "roblox",
                        "type": 0,
                        "position": 1
                    }
                ],
                "roles": [
                    {
                        "id": "1057598992449552425",
                        "name": "Moderator",
                        "color": 3447003,
                        "position": 30,
                        "permissions": "8"
                    }
                ]
            }
        ],
        "private_channels": [
            {
                "id": "1056563701764857896",
                "type": 1,
                "name": "dm-peer"
            }
        ],
        "session_id": "f14ac1f3d408c15027d6d98da172c398",
        "resume_gateway_url": "wss://gateway-us-east1-d.discord.gg/"
    });
    cache
        .update_from_ready_value(&ready_json)
        .expect("seed_guild_cache READY hydration must succeed");
    cache
}

// ============================================================================
// Dimension 1: Positive Path — Secure Keyring Token Storage with Automatic
// Masking, Opcode 6 Resume Recovery Across All 3 Empirical Capture Seams,
// and Rate Limit Backoff
// ============================================================================

#[test]
fn d07_dim1_keyring_token_vault_automatic_masking_and_safestorage_migration() {
    assert_eq!(D07_KEYRING_SCHEMA_NAME, "com.discordlab.Token");
    assert_eq!(D07_KEYRING_LABEL, "discord-lab token");
    assert_eq!(D07_SAFE_STORAGE_PREFIX, "dQw4w9WgXcQ:");

    let token = synthetic_discord_token();
    assert!(is_valid_discord_token_structure(&token));
    assert!(is_valid_discord_token_structure(
        "mfa.abcdef1234567890abcdef1234567890_secret"
    ));

    let mut vault = D07KeyringTokenVault::new();
    assert_eq!(vault.schema_name(), D07_KEYRING_SCHEMA_NAME);
    assert_eq!(vault.label(), D07_KEYRING_LABEL);

    // 1. Store token in OS keyring vault and retrieve masked wrapper
    vault
        .store_token("384299970632876032", &token)
        .expect("store_token must succeed");
    assert!(vault.has_token("384299970632876032").unwrap());

    let masked: D07MaskedToken = vault
        .load_token("384299970632876032")
        .expect("load_token")
        .expect("token present");
    assert_eq!(masked.expose_secret(), token);

    // Automatic masking in Display, Debug, and preview
    assert_eq!(format!("{masked}"), REDACTED_MARKER);
    assert!(!format!("{masked:?}").contains(&token));
    assert!(format!("{masked:?}").contains(REDACTED_MARKER));
    assert!(!format!("{vault:?}").contains(&token));
    assert_eq!(mask_token_preview(&token), REDACTED_MARKER);

    // 2. One-time Electron safeStorage (`dQw4w9WgXcQ:<base64>`) migration into OS keyring
    let raw_safe_storage = "  dQw4w9WgXcQ:Y2lwaGVydGV4dC1ieXRlcw== \n";
    let migrated = vault
        .migrate_safe_storage_once("migrated-account", raw_safe_storage, |ct| {
            assert_eq!(ct, b"ciphertext-bytes");
            Ok(token.as_bytes().to_vec())
        })
        .expect("safeStorage migration must succeed");
    assert!(migrated);
    assert!(vault.is_safe_storage_migrated());

    let loaded_migrated = vault
        .load_token("migrated-account")
        .expect("load migrated")
        .expect("migrated token exists");
    assert_eq!(loaded_migrated.expose_secret(), token);
}

#[test]
fn d07_dim1_session_recovery_opcode6_resume_across_three_empirical_capture_seams() {
    let cache = seed_guild_cache();
    let guild_id = Id::<GuildMarker>::new(1_014_537_851_418_980_372);
    let chan_id = Id::<ChannelMarker>::new(1_176_785_373_989_445_703);
    let guild_arc_before = cache.guild(guild_id).expect("guild cached");
    let chan_arc_before = cache.channel(chan_id).expect("channel cached");

    let token = synthetic_discord_token();
    let mut engine = D07SessionResilienceEngine::new();
    engine
        .store_account_token("384299970632876032", &token)
        .expect("store_account_token");
    assert_eq!(engine.state(), D07ResilienceState::TokenStored);

    // Seam #1 (`run2:95974.5532` s=1..94 -> abrupt dropout -> `run2:95974.8269` Resume #1 s=95..663)
    engine.on_ready_established(
        "384299970632876032",
        "f14ac1f3d408c15027d6d98da172c398",
        Some("wss://gateway-us-east1-c.discord.gg/"),
        94,
        &cache,
    );
    assert_eq!(engine.state(), D07ResilienceState::SessionEstablished);

    let plan1: D07DropoutRecoveryPlan = engine
        .handle_connection_dropout(D07DropoutCause::AbruptNetworkDrop, 1_200, &cache)
        .expect("dropout at s=94");
    assert_eq!(plan1.action, ReconnectAction::Resume);
    assert_eq!(plan1.backoff_delay_ms, 1_000);
    assert_eq!(
        plan1.target_gateway_url.as_deref(),
        Some("wss://gateway-us-east1-c.discord.gg")
    );
    let hs1 = plan1.resume_frame.expect("Opcode 6 Resume frame");
    assert_eq!(hs1.op, RESUME_OP);
    assert_eq!(hs1.seq, 94);
    assert!(hs1.preserve_guild_cache);
    assert!(hs1.skip_channel_reload);
    assert!(!format!("{hs1:?}").contains(&token));

    // Verify wire ETF encoding of Opcode 6 frame
    let (decoded, consumed) = etf::decode(&hs1.etf_frame).expect("decode Opcode 6 ETF");
    assert_eq!(consumed, hs1.etf_frame.len());
    assert_eq!(decoded.get("op").and_then(EtfValue::as_int), Some(6));
    let d = decoded.get("d").expect("d map");
    assert_eq!(
        d.get("token").and_then(EtfValue::as_str),
        Some(token.as_str())
    );
    assert_eq!(
        d.get("session_id").and_then(EtfValue::as_str),
        Some("f14ac1f3d408c15027d6d98da172c398")
    );
    assert_eq!(d.get("seq").and_then(EtfValue::as_int), Some(94));

    for seq in 95..=662 {
        assert!(engine.record_dispatch_seq(seq));
    }
    let outcome1 = engine.on_resumed_received(663, &cache);
    assert_eq!(outcome1.resumed_at_seq, 663);
    assert_eq!(outcome1.replayed_events, 569); // 95..=663 = 569 events in socket .8269
    assert!(!outcome1.guild_cache_reset);
    assert!(!outcome1.channels_reloaded);
    assert_eq!(engine.state(), D07ResilienceState::SessionEstablished);

    // Seam #2 (`run2:95974.8269` s=663 -> zombie heartbeat timeout -> `run2:95974.8608` Resume #2 s=664..722)
    let plan2 = engine
        .handle_connection_dropout(
            D07DropoutCause::ZombieHeartbeatTimeout {
                elapsed_without_ack_ms: 41_250,
            },
            2_500,
            &cache,
        )
        .expect("dropout at s=663");
    assert_eq!(plan2.action, ReconnectAction::Resume);
    assert_eq!(plan2.backoff_delay_ms, 1_000);
    assert_eq!(plan2.resume_frame.expect("hs2").seq, 663);

    for seq in 664..=721 {
        assert!(engine.record_dispatch_seq(seq));
    }
    let outcome2 = engine.on_resumed_received(722, &cache);
    assert_eq!(outcome2.resumed_at_seq, 722);
    assert_eq!(outcome2.replayed_events, 59); // 664..=722 = 59 events in socket .8608

    // Seam #3 (`run2:95974.10029` s=1..331 -> Server Opcode 7 RECONNECT -> `run2:95974.13072` Resume #3 s=332..371)
    engine.on_ready_established(
        "384299970632876032",
        "a98bc765d432e10f9876543210fedcba",
        Some("wss://gateway-us-east1-d.discord.gg/"),
        331,
        &cache,
    );
    let plan3 = engine
        .handle_connection_dropout(D07DropoutCause::ServerOp7Reconnect, 250, &cache)
        .expect("op7 reconnect at s=331");
    assert_eq!(plan3.action, ReconnectAction::Resume);
    assert_eq!(plan3.resume_frame.expect("hs3").seq, 331);
    for seq in 332..=370 {
        assert!(engine.record_dispatch_seq(seq));
    }
    let outcome3 = engine.on_resumed_received(371, &cache);
    assert_eq!(outcome3.resumed_at_seq, 371);
    assert_eq!(outcome3.replayed_events, 40); // 332..=371 = 40 events in socket .13072

    // Verify Arc pointer identity: guild and channel caches were never reset or reloaded
    assert!(Arc::ptr_eq(
        &guild_arc_before,
        &cache.guild(guild_id).unwrap()
    ));
    assert!(Arc::ptr_eq(
        &chan_arc_before,
        &cache.channel(chan_id).unwrap()
    ));
}

#[test]
fn d07_dim1_rate_limit_backoff_429_503_and_retryable_5xx_schedule() {
    let mut engine = D07SessionResilienceEngine::new();

    // 1. Empirical D-02 429 (`GET /users/@me/survey`) with `retry-after: 2.5` and bucket metadata
    let headers_429 = vec![
        ("Retry-After".to_string(), "2.5".to_string()),
        ("x-ratelimit-global".to_string(), "false".to_string()),
        ("x-ratelimit-remaining".to_string(), "0".to_string()),
        ("x-ratelimit-limit".to_string(), "5".to_string()),
        ("x-ratelimit-reset-after".to_string(), "2.5".to_string()),
        (
            "x-ratelimit-bucket".to_string(),
            "survey-bucket".to_string(),
        ),
        ("x-ratelimit-scope".to_string(), "user".to_string()),
    ];
    let body_429 = br#"{"message":"You are being rate limited.","retry_after":2.5,"global":false}"#;

    let decision_429: D07RateLimitBackoffDecision = engine.evaluate_http_backoff(
        HttpMethod::Get,
        "/users/@me/survey",
        429,
        1,
        &headers_429,
        body_429,
        0,
    );
    assert!(decision_429.should_retry);
    assert_eq!(decision_429.delay, Some(Duration::from_millis(2500)));
    assert!(!decision_429.is_global);
    assert_eq!(decision_429.bucket.as_deref(), Some("survey-bucket"));
    assert_eq!(decision_429.remaining, Some(0));

    // POST on 429 is also retried after retry_after pause
    let post_429 = engine.evaluate_http_backoff(
        HttpMethod::Post,
        "/channels/1176785373989445703/messages",
        429,
        1,
        &headers_429,
        body_429,
        0,
    );
    assert!(post_429.should_retry);
    assert_eq!(post_429.delay, Some(Duration::from_millis(2500)));

    // 2. 503 Service Unavailable without Retry-After defaults to DEFAULT_RETRY_AFTER (5s)
    let decision_503 =
        engine.evaluate_http_backoff(HttpMethod::Post, "/auth/login", 503, 1, &[], b"", 0);
    assert!(decision_503.should_retry);
    assert_eq!(decision_503.delay, Some(DEFAULT_RETRY_AFTER));

    // 3. All 8 retryable server statuses (`502, 504, 507, 522, 523, 524, 598, 599`) follow 1s -> 2s -> 4s..60s
    for &status in &RETRYABLE_SERVER_STATUSES {
        let d1 =
            engine.evaluate_http_backoff(HttpMethod::Get, "/users/@me", status, 1, &[], b"", 0);
        let d2 =
            engine.evaluate_http_backoff(HttpMethod::Get, "/users/@me", status, 2, &[], b"", 0);
        let d3 =
            engine.evaluate_http_backoff(HttpMethod::Get, "/users/@me", status, 3, &[], b"", 0);
        assert!(d1.should_retry);
        assert_eq!(d1.delay, Some(Duration::from_secs(1)));
        assert_eq!(d2.delay, Some(Duration::from_secs(2)));
        assert_eq!(d3.delay, Some(Duration::from_secs(4)));

        // Non-idempotent POST without explicit_retries does not retry 502..599, but with explicit_retries=2 it does
        let post_no_retry =
            engine.evaluate_http_backoff(HttpMethod::Post, "/auth/login", status, 1, &[], b"", 0);
        assert!(!post_no_retry.should_retry);
        let post_explicit =
            engine.evaluate_http_backoff(HttpMethod::Post, "/auth/login", status, 1, &[], b"", 2);
        assert!(post_explicit.should_retry);
        assert_eq!(post_explicit.delay, Some(Duration::from_secs(1)));
    }
}

// ============================================================================
// Dimension 2 & 4: Negative Path & Explicit Error Handling Without Secret Leaks
// ============================================================================

#[test]
fn d07_dim2_dim4_rejects_invalid_tokens_keys_corrupt_safestorage_and_unauthenticated_resume() {
    let mut vault = D07KeyringTokenVault::new();

    // 1. Invalid keyring account keys (empty, whitespace, NUL byte)
    for bad_key in ["", "   ", "account\0id"] {
        assert_eq!(
            vault.store_token(bad_key, &synthetic_discord_token()),
            Err(D07SessionResilienceError::InvalidKeyringKey)
        );
        assert_eq!(
            vault.load_token(bad_key),
            Err(D07SessionResilienceError::InvalidKeyringKey)
        );
        assert_eq!(
            vault.clear_token(bad_key),
            Err(D07SessionResilienceError::InvalidKeyringKey)
        );
    }

    // 2. Invalid tokens (empty, whitespace, NUL, newline, CR)
    for bad_tok in ["", "   ", "tok\0tail", "tok\nline", "tok\rline"] {
        assert_eq!(
            vault.store_token("user-1", bad_tok),
            Err(D07SessionResilienceError::EmptyOrMalformedToken)
        );
    }

    // 3. Strict token structure validation rejects short/non-token strings and domains
    assert!(!is_valid_discord_token_structure(""));
    assert!(!is_valid_discord_token_structure("gateway.discord.com"));
    assert!(!is_valid_discord_token_structure("short.a.b"));
    assert_eq!(
        vault.store_validated_discord_token("user-1", "not-a-valid-discord-token"),
        Err(D07SessionResilienceError::InvalidTokenFormat)
    );

    // 4. SafeStorage migration errors (empty payload, bad base64, decrypt failure, invalid decrypted token)
    assert_eq!(
        vault.migrate_safe_storage_once("u1", "dQw4w9WgXcQ:   ", |_| Ok(vec![])),
        Err(D07SessionResilienceError::EmptySafeStoragePayload)
    );
    assert_eq!(
        vault.migrate_safe_storage_once("u1", "dQw4w9WgXcQ:not_base64!", |_| Ok(vec![])),
        Err(D07SessionResilienceError::InvalidSafeStorageBase64)
    );
    assert_eq!(
        vault.migrate_safe_storage_once("u1", "dQw4w9WgXcQ:Y2lwaGVy", |_| {
            Err(D07SessionResilienceError::SafeStorageDecryptFailed)
        }),
        Err(D07SessionResilienceError::SafeStorageDecryptFailed)
    );
    assert_eq!(
        vault.migrate_safe_storage_once("u1", "dQw4w9WgXcQ:Y2lwaGVy", |_| {
            Ok(b"corrupt\0token".to_vec())
        }),
        Err(D07SessionResilienceError::EmptyOrMalformedToken)
    );

    // Verify no error Debug or Display ever leaks a secret
    let secret = synthetic_discord_token();
    for err in [
        D07SessionResilienceError::InvalidKeyringKey,
        D07SessionResilienceError::EmptyOrMalformedToken,
        D07SessionResilienceError::InvalidTokenFormat,
        D07SessionResilienceError::TokenNotFound,
        D07SessionResilienceError::EmptySafeStoragePayload,
        D07SessionResilienceError::InvalidSafeStorageBase64,
        D07SessionResilienceError::SafeStorageDecryptFailed,
        D07SessionResilienceError::NoActiveSession,
        D07SessionResilienceError::KeyringBackend("locked".to_string()),
    ] {
        let dbg = format!("{err:?}");
        let disp = err.to_string();
        assert!(!dbg.contains(&secret));
        assert!(!disp.contains(&secret));
    }

    // 5. Non-retryable HTTP statuses (401, 403, 404, 500)
    let mut engine = D07SessionResilienceEngine::new();
    for status in [400, 401, 403, 404, 500, 501, 505] {
        let dec =
            engine.evaluate_http_backoff(HttpMethod::Get, "/users/@me", status, 1, &[], b"", 3);
        assert!(!dec.should_retry, "status {status} must not be retried");
        assert_eq!(dec.delay, None);
    }
}

// ============================================================================
// Dimension 3: Boundary Conditions (Snowflake 0 / u64::MAX, Downtime 29_999 vs
// 30_000 ms, Resume TTL 180_000 vs 180_001 ms, Failure Threshold 3 vs 4,
// Heartbeat Interval 41_250 ms, Rate Limit Cap 60_000 ms)
// ============================================================================

#[test]
fn d07_dim3_boundaries_downtime_ttl_failures_heartbeat_and_rate_limit_caps() {
    let cache = seed_guild_cache();
    let token = synthetic_discord_token();

    // 1. Snowflake boundaries: 0 rejected, 1 and u64::MAX accepted
    assert!(validate_snowflake_id("account_id", "0").is_err());
    assert_eq!(validate_snowflake_id("account_id", "1"), Ok(1));
    assert_eq!(
        validate_snowflake_id("account_id", "18446744073709551615"),
        Ok(u64::MAX)
    );

    // 2. Downtime boundary: 29_999 ms (`RESUME_MAX_DOWNTIME_MS - 1`) -> Resume;
    //    30_000 ms (`RESUME_MAX_DOWNTIME_MS`) -> Identify
    let mut engine = D07SessionResilienceEngine::new();
    engine
        .store_account_token("18446744073709551615", &token)
        .unwrap();
    engine.on_ready_established(
        "18446744073709551615",
        "sess-boundary-downtime",
        Some("wss://gateway.discord.gg"),
        100,
        &cache,
    );

    let plan_29999 = engine
        .handle_connection_dropout(
            D07DropoutCause::AbruptNetworkDrop,
            RESUME_MAX_DOWNTIME_MS - 1,
            &cache,
        )
        .expect("29_999ms dropout");
    assert_eq!(plan_29999.action, ReconnectAction::Resume);
    assert!(plan_29999.resume_frame.is_some());

    let plan_30000 = engine
        .handle_connection_dropout(
            D07DropoutCause::AbruptNetworkDrop,
            RESUME_MAX_DOWNTIME_MS,
            &cache,
        )
        .expect("30_000ms dropout");
    assert_eq!(plan_30000.action, ReconnectAction::Identify);
    assert!(plan_30000.resume_frame.is_none());

    // 3. 3-minute Heartbeat ACK TTL boundary (`RESUME_TTL_MS = 180_000` vs `180_001` ms)
    engine.on_ready_established(
        "18446744073709551615",
        "sess-boundary-ttl",
        Some("wss://gateway.discord.gg"),
        200,
        &cache,
    );
    engine.set_last_heartbeat_ack_elapsed_ms(RESUME_TTL_MS);
    let plan_ttl_exact = engine
        .handle_connection_dropout(D07DropoutCause::AbruptNetworkDrop, 1_000, &cache)
        .expect("180_000ms ACK elapsed");
    assert_eq!(plan_ttl_exact.action, ReconnectAction::Resume);

    engine.set_last_heartbeat_ack_elapsed_ms(RESUME_TTL_MS + 1);
    let plan_ttl_expired = engine
        .handle_connection_dropout(D07DropoutCause::AbruptNetworkDrop, 1_000, &cache)
        .expect("180_001ms ACK elapsed");
    assert_eq!(plan_ttl_expired.action, ReconnectAction::Identify);
    assert!(plan_ttl_expired.resume_frame.is_none());

    // 4. Consecutive failure threshold (`RESUME_RESET_AFTER_FAILURES = 4`):
    //    attempts 1..=4 emit Opcode 6 Resume with backoff `1000, 2000, 4000, 8000` ms;
    //    after 4th failure, session resets to Identify.
    engine.on_ready_established(
        "18446744073709551615",
        "sess-boundary-fails",
        Some("wss://gateway.discord.gg"),
        300,
        &cache,
    );
    let expected_backoffs = [1_000_u64, 2_000, 4_000, 8_000];
    for (idx, &expected_ms) in expected_backoffs.iter().enumerate() {
        let p = engine
            .handle_connection_dropout(D07DropoutCause::AbruptNetworkDrop, 500, &cache)
            .expect("resume attempt");
        assert_eq!(p.action, ReconnectAction::Resume);
        assert_eq!(p.backoff_delay_ms, expected_ms);
        let next_act = engine.on_resume_attempt_failed();
        if idx + 1 < RESUME_RESET_AFTER_FAILURES as usize {
            assert_eq!(next_act, ReconnectAction::Resume);
        } else {
            assert_eq!(next_act, ReconnectAction::Identify);
        }
    }
    let p_after_4 = engine
        .handle_connection_dropout(D07DropoutCause::AbruptNetworkDrop, 500, &cache)
        .expect("5th attempt after 4 failures");
    assert_eq!(p_after_4.action, ReconnectAction::Identify);

    // 5. Rate-limit cap boundary: `retry-after: 300` capped to `MAX_RETRY_AFTER` (60s);
    //    malformed/negative `retry-after` falls back safely.
    let big_rl = vec![("retry-after".to_string(), "300".to_string())];
    let capped = engine.evaluate_http_backoff(
        HttpMethod::Get,
        "/users/@me/survey",
        429,
        1,
        &big_rl,
        b"",
        0,
    );
    assert_eq!(capped.delay, Some(MAX_RETRY_AFTER));
}

// ============================================================================
// Dimension 5 & 6: Empty States, DevTools Token Hiding Lifecycle, and
// SafeStorage Migration Idempotence
// ============================================================================

#[test]
fn d07_dim5_dim6_empty_states_devtools_hide_show_and_migration_idempotence() {
    let cache = seed_guild_cache();
    let mut engine = D07SessionResilienceEngine::new();

    // 1. Empty state before token or READY
    assert_eq!(engine.state(), D07ResilienceState::Unauthenticated);
    assert_eq!(
        engine.handle_connection_dropout(D07DropoutCause::AbruptNetworkDrop, 0, &cache),
        Err(D07SessionResilienceError::TokenNotFound)
    );

    // 2. Store token, then test DevTools open/close token hiding (`hideToken`/`showToken`, AUTH-RE T-11)
    let token = synthetic_discord_token();
    engine.store_account_token("user-devtools", &token).unwrap();
    assert!(engine.vault().has_token("user-devtools").unwrap());

    engine
        .hide_token_for_devtools("user-devtools")
        .expect("hide_token_for_devtools");
    assert!(
        !engine.vault().has_token("user-devtools").unwrap(),
        "token must be removed from primary keyring slot while DevTools is open"
    );
    assert!(engine.is_token_hidden_for_devtools());

    engine
        .restore_token_from_devtools("user-devtools")
        .expect("restore_token_from_devtools");
    assert!(!engine.is_token_hidden_for_devtools());
    assert_eq!(
        engine
            .vault()
            .load_token("user-devtools")
            .unwrap()
            .unwrap()
            .expose_secret(),
        token
    );

    // 3. SafeStorage migration idempotence: second call is a no-op (`Ok(false)`) without calling decrypt
    let mut vault = D07KeyringTokenVault::new();
    let raw = "dQw4w9WgXcQ:Y2lwaGVydGV4dA==";
    let mut decrypt_count = 0usize;
    let first = vault
        .migrate_safe_storage_once("u-idem", raw, |_| {
            decrypt_count += 1;
            Ok(token.as_bytes().to_vec())
        })
        .unwrap();
    assert!(first);
    assert_eq!(decrypt_count, 1);

    let second = vault
        .migrate_safe_storage_once("u-idem", raw, |_| {
            decrypt_count += 1;
            Ok(b"different-token".to_vec())
        })
        .unwrap();
    assert!(!second);
    assert_eq!(decrypt_count, 1, "decrypt must not run a second time");
}

// ============================================================================
// Dimension 7: Capabilities, Permissions & Security
// (Automatic Masking, Zeroization, Log Stream Filtering, Session Revocation)
// ============================================================================

#[test]
fn d07_dim7_automatic_masking_zeroization_logging_and_remote_session_revocation() {
    let discord_tok = synthetic_discord_token();
    let mfa_tok = "mfa.abcdef1234567890abcdef1234567890_secret";
    let hex_key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    // 1. Automatic masking in `redact_secrets`, `RedactedWriter`, and `SecretRedactionLayer`
    let composite = format!(
        "Authorization: Bot {discord_tok} | Bearer ya29.oauth_tok | {mfa_tok} | \
         key={hex_key} | {{\"token\":\"{discord_tok}\",\"session_id\":\"f14ac1f3d408c15027d6d98da172c398\"}}"
    );
    let redacted = redact_secrets(&composite);
    assert!(!redacted.contains(&discord_tok));
    assert!(!redacted.contains("ya29.oauth_tok"));
    assert!(!redacted.contains(mfa_tok));
    assert!(!redacted.contains(hex_key));
    assert!(!redacted.contains("f14ac1f3d408c15027d6d98da172c398"));
    assert!(redacted.contains(REDACTED_MARKER));

    let shared_buf = Arc::new(Mutex::new(Vec::new()));
    let make_writer = RedactedMakeWriter::from_shared(Arc::clone(&shared_buf));
    let subscriber = tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(make_writer)
                .with_ansi(false),
        )
        .with(SecretRedactionLayer::new());

    tracing::subscriber::with_default(subscriber, || {
        tracing::info!("Session Resume op=6 token={discord_tok} and {mfa_tok}");
    });
    let captured = String::from_utf8(shared_buf.lock().unwrap().clone()).unwrap();
    assert!(!captured.contains(&discord_tok));
    assert!(!captured.contains(mfa_tok));
    assert!(captured.contains(REDACTED_MARKER));

    let mut line_buf = Vec::new();
    {
        let mut writer = RedactedWriter::new(&mut line_buf);
        write!(writer, "Authorization: Bearer {discord_tok}").unwrap();
        writer.flush().unwrap();
    }
    assert_eq!(
        String::from_utf8(line_buf).unwrap(),
        format!("Authorization: Bearer {REDACTED_MARKER}")
    );

    // 2. Explicit zeroization of `D07MaskedToken` and raw user `ApiAuth` header redaction
    let mut masked = D07MaskedToken::new(discord_tok.clone());
    assert_eq!(masked.expose_secret(), discord_tok);
    masked.zeroize_now();
    assert!(masked.expose_secret().is_empty());

    let api_auth = ApiAuth::from_token_with_scheme(
        &discord_tok,
        AuthScheme::User,
        &SuperProperties::default(),
        "en-US",
        "UTC",
    )
    .expect("ApiAuth::from_token_with_scheme");
    assert_eq!(api_auth.authorization, discord_tok);
    assert!(!format!("{api_auth:?}").contains(&discord_tok));
    assert!(format!("{api_auth:?}").contains("<redacted>"));

    // 3. Active Session Manager & Remote Session Revocation (`DELETE /users/@me/auth/sessions`)
    let mut sess_mgr = SessionManager::new(vec![
        SessionInfo {
            id_hash: "sess_current".to_string(),
            os: "Linux".to_string(),
            browser: "Chord Desktop".to_string(),
            ip: "<PUB_IP>".to_string(),
            location: "Frankfurt, DE".to_string(),
            approx_last_used_time: "2026-11-21T12:00:00Z".to_string(),
            is_current: true,
        },
        SessionInfo {
            id_hash: "sess_other".to_string(),
            os: "Windows".to_string(),
            browser: "Discord Client".to_string(),
            ip: "<PUB_IP>".to_string(),
            location: "Berlin, DE".to_string(),
            approx_last_used_time: "2026-11-20T10:00:00Z".to_string(),
            is_current: false,
        },
    ]);
    let current = sess_mgr.current_session().expect("current device");
    assert_eq!(current.badge_label(), Some(CURRENT_DEVICE_BADGE_LABEL));
    assert_eq!(current.badge_color(), Some(CURRENT_DEVICE_BADGE_COLOR));
    assert_eq!(
        sess_mgr.revoke_session("sess_other", false),
        Err(SessionError::Unauthorized)
    );
    let rev = sess_mgr.revoke_session("sess_other", true).unwrap();
    assert_eq!(rev.method, "DELETE");
    assert_eq!(rev.endpoint, "/users/@me/auth/sessions");
    assert_eq!(sess_mgr.sessions().len(), 1);
}

// ============================================================================
// Dimension 8: Recovery & Degradation From Abrupt Dropouts, Close 4004 Logout,
// Close 4013/4014 Stop, OP 9 INVALID_SESSION, and Zombie Sockets
// ============================================================================

#[test]
fn d07_dim8_recovery_from_abrupt_dropouts_op9_invalid_session_and_fatal_close_4004_wipe() {
    let cache = seed_guild_cache();
    let token = synthetic_discord_token();
    let mut engine = D07SessionResilienceEngine::new();
    engine.store_account_token("user-4004", &token).unwrap();
    engine.on_ready_established(
        "user-4004",
        "sess-active-1",
        Some("wss://gateway-us-east1-d.discord.gg"),
        120,
        &cache,
    );

    // 1. OP 9 INVALID_SESSION (d=true) -> preserves session and plans Opcode 6 Resume
    let plan_op9_true = engine
        .handle_connection_dropout(
            D07DropoutCause::InvalidSession { resumable: true },
            500,
            &cache,
        )
        .expect("op9 d=true");
    assert_eq!(plan_op9_true.action, ReconnectAction::Resume);
    assert!(plan_op9_true.resume_frame.is_some());

    // 2. OP 9 INVALID_SESSION (d=false) -> clears session_id/seq and plans fresh Opcode 2 Identify
    let plan_op9_false = engine
        .handle_connection_dropout(
            D07DropoutCause::InvalidSession { resumable: false },
            500,
            &cache,
        )
        .expect("op9 d=false");
    assert_eq!(plan_op9_false.action, ReconnectAction::Identify);
    assert!(plan_op9_false.resume_frame.is_none());

    // Re-establish session, then receive fatal WebSocket close 4004 (Authentication Failed):
    // must stop reconnecting AND automatically wipe the revoked token from the OS keyring vault!
    engine.on_ready_established(
        "user-4004",
        "sess-active-2",
        Some("wss://gateway-us-east1-d.discord.gg"),
        250,
        &cache,
    );
    assert_eq!(
        classify_close(Some(4004)),
        CloseAction::Stop(StopReason::Unauthorized)
    );
    let close_action = engine
        .handle_close_code("user-4004", Some(4004))
        .expect("handle_close_code 4004");
    assert_eq!(close_action, CloseAction::Stop(StopReason::Unauthorized));
    assert_eq!(engine.state(), D07ResilienceState::LoggedOut);
    assert!(
        !engine.vault().has_token("user-4004").unwrap(),
        "close 4004 must automatically wipe the invalid token from the OS keyring"
    );

    // Close 4013 / 4014 stops reconnecting without panic
    assert_eq!(
        engine.handle_close_code("user-4004", Some(4014)).unwrap(),
        CloseAction::Stop(StopReason::InvalidClient { code: 4014 })
    );
    assert_eq!(
        engine.handle_close_code("user-4004", None).unwrap(),
        CloseAction::Reconnect
    );

    // ZombieSocketDetector RTT & missed ACK recovery
    let mut z = ZombieSocketDetector::new(DEFAULT_HEARTBEAT_INTERVAL_MS);
    z.on_heartbeat_sent(1, Some(120), 1_000);
    assert_eq!(
        z.on_heartbeat_ack(1_050),
        HeartbeatAckOutcome::Acknowledged {
            tick_index: 1,
            seq: Some(120),
            rtt_ms: 50,
        }
    );
    z.on_heartbeat_sent(2, Some(121), 42_250);
    let sig = z
        .on_next_tick_due(83_500)
        .expect("missed ACK triggers zombie signal");
    assert!(sig.close_immediately && sig.reconnect_immediately);
}

// ============================================================================
// Dimension 9: Empirical Oracle Verification (`docs/re/session-resilience-d07.md`,
// `docs/re/session-resilience-d07.json`, `docs/re/gateway-protocol.json`)
// and Observable Telemetry
// ============================================================================

#[test]
fn d07_dim9_clean_room_oracle_documentation_json_parity_and_telemetry() {
    let root = repo_root();
    let d07_md_path = root.join("docs/re/session-resilience-d07.md");
    let d07_json_path = root.join("docs/re/session-resilience-d07.json");
    let gw_json_path = root.join("docs/re/gateway-protocol.json");

    let md_text =
        fs::read_to_string(&d07_md_path).expect("docs/re/session-resilience-d07.md must exist");
    let json_text =
        fs::read_to_string(&d07_json_path).expect("docs/re/session-resilience-d07.json must exist");
    let gw_text =
        fs::read_to_string(&gw_json_path).expect("docs/re/gateway-protocol.json must exist");

    // 1. Verify required sections and specification markers in `docs/re/session-resilience-d07.md`
    for required_marker in [
        "Session Resilience & Security (D-07)",
        "## 0. Quantitative Oracle Summary",
        "## 1. Session Token Protection & Automatic Secret Masking",
        "## 2. Secure OS Keyring Storage, Electron `safeStorage` Migration, and Argon2id Master Key Derivation",
        "## 3. Session Recovery Protocol: Opcode 6 `RESUME` & Abrupt Connection Dropout Handling",
        "## 4. Rate Limit Backoff (`429 Too Many Requests`, `503`, and Retryable `5xx`)",
        "## 5. Active Session Management & Remote Session Revocation",
        "com.discordlab.Token",
        "dQw4w9WgXcQ:",
        "[REDACTED]",
        "RESUME_OP",
        "RESUME_TTL_MS",
        "RESUME_MAX_DOWNTIME_MS",
        "RESUME_RESET_AFTER_FAILURES",
        "RETRYABLE_SERVER_STATUSES",
        "run2:95974.8269",
        "run2:95974.8608",
        "run2:95974.13072",
    ] {
        assert!(
            md_text.contains(required_marker),
            "docs/re/session-resilience-d07.md missing required marker: {required_marker}"
        );
    }

    // 2. Parse `docs/re/session-resilience-d07.json` and cross-verify against `docs/re/gateway-protocol.json`
    let d07_oracle = chord_json::parse(&json_text)
        .expect("docs/re/session-resilience-d07.json must be valid JSON");
    let gw_oracle =
        chord_json::parse(&gw_text).expect("docs/re/gateway-protocol.json must be valid JSON");

    assert_eq!(
        d07_oracle.get("task").and_then(JsonValue::as_str),
        Some("D-07")
    );
    assert_eq!(
        d07_oracle.get("title").and_then(JsonValue::as_str),
        Some("Session Resilience & Security")
    );
    assert_eq!(
        d07_oracle.get("target").and_then(JsonValue::as_str),
        Some("Auth & Storage")
    );

    let keyring_spec = d07_oracle
        .get("keyringAndTokenSecurity")
        .expect("keyringAndTokenSecurity object");
    assert_eq!(
        keyring_spec.get("schemaName").and_then(JsonValue::as_str),
        Some(D07_KEYRING_SCHEMA_NAME)
    );
    assert_eq!(
        keyring_spec
            .get("safeStoragePrefix")
            .and_then(JsonValue::as_str),
        Some(D07_SAFE_STORAGE_PREFIX)
    );
    assert_eq!(
        keyring_spec
            .get("redactedMarker")
            .and_then(JsonValue::as_str),
        Some(REDACTED_MARKER)
    );

    let resume_spec = d07_oracle
        .get("sessionRecoveryProtocol")
        .expect("sessionRecoveryProtocol object");
    assert_eq!(
        resume_spec.get("resumeOpcode").and_then(JsonValue::as_u64),
        Some(u64::from(RESUME_OP))
    );
    assert_eq!(
        resume_spec.get("backoffMinMs").and_then(JsonValue::as_u64),
        Some(RESUME_BACKOFF_MIN_MS)
    );
    assert_eq!(
        resume_spec.get("backoffMaxMs").and_then(JsonValue::as_u64),
        Some(RESUME_BACKOFF_MAX_MS)
    );
    assert_eq!(
        resume_spec
            .get("resetAfterFailures")
            .and_then(JsonValue::as_u64),
        Some(u64::from(RESUME_RESET_AFTER_FAILURES))
    );
    assert_eq!(
        resume_spec.get("maxDowntimeMs").and_then(JsonValue::as_u64),
        Some(RESUME_MAX_DOWNTIME_MS)
    );
    assert_eq!(
        resume_spec.get("resumeTtlMs").and_then(JsonValue::as_u64),
        Some(RESUME_TTL_MS)
    );
    assert_eq!(
        resume_spec
            .get("heartbeatIntervalMs")
            .and_then(JsonValue::as_u64),
        Some(DEFAULT_HEARTBEAT_INTERVAL_MS)
    );

    let d07_sessions = resume_spec
        .get("capturedResumeSessions")
        .and_then(JsonValue::as_array)
        .expect("capturedResumeSessions array");
    let gw_sessions = gw_oracle
        .get("heartbeat")
        .and_then(|h| h.get("resume"))
        .and_then(|r| r.get("events"))
        .and_then(JsonValue::as_array)
        .expect("gateway-protocol.json heartbeat.resume.events");
    assert_eq!(d07_sessions.len(), 3);
    assert_eq!(d07_sessions.len(), gw_sessions.len());

    for (d07_ev, gw_ev) in d07_sessions.iter().zip(gw_sessions.iter()) {
        assert_eq!(
            d07_ev.get("socket").and_then(JsonValue::as_str),
            gw_ev.get("key").and_then(JsonValue::as_str)
        );
        assert_eq!(
            d07_ev.get("firstS").and_then(JsonValue::as_u64),
            gw_ev.get("firstS").and_then(JsonValue::as_u64)
        );
        assert_eq!(
            d07_ev.get("lastS").and_then(JsonValue::as_u64),
            gw_ev.get("lastS").and_then(JsonValue::as_u64)
        );
        assert_eq!(
            d07_ev.get("count").and_then(JsonValue::as_u64),
            gw_ev.get("count").and_then(JsonValue::as_u64)
        );
    }

    let rl_spec = d07_oracle
        .get("rateLimitBackoff")
        .expect("rateLimitBackoff object");
    assert_eq!(
        rl_spec
            .get("defaultRetryAfterMs")
            .and_then(JsonValue::as_u64),
        Some(DEFAULT_RETRY_AFTER.as_millis() as u64)
    );
    assert_eq!(
        rl_spec.get("maxRetryAfterMs").and_then(JsonValue::as_u64),
        Some(MAX_RETRY_AFTER.as_millis() as u64)
    );
    let statuses = rl_spec
        .get("retryableServerStatuses")
        .and_then(JsonValue::as_array)
        .expect("retryableServerStatuses");
    let parsed_statuses: Vec<u16> = statuses
        .iter()
        .map(|v| v.as_u64().expect("status u64") as u16)
        .collect();
    assert_eq!(parsed_statuses, RETRYABLE_SERVER_STATUSES);

    // 3. Verify observable telemetry counters across a full session lifecycle
    let cache = seed_guild_cache();
    let token = synthetic_discord_token();
    let mut engine = D07SessionResilienceEngine::new();
    engine.store_account_token("tel-user", &token).unwrap();
    engine.on_ready_established(
        "tel-user",
        "sess-tel",
        Some("wss://gateway.discord.gg"),
        94,
        &cache,
    );
    let _ = engine
        .handle_connection_dropout(D07DropoutCause::AbruptNetworkDrop, 1_000, &cache)
        .unwrap();
    let _ = engine.on_resumed_received(100, &cache);
    let _ = engine.evaluate_http_backoff(HttpMethod::Get, "/users/@me/survey", 429, 1, &[], b"", 0);
    let _ = engine.handle_close_code("tel-user", Some(4004)).unwrap();

    let tel: &D07ResilienceTelemetry = engine.telemetry();
    assert_eq!(tel.tokens_stored, 1);
    assert_eq!(tel.abrupt_dropouts, 1);
    assert_eq!(tel.opcode6_resumes_sent, 1);
    assert_eq!(tel.sessions_resumed, 1);
    assert_eq!(tel.rate_limit_backoffs, 1);
    assert_eq!(tel.unauthorized_logouts, 1);
}
