//! Modular Tauri v2 plugin (`notifications`) encapsulating Discord guild notification settings,
//! channel notification overrides, effective notification policy resolution, and desktop notifications.
//!
//! Provides `update_guild_notification_settings`, `update_channel_notification_override`,
//! `resolve_effective_notifications`, and `send_desktop_notification`
//! via `tauri::plugin::Builder::new("notifications")`.

use std::collections::BTreeMap;
use std::sync::Arc;

use chord_client::auth::{AuthBackend, AuthError};
use chord_client::notifications::{
    self, ChannelNotificationOverride, ChannelNotificationOverridePatch, DiscordNotifications,
    EffectiveNotificationPolicy, GuildNotificationPolicyInput, GuildNotificationSettingsPatch,
    NotificationError, NotificationTransport,
};

async fn token_or_notification<F>(work: F) -> Result<String, NotificationError>
where
    F: FnOnce() -> Result<Option<String>, AuthError> + Send + 'static,
{
    let joined = tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| NotificationError::Storage)?;
    match joined {
        Ok(Some(token)) if !token.trim().is_empty() => Ok(token),
        Ok(_) => Err(NotificationError::MissingToken),
        Err(_) => Err(NotificationError::Storage),
    }
}

/// Input channel override envelope for `update_channel_notification_override`.
///
/// Accepts a direct [`ChannelNotificationOverridePatch`] (`override` / `override_field` /
/// `override_patch` / `payload`), a wrapped `{ "override": ... }` object, or the
/// `{ "channel_overrides": { "<channel_id>": ... } }` map sent by `notifications.js`.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(untagged)]
pub enum ChannelOverrideInput {
    Envelope {
        channel_overrides: BTreeMap<String, ChannelNotificationOverridePatch>,
    },
    OverrideField {
        #[serde(rename = "override", alias = "override_field", alias = "overrideField")]
        override_field: ChannelNotificationOverridePatch,
    },
    Direct(ChannelNotificationOverridePatch),
}

impl ChannelOverrideInput {
    /// Extracts the [`ChannelNotificationOverridePatch`] for `channel_id`.
    ///
    /// # Errors
    /// Returns [`NotificationError::InvalidChannelId`] if `self` is a non-empty `channel_overrides`
    /// envelope that does not contain `channel_id`.
    #[allow(dead_code)]
    pub fn into_patch(
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

    /// Resolves the effective [`ChannelNotificationOverridePatch`] across flat and nested IPC arguments.
    ///
    /// # Errors
    /// Returns [`NotificationError::InvalidChannelId`] if `patch` or `payload` is a non-empty
    /// `channel_overrides` envelope that does not match `channel_id`.
    #[allow(dead_code)]
    pub fn resolve(
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

/// Resolves the effective [`GuildNotificationSettingsPatch`] from `patch`, `settings`, or `payload`.
#[allow(dead_code)]
#[inline]
#[must_use]
pub fn resolve_guild_settings_patch(
    patch: Option<GuildNotificationSettingsPatch>,
    settings: Option<GuildNotificationSettingsPatch>,
    payload: Option<GuildNotificationSettingsPatch>,
) -> GuildNotificationSettingsPatch {
    patch.or(settings).or(payload).unwrap_or_default()
}

/// Updates guild or DM notification settings (`PATCH /users/@me/guilds/{guild_id}/settings`)
/// using a pre-constructed [`NotificationTransport`].
///
/// # Errors
/// Returns [`NotificationError`] if the token, `guild_id`, or patch payload is invalid, or if the REST call fails.
#[allow(dead_code)]
#[inline]
pub async fn update_guild_notification_settings_with_api<T: NotificationTransport + ?Sized>(
    api: &T,
    token: &str,
    guild_id: &str,
    patch: Option<GuildNotificationSettingsPatch>,
    settings: Option<GuildNotificationSettingsPatch>,
    payload: Option<GuildNotificationSettingsPatch>,
) -> Result<serde_json::Value, NotificationError> {
    let resolved = resolve_guild_settings_patch(patch, settings, payload);
    chord_client::update_guild_notification_settings(api, token, guild_id, &resolved).await
}

/// Updates guild or DM notification settings (`PATCH /users/@me/guilds/{guild_id}/settings`).
///
/// Reads the user Bearer token exclusively from the local [`AuthBackend`] keyring on a blocking worker thread.
///
/// # Errors
/// Returns [`NotificationError`] if the keyring token is missing/inaccessible, parameters are invalid, or the REST call fails.
#[allow(dead_code)]
pub async fn update_guild_notification_settings(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    guild_id: String,
    patch: Option<GuildNotificationSettingsPatch>,
    settings: Option<GuildNotificationSettingsPatch>,
    payload: Option<GuildNotificationSettingsPatch>,
) -> Result<serde_json::Value, NotificationError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or_notification(move || backend.token()).await?;
    let api = DiscordNotifications::new(&token)?;
    update_guild_notification_settings_with_api(&api, &token, &guild_id, patch, settings, payload)
        .await
}

/// Updates a channel or category notification override (`PATCH /users/@me/guilds/{guild_id}/settings`)
/// using a pre-constructed [`NotificationTransport`].
///
/// # Errors
/// Returns [`NotificationError`] if `channel_id` does not match an envelope override, parameters are invalid, or the REST call fails.
#[allow(clippy::too_many_arguments, dead_code)]
#[inline]
pub async fn update_channel_notification_override_with_api<T: NotificationTransport + ?Sized>(
    api: &T,
    token: &str,
    guild_id: &str,
    channel_id: &str,
    r#override: Option<ChannelNotificationOverridePatch>,
    override_field: Option<ChannelNotificationOverridePatch>,
    override_patch: Option<ChannelNotificationOverridePatch>,
    patch: Option<ChannelOverrideInput>,
    payload: Option<ChannelOverrideInput>,
) -> Result<serde_json::Value, NotificationError> {
    let resolved = ChannelOverrideInput::resolve(
        channel_id,
        r#override,
        override_field,
        override_patch,
        patch,
        payload,
    )?;
    chord_client::update_channel_notification_override(api, token, guild_id, channel_id, &resolved)
        .await
}

/// Updates a channel or category notification override (`PATCH /users/@me/guilds/{guild_id}/settings`
/// with `{"channel_overrides": {"<channel_id>": { ... }}}`).
///
/// Reads the user Bearer token exclusively from the local [`AuthBackend`] keyring on a blocking worker thread.
///
/// # Errors
/// Returns [`NotificationError`] if the keyring token is missing/inaccessible, parameters are invalid, or the REST call fails.
#[allow(clippy::too_many_arguments, dead_code)]
pub async fn update_channel_notification_override(
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
    let token = token_or_notification(move || backend.token()).await?;
    let api = DiscordNotifications::new(&token)?;
    update_channel_notification_override_with_api(
        &api,
        &token,
        &guild_id,
        &channel_id,
        r#override,
        override_field,
        override_patch,
        patch,
        payload,
    )
    .await
}

/// Resolves the effective notification policy across the `channel -> parent_category -> guild -> default` cascade.
#[allow(dead_code)]
#[inline]
#[must_use]
pub fn resolve_effective_notifications(
    guild: &GuildNotificationPolicyInput,
    parent_category: Option<&ChannelNotificationOverride>,
    channel: Option<&ChannelNotificationOverride>,
    now_unix: i64,
) -> EffectiveNotificationPolicy {
    chord_client::resolve_effective_notifications(guild, parent_category, channel, now_unix)
}

/// Sends a desktop notification via the native D-Bus `org.freedesktop.Notifications` service.
///
/// # Errors
/// Returns an error string if `DBUS_SESSION_BUS_ADDRESS` is missing/invalid or D-Bus notification delivery fails.
#[allow(dead_code)]
#[inline]
pub fn send_desktop_notification(
    summary: &str,
    body: &str,
    category: Option<&str>,
) -> Result<u32, String> {
    notifications::send_desktop_notification(summary, body, category)
}

/// Initializes the modular Tauri v2 `"notifications"` plugin.
#[inline]
#[must_use]
pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("notifications").build()
}

/// Alias for [`init`] (`plugin::notifications()`).
#[allow(dead_code)]
#[inline]
#[must_use]
pub fn notifications<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    init()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Mutex;

    use chord_client::notifications::{
        MentionKind, MessageNotificationLevel, MESSAGE_NOTIFICATIONS_NO_MESSAGES,
        MESSAGE_NOTIFICATIONS_ONLY_MENTIONS,
    };
    use chord_core::rest::transport::HttpResponse;

    #[derive(Clone, Default)]
    struct FakeNotificationTransport {
        status: u16,
        body: Vec<u8>,
        patches: Arc<Mutex<Vec<(String, Vec<u8>)>>>,
    }

    impl FakeNotificationTransport {
        fn with_body(status: u16, body: &[u8]) -> Self {
            Self {
                status,
                body: body.to_vec(),
                patches: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn patches(&self) -> Vec<(String, Vec<u8>)> {
            self.patches.lock().expect("patches lock").clone()
        }
    }

    impl NotificationTransport for FakeNotificationTransport {
        fn patch<'a>(
            &'a self,
            target: &'a str,
            body: &'a [u8],
        ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, NotificationError>> + Send + 'a>>
        {
            self.patches
                .lock()
                .expect("patches lock")
                .push((target.to_string(), body.to_vec()));
            let response = HttpResponse {
                status: self.status,
                headers: Vec::new(),
                body: self.body.clone(),
            };
            Box::pin(async move { Ok(response) })
        }
    }

    #[test]
    fn token_or_notification_maps_valid_missing_and_storage_errors() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime");

        assert_eq!(
            runtime.block_on(token_or_notification(|| Ok(Some("notif-tok".to_string())))),
            Ok("notif-tok".to_string())
        );
        assert_eq!(
            runtime.block_on(token_or_notification(|| Ok(None))),
            Err(NotificationError::MissingToken)
        );
        assert_eq!(
            runtime.block_on(token_or_notification(|| Ok(Some("   ".to_string())))),
            Err(NotificationError::MissingToken)
        );
        assert_eq!(
            runtime.block_on(token_or_notification(|| {
                Err(AuthError::Keyring {
                    message: "keyring locked".to_string(),
                })
            })),
            Err(NotificationError::Storage)
        );
    }

    #[test]
    fn channel_override_input_resolves_envelope_override_field_and_rejects_foreign_channel() {
        let envelope: ChannelOverrideInput = serde_json::from_value(serde_json::json!({
            "channel_overrides": {
                "1173958353802776576": {
                    "message_notifications": 2,
                    "muted": true
                }
            }
        }))
        .expect("valid envelope");
        assert_eq!(
            ChannelOverrideInput::resolve(
                "1173958353802776576",
                None,
                None,
                None,
                Some(envelope.clone()),
                None,
            ),
            Ok(ChannelNotificationOverridePatch::new()
                .with_message_notifications(MESSAGE_NOTIFICATIONS_NO_MESSAGES)
                .with_muted(true))
        );
        assert_eq!(
            envelope.into_patch("999999999999999999"),
            Err(NotificationError::InvalidChannelId)
        );

        let wrapped: ChannelOverrideInput = serde_json::from_value(serde_json::json!({
            "override": {
                "message_notifications": 1,
                "muted": false
            }
        }))
        .expect("wrapped override");
        assert_eq!(
            ChannelOverrideInput::resolve(
                "1173958353802776576",
                None,
                None,
                None,
                None,
                Some(wrapped),
            ),
            Ok(ChannelNotificationOverridePatch::new()
                .with_message_notifications(MESSAGE_NOTIFICATIONS_ONLY_MENTIONS)
                .with_muted(false))
        );
    }

    #[test]
    fn update_guild_and_channel_notifications_with_api_forward_validated_requests() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime");

        let transport = FakeNotificationTransport::with_body(
            200,
            br#"{"guild_id":"1014537851418980372","message_notifications":1,"suppress_everyone":true}"#,
        );
        let guild_patch = GuildNotificationSettingsPatch::new()
            .with_message_notifications(MESSAGE_NOTIFICATIONS_ONLY_MENTIONS)
            .with_suppress_everyone(true);

        let guild_res = runtime
            .block_on(update_guild_notification_settings_with_api(
                &transport,
                "tok-notif-1",
                "1014537851418980372",
                None,
                Some(guild_patch),
                None,
            ))
            .expect("guild settings ok");
        assert_eq!(guild_res["guild_id"], "1014537851418980372");
        assert_eq!(guild_res["message_notifications"], 1);

        let channel_override = ChannelNotificationOverridePatch::new()
            .with_message_notifications(MESSAGE_NOTIFICATIONS_NO_MESSAGES)
            .with_muted(true);
        let ch_res = runtime
            .block_on(update_channel_notification_override_with_api(
                &transport,
                "tok-notif-1",
                "1014537851418980372",
                "1173958353802776576",
                Some(channel_override),
                None,
                None,
                None,
                None,
            ))
            .expect("channel override ok");
        assert_eq!(ch_res["guild_id"], "1014537851418980372");

        let patches = transport.patches();
        assert_eq!(patches.len(), 2);
        assert_eq!(patches[0].0, "/users/@me/guilds/1014537851418980372/settings");
        assert_eq!(patches[1].0, "/users/@me/guilds/1014537851418980372/settings");
        let ch_body: serde_json::Value =
            serde_json::from_slice(&patches[1].1).expect("channel patch json");
        assert_eq!(
            ch_body["channel_overrides"]["1173958353802776576"]["message_notifications"],
            2
        );
        assert_eq!(
            ch_body["channel_overrides"]["1173958353802776576"]["muted"],
            true
        );
    }

    #[test]
    fn resolve_effective_notifications_delegates_to_core_policy_resolver() {
        let guild = GuildNotificationPolicyInput {
            default_message_notifications: MESSAGE_NOTIFICATIONS_ONLY_MENTIONS,
            message_notifications: Some(MESSAGE_NOTIFICATIONS_ONLY_MENTIONS),
            muted: false,
            mute_config: None,
            suppress_everyone: true,
            suppress_roles: false,
            mobile_push: true,
            mute_scheduled_events: false,
            notify_highlights: 0,
            flags: 0,
        };
        let policy = resolve_effective_notifications(&guild, None, None, 1_793_386_800);
        assert_eq!(policy.effective_level, MessageNotificationLevel::OnlyMentions);
        assert!(policy.should_alert_mention(MentionKind::User));
        assert!(!policy.should_alert_mention(MentionKind::Everyone));
    }
}
