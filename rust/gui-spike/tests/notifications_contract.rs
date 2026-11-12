//! RW-40 / N-05-NOTIFICATIONS-BACKEND: контрактные тесты настроек уведомлений гильдий
//! и переопределений каналов (`PATCH /users/@me/guilds/{guild_id}/settings`),
//! каскадного резолвера `resolve_effective_notifications` и транспорта `NotificationTransport`.
//!
//! Оракулы:
//! - `waves/RW-40/NOTIFICATIONS-RE.md` (§1–§12)
//! - `waves/RW-40/DESIGN/NOTIFICATIONS-SPEC.md` (§1.1–§1.6)
//! - `fixtures/rest/rest-static.json` (`USER_GUILD_SETTINGS`, `USER_GUILD_SETTINGS_BULK`)
//! - `fixtures/gateway/by-type/002-READY.json` (`d.user_guild_settings.entries[]`)

use std::collections::BTreeMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use discord_core::rest::transport::HttpResponse;
use chord_client::notifications::{
    build_guild_settings_endpoint, clear_channel_unread_flags, clear_guild_unread_flags,
    format_iso8601_utc, is_entry_muted, is_temporarily_muted, parse_iso8601_unix,
    resolve_effective_notifications, set_channel_unread_flag, set_guild_unread_flag,
    update_channel_notification_override, update_guild_notification_settings,
    ChannelNotificationOverride, ChannelNotificationOverridePatch, DiscordNotifications,
    GuildNotificationPolicyInput, GuildNotificationSettingsPatch, MentionKind,
    MessageNotificationLevel, MuteConfig, NotificationError, NotificationTransport,
    NotifyHighlights, CHANNEL_FLAG_FAVORITED, CHANNEL_FLAG_OPT_IN_ENABLED,
    CHANNEL_FLAG_UNREADS_ALL_MESSAGES, CHANNEL_FLAG_UNREADS_ONLY_MENTIONS,
    GUILD_FLAG_OPT_IN_CHANNELS_ON, GUILD_FLAG_UNREADS_ALL_MESSAGES,
    GUILD_FLAG_UNREADS_ONLY_MENTIONS, MESSAGE_NOTIFICATIONS_ALL_MESSAGES,
    MESSAGE_NOTIFICATIONS_NO_MESSAGES, MESSAGE_NOTIFICATIONS_NULL,
    MESSAGE_NOTIFICATIONS_ONLY_MENTIONS, USER_GUILD_SETTINGS_BULK_PATH, VALID_MUTE_TIME_WINDOWS,
};

const GUILD_ID: &str = "1014537851418980372";
const CATEGORY_ID: &str = "1014537851418980374";
const CHANNEL_ID: &str = "1173958353802776576";
const TOKEN: &str = "test-user-token-n05";
const NOW_UNIX: i64 = 1_793_386_800; // 2026-10-30T19:00:00Z

fn run<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("tokio runtime")
        .block_on(future)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("корень репозитория")
        .to_path_buf()
}

type PatchLog = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

#[derive(Clone, Default)]
struct FakeNotificationTransport {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    patches: PatchLog,
}

impl FakeNotificationTransport {
    fn with_body(status: u16, body: &[u8]) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.to_vec(),
            patches: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn with_headers(mut self, headers: &[(&str, &str)]) -> Self {
        self.headers = headers
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self
    }

    fn patches(&self) -> Vec<(String, Vec<u8>)> {
        self.patches.lock().unwrap().clone()
    }
}

impl NotificationTransport for FakeNotificationTransport {
    fn patch<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, NotificationError>> + Send + 'a>> {
        self.patches
            .lock()
            .unwrap()
            .push((target.to_string(), body.to_vec()));
        let response = HttpResponse {
            status: self.status,
            headers: self.headers.clone(),
            body: self.body.clone(),
        };
        Box::pin(async move { Ok(response) })
    }
}

fn parse_etf_bool(val: &serde_json::Value) -> bool {
    match val {
        serde_json::Value::Bool(b) => *b,
        serde_json::Value::String(s) => s == "true",
        _ => false,
    }
}

fn parse_etf_mute_config(val: &serde_json::Value) -> Option<MuteConfig> {
    let obj = val.as_object()?;
    let selected_time_window = obj
        .get("selected_time_window")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(-1);
    let end_time = match obj.get("end_time") {
        Some(serde_json::Value::String(s)) if s != "nil" && !s.is_empty() => Some(s.clone()),
        _ => None,
    };
    Some(MuteConfig {
        selected_time_window,
        end_time,
    })
}

// ---------------------------------------------------------------------------
// 1. Сверка с реальными оракулами `fixtures/rest/rest-static.json` и `002-READY.json`
// ---------------------------------------------------------------------------

#[test]
fn oracle_ready_and_rest_static_parity_for_user_guild_settings() {
    let root = repo_root();

    // 1.1 Оракул маршрутов `fixtures/rest/rest-static.json`:
    let rest_static_raw = std::fs::read_to_string(root.join("fixtures/rest/rest-static.json"))
        .expect("fixtures/rest/rest-static.json");
    let rest_static: serde_json::Value =
        serde_json::from_str(&rest_static_raw).expect("JSON rest-static");
    let endpoints = rest_static
        .get("endpoints")
        .and_then(serde_json::Value::as_array)
        .expect("endpoints array");

    let single_ep = endpoints
        .iter()
        .find(|ep| ep.get("key").and_then(|v| v.as_str()) == Some("USER_GUILD_SETTINGS"))
        .expect("USER_GUILD_SETTINGS в rest-static.json");
    assert_eq!(
        single_ep.get("method").and_then(|v| v.as_str()),
        Some("PATCH")
    );
    assert_eq!(
        single_ep.get("path").and_then(|v| v.as_str()),
        Some("/users/@me/guilds/{arg}/settings")
    );
    assert_eq!(
        build_guild_settings_endpoint(GUILD_ID).expect("valid guild endpoint"),
        format!("/users/@me/guilds/{GUILD_ID}/settings")
    );
    assert_eq!(
        build_guild_settings_endpoint("@me").expect("valid @me endpoint"),
        "/users/@me/guilds/@me/settings"
    );

    let bulk_ep = endpoints
        .iter()
        .find(|ep| ep.get("key").and_then(|v| v.as_str()) == Some("USER_GUILD_SETTINGS_BULK"))
        .expect("USER_GUILD_SETTINGS_BULK в rest-static.json");
    assert_eq!(
        bulk_ep.get("method").and_then(|v| v.as_str()),
        Some("PATCH")
    );
    assert_eq!(
        bulk_ep.get("path").and_then(|v| v.as_str()),
        Some(USER_GUILD_SETTINGS_BULK_PATH)
    );

    // 1.2 Оракул `fixtures/gateway/by-type/002-READY.json` (NOTIFICATIONS-RE.md §12):
    let ready_raw = std::fs::read_to_string(root.join("fixtures/gateway/by-type/002-READY.json"))
        .expect("002-READY.json");
    let ready_json: serde_json::Value = serde_json::from_str(&ready_raw).expect("READY JSON");
    let entries = ready_json
        .pointer("/d/user_guild_settings/entries")
        .and_then(serde_json::Value::as_array)
        .expect("d.user_guild_settings.entries");
    assert_eq!(entries.len(), 58, "в capture ровно 58 записей гильдий");

    let mut guild_level_counts = BTreeMap::<i64, usize>::new();
    let mut channel_level_counts = BTreeMap::<i64, usize>::new();
    let mut highlights_counts = BTreeMap::<i64, usize>::new();
    let mut total_overrides = 0usize;
    let mut non_empty_override_entries = 0usize;
    let mut max_overrides_in_entry = 0usize;
    let mut raw_muted_guilds = 0usize;
    let mut muted_guilds_with_cfg = 0usize;
    let mut raw_muted_channels = 0usize;
    let mut muted_channels_with_cfg = 0usize;
    let mut expired_muted_channels = 0usize;

    for entry in entries {
        let g_level = entry
            .get("message_notifications")
            .and_then(serde_json::Value::as_i64)
            .expect("guild message_notifications");
        assert!(
            MessageNotificationLevel::from_guild_code(g_level).is_ok(),
            "уровень гильдии в capture обязан быть валидным 0|1|2: {g_level}"
        );
        *guild_level_counts.entry(g_level).or_insert(0) += 1;

        let highlights = entry
            .get("notify_highlights")
            .and_then(serde_json::Value::as_i64)
            .expect("notify_highlights");
        assert!(
            NotifyHighlights::from_code(highlights).is_ok(),
            "notify_highlights в capture обязан быть 0|1|2: {highlights}"
        );
        *highlights_counts.entry(highlights).or_insert(0) += 1;

        let g_muted = entry.get("muted").is_some_and(parse_etf_bool);
        let g_cfg = entry.get("mute_config").and_then(parse_etf_mute_config);
        if g_muted {
            raw_muted_guilds += 1;
            if g_cfg.is_some() {
                muted_guilds_with_cfg += 1;
            }
            assert!(
                is_entry_muted(g_muted, g_cfg.as_ref(), NOW_UNIX),
                "все 22 заглушённые гильдии в capture имеют бессрочный mute"
            );
        }

        let empty_overrides = Vec::new();
        let overrides = entry
            .get("channel_overrides")
            .and_then(serde_json::Value::as_array)
            .unwrap_or(&empty_overrides);
        if !overrides.is_empty() {
            non_empty_override_entries += 1;
        }
        max_overrides_in_entry = max_overrides_in_entry.max(overrides.len());
        total_overrides += overrides.len();

        for ov in overrides {
            let c_level = ov
                .get("message_notifications")
                .and_then(serde_json::Value::as_i64)
                .expect("channel message_notifications");
            assert!(
                MessageNotificationLevel::from_channel_code(c_level).is_ok(),
                "уровень канала в capture обязан быть 0|1|2|3: {c_level}"
            );
            *channel_level_counts.entry(c_level).or_insert(0) += 1;

            let c_muted = ov.get("muted").is_some_and(parse_etf_bool);
            let c_cfg = ov.get("mute_config").and_then(parse_etf_mute_config);
            if c_muted {
                raw_muted_channels += 1;
                if c_cfg.is_some() {
                    muted_channels_with_cfg += 1;
                }
                if !is_entry_muted(c_muted, c_cfg.as_ref(), NOW_UNIX) {
                    expired_muted_channels += 1;
                }
            }
        }
    }

    assert_eq!(total_overrides, 1328);
    assert_eq!(non_empty_override_entries, 52);
    assert_eq!(max_overrides_in_entry, 185);
    assert_eq!(
        guild_level_counts,
        BTreeMap::from([(0, 19), (1, 36), (2, 3)])
    );
    assert_eq!(channel_level_counts, BTreeMap::from([(1, 1), (3, 1327)]));
    assert_eq!(highlights_counts, BTreeMap::from([(0, 56), (1, 2)]));
    assert_eq!((raw_muted_guilds, muted_guilds_with_cfg), (22, 21));
    assert_eq!((raw_muted_channels, muted_channels_with_cfg), (42, 38));
    assert_eq!(
        expired_muted_channels, 3,
        "3 канала с end_time 2021 года обязаны распознаваться как истёкшие (is_muted=false)"
    );
}

// ---------------------------------------------------------------------------
// 2. Построители и валидаторы PATCH-пейлоадов гильдии и переопределений канала
// ---------------------------------------------------------------------------

#[test]
fn guild_and_channel_patch_builders_and_validators_enforce_re_contract() {
    // 2.1 Коды уровней уведомлений: гильдия 0|1|2 (3 запрещён), канал 0|1|2|3:
    assert_eq!(
        MessageNotificationLevel::from_guild_code(0),
        Ok(MessageNotificationLevel::AllMessages)
    );
    assert_eq!(
        MessageNotificationLevel::from_guild_code(1),
        Ok(MessageNotificationLevel::OnlyMentions)
    );
    assert_eq!(
        MessageNotificationLevel::from_guild_code(2),
        Ok(MessageNotificationLevel::NoMessages)
    );
    assert_eq!(
        MessageNotificationLevel::from_guild_code(3),
        Err(NotificationError::InvalidMessageNotifications { value: 3 })
    );
    assert_eq!(
        MessageNotificationLevel::from_guild_code(-1),
        Err(NotificationError::InvalidMessageNotifications { value: -1 })
    );

    assert_eq!(
        MessageNotificationLevel::from_channel_code(3),
        Ok(MessageNotificationLevel::Inherit)
    );
    assert_eq!(
        MessageNotificationLevel::from_channel_code(4),
        Err(NotificationError::InvalidMessageNotifications { value: 4 })
    );

    // 2.2 6 канонических окон mute_config: 900, 3600, 10800, 28800, 86400, -1:
    for window in VALID_MUTE_TIME_WINDOWS {
        let cfg = MuteConfig::from_window(window, NOW_UNIX).expect("valid mute window");
        assert_eq!(cfg.selected_time_window, window);
        assert_eq!(cfg.validate(), Ok(()));
        if window == -1 {
            assert_eq!(cfg.end_time, None);
        } else {
            let end_iso = cfg.end_time.as_deref().expect("ISO-8601 end_time");
            assert_eq!(parse_iso8601_unix(end_iso), Some(NOW_UNIX + window));
        }
    }

    for bad_window in [0, 60, 1800, 7200, -2, 100_000] {
        assert_eq!(
            MuteConfig::from_window(bad_window, NOW_UNIX),
            Err(NotificationError::InvalidMuteConfig),
            "окно {bad_window} не входит в каноническую шестёрку RE"
        );
    }

    // Несогласованный end_time: для -1 должен быть None, для >0 должен быть валидный ISO-8601:
    let bad_forever = MuteConfig {
        selected_time_window: -1,
        end_time: Some("2026-10-30T20:00:00Z".to_string()),
    };
    assert_eq!(
        bad_forever.validate(),
        Err(NotificationError::InvalidMuteConfig)
    );
    let bad_timed_null = MuteConfig {
        selected_time_window: 3600,
        end_time: None,
    };
    assert_eq!(
        bad_timed_null.validate(),
        Err(NotificationError::InvalidMuteConfig)
    );
    let bad_timed_iso = MuteConfig {
        selected_time_window: 3600,
        end_time: Some("not-an-iso-date".to_string()),
    };
    assert_eq!(
        bad_timed_iso.validate(),
        Err(NotificationError::InvalidMuteConfig)
    );

    // 2.3 Битовые маски flags гильдии и канала (mD / PW из @11923745):
    let guild_initial = GUILD_FLAG_OPT_IN_CHANNELS_ON | GUILD_FLAG_UNREADS_ALL_MESSAGES;
    let guild_switched = set_guild_unread_flag(guild_initial, GUILD_FLAG_UNREADS_ONLY_MENTIONS);
    assert_eq!(
        guild_switched,
        GUILD_FLAG_OPT_IN_CHANNELS_ON | GUILD_FLAG_UNREADS_ONLY_MENTIONS,
        "смена бита UNREADS гильдии не должна затирать OPT_IN_CHANNELS_ON (16384)"
    );
    assert_eq!(
        clear_guild_unread_flags(guild_switched),
        GUILD_FLAG_OPT_IN_CHANNELS_ON
    );

    let ch_initial =
        CHANNEL_FLAG_OPT_IN_ENABLED | CHANNEL_FLAG_FAVORITED | CHANNEL_FLAG_UNREADS_ALL_MESSAGES;
    let ch_switched = set_channel_unread_flag(ch_initial, CHANNEL_FLAG_UNREADS_ONLY_MENTIONS);
    assert_eq!(
        ch_switched,
        CHANNEL_FLAG_OPT_IN_ENABLED | CHANNEL_FLAG_FAVORITED | CHANNEL_FLAG_UNREADS_ONLY_MENTIONS,
        "смена бита UNREADS канала обязана сохранять OPT_IN_ENABLED (4096) и FAVORITED (2048)"
    );
    assert_eq!(
        clear_channel_unread_flags(ch_switched),
        CHANNEL_FLAG_OPT_IN_ENABLED | CHANNEL_FLAG_FAVORITED
    );

    // 2.4 Полный PATCH гильдии + channel_overrides map:
    let ch_override = ChannelNotificationOverridePatch::new()
        .with_message_notifications(MESSAGE_NOTIFICATIONS_ONLY_MENTIONS)
        .with_muted(true)
        .with_mute_window(3600, NOW_UNIX)
        .expect("valid channel mute window")
        .with_flags(ch_switched);

    let guild_patch = GuildNotificationSettingsPatch::new()
        .with_message_notifications(MESSAGE_NOTIFICATIONS_ALL_MESSAGES)
        .with_muted(true)
        .with_mute_window(900, NOW_UNIX)
        .expect("valid guild mute window")
        .with_suppress_everyone(true)
        .with_suppress_roles(false)
        .with_mobile_push(false)
        .with_mute_scheduled_events(true)
        .with_flags(guild_switched)
        .with_notify_highlights(2)
        .with_channel_override(CHANNEL_ID, ch_override);

    let patch_json = guild_patch
        .to_patch_value()
        .expect("valid guild patch JSON");
    assert_eq!(
        patch_json,
        serde_json::json!({
            "message_notifications": 0,
            "muted": true,
            "mute_config": {
                "selected_time_window": 900,
                "end_time": format_iso8601_utc(NOW_UNIX + 900),
            },
            "suppress_everyone": true,
            "suppress_roles": false,
            "mobile_push": false,
            "mute_scheduled_events": true,
            "flags": guild_switched,
            "notify_highlights": 2,
            "channel_overrides": {
                CHANNEL_ID: {
                    "message_notifications": 1,
                    "muted": true,
                    "mute_config": {
                        "selected_time_window": 3600,
                        "end_time": format_iso8601_utc(NOW_UNIX + 3600),
                    },
                    "flags": ch_switched,
                }
            }
        })
    );

    let bulk_json = guild_patch
        .to_bulk_patch_value(GUILD_ID)
        .expect("valid bulk patch JSON");
    assert_eq!(
        bulk_json,
        serde_json::json!({
            "guilds": {
                GUILD_ID: patch_json
            }
        })
    );

    // Снятие mute (`with_unmute`) явно сериализует `"muted": false, "mute_config": null`:
    let unmute_json = GuildNotificationSettingsPatch::new()
        .with_unmute()
        .to_patch_value()
        .expect("unmute patch");
    assert_eq!(
        unmute_json,
        serde_json::json!({
            "muted": false,
            "mute_config": null
        })
    );

    // Сброс override канала (`reset_override`, NOTIFICATIONS-SPEC.md §1.4 п.3):
    let reset_patch = ChannelNotificationOverridePatch::reset_override(Some(ch_switched));
    assert_eq!(
        reset_patch.to_patch_value().expect("reset override JSON"),
        serde_json::json!({
            "message_notifications": 3,
            "muted": false,
            "flags": CHANNEL_FLAG_OPT_IN_ENABLED | CHANNEL_FLAG_FAVORITED
        })
    );

    // Пустой PATCH отвергается:
    assert_eq!(
        GuildNotificationSettingsPatch::new().validate(),
        Err(NotificationError::EmptyPatch)
    );
    assert_eq!(
        ChannelNotificationOverridePatch::new().validate(),
        Err(NotificationError::EmptyPatch)
    );
}

// ---------------------------------------------------------------------------
// 3. Каскадный резолвер `resolve_effective_notifications`
// ---------------------------------------------------------------------------

#[test]
fn cascade_resolver_resolves_hierarchy_mute_expiry_and_mention_policy() {
    let base_guild = GuildNotificationPolicyInput {
        guild_id: Some(GUILD_ID.to_string()),
        message_notifications: MESSAGE_NOTIFICATIONS_ONLY_MENTIONS, // 1
        default_message_notifications: MESSAGE_NOTIFICATIONS_ONLY_MENTIONS,
        muted: false,
        mute_config: None,
        suppress_everyone: false,
        suppress_roles: false,
        mobile_push: true,
        mute_scheduled_events: false,
        notify_highlights: 0,
        hide_muted_channels: false,
        flags: 0,
    };

    // 3.1 Каскад уровня: канал (0|1|2) > категория (0|1|2) > гильдия (0|1|2):
    let cat_all = ChannelNotificationOverride {
        channel_id: CATEGORY_ID.to_string(),
        message_notifications: MESSAGE_NOTIFICATIONS_ALL_MESSAGES, // 0
        muted: false,
        mute_config: None,
        flags: Some(CHANNEL_FLAG_OPT_IN_ENABLED),
        collapsed: false,
    };
    let ch_inherit = ChannelNotificationOverride {
        channel_id: CHANNEL_ID.to_string(),
        message_notifications: MESSAGE_NOTIFICATIONS_NULL, // 3 = наследование
        muted: false,
        mute_config: None,
        flags: Some(CHANNEL_FLAG_OPT_IN_ENABLED),
        collapsed: false,
    };

    // Канал = 3 (NULL), категория = 0 (ALL_MESSAGES) -> эффективный уровень = 0:
    let policy_from_cat = chord_client::resolve_effective_notifications(
        &base_guild,
        Some(&cat_all),
        Some(&ch_inherit),
        NOW_UNIX,
    );
    assert_eq!(
        policy_from_cat.effective_level,
        MESSAGE_NOTIFICATIONS_ALL_MESSAGES
    );
    assert!(!policy_from_cat.is_muted);
    assert!(!policy_from_cat.allow_no_messages);
    assert!(policy_from_cat.should_alert_regular_message());
    assert!(policy_from_cat.should_alert_mention(MentionKind::None));
    assert!(!policy_from_cat.should_increment_badge(MentionKind::None));

    // Канал явно задаёт 2 (NO_MESSAGES) поверх категории 0 -> эффективный уровень = 2, allow_no_messages = true:
    let ch_no_messages = ChannelNotificationOverride {
        message_notifications: MESSAGE_NOTIFICATIONS_NO_MESSAGES, // 2
        ..ch_inherit.clone()
    };
    let policy_from_ch = resolve_effective_notifications(
        &base_guild,
        Some(&cat_all),
        Some(&ch_no_messages),
        NOW_UNIX,
    );
    assert_eq!(
        policy_from_ch.effective_level,
        MESSAGE_NOTIFICATIONS_NO_MESSAGES
    );
    assert!(!policy_from_ch.is_muted);
    assert!(
        policy_from_ch.allow_no_messages,
        "при effective_level == 2 allow_no_messages обязан быть true даже без mute"
    );
    assert!(!policy_from_ch.should_alert_mention(MentionKind::User));
    assert!(!policy_from_ch.should_alert_mention(MentionKind::Role));
    assert!(!policy_from_ch.should_alert_mention(MentionKind::Everyone));
    assert!(!policy_from_ch.should_alert_mention(MentionKind::Here));
    // Но бейджи при NO_MESSAGES (и !suppress_*) продолжают увеличиваться!
    assert!(policy_from_ch.should_increment_badge(MentionKind::User));
    assert!(policy_from_ch.should_increment_badge(MentionKind::Role));
    assert!(policy_from_ch.should_increment_badge(MentionKind::Everyone));
    assert!(policy_from_ch.should_increment_badge(MentionKind::Here));

    // Канал = 3, категория = 3 -> наследует гильдию (1 = ONLY_MENTIONS):
    let cat_inherit = ChannelNotificationOverride {
        message_notifications: MESSAGE_NOTIFICATIONS_NULL,
        ..cat_all.clone()
    };
    let policy_from_guild = resolve_effective_notifications(
        &base_guild,
        Some(&cat_inherit),
        Some(&ch_inherit),
        NOW_UNIX,
    );
    assert_eq!(
        policy_from_guild.effective_level,
        MESSAGE_NOTIFICATIONS_ONLY_MENTIONS
    );
    assert!(!policy_from_guild.allow_no_messages);
    assert!(!policy_from_guild.should_alert_regular_message());
    assert!(policy_from_guild.should_alert_mention(MentionKind::User));
    assert!(policy_from_guild.should_alert_mention(MentionKind::Role));
    assert!(policy_from_guild.should_alert_mention(MentionKind::Everyone));

    // 3.2 Таймер mute_config: активный vs истёкший vs бессрочный:
    let active_mute_cfg = MuteConfig {
        selected_time_window: 3600,
        end_time: Some("2026-10-30T20:00:00.000Z".to_string()), // NOW_UNIX + 3600
    };
    let expired_mute_cfg = MuteConfig {
        selected_time_window: 3600,
        end_time: Some("2021-02-27T17:14:08.418000+00:00".to_string()), // в прошлом
    };
    assert!(is_temporarily_muted(true, Some(&active_mute_cfg), NOW_UNIX));
    assert!(!is_temporarily_muted(
        true,
        Some(&expired_mute_cfg),
        NOW_UNIX
    ));
    assert!(!is_temporarily_muted(true, None, NOW_UNIX));
    assert!(is_entry_muted(true, None, NOW_UNIX));

    let cat_muted_active = ChannelNotificationOverride {
        muted: true,
        mute_config: Some(active_mute_cfg),
        ..cat_inherit.clone()
    };
    let policy_cat_muted = resolve_effective_notifications(
        &base_guild,
        Some(&cat_muted_active),
        Some(&ch_inherit),
        NOW_UNIX,
    );
    assert!(policy_cat_muted.category_muted);
    assert!(policy_cat_muted.is_muted);
    assert!(policy_cat_muted.allow_no_messages);
    assert!(!policy_cat_muted.should_alert_mention(MentionKind::User));
    assert!(
        policy_cat_muted.should_increment_badge(MentionKind::User),
        "прямое @user упоминание всегда увеличивает бейдж даже при mute"
    );

    let ch_muted_expired = ChannelNotificationOverride {
        muted: true,
        mute_config: Some(expired_mute_cfg),
        ..ch_inherit.clone()
    };
    let policy_expired = resolve_effective_notifications(
        &base_guild,
        Some(&cat_inherit),
        Some(&ch_muted_expired),
        NOW_UNIX,
    );
    assert!(!policy_expired.channel_muted);
    assert!(!policy_expired.is_muted);
    assert!(!policy_expired.allow_no_messages);
    assert!(policy_expired.should_alert_mention(MentionKind::User));

    // 3.3 Подавление @everyone/@here и @role:
    let guild_suppressed = GuildNotificationPolicyInput {
        suppress_everyone: true,
        suppress_roles: true,
        ..base_guild.clone()
    };
    let policy_suppressed =
        resolve_effective_notifications(&guild_suppressed, None, Some(&ch_inherit), NOW_UNIX);
    assert!(policy_suppressed.should_alert_mention(MentionKind::User));
    assert!(policy_suppressed.should_increment_badge(MentionKind::User));
    assert!(!policy_suppressed.should_alert_mention(MentionKind::Role));
    assert!(!policy_suppressed.should_increment_badge(MentionKind::Role));
    assert!(!policy_suppressed.should_alert_mention(MentionKind::Everyone));
    assert!(!policy_suppressed.should_increment_badge(MentionKind::Everyone));
    assert!(!policy_suppressed.should_alert_mention(MentionKind::Here));
    assert!(!policy_suppressed.should_increment_badge(MentionKind::Here));
}

// ---------------------------------------------------------------------------
// 4. Транспорт `NotificationTransport`, предсетевая валидация и коды ответов REST
// ---------------------------------------------------------------------------

#[test]
fn transport_functions_reject_invalid_inputs_before_network_and_send_exact_patch_payloads() {
    let valid_guild_patch = GuildNotificationSettingsPatch::new()
        .with_message_notifications(MESSAGE_NOTIFICATIONS_ONLY_MENTIONS)
        .with_suppress_everyone(true);
    let valid_channel_patch = ChannelNotificationOverridePatch::new()
        .with_message_notifications(MESSAGE_NOTIFICATIONS_NO_MESSAGES)
        .with_muted(false);

    // 4.1 Предсетевая защита: пустой токен, пустой/небезопасный guild_id, пустой/небезопасный channel_id,
    // невалидный message_notifications — 0 вызовов транспорта:
    let guard_transport = FakeNotificationTransport::with_body(200, b"{}");

    assert_eq!(
        run(chord_client::update_guild_notification_settings(
            &guard_transport,
            "   ",
            GUILD_ID,
            &valid_guild_patch,
        )),
        Err(NotificationError::MissingToken)
    );
    assert_eq!(
        run(chord_client::update_channel_notification_override(
            &guard_transport,
            "",
            GUILD_ID,
            CHANNEL_ID,
            &valid_channel_patch,
        )),
        Err(NotificationError::MissingToken)
    );
    assert_eq!(
        DiscordNotifications::new("   ").unwrap_err(),
        NotificationError::MissingToken
    );

    for bad_guild in ["", "   ", "../bad", "a/b", "a.b", "guild id"] {
        assert_eq!(
            run(update_guild_notification_settings(
                &guard_transport,
                TOKEN,
                bad_guild,
                &valid_guild_patch,
            )),
            Err(NotificationError::InvalidGuildId),
            "bad guild_id={bad_guild:?}"
        );
        assert_eq!(
            run(update_channel_notification_override(
                &guard_transport,
                TOKEN,
                bad_guild,
                CHANNEL_ID,
                &valid_channel_patch,
            )),
            Err(NotificationError::InvalidGuildId),
            "bad guild_id={bad_guild:?}"
        );
    }

    for bad_channel in ["", "   ", "../bad", "a/b", "a.b", "chan id", "@me"] {
        assert_eq!(
            run(update_channel_notification_override(
                &guard_transport,
                TOKEN,
                GUILD_ID,
                bad_channel,
                &valid_channel_patch,
            )),
            Err(NotificationError::InvalidChannelId),
            "bad channel_id={bad_channel:?}"
        );
    }

    // Невалидный message_notifications (3 для гильдии, 4 для канала):
    let invalid_guild_level = GuildNotificationSettingsPatch::new().with_message_notifications(3);
    assert_eq!(
        run(update_guild_notification_settings(
            &guard_transport,
            TOKEN,
            GUILD_ID,
            &invalid_guild_level,
        )),
        Err(NotificationError::InvalidMessageNotifications { value: 3 })
    );

    let invalid_channel_level =
        ChannelNotificationOverridePatch::new().with_message_notifications(4);
    assert_eq!(
        run(update_channel_notification_override(
            &guard_transport,
            TOKEN,
            GUILD_ID,
            CHANNEL_ID,
            &invalid_channel_level,
        )),
        Err(NotificationError::InvalidMessageNotifications { value: 4 })
    );

    assert!(
        guard_transport.patches().is_empty(),
        "ни один невалидный вызов не должен доходить до NotificationTransport::patch"
    );

    // 4.2 Успешная отправка PATCH гильдии и override канала:
    let response_json =
        br#"{"guild_id":"1014537851418980372","message_notifications":1,"suppress_everyone":true}"#;
    let ok_transport = FakeNotificationTransport::with_body(200, response_json);
    let client = DiscordNotifications::with_transport(TOKEN, Arc::new(ok_transport.clone()))
        .expect("client");

    let secret_check = format!("{client:?}");
    assert!(
        !secret_check.contains(TOKEN),
        "Debug клиента не должен раскрывать токен"
    );

    let guild_resp = run(client.update_guild_notification_settings(GUILD_ID, &valid_guild_patch))
        .expect("guild settings update 200");
    assert_eq!(guild_resp["guild_id"], GUILD_ID);
    assert_eq!(guild_resp["message_notifications"], 1);

    let ch_resp = run(client.update_channel_notification_override(
        GUILD_ID,
        CHANNEL_ID,
        &valid_channel_patch,
    ))
    .expect("channel override update 200");
    assert_eq!(ch_resp["guild_id"], GUILD_ID);

    let patches = ok_transport.patches();
    assert_eq!(patches.len(), 2);
    assert_eq!(
        patches[0].0,
        format!("/users/@me/guilds/{GUILD_ID}/settings")
    );
    let sent_guild_body: serde_json::Value =
        serde_json::from_slice(&patches[0].1).expect("sent guild json");
    assert_eq!(
        sent_guild_body,
        serde_json::json!({
            "message_notifications": 1,
            "suppress_everyone": true
        })
    );

    assert_eq!(
        patches[1].0,
        format!("/users/@me/guilds/{GUILD_ID}/settings")
    );
    let sent_ch_body: serde_json::Value =
        serde_json::from_slice(&patches[1].1).expect("sent channel override json");
    assert_eq!(
        sent_ch_body,
        serde_json::json!({
            "channel_overrides": {
                CHANNEL_ID: {
                    "message_notifications": 2,
                    "muted": false
                }
            }
        })
    );

    // 4.3 Маппинг HTTP-ошибок (401, 403, 404, 429 с Retry-After, 500):
    let rl_transport =
        FakeNotificationTransport::with_body(429, b"{}").with_headers(&[("Retry-After", "1.5")]);
    assert_eq!(
        run(update_guild_notification_settings(
            &rl_transport,
            TOKEN,
            GUILD_ID,
            &valid_guild_patch,
        )),
        Err(NotificationError::RateLimited {
            retry_after_ms: 1500
        })
    );

    let unauth_transport = FakeNotificationTransport::with_body(401, b"{}");
    assert_eq!(
        run(update_channel_notification_override(
            &unauth_transport,
            TOKEN,
            GUILD_ID,
            CHANNEL_ID,
            &valid_channel_patch,
        )),
        Err(NotificationError::Unauthorized)
    );

    let forbidden_transport = FakeNotificationTransport::with_body(403, b"{}");
    assert_eq!(
        run(update_guild_notification_settings(
            &forbidden_transport,
            TOKEN,
            GUILD_ID,
            &valid_guild_patch,
        )),
        Err(NotificationError::Forbidden)
    );
}

#[test]
fn desktop_notification_dbus_integration_builds_and_dispatches_without_panic() {
    let res = chord_client::notifications::send_desktop_notification(
        "Discord",
        "Тестовое уведомление чата",
        Some("im.received"),
    );
    match res {
        Ok(id) => println!("Уведомление успешно отправлено по D-Bus, id: {id}"),
        Err(err) => {
            assert!(
                err.contains("D-Bus") || err.contains("connect"),
                "ошибка обязана быть понятной и относиться к D-Bus: {err}"
            );
        }
    }
}

#[test]
fn desktop_notification_fails_cleanly_when_address_is_invalid() {
    // Проверка устойчивости: при пустом/битом адресе не должно быть паники
    let bad_parse = discord_platform::dbus::BusAddress::parse("invalid-bus-address");
    assert!(bad_parse.is_none(), "невалидный адрес парсится в None");
}
