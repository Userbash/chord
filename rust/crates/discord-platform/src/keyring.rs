//! P1-10: хранение токена в Secret Service (`org.freedesktop.secrets`) через libsecret.
//!
//! Безопасный API поверх [`crate::ffi_secret`] (единственное место с `unsafe`):
//! [`Keyring::connect`] поднимает default-сервис сессионной шины, `store`/`lookup`/`clear`
//! работают с одной схемой `com.discordlab.Token` и строковым атрибутом `key`. Метка записи —
//! `discord-lab token`, коллекция — default (`collection = NULL`).
//!
//! Инварианты (TZ W33a §1):
//!
//! - пустой ключ и NUL в ключе → [`KeyringError::InvalidKey`]; NUL в секрете →
//!   [`KeyringError::InvalidSecret`]; невалидный UTF-8 из сервиса → [`KeyringError::Encoding`];
//!   ошибка glib/libsecret → [`KeyringError::Secret`] с кодом и сообщением (без секрета).
//! - `lookup` отсутствующей записи → `Ok(None)`; `clear` отсутствующей → `Ok(false)`,
//!   повторный `clear` — тоже `Ok(false)`.
//! - [`Keyring`] — `!Send + !Sync` (libsecret/GDBus-сервис не переносится между потоками);
//!   `Service`/`Schema` освобождаются ровно один раз при разрушении `Keyring`.
//! - Ручной [`std::fmt::Debug`] печатает только имя схемы и метку: значение секрета и
//!   внутренние указатели не выводятся и не логируются.
//!
//! Ограничения (осознанные):
//!
//! - Проверка live-кейсов — реальный ksecretd на сессионной шине и CLI `secret-tool`;
//!   секреты синтетические. Если коллекция заблокирована, синхронный вызов может ждать
//!   разблокировки (prompt) — live-кейсы запускаются только по `DISCORD_LAB_KEYRING_LIVE=1`.
//! - Wire-совместимость с крейтом `keyring` не заявляется: свой формат атрибутов
//!   (`key`, схема `com.discordlab.Token`), без миграции чужих записей.

use std::fmt;
use std::marker::PhantomData;

use crate::ffi_secret::{FfiError, Schema, Service};

/// Имя схемы записей (TZ W33a §1).
pub const SCHEMA_NAME: &str = "com.discordlab.Token";
/// Строковый атрибут, по которому ищется запись.
const ATTRIBUTE_KEY: &str = "key";
/// Метка записи в Secret Service.
const LABEL: &str = "discord-lab token";

/// Ошибка keyring. Не содержит значения секрета.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyringError {
    /// Пустой ключ или NUL-байт в ключе.
    InvalidKey,
    /// NUL-байт в секрете (не представим C-строкой).
    InvalidSecret,
    /// Сервис вернул значение, которое не является UTF-8.
    Encoding,
    /// Ошибка glib/libsecret: код `GError` (0 — ошибка без `GError`) и сообщение.
    Secret { code: i32, message: String },
}

impl fmt::Display for KeyringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKey => formatter.write_str("пустой ключ или NUL-байт в ключе"),
            Self::InvalidSecret => formatter.write_str("NUL-байт в секрете"),
            Self::Encoding => formatter.write_str("значение из Secret Service не является UTF-8"),
            Self::Secret { code, message } => {
                write!(formatter, "Secret Service (код {code}): {message}")
            }
        }
    }
}

impl std::error::Error for KeyringError {}

impl From<FfiError> for KeyringError {
    fn from(error: FfiError) -> Self {
        Self::Secret {
            code: error.code,
            message: error.message,
        }
    }
}

/// Клиент Secret Service с default-коллекцией. Однопоточный: `!Send + !Sync`.
pub struct Keyring {
    service: Service,
    schema: Schema,
    /// Явная фиксация `!Send + !Sync` независимо от состава полей.
    _not_send_sync: PhantomData<*mut ()>,
}

impl fmt::Debug for Keyring {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Keyring")
            .field("schema", &SCHEMA_NAME)
            .field("label", &LABEL)
            .finish_non_exhaustive()
    }
}

impl Keyring {
    /// Подключение к default-сервису через `secret_service_get_sync`.
    pub fn connect() -> Result<Self, KeyringError> {
        let service = Service::connect()?;
        let schema = Schema::new(SCHEMA_NAME, ATTRIBUTE_KEY)?;
        Ok(Self {
            service,
            schema,
            _not_send_sync: PhantomData,
        })
    }

    /// Записывает секрет по ключу (перезапись существующей записи — обновление значения).
    pub fn store(&mut self, key: &str, secret: &str) -> Result<(), KeyringError> {
        validate_key(key)?;
        if secret.contains('\0') {
            return Err(KeyringError::InvalidSecret);
        }
        self.service
            .store(&self.schema, ATTRIBUTE_KEY, key, LABEL, secret)
            .map_err(KeyringError::from)
    }

    /// Читает секрет по ключу; отсутствие записи — `Ok(None)`.
    pub fn lookup(&mut self, key: &str) -> Result<Option<String>, KeyringError> {
        validate_key(key)?;
        let Some(bytes) = self
            .service
            .lookup(&self.schema, ATTRIBUTE_KEY, key)
            .map_err(KeyringError::from)?
        else {
            return Ok(None);
        };
        String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| KeyringError::Encoding)
    }

    /// Удаляет запись по ключу: `true` — запись была и удалена, `false` — записи не было.
    pub fn clear(&mut self, key: &str) -> Result<bool, KeyringError> {
        validate_key(key)?;
        self.service
            .clear(&self.schema, ATTRIBUTE_KEY, key)
            .map_err(KeyringError::from)
    }
}

/// Ключ должен быть непустой C-строкой: пустой ключ бессмысленен, NUL не представим в C.
fn validate_key(key: &str) -> Result<(), KeyringError> {
    if key.is_empty() || key.contains('\0') {
        return Err(KeyringError::InvalidKey);
    }
    Ok(())
}
