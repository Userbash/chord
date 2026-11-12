//! Chat View Module
//! Provides chat message display and composition for the Discord client.
//! Implements P3-02 chat view integrated with P3-01 navigation framework.
//! Builds upon dialog_window, window_manager, and navigation service contracts.

use serde::{Deserialize, Serialize};

/// Represents a single chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique message identifier
    pub id: u64,
    /// Message content text
    pub content: String,
    /// Message author ID
    pub author_id: u64,
    /// Timestamp of message
    pub timestamp: u64,
    /// Whether the message is from the current user
    pub is_current_user: bool,
    /// Message flags (pinned, system, etc.)
    pub flags: MessageFlags,
}

/// Message flags
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageFlags {
    /// Normal message
    Normal,
    /// System message (join, leave, etc.)
    System,
    /// Pinned message
    Pinned,
    /// Edited message
    Edited,
}

/// State of the chat view
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ChatViewState {
    /// Current scroll position
    pub scroll_position: u64,
    /// Whether messages are loading
    pub is_loading: bool,
    /// Number of unread messages
    pub unread_count: u64,
    /// Current selected message ID
    pub selected_message_id: Option<u64>,
    /// Whether selection mode is active
    pub selection_mode: bool,
}

impl Default for ChatViewService {
    fn default() -> Self {
        Self::new()
    }
}

/// Manages chat message display and interaction
#[derive(Debug, Clone)]
pub struct ChatViewService {
    /// Current chat state
    state: ChatViewState,
    /// Message history
    history: Vec<Message>,
    /// Maximum history to retain
    max_history: usize,
}

impl ChatViewService {
    /// Creates a new chat view service
    pub fn new() -> Self {
        ChatViewService {
            state: ChatViewState::default(),
            history: Vec::new(),
            max_history: 1000,
        }
    }

    /// Add a message to history
    pub fn add_message(&mut self, message: Message) {
        self.history.push(message);
        // Retain only max_history messages
        if self.history.len() > self.max_history {
            self.history.drain(..self.history.len() - self.max_history);
        }
    }

    /// Get message by ID
    pub fn get_message(&self, id: u64) -> Option<&Message> {
        self.history.iter().find(|m| m.id == id)
    }

    /// Get current state
    pub fn state(&self) -> &ChatViewState {
        &self.state
    }

    /// Get history length
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Set selection mode
    pub fn set_selection_mode(&mut self, mode: bool) {
        self.state.selection_mode = mode;
    }

    /// Get whether selection mode is active
    pub fn is_selection_mode(&self) -> bool {
        self.state.selection_mode
    }

    /// Set scroll position
    pub fn set_scroll_position(&mut self, position: u64) {
        self.state.scroll_position = position;
    }

    /// Get scroll position
    pub fn scroll_position(&self) -> u64 {
        self.state.scroll_position
    }

    /// Set unread count
    pub fn set_unread_count(&mut self, count: u64) {
        self.state.unread_count = count;
    }

    /// Get unread count
    pub fn unread_count(&self) -> u64 {
        self.state.unread_count
    }

    /// Select a message
    pub fn select_message(&mut self, id: u64) {
        self.state.selected_message_id = Some(id);
    }

    /// Deselect message
    pub fn deselect_message(&mut self) {
        self.state.selected_message_id = None;
    }
}

/// Extension methods for Message
impl Message {
    /// Creates a new message
    pub fn new(
        id: u64,
        content: String,
        author_id: u64,
        timestamp: u64,
        is_current_user: bool,
        flags: MessageFlags,
    ) -> Self {
        Message {
            id,
            content,
            author_id,
            timestamp,
            is_current_user,
            flags,
        }
    }
}

/// Tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_view_service_new() {
        let service = ChatViewService::new();
        assert_eq!(service.history_len(), 0);
        assert_eq!(service.unread_count(), 0);
        assert!(!service.is_selection_mode());
        assert_eq!(service.scroll_position(), 0);
    }

    #[test]
    fn test_add_message() {
        let mut service = ChatViewService::new();
        let message = Message::new(
            1,
            "Hello".to_string(),
            123,
            1000,
            false,
            MessageFlags::Normal,
        );
        service.add_message(message);
        assert_eq!(service.history_len(), 1);

        let message2 = Message::new(
            2,
            "World".to_string(),
            123,
            1001,
            false,
            MessageFlags::Normal,
        );
        service.add_message(message2);
        assert_eq!(service.history_len(), 2);
    }

    #[test]
    fn test_message_history_limit() {
        let mut service = ChatViewService::new();
        service.max_history = 3;

        for i in 0..5 {
            let message = Message::new(
                i,
                format!("Message {}", i),
                123,
                1000 + i,
                false,
                MessageFlags::Normal,
            );
            service.add_message(message);
        }
        assert_eq!(service.history_len(), 3); // Should be limited to max_history
    }

    #[test]
    fn test_message_flags() {
        let msg_normal = Message::new(
            1,
            "Normal".to_string(),
            123,
            1000,
            false,
            MessageFlags::Normal,
        );
        let msg_system = Message::new(
            2,
            "System".to_string(),
            123,
            1001,
            false,
            MessageFlags::System,
        );
        let msg_pinned = Message::new(
            3,
            "Pinned".to_string(),
            123,
            1002,
            false,
            MessageFlags::Pinned,
        );
        let msg_edited = Message::new(
            4,
            "Edited".to_string(),
            123,
            1003,
            false,
            MessageFlags::Edited,
        );

        assert_eq!(msg_normal.flags, MessageFlags::Normal);
        assert_eq!(msg_system.flags, MessageFlags::System);
        assert_eq!(msg_pinned.flags, MessageFlags::Pinned);
        assert_eq!(msg_edited.flags, MessageFlags::Edited);
    }

    #[test]
    fn test_chat_view_state_default() {
        let state = ChatViewState::default();
        assert_eq!(state.scroll_position, 0);
        assert!(!state.is_loading);
        assert_eq!(state.unread_count, 0);
        assert!(state.selected_message_id.is_none());
        assert!(!state.selection_mode);
    }

    #[test]
    fn test_chat_view_service_selection() {
        let mut service = ChatViewService::new();
        service.set_selection_mode(true);
        assert!(service.is_selection_mode());

        service.select_message(42);
        assert_eq!(service.state().selected_message_id, Some(42));

        service.deselect_message();
        assert!(service.state().selected_message_id.is_none());
    }
}
