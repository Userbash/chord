//! Построение реактивного дерева доступности AT-SPI (`AccessibilityTree`) из `AppState`.

use crate::a11y::{A11yI18nKey, AccessibleNode, AccessibleRole, AccessibleState, AtSpiStateSet};
use crate::state::{AppState, CreateServerStep, LoginStep};

/// Семантическое дерево доступности окна `discord-gpui`.
#[derive(Debug, Clone)]
pub struct AccessibilityTree {
    pub root_id: String,
    pub locale: String,
    pub active_modal_scope: Option<String>,
    pub nodes: Vec<AccessibleNode>,
}

impl AccessibilityTree {
    /// Сборка актуального дерева доступности из состояния приложения и активной локали.
    pub fn from_app_state(state: &AppState, locale: &str, focused_id: Option<&str>) -> Self {
        let active_modal_scope = Self::detect_active_modal_scope(state);
        let mut nodes = Vec::with_capacity(32);

        Self::push_root_and_navigation(state, locale, focused_id, &mut nodes);
        Self::push_channels_and_voice_bar(state, locale, focused_id, &mut nodes);
        Self::push_chat_and_members(state, locale, focused_id, &mut nodes);
        Self::push_active_modals(state, locale, focused_id, &mut nodes);

        Self {
            root_id: "window-main".to_string(),
            locale: locale.to_string(),
            active_modal_scope,
            nodes,
        }
    }

    /// Фильтрация узлов дерева по заданной роли `AccessibleRole`.
    pub fn nodes_by_role(&self, role: AccessibleRole) -> Vec<&AccessibleNode> {
        self.nodes.iter().filter(|n| n.role == role).collect()
    }

    /// Поиск фокусируемых узлов в текущем активном контексте (модальное окно или главный экран).
    pub fn focusable_nodes_in_active_scope(&self) -> Vec<&AccessibleNode> {
        self.nodes
            .iter()
            .filter(|n| {
                n.has_state(AccessibleState::Focusable)
                    && !n.has_state(AccessibleState::Disabled)
                    && n.modal_scope == self.active_modal_scope
            })
            .collect()
    }

    /// Определение активного модального оверлея (в порядке приоритета клавиши Escape).
    pub fn detect_active_modal_scope(state: &AppState) -> Option<String> {
        let checks = [
            (state.active_context_menu.is_some(), "modal-context-menu"),
            (state.is_search_open, "modal-search"),
            (state.is_screen_share_modal_open, "modal-screen-share"),
            (state.is_login_modal_open, "modal-login"),
            (state.is_account_switcher_open, "modal-account-switcher"),
            (
                state.selected_member_profile.is_some(),
                "modal-member-profile",
            ),
            (state.is_create_channel_open, "modal-create-channel"),
            (state.is_emoji_picker_open, "modal-emoji-picker"),
            (state.is_status_picker_open, "modal-status-picker"),
            (state.is_quick_switcher_open, "modal-quick-switcher"),
            (state.is_settings_open, "modal-settings"),
            (state.is_create_server_open, "modal-create-server"),
            (state.active_captcha.is_some(), "modal-captcha"),
        ];
        checks
            .into_iter()
            .find_map(|(active, name)| active.then(|| name.to_string()))
    }

    fn push_root_and_navigation(
        state: &AppState,
        locale: &str,
        focused_id: Option<&str>,
        nodes: &mut Vec<AccessibleNode>,
    ) {
        nodes.push(make_static_node(
            "window-main",
            AccessibleRole::Window,
            A11yI18nKey::Servers,
            locale,
        ));
        nodes.push(make_static_node(
            "list-servers",
            AccessibleRole::List,
            A11yI18nKey::Servers,
            locale,
        ));
        for guild in &state.guilds {
            let id = format!("guild-item-{}", guild.id);
            let selected = state.selected_guild_id.as_deref() == Some(guild.id.as_str());
            let st = build_focusable_states(&id, focused_id, selected, false, false);
            nodes.push(make_node(
                &id,
                AccessibleRole::ListItem,
                A11yI18nKey::Servers,
                locale,
                Some(guild.name.clone()),
                st,
                "",
                None,
            ));
        }
        push_button_node(
            "btn-add-server",
            A11yI18nKey::AddServer,
            locale,
            focused_id,
            false,
            false,
            None,
            nodes,
        );
    }

    fn push_channels_and_voice_bar(
        state: &AppState,
        locale: &str,
        focused_id: Option<&str>,
        nodes: &mut Vec<AccessibleNode>,
    ) {
        nodes.push(make_static_node(
            "list-channels",
            AccessibleRole::List,
            A11yI18nKey::Channels,
            locale,
        ));
        if let Some(gid) = &state.selected_guild_id
            && let Some(guild) = state.guilds.iter().find(|g| &g.id == gid)
        {
            for ch in &guild.channels {
                let id = format!("channel-item-{}", ch.id);
                let sel = state.selected_channel_id.as_deref() == Some(ch.id.as_str());
                let st = build_focusable_states(&id, focused_id, sel, false, ch.is_locked);
                nodes.push(make_node(
                    &id,
                    AccessibleRole::ListItem,
                    A11yI18nKey::Channels,
                    locale,
                    Some(ch.name.clone()),
                    st,
                    "",
                    None,
                ));
            }
        }
        Self::push_voice_controls(state, locale, focused_id, nodes);
    }

    fn push_voice_controls(
        state: &AppState,
        locale: &str,
        focused_id: Option<&str>,
        nodes: &mut Vec<AccessibleNode>,
    ) {
        push_button_node(
            "btn-voice-mute",
            A11yI18nKey::MuteMicrophone,
            locale,
            focused_id,
            state.is_muted,
            false,
            None,
            nodes,
        );
        push_button_node(
            "btn-voice-deafen",
            A11yI18nKey::DeafenAudio,
            locale,
            focused_id,
            state.is_deafened,
            false,
            None,
            nodes,
        );
        push_button_node(
            "btn-user-settings",
            A11yI18nKey::UserSettings,
            locale,
            focused_id,
            false,
            false,
            None,
            nodes,
        );
    }

    fn push_chat_and_members(
        state: &AppState,
        locale: &str,
        focused_id: Option<&str>,
        nodes: &mut Vec<AccessibleNode>,
    ) {
        nodes.push(make_static_node(
            "list-messages",
            AccessibleRole::List,
            A11yI18nKey::DirectMessages,
            locale,
        ));
        nodes.push(make_static_node(
            "list-members",
            AccessibleRole::List,
            A11yI18nKey::Members,
            locale,
        ));
        push_text_input_node(
            "input-chat-composer",
            A11yI18nKey::SendMessage,
            &state.draft_text,
            locale,
            focused_id,
            None,
            nodes,
        );
        push_button_node(
            "btn-chat-send",
            A11yI18nKey::SendMessage,
            locale,
            focused_id,
            false,
            state.draft_text.trim().is_empty(),
            None,
            nodes,
        );
        for (id, key) in [
            ("btn-open-emoji", A11yI18nKey::EmojiPicker),
            ("btn-open-search", A11yI18nKey::Search),
            ("btn-open-quick-switcher", A11yI18nKey::QuickSwitcher),
            ("btn-open-screen-share", A11yI18nKey::ShareScreen),
        ] {
            push_button_node(id, key, locale, focused_id, false, false, None, nodes);
        }
    }

    fn push_active_modals(
        state: &AppState,
        locale: &str,
        focused_id: Option<&str>,
        nodes: &mut Vec<AccessibleNode>,
    ) {
        if state.is_login_modal_open {
            Self::push_login_modal(state, locale, focused_id, nodes);
        }
        if state.is_quick_switcher_open {
            Self::push_scoped_input_modal(
                "modal-quick-switcher",
                "input-quick-switcher",
                A11yI18nKey::QuickSwitcher,
                &state.quick_switcher_query,
                locale,
                focused_id,
                nodes,
            );
        }
        if state.is_search_open {
            Self::push_scoped_input_modal(
                "modal-search",
                "input-search-query",
                A11yI18nKey::Search,
                &state.search_query,
                locale,
                focused_id,
                nodes,
            );
        }
        if state.is_create_server_open {
            Self::push_create_server_modal(state, locale, focused_id, nodes);
        }
        Self::push_secondary_modals(state, locale, focused_id, nodes);
    }

    fn push_login_modal(
        state: &AppState,
        locale: &str,
        focused_id: Option<&str>,
        nodes: &mut Vec<AccessibleNode>,
    ) {
        let scope = Some("modal-login".to_string());
        nodes.push(make_node(
            "modal-login",
            AccessibleRole::Dialog,
            A11yI18nKey::LoginButton,
            locale,
            None,
            vec![AccessibleState::Showing, AccessibleState::Modal],
            "",
            scope.clone(),
        ));
        Self::push_login_fields(state, locale, focused_id, &scope, nodes);
        push_button_node(
            "btn-login-submit",
            A11yI18nKey::LoginButton,
            locale,
            focused_id,
            false,
            false,
            scope,
            nodes,
        );
        Self::push_modal_close_btn("modal-login", locale, focused_id, nodes);
    }

    fn push_login_fields(
        state: &AppState,
        locale: &str,
        focused_id: Option<&str>,
        scope: &Option<String>,
        nodes: &mut Vec<AccessibleNode>,
    ) {
        if matches!(state.login_form.step, LoginStep::Mfa { .. }) {
            push_button_node(
                "btn-login-back",
                A11yI18nKey::Back,
                locale,
                focused_id,
                false,
                false,
                scope.clone(),
                nodes,
            );
        } else {
            push_text_input_node(
                "input-login-email",
                A11yI18nKey::EmailOrPhone,
                &state.login_form.email_or_phone,
                locale,
                focused_id,
                scope.clone(),
                nodes,
            );
            push_text_input_node(
                "input-login-password",
                A11yI18nKey::Password,
                &state.login_form.password,
                locale,
                focused_id,
                scope.clone(),
                nodes,
            );
        }
    }

    fn push_create_server_modal(
        state: &AppState,
        locale: &str,
        focused_id: Option<&str>,
        nodes: &mut Vec<AccessibleNode>,
    ) {
        let scope = Some("modal-create-server".to_string());
        let (input_id, key, val) = match state.create_server_step {
            CreateServerStep::JoinByInvite => (
                "input-invite-code",
                A11yI18nKey::JoinServer,
                state.invite_input.as_str(),
            ),
            _ => (
                "input-server-name",
                A11yI18nKey::AddServer,
                state.new_server_name.as_str(),
            ),
        };
        push_text_input_node(input_id, key, val, locale, focused_id, scope.clone(), nodes);
        push_button_node(
            "btn-create-server-submit",
            A11yI18nKey::Submit,
            locale,
            focused_id,
            false,
            false,
            scope,
            nodes,
        );
        Self::push_modal_close_btn("modal-create-server", locale, focused_id, nodes);
    }

    fn push_scoped_input_modal(
        scope_id: &str,
        input_id: &str,
        key: A11yI18nKey,
        value: &str,
        locale: &str,
        focused_id: Option<&str>,
        nodes: &mut Vec<AccessibleNode>,
    ) {
        push_text_input_node(
            input_id,
            key,
            value,
            locale,
            focused_id,
            Some(scope_id.to_string()),
            nodes,
        );
        Self::push_modal_close_btn(scope_id, locale, focused_id, nodes);
    }

    fn push_secondary_modals(
        state: &AppState,
        locale: &str,
        focused_id: Option<&str>,
        nodes: &mut Vec<AccessibleNode>,
    ) {
        let specs = Self::dialog_overlay_specs(state)
            .into_iter()
            .chain(Self::popover_overlay_specs(state));
        for (active, scope_id, action_id, key) in specs {
            if active {
                push_button_node(
                    action_id,
                    key,
                    locale,
                    focused_id,
                    false,
                    false,
                    Some(scope_id.to_string()),
                    nodes,
                );
                Self::push_modal_close_btn(scope_id, locale, focused_id, nodes);
            }
        }
    }

    fn dialog_overlay_specs(
        state: &AppState,
    ) -> [(bool, &'static str, &'static str, A11yI18nKey); 5] {
        use A11yI18nKey as K;
        [
            (
                state.is_settings_open,
                "modal-settings",
                "btn-settings-action",
                K::UserSettings,
            ),
            (
                state.is_screen_share_modal_open,
                "modal-screen-share",
                "btn-go-live",
                K::ShareScreen,
            ),
            (
                state.active_captcha.is_some(),
                "modal-captcha",
                "btn-captcha-verify",
                K::Submit,
            ),
            (
                state.selected_member_profile.is_some(),
                "modal-member-profile",
                "btn-profile-action",
                K::Friends,
            ),
            (
                state.is_create_channel_open,
                "modal-create-channel",
                "btn-create-channel-submit",
                K::Submit,
            ),
        ]
    }

    fn popover_overlay_specs(
        state: &AppState,
    ) -> [(bool, &'static str, &'static str, A11yI18nKey); 4] {
        use A11yI18nKey as K;
        [
            (
                state.is_emoji_picker_open,
                "modal-emoji-picker",
                "btn-emoji-select",
                K::EmojiPicker,
            ),
            (
                state.is_status_picker_open,
                "modal-status-picker",
                "btn-status-select",
                K::Submit,
            ),
            (
                state.is_account_switcher_open,
                "modal-account-switcher",
                "btn-account-select",
                K::LoginButton,
            ),
            (
                state.active_context_menu.is_some(),
                "modal-context-menu",
                "btn-ctx-action",
                K::Submit,
            ),
        ]
    }

    fn push_modal_close_btn(
        scope_id: &str,
        locale: &str,
        focused_id: Option<&str>,
        nodes: &mut Vec<AccessibleNode>,
    ) {
        let btn_id = format!("btn-close-{scope_id}");
        push_button_node(
            &btn_id,
            A11yI18nKey::Close,
            locale,
            focused_id,
            false,
            false,
            Some(scope_id.to_string()),
            nodes,
        );
    }
}

fn build_focusable_states(
    id: &str,
    focused_id: Option<&str>,
    selected: bool,
    checked: bool,
    disabled: bool,
) -> Vec<AccessibleState> {
    let mut states = vec![AccessibleState::Showing, AccessibleState::Focusable];
    if selected {
        states.push(AccessibleState::Selected);
    }
    if checked {
        states.push(AccessibleState::Checked);
    }
    if disabled {
        states.push(AccessibleState::Disabled);
    }
    if focused_id == Some(id) {
        states.push(AccessibleState::Focused);
    }
    states
}

#[allow(clippy::too_many_arguments)]
fn push_button_node(
    id: &str,
    key: A11yI18nKey,
    locale: &str,
    focused_id: Option<&str>,
    checked: bool,
    disabled: bool,
    modal_scope: Option<String>,
    nodes: &mut Vec<AccessibleNode>,
) {
    let st = build_focusable_states(id, focused_id, false, checked, disabled);
    nodes.push(make_node(
        id,
        AccessibleRole::PushButton,
        key,
        locale,
        None,
        st,
        "",
        modal_scope,
    ));
}

fn push_text_input_node(
    id: &str,
    key: A11yI18nKey,
    value: &str,
    locale: &str,
    focused_id: Option<&str>,
    modal_scope: Option<String>,
    nodes: &mut Vec<AccessibleNode>,
) {
    let mut st = build_focusable_states(id, focused_id, false, false, false);
    st.push(AccessibleState::Editable);
    nodes.push(make_node(
        id,
        AccessibleRole::Text,
        key,
        locale,
        None,
        st,
        value,
        modal_scope,
    ));
}

fn make_static_node(
    id: &str,
    role: AccessibleRole,
    key: A11yI18nKey,
    locale: &str,
) -> AccessibleNode {
    make_node(
        id,
        role,
        key,
        locale,
        None,
        vec![AccessibleState::Showing],
        "",
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn make_node(
    id: &str,
    role: AccessibleRole,
    key: A11yI18nKey,
    locale: &str,
    custom_name: Option<String>,
    states: Vec<AccessibleState>,
    value_text: &str,
    modal_scope: Option<String>,
) -> AccessibleNode {
    let state_set = AtSpiStateSet::from_states(&states);
    let sanitized = id.replace('-', "_");
    AccessibleNode {
        id: id.to_string(),
        object_path: format!("/org/a11y/atspi/accessible/{sanitized}"),
        role,
        name: custom_name.unwrap_or_else(|| key.resolve_name(locale).to_string()),
        description: key.resolve_description(locale).to_string(),
        i18n_key: key.catalog_hash().to_string(),
        states,
        state_set,
        actions: vec!["click".to_string()],
        value_text: value_text.to_string(),
        modal_scope,
        children_ids: Vec::new(),
    }
}
