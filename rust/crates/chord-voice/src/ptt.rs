//! Аппаратный обработчик Push-to-Talk (PTT), комбинаций скан-кодов и автомат микрофона.
//!
//! Оракулы RE (`/home/sanya/re-deep/`):
//! - `voice-deep.md` (§3–4): голосовой тракт, op5 Speaking (`MICROPHONE = 1`, `SOUNDSHARE = 2`, `PRIORITY = 4`).
//! - `native-re.md` (§5.2–5.4, `discord_utils.node` `src/input_events.cpp`, `src/input_events_linux.cpp`):
//!   `InputEventRegister`, `InputEventUnregister`, `InputEventServiceSetAllowed`, `InputSetFocused`,
//!   `OnSystemInputEvent`, `DeviceIdRegex = "^([0-9]+):([0-9]+)$"`, опции `keydown`/`keyup`/`focused`/`blurred`,
//!   лимит комбинации `MAX_KEYBIND_COMBO_LEN = 4`, таймаут захвата `5000` мс, запрет ЛКМ (код 1) и
//!   одиночных буквенных скан-кодов `[24..=33]`, `[38..=46]`, `[52..=58]` на Linux.
//! - `native-re/raw/chord_voice/nm-all-demangled.txt` и `rabin2-strings.txt`:
//!   `discord::PttKeyState`, `discord::AudioModePTT`, `discord::AudioModeType` (`MODE_UNINIT`, `MODE_VAD`, `MODE_PTT`),
//!   `LocalUser::UpdateMuteState`, `LocalUser::UpdateSpeaking`, `SetOnSpeakingWhileMutedCallback`,
//!   `SetNoAudioInputThreshold`.
//! - `waves/RW-40/QUALITY/G5-VOICE-PLAN.md` (§3.3, §6): `org.freedesktop.portal.GlobalShortcuts`
//!   (`Activated`/`Deactivated`) и индикация `PttAccessStatus::Blocked` без фокуса окна при недоступном портале.

use crate::SpeakingFlags;
use std::collections::{BTreeMap, BTreeSet};

/// Минимальная задержка отпускания Push-to-Talk (0 мс).
pub const MIN_PTT_RELEASE_DELAY_MS: u32 = 0;

/// Максимальная задержка отпускания Push-to-Talk в настройках клиента (2000 мс).
pub const MAX_PTT_RELEASE_DELAY_MS: u32 = 2_000;

/// Задержка отпускания Push-to-Talk по умолчанию в UI (200 мс).
pub const DEFAULT_PTT_RELEASE_DELAY_MS: u32 = 200;

/// Внутренняя задержка отпускания движка по умолчанию (`pttReleaseDelay` в `rabin2-strings.txt`).
pub const ENGINE_DEFAULT_PTT_RELEASE_DELAY_MS: u32 = 20;

/// Максимальное число скан-кодов в одной комбинации (`native-re.md` §5.4).
pub const MAX_KEYBIND_COMBO_LEN: usize = 4;

/// Таймаут захвата комбинации клавиш в мс (`native-re.md` §5.4).
pub const KEYBIND_CAPTURE_TIMEOUT_MS: u64 = 5_000;

/// Запрещённая левая кнопка мыши на Linux (`native-re.md` §5.4).
pub const LINUX_LEFT_MOUSE_BUTTON_CODE: u16 = 1;

/// Диапазоны скан-кодов X11/evdev буквенных рядов QWERTY на Linux, запрещённые как одиночный бинд без модификатора.
pub const LINUX_FORBIDDEN_SINGLE_KEY_RANGES: [(u16, u16); 3] = [(24, 33), (38, 46), (52, 58)];

/// Порог отсутствия аудиосигнала микрофона по умолчанию (`SetNoAudioInputThreshold`, дБ полной шкалы).
pub const DEFAULT_NO_AUDIO_INPUT_THRESHOLD_DB: i8 = -90;

/// Число подряд идущих кадров тишины (по 20 мс = 5 с) до предупреждения `NoAudioInput`.
pub const DEFAULT_NO_AUDIO_INPUT_SILENCE_FRAMES: u32 = 250;

/// Режим аудиовхода движка (`discord::AudioModeType` в `rabin2-strings.txt:1634-1637`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioModeType {
    ModeUninit,
    ModeVad,
    ModePtt,
}

impl AudioModeType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ModeUninit => "MODE_UNINIT",
            Self::ModeVad => "MODE_VAD",
            Self::ModePtt => "MODE_PTT",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "MODE_UNINIT" => Some(Self::ModeUninit),
            "MODE_VAD" => Some(Self::ModeVad),
            "MODE_PTT" => Some(Self::ModePtt),
            _ => None,
        }
    }
}

/// Состояние активации PTT (`discord::PttKeyState::GetActivationState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PttActivationState {
    Inactive,
    Active,
    ReleaseHangover { remaining_ms: u32 },
}

impl PttActivationState {
    #[must_use]
    pub const fn is_transmitting(self) -> bool {
        matches!(self, Self::Active | Self::ReleaseHangover { .. })
    }
}

/// Статус доступности глобального PTT (`G5-VOICE-PLAN.md` §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PttAccessStatus {
    Active,
    Blocked,
}

/// Тип аппаратного устройства ввода (`discord_utils` `OnSystemInputEvent`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InputDeviceType {
    Keyboard,
    Mouse,
    Gamepad,
}

impl InputDeviceType {
    #[must_use]
    pub const fn from_code(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::Keyboard),
            1 => Some(Self::Mouse),
            2 => Some(Self::Gamepad),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_code(self) -> u8 {
        match self {
            Self::Keyboard => 0,
            Self::Mouse => 1,
            Self::Gamepad => 2,
        }
    }
}

/// Идентификатор устройства ввода формата `^([0-9]+):([0-9]+)$` (`discord_utils` `rabin2-strings.txt:0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputDeviceId {
    pub vendor_or_bus: u32,
    pub product_or_dev: u32,
}

impl InputDeviceId {
    pub fn parse(raw: &str) -> Result<Self, KeybindError> {
        let Some((left, right)) = raw.split_once(':') else {
            return Err(KeybindError::InvalidDeviceId);
        };
        if left.is_empty()
            || right.is_empty()
            || !left.bytes().all(|b| b.is_ascii_digit())
            || !right.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(KeybindError::InvalidDeviceId);
        }
        let vendor_or_bus = left
            .parse::<u32>()
            .map_err(|_| KeybindError::InvalidDeviceId)?;
        let product_or_dev = right
            .parse::<u32>()
            .map_err(|_| KeybindError::InvalidDeviceId)?;
        Ok(Self {
            vendor_or_bus,
            product_or_dev,
        })
    }

    #[must_use]
    pub fn format_id(self) -> String {
        format!("{}:{}", self.vendor_or_bus, self.product_or_dev)
    }
}

/// Элемент комбинации скан-кодов (`ScanCodeEntry`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanCodeEntry {
    pub device_type: InputDeviceType,
    pub scan_code: u16,
    pub is_modifier: bool,
}

impl ScanCodeEntry {
    #[must_use]
    pub const fn keyboard(scan_code: u16, is_modifier: bool) -> Self {
        Self {
            device_type: InputDeviceType::Keyboard,
            scan_code,
            is_modifier,
        }
    }

    #[must_use]
    pub const fn mouse(button: u16) -> Self {
        Self {
            device_type: InputDeviceType::Mouse,
            scan_code: button,
            is_modifier: false,
        }
    }

    #[must_use]
    pub const fn gamepad(button: u16) -> Self {
        Self {
            device_type: InputDeviceType::Gamepad,
            scan_code: button,
            is_modifier: false,
        }
    }
}

/// Валидированная комбинация скан-кодов (`1..=4` элементов).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanCodeCombo {
    entries: Vec<ScanCodeEntry>,
}

impl ScanCodeCombo {
    pub fn new(entries: Vec<ScanCodeEntry>) -> Result<Self, KeybindError> {
        if entries.is_empty() {
            return Err(KeybindError::EmptyCombo);
        }
        if entries.len() > MAX_KEYBIND_COMBO_LEN {
            return Err(KeybindError::ComboTooLong {
                len: entries.len(),
                max: MAX_KEYBIND_COMBO_LEN,
            });
        }
        let mut seen = BTreeSet::new();
        for entry in &entries {
            if entry.device_type == InputDeviceType::Mouse
                && entry.scan_code == LINUX_LEFT_MOUSE_BUTTON_CODE
            {
                return Err(KeybindError::ForbiddenLeftMouseButton);
            }
            if !seen.insert((entry.device_type, entry.scan_code)) {
                return Err(KeybindError::DuplicateScanCode {
                    scan_code: entry.scan_code,
                });
            }
        }
        if entries.len() == 1 {
            let single = entries[0];
            if single.device_type == InputDeviceType::Keyboard
                && !single.is_modifier
                && Self::is_linux_forbidden_single_key(single.scan_code)
            {
                return Err(KeybindError::ForbiddenSingleCharacterKey {
                    scan_code: single.scan_code,
                });
            }
        }
        Ok(Self { entries })
    }

    #[must_use]
    pub fn entries(&self) -> &[ScanCodeEntry] {
        &self.entries
    }

    #[must_use]
    pub fn is_linux_forbidden_single_key(scan_code: u16) -> bool {
        LINUX_FORBIDDEN_SINGLE_KEY_RANGES
            .iter()
            .any(|&(start, end)| scan_code >= start && scan_code <= end)
    }
}

/// Опции срабатывания бинда (`discord_utils` `rabin2-strings.txt:62-65`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeybindEventOptions {
    pub keydown: bool,
    pub keyup: bool,
    pub focused: bool,
    pub blurred: bool,
}

impl KeybindEventOptions {
    #[must_use]
    pub const fn ptt_default() -> Self {
        Self {
            keydown: true,
            keyup: true,
            focused: true,
            blurred: true,
        }
    }

    #[must_use]
    pub const fn focused_only() -> Self {
        Self {
            keydown: true,
            keyup: true,
            focused: true,
            blurred: false,
        }
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        (self.keydown || self.keyup) && (self.focused || self.blurred)
    }
}

/// Действия аппаратных горячих клавиш голоса.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareKeybindAction {
    PushToTalk,
    PushToTalkPriority,
    PushToMute,
    VadPriority,
    ToggleMute,
    ToggleDeafen,
}

impl HardwareKeybindAction {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PushToTalk => "PUSH_TO_TALK",
            Self::PushToTalkPriority => "PUSH_TO_TALK_PRIORITY",
            Self::PushToMute => "PUSH_TO_MUTE",
            Self::VadPriority => "VAD_PRIORITY",
            Self::ToggleMute => "TOGGLE_MUTE",
            Self::ToggleDeafen => "TOGGLE_DEAFEN",
        }
    }
}

/// Ошибки регистрации и валидации комбинаций клавиш.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeybindError {
    EmptyCombo,
    ComboTooLong { len: usize, max: usize },
    ForbiddenLeftMouseButton,
    ForbiddenSingleCharacterKey { scan_code: u16 },
    DuplicateScanCode { scan_code: u16 },
    InvalidDeviceId,
    InvalidEventOptions,
    DuplicateEventId { event_id: u32 },
    UnknownEventId { event_id: u32 },
    ServiceDisallowed,
}

/// Событие срабатывания зарегистрированного бинда.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeybindFiredEvent {
    pub event_id: u32,
    pub action: HardwareKeybindAction,
    pub is_down: bool,
    pub timestamp_ms: u64,
    pub via_portal: bool,
}

/// Состояние клавиши PTT (`discord::PttKeyState` в `src/audio_mode_ptt.cpp`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PttKeyState {
    release_delay_ms: u32,
    key_down: bool,
    latched: bool,
    wants_priority: bool,
    press_count: u64,
    last_release_ms: Option<u64>,
}

impl PttKeyState {
    #[must_use]
    pub fn new(release_delay_ms: u32) -> Self {
        Self {
            release_delay_ms: release_delay_ms
                .clamp(MIN_PTT_RELEASE_DELAY_MS, MAX_PTT_RELEASE_DELAY_MS),
            key_down: false,
            latched: false,
            wants_priority: false,
            press_count: 0,
            last_release_ms: None,
        }
    }

    #[must_use]
    pub const fn release_delay_ms(&self) -> u32 {
        self.release_delay_ms
    }

    pub fn set_release_delay_ms(&mut self, delay_ms: u32) -> u32 {
        self.release_delay_ms = delay_ms.clamp(MIN_PTT_RELEASE_DELAY_MS, MAX_PTT_RELEASE_DELAY_MS);
        self.release_delay_ms
    }

    pub fn set_key_state(&mut self, active: bool, priority: bool, now_ms: u64) {
        if active {
            if !self.key_down {
                self.press_count = self.press_count.saturating_add(1);
            }
            self.key_down = true;
            self.wants_priority = priority;
            self.last_release_ms = None;
        } else if self.key_down {
            self.key_down = false;
            self.last_release_ms = Some(now_ms);
        }
    }

    pub fn set_wants_priority(&mut self, priority: bool) {
        self.wants_priority = priority;
    }

    #[must_use]
    pub const fn wants_priority(&self) -> bool {
        self.wants_priority
    }

    #[must_use]
    pub const fn press_count(&self) -> u64 {
        self.press_count
    }

    #[must_use]
    pub const fn is_key_down(&self) -> bool {
        self.key_down
    }

    pub fn latch(&mut self, latched: bool, priority: bool, now_ms: u64) {
        if latched {
            self.latched = true;
            self.wants_priority = priority;
            self.last_release_ms = None;
        } else if self.latched {
            self.latched = false;
            if !self.key_down {
                self.last_release_ms = Some(now_ms);
            }
        }
    }

    #[must_use]
    pub const fn is_latched(&self) -> bool {
        self.latched
    }

    pub fn clear_release_tail(&mut self) {
        self.key_down = false;
        self.latched = false;
        self.last_release_ms = None;
    }

    #[must_use]
    pub fn activation_state(&self, now_ms: u64) -> PttActivationState {
        if self.key_down || self.latched {
            return PttActivationState::Active;
        }
        if self.release_delay_ms == 0 {
            return PttActivationState::Inactive;
        }
        let Some(release_ms) = self.last_release_ms else {
            return PttActivationState::Inactive;
        };
        let elapsed = now_ms.saturating_sub(release_ms);
        let delay = u64::from(self.release_delay_ms);
        if elapsed <= delay {
            let remaining_ms = u32::try_from(delay.saturating_sub(elapsed)).unwrap_or(0);
            PttActivationState::ReleaseHangover { remaining_ms }
        } else {
            PttActivationState::Inactive
        }
    }

    #[must_use]
    pub fn speaking_flags(&self, now_ms: u64, priority_speaker_allowed: bool) -> SpeakingFlags {
        if !self.activation_state(now_ms).is_transmitting() {
            return SpeakingFlags::NONE;
        }
        if self.wants_priority && priority_speaker_allowed {
            SpeakingFlags::MICROPHONE.union(SpeakingFlags::PRIORITY)
        } else {
            SpeakingFlags::MICROPHONE
        }
    }
}

#[derive(Debug, Clone)]
struct RegisteredBinding {
    event_id: u32,
    action: HardwareKeybindAction,
    combo: ScanCodeCombo,
    options: KeybindEventOptions,
    portal_shortcut_id: Option<String>,
    currently_active: bool,
}

/// Обработчик аппаратных комбинаций клавиш и XDG Desktop Portal `GlobalShortcuts`.
#[derive(Debug, Clone)]
pub struct HardwarePttKeybindHandler {
    service_allowed: bool,
    window_focused: bool,
    global_shortcuts_available: bool,
    pressed_inputs: BTreeSet<(InputDeviceType, u16)>,
    bindings: BTreeMap<u32, RegisteredBinding>,
}

impl Default for HardwarePttKeybindHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl HardwarePttKeybindHandler {
    #[must_use]
    pub fn new() -> Self {
        Self {
            service_allowed: true,
            window_focused: true,
            global_shortcuts_available: false,
            pressed_inputs: BTreeSet::new(),
            bindings: BTreeMap::new(),
        }
    }

    pub fn register_keybind(
        &mut self,
        event_id: u32,
        action: HardwareKeybindAction,
        combo: ScanCodeCombo,
        options: KeybindEventOptions,
        portal_shortcut_id: Option<&str>,
    ) -> Result<(), KeybindError> {
        if !options.is_valid() {
            return Err(KeybindError::InvalidEventOptions);
        }
        if self.bindings.contains_key(&event_id) {
            return Err(KeybindError::DuplicateEventId { event_id });
        }
        self.bindings.insert(
            event_id,
            RegisteredBinding {
                event_id,
                action,
                combo,
                options,
                portal_shortcut_id: portal_shortcut_id.map(str::to_owned),
                currently_active: false,
            },
        );
        Ok(())
    }

    pub fn unregister_keybind(&mut self, event_id: u32) -> Result<(), KeybindError> {
        if self.bindings.remove(&event_id).is_some() {
            Ok(())
        } else {
            Err(KeybindError::UnknownEventId { event_id })
        }
    }

    pub fn unregister_all(&mut self) {
        self.bindings.clear();
        self.pressed_inputs.clear();
    }

    #[must_use]
    pub fn registered_event_ids(&self) -> Vec<u32> {
        self.bindings.keys().copied().collect()
    }

    pub fn set_service_allowed(&mut self, allowed: bool) {
        self.service_allowed = allowed;
        if !allowed {
            self.pressed_inputs.clear();
            for binding in self.bindings.values_mut() {
                binding.currently_active = false;
            }
        }
    }

    #[must_use]
    pub const fn is_service_allowed(&self) -> bool {
        self.service_allowed
    }

    pub fn set_window_focused(&mut self, focused: bool) {
        self.window_focused = focused;
        if !focused && !self.global_shortcuts_available {
            self.pressed_inputs.clear();
            for binding in self.bindings.values_mut() {
                binding.currently_active = false;
            }
        }
    }

    #[must_use]
    pub const fn is_window_focused(&self) -> bool {
        self.window_focused
    }

    pub fn set_global_shortcuts_available(&mut self, available: bool) {
        self.global_shortcuts_available = available;
    }

    #[must_use]
    pub const fn is_global_shortcuts_available(&self) -> bool {
        self.global_shortcuts_available
    }

    #[must_use]
    pub const fn access_status(&self) -> PttAccessStatus {
        if self.service_allowed && (self.window_focused || self.global_shortcuts_available) {
            PttAccessStatus::Active
        } else {
            PttAccessStatus::Blocked
        }
    }

    pub fn on_system_input_event(
        &mut self,
        device_type: InputDeviceType,
        scan_code: u16,
        is_down: bool,
        timestamp_ms: u64,
    ) -> Vec<KeybindFiredEvent> {
        if self.access_status() == PttAccessStatus::Blocked {
            return Vec::new();
        }
        if is_down {
            self.pressed_inputs.insert((device_type, scan_code));
        } else {
            self.pressed_inputs.remove(&(device_type, scan_code));
        }

        let mut fired = Vec::new();
        for binding in self.bindings.values_mut() {
            let focus_allowed = if self.window_focused {
                binding.options.focused
            } else {
                binding.options.blurred
            };
            if !focus_allowed {
                continue;
            }
            let all_pressed = binding.combo.entries().iter().all(|entry| {
                self.pressed_inputs
                    .contains(&(entry.device_type, entry.scan_code))
            });
            if all_pressed && !binding.currently_active {
                binding.currently_active = true;
                if binding.options.keydown {
                    fired.push(KeybindFiredEvent {
                        event_id: binding.event_id,
                        action: binding.action,
                        is_down: true,
                        timestamp_ms,
                        via_portal: false,
                    });
                }
            } else if !all_pressed && binding.currently_active {
                binding.currently_active = false;
                if binding.options.keyup {
                    fired.push(KeybindFiredEvent {
                        event_id: binding.event_id,
                        action: binding.action,
                        is_down: false,
                        timestamp_ms,
                        via_portal: false,
                    });
                }
            }
        }
        fired
    }

    pub fn on_portal_shortcut(
        &mut self,
        shortcut_id: &str,
        activated: bool,
        timestamp_ms: u64,
    ) -> Vec<KeybindFiredEvent> {
        if !self.service_allowed || !self.global_shortcuts_available || shortcut_id.is_empty() {
            return Vec::new();
        }
        let mut fired = Vec::new();
        for binding in self.bindings.values_mut() {
            if binding.portal_shortcut_id.as_deref() != Some(shortcut_id) {
                continue;
            }
            if activated && !binding.currently_active {
                binding.currently_active = true;
                if binding.options.keydown {
                    fired.push(KeybindFiredEvent {
                        event_id: binding.event_id,
                        action: binding.action,
                        is_down: true,
                        timestamp_ms,
                        via_portal: true,
                    });
                }
            } else if !activated && binding.currently_active {
                binding.currently_active = false;
                if binding.options.keyup {
                    fired.push(KeybindFiredEvent {
                        event_id: binding.event_id,
                        action: binding.action,
                        is_down: false,
                        timestamp_ms,
                        via_portal: true,
                    });
                }
            }
        }
        fired
    }
}

/// Состояния конечного автомата микрофона (`LocalUser` + `AudioModePTT` + `AudioModeVAD`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicCaptureState {
    Uninitialized,
    DeviceUnavailable,
    Muted,
    SpeakingWhileMuted,
    GateClosed,
    Speaking,
    PrioritySpeaking,
    PttReleaseHangover,
    NoAudioInput,
}

/// Результат оценки кадра микрофона.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MicFrameOutput {
    pub state: MicCaptureState,
    pub should_transmit: bool,
    pub speaking_flags: SpeakingFlags,
    pub speaking_changed: bool,
    pub speaking_while_muted: bool,
    pub no_audio_input_warning: bool,
}

/// Конечный автомат состояния микрофона (`LocalUser` в `src/media/local_user.cpp`).
#[derive(Debug, Clone)]
pub struct MicStateMachine {
    audio_mode: AudioModeType,
    state: MicCaptureState,
    has_input_device: bool,
    self_mute: bool,
    self_deaf: bool,
    server_mute: bool,
    suppress: bool,
    push_to_mute_active: bool,
    vad_priority_active: bool,
    priority_speaker_permission: bool,
    ptt_key_state: PttKeyState,
    last_speaking_flags: SpeakingFlags,
    no_input_threshold_db: i8,
    no_input_silence_frames_limit: u32,
    consecutive_silent_frames: u32,
}

impl MicStateMachine {
    #[must_use]
    pub fn new(mode: AudioModeType, release_delay_ms: u32) -> Self {
        let initial_state = if mode == AudioModeType::ModeUninit {
            MicCaptureState::Uninitialized
        } else {
            MicCaptureState::GateClosed
        };
        Self {
            audio_mode: mode,
            state: initial_state,
            has_input_device: true,
            self_mute: false,
            self_deaf: false,
            server_mute: false,
            suppress: false,
            push_to_mute_active: false,
            vad_priority_active: false,
            priority_speaker_permission: false,
            ptt_key_state: PttKeyState::new(release_delay_ms),
            last_speaking_flags: SpeakingFlags::NONE,
            no_input_threshold_db: DEFAULT_NO_AUDIO_INPUT_THRESHOLD_DB,
            no_input_silence_frames_limit: DEFAULT_NO_AUDIO_INPUT_SILENCE_FRAMES,
            consecutive_silent_frames: 0,
        }
    }

    #[must_use]
    pub const fn audio_mode(&self) -> AudioModeType {
        self.audio_mode
    }

    pub fn set_audio_mode(&mut self, mode: AudioModeType) {
        self.audio_mode = mode;
        if mode != AudioModeType::ModePtt {
            self.ptt_key_state.clear_release_tail();
        }
    }

    #[must_use]
    pub const fn state(&self) -> MicCaptureState {
        self.state
    }

    pub fn set_has_input_device(&mut self, available: bool) {
        self.has_input_device = available;
        if !available {
            self.ptt_key_state.clear_release_tail();
        }
    }

    #[must_use]
    pub const fn has_input_device(&self) -> bool {
        self.has_input_device
    }

    pub fn set_self_mute(&mut self, muted: bool) {
        self.self_mute = muted;
        if muted {
            self.ptt_key_state.clear_release_tail();
        }
    }

    #[must_use]
    pub const fn is_self_mute(&self) -> bool {
        self.self_mute
    }

    pub fn set_self_deaf(&mut self, deafened: bool) {
        self.self_deaf = deafened;
        if deafened {
            self.ptt_key_state.clear_release_tail();
        }
    }

    #[must_use]
    pub const fn is_self_deaf(&self) -> bool {
        self.self_deaf
    }

    pub fn set_server_mute(&mut self, server_muted: bool) {
        self.server_mute = server_muted;
        if server_muted {
            self.ptt_key_state.clear_release_tail();
        }
    }

    #[must_use]
    pub const fn is_server_mute(&self) -> bool {
        self.server_mute
    }

    pub fn set_suppress(&mut self, suppressed: bool) {
        self.suppress = suppressed;
        if suppressed {
            self.ptt_key_state.clear_release_tail();
        }
    }

    #[must_use]
    pub const fn is_suppressed(&self) -> bool {
        self.suppress
    }

    #[must_use]
    pub const fn is_effectively_muted(&self) -> bool {
        self.self_mute
            || self.self_deaf
            || self.server_mute
            || self.suppress
            || self.push_to_mute_active
    }

    pub fn set_priority_speaker_permission(&mut self, allowed: bool) {
        self.priority_speaker_permission = allowed;
    }

    #[must_use]
    pub const fn has_priority_speaker_permission(&self) -> bool {
        self.priority_speaker_permission
    }

    pub fn set_vad_priority_active(&mut self, active: bool) {
        self.vad_priority_active = active;
    }

    pub fn set_push_to_mute_active(&mut self, active: bool) {
        self.push_to_mute_active = active;
        if active {
            self.ptt_key_state.clear_release_tail();
        }
    }

    #[must_use]
    pub const fn ptt_key_state(&self) -> &PttKeyState {
        &self.ptt_key_state
    }

    pub fn ptt_key_state_mut(&mut self) -> &mut PttKeyState {
        &mut self.ptt_key_state
    }

    pub fn apply_keybind_event(&mut self, event: &KeybindFiredEvent) {
        match event.action {
            HardwareKeybindAction::PushToTalk => {
                self.ptt_key_state
                    .set_key_state(event.is_down, false, event.timestamp_ms);
            }
            HardwareKeybindAction::PushToTalkPriority => {
                self.ptt_key_state
                    .set_key_state(event.is_down, true, event.timestamp_ms);
            }
            HardwareKeybindAction::PushToMute => {
                self.set_push_to_mute_active(event.is_down);
            }
            HardwareKeybindAction::VadPriority => {
                self.set_vad_priority_active(event.is_down);
            }
            HardwareKeybindAction::ToggleMute => {
                if event.is_down {
                    self.set_self_mute(!self.self_mute);
                }
            }
            HardwareKeybindAction::ToggleDeafen => {
                if event.is_down {
                    self.set_self_deaf(!self.self_deaf);
                }
            }
        }
    }

    pub fn evaluate_frame(
        &mut self,
        now_ms: u64,
        voice_detected: bool,
        level_dbfs: i8,
    ) -> MicFrameOutput {
        if !self.has_input_device {
            let speaking_changed = self.last_speaking_flags != SpeakingFlags::NONE;
            self.last_speaking_flags = SpeakingFlags::NONE;
            self.state = MicCaptureState::DeviceUnavailable;
            return MicFrameOutput {
                state: self.state,
                should_transmit: false,
                speaking_flags: SpeakingFlags::NONE,
                speaking_changed,
                speaking_while_muted: false,
                no_audio_input_warning: false,
            };
        }

        if self.audio_mode == AudioModeType::ModeUninit {
            let speaking_changed = self.last_speaking_flags != SpeakingFlags::NONE;
            self.last_speaking_flags = SpeakingFlags::NONE;
            self.state = MicCaptureState::Uninitialized;
            return MicFrameOutput {
                state: self.state,
                should_transmit: false,
                speaking_flags: SpeakingFlags::NONE,
                speaking_changed,
                speaking_while_muted: false,
                no_audio_input_warning: false,
            };
        }

        if self.is_effectively_muted() {
            self.ptt_key_state.clear_release_tail();
            self.consecutive_silent_frames = 0;
            let speaking_changed = self.last_speaking_flags != SpeakingFlags::NONE;
            self.last_speaking_flags = SpeakingFlags::NONE;
            if voice_detected {
                self.state = MicCaptureState::SpeakingWhileMuted;
                return MicFrameOutput {
                    state: self.state,
                    should_transmit: false,
                    speaking_flags: SpeakingFlags::NONE,
                    speaking_changed,
                    speaking_while_muted: true,
                    no_audio_input_warning: false,
                };
            }
            self.state = MicCaptureState::Muted;
            return MicFrameOutput {
                state: self.state,
                should_transmit: false,
                speaking_flags: SpeakingFlags::NONE,
                speaking_changed,
                speaking_while_muted: false,
                no_audio_input_warning: false,
            };
        }

        // Отслеживание предупреждения об отсутствии входного сигнала (`SetNoAudioInputThreshold`).
        if level_dbfs <= self.no_input_threshold_db && !voice_detected {
            self.consecutive_silent_frames = self.consecutive_silent_frames.saturating_add(1);
        } else {
            self.consecutive_silent_frames = 0;
        }
        let no_audio_input_warning =
            self.consecutive_silent_frames >= self.no_input_silence_frames_limit;

        let (state, speaking_flags) = match self.audio_mode {
            AudioModeType::ModeUninit => (MicCaptureState::Uninitialized, SpeakingFlags::NONE),
            AudioModeType::ModePtt => {
                let act = self.ptt_key_state.activation_state(now_ms);
                let flags = self
                    .ptt_key_state
                    .speaking_flags(now_ms, self.priority_speaker_permission);
                match act {
                    PttActivationState::Active => {
                        if flags.contains(SpeakingFlags::PRIORITY) {
                            (MicCaptureState::PrioritySpeaking, flags)
                        } else {
                            (MicCaptureState::Speaking, flags)
                        }
                    }
                    PttActivationState::ReleaseHangover { .. } => {
                        (MicCaptureState::PttReleaseHangover, flags)
                    }
                    PttActivationState::Inactive => {
                        if no_audio_input_warning {
                            (MicCaptureState::NoAudioInput, SpeakingFlags::NONE)
                        } else {
                            (MicCaptureState::GateClosed, SpeakingFlags::NONE)
                        }
                    }
                }
            }
            AudioModeType::ModeVad => {
                if voice_detected {
                    let flags = if self.vad_priority_active && self.priority_speaker_permission {
                        SpeakingFlags::MICROPHONE.union(SpeakingFlags::PRIORITY)
                    } else {
                        SpeakingFlags::MICROPHONE
                    };
                    let st = if flags.contains(SpeakingFlags::PRIORITY) {
                        MicCaptureState::PrioritySpeaking
                    } else {
                        MicCaptureState::Speaking
                    };
                    (st, flags)
                } else if no_audio_input_warning {
                    (MicCaptureState::NoAudioInput, SpeakingFlags::NONE)
                } else {
                    (MicCaptureState::GateClosed, SpeakingFlags::NONE)
                }
            }
        };

        let should_transmit = !speaking_flags.is_empty();
        let speaking_changed = self.last_speaking_flags != speaking_flags;
        self.last_speaking_flags = speaking_flags;
        self.state = state;

        MicFrameOutput {
            state,
            should_transmit,
            speaking_flags,
            speaking_changed,
            speaking_while_muted: false,
            no_audio_input_warning,
        }
    }
}
