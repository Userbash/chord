//! Глобальные хоткеи и Wayland/X11 In-Game Overlay (`TASK-988`).
//! Контракт (`waves/kanban-board.json` `TASK-988`, `docs/re/main-arch.md` §1.1/§2.3,
//! `docs/re/backend-ipc.json`, `docs/re/native-re.md` §5 `discord_utils.node`):
//! - Протокол Wayland `ext-foreign-toplevel-list-v1` (`ext_foreign_toplevel_list_v1` / `ext_foreign_toplevel_handle_v1`)
//!   и портал `org.freedesktop.portal.GlobalShortcuts`.
//! - Прозрачный клик-сквозь (`input passthrough`) в режиме HUD (`wl_surface.set_input_region(None)` /
//!   `XShapeCombineRegion(..., ShapeInput, empty)`).
//! - Отображение говорящих участников в верхнем левом углу (`x = 16`, `y = 16`, `padding = 16px`, `spacing = 8px`).
//! - Интерактивные состояния `overlay-chat-bubble`: `semi-transparent -> opaque -> typing`.

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fmt;

pub const OVERLAY_CONTAINER: &str = "Overlay HUD";
pub const OVERLAY_DIMENSIONS: &str = "transparent 100vw x 100vh";
pub const OVERLAY_PADDING_PX: u32 = 16;
pub const OVERLAY_SPACING_PX: u32 = 8;
pub const SPEAKER_ROW_HEIGHT_PX: u32 = 40;
pub const SPEAKER_ROW_WIDTH_PX: u32 = 240;

pub const TOKEN_BACKGROUND: &str = "transparent";
pub const TOKEN_SURFACE: &str = "rgba(43,45,49,0.85)";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_BORDER: &str = "#3f4147";
pub const SPEAKING_RING_COLOR: &str = "#23a55a";

pub const EXT_FOREIGN_TOPLEVEL_LIST_IFACE: &str = "ext_foreign_toplevel_list_v1";
pub const EXT_FOREIGN_TOPLEVEL_HANDLE_IFACE: &str = "ext_foreign_toplevel_handle_v1";
pub const EXT_FOREIGN_TOPLEVEL_VERSION: u32 = 1;
pub const GLOBAL_SHORTCUTS_PORTAL_IFACE: &str = "org.freedesktop.portal.GlobalShortcuts";
pub const GLOBAL_SHORTCUTS_PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";

pub const IPC_WINDOW_IS_ALWAYS_ON_TOP: &str = "DISCORD_WINDOW_IS_ALWAYS_ON_TOP";
pub const IPC_WINDOW_SET_ALWAYS_ON_TOP: &str = "DISCORD_WINDOW_SET_ALWAYS_ON_TOP";
pub const IPC_GET_MOUSE_COORDINATES: &str = "DISCORD_GET_MOUSE_COORDINATES";

pub const OVERLAY_WINDOW_TITLE: &str = "Discord Overlay";
pub const OVERLAY_WINDOW_TYPE: &str = "toolbar";
pub const OVERLAY_BG_COLOR_HEX: &str = "#00000000";
pub const OVERLAY_VOICE_FORBIDDEN_ERR: &str = "cannot require chord_voice in overlay";

pub const MAX_KEYBIND_COMBO_EVENTS: usize = 4;
pub const KEYBIND_CAPTURE_TIMEOUT_MS: u64 = 5_000;
pub const EXCLUDED_LEFT_MOUSE_BUTTON: u8 = 1;

pub const VOICE_SPEAKING_MICROPHONE_BIT: u32 = 1 << 0;
pub const VOICE_SPEAKING_SOUNDSHARE_BIT: u32 = 1 << 1;
pub const VOICE_SPEAKING_PRIORITY_BIT: u32 = 1 << 2;

pub const PERM_VIEW_CHANNEL: u64 = 1 << 10;
pub const PERM_CONNECT: u64 = 1 << 20;
pub const PERM_SPEAK: u64 = 1 << 21;

pub const MAX_MESSAGE_CHARS: usize = 2_000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_BADGE_CAP: u32 = 99;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayError {
    ForbiddenSingleCharacterScanCode(u16),
    LeftMouseButtonExcluded,
    ComboTooLong(usize),
    CaptureTimeoutExceeded(u64),
    EmptyKeybind,
    VoiceEngineForbiddenInOverlay,
    ToplevelHandleNotFound(String),
    InvalidSnowflake(u64),
    MessageTooLong(usize),
    TooManyAttachments(usize),
    AttachmentTooLarge(u64),
    SearchOffsetOutOfBounds(u32),
    VolumeOutOfBounds(u16),
    AccessDenied(&'static str),
    AgeGatedSpaceRestricted,
    HttpFailure { status: u16, retry_after_ms: u64 },
}

impl fmt::Display for OverlayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for OverlayError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayBackendKind {
    WaylandExtForeignToplevelV1,
    X11OverrideRedirectXShape,
    HeadlessSynthetic,
}

impl OverlayBackendKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WaylandExtForeignToplevelV1 => "wayland-ext-foreign-toplevel-v1",
            Self::X11OverrideRedirectXShape => "x11-override-redirect-xshape",
            Self::HeadlessSynthetic => "headless-synthetic",
        }
    }

    #[must_use]
    pub const fn passthrough_mechanism(self) -> &'static str {
        match self {
            Self::WaylandExtForeignToplevelV1 => "wl_surface.set_input_region(empty)",
            Self::X11OverrideRedirectXShape => "XShapeCombineRegion(ShapeInput, empty)",
            Self::HeadlessSynthetic => "synthetic-empty-input-region",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayMode {
    Hud,
    Interactive,
}

impl OverlayMode {
    #[must_use]
    pub const fn input_passthrough(self) -> bool {
        match self {
            Self::Hud => true,
            Self::Interactive => false,
        }
    }

    #[must_use]
    pub const fn renderer_event_name(self) -> &'static str {
        match self {
            Self::Hud => "overlay_locked",
            Self::Interactive => "overlay_unlocked",
        }
    }

    #[must_use]
    pub const fn css_pointer_events(self) -> &'static str {
        if self.input_passthrough() {
            "none"
        } else {
            "auto"
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatBubbleTrigger {
    Default,
    Hover,
    Active,
}

impl ChatBubbleTrigger {
    #[must_use]
    pub const fn state_label(self) -> &'static str {
        match self {
            Self::Default => "semi-transparent",
            Self::Hover => "opaque",
            Self::Active => "typing",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputRegionRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl InputRegionRect {
    #[must_use]
    pub const fn contains(self, px: u32, py: u32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointerRouteOutcome {
    pub forwarded_to_game: bool,
    pub intercepted_by_overlay: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignToplevelState {
    pub handle_id: String,
    pub app_id: String,
    pub title: String,
    pub identifier: String,
    pub committed_done: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ForeignToplevelRegistry {
    pending: BTreeMap<String, ForeignToplevelState>,
    committed: BTreeMap<String, ForeignToplevelState>,
    finished: bool,
}

impl ForeignToplevelRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_toplevel(&mut self, handle_id: &str) {
        self.pending.insert(
            handle_id.to_string(),
            ForeignToplevelState {
                handle_id: handle_id.to_string(),
                app_id: String::new(),
                title: String::new(),
                identifier: String::new(),
                committed_done: false,
            },
        );
    }

    pub fn on_app_id(&mut self, handle_id: &str, app_id: &str) -> Result<(), OverlayError> {
        let entry = self
            .pending
            .get_mut(handle_id)
            .ok_or_else(|| OverlayError::ToplevelHandleNotFound(handle_id.to_string()))?;
        entry.app_id = app_id.to_string();
        Ok(())
    }

    pub fn on_title(&mut self, handle_id: &str, title: &str) -> Result<(), OverlayError> {
        let entry = self
            .pending
            .get_mut(handle_id)
            .ok_or_else(|| OverlayError::ToplevelHandleNotFound(handle_id.to_string()))?;
        entry.title = title.to_string();
        Ok(())
    }

    pub fn on_identifier(&mut self, handle_id: &str, identifier: &str) -> Result<(), OverlayError> {
        let entry = self
            .pending
            .get_mut(handle_id)
            .ok_or_else(|| OverlayError::ToplevelHandleNotFound(handle_id.to_string()))?;
        entry.identifier = identifier.to_string();
        Ok(())
    }

    pub fn on_done(&mut self, handle_id: &str) -> Result<ForeignToplevelState, OverlayError> {
        let entry = self
            .pending
            .get_mut(handle_id)
            .ok_or_else(|| OverlayError::ToplevelHandleNotFound(handle_id.to_string()))?;
        entry.committed_done = true;
        let state = entry.clone();
        self.committed.insert(handle_id.to_string(), state);
        Ok(entry.clone())
    }

    pub fn on_closed(&mut self, handle_id: &str) {
        self.pending.remove(handle_id);
        self.committed.remove(handle_id);
    }

    pub fn on_finished(&mut self) {
        self.pending.clear();
        self.committed.clear();
        self.finished = true;
    }

    #[must_use]
    pub fn active_toplevel(&self) -> Option<&ForeignToplevelState> {
        self.committed.values().next_back()
    }

    #[must_use]
    pub fn has_pending_skeleton(&self, handle_id: &str) -> bool {
        self.pending.contains_key(handle_id) && !self.committed.contains_key(handle_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceSpeakerPeer {
    pub user_id: u64,
    pub username: String,
    pub speaking_flags: u32,
    pub muted: bool,
    pub deafened: bool,
    pub volume_percent: u16,
}

impl VoiceSpeakerPeer {
    #[must_use]
    pub const fn is_speaking(&self) -> bool {
        !self.muted
            && !self.deafened
            && (self.speaking_flags
                & (VOICE_SPEAKING_MICROPHONE_BIT | VOICE_SPEAKING_SOUNDSHARE_BIT))
                != 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeakerHudItem {
    pub user_id: u64,
    pub username: String,
    pub speaking: bool,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub surface_color: &'static str,
    pub ring_color: &'static str,
    pub volume_percent: u16,
}

#[must_use]
pub fn is_forbidden_linux_single_scancode(code: u16) -> bool {
    (24..=33).contains(&code) || (38..=46).contains(&code) || (52..=58).contains(&code)
}

pub fn validate_linux_global_keybind(
    scan_codes: &[u16],
    has_modifier: bool,
    mouse_buttons: &[u8],
    capture_duration_ms: u64,
) -> Result<(), OverlayError> {
    if mouse_buttons.contains(&EXCLUDED_LEFT_MOUSE_BUTTON) {
        return Err(OverlayError::LeftMouseButtonExcluded);
    }
    let total_events = scan_codes.len() + mouse_buttons.len();
    if total_events == 0 {
        return Err(OverlayError::EmptyKeybind);
    }
    if total_events > MAX_KEYBIND_COMBO_EVENTS {
        return Err(OverlayError::ComboTooLong(total_events));
    }
    if capture_duration_ms > KEYBIND_CAPTURE_TIMEOUT_MS {
        return Err(OverlayError::CaptureTimeoutExceeded(capture_duration_ms));
    }
    if !has_modifier && scan_codes.len() == 1 && mouse_buttons.is_empty() {
        let code = scan_codes[0];
        if is_forbidden_linux_single_scancode(code) {
            return Err(OverlayError::ForbiddenSingleCharacterScanCode(code));
        }
    }
    Ok(())
}

pub fn guard_overlay_voice_engine_require(in_overlay_process: bool) -> Result<(), OverlayError> {
    if in_overlay_process {
        return Err(OverlayError::VoiceEngineForbiddenInOverlay);
    }
    Ok(())
}

pub fn compute_top_left_speakers_layout(
    peers: &[VoiceSpeakerPeer],
) -> Result<Vec<SpeakerHudItem>, OverlayError> {
    for peer in peers {
        if peer.user_id == 0 {
            return Err(OverlayError::InvalidSnowflake(0));
        }
        if peer.volume_percent > MAX_VOLUME_PERCENT {
            return Err(OverlayError::VolumeOutOfBounds(peer.volume_percent));
        }
    }
    let mut ordered = peers.to_vec();
    ordered.sort_by(|a, b| {
        b.is_speaking()
            .cmp(&a.is_speaking())
            .then_with(|| a.user_id.cmp(&b.user_id))
    });
    let mut items = Vec::with_capacity(ordered.len());
    for (idx, peer) in ordered.into_iter().enumerate() {
        let idx_u32 = u32::try_from(idx).unwrap_or(0);
        let y = OVERLAY_PADDING_PX + idx_u32 * (SPEAKER_ROW_HEIGHT_PX + OVERLAY_SPACING_PX);
        let speaking = peer.is_speaking();
        items.push(SpeakerHudItem {
            user_id: peer.user_id,
            username: peer.username,
            speaking,
            x: OVERLAY_PADDING_PX,
            y,
            width: SPEAKER_ROW_WIDTH_PX,
            height: SPEAKER_ROW_HEIGHT_PX,
            surface_color: TOKEN_SURFACE,
            ring_color: if speaking {
                SPEAKING_RING_COLOR
            } else {
                TOKEN_BORDER
            },
            volume_percent: peer.volume_percent,
        });
    }
    Ok(items)
}

#[derive(Debug, Clone)]
pub struct IngameOverlayState {
    pub backend: OverlayBackendKind,
    pub mode: OverlayMode,
    pub bubble_trigger: ChatBubbleTrigger,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub toplevels: ForeignToplevelRegistry,
    pub speakers: Vec<VoiceSpeakerPeer>,
    pub muted: bool,
    pub deafened: bool,
}

impl IngameOverlayState {
    #[must_use]
    pub fn new(backend: OverlayBackendKind, viewport_width: u32, viewport_height: u32) -> Self {
        Self {
            backend,
            mode: OverlayMode::Hud,
            bubble_trigger: ChatBubbleTrigger::Default,
            viewport_width,
            viewport_height,
            toplevels: ForeignToplevelRegistry::new(),
            speakers: Vec::new(),
            muted: false,
            deafened: false,
        }
    }

    pub fn toggle_lock(&mut self) -> OverlayMode {
        self.mode = match self.mode {
            OverlayMode::Hud => OverlayMode::Interactive,
            OverlayMode::Interactive => {
                self.bubble_trigger = ChatBubbleTrigger::Default;
                OverlayMode::Hud
            }
        };
        self.mode
    }

    #[must_use]
    pub fn input_region_rects(&self) -> Vec<InputRegionRect> {
        if self.mode.input_passthrough() {
            Vec::new()
        } else {
            vec![InputRegionRect {
                x: 0,
                y: 0,
                width: self.viewport_width,
                height: self.viewport_height,
            }]
        }
    }

    #[must_use]
    pub fn route_pointer(&self, x: u32, y: u32) -> PointerRouteOutcome {
        if self.mode.input_passthrough() {
            return PointerRouteOutcome {
                forwarded_to_game: true,
                intercepted_by_overlay: false,
            };
        }
        let hit = self.input_region_rects().iter().any(|r| r.contains(x, y));
        PointerRouteOutcome {
            forwarded_to_game: !hit,
            intercepted_by_overlay: hit,
        }
    }

    pub fn fallback_to_x11_on_compositor_loss(&mut self) {
        self.backend = OverlayBackendKind::X11OverrideRedirectXShape;
    }
}

pub fn validate_overlay_boundaries(
    snowflake_id: u64,
    message_len: usize,
    attachments_count: usize,
    attachment_bytes: u64,
    search_offset: u32,
    volume_percent: u16,
) -> Result<(), OverlayError> {
    if snowflake_id == 0 {
        return Err(OverlayError::InvalidSnowflake(0));
    }
    if message_len > MAX_MESSAGE_CHARS {
        return Err(OverlayError::MessageTooLong(message_len));
    }
    if attachments_count > MAX_ATTACHMENTS_COUNT {
        return Err(OverlayError::TooManyAttachments(attachments_count));
    }
    if attachment_bytes > MAX_ATTACHMENT_BYTES {
        return Err(OverlayError::AttachmentTooLarge(attachment_bytes));
    }
    if search_offset > MAX_SEARCH_OFFSET {
        return Err(OverlayError::SearchOffsetOutOfBounds(search_offset));
    }
    if volume_percent > MAX_VOLUME_PERCENT {
        return Err(OverlayError::VolumeOutOfBounds(volume_percent));
    }
    Ok(())
}

#[must_use]
pub fn format_overlay_unread_badge(count: u32) -> String {
    if count == 0 {
        String::new()
    } else if count > MAX_BADGE_CAP {
        "99+".to_string()
    } else {
        count.to_string()
    }
}

pub fn check_overlay_channel_access(
    perms_mask: u64,
    is_nsfw: bool,
    age_verified: bool,
) -> Result<(), OverlayError> {
    if (perms_mask & PERM_VIEW_CHANNEL) == 0 {
        return Err(OverlayError::AccessDenied("missing VIEW_CHANNEL"));
    }
    if (perms_mask & PERM_CONNECT) == 0 {
        return Err(OverlayError::AccessDenied("missing CONNECT"));
    }
    if is_nsfw && !age_verified {
        return Err(OverlayError::AgeGatedSpaceRestricted);
    }
    Ok(())
}

pub fn classify_overlay_http_status(
    status: u16,
    retry_after_ms: u64,
) -> Result<bool, OverlayError> {
    match status {
        200 | 204 => Ok(false),
        202 => Ok(true),
        401 | 403 | 404 | 413 | 429 | 500..=599 => Err(OverlayError::HttpFailure {
            status,
            retry_after_ms,
        }),
        _ => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sub_988_1_constants_and_oracles() {
        assert_eq!(OVERLAY_CONTAINER, "Overlay HUD");
        assert_eq!(OVERLAY_DIMENSIONS, "transparent 100vw x 100vh");
        assert_eq!(OVERLAY_PADDING_PX, 16);
        assert_eq!(OVERLAY_SPACING_PX, 8);
        assert_eq!(TOKEN_BACKGROUND, "transparent");
        assert_eq!(TOKEN_SURFACE, "rgba(43,45,49,0.85)");
        assert_eq!(TOKEN_TEXT_PRIMARY, "#ffffff");
        assert_eq!(TOKEN_ACCENT, "#5865f2");
        assert_eq!(TOKEN_BORDER, "#3f4147");
        assert_eq!(
            EXT_FOREIGN_TOPLEVEL_LIST_IFACE,
            "ext_foreign_toplevel_list_v1"
        );
        assert_eq!(
            EXT_FOREIGN_TOPLEVEL_HANDLE_IFACE,
            "ext_foreign_toplevel_handle_v1"
        );
        assert_eq!(
            GLOBAL_SHORTCUTS_PORTAL_IFACE,
            "org.freedesktop.portal.GlobalShortcuts"
        );
        assert_eq!(OVERLAY_WINDOW_TITLE, "Discord Overlay");
        assert_eq!(OVERLAY_WINDOW_TYPE, "toolbar");
        assert_eq!(
            guard_overlay_voice_engine_require(true),
            Err(OverlayError::VoiceEngineForbiddenInOverlay)
        );
        assert_eq!(guard_overlay_voice_engine_require(false), Ok(()));
    }

    #[test]
    fn test_sub_988_2_hud_input_passthrough_and_chat_bubble_states() {
        let mut state =
            IngameOverlayState::new(OverlayBackendKind::WaylandExtForeignToplevelV1, 1920, 1080);
        assert_eq!(state.mode, OverlayMode::Hud);
        assert!(state.mode.input_passthrough());
        assert_eq!(state.mode.renderer_event_name(), "overlay_locked");
        assert_eq!(state.mode.css_pointer_events(), "none");
        assert!(state.input_region_rects().is_empty());

        let click = state.route_pointer(32, 32);
        assert!(click.forwarded_to_game);
        assert!(!click.intercepted_by_overlay);

        assert_eq!(state.toggle_lock(), OverlayMode::Interactive);
        assert!(!state.mode.input_passthrough());
        assert_eq!(state.mode.renderer_event_name(), "overlay_unlocked");
        assert_eq!(state.mode.css_pointer_events(), "auto");
        assert_eq!(state.input_region_rects().len(), 1);
        let interactive_click = state.route_pointer(32, 32);
        assert!(!interactive_click.forwarded_to_game);
        assert!(interactive_click.intercepted_by_overlay);

        assert_eq!(ChatBubbleTrigger::Default.state_label(), "semi-transparent");
        assert_eq!(ChatBubbleTrigger::Hover.state_label(), "opaque");
        assert_eq!(ChatBubbleTrigger::Active.state_label(), "typing");

        assert_eq!(
            validate_linux_global_keybind(&[29, 42, 58], true, &[], 350),
            Ok(())
        );
        assert_eq!(
            validate_linux_global_keybind(&[24], false, &[], 120),
            Err(OverlayError::ForbiddenSingleCharacterScanCode(24))
        );
        assert_eq!(
            validate_linux_global_keybind(&[38], false, &[], 120),
            Err(OverlayError::ForbiddenSingleCharacterScanCode(38))
        );
        assert_eq!(
            validate_linux_global_keybind(&[58], false, &[], 120),
            Err(OverlayError::ForbiddenSingleCharacterScanCode(58))
        );
        assert_eq!(
            validate_linux_global_keybind(&[], false, &[1], 100),
            Err(OverlayError::LeftMouseButtonExcluded)
        );
    }

    #[test]
    fn test_sub_988_3_top_left_speakers_and_foreign_toplevel_lifecycle() {
        let mut reg = ForeignToplevelRegistry::new();
        reg.on_toplevel("tl-730");
        assert_eq!(reg.on_app_id("tl-730", "steam_app_730"), Ok(()));
        assert_eq!(reg.on_title("tl-730", "Counter-Strike 2"), Ok(()));
        assert_eq!(reg.on_identifier("tl-730", "kwin-730"), Ok(()));
        assert!(reg.has_pending_skeleton("tl-730"));
        assert!(reg.active_toplevel().is_none());

        let committed = reg.on_done("tl-730");
        assert!(committed.is_ok());
        assert!(!reg.has_pending_skeleton("tl-730"));
        assert!(reg.active_toplevel().is_some());

        let peers = vec![
            VoiceSpeakerPeer {
                user_id: 20,
                username: "Silent".to_string(),
                speaking_flags: 0,
                muted: false,
                deafened: false,
                volume_percent: 100,
            },
            VoiceSpeakerPeer {
                user_id: 10,
                username: "ActiveSpeaker".to_string(),
                speaking_flags: VOICE_SPEAKING_MICROPHONE_BIT,
                muted: false,
                deafened: false,
                volume_percent: 150,
            },
        ];
        let items = compute_top_left_speakers_layout(&peers).unwrap_or_default();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].user_id, 10);
        assert!(items[0].speaking);
        assert_eq!(items[0].x, 16);
        assert_eq!(items[0].y, 16);
        assert_eq!(items[0].ring_color, "#23a55a");
        assert_eq!(items[1].user_id, 20);
        assert!(!items[1].speaking);
        assert_eq!(items[1].x, 16);
        assert_eq!(items[1].y, 64);

        reg.on_closed("tl-730");
        assert!(reg.active_toplevel().is_none());
    }

    #[test]
    fn test_sub_988_4_policy_dimensions_and_boundaries() {
        assert_eq!(
            validate_overlay_boundaries(1, 2_000, 10, 25 * 1024 * 1024, 9_975, 200),
            Ok(())
        );
        assert_eq!(validate_overlay_boundaries(u64::MAX, 0, 0, 0, 0, 0), Ok(()));
        assert_eq!(
            validate_overlay_boundaries(0, 10, 0, 0, 0, 100),
            Err(OverlayError::InvalidSnowflake(0))
        );
        assert_eq!(
            validate_overlay_boundaries(1, 2_001, 0, 0, 0, 100),
            Err(OverlayError::MessageTooLong(2_001))
        );
        assert_eq!(
            validate_overlay_boundaries(1, 10, 11, 0, 0, 100),
            Err(OverlayError::TooManyAttachments(11))
        );
        assert_eq!(
            validate_overlay_boundaries(1, 10, 1, 25 * 1024 * 1024 + 1, 0, 100),
            Err(OverlayError::AttachmentTooLarge(25 * 1024 * 1024 + 1))
        );
        assert_eq!(
            validate_overlay_boundaries(1, 10, 0, 0, 9_976, 100),
            Err(OverlayError::SearchOffsetOutOfBounds(9_976))
        );
        assert_eq!(
            validate_overlay_boundaries(1, 10, 0, 0, 0, 201),
            Err(OverlayError::VolumeOutOfBounds(201))
        );
        assert_eq!(format_overlay_unread_badge(0), "");
        assert_eq!(format_overlay_unread_badge(99), "99");
        assert_eq!(format_overlay_unread_badge(100), "99+");
        assert_eq!(
            check_overlay_channel_access(PERM_VIEW_CHANNEL | PERM_CONNECT, true, false),
            Err(OverlayError::AgeGatedSpaceRestricted)
        );
        assert_eq!(classify_overlay_http_status(202, 0), Ok(true));
        assert_eq!(
            classify_overlay_http_status(429, 1_500),
            Err(OverlayError::HttpFailure {
                status: 429,
                retry_after_ms: 1_500
            })
        );
    }
}
