//! Контрактные тесты R4-03: Unread Badges (Linux Unity/Freedesktop D-Bus, Windows Taskbar, macOS Dock).
//!
//! Проверяет:
//! - Числовое отображение 1..=10
//! - Переполнение (> 10) -> индикатор-точка (`OverflowDot`, `"•"`)
//! - 0 -> очистка бейджа (`Cleared`)
//! - Конфигурацию в настройках (`enabled`, `show_numeric`, `max_numeric`)
//! - Платформенные payload'ы: Linux Unity D-Bus signal (`emit_signal`), Windows Taskbar overlay icon (16x16 RGBA), macOS Dock badge
//! - Отказоустойчивость при недоступном бэкенде

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::thread;

use chord_platform::dbus::message::{self, MessageType};
use chord_platform::dbus::{BusAddress, Connection, Value};
use chord_platform::unread_badge::{
    BADGE_NUMERIC_MAX, DEFAULT_APP_URI, DbusUnityBadgeBackend, MacOsDockBadge, MockOsBadgeBackend,
    OVERFLOW_DOT_GLYPH, OsBadgeUpdate, UNITY_LAUNCHER_ENTRY_INTERFACE, UNITY_LAUNCHER_ENTRY_SIGNAL,
    UnreadBadgeController, UnreadBadgeDisplay, UnreadBadgeError, UnreadBadgeSettings,
    WindowsOverlayKind, WindowsTaskbarOverlay,
};

#[test]
fn r4_03_badge_display_numeric_1_to_10_overflow_dot_and_zero_clears() {
    // 0 очищает бейдж
    let cleared = UnreadBadgeDisplay::from_count(0, true);
    assert_eq!(cleared, UnreadBadgeDisplay::Cleared);
    assert!(!cleared.is_visible());
    assert_eq!(cleared.label(), None);

    // 1..=10 отображаются как числа
    for n in 1..=BADGE_NUMERIC_MAX {
        let display = UnreadBadgeDisplay::from_count(n, true);
        assert_eq!(display, UnreadBadgeDisplay::Numeric(n));
        assert!(display.is_visible());
        assert_eq!(display.label(), Some(n.to_string()));
    }

    // > 10 отображается как точка переполнения
    for overflow in [11, 12, 42, 99, 100, u32::MAX] {
        let display = UnreadBadgeDisplay::from_count(overflow, true);
        assert_eq!(display, UnreadBadgeDisplay::OverflowDot);
        assert!(display.is_visible());
        assert_eq!(display.label(), Some(OVERFLOW_DOT_GLYPH.to_string()));
    }
}

#[test]
fn r4_03_badge_settings_disable_and_dot_only_mode() {
    let mock = MockOsBadgeBackend::default();
    let mut ctrl = UnreadBadgeController::new(Some(mock), UnreadBadgeSettings::default());

    // Ставим 5 непрочитанных -> Numeric(5)
    ctrl.set_unread_count(5).expect("set 5");
    assert_eq!(ctrl.display(), UnreadBadgeDisplay::Numeric(5));
    assert_eq!(
        ctrl.backend()
            .unwrap()
            .last_update
            .as_ref()
            .unwrap()
            .macos_dock
            .label,
        "5"
    );

    // Отключаем бейджи в настройках -> бейдж очищается, но unread_count сохраняется
    ctrl.set_enabled(false).expect("disable badge");
    assert_eq!(ctrl.display(), UnreadBadgeDisplay::Cleared);
    let last = ctrl.backend().unwrap().last_update.as_ref().unwrap();
    assert_eq!(last.macos_dock, MacOsDockBadge::cleared());
    assert_eq!(last.windows_taskbar, WindowsTaskbarOverlay::Cleared);
    assert!(!last.linux_unity.count_visible);
    assert!(!last.linux_unity.urgent);

    // При отключённых настройках обновление счётчика до 15 не показывает бейдж
    ctrl.set_unread_count(15).expect("set 15 while disabled");
    assert_eq!(ctrl.display(), UnreadBadgeDisplay::Cleared);

    // Включаем обратно -> сразу восстанавливается OverflowDot для 15
    ctrl.set_enabled(true).expect("enable badge");
    assert_eq!(ctrl.display(), UnreadBadgeDisplay::OverflowDot);

    // Режим без чисел (show_numeric = false): даже 3 показывают точку
    ctrl.set_settings(UnreadBadgeSettings {
        enabled: true,
        show_numeric: false,
        max_numeric: BADGE_NUMERIC_MAX,
    })
    .expect("dot-only mode");
    ctrl.set_unread_count(3).expect("set 3 in dot-only mode");
    assert_eq!(ctrl.display(), UnreadBadgeDisplay::OverflowDot);
    assert_eq!(
        ctrl.backend()
            .unwrap()
            .last_update
            .as_ref()
            .unwrap()
            .macos_dock
            .label,
        OVERFLOW_DOT_GLYPH
    );

    // 0 очищает даже в dot-only режиме
    ctrl.set_unread_count(0).expect("set 0");
    assert_eq!(ctrl.display(), UnreadBadgeDisplay::Cleared);
}

#[test]
fn r4_03_windows_taskbar_and_macos_dock_payloads() {
    let cleared = OsBadgeUpdate::from_display(UnreadBadgeDisplay::Cleared, DEFAULT_APP_URI);
    assert_eq!(cleared.macos_dock.label, "");
    assert_eq!(cleared.windows_taskbar, WindowsTaskbarOverlay::Cleared);

    let numeric = OsBadgeUpdate::from_display(UnreadBadgeDisplay::Numeric(7), DEFAULT_APP_URI);
    assert_eq!(numeric.macos_dock.label, "7");
    match &numeric.windows_taskbar {
        WindowsTaskbarOverlay::Icon(icon) => {
            assert_eq!(icon.kind, WindowsOverlayKind::Numeric(7));
            assert_eq!(icon.width, 16);
            assert_eq!(icon.height, 16);
            assert_eq!(icon.rgba.len(), 16 * 16 * 4);
            assert!(icon.rgba.iter().any(|&b| b != 0));
            assert!(icon.description.contains('7'));
        }
        WindowsTaskbarOverlay::Cleared => panic!("expected overlay icon for 7"),
    }

    let overflow = OsBadgeUpdate::from_display(UnreadBadgeDisplay::OverflowDot, DEFAULT_APP_URI);
    assert_eq!(overflow.macos_dock.label, OVERFLOW_DOT_GLYPH);
    match &overflow.windows_taskbar {
        WindowsTaskbarOverlay::Icon(icon) => {
            assert_eq!(icon.kind, WindowsOverlayKind::OverflowDot);
            assert_eq!(icon.width, 16);
            assert_eq!(icon.height, 16);
            assert_eq!(icon.rgba.len(), 16 * 16 * 4);
            assert!(icon.rgba.iter().any(|&b| b != 0));
        }
        WindowsTaskbarOverlay::Cleared => panic!("expected overlay dot icon for overflow"),
    }
}

#[test]
fn r4_03_linux_unity_dbus_signal_wire_emission() {
    // Создаём временный unix-сокет и имитируем D-Bus брокер (AUTH EXTERNAL + Hello + приём Signal)
    let dir = std::env::temp_dir().join(format!("chord-r4-03-dbus-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let sock_path = dir.join("bus.sock");
    let listener = std::os::unix::net::UnixListener::bind(&sock_path).unwrap();

    let server_handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        // 1. Читаем NUL + AUTH EXTERNAL <hex> + NEGOTIATE_UNIX_FD + BEGIN
        let mut buf = [0u8; 512];
        let mut collected = Vec::new();
        loop {
            let n = stream.read(&mut buf).unwrap();
            assert!(n > 0);
            collected.extend_from_slice(&buf[..n]);
            let text = String::from_utf8_lossy(&collected);
            if text.contains("AUTH EXTERNAL") && !text.contains("NEGOTIATE_UNIX_FD") {
                stream.write_all(b"OK 1234567890abcdef\r\n").unwrap();
            } else if text.contains("NEGOTIATE_UNIX_FD") && !text.contains("BEGIN") {
                stream.write_all(b"AGREE_UNIX_FD\r\n").unwrap();
            } else if text.contains("BEGIN\r\n") {
                break;
            }
        }

        // 2. Читаем Hello MethodCall и отвечаем MethodReturn(":1.42")
        let read_msg = |s: &mut UnixStream| -> message::Message {
            let mut hdr = [0u8; 16];
            s.read_exact(&mut hdr).unwrap();
            let body_len = u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]) as usize;
            let fields_len = u32::from_le_bytes([hdr[12], hdr[13], hdr[14], hdr[15]]) as usize;
            let body_offset = ((16 + fields_len) + 7) & !7;
            let mut full = vec![0u8; body_offset + body_len];
            full[..16].copy_from_slice(&hdr);
            s.read_exact(&mut full[16..]).unwrap();
            message::parse_with_fds(&full, Vec::new()).unwrap()
        };

        let hello = read_msg(&mut stream);
        assert_eq!(hello.kind, MessageType::MethodCall);
        assert_eq!(hello.member(), Some("Hello"));
        let (reply_bytes, _) = message::build_with_fds(
            MessageType::MethodReturn,
            1,
            &[
                (message::FIELD_REPLY_SERIAL, Value::U32(hello.serial)),
                (message::FIELD_SIGNATURE, Value::String("s".to_string())),
            ],
            "s",
            &[Value::String(":1.42".to_string())],
        )
        .unwrap();
        stream.write_all(&reply_bytes).unwrap();

        // 3. Принимаем 3 сигнала: Numeric(4), OverflowDot, Cleared(0)
        let sig1 = read_msg(&mut stream);
        let sig2 = read_msg(&mut stream);
        let sig3 = read_msg(&mut stream);
        (sig1, sig2, sig3)
    });

    let addr = BusAddress::Path(sock_path.clone());
    let conn = Connection::connect(&addr).expect("connect mock dbus");
    let backend = DbusUnityBadgeBackend::new(conn, DEFAULT_APP_URI);
    let mut ctrl = UnreadBadgeController::new(Some(backend), UnreadBadgeSettings::default());

    ctrl.set_unread_count(4).expect("emit numeric 4");
    ctrl.set_unread_count(12).expect("emit overflow dot");
    ctrl.set_unread_count(0).expect("emit clear 0");

    let (sig1, sig2, sig3) = server_handle.join().unwrap();
    for sig in [&sig1, &sig2, &sig3] {
        assert_eq!(sig.kind, MessageType::Signal);
        assert_eq!(sig.interface(), Some(UNITY_LAUNCHER_ENTRY_INTERFACE));
        assert_eq!(sig.member(), Some(UNITY_LAUNCHER_ENTRY_SIGNAL));
        assert_eq!(sig.body[0], Value::String(DEFAULT_APP_URI.to_string()));
    }

    // Проверяем поля a{sv} для каждого сигнала
    let extract_props = |sig: &message::Message| -> (i64, bool, bool) {
        let Value::Dict(entries) = &sig.body[1] else {
            panic!("expected Dict");
        };
        let mut count = -1;
        let mut visible = false;
        let mut urgent = false;
        for (k, v) in entries {
            if let (Value::String(key), Value::Variant(inner)) = (k, v) {
                match (key.as_str(), inner.as_ref()) {
                    ("count", Value::I32(c)) => count = *c as i64,
                    ("count-visible", Value::Bool(b)) => visible = *b,
                    ("urgent", Value::Bool(b)) => urgent = *b,
                    _ => {}
                }
            }
        }
        (count, visible, urgent)
    };

    assert_eq!(extract_props(&sig1), (4, true, false));
    assert_eq!(extract_props(&sig2), (0, false, true));
    assert_eq!(extract_props(&sig3), (0, false, false));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn r4_03_backend_unavailable_and_error_resilience() {
    // Headless без бэкенда не падает
    let mut headless =
        UnreadBadgeController::<MockOsBadgeBackend>::new(None, UnreadBadgeSettings::default());
    headless.set_unread_count(3).expect("headless ok");
    assert_eq!(headless.display(), UnreadBadgeDisplay::Numeric(3));

    // Отказ бэкенда возвращает явную ошибку и не паникует
    let mut failing = MockOsBadgeBackend::default();
    failing.fail_on_apply = true;
    let mut ctrl = UnreadBadgeController::new(Some(failing), UnreadBadgeSettings::default());
    let err = ctrl.set_unread_count(2).unwrap_err();
    assert!(matches!(err, UnreadBadgeError::BackendFailed(_)));
}
