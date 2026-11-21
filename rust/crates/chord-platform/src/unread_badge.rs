//! R4-03: Бейджи непрочитанных сообщений на уровне ОС (OS Shell Integration).
//!
//! Поддерживает три десктопные платформы:
//! - **Linux**: Unity / Freedesktop D-Bus Launcher (`com.canonical.Unity.LauncherEntry.Update`)
//! - **Windows**: Taskbar Overlay Icon (`ITaskbarList3::SetOverlayIcon`, 16x16 RGBA8888)
//! - **macOS**: Dock Badge (`NSDockTile::setBadgeLabel:` / `app.dock.setBadge`)
//!
//! Правила отображения (R4-03):
//! - `0` -> очищает бейдж (`Cleared`)
//! - `1..=10` -> числовой индикатор (`Numeric(1..=10)`)
//! - `> 10` -> индикатор-точка переполнения (`OverflowDot`, `"•"`)
//! - Конфигурируется в настройках (`UnreadBadgeSettings`).

use std::fmt;

use crate::dbus::{BusAddress, Connection, DbusError, Signal, Value};

/// Максимальное число, отображаемое цифрой на бейдже; всё что выше — точка переполнения.
pub const BADGE_NUMERIC_MAX: u32 = 10;

/// Символ индикатора переполнения для текстовых бейджей (macOS Dock / UI preview).
pub const OVERFLOW_DOT_GLYPH: &str = "•";

/// D-Bus интерфейс Unity/Freedesktop LauncherEntry.
pub const UNITY_LAUNCHER_ENTRY_INTERFACE: &str = "com.canonical.Unity.LauncherEntry";

/// Имя D-Bus сигнала обновления бейджа.
pub const UNITY_LAUNCHER_ENTRY_SIGNAL: &str = "Update";

/// Путь объекта Unity LauncherEntry по умолчанию.
pub const UNITY_LAUNCHER_OBJECT_PATH: &str = "/com/canonical/unity/launcherentry/chord";

/// URI `.desktop`-файла по умолчанию для D-Bus сигнала.
pub const DEFAULT_APP_URI: &str = "application://chord.desktop";

/// Ошибки обновления системного бейджа непрочитанных.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnreadBadgeError {
    /// Системный бэкенд недоступен (например, нет D-Bus сессии).
    BackendUnavailable(String),
    /// Сбой при применении обновления в бэкенде.
    BackendFailed(String),
}

impl fmt::Display for UnreadBadgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BackendUnavailable(msg) => write!(f, "бейдж ОС недоступен: {msg}"),
            Self::BackendFailed(msg) => write!(f, "ошибка обновления бейджа ОС: {msg}"),
        }
    }
}

impl std::error::Error for UnreadBadgeError {}

/// Настройки отображения бейджа непрочитанных на иконке приложения.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnreadBadgeSettings {
    /// Включён ли системный бейдж непрочитанных.
    pub enabled: bool,
    /// Показывать ли числа `1..=max_numeric` (если `false`, любое `> 0` даёт точку).
    pub show_numeric: bool,
    /// Порог числового отображения (по умолчанию `10`).
    pub max_numeric: u32,
}

impl Default for UnreadBadgeSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            show_numeric: true,
            max_numeric: BADGE_NUMERIC_MAX,
        }
    }
}

/// Нормализованное состояние отображения бейджа непрочитанных.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnreadBadgeDisplay {
    /// `0` или отключено в настройках — бейдж очищен.
    Cleared,
    /// Числовой счётчик `1..=10`.
    Numeric(u32),
    /// Переполнение (`> 10` или режим только-точка) — компактный индикатор-точка.
    OverflowDot,
}

impl UnreadBadgeDisplay {
    /// Вычисляет состояние бейджа из количества непрочитанных и флага `enabled`.
    pub const fn from_count(count: u32, enabled: bool) -> Self {
        Self::from_count_with_settings(
            count,
            UnreadBadgeSettings {
                enabled,
                show_numeric: true,
                max_numeric: BADGE_NUMERIC_MAX,
            },
        )
    }

    /// Вычисляет состояние бейджа с учётом полных настроек.
    pub const fn from_count_with_settings(count: u32, settings: UnreadBadgeSettings) -> Self {
        if !settings.enabled || count == 0 {
            return Self::Cleared;
        }
        if settings.show_numeric && count <= settings.max_numeric {
            Self::Numeric(count)
        } else {
            Self::OverflowDot
        }
    }

    /// Видим ли бейдж на иконке приложения.
    pub const fn is_visible(&self) -> bool {
        !matches!(self, Self::Cleared)
    }

    /// Текстовая метка бейджа (`None` при `Cleared`, `"1".."10"`, `"•"` при переполнении).
    pub fn label(&self) -> Option<String> {
        match self {
            Self::Cleared => None,
            Self::Numeric(n) => Some(n.to_string()),
            Self::OverflowDot => Some(OVERFLOW_DOT_GLYPH.to_string()),
        }
    }
}

/// Представление бейджа для Linux Unity / Freedesktop D-Bus LauncherEntry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxUnityLauncherBadge {
    /// URI desktop-файла (`application://chord.desktop`).
    pub app_uri: String,
    /// Числовое значение (`1..=10`), либо `0` при очистке/переполнении.
    pub count: i32,
    /// Видимость числового счётчика (`true` для `1..=10`).
    pub count_visible: bool,
    /// Флаг привлечения внимания / переполнения (точка на доке).
    pub urgent: bool,
}

impl LinuxUnityLauncherBadge {
    /// Строит структуру из `UnreadBadgeDisplay`.
    pub fn from_display(display: UnreadBadgeDisplay, app_uri: impl Into<String>) -> Self {
        let app_uri = app_uri.into();
        match display {
            UnreadBadgeDisplay::Cleared => Self {
                app_uri,
                count: 0,
                count_visible: false,
                urgent: false,
            },
            UnreadBadgeDisplay::Numeric(n) => {
                let count = i32::try_from(n).unwrap_or(i32::MAX);
                Self {
                    app_uri,
                    count,
                    count_visible: true,
                    urgent: false,
                }
            }
            UnreadBadgeDisplay::OverflowDot => Self {
                app_uri,
                count: 0,
                count_visible: false,
                urgent: true,
            },
        }
    }

    /// Формирует словарь свойств `a{sv}` для D-Bus сигнала `Update`.
    pub fn unity_properties(&self) -> Vec<(Value, Value)> {
        vec![
            (
                Value::String("count".to_string()),
                Value::Variant(Box::new(Value::I32(self.count))),
            ),
            (
                Value::String("count-visible".to_string()),
                Value::Variant(Box::new(Value::Bool(self.count_visible))),
            ),
            (
                Value::String("urgent".to_string()),
                Value::Variant(Box::new(Value::Bool(self.urgent))),
            ),
        ]
    }

    /// Формирует D-Bus сигнал `com.canonical.Unity.LauncherEntry.Update`.
    pub fn to_unity_signal(&self) -> Signal {
        Signal {
            path: UNITY_LAUNCHER_OBJECT_PATH.to_string(),
            interface: UNITY_LAUNCHER_ENTRY_INTERFACE.to_string(),
            member: UNITY_LAUNCHER_ENTRY_SIGNAL.to_string(),
            body: vec![
                Value::String(self.app_uri.clone()),
                Value::Dict(self.unity_properties()),
            ],
        }
    }
}

/// Тип иконки наложения на панель задач Windows (`ITaskbarList3::SetOverlayIcon`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsOverlayKind {
    Numeric(u32),
    OverflowDot,
}

/// Данные иконки наложения Windows (16x16 RGBA8888).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsOverlayIcon {
    pub kind: WindowsOverlayKind,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub description: String,
}

/// Состояние Windows Taskbar Overlay Icon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowsTaskbarOverlay {
    /// `SetOverlayIcon(hwnd, NULL, NULL)` — очищает оверлей.
    Cleared,
    /// Оверлей с числом `1..=10` или точкой переполнения.
    Icon(WindowsOverlayIcon),
}

impl WindowsTaskbarOverlay {
    /// Строит Windows Taskbar Overlay из `UnreadBadgeDisplay`.
    pub fn from_display(display: UnreadBadgeDisplay) -> Self {
        match display {
            UnreadBadgeDisplay::Cleared => Self::Cleared,
            UnreadBadgeDisplay::Numeric(n) => Self::Icon(WindowsOverlayIcon {
                kind: WindowsOverlayKind::Numeric(n),
                width: 16,
                height: 16,
                rgba: render_16x16_badge_rgba(Some(n)),
                description: format!("{n} unread messages"),
            }),
            UnreadBadgeDisplay::OverflowDot => Self::Icon(WindowsOverlayIcon {
                kind: WindowsOverlayKind::OverflowDot,
                width: 16,
                height: 16,
                rgba: render_16x16_badge_rgba(None),
                description: "Unread messages (10+)".to_string(),
            }),
        }
    }
}

/// Представление бейджа для macOS Dock (`NSDockTile::setBadgeLabel:`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacOsDockBadge {
    /// Строка для `setBadgeLabel:` (`""` очищает, `"1".."10"`, `"•"`).
    pub label: String,
}

impl MacOsDockBadge {
    /// Пустой бейдж (очищает иконку в Dock).
    pub fn cleared() -> Self {
        Self {
            label: String::new(),
        }
    }

    /// Строит macOS Dock Badge из `UnreadBadgeDisplay`.
    pub fn from_display(display: UnreadBadgeDisplay) -> Self {
        Self {
            label: display.label().unwrap_or_default(),
        }
    }
}

/// Единый пакет обновления бейджей для всех поддерживаемых ОС.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsBadgeUpdate {
    pub display: UnreadBadgeDisplay,
    pub linux_unity: LinuxUnityLauncherBadge,
    pub windows_taskbar: WindowsTaskbarOverlay,
    pub macos_dock: MacOsDockBadge,
}

impl OsBadgeUpdate {
    /// Формирует пакет обновления для всех платформ.
    pub fn from_display(display: UnreadBadgeDisplay, app_uri: &str) -> Self {
        Self {
            display,
            linux_unity: LinuxUnityLauncherBadge::from_display(display, app_uri),
            windows_taskbar: WindowsTaskbarOverlay::from_display(display),
            macos_dock: MacOsDockBadge::from_display(display),
        }
    }
}

/// Абстракция платформенного бэкенда для применения бейджа непрочитанных.
pub trait OsBadgeBackend: Send + Sync {
    fn apply_badge(&mut self, update: &OsBadgeUpdate) -> Result<(), UnreadBadgeError>;
}

/// In-memory mock бэкенда бейджа для контрактного тестирования.
#[derive(Debug, Default, Clone)]
pub struct MockOsBadgeBackend {
    pub last_update: Option<OsBadgeUpdate>,
    pub history: Vec<OsBadgeUpdate>,
    pub fail_on_apply: bool,
}

impl OsBadgeBackend for MockOsBadgeBackend {
    fn apply_badge(&mut self, update: &OsBadgeUpdate) -> Result<(), UnreadBadgeError> {
        if self.fail_on_apply {
            return Err(UnreadBadgeError::BackendFailed(
                "mock apply_badge failure".to_string(),
            ));
        }
        self.last_update = Some(update.clone());
        self.history.push(update.clone());
        Ok(())
    }
}

/// Живой D-Bus бэкенд для Linux Unity / Freedesktop LauncherEntry.
pub struct DbusUnityBadgeBackend {
    connection: Connection,
    app_uri: String,
}

impl DbusUnityBadgeBackend {
    /// Создаёт бэкенд поверх установленного D-Bus соединения.
    pub fn new(connection: Connection, app_uri: impl Into<String>) -> Self {
        Self {
            connection,
            app_uri: app_uri.into(),
        }
    }

    /// Подключается к сессионной шине `DBUS_SESSION_BUS_ADDRESS`.
    pub fn connect_session(app_uri: impl Into<String>) -> Result<Self, UnreadBadgeError> {
        let addr = BusAddress::from_env().ok_or_else(|| {
            UnreadBadgeError::BackendUnavailable("DBUS_SESSION_BUS_ADDRESS не задан".to_string())
        })?;
        let connection = Connection::connect(&addr)
            .map_err(|err| UnreadBadgeError::BackendUnavailable(format_dbus_error(&err)))?;
        Ok(Self::new(connection, app_uri))
    }

    /// Возвращает URI `.desktop`-приложения.
    pub fn app_uri(&self) -> &str {
        &self.app_uri
    }
}

impl OsBadgeBackend for DbusUnityBadgeBackend {
    fn apply_badge(&mut self, update: &OsBadgeUpdate) -> Result<(), UnreadBadgeError> {
        let mut unity = update.linux_unity.clone();
        unity.app_uri = self.app_uri.clone();
        let signal = unity.to_unity_signal();
        self.connection
            .emit_signal(&signal)
            .map_err(|err| UnreadBadgeError::BackendFailed(format_dbus_error(&err)))
    }
}

/// Контроллер системных бейджей непрочитанных сообщений.
pub struct UnreadBadgeController<B: OsBadgeBackend> {
    backend: Option<B>,
    settings: UnreadBadgeSettings,
    unread_count: u32,
    display: UnreadBadgeDisplay,
    app_uri: String,
    last_update: OsBadgeUpdate,
}

impl<B: OsBadgeBackend> UnreadBadgeController<B> {
    /// Создаёт контроллер с опциональным бэкендом и начальными настройками.
    pub fn new(backend: Option<B>, settings: UnreadBadgeSettings) -> Self {
        let display = UnreadBadgeDisplay::from_count_with_settings(0, settings);
        let last_update = OsBadgeUpdate::from_display(display, DEFAULT_APP_URI);
        Self {
            backend,
            settings,
            unread_count: 0,
            display,
            app_uri: DEFAULT_APP_URI.to_string(),
            last_update,
        }
    }

    /// Возвращает текущее вычисленное состояние бейджа.
    pub fn display(&self) -> UnreadBadgeDisplay {
        self.display
    }

    /// Возвращает текущее сырое количество непрочитанных сообщений.
    pub fn unread_count(&self) -> u32 {
        self.unread_count
    }

    /// Возвращает текущие настройки бейджа.
    pub fn settings(&self) -> UnreadBadgeSettings {
        self.settings
    }

    /// Возвращает последний сформированный пакет обновления ОС.
    pub fn last_update(&self) -> &OsBadgeUpdate {
        &self.last_update
    }

    /// Доступ к бэкенду.
    pub fn backend(&self) -> Option<&B> {
        self.backend.as_ref()
    }

    /// Обновляет счётчик непрочитанных и синхронизирует бэкенд ОС.
    pub fn set_unread_count(&mut self, count: u32) -> Result<(), UnreadBadgeError> {
        self.unread_count = count;
        self.sync()
    }

    /// Включает или отключает бейджи в настройках и синхронизирует бэкенд ОС.
    pub fn set_enabled(&mut self, enabled: bool) -> Result<(), UnreadBadgeError> {
        self.settings.enabled = enabled;
        self.sync()
    }

    /// Применяет новые настройки и синхронизирует бэкенд ОС.
    pub fn set_settings(&mut self, settings: UnreadBadgeSettings) -> Result<(), UnreadBadgeError> {
        self.settings = settings;
        self.sync()
    }

    fn sync(&mut self) -> Result<(), UnreadBadgeError> {
        self.display =
            UnreadBadgeDisplay::from_count_with_settings(self.unread_count, self.settings);
        let update = OsBadgeUpdate::from_display(self.display, &self.app_uri);
        self.last_update = update.clone();
        if let Some(backend) = self.backend.as_mut() {
            backend.apply_badge(&update)?;
        }
        Ok(())
    }
}

fn format_dbus_error(err: &DbusError) -> String {
    match err {
        DbusError::Address => "некорректный адрес D-Bus".to_string(),
        DbusError::Io(e) => format!("ошибка ввода-вывода D-Bus: {e}"),
        DbusError::Auth(msg) => format!("ошибка аутентификации D-Bus: {msg}"),
        DbusError::Protocol(msg) => format!("ошибка протокола D-Bus: {msg}"),
        DbusError::Remote { name, message } => format!("удалённая ошибка D-Bus {name}: {message}"),
        DbusError::Timeout => "таймаут D-Bus".to_string(),
    }
}

/// Генерирует детерминированный 16x16 RGBA8888 круглый бейдж (Discord Red `#F23F42`)
/// с белым центром (точка при `None`, числовой глиф при `Some(1..=10)`).
fn render_16x16_badge_rgba(number: Option<u32>) -> Vec<u8> {
    const SIZE: i32 = 16;
    let mut buf = vec![0u8; (SIZE * SIZE * 4) as usize];

    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x * 2 + 1 - SIZE;
            let dy = y * 2 + 1 - SIZE;
            let dist_sq = dx * dx + dy * dy;
            let idx = ((y * SIZE + x) * 4) as usize;

            // Внешний круг (радиус ~7.5 px => 15^2 = 225)
            if dist_sq <= 225 {
                // Discord Red (#F23F42)
                buf[idx] = 0xF2;
                buf[idx + 1] = 0x3F;
                buf[idx + 2] = 0x42;
                buf[idx + 3] = 0xFF;

                let is_foreground = match number {
                    None => dist_sq <= 36, // Внутренняя белая точка (радиус ~3 px)
                    Some(n) => {
                        // Компактный маркер числа: вертикальная/горизонтальная перекладина в центре,
                        // различающаяся для разных значений 1..=10.
                        let in_box = (5..=10).contains(&x) && (4..=11).contains(&y);
                        in_box && (((x + y + n as i32) % 2 == 0) || (n >= 10 && x == 5))
                    }
                };

                if is_foreground {
                    buf[idx] = 0xFF;
                    buf[idx + 1] = 0xFF;
                    buf[idx + 2] = 0xFF;
                    buf[idx + 3] = 0xFF;
                }
            }
        }
    }

    buf
}
