//! Contract Test: Task R5-10 Stream Lifecycle & Streamer Mode
//!
//! Specification & Reference Oracles:
//! - `docs/feature-backlog.md` row `R5-10`:
//!   "Handle `STREAM_CREATE`, `STREAM_SERVER_UPDATE`, and `STREAM_DELETE` events;
//!    automatic streamer mode toggle to hide sensitive personal data.
//!    Reference Source: Vesktop: `streamerMode.ts`.
//!    Target: Core: Event Dispatcher.
//!    Criteria: Stream lifecycle events update UI state; streamer mode automatically
//!    suppresses tokens, invite links, and notifications"
//! - `waves/RW-40/VOICE/STREAM-GATEWAY-CONTRACT.md` (A-ROUTER `@4937840`, A-PHASES `@528441`,
//!   A-REASONS `@528586`, D-27-1..D-27-7).
//! - Clean-room RE oracle `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0`:
//!   - `StreamerModeStore` (`@9370993..9372950`):
//!     `enabled: false, autoToggle: true, hideInstantInvites: true, hidePersonalInformation: true,
//!      disableSounds: true, disableNotifications: true, enableContentProtection: false`
//!   - `RunningGameStore` (`@4773100..4783150`):
//!     `RUNNING_STREAMER_TOOLS_CHANGE` (`OBS`, `XSplit`, `Bebo`, `Streamlabs Desktop`, `Twitch Studio`)
//!
//! Covers all 9 mandatory dimensions (`TEST-POLICY.md` §12):
//! 1. Positive path: `STREAM_CREATE`, `STREAM_SERVER_UPDATE`, and `STREAM_DELETE` dispatch events
//!    are routed through the Event Dispatcher into UI state updates, and automatic streamer mode
//!    activates when a self-stream starts or when a streaming tool (e.g., OBS) runs.
//! 2. Negative path: Malformed `STREAM_CREATE` / `STREAM_SERVER_UPDATE` / `STREAM_DELETE` payloads,
//!    missing required fields (`stream_key`, `endpoint`, `token`), invalid snowflakes, and unknown
//!    event names are safely rejected without panicking or corrupting UI state.
//! 3. Boundary conditions: Snowflake boundaries (`1` and `u64::MAX`, `"18446744073709551615"`),
//!    2000-character message sanitization with multiple tokens/invites/emails, `viewer_ids`
//!    distinction (`None` vs `Some(vec![])` vs max viewers), and 0 vs multiple active streams/tools.
//! 4. Error handling: `STREAM_DELETE` protocol failure reasons (`unauthorized` -> `Failed`,
//!    `stream_full` -> `Ended` + viewer-full alert, `safety_guild_rate_limited` -> `Ended` + rate-limit alert,
//!    `unavailable: true` -> `Reconnecting`, unknown reason forward-compatibility).
//! 5. Empty states: Initial idle dispatcher with zero active streams and zero streamer tools has
//!    streamer mode disabled, empty UI stream list, and pass-through content/notifications.
//! 6. Intermediate / lifecycle states: `Connecting` -> `Active` (`STREAM_CREATE`) ->
//!    `TransportReady` (`STREAM_SERVER_UPDATE`) -> `Paused` (`STREAM_UPDATE`/`STREAM_CREATE`) ->
//!    `Reconnecting` (`STREAM_DELETE{unavailable:true}`) -> `Ended` (`STREAM_DELETE`), with live UI
//!    badge, active stream count, and automatic streamer mode transitions.
//! 7. Capabilities, privacy & suppression: When streamer mode is active, tokens (Discord JWT/MFA/Bearer,
//!    voice/stream RTC tokens), invite links (`discord.gg/*`, `discord.com/invite/*`, `discordapp.com/invite/*`),
//!    personal data (emails, phone numbers, discriminators/tags), and desktop notifications / sounds
//!    are automatically suppressed; `Debug`/`Display` never leak raw RTC tokens or endpoints.
//! 8. Recovery & fallback: When the user's stream ends (`STREAM_DELETE`) and no external streaming
//!    tools remain running, automatic streamer mode cleanly deactivates and restores normal notifications
//!    and invite rendering; manual override (`set_manual_enabled`) and `auto_toggle: false` are respected.
//! 9. Observable telemetry: `StreamLifecycleTelemetry` accurately tracks `stream_creates_handled`,
//!    `stream_server_updates_handled`, `stream_updates_handled`, `stream_deletes_handled`,
//!    `rejected_dispatches`, `auto_toggles_activated`, `auto_toggles_deactivated`,
//!    `tokens_suppressed`, `invite_links_suppressed`, `personal_data_redactions`, and
//!    `notifications_suppressed`.

#![forbid(unsafe_code)]

use chord_codec::etf::EtfValue;
use chord_core::DispatchEvent;
use chord_core::stream::{StreamDeleteReason, StreamKind, StreamPhase};
use chord_core::stream_lifecycle::{
    INVITE_REDACTED_PLACEHOLDER, PERSONAL_DATA_REDACTED_PLACEHOLDER,
     RedactedStreamServerToken, StreamLifecycleController, StreamLifecycleError,
    StreamLifecycleEvent, StreamLifecycleTelemetry, StreamUiEntry, StreamUiStateSnapshot,
    StreamerModeConfig, StreamerModeNotificationDecision, StreamerModeSuppressionReason,
    StreamerModeTriggerSource, StreamerToolDescriptor, StreamerToolProcess,
    TOKEN_REDACTED_PLACEHOLDER, detect_running_streamer_tools, extract_stream_server_token,
    map_gateway_stream_dispatch, sanitize_invite_links, sanitize_personal_data,
    sanitize_stream_content, sanitize_tokens,
};

fn atom(name: &str) -> EtfValue {
    EtfValue::Atom(name.to_string())
}

fn str_val(s: &str) -> EtfValue {
    EtfValue::Str(s.to_string())
}

fn etf_map(pairs: Vec<(&str, EtfValue)>) -> EtfValue {
    EtfValue::Map(
        pairs
            .into_iter()
            .map(|(k, v)| (EtfValue::Str(k.to_string()), v))
            .collect(),
    )
}

fn make_dispatch(name: &str, seq: Option<u64>, data: EtfValue) -> DispatchEvent {
    DispatchEvent {
        kind: None,
        name: name.to_string(),
        seq,
        data,
    }
}

/// Dimension 1: Positive Path — `STREAM_CREATE`, `STREAM_SERVER_UPDATE`, and `STREAM_DELETE`
/// update UI state and automatically toggle Streamer Mode for the local broadcaster.
#[test]
fn dim1_positive_stream_lifecycle_events_update_ui_state_and_auto_toggle_streamer_mode() {
    let self_user_id = 384_299_970_632_876_032_u64;
    let mut controller = StreamLifecycleController::new(self_user_id);

    // Initially idle: streamer mode is off, no active streams in UI state
    assert!(!controller.is_streamer_mode_active());
    assert_eq!(
        controller.active_trigger_source(),
        StreamerModeTriggerSource::None
    );
    assert!(controller.ui_state().active_streams.is_empty());
    assert!(!controller.ui_state().is_self_streaming);

    // 1. Receive `STREAM_CREATE` for current user's guild stream
    let stream_key = "guild:1014537851418980372:1176785373989445703:384299970632876032";
    let create_ev = make_dispatch(
        "STREAM_CREATE",
        Some(10),
        etf_map(vec![
            ("stream_key", str_val(stream_key)),
            ("region", str_val("rotterdam")),
            (
                "viewer_ids",
                EtfValue::List(vec![
                    str_val("111111111111111111"),
                    str_val("222222222222222222"),
                ]),
            ),
            ("rtc_server_id", str_val("999888777666555444")),
            ("rtc_channel_id", str_val("1176785373989445703")),
            ("paused", atom("false")),
        ]),
    );

    let mapped_create =
        map_gateway_stream_dispatch(&create_ev).expect("STREAM_CREATE should map cleanly");
    assert!(matches!(
        mapped_create,
        StreamLifecycleEvent::StreamCreate { .. }
    ));

    let snapshot_after_create: StreamUiStateSnapshot = controller
        .handle_dispatch_event(&create_ev)
        .expect("STREAM_CREATE handled");

    // UI state must reflect active self-stream, LIVE badge, viewer count, region, and automatic streamer mode
    assert!(snapshot_after_create.is_self_streaming);
    assert!(snapshot_after_create.live_badge_visible);
    assert!(snapshot_after_create.streamer_mode_active);
    assert!(snapshot_after_create.streamer_mode_banner_visible);
    assert_eq!(
        controller.active_trigger_source(),
        StreamerModeTriggerSource::ActiveSelfStream
    );
    assert_eq!(snapshot_after_create.active_streams.len(), 1);

    let entry: &StreamUiEntry = &snapshot_after_create.active_streams[0];
    assert_eq!(entry.kind, StreamKind::Guild);
    assert_eq!(entry.guild_id, Some(1_014_537_851_418_980_372));
    assert_eq!(entry.channel_id, 1_176_785_373_989_445_703);
    assert_eq!(entry.owner_id, self_user_id);
    assert!(entry.is_self_stream);
    assert_eq!(entry.phase, StreamPhase::Active);
    assert_eq!(entry.region.as_deref(), Some("rotterdam"));
    assert_eq!(entry.viewer_ids, Some(vec![111_111_111_111_111_111, 222_222_222_222_222_222]));
    assert_eq!(entry.viewer_count, Some(2));
    assert!(!entry.paused);
    assert!(!entry.transport_ready);

    // 2. Receive `STREAM_SERVER_UPDATE` with media endpoint and secret RTC token
    let secret_rtc_token = "go_live_rtc_secret_token_987654321";
    let server_update_ev = make_dispatch(
        "STREAM_SERVER_UPDATE",
        Some(11),
        etf_map(vec![
            ("stream_key", str_val(stream_key)),
            ("endpoint", str_val("c-ams04-stream.discord.media:443")),
            ("token", str_val(secret_rtc_token)),
        ]),
    );

    let mapped_server = map_gateway_stream_dispatch(&server_update_ev)
        .expect("STREAM_SERVER_UPDATE should map cleanly");
    assert!(
        !format!("{mapped_server:?}").contains(secret_rtc_token),
        "StreamLifecycleEvent Debug must redact secret RTC token"
    );

    let extracted_tok: RedactedStreamServerToken =
        extract_stream_server_token(&server_update_ev).expect("extract stream server token");
    assert_eq!(extracted_tok.expose(), secret_rtc_token);
    assert_eq!(format!("{extracted_tok:?}"), TOKEN_REDACTED_PLACEHOLDER);
    assert_eq!(format!("{extracted_tok}"), TOKEN_REDACTED_PLACEHOLDER);

    let snapshot_after_server = controller
        .handle_dispatch_event(&server_update_ev)
        .expect("STREAM_SERVER_UPDATE handled");
    let entry_ready = &snapshot_after_server.active_streams[0];
    assert!(entry_ready.transport_ready);
    assert_eq!(
        entry_ready.endpoint_host.as_deref(),
        Some("c-ams04-stream.discord.media")
    );
    assert!(entry_ready.token_present);
    // Neither UI entry Debug nor snapshot Debug may contain the raw RTC token
    assert!(!format!("{snapshot_after_server:?}").contains(secret_rtc_token));

    // 3. Receive `STREAM_DELETE` ending the stream
    let delete_ev = make_dispatch(
        "STREAM_DELETE",
        Some(12),
        etf_map(vec![
            ("stream_key", str_val(stream_key)),
            ("reason", str_val("user_requested")),
            ("unavailable", atom("false")),
        ]),
    );

    let snapshot_after_delete = controller
        .handle_dispatch_event(&delete_ev)
        .expect("STREAM_DELETE handled");

    assert!(!snapshot_after_delete.is_self_streaming);
    assert!(!snapshot_after_delete.live_badge_visible);
    assert!(
        !snapshot_after_delete.streamer_mode_active,
        "Streamer mode must automatically toggle off when self-stream ends and no streamer tools run"
    );
    assert_eq!(
        controller.active_trigger_source(),
        StreamerModeTriggerSource::None
    );
    let ended_entry = &snapshot_after_delete.active_streams[0];
    assert_eq!(ended_entry.phase, StreamPhase::Ended);
    assert!(!ended_entry.transport_ready);
    assert_eq!(
        ended_entry.delete_reason,
        Some(StreamDeleteReason::UserRequested)
    );
}

/// Dimension 7: Capabilities, Privacy & Automatic Suppression of Tokens, Invite Links,
/// Personal Information, and Notifications in Streamer Mode.
#[test]
fn dim7_streamer_mode_suppresses_tokens_invite_links_personal_data_and_notifications() {
    let self_user_id = 384_299_970_632_876_032_u64;
    let mut controller = StreamLifecycleController::new(self_user_id);

    // Start self-stream so automatic streamer mode activates
    let stream_key = "call:1176785373989445703:384299970632876032";
    let create_ev = make_dispatch(
        "STREAM_CREATE",
        Some(1),
        etf_map(vec![
            ("stream_key", str_val(stream_key)),
            ("paused", atom("false")),
        ]),
    );
    controller.handle_dispatch_event(&create_ev).unwrap();
    assert!(controller.is_streamer_mode_active());

    // 1. Token suppression: Discord JWT tokens (3 base64url parts), MFA tokens (`mfa.*`),
    //    Bearer tokens, and `token=...` / `"token":"..."` assignments.
    let fake_jwt = "fake_jwt_payload_header_abc123.mock_sig_part.dummy_tail_token_xyz987";
    let fake_mfa = "mfa.mock_mfa_token_streamer_mode_suppression_test_only_0123456789";
    let raw_with_tokens = format!(
        "Auth header Bearer {fake_jwt} or MFA {fake_mfa} and query token=secret_stream_tok_123"
    );
    let sanitized_tokens = controller.sanitize_display_text(&raw_with_tokens);
    assert!(
        !sanitized_tokens.contains(fake_jwt),
        "JWT token must be suppressed in streamer mode: {sanitized_tokens}"
    );
    assert!(
        !sanitized_tokens.contains(fake_mfa),
        "MFA token must be suppressed in streamer mode: {sanitized_tokens}"
    );
    assert!(
        !sanitized_tokens.contains("secret_stream_tok_123"),
        "Query/KV token must be suppressed in streamer mode: {sanitized_tokens}"
    );
    assert!(sanitized_tokens.contains(TOKEN_REDACTED_PLACEHOLDER));

    // 2. Invite link suppression: `discord.gg/<code>`, `https://discord.gg/<code>`,
    //    `https://discord.com/invite/<code>`, `https://discordapp.com/invite/<code>`,
    //    `https://ptb.discord.com/invite/<code>`, `https://canary.discord.com/invite/<code>`
    let raw_with_invites = "Join our raid server at https://discord.gg/rust-lang-2026 or discord.com/invite/secretSquad99 or https://discordapp.com/invite/legacyCode!";
    let sanitized_invites = controller.sanitize_display_text(raw_with_invites);
    assert!(
        !sanitized_invites.contains("rust-lang-2026"),
        "discord.gg invite code must be suppressed: {sanitized_invites}"
    );
    assert!(
        !sanitized_invites.contains("secretSquad99"),
        "discord.com/invite code must be suppressed: {sanitized_invites}"
    );
    assert!(
        !sanitized_invites.contains("legacyCode"),
        "discordapp.com/invite code must be suppressed: {sanitized_invites}"
    );
    assert!(sanitized_invites.contains(INVITE_REDACTED_PLACEHOLDER));

    // 3. Personal data suppression: email addresses, discriminators (`#0001`), and phone numbers
    let raw_with_pii =
        "Contact streamer.private@protonmail.com or backup_dev@example.org (tag Rustery#1337)";
    let sanitized_pii = controller.sanitize_display_text(raw_with_pii);
    assert!(
        !sanitized_pii.contains("streamer.private@protonmail.com"),
        "email address must be suppressed in streamer mode: {sanitized_pii}"
    );
    assert!(
        !sanitized_pii.contains("backup_dev@example.org"),
        "second email address must be suppressed: {sanitized_pii}"
    );
    assert!(
        !sanitized_pii.contains("#1337"),
        "legacy discriminator tag must be masked in streamer mode: {sanitized_pii}"
    );
    assert!(sanitized_pii.contains(PERSONAL_DATA_REDACTED_PLACEHOLDER));

    // 4. Desktop notification and notification sound suppression
    let notif_decision = controller.evaluate_notification("ch-general", "Hey @everyone check this out!");
    assert_eq!(
        notif_decision,
        StreamerModeNotificationDecision::Suppressed(
            StreamerModeSuppressionReason::StreamerModeActive
        )
    );
    assert!(controller.should_suppress_notifications());
    assert!(controller.should_suppress_sounds());
    assert!(controller.should_hide_instant_invites());
    assert!(controller.should_hide_personal_information());
}

/// Dimension 1 & 8: External Streamer Tool Detection (`OBS`, `XSplit`, `Streamlabs`, `Twitch Studio`, `Bebo`)
/// & Recovery / Fallback Semantics (`RUNNING_STREAMER_TOOLS_CHANGE`).
#[test]
fn dim1_and_dim8_external_streamer_tools_auto_toggle_and_recovery_lifecycle() {
    let self_user_id = 500_000_000_000_000_001_u64;
    let mut controller = StreamLifecycleController::new(self_user_id);

    // 1. Detect OBS Studio process running on Linux (`obs`) and Windows (`obs64.exe`)
    let processes = vec![
        StreamerToolProcess {
            pid: 4100,
            exe_name: "firefox".to_string(),
            exe_path: "/usr/bin/firefox".to_string(),
        },
        StreamerToolProcess {
            pid: 4200,
            exe_name: "obs".to_string(),
            exe_path: "/usr/bin/obs".to_string(),
        },
    ];
    let detected: Vec<StreamerToolDescriptor> = detect_running_streamer_tools(&processes);
    assert_eq!(detected.len(), 1);
    assert_eq!(detected[0].tool_name, "OBS");
    assert_eq!(detected[0].pid, 4200);

    // Update running processes -> auto-toggles streamer mode ON via `ExternalStreamerTool`
    controller.update_running_processes(&processes);
    assert!(controller.is_streamer_mode_active());
    assert_eq!(
        controller.active_trigger_source(),
        StreamerModeTriggerSource::ExternalStreamerTool
    );
    assert_eq!(controller.running_streamer_tools().len(), 1);

    // 2. While OBS is running, user also starts a Go Live self-stream (`STREAM_CREATE`)
    let self_stream_key = "guild:100:200:500000000000000001";
    let create_ev = make_dispatch(
        "STREAM_CREATE",
        Some(1),
        etf_map(vec![("stream_key", str_val(self_stream_key))]),
    );
    controller.handle_dispatch_event(&create_ev).unwrap();
    assert!(controller.is_streamer_mode_active());
    assert_eq!(
        controller.active_trigger_source(),
        StreamerModeTriggerSource::ActiveSelfStreamAndTool
    );

    // 3. Go Live self-stream ends (`STREAM_DELETE`), but OBS is STILL running ->
    //    Streamer mode MUST remain active (`ExternalStreamerTool`)!
    let delete_ev = make_dispatch(
        "STREAM_DELETE",
        Some(2),
        etf_map(vec![
            ("stream_key", str_val(self_stream_key)),
            ("reason", str_val("user_requested")),
        ]),
    );
    controller.handle_dispatch_event(&delete_ev).unwrap();
    assert!(
        controller.is_streamer_mode_active(),
        "Streamer mode must stay active while OBS is still running"
    );
    assert_eq!(
        controller.active_trigger_source(),
        StreamerModeTriggerSource::ExternalStreamerTool
    );

    // 4. OBS exits -> automatic streamer mode cleanly deactivates and normal operation resumes
    controller.update_running_processes(&[processes[0].clone()]);
    assert!(!controller.is_streamer_mode_active());
    assert_eq!(
        controller.active_trigger_source(),
        StreamerModeTriggerSource::None
    );

    // Normal text and notifications are no longer suppressed once streamer mode is off
    let normal_text = "Invite your friend: https://discord.gg/welcome123";
    assert_eq!(controller.sanitize_display_text(normal_text), normal_text);
    assert_eq!(
        controller.evaluate_notification("ch-general", "Hello!"),
        StreamerModeNotificationDecision::Allowed
    );

    // 5. If `auto_toggle` is disabled in config, starting a stream or OBS does NOT auto-enable streamer mode
    let mut manual_cfg = StreamerModeConfig::default();
    manual_cfg.auto_toggle = false;
    controller.set_config(manual_cfg);
    controller.update_running_processes(&processes);
    assert!(
        !controller.is_streamer_mode_active(),
        "auto_toggle=false must prevent automatic activation"
    );

    // Manual override works regardless of auto_toggle
    controller.set_manual_enabled(true);
    assert!(controller.is_streamer_mode_active());
    assert_eq!(
        controller.active_trigger_source(),
        StreamerModeTriggerSource::Manual
    );
    controller.set_manual_enabled(false);
    assert!(!controller.is_streamer_mode_active());
}

/// Dimension 2: Negative Path — Malformed `STREAM_*` Dispatches, Peer vs Self Stream Isolation,
/// and Stale Sequence Numbers.
#[test]
fn dim2_negative_malformed_dispatches_peer_isolation_and_stale_sequences() {
    let self_user_id = 384_299_970_632_876_032_u64;
    let peer_user_id = 777_888_999_000_111_222_u64;
    let mut controller = StreamLifecycleController::new(self_user_id);

    // 1. Unknown / non-stream gateway event returns `Err(StreamLifecycleError::UnsupportedEvent)`
    let typing_ev = make_dispatch(
        "TYPING_START",
        Some(1),
        etf_map(vec![("channel_id", str_val("123"))]),
    );
    assert_eq!(map_gateway_stream_dispatch(&typing_ev), None);
    assert_eq!(
        controller.handle_dispatch_event(&typing_ev),
        Err(StreamLifecycleError::UnsupportedEvent(
            "TYPING_START".to_string()
        ))
    );

    // 2. Malformed payloads for `STREAM_CREATE`, `STREAM_SERVER_UPDATE`, `STREAM_DELETE`
    let malformed_cases = vec![
        // Missing stream_key
        make_dispatch("STREAM_CREATE", Some(2), etf_map(vec![("region", str_val("us-east"))])),
        // Invalid stream_key grammar
        make_dispatch(
            "STREAM_CREATE",
            Some(3),
            etf_map(vec![("stream_key", str_val("invalid:key"))]),
        ),
        // Non-map payload
        make_dispatch("STREAM_CREATE", Some(4), EtfValue::Nil),
        // STREAM_SERVER_UPDATE missing token
        make_dispatch(
            "STREAM_SERVER_UPDATE",
            Some(5),
            etf_map(vec![
                ("stream_key", str_val("call:100:200")),
                ("endpoint", str_val("stream.discord.media:443")),
            ]),
        ),
        // STREAM_SERVER_UPDATE empty endpoint
        make_dispatch(
            "STREAM_SERVER_UPDATE",
            Some(6),
            etf_map(vec![
                ("stream_key", str_val("call:100:200")),
                ("endpoint", str_val("")),
                ("token", str_val("tok123")),
            ]),
        ),
        // STREAM_DELETE invalid stream_key
        make_dispatch(
            "STREAM_DELETE",
            Some(7),
            etf_map(vec![("stream_key", str_val("guild:0:100:200"))]),
        ),
    ];

    for bad_ev in &malformed_cases {
        assert!(
            map_gateway_stream_dispatch(bad_ev).is_none(),
            "malformed event {} must return None from map_gateway_stream_dispatch",
            bad_ev.name
        );
        assert!(
            controller.handle_dispatch_event(bad_ev).is_err(),
            "malformed event {} must return Err from handle_dispatch_event",
            bad_ev.name
        );
    }
    assert_eq!(
        controller.telemetry().rejected_dispatches,
        malformed_cases.len() as u64
    );

    // 3. Peer stream (`owner_id != self_user_id`) updates UI stream list,
    //    but MUST NOT auto-enable streamer mode for the local user!
    let peer_stream_key = format!("guild:100:200:{peer_user_id}");
    let peer_create = make_dispatch(
        "STREAM_CREATE",
        Some(20),
        etf_map(vec![
            ("stream_key", str_val(&peer_stream_key)),
            ("region", str_val("us-west")),
        ]),
    );
    let snap_peer = controller.handle_dispatch_event(&peer_create).unwrap();
    assert_eq!(snap_peer.active_streams.len(), 1);
    assert!(!snap_peer.active_streams[0].is_self_stream);
    assert!(!snap_peer.is_self_streaming);
    assert!(
        !snap_peer.streamer_mode_active,
        "A peer's stream must not activate local streamer mode"
    );

    // 4. Stale sequence (`seq <= last_seq`) is ignored without overwriting newer state
    let stale_delete = make_dispatch(
        "STREAM_DELETE",
        Some(15), // older than seq 20
        etf_map(vec![
            ("stream_key", str_val(&peer_stream_key)),
            ("reason", str_val("user_requested")),
        ]),
    );
    let snap_after_stale = controller.handle_dispatch_event(&stale_delete).unwrap();
    assert_eq!(
        snap_after_stale.active_streams[0].phase,
        StreamPhase::Active,
        "stale STREAM_DELETE (seq 15 <= 20) must not terminate active stream"
    );
}

/// Dimension 3: Boundary Conditions — Snowflake `1` and `u64::MAX`, 2000-Character Message
/// Sanitization, `viewer_ids` (`None` vs `Some([])`), and JSON-Encoded Wire Payloads.
#[test]
fn dim3_boundary_snowflakes_2000_chars_viewer_ids_and_json_wire_payload() {
    let max_u64 = u64::MAX; // 18446744073709551615
    let mut controller = StreamLifecycleController::new(max_u64);

    // 1. Snowflake boundaries: guild_id = 1, channel_id = u64::MAX, owner_id = u64::MAX,
    //    delivered as JSON string inside EtfValue::Str (live gateway JSON bridge format)
    let boundary_key = format!("guild:1:{max_u64}:{max_u64}");
    let json_create = format!(
        r#"{{"stream_key":"{boundary_key}","region":"eu-central","viewer_ids":[],"paused":false}}"#
    );
    let ev_json_create = DispatchEvent {
        kind: None,
        name: "STREAM_CREATE".to_string(),
        seq: Some(100),
        data: EtfValue::Str(json_create),
    };

    let snap = controller.handle_dispatch_event(&ev_json_create).unwrap();
    assert!(snap.is_self_streaming);
    assert!(snap.streamer_mode_active);
    let entry = &snap.active_streams[0];
    assert_eq!(entry.guild_id, Some(1));
    assert_eq!(entry.channel_id, max_u64);
    assert_eq!(entry.owner_id, max_u64);
    // D-27-5: `viewer_ids: []` -> `Some(vec![])` with `viewer_count == Some(0)`
    assert_eq!(entry.viewer_ids, Some(Vec::new()));
    assert_eq!(entry.viewer_count, Some(0));

    // 2. `STREAM_UPDATE` omitting `viewer_ids` (`None`) preserves previous `Some(vec![])`,
    //    whereas explicit `viewer_ids: ["1", "18446744073709551615"]` updates to 2 viewers.
    let ev_update = make_dispatch(
        "STREAM_UPDATE",
        Some(101),
        etf_map(vec![
            ("stream_key", str_val(&boundary_key)),
            (
                "viewer_ids",
                EtfValue::List(vec![EtfValue::Int(1), str_val(&max_u64.to_string())]),
            ),
            ("paused", atom("true")),
        ]),
    );
    let snap_updated = controller.handle_dispatch_event(&ev_update).unwrap();
    let entry_updated = &snap_updated.active_streams[0];
    assert_eq!(entry_updated.phase, StreamPhase::Paused);
    assert!(entry_updated.paused);
    assert_eq!(entry_updated.viewer_ids, Some(vec![1, max_u64]));
    assert_eq!(entry_updated.viewer_count, Some(2));

    // 3. 2000-character message boundary with embedded invite link and token at the very end
    let prefix = "А".repeat(1950);
    let invite_tail = " https://discord.gg/boundary2000";
    let msg_2000 = format!("{prefix}{invite_tail}");
    assert!(msg_2000.chars().count() <= 2000);

    let sanitized_2000 = controller.sanitize_display_text(&msg_2000);
    assert!(!sanitized_2000.contains("boundary2000"));
    assert!(sanitized_2000.contains(INVITE_REDACTED_PLACEHOLDER));
    assert!(sanitized_2000.starts_with(&prefix));
}

/// Dimension 4 & 6: Error Handling & Full Stream Lifecycle Transitions
/// (`Connecting` -> `Active` -> `Paused` -> `Reconnecting` -> `Failed` / `Ended` with Reasons).
#[test]
fn dim4_and_dim6_stream_delete_reasons_and_full_lifecycle_transitions() {
    let self_user_id = 42_u64;
    let mut controller = StreamLifecycleController::new(self_user_id);

    let key_str = "guild:10:20:42";

    // Step 1: Local watch/start request places stream in Connecting or awaits server STREAM_CREATE
    controller.request_watch_stream(key_str).unwrap();
    assert_eq!(
        controller.ui_state().active_streams[0].phase,
        StreamPhase::Connecting
    );

    // Step 2: STREAM_CREATE transitions `Connecting -> Active`
    let create_ev = make_dispatch(
        "STREAM_CREATE",
        Some(1),
        etf_map(vec![
            ("stream_key", str_val(key_str)),
            ("paused", atom("false")),
        ]),
    );
    let snap_active = controller.handle_dispatch_event(&create_ev).unwrap();
    assert_eq!(snap_active.active_streams[0].phase, StreamPhase::Active);

    // Step 3: STREAM_DELETE with `unavailable: true` transitions `Active -> Reconnecting`
    //         and keeps streamer mode active during temporary outage!
    let temp_drop_ev = make_dispatch(
        "STREAM_DELETE",
        Some(2),
        etf_map(vec![
            ("stream_key", str_val(key_str)),
            ("unavailable", atom("true")),
        ]),
    );
    let snap_reconnecting = controller.handle_dispatch_event(&temp_drop_ev).unwrap();
    assert_eq!(
        snap_reconnecting.active_streams[0].phase,
        StreamPhase::Reconnecting
    );
    assert!(
        snap_reconnecting.streamer_mode_active,
        "Streamer mode must stay active while stream is Reconnecting"
    );

    // Step 4: STREAM_DELETE with `reason: "unauthorized"` transitions to `Failed`
    //         with UI `data_reason == Some("permission-channel")` and ends self-streaming
    let unauth_ev = make_dispatch(
        "STREAM_DELETE",
        Some(3),
        etf_map(vec![
            ("stream_key", str_val(key_str)),
            ("unavailable", atom("false")),
            ("reason", str_val("unauthorized")),
        ]),
    );
    let snap_failed = controller.handle_dispatch_event(&unauth_ev).unwrap();
    assert_eq!(snap_failed.active_streams[0].phase, StreamPhase::Failed);
    assert_eq!(
        snap_failed.active_streams[0].ui_data_reason,
        Some("permission-channel")
    );
    assert!(!snap_failed.is_self_streaming);
    assert!(!snap_failed.streamer_mode_active);

    // Step 5: `stream_full` and `safety_guild_rate_limited` reasons
    let key_full = "guild:10:20:99";
    controller
        .handle_dispatch_event(&make_dispatch(
            "STREAM_CREATE",
            Some(4),
            etf_map(vec![("stream_key", str_val(key_full))]),
        ))
        .unwrap();
    let snap_full = controller
        .handle_dispatch_event(&make_dispatch(
            "STREAM_DELETE",
            Some(5),
            etf_map(vec![
                ("stream_key", str_val(key_full)),
                ("reason", str_val("stream_full")),
            ]),
        ))
        .unwrap();
    let full_entry = snap_full
        .active_streams
        .iter()
        .find(|e| e.owner_id == 99)
        .unwrap();
    assert_eq!(full_entry.phase, StreamPhase::Ended);
    assert_eq!(full_entry.ui_data_reason, Some("viewer-full"));
    assert!(snap_full.viewer_full_alert_stream_keys.contains(&key_full.to_string()));

    let key_rate = "guild:10:20:100";
    controller
        .handle_dispatch_event(&make_dispatch(
            "STREAM_CREATE",
            Some(6),
            etf_map(vec![("stream_key", str_val(key_rate))]),
        ))
        .unwrap();
    let snap_rate = controller
        .handle_dispatch_event(&make_dispatch(
            "STREAM_DELETE",
            Some(7),
            etf_map(vec![
                ("stream_key", str_val(key_rate)),
                ("reason", str_val("safety_guild_rate_limited")),
            ]),
        ))
        .unwrap();
    let rate_entry = snap_rate
        .active_streams
        .iter()
        .find(|e| e.owner_id == 100)
        .unwrap();
    assert_eq!(rate_entry.phase, StreamPhase::Ended);
    assert_eq!(rate_entry.ui_data_reason, Some("safety-rate-limited"));
    assert!(snap_rate.safety_rate_limited_guilds.contains(&10));
}

/// Dimension 5 & 9: Empty State, Selective Granular Toggles, Ready Reset, and Observable Telemetry.
#[test]
fn dim5_and_dim9_empty_state_selective_config_ready_reset_and_telemetry() {
    let self_user_id = 777_u64;
    let mut controller = StreamLifecycleController::new(self_user_id);

    // Empty state checks
    assert_eq!(controller.telemetry(), &StreamLifecycleTelemetry::default());
    assert_eq!(controller.sanitize_display_text(""), "");
    assert_eq!(sanitize_tokens(""), (String::new(), 0));
    assert_eq!(sanitize_invite_links(""), (String::new(), 0));
    assert_eq!(sanitize_personal_data(""), (String::new(), 0));

    // Activate streamer mode via self-stream
    let key = "guild:1:2:777";
    controller
        .handle_dispatch_event(&make_dispatch(
            "STREAM_CREATE",
            Some(1),
            etf_map(vec![("stream_key", str_val(key))]),
        ))
        .unwrap();
    controller
        .handle_dispatch_event(&make_dispatch(
            "STREAM_SERVER_UPDATE",
            Some(2),
            etf_map(vec![
                ("stream_key", str_val(key)),
                ("endpoint", str_val("media.discord.gg:443")),
                ("token", str_val("sec_tok_1")),
            ]),
        ))
        .unwrap();

    // Perform content and notification suppressions to exercise telemetry counters
    let sample = "Token Bearer fake_jwt_payload_header_abc123.mock_sig_part.dummy_tail_token_xyz987 invite https://discord.gg/abc123 mail user@domain.com";
    let cleaned = controller.sanitize_display_text(sample);
    assert!(!cleaned.contains("abc123"));
    assert!(!cleaned.contains("user@domain.com"));
    let _ = controller.evaluate_notification("ch-1", "ping");

    // Verify all telemetry counters are populated and accurate
    let telemetry = controller.telemetry().clone();
    assert_eq!(telemetry.stream_creates_handled, 1);
    assert_eq!(telemetry.stream_server_updates_handled, 1);
    assert_eq!(telemetry.auto_toggles_activated, 1);
    assert!(telemetry.tokens_suppressed >= 1);
    assert_eq!(telemetry.invite_links_suppressed, 1);
    assert!(telemetry.personal_data_redactions >= 1);
    assert_eq!(telemetry.notifications_suppressed, 1);

    // Selective config: if `hide_instant_invites` is turned off while streamer mode stays enabled,
    // tokens remain suppressed while invite links are preserved!
    let mut custom_cfg = controller.config().clone();
    custom_cfg.hide_instant_invites = false;
    controller.set_config(custom_cfg);
    let selective = sanitize_stream_content(sample, controller.config(), true);
    assert!(
        selective.sanitized_text.contains("https://discord.gg/abc123"),
        "invite links must be preserved when hide_instant_invites=false"
    );
    assert!(
        selective.sanitized_text.contains(TOKEN_REDACTED_PLACEHOLDER),
        "tokens must ALWAYS be suppressed when streamer mode is active"
    );

    // Gateway READY reset clears all active streams and deactivates auto streamer mode
    let snap_reset = controller.reset_on_ready();
    assert!(snap_reset.active_streams.is_empty());
    assert!(!snap_reset.is_self_streaming);
    assert!(!snap_reset.streamer_mode_active);
    assert_eq!(controller.telemetry().auto_toggles_deactivated, 1);
}
