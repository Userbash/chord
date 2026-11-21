#![forbid(unsafe_code)]

//! Contract tests for `STAGE-05.3.2`:
//! Механизм Resume (Opcode 6) по `session_id` и `seq` с экспоненциальным backoff (1s..60s):
//! Восстановление сессии без сброса кэша гильдий и повторной загрузки каналов.
//!
//! Verifies:
//! 1. `RESUME_OP == 6`, `RESUME_BACKOFF_MIN_MS == 1_000`, `RESUME_BACKOFF_MAX_MS == 60_000`.
//! 2. `ResumeExponentialBackoff` deterministic exponential schedule:
//!    `1_000 -> 2_000 -> 4_000 -> 8_000 -> 16_000 -> 32_000 -> 60_000 -> 60_000` ms,
//!    with `reset()` on `RESUMED` / `READY`.
//! 3. Opcode 6 payload generator (`build_resume_frame` / `ResumeSessionManager::prepare_resume_frame`)
//!    with `{op: 6, d: {token, session_id, seq}}`, strict token redaction (`REDACTED`), and
//!    non-empty / non-whitespace validation for `token` and `session_id`.
//! 4. Session recovery without resetting guild cache (`InMemoryCache` / `State`) or reloading
//!    channels (`preserve_guild_cache == true`, `skip_channel_reload == true`), and seamless
//!    application of incremental dispatches (`seq + 1..`) onto preserved guilds and channels.
//! 5. Fallback to `Identify` (Opcode 2) after `RESUME_RESET_AFTER_FAILURES` (4 consecutive
//!    failed Resume attempts) or `op9 INVALID_SESSION (d=false)` or TTL expiration (`> 180_000 ms`).

use chord_core::cache::{InMemoryCache, InMemoryCacheBuilder, ResourceType};
use chord_core::codec::etf::{self, EtfValue};
use chord_core::resume::{
    RESUME_BACKOFF_MAX_MS, RESUME_BACKOFF_MIN_MS, RESUME_MAX_DOWNTIME_MS, RESUME_OP,
    RESUME_RESET_AFTER_FAILURES, ReconnectAction, ResumeError, ResumeExponentialBackoff,
    ResumeHandshakeFrame, ResumeParams, ResumeRecoveryOutcome, ResumeSessionManager,
    build_resume_frame, plan_reconnect,
};
use twilight_model::id::Id;
use twilight_model::id::marker::{ChannelMarker, GuildMarker};

fn seed_populated_cache() -> InMemoryCache {
    let cache = InMemoryCacheBuilder::new()
        .resource_types(ResourceType::ALL)
        .build();

    let ready_json = serde_json::json!({
        "v": 9,
        "user": {
            "id": "384299970632876032",
            "username": "chord_operator",
            "discriminator": "0001",
            "avatar": "a_hash",
            "bot": false
        },
        "guilds": [
            {
                "id": "1014537851418980372",
                "name": "Chord Core Engineering",
                "icon": "guild_icon_1",
                "channels": [
                    {
                        "id": "1176785373989445703",
                        "guild_id": "1014537851418980372",
                        "name": "gateway-protocol",
                        "type": 0,
                        "position": 1,
                        "topic": "Opcode 6 Resume & ETF wire contracts"
                    },
                    {
                        "id": "1176785373989445704",
                        "guild_id": "1014537851418980372",
                        "name": "voice-dave-mls",
                        "type": 2,
                        "position": 2
                    }
                ],
                "roles": [
                    {
                        "id": "1014537851418980375",
                        "name": "Maintainer",
                        "color": 5793266,
                        "position": 5,
                        "permissions": "8"
                    }
                ]
            },
            {
                "id": "1014537851418980999",
                "name": "Chord QA & Benchmarks",
                "icon": "guild_icon_2",
                "channels": [
                    {
                        "id": "1176785373989445888",
                        "guild_id": "1014537851418980999",
                        "name": "ci-gates",
                        "type": 0,
                        "position": 0
                    }
                ]
            }
        ],
        "private_channels": [
            {
                "id": "1200000000000000001",
                "type": 1,
                "name": "direct-peer"
            }
        ],
        "session_id": "f14ac1f3d408c15027d6d98da172c398",
        "resume_gateway_url": "wss://gateway-us-east1-b.discord.gg"
    });

    cache
        .update_from_ready_value(&ready_json)
        .expect("cache hydration from READY must succeed");
    cache
}

#[test]
fn test_resume_opcode6_constants_and_exponential_backoff_1s_to_60s() {
    assert_eq!(RESUME_OP, 6);
    assert_eq!(RESUME_BACKOFF_MIN_MS, 1_000);
    assert_eq!(RESUME_BACKOFF_MAX_MS, 60_000);
    assert_eq!(RESUME_MAX_DOWNTIME_MS, 30_000);
    assert_eq!(RESUME_RESET_AFTER_FAILURES, 4);

    let mut backoff = ResumeExponentialBackoff::default();
    assert_eq!(backoff.base_ms(), 1_000);
    assert_eq!(backoff.max_ms(), 60_000);
    assert_eq!(backoff.attempt(), 0);
    assert_eq!(backoff.current_ms(), 1_000);

    // Exact exponential progression 1s -> 2s -> 4s -> 8s -> 16s -> 32s -> 60s (capped)
    let expected_schedule = [1_000, 2_000, 4_000, 8_000, 16_000, 32_000, 60_000, 60_000];
    for (idx, &expected_ms) in expected_schedule.iter().enumerate() {
        assert_eq!(
            backoff.delay_for_attempt(idx as u32),
            expected_ms,
            "delay_for_attempt({idx}) must be {expected_ms}ms"
        );
        let actual_ms = backoff.next_delay_ms();
        assert_eq!(
            actual_ms,
            expected_ms,
            "step {} must yield {}ms backoff",
            idx + 1,
            expected_ms
        );
        assert_eq!(backoff.attempt(), (idx + 1) as u32);
    }

    // Reset after RESUMED restores initial 1s delay and 0 attempts
    backoff.reset();
    assert_eq!(backoff.attempt(), 0);
    assert_eq!(backoff.current_ms(), 1_000);
    assert_eq!(backoff.next_delay_ms(), 1_000);
}

#[test]
fn test_resume_opcode6_frame_encoding_and_validation() {
    let session_id = "f14ac1f3d408c15027d6d98da172c398";
    let token = "mock_dummy_secret_auth_token_for_redaction_test";
    let seq = 663_u64;

    let params = ResumeParams {
        token: token.to_string(),
        session_id: session_id.to_string(),
        seq,
    };

    // Debug output must never leak token
    let debug_text = format!("{params:?}");
    assert!(!debug_text.contains(token));
    assert!(debug_text.contains("REDACTED"));
    assert!(debug_text.contains(session_id));
    assert!(debug_text.contains("663"));

    let wire_bytes = build_resume_frame(&params).expect("build_resume_frame must succeed");
    assert_eq!(wire_bytes.first(), Some(&etf::VERSION_BYTE));

    let (decoded, consumed) = etf::decode(&wire_bytes).expect("ETF decode must succeed");
    assert_eq!(consumed, wire_bytes.len());
    assert_eq!(
        decoded.get("op").and_then(EtfValue::as_int),
        Some(i64::from(RESUME_OP))
    );

    let d = decoded.get("d").expect("d map in Opcode 6 frame");
    assert_eq!(d.get("token").and_then(EtfValue::as_str), Some(token));
    assert_eq!(
        d.get("session_id").and_then(EtfValue::as_str),
        Some(session_id)
    );
    assert_eq!(d.get("seq").and_then(EtfValue::as_int), Some(663));

    // Reject empty or whitespace-only token and session_id
    for bad_token in ["", "   ", "\t\n"] {
        let bad = ResumeParams {
            token: bad_token.to_string(),
            session_id: session_id.to_string(),
            seq,
        };
        assert_eq!(build_resume_frame(&bad), Err(ResumeError::EmptyToken));
    }

    for bad_sid in ["", "   ", "\r\n"] {
        let bad = ResumeParams {
            token: token.to_string(),
            session_id: bad_sid.to_string(),
            seq,
        };
        assert_eq!(build_resume_frame(&bad), Err(ResumeError::EmptySessionId));
    }
}

#[test]
fn test_resume_session_manager_preserves_guild_cache_and_skips_channel_reload() {
    let cache = seed_populated_cache();
    let guild_1 = Id::<GuildMarker>::new(1_014_537_851_418_980_372);
    let guild_2 = Id::<GuildMarker>::new(1_014_537_851_418_980_999);
    let chan_1 = Id::<ChannelMarker>::new(1_176_785_373_989_445_703);
    let chan_2 = Id::<ChannelMarker>::new(1_176_785_373_989_445_704);

    assert_eq!(cache.guilds().len(), 2);
    assert_eq!(cache.channels().len(), 4); // 3 guild channels + 1 private DM channel

    // Snapshot arc pointers before disconnect to prove zero cache reset / zero re-allocation
    let g1_arc_before = cache.guild(guild_1).expect("guild 1 cached");
    let g2_arc_before = cache.guild(guild_2).expect("guild 2 cached");
    let c1_arc_before = cache.channel(chan_1).expect("channel 1 cached");
    let c2_arc_before = cache.channel(chan_2).expect("channel 2 cached");

    let mut manager = ResumeSessionManager::new();
    manager.on_ready_established(
        "f14ac1f3d408c15027d6d98da172c398",
        Some("wss://gateway-us-east1-b.discord.gg"),
        94,
        &cache,
    );

    assert!(manager.has_resumable_session());
    assert_eq!(
        manager.session_id(),
        Some("f14ac1f3d408c15027d6d98da172c398")
    );
    assert_eq!(manager.last_seq(), 94);
    assert_eq!(manager.cached_guild_count(), 2);
    assert_eq!(manager.cached_channel_count(), 4);

    // Advance sequence numbers as dispatches arrive before disconnect (95..663)
    for seq in 95..=663 {
        assert!(manager.record_dispatch_seq(seq));
    }
    assert_eq!(manager.last_seq(), 663);
    // Stale/duplicate sequence numbers must not roll back last_seq
    assert!(!manager.record_dispatch_seq(663));
    assert!(!manager.record_dispatch_seq(500));
    assert_eq!(manager.last_seq(), 663);

    // Abrupt disconnect occurs! Prepare Opcode 6 Resume attempt #1
    let handshake: ResumeHandshakeFrame = manager
        .prepare_resume_frame("secret_session_token_abc", &cache)
        .expect("prepare_resume_frame must succeed");

    assert_eq!(handshake.op, RESUME_OP);
    assert_eq!(handshake.session_id, "f14ac1f3d408c15027d6d98da172c398");
    assert_eq!(handshake.seq, 663);
    assert_eq!(handshake.backoff_delay_ms, 1_000);
    assert_eq!(handshake.attempt, 1);
    assert!(
        handshake.preserve_guild_cache,
        "Resume must preserve guild cache without clearing"
    );
    assert!(
        handshake.skip_channel_reload,
        "Resume must skip channel reloading"
    );

    // Debug redaction on ResumeHandshakeFrame
    let hs_debug = format!("{handshake:?}");
    assert!(!hs_debug.contains("secret_session_token_abc"));
    assert!(hs_debug.contains("REDACTED"));

    // Simulate gateway replaying missed events (664..667) and emitting RESUMED at s=668
    for replayed_seq in 664..=667 {
        assert!(manager.record_dispatch_seq(replayed_seq));
    }
    let outcome: ResumeRecoveryOutcome = manager.on_resumed_received(668, &cache);
    assert_eq!(
        outcome,
        ResumeRecoveryOutcome {
            session_id: "f14ac1f3d408c15027d6d98da172c398".to_string(),
            resumed_at_seq: 668,
            replayed_events: 5, // 664, 665, 666, 667, 668
            preserved_guilds: 2,
            preserved_channels: 4,
            guild_cache_reset: false,
            channels_reloaded: false,
        }
    );

    // Backoff is reset to 1_000ms and failure count is 0
    assert_eq!(manager.backoff().current_ms(), 1_000);
    assert_eq!(manager.backoff().attempt(), 0);
    assert_eq!(manager.consecutive_failures(), 0);
    assert_eq!(manager.total_resumed_count(), 1);

    // Verify exact pointer identity in InMemoryCache: guilds and channels were NOT cleared or reloaded
    let g1_arc_after = cache.guild(guild_1).expect("guild 1 still cached");
    let g2_arc_after = cache.guild(guild_2).expect("guild 2 still cached");
    let c1_arc_after = cache.channel(chan_1).expect("channel 1 still cached");
    let c2_arc_after = cache.channel(chan_2).expect("channel 2 still cached");

    assert!(
        std::sync::Arc::ptr_eq(&g1_arc_before, &g1_arc_after),
        "Guild 1 Arc pointer must remain identical across Resume (zero cache reset)"
    );
    assert!(
        std::sync::Arc::ptr_eq(&g2_arc_before, &g2_arc_after),
        "Guild 2 Arc pointer must remain identical across Resume (zero cache reset)"
    );
    assert!(
        std::sync::Arc::ptr_eq(&c1_arc_before, &c1_arc_after),
        "Channel 1 Arc pointer must remain identical across Resume (no channel reload)"
    );
    assert!(
        std::sync::Arc::ptr_eq(&c2_arc_before, &c2_arc_after),
        "Channel 2 Arc pointer must remain identical across Resume (no channel reload)"
    );
}

#[test]
fn test_resume_session_manager_exponential_backoff_and_fallback_to_identify() {
    let cache = seed_populated_cache();
    let mut manager = ResumeSessionManager::new();
    manager.on_ready_established(
        "f14ac1f3d408c15027d6d98da172c398",
        Some("wss://gateway.discord.gg"),
        331,
        &cache,
    );

    // 4 consecutive failed Resume attempts follow exponential backoff: 1s, 2s, 4s, 8s
    let expected_delays = [1_000_u64, 2_000, 4_000, 8_000];
    for (i, &expected_delay) in expected_delays.iter().enumerate() {
        assert_eq!(manager.plan_action(0), ReconnectAction::Resume);
        let frame = manager
            .prepare_resume_frame("token_xyz", &cache)
            .expect("resume frame");
        assert_eq!(frame.backoff_delay_ms, expected_delay);
        assert_eq!(frame.attempt, (i + 1) as u32);
        let next_action = manager.on_resume_attempt_failed();
        if i + 1 < RESUME_RESET_AFTER_FAILURES as usize {
            assert_eq!(next_action, ReconnectAction::Resume);
            assert!(manager.has_resumable_session());
        } else {
            // 4th failure resets session -> 5th attempt must be Identify (Opcode 2)
            assert_eq!(next_action, ReconnectAction::Identify);
            assert!(!manager.has_resumable_session());
        }
    }

    assert_eq!(manager.plan_action(0), ReconnectAction::Identify);
    assert_eq!(
        manager.prepare_resume_frame("token_xyz", &cache),
        Err(ResumeError::EmptySessionId)
    );
}

#[test]
fn test_resume_session_manager_handles_op9_invalid_session_and_downtime_policy() {
    let cache = seed_populated_cache();
    let mut manager = ResumeSessionManager::new();
    manager.on_ready_established(
        "f14ac1f3d408c15027d6d98da172c398",
        Some("wss://gateway.discord.gg"),
        120,
        &cache,
    );

    // op9 INVALID_SESSION with d=true allows Resume
    assert_eq!(
        manager.on_invalid_session(true),
        ReconnectAction::Resume,
        "op9 d=true must keep session resumable"
    );
    assert!(manager.has_resumable_session());
    assert_eq!(manager.plan_action(3_000), ReconnectAction::Resume);

    // Downtime >= 30_000ms forces Identify in plan_reconnect
    assert_eq!(
        plan_reconnect(manager.resume_state(), 30_000, Some(true)),
        ReconnectAction::Identify
    );

    // op9 INVALID_SESSION with d=false invalidates session and requires fresh Identify
    assert_eq!(
        manager.on_invalid_session(false),
        ReconnectAction::Identify,
        "op9 d=false must transition to Identify"
    );
    assert!(!manager.has_resumable_session());
    assert_eq!(manager.plan_action(0), ReconnectAction::Identify);
}
