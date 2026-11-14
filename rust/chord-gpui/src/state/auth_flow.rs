//! Clean-room live authentication & startup session orchestration (`P0-2.3`, `P0-3.1`, `P0-3.2`, `P2-3.1`, `P2-3.2`).
//!
//! Oracles:
//! - `docs/re/rest-d02.md` (§2, §6.1, §6.2): `POST /api/v9/auth/login`, `POST /api/v9/auth/mfa/totp`,
//!   `GET /api/v9/users/@me`, `400` (`captcha_key`), `401`, `429` (`retry_after`), `5xx`.
//! - Security invariants: never log or embed raw passwords, MFA tickets, TOTP codes, or session tokens
//!   in `Debug`, `Display`, or UI error strings (`SecretToken` / `[REDACTED]`).
//! - Never solve, click through, or bypass CAPTCHA challenges; surface `CaptchaChallenge` metadata
//!   to `AppState.active_captcha` and return `AuthFlowError::CaptchaRequired`.

#![forbid(unsafe_code)]
#![allow(dead_code)]

#[path = "../auth/login_service.rs"]
pub mod login_service;

#[path = "../auth/token_store.rs"]
pub mod token_store;

use std::fmt;
use std::future::Future;

#[allow(unused_imports)]
pub use chord_core::rest::user::Me;
#[allow(unused_imports)]
pub use login_service::{
    AUTH_LOGIN_PATH, AUTH_MFA_TOTP_PATH, ApiAuthUserExt, AuthHttpTransport,
    CaptchaChallenge as ServiceCaptchaChallenge, DEFAULT_CAPTCHA_SERVICE,
    DEFAULT_DISCORD_HCAPTCHA_SITEKEY, DEFAULT_DISCORD_LOCALE, DEFAULT_DISCORD_TIMEZONE,
    LiveAuthTransport, LoginError, LoginOutcome, LoginService, MFA_CODE_MAX_DIGITS,
    MFA_CODE_MIN_DIGITS, MfaChallenge, MockAuthTransport, RecordedAuthCall, USERS_ME_PATH,
};
#[allow(unused_imports)]
pub use token_store::{
    ACCOUNT_KEY_PREFIX, DEFAULT_TOKEN_KEY, MAX_ACCOUNT_ID_LEN, MemoryKeyringBackend, SecretBackend,
    SecretToken, SystemKeyringBackend, TokenStore, TokenStoreError, format_account_key,
};

use super::{
    AppState, CaptchaChallenge, CurrentUser, LoginFormState, LoginStep, PresenceStatus, UserAccount,
};

impl PartialEq for CurrentUser {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.username == other.username
            && self.discriminator == other.discriminator
            && self.global_name == other.global_name
            && self.avatar == other.avatar
            && self.status == other.status
            && self.custom_status == other.custom_status
    }
}

impl Eq for CurrentUser {}

/// Environment variable controlling offline demo fixture hydration (`CHORD_OFFLINE_DEMO=1`).
pub const ENV_CHORD_OFFLINE_DEMO: &str = "CHORD_OFFLINE_DEMO";

/// Startup authentication mode (`P2-3.1`, `P2-3.2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AuthMode {
    /// Live mode (default): zero mock guilds/channels/tokens; opens login modal until authenticated.
    #[default]
    Live,
    /// Offline demo mode (`CHORD_OFFLINE_DEMO == "1"`): populates synthetic demo fixtures.
    OfflineDemo,
}

impl AuthMode {
    /// Returns `true` when operating in live mode (`AuthMode::Live`).
    #[must_use]
    pub const fn is_live(self) -> bool {
        matches!(self, Self::Live)
    }

    /// Returns `true` when operating in offline demo mode (`AuthMode::OfflineDemo`).
    #[must_use]
    pub const fn is_offline_demo(self) -> bool {
        matches!(self, Self::OfflineDemo)
    }

    /// Resolves [`AuthMode`] from an explicit boolean flag (`true` -> `OfflineDemo`, `false` -> `Live`).
    #[must_use]
    pub const fn from_flag(offline_demo: bool) -> Self {
        if offline_demo {
            Self::OfflineDemo
        } else {
            Self::Live
        }
    }

    /// Resolves [`AuthMode`] from a raw `CHORD_OFFLINE_DEMO` environment value.
    #[must_use]
    pub fn from_env_value(raw: Option<&str>) -> Self {
        match raw.map(str::trim) {
            Some("1") => Self::OfflineDemo,
            _ => Self::Live,
        }
    }

    /// Resolves [`AuthMode`] from the process environment (`CHORD_OFFLINE_DEMO`).
    #[must_use]
    pub fn from_env() -> Self {
        let env_val = std::env::var(ENV_CHORD_OFFLINE_DEMO).ok();
        Self::from_env_value(env_val.as_deref())
    }
}

/// Evaluates whether mock demo fixtures should be populated given an environment value and
/// an optional explicit override flag (`P2-3.2`).
#[must_use]
pub fn should_populate_mock_data_with(env_val: Option<&str>, explicit_flag: Option<bool>) -> bool {
    if let Some(flag) = explicit_flag {
        return flag;
    }
    AuthMode::from_env_value(env_val).is_offline_demo()
}

/// Returns `true` only when `CHORD_OFFLINE_DEMO == "1"` in the current environment (`P2-3.2`).
#[must_use]
pub fn should_populate_mock_data() -> bool {
    let env_val = std::env::var(ENV_CHORD_OFFLINE_DEMO).ok();
    should_populate_mock_data_with(env_val.as_deref(), None)
}

/// Alias for [`should_populate_mock_data`].
#[must_use]
pub fn should_use_offline_demo() -> bool {
    should_populate_mock_data()
}

/// Outcome of startup auto-login orchestration (`P0-2.3`, `P2-3.1`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartupAuthResult {
    /// Stored token verified against `GET /api/v9/users/@me` (`200 OK`).
    Authenticated {
        user: CurrentUser,
        token_stored: bool,
    },
    /// No stored token, corrupted keyring secret, or `401 Unauthorized` — interactive login required.
    RequiresLogin { reason: String },
    /// Rate limited (`429 Too Many Requests`) with retry delay in milliseconds.
    RateLimited { retry_after_ms: u64 },
    /// Transport or `5xx` server failure during startup verification.
    NetworkError(String),
}

/// Outcome of interactive credential submission (`P0-3.1`).
#[derive(Clone, PartialEq, Eq)]
pub enum LiveAuthStepOutcome {
    /// Direct authentication succeeded, token persisted, and `@me` profile applied to `AppState`.
    Authenticated {
        user: CurrentUser,
        token: SecretToken,
    },
    /// Server requires second-factor verification (`LoginStep::Mfa { ticket }`).
    MfaRequired(MfaChallenge),
}

impl fmt::Debug for LiveAuthStepOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authenticated { user, token: _ } => f
                .debug_struct("Authenticated")
                .field("user", user)
                .field("token", &"[REDACTED]")
                .finish(),
            Self::MfaRequired(challenge) => f.debug_tuple("MfaRequired").field(challenge).finish(),
        }
    }
}

/// Structured errors returned by [`AuthFlowController`] (`P0-3.1`, `P0-3.2`).
///
/// Invariant: neither `Debug` nor `Display` ever contains raw passwords, MFA tickets, TOTP codes,
/// or session tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthFlowError {
    /// Pre-flight input validation failure (empty email, password, token, or invalid TOTP digits).
    Validation(String),
    /// Attempted TOTP submission without an active MFA session ticket.
    MissingMfaTicket,
    /// Server required CAPTCHA verification (`400` with `captcha_key`). Never bypassed automatically.
    CaptchaRequired(ServiceCaptchaChallenge),
    /// Invalid login credentials or rejected MFA code/ticket (`400` / `401`).
    InvalidCredentials { code: Option<i64>, message: String },
    /// Rate limited by Discord API (`429 Too Many Requests`).
    RateLimited { retry_after_ms: u64, global: bool },
    /// Discord server error (`500..=599`).
    ServerError { status: u16 },
    /// Keyring / [`TokenStore`] error.
    TokenStore(TokenStoreError),
    /// Underlying network or payload parse error.
    Transport(String),
}

impl fmt::Display for AuthFlowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(msg) => write!(f, "validation error: {msg}"),
            Self::MissingMfaTicket => write!(f, "missing or expired MFA session ticket"),
            Self::CaptchaRequired(c) => write!(
                f,
                "captcha required by {} (sitekey: {})",
                c.captcha_service, c.captcha_sitekey
            ),
            Self::InvalidCredentials { code, message } => match code {
                Some(c) => write!(f, "invalid credentials (code {c}): {message}"),
                None => write!(f, "invalid credentials: {message}"),
            },
            Self::RateLimited {
                retry_after_ms,
                global,
            } => write!(
                f,
                "rate limited (retry_after_ms={retry_after_ms}, global={global})"
            ),
            Self::ServerError { status } => write!(f, "server error (HTTP {status})"),
            Self::TokenStore(err) => write!(f, "token store error: {err}"),
            Self::Transport(msg) => write!(f, "transport error: {msg}"),
        }
    }
}

impl std::error::Error for AuthFlowError {}

impl From<TokenStoreError> for AuthFlowError {
    fn from(err: TokenStoreError) -> Self {
        Self::TokenStore(err)
    }
}

/// Stateless orchestrator connecting [`TokenStore`], [`LoginService`], and [`AppState`]
/// (`P0-2.3`, `P0-3.1`, `P0-3.2`, `P2-3.1`).
#[derive(Debug, Default, Clone, Copy)]
pub struct AuthFlowController;

impl AuthFlowController {
    /// Maps a verified `GET /api/v9/users/@me` [`Me`] profile into [`AppState`], records the
    /// active token/account if provided, closes `is_login_modal_open`, and wipes transient
    /// credentials from `login_form`.
    pub fn apply_authenticated_profile(state: &mut AppState, me: &Me, token: Option<&SecretToken>) {
        let user = CurrentUser {
            id: me.id.clone(),
            username: me.username.clone(),
            discriminator: me.discriminator.clone(),
            global_name: me.global_name.clone(),
            avatar: me.avatar.clone(),
            status: PresenceStatus::Online,
            custom_status: None,
        };

        if let Some(secret) = token {
            let raw_token = secret.expose_secret().to_string();
            for acc in &mut state.accounts {
                acc.is_active = false;
            }
            if let Some(existing) = state
                .accounts
                .iter_mut()
                .find(|a| a.user.id == user.id || a.token == raw_token)
            {
                existing.user = user.clone();
                existing.token = raw_token.clone();
                existing.is_active = true;
            } else {
                state.accounts.push(UserAccount {
                    user: user.clone(),
                    token: raw_token.clone(),
                    is_active: true,
                });
            }
            state.active_token = Some(raw_token);
        }

        state.current_user = user;
        state.is_login_modal_open = false;
        state.active_captcha = None;
        state.login_form = LoginFormState::default();
    }

    /// Startup auto-login using an injectable async `@me` fetcher closure (`P0-2.3`).
    pub async fn try_auto_login_with<B, F, Fut>(
        state: &mut AppState,
        store: &TokenStore<B>,
        fetcher: F,
    ) -> StartupAuthResult
    where
        B: SecretBackend,
        F: FnOnce(String) -> Fut,
        Fut: Future<Output = Result<Me, LoginError>>,
    {
        let secret_token = match store.load_active_token() {
            Ok(Some(token)) => token,
            Ok(None) => {
                state.active_token = None;
                state.is_login_modal_open = true;
                return StartupAuthResult::RequiresLogin {
                    reason: "no active token in keyring".to_string(),
                };
            }
            Err(err) => {
                let reason = err.to_string();
                state.active_token = None;
                state.is_login_modal_open = true;
                state.login_form.error_message = Some(reason.clone());
                return StartupAuthResult::RequiresLogin { reason };
            }
        };

        let raw_token = secret_token.expose_secret().to_owned();
        match fetcher(raw_token).await {
            Ok(me) => {
                Self::apply_authenticated_profile(state, &me, Some(&secret_token));
                StartupAuthResult::Authenticated {
                    user: state.current_user.clone(),
                    token_stored: true,
                }
            }
            Err(LoginError::InvalidCredentials { message, .. })
            | Err(LoginError::Validation(message)) => {
                state.active_token = None;
                state.is_login_modal_open = true;
                state.login_form.error_message = Some(message.clone());
                StartupAuthResult::RequiresLogin { reason: message }
            }
            Err(LoginError::RateLimited { retry_after_ms, .. }) => {
                state.is_login_modal_open = true;
                state.login_form.error_message = Some(format!(
                    "Слишком много запросов. Повторите через {retry_after_ms} мс."
                ));
                StartupAuthResult::RateLimited { retry_after_ms }
            }
            Err(LoginError::ServerError { status }) => {
                let msg = format!("server error (HTTP {status})");
                state.is_login_modal_open = true;
                state.login_form.error_message =
                    Some(format!("Ошибка сервера Discord (HTTP {status})"));
                StartupAuthResult::NetworkError(msg)
            }
            Err(other) => {
                let msg = other.to_string();
                state.is_login_modal_open = true;
                state.login_form.error_message = Some(msg.clone());
                StartupAuthResult::NetworkError(msg)
            }
        }
    }

    /// Startup auto-login backed by [`TokenStore`] and [`LoginService::fetch_current_user`] (`P0-2.3`).
    pub async fn try_auto_login<B: SecretBackend, T: AuthHttpTransport>(
        state: &mut AppState,
        store: &TokenStore<B>,
        service: &LoginService<T>,
    ) -> StartupAuthResult {
        Self::try_auto_login_with(state, store, |tok| async move {
            service.fetch_current_user(&tok).await
        })
        .await
    }

    /// Submits credentials (email/phone + password or direct user token) through [`LoginService`] (`P0-3.1`).
    ///
    /// - On direct token (`LoginOutcome::Authenticated`): validates & saves [`SecretToken`] in [`TokenStore`],
    ///   fetches `GET /api/v9/users/@me`, updates `AppState.current_user`, and closes `is_login_modal_open`.
    /// - On `LoginOutcome::MfaRequired`: transitions `AppState.login_form.step` to `LoginStep::Mfa { ticket }`,
    ///   records `mfa_ticket`, and wipes `login_form.password`.
    /// - On `LoginError::CaptchaRequired`: populates `AppState.active_captcha` and `login_form.error_message`
    ///   with the hCaptcha service and sitekey without attempting to solve or bypass the CAPTCHA.
    pub async fn submit_credentials_live<B: SecretBackend, T: AuthHttpTransport>(
        state: &mut AppState,
        store: &mut TokenStore<B>,
        service: &LoginService<T>,
    ) -> Result<LiveAuthStepOutcome, AuthFlowError> {
        state.login_form.error_message = None;
        state.active_captcha = None;

        if state.login_form.is_token_mode {
            let secret_token = SecretToken::new(&state.login_form.token).map_err(|_| {
                let msg = "Пожалуйста, введите корректный токен авторизации".to_string();
                state.login_form.error_message = Some(msg.clone());
                AuthFlowError::Validation(msg)
            })?;

            state.login_form.step = LoginStep::Loading;
            match service
                .fetch_current_user(secret_token.expose_secret())
                .await
            {
                Ok(me) => {
                    if let Err(store_err) = store.save_active_token(&secret_token) {
                        state.login_form.step = LoginStep::Credentials;
                        state.login_form.error_message = Some(store_err.to_string());
                        return Err(AuthFlowError::TokenStore(store_err));
                    }
                    let _ = store.save_account_token(&me.id, &secret_token);
                    Self::apply_authenticated_profile(state, &me, Some(&secret_token));
                    return Ok(LiveAuthStepOutcome::Authenticated {
                        user: state.current_user.clone(),
                        token: secret_token,
                    });
                }
                Err(err) => {
                    state.login_form.step = LoginStep::Credentials;
                    return Err(Self::apply_login_error(state, err));
                }
            }
        }

        let login = state.login_form.email_or_phone.trim().to_string();
        let password = state.login_form.password.clone();

        if login.is_empty() {
            let msg = "Это поле обязательно к заполнению".to_string();
            state.login_form.error_message = Some(msg.clone());
            return Err(AuthFlowError::Validation(msg));
        }
        if password.trim().is_empty() {
            let msg = "Пароль обязателен к заполнению".to_string();
            state.login_form.error_message = Some(msg.clone());
            return Err(AuthFlowError::Validation(msg));
        }

        state.login_form.step = LoginStep::Loading;
        match service.login_password(&login, &password).await {
            Ok(LoginOutcome::Authenticated { token, .. }) => {
                let secret_token = SecretToken::new(token).map_err(|err| {
                    state.login_form.step = LoginStep::Credentials;
                    state.login_form.error_message = Some(err.to_string());
                    AuthFlowError::TokenStore(err)
                })?;

                if let Err(store_err) = store.save_active_token(&secret_token) {
                    state.login_form.step = LoginStep::Credentials;
                    state.login_form.error_message = Some(store_err.to_string());
                    return Err(AuthFlowError::TokenStore(store_err));
                }

                match service
                    .fetch_current_user(secret_token.expose_secret())
                    .await
                {
                    Ok(me) => {
                        let _ = store.save_account_token(&me.id, &secret_token);
                        Self::apply_authenticated_profile(state, &me, Some(&secret_token));
                        Ok(LiveAuthStepOutcome::Authenticated {
                            user: state.current_user.clone(),
                            token: secret_token,
                        })
                    }
                    Err(err) => {
                        let _ = store.delete_active_token();
                        state.login_form.step = LoginStep::Credentials;
                        Err(Self::apply_login_error(state, err))
                    }
                }
            }
            Ok(LoginOutcome::MfaRequired(challenge)) => {
                state.login_form.step = LoginStep::Mfa {
                    ticket: challenge.ticket.clone(),
                };
                state.login_form.mfa_ticket = Some(challenge.ticket.clone());
                state.login_form.mfa_code.clear();
                state.login_form.password.clear();
                state.login_form.error_message = None;
                Ok(LiveAuthStepOutcome::MfaRequired(challenge))
            }
            Err(err) => {
                state.login_form.step = LoginStep::Credentials;
                Err(Self::apply_login_error(state, err))
            }
        }
    }

    /// Submits a 6–8 digit TOTP / backup MFA code (`P0-3.2`), persists the resulting
    /// [`SecretToken`] in [`TokenStore`], fetches `@me`, updates `AppState.current_user`,
    /// and closes `is_login_modal_open`.
    pub async fn submit_totp_live<B: SecretBackend, T: AuthHttpTransport>(
        state: &mut AppState,
        store: &mut TokenStore<B>,
        service: &LoginService<T>,
    ) -> Result<CurrentUser, AuthFlowError> {
        state.login_form.error_message = None;

        let ticket = match &state.login_form.step {
            LoginStep::Mfa { ticket } if !ticket.trim().is_empty() => ticket.trim().to_string(),
            _ => state
                .login_form
                .mfa_ticket
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
                .ok_or_else(|| {
                    state.login_form.error_message =
                        Some("Отсутствует или истёк MFA-тикет сессии".to_string());
                    AuthFlowError::MissingMfaTicket
                })?,
        };

        let code = state.login_form.mfa_code.trim().to_string();
        if !(MFA_CODE_MIN_DIGITS..=MFA_CODE_MAX_DIGITS).contains(&code.len())
            || !code.bytes().all(|b| b.is_ascii_digit())
        {
            let msg =
                "Неверный код аутентификации. Код должен содержать от 6 до 8 цифр.".to_string();
            state.login_form.error_message = Some(msg.clone());
            state.login_form.step = LoginStep::Mfa { ticket };
            return Err(AuthFlowError::Validation(msg));
        }

        state.login_form.step = LoginStep::Loading;
        match service.submit_totp(&ticket, &code).await {
            Ok(raw_token) => {
                let secret_token = match SecretToken::new(raw_token) {
                    Ok(tok) => tok,
                    Err(store_err) => {
                        state.login_form.step = LoginStep::Mfa { ticket };
                        state.login_form.error_message = Some(store_err.to_string());
                        return Err(AuthFlowError::TokenStore(store_err));
                    }
                };

                if let Err(store_err) = store.save_active_token(&secret_token) {
                    state.login_form.step = LoginStep::Mfa { ticket };
                    state.login_form.error_message = Some(store_err.to_string());
                    return Err(AuthFlowError::TokenStore(store_err));
                }

                match service
                    .fetch_current_user(secret_token.expose_secret())
                    .await
                {
                    Ok(me) => {
                        let _ = store.save_account_token(&me.id, &secret_token);
                        Self::apply_authenticated_profile(state, &me, Some(&secret_token));
                        Ok(state.current_user.clone())
                    }
                    Err(err) => {
                        let _ = store.delete_active_token();
                        state.login_form.step = LoginStep::Mfa { ticket };
                        Err(Self::apply_login_error(state, err))
                    }
                }
            }
            Err(err) => {
                state.login_form.step = LoginStep::Mfa { ticket };
                Err(Self::apply_login_error(state, err))
            }
        }
    }

    fn apply_login_error(state: &mut AppState, err: LoginError) -> AuthFlowError {
        match err {
            LoginError::Validation(msg) => {
                state.login_form.error_message = Some(msg.clone());
                AuthFlowError::Validation(msg)
            }
            LoginError::CaptchaRequired(challenge) => {
                state.active_captcha = Some(CaptchaChallenge {
                    sitekey: challenge.captcha_sitekey.clone(),
                    service: challenge.captcha_service.clone(),
                    rqdata: challenge.captcha_rqdata.clone(),
                });
                state.login_form.error_message = Some(format!(
                    "Требуется проверка безопасности {} (sitekey: {}). Автоматический обход запрещён.",
                    challenge.captcha_service, challenge.captcha_sitekey
                ));
                AuthFlowError::CaptchaRequired(challenge)
            }
            LoginError::InvalidCredentials { code, message } => {
                state.login_form.error_message = Some(message.clone());
                AuthFlowError::InvalidCredentials { code, message }
            }
            LoginError::RateLimited {
                retry_after_ms,
                global,
            } => {
                state.login_form.error_message = Some(format!(
                    "Превышен лимит запросов (повтор через {retry_after_ms} мс)"
                ));
                AuthFlowError::RateLimited {
                    retry_after_ms,
                    global,
                }
            }
            LoginError::ServerError { status } => {
                state.login_form.error_message =
                    Some(format!("Ошибка сервера Discord (HTTP {status})"));
                AuthFlowError::ServerError { status }
            }
            LoginError::MalformedResponse(reason) => {
                state.login_form.error_message = Some(reason.clone());
                AuthFlowError::Transport(reason)
            }
            LoginError::Transport(http_err) => {
                let msg = format!("{http_err:?}");
                state.login_form.error_message = Some(msg.clone());
                AuthFlowError::Transport(msg)
            }
        }
    }
}
