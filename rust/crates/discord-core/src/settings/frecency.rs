//! P2-08: `FrecencyUserSettings` (type 2) — structural wrapper (RW-21).
//!
//! В захватах `USER_SETTINGS_PROTO_UPDATE` приходит `settings.type=2`; это **не**
//! `PreloadedUserSettings`, а отдельная схема favorite/frecency. P2-08 сохраняет её
//! top-level поля и версии структурно, не выдумывая семантику map-значений.

use std::fmt;

use super::preloaded::Versions;
use super::wire::{ProtoMessage, SettingsProtoError};

/// `FrecencyUserSettings` из `USER_SETTINGS_PROTO_UPDATE` (type 2).
#[derive(Clone, PartialEq, Eq)]
pub struct FrecencyUserSettings {
    /// Полное top-level сообщение; вложенные map/frecency-структуры не типизированы.
    pub raw: ProtoMessage,
    /// Версии схемы.
    pub versions: Option<Versions>,
}

impl FrecencyUserSettings {
    /// Разобрать bytes `FrecencyUserSettings`.
    pub fn decode(bytes: &[u8]) -> Result<Self, SettingsProtoError> {
        let raw = ProtoMessage::decode(bytes)?;
        let versions = raw
            .message(1)?
            .map(|message| Versions::from_message(&message))
            .transpose()?;
        Ok(Self { raw, versions })
    }
}

impl fmt::Debug for FrecencyUserSettings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FrecencyUserSettings")
            .field("raw_fields", &self.raw.fields.len())
            .field("versions", &self.versions.is_some())
            .finish()
    }
}
