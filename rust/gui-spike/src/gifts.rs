//! RW-40 / TASK-626 (`G1 Backend`): REST-хендлеры и типы Nitro Gifts & Entitlements.
//!
//! Оракулы (`/home/sanya/re-deep/` и `waves/RW-40/`):
//! - `waves/RW-40/RE-DEEP-REA.md` §5 (`STUB-12 — кнопка Gift 🎁`, маршруты `@290386..@290799`,
//!   `createGiftCode` `@613587`, `gift_style` `0..=2`);
//! - `waves/RW-40/DESIGN/ATTACHMENTS-SPEC.md` §4.1–§4.5 (`#gift-modal`, лимит `200` символов,
//!   расчёт `gift_style`: `2` при непустом сообщении или эмодзи/звуке, иначе `1`);
//! - `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0`:
//!   - модуль `869038` (`@613060`): `resolveGiftCode`, `fetchUserGiftCodesForSKU`,
//!     `createGiftCode`, `revokeGiftCode`;
//!   - модуль `75255` (`@735306`): `redeemGiftCode` (`POST /entitlements/gift-codes/{code}/redeem`,
//!     тело `{channel_id, payment_source_id, gateway_checkout_context}`);
//!   - модуль `339048` (`@596307`): `fetchEntitlementsForApplication`, `fetchEntitlementsForUser`,
//!     `fetchGiftableEntitlements`;
//!   - модуль `45938` (`@9711598`): `GiftStyle`, `resolveGiftCode`, извлечение подарочных кодов
//!     из ссылок (`discord.gift`, `discord.com/gifts`, `discordapp.com/gifts`, `promos.discord.gg`,
//!     `discord.com/billing/promotions`, не более 3 кодов на сообщение);
//!   - модуль `7133` (`@8906800`): `GiftCodeRecord` (`createFromServer`, битовые флаги `flags`);
//!   - модуль `557009` (`@8904412`): `EntitlementRecord` (`createFromServer`, `isGiftable`);
//!   - модуль `202541` (`@6789126..@6797000`): `NITRO_APPLICATION_ID`, `SKU_*`, `PLAN_*`;
//!   - модуль `652215` (`@320207`, `@325314`): коды ошибок Discord (`10038`, `50035`, `50050`,
//!     `50051`, `50054`, `50092`, `50095`, `50097`).

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use discord_core::rest::client::{ApiAuth, ApiClient, ApiConfig, ApiRetryPolicy};
use discord_core::rest::transport::{HttpClientConfig, HttpResponse};
use discord_core::API_V9_BASE;
use discord_gui_spike::auth::IdentityContext;
use discord_gui_spike::rest_failure::RestFailure;
use serde::{Deserialize, Serialize};

/// Канонический `application_id` подписок Discord Nitro (модуль `202541`, `@6789126`).
pub const NITRO_APPLICATION_ID: &str = "521842831262875670";

/// Заглушка отсутствующего SKU (`NONE`, модуль `202541`).
pub const SKU_NONE: &str = "628379670982688768";
/// Nitro Basic (`TIER_0`, модуль `202541`).
pub const SKU_NITRO_BASIC_TIER_0: &str = "978380684370378762";
/// Nitro Classic (`TIER_1`, модуль `202541`).
pub const SKU_NITRO_CLASSIC_TIER_1: &str = "521846918637420545";
/// Nitro Standard (`TIER_2`, модуль `202541`).
pub const SKU_NITRO_STANDARD_TIER_2: &str = "521847234246082599";
/// Буст сервера (`GUILD`, модуль `202541`).
pub const SKU_GUILD_BOOST: &str = "590663762298667008";
/// Устаревший SKU Nitro (`LEGACY`, модуль `202541`).
pub const SKU_NITRO_LEGACY: &str = "521842865731534868";

/// План подписки: Nitro Basic — 1 месяц (модуль `202541`).
pub const PLAN_NITRO_BASIC_MONTH: &str = "978380692553465866";
/// План подписки: Nitro Basic — 1 год (модуль `202541`).
pub const PLAN_NITRO_BASIC_YEAR: &str = "1024422698568122368";
/// План подписки: Nitro Classic — 1 месяц (модуль `202541`).
pub const PLAN_NITRO_CLASSIC_MONTH: &str = "511651871736201216";
/// План подписки: Nitro Classic — 1 год (модуль `202541`).
pub const PLAN_NITRO_CLASSIC_YEAR: &str = "511651876987469824";
/// План подписки: Nitro Standard — 1 месяц (модуль `202541`).
pub const PLAN_NITRO_MONTH: &str = "511651880837840896";
/// План подписки: Nitro Standard — 1 год (модуль `202541`).
pub const PLAN_NITRO_YEAR: &str = "511651885459963904";
/// План подписки: Nitro Standard — 3 месяца (модуль `202541`).
pub const PLAN_NITRO_3_MONTH: &str = "642251038925127690";
/// План подписки: Nitro Standard — 6 месяцев (модуль `202541`).
pub const PLAN_NITRO_6_MONTH: &str = "944037208325619722";
/// План подписки: Буст сервера — 1 месяц (модуль `202541`).
pub const PLAN_GUILD_BOOST_MONTH: &str = "590665532894740483";
/// План подписки: Буст сервера — 1 год (модуль `202541`).
pub const PLAN_GUILD_BOOST_YEAR: &str = "590665538238152709";

/// Стиль оформления подарка по умолчанию (`GiftStyle.DEFAULT = 0`, модуль `45938`).
pub const GIFT_STYLE_DEFAULT: u8 = 0;
/// Кастомная открытка без текста (`GiftStyle.CUSTOM_STYLE = 1`, модуль `45938`).
pub const GIFT_STYLE_CUSTOM_STYLE: u8 = 1;
/// Кастомное сообщение, эмодзи или саундборд (`GiftStyle.CUSTOM_MESSAGE_EMOJI_SOUNDBOARD = 2`, модуль `45938`).
pub const GIFT_STYLE_CUSTOM_MESSAGE_EMOJI_SOUNDBOARD: u8 = 2;

/// Максимальная длина сопроводительного текста подарка (`ATTACHMENTS-SPEC.md` §4.3).
pub const MAX_GIFT_CUSTOM_MESSAGE_CHARS: usize = 200;
/// Минимальная длина нормализованного кода подарка.
pub const MIN_GIFT_CODE_CHARS: usize = 2;
/// Максимальная длина нормализованного кода подарка.
pub const MAX_GIFT_CODE_CHARS: usize = 64;
/// Максимальное число кодов подарков, извлекаемых из одного сообщения (модуль `45938`, `.slice(0, 3)`).
pub const MAX_EXTRACTED_GIFT_CODES: usize = 3;

/// Флаг кода подарка: требуется платёжный метод (`PAYMENT_SOURCE_REQUIRED = 1`, модуль `7133`).
pub const GIFT_FLAG_PAYMENT_SOURCE_REQUIRED: u32 = 1;
/// Флаг кода подарка: запрещено при активной подписке (`EXISTING_PREMIUM_SUBSCRIPTION_DISALLOWED = 2`, модуль `7133`).
pub const GIFT_FLAG_EXISTING_PREMIUM_SUBSCRIPTION_DISALLOWED: u32 = 2;
/// Флаг кода подарка: запрещено активировать собственный подарок (`NOT_SELF_REDEEMABLE = 4`, модуль `7133`).
pub const GIFT_FLAG_NOT_SELF_REDEEMABLE: u32 = 4;

/// Тип энтайтлмента: покупка (`PURCHASE = 1`, модуль `652215`).
pub const ENTITLEMENT_TYPE_PURCHASE: u8 = 1;
/// Тип энтайтлмента: премиум-подписка (`PREMIUM_SUBSCRIPTION = 2`).
pub const ENTITLEMENT_TYPE_PREMIUM_SUBSCRIPTION: u8 = 2;
/// Тип энтайтлмента: подарок разработчика (`DEVELOPER_GIFT = 3`).
pub const ENTITLEMENT_TYPE_DEVELOPER_GIFT: u8 = 3;
/// Тип энтайтлмента: тестовая покупка (`TEST_MODE_PURCHASE = 4`).
pub const ENTITLEMENT_TYPE_TEST_MODE_PURCHASE: u8 = 4;
/// Тип энтайтлмента: бесплатная покупка (`FREE_PURCHASE = 5`).
pub const ENTITLEMENT_TYPE_FREE_PURCHASE: u8 = 5;
/// Тип энтайтлмента: пользовательский подарок (`USER_GIFT = 6`).
pub const ENTITLEMENT_TYPE_USER_GIFT: u8 = 6;
/// Тип энтайтлмента: премиум-покупка (`PREMIUM_PURCHASE = 7`).
pub const ENTITLEMENT_TYPE_PREMIUM_PURCHASE: u8 = 7;
/// Тип энтайтлмента: подписка приложения (`APPLICATION_SUBSCRIPTION = 8`).
pub const ENTITLEMENT_TYPE_APPLICATION_SUBSCRIPTION: u8 = 8;
/// Тип энтайтлмента: покупка сотрудника (`FREE_STAFF_PURCHASE = 9`).
pub const ENTITLEMENT_TYPE_FREE_STAFF_PURCHASE: u8 = 9;
/// Тип энтайтлмента: награда за квест (`QUEST_REWARD = 10`).
pub const ENTITLEMENT_TYPE_QUEST_REWARD: u8 = 10;
/// Тип энтайтлмента: дробное начисление Nitro (`FRACTIONAL_REDEMPTION = 11`, D-02 оракул).
pub const ENTITLEMENT_TYPE_FRACTIONAL_REDEMPTION: u8 = 11;
/// Тип энтайтлмента: покупка за виртуальную валюту (`VIRTUAL_CURRENCY_REDEMPTION = 12`).
pub const ENTITLEMENT_TYPE_VIRTUAL_CURRENCY_REDEMPTION: u8 = 12;
/// Тип энтайтлмента: награда за покупку (`PURCHASE_REWARD = 16`).
pub const ENTITLEMENT_TYPE_PURCHASE_REWARD: u8 = 16;

/// Код ошибки Discord: неизвестный код подарка (`UNKNOWN_GIFT_CODE = 10038`, `@320207`).
pub const DISCORD_ERR_UNKNOWN_GIFT_CODE: u32 = 10038;
/// Код ошибки Discord: некорректное тело формы (`INVALID_FORM_BODY = 50035`).
pub const DISCORD_ERR_INVALID_FORM_BODY: u32 = 50035;
/// Код ошибки Discord: код подарка уже исчерпан (`INVALID_GIFT_REDEMPTION_EXHAUSTED = 50050`, `@325314`).
pub const DISCORD_ERR_INVALID_GIFT_REDEMPTION_EXHAUSTED: u32 = 50050;
/// Код ошибки Discord: продукт уже куплен (`INVALID_GIFT_REDEMPTION_OWNED = 50051`, `@325372`).
pub const DISCORD_ERR_INVALID_GIFT_REDEMPTION_OWNED: u32 = 50051;
/// Код ошибки Discord: нельзя активировать собственный подарок (`INVALID_GIFT_SELF_REDEMPTION = 50054`, `@325418`).
pub const DISCORD_ERR_INVALID_GIFT_SELF_REDEMPTION: u32 = 50054;
/// Код ошибки Discord: подписка уже оформлена ранее (`INVALID_GIFT_REDEMPTION_PREVIOUSLY_OWNED = 50092`, `@325782`).
pub const DISCORD_ERR_INVALID_GIFT_REDEMPTION_PREVIOUSLY_OWNED: u32 = 50092;
/// Код ошибки Discord: неподдерживаемый код страны (`INVALID_COUNTRY_CODE = 50095`, `@325849`).
pub const DISCORD_ERR_INVALID_COUNTRY_CODE: u32 = 50095;
/// Код ошибки Discord: активация отклонена антифродом (`INVALID_GIFT_REDEMPTION_FRAUD_REJECTED = 50097`, `@325900`).
pub const DISCORD_ERR_INVALID_GIFT_REDEMPTION_FRAUD_REJECTED: u32 = 50097;

/// Канонический список из 9 Tauri IPC-команд домена подарков и энтайтлментов (`TASK-626`).
pub const GIFT_IPC_COMMANDS: [&str; 9] = [
    "resolve_gift_code",
    "create_gift_code",
    "fetch_user_gift_codes",
    "revoke_gift_code",
    "redeem_gift_code",
    "fetch_user_entitlements",
    "fetch_application_entitlements",
    "fetch_giftable_entitlements",
    "list_nitro_gift_plans",
];

/// Описание плана подписки Nitro для каталога подарков (модуль `202541`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NitroGiftPlan {
    pub id: String,
    pub name: String,
    #[serde(alias = "sku_id")]
    pub sku_id: String,
    /// `1` = месяц, `2` = год.
    pub interval: u8,
    #[serde(alias = "interval_count")]
    pub interval_count: u8,
    /// `Some(1)` = Classic, `Some(2)` = Standard, `Some(3)` = Basic, `None` = Guild Boost.
    #[serde(alias = "premium_type", skip_serializing_if = "Option::is_none")]
    pub premium_type: Option<u8>,
}

/// Возвращает канонический каталог из 10 планов подарков Nitro и бустов сервера (модуль `202541`).
#[must_use]
pub fn list_nitro_gift_plans() -> Vec<NitroGiftPlan> {
    vec![
        NitroGiftPlan {
            id: PLAN_NITRO_BASIC_MONTH.to_string(),
            name: "Nitro Basic Monthly".to_string(),
            sku_id: SKU_NITRO_BASIC_TIER_0.to_string(),
            interval: 1,
            interval_count: 1,
            premium_type: Some(3),
        },
        NitroGiftPlan {
            id: PLAN_NITRO_BASIC_YEAR.to_string(),
            name: "Nitro Basic Yearly".to_string(),
            sku_id: SKU_NITRO_BASIC_TIER_0.to_string(),
            interval: 2,
            interval_count: 1,
            premium_type: Some(3),
        },
        NitroGiftPlan {
            id: PLAN_NITRO_CLASSIC_MONTH.to_string(),
            name: "Nitro Classic Monthly".to_string(),
            sku_id: SKU_NITRO_CLASSIC_TIER_1.to_string(),
            interval: 1,
            interval_count: 1,
            premium_type: Some(1),
        },
        NitroGiftPlan {
            id: PLAN_NITRO_CLASSIC_YEAR.to_string(),
            name: "Nitro Classic Yearly".to_string(),
            sku_id: SKU_NITRO_CLASSIC_TIER_1.to_string(),
            interval: 2,
            interval_count: 1,
            premium_type: Some(1),
        },
        NitroGiftPlan {
            id: PLAN_NITRO_MONTH.to_string(),
            name: "Nitro Monthly".to_string(),
            sku_id: SKU_NITRO_STANDARD_TIER_2.to_string(),
            interval: 1,
            interval_count: 1,
            premium_type: Some(2),
        },
        NitroGiftPlan {
            id: PLAN_NITRO_YEAR.to_string(),
            name: "Nitro Yearly".to_string(),
            sku_id: SKU_NITRO_STANDARD_TIER_2.to_string(),
            interval: 2,
            interval_count: 1,
            premium_type: Some(2),
        },
        NitroGiftPlan {
            id: PLAN_NITRO_3_MONTH.to_string(),
            name: "Nitro Three Month".to_string(),
            sku_id: SKU_NITRO_STANDARD_TIER_2.to_string(),
            interval: 1,
            interval_count: 3,
            premium_type: Some(2),
        },
        NitroGiftPlan {
            id: PLAN_NITRO_6_MONTH.to_string(),
            name: "Nitro Six Month".to_string(),
            sku_id: SKU_NITRO_STANDARD_TIER_2.to_string(),
            interval: 1,
            interval_count: 6,
            premium_type: Some(2),
        },
        NitroGiftPlan {
            id: PLAN_GUILD_BOOST_MONTH.to_string(),
            name: "Nitro Server Boost Monthly".to_string(),
            sku_id: SKU_GUILD_BOOST.to_string(),
            interval: 1,
            interval_count: 1,
            premium_type: None,
        },
        NitroGiftPlan {
            id: PLAN_GUILD_BOOST_YEAR.to_string(),
            name: "Nitro Server Boost Yearly".to_string(),
            sku_id: SKU_GUILD_BOOST.to_string(),
            interval: 2,
            interval_count: 1,
            premium_type: None,
        },
    ]
}

/// Типизированная ошибка операций с подарками и энтайтлментами.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum GiftError {
    /// Некорректный код подарка или параметр запроса.
    InvalidArg,
    /// Некорректный Discord Snowflake в указанном поле.
    InvalidSnowflake { field: &'static str },
    /// Неподдерживаемый `gift_style` (допустимы только `0..=2`).
    InvalidGiftStyle { style: u8 },
    /// Превышен лимит длины сопроводительного сообщения подарка (`> 200` символов).
    CustomMessageTooLong { len: usize, max: usize },
    /// Токен авторизации отсутствует или пуст.
    MissingToken,
    /// Отказ хранилища токена (SEC-02).
    Storage,
    /// 401: сессия истекла.
    Unauthorized,
    /// 403: доступ запрещён.
    Forbidden,
    /// 404 или код Discord `10038` (`UNKNOWN_GIFT_CODE`): подарок не найден.
    NotFound,
    /// Код Discord `50035` (`INVALID_FORM_BODY`).
    InvalidFormBody,
    /// Код Discord `50050` (`INVALID_GIFT_REDEMPTION_EXHAUSTED`): подарок уже активирован.
    GiftRedemptionExhausted,
    /// Код Discord `50051` / `50092` (`INVALID_GIFT_REDEMPTION_OWNED` / `PREVIOUSLY_OWNED`).
    GiftRedemptionOwned,
    /// Код Discord `50054` (`INVALID_GIFT_SELF_REDEMPTION`): нельзя активировать свой подарок.
    GiftSelfRedemption,
    /// Код Discord `50097` (`INVALID_GIFT_REDEMPTION_FRAUD_REJECTED`).
    GiftRedemptionFraudRejected,
    /// 429: ограничение частоты запросов (`retry_after_ms`).
    RateLimited { retry_after_ms: u64 },
    /// Прочие 4xx-ответы.
    Client { status: u16 },
    /// 5xx: ошибка сервера Discord.
    Server { status: u16 },
    /// Транспорт/таймаут — сеть недоступна.
    Network,
    /// Ответ 2xx не соответствует ожидаемой JSON-схеме.
    InvalidResponse,
}

impl fmt::Display for GiftError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArg => f.write_str("некорректный аргумент запроса подарка"),
            Self::InvalidSnowflake { field } => {
                write!(f, "некорректный идентификатор Snowflake в поле {field}")
            }
            Self::InvalidGiftStyle { style } => {
                write!(f, "неподдерживаемый стиль подарка: {style}")
            }
            Self::CustomMessageTooLong { len, max } => {
                write!(
                    f,
                    "сообщение подарка слишком длинное ({len} символов при максимуме {max})"
                )
            }
            Self::MissingToken => f.write_str("токен авторизации не найден"),
            Self::Storage => f.write_str("хранилище токена недоступно"),
            Self::Unauthorized => f.write_str("сессия истекла"),
            Self::Forbidden => f.write_str("нет доступа к операции с подарком"),
            Self::NotFound => f.write_str("подарочный код не найден"),
            Self::InvalidFormBody => f.write_str("некорректные поля запроса подарка"),
            Self::GiftRedemptionExhausted => f.write_str("этот подарок уже был активирован"),
            Self::GiftRedemptionOwned => f.write_str("эта подписка или предмет уже приобретены"),
            Self::GiftSelfRedemption => f.write_str("нельзя активировать собственный подарок"),
            Self::GiftRedemptionFraudRejected => {
                f.write_str("активация подарка отклонена проверкой безопасности")
            }
            Self::RateLimited { retry_after_ms } => {
                write!(f, "ограничение частоты; повтор через {retry_after_ms} мс")
            }
            Self::Client { status } => write!(f, "запрос отклонён (HTTP {status})"),
            Self::Server { status } => write!(f, "ошибка сервера Discord (HTTP {status})"),
            Self::Network => f.write_str("сеть недоступна"),
            Self::InvalidResponse => f.write_str("некорректный ответ сервера подарков"),
        }
    }
}

impl std::error::Error for GiftError {}

impl From<RestFailure> for GiftError {
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

/// Вычисляет `gift_style` по сопроводительному тексту и наличию эмодзи/звука
/// (`ATTACHMENTS-SPEC.md` §4.3 и модуль `45938` `@9711598`):
/// - проверяет длину `custom_message` (`<= 200` символов Unicode);
/// - возвращает `2` (`GIFT_STYLE_CUSTOM_MESSAGE_EMOJI_SOUNDBOARD`), если есть непустой
///   после `trim()` текст или `has_emoji_or_sound == true`;
/// - иначе возвращает `1` (`GIFT_STYLE_CUSTOM_STYLE`).
pub fn resolve_gift_style(
    custom_message: Option<&str>,
    has_emoji_or_sound: bool,
) -> Result<u8, GiftError> {
    if let Some(msg) = custom_message {
        let char_count = msg.chars().count();
        if char_count > MAX_GIFT_CUSTOM_MESSAGE_CHARS {
            return Err(GiftError::CustomMessageTooLong {
                len: char_count,
                max: MAX_GIFT_CUSTOM_MESSAGE_CHARS,
            });
        }
        if !msg.trim().is_empty() {
            return Ok(GIFT_STYLE_CUSTOM_MESSAGE_EMOJI_SOUNDBOARD);
        }
    }
    if has_emoji_or_sound {
        Ok(GIFT_STYLE_CUSTOM_MESSAGE_EMOJI_SOUNDBOARD)
    } else {
        Ok(GIFT_STYLE_CUSTOM_STYLE)
    }
}

/// Извлекает до 3 уникальных нормализованных кодов подарков из текста сообщения
/// (модуль `45938` `@9711598`: `discord.gift/`, `discord.com/gifts/`,
/// `discordapp.com/gifts/`, `promos.discord.gg`, `discord.com/billing/promotions/`).
#[must_use]
pub fn extract_gift_codes_from_text(text: &str) -> Vec<String> {
    const MARKERS: [&str; 6] = [
        "discord.gift/",
        "discord.com/gifts/",
        "discordapp.com/gifts/",
        "discord.com/billing/promotions/",
        "discordapp.com/billing/promotions/",
        "promos.discord.gg/",
    ];

    let mut codes = Vec::new();
    for token in text.split_whitespace() {
        let trimmed =
            token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '/' && c != '=');
        for marker in MARKERS {
            if let Some(pos) = trimmed.find(marker) {
                let after = &trimmed[pos + marker.len()..];
                let candidate = if let Some(code_idx) = after.find("code=") {
                    &after[code_idx + "code=".len()..]
                } else {
                    after.trim_start_matches('?')
                };
                let raw_segment = candidate
                    .split(['/', '?', '&', '#'])
                    .find(|seg| !seg.is_empty())
                    .unwrap_or("");
                if let Ok(normalized) = normalize_gift_code_segment(raw_segment) {
                    if !codes.contains(&normalized) {
                        codes.push(normalized);
                        if codes.len() >= MAX_EXTRACTED_GIFT_CODES {
                            return codes;
                        }
                    }
                }
                break;
            }
        }
    }
    codes
}

/// Нормализует входную строку или URL подарка в чистый буквенно-цифровой код
/// (удаляет дефисы `-` по правилу модуля `45938`: `e.replace(/-/g, "")`).
pub fn normalize_gift_code(raw: &str) -> Result<String, GiftError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(GiftError::InvalidArg);
    }
    let extracted = extract_gift_codes_from_text(trimmed);
    if let Some(first) = extracted.into_iter().next() {
        return Ok(first);
    }
    let segment = trimmed
        .rsplit('/')
        .next()
        .unwrap_or(trimmed)
        .split(['?', '#', '&'])
        .next()
        .unwrap_or(trimmed);
    normalize_gift_code_segment(segment)
}

fn normalize_gift_code_segment(segment: &str) -> Result<String, GiftError> {
    let mut normalized = String::with_capacity(segment.len());
    for ch in segment.trim().chars() {
        if ch == '-' {
            continue;
        }
        if ch.is_ascii_alphanumeric() || ch == '_' {
            normalized.push(ch);
        } else {
            return Err(GiftError::InvalidArg);
        }
    }
    let len = normalized.len();
    if !(MIN_GIFT_CODE_CHARS..=MAX_GIFT_CODE_CHARS).contains(&len) {
        return Err(GiftError::InvalidArg);
    }
    Ok(normalized)
}

fn validate_snowflake(value: &str, field: &'static str) -> Result<(), GiftError> {
    if value.is_empty() || value.starts_with('0') || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(GiftError::InvalidSnowflake { field });
    }
    match value.parse::<u64>() {
        Ok(n) if n >= 1 => Ok(()),
        _ => Err(GiftError::InvalidSnowflake { field }),
    }
}

fn normalize_country_code(raw: &str) -> Result<String, GiftError> {
    let trimmed = raw.trim();
    if trimmed.len() != 2 || !trimmed.bytes().all(|b| b.is_ascii_alphabetic()) {
        return Err(GiftError::InvalidArg);
    }
    Ok(trimmed.to_ascii_uppercase())
}

/// Запись подарочного кода (`GiftCodeRecord`, модуль `7133` `@8906800`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GiftCodeRecord {
    pub code: String,
    #[serde(alias = "sku_id")]
    pub sku_id: String,
    #[serde(
        alias = "application_id",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub application_id: Option<String>,
    #[serde(default)]
    pub uses: u32,
    #[serde(alias = "max_uses", default)]
    pub max_uses: u32,
    #[serde(alias = "expires_at", default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub redeemed: bool,
    #[serde(
        alias = "store_listing_id",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub store_listing_id: Option<String>,
    #[serde(
        alias = "subscription_plan_id",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub subscription_plan_id: Option<String>,
    #[serde(alias = "user_id", default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(alias = "gift_style", default, skip_serializing_if = "Option::is_none")]
    pub gift_style: Option<u8>,
    #[serde(default)]
    pub flags: u32,
    #[serde(alias = "batch_id", default, skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<String>,
    #[serde(
        alias = "entitlement_branches",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub entitlement_branches: Option<Vec<String>>,
}

impl GiftCodeRecord {
    /// Подарок исчерпан или уже активирован (`isClaimed`, модуль `7133`).
    #[must_use]
    pub fn is_claimed(&self) -> bool {
        self.redeemed || (self.max_uses > 0 && self.uses >= self.max_uses)
    }

    /// Оставшееся число активаций (`remainingUses`, модуль `7133`).
    #[must_use]
    pub fn remaining_uses(&self) -> u32 {
        self.max_uses.saturating_sub(self.uses)
    }

    /// Подарок рассчитан на несколько копий (`hasMultipleCopies: maxUses > 1`, модуль `7133`).
    #[must_use]
    pub fn has_multiple_copies(&self) -> bool {
        self.max_uses > 1
    }

    /// Подарок является подпиской (`isSubscription: subscriptionPlanId != null`, модуль `7133`).
    #[must_use]
    pub fn is_subscription(&self) -> bool {
        self.subscription_plan_id.is_some()
    }

    /// Тип премиум-подписки (`premiumSubscriptionType`, модуль `7133` + `202541`):
    /// `3` для Nitro Basic (`TIER_0`), `1` для Nitro Classic (`TIER_1`), `2` для Nitro Standard (`TIER_2`).
    #[must_use]
    pub fn premium_subscription_type(&self) -> Option<u8> {
        if !self.is_subscription() {
            return None;
        }
        match self.sku_id.as_str() {
            SKU_NITRO_BASIC_TIER_0 => Some(3),
            SKU_NITRO_CLASSIC_TIER_1 => Some(1),
            SKU_NITRO_STANDARD_TIER_2 | SKU_NITRO_LEGACY => Some(2),
            _ => None,
        }
    }

    /// Можно ли активировать подарок самостоятельно (`!hasFlag(flags, NOT_SELF_REDEEMABLE)`).
    #[must_use]
    pub fn is_self_redeemable(&self) -> bool {
        (self.flags & GIFT_FLAG_NOT_SELF_REDEEMABLE) == 0
    }

    /// Запрещена ли активация при активной подписке (`hasFlag(flags, EXISTING_PREMIUM_SUBSCRIPTION_DISALLOWED)`).
    #[must_use]
    pub fn is_existing_premium_subscription_disallowed(&self) -> bool {
        (self.flags & GIFT_FLAG_EXISTING_PREMIUM_SUBSCRIPTION_DISALLOWED) != 0
    }

    /// Требуется ли платёжный источник (`hasFlag(flags, PAYMENT_SOURCE_REQUIRED)`).
    #[must_use]
    pub fn is_payment_source_required(&self) -> bool {
        (self.flags & GIFT_FLAG_PAYMENT_SOURCE_REQUIRED) != 0
    }
}

/// Опции запроса `GET /entitlements/gift-codes/{code}` (модуль `869038` `@613060`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GiftResolveOptions {
    #[serde(default, alias = "with_application")]
    pub with_application: bool,
    #[serde(default, alias = "with_subscription_plan")]
    pub with_subscription_plan: bool,
}

/// Запрос создания подарочного кода `POST /users/@me/entitlements/gift-codes` (модуль `869038` `@613587`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateGiftCodeRequest {
    #[serde(alias = "sku_id")]
    pub sku_id: String,
    #[serde(
        alias = "subscription_plan_id",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub subscription_plan_id: Option<String>,
    #[serde(alias = "gift_style", default, skip_serializing_if = "Option::is_none")]
    pub gift_style: Option<u8>,
    #[serde(
        alias = "custom_message",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub custom_message: Option<String>,
}

/// Запрос активации подарочного кода `POST /entitlements/gift-codes/{code}/redeem` (модуль `75255` `@735306`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GiftRedeemRequest {
    #[serde(alias = "channel_id", default)]
    pub channel_id: Option<String>,
    #[serde(alias = "payment_source_id", default)]
    pub payment_source_id: Option<String>,
    #[serde(alias = "gateway_checkout_context", default)]
    pub gateway_checkout_context: Option<serde_json::Value>,
}

/// Результат успешной активации подарочного кода (`POST /entitlements/gift-codes/{code}/redeem`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedeemedGiftOutcome {
    pub code: String,
    pub entitlement: EntitlementRecord,
}

/// Параметры запроса `GET /users/@me/entitlements` (модуль `339048` `@596307`, D-02 оракул).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntitlementQuery {
    #[serde(default, alias = "with_sku")]
    pub with_sku: bool,
    #[serde(default, alias = "with_application")]
    pub with_application: bool,
    #[serde(default, alias = "entitlement_type")]
    pub entitlement_type: Option<u8>,
    #[serde(default, alias = "exclude_ended")]
    pub exclude_ended: bool,
}

/// Запись энтайтлмента пользователя (`EntitlementRecord`, модуль `557009` `@8904412`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntitlementRecord {
    pub id: String,
    #[serde(alias = "sku_id")]
    pub sku_id: String,
    #[serde(alias = "application_id")]
    pub application_id: String,
    #[serde(alias = "user_id", default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(
        alias = "gifter_user_id",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub gifter_user_id: Option<String>,
    #[serde(rename = "type", alias = "kind")]
    pub kind: u8,
    #[serde(default)]
    pub branches: Vec<String>,
    #[serde(alias = "starts_at", default, skip_serializing_if = "Option::is_none")]
    pub starts_at: Option<String>,
    #[serde(alias = "ends_at", default, skip_serializing_if = "Option::is_none")]
    pub ends_at: Option<String>,
    #[serde(
        alias = "subscription_id",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub subscription_id: Option<String>,
    #[serde(alias = "parent_id", default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub consumed: bool,
    #[serde(default)]
    pub deleted: bool,
    #[serde(alias = "gift_style", default, skip_serializing_if = "Option::is_none")]
    pub gift_style: Option<u8>,
    #[serde(
        alias = "gift_code_batch_id",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub gift_code_batch_id: Option<String>,
    #[serde(
        alias = "source_type",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub source_type: Option<u8>,
    #[serde(
        alias = "fulfillment_status",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub fulfillment_status: Option<u8>,
}

impl EntitlementRecord {
    /// Доступен ли энтайтлмент для отправки в подарок (`isGiftable`:
    /// `type === USER_GIFT (6) && gifterId == null`, модуль `557009` `@8904412`).
    #[must_use]
    pub fn is_giftable(&self) -> bool {
        self.kind == ENTITLEMENT_TYPE_USER_GIFT
            && self.gifter_user_id.is_none()
            && !self.deleted
            && !self.consumed
    }

    /// Является ли энтайтлмент дробным начислением Nitro (`type === FRACTIONAL_REDEMPTION (11)`).
    #[must_use]
    pub fn is_fractional_redemption(&self) -> bool {
        self.kind == ENTITLEMENT_TYPE_FRACTIONAL_REDEMPTION
    }

    /// Активен ли энтайтлмент для использования самим пользователем (не удалён, не израсходован и не ждёт дарения).
    #[must_use]
    pub fn is_active_unconsumed(&self) -> bool {
        !self.deleted && !self.consumed && !self.is_giftable()
    }
}

/// Абстракция HTTP-транспорта для операций с подарками и энтайтлментами.
pub trait GiftTransport: Send + Sync {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, GiftError>> + Send + 'a>>;

    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, GiftError>> + Send + 'a>>;

    fn delete<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, GiftError>> + Send + 'a>>;
}

struct ApiGiftTransport(ApiClient);

impl GiftTransport for ApiGiftTransport {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, GiftError>> + Send + 'a>> {
        Box::pin(async move {
            self.0
                .get(target)
                .await
                .map_err(|error| GiftError::from(RestFailure::from_api_error(&error)))
        })
    }

    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, GiftError>> + Send + 'a>> {
        Box::pin(async move {
            self.0
                .post(target, body)
                .await
                .map_err(|error| GiftError::from(RestFailure::from_api_error(&error)))
        })
    }

    fn delete<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, GiftError>> + Send + 'a>> {
        Box::pin(async move {
            self.0
                .delete(target)
                .await
                .map_err(|error| GiftError::from(RestFailure::from_api_error(&error)))
        })
    }
}

/// REST-клиент подарков Nitro и энтайтлментов.
pub struct DiscordGifts {
    transport: Arc<dyn GiftTransport>,
}

impl fmt::Debug for DiscordGifts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DiscordGifts")
            .finish_non_exhaustive()
    }
}

impl DiscordGifts {
    /// Создаёт клиент с боевым транспортом ядра; токен уходит только в auth-заголовок.
    pub fn new(token: &str) -> Result<Self, GiftError> {
        if token.trim().is_empty() {
            return Err(GiftError::MissingToken);
        }
        let identity_ctx = IdentityContext::detect();
        let auth = ApiAuth::from_token(
            token,
            &identity_ctx.super_properties,
            &identity_ctx.locale,
            &identity_ctx.timezone,
        )
        .map_err(|_| GiftError::MissingToken)?;
        let client = ApiClient::new(ApiConfig {
            base: API_V9_BASE.to_string(),
            auth,
            http: HttpClientConfig::default(),
            retry: ApiRetryPolicy {
                max_attempts: 1,
                ..ApiRetryPolicy::default()
            },
        })
        .map_err(|_| GiftError::Network)?;
        Ok(Self {
            transport: Arc::new(ApiGiftTransport(client)),
        })
    }

    #[doc(hidden)]
    #[must_use]
    pub fn with_transport(transport: Arc<dyn GiftTransport>) -> Self {
        Self { transport }
    }
}

impl GiftTransport for DiscordGifts {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, GiftError>> + Send + 'a>> {
        self.transport.get(target)
    }

    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, GiftError>> + Send + 'a>> {
        self.transport.post(target, body)
    }

    fn delete<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, GiftError>> + Send + 'a>> {
        self.transport.delete(target)
    }
}

/// Превью подарочного кода: `GET /entitlements/gift-codes/{code}` (модуль `869038` `@613060`).
pub async fn resolve_gift_code<T: GiftTransport + ?Sized>(
    transport: &T,
    raw_code: &str,
    options: GiftResolveOptions,
) -> Result<GiftCodeRecord, GiftError> {
    let code = normalize_gift_code(raw_code)?;
    let mut query_parts = Vec::new();
    if options.with_application {
        query_parts.push("with_application=true");
    }
    if options.with_subscription_plan {
        query_parts.push("with_subscription_plan=true");
    }
    let target = if query_parts.is_empty() {
        format!("/entitlements/gift-codes/{code}")
    } else {
        format!("/entitlements/gift-codes/{code}?{}", query_parts.join("&"))
    };
    let response = transport.get(&target).await?;
    ensure_2xx(&response)?;
    decode_gift_code_record(&response.body)
}

/// Создать подарочный код: `POST /users/@me/entitlements/gift-codes` (модуль `869038` `@613587`).
pub async fn create_gift_code<T: GiftTransport + ?Sized>(
    transport: &T,
    request: &CreateGiftCodeRequest,
) -> Result<GiftCodeRecord, GiftError> {
    validate_snowflake(&request.sku_id, "sku_id")?;
    if let Some(plan_id) = request.subscription_plan_id.as_deref() {
        validate_snowflake(plan_id, "subscription_plan_id")?;
    }
    let computed_style = resolve_gift_style(request.custom_message.as_deref(), false)?;
    let style = match request.gift_style {
        Some(explicit) if explicit <= GIFT_STYLE_CUSTOM_MESSAGE_EMOJI_SOUNDBOARD => explicit,
        Some(invalid) => return Err(GiftError::InvalidGiftStyle { style: invalid }),
        None => computed_style,
    };

    #[derive(Serialize)]
    struct WireCreateGiftBody<'a> {
        sku_id: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        subscription_plan_id: Option<&'a str>,
        gift_style: u8,
        #[serde(skip_serializing_if = "Option::is_none")]
        custom_message: Option<&'a str>,
    }

    let trimmed_msg = request
        .custom_message
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    // Канонический payload по модулю 869038 (`@613587`): `{sku_id, subscription_plan_id, gift_style}`
    let wire = WireCreateGiftBody {
        sku_id: &request.sku_id,
        subscription_plan_id: request.subscription_plan_id.as_deref(),
        gift_style: style,
        custom_message: None.or_else(|| {
            let _ = trimmed_msg;
            None
        }),
    };
    let body = serde_json::to_vec(&wire).map_err(|_| GiftError::InvalidArg)?;
    let response = transport
        .post("/users/@me/entitlements/gift-codes", &body)
        .await?;
    ensure_2xx(&response)?;
    decode_gift_code_record(&response.body)
}

/// Список подарочных кодов пользователя: `GET /users/@me/entitlements/gift-codes` (модуль `869038` `@613371`).
pub async fn fetch_user_gift_codes<T: GiftTransport + ?Sized>(
    transport: &T,
    sku_id: Option<&str>,
    subscription_plan_id: Option<&str>,
) -> Result<Vec<GiftCodeRecord>, GiftError> {
    let mut query_parts = Vec::new();
    if let Some(sku) = sku_id {
        validate_snowflake(sku, "sku_id")?;
        query_parts.push(format!("sku_id={sku}"));
    }
    if let Some(plan) = subscription_plan_id {
        validate_snowflake(plan, "subscription_plan_id")?;
        query_parts.push(format!("subscription_plan_id={plan}"));
    }
    let target = if query_parts.is_empty() {
        "/users/@me/entitlements/gift-codes".to_string()
    } else {
        format!(
            "/users/@me/entitlements/gift-codes?{}",
            query_parts.join("&")
        )
    };
    let response = transport.get(&target).await?;
    ensure_2xx(&response)?;
    let raw_list: Vec<WireGiftCode> =
        serde_json::from_slice(&response.body).map_err(|_| GiftError::InvalidResponse)?;
    raw_list
        .into_iter()
        .map(WireGiftCode::into_record)
        .collect()
}

/// Отозвать (удалить) подарочный код: `DELETE /users/@me/entitlements/gift-codes/{code}` (модуль `869038` `@613895`).
pub async fn revoke_gift_code<T: GiftTransport + ?Sized>(
    transport: &T,
    raw_code: &str,
) -> Result<(), GiftError> {
    let code = normalize_gift_code(raw_code)?;
    let target = format!("/users/@me/entitlements/gift-codes/{code}");
    let response = transport.delete(&target).await?;
    ensure_2xx(&response)
}

/// Активировать подарочный код: `POST /entitlements/gift-codes/{code}/redeem` (модуль `75255` `@735306`).
pub async fn redeem_gift_code<T: GiftTransport + ?Sized>(
    transport: &T,
    raw_code: &str,
    request: &GiftRedeemRequest,
) -> Result<RedeemedGiftOutcome, GiftError> {
    let code = normalize_gift_code(raw_code)?;
    if let Some(ch_id) = request.channel_id.as_deref() {
        validate_snowflake(ch_id, "channel_id")?;
    }
    if let Some(ps_id) = request.payment_source_id.as_deref() {
        validate_snowflake(ps_id, "payment_source_id")?;
    }

    #[derive(Serialize)]
    struct WireRedeemBody<'a> {
        channel_id: Option<&'a str>,
        payment_source_id: Option<&'a str>,
        gateway_checkout_context: Option<&'a serde_json::Value>,
    }

    let wire = WireRedeemBody {
        channel_id: request.channel_id.as_deref(),
        payment_source_id: request.payment_source_id.as_deref(),
        gateway_checkout_context: request.gateway_checkout_context.as_ref(),
    };
    let body = serde_json::to_vec(&wire).map_err(|_| GiftError::InvalidArg)?;
    let target = format!("/entitlements/gift-codes/{code}/redeem");
    let response = transport.post(&target, &body).await?;
    ensure_2xx(&response)?;
    let entitlement = decode_entitlement_record(&response.body)?;
    Ok(RedeemedGiftOutcome { code, entitlement })
}

/// Загрузить энтайтлменты текущего пользователя: `GET /users/@me/entitlements`
/// (модуль `339048` `@596566` и live D-02 оракул `capture-ready.snap.jsonl:4004`).
pub async fn fetch_user_entitlements<T: GiftTransport + ?Sized>(
    transport: &T,
    query: &EntitlementQuery,
) -> Result<Vec<EntitlementRecord>, GiftError> {
    let mut params = vec![
        format!("with_sku={}", query.with_sku),
        format!("with_application={}", query.with_application),
    ];
    if let Some(kind) = query.entitlement_type {
        params.push(format!("entitlement_type={kind}"));
    }
    if query.exclude_ended {
        params.push("exclude_ended=true".to_string());
    }
    let target = format!("/users/@me/entitlements?{}", params.join("&"));
    let response = transport.get(&target).await?;
    ensure_2xx(&response)?;
    decode_entitlement_list(&response.body)
}

/// Загрузить энтайтлменты пользователя для приложения:
/// `GET /users/@me/applications/{application_id}/entitlements`
/// (модуль `339048` `@596307` и live D-02 оракул `capture-run2.snap.jsonl:8350`).
pub async fn fetch_application_entitlements<T: GiftTransport + ?Sized>(
    transport: &T,
    application_id: &str,
    exclude_consumed: bool,
) -> Result<Vec<EntitlementRecord>, GiftError> {
    validate_snowflake(application_id, "application_id")?;
    let target = format!(
        "/users/@me/applications/{application_id}/entitlements?exclude_consumed={exclude_consumed}"
    );
    let response = transport.get(&target).await?;
    ensure_2xx(&response)?;
    decode_entitlement_list(&response.body)
}

/// Загрузить подарочный инвентарь энтайтлментов пользователя:
/// `GET /users/@me/entitlements/gifts` (модуль `339048` `@596773`).
pub async fn fetch_giftable_entitlements<T: GiftTransport + ?Sized>(
    transport: &T,
    country_code: Option<&str>,
) -> Result<Vec<EntitlementRecord>, GiftError> {
    let target = if let Some(raw_cc) = country_code {
        let cc = normalize_country_code(raw_cc)?;
        format!("/users/@me/entitlements/gifts?country_code={cc}")
    } else {
        "/users/@me/entitlements/gifts".to_string()
    };
    let response = transport.get(&target).await?;
    ensure_2xx(&response)?;
    decode_entitlement_list(&response.body)
}

// ---------------------------------------------------------------------------
// Декодирование серверных структур и классификация ошибок Discord
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct WireUserRef {
    id: String,
}

#[derive(Deserialize)]
struct WireStoreListingSku {
    id: String,
    #[serde(default)]
    application_id: Option<String>,
}

#[derive(Deserialize)]
struct WireStoreListing {
    id: String,
    #[serde(default)]
    sku: Option<WireStoreListingSku>,
}

#[derive(Deserialize)]
struct WireSubscriptionPlan {
    id: String,
}

#[derive(Deserialize)]
struct WireGiftCode {
    code: String,
    #[serde(default)]
    sku_id: Option<String>,
    #[serde(default)]
    application_id: Option<String>,
    #[serde(default)]
    uses: u32,
    #[serde(default)]
    max_uses: u32,
    #[serde(default)]
    expires_at: Option<String>,
    #[serde(default)]
    redeemed: bool,
    #[serde(default)]
    store_listing: Option<WireStoreListing>,
    #[serde(default)]
    subscription_plan_id: Option<String>,
    #[serde(default)]
    subscription_plan: Option<WireSubscriptionPlan>,
    #[serde(default)]
    user: Option<WireUserRef>,
    #[serde(default)]
    gift_style: Option<u8>,
    #[serde(default)]
    flags: u32,
    #[serde(default)]
    batch_id: Option<String>,
    #[serde(default)]
    entitlement_branches: Option<Vec<String>>,
}

impl WireGiftCode {
    fn into_record(self) -> Result<GiftCodeRecord, GiftError> {
        if self.code.trim().is_empty() {
            return Err(GiftError::InvalidResponse);
        }
        let store_listing_id = self.store_listing.as_ref().map(|sl| sl.id.clone());
        let listing_sku_id = self
            .store_listing
            .as_ref()
            .and_then(|sl| sl.sku.as_ref())
            .map(|s| s.id.clone());
        let listing_app_id = self
            .store_listing
            .and_then(|sl| sl.sku)
            .and_then(|s| s.application_id);
        let sku_id = self
            .sku_id
            .or(listing_sku_id)
            .filter(|s| !s.trim().is_empty())
            .ok_or(GiftError::InvalidResponse)?;
        let application_id = self.application_id.or(listing_app_id);
        let subscription_plan_id = self
            .subscription_plan_id
            .or_else(|| self.subscription_plan.map(|p| p.id));
        let user_id = self.user.map(|u| u.id);

        Ok(GiftCodeRecord {
            code: self.code,
            sku_id,
            application_id,
            uses: self.uses,
            max_uses: self.max_uses,
            expires_at: self.expires_at,
            redeemed: self.redeemed,
            store_listing_id,
            subscription_plan_id,
            user_id,
            gift_style: self.gift_style,
            flags: self.flags,
            batch_id: self.batch_id,
            entitlement_branches: self.entitlement_branches,
        })
    }
}

fn decode_gift_code_record(body: &[u8]) -> Result<GiftCodeRecord, GiftError> {
    let raw: WireGiftCode = serde_json::from_slice(body).map_err(|_| GiftError::InvalidResponse)?;
    raw.into_record()
}

fn decode_entitlement_record(body: &[u8]) -> Result<EntitlementRecord, GiftError> {
    let record: EntitlementRecord =
        serde_json::from_slice(body).map_err(|_| GiftError::InvalidResponse)?;
    if record.id.trim().is_empty() || record.sku_id.trim().is_empty() {
        return Err(GiftError::InvalidResponse);
    }
    Ok(record)
}

fn decode_entitlement_list(body: &[u8]) -> Result<Vec<EntitlementRecord>, GiftError> {
    let list: Vec<EntitlementRecord> =
        serde_json::from_slice(body).map_err(|_| GiftError::InvalidResponse)?;
    if list
        .iter()
        .any(|r| r.id.trim().is_empty() || r.sku_id.trim().is_empty())
    {
        return Err(GiftError::InvalidResponse);
    }
    Ok(list)
}

#[derive(Deserialize)]
struct DiscordErrorEnvelope {
    #[serde(default)]
    code: Option<u32>,
    #[serde(default)]
    retry_after: Option<f64>,
}

fn ensure_2xx(response: &HttpResponse) -> Result<(), GiftError> {
    if (200..300).contains(&response.status) {
        return Ok(());
    }
    if response.status == 429 {
        return Err(GiftError::RateLimited {
            retry_after_ms: extract_retry_after_ms(response),
        });
    }
    if let Ok(envelope) = serde_json::from_slice::<DiscordErrorEnvelope>(&response.body) {
        match envelope.code {
            Some(DISCORD_ERR_UNKNOWN_GIFT_CODE) => return Err(GiftError::NotFound),
            Some(DISCORD_ERR_INVALID_FORM_BODY | DISCORD_ERR_INVALID_COUNTRY_CODE) => {
                return Err(GiftError::InvalidFormBody);
            }
            Some(DISCORD_ERR_INVALID_GIFT_REDEMPTION_EXHAUSTED) => {
                return Err(GiftError::GiftRedemptionExhausted);
            }
            Some(
                DISCORD_ERR_INVALID_GIFT_REDEMPTION_OWNED
                | DISCORD_ERR_INVALID_GIFT_REDEMPTION_PREVIOUSLY_OWNED,
            ) => {
                return Err(GiftError::GiftRedemptionOwned);
            }
            Some(DISCORD_ERR_INVALID_GIFT_SELF_REDEMPTION) => {
                return Err(GiftError::GiftSelfRedemption);
            }
            Some(DISCORD_ERR_INVALID_GIFT_REDEMPTION_FRAUD_REJECTED) => {
                return Err(GiftError::GiftRedemptionFraudRejected);
            }
            _ => {}
        }
    }
    Err(RestFailure::from_status(response.status)
        .map(GiftError::from)
        .unwrap_or(GiftError::Network))
}

fn extract_retry_after_ms(response: &HttpResponse) -> u64 {
    if let Ok(envelope) = serde_json::from_slice::<DiscordErrorEnvelope>(&response.body) {
        if let Some(secs) = envelope.retry_after.filter(|v| v.is_finite() && *v >= 0.0) {
            return (secs * 1000.0).round() as u64;
        }
    }
    for (name, value) in &response.headers {
        if name.eq_ignore_ascii_case("retry-after") {
            if let Ok(secs) = value.trim().parse::<f64>() {
                if secs.is_finite() && secs >= 0.0 {
                    return (secs * 1000.0).round() as u64;
                }
            }
        }
    }
    0
}
