//! Автоматический перевод в AFK-канал при простое (`TASK-1010` / `TASK-1010A` / `TASK-1010B`).
//!
//! Оракулы:
//! - `waves/kanban-board.json` (`TASK-1010`, `TASK-1010A`, `TASK-1010B`): `Voice State Service`
//!   (`background daemon`, `0px`, `0px`), дизайн-токены (`#1e1f22`, `#2b2d31`, `#ffffff`, `#f0b232`, `#3f4147`),
//!   состояния `afk-badge` (`hidden -> visible -> moved to afk`), `idle-detect` (`active -> counting -> idle triggered`),
//!   `voice-afk` (`normal voice -> warn afk -> moved to afk`).
//! - `fixtures/dbus/power.json`: `org.freedesktop.ScreenSaver` (`GetSessionIdleTime -> 1234`, `IsActive`)
//!   и `org.freedesktop.login1.Manager` (`PrepareForSleep(b)`).
//! - `docs/re/gateway-protocol.json` (`VOICE_STATE_UPDATE = 8`), `docs/re/gateway-schema.json`
//!   и `/home/sanya/re-deep/gw-proto/msgs-run1-57992_5112.jsonl` (7 живых гильдий с `afk_channel_id` и `afk_timeout`).
//! - `/home/sanya/re-deep/modules-index.json`: модули `885576` (`IdleStore`), `576991` (`voiceStateUpdate`),
//!   `926456` (`VoiceAFKManager`), `185255` (`afkTimeout` пресеты `60, 300, 900, 1800, 3600` с), `260509` (`GuildRecord`).

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::fmt;

pub const AFK_SERVICE_CONTAINER: &str = "Voice State Service";
pub const AFK_SERVICE_DIMENSIONS: &str = "background daemon";
pub const AFK_SERVICE_PADDING_PX: u32 = 0;
pub const AFK_SERVICE_SPACING_PX: u32 = 0;

pub const TOKEN_BACKGROUND: &str = "#1e1f22";
pub const TOKEN_SURFACE: &str = "#2b2d31";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT_WARNING: &str = "#f0b232";
pub const TOKEN_BORDER: &str = "#3f4147";

pub const AFK_BADGE_ELEMENT: &str = "afk-badge";
pub const IDLE_DETECT_ELEMENT: &str = "idle-detect";
pub const VOICE_AFK_ELEMENT: &str = "voice-afk";

pub const GATEWAY_OP_VOICE_STATE_UPDATE: u8 = 4;
pub const VOICE_STATE_DEFAULT_FLAGS: u32 = 2;

pub const DBUS_SCREENSAVER_DESTINATION: &str = "org.freedesktop.ScreenSaver";
pub const DBUS_SCREENSAVER_PATH: &str = "/org/freedesktop/ScreenSaver";
pub const DBUS_SCREENSAVER_INTERFACE: &str = "org.freedesktop.ScreenSaver";
pub const DBUS_IDLE_METHOD: &str = "GetSessionIdleTime";
pub const DBUS_IS_ACTIVE_METHOD: &str = "IsActive";
pub const DBUS_LOGIN1_DESTINATION: &str = "org.freedesktop.login1";
pub const DBUS_LOGIN1_PATH: &str = "/org/freedesktop/login1";
pub const DBUS_LOGIN1_INTERFACE: &str = "org.freedesktop.login1.Manager";
pub const DBUS_PREPARE_FOR_SLEEP_SIGNAL: &str = "PrepareForSleep";

pub const WAYLAND_EXT_IDLE_PROTOCOL: &str = "ext-idle-notifier-v1";
pub const X11_SCREENSAVER_QUERY: &str = "XScreenSaverQueryInfo";

pub const RE_MODULE_IDLE_STORE: &str = "885576";
pub const RE_MODULE_GATEWAY_SOCKET: &str = "576991";
pub const RE_MODULE_VOICE_AFK_MANAGER: &str = "926456";
pub const RE_MODULE_GUILD_AFK_SETTINGS: &str = "185255";
pub const RE_MODULE_GUILD_RECORD: &str = "260509";

pub const IDLE_POLL_INTERVAL_MS: u64 = 10_000;
pub const IDLE_FALLBACK_POLL_INTERVAL_MS: u64 = 30_000;
pub const VOICE_HARD_IDLE_DISCONNECT_MS: u64 = 18_000_000;
pub const ALLOWED_AFK_TIMEOUTS_SECS: [u32; 5] = [60, 300, 900, 1_800, 3_600];

pub const PERM_VIEW_CHANNEL: u64 = 1 << 10;
pub const PERM_CONNECT: u64 = 1 << 20;
pub const PERM_SPEAK: u64 = 1 << 21;
pub const PERM_MOVE_MEMBERS: u64 = 1 << 24;

pub const MAX_MESSAGE_CHARS: usize = 2_000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_BADGE_CAP: u32 = 99;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AfkBadgeInteractiveState {
    Hidden,
    Visible,
    MovedToAfk,
}

impl AfkBadgeInteractiveState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hidden => "hidden",
            Self::Visible => "visible",
            Self::MovedToAfk => "moved to afk",
        }
    }

    #[must_use]
    pub const fn resolve(moved_to_afk: bool, hovered: bool) -> Self {
        if moved_to_afk {
            Self::MovedToAfk
        } else if hovered {
            Self::Visible
        } else {
            Self::Hidden
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleDetectInteractiveState {
    Active,
    Counting,
    IdleTriggered,
}

impl IdleDetectInteractiveState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Counting => "counting",
            Self::IdleTriggered => "idle triggered",
        }
    }

    #[must_use]
    pub const fn resolve(effective_idle_ms: u64, threshold_ms: u64) -> Self {
        if threshold_ms > 0 && effective_idle_ms >= threshold_ms {
            Self::IdleTriggered
        } else if effective_idle_ms > 0 {
            Self::Counting
        } else {
            Self::Active
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceAfkInteractiveState {
    NormalVoice,
    WarnAfk,
    MovedToAfk,
}

impl VoiceAfkInteractiveState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NormalVoice => "normal voice",
            Self::WarnAfk => "warn afk",
            Self::MovedToAfk => "moved to afk",
        }
    }

    #[must_use]
    pub const fn resolve(effective_idle_ms: u64, timeout_ms: u64, moved_to_afk: bool) -> Self {
        if moved_to_afk || (timeout_ms > 0 && effective_idle_ms >= timeout_ms) {
            Self::MovedToAfk
        } else if timeout_ms > 0 && effective_idle_ms >= (timeout_ms * 4) / 5 {
            Self::WarnAfk
        } else {
            Self::NormalVoice
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleBackendKind {
    WaylandExtIdleNotifierV1,
    DbusFreedesktopScreenSaver,
    X11ScreenSaver,
    MonotonicFallback,
}

impl IdleBackendKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WaylandExtIdleNotifierV1 => "wayland_ext_idle_notifier_v1",
            Self::DbusFreedesktopScreenSaver => "dbus_freedesktop_screensaver",
            Self::X11ScreenSaver => "x11_screensaver",
            Self::MonotonicFallback => "monotonic_fallback",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdleBackendCapabilities {
    pub wayland_ext_idle_available: bool,
    pub dbus_screensaver_available: bool,
    pub x11_screensaver_available: bool,
}

#[must_use]
pub const fn select_idle_backend(caps: IdleBackendCapabilities) -> IdleBackendKind {
    if caps.wayland_ext_idle_available {
        IdleBackendKind::WaylandExtIdleNotifierV1
    } else if caps.dbus_screensaver_available {
        IdleBackendKind::DbusFreedesktopScreenSaver
    } else if caps.x11_screensaver_available {
        IdleBackendKind::X11ScreenSaver
    } else {
        IdleBackendKind::MonotonicFallback
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AfkError {
    InvalidSnowflake(String),
    InvalidAfkTimeoutSecs(u32),
    MissingPermission(&'static str),
    AgeGatedRestricted,
    DbusError(String),
    MessageTooLong(usize),
    TooManyAttachments(usize),
    AttachmentTooLarge(u64),
    SearchOffsetOutOfBounds(u32),
    VolumeOutOfBounds(u16),
    HttpFailure { status: u16, retry_after_ms: u64 },
}

impl fmt::Display for AfkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for AfkError {}

pub fn validate_snowflake(raw: &str) -> Result<u64, AfkError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || !trimmed.bytes().all(|b| b.is_ascii_digit()) {
        return Err(AfkError::InvalidSnowflake(raw.to_string()));
    }
    let parsed = trimmed
        .parse::<u64>()
        .map_err(|_| AfkError::InvalidSnowflake(raw.to_string()))?;
    if parsed == 0 {
        return Err(AfkError::InvalidSnowflake(raw.to_string()));
    }
    Ok(parsed)
}

#[must_use]
pub const fn is_valid_afk_timeout_secs(seconds: u32) -> bool {
    matches!(seconds, 60 | 300 | 900 | 1_800 | 3_600)
}

pub fn normalize_dbus_idle_reply(result: Result<u32, &str>) -> Result<Option<u64>, AfkError> {
    match result {
        Ok(secs) => Ok(Some(u64::from(secs) * 1_000)),
        Err(
            "org.freedesktop.DBus.Error.UnknownMethod"
            | "org.freedesktop.DBus.Error.UnknownInterface",
        ) => Ok(None),
        Err(other) => Err(AfkError::DbusError(other.to_string())),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdleSampleInput {
    pub raw_idle_ms: u64,
    pub media_playing: bool,
    pub active_stream_count: u32,
    pub user_speaking: bool,
    pub system_suspended: bool,
    pub screen_locked: bool,
    pub forced_lock_idle_ms: u64,
}

#[must_use]
pub const fn compute_effective_idle_ms(input: IdleSampleInput) -> u64 {
    if input.media_playing || input.active_stream_count > 0 || input.user_speaking {
        return 0;
    }
    if input.system_suspended || input.screen_locked {
        if input.raw_idle_ms > input.forced_lock_idle_ms {
            return input.raw_idle_ms;
        }
        return input.forced_lock_idle_ms;
    }
    input.raw_idle_ms
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdleStateChangedEvent {
    pub is_idle: bool,
    pub effective_idle_ms: u64,
    pub threshold_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceStateUpdateOpcode4 {
    pub op: u8,
    pub guild_id: Option<String>,
    pub channel_id: Option<String>,
    pub self_mute: bool,
    pub self_deaf: bool,
    pub self_video: bool,
    pub flags: u32,
}

impl VoiceStateUpdateOpcode4 {
    #[must_use]
    pub fn to_json(&self) -> String {
        let g = self
            .guild_id
            .as_deref()
            .map_or_else(|| "null".to_string(), |id| format!("\"{id}\""));
        let c = self
            .channel_id
            .as_deref()
            .map_or_else(|| "null".to_string(), |id| format!("\"{id}\""));
        format!(
            "{{\"op\":{},\"d\":{{\"guild_id\":{},\"channel_id\":{},\"self_mute\":{},\"self_deaf\":{},\"self_video\":{},\"flags\":{}}}}}",
            self.op, g, c, self.self_mute, self.self_deaf, self.self_video, self.flags
        )
    }
}

pub fn build_voice_state_update_opcode4(
    guild_id: Option<&str>,
    channel_id: Option<&str>,
    self_mute: bool,
    self_deaf: bool,
    self_video: bool,
    flags: u32,
) -> Result<VoiceStateUpdateOpcode4, AfkError> {
    if let Some(gid) = guild_id {
        validate_snowflake(gid)?;
    }
    if let Some(cid) = channel_id {
        validate_snowflake(cid)?;
    }
    Ok(VoiceStateUpdateOpcode4 {
        op: GATEWAY_OP_VOICE_STATE_UPDATE,
        guild_id: guild_id.map(str::to_string),
        channel_id: channel_id.map(str::to_string),
        self_mute,
        self_deaf,
        self_video,
        flags,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AfkTickOutcome {
    pub effective_idle_ms: u64,
    pub timeout_ms: u64,
    pub should_move_to_afk: bool,
    pub should_hard_disconnect: bool,
    pub bypass_idle_update: bool,
    pub opcode4_frame: Option<VoiceStateUpdateOpcode4>,
    pub idle_event: IdleStateChangedEvent,
    pub afk_badge_state: AfkBadgeInteractiveState,
    pub idle_detect_state: IdleDetectInteractiveState,
    pub voice_afk_state: VoiceAfkInteractiveState,
}

#[derive(Debug, Clone)]
struct RollbackState {
    channel_id: Option<String>,
    self_mute: bool,
    moved_to_afk: bool,
}

#[derive(Debug, Clone)]
pub struct VoiceAfkMonitor {
    pub guild_id: String,
    pub current_channel_id: Option<String>,
    pub afk_channel_id: Option<String>,
    pub afk_timeout_secs: u32,
    pub self_mute: bool,
    pub self_deaf: bool,
    pub is_stage_channel: bool,
    pub moved_to_afk: bool,
    rollback: Option<RollbackState>,
}

impl VoiceAfkMonitor {
    pub fn new(
        guild_id: &str,
        current_channel_id: Option<&str>,
        afk_channel_id: Option<&str>,
        afk_timeout_secs: u32,
        self_mute: bool,
        self_deaf: bool,
        is_stage_channel: bool,
    ) -> Result<Self, AfkError> {
        validate_snowflake(guild_id)?;
        if let Some(cid) = current_channel_id {
            validate_snowflake(cid)?;
        }
        if let Some(aid) = afk_channel_id {
            validate_snowflake(aid)?;
        }
        if !is_valid_afk_timeout_secs(afk_timeout_secs) {
            return Err(AfkError::InvalidAfkTimeoutSecs(afk_timeout_secs));
        }
        let already_in_afk =
            matches!((current_channel_id, afk_channel_id), (Some(c), Some(a)) if c == a);
        Ok(Self {
            guild_id: guild_id.to_string(),
            current_channel_id: current_channel_id.map(str::to_string),
            afk_channel_id: afk_channel_id.map(str::to_string),
            afk_timeout_secs,
            self_mute,
            self_deaf,
            is_stage_channel,
            moved_to_afk: already_in_afk,
            rollback: None,
        })
    }

    pub fn evaluate_tick(&mut self, input: IdleSampleInput) -> AfkTickOutcome {
        let timeout_ms = u64::from(self.afk_timeout_secs) * 1_000;
        let effective_idle_ms = compute_effective_idle_ms(input);
        let should_hard_disconnect =
            self.current_channel_id.is_some() && effective_idle_ms >= VOICE_HARD_IDLE_DISCONNECT_MS;

        let (Some(current_channel), Some(afk_channel)) =
            (self.current_channel_id.clone(), self.afk_channel_id.clone())
        else {
            return self.make_outcome(effective_idle_ms, timeout_ms, should_hard_disconnect, None);
        };

        if self.is_stage_channel {
            return self.make_outcome(effective_idle_ms, timeout_ms, should_hard_disconnect, None);
        }

        if self.moved_to_afk || current_channel == afk_channel {
            self.moved_to_afk = true;
            return self.make_outcome(effective_idle_ms, timeout_ms, should_hard_disconnect, None);
        }

        if effective_idle_ms >= timeout_ms {
            let frame = self.commit_afk_move(afk_channel);
            return self.make_outcome(
                effective_idle_ms,
                timeout_ms,
                should_hard_disconnect,
                Some(frame),
            );
        }

        self.make_outcome(effective_idle_ms, timeout_ms, should_hard_disconnect, None)
    }

    fn commit_afk_move(&mut self, afk_channel: String) -> VoiceStateUpdateOpcode4 {
        self.rollback = Some(RollbackState {
            channel_id: self.current_channel_id.clone(),
            self_mute: self.self_mute,
            moved_to_afk: self.moved_to_afk,
        });
        self.current_channel_id = Some(afk_channel.clone());
        self.self_mute = true;
        self.moved_to_afk = true;
        VoiceStateUpdateOpcode4 {
            op: GATEWAY_OP_VOICE_STATE_UPDATE,
            guild_id: Some(self.guild_id.clone()),
            channel_id: Some(afk_channel),
            self_mute: true,
            self_deaf: self.self_deaf,
            self_video: false,
            flags: VOICE_STATE_DEFAULT_FLAGS,
        }
    }

    fn make_outcome(
        &self,
        effective_idle_ms: u64,
        timeout_ms: u64,
        should_hard_disconnect: bool,
        opcode4_frame: Option<VoiceStateUpdateOpcode4>,
    ) -> AfkTickOutcome {
        let should_move = opcode4_frame.is_some();
        AfkTickOutcome {
            effective_idle_ms,
            timeout_ms,
            should_move_to_afk: should_move,
            should_hard_disconnect,
            bypass_idle_update: should_move,
            opcode4_frame,
            idle_event: IdleStateChangedEvent {
                is_idle: effective_idle_ms >= timeout_ms,
                effective_idle_ms,
                threshold_ms: timeout_ms,
            },
            afk_badge_state: AfkBadgeInteractiveState::resolve(self.moved_to_afk, false),
            idle_detect_state: IdleDetectInteractiveState::resolve(effective_idle_ms, timeout_ms),
            voice_afk_state: VoiceAfkInteractiveState::resolve(
                effective_idle_ms,
                timeout_ms,
                self.moved_to_afk,
            ),
        }
    }

    pub fn rollback_afk_move(&mut self) -> bool {
        let Some(prev) = self.rollback.take() else {
            return false;
        };
        self.current_channel_id = prev.channel_id;
        self.self_mute = prev.self_mute;
        self.moved_to_afk = prev.moved_to_afk;
        true
    }
}

pub fn check_afk_channel_permissions(
    permissions: u64,
    age_gated_restricted: bool,
) -> Result<(), AfkError> {
    if age_gated_restricted {
        return Err(AfkError::AgeGatedRestricted);
    }
    if (permissions & PERM_VIEW_CHANNEL) == 0 {
        return Err(AfkError::MissingPermission("VIEW_CHANNEL"));
    }
    if (permissions & PERM_CONNECT) == 0 {
        return Err(AfkError::MissingPermission("CONNECT"));
    }
    Ok(())
}

pub fn validate_system_boundaries(
    message_chars: usize,
    attachments_count: usize,
    attachment_bytes: u64,
    search_offset: u32,
    volume_pct: u16,
) -> Result<(), AfkError> {
    if message_chars > MAX_MESSAGE_CHARS {
        return Err(AfkError::MessageTooLong(message_chars));
    }
    if attachments_count > MAX_ATTACHMENTS_COUNT {
        return Err(AfkError::TooManyAttachments(attachments_count));
    }
    if attachment_bytes > MAX_ATTACHMENT_BYTES {
        return Err(AfkError::AttachmentTooLarge(attachment_bytes));
    }
    if search_offset > MAX_SEARCH_OFFSET {
        return Err(AfkError::SearchOffsetOutOfBounds(search_offset));
    }
    if volume_pct > MAX_VOLUME_PERCENT {
        return Err(AfkError::VolumeOutOfBounds(volume_pct));
    }
    Ok(())
}

#[must_use]
pub fn format_badge_cap(count: u32) -> String {
    if count > MAX_BADGE_CAP {
        "99+".to_string()
    } else {
        count.to_string()
    }
}

#[must_use]
pub fn redact_afk_diagnostic(raw: &str) -> String {
    let mut out = raw.to_string();
    for marker in ["mfa.", "Bearer ", "session_id="] {
        if let Some(pos) = out.find(marker) {
            let start = pos + marker.len();
            let end = out[start..]
                .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
                .map_or(out.len(), |i| start + i);
            out.replace_range(start..end, "[REDACTED]");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active_sample(raw_idle_ms: u64) -> IdleSampleInput {
        IdleSampleInput {
            raw_idle_ms,
            media_playing: false,
            active_stream_count: 0,
            user_speaking: false,
            system_suspended: false,
            screen_locked: false,
            forced_lock_idle_ms: 60_000,
        }
    }

    #[test]
    fn test_sub_1010_1_constants_and_tokens() {
        assert_eq!(AFK_SERVICE_CONTAINER, "Voice State Service");
        assert_eq!(AFK_SERVICE_DIMENSIONS, "background daemon");
        assert_eq!(AFK_SERVICE_PADDING_PX, 0);
        assert_eq!(AFK_SERVICE_SPACING_PX, 0);
        assert_eq!(TOKEN_BACKGROUND, "#1e1f22");
        assert_eq!(TOKEN_SURFACE, "#2b2d31");
        assert_eq!(TOKEN_TEXT_PRIMARY, "#ffffff");
        assert_eq!(TOKEN_ACCENT_WARNING, "#f0b232");
        assert_eq!(TOKEN_BORDER, "#3f4147");
        assert_eq!(GATEWAY_OP_VOICE_STATE_UPDATE, 4);
        assert_eq!(ALLOWED_AFK_TIMEOUTS_SECS, [60, 300, 900, 1_800, 3_600]);
        assert_eq!(DBUS_SCREENSAVER_DESTINATION, "org.freedesktop.ScreenSaver");
        assert_eq!(DBUS_IDLE_METHOD, "GetSessionIdleTime");
        assert_eq!(WAYLAND_EXT_IDLE_PROTOCOL, "ext-idle-notifier-v1");
        assert_eq!(RE_MODULE_IDLE_STORE, "885576");
        assert_eq!(RE_MODULE_GATEWAY_SOCKET, "576991");
        assert_eq!(RE_MODULE_VOICE_AFK_MANAGER, "926456");
        assert_eq!(RE_MODULE_GUILD_AFK_SETTINGS, "185255");
        assert_eq!(RE_MODULE_GUILD_RECORD, "260509");
    }

    #[test]
    fn test_sub_1010_1_interactive_states() {
        assert_eq!(
            AfkBadgeInteractiveState::resolve(false, false).as_str(),
            "hidden"
        );
        assert_eq!(
            AfkBadgeInteractiveState::resolve(false, true).as_str(),
            "visible"
        );
        assert_eq!(
            AfkBadgeInteractiveState::resolve(true, false).as_str(),
            "moved to afk"
        );
        assert_eq!(
            IdleDetectInteractiveState::resolve(0, 300_000).as_str(),
            "active"
        );
        assert_eq!(
            IdleDetectInteractiveState::resolve(120_000, 300_000).as_str(),
            "counting"
        );
        assert_eq!(
            IdleDetectInteractiveState::resolve(300_000, 300_000).as_str(),
            "idle triggered"
        );
        assert_eq!(
            VoiceAfkInteractiveState::resolve(0, 300_000, false).as_str(),
            "normal voice"
        );
        assert_eq!(
            VoiceAfkInteractiveState::resolve(250_000, 300_000, false).as_str(),
            "warn afk"
        );
        assert_eq!(
            VoiceAfkInteractiveState::resolve(300_000, 300_000, true).as_str(),
            "moved to afk"
        );
    }

    #[test]
    fn test_sub_1010_2_idle_backend_selection_and_media_inhibition() {
        let caps = |w, d, x| IdleBackendCapabilities {
            wayland_ext_idle_available: w,
            dbus_screensaver_available: d,
            x11_screensaver_available: x,
        };
        assert_eq!(
            select_idle_backend(caps(true, true, true)),
            IdleBackendKind::WaylandExtIdleNotifierV1
        );
        assert_eq!(
            select_idle_backend(caps(false, true, true)),
            IdleBackendKind::DbusFreedesktopScreenSaver
        );
        assert_eq!(
            select_idle_backend(caps(false, false, true)),
            IdleBackendKind::X11ScreenSaver
        );
        assert_eq!(
            select_idle_backend(caps(false, false, false)),
            IdleBackendKind::MonotonicFallback
        );

        let mut s = active_sample(1_234_000);
        s.media_playing = true;
        assert_eq!(compute_effective_idle_ms(s), 0);
        s.media_playing = false;
        s.active_stream_count = 2;
        assert_eq!(compute_effective_idle_ms(s), 0);
        s.active_stream_count = 0;
        s.user_speaking = true;
        assert_eq!(compute_effective_idle_ms(s), 0);
        assert_eq!(
            compute_effective_idle_ms(active_sample(1_234_000)),
            1_234_000
        );
        assert_eq!(
            normalize_dbus_idle_reply(Ok(1234)).unwrap(),
            Some(1_234_000)
        );
        assert_eq!(
            normalize_dbus_idle_reply(Err("org.freedesktop.DBus.Error.UnknownMethod")).unwrap(),
            None
        );
    }

    #[test]
    fn test_sub_1010_3_opcode4_afk_move_and_duplicate_guard() {
        let mut monitor = VoiceAfkMonitor::new(
            "274093052673392641",
            Some("351403515723120651"),
            Some("274955515333836801"),
            3_600,
            false,
            false,
            false,
        )
        .unwrap();
        let pre = monitor.evaluate_tick(active_sample(3_599_999));
        assert!(!pre.should_move_to_afk);
        assert!(pre.opcode4_frame.is_none());
        assert_eq!(pre.voice_afk_state, VoiceAfkInteractiveState::WarnAfk);

        let trigger = monitor.evaluate_tick(active_sample(3_600_000));
        assert!(trigger.should_move_to_afk);
        assert!(trigger.bypass_idle_update);
        let frame = trigger.opcode4_frame.unwrap();
        assert_eq!(frame.op, 4);
        assert_eq!(frame.guild_id.as_deref(), Some("274093052673392641"));
        assert_eq!(frame.channel_id.as_deref(), Some("274955515333836801"));
        assert!(frame.self_mute);
        assert!(!frame.self_video);
        assert!(frame.to_json().contains("\"op\":4"));
        assert!(
            !monitor
                .evaluate_tick(active_sample(3_610_000))
                .should_move_to_afk
        );
    }

    #[test]
    fn test_sub_1010_3_stage_channel_skipped() {
        let mut stage_mon = VoiceAfkMonitor::new(
            "274093052673392641",
            Some("351403515723120651"),
            Some("274955515333836801"),
            60,
            false,
            false,
            true,
        )
        .unwrap();
        assert!(
            !stage_mon
                .evaluate_tick(active_sample(120_000))
                .should_move_to_afk
        );
    }

    #[test]
    fn test_sub_1010_4_nine_dimensions_boundaries() {
        assert!(validate_snowflake("0").is_err());
        assert_eq!(
            validate_snowflake("18446744073709551615").unwrap(),
            u64::MAX
        );
        assert!(!is_valid_afk_timeout_secs(0));
        assert!(!is_valid_afk_timeout_secs(120));
        assert!(is_valid_afk_timeout_secs(60));
        assert!(is_valid_afk_timeout_secs(3_600));
        assert!(validate_system_boundaries(2_000, 10, 25 * 1024 * 1024, 9_975, 200).is_ok());
        assert!(validate_system_boundaries(2_001, 10, 0, 0, 100).is_err());
        assert!(validate_system_boundaries(100, 11, 0, 0, 100).is_err());
        assert!(validate_system_boundaries(100, 1, 25 * 1024 * 1024 + 1, 0, 100).is_err());
        assert!(validate_system_boundaries(100, 1, 0, 9_976, 100).is_err());
        assert!(validate_system_boundaries(100, 1, 0, 0, 201).is_err());
        assert_eq!(format_badge_cap(99), "99");
        assert_eq!(format_badge_cap(100), "99+");
    }

    #[test]
    fn test_sub_1010_4_permissions_rollback_and_redaction() {
        let perms = PERM_VIEW_CHANNEL | PERM_CONNECT;
        assert!(check_afk_channel_permissions(perms, false).is_ok());
        assert!(check_afk_channel_permissions(perms, true).is_err());
        assert!(check_afk_channel_permissions(0, false).is_err());

        let mut mon = VoiceAfkMonitor::new(
            "274093052673392641",
            Some("351403515723120651"),
            Some("274955515333836801"),
            60,
            false,
            false,
            false,
        )
        .unwrap();
        assert!(mon.evaluate_tick(active_sample(60_000)).should_move_to_afk);
        assert!(mon.rollback_afk_move());
        assert!(!mon.moved_to_afk);
        assert_eq!(
            mon.current_channel_id.as_deref(),
            Some("351403515723120651")
        );

        let redacted = redact_afk_diagnostic("Bearer mfa.secret123 session_id=abc999");
        assert!(!redacted.contains("secret123"));
        assert!(!redacted.contains("abc999"));
    }
}
