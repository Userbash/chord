use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn make_temp_config_dir(suffix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "discord_gpui_autostart_1015_{}_{}_{}",
        std::process::id(),
        suffix,
        nanos
    ));
    let _ = fs::create_dir_all(&dir);
    dir
}

fn remove_temp_config_dir(path: &Path) {
    let target = path
        .canonicalize()
        .expect("temporary configuration directory");
    let parent = std::env::temp_dir().canonicalize().expect("temporary root");
    assert_eq!(target.parent(), Some(parent.as_path()));
    assert!(
        target
            .file_name()
            .expect("temporary directory name")
            .to_string_lossy()
            .starts_with("discord_gpui_autostart_1015_")
    );
    fs::remove_dir_all(target).expect("remove temporary configuration directory");
}

#[test]
fn test_sub_1015_1_oracle_and_tokens() {
    assert_eq!(AUTOSTART_SERVICE_CONTAINER, "Autostart Service");
    assert_eq!(AUTOSTART_SERVICE_DIMENSIONS, "CLI & Config");
    assert_eq!(AUTOSTART_CONFIG_CONTAINER, "Autostart Config");
    assert_eq!(AUTOSTART_CONFIG_DIMENSIONS, "Desktop Entry");
    assert_eq!(HEADLESS_INIT_CONTAINER, "Headless Init");
    assert_eq!(HEADLESS_INIT_DIMENSIONS, "RAM < 8MB");
    assert_eq!(TOKEN_BG, "#1e1f22");
    assert_eq!(TOKEN_SURFACE, "#2b2d31");
    assert_eq!(TOKEN_TEXT_PRIMARY, "#ffffff");
    assert_eq!(TOKEN_ACCENT, "#5865f2");
    assert_eq!(TOKEN_BORDER, "#3f4147");
    assert_eq!(AutostartToggleUiState::Off.as_label(), "off");
    assert_eq!(
        AutostartToggleUiState::HighlightToggle.as_label(),
        "highlight toggle"
    );
    assert_eq!(AutostartToggleUiState::Enabled.as_label(), "enabled");
    assert_eq!(AutostartFileUiState::Absent.as_label(), "absent");
    assert_eq!(AutostartFileUiState::Writing.as_label(), "writing");
    assert_eq!(AutostartFileUiState::Registered.as_label(), "registered");
    assert_eq!(TrayIconUiState::IdleInTray.as_label(), "idle in tray");
    assert_eq!(TrayIconUiState::Tooltip.as_label(), "tooltip");
    assert_eq!(TrayIconUiState::WindowMapped.as_label(), "window mapped");
}

#[test]
fn test_sub_1015_2_xdg_desktop_file_lifecycle() {
    let temp_cfg = make_temp_config_dir("sub2");
    let spec = XdgDesktopEntrySpec::new("/opt/Discord GPUI/discord-gpui", true);
    let mut mgr = XdgAutostartManager::new(temp_cfg.clone(), spec);
    assert_eq!(mgr.file_state(), AutostartFileUiState::Absent);
    assert_eq!(mgr.toggle_state(false), AutostartToggleUiState::Off);
    assert_eq!(
        mgr.toggle_state(true),
        AutostartToggleUiState::HighlightToggle
    );

    let path = mgr.install().unwrap();
    assert!(path.is_file());
    assert_eq!(mgr.file_state(), AutostartFileUiState::Registered);
    assert_eq!(mgr.toggle_state(false), AutostartToggleUiState::Enabled);
    assert_eq!(
        read_desktop_permissions_mode(&path).unwrap(),
        DESKTOP_FILE_MODE
    );

    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("[Desktop Entry]\nType=Application\n"));
    assert!(text.contains("Exec=\"/opt/Discord GPUI/discord-gpui\" --start-minimized\n"));
    assert!(text.contains("X-GNOME-Autostart-enabled=true\n"));
    assert!(text.contains("Terminal=false\n"));

    verify_desktop_file_update_and_uninstall(&mut mgr, &path);
    remove_temp_config_dir(&temp_cfg);
}

fn verify_desktop_file_update_and_uninstall(mgr: &mut XdgAutostartManager, path: &Path) {
    mgr.set_start_minimized(false).unwrap();
    let text_no_min = fs::read_to_string(path).unwrap();
    assert!(text_no_min.contains("Exec=\"/opt/Discord GPUI/discord-gpui\"\n"));
    assert!(!text_no_min.contains("--start-minimized"));

    mgr.spec.enabled = false;
    mgr.install().unwrap();
    let text_disabled = fs::read_to_string(path).unwrap();
    assert!(text_disabled.contains("X-GNOME-Autostart-enabled=false\n"));

    let removed = mgr.uninstall().unwrap();
    assert!(removed);
    assert!(!path.exists());
    assert_eq!(mgr.file_state(), AutostartFileUiState::Absent);
}

#[test]
fn test_sub_1015_3_minimized_tray_launch() {
    let flags = LaunchCliFlags::parse(["discord-gpui", "--start-minimized"]);
    assert!(flags.start_minimized);
    assert!(!flags.start_inactive);

    let mut rt = MinimizedTrayRuntime::launch(&flags, false, true, 6_400).unwrap();
    assert!(!rt.wgpu_window_mapped);
    assert!(!rt.window_focused);
    assert!(rt.skip_taskbar);
    assert_eq!(rt.tray_ui_state(), TrayIconUiState::IdleInTray);

    rt.on_tray_hover(true);
    assert_eq!(rt.tray_ui_state(), TrayIconUiState::Tooltip);

    rt.on_tray_click();
    assert!(rt.wgpu_window_mapped);
    assert!(rt.window_focused);
    assert!(!rt.skip_taskbar);
    assert_eq!(rt.tray_ui_state(), TrayIconUiState::WindowMapped);

    assert!(!rt.on_window_close_requested());
    assert!(!rt.wgpu_window_mapped);
    assert!(rt.skip_taskbar);

    rt.on_second_instance_activate();
    assert!(rt.wgpu_window_mapped);
    assert!(rt.window_focused);

    rt.minimize_to_tray = false;
    assert!(rt.on_window_close_requested());
    assert!(rt.quit_requested);
}

#[test]
fn test_sub_1015_4_ipc_and_nine_dimensions() {
    let temp_cfg = make_temp_config_dir("sub4");
    let spec = XdgDesktopEntrySpec::new("/usr/bin/discord-gpui", true);
    let mut mgr = XdgAutostartManager::new(temp_cfg.clone(), spec);

    assert!(!mgr.handle_ipc(IPC_APP_GET_OPEN_ON_START, None).unwrap());
    assert!(
        mgr.handle_ipc(IPC_TOGGLE_OPEN_ON_STARTUP, Some(true))
            .unwrap()
    );
    assert!(mgr.handle_ipc(IPC_APP_GET_OPEN_ON_START, None).unwrap());
    assert!(
        !mgr.handle_ipc(IPC_TOGGLE_START_MINIMIZED, Some(false))
            .unwrap()
    );
    assert!(
        !mgr.handle_ipc(IPC_TOGGLE_MINIMIZE_TO_TRAY, Some(false))
            .unwrap()
    );
    assert!(mgr.handle_ipc(IPC_UPDATE_OPEN_ON_STARTUP, None).unwrap());
    assert!(
        !mgr.handle_ipc(IPC_TOGGLE_OPEN_ON_STARTUP, Some(false))
            .unwrap()
    );
    assert!(!mgr.is_installed());

    check_nine_dimensions_validation_and_ram();
    check_nine_dimensions_limits_and_security();
    remove_temp_config_dir(&temp_cfg);
}

fn check_nine_dimensions_validation_and_ram() {
    let bad_empty = XdgDesktopEntrySpec::new("", true);
    assert_eq!(bad_empty.validate(), Err(AutostartError::EmptyExecPath));

    let bad_rel = XdgDesktopEntrySpec::new("relative/bin/discord-gpui", true);
    assert_eq!(bad_rel.validate(), Err(AutostartError::RelativeExecPath));

    let mut bad_ctrl = XdgDesktopEntrySpec::new("/usr/bin/discord-gpui", true);
    bad_ctrl.app_name = "Discord\nInjected=true".to_string();
    assert_eq!(
        bad_ctrl.validate(),
        Err(AutostartError::ControlCharacterInField("Name"))
    );

    let flags = LaunchCliFlags::parse(["--start-minimized"]);
    let ram_err = MinimizedTrayRuntime::launch(&flags, true, true, 9_000);
    assert_eq!(
        ram_err,
        Err(AutostartError::RamBudgetExceeded {
            actual_kb: 9_000,
            limit_kb: MINIMIZED_RAM_LIMIT_KB,
        })
    );
    assert!(MinimizedTrayRuntime::launch(&flags, true, true, MINIMIZED_RAM_LIMIT_KB).is_ok());
}

fn check_nine_dimensions_limits_and_security() {
    let snowflake_min: u64 = 0;
    let snowflake_max: u64 = u64::MAX;
    assert_eq!(snowflake_min, 0);
    assert_eq!(snowflake_max, 18_446_744_073_709_551_615);
    assert_eq!(MAX_MESSAGE_CHARS, 2_000);
    assert_eq!(MAX_ATTACHMENTS_COUNT, 10);
    assert_eq!(MAX_ATTACHMENT_BYTES, 25 * 1024 * 1024);
    assert_eq!(MAX_SEARCH_OFFSET, 9_975);
    assert_eq!(MAX_VOLUME_PERCENT, 200);
    assert_eq!(MAX_BADGE_CAP, 99);
    assert_eq!(PERM_AGE_GATED_SPACES, 1 << 39);

    let portal_cmd = build_portal_background_dbus_request(true, true);
    assert!(portal_cmd.contains("org.freedesktop.portal.Background.RequestBackground"));
    assert!(portal_cmd.contains("--start-minimized"));

    let masked =
        mask_autostart_secrets("discord-gpui --start-minimized --token=secret123 mfa.abcd");
    assert!(!masked.contains("secret123"));
    assert!(!masked.contains("mfa.abcd"));
    assert!(masked.contains("token=[REDACTED]"));
}
