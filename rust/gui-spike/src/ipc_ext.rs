//! RW-40 / task-196 & LIB-SIMPLIFY-01: расширенный IPC-диспетчер (`ExtendedIpcCommand`,
//! `ExtendedIpcPlan`, `validate_and_build_plan`) и прозрачный мост `reqwest + url + serde`
//! ([`DiscordRestBridge`]) для команд:
//! - `get_user_profile` (`GET /users/{user_id}/profile?with_mutual_guilds=...&with_mutual_friends_count=...[&guild_id=...]`)
//! - `ack_messages_bulk` (`POST /read-states/ack-bulk`, тело `{"read_states": [...]}`)
//! - `add_reaction_empty_body` (`PUT /channels/{channel_id}/messages/{message_id}/reactions/{emoji}/@me`, тело `None`)
//! - `vote_poll` (`PUT /channels/{channel_id}/polls/{message_id}/answers/@me`, тело `{"answer_ids": [...]}`)
//! - `search_guild_messages` (`GET /guilds/{guild_id}/messages/search?[content=...&]offset=...&limit=...`)

use chord_core::rest::bridge::{
    DiscordRestBridgeError, build_relative_url_with_query, validate_snowflake_id,
};
use chord_core::rest::routes::HttpMethod;
use chord_core::rest::transport::HttpResponse;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

pub use chord_core::rest::bridge::{
    DiscordHttpClient, DiscordRestBridge as CoreDiscordRestBridge, RateLimitBucketState,
};

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

/// Типизированный Serde DTO query-параметров запроса профиля пользователя (`GET /users/{id}/profile`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserProfileQueryDto {
    pub with_mutual_guilds: bool,
    pub with_mutual_friends_count: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<String>,
}

/// Типизированный Serde DTO тела пачки подтверждений прочитанного (`POST /read-states/ack-bulk`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AckMessagesBulkRequestDto {
    pub read_states: Vec<BulkReadStateEntry>,
}

/// Типизированный Serde DTO тела голосования в опросе (`PUT /channels/{id}/polls/{id}/answers/@me`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PollVoteBodyDto {
    pub answer_ids: Vec<u32>,
}

/// Типизированный Serde DTO query-параметров поиска сообщений гильдии (`GET /guilds/{id}/messages/search`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuildMessageSearchQueryDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    pub offset: u32,
    pub limit: u32,
}

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

impl ExtendedIpcPlan {
    /// Преобразует строковый HTTP-метод плана в типизированный [`HttpMethod`].
    pub fn http_method(&self) -> Result<HttpMethod, String> {
        match self.method {
            "GET" => Ok(HttpMethod::Get),
            "POST" => Ok(HttpMethod::Post),
            "PUT" => Ok(HttpMethod::Put),
            "PATCH" => Ok(HttpMethod::Patch),
            "DELETE" => Ok(HttpMethod::Delete),
            other => Err(format!("неподдерживаемый HTTP-метод: {other}")),
        }
    }

    /// Разрешает относительный путь плана относительно базового URL (`https://discord.com/api/v9`)
    /// через `url::Url`.
    pub fn resolve_url(&self, base: &str) -> Result<Url, String> {
        let base_trimmed = base.trim_end_matches('/');
        let path = if self.path.starts_with('/') {
            self.path.as_str()
        } else {
            return Err(format!("путь должен начинаться с '/': {}", self.path));
        };
        Url::parse(&format!("{base_trimmed}{path}"))
            .map_err(|e| format!("некорректный URL плана: {e}"))
    }
}

/// Прозрачный мост выполнения [`ExtendedIpcCommand`] и [`ExtendedIpcPlan`] через `reqwest + url + serde`.
#[derive(Debug, Clone)]
pub struct DiscordRestBridge {
    inner: DiscordHttpClient,
}

impl DiscordRestBridge {
    /// Создаёт мост поверх настроенного [`DiscordHttpClient`].
    #[must_use]
    pub fn new(inner: DiscordHttpClient) -> Self {
        Self { inner }
    }

    /// Создаёт мост с анонимной конфигурацией для заданного базового URL (`https://discord.com/api/v9`).
    pub fn with_anonymous_base(base: &str) -> Result<Self, String> {
        let client = DiscordHttpClient::with_anonymous_base(base).map_err(|e| e.to_string())?;
        Ok(Self { inner: client })
    }

    /// Возвращает ссылку на внутренний [`DiscordHttpClient`].
    #[must_use]
    pub fn client(&self) -> &DiscordHttpClient {
        &self.inner
    }

    /// Строит готовый [`reqwest::Request`] из [`ExtendedIpcPlan`].
    pub fn build_reqwest_request(&self, plan: &ExtendedIpcPlan) -> Result<reqwest::Request, String> {
        let method = plan.http_method()?;
        let body_bytes = match &plan.body {
            Some(val) => Some(
                serde_json::to_vec(val)
                    .map_err(|e| format!("ошибка сериализации тела плана: {e}"))?,
            ),
            None => None,
        };
        self.inner
            .build_reqwest_request_with_body(method, &plan.path, &[], body_bytes)
            .map_err(|e| e.to_string())
    }

    /// Валидирует и выполняет [`ExtendedIpcCommand`] через пул соединений `reqwest` с учётом rate-limit.
    pub async fn execute_command(
        &self,
        cmd: &ExtendedIpcCommand,
    ) -> Result<HttpResponse, DiscordRestBridgeError> {
        let plan =
            validate_and_build_plan(cmd).map_err(DiscordRestBridgeError::InvalidUrl)?;
        let method = plan
            .http_method()
            .map_err(DiscordRestBridgeError::InvalidUrl)?;
        let body_bytes = match &plan.body {
            Some(val) => Some(
                serde_json::to_vec(val)
                    .map_err(|e| DiscordRestBridgeError::Serialization(e.to_string()))?,
            ),
            None => None,
        };
        self.inner
            .execute_raw(method, &plan.path, &[], body_bytes.as_deref())
            .await
    }

    /// Валидирует, выполняет [`ExtendedIpcCommand`] и десериализует JSON-ответ в типизированный DTO `Resp`.
    pub async fn execute_command_json<Resp: DeserializeOwned>(
        &self,
        cmd: &ExtendedIpcCommand,
    ) -> Result<Resp, DiscordRestBridgeError> {
        let response = self.execute_command(cmd).await?;
        serde_json::from_slice(&response.body)
            .map_err(|e| DiscordRestBridgeError::Serialization(e.to_string()))
    }
}

/// Проверяет, что строка является непустым положительным Snowflake ID
/// (только ASCII-цифры, влезает в `u64` и `> 0`), делегируя ядру `chord_core::rest::bridge`.
fn validate_snowflake(field: &str, value: &str) -> Result<(), String> {
    validate_snowflake_id(field, value)
        .map(|_| ())
        .map_err(|_| format!("некорректный Snowflake ID в поле {field}"))
}

/// Валидирует [`ExtendedIpcCommand`] и строит [`ExtendedIpcPlan`] для Discord REST API v9
/// с использованием `url::Url` и типизированных Serde DTO.
pub fn validate_and_build_plan(cmd: &ExtendedIpcCommand) -> Result<ExtendedIpcPlan, String> {
    match cmd {
        ExtendedIpcCommand::GetUserProfile {
            user_id,
            with_mutual_guilds,
            with_mutual_friends_count,
            guild_id,
        } => {
            validate_snowflake("user_id", user_id)?;
            let dto = UserProfileQueryDto {
                with_mutual_guilds: *with_mutual_guilds,
                with_mutual_friends_count: *with_mutual_friends_count,
                guild_id: guild_id.clone(),
            };
            let mut query_pairs = vec![
                ("with_mutual_guilds", dto.with_mutual_guilds.to_string()),
                (
                    "with_mutual_friends_count",
                    dto.with_mutual_friends_count.to_string(),
                ),
            ];
            if let Some(gid) = dto.guild_id.as_deref() {
                validate_snowflake("guild_id", gid)?;
                query_pairs.push(("guild_id", gid.to_string()));
            }
            let route_base = format!("/users/{user_id}/profile");
            let path = build_relative_url_with_query(&route_base, &query_pairs)
                .map_err(|e| e.to_string())?;
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
            let dto = AckMessagesBulkRequestDto {
                read_states: read_states.clone(),
            };
            let body = serde_json::to_value(&dto)
                .map_err(|err| format!("ошибка сериализации read_states: {err}"))?;
            let path = build_relative_url_with_query("/read-states/ack-bulk", &[])
                .map_err(|e| e.to_string())?;
            Ok(ExtendedIpcPlan {
                method: "POST",
                path,
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
            let route_base =
                format!("/channels/{channel_id}/messages/{message_id}/reactions/{emoji}/@me");
            let path =
                build_relative_url_with_query(&route_base, &[]).map_err(|e| e.to_string())?;
            Ok(ExtendedIpcPlan {
                method: "PUT",
                path,
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
            let dto = PollVoteBodyDto {
                answer_ids: answer_ids.clone(),
            };
            let body = serde_json::to_value(&dto)
                .map_err(|err| format!("ошибка сериализации answer_ids: {err}"))?;
            let route_base = format!("/channels/{channel_id}/polls/{message_id}/answers/@me");
            let path =
                build_relative_url_with_query(&route_base, &[]).map_err(|e| e.to_string())?;
            Ok(ExtendedIpcPlan {
                method: "PUT",
                path,
                body: Some(body),
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
            let dto = GuildMessageSearchQueryDto {
                content: content.clone(),
                offset: *offset,
                limit: *limit,
            };
            let mut query_pairs = Vec::with_capacity(3);
            if let Some(text) = dto.content.as_deref() {
                query_pairs.push(("content", text.to_string()));
            }
            query_pairs.push(("offset", dto.offset.to_string()));
            query_pairs.push(("limit", dto.limit.to_string()));
            let route_base = format!("/guilds/{guild_id}/messages/search");
            let path = build_relative_url_with_query(&route_base, &query_pairs)
                .map_err(|e| e.to_string())?;
            Ok(ExtendedIpcPlan {
                method: "GET",
                path,
                body: None,
            })
        }
    }
}
