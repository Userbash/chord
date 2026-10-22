use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

// Dialog Service Module

// Provides CLDR locale validation and dialog state management for the Discord client.

// Static table of 31 supported CLDR locales for the Discord client.

#[allow(dead_code)]
static SUPPORTED_LOCALES: &[&str] = &[
    "en-US", "en-GB", "en-CA", "en-AU", "en-NZ", "en-ZA", "es-ES", "es-MX", "es-AR", "es-CO",
    "es-PE", "es-CL", "es-VE", "fr", "fr-FR", "de-DE", "it-IT", "nl-NL", "ru-RU", "uk-UA", "pl-PL",
    "cs-CZ", "sk-SK", "tr-TR", "ja-JP", "ko-KR", "zh-CN", "zh-TW", "hi-IN", "vi-VN", "th-Th",
];

// Validates a CLDR locale identifier against the supported locales table.

#[allow(dead_code)]
pub fn validate_cldr_locale(locale: &str) -> Result<(), String> {
    if locale.is_empty() {
        return Err("Locale string must not be empty".to_string());
    }

    let is_supported = SUPPORTED_LOCALES.contains(&locale);

    if !is_supported {
        return Err(format!(
            "Locale '{}' is not supported. Supported locales count: {}",
            locale,
            SUPPORTED_LOCALES.len()
        ));
    }

    Ok(())
}

// Type-safe wrapper around u64 for dialog identifiers.

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DialogId(u64);

impl DialogId {
    #[allow(dead_code)]
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    #[allow(dead_code)]
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

// Serialize implementation for DialogId.

impl Serialize for DialogId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u64(self.0)
    }
}

// Deserialize implementation for DialogId.

impl<'de> Deserialize<'de> for DialogId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let u64_val = u64::deserialize(deserializer)?;
        Ok(DialogId(u64_val))
    }
}

// Message payload structure sent between dialog and frontend.

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogMessage {
    pub dialog_id: DialogId,
    pub locale: String,
    pub message_key: String,
    pub user_input: Option<String>,
}

// Outcome of a dialog operation.

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogResult {
    Opened {
        window_label: String,
    },
    Sent {
        payload: String,
    },
    Closed {
        reason: String,
    },
    InvalidLocale {
        locale: String,
        supported: Vec<String>,
    },
    Error {
        message: String,
    },
    Cancelled {
        context: String,
    },
}

// Internal state tracking for a specific dialog window.

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DialogState {
    pub id: String,
    pub locale: String,
    pub last_updated: u64,
    pub last_message_key: Option<String>,
    pub is_open: bool,
}

// Configuration for a dialog window.

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct DialogConfig {
    pub dialog_id: String,
    pub title_key: String,
    pub fallback_title: String,
    pub closable: bool,
    pub modal: bool,
    pub locale: String,
}

// Service managing dialog windows and their lifecycle.

#[allow(dead_code)]
pub struct DialogService {
    active_dialogs: HashMap<String, DialogState>,
    next_id: u64,
}

impl Default for DialogService {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl DialogService {
    pub fn new() -> DialogService {
        DialogService {
            active_dialogs: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn open_window(
        &mut self,
        dialog_id: &str,
        locale: &str,
        _title_key: &str,
    ) -> Result<DialogId, String> {
        validate_cldr_locale(locale)?;

        if self.active_dialogs.contains_key(dialog_id) {
            return Err(format!("Dialog with ID '{}' already exists", dialog_id));
        }

        let state = DialogState {
            id: dialog_id.to_string(),
            locale: locale.to_string(),
            last_updated: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            last_message_key: None,
            is_open: true,
        };

        self.active_dialogs.insert(dialog_id.to_string(), state);

        let new_id = DialogId::new(self.next_id);
        self.next_id += 1;

        Ok(new_id)
    }

    pub fn send_message(
        &mut self,
        dialog_id: &str,
        locale: &str,
        message_key: &str,
    ) -> Result<DialogResult, String> {
        validate_cldr_locale(locale)?;

        let state = match self.active_dialogs.get(dialog_id) {
            Some(s) => s,
            None => return Err(format!("Dialog with ID '{}' not found", dialog_id)),
        };

        let mut state = state.clone();
        state.last_message_key = Some(message_key.to_string());
        state.last_updated = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        state.is_open = true;

        self.active_dialogs.insert(dialog_id.to_string(), state);

        Ok(DialogResult::Sent {
            payload: format!("Message '{}' sent to dialog '{}'", message_key, dialog_id),
        })
    }

    pub fn close_window(&mut self, dialog_id: &str) -> Result<(), String> {
        if !self.active_dialogs.contains_key(dialog_id) {
            return Err(format!("Dialog with ID '{}' not found", dialog_id));
        }

        let state = self.active_dialogs.get_mut(dialog_id).unwrap();
        if state.is_open {
            state.is_open = false;
            Ok(())
        } else {
            Err(format!("Dialog with ID '{}' is already closed", dialog_id))
        }
    }
}

// Oracle tests for DialogService (Red pattern -> Green pattern).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_cldr_locale_valid() {
        assert_eq!(validate_cldr_locale("en-US"), Ok(()));
        assert_eq!(validate_cldr_locale("ru-RU"), Ok(()));
        assert_eq!(validate_cldr_locale("fr"), Ok(()));
        assert_eq!(validate_cldr_locale("de-DE"), Ok(()));
    }

    #[test]
    fn test_validate_cldr_locale_invalid() {
        assert_eq!(
            validate_cldr_locale(""),
            Err("Locale string must not be empty".to_string())
        );
        assert_eq!(
            validate_cldr_locale("invalid-locale"),
            Err(
                "Locale 'invalid-locale' is not supported. Supported locales count: 31".to_string()
            )
        );
        assert_eq!(
            validate_cldr_locale("EN-US"),
            Err("Locale 'EN-US' is not supported. Supported locales count: 31".to_string())
        );
    }

    #[test]
    fn test_open_window() {
        let mut service = DialogService::new();

        let result = service.open_window("dialog-1", "en-US", "test_title");
        assert!(result.is_ok(), "Opening valid dialog should succeed");
        let id = result.unwrap();
        assert_eq!(id.as_u64(), 1, "First dialog should have ID 1");

        let result = service.open_window("dialog-1", "en-US", "test_title");
        assert!(result.is_err(), "Opening existing dialog should fail");
    }

    #[test]
    fn test_send_message() {
        let mut service = DialogService::new();

        service
            .open_window("dialog-1", "en-US", "test_title")
            .unwrap();

        let result = service.send_message("dialog-1", "en-US", "greeting");
        assert!(result.is_ok(), "Sending message should succeed");

        let result = service.send_message("dialog-1", "en-US", "nonexistent_key");
        assert!(
            result.is_ok(),
            "Sending message with unknown key should not crash"
        );
    }

    #[test]
    fn test_close_window() {
        let mut service = DialogService::new();

        service
            .open_window("dialog-1", "en-US", "test_title")
            .unwrap();

        let result = service.close_window("dialog-1");
        assert!(result.is_ok(), "Closing existing dialog should succeed");

        let result = service.close_window("dialog-1");
        assert!(result.is_err(), "Closing non-existent dialog should fail");
    }

    #[test]
    fn test_dialog_lifecycle() {
        let mut service = DialogService::new();

        service.open_window("dialog-1", "en-US", "title").unwrap();
        assert!(service.active_dialogs.get("dialog-1").unwrap().is_open);

        let msg_result = service.send_message("dialog-1", "en-US", "key");
        assert!(msg_result.is_ok());

        let close_result = service.close_window("dialog-1");
        assert!(close_result.is_ok());
        assert!(!service.active_dialogs.get("dialog-1").unwrap().is_open);
    }

    #[test]
    fn test_duplicate_dialog_id() {
        let mut service = DialogService::new();

        service.open_window("dialog-1", "en-US", "title").unwrap();

        let result = service.open_window("dialog-1", "en-US", "title");
        assert!(result.is_err(), "Opening duplicate dialog ID should fail");
    }

    #[test]
    fn test_close_nonexistent() {
        let mut service = DialogService::new();

        let result = service.close_window("nonexistent");
        assert!(result.is_err(), "Closing non-existent dialog should fail");
    }

    #[test]
    fn test_multiple_dialogs() {
        let mut service = DialogService::new();

        service.open_window("dialog-1", "en-US", "title1").unwrap();
        service.open_window("dialog-2", "en-GB", "title2").unwrap();
        service.open_window("dialog-3", "en-US", "title3").unwrap();

        assert_eq!(service.active_dialogs.len(), 3);
        assert!(service.active_dialogs.get("dialog-1").unwrap().is_open);
        assert!(service.active_dialogs.get("dialog-2").unwrap().is_open);
        assert!(service.active_dialogs.get("dialog-3").unwrap().is_open);

        service.close_window("dialog-2").unwrap();
        assert_eq!(service.active_dialogs.len(), 3);
        assert!(!service.active_dialogs.get("dialog-2").unwrap().is_open);
        assert!(service.active_dialogs.get("dialog-1").unwrap().is_open);
        assert!(service.active_dialogs.get("dialog-3").unwrap().is_open);
    }
}
