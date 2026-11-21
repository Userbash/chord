#![forbid(unsafe_code)]
//! Task D-07: Session Resilience & Security (`docs/re/session-resilience-d07.md`).
//!
//! Clean-room specification and Auth & Storage engine implementation covering:
//! 1. **Session Token Protection & Automatic Masking**:
//!    - [`D07MaskedToken`] wraps session tokens with automatic `[REDACTED]` masking in [`fmt::Debug`]
//!      and [`fmt::Display`], explicit [`D07MaskedToken::zeroize_now`] + [`Drop`] memory zeroization
//!      with `compiler_fence(Ordering::SeqCst)`, and structural validation (`[\w-]{24,}\.[\w-]{6,}\.[\w-]{27,}`
//!      or `mfa.[\w-]{8,}`).
//!    - Integrates with `chord_core::logging` (`redact_secrets`, `RedactedWriter`, `SecretRedactionLayer`).
//! 2. **Secure OS Keyring Storage & Electron `safeStorage` One-Time Migration**:
//!    - [`D07KeyringTokenVault`] stores tokens under schema `com.discordlab.Token` (`label = "discord-lab token"`),
//!      never in plaintext `localStorage`.
//!    - Supports one-time idempotent migration from legacy Electron `safeStorage` (`dQw4w9WgXcQ:<base64>`)
//!      and DevTools open/close token hiding (`hide_token_for_devtools` / `restore_token_from_devtools`,
//!      `AUTH-RE.md` `T-11`).
//! 3. **Session Recovery Protocol (`Opcode 6 RESUME`) & Abrupt Connection Dropout Handling**:
//!    - [`D07SessionResilienceEngine`] coordinates token vault state with [`ResumeSessionManager`]:
//!      handles abrupt TCP connection drops (`None` close frame), zombie socket heartbeat timeouts
//!      (`> 41,250 ms` without `OP 11 HEARTBEAT_ACK`), server `OP 7 RECONNECT`, and `OP 9 INVALID_SESSION`
//!      (`d=true -> Opcode 6 RESUME`, `d=false -> Opcode 2 IDENTIFY`).
//!    - Enforces `RESUME_MAX_DOWNTIME_MS` (`30,000` ms), `RESUME_TTL_MS` (`180,000` ms = 3 min from last
//!      `HEARTBEAT_ACK`), exponential backoff `1,000..=60,000` ms (`1s -> 2s -> 4s -> 8s -> 16s -> 32s -> 60s`),
//!      and fallback to `IDENTIFY` after `RESUME_RESET_AFTER_FAILURES` (`4`) consecutive failed Resume attempts.
//!    - Preserves `InMemoryCache` guild and channel `Arc` allocations across `RESUME -> RESUMED`.
//!    - On fatal close code `4004` (`Authentication Failed`), stops reconnection and automatically wipes
//!      the revoked token from the OS keyring vault.
//! 4. **Rate Limit Backoff (`429 Too Many Requests`, `503 Service Unavailable`, Retryable `5xx`)**:
//!    - Evaluates per-route/bucket HTTP `429` (`Retry-After` header / JSON `retry_after` / `x-ratelimit-reset-after`),
//!      `503` (default `5,000` ms, cap `60,000` ms), and the 8 retryable server statuses
//!      (`502, 504, 507, 522, 523, 524, 598, 599`) with exponential backoff `1s * 2^(attempt-1)` capped at `60s`.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{Ordering, compiler_fence};
use std::time::Duration;

use crate::cache::InMemoryCache;
use crate::gateway::supervisor::{CloseAction, RESUME_TTL_MS, StopReason, classify_close};
use crate::logging::REDACTED_MARKER;
use crate::rest::rate_limit::{
    RestFailure, classify_failure, rate_limit_info, retry_delay_client_parity,
    should_retry_client_parity,
};
use crate::rest::routes::HttpMethod;
use crate::resume::{
    ReconnectAction, ResumeHandshakeFrame, ResumeRecoveryOutcome, ResumeSessionManager,
};

/// Frozen OS keyring schema name (`org.freedesktop.secrets` / Windows Credential Manager / macOS Keychain).
pub const D07_KEYRING_SCHEMA_NAME: &str = "com.discordlab.Token";

/// Human-readable entry label in the OS Secret Service collection.
pub const D07_KEYRING_LABEL: &str = "discord-lab token";

/// Prefix for tokens encrypted via Electron `safeStorage.encryptString` (`core/bundle.js @185907`).
pub const D07_SAFE_STORAGE_PREFIX: &str = "dQw4w9WgXcQ:";

/// Typed errors for the D-07 Session Resilience & Security engine.
///
/// Security invariant: neither `Debug` nor `Display` ever outputs a raw token, decrypted secret,
/// or base64 ciphertext.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum D07SessionResilienceError {
    /// Keyring account key is empty, whitespace-only, or contains a NUL byte.
    InvalidKeyringKey,
    /// Token is empty, whitespace-only, or contains forbidden control characters (`\0`, `\n`, `\r`).
    EmptyOrMalformedToken,
    /// Token does not match the canonical Discord 3-segment or `mfa.*` token format.
    InvalidTokenFormat,
    /// No token is stored in the OS keyring for the requested account.
    TokenNotFound,
    /// Electron `safeStorage` entry has an empty base64 payload after `dQw4w9WgXcQ:`.
    EmptySafeStoragePayload,
    /// Electron `safeStorage` entry contains malformed base64 characters or padding.
    InvalidSafeStorageBase64,
    /// Native decryption provider failed to decrypt the `safeStorage` ciphertext.
    SafeStorageDecryptFailed,
    /// Attempted to prepare a Resume plan when no session has been established.
    NoActiveSession,
    /// Underlying OS keyring backend reported an error.
    KeyringBackend(String),
}

impl fmt::Display for D07SessionResilienceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKeyringKey => {
                f.write_str("keyring account key must be non-empty and contain no NUL bytes")
            }
            Self::EmptyOrMalformedToken => {
                f.write_str("session token must be non-empty UTF-8 without NUL or newline bytes")
            }
            Self::InvalidTokenFormat => {
                f.write_str("token does not match canonical 3-part or mfa.* structure")
            }
            Self::TokenNotFound => f.write_str("session token not found in OS keyring"),
            Self::EmptySafeStoragePayload => {
                f.write_str("empty base64 payload in safeStorage token entry")
            }
            Self::InvalidSafeStorageBase64 => {
                f.write_str("invalid base64 encoding in safeStorage token entry")
            }
            Self::SafeStorageDecryptFailed => {
                f.write_str("failed to decrypt Electron safeStorage token ciphertext")
            }
            Self::NoActiveSession => {
                f.write_str("no active gateway session available for recovery")
            }
            Self::KeyringBackend(msg) => write!(f, "OS keyring backend error: {msg}"),
        }
    }
}

impl std::error::Error for D07SessionResilienceError {}

/// Zeroizing session token wrapper with automatic `[REDACTED]` masking in `Debug` and `Display`.
#[derive(Clone, PartialEq, Eq)]
pub struct D07MaskedToken {
    secret: Vec<u8>,
}

impl D07MaskedToken {
    /// Wraps a raw session token string in a zeroizing, auto-masked container.
    #[must_use]
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            secret: token.into().into_bytes(),
        }
    }

    /// Exposes the underlying token string slice for authorized wire framing (`Authorization` header,
    /// Opcode 2 `IDENTIFY`, Opcode 6 `RESUME`).
    #[must_use]
    pub fn expose_secret(&self) -> &str {
        std::str::from_utf8(&self.secret).unwrap_or("")
    }

    /// Returns the masked representation (`"[REDACTED]"`).
    #[must_use]
    pub const fn masked(&self) -> &'static str {
        REDACTED_MARKER
    }

    /// Overwrites the in-memory secret buffer with zeros and clears it.
    pub fn zeroize_now(&mut self) {
        self.secret.fill(0);
        compiler_fence(Ordering::SeqCst);
        self.secret.clear();
    }
}

impl fmt::Debug for D07MaskedToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("D07MaskedToken")
            .field(&REDACTED_MARKER)
            .finish()
    }
}

impl fmt::Display for D07MaskedToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(REDACTED_MARKER)
    }
}

impl Drop for D07MaskedToken {
    fn drop(&mut self) {
        self.zeroize_now();
    }
}

/// Always masks a token preview as `"[REDACTED]"` so UI/diagnostics never expose prefix or suffix characters.
#[must_use]
pub fn mask_token_preview(_token: &str) -> &'static str {
    REDACTED_MARKER
}

/// Validates whether `raw` matches Discord's empirical token format (`AUTH-RE.md` `T-06`,
/// `core/bundle.js @186263`: 3 dot-separated base64url segments `[\w-]{24,}\.[\w-]{6,}\.[\w-]{27,}`
/// or `mfa.[\w-]{8,}`).
#[must_use]
pub fn is_valid_discord_token_structure(raw: &str) -> bool {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.contains(['\0', '\n', '\r']) {
        return false;
    }
    if let Some(mfa_tail) = trimmed.strip_prefix("mfa.") {
        return mfa_tail.len() >= 8
            && mfa_tail
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    }
    let parts: Vec<&str> = trimmed.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    let (p0, p1, p2) = (parts[0], parts[1], parts[2]);
    if p0.len() < 24 || p1.len() < 6 || p2.len() < 27 {
        return false;
    }
    parts.iter().all(|seg| {
        seg.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    })
}

/// Abstraction over an OS keyring provider (`com.discordlab.Token`).
pub trait D07KeyringBackend {
    fn store(&mut self, key: &str, secret: &str) -> Result<(), D07SessionResilienceError>;
    fn lookup(&self, key: &str) -> Result<Option<D07MaskedToken>, D07SessionResilienceError>;
    fn clear(&mut self, key: &str) -> Result<bool, D07SessionResilienceError>;
}

/// Deterministic in-memory OS keyring backend with zeroizing entries and secret-redacted `Debug`.
#[derive(Default, Clone)]
pub struct D07InMemoryKeyringBackend {
    entries: BTreeMap<String, D07MaskedToken>,
}

impl fmt::Debug for D07InMemoryKeyringBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("D07InMemoryKeyringBackend")
            .field("entry_count", &self.entries.len())
            .finish()
    }
}

impl D07KeyringBackend for D07InMemoryKeyringBackend {
    fn store(&mut self, key: &str, secret: &str) -> Result<(), D07SessionResilienceError> {
        validate_keyring_key(key)?;
        validate_token_non_empty(secret)?;
        self.entries
            .insert(key.to_string(), D07MaskedToken::new(secret.trim()));
        Ok(())
    }

    fn lookup(&self, key: &str) -> Result<Option<D07MaskedToken>, D07SessionResilienceError> {
        validate_keyring_key(key)?;
        Ok(self.entries.get(key).cloned())
    }

    fn clear(&mut self, key: &str) -> Result<bool, D07SessionResilienceError> {
        validate_keyring_key(key)?;
        if let Some(mut old) = self.entries.remove(key) {
            old.zeroize_now();
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

/// Secure OS keyring token vault (`schema = "com.discordlab.Token"`, `label = "discord-lab token"`).
#[derive(Clone)]
pub struct D07KeyringTokenVault<B: D07KeyringBackend = D07InMemoryKeyringBackend> {
    backend: B,
    safe_storage_migrated: bool,
}

impl Default for D07KeyringTokenVault<D07InMemoryKeyringBackend> {
    fn default() -> Self {
        Self::new()
    }
}

impl D07KeyringTokenVault<D07InMemoryKeyringBackend> {
    /// Creates a new token vault backed by [`D07InMemoryKeyringBackend`].
    #[must_use]
    pub fn new() -> Self {
        Self::with_backend(D07InMemoryKeyringBackend::default())
    }
}

impl<B: D07KeyringBackend> fmt::Debug for D07KeyringTokenVault<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("D07KeyringTokenVault")
            .field("schema", &D07_KEYRING_SCHEMA_NAME)
            .field("label", &D07_KEYRING_LABEL)
            .field("safe_storage_migrated", &self.safe_storage_migrated)
            .finish()
    }
}

impl<B: D07KeyringBackend> D07KeyringTokenVault<B> {
    /// Creates a token vault wrapping a custom [`D07KeyringBackend`].
    #[must_use]
    pub const fn with_backend(backend: B) -> Self {
        Self {
            backend,
            safe_storage_migrated: false,
        }
    }

    /// Frozen OS keyring schema name (`"com.discordlab.Token"`).
    #[must_use]
    pub const fn schema_name(&self) -> &'static str {
        D07_KEYRING_SCHEMA_NAME
    }

    /// Frozen OS keyring entry label (`"discord-lab token"`).
    #[must_use]
    pub const fn label(&self) -> &'static str {
        D07_KEYRING_LABEL
    }

    /// Returns `true` if one-time Electron `safeStorage` migration has completed.
    #[must_use]
    pub const fn is_safe_storage_migrated(&self) -> bool {
        self.safe_storage_migrated
    }

    /// Stores a non-empty UTF-8 session token in the OS keyring under `account_key`.
    pub fn store_token(
        &mut self,
        account_key: &str,
        token: &str,
    ) -> Result<(), D07SessionResilienceError> {
        validate_keyring_key(account_key)?;
        validate_token_non_empty(token)?;
        self.backend.store(account_key, token.trim())
    }

    /// Stores a session token after verifying it matches Discord's canonical 3-part or `mfa.*` format.
    pub fn store_validated_discord_token(
        &mut self,
        account_key: &str,
        token: &str,
    ) -> Result<(), D07SessionResilienceError> {
        validate_keyring_key(account_key)?;
        validate_token_non_empty(token)?;
        if !is_valid_discord_token_structure(token) {
            return Err(D07SessionResilienceError::InvalidTokenFormat);
        }
        self.backend.store(account_key, token.trim())
    }

    /// Loads the masked session token for `account_key` from the OS keyring.
    pub fn load_token(
        &self,
        account_key: &str,
    ) -> Result<Option<D07MaskedToken>, D07SessionResilienceError> {
        self.backend.lookup(account_key)
    }

    /// Returns `true` if a session token is currently stored for `account_key`.
    pub fn has_token(&self, account_key: &str) -> Result<bool, D07SessionResilienceError> {
        Ok(self.backend.lookup(account_key)?.is_some())
    }

    /// Removes and zeroizes the session token for `account_key` from the OS keyring.
    pub fn clear_token(&mut self, account_key: &str) -> Result<bool, D07SessionResilienceError> {
        self.backend.clear(account_key)
    }

    /// Performs a one-time idempotent migration of an Electron `safeStorage` token
    /// (`dQw4w9WgXcQ:<base64>`) into the OS keyring under `account_key`.
    pub fn migrate_safe_storage_once<D>(
        &mut self,
        account_key: &str,
        raw_entry: &str,
        decrypt: D,
    ) -> Result<bool, D07SessionResilienceError>
    where
        D: FnOnce(&[u8]) -> Result<Vec<u8>, D07SessionResilienceError>,
    {
        validate_keyring_key(account_key)?;
        if self.safe_storage_migrated {
            return Ok(false);
        }
        if self.has_token(account_key)? {
            self.safe_storage_migrated = true;
            return Ok(false);
        }
        let trimmed = raw_entry.trim();
        let Some(b64_payload) = trimmed.strip_prefix(D07_SAFE_STORAGE_PREFIX) else {
            return Ok(false);
        };
        let b64_trimmed = b64_payload.trim();
        if b64_trimmed.is_empty() {
            return Err(D07SessionResilienceError::EmptySafeStoragePayload);
        }
        let mut ciphertext = decode_base64_strict(b64_trimmed)?;
        let decrypt_res = decrypt(&ciphertext);
        ciphertext.fill(0);
        compiler_fence(Ordering::SeqCst);
        let mut decrypted = decrypt_res?;
        let token_res = match std::str::from_utf8(&decrypted) {
            Ok(s) => {
                validate_token_non_empty(s)?;
                self.backend.store(account_key, s.trim())
            }
            Err(_) => Err(D07SessionResilienceError::EmptyOrMalformedToken),
        };
        decrypted.fill(0);
        compiler_fence(Ordering::SeqCst);
        token_res?;
        self.safe_storage_migrated = true;
        Ok(true)
    }
}

/// Cause of an abrupt Gateway connection dropout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum D07DropoutCause {
    /// Abrupt TCP/WebSocket drop without a close frame (`classify_close(None)`).
    AbruptNetworkDrop,
    /// Missed `OP 11 HEARTBEAT_ACK` within `heartbeat_interval_ms` (`41,250` ms).
    ZombieHeartbeatTimeout { elapsed_without_ack_ms: u64 },
    /// Server sent `OP 7 RECONNECT`.
    ServerOp7Reconnect,
    /// Server sent `OP 9 INVALID_SESSION` with `d: resumable`.
    InvalidSession { resumable: bool },
}

/// Recovery plan computed by [`D07SessionResilienceEngine::handle_connection_dropout`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct D07DropoutRecoveryPlan {
    /// Selected reconnect action (`ReconnectAction::Resume` op6 or `ReconnectAction::Identify` op2).
    pub action: ReconnectAction,
    /// Exponential backoff delay in milliseconds (`1,000..=60,000` ms).
    pub backoff_delay_ms: u64,
    /// Normalized target gateway URL (`resume_gateway_url` without trailing `/` if resuming).
    pub target_gateway_url: Option<String>,
    /// Prepared Opcode 6 `RESUME` handshake frame (present when `action == ReconnectAction::Resume`).
    pub resume_frame: Option<ResumeHandshakeFrame>,
}

/// High-level lifecycle state of the D-07 Session Resilience engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum D07ResilienceState {
    /// No token stored in keyring and no active session.
    Unauthenticated,
    /// Token stored in OS keyring; ready for initial `IDENTIFY`.
    TokenStored,
    /// Gateway session established (`READY` or `RESUMED`).
    SessionEstablished,
    /// Recovering from abrupt connection dropout via Opcode 6 `RESUME`.
    Resuming,
    /// Logged out or terminated due to fatal close code `4004`.
    LoggedOut,
}

/// Decision returned by [`D07SessionResilienceEngine::evaluate_http_backoff`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct D07RateLimitBackoffDecision {
    /// HTTP status code evaluated.
    pub status: u16,
    /// Whether the request should be retried after `delay`.
    pub should_retry: bool,
    /// Computed backoff pause (`Retry-After` or exponential `1s..60s`).
    pub delay: Option<Duration>,
    /// Whether the rate limit is global across all routes (`x-ratelimit-global`).
    pub is_global: bool,
    /// Rate-limit bucket ID (`x-ratelimit-bucket`), if present.
    pub bucket: Option<String>,
    /// Remaining requests in the bucket (`x-ratelimit-remaining`), if present.
    pub remaining: Option<u32>,
}

/// Observable telemetry counters for Task D-07 Session Resilience & Security.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct D07ResilienceTelemetry {
    pub tokens_stored: u64,
    pub tokens_masked: u64,
    pub safe_storage_migrations: u64,
    pub abrupt_dropouts: u64,
    pub opcode6_resumes_sent: u64,
    pub sessions_resumed: u64,
    pub identify_fallbacks: u64,
    pub zombie_sockets_detected: u64,
    pub rate_limit_backoffs: u64,
    pub unauthorized_logouts: u64,
}

/// Unified Session Resilience & Security Engine (Task D-07).
pub struct D07SessionResilienceEngine<B: D07KeyringBackend = D07InMemoryKeyringBackend> {
    vault: D07KeyringTokenVault<B>,
    resume_manager: ResumeSessionManager,
    active_account_id: Option<String>,
    last_heartbeat_ack_elapsed_ms: u64,
    devtools_stash: Option<(String, D07MaskedToken)>,
    state: D07ResilienceState,
    telemetry: D07ResilienceTelemetry,
}

impl Default for D07SessionResilienceEngine<D07InMemoryKeyringBackend> {
    fn default() -> Self {
        Self::new()
    }
}

impl D07SessionResilienceEngine<D07InMemoryKeyringBackend> {
    /// Creates a new [`D07SessionResilienceEngine`] with an in-memory keyring vault.
    #[must_use]
    pub fn new() -> Self {
        Self {
            vault: D07KeyringTokenVault::new(),
            resume_manager: ResumeSessionManager::new(),
            active_account_id: None,
            last_heartbeat_ack_elapsed_ms: 0,
            devtools_stash: None,
            state: D07ResilienceState::Unauthenticated,
            telemetry: D07ResilienceTelemetry::default(),
        }
    }
}

impl<B: D07KeyringBackend> D07SessionResilienceEngine<B> {
    /// Returns the current lifecycle state.
    #[must_use]
    pub const fn state(&self) -> D07ResilienceState {
        self.state
    }

    /// Returns a reference to the OS keyring token vault.
    #[must_use]
    pub const fn vault(&self) -> &D07KeyringTokenVault<B> {
        &self.vault
    }

    /// Returns a reference to the underlying Opcode 6 [`ResumeSessionManager`].
    #[must_use]
    pub const fn resume_manager(&self) -> &ResumeSessionManager {
        &self.resume_manager
    }

    /// Returns observable telemetry counters.
    #[must_use]
    pub const fn telemetry(&self) -> &D07ResilienceTelemetry {
        &self.telemetry
    }

    /// Stores the account session token in the OS keyring vault with automatic masking.
    pub fn store_account_token(
        &mut self,
        account_id: &str,
        token: &str,
    ) -> Result<(), D07SessionResilienceError> {
        self.vault.store_token(account_id, token)?;
        self.active_account_id = Some(account_id.to_string());
        self.state = D07ResilienceState::TokenStored;
        self.telemetry.tokens_stored = self.telemetry.tokens_stored.saturating_add(1);
        self.telemetry.tokens_masked = self.telemetry.tokens_masked.saturating_add(1);
        Ok(())
    }

    /// Hides the active token from primary storage while DevTools is opened (`AUTH-RE.md` `T-11`: `hideToken`).
    pub fn hide_token_for_devtools(
        &mut self,
        account_id: &str,
    ) -> Result<(), D07SessionResilienceError> {
        let token = self
            .vault
            .load_token(account_id)?
            .ok_or(D07SessionResilienceError::TokenNotFound)?;
        self.vault.clear_token(account_id)?;
        self.devtools_stash = Some((account_id.to_string(), token));
        Ok(())
    }

    /// Restores the hidden token when DevTools is closed (`AUTH-RE.md` `T-11`: `showToken`).
    pub fn restore_token_from_devtools(
        &mut self,
        account_id: &str,
    ) -> Result<(), D07SessionResilienceError> {
        let Some((stashed_account, mut masked)) = self.devtools_stash.take() else {
            return Err(D07SessionResilienceError::TokenNotFound);
        };
        if stashed_account != account_id {
            self.devtools_stash = Some((stashed_account, masked));
            return Err(D07SessionResilienceError::InvalidKeyringKey);
        }
        let res = self.vault.store_token(account_id, masked.expose_secret());
        masked.zeroize_now();
        res
    }

    /// Returns `true` if the token is currently hidden due to an open DevTools window.
    #[must_use]
    pub const fn is_token_hidden_for_devtools(&self) -> bool {
        self.devtools_stash.is_some()
    }

    /// Records an established `READY` session, preserving guild and channel counts from `cache`.
    pub fn on_ready_established(
        &mut self,
        account_id: &str,
        session_id: &str,
        resume_gateway_url: Option<&str>,
        initial_seq: u64,
        cache: &InMemoryCache,
    ) {
        self.active_account_id = Some(account_id.to_string());
        self.resume_manager.on_ready_established(
            session_id,
            resume_gateway_url,
            initial_seq,
            cache,
        );
        self.last_heartbeat_ack_elapsed_ms = 0;
        self.state = D07ResilienceState::SessionEstablished;
    }

    /// Sets the elapsed time in milliseconds since the last `OP 11 HEARTBEAT_ACK`.
    pub fn set_last_heartbeat_ack_elapsed_ms(&mut self, elapsed_ms: u64) {
        self.last_heartbeat_ack_elapsed_ms = elapsed_ms;
    }

    /// Records a monotonically increasing `OP 0 DISPATCH` sequence number `s`.
    pub fn record_dispatch_seq(&mut self, seq: u64) -> bool {
        self.resume_manager.record_dispatch_seq(seq)
    }

    /// Handles an abrupt connection dropout, zombie heartbeat timeout, `OP 7 RECONNECT`,
    /// or `OP 9 INVALID_SESSION`, building an Opcode 6 `RESUME` frame when eligible or
    /// falling back to Opcode 2 `IDENTIFY`.
    pub fn handle_connection_dropout(
        &mut self,
        cause: D07DropoutCause,
        downtime_ms: u64,
        cache: &InMemoryCache,
    ) -> Result<D07DropoutRecoveryPlan, D07SessionResilienceError> {
        let account_id = self
            .active_account_id
            .as_deref()
            .ok_or(D07SessionResilienceError::TokenNotFound)?;
        let token = self
            .vault
            .load_token(account_id)?
            .ok_or(D07SessionResilienceError::TokenNotFound)?;

        self.telemetry.abrupt_dropouts = self.telemetry.abrupt_dropouts.saturating_add(1);
        match cause {
            D07DropoutCause::ZombieHeartbeatTimeout { .. } => {
                self.telemetry.zombie_sockets_detected =
                    self.telemetry.zombie_sockets_detected.saturating_add(1);
            }
            D07DropoutCause::InvalidSession { resumable } => {
                let _ = self.resume_manager.on_invalid_session(resumable);
            }
            D07DropoutCause::AbruptNetworkDrop | D07DropoutCause::ServerOp7Reconnect => {}
        }

        // Enforce 3-minute Heartbeat ACK TTL (`RESUME_TTL_MS = 180_000` ms, `_doResumeOrIdentify`)
        if self.last_heartbeat_ack_elapsed_ms > RESUME_TTL_MS {
            let _ = self.resume_manager.on_invalid_session(false);
            self.telemetry.identify_fallbacks = self.telemetry.identify_fallbacks.saturating_add(1);
            return Ok(D07DropoutRecoveryPlan {
                action: ReconnectAction::Identify,
                backoff_delay_ms: self.resume_manager.on_disconnect_next_backoff_ms(),
                target_gateway_url: None,
                resume_frame: None,
            });
        }

        let action = self.resume_manager.plan_action(downtime_ms);
        if action == ReconnectAction::Resume && self.resume_manager.has_resumable_session() {
            let frame = self
                .resume_manager
                .prepare_resume_frame(token.expose_secret(), cache)
                .map_err(|_| D07SessionResilienceError::NoActiveSession)?;
            let delay = frame.backoff_delay_ms;
            let url = frame.resume_gateway_url.clone();
            self.state = D07ResilienceState::Resuming;
            self.telemetry.opcode6_resumes_sent =
                self.telemetry.opcode6_resumes_sent.saturating_add(1);
            Ok(D07DropoutRecoveryPlan {
                action: ReconnectAction::Resume,
                backoff_delay_ms: delay,
                target_gateway_url: url,
                resume_frame: Some(frame),
            })
        } else {
            self.telemetry.identify_fallbacks = self.telemetry.identify_fallbacks.saturating_add(1);
            Ok(D07DropoutRecoveryPlan {
                action: ReconnectAction::Identify,
                backoff_delay_ms: self.resume_manager.on_disconnect_next_backoff_ms(),
                target_gateway_url: None,
                resume_frame: None,
            })
        }
    }

    /// Records a failed Opcode 6 `RESUME` attempt; after `RESUME_RESET_AFTER_FAILURES` (4)
    /// failures, resets the session and switches to `ReconnectAction::Identify`.
    pub fn on_resume_attempt_failed(&mut self) -> ReconnectAction {
        let next = self.resume_manager.on_resume_attempt_failed();
        if next == ReconnectAction::Identify {
            self.telemetry.identify_fallbacks = self.telemetry.identify_fallbacks.saturating_add(1);
        }
        next
    }

    /// Completes session recovery upon receiving `RESUMED` from the Gateway, resetting
    /// exponential backoff to `1,000` ms and preserving guild/channel caches.
    pub fn on_resumed_received(
        &mut self,
        resumed_seq: u64,
        cache: &InMemoryCache,
    ) -> ResumeRecoveryOutcome {
        self.state = D07ResilienceState::SessionEstablished;
        self.last_heartbeat_ack_elapsed_ms = 0;
        self.telemetry.sessions_resumed = self.telemetry.sessions_resumed.saturating_add(1);
        self.resume_manager.on_resumed_received(resumed_seq, cache)
    }

    /// Handles a WebSocket close code according to Discord's resilience policy:
    /// - `Some(4004)` (`Unauthorized`): stops reconnection, clears the session, and automatically
    ///   wipes the invalid token from the OS keyring vault.
    /// - `Some(4013 | 4014)` (`InvalidClient`): stops reconnection without wiping the token.
    /// - `None` or any transient close code (`1000`, `1006`, `4000`, `4009`): returns `CloseAction::Reconnect`.
    pub fn handle_close_code(
        &mut self,
        account_id: &str,
        code: Option<u16>,
    ) -> Result<CloseAction, D07SessionResilienceError> {
        let action = classify_close(code);
        if action == CloseAction::Stop(StopReason::Unauthorized) {
            let _ = self.vault.clear_token(account_id)?;
            let _ = self.resume_manager.on_invalid_session(false);
            self.state = D07ResilienceState::LoggedOut;
            self.telemetry.unauthorized_logouts =
                self.telemetry.unauthorized_logouts.saturating_add(1);
        }
        Ok(action)
    }

    /// Evaluates HTTP `429 Too Many Requests`, `503 Service Unavailable`, and retryable `5xx`
    /// rate-limit backoff using Discord client parity rules (`web.js @10692340`, `rest-d02.md`).
    #[allow(clippy::too_many_arguments)]
    pub fn evaluate_http_backoff(
        &mut self,
        method: HttpMethod,
        _route_path: &str,
        status: u16,
        attempt: u32,
        headers: &[(String, String)],
        body: &[u8],
        explicit_retries: u32,
    ) -> D07RateLimitBackoffDecision {
        let Some(failure) = classify_failure(status, headers, body) else {
            return D07RateLimitBackoffDecision {
                status,
                should_retry: false,
                delay: None,
                is_global: false,
                bucket: None,
                remaining: None,
            };
        };

        let should_retry = should_retry_client_parity(method, &failure, explicit_retries);
        let delay = if should_retry {
            retry_delay_client_parity(attempt, &failure, headers, body)
        } else {
            None
        };

        let (is_global, bucket, remaining) = match &failure {
            RestFailure::RateLimited(info) => (info.global, info.bucket.clone(), info.remaining),
            _ => {
                let info = rate_limit_info(429, headers, body);
                match info {
                    Some(i) => (i.global, i.bucket, i.remaining),
                    None => (false, None, None),
                }
            }
        };

        if should_retry && delay.is_some() {
            self.telemetry.rate_limit_backoffs =
                self.telemetry.rate_limit_backoffs.saturating_add(1);
        }

        D07RateLimitBackoffDecision {
            status,
            should_retry,
            delay,
            is_global,
            bucket,
            remaining,
        }
    }
}

fn validate_keyring_key(key: &str) -> Result<(), D07SessionResilienceError> {
    if key.trim().is_empty() || key.contains('\0') {
        return Err(D07SessionResilienceError::InvalidKeyringKey);
    }
    Ok(())
}

fn validate_token_non_empty(token: &str) -> Result<(), D07SessionResilienceError> {
    let trimmed = token.trim();
    if trimmed.is_empty() || token.contains(['\0', '\n', '\r']) {
        return Err(D07SessionResilienceError::EmptyOrMalformedToken);
    }
    Ok(())
}

fn decode_base64_strict(input: &str) -> Result<Vec<u8>, D07SessionResilienceError> {
    let bytes = input.as_bytes();
    if bytes.is_empty() {
        return Err(D07SessionResilienceError::EmptySafeStoragePayload);
    }
    let pad_count = bytes.iter().rev().take_while(|&&b| b == b'=').count();
    if pad_count > 2 {
        return Err(D07SessionResilienceError::InvalidSafeStorageBase64);
    }
    let data_bytes = &bytes[..bytes.len() - pad_count];
    if data_bytes.contains(&b'=')
        || (pad_count > 0 && !bytes.len().is_multiple_of(4))
        || data_bytes.len() % 4 == 1
    {
        return Err(D07SessionResilienceError::InvalidSafeStorageBase64);
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
            _ => return Err(D07SessionResilienceError::InvalidSafeStorageBase64),
        };
        accum = (accum << 6) | u32::from(val);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((accum >> bits) as u8);
        }
    }

    if out.is_empty() {
        return Err(D07SessionResilienceError::EmptySafeStoragePayload);
    }
    Ok(out)
}
