//! RW-40 / Task-124: BFF-модуль управления аудио/видео-устройствами, громкостью,
//! режимами ввода (`VoiceActivity` / `PushToTalk`), состояниями `self_mute` / `self_deaf`
//! и диагностикой микрофона (`MicTest`).
//!
//! Источники контракта:
//! - `waves/RW-40/DESIGN/VOICE-DEVICES-SETTINGS-SPEC.md` (§0–§6);
//! - `waves/RW-40/DESIGN/voice-spec.json` (`OX2Bnr`, `aUJ062`, `devices.hotplug.autoswitch: false`);
//! - `rust/crates/discord-voice/src/devices.rs`;
//! - `rust/gui-spike/ui/features/voice-devices-settings.js`.

use std::fmt;

/// Минимальная громкость (ввод и вывод), %.
pub const MIN_VOLUME: u16 = 0;

/// Максимальная громкость микрофона по пользовательскому слайдеру (`OX2Bnr`), %.
pub const MAX_INPUT_VOLUME: u16 = 100;

/// Начальная громкость микрофона по оракулу `OX2Bnr`, %.
pub const DEFAULT_INPUT_VOLUME: u16 = 100;

/// Максимальная громкость выходного канала (динамики), %.
pub const MAX_OUTPUT_VOLUME: u16 = 200;

/// Начальная громкость вывода по оракулу `aUJ062`, %.
pub const DEFAULT_OUTPUT_VOLUME: u16 = 100;

/// Нижняя граница порога активации голосом (`threshold_db: i8`), дБ.
pub const MIN_VAD_THRESHOLD_DB: i8 = -128;

/// Верхняя граница порога активации голосом (`threshold_db: i8`), дБ.
pub const MAX_VAD_THRESHOLD_DB: i8 = 127;

/// Нижняя граница рабочего окна шкалы чувствительности `0..=100%` (`[A]`, спека §3.4), дБ.
pub const MIN_SENSITIVITY_DB: i8 = -100;

/// Верхняя граница рабочего окна шкалы чувствительности `0..=100%` (`[A]`, спека §3.4), дБ.
pub const MAX_SENSITIVITY_DB: i8 = 0;

/// Дефолтный порог активации голосом (`VoiceActivity`), дБ (соответствует 60% чувствительности).
pub const DEFAULT_VAD_THRESHOLD_DB: i8 = -40;

/// Минимальная задержка отпускания клавиши рации (`PushToTalk`), мс.
pub const MIN_PTT_RELEASE_DELAY_MS: u32 = 0;

/// Максимальная задержка отпускания клавиши рации (`PushToTalk`), мс (`z0SZld`).
pub const MAX_PTT_RELEASE_DELAY_MS: u32 = 2000;

/// Дефолтная задержка отпускания клавиши рации (`PushToTalk`), мс.
pub const DEFAULT_PTT_RELEASE_DELAY_MS: u32 = 200;

/// Канонический идентификатор fallback-устройства ввода (`audio_input`).
pub const DEFAULT_INPUT_DEVICE_ID: &str = "default-input";

/// Канонический идентификатор fallback-устройства вывода (`audio_output`).
pub const DEFAULT_OUTPUT_DEVICE_ID: &str = "default-output";

/// Канонический идентификатор fallback-устройства видеоввода (`video_input`).
pub const DEFAULT_VIDEO_DEVICE_ID: &str = "default-video";

/// Начальный уровень сигнала индикатора проверки микрофона в покое (`idle`), дБ.
pub const DEFAULT_MIC_TEST_LEVEL_DB: i8 = -100;

/// Одиночные клавиши-модификаторы, которые не завершают захват клавиши рации (`PTT`).
pub const MODIFIER_KEYS: [&str; 6] = ["Shift", "Control", "Alt", "Meta", "AltGraph", "CapsLock"];

/// Локализованный текст предупреждения `vxfv7v` при блокировке PTT вне фокуса / без портала.
pub const PTT_BLOCKED_ALERT_TEXT: &str =
    "Горячая клавиша режима рации не работает, когда окно не в фокусе или глобальные горячие клавиши недоступны в системе.";

/// Направление / тип медиа-устройства.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioDeviceKind {
    /// Микрофон / линейный аудиовход (`audio_input`).
    Input,
    /// Динамики / наушники (`audio_output`).
    Output,
    /// Веб-камера / устройство видеозахвата (`video_input`).
    VideoInput,
}

impl AudioDeviceKind {
    /// Синоним `AudioDeviceKind::Input` для контрактов перечисления `audio_input`.
    pub const AUDIO_INPUT: Self = Self::Input;
    /// Синоним `AudioDeviceKind::Output` для контрактов перечисления `audio_output`.
    pub const AUDIO_OUTPUT: Self = Self::Output;
    /// Синоним `AudioDeviceKind::VideoInput` для контрактов перечисления `video_input`.
    pub const VIDEO_INPUT: Self = Self::VideoInput;
}

/// Дескриптор аудио- или видео-устройства.
///
/// Приватность (`G5-VOICE-PLAN.md` §5.1, §7.4 п.4): реализация `fmt::Debug`
/// редактирует системное имя `label` маркером `"<redacted>"`, чтобы названия
/// устройств ОС не утекали в диагностические логи.
#[derive(Clone, PartialEq, Eq)]
pub struct AudioDevice {
    pub id: String,
    pub label: String,
    pub kind: AudioDeviceKind,
    pub is_default: bool,
}

impl fmt::Debug for AudioDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AudioDevice")
            .field("id", &self.id)
            .field("label", &"<redacted>")
            .field("kind", &self.kind)
            .field("is_default", &self.is_default)
            .finish()
    }
}

/// Сгруппированный каталог устройств по направлениям (`audio_input`, `audio_output`, `video_input`)
/// с гарантированным fallback на `default-input` / `default-output` / `default-video`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeviceCatalog {
    pub audio_input: Vec<AudioDevice>,
    pub audio_output: Vec<AudioDevice>,
    pub video_input: Vec<AudioDevice>,
}

impl DeviceCatalog {
    /// Группирует список устройств и подставляет дефолтные заглушки (`default-input`,
    /// `default-output`, `default-video`), если в какой-либо категории нет валидных устройств.
    pub fn from_devices_with_fallback(devices: Vec<AudioDevice>) -> Self {
        let mut audio_input = Vec::new();
        let mut audio_output = Vec::new();
        let mut video_input = Vec::new();

        for device in normalize_devices(devices) {
            match device.kind {
                AudioDeviceKind::Input => audio_input.push(device),
                AudioDeviceKind::Output => audio_output.push(device),
                AudioDeviceKind::VideoInput => video_input.push(device),
            }
        }

        if audio_input.is_empty() {
            audio_input.push(AudioDevice {
                id: DEFAULT_INPUT_DEVICE_ID.to_owned(),
                label: "Устройство ввода по умолчанию".to_owned(),
                kind: AudioDeviceKind::Input,
                is_default: true,
            });
        }
        if audio_output.is_empty() {
            audio_output.push(AudioDevice {
                id: DEFAULT_OUTPUT_DEVICE_ID.to_owned(),
                label: "Устройство вывода по умолчанию".to_owned(),
                kind: AudioDeviceKind::Output,
                is_default: true,
            });
        }
        if video_input.is_empty() {
            video_input.push(AudioDevice {
                id: DEFAULT_VIDEO_DEVICE_ID.to_owned(),
                label: "Камера по умолчанию".to_owned(),
                kind: AudioDeviceKind::VideoInput,
                is_default: true,
            });
        }

        Self {
            audio_input,
            audio_output,
            video_input,
        }
    }

    /// Возвращает плоский список всех устройств каталога.
    pub fn into_devices(self) -> Vec<AudioDevice> {
        let mut all = Vec::with_capacity(
            self.audio_input.len() + self.audio_output.len() + self.video_input.len(),
        );
        all.extend(self.audio_input);
        all.extend(self.audio_output);
        all.extend(self.video_input);
        all
    }
}

/// Событие изменения активного устройства при hot-plug обновлении списка.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceEvent {
    /// Ранее выбранное устройство исчезло; выбран кандидат отката (`is_default == true`,
    /// иначе первое доступное устройство того же направления).
    DeviceFallback {
        kind: AudioDeviceKind,
        previous_id: String,
        fallback_id: String,
    },
    /// Все устройства данного направления отключены.
    NoDeviceAvailable {
        kind: AudioDeviceKind,
        previous_id: Option<String>,
    },
}

/// Ошибка явного выбора устройства пользователем.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceSelectError {
    /// Передан пустой идентификатор устройства.
    EmptyId,
    /// Устройство с указанным `id` отсутствует в актуальном списке.
    NotFound { id: String, kind: AudioDeviceKind },
    /// Устройство найдено, но относится к другому направлению.
    WrongKind {
        id: String,
        expected: AudioDeviceKind,
        actual: AudioDeviceKind,
    },
}

/// Режим активации передачи голоса.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceInputMode {
    /// Активация по голосу (VAD) с порогом в дБ (`-100..=0` рабочий диапазон, `i8` `-128..=127`).
    VoiceActivity { threshold_db: i8 },
    /// Режим рации (Push-to-Talk) с задержкой отпускания в мс (`0..=2000`).
    PushToTalk { release_delay_ms: u32 },
}

/// Уровень важности уведомления в `#voice-devices-notice`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeLevel {
    /// Информационный статус (`role="status"`, `aria-live="polite"`).
    Status,
    /// Предупреждение или ошибка (`role="alert"`, `aria-live="assertive"`).
    Alert,
}

/// Происхождение текста уведомления — определяет, безопасен ли текст для диагностических логов.
///
/// Спека §0.1 п.3: метки устройств ОС показываются только в DOM и никогда не попадают
/// в диагностику, поэтому в `fmt::Debug` текст с метками заменяется на `"<redacted>"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceNoticeOrigin {
    /// Текст собран из статических строк без меток устройств ОС.
    Static,
    /// Текст содержит системные метки устройств (`#voice-devices-notice`).
    DeviceLabels,
}

/// Состояние уведомления `#voice-devices-notice`.
#[derive(Clone, PartialEq, Eq)]
pub struct VoiceNotice {
    pub level: NoticeLevel,
    pub message: String,
    origin: VoiceNoticeOrigin,
}

impl fmt::Debug for VoiceNotice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("VoiceNotice");
        debug.field("level", &self.level);
        match self.origin {
            VoiceNoticeOrigin::Static => debug.field("message", &self.message),
            VoiceNoticeOrigin::DeviceLabels => debug.field("message", &"<redacted>"),
        };
        debug.finish()
    }
}

impl VoiceNotice {
    pub fn status(message: impl Into<String>) -> Self {
        Self {
            level: NoticeLevel::Status,
            message: message.into(),
            origin: VoiceNoticeOrigin::Static,
        }
    }

    pub fn alert(message: impl Into<String>) -> Self {
        Self {
            level: NoticeLevel::Alert,
            message: message.into(),
            origin: VoiceNoticeOrigin::Static,
        }
    }

    /// Уведомление об откате на резервное устройство: текст содержит метки ОС и попадает
    /// только в DOM (`#voice-devices-notice`), в `fmt::Debug` он редактируется.
    pub fn device_fallback(previous_label: &str, fallback_label: &str) -> Self {
        Self {
            level: NoticeLevel::Status,
            message: format!(
                "Устройство «{previous_label}» отключено. Выбрано «{fallback_label}»."
            ),
            origin: VoiceNoticeOrigin::DeviceLabels,
        }
    }

    pub fn role(&self) -> &'static str {
        match self.level {
            NoticeLevel::Status => "status",
            NoticeLevel::Alert => "alert",
        }
    }

    pub fn aria_live(&self) -> &'static str {
        match self.level {
            NoticeLevel::Status => "polite",
            NoticeLevel::Alert => "assertive",
        }
    }
}

/// Диагностический статус доступности горячей клавиши рации (`PushToTalk`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PttStatus {
    /// Рация готова к передаче (`data-ptt-status="ready"`).
    Ready,
    /// Рация заблокирована из-за потери фокуса окна без портала GlobalShortcuts (`data-ptt-status="blocked"`).
    Blocked,
}

impl PttStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Blocked => "blocked",
        }
    }
}

/// Состояние встроенной проверки микрофона (`VOICE-DEVICES-SETTINGS-SPEC.md` §3.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicTestState {
    /// Проверка не запущена (`idle`).
    Idle,
    /// Идёт активный захват и отображение уровня (`testing`).
    Testing,
    /// Захват запущен, но сигнал отсутствует (`no-signal`).
    NoSignal,
    /// Захват заблокирован активным `effective_mute` (`muted`).
    Muted,
}

impl MicTestState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Testing => "testing",
            Self::NoSignal => "no-signal",
            Self::Muted => "muted",
        }
    }
}

/// Проекция элемента `<option>` для `<select>` выбора устройства.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDeviceOptionView {
    pub value: String,
    pub text: String,
    pub title: String,
}

/// Проекция `<select>` выбора устройства (`#voice-input-select` / `#voice-output-select`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceSelectView {
    pub value: String,
    pub aria_disabled: bool,
    pub options: Vec<AudioDeviceOptionView>,
}

/// Проекция индикатора и кнопки проверки микрофона (`#voice-mic-test-*`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MicTestMeterView {
    pub level_db: i8,
    pub fill_percent: u8,
    pub threshold_percent: u8,
    pub tone: String,
    pub aria_valuetext: String,
    pub button_text: String,
    pub aria_pressed: bool,
    pub aria_disabled: bool,
    pub status_text: String,
}

/// BFF-сервис применения настроек голоса и устройств (`VOICE-DEVICES-SETTINGS-SPEC.md` §1.3).
pub trait VoiceDevicesService: Send + Sync {
    fn select_input(&self, _id: &str) -> Result<(), String> {
        Ok(())
    }
    fn select_output(&self, _id: &str) -> Result<(), String> {
        Ok(())
    }
    fn select_video(&self, _id: &str) -> Result<(), String> {
        Ok(())
    }
    fn set_input_volume(&self, _percent: u16) -> Result<(), String> {
        Ok(())
    }
    fn set_output_volume(&self, _percent: u16) -> Result<(), String> {
        Ok(())
    }
    fn set_input_mode(&self, _mode: VoiceInputMode) -> Result<(), String> {
        Ok(())
    }
    fn set_user_mute(&self, _mute: bool) -> Result<(), String> {
        Ok(())
    }
    fn set_self_deaf(&self, _deaf: bool) -> Result<(), String> {
        Ok(())
    }
    fn set_ptt_key(&self, _key: Option<&str>) -> Result<(), String> {
        Ok(())
    }
    fn start_mic_test(&self) -> Result<(), String> {
        Ok(())
    }
    fn stop_mic_test(&self) -> Result<(), String> {
        Ok(())
    }
}

/// Проверяет, является ли клавиша одиночным модификатором (`MODIFIER_KEYS`).
pub fn is_modifier_key(key: &str) -> bool {
    MODIFIER_KEYS.contains(&key)
}

/// Переводит процент чувствительности `0..=100%` (из дампа эталона `voice-spec.json`)
/// в порог активации VAD в дБ (`-100..=0`, рабочее окно шкалы).
pub fn sensitivity_percent_to_threshold_db(percent: f64) -> i8 {
    if !percent.is_finite() {
        return DEFAULT_VAD_THRESHOLD_DB;
    }
    let clamped = percent.clamp(0.0, 100.0);
    let db = (clamped - 100.0).round() as i16;
    db.clamp(i16::from(MIN_SENSITIVITY_DB), i16::from(MAX_SENSITIVITY_DB)) as i8
}

/// Переводит порог активации VAD в дБ в процент заполнения шкалы `0..=100%`
/// (`min(100, max(0, db + 100))`, спека §3.4).
pub fn threshold_db_to_sensitivity_percent(threshold_db: i8) -> u8 {
    let clamped = threshold_db.clamp(MIN_SENSITIVITY_DB, MAX_SENSITIVITY_DB);
    (i16::from(clamped) + 100) as u8
}

fn normalize_devices(devices: Vec<AudioDevice>) -> Vec<AudioDevice> {
    devices
        .into_iter()
        .filter_map(|d| {
            let trimmed_id = d.id.trim().to_owned();
            if trimmed_id.is_empty() {
                return None;
            }
            let label = if d.label.trim().is_empty() {
                trimmed_id.clone()
            } else {
                d.label
            };
            Some(AudioDevice {
                id: trimmed_id,
                label,
                kind: d.kind,
                is_default: d.is_default,
            })
        })
        .collect()
}

fn pick_fallback(devices: &[AudioDevice], kind: AudioDeviceKind) -> Option<String> {
    devices
        .iter()
        .find(|d| d.kind == kind && d.is_default)
        .or_else(|| devices.iter().find(|d| d.kind == kind))
        .map(|d| d.id.clone())
}

/// Состояние BFF-модуля управления аудио/видео-устройствами и настройками голоса.
pub struct VoiceDevicesState {
    devices: Vec<AudioDevice>,
    selected_input: Option<String>,
    selected_output: Option<String>,
    selected_video: Option<String>,
    input_volume: u16,
    output_volume: u16,
    input_mode: VoiceInputMode,
    vad_threshold_db: i8,
    auto_sensitivity: bool,
    ptt_release_delay_ms: u32,
    ptt_key: Option<String>,
    ptt_capturing: bool,
    window_focused: bool,
    global_shortcuts_available: bool,
    user_mute: bool,
    self_deaf: bool,
    ptt_last_active_ms: Option<u64>,
    mic_test_active: bool,
    mic_test_level_db: i8,
    mic_test_has_sample: bool,
    notice: Option<VoiceNotice>,
    voice_panel_diagnostic_state: Option<&'static str>,
    service: Option<Box<dyn VoiceDevicesService>>,
}

impl Default for VoiceDevicesState {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for VoiceDevicesState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VoiceDevicesState")
            .field("devices", &self.devices)
            .field("selected_input", &self.selected_input)
            .field("selected_output", &self.selected_output)
            .field("selected_video", &self.selected_video)
            .field("input_volume", &self.input_volume)
            .field("output_volume", &self.output_volume)
            .field("input_mode", &self.input_mode)
            .field("vad_threshold_db", &self.vad_threshold_db)
            .field("auto_sensitivity", &self.auto_sensitivity)
            .field("ptt_release_delay_ms", &self.ptt_release_delay_ms)
            .field("ptt_key", &self.ptt_key)
            .field("ptt_capturing", &self.ptt_capturing)
            .field("window_focused", &self.window_focused)
            .field(
                "global_shortcuts_available",
                &self.global_shortcuts_available,
            )
            .field("user_mute", &self.user_mute)
            .field("self_deaf", &self.self_deaf)
            .field("effective_mute", &self.effective_mute())
            .field("mic_test_state", &self.mic_test_state())
            .field("notice", &self.notice)
            .finish()
    }
}

impl VoiceDevicesState {
    /// Создаёт пустое состояние с дефолтными параметрами громкости и режима ввода.
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
            selected_input: None,
            selected_output: None,
            selected_video: None,
            input_volume: DEFAULT_INPUT_VOLUME,
            output_volume: DEFAULT_OUTPUT_VOLUME,
            input_mode: VoiceInputMode::VoiceActivity {
                threshold_db: DEFAULT_VAD_THRESHOLD_DB,
            },
            vad_threshold_db: DEFAULT_VAD_THRESHOLD_DB,
            auto_sensitivity: false,
            ptt_release_delay_ms: DEFAULT_PTT_RELEASE_DELAY_MS,
            ptt_key: None,
            ptt_capturing: false,
            window_focused: true,
            global_shortcuts_available: true,
            user_mute: false,
            self_deaf: false,
            ptt_last_active_ms: None,
            mic_test_active: false,
            mic_test_level_db: DEFAULT_MIC_TEST_LEVEL_DB,
            mic_test_has_sample: false,
            notice: None,
            voice_panel_diagnostic_state: None,
            service: None,
        }
    }

    /// Создаёт состояние с начальным списком устройств, выбирая `is_default == true`
    /// (или первое доступное устройство соответствующего типа) без генерации hot-plug уведомлений.
    pub fn with_devices(devices: Vec<AudioDevice>) -> Self {
        let mut state = Self::new();
        let normalized = normalize_devices(devices);
        state.selected_input = pick_fallback(&normalized, AudioDeviceKind::Input);
        state.selected_output = pick_fallback(&normalized, AudioDeviceKind::Output);
        state.selected_video = pick_fallback(&normalized, AudioDeviceKind::VideoInput);
        state.devices = normalized;
        state
    }

    /// Создаёт состояние с гарантированным каталогом по умолчанию (`default-input`, `default-output`, `default-video`).
    pub fn with_default_catalog() -> Self {
        Self::from_catalog_with_fallback(Vec::new())
    }

    /// Создаёт состояние из произвольного списка устройств, дополняя отсутствующие направления
    /// дефолтными устройствами (`default-input`, `default-output`, `default-video`).
    pub fn from_catalog_with_fallback(devices: Vec<AudioDevice>) -> Self {
        let catalog = DeviceCatalog::from_devices_with_fallback(devices);
        Self::with_devices(catalog.into_devices())
    }

    /// Подключает BFF-сервис для синхронизации изменений с аудио-движком.
    pub fn set_service(&mut self, service: Box<dyn VoiceDevicesService>) {
        self.service = Some(service);
    }

    fn handle_service_result(&mut self, result: Result<(), String>) {
        if let Err(err) = result {
            let reason = if err.trim().is_empty() {
                "неизвестная ошибка"
            } else {
                err.trim()
            };
            self.notice = Some(VoiceNotice::alert(format!(
                "Не удалось сохранить настройку: {reason}"
            )));
        }
    }

    /// Список доступных микрофонов (`audio_input`).
    pub fn input_devices(&self) -> Vec<&AudioDevice> {
        self.devices
            .iter()
            .filter(|d| d.kind == AudioDeviceKind::Input)
            .collect()
    }

    /// Список доступных устройств вывода (`audio_output`).
    pub fn output_devices(&self) -> Vec<&AudioDevice> {
        self.devices
            .iter()
            .filter(|d| d.kind == AudioDeviceKind::Output)
            .collect()
    }

    /// Список доступных видеокамер (`video_input`).
    pub fn video_devices(&self) -> Vec<&AudioDevice> {
        self.devices
            .iter()
            .filter(|d| d.kind == AudioDeviceKind::VideoInput)
            .collect()
    }

    /// Выбранный идентификатор микрофона.
    pub fn selected_input(&self) -> Option<&str> {
        self.selected_input.as_deref()
    }

    /// Выбранный идентификатор устройства вывода.
    pub fn selected_output(&self) -> Option<&str> {
        self.selected_output.as_deref()
    }

    /// Выбранный идентификатор видеокамеры.
    pub fn selected_video(&self) -> Option<&str> {
        self.selected_video.as_deref()
    }

    /// Дескриптор активного микрофона.
    pub fn active_input_device(&self) -> Option<&AudioDevice> {
        let id = self.selected_input.as_deref()?;
        self.devices
            .iter()
            .find(|d| d.kind == AudioDeviceKind::Input && d.id == id)
    }

    /// Дескриптор активного устройства вывода.
    pub fn active_output_device(&self) -> Option<&AudioDevice> {
        let id = self.selected_output.as_deref()?;
        self.devices
            .iter()
            .find(|d| d.kind == AudioDeviceKind::Output && d.id == id)
    }

    /// Дескриптор активной видеокамеры.
    pub fn active_video_device(&self) -> Option<&AudioDevice> {
        let id = self.selected_video.as_deref()?;
        self.devices
            .iter()
            .find(|d| d.kind == AudioDeviceKind::VideoInput && d.id == id)
    }

    /// Явный выбор микрофона пользователем.
    pub fn select_input(&mut self, id: &str) -> Result<(), DeviceSelectError> {
        self.select_by_kind(id, AudioDeviceKind::Input)
    }

    /// Явный выбор устройства вывода пользователем.
    pub fn select_output(&mut self, id: &str) -> Result<(), DeviceSelectError> {
        self.select_by_kind(id, AudioDeviceKind::Output)
    }

    /// Явный выбор видеокамеры пользователем.
    pub fn select_video(&mut self, id: &str) -> Result<(), DeviceSelectError> {
        self.select_by_kind(id, AudioDeviceKind::VideoInput)
    }

    fn select_by_kind(
        &mut self,
        id: &str,
        expected: AudioDeviceKind,
    ) -> Result<(), DeviceSelectError> {
        if id.trim().is_empty() {
            self.notice = Some(VoiceNotice::alert("Устройство не найдено."));
            return Err(DeviceSelectError::EmptyId);
        }

        let found = self.devices.iter().find(|d| d.id == id);
        match found {
            None => {
                self.notice = Some(VoiceNotice::alert("Устройство не найдено."));
                Err(DeviceSelectError::NotFound {
                    id: id.to_owned(),
                    kind: expected,
                })
            }
            Some(device) if device.kind != expected => {
                let actual = device.kind;
                self.notice = Some(VoiceNotice::alert("Устройство не найдено."));
                Err(DeviceSelectError::WrongKind {
                    id: id.to_owned(),
                    expected,
                    actual,
                })
            }
            Some(_) => {
                let owned_id = id.to_owned();
                match expected {
                    AudioDeviceKind::Input => {
                        self.selected_input = Some(owned_id.clone());
                        self.voice_panel_diagnostic_state = None;
                    }
                    AudioDeviceKind::Output => {
                        self.selected_output = Some(owned_id.clone());
                    }
                    AudioDeviceKind::VideoInput => {
                        self.selected_video = Some(owned_id.clone());
                    }
                }
                self.notice = None;
                let res = match (&self.service, expected) {
                    (Some(svc), AudioDeviceKind::Input) => svc.select_input(&owned_id),
                    (Some(svc), AudioDeviceKind::Output) => svc.select_output(&owned_id),
                    (Some(svc), AudioDeviceKind::VideoInput) => svc.select_video(&owned_id),
                    (None, _) => Ok(()),
                };
                self.handle_service_result(res);
                Ok(())
            }
        }
    }

    /// Обновляет список устройств при hot-plug событии без авто-переключения живого тракта
    /// (`devices.hotplug.autoswitch: false`, `VOICE-DEVICES-SETTINGS-SPEC.md` §3.2).
    pub fn update_device_list(&mut self, devices: Vec<AudioDevice>) -> Vec<DeviceEvent> {
        let previous_devices = std::mem::take(&mut self.devices);
        self.devices = normalize_devices(devices);

        let mut events = Vec::new();
        let mut next_notice: Option<VoiceNotice> = None;

        for kind in [AudioDeviceKind::Input, AudioDeviceKind::Output] {
            let current_selected = match kind {
                AudioDeviceKind::Input => self.selected_input.clone(),
                AudioDeviceKind::Output => self.selected_output.clone(),
                AudioDeviceKind::VideoInput => unreachable!(),
            };

            let still_present = current_selected.as_deref().is_some_and(|sel_id| {
                self.devices
                    .iter()
                    .any(|d| d.kind == kind && d.id == sel_id)
            });

            if still_present {
                if kind == AudioDeviceKind::Input {
                    self.voice_panel_diagnostic_state = None;
                }
                continue;
            }

            let fallback = pick_fallback(&self.devices, kind);
            match (&current_selected, fallback) {
                (Some(prev_id), Some(fallback_id)) => {
                    let prev_label = previous_devices
                        .iter()
                        .find(|d| d.kind == kind && d.id == *prev_id)
                        .map(|d| d.label.as_str())
                        .unwrap_or(prev_id.as_str());
                    let next_label = self
                        .devices
                        .iter()
                        .find(|d| d.kind == kind && d.id == fallback_id)
                        .map(|d| d.label.as_str())
                        .unwrap_or(fallback_id.as_str());

                    match kind {
                        AudioDeviceKind::Input => {
                            self.selected_input = Some(fallback_id.clone());
                            self.voice_panel_diagnostic_state = Some("device-missing");
                        }
                        AudioDeviceKind::Output => {
                            self.selected_output = Some(fallback_id.clone());
                        }
                        AudioDeviceKind::VideoInput => unreachable!(),
                    }

                    next_notice = Some(VoiceNotice::device_fallback(prev_label, next_label));
                    events.push(DeviceEvent::DeviceFallback {
                        kind,
                        previous_id: prev_id.clone(),
                        fallback_id,
                    });
                }
                (None, Some(fallback_id)) => match kind {
                    AudioDeviceKind::Input => {
                        self.selected_input = Some(fallback_id);
                        self.voice_panel_diagnostic_state = None;
                    }
                    AudioDeviceKind::Output => {
                        self.selected_output = Some(fallback_id);
                    }
                    AudioDeviceKind::VideoInput => unreachable!(),
                },
                (prev_opt, None) => {
                    match kind {
                        AudioDeviceKind::Input => {
                            self.selected_input = None;
                            self.voice_panel_diagnostic_state = Some("device-missing");
                            next_notice = Some(VoiceNotice::alert("Нет доступных микрофонов."));
                        }
                        AudioDeviceKind::Output => {
                            self.selected_output = None;
                            next_notice =
                                Some(VoiceNotice::alert("Нет доступных устройств вывода."));
                        }
                        AudioDeviceKind::VideoInput => unreachable!(),
                    }
                    events.push(DeviceEvent::NoDeviceAvailable {
                        kind,
                        previous_id: prev_opt.clone(),
                    });
                }
            }
        }

        // Реконсиляция выбранной камеры (`video_input`):
        let video_still_present = self.selected_video.as_deref().is_some_and(|sel_id| {
            self.devices
                .iter()
                .any(|d| d.kind == AudioDeviceKind::VideoInput && d.id == sel_id)
        });
        if !video_still_present {
            self.selected_video = pick_fallback(&self.devices, AudioDeviceKind::VideoInput);
        }

        if let Some(notice) = next_notice {
            self.notice = Some(notice);
        } else if self.ptt_status() != PttStatus::Blocked {
            self.notice = None;
        }

        events
    }

    /// Проекция `<select id="voice-input-select">` для рендера в UI.
    pub fn input_select_view(&self) -> DeviceSelectView {
        self.build_select_view(AudioDeviceKind::Input, "Микрофон не выбран")
    }

    /// Проекция `<select id="voice-output-select">` для рендера в UI.
    pub fn output_select_view(&self) -> DeviceSelectView {
        self.build_select_view(AudioDeviceKind::Output, "Устройство вывода не выбрано")
    }

    fn build_select_view(&self, kind: AudioDeviceKind, empty_label: &str) -> DeviceSelectView {
        let list: Vec<&AudioDevice> = self.devices.iter().filter(|d| d.kind == kind).collect();
        if list.is_empty() {
            return DeviceSelectView {
                value: String::new(),
                aria_disabled: true,
                options: vec![AudioDeviceOptionView {
                    value: String::new(),
                    text: empty_label.to_owned(),
                    title: empty_label.to_owned(),
                }],
            };
        }

        let selected = match kind {
            AudioDeviceKind::Input => self.selected_input.as_deref(),
            AudioDeviceKind::Output => self.selected_output.as_deref(),
            AudioDeviceKind::VideoInput => self.selected_video.as_deref(),
        };

        let options = list
            .iter()
            .map(|d| AudioDeviceOptionView {
                value: d.id.clone(),
                text: d.label.clone(),
                title: d.label.clone(),
            })
            .collect();

        DeviceSelectView {
            value: selected.unwrap_or_default().to_owned(),
            aria_disabled: false,
            options,
        }
    }

    /// Текущая громкость микрофона (`input_volume`), %.
    pub fn input_volume(&self) -> u16 {
        self.input_volume
    }

    /// Текущая громкость вывода (`output_volume`), %.
    pub fn output_volume(&self) -> u16 {
        self.output_volume
    }

    /// Устанавливает громкость микрофона с клампингом к пользовательскому диапазону `0..=100%` (`OX2Bnr`).
    pub fn set_input_volume(&mut self, percent: u16) -> u16 {
        let clamped = percent.clamp(MIN_VOLUME, MAX_INPUT_VOLUME);
        self.input_volume = clamped;
        let res = self
            .service
            .as_ref()
            .map_or(Ok(()), |svc| svc.set_input_volume(clamped));
        self.handle_service_result(res);
        clamped
    }

    /// Устанавливает громкость микрофона со знаковым входом и клампингом к `0..=100%`.
    pub fn set_input_volume_signed(&mut self, percent: i32) -> u16 {
        let clamped = percent.clamp(i32::from(MIN_VOLUME), i32::from(MAX_INPUT_VOLUME)) as u16;
        self.set_input_volume(clamped)
    }

    /// Устанавливает громкость вывода с клампингом к диапазону `0..=200%` (`aUJ062`).
    pub fn set_output_volume(&mut self, percent: u16) -> u16 {
        let clamped = percent.clamp(MIN_VOLUME, MAX_OUTPUT_VOLUME);
        self.output_volume = clamped;
        let res = self
            .service
            .as_ref()
            .map_or(Ok(()), |svc| svc.set_output_volume(clamped));
        self.handle_service_result(res);
        clamped
    }

    /// Устанавливает громкость вывода со знаковым входом и клампингом к `0..=200%`.
    pub fn set_output_volume_signed(&mut self, percent: i32) -> u16 {
        let clamped = percent.clamp(i32::from(MIN_VOLUME), i32::from(MAX_OUTPUT_VOLUME)) as u16;
        self.set_output_volume(clamped)
    }

    /// Текстовое представление громкости микрофона (`"{input_volume}%"`).
    pub fn input_volume_text(&self) -> String {
        format!("{}%", self.input_volume)
    }

    /// Текстовое представление громкости вывода (`"{output_volume}%"`).
    pub fn output_volume_text(&self) -> String {
        format!("{}%", self.output_volume)
    }

    /// Активный режим ввода (`VoiceActivity` или `PushToTalk`).
    pub fn input_mode(&self) -> VoiceInputMode {
        self.input_mode
    }

    /// Устанавливает режим ввода напрямую, сохраняя и валидируя параметры.
    pub fn set_input_mode(&mut self, mode: VoiceInputMode) {
        match mode {
            VoiceInputMode::VoiceActivity { threshold_db } => {
                self.vad_threshold_db = threshold_db;
                self.cancel_ptt_key_capture();
                self.ptt_last_active_ms = None;
                self.input_mode = VoiceInputMode::VoiceActivity { threshold_db };
            }
            VoiceInputMode::PushToTalk { release_delay_ms } => {
                let clamped =
                    release_delay_ms.clamp(MIN_PTT_RELEASE_DELAY_MS, MAX_PTT_RELEASE_DELAY_MS);
                self.ptt_release_delay_ms = clamped;
                self.input_mode = VoiceInputMode::PushToTalk {
                    release_delay_ms: clamped,
                };
            }
        }
        let current_mode = self.input_mode;
        let res = self
            .service
            .as_ref()
            .map_or(Ok(()), |svc| svc.set_input_mode(current_mode));
        self.handle_service_result(res);
        self.sync_ptt_focus_notice();
    }

    /// Текущий порог активации голосом (VAD), дБ.
    pub fn vad_threshold_db(&self) -> i8 {
        self.vad_threshold_db
    }

    /// Устанавливает порог активации голосом в полном диапазоне `i8` (`-128..=127` дБ),
    /// как слайдер `#voice-devices-vad-threshold` и JS-контроллер `setInputMode`.
    pub fn set_vad_threshold_db(&mut self, db: i16) -> i8 {
        let clamped = db.clamp(
            i16::from(MIN_VAD_THRESHOLD_DB),
            i16::from(MAX_VAD_THRESHOLD_DB),
        ) as i8;
        self.vad_threshold_db = clamped;
        if matches!(self.input_mode, VoiceInputMode::VoiceActivity { .. }) {
            self.input_mode = VoiceInputMode::VoiceActivity {
                threshold_db: clamped,
            };
            let mode = self.input_mode;
            let res = self
                .service
                .as_ref()
                .map_or(Ok(()), |svc| svc.set_input_mode(mode));
            self.handle_service_result(res);
        }
        clamped
    }

    /// Синоним `set_vad_threshold_db` для явного инженерного контроля порога (`-128..=127` дБ).
    pub fn set_raw_vad_threshold_db(&mut self, db: i16) -> i8 {
        self.set_vad_threshold_db(db)
    }

    /// Текстовое представление порога VAD (`"{vad_threshold_db} дБ"`).
    pub fn vad_threshold_text(&self) -> String {
        format!("{} дБ", self.vad_threshold_db)
    }

    /// Включено ли автоматическое определение чувствительности микрофона (`lY6j47`).
    pub fn auto_sensitivity(&self) -> bool {
        self.auto_sensitivity
    }

    /// Включает или выключает автоматическое определение чувствительности (`lY6j47`),
    /// не сбрасывая сохранённый ручной порог `vad_threshold_db`.
    pub fn set_auto_sensitivity(&mut self, enabled: bool) {
        self.auto_sensitivity = enabled;
    }

    /// Заблокирован ли ручной слайдер порога VAD (`aria-disabled="true"` при `auto_sensitivity == true`).
    pub fn is_manual_vad_disabled(&self) -> bool {
        self.auto_sensitivity
    }

    /// Задержка отпускания клавиши рации (`PushToTalk`), мс.
    pub fn ptt_release_delay_ms(&self) -> u32 {
        self.ptt_release_delay_ms
    }

    /// Устанавливает задержку отпускания клавиши рации с клампингом к `0..=2000` мс (`z0SZld`).
    pub fn set_ptt_release_delay_ms(&mut self, delay_ms: i64) -> u32 {
        let clamped = delay_ms.clamp(
            i64::from(MIN_PTT_RELEASE_DELAY_MS),
            i64::from(MAX_PTT_RELEASE_DELAY_MS),
        ) as u32;
        self.ptt_release_delay_ms = clamped;
        if matches!(self.input_mode, VoiceInputMode::PushToTalk { .. }) {
            self.input_mode = VoiceInputMode::PushToTalk {
                release_delay_ms: clamped,
            };
            let mode = self.input_mode;
            let res = self
                .service
                .as_ref()
                .map_or(Ok(()), |svc| svc.set_input_mode(mode));
            self.handle_service_result(res);
        }
        clamped
    }

    /// Текстовое представление задержки отпускания PTT (`"{ptt_release_delay_ms} мс"`).
    pub fn ptt_release_delay_text(&self) -> String {
        format!("{} мс", self.ptt_release_delay_ms)
    }

    /// Переключает тумблер «Режим рации» (`#voice-ptt-switch`), сохраняя настройки обоих режимов.
    pub fn set_ptt_enabled(&mut self, enabled: bool) {
        if enabled {
            self.input_mode = VoiceInputMode::PushToTalk {
                release_delay_ms: self.ptt_release_delay_ms,
            };
        } else {
            self.cancel_ptt_key_capture();
            self.ptt_last_active_ms = None;
            self.input_mode = VoiceInputMode::VoiceActivity {
                threshold_db: self.vad_threshold_db,
            };
        }
        let mode = self.input_mode;
        let res = self
            .service
            .as_ref()
            .map_or(Ok(()), |svc| svc.set_input_mode(mode));
        self.handle_service_result(res);
        self.sync_ptt_focus_notice();
    }

    /// Состояние тумблера `#voice-ptt-switch` (`true`, если активен `PushToTalk`).
    pub fn ptt_switch_checked(&self) -> bool {
        matches!(self.input_mode, VoiceInputMode::PushToTalk { .. })
    }

    /// Скрыта ли группа настроек VAD (`#voice-vad-group.hidden`).
    pub fn vad_group_hidden(&self) -> bool {
        self.ptt_switch_checked()
    }

    /// Скрыта ли группа настроек PTT (`#voice-ptt-group.hidden`).
    pub fn ptt_group_hidden(&self) -> bool {
        !self.ptt_switch_checked()
    }

    /// ARIA-роль контейнера `#voice-vad-group`.
    pub fn vad_group_role(&self) -> &'static str {
        "group"
    }

    /// ARIA-роль контейнера `#voice-ptt-group`.
    pub fn ptt_group_role(&self) -> &'static str {
        "group"
    }

    /// Назначенная клавиша рации (`ptt_key` / `shortcut`).
    pub fn ptt_key(&self) -> Option<&str> {
        self.ptt_key.as_deref()
    }

    /// Синоним `ptt_key()` для контракта `PushToTalk` с `shortcut`.
    pub fn shortcut(&self) -> Option<&str> {
        self.ptt_key.as_deref()
    }

    /// Устанавливает или сбрасывает `shortcut` для режима `PushToTalk`.
    pub fn set_shortcut(&mut self, shortcut: Option<&str>) {
        self.ptt_key = shortcut.map(str::to_owned).filter(|s| !s.is_empty());
        self.ptt_capturing = false;
        let key_ref = self.ptt_key.as_deref();
        let res = self
            .service
            .as_ref()
            .map_or(Ok(()), |svc| svc.set_ptt_key(key_ref));
        self.handle_service_result(res);
    }

    /// Сбрасывает назначенную горячую клавишу рации.
    pub fn clear_ptt_key(&mut self) {
        self.set_shortcut(None);
    }

    /// Находится ли кнопка `#voice-ptt-key-btn` в режиме ожидания нажатия клавиши.
    pub fn ptt_capturing(&self) -> bool {
        self.ptt_capturing
    }

    /// Начинает захват клавиши для рации (`#voice-ptt-key-btn`).
    pub fn begin_ptt_key_capture(&mut self) {
        self.ptt_capturing = true;
    }

    /// Отменяет активный захват клавиши рации без изменения прежней привязки.
    pub fn cancel_ptt_key_capture(&mut self) {
        self.ptt_capturing = false;
    }

    /// Обрабатывает нажатие клавиши во время активного захвата `#voice-ptt-key-btn`.
    /// Возвращает `true`, если событие было перехвачено режимом захвата.
    pub fn handle_ptt_capture_keydown(&mut self, key: &str) -> bool {
        if !self.ptt_capturing {
            return false;
        }
        if key == "Escape" {
            self.cancel_ptt_key_capture();
            return true;
        }
        if key.is_empty() || is_modifier_key(key) {
            return true;
        }
        self.ptt_capturing = false;
        self.ptt_key = Some(key.to_owned());
        let key_ref = self.ptt_key.as_deref();
        let res = self
            .service
            .as_ref()
            .map_or(Ok(()), |svc| svc.set_ptt_key(key_ref));
        self.handle_service_result(res);
        true
    }

    /// Форматирует подпись кнопки `#voice-ptt-key-btn` по правилам `VOICE-DEVICES-SETTINGS-SPEC.md` §3.6.
    pub fn format_ptt_key_button_text(&self) -> String {
        if self.ptt_capturing {
            return "Нажмите клавишу…".to_owned();
        }
        match self.ptt_key.as_deref() {
            None | Some("") => "Горячие клавиши не назначены".to_owned(),
            Some(" ") => "Клавиша: Пробел".to_owned(),
            Some(key) => format!("Клавиша: {key}"),
        }
    }

    /// Обновляет состояние фокуса окна и проверяет блокировку PTT вне фокуса (`vxfv7v`).
    pub fn set_window_focused(&mut self, focused: bool) {
        self.window_focused = focused;
        if !focused && self.ptt_capturing {
            self.cancel_ptt_key_capture();
        }
        self.sync_ptt_focus_notice();
    }

    /// Обновляет признак доступности системного портала `GlobalShortcuts`.
    pub fn set_global_shortcuts_available(&mut self, available: bool) {
        self.global_shortcuts_available = available;
        self.sync_ptt_focus_notice();
    }

    fn sync_ptt_focus_notice(&mut self) {
        if self.ptt_status() == PttStatus::Blocked {
            self.ptt_last_active_ms = None;
            self.notice = Some(VoiceNotice::alert(PTT_BLOCKED_ALERT_TEXT));
        } else if self
            .notice
            .as_ref()
            .is_some_and(|n| n.message == PTT_BLOCKED_ALERT_TEXT)
        {
            self.notice = None;
        }
    }

    /// Текущий диагностический статус рации (`Ready` или `Blocked`).
    pub fn ptt_status(&self) -> PttStatus {
        if self.ptt_switch_checked() && !self.window_focused && !self.global_shortcuts_available {
            PttStatus::Blocked
        } else {
            PttStatus::Ready
        }
    }

    /// Значение атрибута `data-ptt` на `#voice-panel`.
    pub fn voice_panel_ptt_attr(&self) -> &'static str {
        match self.ptt_status() {
            PttStatus::Ready => "idle",
            PttStatus::Blocked => "blocked",
        }
    }

    /// Значение `data-voice-state` на `#voice-panel` (например, `"device-missing"` при потере микрофона).
    pub fn voice_panel_diagnostic_state(&self) -> Option<&'static str> {
        self.voice_panel_diagnostic_state
    }

    /// Явно выставленный пользователем флаг отключения микрофона (`user_mute`).
    pub fn user_mute(&self) -> bool {
        self.user_mute
    }

    /// Синоним `user_mute()` для контракта `self_mute`.
    pub fn self_mute(&self) -> bool {
        self.user_mute
    }

    /// Флаг отключения звука (`self_deaf`).
    pub fn self_deaf(&self) -> bool {
        self.self_deaf
    }

    /// Эффективное состояние отключения микрофона: `effective_mute = self_mute || self_deaf`.
    pub fn effective_mute(&self) -> bool {
        self.user_mute || self.self_deaf
    }

    /// Устанавливает пользовательский `self_mute` (`user_mute`).
    pub fn set_self_mute(&mut self, mute: bool) {
        self.user_mute = mute;
        if self.effective_mute() {
            self.ptt_last_active_ms = None;
        }
        let res = self
            .service
            .as_ref()
            .map_or(Ok(()), |svc| svc.set_user_mute(mute));
        self.handle_service_result(res);
    }

    /// Устанавливает `self_deaf`. Снятие `self_deaf` НЕ сбрасывает явно выставленный `self_mute`.
    pub fn set_self_deaf(&mut self, deaf: bool) {
        self.self_deaf = deaf;
        if self.effective_mute() {
            self.ptt_last_active_ms = None;
        }
        let res = self
            .service
            .as_ref()
            .map_or(Ok(()), |svc| svc.set_self_deaf(deaf));
        self.handle_service_result(res);
    }

    /// Текст бейджа `#voice-effective-mute-val`.
    pub fn effective_mute_text(&self) -> &'static str {
        if self.effective_mute() {
            "Микрофон отключён"
        } else {
            "Микрофон активен"
        }
    }

    /// Значение `aria-checked` на `#voice-user-mute-switch` (отражает только `user_mute`).
    pub fn user_mute_aria_checked(&self) -> &'static str {
        if self.user_mute {
            "true"
        } else {
            "false"
        }
    }

    /// Значение `aria-checked` на `#voice-self-deaf-switch`.
    pub fn self_deaf_aria_checked(&self) -> &'static str {
        if self.self_deaf {
            "true"
        } else {
            "false"
        }
    }

    /// Кортеж `(self_mute, self_deaf)` для отправки в Gateway OP4 Voice State Update,
    /// где первый элемент равен `effective_mute = self_mute || self_deaf`.
    pub fn gateway_mute_deaf(&self) -> (bool, bool) {
        (self.effective_mute(), self.self_deaf)
    }

    /// Вычисляет, открыта ли передача голоса в момент `now_ms`.
    pub fn is_transmitting(&mut self, now_ms: u64, ptt_pressed: bool, level_db: i8) -> bool {
        if self.effective_mute() || self.ptt_status() == PttStatus::Blocked {
            self.ptt_last_active_ms = None;
            return false;
        }

        match self.input_mode {
            VoiceInputMode::VoiceActivity { threshold_db } => level_db >= threshold_db,
            VoiceInputMode::PushToTalk { release_delay_ms } => {
                if ptt_pressed {
                    self.ptt_last_active_ms = Some(now_ms);
                    true
                } else if let Some(last_ms) = self.ptt_last_active_ms {
                    now_ms.saturating_sub(last_ms) <= u64::from(release_delay_ms)
                } else {
                    false
                }
            }
        }
    }

    /// Текущее состояние встроенной проверки микрофона (`MicTestState`).
    pub fn mic_test_state(&self) -> MicTestState {
        if !self.mic_test_active {
            MicTestState::Idle
        } else if self.effective_mute() {
            MicTestState::Muted
        } else if self.mic_test_has_sample && self.mic_test_level_db <= -95 {
            MicTestState::NoSignal
        } else {
            MicTestState::Testing
        }
    }

    /// Запущена ли проверка микрофона пользователем.
    pub fn mic_test_active(&self) -> bool {
        self.mic_test_active
    }

    /// Текущий уровень сигнала в проверке микрофона, дБ.
    pub fn mic_test_level_db(&self) -> i8 {
        self.mic_test_level_db
    }

    /// Явный запуск проверки микрофона пользователем (`VOICE-DEVICES-SETTINGS-SPEC.md` §3.7).
    pub fn start_mic_test(&mut self) -> bool {
        if self.selected_input.is_none() || self.input_devices().is_empty() {
            return false;
        }
        self.mic_test_active = true;
        self.mic_test_level_db = DEFAULT_MIC_TEST_LEVEL_DB;
        self.mic_test_has_sample = false;
        let res = self
            .service
            .as_ref()
            .map_or(Ok(()), |svc| svc.start_mic_test());
        self.handle_service_result(res);
        true
    }

    /// Остановка проверки микрофона и освобождение тестового захвата.
    pub fn stop_mic_test(&mut self) {
        let was_active = self.mic_test_active;
        self.mic_test_active = false;
        self.mic_test_level_db = DEFAULT_MIC_TEST_LEVEL_DB;
        self.mic_test_has_sample = false;
        if was_active {
            let res = self
                .service
                .as_ref()
                .map_or(Ok(()), |svc| svc.stop_mic_test());
            self.handle_service_result(res);
        }
    }

    /// Передаёт очередной замер уровня входа (в дБ) в индикатор проверки микрофона.
    pub fn feed_mic_test_sample(&mut self, level_db: i8) {
        if !self.mic_test_active {
            return;
        }
        self.mic_test_level_db = level_db.clamp(MIN_SENSITIVITY_DB, MAX_SENSITIVITY_DB);
        self.mic_test_has_sample = true;
    }

    /// Проекция индикатора `#voice-mic-test-meter` и кнопки `#voice-mic-test-btn`.
    pub fn mic_test_meter_view(&self) -> MicTestMeterView {
        let state = self.mic_test_state();
        let threshold_percent = threshold_db_to_sensitivity_percent(self.vad_threshold_db);
        let level_db = if self.mic_test_active && !self.effective_mute() {
            self.mic_test_level_db
        } else {
            DEFAULT_MIC_TEST_LEVEL_DB
        };
        let fill_percent = match state {
            MicTestState::Idle | MicTestState::Muted | MicTestState::NoSignal => 0,
            MicTestState::Testing => threshold_db_to_sensitivity_percent(level_db),
        };
        let tone = if level_db >= self.vad_threshold_db {
            "positive".to_owned()
        } else {
            "warning".to_owned()
        };
        let button_text = if self.mic_test_active {
            "Остановить проверку".to_owned()
        } else {
            "Проверка микрофона".to_owned()
        };
        let aria_disabled = self.effective_mute() || self.input_devices().is_empty();
        let status_text = match state {
            MicTestState::Muted => "Микрофон отключён".to_owned(),
            MicTestState::NoSignal => "Discord не может обнаружить звук с вашего микрофона. Проверьте, выбран ли правильный микрофон и не отключён ли он на уровне системы.".to_owned(),
            MicTestState::Testing => format!("Уровень входа: {level_db} дБ"),
            MicTestState::Idle => String::new(),
        };

        MicTestMeterView {
            level_db,
            fill_percent,
            threshold_percent,
            tone,
            aria_valuetext: format!("{level_db} дБ"),
            button_text,
            aria_pressed: self.mic_test_active,
            aria_disabled,
            status_text,
        }
    }

    /// Активное уведомление `#voice-devices-notice` (`None`, если баннер скрыт).
    pub fn notice(&self) -> Option<&VoiceNotice> {
        self.notice.as_ref()
    }
}
