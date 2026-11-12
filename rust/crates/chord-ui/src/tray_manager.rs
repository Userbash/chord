//! Tray Manager Module
//!
//! Provides system tray integration for the Discord client UI.
//! Builds upon the window_manager service contracts and implements R4-02
//! tray functionality from the feature backlog.

use std::collections::HashMap;
use std::time::SystemTime;

/// Tray icon states
/// Represents the state of a tray icon (available, hidden, error)
#[derive(Debug, Clone, PartialEq)]
pub enum TrayState {
    /// Tray icon is visible and active
    Visible,
    /// Tray icon is hidden
    Hidden,
    /// Tray icon shows an error state
    Error,
    /// Tray icon shows unread count
    UnreadCount(usize),
}

/// Tray menu item action
#[derive(Debug, Clone, PartialEq)]
pub enum TrayMenuAction {
    /// Show/hide the main window
    ToggleWindow,
    /// Quit the application
    Quit,
    /// Reload/discord reconnection
    Reload,
    /// Open settings
    Settings,
}

/// Tray menu item description
#[derive(Debug, Clone, PartialEq)]
pub struct TrayMenuItem {
    /// Internal action identifier
    pub action: TrayMenuAction,
    /// Display text for the menu item
    pub label: String,
    /// Whether the item is disabled
    pub disabled: bool,
}

/// Tray configuration
#[derive(Debug, Clone)]
pub struct TrayConfig {
    /// Application name for the tray
    pub app_name: String,
    /// Default tray state
    pub default_state: TrayState,
    /// Menu items to display
    pub menu_items: Vec<TrayMenuItem>,
    /// Whether double-click should toggle window
    pub double_click_toggles_window: bool,
}

impl Default for TrayConfig {
    fn default() -> Self {
        Self {
            app_name: "Discord".to_string(),
            default_state: TrayState::Visible,
            menu_items: vec![
                TrayMenuItem {
                    action: TrayMenuAction::ToggleWindow,
                    label: "Показать окно".to_string(),
                    disabled: false,
                },
                TrayMenuItem {
                    action: TrayMenuAction::Quit,
                    label: "Выход".to_string(),
                    disabled: false,
                },
            ],
            double_click_toggles_window: true,
        }
    }
}

/// Tray state tracking
#[derive(Debug, Clone)]
struct TrayStateInfo {
    /// Current tray state
    state: TrayState,
    /// Last updated timestamp
    last_updated: u64,
    /// Associated window ID (if any)
    associated_window: Option<String>,
}

/// Tray manager service
/// Orchestrates tray icon creation, lifecycle, and menu handling.

#[derive(Debug, Clone)]
pub struct TrayManager {
    /// Active tray instances indexed by app identifier
    active_trays: HashMap<String, TrayStateInfo>,
    /// Configuration for each tray
    configs: HashMap<String, TrayConfig>,
}

impl Default for TrayManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TrayManager {
    /// Creates a new TrayManager with empty state
    pub fn new() -> Self {
        TrayManager {
            active_trays: HashMap::new(),
            configs: HashMap::new(),
        }
    }

    /// Sets up a tray with the given configuration
    pub fn setup_tray(&mut self, tray_id: &str, config: TrayConfig) -> Result<(), String> {
        if self.active_trays.contains_key(tray_id) {
            return Err(format!("Tray with ID '{}' already exists", tray_id));
        }

        let state = TrayStateInfo {
            state: config.default_state.clone(),
            last_updated: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            associated_window: None,
        };

        self.active_trays.insert(tray_id.to_string(), state);
        self.configs.insert(tray_id.to_string(), config);

        Ok(())
    }

    /// Toggles the associated window visibility
    pub fn toggle_window(&mut self, tray_id: &str) -> Result<(), String> {
        let state = match self.active_trays.get_mut(tray_id) {
            Some(s) => s,
            None => return Err(format!("Tray with ID '{}' not found", tray_id)),
        };

        // If there's an associated window, we would toggle it
        // For now, just update the tray state
        state.last_updated = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Ok(())
    }

    /// Sets the tray state
    pub fn set_state(&mut self, tray_id: &str, state: TrayState) -> Result<(), String> {
        let tray_state = match self.active_trays.get_mut(tray_id) {
            Some(s) => s,
            None => return Err(format!("Tray with ID '{}' not found", tray_id)),
        };

        tray_state.state = state;
        tray_state.last_updated = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Ok(())
    }

    /// Gets the current tray state
    pub fn get_state(&self, tray_id: &str) -> Option<TrayState> {
        self.active_trays.get(tray_id).map(|s| s.state.clone())
    }

    /// Gets the tray configuration
    pub fn get_config(&self, tray_id: &str) -> Option<TrayConfig> {
        self.configs.get(tray_id).cloned()
    }

    /// Associates a window with a tray
    pub fn associate_window(&mut self, tray_id: &str, window_id: &str) -> Result<(), String> {
        let state = match self.active_trays.get_mut(tray_id) {
            Some(s) => s,
            None => return Err(format!("Tray with ID '{}' not found", tray_id)),
        };

        state.associated_window = Some(window_id.to_string());
        state.last_updated = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Ok(())
    }

    /// Removes association between tray and window
    pub fn disassociate_window(&mut self, tray_id: &str) -> Result<(), String> {
        let state = match self.active_trays.get_mut(tray_id) {
            Some(s) => s,
            None => return Err(format!("Tray with ID '{}' not found", tray_id)),
        };

        state.associated_window = None;
        state.last_updated = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Ok(())
    }

    /// Gets all open tray IDs
    pub fn get_open_tray_ids(&self) -> Vec<String> {
        self.active_trays
            .iter()
            .filter(|(_, s)| matches!(s.state, TrayState::Visible | TrayState::UnreadCount(_)))
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Gets the number of active trays
    pub fn active_tray_count(&self) -> usize {
        self.active_trays
            .values()
            .filter(|s| {
                matches!(
                    s.state,
                    TrayState::Visible | TrayState::Error | TrayState::UnreadCount(_)
                )
            })
            .count()
    }

    /// Gets the total number of tracked trays
    pub fn total_tray_count(&self) -> usize {
        self.active_trays.len()
    }

    /// Handles a tray menu action
    pub fn handle_action(&mut self, tray_id: &str, action: TrayMenuAction) -> Result<(), String> {
        let state = match self.active_trays.get_mut(tray_id) {
            Some(s) => s,
            None => return Err(format!("Tray with ID '{}' not found", tray_id)),
        };

        match action {
            TrayMenuAction::ToggleWindow => {
                self.toggle_window(tray_id)?;
            }
            TrayMenuAction::Quit => {
                state.state = TrayState::Error;
                state.last_updated = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                // In a real application, this would quit the process
            }
            TrayMenuAction::Reload => {
                state.last_updated = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                // In a real application, this would reload the app
            }
            TrayMenuAction::Settings => {
                state.last_updated = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                // In a real application, this would open settings
            }
        }

        Ok(())
    }
}

/// Oracle tests for TrayManager (Red→Green pattern)
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tray_manager_new() {
        let manager = TrayManager::new();
        assert_eq!(manager.active_tray_count(), 0);
        assert_eq!(manager.total_tray_count(), 0);
    }

    #[test]
    fn test_setup_tray() {
        let mut manager = TrayManager::new();
        let config = TrayConfig::default();

        let result = manager.setup_tray("main-tray", config.clone());
        assert!(result.is_ok(), "Setting up tray should succeed");
        assert_eq!(manager.total_tray_count(), 1);
        assert_eq!(manager.get_state("main-tray"), Some(TrayState::Visible));
    }

    #[test]
    fn test_duplicate_tray_id() {
        let mut manager = TrayManager::new();
        let config = TrayConfig::default();
        let config2 = config.clone();

        manager.setup_tray("main-tray", config).unwrap();
        let result = manager.setup_tray("main-tray", config2);
        assert!(result.is_err(), "Setting up duplicate tray ID should fail");
    }

    #[test]
    fn test_toggle_window() {
        let mut manager = TrayManager::new();
        let config = TrayConfig::default();

        manager.setup_tray("main-tray", config).unwrap();
        let result = manager.toggle_window("main-tray");
        assert!(result.is_ok(), "Toggling window should succeed");
    }

    #[test]
    fn test_set_state() {
        let mut manager = TrayManager::new();
        let config = TrayConfig::default();

        manager.setup_tray("main-tray", config).unwrap();

        let result = manager.set_state("main-tray", TrayState::Error);
        assert!(result.is_ok(), "Setting tray state should succeed");
        assert_eq!(manager.get_state("main-tray"), Some(TrayState::Error));
    }

    #[test]
    fn test_handle_action_toggle() {
        let mut manager = TrayManager::new();
        let config = TrayConfig::default();

        manager.setup_tray("main-tray", config).unwrap();
        let result = manager.handle_action("main-tray", TrayMenuAction::ToggleWindow);
        assert!(result.is_ok(), "Handling toggle action should succeed");
    }

    #[test]
    fn test_handle_action_quit() {
        let mut manager = TrayManager::new();
        let config = TrayConfig::default();

        manager.setup_tray("main-tray", config).unwrap();
        let result = manager.handle_action("main-tray", TrayMenuAction::Quit);
        assert!(result.is_ok(), "Handling quit action should succeed");
    }

    #[test]
    fn test_multiple_trays() {
        let mut manager = TrayManager::new();
        let config1 = TrayConfig::default();
        let config2 = TrayConfig {
            app_name: "Discord Canary".to_string(),
            default_state: TrayState::Error,
            ..TrayConfig::default()
        };

        manager.setup_tray("discord-tray", config1).unwrap();
        manager.setup_tray("canary-tray", config2).unwrap();

        assert_eq!(manager.total_tray_count(), 2);
        assert_eq!(manager.active_tray_count(), 2);
        assert_eq!(manager.get_state("discord-tray"), Some(TrayState::Visible));
        assert_eq!(manager.get_state("canary-tray"), Some(TrayState::Error));
    }

    #[test]
    fn test_associate_disassociate_window() {
        let mut manager = TrayManager::new();
        let config = TrayConfig::default();

        manager.setup_tray("main-tray", config).unwrap();

        let result = manager.associate_window("main-tray", "window-1");
        assert!(result.is_ok(), "Associating window should succeed");

        let result = manager.disassociate_window("main-tray");
        assert!(result.is_ok(), "Disassociating window should succeed");
        // associated_window tracked in TrayStateInfo, not TrayConfig;
        // verify tray state is still Visible after disassociation
        assert!(manager.get_state("main-tray") == Some(TrayState::Visible));
    }

    #[test]
    fn test_handle_action_unknown() {
        let mut manager = TrayManager::new();
        let config = TrayConfig::default();

        manager.setup_tray("main-tray", config).unwrap();
        let result = manager.handle_action("main-tray", TrayMenuAction::Settings);
        assert!(result.is_ok(), "Handling settings action should succeed");
    }
}
