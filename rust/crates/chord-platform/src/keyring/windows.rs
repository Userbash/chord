//! Windows Credential Manager adapter. Explicit WinCredential construction
//! avoids keyring's process-global default builder and its mock fallback.
//! Binary secrets retain the public UTF-8 validation contract; no UTF-16
//! conversion or format is shared with the existing Unix Secret Service schema.
use super::{KeyringError, SCHEMA_NAME};
use keyring::{Entry, Error, windows::WinCredential};

fn entry(key: &str) -> Result<Entry, KeyringError> {
    WinCredential::new_with_target(None, SCHEMA_NAME, key)
        .map(|credential| Entry::new_with_credential(Box::new(credential)))
        .map_err(map_error)
}

pub(super) fn store(key: &str, secret: &str) -> Result<(), KeyringError> {
    entry(key)?.set_secret(secret.as_bytes()).map_err(map_error)
}

pub(super) fn lookup(key: &str) -> Result<Option<Vec<u8>>, KeyringError> {
    match entry(key)?.get_secret() {
        Ok(secret) => Ok(Some(secret)),
        Err(Error::NoEntry) => Ok(None),
        Err(error) => Err(map_error(error)),
    }
}

pub(super) fn clear(key: &str) -> Result<bool, KeyringError> {
    match entry(key)?.delete_credential() {
        Ok(()) => Ok(true),
        Err(Error::NoEntry) => Ok(false),
        Err(error) => Err(map_error(error)),
    }
}

fn map_error(error: Error) -> KeyringError {
    // Some provider errors contain raw credential bytes or credential objects.
    // Never format those errors into public Debug/Display diagnostics.
    let message = match error {
        Error::BadEncoding(_) => return KeyringError::Encoding,
        Error::NoStorageAccess(_) => "Windows Credential Manager access denied",
        Error::PlatformFailure(_) => "Windows Credential Manager operation failed",
        Error::TooLong(_, _) => "Windows credential field exceeds the native length limit",
        Error::Invalid(_, _) => "Windows credential field is invalid",
        Error::NoEntry => "Windows credential does not exist",
        Error::Ambiguous(_) => "Windows credential is ambiguous",
        _ => "Windows credential provider error",
    };
    KeyringError::Secret {
        code: 0,
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn entry_is_native_and_provider_error_never_formats_secret_bytes() {
        assert!(
            entry("synthetic-provider-type-check")
                .unwrap()
                .get_credential()
                .is::<WinCredential>()
        );
        let error = map_error(Error::BadEncoding(b"SYNTHETIC_SECRET".to_vec()));
        assert_eq!(error, KeyringError::Encoding);
        assert!(!format!("{error:?} {error}").contains("SYNTHETIC_SECRET"));
    }
}
