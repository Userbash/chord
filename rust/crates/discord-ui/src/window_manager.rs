//! Window Manager Module
//!
//! Provides window lifecycle management, tray integration, and geometry tracking
//! for the Discord client UI. Builds upon the dialog_window service contracts.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

// Window configuration constants
// These values are derived from the R4-01 window shell requirements specification.

/// Default minimum window width in pixels
pub const DEFAULT_MIN_WIDTH: u32 = 940;
/// Default default window width in pixels
pub const DEFAULT_WIDTH: u32 = 1280;
/// Default minimum window height in pixels
pub const DEFAULT_MIN_HEIGHT: u32 = 500;
/// Default default window height in pixels
pub const DEFAULT_HEIGHT: u32 = 720;
/// Minimum allowed width decrement step
pub const MIN_WIDTH_STEP: u32 = 10;
/// Minimum allowed height decrement step
pub const MIN_HEIGHT_STEP: u32 = 10;

// Window state tracking
// Tracks the state of native windows in the Discord client.

/// Window geometry state
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowGeometry {
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// X position in pixels (screen coordinate)
    pub x: i32,
    /// Y position in pixels (screen coordinate)
    pub y: i32,
    /// Whether the window is maximized
    pub is_maximized: bool,
    /// Whether the window is minimized
    pub is_minimized: bool,
}

impl Default for WindowGeometry {
    fn default() -> Self {
        Self {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            x: 0,
            y: 0,
            is_maximized: false,
            is_minimized: false,
        }
    }
}

/// Window configuration
#[derive(Debug, Clone, PartialEq)]
pub struct WindowConfig {
    /// Window title key (looked up from design tokens)
    pub title_key: String,
    /// Fallback title if key not found
    pub fallback_title: String,
    /// Whether the window is closable
    pub closable: bool,
    /// Whether the window is modal
    pub modal: bool,
    /// Initial geometry
    pub initial_geometry: WindowGeometry,
    /// Minimum allowed width
    pub min_width: u32,
    /// Minimum allowed height
    pub min_height: u32,
    /// Whether to show system title bar
    pub decorators: bool,
}

// Window manager service
// Orchestrates window creation, lifecycle, and geometry tracking.

#[derive(Debug, Clone)]
pub struct WindowManager {
    /// Active windows indexed by window ID
    active_windows: HashMap<String, WindowState>,
    /// Track geometry history for restore-on-restart
    geometry_history: Vec<WindowGeometry>,
}

#[derive(Debug, Clone)]
struct WindowState {
    geometry: WindowGeometry,
    is_open: bool,
    last_updated: u64,
}

impl Default for WindowManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowManager {
    /// Creates a new WindowManager with empty state
    pub fn new() -> Self {
        WindowManager {
            active_windows: HashMap::new(),
            geometry_history: Vec::new(),
        }
    }

    /// Opens a new window with the given configuration
    pub fn open_window(&mut self, window_id: &str, config: WindowConfig) -> Result<String, String> {
        if self.active_windows.contains_key(window_id) {
            return Err(format!("Window with ID '{}' already exists", window_id));
        }

        let state = WindowState {
            geometry: config.initial_geometry.clone(),
            is_open: true,
            last_updated: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        };

        self.active_windows.insert(window_id.to_string(), state);
        self.geometry_history.push(config.initial_geometry);

        Ok(window_id.to_string())
    }

    /// Updates window geometry
    pub fn update_geometry(
        &mut self,
        window_id: &str,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        let state = match self.active_windows.get_mut(window_id) {
            Some(s) => s,
            None => return Err(format!("Window with ID '{}' not found", window_id)),
        };

        // Enforce minimum dimensions
        if width < DEFAULT_MIN_WIDTH {
            return Err(format!(
                "Width {} is below minimum {}",
                width, DEFAULT_MIN_WIDTH
            ));
        }
        if height < DEFAULT_MIN_HEIGHT {
            return Err(format!(
                "Height {} is below minimum {}",
                height, DEFAULT_MIN_HEIGHT
            ));
        }

        state.geometry.width = width;
        state.geometry.height = height;
        state.last_updated = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Save to history for restore-on-restart
        self.geometry_history.push(state.geometry.clone());

        Ok(())
    }

    /// Closes a window and marks it as not open (not removed from HashMap)
    pub fn close_window(&mut self, window_id: &str) -> Result<(), String> {
        let state = match self.active_windows.get_mut(window_id) {
            Some(s) => s,
            None => return Err(format!("Window with ID '{}' not found", window_id)),
        };

        if state.is_open {
            state.is_open = false;
            Ok(())
        } else {
            Err(format!("Window with ID '{}' is already closed", window_id))
        }
    }

    /// Returns whether a window is open
    pub fn is_window_open(&self, window_id: &str) -> bool {
        self.active_windows
            .get(window_id)
            .map(|s| s.is_open)
            .unwrap_or(false)
    }

    /// Gets the geometry of a window
    pub fn get_geometry(&self, window_id: &str) -> Option<WindowGeometry> {
        self.active_windows
            .get(window_id)
            .map(|s| s.geometry.clone())
    }

    /// Gets all open window IDs
    pub fn get_open_window_ids(&self) -> Vec<String> {
        self.active_windows
            .iter()
            .filter(|(_, s)| s.is_open)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Gets all window IDs (open and closed)
    pub fn get_all_window_ids(&self) -> Vec<String> {
        self.active_windows.keys().cloned().collect()
    }

    /// Restores window from geometry history
    pub fn restore_from_history(&mut self, index: usize) -> Result<WindowGeometry, String> {
        if index >= self.geometry_history.len() {
            return Err(format!(
                "History index {} out of range (max: {})",
                index,
                self.geometry_history.len() - 1
            ));
        }
        Ok(self.geometry_history[index].clone())
    }

    /// Gets the number of active (open) windows
    pub fn active_window_count(&self) -> usize {
        self.active_windows.values().filter(|s| s.is_open).count()
    }

    /// Gets the total number of tracked windows
    pub fn total_window_count(&self) -> usize {
        self.active_windows.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_manager_new() {
        let manager = WindowManager::new();
        assert_eq!(manager.active_window_count(), 0);
        assert_eq!(manager.total_window_count(), 0);
    }

    #[test]
    fn test_open_window() {
        let mut manager = WindowManager::new();
        let config = WindowConfig {
            title_key: "window.title".to_string(),
            fallback_title: "Discord".to_string(),
            closable: true,
            modal: false,
            initial_geometry: WindowGeometry::default(),
            min_width: 940,
            min_height: 500,
            decorators: true,
        };

        let result = manager.open_window("main-window", config);
        assert!(result.is_ok(), "Opening window should succeed");
        let window_id = result.unwrap();
        assert!(manager.is_window_open(&window_id));
        assert_eq!(manager.active_window_count(), 1);
        assert_eq!(manager.total_window_count(), 1);
    }

    #[test]
    fn test_duplicate_window_id() {
        let mut manager = WindowManager::new();
        let config = WindowConfig {
            title_key: "window.title".to_string(),
            fallback_title: "Discord".to_string(),
            closable: true,
            modal: false,
            initial_geometry: WindowGeometry::default(),
            min_width: 940,
            min_height: 500,
            decorators: true,
        };

        manager.open_window("main-window", config.clone()).unwrap();

        let result = manager.open_window("main-window", config);
        assert!(result.is_err(), "Opening duplicate window ID should fail");
    }

    #[test]
    fn test_close_window() {
        let mut manager = WindowManager::new();
        let config = WindowConfig {
            title_key: "window.title".to_string(),
            fallback_title: "Discord".to_string(),
            closable: true,
            modal: false,
            initial_geometry: WindowGeometry::default(),
            min_width: 940,
            min_height: 500,
            decorators: true,
        };

        manager.open_window("main-window", config.clone()).unwrap();

        let result = manager.close_window("main-window");
        assert!(result.is_ok(), "Closing window should succeed");
        assert!(!manager.is_window_open("main-window"));

        // Try closing already closed window
        let result = manager.close_window("main-window");
        assert!(result.is_err(), "Closing already closed window should fail");
    }

    #[test]
    fn test_update_geometry() {
        let mut manager = WindowManager::new();
        let config = WindowConfig {
            title_key: "window.title".to_string(),
            fallback_title: String::from("Discord"),
            closable: true,
            modal: false,
            initial_geometry: WindowGeometry::default(),
            min_width: 940,
            min_height: 500,
            decorators: true,
        };

        manager.open_window("main-window", config.clone()).unwrap();

        // Update to valid geometry
        let result = manager.update_geometry("main-window", 1280, 720);
        assert!(result.is_ok(), "Updating geometry should succeed");

        // Try below minimum width
        let result = manager.update_geometry("main-window", 800, 720);
        assert!(
            result.is_err(),
            "Updating geometry below minimum should fail"
        );
    }

    #[test]
    fn test_multiple_windows() {
        let mut manager = WindowManager::new();
        let config = WindowConfig {
            title_key: String::from("window.title"),
            fallback_title: String::from("Discord"),
            closable: true,
            modal: false,
            initial_geometry: WindowGeometry::default(),
            min_width: 940,
            min_height: 500,
            decorators: true,
        };

        manager.open_window("window-1", config.clone()).unwrap();
        manager.open_window("window-2", config.clone()).unwrap();

        assert_eq!(manager.active_window_count(), 2);
        assert_eq!(manager.total_window_count(), 2);
        assert!(manager.is_window_open("window-1"));
        assert!(manager.is_window_open("window-2"));

        // Close one window
        manager.close_window("window-2").unwrap();
        assert_eq!(manager.active_window_count(), 1);
        assert!(!manager.is_window_open("window-2"));
        assert!(manager.is_window_open("window-1"));
    }

    #[test]
    fn test_window_geometry_history() {
        let mut manager = WindowManager::new();
        let config = WindowConfig {
            title_key: "window.title".to_string(),
            fallback_title: "Discord".to_string(),
            closable: true,
            modal: false,
            initial_geometry: WindowGeometry::default(),
            min_width: 940,
            min_height: 500,
            decorators: true,
        };

        manager.open_window("window-1", config.clone()).unwrap();

        // Update geometry multiple times
        manager.update_geometry("window-1", 1920, 1080).unwrap();
        manager.update_geometry("window-1", 1600, 900).unwrap();

        // Restore from history
        let restored = manager.restore_from_history(0).unwrap();
        assert_eq!(restored.width, DEFAULT_WIDTH);
        assert_eq!(restored.height, DEFAULT_HEIGHT);

        let restored = manager.restore_from_history(2).unwrap();
        assert_eq!(restored.width, 1600);
        assert_eq!(restored.height, 900);
    }
}
