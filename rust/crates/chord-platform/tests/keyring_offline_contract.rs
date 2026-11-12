//! Офлайн-часть контракта keyring (PLAT-02): то, что доказуемо без `org.freedesktop.secrets`.
//!
//! Live-кейсы k01–k12 лежат в `keyring_contract.rs` и помечены `#[ignore]`; здесь — только
//! инварианты, не требующие [`Keyring`]: redaction ошибок [`KeyringError`] и значение схемы.
//! Файл не заменяет live-набор, а держит security-инвариант под гейтом, даже когда live-окна нет.
//!
//! Оракул — контракт из `docs/re` и TZ W33a §1 (имя схемы, отсутствие значения секрета в
//! `Debug`/`Display`), а не текущая реализация.

use chord_platform::keyring::{KeyringError, SCHEMA_NAME};

/// Синтетический секрет, которого не должно быть ни в одном сообщении об ошибке.
const SECRET: &str = "test-secret-offline-do-not-leak";

#[test]
fn kr01_validation_errors_carry_no_secret_value() {
    for error in [KeyringError::InvalidKey, KeyringError::InvalidSecret] {
        let debug = format!("{error:?}");
        let display = error.to_string();
        assert!(
            !debug.contains(SECRET),
            "Debug ошибки содержит секрет: {debug}"
        );
        assert!(
            !display.contains(SECRET),
            "Display ошибки содержит секрет: {display}"
        );
        assert!(
            !debug.contains("0x"),
            "Debug ошибки содержит указатель: {debug}"
        );
        assert!(
            !display.contains("0x"),
            "Display ошибки содержит указатель: {display}"
        );
    }
    // Тексты — часть контракта: они попадают в логи и не должны содержать данных записи.
    assert_eq!(
        KeyringError::InvalidKey.to_string(),
        "пустой ключ или NUL-байт в ключе"
    );
    assert_eq!(
        KeyringError::InvalidSecret.to_string(),
        "NUL-байт в секрете"
    );
}

#[test]
fn kr02_schema_name_is_frozen() {
    // Имя схемы — wire-контракт с `secret-tool --xdp:schema` и чужими читателями коллекции.
    assert_eq!(SCHEMA_NAME, "com.discordlab.Token");
    assert!(!SCHEMA_NAME.is_empty());
    assert!(!SCHEMA_NAME.contains('\0'));
}

#[test]
fn kr03_secret_error_keeps_code_and_message_only() {
    // `From<FfiError>` даёт ровно эту форму (`Secret { code, message }`); сам `FfiError` —
    // `pub(crate)` (API заморожен PLAT-02), поэтому конверсия проверяется через публичный
    // вариант: в сообщении обязаны остаться код и текст, но не значение секрета.
    let error = KeyringError::Secret {
        code: 42,
        message: "org.freedesktop.Secret.Error.IsLocked".to_owned(),
    };
    let debug = format!("{error:?}");
    let display = error.to_string();
    assert!(
        debug.contains("code: 42"),
        "Debug потерял код ошибки: {debug}"
    );
    assert!(
        debug.contains("IsLocked"),
        "Debug потерял сообщение сервиса: {debug}"
    );
    assert!(
        display.contains("42"),
        "Display потерял код ошибки: {display}"
    );
    assert!(
        display.contains("IsLocked"),
        "Display потерял сообщение сервиса: {display}"
    );
    for text in [&debug, &display] {
        assert!(
            !text.contains(SECRET),
            "ошибка сервиса содержит секрет: {text}"
        );
        assert!(
            !text.contains("0x"),
            "ошибка сервиса содержит указатель: {text}"
        );
    }
    // Ошибка — обычный `std::error::Error`: source отсутствует, цепочка не тянет данные.
    let as_error: &dyn std::error::Error = &error;
    assert!(
        as_error.source().is_none(),
        "у KeyringError не должно быть source"
    );
}
