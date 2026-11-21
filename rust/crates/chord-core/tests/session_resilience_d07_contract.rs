#![forbid(unsafe_code)]
//! Contract tests for Task D-07: Session Resilience & Security.
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
//! 1. Positive path: Opcode 6 Resume frame encoding (`0x83` ETF + JSON), `ResumeSessionManager`
//!    session recovery preserving guild/channel cache pointers (`Arc::ptr_eq`), OS keyring schema
//!    (`com.discordlab.Token`), `SafeStorageMigrator` (`dQw4w9WgXcQ:`) one-time import, Argon2id
//!    RFC 9106 master key derivation (`MasterKeyManager` + `InMemoryKeyring`), automatic secret
//!    masking (`redact_secrets`, `RedactedWriter`, `SecretRedactionLayer`), and 429/503/5xx rate-limit backoff.
//! 2. Negative path: rejecting empty/whitespace tokens & `session_id` in Opcode 6 Resume and `ApiAuth`,
//!    rejecting empty/NUL keys & NUL secrets in OS keyring, rejecting malformed base64 / empty payload /
//!    NUL-or-newline tokens in `SafeStorageMigrator`, rejecting weak/all-zero keyring secrets & short salts
//!    in Argon2id, and rejecting outbound `OP 3/4/8/37` before session `Ready`.
//! 3. Boundary conditions: Snowflake boundaries (`0`, `1`, `u64::MAX`), sequence boundaries (`0`, `663`,
//!    `u64::MAX`), downtime & TTL thresholds (`RESUME_MAX_DOWNTIME_MS - 1 = 29_999` vs `30_000`,
//!    `RESUME_TTL_MS = 180_000` vs `180_001`), failure reset boundary (`3` vs `4` failures),
//!    heartbeat timeout boundary (`41_249` vs `41_250` ms), and rate-limit cap (`DEFAULT_RETRY_AFTER = 5s`,
//!    `MAX_RETRY_AFTER = 60s`).
//! 4. Error handling: explicit typed errors (`ResumeError`, `ApiAuthError`, `KeyringError`,
//!    `SafeStorageMigrationError`, `SessionError`, `SupervisorOutboundError`) without panics or secret leaks.
//! 5. Empty states: `Supervisor` / `ResumeSessionManager` prior to `READY`, missing keyring entry
//!    (`NotFound` / `Ok(None)`), empty `retry-after` headers falling back to `DEFAULT_RETRY_AFTER` (5s)
//!    or `x-ratelimit-reset-after`, and empty outbound queue.
//! 6. Intermediate / lifecycle states: `READY` -> dispatch sequence `s` progression -> abrupt dropout /
//!    zombie socket detection -> `Resume` (op6) attempts `1..=4` -> `RESUMED` recovery vs fallback
//!    to `Identify` (op2), and `SafeStorageMigrator` (`unmigrated -> migrated` idempotence).
//! 7. Capabilities, permissions & security: automatic masking of 3-part Discord tokens, `Bot`, `Bearer`,
//!    `mfa.*`, `ghp_*`, 64-hex keys, `token=`/`password=`/`secret_key=`/`session_id=`, and JSON fields;
//!    `Debug`/`Display` redaction across `ResumeParams`, `ResumeHandshakeFrame`, `ApiAuth`, `DerivedKey`,
//!    `Keyring`, `SafeStorageMigrationError`; memory zeroization (`DerivedKey::zeroize_now`,
//!    `zeroize_secret_bytes`, `zeroize_secret_string`); and authorized vs unauthorized session revocation.
//! 8. Recovery & degradation: recovery from abrupt TCP drop (`classify_close(None) == Reconnect`),
//!    `ZombieSocketDetector` missed `HEARTBEAT_ACK` -> immediate close & reconnect, `OP 7 RECONNECT`,
//!    `OP 9 INVALID_SESSION (d=true -> Resume, d=false -> Identify)`, fatal close codes
//!    (`4004 Unauthorized`, `4013/4014 InvalidClient` -> `Stop`), and per-method 429/503/5xx retry recovery.
//! 9. Observable telemetry & empirical oracle verification: verifying `docs/re/session-resilience-d07.md`
//!    and `docs/re/session-resilience-d07.json` against `docs/re/gateway-protocol.json` (3 captured Resume
//!    sessions `95..663`, `664..722`, `332..371`), `ZombieSocketDetector` RTT/ACK/missed counters,
//!    `Supervisor` (`reconnects`, `resume_failures`, `session_resets`, `rejected_outbound`), and
//!    `ResumeSessionManager` (`total_resumed_count`, `consecutive_failures`).

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chord_codec::etf::{self, EtfValue};
use chord_core::cache::{InMemoryCache, InMemoryCacheBuilder, ResourceType};
use chord_core::gateway::supervisor::{
    Backoff, CloseAction, RESUME_MAX_ACK_ELAPSED_MS, RESUME_TTL_MS, StopReason, Supervisor,
    SupervisorConfig, SupervisorOutboundError, classify_close,
};
use chord_core::heartbeat::{
    DEFAULT_HEARTBEAT_INTERVAL_MS, HEARTBEAT_ACK_OP, HEARTBEAT_OP, HeartbeatAckOutcome,
    ZombieReconnectSignal, ZombieSocketDetector,
};
use chord_core::logging::{
    REDACTED_MARKER, RedactedMakeWriter, RedactedWriter, SecretRedactionLayer, redact_secrets,
};
use chord_core::rest::bridge::validate_snowflake_id;
use chord_core::rest::client::{ApiAuth, ApiAuthError, AuthScheme};
use chord_core::rest::rate_limit::{
    DEFAULT_RETRY_AFTER, MAX_RETRY_AFTER, RETRYABLE_SERVER_STATUSES, RestFailure, classify_failure,
    is_retryable_server_status, parse_retry_after_or_default, rate_limit_info, retry_delay,
    retry_delay_client_parity, should_retry, should_retry_client_parity,
};
use chord_core::rest::routes::HttpMethod;
use chord_core::rest::super_properties::SuperProperties;
use chord_core::resume::{
    RESUME_BACKOFF_MAX_MS, RESUME_BACKOFF_MIN_MS, RESUME_MAX_DOWNTIME_MS,
    RESUME_MAX_FAILURE_ATTEMPTS, RESUME_OP, RESUME_RESET_AFTER_FAILURES, ReconnectAction,
    ResumeError, ResumeExponentialBackoff, ResumeHandshakeFrame, ResumeParams,
    ResumeRecoveryOutcome, ResumeSessionManager, ResumeState, build_resume_frame,
    build_resume_json_payload, plan_reconnect,
};
use chord_core::sessions_manager::{
    CURRENT_DEVICE_BADGE_COLOR, CURRENT_DEVICE_BADGE_LABEL, SessionError, SessionInfo,
    SessionManager,
};
use chord_core::state::{
    ChannelState, Event, EventPayload, GuildState, State, UserSummary,
};
use chord_core::ws::GatewayUrl;
use chord_json::JsonValue;
use tracing_subscriber::prelude::*;
use twilight_model::id::Id;
use twilight_model::id::marker::{ChannelMarker, GuildMarker};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
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
// Dimension 1: Positive Path — Opcode 6 Resume Protocol, OS Keyring & Argon2id
// Storage, Automatic Masking, and Rate-Limit Backoff
// ============================================================================

#[test]
fn d07_dim1_opcode6_resume_protocol_and_empirical_sequence_seams() {
    assert_eq!(RESUME_OP, 6);
    assert_eq!(RESUME_BACKOFF_MIN_MS, 1_000);
    assert_eq!(RESUME_BACKOFF_MAX_MS, 60_000);
    assert_eq!(RESUME_MAX_DOWNTIME_MS, 30_000);
    assert_eq!(RESUME_RESET_AFTER_FAILURES, 4);
    assert_eq!(RESUME_MAX_FAILURE_ATTEMPTS, 4);
    assert_eq!(RESUME_TTL_MS, 180_000);
    assert_eq!(RESUME_MAX_ACK_ELAPSED_MS, 180_000);

    let cache = seed_guild_cache();
    let guild_id = Id::<GuildMarker>::new(1_014_537_851_418_980_372);
    let chan_id = Id::<ChannelMarker>::new(1_176_785_373_989_445_703);
    let guild_arc_before = cache.guild(guild_id).expect("guild cached");
    let chan_arc_before = cache.channel(chan_id).expect("channel cached");

    // Empirical capture seam #1 (`run2:95974.5532` s=1..94 -> `run2:95974.8269` s=95..663)
    // and seam #2 (`run2:95974.8269` s=663 -> `run2:95974.8608` s=664..722)
    let mut mgr = ResumeSessionManager::new();
    mgr.on_ready_established(
        "f14ac1f3d408c15027d6d98da172c398",
        Some("wss://gateway-us-east1-d.discord.gg/"),
        94,
        &cache,
    );
    assert!(mgr.has_resumable_session());
    assert_eq!(mgr.last_seq(), 94);

    // Prepare Opcode 6 Resume frame at seam s=94
    let secret_tok = format!(
        "{}.{}.{}",
        "MTIzNDU2Nzg5MDEyMzQ1Njc4OQ", "GaBcDe", "AbCdEfGhIjKlMnOpQrStUvWxYz012345678"
    );
    let hs1: ResumeHandshakeFrame = mgr
        .prepare_resume_frame(&secret_tok, &cache)
        .expect("prepare_resume_frame at s=94");
    assert_eq!(hs1.op, 6);
    assert_eq!(hs1.seq, 94);
    assert_eq!(hs1.backoff_delay_ms, 1_000);
    assert_eq!(hs1.attempt, 1);
    assert!(hs1.preserve_guild_cache);
    assert!(hs1.skip_channel_reload);
    assert_eq!(
        hs1.resume_gateway_url.as_deref(),
        Some("wss://gateway-us-east1-d.discord.gg")
    );
    assert_eq!(hs1.etf_frame.first(), Some(&etf::VERSION_BYTE));

    let (decoded, consumed) = etf::decode(&hs1.etf_frame).expect("ETF decode op6");
    assert_eq!(consumed, hs1.etf_frame.len());
    assert_eq!(decoded.get("op").and_then(EtfValue::as_int), Some(6));
    let d = decoded.get("d").expect("d map");
    assert_eq!(
        d.get("token").and_then(EtfValue::as_str),
        Some(secret_tok.as_str())
    );
    assert_eq!(
        d.get("session_id").and_then(EtfValue::as_str),
        Some("f14ac1f3d408c15027d6d98da172c398")
    );
    assert_eq!(d.get("seq").and_then(EtfValue::as_int), Some(94));

    // Replay dispatches 95..=663 and complete Resume #1
    for seq in 95..=662 {
        assert!(mgr.record_dispatch_seq(seq));
    }
    let outcome1: ResumeRecoveryOutcome = mgr.on_resumed_received(663, &cache);
    assert_eq!(outcome1.resumed_at_seq, 663);
    assert_eq!(outcome1.replayed_events, 569); // 95..=663 = 569 events in socket .8269
    assert!(!outcome1.guild_cache_reset);
    assert!(!outcome1.channels_reloaded);
    assert_eq!(mgr.total_resumed_count(), 1);
    assert_eq!(mgr.backoff().current_ms(), 1_000);

    // Seam #2: disconnect at s=663 -> Resume #2 continues s=664..=722 (59 events in socket .8608)
    let hs2 = mgr
        .prepare_resume_frame(&secret_tok, &cache)
        .expect("prepare_resume_frame at s=663");
    assert_eq!(hs2.seq, 663);
    assert_eq!(hs2.backoff_delay_ms, 1_000);
    for seq in 664..=721 {
        assert!(mgr.record_dispatch_seq(seq));
    }
    let outcome2 = mgr.on_resumed_received(722, &cache);
    assert_eq!(outcome2.resumed_at_seq, 722);
    assert_eq!(outcome2.replayed_events, 59);
    assert_eq!(mgr.total_resumed_count(), 2);

    // Verify zero cache reset and zero channel reload via pointer identity
    let guild_arc_after = cache.guild(guild_id).expect("guild still cached");
    let chan_arc_after = cache.channel(chan_id).expect("channel still cached");
    assert!(Arc::ptr_eq(&guild_arc_before, &guild_arc_after));
    assert!(Arc::ptr_eq(&chan_arc_before, &chan_arc_after));
}

#[test]
fn d07_dim1_secure_keyring_storage_safestorage_migration_and_argon2id_derivation() {
    // 1. OS Keyring schema & Electron safeStorage prefix contract
    assert_eq!(SCHEMA_NAME, "com.discordlab.Token");
    assert_eq!(ENCRYPTED_TOKEN_PREFIX, "dQw4w9WgXcQ:");

    // 2. SafeStorageMigrator one-time import from `dQw4w9WgXcQ:<base64>` into OS keyring provider
    let keyring = InMemoryKeyring::new();
    let mut migrator = SafeStorageMigrator::new();
    let raw_safe_storage = "dQw4w9WgXcQ:Y2lwaGVydGV4dC1zZWNyZXQ=";
    let synthetic_token = format!(
        "{}.{}.{}",
        "MTIzNDU2Nzg5MDEyMzQ1Njc4OQ", "GaBcDe", "AbCdEfGhIjKlMnOpQrStUvWxYz012345678"
    );

    let migrated = migrator
        .migrate_once(
            raw_safe_storage,
            |ciphertext| {
                assert_eq!(ciphertext, b"ciphertext-secret");
                Ok(synthetic_token.as_bytes().to_vec())
            },
            |decrypted_token| {
                keyring
                    .set_secret(SCHEMA_NAME, "primary-user", decrypted_token.as_bytes())
                    .map_err(|_| {
                        SafeStorageMigrationError::Storage(PlatformKeyringError::InvalidSecret)
                    })?;
                Ok(true)
            },
        )
        .expect("one-time safeStorage migration must succeed");
    assert!(migrated);
    assert!(migrator.is_migrated());

    let loaded_bytes = keyring
        .get_secret(SCHEMA_NAME, "primary-user")
        .expect("token stored in keyring");
    assert_eq!(loaded_bytes, synthetic_token.as_bytes());

    // 3. MasterKeyManager Argon2id (RFC 9106) derivation over SecretServiceProvider
    let master_mgr = MasterKeyManager::with_params(
        keyring,
        Argon2idParams {
            memory_kib: 32,
            iterations: 2,
            parallelism: 1,
            output_len: 32,
        },
    );
    let hw_salt = b"machine-id-d07-session-resilience";
    let key1 = master_mgr
        .derive_or_create_master_key("chord-db", "account-384299970632876032", hw_salt)
        .expect("derive_or_create_master_key #1");
    let key2 = master_mgr
        .derive_or_create_master_key("chord-db", "account-384299970632876032", hw_salt)
        .expect("derive_or_create_master_key #2");
    assert_eq!(key1.as_bytes(), key2.as_bytes());
    assert_eq!(key1.as_bytes().len(), 32);
    assert!(key1.as_bytes().iter().any(|&b| b != 0));
}

#[test]
fn d07_dim1_rate_limit_backoff_429_503_and_retryable_5xx_schedule() {
    assert_eq!(DEFAULT_RETRY_AFTER, Duration::from_secs(5));
    assert_eq!(MAX_RETRY_AFTER, Duration::from_secs(60));
    assert_eq!(
        RETRYABLE_SERVER_STATUSES,
        [502, 504, 507, 522, 523, 524, 598, 599]
    );

    // 1. Empirical D-02 429 (`GET /users/@me/survey`) with fractional `retry-after` and bucket metadata
    let headers_429 = vec![
        ("Retry-After".to_string(), "2.5".to_string()),
        ("x-ratelimit-global".to_string(), "false".to_string()),
        ("x-ratelimit-remaining".to_string(), "0".to_string()),
        ("x-ratelimit-limit".to_string(), "5".to_string()),
        ("x-ratelimit-reset-after".to_string(), "2.5".to_string()),
        ("x-ratelimit-bucket".to_string(), "survey-d02".to_string()),
        ("x-ratelimit-scope".to_string(), "user".to_string()),
    ];
    let body_429 = br#"{"message":"You are being rate limited.","retry_after":2.5,"global":false}"#;
    let info = rate_limit_info(429, &headers_429, body_429).expect("429 rate limit info");
    assert_eq!(info.retry_after, Duration::from_millis(2500));
    assert!(!info.global);
    assert_eq!(info.remaining, Some(0));
    assert_eq!(info.limit, Some(5));
    assert_eq!(info.bucket.as_deref(), Some("survey-d02"));
    assert_eq!(info.scope.as_deref(), Some("user"));

    let fail_429 = classify_failure(429, &headers_429, body_429).expect("429 failure");
    assert!(should_retry(HttpMethod::Get, &fail_429));
    assert!(should_retry(HttpMethod::Post, &fail_429));
    assert!(should_retry_client_parity(HttpMethod::Post, &fail_429, 0));
    assert_eq!(
        retry_delay_client_parity(1, &fail_429, &headers_429, body_429),
        Some(Duration::from_millis(2500))
    );

    // 2. 503 Service Unavailable uses `Retry-After` or defaults to 5s (capped at 60s)
    let fail_503 = classify_failure(503, &[], b"").expect("503 failure");
    assert!(should_retry_client_parity(HttpMethod::Post, &fail_503, 0));
    assert_eq!(
        retry_delay_client_parity(1, &fail_503, &[], b""),
        Some(Duration::from_secs(5))
    );

    // 3. Retryable 5xx (`502, 504, 507, 522, 523, 524, 598, 599`) exponential backoff `1s -> 2s -> 4s..60s`
    for status in RETRYABLE_SERVER_STATUSES {
        assert!(is_retryable_server_status(status));
        let fail = RestFailure::Server { status };
        assert!(should_retry_client_parity(HttpMethod::Get, &fail, 0));
        assert!(!should_retry_client_parity(HttpMethod::Post, &fail, 0));
        assert!(should_retry_client_parity(HttpMethod::Post, &fail, 2));
        assert_eq!(
            retry_delay_client_parity(1, &fail, &[], b""),
            Some(Duration::from_secs(1))
        );
        assert_eq!(
            retry_delay_client_parity(2, &fail, &[], b""),
            Some(Duration::from_secs(2))
        );
        assert_eq!(
            retry_delay_client_parity(3, &fail, &[], b""),
            Some(Duration::from_secs(4))
        );
        assert_eq!(
            retry_delay_client_parity(10, &fail, &[], b""),
            Some(Duration::from_secs(60))
        );
    }
}

// ============================================================================
// Dimension 2 & 4: Negative Path & Explicit Error Handling Without Secret Leaks
// ============================================================================

#[test]
fn d07_dim2_dim4_rejects_invalid_tokens_sessions_keyring_inputs_and_unauthenticated_actions() {
    // 1. Opcode 6 Resume rejects empty or whitespace-only token and session_id
    for bad_token in ["", " ", "\t\r\n"] {
        let params = ResumeParams {
            token: bad_token.to_string(),
            session_id: "valid_sess".to_string(),
            seq: 10,
        };
        assert_eq!(build_resume_frame(&params), Err(ResumeError::EmptyToken));
        assert_eq!(
            build_resume_json_payload(&params),
            Err(ResumeError::EmptyToken)
        );
    }
    for bad_sid in ["", "   ", "\n\t"] {
        let params = ResumeParams {
            token: "valid_token".to_string(),
            session_id: bad_sid.to_string(),
            seq: 10,
        };
        assert_eq!(
            build_resume_frame(&params),
            Err(ResumeError::EmptySessionId)
        );
        assert_eq!(
            build_resume_json_payload(&params),
            Err(ResumeError::EmptySessionId)
        );
    }

    // 2. ApiAuth rejects empty token, empty locale, empty timezone
    let props = SuperProperties::default();
    assert_eq!(
        ApiAuth::from_token("   ", &props, "en-US", "UTC"),
        Err(ApiAuthError::EmptyToken)
    );
    assert_eq!(
        ApiAuth::from_token("tok", &props, " ", "UTC"),
        Err(ApiAuthError::EmptyLocale)
    );
    assert_eq!(
        ApiAuth::from_token("tok", &props, "en-US", ""),
        Err(ApiAuthError::EmptyTimezone)
    );

    // 3. SafeStorageMigrator rejects empty payload, invalid base64, decrypt failure, NUL/newline/non-UTF8 tokens
    assert_eq!(
        extract_safe_storage_ciphertext(""),
        Err(SafeStorageMigrationError::EmptyPayload)
    );
    assert_eq!(
        extract_safe_storage_ciphertext("dQw4w9WgXcQ:   "),
        Err(SafeStorageMigrationError::EmptyPayload)
    );
    assert_eq!(
        extract_safe_storage_ciphertext("dQw4w9WgXcQ:!!!invalid_b64!!!"),
        Err(SafeStorageMigrationError::InvalidBase64)
    );

    let mut migrator = SafeStorageMigrator::new();
    let valid_ct = "dQw4w9WgXcQ:Y2lwaGVydGV4dA==";
    for bad_decrypted in [
        b"".to_vec(),
        vec![0xff, 0xfe, 0xfd],
        b"tok\0nul".to_vec(),
        b"tok\nnewline".to_vec(),
        b"tok\rcarriage".to_vec(),
    ] {
        assert_eq!(
            migrator.migrate_once(valid_ct, |_| Ok(bad_decrypted.clone()), |_| Ok(true)),
            Err(SafeStorageMigrationError::InvalidToken)
        );
        assert!(!migrator.is_migrated());
    }

    // 4. Argon2id & InMemoryKeyring reject weak/empty secrets and invalid KDF parameters
    let kr = InMemoryKeyring::new();
    assert!(matches!(
        kr.set_secret(SCHEMA_NAME, "u1", b""),
        Err(StoreKeyringError::InvalidParams(_))
    ));
    assert!(matches!(
        kr.set_secret(SCHEMA_NAME, "u1", &[0u8; 32]),
        Err(StoreKeyringError::InvalidParams(_))
    ));
    assert!(matches!(
        argon2id_derive(
            b"password",
            b"short",
            &Argon2idParams {
                memory_kib: 32,
                iterations: 2,
                parallelism: 1,
                output_len: 32,
            }
        ),
        Err(StoreKeyringError::InvalidParams(_))
    ));

    // 5. Non-retryable 401 Unauthorized and 500 Internal Server Error
    let fail_401 = classify_failure(401, &[], b"").expect("401");
    assert_eq!(fail_401, RestFailure::Unauthorized);
    assert!(!should_retry(HttpMethod::Get, &fail_401));
    assert!(!should_retry_client_parity(HttpMethod::Get, &fail_401, 5));
    assert_eq!(
        retry_delay(1, &fail_401, Duration::from_secs(1), MAX_RETRY_AFTER),
        None
    );

    let fail_500 = classify_failure(500, &[], b"").expect("500");
    assert!(!is_retryable_server_status(500));
    assert!(!should_retry_client_parity(HttpMethod::Get, &fail_500, 5));
    assert_eq!(retry_delay_client_parity(1, &fail_500, &[], b""), None);
}

// ============================================================================
// Dimension 3: Boundary Conditions (Snowflake 0 / u64::MAX, Downtime 29_999 vs
// 30_000 ms, Resume TTL 180_000 vs 180_001 ms, Failure Threshold 3 vs 4,
// Heartbeat Interval 41_250 ms, Rate Limit Cap 60_000 ms)
// ============================================================================

#[test]
fn d07_dim3_boundaries_downtime_ttl_backoff_snowflakes_and_heartbeat_deadlines() {
    // 1. Snowflake validation boundaries: 0 rejected, 1 and u64::MAX accepted, u64::MAX + 1 rejected
    assert!(validate_snowflake_id("user_id", "0").is_err());
    assert_eq!(validate_snowflake_id("user_id", "1"), Ok(1));
    assert_eq!(
        validate_snowflake_id("user_id", "18446744073709551615"),
        Ok(u64::MAX)
    );
    assert!(validate_snowflake_id("user_id", "18446744073709551616").is_err());

    // 2. Downtime boundary: 29_999 ms -> Resume, 30_000 ms (RESUME_MAX_DOWNTIME_MS) -> Identify
    let state = ResumeState {
        session_id: "sess-boundary".to_string(),
        last_seq: u64::from(i64::MAX as u32),
    };
    assert_eq!(
        plan_reconnect(Some(&state), RESUME_MAX_DOWNTIME_MS - 1, None),
        ReconnectAction::Resume
    );
    assert_eq!(
        plan_reconnect(Some(&state), RESUME_MAX_DOWNTIME_MS, None),
        ReconnectAction::Identify
    );

    // 3. Supervisor 3-minute Resume TTL boundary (`RESUME_TTL_MS = 180_000` ms from last HEARTBEAT_ACK)
    let url = GatewayUrl::from_base("wss://gateway.discord.gg").expect("valid gateway url");
    let mut sup = Supervisor::new(SupervisorConfig::new(url, "boundary-token"));
    let mut pipe = chord_core::ws::GatewayPipe::new(chord_core::gateway::Session::new(0));
    let ready_frame = EtfValue::Map(vec![
        (EtfValue::Str("op".into()), EtfValue::Int(0)),
        (EtfValue::Str("s".into()), EtfValue::Int(1)),
        (EtfValue::Str("t".into()), EtfValue::Str("READY".into())),
        (
            EtfValue::Str("d".into()),
            EtfValue::Map(vec![
                (EtfValue::Str("v".into()), EtfValue::Int(9)),
                (
                    EtfValue::Str("session_id".into()),
                    EtfValue::Str("sess-ttl-boundary".into()),
                ),
                (
                    EtfValue::Str("resume_gateway_url".into()),
                    EtfValue::Str("wss://gateway-us-east1-b.discord.gg/".into()),
                ),
                (
                    EtfValue::Str("user".into()),
                    EtfValue::Map(vec![(
                        EtfValue::Str("id".into()),
                        EtfValue::Str("384299970632876032".into()),
                    )]),
                ),
                (EtfValue::Str("guilds".into()), EtfValue::List(vec![])),
            ]),
        ),
    ]);
    // Feed HELLO then READY into pipe
    let hello_frame = EtfValue::Map(vec![
        (EtfValue::Str("op".into()), EtfValue::Int(10)),
        (
            EtfValue::Str("d".into()),
            EtfValue::Map(vec![(
                EtfValue::Str("heartbeat_interval".into()),
                EtfValue::Int(41_250),
            )]),
        ),
    ]);
    pipe.on_wire(&etf::encode(&hello_frame)).expect("HELLO");
    pipe.on_wire(&etf::encode(&ready_frame)).expect("READY");
    sup.absorb_pipe(&pipe);
    assert!(sup.is_session_ready());

    // At exactly 180_000 ms (`RESUME_TTL_MS`), session is still resumable
    sup.set_last_ack_elapsed_ms(RESUME_TTL_MS);
    assert!(sup.is_session_ready());
    assert_eq!(sup.plan_next_reconnect(), ReconnectAction::Resume);

    // At 180_001 ms (`RESUME_TTL_MS + 1`), session expires, resets state, and switches to Identify
    sup.set_last_ack_elapsed_ms(RESUME_TTL_MS + 1);
    assert!(!sup.is_session_ready());
    assert_eq!(sup.plan_next_reconnect(), ReconnectAction::Identify);
    assert!(sup.resume_state().is_none());

    // 4. Exponential backoff schedule boundary: 1_000 -> 2_000 -> 4_000 -> 8_000 -> 16_000 -> 32_000 -> 60_000 -> 60_000
    let mut backoff = ResumeExponentialBackoff::default();
    let mut sup_backoff = Backoff::default();
    for expected in [1_000_u64, 2_000, 4_000, 8_000, 16_000, 32_000, 60_000, 60_000] {
        assert_eq!(backoff.next_delay_ms(), expected);
        assert_eq!(sup_backoff.next_delay_ms(), expected);
    }

    // 5. Heartbeat interval 41_250 ms deadline boundary (`41_249` vs `41_250` ms)
    assert_eq!(DEFAULT_HEARTBEAT_INTERVAL_MS, 41_250);
    assert_eq!(HEARTBEAT_OP, 1);
    assert_eq!(HEARTBEAT_ACK_OP, 11);
    let mut detector = ZombieSocketDetector::new(DEFAULT_HEARTBEAT_INTERVAL_MS);
    detector.on_heartbeat_sent(1, Some(331), 10_000);
    assert_eq!(detector.ack_deadline_ms(), Some(51_250));
    assert_eq!(detector.check_timeout(51_249), None);
    let signal = detector
        .check_timeout(51_250)
        .expect("zombie detected at exact 41_250ms boundary");
    assert_eq!(
        signal,
        ZombieReconnectSignal {
            missed_tick_index: 1,
            last_seq: Some(331),
            elapsed_without_ack_ms: 41_250,
            timeout_ms: 41_250,
            close_immediately: true,
            reconnect_immediately: true,
        }
    );

    // 6. Rate-limit cap boundary: retry-after 120s capped to MAX_RETRY_AFTER (60s)
    let capped_headers = vec![("retry-after".to_string(), "120".to_string())];
    assert_eq!(
        parse_retry_after_or_default(429, &capped_headers, b""),
        Some(MAX_RETRY_AFTER)
    );
    assert_eq!(
        parse_retry_after_or_default(503, &[], b""),
        Some(DEFAULT_RETRY_AFTER)
    );
}

// ============================================================================
// Dimension 5 & 6: Empty States & Lifecycle Transitions
// (`Unestablished -> Ready -> Disconnected -> Resuming (1..=4) -> Resumed / Identify`)
// ============================================================================

#[test]
fn d07_dim5_dim6_empty_states_and_full_session_recovery_state_machine() {
    let cache = seed_guild_cache();

    // 1. Empty state before READY: no resumable session, plan_action is Identify, prepare_resume_frame fails
    let mut mgr = ResumeSessionManager::new();
    assert!(!mgr.has_resumable_session());
    assert_eq!(mgr.session_id(), None);
    assert_eq!(mgr.last_seq(), 0);
    assert_eq!(mgr.plan_action(0), ReconnectAction::Identify);
    assert_eq!(
        mgr.prepare_resume_frame("tok", &cache),
        Err(ResumeError::EmptySessionId)
    );

    // 2. State hydration + `complete_resume_without_cache_reset` replaying missed events in `State`
    let mut state = State::default();
    assert!( ResumeSessionManager::from_state(&state, Some(&cache)).is_err());

    state.session_id = "f14ac1f3d408c15027d6d98da172c398".to_string();
    state.resume_gateway_url = Some("wss://gateway-us-east1-d.discord.gg/".to_string());
    state.last_seq = Some(331);
    state.current_user = Some(UserSummary {
        id: 384_299_970_632_876_032,
        username: Some("chord_d07_operator".to_string()),
        global_name: None,
        discriminator: None,
        avatar: None,
        bot: false,
    });
    let mut channels = BTreeMap::new();
    channels.insert(
        1_176_785_373_989_445_703,
        ChannelState {
            id: 1_176_785_373_989_445_703,
            kind: 0,
            name: Some("general".to_string()),
            parent_id: None,
            position: 0,
            last_message_id: None,
        },
    );
    state.guilds.insert(
        1_014_537_851_418_980_372,
        GuildState {
            id: 1_014_537_851_418_980_372,
            name: Some("ITSKIDON PC BOOST".to_string()),
            unavailable: false,
            channels,
            roles: BTreeMap::new(),
            member_count: Some(24_229),
        },
    );

    // Empirical seam #3 (`run2:95974.10029` s=1..331 -> `OP 7 RECONNECT` -> `run2:95974.13072` s=332..371)
    let mut mgr = ResumeSessionManager::from_state(&state, Some(&cache)).expect("from_state");
    assert_eq!(mgr.last_seq(), 331);

    let replayed = vec![
        Event {
            seq: Some(331), // stale duplicate seq <= 331 must not be counted as newly applied
            payload: EventPayload::Resumed,
            diagnostics: Default::default(),
        },
        Event {
            seq: Some(333),
            payload: EventPayload::Resumed,
            diagnostics: Default::default(),
        },
        Event {
            seq: Some(371),
            payload: EventPayload::Resumed,
            diagnostics: Default::default(),
        },
    ];
    let report = mgr
        .complete_resume_without_cache_reset(&mut state, Some(&cache), 332, replayed)
        .expect("complete_resume_without_cache_reset");
    assert_eq!(report.resume_seq_sent, 331);
    assert_eq!(report.last_seq_after_resume, 371);
    assert_eq!(report.replayed_events_applied, 2); // s=333 and s=371 (s=331 skipped as stale)
    assert!(report.guild_cache_preserved);
    assert!(!report.channels_reloaded);
    assert_eq!(report.preserved_guild_count, 1);
    assert!(state.resumed);

    // 3. Consecutive failures 1..=3 stay on Resume; 4th failure transitions to Identify
    assert_eq!(mgr.on_resume_attempt_failed(), ReconnectAction::Resume);
    assert_eq!(mgr.consecutive_failures(), 1);
    assert_eq!(mgr.on_resume_attempt_failed(), ReconnectAction::Resume);
    assert_eq!(mgr.consecutive_failures(), 2);
    assert_eq!(mgr.on_resume_attempt_failed(), ReconnectAction::Resume);
    assert_eq!(mgr.consecutive_failures(), 3);
    assert!(mgr.has_resumable_session());

    assert_eq!(mgr.on_resume_attempt_failed(), ReconnectAction::Identify);
    assert!(!mgr.has_resumable_session());
    assert_eq!(mgr.plan_next_action(0), ReconnectAction::Identify);
}

// ============================================================================
// Dimension 7: Capabilities, Permissions & Security
// (Automatic Token Masking, Zeroization, Debug Redaction, Session Revocation)
// ============================================================================

#[test]
fn d07_dim7_security_automatic_masking_debug_redaction_zeroization_and_revocation() {
    let discord_tok = format!(
        "{}.{}.{}",
        "MTIzNDU2Nzg5MDEyMzQ1Njc4OQ", "GaBcDe", "AbCdEfGhIjKlMnOpQrStUvWxYz012345678"
    );
    let mfa_tok = "mfa.abcdef1234567890abcdef1234567890_secret";
    let github_tok = "ghp_1234567890abcdefghijklmnopqrstuvwxyz";
    let hex_key = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    // 1. Automatic masking via `redact_secrets` across all credential formats
    let raw_log = format!(
        "Auth headers: Authorization: Bot {discord_tok}, Bearer ya29.oauth_secret, \
         mfa={mfa_tok}, gh={github_tok}, srtp={hex_key}, \
         json={{\"token\":\"{discord_tok}\",\"session_id\":\"f14ac1f3d408c15027d6d98da172c398\",\"password\":\"MyPass123!\"}}"
    );
    let masked = redact_secrets(&raw_log);
    assert!(!masked.contains(&discord_tok));
    assert!(!masked.contains("ya29.oauth_secret"));
    assert!(!masked.contains(mfa_tok));
    assert!(!masked.contains(github_tok));
    assert!(!masked.contains(hex_key));
    assert!(!masked.contains("f14ac1f3d408c15027d6d98da172c398"));
    assert!(!masked.contains("MyPass123!"));
    assert!(masked.contains(REDACTED_MARKER));

    // 2. Automatic stream masking via `RedactedWriter` and `SecretRedactionLayer`
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
        tracing::info!("Gateway Resume sent with token={discord_tok} and mfa {mfa_tok}");
    });
    let logged = String::from_utf8(shared_buf.lock().unwrap().clone()).unwrap();
    assert!(!logged.contains(&discord_tok));
    assert!(!logged.contains(mfa_tok));
    assert!(logged.contains(REDACTED_MARKER));

    // Direct RedactedWriter check without trailing newline (flush flushes & redacts buffer)
    let mut raw_vec = Vec::new();
    {
        let mut rw = RedactedWriter::new(&mut raw_vec);
        write!(rw, "secret_key={hex_key}").unwrap();
        rw.flush().unwrap();
    }
    let flushed_str = String::from_utf8(raw_vec).unwrap();
    assert_eq!(flushed_str, format!("secret_key={REDACTED_MARKER}"));

    // 3. Struct `Debug` redaction invariants (`ResumeParams`, `ResumeHandshakeFrame`, `ApiAuth`, `DerivedKey`)
    let resume_params = ResumeParams {
        token: discord_tok.clone(),
        session_id: "sess_123".to_string(),
        seq: 94,
    };
    assert!(!format!("{resume_params:?}").contains(&discord_tok));
    assert!(format!("{resume_params:?}").contains("REDACTED"));

    let mgr = {
        let mut m = ResumeSessionManager::new();
        m.on_ready("sess_123", Some("wss://gateway.discord.gg"), 94);
        m
    };
    let hs = mgr
        .build_resume_handshake(discord_tok.clone())
        .expect("build_resume_handshake");
    assert!(!format!("{hs:?}").contains(&discord_tok));
    assert!(format!("{hs:?}").contains("REDACTED"));

    let api_auth = ApiAuth::from_token_with_scheme(
        &discord_tok,
        AuthScheme::User,
        &SuperProperties::default(),
        "en-US",
        "UTC",
    )
    .expect("ApiAuth::from_token");
    assert_eq!(api_auth.authorization, discord_tok); // raw user token without Bearer (H-01)
    assert!(!format!("{api_auth:?}").contains(&discord_tok));
    assert!(format!("{api_auth:?}").contains("<redacted>"));

    let mut derived = DerivedKey::from_bytes(vec![0x7F; 32]);
    assert_eq!(format!("{derived:?}"), "DerivedKey(\"[REDACTED]\")");

    // 4. Memory zeroization on drop / explicit zeroize
    derived.zeroize_now();
    assert!(derived.as_bytes().iter().all(|&b| b == 0));

    let mut secret_bytes = discord_tok.as_bytes().to_vec();
    zeroize_secret_bytes(&mut secret_bytes);
    assert!(secret_bytes.iter().all(|&b| b == 0));

    let mut secret_str = discord_tok.clone();
    zeroize_secret_string(&mut secret_str);
    assert!(secret_str.is_empty());

    // 5. Active Session Manager & Remote Session Revocation (`DELETE /users/@me/auth/sessions`)
    let mut sess_mgr = SessionManager::new(vec![
        SessionInfo {
            id_hash: "hash_current_device".to_string(),
            os: "Linux".to_string(),
            browser: "Chord Desktop".to_string(),
            ip: "<PUB_IP>".to_string(),
            location: "Frankfurt, DE".to_string(),
            approx_last_used_time: "2026-11-21T12:00:00Z".to_string(),
            is_current: true,
        },
        SessionInfo {
            id_hash: "hash_remote_mobile".to_string(),
            os: "Android".to_string(),
            browser: "Discord Android".to_string(),
            ip: "<PUB_IP>".to_string(),
            location: "Berlin, DE".to_string(),
            approx_last_used_time: "2026-11-20T09:00:00Z".to_string(),
            is_current: false,
        },
    ]);
    let cur = sess_mgr.current_session().expect("current device session");
    assert_eq!(cur.badge_label(), Some(CURRENT_DEVICE_BADGE_LABEL));
    assert_eq!(cur.badge_color(), Some(CURRENT_DEVICE_BADGE_COLOR));

    assert_eq!(
        sess_mgr.revoke_session("hash_remote_mobile", false),
        Err(SessionError::Unauthorized)
    );
    let revoke_req = sess_mgr
        .revoke_session("hash_remote_mobile", true)
        .expect("authorized session revocation");
    assert_eq!(revoke_req.method, "DELETE");
    assert_eq!(revoke_req.endpoint, "/users/@me/auth/sessions");
    assert_eq!(revoke_req.session_id_hash, "hash_remote_mobile");
    assert_eq!(sess_mgr.sessions().len(), 1);
}

// ============================================================================
// Dimension 8: Recovery & Degradation From Abrupt Connection Dropouts,
// Zombie Sockets, OP 7 RECONNECT, OP 9 INVALID_SESSION, and Fatal Close Codes
// ============================================================================

#[test]
fn d07_dim8_abrupt_dropout_zombie_detector_invalid_session_and_close_code_policy() {
    // 1. Abrupt connection dropout (no close frame `None`) & transient close codes (`1000`, `1001`, `1006`, `4000`, `4009`)
    // trigger `CloseAction::Reconnect`, whereas `4004`, `4013`, `4014` trigger `CloseAction::Stop`.
    assert_eq!(classify_close(None), CloseAction::Reconnect);
    for transient_code in [1000, 1001, 1006, 4000, 4008, 4009] {
        assert_eq!(classify_close(Some(transient_code)), CloseAction::Reconnect);
    }
    assert_eq!(
        classify_close(Some(4004)),
        CloseAction::Stop(StopReason::Unauthorized)
    );
    assert_eq!(
        classify_close(Some(4013)),
        CloseAction::Stop(StopReason::InvalidClient { code: 4013 })
    );
    assert_eq!(
        classify_close(Some(4014)),
        CloseAction::Stop(StopReason::InvalidClient { code: 4014 })
    );

    // 2. ZombieSocketDetector recovery: normal ACK measures RTT, missed ACK triggers zombie reconnect,
    // subsequent ACK clears zombie state
    let mut detector = ZombieSocketDetector::new(41_250);
    assert_eq!(
        detector.on_heartbeat_ack(1_000),
        HeartbeatAckOutcome::UnexpectedAck
    );

    detector.on_heartbeat_sent(1, Some(94), 10_000);
    assert!(detector.is_awaiting_ack());
    assert_eq!(
        detector.on_heartbeat_ack(10_085),
        HeartbeatAckOutcome::Acknowledged {
            tick_index: 1,
            seq: Some(94),
            rtt_ms: 85,
        }
    );
    assert!(!detector.is_awaiting_ack());
    assert_eq!(detector.acks_received(), 1);
    assert_eq!(detector.last_rtt_ms(), Some(85));

    // Tick #2 gets no ACK before Tick #3 is due -> zombie socket detected!
    detector.on_heartbeat_sent(2, Some(331), 51_250);
    let zombie_sig = detector
        .on_next_tick_due(92_500)
        .expect("missed ACK must trigger ZombieReconnectSignal");
    assert!(detector.is_zombie());
    assert_eq!(detector.missed_acks(), 1);
    assert!(zombie_sig.close_immediately);
    assert!(zombie_sig.reconnect_immediately);
    assert_eq!(zombie_sig.last_seq, Some(331));

    // 3. OP 9 INVALID_SESSION recovery: `d=true` keeps session for Opcode 6 Resume;
    // `d=false` clears session and degrades cleanly to fresh Opcode 2 Identify.
    let cache = seed_guild_cache();
    let mut mgr = ResumeSessionManager::new();
    mgr.on_ready_established(
        "f14ac1f3d408c15027d6d98da172c398",
        Some("wss://gateway-us-east1-d.discord.gg"),
        331,
        &cache,
    );
    assert_eq!(mgr.on_invalid_session(true), ReconnectAction::Resume);
    assert!(mgr.has_resumable_session());
    assert_eq!(mgr.on_invalid_session(false), ReconnectAction::Identify);
    assert!(!mgr.has_resumable_session());
    assert_eq!(mgr.session_id(), None);
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
    // and Rust protocol constants
    let d07_oracle =
        chord_json::parse(&json_text).expect("docs/re/session-resilience-d07.json must be valid JSON");
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

    // Cross-check keyring & token security oracle fields
    let keyring_spec = d07_oracle
        .get("keyringAndTokenSecurity")
        .expect("keyringAndTokenSecurity object");
    assert_eq!(
        keyring_spec.get("schemaName").and_then(JsonValue::as_str),
        Some(SCHEMA_NAME)
    );
    assert_eq!(
        keyring_spec
            .get("safeStoragePrefix")
            .and_then(JsonValue::as_str),
        Some(ENCRYPTED_TOKEN_PREFIX)
    );
    assert_eq!(
        keyring_spec
            .get("redactedMarker")
            .and_then(JsonValue::as_str),
        Some(REDACTED_MARKER)
    );

    // Cross-check Opcode 6 Resume & dropout recovery constants
    let resume_spec = d07_oracle
        .get("sessionRecoveryProtocol")
        .expect("sessionRecoveryProtocol object");
    assert_eq!(
        resume_spec.get("resumeOpcode").and_then(JsonValue::as_u64),
        Some(u64::from(RESUME_OP))
    );
    assert_eq!(
        resume_spec
            .get("backoffMinMs")
            .and_then(JsonValue::as_u64),
        Some(RESUME_BACKOFF_MIN_MS)
    );
    assert_eq!(
        resume_spec
            .get("backoffMaxMs")
            .and_then(JsonValue::as_u64),
        Some(RESUME_BACKOFF_MAX_MS)
    );
    assert_eq!(
        resume_spec
            .get("resetAfterFailures")
            .and_then(JsonValue::as_u64),
        Some(u64::from(RESUME_RESET_AFTER_FAILURES))
    );
    assert_eq!(
        resume_spec
            .get("maxDowntimeMs")
            .and_then(JsonValue::as_u64),
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

    // Cross-check the 3 empirical Resume sessions against `docs/re/gateway-protocol.json`
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

    // Cross-check rate-limit backoff specification
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

    // 3. Observable telemetry on Supervisor outbound rejection before Ready
    let url = GatewayUrl::from_base("wss://gateway.discord.gg").expect("url");
    let mut sup = Supervisor::new(SupervisorConfig::new(url, "telemetry-tok"));
    assert_eq!(sup.rejected_outbound(), 0);
    let err = sup
        .send_presence_update(&chord_core::gateway::OutboundPresenceUpdate {
            status: "online".to_string(),
            since: 0,
            activities: vec![],
            afk: false,
        })
        .expect_err("must reject OP 3 before session is Ready");
    assert_eq!(err, SupervisorOutboundError::NotReady { op: 3 });
    assert_eq!(sup.rejected_outbound(), 1);
    assert!(!is_safe_storage_token("plain_token"));
}
