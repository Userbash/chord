//! RW-40 / task-258 (`BE-P1-30`): BFF-контракт онбординга и экрана приветствия гильдии (`onboarding`).
//!
//! Источники контракта:
//! - `/home/sanya/re-deep/rest-static.json:1001-1015` (`Endpoints.GUILD_ONBOARDING: "/guilds/{arg}/onboarding"`);
//! - `/home/sanya/re-deep/rest-static.json:1542-1558` (`Endpoints.GUILD_WELCOME_SCREEN: "/guilds/{arg}/welcome-screen"`);
//! - Discord API v9:
//!   - режимы онбординга: `0 = ONBOARDING_DEFAULT`, `1 = ONBOARDING_ADVANCED`;
//!   - лимит описания экрана приветствия `1..=140` символов Unicode после `trim()`;
//!   - лимит каналов экрана приветствия `<= 5`;
//!   - лимит описания каждого канала экрана приветствия `1..=50` символов Unicode после `trim()`.

use serde::{Deserialize, Serialize};

/// Режим онбординга гильдии по умолчанию (`0 = ONBOARDING_DEFAULT`).
pub const ONBOARDING_MODE_DEFAULT: u8 = 0;

/// Расширенный режим онбординга гильдии (`1 = ONBOARDING_ADVANCED`).
pub const ONBOARDING_MODE_ADVANCED: u8 = 1;

/// Максимальная длина описания экрана приветствия в символах Unicode после `trim()` (`140`).
pub const MAX_WELCOME_DESCRIPTION_CHARS: usize = 140;

/// Максимальное количество каналов на экране приветствия гильдии (`5`).
pub const MAX_WELCOME_CHANNELS: usize = 5;

/// Максимальная длина описания канала на экране приветствия в символах Unicode после `trim()` (`50`).
pub const MAX_WELCOME_CHANNEL_DESC_CHARS: usize = 50;

/// Элемент списка каналов на экране приветствия гильдии (`welcome_channels`).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct WelcomeScreenChannelEntry {
    /// Идентификатор канала (положительный числовой Snowflake `1..=20` ASCII-цифр, без ведущего `'0'`).
    pub channel_id: String,
    /// Краткое описание канала (`1..=50` символов Unicode после `trim()`).
    pub description: String,
    /// Опциональный Snowflake кастомного эмодзи.
    pub emoji_id: Option<String>,
    /// Опциональное имя кастомного эмодзи или Unicode-эмодзи.
    pub emoji_name: Option<String>,
}

/// Запрос на обновление экрана приветствия гильдии (`PATCH /api/v9/guilds/{guild_id}/welcome-screen`).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct UpdateWelcomeScreenRequest {
    /// Признак активности экрана приветствия.
    pub enabled: bool,
    /// Опциональное описание сервера на экране приветствия (`1..=140` символов Unicode после `trim()`).
    pub description: Option<String>,
    /// Рекомендуемые каналы на экране приветствия (`0..=5` элементов).
    pub welcome_channels: Vec<WelcomeScreenChannelEntry>,
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

/// Валидирует запрос обновления экрана приветствия гильдии [`UpdateWelcomeScreenRequest`]:
/// - если `req.description` равен `Some(ref d)`, длина `d.trim()` должна быть от `1` до
///   [`MAX_WELCOME_DESCRIPTION_CHARS`] (`140`) символов Unicode;
/// - количество элементов `req.welcome_channels` не превышает [`MAX_WELCOME_CHANNELS`] (`5`);
/// - для каждого канала в `req.welcome_channels`:
///   * `channel_id` — валидный положительный числовой Snowflake (`1..=20` ASCII-цифр, без ведущего `'0'`);
///   * `description.trim()` — длина от `1` до [`MAX_WELCOME_CHANNEL_DESC_CHARS`] (`50`) символов Unicode;
///   * если `emoji_id` равен `Some(ref eid)`, `eid` — валидный положительный числовой Snowflake.
pub fn validate_welcome_screen_request(req: &UpdateWelcomeScreenRequest) -> Result<(), String> {
    if let Some(ref desc) = req.description {
        let desc_len = desc.trim().chars().count();
        if !(1..=MAX_WELCOME_DESCRIPTION_CHARS).contains(&desc_len) {
            return Err(format!(
                "длина описания экрана приветствия после trim должна быть от 1 до {MAX_WELCOME_DESCRIPTION_CHARS} символов (получено {desc_len})"
            ));
        }
    }

    if req.welcome_channels.len() > MAX_WELCOME_CHANNELS {
        return Err(format!(
            "количество каналов экрана приветствия не может превышать {MAX_WELCOME_CHANNELS} (получено {})",
            req.welcome_channels.len()
        ));
    }

    for entry in &req.welcome_channels {
        validate_snowflake("channel_id", &entry.channel_id)?;

        let entry_desc_len = entry.description.trim().chars().count();
        if !(1..=MAX_WELCOME_CHANNEL_DESC_CHARS).contains(&entry_desc_len) {
            return Err(format!(
                "длина описания канала экрана приветствия после trim должна быть от 1 до {MAX_WELCOME_CHANNEL_DESC_CHARS} символов (получено {entry_desc_len})"
            ));
        }

        if let Some(ref eid) = entry.emoji_id {
            validate_snowflake("emoji_id", eid)?;
        }
    }

    Ok(())
}

/// Валидирует `guild_id` как положительный числовой Snowflake (`1..=20` ASCII-цифр, без ведущего `'0'`)
/// и возвращает REST-маршрут экрана приветствия гильдии `"/api/v9/guilds/{guild_id}/welcome-screen"`.
pub fn build_guild_welcome_screen_route(guild_id: &str) -> Result<String, String> {
    validate_snowflake("guild_id", guild_id)?;
    Ok(format!("/api/v9/guilds/{guild_id}/welcome-screen"))
}

/// Валидирует `guild_id` как положительный числовой Snowflake (`1..=20` ASCII-цифр, без ведущего `'0'`)
/// и возвращает REST-маршрут онбординга гильдии `"/api/v9/guilds/{guild_id}/onboarding"`.
pub fn build_guild_onboarding_route(guild_id: &str) -> Result<String, String> {
    validate_snowflake("guild_id", guild_id)?;
    Ok(format!("/api/v9/guilds/{guild_id}/onboarding"))
}
