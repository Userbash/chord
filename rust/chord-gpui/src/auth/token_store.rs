#![forbid(unsafe_code)]
//! P0-2.2: Redacted Keyring `TokenStore` Adapter.
//!
//! Provides a clean-room security boundary around token storage:
//! - [`SecretToken`] validates non-empty tokens without control characters (`\0`, `\r`, `\n`)
//!   and redacts both [`fmt::Debug`] and [`fmt::Display`] outputs (`SecretToken([REDACTED])` /
//!   `[REDACTED]`) so raw tokens never leak into logs, panic messages, or diagnostics.
//! - [`TokenStoreError`] reports validation and storage errors without ever embedding secret bytes.
//! - [`SecretBackend`] abstracts over [`MemoryKeyringBackend`] (deterministic in-memory backend
//!   with fault injection for testing) and [`SystemKeyringBackend`] (backed by
//!   `chord_platform::keyring::Keyring`).
//! - [`TokenStore`] manages the active account token (`chord.active_account`) and per-account
//!   tokens (`chord.account.<account_id>`).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;

use chord_platform::keyring::{Keyring, KeyringError};

/// Default keyring key for the currently active Discord account token.
pub const DEFAULT_TOKEN_KEY: &str = "chord.active_account";

/// Prefix for per-account keyring entries (`"chord.account.<account_id>"`).
pub const ACCOUNT_KEY_PREFIX: &str = "chord.account.";

/// Maximum allowed length for an `account_id` (ASCII alphanumeric, `_`, `-`, `.`).
pub const MAX_ACCOUNT_ID_LEN: usize = 128;

/// Validated, redacted authentication token wrapper.
///
/// Invariants:
/// - Stored string is trimmed and guaranteed non-empty.
/// - Rejects NUL (`\0`), CR (`\r`), and LF (`\n`) characters.
/// - [`fmt::Debug`] always formats as `"SecretToken([REDACTED])"`.
/// - [`fmt::Display`] always formats as `"[REDACTED]"`.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretToken {
    inner: String,
}

impl SecretToken {
    /// Validates and constructs a [`SecretToken`] from a raw string slice.
    ///
    /// Rejects strings that contain `\0`, `\r`, or `\n` anywhere, or that are empty
    /// after trimming leading/trailing whitespace.
    pub fn new(raw: impl AsRef<str>) -> Result<Self, TokenStoreError> {
        let raw_ref = raw.as_ref();
        let trimmed = raw_ref.trim();
        if trimmed.is_empty() || trimmed.contains(['\0', '\r', '\n']) {
            return Err(TokenStoreError::InvalidToken);
        }
        Ok(Self {
            inner: trimmed.to_owned(),
        })
    }

    /// Explicitly exposes the underlying raw secret token string for authorized wire/storage use.
    #[must_use]
    pub fn expose_secret(&self) -> &str {
        &self.inner
    }
}

impl fmt::Debug for SecretToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretToken([REDACTED])")
    }
}

impl fmt::Display for SecretToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl TryFrom<&str> for SecretToken {
    type Error = TokenStoreError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for SecretToken {
    type Error = TokenStoreError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// Errors returned by [`TokenStore`], [`SecretToken`], and [`SecretBackend`] implementations.
///
/// Security invariant: no variant ever embeds raw token bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenStoreError {
    /// Account identifier is empty, exceeds 128 characters, or contains disallowed characters.
    InvalidAccountId,
    /// Token is empty, whitespace-only, or contains forbidden control characters (`\0`, `\r`, `\n`).
    InvalidToken,
    /// Secret Service / keyring backend is unavailable or locked.
    BackendUnavailable(String),
    /// Reading, writing, or deleting a keyring entry failed.
    StorageFailure(String),
}

impl fmt::Display for TokenStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAccountId => {
                f.write_str("invalid account id (expected 1..=128 ASCII [A-Za-z0-9_.-] chars)")
            }
            Self::InvalidToken => {
                f.write_str("invalid secret token (empty or contains control characters)")
            }
            Self::BackendUnavailable(reason) => {
                write!(f, "keyring backend unavailable: {reason}")
            }
            Self::StorageFailure(reason) => {
                write!(f, "keyring storage failure: {reason}")
            }
        }
    }
}

impl std::error::Error for TokenStoreError {}

/// Backend trait abstracting secret storage operations.
pub trait SecretBackend {
    /// Stores or overwrites `secret` under `key`.
    fn set_secret(&mut self, key: &str, secret: &str) -> Result<(), TokenStoreError>;

    /// Retrieves the secret stored under `key`, or `Ok(None)` if not present.
    fn get_secret(&self, key: &str) -> Result<Option<String>, TokenStoreError>;

    /// Deletes the secret stored under `key`, returning `Ok(true)` if an entry existed.
    fn delete_secret(&mut self, key: &str) -> Result<bool, TokenStoreError>;
}

/// Deterministic in-memory [`SecretBackend`] backed by a [`BTreeMap`] with optional fault injection.
#[derive(Clone, Default)]
pub struct MemoryKeyringBackend {
    entries: BTreeMap<String, String>,
    unavailable: bool,
    fail_storage: bool,
}

impl fmt::Debug for MemoryKeyringBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let keys: Vec<&str> = self.entries.keys().map(String::as_str).collect();
        f.debug_struct("MemoryKeyringBackend")
            .field("keys", &keys)
            .field("unavailable", &self.unavailable)
            .field("fail_storage", &self.fail_storage)
            .finish()
    }
}

impl MemoryKeyringBackend {
    /// Creates an empty in-memory keyring backend.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggles simulated `BackendUnavailable` errors on all operations.
    pub fn set_unavailable(&mut self, unavailable: bool) {
        self.unavailable = unavailable;
    }

    /// Toggles simulated `StorageFailure` errors on read/write/delete operations.
    pub fn set_fail_storage(&mut self, fail_storage: bool) {
        self.fail_storage = fail_storage;
    }

    /// Returns the number of stored entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if no entries are stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn check_faults(&self) -> Result<(), TokenStoreError> {
        if self.unavailable {
            return Err(TokenStoreError::BackendUnavailable(
                "memory keyring simulated unavailable".to_owned(),
            ));
        }
        if self.fail_storage {
            return Err(TokenStoreError::StorageFailure(
                "memory keyring simulated storage failure".to_owned(),
            ));
        }
        Ok(())
    }
}

impl SecretBackend for MemoryKeyringBackend {
    fn set_secret(&mut self, key: &str, secret: &str) -> Result<(), TokenStoreError> {
        self.check_faults()?;
        if key.is_empty() || key.contains('\0') {
            return Err(TokenStoreError::StorageFailure(
                "invalid keyring key".to_owned(),
            ));
        }
        if secret.contains('\0') {
            return Err(TokenStoreError::InvalidToken);
        }
        self.entries.insert(key.to_owned(), secret.to_owned());
        Ok(())
    }

    fn get_secret(&self, key: &str) -> Result<Option<String>, TokenStoreError> {
        self.check_faults()?;
        if key.is_empty() || key.contains('\0') {
            return Err(TokenStoreError::StorageFailure(
                "invalid keyring key".to_owned(),
            ));
        }
        Ok(self.entries.get(key).cloned())
    }

    fn delete_secret(&mut self, key: &str) -> Result<bool, TokenStoreError> {
        self.check_faults()?;
        if key.is_empty() || key.contains('\0') {
            return Err(TokenStoreError::StorageFailure(
                "invalid keyring key".to_owned(),
            ));
        }
        Ok(self.entries.remove(key).is_some())
    }
}

/// System Secret Service (`org.freedesktop.secrets` / libsecret) backend wrapping
/// [`chord_platform::keyring::Keyring`].
///
/// Note: [`Keyring`] is `!Send + !Sync`, and `Keyring::lookup` takes `&mut self` while
/// [`SecretBackend::get_secret`] takes `&self`; interior mutability via [`RefCell`] bridges
/// single-threaded lookup calls cleanly.
pub struct SystemKeyringBackend {
    keyring: RefCell<Keyring>,
}

impl fmt::Debug for SystemKeyringBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SystemKeyringBackend")
            .field("schema", &chord_platform::keyring::SCHEMA_NAME)
            .finish_non_exhaustive()
    }
}

impl SystemKeyringBackend {
    /// Connects to the session Secret Service via [`Keyring::connect`].
    pub fn connect() -> Result<Self, TokenStoreError> {
        let keyring = Keyring::connect().map_err(Self::map_connect_error)?;
        Ok(Self {
            keyring: RefCell::new(keyring),
        })
    }

    /// Wraps an already-connected [`Keyring`].
    #[must_use]
    pub fn from_keyring(keyring: Keyring) -> Self {
        Self {
            keyring: RefCell::new(keyring),
        }
    }

    /// Maps a [`KeyringError`] from `Keyring::connect()` into [`TokenStoreError::BackendUnavailable`].
    #[must_use]
    pub fn map_connect_error(err: KeyringError) -> TokenStoreError {
        match err {
            KeyringError::Secret { code, .. } => TokenStoreError::BackendUnavailable(format!(
                "Secret Service connection failed (code {code})"
            )),
            KeyringError::InvalidKey => {
                TokenStoreError::BackendUnavailable("invalid keyring schema key".to_owned())
            }
            KeyringError::InvalidSecret => TokenStoreError::InvalidToken,
            KeyringError::Encoding => {
                TokenStoreError::BackendUnavailable("keyring encoding error".to_owned())
            }
        }
    }

    /// Maps a [`KeyringError`] from `store`/`lookup`/`clear` into [`TokenStoreError`] without
    /// propagating any raw secret bytes.
    #[must_use]
    pub fn map_keyring_error(err: KeyringError) -> TokenStoreError {
        match err {
            KeyringError::InvalidKey => {
                TokenStoreError::StorageFailure("invalid keyring key".to_owned())
            }
            KeyringError::InvalidSecret => TokenStoreError::InvalidToken,
            KeyringError::Encoding => {
                TokenStoreError::StorageFailure("stored secret is not valid UTF-8".to_owned())
            }
            KeyringError::Secret { code, .. } => TokenStoreError::StorageFailure(format!(
                "Secret Service operation failed (code {code})"
            )),
        }
    }
}

impl SecretBackend for SystemKeyringBackend {
    fn set_secret(&mut self, key: &str, secret: &str) -> Result<(), TokenStoreError> {
        self.keyring
            .get_mut()
            .store(key, secret)
            .map_err(Self::map_keyring_error)
    }

    fn get_secret(&self, key: &str) -> Result<Option<String>, TokenStoreError> {
        let mut keyring = self
            .keyring
            .try_borrow_mut()
            .map_err(|_| TokenStoreError::StorageFailure("concurrent keyring borrow".to_owned()))?;
        keyring.lookup(key).map_err(Self::map_keyring_error)
    }

    fn delete_secret(&mut self, key: &str) -> Result<bool, TokenStoreError> {
        self.keyring
            .get_mut()
            .clear(key)
            .map_err(Self::map_keyring_error)
    }
}

/// Validates an `account_id` and formats the canonical keyring key `"chord.account.<account_id>"`.
///
/// Rules:
/// - Non-empty and `<= 128` bytes (`MAX_ACCOUNT_ID_LEN`).
/// - Only ASCII alphanumeric characters or `_`, `-`, `.`.
pub fn format_account_key(account_id: &str) -> Result<String, TokenStoreError> {
    if account_id.is_empty() || account_id.len() > MAX_ACCOUNT_ID_LEN {
        return Err(TokenStoreError::InvalidAccountId);
    }
    let valid_chars = account_id
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'));
    if !valid_chars {
        return Err(TokenStoreError::InvalidAccountId);
    }
    Ok(format!("{ACCOUNT_KEY_PREFIX}{account_id}"))
}

/// High-level token store adapter parameterized over a [`SecretBackend`].
#[derive(Debug, Clone)]
pub struct TokenStore<B: SecretBackend> {
    backend: B,
}

impl<B: SecretBackend> TokenStore<B> {
    /// Creates a new [`TokenStore`] wrapping `backend`.
    #[must_use]
    pub const fn new(backend: B) -> Self {
        Self { backend }
    }

    /// Returns a shared reference to the underlying backend.
    #[must_use]
    pub const fn backend(&self) -> &B {
        &self.backend
    }

    /// Returns a mutable reference to the underlying backend.
    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    /// Consumes the store and returns the underlying backend.
    #[must_use]
    pub fn into_backend(self) -> B {
        self.backend
    }

    /// Saves or overwrites the active account token under [`DEFAULT_TOKEN_KEY`].
    pub fn save_active_token(&mut self, token: &SecretToken) -> Result<(), TokenStoreError> {
        self.backend
            .set_secret(DEFAULT_TOKEN_KEY, token.expose_secret())
    }

    /// Loads the active account token from [`DEFAULT_TOKEN_KEY`], validating it as a [`SecretToken`].
    pub fn load_active_token(&self) -> Result<Option<SecretToken>, TokenStoreError> {
        match self.backend.get_secret(DEFAULT_TOKEN_KEY)? {
            Some(raw) => SecretToken::new(raw).map(Some),
            None => Ok(None),
        }
    }

    /// Deletes the active account token under [`DEFAULT_TOKEN_KEY`].
    pub fn delete_active_token(&mut self) -> Result<bool, TokenStoreError> {
        self.backend.delete_secret(DEFAULT_TOKEN_KEY)
    }

    /// Saves or overwrites the token for `account_id` under `"chord.account.<account_id>"`.
    pub fn save_account_token(
        &mut self,
        account_id: &str,
        token: &SecretToken,
    ) -> Result<(), TokenStoreError> {
        let key = format_account_key(account_id)?;
        self.backend.set_secret(&key, token.expose_secret())
    }

    /// Loads the token for `account_id` from `"chord.account.<account_id>"`.
    pub fn load_account_token(
        &self,
        account_id: &str,
    ) -> Result<Option<SecretToken>, TokenStoreError> {
        let key = format_account_key(account_id)?;
        match self.backend.get_secret(&key)? {
            Some(raw) => SecretToken::new(raw).map(Some),
            None => Ok(None),
        }
    }

    /// Deletes the token for `account_id` under `"chord.account.<account_id>"`.
    pub fn delete_account_token(&mut self, account_id: &str) -> Result<bool, TokenStoreError> {
        let key = format_account_key(account_id)?;
        self.backend.delete_secret(&key)
    }
}
