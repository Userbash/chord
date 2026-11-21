//! Steam Deck & Controller Mode (`R6-01`):
//! - Handheld UI mode detection & dynamic scaling for Steam Deck Game Mode (`1280x800`).
//! - Full platform navigation via gamepad D-Pad, thumbstick, bumpers, triggers, and face buttons.
//! - Automatic SteamOS on-screen keyboard summoning via D-Bus (`org.kde.kwin.VirtualKeyboard`).

#![forbid(unsafe_code)]

use crate::gamepad_driver::{GamepadDriver, GilrsEvent, NavAction};

pub const DBUS_KWIN_SERVICE: &str = "org.kde.kwin.VirtualKeyboard";
pub const DBUS_KWIN_PATH: &str = "/VirtualKeyboard";
pub const DBUS_KWIN_INTERFACE: &str = "org.kde.kwin.VirtualKeyboard";
pub const STEAM_URL_KEYBOARD: &str = "steam://open/keyboard";

pub const STEAM_DECK_NATIVE_WIDTH: u32 = 1280;
pub const STEAM_DECK_NATIVE_HEIGHT: u32 = 800;
pub const HANDHELD_BASE_SCALE: f32 = 1.25;
pub const MIN_UI_SCALE: f32 = 1.0;
pub const MAX_UI_SCALE: f32 = 2.0;
pub const BASE_FOCUS_TARGET_PX: u32 = 36;

/// Environment signals used to detect Steam Deck / Gamescope Game Mode.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SteamDeckEnv {
    pub steam_deck: bool,
    pub gamescope_wayland_display: Option<String>,
    pub xdg_current_desktop: Option<String>,
}

impl SteamDeckEnv {
    /// Reads Steam Deck and Gamescope environment variables from the current process.
    pub fn from_process() -> Self {
        Self {
            steam_deck: std::env::var("STEAM_DECK")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            gamescope_wayland_display: std::env::var("GAMESCOPE_WAYLAND_DISPLAY").ok(),
            xdg_current_desktop: std::env::var("XDG_CURRENT_DESKTOP").ok(),
        }
    }
}

/// Configuration for Handheld UI mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HandheldModeConfig {
    pub is_handheld_mode: bool,
    pub is_game_mode: bool,
    pub preferred_scale: f32,
}

impl Default for HandheldModeConfig {
    fn default() -> Self {
        Self {
            is_handheld_mode: false,
            is_game_mode: false,
            preferred_scale: MIN_UI_SCALE,
        }
    }
}

impl HandheldModeConfig {
    /// Detects handheld and Game Mode state from environment signals.
    pub fn from_env(env: &SteamDeckEnv) -> Self {
        let is_gamescope = env.gamescope_wayland_display.is_some()
            || env
                .xdg_current_desktop
                .as_deref()
                .map(|d| d.eq_ignore_ascii_case("gamescope"))
                .unwrap_or(false);
        let is_handheld_mode = env.steam_deck || is_gamescope;
        Self {
            is_handheld_mode,
            is_game_mode: is_gamescope,
            preferred_scale: if is_handheld_mode {
                HANDHELD_BASE_SCALE
            } else {
                MIN_UI_SCALE
            },
        }
    }
}

/// Dynamic layout metrics scaled for Steam Deck Game Mode or external displays.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HandheldLayoutMetrics {
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub ui_scale: f32,
    pub min_focus_target_px: u32,
}

impl HandheldLayoutMetrics {
    /// Computes dynamic UI scaling for the given viewport and handheld configuration.
    pub fn for_viewport(width: u32, height: u32, config: &HandheldModeConfig) -> Self {
        let raw_scale = if !config.is_handheld_mode {
            MIN_UI_SCALE
        } else if width <= STEAM_DECK_NATIVE_WIDTH && height <= STEAM_DECK_NATIVE_HEIGHT {
            config.preferred_scale
        } else {
            // Scale proportionally for docked external displays while staying within [1.0, 2.0].
            let width_ratio = width as f32 / STEAM_DECK_NATIVE_WIDTH as f32;
            (config.preferred_scale / width_ratio).clamp(MIN_UI_SCALE, MAX_UI_SCALE)
        };
        let ui_scale = raw_scale.clamp(MIN_UI_SCALE, MAX_UI_SCALE);
        let min_focus_target_px = ((BASE_FOCUS_TARGET_PX as f32) * ui_scale).round() as u32;

        Self {
            viewport_width: width,
            viewport_height: height,
            ui_scale,
            min_focus_target_px,
        }
    }
}

/// Platform-level focus zones reachable via gamepad navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlatformFocusZone {
    ServerRail,
    ChannelSidebar,
    ChatTimeline,
    MessageInputBar,
}

impl PlatformFocusZone {
    fn next_right(self) -> Self {
        match self {
            Self::ServerRail => Self::ChannelSidebar,
            Self::ChannelSidebar => Self::ChatTimeline,
            Self::ChatTimeline => Self::MessageInputBar,
            Self::MessageInputBar => Self::MessageInputBar,
        }
    }

    fn next_left(self) -> Self {
        match self {
            Self::ServerRail => Self::ServerRail,
            Self::ChannelSidebar => Self::ServerRail,
            Self::ChatTimeline => Self::ChannelSidebar,
            Self::MessageInputBar => Self::ChatTimeline,
        }
    }
}

/// Stateful gamepad navigator for platform UI zones, servers, and channels.
#[derive(Debug, Clone)]
pub struct HandheldNavigator {
    driver: GamepadDriver,
    current_zone: PlatformFocusZone,
    server_index: usize,
    server_count: usize,
    channel_index: usize,
    channel_count: usize,
}

impl HandheldNavigator {
    pub fn new(server_count: usize, channel_count: usize) -> Self {
        Self {
            driver: GamepadDriver::default(),
            current_zone: PlatformFocusZone::ServerRail,
            server_index: 0,
            server_count: server_count.max(1),
            channel_index: 0,
            channel_count: channel_count.max(1),
        }
    }

    pub fn current_zone(&self) -> PlatformFocusZone {
        self.current_zone
    }

    pub fn server_index(&self) -> usize {
        self.server_index
    }

    pub fn channel_index(&self) -> usize {
        self.channel_index
    }

    /// Processes a raw `GilrsEvent`, updates focus/selection state, and returns the `NavAction`.
    pub fn handle_event(&mut self, event: GilrsEvent) -> Option<NavAction> {
        let action = self.driver.process_event(event)?;
        match action {
            NavAction::NavRight => self.current_zone = self.current_zone.next_right(),
            NavAction::NavLeft => self.current_zone = self.current_zone.next_left(),
            NavAction::NextServer => {
                self.server_index = (self.server_index + 1) % self.server_count;
            }
            NavAction::PrevServer => {
                self.server_index = if self.server_index == 0 {
                    self.server_count - 1
                } else {
                    self.server_index - 1
                };
            }
            NavAction::NextChannel => {
                self.channel_index = (self.channel_index + 1) % self.channel_count;
            }
            NavAction::PrevChannel => {
                self.channel_index = if self.channel_index == 0 {
                    self.channel_count - 1
                } else {
                    self.channel_index - 1
                };
            }
            NavAction::NavUp
            | NavAction::NavDown
            | NavAction::Select
            | NavAction::Back
            | NavAction::OpenKeyboard => {}
        }
        Some(action)
    }
}

/// Abstraction for executing D-Bus method calls (enables deterministic contract testing).
pub trait DbusInvoker {
    fn call_method(
        &mut self,
        service: &str,
        path: &str,
        interface: &str,
        method: &str,
    ) -> Result<(), String>;
}

/// State of the SteamOS virtual keyboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VirtualKeyboardState {
    Hidden,
    Visible,
    Failed(String),
}

/// D-Bus client for summoning and dismissing the SteamOS / KWin on-screen keyboard.
#[derive(Debug, Clone)]
pub struct VirtualKeyboardDbusClient {
    state: VirtualKeyboardState,
}

impl Default for VirtualKeyboardDbusClient {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualKeyboardDbusClient {
    pub fn new() -> Self {
        Self {
            state: VirtualKeyboardState::Hidden,
        }
    }

    pub fn state(&self) -> &VirtualKeyboardState {
        &self.state
    }

    pub fn steam_url_fallback(&self) -> &'static str {
        STEAM_URL_KEYBOARD
    }

    pub fn show<I: DbusInvoker>(&mut self, invoker: &mut I) -> Result<(), String> {
        match invoker.call_method(DBUS_KWIN_SERVICE, DBUS_KWIN_PATH, DBUS_KWIN_INTERFACE, "show") {
            Ok(()) => {
                self.state = VirtualKeyboardState::Visible;
                Ok(())
            }
            Err(err) => {
                self.state = VirtualKeyboardState::Failed(err.clone());
                Err(err)
            }
        }
    }

    pub fn hide<I: DbusInvoker>(&mut self, invoker: &mut I) -> Result<(), String> {
        match invoker.call_method(DBUS_KWIN_SERVICE, DBUS_KWIN_PATH, DBUS_KWIN_INTERFACE, "hide") {
            Ok(()) => {
                self.state = VirtualKeyboardState::Hidden;
                Ok(())
            }
            Err(err) => {
                self.state = VirtualKeyboardState::Failed(err.clone());
                Err(err)
            }
        }
    }

    /// Automatically summons the virtual keyboard when a text input field gains focus,
    /// and hides it when focus leaves text input. Returns `Ok(true)` if visible.
    pub fn on_focus_changed<I: DbusInvoker>(
        &mut self,
        zone: PlatformFocusZone,
        is_text_input: bool,
        invoker: &mut I,
    ) -> Result<bool, String> {
        let should_show = is_text_input || zone == PlatformFocusZone::MessageInputBar;
        if should_show {
            self.show(invoker)?;
            Ok(true)
        } else {
            if self.state == VirtualKeyboardState::Visible {
                self.hide(invoker)?;
            }
            Ok(false)
        }
    }
}
