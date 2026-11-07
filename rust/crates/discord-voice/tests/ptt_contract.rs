//! Контрактные тесты аппаратного обработчика Push-to-Talk (PTT), комбинаций скан-кодов,
//! приоритетного голоса (`SpeakingFlags::PRIORITY`) и автомата микрофона (`MicStateMachine`).
//!
//! Оракулы RE (`/home/sanya/re-deep/`):
//! - `voice-deep.md` (§3–4): голосовой тракт, op5 Speaking (`MICROPHONE = 1`, `SOUNDSHARE = 2`, `PRIORITY = 4`).
//! - `native-re.md` (§5.2–5.4, `discord_utils.node` `src/input_events.cpp`, `src/input_events_linux.cpp`):
//!   `InputEventRegister`, `InputEventUnregister`, `InputEventServiceSetAllowed`, `InputSetFocused`,
//!   `OnSystemInputEvent`, `DeviceIdRegex = "^([0-9]+):([0-9]+)$"`, опции `keydown`/`keyup`/`focused`/`blurred`,
//!   лимит комбинации `MAX_KEYBIND_COMBO_LEN = 4`, запрет ЛКМ (код 1) и одиночных клавиш
//!   в скан-кодах `[24..=33]`, `[38..=46]`, `[52..=58]` на Linux.
//! - `native-re/raw/discord_voice/nm-all-demangled.txt` и `rabin2-strings.txt`:
//!   `discord::PttKeyState` (`SetKeyState`, `SetWantsPriority`, `GetWantsPriority`, `GetPressCount`,
//!   `GetActivationState`), `discord::AudioModeType` (`MODE_UNINIT`, `MODE_VAD`, `MODE_PTT`),
//!   `LocalUser::UpdateMuteState`, `LocalUser::UpdateSpeaking`, `SetOnSpeakingWhileMutedCallback`,
//!   `SetNoAudioInputThreshold`.
//! - `waves/RW-40/QUALITY/G5-VOICE-PLAN.md` (§3.3, §6): `org.freedesktop.portal.GlobalShortcuts`
//!   (`Activated`/`Deactivated`) и индикация `PttAccessStatus::Blocked` без фокуса окна при недоступном портале.

use discord_voice::SpeakingFlags;
use discord_voice::ptt::{
    AudioModeType, DEFAULT_NO_AUDIO_INPUT_SILENCE_FRAMES, DEFAULT_NO_AUDIO_INPUT_THRESHOLD_DB,
    DEFAULT_PTT_RELEASE_DELAY_MS, ENGINE_DEFAULT_PTT_RELEASE_DELAY_MS, HardwareKeybindAction,
    HardwarePttKeybindHandler, InputDeviceId, InputDeviceType, KEYBIND_CAPTURE_TIMEOUT_MS,
    KeybindError, KeybindEventOptions, KeybindFiredEvent, LINUX_FORBIDDEN_SINGLE_KEY_RANGES,
    LINUX_LEFT_MOUSE_BUTTON_CODE, MAX_KEYBIND_COMBO_LEN, MAX_PTT_RELEASE_DELAY_MS,
    MIN_PTT_RELEASE_DELAY_MS, MicCaptureState, MicStateMachine, PttAccessStatus,
    PttActivationState, PttKeyState, ScanCodeCombo, ScanCodeEntry,
};

#[test]
fn t_ptt_01_constants_and_audio_mode_type_oracle() {
    assert_eq!(MIN_PTT_RELEASE_DELAY_MS, 0);
    assert_eq!(MAX_PTT_RELEASE_DELAY_MS, 2_000);
    assert_eq!(DEFAULT_PTT_RELEASE_DELAY_MS, 200);
    assert_eq!(ENGINE_DEFAULT_PTT_RELEASE_DELAY_MS, 20);
    assert_eq!(MAX_KEYBIND_COMBO_LEN, 4);
    assert_eq!(KEYBIND_CAPTURE_TIMEOUT_MS, 5_000);
    assert_eq!(LINUX_LEFT_MOUSE_BUTTON_CODE, 1);
    assert_eq!(
        LINUX_FORBIDDEN_SINGLE_KEY_RANGES,
        [(24, 33), (38, 46), (52, 58)]
    );
    assert_eq!(DEFAULT_NO_AUDIO_INPUT_THRESHOLD_DB, -90);
    assert_eq!(DEFAULT_NO_AUDIO_INPUT_SILENCE_FRAMES, 250);

    // Оракул `discord::AudioModeTypeToString` в `rabin2-strings.txt:1634-1637`
    assert_eq!(AudioModeType::ModeUninit.as_str(), "MODE_UNINIT");
    assert_eq!(AudioModeType::ModeVad.as_str(), "MODE_VAD");
    assert_eq!(AudioModeType::ModePtt.as_str(), "MODE_PTT");
    assert_eq!(
        AudioModeType::parse("MODE_UNINIT"),
        Some(AudioModeType::ModeUninit)
    );
    assert_eq!(
        AudioModeType::parse("MODE_VAD"),
        Some(AudioModeType::ModeVad)
    );
    assert_eq!(
        AudioModeType::parse("MODE_PTT"),
        Some(AudioModeType::ModePtt)
    );
    assert_eq!(AudioModeType::parse("UNKNOWN"), None);
    assert_eq!(AudioModeType::parse(""), None);

    // Коды действий клавиш
    assert_eq!(HardwareKeybindAction::PushToTalk.as_str(), "PUSH_TO_TALK");
    assert_eq!(
        HardwareKeybindAction::PushToTalkPriority.as_str(),
        "PUSH_TO_TALK_PRIORITY"
    );
    assert_eq!(HardwareKeybindAction::PushToMute.as_str(), "PUSH_TO_MUTE");
    assert_eq!(HardwareKeybindAction::VadPriority.as_str(), "VAD_PRIORITY");
    assert_eq!(HardwareKeybindAction::ToggleMute.as_str(), "TOGGLE_MUTE");
    assert_eq!(
        HardwareKeybindAction::ToggleDeafen.as_str(),
        "TOGGLE_DEAFEN"
    );
}

#[test]
fn t_ptt_02_ptt_key_state_release_delay_boundaries_and_priority_flags() {
    // 1. Граничное значение release_delay_ms = 0 мс: мгновенное отключение при отпускании клавиши.
    let mut state_zero = PttKeyState::new(0);
    assert_eq!(state_zero.release_delay_ms(), 0);
    assert_eq!(
        state_zero.activation_state(1_000),
        PttActivationState::Inactive
    );
    assert!(!state_zero.activation_state(1_000).is_transmitting());
    assert_eq!(state_zero.press_count(), 0);

    state_zero.set_key_state(true, false, 1_000);
    assert!(state_zero.is_key_down());
    assert!(!state_zero.wants_priority());
    assert_eq!(state_zero.press_count(), 1);
    assert_eq!(
        state_zero.activation_state(1_050),
        PttActivationState::Active
    );
    assert!(state_zero.activation_state(1_050).is_transmitting());
    assert_eq!(
        state_zero.speaking_flags(1_050, true),
        SpeakingFlags::MICROPHONE
    );

    // Повторный вызов set_key_state(true) при уже зажатой клавише не увеличивает pttActiveCount.
    state_zero.set_key_state(true, false, 1_060);
    assert_eq!(state_zero.press_count(), 1);

    // Отпускание при release_delay_ms == 0 мгновенно переводит в Inactive даже в ту же миллисекунду.
    state_zero.set_key_state(false, false, 1_100);
    assert!(!state_zero.is_key_down());
    assert_eq!(
        state_zero.activation_state(1_100),
        PttActivationState::Inactive
    );
    assert_eq!(state_zero.speaking_flags(1_100, true), SpeakingFlags::NONE);

    // 2. Кламп release_delay_ms выше 2000 мс (например 5000 мс или u32::MAX) до 2000 мс.
    let mut state_max = PttKeyState::new(5_000);
    assert_eq!(state_max.release_delay_ms(), MAX_PTT_RELEASE_DELAY_MS);
    assert_eq!(
        state_max.set_release_delay_ms(u32::MAX),
        MAX_PTT_RELEASE_DELAY_MS
    );

    // Нажатие с флагом приоритетного динамика (Priority Speaker).
    state_max.set_key_state(true, true, 2_000);
    assert!(state_max.wants_priority());
    assert_eq!(state_max.press_count(), 1);
    assert_eq!(
        state_max.speaking_flags(2_010, true),
        SpeakingFlags::MICROPHONE.union(SpeakingFlags::PRIORITY),
        "при разрешённом Priority Speaker должен выставляться бит PRIORITY (1 | 4 = 5)"
    );
    assert_eq!(
        state_max.speaking_flags(2_010, false),
        SpeakingFlags::MICROPHONE,
        "без права Priority Speaker бит PRIORITY выставляться не должен"
    );

    // Отпускание в t = 3_000 мс при release_delay_ms = 2_000 мс:
    // В t = 3_000 мс остаток = 2_000 мс; в t = 4_999 мс остаток = 1 мс;
    // на точной границе t = 5_000 мс остаток = 0 мс (ещё удерживается), а в t = 5_001 мс -> Inactive.
    state_max.set_key_state(false, true, 3_000);
    assert_eq!(
        state_max.activation_state(3_500),
        PttActivationState::ReleaseHangover {
            remaining_ms: 1_500
        }
    );
    assert!(state_max.activation_state(3_500).is_transmitting());
    assert_eq!(
        state_max.speaking_flags(3_500, true),
        SpeakingFlags::MICROPHONE.union(SpeakingFlags::PRIORITY),
        "в окне release_delay приоритетный флаг сохраняется"
    );
    assert_eq!(
        state_max.activation_state(5_000),
        PttActivationState::ReleaseHangover { remaining_ms: 0 }
    );
    assert!(state_max.activation_state(5_000).is_transmitting());
    assert_eq!(
        state_max.activation_state(5_001),
        PttActivationState::Inactive
    );
    assert_eq!(state_max.speaking_flags(5_001, true), SpeakingFlags::NONE);

    // 3. Latch-режим (`VoiceConnectionWrapper::SetPTTActive(..., Optional<bool> latch)`).
    state_max.latch(true, false, 6_000);
    assert!(state_max.is_latched());
    assert_eq!(
        state_max.activation_state(10_000),
        PttActivationState::Active
    );
    state_max.latch(false, false, 10_000);
    assert!(!state_max.is_latched());
    assert_eq!(
        state_max.activation_state(11_000),
        PttActivationState::ReleaseHangover {
            remaining_ms: 1_000
        }
    );
    state_max.clear_release_tail();
    assert_eq!(
        state_max.activation_state(11_000),
        PttActivationState::Inactive
    );
}

#[test]
fn t_ptt_03_linux_scancode_validation_and_device_id_oracle() {
    // 1. Валидация DeviceId по `^([0-9]+):([0-9]+)$` (`discord_utils` `rabin2-strings.txt:0`).
    let dev = InputDeviceId::parse("1133:49970").expect("валидный DeviceId обязан разбираться");
    assert_eq!(dev.vendor_or_bus, 1133);
    assert_eq!(dev.product_or_dev, 49970);
    assert_eq!(dev.format_id(), "1133:49970");

    for invalid in ["", "1133", "1133:", ":49970", "11a3:49970", "1:2:3", "-1:5"] {
        assert_eq!(
            InputDeviceId::parse(invalid),
            Err(KeybindError::InvalidDeviceId),
            "невалидный DeviceId {invalid:?} обязан отклоняться"
        );
    }

    // 2. Коды типов устройств ввода (`0 => Keyboard`, `1 => Mouse`, `2 => Gamepad`).
    assert_eq!(
        InputDeviceType::from_code(0),
        Some(InputDeviceType::Keyboard)
    );
    assert_eq!(InputDeviceType::from_code(1), Some(InputDeviceType::Mouse));
    assert_eq!(
        InputDeviceType::from_code(2),
        Some(InputDeviceType::Gamepad)
    );
    assert_eq!(InputDeviceType::from_code(3), None);

    // 3. Пустая комбинация и превышение MAX_KEYBIND_COMBO_LEN = 4 отклоняются.
    assert_eq!(ScanCodeCombo::new(vec![]), Err(KeybindError::EmptyCombo));
    assert_eq!(
        ScanCodeCombo::new(vec![
            ScanCodeEntry::keyboard(37, true),
            ScanCodeEntry::keyboard(50, true),
            ScanCodeEntry::keyboard(64, true),
            ScanCodeEntry::keyboard(133, true),
            ScanCodeEntry::keyboard(65, false),
        ]),
        Err(KeybindError::ComboTooLong {
            len: 5,
            max: MAX_KEYBIND_COMBO_LEN
        })
    );

    // 4. Левая кнопка мыши (Mouse button 1) запрещена (`native-re.md` §5.4).
    assert_eq!(
        ScanCodeCombo::new(vec![ScanCodeEntry::mouse(LINUX_LEFT_MOUSE_BUTTON_CODE)]),
        Err(KeybindError::ForbiddenLeftMouseButton)
    );
    // Боковая кнопка мыши (Mouse button 4 или 5) разрешена как одиночный бинд.
    let mouse4 =
        ScanCodeCombo::new(vec![ScanCodeEntry::mouse(4)]).expect("Mouse4 разрешён для PTT");
    assert_eq!(mouse4.entries().len(), 1);

    // 5. На Linux одиночные буквенные скан-коды без модификатора (`[24..=33]`, `[38..=46]`, `[52..=58]`) запрещены.
    for forbidden_code in [24u16, 25, 33, 38, 42, 46, 52, 55, 58] {
        assert!(ScanCodeCombo::is_linux_forbidden_single_key(forbidden_code));
        assert_eq!(
            ScanCodeCombo::new(vec![ScanCodeEntry::keyboard(forbidden_code, false)]),
            Err(KeybindError::ForbiddenSingleCharacterKey {
                scan_code: forbidden_code
            })
        );
    }
    // Но тот же скан-код (например 24 = 'Q') в паре с модификатором (37 = Left Ctrl) разрешён!
    let ctrl_q = ScanCodeCombo::new(vec![
        ScanCodeEntry::keyboard(37, true),
        ScanCodeEntry::keyboard(24, false),
    ])
    .expect("Ctrl+Q с модификатором разрешён");
    assert_eq!(ctrl_q.entries().len(), 2);

    // Специальные клавиши вне буквенных рядов (например Space = 65, CapsLock = 66, F1 = 67, RightCtrl = 105) разрешены одиночно.
    assert!(!ScanCodeCombo::is_linux_forbidden_single_key(65));
    assert!(ScanCodeCombo::new(vec![ScanCodeEntry::keyboard(65, false)]).is_ok());

    // Дублирующийся скан-код внутри одной комбинации отклоняется.
    assert_eq!(
        ScanCodeCombo::new(vec![
            ScanCodeEntry::keyboard(37, true),
            ScanCodeEntry::keyboard(37, true),
        ]),
        Err(KeybindError::DuplicateScanCode { scan_code: 37 })
    );
}

#[test]
fn t_ptt_04_hardware_keybind_handler_focus_portal_and_combo_matching() {
    let mut handler = HardwarePttKeybindHandler::new();
    assert!(handler.is_service_allowed());
    assert!(handler.is_window_focused());
    assert!(!handler.is_global_shortcuts_available());
    assert_eq!(handler.access_status(), PttAccessStatus::Active);

    // Регистрация Ctrl (37) + Space (65) для PUSH_TO_TALK и Mouse4 (4) для PUSH_TO_TALK_PRIORITY.
    let ctrl_space = ScanCodeCombo::new(vec![
        ScanCodeEntry::keyboard(37, true),
        ScanCodeEntry::keyboard(65, false),
    ])
    .unwrap();
    let mouse4 = ScanCodeCombo::new(vec![ScanCodeEntry::mouse(4)]).unwrap();

    handler
        .register_keybind(
            1,
            HardwareKeybindAction::PushToTalk,
            ctrl_space.clone(),
            KeybindEventOptions::ptt_default(),
            Some("discord-ptt-main"),
        )
        .expect("регистрация бинда #1");

    // Дубликат event_id отклоняется.
    assert_eq!(
        handler.register_keybind(
            1,
            HardwareKeybindAction::PushToTalk,
            ctrl_space,
            KeybindEventOptions::ptt_default(),
            None,
        ),
        Err(KeybindError::DuplicateEventId { event_id: 1 })
    );

    handler
        .register_keybind(
            2,
            HardwareKeybindAction::PushToTalkPriority,
            mouse4,
            KeybindEventOptions::focused_only(),
            Some("discord-ptt-priority"),
        )
        .expect("регистрация бинда #2");
    assert_eq!(handler.registered_event_ids(), vec![1, 2]);

    // 1. В фокусе окна: нажатие только Ctrl (37) ещё не активирует комбо Ctrl+Space.
    assert!(
        handler
            .on_system_input_event(InputDeviceType::Keyboard, 37, true, 1_000)
            .is_empty()
    );
    // Дожатие Space (65) активирует бинд #1 (`is_down = true`).
    let events = handler.on_system_input_event(InputDeviceType::Keyboard, 65, true, 1_010);
    assert_eq!(
        events,
        vec![KeybindFiredEvent {
            event_id: 1,
            action: HardwareKeybindAction::PushToTalk,
            is_down: true,
            timestamp_ms: 1_010,
            via_portal: false,
        }]
    );
    // Повторный автоповтор клавиши Space (keydown при уже активном комбо) не спамит повторными событиями.
    assert!(
        handler
            .on_system_input_event(InputDeviceType::Keyboard, 65, true, 1_020)
            .is_empty()
    );
    // Отпускание любой из клавиш комбо (Ctrl = 37) деактивирует бинд #1 (`is_down = false`).
    let release_events = handler.on_system_input_event(InputDeviceType::Keyboard, 37, false, 1_100);
    assert_eq!(
        release_events,
        vec![KeybindFiredEvent {
            event_id: 1,
            action: HardwareKeybindAction::PushToTalk,
            is_down: false,
            timestamp_ms: 1_100,
            via_portal: false,
        }]
    );
    let _ = handler.on_system_input_event(InputDeviceType::Keyboard, 65, false, 1_105);

    // 2. Потеря фокуса окна (`window_focused = false`) без `GlobalShortcuts`:
    // статус переходит в `PttAccessStatus::Blocked`, системные события блокируются.
    handler.set_window_focused(false);
    assert_eq!(handler.access_status(), PttAccessStatus::Blocked);
    assert!(
        handler
            .on_system_input_event(InputDeviceType::Mouse, 4, true, 2_000)
            .is_empty()
    );

    // 3. Подключение XDG Desktop Portal `GlobalShortcuts` (`global_shortcuts_available = true`):
    // статус возвращается в `PttAccessStatus::Active`, сигналы портала `Activated`/`Deactivated` работают.
    handler.set_global_shortcuts_available(true);
    assert_eq!(handler.access_status(), PttAccessStatus::Active);

    let portal_down = handler.on_portal_shortcut("discord-ptt-main", true, 2_100);
    assert_eq!(
        portal_down,
        vec![KeybindFiredEvent {
            event_id: 1,
            action: HardwareKeybindAction::PushToTalk,
            is_down: true,
            timestamp_ms: 2_100,
            via_portal: true,
        }]
    );
    let portal_up = handler.on_portal_shortcut("discord-ptt-main", false, 2_300);
    assert_eq!(
        portal_up,
        vec![KeybindFiredEvent {
            event_id: 1,
            action: HardwareKeybindAction::PushToTalk,
            is_down: false,
            timestamp_ms: 2_300,
            via_portal: true,
        }]
    );

    // 4. Запрет сервиса `set_service_allowed(false)` (`InputEventServiceSetAllowed(false)`) блокирует всё.
    handler.set_service_allowed(false);
    assert_eq!(handler.access_status(), PttAccessStatus::Blocked);
    assert!(
        handler
            .on_portal_shortcut("discord-ptt-main", true, 3_000)
            .is_empty()
    );

    // 5. Удаление бинда по ID и полная очистка.
    assert_eq!(handler.unregister_keybind(1), Ok(()));
    assert_eq!(
        handler.unregister_keybind(999),
        Err(KeybindError::UnknownEventId { event_id: 999 })
    );
    handler.unregister_all();
    assert!(handler.registered_event_ids().is_empty());
}

#[test]
fn t_ptt_05_mic_state_machine_mute_deafen_speaking_while_muted_and_no_input() {
    let mut mic = MicStateMachine::new(AudioModeType::ModePtt, 200);
    assert_eq!(mic.audio_mode(), AudioModeType::ModePtt);
    assert!(mic.has_input_device());
    assert!(!mic.is_effectively_muted());

    // 1. В режиме PTT без нажатой клавиши передача закрыта (GateClosed), даже если есть голос.
    let out_idle = mic.evaluate_frame(1_000, true, -20);
    assert_eq!(out_idle.state, MicCaptureState::GateClosed);
    assert!(!out_idle.should_transmit);
    assert_eq!(out_idle.speaking_flags, SpeakingFlags::NONE);
    assert!(!out_idle.speaking_changed);
    assert!(!out_idle.speaking_while_muted);

    // 2. Нажатие приоритетного PTT с правом Priority Speaker -> PrioritySpeaking (`MICROPHONE | PRIORITY`).
    mic.set_priority_speaker_permission(true);
    mic.apply_keybind_event(&KeybindFiredEvent {
        event_id: 1,
        action: HardwareKeybindAction::PushToTalkPriority,
        is_down: true,
        timestamp_ms: 1_100,
        via_portal: false,
    });
    let out_active = mic.evaluate_frame(1_120, false, -80);
    assert_eq!(out_active.state, MicCaptureState::PrioritySpeaking);
    assert!(out_active.should_transmit);
    assert_eq!(
        out_active.speaking_flags,
        SpeakingFlags::MICROPHONE.union(SpeakingFlags::PRIORITY)
    );
    assert!(
        out_active.speaking_changed,
        "переход 0 -> 5 обязан сигнализировать speaking_changed"
    );

    // Отпускание клавиши -> PttReleaseHangover в течение 200 мс.
    mic.apply_keybind_event(&KeybindFiredEvent {
        event_id: 1,
        action: HardwareKeybindAction::PushToTalkPriority,
        is_down: false,
        timestamp_ms: 1_200,
        via_portal: false,
    });
    let out_hangover = mic.evaluate_frame(1_350, false, -80);
    assert_eq!(out_hangover.state, MicCaptureState::PttReleaseHangover);
    assert!(out_hangover.should_transmit);
    assert!(!out_hangover.speaking_changed);

    // Истечение 200 мс (t = 1_401 мс) -> GateClosed, speaking_changed = true.
    let out_expired = mic.evaluate_frame(1_401, false, -80);
    assert_eq!(out_expired.state, MicCaptureState::GateClosed);
    assert!(!out_expired.should_transmit);
    assert_eq!(out_expired.speaking_flags, SpeakingFlags::NONE);
    assert!(out_expired.speaking_changed);

    // 3. Переключение в режим VAD и проверка `SpeakingWhileMuted` (`SetOnSpeakingWhileMutedCallback`).
    mic.set_audio_mode(AudioModeType::ModeVad);
    let out_vad_speak = mic.evaluate_frame(2_000, true, -18);
    assert_eq!(out_vad_speak.state, MicCaptureState::Speaking);
    assert!(out_vad_speak.should_transmit);
    assert_eq!(out_vad_speak.speaking_flags, SpeakingFlags::MICROPHONE);

    // Нажатие `ToggleMute` включает self_mute: при попытке говорить (`voice_detected = true`)
    // передача заблокирована (`should_transmit = false`), но срабатывает `speaking_while_muted = true`!
    mic.apply_keybind_event(&KeybindFiredEvent {
        event_id: 2,
        action: HardwareKeybindAction::ToggleMute,
        is_down: true,
        timestamp_ms: 2_020,
        via_portal: false,
    });
    assert!(mic.is_self_mute());
    assert!(mic.is_effectively_muted());

    let out_swm = mic.evaluate_frame(2_040, true, -15);
    assert_eq!(out_swm.state, MicCaptureState::SpeakingWhileMuted);
    assert!(!out_swm.should_transmit);
    assert_eq!(out_swm.speaking_flags, SpeakingFlags::NONE);
    assert!(out_swm.speaking_changed);
    assert!(out_swm.speaking_while_muted);

    // В тишине при включённом mute состояние просто `Muted`, без `speaking_while_muted`.
    let out_muted_silent = mic.evaluate_frame(2_060, false, -85);
    assert_eq!(out_muted_silent.state, MicCaptureState::Muted);
    assert!(!out_muted_silent.speaking_while_muted);

    // Повторное нажатие `ToggleMute` снимает mute; проверим `PushToMute` (удержание временного mute).
    mic.apply_keybind_event(&KeybindFiredEvent {
        event_id: 2,
        action: HardwareKeybindAction::ToggleMute,
        is_down: true,
        timestamp_ms: 2_080,
        via_portal: false,
    });
    assert!(!mic.is_self_mute());

    mic.apply_keybind_event(&KeybindFiredEvent {
        event_id: 3,
        action: HardwareKeybindAction::PushToMute,
        is_down: true,
        timestamp_ms: 2_100,
        via_portal: false,
    });
    assert!(mic.is_effectively_muted());
    assert!(!mic.evaluate_frame(2_120, true, -15).should_transmit);

    mic.apply_keybind_event(&KeybindFiredEvent {
        event_id: 3,
        action: HardwareKeybindAction::PushToMute,
        is_down: false,
        timestamp_ms: 2_140,
        via_portal: false,
    });
    assert!(!mic.is_effectively_muted());

    // 4. Детектор `NoAudioInput` (`SetNoAudioInputThreshold`): после 250 кадров абсолютной тишины (<= -90 dBFS)
    // выставляется предупреждение `no_audio_input_warning = true`.
    let mut warned = false;
    for step in 0..DEFAULT_NO_AUDIO_INPUT_SILENCE_FRAMES {
        let res = mic.evaluate_frame(3_000 + u64::from(step) * 20, false, -95);
        if res.no_audio_input_warning {
            warned = true;
            assert_eq!(res.state, MicCaptureState::NoAudioInput);
        }
    }
    assert!(
        warned,
        "после 250 кадров тишины ниже порога обязано сработать предупреждение NoAudioInput"
    );

    // 5. Отсутствие входного устройства (`has_input_device = false`) блокирует захват (`DeviceUnavailable`).
    mic.set_has_input_device(false);
    let out_no_dev = mic.evaluate_frame(9_000, true, -10);
    assert_eq!(out_no_dev.state, MicCaptureState::DeviceUnavailable);
    assert!(!out_no_dev.should_transmit);
}
