//! Контракт BE-P1-09: Системный трей Linux, иконки состояний, бейдж непрочитанных и 6-пунктовое меню.
//!
//! Оракулы:
//! - `RE/main-arch.json` §9: TRAY_ICON_NAMES (tray, tray-unread, tray-speaking, tray-muted, tray-deafened)
//! - `RE/backend-ipc.json`: DISCORD_APP_SET_BADGE_COUNT, com.canonical.Unity.LauncherEntry
//! - DoD BE-P1-09:
//!   1. Трей появляется без окна, меню содержит 6 пунктов, «Quit» завершает процесс.
//!   2. Состояние иконки меняется по mute/deafen/входящему звонку.
//!   3. Нет иконки в системе → приложение работает, пишет причину в лог (не падает).

use chord_platform::dbus::Value;
use chord_platform::tray::{
    BadgeState, CloseDisposition, DEFAULT_DESKTOP_ENTRY, MinimizeToTrayPolicy, MockTrayBackend,
    TrayController, TrayError, TrayIconState, TrayMenu, TrayMenuAction, TrayMenuItemType,
    UNITY_LAUNCHER_ENTRY_INTERFACE, UNITY_LAUNCHER_ENTRY_SIGNAL, compute_tray_icon_state,
    format_badge_count, should_hide_on_close,
};

#[test]
fn test_tray_icon_state_mapping_and_oracle() {
    // 5 эталонных состояний иконки
    assert_eq!(TrayIconState::Normal.icon_name(), "discord-tray");
    assert_eq!(TrayIconState::Unread.icon_name(), "discord-tray-unread");
    assert_eq!(TrayIconState::Speaking.icon_name(), "discord-tray-speaking");
    assert_eq!(TrayIconState::Muted.icon_name(), "discord-tray-muted");
    assert_eq!(TrayIconState::Deafened.icon_name(), "discord-tray-deafened");

    // Имена ассетов из main-arch.json §9
    assert_eq!(TrayIconState::Normal.asset_name(), "tray");
    assert_eq!(TrayIconState::Unread.asset_name(), "tray-unread");
    assert_eq!(TrayIconState::Speaking.asset_name(), "tray-speaking");
    assert_eq!(TrayIconState::Muted.asset_name(), "tray-muted");
    assert_eq!(TrayIconState::Deafened.asset_name(), "tray-deafened");

    // Алиас InCall -> Speaking
    assert_eq!(TrayIconState::IN_CALL, TrayIconState::Speaking);
}

#[test]
fn test_compute_tray_icon_state_priority() {
    // 1. Дефолт (не в звонке, нет непрочитанных)
    assert_eq!(
        compute_tray_icon_state(false, false, false, 0),
        TrayIconState::Normal
    );

    // 2. Непрочитанные сообщения вне звонка
    assert_eq!(
        compute_tray_icon_state(false, false, false, 3),
        TrayIconState::Unread
    );

    // 3. В звонке (активен, микрофон/звук включены)
    assert_eq!(
        compute_tray_icon_state(true, false, false, 0),
        TrayIconState::Speaking
    );

    // 4. В звонке с непрочитанными — статус звонка имеет приоритет над unread
    assert_eq!(
        compute_tray_icon_state(true, false, false, 10),
        TrayIconState::Speaking
    );

    // 5. В звонке, микрофон отключен (self_mute)
    assert_eq!(
        compute_tray_icon_state(true, true, false, 0),
        TrayIconState::Muted
    );

    // 6. В звонке, звук отключен (self_deaf) — высший приоритет в звонке
    assert_eq!(
        compute_tray_icon_state(true, false, true, 0),
        TrayIconState::Deafened
    );

    // 7. В звонке, и mute, и deafen -> Deafened побеждает
    assert_eq!(
        compute_tray_icon_state(true, true, true, 5),
        TrayIconState::Deafened
    );
}

#[test]
fn test_tray_menu_structure_and_order() {
    let menu = TrayMenu::default();

    // Ровно 6 пунктов меню
    assert_eq!(menu.len(), 6);

    // Проверка порядка и типов пунктов
    let items = &menu.items;
    assert_eq!(items[0].action, TrayMenuAction::Open);
    assert_eq!(items[0].label, "Открыть Discord");
    assert_eq!(items[0].item_type, TrayMenuItemType::Normal);
    assert!(items[0].enabled);

    assert_eq!(items[1].action, TrayMenuAction::Mute);
    assert_eq!(items[1].label, "Откл. микрофон");
    assert_eq!(items[1].item_type, TrayMenuItemType::Checkbox);

    assert_eq!(items[2].action, TrayMenuAction::Deafen);
    assert_eq!(items[2].label, "Откл. звук");
    assert_eq!(items[2].item_type, TrayMenuItemType::Checkbox);

    assert_eq!(items[3].action, TrayMenuAction::VoiceSettings);
    assert_eq!(items[3].label, "Настройки голоса и видео");
    assert_eq!(items[3].item_type, TrayMenuItemType::Normal);

    assert_eq!(items[4].action, TrayMenuAction::CheckForUpdates);
    assert_eq!(items[4].label, "Проверить обновления");
    assert_eq!(items[4].item_type, TrayMenuItemType::Normal);

    assert_eq!(items[5].action, TrayMenuAction::Quit);
    assert_eq!(items[5].label, "Выйти из Discord");
    assert_eq!(items[5].item_type, TrayMenuItemType::Normal);
}

#[test]
fn test_tray_menu_voice_state_updates() {
    let mut menu = TrayMenu::new(false, false, false);
    // Вне звонка чекбоксы голоса неактивны
    assert!(!menu.get(TrayMenuAction::Mute).unwrap().enabled);
    assert!(!menu.get(TrayMenuAction::Deafen).unwrap().enabled);

    // Вход в звонок с mute=true, deaf=false
    menu.update_voice_state(true, true, false);
    let mute = menu.get(TrayMenuAction::Mute).unwrap();
    assert!(mute.enabled);
    assert!(mute.checked);

    let deaf = menu.get(TrayMenuAction::Deafen).unwrap();
    assert!(deaf.enabled);
    assert!(!deaf.checked);

    // Включение deafen
    menu.update_voice_state(true, true, true);
    assert!(menu.get(TrayMenuAction::Deafen).unwrap().checked);
}

#[test]
fn test_format_badge_count_oracle() {
    // 0 -> скрыт
    assert_eq!(format_badge_count(0), None);

    // 1..=99 -> точное число
    assert_eq!(format_badge_count(1), Some("1".to_string()));
    assert_eq!(format_badge_count(42), Some("42".to_string()));
    assert_eq!(format_badge_count(99), Some("99".to_string()));

    // >99 -> 99+
    assert_eq!(format_badge_count(100), Some("99+".to_string()));
    assert_eq!(format_badge_count(999), Some("99+".to_string()));
    assert_eq!(format_badge_count(u32::MAX), Some("99+".to_string()));
}

#[test]
fn test_badge_unity_dbus_payload() {
    let badge_hidden = BadgeState::new(0);
    assert!(!badge_hidden.visible);
    assert_eq!(badge_hidden.count, 0);

    let props_hidden = badge_hidden.unity_properties();
    // count = 0, count-visible = false
    assert_eq!(props_hidden.len(), 2);
    assert_eq!(props_hidden[0].0, Value::String("count".to_string()));
    assert_eq!(props_hidden[0].1, Value::Variant(Box::new(Value::I32(0))));
    assert_eq!(
        props_hidden[1].0,
        Value::String("count-visible".to_string())
    );
    assert_eq!(
        props_hidden[1].1,
        Value::Variant(Box::new(Value::Bool(false)))
    );

    let badge_active = BadgeState::new(25);
    assert!(badge_active.visible);
    assert_eq!(badge_active.count, 25);
    let props_active = badge_active.unity_properties();
    assert_eq!(props_active[0].1, Value::Variant(Box::new(Value::I32(25))));
    assert_eq!(
        props_active[1].1,
        Value::Variant(Box::new(Value::Bool(true)))
    );

    // D-Bus Signal
    let signal = badge_active.to_unity_signal(DEFAULT_DESKTOP_ENTRY);
    assert_eq!(signal.interface, UNITY_LAUNCHER_ENTRY_INTERFACE);
    assert_eq!(signal.member, UNITY_LAUNCHER_ENTRY_SIGNAL);
    assert_eq!(signal.body.len(), 2);
    assert_eq!(
        signal.body[0],
        Value::String(DEFAULT_DESKTOP_ENTRY.to_string())
    );
}

#[test]
fn test_minimize_to_tray_policy() {
    // Включено сворачивание в трей: при обычном закрытии прячем в трей
    assert_eq!(
        should_hide_on_close(true, false),
        CloseDisposition::HideToTray
    );

    // Выключено сворачивание в трей: процесс завершается
    assert_eq!(
        should_hide_on_close(false, false),
        CloseDisposition::QuitProcess
    );

    // Если запрошен Quit — всегда выход из процесса, независимо от настройки трея
    assert_eq!(
        should_hide_on_close(true, true),
        CloseDisposition::QuitProcess
    );
    assert_eq!(
        should_hide_on_close(false, true),
        CloseDisposition::QuitProcess
    );

    let policy = MinimizeToTrayPolicy::new(true);
    assert_eq!(policy.disposition(false), CloseDisposition::HideToTray);
    assert_eq!(policy.disposition(true), CloseDisposition::QuitProcess);
}

#[test]
fn test_tray_controller_lifecycle_and_actions() {
    let mock = MockTrayBackend::default();
    let mut controller = TrayController::new(Some(mock), MinimizeToTrayPolicy::new(true));

    // Исходное состояние: окно не видно, иконка normal, 6 пунктов
    assert!(!controller.is_window_visible());
    assert_eq!(controller.icon_state(), TrayIconState::Normal);
    assert_eq!(controller.menu().len(), 6);

    // 1. Клик по Open -> окно открывается
    let disp = controller.on_menu_action(TrayMenuAction::Open);
    assert_eq!(disp, CloseDisposition::HideToTray);
    assert!(controller.is_window_visible());

    // 2. Клик по трею -> переключает видимость окна (toggle)
    controller.on_tray_clicked();
    assert!(!controller.is_window_visible());
    controller.on_tray_clicked();
    assert!(controller.is_window_visible());

    // 3. Закрытие окна при активной политике сворачивания -> скрывается в трей
    let disp = controller.handle_window_close();
    assert_eq!(disp, CloseDisposition::HideToTray);
    assert!(!controller.is_window_visible());
    assert!(!controller.is_quitting());

    // 4. Клик по Quit -> процесс выходит
    let disp = controller.on_menu_action(TrayMenuAction::Quit);
    assert_eq!(disp, CloseDisposition::QuitProcess);
    assert!(controller.is_quitting());
}

#[test]
fn test_tray_controller_voice_and_unread_events() {
    let mock = MockTrayBackend::default();
    let mut controller = TrayController::new(Some(mock), MinimizeToTrayPolicy::new(true));

    // Непрочитанные сообщения -> иконка unread
    controller.set_unread_count(5).unwrap();
    assert_eq!(controller.icon_state(), TrayIconState::Unread);
    assert_eq!(
        controller.backend().unwrap().current_icon.as_deref(),
        Some("discord-tray-unread")
    );

    // Звонок -> иконка speaking
    controller.update_voice_state(true, false, false).unwrap();
    assert_eq!(controller.icon_state(), TrayIconState::Speaking);
    assert_eq!(
        controller.backend().unwrap().current_icon.as_deref(),
        Some("discord-tray-speaking")
    );

    // Отключение микрофона -> muted
    controller.update_voice_state(true, true, false).unwrap();
    assert_eq!(controller.icon_state(), TrayIconState::Muted);
    assert_eq!(
        controller.backend().unwrap().current_icon.as_deref(),
        Some("discord-tray-muted")
    );

    // Отключение звука -> deafened
    controller.update_voice_state(true, true, true).unwrap();
    assert_eq!(controller.icon_state(), TrayIconState::Deafened);
    assert_eq!(
        controller.backend().unwrap().current_icon.as_deref(),
        Some("discord-tray-deafened")
    );

    // Завершение звонка -> возврат к unread (так как unread_count=5)
    controller.update_voice_state(false, false, false).unwrap();
    assert_eq!(controller.icon_state(), TrayIconState::Unread);
    assert_eq!(
        controller.backend().unwrap().current_icon.as_deref(),
        Some("discord-tray-unread")
    );

    // Очистка непрочитанных -> normal
    controller.set_unread_count(0).unwrap();
    assert_eq!(controller.icon_state(), TrayIconState::Normal);
    assert_eq!(
        controller.backend().unwrap().current_icon.as_deref(),
        Some("discord-tray")
    );
}

#[test]
fn test_tray_controller_backend_unavailable_resilience() {
    // DoD 3: Нет иконки в системе -> приложение работает, пишет причину в лог (не падает)
    let mut controller =
        TrayController::<MockTrayBackend>::new_headless(MinimizeToTrayPolicy::new(true));
    assert!(controller.backend().is_none());

    // Все вызовы должны отрабатывать без паники
    assert!(controller.set_unread_count(10).is_ok());
    assert!(controller.update_voice_state(true, false, false).is_ok());
    assert_eq!(controller.icon_state(), TrayIconState::Speaking);

    let disp = controller.on_menu_action(TrayMenuAction::Open);
    assert_eq!(disp, CloseDisposition::HideToTray);
    assert!(controller.is_window_visible());

    let disp = controller.on_menu_action(TrayMenuAction::Quit);
    assert_eq!(disp, CloseDisposition::QuitProcess);
    assert!(controller.is_quitting());

    // Если бэкенд возвращает ошибку, контроллер возвращает TrayError без паники
    let failing_mock = MockTrayBackend {
        fail_on_set_icon: true,
        ..Default::default()
    };
    let mut controller2 = TrayController::new(Some(failing_mock), MinimizeToTrayPolicy::new(true));
    let res = controller2.update_voice_state(true, false, false);
    assert!(matches!(res, Err(TrayError::OperationFailed(_))));
}
