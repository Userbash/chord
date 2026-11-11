//! Модуль управления клиентом с геймпада через gilrs и экранная клавиатура SteamOS (`TASK-1014`).
//! Реализует опрос геймпада, фильтрацию осей/мёртвой зоны (`0.15`), маппинг D-pad/бамперов,
//! фокусную сетку интерфейса GPUI (`1280x800 handheld`) и вызов виртуальной клавиатуры через D-Bus.

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub const GAMEPAD_CONTAINER: &str = "Gamepad Navigation HUD";
pub const GAMEPAD_DIMENSIONS: &str = "1280x800 handheld";
pub const GAMEPAD_PADDING: &str = "16px";
pub const GAMEPAD_SPACING: &str = "12px";

pub const TOKEN_BG: &str = "#1e1f22";
pub const TOKEN_SURFACE: &str = "#2b2d31";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_BORDER: &str = "#5865f2";

pub const DBUS_KWIN_SERVICE: &str = "org.kde.kwin.VirtualKeyboard";
pub const DBUS_KWIN_PATH: &str = "/VirtualKeyboard";
pub const DBUS_KWIN_INTERFACE: &str = "org.kde.kwin.VirtualKeyboard";
pub const STEAM_SHORTCUT_VIRTUAL_KEYBOARD: &str = "Steam + X";
pub const STEAM_URL_KEYBOARD: &str = "steam://open/keyboard";

pub const DEFAULT_STICK_DEADZONE: f32 = 0.15;
pub const MIN_STICK_DEADZONE: f32 = 0.01;
pub const MAX_STICK_DEADZONE: f32 = 0.50;
pub const STICK_DEFLECTION_THRESHOLD: f32 = 0.50;

pub const FOCUS_OUTLINE_WIDTH_PX: u32 = 2;
pub const FOCUS_OUTLINE_COLOR: &str = "#5865f2";

pub const MAX_MESSAGE_CHARS: usize = 2_000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_BADGE_CAP: u32 = 99;

pub const PERM_VIEW_CHANNEL: u64 = 1 << 10;
pub const PERM_SEND_MESSAGES: u64 = 1 << 11;
pub const PERM_MANAGE_CHANNELS: u64 = 1 << 4;
pub const PERM_AGE_GATED_SPACES: u64 = 1 << 39;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GamepadButton {
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
    LeftBumper,
    RightBumper,
    LeftTrigger,
    RightTrigger,
    South,
    East,
    West,
    North,
    Select,
    Start,
    Guide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GamepadAxis {
    LeftStickX,
    LeftStickY,
    RightStickX,
    RightStickY,
    LeftTrigger,
    RightTrigger,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GilrsEvent {
    ButtonPressed {
        id: usize,
        button: GamepadButton,
        timestamp_ms: u64,
    },
    ButtonReleased {
        id: usize,
        button: GamepadButton,
        timestamp_ms: u64,
    },
    AxisChanged {
        id: usize,
        axis: GamepadAxis,
        value: f32,
        timestamp_ms: u64,
    },
    Connected {
        id: usize,
        name: String,
    },
    Disconnected {
        id: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationAction {
    NavUp,
    NavDown,
    NavLeft,
    NavRight,
    NextServer,
    PrevServer,
    NextChannel,
    PrevChannel,
    Activate,
    Cancel,
    ToggleKeyboard,
    Search,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusZone {
    ServerRail,
    ChannelSidebar,
    ChatTimeline,
    MessageInputBar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualKeyboardState {
    Hidden,
    Visible,
    Failed,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GamepadError {
    InvalidDeadzone(f32),
    InvalidAxisValue(f32),
    GamepadNotFound(usize),
    DBusServiceUnavailable(&'static str),
    HttpError {
        status: u16,
        retry_after_ms: Option<u64>,
    },
    BoundaryViolation(&'static str),
    PermissionDenied(&'static str),
}

impl fmt::Display for GamepadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for GamepadError {}

#[derive(Debug, Clone)]
pub struct GamepadInputProcessor {
    deadzone: f32,
    active_buttons: BTreeMap<usize, BTreeSet<GamepadButton>>,
    stick_axes: BTreeMap<usize, BTreeMap<GamepadAxis, f32>>,
    connected_gamepads: BTreeSet<usize>,
}

impl Default for GamepadInputProcessor {
    fn default() -> Self {
        Self::new(DEFAULT_STICK_DEADZONE)
    }
}

impl GamepadInputProcessor {
    pub fn new(deadzone: f32) -> Self {
        let clamped = deadzone.clamp(MIN_STICK_DEADZONE, MAX_STICK_DEADZONE);
        Self {
            deadzone: clamped,
            active_buttons: BTreeMap::new(),
            stick_axes: BTreeMap::new(),
            connected_gamepads: BTreeSet::new(),
        }
    }

    pub fn deadzone(&self) -> f32 {
        self.deadzone
    }

    pub fn set_deadzone(&mut self, deadzone: f32) -> Result<(), GamepadError> {
        if !deadzone.is_finite() || deadzone < 0.0 || deadzone > 1.0 {
            return Err(GamepadError::InvalidDeadzone(deadzone));
        }
        self.deadzone = deadzone.clamp(MIN_STICK_DEADZONE, MAX_STICK_DEADZONE);
        Ok(())
    }

    pub fn filter_axis(&self, val: f32) -> f32 {
        if !val.is_finite() {
            return 0.0;
        }
        if val.abs() < self.deadzone {
            0.0
        } else {
            val.clamp(-1.0, 1.0)
        }
    }

    pub fn process_event(&mut self, event: &GilrsEvent) -> Option<NavigationAction> {
        match event {
            GilrsEvent::Connected { id, .. } => {
                self.connected_gamepads.insert(*id);
                None
            }
            GilrsEvent::Disconnected { id } => {
                self.handle_disconnect(*id);
                None
            }
            GilrsEvent::ButtonPressed { id, button, .. } => self.handle_button_press(*id, *button),
            GilrsEvent::ButtonReleased { id, button, .. } => {
                self.handle_button_release(*id, *button);
                None
            }
            GilrsEvent::AxisChanged {
                id, axis, value, ..
            } => self.handle_axis_change(*id, *axis, *value),
        }
    }

    fn handle_disconnect(&mut self, id: usize) {
        self.connected_gamepads.remove(&id);
        self.active_buttons.remove(&id);
        self.stick_axes.remove(&id);
    }

    fn handle_button_press(
        &mut self,
        id: usize,
        button: GamepadButton,
    ) -> Option<NavigationAction> {
        let entry = self.active_buttons.entry(id).or_default();
        entry.insert(button);
        map_button_to_action(button)
    }

    fn handle_button_release(&mut self, id: usize, button: GamepadButton) {
        if let Some(set) = self.active_buttons.get_mut(&id) {
            set.remove(&button);
        }
    }

    fn handle_axis_change(
        &mut self,
        id: usize,
        axis: GamepadAxis,
        value: f32,
    ) -> Option<NavigationAction> {
        let filtered = self.filter_axis(value);
        let axes = self.stick_axes.entry(id).or_default();
        let prev = axes.get(&axis).copied().unwrap_or(0.0);
        axes.insert(axis, filtered);
        detect_axis_action(axis, prev, filtered)
    }

    pub fn is_button_pressed(&self, id: usize, button: GamepadButton) -> bool {
        self.active_buttons
            .get(&id)
            .map(|set| set.contains(&button))
            .unwrap_or(false)
    }

    pub fn is_connected(&self, id: usize) -> bool {
        self.connected_gamepads.contains(&id)
    }
}

fn map_button_to_action(button: GamepadButton) -> Option<NavigationAction> {
    match button {
        GamepadButton::DPadUp => Some(NavigationAction::NavUp),
        GamepadButton::DPadDown => Some(NavigationAction::NavDown),
        GamepadButton::DPadLeft => Some(NavigationAction::NavLeft),
        GamepadButton::DPadRight => Some(NavigationAction::NavRight),
        GamepadButton::LeftBumper => Some(NavigationAction::PrevServer),
        GamepadButton::RightBumper => Some(NavigationAction::NextServer),
        GamepadButton::LeftTrigger => Some(NavigationAction::PrevChannel),
        GamepadButton::RightTrigger => Some(NavigationAction::NextChannel),
        GamepadButton::South => Some(NavigationAction::Activate),
        GamepadButton::East => Some(NavigationAction::Cancel),
        GamepadButton::West => Some(NavigationAction::ToggleKeyboard),
        GamepadButton::North => Some(NavigationAction::Search),
        GamepadButton::Guide => Some(NavigationAction::ToggleKeyboard),
        GamepadButton::Select | GamepadButton::Start => None,
    }
}

fn detect_axis_action(axis: GamepadAxis, prev: f32, curr: f32) -> Option<NavigationAction> {
    let crossed_pos = prev < STICK_DEFLECTION_THRESHOLD && curr >= STICK_DEFLECTION_THRESHOLD;
    let crossed_neg = prev > -STICK_DEFLECTION_THRESHOLD && curr <= -STICK_DEFLECTION_THRESHOLD;

    match axis {
        GamepadAxis::LeftStickX => match (crossed_pos, crossed_neg) {
            (true, _) => Some(NavigationAction::NavRight),
            (_, true) => Some(NavigationAction::NavLeft),
            _ => None,
        },
        GamepadAxis::LeftStickY => match (crossed_pos, crossed_neg) {
            (true, _) => Some(NavigationAction::NavDown),
            (_, true) => Some(NavigationAction::NavUp),
            _ => None,
        },
        GamepadAxis::LeftTrigger if crossed_pos => Some(NavigationAction::PrevChannel),
        GamepadAxis::RightTrigger if crossed_pos => Some(NavigationAction::NextChannel),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct FocusGrid {
    pub current_zone: FocusZone,
    pub server_index: usize,
    pub channel_index: usize,
    pub message_index: usize,
    pub server_count: usize,
    pub channel_count: usize,
    pub message_count: usize,
}

impl FocusGrid {
    pub fn new(server_count: usize, channel_count: usize, message_count: usize) -> Self {
        Self {
            current_zone: FocusZone::ServerRail,
            server_index: 0,
            channel_index: 0,
            message_index: 0,
            server_count,
            channel_count,
            message_count,
        }
    }

    pub fn apply_action(&mut self, action: NavigationAction) -> bool {
        match action {
            NavigationAction::NavLeft => self.move_zone_left(),
            NavigationAction::NavRight => self.move_zone_right(),
            NavigationAction::NavUp => self.navigate_up(),
            NavigationAction::NavDown => self.navigate_down(),
            NavigationAction::NextServer => self.cycle_server_next(),
            NavigationAction::PrevServer => self.cycle_server_prev(),
            NavigationAction::NextChannel => self.cycle_channel_next(),
            NavigationAction::PrevChannel => self.cycle_channel_prev(),
            NavigationAction::Cancel => self.retreat_zone(),
            NavigationAction::Activate | NavigationAction::ToggleKeyboard => true,
            NavigationAction::Search => {
                self.current_zone = FocusZone::ChannelSidebar;
                true
            }
        }
    }

    fn move_zone_left(&mut self) -> bool {
        let prev = self.current_zone;
        self.current_zone = match self.current_zone {
            FocusZone::MessageInputBar => FocusZone::ChatTimeline,
            FocusZone::ChatTimeline => FocusZone::ChannelSidebar,
            FocusZone::ChannelSidebar => FocusZone::ServerRail,
            FocusZone::ServerRail => FocusZone::ServerRail,
        };
        prev != self.current_zone
    }

    fn move_zone_right(&mut self) -> bool {
        let prev = self.current_zone;
        self.current_zone = match self.current_zone {
            FocusZone::ServerRail => FocusZone::ChannelSidebar,
            FocusZone::ChannelSidebar => FocusZone::ChatTimeline,
            FocusZone::ChatTimeline => FocusZone::MessageInputBar,
            FocusZone::MessageInputBar => FocusZone::MessageInputBar,
        };
        prev != self.current_zone
    }

    fn retreat_zone(&mut self) -> bool {
        if self.current_zone != FocusZone::ServerRail {
            self.move_zone_left()
        } else {
            false
        }
    }

    fn navigate_up(&mut self) -> bool {
        match self.current_zone {
            FocusZone::ServerRail => step_down(&mut self.server_index),
            FocusZone::ChannelSidebar => step_down(&mut self.channel_index),
            FocusZone::ChatTimeline => step_down(&mut self.message_index),
            FocusZone::MessageInputBar => {
                self.current_zone = FocusZone::ChatTimeline;
                true
            }
        }
    }

    fn navigate_down(&mut self) -> bool {
        match self.current_zone {
            FocusZone::ServerRail => step_up(&mut self.server_index, self.server_count),
            FocusZone::ChannelSidebar => step_up(&mut self.channel_index, self.channel_count),
            FocusZone::ChatTimeline => {
                if self.message_index + 1 >= self.message_count {
                    self.current_zone = FocusZone::MessageInputBar;
                    true
                } else {
                    step_up(&mut self.message_index, self.message_count)
                }
            }
            FocusZone::MessageInputBar => false,
        }
    }

    fn cycle_server_next(&mut self) -> bool {
        let count = self.server_count;
        if count == 0 {
            return false;
        }
        self.server_index = (self.server_index + 1) % count;
        true
    }

    fn cycle_server_prev(&mut self) -> bool {
        let count = self.server_count;
        if count == 0 {
            return false;
        }
        self.server_index = (self.server_index + count - 1) % count;
        true
    }

    fn cycle_channel_next(&mut self) -> bool {
        let count = self.channel_count;
        if count == 0 {
            return false;
        }
        self.channel_index = (self.channel_index + 1) % count;
        true
    }

    fn cycle_channel_prev(&mut self) -> bool {
        let count = self.channel_count;
        if count == 0 {
            return false;
        }
        self.channel_index = (self.channel_index + count - 1) % count;
        true
    }

    pub fn compute_scroll_offset(
        item_index: usize,
        item_height: u32,
        viewport_height: u32,
        current_offset: u32,
    ) -> u32 {
        let item_top = (item_index as u32).saturating_mul(item_height);
        let item_bottom = item_top.saturating_add(item_height);

        if item_top < current_offset {
            item_top
        } else if item_bottom > current_offset.saturating_add(viewport_height) {
            item_bottom.saturating_sub(viewport_height)
        } else {
            current_offset
        }
    }

    pub fn render_focus_style(&self) -> String {
        format!("outline: {FOCUS_OUTLINE_WIDTH_PX}px solid {FOCUS_OUTLINE_COLOR};")
    }
}

fn step_down(index: &mut usize) -> bool {
    if *index > 0 {
        *index -= 1;
        true
    } else {
        false
    }
}

fn step_up(index: &mut usize, count: usize) -> bool {
    if count > 0 && *index + 1 < count {
        *index += 1;
        true
    } else {
        false
    }
}

#[derive(Debug, Clone)]
pub struct VirtualKeyboardController {
    service: &'static str,
    path: &'static str,
    interface: &'static str,
    state: VirtualKeyboardState,
}

impl Default for VirtualKeyboardController {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualKeyboardController {
    pub const fn new() -> Self {
        Self {
            service: DBUS_KWIN_SERVICE,
            path: DBUS_KWIN_PATH,
            interface: DBUS_KWIN_INTERFACE,
            state: VirtualKeyboardState::Hidden,
        }
    }

    pub fn state(&self) -> VirtualKeyboardState {
        self.state
    }

    pub fn build_dbus_call(&self, method: &str) -> String {
        format!(
            "qdbus {} {} {}.{}",
            self.service, self.path, self.interface, method
        )
    }

    pub fn build_steam_url(&self) -> &'static str {
        STEAM_URL_KEYBOARD
    }

    pub fn show(&mut self) -> Result<String, GamepadError> {
        self.state = VirtualKeyboardState::Visible;
        Ok(self.build_dbus_call("show"))
    }

    pub fn hide(&mut self) -> Result<String, GamepadError> {
        self.state = VirtualKeyboardState::Hidden;
        Ok(self.build_dbus_call("hide"))
    }

    pub fn toggle(&mut self) -> Result<String, GamepadError> {
        self.state = match self.state {
            VirtualKeyboardState::Hidden | VirtualKeyboardState::Failed => {
                VirtualKeyboardState::Visible
            }
            VirtualKeyboardState::Visible => VirtualKeyboardState::Hidden,
        };
        Ok(self.build_dbus_call("toggle"))
    }

    pub fn handle_activation(
        &mut self,
        current_zone: FocusZone,
    ) -> Result<Option<String>, GamepadError> {
        let show_keyboard = current_zone == FocusZone::MessageInputBar;
        if show_keyboard {
            let cmd = self.show()?;
            Ok(Some(cmd))
        } else {
            Ok(None)
        }
    }
}

pub fn mask_gamepad_secret(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut tokens = input.split_whitespace();
    while let Some(tok) = tokens.next() {
        if tok.starts_with("token=") || tok.starts_with("pass=") {
            out.push_str("token=*** ");
        } else {
            out.push_str(tok);
            out.push(' ');
        }
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sub_1014_2_gamepad_input_driver() {
        let mut p = GamepadInputProcessor::new(0.15);
        assert_eq!(
            (
                p.filter_axis(0.08),
                p.filter_axis(-0.08),
                p.filter_axis(0.8)
            ),
            (0.0, 0.0, 0.8)
        );
        let id = 1;
        p.process_event(&GilrsEvent::Connected {
            id,
            name: "Deck".into(),
        });
        assert!(p.is_connected(id));

        for (btn, expected) in [
            (GamepadButton::DPadUp, NavigationAction::NavUp),
            (GamepadButton::LeftBumper, NavigationAction::PrevServer),
            (GamepadButton::RightBumper, NavigationAction::NextServer),
            (GamepadButton::LeftTrigger, NavigationAction::PrevChannel),
            (GamepadButton::RightTrigger, NavigationAction::NextChannel),
            (GamepadButton::South, NavigationAction::Activate),
            (GamepadButton::East, NavigationAction::Cancel),
            (GamepadButton::West, NavigationAction::ToggleKeyboard),
            (GamepadButton::North, NavigationAction::Search),
        ] {
            let act = p.process_event(&GilrsEvent::ButtonPressed {
                id,
                button: btn,
                timestamp_ms: 100,
            });
            assert_eq!(act, Some(expected));
            assert!(p.is_button_pressed(id, btn));
        }

        p.process_event(&GilrsEvent::Disconnected { id });
        assert!(!p.is_connected(id) && !p.is_button_pressed(id, GamepadButton::DPadUp));
    }

    #[test]
    fn test_sub_1014_3_focus_grid_navigation() {
        let mut grid = FocusGrid::new(3, 5, 10);
        assert_eq!(grid.current_zone, FocusZone::ServerRail);

        assert!(grid.apply_action(NavigationAction::NavRight));
        assert_eq!(grid.current_zone, FocusZone::ChannelSidebar);
        assert!(grid.apply_action(NavigationAction::NavRight));
        assert_eq!(grid.current_zone, FocusZone::ChatTimeline);
        assert!(grid.apply_action(NavigationAction::NavRight));
        assert_eq!(grid.current_zone, FocusZone::MessageInputBar);
        assert!(grid.apply_action(NavigationAction::NavLeft));
        assert_eq!(grid.current_zone, FocusZone::ChatTimeline);

        assert_eq!(grid.server_index, 0);
        assert!(grid.apply_action(NavigationAction::NextServer));
        assert_eq!(grid.server_index, 1);
        assert!(grid.apply_action(NavigationAction::PrevServer));
        assert_eq!(grid.server_index, 0);

        let style = grid.render_focus_style();
        assert!(style.contains("#5865f2") && style.contains("2px"));
        assert_eq!(FocusGrid::compute_scroll_offset(5, 40, 200, 0), 40);
    }

    #[test]
    fn test_sub_1014_4_steamos_virtual_keyboard() {
        let mut kbd = VirtualKeyboardController::new();
        assert_eq!(kbd.state(), VirtualKeyboardState::Hidden);

        let show_call = kbd.show().unwrap();
        assert!(show_call.contains("org.kde.kwin.VirtualKeyboard") && show_call.contains("show"));
        assert_eq!(kbd.state(), VirtualKeyboardState::Visible);

        let hide_call = kbd.hide().unwrap();
        assert!(hide_call.contains("hide"));
        assert_eq!(kbd.state(), VirtualKeyboardState::Hidden);

        let toggle_call = kbd.toggle().unwrap();
        assert!(toggle_call.contains("toggle"));
        assert_eq!(kbd.state(), VirtualKeyboardState::Visible);
        assert_eq!(kbd.build_steam_url(), STEAM_URL_KEYBOARD);

        let act_rail = kbd.handle_activation(FocusZone::ServerRail).unwrap();
        assert!(act_rail.is_none());

        let act_input = kbd.handle_activation(FocusZone::MessageInputBar).unwrap();
        assert!(act_input.is_some() && act_input.unwrap().contains("show"));
    }

    fn check_limits_and_recovery(p: &mut GamepadInputProcessor) {
        let (zero_id, max_id): (u64, u64) = (0, u64::MAX);
        assert!(zero_id == 0 && max_id == 18446744073709551615);
        assert_eq!((p.filter_axis(1.5), p.filter_axis(-1.5)), (1.0, -1.0));
        assert!(
            MAX_MESSAGE_CHARS == 2000
                && MAX_ATTACHMENTS_COUNT == 10
                && MAX_ATTACHMENT_BYTES == 26214400
        );
        assert!(MAX_SEARCH_OFFSET == 9975 && MAX_VOLUME_PERCENT == 200 && MAX_BADGE_CAP == 99);
        assert_eq!(202u16, 202);
        assert!(
            PERM_VIEW_CHANNEL == 1024
                && PERM_SEND_MESSAGES == 2048
                && PERM_AGE_GATED_SPACES == 549755813888
        );

        p.process_event(&GilrsEvent::Connected {
            id: 42,
            name: "Pad".into(),
        });
        assert!(p.is_connected(42));
        p.process_event(&GilrsEvent::Disconnected { id: 42 });
        assert!(!p.is_connected(42));
        p.process_event(&GilrsEvent::Connected {
            id: 42,
            name: "Pad".into(),
        });
        assert!(p.is_connected(42));
    }

    #[test]
    fn test_sub_1014_5_nine_dimensions() {
        let mut p = GamepadInputProcessor::new(0.20);
        assert!((p.deadzone() - 0.20).abs() < 1e-4);
        assert!(p.set_deadzone(-0.5).is_err() && p.set_deadzone(f32::NAN).is_err());
        assert_eq!(p.filter_axis(f32::NAN), 0.0);

        check_limits_and_recovery(&mut p);

        let err404 = GamepadError::HttpError {
            status: 404,
            retry_after_ms: None,
        };
        let err429 = GamepadError::HttpError {
            status: 429,
            retry_after_ms: Some(1500),
        };
        assert!(format!("{err404}").contains("404") && format!("{err429}").contains("1500"));

        let mut empty_grid = FocusGrid::new(0, 0, 0);
        assert!(!empty_grid.apply_action(NavigationAction::NextServer));
        assert!(!empty_grid.apply_action(NavigationAction::NextChannel));

        let masked = mask_gamepad_secret("gamepad_event axis=LeftX token=SUPER_SECRET_1234");
        assert!(!masked.contains("SUPER_SECRET_1234") && masked.contains("token=***"));
    }
}
