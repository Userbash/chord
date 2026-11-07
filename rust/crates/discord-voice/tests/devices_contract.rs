//! RW-40 / V-07-VOICE-DEVICES-CONTRACT: детерминированные контрактные тесты модели
//! аудио-устройств, hot-plug с откатом на default, инварианта mute/deafen,
//! режимов активации (VoiceActivity / PushToTalk) и регулировки громкости (0..=100% / 0..=200%).
//!
//! Оракул: `waves/RW-40/DESIGN/VOICE.md` (§2.4, §3.1–§3.5), `waves/RW-40/DESIGN/voice-spec.json`
//! (`sliders`: `OX2Bnr` 0..=100 = 100, `aUJ062` 0..=200 = 100) и `waves/RW-40/QUALITY/G5-VOICE-PLAN.md` (§5.4, §6, §7.4).

use discord_voice::devices::{
    AudioDevice, AudioDeviceKind, DEFAULT_INPUT_VOLUME, DEFAULT_OUTPUT_VOLUME, DeviceEvent,
    DeviceSelectError, MAX_INPUT_VOLUME, MAX_OUTPUT_VOLUME, MIN_VOLUME, VoiceDevicesState,
    VoiceInputMode,
};

fn sample_devices() -> Vec<AudioDevice> {
    vec![
        AudioDevice {
            id: "mic-default".to_owned(),
            label: "Система: Easy Effects Source".to_owned(),
            kind: AudioDeviceKind::Input,
            is_default: true,
        },
        AudioDevice {
            id: "mic-usb".to_owned(),
            label: "USB Studio Mic".to_owned(),
            kind: AudioDeviceKind::Input,
            is_default: false,
        },
        AudioDevice {
            id: "spk-default".to_owned(),
            label: "Система: Easy Effects Sink".to_owned(),
            kind: AudioDeviceKind::Output,
            is_default: true,
        },
        AudioDevice {
            id: "spk-headphones".to_owned(),
            label: "USB Headphones".to_owned(),
            kind: AudioDeviceKind::Output,
            is_default: false,
        },
    ]
}

#[test]
fn t_v07_01_initial_state_and_device_selection_by_kind() {
    let mut state = VoiceDevicesState::new();
    assert_eq!(state.selected_input(), None);
    assert_eq!(state.selected_output(), None);
    assert!(!state.user_mute());
    assert!(!state.self_deaf());
    assert!(!state.effective_mute());
    assert_eq!(state.input_volume(), DEFAULT_INPUT_VOLUME);
    assert_eq!(state.output_volume(), DEFAULT_OUTPUT_VOLUME);

    let events = state.update_device_list(sample_devices());
    assert!(
        events.is_empty(),
        "первичная инициализация со списком default-устройств не эмитит аварийный откат"
    );
    assert_eq!(state.selected_input(), Some("mic-default"));
    assert_eq!(state.selected_output(), Some("spk-default"));
    assert_eq!(state.input_devices().len(), 2);
    assert_eq!(state.output_devices().len(), 2);

    // Явный выбор валидных устройств ввода и вывода.
    state
        .select_input("mic-usb")
        .expect("выбор существующего микрофона должен проходить");
    state
        .select_output("spk-headphones")
        .expect("выбор существующего динамика должен проходить");
    assert_eq!(state.selected_input(), Some("mic-usb"));
    assert_eq!(state.selected_output(), Some("spk-headphones"));
    assert_eq!(
        state.active_input_device().map(|d| d.id.as_str()),
        Some("mic-usb")
    );
    assert_eq!(
        state.active_output_device().map(|d| d.id.as_str()),
        Some("spk-headphones")
    );

    // Попытка выбрать выходное устройство как микрофон и наоборот отвергается.
    assert_eq!(
        state.select_input("spk-headphones"),
        Err(DeviceSelectError::WrongKind {
            id: "spk-headphones".to_owned(),
            expected: AudioDeviceKind::Input,
            actual: AudioDeviceKind::Output,
        })
    );
    assert_eq!(
        state.select_output("mic-usb"),
        Err(DeviceSelectError::WrongKind {
            id: "mic-usb".to_owned(),
            expected: AudioDeviceKind::Output,
            actual: AudioDeviceKind::Input,
        })
    );

    // Попытка выбрать несуществующее или пустое устройство отвергается без изменения выбора.
    assert_eq!(
        state.select_input("nonexistent"),
        Err(DeviceSelectError::NotFound {
            id: "nonexistent".to_owned(),
            kind: AudioDeviceKind::Input,
        })
    );
    assert_eq!(state.select_output(""), Err(DeviceSelectError::EmptyId));
    assert_eq!(state.selected_input(), Some("mic-usb"));
    assert_eq!(state.selected_output(), Some("spk-headphones"));
}

#[test]
fn t_v07_02_hotplug_preserves_present_device_and_falls_back_to_default_on_removal() {
    let mut state = VoiceDevicesState::with_devices(sample_devices());
    state.select_input("mic-usb").unwrap();
    state.select_output("spk-headphones").unwrap();

    // 1. Hot-plug добавляет третье устройство, но выбранные устройства на месте -> выбор НЕ сбрасывается.
    let mut expanded = sample_devices();
    expanded.push(AudioDevice {
        id: "mic-bluetooth".to_owned(),
        label: "BT Headset Mic".to_owned(),
        kind: AudioDeviceKind::Input,
        is_default: false,
    });
    let events = state.update_device_list(expanded);
    assert!(
        events.is_empty(),
        "пока выбранные устройства присутствуют, отката нет"
    );
    assert_eq!(state.selected_input(), Some("mic-usb"));
    assert_eq!(state.selected_output(), Some("spk-headphones"));

    // 2. Отключение выбранного `mic-usb` при наличии не-default микрофона ПЕРВЫМ в списке и системного `mic-default` ВТОРЫМ -> обязан выбрать именно `is_default == true` (`mic-default`), а не просто первый элемент.
    let without_usb_mic = vec![
        AudioDevice {
            id: "mic-bluetooth".to_owned(),
            label: "BT Headset Mic".to_owned(),
            kind: AudioDeviceKind::Input,
            is_default: false,
        },
        AudioDevice {
            id: "mic-default".to_owned(),
            label: "Система: Easy Effects Source".to_owned(),
            kind: AudioDeviceKind::Input,
            is_default: true,
        },
        AudioDevice {
            id: "spk-default".to_owned(),
            label: "Система: Easy Effects Sink".to_owned(),
            kind: AudioDeviceKind::Output,
            is_default: true,
        },
        AudioDevice {
            id: "spk-headphones".to_owned(),
            label: "USB Headphones".to_owned(),
            kind: AudioDeviceKind::Output,
            is_default: false,
        },
    ];
    let events = state.update_device_list(without_usb_mic);
    assert_eq!(
        events,
        vec![DeviceEvent::DeviceFallback {
            kind: AudioDeviceKind::Input,
            previous_id: "mic-usb".to_owned(),
            fallback_id: "mic-default".to_owned(),
        }]
    );
    assert_eq!(state.selected_input(), Some("mic-default"));
    assert_eq!(state.selected_output(), Some("spk-headphones"));

    // 3. Отключение всех микрофонов и выбранных наушников (при оставшемся не-default динамике).
    let only_non_default_speaker = vec![AudioDevice {
        id: "spk-hdmi".to_owned(),
        label: "HDMI Audio Out".to_owned(),
        kind: AudioDeviceKind::Output,
        is_default: false,
    }];
    let events = state.update_device_list(only_non_default_speaker);
    assert_eq!(
        events,
        vec![
            DeviceEvent::NoDeviceAvailable {
                kind: AudioDeviceKind::Input,
                previous_id: Some("mic-default".to_owned()),
            },
            DeviceEvent::DeviceFallback {
                kind: AudioDeviceKind::Output,
                previous_id: "spk-headphones".to_owned(),
                fallback_id: "spk-hdmi".to_owned(),
            },
        ]
    );
    assert_eq!(state.selected_input(), None);
    assert_eq!(state.selected_output(), Some("spk-hdmi"));

    // 4. Полное отключение всех устройств.
    let events = state.update_device_list(vec![]);
    assert_eq!(
        events,
        vec![
            DeviceEvent::NoDeviceAvailable {
                kind: AudioDeviceKind::Input,
                previous_id: None,
            },
            DeviceEvent::NoDeviceAvailable {
                kind: AudioDeviceKind::Output,
                previous_id: Some("spk-hdmi".to_owned()),
            },
        ]
    );
    assert_eq!(state.selected_input(), None);
    assert_eq!(state.selected_output(), None);
}

#[test]
fn t_v07_03_mute_deafen_invariant_forces_effective_mute_and_restores_user_mute() {
    let mut state = VoiceDevicesState::with_devices(sample_devices());
    assert!(!state.user_mute());
    assert!(!state.self_deaf());
    assert!(!state.effective_mute());

    // Сценарий А: user_mute == false -> включаем self_deaf = true -> effective_mute обязан стать true!
    state.set_self_deaf(true);
    assert!(state.self_deaf());
    assert!(!state.user_mute(), "явный user_mute остаётся false");
    assert!(
        state.effective_mute(),
        "при self_deaf = true эффективный self_mute обязан автоматически становиться true"
    );
    assert_eq!(state.gateway_mute_deaf(), (true, true));

    // Выключаем self_deaf -> эффективный self_mute возвращается к user_mute (false).
    state.set_self_deaf(false);
    assert!(!state.self_deaf());
    assert!(!state.user_mute());
    assert!(
        !state.effective_mute(),
        "после снятия self_deaf эффективный mute должен вернуться к user_mute (false)"
    );
    assert_eq!(state.gateway_mute_deaf(), (false, false));

    // Сценарий Б: пользователь явно включил user_mute = true, затем включил и выключил self_deaf.
    state.set_user_mute(true);
    assert!(state.user_mute());
    assert!(state.effective_mute());

    state.set_self_deaf(true);
    assert!(state.self_deaf());
    assert!(state.user_mute());
    assert!(state.effective_mute());

    state.set_self_deaf(false);
    assert!(!state.self_deaf());
    assert!(
        state.user_mute(),
        "явный user_mute == true не должен сбрасываться при выключении self_deaf"
    );
    assert!(
        state.effective_mute(),
        "при выключении self_deaf эффективный self_mute возвращается к явному user_mute (true)"
    );

    // Сценарий В: переключение user_mute во время активного self_deaf запоминается и применяется после снятия deaf.
    state.set_self_deaf(true);
    state.set_user_mute(false);
    assert!(
        state.effective_mute(),
        "пока self_deaf активен, даже set_user_mute(false) не снимает эффективный mute"
    );
    state.set_self_deaf(false);
    assert!(
        !state.effective_mute(),
        "после снятия self_deaf вступает в силу обновлённый user_mute (false)"
    );
}

#[test]
fn t_v07_04_voice_activity_and_ptt_release_delay_and_mute_blocking() {
    let mut state = VoiceDevicesState::with_devices(sample_devices());

    // 1. Режим активации по голосу (VoiceActivity) с порогом -40 дБ.
    state.set_input_mode(VoiceInputMode::VoiceActivity { threshold_db: -40 });
    assert!(
        !state.is_transmitting(1_000, false, -55),
        "ниже порога -40 дБ передача не идёт"
    );
    assert!(
        state.is_transmitting(1_010, false, -40),
        "на границе порога -40 дБ передача активна"
    );
    assert!(
        state.is_transmitting(1_020, false, -20),
        "выше порога передача активна"
    );

    // При effective_mute == true (через user_mute или self_deaf) передача ВСЕГДА false!
    state.set_user_mute(true);
    assert!(
        !state.is_transmitting(1_030, true, 0),
        "при user_mute == true передача обязана быть false при любом уровне сигнала"
    );
    state.set_user_mute(false);
    state.set_self_deaf(true);
    assert!(
        !state.is_transmitting(1_040, true, 0),
        "при self_deaf == true (effective_mute == true) передача обязана быть false"
    );
    state.set_self_deaf(false);

    // 2. Режим PushToTalk с задержкой отпускания 200 мс.
    state.set_input_mode(VoiceInputMode::PushToTalk {
        release_delay_ms: 200,
    });
    // До нажатия клавиши передача не идёт даже при громком звуке в микрофоне.
    assert!(!state.is_transmitting(2_000, false, -10));

    // Нажатие клавиши (key_down = true) в t = 2_100 мс -> передача идёт независимо от vad_db.
    assert!(state.is_transmitting(2_100, true, -90));
    assert!(state.is_transmitting(2_200, true, -90));

    // Клавиша отпущена после t = 2_200 мс: в пределах release_delay_ms (до 2_400 мс включительно) передача продолжается.
    assert!(
        state.is_transmitting(2_250, false, -90),
        "внутри окна release_delay_ms (50 мс <= 200 мс) передача должна удерживаться"
    );
    assert!(
        state.is_transmitting(2_400, false, -90),
        "на точной границе release_delay_ms (200 мс) передача ещё активна"
    );
    assert!(
        !state.is_transmitting(2_401, false, -90),
        "после истечения release_delay_ms (201 мс > 200 мс) передача прекращается"
    );

    // Если во время удержания PTT (или в окне release_delay) включается mute/deafen -> передача немедленно false.
    assert!(state.is_transmitting(3_000, true, -10));
    state.set_self_deaf(true);
    assert!(
        !state.is_transmitting(3_010, true, -10),
        "включение self_deaf обязано мгновенно блокировать PTT-передачу"
    );
    assert!(
        !state.is_transmitting(3_050, false, -10),
        "окно release_delay не должно обходить effective_mute"
    );
    state.set_self_deaf(false);
    // После снятия mute без нового нажатия клавиши старый хвост PTT не возобновляет передачу.
    assert!(!state.is_transmitting(3_100, false, -10));

    // 3. При отсутствии выбранного входного устройства (None) передача невозможна.
    state.update_device_list(vec![]);
    assert_eq!(state.selected_input(), None);
    assert!(!state.is_transmitting(4_000, true, 0));
}

#[test]
fn t_v07_05_volume_clamping_and_privacy_redaction() {
    let mut state = VoiceDevicesState::with_devices(sample_devices());
    assert_eq!(MIN_VOLUME, 0);
    assert_eq!(MAX_INPUT_VOLUME, 100);
    assert_eq!(MAX_OUTPUT_VOLUME, 200);

    // Громкость микрофона клампится в 0..=100%.
    assert_eq!(state.set_input_volume(75), 75);
    assert_eq!(state.input_volume(), 75);
    assert_eq!(state.set_input_volume(100), 100);
    assert_eq!(
        state.set_input_volume(101),
        100,
        "превышение 100% микрофона клампится до 100"
    );
    assert_eq!(state.set_input_volume(250), 100);
    assert_eq!(state.input_volume(), 100);
    assert_eq!(state.set_input_volume(0), 0);
    assert_eq!(state.input_volume(), 0);

    // Громкость вывода клампится в 0..=200% (оракул `aUJ062` в voice-spec.json).
    assert_eq!(state.set_output_volume(150), 150);
    assert_eq!(state.output_volume(), 150);
    assert_eq!(state.set_output_volume(200), 200);
    assert_eq!(
        state.set_output_volume(201),
        200,
        "превышение 200% вывода клампится до 200"
    );
    assert_eq!(state.set_output_volume(u16::MAX), 200);
    assert_eq!(state.output_volume(), 200);
    assert_eq!(state.set_output_volume(0), 0);
    assert_eq!(state.output_volume(), 0);

    // Инвариант приватности (G5-VOICE-PLAN §5.1, §7.4 п.4): Debug не должен светить сырой человекочитаемый label устройства.
    let debug_repr = format!("{state:?}");
    assert!(
        !debug_repr.contains("Easy Effects Source") && !debug_repr.contains("USB Studio Mic"),
        "Debug представление не должно содержать приватные метки устройств ОС: {debug_repr}"
    );
}
