//! P2-08: settings-proto (RW-21).
//!
//! Публичная граница: base64/bytes → [`SettingsProto`]. Type 1 — READY
//! `user_settings_proto` (`PreloadedUserSettings`), type 2 — `USER_SETTINGS_PROTO_UPDATE`
//! (`FrecencyUserSettings`). Неизвестный type сохраняется как generic [`ProtoMessage`].
//!
//! Модуль не делает I/O, не знает про `State`, REST, UI и live Discord; base64 берётся из
//! уже принятого `rest::base64` (P1-08), чтобы не дублировать декодер.

pub mod frecency;
pub mod preloaded;
pub mod wire;

use crate::rest::base64;

pub use frecency::FrecencyUserSettings;
pub use preloaded::{
    AppearanceSettings, InboxSettings, LocalizationSettings, NotificationSettings,
    PreloadedUserSettings, StatusSettings, TextAndImagesSettings, Versions, VoiceAndVideoSettings,
};
pub use wire::{
    MAX_FIELDS, MAX_PROTO_BYTES, ProtoField, ProtoMessage, ProtoValue, SettingsProtoError,
};

/// `PRELOADED_USER_SETTINGS` из enum `UserSettingsProtoType`.
pub const PRELOADED_USER_SETTINGS_TYPE: u64 = 1;

/// `FRECENCY_AND_FAVORITES_SETTINGS` из enum `UserSettingsProtoType`.
pub const FRECENCY_AND_FAVORITES_SETTINGS_TYPE: u64 = 2;

/// Разобранные настройки в зависимости от `settings.type`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsProto {
    /// Type 1 — READY `user_settings_proto`.
    Preloaded(Box<PreloadedUserSettings>),
    /// Type 2 — `USER_SETTINGS_PROTO_UPDATE`.
    Frecency(FrecencyUserSettings),
    /// Неизвестный type; wire сохранён без интерпретации.
    Unsupported {
        /// Числовой type из события.
        proto_type: u64,
        /// Generic top-level сообщение.
        message: ProtoMessage,
    },
}

impl SettingsProto {
    /// Разобрать bytes по числовому `settings.type`.
    pub fn parse(proto_type: u64, bytes: &[u8]) -> Result<Self, SettingsProtoError> {
        match proto_type {
            PRELOADED_USER_SETTINGS_TYPE => Ok(Self::Preloaded(Box::new(
                PreloadedUserSettings::decode(bytes)?,
            ))),
            FRECENCY_AND_FAVORITES_SETTINGS_TYPE => {
                Ok(Self::Frecency(FrecencyUserSettings::decode(bytes)?))
            }
            other => Ok(Self::Unsupported {
                proto_type: other,
                message: ProtoMessage::decode(bytes)?,
            }),
        }
    }

    /// Числовой type, с которым настройки пришли.
    #[must_use]
    pub fn proto_type(&self) -> u64 {
        match self {
            Self::Preloaded(_) => PRELOADED_USER_SETTINGS_TYPE,
            Self::Frecency(_) => FRECENCY_AND_FAVORITES_SETTINGS_TYPE,
            Self::Unsupported { proto_type, .. } => *proto_type,
        }
    }

    /// Top-level wire-сообщение (включая неизвестные поля).
    #[must_use]
    pub fn raw(&self) -> &ProtoMessage {
        match self {
            Self::Preloaded(settings) => &settings.raw,
            Self::Frecency(settings) => &settings.raw,
            Self::Unsupported { message, .. } => message,
        }
    }
}

/// Разобрать base64-строку настроек по числовому type.
pub fn decode_settings(
    proto_type: u64,
    base64_text: &str,
) -> Result<SettingsProto, SettingsProtoError> {
    let bytes = base64::decode(base64_text)?;
    SettingsProto::parse(proto_type, &bytes)
}

/// Разобрать base64 `PreloadedUserSettings` (type 1, READY).
pub fn decode_preloaded_user_settings(
    base64_text: &str,
) -> Result<PreloadedUserSettings, SettingsProtoError> {
    let bytes = base64::decode(base64_text)?;
    PreloadedUserSettings::decode(&bytes)
}

/// Разобрать base64 `FrecencyUserSettings` (type 2, UPDATE).
pub fn decode_frecency_user_settings(
    base64_text: &str,
) -> Result<FrecencyUserSettings, SettingsProtoError> {
    let bytes = base64::decode(base64_text)?;
    FrecencyUserSettings::decode(&bytes)
}
