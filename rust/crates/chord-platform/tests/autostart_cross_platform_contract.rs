//! Contract Test: R4-04 System Autostart (Cross-Platform: Linux XDG/Portal, Windows Run, macOS LaunchAgent, --start-minimized).
//!
//! Independent oracles:
//! - Linux: XDG Desktop Entry 1.5 (`~/.config/autostart/<id>.desktop`, 0o644) & FreeDesktop Portal Background D-Bus spec.
//! - Windows: HKCU\Software\Microsoft\Windows\CurrentVersion\Run value quoting & registry command spec.
//! - macOS: LaunchAgent property list (`~/Library/LaunchAgents/<bundle_id>.plist`, XML DTD 1.0, RunAtLoad, ProgramArguments).
//! - Startup policy: `--start-minimized` / `-m` suppresses initial window creation/mapping (zero window flicker) until tray/second-instance activation.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use chord_platform::autostart::{
    build_portal_background_spec, macos_launch_agent_path, macos_launch_agent_plist,
    windows_run_command_line, windows_run_registry_entry, AutostartConfig, AutostartError,
    LaunchArgs, MacOsLaunchAgent, PortalBackgroundSpec, StartupWindowState,
    SystemAutostartController, TargetOs, WindowsRunEntry,
};

fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "chord_r4_04_autostart_{}_{}_{}",
        std::process::id(),
        label,
        nanos
    ));
    let _ = fs::create_dir_all(&dir);
    dir
}

fn sample_config(exec_path: &str, start_minimized: bool) -> AutostartConfig {
    AutostartConfig {
        app_name: "Chord".to_string(),
        app_id: "dev.chord.desktop".to_string(),
        exec_path: exec_path.to_string(),
        extra_args: Vec::new(),
        start_minimized,
        enabled: true,
    }
}

#[test]
fn r4_04_01_linux_xdg_and_portal_autostart_lifecycle() {
    let root = temp_dir("linux");
    let cfg = sample_config("/usr/bin/chord", true);
    let mut ctrl = SystemAutostartController::with_root(TargetOs::Linux, root.clone(), cfg);

    assert!(!ctrl.is_enabled());
    ctrl.set_enabled(true).expect("enable linux autostart");
    assert!(ctrl.is_enabled());

    let desktop_path = root.join("autostart").join("dev.chord.desktop.desktop");
    assert!(desktop_path.is_file(), "XDG .desktop file must be created");
    let content = fs::read_to_string(&desktop_path).expect("read .desktop");
    assert!(content.contains("[Desktop Entry]"));
    assert!(content.contains("Exec=/usr/bin/chord --start-minimized"));
    assert!(content.contains("X-GNOME-Autostart-enabled=true"));

    // Portal spec oracle
    let portal: PortalBackgroundSpec =
        build_portal_background_spec("dev.chord.desktop", true, true);
    assert_eq!(portal.bus_name, "org.freedesktop.portal.Desktop");
    assert_eq!(portal.interface, "org.freedesktop.portal.Background");
    assert_eq!(portal.method, "RequestBackground");
    assert!(portal.autostart);
    assert_eq!(
        portal.commandline,
        vec![
            "dev.chord.desktop".to_string(),
            "--start-minimized".to_string()
        ]
    );

    // Toggle start_minimized off updates .desktop Exec=
    ctrl.set_start_minimized(false)
        .expect("disable start_minimized");
    let updated = fs::read_to_string(&desktop_path).expect("read updated .desktop");
    assert!(updated.contains("Exec=/usr/bin/chord\n"));
    assert!(!updated.contains("--start-minimized"));

    // Disable removes .desktop
    ctrl.set_enabled(false).expect("disable linux autostart");
    assert!(!ctrl.is_enabled());
    assert!(!desktop_path.exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn r4_04_02_macos_launch_agent_plist_and_toggle() {
    let root = temp_dir("macos");
    let cfg = sample_config("/Applications/Chord.app/Contents/MacOS/chord", true);
    let mut ctrl = SystemAutostartController::with_root(TargetOs::MacOs, root.clone(), cfg.clone());

    let plist_path = macos_launch_agent_path(&root, &cfg.app_id);
    assert_eq!(
        plist_path,
        root.join("Library")
            .join("LaunchAgents")
            .join("dev.chord.desktop.plist")
    );

    ctrl.set_enabled(true).expect("enable macos autostart");
    assert!(ctrl.is_enabled());
    assert!(plist_path.is_file(), "LaunchAgent plist must exist");

    let plist = fs::read_to_string(&plist_path).expect("read plist");
    assert!(plist.contains("<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\""));
    assert!(plist.contains("<key>Label</key>\n\t<string>dev.chord.desktop</string>"));
    assert!(plist.contains("<string>/Applications/Chord.app/Contents/MacOS/chord</string>"));
    assert!(plist.contains("<string>--start-minimized</string>"));
    assert!(plist.contains("<key>RunAtLoad</key>\n\t<true/>"));

    // XML escaping oracle
    let xml_cfg = AutostartConfig {
        app_name: "Chord & Co".to_string(),
        app_id: "dev.chord.test&1".to_string(),
        exec_path: "/Applications/Chord & Co/chord".to_string(),
        extra_args: vec!["--profile=<main>".to_string()],
        start_minimized: false,
        enabled: true,
    };
    let agent = MacOsLaunchAgent::from_config(&xml_cfg).expect("valid macos agent");
    let rendered = macos_launch_agent_plist(&agent).expect("render plist");
    assert!(rendered.contains("dev.chord.test&amp;1"));
    assert!(rendered.contains("/Applications/Chord &amp; Co/chord"));
    assert!(rendered.contains("--profile=&lt;main&gt;"));

    ctrl.set_start_minimized(false)
        .expect("disable start_minimized on macos");
    let plist_no_min = fs::read_to_string(&plist_path).expect("read updated plist");
    assert!(!plist_no_min.contains("--start-minimized"));

    ctrl.set_enabled(false).expect("disable macos autostart");
    assert!(!ctrl.is_enabled());
    assert!(!plist_path.exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn r4_04_03_windows_run_registry_and_toggle() {
    let root = temp_dir("windows");
    let cfg = sample_config(r"C:\Program Files\Chord\chord.exe", true);
    let mut ctrl =
        SystemAutostartController::with_root(TargetOs::Windows, root.clone(), cfg.clone());

    let cmd = windows_run_command_line(&cfg.exec_path, &cfg.extra_args, cfg.start_minimized)
        .expect("windows command line");
    assert_eq!(
        cmd,
        r#""C:\Program Files\Chord\chord.exe" --start-minimized"#
    );

    let entry: WindowsRunEntry = windows_run_registry_entry(&cfg).expect("registry entry");
    assert_eq!(
        entry.registry_key,
        r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run"
    );
    assert_eq!(entry.value_name, "Chord");
    assert_eq!(entry.command_line, cmd);

    ctrl.set_enabled(true).expect("enable windows autostart");
    assert!(ctrl.is_enabled());

    ctrl.set_start_minimized(false)
        .expect("disable start_minimized on windows");
    assert!(ctrl.is_enabled());

    ctrl.toggle().expect("toggle off windows autostart");
    assert!(!ctrl.is_enabled());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn r4_04_04_start_minimized_prevents_window_flicker() {
    let flags_long = LaunchArgs::parse(["chord", "--start-minimized"]);
    assert!(flags_long.start_minimized);

    let flags_short = LaunchArgs::parse(["chord", "-m"]);
    assert!(flags_short.start_minimized);

    let state = StartupWindowState::resolve(&flags_long, false);
    assert!(
        !state.open_window_initially,
        "window must NOT be created/mapped on startup when --start-minimized is passed"
    );
    assert!(!state.window_visible);
    assert!(state.skip_taskbar);
    assert!(state.tray_active);

    // Setting alone without CLI flag also starts minimized without flicker
    let no_flags = LaunchArgs::parse(["chord"]);
    let state_setting = StartupWindowState::resolve(&no_flags, true);
    assert!(!state_setting.open_window_initially);
    assert!(!state_setting.window_visible);

    // Normal launch opens window immediately
    let normal = StartupWindowState::resolve(&no_flags, false);
    assert!(normal.open_window_initially);
    assert!(normal.window_visible);
    assert!(!normal.skip_taskbar);

    // Activating from tray or second instance reveals window
    let mut activated = state;
    activated.activate_window();
    assert!(activated.window_visible);
    assert!(!activated.skip_taskbar);
}

#[test]
fn r4_04_05_rejects_control_chars_and_empty_fields() {
    let bad_name = AutostartConfig {
        app_name: "".to_string(),
        ..sample_config("/usr/bin/chord", true)
    };
    assert_eq!(
        windows_run_registry_entry(&bad_name),
        Err(AutostartError::EmptyName)
    );

    let bad_ctrl = AutostartConfig {
        exec_path: "/usr/bin/chord\n--evil".to_string(),
        ..sample_config("/usr/bin/chord", true)
    };
    assert_eq!(
        windows_run_registry_entry(&bad_ctrl),
        Err(AutostartError::ControlCharacter)
    );
    assert_eq!(
        MacOsLaunchAgent::from_config(&bad_ctrl),
        Err(AutostartError::ControlCharacter)
    );
}
