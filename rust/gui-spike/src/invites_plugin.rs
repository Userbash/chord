//! Modular Tauri v2 plugin (`invites`) encapsulating Discord invite IPC commands.
//!
//! Provides `resolve_invite`, `accept_invite`, `create_channel_invite`, and `create_invite`
//! via `tauri::plugin::Builder::new("invites")`.

use std::sync::Arc;

use chord_client::auth::{AuthBackend, AuthError, IdentityContext};
use chord_client::data;
use chord_client::guilds;
use chord_client::invites::{
    self, AcceptedInvite, CreateInviteParams, CreatedInvite, DiscordInvites, InviteError,
    InvitePreview,
};

async fn token_or_invite<F>(work: F) -> Result<String, InviteError>
where
    F: FnOnce() -> Result<Option<String>, AuthError> + Send + 'static,
{
    let joined = tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| InviteError::Storage)?;
    match joined {
        Ok(Some(token)) if !token.trim().is_empty() => Ok(token),
        Ok(_) => Err(InviteError::MissingToken),
        Err(_) => Err(InviteError::Storage),
    }
}

/// Optional `params` input for `create_invite`, accepting both `camelCase` (`maxAge`, `maxUses`)
/// and `snake_case` (`max_age`, `max_uses`) fields.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInviteParamsInput {
    #[serde(default, alias = "max_age")]
    pub max_age: Option<u32>,
    #[serde(default, alias = "max_uses")]
    pub max_uses: Option<u32>,
    #[serde(default)]
    pub temporary: Option<bool>,
    #[serde(default)]
    pub flags: Option<u32>,
}

impl CreateInviteParamsInput {
    /// Resolves flat and nested invite parameters with Discord's canonical defaults.
    #[must_use]
    pub fn resolve(
        params: Option<Self>,
        max_age: Option<u32>,
        max_uses: Option<u32>,
        temporary: Option<bool>,
        flags: Option<u32>,
    ) -> CreateInviteParams {
        let defaults = CreateInviteParams::default();
        let nested = params.unwrap_or_default();
        CreateInviteParams {
            max_age: max_age.or(nested.max_age).unwrap_or(defaults.max_age),
            max_uses: max_uses.or(nested.max_uses).unwrap_or(defaults.max_uses),
            temporary: temporary.or(nested.temporary).unwrap_or(defaults.temporary),
            flags: flags.or(nested.flags),
        }
    }
}

/// Fetches an invite preview (`GET /invites/{code}?with_counts=true&with_expiration=true`)
/// using a pre-constructed [`DiscordInvites`] client.
pub async fn resolve_invite_with_api(
    api: DiscordInvites,
    code: String,
) -> Result<InvitePreview, InviteError> {
    invites::resolve_invite(&api, &code).await
}

/// Fetches an invite preview (`GET /invites/{code}?with_counts=true&with_expiration=true`).
///
/// Falls back to an anonymous client if no user token is present in the keyring.
#[allow(dead_code)]
pub async fn resolve_invite(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    code: String,
) -> Result<InvitePreview, InviteError> {
    let backend = Arc::clone(backend.inner());
    let token_res = token_or_invite(move || backend.token()).await;
    let api = match token_res {
        Ok(token) => DiscordInvites::new(&token)?,
        Err(_) => DiscordInvites::anonymous()?,
    };
    resolve_invite_with_api(api, code).await
}

/// Finalizes an `accept_invite` outcome by running Rules Screening and member-verification bypass.
pub async fn finalize_accept_invite(
    api: &DiscordInvites,
    token: &str,
    code: &str,
    outcome: Result<AcceptedInvite, InviteError>,
) -> Result<AcceptedInvite, InviteError> {
    match outcome {
        Ok(accepted) => {
            data::auto_accept_guild_rules_if_available(code, &accepted.guild_id);
            let join_transport =
                guilds::LiveGuildJoinTransport::with_accepted_guild(token, &accepted.guild_id);
            let self_uid = join_transport.current_user_id();
            if let Ok(join_outcome) = guilds::accept_invite_and_join_guild(
                &join_transport,
                code,
                &accepted.guild_id,
                &self_uid,
            ) {
                guilds::apply_guild_join_outcome(&join_outcome);
            }
            Ok(accepted)
        }
        Err(InviteError::CaptchaRequired {
            code: err_code,
            captcha_service,
            captcha_sitekey,
            captcha_rqdata,
            captcha_rqtoken,
            captcha_session_id,
        }) => {
            if let Ok(preview) = invites::resolve_invite(api, code).await {
                let gid = preview.guild_id.clone();
                let identity = IdentityContext::detect();
                if let Ok(auth) = chord_core::rest::client::ApiAuth::from_token_with_scheme(
                    token,
                    chord_core::rest::client::AuthScheme::User,
                    &identity.super_properties,
                    &identity.locale,
                    &identity.timezone,
                ) {
                    if let Ok(client) = chord_core::rest::client::ApiClient::new(
                        chord_core::rest::client::ApiConfig {
                            base: chord_core::API_V9_BASE.to_string(),
                            auth,
                            http: chord_core::rest::transport::HttpClientConfig::default(),
                            retry: chord_core::rest::client::ApiRetryPolicy::default(),
                        },
                    ) {
                        let ver_path = format!("/guilds/{gid}/member-verification");
                        if let Ok(ver_resp) = client.get(&ver_path).await {
                            if (200..300).contains(&ver_resp.status) {
                                if let Ok(ver_json) =
                                    serde_json::from_slice::<serde_json::Value>(&ver_resp.body)
                                {
                                    let accept_body =
                                        guilds::build_member_verification_accept_body(&ver_json);
                                    if let Ok(body_bytes) = serde_json::to_vec(&accept_body) {
                                        let req_path = format!("/guilds/{gid}/requests/@me");
                                        if let Ok(put_resp) =
                                            client.put(&req_path, &body_bytes).await
                                        {
                                            if (200..300).contains(&put_resp.status) {
                                                return Ok(AcceptedInvite {
                                                    guild_id: gid,
                                                    target_channel_id: preview.channel_id,
                                                    target_message_id: None,
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(InviteError::CaptchaRequired {
                code: err_code,
                captcha_service,
                captcha_sitekey,
                captcha_rqdata,
                captcha_rqtoken,
                captcha_session_id,
            })
        }
        Err(other) => Err(other),
    }
}

/// Accepts a Discord invite (`POST /invites/{code}`) using a pre-constructed [`DiscordInvites`] client,
/// handling captcha challenges and Rules Screening.
#[allow(clippy::too_many_arguments, dead_code)]
pub async fn accept_invite_with_api(
    api: DiscordInvites,
    token: String,
    code: String,
    session_id: Option<String>,
    invite_instance_id: Option<String>,
    received_installation_id: Option<String>,
    captcha_key: Option<String>,
    captcha_rqtoken: Option<String>,
    captcha_session_id: Option<String>,
) -> Result<AcceptedInvite, InviteError> {
    let effective_session_id = session_id
        .filter(|s| !s.trim().is_empty())
        .or_else(data::current_gateway_session_id);
    let outcome = if let Some(key) = captcha_key.filter(|s| !s.trim().is_empty()) {
        let solution = invites::CaptchaSolution {
            captcha_key: key,
            captcha_rqtoken: captcha_rqtoken.filter(|s| !s.trim().is_empty()),
            captcha_session_id: captcha_session_id.filter(|s| !s.trim().is_empty()),
        };
        invites::accept_invite_with_captcha(
            &api,
            &code,
            effective_session_id.as_deref(),
            invite_instance_id.as_deref(),
            received_installation_id.as_deref(),
            &solution,
        )
        .await
    } else {
        invites::accept_invite(
            &api,
            &code,
            effective_session_id.as_deref(),
            invite_instance_id.as_deref(),
            received_installation_id.as_deref(),
        )
        .await
    };
    finalize_accept_invite(&api, &token, &code, outcome).await
}

/// Accepts a Discord invite (`POST /invites/{code}`), handling captcha challenges and Rules Screening.
#[allow(clippy::too_many_arguments, dead_code)]
pub async fn accept_invite(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    code: String,
    session_id: Option<String>,
    invite_instance_id: Option<String>,
    received_installation_id: Option<String>,
    captcha_key: Option<String>,
    captcha_rqtoken: Option<String>,
    captcha_session_id: Option<String>,
) -> Result<AcceptedInvite, InviteError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or_invite(move || backend.token()).await?;
    let api = DiscordInvites::new(&token)?;
    accept_invite_with_api(
        api,
        token,
        code,
        session_id,
        invite_instance_id,
        received_installation_id,
        captcha_key,
        captcha_rqtoken,
        captcha_session_id,
    )
    .await
}

/// Creates a channel invite (`POST /channels/{channel_id}/invites`) using canonical defaults when `params` is `None`.
#[allow(dead_code)]
pub async fn create_channel_invite(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    params: Option<CreateInviteParams>,
) -> Result<CreatedInvite, InviteError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or_invite(move || backend.token()).await?;
    let api = DiscordInvites::new(&token)?;
    let params = params.unwrap_or_default();
    chord_client::create_channel_invite(&api, &channel_id, &params).await
}

/// Creates a channel invite from either flat (`maxAge`, `maxUses`, ...) or nested (`params`) IPC arguments.
#[allow(dead_code)]
pub async fn create_invite(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    params: Option<CreateInviteParamsInput>,
    max_age: Option<u32>,
    max_uses: Option<u32>,
    temporary: Option<bool>,
    flags: Option<u32>,
) -> Result<CreatedInvite, InviteError> {
    let resolved = CreateInviteParamsInput::resolve(params, max_age, max_uses, temporary, flags);
    create_channel_invite(backend, channel_id, Some(resolved)).await
}

/// Initializes the modular Tauri v2 `"invites"` plugin.
#[must_use]
pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("invites").build()
}
