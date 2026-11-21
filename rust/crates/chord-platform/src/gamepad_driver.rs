//! `gilrs`-compatible gamepad driver with axis deadzone filtering and button debouncing (`TASK-1014A`).
//!
//! Maps D-pad and analog stick inputs to spatial navigation (`NavUp`, `NavDown`, `NavLeft`, `NavRight`),
//! L1/R1 bumpers to server switching (`PrevServer`, `NextServer`), and L2/R2 triggers to channel switching
//! (`PrevChannel`, `NextChannel`).

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

/// Default analog stick deadzone to suppress hardware drift.
pub const DEFAULT_DEADZONE: f32 = 0.15;

/// Default button debounce window in milliseconds to suppress chatter.
pub const DEFAULT_DEBOUNCE_MS: u64 = 120;

/// Minimum filtered axis deflection required to emit a directional navigation event.
pub const AXIS_DEFLECTION_THRESHOLD: f32 = 0.50;

/// Gamepad buttons modeled after `gilrs::Button`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GilrsButton {
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
    /// L1 bumper.
    LeftTrigger,
    /// R1 bumper.
    RightTrigger,
    /// L2 trigger.
    LeftTrigger2,
    /// R2 trigger.
    RightTrigger2,
    South,
    East,
    West,
    North,
    Start,
    Select,
}

/// Gamepad axes modeled after `gilrs::Axis`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GilrsAxis {
    LeftStickX,
    LeftStickY,
    RightStickX,
    RightStickY,
    DPadX,
    DPadY,
}

/// Raw gamepad event modeled after `gilrs::EventType`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GilrsEventType {
    Connected,
    Disconnected,
    ButtonPressed(GilrsButton),
    ButtonReleased(GilrsButton),
    AxisChanged(GilrsAxis, f32),
}

/// Timestamped gamepad event from a `gilrs` backend.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GilrsEvent {
    pub id: u64,
    pub timestamp_ms: u64,
    pub event: GilrsEventType,
}

/// High-level navigation actions emitted by the gamepad driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NavAction {
    NavUp,
    NavDown,
    NavLeft,
    NavRight,
    PrevServer,
    NextServer,
    PrevChannel,
    NextChannel,
    Select,
    Back,
    OpenKeyboard,
}

/// Stateful `gilrs` gamepad driver with deadzone, debounce, and disconnect handling.
#[derive(Debug, Clone)]
pub struct GamepadDriver {
    deadzone: f32,
    debounce_ms: u64,
    connected_ids: BTreeSet<u64>,
    last_button_ms: BTreeMap<(u64, GilrsButton), u64>,
    last_axis_nav: BTreeMap<(u64, GilrsAxis), NavAction>,
}

impl Default for GamepadDriver {
    fn default() -> Self {
        Self::new(DEFAULT_DEADZONE, DEFAULT_DEBOUNCE_MS)
    }
}

impl GamepadDriver {
    /// Creates a new `GamepadDriver` with the given deadzone and debounce window.
    pub fn new(deadzone: f32, debounce_ms: u64) -> Self {
        Self {
            deadzone: deadzone.clamp(0.0, 0.95),
            debounce_ms,
            connected_ids: BTreeSet::new(),
            last_button_ms: BTreeMap::new(),
            last_axis_nav: BTreeMap::new(),
        }
    }

    /// Returns the number of currently connected gamepads.
    pub fn connected_count(&self) -> usize {
        self.connected_ids.len()
    }

    /// Returns whether a specific gamepad ID is currently connected.
    pub fn is_connected(&self, id: u64) -> bool {
        self.connected_ids.contains(&id)
    }

    /// Filters an analog axis value using the configured deadzone.
    pub fn filter_axis(&self, value: f32) -> f32 {
        if !value.is_finite() {
            return 0.0;
        }
        if value.abs() < self.deadzone {
            return 0.0;
        }
        value.clamp(-1.0, 1.0)
    }

    /// Processes a single `GilrsEvent` and returns an optional `NavAction`.
    pub fn process_event(&mut self, ev: GilrsEvent) -> Option<NavAction> {
        match ev.event {
            GilrsEventType::Connected => {
                self.connected_ids.insert(ev.id);
                None
            }
            GilrsEventType::Disconnected => {
                self.handle_disconnect(ev.id);
                None
            }
            GilrsEventType::ButtonPressed(button) => {
                self.connected_ids.insert(ev.id);
                self.handle_button_press(ev.id, button, ev.timestamp_ms)
            }
            GilrsEventType::ButtonReleased(_) => None,
            GilrsEventType::AxisChanged(axis, raw_value) => {
                self.connected_ids.insert(ev.id);
                self.handle_axis_change(ev.id, axis, raw_value)
            }
        }
    }

    fn handle_disconnect(&mut self, id: u64) {
        self.connected_ids.remove(&id);
        self.last_button_ms.retain(|(gid, _), _| *gid != id);
        self.last_axis_nav.retain(|(gid, _), _| *gid != id);
    }

    fn handle_button_press(
        &mut self,
        id: u64,
        button: GilrsButton,
        now_ms: u64,
    ) -> Option<NavAction> {
        if let Some(&last_ms) = self.last_button_ms.get(&(id, button)) {
            if now_ms.saturating_sub(last_ms) < self.debounce_ms {
                return None;
            }
        }
        self.last_button_ms.insert((id, button), now_ms);
        map_button_to_action(button)
    }

    fn handle_axis_change(
        &mut self,
        id: u64,
        axis: GilrsAxis,
        raw_value: f32,
    ) -> Option<NavAction> {
        let filtered = self.filter_axis(raw_value);
        let action = map_axis_to_action(axis, filtered);
        match action {
            Some(nav) => {
                let prev = self.last_axis_nav.insert((id, axis), nav);
                if prev == Some(nav) {
                    None
                } else {
                    Some(nav)
                }
            }
            None => {
                self.last_axis_nav.remove(&(id, axis));
                None
            }
        }
    }
}

/// Maps a `GilrsButton` to a `NavAction`.
pub fn map_button_to_action(button: GilrsButton) -> Option<NavAction> {
    match button {
        GilrsButton::DPadUp => Some(NavAction::NavUp),
        GilrsButton::DPadDown => Some(NavAction::NavDown),
        GilrsButton::DPadLeft => Some(NavAction::NavLeft),
        GilrsButton::DPadRight => Some(NavAction::NavRight),
        GilrsButton::LeftTrigger => Some(NavAction::PrevServer),
        GilrsButton::RightTrigger => Some(NavAction::NextServer),
        GilrsButton::LeftTrigger2 => Some(NavAction::PrevChannel),
        GilrsButton::RightTrigger2 => Some(NavAction::NextChannel),
        GilrsButton::South => Some(NavAction::Select),
        GilrsButton::East => Some(NavAction::Back),
        GilrsButton::West => Some(NavAction::OpenKeyboard),
        GilrsButton::North | GilrsButton::Start | GilrsButton::Select => None,
    }
}

fn map_axis_to_action(axis: GilrsAxis, filtered: f32) -> Option<NavAction> {
    if filtered.abs() < AXIS_DEFLECTION_THRESHOLD {
        return None;
    }
    match axis {
        GilrsAxis::LeftStickX | GilrsAxis::DPadX => {
            if filtered < 0.0 {
                Some(NavAction::NavLeft)
            } else {
                Some(NavAction::NavRight)
            }
        }
        GilrsAxis::LeftStickY | GilrsAxis::DPadY => {
            if filtered > 0.0 {
                Some(NavAction::NavUp)
            } else {
                Some(NavAction::NavDown)
            }
        }
        GilrsAxis::RightStickX | GilrsAxis::RightStickY => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deadzone_filters_analog_stick_drift() {
        let driver = GamepadDriver::default();
        assert_eq!(driver.filter_axis(0.05), 0.0);
        assert_eq!(driver.filter_axis(-0.14), 0.0);
        assert_eq!(driver.filter_axis(f32::NAN), 0.0);
        assert!((driver.filter_axis(0.75) - 0.75).abs() < f32::EPSILON);
    }

    #[test]
    fn dpad_maps_to_nav_directions() {
        let mut driver = GamepadDriver::default();
        let cases = [
            (GilrsButton::DPadUp, NavAction::NavUp, 100),
            (GilrsButton::DPadDown, NavAction::NavDown, 300),
            (GilrsButton::DPadLeft, NavAction::NavLeft, 500),
            (GilrsButton::DPadRight, NavAction::NavRight, 700),
        ];
        for (btn, expected, ts) in cases {
            let act = driver.process_event(GilrsEvent {
                id: 1,
                timestamp_ms: ts,
                event: GilrsEventType::ButtonPressed(btn),
            });
            assert_eq!(act, Some(expected));
        }
    }

    #[test]
    fn bumpers_map_to_servers_and_triggers_map_to_channels() {
        let mut driver = GamepadDriver::default();
        assert_eq!(
            driver.process_event(GilrsEvent {
                id: 1,
                timestamp_ms: 100,
                event: GilrsEventType::ButtonPressed(GilrsButton::LeftTrigger),
            }),
            Some(NavAction::PrevServer)
        );
        assert_eq!(
            driver.process_event(GilrsEvent {
                id: 1,
                timestamp_ms: 300,
                event: GilrsEventType::ButtonPressed(GilrsButton::RightTrigger),
            }),
            Some(NavAction::NextServer)
        );
        assert_eq!(
            driver.process_event(GilrsEvent {
                id: 1,
                timestamp_ms: 500,
                event: GilrsEventType::ButtonPressed(GilrsButton::LeftTrigger2),
            }),
            Some(NavAction::PrevChannel)
        );
        assert_eq!(
            driver.process_event(GilrsEvent {
                id: 1,
                timestamp_ms: 700,
                event: GilrsEventType::ButtonPressed(GilrsButton::RightTrigger2),
            }),
            Some(NavAction::NextChannel)
        );
    }

    #[test]
    fn button_chatter_is_suppressed_within_debounce_window() {
        let mut driver = GamepadDriver::default();
        let first = driver.process_event(GilrsEvent {
            id: 1,
            timestamp_ms: 1000,
            event: GilrsEventType::ButtonPressed(GilrsButton::DPadUp),
        });
        let chatter = driver.process_event(GilrsEvent {
            id: 1,
            timestamp_ms: 1050,
            event: GilrsEventType::ButtonPressed(GilrsButton::DPadUp),
        });
        let after_window = driver.process_event(GilrsEvent {
            id: 1,
            timestamp_ms: 1200,
            event: GilrsEventType::ButtonPressed(GilrsButton::DPadUp),
        });
        assert_eq!(first, Some(NavAction::NavUp));
        assert_eq!(chatter, None);
        assert_eq!(after_window, Some(NavAction::NavUp));
    }

    #[test]
    fn disconnect_clears_active_gamepad_without_crashing() {
        let mut driver = GamepadDriver::default();
        driver.process_event(GilrsEvent {
            id: 42,
            timestamp_ms: 100,
            event: GilrsEventType::Connected,
        });
        assert!(driver.is_connected(42));
        assert_eq!(driver.connected_count(), 1);

        let res = driver.process_event(GilrsEvent {
            id: 42,
            timestamp_ms: 200,
            event: GilrsEventType::Disconnected,
        });
        assert_eq!(res, None);
        assert!(!driver.is_connected(42));
        assert_eq!(driver.connected_count(), 0);
    }

    #[test]
    fn analog_stick_deflection_emits_nav_once_until_neutral() {
        let mut driver = GamepadDriver::default();
        assert_eq!(
            driver.process_event(GilrsEvent {
                id: 1,
                timestamp_ms: 100,
                event: GilrsEventType::AxisChanged(GilrsAxis::LeftStickX, -0.8),
            }),
            Some(NavAction::NavLeft)
        );
        // Repeated deflection in same direction does not spam
        assert_eq!(
            driver.process_event(GilrsEvent {
                id: 1,
                timestamp_ms: 120,
                event: GilrsEventType::AxisChanged(GilrsAxis::LeftStickX, -0.9),
            }),
            None
        );
        // Return to neutral clears latch
        assert_eq!(
            driver.process_event(GilrsEvent {
                id: 1,
                timestamp_ms: 140,
                event: GilrsEventType::AxisChanged(GilrsAxis::LeftStickX, 0.05),
            }),
            None
        );
        assert_eq!(
            driver.process_event(GilrsEvent {
                id: 1,
                timestamp_ms: 160,
                event: GilrsEventType::AxisChanged(GilrsAxis::LeftStickX, -0.8),
            }),
            Some(NavAction::NavLeft)
        );
    }
}
