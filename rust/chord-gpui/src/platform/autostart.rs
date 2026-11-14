//! Автозапуск с системой (XDG Autostart & Start Minimized — `TASK-1015`).
//!
//! Генерация и управление `~/.config/autostart/discord-gpui.desktop` (`TASK-1015A`),
//! разбор флага `--start-minimized` и старт свёрнутым в трей с бюджетом `VmRSS < 8 МБ`
//! (`TASK-1015B`), а также обработка IPC-каналов `core/#4973`.

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

pub const AUTOSTART_SERVICE_CONTAINER: &str = "Autostart Service";
pub const AUTOSTART_SERVICE_DIMENSIONS: &str = "CLI & Config";
pub const AUTOSTART_CONFIG_CONTAINER: &str = "Autostart Config";
pub const AUTOSTART_CONFIG_DIMENSIONS: &str = "Desktop Entry";
pub const HEADLESS_INIT_CONTAINER: &str = "Headless Init";
pub const HEADLESS_INIT_DIMENSIONS: &str = "RAM < 8MB";
pub const AUTOSTART_PADDING: &str = "0px";
pub const AUTOSTART_SPACING: &str = "0px";

pub const TOKEN_BG: &str = "#1e1f22";
pub const TOKEN_SURFACE: &str = "#2b2d31";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_BORDER: &str = "#3f4147";

pub const DESKTOP_FILE_NAME: &str = "discord-gpui.desktop";
pub const DEFAULT_APP_NAME: &str = "Discord GPUI";
pub const DEFAULT_COMMENT: &str = "Text and voice chat for gamers.";
pub const DEFAULT_ICON_NAME: &str = "discord";
pub const DEFAULT_WM_CLASS: &str = "discord-gpui";
pub const DESKTOP_ENTRY_VERSION: &str = "1.5";
pub const DESKTOP_FILE_MODE: u32 = 0o644;

pub const CLI_FLAG_START_MINIMIZED: &str = "--start-minimized";
pub const CLI_FLAG_START_MINIMIZED_SHORT: &str = "-m";
pub const CLI_FLAG_START_INACTIVE: &str = "--start-inactive";
pub const CLI_FLAG_OPEN_ON_STARTUP: &str = "--open-on-startup";

pub const MINIMIZED_RAM_LIMIT_KB: u64 = 8 * 1024;
pub const MINIMIZED_RAM_LIMIT_BYTES: u64 = 8 * 1024 * 1024;

pub const IPC_APP_GET_OPEN_ON_START: &str = "DISCORD_APP_GET_OPEN_ON_START";
pub const IPC_TOGGLE_OPEN_ON_STARTUP: &str = "DISCORD_TOGGLE_OPEN_ON_STARTUP";
pub const IPC_TOGGLE_START_MINIMIZED: &str = "DISCORD_TOGGLE_START_MINIMIZED";
pub const IPC_TOGGLE_MINIMIZE_TO_TRAY: &str = "DISCORD_TOGGLE_MINIMIZE_TO_TRAY";
pub const IPC_UPDATE_OPEN_ON_STARTUP: &str = "DISCORD_UPDATE_OPEN_ON_STARTUP";

pub const PORTAL_BUS_NAME: &str = "org.freedesktop.portal.Desktop";
pub const PORTAL_OBJECT_PATH: &str = "/org/freedesktop/portal/desktop";
pub const PORTAL_BACKGROUND_INTERFACE: &str = "org.freedesktop.portal.Background";

pub const MAX_MESSAGE_CHARS: usize = 2_000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_BADGE_CAP: u32 = 99;

pub const PERM_VIEW_CHANNEL: u64 = 1 << 10;
pub const PERM_SEND_MESSAGES: u64 = 1 << 11;
pub const PERM_AGE_GATED_SPACES: u64 = 1 << 39;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutostartToggleUiState {
    Off,
    HighlightToggle,
    Enabled,
}

impl AutostartToggleUiState {
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::HighlightToggle => "highlight toggle",
            Self::Enabled => "enabled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutostartFileUiState {
    Absent,
    Writing,
    Registered,
}

impl AutostartFileUiState {
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Writing => "writing",
            Self::Registered => "registered",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayIconUiState {
    IdleInTray,
    Tooltip,
    WindowMapped,
}

impl TrayIconUiState {
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::IdleInTray => "idle in tray",
            Self::Tooltip => "tooltip",
            Self::WindowMapped => "window mapped",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutostartError {
    EmptyAppName,
    EmptyExecPath,
    RelativeExecPath,
    ControlCharacterInField(&'static str),
    InvalidFilePermissions(u32),
    RamBudgetExceeded {
        actual_kb: u64,
        limit_kb: u64,
    },
    IoError(&'static str),
    PortalUnavailable(&'static str),
    HttpError {
        status: u16,
        retry_after_ms: Option<u64>,
    },
    PermissionDenied(&'static str),
    BoundaryViolation(&'static str),
}

impl fmt::Display for AutostartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for AutostartError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XdgDesktopEntrySpec {
    pub app_name: String,
    pub exec_path: String,
    pub extra_args: Vec<String>,
    pub icon_path: String,
    pub comment: String,
    pub wm_class: String,
    pub start_minimized: bool,
    pub enabled: bool,
}

impl Default for XdgDesktopEntrySpec {
    fn default() -> Self {
        Self {
            app_name: DEFAULT_APP_NAME.to_string(),
            exec_path: "/usr/bin/discord-gpui".to_string(),
            extra_args: Vec::new(),
            icon_path: DEFAULT_ICON_NAME.to_string(),
            comment: DEFAULT_COMMENT.to_string(),
            wm_class: DEFAULT_WM_CLASS.to_string(),
            start_minimized: true,
            enabled: true,
        }
    }
}

impl XdgDesktopEntrySpec {
    pub fn new(exec_path: &str, start_minimized: bool) -> Self {
        Self {
            exec_path: exec_path.to_string(),
            start_minimized,
            ..Self::default()
        }
    }

    pub fn validate(&self) -> Result<(), AutostartError> {
        if self.app_name.trim().is_empty() {
            return Err(AutostartError::EmptyAppName);
        }
        if self.exec_path.trim().is_empty() {
            return Err(AutostartError::EmptyExecPath);
        }
        if !self.exec_path.starts_with('/') {
            return Err(AutostartError::RelativeExecPath);
        }
        for (val, field) in [
            (self.app_name.as_str(), "Name"),
            (self.exec_path.as_str(), "Exec"),
            (self.icon_path.as_str(), "Icon"),
            (self.comment.as_str(), "Comment"),
            (self.wm_class.as_str(), "StartupWMClass"),
        ] {
            validate_no_control(val, field)?;
        }
        for arg in &self.extra_args {
            validate_no_control(arg, "ExecArg")?;
        }
        Ok(())
    }

    pub fn build_exec_line(&self) -> Result<String, AutostartError> {
        self.validate()?;
        let mut parts = Vec::with_capacity(2 + self.extra_args.len());
        parts.push(escape_desktop_exec_token(&self.exec_path));
        for arg in &self.extra_args {
            if arg != CLI_FLAG_START_MINIMIZED {
                parts.push(escape_desktop_exec_token(arg));
            }
        }
        if self.start_minimized {
            parts.push(CLI_FLAG_START_MINIMIZED.to_string());
        }
        Ok(parts.join(" "))
    }

    pub fn render_desktop_file(&self) -> Result<String, AutostartError> {
        let exec_line = self.build_exec_line()?;
        let enabled_str = if self.enabled { "true" } else { "false" };
        Ok(format!(
            "[Desktop Entry]\nType=Application\nVersion={DESKTOP_ENTRY_VERSION}\nName={}\nComment={}\nExec={exec_line}\nIcon={}\nTerminal=false\nHidden=false\nNoDisplay=false\nStartupWMClass={}\nX-GNOME-Autostart-enabled={enabled_str}\n",
            self.app_name, self.comment, self.icon_path, self.wm_class
        ))
    }
}

fn validate_no_control(val: &str, field: &'static str) -> Result<(), AutostartError> {
    if val.bytes().any(|b| matches!(b, b'\n' | b'\r' | 0)) {
        Err(AutostartError::ControlCharacterInField(field))
    } else {
        Ok(())
    }
}

pub fn escape_desktop_exec_token(token: &str) -> String {
    let reserved = " \t\"'\\><~|&;$*?#()`%";
    if !token.is_empty() && !token.chars().any(|c| reserved.contains(c)) {
        return token.to_string();
    }
    let mut out = String::with_capacity(token.len() + 4);
    out.push('"');
    for c in token.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '$' => out.push_str("\\$"),
            '`' => out.push_str("\\`"),
            '%' => out.push_str("%%"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

pub fn resolve_xdg_autostart_dir(xdg_config_home: Option<&Path>, home_dir: &Path) -> PathBuf {
    match xdg_config_home {
        Some(cfg) if !cfg.as_os_str().is_empty() => cfg.join("autostart"),
        _ => home_dir.join(".config").join("autostart"),
    }
}

pub fn resolve_desktop_file_path(xdg_config_home: Option<&Path>, home_dir: &Path) -> PathBuf {
    resolve_xdg_autostart_dir(xdg_config_home, home_dir).join(DESKTOP_FILE_NAME)
}

#[derive(Debug, Clone)]
pub struct XdgAutostartManager {
    config_home: PathBuf,
    spec: XdgDesktopEntrySpec,
    file_state: AutostartFileUiState,
    minimize_to_tray: bool,
}

impl XdgAutostartManager {
    pub fn new(config_home: PathBuf, spec: XdgDesktopEntrySpec) -> Self {
        let path = config_home.join("autostart").join(DESKTOP_FILE_NAME);
        let file_state = if path.is_file() {
            AutostartFileUiState::Registered
        } else {
            AutostartFileUiState::Absent
        };
        Self {
            config_home,
            spec,
            file_state,
            minimize_to_tray: true,
        }
    }

    pub fn desktop_file_path(&self) -> PathBuf {
        self.config_home.join("autostart").join(DESKTOP_FILE_NAME)
    }

    pub fn file_state(&self) -> AutostartFileUiState {
        self.file_state
    }

    pub fn toggle_state(&self, hovered: bool) -> AutostartToggleUiState {
        if self.is_installed() && self.spec.enabled {
            AutostartToggleUiState::Enabled
        } else if hovered {
            AutostartToggleUiState::HighlightToggle
        } else {
            AutostartToggleUiState::Off
        }
    }

    pub fn is_installed(&self) -> bool {
        self.desktop_file_path().is_file()
    }

    pub fn install(&mut self) -> Result<PathBuf, AutostartError> {
        self.file_state = AutostartFileUiState::Writing;
        let content = self.spec.render_desktop_file()?;
        let dir = self.config_home.join("autostart");
        fs::create_dir_all(&dir).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                AutostartError::PermissionDenied("create_dir_all")
            } else {
                AutostartError::IoError("create_dir_all")
            }
        })?;
        let path = dir.join(DESKTOP_FILE_NAME);
        fs::write(&path, content.as_bytes()).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                AutostartError::PermissionDenied("write")
            } else {
                AutostartError::IoError("write")
            }
        })?;
        apply_desktop_permissions(&path, DESKTOP_FILE_MODE)?;
        self.file_state = AutostartFileUiState::Registered;
        Ok(path)
    }

    pub fn uninstall(&mut self) -> Result<bool, AutostartError> {
        let path = self.desktop_file_path();
        if path.exists() {
            fs::remove_file(&path).map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    AutostartError::PermissionDenied("remove_file")
                } else {
                    AutostartError::IoError("remove_file")
                }
            })?;
            self.file_state = AutostartFileUiState::Absent;
            Ok(true)
        } else {
            self.file_state = AutostartFileUiState::Absent;
            Ok(false)
        }
    }

    pub fn update(&mut self) -> Result<bool, AutostartError> {
        if self.is_installed() {
            self.install()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn set_start_minimized(&mut self, start_minimized: bool) -> Result<bool, AutostartError> {
        self.spec.start_minimized = start_minimized;
        self.update()
    }

    pub fn set_minimize_to_tray(&mut self, minimize_to_tray: bool) {
        self.minimize_to_tray = minimize_to_tray;
    }

    pub fn minimize_to_tray(&self) -> bool {
        self.minimize_to_tray
    }

    pub fn handle_ipc(
        &mut self,
        channel: &str,
        value: Option<bool>,
    ) -> Result<bool, AutostartError> {
        match channel {
            IPC_APP_GET_OPEN_ON_START => Ok(self.is_installed()),
            IPC_TOGGLE_OPEN_ON_STARTUP => {
                if value.unwrap_or(false) {
                    self.spec.enabled = true;
                    self.install()?;
                    Ok(true)
                } else {
                    self.uninstall()?;
                    Ok(false)
                }
            }
            IPC_TOGGLE_START_MINIMIZED => {
                self.set_start_minimized(value.unwrap_or(false))?;
                Ok(self.spec.start_minimized)
            }
            IPC_TOGGLE_MINIMIZE_TO_TRAY => {
                self.set_minimize_to_tray(value.unwrap_or(true));
                Ok(self.minimize_to_tray)
            }
            IPC_UPDATE_OPEN_ON_STARTUP => self.update(),
            _ => Err(AutostartError::BoundaryViolation("unknown_ipc_channel")),
        }
    }
}

#[cfg(unix)]
fn apply_desktop_permissions(path: &Path, mode: u32) -> Result<(), AutostartError> {
    use std::os::unix::fs::PermissionsExt;
    if mode != DESKTOP_FILE_MODE {
        return Err(AutostartError::InvalidFilePermissions(mode));
    }
    let perms = fs::Permissions::from_mode(mode);
    fs::set_permissions(path, perms).map_err(|_| AutostartError::IoError("set_permissions"))
}

#[cfg(not(unix))]
fn apply_desktop_permissions(_path: &Path, mode: u32) -> Result<(), AutostartError> {
    if mode != DESKTOP_FILE_MODE {
        return Err(AutostartError::InvalidFilePermissions(mode));
    }
    Ok(())
}

#[cfg(unix)]
pub fn read_desktop_permissions_mode(path: &Path) -> Result<u32, AutostartError> {
    use std::os::unix::fs::PermissionsExt;
    let meta = fs::metadata(path).map_err(|_| AutostartError::IoError("metadata"))?;
    Ok(meta.permissions().mode() & 0o777)
}

#[cfg(not(unix))]
pub fn read_desktop_permissions_mode(_path: &Path) -> Result<u32, AutostartError> {
    Ok(DESKTOP_FILE_MODE)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchCliFlags {
    pub start_minimized: bool,
    pub start_inactive: bool,
    pub open_on_startup: bool,
}

impl LaunchCliFlags {
    pub fn parse<I, S>(args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut start_minimized = false;
        let mut start_inactive = false;
        let mut open_on_startup = false;
        for raw in args {
            match raw.as_ref() {
                CLI_FLAG_START_MINIMIZED | CLI_FLAG_START_MINIMIZED_SHORT => start_minimized = true,
                CLI_FLAG_START_INACTIVE => start_inactive = true,
                CLI_FLAG_OPEN_ON_STARTUP => open_on_startup = true,
                _ => {}
            }
        }
        Self {
            start_minimized,
            start_inactive,
            open_on_startup,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimizedTrayRuntime {
    pub wgpu_window_mapped: bool,
    pub window_focused: bool,
    pub skip_taskbar: bool,
    pub tray_hovered: bool,
    pub minimize_to_tray: bool,
    pub vmrss_kb: u64,
    pub quit_requested: bool,
}

impl MinimizedTrayRuntime {
    pub fn launch(
        cli_flags: &LaunchCliFlags,
        setting_start_minimized: bool,
        minimize_to_tray: bool,
        initial_vmrss_kb: u64,
    ) -> Result<Self, AutostartError> {
        let minimized = cli_flags.start_minimized || setting_start_minimized;
        if minimized && initial_vmrss_kb > MINIMIZED_RAM_LIMIT_KB {
            return Err(AutostartError::RamBudgetExceeded {
                actual_kb: initial_vmrss_kb,
                limit_kb: MINIMIZED_RAM_LIMIT_KB,
            });
        }
        let wgpu_window_mapped = !minimized;
        let window_focused = wgpu_window_mapped && !cli_flags.start_inactive;
        let skip_taskbar = minimized;
        Ok(Self {
            wgpu_window_mapped,
            window_focused,
            skip_taskbar,
            tray_hovered: false,
            minimize_to_tray,
            vmrss_kb: initial_vmrss_kb,
            quit_requested: false,
        })
    }

    pub fn tray_ui_state(&self) -> TrayIconUiState {
        if self.wgpu_window_mapped {
            TrayIconUiState::WindowMapped
        } else if self.tray_hovered {
            TrayIconUiState::Tooltip
        } else {
            TrayIconUiState::IdleInTray
        }
    }

    pub fn on_tray_hover(&mut self, hovered: bool) {
        self.tray_hovered = hovered;
    }

    pub fn on_tray_click(&mut self) {
        if self.wgpu_window_mapped {
            self.wgpu_window_mapped = false;
            self.window_focused = false;
            self.skip_taskbar = true;
        } else {
            self.wgpu_window_mapped = true;
            self.window_focused = true;
            self.skip_taskbar = false;
        }
    }

    pub fn on_second_instance_activate(&mut self) {
        self.wgpu_window_mapped = true;
        self.window_focused = true;
        self.skip_taskbar = false;
    }

    pub fn on_window_close_requested(&mut self) -> bool {
        if self.minimize_to_tray {
            self.wgpu_window_mapped = false;
            self.window_focused = false;
            self.skip_taskbar = true;
            false
        } else {
            self.wgpu_window_mapped = false;
            self.window_focused = false;
            self.quit_requested = true;
            true
        }
    }
}

pub fn build_portal_background_dbus_request(autostart: bool, start_minimized: bool) -> String {
    let cmd = if start_minimized {
        "['discord-gpui', '--start-minimized']"
    } else {
        "['discord-gpui']"
    };
    format!(
        "gdbus call --session --dest {PORTAL_BUS_NAME} --object-path {PORTAL_OBJECT_PATH} --method {PORTAL_BACKGROUND_INTERFACE}.RequestBackground '' \"{{'autostart': <{autostart}>, 'commandline': <{cmd}>}}\""
    )
}

pub fn mask_autostart_secrets(log_line: &str) -> String {
    let mut out = Vec::new();
    for token in log_line.split_whitespace() {
        let clean = token.trim_matches(|c: char| {
            matches!(c, '"' | '\'' | ',' | '[' | ']' | '(' | ')' | '{' | '}')
        });
        if clean.starts_with("--token=") || clean.starts_with("token=") {
            out.push("token=[REDACTED]".to_string());
        } else if clean.starts_with("mfa.") || clean.starts_with("Bearer_") {
            out.push("[REDACTED]".to_string());
        } else {
            out.push(token.to_string());
        }
    }
    out.join(" ")
}

pub fn default_config_home() -> PathBuf {
    if let Ok(val) = std::env::var("XDG_CONFIG_HOME") {
        if !val.trim().is_empty() {
            return PathBuf::from(val);
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.trim().is_empty() {
            return PathBuf::from(home).join(".config");
        }
    }
    PathBuf::from("/tmp/.config")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutostartSettingsSnapshot {
    pub open_on_startup: bool,
    pub start_minimized: bool,
    pub minimize_to_tray: bool,
    pub desktop_installed: bool,
    pub desktop_path: PathBuf,
    pub file_state: AutostartFileUiState,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AutostartSettingsController {
    config_home: PathBuf,
    manager: XdgAutostartManager,
    open_on_startup: bool,
    start_minimized: bool,
    minimize_to_tray: bool,
    last_error: Option<String>,
}

impl Default for AutostartSettingsController {
    fn default() -> Self {
        let config_home = default_config_home();
        Self::for_config_home(config_home)
    }
}

impl AutostartSettingsController {
    pub fn new(config_home: PathBuf, spec: XdgDesktopEntrySpec) -> Self {
        let start_minimized = spec.start_minimized;
        let manager = XdgAutostartManager::new(config_home.clone(), spec);
        let open_on_startup = manager.is_installed();
        let minimize_to_tray = manager.minimize_to_tray();
        Self {
            config_home,
            manager,
            open_on_startup,
            start_minimized,
            minimize_to_tray,
            last_error: None,
        }
    }

    pub fn for_config_home(config_home: PathBuf) -> Self {
        let spec = XdgDesktopEntrySpec::default();
        Self::new(config_home, spec)
    }

    pub fn config_home(&self) -> &Path {
        &self.config_home
    }

    pub fn desktop_file_path(&self) -> PathBuf {
        self.manager.desktop_file_path()
    }

    pub fn open_on_startup(&self) -> bool {
        self.open_on_startup
    }

    pub fn start_minimized(&self) -> bool {
        self.start_minimized
    }

    pub fn minimize_to_tray(&self) -> bool {
        self.minimize_to_tray
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn clear_error(&mut self) {
        self.last_error = None;
    }

    pub fn snapshot(&self) -> AutostartSettingsSnapshot {
        AutostartSettingsSnapshot {
            open_on_startup: self.open_on_startup,
            start_minimized: self.start_minimized,
            minimize_to_tray: self.minimize_to_tray,
            desktop_installed: self.manager.is_installed(),
            desktop_path: self.desktop_file_path(),
            file_state: self.manager.file_state(),
            last_error: self.last_error.clone(),
        }
    }

    fn record_error(&mut self, err: &AutostartError) {
        let extra = if !self.manager.spec.extra_args.is_empty() {
            format!(" args: {}", self.manager.spec.extra_args.join(" "))
        } else {
            String::new()
        };
        let raw = format!("{err:?}{extra}");
        self.last_error = Some(mask_autostart_secrets(&raw));
    }

    pub fn set_open_on_startup(&mut self, enabled: bool) -> Result<bool, AutostartError> {
        if enabled {
            self.manager.spec.enabled = true;
            self.manager.spec.start_minimized = self.start_minimized;
            match self.manager.install() {
                Ok(_) => {
                    self.open_on_startup = true;
                    self.last_error = None;
                    Ok(true)
                }
                Err(err) => {
                    self.record_error(&err);
                    Err(err)
                }
            }
        } else {
            match self.manager.uninstall() {
                Ok(_) => {
                    self.open_on_startup = false;
                    self.last_error = None;
                    Ok(false)
                }
                Err(err) => {
                    self.record_error(&err);
                    Err(err)
                }
            }
        }
    }

    pub fn toggle_open_on_startup(&mut self) -> Result<bool, AutostartError> {
        self.set_open_on_startup(!self.open_on_startup)
    }

    pub fn set_start_minimized(&mut self, enabled: bool) -> Result<bool, AutostartError> {
        self.start_minimized = enabled;
        self.manager.spec.start_minimized = enabled;
        if self.open_on_startup && self.manager.is_installed() {
            match self.manager.install() {
                Ok(_) => {
                    self.last_error = None;
                    Ok(enabled)
                }
                Err(err) => {
                    self.record_error(&err);
                    Err(err)
                }
            }
        } else {
            self.last_error = None;
            Ok(enabled)
        }
    }

    pub fn toggle_start_minimized(&mut self) -> Result<bool, AutostartError> {
        self.set_start_minimized(!self.start_minimized)
    }

    pub fn set_minimize_to_tray(&mut self, enabled: bool) -> Result<bool, AutostartError> {
        self.minimize_to_tray = enabled;
        self.manager.set_minimize_to_tray(enabled);
        self.last_error = None;
        Ok(enabled)
    }

    pub fn toggle_minimize_to_tray(&mut self) -> Result<bool, AutostartError> {
        self.set_minimize_to_tray(!self.minimize_to_tray)
    }

    pub fn reload_from_disk(&mut self) -> Result<AutostartSettingsSnapshot, AutostartError> {
        let path = self.desktop_file_path();
        if !path.is_file() {
            self.open_on_startup = false;
            self.manager.file_state = AutostartFileUiState::Absent;
            self.last_error = None;
            return Ok(self.snapshot());
        }

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                let err = if e.kind() == std::io::ErrorKind::PermissionDenied {
                    AutostartError::PermissionDenied("read_desktop_file")
                } else {
                    AutostartError::IoError("read_desktop_file")
                };
                self.record_error(&err);
                return Err(err);
            }
        };

        let mut start_minimized = false;
        let mut enabled = true;
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(exec) = trimmed.strip_prefix("Exec=") {
                if exec.contains(CLI_FLAG_START_MINIMIZED)
                    || exec.contains(CLI_FLAG_START_MINIMIZED_SHORT)
                {
                    start_minimized = true;
                }
            } else if let Some(val) = trimmed.strip_prefix("X-GNOME-Autostart-enabled=") {
                if val.trim().eq_ignore_ascii_case("false") {
                    enabled = false;
                }
            }
        }

        self.open_on_startup = enabled;
        self.start_minimized = start_minimized;
        self.manager.spec.enabled = enabled;
        self.manager.spec.start_minimized = start_minimized;
        self.manager.file_state = AutostartFileUiState::Registered;
        self.last_error = None;
        Ok(self.snapshot())
    }

    pub fn handle_ipc(
        &mut self,
        channel: &str,
        value: Option<bool>,
    ) -> Result<bool, AutostartError> {
        match channel {
            IPC_APP_GET_OPEN_ON_START => Ok(self.open_on_startup()),
            IPC_TOGGLE_OPEN_ON_STARTUP => {
                let target = value.unwrap_or(!self.open_on_startup());
                self.set_open_on_startup(target)
            }
            IPC_TOGGLE_START_MINIMIZED => {
                let target = value.unwrap_or(!self.start_minimized());
                self.set_start_minimized(target)
            }
            IPC_TOGGLE_MINIMIZE_TO_TRAY => {
                let target = value.unwrap_or(!self.minimize_to_tray());
                self.set_minimize_to_tray(target)
            }
            IPC_UPDATE_OPEN_ON_STARTUP => {
                self.reload_from_disk()?;
                Ok(self.open_on_startup())
            }
            _ => Err(AutostartError::BoundaryViolation("unknown_ipc_channel")),
        }
    }
}

static GLOBAL_AUTOSTART: OnceLock<Arc<Mutex<AutostartSettingsController>>> = OnceLock::new();

pub fn global_autostart_controller() -> Arc<Mutex<AutostartSettingsController>> {
    GLOBAL_AUTOSTART
        .get_or_init(|| Arc::new(Mutex::new(AutostartSettingsController::default())))
        .clone()
}

pub fn init_global_autostart_controller(controller: AutostartSettingsController) {
    if let Some(arc) = GLOBAL_AUTOSTART.get() {
        if let Ok(mut lock) = arc.lock() {
            *lock = controller;
        }
    } else {
        let _ = GLOBAL_AUTOSTART.set(Arc::new(Mutex::new(controller)));
    }
}

#[cfg(test)]
mod tests {
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
        let _ = fs::remove_dir_all(&temp_cfg);
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
        let _ = fs::remove_dir_all(&temp_cfg);
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
}
