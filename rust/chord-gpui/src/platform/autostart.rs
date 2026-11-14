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
use std::sync::{Arc, Mutex};

#[path = "autostart/settings.rs"]
mod settings;
pub use settings::{AutostartSettingsController, AutostartSettingsSnapshot};

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

pub fn global_autostart_controller() -> Arc<Mutex<AutostartSettingsController>> {
    settings::global_autostart_controller()
}

pub fn init_global_autostart_controller(controller: AutostartSettingsController) {
    settings::init_global_autostart_controller(controller);
}

#[cfg(test)]
#[path = "autostart/tests.rs"]
mod tests;
