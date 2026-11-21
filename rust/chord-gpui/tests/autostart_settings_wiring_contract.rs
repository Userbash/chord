//! Contract Test: Autostart XDG Wiring in `SettingsDialog` (`P1-4.2`).
//!
//! Verifies:
//! 1. `AutostartSettingsController` lifecycle, getters, setters, IPC channels, and disk persistence.
//! 2. `SettingsDialog` autostart section view-model snapshot and interactive toggle actions.
//! 3. 9 mandatory dimensions: positive creation (0o644), flags update, removal, reload from disk,
//!    tray toggle, validation error handling, secret redaction, unwritable directory recovery,
//!    and view-model wiring.

#![recursion_limit = "512"]

#[path = "../src/audio/mod.rs"]
mod audio;
#[path = "../src/dispatch/mod.rs"]
mod dispatch;
#[path = "../src/media/mod.rs"]
mod media;
#[path = "../src/net/mod.rs"]
mod net;
#[path = "../src/platform/mod.rs"]
mod platform;
#[path = "../src/state/mod.rs"]
mod state;
#[path = "../src/storage/mod.rs"]
mod storage;
#[path = "../src/theme/mod.rs"]
mod theme;
#[path = "../src/ui/mod.rs"]
mod ui;
#[path = "../src/video/mod.rs"]
mod video;

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use platform::autostart::{
    AutostartError, AutostartFileUiState, AutostartSettingsController, CLI_FLAG_START_MINIMIZED,
    DESKTOP_FILE_MODE, DESKTOP_FILE_NAME, XdgDesktopEntrySpec, read_desktop_permissions_mode,
};
use ui::settings_dialog::{AutostartToggleKind, SettingsDialog};

fn make_temp_test_config_dir(suffix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "chord_test_xdg_autostart_{}_{}_{}",
        std::process::id(),
        suffix,
        nanos
    ));
    let _ = fs::create_dir_all(&dir);
    dir
}

/// Dimension 1: Positive Happy Path
/// Enabling `open_on_startup` creates `<config_home>/autostart/discord-gpui.desktop`
/// with 0o644 permissions, `Exec=... --start-minimized`, and GNOME enabled flag.
#[test]
fn test_dim1_positive_open_on_startup_creates_desktop_file() {
    let temp_dir = make_temp_test_config_dir("dim1");
    let spec = XdgDesktopEntrySpec::new("/usr/bin/discord-gpui", true);
    let mut ctrl = AutostartSettingsController::new(temp_dir.clone(), spec);

    assert!(!ctrl.open_on_startup());
    assert!(ctrl.start_minimized());
    assert!(ctrl.minimize_to_tray());
    assert!(!ctrl.snapshot().desktop_installed);
    assert_eq!(ctrl.snapshot().file_state, AutostartFileUiState::Absent);

    let installed = ctrl
        .set_open_on_startup(true)
        .expect("install should succeed");
    assert!(installed);
    assert!(ctrl.open_on_startup());

    let desktop_path = temp_dir.join("autostart").join(DESKTOP_FILE_NAME);
    assert!(desktop_path.is_file(), "desktop file must exist on disk");

    let perms = read_desktop_permissions_mode(&desktop_path).expect("read mode");
    assert_eq!(
        perms, DESKTOP_FILE_MODE,
        "desktop file permissions must be 0o644"
    );

    let content = fs::read_to_string(&desktop_path).expect("read file");
    assert!(content.contains("[Desktop Entry]"));
    assert!(content.contains("Exec=/usr/bin/discord-gpui --start-minimized"));
    assert!(content.contains("X-GNOME-Autostart-enabled=true"));
    assert!(content.contains("Terminal=false"));

    let snap = ctrl.snapshot();
    assert!(snap.desktop_installed);
    assert_eq!(snap.file_state, AutostartFileUiState::Registered);
    assert_eq!(snap.desktop_path, desktop_path);
    assert!(snap.last_error.is_none());

    let _ = fs::remove_dir_all(&temp_dir);
}

/// Dimension 2: Toggle `start_minimized`
/// Toggling updates `--start-minimized` in the desktop file's `Exec=` line.
#[test]
fn test_dim2_start_minimized_toggle_updates_exec_line() {
    let temp_dir = make_temp_test_config_dir("dim2");
    let spec = XdgDesktopEntrySpec::new("/usr/bin/discord-gpui", true);
    let mut ctrl = AutostartSettingsController::new(temp_dir.clone(), spec);

    ctrl.set_open_on_startup(true).expect("enable autostart");
    let desktop_path = temp_dir.join("autostart").join(DESKTOP_FILE_NAME);

    // Toggle start_minimized off
    let res = ctrl.set_start_minimized(false).expect("disable minimized");
    assert!(!res);
    assert!(!ctrl.start_minimized());

    let content_off = fs::read_to_string(&desktop_path).expect("read file");
    assert!(content_off.contains("Exec=/usr/bin/discord-gpui\n"));
    assert!(!content_off.contains(CLI_FLAG_START_MINIMIZED));

    // Toggle start_minimized back on
    let res_on = ctrl.set_start_minimized(true).expect("enable minimized");
    assert!(res_on);
    assert!(ctrl.start_minimized());

    let content_on = fs::read_to_string(&desktop_path).expect("read file");
    assert!(content_on.contains("Exec=/usr/bin/discord-gpui --start-minimized"));

    let _ = fs::remove_dir_all(&temp_dir);
}

/// Dimension 3: Disable Autostart (`open_on_startup = false`)
/// Setting open_on_startup to false cleanly removes the desktop file.
#[test]
fn test_dim3_open_on_startup_false_removes_desktop_file() {
    let temp_dir = make_temp_test_config_dir("dim3");
    let spec = XdgDesktopEntrySpec::new("/usr/bin/discord-gpui", true);
    let mut ctrl = AutostartSettingsController::new(temp_dir.clone(), spec);

    ctrl.set_open_on_startup(true).expect("enable autostart");
    let desktop_path = temp_dir.join("autostart").join(DESKTOP_FILE_NAME);
    assert!(desktop_path.is_file());

    let removed = ctrl.set_open_on_startup(false).expect("disable autostart");
    assert!(!removed);
    assert!(!ctrl.open_on_startup());
    assert!(!desktop_path.exists(), "desktop file must be removed");

    let snap = ctrl.snapshot();
    assert!(!snap.desktop_installed);
    assert_eq!(snap.file_state, AutostartFileUiState::Absent);

    let _ = fs::remove_dir_all(&temp_dir);
}

/// Dimension 4: Persistence and Reload From Disk
/// Reopening settings reloads the actual desktop file state from disk.
#[test]
fn test_dim4_reload_from_disk_preserves_persisted_state() {
    let temp_dir = make_temp_test_config_dir("dim4");
    let desktop_dir = temp_dir.join("autostart");
    fs::create_dir_all(&desktop_dir).expect("create autostart dir");
    let desktop_path = desktop_dir.join(DESKTOP_FILE_NAME);

    // Write a desktop file without --start-minimized
    let raw_desktop = "[Desktop Entry]\n\
        Type=Application\n\
        Version=1.5\n\
        Name=Discord GPUI\n\
        Exec=/usr/bin/discord-gpui\n\
        Terminal=false\n\
        X-GNOME-Autostart-enabled=true\n";
    fs::write(&desktop_path, raw_desktop).expect("write desktop file");

    let mut ctrl = AutostartSettingsController::for_config_home(temp_dir.clone());
    let snap = ctrl.reload_from_disk().expect("reload from disk");

    assert!(snap.open_on_startup);
    assert!(!snap.start_minimized, "file had no --start-minimized flag");
    assert!(snap.desktop_installed);

    // Test when file is deleted outside
    fs::remove_file(&desktop_path).expect("delete desktop file");
    let snap_after_del = ctrl.reload_from_disk().expect("reload after delete");
    assert!(!snap_after_del.open_on_startup);
    assert!(!snap_after_del.desktop_installed);

    let _ = fs::remove_dir_all(&temp_dir);
}

/// Dimension 5: Minimize to Tray Toggle
/// Toggling minimize_to_tray updates state and snapshot.
#[test]
fn test_dim5_minimize_to_tray_toggle() {
    let temp_dir = make_temp_test_config_dir("dim5");
    let spec = XdgDesktopEntrySpec::new("/usr/bin/discord-gpui", true);
    let mut ctrl = AutostartSettingsController::new(temp_dir.clone(), spec);

    assert!(ctrl.minimize_to_tray());
    ctrl.set_minimize_to_tray(false)
        .expect("disable minimize to tray");
    assert!(!ctrl.minimize_to_tray());
    assert!(!ctrl.snapshot().minimize_to_tray);

    ctrl.toggle_minimize_to_tray()
        .expect("toggle minimize to tray");
    assert!(ctrl.minimize_to_tray());
    assert!(ctrl.snapshot().minimize_to_tray);

    let _ = fs::remove_dir_all(&temp_dir);
}

/// Dimension 6: Negative & Edge Case Validation
/// Invalid exec path (empty, relative, control chars) returns Err(AutostartError)
/// and records last_error without crashing.
#[test]
fn test_dim6_negative_validation_and_error_recording() {
    let temp_dir = make_temp_test_config_dir("dim6");

    // 1. Relative exec path
    let bad_rel_spec = XdgDesktopEntrySpec::new("relative/bin/discord", true);
    let mut ctrl_rel = AutostartSettingsController::new(temp_dir.clone(), bad_rel_spec);
    let err_rel = ctrl_rel.set_open_on_startup(true).unwrap_err();
    assert_eq!(err_rel, AutostartError::RelativeExecPath);
    assert!(ctrl_rel.last_error().is_some());
    assert!(ctrl_rel.snapshot().last_error.is_some());

    // 2. Control characters in field
    let mut bad_ctrl_spec = XdgDesktopEntrySpec::new("/usr/bin/discord-gpui", true);
    bad_ctrl_spec.app_name = "Discord\nInjected=true".to_string();
    let mut ctrl_ctrl = AutostartSettingsController::new(temp_dir.clone(), bad_ctrl_spec);
    let err_ctrl = ctrl_ctrl.set_open_on_startup(true).unwrap_err();
    assert_eq!(err_ctrl, AutostartError::ControlCharacterInField("Name"));
    assert!(ctrl_ctrl.last_error().is_some());

    let _ = fs::remove_dir_all(&temp_dir);
}

/// Dimension 7: Secret Redaction
/// Sensitive tokens (e.g. --token=..., mfa....) in error messages or logs must be redacted.
#[test]
fn test_dim7_secret_redaction_in_errors_and_logs() {
    let temp_dir = make_temp_test_config_dir("dim7");
    let mut spec = XdgDesktopEntrySpec::new("/usr/bin/discord-gpui", true);
    spec.extra_args = vec![
        "--token=secret_super_token_987".to_string(),
        "mfa.auth_code_secret_xyz".to_string(),
    ];
    // Force an invalid relative exec path to trigger an error while carrying sensitive extra args
    spec.exec_path = "bin/discord-gpui".to_string();

    let mut ctrl = AutostartSettingsController::new(temp_dir.clone(), spec);
    let err = ctrl.set_open_on_startup(true).unwrap_err();
    assert_eq!(err, AutostartError::RelativeExecPath);

    let err_str = ctrl.last_error().expect("error recorded");
    assert!(
        !err_str.contains("secret_super_token_987"),
        "token secret must be masked"
    );
    assert!(
        !err_str.contains("auth_code_secret_xyz"),
        "mfa secret must be masked"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

/// Dimension 8: Unwritable / Permission-Denied Directory Handling
/// When autostart directory cannot be created or written, returns structured error and sets last_error.
#[test]
fn test_dim8_unwritable_directory_handling() {
    let forbidden_dir = PathBuf::from("/proc/forbidden_autostart_nonexistent");
    let spec = XdgDesktopEntrySpec::new("/usr/bin/discord-gpui", true);
    let mut ctrl = AutostartSettingsController::new(forbidden_dir, spec);

    let res = ctrl.set_open_on_startup(true);
    assert!(
        res.is_err(),
        "install must fail gracefully on unwritable path"
    );
    let snap = ctrl.snapshot();
    assert!(snap.last_error.is_some(), "last_error must be recorded");
    assert!(!snap.open_on_startup);
}

/// Dimension 9: UI View-Model & Interactive Action Helper Wiring
/// Tests `SettingsDialog::autostart_rows_snapshot` and `SettingsDialog::apply_autostart_toggle`.
#[test]
fn test_dim9_settings_dialog_view_model_and_action_helpers() {
    let temp_dir = make_temp_test_config_dir("dim9");
    let spec = XdgDesktopEntrySpec::new("/usr/bin/discord-gpui", true);
    let mut ctrl = AutostartSettingsController::new(temp_dir.clone(), spec);

    // Initial snapshot via SettingsDialog helper
    let vm_snap = SettingsDialog::autostart_rows_snapshot(&ctrl);
    assert_eq!(vm_snap.rows.len(), 3);
    assert_eq!(vm_snap.rows[0].title, SettingsDialog::TITLE_OPEN_ON_STARTUP);
    assert_eq!(vm_snap.rows[1].title, SettingsDialog::TITLE_START_MINIMIZED);
    assert_eq!(
        vm_snap.rows[2].title,
        SettingsDialog::TITLE_MINIMIZE_TO_TRAY
    );
    assert!(!vm_snap.rows[0].is_on);
    assert!(vm_snap.rows[1].is_on);
    assert!(vm_snap.rows[2].is_on);

    // Apply toggle: OpenOnStartup -> true
    let updated_snap = SettingsDialog::apply_autostart_toggle(
        &mut ctrl,
        AutostartToggleKind::OpenOnStartup,
        Some(true),
    )
    .expect("apply open_on_startup toggle");
    assert!(updated_snap.open_on_startup);

    let vm_snap2 = SettingsDialog::autostart_rows_snapshot(&ctrl);
    assert!(vm_snap2.rows[0].is_on);

    // Apply toggle without explicit value (inverts current): StartMinimized (true -> false)
    let updated_snap2 = SettingsDialog::apply_autostart_toggle(
        &mut ctrl,
        AutostartToggleKind::StartMinimized,
        None,
    )
    .expect("toggle start_minimized");
    assert!(!updated_snap2.start_minimized);

    let vm_snap3 = SettingsDialog::autostart_rows_snapshot(&ctrl);
    assert!(!vm_snap3.rows[1].is_on);

    // Apply toggle: MinimizeToTray (true -> false)
    let updated_snap3 = SettingsDialog::apply_autostart_toggle(
        &mut ctrl,
        AutostartToggleKind::MinimizeToTray,
        Some(false),
    )
    .expect("set minimize_to_tray false");
    assert!(!updated_snap3.minimize_to_tray);

    let vm_snap4 = SettingsDialog::autostart_rows_snapshot(&ctrl);
    assert!(!vm_snap4.rows[2].is_on);

    let _ = fs::remove_dir_all(&temp_dir);
}
