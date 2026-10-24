//! BFF фазы 3 (W39e): типизированные данные для фронтенда строго из `fixtures/`.
//!
//! Сети здесь нет: каждый вызов читает санитизированные артефакты RE
//! (`fixtures/gateway/by-type/*.json`) с диска. Корень задаётся `DL_FIXTURES`
//! (по умолчанию — `fixtures/` в корне репозитория), поэтому домашний каталог
//! лаборатории в коде не зашит.
//!
//! Контракт Tauri-команд (`main.rs`):
//! - `ready()` → [`ReadyPayload`] — пользователь, гильдии, счётчики READY;
//! - `read_state()` → `Vec<ReadStateEntry>` — записи `READY.read_state`
//!   (курсор `lastViewed` для ack, `flags`, `mentionCount`, badge-форма
//!   `badgeCount`/`lastAckedId`/`readStateType`) без сети;
//! - `channels(guildId?)` → `Vec<ChannelRow>` — дерево каналов из READY
//!   (гильдейские + ЛС), типы числом, порядок «гильдия → position → id»;
//! - `messages_page(channelId, cursor?, before?, position?, limit?)` →
//!   [`MessagesPage`] — страница сообщений канала. `cursor` — forward-курсор:
//!   id последней строки предыдущей страницы; `before` — older-курсор: id
//!   первой строки уже загруженного окна, отдаются строки **старше** него;
//!   `position = "latest"` — стартовая страница с конца канала (без id).
//!   Строка несёт `embeds`/`attachments`/`reactions`/`replyTo`/`editedAt`,
//!   `day` (дата разделителя) и `group` (`start`/`continuation`);
//! - `members_page(guildId, cursor?, limit?)` → [`MembersPage`] — участники
//!   гильдии из снятых `GUILD_MEMBER_LIST_UPDATE`, роли, группы и счётчики;
//! - `search_page(query, filters, cursor, limit)` → [`SearchPage`] — локальный
//!   поиск по фикстурам с 7 фильтрами R-10 (автор, канал, содержит,
//!   упоминание, дата, тип, закреплено);
//! - `settings(section?)` → [`SettingsPayload`] — секции настроек из READY.
//!
//! Пагинация: `cursor`/`before` непрозрачны для UI и стабильны (id строки),
//! `limit` — `1..=100` ([`MAX_PAGE_LIMIT`]), пустая страница — валидный ответ.
//! Forward-страница отдаёт `limit` строк строго после `cursor` и `nextCursor` —
//! id последней строки. Older-страница отдаёт не более `limit` строк строго до
//! `before` в том же возрастающем порядке (UI делает prepend), `olderCursor` —
//! id первой строки (следующий `before`). `hasOlder`/`hasMore` независимы:
//! первое — есть ли строки старше страницы, второе — есть ли строки новее.
//! Оба курсора взаимоисключающие; `position = "latest"` не сочетается с ними.
//! `loaded_rows_limit` = [`UI_WINDOW_ROWS`] — инвариант UI (окно виртуализации
//! ≤ 500 строк), сервер его не навязывает и отдаёт не более `limit` строк.
//!
//! Пределы входа: файл фикстуры ≤ [`MAX_FIXTURE_BYTES`] (проверяется по
//! `metadata` до чтения в память), `channelId`/`guildId`/`cursor`/`section`/
//! `query` и значения фильтров ≤ [`MAX_INPUT_LEN`] символов, кадр `by-type`
//! обязан иметь непустой строковый `t` и присутствующий `d` — иначе `Parse`,
//! а не тихий пропуск.
//!
//! Ошибки — [`DataError`] с `kind`: `Io`, `Parse`, `NotFound`, `InvalidArg`;
//! сериализуются в `{ "kind", "message", "path"?/"resource"? }`. На входных
//! данных нет `unwrap()`: отсутствие обязательного поля — `Parse`, а не паника.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::{json, Value};

/// Размер страницы по умолчанию, строк.
pub const DEFAULT_PAGE_LIMIT: usize = 50;
/// Максимальный размер страницы: `page ≤ 100` из ТЗ.
pub const MAX_PAGE_LIMIT: usize = 100;
/// Бюджет окна UI (`loadedRows ≤ 500`), строк. Инвариант фронтенда, не сервера:
/// BFF лишь сообщает его в ответе, чтобы UI не расходился с контрактом.
pub const UI_WINDOW_ROWS: usize = 500;
/// Максимальный размер файла фикстуры, байт (64 MiB). Файл больше лимита
/// отвергается по `metadata.len()` **до** чтения: иначе локальный артефакт
/// в сотни MB съел бы память и CPU парсера.
pub const MAX_FIXTURE_BYTES: u64 = 64 * 1024 * 1024;
/// Максимальная длина пользовательских строк `channelId`/`guildId`/`cursor`,
/// символов. Более длинный вход — `InvalidArg`, в сообщении усечённое значение.
pub const MAX_INPUT_LEN: usize = 256;
/// Сколько символов значения показывать в тексте ошибки (плюс `…` при усечении).
const ERROR_VALUE_PREVIEW: usize = 32;

/// Каталог `fixtures/` в репозитории (на два уровня выше crate `rust/gui-spike`).
/// Используется только как значение по умолчанию; рабочий путь задаёт `DL_FIXTURES`.
fn default_fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(|repo| repo.join("fixtures"))
        .unwrap_or_else(|| PathBuf::from("fixtures"))
}

/// Разрешает корень фикстур: явный аргумент (из `DL_FIXTURES`) важнее дефолта.
/// Вынесено из [`fixtures_root`], чтобы проверять приоритет без мутации env.
pub fn resolve_root(configured: Option<&str>) -> PathBuf {
    match configured {
        Some(value) if !value.trim().is_empty() => PathBuf::from(value),
        _ => default_fixtures_root(),
    }
}

/// Корень фикстур для команд: `DL_FIXTURES` или репозиторный `fixtures/`.
/// Отсутствие каталога — `NotFound` с подсказкой про переменную окружения.
pub fn fixtures_root() -> Result<PathBuf, DataError> {
    let configured = std::env::var("DL_FIXTURES").ok();
    let root = resolve_root(configured.as_deref());
    if !root.is_dir() {
        return Err(DataError::not_found(
            "fixtures",
            format!(
                "каталог фикстур не найден: {} (задайте DL_FIXTURES)",
                root.display()
            ),
        ));
    }
    Ok(root)
}

// ---------------------------------------------------------------------------
// Ошибки
// ---------------------------------------------------------------------------

/// Типизированная ошибка BFF. Варианты фиксированы контрактом: `Io`, `Parse`,
/// `NotFound`, `InvalidArg`; `kind()` отдаёт это имя как есть.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataError {
    /// Файловая система недоступна/отказала (не «файла нет» — это `NotFound`).
    Io { path: PathBuf, message: String },
    /// Файл есть, но это не ожидаемый JSON/структура.
    Parse { path: PathBuf, message: String },
    /// Ресурс (файл, канал, гильдия, секция) не найден.
    NotFound { resource: String, message: String },
    /// Аргумент команды вне контракта (пустой id, limit, чужой курсор).
    InvalidArg { message: String },
}

impl DataError {
    /// Имя варианта для JSON/логов и для фронтенда.
    pub fn kind(&self) -> &'static str {
        match self {
            DataError::Io { .. } => "Io",
            DataError::Parse { .. } => "Parse",
            DataError::NotFound { .. } => "NotFound",
            DataError::InvalidArg { .. } => "InvalidArg",
        }
    }

    /// Ошибка ввода-вывода с путём.
    pub fn io(path: &Path, message: impl Into<String>) -> Self {
        DataError::Io {
            path: path.to_path_buf(),
            message: message.into(),
        }
    }

    /// Ошибка разбора JSON/структуры с путём.
    pub fn parse(path: &Path, message: impl Into<String>) -> Self {
        DataError::Parse {
            path: path.to_path_buf(),
            message: message.into(),
        }
    }

    /// Ресурс не найден: `resource` — что искали (`fixtures`, `канал`, `секция`).
    pub fn not_found(resource: impl Into<String>, message: impl Into<String>) -> Self {
        DataError::NotFound {
            resource: resource.into(),
            message: message.into(),
        }
    }

    /// Некорректный аргумент команды.
    pub fn invalid_arg(message: impl Into<String>) -> Self {
        DataError::InvalidArg {
            message: message.into(),
        }
    }

    fn message(&self) -> &str {
        match self {
            DataError::Io { message, .. }
            | DataError::Parse { message, .. }
            | DataError::NotFound { message, .. }
            | DataError::InvalidArg { message } => message,
        }
    }
}

impl fmt::Display for DataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataError::Io { path, message } => {
                write!(formatter, "Io ({}): {message}", path.display())
            }
            DataError::Parse { path, message } => {
                write!(formatter, "Parse ({}): {message}", path.display())
            }
            DataError::NotFound { resource, message } => {
                write!(formatter, "NotFound [{resource}]: {message}")
            }
            DataError::InvalidArg { message } => {
                write!(formatter, "InvalidArg: {message}")
            }
        }
    }
}

impl std::error::Error for DataError {}

/// Представление для JSON: стабильные `kind`/`message` + контекст, если есть.
struct ErrorView<'a> {
    kind: &'static str,
    message: &'a str,
    path: Option<String>,
    resource: Option<&'a str>,
}

impl Serialize for DataError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let view = ErrorView {
            kind: self.kind(),
            message: self.message(),
            path: match self {
                DataError::Io { path, .. } | DataError::Parse { path, .. } => {
                    Some(path.display().to_string())
                }
                DataError::NotFound { .. } | DataError::InvalidArg { .. } => None,
            },
            resource: match self {
                DataError::NotFound { resource, .. } => Some(resource),
                _ => None,
            },
        };
        // Компактный объект `{kind, message, path?, resource?}` — без вложенности.
        let mut state = serializer.serialize_struct("DataError", 4)?;
        state.serialize_field("kind", view.kind)?;
        state.serialize_field("message", view.message)?;
        if let Some(path) = view.path.as_deref() {
            state.serialize_field("path", path)?;
        }
        if let Some(resource) = view.resource {
            state.serialize_field("resource", resource)?;
        }
        state.end()
    }
}

// ---------------------------------------------------------------------------
// Чтение фикстур
// ---------------------------------------------------------------------------

/// Кадр диспатча из `gateway/by-type/*.json`: тип (`t`), данные (`d`), путь.
struct Frame {
    kind: String,
    data: Value,
    path: PathBuf,
}

fn map_io(path: &Path, error: &io::Error) -> DataError {
    if error.kind() == io::ErrorKind::NotFound {
        DataError::not_found(
            path.display().to_string(),
            format!("файл фикстуры не найден: {}", path.display()),
        )
    } else {
        DataError::io(path, error.to_string())
    }
}

/// Читает JSON-файл фикстуры. Нет файла → `NotFound`, битый JSON → `Parse`.
/// Файл больше [`MAX_FIXTURE_BYTES`] отвергается до чтения в память (по `metadata`).
fn read_json(path: &Path) -> Result<Value, DataError> {
    let size = fs::metadata(path)
        .map_err(|error| map_io(path, &error))?
        .len();
    if size > MAX_FIXTURE_BYTES {
        return Err(DataError::parse(
            path,
            format!(
                "фикстура больше лимита {MAX_FIXTURE_BYTES} байт ({size} байт): чтение отклонено"
            ),
        ));
    }
    let text = fs::read_to_string(path).map_err(|error| map_io(path, &error))?;
    serde_json::from_str(&text)
        .map_err(|error| DataError::parse(path, format!("некорректный JSON: {error}")))
}

/// Все кадры `by-type` в детерминированном порядке (по имени файла).
/// Пустой каталог — валидный (пустой) набор, а не ошибка.
///
/// Кадр строгий: `t` — непустая строка, `d` — присутствует и не `null`
/// (nil-кадры дампа несут `"d": "nil"`, `"d": null` не встречается ни разу,
/// поэтому `null` означает испорченный файл). Иначе `Parse` с путём и индексом:
/// кадр без типа иначе молча терялся бы и давал ложный `NotFound`/пустую страницу.
fn read_frames(root: &Path) -> Result<Vec<Frame>, DataError> {
    let dir = root.join("gateway").join("by-type");
    let entries = fs::read_dir(&dir).map_err(|error| map_io(&dir, &error))?;
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| match entry {
            Ok(entry) => Some(entry.path()),
            Err(error) => {
                // Не глотаем молча: сообщаем путь каталога и причину.
                eprintln!("data: пропущена запись {dir:?}: {error}");
                None
            }
        })
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect();
    paths.sort();

    let mut frames = Vec::with_capacity(paths.len());
    for (index, path) in paths.into_iter().enumerate() {
        let value = read_json(&path)?;
        let kind = value
            .get("t")
            .and_then(Value::as_str)
            .filter(|kind| !kind.is_empty())
            .ok_or_else(|| {
                DataError::parse(
                    &path,
                    format!("кадр by-type #{index}: поле t должно быть непустой строкой"),
                )
            })?
            .to_string();
        let data = match value.get("d") {
            None => {
                return Err(DataError::parse(
                    &path,
                    format!("кадр by-type #{index} ({kind}): нет поля d"),
                ));
            }
            Some(Value::Null) => {
                return Err(DataError::parse(
                    &path,
                    format!("кадр by-type #{index} ({kind}): поле d = null"),
                ));
            }
            Some(data) => data.clone(),
        };
        frames.push(Frame { kind, data, path });
    }
    Ok(frames)
}

fn array(value: Option<&Value>) -> &[Value] {
    value.and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

fn array_len(value: Option<&Value>) -> usize {
    array(value).len()
}

/// Строка фикстуры; санитизированная заглушка `"nil"` и `""` — это «нет значения».
fn as_non_nil_str(value: &Value) -> Option<&str> {
    value
        .as_str()
        .filter(|text| !text.is_empty() && *text != "nil")
}

fn field_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(as_non_nil_str)
}

fn field_u64(value: &Value, key: &str) -> Option<u64> {
    match value.get(key) {
        Some(Value::Number(number)) => number.as_u64(),
        Some(Value::String(text)) => text.parse().ok(),
        _ => None,
    }
}

fn field_i64(value: &Value, key: &str) -> Option<i64> {
    match value.get(key) {
        Some(Value::Number(number)) => number.as_i64(),
        Some(Value::String(text)) => text.parse().ok(),
        _ => None,
    }
}

fn field_bool(value: &Value, key: &str) -> bool {
    match value.get(key) {
        Some(Value::Bool(flag)) => *flag,
        Some(Value::String(text)) => text == "true",
        Some(Value::Number(number)) => number.as_u64() == Some(1),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// ready
// ---------------------------------------------------------------------------

/// Пользователь из READY. `email` остаётся сантизированным (`REDACTED`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadyUser {
    pub id: String,
    pub username: String,
    pub global_name: Option<String>,
    pub avatar: Option<String>,
    pub email: Option<String>,
    pub bio: Option<String>,
    pub premium: bool,
    pub mfa_enabled: bool,
}

/// Гильдия READY: имя берётся из `properties.name`, счётчики — из READY.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildSummary {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub member_count: u64,
    pub channel_count: usize,
}

/// Сессия клиента (READY `sessions[]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub client: String,
    pub os: String,
    pub version: u64,
    pub status: String,
}

/// Ответ `ready`: кто вошёл, где, сколько данных в READY.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadyPayload {
    pub user: ReadyUser,
    pub guilds: Vec<GuildSummary>,
    pub guild_count: usize,
    pub private_channel_count: usize,
    pub relationship_count: usize,
    pub read_state_count: usize,
    pub session: Option<SessionSummary>,
}

fn parse_ready_user(value: &Value, path: &Path) -> Result<ReadyUser, DataError> {
    let id = field_str(value, "id")
        .ok_or_else(|| DataError::parse(path, "READY.user без строкового id"))?;
    let username = field_str(value, "username")
        .ok_or_else(|| DataError::parse(path, "READY.user без строкового username"))?;
    Ok(ReadyUser {
        id: id.to_string(),
        username: username.to_string(),
        global_name: field_str(value, "global_name").map(str::to_string),
        avatar: field_str(value, "avatar").map(str::to_string),
        email: field_str(value, "email").map(str::to_string),
        bio: field_str(value, "bio").map(str::to_string),
        premium: field_bool(value, "premium"),
        mfa_enabled: field_bool(value, "mfa_enabled"),
    })
}

fn parse_guild(value: &Value) -> GuildSummary {
    let properties = value.get("properties");
    let name = properties
        .and_then(|props| field_str(props, "name"))
        .unwrap_or_default()
        .to_string();
    GuildSummary {
        id: field_str(value, "id").unwrap_or_default().to_string(),
        name,
        icon: properties
            .and_then(|props| field_str(props, "icon"))
            .map(str::to_string),
        member_count: field_u64(value, "member_count").unwrap_or(0),
        channel_count: array_len(value.get("channels")),
    }
}

/// Читает `gateway/by-type/002-READY.json` и отдаёт сводку для первого экрана.
pub fn ready(root: &Path) -> Result<ReadyPayload, DataError> {
    let path = root.join("gateway").join("by-type").join("002-READY.json");
    let value = read_json(&path)?;
    let data = value
        .get("d")
        .ok_or_else(|| DataError::parse(&path, "READY без объекта d"))?;
    let user_value = data
        .get("user")
        .ok_or_else(|| DataError::parse(&path, "READY без объекта user"))?;
    let user = parse_ready_user(user_value, &path)?;
    let guilds: Vec<GuildSummary> = array(data.get("guilds")).iter().map(parse_guild).collect();
    let session = array(data.get("sessions")).first().map(|session| {
        let info = session.get("client_info");
        SessionSummary {
            client: info
                .and_then(|value| field_str(value, "client"))
                .unwrap_or_default()
                .to_string(),
            os: info
                .and_then(|value| field_str(value, "os"))
                .unwrap_or_default()
                .to_string(),
            version: info
                .and_then(|value| field_u64(value, "version"))
                .unwrap_or(0),
            status: field_str(session, "status").unwrap_or_default().to_string(),
        }
    });
    let read_state_count = data
        .get("read_state")
        .and_then(|state| state.get("entries"))
        .and_then(Value::as_array)
        .map_or(0, Vec::len);

    Ok(ReadyPayload {
        guild_count: guilds.len(),
        private_channel_count: array_len(data.get("private_channels")),
        relationship_count: array_len(data.get("relationships")),
        read_state_count,
        guilds,
        user,
        session,
    })
}

// ---------------------------------------------------------------------------
// read_state — READY-проекция для UI (RW-40/M-07-BE2, badge — M-07-BE3)
// ---------------------------------------------------------------------------

/// Запись `READY.read_state.entries[]` в терминах IPC.
///
/// Поля — ровно ключи READY (`docs/re/gateway-schema.md`
/// `READY.d.read_state.entries[]`), имена — их точный camelCase: общая форма
/// `{channelId, flags, lastMessageId, lastPinTimestamp, lastViewed, mentionCount}`
/// и badge-форма (`read_state_type`, 2/157) —
/// `{badgeCount, lastAckedId, readStateType}`. `lastViewed` — счётчик read-state
/// (не snowflake; пример `4310` из `MESSAGE_ACK`).
///
/// Отсутствующее или санитизированное (`nil`) поле — `null`, а не 0: у обычной
/// записи нет badge-полей, у badge-записи нет `flags`/`lastViewed`, и выдуманный
/// 0 соврал бы UI. `badgeCount = 0` — **валидное значение** (16 и 0 в READY),
/// не «нет бейджа»: предикат `> 0` — политика UI/селектора, не проекции.
/// `lastAckedId` — десятичная строка, как `lastMessageId`: snowflake READY
/// (`1294268550826950656`) не помещается в JS-safe integer. `readStateType`
/// отдаётся числом как есть, без интерпретации (семантика 2/4 — RE-оракул,
/// `docs/rust-client/CODE-STANDARD.md` RW-17).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadStateEntry {
    pub channel_id: String,
    pub flags: Option<u64>,
    pub last_message_id: Option<String>,
    pub last_pin_timestamp: Option<String>,
    /// `last_viewed` из READY — счётчик ack для канала.
    pub last_viewed: Option<u64>,
    pub mention_count: Option<u64>,
    /// `badge_count` — число бейджей badge-формы; `None` у обычной записи.
    pub badge_count: Option<i64>,
    /// `last_acked_id` — non-channel ack-точка badge-формы, decimal-строка.
    pub last_acked_id: Option<String>,
    /// `read_state_type` — тип записи READY (2/4), как есть, без трактовки.
    pub read_state_type: Option<i64>,
}

/// Строковое поле, где `null`, `nil` и `""` — «значения нет». Другой тип —
/// `Parse`: битый READY не должен молча превращаться в `null`.
fn optional_str(
    value: &Value,
    key: &str,
    path: &Path,
    at: &str,
) -> Result<Option<String>, DataError> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) if text == "nil" || text.is_empty() => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(DataError::parse(
            path,
            format!("{at}: поле {key} должно быть строкой или null"),
        )),
    }
}

/// Числовое поле `u64`, где отсутствие/`null`/`nil` — `None`. Нераспознанное
/// значение — `Parse`, а не тихий `null`.
fn optional_u64(value: &Value, key: &str, path: &Path, at: &str) -> Result<Option<u64>, DataError> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) if text == "nil" || text.is_empty() => Ok(None),
        Some(_) => field_u64(value, key).map(Some).ok_or_else(|| {
            DataError::parse(path, format!("{at}: поле {key} должно быть числом u64"))
        }),
    }
}

/// Числовое поле `i64` (`badge_count`/`read_state_type` в READY): отсутствие/
/// `null`/`nil` — `None`, нераспознанное значение — `Parse`. Знак сохраняется
/// как есть: отрицательный `badge_count` — решение селектора, не проекции.
fn optional_i64(value: &Value, key: &str, path: &Path, at: &str) -> Result<Option<i64>, DataError> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) if text == "nil" || text.is_empty() => Ok(None),
        Some(_) => field_i64(value, key).map(Some).ok_or_else(|| {
            DataError::parse(path, format!("{at}: поле {key} должно быть числом i64"))
        }),
    }
}

/// Записи `READY.read_state.entries[]` для UI.
///
/// Источник — только локальный `gateway/by-type/002-READY.json`: сеть, токен и
/// keyring не нужны. Порядок записей сохраняется как в READY (UI сопоставляет
/// строки по `channelId`; пересортировка сломала бы соответствие серверу).
/// Badge-форма READY проецируется аддитивно: обычная запись получает три
/// `null`, badge-запись — `badgeCount`/`lastAckedId`/`readStateType` как есть.
///
/// Ошибки — [`DataError`]: нет READY — `NotFound`; битый JSON, отсутствие
/// `read_state`/`entries`, запись без строкового `id`, поле не того типа
/// (включая `badge_count`/`read_state_type`/`last_acked_id`) — `Parse` с
/// индексом записи. Ошибка никогда не маскируется пустым списком; пустой
/// `entries: []` — валидный ответ (у пользователя просто нет read-state).
pub fn read_state(root: &Path) -> Result<Vec<ReadStateEntry>, DataError> {
    let path = root.join("gateway").join("by-type").join("002-READY.json");
    let value = read_json(&path)?;
    let data = value
        .get("d")
        .ok_or_else(|| DataError::parse(&path, "READY без объекта d"))?;
    let read_state = data
        .get("read_state")
        .ok_or_else(|| DataError::parse(&path, "READY без объекта read_state"))?;
    let entries = match read_state.get("entries") {
        Some(Value::Array(items)) => items.as_slice(),
        _ => {
            return Err(DataError::parse(
                &path,
                "READY.read_state.entries должен быть массивом",
            ))
        }
    };

    let mut rows = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let at = format!("READY.read_state.entries[{index}]");
        let channel_id = field_str(entry, "id")
            .ok_or_else(|| DataError::parse(&path, format!("{at}: нет строкового id")))?;
        rows.push(ReadStateEntry {
            channel_id: channel_id.to_string(),
            flags: optional_u64(entry, "flags", &path, &at)?,
            last_message_id: optional_str(entry, "last_message_id", &path, &at)?,
            last_pin_timestamp: optional_str(entry, "last_pin_timestamp", &path, &at)?,
            last_viewed: optional_u64(entry, "last_viewed", &path, &at)?,
            mention_count: optional_u64(entry, "mention_count", &path, &at)?,
            badge_count: optional_i64(entry, "badge_count", &path, &at)?,
            last_acked_id: optional_str(entry, "last_acked_id", &path, &at)?,
            read_state_type: optional_i64(entry, "read_state_type", &path, &at)?,
        });
    }
    Ok(rows)
}

// ---------------------------------------------------------------------------
// channels — READY-проекция дерева каналов (C-02)
// ---------------------------------------------------------------------------

/// Канал READY для дерева каналов.
///
/// Поля — ровно те, что есть в READY: у гильдейского канала это `name`,
/// `position` и `parent_id`; у ЛС (`private_channels[]`) нет ни гильдии, ни
/// имени, ни позиции — там честный `None`, а не придуманное значение.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelRow {
    pub id: String,
    /// id гильдии; `None` — личный канал (ЛС).
    pub guild_id: Option<String>,
    /// Имя канала; у ЛС в READY имени нет.
    pub name: Option<String>,
    /// `type` кадра числом: 0 текст, 2 голос, 4 категория, 5 объявление,
    /// 15 форум, 1 ЛС. Не переименовывается: UI решает по числу.
    #[serde(rename = "type")]
    pub kind: u64,
    /// `parent_id` категории; санитизированное `nil` — `None`.
    pub parent_id: Option<String>,
    /// `position` внутри гильдии; у ЛС позиции нет.
    pub position: Option<i64>,
}

/// Массив из объекта READY: отсутствие/`null` — пустой набор, другой тип —
/// `Parse`. Иначе опечатка в фикстуре давала бы «каналов нет» вместо ошибки.
fn ready_array<'a>(data: &'a Value, key: &str, path: &Path) -> Result<&'a [Value], DataError> {
    match data.get(key) {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Array(items)) => Ok(items),
        Some(_) => Err(DataError::parse(
            path,
            format!("READY: поле {key} должно быть массивом"),
        )),
    }
}

/// Канал READY: `id` и `type` обязательны (без типа UI не решит, открывать ли
/// чат), остальные поля — как в фикстуре, `nil` — отсутствие значения.
fn parse_channel(
    channel: &Value,
    guild_id: Option<&str>,
    path: &Path,
    at: &str,
) -> Result<ChannelRow, DataError> {
    let id = field_str(channel, "id")
        .ok_or_else(|| DataError::parse(path, format!("{at}: нет строкового id")))?;
    let kind = field_u64(channel, "type")
        .ok_or_else(|| DataError::parse(path, format!("{at} (канал {id}): нет числового type")))?;
    Ok(ChannelRow {
        id: id.to_string(),
        guild_id: guild_id.map(str::to_string),
        name: field_str(channel, "name").map(str::to_string),
        kind,
        parent_id: field_str(channel, "parent_id").map(str::to_string),
        position: field_i64(channel, "position"),
    })
}

/// Каналы из READY: гильдейские `guilds[].channels[]` и ЛС `private_channels[]`.
///
/// `guild_id` — необязательный фильтр (обрезается, ≤ [`MAX_INPUT_LEN`]):
/// неизвестная гильдия — `NotFound`, а не пустой список. Порядок стабильный:
/// гильдии в порядке READY (как рейл), внутри — `position`, затем `id`;
/// у ЛС нет ни гильдии, ни позиции, поэтому они идут последними по `id`.
///
/// Это READY-проекция, а не «все каналы, где были сообщения»: каналы,
/// встреченные только в `MESSAGE_*`, в дерево не попадают. Сеть/токен не нужны.
pub fn channels(root: &Path, guild_id: Option<&str>) -> Result<Vec<ChannelRow>, DataError> {
    let guild_filter = match guild_id {
        None => None,
        Some(value) => {
            let value = value.trim();
            if value.is_empty() {
                return Err(DataError::invalid_arg("guildId не может быть пустым"));
            }
            ensure_input_len(value, "guildId")?;
            Some(value)
        }
    };

    let path = root.join("gateway").join("by-type").join("002-READY.json");
    let value = read_json(&path)?;
    let data = value
        .get("d")
        .ok_or_else(|| DataError::parse(&path, "READY без объекта d"))?;
    let guilds = ready_array(data, "guilds", &path)?;
    let private = ready_array(data, "private_channels", &path)?;

    if let Some(filter) = guild_filter {
        let known = guilds
            .iter()
            .any(|guild| field_str(guild, "id") == Some(filter));
        if !known {
            return Err(DataError::not_found(
                "гильдия",
                format!("гильдия {filter} не найдена в READY"),
            ));
        }
    }

    // Ключ сортировки: индекс гильдии в READY; `usize::MAX` — ЛС (в конце).
    let mut rows: Vec<(usize, ChannelRow)> = Vec::new();
    for (guild_index, guild) in guilds.iter().enumerate() {
        let id = field_str(guild, "id").ok_or_else(|| {
            DataError::parse(
                &path,
                format!("READY.guilds[{guild_index}] без строкового id"),
            )
        })?;
        if guild_filter.is_some_and(|filter| filter != id) {
            continue;
        }
        for (channel_index, channel) in array(guild.get("channels")).iter().enumerate() {
            rows.push((
                guild_index,
                parse_channel(
                    channel,
                    Some(id),
                    &path,
                    &format!("READY.guilds[{guild_index}].channels[{channel_index}]"),
                )?,
            ));
        }
    }
    if guild_filter.is_none() {
        for (index, channel) in private.iter().enumerate() {
            rows.push((
                usize::MAX,
                parse_channel(
                    channel,
                    None,
                    &path,
                    &format!("READY.private_channels[{index}]"),
                )?,
            ));
        }
    }

    rows.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| {
                left.1
                    .position
                    .unwrap_or(i64::MAX)
                    .cmp(&right.1.position.unwrap_or(i64::MAX))
            })
            .then_with(|| left.1.id.cmp(&right.1.id))
    });
    Ok(rows.into_iter().map(|(_, row)| row).collect())
}

// ---------------------------------------------------------------------------
// Общая пагинация
// ---------------------------------------------------------------------------

/// Проверенный размер страницы: `1..=MAX_PAGE_LIMIT`, по умолчанию `DEFAULT_PAGE_LIMIT`.
/// `pub(crate)` — мост ядра (`core_bridge`) обязан применять тот же лимит.
pub(crate) fn page_limit(limit: Option<usize>) -> Result<usize, DataError> {
    let limit = limit.unwrap_or(DEFAULT_PAGE_LIMIT);
    if !(1..=MAX_PAGE_LIMIT).contains(&limit) {
        return Err(DataError::invalid_arg(format!(
            "limit должен быть в 1..={MAX_PAGE_LIMIT}, получено {limit}"
        )));
    }
    Ok(limit)
}

/// Усечённое пользовательское значение для текста ошибки: не более
/// [`ERROR_VALUE_PREVIEW`] символов плюс `…`. Гигантский вход не копируется
/// в сообщение целиком (иначе сам отчёт об ошибке — аллокация на весь вход).
fn preview(value: &str) -> String {
    let mut short = String::new();
    let mut chars = value.chars();
    for _ in 0..ERROR_VALUE_PREVIEW {
        match chars.next() {
            Some(ch) => short.push(ch),
            None => return short,
        }
    }
    if chars.next().is_some() {
        short.push('…');
    }
    short
}

/// Проверяет длину пользовательского id/курсора: длиннее [`MAX_INPUT_LEN`] —
/// `InvalidArg` с усечённым значением. Пустую строку вызывающий отсекает сам.
fn ensure_input_len(value: &str, name: &str) -> Result<(), DataError> {
    let len = value.chars().count();
    if len > MAX_INPUT_LEN {
        return Err(DataError::invalid_arg(format!(
            "{name} длиннее {MAX_INPUT_LEN} символов: {} (всего {len})",
            preview(value)
        )));
    }
    Ok(())
}

/// Индекс первой строки после курсора. Курсор — id строки из этой же выборки;
/// `None`/`""` — с начала. Чужой id — `InvalidArg`, а не молчаливый сброс.
/// `pub(crate)` — мост ядра использует ту же семантику курсора, что и BFF.
pub(crate) fn cursor_start(
    ids: &[String],
    cursor: Option<&str>,
    what: &str,
) -> Result<usize, DataError> {
    match cursor.map(str::trim) {
        None | Some("") => Ok(0),
        Some(cursor) => {
            ensure_input_len(cursor, "курсор")?;
            ids.iter()
                .position(|id| id == cursor)
                .map(|position| position + 1)
                .ok_or_else(|| {
                    DataError::invalid_arg(format!(
                        "курсор {what} не найден в текущей выборке: {}",
                        preview(cursor)
                    ))
                })
        }
    }
}

/// Индекс строки-якоря older-страницы: строки строго **до** неё. `None`/`""` —
/// якоря нет (`Ok(None)`): вызывающий берёт хвост выборки. Чужой id —
/// `InvalidArg`, а не молчаливый сброс на начало/конец.
///
/// `pub(crate)` — мост ядра сможет применять ту же семантику `before`,
/// что и BFF (сейчас мост остаётся forward-only).
pub(crate) fn cursor_end(
    ids: &[String],
    before: Option<&str>,
    what: &str,
) -> Result<Option<usize>, DataError> {
    match before.map(str::trim) {
        None | Some("") => Ok(None),
        Some(before) => {
            ensure_input_len(before, "before-курсор")?;
            ids.iter()
                .position(|id| id == before)
                .map(Some)
                .ok_or_else(|| {
                    DataError::invalid_arg(format!(
                        "before-курсор {what} не найден в текущей выборке: {}",
                        preview(before)
                    ))
                })
        }
    }
}

// ---------------------------------------------------------------------------
// messages_page
// ---------------------------------------------------------------------------

/// Normalized channel id: trimmed, non-empty, ≤ [`MAX_INPUT_LEN`] chars.
fn ensure_channel_id(channel_id: &str) -> Result<&str, DataError> {
    let channel_id = channel_id.trim();
    if channel_id.is_empty() {
        return Err(DataError::invalid_arg("channelId не может быть пустым"));
    }
    ensure_input_len(channel_id, "channelId")?;
    Ok(channel_id)
}

/// Все строки канала без пагинации (общий загрузчик BFF и моста ядра).
///
/// Повторяет проверки `messages_page`: пустой/длинный `channelId` — `InvalidArg`,
/// неизвестный канал — `NotFound`, битый by-type — `Parse`.
pub(crate) fn channel_rows(root: &Path, channel_id: &str) -> Result<Vec<MessageRow>, DataError> {
    let channel_id = ensure_channel_id(channel_id)?;
    let frames = read_frames(root)?;
    if !known_channels(&frames).contains(channel_id) {
        return Err(DataError::not_found(
            "канал",
            format!("канал {channel_id} не найден в READY/by-type"),
        ));
    }
    collect_messages(&frames, channel_id)
}

/// Автор сообщения (только поля, нужные списку чата).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageAuthor {
    pub id: String,
    pub username: String,
    pub global_name: Option<String>,
    pub avatar: Option<String>,
    pub bot: bool,
}

/// Вложение сообщения. В снятых фикстурах `attachments` всегда `null`, поэтому
/// на практике список пуст; поля разбираются из кадра, если вложение появится.
/// Элемент без обязательных полей не роняет строку: `filename` пустой — это
/// отсутствие данных, а не выдуманное имя.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageAttachment {
    pub id: Option<String>,
    pub filename: String,
    pub size: u64,
    pub url: Option<String>,
    pub content_type: Option<String>,
    pub width: Option<u64>,
    pub height: Option<u64>,
}

/// Embed сообщения: только те поля, что реально есть в кадре.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageEmbed {
    pub id: Option<String>,
    /// `type` кадра: `article`, `rich`, `image`, …
    pub kind: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub url: Option<String>,
    pub color: Option<u64>,
    pub provider_name: Option<String>,
    pub author_name: Option<String>,
    pub footer_text: Option<String>,
    pub thumbnail_url: Option<String>,
    pub field_count: usize,
}

/// Реакция сообщения (`reactions[]` кадра). В снятых фикстурах поля нет.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageReaction {
    /// `emoji.name`, у кастомных — `emoji.id`, если имени нет.
    pub emoji: String,
    pub count: u64,
    pub me: bool,
}

/// Строка сообщения. `content` не рендерится как HTML — это данные, не разметка.
///
/// `day`/`group` вычислены из `timestamp` и соседних строк этого же канала:
/// `group = "start"` у первого сообщения и при смене автора или дня,
/// иначе `"continuation"`. Разрыв по времени (> 7 минут) не считается: для него
/// нужен парсер ISO-времени, а новых зависимостей не добавляем.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageRow {
    pub id: String,
    pub channel_id: String,
    pub guild_id: Option<String>,
    pub author: MessageAuthor,
    pub content: String,
    pub timestamp: Option<String>,
    pub edited: bool,
    pub edited_at: Option<String>,
    /// Дата `YYYY-MM-DD` из `timestamp` — для разделителя дня.
    pub day: Option<String>,
    /// `start` | `continuation` — для `data-group` строки.
    pub group: String,
    pub pinned: bool,
    /// `type` кадра (0 — обычное сообщение).
    pub message_type: u64,
    pub embeds: Vec<MessageEmbed>,
    pub attachments: Vec<MessageAttachment>,
    pub reactions: Vec<MessageReaction>,
    /// id сообщения, на которое отвечают (`message_reference`/`referenced_message`).
    pub reply_to: Option<String>,
    /// id упомянутых пользователей из `mentions[]`.
    pub mentions: Vec<String>,
    pub mention_everyone: bool,
}

/// Страница сообщений канала. `next_cursor == None` — строк новее нет;
/// пустая страница при `has_more == false` — валидный конец выборки.
///
/// `has_more`/`next_cursor` — forward (как было), `has_older`/`older_cursor` —
/// older-направление. `older_cursor` — id первой строки страницы: его передают
/// в `before`, чтобы получить следующую более старую страницу. Если строк
/// старше нет, оба older-поля — `false`/`None`, а не выдуманный курсор.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessagesPage {
    pub channel_id: String,
    pub items: Vec<MessageRow>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub has_older: bool,
    pub older_cursor: Option<String>,
    pub total: usize,
    pub limit: usize,
    pub loaded_rows_limit: usize,
}

fn parse_message_author(value: &Value, path: &Path) -> Result<MessageAuthor, DataError> {
    let id =
        field_str(value, "id").ok_or_else(|| DataError::parse(path, "author без строкового id"))?;
    let username = field_str(value, "username").unwrap_or(id);
    Ok(MessageAuthor {
        id: id.to_string(),
        username: username.to_string(),
        global_name: field_str(value, "global_name").map(str::to_string),
        avatar: field_str(value, "avatar").map(str::to_string),
        bot: field_bool(value, "bot"),
    })
}

/// Дата `YYYY-MM-DD` из ISO-подобного `timestamp`. Непарсящееся значение
/// (санитизированное `nil`, пустая строка, чужой формат) — `None`, не фейк.
fn day_of(timestamp: Option<&str>) -> Option<String> {
    let text = timestamp?;
    let date = text.get(..10)?;
    let bytes = date.as_bytes();
    let shape = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit());
    shape.then(|| date.to_string())
}

fn parse_mentions(data: &Value) -> Vec<String> {
    array(data.get("mentions"))
        .iter()
        .filter_map(|user| field_str(user, "id").map(str::to_string))
        .collect()
}

fn parse_reactions(value: Option<&Value>) -> Vec<MessageReaction> {
    array(value)
        .iter()
        .filter_map(|reaction| {
            let emoji = reaction.get("emoji")?;
            let name = field_str(emoji, "name").or_else(|| field_str(emoji, "id"));
            let Some(name) = name else {
                // Не глотаем молча: реакция без emoji.name/id не отрисуется.
                eprintln!("data: реакция без emoji.name/id пропущена");
                return None;
            };
            Some(MessageReaction {
                emoji: name.to_string(),
                count: field_u64(reaction, "count").unwrap_or(0),
                me: field_bool(reaction, "me"),
            })
        })
        .collect()
}

fn parse_embeds(data: &Value) -> Vec<MessageEmbed> {
    array(data.get("embeds"))
        .iter()
        .map(|embed| MessageEmbed {
            id: field_str(embed, "id").map(str::to_string),
            kind: field_str(embed, "type").unwrap_or("rich").to_string(),
            title: field_str(embed, "title").map(str::to_string),
            description: field_str(embed, "description").map(str::to_string),
            url: field_str(embed, "url").map(str::to_string),
            color: field_u64(embed, "color"),
            provider_name: embed
                .get("provider")
                .and_then(|provider| field_str(provider, "name"))
                .map(str::to_string),
            author_name: embed
                .get("author")
                .and_then(|author| field_str(author, "name"))
                .map(str::to_string),
            footer_text: embed
                .get("footer")
                .and_then(|footer| field_str(footer, "text"))
                .map(str::to_string),
            thumbnail_url: embed
                .get("thumbnail")
                .and_then(|thumbnail| field_str(thumbnail, "url"))
                .map(str::to_string),
            field_count: array_len(embed.get("fields")),
        })
        .collect()
}

fn parse_attachments(data: &Value) -> Vec<MessageAttachment> {
    array(data.get("attachments"))
        .iter()
        .map(|attachment| MessageAttachment {
            id: field_str(attachment, "id").map(str::to_string),
            filename: field_str(attachment, "filename")
                .unwrap_or_default()
                .to_string(),
            size: field_u64(attachment, "size").unwrap_or(0),
            url: field_str(attachment, "url").map(str::to_string),
            content_type: field_str(attachment, "content_type").map(str::to_string),
            width: field_u64(attachment, "width"),
            height: field_u64(attachment, "height"),
        })
        .collect()
}

/// id сообщения, на которое отвечают: `message_reference.message_id` или
/// `referenced_message.id`. Нет обоих — `None` (в фикстурах ответов нет).
fn parse_reply_to(data: &Value) -> Option<String> {
    data.get("message_reference")
        .and_then(|reference| field_str(reference, "message_id"))
        .or_else(|| {
            data.get("referenced_message")
                .and_then(|message| field_str(message, "id"))
        })
        .map(str::to_string)
}

fn parse_message(data: &Value, path: &Path) -> Result<MessageRow, DataError> {
    let id = field_str(data, "id")
        .ok_or_else(|| DataError::parse(path, "сообщение без строкового id"))?;
    let channel_id = field_str(data, "channel_id")
        .ok_or_else(|| DataError::parse(path, format!("сообщение {id} без channel_id")))?;
    let author_value = data
        .get("author")
        .ok_or_else(|| DataError::parse(path, format!("сообщение {id} без author")))?;
    let timestamp = field_str(data, "timestamp").map(str::to_string);
    let edited_at = field_str(data, "edited_timestamp").map(str::to_string);
    Ok(MessageRow {
        id: id.to_string(),
        channel_id: channel_id.to_string(),
        guild_id: field_str(data, "guild_id").map(str::to_string),
        author: parse_message_author(author_value, path)?,
        content: data
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        day: day_of(timestamp.as_deref()),
        timestamp,
        edited: edited_at.is_some(),
        edited_at,
        // Заполняется в `collect_messages` по соседним строкам канала.
        group: "start".to_string(),
        pinned: field_bool(data, "pinned"),
        message_type: field_u64(data, "type").unwrap_or(0),
        embeds: parse_embeds(data),
        attachments: parse_attachments(data),
        reactions: parse_reactions(data.get("reactions")),
        reply_to: parse_reply_to(data),
        mentions: parse_mentions(data),
        mention_everyone: field_bool(data, "mention_everyone"),
    })
}

/// Каналы, известные READY (гильдейские и ЛС) плюс каналы из самих кадров
/// сообщений: молчащий канал остаётся известным и отдаёт пустую страницу.
fn known_channels(frames: &[Frame]) -> HashSet<String> {
    let mut known = HashSet::new();
    for frame in frames {
        if frame.kind == "READY" {
            for guild in array(frame.data.get("guilds")) {
                for channel in array(guild.get("channels")) {
                    if let Some(id) = field_str(channel, "id") {
                        known.insert(id.to_string());
                    }
                }
            }
            for channel in array(frame.data.get("private_channels")) {
                if let Some(id) = field_str(channel, "id") {
                    known.insert(id.to_string());
                }
            }
        } else if frame.kind.starts_with("MESSAGE_") {
            if let Some(id) = field_str(&frame.data, "channel_id") {
                known.insert(id.to_string());
            }
        }
    }
    known
}

/// Метаданные канала для результатов поиска: имя и гильдия из READY.
/// Каналы, которых нет в READY (сняты в другой сессии), остаются с `None`.
#[derive(Debug, Clone, Default)]
struct ChannelInfo {
    name: Option<String>,
    guild_name: Option<String>,
}

fn channel_index(frames: &[Frame]) -> HashMap<String, ChannelInfo> {
    let mut index: HashMap<String, ChannelInfo> = HashMap::new();
    for frame in frames {
        if frame.kind != "READY" {
            continue;
        }
        for guild in array(frame.data.get("guilds")) {
            let guild_name = guild
                .get("properties")
                .and_then(|properties| field_str(properties, "name"))
                .map(str::to_string);
            for channel in array(guild.get("channels")) {
                let Some(id) = field_str(channel, "id") else {
                    continue;
                };
                index.insert(
                    id.to_string(),
                    ChannelInfo {
                        name: field_str(channel, "name").map(str::to_string),
                        guild_name: guild_name.clone(),
                    },
                );
            }
        }
        for channel in array(frame.data.get("private_channels")) {
            let Some(id) = field_str(channel, "id") else {
                continue;
            };
            index.entry(id.to_string()).or_insert_with(|| ChannelInfo {
                name: field_str(channel, "name").map(str::to_string),
                guild_name: None,
            });
        }
    }
    index
}

/// Собирает сообщения канала без дублей. `MESSAGE_CREATE` — база,
/// `MESSAGE_UPDATE` того же id лишь уточняет строку (текст, embed, правку),
/// не создавая вторую.
fn collect_messages(frames: &[Frame], channel_id: &str) -> Result<Vec<MessageRow>, DataError> {
    let mut messages: HashMap<String, MessageRow> = HashMap::new();
    for frame in frames {
        if frame.kind != "MESSAGE_CREATE" && frame.kind != "MESSAGE_UPDATE" {
            continue;
        }
        if field_str(&frame.data, "channel_id") != Some(channel_id) {
            continue;
        }
        let row = parse_message(&frame.data, &frame.path)?;
        match messages.get_mut(&row.id) {
            Some(existing) => {
                if frame.kind == "MESSAGE_UPDATE" {
                    existing.edited = true;
                    if existing.edited_at.is_none() {
                        existing.edited_at = row.edited_at.clone();
                    }
                    if !row.content.is_empty() {
                        existing.content = row.content;
                    }
                    // CREATE несёт `embeds: null`, UPDATE — реальный объект:
                    // без переноса embed терялся бы при дедупликации.
                    if existing.embeds.is_empty() {
                        existing.embeds = row.embeds;
                    }
                    if existing.attachments.is_empty() {
                        existing.attachments = row.attachments;
                    }
                    if existing.reactions.is_empty() {
                        existing.reactions = row.reactions;
                    }
                    if existing.reply_to.is_none() {
                        existing.reply_to = row.reply_to;
                    }
                    if existing.mentions.is_empty() {
                        existing.mentions = row.mentions;
                    }
                    existing.pinned |= row.pinned;
                }
            }
            None => {
                messages.insert(row.id.clone(), row);
            }
        }
    }
    let mut rows: Vec<MessageRow> = messages.into_values().collect();
    // Стабильный порядок: время, затем id. Числовые snowflake не парсим:
    // в санитизированных фикстурах встречаются отрицательные id.
    rows.sort_by(|left, right| {
        left.timestamp
            .cmp(&right.timestamp)
            .then_with(|| left.id.cmp(&right.id))
    });
    // `group` — по соседям в полном списке канала (до пагинации), поэтому
    // граница страницы не меняет разметку строки.
    let mut previous: Option<(String, Option<String>)> = None;
    for row in rows.iter_mut() {
        let starts = match &previous {
            None => true,
            Some((author, day)) => *author != row.author.id || *day != row.day,
        };
        row.group = if starts { "start" } else { "continuation" }.to_string();
        previous = Some((row.author.id.clone(), row.day.clone()));
    }
    Ok(rows)
}

/// Сторона открытия истории, когда якоря (`cursor`/`before`) не заданы.
///
/// В IPC приходит строкой: `"start"` (по умолчанию, первая страница с начала)
/// или `"latest"` (страница с конца канала). Другие значения — `InvalidArg`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MessagesPosition {
    /// С начала истории — прежнее поведение `messages_page` без курсора.
    #[default]
    Start,
    /// С конца истории: последние `limit` строк, порядок возрастающий.
    Latest,
}

impl MessagesPosition {
    /// Разбор IPC-значения: `None`/`""`/`"start"` → [`Self::Start`],
    /// `"latest"` → [`Self::Latest`], иначе `InvalidArg` с усечённым значением.
    pub fn parse(raw: Option<&str>) -> Result<Self, DataError> {
        match raw.map(str::trim) {
            None | Some("") | Some("start") => Ok(Self::Start),
            Some("latest") => Ok(Self::Latest),
            Some(other) => Err(DataError::invalid_arg(format!(
                "position должен быть \"start\" или \"latest\", получено: {}",
                preview(other)
            ))),
        }
    }
}

/// Запрос страницы сообщений: forward-курсор, older-курсор или сторона старта.
///
/// `cursor` и `before` взаимоисключающие, `position = Latest` не сочетается ни
/// с одним из них: комбинация — `InvalidArg`, а не тихий приоритет одного.
/// Пустые/пробельные курсоры считаются отсутствующими (как и раньше у `cursor`).
#[derive(Debug, Clone, Copy, Default)]
pub struct MessagesPageRequest<'a> {
    pub cursor: Option<&'a str>,
    pub before: Option<&'a str>,
    pub position: MessagesPosition,
    pub limit: Option<usize>,
}

/// Окно выборки → [`MessagesPage`]. `start..end` — полуинтервал строк канала
/// в порядке backend (`timestamp`, затем `id`).
///
/// `has_more`/`next_cursor` — есть ли строки после окна и id последней строки
/// (пара согласована: пустая страница не обещает курсор). `has_older` /
/// `older_cursor` — есть ли строки до окна и id первой строки, следующий
/// `before`. Строки старше/новее не выдумываются: только факты выборки.
fn messages_window(
    channel_id: &str,
    rows: &[MessageRow],
    start: usize,
    end: usize,
    limit: usize,
) -> MessagesPage {
    debug_assert!(start <= end && end <= rows.len(), "окно вне выборки");
    let items: Vec<MessageRow> = rows[start..end].to_vec();
    let has_more = !items.is_empty() && end < rows.len();
    let next_cursor = if has_more {
        items.last().map(|row| row.id.clone())
    } else {
        None
    };
    let has_older = start > 0 && !items.is_empty();
    let older_cursor = if has_older {
        items.first().map(|row| row.id.clone())
    } else {
        None
    };
    MessagesPage {
        channel_id: channel_id.to_string(),
        items,
        next_cursor,
        has_more,
        has_older,
        older_cursor,
        total: rows.len(),
        limit,
        loaded_rows_limit: UI_WINDOW_ROWS,
    }
}

/// Страница сообщений канала: `limit` строк строго после `cursor`.
///
/// Прежняя forward-сигнатура сохранена для существующих вызовов; older-режим —
/// [`messages_page_request`] с `before` или `position = Latest`.
/// Неизвестный канал → `NotFound`; известный молчащий канал → пустая страница
/// (`items: []`, `has_more: false`), это не ошибка.
pub fn messages_page(
    root: &Path,
    channel_id: &str,
    cursor: Option<&str>,
    limit: Option<usize>,
) -> Result<MessagesPage, DataError> {
    messages_page_request(
        root,
        channel_id,
        MessagesPageRequest {
            cursor,
            limit,
            ..MessagesPageRequest::default()
        },
    )
}

/// Страница сообщений по полному контракту C-02-BEFORE.
///
/// - `cursor` — forward: строки строго после id (поведение не изменилось);
/// - `before` — older: не более `limit` строк строго до id, порядок
///   возрастающий (как у forward-страниц — UI делает prepend). `before`
///   обязан быть id строки этого канала, иначе `InvalidArg`;
/// - `position = Latest` — последние `limit` строк канала без якоря;
/// - без якорей и `Start` — первая страница с начала истории.
///
/// `olderCursor` ответа — следующий `before`; когда `hasOlder == false`,
/// `olderCursor == None`. `hasMore`/`nextCursor` описывают forward-направление
/// и не меняются из-за older-запроса.
pub fn messages_page_request(
    root: &Path,
    channel_id: &str,
    request: MessagesPageRequest<'_>,
) -> Result<MessagesPage, DataError> {
    let channel_id = ensure_channel_id(channel_id)?;
    let limit = page_limit(request.limit)?;

    let cursor = request.cursor.map(str::trim).filter(|v| !v.is_empty());
    let before = request.before.map(str::trim).filter(|v| !v.is_empty());

    if cursor.is_some() && before.is_some() {
        return Err(DataError::invalid_arg(
            "cursor и before взаимоисключающие: задайте одно направление",
        ));
    }
    if request.position == MessagesPosition::Latest {
        if cursor.is_some() {
            return Err(DataError::invalid_arg(
                "position \"latest\" не сочетается с cursor: latest — старт с конца",
            ));
        }
        if before.is_some() {
            return Err(DataError::invalid_arg(
                "position \"latest\" не сочетается с before: latest — старт с конца",
            ));
        }
    }

    let rows = channel_rows(root, channel_id)?;
    let ids: Vec<String> = rows.iter().map(|row| row.id.clone()).collect();

    let (start, end) = match (before, request.position) {
        (Some(before), _) => {
            // Строки строго до якоря; якорь обязан существовать в канале.
            let end = cursor_end(&ids, Some(before), "сообщений")?.unwrap_or(rows.len());
            (end.saturating_sub(limit), end)
        }
        (None, MessagesPosition::Latest) => (rows.len().saturating_sub(limit), rows.len()),
        (None, MessagesPosition::Start) => {
            let start = cursor_start(&ids, cursor, "сообщений")?;
            (start, (start + limit).min(rows.len()))
        }
    };

    Ok(messages_window(channel_id, &rows, start, end, limit))
}

// ---------------------------------------------------------------------------
// members_page
// ---------------------------------------------------------------------------

/// Строка участника. `status` — presence из снимка (`offline`, если не снят).
///
/// `roles` — реальные id ролей из `member.roles`, отсортированы по позиции
/// (старшая первой), поэтому `roles[0]` — роль для чипа строки. `role_name` —
/// её имя из READY; у гильдии, которой нет в READY, имя честно отсутствует.
/// `group` — id группы снимка (роль или `online`/`idle`/`dnd`/`offline`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberRow {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub avatar: Option<String>,
    pub status: String,
    pub bot: bool,
    pub roles: Vec<String>,
    pub role_name: Option<String>,
    pub group: String,
}

/// Группа панели участников: роль (`kind = "role"`) или статус (`"status"`).
///
/// `announced_count` — `count` из `groups[]` самого свежего снимка (Discord
/// считает по всей гильдии; для больших гильдий список снят частично),
/// `loaded_count` — сколько строк этой группы реально есть в фикстурах.
/// `title` заполняется только для роли (имя из READY); для статуса — `None`,
/// подпись даёт фронтенд по id, чтобы не выдумывать текст в BFF.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberGroup {
    pub id: String,
    pub kind: String,
    pub title: Option<String>,
    pub position: Option<i64>,
    pub announced_count: Option<u64>,
    pub loaded_count: usize,
}

/// Страница участников гильдии.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MembersPage {
    pub guild_id: String,
    pub guild_name: Option<String>,
    pub member_count: Option<u64>,
    pub items: Vec<MemberRow>,
    pub groups: Vec<MemberGroup>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub total: usize,
    pub limit: usize,
    pub loaded_rows_limit: usize,
}

fn status_rank(status: &str) -> u8 {
    match status {
        "online" => 0,
        "idle" => 1,
        "dnd" => 2,
        "offline" => 3,
        _ => 4,
    }
}

/// Роль гильдии из READY: id, имя и позиция (для порядка групп).
#[derive(Debug, Clone, PartialEq, Eq)]
struct RoleInfo {
    id: String,
    name: String,
    position: i64,
}

/// Метаданные гильдии: READY (имя, счётчик, роли) + последний снимок
/// участников (`groups[]` с счётчиками). Счётчики последнего снимка
/// перекрывают предыдущие: Discord присылает их заново при каждом UPDATE.
#[derive(Debug, Clone, Default)]
struct GuildInfo {
    name: Option<String>,
    member_count: Option<u64>,
    roles: Vec<RoleInfo>,
    announced: Vec<(String, u64)>,
}

fn guild_index(frames: &[Frame]) -> HashMap<String, GuildInfo> {
    let mut index: HashMap<String, GuildInfo> = HashMap::new();
    for frame in frames {
        match frame.kind.as_str() {
            "READY" => {
                for guild in array(frame.data.get("guilds")) {
                    let Some(id) = field_str(guild, "id") else {
                        continue;
                    };
                    let entry = index.entry(id.to_string()).or_default();
                    entry.name = guild
                        .get("properties")
                        .and_then(|properties| field_str(properties, "name"))
                        .map(str::to_string);
                    entry.member_count = field_u64(guild, "member_count");
                    entry.roles = array(guild.get("roles"))
                        .iter()
                        .filter_map(|role| {
                            Some(RoleInfo {
                                id: field_str(role, "id")?.to_string(),
                                name: field_str(role, "name").unwrap_or_default().to_string(),
                                position: field_i64(role, "position").unwrap_or(0),
                            })
                        })
                        .collect();
                }
            }
            "GUILD_MEMBER_LIST_UPDATE" => {
                let Some(id) = field_str(&frame.data, "guild_id") else {
                    continue;
                };
                let entry = index.entry(id.to_string()).or_default();
                entry.member_count = field_u64(&frame.data, "member_count").or(entry.member_count);
                let announced: Vec<(String, u64)> = array(frame.data.get("groups"))
                    .iter()
                    .filter_map(|group| {
                        Some((
                            field_str(group, "id")?.to_string(),
                            field_u64(group, "count")?,
                        ))
                    })
                    .collect();
                if !announced.is_empty() {
                    entry.announced = announced;
                }
            }
            _ => {}
        }
    }
    index
}

fn is_status_group(id: &str) -> bool {
    matches!(id, "online" | "idle" | "dnd" | "offline")
}

/// Группа по presence, если снимок пришёл без маркера группы (UPDATE/INSERT
/// без `items` или повреждённый маркер): `idle`/`dnd` Discord показывает в
/// «В сети», `offline` — в «Не в сети».
fn status_group(status: &str) -> &'static str {
    match status {
        "online" | "idle" | "dnd" => "online",
        _ => "offline",
    }
}

fn parse_member(value: &Value, guild: &GuildInfo) -> Option<MemberRow> {
    let user = value.get("user")?;
    let id = field_str(user, "id")?;
    let status = value
        .get("presence")
        .and_then(|presence| field_str(presence, "status"))
        .unwrap_or("offline");
    let username = field_str(user, "username").unwrap_or(id);
    let display_name = field_str(user, "global_name")
        .or_else(|| field_str(user, "display_name"))
        .unwrap_or(username);
    let mut roles: Vec<String> = array(value.get("roles"))
        .iter()
        .filter_map(|role| role.as_str().map(str::to_string))
        .collect();
    // Старшая роль первой: позиция из READY, неизвестные роли — в конце.
    roles.sort_by_key(|role| {
        std::cmp::Reverse(
            guild
                .roles
                .iter()
                .find(|info| &info.id == role)
                .map(|info| info.position)
                .unwrap_or(i64::MIN),
        )
    });
    roles.dedup();
    let role_name = roles
        .first()
        .and_then(|role| guild.roles.iter().find(|info| &info.id == role))
        .map(|info| info.name.clone());
    Some(MemberRow {
        id: id.to_string(),
        username: username.to_string(),
        display_name: display_name.to_string(),
        avatar: field_str(user, "avatar").map(str::to_string),
        status: status.to_string(),
        bot: field_bool(user, "bot"),
        roles,
        role_name,
        group: status_group(status).to_string(),
    })
}

/// Участники гильдии из всех `GUILD_MEMBER_LIST_UPDATE` + роли/имя/счётчик из READY.
///
/// Элемент `ops[].items[]` — либо группа (`{"group": …}`, не участник, задаёт
/// группу следующим строкам), либо участник (`{"member": …}`). Всё остальное и
/// участник без `member.user.id` — повреждённый вход: `Parse` с индексом
/// элемента, а не молчаливый пропуск (иначе страница тихо теряет строки).
fn collect_members(
    frames: &[Frame],
    guild_id: &str,
    guild: &GuildInfo,
) -> Result<Vec<MemberRow>, DataError> {
    let mut members: HashMap<String, MemberRow> = HashMap::new();
    for frame in frames {
        if frame.kind != "GUILD_MEMBER_LIST_UPDATE" {
            continue;
        }
        if field_str(&frame.data, "guild_id") != Some(guild_id) {
            continue;
        }
        // Маркер группы действует на последующие элементы этого кадра; каждый
        // кадр начинает с чистого состояния (иначе группа «протекла» бы из
        // предыдущего снимка другой гильдии).
        let mut group: Option<String> = None;
        for (op_index, op) in array(frame.data.get("ops")).iter().enumerate() {
            for (item_index, item) in array(op.get("items")).iter().enumerate() {
                if item.get("member").is_none() {
                    if let Some(marker) = item.get("group") {
                        group = field_str(marker, "id").map(str::to_string);
                        continue;
                    }
                }
                let Some(member) = item.get("member") else {
                    return Err(DataError::parse(
                        &frame.path,
                        format!(
                            "GUILD_MEMBER_LIST_UPDATE: элемент ops[{op_index}].items[{item_index}] \
                             без member и без group"
                        ),
                    ));
                };
                let mut row = parse_member(member, guild).ok_or_else(|| {
                    DataError::parse(
                        &frame.path,
                        format!(
                            "GUILD_MEMBER_LIST_UPDATE: элемент ops[{op_index}].items[{item_index}] \
                             без member.user.id"
                        ),
                    )
                })?;
                row.group = group
                    .clone()
                    .unwrap_or_else(|| status_group(&row.status).to_string());
                match members.get_mut(&row.id) {
                    Some(existing) => {
                        // Более поздний снимок уточняет presence, если он был пуст.
                        if existing.status.is_empty() {
                            existing.status = row.status;
                        }
                    }
                    None => {
                        members.insert(row.id.clone(), row);
                    }
                }
            }
        }
    }
    Ok(members.into_values().collect())
}

/// Группы для ответа: объявленные в `groups[]` плюс встреченные в строках.
/// Порядок — роли по позиции (старшие выше), затем статусы
/// `online`/`idle`/`dnd`/`offline`, затем незнакомые id.
fn member_groups(guild: &GuildInfo, rows: &[MemberRow]) -> Vec<MemberGroup> {
    let mut ids: Vec<String> = guild.announced.iter().map(|(id, _)| id.clone()).collect();
    for row in rows {
        if !ids.contains(&row.group) {
            ids.push(row.group.clone());
        }
    }
    let mut groups: Vec<MemberGroup> = ids
        .into_iter()
        .map(|id| {
            let role = guild.roles.iter().find(|info| info.id == id);
            MemberGroup {
                kind: if role.is_some() || !is_status_group(&id) {
                    "role".to_string()
                } else {
                    "status".to_string()
                },
                title: role.map(|info| info.name.clone()),
                position: role.map(|info| info.position),
                announced_count: guild
                    .announced
                    .iter()
                    .find(|(group, _)| group == &id)
                    .map(|(_, count)| *count),
                loaded_count: rows.iter().filter(|row| row.group == id).count(),
                id,
            }
        })
        .collect();
    groups.sort_by(
        |left, right| match (left.kind.as_str(), right.kind.as_str()) {
            ("role", "role") => right
                .position
                .unwrap_or(i64::MIN)
                .cmp(&left.position.unwrap_or(i64::MIN))
                .then_with(|| left.id.cmp(&right.id)),
            ("role", _) => std::cmp::Ordering::Less,
            (_, "role") => std::cmp::Ordering::Greater,
            _ => status_rank(&left.id)
                .cmp(&status_rank(&right.id))
                .then_with(|| left.id.cmp(&right.id)),
        },
    );
    groups
}

/// Страница участников гильдии: `limit` строк строго после `cursor`.
///
/// Неизвестная гильдия → `NotFound`; известная гильдия без снятых
/// `GUILD_MEMBER_LIST_UPDATE` → пустая страница (не ошибка).
pub fn members_page(
    root: &Path,
    guild_id: &str,
    cursor: Option<&str>,
    limit: Option<usize>,
) -> Result<MembersPage, DataError> {
    let guild_id = guild_id.trim();
    if guild_id.is_empty() {
        return Err(DataError::invalid_arg("guildId не может быть пустым"));
    }
    ensure_input_len(guild_id, "guildId")?;
    let limit = page_limit(limit)?;
    let frames = read_frames(root)?;
    let guilds = guild_index(&frames);
    let Some(guild) = guilds.get(guild_id).cloned() else {
        return Err(DataError::not_found(
            "гильдия",
            format!("гильдия {guild_id} не найдена в READY/by-type"),
        ));
    };

    let mut rows = collect_members(&frames, guild_id, &guild)?;
    let groups = member_groups(&guild, &rows);
    let order: HashMap<&str, usize> = groups
        .iter()
        .enumerate()
        .map(|(position, group)| (group.id.as_str(), position))
        .collect();
    rows.sort_by(|left, right| {
        order[left.group.as_str()]
            .cmp(&order[right.group.as_str()])
            .then_with(|| {
                left.display_name
                    .to_lowercase()
                    .cmp(&right.display_name.to_lowercase())
            })
            .then_with(|| left.id.cmp(&right.id))
    });

    let ids: Vec<String> = rows.iter().map(|row| row.id.clone()).collect();
    let start = cursor_start(&ids, cursor, "участников")?;
    let end = (start + limit).min(rows.len());
    let items: Vec<MemberRow> = if start < end {
        rows[start..end].to_vec()
    } else {
        Vec::new()
    };
    let has_more = end < rows.len();
    let next_cursor = if has_more {
        items.last().map(|row| row.id.clone())
    } else {
        None
    };

    Ok(MembersPage {
        guild_id: guild_id.to_string(),
        guild_name: guild.name,
        member_count: guild.member_count,
        items,
        groups,
        next_cursor,
        has_more,
        total: rows.len(),
        limit,
        loaded_rows_limit: UI_WINDOW_ROWS,
    })
}

// ---------------------------------------------------------------------------
// search_page — локальный поиск по загруженным фикстурам
// ---------------------------------------------------------------------------

/// Фильтры поиска (7 секций R-10). `None` — фильтр не задан.
///
/// Строковые значения обрезаются по краям и ограничены [`MAX_INPUT_LEN`];
/// `day` обязан быть `YYYY-MM-DD`; `message_type` — `type` кадра;
/// `pinned` — `pinned` кадра. Пустой фильтр/запрос — «без ограничения»,
/// поэтому пустой результат — валидный ответ, а не ошибка.
///
/// `Deserialize` — канонический IPC-контракт (`camelCase`, отсутствующие ключи
/// `None`): команда `search_page` принимает этот тип напрямую, без зеркала в
/// `main.rs`. Семантика фильтров остаётся здесь, в слое данных.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SearchFilters {
    pub author_id: Option<String>,
    pub channel_id: Option<String>,
    pub content: Option<String>,
    pub mention_id: Option<String>,
    pub day: Option<String>,
    pub message_type: Option<u64>,
    pub pinned: Option<bool>,
}

/// Найденное сообщение: та же строка, что в чате, плюс метаданные канала
/// для строки результата (имя канала/гильдии может отсутствовать, если
/// канал не попал в READY).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub message: MessageRow,
    pub channel_name: Option<String>,
    pub guild_name: Option<String>,
}

/// Страница результатов поиска. `total` — сколько совпало во всех фикстурах,
/// `scanned` — сколько сообщений просмотрено (для честного «найдено / загружено»).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
    pub query: String,
    pub items: Vec<SearchHit>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub total: usize,
    pub scanned: usize,
    pub limit: usize,
    pub loaded_rows_limit: usize,
}

/// Пустая/пробельная строка фильтра = фильтр не задан.
fn normalized(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

/// `YYYY-MM-DD` — единственный принимаемый формат даты (как в `day` строки).
fn ensure_day(value: &str) -> Result<(), DataError> {
    ensure_input_len(value, "дата")?;
    let bytes = value.as_bytes();
    let shape = value.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit());
    if !shape {
        return Err(DataError::invalid_arg(format!(
            "дата должна быть в формате YYYY-MM-DD: {}",
            preview(value)
        )));
    }
    Ok(())
}

fn matches_query(row: &MessageRow, query_lower: &str) -> bool {
    query_lower.is_empty() || row.content.to_lowercase().contains(query_lower)
}

fn matches_mention(row: &MessageRow, mention_id: &str) -> bool {
    row.mentions.iter().any(|id| id == mention_id)
        || row.content.contains(&format!("<@{mention_id}>"))
        || row.content.contains(&format!("<@!{mention_id}>"))
}

/// Проверяет один фильтр по строке. Значения уже обрезаны и провалидированы.
fn matches_filters(row: &MessageRow, filters: &SearchFilters) -> bool {
    if let Some(author) = filters.author_id.as_deref().map(str::trim) {
        if row.author.id != author {
            return false;
        }
    }
    if let Some(channel) = filters.channel_id.as_deref().map(str::trim) {
        if row.channel_id != channel {
            return false;
        }
    }
    if let Some(content) = filters.content.as_deref().map(str::trim) {
        if !row.content.to_lowercase().contains(&content.to_lowercase()) {
            return false;
        }
    }
    if let Some(mention) = filters.mention_id.as_deref().map(str::trim) {
        if !matches_mention(row, mention) {
            return false;
        }
    }
    if let Some(day) = filters.day.as_deref().map(str::trim) {
        if row.day.as_deref() != Some(day) {
            return false;
        }
    }
    if let Some(kind) = filters.message_type {
        if row.message_type != kind {
            return false;
        }
    }
    if let Some(pinned) = filters.pinned {
        if row.pinned != pinned {
            return false;
        }
    }
    true
}

/// Локальный поиск по снятым фикстурам: тот же набор сообщений, что отдают
/// `messages_page`, но по всем каналам сразу, с фильтрами R-10.
///
/// Сортировка — время, затем id, затем канал (детерминированная при равных
/// timestamp). `limit` — `1..=100`; курсор — id последнего результата.
/// Нет совпадений — пустая страница с `total: 0`, это не ошибка.
pub fn search_page(
    root: &Path,
    query: &str,
    filters: &SearchFilters,
    cursor: Option<&str>,
    limit: Option<usize>,
) -> Result<SearchPage, DataError> {
    let query = query.trim();
    ensure_input_len(query, "query")?;
    for (value, name) in [
        (filters.author_id.as_deref(), "автор"),
        (filters.channel_id.as_deref(), "канал"),
        (filters.content.as_deref(), "содержит"),
        (filters.mention_id.as_deref(), "упоминание"),
    ] {
        if let Some(value) = value.map(str::trim) {
            ensure_input_len(value, name)?;
        }
    }
    if let Some(day) = filters
        .day
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        ensure_day(day)?;
    }
    // Пустая строка из поля ввода — «фильтр не задан», а не «искать пустое»:
    // иначе `authorId: ""` отдавал бы нулевой результат на пустом UI.
    let filters = SearchFilters {
        author_id: normalized(filters.author_id.as_deref()),
        channel_id: normalized(filters.channel_id.as_deref()),
        content: normalized(filters.content.as_deref()),
        mention_id: normalized(filters.mention_id.as_deref()),
        day: normalized(filters.day.as_deref()),
        message_type: filters.message_type,
        pinned: filters.pinned,
    };
    let limit = page_limit(limit)?;

    let frames = read_frames(root)?;
    let channels = channel_index(&frames);
    let query_lower = query.to_lowercase();
    let mut channel_ids: Vec<String> = known_channels(&frames).into_iter().collect();
    channel_ids.sort();
    let mut hits: Vec<SearchHit> = Vec::new();
    let mut scanned = 0;
    for channel_id in channel_ids {
        let rows = collect_messages(&frames, &channel_id)?;
        scanned += rows.len();
        let info = channels.get(&channel_id);
        for row in rows {
            if !matches_query(&row, &query_lower) || !matches_filters(&row, &filters) {
                continue;
            }
            hits.push(SearchHit {
                message: row,
                channel_name: info.and_then(|info| info.name.clone()),
                guild_name: info.and_then(|info| info.guild_name.clone()),
            });
        }
    }
    hits.sort_by(|left, right| {
        left.message
            .timestamp
            .cmp(&right.message.timestamp)
            .then_with(|| left.message.id.cmp(&right.message.id))
            .then_with(|| left.message.channel_id.cmp(&right.message.channel_id))
    });

    let ids: Vec<String> = hits.iter().map(|hit| hit.message.id.clone()).collect();
    let start = cursor_start(&ids, cursor, "результатов поиска")?;
    let end = (start + limit).min(hits.len());
    let items: Vec<SearchHit> = if start < end {
        hits[start..end].to_vec()
    } else {
        Vec::new()
    };
    let has_more = end < hits.len();
    let next_cursor = if has_more {
        items.last().map(|hit| hit.message.id.clone())
    } else {
        None
    };

    Ok(SearchPage {
        query: query.to_string(),
        items,
        next_cursor,
        has_more,
        total: hits.len(),
        scanned,
        limit,
        loaded_rows_limit: UI_WINDOW_ROWS,
    })
}

// ---------------------------------------------------------------------------
// settings
// ---------------------------------------------------------------------------

/// Секция настроек: стабильный `id` для навигации, заголовок и значения.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SettingsSection {
    pub id: String,
    pub title: String,
    pub values: Value,
}

/// Ответ `settings`: все секции или одна выбранная (`selected`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SettingsPayload {
    pub sections: Vec<SettingsSection>,
    pub selected: Option<String>,
}

fn muted_channels(data: &Value) -> usize {
    array(
        data.get("user_guild_settings")
            .and_then(|value| value.get("entries")),
    )
    .iter()
    .flat_map(|entry| array(entry.get("channel_overrides")))
    .filter(|overrides| field_bool(overrides, "muted"))
    .count()
}

/// Настройки из READY. Секции честно отражают только то, что есть в фикстуре:
/// профиль, уведомления, сессии, приватность и счётчики данных.
/// Неизвестная секция → `NotFound` (пустая строка — тоже неизвестная);
/// секция длиннее [`MAX_INPUT_LEN`] → `InvalidArg` до чтения READY (R5).
pub fn settings(root: &Path, section: Option<&str>) -> Result<SettingsPayload, DataError> {
    if let Some(section) = section {
        ensure_input_len(section, "section")?;
    }
    let path = root.join("gateway").join("by-type").join("002-READY.json");
    let value = read_json(&path)?;
    let data = value
        .get("d")
        .ok_or_else(|| DataError::parse(&path, "READY без объекта d"))?;
    let user_value = data
        .get("user")
        .ok_or_else(|| DataError::parse(&path, "READY без объекта user"))?;
    let user = parse_ready_user(user_value, &path)?;
    let guilds = array(data.get("guilds"));

    let session_entries: Vec<Value> = array(data.get("sessions"))
        .iter()
        .map(|session| {
            let info = session.get("client_info");
            json!({
                "client": info.and_then(|value| field_str(value, "client")),
                "os": info.and_then(|value| field_str(value, "os")),
                "version": info.and_then(|value| field_u64(value, "version")),
                "status": field_str(session, "status"),
            })
        })
        .collect();

    let sections = vec![
        SettingsSection {
            id: "profile".to_string(),
            title: "Профиль".to_string(),
            values: json!({
                "id": user.id,
                "username": user.username,
                "globalName": user.global_name,
                "avatar": user.avatar,
                "email": user.email,
                "bio": user.bio,
                "premium": user.premium,
                "mfaEnabled": user.mfa_enabled,
                "phoneSet": user_value.get("phone").and_then(as_non_nil_str).is_some(),
                "verified": field_bool(user_value, "verified"),
            }),
        },
        SettingsSection {
            id: "notifications".to_string(),
            title: "Уведомления".to_string(),
            values: json!({
                "flags": data
                    .get("notification_settings")
                    .and_then(|value| field_u64(value, "flags"))
                    .unwrap_or(0),
                "guildOverrides": array(
                    data.get("user_guild_settings").and_then(|value| value.get("entries"))
                )
                .len(),
                "mutedChannels": muted_channels(data),
            }),
        },
        SettingsSection {
            id: "sessions".to_string(),
            title: "Сессии".to_string(),
            values: json!({
                "count": session_entries.len(),
                "entries": session_entries,
            }),
        },
        SettingsSection {
            id: "privacy".to_string(),
            title: "Приватность".to_string(),
            values: json!({
                "consents": data.get("consents").cloned().unwrap_or(Value::Null),
                "nsfwAllowed": field_bool(user_value, "nsfw_allowed"),
                "ageVerificationStatus": field_u64(user_value, "age_verification_status"),
                "explicitContentScanVersion": field_u64(data, "explicit_content_scan_version"),
            }),
        },
        SettingsSection {
            id: "data".to_string(),
            title: "Данные".to_string(),
            values: json!({
                "guilds": guilds.len(),
                "privateChannels": array_len(data.get("private_channels")),
                "relationships": array_len(data.get("relationships")),
                "readStateEntries": data
                    .get("read_state")
                    .and_then(|state| state.get("entries"))
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len),
                "users": array_len(data.get("users")),
                "channels": guilds
                    .iter()
                    .map(|guild| array_len(guild.get("channels")))
                    .sum::<usize>(),
            }),
        },
    ];

    let Some(section) = section else {
        return Ok(SettingsPayload {
            sections,
            selected: None,
        });
    };
    let available: Vec<String> = sections.iter().map(|item| item.id.clone()).collect();
    match sections.into_iter().find(|item| item.id == section) {
        Some(found) => Ok(SettingsPayload {
            selected: Some(found.id.clone()),
            sections: vec![found],
        }),
        None => Err(DataError::not_found(
            "секция настроек",
            format!(
                "секция настроек «{section}» не найдена; доступны: {}",
                available.join(", ")
            ),
        )),
    }
}
