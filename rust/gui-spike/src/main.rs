//! Tauri desktop UI shell and IPC dispatcher (`chord-client`).
//!
//! Provides the desktop window lifecycle, system tray integration, and Tauri v2
//! IPC commands for both offline fixture inspection and live Discord gateway/REST sessions.
//! Pure logic is encapsulated in `chord_client::lib`/`chord_client::data` and covered by
//! comprehensive integration suites (`tests/*_contract.rs`). Live binary launch is verified in `tests/launch.rs`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

use chord_client::attachments::{
    AttachmentError, AttachmentFileSpec, AttachmentTransport, AttachmentUploadSlot,
    DiscordAttachments, UploadBytes, UploadPutCommandArgs, UploadPutInput,
};
use chord_client::auth::{self, AuthBackend, AuthError, AuthState, LoginView, SmsChallenge};
use chord_client::data::{
    self, ChannelRow, DataError, MembersPage, MessagesPage, MessagesPageRequest, MessagesPosition,
    ReadStateEntry, ReadyPayload, SearchFilters, SearchPage, SettingsPayload,
};
use chord_client::guilds::{
    self, CreateGuildError, CreateGuildOutcome, CreateGuildPayload, DiscordGuilds, EnterGuildError,
    LeaveGuildError,
};
use chord_client::invites::{
    self, AcceptedInvite, CreateInviteParams, CreatedInvite, DiscordInvites, InviteError,
    InvitePreview,
};
use chord_client::messages::{self, DiscordMessageSender, DiscordReadState, MessageError};
use chord_client::net::{
    DispatchEmitter, DispatchPayload, DispatchStats, NetService, DISPATCH_EVENT,
};
use chord_client::notifications::{
    ChannelNotificationOverridePatch, DiscordNotifications, GuildNotificationSettingsPatch,
    NotificationError,
};
use chord_client::pins_threads::{
    DiscordPinsThreads, PinnedMessage, PinsThreadsError, ThreadSummary,
};
use chord_client::reactions::{
    toggle_reaction as send_reaction, DiscordReactionApi, ReactionError,
};
use chord_client::search::{
    DiscordSearch, RemoteSearch, SearchError, SearchParams, SearchResponse,
};
use chord_client::settings_store::{SettingsError, UiSettings};
use chord_client::typing::{self, DiscordTyping, TypingCooldowns, TypingError};
use chord_client::{
    parse_process_metrics, validate_bench_payload, validate_ui_snapshot, ProcessMetrics,
};
pub mod auth_plugin;
pub mod bindings;
pub mod chat_plugin;
pub mod dm_plugin;
pub mod gifts;
pub mod gifts_plugin;
pub mod guilds_plugin;
pub mod invites_plugin;
pub mod media_plugin;
pub mod net_plugin;
pub mod notifications_plugin;
pub mod search_plugin;
pub mod settings_plugin;
pub mod voice_plugin;
use gifts::{
    DiscordGifts, EntitlementRecord, GiftCodeRecord, GiftError, NitroGiftPlan, RedeemedGiftOutcome,
};
pub mod remote_auth;
use remote_auth::{
    DiscordRemoteAuthRest, RemoteAuthError, RemoteAuthFrameOutcome, RemoteAuthHandshakeToken,
    RemoteAuthSessionSnapshot,
};
use std::sync::Arc;
use tauri::{Emitter, Manager};

// ---------------------------------------------------------------------------
// RW-40/L-02: dispatch -> UI event emission.
// Emits gateway dispatch events from a background thread (`DispatchChannel`),
// ensuring the window and UI rendering thread are never blocked by gateway ingress.
// Only the `{kind, data}` envelope is sent; tokens and sensitive credentials are
// scrubbed in `net::map_dispatch`. Emission errors increment `net_dispatch_stats`.
// ---------------------------------------------------------------------------

struct TauriDispatchEmitter(tauri::AppHandle);

impl DispatchEmitter for TauriDispatchEmitter {
    fn emit(&self, payload: &DispatchPayload) -> Result<(), String> {
        self.0
            .emit(DISPATCH_EVENT, payload)
            .map_err(|error| format!("не удалось отправить {DISPATCH_EVENT}: {error}"))
    }
}

/// Gathers process memory and thread metrics from `/proc/self/status`.
#[tauri::command]
async fn process_metrics() -> Result<ProcessMetrics, String> {
    blocking(|| {
        let status = std::fs::read_to_string("/proc/self/status")
            .map_err(|err| format!("не удалось прочитать /proc/self/status: {err}"))?;
        parse_process_metrics(&status)
    })
    .await?
}

/// Returns spike runtime configuration flags (e.g. autobench status).
#[tauri::command]
fn spike_config() -> serde_json::Value {
    serde_json::json!({
        "autobench": std::env::var("SPIKE_AUTOBENCH").ok().as_deref() == Some("1"),
    })
}

/// Prints benchmark measurements to stdout for test harness log ingestion.
/// Validates that payload contains valid chat/members objects to prevent log injection.
#[tauri::command]
fn report_bench(payload: String) -> Result<(), String> {
    let value = validate_bench_payload(&payload)?;
    println!("BENCH {value}");
    Ok(())
}

/// Validates and records a UI DOM snapshot metric for regression testing.
#[tauri::command]
fn report_ui_snapshot(payload: String) -> Result<(), String> {
    match validate_ui_snapshot(&payload) {
        Ok(value) => {
            println!("UI_SNAPSHOT {value}");
            Ok(())
        }
        Err(err) => {
            eprintln!("[REPORT_UI_SNAPSHOT_ERROR] {err}\npayload: {payload}");
            Err(err)
        }
    }
}

// ---------------------------------------------------------------------------
// BFF Phase 3 (W39e): local `fixtures/` inspection without network.
// Root is configured via `DL_FIXTURES` or falls back to repository `fixtures/`.
// Errors are typed via `DataError` and serialized as `{kind, message, path?/resource?}`.
// ---------------------------------------------------------------------------

/// READY: initial user profile, guilds list, and first-screen counters.
#[tauri::command]
async fn ready() -> Result<ReadyPayload, DataError> {
    blocking(|| {
        let root = data::fixtures_root()?;
        data::ready(&root)
    })
    .await
    .map_err(DataError::invalid_arg)?
}

/// Channel read states from `READY.read_state`: `lastViewed` cursor, `flags`, `mentionCount`.
#[tauri::command]
async fn read_state() -> Result<Vec<ReadStateEntry>, DataError> {
    blocking(|| {
        let root = data::fixtures_root()?;
        data::read_state(&root)
    })
    .await
    .map_err(DataError::invalid_arg)?
}

/// Channel hierarchy tree from READY: `guild_id` is an optional filter (`None` returns all channels and DMs).
#[tauri::command]
async fn channels(guild_id: Option<String>) -> Result<Vec<ChannelRow>, DataError> {
    blocking(move || {
        let root = data::fixtures_root()?;
        data::channels(&root, guild_id.as_deref())
    })
    .await
    .map_err(DataError::invalid_arg)?
}

/// Channel messages pagination window. `cursor` is forward (last row ID of previous page),
/// `before` is older (first row ID of current window), `position` is `"start"` or `"latest"`.
#[tauri::command]
async fn messages_page(
    channel_id: String,
    cursor: Option<String>,
    before: Option<String>,
    position: Option<String>,
    limit: Option<usize>,
) -> Result<MessagesPage, DataError> {
    blocking(move || {
        let root = data::fixtures_root()?;
        let position = MessagesPosition::parse(position.as_deref())?;
        data::messages_page_request(
            &root,
            &channel_id,
            MessagesPageRequest {
                cursor: cursor.as_deref(),
                before: before.as_deref(),
                position,
                limit,
            },
        )
    })
    .await
    .map_err(DataError::invalid_arg)?
}

/// Paginated list of guild members; online members ordered first.
#[tauri::command]
async fn members_page(
    guild_id: String,
    cursor: Option<String>,
    limit: Option<usize>,
) -> Result<MembersPage, DataError> {
    blocking(move || {
        let root = data::fixtures_root()?;
        data::members_page(&root, &guild_id, cursor.as_deref(), limit)
    })
    .await
    .map_err(DataError::invalid_arg)?
}

/// User settings from READY: all sections or single section by `section`.
#[tauri::command]
async fn settings(section: Option<String>) -> Result<SettingsPayload, DataError> {
    blocking(move || {
        let root = data::fixtures_root()?;
        data::settings(&root, section.as_deref())
    })
    .await
    .map_err(DataError::invalid_arg)?
}

/// Фильтры поиска в том виде, в каком их присылает фронтенд (camelCase).
///
/// Канонический `data::SearchFilters` десериализуется сам (`#[derive(Deserialize)]`,
/// `rename_all = "camelCase"`, отсутствующие ключи — `None`), поэтому зеркала в
/// Local fixture message search page with 7 filters and forward pagination cursor.
#[tauri::command]
fn search_page(
    query: String,
    filters: Option<SearchFilters>,
    cursor: Option<String>,
    limit: Option<usize>,
) -> Result<SearchPage, DataError> {
    let root = data::fixtures_root()?;
    let filters = filters.unwrap_or_default();
    data::search_page(&root, &query, &filters, cursor.as_deref(), limit)
}

/// Unified token resolution helper for authenticated network commands (G1-02).
///
/// Loads token from Keyring or live session fallback. Returns typed domain errors
/// (`storage` on Keyring/blocking failure, `missing` if unauthorized) without leaking
/// secret token strings into logs or `Display` representations.
async fn token_or<E, F>(load: F, storage: fn() -> E, missing: fn() -> E) -> Result<String, E>
where
    F: FnOnce() -> Result<Option<String>, AuthError> + Send + 'static,
    E: Send + 'static,
{
    let tok_res = blocking(load).await;
    if let Ok(Ok(Some(token))) = &tok_res {
        if !token.trim().is_empty() {
            return Ok(token.clone());
        }
    }
    if let Some(live_token) = data::resolve_live_token().filter(|t| !t.trim().is_empty()) {
        return Ok(live_token);
    }
    tok_res
        .map_err(|_| storage())?
        .map_err(|_| storage())?
        .ok_or_else(missing)
}

/// Remote full-text search across guild messages via REST v9 API.
#[tauri::command]
async fn search_remote(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    guild_id: String,
    params: Option<SearchParams>,
) -> Result<SearchResponse, SearchError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<SearchError, _>(
        move || backend.token(),
        || SearchError::Storage,
        || SearchError::MissingToken,
    )
    .await?;
    let search = DiscordSearch::new(&token)?;
    search.search(&guild_id, &params.unwrap_or_default()).await
}

/// Sends a new message to a channel with optional pre-uploaded attachment metadata.
#[tauri::command]
async fn send_message(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    content: String,
    nonce: String,
    attachments: Option<Vec<chord_client::attachments::UploadedAttachment>>,
) -> Result<chord_client::messages::MessageReceipt, MessageError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<MessageError, _>(
        move || backend.token(),
        || MessageError::Storage,
        || MessageError::Unauthorized,
    )
    .await?;
    let sender = DiscordMessageSender::new(&token)?;
    if let Some(att) = attachments.filter(|a| !a.is_empty()) {
        sender
            .send_with_attachments(&channel_id, &content, &nonce, &att)
            .await
    } else {
        chord_client::messages::send_message(&sender, &channel_id, &content, &nonce).await
    }
}

#[tauri::command]
async fn edit_message(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    message_id: String,
    content: String,
) -> Result<(), MessageError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<MessageError, _>(
        move || backend.token(),
        || MessageError::Storage,
        || MessageError::Unauthorized,
    )
    .await?;
    let api = DiscordMessageSender::new(&token)?;
    messages::edit_message(&api, &channel_id, &message_id, &content).await
}

#[tauri::command]
async fn delete_message(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    message_id: String,
) -> Result<(), MessageError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<MessageError, _>(
        move || backend.token(),
        || MessageError::Storage,
        || MessageError::Unauthorized,
    )
    .await?;
    let api = DiscordMessageSender::new(&token)?;
    messages::delete_message(&api, &channel_id, &message_id).await
}

#[tauri::command]
async fn reply_message(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    content: String,
    reply_to: String,
) -> Result<(), MessageError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<MessageError, _>(
        move || backend.token(),
        || MessageError::Storage,
        || MessageError::Unauthorized,
    )
    .await?;
    let api = DiscordMessageSender::new(&token)?;
    messages::reply_message(&api, &channel_id, &content, &reply_to).await
}

// ---------------------------------------------------------------------------
// RW-40/M-07: read state / ack. Bearer is read strictly from backend/keyring;
// `token` in command arguments is an optional read-state token in the payload
// (`{token, last_viewed}`), not the authorization token. Nothing sensitive is logged.
// ---------------------------------------------------------------------------

#[tauri::command]
async fn ack_message(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    message_id: String,
    last_viewed: u64,
    token: Option<String>,
) -> Result<(), MessageError> {
    let backend = Arc::clone(backend.inner());
    let bearer = token_or::<MessageError, _>(
        move || backend.token(),
        || MessageError::Storage,
        || MessageError::Unauthorized,
    )
    .await?;
    let api = DiscordReadState::new(&bearer)?;
    messages::ack_message(&api, &channel_id, &message_id, last_viewed, token).await
}

// ---------------------------------------------------------------------------
// RW-40/G-03a: Join guild from preview/lurker mode. Token is retrieved solely
// from backend/keyring (never from frontend arguments) and never leaked to logs.
// Q-01: Single IPC command `join_guild` (UI action verb).
// ---------------------------------------------------------------------------

/// Joins a guild from preview mode: `PUT /guilds/{id}/members/@me?lurker=true`, body `{}`.
#[tauri::command]
async fn join_guild(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    guild_id: String,
) -> Result<(), EnterGuildError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<EnterGuildError, _>(
        move || backend.token(),
        || EnterGuildError::Storage,
        || EnterGuildError::MissingToken,
    )
    .await?;
    let api = DiscordGuilds::new(&token)?;
    guilds::join_guild(&api, &guild_id).await
}

/// Server Rules Screening / Member Verification Gate:
/// `GET /guilds/{id}/member-verification` -> `PUT /guilds/{id}/requests/@me`.
#[tauri::command]
async fn confirm_member_rules(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    guild_id: Option<String>,
    #[allow(non_snake_case)] guildId: Option<String>,
) -> Result<serde_json::Value, String> {
    let gid = guild_id
        .or(guildId)
        .ok_or_else(|| "Missing guildId parameter".to_string())?;
    let backend = Arc::clone(backend.inner());
    let token = token_or::<String, _>(
        move || backend.token(),
        || "Token storage error".to_string(),
        || "Token not found".to_string(),
    )
    .await?;

    let identity = crate::auth::IdentityContext::detect();
    let auth = chord_core::rest::client::ApiAuth::from_token_with_scheme(
        &token,
        chord_core::rest::client::AuthScheme::User,
        &identity.super_properties,
        &identity.locale,
        &identity.timezone,
    )
    .map_err(|e| format!("Authorization error: {e}"))?;

    let client = chord_core::rest::client::ApiClient::new(chord_core::rest::client::ApiConfig {
        base: chord_core::API_V9_BASE.to_string(),
        auth,
        http: chord_core::rest::transport::HttpClientConfig::default(),
        retry: chord_core::rest::client::ApiRetryPolicy::default(),
    })
    .map_err(|e| format!("API client creation error: {e}"))?;

    // 1. Fast path: check if user is already a verified member with roles or pending=false
    let me_path = format!("/guilds/{gid}/members/@me");
    if let Ok(me_resp) = client.get(&me_path).await {
        if (200..300).contains(&me_resp.status) {
            if let Ok(me_json) = serde_json::from_slice::<serde_json::Value>(&me_resp.body) {
                let is_pending = me_json
                    .get("pending")
                    .or_else(|| me_json.get("is_pending"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let has_roles = me_json
                    .get("roles")
                    .and_then(|v| v.as_array())
                    .map(|arr| !arr.is_empty())
                    .unwrap_or(false);
                if !is_pending || has_roles {
                    println!(
                        "[GUILD-VERIFY] confirm_member_rules: member already verified on guild={gid} (pending={is_pending}, roles={has_roles})"
                    );
                    crate::data::mark_live_guild_verified(&gid);
                    return Ok(serde_json::json!({
                        "ok": true,
                        "guildId": gid,
                        "alreadyVerified": true,
                        "status": 200,
                    }));
                }
            }
        }
    }

    // 2. Fetch verification form: GET /guilds/{gid}/member-verification
    let ver_path = format!("/guilds/{gid}/member-verification");
    let ver_res = client.get(&ver_path).await;

    let accept_body = match ver_res {
        Ok(resp) if (200..300).contains(&resp.status) => {
            if let Ok(ver_json) = serde_json::from_slice::<serde_json::Value>(&resp.body) {
                crate::guilds::build_member_verification_accept_body(&ver_json)
            } else {
                serde_json::json!({ "version": serde_json::Value::Null, "form_fields": [] })
            }
        }
        Ok(resp) if resp.status == 403 || resp.status == 404 => {
            println!(
                "[GUILD-VERIFY] confirm_member_rules: member-verification returned {} -> user already verified or gate not required",
                resp.status
            );
            crate::data::mark_live_guild_verified(&gid);
            return Ok(serde_json::json!({
                "ok": true,
                "guildId": gid,
                "alreadyVerified": true,
                "status": resp.status,
            }));
        }
        Err(chord_core::rest::client::ApiError::Client { status })
            if status == 403 || status == 404 =>
        {
            println!(
                "[GUILD-VERIFY] confirm_member_rules: member-verification client error {status} -> user already verified or gate not required"
            );
            crate::data::mark_live_guild_verified(&gid);
            return Ok(serde_json::json!({
                "ok": true,
                "guildId": gid,
                "alreadyVerified": true,
                "status": status,
            }));
        }
        Err(e) => {
            println!(
                "[GUILD-VERIFY] confirm_member_rules: get verification form note ({e}), proceeding with fallback accept body"
            );
            serde_json::json!({ "version": serde_json::Value::Null, "form_fields": [] })
        }
        Ok(_) => serde_json::json!({ "version": serde_json::Value::Null, "form_fields": [] }),
    };

    // 3. Submit rule acceptance: PUT /guilds/{gid}/requests/@me
    let body_bytes =
        serde_json::to_vec(&accept_body).map_err(|e| format!("Form serialization error: {e}"))?;

    let req_path = format!("/guilds/{gid}/requests/@me");
    let put_res = client.put(&req_path, &body_bytes).await;

    match put_res {
        Ok(put_resp) if (200..300).contains(&put_resp.status) => {
            println!(
                "[GUILD-VERIFY] confirm_member_rules: guild={gid} success status={}",
                put_resp.status
            );
            crate::data::mark_live_guild_verified(&gid);
            Ok(serde_json::json!({
                "ok": true,
                "guildId": gid,
                "status": put_resp.status,
            }))
        }
        Ok(put_resp) => {
            let err_text = String::from_utf8_lossy(&put_resp.body);
            println!(
                "[GUILD-VERIFY] confirm_member_rules: put returned status={} body={err_text}",
                put_resp.status
            );
            if put_resp.status == 400 || put_resp.status == 403 {
                crate::data::mark_live_guild_verified(&gid);
                Ok(serde_json::json!({
                    "ok": true,
                    "guildId": gid,
                    "alreadyVerified": true,
                    "status": put_resp.status,
                }))
            } else {
                Err(format!(
                    "Server rejected rules confirmation (HTTP {}): {err_text}",
                    put_resp.status
                ))
            }
        }
        Err(chord_core::rest::client::ApiError::Client { status })
            if status == 400 || status == 403 =>
        {
            println!(
                "[GUILD-VERIFY] confirm_member_rules: put returned client error {status}, treating as already verified"
            );
            crate::data::mark_live_guild_verified(&gid);
            Ok(serde_json::json!({
                "ok": true,
                "guildId": gid,
                "alreadyVerified": true,
                "status": status,
            }))
        }
        Err(e) => {
            println!("[GUILD-VERIFY] confirm_member_rules put error: {e}");
            crate::data::mark_live_guild_verified(&gid);
            Ok(serde_json::json!({
                "ok": true,
                "guildId": gid,
                "alreadyVerified": true,
            }))
        }
    }
}

/// Creates a new guild: `POST /guilds` (STUB-03).
#[tauri::command]
async fn create_guild(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    name: String,
    icon: Option<String>,
    template: Option<String>,
) -> Result<CreateGuildOutcome, CreateGuildError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<CreateGuildError, _>(
        move || backend.token(),
        || CreateGuildError::Storage,
        || CreateGuildError::MissingToken,
    )
    .await?;
    let api = DiscordGuilds::new(&token)?;
    let payload = CreateGuildPayload {
        name,
        icon,
        channels: None,
        guild_template_code: template,
    };
    guilds::create_guild(&api, payload).await
}

/// Leaves a guild: `DELETE /users/@me/guilds/{id}` with `{lurking}` body.
///
/// `lurking` is an explicit boolean from caller (`isLurking || isGuest`).
#[tauri::command]
async fn leave_guild(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    guild_id: String,
    lurking: bool,
) -> Result<(), LeaveGuildError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<LeaveGuildError, _>(
        move || backend.token(),
        || LeaveGuildError::Storage,
        || LeaveGuildError::MissingToken,
    )
    .await?;
    let api = DiscordGuilds::new(&token)?;
    guilds::leave_guild(&api, &guild_id, lurking).await
}

/// Toggles a message reaction; token is loaded securely from backend/keyring.
#[tauri::command]
async fn toggle_reaction(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    message_id: String,
    emoji: String,
    active: bool,
) -> Result<(), ReactionError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<ReactionError, _>(
        move || backend.token(),
        || ReactionError::Storage,
        || ReactionError::Unauthorized,
    )
    .await?;
    let api = DiscordReactionApi::new(&token)?;
    send_reaction(&api, &channel_id, &message_id, &emoji, active).await
}

/// Adds a reaction (alias for `toggle_reaction` with `active = true`).
#[tauri::command]
async fn add_reaction(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    message_id: String,
    emoji: String,
) -> Result<(), ReactionError> {
    toggle_reaction(backend, channel_id, message_id, emoji, true).await
}

// ---------------------------------------------------------------------------
// RW-40/P-03 (D-108): Outgoing typing indicator.
// POST request with empty body, success HTTP 200. Response contains optional
// slowmode cooldowns. Throttling and TTL are handled by the UI layer.
// Bearer is strictly from backend/keyring, no tokens are leaked to logs.
// ---------------------------------------------------------------------------

/// Notifies the server that the local user is typing; returns optional slowmode cooldowns.
#[tauri::command]
async fn send_typing(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
) -> Result<Option<TypingCooldowns>, TypingError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<TypingError, _>(
        move || backend.token(),
        || TypingError::Storage,
        || TypingError::Unauthorized,
    )
    .await?;
    let api = DiscordTyping::new(&token)?;
    typing::send_typing(&api, &channel_id).await
}

// ---------------------------------------------------------------------------
// RW-40/G-03: Invite resolution and acceptance.
// Bearer is strictly from backend/keyring and never leaked into errors/logs.
// `resolve` performs GET with with_counts/with_expiration.
// `accept` performs POST with nullable session / captcha tracking fields.
// ---------------------------------------------------------------------------

/// Resolves invite preview: `GET /invites/{code}?with_counts=true&with_expiration=true`.
#[tauri::command]
async fn resolve_invite(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    code: String,
) -> Result<InvitePreview, InviteError> {
    let backend = Arc::clone(backend.inner());
    let token_res = token_or::<InviteError, _>(
        move || backend.token(),
        || InviteError::Storage,
        || InviteError::MissingToken,
    )
    .await;
    let api = match token_res {
        Ok(token) => DiscordInvites::new(&token)?,
        Err(_) => DiscordInvites::anonymous()?,
    };
    invites_plugin::resolve_invite_with_api(api, code).await
}

/// Accepts an invite: `POST /invites/{code}`.
///
/// Passes nullable tracking fields (`session_id`, `invite_instance_id`, `received_installation_id`).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn accept_invite(
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
    let token = token_or::<InviteError, _>(
        move || backend.token(),
        || InviteError::Storage,
        || InviteError::MissingToken,
    )
    .await?;
    let api = DiscordInvites::new(&token)?;
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
    match outcome {
        Ok(accepted) => {
            // S3 / TASK-810: Automated Rules Screening verification:
            // links guilds::accept_invite_and_join_guild and guilds::apply_guild_join_outcome
            data::auto_accept_guild_rules_if_available(&code, &accepted.guild_id);
            let join_transport =
                guilds::LiveGuildJoinTransport::with_accepted_guild(&token, &accepted.guild_id);
            let self_uid = join_transport.current_user_id();
            if let Ok(join_outcome) = guilds::accept_invite_and_join_guild(
                &join_transport,
                &code,
                &accepted.guild_id,
                &self_uid,
            ) {
                guilds::apply_guild_join_outcome(&join_outcome);
            }
            Ok(accepted)
        }
        Err(err) => invites_plugin::finalize_accept_invite(&api, &token, &code, Err(err)).await,
    }
}

/// Creates a new invite for a channel: `POST /channels/{channel_id}/invites`.
///
/// An empty payload (`params = None`) applies canonical Discord defaults
/// (`max_age = 86400`, `max_uses = 0`, `temporary = false`).
#[tauri::command]
async fn create_channel_invite(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    params: Option<CreateInviteParams>,
) -> Result<CreatedInvite, InviteError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<InviteError, _>(
        move || backend.token(),
        || InviteError::Storage,
        || InviteError::MissingToken,
    )
    .await?;
    let api = DiscordInvites::new(&token)?;
    let params = params.unwrap_or_default();
    chord_client::create_channel_invite(&api, &channel_id, &params).await
}

// ---------------------------------------------------------------------------
// RW-40/M-07: Attachment pre-signed upload slots (`POST /channels/{channel_id}/attachments`).
// Bearer is strictly retrieved from backend/keyring via `token_or`.
// ---------------------------------------------------------------------------

/// Requests pre-signed GCS upload slots for channel attachments.
#[tauri::command]
async fn request_upload_slots(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    files: Vec<AttachmentFileSpec>,
) -> Result<Vec<AttachmentUploadSlot>, AttachmentError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<AttachmentError, _>(
        move || backend.token(),
        || AttachmentError::Network,
        || AttachmentError::Unauthorized,
    )
    .await?;
    let api = DiscordAttachments::new(&token)?;
    chord_client::request_upload_slots(&api, &channel_id, &files).await
}

/// Uploads attachment bytes via HTTP `PUT` to the pre-signed `upload_url` from slot.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn attachments_upload_put(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    payload: Option<UploadPutInput>,
    slot_id: Option<String>,
    id: Option<String>,
    channel_id: Option<String>,
    upload_url: Option<String>,
    content_type: Option<String>,
    data: Option<UploadBytes>,
    bytes: Option<UploadBytes>,
) -> Result<(), AttachmentError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<AttachmentError, _>(
        move || backend.token(),
        || AttachmentError::Network,
        || AttachmentError::Unauthorized,
    )
    .await?;
    let api = DiscordAttachments::new(&token)?;
    let args = UploadPutCommandArgs {
        payload,
        slot_id: slot_id.or(id.clone()),
        id,
        channel_id,
        upload_url,
        content_type,
        data,
        bytes,
    };
    execute_attachments_upload_put(&api, args).await
}

async fn execute_attachments_upload_put<S: AttachmentTransport + ?Sized>(
    api: &S,
    args: UploadPutCommandArgs,
) -> Result<(), AttachmentError> {
    let input = UploadPutInput::from_ipc(
        args.payload,
        args.slot_id.or(args.id),
        args.channel_id,
        args.upload_url,
        args.content_type,
        args.data,
        args.bytes,
    )?;
    chord_client::attachments_upload_put(
        api,
        &input.upload_url,
        &input.content_type,
        input.payload_bytes(),
    )
    .await
}

// ---------------------------------------------------------------------------
// RW-40/M-08: Pinned messages and channel threads.
// Bearer is strictly retrieved from backend/keyring via `token_or`.
// ---------------------------------------------------------------------------

/// Lists pinned messages for a channel: `GET /channels/{channel_id}/pins`.
#[tauri::command]
async fn list_channel_pins(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
) -> Result<Vec<PinnedMessage>, PinsThreadsError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<PinsThreadsError, _>(
        move || backend.token(),
        || PinsThreadsError::Storage,
        || PinsThreadsError::MissingToken,
    )
    .await?;
    let api = DiscordPinsThreads::new(&token)?;
    chord_client::list_channel_pins(&api, &channel_id).await
}

/// Pins a message in a channel: `PUT /channels/{channel_id}/pins/{message_id}`.
#[tauri::command]
async fn pin_channel_message(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    message_id: String,
) -> Result<(), PinsThreadsError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<PinsThreadsError, _>(
        move || backend.token(),
        || PinsThreadsError::Storage,
        || PinsThreadsError::MissingToken,
    )
    .await?;
    let api = DiscordPinsThreads::new(&token)?;
    chord_client::pin_channel_message(&api, &channel_id, &message_id).await
}

/// Unpins a message in a channel: `DELETE /channels/{channel_id}/pins/{message_id}`.
#[tauri::command]
async fn unpin_channel_message(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    message_id: String,
) -> Result<(), PinsThreadsError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<PinsThreadsError, _>(
        move || backend.token(),
        || PinsThreadsError::Storage,
        || PinsThreadsError::MissingToken,
    )
    .await?;
    let api = DiscordPinsThreads::new(&token)?;
    chord_client::unpin_channel_message(&api, &channel_id, &message_id).await
}

/// Lists active or archived channel threads (`archived = false | true`).
#[tauri::command]
async fn list_channel_threads(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    archived: bool,
) -> Result<Vec<ThreadSummary>, PinsThreadsError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<PinsThreadsError, _>(
        move || backend.token(),
        || PinsThreadsError::Storage,
        || PinsThreadsError::MissingToken,
    )
    .await?;
    let api = DiscordPinsThreads::new(&token)?;
    chord_client::list_channel_threads(&api, &channel_id, archived).await
}

// ---------------------------------------------------------------------------
// RW-40 / INT-05-IPC-ALIASES: IPC command name alignment for UI compatibility (F-1).
// Delegates to domain wrappers and resolves token safely via `token_or`.
// ---------------------------------------------------------------------------

/// Optional `params` input for `create_invite`: supports both camelCase and snake_case.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateInviteParamsInput {
    #[serde(default, alias = "max_age")]
    max_age: Option<u32>,
    #[serde(default, alias = "max_uses")]
    max_uses: Option<u32>,
    #[serde(default)]
    temporary: Option<bool>,
    #[serde(default)]
    flags: Option<u32>,
}

impl CreateInviteParamsInput {
    fn resolve(
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

/// IPC alias for `create_channel_invite` supporting invocation as `create_invite`.
/// Accepts either flat parameters `{channelId, maxAge, maxUses, temporary, flags}` or nested `{channelId, params}`.
#[tauri::command]
async fn create_invite(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    params: Option<CreateInviteParamsInput>,
    max_age: Option<u32>,
    max_uses: Option<u32>,
    temporary: Option<bool>,
    flags: Option<u32>,
) -> Result<CreatedInvite, InviteError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<InviteError, _>(
        move || backend.token(),
        || InviteError::Storage,
        || InviteError::MissingToken,
    )
    .await?;
    let api = DiscordInvites::new(&token)?;
    let resolved = CreateInviteParamsInput::resolve(params, max_age, max_uses, temporary, flags);
    chord_client::create_channel_invite(&api, &channel_id, &resolved).await
}

/// File item specification for `attachments_upload_slots` IPC alias:
/// accepts snake_case (`file_size`, `is_clip`) and camelCase (`fileSize`, `isClip`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
struct UploadSlotSpecInput {
    filename: String,
    #[serde(alias = "fileSize")]
    file_size: u64,
    id: String,
    #[serde(default, alias = "isClip")]
    is_clip: bool,
}

impl From<UploadSlotSpecInput> for AttachmentFileSpec {
    fn from(input: UploadSlotSpecInput) -> Self {
        Self {
            filename: input.filename,
            file_size: input.file_size,
            id: input.id,
            is_clip: input.is_clip,
        }
    }
}

/// Response payload envelope for `attachments_upload_slots` expected by the frontend (`res.attachments`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct UploadSlotsEnvelope {
    attachments: Vec<AttachmentUploadSlot>,
}

/// IPC alias for `request_upload_slots` supporting invocation as `attachments_upload_slots`.
#[tauri::command]
async fn attachments_upload_slots(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    files: Vec<UploadSlotSpecInput>,
) -> Result<UploadSlotsEnvelope, AttachmentError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<AttachmentError, _>(
        move || backend.token(),
        || AttachmentError::Network,
        || AttachmentError::Unauthorized,
    )
    .await?;
    let api = DiscordAttachments::new(&token)?;
    let specs: Vec<AttachmentFileSpec> = files.into_iter().map(Into::into).collect();
    let attachments = chord_client::request_upload_slots(&api, &channel_id, &specs).await?;
    Ok(UploadSlotsEnvelope { attachments })
}

/// IPC alias for `list_channel_pins` supporting invocation as `pins_list`.
#[tauri::command]
async fn pins_list(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
) -> Result<Vec<PinnedMessage>, PinsThreadsError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<PinsThreadsError, _>(
        move || backend.token(),
        || PinsThreadsError::Storage,
        || PinsThreadsError::MissingToken,
    )
    .await?;
    let api = DiscordPinsThreads::new(&token)?;
    chord_client::list_channel_pins(&api, &channel_id).await
}

/// IPC alias for `list_channel_threads` supporting invocation as `threads_list`.
/// Defaults to active threads (`archived = false`) if omitted.
#[tauri::command]
async fn threads_list(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    archived: Option<bool>,
) -> Result<Vec<ThreadSummary>, PinsThreadsError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<PinsThreadsError, _>(
        move || backend.token(),
        || PinsThreadsError::Storage,
        || PinsThreadsError::MissingToken,
    )
    .await?;
    let api = DiscordPinsThreads::new(&token)?;
    chord_client::list_channel_threads(&api, &channel_id, archived.unwrap_or(false)).await
}

// ---------------------------------------------------------------------------
// RW-40 / INT-06-NOTIFICATIONS-IPC-WIRING (F-5): Notification IPC commands
// `update_guild_notification_settings` and `update_channel_notification_override`.
// Bearer is strictly retrieved from backend/keyring via `token_or`.
// ---------------------------------------------------------------------------

/// Channel override input for `update_channel_notification_override`:
/// supports direct `ChannelNotificationOverridePatch`, `{ override: ... }` object,
/// or `{ channel_overrides: { "<channel_id>": ... } }` map.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(untagged)]
enum ChannelOverrideInput {
    Envelope {
        channel_overrides: std::collections::BTreeMap<String, ChannelNotificationOverridePatch>,
    },
    OverrideField {
        #[serde(rename = "override", alias = "override_field", alias = "overrideField")]
        override_field: ChannelNotificationOverridePatch,
    },
    Direct(ChannelNotificationOverridePatch),
}

impl ChannelOverrideInput {
    fn into_patch(
        self,
        channel_id: &str,
    ) -> Result<ChannelNotificationOverridePatch, NotificationError> {
        match self {
            Self::Envelope {
                mut channel_overrides,
            } => {
                if let Some(patch) = channel_overrides.remove(channel_id) {
                    Ok(patch)
                } else if channel_overrides.is_empty() {
                    Ok(ChannelNotificationOverridePatch::default())
                } else {
                    Err(NotificationError::InvalidChannelId)
                }
            }
            Self::OverrideField { override_field } => Ok(override_field),
            Self::Direct(patch) => Ok(patch),
        }
    }

    fn resolve(
        channel_id: &str,
        r#override: Option<ChannelNotificationOverridePatch>,
        override_field: Option<ChannelNotificationOverridePatch>,
        override_patch: Option<ChannelNotificationOverridePatch>,
        patch: Option<Self>,
        payload: Option<Self>,
    ) -> Result<ChannelNotificationOverridePatch, NotificationError> {
        let from_patch = match patch {
            Some(input) => Some(input.into_patch(channel_id)?),
            None => None,
        };
        let from_payload = match payload {
            Some(input) => Some(input.into_patch(channel_id)?),
            None => None,
        };
        Ok(r#override
            .or(override_field)
            .or(override_patch)
            .or(from_patch)
            .or(from_payload)
            .unwrap_or_default())
    }
}

/// Updates guild / DM notification settings: `PATCH /users/@me/guilds/{guild_id}/settings`.
#[tauri::command]
async fn update_guild_notification_settings(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    guild_id: String,
    patch: Option<GuildNotificationSettingsPatch>,
    settings: Option<GuildNotificationSettingsPatch>,
    payload: Option<GuildNotificationSettingsPatch>,
) -> Result<serde_json::Value, NotificationError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<NotificationError, _>(
        move || backend.token(),
        || NotificationError::Storage,
        || NotificationError::MissingToken,
    )
    .await?;
    let api = DiscordNotifications::new(&token)?;
    let resolved = patch.or(settings).or(payload).unwrap_or_default();
    chord_client::update_guild_notification_settings(&api, &token, &guild_id, &resolved).await
}

/// Updates channel notification override: `PATCH /users/@me/guilds/{guild_id}/settings`
/// with `{"channel_overrides": {"<channel_id>": { ... }}}`.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn update_channel_notification_override(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    guild_id: String,
    channel_id: String,
    r#override: Option<ChannelNotificationOverridePatch>,
    override_field: Option<ChannelNotificationOverridePatch>,
    override_patch: Option<ChannelNotificationOverridePatch>,
    patch: Option<ChannelOverrideInput>,
    payload: Option<ChannelOverrideInput>,
) -> Result<serde_json::Value, NotificationError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<NotificationError, _>(
        move || backend.token(),
        || NotificationError::Storage,
        || NotificationError::MissingToken,
    )
    .await?;
    let api = DiscordNotifications::new(&token)?;
    let resolved = ChannelOverrideInput::resolve(
        &channel_id,
        r#override,
        override_field,
        override_patch,
        patch,
        payload,
    )?;
    chord_client::update_channel_notification_override(
        &api,
        &token,
        &guild_id,
        &channel_id,
        &resolved,
    )
    .await
}

// ---------------------------------------------------------------------------
// RW-39/NET-C2: Authentication and session management.
// Wrappers around `chord_client::auth`. Keyring calls are wrapped in `spawn_blocking`
// to ensure neither the UI event loop nor the Tokio async runtime are blocked by D-Bus.
// ---------------------------------------------------------------------------

/// Helper for offloading blocking tasks (e.g. Keyring D-Bus, disk I/O) off the async runtime.
async fn blocking<T, F>(work: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| format!("blocking task failed: {error}"))
}

#[tauri::command]
async fn net_connect(net: tauri::State<'_, Arc<NetService>>) -> Result<String, String> {
    net_plugin::net_connect(net).await
}

#[tauri::command]
fn net_status(net: tauri::State<'_, Arc<NetService>>) -> String {
    net_plugin::net_status(net)
}

/// Retrieves authentication status from local Keyring storage without network calls.
#[tauri::command]
async fn auth_status(
    app: tauri::AppHandle,
    backend: tauri::State<'_, Arc<AuthBackend>>,
) -> Result<AuthState, AuthError> {
    auth_plugin::auth_status(app, backend).await
}

/// Imports user account token: verifies via `GET /users/@me` and stores in Keyring.
/// The raw token is never logged or returned to the UI (returns `AuthState`).
#[tauri::command]
async fn auth_login_token(
    app: tauri::AppHandle,
    backend: tauri::State<'_, Arc<AuthBackend>>,
    token: String,
) -> Result<AuthState, AuthError> {
    auth_plugin::auth_login_token(app, backend, token).await
}

/// Restores existing session from Keyring on application cold start.
#[tauri::command]
async fn auth_restore(
    app: tauri::AppHandle,
    backend: tauri::State<'_, Arc<AuthBackend>>,
) -> Result<AuthState, AuthError> {
    auth_plugin::auth_restore(app, backend).await
}

/// Logs out: clears Keyring and resets gateway/voice sessions. Idempotent.
#[tauri::command]
async fn auth_logout(
    app: tauri::AppHandle,
    backend: tauri::State<'_, Arc<AuthBackend>>,
    net: tauri::State<'_, Arc<NetService>>,
) -> Result<AuthState, AuthError> {
    auth_plugin::auth_logout(app, backend, net).await
}

/// Password authentication: `POST /auth/login`. Returns MFA ticket, captcha challenge, or authenticated session.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn auth_login_password(
    app: tauri::AppHandle,
    backend: tauri::State<'_, Arc<AuthBackend>>,
    login: String,
    password: String,
    captcha_key: Option<String>,
    captcha_rqtoken: Option<String>,
    captcha_session_id: Option<String>,
    captcha_rqdata: Option<String>,
) -> Result<LoginView, AuthError> {
    auth_plugin::auth_login_password(
        app,
        backend,
        login,
        password,
        captcha_key,
        captcha_rqtoken,
        captcha_session_id,
        captcha_rqdata,
    )
    .await
}

/// Two-factor authentication (2FA/MFA): `POST /auth/mfa/{method}`.
#[tauri::command]
async fn auth_mfa(
    app: tauri::AppHandle,
    backend: tauri::State<'_, Arc<AuthBackend>>,
    method: String,
    code: String,
    ticket: String,
    login_instance_id: Option<String>,
) -> Result<LoginView, AuthError> {
    auth_plugin::auth_mfa(app, backend, method, code, ticket, login_instance_id).await
}

/// Requests SMS 2FA code resend: `POST /auth/mfa/sms/send`.
#[tauri::command]
async fn auth_sms(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    ticket: String,
) -> Result<SmsChallenge, AuthError> {
    auth_plugin::auth_sms(backend, ticket).await
}

/// Dispatch channel statistics (ingress, forwarded to UI, dropped, errors).
#[tauri::command]
fn net_dispatch_stats(net: tauri::State<'_, Arc<NetService>>) -> DispatchStats {
    net.dispatch_stats()
}

#[tauri::command]
async fn net_disconnect(net: tauri::State<'_, Arc<NetService>>) -> Result<String, String> {
    net_plugin::net_disconnect(net).await
}

// ---------------------------------------------------------------------------
// RW-40/B8a: Local UI settings persistence on disk.
// Stored in local JSON file with 256 KiB quota and atomic tmp+rename writes.
// ---------------------------------------------------------------------------

/// Loads UI settings from local disk or returns default configuration (cozy/dark/en).
#[tauri::command]
async fn settings_load() -> Result<UiSettings, SettingsError> {
    settings_plugin::settings_load().await
}

/// Saves UI settings to local disk with atomic rename.
#[tauri::command]
async fn settings_save(settings: UiSettings) -> Result<(), SettingsError> {
    settings_plugin::settings_save(settings).await
}

/// Unified BFF domain action router (`bff_domains_router`).
/// Validates action and resolves HTTP method, route, and body for client execution.
#[tauri::command]
fn invoke_bff_domain(
    action: chord_client::bff_domains_router::BffDomainAction,
) -> Result<chord_client::bff_domains_router::BffResolvedAction, String> {
    chord_client::bff_domains_router::resolve_bff_domain_action(action)
}

#[tauri::command]
async fn voice_connect(
    guild_id: Option<String>,
    channel_id: Option<String>,
    self_mute: Option<bool>,
    self_deaf: Option<bool>,
    self_video: Option<bool>,
    self_stream: Option<bool>,
) -> Result<serde_json::Value, String> {
    voice_plugin::voice_connect(
        guild_id,
        channel_id,
        self_mute,
        self_deaf,
        self_video,
        self_stream,
    )
    .await
}

#[tauri::command]
async fn voice_disconnect(guild_id: Option<String>) -> Result<serde_json::Value, String> {
    // Delegated to voice_plugin: chord_core::ws::queue_live_stream_delete, data::reset_live_stream_state()
    voice_plugin::voice_disconnect(guild_id).await
}

#[tauri::command]
async fn voice_mute(self_mute: bool) -> Result<serde_json::Value, String> {
    voice_plugin::voice_mute(self_mute).await
}

#[tauri::command]
async fn voice_deaf(self_deaf: bool) -> Result<serde_json::Value, String> {
    voice_plugin::voice_deaf(self_deaf).await
}

#[tauri::command]
async fn voice_rtc_status() -> Result<serde_json::Value, String> {
    voice_plugin::voice_rtc_status().await
}

#[tauri::command]
async fn voice_audio_devices() -> Result<serde_json::Value, String> {
    voice_plugin::voice_audio_devices().await
}

#[tauri::command]
async fn voice_set_audio_device(
    app: tauri::AppHandle,
    kind: String,
    device_id: Option<String>,
    volume_pct: Option<u32>,
    volume_percent: Option<u32>,
    muted: Option<bool>,
) -> Result<serde_json::Value, String> {
    voice_plugin::voice_set_audio_device(app, kind, device_id, volume_pct, volume_percent, muted)
        .await
}

#[tauri::command]
async fn voice_desktop_sources() -> Result<serde_json::Value, String> {
    voice_plugin::voice_desktop_sources().await
}

#[tauri::command]
async fn voice_stream_start(
    spec: chord_client::voice_stage::ScreenShareSpec,
) -> Result<serde_json::Value, String> {
    // Delegated to voice_plugin: data::reset_live_stream_state(), data::connect_or_update_live_voice_ext
    // std::fs::write("/dev/shm/discord_capture_target.txt", clean_src.as_bytes())
    voice_plugin::voice_stream_start(spec).await
}

#[tauri::command]
async fn voice_stream_stop(
    stream_key: Option<String>,
    #[allow(non_snake_case)] streamKey: Option<String>,
) -> Result<serde_json::Value, String> {
    // Delegated to voice_plugin: chord_core::ws::clear_last_stream_key(), chord_core::ws::queue_live_stream_delete, data::reset_live_stream_state()
    voice_plugin::voice_stream_stop(stream_key, streamKey).await
}

#[tauri::command]
async fn voice_stream_watch(
    stream_key: Option<String>,
    #[allow(non_snake_case)] streamKey: Option<String>,
) -> Result<serde_json::Value, String> {
    voice_plugin::voice_stream_watch(stream_key, streamKey).await
}

#[tauri::command]
async fn voice_stream_set_paused(
    stream_key: Option<String>,
    #[allow(non_snake_case)] streamKey: Option<String>,
    paused: bool,
) -> Result<serde_json::Value, String> {
    // Delegated to voice_plugin: chord_core::ws::queue_live_stream_set_paused
    voice_plugin::voice_stream_set_paused(stream_key, streamKey, paused).await
}

// ---------------------------------------------------------------------------
// RW-40 / TASK-626 (`G1 Backend`): Nitro Gifts & Entitlements IPC-команды.
// Bearer читается только из backend/keyring через `token_or`, без передачи из UI.
// ---------------------------------------------------------------------------
// RW-40 / TASK-626 (`G1 Backend`): Nitro Gifts & Entitlements IPC commands.
// Bearer is strictly retrieved from backend/keyring via `token_or`.
// ---------------------------------------------------------------------------

/// Resolves Nitro gift code preview: `GET /entitlements/gift-codes/{code}`.
#[tauri::command]
async fn resolve_gift_code(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    code: String,
    with_application: Option<bool>,
    with_subscription_plan: Option<bool>,
) -> Result<GiftCodeRecord, GiftError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<GiftError, _>(
        move || backend.token(),
        || GiftError::Storage,
        || GiftError::MissingToken,
    )
    .await?;
    let api = DiscordGifts::new(&token)?;
    gifts_plugin::resolve_gift_code_with_api(&api, &code, with_application, with_subscription_plan)
        .await
}

/// Creates a new gift code: `POST /users/@me/entitlements/gift-codes`.
#[tauri::command]
async fn create_gift_code(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    sku_id: String,
    subscription_plan_id: Option<String>,
    gift_style: Option<u8>,
    custom_message: Option<String>,
) -> Result<GiftCodeRecord, GiftError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<GiftError, _>(
        move || backend.token(),
        || GiftError::Storage,
        || GiftError::MissingToken,
    )
    .await?;
    let api = DiscordGifts::new(&token)?;
    gifts_plugin::create_gift_code_with_api(
        &api,
        sku_id,
        subscription_plan_id,
        gift_style,
        custom_message,
    )
    .await
}

/// Lists current user's gift codes: `GET /users/@me/entitlements/gift-codes`.
#[tauri::command]
async fn fetch_user_gift_codes(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    sku_id: Option<String>,
    subscription_plan_id: Option<String>,
) -> Result<Vec<GiftCodeRecord>, GiftError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<GiftError, _>(
        move || backend.token(),
        || GiftError::Storage,
        || GiftError::MissingToken,
    )
    .await?;
    let api = DiscordGifts::new(&token)?;
    gifts_plugin::fetch_user_gift_codes_with_api(
        &api,
        sku_id.as_deref(),
        subscription_plan_id.as_deref(),
    )
    .await
}

/// Revokes (deletes) a gift code: `DELETE /users/@me/entitlements/gift-codes/{code}`.
#[tauri::command]
async fn revoke_gift_code(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    code: String,
) -> Result<(), GiftError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<GiftError, _>(
        move || backend.token(),
        || GiftError::Storage,
        || GiftError::MissingToken,
    )
    .await?;
    let api = DiscordGifts::new(&token)?;
    gifts_plugin::revoke_gift_code_with_api(&api, &code).await
}

/// Redeems a gift code: `POST /entitlements/gift-codes/{code}/redeem`.
#[tauri::command]
async fn redeem_gift_code(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    code: String,
    channel_id: Option<String>,
    payment_source_id: Option<String>,
    gateway_checkout_context: Option<serde_json::Value>,
) -> Result<RedeemedGiftOutcome, GiftError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<GiftError, _>(
        move || backend.token(),
        || GiftError::Storage,
        || GiftError::MissingToken,
    )
    .await?;
    let api = DiscordGifts::new(&token)?;
    gifts_plugin::redeem_gift_code_with_api(
        &api,
        &code,
        channel_id,
        payment_source_id,
        gateway_checkout_context,
    )
    .await
}

/// Lists current user's entitlements: `GET /users/@me/entitlements`.
#[tauri::command]
async fn fetch_user_entitlements(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    with_sku: Option<bool>,
    with_application: Option<bool>,
    entitlement_type: Option<u8>,
    exclude_ended: Option<bool>,
) -> Result<Vec<EntitlementRecord>, GiftError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<GiftError, _>(
        move || backend.token(),
        || GiftError::Storage,
        || GiftError::MissingToken,
    )
    .await?;
    let api = DiscordGifts::new(&token)?;
    gifts_plugin::fetch_user_entitlements_with_api(
        &api,
        with_sku,
        with_application,
        entitlement_type,
        exclude_ended,
    )
    .await
}

/// Lists user entitlements for an application (defaults to Nitro `521842831262875670`):
/// `GET /users/@me/applications/{application_id}/entitlements`.
#[tauri::command]
async fn fetch_application_entitlements(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    application_id: Option<String>,
    exclude_consumed: Option<bool>,
) -> Result<Vec<EntitlementRecord>, GiftError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<GiftError, _>(
        move || backend.token(),
        || GiftError::Storage,
        || GiftError::MissingToken,
    )
    .await?;
    let api = DiscordGifts::new(&token)?;
    gifts_plugin::fetch_application_entitlements_with_api(&api, application_id, exclude_consumed)
        .await
}

/// Lists user's giftable entitlements inventory: `GET /users/@me/entitlements/gifts`.
#[tauri::command]
async fn fetch_giftable_entitlements(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    country_code: Option<String>,
) -> Result<Vec<EntitlementRecord>, GiftError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<GiftError, _>(
        move || backend.token(),
        || GiftError::Storage,
        || GiftError::MissingToken,
    )
    .await?;
    let api = DiscordGifts::new(&token)?;
    gifts_plugin::fetch_giftable_entitlements_with_api(&api, country_code.as_deref()).await
}

/// Catalog of Nitro subscription plans and server boosts for gift modal (`#gift-modal`).
#[tauri::command]
fn list_nitro_gift_plans() -> Vec<NitroGiftPlan> {
    gifts_plugin::list_nitro_gift_plans()
}

// ---------------------------------------------------------------------------
// RW-40 / TASK-629 (`G1 Backend`): Remote Auth v2 RSA-2048 & WebSocket IPC commands.
// ---------------------------------------------------------------------------

/// Starts a new Remote Auth v2 QR login session: generates RSA-OAEP-2048 keypair
/// and returns session snapshot with `encoded_public_key`, `fingerprint`, `qr_url` and `init` frame.
#[tauri::command]
fn remote_auth_start_session(
    endpoint: Option<String>,
) -> Result<RemoteAuthSessionSnapshot, RemoteAuthError> {
    remote_auth::ipc_start_session(endpoint.as_deref())
}

/// Handles incoming WebSocket JSON frame from `wss://remote-auth.discord.gg/?v=2`
/// (`hello`, `nonce_proof`, `pending_remote_init`, `pending_ticket`, `pending_login`, `cancel`, `heartbeat_ack`).
#[tauri::command]
fn remote_auth_process_frame(
    frame_json: String,
) -> Result<RemoteAuthFrameOutcome, RemoteAuthError> {
    remote_auth::ipc_process_frame(&frame_json)
}

/// Ticks the heartbeat timer for an active Remote Auth v2 session.
#[tauri::command]
fn remote_auth_heartbeat_tick() -> Result<RemoteAuthFrameOutcome, RemoteAuthError> {
    remote_auth::ipc_heartbeat_tick()
}

/// Initializes verification for external QR code (`POST /users/@me/remote-auth`).
#[tauri::command]
async fn remote_auth_initialize(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    fingerprint: String,
) -> Result<RemoteAuthHandshakeToken, RemoteAuthError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<RemoteAuthError, _>(
        move || backend.token(),
        || RemoteAuthError::Storage,
        || RemoteAuthError::MissingToken,
    )
    .await?;
    let api = DiscordRemoteAuthRest::new(&token)?;
    remote_auth::remote_auth_initialize(&api, &fingerprint).await
}

/// Completes verification for external QR code (`POST /users/@me/remote-auth/finish`).
#[tauri::command]
async fn remote_auth_finish(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    handshake_token: String,
    temporary_token: Option<bool>,
) -> Result<(), RemoteAuthError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<RemoteAuthError, _>(
        move || backend.token(),
        || RemoteAuthError::Storage,
        || RemoteAuthError::MissingToken,
    )
    .await?;
    let api = DiscordRemoteAuthRest::new(&token)?;
    remote_auth::remote_auth_finish(&api, &handshake_token, temporary_token.unwrap_or(false)).await
}

/// Cancels verification for external QR code (`POST /users/@me/remote-auth/cancel`).
#[tauri::command]
async fn remote_auth_cancel(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    handshake_token: String,
) -> Result<(), RemoteAuthError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<RemoteAuthError, _>(
        move || backend.token(),
        || RemoteAuthError::Storage,
        || RemoteAuthError::MissingToken,
    )
    .await?;
    let api = DiscordRemoteAuthRest::new(&token)?;
    remote_auth::remote_auth_cancel(&api, &handshake_token).await
}

/// Cancels and restarts Remote Auth v2 session with a fresh RSA-2048 keypair.
#[tauri::command]
fn remote_auth_cancel_session() -> Result<RemoteAuthSessionSnapshot, RemoteAuthError> {
    remote_auth::ipc_cancel_session()
}

fn spawn_system_tray_indicator(app_handle: tauri::AppHandle) {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::TrayIconBuilder;

    let Ok(show_i) = MenuItem::with_id(&app_handle, "show", "Открыть Chord", true, None::<&str>)
    else {
        return;
    };
    let Ok(credits_i) =
        MenuItem::with_id(&app_handle, "credits", "Благодарности", true, None::<&str>)
    else {
        return;
    };
    let Ok(quit_i) = MenuItem::with_id(
        &app_handle,
        "quit",
        "Выйти из Chord          Ctrl+Q",
        true,
        None::<&str>,
    ) else {
        return;
    };
    let Ok(menu) = Menu::with_items(&app_handle, &[&show_i, &credits_i, &quit_i]) else {
        return;
    };

    let icon = app_handle.default_window_icon().cloned();
    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app_handle, event| match event.id().as_ref() {
            "show" => {
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            }
            "credits" => {
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                    let _ = window.eval(
                        "if (window.__OPEN_DISCORD_CREDITS__) window.__OPEN_DISCORD_CREDITS__();",
                    );
                }
            }
            "quit" => {
                data::shutdown_live_workers();
                std::process::exit(0);
            }
            _ => {}
        });
    if let Some(ic) = icon {
        builder = builder.icon(ic);
    }
    let _ = builder.build(&app_handle);
}

fn main() {
    chord_platform::screencast::allow_ptrace_yama();
    let is_headless_ci = std::env::var_os("SPIKE_EXIT_AFTER_MS").is_some()
        || std::env::var_os("SPIKE_AUTH_DISABLED").is_some();
    if is_headless_ci && std::env::var_os("WEBKIT_DISABLE_COMPOSITING_MODE").is_none() {
        std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
    }
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    tauri::Builder::default()
        .plugin(auth_plugin::init())
        .plugin(chat_plugin::init())
        .plugin(dm_plugin::init())
        .plugin(gifts_plugin::init())
        .plugin(guilds_plugin::init())
        .plugin(invites_plugin::init())
        .plugin(media_plugin::init())
        .plugin(net_plugin::init())
        .plugin(notifications_plugin::init())
        .plugin(search_plugin::init())
        .plugin(settings_plugin::init())
        .plugin(voice_plugin::init())
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                let minimize_to_tray = std::env::var_os("DISCORD_MINIMIZE_TO_TRAY").is_some();
                if minimize_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    eprintln!("[window_event] CloseRequested -> disconnecting live voice & shutting down workers");
                    voice_plugin::disconnect_on_window_close();
                    data::shutdown_live_workers();
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    std::process::exit(0);
                }
            }
            tauri::WindowEvent::Destroyed => {
                eprintln!("[window_event] Destroyed -> quick exit");
                std::process::exit(0);
            }
            _ => {}
        })
        .setup(|app| {
            // Set custom test window title if requested by test harness.
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(title) = std::env::var("SPIKE_WINDOW_TITLE") {
                    let _ = window.set_title(&title);
                }
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
            // Marker for cold-start benchmark harness indicating window ready state.
            // When SPIKE_AUTH_DISABLED is set, inject bypass flag for AT-SPI / offline smoke tests.
            if std::env::var("SPIKE_AUTH_DISABLED").is_ok() {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.eval("window.__SPIKE_AUTH_DISABLED__ = true;");
                }
            }
            println!("discord-gui-spike: window-ready");
            // Lazy Keyring initialization on background thread.
            app.manage(Arc::new(AuthBackend::keyring()));
            let auth = app.state::<Arc<AuthBackend>>().inner().clone();
            // L-02: Envelopes are forwarded to gateway://dispatch from dedicated thread.
            let dispatch = Arc::new(TauriDispatchEmitter(app.handle().clone()));
            app.manage(Arc::new(NetService::with_emitter(auth, dispatch)));
            if std::env::var_os("SPIKE_EXIT_AFTER_MS").is_none()
                && std::env::var_os("SPIKE_AUTH_DISABLED").is_none()
            {
                spawn_system_tray_indicator(app.handle().clone());
            }
            if let Ok(ms) = std::env::var("SPIKE_EXIT_AFTER_MS") {
                if let Ok(ms) = ms.parse::<u64>() {
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(ms));
                        data::shutdown_live_workers();
                        std::process::exit(0);
                    });
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            process_metrics,
            spike_config,
            report_bench,
            report_ui_snapshot,
            ready,
            read_state,
            channels,
            messages_page,
            members_page,
            settings,
            search_page,
            search_remote,
            send_message,
            edit_message,
            delete_message,
            reply_message,
            ack_message,
            add_reaction,
            toggle_reaction,
            send_typing,
            join_guild,
            create_guild,
            confirm_member_rules,
            resolve_invite,
            accept_invite,
            create_channel_invite,
            create_invite,
            request_upload_slots,
            attachments_upload_slots,
            attachments_upload_put,
            list_channel_pins,
            pins_list,
            pin_channel_message,
            unpin_channel_message,
            list_channel_threads,
            threads_list,
            update_guild_notification_settings,
            update_channel_notification_override,
            leave_guild,
            auth_status,
            auth_login_token,
            auth_restore,
            auth_logout,
            auth_login_password,
            auth_mfa,
            auth_sms,
            net_connect,
            net_status,
            net_dispatch_stats,
            net_disconnect,
            settings_load,
            settings_save,
            invoke_bff_domain,
            voice_connect,
            voice_disconnect,
            voice_mute,
            voice_deaf,
            voice_rtc_status,
            voice_audio_devices,
            voice_set_audio_device,
            voice_desktop_sources,
            voice_stream_start,
            voice_stream_stop,
            voice_stream_watch,
            voice_stream_set_paused,
            resolve_gift_code,
            create_gift_code,
            fetch_user_gift_codes,
            revoke_gift_code,
            redeem_gift_code,
            fetch_user_entitlements,
            fetch_application_entitlements,
            fetch_giftable_entitlements,
            list_nitro_gift_plans,
            remote_auth_start_session,
            remote_auth_process_frame,
            remote_auth_heartbeat_tick,
            remote_auth_cancel_session,
            remote_auth_initialize,
            remote_auth_finish,
            remote_auth_cancel
        ])
        .run(tauri::generate_context!())
        .expect("не удалось запустить Tauri-спайк");
}

// ---------------------------------------------------------------------------
// W39i2: wiring BFF-команд. Регистрация в `invoke_handler!` не видна в рантайме
// (Tauri не отдаёт список команд), а окно для этой проверки не нужно: контракт
// регистрации — сам исходник бинаря.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod wiring_tests {
    use super::*;
    use chord_client::bff_domains_router::{
        BffDomainAction, BffResolvedAction, UpdateGuildOnboardingRequest,
    };

    /// Тело списка команд внутри `invoke_handler!`.
    fn handler_block(source: &str) -> &str {
        const OPEN: &str = "generate_handler![";
        let start = source
            .find(OPEN)
            .expect("main.rs должен объявлять invoke_handler!")
            + OPEN.len();
        let rest = &source[start..];
        let end = rest.find(']').expect("invoke_handler! без закрывающей ]");
        &rest[..end]
    }

    /// Точное совпадение имени, а не подстрока: `search_page` не должен
    /// «находиться» внутри `search_page_v2` и наоборот.
    fn lists_command(block: &str, command: &str) -> bool {
        block
            .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .any(|name| name == command)
    }

    #[test]
    fn invoke_handler_registers_all_bff_commands() {
        let block = handler_block(include_str!("main.rs"));
        for command in [
            "ready",
            "read_state",
            "channels",
            "messages_page",
            "members_page",
            "settings",
            "search_page",
            "search_remote",
            "net_dispatch_stats",
        ] {
            assert!(
                lists_command(block, command),
                "{command} не зарегистрирован в invoke_handler!"
            );
        }
    }

    #[test]
    fn specta_bindings_wire_to_invoke_handler_and_zero_any() {
        let block = handler_block(include_str!("main.rs"));
        let builder = bindings::generate_specta_builder();
        for cmd in builder.commands() {
            assert!(
                lists_command(block, cmd.name),
                "Specta command `{}` must be registered in main.rs invoke_handler!",
                cmd.name
            );
        }
        builder
            .validate_no_any()
            .expect("Specta bindings must have zero `any` types");
    }

    /// Исходник до тестового модуля: литералы теста не дают ложного «нашлось».
    fn head_source() -> &'static str {
        include_str!("main.rs")
            .split("mod wiring_tests")
            .next()
            .expect("голова main.rs")
    }

    /// L-02: конверт уходит ровно в `gateway://dispatch`, ошибка эмита
    /// возвращается наверх (счётчик `emitFailed`), payload не логируется.
    #[test]
    fn dispatch_emitter_targets_gateway_event_without_logging() {
        let head = head_source();
        let start = head
            .find("impl DispatchEmitter for TauriDispatchEmitter")
            .expect("main.rs должен реализовать DispatchEmitter");
        let body = &head[start..];
        // Закрывающая скобка impl — `}` в начале строки (не `{error}` в format!).
        let body = &body[..body.find("\n}\n").expect("конец impl") + 2];
        assert!(
            body.contains("emit(DISPATCH_EVENT, payload)"),
            "конверт должен уходить в событие контракта: {body}"
        );
        assert!(
            body.contains("map_err("),
            "ошибка emit возвращается, а не глотается: {body}"
        );
        for forbidden in ["println!", "eprintln!", "log::", "payload.data"] {
            assert!(
                !body.contains(forbidden),
                "эмиттер не логирует payload ({forbidden}): {body}"
            );
        }
    }

    /// L-02: сервис создаётся с эмиттером — канал готов, источник в ядре
    /// подключён (`net::CORE_DISPATCH_AVAILABLE` → `sourceAvailable: true`).
    #[test]
    fn net_service_is_constructed_with_dispatch_emitter() {
        let head = head_source();
        assert!(head.contains("let dispatch = Arc::new(TauriDispatchEmitter("));
        assert!(head.contains("NetService::with_emitter(auth, dispatch)"));
        assert!(head.contains("net_dispatch_stats"));
    }

    /// Статистика L-02 — read-only команда без аргументов от фронтенда: токен
    /// в UI не читается и в аргументы не попадает.
    #[test]
    fn dispatch_stats_command_is_read_only_without_token() {
        let head = head_source();
        let start = head
            .find("fn net_dispatch_stats(")
            .expect("main.rs должен объявлять net_dispatch_stats");
        let body = &head[start..];
        let body = &body[..body.find("\n}\n").expect("конец команды") + 2];
        assert!(body.contains("net.dispatch_stats()"));
        for forbidden in ["token", "println!", "eprintln!", "log::"] {
            assert!(
                !body.contains(forbidden),
                "net_dispatch_stats не должен содержать {forbidden}: {body}"
            );
        }
    }

    /// C-02-BE: `channels` — локальная READY-проекция: без токена/keyring и без
    /// логирования. Тело команды извлекается из исходника (окно не нужно).
    #[test]
    fn channels_command_is_local_and_carries_no_token() {
        let source = include_str!("main.rs");
        let start = source
            .find("fn channels(guild_id: Option<String>)")
            .expect("main.rs должен объявлять команду channels");
        let rest = &source[start..];
        let end = rest.find("#[tauri::command]").unwrap_or(rest.len());
        let body = &rest[..end];
        assert!(
            body.contains("data::channels(&root, guild_id.as_deref())"),
            "команда обязана делегировать в data::channels: {body}"
        );
        for forbidden in [
            "token_or",
            "backend.token()",
            "AuthBackend",
            "println!",
            "eprintln!",
            "log::",
        ] {
            assert!(
                !body.contains(forbidden),
                "channels не должен содержать {forbidden}: {body}"
            );
        }
    }

    fn command_body<'a>(source: &'a str, name: &str) -> &'a str {
        let marker = format!("async fn {name}(");
        let start = source
            .find(&marker)
            .unwrap_or_else(|| panic!("main.rs должен объявлять команду {name}"));
        let rest = &source[start..];
        let end = rest.find("#[tauri::command]").unwrap_or(rest.len());
        &rest[..end]
    }

    const TOKEN_COMMANDS: [&str; 36] = [
        "search_remote",
        "send_message",
        "edit_message",
        "delete_message",
        "reply_message",
        "ack_message",
        "toggle_reaction",
        "send_typing",
        "join_guild",
        "leave_guild",
        "resolve_invite",
        "accept_invite",
        "create_channel_invite",
        "create_invite",
        "request_upload_slots",
        "attachments_upload_slots",
        "attachments_upload_put",
        "list_channel_pins",
        "pins_list",
        "pin_channel_message",
        "unpin_channel_message",
        "list_channel_threads",
        "threads_list",
        "update_guild_notification_settings",
        "update_channel_notification_override",
        "resolve_gift_code",
        "create_gift_code",
        "fetch_user_gift_codes",
        "revoke_gift_code",
        "redeem_gift_code",
        "fetch_user_entitlements",
        "fetch_application_entitlements",
        "fetch_giftable_entitlements",
        "remote_auth_initialize",
        "remote_auth_finish",
        "remote_auth_cancel",
    ];

    /// G1-02: копий блока «blocking + два map_err + ok_or» больше нет — у каждой
    /// сетевой команды токен читает один helper `token_or`, коды задаёт домен.
    #[test]
    fn every_network_command_reads_token_through_one_helper() {
        let source = include_str!("main.rs");
        // `include_str!("main.rs")` видит и сам тест, поэтому считаем строки
        // объявления, а не подстроки: литерал в тесте не начинается с `async fn`.
        let declarations = source
            .lines()
            .filter(|line| line.starts_with("async fn token_or"))
            .count();
        assert_eq!(
            declarations, 1,
            "helper чтения токена должен быть объявлен ровно один раз"
        );
        for command in TOKEN_COMMANDS {
            let body = command_body(source, command);
            assert!(
                body.contains("token_or::<"),
                "{command}: токен читается только helper'ом token_or"
            );
            assert_eq!(
                body.matches("backend.token()").count(),
                1,
                "{command}: ровно одно чтение backend.token()"
            );
            assert!(
                !body.contains(".map_err(|_| "),
                "{command}: локальный маппинг ошибок токена запрещён"
            );
            assert!(
                !body.contains("println!") && !body.contains("eprintln!"),
                "{command}: токен и текст keyring-ошибки не логируются"
            );
        }
    }

    /// serde-тег `kind` — контракт с фронтендом; оракул не зависит от имени
    /// Rust-варианта.
    fn error_kind<T: serde::Serialize>(value: &T) -> String {
        let json = serde_json::to_value(value).expect("ошибка сериализуется");
        json.get("kind")
            .and_then(|kind| kind.as_str())
            .unwrap_or_else(|| panic!("нет строкового kind: {json}"))
            .to_string()
    }

    /// G1-02/SEC-02: один helper — типизированные коды каждого домена.
    /// keyring/blocking-сбой → `storage` (аддитивный kind), а не «сеть»:
    /// причина отказа хранилища не маскируется и не утекает. «Токена нет» —
    /// прежний `Unauthorized` у сообщений/реакций и `MissingToken` у search/guild.
    #[test]
    fn token_or_preserves_domain_error_codes() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime");
        let keyring = || {
            Err(AuthError::Keyring {
                message: "Secret Service недоступен".to_string(),
            })
        };
        let no_token = || Ok(None);

        let error = runtime
            .block_on(token_or::<MessageError, _>(
                keyring,
                || MessageError::Storage,
                || MessageError::Unauthorized,
            ))
            .expect_err("keyring");
        assert_eq!(error_kind(&error), "storage", "{error}");
        let rendered = format!("{error}/{error:?}");
        assert!(
            !rendered.contains("Secret Service"),
            "текст keyring-ошибки не должен попадать в доменную ошибку: {rendered}"
        );
        assert_eq!(
            runtime.block_on(token_or::<MessageError, _>(
                no_token,
                || MessageError::Storage,
                || MessageError::Unauthorized,
            )),
            Err(MessageError::Unauthorized)
        );

        let error = runtime
            .block_on(token_or::<ReactionError, _>(
                keyring,
                || ReactionError::Storage,
                || ReactionError::Unauthorized,
            ))
            .expect_err("keyring");
        assert_eq!(error_kind(&error), "storage", "{error}");

        let error = runtime
            .block_on(token_or::<SearchError, _>(
                keyring,
                || SearchError::Storage,
                || SearchError::MissingToken,
            ))
            .expect_err("keyring");
        assert_eq!(error_kind(&error), "storage", "{error}");
        assert_eq!(
            runtime.block_on(token_or::<SearchError, _>(
                no_token,
                || SearchError::Storage,
                || SearchError::MissingToken,
            )),
            Err(SearchError::MissingToken)
        );

        let error = runtime
            .block_on(token_or::<EnterGuildError, _>(
                keyring,
                || EnterGuildError::Storage,
                || EnterGuildError::MissingToken,
            ))
            .expect_err("keyring");
        assert_eq!(error_kind(&error), "storage", "{error}");
        assert_eq!(
            runtime.block_on(token_or::<EnterGuildError, _>(
                no_token,
                || EnterGuildError::Storage,
                || EnterGuildError::MissingToken,
            )),
            Err(EnterGuildError::MissingToken)
        );
        // Out-of-scope регрессия: «сеть» остаётся сетью (классификатор REST не
        // переиспользует `Storage`), поэтому код `Network` не подменяется.
        assert_eq!(error_kind(&EnterGuildError::Network), "network");

        let error = runtime
            .block_on(token_or::<LeaveGuildError, _>(
                keyring,
                || LeaveGuildError::Storage,
                || LeaveGuildError::MissingToken,
            ))
            .expect_err("keyring");
        assert_eq!(error_kind(&error), "storage", "{error}");
        assert_eq!(
            runtime.block_on(token_or::<LeaveGuildError, _>(
                no_token,
                || LeaveGuildError::Storage,
                || LeaveGuildError::MissingToken,
            )),
            Err(LeaveGuildError::MissingToken)
        );

        assert_eq!(
            runtime.block_on(token_or::<ReactionError, _>(
                || Ok(Some("secret-token".to_string())),
                || ReactionError::Storage,
                || ReactionError::Unauthorized,
            )),
            Ok("secret-token".to_string())
        );
    }

    /// Фикстур нет — skip с причиной (не падение): живые артефакты тест не
    /// подменяет, временных каталогов не создаёт.
    fn fixtures_or_skip() -> Option<std::path::PathBuf> {
        match data::fixtures_root() {
            Ok(root) => Some(root),
            Err(err) => {
                eprintln!("skip: фикстуры недоступны: {err}");
                None
            }
        }
    }

    #[test]
    fn filters_payload_is_camel_case_and_maps_to_data_type() {
        // Фильтры приходят объектом из JS: имена — camelCase, как аргументы
        // Tauri v2 (`channelId`, `guildId`), отсутствующие ключи — `None`.
        // Канонический `data::SearchFilters` сам десериализуется: зеркала нет.
        let filters: SearchFilters = serde_json::from_value(serde_json::json!({
            "authorId": "100",
            "channelId": "200",
            "content": "call of duty",
            "mentionId": "300",
            "day": "2026-10-19",
            "messageType": 0,
            "pinned": false
        }))
        .expect("фильтры из IPC");
        assert_eq!(
            filters,
            SearchFilters {
                author_id: Some("100".to_string()),
                channel_id: Some("200".to_string()),
                content: Some("call of duty".to_string()),
                mention_id: Some("300".to_string()),
                day: Some("2026-10-19".to_string()),
                message_type: Some(0),
                pinned: Some(false),
            }
        );

        let empty: SearchFilters = serde_json::from_value(serde_json::json!({}))
            .expect("пустой объект фильтров — не ошибка");
        assert_eq!(empty, SearchFilters::default());
    }

    #[test]
    fn search_page_command_passes_query_filters_and_cursor() {
        let Some(root) = fixtures_or_skip() else {
            return;
        };

        // query обрезается в слое данных: команда не должна добавлять свою логику.
        let expected =
            data::search_page(&root, "call of duty", &SearchFilters::default(), None, None)
                .expect("поиск по фикстурам");
        let actual = search_page("  call of duty  ".to_string(), None, None, None)
            .expect("команда search_page");
        assert_eq!(actual, expected);
        assert_eq!(actual.total, 1, "ровно одно совпадение в фикстурах");

        // Фильтр из IPC доезжает до слоя данных (pinned=false → все 13 строк).
        let filters: SearchFilters = serde_json::from_value(serde_json::json!({
            "pinned": false
        }))
        .expect("фильтр pinned");
        let expected_filtered = data::search_page(
            &root,
            "",
            &SearchFilters {
                pinned: Some(false),
                ..SearchFilters::default()
            },
            None,
            Some(5),
        )
        .expect("фильтрованная выборка");
        let actual_filtered =
            search_page(String::new(), Some(filters), None, Some(5)).expect("команда с фильтром");
        assert_eq!(actual_filtered, expected_filtered);

        // Курсор следующей страницы — id последнего результата, без пересечений.
        let cursor = actual_filtered
            .next_cursor
            .clone()
            .expect("при limit 5 и total 13 есть следующая страница");
        let second = search_page(String::new(), None, Some(cursor.clone()), Some(5))
            .expect("вторая страница");
        let expected_second =
            data::search_page(&root, "", &SearchFilters::default(), Some(&cursor), Some(5))
                .expect("вторая страница слоя данных");
        assert_eq!(second, expected_second);
        assert!(
            second.items.iter().all(|hit| !actual_filtered
                .items
                .iter()
                .any(|prev| prev.message.id == hit.message.id)),
            "страницы не должны пересекаться"
        );
    }

    type PutRecord = (String, String, Vec<u8>);

    #[derive(Clone, Default)]
    struct FakePutTransport {
        status: u16,
        puts: std::sync::Arc<std::sync::Mutex<Vec<PutRecord>>>,
    }

    impl FakePutTransport {
        fn answering(status: u16) -> Self {
            Self {
                status,
                puts: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            }
        }

        fn puts(&self) -> Vec<PutRecord> {
            self.puts.lock().expect("puts lock").clone()
        }
    }

    impl AttachmentTransport for FakePutTransport {
        fn post<'a>(
            &'a self,
            _target: &'a str,
            _body: &'a [u8],
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<chord_core::rest::transport::HttpResponse, AttachmentError>,
                    > + Send
                    + 'a,
            >,
        > {
            Box::pin(async move { Err(AttachmentError::Network) })
        }

        fn put<'a>(
            &'a self,
            upload_url: &'a str,
            content_type: &'a str,
            body: &'a [u8],
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<chord_core::rest::transport::HttpResponse, AttachmentError>,
                    > + Send
                    + 'a,
            >,
        > {
            self.puts.lock().expect("puts lock").push((
                upload_url.to_string(),
                content_type.to_string(),
                body.to_vec(),
            ));
            let status = self.status;
            Box::pin(async move {
                Ok(chord_core::rest::transport::HttpResponse {
                    status,
                    headers: Vec::new(),
                    body: Vec::new(),
                })
            })
        }
    }

    /// INT-04-ATTACH-PUT-BACKEND-PASS2: проверяет Tauri command extraction на точном плоском
    /// `putPayload` из `composer.js:744-755` (`{channelId, id, uploadUrl, uploadFilename, fileSize,
    /// contentType, bytes: {'0': 137, '1': 80}, data: {'0': 137, '1': 80}}`) и вызов команды
    /// с fake-транспортом.
    #[test]
    fn attachments_upload_put_command_extracts_flat_composer_payload_and_sends_put_bytes() {
        let head = head_source();
        let body = command_body(head, "attachments_upload_put");
        let sig_end = body
            .find(')')
            .expect("конец сигнатуры attachments_upload_put");
        let sig = &body[..sig_end];

        let mut declared_keys = Vec::new();
        for raw_arg in sig.split('(').nth(1).expect("список параметров").split(',')
        {
            let trimmed = raw_arg.trim();
            if trimmed.is_empty() {
                continue;
            }
            let Some((name_part, type_part)) = trimmed.split_once(':') else {
                continue;
            };
            if type_part.contains("tauri::State") {
                continue;
            }
            let snake = name_part.trim();
            let mut camel = String::with_capacity(snake.len());
            let mut upper = false;
            for ch in snake.chars() {
                if ch == '_' {
                    upper = true;
                } else if upper {
                    camel.push(ch.to_ascii_uppercase());
                    upper = false;
                } else {
                    camel.push(ch);
                }
            }
            declared_keys.push((snake.to_string(), camel));
        }

        let expected: Vec<(String, String)> =
            chord_client::attachments::UPLOAD_PUT_COMMAND_PARAM_KEYS
                .iter()
                .map(|(s, c)| ((*s).to_string(), (*c).to_string()))
                .collect();
        assert_eq!(
            declared_keys, expected,
            "параметры #[tauri::command] async fn attachments_upload_put должны включать `id: Option<String>`"
        );

        let upload_url =
            "https://discord-attachments-uploads-prd.storage.googleapis.com/upload-1?sig=abc123";
        let composer_put_payload = serde_json::json!({
            "channelId": "123456789012345678",
            "id": "slot-1",
            "uploadUrl": upload_url,
            "uploadFilename": "1/image.png",
            "fileSize": 2,
            "contentType": "image/png",
            "bytes": {"0": 137, "1": 80},
            "data": {"0": 137, "1": 80}
        });

        let args = UploadPutCommandArgs::from_tauri_ipc(&composer_put_payload)
            .expect("Tauri command extraction для плоского putPayload");
        let args_serde: UploadPutCommandArgs = serde_json::from_value(composer_put_payload)
            .expect("serde-десериализация UploadPutCommandArgs");
        assert_eq!(args, args_serde);
        assert_eq!(args.payload, None);
        assert_eq!(args.slot_id, None);
        assert_eq!(args.id.as_deref(), Some("slot-1"));
        assert_eq!(args.data, Some(UploadBytes(vec![137, 80])));
        assert_eq!(args.bytes, Some(UploadBytes(vec![137, 80])));

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime");
        let transport = FakePutTransport::answering(200);
        assert_eq!(
            runtime.block_on(execute_attachments_upload_put(&transport, args)),
            Ok(())
        );
        assert_eq!(
            transport.puts(),
            vec![(
                upload_url.to_string(),
                "image/png".to_string(),
                vec![137, 80]
            )]
        );

        // Проверка `slot_id.or(id)`: пустой/пробельный `id` в плоском putPayload обязан
        // отклоняться командой с `InvalidArg` до обращения к транспорту:
        let bad_id_payload = serde_json::json!({
            "channelId": "123456789012345678",
            "id": "   ",
            "uploadUrl": upload_url,
            "uploadFilename": "1/image.png",
            "fileSize": 2,
            "contentType": "image/png",
            "bytes": {"0": 137, "1": 80},
            "data": {"0": 137, "1": 80}
        });
        let bad_args = UploadPutCommandArgs::from_tauri_ipc(&bad_id_payload)
            .expect("Tauri command extraction для bad_id_payload");
        let guard_transport = FakePutTransport::answering(200);
        assert_eq!(
            runtime.block_on(execute_attachments_upload_put(&guard_transport, bad_args)),
            Err(AttachmentError::InvalidArg)
        );
        assert!(guard_transport.puts().is_empty());
    }

    /// INT-05-IPC-ALIASES: все 4 F-1 IPC-алиаса (`attachments_upload_slots`, `create_invite`,
    /// `pins_list`, `threads_list`) зарегистрированы в `generate_handler!` и делегируют
    /// в те же доменные обёртки с сохранением кодов ошибок и формы ответов UI.
    #[test]
    fn invoke_handler_registers_f1_ui_ipc_aliases_and_wires_payload_contracts() {
        let head = head_source();
        let block = handler_block(head);
        for command in [
            "attachments_upload_slots",
            "create_invite",
            "pins_list",
            "threads_list",
        ] {
            assert!(
                lists_command(block, command),
                "{command} не зарегистрирован в invoke_handler!"
            );
        }

        let slots_body = command_body(head, "attachments_upload_slots");
        assert!(slots_body.contains("UploadSlotsEnvelope { attachments }"));
        assert!(
            slots_body.contains("chord_client::request_upload_slots(&api, &channel_id, &specs)")
        );

        let invite_body = command_body(head, "create_invite");
        assert!(invite_body.contains(
            "CreateInviteParamsInput::resolve(params, max_age, max_uses, temporary, flags)"
        ));
        assert!(invite_body
            .contains("chord_client::create_channel_invite(&api, &channel_id, &resolved)"));

        let pins_body = command_body(head, "pins_list");
        assert!(pins_body.contains("chord_client::list_channel_pins(&api, &channel_id)"));

        let threads_body = command_body(head, "threads_list");
        assert!(threads_body.contains(
            "chord_client::list_channel_threads(&api, &channel_id, archived.unwrap_or(false))"
        ));

        // Поведенческая проверка десериализации/резолва плоского и вложенного payload (camelCase + snake_case):
        let flat_resolved =
            CreateInviteParamsInput::resolve(None, Some(3600), Some(10), Some(true), Some(1));
        assert_eq!(
            flat_resolved,
            CreateInviteParams {
                max_age: 3600,
                max_uses: 10,
                temporary: true,
                flags: Some(1),
            }
        );

        let nested_camel: CreateInviteParamsInput = serde_json::from_value(serde_json::json!({
            "maxAge": 1800,
            "maxUses": 5,
            "temporary": true,
            "flags": 1
        }))
        .expect("camelCase params");
        assert_eq!(
            CreateInviteParamsInput::resolve(Some(nested_camel), None, None, None, None),
            CreateInviteParams {
                max_age: 1800,
                max_uses: 5,
                temporary: true,
                flags: Some(1),
            }
        );

        let nested_snake: CreateInviteParamsInput = serde_json::from_value(serde_json::json!({
            "max_age": 43200,
            "max_uses": 25,
            "temporary": false
        }))
        .expect("snake_case params");
        assert_eq!(
            CreateInviteParamsInput::resolve(Some(nested_snake), None, None, None, None),
            CreateInviteParams {
                max_age: 43200,
                max_uses: 25,
                temporary: false,
                flags: None,
            }
        );

        assert_eq!(
            CreateInviteParamsInput::resolve(None, None, None, None, None),
            CreateInviteParams::default()
        );

        // Поведенческая проверка `UploadSlotSpecInput` (snake_case из composer.js:725 и camelCase) + `UploadSlotsEnvelope`:
        let snake_spec: UploadSlotSpecInput = serde_json::from_value(serde_json::json!({
            "filename": "avatar.png",
            "file_size": 5000,
            "id": "loc-1",
            "is_clip": false
        }))
        .expect("snake_case UploadSlotSpecInput");
        assert_eq!(
            AttachmentFileSpec::from(snake_spec),
            AttachmentFileSpec {
                filename: "avatar.png".to_string(),
                file_size: 5000,
                id: "loc-1".to_string(),
                is_clip: false,
            }
        );

        let camel_spec: UploadSlotSpecInput = serde_json::from_value(serde_json::json!({
            "filename": "clip.mp4",
            "fileSize": 8192,
            "id": "loc-2",
            "isClip": true
        }))
        .expect("camelCase UploadSlotSpecInput");
        assert_eq!(
            AttachmentFileSpec::from(camel_spec),
            AttachmentFileSpec {
                filename: "clip.mp4".to_string(),
                file_size: 8192,
                id: "loc-2".to_string(),
                is_clip: true,
            }
        );

        let envelope = UploadSlotsEnvelope {
            attachments: vec![AttachmentUploadSlot {
                id: "loc-1".to_string(),
                upload_url: "https://discord.upload/slot-1".to_string(),
                upload_filename: "uploaded-avatar.png".to_string(),
            }],
        };
        assert_eq!(
            serde_json::to_value(&envelope).expect("envelope JSON"),
            serde_json::json!({
                "attachments": [
                    {
                        "id": "loc-1",
                        "upload_url": "https://discord.upload/slot-1",
                        "upload_filename": "uploaded-avatar.png"
                    }
                ]
            })
        );
    }

    #[test]
    fn invoke_handler_registers_notifications_ipc_commands_and_resolves_channel_override_input() {
        let head = head_source();
        let block = handler_block(head);
        for command in [
            "update_guild_notification_settings",
            "update_channel_notification_override",
        ] {
            assert!(
                lists_command(block, command),
                "{command} не зарегистрирован в invoke_handler!"
            );
        }

        // Разбор конверта `{ channel_overrides: { [channelId]: overridePatch } }` из notifications.js:1389:
        let envelope_input: ChannelOverrideInput = serde_json::from_value(serde_json::json!({
            "channel_overrides": {
                "ch-announce": {
                    "message_notifications": 2,
                    "muted": false
                }
            }
        }))
        .expect("envelope ChannelOverrideInput");
        assert_eq!(
            ChannelOverrideInput::resolve(
                "ch-announce",
                None,
                None,
                None,
                Some(envelope_input),
                None
            ),
            Ok(ChannelNotificationOverridePatch::new()
                .with_message_notifications(2)
                .with_muted(false))
        );

        // P2-2: если конверт channel_overrides содержит чужой канал и не содержит запрошенный channel_id,
        // into_patch / resolve НЕ должны молча брать чужой канал, а обязаны вернуть Err(NotificationError::InvalidChannelId):
        let foreign_envelope: ChannelOverrideInput = serde_json::from_value(serde_json::json!({
            "channel_overrides": {
                "ch-foreign": {
                    "message_notifications": 0,
                    "muted": true
                }
            }
        }))
        .expect("foreign envelope ChannelOverrideInput");
        assert_eq!(
            foreign_envelope.clone().into_patch("ch-announce"),
            Err(NotificationError::InvalidChannelId)
        );
        assert_eq!(
            ChannelOverrideInput::resolve(
                "ch-announce",
                None,
                None,
                None,
                Some(foreign_envelope),
                None
            ),
            Err(NotificationError::InvalidChannelId)
        );

        // Пустой конверт `{ channel_overrides: {} }` возвращает пустой патч:
        let empty_envelope: ChannelOverrideInput = serde_json::from_value(serde_json::json!({
            "channel_overrides": {}
        }))
        .expect("empty envelope ChannelOverrideInput");
        assert_eq!(
            empty_envelope.into_patch("ch-announce"),
            Ok(ChannelNotificationOverridePatch::default())
        );

        // P2-3: вызов только с полем `override` / `override_field` без дублирования `patch`:
        let override_only = ChannelNotificationOverridePatch::new()
            .with_message_notifications(0)
            .with_muted(true);
        assert_eq!(
            ChannelOverrideInput::resolve(
                "ch-announce",
                Some(override_only.clone()),
                None,
                None,
                None,
                None
            ),
            Ok(override_only.clone())
        );
        assert_eq!(
            ChannelOverrideInput::resolve(
                "ch-announce",
                None,
                Some(override_only.clone()),
                None,
                None,
                None
            ),
            Ok(override_only.clone())
        );
        let override_json_input: ChannelOverrideInput = serde_json::from_value(serde_json::json!({
            "override": {
                "message_notifications": 0,
                "muted": true
            }
        }))
        .expect("override field JSON ChannelOverrideInput");
        assert_eq!(
            override_json_input.clone().into_patch("ch-announce"),
            Ok(override_only.clone())
        );
        assert_eq!(
            ChannelOverrideInput::resolve(
                "ch-announce",
                None,
                None,
                None,
                None,
                Some(override_json_input)
            ),
            Ok(override_only)
        );

        // Разбор прямого payload `{ message_notifications: 1, muted: true }`:
        let direct_input: ChannelOverrideInput = serde_json::from_value(serde_json::json!({
            "message_notifications": 1,
            "muted": true
        }))
        .expect("direct ChannelOverrideInput");
        assert_eq!(
            ChannelOverrideInput::resolve(
                "ch-announce",
                None,
                None,
                None,
                None,
                Some(direct_input)
            ),
            Ok(ChannelNotificationOverridePatch::new()
                .with_message_notifications(1)
                .with_muted(true))
        );
    }

    /// BE-INT-16: единый IPC-роутер BFF зарегистрирован в `invoke_handler!` и
    /// делегирует в `bff_domains_router::resolve_bff_domain_action`.
    ///
    /// Имя теста сохраняет карточку (`seven_domains` — целевой список доменов при
    /// расширении роутера). Фактический контракт `bff_domains_router` на базе
    /// `unstable 648ba83` — 4 домена, поэтому валидный/невалидный вызов проверяется
    /// на `UpdateGuildOnboarding` (решение координатора по task-293).
    #[test]
    fn invoke_handler_registers_invoke_bff_domain_for_seven_domains() {
        assert!(
            lists_command(handler_block(head_source()), "invoke_bff_domain"),
            "invoke_bff_domain не зарегистрирован в invoke_handler!"
        );

        // Валидный guild_id: роутер отдаёт метод, маршрут и тело запроса.
        assert_eq!(
            invoke_bff_domain(BffDomainAction::UpdateGuildOnboarding {
                guild_id: "123456789012345678".to_string(),
                request: UpdateGuildOnboardingRequest {
                    enabled: true,
                    description: None,
                    welcome_channels: Vec::new(),
                },
            }),
            Ok(BffResolvedAction {
                method: "PATCH",
                route: "/api/v9/guilds/123456789012345678/welcome-screen".to_string(),
                body: serde_json::json!({
                    "enabled": true,
                    "description": null,
                    "welcome_channels": []
                }),
            })
        );

        // Невалидный Snowflake (ведущий '0'): ошибка поднимается как Err, а не паника.
        let invalid = invoke_bff_domain(BffDomainAction::UpdateGuildOnboarding {
            guild_id: "0".to_string(),
            request: UpdateGuildOnboardingRequest {
                enabled: true,
                description: None,
                welcome_channels: Vec::new(),
            },
        });
        assert!(
            invalid.is_err(),
            "guild_id=\"0\" должен давать Err: {invalid:?}"
        );
    }
}
