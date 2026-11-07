//! RW-40 / V-07-VOICE-DEVICES-CONTRACT: детерминированная модель аудио-устройств,
//! hot-plug с автоматическим откатом на системное default-устройство,
//! инвариант mute/deafen, режимы активации (VoiceActivity / PushToTalk) и регулировка громкости.
//!
//! Оракул и спецификация:
//! - `waves/RW-40/PARITY/PARITY-MATRIX.md` строка `V-07` (`F162:63-83` · `[M]`);
//! - `waves/RW-40/DESIGN/VOICE.md` §2.4, §3.1–§3.5 и `waves/RW-40/DESIGN/voice-spec.json`
//!   (`sliders`: `OX2Bnr` 0..=100 = 100, `aUJ062` 0..=200 = 100);
//! - `waves/RW-40/QUALITY/G5-VOICE-PLAN.md` §5.1, §5.4, §6, §7.4 (приватность меток устройств).

use std::fmt;

/// Минимальная громкость (0%).
pub const MIN_VOLUME: u16 = 0;

/// Максимальная громкость микрофона (`OX2Bnr`: 0..=100%).
pub const MAX_INPUT_VOLUME: u16 = 100;

/// Максимальная громкость вывода (`aUJ062`: 0..=200%).
pub const MAX_OUTPUT_VOLUME: u16 = 200;

/// Громкость микрофона по умолчанию (100%).
pub const DEFAULT_INPUT_VOLUME: u16 = 100;

/// Громкость вывода по умолчанию (100%).
pub const DEFAULT_OUTPUT_VOLUME: u16 = 100;

/// Порог активации по голосу по умолчанию (-40 дБ).
pub const DEFAULT_VAD_THRESHOLD_DB: i8 = -40;

/// Тип аудио-устройства.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioDeviceKind {
    /// Устройство ввода (микрофон).
    Input,
    /// Устройство вывода (динамики / наушники).
    Output,
}

/// Описание аудио-устройства ввода или вывода (`V-07`).
///
/// По правилу приватности (`G5-VOICE-PLAN.md` §5.1, §7.4 п.4) `Debug` скрывает
/// человекочитаемое имя устройства `label`, чтобы описания устройств ОС не утекали в логи.
#[derive(Clone, PartialEq, Eq)]
pub struct AudioDevice {
    /// Стабильный идентификатор устройства.
    pub id: String,
    /// Человекочитаемое имя устройства для отображения в UI.
    pub label: String,
    /// Направление потока (ввод или вывод).
    pub kind: AudioDeviceKind,
    /// Является ли устройство системным устройством по умолчанию для своего `kind`.
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

/// Уведомление об изменении активного устройства при hot-plug (`update_device_list`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceEvent {
    /// Выбранное устройство `previous_id` отключено; выполнен автоматический откат на `fallback_id`.
    DeviceFallback {
        kind: AudioDeviceKind,
        previous_id: String,
        fallback_id: String,
    },
    /// Для направления `kind` не осталось ни одного доступного устройства.
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
    /// Устройство с указанным `id` отсутствует в текущем списке.
    NotFound { id: String, kind: AudioDeviceKind },
    /// Устройство найдено, но имеет противоположный `AudioDeviceKind`.
    WrongKind {
        id: String,
        expected: AudioDeviceKind,
        actual: AudioDeviceKind,
    },
}

impl fmt::Display for DeviceSelectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => f.write_str("audio device id must not be empty"),
            Self::NotFound { id, kind } => {
                write!(f, "audio device '{id}' of kind {kind:?} not found")
            }
            Self::WrongKind {
                id,
                expected,
                actual,
            } => write!(
                f,
                "audio device '{id}' has kind {actual:?}, expected {expected:?}"
            ),
        }
    }
}

impl std::error::Error for DeviceSelectError {}

/// Режим активации передачи голоса (`V-07`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceInputMode {
    /// Активация по голосу (VAD): передача идёт при `vad_db >= threshold_db`.
    VoiceActivity { threshold_db: i8 },
    /// Режим рации (Push-to-Talk): передача идёт при `key_down == true`
    /// и в течение `release_delay_ms` после отпускания клавиши.
    PushToTalk { release_delay_ms: u32 },
}

impl Default for VoiceInputMode {
    fn default() -> Self {
        Self::VoiceActivity {
            threshold_db: DEFAULT_VAD_THRESHOLD_DB,
        }
    }
}

/// Детерминированное состояние аудио-устройств, mute/deafen, режима активации и громкости (`V-07`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceDevicesState {
    devices: Vec<AudioDevice>,
    selected_input: Option<String>,
    selected_output: Option<String>,
    user_mute: bool,
    self_deaf: bool,
    input_mode: VoiceInputMode,
    last_ptt_key_down_ms: Option<u64>,
    input_volume: u16,
    output_volume: u16,
}

impl Default for VoiceDevicesState {
    fn default() -> Self {
        Self::new()
    }
}

impl VoiceDevicesState {
    /// Создаёт пустое состояние со стандартной громкостью (100% / 100%) и режимом `VoiceActivity`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
            selected_input: None,
            selected_output: None,
            user_mute: false,
            self_deaf: false,
            input_mode: VoiceInputMode::default(),
            last_ptt_key_down_ms: None,
            input_volume: DEFAULT_INPUT_VOLUME,
            output_volume: DEFAULT_OUTPUT_VOLUME,
        }
    }

    /// Создаёт состояние и сразу инициализирует его списком устройств.
    #[must_use]
    pub fn with_devices(devices: Vec<AudioDevice>) -> Self {
        let mut state = Self::new();
        let _ = state.update_device_list(devices);
        state
    }

    /// Все известные устройства (в порядке перечисления).
    #[must_use]
    pub fn devices(&self) -> &[AudioDevice] {
        &self.devices
    }

    /// Доступные устройства ввода (микрофоны).
    #[must_use]
    pub fn input_devices(&self) -> Vec<&AudioDevice> {
        self.devices
            .iter()
            .filter(|d| d.kind == AudioDeviceKind::Input)
            .collect()
    }

    /// Доступные устройства вывода (динамики/наушники).
    #[must_use]
    pub fn output_devices(&self) -> Vec<&AudioDevice> {
        self.devices
            .iter()
            .filter(|d| d.kind == AudioDeviceKind::Output)
            .collect()
    }

    /// Идентификатор выбранного микрофона.
    #[must_use]
    pub fn selected_input(&self) -> Option<&str> {
        self.selected_input.as_deref()
    }

    /// Идентификатор выбранного устройства вывода.
    #[must_use]
    pub fn selected_output(&self) -> Option<&str> {
        self.selected_output.as_deref()
    }

    /// Ссылка на активное устройство ввода.
    #[must_use]
    pub fn active_input_device(&self) -> Option<&AudioDevice> {
        let id = self.selected_input.as_deref()?;
        self.devices
            .iter()
            .find(|d| d.kind == AudioDeviceKind::Input && d.id == id)
    }

    /// Ссылка на активное устройство вывода.
    #[must_use]
    pub fn active_output_device(&self) -> Option<&AudioDevice> {
        let id = self.selected_output.as_deref()?;
        self.devices
            .iter()
            .find(|d| d.kind == AudioDeviceKind::Output && d.id == id)
    }

    /// Явный выбор устройства ввода по `id`.
    pub fn select_input(&mut self, id: &str) -> Result<(), DeviceSelectError> {
        self.select_by_kind(id, AudioDeviceKind::Input)
    }

    /// Явный выбор устройства вывода по `id`.
    pub fn select_output(&mut self, id: &str) -> Result<(), DeviceSelectError> {
        self.select_by_kind(id, AudioDeviceKind::Output)
    }

    fn select_by_kind(
        &mut self,
        id: &str,
        expected: AudioDeviceKind,
    ) -> Result<(), DeviceSelectError> {
        if id.is_empty() {
            return Err(DeviceSelectError::EmptyId);
        }
        if let Some(device) = self
            .devices
            .iter()
            .find(|d| d.id == id && d.kind == expected)
        {
            match expected {
                AudioDeviceKind::Input => self.selected_input = Some(device.id.clone()),
                AudioDeviceKind::Output => self.selected_output = Some(device.id.clone()),
            }
            return Ok(());
        }
        if let Some(wrong) = self.devices.iter().find(|d| d.id == id) {
            return Err(DeviceSelectError::WrongKind {
                id: id.to_owned(),
                expected,
                actual: wrong.kind,
            });
        }
        Err(DeviceSelectError::NotFound {
            id: id.to_owned(),
            kind: expected,
        })
    }

    /// Обновляет список аудио-устройств при hot-plug.
    ///
    /// Инварианты:
    /// - если выбранное устройство всё ещё присутствует в новом списке для своего `kind`,
    ///   выбор сохраняется и уведомления не эмитятся;
    /// - если выбранное устройство исчезло, происходит автоматический откат на системное
    ///   `is_default == true` устройство (или первое доступное того же `kind`) с уведомлением
    ///   [`DeviceEvent::DeviceFallback`];
    /// - если для `kind` не осталось ни одного устройства, выбор сбрасывается в `None`
    ///   и возвращается [`DeviceEvent::NoDeviceAvailable`].
    pub fn update_device_list(&mut self, devices: Vec<AudioDevice>) -> Vec<DeviceEvent> {
        self.devices = devices;
        let mut events = Vec::new();
        Self::reconcile_kind(
            &self.devices,
            AudioDeviceKind::Input,
            &mut self.selected_input,
            &mut events,
        );
        Self::reconcile_kind(
            &self.devices,
            AudioDeviceKind::Output,
            &mut self.selected_output,
            &mut events,
        );
        events
    }

    fn reconcile_kind(
        devices: &[AudioDevice],
        kind: AudioDeviceKind,
        selected: &mut Option<String>,
        events: &mut Vec<DeviceEvent>,
    ) {
        if let Some(current_id) = selected.as_deref()
            && devices.iter().any(|d| d.kind == kind && d.id == current_id)
        {
            return;
        }

        let fallback = devices
            .iter()
            .find(|d| d.kind == kind && d.is_default)
            .or_else(|| devices.iter().find(|d| d.kind == kind));

        let previous = selected.take();
        match (previous, fallback) {
            (Some(previous_id), Some(next)) => {
                *selected = Some(next.id.clone());
                events.push(DeviceEvent::DeviceFallback {
                    kind,
                    previous_id,
                    fallback_id: next.id.clone(),
                });
            }
            (None, Some(next)) => {
                *selected = Some(next.id.clone());
            }
            (previous_id, None) => {
                events.push(DeviceEvent::NoDeviceAvailable { kind, previous_id });
            }
        }
    }

    /// Явный пользовательский флаг отключения микрофона (`user_mute`).
    #[must_use]
    pub fn user_mute(&self) -> bool {
        self.user_mute
    }

    /// Флаг полного отключения звука (`self_deaf`).
    #[must_use]
    pub fn self_deaf(&self) -> bool {
        self.self_deaf
    }

    /// Эффективный флаг `self_mute`: равен `true`, если включён `user_mute` ИЛИ `self_deaf`.
    #[must_use]
    pub fn effective_mute(&self) -> bool {
        self.user_mute || self.self_deaf
    }

    /// Кортеж `(self_mute, self_deaf)` для отправки в gateway op4 `VOICE_STATE_UPDATE`.
    #[must_use]
    pub fn gateway_mute_deaf(&self) -> (bool, bool) {
        (self.effective_mute(), self.self_deaf)
    }

    /// Устанавливает явный пользовательский mute (`user_mute`).
    ///
    /// Если эффективный mute становится `true`, активное удержание PTT сбрасывается.
    pub fn set_user_mute(&mut self, mute: bool) {
        self.user_mute = mute;
        if self.effective_mute() {
            self.last_ptt_key_down_ms = None;
        }
    }

    /// Устанавливает режим `self_deaf`.
    ///
    /// При включении `self_deaf = true` эффективный `self_mute` автоматически становится `true`,
    /// а при выключении `self_deaf = false` возвращается к явному `user_mute`.
    pub fn set_self_deaf(&mut self, deaf: bool) {
        self.self_deaf = deaf;
        if self.effective_mute() {
            self.last_ptt_key_down_ms = None;
        }
    }

    /// Текущий режим активации микрофона.
    #[must_use]
    pub fn input_mode(&self) -> VoiceInputMode {
        self.input_mode
    }

    /// Смена режима активации микрофона (сбрасывает хвост удержания PTT).
    pub fn set_input_mode(&mut self, mode: VoiceInputMode) {
        self.input_mode = mode;
        self.last_ptt_key_down_ms = None;
    }

    /// Расчёт признака активной передачи голоса в момент `now_ms`.
    ///
    /// Инварианты:
    /// - если `effective_mute() == true` или отсутствует выбранное устройство ввода,
    ///   передача **всегда** `false`;
    /// - в режиме [`VoiceInputMode::VoiceActivity`] передача активна при `vad_db >= threshold_db`;
    /// - в режиме [`VoiceInputMode::PushToTalk`] передача активна при `key_down == true`, а также
    ///   в окне `now_ms - last_key_down_ms <= release_delay_ms` после отпускания клавиши.
    pub fn is_transmitting(&mut self, now_ms: u64, key_down: bool, vad_db: i8) -> bool {
        if self.effective_mute() || self.selected_input.is_none() {
            self.last_ptt_key_down_ms = None;
            return false;
        }

        match self.input_mode {
            VoiceInputMode::VoiceActivity { threshold_db } => vad_db >= threshold_db,
            VoiceInputMode::PushToTalk { release_delay_ms } => {
                if key_down {
                    let prev = self.last_ptt_key_down_ms.unwrap_or(now_ms);
                    self.last_ptt_key_down_ms = Some(now_ms.max(prev));
                    return true;
                }
                let Some(last_down_ms) = self.last_ptt_key_down_ms else {
                    return false;
                };
                let elapsed_ms = now_ms.saturating_sub(last_down_ms);
                elapsed_ms <= u64::from(release_delay_ms)
            }
        }
    }

    /// Текущая громкость микрофона (`0..=100%`).
    #[must_use]
    pub fn input_volume(&self) -> u16 {
        self.input_volume
    }

    /// Текущая громкость вывода (`0..=200%`).
    #[must_use]
    pub fn output_volume(&self) -> u16 {
        self.output_volume
    }

    /// Устанавливает громкость микрофона с клампом в диапазон `0..=100%` и возвращает итоговое значение.
    pub fn set_input_volume(&mut self, percent: u16) -> u16 {
        let clamped = percent.clamp(MIN_VOLUME, MAX_INPUT_VOLUME);
        self.input_volume = clamped;
        clamped
    }

    /// Устанавливает громкость вывода с клампом в диапазон `0..=200%` и возвращает итоговое значение.
    pub fn set_output_volume(&mut self, percent: u16) -> u16 {
        let clamped = percent.clamp(MIN_VOLUME, MAX_OUTPUT_VOLUME);
        self.output_volume = clamped;
        clamped
    }
}

/// Максимальная длина имени бинарника/приложения в proplist PulseAudio/PipeWire (`Serein/crates/discord-voice/src/screen/audio_linux.rs:L94`).
pub const MAX_PROCESS_BINARY_LEN: usize = 256;

/// Максимальная длина десятичной строки PID (`u32::MAX` = 10 цифр, `audio_linux.rs:L109`).
pub const MAX_PID_STRING_LEN: usize = 10;

/// Фильтр захвата системного звука демонстрации экрана на Linux (`P1.4`).
///
/// Эталон: `Serein/crates/discord-voice/src/screen/audio_linux.rs` (`OwnApplication::allows`, строки 83–126).
/// Исключает собственный PID процесса (`application.process.id != std::process::id()`),
/// нулевой PID и собственные бинарники клиента, чтобы голоса зрителей звонка и микрофон
/// никогда не попадали обратно в поток демонстрации экрана (анти-эхо).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnApplicationFilter {
    own_pid: u32,
    own_binary: Option<String>,
}

impl OwnApplicationFilter {
    /// Создаёт фильтр для заданного собственного `own_pid`.
    #[must_use]
    pub const fn new(own_pid: u32) -> Self {
        Self {
            own_pid,
            own_binary: None,
        }
    }

    /// Создаёт фильтр с проверкой как по `own_pid`, так и по имени бинарника (для Flatpak / PID namespace).
    #[must_use]
    pub fn with_binary(own_pid: u32, own_binary: impl Into<String>) -> Self {
        Self {
            own_pid,
            own_binary: Some(own_binary.into()),
        }
    }

    /// Создаёт фильтр для текущего запущенного процесса (`std::process::id()`).
    #[must_use]
    pub fn current_process() -> Self {
        let own_binary = std::env::current_exe().ok().and_then(|path| {
            path.file_name()
                .and_then(|v| v.to_str())
                .filter(|v| !v.is_empty() && v.len() <= MAX_PROCESS_BINARY_LEN)
                .map(str::to_owned)
        });
        Self {
            own_pid: std::process::id(),
            own_binary,
        }
    }

    /// Возвращает PID собственного процесса.
    #[must_use]
    pub const fn own_pid(&self) -> u32 {
        self.own_pid
    }

    /// Возвращает имя бинарника собственного процесса, если задано.
    #[must_use]
    pub fn own_binary(&self) -> Option<&str> {
        self.own_binary.as_deref()
    }

    /// Проверяет, разрешён ли захват аудиопотока процесса `target_pid` при собственном PID `own_pid`.
    ///
    /// Возвращает `true`, только если оба PID ненулевые и `target_pid != own_pid`.
    #[must_use]
    pub const fn allows(target_pid: u32, own_pid: u32) -> bool {
        target_pid != 0 && own_pid != 0 && target_pid != own_pid
    }

    /// Проверяет `target_pid` относительно `self.own_pid`.
    #[must_use]
    pub const fn allows_pid(&self, target_pid: u32) -> bool {
        Self::allows(target_pid, self.own_pid)
    }

    /// Проверяет строковое свойство `application.process.id` из PulseAudio/PipeWire proplist.
    #[must_use]
    pub fn allows_pid_str(target_pid: Option<&str>, own_pid: u32) -> bool {
        let Some(pid_num) = target_pid
            .filter(|v| !v.is_empty() && v.len() <= MAX_PID_STRING_LEN)
            .and_then(|v| v.parse::<u32>().ok())
        else {
            return false;
        };
        Self::allows(pid_num, own_pid)
    }

    /// Полная проверка `pa_sink_input_info` proplist по эталону `Serein` (`audio_linux.rs:L101-125`).
    #[must_use]
    pub fn allows_proplist(
        &self,
        pid: Option<&str>,
        binary: Option<&str>,
        name: Option<&str>,
        app_id: Option<&str>,
    ) -> bool {
        if !Self::allows_pid_str(pid, self.own_pid) {
            return false;
        }
        let Some(binary) = binary.filter(|v| !v.is_empty() && v.len() <= MAX_PROCESS_BINARY_LEN)
        else {
            return false;
        };
        if let Some(ref own_binary) = self.own_binary
            && binary.eq_ignore_ascii_case(own_binary)
        {
            return false;
        }
        ![Some(binary), name, app_id].into_iter().flatten().any(|v| {
            let lower = v.to_ascii_lowercase();
            v.len() > MAX_PROCESS_BINARY_LEN
                || lower.contains("serein")
                || lower.contains("discord-lab")
                || lower.contains("discord-gui-spike")
                || v.eq_ignore_ascii_case("rustcord")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_application_filter_blocks_own_pid_and_zero_and_allows_other_processes() {
        let own_pid = std::process::id();
        assert!(!OwnApplicationFilter::allows(own_pid, own_pid));
        assert!(!OwnApplicationFilter::allows(0, own_pid));
        assert!(!OwnApplicationFilter::allows(4242, 0));
        assert!(OwnApplicationFilter::allows(
            own_pid.wrapping_add(1),
            own_pid
        ));

        let filter = OwnApplicationFilter::with_binary(1000, "discord-gui-spike");
        assert!(!filter.allows_pid(1000));
        assert!(!filter.allows_pid(0));
        assert!(filter.allows_pid(2048));

        // Проверка строкового application.process.id
        assert!(OwnApplicationFilter::allows_pid_str(Some("2048"), 1000));
        assert!(!OwnApplicationFilter::allows_pid_str(Some("1000"), 1000));
        assert!(!OwnApplicationFilter::allows_pid_str(Some("0"), 1000));
        assert!(!OwnApplicationFilter::allows_pid_str(None, 1000));
        assert!(!OwnApplicationFilter::allows_pid_str(
            Some("not-a-pid"),
            1000
        ));
        assert!(!OwnApplicationFilter::allows_pid_str(
            Some("12345678901"),
            1000
        ));

        // Полная проверка proplist (PID + binary + app_name/app_id)
        assert!(filter.allows_proplist(
            Some("2048"),
            Some("firefox"),
            Some("Mozilla Firefox"),
            Some("org.mozilla.firefox"),
        ));
        // Тот же PID -> запрет (анти-эхо)
        assert!(!filter.allows_proplist(
            Some("1000"),
            Some("firefox"),
            Some("Mozilla Firefox"),
            None,
        ));
        // Тот же бинарник в другом PID namespace (Flatpak) -> запрет
        assert!(!filter.allows_proplist(
            Some("2048"),
            Some("DISCORD-GUI-SPIKE"),
            Some("Discord"),
            None,
        ));
        assert!(!filter.allows_proplist(Some("2048"), Some("serein"), Some("Serein"), None,));
    }
}
