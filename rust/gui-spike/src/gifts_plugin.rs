//! Modular Tauri v2 plugin (`gifts`) encapsulating Discord Nitro gifts and entitlements IPC commands.
//!
//! Provides `resolve_gift_code`, `create_gift_code`, `fetch_user_gift_codes`,
//! `revoke_gift_code`, `redeem_gift_code`, `fetch_user_entitlements`,
//! `fetch_application_entitlements`, `fetch_giftable_entitlements`, and
//! `list_nitro_gift_plans` via `tauri::plugin::Builder::new("gifts")`.

use std::sync::Arc;

use chord_client::auth::{AuthBackend, AuthError};

use crate::gifts::{
    self, CreateGiftCodeRequest, DiscordGifts, EntitlementQuery, EntitlementRecord, GiftCodeRecord,
    GiftError, GiftRedeemRequest, GiftResolveOptions, NitroGiftPlan, RedeemedGiftOutcome,
};

async fn token_or_gift<F>(work: F) -> Result<String, GiftError>
where
    F: FnOnce() -> Result<Option<String>, AuthError> + Send + 'static,
{
    let joined = tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| GiftError::Storage)?;
    match joined {
        Ok(Some(token)) if !token.trim().is_empty() => Ok(token),
        Ok(_) => Err(GiftError::MissingToken),
        Err(_) => Err(GiftError::Storage),
    }
}

/// Resolves a gift code preview (`GET /entitlements/gift-codes/{code}`) using an initialized [`DiscordGifts`] client.
///
/// # Errors
/// Returns [`GiftError`] if the code is invalid or the REST call fails.
#[inline]
pub async fn resolve_gift_code_with_api(
    api: &DiscordGifts,
    code: &str,
    with_application: Option<bool>,
    with_subscription_plan: Option<bool>,
) -> Result<GiftCodeRecord, GiftError> {
    let opts = GiftResolveOptions {
        with_application: with_application.unwrap_or(true),
        with_subscription_plan: with_subscription_plan.unwrap_or(true),
    };
    gifts::resolve_gift_code(api, code, opts).await
}

/// Resolves a gift code preview (`GET /entitlements/gift-codes/{code}`).
///
/// # Errors
/// Returns [`GiftError`] if the keyring token is missing/inaccessible or the REST call fails.
#[allow(dead_code)]
pub async fn resolve_gift_code(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    code: String,
    with_application: Option<bool>,
    with_subscription_plan: Option<bool>,
) -> Result<GiftCodeRecord, GiftError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or_gift(move || backend.token()).await?;
    let api = DiscordGifts::new(&token)?;
    resolve_gift_code_with_api(&api, &code, with_application, with_subscription_plan).await
}

/// Creates a new gift code (`POST /users/@me/entitlements/gift-codes`) using an initialized [`DiscordGifts`] client.
///
/// # Errors
/// Returns [`GiftError`] if the request parameters are invalid or the REST call fails.
#[inline]
pub async fn create_gift_code_with_api(
    api: &DiscordGifts,
    sku_id: String,
    subscription_plan_id: Option<String>,
    gift_style: Option<u8>,
    custom_message: Option<String>,
) -> Result<GiftCodeRecord, GiftError> {
    let req = CreateGiftCodeRequest {
        sku_id,
        subscription_plan_id,
        gift_style,
        custom_message,
    };
    gifts::create_gift_code(api, &req).await
}

/// Creates a new gift code (`POST /users/@me/entitlements/gift-codes`).
///
/// # Errors
/// Returns [`GiftError`] if the keyring token is missing/inaccessible or the REST call fails.
#[allow(dead_code)]
pub async fn create_gift_code(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    sku_id: String,
    subscription_plan_id: Option<String>,
    gift_style: Option<u8>,
    custom_message: Option<String>,
) -> Result<GiftCodeRecord, GiftError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or_gift(move || backend.token()).await?;
    let api = DiscordGifts::new(&token)?;
    create_gift_code_with_api(&api, sku_id, subscription_plan_id, gift_style, custom_message).await
}

/// Fetches gift codes owned by the current user (`GET /users/@me/entitlements/gift-codes`) using an initialized [`DiscordGifts`] client.
///
/// # Errors
/// Returns [`GiftError`] if the filter parameters are invalid or the REST call fails.
#[inline]
pub async fn fetch_user_gift_codes_with_api(
    api: &DiscordGifts,
    sku_id: Option<&str>,
    subscription_plan_id: Option<&str>,
) -> Result<Vec<GiftCodeRecord>, GiftError> {
    gifts::fetch_user_gift_codes(api, sku_id, subscription_plan_id).await
}

/// Fetches gift codes owned by the current user (`GET /users/@me/entitlements/gift-codes`).
///
/// # Errors
/// Returns [`GiftError`] if the keyring token is missing/inaccessible or the REST call fails.
#[allow(dead_code)]
pub async fn fetch_user_gift_codes(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    sku_id: Option<String>,
    subscription_plan_id: Option<String>,
) -> Result<Vec<GiftCodeRecord>, GiftError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or_gift(move || backend.token()).await?;
    let api = DiscordGifts::new(&token)?;
    fetch_user_gift_codes_with_api(&api, sku_id.as_deref(), subscription_plan_id.as_deref()).await
}

/// Revokes (deletes) a gift code (`DELETE /users/@me/entitlements/gift-codes/{code}`) using an initialized [`DiscordGifts`] client.
///
/// # Errors
/// Returns [`GiftError`] if the code is invalid or the REST call fails.
#[inline]
pub async fn revoke_gift_code_with_api(api: &DiscordGifts, code: &str) -> Result<(), GiftError> {
    gifts::revoke_gift_code(api, code).await
}

/// Revokes (deletes) a gift code (`DELETE /users/@me/entitlements/gift-codes/{code}`).
///
/// # Errors
/// Returns [`GiftError`] if the keyring token is missing/inaccessible or the REST call fails.
#[allow(dead_code)]
pub async fn revoke_gift_code(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    code: String,
) -> Result<(), GiftError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or_gift(move || backend.token()).await?;
    let api = DiscordGifts::new(&token)?;
    revoke_gift_code_with_api(&api, &code).await
}

/// Redeems a gift code (`POST /entitlements/gift-codes/{code}/redeem`) using an initialized [`DiscordGifts`] client.
///
/// # Errors
/// Returns [`GiftError`] if the code or checkout parameters are invalid or the REST call fails.
#[inline]
pub async fn redeem_gift_code_with_api(
    api: &DiscordGifts,
    code: &str,
    channel_id: Option<String>,
    payment_source_id: Option<String>,
    gateway_checkout_context: Option<serde_json::Value>,
) -> Result<RedeemedGiftOutcome, GiftError> {
    let req = GiftRedeemRequest {
        channel_id,
        payment_source_id,
        gateway_checkout_context,
    };
    gifts::redeem_gift_code(api, code, &req).await
}

/// Redeems a gift code (`POST /entitlements/gift-codes/{code}/redeem`).
///
/// # Errors
/// Returns [`GiftError`] if the keyring token is missing/inaccessible or the REST call fails.
#[allow(dead_code)]
pub async fn redeem_gift_code(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    code: String,
    channel_id: Option<String>,
    payment_source_id: Option<String>,
    gateway_checkout_context: Option<serde_json::Value>,
) -> Result<RedeemedGiftOutcome, GiftError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or_gift(move || backend.token()).await?;
    let api = DiscordGifts::new(&token)?;
    redeem_gift_code_with_api(
        &api,
        &code,
        channel_id,
        payment_source_id,
        gateway_checkout_context,
    )
    .await
}

/// Fetches entitlements for the current user (`GET /users/@me/entitlements`) using an initialized [`DiscordGifts`] client.
///
/// # Errors
/// Returns [`GiftError`] if the REST call or response parsing fails.
#[inline]
pub async fn fetch_user_entitlements_with_api(
    api: &DiscordGifts,
    with_sku: Option<bool>,
    with_application: Option<bool>,
    entitlement_type: Option<u8>,
    exclude_ended: Option<bool>,
) -> Result<Vec<EntitlementRecord>, GiftError> {
    let query = EntitlementQuery {
        with_sku: with_sku.unwrap_or(false),
        with_application: with_application.unwrap_or(false),
        entitlement_type,
        exclude_ended: exclude_ended.unwrap_or(true),
    };
    gifts::fetch_user_entitlements(api, &query).await
}

/// Fetches entitlements for the current user (`GET /users/@me/entitlements`).
///
/// # Errors
/// Returns [`GiftError`] if the keyring token is missing/inaccessible or the REST call fails.
#[allow(dead_code)]
pub async fn fetch_user_entitlements(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    with_sku: Option<bool>,
    with_application: Option<bool>,
    entitlement_type: Option<u8>,
    exclude_ended: Option<bool>,
) -> Result<Vec<EntitlementRecord>, GiftError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or_gift(move || backend.token()).await?;
    let api = DiscordGifts::new(&token)?;
    fetch_user_entitlements_with_api(
        &api,
        with_sku,
        with_application,
        entitlement_type,
        exclude_ended,
    )
    .await
}

/// Fetches application entitlements (`GET /users/@me/applications/{application_id}/entitlements`) using an initialized [`DiscordGifts`] client.
///
/// # Errors
/// Returns [`GiftError`] if the `application_id` is invalid or the REST call fails.
#[inline]
pub async fn fetch_application_entitlements_with_api(
    api: &DiscordGifts,
    application_id: Option<String>,
    exclude_consumed: Option<bool>,
) -> Result<Vec<EntitlementRecord>, GiftError> {
    let app_id = application_id.unwrap_or_else(|| gifts::NITRO_APPLICATION_ID.to_string());
    gifts::fetch_application_entitlements(api, &app_id, exclude_consumed.unwrap_or(true)).await
}

/// Fetches application entitlements (`GET /users/@me/applications/{application_id}/entitlements`).
///
/// # Errors
/// Returns [`GiftError`] if the keyring token is missing/inaccessible or the REST call fails.
#[allow(dead_code)]
pub async fn fetch_application_entitlements(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    application_id: Option<String>,
    exclude_consumed: Option<bool>,
) -> Result<Vec<EntitlementRecord>, GiftError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or_gift(move || backend.token()).await?;
    let api = DiscordGifts::new(&token)?;
    fetch_application_entitlements_with_api(&api, application_id, exclude_consumed).await
}

/// Fetches giftable entitlements (`GET /users/@me/entitlements/gifts`) using an initialized [`DiscordGifts`] client.
///
/// # Errors
/// Returns [`GiftError`] if `country_code` is invalid or the REST call fails.
#[inline]
pub async fn fetch_giftable_entitlements_with_api(
    api: &DiscordGifts,
    country_code: Option<&str>,
) -> Result<Vec<EntitlementRecord>, GiftError> {
    gifts::fetch_giftable_entitlements(api, country_code).await
}

/// Fetches giftable entitlements (`GET /users/@me/entitlements/gifts`).
///
/// # Errors
/// Returns [`GiftError`] if the keyring token is missing/inaccessible or the REST call fails.
#[allow(dead_code)]
pub async fn fetch_giftable_entitlements(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    country_code: Option<String>,
) -> Result<Vec<EntitlementRecord>, GiftError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or_gift(move || backend.token()).await?;
    let api = DiscordGifts::new(&token)?;
    fetch_giftable_entitlements_with_api(&api, country_code.as_deref()).await
}

/// Returns the static catalog of Nitro subscription and Server Boost gift plans for `#gift-modal`.
#[inline]
#[must_use]
pub fn list_nitro_gift_plans() -> Vec<NitroGiftPlan> {
    gifts::list_nitro_gift_plans()
}

/// Initializes the modular Tauri v2 `"gifts"` plugin.
#[inline]
#[must_use]
pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("gifts").build()
}

/// Alias for [`init`] (`plugin::gifts()`).
#[allow(dead_code)]
#[inline]
#[must_use]
pub fn gifts<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    init()
}
