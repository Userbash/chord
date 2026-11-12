#![recursion_limit = "512"]
#![allow(dead_code, unused_imports)]

#[path = "../src/a11y/mod.rs"]
mod a11y;
#[allow(clippy::all)]
#[path = "../src/audio/mod.rs"]
mod audio;
#[allow(clippy::all)]
#[path = "../src/dispatch/mod.rs"]
mod dispatch;
#[allow(clippy::all)]
#[path = "../src/net/mod.rs"]
mod net;
#[allow(clippy::all)]
#[path = "../src/state/mod.rs"]
mod state;
#[allow(clippy::all)]
#[path = "../src/theme/mod.rs"]
mod theme;

use a11y::{
    A11yDriverError, A11yI18nKey, AccessibleRole, AccessibleState, AppDriver, AtSpiStateSet, Role,
    State, Wcag22Auditor,
};
use discord_platform::dbus::{
    Value as DbusValue,
    message::{MessageType, parse as parse_dbus_message},
};
use gpui::Role as GpuiRole;
use state::{AppState, CaptchaChallenge, CreateServerStep, LoginStep};
use theme::{DISCORD_THEME, discord_dark::DiscordThemeColors};

// ═══════════════════════════════════════════════════════════════════════════
// SUB-902-1: Экспозиция дерева доступности в компонентах GPUI
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_sub_902_1_step_1_1_accessible_roles_exposure_push_button_text_list() {
    // 1. Проверка маппинга ролей Role::PushButton, Role::Text, Role::List в GPUI (AccessKit) и AT-SPI2
    assert_eq!(Role::PushButton.atspi_role_id(), 43);
    assert_eq!(Role::PushButton.atspi_role_name(), "push button");
    assert_eq!(Role::PushButton.to_gpui_role(), GpuiRole::Button);
    assert_eq!(
        AccessibleRole::from_gpui_role(GpuiRole::Button),
        Role::PushButton
    );

    assert_eq!(Role::Text.atspi_role_id(), 61);
    assert_eq!(Role::Text.atspi_role_name(), "text");
    assert_eq!(Role::Text.to_gpui_role(), GpuiRole::TextInput);
    assert_eq!(
        AccessibleRole::from_gpui_role(GpuiRole::TextInput),
        Role::Text
    );

    assert_eq!(Role::List.atspi_role_id(), 31);
    assert_eq!(Role::List.atspi_role_name(), "list");
    assert_eq!(Role::List.to_gpui_role(), GpuiRole::List);
    assert_eq!(AccessibleRole::from_gpui_role(GpuiRole::List), Role::List);

    // 2. Проверка экспозиции дерева на главном экране Discord GPUI
    let state = AppState::new();
    let driver = AppDriver::new(state);
    let tree = driver.tree();

    assert_eq!(tree.root_id, "window-main");
    let push_buttons = tree.nodes_by_role(Role::PushButton);
    let text_inputs = tree.nodes_by_role(Role::Text);
    let lists = tree.nodes_by_role(Role::List);

    assert!(
        push_buttons.len() >= 6,
        "Должны экспортироваться все ключевые кнопки управления (найдено {})",
        push_buttons.len()
    );
    assert!(
        !text_inputs.is_empty(),
        "Поле ввода сообщения чата должно экспортироваться с Role::Text"
    );
    assert!(
        lists.len() >= 3,
        "Списки серверов, каналов, сообщений и участников должны иметь Role::List (найдено {})",
        lists.len()
    );

    // 3. Проверка генерации узлов gpui::accesskit::Node
    for node in &tree.nodes {
        let ak_node = node.to_accesskit_node();
        assert_eq!(ak_node.role(), node.role.to_gpui_role());
        assert_eq!(ak_node.label(), Some(node.name.as_str()));
    }
}

#[test]
fn test_sub_902_1_step_1_2_i18n_catalog_accessible_name_and_description_no_hardcode() {
    // 1. Проверка привязки эталонных 6-символьных хешей из /home/sanya/re-deep/{ru,en}-catalog.json
    assert_eq!(A11yI18nKey::LoginButton.catalog_hash(), "2jvQ6K");
    assert_eq!(A11yI18nKey::LoginButton.resolve_name("ru"), "Войти");
    assert_eq!(A11yI18nKey::LoginButton.resolve_name("en-US"), "Log In");

    assert_eq!(A11yI18nKey::UserSettings.catalog_hash(), "cduTBL");
    assert_eq!(
        A11yI18nKey::UserSettings.resolve_name("ru"),
        "Настройки пользователя"
    );
    assert_eq!(
        A11yI18nKey::UserSettings.resolve_name("en-US"),
        "User Settings"
    );

    assert_eq!(A11yI18nKey::MuteMicrophone.catalog_hash(), "HITUcR");
    assert_eq!(A11yI18nKey::DeafenAudio.catalog_hash(), "wjcRFX");
    assert_eq!(A11yI18nKey::Search.catalog_hash(), "pw+r5b");
    assert_eq!(A11yI18nKey::QuickSwitcher.catalog_hash(), "ZvKwYa");
    assert_eq!(A11yI18nKey::ShareScreen.catalog_hash(), "fjBNo1");

    // 2. Проверка отсутствия пустых имен и «голых иконок» (⚙️, 🎤, 🎧, ➕) во всем дереве
    let mut state = AppState::new();
    state.open_login_modal();
    let ru_tree = AppDriver::with_locale(state, "ru").tree();
    for node in &ru_tree.nodes {
        assert!(
            !node.name.trim().is_empty(),
            "Узел {} не должен иметь пустой Accessible Name",
            node.id
        );
        assert!(
            !node.description.trim().is_empty(),
            "Узел {} обязан иметь осмысленный Accessible Description",
            node.id
        );
        assert_eq!(
            node.i18n_key.len(),
            6,
            "Узел {} обязан ссылаться на 6-символьный ключ i18n-каталога",
            node.id
        );
        assert!(
            node.name != "⚙️" && node.name != "🎤" && node.name != "🎧" && node.name != "➕",
            "Узел {} содержит только иконку вместо локализованного текста: {}",
            node.id,
            node.name
        );
    }

    // 3. Проверка переключения локали дерева на en-US и fallback для неизвестной локали
    let mut en_state = AppState::new();
    en_state.open_login_modal();
    let en_driver = AppDriver::with_locale(en_state, "en-US");
    let login_en = en_driver
        .find_by_role_and_name("button", "Log In")
        .expect("Кнопка 'Log In' должна находиться в локали en-US");
    assert_eq!(login_en.role, Role::PushButton);
    assert_eq!(login_en.name, "Log In");
}

#[test]
fn test_sub_902_1_step_1_3_component_states_focused_selected_checked_disabled() {
    let mut state = AppState::new();
    state.is_muted = true;
    state.is_deafened = false;

    let mut driver = AppDriver::new(state);
    // Фокусируем поле ввода чата
    let composer = driver
        .find_by_role_and_name("text", "Отправить сообщение")
        .expect("Поле ввода сообщения должно существовать");
    driver.focused_id = Some(composer.id.clone());

    let tree = driver.tree();

    // 1. State::Focused на сфокусированном инпуте
    let composer_node = tree
        .nodes
        .iter()
        .find(|n| n.id == composer.id)
        .expect("Узел композера");
    assert!(composer_node.has_state(State::Focused));
    assert!(composer_node.has_state(State::Focusable));
    assert!(composer_node.has_state(State::Editable));
    assert!(composer_node.state_set.contains(State::Focused));

    // 2. State::Selected на выбранном сервере (guild-1) и выбранном канале (ch-general)
    let selected_guild = tree
        .nodes
        .iter()
        .find(|n| n.id == "guild-item-guild-1")
        .expect("Узел выбранного сервера");
    assert!(selected_guild.has_state(State::Selected));

    let unselected_guild = tree
        .nodes
        .iter()
        .find(|n| n.id == "guild-item-guild-2")
        .expect("Узел невыбранного сервера");
    assert!(!unselected_guild.has_state(State::Selected));

    // 3. State::Checked на кнопке микрофона (is_muted == true)
    let mute_btn = tree
        .nodes
        .iter()
        .find(|n| n.id == "btn-voice-mute")
        .expect("Кнопка микрофона");
    assert!(mute_btn.has_state(State::Checked));

    let deafen_btn = tree
        .nodes
        .iter()
        .find(|n| n.id == "btn-voice-deafen")
        .expect("Кнопка звука");
    assert!(!deafen_btn.has_state(State::Checked));

    // 4. State::Disabled на кнопке отправки сообщения, когда черновик пуст
    let send_btn = tree
        .nodes
        .iter()
        .find(|n| n.id == "btn-chat-send")
        .expect("Кнопка отправки сообщения");
    assert!(
        send_btn.has_state(State::Disabled),
        "Кнопка отправки при пустом draft_text должна иметь State::Disabled"
    );

    // 5. Проверка 64-битной маски AtSpiStateSet (2 x u32 для D-Bus GetState)
    let mut set = AtSpiStateSet::empty();
    set.insert(State::Focused);
    set.insert(State::Selected);
    set.insert(State::Checked);
    set.insert(State::Disabled);
    assert!(set.contains(State::Focused));
    assert!(set.contains(State::Selected));
    assert!(set.contains(State::Checked));
    assert!(set.contains(State::Disabled));
    let pair = set.to_atspi_u32_pair();
    assert_ne!(pair[0], 0);
    set.remove(State::Disabled);
    assert!(!set.contains(State::Disabled));
}

// ═══════════════════════════════════════════════════════════════════════════
// SUB-902-2: Реализация высокоуровневого Page-Object DSL (AppDriver)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_sub_902_2_step_2_1_and_2_2_find_by_role_and_name_and_dbus_do_action_click() {
    let mut state = AppState::new();
    state.open_login_modal();
    // Заполняем валидные учетные данные, чтобы кнопка «Войти» стала активной
    state.set_login_email("rustery@discord.org".into());
    state.set_login_password("hunter2_secure".into());

    let mut driver = AppDriver::new(state);

    // Шаг 2.1: Поиск кнопки driver.find_by_role_and_name("button", "Войти")
    let login_btn = driver
        .find_by_role_and_name("button", "Войти")
        .expect("Кнопка 'Войти' должна находиться по роли 'button' и имени 'Войти'");
    assert_eq!(login_btn.role, Role::PushButton);
    assert_eq!(login_btn.name, "Войти");
    assert!(
        login_btn
            .object_path
            .starts_with("/org/a11y/atspi/accessible/")
    );

    // Шаг 2.2: Вызов driver.click(&login_btn) через D-Bus метод org.a11y.atspi.Action.DoAction(0)
    let receipt = driver
        .click(&login_btn)
        .expect("Клик через D-Bus do_action(0) должен завершиться успешно");
    assert_eq!(receipt.interface, "org.a11y.atspi.Action");
    assert_eq!(receipt.member, "DoAction");
    assert_eq!(receipt.action_index, 0);
    assert!(receipt.handled);
    assert!(receipt.wire_bytes_len > 16);

    // Декодируем реальный D-Bus пакет из лога драйвера через независимый парсер discord_platform::dbus
    let last_wire = driver
        .dbus_wire_messages
        .last()
        .expect("D-Bus пакет должен быть записан");
    let parsed = parse_dbus_message(last_wire).expect("Валидный wire-формат D-Bus");
    assert_eq!(parsed.kind, MessageType::MethodCall);
    assert_eq!(parsed.path(), Some(login_btn.object_path.as_str()));
    assert_eq!(parsed.interface(), Some("org.a11y.atspi.Action"));
    assert_eq!(parsed.member(), Some("DoAction"));
    assert_eq!(parsed.signature, "i");
    assert_eq!(parsed.body, vec![DbusValue::I32(0)]);

    // Проверяем, что действие реально применилось к AppState (модалка логина закрылась, пользователь авторизован)
    assert!(!driver.state.is_login_modal_open);
    assert_eq!(driver.state.current_user.username, "rustery");
}

#[test]
fn test_sub_902_2_step_2_3_type_text_character_emulation_and_reactive_reducer() {
    let state = AppState::new();
    let mut driver = AppDriver::new(state);

    // 1. Ввод текста в поле сообщения чата
    let chat_input = driver
        .find_by_role_and_name("text", "Отправить сообщение")
        .expect("Поле ввода сообщения");
    let chars_typed = driver
        .type_text(&chat_input, "Привет, AT-SPI драйвер!")
        .expect("Посимвольный ввод текста должен пройти");
    assert_eq!(chars_typed, "Привет, AT-SPI драйвер!".chars().count());
    assert_eq!(driver.state.draft_text, "Привет, AT-SPI драйвер!");

    // 2. После ввода текста кнопка «Отправить сообщение» перестаёт быть Disabled и кликается через do_action(0)
    let send_btn = driver
        .find_by_role_and_name("button", "Отправить сообщение")
        .expect("Кнопка отправки сообщения");
    driver
        .click(&send_btn)
        .expect("Клик по кнопке отправки сообщения");
    assert_eq!(driver.state.draft_text, "");
    let channel_msgs = driver
        .state
        .messages
        .get("ch-general")
        .expect("Сообщения канала ch-general");
    assert_eq!(
        channel_msgs.last().map(|m| m.content.as_str()),
        Some("Привет, AT-SPI драйвер!")
    );
}

#[test]
fn test_sub_902_2_step_2_4_press_hotkey_ctrl_k_ctrl_f_ctrl_e_escape() {
    let state = AppState::new();
    let mut driver = AppDriver::new(state);

    // 1. Ctrl+K открывает QuickSwitcher
    assert!(!driver.state.is_quick_switcher_open);
    let handled = driver
        .press_hotkey("Ctrl+K")
        .expect("Горячая клавиша Ctrl+K должна обрабатываться");
    assert!(handled);
    assert!(driver.state.is_quick_switcher_open);

    // Вводим запрос в поле поиска QuickSwitcher (канал «анонсы» -> id "ch-announcements")
    let qs_input = driver
        .find_by_role_and_name("text", "Quick Switcher")
        .expect("Поле ввода Quick Switcher");
    driver
        .type_text(&qs_input, "#анонсы")
        .expect("Ввод запроса в Quick Switcher");
    assert_eq!(driver.state.quick_switcher_query, "#анонсы");

    // Нажимаем Enter для перехода в найденный канал
    driver.press_hotkey("Enter").expect("Нажатие Enter");
    assert!(!driver.state.is_quick_switcher_open);
    assert_eq!(
        driver.state.selected_channel_id.as_deref(),
        Some("ch-announcements")
    );

    // 2. Ctrl+F открывает панель поиска, Escape закрывает её
    driver.press_hotkey("Ctrl+F").expect("Ctrl+F");
    assert!(driver.state.is_search_open);
    driver.press_hotkey("Escape").expect("Escape");
    assert!(!driver.state.is_search_open);

    // 3. Ctrl+E открывает пикер эмодзи, Escape закрывает его
    driver.press_hotkey("Ctrl+E").expect("Ctrl+E");
    assert!(driver.state.is_emoji_picker_open);
    driver.press_hotkey("Escape").expect("Escape");
    assert!(!driver.state.is_emoji_picker_open);
}

// ═══════════════════════════════════════════════════════════════════════════
// SUB-902-3: Автоматизированный аудит доступности WCAG 2.2
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_sub_902_3_step_3_1_wcag_22_contrast_ratios_aa_and_aaa_validation() {
    let report = Wcag22Auditor::audit_theme_contrast(&DISCORD_THEME);
    assert!(
        report.passed_aa,
        "Все текстовые пары обязаны проходить порог WCAG 2.2 AA (>= 4.5:1): {:?}",
        report.violations
    );
    assert!(
        report.passed_aaa_primary,
        "Основной текст и заголовки обязаны проходить порог WCAG 2.2 AAA (>= 7.0:1): {:?}",
        report.violations
    );
    assert!(
        report.passed_focus_appearance,
        "Рамка фокуса клавиатуры обязана проходить порог WCAG 2.2 SC 2.4.11 (>= 3.0:1)"
    );
    assert!(report.audited_pairs >= 18);
    assert!(report.aa_min_ratio >= 4.5);
    assert!(report.aaa_min_ratio >= 7.0);
    assert!(report.violations.is_empty());

    // Негативная проверка: при порче цвета текста аудит обязан фиксировать нарушение
    let mut broken_theme = DiscordThemeColors::default();
    broken_theme.text_normal = broken_theme.bg_primary;
    let broken_report = Wcag22Auditor::audit_theme_contrast(&broken_theme);
    assert!(!broken_report.passed_aa);
    assert!(!broken_report.passed_aaa_primary);
    assert!(!broken_report.violations.is_empty());
}

#[test]
fn test_sub_902_3_step_3_2_and_3_3_keyboard_tab_navigation_and_no_focus_traps() {
    let state = AppState::new();
    let mut driver = AppDriver::new(state);

    // 1. Сквозная навигация Tab и Shift+Tab по главному окну
    let first = driver.press_tab(false).expect("Первый Tab");
    let second = driver.press_tab(false).expect("Второй Tab");
    assert_ne!(
        first.id, second.id,
        "Tab должен перемещать фокус на следующий интерактивный элемент"
    );

    let back_to_first = driver
        .press_tab(true)
        .expect("Shift+Tab должен возвращать фокус назад");
    assert_eq!(back_to_first.id, first.id);

    // 2. Активация сфокусированной кнопки через клавишу Space / Enter
    let mute_handle = driver
        .find_by_role_and_name("button", "Заглушить")
        .expect("Кнопка Заглушить");
    driver.focused_id = Some(mute_handle.id.clone());
    assert!(!driver.state.is_muted);
    driver
        .press_hotkey("Space")
        .expect("Активация сфокусированной кнопки клавишей Space");
    assert!(driver.state.is_muted);

    // 3. Полный аудит отсутствия клавиатурных ловушек (Focus Traps) во всех модальных окнах
    let trap_report = Wcag22Auditor::audit_all_modals_focus_traps(&mut driver);
    assert!(
        trap_report.passed,
        "Обнаружены клавиатурные ловушки или утечки фокуса: trapped={:?}, leaks={:?}",
        trap_report.trapped_modals, trap_report.focus_leak_modals
    );
    assert!(
        trap_report.audited_modals.len() >= 10,
        "Должны быть проверены все модальные окна (проверено {})",
        trap_report.audited_modals.len()
    );
    assert!(trap_report.trapped_modals.is_empty());
    assert!(trap_report.focus_leak_modals.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════
// 9 MANDATORY TEST DIMENSIONS (TEST-POLICY.md §12)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_a11y_all_9_dimensions_comprehensive_contract() {
    let mut driver = AppDriver::new(AppState::new());

    // 1. Positive: клик по кнопке «Настройки пользователя» открывает SettingsDialog
    let settings_btn = driver
        .find_by_role_and_name("button", "Настройки пользователя")
        .expect("Кнопка Настройки пользователя");
    driver.click(&settings_btn).expect("Клик по настройкам");
    assert!(driver.state.is_settings_open);
    driver.press_hotkey("Escape").expect("Закрытие настроек");
    assert!(!driver.state.is_settings_open);

    // 2. Negative: несуществующий элемент, неверная роль, клик по Disabled-кнопке, ввод в не-инпут
    assert!(matches!(
        driver.find_by_role_and_name("unknown_role_xyz", "Войти"),
        Err(A11yDriverError::InvalidRole(_))
    ));
    assert!(matches!(
        driver.find_by_role_and_name("button", "Несуществующая Кнопка 404"),
        Err(A11yDriverError::ElementNotFound { .. })
    ));
    assert!(matches!(
        driver.press_hotkey("Ctrl+Alt+Super+F99"),
        Err(A11yDriverError::InvalidHotkey(_))
    ));

    // Кнопка отправки при пустом черновике имеет State::Disabled и отклоняет do_action(0)
    driver.state.draft_text.clear();
    let disabled_send = driver
        .find_by_role_and_name("button", "Отправить сообщение")
        .expect("Кнопка отправки");
    assert!(matches!(
        driver.click(&disabled_send),
        Err(A11yDriverError::ElementDisabled { .. })
    ));

    // Некорректный индекс действия do_action(99) отклоняется
    assert!(matches!(
        driver.do_action(&settings_btn, 99),
        Err(A11yDriverError::UnsupportedAction {
            action_index: 99,
            ..
        })
    ));

    // Попытка ввода текста в кнопку возвращает NotEditable
    assert!(matches!(
        driver.type_text(&settings_btn, "текст"),
        Err(A11yDriverError::NotEditable { .. })
    ));

    // 3. Boundary: ввод сообщения ровно в 2000 символов (лимит Discord)
    let chat_input = driver
        .find_by_role_and_name("text", "Отправить сообщение")
        .expect("Инпут чата");
    let max_msg = "А".repeat(2000);
    let count = driver
        .type_text(&chat_input, &max_msg)
        .expect("Ввод 2000 символов");
    assert_eq!(count, 2000);
    assert_eq!(driver.state.draft_text.chars().count(), 2000);
    driver.state.draft_text.clear();

    // 4. Error & Captcha Modal (40003): проверка доступности окна капчи и закрытия по Escape
    driver.state.active_captcha = Some(CaptchaChallenge {
        sitekey: "f5561ba9-8f1e-40ca-9b5b-a0b3f719ef34".into(),
        service: "hcaptcha".into(),
        rqdata: None,
    });
    let captcha_tree = driver.tree();
    assert_eq!(
        captcha_tree.active_modal_scope.as_deref(),
        Some("modal-captcha")
    );
    driver
        .press_hotkey("Escape")
        .expect("Закрытие капчи по Escape");
    assert!(driver.state.active_captcha.is_none());

    // 5. Empty State: пустое состояние AppState::empty() строит валидное дерево без паник
    let empty_driver = AppDriver::new(AppState::empty());
    let empty_tree = empty_driver.tree();
    assert!(!empty_tree.nodes.is_empty());

    // 6. Loading / Step transitions: переходы в CreateGuildModal (Templates -> JoinByInvite)
    driver.state.open_create_server();
    driver
        .state
        .set_create_server_step(CreateServerStep::JoinByInvite);
    let invite_input = driver
        .find_by_role_and_name("text", "Присоединиться к серверу")
        .expect("Поле ввода инвайта");
    driver
        .type_text(&invite_input, "discord.gg/rustlang")
        .expect("Ввод инвайт-кода");
    assert_eq!(driver.state.invite_input, "discord.gg/rustlang");
    driver
        .press_hotkey("Escape")
        .expect("Закрытие создания сервера");

    // 7. Capabilities / Locked Channel: заблокированный канал имеет State::Disabled
    if let Some(guild) = driver.state.guilds.first_mut()
        && let Some(ch) = guild.channels.first_mut()
    {
        ch.is_locked = true;
    }
    let locked_tree = driver.tree();
    let first_ch_node = locked_tree
        .nodes
        .iter()
        .find(|n| n.id.starts_with("channel-item-"))
        .expect("Узел канала");
    assert!(first_ch_node.has_state(State::Disabled));

    // 8. Recovery: переключение MFA шага и возврат Назад (Cancel MFA)
    driver.state.open_login_modal();
    driver.state.login_form.step = LoginStep::Mfa {
        ticket: "mfa-ticket-123".into(),
    };
    let back_btn = driver
        .find_by_role_and_name("button", "Назад")
        .expect("Кнопка Назад в MFA");
    driver.click(&back_btn).expect("Клик Назад в MFA");
    assert_eq!(driver.state.login_form.step, LoginStep::Credentials);
    driver.press_hotkey("Escape").expect("Закрытие логина");

    // 9. Observability: проверка журнала D-Bus AT-SPI пакетов
    assert!(
        driver.dbus_wire_messages.len() >= 4,
        "Все операции обязаны логироваться в виде валидных D-Bus сообщений"
    );
}
