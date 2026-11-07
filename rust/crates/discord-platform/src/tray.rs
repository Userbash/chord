//! BE-P1-09: Системный трей Linux, иконки состояний, бейдж непрочитанных и 6-пунктовое меню.
//!
//! # Обзор
//!
//! Модуль реализует управление системным треем десктопного клиента на Linux:
//! - Состояния иконки трея (`TrayIconState`): Normal, Unread, Speaking / InCall, Muted, Deafened
//!   с эталонными именами Discord Desktop (`discord-tray*` / `tray*`).
//! - 6-пунктовое контекстное меню (`TrayMenu`):
//!   1. Open («Открыть Discord»)
//!   2. Mute (чекбокс «Откл. микрофон»)
//!   3. Deafen (чекбокс «Откл. звук»)
//!   4. Voice Settings («Настройки голоса и видео»)
//!   5. Check for Updates («Проверить обновления»)
//!   6. Quit («Выйти из Discord»)
//! - Бейдж непрочитанных (`BadgeState`, `format_badge_count`) и полезная нагрузка D-Bus для
//!   `com.canonical.Unity.LauncherEntry.Update` (properties `count` и `count-visible`).
//! - Политика закрытия окна в трей (`MinimizeToTrayPolicy`).
//! - Контроллер трея (`TrayController`) с абстракцией бэкенда (`TrayBackend`) для
//!   надежной работы без живого X11/Wayland/SNI демона (DoD: отсутствие трея не роняет приложение).

use std::fmt;

use crate::dbus::{Signal, Value};

/// Интерфейс D-Bus для обновления счетчика непрочитанных в лаунчере Unity / Ubuntu Dock.
pub const UNITY_LAUNCHER_ENTRY_INTERFACE: &str = "com.canonical.Unity.LauncherEntry";
/// Имя сигнала обновления лаунчера Unity.
pub const UNITY_LAUNCHER_ENTRY_SIGNAL: &str = "Update";
/// Идентификатор desktop-файла приложения по умолчанию.
pub const DEFAULT_DESKTOP_ENTRY: &str = "application://discord.desktop";

/// Ошибки управления системным треем.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayError {
    /// Системный трей или D-Bus недоступны в текущем графическом окружении.
    BackendUnavailable(String),
    /// Сбой вызова операции бэкенда.
    OperationFailed(String),
    /// Ошибка ввода-вывода или передачи сообщений.
    Io(String),
    /// Ошибка протокола D-Bus.
    Dbus(String),
}

impl fmt::Display for TrayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BackendUnavailable(msg) => write!(f, "системный трей недоступен: {msg}"),
            Self::OperationFailed(msg) => write!(f, "сбой операции трея: {msg}"),
            Self::Io(msg) => write!(f, "ошибка ввода-вывода трея: {msg}"),
            Self::Dbus(msg) => write!(f, "ошибка D-Bus трея: {msg}"),
        }
    }
}

impl std::error::Error for TrayError {}

/// Состояния иконки системного трея Discord.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrayIconState {
    /// Обычное состояние (в сети, нет непрочитанных сообщений, не в звонке).
    Normal,
    /// Есть непрочитанные сообщения вне голосового звонка.
    Unread,
    /// В голосовом звонке / канале, активная речь или нормальный режим.
    Speaking,
    /// В голосовом звонке с выключенным микрофоном (self_mute).
    Muted,
    /// В голосовом звонке с выключенным звуком (self_deaf).
    Deafened,
}

impl TrayIconState {
    /// Алиас `InCall` для состояния активного голосового звонка (`Speaking`).
    pub const IN_CALL: Self = Self::Speaking;

    /// Имя системной иконки Freedesktop / Discord Desktop.
    pub const fn icon_name(&self) -> &'static str {
        match self {
            Self::Normal => "discord-tray",
            Self::Unread => "discord-tray-unread",
            Self::Speaking => "discord-tray-speaking",
            Self::Muted => "discord-tray-muted",
            Self::Deafened => "discord-tray-deafened",
        }
    }

    /// Имя ассета из `RE/main-arch.json` §9 (`TRAY_ICON_NAMES`).
    pub const fn asset_name(&self) -> &'static str {
        match self {
            Self::Normal => "tray",
            Self::Unread => "tray-unread",
            Self::Speaking => "tray-speaking",
            Self::Muted => "tray-muted",
            Self::Deafened => "tray-deafened",
        }
    }

    /// Вычисляет состояние иконки трея из параметров связи и непрочитанных сообщений.
    pub const fn compute(
        in_call: bool,
        self_mute: bool,
        self_deaf: bool,
        unread_count: u32,
    ) -> Self {
        compute_tray_icon_state(in_call, self_mute, self_deaf, unread_count)
    }
}

/// Вычисляет состояние иконки трея из параметров связи и непрочитанных сообщений.
///
/// Логика приоритетов согласуется с эталонным оракулом `m-core-1275`:
/// 1. В звонке:
///    - `self_deaf` имеет наивысший приоритет (`Deafened`).
///    - Затем `self_mute` (`Muted`).
///    - Иначе `Speaking` (`InCall`).
/// 2. Вне звонка:
///    - `unread_count > 0` (`Unread`).
///    - Иначе `Normal`.
pub const fn compute_tray_icon_state(
    in_call: bool,
    self_mute: bool,
    self_deaf: bool,
    unread_count: u32,
) -> TrayIconState {
    if in_call {
        if self_deaf {
            TrayIconState::Deafened
        } else if self_mute {
            TrayIconState::Muted
        } else {
            TrayIconState::Speaking
        }
    } else if unread_count > 0 {
        TrayIconState::Unread
    } else {
        TrayIconState::Normal
    }
}

/// Идентификатор действия пункта контекстного меню трея.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrayMenuAction {
    /// Открыть главное окно Discord.
    Open,
    /// Переключить отключение микрофона (чекбокс).
    Mute,
    /// Переключить отключение звука (чекбокс).
    Deafen,
    /// Открыть настройки голоса и видео.
    VoiceSettings,
    /// Проверить наличие обновлений клиента.
    CheckForUpdates,
    /// Завершить работу приложения.
    Quit,
}

/// Тип пункта контекстного меню.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayMenuItemType {
    /// Обычная кнопка вызова действия.
    Normal,
    /// Пункт-переключатель с состоянием `checked`.
    Checkbox,
    /// Разделитель меню.
    Separator,
}

/// Пункт контекстного меню системного трея.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayMenuItem {
    /// Действие пункта.
    pub action: TrayMenuAction,
    /// Отображаемый текст (локализованная подпись).
    pub label: String,
    /// Тип элемента меню (обычный или чекбокс).
    pub item_type: TrayMenuItemType,
    /// Флаг выбора для чекбоксов (`Mute`, `Deafen`).
    pub checked: bool,
    /// Доступность пункта для взаимодействия.
    pub enabled: bool,
    /// Видимость пункта.
    pub visible: bool,
}

impl TrayMenuItem {
    /// Создает пункт «Открыть Discord».
    pub fn open() -> Self {
        Self {
            action: TrayMenuAction::Open,
            label: "Открыть Discord".to_string(),
            item_type: TrayMenuItemType::Normal,
            checked: false,
            enabled: true,
            visible: true,
        }
    }

    /// Создает пункт «Откл. микрофон» (чекбокс).
    pub fn mute(checked: bool) -> Self {
        Self {
            action: TrayMenuAction::Mute,
            label: "Откл. микрофон".to_string(),
            item_type: TrayMenuItemType::Checkbox,
            checked,
            enabled: true,
            visible: true,
        }
    }

    /// Создает пункт «Откл. звук» (чекбокс).
    pub fn deafen(checked: bool) -> Self {
        Self {
            action: TrayMenuAction::Deafen,
            label: "Откл. звук".to_string(),
            item_type: TrayMenuItemType::Checkbox,
            checked,
            enabled: true,
            visible: true,
        }
    }

    /// Создает пункт «Настройки голоса и видео».
    pub fn voice_settings() -> Self {
        Self {
            action: TrayMenuAction::VoiceSettings,
            label: "Настройки голоса и видео".to_string(),
            item_type: TrayMenuItemType::Normal,
            checked: false,
            enabled: true,
            visible: true,
        }
    }

    /// Создает пункт «Проверить обновления».
    pub fn check_for_updates() -> Self {
        Self {
            action: TrayMenuAction::CheckForUpdates,
            label: "Проверить обновления".to_string(),
            item_type: TrayMenuItemType::Normal,
            checked: false,
            enabled: true,
            visible: true,
        }
    }

    /// Создает пункт «Выйти из Discord».
    pub fn quit() -> Self {
        Self {
            action: TrayMenuAction::Quit,
            label: "Выйти из Discord".to_string(),
            item_type: TrayMenuItemType::Normal,
            checked: false,
            enabled: true,
            visible: true,
        }
    }
}

/// 6-пунктовое контекстное меню приложения в трее.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayMenu {
    /// Список пунктов меню в строгом порядке.
    pub items: Vec<TrayMenuItem>,
}

impl Default for TrayMenu {
    fn default() -> Self {
        Self::new(false, false, false)
    }
}

impl TrayMenu {
    /// Создает новое меню с 6 пунктами согласно спецификации.
    pub fn new(in_call: bool, self_mute: bool, self_deaf: bool) -> Self {
        let mut mute = TrayMenuItem::mute(self_mute);
        let mut deafen = TrayMenuItem::deafen(self_deaf);
        // Вне звонка пункты управления голосом неактивны
        if !in_call {
            mute.enabled = false;
            deafen.enabled = false;
        }

        Self {
            items: vec![
                TrayMenuItem::open(),
                mute,
                deafen,
                TrayMenuItem::voice_settings(),
                TrayMenuItem::check_for_updates(),
                TrayMenuItem::quit(),
            ],
        }
    }

    /// Количество пунктов меню.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Проверка на пустоту меню.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Поиск пункта меню по действию.
    pub fn get(&self, action: TrayMenuAction) -> Option<&TrayMenuItem> {
        self.items.iter().find(|i| i.action == action)
    }

    /// Мутабельный поиск пункта меню по действию.
    pub fn get_mut(&mut self, action: TrayMenuAction) -> Option<&mut TrayMenuItem> {
        self.items.iter_mut().find(|i| i.action == action)
    }

    /// Обновляет состояние голосовых пунктов меню (`Mute` и `Deafen`).
    pub fn update_voice_state(&mut self, in_call: bool, self_mute: bool, self_deaf: bool) {
        if let Some(mute) = self.get_mut(TrayMenuAction::Mute) {
            mute.checked = self_mute;
            mute.enabled = in_call;
        }
        if let Some(deafen) = self.get_mut(TrayMenuAction::Deafen) {
            deafen.checked = self_deaf;
            deafen.enabled = in_call;
        }
    }
}

/// Форматирует счетчик непрочитанных сообщений:
/// - `0` -> `None` (скрыт)
/// - `1..=99` -> точное число в строке
/// - `>99` -> `'99+'`
pub fn format_badge_count(count: u32) -> Option<String> {
    match count {
        0 => None,
        1..=99 => Some(count.to_string()),
        _ => Some("99+".to_string()),
    }
}

/// Состояние бейджа непрочитанных на лаунчере / иконке.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BadgeState {
    /// Число непрочитанных сообщений.
    pub count: u32,
    /// Флаг видимости бейджа.
    pub visible: bool,
}

impl BadgeState {
    /// Создает новое состояние бейджа.
    pub const fn new(count: u32) -> Self {
        Self {
            count,
            visible: count > 0,
        }
    }

    /// Форматированная текстовая строка бейджа (`None` при `count == 0`).
    pub fn formatted(&self) -> Option<String> {
        format_badge_count(self.count)
    }

    /// Формирует словарь свойств D-Bus (`a{sv}`) для `com.canonical.Unity.LauncherEntry.Update`:
    /// - `"count"` -> `i32` (допустимое неотрицательное число)
    /// - `"count-visible"` -> `bool`
    pub fn unity_properties(&self) -> Vec<(Value, Value)> {
        let count_i32 = if self.count > i32::MAX as u32 {
            i32::MAX
        } else {
            self.count as i32
        };

        vec![
            (
                Value::String("count".to_string()),
                Value::Variant(Box::new(Value::I32(count_i32))),
            ),
            (
                Value::String("count-visible".to_string()),
                Value::Variant(Box::new(Value::Bool(self.visible))),
            ),
        ]
    }

    /// Формирует D-Bus сигнал `com.canonical.Unity.LauncherEntry.Update`.
    pub fn to_unity_signal(&self, app_uri: &str) -> Signal {
        Signal {
            path: "/com/canonical/unity/launcherentry/discord".to_string(),
            interface: UNITY_LAUNCHER_ENTRY_INTERFACE.to_string(),
            member: UNITY_LAUNCHER_ENTRY_SIGNAL.to_string(),
            body: vec![
                Value::String(app_uri.to_string()),
                Value::Dict(self.unity_properties()),
            ],
        }
    }
}

/// Результат обработки попытки закрытия окна приложения.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseDisposition {
    /// Окно прячется в трей, процесс продолжает работу.
    HideToTray,
    /// Процесс приложения завершается.
    QuitProcess,
}

/// Политика поведения при закрытии окна в системный трей.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinimizeToTrayPolicy {
    /// Включено ли сворачивание в системный трей при нажатии «закрыть».
    pub enabled: bool,
}

impl MinimizeToTrayPolicy {
    /// Создает новую политику сворачивания в трей.
    pub const fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    /// Определяет действие при закрытии окна.
    pub const fn should_hide_on_close(
        minimize_to_tray_enabled: bool,
        is_quitting: bool,
    ) -> CloseDisposition {
        if is_quitting {
            CloseDisposition::QuitProcess
        } else if minimize_to_tray_enabled {
            CloseDisposition::HideToTray
        } else {
            CloseDisposition::QuitProcess
        }
    }

    /// Вычисляет действие на основе собственного состояния и флага завершения.
    pub const fn disposition(&self, is_quitting: bool) -> CloseDisposition {
        Self::should_hide_on_close(self.enabled, is_quitting)
    }
}

/// Вычисляет действие при закрытии окна.
pub const fn should_hide_on_close(
    minimize_to_tray_enabled: bool,
    is_quitting: bool,
) -> CloseDisposition {
    MinimizeToTrayPolicy::should_hide_on_close(minimize_to_tray_enabled, is_quitting)
}

/// Трейт системного бэкенда трея (абстракция над StatusNotifierItem / AppIndicator / XEmbed).
pub trait TrayBackend: Send + Sync {
    /// Устанавливает иконку трея.
    fn set_icon(&mut self, icon_name: &str) -> Result<(), TrayError>;

    /// Устанавливает подсказку (tooltip) трея.
    fn set_tooltip(&mut self, tooltip: &str) -> Result<(), TrayError>;

    /// Обновляет контекстное меню трея.
    fn set_menu(&mut self, menu: &TrayMenu) -> Result<(), TrayError>;

    /// Обновляет бейдж непрочитанных (например, через Unity / D-Bus).
    fn set_badge(&mut self, badge: &BadgeState) -> Result<(), TrayError>;

    /// Уничтожает иконку в системном трее.
    fn destroy(&mut self) -> Result<(), TrayError>;
}

/// Тестовый бэкенд трея без оконного сервера (in-memory mock).
#[derive(Debug, Default, Clone)]
pub struct MockTrayBackend {
    /// Текущее установленное имя иконки.
    pub current_icon: Option<String>,
    /// Текущий текст подсказки.
    pub current_tooltip: Option<String>,
    /// Текущее состояние меню.
    pub current_menu: Option<TrayMenu>,
    /// Текущее состояние бейджа.
    pub current_badge: Option<BadgeState>,
    /// Флаг вызова уничтожения трея.
    pub destroyed: bool,
    /// Искусственный флаг сбоя установки иконки (для тестирования отказоустойчивости).
    pub fail_on_set_icon: bool,
    /// Искусственный флаг сбоя установки меню.
    pub fail_on_set_menu: bool,
    /// Искусственный флаг сбоя установки бейджа.
    pub fail_on_set_badge: bool,
}

impl TrayBackend for MockTrayBackend {
    fn set_icon(&mut self, icon_name: &str) -> Result<(), TrayError> {
        if self.fail_on_set_icon {
            return Err(TrayError::OperationFailed(
                "mock failure set_icon".to_string(),
            ));
        }
        self.current_icon = Some(icon_name.to_string());
        Ok(())
    }

    fn set_tooltip(&mut self, tooltip: &str) -> Result<(), TrayError> {
        self.current_tooltip = Some(tooltip.to_string());
        Ok(())
    }

    fn set_menu(&mut self, menu: &TrayMenu) -> Result<(), TrayError> {
        if self.fail_on_set_menu {
            return Err(TrayError::OperationFailed(
                "mock failure set_menu".to_string(),
            ));
        }
        self.current_menu = Some(menu.clone());
        Ok(())
    }

    fn set_badge(&mut self, badge: &BadgeState) -> Result<(), TrayError> {
        if self.fail_on_set_badge {
            return Err(TrayError::OperationFailed(
                "mock failure set_badge".to_string(),
            ));
        }
        self.current_badge = Some(*badge);
        Ok(())
    }

    fn destroy(&mut self) -> Result<(), TrayError> {
        self.destroyed = true;
        Ok(())
    }
}

/// Контроллер системного трея Discord.
///
/// Управляет состояниями иконки, контекстным меню, бейджем лаунчера и окном приложения.
/// Поддерживает безопасную работу в headless/fallback режиме, когда бэкенд трея отсутствует.
pub struct TrayController<B: TrayBackend> {
    backend: Option<B>,
    icon_state: TrayIconState,
    menu: TrayMenu,
    badge: BadgeState,
    minimize_policy: MinimizeToTrayPolicy,
    in_call: bool,
    self_mute: bool,
    self_deaf: bool,
    unread_count: u32,
    window_visible: bool,
    quitting: bool,
}

impl<B: TrayBackend> TrayController<B> {
    /// Создает новый контроллер с переданным бэкендом (или `None` при его отсутствии).
    pub fn new(backend: Option<B>, minimize_policy: MinimizeToTrayPolicy) -> Self {
        let in_call = false;
        let self_mute = false;
        let self_deaf = false;
        let unread_count = 0;
        let icon_state = compute_tray_icon_state(in_call, self_mute, self_deaf, unread_count);
        let menu = TrayMenu::new(in_call, self_mute, self_deaf);
        let badge = BadgeState::new(unread_count);

        let mut controller = Self {
            backend,
            icon_state,
            menu,
            badge,
            minimize_policy,
            in_call,
            self_mute,
            self_deaf,
            unread_count,
            window_visible: false,
            quitting: false,
        };

        // Инициализация бэкенда, если он есть
        if let Some(b) = controller.backend.as_mut() {
            let _ = b.set_icon(controller.icon_state.icon_name());
            let _ = b.set_menu(&controller.menu);
            let _ = b.set_badge(&controller.badge);
            let _ = b.set_tooltip("Discord");
        }

        controller
    }

    /// Создает headless-контроллер без физического трея (для тестов или серверов).
    pub fn new_headless(minimize_policy: MinimizeToTrayPolicy) -> Self {
        Self::new(None, minimize_policy)
    }

    /// Доступ к бэкенду трея.
    pub fn backend(&self) -> Option<&B> {
        self.backend.as_ref()
    }

    /// Мутабельный доступ к бэкенду трея.
    pub fn backend_mut(&mut self) -> Option<&mut B> {
        self.backend.as_mut()
    }

    /// Текущее вычисленное состояние иконки трея.
    pub fn icon_state(&self) -> TrayIconState {
        self.icon_state
    }

    /// Текущее состояние контекстного меню.
    pub fn menu(&self) -> &TrayMenu {
        &self.menu
    }

    /// Текущее состояние бейджа непрочитанных.
    pub fn badge(&self) -> &BadgeState {
        &self.badge
    }

    /// Видимо ли главное окно приложения.
    pub fn is_window_visible(&self) -> bool {
        self.window_visible
    }

    /// Находится ли клиент в процессе завершения работы.
    pub fn is_quitting(&self) -> bool {
        self.quitting
    }

    /// Обновляет параметры голосового звонка и синхронизирует иконку и меню.
    pub fn update_voice_state(
        &mut self,
        in_call: bool,
        self_mute: bool,
        self_deaf: bool,
    ) -> Result<(), TrayError> {
        self.in_call = in_call;
        self.self_mute = self_mute;
        self.self_deaf = self_deaf;

        self.icon_state = compute_tray_icon_state(in_call, self_mute, self_deaf, self.unread_count);
        self.menu.update_voice_state(in_call, self_mute, self_deaf);

        if let Some(b) = self.backend.as_mut() {
            b.set_icon(self.icon_state.icon_name())?;
            b.set_menu(&self.menu)?;
        }
        Ok(())
    }

    /// Обновляет количество непрочитанных сообщений и синхронизирует иконку и бейдж.
    pub fn set_unread_count(&mut self, count: u32) -> Result<(), TrayError> {
        self.unread_count = count;
        self.badge = BadgeState::new(count);
        self.icon_state =
            compute_tray_icon_state(self.in_call, self.self_mute, self.self_deaf, count);

        if let Some(b) = self.backend.as_mut() {
            b.set_icon(self.icon_state.icon_name())?;
            b.set_badge(&self.badge)?;
        }
        Ok(())
    }

    /// Обработка клика по иконке трея: переключает видимость окна.
    pub fn on_tray_clicked(&mut self) {
        self.window_visible = !self.window_visible;
    }

    /// Обработка действия пункта контекстного меню.
    pub fn on_menu_action(&mut self, action: TrayMenuAction) -> CloseDisposition {
        match action {
            TrayMenuAction::Open => {
                self.window_visible = true;
                CloseDisposition::HideToTray
            }
            TrayMenuAction::Mute => {
                if self.in_call {
                    let next = !self.self_mute;
                    let _ = self.update_voice_state(self.in_call, next, self.self_deaf);
                }
                CloseDisposition::HideToTray
            }
            TrayMenuAction::Deafen => {
                if self.in_call {
                    let next = !self.self_deaf;
                    let _ = self.update_voice_state(self.in_call, self.self_mute, next);
                }
                CloseDisposition::HideToTray
            }
            TrayMenuAction::VoiceSettings => {
                self.window_visible = true;
                CloseDisposition::HideToTray
            }
            TrayMenuAction::CheckForUpdates => CloseDisposition::HideToTray,
            TrayMenuAction::Quit => {
                self.quitting = true;
                CloseDisposition::QuitProcess
            }
        }
    }

    /// Обработка попытки закрытия окна пользователя (например, клик по крестику).
    pub fn handle_window_close(&mut self) -> CloseDisposition {
        let disposition = self.minimize_policy.disposition(self.quitting);
        if disposition == CloseDisposition::HideToTray {
            self.window_visible = false;
        }
        disposition
    }
}
