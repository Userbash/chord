//! Modular Tauri v2 plugin (`auth`) encapsulating all authentication IPC commands.
//!
//! Provides `auth_status`, `auth_login_token`, `auth_restore`, `auth_logout`,
//! `auth_login_password`, `auth_mfa`, and `auth_sms` via `tauri::plugin::Builder::new("auth")`.

use std::sync::Arc;

use chord_client::auth::{self, AuthBackend, AuthError, AuthState, LoginView, SmsChallenge};
use chord_client::data;
use chord_client::net::NetService;
use tauri::Emitter;

struct TauriAuthEmitter<R: tauri::Runtime>(tauri::AppHandle<R>);

impl<R: tauri::Runtime> auth::AuthStateEmitter for TauriAuthEmitter<R> {
    fn emit_state(&self, state: &AuthState) -> Result<(), String> {
        self.0
            .emit("auth://state", state)
            .map_err(|error| format!("не удалось отправить auth://state: {error}"))
    }
}

fn emit_auth_state<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    state: &AuthState,
) -> Result<(), AuthError> {
    auth::emit_auth_state(&TauriAuthEmitter(app.clone()), state)
        .map_err(|message| AuthError::Internal { message })
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| format!("блокирующая операция не завершилась: {error}"))
}

/// Returns the current authentication state from the local keyring without network I/O.
pub async fn auth_status<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    backend: tauri::State<'_, Arc<AuthBackend>>,
) -> Result<AuthState, AuthError> {
    let backend = Arc::clone(backend.inner());
    let state = blocking(move || backend.status())
        .await
        .map_err(|message| AuthError::Internal { message })?;
    emit_auth_state(&app, &state)?;
    Ok(state)
}

/// Validates a user token via `GET /users/@me` and persists it to the keyring.
///
/// The token is never logged or returned to the frontend; only [`AuthState`] is emitted.
pub async fn auth_login_token<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    backend: tauri::State<'_, Arc<AuthBackend>>,
    token: String,
) -> Result<AuthState, AuthError> {
    let backend = Arc::clone(backend.inner());
    emit_auth_state(&app, &AuthState::validating())?;
    let state = blocking(move || {
        let st = backend.login(&token);
        let tok = backend.token().ok().flatten();
        data::set_live_session(tok, st.user.as_ref());
        st
    })
    .await
    .map_err(|message| AuthError::Internal { message })?;
    emit_auth_state(&app, &state)?;
    Ok(state)
}

/// Explicitly restores the session from the local keyring on startup.
pub async fn auth_restore<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    backend: tauri::State<'_, Arc<AuthBackend>>,
) -> Result<AuthState, AuthError> {
    let backend = Arc::clone(backend.inner());
    let state = blocking(move || {
        let st = backend.restore();
        let tok = backend.token().ok().flatten();
        data::set_live_session(tok, st.user.as_ref());
        st
    })
    .await
    .map_err(|message| AuthError::Internal { message })?;
    emit_auth_state(&app, &state)?;
    Ok(state)
}

/// Logs out by clearing the keyring, disconnecting the gateway, and resetting the live session.
pub async fn auth_logout<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    backend: tauri::State<'_, Arc<AuthBackend>>,
    net: tauri::State<'_, Arc<NetService>>,
) -> Result<AuthState, AuthError> {
    let backend = Arc::clone(backend.inner());
    let net = Arc::clone(net.inner());
    let state = blocking(move || {
        let st = backend.logout_with_disconnect(|| {
            net.disconnect()
                .map(|_| ())
                .map_err(|error| error.to_string())
        });
        data::set_live_session(None, None);
        st
    })
    .await
    .map_err(|message| AuthError::Internal { message })?;
    emit_auth_state(&app, &state)?;
    Ok(state)
}

/// Authenticates with email/phone and password (`POST /auth/login`), handling MFA and captcha challenges.
#[allow(clippy::too_many_arguments)]
pub async fn auth_login_password<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    backend: tauri::State<'_, Arc<AuthBackend>>,
    login: String,
    password: String,
    captcha_key: Option<String>,
    captcha_rqtoken: Option<String>,
    captcha_session_id: Option<String>,
    captcha_rqdata: Option<String>,
) -> Result<LoginView, AuthError> {
    emit_auth_state(&app, &AuthState::validating())?;
    let backend = Arc::clone(backend.inner());
    let state_backend = Arc::clone(&backend);
    let view = blocking(move || {
        let outcome = backend.login_password(
            &login,
            password,
            captcha_key.as_deref(),
            captcha_rqtoken.as_deref(),
            captcha_session_id.as_deref(),
            captcha_rqdata.as_deref(),
        );
        let st = backend.status();
        if st.user.is_some() {
            let tok = backend.token().ok().flatten();
            data::set_live_session(tok, st.user.as_ref());
        }
        LoginView::from_outcome(outcome, &st)
    })
    .await
    .map_err(|message| AuthError::Internal { message })?;
    emit_auth_state(&app, &state_backend.status())?;
    view
}

/// Submits a 2FA verification code (`POST /auth/mfa/{method}`).
pub async fn auth_mfa<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    backend: tauri::State<'_, Arc<AuthBackend>>,
    method: String,
    code: String,
    ticket: String,
    login_instance_id: Option<String>,
) -> Result<LoginView, AuthError> {
    emit_auth_state(&app, &AuthState::validating())?;
    let backend = Arc::clone(backend.inner());
    let state_backend = Arc::clone(&backend);
    let view = blocking(move || {
        let outcome = backend.submit_mfa(&method, code, &ticket, login_instance_id.as_deref());
        let st = backend.status();
        if st.user.is_some() {
            let tok = backend.token().ok().flatten();
            data::set_live_session(tok, st.user.as_ref());
        }
        LoginView::from_outcome(outcome, &st)
    })
    .await
    .map_err(|message| AuthError::Internal { message })?;
    emit_auth_state(&app, &state_backend.status())?;
    view
}

/// Requests an SMS 2FA verification code (`POST /auth/mfa/sms/send`).
pub async fn auth_sms(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    ticket: String,
) -> Result<SmsChallenge, AuthError> {
    let backend = Arc::clone(backend.inner());
    let challenge = blocking(move || backend.request_sms(&ticket))
        .await
        .map_err(|message| AuthError::Internal { message })?;
    challenge
}

/// Initializes the modular Tauri v2 `"auth"` plugin.
#[must_use]
pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("auth").build()
}
