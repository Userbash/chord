//! B2a-ATTACH-BACKEND: слоты загрузки вложений — `POST /channels/{id}/attachments`.
//!
//! Контракт `[D]` (RE-DEEP-REA §1, ATTACH-RE §2):
//! - маршрут `MESSAGE_CREATE_ATTACHMENT_UPLOAD` = `POST /channels/{channel_id}/attachments`;
//! - тело `{"files":[{filename,file_size,id,is_clip}]}`, `id` — локальный UUID клиента;
//! - ответ `{"attachments":[{id,upload_url,upload_filename}]}`, больше ничего клиент не читает;
//! - лимиты (`RE-DEEP-REA.md:122-125`, `Math.max(0x1400000, tierLimit)`):
//!   20 МиБ на файл (`NONE`/`TIER_1`, базовый пол `0x1400000` = `20_971_520` байт),
//!   50 МиБ (`TIER_2` = `0x3200000`), 100 МиБ (`TIER_3` = `0x6400000`)
//!   и 10 вложений на сообщение (`n4=10`);
//!   сверх лимита — `413`/`40005 ENTITY_TOO_LARGE`, локальная предпроверка режет до сети;
//! - `429` → `retry-after` из заголовка в миллисекундах.
//!
//! PUT файла на `upload_url` и докачка `308` — следующий шаг (B2b), здесь только
//! получение слотов и сборка `uploaded_filename` для отправки сообщения.
//!
//! Сеть — только через [`AttachmentTransport`]; тесты подменяют транспорт фейком.
//! Токен живёт в auth-заголовке [`ApiClient`] и не попадает в ошибки/`Debug`.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock};

use discord_core::rest::client::{ApiAuth, ApiClient, ApiConfig, ApiRetryPolicy};
use discord_core::rest::routes::HttpMethod;
use discord_core::rest::transport::{
    send_with, Endpoint, HttpClientConfig, HttpRequest, HttpResponse,
};
use discord_core::API_V9_BASE;
use serde::{Deserialize, Deserializer, Serialize};

use crate::rest_failure::RestFailure;

fn clipboard_bytes_cache() -> &'static Mutex<HashMap<String, Vec<u8>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Vec<u8>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn percent_decode_path(raw: &str) -> String {
    let stripped = raw.trim().strip_prefix("file://").unwrap_or(raw.trim());
    let bytes = stripped.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (
                (bytes[i + 1] as char).to_digit(16),
                (bytes[i + 2] as char).to_digit(16),
            ) {
                out.push(((h << 4) | l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn read_system_clipboard_file_or_image() -> Option<(String, Vec<u8>)> {
    if let Ok(out) = std::process::Command::new("python3")
        .arg("-c")
        .arg(
            "import sys, os, urllib.parse\n\
             import gi\n\
             gi.require_version('Gtk', '3.0')\n\
             from gi.repository import Gtk, Gdk\n\
             cb = Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD)\n\
             pb = cb.wait_for_image()\n\
             if pb is not None:\n\
                 ok, buf = pb.save_to_bufferv('png', [], [])\n\
                 if ok and len(buf) > 0:\n\
                     sys.stdout.buffer.write(b'FILE:clipboard.png\\n' + buf)\n\
                     sys.exit(0)\n\
             uris = cb.wait_for_uris()\n\
             if not uris:\n\
                 txt = cb.wait_for_text() or ''\n\
                 if txt.strip().startswith('file://'):\n\
                     uris = [line.strip() for line in txt.splitlines() if line.strip().startswith('file://')]\n\
             if uris:\n\
                 p = urllib.parse.unquote(urllib.parse.urlparse(uris[0]).path)\n\
                 if os.path.isfile(p):\n\
                     with open(p, 'rb') as f:\n\
                         data = f.read()\n\
                     if len(data) > 0:\n\
                         name = os.path.basename(p) or 'file.bin'\n\
                         sys.stdout.buffer.write(('FILE:' + name + '\\n').encode('utf-8') + data)\n\
                         sys.exit(0)\n\
             sys.exit(1)\n",
        )
        .output()
    {
        if out.status.success() && out.stdout.starts_with(b"FILE:") {
            if let Some(nl) = out.stdout.iter().position(|&b| b == b'\n') {
                let name = String::from_utf8_lossy(&out.stdout[5..nl]).trim().to_string();
                let data = out.stdout[nl + 1..].to_vec();
                if !data.is_empty() {
                    return Some((if name.is_empty() { "clipboard.png".to_string() } else { name }, data));
                }
            }
        }
    }
    if let Ok(out) = std::process::Command::new("wl-paste")
        .args(["--type", "image/png"])
        .output()
    {
        if out.status.success() && !out.stdout.is_empty() {
            return Some(("clipboard.png".to_string(), out.stdout));
        }
    }
    None
}

fn resolve_clipboard_or_uri_files(files: &[AttachmentFileSpec]) -> Vec<AttachmentFileSpec> {
    files
        .iter()
        .map(|spec| {
            if spec.is_clip && spec.file_size <= 1 {
                if spec.filename.starts_with("file://") || spec.filename.starts_with('/') {
                    let path_str = percent_decode_path(&spec.filename);
                    let path = std::path::Path::new(&path_str);
                    if let Ok(bytes) = std::fs::read(path) {
                        if !bytes.is_empty() {
                            let name = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("file.bin")
                                .to_string();
                            let len = bytes.len() as u64;
                            if let Ok(mut guard) = clipboard_bytes_cache().lock() {
                                guard.insert(spec.id.clone(), bytes.clone());
                                guard.insert("__latest__".to_string(), bytes);
                            }
                            return AttachmentFileSpec {
                                filename: name,
                                file_size: len,
                                id: spec.id.clone(),
                                is_clip: true,
                            };
                        }
                    }
                }
                if let Some((name, bytes)) = read_system_clipboard_file_or_image() {
                    let len = bytes.len() as u64;
                    if let Ok(mut guard) = clipboard_bytes_cache().lock() {
                        guard.insert(spec.id.clone(), bytes.clone());
                        guard.insert("__latest__".to_string(), bytes);
                    }
                    return AttachmentFileSpec {
                        filename: name,
                        file_size: len,
                        id: spec.id.clone(),
                        is_clip: true,
                    };
                }
            }
            spec.clone()
        })
        .collect()
}

/// §1.5 (`RE-DEEP-REA.md:122-125`): базовый лимит на файл по формуле
/// `Math.max(0x1400000, tierLimit)` — 20 МиБ (`0x1400000` = `20_971_520` байт, `NONE`/`TIER_1`).
pub const MAX_UPLOAD_FILE_BYTES: u64 = 20 * 1024 * 1024;

/// §1.5: лимит на файл для Guild Boost Tier 2 — 50 МиБ (`0x3200000`).
pub const MAX_UPLOAD_FILE_BYTES_TIER2: u64 = 50 * 1024 * 1024;

/// §1.5: лимит на файл для Guild Boost Tier 3 — 100 МиБ (`0x6400000`).
pub const MAX_UPLOAD_FILE_BYTES_TIER3: u64 = 100 * 1024 * 1024;

/// §1.5: не больше 10 вложений в сообщении (`n4=10`).
pub const MAX_ATTACHMENTS_PER_MESSAGE: usize = 10;

/// §1.4 (`RE-DEEP-REA.md:100`): дефолтный `Content-Type` для `PUT {upload_url}`,
/// если вызывающий не передал MIME-тип.
pub const DEFAULT_UPLOAD_CONTENT_TYPE: &str = "application/octet-stream";

/// Маршрут обновления истёкших CDN-ссылок вложений (`POST /attachments/refresh-urls`).
pub const REFRESH_ATTACHMENT_URLS_PATH: &str = "/attachments/refresh-urls";

/// Максимальное количество ссылок на вложения в одном запросе обновления.
pub const MAX_REFRESH_URLS_PER_REQUEST: usize = 50;

/// Уровень буста сервера для расчёта лимита размера загружаемого файла (§1.5, `Math.max(0x1400000, tierLimit)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GuildBoostTier {
    /// Без буста (`NONE`): базовый лимит 20 МиБ (`0x1400000`, `max(20 MiB, 10 MiB)`).
    #[default]
    None,
    /// Уровень 1 (`TIER_1`): базовый лимит 20 МиБ (`0x1400000`, `max(20 MiB, 10 MiB)`).
    Tier1,
    /// Уровень 2 (`TIER_2`): 50 МиБ (`0x3200000`).
    Tier2,
    /// Уровень 3 (`TIER_3`): 100 МиБ (`0x6400000`).
    Tier3,
}

impl GuildBoostTier {
    /// Максимальный размер файла в байтах для данного уровня буста (§1.5).
    pub const fn max_upload_bytes(self) -> u64 {
        match self {
            Self::None | Self::Tier1 => MAX_UPLOAD_FILE_BYTES,
            Self::Tier2 => MAX_UPLOAD_FILE_BYTES_TIER2,
            Self::Tier3 => MAX_UPLOAD_FILE_BYTES_TIER3,
        }
    }
}

/// §1.1: шаблон пути; `channel_id` подставляется только после allowlist-проверки.
const ATTACHMENTS_PATH_TEMPLATE: &str = "/channels/{id}/attachments";

/// Как в `messages`: id идёт в URL-путь, поэтому только ASCII-цифры/буквы, `_`, `-`.
const MAX_CHANNEL_ID_BYTES: usize = 64;

/// Код Discord `ENTITY_TOO_LARGE`: сервер присылает его в `body.code` (§1.6).
const ENTITY_TOO_LARGE_CODE: u64 = 40005;

/// Метаданные файла для запроса слотов (§1.3, `getUploadPayload`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentFileSpec {
    /// `filename` — имя файла на клиенте.
    pub filename: String,
    /// `file_size` — размер в байтах; 0 байт отклоняется (`ENTITY_EMPTY`).
    pub file_size: u64,
    /// `id` — локальный UUID клиента, не серверный id.
    pub id: String,
    /// `is_clip` — вложение из клипа (экрана), а не обычный файл.
    pub is_clip: bool,
}

/// Слот загрузки из ответа `attachments[]` (§1.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentUploadSlot {
    /// `id` — тот же локальный id, что уходил в `files[]`.
    pub id: String,
    /// `upload_url` — цель PUT файла.
    pub upload_url: String,
    /// `upload_filename` — имя, которое пойдёт в сообщение.
    pub upload_filename: String,
}

/// Готовое вложение для `POST /channels/{id}/messages` (§1.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadedAttachment {
    /// Локальный id файла.
    pub id: String,
    /// Исходное имя файла.
    pub filename: String,
    /// `uploaded_filename` — имя после загрузки.
    pub uploaded_filename: String,
}

/// Обновлённая ссылка на вложение из ответа `POST /attachments/refresh-urls`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshedAttachmentUrl {
    /// Исходный URL вложения.
    pub original: String,
    /// Обновлённый URL с новыми HMAC/экспирацией.
    pub refreshed: String,
}

/// Ошибка загрузки вложений; не несёт токен и тело файла.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AttachmentError {
    /// Некорректный аргумент: id канала, список файлов, имя или размер.
    InvalidArg,
    /// Файл больше лимита: локальная предпроверка или `413`/`40005`.
    EntityTooLarge,
    /// 401: сессия истекла.
    Unauthorized,
    /// 403: нет права загружать в канал.
    Forbidden,
    /// 404: канал не найден.
    NotFound,
    /// 429: ограничение частоты.
    RateLimited {
        /// Задержка из заголовка ответа; 0 — заголовка нет.
        retry_after_ms: u64,
    },
    /// Прочие 4xx (кроме 401/403/404/429/413).
    Client {
        /// HTTP-статус.
        status: u16,
    },
    /// 5xx: ошибка сервера Discord.
    Server {
        /// HTTP-статус.
        status: u16,
    },
    /// Транспорт/таймаут — операция не выполнена.
    Network,
    /// 2xx или тело ответа не соответствуют контракту.
    InvalidResponse,
}

impl fmt::Display for AttachmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArg => formatter.write_str("некорректные параметры загрузки вложения"),
            Self::EntityTooLarge => formatter.write_str("файл больше допустимого размера"),
            Self::Unauthorized => formatter.write_str("сессия истекла"),
            Self::Forbidden => formatter.write_str("нет права загружать в канал"),
            Self::NotFound => formatter.write_str("канал не найден"),
            Self::RateLimited { retry_after_ms } => {
                write!(
                    formatter,
                    "ограничение частоты; повтор через {retry_after_ms} мс"
                )
            }
            Self::Client { status } => write!(formatter, "запрос отклонён (HTTP {status})"),
            Self::Server { status } => {
                write!(formatter, "ошибка сервера Discord (HTTP {status})")
            }
            Self::Network => formatter.write_str("сеть недоступна"),
            Self::InvalidResponse => formatter.write_str("неожиданный ответ запроса слотов"),
        }
    }
}

impl std::error::Error for AttachmentError {}

impl From<RestFailure> for AttachmentError {
    fn from(failure: RestFailure) -> Self {
        match failure {
            RestFailure::Unauthorized => Self::Unauthorized,
            RestFailure::Forbidden => Self::Forbidden,
            RestFailure::NotFound => Self::NotFound,
            RestFailure::RateLimited { retry_after_ms } => Self::RateLimited { retry_after_ms },
            RestFailure::ClientOther { status } => Self::Client { status },
            RestFailure::Server { status } => Self::Server { status },
            RestFailure::Network => Self::Network,
            RestFailure::InvalidResponse => Self::InvalidResponse,
        }
    }
}

/// Транспорт запроса слотов: один POST и один PUT на `upload_url`. Фейк в тестах фиксирует target и тело.
pub trait AttachmentTransport: Send + Sync {
    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, AttachmentError>> + Send + 'a>>;

    fn put<'a>(
        &'a self,
        upload_url: &'a str,
        content_type: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, AttachmentError>> + Send + 'a>> {
        let _ = (upload_url, content_type, body);
        Box::pin(async move { Err(AttachmentError::Network) })
    }
}

struct ApiAttachmentTransport(ApiClient);

impl AttachmentTransport for ApiAttachmentTransport {
    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, AttachmentError>> + Send + 'a>> {
        Box::pin(async move {
            self.0
                .post(target, body)
                .await
                .map_err(|error| AttachmentError::from(RestFailure::from_api_error(&error)))
        })
    }

    fn put<'a>(
        &'a self,
        upload_url: &'a str,
        content_type: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, AttachmentError>> + Send + 'a>> {
        Box::pin(async move {
            let (base, target) = parse_https_upload_url(upload_url)?;
            let mime = normalize_content_type(content_type)?;
            let request = HttpRequest::new(HttpMethod::Put, target)
                .header("Content-Type", mime)
                .body(body.to_vec());
            send_with(&HttpClientConfig::default(), &base, &request)
                .await
                .map_err(|_| AttachmentError::Network)
        })
    }
}

/// REST-клиент слотов загрузки через ядро.
pub struct DiscordAttachments {
    transport: Arc<dyn AttachmentTransport>,
}

impl fmt::Debug for DiscordAttachments {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DiscordAttachments")
            .finish_non_exhaustive()
    }
}

impl DiscordAttachments {
    /// Клиент с боевым транспортом; токен — только в auth-заголовке.
    pub fn new(token: &str) -> Result<Self, AttachmentError> {
        if token.trim().is_empty() {
            return Err(AttachmentError::Unauthorized);
        }
        let identity = crate::auth::IdentityContext::detect();
        let auth = ApiAuth::from_token(
            token,
            &identity.super_properties,
            &identity.locale,
            &identity.timezone,
        )
        .map_err(|_| AttachmentError::InvalidArg)?;
        let client = ApiClient::new(ApiConfig {
            base: API_V9_BASE.to_string(),
            auth,
            http: HttpClientConfig::default(),
            // §1.2: у запроса слотов свои retry-опции на 12 попыток; политику
            // повторов держит ядро, локально её не дублируем.
            retry: ApiRetryPolicy::default(),
        })
        .map_err(|_| AttachmentError::Network)?;
        Ok(Self {
            transport: Arc::new(ApiAttachmentTransport(client)),
        })
    }

    #[doc(hidden)]
    pub fn with_transport(transport: Arc<dyn AttachmentTransport>) -> Self {
        Self { transport }
    }

    /// Запросить слоты загрузки для списка файлов (базовый лимит `GuildBoostTier::None`).
    pub async fn request_upload_slots(
        &self,
        channel_id: &str,
        files: &[AttachmentFileSpec],
    ) -> Result<Vec<AttachmentUploadSlot>, AttachmentError> {
        crate::attachments::request_upload_slots(self, channel_id, files).await
    }

    /// Запросить слоты загрузки с учётом уровня буста сервера (`GuildBoostTier`).
    pub async fn request_upload_slots_with_tier(
        &self,
        channel_id: &str,
        tier: GuildBoostTier,
        files: &[AttachmentFileSpec],
    ) -> Result<Vec<AttachmentUploadSlot>, AttachmentError> {
        crate::attachments::request_upload_slots_with_tier(self, channel_id, tier, files).await
    }

    /// Загрузить байты вложения методом `PUT` по `upload_url` из слота (§1.4).
    pub async fn upload_attachment_bytes(
        &self,
        upload_url: &str,
        content_type: &str,
        data: &[u8],
    ) -> Result<(), AttachmentError> {
        crate::attachments::upload_attachment_bytes(self, upload_url, content_type, data).await
    }

    /// Обновить истёкшие CDN-ссылки на вложения (`POST /attachments/refresh-urls`).
    pub async fn refresh_attachment_urls(
        &self,
        attachment_urls: &[String],
    ) -> Result<Vec<RefreshedAttachmentUrl>, AttachmentError> {
        crate::attachments::refresh_attachment_urls(self, attachment_urls).await
    }
}

impl AttachmentTransport for DiscordAttachments {
    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, AttachmentError>> + Send + 'a>> {
        self.transport.post(target, body)
    }

    fn put<'a>(
        &'a self,
        upload_url: &'a str,
        content_type: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, AttachmentError>> + Send + 'a>> {
        self.transport.put(upload_url, content_type, body)
    }
}

/// Буфер байтов вложения из IPC (поддерживает как JSON-массив `[u8]`, так и
/// объект `Uint8Array` вида `{"0": 10, "1": 20}`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct UploadBytes(pub Vec<u8>);

impl UploadBytes {
    /// Извлечь вектор байтов.
    #[must_use]
    pub fn into_vec(self) -> Vec<u8> {
        self.0
    }

    /// Срез байтов.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

impl From<Vec<u8>> for UploadBytes {
    fn from(value: Vec<u8>) -> Self {
        Self(value)
    }
}

impl From<&[u8]> for UploadBytes {
    fn from(value: &[u8]) -> Self {
        Self(value.to_vec())
    }
}

impl AsRef<[u8]> for UploadBytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl<'de> Deserialize<'de> for UploadBytes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_upload_bytes(deserializer).map(Self)
    }
}

fn decode_base64_str(encoded: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(encoded.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u8;
    for byte in encoded.bytes() {
        if byte.is_ascii_whitespace() {
            continue;
        }
        if byte == b'=' {
            break;
        }
        let val = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        };
        buf = (buf << 6) | u32::from(val);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xFF) as u8);
        }
    }
    Some(out)
}

fn deserialize_upload_bytes<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Option::<serde_json::Value>::deserialize(deserializer)?;
    let Some(value) = raw else {
        return Ok(Vec::new());
    };
    match value {
        serde_json::Value::Null => Ok(Vec::new()),
        serde_json::Value::String(s) => {
            if let Some(b64) = s.strip_prefix("base64:") {
                decode_base64_str(b64).ok_or_else(|| {
                    serde::de::Error::custom("некорректная строка base64 во вложении")
                })
            } else {
                Err(serde::de::Error::custom(
                    "ожидался массив байтов, base64-строка или сериализованный Uint8Array",
                ))
            }
        }
        serde_json::Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                let number = item
                    .as_u64()
                    .and_then(|val| u8::try_from(val).ok())
                    .ok_or_else(|| {
                        serde::de::Error::custom("ожидался байт 0..=255 в массиве вложения")
                    })?;
                out.push(number);
            }
            Ok(out)
        }
        serde_json::Value::Object(map) => {
            let mut out = Vec::with_capacity(map.len());
            for index in 0..map.len() {
                let key = index.to_string();
                let item = map.get(&key).ok_or_else(|| {
                    serde::de::Error::custom(
                        "ожидались последовательные индексы 0..len в Uint8Array",
                    )
                })?;
                let number = item
                    .as_u64()
                    .and_then(|val| u8::try_from(val).ok())
                    .ok_or_else(|| {
                        serde::de::Error::custom("ожидался байт 0..=255 в Uint8Array")
                    })?;
                out.push(number);
            }
            Ok(out)
        }
        _ => Err(serde::de::Error::custom(
            "ожидался массив байтов или сериализованный Uint8Array",
        )),
    }
}

/// Входные данные IPC-команды `attachments_upload_put`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadPutInput {
    /// Идентификатор канала (опционально; если задан — проверяется по allowlist).
    #[serde(default, alias = "channel_id")]
    pub channel_id: Option<String>,
    /// Локальный идентификатор слота (`files[].id`, опционально).
    #[serde(default, alias = "slot_id", alias = "id")]
    pub slot_id: Option<String>,
    /// Pre-signed HTTPS URL загрузки из `AttachmentUploadSlot::upload_url`.
    #[serde(default, alias = "upload_url")]
    pub upload_url: String,
    /// MIME-тип содержимого (при пустом значении — `application/octet-stream`).
    #[serde(default, alias = "content_type")]
    pub content_type: String,
    /// Сырые байты загружаемого файла.
    #[serde(default, deserialize_with = "deserialize_upload_bytes")]
    pub data: Vec<u8>,
    /// Альтернативное поле байтов (`bytes` из UI-модели `composer.js`), если `data` пусто.
    #[serde(default, deserialize_with = "deserialize_upload_bytes")]
    pub bytes: Vec<u8>,
}

impl UploadPutInput {
    /// Возвращает непустой буфер байтов (`data`, либо `bytes` если `data` пуст).
    #[must_use]
    pub fn payload_bytes(&self) -> &[u8] {
        if !self.data.is_empty() {
            &self.data
        } else {
            &self.bytes
        }
    }

    /// Собрать и предварительно провалидировать [`UploadPutInput`] из аргументов Tauri IPC.
    pub fn from_ipc(
        payload: Option<Self>,
        slot_id: Option<String>,
        channel_id: Option<String>,
        upload_url: Option<String>,
        content_type: Option<String>,
        data: Option<UploadBytes>,
        bytes: Option<UploadBytes>,
    ) -> Result<Self, AttachmentError> {
        let mut input = if let Some(mut item) = payload {
            if item.slot_id.is_none() {
                item.slot_id = slot_id;
            }
            if item.channel_id.is_none() {
                item.channel_id = channel_id;
            }
            if item.upload_url.is_empty() {
                item.upload_url = upload_url.unwrap_or_default();
            }
            if item.content_type.is_empty() {
                item.content_type = content_type.unwrap_or_default();
            }
            if item.data.is_empty() {
                item.data = data.map_or_else(Vec::new, UploadBytes::into_vec);
            }
            if item.bytes.is_empty() {
                item.bytes = bytes.map_or_else(Vec::new, UploadBytes::into_vec);
            }
            item
        } else {
            Self {
                channel_id,
                slot_id,
                upload_url: upload_url.unwrap_or_default(),
                content_type: content_type.unwrap_or_default(),
                data: data.map_or_else(Vec::new, UploadBytes::into_vec),
                bytes: bytes.map_or_else(Vec::new, UploadBytes::into_vec),
            }
        };
        if let Some(cid) = input.channel_id.as_deref() {
            validate_channel_id(cid)?;
        }
        if input
            .slot_id
            .as_deref()
            .is_some_and(|sid| sid.trim().is_empty())
        {
            return Err(AttachmentError::InvalidArg);
        }
        if input.data.is_empty() && !input.bytes.is_empty() {
            input.data = input.bytes.clone();
        }
        if input.data.len() <= 1 {
            if let Ok(guard) = clipboard_bytes_cache().lock() {
                if let Some(sid) = input.slot_id.as_deref() {
                    if let Some(cached) = guard.get(sid) {
                        input.data = cached.clone();
                        input.bytes = cached.clone();
                    }
                }
                if input.data == [0] {
                    if let Some(cached) = guard.get("__latest__") {
                        input.data = cached.clone();
                        input.bytes = cached.clone();
                    }
                }
            }
        }
        Ok(input)
    }
}

/// Канонический список параметров `#[tauri::command] async fn attachments_upload_put`
/// в `main.rs` (кроме `backend: tauri::State`) и их `camelCase`-ключей в `InvokeBody::Json`.
pub const UPLOAD_PUT_COMMAND_PARAM_KEYS: [(&str, &str); 8] = [
    ("payload", "payload"),
    ("slot_id", "slotId"),
    ("id", "id"),
    ("channel_id", "channelId"),
    ("upload_url", "uploadUrl"),
    ("content_type", "contentType"),
    ("data", "data"),
    ("bytes", "bytes"),
];

/// Аргументы Tauri-команды `attachments_upload_put` на уровне Tauri command extraction.
///
/// В Tauri v2 (`tauri::ipc::CommandItem`) каждый параметр `#[tauri::command]`
/// типа `Option<T>` извлекается по своему `camelCase`-ключу из верхнеуровневого
/// JSON-объекта `invoke("attachments_upload_put", putPayload)`:
/// `match payload.get(key) { Some(v) => v.deserialize_option(visitor), None => visitor.visit_none() }`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadPutCommandArgs {
    /// Опциональный вложенный объект `payload` (для вызовов `{ payload: { ... } }`).
    #[serde(default)]
    pub payload: Option<UploadPutInput>,
    /// Идентификатор слота при передаче поля `slotId`.
    #[serde(default)]
    pub slot_id: Option<String>,
    /// Идентификатор слота при передаче плоского `putPayload` из `composer.js` (`id: slot.id`).
    #[serde(default)]
    pub id: Option<String>,
    /// Идентификатор канала (`channelId`).
    #[serde(default)]
    pub channel_id: Option<String>,
    /// Pre-signed HTTPS URL загрузки (`uploadUrl`).
    #[serde(default)]
    pub upload_url: Option<String>,
    /// MIME-тип содержимого (`contentType`).
    #[serde(default)]
    pub content_type: Option<String>,
    /// Сырые байты файла (`data` — массив `[u8]` или индексный объект `Uint8Array`).
    #[serde(default)]
    pub data: Option<UploadBytes>,
    /// Альтернативное поле сырых байтов (`bytes` из `composer.js`).
    #[serde(default)]
    pub bytes: Option<UploadBytes>,
}

fn extract_tauri_optional_arg<'de, T: Deserialize<'de>>(
    invoke_payload: &'de serde_json::Value,
    key: &str,
) -> Result<Option<T>, AttachmentError> {
    let Some(obj) = invoke_payload.as_object() else {
        return Err(AttachmentError::InvalidArg);
    };
    match obj.get(key) {
        Some(value) => Option::<T>::deserialize(value).map_err(|_| AttachmentError::InvalidArg),
        None => Ok(None),
    }
}

impl UploadPutCommandArgs {
    /// Извлечь аргументы команды `attachments_upload_put` из плоского JSON-объекта
    /// `invoke("attachments_upload_put", putPayload)` в точности по алгоритму
    /// `tauri::ipc::CommandItem::deserialize_option` для каждого параметра `#[tauri::command]`.
    pub fn from_tauri_ipc(invoke_payload: &serde_json::Value) -> Result<Self, AttachmentError> {
        if !invoke_payload.is_object() {
            return Err(AttachmentError::InvalidArg);
        }
        Ok(Self {
            payload: extract_tauri_optional_arg(invoke_payload, "payload")?,
            slot_id: extract_tauri_optional_arg(invoke_payload, "slotId")?,
            id: extract_tauri_optional_arg(invoke_payload, "id")?,
            channel_id: extract_tauri_optional_arg(invoke_payload, "channelId")?,
            upload_url: extract_tauri_optional_arg(invoke_payload, "uploadUrl")?,
            content_type: extract_tauri_optional_arg(invoke_payload, "contentType")?,
            data: extract_tauri_optional_arg(invoke_payload, "data")?,
            bytes: extract_tauri_optional_arg(invoke_payload, "bytes")?,
        })
    }

    /// Преобразовать извлечённые аргументы команды в валидированный [`UploadPutInput`].
    pub fn into_input(self) -> Result<UploadPutInput, AttachmentError> {
        UploadPutInput::from_ipc(
            self.payload,
            self.slot_id.or(self.id),
            self.channel_id,
            self.upload_url,
            self.content_type,
            self.data,
            self.bytes,
        )
    }
}

/// Выполнить команду `attachments_upload_put` с извлечёнными Tauri-аргументами [`UploadPutCommandArgs`].
pub async fn upload_put_command<S: AttachmentTransport + ?Sized>(
    sender: &S,
    args: UploadPutCommandArgs,
) -> Result<(), AttachmentError> {
    let input = args.into_input()?;
    upload_attachment_bytes(
        sender,
        &input.upload_url,
        &input.content_type,
        input.payload_bytes(),
    )
    .await
}

/// Загрузить байты вложения методом `PUT` по `upload_url` из слота (базовый лимит `GuildBoostTier::None`).
///
/// Проверяет до обращения к сети:
/// - непустой `upload_url` со схемой `https://` и валидным хостом (`http://` и пустой URL → `InvalidArg`);
/// - непустой буфер байтов `data` в пределах `MAX_UPLOAD_FILE_BYTES`;
/// - отсутствие управляющих символов в `content_type` (пустой тип заменяется на `application/octet-stream`).
pub async fn upload_attachment_bytes<S: AttachmentTransport + ?Sized>(
    sender: &S,
    upload_url: &str,
    content_type: &str,
    data: &[u8],
) -> Result<(), AttachmentError> {
    upload_attachment_bytes_with_tier(sender, upload_url, content_type, data, GuildBoostTier::None)
        .await
}

/// Загрузить байты вложения методом `PUT` по `upload_url` с учётом `GuildBoostTier`.
pub async fn upload_attachment_bytes_with_tier<S: AttachmentTransport + ?Sized>(
    sender: &S,
    upload_url: &str,
    content_type: &str,
    data: &[u8],
    tier: GuildBoostTier,
) -> Result<(), AttachmentError> {
    let trimmed_url = upload_url.trim();
    let _ = parse_https_upload_url(trimmed_url)?;
    let mime = normalize_content_type(content_type)?;
    if data.is_empty() {
        return Err(AttachmentError::InvalidArg);
    }
    if (data.len() as u64) > tier.max_upload_bytes() {
        return Err(AttachmentError::EntityTooLarge);
    }
    let response = sender.put(trimmed_url, mime, data).await?;
    decode_put(response)
}

/// Валидировать [`UploadPutInput`] и отправить байты вложения методом `PUT` на `upload_url`.
pub async fn upload_put_input<S: AttachmentTransport + ?Sized>(
    sender: &S,
    input: &UploadPutInput,
) -> Result<(), AttachmentError> {
    if let Some(channel_id) = input.channel_id.as_deref() {
        validate_channel_id(channel_id)?;
    }
    if input
        .slot_id
        .as_deref()
        .is_some_and(|slot_id| slot_id.trim().is_empty())
    {
        return Err(AttachmentError::InvalidArg);
    }
    upload_attachment_bytes(
        sender,
        &input.upload_url,
        &input.content_type,
        input.payload_bytes(),
    )
    .await
}

/// Запросить слоты загрузки (базовый лимит `GuildBoostTier::None`); валидация — до обращения к сети.
pub async fn request_upload_slots<S: AttachmentTransport + ?Sized>(
    sender: &S,
    channel_id: &str,
    files: &[AttachmentFileSpec],
) -> Result<Vec<AttachmentUploadSlot>, AttachmentError> {
    request_upload_slots_with_tier(sender, channel_id, GuildBoostTier::None, files).await
}

/// Запросить слоты загрузки с учётом уровня буста сервера (`GuildBoostTier`); валидация — до обращения к сети.
pub async fn request_upload_slots_with_tier<S: AttachmentTransport + ?Sized>(
    sender: &S,
    channel_id: &str,
    tier: GuildBoostTier,
    files: &[AttachmentFileSpec],
) -> Result<Vec<AttachmentUploadSlot>, AttachmentError> {
    validate_channel_id(channel_id)?;
    let resolved = resolve_clipboard_or_uri_files(files);
    validate_files(&resolved, tier)?;
    let path = render_path(channel_id);
    let body = encode_files(&resolved)?;
    let response = sender.post(&path, &body).await?;
    decode(response)
}

/// Обновить истёкшие CDN-ссылки на вложения через `POST /attachments/refresh-urls`.
///
/// Перед обращением к сети проверяет:
/// - список ссылок не пуст и содержит не более [`MAX_REFRESH_URLS_PER_REQUEST`] (50) элементов;
/// - каждая ссылка — валидный HTTPS URL со схемой `https://` без пробелов, управляющих символов и `#`;
/// - в списке отсутствуют дубликаты.
pub async fn refresh_attachment_urls<S: AttachmentTransport + ?Sized>(
    sender: &S,
    attachment_urls: &[String],
) -> Result<Vec<RefreshedAttachmentUrl>, AttachmentError> {
    validate_refresh_urls(attachment_urls)?;
    let body = serde_json::to_vec(&RefreshUrlsBody { attachment_urls })
        .map_err(|_| AttachmentError::InvalidArg)?;
    let response = sender.post(REFRESH_ATTACHMENT_URLS_PATH, &body).await?;
    decode_refresh(response)
}

/// Проверка списка URL для обновления до обращения к сети.
fn validate_refresh_urls(urls: &[String]) -> Result<(), AttachmentError> {
    if urls.is_empty() || urls.len() > MAX_REFRESH_URLS_PER_REQUEST {
        return Err(AttachmentError::InvalidArg);
    }
    let mut seen = HashSet::with_capacity(urls.len());
    for url in urls {
        if url.trim().is_empty()
            || url.bytes().any(|byte| byte <= 0x20 || byte >= 0x7f)
            || !seen.insert(url.as_str())
        {
            return Err(AttachmentError::InvalidArg);
        }
        parse_https_upload_url(url)?;
    }
    Ok(())
}

/// Тело запроса `POST /attachments/refresh-urls`.
#[derive(Serialize)]
struct RefreshUrlsBody<'a> {
    attachment_urls: &'a [String],
}

/// Тело ответа `POST /attachments/refresh-urls`.
#[derive(Deserialize)]
struct RefreshUrlsResponse {
    refreshed_urls: Vec<RefreshedAttachmentUrl>,
}

/// Разбор статуса ответа `POST /attachments/refresh-urls`.
fn decode_refresh(response: HttpResponse) -> Result<Vec<RefreshedAttachmentUrl>, AttachmentError> {
    match response.status {
        429 => Err(AttachmentError::RateLimited {
            retry_after_ms: retry_after_ms(&response.headers),
        }),
        status if (200..300).contains(&status) => parse_refreshed_urls(&response.body),
        status => Err(RestFailure::from_status(status)
            .map_or(AttachmentError::Network, AttachmentError::from)),
    }
}

/// Разбор и валидация тела ответа `refreshed_urls[]`:
/// пустой `original`/`refreshed` или не-https `refreshed` URL отклоняются как `InvalidResponse`.
fn parse_refreshed_urls(body: &[u8]) -> Result<Vec<RefreshedAttachmentUrl>, AttachmentError> {
    let payload: RefreshUrlsResponse =
        serde_json::from_slice(body).map_err(|_| AttachmentError::InvalidResponse)?;
    for item in &payload.refreshed_urls {
        if item.original.trim().is_empty()
            || item.refreshed.trim().is_empty()
            || parse_https_upload_url(&item.refreshed).is_err()
        {
            return Err(AttachmentError::InvalidResponse);
        }
    }
    Ok(payload.refreshed_urls)
}

/// Собрать вложения сообщения по парам «слот + файл».
///
/// Порядок результата — порядок `specs`; пара ищется по локальному `id`.
/// Лишний/недостающий/повторный слот — `InvalidArg`, а не молчаливая подмена:
/// иначе в сообщение уйдёт чужой `uploaded_filename`.
pub fn build_message_attachments(
    slots: &[AttachmentUploadSlot],
    specs: &[AttachmentFileSpec],
) -> Result<Vec<UploadedAttachment>, AttachmentError> {
    if slots.is_empty() || specs.is_empty() || slots.len() != specs.len() {
        return Err(AttachmentError::InvalidArg);
    }

    let mut by_id = HashMap::with_capacity(slots.len());
    for slot in slots {
        if slot.id.trim().is_empty()
            || slot.upload_url.trim().is_empty()
            || slot.upload_filename.trim().is_empty()
        {
            return Err(AttachmentError::InvalidArg);
        }
        if by_id.insert(slot.id.as_str(), slot).is_some() {
            return Err(AttachmentError::InvalidArg);
        }
    }

    let mut built = Vec::with_capacity(specs.len());
    let mut seen = HashSet::with_capacity(specs.len());
    for spec in specs {
        if spec.filename.trim().is_empty() || spec.id.trim().is_empty() {
            return Err(AttachmentError::InvalidArg);
        }
        if !seen.insert(spec.id.as_str()) {
            return Err(AttachmentError::InvalidArg);
        }
        let slot = by_id
            .get(spec.id.as_str())
            .ok_or(AttachmentError::InvalidArg)?;
        built.push(UploadedAttachment {
            id: spec.id.clone(),
            filename: spec.filename.clone(),
            uploaded_filename: slot.upload_filename.clone(),
        });
    }
    Ok(built)
}

/// Allowlist как в `messages::validate_channel_id`: ASCII-цифры/буквы, `_`, `-`,
/// 1..=64 байт. Отсекает `../..`, слэши, `%`, `?`, `#`, CR/LF до I/O.
fn validate_channel_id(channel_id: &str) -> Result<(), AttachmentError> {
    let bytes = channel_id.as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_CHANNEL_ID_BYTES {
        return Err(AttachmentError::InvalidArg);
    }
    if !bytes
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_' || *byte == b'-')
    {
        return Err(AttachmentError::InvalidArg);
    }
    Ok(())
}

/// Проверка списка файлов до сети: пустой/длинный список, пустые имена и id,
/// дубликаты, нулевой размер и превышение лимита `tier.max_upload_bytes()`.
fn validate_files(
    files: &[AttachmentFileSpec],
    tier: GuildBoostTier,
) -> Result<(), AttachmentError> {
    if files.is_empty() || files.len() > MAX_ATTACHMENTS_PER_MESSAGE {
        return Err(AttachmentError::InvalidArg);
    }
    let max_bytes = tier.max_upload_bytes();
    let mut seen = HashSet::with_capacity(files.len());
    for file in files {
        if file.filename.trim().is_empty() || file.id.trim().is_empty() || file.file_size == 0 {
            return Err(AttachmentError::InvalidArg);
        }
        if !seen.insert(file.id.as_str()) {
            return Err(AttachmentError::InvalidArg);
        }
        if file.file_size > max_bytes {
            return Err(AttachmentError::EntityTooLarge);
        }
    }
    Ok(())
}

fn render_path(channel_id: &str) -> String {
    ATTACHMENTS_PATH_TEMPLATE.replace("{id}", channel_id)
}

/// Тело запроса ровно из четырёх полей §1.3; лишние ключи сервер не ждёт.
#[derive(Serialize)]
struct FilesBody<'a> {
    files: Vec<WireFile<'a>>,
}

#[derive(Serialize)]
struct WireFile<'a> {
    filename: &'a str,
    file_size: u64,
    id: &'a str,
    is_clip: bool,
}

fn encode_files(files: &[AttachmentFileSpec]) -> Result<Vec<u8>, AttachmentError> {
    let wire = FilesBody {
        files: files
            .iter()
            .map(|file| WireFile {
                filename: &file.filename,
                file_size: file.file_size,
                id: &file.id,
                is_clip: file.is_clip,
            })
            .collect(),
    };
    serde_json::to_vec(&wire).map_err(|_| AttachmentError::InvalidArg)
}

#[derive(Deserialize)]
struct SlotsResponse {
    attachments: Vec<WireSlot>,
}

fn deserialize_slot_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let val = serde_json::Value::deserialize(deserializer)?;
    match val {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        _ => Err(serde::de::Error::custom(
            "ожидался строковый или числовой id слота",
        )),
    }
}

#[derive(Deserialize)]
struct WireSlot {
    #[serde(deserialize_with = "deserialize_slot_id")]
    id: String,
    upload_url: String,
    upload_filename: String,
}

/// Статус → типизированный результат по §1.6: `413` и `40005` — один исход
/// (`EntityTooLarge`), `429` несёт `retry-after` из заголовка.
fn decode(response: HttpResponse) -> Result<Vec<AttachmentUploadSlot>, AttachmentError> {
    match response.status {
        429 => Err(AttachmentError::RateLimited {
            retry_after_ms: retry_after_ms(&response.headers),
        }),
        413 => Err(AttachmentError::EntityTooLarge),
        status if (200..300).contains(&status) => parse_slots(&response.body),
        400 if entity_too_large_code(&response.body) => Err(AttachmentError::EntityTooLarge),
        status => Err(RestFailure::from_status(status)
            .map_or(AttachmentError::Network, AttachmentError::from)),
    }
}

/// Разбор и валидация `upload_url`: только `https://` с непустым хостом и валидным путём.
/// Отсекает пустой URL, `http://`, относительные пути, пробелы, `#` и `@` до сети.
fn parse_https_upload_url(upload_url: &str) -> Result<(String, String), AttachmentError> {
    let url = upload_url.trim();
    let Some(rest) = url.strip_prefix("https://") else {
        return Err(AttachmentError::InvalidArg);
    };
    if rest.is_empty()
        || rest
            .bytes()
            .any(|byte| !(0x21..=0x7e).contains(&byte) || byte == b'#')
    {
        return Err(AttachmentError::InvalidArg);
    }
    let (authority, target) = match rest.find('/') {
        Some(index) => (&rest[..index], &rest[index..]),
        None => (rest, "/"),
    };
    if authority.is_empty() || authority.contains('?') || authority.contains('@') {
        return Err(AttachmentError::InvalidArg);
    }
    let base = format!("https://{authority}");
    let endpoint = Endpoint::parse(&base).map_err(|_| AttachmentError::InvalidArg)?;
    let normalized_target = endpoint
        .target(target)
        .map_err(|_| AttachmentError::InvalidArg)?;
    Ok((base, normalized_target))
}

/// Валидация и нормализация `Content-Type` для `PUT` (§1.4): управляющие символы запрещены,
/// пустое значение заменяется на `application/octet-stream`.
fn normalize_content_type(content_type: &str) -> Result<&str, AttachmentError> {
    if content_type.bytes().any(|byte| byte < 0x20 || byte == 0x7f) {
        return Err(AttachmentError::InvalidArg);
    }
    let trimmed = content_type.trim();
    if trimmed.is_empty() {
        Ok(DEFAULT_UPLOAD_CONTENT_TYPE)
    } else {
        Ok(trimmed)
    }
}

/// Статус ответа `PUT {upload_url}` → типизированный результат: `200`/`201`/`204` → `Ok(())`,
/// `413` и `400 + 40005` → `EntityTooLarge`, `429` → `RateLimited { retry_after_ms }`,
/// `5xx` → `Server { status }`.
fn decode_put(response: HttpResponse) -> Result<(), AttachmentError> {
    match response.status {
        200 | 201 | 204 => Ok(()),
        429 => Err(AttachmentError::RateLimited {
            retry_after_ms: retry_after_ms(&response.headers),
        }),
        413 => Err(AttachmentError::EntityTooLarge),
        400 if entity_too_large_code(&response.body) => Err(AttachmentError::EntityTooLarge),
        status if (200..300).contains(&status) => Ok(()),
        status => Err(RestFailure::from_status(status)
            .map_or(AttachmentError::Network, AttachmentError::from)),
    }
}

/// `retry-after` в секундах; нечисловое или отрицательное значение — 0,
/// задержку не выдумываем (контракт G-03a).
fn retry_after_ms(headers: &[(String, String)]) -> u64 {
    headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("retry-after"))
        .and_then(|(_, value)| value.trim().parse::<f64>().ok())
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
        .map_or(0, |seconds| (seconds * 1000.0).round() as u64)
}

/// `{"code":40005}` в теле ответа; неразобранное тело — не `EntityTooLarge`.
fn entity_too_large_code(body: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| value.get("code").and_then(serde_json::Value::as_u64))
        .is_some_and(|code| code == ENTITY_TOO_LARGE_CODE)
}

/// Разбор `attachments[]`: пустой список и пустые поля — `InvalidResponse`.
fn parse_slots(body: &[u8]) -> Result<Vec<AttachmentUploadSlot>, AttachmentError> {
    let response: SlotsResponse =
        serde_json::from_slice(body).map_err(|_| AttachmentError::InvalidResponse)?;
    if response.attachments.is_empty() {
        return Err(AttachmentError::InvalidResponse);
    }
    let mut slots = Vec::with_capacity(response.attachments.len());
    for slot in response.attachments {
        if slot.id.trim().is_empty()
            || slot.upload_url.trim().is_empty()
            || slot.upload_filename.trim().is_empty()
        {
            return Err(AttachmentError::InvalidResponse);
        }
        slots.push(AttachmentUploadSlot {
            id: slot.id,
            upload_url: slot.upload_url,
            upload_filename: slot.upload_filename,
        });
    }
    Ok(slots)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_after_accepts_seconds_and_fractions() {
        let header = |value: &str| vec![("Retry-After".to_string(), value.to_string())];
        assert_eq!(retry_after_ms(&header("3")), 3000);
        assert_eq!(retry_after_ms(&header("1.5")), 1500);
        assert_eq!(retry_after_ms(&header(" 2 ")), 2000);
        assert_eq!(retry_after_ms(&header("soon")), 0);
        assert_eq!(retry_after_ms(&header("-1")), 0);
        assert_eq!(retry_after_ms(&[]), 0);
    }

    #[test]
    fn entity_too_large_code_reads_body_code() {
        assert!(entity_too_large_code(br#"{"code":40005}"#));
        assert!(!entity_too_large_code(br#"{"code":50035}"#));
        assert!(!entity_too_large_code(b""));
        assert!(!entity_too_large_code(b"[]"));
    }
}
