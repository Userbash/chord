//! Windows-specific platform integration (Task R6-03).
//!
//! Covers:
//! - Taskbar badge overlays via `WindowsTaskbarOverlay` (`ITaskbarList3::SetOverlayIcon` abstraction).
//! - Portable mode (isolated data alongside executable; never touches AppData or Windows Registry).
//! - Standard Windows hotkeys (`Alt+F4` window close / minimize-to-tray, `Ctrl+Q` quit application).

use std::fmt;
use std::path::{Path, PathBuf};

use crate::unread_badge::{UnreadBadgeDisplay, WindowsTaskbarOverlay};

/// Errors produced by Windows platform operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowsPlatformError {
    BackendFailure(String),
}

impl fmt::Display for WindowsPlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BackendFailure(msg) => write!(f, "windows platform backend failure: {msg}"),
        }
    }
}

impl std::error::Error for WindowsPlatformError {}

/// Abstraction over Windows `ITaskbarList3::SetOverlayIcon`.
pub trait TaskbarOverlayBackend {
    fn apply_overlay(&mut self, overlay: &WindowsTaskbarOverlay)
        -> Result<(), WindowsPlatformError>;
    fn clear_overlay(&mut self) -> Result<(), WindowsPlatformError>;
}

/// Deterministic in-memory backend for testing taskbar badge overlays.
#[derive(Debug, Default, Clone)]
pub struct InMemoryTaskbarOverlayBackend {
    last_overlay: Option<WindowsTaskbarOverlay>,
    apply_calls: usize,
    clear_calls: usize,
}

impl InMemoryTaskbarOverlayBackend {
    pub fn apply_calls(&self) -> usize {
        self.apply_calls
    }

    pub fn clear_calls(&self) -> usize {
        self.clear_calls
    }

    pub fn last_overlay(&self) -> Option<&WindowsTaskbarOverlay> {
        self.last_overlay.as_ref()
    }
}

impl TaskbarOverlayBackend for InMemoryTaskbarOverlayBackend {
    fn apply_overlay(
        &mut self,
        overlay: &WindowsTaskbarOverlay,
    ) -> Result<(), WindowsPlatformError> {
        self.apply_calls += 1;
        self.last_overlay = Some(overlay.clone());
        Ok(())
    }

    fn clear_overlay(&mut self) -> Result<(), WindowsPlatformError> {
        self.clear_calls += 1;
        self.last_overlay = Some(WindowsTaskbarOverlay::Cleared);
        Ok(())
    }
}

/// Coordinates unread badge state with the Windows taskbar overlay backend.
#[derive(Debug)]
pub struct WindowsTaskbarBadgeController<B: TaskbarOverlayBackend> {
    backend: B,
    current_overlay: WindowsTaskbarOverlay,
}

impl<B: TaskbarOverlayBackend> WindowsTaskbarBadgeController<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            current_overlay: WindowsTaskbarOverlay::Cleared,
        }
    }

    /// Updates the taskbar overlay from `UnreadBadgeDisplay`.
    pub fn update_badge(
        &mut self,
        display: UnreadBadgeDisplay,
    ) -> Result<(), WindowsPlatformError> {
        let next = WindowsTaskbarOverlay::from_display(display);
        match &next {
            WindowsTaskbarOverlay::Cleared => self.backend.clear_overlay()?,
            WindowsTaskbarOverlay::Icon(_) => self.backend.apply_overlay(&next)?,
        }
        self.current_overlay = next;
        Ok(())
    }

    /// Returns `true` when an unread badge icon is currently visible on the taskbar.
    pub fn is_badge_visible(&self) -> bool {
        !matches!(self.current_overlay, WindowsTaskbarOverlay::Cleared)
    }

    pub fn current_overlay(&self) -> &WindowsTaskbarOverlay {
        &self.current_overlay
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }
}

/// Execution mode for Windows storage and system integration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortableMode {
    /// Isolated data alongside the executable; never touches AppData or Registry.
    Portable { exe_dir: PathBuf },
    /// Standard installed mode using `%APPDATA%` and Windows Registry.
    Standard { app_data_dir: PathBuf },
}

/// Resolved Windows storage paths and side-effect policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsStoragePaths {
    portable: bool,
    config_dir: PathBuf,
    data_dir: PathBuf,
    cache_dir: PathBuf,
    logs_dir: PathBuf,
}

impl WindowsStoragePaths {
    /// Resolves storage directories for either Portable or Standard mode.
    pub fn resolve(mode: PortableMode) -> Self {
        match mode {
            PortableMode::Portable { exe_dir } => {
                let root = exe_dir.join("data");
                Self {
                    portable: true,
                    config_dir: root.join("config"),
                    data_dir: root.join("storage"),
                    cache_dir: root.join("cache"),
                    logs_dir: root.join("logs"),
                }
            }
            PortableMode::Standard { app_data_dir } => Self {
                portable: false,
                config_dir: app_data_dir.join("config"),
                data_dir: app_data_dir.join("storage"),
                cache_dir: app_data_dir.join("cache"),
                logs_dir: app_data_dir.join("logs"),
            },
        }
    }

    pub fn is_portable(&self) -> bool {
        self.portable
    }

    /// Portable builds must never touch `%APPDATA%` / `%LOCALAPPDATA%`.
    pub fn touches_app_data(&self) -> bool {
        !self.portable
    }

    /// Portable builds must never touch the Windows Registry.
    pub fn touches_registry(&self) -> bool {
        !self.portable
    }

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    pub fn logs_dir(&self) -> &Path {
        &self.logs_dir
    }
}

/// Windows-specific application hotkeys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsHotkey {
    AltF4,
    CtrlQ,
}

/// Action triggered by a Windows hotkey.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    CloseWindow,
    MinimizeToTray,
    QuitApplication,
}

/// Parses a modifier list and key name into a `WindowsHotkey` if recognized.
pub fn parse_hotkey(modifiers: &[&str], key: &str) -> Option<WindowsHotkey> {
    if modifiers.len() != 1 {
        return None;
    }
    let modifier = modifiers[0].trim();
    let key = key.trim();
    if modifier.eq_ignore_ascii_case("alt") && key.eq_ignore_ascii_case("f4") {
        Some(WindowsHotkey::AltF4)
    } else if modifier.eq_ignore_ascii_case("ctrl") && key.eq_ignore_ascii_case("q") {
        Some(WindowsHotkey::CtrlQ)
    } else {
        None
    }
}

/// Maps a `WindowsHotkey` to its `HotkeyAction` respecting `minimize_to_tray`.
pub fn handle_hotkey(hotkey: WindowsHotkey, minimize_to_tray: bool) -> HotkeyAction {
    match hotkey {
        WindowsHotkey::AltF4 => {
            if minimize_to_tray {
                HotkeyAction::MinimizeToTray
            } else {
                HotkeyAction::CloseWindow
            }
        }
        WindowsHotkey::CtrlQ => HotkeyAction::QuitApplication,
    }
}
