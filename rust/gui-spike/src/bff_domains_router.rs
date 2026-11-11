//! RW-40 / task-269 (`BE-INT-12`), task-301 (`BE-L2-7DOMAINS`): единый типизированный роутер
//! команд BFF (`bff_domains_router`).
//!
//! Объединяет маршрутизацию и валидацию семи доменов BFF:
//! - `slash_commands`: вызов slash-команды приложения (`POST /api/v9/interactions`);
//! - `onboarding`: обновление экрана приветствия / онбординга гильдии (`PATCH /api/v9/guilds/{guild_id}/welcome-screen`);
//! - `soundboard`: отправка звука саундборда в голосовой канал (`POST /api/v9/channels/{channel_id}/send-soundboard-sound`);
//! - `stage_instances`: создание экземпляра трибуны (`POST /api/v9/stage-instances`);
//! - `automod`: создание/обновление правила автомодерации гильдии
//!   (`POST /api/v9/guilds/{guild_id}/auto-moderation/rules`,
//!   `PATCH /api/v9/guilds/{guild_id}/auto-moderation/rules/{rule_id}`);
//! - `expressions`: создание кастомного эмодзи гильдии (`POST /api/v9/guilds/{guild_id}/emojis`);
//! - `webhooks`: создание вебхука канала (`POST /api/v9/channels/{channel_id}/webhooks`) и
//!   подписка на новостной канал (`POST /api/v9/channels/{news_channel_id}/followers`).
//!
//! Pass 2 (`REVIEW-182A`, P3-1 и P3-2): [`resolve_bff_domain_action`] возвращает HTTP-метод
//! вместе с маршрутом и телом ([`BffResolvedAction`]), а `bff_domains_router_contract` закрепляет
//! wire-формат [`BffDomainRequest`] буквальным JSON.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use crate::automod::UpsertAutoModRuleRequest;
pub use crate::expressions::CreateGuildEmojiRequest;
pub use crate::slash_commands::{
    ClickMessageComponentRequest, InteractionType, InvokeSlashCommandRequest,
};
pub use crate::soundboard::SendSoundboardSoundRequest;
pub use crate::stage_instances::CreateStageInstanceRequest;
pub use crate::webhooks::CreateWebhookRequest;

/// Алиас запроса обновления экрана приветствия / онбординга гильдии для единого роутера BFF.
pub type UpdateGuildOnboardingRequest = crate::onboarding::UpdateWelcomeScreenRequest;

/// Канонический список поддерживаемых доменов единого роутера BFF.
pub const SUPPORTED_BFF_DOMAINS: &[&str] = &[
    "slash_commands",
    "onboarding",
    "soundboard",
    "stage_instances",
    "automod",
    "expressions",
    "webhooks",
];

/// Единое типизированное действие домена BFF.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum BffDomainAction {
    /// Вызов slash-команды приложения (`POST /api/v9/interactions`, `type = 2`).
    InvokeSlashCommand(InvokeSlashCommandRequest),
    /// Клик по интерактивной кнопке сообщения (`POST /api/v9/interactions`, `type = 3`).
    #[serde(alias = "MessageComponent", alias = "InvokeMessageComponent")]
    ClickMessageComponent(ClickMessageComponentRequest),
    /// Обновление онбординга / экрана приветствия гильдии (`PATCH /api/v9/guilds/{guild_id}/welcome-screen`).
    UpdateGuildOnboarding {
        /// Snowflake ID гильдии.
        guild_id: String,
        /// Полезная нагрузка обновления экрана приветствия гильдии.
        request: UpdateGuildOnboardingRequest,
    },
    /// Отправка звука саундборда в голосовой канал (`POST /api/v9/channels/{channel_id}/send-soundboard-sound`).
    SendSoundboardSound {
        /// Snowflake ID голосового канала.
        channel_id: String,
        /// Полезная нагрузка отправки звука саундборда.
        request: SendSoundboardSoundRequest,
    },
    /// Создание экземпляра трибуны (`POST /api/v9/stage-instances`).
    CreateStageInstance(CreateStageInstanceRequest),
    /// Создание (`rule_id: None`, `POST`) или обновление (`rule_id: Some(_)`, `PATCH`)
    /// правила автомодерации гильдии.
    UpsertAutoModRule {
        /// Snowflake ID гильдии.
        guild_id: String,
        /// Snowflake ID существующего правила; `None` — создание нового.
        rule_id: Option<String>,
        /// Полезная нагрузка правила автомодерации.
        request: UpsertAutoModRuleRequest,
    },
    /// Создание кастомного эмодзи гильдии (`POST /api/v9/guilds/{guild_id}/emojis`).
    CreateGuildEmoji {
        /// Snowflake ID гильдии.
        guild_id: String,
        /// Размер загружаемого изображения в байтах (проверяется доменом `expressions`).
        byte_len: u64,
        /// Полезная нагрузка создания эмодзи.
        request: CreateGuildEmojiRequest,
    },
    /// Создание вебхука канала (`POST /api/v9/channels/{channel_id}/webhooks`).
    CreateWebhook {
        /// Snowflake ID канала.
        channel_id: String,
        /// Полезная нагрузка создания вебхука.
        request: CreateWebhookRequest,
    },
    /// Подписка на новостной канал (`POST /api/v9/channels/{news_channel_id}/followers`).
    FollowNewsChannel {
        /// Snowflake ID новостного канала, на который подписываются.
        news_channel_id: String,
        /// Snowflake ID канала, куда будут приходить публикации.
        target_channel_id: String,
    },
}

/// Алиас типа действия домена BFF для совместимости.
pub type BffDomainRequest = BffDomainAction;

/// Проверяет, входит ли имя домена `domain` в [`SUPPORTED_BFF_DOMAINS`].
#[must_use]
pub fn is_supported_bff_domain(domain: &str) -> bool {
    SUPPORTED_BFF_DOMAINS.contains(&domain)
}

/// Валидирует действие [`BffDomainAction`] и строит пару `(route, body)` для Discord REST API v9,
/// делегируя профильным функциям модулей `slash_commands`, `onboarding`, `soundboard`,
/// `stage_instances`, `automod`, `expressions` и `webhooks`.
pub fn dispatch_bff_domain_action(action: &BffDomainAction) -> Result<(String, Value), String> {
    match action {
        BffDomainAction::InvokeSlashCommand(req) => {
            crate::slash_commands::build_interactions_route_and_body(req)
        }
        BffDomainAction::ClickMessageComponent(req) => {
            crate::slash_commands::build_message_component_interactions_route_and_body(req)
        }
        BffDomainAction::UpdateGuildOnboarding { guild_id, request } => {
            crate::onboarding::validate_welcome_screen_request(request)?;
            let route = crate::onboarding::build_guild_welcome_screen_route(guild_id)?;
            let body = serde_json::to_value(request).map_err(|err| {
                format!("ошибка сериализации UpdateGuildOnboardingRequest: {err}")
            })?;
            Ok((route, body))
        }
        BffDomainAction::SendSoundboardSound {
            channel_id,
            request,
        } => crate::soundboard::build_send_soundboard_route_and_body(channel_id, request),
        BffDomainAction::CreateStageInstance(req) => {
            crate::stage_instances::build_create_stage_instance_route_and_body(req)
        }
        BffDomainAction::UpsertAutoModRule {
            guild_id,
            rule_id,
            request,
        } => {
            crate::automod::validate_automod_rule_request(request)?;
            let route = crate::automod::build_guild_automod_route(guild_id, rule_id.as_deref())?;
            let body = serde_json::to_value(request)
                .map_err(|err| format!("ошибка сериализации UpsertAutoModRuleRequest: {err}"))?;
            Ok((route, body))
        }
        BffDomainAction::CreateGuildEmoji {
            guild_id,
            byte_len,
            request,
        } => {
            crate::expressions::validate_create_guild_emoji(request, *byte_len)?;
            let route = crate::expressions::build_guild_emojis_route(guild_id, None)?;
            let body = serde_json::to_value(request)
                .map_err(|err| format!("ошибка сериализации CreateGuildEmojiRequest: {err}"))?;
            Ok((route, body))
        }
        BffDomainAction::CreateWebhook {
            channel_id,
            request,
        } => crate::webhooks::build_create_webhook_route_and_body(channel_id, request),
        BffDomainAction::FollowNewsChannel {
            news_channel_id,
            target_channel_id,
        } => {
            crate::webhooks::build_follow_channel_route_and_body(news_channel_id, target_channel_id)
        }
    }
}

/// Готовый к отправке запрос BFF: HTTP-метод, маршрут и тело.
///
/// Строится только через [`resolve_bff_domain_action`], поэтому метод всегда согласован
/// с маршрутом. Сериализуется для IPC/логов; десериализация не поддерживается —
/// `method` хранится как `&'static str`.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct BffResolvedAction {
    /// HTTP-метод Discord REST API v9 (`"POST"` или `"PATCH"`).
    pub method: &'static str,
    /// Маршрут Discord REST API v9 без базового хоста.
    pub route: String,
    /// JSON-тело запроса, прошедшее валидацию домена.
    pub body: Value,
}

/// Валидирует действие [`BffDomainRequest`] и возвращает [`BffResolvedAction`]:
/// HTTP-метод, маршрут и тело. Маршрут и тело делегируются
/// [`dispatch_bff_domain_action`], метод берётся из домена:
/// `POST` для `InvokeSlashCommand`/`SendSoundboardSound`/`CreateStageInstance`/`CreateGuildEmoji`/
/// `CreateWebhook`/`FollowNewsChannel` и `UpsertAutoModRule` с `rule_id: None`,
/// `PATCH` для `UpdateGuildOnboarding` и `UpsertAutoModRule` с `rule_id: Some(_)`.
pub fn resolve_bff_domain_action(req: BffDomainRequest) -> Result<BffResolvedAction, String> {
    let method = match &req {
        BffDomainAction::UpdateGuildOnboarding { .. } => "PATCH",
        BffDomainAction::UpsertAutoModRule {
            rule_id: Some(_), ..
        } => "PATCH",
        BffDomainAction::InvokeSlashCommand(_)
        | BffDomainAction::ClickMessageComponent(_)
        | BffDomainAction::SendSoundboardSound { .. }
        | BffDomainAction::CreateStageInstance(_)
        | BffDomainAction::UpsertAutoModRule { rule_id: None, .. }
        | BffDomainAction::CreateGuildEmoji { .. }
        | BffDomainAction::CreateWebhook { .. }
        | BffDomainAction::FollowNewsChannel { .. } => "POST",
    };
    let (route, body) = dispatch_bff_domain_action(&req)?;
    Ok(BffResolvedAction {
        method,
        route,
        body,
    })
}
