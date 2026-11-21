//! Saved autostart settings, disk synchronization and the shared controller.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use super::{
    AutostartError, AutostartFileUiState, CLI_FLAG_START_MINIMIZED, CLI_FLAG_START_MINIMIZED_SHORT,
    IPC_APP_GET_OPEN_ON_START, IPC_TOGGLE_MINIMIZE_TO_TRAY, IPC_TOGGLE_OPEN_ON_STARTUP,
    IPC_TOGGLE_START_MINIMIZED, IPC_UPDATE_OPEN_ON_STARTUP, XdgAutostartManager,
    XdgDesktopEntrySpec, default_config_home, mask_autostart_secrets,
};

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

        let (start_minimized, enabled) = parse_desktop_settings(&content);
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

fn parse_desktop_settings(content: &str) -> (bool, bool) {
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
    (start_minimized, enabled)
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
