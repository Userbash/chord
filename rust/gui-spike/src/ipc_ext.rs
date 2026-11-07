//! RW-40 / task-196: расширенный IPC-диспетчер (`ExtendedIpcCommand`, `ExtendedIpcPlan`,
//! `validate_and_build_plan`) для команд:
//! - `get_user_profile` (`GET /users/{user_id}/profile?with_mutual_guilds=...&with_mutual_friends_count=...[&guild_id=...]`)
//! - `ack_messages_bulk` (`POST /read-states/ack-bulk`, тело `{"read_states": [...]}`)
//! - `add_reaction_empty_body` (`PUT /channels/{channel_id}/messages/{message_id}/reactions/{emoji}/@me`, тело `None`)
//! - `vote_poll` (`PUT /channels/{channel_id}/polls/{message_id}/answers/@me`, тело `{"answer_ids": [...]}`)
//! - `search_guild_messages` (`GET /guilds/{guild_id}/messages/search?[content=...&]offset=...&limit=...`)

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Элемент пачки bulk read-state (`POST /read-states/ack-bulk`).
pub type BulkReadStateEntry = crate::messages::AckBulkEntry;

/// Максимальное число элементов `read_states` в `AckMessagesBulk` (`splice(0, 100)`).
pub const MAX_BULK_READ_STATES: usize = 100;

/// Максимальное число выбранных ответов в опросе `VotePoll` (`1..=10`).
pub const MAX_POLL_ANSWERS: usize = 10;

/// Максимальное смещение пагинации поиска `SearchGuildMessages` (`offset <= 9975`).
pub const MAX_SEARCH_OFFSET: u32 = 9975;

/// Максимальный размер страницы поиска `SearchGuildMessages` (`limit` в `1..=25`).
pub const MAX_SEARCH_LIMIT: u32 = 25;

/// Команды расширенного IPC-диспетчера.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cmd", content = "args", rename_all = "snake_case")]
pub enum ExtendedIpcCommand {
    /// Запрос профиля пользователя (`GET /users/{id}/profile`).
    GetUserProfile {
        user_id: String,
        with_mutual_guilds: bool,
        with_mutual_friends_count: bool,
        guild_id: Option<String>,
    },
    /// Пачка подтверждений прочитанного (`POST /read-states/ack-bulk`).
    AckMessagesBulk {
        read_states: Vec<BulkReadStateEntry>,
    },
    /// Установка реакции с пустым телом (`PUT /channels/{id}/messages/{id}/reactions/{emoji}/@me`).
    AddReactionEmptyBody {
        channel_id: String,
        message_id: String,
        emoji: String,
    },
    /// Голосование в опросе (`PUT /channels/{id}/polls/{id}/answers/@me`).
    VotePoll {
        channel_id: String,
        message_id: String,
        answer_ids: Vec<u32>,
    },
    /// Поиск сообщений на сервере (`GET /guilds/{id}/messages/search`).
    SearchGuildMessages {
        guild_id: String,
        content: Option<String>,
        offset: u32,
        limit: u32,
    },
}

/// План HTTP-запроса Discord REST API v9, построенный из [`ExtendedIpcCommand`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtendedIpcPlan {
    /// HTTP-метод (`"GET"`, `"POST"`, `"PUT"`).
    pub method: &'static str,
    /// Путь маршрута Discord v9 REST (включая query-строку, если есть).
    pub path: String,
    /// JSON-тело запроса (`None` для `GET` и `PUT` без тела).
    pub body: Option<Value>,
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

/// Валидирует [`ExtendedIpcCommand`] и строит [`ExtendedIpcPlan`] для Discord REST API v9.
pub fn validate_and_build_plan(cmd: &ExtendedIpcCommand) -> Result<ExtendedIpcPlan, String> {
    match cmd {
        ExtendedIpcCommand::GetUserProfile {
            user_id,
            with_mutual_guilds,
            with_mutual_friends_count,
            guild_id,
        } => {
            validate_snowflake("user_id", user_id)?;
            let mut path = format!(
                "/users/{user_id}/profile?with_mutual_guilds={with_mutual_guilds}&with_mutual_friends_count={with_mutual_friends_count}"
            );
            if let Some(gid) = guild_id.as_deref() {
                validate_snowflake("guild_id", gid)?;
                path.push_str(&format!("&guild_id={gid}"));
            }
            Ok(ExtendedIpcPlan {
                method: "GET",
                path,
                body: None,
            })
        }
        ExtendedIpcCommand::AckMessagesBulk { read_states } => {
            if read_states.is_empty() || read_states.len() > MAX_BULK_READ_STATES {
                return Err(format!(
                    "read_states должен содержать от 1 до {MAX_BULK_READ_STATES} элементов"
                ));
            }
            for entry in read_states {
                validate_snowflake("channel_id", &entry.channel_id)?;
                validate_snowflake("message_id", &entry.message_id)?;
            }
            let body = serde_json::to_value(json!({ "read_states": read_states }))
                .map_err(|err| format!("ошибка сериализации read_states: {err}"))?;
            Ok(ExtendedIpcPlan {
                method: "POST",
                path: "/read-states/ack-bulk".to_string(),
                body: Some(body),
            })
        }
        ExtendedIpcCommand::AddReactionEmptyBody {
            channel_id,
            message_id,
            emoji,
        } => {
            validate_snowflake("channel_id", channel_id)?;
            validate_snowflake("message_id", message_id)?;
            if emoji.trim().is_empty() {
                return Err("emoji не должен быть пустым".to_string());
            }
            Ok(ExtendedIpcPlan {
                method: "PUT",
                path: format!("/channels/{channel_id}/messages/{message_id}/reactions/{emoji}/@me"),
                body: None,
            })
        }
        ExtendedIpcCommand::VotePoll {
            channel_id,
            message_id,
            answer_ids,
        } => {
            validate_snowflake("channel_id", channel_id)?;
            validate_snowflake("message_id", message_id)?;
            if answer_ids.is_empty() || answer_ids.len() > MAX_POLL_ANSWERS {
                return Err(format!(
                    "answer_ids должен содержать от 1 до {MAX_POLL_ANSWERS} элементов"
                ));
            }
            Ok(ExtendedIpcPlan {
                method: "PUT",
                path: format!("/channels/{channel_id}/polls/{message_id}/answers/@me"),
                body: Some(json!({ "answer_ids": answer_ids })),
            })
        }
        ExtendedIpcCommand::SearchGuildMessages {
            guild_id,
            content,
            offset,
            limit,
        } => {
            validate_snowflake("guild_id", guild_id)?;
            if *offset > MAX_SEARCH_OFFSET {
                return Err(format!(
                    "offset не должен превышать {MAX_SEARCH_OFFSET}, получено {offset}"
                ));
            }
            if !(1..=MAX_SEARCH_LIMIT).contains(limit) {
                return Err(format!(
                    "limit должен быть в диапазоне 1..={MAX_SEARCH_LIMIT}, получено {limit}"
                ));
            }
            let path = match content.as_deref() {
                Some(text) => format!(
                    "/guilds/{guild_id}/messages/search?content={text}&offset={offset}&limit={limit}"
                ),
                None => format!(
                    "/guilds/{guild_id}/messages/search?offset={offset}&limit={limit}"
                ),
            };
            Ok(ExtendedIpcPlan {
                method: "GET",
                path,
                body: None,
            })
        }
    }
}
