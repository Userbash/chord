//! P2-08: типизированные секции `PreloadedUserSettings` (RW-21).
//!
//! Схема подтверждена generated-классами Discord (`waves/RW-21/RESEARCH-P2-08.md` §3).
//! Типизируются только поля, нужные профилю/голосу/уведомлениям; остальные top-level
//! поля сохраняются в [`PreloadedUserSettings::raw`] без потери.

use std::fmt;

use super::wire::{ProtoMessage, SettingsProtoError};

/// `PreloadedUserSettings` из READY (`user_settings_proto`, type 1).
#[derive(Clone, PartialEq, Eq)]
pub struct PreloadedUserSettings {
    /// Полное top-level сообщение, включая неизвестные поля (guilds/folders/…).
    pub raw: ProtoMessage,
    /// Версии схемы настроек.
    pub versions: Option<Versions>,
    /// Inbox-вкладка/туториал.
    pub inbox: Option<InboxSettings>,
    /// Голос и видео.
    pub voice_and_video: Option<VoiceAndVideoSettings>,
    /// Текст и изображения.
    pub text_and_images: Option<TextAndImagesSettings>,
    /// Уведомления.
    pub notifications: Option<NotificationSettings>,
    /// Статус.
    pub status: Option<StatusSettings>,
    /// Локализация.
    pub localization: Option<LocalizationSettings>,
    /// Внешний вид.
    pub appearance: Option<AppearanceSettings>,
}

impl PreloadedUserSettings {
    /// Разобрать bytes `PreloadedUserSettings`.
    pub fn decode(bytes: &[u8]) -> Result<Self, SettingsProtoError> {
        let raw = ProtoMessage::decode(bytes)?;
        Ok(Self {
            versions: raw
                .message(1)?
                .map(|message| Versions::from_message(&message))
                .transpose()?,
            inbox: raw
                .message(2)?
                .map(|message| InboxSettings::from_message(&message))
                .transpose()?,
            voice_and_video: raw
                .message(5)?
                .map(|message| VoiceAndVideoSettings::from_message(&message))
                .transpose()?,
            text_and_images: raw
                .message(6)?
                .map(|message| TextAndImagesSettings::from_message(&message))
                .transpose()?,
            notifications: raw
                .message(7)?
                .map(|message| NotificationSettings::from_message(&message))
                .transpose()?,
            status: raw
                .message(11)?
                .map(|message| StatusSettings::from_message(&message))
                .transpose()?,
            localization: raw
                .message(12)?
                .map(|message| LocalizationSettings::from_message(&message))
                .transpose()?,
            appearance: raw
                .message(13)?
                .map(|message| AppearanceSettings::from_message(&message))
                .transpose()?,
            raw,
        })
    }
}

impl fmt::Debug for PreloadedUserSettings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreloadedUserSettings")
            .field("raw_fields", &self.raw.fields.len())
            .field("versions", &self.versions.is_some())
            .field("inbox", &self.inbox.is_some())
            .field("voice_and_video", &self.voice_and_video.is_some())
            .field("text_and_images", &self.text_and_images.is_some())
            .field("notifications", &self.notifications.is_some())
            .field("status", &self.status.is_some())
            .field("localization", &self.localization.is_some())
            .field("appearance", &self.appearance.is_some())
            .finish()
    }
}

/// `Versions` настройки.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Versions {
    /// Клиентская версия.
    pub client_version: Option<i32>,
    /// Серверная версия.
    pub server_version: Option<i32>,
    /// Версия данных.
    pub data_version: Option<i32>,
}

impl Versions {
    pub(crate) fn from_message(message: &ProtoMessage) -> Result<Self, SettingsProtoError> {
        Ok(Self {
            client_version: message.scalar_i32(1)?,
            server_version: message.scalar_i32(2)?,
            data_version: message.scalar_i32(3)?,
        })
    }
}

/// `InboxSettings`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxSettings {
    /// Текущая вкладка.
    pub current_tab: Option<i32>,
    /// Просмотрен ли туториал.
    pub viewed_tutorial: Option<bool>,
}

impl InboxSettings {
    fn from_message(message: &ProtoMessage) -> Result<Self, SettingsProtoError> {
        Ok(Self {
            current_tab: message.scalar_i32(1)?,
            viewed_tutorial: message.scalar_bool(2)?,
        })
    }
}

/// `AppearanceSettings`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppearanceSettings {
    /// `Theme`: 1 DARK, 2 LIGHT, 3 DARKER, 4 MIDNIGHT.
    pub theme: Option<i32>,
    /// Developer mode.
    pub developer_mode: Option<bool>,
    /// Mobile redesign disabled.
    pub mobile_redesign_disabled: Option<bool>,
    /// `TimestampHourCycle`: 1 H12, 2 H23.
    pub timestamp_hour_cycle: Option<i32>,
    /// `LaunchPadMode`.
    pub launch_pad_mode: Option<i32>,
    /// `UIDensity`: 1 COMPACT, 2 COZY, 3 RESPONSIVE, 4 DEFAULT.
    pub ui_density: Option<i32>,
    /// `SwipeRightToLeftMode`.
    pub swipe_right_to_left_mode: Option<i32>,
    /// `GuildThemeSourcePreference`.
    pub default_guild_theme_preference: Option<i32>,
    /// Dark sidebar.
    pub dark_sidebar: Option<bool>,
}

impl AppearanceSettings {
    fn from_message(message: &ProtoMessage) -> Result<Self, SettingsProtoError> {
        Ok(Self {
            theme: message.scalar_i32(1)?,
            developer_mode: message.scalar_bool(2)?,
            mobile_redesign_disabled: message.scalar_bool(4)?,
            timestamp_hour_cycle: message.scalar_i32(9)?,
            launch_pad_mode: message.scalar_i32(11)?,
            ui_density: message.scalar_i32(12)?,
            swipe_right_to_left_mode: message.scalar_i32(13)?,
            default_guild_theme_preference: message.scalar_i32(14)?,
            dark_sidebar: message.scalar_bool(15)?,
        })
    }
}

/// `VoiceAndVideoSettings`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceAndVideoSettings {
    /// Пресет видео.
    pub preset_option: Option<i32>,
    /// Всегда показывать превью видео.
    pub always_preview_video: Option<bool>,
    /// AFK timeout, секунды.
    pub afk_timeout: Option<i32>,
    /// Уведомления о стриме.
    pub stream_notifications_enabled: Option<bool>,
    /// Нативная телефонная интеграция.
    pub native_phone_integration_enabled: Option<bool>,
    /// Отключить превью стрима.
    pub disable_stream_previews: Option<bool>,
}

impl VoiceAndVideoSettings {
    fn from_message(message: &ProtoMessage) -> Result<Self, SettingsProtoError> {
        Ok(Self {
            preset_option: message.scalar_i32(2)?,
            always_preview_video: message.bool_wrapper(5)?,
            afk_timeout: message.int32_wrapper(6)?,
            stream_notifications_enabled: message.bool_wrapper(7)?,
            native_phone_integration_enabled: message.bool_wrapper(8)?,
            disable_stream_previews: message.bool_wrapper(10)?,
        })
    }
}

/// `TextAndImagesSettings` (подмножество подтверждённых полей READY).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextAndImagesSettings {
    /// Показывать описания изображений.
    pub view_image_descriptions: Option<bool>,
    /// Inline-вложения.
    pub inline_attachment_media: Option<bool>,
    /// Inline-embeds.
    pub inline_embed_media: Option<bool>,
    /// Автовоспроизведение GIF.
    pub gif_auto_play: Option<bool>,
    /// Рендерить embeds.
    pub render_embeds: Option<bool>,
    /// Рендерить реакции.
    pub render_reactions: Option<bool>,
    /// Анимировать emoji.
    pub animate_emoji: Option<bool>,
    /// `AnimateStickers` int32.
    pub animate_stickers: Option<i32>,
    /// TTS-команда.
    pub enable_tts_command: Option<bool>,
    /// Компактный режим сообщений.
    pub message_display_compact: Option<bool>,
    /// Explicit content filter.
    pub explicit_content_filter: Option<i32>,
    /// NSFW-гильдии.
    pub view_nsfw_guilds: Option<bool>,
    /// Конвертировать emoticons.
    pub convert_emoticons: Option<bool>,
    /// Подсказки выражений.
    pub expression_suggestions_enabled: Option<bool>,
    /// NSFW-команды.
    pub view_nsfw_commands: Option<bool>,
    /// `DmSpamFilterV2`.
    pub dm_spam_filter_v2: Option<i32>,
}

impl TextAndImagesSettings {
    fn from_message(message: &ProtoMessage) -> Result<Self, SettingsProtoError> {
        Ok(Self {
            view_image_descriptions: message.bool_wrapper(7)?,
            inline_attachment_media: message.bool_wrapper(9)?,
            inline_embed_media: message.bool_wrapper(10)?,
            gif_auto_play: message.bool_wrapper(11)?,
            render_embeds: message.bool_wrapper(12)?,
            render_reactions: message.bool_wrapper(13)?,
            animate_emoji: message.bool_wrapper(14)?,
            animate_stickers: message.int32_wrapper(15)?,
            enable_tts_command: message.bool_wrapper(16)?,
            message_display_compact: message.bool_wrapper(17)?,
            explicit_content_filter: message.int32_wrapper(19)?,
            view_nsfw_guilds: message.bool_wrapper(20)?,
            convert_emoticons: message.bool_wrapper(21)?,
            expression_suggestions_enabled: message.bool_wrapper(22)?,
            view_nsfw_commands: message.bool_wrapper(23)?,
            dm_spam_filter_v2: message.scalar_i32(27)?,
        })
    }
}

/// `NotificationSettings` (подмножество подтверждённых полей).
#[derive(Clone, PartialEq, Eq)]
pub struct NotificationSettings {
    /// In-app уведомления.
    pub show_in_app_notifications: Option<bool>,
    /// Уведомления о go-live друзей.
    pub notify_friends_on_go_live: Option<bool>,
    /// Fixed64 id последнего прочитанного уведомления.
    pub notification_center_acked_before_id: Option<u64>,
    /// Взрывные реакции.
    pub enable_burst_reaction_notifications: Option<bool>,
    /// Quiet mode.
    pub quiet_mode: Option<bool>,
    /// `ReactionNotificationType`.
    pub reaction_notifications: Option<i32>,
    /// `GameActivityNotificationType`.
    pub game_activity_notifications: Option<i32>,
    /// `CustomStatusPushNotificationType`.
    pub custom_status_push_notifications: Option<i32>,
}

impl NotificationSettings {
    fn from_message(message: &ProtoMessage) -> Result<Self, SettingsProtoError> {
        Ok(Self {
            show_in_app_notifications: message.bool_wrapper(1)?,
            notify_friends_on_go_live: message.bool_wrapper(2)?,
            notification_center_acked_before_id: message.scalar_fixed64(3)?,
            enable_burst_reaction_notifications: message.bool_wrapper(4)?,
            quiet_mode: message.bool_wrapper(5)?,
            reaction_notifications: message.scalar_i32(7)?,
            game_activity_notifications: message.scalar_i32(8)?,
            custom_status_push_notifications: message.scalar_i32(9)?,
        })
    }
}

impl fmt::Debug for NotificationSettings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NotificationSettings")
            .field("show_in_app_notifications", &self.show_in_app_notifications)
            .field("notify_friends_on_go_live", &self.notify_friends_on_go_live)
            // Значение id — пользовательский идентификатор; в логах только факт наличия.
            .field(
                "notification_center_acked_before_id_present",
                &self.notification_center_acked_before_id.is_some(),
            )
            .field(
                "enable_burst_reaction_notifications",
                &self.enable_burst_reaction_notifications,
            )
            .field("quiet_mode", &self.quiet_mode)
            .field("reaction_notifications", &self.reaction_notifications)
            .field(
                "game_activity_notifications",
                &self.game_activity_notifications,
            )
            .field(
                "custom_status_push_notifications",
                &self.custom_status_push_notifications,
            )
            .finish()
    }
}

/// `StatusSettings` (custom_status намеренно не типизирован в P2-08).
#[derive(Clone, PartialEq, Eq)]
pub struct StatusSettings {
    /// Текстовый статус (`online`/`idle`/`dnd`/`invisible`/…).
    pub status: Option<String>,
    /// Показывать текущую игру.
    pub show_current_game: Option<bool>,
    /// Истечение статуса, мс.
    pub status_expires_at_ms: Option<u64>,
    /// Создание статуса, мс.
    pub status_created_at_ms: Option<u64>,
}

impl StatusSettings {
    fn from_message(message: &ProtoMessage) -> Result<Self, SettingsProtoError> {
        Ok(Self {
            status: message.string(1)?,
            show_current_game: message.bool_wrapper(3)?,
            status_expires_at_ms: message.scalar_u64(4)?,
            status_created_at_ms: message.uint64_wrapper(5)?,
        })
    }
}

impl fmt::Debug for StatusSettings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StatusSettings")
            .field("status_len", &self.status.as_ref().map(String::len))
            .field("show_current_game", &self.show_current_game)
            .field("has_expires", &self.status_expires_at_ms.is_some())
            .field("has_created", &self.status_created_at_ms.is_some())
            .finish()
    }
}

/// `LocalizationSettings`.
#[derive(Clone, PartialEq, Eq)]
pub struct LocalizationSettings {
    /// Locale, например `ru`.
    pub locale: Option<String>,
    /// Смещение timezone в секундах (signed int64).
    pub timezone_offset: Option<i64>,
    /// IANA timezone.
    pub timezone_name: Option<String>,
}

impl fmt::Debug for LocalizationSettings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalizationSettings")
            .field("locale_len", &self.locale.as_ref().map(String::len))
            .field("timezone_offset_present", &self.timezone_offset.is_some())
            .field(
                "timezone_name_len",
                &self.timezone_name.as_ref().map(String::len),
            )
            .finish()
    }
}

impl LocalizationSettings {
    fn from_message(message: &ProtoMessage) -> Result<Self, SettingsProtoError> {
        Ok(Self {
            locale: message.string(1)?,
            timezone_offset: message.int64_wrapper(2)?,
            timezone_name: message.string(3)?,
        })
    }
}
