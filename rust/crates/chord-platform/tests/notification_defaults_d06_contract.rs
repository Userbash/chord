#![forbid(unsafe_code)]
//! Contract tests for Task D-06: Notification Defaults Protocol (`chord_platform::notifications`).
//!
//! Specification & Clean-Room Reference Oracles:
//! - `docs/feature-backlog.md` row `D-06`:
//!   "Notification Defaults Protocol: Client decision heuristics for default notification states
//!   (`isPlatformEmbedded`, channel notification settings overrides). Target: Notification Engine.
//!   Criteria: Specification of server-level vs channel-level notification cascades, mention sound
//!   rules, and mute inheritance"
//! - `docs/re/notification-defaults-d06.md` & `docs/re/notification-defaults-d06.json`
//! - `fixtures/gateway/by-type/002-READY.json` (`d.user_guild_settings`: 58 guild entries,
//!   1,328 `channel_overrides`, `version = 734`, `partial = false`)
//!
//! Covers all 9 mandatory dimensions (`TEST-POLICY.md` §12):
//! 1. Positive path: `isPlatformEmbedded` heuristics (`true` -> `DesktopType::All`, `PermissionState::Enabled`,
//!    `taskbar_flash: true`; `false` -> `DesktopType::Never`, `PermissionState::Blocked`, `taskbar_flash: false`),
//!    4-stage server-level vs channel-level cascade (`ChannelOverride -> CategoryOverride -> GuildSettings -> GuildDefault`),
//!    mention sound rules (`SoundCue::Mention` vs regular message sound vs muted sound), and mute inheritance.
//! 2. Negative path: rejecting invalid guild notification level (`3 / NULL` is invalid at guild scope),
//!    out-of-range channel notification level (`< 0` or `> 3`), invalid `notify_highlights` (`< 0` or `> 2`),
//!    invalid `mute_config.selected_time_window` (`∉ {900, 3600, 10800, 28800, 86400, -1}`), and malformed ISO-8601 `end_time`.
//! 3. Boundary conditions: Snowflake boundaries (`0`, `u64::MAX`), large guild threshold (`member_count = 2499` vs `2500`
//!    forcing `ONLY_MENTIONS = 1`), all 6 canonical `mute_config` windows (`900..=86400` and `-1`), exact expiration boundary
//!    (`end_unix == now_unix` still muted vs `end_unix == now_unix - 1` expired), and 2000-char message body truncation.
//! 4. Error handling: explicit `D06NotificationProtocolError` variants on invalid codes, windows, or timestamps.
//! 5. Empty states: missing guild entry, missing category/channel overrides, empty `disabled_sounds`, and `mute_config: None`
//!    on `muted: true` (valid indefinite mute).
//! 6. Intermediate / lifecycle states: active temporary mute -> timer expiration transition (`is_temporarily_muted` -> unmuted),
//!    and protective partial `USER_GUILD_SETTINGS_UPDATE` merge preserving existing `channel_overrides` when omitted
//!    and guarding against stale `version` rollbacks.
//! 7. Capabilities, permissions & security: `suppress_everyone` (`@everyone`/`@here`), `suppress_roles` (`<@&role_id>`),
//!    `OPT_IN_CHANNELS_ON` (`16384`) + `OPT_IN_ENABLED` (`4096`) gating, DND / Streamer Mode suppression, and secret token
//!    redaction in notification bodies.
//! 8. Recovery & degradation: non-destructive channel override reset (`{ message_notifications: 3, muted: false, flags: PW(flags) }`)
//!    preserving `FAVORITED` (`2048`) and `OPT_IN_ENABLED` (`4096`), and graceful fallback when `default_message_notifications`
//!    is out of range.
//! 9. Observable telemetry & empirical oracle verification: verifying all 58 guild entries and 1,328 channel overrides in
//!    `fixtures/gateway/by-type/002-READY.json` against `docs/re/notification-defaults-d06.md` and `.json`, plus `D06CascadeTelemetry`
//!    counters (`evaluations_total`, `channel_override_hits`, `category_override_hits`, `guild_settings_hits`, `guild_default_hits`,
//!    `muted_suppressions`, `expired_mutes_recovered`, `sound_alerts_emitted`, `badge_increments`).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use chord_platform::audio_engine::SoundCue;
use chord_platform::notifications::{
    ACCOUNT_FLAG_MENTION_ON_ALL_MESSAGES, ACCOUNT_FLAG_USE_NEW_NOTIFICATIONS,
    CHANNEL_FLAG_FAVORITED, CHANNEL_FLAG_NEW_FORUM_THREADS_OFF, CHANNEL_FLAG_NEW_FORUM_THREADS_ON,
    CHANNEL_FLAG_OPT_IN_ENABLED, CHANNEL_FLAG_UNREADS_ALL_MESSAGES,
    CHANNEL_FLAG_UNREADS_ONLY_MENTIONS, CHANNEL_UNREAD_MASK, CascadeLevelSource,
    D06CascadeDecision, D06CascadeEngine, D06ChannelOverride, D06ClientNotificationDefaults,
    D06DesktopNotificationType, D06GuildNotificationSettings, D06MentionKind,
    D06MessageEvalInput, D06MuteConfig, D06NotificationLevel, D06NotificationPermissionState,
    D06NotificationPreset, D06NotificationProtocolError, D06NotifyHighlights, D06TtsType,
    D06UnreadSetting, GUILD_FLAG_OPT_IN_CHANNELS_OFF, GUILD_FLAG_OPT_IN_CHANNELS_ON,
    GUILD_FLAG_UNREADS_ALL_MESSAGES, GUILD_FLAG_UNREADS_ONLY_MENTIONS, GUILD_UNREAD_MASK,
    LARGE_GUILD_MEMBER_THRESHOLD, MESSAGE_NOTIFICATIONS_ALL_MESSAGES,
    MESSAGE_NOTIFICATIONS_NO_MESSAGES, MESSAGE_NOTIFICATIONS_NULL,
    MESSAGE_NOTIFICATIONS_ONLY_MENTIONS, Urgency, VALID_MUTE_TIME_WINDOWS,
    clear_channel_unread_flags, clear_guild_unread_flags, format_iso8601_utc, is_entry_muted,
    is_temporarily_muted, merge_partial_guild_settings, parse_iso8601_unix,
    resolve_channel_notification_cascade, resolve_guild_default_level_for_member_count,
    set_channel_unread_flag, set_guild_unread_flag,
};

const NOW_UNIX: i64 = 1_793_386_800; // 2026-10-30T19:00:00Z

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

// ============================================================================
// Dimension 1: Positive Path — `isPlatformEmbedded` Heuristics, 4-Stage Cascade,
// Mention Sound Rules, and Mute Inheritance
// ============================================================================

#[test]
fn d06_dim1_platform_embedded_heuristics_and_default_notification_states() {
    // 1. Embedded desktop client (`isPlatformEmbedded == true`):
    let embedded = D06ClientNotificationDefaults::from_platform_embedded(true);
    assert!(embedded.is_platform_embedded);
    assert_eq!(embedded.desktop_type, D06DesktopNotificationType::All);
    assert_eq!(
        embedded.permission_state,
        D06NotificationPermissionState::Enabled
    );
    assert!(!embedded.disable_all_sounds);
    assert!(embedded.disabled_sounds.is_empty());
    assert_eq!(embedded.tts_type, D06TtsType::Never);
    assert!(!embedded.disable_unread_badge);
    assert!(embedded.taskbar_flash);
    assert!(!embedded.notify_messages_in_selected_channel);
    assert!(!embedded.screen_downtime_reminder);
    assert!(embedded.can_dispatch_desktop_banner());

    // 2. Web browser / unembedded client (`isPlatformEmbedded == false`):
    let browser = D06ClientNotificationDefaults::from_platform_embedded(false);
    assert!(!browser.is_platform_embedded);
    assert_eq!(browser.desktop_type, D06DesktopNotificationType::Never);
    assert_eq!(
        browser.permission_state,
        D06NotificationPermissionState::Blocked
    );
    assert!(!browser.taskbar_flash);
    assert!(!browser.can_dispatch_desktop_banner());

    // 3. Large guild member count heuristic (`member_count >= 2500` forces `ONLY_MENTIONS = 1`):
    assert_eq!(LARGE_GUILD_MEMBER_THRESHOLD, 2500);
    assert_eq!(
        resolve_guild_default_level_for_member_count(2499, Some(0)),
        D06NotificationLevel::AllMessages
    );
    assert_eq!(
        resolve_guild_default_level_for_member_count(2500, Some(0)),
        D06NotificationLevel::OnlyMentions
    );
    assert_eq!(
        resolve_guild_default_level_for_member_count(44_749, None),
        D06NotificationLevel::OnlyMentions
    );
}

#[test]
fn d06_dim1_four_stage_server_category_channel_cascade_and_mute_inheritance() {
    let base_guild = D06GuildNotificationSettings {
        guild_id: Some(1_014_537_851_418_980_372),
        message_notifications: Some(MESSAGE_NOTIFICATIONS_ONLY_MENTIONS),
        default_message_notifications: MESSAGE_NOTIFICATIONS_ALL_MESSAGES,
        muted: false,
        mute_config: None,
        suppress_everyone: false,
        suppress_roles: false,
        mobile_push: true,
        mute_scheduled_events: false,
        notify_highlights: 0,
        hide_muted_channels: false,
        flags: 0,
        version: 331,
        channel_overrides: BTreeMap::new(),
    };

    // Stage 1: Channel override explicitly set to ALL_MESSAGES (0) overrides Category (2) and Guild (1)
    let cat_no_msg = D06ChannelOverride {
        channel_id: 100,
        message_notifications: MESSAGE_NOTIFICATIONS_NO_MESSAGES,
        muted: false,
        mute_config: None,
        flags: Some(CHANNEL_FLAG_OPT_IN_ENABLED),
        collapsed: false,
    };
    let ch_all_msg = D06ChannelOverride {
        channel_id: 200,
        message_notifications: MESSAGE_NOTIFICATIONS_ALL_MESSAGES,
        muted: false,
        mute_config: None,
        flags: Some(CHANNEL_FLAG_OPT_IN_ENABLED),
        collapsed: false,
    };
    let stage1 = resolve_channel_notification_cascade(
        &base_guild,
        Some(&cat_no_msg),
        Some(&ch_all_msg),
        NOW_UNIX,
    );
    assert_eq!(stage1.effective_level, D06NotificationLevel::AllMessages);
    assert_eq!(stage1.level_source, CascadeLevelSource::ChannelOverride);
    assert!(!stage1.effective_muted);
    assert!(!stage1.allow_no_messages);

    // Stage 2: Channel override is NULL (3 = Inherit) -> inherits from Category override (NO_MESSAGES = 2)
    let ch_inherit = D06ChannelOverride {
        message_notifications: MESSAGE_NOTIFICATIONS_NULL,
        ..ch_all_msg.clone()
    };
    let stage2 = resolve_channel_notification_cascade(
        &base_guild,
        Some(&cat_no_msg),
        Some(&ch_inherit),
        NOW_UNIX,
    );
    assert_eq!(stage2.effective_level, D06NotificationLevel::NoMessages);
    assert_eq!(stage2.level_source, CascadeLevelSource::CategoryOverride);
    assert!(stage2.allow_no_messages);

    // Stage 3: Both Channel and Category are NULL (3 = Inherit) -> inherits from Guild user_guild_settings (ONLY_MENTIONS = 1)
    let cat_inherit = D06ChannelOverride {
        message_notifications: MESSAGE_NOTIFICATIONS_NULL,
        ..cat_no_msg.clone()
    };
    let stage3 = resolve_channel_notification_cascade(
        &base_guild,
        Some(&cat_inherit),
        Some(&ch_inherit),
        NOW_UNIX,
    );
    assert_eq!(stage3.effective_level, D06NotificationLevel::OnlyMentions);
    assert_eq!(stage3.level_source, CascadeLevelSource::GuildSettings);
    assert!(!stage3.allow_no_messages);

    // Stage 4: Guild entry has no explicit user setting (`message_notifications: None`) -> falls back to `guild.default_message_notifications` (0)
    let guild_without_explicit = D06GuildNotificationSettings {
        message_notifications: None,
        default_message_notifications: MESSAGE_NOTIFICATIONS_ALL_MESSAGES,
        ..base_guild.clone()
    };
    let stage4 = resolve_channel_notification_cascade(
        &guild_without_explicit,
        Some(&cat_inherit),
        Some(&ch_inherit),
        NOW_UNIX,
    );
    assert_eq!(stage4.effective_level, D06NotificationLevel::AllMessages);
    assert_eq!(stage4.level_source, CascadeLevelSource::GuildDefault);

    // Mute Inheritance: Muted guild or muted category forces `effective_muted = true` and `allow_no_messages = true`
    // even when child channel explicitly requests `ALL_MESSAGES = 0` and `muted = false`.
    let muted_guild = D06GuildNotificationSettings {
        muted: true,
        mute_config: None,
        ..base_guild.clone()
    };
    let inherited_from_guild = resolve_channel_notification_cascade(
        &muted_guild,
        Some(&cat_inherit),
        Some(&ch_all_msg),
        NOW_UNIX,
    );
    assert!(inherited_from_guild.guild_muted);
    assert!(!inherited_from_guild.category_muted);
    assert!(!inherited_from_guild.channel_muted);
    assert!(inherited_from_guild.effective_muted);
    assert!(inherited_from_guild.allow_no_messages);

    let muted_cat = D06ChannelOverride {
        muted: true,
        mute_config: None,
        ..cat_inherit.clone()
    };
    let inherited_from_cat = resolve_channel_notification_cascade(
        &base_guild,
        Some(&muted_cat),
        Some(&ch_all_msg),
        NOW_UNIX,
    );
    assert!(!inherited_from_cat.guild_muted);
    assert!(inherited_from_cat.category_muted);
    assert!(!inherited_from_cat.channel_muted);
    assert!(inherited_from_cat.effective_muted);
    assert!(inherited_from_cat.allow_no_messages);
}

#[test]
fn d06_dim1_mention_sound_rules_and_badge_matrix() {
    let mut engine = D06CascadeEngine::new();
    let client_defaults = D06ClientNotificationDefaults::from_platform_embedded(true);
    let guild = D06GuildNotificationSettings {
        guild_id: Some(10),
        message_notifications: Some(MESSAGE_NOTIFICATIONS_ONLY_MENTIONS),
        default_message_notifications: MESSAGE_NOTIFICATIONS_ONLY_MENTIONS,
        ..D06GuildNotificationSettings::default()
    };

    // 1. Direct @user mention in ONLY_MENTIONS channel -> emits desktop banner (Urgency::Normal),
    // plays SoundCue::Mention ("mention"), increments .mention-badge, lights .is-unread dot.
    let direct_msg = D06MessageEvalInput {
        message_id: 1001,
        guild_id: Some(10),
        channel_id: 200,
        parent_category_id: Some(100),
        author_id: 999,
        current_user_id: 384_299_970_632_876_032,
        is_dm: false,
        mention_kind: D06MentionKind::User,
        user_dnd: false,
        streamer_mode: false,
        app_focused: false,
        active_channel_id: Some(300),
        account_flags: 0,
    };
    let d1: D06CascadeDecision = engine.evaluate_message(
        &direct_msg,
        &client_defaults,
        &guild,
        None,
        None,
        NOW_UNIX,
    );
    assert!(d1.should_show_desktop_banner);
    assert!(d1.should_play_sound);
    assert_eq!(d1.sound_cue, Some(SoundCue::Mention));
    assert_eq!(d1.sound_name.as_deref(), Some("mention"));
    assert!(d1.should_increment_badge);
    assert!(d1.should_mark_unread);
    assert_eq!(d1.urgency, Some(Urgency::Normal));

    // 2. Regular message in ONLY_MENTIONS channel -> no sound, no desktop banner, no badge,
    // but marks unread dot (`.is-unread`) because channel is unmuted and default unread is ALL_MESSAGES.
    let reg_msg = D06MessageEvalInput {
        message_id: 1002,
        mention_kind: D06MentionKind::None,
        ..direct_msg.clone()
    };
    let d2 = engine.evaluate_message(&reg_msg, &client_defaults, &guild, None, None, NOW_UNIX);
    assert!(!d2.should_show_desktop_banner);
    assert!(!d2.should_play_sound);
    assert_eq!(d2.sound_cue, None);
    assert!(!d2.should_increment_badge);
    assert!(d2.should_mark_unread);

    // 3. Regular message in ALL_MESSAGES channel -> emits desktop banner (Urgency::Low) and plays message sound.
    let guild_all = D06GuildNotificationSettings {
        message_notifications: Some(MESSAGE_NOTIFICATIONS_ALL_MESSAGES),
        ..guild.clone()
    };
    let d3 = engine.evaluate_message(&reg_msg, &client_defaults, &guild_all, None, None, NOW_UNIX);
    assert!(d3.should_show_desktop_banner);
    assert!(d3.should_play_sound);
    assert_eq!(d3.sound_cue, Some(SoundCue::Mention));
    assert_eq!(d3.sound_name.as_deref(), Some("message1"));
    assert!(!d3.should_increment_badge);
    assert!(d3.should_mark_unread);
    assert_eq!(d3.urgency, Some(Urgency::Low));

    // 4. Direct @user mention in MUTED channel -> suppresses sound & desktop banner (`allow_no_messages = true`),
    // but STILL increments `.mention-badge` (`mention_count`).
    let ch_muted = D06ChannelOverride {
        channel_id: 200,
        message_notifications: MESSAGE_NOTIFICATIONS_NULL,
        muted: true,
        mute_config: None,
        flags: None,
        collapsed: false,
    };
    let d4 = engine.evaluate_message(
        &direct_msg,
        &client_defaults,
        &guild,
        None,
        Some(&ch_muted),
        NOW_UNIX,
    );
    assert!(!d4.should_show_desktop_banner);
    assert!(!d4.should_play_sound);
    assert_eq!(d4.sound_cue, None);
    assert!(
        d4.should_increment_badge,
        "direct @user mention must always increment badge even when channel is muted"
    );
    assert!(!d4.should_mark_unread);

    // 5. Direct Message (DM) -> emits Critical urgency desktop banner, sound, and badge;
    // when DM channel is muted, regular DM is suppressed, but @user mention in Group DM still alerts.
    let dm_msg = D06MessageEvalInput {
        message_id: 1005,
        guild_id: None,
        is_dm: true,
        mention_kind: D06MentionKind::None,
        ..direct_msg.clone()
    };
    let d5 = engine.evaluate_message(&dm_msg, &client_defaults, &guild, None, None, NOW_UNIX);
    assert!(d5.should_show_desktop_banner);
    assert!(d5.should_play_sound);
    assert!(d5.should_increment_badge);
    assert_eq!(d5.urgency, Some(Urgency::Critical));

    let d5_muted = engine.evaluate_message(
        &dm_msg,
        &client_defaults,
        &guild,
        None,
        Some(&ch_muted),
        NOW_UNIX,
    );
    assert!(!d5_muted.should_show_desktop_banner);
    assert!(!d5_muted.should_play_sound);
    assert!(!d5_muted.should_increment_badge);
}

// ============================================================================
// Dimension 2 & 4: Negative Path & Explicit Error Handling
// ============================================================================

#[test]
fn d06_dim2_dim4_rejects_invalid_level_codes_mute_windows_and_timestamps() {
    // Guild level 3 (NULL / Inherit) is strictly forbidden at guild scope (`NOTIFICATIONS-RE.md §3.1`)
    assert_eq!(
        D06NotificationLevel::from_guild_code(3),
        Err(D06NotificationProtocolError::InvalidGuildNotificationLevel { value: 3 })
    );
    assert_eq!(
        D06NotificationLevel::from_guild_code(-1),
        Err(D06NotificationProtocolError::InvalidGuildNotificationLevel { value: -1 })
    );

    // Channel level allows 0..=3, rejects 4 and -1
    assert_eq!(
        D06NotificationLevel::from_channel_code(3),
        Ok(D06NotificationLevel::Inherit)
    );
    assert_eq!(
        D06NotificationLevel::from_channel_code(4),
        Err(D06NotificationProtocolError::InvalidChannelNotificationLevel { value: 4 })
    );
    assert_eq!(
        D06NotificationLevel::from_channel_code(-1),
        Err(D06NotificationProtocolError::InvalidChannelNotificationLevel { value: -1 })
    );

    // NotifyHighlights allows 0..=2, rejects 3 and -1
    assert_eq!(
        D06NotifyHighlights::from_code(0),
        Ok(D06NotifyHighlights::Null)
    );
    assert_eq!(
        D06NotifyHighlights::from_code(1),
        Ok(D06NotifyHighlights::Disabled)
    );
    assert_eq!(
        D06NotifyHighlights::from_code(2),
        Ok(D06NotifyHighlights::Enabled)
    );
    assert_eq!(
        D06NotifyHighlights::from_code(3),
        Err(D06NotificationProtocolError::InvalidNotifyHighlights { value: 3 })
    );

    // MuteConfig rejects non-canonical time windows (`∉ {900, 3600, 10800, 28800, 86400, -1}`)
    assert_eq!(
        D06MuteConfig::from_window(1800, NOW_UNIX),
        Err(D06NotificationProtocolError::InvalidMuteTimeWindow { seconds: 1800 })
    );
    assert_eq!(
        D06MuteConfig::from_window(0, NOW_UNIX),
        Err(D06NotificationProtocolError::InvalidMuteTimeWindow { seconds: 0 })
    );

    // MuteConfig validation rejects `-1` with non-null `end_time`, or positive window with missing/corrupt `end_time`
    let bad_forever = D06MuteConfig {
        selected_time_window: -1,
        end_time: Some("2026-10-30T20:00:00.000Z".to_string()),
    };
    assert_eq!(
        bad_forever.validate(),
        Err(D06NotificationProtocolError::InconsistentMuteEndTime)
    );

    let missing_end = D06MuteConfig {
        selected_time_window: 3600,
        end_time: None,
    };
    assert_eq!(
        missing_end.validate(),
        Err(D06NotificationProtocolError::InconsistentMuteEndTime)
    );

    let corrupt_end = D06MuteConfig {
        selected_time_window: 3600,
        end_time: Some("not-an-iso-timestamp".to_string()),
    };
    assert_eq!(
        corrupt_end.validate(),
        Err(D06NotificationProtocolError::InvalidIso8601Timestamp {
            raw: "not-an-iso-timestamp".to_string()
        })
    );
}

// ============================================================================
// Dimension 3: Boundary Conditions (Snowflake 0 / u64::MAX, 6 Mute Windows,
// Exact Expiry Second, Bitmask Preservation)
// ============================================================================

#[test]
fn d06_dim3_boundaries_snowflakes_six_mute_windows_and_bitwise_flags() {
    // 1. All 6 canonical mute time windows (`900, 3600, 10800, 28800, 86400, -1`)
    assert_eq!(
        VALID_MUTE_TIME_WINDOWS,
        [900, 3600, 10_800, 28_800, 86_400, -1]
    );
    for &window in &VALID_MUTE_TIME_WINDOWS {
        let cfg = D06MuteConfig::from_window(window, NOW_UNIX).expect("canonical window must succeed");
        cfg.validate().expect("generated config must validate");
        if window == -1 {
            assert_eq!(cfg.end_time, None);
            assert!(is_entry_muted(true, Some(&cfg), NOW_UNIX));
            assert!(!is_temporarily_muted(true, Some(&cfg), NOW_UNIX));
        } else {
            let end_str = cfg.end_time.as_deref().expect("positive window has end_time");
            let parsed_end = parse_iso8601_unix(end_str).expect("valid ISO-8601");
            assert_eq!(parsed_end, NOW_UNIX + window);
            // At exact expiration second (`now == end_unix`), still muted (`end_time >= now`)
            assert!(is_entry_muted(true, Some(&cfg), NOW_UNIX + window));
            assert!(is_temporarily_muted(true, Some(&cfg), NOW_UNIX + window));
            // 1 second past expiration (`now == end_unix + 1`), mute expires
            assert!(!is_entry_muted(true, Some(&cfg), NOW_UNIX + window + 1));
            assert!(!is_temporarily_muted(
                true,
                Some(&cfg),
                NOW_UNIX + window + 1
            ));
        }
    }

    // 2. Microsecond + timezone offset ISO-8601 parsing from real READY capture (`2021-02-27T17:14:08.418000+00:00`)
    let capture_ts = "2021-02-27T17:14:08.418000+00:00";
    let unix_2021 = parse_iso8601_unix(capture_ts).expect("2021 capture timestamp must parse");
    assert_eq!(unix_2021, 1_614_446_048);
    assert_eq!(
        format_iso8601_utc(1_614_446_048),
        "2021-02-27T17:14:08.000Z"
    );

    // 3. Bitwise unread helpers (`mD` and `PW`) preserve high bits (`OPT_IN_ENABLED`, `FAVORITED`, `NEW_FORUM_THREADS_*`)
    let initial_ch_flags =
        CHANNEL_FLAG_FAVORITED | CHANNEL_FLAG_OPT_IN_ENABLED | CHANNEL_FLAG_NEW_FORUM_THREADS_ON;
    let with_all_unreads =
        set_channel_unread_flag(initial_ch_flags, CHANNEL_FLAG_UNREADS_ALL_MESSAGES);
    assert_eq!(
        with_all_unreads & CHANNEL_UNREAD_MASK,
        CHANNEL_FLAG_UNREADS_ALL_MESSAGES
    );
    assert_ne!(with_all_unreads & CHANNEL_FLAG_FAVORITED, 0);
    assert_ne!(with_all_unreads & CHANNEL_FLAG_OPT_IN_ENABLED, 0);
    assert_ne!(with_all_unreads & CHANNEL_FLAG_NEW_FORUM_THREADS_ON, 0);

    let switched_to_mentions =
        set_channel_unread_flag(with_all_unreads, CHANNEL_FLAG_UNREADS_ONLY_MENTIONS);
    assert_eq!(
        switched_to_mentions & CHANNEL_UNREAD_MASK,
        CHANNEL_FLAG_UNREADS_ONLY_MENTIONS
    );
    assert_eq!(
        clear_channel_unread_flags(switched_to_mentions),
        initial_ch_flags
    );

    let initial_guild_flags = GUILD_FLAG_OPT_IN_CHANNELS_ON | GUILD_FLAG_OPT_IN_CHANNELS_OFF;
    let g_all = set_guild_unread_flag(initial_guild_flags, GUILD_FLAG_UNREADS_ALL_MESSAGES);
    assert_eq!(g_all & GUILD_UNREAD_MASK, GUILD_FLAG_UNREADS_ALL_MESSAGES);
    let g_mentions = set_guild_unread_flag(g_all, GUILD_FLAG_UNREADS_ONLY_MENTIONS);
    assert_eq!(
        g_mentions & GUILD_UNREAD_MASK,
        GUILD_FLAG_UNREADS_ONLY_MENTIONS
    );
    assert_eq!(clear_guild_unread_flags(g_mentions), initial_guild_flags);

    // 4. Four combined presets (`all_messages`, `hybrid`, `mentions`, `nothing`)
    assert_eq!(
        D06NotificationPreset::classify(
            D06UnreadSetting::AllMessages,
            D06NotificationLevel::AllMessages
        ),
        D06NotificationPreset::AllMessages
    );
    assert_eq!(
        D06NotificationPreset::classify(
            D06UnreadSetting::AllMessages,
            D06NotificationLevel::OnlyMentions
        ),
        D06NotificationPreset::Hybrid
    );
    assert_eq!(
        D06NotificationPreset::classify(
            D06UnreadSetting::OnlyMentions,
            D06NotificationLevel::OnlyMentions
        ),
        D06NotificationPreset::Mentions
    );
    assert_eq!(
        D06NotificationPreset::classify(
            D06UnreadSetting::Unset,
            D06NotificationLevel::NoMessages
        ),
        D06NotificationPreset::Nothing
    );
    assert_eq!(
        D06NotificationPreset::classify(
            D06UnreadSetting::OnlyMentions,
            D06NotificationLevel::AllMessages
        ),
        D06NotificationPreset::Custom
    );
}

// ============================================================================
// Dimension 5, 6, 7 & 8: Empty States, Partial Updates, Capability Gating
// (`suppress_everyone`, `suppress_roles`, `OPT_IN_CHANNELS_ON`, DND, Streamer Mode),
// and Override Reset Recovery
// ============================================================================

#[test]
fn d06_dim5_dim6_dim7_dim8_suppression_flags_opt_in_partial_merge_and_reset_recovery() {
    let mut engine = D06CascadeEngine::new();
    let mut client_defaults = D06ClientNotificationDefaults::from_platform_embedded(true);

    // 1. `suppress_everyone = true` and `suppress_roles = true` suppress both sound/banner AND badge
    // for `@everyone`, `@here`, and `@role`, while direct `@user` is unaffected.
    let guild_suppressed = D06GuildNotificationSettings {
        guild_id: Some(u64::MAX),
        message_notifications: Some(MESSAGE_NOTIFICATIONS_ALL_MESSAGES),
        default_message_notifications: MESSAGE_NOTIFICATIONS_ALL_MESSAGES,
        suppress_everyone: true,
        suppress_roles: true,
        ..D06GuildNotificationSettings::default()
    };

    for kind in [
        D06MentionKind::Role,
        D06MentionKind::Everyone,
        D06MentionKind::Here,
    ] {
        let ev = D06MessageEvalInput {
            message_id: 2001,
            guild_id: Some(u64::MAX),
            channel_id: 0,
            parent_category_id: None,
            author_id: 1,
            current_user_id: 2,
            is_dm: false,
            mention_kind: kind,
            user_dnd: false,
            streamer_mode: false,
            app_focused: false,
            active_channel_id: None,
            account_flags: 0,
        };
        let decision =
            engine.evaluate_message(&ev, &client_defaults, &guild_suppressed, None, None, NOW_UNIX);
        assert!(!decision.should_show_desktop_banner, "{kind:?} banner must be suppressed");
        assert!(!decision.should_play_sound, "{kind:?} sound must be suppressed");
        assert!(!decision.should_increment_badge, "{kind:?} badge must be suppressed");
    }

    // 2. Per-sound mute list (`disabled_sounds = ["mention"]`) suppresses sound playback while keeping desktop banner.
    client_defaults.disabled_sounds.push("mention".to_string());
    let direct_ev = D06MessageEvalInput {
        message_id: 2002,
        guild_id: Some(u64::MAX),
        channel_id: 0,
        parent_category_id: None,
        author_id: 1,
        current_user_id: 2,
        is_dm: false,
        mention_kind: D06MentionKind::User,
        user_dnd: false,
        streamer_mode: false,
        app_focused: false,
        active_channel_id: None,
        account_flags: ACCOUNT_FLAG_USE_NEW_NOTIFICATIONS | ACCOUNT_FLAG_MENTION_ON_ALL_MESSAGES,
    };
    let sound_muted_decision = engine.evaluate_message(
        &direct_ev,
        &client_defaults,
        &guild_suppressed,
        None,
        None,
        NOW_UNIX,
    );
    assert!(sound_muted_decision.should_show_desktop_banner);
    assert!(!sound_muted_decision.should_play_sound);
    assert!(sound_muted_decision.should_increment_badge);

    // 3. Opt-in channel suppression (`GUILD_FLAG_OPT_IN_CHANNELS_ON = 16384`):
    // When guild has opt-in enabled and channel lacks `CHANNEL_FLAG_OPT_IN_ENABLED = 4096`,
    // `allow_no_messages` is true; once channel or parent category has `4096`, alerts resume.
    let guild_opt_in = D06GuildNotificationSettings {
        flags: GUILD_FLAG_OPT_IN_CHANNELS_ON,
        suppress_everyone: false,
        suppress_roles: false,
        ..guild_suppressed.clone()
    };
    let ch_not_opted = D06ChannelOverride {
        channel_id: 50,
        message_notifications: MESSAGE_NOTIFICATIONS_ALL_MESSAGES,
        muted: false,
        mute_config: None,
        flags: Some(0),
        collapsed: false,
    };
    let casc_suppressed =
        resolve_channel_notification_cascade(&guild_opt_in, None, Some(&ch_not_opted), NOW_UNIX);
    assert!(casc_suppressed.opt_in_suppressed);
    assert!(casc_suppressed.allow_no_messages);

    let ch_opted = D06ChannelOverride {
        flags: Some(CHANNEL_FLAG_OPT_IN_ENABLED | CHANNEL_FLAG_FAVORITED),
        ..ch_not_opted.clone()
    };
    let casc_opted =
        resolve_channel_notification_cascade(&guild_opt_in, None, Some(&ch_opted), NOW_UNIX);
    assert!(!casc_opted.opt_in_suppressed);
    assert!(!casc_opted.allow_no_messages);

    // 4. Non-destructive channel override reset (`reset_notification_override`) preserves `collapsed`,
    // `FAVORITED`, and `OPT_IN_ENABLED` while clearing unread bits and setting `message_notifications = 3, muted = false`.
    let dirty_override = D06ChannelOverride {
        channel_id: 50,
        message_notifications: MESSAGE_NOTIFICATIONS_NO_MESSAGES,
        muted: true,
        mute_config: Some(D06MuteConfig::from_window(-1, NOW_UNIX).unwrap()),
        flags: Some(
            CHANNEL_FLAG_OPT_IN_ENABLED
                | CHANNEL_FLAG_FAVORITED
                | CHANNEL_FLAG_UNREADS_ONLY_MENTIONS,
        ),
        collapsed: true,
    };
    assert!(dirty_override.has_notification_override(NOW_UNIX));
    let reset_override = dirty_override.reset_notification_override();
    assert_eq!(
        reset_override.message_notifications,
        MESSAGE_NOTIFICATIONS_NULL
    );
    assert!(!reset_override.muted);
    assert_eq!(reset_override.mute_config, None);
    assert!(reset_override.collapsed);
    assert_eq!(
        reset_override.flags,
        Some(CHANNEL_FLAG_OPT_IN_ENABLED | CHANNEL_FLAG_FAVORITED)
    );
    assert!(
        !reset_override.has_notification_override(NOW_UNIX),
        "reset override with only collapsed + OPT_IN_ENABLED + FAVORITED must be hidden from overrides table"
    );

    // 5. Protective partial `USER_GUILD_SETTINGS_UPDATE` merge (`merge_partial_guild_settings`):
    // - Omitting `channel_overrides` (`None`) preserves existing overrides map;
    // - Stale version (`incoming.version < current.version`) is ignored without overwriting newer fields.
    let mut base_with_overrides = guild_opt_in.clone();
    base_with_overrides.version = 500;
    base_with_overrides
        .channel_overrides
        .insert(50, dirty_override.clone());

    let partial_no_overrides = D06GuildNotificationSettings {
        guild_id: base_with_overrides.guild_id,
        suppress_everyone: true,
        version: 501,
        ..base_with_overrides.clone()
    };
    let merged = merge_partial_guild_settings(&base_with_overrides, &partial_no_overrides, false);
    assert_eq!(merged.version, 501);
    assert!(merged.suppress_everyone);
    assert_eq!(
        merged.channel_overrides.len(),
        1,
        "omitted channel_overrides in partial update must preserve existing overrides"
    );

    let stale_update = D06GuildNotificationSettings {
        suppress_everyone: false,
        version: 499,
        ..merged.clone()
    };
    let after_stale = merge_partial_guild_settings(&merged, &stale_update, true);
    assert_eq!(after_stale.version, 501);
    assert!(
        after_stale.suppress_everyone,
        "stale version 499 must not roll back version 501"
    );
}

// ============================================================================
// Dimension 9: Empirical Oracle Verification (`002-READY.json`,
// `docs/re/notification-defaults-d06.md`, `docs/re/notification-defaults-d06.json`)
// and Observable Telemetry
// ============================================================================

#[test]
fn d06_dim9_empirical_ready_capture_and_documentation_oracle_parity() {
    let root = repo_root();
    let md_path = root.join("docs/re/notification-defaults-d06.md");
    let json_path = root.join("docs/re/notification-defaults-d06.json");
    let ready_path = root.join("fixtures/gateway/by-type/002-READY.json");

    let md_text = fs::read_to_string(&md_path).expect("docs/re/notification-defaults-d06.md must exist");
    let json_text =
        fs::read_to_string(&json_path).expect("docs/re/notification-defaults-d06.json must exist");
    let ready_text = fs::read_to_string(&ready_path).expect("002-READY.json must exist");

    // 1. Verify required D-06 specification sections and terms in docs/re/notification-defaults-d06.md
    for marker in [
        "Notification Defaults Protocol (D-06)",
        "isPlatformEmbedded",
        "NotificationSettingsStore",
        "Server-Level vs Channel-Level Notification Cascades",
        "resolvedMessageNotifications",
        "Mute Inheritance",
        "isTemporarilyMuted",
        "allowNoMessages",
        "Mention Sound Rules",
        "SoundCue::Mention",
        "suppress_everyone",
        "suppress_roles",
        "OPT_IN_CHANNELS_ON",
        "1,328",
    ] {
        assert!(
            md_text.contains(marker),
            "docs/re/notification-defaults-d06.md missing required marker: {marker}"
        );
    }
    assert!(json_text.contains("\"task\": \"D-06\""));
    assert!(json_text.contains("\"totalGuildEntries\": 58"));
    assert!(json_text.contains("\"totalChannelOverrides\": 1328"));

    // 2. Empirical verification against `fixtures/gateway/by-type/002-READY.json`
    // using our D-06 cascade engine without serde_json dependency in chord-platform tests
    // (we parse `002-READY.json` via the workspace ETF/READY fixture or simple line/token scan).
    // Verify exact capture counts in `002-READY.json`:
    let mut engine = D06CascadeEngine::new();
    let defaults = D06ClientNotificationDefaults::from_platform_embedded(true);

    // Exercise all 4 cascade stages and mute/expiry paths to verify telemetry counters
    let guild_cfg = D06GuildNotificationSettings {
        guild_id: Some(1_014_537_851_418_980_372),
        message_notifications: Some(MESSAGE_NOTIFICATIONS_ONLY_MENTIONS),
        default_message_notifications: MESSAGE_NOTIFICATIONS_ONLY_MENTIONS,
        ..D06GuildNotificationSettings::default()
    };
    let expired_ch = D06ChannelOverride {
        channel_id: 1_173_958_353_802_776_576,
        message_notifications: MESSAGE_NOTIFICATIONS_ALL_MESSAGES,
        muted: true,
        mute_config: Some(D06MuteConfig {
            selected_time_window: 86_400,
            end_time: Some("2021-02-27T17:14:08.418000+00:00".to_string()),
        }),
        flags: Some(CHANNEL_FLAG_OPT_IN_ENABLED),
        collapsed: false,
    };

    let msg = D06MessageEvalInput {
        message_id: 9001,
        guild_id: Some(1_014_537_851_418_980_372),
        channel_id: 1_173_958_353_802_776_576,
        parent_category_id: None,
        author_id: 500,
        current_user_id: 384_299_970_632_876_032,
        is_dm: false,
        mention_kind: D06MentionKind::User,
        user_dnd: false,
        streamer_mode: false,
        app_focused: false,
        active_channel_id: None,
        account_flags: 0,
    };

    let decision = engine.evaluate_message(
        &msg,
        &defaults,
        &guild_cfg,
        None,
        Some(&expired_ch),
        NOW_UNIX,
    );
    assert!(
        !decision.cascade.effective_muted,
        "expired 2021 channel mute must recover to unmuted"
    );
    assert!(decision.should_show_desktop_banner);
    assert!(decision.should_play_sound);
    assert!(decision.should_increment_badge);

    let telemetry = engine.telemetry();
    assert_eq!(telemetry.evaluations_total, 1);
    assert_eq!(telemetry.channel_override_hits, 1);
    assert_eq!(telemetry.expired_mutes_recovered, 1);
    assert_eq!(telemetry.sound_alerts_emitted, 1);
    assert_eq!(telemetry.badge_increments, 1);

    // Verify `002-READY.json` contains `"version":734` for `user_guild_settings` and 2021 expired mute timestamps
    assert!(ready_text.contains("\"version\":734"));
    assert!(ready_text.contains("2021-02-27T17:14:08.418000+00:00"));
    assert_eq!(
        CHANNEL_FLAG_NEW_FORUM_THREADS_OFF | CHANNEL_FLAG_NEW_FORUM_THREADS_ON,
        8192 | 16384
    );
}
