//! Contract tests for Task R6-03: Windows Platform (Windows-specific).
//!
//! Covers:
//! 1. Taskbar badge overlays (unread badge visible on taskbar icon, cleared when 0).
//! 2. Portable mode (isolated data alongside executable; does not touch AppData or registry).
//! 3. Alt+F4 / Ctrl+Q hotkeys.

use std::path::PathBuf;

use chord_platform::unread_badge::{UnreadBadgeDisplay, WindowsTaskbarOverlay};
use chord_platform::windows::{
    handle_hotkey, parse_hotkey, HotkeyAction, InMemoryTaskbarOverlayBackend, PortableMode,
    WindowsHotkey, WindowsStoragePaths, WindowsTaskbarBadgeController,
};

#[test]
fn r6_03_taskbar_badge_overlay_visible_when_unread_and_cleared_when_zero() {
    let backend = InMemoryTaskbarOverlayBackend::default();
    let mut controller = WindowsTaskbarBadgeController::new(backend);

    assert!(!controller.is_badge_visible());
    assert_eq!(controller.current_overlay(), &WindowsTaskbarOverlay::Cleared);

    // Unread count > 0 -> badge visible on taskbar icon
    controller
        .update_badge(UnreadBadgeDisplay::Numeric(5))
        .expect("update_badge(5) should succeed");
    assert!(controller.is_badge_visible());
    assert_eq!(
        controller.current_overlay(),
        &WindowsTaskbarOverlay::from_display(UnreadBadgeDisplay::Numeric(5))
    );
    assert_eq!(controller.backend().apply_calls(), 1);

    // Overflow count -> still visible
    controller
        .update_badge(UnreadBadgeDisplay::OverflowDot)
        .expect("update_badge(OverflowDot) should succeed");
    assert!(controller.is_badge_visible());
    assert_eq!(
        controller.current_overlay(),
        &WindowsTaskbarOverlay::from_display(UnreadBadgeDisplay::OverflowDot)
    );

    // Cleared -> badge no longer visible
    controller
        .update_badge(UnreadBadgeDisplay::Cleared)
        .expect("update_badge(Cleared) should succeed");
    assert!(!controller.is_badge_visible());
    assert_eq!(controller.current_overlay(), &WindowsTaskbarOverlay::Cleared);
    assert_eq!(controller.backend().clear_calls(), 1);
}

#[test]
fn r6_03_portable_mode_isolates_data_alongside_executable_without_appdata_or_registry() {
    let exe_dir = PathBuf::from(r"C:\PortableApps\Chord");
    let paths = WindowsStoragePaths::resolve(PortableMode::Portable {
        exe_dir: exe_dir.clone(),
    });

    assert!(paths.is_portable());
    assert!(!paths.touches_app_data());
    assert!(!paths.touches_registry());

    assert!(paths.config_dir().starts_with(&exe_dir));
    assert!(paths.data_dir().starts_with(&exe_dir));
    assert!(paths.cache_dir().starts_with(&exe_dir));
    assert!(paths.logs_dir().starts_with(&exe_dir));

    // Standard mode touches AppData and registry
    let app_data = PathBuf::from(r"C:\Users\Alice\AppData\Roaming\Chord");
    let std_paths = WindowsStoragePaths::resolve(PortableMode::Standard {
        app_data_dir: app_data.clone(),
    });

    assert!(!std_paths.is_portable());
    assert!(std_paths.touches_app_data());
    assert!(std_paths.touches_registry());
    assert!(std_paths.config_dir().starts_with(&app_data));
}

#[test]
fn r6_03_alt_f4_and_ctrl_q_hotkeys() {
    assert_eq!(parse_hotkey(&["Alt"], "F4"), Some(WindowsHotkey::AltF4));
    assert_eq!(parse_hotkey(&["alt"], "f4"), Some(WindowsHotkey::AltF4));
    assert_eq!(parse_hotkey(&["Ctrl"], "Q"), Some(WindowsHotkey::CtrlQ));
    assert_eq!(parse_hotkey(&["ctrl"], "q"), Some(WindowsHotkey::CtrlQ));
    assert_eq!(parse_hotkey(&["Ctrl", "Shift"], "I"), None);

    // Alt+F4 closes window (or minimizes to tray if configured)
    assert_eq!(
        handle_hotkey(WindowsHotkey::AltF4, false),
        HotkeyAction::CloseWindow
    );
    assert_eq!(
        handle_hotkey(WindowsHotkey::AltF4, true),
        HotkeyAction::MinimizeToTray
    );

    // Ctrl+Q always quits the application, even if minimize_to_tray is true
    assert_eq!(
        handle_hotkey(WindowsHotkey::CtrlQ, false),
        HotkeyAction::QuitApplication
    );
    assert_eq!(
        handle_hotkey(WindowsHotkey::CtrlQ, true),
        HotkeyAction::QuitApplication
    );
}
