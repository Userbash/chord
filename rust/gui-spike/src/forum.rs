//! RW-40 / task-209: контракт создания публикации в форум-канале (`GUILD_FORUM`, тип `15`)
//! и порядок сортировки форума (`POST /channels/{channel_id}/threads`).
//!
//! Оракул: `/home/sanya/re-deep/rest-static.json` (`Endpoints.CHANNEL_THREADS`, `FORUM_TAG`)
//! и `/home/sanya/re-deep/gateway-schema.md` (`default_sort_order`, `default_auto_archive_duration`).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Максимальное число тегов (`applied_tags`), прикрепляемых к публикации форума.
pub const MAX_FORUM_APPLIED_TAGS: usize = 5;

/// Максимальная длина заголовка публикации (`name.trim()`) в символах Unicode.
pub const MAX_FORUM_TITLE_CHARS: usize = 100;

/// Максимальная длина текста сообщения (`content.trim()`) в символах Unicode.
pub const MAX_FORUM_CONTENT_CHARS: usize = 2000;

/// Допустимые значения `auto_archive_duration` в минутах (`1ч`, `24ч`, `3д`, `7д`).
pub const ALLOWED_AUTO_ARCHIVE_MINUTES: [u32; 4] = [60, 1440, 4320, 10080];

/// Порядок сортировки публикаций в форум-канале (`default_sort_order`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ForumSortOrder {
    /// По недавней активности (`0`).
    LatestActivity = 0,
    /// По дате создания (`1`).
    CreationDate = 1,
}

impl ForumSortOrder {
    /// Возвращает числовой код сортировки в протоколе Discord (`0` или `1`).
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Преобразует числовой код (`0` или `1`) в [`ForumSortOrder`].
    #[must_use]
    pub const fn from_u8(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::LatestActivity),
            1 => Some(Self::CreationDate),
            _ => None,
        }
    }
}

/// Запрос на создание новой публикации (ветки) в форум-канале (`POST /channels/{channel_id}/threads`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateForumPostRequest {
    /// Snowflake ID форум-канала (`channel_id`).
    pub channel_id: String,
    /// Заголовок публикации (`1..=100` символов после `trim()`).
    pub name: String,
    /// Время автоархивации в минутах (`60`, `1440`, `4320` или `10080`).
    pub auto_archive_duration: u32,
    /// Текст стартового сообщения (`1..=2000` символов после `trim()`).
    pub content: String,
    /// Список Snowflake ID применённых тегов (`0..=5` элементов).
    pub applied_tags: Vec<String>,
    /// Опциональный slowmode (`rate_limit_per_user`) в секундах.
    pub rate_limit_per_user: Option<u32>,
}

/// Проверяет, что строка является непустым положительным Snowflake ID
/// (только ASCII-цифры, влезает в `u64` и `> 0`).
fn validate_snowflake(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("некорректный Snowflake ID в поле {field}"));
    }
    match value.parse::<u64>() {
        Ok(n) if n > 0 => Ok(()),
        _ => Err(format!("некорректный Snowflake ID в поле {field}")),
    }
}

/// Валидирует [`CreateForumPostRequest`] перед отправкой в Discord REST API v9.
///
/// Проверяет:
/// - `channel_id` — непустой числовой Snowflake > 0;
/// - `name.trim()` — от `1` до [`MAX_FORUM_TITLE_CHARS`] (`100`) символов;
/// - `auto_archive_duration` — входит в [`ALLOWED_AUTO_ARCHIVE_MINUTES`] (`[60, 1440, 4320, 10080]`);
/// - `content.trim()` — непустой и `<= 2000` символов;
/// - `applied_tags.len() <= MAX_FORUM_APPLIED_TAGS` (`5`) и каждый тег — непустой числовой Snowflake > 0.
pub fn validate_forum_post_request(req: &CreateForumPostRequest) -> Result<(), String> {
    validate_snowflake("channel_id", &req.channel_id)?;

    let title_chars = req.name.trim().chars().count();
    if !(1..=MAX_FORUM_TITLE_CHARS).contains(&title_chars) {
        return Err(format!(
            "name.trim() должен содержать от 1 до {MAX_FORUM_TITLE_CHARS} символов, получено {title_chars}"
        ));
    }

    if !ALLOWED_AUTO_ARCHIVE_MINUTES.contains(&req.auto_archive_duration) {
        return Err(format!(
            "недопустимое значение auto_archive_duration: {}",
            req.auto_archive_duration
        ));
    }

    let content_chars = req.content.trim().chars().count();
    if !(1..=MAX_FORUM_CONTENT_CHARS).contains(&content_chars) {
        return Err(format!(
            "content.trim() должен содержать от 1 до {MAX_FORUM_CONTENT_CHARS} символов, получено {content_chars}"
        ));
    }

    if req.applied_tags.len() > MAX_FORUM_APPLIED_TAGS {
        return Err(format!(
            "applied_tags не может содержать более {MAX_FORUM_APPLIED_TAGS} тегов, получено {}",
            req.applied_tags.len()
        ));
    }
    for tag in &req.applied_tags {
        validate_snowflake("applied_tags", tag)?;
    }

    Ok(())
}

/// Валидирует [`CreateForumPostRequest`] и возвращает маршрут и JSON-тело для
/// `POST /channels/{channel_id}/threads`.
pub fn build_forum_post_route_and_body(
    req: &CreateForumPostRequest,
) -> Result<(String, Value), String> {
    validate_forum_post_request(req)?;

    let route = format!("/channels/{}/threads", req.channel_id);
    let mut body = json!({
        "name": req.name,
        "auto_archive_duration": req.auto_archive_duration,
        "applied_tags": req.applied_tags,
        "message": {
            "content": req.content,
        },
    });

    if let Some(rate_limit) = req.rate_limit_per_user {
        if let Some(obj) = body.as_object_mut() {
            obj.insert("rate_limit_per_user".to_string(), json!(rate_limit));
        }
    }

    Ok((route, body))
}
