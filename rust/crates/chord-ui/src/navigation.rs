//! Navigation Module
//! Provides UI navigation framework and view management for the Discord client.
//! Implements P3-01 UI framework and navigation from the plan.
//! Builds upon dialog_window and window_manager service contracts.

use serde::{Deserialize, Serialize};
use std::time::SystemTime;

// View identifiers
/// Identifiers for different UI views
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ViewId {
    /// Chat view
    Chat,
    /// Settings view
    Settings,
    /// User profile view
    UserProfile,
    /// Friends list view
    Friends,
    /// Search view
    Search,
    /// Notification view
    Notifications,
}

// Navigation state
/// State of the navigation system
#[derive(Debug, Clone, PartialEq)]
pub struct NavigationState {
    /// Current active view
    pub current_view: ViewId,
    /// Whether navigation is enabled
    pub enabled: bool,
    /// History of recent views
    pub history: Vec<ViewId>,
    /// Last updated timestamp
    pub last_updated: u64,
}

impl Default for NavigationState {
    fn default() -> Self {
        Self {
            current_view: ViewId::Chat,
            enabled: true,
            history: Vec::new(),
            last_updated: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }
    }
}

// Navigation service
/// Manages UI navigation and view state
#[derive(Debug, Clone)]
pub struct NavigationService {
    /// Active navigation state
    state: NavigationState,
    /// History capacity limit
    max_history: usize,
}

impl Default for NavigationService {
    fn default() -> Self {
        Self::new()
    }
}

impl NavigationService {
    /// Creates a new navigation service
    pub fn new() -> Self {
        NavigationService {
            state: NavigationState::default(),
            max_history: 10,
        }
    }
    /// Navigate to a new view
    pub fn navigate(&mut self, view: ViewId) -> Result<(), String> {
        if !self.state.enabled {
            return Err("Navigation is disabled".to_string());
        }
        // Add to history if not already there (clone view to avoid move issues)
        let view_for_history = view.clone();
        if self.state.history.last() != Some(&view_for_history) {
            if self.state.history.len() >= self.max_history {
                self.state.history.remove(0);
            }
            self.state.history.push(view_for_history);
        }
        self.state.current_view = view;
        Ok(())
    }
    /// Get current view
    pub fn current_view(&self) -> ViewId {
        self.state.current_view.clone()
    }
    /// Is navigation enabled
    pub fn is_enabled(&self) -> bool {
        self.state.enabled
    }
    /// Set navigation enabled/disabled
    pub fn set_enabled(&mut self, enabled: bool) {
        self.state.enabled = enabled;
    }
    /// Get navigation state
    pub fn state(&self) -> &NavigationState {
        &self.state
    }
    /// Get history
    pub fn history(&self) -> &Vec<ViewId> {
        &self.state.history
    }
}

/// Tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigation_service_new() {
        let service = NavigationService::new();
        assert_eq!(service.current_view(), ViewId::Chat);
        assert!(service.is_enabled());
        assert_eq!(service.history().len(), 0);
    }

    #[test]
    fn test_navigate_to_view() {
        let mut service = NavigationService::new();
        let result = service.navigate(ViewId::Settings);
        assert!(result.is_ok());
        assert_eq!(service.current_view(), ViewId::Settings);
        assert_eq!(service.history().len(), 1);
    }

    #[test]
    fn test_navigate_disabled() {
        let mut service = NavigationService::new();
        service.set_enabled(false);
        let result = service.navigate(ViewId::Settings);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Navigation is disabled");
    }

    #[test]
    fn test_navigate_history_limit() {
        let mut service = NavigationService::new();
        service.max_history = 3;

        service.navigate(ViewId::Chat).unwrap();
        service.navigate(ViewId::Settings).unwrap();
        service.navigate(ViewId::Friends).unwrap();
        service.navigate(ViewId::Search).unwrap();

        assert_eq!(service.history().len(), 3); // Should be limited to max_history
    }

    #[test]
    fn test_navigation_state_default() {
        let state = NavigationState::default();
        assert_eq!(state.current_view, ViewId::Chat);
        assert!(state.enabled);
        assert_eq!(state.history.len(), 0);
    }
}
