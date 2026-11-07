//! RW-40 / Task-112 (N1-07): личные сообщения и друзья (`dm_friends`).
//!
//! Реализует контракт подтверждённого RE-оракула `waves/RW-40/DM-FRIENDS-RE.md` и
//! спецификации `waves/RW-40/DESIGN/DM-FRIENDS-SPEC.md`:
//! 1. Бессетевой разбор `READY.private_channels` (типы `1 = DM` и `3 = GROUP_DM`,
//!    разрешение `recipient_ids` через `READY.users`, сортировка по убыванию
//!    числового `u64` `last_message_id`, `0` вызовов `GET /users/@me/channels`).
//! 2. Разбор `READY.relationships` (типы `1 = FRIEND`, `2 = BLOCKED`,
//!    `3 = PENDING_INCOMING`, `4 = PENDING_OUTGOING`), расчёт счётчиков
//!    (`RelationshipStore` @9318383, @9319095) и фильтрация по вкладкам
//!    `Online / All / Pending / Blocked` с учётом `003-READY_SUPPLEMENTAL.json`.
//! 3. REST-клиент:
//!    - `POST /api/v9/users/@me/channels` (`/users/@me/channels` относительно `API_V9_BASE`)
//!      с телом `{"recipients": [...]}` для `1..=9` получателей и предсетевой
//!      валидацией snowflake ID (`0` сетевых вызовов при невалидном ID);
//!    - `POST /api/v9/users/@me/channels/group-dm-shell` с `{"recipient_id": "..."}`;
//!    - `PUT /api/v9/users/@me/relationships/{user_id}` (принятие заявки `{"type": 1, "confirm_stranger_request": ...}`
//!      или блокировка `{"type": 2}`) с типизированным перехватом кода `80013`
//!      (`RELATIONSHIP_INVALID_NO_CONFIRMATION` — Stranger Friend Request Confirmation);
//!    - `DELETE /api/v9/users/@me/relationships/{user_id}` (удаление/отклонение/разблокировка);
//!    - `POST /api/v9/users/@me/relationships` с поддержкой Pomelo (`"discriminator": null`)
//!      и классического `username#1234` (`"discriminator": 1234`).
//! 4. Защита от утечки токена в `Display` и `Debug` ошибок и клиента.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use discord_core::rest::client::{ApiAuth, ApiClient, ApiConfig, ApiRetryPolicy};
use discord_core::rest::transport::{HttpClientConfig, HttpResponse};
use discord_core::API_V9_BASE;
use serde::{Deserialize, Serialize};

use crate::rest_failure::RestFailure;

/// Относительный маршрут создания/открытия DM или Group DM в `API_V9_BASE` (`rest-static.json`: `USER_CHANNELS`).
pub const USER_CHANNELS_PATH: &str = "/users/@me/channels";

/// Относительный маршрут создания пустой оболочки Group DM (`rest-static.json`: `USER_GROUP_DM_SHELL`).
pub const USER_GROUP_DM_SHELL_PATH: &str = "/users/@me/channels/group-dm-shell";

/// Относительный маршрут списка/создания связей (`rest-static.json`: `USER_RELATIONSHIPS`).
pub const USER_RELATIONSHIPS_PATH: &str = "/users/@me/relationships";

/// Полный wire-путь `POST /api/v9/users/@me/channels`.
pub const API_V9_USER_CHANNELS_PATH: &str = "/api/v9/users/@me/channels";

/// Полный wire-путь `POST /api/v9/users/@me/relationships`.
pub const API_V9_USER_RELATIONSHIPS_PATH: &str = "/api/v9/users/@me/relationships";

/// Тип приватного канала `1` — одиночная личная переписка (`ChannelTypes.DM`).
pub const CHANNEL_TYPE_DM: i64 = 1;

/// Тип приватного канала `3` — групповая личная переписка (`ChannelTypes.GROUP_DM`).
pub const CHANNEL_TYPE_GROUP_DM: i64 = 3;

/// Бит закреплённого приватного канала (`ChannelFlags.PINNED = 1 << 1`).
pub const CHANNEL_FLAG_PINNED: u64 = 1 << 1;

/// Минимальное число получателей для `POST /users/@me/channels` (одиночная DM).
pub const MIN_DM_RECIPIENTS: usize = 1;

/// Максимальное число получателей для `POST /users/@me/channels` (Group DM: лимит 10 включая автора).
pub const MAX_DM_RECIPIENTS: usize = 9;

/// Тип связи `1` — друг (`RelationshipTypes.FRIEND`).
pub const RELATIONSHIP_TYPE_FRIEND: i64 = 1;

/// Тип связи `2` — заблокированный пользователь (`RelationshipTypes.BLOCKED`).
pub const RELATIONSHIP_TYPE_BLOCKED: i64 = 2;

/// Тип связи `3` — входящая заявка в друзья (`RelationshipTypes.PENDING_INCOMING`).
pub const RELATIONSHIP_TYPE_PENDING_INCOMING: i64 = 3;

/// Тип связи `4` — исходящая заявка в друзья (`RelationshipTypes.PENDING_OUTGOING`).
pub const RELATIONSHIP_TYPE_PENDING_OUTGOING: i64 = 4;

/// Код ошибки Discord API `30004` (`TOO_MANY_RECIPIENTS`).
pub const DISCORD_ERR_TOO_MANY_RECIPIENTS: u64 = 30004;

/// Код ошибки Discord API `80013` (`RELATIONSHIP_INVALID_NO_CONFIRMATION` — Stranger Friend Request Confirmation).
pub const DISCORD_ERR_RELATIONSHIP_INVALID_NO_CONFIRMATION: u64 = 80013;

/// Тип приватного канала из `READY.private_channels` или ответа `POST /users/@me/channels`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrivateChannelType {
    /// Одиночная переписка (`type = 1`).
    Dm,
    /// Групповая переписка (`type = 3`).
    GroupDm,
    /// Неизвестный тип канала для forward-compatibility.
    Unknown(i64),
}

impl PrivateChannelType {
    /// Конструирует [`PrivateChannelType`] из числового кода Discord.
    #[must_use]
    pub const fn from_i64(value: i64) -> Self {
        match value {
            CHANNEL_TYPE_DM => Self::Dm,
            CHANNEL_TYPE_GROUP_DM => Self::GroupDm,
            other => Self::Unknown(other),
        }
    }

    /// Возвращает числовой код типа канала в протоколе Discord.
    #[must_use]
    pub const fn as_i64(self) -> i64 {
        match self {
            Self::Dm => CHANNEL_TYPE_DM,
            Self::GroupDm => CHANNEL_TYPE_GROUP_DM,
            Self::Unknown(v) => v,
        }
    }
}

/// Разрешённый профиль пользователя из `READY.users` или вложенного `recipients[]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedDmUser {
    pub id: String,
    pub username: String,
    pub global_name: Option<String>,
    pub discriminator: String,
    pub avatar: Option<String>,
    pub bot: bool,
}

impl ResolvedDmUser {
    /// Отображаемое имя пользователя (`global_name` с откатом к `username`).
    #[must_use]
    pub fn display_name(&self) -> &str {
        self.global_name.as_deref().unwrap_or(&self.username)
    }
}

/// Запись приватного канала (DM или Group DM) с разрешёнными получателями.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateChannelRecord {
    pub id: String,
    pub channel_type: PrivateChannelType,
    pub flags: u64,
    pub last_message_id: Option<String>,
    pub recipient_ids: Vec<String>,
    pub recipients: Vec<ResolvedDmUser>,
    pub name: Option<String>,
    pub icon: Option<String>,
    pub owner_id: Option<String>,
    pub nicks: BTreeMap<String, String>,
    pub is_message_request: bool,
    pub is_message_request_timestamp: Option<String>,
    pub is_spam: bool,
}

impl PrivateChannelRecord {
    /// Числовое значение `last_message_id` (`0`, если сообщений ещё нет или ID отсутствует).
    #[must_use]
    pub fn last_message_snowflake(&self) -> u64 {
        self.last_message_id
            .as_deref()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0)
    }

    /// Числовое значение собственного snowflake `id` канала для вторичного ключа сортировки.
    #[must_use]
    pub fn channel_snowflake(&self) -> u64 {
        self.id.parse::<u64>().unwrap_or(0)
    }

    /// Признак закреплённого приватного канала (`(flags & (1 << 1)) != 0`).
    #[must_use]
    pub const fn is_pinned(&self) -> bool {
        (self.flags & CHANNEL_FLAG_PINNED) != 0
    }
}

/// Тип связи пользователя из `READY.relationships` (`RelationshipTypes`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationshipType {
    /// Друг (`type = 1`).
    Friend,
    /// Заблокированный пользователь (`type = 2`).
    Blocked,
    /// Входящая заявка в друзья (`type = 3`).
    PendingIncoming,
    /// Исходящая заявка в друзья (`type = 4`).
    PendingOutgoing,
    /// Неизвестный тип связи для forward-compatibility.
    Unknown(i64),
}

impl RelationshipType {
    /// Конструирует [`RelationshipType`] из числового кода Discord.
    #[must_use]
    pub const fn from_i64(value: i64) -> Self {
        match value {
            RELATIONSHIP_TYPE_FRIEND => Self::Friend,
            RELATIONSHIP_TYPE_BLOCKED => Self::Blocked,
            RELATIONSHIP_TYPE_PENDING_INCOMING => Self::PendingIncoming,
            RELATIONSHIP_TYPE_PENDING_OUTGOING => Self::PendingOutgoing,
            other => Self::Unknown(other),
        }
    }

    /// Возвращает числовой код типа связи в протоколе Discord.
    #[must_use]
    pub const fn as_i64(self) -> i64 {
        match self {
            Self::Friend => RELATIONSHIP_TYPE_FRIEND,
            Self::Blocked => RELATIONSHIP_TYPE_BLOCKED,
            Self::PendingIncoming => RELATIONSHIP_TYPE_PENDING_INCOMING,
            Self::PendingOutgoing => RELATIONSHIP_TYPE_PENDING_OUTGOING,
            Self::Unknown(v) => v,
        }
    }
}

/// Запись связи пользователя из `READY.relationships` с разрешённым профилем.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationshipRecord {
    pub id: String,
    pub user_id: String,
    pub relationship_type: RelationshipType,
    pub nickname: Option<String>,
    pub since: Option<String>,
    pub is_spam_request: bool,
    pub user_ignored: bool,
    pub user: Option<ResolvedDmUser>,
}

impl RelationshipRecord {
    /// В `RelationshipStore` (@9319095) пользователь считается ignored только если
    /// `type !== BLOCKED && user_ignored == true`.
    #[must_use]
    pub fn is_ignored(&self) -> bool {
        self.relationship_type != RelationshipType::Blocked && self.user_ignored
    }
}

/// Агрегированные счётчики связей по алгоритму `RelationshipStore` (@9318383).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RelationshipCounts {
    pub friend_count: usize,
    pub blocked_count: usize,
    pub pending_incoming_count: usize,
    pub pending_outgoing_count: usize,
    pub spam_count: usize,
    pub pending_ignored_count: usize,
}

impl RelationshipCounts {
    /// Рассчитывает счётчики вкладок друзей по точным правилам `RelationshipStore`:
    /// `pending_incoming_count = max(incoming_total - spam_count - pending_ignored_count, 0)`.
    #[must_use]
    pub fn from_relationships(relationships: &[RelationshipRecord]) -> Self {
        let mut friend_count = 0usize;
        let mut blocked_count = 0usize;
        let mut incoming_total = 0usize;
        let mut pending_outgoing_count = 0usize;
        let mut spam_count = 0usize;
        let mut pending_ignored_count = 0usize;

        for rel in relationships {
            match rel.relationship_type {
                RelationshipType::Friend => {
                    friend_count += 1;
                }
                RelationshipType::Blocked => {
                    blocked_count += 1;
                }
                RelationshipType::PendingIncoming => {
                    incoming_total += 1;
                    if rel.is_spam_request {
                        spam_count += 1;
                    }
                    if rel.is_ignored() {
                        pending_ignored_count += 1;
                    }
                }
                RelationshipType::PendingOutgoing => {
                    pending_outgoing_count += 1;
                }
                RelationshipType::Unknown(_) => {}
            }
        }

        let pending_incoming_count = incoming_total
            .saturating_sub(spam_count)
            .saturating_sub(pending_ignored_count);

        Self {
            friend_count,
            blocked_count,
            pending_incoming_count,
            pending_outgoing_count,
            spam_count,
            pending_ignored_count,
        }
    }
}

/// Вкладки раздела «Друзья» (`FriendsSection` в `waves/RW-40/DM-FRIENDS-RE.md` §10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FriendsTab {
    /// Друзья (`type == 1`), находящиеся в сети и не игнорируемые (`!is_ignored()`).
    Online,
    /// Все друзья (`type == 1`).
    All,
    /// Активные входящие (без спама и игнора) и исходящие заявки (`type == 3 || type == 4`).
    Pending,
    /// Заблокированные пользователи (`type == 2`).
    Blocked,
}

/// Полный снимок пользователей, приватных каналов и связей из `READY`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReadyDmFriendsSnapshot {
    pub users: BTreeMap<String, ResolvedDmUser>,
    pub private_channels: Vec<PrivateChannelRecord>,
    pub relationships: Vec<RelationshipRecord>,
    pub counts: RelationshipCounts,
}

/// Параметры `PUT /users/@me/relationships/{user_id}` (`DM-FRIENDS-RE.md` §2.2, @694401).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PutRelationshipParams {
    pub relationship_type: Option<RelationshipType>,
    pub from_friend_suggestion: Option<bool>,
    pub confirm_stranger_request: Option<bool>,
}

/// Проверенный ввод для `POST /users/@me/relationships` (`sendRequest` @697487).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FriendRequestTagInput {
    pub username: String,
    pub discriminator: Option<u16>,
    pub note: Option<String>,
}

impl FriendRequestTagInput {
    /// Разбирает строку `discordTag` по алгоритму `sendRequest` (@697487):
    /// - `"username"` или `"username#0"` -> `discriminator: None` (сериализуется как `null` в JSON для Pomelo);
    /// - `"username#0421"` -> `discriminator: Some(421)` (число `1..=9999`).
    pub fn parse(discord_tag: &str, note: Option<&str>) -> Result<Self, DmFriendsError> {
        let trimmed = discord_tag.trim();
        if trimmed.is_empty() {
            return Err(DmFriendsError::InvalidDiscordTag);
        }

        let mut parts = trimmed.split('#');
        let raw_username = parts.next().unwrap_or("");
        let username = raw_username.trim();
        if username.is_empty() {
            return Err(DmFriendsError::InvalidDiscordTag);
        }

        let discriminator = match (parts.next(), parts.next()) {
            (None, None) => None,
            (Some(disc_str), None) => {
                if disc_str.is_empty()
                    || disc_str.len() > 4
                    || !disc_str.bytes().all(|b| b.is_ascii_digit())
                {
                    return Err(DmFriendsError::InvalidDiscordTag);
                }
                let parsed = disc_str
                    .parse::<u16>()
                    .map_err(|_| DmFriendsError::InvalidDiscordTag)?;
                if parsed == 0 {
                    None
                } else {
                    Some(parsed)
                }
            }
            _ => return Err(DmFriendsError::InvalidDiscordTag),
        };

        let normalized_note = note
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        Ok(Self {
            username: username.to_string(),
            discriminator,
            note: normalized_note,
        })
    }
}

/// Типизированная ошибка модуля личных сообщений и друзей.
/// Ни один вариант не хранит токен или сырое тело ответа сервера, исключая утечку секретов.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DmFriendsError {
    /// Токен пустой или состоит из пробелов.
    MissingToken,
    /// Передан невалидный snowflake ID (пустой, `0`, нечисловой или с разделителями пути).
    InvalidSnowflakeId { value: String },
    /// Количество получателей вне допустимого диапазона `1..=9`.
    InvalidRecipientCount { count: usize },
    /// Повторяющийся snowflake ID в списке получателей.
    DuplicateRecipient { user_id: String },
    /// Некорректный формат `username` или `username#discriminator` для заявки в друзья.
    InvalidDiscordTag,
    /// Код ошибки Discord `80013` (`RELATIONSHIP_INVALID_NO_CONFIRMATION`):
    /// требуется повторный запрос с `confirm_stranger_request: true`.
    StrangerConfirmationRequired { user_id: String, code: u64 },
    /// Типизированный числовой код ошибки Discord API (например, `30004` `TOO_MANY_RECIPIENTS`,
    /// `40002` `EMAIL_VERIFICATION_REQUIRED` на `403`, `10013` `UNKNOWN_USER` на `404`).
    DiscordApiCode { status: u16, code: u64 },
    /// `401 Unauthorized`.
    Unauthorized,
    /// `403 Forbidden` без распознанного кода Discord (`code` отсутствует или равен `0`).
    Forbidden,
    /// `404 Not Found` без распознанного кода Discord (`code` отсутствует или равен `0`).
    NotFound,
    /// `429 Too Many Requests` с временем ожидания в миллисекундах.
    RateLimited { retry_after_ms: u64 },
    /// Прочие клиентские ошибки `4xx`.
    Client { status: u16 },
    /// Ошибки сервера `5xx`.
    Server { status: u16 },
    /// Сбой транспорта/сети.
    Network,
    /// Ошибка чтения локальной фикстуры/хранилища.
    Storage,
    /// Некорректный JSON или неожиданная схема ответа.
    InvalidResponse,
}

impl DmFriendsError {
    /// Возвращает `true`, если сервер вернул код `80013` (`RELATIONSHIP_INVALID_NO_CONFIRMATION`).
    #[must_use]
    pub const fn is_stranger_confirmation_required(&self) -> bool {
        matches!(self, Self::StrangerConfirmationRequired { .. })
    }

    /// Возвращает числовой код ошибки Discord API, если он был распознан.
    #[must_use]
    pub const fn discord_error_code(&self) -> Option<u64> {
        match self {
            Self::StrangerConfirmationRequired { code, .. } | Self::DiscordApiCode { code, .. } => {
                Some(*code)
            }
            _ => None,
        }
    }

    /// Конвертирует общий [`RestFailure`] в [`DmFriendsError`].
    #[must_use]
    pub const fn from_rest_failure(failure: RestFailure) -> Self {
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

impl fmt::Display for DmFriendsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingToken => write!(f, "токен авторизации отсутствует или пуст"),
            Self::InvalidSnowflakeId { value } => {
                write!(f, "некорректный snowflake ID: {value}")
            }
            Self::InvalidRecipientCount { count } => write!(
                f,
                "недопустимое число получателей ЛС ({count}); ожидается от {MIN_DM_RECIPIENTS} до {MAX_DM_RECIPIENTS}"
            ),
            Self::DuplicateRecipient { user_id } => {
                write!(f, "дублирующийся получатель в списке ЛС: {user_id}")
            }
            Self::InvalidDiscordTag => {
                write!(f, "некорректный тег пользователя Discord для заявки в друзья")
            }
            Self::StrangerConfirmationRequired { user_id, code } => write!(
                f,
                "требуется подтверждение заявки от незнакомца для пользователя {user_id} (код Discord {code})"
            ),
            Self::DiscordApiCode { status, code } => {
                write!(f, "ошибка Discord API (HTTP {status}, код {code})")
            }
            Self::Unauthorized => write!(f, "токен отклонен сервером Discord (401 Unauthorized)"),
            Self::Forbidden => write!(f, "доступ к операции ЛС/друзей запрещен (403 Forbidden)"),
            Self::NotFound => write!(f, "пользователь или канал не найден (404 Not Found)"),
            Self::RateLimited { retry_after_ms } => write!(
                f,
                "превышен лимит запросов Discord (429), повторите через {retry_after_ms} мс"
            ),
            Self::Client { status } => write!(f, "клиентская ошибка запроса (HTTP {status})"),
            Self::Server { status } => write!(f, "ошибка сервера Discord (HTTP {status})"),
            Self::Network => write!(f, "сетевая ошибка при обращении к Discord API"),
            Self::Storage => write!(f, "ошибка чтения локальных данных READY"),
            Self::InvalidResponse => write!(f, "некорректный формат ответа Discord"),
        }
    }
}

impl std::error::Error for DmFriendsError {}

/// Асинхронный транспорт для REST-операций ЛС и друзей.
pub trait DmFriendsTransport: Send + Sync {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, DmFriendsError>> + Send + 'a>>;

    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, DmFriendsError>> + Send + 'a>>;

    fn put<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, DmFriendsError>> + Send + 'a>>;

    fn delete<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, DmFriendsError>> + Send + 'a>>;
}

/// Валидирует snowflake ID (строго непустая строка из ASCII-цифр, парсящаяся в `u64 > 0`).
pub fn validate_snowflake_id(id: &str) -> Result<u64, DmFriendsError> {
    if id.is_empty() || id != id.trim() || !id.bytes().all(|b| b.is_ascii_digit()) {
        return Err(DmFriendsError::InvalidSnowflakeId {
            value: id.to_string(),
        });
    }
    let parsed = id
        .parse::<u64>()
        .map_err(|_| DmFriendsError::InvalidSnowflakeId {
            value: id.to_string(),
        })?;
    if parsed == 0 {
        return Err(DmFriendsError::InvalidSnowflakeId {
            value: id.to_string(),
        });
    }
    Ok(parsed)
}

/// Строит относительный маршрут `/users/@me/relationships/{user_id}` после проверки snowflake ID.
pub fn build_user_relationship_endpoint(user_id: &str) -> Result<String, DmFriendsError> {
    validate_snowflake_id(user_id)?;
    Ok(format!("{USER_RELATIONSHIPS_PATH}/{user_id}"))
}

/// Сортирует список приватных каналов по убыванию числового `u64` `last_message_id`
/// (при равенстве — по убыванию числового `u64` `id` канала).
pub fn sort_private_channels_by_last_message_desc(channels: &mut [PrivateChannelRecord]) {
    channels.sort_by(|a, b| {
        (b.last_message_snowflake(), b.channel_snowflake())
            .cmp(&(a.last_message_snowflake(), a.channel_snowflake()))
    });
}

fn opt_str(value: Option<&serde_json::Value>) -> Option<String> {
    match value? {
        serde_json::Value::Null => None,
        serde_json::Value::String(s) => {
            if s.is_empty() || s == "nil" {
                None
            } else {
                Some(s.clone())
            }
        }
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn bool_val(value: Option<&serde_json::Value>) -> bool {
    match value {
        Some(serde_json::Value::Bool(b)) => *b,
        Some(serde_json::Value::String(s)) => s.eq_ignore_ascii_case("true") || s == "1",
        Some(serde_json::Value::Number(n)) => n.as_i64() == Some(1),
        _ => false,
    }
}

fn i64_val(value: Option<&serde_json::Value>) -> Option<i64> {
    match value? {
        serde_json::Value::Number(n) => n.as_i64(),
        serde_json::Value::String(s) => s.parse::<i64>().ok(),
        _ => None,
    }
}

fn u64_val(value: Option<&serde_json::Value>) -> Option<u64> {
    match value? {
        serde_json::Value::Number(n) => n.as_u64(),
        serde_json::Value::String(s) => s.parse::<u64>().ok(),
        _ => None,
    }
}

fn parse_user_obj(obj: &serde_json::Value) -> Option<ResolvedDmUser> {
    let id = opt_str(obj.get("id"))?;
    let username = opt_str(obj.get("username")).unwrap_or_default();
    let global_name = opt_str(obj.get("global_name"));
    let discriminator = opt_str(obj.get("discriminator")).unwrap_or_else(|| "0".to_string());
    let avatar = opt_str(obj.get("avatar"));
    let bot = bool_val(obj.get("bot"));
    Some(ResolvedDmUser {
        id,
        username,
        global_name,
        discriminator,
        avatar,
        bot,
    })
}

fn parse_private_channel_obj(
    obj: &serde_json::Value,
    users_by_id: &BTreeMap<String, ResolvedDmUser>,
) -> Result<PrivateChannelRecord, DmFriendsError> {
    let id = opt_str(obj.get("id")).ok_or(DmFriendsError::InvalidResponse)?;
    let raw_type = i64_val(obj.get("type")).unwrap_or(CHANNEL_TYPE_DM);
    let channel_type = PrivateChannelType::from_i64(raw_type);
    let flags = u64_val(obj.get("flags")).unwrap_or(0);
    let last_message_id = opt_str(obj.get("last_message_id"));
    let name = opt_str(obj.get("name"));
    let icon = opt_str(obj.get("icon"));
    let owner_id = opt_str(obj.get("owner_id"));
    let is_message_request = bool_val(obj.get("is_message_request"));
    let is_message_request_timestamp = opt_str(obj.get("is_message_request_timestamp"));
    let is_spam = bool_val(obj.get("is_spam"));

    let mut nicks = BTreeMap::new();
    if let Some(nicks_obj) = obj.get("nicks").and_then(serde_json::Value::as_object) {
        for (uid, nick_val) in nicks_obj {
            if let Some(nick_str) = opt_str(Some(nick_val)) {
                nicks.insert(uid.clone(), nick_str);
            }
        }
    }

    let mut recipient_ids = Vec::new();
    let mut recipients = Vec::new();

    if let Some(rec_objs) = obj.get("recipients").and_then(serde_json::Value::as_array) {
        for rec_val in rec_objs {
            if let Some(user) = parse_user_obj(rec_val) {
                recipient_ids.push(user.id.clone());
                recipients.push(user);
            } else if let Some(uid) = opt_str(Some(rec_val)) {
                if let Some(user) = users_by_id.get(&uid) {
                    recipients.push(user.clone());
                }
                recipient_ids.push(uid);
            }
        }
    }

    if let Some(id_list) = obj
        .get("recipient_ids")
        .and_then(serde_json::Value::as_array)
    {
        if recipient_ids.is_empty() {
            for uid_val in id_list {
                if let Some(uid) = opt_str(Some(uid_val)) {
                    if let Some(user) = users_by_id.get(&uid) {
                        recipients.push(user.clone());
                    }
                    recipient_ids.push(uid);
                }
            }
        }
    }

    Ok(PrivateChannelRecord {
        id,
        channel_type,
        flags,
        last_message_id,
        recipient_ids,
        recipients,
        name,
        icon,
        owner_id,
        nicks,
        is_message_request,
        is_message_request_timestamp,
        is_spam,
    })
}

/// Разбирает `READY` JSON (`d.users`, `d.private_channels`, `d.relationships`) без единого сетевого вызова.
pub fn parse_ready_dm_friends(
    ready_json: &serde_json::Value,
) -> Result<ReadyDmFriendsSnapshot, DmFriendsError> {
    let d = ready_json.get("d").unwrap_or(ready_json);
    if !d.is_object() {
        return Err(DmFriendsError::InvalidResponse);
    }

    let mut users = BTreeMap::new();
    if let Some(users_arr) = d.get("users").and_then(serde_json::Value::as_array) {
        for u_val in users_arr {
            if let Some(user) = parse_user_obj(u_val) {
                users.insert(user.id.clone(), user);
            }
        }
    }

    let mut private_channels = Vec::new();
    if let Some(ch_arr) = d
        .get("private_channels")
        .and_then(serde_json::Value::as_array)
    {
        for ch_val in ch_arr {
            let ch = parse_private_channel_obj(ch_val, &users)?;
            private_channels.push(ch);
        }
    }
    sort_private_channels_by_last_message_desc(&mut private_channels);

    let mut relationships = Vec::new();
    if let Some(rel_arr) = d.get("relationships").and_then(serde_json::Value::as_array) {
        for rel_val in rel_arr {
            let inline_user = rel_val.get("user").and_then(parse_user_obj);
            let id = opt_str(rel_val.get("id"))
                .or_else(|| opt_str(rel_val.get("user_id")))
                .or_else(|| inline_user.as_ref().map(|u| u.id.clone()))
                .ok_or(DmFriendsError::InvalidResponse)?;
            let user_id = opt_str(rel_val.get("user_id"))
                .or_else(|| inline_user.as_ref().map(|u| u.id.clone()))
                .unwrap_or_else(|| id.clone());
            let raw_type = i64_val(rel_val.get("type")).ok_or(DmFriendsError::InvalidResponse)?;
            let relationship_type = RelationshipType::from_i64(raw_type);
            let nickname = opt_str(rel_val.get("nickname"));
            let since = opt_str(rel_val.get("since"));
            let is_spam_request = bool_val(rel_val.get("is_spam_request"));
            let user_ignored = bool_val(rel_val.get("user_ignored"));
            let user = inline_user.or_else(|| users.get(&user_id).cloned());

            relationships.push(RelationshipRecord {
                id,
                user_id,
                relationship_type,
                nickname,
                since,
                is_spam_request,
                user_ignored,
                user,
            });
        }
    }

    let counts = RelationshipCounts::from_relationships(&relationships);

    Ok(ReadyDmFriendsSnapshot {
        users,
        private_channels,
        relationships,
        counts,
    })
}

/// Загружает и разбирает живую фикстуру `gateway/by-type/002-READY.json` из каталога `fixtures_root`.
pub fn load_ready_dm_friends(
    fixtures_root: &Path,
) -> Result<ReadyDmFriendsSnapshot, DmFriendsError> {
    let path = fixtures_root.join("gateway/by-type/002-READY.json");
    let raw = std::fs::read_to_string(path).map_err(|_| DmFriendsError::Storage)?;
    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|_| DmFriendsError::InvalidResponse)?;
    parse_ready_dm_friends(&value)
}

/// Извлекает множество `user_id` друзей в сети (`online` / `idle` / `dnd`) из
/// `003-READY_SUPPLEMENTAL.json` (`d.merged_presences.friends`).
pub fn parse_supplemental_online_friends(
    supplemental_json: &serde_json::Value,
) -> Result<BTreeSet<String>, DmFriendsError> {
    let d = supplemental_json.get("d").unwrap_or(supplemental_json);
    let friends = d
        .get("merged_presences")
        .and_then(|mp| mp.get("friends"))
        .and_then(serde_json::Value::as_array)
        .ok_or(DmFriendsError::InvalidResponse)?;

    let mut online_ids = BTreeSet::new();
    for entry in friends {
        let uid = opt_str(entry.get("user_id")).or_else(|| opt_str(entry.get("id")));
        let status = opt_str(entry.get("status")).unwrap_or_default();
        if let Some(user_id) = uid {
            if !status.is_empty()
                && !status.eq_ignore_ascii_case("offline")
                && !status.eq_ignore_ascii_case("invisible")
            {
                online_ids.insert(user_id);
            }
        }
    }
    Ok(online_ids)
}

/// Фильтрует связи по выбранной вкладке раздела «Друзья» (`Online / All / Pending / Blocked`).
#[must_use]
pub fn filter_relationships_by_tab(
    relationships: &[RelationshipRecord],
    tab: FriendsTab,
    online_user_ids: &BTreeSet<String>,
) -> Vec<RelationshipRecord> {
    relationships
        .iter()
        .filter(|rel| match tab {
            FriendsTab::Online => {
                rel.relationship_type == RelationshipType::Friend
                    && !rel.is_ignored()
                    && online_user_ids.contains(&rel.user_id)
            }
            FriendsTab::All => rel.relationship_type == RelationshipType::Friend,
            FriendsTab::Pending => {
                (rel.relationship_type == RelationshipType::PendingIncoming
                    && !rel.is_spam_request
                    && !rel.is_ignored())
                    || rel.relationship_type == RelationshipType::PendingOutgoing
            }
            FriendsTab::Blocked => rel.relationship_type == RelationshipType::Blocked,
        })
        .cloned()
        .collect()
}

/// Возвращает входящие заявки, отфильтрованные в подраздел «Спам / Игнорируемые»
/// (`type == 3 && (is_spam_request || is_ignored())`).
#[must_use]
pub fn filter_pending_spam_and_ignored(
    relationships: &[RelationshipRecord],
) -> Vec<RelationshipRecord> {
    relationships
        .iter()
        .filter(|rel| {
            rel.relationship_type == RelationshipType::PendingIncoming
                && (rel.is_spam_request || rel.is_ignored())
        })
        .cloned()
        .collect()
}

fn validate_non_empty_token(token: &str) -> Result<(), DmFriendsError> {
    if token.trim().is_empty() {
        return Err(DmFriendsError::MissingToken);
    }
    Ok(())
}

fn validate_recipients_list(recipient_ids: &[&str]) -> Result<Vec<String>, DmFriendsError> {
    let count = recipient_ids.len();
    if !(MIN_DM_RECIPIENTS..=MAX_DM_RECIPIENTS).contains(&count) {
        return Err(DmFriendsError::InvalidRecipientCount { count });
    }
    let mut seen = BTreeSet::new();
    let mut validated = Vec::with_capacity(count);
    for &raw_id in recipient_ids {
        validate_snowflake_id(raw_id)?;
        if !seen.insert(raw_id) {
            return Err(DmFriendsError::DuplicateRecipient {
                user_id: raw_id.to_string(),
            });
        }
        validated.push(raw_id.to_string());
    }
    Ok(validated)
}

fn extract_discord_api_error_code(body: &[u8]) -> Option<u64> {
    let parsed: serde_json::Value = serde_json::from_slice(body).ok()?;
    u64_val(parsed.get("code"))
}

fn parse_retry_after_ms(headers: &[(String, String)], body: &[u8]) -> u64 {
    if let Some(ms) = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("retry-after"))
        .and_then(|(_, value)| value.trim().parse::<f64>().ok())
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
        .map(|seconds| (seconds * 1000.0).round() as u64)
    {
        return ms;
    }
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|val| val.get("retry_after").and_then(serde_json::Value::as_f64))
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
        .map_or(0, |seconds| (seconds * 1000.0).round() as u64)
}

/// Переводит неуспешный HTTP-ответ в [`DmFriendsError`].
///
/// Числовой код Discord извлекается из тела **до** статусного fallback для любого
/// `4xx`, кроме `401`: `403 {"code":40002}` (`EMAIL_VERIFICATION_REQUIRED`,
/// SPEC §1.4) и `404 {"code":10013}` (`UNKNOWN_USER`) сохраняют код, чтобы UI
/// отличал их от произвольного запрета или отсутствия объекта (Review 40A, P2-2).
/// `code == 0` (общая ошибка Discord) кодом не считается — остаётся [`DmFriendsError::Forbidden`]
/// / [`DmFriendsError::NotFound`]; `401` всегда [`DmFriendsError::Unauthorized`].
fn map_error_response(
    status: u16,
    headers: &[(String, String)],
    body: &[u8],
    relationship_user_id: Option<&str>,
) -> DmFriendsError {
    if status == 429 {
        return DmFriendsError::RateLimited {
            retry_after_ms: parse_retry_after_ms(headers, body),
        };
    }
    if (400..500).contains(&status) && status != 401 {
        if let Some(code) = extract_discord_api_error_code(body).filter(|code| *code != 0) {
            if code == DISCORD_ERR_RELATIONSHIP_INVALID_NO_CONFIRMATION {
                if let Some(uid) = relationship_user_id {
                    return DmFriendsError::StrangerConfirmationRequired {
                        user_id: uid.to_string(),
                        code,
                    };
                }
            }
            return DmFriendsError::DiscordApiCode { status, code };
        }
    }
    RestFailure::from_status(status)
        .map_or(DmFriendsError::Network, DmFriendsError::from_rest_failure)
}

/// Открывает или создаёт DM / Group DM через `POST /users/@me/channels` с телом `{"recipients": [...]}`.
/// До обращения к сети проверяет токен, диапазон `1..=9` получателей, отсутствие дубликатов и snowflake ID.
pub async fn create_private_channel<T: DmFriendsTransport + ?Sized>(
    transport: &T,
    token: &str,
    recipient_ids: &[&str],
) -> Result<PrivateChannelRecord, DmFriendsError> {
    validate_non_empty_token(token)?;
    let validated_ids = validate_recipients_list(recipient_ids)?;

    let payload = serde_json::json!({
        "recipients": validated_ids,
    });
    let body = serde_json::to_vec(&payload).map_err(|_| DmFriendsError::InvalidResponse)?;
    let resp = transport.post(USER_CHANNELS_PATH, &body).await?;

    if resp.status == 200 || resp.status == 201 {
        let value: serde_json::Value =
            serde_json::from_slice(&resp.body).map_err(|_| DmFriendsError::InvalidResponse)?;
        let empty_users = BTreeMap::new();
        return parse_private_channel_obj(&value, &empty_users);
    }

    Err(map_error_response(
        resp.status,
        &resp.headers,
        &resp.body,
        None,
    ))
}

/// Проверяет локальный кэш `cached_channels` (`PrivateChannelStore` @567585) перед вызовом
/// `POST /users/@me/channels`: если запрашивается 1-на-1 DM и канал с этим получателем уже
/// есть в кэше, возвращает его с `0` сетевых вызовов.
pub async fn open_private_channel_with_cache<T: DmFriendsTransport + ?Sized>(
    transport: &T,
    token: &str,
    cached_channels: &[PrivateChannelRecord],
    recipient_ids: &[&str],
) -> Result<PrivateChannelRecord, DmFriendsError> {
    validate_non_empty_token(token)?;
    let validated_ids = validate_recipients_list(recipient_ids)?;

    if validated_ids.len() == 1 {
        let target_uid = &validated_ids[0];
        if let Some(existing) = cached_channels.iter().find(|ch| {
            ch.channel_type == PrivateChannelType::Dm
                && ((ch.recipient_ids.len() == 1 && &ch.recipient_ids[0] == target_uid)
                    || (ch.recipients.len() == 1 && &ch.recipients[0].id == target_uid))
        }) {
            return Ok(existing.clone());
        }
    }

    create_private_channel(transport, token, recipient_ids).await
}

/// Создаёт пустую оболочку Group DM через `POST /users/@me/channels/group-dm-shell`
/// с телом `{"recipient_id": "..."}` (`DM-FRIENDS-RE.md` §2.1, @568215).
pub async fn create_group_dm_shell<T: DmFriendsTransport + ?Sized>(
    transport: &T,
    token: &str,
    recipient_id: &str,
) -> Result<PrivateChannelRecord, DmFriendsError> {
    validate_non_empty_token(token)?;
    validate_snowflake_id(recipient_id)?;

    let payload = serde_json::json!({
        "recipient_id": recipient_id,
    });
    let body = serde_json::to_vec(&payload).map_err(|_| DmFriendsError::InvalidResponse)?;
    let resp = transport.post(USER_GROUP_DM_SHELL_PATH, &body).await?;

    if resp.status == 200 || resp.status == 201 {
        let value: serde_json::Value =
            serde_json::from_slice(&resp.body).map_err(|_| DmFriendsError::InvalidResponse)?;
        let empty_users = BTreeMap::new();
        return parse_private_channel_obj(&value, &empty_users);
    }

    Err(map_error_response(
        resp.status,
        &resp.headers,
        &resp.body,
        None,
    ))
}

/// Выполняет `PUT /users/@me/relationships/{user_id}` (`addRelationship` @694401).
/// При коде ошибки `80013` возвращает [`DmFriendsError::StrangerConfirmationRequired`].
pub async fn put_relationship<T: DmFriendsTransport + ?Sized>(
    transport: &T,
    token: &str,
    user_id: &str,
    params: &PutRelationshipParams,
) -> Result<(), DmFriendsError> {
    validate_non_empty_token(token)?;
    let endpoint = build_user_relationship_endpoint(user_id)?;

    let mut body_map = serde_json::Map::new();
    if let Some(rel_type) = params.relationship_type {
        body_map.insert(
            "type".to_string(),
            serde_json::Value::Number(rel_type.as_i64().into()),
        );
    }
    if let Some(from_suggestion) = params.from_friend_suggestion {
        body_map.insert(
            "from_friend_suggestion".to_string(),
            serde_json::Value::Bool(from_suggestion),
        );
    }
    if let Some(confirm_stranger) = params.confirm_stranger_request {
        body_map.insert(
            "confirm_stranger_request".to_string(),
            serde_json::Value::Bool(confirm_stranger),
        );
    }

    let body = serde_json::to_vec(&serde_json::Value::Object(body_map))
        .map_err(|_| DmFriendsError::InvalidResponse)?;
    let resp = transport.put(&endpoint, &body).await?;

    if resp.status == 200 || resp.status == 204 {
        return Ok(());
    }

    Err(map_error_response(
        resp.status,
        &resp.headers,
        &resp.body,
        Some(user_id),
    ))
}

/// Принимает входящую заявку в друзья (`PUT /users/@me/relationships/{user_id}` с `{"type": 1, "confirm_stranger_request": ...}`).
pub async fn accept_friend_request<T: DmFriendsTransport + ?Sized>(
    transport: &T,
    token: &str,
    user_id: &str,
    confirm_stranger_request: bool,
) -> Result<(), DmFriendsError> {
    let params = PutRelationshipParams {
        relationship_type: Some(RelationshipType::Friend),
        from_friend_suggestion: None,
        confirm_stranger_request: Some(confirm_stranger_request),
    };
    put_relationship(transport, token, user_id, &params).await
}

/// Блокирует пользователя (`PUT /users/@me/relationships/{user_id}` с `{"type": 2}`).
pub async fn block_user<T: DmFriendsTransport + ?Sized>(
    transport: &T,
    token: &str,
    user_id: &str,
) -> Result<(), DmFriendsError> {
    let params = PutRelationshipParams {
        relationship_type: Some(RelationshipType::Blocked),
        from_friend_suggestion: None,
        confirm_stranger_request: None,
    };
    put_relationship(transport, token, user_id, &params).await
}

/// Удаляет из друзей, отклоняет/отменяет заявку или разблокирует пользователя
/// (`DELETE /users/@me/relationships/{user_id}`).
pub async fn delete_relationship<T: DmFriendsTransport + ?Sized>(
    transport: &T,
    token: &str,
    user_id: &str,
) -> Result<(), DmFriendsError> {
    validate_non_empty_token(token)?;
    let endpoint = build_user_relationship_endpoint(user_id)?;

    let resp = transport.delete(&endpoint).await?;
    if resp.status == 200 || resp.status == 204 {
        return Ok(());
    }

    Err(map_error_response(
        resp.status,
        &resp.headers,
        &resp.body,
        Some(user_id),
    ))
}

/// Отправляет заявку в друзья (`POST /users/@me/relationships`) с поддержкой Pomelo
/// (`"discriminator": null`) и классического `username#1234` (`"discriminator": 1234`).
pub async fn send_friend_request<T: DmFriendsTransport + ?Sized>(
    transport: &T,
    token: &str,
    input: &FriendRequestTagInput,
) -> Result<(), DmFriendsError> {
    validate_non_empty_token(token)?;
    let username = input.username.trim();
    if username.is_empty() || username.contains('#') {
        return Err(DmFriendsError::InvalidDiscordTag);
    }
    if let Some(disc) = input.discriminator {
        if disc == 0 || disc > 9999 {
            return Err(DmFriendsError::InvalidDiscordTag);
        }
    }

    let mut body_map = serde_json::Map::new();
    body_map.insert(
        "username".to_string(),
        serde_json::Value::String(username.to_string()),
    );
    let disc_val = match input.discriminator {
        Some(d) => serde_json::Value::Number(d.into()),
        None => serde_json::Value::Null,
    };
    body_map.insert("discriminator".to_string(), disc_val);
    if let Some(note) = &input.note {
        let trimmed_note = note.trim();
        if !trimmed_note.is_empty() {
            body_map.insert(
                "note".to_string(),
                serde_json::Value::String(trimmed_note.to_string()),
            );
        }
    }

    let body = serde_json::to_vec(&serde_json::Value::Object(body_map))
        .map_err(|_| DmFriendsError::InvalidResponse)?;
    let resp = transport.post(USER_RELATIONSHIPS_PATH, &body).await?;

    if resp.status == 200 || resp.status == 204 {
        return Ok(());
    }

    Err(map_error_response(
        resp.status,
        &resp.headers,
        &resp.body,
        None,
    ))
}

/// Реальный транспорт поверх [`ApiClient`] из `discord-core`.
struct ApiClientDmFriendsTransport(ApiClient);

impl ApiClientDmFriendsTransport {
    fn new(token: &str) -> Result<Self, DmFriendsError> {
        let identity_ctx = crate::auth::IdentityContext::detect();
        let auth = ApiAuth::from_token(
            token,
            &identity_ctx.super_properties,
            &identity_ctx.locale,
            &identity_ctx.timezone,
        )
        .map_err(|_| DmFriendsError::MissingToken)?;
        let client = ApiClient::new(ApiConfig {
            base: API_V9_BASE.to_string(),
            auth,
            http: HttpClientConfig::default(),
            retry: ApiRetryPolicy {
                max_attempts: 1,
                ..ApiRetryPolicy::default()
            },
        })
        .map_err(|_| DmFriendsError::Network)?;
        Ok(Self(client))
    }
}

impl DmFriendsTransport for ApiClientDmFriendsTransport {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, DmFriendsError>> + Send + 'a>> {
        Box::pin(async move {
            self.0
                .get(target)
                .await
                .map_err(|err| DmFriendsError::from_rest_failure(RestFailure::from_api_error(&err)))
        })
    }

    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, DmFriendsError>> + Send + 'a>> {
        Box::pin(async move {
            self.0
                .post(target, body)
                .await
                .map_err(|err| DmFriendsError::from_rest_failure(RestFailure::from_api_error(&err)))
        })
    }

    fn put<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, DmFriendsError>> + Send + 'a>> {
        Box::pin(async move {
            self.0
                .put(target, body)
                .await
                .map_err(|err| DmFriendsError::from_rest_failure(RestFailure::from_api_error(&err)))
        })
    }

    fn delete<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, DmFriendsError>> + Send + 'a>> {
        Box::pin(async move {
            self.0
                .delete(target)
                .await
                .map_err(|err| DmFriendsError::from_rest_failure(RestFailure::from_api_error(&err)))
        })
    }
}

/// Высокоуровневый клиент ЛС и друзей с маскированием токена в `Debug`.
#[derive(Clone)]
pub struct DiscordDmFriends {
    token: String,
    transport: Arc<dyn DmFriendsTransport>,
}

impl fmt::Debug for DiscordDmFriends {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DiscordDmFriends")
            .field("token", &"<redacted>")
            .finish()
    }
}

impl DiscordDmFriends {
    /// Создаёт клиент с реальным транспортом `discord-core` (`ApiClient<UreqTransport>`).
    pub fn new(token: &str) -> Result<Self, DmFriendsError> {
        validate_non_empty_token(token)?;
        let transport = ApiClientDmFriendsTransport::new(token)?;
        Ok(Self {
            token: token.to_string(),
            transport: Arc::new(transport),
        })
    }

    /// Создаёт клиент с произвольным (например, тестовым fake) транспортом.
    pub fn with_transport(
        token: &str,
        transport: Arc<dyn DmFriendsTransport>,
    ) -> Result<Self, DmFriendsError> {
        validate_non_empty_token(token)?;
        Ok(Self {
            token: token.to_string(),
            transport,
        })
    }

    /// Открывает или создаёт DM / Group DM (`POST /users/@me/channels`).
    pub async fn create_private_channel(
        &self,
        recipient_ids: &[&str],
    ) -> Result<PrivateChannelRecord, DmFriendsError> {
        create_private_channel(self.transport.as_ref(), &self.token, recipient_ids).await
    }

    /// Открывает 1-на-1 DM с проверкой локального кэша или создаёт новый приватный канал.
    pub async fn open_private_channel_with_cache(
        &self,
        cached_channels: &[PrivateChannelRecord],
        recipient_ids: &[&str],
    ) -> Result<PrivateChannelRecord, DmFriendsError> {
        open_private_channel_with_cache(
            self.transport.as_ref(),
            &self.token,
            cached_channels,
            recipient_ids,
        )
        .await
    }

    /// Создаёт пустую оболочку Group DM (`POST /users/@me/channels/group-dm-shell`).
    pub async fn create_group_dm_shell(
        &self,
        recipient_id: &str,
    ) -> Result<PrivateChannelRecord, DmFriendsError> {
        create_group_dm_shell(self.transport.as_ref(), &self.token, recipient_id).await
    }

    /// Выполняет `PUT /users/@me/relationships/{user_id}`.
    pub async fn put_relationship(
        &self,
        user_id: &str,
        params: &PutRelationshipParams,
    ) -> Result<(), DmFriendsError> {
        put_relationship(self.transport.as_ref(), &self.token, user_id, params).await
    }

    /// Принимает входящую заявку в друзья (`PUT /users/@me/relationships/{user_id}`).
    pub async fn accept_friend_request(
        &self,
        user_id: &str,
        confirm_stranger_request: bool,
    ) -> Result<(), DmFriendsError> {
        accept_friend_request(
            self.transport.as_ref(),
            &self.token,
            user_id,
            confirm_stranger_request,
        )
        .await
    }

    /// Блокирует пользователя (`PUT /users/@me/relationships/{user_id}`).
    pub async fn block_user(&self, user_id: &str) -> Result<(), DmFriendsError> {
        block_user(self.transport.as_ref(), &self.token, user_id).await
    }

    /// Удаляет связь (`DELETE /users/@me/relationships/{user_id}`).
    pub async fn delete_relationship(&self, user_id: &str) -> Result<(), DmFriendsError> {
        delete_relationship(self.transport.as_ref(), &self.token, user_id).await
    }

    /// Отправляет заявку в друзья (`POST /users/@me/relationships`).
    pub async fn send_friend_request(
        &self,
        input: &FriendRequestTagInput,
    ) -> Result<(), DmFriendsError> {
        send_friend_request(self.transport.as_ref(), &self.token, input).await
    }
}
