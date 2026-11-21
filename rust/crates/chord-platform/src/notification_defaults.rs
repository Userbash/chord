//! Task D-06: Notification Defaults Protocol (`docs/re/notification-defaults-d06.md`).
//!
//! Implements the clean-room Notification Engine protocol specification:
//! 1. Client decision heuristics for default notification states (`isPlatformEmbedded`,
//!    `NotificationSettingsStore` defaults, large guild `member_count >= 2500` threshold).
//! 2. Server-level vs category-level vs channel-level 4-stage notification cascades
//!    (`ChannelOverride != 3 -> CategoryOverride != 3 -> GuildSettings -> GuildDefault`).
//! 3. Hierarchical mute inheritance (`guild_muted || category_muted || channel_muted`),
//!    6 canonical `mute_config.selected_time_window` presets (`900, 3600, 10800, 28800, 86400, -1`),
//!    temporal ISO-8601 expiration (`isMuted` vs `isTemporarilyMuted`), and opt-in channel suppression.
//! 4. Mention sound rules (`SoundCue::Mention` / `"mention"` vs `"message1"`), D-Bus urgency
//!    mapping (`Critical` for DM, `Normal` for mentions, `Low` for regular `ALL_MESSAGES`),
//!    `.mention-badge` increment matrix, and protective partial `USER_GUILD_SETTINGS_UPDATE` merge.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt;

use crate::audio_engine::SoundCue;
use crate::notifications::Urgency;

/// Wire code `ALL_MESSAGES = 0`: alert on every message (`web.4d4b2f75fdea5470.js@313815`, export `orn`).
pub const MESSAGE_NOTIFICATIONS_ALL_MESSAGES: i64 = 0;
/// Wire code `ONLY_MENTIONS = 1`: alert only on mentions (`@313815`).
pub const MESSAGE_NOTIFICATIONS_ONLY_MENTIONS: i64 = 1;
/// Wire code `NO_MESSAGES = 2`: suppress sound and desktop alerts (`@313815`).
pub const MESSAGE_NOTIFICATIONS_NO_MESSAGES: i64 = 2;
/// Wire code `NULL = 3`: inherit parent category / server level (valid only for channel/category overrides, `@313815`).
pub const MESSAGE_NOTIFICATIONS_NULL: i64 = 3;

/// Valid `message_notifications` wire codes at guild scope (`0 | 1 | 2`).
pub const VALID_GUILD_MESSAGE_NOTIFICATIONS: [i64; 3] = [
    MESSAGE_NOTIFICATIONS_ALL_MESSAGES,
    MESSAGE_NOTIFICATIONS_ONLY_MENTIONS,
    MESSAGE_NOTIFICATIONS_NO_MESSAGES,
];

/// Valid `message_notifications` wire codes at channel/category override scope (`0 | 1 | 2 | 3`).
pub const VALID_CHANNEL_MESSAGE_NOTIFICATIONS: [i64; 4] = [
    MESSAGE_NOTIFICATIONS_ALL_MESSAGES,
    MESSAGE_NOTIFICATIONS_ONLY_MENTIONS,
    MESSAGE_NOTIFICATIONS_NO_MESSAGES,
    MESSAGE_NOTIFICATIONS_NULL,
];

/// Canonical `mute_config.selected_time_window` presets in seconds (`@8349873` export `Xx`, `@6225743` `b8()`).
pub const VALID_MUTE_TIME_WINDOWS: [i64; 6] = [900, 3600, 10_800, 28_800, 86_400, -1];

/// Member count threshold (`2500`) above which guilds default to `ONLY_MENTIONS` (`i18n` key `Dh5p5j`).
pub const LARGE_GUILD_MEMBER_THRESHOLD: u64 = 2_500;

/// Guild flag: mark all messages as unread (`1 << 11 = 2048`, `@8352891`).
pub const GUILD_FLAG_UNREADS_ALL_MESSAGES: i64 = 2048;
/// Guild flag: mark only mentions as unread (`1 << 12 = 4096`, `@8352891`).
pub const GUILD_FLAG_UNREADS_ONLY_MENTIONS: i64 = 4096;
/// Guild flag: opt-in channels mode explicitly off (`1 << 13 = 8192`, `@8352891`).
pub const GUILD_FLAG_OPT_IN_CHANNELS_OFF: i64 = 8192;
/// Guild flag: opt-in channels mode enabled (`1 << 14 = 16384`, `@8352891`).
pub const GUILD_FLAG_OPT_IN_CHANNELS_ON: i64 = 16384;
/// Bitmask covering both guild unread flags (`2048 | 4096 = 6144`).
pub const GUILD_UNREAD_MASK: i64 =
    GUILD_FLAG_UNREADS_ALL_MESSAGES | GUILD_FLAG_UNREADS_ONLY_MENTIONS;

/// Channel flag: unread indicator only on mentions (`1 << 9 = 512`, `@8352746`).
pub const CHANNEL_FLAG_UNREADS_ONLY_MENTIONS: i64 = 512;
/// Channel flag: unread indicator on all messages (`1 << 10 = 1024`, `@8352746`).
pub const CHANNEL_FLAG_UNREADS_ALL_MESSAGES: i64 = 1024;
/// Channel flag: channel is favorited (`1 << 11 = 2048`, `@8352746`).
pub const CHANNEL_FLAG_FAVORITED: i64 = 2048;
/// Channel flag: channel is selected in opt-in channels list (`1 << 12 = 4096`, `@8352746`).
pub const CHANNEL_FLAG_OPT_IN_ENABLED: i64 = 4096;
/// Channel flag: new forum thread notifications off (`1 << 13 = 8192`, `@8352746`).
pub const CHANNEL_FLAG_NEW_FORUM_THREADS_OFF: i64 = 8192;
/// Channel flag: new forum thread notifications on (`1 << 14 = 16384`, `@8352746`).
pub const CHANNEL_FLAG_NEW_FORUM_THREADS_ON: i64 = 16384;
/// Bitmask covering both channel unread flags (`512 | 1024 = 1536`).
pub const CHANNEL_UNREAD_MASK: i64 =
    CHANNEL_FLAG_UNREADS_ONLY_MENTIONS | CHANNEL_FLAG_UNREADS_ALL_MESSAGES;

/// Global account notification flag: use new 3-option notification modal (`1 << 4 = 16`, `@6216165`).
pub const ACCOUNT_FLAG_USE_NEW_NOTIFICATIONS: i64 = 16;
/// Global account notification flag: mention on all messages (`1 << 5 = 32`, `@6216165`).
pub const ACCOUNT_FLAG_MENTION_ON_ALL_MESSAGES: i64 = 32;

/// Typed errors for the D-06 Notification Defaults Protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum D06NotificationProtocolError {
    /// Invalid guild notification level (`must be 0, 1, or 2`; `3 / NULL` is forbidden at guild scope).
    InvalidGuildNotificationLevel { value: i64 },
    /// Invalid channel/category notification level (`must be 0, 1, 2, or 3`).
    InvalidChannelNotificationLevel { value: i64 },
    /// Invalid `notify_highlights` code (`must be 0, 1, or 2`).
    InvalidNotifyHighlights { value: i64 },
    /// Invalid `mute_config.selected_time_window` (`must be one of 900, 3600, 10800, 28800, 86400, -1`).
    InvalidMuteTimeWindow { seconds: i64 },
    /// `mute_config.end_time` is inconsistent with `selected_time_window` (e.g. non-null on `-1` or missing on `> 0`).
    InconsistentMuteEndTime,
    /// Malformed ISO-8601 timestamp in `mute_config.end_time`.
    InvalidIso8601Timestamp { raw: String },
}

impl fmt::Display for D06NotificationProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGuildNotificationLevel { value } => {
                write!(f, "invalid guild message_notifications level: {value}")
            }
            Self::InvalidChannelNotificationLevel { value } => {
                write!(f, "invalid channel message_notifications level: {value}")
            }
            Self::InvalidNotifyHighlights { value } => {
                write!(f, "invalid notify_highlights code: {value}")
            }
            Self::InvalidMuteTimeWindow { seconds } => {
                write!(f, "invalid mute_config time window: {seconds}s")
            }
            Self::InconsistentMuteEndTime => {
                f.write_str("inconsistent mute_config end_time for selected_time_window")
            }
            Self::InvalidIso8601Timestamp { raw } => {
                write!(f, "invalid ISO-8601 timestamp in mute_config: {raw}")
            }
        }
    }
}

impl std::error::Error for D06NotificationProtocolError {}

/// Desktop notification delivery policy in `NotificationSettingsStore` (`@243859`, module `652215`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum D06DesktopNotificationType {
    /// Never display desktop notifications (`NEVER`).
    Never,
    /// Display desktop notifications for all eligible messages (`ALL`).
    All,
}

/// OS/Browser notification permission state (`@243967`, module `652215`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum D06NotificationPermissionState {
    /// Notifications disabled by user (`NEVER`).
    Never,
    /// Notifications authorized and active (`ENABLED`).
    Enabled,
    /// Notifications blocked by browser or OS (`BLOCKED`).
    Blocked,
}

/// Text-to-Speech (TTS) notification playback policy (`@243899`, module `652215`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum D06TtsType {
    /// Never read notifications aloud (`NEVER`).
    Never,
    /// Read notifications aloud across all channels (`ALL_CHANNELS`).
    AllChannels,
    /// Read notifications aloud only in the currently selected channel (`SELECTED_CHANNEL`).
    SelectedChannel,
}

/// Client decision heuristics for default notification states (`NotificationSettingsStore`, `@9225345`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct D06ClientNotificationDefaults {
    /// Whether the client is running inside a native embedded desktop shell (`isPlatformEmbedded`).
    pub is_platform_embedded: bool,
    /// Desktop notification mode (`ALL` when embedded, `NEVER` in unprompted browser).
    pub desktop_type: D06DesktopNotificationType,
    /// Runtime notification permission state (`ENABLED` when embedded, `BLOCKED` in unprompted browser).
    pub permission_state: D06NotificationPermissionState,
    /// Master mute switch for all client notification sounds (`disableAllSounds`).
    pub disable_all_sounds: bool,
    /// List of individual sound cue names disabled by the user (`disabledSounds[]`).
    pub disabled_sounds: Vec<String>,
    /// Text-to-speech announcement policy (`ttsType`).
    pub tts_type: D06TtsType,
    /// Whether unread counter badges on the OS dock/taskbar are disabled (`disableUnreadBadge`).
    pub disable_unread_badge: bool,
    /// Whether incoming notifications flash the OS taskbar frame (`taskbarFlash`).
    pub taskbar_flash: bool,
    /// Whether messages in the currently focused & active channel still emit notifications.
    pub notify_messages_in_selected_channel: bool,
    /// Screen downtime reminder toggle (`screenDowntimeReminder`).
    pub screen_downtime_reminder: bool,
}

impl D06ClientNotificationDefaults {
    /// Derives default client notification settings from the `isPlatformEmbedded` heuristic (`D-06`).
    #[must_use]
    pub fn from_platform_embedded(is_platform_embedded: bool) -> Self {
        if is_platform_embedded {
            Self {
                is_platform_embedded: true,
                desktop_type: D06DesktopNotificationType::All,
                permission_state: D06NotificationPermissionState::Enabled,
                disable_all_sounds: false,
                disabled_sounds: Vec::new(),
                tts_type: D06TtsType::Never,
                disable_unread_badge: false,
                taskbar_flash: true,
                notify_messages_in_selected_channel: false,
                screen_downtime_reminder: false,
            }
        } else {
            Self {
                is_platform_embedded: false,
                desktop_type: D06DesktopNotificationType::Never,
                permission_state: D06NotificationPermissionState::Blocked,
                disable_all_sounds: false,
                disabled_sounds: Vec::new(),
                tts_type: D06TtsType::Never,
                disable_unread_badge: false,
                taskbar_flash: false,
                notify_messages_in_selected_channel: false,
                screen_downtime_reminder: false,
            }
        }
    }

    /// Returns `true` if desktop banner notifications are enabled and permitted.
    #[must_use]
    pub const fn can_dispatch_desktop_banner(&self) -> bool {
        matches!(self.desktop_type, D06DesktopNotificationType::All)
            && matches!(
                self.permission_state,
                D06NotificationPermissionState::Enabled
            )
    }

    /// Returns `true` if the given sound cue name (`"mention"` or `"message1"`) is permitted to play.
    #[must_use]
    pub fn is_sound_enabled(&self, sound_name: &str) -> bool {
        if self.disable_all_sounds {
            return false;
        }
        !self
            .disabled_sounds
            .iter()
            .any(|disabled| disabled.eq_ignore_ascii_case(sound_name))
    }
}

/// Strongly-typed `message_notifications` level (`@313815`, module `652215` export `orn`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum D06NotificationLevel {
    /// `0` — `ALL_MESSAGES`.
    AllMessages = 0,
    /// `1` — `ONLY_MENTIONS`.
    OnlyMentions = 1,
    /// `2` — `NO_MESSAGES`.
    NoMessages = 2,
    /// `3` — `NULL` (inherit from parent category or guild; valid only on channel/category overrides).
    Inherit = 3,
}

impl D06NotificationLevel {
    /// Numeric wire code (`0..=3`).
    #[must_use]
    pub const fn as_i64(self) -> i64 {
        self as i64
    }

    /// Validates and converts a guild-level `message_notifications` code (`0 | 1 | 2`).
    pub fn from_guild_code(value: i64) -> Result<Self, D06NotificationProtocolError> {
        match value {
            MESSAGE_NOTIFICATIONS_ALL_MESSAGES => Ok(Self::AllMessages),
            MESSAGE_NOTIFICATIONS_ONLY_MENTIONS => Ok(Self::OnlyMentions),
            MESSAGE_NOTIFICATIONS_NO_MESSAGES => Ok(Self::NoMessages),
            other => Err(
                D06NotificationProtocolError::InvalidGuildNotificationLevel { value: other },
            ),
        }
    }

    /// Validates and converts a channel/category `message_notifications` code (`0 | 1 | 2 | 3`).
    pub fn from_channel_code(value: i64) -> Result<Self, D06NotificationProtocolError> {
        match value {
            MESSAGE_NOTIFICATIONS_ALL_MESSAGES => Ok(Self::AllMessages),
            MESSAGE_NOTIFICATIONS_ONLY_MENTIONS => Ok(Self::OnlyMentions),
            MESSAGE_NOTIFICATIONS_NO_MESSAGES => Ok(Self::NoMessages),
            MESSAGE_NOTIFICATIONS_NULL => Ok(Self::Inherit),
            other => Err(
                D06NotificationProtocolError::InvalidChannelNotificationLevel { value: other },
            ),
        }
    }
}

/// Resolves the default guild notification level given `member_count` and an optional server default (`D-06`).
///
/// Guilds with `member_count >= 2500` (`LARGE_GUILD_MEMBER_THRESHOLD`) default to `ONLY_MENTIONS` (`1`)
/// to prevent notification floods (`NOTIFICATIONS-RE.md §10`).
#[must_use]
pub fn resolve_guild_default_level_for_member_count(
    member_count: u64,
    configured_default: Option<i64>,
) -> D06NotificationLevel {
    if member_count >= LARGE_GUILD_MEMBER_THRESHOLD {
        return D06NotificationLevel::OnlyMentions;
    }
    match configured_default {
        Some(MESSAGE_NOTIFICATIONS_ALL_MESSAGES) => D06NotificationLevel::AllMessages,
        Some(MESSAGE_NOTIFICATIONS_NO_MESSAGES) => D06NotificationLevel::NoMessages,
        _ => D06NotificationLevel::OnlyMentions,
    }
}

/// Strongly-typed `notify_highlights` setting (`@529852`, module `652215` export `guM`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum D06NotifyHighlights {
    /// `0` — `NULL` (default enabled unless guild is muted).
    Null = 0,
    /// `1` — `DISABLED`.
    Disabled = 1,
    /// `2` — `ENABLED`.
    Enabled = 2,
}

impl D06NotifyHighlights {
    /// Numeric wire code (`0..=2`).
    #[must_use]
    pub const fn as_i64(self) -> i64 {
        self as i64
    }

    /// Validates and converts a `notify_highlights` wire code (`0 | 1 | 2`).
    pub fn from_code(value: i64) -> Result<Self, D06NotificationProtocolError> {
        match value {
            0 => Ok(Self::Null),
            1 => Ok(Self::Disabled),
            2 => Ok(Self::Enabled),
            other => Err(D06NotificationProtocolError::InvalidNotifyHighlights { value: other }),
        }
    }
}

/// Unread indicator setting derived from `flags` (`@7338127`, module `790782`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum D06UnreadSetting {
    /// `0` — neither unread bit is set.
    Unset = 0,
    /// `1` — `UNREADS_ALL_MESSAGES` bit is set.
    AllMessages = 1,
    /// `2` — `UNREADS_ONLY_MENTIONS` bit is set.
    OnlyMentions = 2,
}

impl D06UnreadSetting {
    /// Extracts the unread setting from a channel `flags` bitmask (`512` vs `1024`).
    #[must_use]
    pub const fn from_channel_flags(flags: Option<i64>) -> Self {
        let f = match flags {
            Some(v) => v,
            None => 0,
        };
        if (f & CHANNEL_FLAG_UNREADS_ALL_MESSAGES) != 0 {
            Self::AllMessages
        } else if (f & CHANNEL_FLAG_UNREADS_ONLY_MENTIONS) != 0 {
            Self::OnlyMentions
        } else {
            Self::Unset
        }
    }

    /// Extracts the unread setting from a guild `flags` bitmask (`2048` vs `4096`).
    #[must_use]
    pub const fn from_guild_flags(flags: i64) -> Self {
        if (flags & GUILD_FLAG_UNREADS_ALL_MESSAGES) != 0 {
            Self::AllMessages
        } else if (flags & GUILD_FLAG_UNREADS_ONLY_MENTIONS) != 0 {
            Self::OnlyMentions
        } else {
            Self::Unset
        }
    }
}

/// Four combined `(unread, message_notifications)` presets shown in the channel overrides table (`@6231609`, module `24873`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum D06NotificationPreset {
    /// `unread = ALL_MESSAGES`, `message_notifications = ALL_MESSAGES (0)`.
    AllMessages,
    /// `unread = ALL_MESSAGES`, `message_notifications = ONLY_MENTIONS (1)`.
    Hybrid,
    /// `unread in {UNSET, ONLY_MENTIONS}`, `message_notifications = ONLY_MENTIONS (1)`.
    Mentions,
    /// `unread in {UNSET, ONLY_MENTIONS}`, `message_notifications = NO_MESSAGES (2)`.
    Nothing,
    /// Any other custom combination.
    Custom,
}

impl D06NotificationPreset {
    /// Classifies a `(D06UnreadSetting, D06NotificationLevel)` pair into one of the 4 canonical presets or `Custom`.
    #[must_use]
    pub const fn classify(unread: D06UnreadSetting, level: D06NotificationLevel) -> Self {
        match (unread, level) {
            (D06UnreadSetting::AllMessages, D06NotificationLevel::AllMessages) => Self::AllMessages,
            (D06UnreadSetting::AllMessages, D06NotificationLevel::OnlyMentions) => Self::Hybrid,
            (
                D06UnreadSetting::Unset | D06UnreadSetting::OnlyMentions,
                D06NotificationLevel::OnlyMentions,
            ) => Self::Mentions,
            (
                D06UnreadSetting::Unset | D06UnreadSetting::OnlyMentions,
                D06NotificationLevel::NoMessages,
            ) => Self::Nothing,
            _ => Self::Custom,
        }
    }
}

/// Kind of mention present in an evaluated message (`D-06` mention sound & badge matrix).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum D06MentionKind {
    /// Regular message with no mentions.
    None,
    /// Direct `@user` mention targeting the current user.
    User,
    /// Role mention `<@&role_id>` matching one of the current user's roles.
    Role,
    /// `@everyone` broadcast mention.
    Everyone,
    /// `@here` online broadcast mention.
    Here,
}

/// Temporal mute timer configuration (`mute_config`, `NOTIFICATIONS-RE.md §4`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct D06MuteConfig {
    /// Selected duration window in seconds (`900 | 3600 | 10800 | 28800 | 86400 | -1`).
    pub selected_time_window: i64,
    /// ISO-8601 expiration timestamp (`None` when `selected_time_window == -1`).
    pub end_time: Option<String>,
}

impl D06MuteConfig {
    /// Builds a validated `D06MuteConfig` from one of the 6 canonical time windows and `now_unix`.
    pub fn from_window(
        selected_time_window: i64,
        now_unix: i64,
    ) -> Result<Self, D06NotificationProtocolError> {
        if !VALID_MUTE_TIME_WINDOWS.contains(&selected_time_window) {
            return Err(D06NotificationProtocolError::InvalidMuteTimeWindow {
                seconds: selected_time_window,
            });
        }
        let end_time = if selected_time_window == -1 {
            None
        } else {
            let end_unix = now_unix.checked_add(selected_time_window).ok_or(
                D06NotificationProtocolError::InvalidMuteTimeWindow {
                    seconds: selected_time_window,
                },
            )?;
            Some(format_iso8601_utc(end_unix))
        };
        Ok(Self {
            selected_time_window,
            end_time,
        })
    }

    /// Validates `selected_time_window` and `end_time` consistency.
    pub fn validate(&self) -> Result<(), D06NotificationProtocolError> {
        if !VALID_MUTE_TIME_WINDOWS.contains(&self.selected_time_window) {
            return Err(D06NotificationProtocolError::InvalidMuteTimeWindow {
                seconds: self.selected_time_window,
            });
        }
        if self.selected_time_window == -1 {
            if self.end_time.as_deref().is_some_and(|s| s != "nil") {
                return Err(D06NotificationProtocolError::InconsistentMuteEndTime);
            }
            return Ok(());
        }
        let Some(end_raw) = self.end_time.as_deref() else {
            return Err(D06NotificationProtocolError::InconsistentMuteEndTime);
        };
        if parse_iso8601_unix(end_raw).is_none() {
            return Err(D06NotificationProtocolError::InvalidIso8601Timestamp {
                raw: end_raw.to_string(),
            });
        }
        Ok(())
    }
}

/// Per-channel or per-category notification override entry (`channel_overrides[]`, 6 wire fields).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct D06ChannelOverride {
    pub channel_id: u64,
    pub message_notifications: i64,
    pub muted: bool,
    pub mute_config: Option<D06MuteConfig>,
    pub flags: Option<i64>,
    pub collapsed: bool,
}

impl Default for D06ChannelOverride {
    fn default() -> Self {
        Self {
            channel_id: 0,
            message_notifications: MESSAGE_NOTIFICATIONS_NULL,
            muted: false,
            mute_config: None,
            flags: None,
            collapsed: false,
        }
    }
}

impl D06ChannelOverride {
    /// Returns `true` if this entry carries an active notification override (`muted`, explicit level `0..=2`,
    /// or `UNREADS_*` flag bits), rather than existing solely for `collapsed: true` or `OPT_IN_ENABLED: 4096`.
    #[must_use]
    pub fn has_notification_override(&self, now_unix: i64) -> bool {
        is_entry_muted(self.muted, self.mute_config.as_ref(), now_unix)
            || VALID_GUILD_MESSAGE_NOTIFICATIONS.contains(&self.message_notifications)
            || (self.flags.unwrap_or(0) & CHANNEL_UNREAD_MASK) != 0
    }

    /// Non-destructively resets notification overrides (`{ message_notifications: 3, muted: false, flags: PW(flags) }`,
    /// `@1622`, `@6230633`) while preserving `collapsed`, `FAVORITED`, and `OPT_IN_ENABLED`.
    #[must_use]
    pub fn reset_notification_override(&self) -> Self {
        Self {
            channel_id: self.channel_id,
            message_notifications: MESSAGE_NOTIFICATIONS_NULL,
            muted: false,
            mute_config: None,
            flags: self.flags.map(clear_channel_unread_flags),
            collapsed: self.collapsed,
        }
    }
}

/// Per-guild notification settings entry (`d.user_guild_settings.entries[]`, 13 wire fields).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct D06GuildNotificationSettings {
    /// Guild Snowflake ID (`None` for DM settings `@me`).
    pub guild_id: Option<u64>,
    /// Explicit user-configured guild level (`Some(0 | 1 | 2)`), or `None` to inherit `default_message_notifications`.
    pub message_notifications: Option<i64>,
    /// Server-defined default notification level (`guild.default_message_notifications`, `0` or `1`).
    pub default_message_notifications: i64,
    /// Raw guild `muted` flag.
    pub muted: bool,
    /// Guild mute timer configuration.
    pub mute_config: Option<D06MuteConfig>,
    /// Suppress `@everyone` and `@here` mentions.
    pub suppress_everyone: bool,
    /// Suppress all `@role` mentions.
    pub suppress_roles: bool,
    /// Enable mobile push notifications.
    pub mobile_push: bool,
    /// Mute scheduled server events.
    pub mute_scheduled_events: bool,
    /// Highlight notification setting (`0 = NULL`, `1 = DISABLED`, `2 = ENABLED`).
    pub notify_highlights: i64,
    /// Hide muted channels in the channel sidebar.
    pub hide_muted_channels: bool,
    /// Guild notification flags bitmask (`2048 | 4096 | 8192 | 16384`).
    pub flags: i64,
    /// Monotonic entry version (`last-wins`).
    pub version: u64,
    /// Normalized map of channel and category overrides keyed by `channel_id`.
    pub channel_overrides: BTreeMap<u64, D06ChannelOverride>,
}

impl Default for D06GuildNotificationSettings {
    fn default() -> Self {
        Self {
            guild_id: None,
            message_notifications: Some(MESSAGE_NOTIFICATIONS_ONLY_MENTIONS),
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
            version: 0,
            channel_overrides: BTreeMap::new(),
        }
    }
}

/// Identifies which stage of the 4-stage cascade determined the channel's `effective_level`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CascadeLevelSource {
    /// Stage 1: Channel override (`message_notifications in {0, 1, 2}`).
    ChannelOverride,
    /// Stage 2: Parent category override (`message_notifications in {0, 1, 2}`).
    CategoryOverride,
    /// Stage 3: User's guild notification settings (`user_guild_settings.message_notifications`).
    GuildSettings,
    /// Stage 4: Server's `guild.default_message_notifications` (or fallback `ONLY_MENTIONS`).
    GuildDefault,
}

/// Resolved notification policy produced by the 4-stage server -> category -> channel cascade (`@9387678`, `@9388508`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct D06ResolvedChannelCascade {
    /// Effective notification level (`AllMessages`, `OnlyMentions`, or `NoMessages`).
    pub effective_level: D06NotificationLevel,
    /// Cascade stage that supplied `effective_level`.
    pub level_source: CascadeLevelSource,
    /// Whether the guild itself is actively muted at `now_unix`.
    pub guild_muted: bool,
    /// Whether the parent category is actively muted at `now_unix`.
    pub category_muted: bool,
    /// Whether the channel itself is actively muted at `now_unix`.
    pub channel_muted: bool,
    /// Inherited mute state (`guild_muted || category_muted || channel_muted`).
    pub effective_muted: bool,
    /// Whether an expired `mute_config.end_time < now_unix` was detected and auto-recovered to unmuted.
    pub expired_mute_recovered: bool,
    /// Whether opt-in channels mode (`OPT_IN_CHANNELS_ON = 16384`) suppresses this channel.
    pub opt_in_suppressed: bool,
    /// Master sound/desktop suppression flag (`effective_muted || effective_level == NoMessages || opt_in_suppressed`).
    pub allow_no_messages: bool,
    /// Effective unread indicator policy (`AllMessages` vs `OnlyMentions`).
    pub effective_unread: D06UnreadSetting,
    /// Whether `@everyone` and `@here` are suppressed.
    pub suppress_everyone: bool,
    /// Whether `@role` mentions are suppressed.
    pub suppress_roles: bool,
}

/// Evaluates whether an entry (`guild`, `category`, or `channel`) is actively muted at `now_unix` (`@2364848`, module `544743`):
/// `muted && (!cfg || !cfg.end_time || cfg.end_time == "nil" || end_time >= now_unix)`.
#[must_use]
pub fn is_entry_muted(muted: bool, mute_config: Option<&D06MuteConfig>, now_unix: i64) -> bool {
    if !muted {
        return false;
    }
    let Some(cfg) = mute_config else {
        return true;
    };
    let Some(end_raw) = cfg.end_time.as_deref() else {
        return true;
    };
    if end_raw.is_empty() || end_raw == "nil" {
        return true;
    }
    parse_iso8601_unix(end_raw).is_some_and(|end_unix| end_unix >= now_unix)
}

/// Evaluates whether an entry is temporarily muted with an active future timer (`isTemporarilyMuted`, `@2364848`):
/// `muted && cfg && cfg.end_time && cfg.end_time != "nil" && end_time >= now_unix`.
#[must_use]
pub fn is_temporarily_muted(
    muted: bool,
    mute_config: Option<&D06MuteConfig>,
    now_unix: i64,
) -> bool {
    if !muted {
        return false;
    }
    let Some(cfg) = mute_config else {
        return false;
    };
    let Some(end_raw) = cfg.end_time.as_deref() else {
        return false;
    };
    if end_raw.is_empty() || end_raw == "nil" {
        return false;
    }
    parse_iso8601_unix(end_raw).is_some_and(|end_unix| end_unix >= now_unix)
}

/// Resolves the 4-stage notification cascade, mute inheritance, and opt-in suppression for a channel (`D-06`).
#[must_use]
pub fn resolve_channel_notification_cascade(
    guild: &D06GuildNotificationSettings,
    parent_category: Option<&D06ChannelOverride>,
    channel: Option<&D06ChannelOverride>,
    now_unix: i64,
) -> D06ResolvedChannelCascade {
    let guild_muted = is_entry_muted(guild.muted, guild.mute_config.as_ref(), now_unix);
    let category_muted = parent_category
        .is_some_and(|cat| is_entry_muted(cat.muted, cat.mute_config.as_ref(), now_unix));
    let channel_muted =
        channel.is_some_and(|ch| is_entry_muted(ch.muted, ch.mute_config.as_ref(), now_unix));
    let effective_muted = guild_muted || category_muted || channel_muted;

    let guild_expired = guild.muted && !guild_muted;
    let cat_expired = parent_category.is_some_and(|cat| cat.muted) && !category_muted;
    let ch_expired = channel.is_some_and(|ch| ch.muted) && !channel_muted;
    let expired_mute_recovered = guild_expired || cat_expired || ch_expired;

    let (effective_level, level_source) = if let Some(ch_lvl) = channel
        .and_then(|ch| D06NotificationLevel::from_guild_code(ch.message_notifications).ok())
    {
        (ch_lvl, CascadeLevelSource::ChannelOverride)
    } else if let Some(cat_lvl) = parent_category
        .and_then(|cat| D06NotificationLevel::from_guild_code(cat.message_notifications).ok())
    {
        (cat_lvl, CascadeLevelSource::CategoryOverride)
    } else if let Some(g_lvl) = guild
        .message_notifications
        .and_then(|code| D06NotificationLevel::from_guild_code(code).ok())
    {
        (g_lvl, CascadeLevelSource::GuildSettings)
    } else if let Ok(def_lvl) =
        D06NotificationLevel::from_guild_code(guild.default_message_notifications)
    {
        (def_lvl, CascadeLevelSource::GuildDefault)
    } else {
        (
            D06NotificationLevel::OnlyMentions,
            CascadeLevelSource::GuildDefault,
        )
    };

    let opt_in_suppressed = if (guild.flags & GUILD_FLAG_OPT_IN_CHANNELS_ON) != 0 {
        let ch_opted = channel
            .and_then(|c| c.flags)
            .is_some_and(|f| (f & CHANNEL_FLAG_OPT_IN_ENABLED) != 0);
        let cat_opted = parent_category
            .and_then(|c| c.flags)
            .is_some_and(|f| (f & CHANNEL_FLAG_OPT_IN_ENABLED) != 0);
        !ch_opted && !cat_opted
    } else {
        false
    };

    let allow_no_messages = effective_muted
        || effective_level == D06NotificationLevel::NoMessages
        || opt_in_suppressed;

    let ch_unread = D06UnreadSetting::from_channel_flags(channel.and_then(|c| c.flags));
    let cat_unread = D06UnreadSetting::from_channel_flags(parent_category.and_then(|c| c.flags));
    let guild_unread = D06UnreadSetting::from_guild_flags(guild.flags);
    let effective_unread = if ch_unread != D06UnreadSetting::Unset {
        ch_unread
    } else if cat_unread != D06UnreadSetting::Unset {
        cat_unread
    } else if guild_unread != D06UnreadSetting::Unset {
        guild_unread
    } else if effective_level == D06NotificationLevel::NoMessages {
        D06UnreadSetting::OnlyMentions
    } else {
        D06UnreadSetting::AllMessages
    };

    D06ResolvedChannelCascade {
        effective_level,
        level_source,
        guild_muted,
        category_muted,
        channel_muted,
        effective_muted,
        expired_mute_recovered,
        opt_in_suppressed,
        allow_no_messages,
        effective_unread,
        suppress_everyone: guild.suppress_everyone,
        suppress_roles: guild.suppress_roles,
    }
}

/// Sets the target `UNREADS_*` bit in a guild `flags` bitmask (`mD`, `@11923745`), preserving `OPT_IN_CHANNELS_*`.
#[must_use]
pub const fn set_guild_unread_flag(flags: i64, bit: i64) -> i64 {
    (flags & !GUILD_UNREAD_MASK) | (bit & GUILD_UNREAD_MASK)
}

/// Clears both `UNREADS_*` bits in a guild `flags` bitmask (`PW`, `@11923745`).
#[must_use]
pub const fn clear_guild_unread_flags(flags: i64) -> i64 {
    flags & !GUILD_UNREAD_MASK
}

/// Sets the target `UNREADS_*` bit in a channel `flags` bitmask (`mD`, `@11923745`), preserving `FAVORITED`, `OPT_IN_ENABLED`, and `NEW_FORUM_THREADS_*`.
#[must_use]
pub const fn set_channel_unread_flag(flags: i64, bit: i64) -> i64 {
    (flags & !CHANNEL_UNREAD_MASK) | (bit & CHANNEL_UNREAD_MASK)
}

/// Clears both `UNREADS_*` bits in a channel `flags` bitmask (`PW`, `@11923745`).
#[must_use]
pub const fn clear_channel_unread_flags(flags: i64) -> i64 {
    flags & !CHANNEL_UNREAD_MASK
}

/// Protectively merges an incoming `USER_GUILD_SETTINGS_UPDATE` entry into existing guild settings (`@9391654`).
///
/// - If `incoming.version > 0 && current.version > 0 && incoming.version < current.version`,
///   rejects the stale update and returns `current` unchanged.
/// - If `replace_channel_overrides` is `false` (key absent in partial update), preserves `current.channel_overrides`.
#[must_use]
pub fn merge_partial_guild_settings(
    current: &D06GuildNotificationSettings,
    incoming: &D06GuildNotificationSettings,
    replace_channel_overrides: bool,
) -> D06GuildNotificationSettings {
    if incoming.version > 0 && current.version > 0 && incoming.version < current.version {
        return current.clone();
    }

    let channel_overrides = if replace_channel_overrides {
        incoming.channel_overrides.clone()
    } else {
        current.channel_overrides.clone()
    };

    D06GuildNotificationSettings {
        guild_id: incoming.guild_id.or(current.guild_id),
        message_notifications: incoming
            .message_notifications
            .or(current.message_notifications),
        default_message_notifications: incoming.default_message_notifications,
        muted: incoming.muted,
        mute_config: incoming.mute_config.clone(),
        suppress_everyone: incoming.suppress_everyone,
        suppress_roles: incoming.suppress_roles,
        mobile_push: incoming.mobile_push,
        mute_scheduled_events: incoming.mute_scheduled_events,
        notify_highlights: incoming.notify_highlights,
        hide_muted_channels: incoming.hide_muted_channels,
        flags: incoming.flags,
        version: incoming.version.max(current.version),
        channel_overrides,
    }
}

/// Input parameters for evaluating a single incoming message against the D-06 Notification Engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct D06MessageEvalInput {
    pub message_id: u64,
    pub guild_id: Option<u64>,
    pub channel_id: u64,
    pub parent_category_id: Option<u64>,
    pub author_id: u64,
    pub current_user_id: u64,
    pub is_dm: bool,
    pub mention_kind: D06MentionKind,
    pub user_dnd: bool,
    pub streamer_mode: bool,
    pub app_focused: bool,
    pub active_channel_id: Option<u64>,
    pub account_flags: i64,
}

/// Complete decision output for an evaluated message (`D-06`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct D06CascadeDecision {
    /// Resolved channel cascade snapshot.
    pub cascade: D06ResolvedChannelCascade,
    /// Whether a native OS desktop banner notification should be dispatched.
    pub should_show_desktop_banner: bool,
    /// Whether an audio notification cue should be played.
    pub should_play_sound: bool,
    /// Strongly-typed audio engine sound cue (`Some(SoundCue::Mention)` when `should_play_sound == true`).
    pub sound_cue: Option<SoundCue>,
    /// Canonical sound effect identifier (`"mention"` for mentions, `"message1"` for regular messages/DMs).
    pub sound_name: Option<String>,
    /// Whether the `.mention-badge` (`mention_count`) counter should increment.
    pub should_increment_badge: bool,
    /// Whether the channel unread dot (`.is-unread`) should light up.
    pub should_mark_unread: bool,
    /// Freedesktop D-Bus urgency level when `should_show_desktop_banner == true`.
    pub urgency: Option<Urgency>,
}

/// Observable telemetry counters for the D-06 Notification Engine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct D06CascadeTelemetry {
    pub evaluations_total: u64,
    pub channel_override_hits: u64,
    pub category_override_hits: u64,
    pub guild_settings_hits: u64,
    pub guild_default_hits: u64,
    pub muted_suppressions: u64,
    pub expired_mutes_recovered: u64,
    pub sound_alerts_emitted: u64,
    pub badge_increments: u64,
}

/// Stateful D-06 Notification Defaults & Cascade Engine tracking telemetry across evaluations.
#[derive(Debug, Clone, Default)]
pub struct D06CascadeEngine {
    telemetry: D06CascadeTelemetry,
}

impl D06CascadeEngine {
    /// Creates a new D-06 Notification Cascade Engine with zeroed telemetry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a snapshot of the engine's telemetry counters.
    #[must_use]
    pub const fn telemetry(&self) -> &D06CascadeTelemetry {
        &self.telemetry
    }

    /// Evaluates an incoming message against client platform defaults, guild settings, category override, and channel override.
    pub fn evaluate_message(
        &mut self,
        input: &D06MessageEvalInput,
        client_defaults: &D06ClientNotificationDefaults,
        guild: &D06GuildNotificationSettings,
        parent_category: Option<&D06ChannelOverride>,
        channel: Option<&D06ChannelOverride>,
        now_unix: i64,
    ) -> D06CascadeDecision {
        self.telemetry.evaluations_total = self.telemetry.evaluations_total.saturating_add(1);

        let cascade =
            resolve_channel_notification_cascade(guild, parent_category, channel, now_unix);

        match cascade.level_source {
            CascadeLevelSource::ChannelOverride => {
                self.telemetry.channel_override_hits =
                    self.telemetry.channel_override_hits.saturating_add(1);
            }
            CascadeLevelSource::CategoryOverride => {
                self.telemetry.category_override_hits =
                    self.telemetry.category_override_hits.saturating_add(1);
            }
            CascadeLevelSource::GuildSettings => {
                self.telemetry.guild_settings_hits =
                    self.telemetry.guild_settings_hits.saturating_add(1);
            }
            CascadeLevelSource::GuildDefault => {
                self.telemetry.guild_default_hits =
                    self.telemetry.guild_default_hits.saturating_add(1);
            }
        }

        if cascade.expired_mute_recovered {
            self.telemetry.expired_mutes_recovered =
                self.telemetry.expired_mutes_recovered.saturating_add(1);
        }
        if cascade.effective_muted {
            self.telemetry.muted_suppressions =
                self.telemetry.muted_suppressions.saturating_add(1);
        }

        // 1. Self-authored messages never trigger alerts, badges, or unread dots.
        if input.author_id == input.current_user_id {
            return D06CascadeDecision {
                cascade,
                should_show_desktop_banner: false,
                should_play_sound: false,
                sound_cue: None,
                sound_name: None,
                should_increment_badge: false,
                should_mark_unread: false,
                urgency: None,
            };
        }

        let active_focused = input.app_focused
            && input.active_channel_id == Some(input.channel_id)
            && !client_defaults.notify_messages_in_selected_channel;
        let global_alert_suppressed = input.user_dnd || input.streamer_mode || active_focused;

        // 2. Direct Message (DM / Group DM) path:
        if input.is_dm {
            let dm_allowed =
                !cascade.channel_muted || matches!(input.mention_kind, D06MentionKind::User);
            let should_increment_badge = dm_allowed && !client_defaults.disable_unread_badge;
            let should_mark_unread = dm_allowed;
            let alert_eligible = dm_allowed && !global_alert_suppressed;
            let should_show_desktop_banner =
                alert_eligible && client_defaults.can_dispatch_desktop_banner();
            let sound_key = if matches!(input.mention_kind, D06MentionKind::User) {
                "mention"
            } else {
                "message1"
            };
            let should_play_sound = alert_eligible && client_defaults.is_sound_enabled(sound_key);

            if should_play_sound {
                self.telemetry.sound_alerts_emitted =
                    self.telemetry.sound_alerts_emitted.saturating_add(1);
            }
            if should_increment_badge {
                self.telemetry.badge_increments =
                    self.telemetry.badge_increments.saturating_add(1);
            }

            return D06CascadeDecision {
                cascade,
                should_show_desktop_banner,
                should_play_sound,
                sound_cue: should_play_sound.then_some(SoundCue::Mention),
                sound_name: should_play_sound.then(|| sound_key.to_string()),
                should_increment_badge,
                should_mark_unread,
                urgency: should_show_desktop_banner.then_some(Urgency::Critical),
            };
        }

        // 3. Guild channel path: evaluate effective mention, badge, unread dot, desktop banner, and sound rules.
        let (is_effective_mention, should_increment_badge) = match input.mention_kind {
            D06MentionKind::User => (true, !client_defaults.disable_unread_badge),
            D06MentionKind::Role => {
                let allowed = !cascade.suppress_roles;
                (allowed, allowed && !client_defaults.disable_unread_badge)
            }
            D06MentionKind::Everyone | D06MentionKind::Here => {
                let allowed = !cascade.suppress_everyone;
                (allowed, allowed && !client_defaults.disable_unread_badge)
            }
            D06MentionKind::None => {
                let all_as_mention =
                    (input.account_flags & ACCOUNT_FLAG_MENTION_ON_ALL_MESSAGES) != 0;
                (all_as_mention, false)
            }
        };

        let should_mark_unread = if cascade.channel_muted || cascade.category_muted {
            false
        } else if is_effective_mention {
            true
        } else {
            !cascade.opt_in_suppressed
                && cascade.effective_unread == D06UnreadSetting::AllMessages
        };

        let level_allows_alert = match cascade.effective_level {
            D06NotificationLevel::AllMessages => true,
            D06NotificationLevel::OnlyMentions => is_effective_mention,
            D06NotificationLevel::NoMessages | D06NotificationLevel::Inherit => false,
        };

        // Notice: When `input.mention_kind != None` (e.g., suppressed `@role` or `@everyone`),
        // a suppressed mention must NOT fall through and alert as a regular message even in `ALL_MESSAGES` mode
        // (`NOTIFICATIONS-SPEC.md §1.5 п.3`).
        let mention_not_suppressed = match input.mention_kind {
            D06MentionKind::User | D06MentionKind::None => true,
            D06MentionKind::Role => !cascade.suppress_roles,
            D06MentionKind::Everyone | D06MentionKind::Here => !cascade.suppress_everyone,
        };

        let alert_eligible = !cascade.allow_no_messages
            && !global_alert_suppressed
            && level_allows_alert
            && mention_not_suppressed;

        let should_show_desktop_banner =
            alert_eligible && client_defaults.can_dispatch_desktop_banner();
        let sound_key = if is_effective_mention {
            "mention"
        } else {
            "message1"
        };
        let should_play_sound = alert_eligible && client_defaults.is_sound_enabled(sound_key);

        if should_play_sound {
            self.telemetry.sound_alerts_emitted =
                self.telemetry.sound_alerts_emitted.saturating_add(1);
        }
        if should_increment_badge {
            self.telemetry.badge_increments = self.telemetry.badge_increments.saturating_add(1);
        }

        let urgency = if !should_show_desktop_banner {
            None
        } else if is_effective_mention {
            Some(Urgency::Normal)
        } else {
            Some(Urgency::Low)
        };

        D06CascadeDecision {
            cascade,
            should_show_desktop_banner,
            should_play_sound,
            sound_cue: should_play_sound.then_some(SoundCue::Mention),
            sound_name: should_play_sound.then(|| sound_key.to_string()),
            should_increment_badge,
            should_mark_unread,
            urgency,
        }
    }
}

/// Parses an ISO-8601 timestamp (including microseconds and timezone offsets like
/// `2021-02-27T17:14:08.418000+00:00` or `2026-10-30T20:00:00.000Z`) into Unix epoch seconds.
#[must_use]
pub fn parse_iso8601_unix(raw: &str) -> Option<i64> {
    let s = raw.trim();
    if s.len() < 19 {
        return None;
    }
    let bytes = s.as_bytes();
    if bytes[4] != b'-' || bytes[7] != b'-' || (bytes[10] != b'T' && bytes[10] != b't' && bytes[10] != b' ')
        || bytes[13] != b':' || bytes[16] != b':'
    {
        return None;
    }

    let year: i64 = s[0..4].parse().ok()?;
    let month: i64 = s[5..7].parse().ok()?;
    let day: i64 = s[8..10].parse().ok()?;
    let hour: i64 = s[11..13].parse().ok()?;
    let minute: i64 = s[14..16].parse().ok()?;
    let second: i64 = s[17..19].parse().ok()?;

    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || !(0..=23).contains(&hour)
        || !(0..=59).contains(&minute)
        || !(0..=60).contains(&second)
    {
        return None;
    }

    let max_day = days_in_month(year, month);
    if day > max_day {
        return None;
    }

    let mut tail = &s[19..];
    if let Some(after_dot) = tail.strip_prefix('.') {
        let frac_len = after_dot
            .bytes()
            .take_while(u8::is_ascii_digit)
            .count();
        if frac_len == 0 {
            return None;
        }
        tail = &after_dot[frac_len..];
    }

    let tz_offset_seconds: i64 = if tail.eq_ignore_ascii_case("Z") {
        0
    } else if tail.len() == 6 && (tail.starts_with('+') || tail.starts_with('-')) && tail.as_bytes()[3] == b':' {
        let sign = if tail.starts_with('-') { -1 } else { 1 };
        let tz_h: i64 = tail[1..3].parse().ok()?;
        let tz_m: i64 = tail[4..6].parse().ok()?;
        if tz_h > 23 || tz_m > 59 {
            return None;
        }
        sign * (tz_h * 3600 + tz_m * 60)
    } else {
        return None;
    };

    let days = days_from_civil(year, month, day);
    let utc_seconds = days * 86_400 + hour * 3600 + minute * 60 + second - tz_offset_seconds;
    Some(utc_seconds)
}

/// Formats a Unix epoch timestamp in seconds into canonical ISO-8601 UTC (`YYYY-MM-DDTHH:MM:SS.000Z`).
#[must_use]
pub fn format_iso8601_utc(unix_seconds: i64) -> String {
    let days = unix_seconds.div_euclid(86_400);
    let secs_of_day = unix_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = secs_of_day / 3600;
    let minute = (secs_of_day % 3600) / 60;
    let second = secs_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.000Z")
}

const fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

const fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// Howard Hinnant's `days_from_civil` algorithm converting `(year, month, day)` to days since 1970-01-01.
const fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * m + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Howard Hinnant's `civil_from_days` algorithm converting days since 1970-01-01 to `(year, month, day)`.
const fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year, m, d)
}
