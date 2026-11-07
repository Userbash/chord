//! RW-40 / task-249 (`BE-P1-28`): BFF-контракт правил автомодерации гильдии (`automod`).
//!
//! Источники контракта:
//! - `/home/sanya/re-deep/rest-static.json:8353-8380` (`Endpoints.GUILD_AUTOMOD_RULES: "/guilds/{arg}/auto-moderation/rules"`,
//!   `Endpoints.GUILD_AUTOMOD_RULE: "/guilds/{arg}/auto-moderation/rules/{arg}"`);
//! - Discord API v9:
//!   - тип события `1 = MESSAGE_SEND`;
//!   - типы триггеров: `1 = KEYWORD`, `3 = SPAM`, `4 = KEYWORD_PRESET`, `5 = MENTION_SPAM`;
//!   - типы действий: `1 = BLOCK_MESSAGE`, `2 = SEND_ALERT_MESSAGE`, `3 = TIMEOUT`;
//!   - лимит длины названия правила `1..=100` символов Unicode;
//!   - лимит элементов `keyword_filter` `1..=1000` (каждый `1..=60` символов Unicode после `trim()`);
//!   - лимит упоминаний `mention_total_limit` `1..=50`;
//!   - лимит длительности таймаута `timeout_duration_secs` `1..=2_419_200` секунд (28 суток).

use serde::{Deserialize, Serialize};

/// Тип события автомодерации `1` — отправка сообщения (`MESSAGE_SEND`).
pub const AUTOMOD_EVENT_MESSAGE_SEND: u8 = 1;

/// Тип триггера автомодерации `1` — фильтр по ключевым словам (`KEYWORD`).
pub const AUTOMOD_TRIGGER_KEYWORD: u8 = 1;

/// Тип триггера автомодерации `3` — защита от спама (`SPAM`).
pub const AUTOMOD_TRIGGER_SPAM: u8 = 3;

/// Тип триггера автомодерации `4` — предустановленные списки слов (`KEYWORD_PRESET`).
pub const AUTOMOD_TRIGGER_KEYWORD_PRESET: u8 = 4;

/// Тип триггера автомодерации `5` — защита от спама упоминаниями (`MENTION_SPAM`).
pub const AUTOMOD_TRIGGER_MENTION_SPAM: u8 = 5;

/// Тип действия автомодерации `1` — блокировка сообщения (`BLOCK_MESSAGE`).
pub const AUTOMOD_ACTION_BLOCK_MESSAGE: u8 = 1;

/// Тип действия автомодерации `2` — отправка оповещения в канал (`SEND_ALERT_MESSAGE`).
pub const AUTOMOD_ACTION_SEND_ALERT: u8 = 2;

/// Тип действия автомодерации `3` — временное ограничение участника (`TIMEOUT`).
pub const AUTOMOD_ACTION_TIMEOUT: u8 = 3;

/// Максимальная длина названия правила автомодерации в символах Unicode (`100`).
pub const MAX_AUTOMOD_NAME_CHARS: usize = 100;

/// Максимальное количество паттернов в `keyword_filter` (`1000`).
pub const MAX_KEYWORD_FILTER_ITEMS: usize = 1000;

/// Максимальный лимит упоминаний в одном сообщении для `MENTION_SPAM` (`50`).
pub const MAX_MENTION_TOTAL_LIMIT: u8 = 50;

/// Максимальная длительность таймаута в секундах (`2_419_200` = 28 суток).
pub const MAX_TIMEOUT_DURATION_SECS: u32 = 2_419_200;

/// Максимальная длина одного ключевого слова в `keyword_filter` в символах Unicode (`60`).
const MAX_KEYWORD_ITEM_CHARS: usize = 60;

/// Запрос на создание или обновление правила автомодерации гильдии
/// (`POST /api/v9/guilds/{guild_id}/auto-moderation/rules` или `PATCH /api/v9/guilds/{guild_id}/auto-moderation/rules/{rule_id}`).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct UpsertAutoModRuleRequest {
    /// Название правила (`1..=MAX_AUTOMOD_NAME_CHARS` символов Unicode после `trim()`).
    pub name: String,
    /// Тип события (`1 = MESSAGE_SEND`).
    pub event_type: u8,
    /// Тип триггера (`1 = KEYWORD`, `3 = SPAM`, `4 = KEYWORD_PRESET`, `5 = MENTION_SPAM`).
    pub trigger_type: u8,
    /// Список ключевых слов/паттернов для `AUTOMOD_TRIGGER_KEYWORD` (`1..=1000` элементов по `1..=60` символов).
    pub keyword_filter: Vec<String>,
    /// Лимит упоминаний для `AUTOMOD_TRIGGER_MENTION_SPAM` (`1..=50`).
    pub mention_total_limit: Option<u8>,
    /// Тип действия (`1 = BLOCK_MESSAGE`, `2 = SEND_ALERT`, `3 = TIMEOUT`).
    pub action_type: u8,
    /// Длительность таймаута в секундах для `AUTOMOD_ACTION_TIMEOUT` (`1..=2_419_200`).
    pub timeout_duration_secs: Option<u32>,
    /// Признак активности правила.
    pub enabled: bool,
}

/// Проверяет, что строка является валидным положительным числовым Snowflake ID
/// (`1..=20` ASCII-цифр, не начинается с `'0'`).
fn validate_snowflake(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 20
        || value.starts_with('0')
        || !value.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(format!("некорректный Snowflake ID в поле {field}"));
    }
    Ok(())
}

/// Валидирует запрос на создание или обновление правила автомодерации [`UpsertAutoModRuleRequest`]:
/// - `req.name.trim()` должен содержать от `1` до [`MAX_AUTOMOD_NAME_CHARS`] (`100`) символов Unicode;
/// - `req.event_type` должен быть равен [`AUTOMOD_EVENT_MESSAGE_SEND`] (`1`);
/// - при `trigger_type == 1` (`KEYWORD`): `keyword_filter` должен содержать `1..=1000` элементов,
///   каждый после `trim()` непустой и длиной `<= 60` символов Unicode;
/// - при `trigger_type == 5` (`MENTION_SPAM`): `mention_total_limit` должен быть `Some(1..=50)`;
/// - при `trigger_type == 3` (`SPAM`) или `4` (`KEYWORD_PRESET`): допустимы;
/// - иные значения `trigger_type` возвращают `Err`;
/// - `action_type` должен быть `1`, `2` или `3`; при `action_type == 3` (`TIMEOUT`)
///   `timeout_duration_secs` должен быть `Some(1..=2_419_200)`.
pub fn validate_automod_rule_request(req: &UpsertAutoModRuleRequest) -> Result<(), String> {
    let name_len = req.name.trim().chars().count();
    if !(1..=MAX_AUTOMOD_NAME_CHARS).contains(&name_len) {
        return Err(format!(
            "длина названия правила автомодерации после trim должна быть от 1 до {MAX_AUTOMOD_NAME_CHARS} символов (получено {name_len})"
        ));
    }

    if req.event_type != AUTOMOD_EVENT_MESSAGE_SEND {
        return Err(format!(
            "неподдерживаемый event_type={} (ожидается {AUTOMOD_EVENT_MESSAGE_SEND} = MESSAGE_SEND)",
            req.event_type
        ));
    }

    match req.trigger_type {
        AUTOMOD_TRIGGER_KEYWORD => {
            let count = req.keyword_filter.len();
            if !(1..=MAX_KEYWORD_FILTER_ITEMS).contains(&count) {
                return Err(format!(
                    "для триггера KEYWORD список keyword_filter должен содержать от 1 до {MAX_KEYWORD_FILTER_ITEMS} элементов (получено {count})"
                ));
            }
            for item in &req.keyword_filter {
                let item_len = item.trim().chars().count();
                if !(1..=MAX_KEYWORD_ITEM_CHARS).contains(&item_len) {
                    return Err(format!(
                        "элемент keyword_filter после trim должен содержать от 1 до {MAX_KEYWORD_ITEM_CHARS} символов (получено {item_len})"
                    ));
                }
            }
        }
        AUTOMOD_TRIGGER_SPAM | AUTOMOD_TRIGGER_KEYWORD_PRESET => {}
        AUTOMOD_TRIGGER_MENTION_SPAM => {
            let limit = req.mention_total_limit.ok_or_else(|| {
                "для триггера MENTION_SPAM обязателен параметр mention_total_limit".to_string()
            })?;
            if !(1..=MAX_MENTION_TOTAL_LIMIT).contains(&limit) {
                return Err(format!(
                    "mention_total_limit должен быть в диапазоне 1..={MAX_MENTION_TOTAL_LIMIT} (получено {limit})"
                ));
            }
        }
        other => {
            return Err(format!("неподдерживаемый trigger_type={other}"));
        }
    }

    match req.action_type {
        AUTOMOD_ACTION_BLOCK_MESSAGE | AUTOMOD_ACTION_SEND_ALERT => {}
        AUTOMOD_ACTION_TIMEOUT => {
            let secs = req.timeout_duration_secs.ok_or_else(|| {
                "для действия TIMEOUT обязателен параметр timeout_duration_secs".to_string()
            })?;
            if !(1..=MAX_TIMEOUT_DURATION_SECS).contains(&secs) {
                return Err(format!(
                    "timeout_duration_secs должен быть в диапазоне 1..={MAX_TIMEOUT_DURATION_SECS} (получено {secs})"
                ));
            }
        }
        other => {
            return Err(format!("неподдерживаемый action_type={other}"));
        }
    }

    Ok(())
}

/// Валидирует `guild_id` (и `rule_id`, если передан `Some`) как положительные числовые Snowflake
/// (`1..=20` ASCII-цифр, без ведущего `'0'`) и возвращает REST-маршрут:
/// - `"/api/v9/guilds/{guild_id}/auto-moderation/rules"` при `rule_id == None`;
/// - `"/api/v9/guilds/{guild_id}/auto-moderation/rules/{rule_id}"` при `rule_id == Some(id)`.
pub fn build_guild_automod_route(guild_id: &str, rule_id: Option<&str>) -> Result<String, String> {
    validate_snowflake("guild_id", guild_id)?;
    match rule_id {
        Some(rid) => {
            validate_snowflake("rule_id", rid)?;
            Ok(format!(
                "/api/v9/guilds/{guild_id}/auto-moderation/rules/{rid}"
            ))
        }
        None => Ok(format!("/api/v9/guilds/{guild_id}/auto-moderation/rules")),
    }
}
