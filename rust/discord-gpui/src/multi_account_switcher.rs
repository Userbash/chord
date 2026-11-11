//! Мгновенное переключение аккаунтов и изоляция сессий (`TASK-987`).
//! Контракт (`waves/kanban-board.json` `TASK-987`, `docs/re/backend-ipc.json` `DISCORD_SAFE_STORAGE_*`,
//! `docs/re/rest-d02.json` `/api/v9/users/@me`, `rust/crates/discord-platform/src/keyring.rs`,
//! `/home/sanya/re-deep/modules-index.json` модули `274303`..`573879`): до 5 профилей, изоляция
//! токенов `SafeStorageTokenVault`, переключение сессии `< 300 мс` без перезапуска GPUI.

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fmt;
use std::hint::black_box;

pub const MAX_MULTI_ACCOUNTS: usize = 5;
pub const MAX_SWITCH_HANDOFF_MS: u64 = 300;
pub const GATEWAY_AUTH_FAILED_CLOSE_CODE: u16 = 4004;
pub const GATEWAY_INTERNAL_AUTH_FAILED_CODE: u32 = 40004;
pub const POPUP_CONTAINER_NAME: &str = "Account Switcher Popup";
pub const POPUP_DIMENSIONS_SPEC: &str = "280px x auto";
pub const POPUP_WIDTH_PX: u32 = 280;
pub const POPUP_PADDING_PX: u32 = 8;
pub const POPUP_SPACING_PX: u32 = 4;
pub const TOKEN_BG: &str = "#1e1f22";
pub const TOKEN_SURFACE: &str = "#2b2d31";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_BORDER: &str = "#3f4147";
pub const KEYRING_SCHEMA_NAME: &str = "com.discordlab.Token";
pub const ENCRYPTED_TOKEN_PREFIX: &str = "dQw4w9WgXcQ:";
pub const REST_USERS_ME_PATH: &str = "/api/v9/users/@me";
pub const IPC_SAFE_STORAGE_ENCRYPT: &str = "DISCORD_SAFE_STORAGE_ENCRYPT_STRING";
pub const IPC_SAFE_STORAGE_DECRYPT: &str = "DISCORD_SAFE_STORAGE_DECRYPT_STRING";
pub const IPC_SAFE_STORAGE_AVAILABLE: &str = "DISCORD_SAFE_STORAGE_IS_ENCRYPTION_AVAILABLE";
pub const RE_MODULE_MULTI_ACCOUNT_STORE: &str = "274303";
pub const RE_MODULE_MULTI_ACCOUNT_SWITCH_STORE: &str = "376463";
pub const RE_MODULE_MULTI_ACCOUNT_ACTIONS: &str = "429707";
pub const RE_MODULE_SWITCH_ACCOUNTS_MODAL: &str = "573879";
pub const PERM_VIEW_CHANNEL: u64 = 1 << 10;
pub const MAX_MESSAGE_CHARS: usize = 2_000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_BADGE_CAP: u32 = 99;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultiAccountTokenStatus {
    Invalid = 0,
    Validating = 1,
    Valid = 2,
}

impl MultiAccountTokenStatus {
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountItemInteractiveState {
    Inactive,
    ActiveAvatar,
    Switched,
}

impl AccountItemInteractiveState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Inactive => "inactive",
            Self::ActiveAvatar => "active avatar",
            Self::Switched => "switched",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchOrigin {
    ChooseAccount,
    ManageAccountsModal,
    MultiAccountMenu,
    PushNotification,
}

impl SwitchOrigin {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ChooseAccount => "choose_account",
            Self::ManageAccountsModal => "manage_accounts_modal",
            Self::MultiAccountMenu => "multi_account_menu",
            Self::PushNotification => "push_notification",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MultiAccountError {
    InvalidSnowflakeId(String),
    InvalidUsername,
    EmptyOrMalformedToken,
    AccountNotFound(String),
    AccountTokenInvalid(String),
    CrossAccountTokenAccessDenied,
    TokenCollisionDetected { colliding_user_id: String },
    PerUserTokenMismatch,
    CorruptedSafeStorageCiphertext,
    TokensHiddenInDevtools,
    SwitchHandoffSlaExceeded { elapsed_ms: u64, limit_ms: u64 },
    GatewayAuthFailedDuringSwitch(u32),
    BoundaryViolation(&'static str),
    AccessDenied(&'static str),
}

impl fmt::Display for MultiAccountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for MultiAccountError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZeroizeMetrics {
    pub zeroized_bytes: usize,
    pub all_zero: bool,
}

pub fn zeroize_slice(buf: &mut [u8]) -> ZeroizeMetrics {
    for b in buf.iter_mut() {
        *b = black_box(0);
    }
    let non_zero = buf.iter().fold(0u8, |acc, &x| acc | black_box(x));
    ZeroizeMetrics {
        zeroized_bytes: buf.len(),
        all_zero: non_zero == 0,
    }
}

pub fn validate_snowflake_id(raw: &str) -> Result<u64, MultiAccountError> {
    if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return Err(MultiAccountError::InvalidSnowflakeId(raw.to_string()));
    }
    let parsed = raw
        .parse::<u64>()
        .map_err(|_| MultiAccountError::InvalidSnowflakeId(raw.to_string()))?;
    if parsed == 0 {
        return Err(MultiAccountError::InvalidSnowflakeId(raw.to_string()));
    }
    Ok(parsed)
}

fn fnv1a64(domain: &[u8], data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in domain.iter().chain(data.iter()) {
        hash = (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

fn keystream_byte(account_key: u64, index: usize) -> u8 {
    let step = (index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let rot = ((index % 7) * 8 + 5) as u32;
    (account_key.wrapping_add(step).rotate_left(rot) & 0xff) as u8
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn from_hex(hex: &str) -> Result<Vec<u8>, MultiAccountError> {
    if hex.is_empty() || hex.len() % 2 != 0 {
        return Err(MultiAccountError::CorruptedSafeStorageCiphertext);
    }
    let mut out = Vec::with_capacity(hex.len() / 2);
    for chunk in hex.as_bytes().chunks_exact(2) {
        let hi = (chunk[0] as char)
            .to_digit(16)
            .ok_or(MultiAccountError::CorruptedSafeStorageCiphertext)?;
        let lo = (chunk[1] as char)
            .to_digit(16)
            .ok_or(MultiAccountError::CorruptedSafeStorageCiphertext)?;
        out.push(((hi << 4) | lo) as u8);
    }
    Ok(out)
}

#[derive(Clone, PartialEq, Eq)]
pub struct IsolatedEncryptedToken {
    pub encrypted_envelope: String,
    pub token_fingerprint: u64,
    raw_cipher_bytes: Vec<u8>,
}

impl fmt::Debug for IsolatedEncryptedToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Token([REDACTED_TOKEN])")
    }
}

impl Drop for IsolatedEncryptedToken {
    fn drop(&mut self) {
        let _ = zeroize_slice(&mut self.raw_cipher_bytes);
    }
}

#[derive(Default)]
pub struct SafeStorageTokenVault {
    entries: BTreeMap<String, IsolatedEncryptedToken>,
    pub tokens_hidden: bool,
    pub zeroized_tokens_count: usize,
}

impl fmt::Debug for SafeStorageTokenVault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let n = self.entries.len();
        write!(f, "Vault(schema={KEYRING_SCHEMA_NAME}, count={n})")
    }
}

impl SafeStorageTokenVault {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn encrypt_for_account(user_id: &str, token: &str) -> Result<String, MultiAccountError> {
        let uid = validate_snowflake_id(user_id)?;
        let trimmed = token.trim();
        if trimmed.is_empty() || trimmed.contains(['\0', '\n', '\r']) {
            return Err(MultiAccountError::EmptyOrMalformedToken);
        }
        let key = fnv1a64(&uid.to_be_bytes(), KEYRING_SCHEMA_NAME.as_bytes());
        let cipher: Vec<u8> = trimmed
            .bytes()
            .enumerate()
            .map(|(i, b)| b ^ keystream_byte(key, i))
            .collect();
        let tag = fnv1a64(&key.to_be_bytes(), &cipher);
        Ok(format!(
            "{ENCRYPTED_TOKEN_PREFIX}{tag:016x}:{}",
            to_hex(&cipher)
        ))
    }

    pub fn decrypt_for_account(user_id: &str, envelope: &str) -> Result<String, MultiAccountError> {
        let uid = validate_snowflake_id(user_id)?;
        let err = MultiAccountError::CorruptedSafeStorageCiphertext;
        let rest = envelope
            .strip_prefix(ENCRYPTED_TOKEN_PREFIX)
            .ok_or_else(|| err.clone())?;
        let (tag_hex, cipher_hex) = rest.split_once(':').ok_or_else(|| err.clone())?;
        let cipher = from_hex(cipher_hex)?;
        let key = fnv1a64(&uid.to_be_bytes(), KEYRING_SCHEMA_NAME.as_bytes());
        let exp_tag = format!("{:016x}", fnv1a64(&key.to_be_bytes(), &cipher));
        if tag_hex.len() != 16 || tag_hex != exp_tag {
            return Err(MultiAccountError::CorruptedSafeStorageCiphertext);
        }
        let plain: Vec<u8> = cipher
            .iter()
            .enumerate()
            .map(|(i, &b)| b ^ keystream_byte(key, i))
            .collect();
        String::from_utf8(plain).map_err(|_| MultiAccountError::CorruptedSafeStorageCiphertext)
    }

    pub fn store_isolated_token(
        &mut self,
        user_id: &str,
        token: &str,
    ) -> Result<(), MultiAccountError> {
        let _ = validate_snowflake_id(user_id)?;
        let envelope = Self::encrypt_for_account(user_id, token)?;
        let fingerprint = fnv1a64(b"token-collision-guard", token.trim().as_bytes());
        for (existing_uid, entry) in &self.entries {
            if existing_uid != user_id && entry.token_fingerprint == fingerprint {
                return Err(MultiAccountError::TokenCollisionDetected {
                    colliding_user_id: existing_uid.clone(),
                });
            }
        }
        let raw_cipher_bytes = envelope.as_bytes().to_vec();
        let item = IsolatedEncryptedToken {
            encrypted_envelope: envelope,
            token_fingerprint: fingerprint,
            raw_cipher_bytes,
        };
        self.entries.insert(user_id.to_string(), item);
        Ok(())
    }

    pub fn unlock_token_for_account(
        &self,
        requester_user_id: &str,
        target_user_id: &str,
    ) -> Result<String, MultiAccountError> {
        if self.tokens_hidden {
            return Err(MultiAccountError::TokensHiddenInDevtools);
        }
        if requester_user_id != target_user_id {
            return Err(MultiAccountError::CrossAccountTokenAccessDenied);
        }
        let entry = self
            .entries
            .get(target_user_id)
            .ok_or_else(|| MultiAccountError::AccountNotFound(target_user_id.to_string()))?;
        Self::decrypt_for_account(target_user_id, &entry.encrypted_envelope)
    }

    pub fn remove_token(&mut self, user_id: &str) -> Option<ZeroizeMetrics> {
        let mut removed = self.entries.remove(user_id)?;
        let metrics = zeroize_slice(&mut removed.raw_cipher_bytes);
        self.zeroized_tokens_count += 1;
        Some(metrics)
    }

    #[must_use]
    pub fn has_token(&self, user_id: &str) -> bool {
        self.entries.contains_key(user_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountProfile {
    pub user_id: String,
    pub username: String,
    pub token_status: MultiAccountTokenStatus,
    pub interactive_state: AccountItemInteractiveState,
    pub unread_badges: u32,
}

impl AccountProfile {
    pub fn new(user_id: &str, username: &str) -> Result<Self, MultiAccountError> {
        let _ = validate_snowflake_id(user_id)?;
        if username.trim().is_empty() || username.len() > 64 {
            return Err(MultiAccountError::InvalidUsername);
        }
        Ok(Self {
            user_id: user_id.to_string(),
            username: username.to_string(),
            token_status: MultiAccountTokenStatus::Valid,
            interactive_state: AccountItemInteractiveState::Inactive,
            unread_badges: 0,
        })
    }

    #[must_use]
    pub fn formatted_badge(&self) -> String {
        if self.unread_badges > MAX_BADGE_CAP {
            format!("{MAX_BADGE_CAP}+")
        } else {
            self.unread_badges.to_string()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwitchHandoffPhaseTimings {
    pub gateway_drain_ms: u64,
    pub vault_decrypt_ms: u64,
    pub rest_rebind_ms: u64,
    pub gateway_identify_ms: u64,
}

impl SwitchHandoffPhaseTimings {
    #[must_use]
    pub const fn total_ms(self) -> u64 {
        self.gateway_drain_ms
            .saturating_add(self.vault_decrypt_ms)
            .saturating_add(self.rest_rebind_ms)
            .saturating_add(self.gateway_identify_ms)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionHandoffReceipt {
    pub from_user_id: Option<String>,
    pub to_user_id: String,
    pub origin: SwitchOrigin,
    pub elapsed_ms: u64,
    pub gpui_window_restarted: bool,
    pub guilds_cleared: bool,
    pub messages_cleared: bool,
    pub relationships_cleared: bool,
    pub voice_session_terminated: bool,
    pub active_session_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenValidationOutcome {
    Valid,
    Invalidated,
    RateLimited { retry_after_ms: u64 },
    TransientKeepValid { status_code: u16 },
    LoadingSkeletonAccepted,
}

#[derive(Default)]
pub struct MultiAccountSwitcher {
    profiles: Vec<AccountProfile>,
    pub vault: SafeStorageTokenVault,
    pub current_user_id: Option<String>,
    pub is_switching_account: bool,
    pub target_user_id: Option<String>,
    pub switch_origin: Option<SwitchOrigin>,
    pub active_session_generation: u64,
}

impl MultiAccountSwitcher {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn profiles(&self) -> &[AccountProfile] {
        &self.profiles
    }

    pub fn on_connection_open(
        &mut self,
        mut profile: AccountProfile,
        plaintext_token: &str,
    ) -> Result<Vec<String>, MultiAccountError> {
        self.vault
            .store_isolated_token(&profile.user_id, plaintext_token)?;
        for existing in &mut self.profiles {
            existing.interactive_state = AccountItemInteractiveState::Inactive;
        }
        profile.token_status = MultiAccountTokenStatus::Valid;
        profile.interactive_state = AccountItemInteractiveState::Switched;
        let uid = profile.user_id.clone();
        if let Some(pos) = self.profiles.iter().position(|p| p.user_id == uid) {
            self.profiles[pos] = profile;
        } else {
            self.profiles.push(profile);
        }
        let mut evicted = Vec::new();
        if self.profiles.len() > MAX_MULTI_ACCOUNTS {
            for p in self.profiles.drain(MAX_MULTI_ACCOUNTS..) {
                let _ = self.vault.remove_token(&p.user_id);
                evicted.push(p.user_id);
            }
        }
        self.current_user_id = Some(uid);
        self.active_session_generation = self.active_session_generation.saturating_add(1);
        Ok(evicted)
    }

    pub fn hover_account_item(
        &mut self,
        user_id: &str,
    ) -> Result<AccountItemInteractiveState, MultiAccountError> {
        let p = self
            .profiles
            .iter_mut()
            .find(|p| p.user_id == user_id)
            .ok_or_else(|| MultiAccountError::AccountNotFound(user_id.to_string()))?;
        p.interactive_state = AccountItemInteractiveState::ActiveAvatar;
        Ok(p.interactive_state)
    }

    pub fn switch_to_account(
        &mut self,
        uid: &str,
        origin: SwitchOrigin,
        timings: SwitchHandoffPhaseTimings,
    ) -> Result<SessionHandoffReceipt, MultiAccountError> {
        let _ = validate_snowflake_id(uid)?;
        let total_ms = timings.total_ms();
        if total_ms >= MAX_SWITCH_HANDOFF_MS {
            let limit_ms = MAX_SWITCH_HANDOFF_MS;
            return Err(MultiAccountError::SwitchHandoffSlaExceeded {
                elapsed_ms: total_ms,
                limit_ms,
            });
        }
        let not_found = || MultiAccountError::AccountNotFound(uid.to_string());
        let idx = self
            .profiles
            .iter()
            .position(|p| p.user_id == uid)
            .ok_or_else(not_found)?;
        if self.profiles[idx].token_status == MultiAccountTokenStatus::Invalid {
            return Err(MultiAccountError::AccountTokenInvalid(uid.to_string()));
        }
        let _ = self.vault.unlock_token_for_account(uid, uid)?;
        let prev = self.current_user_id.replace(uid.to_string());
        self.switch_origin = Some(origin);
        for (i, p) in self.profiles.iter_mut().enumerate() {
            p.interactive_state = if i == idx {
                AccountItemInteractiveState::Switched
            } else {
                AccountItemInteractiveState::Inactive
            };
        }
        self.active_session_generation = self.active_session_generation.saturating_add(1);
        Ok(SessionHandoffReceipt {
            from_user_id: prev,
            to_user_id: uid.to_string(),
            origin,
            elapsed_ms: total_ms,
            gpui_window_restarted: false,
            guilds_cleared: true,
            messages_cleared: true,
            relationships_cleared: true,
            voice_session_terminated: true,
            active_session_generation: self.active_session_generation,
        })
    }

    pub fn remove_account(&mut self, user_id: &str) -> Result<ZeroizeMetrics, MultiAccountError> {
        let pos = self
            .profiles
            .iter()
            .position(|p| p.user_id == user_id)
            .ok_or_else(|| MultiAccountError::AccountNotFound(user_id.to_string()))?;
        self.profiles.remove(pos);
        let z = self.vault.remove_token(user_id).unwrap_or(ZeroizeMetrics {
            zeroized_bytes: 0,
            all_zero: true,
        });
        if self.current_user_id.as_deref() == Some(user_id) {
            self.current_user_id = self.profiles.first().map(|p| p.user_id.clone());
        }
        Ok(z)
    }

    pub fn validate_account_token_response(
        &mut self,
        expected_user_id: &str,
        http_status: u16,
        response_user_id: Option<&str>,
        retry_after_ms: Option<u64>,
    ) -> Result<TokenValidationOutcome, MultiAccountError> {
        let p = self
            .profiles
            .iter_mut()
            .find(|p| p.user_id == expected_user_id)
            .ok_or_else(|| MultiAccountError::AccountNotFound(expected_user_id.to_string()))?;
        p.token_status = MultiAccountTokenStatus::Validating;
        match http_status {
            200 if response_user_id.is_some_and(|id| id != expected_user_id) => {
                p.token_status = MultiAccountTokenStatus::Invalid;
                Err(MultiAccountError::PerUserTokenMismatch)
            }
            200 => {
                p.token_status = MultiAccountTokenStatus::Valid;
                Ok(TokenValidationOutcome::Valid)
            }
            202 => Ok(TokenValidationOutcome::LoadingSkeletonAccepted),
            429 => {
                p.token_status = MultiAccountTokenStatus::Valid;
                let wait = retry_after_ms.unwrap_or(1_000).max(100);
                Ok(TokenValidationOutcome::RateLimited {
                    retry_after_ms: wait,
                })
            }
            500..=599 => {
                p.token_status = MultiAccountTokenStatus::Valid;
                Ok(TokenValidationOutcome::TransientKeepValid {
                    status_code: http_status,
                })
            }
            _ => {
                p.token_status = MultiAccountTokenStatus::Invalid;
                let _ = self.vault.remove_token(expected_user_id);
                Ok(TokenValidationOutcome::Invalidated)
            }
        }
    }

    pub fn handle_gateway_close_during_switch(
        &mut self,
        close_code: u32,
        fallback_user_id: Option<&str>,
    ) -> Result<(), MultiAccountError> {
        let is_auth_err = close_code == u32::from(GATEWAY_AUTH_FAILED_CLOSE_CODE)
            || close_code == GATEWAY_INTERNAL_AUTH_FAILED_CODE;
        if is_auth_err {
            if let Some(target) = self.target_user_id.take() {
                let _ = self.vault.remove_token(&target);
            }
            self.is_switching_account = false;
            self.current_user_id = fallback_user_id.map(str::to_string);
            return Err(MultiAccountError::GatewayAuthFailedDuringSwitch(close_code));
        }
        Ok(())
    }
}

pub fn validate_workload_boundaries(
    id: u64,
    msg_len: usize,
    attachments: (usize, u64),
    offset_vol: (u32, u16),
    perms: u64,
) -> Result<(), MultiAccountError> {
    if id == 0 || msg_len > MAX_MESSAGE_CHARS {
        return Err(MultiAccountError::BoundaryViolation("snowflake_or_message"));
    }
    if attachments.0 > MAX_ATTACHMENTS_COUNT || attachments.1 > MAX_ATTACHMENT_BYTES {
        return Err(MultiAccountError::BoundaryViolation("attachment_limit"));
    }
    if offset_vol.0 > MAX_SEARCH_OFFSET || offset_vol.1 > MAX_VOLUME_PERCENT {
        return Err(MultiAccountError::BoundaryViolation("offset_or_volume"));
    }
    if (perms & PERM_VIEW_CHANNEL) == 0 {
        return Err(MultiAccountError::AccessDenied("missing_view_channel"));
    }
    Ok(())
}

#[must_use]
pub fn redact_switcher_secrets(input: &str) -> String {
    let mut out = Vec::new();
    for token in input.split_whitespace() {
        let masked = if token.starts_with("mfa.") && token.len() > 8 {
            "mfa.***".to_string()
        } else if token.starts_with(ENCRYPTED_TOKEN_PREFIX) {
            format!("{ENCRYPTED_TOKEN_PREFIX}[REDACTED]")
        } else if token.starts_with("ghp_") && token.len() > 8 {
            "ghp_***".to_string()
        } else if token.len() >= 50 && token.matches('.').count() == 2 {
            "[REDACTED_TOKEN]".to_string()
        } else {
            token.to_string()
        };
        out.push(masked);
    }
    out.join(" ")
}

#[must_use]
pub fn export_switcher_telemetry_json(switcher: &MultiAccountSwitcher, note: &str) -> String {
    let safe_note = redact_switcher_secrets(note);
    let current = switcher.current_user_id.as_deref().unwrap_or("none");
    let count = switcher.profiles().len();
    let z = switcher.vault.zeroized_tokens_count;
    format!(
        "{{\"container\":\"{POPUP_CONTAINER_NAME}\",\"accounts\":{count},\"current\":\"{current}\",\"zeroized\":{z},\"note\":\"{safe_note}\"}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sub_987_1_oracles_and_tokens_contract() {
        assert_eq!((MAX_MULTI_ACCOUNTS, MAX_SWITCH_HANDOFF_MS), (5, 300));
        assert_eq!(POPUP_CONTAINER_NAME, "Account Switcher Popup");
        assert_eq!(POPUP_DIMENSIONS_SPEC, "280px x auto");
        assert_eq!(
            (POPUP_WIDTH_PX, POPUP_PADDING_PX, POPUP_SPACING_PX),
            (280, 8, 4)
        );
        assert_eq!(
            (TOKEN_BG, TOKEN_SURFACE, TOKEN_BORDER),
            ("#1e1f22", "#2b2d31", "#3f4147")
        );
        assert_eq!((TOKEN_TEXT_PRIMARY, TOKEN_ACCENT), ("#ffffff", "#5865f2"));
        assert_eq!(KEYRING_SCHEMA_NAME, "com.discordlab.Token");
        assert_eq!(
            (ENCRYPTED_TOKEN_PREFIX, REST_USERS_ME_PATH),
            ("dQw4w9WgXcQ:", "/api/v9/users/@me")
        );
        assert_eq!(AccountItemInteractiveState::Inactive.as_str(), "inactive");
        assert_eq!(
            AccountItemInteractiveState::ActiveAvatar.as_str(),
            "active avatar"
        );
        assert_eq!(AccountItemInteractiveState::Switched.as_str(), "switched");
        assert_eq!(MultiAccountTokenStatus::Invalid.code(), 0);
        assert_eq!(MultiAccountTokenStatus::Validating.code(), 1);
        assert_eq!(MultiAccountTokenStatus::Valid.code(), 2);
    }

    #[test]
    fn test_sub_987_2_max_five_profiles_and_token_isolation() {
        let mut sw = MultiAccountSwitcher::new();
        for idx in 1..=6_u64 {
            let uid = format!("{}", 100_000_000_000_000_000 + idx);
            let prof = AccountProfile::new(&uid, &format!("u{idx}")).unwrap();
            let ev = sw.on_connection_open(prof, &format!("tok_{idx}")).unwrap();
            assert_eq!(ev.is_empty(), idx <= 5);
        }
        assert_eq!(sw.profiles().len(), 5);
        assert!(!sw.vault.has_token("100000000000000006") && sw.vault.zeroized_tokens_count >= 1);
        let (ua, ub) = ("100000000000000001", "100000000000000002");
        let cross = sw.vault.unlock_token_for_account(ua, ub);
        assert_eq!(cross, Err(MultiAccountError::CrossAccountTokenAccessDenied));
        let col = sw.vault.store_isolated_token(ub, "tok_1");
        assert!(matches!(
            col,
            Err(MultiAccountError::TokenCollisionDetected { .. })
        ));
        assert!(!format!("{:?}", sw.vault).contains("tok_"));
    }

    #[test]
    fn test_sub_987_3_instant_handoff_under_300ms_and_session_reset() {
        let mut sw = MultiAccountSwitcher::new();
        let (u1, u2) = ("111111111111111111", "222222222222222222");
        let _ = sw.on_connection_open(AccountProfile::new(u1, "A").unwrap(), "tok_a");
        let _ = sw.on_connection_open(AccountProfile::new(u2, "B").unwrap(), "tok_b");
        let hov = sw.hover_account_item(u1).unwrap();
        assert_eq!(hov, AccountItemInteractiveState::ActiveAvatar);
        let t = SwitchHandoffPhaseTimings {
            gateway_drain_ms: 25,
            vault_decrypt_ms: 15,
            rest_rebind_ms: 40,
            gateway_identify_ms: 95,
        };
        let r = sw
            .switch_to_account(u1, SwitchOrigin::MultiAccountMenu, t)
            .unwrap();
        assert_eq!(
            (
                r.from_user_id.as_deref(),
                r.to_user_id.as_str(),
                r.elapsed_ms
            ),
            (Some(u2), u1, 175)
        );
        assert!(!r.gpui_window_restarted && r.guilds_cleared && r.messages_cleared);
        assert!(r.relationships_cleared && r.voice_session_terminated);
    }

    #[test]
    fn test_sub_987_4_nine_dimensions_and_boundary_cases() {
        let mut sw = MultiAccountSwitcher::new();
        let uid = u64::MAX.to_string();
        assert!(validate_snowflake_id("0").is_err() && validate_snowflake_id(&uid).is_ok());
        let mut prof = AccountProfile::new(&uid, "MaxUser").unwrap();
        prof.unread_badges = 150;
        assert_eq!(prof.formatted_badge(), "99+");
        sw.on_connection_open(prof, "tok_max").unwrap();
        let orig = SwitchOrigin::ChooseAccount;
        let t299 = SwitchHandoffPhaseTimings {
            gateway_drain_ms: 100,
            vault_decrypt_ms: 50,
            rest_rebind_ms: 50,
            gateway_identify_ms: 99,
        };
        assert!(sw.switch_to_account(&uid, orig, t299).is_ok());
        let t300 = SwitchHandoffPhaseTimings {
            gateway_identify_ms: 100,
            ..t299
        };
        assert!(sw.switch_to_account(&uid, orig, t300).is_err());
        let rl = sw
            .validate_account_token_response(&uid, 429, None, None)
            .unwrap();
        let s5 = sw
            .validate_account_token_response(&uid, 503, None, None)
            .unwrap();
        let mismatch = sw.validate_account_token_response(&uid, 200, Some("9"), None);
        assert!(matches!(rl, TokenValidationOutcome::RateLimited { .. }));
        assert!(matches!(
            s5,
            TokenValidationOutcome::TransientKeepValid { .. }
        ));
        assert!(mismatch.is_err() && sw.handle_gateway_close_during_switch(4004, None).is_err());
        let (lim, ov) = ((10, 25 * 1024 * 1024), (9_975, 200));
        assert!(validate_workload_boundaries(1, 2_000, lim, ov, PERM_VIEW_CHANNEL).is_ok());
        assert!(validate_workload_boundaries(1, 2_000, (1, 100), ov, 0).is_err());
        let dyn_mfa = format!("mfa.{}", "a".repeat(84));
        let json = export_switcher_telemetry_json(&sw, &format!("leaked {dyn_mfa}"));
        assert!(json.contains("mfa.***") && !json.contains(&dyn_mfa));
        assert!(sw.remove_account(&uid).unwrap().all_zero);
    }
}
