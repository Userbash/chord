//! RW-40 / N-05-NOTIFICATIONS-BACKEND: настройки уведомлений гильдий и переопределения каналов.
//!
//! Контракт (`waves/RW-40/NOTIFICATIONS-RE.md`, `waves/RW-40/DESIGN/NOTIFICATIONS-SPEC.md`):
//! - Маршрут одиночной гильдии / DM: `PATCH /users/@me/guilds/{guild_id}/settings`
//!   (`Endpoints.USER_GUILD_SETTINGS`), а также bulk-конверт `PATCH /users/@me/guilds/settings`
//!   (`Endpoints.USER_GUILD_SETTINGS_BULK`, тело `{"guilds": {"<guild_id>": { ... }}}`).
//! - Поля настроек гильдии:
//!   - `message_notifications`: `0` (`ALL_MESSAGES`), `1` (`ONLY_MENTIONS`), `2` (`NO_MESSAGES`);
//!     значение `3` (`NULL`) для гильдии запрещено (`[D] NOTIFICATIONS-RE.md §3.1`);
//!   - `muted`: `bool`;
//!   - `mute_config`: `{ selected_time_window, end_time }` с 6 каноническими окнами
//!     `900 | 3600 | 10800 | 28800 | 86400 | -1` и `end_time` (ISO-8601 или `null` при `-1`);
//!   - `suppress_everyone`, `suppress_roles`, `mobile_push`, `mute_scheduled_events`: `bool`;
//!   - `flags`: битовая маска гильдии (`UNREADS_ALL_MESSAGES = 2048`,
//!     `UNREADS_ONLY_MENTIONS = 4096`, `OPT_IN_CHANNELS_OFF = 8192`, `OPT_IN_CHANNELS_ON = 16384`);
//!   - `notify_highlights`: `0` (`NULL`), `1` (`DISABLED`), `2` (`ENABLED`);
//!   - `channel_overrides`: на запись всегда JSON-словарь `{"<channel_id>": { ... }}` с полями
//!     `message_notifications` (`0 | 1 | 2 | 3`), `muted`, `mute_config`, `flags`, `collapsed`.
//! - Каскадный резолвер [`resolve_effective_notifications`]:
//!   `channel (!= 3) -> parent_category (!= 3) -> guild -> default_message_notifications`,
//!   расчёт эффективного `is_muted` с учётом истечения `mute_config.end_time` относительно
//!   `now_unix`, `allow_no_messages = is_muted || effective_level == 2` (плюс opt-in подавление),
//!   и методы `should_alert_mention` / `should_increment_badge` для `@user`, `@role`, `@everyone`/`@here`.
//! - Транспорт [`NotificationTransport`] и функции [`update_guild_notification_settings`] /
//!   [`update_channel_notification_override`], отвергающие пустой `token`/`guild_id`/`channel_id`
//!   и невалидные уровни `message_notifications` или `mute_config` до обращения к сети.

use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chord_core::rest::client::{ApiAuth, ApiClient, ApiConfig, ApiRetryPolicy};
use chord_core::rest::transport::{HttpClientConfig, HttpResponse};
use chord_core::API_V9_BASE;
use serde::{Deserialize, Serialize};

use crate::rest_failure::RestFailure;

/// Максимальная длина идентификаторов `guild_id` и `channel_id` в байтах.
pub const MAX_ID_BYTES: usize = 64;

/// `ALL_MESSAGES = 0`: уведомлять о каждом сообщении (`[D] NOTIFICATIONS-RE.md §3.1`).
pub const MESSAGE_NOTIFICATIONS_ALL_MESSAGES: i64 = 0;
/// `ONLY_MENTIONS = 1`: уведомлять только при упоминаниях (`[D] NOTIFICATIONS-RE.md §3.1`).
pub const MESSAGE_NOTIFICATIONS_ONLY_MENTIONS: i64 = 1;
/// `NO_MESSAGES = 2`: не создавать звуковых/desktop-уведомлений (`[D] NOTIFICATIONS-RE.md §3.1`).
pub const MESSAGE_NOTIFICATIONS_NO_MESSAGES: i64 = 2;
/// `NULL = 3`: наследовать уровень категории/сервера (валиден только для канала/категории).
pub const MESSAGE_NOTIFICATIONS_NULL: i64 = 3;

/// Допустимые коды `message_notifications` на уровне гильдии (`0 | 1 | 2`).
pub const VALID_GUILD_MESSAGE_NOTIFICATIONS: [i64; 3] = [
    MESSAGE_NOTIFICATIONS_ALL_MESSAGES,
    MESSAGE_NOTIFICATIONS_ONLY_MENTIONS,
    MESSAGE_NOTIFICATIONS_NO_MESSAGES,
];

/// Допустимые коды `message_notifications` на уровне канала/категории (`0 | 1 | 2 | 3`).
pub const VALID_CHANNEL_MESSAGE_NOTIFICATIONS: [i64; 4] = [
    MESSAGE_NOTIFICATIONS_ALL_MESSAGES,
    MESSAGE_NOTIFICATIONS_ONLY_MENTIONS,
    MESSAGE_NOTIFICATIONS_NO_MESSAGES,
    MESSAGE_NOTIFICATIONS_NULL,
];

/// 6 канонических пресетов `mute_config.selected_time_window` в секундах (`@8349873` `Xx`, `@6225743` `b8()`).
pub const VALID_MUTE_TIME_WINDOWS: [i64; 6] = [900, 3600, 10800, 28800, 86400, -1];

/// Допустимые коды `notify_highlights` (`0 = NULL`, `1 = DISABLED`, `2 = ENABLED`, `@529852`).
pub const VALID_NOTIFY_HIGHLIGHTS: [i64; 3] = [0, 1, 2];

/// `notify_highlights = 0` (`NULL` — по умолчанию).
pub const NOTIFY_HIGHLIGHTS_NULL: i64 = 0;
/// `notify_highlights = 1` (`DISABLED` — выключено).
pub const NOTIFY_HIGHLIGHTS_DISABLED: i64 = 1;
/// `notify_highlights = 2` (`ENABLED` — включено).
pub const NOTIFY_HIGHLIGHTS_ENABLED: i64 = 2;

/// Бит флагов гильдии: считать непрочитанными все сообщения (`2048`).
pub const GUILD_FLAG_UNREADS_ALL_MESSAGES: i64 = 2048;
/// Бит флагов гильдии: считать непрочитанными только упоминания (`4096`).
pub const GUILD_FLAG_UNREADS_ONLY_MENTIONS: i64 = 4096;
/// Бит флагов гильдии: режим «каналы по подписке» выключен (`8192`).
pub const GUILD_FLAG_OPT_IN_CHANNELS_OFF: i64 = 8192;
/// Бит флагов гильдии: режим «каналы по подписке» включён (`16384`).
pub const GUILD_FLAG_OPT_IN_CHANNELS_ON: i64 = 16384;
/// Маска пары битов `UNREADS_*` гильдии (`2048 | 4096 = 6144`).
pub const GUILD_UNREAD_MASK: i64 =
    GUILD_FLAG_UNREADS_ALL_MESSAGES | GUILD_FLAG_UNREADS_ONLY_MENTIONS;

/// Бит флагов канала: непрочитанное только от упоминаний (`512`).
pub const CHANNEL_FLAG_UNREADS_ONLY_MENTIONS: i64 = 512;
/// Бит флагов канала: непрочитанное от всех сообщений (`1024`).
pub const CHANNEL_FLAG_UNREADS_ALL_MESSAGES: i64 = 1024;
/// Бит флагов канала: избранный канал (`2048`).
pub const CHANNEL_FLAG_FAVORITED: i64 = 2048;
/// Бит флагов канала: канал выбран в opt-in режиме (`4096`).
pub const CHANNEL_FLAG_OPT_IN_ENABLED: i64 = 4096;
/// Бит флагов канала: уведомления о новых ветках форума выключены (`8192`).
pub const CHANNEL_FLAG_NEW_FORUM_THREADS_OFF: i64 = 8192;
/// Бит флагов канала: уведомления о новых ветках форума включены (`16384`).
pub const CHANNEL_FLAG_NEW_FORUM_THREADS_ON: i64 = 16384;
/// Маска пары битов `UNREADS_*` канала (`512 | 1024 = 1536`).
pub const CHANNEL_UNREAD_MASK: i64 =
    CHANNEL_FLAG_UNREADS_ONLY_MENTIONS | CHANNEL_FLAG_UNREADS_ALL_MESSAGES;

/// Маршрут bulk-обновления настроек гильдий (`Endpoints.USER_GUILD_SETTINGS_BULK`).
pub const USER_GUILD_SETTINGS_BULK_PATH: &str = "/users/@me/guilds/settings";

/// Перечисление уровней `message_notifications` (`orn`, модуль `652215`, `@313815`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageNotificationLevel {
    /// `0` — все сообщения.
    AllMessages = 0,
    /// `1` — только `@упоминания`.
    OnlyMentions = 1,
    /// `2` — ничего.
    NoMessages = 2,
    /// `3` — наследовать от категории/гильдии (допустимо только для канала/категории).
    Inherit = 3,
}

impl MessageNotificationLevel {
    /// Числовой wire-код уровня (`0..=3`).
    #[must_use]
    pub const fn as_i64(self) -> i64 {
        self as i64
    }

    /// Проверяет и преобразует числовой код уровня для гильдии (`0 | 1 | 2`).
    pub fn from_guild_code(value: i64) -> Result<Self, NotificationError> {
        match value {
            MESSAGE_NOTIFICATIONS_ALL_MESSAGES => Ok(Self::AllMessages),
            MESSAGE_NOTIFICATIONS_ONLY_MENTIONS => Ok(Self::OnlyMentions),
            MESSAGE_NOTIFICATIONS_NO_MESSAGES => Ok(Self::NoMessages),
            other => Err(NotificationError::InvalidMessageNotifications { value: other }),
        }
    }

    /// Проверяет и преобразует числовой код уровня для канала или категории (`0 | 1 | 2 | 3`).
    pub fn from_channel_code(value: i64) -> Result<Self, NotificationError> {
        match value {
            MESSAGE_NOTIFICATIONS_ALL_MESSAGES => Ok(Self::AllMessages),
            MESSAGE_NOTIFICATIONS_ONLY_MENTIONS => Ok(Self::OnlyMentions),
            MESSAGE_NOTIFICATIONS_NO_MESSAGES => Ok(Self::NoMessages),
            MESSAGE_NOTIFICATIONS_NULL => Ok(Self::Inherit),
            other => Err(NotificationError::InvalidMessageNotifications { value: other }),
        }
    }
}

/// Перечисление `notify_highlights` (`guM`, модуль `652215`, `@529852`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotifyHighlights {
    /// `0` — `NULL` (по умолчанию).
    Null = 0,
    /// `1` — `DISABLED` (выключено).
    Disabled = 1,
    /// `2` — `ENABLED` (включено).
    Enabled = 2,
}

impl NotifyHighlights {
    /// Числовой wire-код (`0..=2`).
    #[must_use]
    pub const fn as_i64(self) -> i64 {
        self as i64
    }

    /// Проверяет и преобразует числовой код `notify_highlights` (`0 | 1 | 2`).
    pub fn from_code(value: i64) -> Result<Self, NotificationError> {
        match value {
            NOTIFY_HIGHLIGHTS_NULL => Ok(Self::Null),
            NOTIFY_HIGHLIGHTS_DISABLED => Ok(Self::Disabled),
            NOTIFY_HIGHLIGHTS_ENABLED => Ok(Self::Enabled),
            other => Err(NotificationError::InvalidNotifyHighlights { value: other }),
        }
    }
}

/// Тип упоминания в сообщении для матрицы срабатывания уведомлений и бейджей (`NOTIFICATIONS-SPEC.md §1.5 п.3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MentionKind {
    /// Обычное сообщение без упоминаний.
    None,
    /// Прямое упоминание текущего пользователя (`@user`).
    User,
    /// Упоминание роли пользователя (`<@&role_id>`).
    Role,
    /// Массовое упоминание `@everyone`.
    Everyone,
    /// Массовое упоминание `@here`.
    Here,
}

/// Конфигурация таймера приглушения (`mute_config`, `[D] NOTIFICATIONS-RE.md §4`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MuteConfig {
    /// Выбранное окно в секундах (`900 | 3600 | 10800 | 28800 | 86400 | -1`).
    pub selected_time_window: i64,
    /// Момент окончания mute в формате ISO-8601 или `None` (`null` при `-1`).
    pub end_time: Option<String>,
}

impl MuteConfig {
    /// Строит `MuteConfig` из одного из 6 канонических окон длительности и текущего времени `now_unix`.
    ///
    /// Для `-1` («Пока не включу») `end_time = None`; для положительного окна
    /// `end_time = Some(format_iso8601_utc(now_unix + selected_time_window))`.
    pub fn from_window(
        selected_time_window: i64,
        now_unix: i64,
    ) -> Result<Self, NotificationError> {
        if !VALID_MUTE_TIME_WINDOWS.contains(&selected_time_window) {
            return Err(NotificationError::InvalidMuteConfig);
        }
        let end_time = if selected_time_window == -1 {
            None
        } else {
            let end_unix = now_unix
                .checked_add(selected_time_window)
                .ok_or(NotificationError::InvalidMuteConfig)?;
            Some(format_iso8601_utc(end_unix))
        };
        Ok(Self {
            selected_time_window,
            end_time,
        })
    }

    /// Проверяет корректность `selected_time_window` и согласованность `end_time`.
    pub fn validate(&self) -> Result<(), NotificationError> {
        if !VALID_MUTE_TIME_WINDOWS.contains(&self.selected_time_window) {
            return Err(NotificationError::InvalidMuteConfig);
        }
        if self.selected_time_window == -1 {
            if self.end_time.as_deref().is_some_and(|end| end != "nil") {
                return Err(NotificationError::InvalidMuteConfig);
            }
            return Ok(());
        }
        let Some(end_iso) = self.end_time.as_deref() else {
            return Err(NotificationError::InvalidMuteConfig);
        };
        if parse_iso8601_unix(end_iso).is_none() {
            return Err(NotificationError::InvalidMuteConfig);
        }
        Ok(())
    }
}

/// Состояние переопределения канала или родительской категории (`channel_overrides[]`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelNotificationOverride {
    pub channel_id: String,
    pub message_notifications: i64,
    pub muted: bool,
    pub mute_config: Option<MuteConfig>,
    pub flags: Option<i64>,
    pub collapsed: bool,
}

impl ChannelNotificationOverride {
    /// Признак того, что запись несёт реальное переопределение уведомлений, а не только
    /// `collapsed` или `OPT_IN_ENABLED` (`NOTIFICATIONS-SPEC.md §1.4 п.2`).
    #[must_use]
    pub fn has_notification_override(&self, now_unix: i64) -> bool {
        is_entry_muted(self.muted, self.mute_config.as_ref(), now_unix)
            || (VALID_GUILD_MESSAGE_NOTIFICATIONS.contains(&self.message_notifications))
            || (self.flags.unwrap_or(0) & CHANNEL_UNREAD_MASK) != 0
    }
}

/// Входные настройки гильдии для каскадного резолвера [`resolve_effective_notifications`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuildNotificationPolicyInput {
    pub guild_id: Option<String>,
    pub message_notifications: i64,
    pub default_message_notifications: i64,
    pub muted: bool,
    pub mute_config: Option<MuteConfig>,
    pub suppress_everyone: bool,
    pub suppress_roles: bool,
    pub mobile_push: bool,
    pub mute_scheduled_events: bool,
    pub notify_highlights: i64,
    pub hide_muted_channels: bool,
    pub flags: i64,
}

impl Default for GuildNotificationPolicyInput {
    fn default() -> Self {
        Self {
            guild_id: None,
            message_notifications: MESSAGE_NOTIFICATIONS_ONLY_MENTIONS,
            default_message_notifications: MESSAGE_NOTIFICATIONS_ONLY_MENTIONS,
            muted: false,
            mute_config: None,
            suppress_everyone: false,
            suppress_roles: false,
            mobile_push: true,
            mute_scheduled_events: false,
            notify_highlights: NOTIFY_HIGHLIGHTS_NULL,
            hide_muted_channels: false,
            flags: 0,
        }
    }
}

/// Результат каскадного разрешения политики уведомлений канала (`NOTIFICATIONS-SPEC.md §1.5`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectiveNotificationPolicy {
    /// Заглушён ли канал на любом уровне (`guild_muted || category_muted || channel_muted`).
    pub is_muted: bool,
    /// Заглушена ли сама гильдия на момент `now_unix`.
    pub guild_muted: bool,
    /// Заглушена ли родительская категория на момент `now_unix`.
    pub category_muted: bool,
    /// Заглушён ли непосредственно канал на момент `now_unix`.
    pub channel_muted: bool,
    /// Разрешённый уровень уведомлений (`0 = ALL_MESSAGES`, `1 = ONLY_MENTIONS`, `2 = NO_MESSAGES`).
    pub effective_level: i64,
    /// Полное подавление звуковых/desktop-уведомлений (`is_muted || effective_level == 2` или opt-in подавление).
    pub allow_no_messages: bool,
    /// Подавлять ли `@everyone` и `@here`.
    pub suppress_everyone: bool,
    /// Подавлять ли `@role`.
    pub suppress_roles: bool,
}

impl EffectiveNotificationPolicy {
    /// Определяет, должно ли сообщение с данным типом упоминания вызывать звуковое / desktop-уведомление.
    #[must_use]
    pub fn should_alert_mention(&self, kind: MentionKind) -> bool {
        match kind {
            MentionKind::User => !self.allow_no_messages,
            MentionKind::Role => !self.suppress_roles && !self.allow_no_messages,
            MentionKind::Everyone | MentionKind::Here => {
                !self.suppress_everyone && !self.allow_no_messages
            }
            MentionKind::None => self.should_alert_regular_message(),
        }
    }

    /// Определяет, должно ли сообщение с данным типом упоминания увеличивать счётчик бейджа `.mention-badge`.
    ///
    /// Прямое упоминание `@user` увеличивает бейдж всегда (даже при `is_muted` и `NO_MESSAGES`);
    /// `@role` и `@everyone`/`@here` увеличивают бейдж, если не подавлены соответствующим флагом `suppress_*`.
    #[must_use]
    pub fn should_increment_badge(&self, kind: MentionKind) -> bool {
        match kind {
            MentionKind::User => true,
            MentionKind::Role => !self.suppress_roles,
            MentionKind::Everyone | MentionKind::Here => !self.suppress_everyone,
            MentionKind::None => false,
        }
    }

    /// Определяет, вызывает ли обычное сообщение без упоминаний звуковое / desktop-уведомление.
    #[must_use]
    pub fn should_alert_regular_message(&self) -> bool {
        !self.allow_no_messages && self.effective_level == MESSAGE_NOTIFICATIONS_ALL_MESSAGES
    }
}

/// Частичный PATCH-пейлоад переопределения канала (`channel_overrides["<channel_id>"]`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelNotificationOverridePatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_notifications: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub muted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mute_config: Option<Option<MuteConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collapsed: Option<bool>,
}

impl ChannelNotificationOverridePatch {
    /// Создаёт пустой построитель PATCH-переопределения канала.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Строит PATCH сброса переопределения канала (`@1622`, `NOTIFICATIONS-SPEC.md §1.4 п.3`):
    /// `{ message_notifications: 3, muted: false, flags: PW(current_flags) }`.
    #[must_use]
    pub fn reset_override(current_flags: Option<i64>) -> Self {
        Self {
            message_notifications: Some(MESSAGE_NOTIFICATIONS_NULL),
            muted: Some(false),
            mute_config: None,
            flags: current_flags.map(clear_channel_unread_flags),
            collapsed: None,
        }
    }

    /// Задаёт уровень уведомлений канала (`0 | 1 | 2 | 3`).
    #[must_use]
    pub fn with_message_notifications(mut self, level: i64) -> Self {
        self.message_notifications = Some(level);
        self
    }

    /// Задаёт флаг приглушения канала (`muted`).
    #[must_use]
    pub fn with_muted(mut self, muted: bool) -> Self {
        self.muted = Some(muted);
        self
    }

    /// Задаёт `mute_config` (или `None` для сериализации `"mute_config": null`).
    #[must_use]
    pub fn with_mute_config(mut self, mute_config: Option<MuteConfig>) -> Self {
        self.mute_config = Some(mute_config);
        self
    }

    /// Выбирает одно из 6 канонических окон длительности mute канала.
    pub fn with_mute_window(
        mut self,
        window: i64,
        now_unix: i64,
    ) -> Result<Self, NotificationError> {
        let cfg = MuteConfig::from_window(window, now_unix)?;
        self.muted = Some(true);
        self.mute_config = Some(Some(cfg));
        Ok(self)
    }

    /// Задаёт битовую маску `flags` канала.
    #[must_use]
    pub fn with_flags(mut self, flags: i64) -> Self {
        self.flags = Some(flags);
        self
    }

    /// Задаёт состояние свёрнутости категории `collapsed`.
    #[must_use]
    pub fn with_collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = Some(collapsed);
        self
    }

    /// Проверяет, что хотя бы одно поле задано и все значения соответствуют контракту RE.
    pub fn validate(&self) -> Result<(), NotificationError> {
        if self.message_notifications.is_none()
            && self.muted.is_none()
            && self.mute_config.is_none()
            && self.flags.is_none()
            && self.collapsed.is_none()
        {
            return Err(NotificationError::EmptyPatch);
        }
        if let Some(level) = self.message_notifications {
            MessageNotificationLevel::from_channel_code(level)?;
        }
        if let Some(Some(cfg)) = &self.mute_config {
            cfg.validate()?;
        }
        Ok(())
    }

    /// Валидирует и сериализует переопределение канала в JSON-объект.
    pub fn to_patch_value(&self) -> Result<serde_json::Value, NotificationError> {
        self.validate()?;
        let mut map = serde_json::Map::new();
        if let Some(level) = self.message_notifications {
            map.insert(
                "message_notifications".to_string(),
                serde_json::Value::from(level),
            );
        }
        if let Some(muted) = self.muted {
            map.insert("muted".to_string(), serde_json::Value::from(muted));
        }
        if let Some(mute_cfg) = &self.mute_config {
            match mute_cfg {
                Some(cfg) => {
                    map.insert(
                        "mute_config".to_string(),
                        serde_json::json!({
                            "selected_time_window": cfg.selected_time_window,
                            "end_time": cfg.end_time,
                        }),
                    );
                }
                None => {
                    map.insert("mute_config".to_string(), serde_json::Value::Null);
                }
            }
        }
        if let Some(flags) = self.flags {
            map.insert("flags".to_string(), serde_json::Value::from(flags));
        }
        if let Some(collapsed) = self.collapsed {
            map.insert("collapsed".to_string(), serde_json::Value::from(collapsed));
        }
        Ok(serde_json::Value::Object(map))
    }
}

/// Частичный PATCH-пейлоад настроек уведомлений гильдии (`PATCH /users/@me/guilds/{guild_id}/settings`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuildNotificationSettingsPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_notifications: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub muted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mute_config: Option<Option<MuteConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suppress_everyone: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suppress_roles: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mobile_push: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mute_scheduled_events: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notify_highlights: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_overrides: Option<BTreeMap<String, ChannelNotificationOverridePatch>>,
}

impl GuildNotificationSettingsPatch {
    /// Создаёт пустой построитель PATCH-пейлоада гильдии.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Задаёт базовый уровень уведомлений гильдии (`0 | 1 | 2`).
    #[must_use]
    pub fn with_message_notifications(mut self, level: i64) -> Self {
        self.message_notifications = Some(level);
        self
    }

    /// Задаёт флаг приглушения гильдии (`muted`).
    #[must_use]
    pub fn with_muted(mut self, muted: bool) -> Self {
        self.muted = Some(muted);
        self
    }

    /// Задаёт `mute_config` (или `None` для сериализации `"mute_config": null`).
    #[must_use]
    pub fn with_mute_config(mut self, mute_config: Option<MuteConfig>) -> Self {
        self.mute_config = Some(mute_config);
        self
    }

    /// Включает mute гильдии на одно из 6 канонических окон длительности (`900 | 3600 | 10800 | 28800 | 86400 | -1`).
    pub fn with_mute_window(
        mut self,
        window: i64,
        now_unix: i64,
    ) -> Result<Self, NotificationError> {
        let cfg = MuteConfig::from_window(window, now_unix)?;
        self.muted = Some(true);
        self.mute_config = Some(Some(cfg));
        Ok(self)
    }

    /// Снимает mute гильдии (`{ "muted": false, "mute_config": null }`, `NOTIFICATIONS-SPEC.md §1.6.3`).
    #[must_use]
    pub fn with_unmute(mut self) -> Self {
        self.muted = Some(false);
        self.mute_config = Some(None);
        self
    }

    /// Задаёт подавление `@everyone` и `@here`.
    #[must_use]
    pub fn with_suppress_everyone(mut self, suppress: bool) -> Self {
        self.suppress_everyone = Some(suppress);
        self
    }

    /// Задаёт подавление всех `@ролей`.
    #[must_use]
    pub fn with_suppress_roles(mut self, suppress: bool) -> Self {
        self.suppress_roles = Some(suppress);
        self
    }

    /// Задаёт отправку мобильных push-уведомлений.
    #[must_use]
    pub fn with_mobile_push(mut self, mobile_push: bool) -> Self {
        self.mobile_push = Some(mobile_push);
        self
    }

    /// Задаёт приглушение запланированных событий сервера.
    #[must_use]
    pub fn with_mute_scheduled_events(mut self, mute_events: bool) -> Self {
        self.mute_scheduled_events = Some(mute_events);
        self
    }

    /// Задаёт битовую маску `flags` гильдии.
    #[must_use]
    pub fn with_flags(mut self, flags: i64) -> Self {
        self.flags = Some(flags);
        self
    }

    /// Задаёт режим уведомлений о хайлайтах (`0 | 1 | 2`).
    #[must_use]
    pub fn with_notify_highlights(mut self, highlights: i64) -> Self {
        self.notify_highlights = Some(highlights);
        self
    }

    /// Добавляет или обновляет запись в карте `channel_overrides`.
    #[must_use]
    pub fn with_channel_override(
        mut self,
        channel_id: impl Into<String>,
        override_patch: ChannelNotificationOverridePatch,
    ) -> Self {
        self.channel_overrides
            .get_or_insert_with(BTreeMap::new)
            .insert(channel_id.into(), override_patch);
        self
    }

    /// Валидирует все поля PATCH-пейлоада гильдии и вложенных `channel_overrides` до обращения к сети.
    pub fn validate(&self) -> Result<(), NotificationError> {
        let has_any_field = self.message_notifications.is_some()
            || self.muted.is_some()
            || self.mute_config.is_some()
            || self.suppress_everyone.is_some()
            || self.suppress_roles.is_some()
            || self.mobile_push.is_some()
            || self.mute_scheduled_events.is_some()
            || self.flags.is_some()
            || self.notify_highlights.is_some()
            || self
                .channel_overrides
                .as_ref()
                .is_some_and(|map| !map.is_empty());

        if !has_any_field {
            return Err(NotificationError::EmptyPatch);
        }

        if let Some(level) = self.message_notifications {
            MessageNotificationLevel::from_guild_code(level)?;
        }

        if let Some(Some(cfg)) = &self.mute_config {
            cfg.validate()?;
        }

        if let Some(highlights) = self.notify_highlights {
            NotifyHighlights::from_code(highlights)?;
        }

        if let Some(overrides) = &self.channel_overrides {
            if overrides.is_empty() {
                return Err(NotificationError::EmptyPatch);
            }
            for (ch_id, ov) in overrides {
                validate_channel_id(ch_id)?;
                ov.validate()?;
            }
        }

        Ok(())
    }

    /// Валидирует и сериализует PATCH-пейлоад для `PATCH /users/@me/guilds/{guild_id}/settings`.
    pub fn to_patch_value(&self) -> Result<serde_json::Value, NotificationError> {
        self.validate()?;
        let mut map = serde_json::Map::new();
        if let Some(level) = self.message_notifications {
            map.insert(
                "message_notifications".to_string(),
                serde_json::Value::from(level),
            );
        }
        if let Some(muted) = self.muted {
            map.insert("muted".to_string(), serde_json::Value::from(muted));
        }
        if let Some(mute_cfg) = &self.mute_config {
            match mute_cfg {
                Some(cfg) => {
                    map.insert(
                        "mute_config".to_string(),
                        serde_json::json!({
                            "selected_time_window": cfg.selected_time_window,
                            "end_time": cfg.end_time,
                        }),
                    );
                }
                None => {
                    map.insert("mute_config".to_string(), serde_json::Value::Null);
                }
            }
        }
        if let Some(val) = self.suppress_everyone {
            map.insert(
                "suppress_everyone".to_string(),
                serde_json::Value::from(val),
            );
        }
        if let Some(val) = self.suppress_roles {
            map.insert("suppress_roles".to_string(), serde_json::Value::from(val));
        }
        if let Some(val) = self.mobile_push {
            map.insert("mobile_push".to_string(), serde_json::Value::from(val));
        }
        if let Some(val) = self.mute_scheduled_events {
            map.insert(
                "mute_scheduled_events".to_string(),
                serde_json::Value::from(val),
            );
        }
        if let Some(flags) = self.flags {
            map.insert("flags".to_string(), serde_json::Value::from(flags));
        }
        if let Some(highlights) = self.notify_highlights {
            map.insert(
                "notify_highlights".to_string(),
                serde_json::Value::from(highlights),
            );
        }
        if let Some(overrides) = &self.channel_overrides {
            let mut ov_map = serde_json::Map::new();
            for (ch_id, ov) in overrides {
                ov_map.insert(ch_id.clone(), ov.to_patch_value()?);
            }
            map.insert(
                "channel_overrides".to_string(),
                serde_json::Value::Object(ov_map),
            );
        }
        Ok(serde_json::Value::Object(map))
    }

    /// Валидирует и сериализует bulk-конверт `{ "guilds": { "<guild_id>": { ... } } }`
    /// для `PATCH /users/@me/guilds/settings` (`NOTIFICATIONS-RE.md §9.1–§9.2`).
    pub fn to_bulk_patch_value(
        &self,
        guild_id: &str,
    ) -> Result<serde_json::Value, NotificationError> {
        validate_guild_id(guild_id)?;
        if guild_id == "@me" {
            return Err(NotificationError::InvalidGuildId);
        }
        let inner = self.to_patch_value()?;
        let mut guilds = serde_json::Map::new();
        guilds.insert(guild_id.to_string(), inner);
        let mut root = serde_json::Map::new();
        root.insert("guilds".to_string(), serde_json::Value::Object(guilds));
        Ok(serde_json::Value::Object(root))
    }
}

/// Типизированная ошибка операций с настройками уведомлений (`N-05`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum NotificationError {
    /// Отсутствует или пустой токен авторизации.
    MissingToken,
    /// Некорректный или пустой `guild_id`.
    InvalidGuildId,
    /// Некорректный или пустой `channel_id`.
    InvalidChannelId,
    /// Недопустимое значение `message_notifications` (для гильдии допустимы `0|1|2`, для канала `0|1|2|3`).
    InvalidMessageNotifications { value: i64 },
    /// Недопустимая конфигурация `mute_config` (окно не из `900|3600|10800|28800|86400|-1` или битый `end_time`).
    InvalidMuteConfig,
    /// Недопустимое значение `notify_highlights` (допустимы `0|1|2`).
    InvalidNotifyHighlights { value: i64 },
    /// Пустой PATCH-пейлоад (ни одно поле не задано).
    EmptyPatch,
    /// 401: сессия истекла.
    Unauthorized,
    /// 403: нет доступа к настройкам гильдии.
    Forbidden,
    /// 404: гильдия или канал не найдены.
    NotFound,
    /// 429: ограничение частоты запросов; задержка `retry-after` в миллисекундах.
    RateLimited { retry_after_ms: u64 },
    /// Прочие 4xx (кроме 401/403/404/429).
    Client { status: u16 },
    /// 5xx: ошибка сервера Discord.
    Server { status: u16 },
    /// Транспорт/таймаут — сеть недоступна.
    Network,
    /// Отказ хранилища токена (SEC-02).
    Storage,
    /// Ответ 2xx не соответствует ожидаемой JSON-схеме.
    InvalidResponse,
}

impl fmt::Display for NotificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingToken => formatter.write_str("токен не найден"),
            Self::InvalidGuildId => formatter.write_str("некорректный идентификатор сервера"),
            Self::InvalidChannelId => formatter.write_str("некорректный идентификатор канала"),
            Self::InvalidMessageNotifications { value } => {
                write!(
                    formatter,
                    "недопустимый уровень message_notifications: {value}"
                )
            }
            Self::InvalidMuteConfig => formatter.write_str("некорректная конфигурация mute_config"),
            Self::InvalidNotifyHighlights { value } => {
                write!(
                    formatter,
                    "недопустимое значение notify_highlights: {value}"
                )
            }
            Self::EmptyPatch => {
                formatter.write_str("пустой запрос обновления настроек уведомлений")
            }
            Self::Unauthorized => formatter.write_str("сессия истекла"),
            Self::Forbidden => formatter.write_str("нет доступа к настройкам сервера"),
            Self::NotFound => formatter.write_str("сервер или канал не найдены"),
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
            Self::Storage => formatter.write_str("хранилище токена недоступно"),
            Self::InvalidResponse => formatter.write_str("некорректный ответ настроек уведомлений"),
        }
    }
}

impl std::error::Error for NotificationError {}

impl From<RestFailure> for NotificationError {
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

/// Асинхронный транспорт для `PATCH`-запросов настроек уведомлений.
pub trait NotificationTransport: Send + Sync {
    fn patch<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, NotificationError>> + Send + 'a>>;
}

struct ApiNotificationTransport(ApiClient);

impl NotificationTransport for ApiNotificationTransport {
    fn patch<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, NotificationError>> + Send + 'a>> {
        Box::pin(async move {
            self.0
                .patch(target, body)
                .await
                .map_err(|error| NotificationError::from(RestFailure::from_api_error(&error)))
        })
    }
}

/// REST-клиент настроек уведомлений гильдий и переопределений каналов (`N-05`).
pub struct DiscordNotifications {
    transport: Arc<dyn NotificationTransport>,
    token: String,
}

impl fmt::Debug for DiscordNotifications {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DiscordNotifications")
            .field("token", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl DiscordNotifications {
    /// Создаёт клиент с боевым транспортом ядра; пустой токен отклоняется до сети.
    pub fn new(token: &str) -> Result<Self, NotificationError> {
        validate_token(token)?;
        let identity_ctx = crate::auth::IdentityContext::detect();
        let auth = ApiAuth::from_token(
            token,
            &identity_ctx.super_properties,
            &identity_ctx.locale,
            &identity_ctx.timezone,
        )
        .map_err(|_| NotificationError::MissingToken)?;
        let client = ApiClient::new(ApiConfig {
            base: API_V9_BASE.to_string(),
            auth,
            http: HttpClientConfig::default(),
            retry: ApiRetryPolicy {
                max_attempts: 1,
                ..ApiRetryPolicy::default()
            },
        })
        .map_err(|_| NotificationError::Network)?;
        Ok(Self {
            transport: Arc::new(ApiNotificationTransport(client)),
            token: token.trim().to_string(),
        })
    }

    /// Конструктор с инъекцией транспорта для контрактных тестов.
    pub fn with_transport(
        token: &str,
        transport: Arc<dyn NotificationTransport>,
    ) -> Result<Self, NotificationError> {
        validate_token(token)?;
        Ok(Self {
            transport,
            token: token.trim().to_string(),
        })
    }

    /// Обновляет настройки уведомлений гильдии (`PATCH /users/@me/guilds/{guild_id}/settings`).
    pub async fn update_guild_notification_settings(
        &self,
        guild_id: &str,
        patch: &GuildNotificationSettingsPatch,
    ) -> Result<serde_json::Value, NotificationError> {
        update_guild_notification_settings(&*self.transport, &self.token, guild_id, patch).await
    }

    /// Обновляет переопределение уведомлений канала (`PATCH /users/@me/guilds/{guild_id}/settings`).
    pub async fn update_channel_notification_override(
        &self,
        guild_id: &str,
        channel_id: &str,
        override_patch: &ChannelNotificationOverridePatch,
    ) -> Result<serde_json::Value, NotificationError> {
        update_channel_notification_override(
            &*self.transport,
            &self.token,
            guild_id,
            channel_id,
            override_patch,
        )
        .await
    }
}

impl NotificationTransport for DiscordNotifications {
    fn patch<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, NotificationError>> + Send + 'a>> {
        self.transport.patch(target, body)
    }
}

/// Устанавливает бит `UNREADS_*` в флагах гильдии (`mD`, `@11923745`), сохраняя старшие биты (`OPT_IN_CHANNELS_*`).
#[must_use]
pub const fn set_guild_unread_flag(flags: i64, bit: i64) -> i64 {
    (flags & !GUILD_UNREAD_MASK) | (bit & GUILD_UNREAD_MASK)
}

/// Сбрасывает оба бита `UNREADS_*` в флагах гильдии (`PW`, `@11923745`).
#[must_use]
pub const fn clear_guild_unread_flags(flags: i64) -> i64 {
    flags & !GUILD_UNREAD_MASK
}

/// Устанавливает бит `UNREADS_*` в флагах канала (`mD`, `@11923745`), сохраняя `OPT_IN_ENABLED` и `FAVORITED`.
#[must_use]
pub const fn set_channel_unread_flag(flags: i64, bit: i64) -> i64 {
    (flags & !CHANNEL_UNREAD_MASK) | (bit & CHANNEL_UNREAD_MASK)
}

/// Сбрасывает оба бита `UNREADS_*` в флагах канала (`PW`, `@11923745`).
#[must_use]
pub const fn clear_channel_unread_flags(flags: i64) -> i64 {
    flags & !CHANNEL_UNREAD_MASK
}

/// Формирует путь `PATCH /users/@me/guilds/{guild_id}/settings` после валидации `guild_id`.
pub fn build_guild_settings_endpoint(guild_id: &str) -> Result<String, NotificationError> {
    validate_guild_id(guild_id)?;
    Ok(format!("/users/@me/guilds/{guild_id}/settings"))
}

/// Вычисляет эффективный статус `isMuted` записи (`@2364848`, модуль `544743`):
/// `muted && (!cfg || !cfg.end_time || end_time >= now_unix)`.
#[must_use]
pub fn is_entry_muted(muted: bool, mute_config: Option<&MuteConfig>, now_unix: i64) -> bool {
    if !muted {
        return false;
    }
    let Some(cfg) = mute_config else {
        return true;
    };
    let Some(end_raw) = cfg.end_time.as_deref() else {
        return true;
    };
    if end_raw.is_empty() || end_raw == "nil" {
        return true;
    }
    parse_iso8601_unix(end_raw).is_some_and(|end_unix| end_unix >= now_unix)
}

/// Вычисляет признак временного mute с активным таймером (`isTemporarilyMuted`, `@2364848`):
/// `muted && cfg && cfg.end_time && end_time >= now_unix`.
#[must_use]
pub fn is_temporarily_muted(muted: bool, mute_config: Option<&MuteConfig>, now_unix: i64) -> bool {
    if !muted {
        return false;
    }
    let Some(cfg) = mute_config else {
        return false;
    };
    let Some(end_raw) = cfg.end_time.as_deref() else {
        return false;
    };
    if end_raw.is_empty() || end_raw == "nil" {
        return false;
    }
    parse_iso8601_unix(end_raw).is_some_and(|end_unix| end_unix >= now_unix)
}

/// Каскадный резолвер эффективной политики уведомлений канала (`NOTIFICATIONS-RE.md §6`, `NOTIFICATIONS-SPEC.md §1.5`).
#[must_use]
pub fn resolve_effective_notifications(
    guild: &GuildNotificationPolicyInput,
    parent_category: Option<&ChannelNotificationOverride>,
    channel: Option<&ChannelNotificationOverride>,
    now_unix: i64,
) -> EffectiveNotificationPolicy {
    let guild_muted = is_entry_muted(guild.muted, guild.mute_config.as_ref(), now_unix);
    let category_muted = parent_category
        .is_some_and(|cat| is_entry_muted(cat.muted, cat.mute_config.as_ref(), now_unix));
    let channel_muted =
        channel.is_some_and(|ch| is_entry_muted(ch.muted, ch.mute_config.as_ref(), now_unix));
    let is_muted = guild_muted || category_muted || channel_muted;

    let effective_level = if let Some(ch_level) = channel
        .map(|ch| ch.message_notifications)
        .filter(|lvl| VALID_GUILD_MESSAGE_NOTIFICATIONS.contains(lvl))
    {
        ch_level
    } else if let Some(cat_level) = parent_category
        .map(|cat| cat.message_notifications)
        .filter(|lvl| VALID_GUILD_MESSAGE_NOTIFICATIONS.contains(lvl))
    {
        cat_level
    } else if VALID_GUILD_MESSAGE_NOTIFICATIONS.contains(&guild.message_notifications) {
        guild.message_notifications
    } else if VALID_GUILD_MESSAGE_NOTIFICATIONS.contains(&guild.default_message_notifications) {
        guild.default_message_notifications
    } else {
        MESSAGE_NOTIFICATIONS_ONLY_MENTIONS
    };

    let opt_in_suppressed = if (guild.flags & GUILD_FLAG_OPT_IN_CHANNELS_ON) != 0 {
        let ch_opted_in = channel
            .and_then(|c| c.flags)
            .is_some_and(|f| (f & CHANNEL_FLAG_OPT_IN_ENABLED) != 0);
        let cat_opted_in = parent_category
            .and_then(|c| c.flags)
            .is_some_and(|f| (f & CHANNEL_FLAG_OPT_IN_ENABLED) != 0);
        !ch_opted_in && !cat_opted_in
    } else {
        false
    };

    let allow_no_messages =
        is_muted || effective_level == MESSAGE_NOTIFICATIONS_NO_MESSAGES || opt_in_suppressed;

    EffectiveNotificationPolicy {
        is_muted,
        guild_muted,
        category_muted,
        channel_muted,
        effective_level,
        allow_no_messages,
        suppress_everyone: guild.suppress_everyone,
        suppress_roles: guild.suppress_roles,
    }
}

/// Обновляет настройки уведомлений гильдии (`PATCH /users/@me/guilds/{guild_id}/settings`).
///
/// Отклоняет пустой `token`, некорректный `guild_id` и невалидный `patch` до вызова транспорта.
pub async fn update_guild_notification_settings<T: NotificationTransport + ?Sized>(
    transport: &T,
    token: &str,
    guild_id: &str,
    patch: &GuildNotificationSettingsPatch,
) -> Result<serde_json::Value, NotificationError> {
    validate_token(token)?;
    let target = build_guild_settings_endpoint(guild_id)?;
    let payload = patch.to_patch_value()?;
    let body = serde_json::to_vec(&payload).map_err(|_| NotificationError::InvalidResponse)?;
    let response = transport.patch(&target, &body).await?;
    decode_settings_response(&response)
}

/// Обновляет переопределение уведомлений канала (`PATCH /users/@me/guilds/{guild_id}/settings`
/// с картой `{"channel_overrides": {"<channel_id>": { ... }}}`).
///
/// Отклоняет пустой `token`, некорректный `guild_id`, некорректный `channel_id` и невалидный
/// `override_patch` до вызова транспорта.
pub async fn update_channel_notification_override<T: NotificationTransport + ?Sized>(
    transport: &T,
    token: &str,
    guild_id: &str,
    channel_id: &str,
    override_patch: &ChannelNotificationOverridePatch,
) -> Result<serde_json::Value, NotificationError> {
    validate_token(token)?;
    validate_guild_id(guild_id)?;
    validate_channel_id(channel_id)?;
    override_patch.validate()?;

    let guild_patch = GuildNotificationSettingsPatch::new()
        .with_channel_override(channel_id.to_string(), override_patch.clone());
    update_guild_notification_settings(transport, token, guild_id, &guild_patch).await
}

fn validate_token(token: &str) -> Result<(), NotificationError> {
    if token.trim().is_empty() {
        return Err(NotificationError::MissingToken);
    }
    Ok(())
}

fn is_safe_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= MAX_ID_BYTES
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_' || *byte == b'-')
}

fn validate_guild_id(guild_id: &str) -> Result<(), NotificationError> {
    if guild_id == "@me" || is_safe_id(guild_id) {
        return Ok(());
    }
    Err(NotificationError::InvalidGuildId)
}

fn validate_channel_id(channel_id: &str) -> Result<(), NotificationError> {
    if !is_safe_id(channel_id) {
        return Err(NotificationError::InvalidChannelId);
    }
    Ok(())
}

fn decode_settings_response(
    response: &HttpResponse,
) -> Result<serde_json::Value, NotificationError> {
    if response.status == 429 {
        return Err(NotificationError::RateLimited {
            retry_after_ms: parse_retry_after_ms(response),
        });
    }
    match response.status {
        200..=299 => {
            if response.body.iter().all(|byte| byte.is_ascii_whitespace()) {
                return Ok(serde_json::json!({}));
            }
            let val: serde_json::Value = serde_json::from_slice(&response.body)
                .map_err(|_| NotificationError::InvalidResponse)?;
            if !val.is_object() {
                return Err(NotificationError::InvalidResponse);
            }
            Ok(val)
        }
        other => Err(RestFailure::from_status(other)
            .map_or(NotificationError::Network, NotificationError::from)),
    }
}

fn parse_retry_after_ms(response: &HttpResponse) -> u64 {
    if let Some(ms) = response
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("retry-after"))
        .and_then(|(_, value)| value.trim().parse::<f64>().ok())
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
        .map(|seconds| (seconds * 1000.0).round() as u64)
    {
        return ms;
    }
    serde_json::from_slice::<serde_json::Value>(&response.body)
        .ok()
        .and_then(|val| val.get("retry_after").and_then(serde_json::Value::as_f64))
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
        .map_or(0, |seconds| (seconds * 1000.0).round() as u64)
}

const fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

const fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

const fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let m_adj = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * m_adj + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + (doe as i64) - 719_468
}

const fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year, m, d)
}

/// Разбирает ISO-8601 / RFC 3339 строку (`YYYY-MM-DDTHH:MM:SS[.ffffff](Z|+HH:MM|-HH:MM)`)
/// в Unix-время (секунды от 1970-01-01T00:00:00Z).
#[must_use]
pub fn parse_iso8601_unix(iso: &str) -> Option<i64> {
    let s = iso.trim();
    if s.len() < 20 {
        return None;
    }
    let bytes = s.as_bytes();
    if bytes[4] != b'-' || bytes[7] != b'-' || (bytes[10] != b'T' && bytes[10] != b't') {
        return None;
    }
    if bytes[13] != b':' || bytes[16] != b':' {
        return None;
    }

    let year: i64 = s.get(0..4)?.parse().ok()?;
    let month: u32 = s.get(5..7)?.parse().ok()?;
    let day: u32 = s.get(8..10)?.parse().ok()?;
    let hour: u32 = s.get(11..13)?.parse().ok()?;
    let minute: u32 = s.get(14..16)?.parse().ok()?;
    let second: u32 = s.get(17..19)?.parse().ok()?;

    if !(1..=12).contains(&month) {
        return None;
    }
    let max_day = days_in_month(year, month);
    if day < 1 || day > max_day || hour > 23 || minute > 59 || second > 59 {
        return None;
    }

    let mut rest = s.get(19..)?;
    if let Some(after_dot) = rest.strip_prefix('.') {
        let frac_len = after_dot.bytes().take_while(u8::is_ascii_digit).count();
        if frac_len == 0 {
            return None;
        }
        rest = after_dot.get(frac_len..)?;
    }

    let offset_secs: i64 = if rest == "Z" || rest == "z" {
        0
    } else if rest.len() == 6
        && (rest.starts_with('+') || rest.starts_with('-'))
        && rest.as_bytes()[3] == b':'
    {
        let sign = if rest.starts_with('-') { -1i64 } else { 1i64 };
        let tz_hour: u32 = rest.get(1..3)?.parse().ok()?;
        let tz_min: u32 = rest.get(4..6)?.parse().ok()?;
        if tz_hour > 23 || tz_min > 59 {
            return None;
        }
        sign * i64::from(tz_hour * 3600 + tz_min * 60)
    } else {
        return None;
    };

    let days = days_from_civil(year, month, day);
    let day_secs = i64::from(hour * 3600 + minute * 60 + second);
    Some(days * 86_400 + day_secs - offset_secs)
}

/// Форматирует Unix-время (секунды) в каноническую UTC-строку ISO-8601 (`YYYY-MM-DDTHH:MM:SS.000Z`).
#[must_use]
pub fn format_iso8601_utc(unix_secs: i64) -> String {
    let days = unix_secs.div_euclid(86_400);
    let secs_of_day = unix_secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = secs_of_day / 3600;
    let minute = (secs_of_day % 3600) / 60;
    let second = secs_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.000Z")
}

/// Подключает нативный `chord_platform::notifications::Notifier` по D-Bus
/// и отправляет десктопное системное уведомление по спецификации freedesktop
/// (STUB-28, N-01/N-02).
pub fn send_desktop_notification(
    summary: &str,
    body: &str,
    category: Option<&str>,
) -> Result<u32, String> {
    let raw = std::env::var("DBUS_SESSION_BUS_ADDRESS")
        .map_err(|_| "Переменная окружения DBUS_SESSION_BUS_ADDRESS не задана".to_string())?;
    let address = chord_platform::dbus::BusAddress::parse(&raw)
        .ok_or_else(|| format!("Не удалось распарсить DBUS_SESSION_BUS_ADDRESS: {raw}"))?;
    let mut notifier = chord_platform::notifications::Notifier::connect(&address)
        .map_err(|e| format!("D-Bus Notifier connect error: {e:?}"))?;
    let mut hints = chord_platform::notifications::Hints::default();
    hints.desktop_entry = Some("discord".to_string());
    if let Some(cat) = category {
        hints.category = Some(cat.to_string());
    }
    let notif = chord_platform::notifications::Notification {
        app_name: "Discord".to_string(),
        summary: summary.to_string(),
        body: body.to_string(),
        replaces_id: 0,
        actions: vec![],
        timeout_ms: 5000,
    };
    notifier
        .notify_with_hints(&notif, &hints)
        .map_err(|e| format!("D-Bus notify error: {e:?}"))
}
