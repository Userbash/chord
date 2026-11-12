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

use std::fmt;

use chord_json::JsonValue;

use crate::rest::base64::{self, Base64Error};
use crate::rest::rate_limit::{RateLimitInfo, rate_limit_info};
use crate::rest::routes::HttpMethod;
use crate::rest::transport::HttpRequest;

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

    /// Закодировать настройки в protobuf wire-байты.
    #[must_use]
    pub fn encode_bytes(&self) -> Vec<u8> {
        self.raw().encode()
    }

    /// Закодировать настройки в base64-строку.
    #[must_use]
    pub fn encode_base64(&self) -> String {
        base64::encode(&self.encode_bytes())
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

/// Ошибка HTTP-операций с settings-proto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsProtoHttpError {
    /// Неподдерживаемый тип proto (допустимы только 1 и 2).
    UnsupportedProtoType(u64),
    /// Ошибка wire-декодера или base64.
    Proto(SettingsProtoError),
    /// Невалидный формат ответа или JSON.
    InvalidResponse(String),
    /// 401 Unauthorized.
    Unauthorized,
    /// 429 Rate limited.
    RateLimited(RateLimitInfo),
    /// 5xx Server error.
    Server { status: u16 },
    /// 4xx Client error.
    Client { status: u16 },
}

impl fmt::Display for SettingsProtoHttpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedProtoType(proto_type) => {
                write!(formatter, "unsupported settings proto type {proto_type}")
            }
            Self::Proto(error) => write!(formatter, "proto error: {error}"),
            Self::InvalidResponse(reason) => write!(formatter, "invalid response: {reason}"),
            Self::Unauthorized => write!(formatter, "unauthorized (401)"),
            Self::RateLimited(info) => {
                write!(
                    formatter,
                    "rate limited (429): retry after {:?}",
                    info.retry_after
                )
            }
            Self::Server { status } => write!(formatter, "server error ({status})"),
            Self::Client { status } => write!(formatter, "client error ({status})"),
        }
    }
}

impl std::error::Error for SettingsProtoHttpError {}

impl From<SettingsProtoError> for SettingsProtoHttpError {
    fn from(error: SettingsProtoError) -> Self {
        Self::Proto(error)
    }
}

impl From<Base64Error> for SettingsProtoHttpError {
    fn from(error: Base64Error) -> Self {
        Self::Proto(SettingsProtoError::Base64(error))
    }
}

/// Сформировать путь REST endpoint для `settings-proto/{proto_type}`.
///
/// Допустимы только типы 1 (`PRELOADED_USER_SETTINGS_TYPE`) и 2 (`FRECENCY_AND_FAVORITES_SETTINGS_TYPE`).
/// Любой иной тип отвергается ошибкой [`SettingsProtoHttpError::UnsupportedProtoType`] до сетевого запроса.
///
/// # Errors
///
/// Возвращает [`SettingsProtoHttpError::UnsupportedProtoType`], если `proto_type` не равен 1 или 2.
pub fn settings_proto_path(proto_type: u64) -> Result<String, SettingsProtoHttpError> {
    match proto_type {
        PRELOADED_USER_SETTINGS_TYPE | FRECENCY_AND_FAVORITES_SETTINGS_TYPE => {
            Ok(format!("/users/@me/settings-proto/{proto_type}"))
        }
        other => Err(SettingsProtoHttpError::UnsupportedProtoType(other)),
    }
}

/// Построить `GET /users/@me/settings-proto/{proto_type}` запрос.
///
/// # Errors
///
/// Возвращает [`SettingsProtoHttpError::UnsupportedProtoType`], если `proto_type` не равен 1 или 2.
pub fn build_get_settings_proto_request(
    proto_type: u64,
) -> Result<HttpRequest, SettingsProtoHttpError> {
    let path = settings_proto_path(proto_type)?;
    Ok(HttpRequest::new(HttpMethod::Get, path))
}

/// Разобрать ответ от endpoint'а `settings-proto`.
///
/// На 200 OK разбирает JSON вида `{"settings": "<base64>"}` в [`SettingsProto`].
/// На ошибки:
/// - 401 -> [`SettingsProtoHttpError::Unauthorized`]
/// - 429 -> [`SettingsProtoHttpError::RateLimited`]
/// - 5xx -> [`SettingsProtoHttpError::Server`]
/// - 4xx -> [`SettingsProtoHttpError::Client`]
///
/// # Errors
///
/// Возвращает [`SettingsProtoHttpError`] при ошибке статуса, формата ответа или декодирования protobuf.
pub fn parse_settings_proto_response(
    proto_type: u64,
    status: u16,
    headers: &[(String, String)],
    body: &[u8],
) -> Result<SettingsProto, SettingsProtoHttpError> {
    match status {
        200 => {
            let text = std::str::from_utf8(body)
                .map_err(|err| SettingsProtoHttpError::InvalidResponse(err.to_string()))?;
            let json = chord_json::parse(text)
                .map_err(|err| SettingsProtoHttpError::InvalidResponse(err.to_string()))?;
            let b64_str = json
                .get("settings")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| {
                    SettingsProtoHttpError::InvalidResponse(
                        "missing 'settings' string field".to_string(),
                    )
                })?;
            let proto = decode_settings(proto_type, b64_str)?;
            Ok(proto)
        }
        401 => Err(SettingsProtoHttpError::Unauthorized),
        429 => {
            let info = rate_limit_info(status, headers, body).unwrap_or_default();
            Err(SettingsProtoHttpError::RateLimited(info))
        }
        500..=599 => Err(SettingsProtoHttpError::Server { status }),
        400..=499 => Err(SettingsProtoHttpError::Client { status }),
        other => Err(SettingsProtoHttpError::Client { status: other }),
    }
}

/// Построить JSON-тело для `PATCH /users/@me/settings-proto/{type}`.
///
/// Сначала валидирует base64 и protobuf-декодирование переданной строки `base64_settings`.
/// Затем сериализует JSON:
/// - без версии: `{"settings":"..."}`
/// - с версией: `{"required_data_version":N,"settings":"..."}`
///
/// # Errors
///
/// Возвращает [`SettingsProtoHttpError::Proto`], если `base64_settings` не является валидным base64
/// или не может быть разобран как валидное protobuf-сообщение.
pub fn build_patch_settings_proto_body(
    base64_settings: &str,
    required_data_version: Option<u64>,
) -> Result<Vec<u8>, SettingsProtoHttpError> {
    // Валидация base64 + protobuf decode до формирования тела
    let raw_bytes = base64::decode(base64_settings)?;
    ProtoMessage::decode(&raw_bytes)?;

    let json_text = match required_data_version {
        Some(version) => {
            format!("{{\"required_data_version\":{version},\"settings\":\"{base64_settings}\"}}")
        }
        None => format!("{{\"settings\":\"{base64_settings}\"}}"),
    };
    Ok(json_text.into_bytes())
}

/// Построить `PATCH /users/@me/settings-proto/{proto_type}` запрос.
///
/// # Errors
///
/// Возвращает [`SettingsProtoHttpError`], если `proto_type` не поддержан
/// или `base64_settings` не проходит валидацию base64/protobuf.
pub fn build_patch_settings_proto_request(
    proto_type: u64,
    base64_settings: &str,
    required_data_version: Option<u64>,
) -> Result<HttpRequest, SettingsProtoHttpError> {
    let path = settings_proto_path(proto_type)?;
    let body = build_patch_settings_proto_body(base64_settings, required_data_version)?;
    Ok(HttpRequest::new(HttpMethod::Patch, path)
        .header("content-type", "application/json")
        .body(body))
}
