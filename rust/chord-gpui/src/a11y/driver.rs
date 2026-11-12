//! Высокоуровневый Page-Object DSL драйвер (`AppDriver`) поверх D-Bus AT-SPI (`org.a11y.Bus`).

use crate::a11y::{
    A11yDriverError, AccessibilityTree, AccessibleElementHandle, AccessibleNode, AccessibleRole,
    AccessibleState, AtSpiActionReceipt,
};
use crate::state::AppState;
use chord_platform::dbus::{
    Value as DbusValue,
    message::{
        FIELD_DESTINATION, FIELD_INTERFACE, FIELD_MEMBER, FIELD_PATH, FIELD_SIGNATURE, MessageType,
        build as build_dbus_message,
    },
};

/// Высокоуровневый Page-Object DSL драйвер (`AppDriver`) поверх D-Bus AT-SPI (`org.a11y.Bus`).
pub struct AppDriver {
    pub state: AppState,
    pub locale: String,
    pub focused_id: Option<String>,
    pub dbus_wire_messages: Vec<Vec<u8>>,
    next_serial: u32,
}

impl AppDriver {
    pub fn new(state: AppState) -> Self {
        Self::with_locale(state, "ru")
    }

    pub fn with_locale(state: AppState, locale: &str) -> Self {
        Self {
            state,
            locale: locale.to_string(),
            focused_id: None,
            dbus_wire_messages: Vec::new(),
            next_serial: 1,
        }
    }

    /// Получение актуального семантического дерева доступности окна.
    pub fn tree(&self) -> AccessibilityTree {
        AccessibilityTree::from_app_state(&self.state, &self.locale, self.focused_id.as_deref())
    }

    /// Поиск элемента по строковому обозначению роли и локализованному `Accessible Name`.
    pub fn find_by_role_and_name(
        &self,
        role: &str,
        name: &str,
    ) -> Result<AccessibleElementHandle, A11yDriverError> {
        let parsed_role = AccessibleRole::parse_role(role)
            .ok_or_else(|| A11yDriverError::InvalidRole(role.to_string()))?;
        let tree = self.tree();
        let target = tree
            .nodes
            .iter()
            .rev()
            .find(|n| n.role == parsed_role && n.name == name)
            .ok_or_else(|| A11yDriverError::ElementNotFound {
                role: role.to_string(),
                name: name.to_string(),
            })?;
        Ok(Self::node_to_handle(target))
    }

    /// Поиск элемента по внутреннему семантическому идентификатору узла.
    pub fn find_by_id(&self, id: &str) -> Result<AccessibleElementHandle, A11yDriverError> {
        let tree = self.tree();
        let target = tree.nodes.iter().find(|n| n.id == id).ok_or_else(|| {
            A11yDriverError::ElementNotFound {
                role: "any".to_string(),
                name: id.to_string(),
            }
        })?;
        Ok(Self::node_to_handle(target))
    }

    /// Семантический клик по элементу через вызов D-Bus метода `org.a11y.atspi.Action.DoAction(0)`.
    pub fn click(
        &mut self,
        element: &AccessibleElementHandle,
    ) -> Result<AtSpiActionReceipt, A11yDriverError> {
        self.do_action(element, 0)
    }

    /// Выполнение действия `action_index` через D-Bus интерфейс `org.a11y.atspi.Action.DoAction`.
    pub fn do_action(
        &mut self,
        element: &AccessibleElementHandle,
        action_index: i32,
    ) -> Result<AtSpiActionReceipt, A11yDriverError> {
        let tree = self.tree();
        let node = tree
            .nodes
            .iter()
            .find(|n| n.id == element.id)
            .ok_or_else(|| A11yDriverError::ElementNotFound {
                role: element.role.atspi_role_name().to_string(),
                name: element.name.clone(),
            })?;

        if node.has_state(AccessibleState::Disabled) {
            return Err(A11yDriverError::ElementDisabled {
                id: node.id.clone(),
                name: node.name.clone(),
            });
        }
        if action_index != 0 {
            return Err(A11yDriverError::UnsupportedAction {
                id: node.id.clone(),
                action_index,
            });
        }

        let (serial, wire_len) = self.record_dbus_call(
            &element.object_path,
            "org.a11y.atspi.Action",
            "DoAction",
            "i",
            &[DbusValue::I32(action_index)],
        )?;
        self.focused_id = Some(element.id.clone());
        let handled = self.apply_semantic_action(&element.id);

        Ok(AtSpiActionReceipt {
            object_path: element.object_path.clone(),
            interface: "org.a11y.atspi.Action".to_string(),
            member: "DoAction".to_string(),
            action_index,
            dbus_serial: serial,
            wire_bytes_len: wire_len,
            handled,
        })
    }

    /// Посимвольный ввод текста через `org.a11y.atspi.EditableText.InsertText` и реактивный редьюсер.
    pub fn type_text(
        &mut self,
        element: &AccessibleElementHandle,
        text: &str,
    ) -> Result<usize, A11yDriverError> {
        let tree = self.tree();
        let node = tree
            .nodes
            .iter()
            .find(|n| n.id == element.id)
            .ok_or_else(|| A11yDriverError::ElementNotFound {
                role: element.role.atspi_role_name().to_string(),
                name: element.name.clone(),
            })?;

        if node.role != AccessibleRole::Text || !node.has_state(AccessibleState::Editable) {
            return Err(A11yDriverError::NotEditable {
                id: node.id.clone(),
                role: node.role,
            });
        }
        if node.has_state(AccessibleState::Disabled) {
            return Err(A11yDriverError::ElementDisabled {
                id: node.id.clone(),
                name: node.name.clone(),
            });
        }

        self.focused_id = Some(element.id.clone());
        let char_count = text.chars().count();
        self.record_dbus_call(
            &element.object_path,
            "org.a11y.atspi.EditableText",
            "InsertText",
            "isi",
            &[
                DbusValue::I32(0),
                DbusValue::String(text.to_string()),
                DbusValue::I32(char_count as i32),
            ],
        )?;

        for ch in text.chars() {
            self.dispatch_char_to_target(&element.id, ch);
        }
        Ok(char_count)
    }

    /// Нажатие горячих клавиш (`Ctrl+K`, `Ctrl+F`, `Ctrl+E`, `Escape`, `Enter`, `Space`, `Tab`, `Shift+Tab`, стрелки).
    pub fn press_hotkey(&mut self, hotkey: &str) -> Result<bool, A11yDriverError> {
        let normalized = hotkey.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "ctrl+k" => {
                self.state.toggle_quick_switcher();
                Ok(true)
            }
            "ctrl+f" => {
                self.state.toggle_search();
                Ok(true)
            }
            "ctrl+e" => {
                self.state.toggle_emoji_picker();
                Ok(true)
            }
            "escape" | "esc" => Ok(self.state.handle_escape_key()),
            "tab" => self.press_tab(false).map(|_| true),
            "shift+tab" => self.press_tab(true).map(|_| true),
            "space" => self.activate_focused_or_key("space"),
            "enter" => self.activate_focused_or_key("enter"),
            "arrowdown" | "down" => Ok(self.state.handle_key_input("arrowdown", false)),
            "arrowup" | "up" => Ok(self.state.handle_key_input("arrowup", false)),
            "backspace" => Ok(self.state.handle_key_input("backspace", false)),
            _ => Err(A11yDriverError::InvalidHotkey(hotkey.to_string())),
        }
    }

    /// Перемещение фокуса по клавише `Tab` (`reverse = false`) или `Shift+Tab` (`reverse = true`).
    pub fn press_tab(&mut self, reverse: bool) -> Result<AccessibleElementHandle, A11yDriverError> {
        let tree = self.tree();
        let focusables = tree.focusable_nodes_in_active_scope();
        if focusables.is_empty() {
            return Err(A11yDriverError::ElementNotFound {
                role: "focusable".to_string(),
                name: "tab".to_string(),
            });
        }

        let current_idx = self
            .focused_id
            .as_deref()
            .and_then(|id| focusables.iter().position(|n| n.id == id));

        let next_idx = match (current_idx, reverse) {
            (Some(idx), false) => (idx + 1) % focusables.len(),
            (Some(0), true) => focusables.len() - 1,
            (Some(idx), true) => idx - 1,
            (None, false) => 0,
            (None, true) => focusables.len() - 1,
        };

        let next_handle = Self::node_to_handle(focusables[next_idx]);
        self.focused_id = Some(next_handle.id.clone());
        self.record_dbus_call(
            &next_handle.object_path,
            "org.a11y.atspi.Component",
            "GrabFocus",
            "",
            &[],
        )?;
        Ok(next_handle)
    }

    fn activate_focused_or_key(&mut self, key: &str) -> Result<bool, A11yDriverError> {
        if let Some(focused_id) = self.focused_id.clone() {
            let tree = self.tree();
            if let Some(node) = tree.nodes.iter().find(|n| n.id == focused_id)
                && matches!(
                    node.role,
                    AccessibleRole::PushButton | AccessibleRole::ListItem
                )
                && !node.has_state(AccessibleState::Disabled)
            {
                let handle = Self::node_to_handle(node);
                let receipt = self.click(&handle)?;
                return Ok(receipt.handled);
            }
        }
        Ok(self.state.handle_key_input(key, false))
    }

    fn dispatch_char_to_target(&mut self, target_id: &str, ch: char) {
        match target_id {
            "input-login-email" => self.state.login_form.email_or_phone.push(ch),
            "input-login-password" => self.state.login_form.password.push(ch),
            _ => {
                let mut buf = [0u8; 4];
                let key_str = if ch == ' ' {
                    "space"
                } else {
                    ch.encode_utf8(&mut buf)
                };
                self.state.handle_key_input(key_str, false);
            }
        }
    }

    fn apply_semantic_action(&mut self, id: &str) -> bool {
        if let Some(gid) = id.strip_prefix("guild-item-") {
            self.state.select_guild(Some(gid.to_string()));
            return true;
        }
        if let Some(cid) = id.strip_prefix("channel-item-") {
            self.state.select_channel(cid.to_string());
            return true;
        }
        if id.starts_with("btn-close-modal-") {
            return self.state.handle_escape_key();
        }
        self.apply_button_action(id)
    }

    fn apply_button_action(&mut self, id: &str) -> bool {
        match id {
            "btn-voice-mute" => {
                self.state.toggle_mute();
                true
            }
            "btn-voice-deafen" => {
                self.state.toggle_deaf();
                true
            }
            "btn-user-settings" => {
                self.state.is_settings_open = !self.state.is_settings_open;
                true
            }
            "btn-add-server" => {
                self.state.open_create_server();
                true
            }
            "btn-chat-send" => self.state.handle_key_input("enter", false),
            "btn-open-emoji" => {
                self.state.toggle_emoji_picker();
                true
            }
            "btn-open-search" => {
                self.state.toggle_search();
                true
            }
            "btn-open-quick-switcher" => {
                self.state.toggle_quick_switcher();
                true
            }
            "btn-open-screen-share" => {
                self.state.open_screen_share_modal();
                true
            }
            "btn-login-submit" => self.state.submit_login().is_ok(),
            "btn-login-back" => {
                self.state.cancel_mfa();
                true
            }
            "btn-create-server-submit" => self.state.handle_key_input("enter", false),
            "btn-create-channel-submit" => self.state.handle_key_input("enter", false),
            "btn-go-live" => {
                self.state.start_screen_sharing("screen-full-1");
                true
            }
            _ => true,
        }
    }

    fn record_dbus_call(
        &mut self,
        path: &str,
        interface: &str,
        member: &str,
        signature: &str,
        body: &[DbusValue],
    ) -> Result<(u32, usize), A11yDriverError> {
        let serial = self.next_serial;
        self.next_serial = self.next_serial.saturating_add(1);
        let mut fields = vec![
            (FIELD_PATH, DbusValue::String(path.to_string())),
            (
                FIELD_DESTINATION,
                DbusValue::String("org.a11y.Bus".to_string()),
            ),
            (FIELD_INTERFACE, DbusValue::String(interface.to_string())),
            (FIELD_MEMBER, DbusValue::String(member.to_string())),
        ];
        if !signature.is_empty() {
            fields.push((FIELD_SIGNATURE, DbusValue::String(signature.to_string())));
        }
        let wire = build_dbus_message(MessageType::MethodCall, serial, &fields, signature, body)
            .map_err(|e| A11yDriverError::DbusWireError(format!("{e:?}")))?;
        let len = wire.len();
        self.dbus_wire_messages.push(wire);
        Ok((serial, len))
    }

    fn node_to_handle(node: &AccessibleNode) -> AccessibleElementHandle {
        AccessibleElementHandle {
            id: node.id.clone(),
            object_path: node.object_path.clone(),
            role: node.role,
            name: node.name.clone(),
        }
    }
}
