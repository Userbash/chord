//! Wave 4 Contract Tests (`W4-3`):
//! Mention/DM Priority D-Bus Notification Routing (`P1-4.3`) &
//! `SET_ACTIVITY` RPC -> `AppState` / `UserProfileCard` UI (`P2-2.3`).
//!
//! Covers all 9 mandatory test dimensions (`TEST-POLICY.md` §12):
//! 1. Positive:
//!    - `P1-4.3`: DM `MESSAGE_CREATE` and Guild `MESSAGE_CREATE` mentioning `current_user`
//!      (`<@id>`, `<@!id>`, `<@&role_id>`, `@everyone`, `@here`) call `prepare_message_notification`
//!      and emit `NotificationPriorityIntent::HighPriorityMentionOrDm` (`Urgency::Critical` for DM,
//!      `Urgency::Normal` for guild mentions), while unmuted `AllMessages` guild messages without
//!      mentions emit `NotificationPriorityIntent::Normal` (`Urgency::Low`).
//!    - `P2-2.3`: Incoming `SetActivityPayload` / `Option<RpcActivity>` updates `AppState`,
//!      maintains custom Rich Presence fields (`name`, `details`, `state`, `timestamps`, `assets`,
//!      `party`, `buttons`), enqueues Gateway Op 3 `PRESENCE_UPDATE` via
//!      `RpcActivity::to_gateway_presence_update_json()`, and renders the live Rich Presence block
//!      in `UserProfileCard`, including clearing presence when `activity == None`.
//! 2. Negative:
//!    - Self-authored messages, DND status, focused active channel, muted channels (normal &
//!      `@everyone`/`@here`), `NotificationLevel::Nothing`, non-matching mentions in `OnlyMentions`
//!      channels, and duplicate message IDs emit `NotificationPriorityIntent::Suppressed` without
//!      spamming `pending_notifications`.
//! 3. Edge Boundaries:
//!    - Snowflake boundaries `"0"` and `"18446744073709551615"` (`u64::MAX`), prefix non-collision
//!      (`<@123>` vs `<@1234>`), 2000-char UTF-8 truncation to 256 chars, `MAX_RPC_TEXT_LEN` (128),
//!      `MAX_RPC_BUTTONS` (2), legacy seconds-to-millis timestamp normalization, party `[1, 1]`..`[9999, 9999]`,
//!      and all activity types `0..=5`.
//! 4. Error Handling:
//!    - Invalid `SET_ACTIVITY` JSON, `pid == 0`, >2 buttons, non-HTTPS button URLs, `end < start`
//!      timestamps, and control chars return `Err(RpcIpcError)`, increment error telemetry, and
//!      preserve prior presence without emitting corrupt Gateway frames.
//! 5. Empty States:
//!    - Whitespace-only message without attachments is suppressed; empty content with attachment
//!      emits `"[Вложение]"`. Clean `AppState` and cleared `activity == None` yield `None` in
//!      `UserProfileCard::rich_presence_block`.
//! 6. Loading / Transition States:
//!    - Multi-step `None -> Activity A -> Activity B -> Error -> None` presence lifecycle.
//! 7. Capability / Permission Controls:
//!    - Per-channel `NotificationLevel`, `suppress_everyone_mentions`, and `muted_channels` controls.
//! 8. Recovery / Security Redaction:
//!    - Secret tokens (`mfa.*`, 3-part JWT-like tokens, `Bearer *`) are redacted in notification bodies;
//!      `logout()` and `switch_account()` clear active RPC presence and pending notifications.
//! 9. Observable Telemetry & Boundedness:
//!    - `NotificationTelemetry` and `RpcActivityTelemetry` track exact counts while bounding queued
//!      notifications and recent intent history under flood conditions.

#![forbid(unsafe_code)]
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

use std::sync::mpsc;

use chord_platform::notifications::{NotificationLevel, Urgency};
use chord_platform::rpc_ipc::{
    RpcActivity, RpcAssets, RpcButton, RpcIpcError, RpcParty, RpcTimestamps, SetActivityPayload,
};
use dispatch::BackgroundEvent;
use state::{
    AppState, GuildMember, NotificationPriorityIntent, NotificationSuppressionReason,
    PresenceStatus,
};
use ui::user_profile_card::UserProfileCard;

fn setup_guild_state() -> AppState {
    let mut state = AppState::new();
    state.current_user.id = "384299970632876032".into();
    state.current_user.username = "pharos12510".into();
    state.current_user.status = PresenceStatus::Online;
    state.select_guild(Some("guild-1".into()));
    state.select_channel("ch-general".into());
    state.set_app_focused(true);
    state
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. P1-4.3 Positive: DM & Mention Detection -> HighPriorityMentionOrDm vs Normal
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn p1_4_3_dm_and_mentions_emit_high_priority_while_all_messages_emits_normal() {
    let mut state = setup_guild_state();
    state.set_current_user_roles(vec!["role-rust-core".into(), "987654321098765432".into()]);

    // 1. Incoming DM in unselected DM channel -> HighPriorityMentionOrDm with Urgency::Critical
    state.apply_background_event(BackgroundEvent::GatewayMessageCreate {
        id: "msg-dm-100".into(),
        channel_id: "dm-u-demo-1".into(),
        guild_id: None,
        author_id: "u-demo-1".into(),
        author_name: "DemoUser".into(),
        content: "Привет в ЛС! Проверь PR.".into(),
        timestamp: Some("2026-11-14T15:00:00Z".into()),
    });

    assert_eq!(state.notification_telemetry.total_evaluated, 1);
    assert_eq!(state.notification_telemetry.high_priority_emitted, 1);
    assert_eq!(state.notification_telemetry.normal_emitted, 0);
    assert_eq!(state.pending_notifications.len(), 1);

    let dm_intent = state
        .last_notification_intent
        .as_ref()
        .expect("DM intent recorded");
    assert!(dm_intent.is_high_priority());
    let dm_prepared = dm_intent.prepared().expect("prepared DM notification");
    assert_eq!(dm_prepared.notification.app_name, "Discord");
    assert_eq!(dm_prepared.notification.summary, "DemoUser");
    assert_eq!(dm_prepared.notification.body, "Привет в ЛС! Проверь PR.");
    assert_eq!(dm_prepared.hints.urgency, Some(Urgency::Critical));
    assert_eq!(dm_prepared.hints.category.as_deref(), Some("im.received"));

    // 2. Direct <@id> mention in unselected guild channel -> HighPriorityMentionOrDm with Urgency::Normal
    state.apply_background_event(BackgroundEvent::GatewayMessageCreate {
        id: "msg-mention-direct".into(),
        channel_id: "ch-gpui".into(),
        guild_id: Some("guild-1".into()),
        author_id: "u-architect".into(),
        author_name: "ChiefArchitect".into(),
        content: "Пинг <@384299970632876032> по задаче W4-3".into(),
        timestamp: None,
    });

    assert_eq!(state.notification_telemetry.high_priority_emitted, 2);
    let direct_intent = state.last_notification_intent.as_ref().unwrap();
    assert!(direct_intent.is_high_priority());
    let direct_prepared = direct_intent.prepared().unwrap();
    assert_eq!(
        direct_prepared.notification.summary,
        "ChiefArchitect (#gpui-architecture)"
    );
    assert_eq!(direct_prepared.hints.urgency, Some(Urgency::Normal));

    let g1 = state.guilds.iter().find(|g| g.id == "guild-1").unwrap();
    assert_eq!(
        g1.mention_count, 1,
        "unselected guild channel mention must increment guild.mention_count"
    );

    // 3. Nickname mention <@!id> -> HighPriorityMentionOrDm
    state.apply_background_event(BackgroundEvent::GatewayMessageCreate {
        id: "msg-mention-nick".into(),
        channel_id: "ch-gpui".into(),
        guild_id: Some("guild-1".into()),
        author_id: "u-architect".into(),
        author_name: "ChiefArchitect".into(),
        content: "Срочно <@!384299970632876032>!".into(),
        timestamp: None,
    });
    assert_eq!(state.notification_telemetry.high_priority_emitted, 3);
    assert!(
        state
            .last_notification_intent
            .as_ref()
            .unwrap()
            .is_high_priority()
    );

    // 4. Role mention <@&role_id> -> HighPriorityMentionOrDm
    state.apply_background_event(BackgroundEvent::GatewayMessageCreate {
        id: "msg-mention-role".into(),
        channel_id: "ch-gpui".into(),
        guild_id: Some("guild-1".into()),
        author_id: "u-architect".into(),
        author_name: "ChiefArchitect".into(),
        content: "Внимание группе <@&987654321098765432> релиз готов".into(),
        timestamp: None,
    });
    assert_eq!(state.notification_telemetry.high_priority_emitted, 4);
    assert!(
        state
            .last_notification_intent
            .as_ref()
            .unwrap()
            .is_high_priority()
    );

    // 5. @everyone and @here when enabled -> HighPriorityMentionOrDm
    state.apply_background_event(BackgroundEvent::GatewayMessageCreate {
        id: "msg-mention-everyone".into(),
        channel_id: "ch-announcements".into(),
        guild_id: Some("guild-1".into()),
        author_id: "u-architect".into(),
        author_name: "ChiefArchitect".into(),
        content: "@everyone сбор через 5 минут".into(),
        timestamp: None,
    });
    assert_eq!(state.notification_telemetry.high_priority_emitted, 5);

    state.apply_background_event(BackgroundEvent::GatewayMessageCreate {
        id: "msg-mention-here".into(),
        channel_id: "ch-announcements".into(),
        guild_id: Some("guild-1".into()),
        author_id: "u-architect".into(),
        author_name: "ChiefArchitect".into(),
        content: "@here кто онлайн?".into(),
        timestamp: None,
    });
    assert_eq!(state.notification_telemetry.high_priority_emitted, 6);

    // 6. Regular message in channel with NotificationLevel::AllMessages -> Normal(Urgency::Low)
    state.set_channel_notification_level("ch-gpui", NotificationLevel::AllMessages);
    state.apply_background_event(BackgroundEvent::GatewayMessageCreate {
        id: "msg-normal-all".into(),
        channel_id: "ch-gpui".into(),
        guild_id: Some("guild-1".into()),
        author_id: "u-demo-1".into(),
        author_name: "DemoUser".into(),
        content: "Обычное сообщение в канале AllMessages".into(),
        timestamp: None,
    });

    assert_eq!(state.notification_telemetry.high_priority_emitted, 6);
    assert_eq!(state.notification_telemetry.normal_emitted, 1);
    let normal_intent = state.last_notification_intent.as_ref().unwrap();
    assert!(normal_intent.is_normal());
    let normal_prepared = normal_intent.prepared().unwrap();
    assert_eq!(normal_prepared.hints.urgency, Some(Urgency::Low));
    assert_eq!(
        normal_prepared.notification.summary,
        "DemoUser (#gpui-architecture)"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. P1-4.3 Negative: Self-Author, DND, Focused Active, Muted, Duplicate
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn p1_4_3_suppresses_self_author_dnd_focused_active_muted_and_duplicates() {
    let mut state = setup_guild_state();
    let self_id = state.current_user.id.clone();

    // 1. Self-authored message in DM or Guild with self-mention must be suppressed
    state.apply_background_event(BackgroundEvent::GatewayMessageCreate {
        id: "msg-self-1".into(),
        channel_id: "dm-u-demo-1".into(),
        guild_id: None,
        author_id: self_id.clone(),
        author_name: "pharos12510".into(),
        content: format!("Моё собственное сообщение <@{self_id}>"),
        timestamp: None,
    });
    assert_eq!(
        state.last_notification_intent,
        Some(NotificationPriorityIntent::Suppressed(
            NotificationSuppressionReason::SelfAuthor
        ))
    );
    assert_eq!(state.notification_telemetry.suppressed_self_author, 1);
    assert!(state.pending_notifications.is_empty());

    // 2. Focused active channel (ch-general) suppresses even direct mentions and DMs,
    //    but emits when app is unfocused!
    state.apply_background_event(BackgroundEvent::GatewayMessageCreate {
        id: "msg-active-focused".into(),
        channel_id: "ch-general".into(),
        guild_id: Some("guild-1".into()),
        author_id: "u-demo-1".into(),
        author_name: "DemoUser".into(),
        content: format!("Привет <@{self_id}> в активном канале"),
        timestamp: None,
    });
    assert_eq!(
        state.last_notification_intent,
        Some(NotificationPriorityIntent::Suppressed(
            NotificationSuppressionReason::FocusedActiveChannel
        ))
    );
    assert_eq!(
        state
            .notification_telemetry
            .suppressed_focused_active_channel,
        1
    );
    assert!(state.pending_notifications.is_empty());

    // Unfocus window -> message in selected channel now emits HighPriorityMentionOrDm!
    state.set_app_focused(false);
    state.apply_background_event(BackgroundEvent::GatewayMessageCreate {
        id: "msg-active-unfocused".into(),
        channel_id: "ch-general".into(),
        guild_id: Some("guild-1".into()),
        author_id: "u-demo-1".into(),
        author_name: "DemoUser".into(),
        content: format!("Привет <@{self_id}> когда окно свёрнуто"),
        timestamp: None,
    });
    assert!(
        state
            .last_notification_intent
            .as_ref()
            .unwrap()
            .is_high_priority()
    );
    assert_eq!(state.pending_notifications.len(), 1);
    state.set_app_focused(true);

    // 3. User DND suppresses all notifications (DM, direct mention, normal)
    state.set_status(PresenceStatus::Dnd);
    state.apply_background_event(BackgroundEvent::GatewayMessageCreate {
        id: "msg-dnd-dm".into(),
        channel_id: "dm-u-demo-1".into(),
        guild_id: None,
        author_id: "u-demo-1".into(),
        author_name: "DemoUser".into(),
        content: "Срочное ЛС во время DND".into(),
        timestamp: None,
    });
    assert_eq!(
        state.last_notification_intent,
        Some(NotificationPriorityIntent::Suppressed(
            NotificationSuppressionReason::UserDnd
        ))
    );
    assert_eq!(state.notification_telemetry.suppressed_dnd, 1);
    state.set_status(PresenceStatus::Online);

    // 4. Muted channel suppresses normal messages, @everyone/@here, and unmentioned DMs
    state.set_channel_muted("ch-gpui", true);
    state.set_channel_notification_level("ch-gpui", NotificationLevel::AllMessages);

    state.apply_background_event(BackgroundEvent::GatewayMessageCreate {
        id: "msg-muted-normal".into(),
        channel_id: "ch-gpui".into(),
        guild_id: Some("guild-1".into()),
        author_id: "u-demo-1".into(),
        author_name: "DemoUser".into(),
        content: "Шум в заглушенном канале".into(),
        timestamp: None,
    });
    assert_eq!(
        state.last_notification_intent,
        Some(NotificationPriorityIntent::Suppressed(
            NotificationSuppressionReason::ChannelMuted
        ))
    );

    state.apply_background_event(BackgroundEvent::GatewayMessageCreate {
        id: "msg-muted-everyone".into(),
        channel_id: "ch-gpui".into(),
        guild_id: Some("guild-1".into()),
        author_id: "u-demo-1".into(),
        author_name: "DemoUser".into(),
        content: "@everyone спам в заглушенном канале".into(),
        timestamp: None,
    });
    assert_eq!(
        state.last_notification_intent,
        Some(NotificationPriorityIntent::Suppressed(
            NotificationSuppressionReason::ChannelMuted
        ))
    );
    assert_eq!(state.notification_telemetry.suppressed_muted_channel, 2);

    // Muted DM channel without direct mention is suppressed
    state.set_channel_muted("dm-u-demo-1", true);
    state.apply_background_event(BackgroundEvent::GatewayMessageCreate {
        id: "msg-muted-dm".into(),
        channel_id: "dm-u-demo-1".into(),
        guild_id: None,
        author_id: "u-demo-1".into(),
        author_name: "DemoUser".into(),
        content: "ЛС в заглушенном диалоге".into(),
        timestamp: None,
    });
    assert_eq!(
        state.last_notification_intent,
        Some(NotificationPriorityIntent::Suppressed(
            NotificationSuppressionReason::ChannelMuted
        ))
    );
    assert_eq!(state.notification_telemetry.suppressed_muted_channel, 3);
    state.set_channel_muted("dm-u-demo-1", false);
    state.set_channel_muted("ch-gpui", false);

    // 5. NotificationLevel::Nothing suppresses even direct <@id> mentions
    state.set_channel_notification_level("ch-gpui", NotificationLevel::Nothing);
    state.apply_background_event(BackgroundEvent::GatewayMessageCreate {
        id: "msg-nothing-mention".into(),
        channel_id: "ch-gpui".into(),
        guild_id: Some("guild-1".into()),
        author_id: "u-demo-1".into(),
        author_name: "DemoUser".into(),
        content: format!("Прямой пинг <@{self_id}> при уровне Nothing"),
        timestamp: None,
    });
    assert_eq!(
        state.last_notification_intent,
        Some(NotificationPriorityIntent::Suppressed(
            NotificationSuppressionReason::ChannelNothing
        ))
    );

    // 6. OnlyMentions channel suppresses messages without mentions or mentioning another user
    state.set_channel_notification_level("ch-gpui", NotificationLevel::OnlyMentions);
    state.apply_background_event(BackgroundEvent::GatewayMessageCreate {
        id: "msg-only-mentions-other".into(),
        channel_id: "ch-gpui".into(),
        guild_id: Some("guild-1".into()),
        author_id: "u-demo-1".into(),
        author_name: "DemoUser".into(),
        content: "Пинг другого участника <@999999999999999999>".into(),
        timestamp: None,
    });
    assert_eq!(
        state.last_notification_intent,
        Some(NotificationPriorityIntent::Suppressed(
            NotificationSuppressionReason::NotMentionInOnlyMentionsChannel
        ))
    );

    // 7. Duplicate message ID is suppressed
    state.apply_background_event(BackgroundEvent::GatewayMessageCreate {
        id: "msg-active-unfocused".into(),
        channel_id: "ch-general".into(),
        guild_id: Some("guild-1".into()),
        author_id: "u-demo-1".into(),
        author_name: "DemoUser".into(),
        content: format!("Дубликат <@{self_id}>"),
        timestamp: None,
    });
    assert_eq!(
        state.last_notification_intent,
        Some(NotificationPriorityIntent::Suppressed(
            NotificationSuppressionReason::DuplicateMessage
        ))
    );
    assert_eq!(state.notification_telemetry.suppressed_duplicate, 1);
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. P1-4.3 Boundaries, Redaction, Empty/Attachment & Flood Telemetry
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn p1_4_3_snowflake_boundaries_2000_char_truncation_redaction_and_bounded_flood() {
    let mut state = setup_guild_state();

    // Snowflake "0" boundary and prefix non-collision (<@0> vs <@01>)
    state.current_user.id = "0".into();
    state.apply_background_event(BackgroundEvent::GatewayMessageCreate {
        id: "msg-prefix-collision".into(),
        channel_id: "ch-gpui".into(),
        guild_id: Some("guild-1".into()),
        author_id: "u-demo-1".into(),
        author_name: "DemoUser".into(),
        content: "Не наш ID: <@01> и email user@everyone.com".into(),
        timestamp: None,
    });
    assert!(
        state
            .last_notification_intent
            .as_ref()
            .unwrap()
            .is_suppressed(),
        "<@01> and user@everyone.com must not false-match user ID 0 or @everyone"
    );

    state.apply_background_event(BackgroundEvent::GatewayMessageCreate {
        id: "msg-snowflake-zero".into(),
        channel_id: "ch-gpui".into(),
        guild_id: Some("guild-1".into()),
        author_id: "u-demo-1".into(),
        author_name: "DemoUser".into(),
        content: "Точный пинг <@0>!".into(),
        timestamp: None,
    });
    assert!(
        state
            .last_notification_intent
            .as_ref()
            .unwrap()
            .is_high_priority()
    );

    // Snowflake u64::MAX ("18446744073709551615") + secret token redaction + 2000-char boundary
    state.current_user.id = "18446744073709551615".into();
    let long_tail: String = "Я🚀".chars().cycle().take(1950).collect();
    let secret_and_long =
        format!("<@!18446744073709551615> mfa.secret_leak_token_1234567890 {long_tail}");
    state.apply_background_event(BackgroundEvent::GatewayMessageCreate {
        id: "msg-u64-max-redacted".into(),
        channel_id: "ch-gpui".into(),
        guild_id: Some("guild-1".into()),
        author_id: "u-demo-1".into(),
        author_name: "DemoUser".into(),
        content: secret_and_long,
        timestamp: None,
    });

    let prepared = state
        .last_notification_intent
        .as_ref()
        .unwrap()
        .prepared()
        .expect("u64::MAX mention must emit");
    assert!(
        !prepared.notification.body.contains("secret_leak_token"),
        "MFA token must be redacted in notification body"
    );
    assert!(prepared.notification.body.contains("<redacted>"));
    assert!(prepared.notification.body.chars().count() <= 256);
    assert!(prepared.notification.body.ends_with('…'));

    // Empty content without attachment is suppressed; empty content with attachment emits "[Вложение]"
    state.apply_background_event(BackgroundEvent::GatewayMessageCreate {
        id: "msg-empty-whitespace".into(),
        channel_id: "dm-u-demo-1".into(),
        guild_id: None,
        author_id: "u-demo-1".into(),
        author_name: "DemoUser".into(),
        content: "   \t\n  ".into(),
        timestamp: None,
    });
    assert_eq!(
        state.last_notification_intent,
        Some(NotificationPriorityIntent::Suppressed(
            NotificationSuppressionReason::EmptyContentWithoutAttachments
        ))
    );

    let attach_intent = state.evaluate_message_notification(
        "msg-attach-only",
        "dm-u-demo-1",
        None,
        "u-demo-1",
        "DemoUser",
        "",
        true,
    );
    assert!(attach_intent.is_high_priority());
    assert_eq!(
        attach_intent.prepared().unwrap().notification.body,
        "[Вложение]"
    );

    // Flood 300 DMs -> pending_notifications and recent_intents remain strictly bounded
    for idx in 0..300u32 {
        state.apply_background_event(BackgroundEvent::GatewayMessageCreate {
            id: format!("msg-flood-{idx}"),
            channel_id: "dm-u-demo-1".into(),
            guild_id: None,
            author_id: "u-demo-1".into(),
            author_name: "DemoUser".into(),
            content: format!("Flood message #{idx}"),
            timestamp: None,
        });
    }
    assert!(
        state.pending_notifications.len() <= AppState::MAX_NOTIFICATION_QUEUE_LEN,
        "pending_notifications must be bounded to MAX_NOTIFICATION_QUEUE_LEN"
    );
    assert!(
        state.notification_telemetry.recent_intents.len()
            <= AppState::MAX_NOTIFICATION_TELEMETRY_LOG,
        "recent_intents must be bounded to MAX_NOTIFICATION_TELEMETRY_LOG"
    );
    assert!(state.notification_telemetry.high_priority_emitted >= 303);
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. P2-2.3 Positive & Transitions: SET_ACTIVITY RPC -> AppState & UserProfileCard
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn p2_2_3_set_activity_updates_app_state_emits_op3_json_and_renders_in_user_profile_card() {
    let mut state = setup_guild_state();
    state.open_member_profile(&state.current_user.id.clone());
    assert!(state.selected_member_profile.is_some());

    // Initially no RPC activity is active
    assert!(state.active_rpc_activity.is_none());
    assert!(UserProfileCard::rich_presence_block(&state).is_none());

    // Build rich SET_ACTIVITY payload with all fields: name, details, state, timestamps (legacy seconds),
    // assets, party, buttons (2 buttons)
    let activity = RpcActivity::new()
        .with_name("Rust Analyzer")
        .with_activity_type(0)
        .with_details("Editing state/mod.rs")
        .with_state("Workspace: discord-lab")
        .with_application_id("383226320970055681")
        .with_timestamps(RpcTimestamps::new(Some(1_700_000_000), Some(1_700_003_600)))
        .with_assets(RpcAssets::new(
            Some("rust_logo_large".into()),
            Some("Rust 2024 Edition".into()),
            Some("gpui_small".into()),
            Some("GPUI 120 FPS".into()),
        ))
        .with_party(RpcParty::new(Some("party-rust-01".into()), Some([2, 4])))
        .with_buttons(vec![
            RpcButton::new("Repository", "https://github.com/rust-lang/rust"),
            RpcButton::new("Docs", "https://doc.rust-lang.org/std/"),
        ])
        .with_instance(true);

    let payload = SetActivityPayload::new(4242, Some(activity.clone()), "nonce-rpc-001");
    let raw_cmd_json = payload.to_json();

    // Apply via JSON command helper
    let applied = state
        .apply_set_activity_json(&raw_cmd_json)
        .expect("valid SET_ACTIVITY JSON must succeed");
    assert_eq!(applied.pid, 4242);
    assert_eq!(applied.nonce, "nonce-rpc-001");

    // Verify AppState stores normalized Rich Presence
    assert_eq!(state.active_rpc_pid, Some(4242));
    assert_eq!(state.last_rpc_nonce.as_deref(), Some("nonce-rpc-001"));
    let stored = state
        .active_rpc_activity
        .as_ref()
        .expect("active_rpc_activity must be populated");
    assert_eq!(stored.name.as_deref(), Some("Rust Analyzer"));
    assert_eq!(stored.details.as_deref(), Some("Editing state/mod.rs"));
    assert_eq!(stored.state.as_deref(), Some("Workspace: discord-lab"));
    assert_eq!(
        stored.timestamps,
        Some(RpcTimestamps::new(
            Some(1_700_000_000_000),
            Some(1_700_003_600_000)
        )),
        "legacy seconds timestamps (< 10_000_000_000) must be normalized to milliseconds"
    );
    assert_eq!(stored.buttons.len(), 2);

    // Verify Gateway Op 3 PRESENCE_UPDATE frame in outbound_gateway_frames matches to_gateway_presence_update_json()
    assert_eq!(state.outbound_gateway_frames.len(), 1);
    let op3_raw = &state.outbound_gateway_frames[0];
    assert_eq!(op3_raw, &stored.to_gateway_presence_update_json());

    let op3_val: serde_json::Value =
        serde_json::from_str(op3_raw).expect("valid Gateway Op 3 JSON");
    assert_eq!(op3_val["op"], 3);
    assert_eq!(op3_val["d"]["status"], "online");
    assert_eq!(op3_val["d"]["afk"], false);
    assert!(op3_val["d"]["since"].is_null());
    let act_json = &op3_val["d"]["activities"][0];
    assert_eq!(act_json["name"], "Rust Analyzer");
    assert_eq!(act_json["type"], 0);
    assert_eq!(act_json["details"], "Editing state/mod.rs");
    assert_eq!(act_json["state"], "Workspace: discord-lab");
    assert_eq!(act_json["timestamps"]["start"], 1_700_000_000_000u64);
    assert_eq!(act_json["timestamps"]["end"], 1_700_003_600_000u64);
    assert_eq!(act_json["assets"]["large_image"], "rust_logo_large");
    assert_eq!(act_json["assets"]["large_text"], "Rust 2024 Edition");
    assert_eq!(act_json["assets"]["small_image"], "gpui_small");
    assert_eq!(act_json["assets"]["small_text"], "GPUI 120 FPS");
    assert_eq!(act_json["party"]["id"], "party-rust-01");
    assert_eq!(act_json["party"]["size"], serde_json::json!([2, 4]));
    assert_eq!(act_json["buttons"][0]["label"], "Repository");
    assert_eq!(act_json["buttons"][1]["label"], "Docs");

    // Verify UserProfileCard live Rich Presence block view model
    let block = UserProfileCard::rich_presence_block(&state)
        .expect("UserProfileCard must render live Rich Presence block");
    assert_eq!(block.name, "Rust Analyzer");
    assert_eq!(block.details.as_deref(), Some("Editing state/mod.rs"));
    assert_eq!(
        block.state_line.as_deref(),
        Some("Workspace: discord-lab (2 из 4)")
    );
    assert_eq!(block.party_text.as_deref(), Some("(2 из 4)"));
    assert_eq!(block.large_image.as_deref(), Some("rust_logo_large"));
    assert_eq!(block.large_text.as_deref(), Some("Rust 2024 Edition"));
    assert_eq!(block.small_image.as_deref(), Some("gpui_small"));
    assert_eq!(block.small_text.as_deref(), Some("GPUI 120 FPS"));
    assert!(block.timestamps_text.is_some());
    assert_eq!(
        block.buttons,
        vec![
            (
                "Repository".to_string(),
                "https://github.com/rust-lang/rust".to_string()
            ),
            (
                "Docs".to_string(),
                "https://doc.rust-lang.org/std/".to_string()
            ),
        ]
    );

    // Clear presence with activity == None -> clears AppState, enqueues empty activities Op 3, and clears UserProfileCard
    let clear_payload = SetActivityPayload::new(4242, None, "nonce-rpc-clear");
    state
        .apply_set_activity_payload(clear_payload)
        .expect("clearing SET_ACTIVITY must succeed");

    assert!(state.active_rpc_activity.is_none());
    assert!(state.active_rpc_pid.is_none());
    assert_eq!(state.last_rpc_nonce.as_deref(), Some("nonce-rpc-clear"));
    assert_eq!(state.outbound_gateway_frames.len(), 2);

    let clear_op3: serde_json::Value =
        serde_json::from_str(&state.outbound_gateway_frames[1]).expect("valid clear Op 3 JSON");
    assert_eq!(clear_op3["op"], 3);
    assert_eq!(clear_op3["d"]["activities"], serde_json::json!([]));

    assert!(
        UserProfileCard::rich_presence_block(&state).is_none(),
        "UserProfileCard must clear Rich Presence block when activity == None"
    );
    assert_eq!(state.rpc_activity_telemetry.updates_applied, 1);
    assert_eq!(state.rpc_activity_telemetry.clears_applied, 1);
    assert_eq!(state.rpc_activity_telemetry.frames_emitted, 2);
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. P2-2.3 Negative, Validation Errors, Boundaries & Account Isolation
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn p2_2_3_validation_errors_preserve_state_and_logout_clears_presence() {
    let mut state = setup_guild_state();

    // Establish valid baseline activity
    let valid_act = RpcActivity::new()
        .with_name("Neovim")
        .with_activity_type(2) // Listening
        .with_details("Lo-Fi Beats")
        .with_state("Coding")
        .with_party(RpcParty::new(None, Some([1, 1])));
    state
        .apply_set_activity_payload(SetActivityPayload::new(
            100,
            Some(valid_act.clone()),
            "nonce-1",
        ))
        .unwrap();
    assert_eq!(state.outbound_gateway_frames.len(), 1);

    // 1. Invalid activity_type (> 5) rejected
    let mut bad_type = valid_act.clone();
    bad_type.activity_type = 9;
    assert_eq!(
        state.apply_set_activity_payload(SetActivityPayload::new(
            100,
            Some(bad_type),
            "nonce-bad1"
        )),
        Err(RpcIpcError::InvalidField("type"))
    );

    // 2. Invalid timestamps (end < start) rejected
    let bad_ts = valid_act
        .clone()
        .with_timestamps(RpcTimestamps::new(Some(2000), Some(1000)));
    assert_eq!(
        state.apply_set_activity_payload(SetActivityPayload::new(100, Some(bad_ts), "nonce-bad2")),
        Err(RpcIpcError::InvalidField("timestamps"))
    );

    // 3. Too many buttons (> 2) or non-HTTPS URL rejected
    let bad_btns = valid_act.clone().with_buttons(vec![
        RpcButton::new("B1", "https://example.com/1"),
        RpcButton::new("B2", "https://example.com/2"),
        RpcButton::new("B3", "https://example.com/3"),
    ]);
    assert_eq!(
        state.apply_set_activity_payload(SetActivityPayload::new(
            100,
            Some(bad_btns),
            "nonce-bad3"
        )),
        Err(RpcIpcError::InvalidField("buttons"))
    );

    let bad_http_btn = valid_act
        .clone()
        .with_buttons(vec![RpcButton::new("Insecure", "http://example.com")]);
    assert_eq!(
        state.apply_set_activity_payload(SetActivityPayload::new(
            100,
            Some(bad_http_btn),
            "nonce-bad4"
        )),
        Err(RpcIpcError::InvalidField("buttons"))
    );

    // 4. pid == 0 rejected
    assert_eq!(
        state.apply_set_activity_payload(SetActivityPayload::new(0, None, "nonce-bad-pid")),
        Err(RpcIpcError::InvalidField("pid"))
    );

    // 5. Malformed JSON rejected
    assert!(state.apply_set_activity_json("{not_valid_json").is_err());

    // Baseline activity and outbound frames must remain untouched after all rejected errors
    assert_eq!(
        state
            .active_rpc_activity
            .as_ref()
            .and_then(|a| a.name.as_deref()),
        Some("Neovim")
    );
    assert_eq!(state.outbound_gateway_frames.len(), 1);
    assert_eq!(state.rpc_activity_telemetry.rejected_errors, 6);

    // Verify all activity types 0..=5 produce distinct non-empty header labels in UserProfileCard
    for act_type in 0..=5u8 {
        let typed = RpcActivity::new()
            .with_name("TestApp")
            .with_activity_type(act_type);
        state.set_rpc_activity(Some(typed)).unwrap();
        let vm = UserProfileCard::rich_presence_block(&state).unwrap();
        assert!(!vm.header_label.is_empty());
        assert!(UserProfileCard::has_active_rich_presence(&state));
    }

    // Verify member profile with no activity and no active RPC activity returns None
    state.set_rpc_activity(None).unwrap();
    state.selected_member_profile = Some(GuildMember {
        id: "u-no-act".into(),
        username: "QuietMember".into(),
        global_name: None,
        avatar: None,
        status: PresenceStatus::Online,
        activity: None,
        roles: Vec::new(),
    });
    assert!(UserProfileCard::rich_presence_block(&state).is_none());
    assert!(!UserProfileCard::has_active_rich_presence(&state));

    // Logout must clear active_rpc_activity and pending_notifications
    let (tx, rx) = mpsc::channel();
    state.attach_background_receiver(rx);
    tx.send(BackgroundEvent::GatewayDisconnected {
        close_code: Some(4004),
        resumable: false,
        reason: "Authentication failed.".into(),
    })
    .unwrap();
    state.poll_background_events();
    assert!(state.active_rpc_activity.is_none());
    assert!(state.active_rpc_pid.is_none());
    assert!(state.pending_notifications.is_empty());
}
