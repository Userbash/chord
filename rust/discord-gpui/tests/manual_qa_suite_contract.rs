#![recursion_limit = "512"]
#![allow(dead_code, unused_imports)]

#[path = "../src/audio/mod.rs"]
mod audio;
#[path = "../src/dispatch/mod.rs"]
mod dispatch;
#[path = "../src/manual_qa/mod.rs"]
mod manual_qa;
#[path = "../src/media/mod.rs"]
mod media;
#[path = "../src/net/mod.rs"]
mod net;
#[path = "../src/state/mod.rs"]
mod state;
#[path = "../src/storage/mod.rs"]
mod storage;
#[path = "../src/theme/mod.rs"]
mod theme;
#[path = "../src/ui/mod.rs"]
mod ui;
#[path = "../src/video/mod.rs"]
mod video;

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use manual_qa::{
    AttachmentPayload, AudioCaptureBackendKind, AuthChecklistValidator, BugBashParticipantRole,
    BugBashSession, BugReport, CaptureBackendKind, ChatChecklistValidator,
    CompositorCaptureSessionReport, DefectPriority, DesktopEnvironmentKind,
    DesktopEnvironmentProfile, DesktopMatrixReport, ExploratoryCharterRunner,
    FractionalScaleFactor, KWinCaptureMethod, LinuxDesktopMatrix, ManualQaDomain, ManualQaError,
    ManualQaInteractiveState, ManualQaSuite, MonkeyClickEvent, NetworkCableSimulation,
    ScreenShareChecklistValidator, SmokeSanitySuiteReport, StressInputAnalyzer,
    VoiceChecklistValidator, X11FallbackVerification,
};
use state::{
    AppState, CallStatus, CaptchaChallenge, Channel, ChannelType, Guild, LoginStep, Message,
    MessageAttachment, MessageReaction, ScreenSource, UserSummary, VoiceSessionState,
};
use ui::screen_share_modal::{ScreenShareFps, ScreenShareResolution, ScreenShareTab};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("Корень репозитория должен существовать")
}

// ═══════════════════════════════════════════════════════════════════════════
// SUB-908-1: Чек-листы приемочного тестирования (Smoke & Sanity Checklists)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_sub_908_1_step_1_1_auth_checklist_password_qr_captcha_40003_token_logout() {
    let mut state = AppState::empty();
    let validator = AuthChecklistValidator::new();

    // 1. Вход по паролю + переход в MFA и успешное получение токена
    let pwd_res =
        validator.verify_password_login(&mut state, "qa@discord.gg", "CorrectHorseBatteryStaple!");
    assert!(
        pwd_res.is_ok(),
        "Вход по валидным учетным данным должен проходить"
    );
    assert_eq!(state.login_form.email_or_phone, "qa@discord.gg");

    // 2. QR-код Remote Auth рукопожатие
    let qr_res = validator.verify_qr_code_flow(&mut state, "discord-remote-auth-7f9a2b8c4d1e");
    assert!(
        qr_res.is_ok(),
        "QR-код с валидным отпечатком должен проходить проверку"
    );
    assert!(
        validator
            .verify_qr_code_flow(&mut state, "invalid-qr")
            .is_err(),
        "Невалидный QR-код должен отклоняться"
    );

    // 3. Обработка капчи и кода ошибки Discord 40003 (Opening direct messages too fast / rate-limited challenge)
    let captcha_res = validator.verify_captcha_and_code_40003(
        &mut state,
        40003,
        "f5561ba9-8f1e-40ca-9b5b-a0b3f719ef34",
        "hcaptcha",
        Some("rqdata-token-xyz"),
    );
    assert!(
        captcha_res.is_ok(),
        "Сценарий капчи 40003 должен корректно переводить состояние в CaptchaChallenge"
    );
    assert!(
        state.active_captcha.is_some(),
        "active_captcha должен быть выставлен"
    );
    assert_eq!(
        state.active_captcha.as_ref().unwrap().sitekey,
        "f5561ba9-8f1e-40ca-9b5b-a0b3f719ef34"
    );

    // 4. Сохранение токена и маскирование секретов
    let token_res =
        validator.verify_token_persistence(&mut state, "mfa.secret_token_value_1234567890");
    assert!(token_res.is_ok());
    let masked = token_res.unwrap();
    assert!(
        !masked.contains("secret_token_value_1234567890"),
        "Токен в отчете чек-листа обязан быть замаскирован: {masked}"
    );
    assert_eq!(
        state.active_token.as_deref(),
        Some("mfa.secret_token_value_1234567890")
    );

    // 5. Выход из аккаунта (Logout) очищает токен и сбрасывает форму
    let logout_res = validator.verify_logout(&mut state);
    assert!(logout_res.is_ok());
    assert!(
        state.active_token.is_none(),
        "После выхода active_token должен быть None"
    );
    assert!(
        state.active_captcha.is_none(),
        "После выхода active_captcha должен быть очищен"
    );
    assert_eq!(state.login_form.step, LoginStep::Credentials);
}

#[test]
fn test_sub_908_1_step_1_2_chat_checklist_text_emoji_attachments_25mb_pin_edit_delete_quote() {
    let mut state = AppState::default();
    let channel_id = "ch-general".to_string();
    state.selected_channel_id = Some(channel_id.clone());
    state.messages.insert(channel_id.clone(), Vec::new());

    let validator = ChatChecklistValidator::new();

    // 1. Отправка текста и граничные значения (2000 символов OK, 2001 — ошибка, пустое — ошибка)
    let exact_2000 = "а".repeat(2000);
    let msg_id = validator
        .verify_send_message(&mut state, &channel_id, &exact_2000, &[])
        .expect("Сообщение ровно в 2000 символов должно отправляться");
    assert_eq!(state.messages[&channel_id].len(), 1);

    let over_2001 = "б".repeat(2001);
    assert_eq!(
        validator.verify_send_message(&mut state, &channel_id, &over_2001, &[]),
        Err(ManualQaError::MessageTooLong {
            char_count: 2001,
            max_chars: 2000,
        })
    );
    assert_eq!(
        validator.verify_send_message(&mut state, &channel_id, "   ", &[]),
        Err(ManualQaError::EmptyMessagePayload)
    );

    // 2. Эмодзи и реакции
    validator
        .verify_emoji_reaction(&mut state, &channel_id, &msg_id, "🦀")
        .expect("Добавление реакции должно работать");
    assert_eq!(state.messages[&channel_id][0].reactions[0].emoji, "🦀");
    assert_eq!(state.messages[&channel_id][0].reactions[0].count, 1);

    // 3. Вложения: до 10 файлов и до 25 МБ (26_214_400 байт) — OK; 25 МБ + 1 байт -> HTTP 413; 11 файлов -> ошибка
    let valid_att = vec![AttachmentPayload {
        filename: "trace.pcap".into(),
        size_bytes: 25 * 1024 * 1024, // ровно 26_214_400 байт
    }];
    let att_msg_id = validator
        .verify_send_message(&mut state, &channel_id, "Дамп трафика", &valid_att)
        .expect("Вложение 25 МБ должно проходить");
    assert!(!att_msg_id.is_empty());

    let oversize_att = vec![AttachmentPayload {
        filename: "huge.bin".into(),
        size_bytes: 25 * 1024 * 1024 + 1,
    }];
    assert_eq!(
        validator.verify_send_message(
            &mut state,
            &channel_id,
            "Слишком большой файл",
            &oversize_att
        ),
        Err(ManualQaError::PayloadTooLarge413 {
            size_bytes: 26_214_401,
            max_bytes: 26_214_400,
        })
    );

    let eleven_files: Vec<AttachmentPayload> = (0..11)
        .map(|i| AttachmentPayload {
            filename: format!("file_{i}.txt"),
            size_bytes: 1024,
        })
        .collect();
    assert_eq!(
        validator.verify_send_message(&mut state, &channel_id, "11 файлов", &eleven_files),
        Err(ManualQaError::TooManyAttachments {
            count: 11,
            max_count: 10,
        })
    );

    // 4. Закрепление (pin), редактирование (edit), цитирование (quote) и удаление (delete)
    validator
        .verify_pin_message(&mut state, &channel_id, &msg_id, true)
        .expect("Закрепление сообщения должно работать");
    assert!(state.messages[&channel_id][0].is_pinned);

    validator
        .verify_edit_message(&mut state, &channel_id, &msg_id, "Обновленный текст")
        .expect("Редактирование сообщения должно работать");
    assert_eq!(state.messages[&channel_id][0].content, "Обновленный текст");
    assert!(state.messages[&channel_id][0].edited_timestamp.is_some());

    let quote_id = validator
        .verify_quote_reply(&mut state, &channel_id, &msg_id, "Согласен с автором")
        .expect("Цитирование сообщения должно работать");
    let quoted_msg = state.messages[&channel_id]
        .iter()
        .find(|m| m.id == quote_id)
        .unwrap();
    assert!(
        quoted_msg.content.starts_with("> Обновленный текст\n"),
        "Цитата должна содержать префикс '> ': {}",
        quoted_msg.content
    );

    validator
        .verify_delete_message(&mut state, &channel_id, &msg_id)
        .expect("Удаление сообщения должно работать");
    assert!(state.messages[&channel_id].iter().all(|m| m.id != msg_id));
}

#[test]
fn test_sub_908_1_step_1_3_voice_checklist_connect_mute_deafen_devices_ptt_volume_bounds() {
    let mut state = AppState::default();
    let validator = VoiceChecklistValidator::new();

    // 1. Подключение к голосовому каналу
    validator
        .verify_voice_connect(&mut state, "vc-rust", "Rust Core Voice", "Discord Lab", 24)
        .expect("Подключение к голосу должно инициализировать VoiceSessionState");
    assert!(state.voice_session.is_some());
    assert_eq!(state.voice_session.as_ref().unwrap().ping_ms, 24);

    // 2. Mute / Deafen (при включении Deafen автоматически включается Mute — инвариант Discord)
    validator
        .verify_mute_deafen_transition(&mut state, false, true)
        .expect("Переход в Deafen должен автоматически включать Mute");
    assert!(state.is_deafened);
    assert!(
        state.is_muted,
        "Deafen обязан принудительно выставлять is_muted = true"
    );

    // 3. Переключение аудиоустройств и аварийный fallback на Default при отключении
    state.available_audio_inputs = vec!["Default".into(), "PipeWire USB Condenser Mic".into()];
    state.available_audio_outputs = vec!["Default".into(), "PipeWire Studio Headphones".into()];
    validator
        .verify_audio_device_switch(
            &mut state,
            "PipeWire USB Condenser Mic",
            "PipeWire Studio Headphones",
        )
        .expect("Переключение на существующие устройства должно проходить");
    assert_eq!(
        state.selected_audio_input.as_deref(),
        Some("PipeWire USB Condenser Mic")
    );

    // Если устройство исчезло — fallback на Default
    validator
        .verify_audio_device_hotplug_fallback(&mut state, "Unplugged USB Mic")
        .expect("При пропаже устройства должен срабатывать fallback на Default");
    assert_eq!(state.selected_audio_input.as_deref(), Some("Default"));

    // 4. Push-to-Talk и индикатор громкости (граничные значения 0..=200%)
    validator
        .verify_push_to_talk(&mut state, true, true)
        .expect("Активация PTT при зажатой клавише должна включать передачу голоса");
    assert!(state.is_ptt_enabled && state.ptt_active);
    assert!(state.voice_session.as_ref().unwrap().is_speaking);

    assert!(validator.verify_volume_bounds(&mut state, 0, 200).is_ok());
    assert_eq!(state.input_volume, 0.0);
    assert_eq!(state.output_volume, 2.0);
    assert_eq!(
        validator.verify_volume_bounds(&mut state, 201, 100),
        Err(ManualQaError::VolumeOutOfBounds {
            requested_percent: 201,
            max_percent: 200,
        })
    );
}

#[test]
fn test_sub_908_1_step_1_4_screen_share_checklist_screens_windows_presets_720p_1080p_hot_switch() {
    let mut state = AppState::default();
    let voice_validator = VoiceChecklistValidator::new();
    voice_validator
        .verify_voice_connect(&mut state, "vc-stream", "Stream Room", "Discord Lab", 18)
        .unwrap();

    state.available_screen_sources = vec![
        ScreenSource {
            id: "screen:0".into(),
            name: "Экран 1 (2560x1440 @ 120Hz)".into(),
            is_screen: true,
            preview_icon: "🖥️".into(),
        },
        ScreenSource {
            id: "{6ad0aefd-103b-4fa1-b650-3e016c818a00}".into(),
            name: "Alacritty — cargo test".into(),
            is_screen: false,
            preview_icon: "🪟".into(),
        },
    ];

    let validator = ScreenShareChecklistValidator::new();

    // 1. Старт стрима всего экрана в 720p @ 30 FPS
    let start_params = validator
        .verify_start_stream(
            &mut state,
            ScreenShareTab::Monitors,
            "screen:0",
            ScreenShareResolution::P720,
            ScreenShareFps::Fps30,
        )
        .expect("Старт стрима экрана 720p должен проходить");
    assert!(state.is_screen_sharing);
    assert_eq!(start_params.width, 1280);
    assert_eq!(start_params.height, 720);
    assert_eq!(start_params.fps, 30);

    // 2. Переключение на окно приложения и 1080p @ 60 FPS прямо в процессе стрима (без обрыва звонка)
    let switch_params = validator
        .verify_in_stream_source_and_preset_switch(
            &mut state,
            ScreenShareTab::Windows,
            "{6ad0aefd-103b-4fa1-b650-3e016c818a00}",
            ScreenShareResolution::P1080,
            ScreenShareFps::Fps60,
        )
        .expect("Горячее переключение источника и пресета 1080p60 должно работать");
    assert!(state.is_screen_sharing);
    assert!(
        state.voice_session.is_some(),
        "Голосовая сессия не должна рваться при смене источника стрима"
    );
    assert_eq!(
        state.selected_screen_source_id.as_deref(),
        Some("{6ad0aefd-103b-4fa1-b650-3e016c818a00}")
    );
    assert_eq!(switch_params.width, 1920);
    assert_eq!(switch_params.height, 1080);
    assert_eq!(switch_params.fps, 60);

    // 3. Остановка стрима
    validator
        .verify_stop_stream(&mut state)
        .expect("Остановка стрима должна работать");
    assert!(!state.is_screen_sharing);
    assert!(state.selected_screen_source_id.is_none());
}

// ═══════════════════════════════════════════════════════════════════════════
// SUB-908-2: Кросс-платформенная матрица тестирования Linux Desktop
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_sub_908_2_linux_desktop_matrix_gnome_kde_hyprland_x11() {
    let matrix = LinuxDesktopMatrix::canonical_four_environments();
    assert_eq!(
        matrix.profiles().len(),
        4,
        "Матрица должна содержать ровно 4 основных окружения Linux Desktop"
    );

    // Шаг 2.1: GNOME 46/47 Wayland (AppIndicator трей + нативные уведомления org.freedesktop.Notifications)
    let gnome = matrix
        .profile(DesktopEnvironmentKind::GnomeWayland)
        .expect("Профиль GNOME 46/47 Wayland должен присутствовать");
    let gnome_audit = matrix
        .verify_environment(gnome)
        .expect("Аудит GNOME Wayland должен проходить");
    assert!(
        gnome_audit.appindicator_tray_ok,
        "В GNOME должен быть проверен AppIndicator / StatusNotifierItem"
    );
    assert!(
        gnome_audit.native_notifications_ok,
        "В GNOME должны работать D-Bus уведомления org.freedesktop.Notifications"
    );
    assert_eq!(
        gnome_audit.capture_backend,
        CaptureBackendKind::XdgDesktopPortalPipeWire
    );

    // Шаг 2.2: KDE Plasma 6 Wayland (PipeWire portal + Fractional Scaling 125% и 150%)
    let kde = matrix
        .profile(DesktopEnvironmentKind::KdePlasma6Wayland)
        .expect("Профиль KDE Plasma 6 Wayland должен присутствовать");
    let kde_audit = matrix
        .verify_environment(kde)
        .expect("Аудит KDE Plasma 6 должен проходить");
    assert!(kde_audit.pipewire_portal_ok);
    assert!(
        kde_audit.fractional_scaling_125_ok,
        "Масштабирование 125% (1.25x) должно быть без размытия и наложений"
    );
    assert!(
        kde_audit.fractional_scaling_150_ok,
        "Масштабирование 150% (1.50x) должно сохранять сетку 72/240/flex-1/240"
    );

    let layout_125 = matrix
        .verify_fractional_scaling_geometry(1280, 720, FractionalScaleFactor::Percent125)
        .expect("Геометрия при 125% на 1280x720 должна быть валидной");
    assert_eq!(layout_125.physical_width, 1600);
    assert_eq!(layout_125.physical_height, 900);
    assert!(!layout_125.has_text_overlap);
    assert!(layout_125.user_panel_anchored_bottom);

    let layout_150 = matrix
        .verify_fractional_scaling_geometry(1280, 720, FractionalScaleFactor::Percent150)
        .expect("Геометрия при 150% на 1280x720 должна быть валидной");
    assert_eq!(layout_150.physical_width, 1920);
    assert_eq!(layout_150.physical_height, 1080);

    // Окно меньше минимального 900x600 должно отклоняться проверкой геометрии
    assert!(
        matrix
            .verify_fractional_scaling_geometry(800, 500, FractionalScaleFactor::Percent100)
            .is_err(),
        "Разрешение ниже минимального 900x600 должно фиксироваться как нарушение контракта верстки"
    );

    // Шаг 2.3: Hyprland / Sway (тилинговые оконные менеджеры, корректность размеров и динамического ресайза)
    let hypr = matrix
        .profile(DesktopEnvironmentKind::HyprlandSwayWayland)
        .expect("Профиль Hyprland / Sway должен присутствовать");
    let hypr_audit = matrix
        .verify_environment(hypr)
        .expect("Аудит Hyprland / Sway должен проходить");
    assert!(
        hypr_audit.tiling_resize_invariants_ok,
        "Тилинг-ресайз в Hyprland/Sway не должен выталкивать UserPanel"
    );

    // Шаг 2.4: Классическая сессия X11 (Xfce / i3) с проверкой fallback захвата экрана X11 SHM
    let x11 = matrix
        .profile(DesktopEnvironmentKind::ClassicX11)
        .expect("Профиль X11 (Xfce / i3) должен присутствовать");
    let x11_audit = matrix
        .verify_environment(x11)
        .expect("Аудит X11 должен проходить");
    assert_eq!(
        x11_audit.capture_backend,
        CaptureBackendKind::X11ShmFallback,
        "В сессии X11 без Wayland портала обязан активироваться fallback X11 MIT-SHM (XShmGetImage)"
    );
    assert!(x11_audit.x11_shm_fallback_ok);
}

// ═══════════════════════════════════════════════════════════════════════════
// SUB-908-3: Сценарии исследовательского тестирования и Bug-bash сессии
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_sub_908_3_step_3_1_stress_input_2000_chars_unicode_rtl_broken_markdown() {
    let analyzer = StressInputAnalyzer::new();

    // Стрессовая комбинация: Unicode + RTL (арабский/иврит + U+202E) + битые незакрытые теги Markdown
    let stress_payload = format!(
        "**незакрытый жирный ```rust\nfn broken() {{ ||спойлер без конца \u{202E}مرحبا بالعالم שלום עולם 🦀👨‍👩‍👧‍👦 {}",
        "z".repeat(1850)
    );
    assert!(stress_payload.chars().count() <= 2000);

    let report = analyzer
        .analyze_and_sanitize(&stress_payload)
        .expect("Стрессовый ввод до 2000 символов должен безопасно обрабатываться без паники");
    assert!(report.contains_rtl_text, "Должен быть распознан RTL-текст");
    assert!(
        report.rtl_isolated_safely,
        "Bidi-изоляция (U+2068/U+2069) должна предотвращать порчу верстки"
    );
    assert!(
        report.has_unclosed_markdown,
        "Должны быть обнаружены незакрытые теги Markdown"
    );
    assert!(
        report.markdown_ast_balanced,
        "Парсер должен безопасно закрывать/экранировать битые теги без паники"
    );
    assert!(report.unicode_graphemes_valid);
}

#[test]
fn test_sub_908_3_step_3_2_monkey_testing_10_clicks_per_second_server_channel_switch() {
    let mut state = AppState::default();
    let runner = ExploratoryCharterRunner::new();

    // Генерируем 30 быстрых кликов со скоростью 10 кликов в секунду (интервал 100 мс)
    let events: Vec<MonkeyClickEvent> = (0..30)
        .map(|i| MonkeyClickEvent {
            timestamp_ms: i * 100, // 10 кликов в секунду
            guild_index: (i as usize) % state.guilds.len().max(1),
            channel_index: (i as usize) % 2,
        })
        .collect();

    let monkey_report = runner
        .run_monkey_switching_charter(&mut state, &events, "Черновик не должен пропасть")
        .expect("Monkey Testing (10 кликов/сек) должен завершаться без гонок и утечек");

    assert_eq!(monkey_report.total_clicks, 30);
    assert_eq!(monkey_report.clicks_per_second, 10.0);
    assert!(
        monkey_report.max_frame_time_ms < 8.33,
        "При быстром переключении бюджет кадра должен оставаться < 8.33 мс (120 FPS)"
    );
    assert!(
        monkey_report.vmrss_mb < 60.0,
        "Память VmRSS должна оставаться < 60 МБ"
    );
    assert!(
        monkey_report.draft_preserved,
        "Черновик сообщения должен сохраняться при быстром переключении"
    );
    assert_eq!(state.draft_text, "Черновик не должен пропасть");
}

#[test]
fn test_sub_908_3_step_3_3_unstable_internet_cable_disconnect_30s_resume_recovery() {
    let mut state = AppState::default();
    let runner = ExploratoryCharterRunner::new();

    let sim = NetworkCableSimulation {
        disconnect_duration: Duration::from_secs(30),
        session_id: "sess_908_exploratory_abc".into(),
        last_seq_before_drop: 1420,
        inflight_draft: "Сообщение во время обрыва кабеля".into(),
        replayed_seq_range: (1421, 1425),
    };

    let net_report = runner
        .run_cable_disconnect_charter(&mut state, &sim)
        .expect(
            "Сценарий 30-секундного отключения кабеля должен восстанавливать сессию через RESUME",
        );

    assert_eq!(net_report.disconnect_secs, 30);
    assert!(
        net_report.used_opcode_6_resume,
        "Должен использоваться Opcode 6 RESUME, а не полный ре-логин"
    );
    assert_eq!(net_report.resumed_seq, 1425);
    assert_eq!(
        net_report.duplicate_messages_count, 0,
        "После RESUME не должно быть дубликатов сообщений"
    );
    assert!(
        net_report.draft_retained,
        "Введенный черновик не должен теряться при обрыве сети"
    );
}

#[test]
fn test_sub_908_3_step_3_4_bug_bash_session_defect_reporting_gdb_masking_and_p0_p3_gate() {
    let mut session = BugBashSession::new(
        "BUG-BASH-RW42-RC1",
        "v0.42.0-rc1",
        vec![
            DesktopEnvironmentKind::GnomeWayland,
            DesktopEnvironmentKind::KdePlasma6Wayland,
            DesktopEnvironmentKind::HyprlandSwayWayland,
            DesktopEnvironmentKind::ClassicX11,
        ],
    );
    assert_eq!(
        session.interactive_state(),
        ManualQaInteractiveState::ChecklistReady
    );

    session.start_session();
    assert_eq!(
        session.interactive_state(),
        ManualQaInteractiveState::RunningBugBash
    );

    // 1. Фиксация минорного дефекта P2 с логом GDB (содержащим токен, который обязан замаскироваться)
    let p2_bug = BugReport::new(
        "BUG-908-01",
        "Микро-сдвиг тултипа на 1px при масштабе 150% в KDE Plasma 6",
        DefectPriority::P2,
        DesktopEnvironmentKind::KdePlasma6Wayland,
        vec![
            "1. Запустить discord-gpui в KDE Plasma 6 Wayland с масштабом 150%".into(),
            "2. Навести курсор на иконку сервера в ServerRail".into(),
            "3. Наблюдать смещение тултипа на 1px вправо".into(),
        ],
        "artifacts/screenshots/kde-150-tooltip.png",
        "Thread 1 (GPUI): #0 render_tooltip() auth_header=\"Bearer mfa.super_secret_token_999\" at src/ui/server_rail.rs:142",
    )
    .expect("Валидный баг-репорт должен создаваться");

    assert!(
        !p2_bug
            .gdb_backtrace_sanitized()
            .contains("super_secret_token_999"),
        "Токен в GDB логе баг-репорта обязан маскироваться: {}",
        p2_bug.gdb_backtrace_sanitized()
    );
    assert!(p2_bug.gdb_backtrace_sanitized().contains("mfa.[REDACTED]"));

    session.record_defect(p2_bug).unwrap();

    // При P0=0, P1=0 и наличии только P2 релизный sign-off разрешён
    let sign_off_ok = session
        .evaluate_sign_off()
        .expect("При P0=0 и P1=0 sign-off должен проходить");
    assert!(sign_off_ok.release_approved);
    assert_eq!(
        session.interactive_state(),
        ManualQaInteractiveState::SignOffComplete
    );

    // 2. Если фиксируется блокер P0 или критический P1 — sign-off блокируется!
    let p0_blocker = BugReport::new(
        "BUG-908-02",
        "Паника при переключении монитора во время стрима в Hyprland",
        DefectPriority::P0,
        DesktopEnvironmentKind::HyprlandSwayWayland,
        vec![
            "1. Начать стрим экрана 1080p60".into(),
            "2. Отключить монитор DP-1".into(),
        ],
        "artifacts/screenshots/hypr-panic.png",
        "Thread 1 panicked at src/video/mod.rs:88",
    )
    .unwrap();
    session.record_defect(p0_blocker).unwrap();

    let blocked = session.evaluate_sign_off().unwrap();
    assert!(
        !blocked.release_approved,
        "При наличии P0/P1 дефектов релизный sign-off обязан блокироваться"
    );
    assert_eq!(blocked.p0_count, 1);
    assert_eq!(blocked.p2_count, 1);

    // 3. Негативная проверка: баг-репорт без шагов воспроизведения или без скриншота отклоняется
    assert_eq!(
        BugReport::new(
            "BUG-INVALID",
            "Пустые шаги",
            DefectPriority::P1,
            DesktopEnvironmentKind::GnomeWayland,
            vec![],
            "shot.png",
            "bt",
        ),
        Err(ManualQaError::EmptyReproductionSteps)
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Оракул спецификации: сверка с docs/specs/MANUAL-QA-SUITE.md
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_manual_qa_suite_spec_document_and_100_percent_coverage_oracle() {
    let spec_path = repo_root().join("docs/specs/MANUAL-QA-SUITE.md");
    assert!(
        spec_path.exists(),
        "Документ регламента docs/specs/MANUAL-QA-SUITE.md должен существовать"
    );
    let content = fs::read_to_string(&spec_path).expect("Чтение docs/specs/MANUAL-QA-SUITE.md");

    // Проверяем наличие всех обязательных разделов из TASK-908
    for required_marker in [
        "TASK-908",
        "SUB-908-1",
        "SUB-908-2",
        "SUB-908-3",
        "40003",
        "25 МБ",
        "Push-to-Talk",
        "720p",
        "1080p",
        "GNOME 46/47",
        "AppIndicator",
        "KDE Plasma 6",
        "125%",
        "150%",
        "Hyprland",
        "Sway",
        "X11",
        "XShmGetImage",
        "2000 символов",
        "RTL",
        "10 кликов в секунду",
        "30 секунд",
        "GDB",
        "Bug-bash",
        "/home/sanya/re-deep/",
    ] {
        assert!(
            content.contains(required_marker),
            "В docs/specs/MANUAL-QA-SUITE.md отсутствует обязательный маркер контракта: «{required_marker}»"
        );
    }

    let suite = ManualQaSuite::new_canonical();
    let coverage = suite
        .verify_full_coverage()
        .expect("Проверка 100% покрытия пользовательских сценариев");
    assert_eq!(coverage.covered_domains.len(), 4);
    assert!(coverage.covered_domains.contains(&ManualQaDomain::Auth));
    assert!(coverage.covered_domains.contains(&ManualQaDomain::Chat));
    assert!(coverage.covered_domains.contains(&ManualQaDomain::Voice));
    assert!(
        coverage
            .covered_domains
            .contains(&ManualQaDomain::ScreenShare)
    );
    assert_eq!(coverage.coverage_percent, 100);
}

#[test]
fn test_nine_mandatory_dimensions_and_boundary_error_recovery_invariants() {
    let mut state = AppState::empty();
    let auth = AuthChecklistValidator::new();
    let voice = VoiceChecklistValidator::new();
    let screen = ScreenShareChecklistValidator::new();
    let stress = StressInputAnalyzer::new();

    // Негативные проверки авторизации
    assert!(
        auth.verify_password_login(&mut state, "invalid-email", "12345678")
            .is_err()
    );
    assert!(
        auth.verify_password_login(&mut state, "qa@discord.gg", "short")
            .is_err()
    );
    assert_eq!(
        auth.verify_captcha_and_code_40003(&mut state, 50001, "sitekey", "hcaptcha", None),
        Err(ManualQaError::UnexpectedCaptchaErrorCode(50001))
    );
    assert_eq!(
        auth.verify_captcha_and_code_40003(&mut state, 40003, "   ", "hcaptcha", None),
        Err(ManualQaError::InvalidCaptchaSitekey)
    );
    assert_eq!(
        auth.verify_token_persistence(&mut state, "   "),
        Err(ManualQaError::EmptyToken)
    );

    // Негативные проверки голоса и стрима без активной голосовой сессии
    assert_eq!(
        voice.verify_mute_deafen_transition(&mut state, true, false),
        Err(ManualQaError::VoiceSessionNotConnected)
    );
    assert_eq!(
        voice.verify_push_to_talk(&mut state, true, true),
        Err(ManualQaError::VoiceSessionNotConnected)
    );
    assert_eq!(
        screen.verify_start_stream(
            &mut state,
            ScreenShareTab::Monitors,
            "screen:0",
            ScreenShareResolution::P720,
            ScreenShareFps::Fps30,
        ),
        Err(ManualQaError::VoiceSessionNotConnected)
    );
    assert_eq!(
        screen.verify_in_stream_source_and_preset_switch(
            &mut state,
            ScreenShareTab::Monitors,
            "screen:0",
            ScreenShareResolution::P1080,
            ScreenShareFps::Fps60,
        ),
        Err(ManualQaError::ScreenShareNotActive)
    );
    assert_eq!(
        screen.verify_stop_stream(&mut state),
        Err(ManualQaError::ScreenShareNotActive)
    );

    // Стрессовый ввод: граничные значения 0 и 2001 символ
    assert_eq!(
        stress.analyze_and_sanitize(""),
        Err(ManualQaError::EmptyMessagePayload)
    );
    assert_eq!(
        stress.analyze_and_sanitize(&"x".repeat(2001)),
        Err(ManualQaError::MessageTooLong {
            char_count: 2001,
            max_chars: 2000,
        })
    );

    // Проверка P1 блокировки релиза и P3 разрешения релиза в Bug-bash сессии
    let mut session_p1 = BugBashSession::new(
        "BUG-BASH-P1-CHECK",
        "v0.42.0-rc2",
        vec![DesktopEnvironmentKind::GnomeWayland],
    );
    session_p1.start_session();
    let p3_defect = BugReport::new(
        "BUG-908-P3",
        "Опечатка в тултипе",
        DefectPriority::P3,
        DesktopEnvironmentKind::GnomeWayland,
        vec!["Открыть настройки".into()],
        "shot-p3.png",
        "bt-ok",
    )
    .unwrap();
    session_p1.record_defect(p3_defect).unwrap();
    assert!(session_p1.evaluate_sign_off().unwrap().release_approved);

    let p1_defect = BugReport::new(
        "BUG-908-P1",
        "Не отправляется вложение 25 МБ",
        DefectPriority::P1,
        DesktopEnvironmentKind::GnomeWayland,
        vec!["Прикрепить файл 25 МБ".into()],
        "shot-p1.png",
        "bt-p1",
    )
    .unwrap();
    session_p1.record_defect(p1_defect).unwrap();
    let sign_off_p1 = session_p1.evaluate_sign_off().unwrap();
    assert!(
        !sign_off_p1.release_approved,
        "Дефект уровня P1 обязан блокировать релизный sign-off"
    );
    assert_eq!(sign_off_p1.p1_count, 1);
    assert_eq!(sign_off_p1.p3_count, 1);
}

// ═══════════════════════════════════════════════════════════════════════════
// TASK-954: Матричное тестирование захвата экрана и звука под 4 композиторами
// Wayland/X11 (GNOME Mutter, KDE Plasma 6 KWin, Hyprland/wlroots, X11 SHM)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_desktop_matrix_sub_954_1_gnome_mutter_portal_dbus_pipewire_fd_nv12_60fps_and_audio() {
    let matrix = LinuxDesktopMatrix::canonical_four_environments();
    let report = matrix
        .verify_gnome_mutter_portal_nv12_60fps()
        .expect("954.1: Тест портала GNOME (Mutter) через D-Bus с NV12 60 FPS обязан проходить");

    assert_eq!(report.environment, DesktopEnvironmentKind::GnomeWayland);
    assert_eq!(
        report.capture_backend,
        CaptureBackendKind::XdgDesktopPortalPipeWire
    );
    assert_eq!(report.portal_implementation, "xdg-desktop-portal-gnome");
    assert!(
        report.dbus_screencast_called,
        "Должен быть выполнен D-Bus вызов org.freedesktop.portal.ScreenCast (CreateSession -> SelectSources -> Start -> OpenPipeWireRemote)"
    );
    assert!(
        report.pipewire_fd_transferred,
        "Файловый дескриптор PipeWire должен корректно передаваться через SCM_RIGHTS"
    );
    assert_eq!(report.negotiated_format, video::VideoPixelFormat::Nv12);
    assert_eq!(report.drm_fourcc, video::DRM_FOURCC_NV12);
    assert_eq!(report.negotiated_width, 1920);
    assert_eq!(report.negotiated_height, 1080);
    assert_eq!(report.negotiated_fps, 60);
    assert_eq!(report.frame_interval_us, 16_666);
    assert_eq!(report.rtp_timestamp_step, 1_500);
    assert!(
        report.dma_buf_zero_copy_verified,
        "В GNOME Wayland должен подтверждаться zero-copy импорт DMA-BUF кадров NV12"
    );
    assert_eq!(
        report.audio_backend,
        AudioCaptureBackendKind::PipeWireMonitorNode
    );
    assert!(
        report.audio_monitor_source.ends_with(".monitor"),
        "Системный звук в GNOME должен захватываться с PipeWire monitor-узла: {}",
        report.audio_monitor_source
    );
    assert_eq!(
        report.voice_speaking_mask, 2,
        "При шаринге экрана со звуком должен выставляться бит SOUNDSHARE (speaking == 2)"
    );
    assert_eq!(report.audio_pcm_frame_samples, 1920); // 48 кГц * 20 мс * 2 канала
    assert_eq!(report.leaked_fds_after_close, 0);
}

#[test]
fn test_desktop_matrix_sub_954_2_kde_plasma6_kwin_portal_window_monitor_picker_and_zero_fd_leaks() {
    let matrix = LinuxDesktopMatrix::canonical_four_environments();
    let report = matrix
        .verify_kde_plasma6_kwin_portal_and_fd_hygiene()
        .expect("954.2: Тест портала KDE Plasma 6 (KWin) и гигиены дескрипторов обязан проходить");

    assert_eq!(
        report.environment,
        DesktopEnvironmentKind::KdePlasma6Wayland
    );
    assert_eq!(report.portal_implementation, "xdg-desktop-portal-kde");
    assert!(
        report.monitor_selection_verified,
        "Должен поддерживаться выбор мониторов (ScreenShareTab::Monitors / CaptureWorkspace)"
    );
    assert!(
        report.window_selection_verified,
        "Должен поддерживаться выбор окон по UUID KWin (ScreenShareTab::Windows / CaptureWindow)"
    );
    assert_eq!(
        matrix.resolve_kwin_capture_method("screen:0"),
        KWinCaptureMethod::CaptureWorkspace
    );
    assert_eq!(
        matrix.resolve_kwin_capture_method("{6ad0aefd-103b-4fa1-b650-3e016c818a00}"),
        KWinCaptureMethod::CaptureWindow {
            window_uuid: "{6ad0aefd-103b-4fa1-b650-3e016c818a00}".into(),
        }
    );
    assert_eq!(
        matrix
            .resolve_kwin_capture_method("windows:{6ad0aefd-103b-4fa1-b650-3e016c818a00}:Firefox"),
        KWinCaptureMethod::CaptureWindow {
            window_uuid: "{6ad0aefd-103b-4fa1-b650-3e016c818a00}".into(),
        }
    );
    assert!(
        report.dma_buf_zero_copy_verified,
        "В KDE KWin Wayland должен работать zero-copy DMA-BUF захват"
    );
    assert_eq!(
        report.leaked_fds_after_close, 0,
        "После закрытия стрима в KDE KWin не должно оставаться ни одного открытого дескриптора PipeWire/DMA-BUF"
    );
}

#[test]
fn test_desktop_matrix_sub_954_3_hyprland_sway_wlroots_portal_wlr_and_tiling_dma_buf() {
    let matrix = LinuxDesktopMatrix::canonical_four_environments();
    let report = matrix
        .verify_hyprland_wlroots_portal_wlr()
        .expect("954.3: Тест захвата экрана через xdg-desktop-portal-wlr (Hyprland / Sway) обязан проходить");

    assert_eq!(
        report.environment,
        DesktopEnvironmentKind::HyprlandSwayWayland
    );
    assert_eq!(
        report.capture_backend,
        CaptureBackendKind::WlrootsScreencopyPortal
    );
    assert_eq!(report.portal_implementation, "xdg-desktop-portal-wlr");
    assert!(report.dbus_screencast_called);
    assert!(report.pipewire_fd_transferred);
    assert!(report.dma_buf_zero_copy_verified);
    assert_eq!(report.negotiated_format, video::VideoPixelFormat::Nv12);
    assert_eq!(report.negotiated_fps, 60);
    assert_eq!(
        report.audio_backend,
        AudioCaptureBackendKind::PipeWireMonitorNode
    );
    assert_eq!(report.leaked_fds_after_close, 0);
}

#[test]
fn test_desktop_matrix_sub_954_4_x11_shm_fallback_when_wayland_absent_and_degradation_chain() {
    let matrix = LinuxDesktopMatrix::canonical_four_environments();
    let x11_ver = matrix.verify_x11_shm_fallback_without_wayland().expect(
        "954.4: Тест автоматического fallback на XShm при отсутствии Wayland обязан проходить",
    );

    assert_eq!(x11_ver.environment, DesktopEnvironmentKind::ClassicX11);
    assert_eq!(x11_ver.capture_backend, CaptureBackendKind::X11ShmFallback);
    assert!(
        !x11_ver.wayland_daemon_present,
        "В тестовом сценарии X11 демон Wayland должен отсутствовать"
    );
    assert_eq!(x11_ver.x11_display, ":0");
    assert_eq!(x11_ver.full_desktop_screen_id, -1); // webrtc::kFullDesktopScreenId = -1
    assert!(
        x11_ver.xshm_capture_succeeded,
        "Захват через X11 MIT-SHM (XShmGetImage) должен отрабатывать без падений и артефактов"
    );
    assert!(
        x11_ver.window_filtering_verified,
        "Фильтрация окон X11 должна отсеивать свёрнутые и override_redirect окна"
    );
    assert!(
        x11_ver.degradation_chain_verified,
        "Цепочка деградации MitShm -> XGetImageFallback -> SyntheticFallback должна работать без паники"
    );
    assert_eq!(
        x11_ver.audio_backend,
        AudioCaptureBackendKind::PulseAudioMonitorFallback
    );
    assert!(
        x11_ver.mic_isolation_verified,
        "В стрим-воркере при отключённом системном звуке должен выставляться __discord_stream_no_mic__"
    );
}

#[test]
fn test_desktop_matrix_full_four_compositors_dma_buf_x11_and_soundshare_audio_matrix() {
    let matrix = LinuxDesktopMatrix::canonical_four_environments();
    let full_report: DesktopMatrixReport = matrix.run_full_capture_and_audio_matrix().expect(
        "Сквозная матрица 4 композиторов (GNOME, KDE, Hyprland, X11) должна проходить на 100%",
    );

    assert!(full_report.all_passed);
    assert_eq!(full_report.audits.len(), 4);
    assert_eq!(full_report.compositor_sessions.len(), 4);

    // Проверяем, что все 4 окружения присутствуют и не имеют утечек FD
    for session in &full_report.compositor_sessions {
        assert_eq!(
            session.leaked_fds_after_close, 0,
            "Утечка FD в окружении {:?}",
            session.environment
        );
        assert_eq!(session.negotiated_fps, 60);
        assert_eq!(session.audio_pcm_frame_samples, 1920);
    }
}

#[test]
fn test_desktop_matrix_negative_boundary_and_error_recovery_nine_dimensions() {
    let matrix = LinuxDesktopMatrix::canonical_four_environments();

    // 1. Негативные сценарии: отказ пользователя в диалоге портала (Response == 1 ConsentDenied)
    let denied_err = matrix.simulate_portal_error_response(1);
    assert_eq!(
        denied_err,
        Err(ManualQaError::PermissionDenied403(
            "org.freedesktop.portal.ScreenCast: user cancelled dialog (Response=1)"
        ))
    );

    // 2. Прерывание диалога портала (Response == 2 ConsentAborted)
    let aborted_err = matrix.simulate_portal_error_response(2);
    assert_eq!(
        aborted_err,
        Err(ManualQaError::DesktopRequirementUnmet(
            "org.freedesktop.portal.ScreenCast: dialog aborted (Response=2)"
        ))
    );

    // 3. Невалидный дисплей X11 при отсутствии Wayland
    assert!(
        matrix
            .verify_x11_display_fallback("invalid_display")
            .is_err()
    );
    assert!(matrix.verify_x11_display_fallback("").is_err());
    assert!(matrix.verify_x11_display_fallback(":0").is_ok());
    assert!(matrix.verify_x11_display_fallback("unix:1.0").is_ok());
}
