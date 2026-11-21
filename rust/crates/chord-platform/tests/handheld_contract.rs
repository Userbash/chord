//! Contract tests for Task R6-01: Steam Deck & Controller Mode (`chord-platform::handheld`).
//!
//! Verifies:
//! 1. Handheld UI mode detection & dynamic scaling for Steam Deck Game Mode (1280x800).
//! 2. Full platform navigation via gamepad D-Pad, thumbstick, bumpers, triggers, and face buttons.
//! 3. Automatic SteamOS on-screen keyboard summoning via D-Bus when input fields gain focus.

use chord_platform::gamepad_driver::{GilrsAxis, GilrsButton, GilrsEvent, GilrsEventType, NavAction};
use chord_platform::handheld::{
    DbusInvoker, HandheldLayoutMetrics, HandheldModeConfig, HandheldNavigator, PlatformFocusZone,
    SteamDeckEnv, VirtualKeyboardDbusClient, VirtualKeyboardState, DBUS_KWIN_INTERFACE,
    DBUS_KWIN_PATH, DBUS_KWIN_SERVICE, STEAM_URL_KEYBOARD,
};

#[derive(Default)]
struct MockDbusInvoker {
    calls: Vec<(String, String, String, String)>,
    should_fail: bool,
}

impl DbusInvoker for MockDbusInvoker {
    fn call_method(
        &mut self,
        service: &str,
        path: &str,
        interface: &str,
        method: &str,
    ) -> Result<(), String> {
        if self.should_fail {
            return Err("org.freedesktop.DBus.Error.ServiceUnknown".to_string());
        }
        self.calls.push((
            service.to_string(),
            path.to_string(),
            interface.to_string(),
            method.to_string(),
        ));
        Ok(())
    }
}

#[test]
fn steam_deck_game_mode_detection_and_dynamic_scaling() {
    let env = SteamDeckEnv {
        steam_deck: true,
        gamescope_wayland_display: Some("gamescope-0".to_string()),
        xdg_current_desktop: Some("gamescope".to_string()),
    };
    let config = HandheldModeConfig::from_env(&env);
    assert!(config.is_handheld_mode);
    assert!(config.is_game_mode);

    // Native Steam Deck 1280x800
    let deck_metrics = HandheldLayoutMetrics::for_viewport(1280, 800, &config);
    assert!((deck_metrics.ui_scale - 1.25).abs() < f32::EPSILON);
    assert!(deck_metrics.min_focus_target_px >= 44);
    assert_eq!(deck_metrics.viewport_width, 1280);
    assert_eq!(deck_metrics.viewport_height, 800);

    // Docked external 1080p display
    let docked_metrics = HandheldLayoutMetrics::for_viewport(1920, 1080, &config);
    assert!(docked_metrics.ui_scale >= 1.0 && docked_metrics.ui_scale <= 2.0);
}

#[test]
fn full_navigation_via_gamepad_dpad_thumbstick_and_buttons() {
    let mut nav = HandheldNavigator::new(3, 4);
    let id = 1;

    // Connect gamepad
    nav.handle_event(GilrsEvent {
        id,
        timestamp_ms: 10,
        event: GilrsEventType::Connected,
    });

    assert_eq!(nav.current_zone(), PlatformFocusZone::ServerRail);

    // D-Pad Right moves across zones: ServerRail -> ChannelSidebar -> ChatTimeline -> MessageInputBar
    let mut invoker = MockDbusInvoker::default();
    let mut kbd = VirtualKeyboardDbusClient::new();

    for (ts, expected_zone) in [
        (100, PlatformFocusZone::ChannelSidebar),
        (250, PlatformFocusZone::ChatTimeline),
        (400, PlatformFocusZone::MessageInputBar),
    ] {
        let action = nav.handle_event(GilrsEvent {
            id,
            timestamp_ms: ts,
            event: GilrsEventType::ButtonPressed(GilrsButton::DPadRight),
        });
        assert_eq!(action, Some(NavAction::NavRight));
        assert_eq!(nav.current_zone(), expected_zone);
    }

    // Focusing MessageInputBar automatically summons SteamOS on-screen keyboard
    let summoned = kbd
        .on_focus_changed(nav.current_zone(), true, &mut invoker)
        .unwrap();
    assert!(summoned);
    assert_eq!(kbd.state(), VirtualKeyboardState::Visible);
    assert_eq!(invoker.calls.len(), 1);
    assert_eq!(
        invoker.calls[0],
        (
            DBUS_KWIN_SERVICE.to_string(),
            DBUS_KWIN_PATH.to_string(),
            DBUS_KWIN_INTERFACE.to_string(),
            "show".to_string()
        )
    );

    // Left thumbstick deflection navigates back left to ChatTimeline and hides keyboard
    let stick_action = nav.handle_event(GilrsEvent {
        id,
        timestamp_ms: 550,
        event: GilrsEventType::AxisChanged(GilrsAxis::LeftStickX, -0.85),
    });
    assert_eq!(stick_action, Some(NavAction::NavLeft));
    assert_eq!(nav.current_zone(), PlatformFocusZone::ChatTimeline);

    let hidden = kbd
        .on_focus_changed(nav.current_zone(), false, &mut invoker)
        .unwrap();
    assert!(!hidden);
    assert_eq!(kbd.state(), VirtualKeyboardState::Hidden);
    assert_eq!(invoker.calls.last().unwrap().3, "hide");

    // Bumpers switch servers, triggers switch channels
    assert_eq!(
        nav.handle_event(GilrsEvent {
            id,
            timestamp_ms: 700,
            event: GilrsEventType::ButtonPressed(GilrsButton::RightTrigger),
        }),
        Some(NavAction::NextServer)
    );
    assert_eq!(nav.server_index(), 1);

    assert_eq!(
        nav.handle_event(GilrsEvent {
            id,
            timestamp_ms: 850,
            event: GilrsEventType::ButtonPressed(GilrsButton::RightTrigger2),
        }),
        Some(NavAction::NextChannel)
    );
    assert_eq!(nav.channel_index(), 1);
}

#[test]
fn virtual_keyboard_dbus_failure_records_error_and_provides_steam_url_fallback() {
    let mut invoker = MockDbusInvoker {
        should_fail: true,
        ..Default::default()
    };
    let mut kbd = VirtualKeyboardDbusClient::new();

    let res = kbd.on_focus_changed(PlatformFocusZone::MessageInputBar, true, &mut invoker);
    assert!(res.is_err());
    assert!(matches!(kbd.state(), VirtualKeyboardState::Failed(_)));
    assert_eq!(kbd.steam_url_fallback(), STEAM_URL_KEYBOARD);
}
