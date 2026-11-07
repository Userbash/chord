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
/// Префикс токена, зашифрованного через Electron safeStorage (BE-P3-08, RE/rest-static.md §4).
pub const ENCRYPTED_TOKEN_PREFIX: &str = "dQw4w9WgXcQ:";
/// Строковый атрибут, по которому ищется запись.
const ATTRIBUTE_KEY: &str = "key";
/// Метка записи в Secret Service.
const LABEL: &str = "discord-lab token";

/// Ошибка миграции токена из Electron safeStorage (BE-P3-08).
///
/// Инвариант безопасности: `Debug` и `Display` никогда не выводят сырой токен,
/// расшифрованный секрет или base64 ciphertext.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SafeStorageMigrationError {
    /// Пустой зашифрованный payload (нет base64 данных).
    EmptyPayload,
    /// Некорректный base64 в зашифрованном токене.
    InvalidBase64,
    /// Ошибка расшифровки (сбой платформенного провайдера crypto/safeStorage).
    DecryptFailed,
    /// Расшифрованный токен невалиден (пустой, не UTF-8 или содержит NUL/переводы строк).
    InvalidToken,
    /// Токен уже был мигрирован ранее.
    AlreadyMigrated,
    /// Ошибка сохранения токена в целевой keyring.
    Storage(KeyringError),
}

impl fmt::Display for SafeStorageMigrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPayload => formatter.write_str("пустой зашифрованный payload safeStorage"),
            Self::InvalidBase64 => formatter.write_str("некорректный base64 в safeStorage токене"),
            Self::DecryptFailed => formatter.write_str("ошибка расшифровки safeStorage токена"),
            Self::InvalidToken => {
                formatter.write_str("невалидный токен после расшифровки (пустой, не UTF-8 или содержит NUL/переводы строк)")
            }
            Self::AlreadyMigrated => formatter.write_str("миграция safeStorage токена уже была выполнена"),
            Self::Storage(error) => write!(formatter, "ошибка записи токена в keyring: {error}"),
        }
    }
}

impl std::error::Error for SafeStorageMigrationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Storage(error) => Some(error),
            _ => None,
        }
    }
}

impl From<KeyringError> for SafeStorageMigrationError {
    fn from(error: KeyringError) -> Self {
        Self::Storage(error)
    }
}

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

    /// Миграция токена из safeStorage Electron (BE-P3-08).
    pub fn migrate_safe_storage<D>(
        &mut self,
        migrator: &mut SafeStorageMigrator,
        key: &str,
        raw_entry: &str,
        decrypt: D,
    ) -> Result<bool, SafeStorageMigrationError>
    where
        D: FnOnce(&[u8]) -> Result<Vec<u8>, SafeStorageMigrationError>,
    {
        migrator.migrate_keyring(self, key, raw_entry, decrypt)
    }
}

/// Проверяет, является ли строка зашифрованным токеном Electron safeStorage
/// (начинается ли после trim с префикса [`ENCRYPTED_TOKEN_PREFIX`]).
#[must_use]
pub fn is_safe_storage_token(raw: &str) -> bool {
    raw.trim().starts_with(ENCRYPTED_TOKEN_PREFIX)
}

/// Извлекает зашифрованный ciphertext из строки токена safeStorage:
/// срезает префикс [`ENCRYPTED_TOKEN_PREFIX`], декодирует base64 и отклоняет
/// пустой или невалидный payload.
pub fn extract_safe_storage_ciphertext(raw: &str) -> Result<Vec<u8>, SafeStorageMigrationError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(SafeStorageMigrationError::EmptyPayload);
    }
    let payload = match trimmed.strip_prefix(ENCRYPTED_TOKEN_PREFIX) {
        Some(rest) => rest.trim(),
        None => return Err(SafeStorageMigrationError::InvalidBase64),
    };
    if payload.is_empty() {
        return Err(SafeStorageMigrationError::EmptyPayload);
    }
    decode_base64(payload)
}

/// Декодирует base64 ciphertext safeStorage.
fn decode_base64(input: &str) -> Result<Vec<u8>, SafeStorageMigrationError> {
    let bytes = input.as_bytes();
    if bytes.is_empty() {
        return Err(SafeStorageMigrationError::EmptyPayload);
    }

    let pad_count = bytes.iter().rev().take_while(|&&b| b == b'=').count();
    if pad_count > 2 {
        return Err(SafeStorageMigrationError::InvalidBase64);
    }
    let data_bytes = &bytes[..bytes.len() - pad_count];
    if data_bytes.contains(&b'=') {
        return Err(SafeStorageMigrationError::InvalidBase64);
    }
    if pad_count > 0 && !bytes.len().is_multiple_of(4) {
        return Err(SafeStorageMigrationError::InvalidBase64);
    }
    if data_bytes.len() % 4 == 1 {
        return Err(SafeStorageMigrationError::InvalidBase64);
    }

    let mut out = Vec::with_capacity((bytes.len() / 4 + 1) * 3);
    let mut accum = 0u32;
    let mut bits = 0u32;

    for &b in data_bytes {
        let val = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return Err(SafeStorageMigrationError::InvalidBase64),
        };
        accum = (accum << 6) | u32::from(val);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((accum >> bits) as u8);
        }
    }

    if out.is_empty() {
        return Err(SafeStorageMigrationError::EmptyPayload);
    }

    Ok(out)
}

/// Конечный автомат одноразовой миграции токена из Electron safeStorage (BE-P3-08).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SafeStorageMigrator {
    migrated: bool,
}

impl SafeStorageMigrator {
    /// Создаёт новый экземпляр мигратора в исходном (не мигрированном) состоянии.
    #[must_use]
    pub const fn new() -> Self {
        Self { migrated: false }
    }

    /// Возвращает `true`, если миграция уже была успешно выполнена или зафиксирована.
    #[must_use]
    pub fn is_migrated(&self) -> bool {
        self.migrated
    }

    /// Помечает мигратор как завершённый.
    pub fn mark_migrated(&mut self) {
        self.migrated = true;
    }

    /// Сбрасывает флаг миграции.
    pub fn reset(&mut self) {
        self.migrated = false;
    }

    /// Выполняет одноразовую миграцию токена.
    ///
    /// Если миграция уже была выполнена (`self.migrated`) или целевое хранилище уже
    /// содержит токен (store вернул `false`), повторный импорт не производится и
    /// возвращается `Ok(false)`.
    ///
    /// Если `raw_entry` начинается с `"dQw4w9WgXcQ:"`, декодирует base64, вызывает
    /// `decrypt(ciphertext)`, валидирует непустой UTF-8 без NUL/переводов строк,
    /// сохраняет через `store(token)`, помечает миграцию завершённой и возвращает `Ok(true)`.
    /// Если `raw_entry` не является safeStorage токеном, возвращает `Ok(false)`.
    pub fn migrate_once<D, S>(
        &mut self,
        raw_entry: &str,
        decrypt: D,
        store: S,
    ) -> Result<bool, SafeStorageMigrationError>
    where
        D: FnOnce(&[u8]) -> Result<Vec<u8>, SafeStorageMigrationError>,
        S: FnOnce(&str) -> Result<bool, SafeStorageMigrationError>,
    {
        if self.migrated {
            return Ok(false);
        }
        if !is_safe_storage_token(raw_entry) {
            return Ok(false);
        }
        let ciphertext = extract_safe_storage_ciphertext(raw_entry)?;
        let decrypted_bytes = decrypt(&ciphertext)?;
        let token = std::str::from_utf8(&decrypted_bytes)
            .map_err(|_| SafeStorageMigrationError::InvalidToken)?;
        if token.is_empty() || token.contains(['\0', '\n', '\r']) {
            return Err(SafeStorageMigrationError::InvalidToken);
        }
        let stored = store(token)?;
        if stored {
            self.migrated = true;
            Ok(true)
        } else {
            self.migrated = true;
            Ok(false)
        }
    }

    /// Выполняет миграцию в целевой [`Keyring`]. Если запись уже есть в keyring или мигратор
    /// уже отработал, не производит повторный импорт и возвращает `Ok(false)`.
    pub fn migrate_keyring<D>(
        &mut self,
        keyring: &mut Keyring,
        key: &str,
        raw_entry: &str,
        decrypt: D,
    ) -> Result<bool, SafeStorageMigrationError>
    where
        D: FnOnce(&[u8]) -> Result<Vec<u8>, SafeStorageMigrationError>,
    {
        if self.migrated {
            return Ok(false);
        }
        if keyring.lookup(key)?.is_some() {
            self.migrated = true;
            return Ok(false);
        }
        self.migrate_once(raw_entry, decrypt, |token| {
            keyring.store(key, token)?;
            Ok(true)
        })
    }
}

/// Ключ должен быть непустой C-строкой: пустой ключ бессмысленен, NUL не представим в C.
fn validate_key(key: &str) -> Result<(), KeyringError> {
    if key.is_empty() || key.contains('\0') {
        return Err(KeyringError::InvalidKey);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn k13_safe_storage_prefix_and_detection() {
        assert_eq!(ENCRYPTED_TOKEN_PREFIX, "dQw4w9WgXcQ:");
        assert!(is_safe_storage_token("dQw4w9WgXcQ:aGVsbG8="));
        assert!(is_safe_storage_token("  dQw4w9WgXcQ:aGVsbG8=  \n"));
        assert!(is_safe_storage_token("dQw4w9WgXcQ:"));
        assert!(!is_safe_storage_token("mfa.synthetic_plaintext_token_123"));
        assert!(!is_safe_storage_token(""));
        assert!(!is_safe_storage_token("   \t\n"));
    }

    #[test]
    fn k14_extract_safe_storage_ciphertext_decodes_base64() {
        let raw = "dQw4w9WgXcQ:aGVsbG8gd29ybGQ=";
        let bytes = extract_safe_storage_ciphertext(raw).expect("valid base64 extraction");
        assert_eq!(bytes, b"hello world");

        let padded = "  dQw4w9WgXcQ:YQ== \n";
        let bytes = extract_safe_storage_ciphertext(padded).expect("valid single byte");
        assert_eq!(bytes, b"a");

        let binary = "dQw4w9WgXcQ:/+7z";
        let bytes = extract_safe_storage_ciphertext(binary).expect("binary base64");
        assert_eq!(bytes, vec![0xff, 0xee, 0xf3]);
    }

    #[test]
    fn k15_extract_safe_storage_ciphertext_rejects_empty_payload() {
        assert_eq!(
            extract_safe_storage_ciphertext("dQw4w9WgXcQ:"),
            Err(SafeStorageMigrationError::EmptyPayload)
        );
        assert_eq!(
            extract_safe_storage_ciphertext("   dQw4w9WgXcQ:   \n"),
            Err(SafeStorageMigrationError::EmptyPayload)
        );
        assert_eq!(
            extract_safe_storage_ciphertext(""),
            Err(SafeStorageMigrationError::EmptyPayload)
        );
        assert_eq!(
            extract_safe_storage_ciphertext("   "),
            Err(SafeStorageMigrationError::EmptyPayload)
        );
    }

    #[test]
    fn k16_extract_safe_storage_ciphertext_rejects_invalid_base64() {
        assert_eq!(
            extract_safe_storage_ciphertext("dQw4w9WgXcQ:not-valid-base64!"),
            Err(SafeStorageMigrationError::InvalidBase64)
        );
        assert_eq!(
            extract_safe_storage_ciphertext("dQw4w9WgXcQ:A"),
            Err(SafeStorageMigrationError::InvalidBase64)
        );
        assert_eq!(
            extract_safe_storage_ciphertext("dQw4w9WgXcQ:A==="),
            Err(SafeStorageMigrationError::InvalidBase64)
        );
        assert_eq!(
            extract_safe_storage_ciphertext("no_prefix_payload="),
            Err(SafeStorageMigrationError::InvalidBase64)
        );
    }

    #[test]
    fn k17_migration_errors_do_not_leak_secrets() {
        let secret_payload = "super-secret-raw-token-data";
        let errors = [
            SafeStorageMigrationError::EmptyPayload,
            SafeStorageMigrationError::InvalidBase64,
            SafeStorageMigrationError::DecryptFailed,
            SafeStorageMigrationError::InvalidToken,
            SafeStorageMigrationError::AlreadyMigrated,
            SafeStorageMigrationError::Storage(KeyringError::InvalidSecret),
        ];

        for error in &errors {
            let debug = format!("{error:?}");
            let display = error.to_string();
            assert!(
                !debug.contains(secret_payload),
                "Debug ошибки содержит секрет: {debug}"
            );
            assert!(
                !display.contains(secret_payload),
                "Display ошибки содержит секрет: {display}"
            );
            assert!(
                !debug.contains("0x"),
                "Debug ошибки содержит внутренние указатели: {debug}"
            );
        }
    }

    #[test]
    fn k18_migrator_happy_path_migrates_once() {
        let mut migrator = SafeStorageMigrator::new();
        assert!(!migrator.is_migrated());

        let raw = "dQw4w9WgXcQ:Y2lwaGVydGV4dC1ieXRlcw==";
        let mut stored_val = None;

        let result = migrator.migrate_once(
            raw,
            |ct| {
                assert_eq!(ct, b"ciphertext-bytes");
                Ok(b"decrypted-mfa-token-12345".to_vec())
            },
            |token| {
                stored_val = Some(token.to_owned());
                Ok(true)
            },
        );

        assert_eq!(result, Ok(true));
        assert!(migrator.is_migrated());
        assert_eq!(stored_val.as_deref(), Some("decrypted-mfa-token-12345"));
    }

    #[test]
    fn k19_migrator_idempotent_no_reimport() {
        let mut migrator = SafeStorageMigrator::new();
        let raw = "dQw4w9WgXcQ:Y2lwaGVydGV4dA==";

        let mut decrypt_calls = 0;
        let mut store_calls = 0;

        let res1 = migrator.migrate_once(
            raw,
            |_ct| {
                decrypt_calls += 1;
                Ok(b"valid-token".to_vec())
            },
            |_token| {
                store_calls += 1;
                Ok(true)
            },
        );
        assert_eq!(res1, Ok(true));
        assert_eq!(decrypt_calls, 1);
        assert_eq!(store_calls, 1);

        let res2 = migrator.migrate_once(
            raw,
            |_ct| {
                decrypt_calls += 1;
                Ok(b"second-token".to_vec())
            },
            |_token| {
                store_calls += 1;
                Ok(true)
            },
        );
        assert_eq!(res2, Ok(false));
        assert_eq!(decrypt_calls, 1, "decrypt не должен вызываться повторно");
        assert_eq!(store_calls, 1, "store не должен вызываться повторно");
    }

    #[test]
    fn k20_migrator_skips_when_target_already_has_token() {
        let mut migrator = SafeStorageMigrator::new();
        let raw = "dQw4w9WgXcQ:Y2lwaGVydGV4dA==";

        let res = migrator.migrate_once(
            raw,
            |_ct| Ok(b"token-from-safestorage".to_vec()),
            |_token| Ok(false),
        );

        assert_eq!(res, Ok(false));
        assert!(migrator.is_migrated());
    }

    #[test]
    fn k21_migrator_skips_non_safe_storage_token() {
        let mut migrator = SafeStorageMigrator::new();
        let mut decrypt_called = false;
        let mut store_called = false;

        let res = migrator.migrate_once(
            "regular-unencrypted-token",
            |_ct| {
                decrypt_called = true;
                Ok(vec![])
            },
            |_tok| {
                store_called = true;
                Ok(true)
            },
        );

        assert_eq!(res, Ok(false));
        assert!(!decrypt_called);
        assert!(!store_called);
    }

    #[test]
    fn k22_migrator_fails_on_decrypt_error() {
        let mut migrator = SafeStorageMigrator::new();
        let raw = "dQw4w9WgXcQ:Y2lwaGVydGV4dA==";
        let mut store_called = false;

        let res = migrator.migrate_once(
            raw,
            |_ct| Err(SafeStorageMigrationError::DecryptFailed),
            |_tok| {
                store_called = true;
                Ok(true)
            },
        );

        assert_eq!(res, Err(SafeStorageMigrationError::DecryptFailed));
        assert!(!store_called);
        assert!(
            !migrator.is_migrated(),
            "migrated state stays false on error"
        );
    }

    #[test]
    fn k23_migrator_rejects_invalid_tokens() {
        let mut migrator = SafeStorageMigrator::new();
        let raw = "dQw4w9WgXcQ:Y2lwaGVydGV4dA==";

        let res_empty = migrator.migrate_once(raw, |_ct| Ok(b"".to_vec()), |_tok| Ok(true));
        assert_eq!(res_empty, Err(SafeStorageMigrationError::InvalidToken));

        let res_utf8 = migrator.migrate_once(raw, |_ct| Ok(vec![0xff, 0xfe, 0xfd]), |_tok| Ok(true));
        assert_eq!(res_utf8, Err(SafeStorageMigrationError::InvalidToken));

        let res_nul = migrator.migrate_once(raw, |_ct| Ok(b"token\0corrupt".to_vec()), |_tok| Ok(true));
        assert_eq!(res_nul, Err(SafeStorageMigrationError::InvalidToken));

        let res_nl = migrator.migrate_once(raw, |_ct| Ok(b"token\nnewline".to_vec()), |_tok| Ok(true));
        assert_eq!(res_nl, Err(SafeStorageMigrationError::InvalidToken));

        let res_cr = migrator.migrate_once(raw, |_ct| Ok(b"token\rcarriage".to_vec()), |_tok| Ok(true));
        assert_eq!(res_cr, Err(SafeStorageMigrationError::InvalidToken));
    }

    #[test]
    fn k24_migrator_propagates_keyring_storage_error() {
        let mut migrator = SafeStorageMigrator::new();
        let raw = "dQw4w9WgXcQ:Y2lwaGVydGV4dA==";

        let res = migrator.migrate_once(
            raw,
            |_ct| Ok(b"valid-token".to_vec()),
            |_tok| Err(KeyringError::InvalidSecret.into()),
        );

        assert_eq!(
            res,
            Err(SafeStorageMigrationError::Storage(
                KeyringError::InvalidSecret
            ))
        );
        assert!(!migrator.is_migrated());
    }

    #[test]
    fn k26_migration_failure_paths_and_state_do_not_leak_secrets() {
        let raw = "dQw4w9WgXcQ:c2VjcmV0LWNpcGhlcnRleHQ=";
        let secret_token = "mfa.super_secret_token_value_123";

        let mut migrator = SafeStorageMigrator::new();
        let err = migrator
            .migrate_once(
                raw,
                |_| Err(SafeStorageMigrationError::DecryptFailed),
                |_| Ok(true),
            )
            .unwrap_err();
        for text in [format!("{err:?}"), err.to_string()] {
            assert!(
                !text.contains("c2VjcmV0")
                    && !text.contains(raw)
                    && !text.contains("secret-ciphertext")
            );
        }

        let mut migrator = SafeStorageMigrator::new();
        let err = migrator
            .migrate_once(
                raw,
                |_| Ok(format!("{secret_token}\0").into_bytes()),
                |_| Ok(true),
            )
            .unwrap_err();
        for text in [format!("{err:?}"), err.to_string()] {
            assert!(!text.contains(secret_token) && !text.contains(raw) && !text.contains("c2VjcmV0"));
        }

        let mut migrator = SafeStorageMigrator::new();
        let err = migrator
            .migrate_once(
                raw,
                |_| Ok(b"valid-token".to_vec()),
                |_| Err(KeyringError::InvalidSecret.into()),
            )
            .unwrap_err();
        for text in [format!("{err:?}"), err.to_string()] {
            assert!(!text.contains("c2VjcmV0") && !text.contains(raw) && !text.contains("valid-token"));
        }

        let mut migrator = SafeStorageMigrator::new();
        assert_eq!(
            migrator.migrate_once(raw, |_| Ok(b"valid-token".to_vec()), |_| Ok(true)),
            Ok(true)
        );
        let debug = format!("{migrator:?}");
        assert!(
            !debug.contains("c2VjcmV0") && !debug.contains(raw),
            "Debug мигратора содержит ciphertext: {debug}"
        );
    }
}
