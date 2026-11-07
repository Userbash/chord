//! Сохранение UI-настроек (RW-40 / B8a): один локальный JSON-файл, без сети.
//!
//! В файле **только UI-настройки**: карта тумблеров `id → bool`, плотность
//! (`cozy`/`compact`), тема (`dark`/`light`), язык (`ru`/`en`), последний
//! выбранный сервер (`lastGuildId`, G-01b — подсказка восстановления, не
//! источник истины) и версия схемы.
//! Ни токенов, ни ключей keyring, ни паролей — посторонние поля отвергаются
//! (`deny_unknown_fields`), чтобы настройки нельзя было использовать как
//! хранилище секретов.
//!
//! Восстановление (G-01b): `lastGuildId` — аддитивное поле, файлы прежней
//! схемы без него читаются как `None`. Backend хранит ровно то, что записал
//! UI: гильдию из READY/списка он не подставляет. Значение — только snowflake;
//! пустое/чужое при чтении и записи нормализуется в `None`, потому что битая
//! подсказка не должна ломать загрузку остальных настроек.
//!
//! Путь: `DL_SETTINGS_PATH`, иначе `<repo>/.local/settings.json` — от корня
//! репозитория (`CARGO_MANIFEST_DIR` два уровня вверх), а не от `$HOME`:
//! спайк не должен писать в домашний каталог пользователя.
//!
//! Семантика ошибок [`SettingsError`]:
//! - `Parse` — файл не JSON/не UTF-8/не та структура (в т.ч. неизвестные поля);
//! - `Invalid` — JSON разобран, но значение вне контракта: чужая `version`,
//!   `density`/`theme`/`locale` вне белого списка, плохие id тумблеров,
//!   превышение [`MAX_SETTINGS_BYTES`];
//! - `Io` — файловая система отказала (нет прав, каталог вместо файла) или
//!   запись не удалась. Отсутствие файла — **не** ошибка: это
//!   [`UiSettings::default`].
//!
//! Запись атомарна: временный файл в том же каталоге (`.<имя>.tmp-<pid>-<n>`)
//! → `sync_all` → `rename`. Читатель видит либо старый файл, либо новый; при
//! сбое временный файл удаляется best-effort и рабочий файл не портится.
//! Каталог создаётся заново (`create_dir_all`), права — из umask: секретов в
//! файле нет.

use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

/// Версия схемы файла настроек. Чужая версия → `Invalid` (миграций пока нет).
pub const SETTINGS_VERSION: u32 = 1;
/// Жёсткий лимит файла настроек: 256 KiB. Больше — `Invalid { field: "size" }`.
pub const MAX_SETTINGS_BYTES: usize = 256 * 1024;
/// Потолок числа тумблеров в файле (рабочий UI — 38, SET-06/07/08).
/// Нужен как защита от раздувания, лимит байт остаётся главным ограничителем.
pub const MAX_TOGGLES: usize = 2048;
/// Максимальная длина id тумблера в байтах.
pub const MAX_TOGGLE_ID_LEN: usize = 256;
/// Имя файла настроек по умолчанию в `<repo>/.local`.
pub const SETTINGS_FILE_NAME: &str = "settings.json";
/// Переменная окружения с явным путём к файлу настроек.
pub const SETTINGS_PATH_ENV: &str = "DL_SETTINGS_PATH";
/// Специальный идентификатор режима Home («Личные сообщения и Друзья», N1-07 / G-01b)
/// в поле `lastGuildId`.
pub const HOME_GUILD_ID: &str = "@me";
/// Сколько символов значения показывать в тексте ошибки (плюс `…` при усечении).
const ERROR_VALUE_PREVIEW: usize = 32;

/// Счётчик временных файлов: имя уникально даже при двух записях из процесса.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

// ---------------------------------------------------------------------------
// Значения с белым списком
// ---------------------------------------------------------------------------

/// Плотность сообщений (UI S-14; ядро: `UIDensity` 1 COMPACT / 2 COZY).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Density {
    /// Уютная (дефолт отгруженного UI: `data-density="cozy"`).
    Cozy,
    /// Компактная.
    Compact,
}

impl Density {
    /// Допустимые строки файла — в порядке UI (`settings.js`: compact, cozy).
    pub const ALLOWED: [&'static str; 2] = ["cozy", "compact"];

    fn from_wire(value: &str) -> Option<Self> {
        match value {
            "cozy" => Some(Density::Cozy),
            "compact" => Some(Density::Compact),
            _ => None,
        }
    }
}

/// Тема оформления (UI: light/dark; ядро: `Theme` 1 DARK / 2 LIGHT).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Тёмная (дефолт: `color-scheme: dark`).
    Dark,
    /// Светлая.
    Light,
}

impl Theme {
    /// Допустимые строки файла.
    pub const ALLOWED: [&'static str; 2] = ["dark", "light"];

    fn from_wire(value: &str) -> Option<Self> {
        match value {
            "dark" => Some(Theme::Dark),
            "light" => Some(Theme::Light),
            _ => None,
        }
    }
}

/// Язык интерфейса. Список ровно тот, что реализован в UI (C4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    /// Русский (дефолт: `<html lang="ru">`).
    Ru,
    /// Английский.
    En,
}

impl Locale {
    /// Допустимые строки файла.
    pub const ALLOWED: [&'static str; 2] = ["ru", "en"];

    fn from_wire(value: &str) -> Option<Self> {
        match value {
            "ru" => Some(Locale::Ru),
            "en" => Some(Locale::En),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Модель настроек
// ---------------------------------------------------------------------------

/// UI-настройки, которые переживают перезапуск. Сикретов нет.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiSettings {
    /// Версия схемы файла ([`SETTINGS_VERSION`]).
    pub version: u32,
    /// Тумблеры: стабильный id → включён. Пустая карта = состояния из UI.
    pub toggles: BTreeMap<String, bool>,
    /// Плотность сообщений.
    pub density: Density,
    /// Тема.
    pub theme: Theme,
    /// Язык интерфейса.
    pub locale: Locale,
    /// G-01b: последний выбранный сервер или [`HOME_GUILD_ID`] (`"@me"`) —
    /// **подсказка** восстановления рейла. `None` — выбора не было или значение
    /// вне контракта; backend не выдумывает гильдию из READY/списка. В файле и
    /// IPC — camelCase `lastGuildId`; отсутствие ключа и `null` читаются как
    /// `None` (аддитивная миграция).
    #[serde(
        default,
        rename = "lastGuildId",
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_last_guild_id"
    )]
    pub last_guild_id: Option<String>,
    /// Признак активного режима Home («Личные сообщения и Друзья», N1-07 / G-01b)
    /// для обратной и прямой совместимости. Отсутствие ключа и `null` читаются
    /// как `None`; при `None` ключ не сериализуется.
    #[serde(default, rename = "homeMode", skip_serializing_if = "Option::is_none")]
    pub home_mode: Option<bool>,
}

/// Частичное обновление [`UiSettings`] (patch): изменяет только переданные поля,
/// сохраняя остальные значения настроек нетронутыми.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiSettingsPatch {
    /// Обновления тумблеров (`id → bool`), мержатся поверх существующей карты.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toggles: Option<BTreeMap<String, bool>>,
    /// Новая плотность сообщений.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub density: Option<Density>,
    /// Новая тема оформления.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<Theme>,
    /// Новый язык интерфейса.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<Locale>,
    /// Новый `lastGuildId` (`"@me"` или snowflake u64).
    #[serde(
        default,
        rename = "lastGuildId",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_guild_id: Option<String>,
    /// Признак режима Home (`homeMode`).
    #[serde(default, rename = "homeMode", skip_serializing_if = "Option::is_none")]
    pub home_mode: Option<bool>,
}

impl Default for UiSettings {
    /// Дефолты совпадают с отгруженным UI (`ui/index.html`): cozy/dark/ru,
    /// тумблеры пусты — их стартовые состояния принадлежат UI (D-17),
    /// `lastGuildId` и `homeMode` пусты — выбор сервера не выдумывается.
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            toggles: BTreeMap::new(),
            density: Density::Cozy,
            theme: Theme::Dark,
            locale: Locale::Ru,
            last_guild_id: None,
            home_mode: None,
        }
    }
}

impl UiSettings {
    /// Применяет частичное обновление [`UiSettingsPatch`] к текущим настройкам,
    /// нормализуя `lastGuildId` (если передан).
    pub fn apply_patch(&mut self, patch: &UiSettingsPatch) {
        if let Some(toggles) = &patch.toggles {
            for (key, value) in toggles {
                self.toggles.insert(key.clone(), *value);
            }
        }
        if let Some(density) = patch.density {
            self.density = density;
        }
        if let Some(theme) = patch.theme {
            self.theme = theme;
        }
        if let Some(locale) = patch.locale {
            self.locale = locale;
        }
        if let Some(last_guild_id) = patch.last_guild_id.as_deref() {
            self.last_guild_id = normalize_last_guild_id(Some(last_guild_id));
        }
        if let Some(home_mode) = patch.home_mode {
            self.home_mode = Some(home_mode);
        }
    }

    /// Возвращает копию настроек с применённым [`UiSettingsPatch`].
    pub fn patched(&self, patch: &UiSettingsPatch) -> Self {
        let mut updated = self.clone();
        updated.apply_patch(patch);
        updated
    }

    /// Проверка значений, которые не ловятся типами: версия схемы, число и id
    /// тумблеров, форма `lastGuildId`. Размер проверяется отдельно — по
    /// сериализованным байтам.
    ///
    /// [`SettingsStore::save`] нормализует `lastGuildId` до валидации, поэтому
    /// `Invalid` здесь достижим только у структуры, собранной в коде напрямую.
    pub fn validate(&self) -> Result<(), SettingsError> {
        if self.version != SETTINGS_VERSION {
            return Err(SettingsError::invalid(
                "version",
                format!(
                    "версия настроек {} не поддерживается (ожидается {SETTINGS_VERSION})",
                    self.version
                ),
            ));
        }
        if let Some(guild_id) = self.last_guild_id.as_deref() {
            if !is_valid_last_guild_id(guild_id) {
                return Err(SettingsError::invalid(
                    "lastGuildId",
                    format!(
                        "lastGuildId должен быть snowflake (только цифры, u64) или \"{HOME_GUILD_ID}\": \"{}\"",
                        preview(guild_id)
                    ),
                ));
            }
        }
        if self.toggles.len() > MAX_TOGGLES {
            return Err(SettingsError::invalid(
                "toggles",
                format!(
                    "слишком много тумблеров: {} (максимум {MAX_TOGGLES})",
                    self.toggles.len()
                ),
            ));
        }
        for key in self.toggles.keys() {
            if key.trim().is_empty() {
                return Err(SettingsError::invalid(
                    "toggles",
                    "id тумблера не может быть пустым",
                ));
            }
            if key.chars().any(char::is_control) {
                return Err(SettingsError::invalid(
                    "toggles",
                    format!(
                        "id тумблера содержит управляющий символ: \"{}\"",
                        preview(key)
                    ),
                ));
            }
            if key.len() > MAX_TOGGLE_ID_LEN {
                return Err(SettingsError::invalid(
                    "toggles",
                    format!(
                        "id тумблера длиннее {MAX_TOGGLE_ID_LEN} байт: \"{}…\"",
                        preview(key)
                    ),
                ));
            }
        }
        Ok(())
    }
}

/// Усечённое и экранированное значение для текста ошибки: переводы строк и
/// управляющие символы не должны ломать лог, а длинный id — раздувать ответ.
fn preview(value: &str) -> String {
    let escaped = value.escape_debug().to_string();
    let mut chars = escaped.chars();
    let head: String = chars.by_ref().take(ERROR_VALUE_PREVIEW).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

// ---------------------------------------------------------------------------
// G-01b: lastGuildId
// ---------------------------------------------------------------------------

/// Снежинка Discord: непустая десятичная строка, влезающая в `u64`.
/// Та же проверка, что `guilds::is_snowflake` (enter/leave guild): одно правило
/// на все входные id, чтобы выбор сервера не расходился с его вызовом.
fn is_snowflake(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && value.parse::<u64>().is_ok()
}

/// Допустимое значение `lastGuildId`: специальный идентификатор режима Home
/// ([`HOME_GUILD_ID`] = `"@me"`) либо числовой snowflake `u64`.
fn is_valid_last_guild_id(value: &str) -> bool {
    value == HOME_GUILD_ID || is_snowflake(value)
}

/// G-01b / N1-07: `lastGuildId` — только [`HOME_GUILD_ID`] (`"@me"`) или
/// snowflake; пустое/чужое значение даёт `None`.
/// Функция не ищет замену (первая гильдия из READY и т.п.): источник — явная
/// запись UI, backend лишь отбрасывает негодное.
pub fn normalize_last_guild_id(value: Option<&str>) -> Option<String> {
    value
        .filter(|text| is_valid_last_guild_id(text))
        .map(str::to_string)
}

/// Разбор `lastGuildId` из файла/IPC. Строка-snowflake → `Some`, всё остальное
/// (число, массив, `null`, пустая строка, нецифры, переполнение `u64`) → `None`:
/// битая подсказка восстановления не должна ломать чтение остальных настроек
/// или падать типом поля.
fn deserialize_last_guild_id<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(match value {
        Some(serde_json::Value::String(text)) => normalize_last_guild_id(Some(&text)),
        _ => None,
    })
}

// ---------------------------------------------------------------------------
// Ошибки
// ---------------------------------------------------------------------------

/// Типизированная ошибка хранилища настроек: `Io`, `Parse`, `Invalid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsError {
    /// Файловая система отказала: чтение/запись/каталог.
    Io { path: PathBuf, message: String },
    /// Файл есть, но это не ожидаемый JSON/структура (включая не-UTF-8 и
    /// неизвестные поля).
    Parse { path: PathBuf, message: String },
    /// JSON корректен, но значение вне контракта; `field` — имя поля.
    Invalid { field: String, message: String },
}

impl SettingsError {
    /// Имя варианта для JSON/логов и фронтенда.
    pub fn kind(&self) -> &'static str {
        match self {
            SettingsError::Io { .. } => "Io",
            SettingsError::Parse { .. } => "Parse",
            SettingsError::Invalid { .. } => "Invalid",
        }
    }

    /// Ошибка ввода-вывода с путём.
    pub fn io(path: &Path, message: impl Into<String>) -> Self {
        SettingsError::Io {
            path: path.to_path_buf(),
            message: message.into(),
        }
    }

    /// Ошибка разбора с путём.
    pub fn parse(path: &Path, message: impl Into<String>) -> Self {
        SettingsError::Parse {
            path: path.to_path_buf(),
            message: message.into(),
        }
    }

    /// Значение вне контракта: `field` — поле файла/команды.
    pub fn invalid(field: impl Into<String>, message: impl Into<String>) -> Self {
        SettingsError::Invalid {
            field: field.into(),
            message: message.into(),
        }
    }

    fn message(&self) -> &str {
        match self {
            SettingsError::Io { message, .. }
            | SettingsError::Parse { message, .. }
            | SettingsError::Invalid { message, .. } => message,
        }
    }
}

impl fmt::Display for SettingsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettingsError::Io { path, message } => {
                write!(formatter, "Io ({}): {message}", path.display())
            }
            SettingsError::Parse { path, message } => {
                write!(formatter, "Parse ({}): {message}", path.display())
            }
            SettingsError::Invalid { field, message } => {
                write!(formatter, "Invalid [{field}]: {message}")
            }
        }
    }
}

impl std::error::Error for SettingsError {}

/// JSON для фронтенда: стабильные `kind`/`message` + `path`/`field`, если есть.
struct ErrorView<'a> {
    kind: &'static str,
    message: &'a str,
    path: Option<String>,
    field: Option<&'a str>,
}

impl Serialize for SettingsError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let view = ErrorView {
            kind: self.kind(),
            message: self.message(),
            path: match self {
                SettingsError::Io { path, .. } | SettingsError::Parse { path, .. } => {
                    Some(path.display().to_string())
                }
                SettingsError::Invalid { .. } => None,
            },
            field: match self {
                SettingsError::Invalid { field, .. } => Some(field),
                _ => None,
            },
        };
        let mut state = serializer.serialize_struct("SettingsError", 4)?;
        state.serialize_field("kind", view.kind)?;
        state.serialize_field("message", view.message)?;
        if let Some(path) = view.path.as_deref() {
            state.serialize_field("path", path)?;
        }
        if let Some(field) = view.field {
            state.serialize_field("field", field)?;
        }
        state.end()
    }
}

// ---------------------------------------------------------------------------
// Путь к файлу
// ---------------------------------------------------------------------------

/// Корень репозитория: `rust/gui-spike` → два уровня вверх. Не `$HOME`.
pub fn default_settings_path() -> PathBuf {
    match PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
    {
        Some(repo) => repo.join(".local").join(SETTINGS_FILE_NAME),
        // Манифест вне ожидаемой раскладки (переносимый билд) — относительный
        // путь от текущего каталога, но всё равно не домашний.
        None => PathBuf::from(".local").join(SETTINGS_FILE_NAME),
    }
}

/// Разрешает путь: явное значение `DL_SETTINGS_PATH` важнее дефолта; пустая
/// или пробельная переменная — дефолт. Вынесено из [`settings_path`], чтобы
/// проверять приоритет без мутации env.
pub fn resolve_path(configured: Option<&str>) -> PathBuf {
    match configured {
        Some(value) if !value.trim().is_empty() => PathBuf::from(value),
        _ => default_settings_path(),
    }
}

/// Путь для команд: `DL_SETTINGS_PATH` или `<repo>/.local/settings.json`.
pub fn settings_path() -> PathBuf {
    let configured = std::env::var(SETTINGS_PATH_ENV).ok();
    resolve_path(configured.as_deref())
}

// ---------------------------------------------------------------------------
// Хранилище
// ---------------------------------------------------------------------------

/// Хранилище настроек поверх одного файла. Путь задаётся явно (тесты) или из
/// окружения ([`SettingsStore::from_env`]).
#[derive(Debug, Clone)]
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    /// Хранилище по конкретному пути.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Хранилище по `DL_SETTINGS_PATH` или репозиторному дефолту.
    pub fn from_env() -> Self {
        Self::new(settings_path())
    }

    /// Путь к файлу настроек.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Читает настройки. Нет файла → [`UiSettings::default`] (файл не
    /// создаётся); битый JSON/структура → `Parse`; значение вне контракта →
    /// `Invalid`; отказ ФС → `Io`. Паники на данных файла нет.
    pub fn load(&self) -> Result<UiSettings, SettingsError> {
        let metadata = match fs::metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(UiSettings::default())
            }
            Err(error) => {
                return Err(SettingsError::io(
                    &self.path,
                    format!("не удалось прочитать файл настроек: {error}"),
                ))
            }
        };
        // Лимит проверяется по metadata: файл-гигант не читается в память.
        if metadata.len() > MAX_SETTINGS_BYTES as u64 {
            return Err(SettingsError::invalid(
                "size",
                format!(
                    "файл настроек {} занимает {} байт, лимит {MAX_SETTINGS_BYTES}",
                    self.path.display(),
                    metadata.len()
                ),
            ));
        }
        let bytes = fs::read(&self.path).map_err(|error| {
            SettingsError::io(
                &self.path,
                format!("не удалось прочитать файл настроек: {error}"),
            )
        })?;
        let raw = String::from_utf8(bytes)
            .map_err(|_| SettingsError::parse(&self.path, "файл настроек не является UTF-8"))?;
        parse_settings(&self.path, &raw)
    }

    /// Сохраняет настройки: нормализация `lastGuildId` (G-01b) → валидация
    /// значений → лимит размера → атомарная запись (tmp + rename), каталог
    /// создаётся. При любом отказе рабочий файл не меняется, временных файлов
    /// не остаётся. Пустой/чужой `lastGuildId` не ошибка: он записывается как
    /// отсутствие выбора (`None`), иначе битая подсказка блокировала бы
    /// сохранение всех настроек.
    pub fn save(&self, settings: &UiSettings) -> Result<(), SettingsError> {
        let normalized = UiSettings {
            last_guild_id: normalize_last_guild_id(settings.last_guild_id.as_deref()),
            ..settings.clone()
        };
        normalized.validate()?;
        let bytes = serde_json::to_vec(&normalized).map_err(|error| {
            SettingsError::invalid(
                "json",
                format!("не удалось сериализовать настройки: {error}"),
            )
        })?;
        if bytes.len() > MAX_SETTINGS_BYTES {
            return Err(SettingsError::invalid(
                "size",
                format!(
                    "настройки занимают {} байт, лимит {MAX_SETTINGS_BYTES}",
                    bytes.len()
                ),
            ));
        }
        self.write_atomic(&bytes)
    }

    /// Загружает текущие настройки (или дефолты при отсутствии файла),
    /// применяет [`UiSettingsPatch`], сохраняет результат и возвращает его.
    pub fn patch(&self, patch: &UiSettingsPatch) -> Result<UiSettings, SettingsError> {
        let mut settings = self.load()?;
        settings.apply_patch(patch);
        settings.last_guild_id = normalize_last_guild_id(settings.last_guild_id.as_deref());
        self.save(&settings)?;
        Ok(settings)
    }

    /// Пишет байты во временный файл рядом с целевым и переименовывает его.
    fn write_atomic(&self, bytes: &[u8]) -> Result<(), SettingsError> {
        let parent = match self.path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            _ => Path::new("."),
        };
        fs::create_dir_all(parent).map_err(|error| {
            SettingsError::io(
                parent,
                format!("не удалось создать каталог настроек: {error}"),
            )
        })?;

        let file_name = self
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| SETTINGS_FILE_NAME.to_string());
        let temp = parent.join(format!(
            ".{file_name}.tmp-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));

        let outcome = write_temp(&temp, bytes).and_then(|()| fs::rename(&temp, &self.path));
        if let Err(error) = outcome {
            // best-effort: уборка мусора не должна подменять исходную ошибку.
            let _ = fs::remove_file(&temp);
            return Err(SettingsError::io(
                &self.path,
                format!("не удалось записать настройки: {error}"),
            ));
        }
        Ok(())
    }
}

/// Полная запись временного файла: `create_new` не даёт переиспользовать чужой
/// файл, `sync_all` фиксирует данные до `rename`.
fn write_temp(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

// ---------------------------------------------------------------------------
// Разбор
// ---------------------------------------------------------------------------

/// Разбор файла в [`UiSettings`]: сначала синтаксис JSON (`Parse`), затем
/// значения белого списка (`Invalid` с именем поля), затем структура (`Parse`).
fn parse_settings(path: &Path, raw: &str) -> Result<UiSettings, SettingsError> {
    let value: serde_json::Value = serde_json::from_str(raw).map_err(|error| {
        SettingsError::parse(path, format!("некорректный JSON настроек: {error}"))
    })?;
    let object = value
        .as_object()
        .ok_or_else(|| SettingsError::parse(path, "ожидался JSON-объект настроек"))?;

    // Явная проверка белых списков: неизвестное значение — `Invalid` с именем
    // поля, а не безликая ошибка serde.
    enum_field(
        object,
        "density",
        &Density::ALLOWED,
        Density::from_wire,
        path,
    )?;
    enum_field(object, "theme", &Theme::ALLOWED, Theme::from_wire, path)?;
    enum_field(object, "locale", &Locale::ALLOWED, Locale::from_wire, path)?;

    // Структура и типы остальных полей; `deny_unknown_fields` не пускает в файл
    // посторонние ключи (в том числе токены).
    let settings: UiSettings = serde_json::from_value(value).map_err(|error| {
        SettingsError::parse(path, format!("некорректная структура настроек: {error}"))
    })?;
    settings.validate()?;
    Ok(settings)
}

/// Строковое поле из белого списка: нет поля/не строка → `Parse`, значение вне
/// списка → `Invalid { field }`.
fn enum_field<T>(
    object: &serde_json::Map<String, serde_json::Value>,
    field: &'static str,
    allowed: &[&str],
    parse: impl Fn(&str) -> Option<T>,
    path: &Path,
) -> Result<T, SettingsError> {
    let raw = object.get(field).ok_or_else(|| {
        SettingsError::parse(path, format!("в настройках нет строкового поля {field}"))
    })?;
    let text = raw
        .as_str()
        .ok_or_else(|| SettingsError::parse(path, format!("поле {field} должно быть строкой")))?;
    parse(text).ok_or_else(|| {
        SettingsError::invalid(
            field,
            format!(
                "недопустимое значение {field}: \"{}\"; допустимо: {}",
                preview(text),
                allowed.join(", ")
            ),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_accepts_home_guild_id_and_snowflakes_only() {
        assert_eq!(
            normalize_last_guild_id(Some(HOME_GUILD_ID)),
            Some("@me".to_string())
        );
        assert_eq!(
            normalize_last_guild_id(Some("1234567890123456789")),
            Some("1234567890123456789".to_string())
        );
        for invalid in ["", " ", "abc", "@you", " @me", "@me ", "@ME"] {
            assert_eq!(
                normalize_last_guild_id(Some(invalid)),
                None,
                "{invalid:?} должен отбрасываться в None"
            );
        }
    }

    #[test]
    fn validate_accepts_home_guild_id_and_rejects_other_strings() {
        let mut settings = UiSettings {
            last_guild_id: Some(HOME_GUILD_ID.to_string()),
            home_mode: Some(true),
            ..UiSettings::default()
        };
        assert!(settings.validate().is_ok());

        for bad in ["abc", "@you", " @me", "@ME"] {
            settings.last_guild_id = Some(bad.to_string());
            let err = settings.validate().expect_err("ожидалась ошибка валидации");
            assert_eq!(
                err,
                SettingsError::invalid(
                    "lastGuildId",
                    format!(
                        "lastGuildId должен быть snowflake (только цифры, u64) или \"{HOME_GUILD_ID}\": \"{}\"",
                        preview(bad)
                    )
                )
            );
        }
    }

    #[test]
    fn patch_updates_home_mode_and_last_guild_id() {
        let base = UiSettings::default();
        let patch = UiSettingsPatch {
            last_guild_id: Some(HOME_GUILD_ID.to_string()),
            home_mode: Some(true),
            ..UiSettingsPatch::default()
        };
        let updated = base.patched(&patch);
        assert_eq!(updated.last_guild_id.as_deref(), Some("@me"));
        assert_eq!(updated.home_mode, Some(true));
        assert_eq!(updated.density, Density::Cozy);
    }
}
