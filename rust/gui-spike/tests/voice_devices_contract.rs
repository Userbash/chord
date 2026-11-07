//! RW-40 / Task-124 (`voice_devices.rs`): контрактные тесты BFF-модуля управления
//! аудио/видео-устройствами, громкостью (`0..=100` / `0..=200`), режимами ввода
//! (`VoiceActivity` с `vad_threshold_db` `-100..=0` / `i8` и `PushToTalk` с `shortcut`
//! и задержкой отпускания `0..=2000` мс), инвариантом `effective_mute = self_mute || self_deaf`,
//! проверкой микрофона (`MicTest`) и перечислением устройств (`audio_input`, `audio_output`,
//! `video_input` с откатом на `default-input` / `default-output` / `default-video`).
//!
//! Нормативные оракулы:
//! - `waves/RW-40/DESIGN/VOICE-DEVICES-SETTINGS-SPEC.md` (§0–§6, проверки `T-V07-UI-01`..`T-V07-UI-11`);
//! - `waves/RW-40/DESIGN/voice-spec.json` (`OX2Bnr`, `aUJ062`, `devices.hotplug.autoswitch: false`,
//!   `devices.sensitivity.slider`, ключи локализации `copy`);
//! - `rust/crates/discord-voice/src/devices.rs` и `rust/gui-spike/ui/features/voice-devices-settings.js`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use discord_gui_spike::voice_devices::{
    is_modifier_key, sensitivity_percent_to_threshold_db, threshold_db_to_sensitivity_percent,
    AudioDevice, AudioDeviceKind, DeviceCatalog, DeviceEvent, DeviceSelectError, MicTestState,
    NoticeLevel, PttStatus, VoiceDevicesService, VoiceDevicesState, VoiceInputMode,
    DEFAULT_INPUT_DEVICE_ID, DEFAULT_INPUT_VOLUME, DEFAULT_MIC_TEST_LEVEL_DB,
    DEFAULT_OUTPUT_DEVICE_ID, DEFAULT_OUTPUT_VOLUME, DEFAULT_PTT_RELEASE_DELAY_MS,
    DEFAULT_VAD_THRESHOLD_DB, DEFAULT_VIDEO_DEVICE_ID, MAX_INPUT_VOLUME, MAX_OUTPUT_VOLUME,
    MAX_PTT_RELEASE_DELAY_MS, MAX_VAD_THRESHOLD_DB, MIN_PTT_RELEASE_DELAY_MS, MIN_VAD_THRESHOLD_DB,
    MIN_VOLUME, MODIFIER_KEYS, PTT_BLOCKED_ALERT_TEXT,
};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("корень репозитория")
        .to_path_buf()
}

fn load_voice_spec_json() -> serde_json::Value {
    let raw = std::fs::read_to_string(repo_root().join("waves/RW-40/DESIGN/voice-spec.json"))
        .expect("waves/RW-40/DESIGN/voice-spec.json");
    serde_json::from_str(&raw).expect("валидный JSON voice-spec.json")
}

/// Читает сестринский UI-контроллер `V-07-UI` — независимый оракул диапазонов порога VAD.
fn load_sister_ui_controller() -> String {
    std::fs::read_to_string(
        repo_root().join("rust/gui-spike/ui/features/voice-devices-settings.js"),
    )
    .expect("rust/gui-spike/ui/features/voice-devices-settings.js")
}

/// Достаёт `export const <name> = <int>;` из JS-контроллера, не завязываясь на реализацию Rust.
fn js_const_i64(source: &str, name: &str) -> i64 {
    let prefix = format!("export const {name} = ");
    let line = source
        .lines()
        .find(|line| line.trim_start().starts_with(&prefix))
        .unwrap_or_else(|| panic!("константа {name} в voice-devices-settings.js"));
    let raw = line.trim_start()[prefix.len()..]
        .trim()
        .trim_end_matches(';')
        .trim();
    raw.parse::<i64>()
        .unwrap_or_else(|_| panic!("{name} должна быть целым числом, получено: {raw}"))
}

/// Тестовая матрица устройств, в которой системные `is_default == true` идут НЕ первыми.
fn sample_devices() -> Vec<AudioDevice> {
    vec![
        AudioDevice {
            id: "mic-usb".to_owned(),
            label: "Система: USB Studio Microphone".to_owned(),
            kind: AudioDeviceKind::Input,
            is_default: false,
        },
        AudioDevice {
            id: "mic-builtin".to_owned(),
            label: "Система: Built-in Microphone".to_owned(),
            kind: AudioDeviceKind::Input,
            is_default: true,
        },
        AudioDevice {
            id: "spk-hdmi".to_owned(),
            label: "Система: HDMI Output".to_owned(),
            kind: AudioDeviceKind::Output,
            is_default: false,
        },
        AudioDevice {
            id: "spk-headset".to_owned(),
            label: "Система: Headset Earphone".to_owned(),
            kind: AudioDeviceKind::Output,
            is_default: true,
        },
        AudioDevice {
            id: "cam-virtual".to_owned(),
            label: "Система: OBS Virtual Camera".to_owned(),
            kind: AudioDeviceKind::VideoInput,
            is_default: false,
        },
        AudioDevice {
            id: "cam-usb".to_owned(),
            label: "Система: USB HD Webcam".to_owned(),
            kind: AudioDeviceKind::VideoInput,
            is_default: true,
        },
    ]
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ServiceCall {
    SelectInput(String),
    SelectOutput(String),
    SelectVideo(String),
    SetInputVolume(u16),
    SetOutputVolume(u16),
    SetInputMode(VoiceInputMode),
    SetUserMute(bool),
    SetSelfDeaf(bool),
    SetPttKey(Option<String>),
    StartMicTest,
    StopMicTest,
}

#[derive(Clone, Default)]
struct RecordingService {
    calls: Arc<Mutex<Vec<ServiceCall>>>,
    fail_with: Option<String>,
}

impl RecordingService {
    fn new() -> Self {
        Self::default()
    }

    fn failing(message: &str) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            fail_with: Some(message.to_owned()),
        }
    }

    fn calls(&self) -> Vec<ServiceCall> {
        self.calls.lock().expect("lock service calls").clone()
    }

    fn record(&self, call: ServiceCall) -> Result<(), String> {
        self.calls.lock().expect("lock service calls").push(call);
        if let Some(msg) = &self.fail_with {
            return Err(msg.clone());
        }
        Ok(())
    }
}

impl VoiceDevicesService for RecordingService {
    fn select_input(&self, id: &str) -> Result<(), String> {
        self.record(ServiceCall::SelectInput(id.to_owned()))
    }

    fn select_output(&self, id: &str) -> Result<(), String> {
        self.record(ServiceCall::SelectOutput(id.to_owned()))
    }

    fn select_video(&self, id: &str) -> Result<(), String> {
        self.record(ServiceCall::SelectVideo(id.to_owned()))
    }

    fn set_input_volume(&self, percent: u16) -> Result<(), String> {
        self.record(ServiceCall::SetInputVolume(percent))
    }

    fn set_output_volume(&self, percent: u16) -> Result<(), String> {
        self.record(ServiceCall::SetOutputVolume(percent))
    }

    fn set_input_mode(&self, mode: VoiceInputMode) -> Result<(), String> {
        self.record(ServiceCall::SetInputMode(mode))
    }

    fn set_user_mute(&self, mute: bool) -> Result<(), String> {
        self.record(ServiceCall::SetUserMute(mute))
    }

    fn set_self_deaf(&self, deaf: bool) -> Result<(), String> {
        self.record(ServiceCall::SetSelfDeaf(deaf))
    }

    fn set_ptt_key(&self, key: Option<&str>) -> Result<(), String> {
        self.record(ServiceCall::SetPttKey(key.map(str::to_owned)))
    }

    fn start_mic_test(&self) -> Result<(), String> {
        self.record(ServiceCall::StartMicTest)
    }

    fn stop_mic_test(&self) -> Result<(), String> {
        self.record(ServiceCall::StopMicTest)
    }
}

// ---------------------------------------------------------------------------
// 1. T-V07-UI-01: Начальная инициализация устройств и громкости по оракулу
// ---------------------------------------------------------------------------

#[test]
fn t01_initial_state_and_default_selection_ignores_list_order() {
    let spec = load_voice_spec_json();
    let sliders = spec["devices"]["sliders"]
        .as_array()
        .expect("devices.sliders в voice-spec.json");
    let input_oracle = sliders
        .iter()
        .find(|s| s["key"].as_str() == Some("OX2Bnr"))
        .expect("слайдер OX2Bnr");
    let output_oracle = sliders
        .iter()
        .find(|s| s["key"].as_str() == Some("aUJ062"))
        .expect("слайдер aUJ062");

    assert_eq!(u64::from(MIN_VOLUME), 0);
    assert_eq!(
        u64::from(MAX_INPUT_VOLUME),
        input_oracle["max"].as_u64().unwrap()
    );
    assert_eq!(
        u64::from(DEFAULT_INPUT_VOLUME),
        input_oracle["value"].as_u64().unwrap()
    );
    assert_eq!(
        u64::from(MAX_OUTPUT_VOLUME),
        output_oracle["max"].as_u64().unwrap()
    );
    assert_eq!(
        u64::from(DEFAULT_OUTPUT_VOLUME),
        output_oracle["value"].as_u64().unwrap()
    );

    let empty = VoiceDevicesState::new();
    assert_eq!(empty.selected_input(), None);
    assert_eq!(empty.selected_output(), None);
    assert_eq!(empty.selected_video(), None);
    assert!(!empty.user_mute());
    assert!(!empty.self_mute());
    assert!(!empty.self_deaf());
    assert!(!empty.effective_mute());
    assert_eq!(empty.input_volume(), DEFAULT_INPUT_VOLUME);
    assert_eq!(empty.output_volume(), DEFAULT_OUTPUT_VOLUME);
    assert_eq!(empty.vad_threshold_db(), DEFAULT_VAD_THRESHOLD_DB);
    assert_eq!(empty.ptt_release_delay_ms(), DEFAULT_PTT_RELEASE_DELAY_MS);
    assert_eq!(empty.ptt_key(), None);
    assert_eq!(empty.shortcut(), None);
    assert_eq!(empty.ptt_status(), PttStatus::Ready);
    assert_eq!(empty.mic_test_state(), MicTestState::Idle);
    assert_eq!(empty.notice(), None);

    // Инициализация со списком, где `is_default == true` стоят вторыми:
    let state = VoiceDevicesState::with_devices(sample_devices());
    assert_eq!(
        state.selected_input(),
        Some("mic-builtin"),
        "должен выбираться микрофон с is_default == true, а не первый в списке"
    );
    assert_eq!(
        state.selected_output(),
        Some("spk-headset"),
        "должно выбираться устройство вывода с is_default == true, а не первое в списке"
    );
    assert_eq!(
        state.selected_video(),
        Some("cam-usb"),
        "должна выбираться камера с is_default == true, а не первая в списке"
    );
    assert_eq!(state.input_devices().len(), 2);
    assert_eq!(state.output_devices().len(), 2);
    assert_eq!(state.video_devices().len(), 2);
    assert_eq!(
        state.notice(),
        None,
        "первичная инициализация не должна показывать баннер hot-plug"
    );
}

// ---------------------------------------------------------------------------
// 2. Перечисление устройств (audio_input, audio_output, video_input) с fallback
// ---------------------------------------------------------------------------

#[test]
fn t02_device_catalog_enumeration_and_default_fallbacks() {
    // 1. При пустом списке от ОС каталог обязан подставить default-input, default-output и default-video.
    let fallback_catalog = DeviceCatalog::from_devices_with_fallback(vec![]);
    assert_eq!(fallback_catalog.audio_input.len(), 1);
    assert_eq!(fallback_catalog.audio_input[0].id, DEFAULT_INPUT_DEVICE_ID);
    assert!(fallback_catalog.audio_input[0].is_default);
    assert_eq!(
        fallback_catalog.audio_input[0].kind,
        AudioDeviceKind::AUDIO_INPUT
    );

    assert_eq!(fallback_catalog.audio_output.len(), 1);
    assert_eq!(
        fallback_catalog.audio_output[0].id,
        DEFAULT_OUTPUT_DEVICE_ID
    );
    assert!(fallback_catalog.audio_output[0].is_default);
    assert_eq!(
        fallback_catalog.audio_output[0].kind,
        AudioDeviceKind::AUDIO_OUTPUT
    );

    assert_eq!(fallback_catalog.video_input.len(), 1);
    assert_eq!(fallback_catalog.video_input[0].id, DEFAULT_VIDEO_DEVICE_ID);
    assert!(fallback_catalog.video_input[0].is_default);
    assert_eq!(
        fallback_catalog.video_input[0].kind,
        AudioDeviceKind::VIDEO_INPUT
    );

    let default_state = VoiceDevicesState::with_default_catalog();
    assert_eq!(default_state.selected_input(), Some("default-input"));
    assert_eq!(default_state.selected_output(), Some("default-output"));
    assert_eq!(default_state.selected_video(), Some("default-video"));

    // 2. Если передан только микрофон и мусорные пустые id, микрофон сохраняется,
    //    а для вывода и видео подставляются default-output / default-video.
    let partial = vec![
        AudioDevice {
            id: "   ".to_owned(),
            label: "Пустой ID".to_owned(),
            kind: AudioDeviceKind::Input,
            is_default: true,
        },
        AudioDevice {
            id: "mic-ext".to_owned(),
            label: "External XLR Interface".to_owned(),
            kind: AudioDeviceKind::Input,
            is_default: false,
        },
    ];
    let state = VoiceDevicesState::from_catalog_with_fallback(partial);
    assert_eq!(state.selected_input(), Some("mic-ext"));
    assert_eq!(state.selected_output(), Some("default-output"));
    assert_eq!(state.selected_video(), Some("default-video"));
    assert_eq!(state.input_devices().len(), 1);
    assert_eq!(state.output_devices().len(), 1);
    assert_eq!(state.video_devices().len(), 1);
}

// ---------------------------------------------------------------------------
// 3. T-V07-UI-02 & T-V07-UI-11: Длинные имена в DOM и редактирование в Debug
// ---------------------------------------------------------------------------

#[test]
fn t03_long_device_labels_preserved_in_dom_options_and_redacted_in_debug() {
    let long_label = format!(
        "Система: Easy Effects Source — {}",
        "Very Long Studio Audio Device Name ".repeat(6)
    );
    let device = AudioDevice {
        id: "mic-long".to_owned(),
        label: long_label.clone(),
        kind: AudioDeviceKind::Input,
        is_default: true,
    };
    let state = VoiceDevicesState::with_devices(vec![device.clone()]);

    let input_view = state.input_select_view();
    assert!(!input_view.aria_disabled);
    assert_eq!(input_view.options.len(), 1);
    assert_eq!(input_view.options[0].value, "mic-long");
    assert_eq!(
        input_view.options[0].text, long_label,
        "текст <option> в DOM никогда не обрезается программно (VOICE.md §3.1)"
    );
    assert_eq!(
        input_view.options[0].title, long_label,
        "полное имя устройства обязано дублироваться в атрибуте title"
    );

    // Приватность (G5-VOICE-PLAN.md §5.1, §7.4 п.4): fmt::Debug обязан скрывать label как <redacted>.
    let device_debug = format!("{device:?}");
    let state_debug = format!("{state:?}");
    assert!(
        device_debug.contains("<redacted>"),
        "Debug AudioDevice должен содержать маркер <redacted>: {device_debug}"
    );
    assert!(
        !device_debug.contains("Easy Effects") && !device_debug.contains("Studio Audio"),
        "Debug AudioDevice не должен светить имя устройства ОС: {device_debug}"
    );
    assert!(
        !state_debug.contains("Easy Effects") && !state_debug.contains("Studio Audio"),
        "Debug VoiceDevicesState не должен светить имена устройств ОС: {state_debug}"
    );
}

// ---------------------------------------------------------------------------
// 4. T-V07-UI-03: Явный выбор и защита от EmptyId / NotFound / WrongKind
// ---------------------------------------------------------------------------

#[test]
fn t04_explicit_device_selection_and_wrong_kind_not_found_empty_id_errors() {
    let service = RecordingService::new();
    let mut state = VoiceDevicesState::with_devices(sample_devices());
    state.set_service(Box::new(service.clone()));

    // Валидный выбор каждого типа вызывает соответствующий метод сервиса ровно 1 раз.
    state
        .select_input("mic-usb")
        .expect("выбор валидного микрофона");
    state
        .select_output("spk-hdmi")
        .expect("выбор валидного динамика");
    state
        .select_video("cam-virtual")
        .expect("выбор валидной камеры");

    assert_eq!(state.selected_input(), Some("mic-usb"));
    assert_eq!(state.selected_output(), Some("spk-hdmi"));
    assert_eq!(state.selected_video(), Some("cam-virtual"));
    assert_eq!(
        state.active_input_device().map(|d| d.id.as_str()),
        Some("mic-usb")
    );
    assert_eq!(
        state.active_output_device().map(|d| d.id.as_str()),
        Some("spk-hdmi")
    );
    assert_eq!(
        state.active_video_device().map(|d| d.id.as_str()),
        Some("cam-virtual")
    );
    assert_eq!(state.notice(), None);
    assert_eq!(
        service.calls(),
        vec![
            ServiceCall::SelectInput("mic-usb".to_owned()),
            ServiceCall::SelectOutput("spk-hdmi".to_owned()),
            ServiceCall::SelectVideo("cam-virtual".to_owned()),
        ]
    );

    // Ошибки выбора: чужой kind, несуществующий id, пустой id.
    assert_eq!(
        state.select_input("spk-hdmi"),
        Err(DeviceSelectError::WrongKind {
            id: "spk-hdmi".to_owned(),
            expected: AudioDeviceKind::Input,
            actual: AudioDeviceKind::Output,
        })
    );
    assert_eq!(
        state.select_output("cam-virtual"),
        Err(DeviceSelectError::WrongKind {
            id: "cam-virtual".to_owned(),
            expected: AudioDeviceKind::Output,
            actual: AudioDeviceKind::VideoInput,
        })
    );
    assert_eq!(
        state.select_video("mic-usb"),
        Err(DeviceSelectError::WrongKind {
            id: "mic-usb".to_owned(),
            expected: AudioDeviceKind::VideoInput,
            actual: AudioDeviceKind::Input,
        })
    );
    assert_eq!(
        state.select_input("missing-mic"),
        Err(DeviceSelectError::NotFound {
            id: "missing-mic".to_owned(),
            kind: AudioDeviceKind::Input,
        })
    );
    assert_eq!(state.select_output(""), Err(DeviceSelectError::EmptyId));
    assert_eq!(state.select_input("   "), Err(DeviceSelectError::EmptyId));

    // Выбор не должен измениться, в сервис не должно уйти ни одного лишнего вызова,
    // а в #voice-devices-notice должен появиться alert «Устройство не найдено.».
    assert_eq!(state.selected_input(), Some("mic-usb"));
    assert_eq!(state.selected_output(), Some("spk-hdmi"));
    assert_eq!(state.selected_video(), Some("cam-virtual"));
    assert_eq!(
        service.calls().len(),
        3,
        "отклонённый выбор не должен вызывать сервис"
    );
    let notice = state
        .notice()
        .expect("ошибка выбора должна показать уведомление");
    assert_eq!(notice.level, NoticeLevel::Alert);
    assert_eq!(notice.role(), "alert");
    assert_eq!(notice.aria_live(), "assertive");
    assert_eq!(notice.message, "Устройство не найдено.");
}

// ---------------------------------------------------------------------------
// 5. T-V07-UI-04: Hot-plug при сохранении активного устройства (autoswitch: false)
// ---------------------------------------------------------------------------

#[test]
fn t05_hotplug_preserves_present_active_device_without_autoswitch() {
    let spec = load_voice_spec_json();
    assert_eq!(
        spec["devices"]["hotplug"]["autoswitch"].as_bool(),
        Some(false),
        "в voice-spec.json зафиксировано autoswitch: false"
    );

    let service = RecordingService::new();
    let mut state = VoiceDevicesState::with_devices(sample_devices());
    state.select_input("mic-usb").unwrap();
    state.select_output("spk-hdmi").unwrap();
    state.set_service(Box::new(service.clone()));

    // Подключается новое устройство (даже помеченное как is_default: true), но выбранные
    // mic-usb и spk-hdmi остаются в списке -> переключения не происходит, событий 0, вызовов сервиса 0.
    let mut next = sample_devices();
    next.push(AudioDevice {
        id: "mic-new-dac".to_owned(),
        label: "Система: New USB DAC Mic".to_owned(),
        kind: AudioDeviceKind::Input,
        is_default: true,
    });
    let events = state.update_device_list(next);
    assert!(
        events.is_empty(),
        "при сохранении активного устройства событий нет"
    );
    assert_eq!(state.selected_input(), Some("mic-usb"));
    assert_eq!(state.selected_output(), Some("spk-hdmi"));
    assert!(
        service.calls().is_empty(),
        "hot-plug никогда не вызывает service.select* самостоятельно"
    );
    assert_eq!(state.notice(), None);
    assert_eq!(state.voice_panel_diagnostic_state(), None);
}

// ---------------------------------------------------------------------------
// 6. T-V07-UI-05: Hot-plug с отключением активного устройства (DeviceFallback)
// ---------------------------------------------------------------------------

#[test]
fn t06_hotplug_active_device_removal_falls_back_to_is_default_or_first_without_service_call() {
    let service = RecordingService::new();
    let mut state = VoiceDevicesState::with_devices(sample_devices());
    state.select_input("mic-usb").unwrap();
    state.select_output("spk-hdmi").unwrap();
    state.set_service(Box::new(service.clone()));

    // 1. Отключаем выбранный `mic-usb`. В списке первым идёт не-default `mic-aux`,
    //    вторым — системный `mic-builtin` (is_default: true).
    //    Кандидат отката обязан стать `mic-builtin`, а НЕ `mic-aux`.
    let after_usb_unplug = vec![
        AudioDevice {
            id: "mic-aux".to_owned(),
            label: "Система: Aux Line-In".to_owned(),
            kind: AudioDeviceKind::Input,
            is_default: false,
        },
        AudioDevice {
            id: "mic-builtin".to_owned(),
            label: "Система: Built-in Microphone".to_owned(),
            kind: AudioDeviceKind::Input,
            is_default: true,
        },
        AudioDevice {
            id: "spk-hdmi".to_owned(),
            label: "Система: HDMI Output".to_owned(),
            kind: AudioDeviceKind::Output,
            is_default: false,
        },
    ];
    let events = state.update_device_list(after_usb_unplug);
    assert_eq!(
        events,
        vec![DeviceEvent::DeviceFallback {
            kind: AudioDeviceKind::Input,
            previous_id: "mic-usb".to_owned(),
            fallback_id: "mic-builtin".to_owned(),
        }]
    );
    assert_eq!(state.selected_input(), Some("mic-builtin"));
    assert_eq!(state.selected_output(), Some("spk-hdmi"));
    assert!(
        service.calls().is_empty(),
        "реконсиляция кандидата отката в модели НЕ должна вызывать service.select_input"
    );
    assert_eq!(
        state.voice_panel_diagnostic_state(),
        Some("device-missing"),
        "при исчезновении активного микрофона панель связи отражает device-missing (VOICE.md §3.2)"
    );

    let notice = state.notice().expect("должен появиться баннер отката");
    assert_eq!(notice.level, NoticeLevel::Status);
    assert_eq!(notice.role(), "status");
    assert_eq!(notice.aria_live(), "polite");
    assert_eq!(
        notice.message,
        "Устройство «Система: USB Studio Microphone» отключено. Выбрано «Система: Built-in Microphone»."
    );

    // 2. Отключаем `mic-builtin`, когда остался только не-default `mic-aux`:
    //    откат идёт на первое доступное устройство `mic-aux`.
    let only_aux = vec![
        AudioDevice {
            id: "mic-aux".to_owned(),
            label: "Система: Aux Line-In".to_owned(),
            kind: AudioDeviceKind::Input,
            is_default: false,
        },
        AudioDevice {
            id: "spk-hdmi".to_owned(),
            label: "Система: HDMI Output".to_owned(),
            kind: AudioDeviceKind::Output,
            is_default: false,
        },
    ];
    let events = state.update_device_list(only_aux);
    assert_eq!(
        events,
        vec![DeviceEvent::DeviceFallback {
            kind: AudioDeviceKind::Input,
            previous_id: "mic-builtin".to_owned(),
            fallback_id: "mic-aux".to_owned(),
        }]
    );
    assert_eq!(state.selected_input(), Some("mic-aux"));
    assert!(service.calls().is_empty());
}

// ---------------------------------------------------------------------------
// 7. T-V07-UI-06: Полное отключение устройств направления (NoDeviceAvailable)
// ---------------------------------------------------------------------------

#[test]
fn t07_hotplug_all_devices_removed_emits_no_device_available_and_disables_select() {
    let service = RecordingService::new();
    let mut state = VoiceDevicesState::with_devices(sample_devices());
    state.set_service(Box::new(service.clone()));

    let events = state.update_device_list(vec![]);
    assert_eq!(
        events,
        vec![
            DeviceEvent::NoDeviceAvailable {
                kind: AudioDeviceKind::Input,
                previous_id: Some("mic-builtin".to_owned()),
            },
            DeviceEvent::NoDeviceAvailable {
                kind: AudioDeviceKind::Output,
                previous_id: Some("spk-headset".to_owned()),
            },
        ]
    );
    assert_eq!(state.selected_input(), None);
    assert_eq!(state.selected_output(), None);
    assert_eq!(state.selected_video(), None);
    assert!(service.calls().is_empty());

    let input_view = state.input_select_view();
    assert!(input_view.aria_disabled);
    assert_eq!(input_view.value, "");
    assert_eq!(input_view.options.len(), 1);
    assert_eq!(input_view.options[0].text, "Микрофон не выбран");

    let output_view = state.output_select_view();
    assert!(output_view.aria_disabled);
    assert_eq!(output_view.value, "");
    assert_eq!(output_view.options.len(), 1);
    assert_eq!(output_view.options[0].text, "Устройство вывода не выбрано");

    let notice = state
        .notice()
        .expect("при отсутствии устройств должен быть показан alert");
    assert_eq!(notice.level, NoticeLevel::Alert);
    assert_eq!(notice.role(), "alert");
    assert_eq!(notice.aria_live(), "assertive");
    assert!(
        notice.message == "Нет доступных микрофонов."
            || notice.message == "Нет доступных устройств вывода."
    );
}

// ---------------------------------------------------------------------------
// 8. T-V07-UI-07: Клампинг громкости (0..=100 микрофон, 0..=200 вывод)
// ---------------------------------------------------------------------------

#[test]
fn t08_volume_clamping_0_to_100_input_and_0_to_200_output_and_channel_independence() {
    let service = RecordingService::new();
    let mut state = VoiceDevicesState::with_devices(sample_devices());
    state.set_service(Box::new(service.clone()));

    // 1. Громкость микрофона по оракулу OX2Bnr (0..=100%):
    assert_eq!(state.set_input_volume(75), 75);
    assert_eq!(state.input_volume(), 75);
    assert_eq!(state.input_volume_text(), "75%");
    assert_eq!(state.set_input_volume(250), 100);
    assert_eq!(state.input_volume(), 100);
    assert_eq!(state.input_volume_text(), "100%");
    assert_eq!(state.set_input_volume_signed(-15), 0);
    assert_eq!(state.input_volume(), 0);
    assert_eq!(state.input_volume_text(), "0%");

    // Микрофон никогда не выходит за `OX2Bnr` (0..=100%) — буста ввода в оракуле нет:
    assert_eq!(state.set_input_volume(MAX_INPUT_VOLUME + 100), 100);
    assert_eq!(state.input_volume(), 100);
    assert_eq!(state.set_input_volume_signed(200), 100);
    assert_eq!(state.input_volume(), 100);

    // 2. Громкость вывода по оракулу aUJ062 (0..=200%):
    assert_eq!(state.set_output_volume(150), 150);
    assert_eq!(state.output_volume(), 150);
    assert_eq!(state.output_volume_text(), "150%");
    assert_eq!(
        state.input_volume(),
        100,
        "изменение громкости вывода не должно затрагивать громкость ввода"
    );

    assert_eq!(state.set_output_volume(250), 200);
    assert_eq!(state.output_volume(), 200);
    assert_eq!(state.output_volume_text(), "200%");

    assert_eq!(state.set_output_volume_signed(-10), 0);
    assert_eq!(state.output_volume(), 0);
    assert_eq!(state.output_volume_text(), "0%");
}

// ---------------------------------------------------------------------------
// 9. T-V07-UI-08a: Полный диапазон порога VAD (`i8`) и шкала чувствительности 0..=100%
// ---------------------------------------------------------------------------

#[test]
fn t09_vad_threshold_full_i8_range_matches_spec_and_sister_ui_controller() {
    // Оракул диапазона — сестринский контроллер `V-07-UI` (синхронизирован со спекой §1.1/§1.2);
    // значения берутся из JS, а не из констант проверяемой реализации.
    let js = load_sister_ui_controller();
    assert_eq!(
        i64::from(MIN_VAD_THRESHOLD_DB),
        js_const_i64(&js, "MIN_VAD_THRESHOLD_DB")
    );
    assert_eq!(
        i64::from(MAX_VAD_THRESHOLD_DB),
        js_const_i64(&js, "MAX_VAD_THRESHOLD_DB")
    );
    assert_eq!(
        i64::from(DEFAULT_VAD_THRESHOLD_DB),
        js_const_i64(&js, "DEFAULT_VAD_THRESHOLD_DB")
    );
    // Нормативная таблица спеки §1.1: `-128` / `127` дБ (полный диапазон `i8`), дефолт `-40` дБ.
    assert_eq!((MIN_VAD_THRESHOLD_DB, MAX_VAD_THRESHOLD_DB), (-128, 127));
    assert_eq!(DEFAULT_VAD_THRESHOLD_DB, -40);

    // Линейная формула связи со шкалой чувствительности 0..=100% (спека §1.1, §3.4):
    // рабочее окно `-100..=0` дБ, вне окна шкала клампится в `0` / `100`.
    assert_eq!(sensitivity_percent_to_threshold_db(0.0), -100);
    assert_eq!(
        sensitivity_percent_to_threshold_db(26.109_350_171_958_468),
        -74,
        "значение дампа 26.109% обязано отображаться в -74 дБ"
    );
    assert_eq!(
        sensitivity_percent_to_threshold_db(60.0),
        DEFAULT_VAD_THRESHOLD_DB
    );
    assert_eq!(sensitivity_percent_to_threshold_db(100.0), 0);

    assert_eq!(threshold_db_to_sensitivity_percent(-100), 0);
    assert_eq!(threshold_db_to_sensitivity_percent(-74), 26);
    assert_eq!(threshold_db_to_sensitivity_percent(-40), 60);
    assert_eq!(threshold_db_to_sensitivity_percent(0), 100);
    assert_eq!(threshold_db_to_sensitivity_percent(-128), 0);
    assert_eq!(threshold_db_to_sensitivity_percent(127), 100);
    assert_eq!(threshold_db_to_sensitivity_percent(-120), 0);
    assert_eq!(threshold_db_to_sensitivity_percent(45), 100);

    let service = RecordingService::new();
    let mut state = VoiceDevicesState::with_devices(sample_devices());
    state.set_service(Box::new(service.clone()));

    // -120 дБ валиден по слайдеру `-128..=127` и НЕ должен молча превращаться в -100.
    assert_eq!(state.set_vad_threshold_db(-120), -120);
    assert_eq!(state.vad_threshold_db(), -120);
    assert_eq!(state.vad_threshold_text(), "-120 дБ");
    assert_eq!(
        state.input_mode(),
        VoiceInputMode::VoiceActivity { threshold_db: -120 }
    );
    assert_eq!(
        service.calls(),
        vec![ServiceCall::SetInputMode(VoiceInputMode::VoiceActivity {
            threshold_db: -120
        })]
    );

    assert_eq!(state.set_vad_threshold_db(-65), -65);
    assert_eq!(state.vad_threshold_db(), -65);
    assert_eq!(state.vad_threshold_text(), "-65 дБ");
    assert_eq!(
        state.input_mode(),
        VoiceInputMode::VoiceActivity { threshold_db: -65 }
    );

    // Клампинг полного диапазона `-128..=127` дБ:
    assert_eq!(state.set_vad_threshold_db(-999), -128);
    assert_eq!(state.vad_threshold_db(), -128);
    assert_eq!(state.set_vad_threshold_db(999), 127);
    assert_eq!(state.vad_threshold_db(), 127);

    // Прямой инженерный контрол — тот же диапазон `i8`:
    assert_eq!(state.set_raw_vad_threshold_db(-999), -128);
    assert_eq!(state.vad_threshold_db(), -128);
    assert_eq!(state.set_raw_vad_threshold_db(999), 127);
    assert_eq!(state.vad_threshold_db(), 127);

    // Авто-чувствительность (lY6j47) блокирует ручной слайдер (aria-disabled="true"),
    // но сохраняет настроенный порог в памяти.
    state.set_vad_threshold_db(-48);
    assert!(!state.is_manual_vad_disabled());
    state.set_auto_sensitivity(true);
    assert!(state.auto_sensitivity());
    assert!(state.is_manual_vad_disabled());
    assert_eq!(state.vad_threshold_db(), -48);
    state.set_auto_sensitivity(false);
    assert!(!state.is_manual_vad_disabled());
    assert_eq!(state.vad_threshold_db(), -48);
}

// ---------------------------------------------------------------------------
// 10. T-V07-UI-08b: Переключение VoiceActivity <-> PushToTalk и задержка 0..=2000 мс
// ---------------------------------------------------------------------------

#[test]
fn t10_mode_switch_voice_activity_vs_push_to_talk_and_release_delay_0_to_2000_ms() {
    assert_eq!(MIN_PTT_RELEASE_DELAY_MS, 0);
    assert_eq!(MAX_PTT_RELEASE_DELAY_MS, 2000);
    assert_eq!(DEFAULT_PTT_RELEASE_DELAY_MS, 200);

    let service = RecordingService::new();
    let mut state = VoiceDevicesState::with_devices(sample_devices());
    state.set_service(Box::new(service.clone()));

    // По умолчанию активен VoiceActivity (-40 дБ): группа VAD видима, группа PTT скрыта.
    assert_eq!(
        state.input_mode(),
        VoiceInputMode::VoiceActivity {
            threshold_db: DEFAULT_VAD_THRESHOLD_DB
        }
    );
    assert!(!state.vad_group_hidden());
    assert!(state.ptt_group_hidden());
    assert!(!state.ptt_switch_checked());
    assert_eq!(state.vad_group_role(), "group");
    assert_eq!(state.ptt_group_role(), "group");

    // Меняем порог VAD на -52 дБ, затем включаем PTT:
    state.set_vad_threshold_db(-52);
    state.set_ptt_enabled(true);
    assert!(state.ptt_switch_checked());
    assert!(state.vad_group_hidden());
    assert!(!state.ptt_group_hidden());
    assert_eq!(
        state.input_mode(),
        VoiceInputMode::PushToTalk {
            release_delay_ms: DEFAULT_PTT_RELEASE_DELAY_MS
        }
    );
    assert_eq!(state.ptt_release_delay_text(), "200 мс");

    // Клампинг задержки отпускания PTT в 0..=2000 мс:
    assert_eq!(state.set_ptt_release_delay_ms(5000), 2000);
    assert_eq!(state.ptt_release_delay_ms(), 2000);
    assert_eq!(state.ptt_release_delay_text(), "2000 мс");
    assert_eq!(
        state.input_mode(),
        VoiceInputMode::PushToTalk {
            release_delay_ms: 2000
        }
    );

    assert_eq!(state.set_ptt_release_delay_ms(-30), 0);
    assert_eq!(state.ptt_release_delay_ms(), 0);
    assert_eq!(state.ptt_release_delay_text(), "0 мс");

    assert_eq!(state.set_ptt_release_delay_ms(350), 350);
    assert_eq!(state.ptt_release_delay_ms(), 350);

    // Выключаем PTT -> возвращается VoiceActivity с сохранённым порогом -52 дБ!
    state.set_ptt_enabled(false);
    assert!(!state.ptt_switch_checked());
    assert!(!state.vad_group_hidden());
    assert!(state.ptt_group_hidden());
    assert_eq!(state.vad_threshold_db(), -52);
    assert_eq!(
        state.input_mode(),
        VoiceInputMode::VoiceActivity { threshold_db: -52 }
    );
    assert_eq!(
        state.ptt_release_delay_ms(),
        350,
        "задержка PTT также сохраняется при возврате в VAD"
    );
}

// ---------------------------------------------------------------------------
// 11. T-V07-UI-09a: Захват клавиши PTT (shortcut), модификаторы, Escape, Пробел
// ---------------------------------------------------------------------------

#[test]
fn t11_ptt_shortcut_capture_ignores_modifiers_cancels_on_escape_and_formats_space() {
    for modifier in MODIFIER_KEYS {
        assert!(
            is_modifier_key(modifier),
            "{modifier} должен распознаваться как модификатор"
        );
    }
    assert!(!is_modifier_key(" "));
    assert!(!is_modifier_key("v"));
    assert!(!is_modifier_key("F4"));

    let service = RecordingService::new();
    let mut state = VoiceDevicesState::with_devices(sample_devices());
    state.set_service(Box::new(service.clone()));

    assert_eq!(state.ptt_key(), None);
    assert_eq!(state.shortcut(), None);
    assert!(!state.ptt_capturing());
    assert_eq!(
        state.format_ptt_key_button_text(),
        "Горячие клавиши не назначены"
    );

    // 1. Вход в захват и отмена по Escape:
    state.begin_ptt_key_capture();
    assert!(state.ptt_capturing());
    assert_eq!(state.format_ptt_key_button_text(), "Нажмите клавишу…");

    let handled = state.handle_ptt_capture_keydown("Escape");
    assert!(handled, "Escape во время захвата перехватывается кнопкой");
    assert!(!state.ptt_capturing());
    assert_eq!(state.ptt_key(), None);
    assert_eq!(
        state.format_ptt_key_button_text(),
        "Горячие клавиши не назначены"
    );
    assert!(
        service.calls().is_empty(),
        "отмена захвата по Escape не должна вызывать service.set_ptt_key"
    );

    // 2. Игнорирование одиночных модификаторов и назначение Пробела:
    state.begin_ptt_key_capture();
    for modifier in MODIFIER_KEYS {
        assert!(state.handle_ptt_capture_keydown(modifier));
        assert!(
            state.ptt_capturing(),
            "нажатие {modifier} не должно завершать захват"
        );
    }
    assert!(service.calls().is_empty());

    assert!(state.handle_ptt_capture_keydown(" "));
    assert!(!state.ptt_capturing());
    assert_eq!(state.ptt_key(), Some(" "));
    assert_eq!(state.shortcut(), Some(" "));
    assert_eq!(state.format_ptt_key_button_text(), "Клавиша: Пробел");
    assert_eq!(
        service.calls(),
        vec![ServiceCall::SetPttKey(Some(" ".to_owned()))]
    );

    // 3. Повторный захват и отмена сохраняют ранее назначенную клавишу:
    state.begin_ptt_key_capture();
    state.cancel_ptt_key_capture();
    assert_eq!(state.ptt_key(), Some(" "));
    assert_eq!(state.format_ptt_key_button_text(), "Клавиша: Пробел");

    // 4. Явная установка shortcut и сброс:
    state.set_shortcut(Some("F4"));
    assert_eq!(state.shortcut(), Some("F4"));
    assert_eq!(state.format_ptt_key_button_text(), "Клавиша: F4");

    state.clear_ptt_key();
    assert_eq!(state.ptt_key(), None);
    assert_eq!(
        state.format_ptt_key_button_text(),
        "Горячие клавиши не назначены"
    );
}

// ---------------------------------------------------------------------------
// 12. T-V07-UI-09b: Состояние PTT `blocked` (vxfv7v) при потере фокуса / без портала
// ---------------------------------------------------------------------------

#[test]
fn t12_ptt_blocked_state_on_focus_loss_or_missing_global_shortcuts_portal() {
    let mut state = VoiceDevicesState::with_devices(sample_devices());
    state.set_ptt_enabled(true);
    state.set_shortcut(Some("v"));
    state.set_global_shortcuts_available(false);
    state.set_window_focused(true);

    assert_eq!(state.ptt_status(), PttStatus::Ready);
    assert_eq!(state.ptt_status().as_str(), "ready");
    assert_eq!(state.voice_panel_ptt_attr(), "idle");
    assert!(state.is_transmitting(1_000, true, -80));

    // Начинаем захват клавиши и теряем фокус окна при недоступном портале GlobalShortcuts:
    state.begin_ptt_key_capture();
    assert!(state.ptt_capturing());

    state.set_window_focused(false);
    assert!(
        !state.ptt_capturing(),
        "потеря фокуса обязана немедленно отменить активный захват клавиши"
    );
    assert_eq!(
        state.ptt_key(),
        Some("v"),
        "прежняя привязка клавиши сохраняется"
    );
    assert_eq!(state.ptt_status(), PttStatus::Blocked);
    assert_eq!(state.ptt_status().as_str(), "blocked");
    assert_eq!(state.voice_panel_ptt_attr(), "blocked");

    let notice = state
        .notice()
        .expect("при блокировке PTT обязан выводиться alert vxfv7v");
    assert_eq!(notice.level, NoticeLevel::Alert);
    assert_eq!(notice.role(), "alert");
    assert_eq!(notice.aria_live(), "assertive");
    assert_eq!(notice.message, PTT_BLOCKED_ALERT_TEXT);

    // В состоянии blocked нажатия вне фокуса НЕ активируют передачу:
    assert!(
        !state.is_transmitting(1_050, true, 0),
        "в состоянии blocked передача голоса вне фокуса запрещена"
    );

    // Возврат фокуса окна снимает блокировку и скрывает предупреждение:
    state.set_window_focused(true);
    assert_eq!(state.ptt_status(), PttStatus::Ready);
    assert_eq!(state.voice_panel_ptt_attr(), "idle");
    assert_eq!(state.notice(), None);
    assert!(state.is_transmitting(1_100, true, -80));
}

// ---------------------------------------------------------------------------
// 13. T-V07-UI-10: Инвариант effective_mute = self_mute || self_deaf и передача
// ---------------------------------------------------------------------------

#[test]
fn t13_mute_deafen_invariant_effective_mute_and_transmission_gating() {
    let mut state = VoiceDevicesState::with_devices(sample_devices());
    assert!(!state.user_mute());
    assert!(!state.self_mute());
    assert!(!state.self_deaf());
    assert!(!state.effective_mute());
    assert_eq!(state.effective_mute_text(), "Микрофон активен");
    assert_eq!(state.user_mute_aria_checked(), "false");
    assert_eq!(state.self_deaf_aria_checked(), "false");

    // 1. Включение self_deaf при выключенном user_mute:
    state.set_self_deaf(true);
    assert!(state.self_deaf());
    assert!(
        !state.user_mute() && !state.self_mute(),
        "self_deaf не должен перезаписывать явный флаг user_mute/self_mute"
    );
    assert_eq!(
        state.user_mute_aria_checked(),
        "false",
        "aria-checked тумблера микрофона остаётся false (VOICE-DEVICES-SETTINGS-SPEC.md §3.8 п.2)"
    );
    assert!(state.effective_mute());
    assert_eq!(state.effective_mute_text(), "Микрофон отключён");
    assert_eq!(state.gateway_mute_deaf(), (true, true));

    // Снятие self_deaf возвращает effective_mute к user_mute (false):
    state.set_self_deaf(false);
    assert!(!state.self_deaf());
    assert!(!state.user_mute());
    assert!(!state.effective_mute());
    assert_eq!(state.effective_mute_text(), "Микрофон активен");
    assert_eq!(state.gateway_mute_deaf(), (false, false));

    // 2. Явно выставленный self_mute (user_mute = true) НЕ сбрасывается при снятии self_deaf:
    state.set_self_mute(true);
    assert!(state.self_mute());
    assert!(state.user_mute());
    assert!(state.effective_mute());
    assert_eq!(state.user_mute_aria_checked(), "true");

    state.set_self_deaf(true);
    assert!(state.self_deaf());
    assert!(state.self_mute());
    assert!(state.effective_mute());

    state.set_self_deaf(false);
    assert!(!state.self_deaf());
    assert!(
        state.self_mute(),
        "снятие self_deaf не должно сбрасывать явно выставленный user self_mute"
    );
    assert!(
        state.effective_mute(),
        "после снятия self_deaf эффективный mute остаётся true из-за self_mute"
    );
    assert_eq!(state.gateway_mute_deaf(), (true, false));

    state.set_self_mute(false);
    assert!(!state.effective_mute());

    // 3. Проверка гейтинга передачи в режимах VAD и PTT (с окном release_delay_ms):
    state.set_vad_threshold_db(-40);
    assert!(!state.is_transmitting(1_000, false, -50));
    assert!(state.is_transmitting(1_010, false, -40));
    assert!(state.is_transmitting(1_020, false, -10));

    state.set_input_mode(VoiceInputMode::PushToTalk {
        release_delay_ms: 200,
    });
    assert!(!state.is_transmitting(2_000, false, 0));
    assert!(state.is_transmitting(2_100, true, -90));
    assert!(
        state.is_transmitting(2_300, false, -90),
        "на границе 200 мс после отпускания клавиши передача ещё активна"
    );
    assert!(
        !state.is_transmitting(2_301, false, -90),
        "через 201 мс (> 200 мс) передача прекращается"
    );

    // Включение mute/deafen мгновенно обрывает удержание PTT и сбрасывает хвост:
    assert!(state.is_transmitting(3_000, true, 0));
    state.set_self_deaf(true);
    assert!(!state.is_transmitting(3_010, true, 0));
    state.set_self_deaf(false);
    assert!(
        !state.is_transmitting(3_020, false, 0),
        "после снятия deaf старый хвост PTT не должен возобновлять передачу без нового нажатия"
    );
}

// ---------------------------------------------------------------------------
// 14. MicTest (VOICE-DEVICES-SETTINGS-SPEC.md §3.7) и перехват ошибок сервиса (§1.3)
// ---------------------------------------------------------------------------

#[test]
fn t14_mic_test_consent_meter_states_and_service_error_alert_recovery() {
    let mut state = VoiceDevicesState::with_devices(sample_devices());
    state.set_vad_threshold_db(-40);

    // При монтировании захват не стартует (idle, -100 дБ, 0%):
    assert_eq!(state.mic_test_state(), MicTestState::Idle);
    assert_eq!(state.mic_test_state().as_str(), "idle");
    assert!(!state.mic_test_active());
    // Нормативная таблица §1.1: тишина индикатора — `-100` дБ (литерал спеки, не константа модуля).
    assert_eq!(DEFAULT_MIC_TEST_LEVEL_DB, -100);
    assert_eq!(state.mic_test_level_db(), -100);
    let meter = state.mic_test_meter_view();
    assert_eq!(meter.fill_percent, 0);
    assert_eq!(meter.threshold_percent, 60);
    assert_eq!(meter.button_text, "Проверка микрофона");
    assert!(!meter.aria_pressed);

    // Явный запуск пользователем:
    assert!(state.start_mic_test());
    assert!(state.mic_test_active());
    assert_eq!(state.mic_test_state(), MicTestState::Testing);

    // Сигнал -25 дБ >= порога -40 дБ -> заполнение 75%, цвет positive:
    state.feed_mic_test_sample(-25);
    let meter = state.mic_test_meter_view();
    assert_eq!(meter.level_db, -25);
    assert_eq!(meter.fill_percent, 75);
    assert_eq!(meter.tone, "positive");
    assert_eq!(meter.aria_valuetext, "-25 дБ");
    assert_eq!(meter.button_text, "Остановить проверку");
    assert!(meter.aria_pressed);

    // Слабый сигнал -60 дБ < порога -40 дБ -> заполнение 40%, цвет warning:
    state.feed_mic_test_sample(-60);
    let meter = state.mic_test_meter_view();
    assert_eq!(meter.level_db, -60);
    assert_eq!(meter.fill_percent, 40);
    assert_eq!(meter.tone, "warning");

    // Отсутствие сигнала (-100 дБ) -> состояние no-signal с диагностической подсказкой:
    state.feed_mic_test_sample(-100);
    assert_eq!(state.mic_test_state(), MicTestState::NoSignal);
    let meter = state.mic_test_meter_view();
    assert_eq!(meter.fill_percent, 0);
    assert!(meter
        .status_text
        .contains("Discord не может обнаружить звук с вашего микрофона."));

    // При включении mute проверка переходит в состояние muted (aria-disabled="true"):
    state.set_self_mute(true);
    assert_eq!(state.mic_test_state(), MicTestState::Muted);
    let meter = state.mic_test_meter_view();
    assert!(meter.aria_disabled);
    assert_eq!(meter.fill_percent, 0);
    assert_eq!(meter.status_text, "Микрофон отключён");
    state.set_self_mute(false);
    state.stop_mic_test();
    assert_eq!(state.mic_test_state(), MicTestState::Idle);

    // Ошибка сервиса (VOICE-DEVICES-SETTINGS-SPEC.md §1.3) не сбрасывает применённое состояние
    // и выводит `Не удалось сохранить настройку: <описание ошибки>` с role="alert":
    let failing = RecordingService::failing("IPC channel closed");
    state.set_service(Box::new(failing));
    assert_eq!(state.set_output_volume(140), 140);
    assert_eq!(state.output_volume(), 140);
    let notice = state
        .notice()
        .expect("ошибка сервиса должна отображаться в #voice-devices-notice");
    assert_eq!(notice.level, NoticeLevel::Alert);
    assert_eq!(notice.role(), "alert");
    assert_eq!(notice.aria_live(), "assertive");
    assert_eq!(
        notice.message,
        "Не удалось сохранить настройку: IPC channel closed"
    );
}

// ---------------------------------------------------------------------------
// 15. P1-1: `fmt::Debug` не утекает метками ОС через текст hot-plug уведомления
// ---------------------------------------------------------------------------

#[test]
fn t15_debug_after_device_fallback_does_not_leak_os_device_labels() {
    let mut state = VoiceDevicesState::with_devices(sample_devices());
    state.select_input("mic-usb").unwrap();
    assert_eq!(state.selected_input(), Some("mic-usb"));

    // Отключаем выбранный микрофон: кандидат отката — `mic-builtin` (`is_default == true`).
    let events = state.update_device_list(vec![
        AudioDevice {
            id: "mic-builtin".to_owned(),
            label: "Система: Built-in Microphone".to_owned(),
            kind: AudioDeviceKind::Input,
            is_default: true,
        },
        AudioDevice {
            id: "spk-headset".to_owned(),
            label: "Система: Headset Earphone".to_owned(),
            kind: AudioDeviceKind::Output,
            is_default: true,
        },
    ]);
    assert_eq!(
        events,
        vec![DeviceEvent::DeviceFallback {
            kind: AudioDeviceKind::Input,
            previous_id: "mic-usb".to_owned(),
            fallback_id: "mic-builtin".to_owned(),
        }]
    );

    // Текст уведомления — это DOM-слой `#voice-devices-notice`, метки там разрешены (спека §0.1 п.3).
    let notice = state.notice().expect("баннер отката обязан быть");
    assert!(
        notice.message.contains("USB Studio Microphone")
            && notice.message.contains("Built-in Microphone"),
        "текст баннера в DOM сохраняет системные метки: {}",
        notice.message
    );

    // Диагностика (`fmt::Debug`) — не DOM: метки ОС туда попадать не должны ни через `devices`,
    // ни через `notice`. Ожидается маркер `<redacted>`.
    let debug = format!("{state:?}");
    assert!(
        !debug.contains("USB Studio Microphone"),
        "Debug VoiceDevicesState не должен светить метку отключённого устройства: {debug}"
    );
    assert!(
        !debug.contains("Built-in Microphone"),
        "Debug VoiceDevicesState не должен светить метку устройства отката: {debug}"
    );
    assert!(
        debug.contains("<redacted>"),
        "Debug VoiceDevicesState обязан помечать скрытые метки маркером <redacted>: {debug}"
    );
}
