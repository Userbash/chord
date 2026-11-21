//! Modular Tauri v2 plugin (`dm`) encapsulating Discord Direct Messages (DM), Group DM, and
//! Friends/Relationships IPC commands.
//!
//! Provides `ready_dm_friends`, `filter_friends_by_tab`, `create_private_channel`,
//! `open_private_channel_with_cache`, `create_group_dm_shell`, `put_relationship`,
//! `accept_friend_request`, `block_user`, `delete_relationship`, `send_friend_request`,
//! `build_create_dm_payload`, `build_group_dm_recipient_route`,
//! `build_send_friend_request_payload`, and `build_relationship_user_route`
//! via `tauri::plugin::Builder::new("dm")`.

use std::collections::BTreeSet;
use std::sync::Arc;

use chord_client::auth::{AuthBackend, AuthError};
use chord_client::data;
use chord_client::dm_channels::{self, CreateDmRequest};
use chord_client::dm_friends::{
    self, DiscordDmFriends, DmFriendsError, FriendRequestTagInput, FriendsTab,
    PrivateChannelRecord, PutRelationshipParams, ReadyDmFriendsSnapshot, RelationshipRecord,
};
use chord_client::relationships::{self, SendFriendRequest};

async fn token_or_dm<F>(work: F) -> Result<String, DmFriendsError>
where
    F: FnOnce() -> Result<Option<String>, AuthError> + Send + 'static,
{
    let joined = tokio::task::spawn_blocking(work).await;
    if let Ok(Ok(Some(token))) = &joined {
        if !token.trim().is_empty() {
            return Ok(token.clone());
        }
    }
    if let Some(live_token) = data::resolve_live_token().filter(|t| !t.trim().is_empty()) {
        return Ok(live_token);
    }
    match joined {
        Ok(Ok(Some(token))) if !token.trim().is_empty() => Ok(token),
        Ok(Ok(_)) => Err(DmFriendsError::MissingToken),
        Ok(Err(_)) | Err(_) => Err(DmFriendsError::Storage),
    }
}

/// Loads and parses the local `002-READY.json` DM, Group DM, and relationships snapshot without network I/O.
///
/// # Errors
/// Returns [`DmFriendsError`] if the fixtures root cannot be resolved, the file is unreadable,
/// or the READY JSON structure is invalid.
#[allow(dead_code)]
#[inline]
pub fn ready_dm_friends() -> Result<ReadyDmFriendsSnapshot, DmFriendsError> {
    let root = data::fixtures_root().map_err(|_| DmFriendsError::Storage)?;
    dm_friends::load_ready_dm_friends(&root)
}

/// Filters relationship records by the selected Friends tab (`Online`, `All`, `Pending`, `Blocked`).
#[allow(dead_code)]
#[inline]
#[must_use]
pub fn filter_friends_by_tab(
    relationships: &[RelationshipRecord],
    tab: FriendsTab,
    online_user_ids: &BTreeSet<String>,
) -> Vec<RelationshipRecord> {
    dm_friends::filter_relationships_by_tab(relationships, tab, online_user_ids)
}

/// Validates and builds the JSON payload for `POST /users/@me/channels` (`recipient_id` for 1-on-1 DM, `recipients` for Group DM).
///
/// # Errors
/// Returns an error string if `recipient_ids` is empty, exceeds `9` recipients, contains duplicates, or has invalid snowflakes.
#[allow(dead_code)]
#[inline]
pub fn build_create_dm_payload(recipient_ids: Vec<String>) -> Result<serde_json::Value, String> {
    let req = CreateDmRequest { recipient_ids };
    dm_channels::build_create_dm_body(&req)
}

/// Validates `channel_id` and `user_id` snowflakes and builds `/channels/{channel_id}/recipients/{user_id}`.
///
/// # Errors
/// Returns an error string if either `channel_id` or `user_id` is not a canonical positive `u64` snowflake.
#[allow(dead_code)]
#[inline]
pub fn build_group_dm_recipient_route(channel_id: &str, user_id: &str) -> Result<String, String> {
    dm_channels::build_group_dm_recipient_route(channel_id, user_id)
}

/// Validates a Pomelo username (`2..=32` chars, no `@`, `:`, `#`) and builds the `POST /users/@me/relationships` JSON body.
///
/// # Errors
/// Returns an error string if `username` fails validation.
#[allow(dead_code)]
#[inline]
pub fn build_send_friend_request_payload(username: String) -> Result<serde_json::Value, String> {
    let req = SendFriendRequest { username };
    relationships::build_send_friend_request_body(&req)
}

/// Validates `target_user_id` and builds `/users/@me/relationships/{target_user_id}`.
///
/// # Errors
/// Returns an error string if `target_user_id` is not a canonical positive `u64` snowflake.
#[allow(dead_code)]
#[inline]
pub fn build_relationship_user_route(target_user_id: &str) -> Result<String, String> {
    relationships::build_relationship_user_route(target_user_id)
}

/// Opens or creates a DM or Group DM channel (`POST /users/@me/channels`) using an initialized [`DiscordDmFriends`] client.
///
/// # Errors
/// Returns [`DmFriendsError`] if recipient IDs are invalid or the REST call fails.
#[allow(dead_code)]
#[inline]
pub async fn create_private_channel_with_api(
    api: &DiscordDmFriends,
    recipient_ids: &[String],
) -> Result<PrivateChannelRecord, DmFriendsError> {
    let refs: Vec<&str> = recipient_ids.iter().map(String::as_str).collect();
    api.create_private_channel(&refs).await
}

/// Opens or creates a DM or Group DM channel (`POST /users/@me/channels`), reading the user token from [`AuthBackend`].
///
/// # Errors
/// Returns [`DmFriendsError`] if the keyring token is missing/inaccessible, recipient IDs are invalid, or the REST call fails.
#[allow(dead_code)]
pub async fn create_private_channel(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    recipient_ids: Vec<String>,
) -> Result<PrivateChannelRecord, DmFriendsError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or_dm(move || backend.token()).await?;
    let api = DiscordDmFriends::new(&token)?;
    create_private_channel_with_api(&api, &recipient_ids).await
}

/// Opens a 1-on-1 DM channel from `cached_channels` if present (`0` network calls) or creates a private channel via [`DiscordDmFriends`].
///
/// # Errors
/// Returns [`DmFriendsError`] if recipient IDs are invalid or the REST call fails.
#[allow(dead_code)]
#[inline]
pub async fn open_private_channel_with_cache_with_api(
    api: &DiscordDmFriends,
    cached_channels: &[PrivateChannelRecord],
    recipient_ids: &[String],
) -> Result<PrivateChannelRecord, DmFriendsError> {
    let refs: Vec<&str> = recipient_ids.iter().map(String::as_str).collect();
    api.open_private_channel_with_cache(cached_channels, &refs)
        .await
}

/// Opens a 1-on-1 DM channel from `cached_channels` if present or creates a private channel (`POST /users/@me/channels`).
///
/// # Errors
/// Returns [`DmFriendsError`] if the keyring token is missing/inaccessible, recipient IDs are invalid, or the REST call fails.
#[allow(dead_code)]
pub async fn open_private_channel_with_cache(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    cached_channels: Vec<PrivateChannelRecord>,
    recipient_ids: Vec<String>,
) -> Result<PrivateChannelRecord, DmFriendsError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or_dm(move || backend.token()).await?;
    let api = DiscordDmFriends::new(&token)?;
    open_private_channel_with_cache_with_api(&api, &cached_channels, &recipient_ids).await
}

/// Creates a Group DM shell (`POST /users/@me/channels/group-dm-shell`) using an initialized [`DiscordDmFriends`] client.
///
/// # Errors
/// Returns [`DmFriendsError`] if `recipient_id` is invalid or the REST call fails.
#[allow(dead_code)]
#[inline]
pub async fn create_group_dm_shell_with_api(
    api: &DiscordDmFriends,
    recipient_id: &str,
) -> Result<PrivateChannelRecord, DmFriendsError> {
    api.create_group_dm_shell(recipient_id).await
}

/// Creates a Group DM shell (`POST /users/@me/channels/group-dm-shell`), reading the user token from [`AuthBackend`].
///
/// # Errors
/// Returns [`DmFriendsError`] if the keyring token is missing/inaccessible, `recipient_id` is invalid, or the REST call fails.
#[allow(dead_code)]
pub async fn create_group_dm_shell(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    recipient_id: String,
) -> Result<PrivateChannelRecord, DmFriendsError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or_dm(move || backend.token()).await?;
    let api = DiscordDmFriends::new(&token)?;
    create_group_dm_shell_with_api(&api, &recipient_id).await
}

/// Updates or creates a user relationship (`PUT /users/@me/relationships/{user_id}`) using an initialized [`DiscordDmFriends`] client.
///
/// # Errors
/// Returns [`DmFriendsError`] (including [`DmFriendsError::StrangerConfirmationRequired`] for Discord code `80013`) if the request fails.
#[allow(dead_code)]
#[inline]
pub async fn put_relationship_with_api(
    api: &DiscordDmFriends,
    user_id: &str,
    params: &PutRelationshipParams,
) -> Result<(), DmFriendsError> {
    api.put_relationship(user_id, params).await
}

/// Updates or creates a user relationship (`PUT /users/@me/relationships/{user_id}`), reading the user token from [`AuthBackend`].
///
/// # Errors
/// Returns [`DmFriendsError`] if the keyring token is missing/inaccessible or the REST call fails.
#[allow(dead_code)]
pub async fn put_relationship(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    user_id: String,
    params: Option<PutRelationshipParams>,
) -> Result<(), DmFriendsError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or_dm(move || backend.token()).await?;
    let api = DiscordDmFriends::new(&token)?;
    put_relationship_with_api(&api, &user_id, &params.unwrap_or_default()).await
}

/// Accepts an incoming friend request (`PUT /users/@me/relationships/{user_id}`) using an initialized [`DiscordDmFriends`] client.
///
/// # Errors
/// Returns [`DmFriendsError`] if `user_id` is invalid or the REST call fails.
#[allow(dead_code)]
#[inline]
pub async fn accept_friend_request_with_api(
    api: &DiscordDmFriends,
    user_id: &str,
    confirm_stranger_request: Option<bool>,
) -> Result<(), DmFriendsError> {
    api.accept_friend_request(user_id, confirm_stranger_request.unwrap_or(false))
        .await
}

/// Accepts an incoming friend request (`PUT /users/@me/relationships/{user_id}`), reading the user token from [`AuthBackend`].
///
/// # Errors
/// Returns [`DmFriendsError`] if the keyring token is missing/inaccessible or the REST call fails.
#[allow(dead_code)]
pub async fn accept_friend_request(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    user_id: String,
    confirm_stranger_request: Option<bool>,
) -> Result<(), DmFriendsError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or_dm(move || backend.token()).await?;
    let api = DiscordDmFriends::new(&token)?;
    accept_friend_request_with_api(&api, &user_id, confirm_stranger_request).await
}

/// Blocks a user (`PUT /users/@me/relationships/{user_id}` with `{"type": 2}`) using an initialized [`DiscordDmFriends`] client.
///
/// # Errors
/// Returns [`DmFriendsError`] if `user_id` is invalid or the REST call fails.
#[allow(dead_code)]
#[inline]
pub async fn block_user_with_api(
    api: &DiscordDmFriends,
    user_id: &str,
) -> Result<(), DmFriendsError> {
    api.block_user(user_id).await
}

/// Blocks a user (`PUT /users/@me/relationships/{user_id}` with `{"type": 2}`), reading the user token from [`AuthBackend`].
///
/// # Errors
/// Returns [`DmFriendsError`] if the keyring token is missing/inaccessible or the REST call fails.
#[allow(dead_code)]
pub async fn block_user(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    user_id: String,
) -> Result<(), DmFriendsError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or_dm(move || backend.token()).await?;
    let api = DiscordDmFriends::new(&token)?;
    block_user_with_api(&api, &user_id).await
}

/// Removes a friend, declines/cancels a friend request, or unblocks a user (`DELETE /users/@me/relationships/{user_id}`)
/// using an initialized [`DiscordDmFriends`] client.
///
/// # Errors
/// Returns [`DmFriendsError`] if `user_id` is invalid or the REST call fails.
#[allow(dead_code)]
#[inline]
pub async fn delete_relationship_with_api(
    api: &DiscordDmFriends,
    user_id: &str,
) -> Result<(), DmFriendsError> {
    api.delete_relationship(user_id).await
}

/// Removes a friend, declines/cancels a friend request, or unblocks a user (`DELETE /users/@me/relationships/{user_id}`),
/// reading the user token from [`AuthBackend`].
///
/// # Errors
/// Returns [`DmFriendsError`] if the keyring token is missing/inaccessible or the REST call fails.
#[allow(dead_code)]
pub async fn delete_relationship(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    user_id: String,
) -> Result<(), DmFriendsError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or_dm(move || backend.token()).await?;
    let api = DiscordDmFriends::new(&token)?;
    delete_relationship_with_api(&api, &user_id).await
}

/// Sends a friend request (`POST /users/@me/relationships`) supporting both Pomelo (`username`) and legacy (`username#1234`) tags
/// using an initialized [`DiscordDmFriends`] client.
///
/// # Errors
/// Returns [`DmFriendsError`] if `discord_tag` is invalid or the REST call fails.
#[allow(dead_code)]
#[inline]
pub async fn send_friend_request_with_api(
    api: &DiscordDmFriends,
    discord_tag: &str,
    note: Option<&str>,
) -> Result<(), DmFriendsError> {
    let input = FriendRequestTagInput::parse(discord_tag, note)?;
    api.send_friend_request(&input).await
}

/// Sends a friend request (`POST /users/@me/relationships`), reading the user token from [`AuthBackend`].
///
/// # Errors
/// Returns [`DmFriendsError`] if the keyring token is missing/inaccessible, `discord_tag` is invalid, or the REST call fails.
#[allow(dead_code)]
pub async fn send_friend_request(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    discord_tag: String,
    note: Option<String>,
) -> Result<(), DmFriendsError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or_dm(move || backend.token()).await?;
    let api = DiscordDmFriends::new(&token)?;
    send_friend_request_with_api(&api, &discord_tag, note.as_deref()).await
}

/// Initializes the modular Tauri v2 `"dm"` plugin.
#[inline]
#[must_use]
pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("dm").build()
}

/// Alias for [`init`] (`plugin::dm()`).
#[allow(dead_code)]
#[inline]
#[must_use]
pub fn dm<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    init()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chord_client::dm_friends::{
        DmFriendsTransport, PrivateChannelType, RelationshipType,
        DISCORD_ERR_RELATIONSHIP_INVALID_NO_CONFIRMATION, USER_CHANNELS_PATH,
        USER_GROUP_DM_SHELL_PATH, USER_RELATIONSHIPS_PATH,
    };
    use chord_core::rest::transport::HttpResponse;
    use std::collections::BTreeMap;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Mutex;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RecordedCall {
        method: String,
        target: String,
        body: Vec<u8>,
    }

    #[derive(Clone, Default)]
    struct FakeDmTransport {
        status: u16,
        body: Vec<u8>,
        calls: Arc<Mutex<Vec<RecordedCall>>>,
    }

    impl FakeDmTransport {
        fn with_response(status: u16, body: &[u8]) -> Self {
            Self {
                status,
                body: body.to_vec(),
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn calls(&self) -> Vec<RecordedCall> {
            self.calls.lock().expect("calls lock").clone()
        }

        fn record(&self, method: &str, target: &str, body: &[u8]) -> HttpResponse {
            self.calls.lock().expect("calls lock").push(RecordedCall {
                method: method.to_string(),
                target: target.to_string(),
                body: body.to_vec(),
            });
            HttpResponse {
                status: self.status,
                headers: Vec::new(),
                body: self.body.clone(),
            }
        }
    }

    impl DmFriendsTransport for FakeDmTransport {
        fn get<'a>(
            &'a self,
            target: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, DmFriendsError>> + Send + 'a>>
        {
            let resp = self.record("GET", target, &[]);
            Box::pin(async move { Ok(resp) })
        }

        fn post<'a>(
            &'a self,
            target: &'a str,
            body: &'a [u8],
        ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, DmFriendsError>> + Send + 'a>>
        {
            let resp = self.record("POST", target, body);
            Box::pin(async move { Ok(resp) })
        }

        fn put<'a>(
            &'a self,
            target: &'a str,
            body: &'a [u8],
        ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, DmFriendsError>> + Send + 'a>>
        {
            let resp = self.record("PUT", target, body);
            Box::pin(async move { Ok(resp) })
        }

        fn delete<'a>(
            &'a self,
            target: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, DmFriendsError>> + Send + 'a>>
        {
            let resp = self.record("DELETE", target, &[]);
            Box::pin(async move { Ok(resp) })
        }
    }

    #[test]
    fn create_and_cache_private_channels_and_group_dm_shell_delegate_accurately() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime");

        let dm_json = br#"{
            "id": "555000111222333444",
            "type": 1,
            "flags": 0,
            "last_message_id": "555000111222333999",
            "recipients": [
                {"id": "100000000000000001", "username": "alice", "discriminator": "0"}
            ]
        }"#;
        let transport = FakeDmTransport::with_response(200, dm_json);
        let api = DiscordDmFriends::with_transport("dm-secret-token", Arc::new(transport.clone()))
            .expect("client");

        let recipients = vec!["100000000000000001".to_string()];
        let created = runtime
            .block_on(create_private_channel_with_api(&api, &recipients))
            .expect("create_private_channel_with_api");
        assert_eq!(created.id, "555000111222333444");
        assert_eq!(created.channel_type, PrivateChannelType::Dm);
        assert_eq!(transport.calls().len(), 1);
        assert_eq!(transport.calls()[0].method, "POST");
        assert_eq!(transport.calls()[0].target, USER_CHANNELS_PATH);

        // Cache hit for 1-on-1 DM performs 0 additional network requests
        let cached = vec![created.clone()];
        let hit = runtime
            .block_on(open_private_channel_with_cache_with_api(
                &api,
                &cached,
                &recipients,
            ))
            .expect("cache hit");
        assert_eq!(hit, created);
        assert_eq!(transport.calls().len(), 1);

        // Group DM shell creation
        let shell_json = br#"{"id": "777000111222333444", "type": 3, "recipients": []}"#;
        let shell_transport = FakeDmTransport::with_response(201, shell_json);
        let shell_api =
            DiscordDmFriends::with_transport("dm-secret-token", Arc::new(shell_transport.clone()))
                .expect("shell client");
        let shell = runtime
            .block_on(create_group_dm_shell_with_api(
                &shell_api,
                "100000000000000002",
            ))
            .expect("create_group_dm_shell_with_api");
        assert_eq!(shell.id, "777000111222333444");
        assert_eq!(shell.channel_type, PrivateChannelType::GroupDm);
        assert_eq!(shell_transport.calls().len(), 1);
        assert_eq!(
            shell_transport.calls()[0].target,
            USER_GROUP_DM_SHELL_PATH
        );
    }

    #[test]
    fn relationship_commands_handle_accept_block_delete_send_and_stranger_80013() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime");

        let ok_transport = FakeDmTransport::with_response(204, b"");
        let api =
            DiscordDmFriends::with_transport("dm-secret-token", Arc::new(ok_transport.clone()))
                .expect("client");

        runtime
            .block_on(accept_friend_request_with_api(
                &api,
                "100000000000000003",
                None,
            ))
            .expect("accept_friend_request_with_api");
        runtime
            .block_on(block_user_with_api(&api, "100000000000000004"))
            .expect("block_user_with_api");
        runtime
            .block_on(delete_relationship_with_api(&api, "100000000000000005"))
            .expect("delete_relationship_with_api");
        runtime
            .block_on(send_friend_request_with_api(
                &api,
                "wumpus_friend",
                Some("Hello!"),
            ))
            .expect("send_friend_request_with_api");

        let calls = ok_transport.calls();
        assert_eq!(calls.len(), 4);
        assert_eq!(calls[0].method, "PUT");
        assert_eq!(
            calls[0].target,
            "/users/@me/relationships/100000000000000003"
        );
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&calls[0].body).expect("accept body"),
            serde_json::json!({"type": 1, "confirm_stranger_request": false})
        );
        assert_eq!(calls[1].method, "PUT");
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&calls[1].body).expect("block body"),
            serde_json::json!({"type": 2})
        );
        assert_eq!(calls[2].method, "DELETE");
        assert_eq!(
            calls[2].target,
            "/users/@me/relationships/100000000000000005"
        );
        assert_eq!(calls[3].method, "POST");
        assert_eq!(calls[3].target, USER_RELATIONSHIPS_PATH);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&calls[3].body).expect("send body"),
            serde_json::json!({"username": "wumpus_friend", "discriminator": null, "note": "Hello!"})
        );

        // Stranger confirmation required (80013)
        let stranger_transport = FakeDmTransport::with_response(
            400,
            br#"{"code": 80013, "message": "Confirm stranger friend request"}"#,
        );
        let stranger_api =
            DiscordDmFriends::with_transport("dm-secret-token", Arc::new(stranger_transport))
                .expect("stranger client");
        let err = runtime
            .block_on(put_relationship_with_api(
                &stranger_api,
                "100000000000000006",
                &PutRelationshipParams {
                    relationship_type: Some(RelationshipType::Friend),
                    from_friend_suggestion: None,
                    confirm_stranger_request: Some(false),
                },
            ))
            .expect_err("must return StrangerConfirmationRequired");
        assert_eq!(
            err,
            DmFriendsError::StrangerConfirmationRequired {
                user_id: "100000000000000006".to_string(),
                code: DISCORD_ERR_RELATIONSHIP_INVALID_NO_CONFIRMATION,
            }
        );
    }

    #[test]
    fn route_and_payload_builders_and_tab_filters_enforce_contracts() {
        assert_eq!(
            build_create_dm_payload(vec!["100000000000000001".to_string()]).expect("single dm"),
            serde_json::json!({"recipient_id": "100000000000000001"})
        );
        assert_eq!(
            build_group_dm_recipient_route("100000000000000001", "100000000000000002")
                .expect("group dm route"),
            "/channels/100000000000000001/recipients/100000000000000002"
        );
        assert_eq!(
            build_send_friend_request_payload("  pomelo_user  ".to_string())
                .expect("friend request payload"),
            serde_json::json!({"username": "pomelo_user", "discriminator": null})
        );
        assert_eq!(
            build_relationship_user_route("100000000000000003").expect("rel route"),
            "/users/@me/relationships/100000000000000003"
        );

        let relationships = vec![
            RelationshipRecord {
                id: "1".to_string(),
                user_id: "1".to_string(),
                relationship_type: RelationshipType::Friend,
                nickname: None,
                since: None,
                is_spam_request: false,
                user_ignored: false,
                user: None,
            },
            RelationshipRecord {
                id: "2".to_string(),
                user_id: "2".to_string(),
                relationship_type: RelationshipType::Blocked,
                nickname: None,
                since: None,
                is_spam_request: false,
                user_ignored: false,
                user: None,
            },
        ];
        let mut online = BTreeSet::new();
        online.insert("1".to_string());
        let online_friends = filter_friends_by_tab(&relationships, FriendsTab::Online, &online);
        assert_eq!(online_friends.len(), 1);
        assert_eq!(online_friends[0].user_id, "1");

        let blocked = filter_friends_by_tab(&relationships, FriendsTab::Blocked, &online);
        assert_eq!(blocked.len(), 1);
        assert_eq!(blocked[0].user_id, "2");
        let _ = BTreeMap::<String, String>::new();
    }

    #[test]
    fn token_or_dm_maps_missing_and_storage_errors() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime");

        assert_eq!(
            runtime.block_on(token_or_dm(|| Ok(Some("valid-dm-token".to_string())))),
            Ok("valid-dm-token".to_string())
        );
        assert_eq!(
            runtime.block_on(token_or_dm(|| Ok(None))),
            Err(DmFriendsError::MissingToken)
        );
        assert_eq!(
            runtime.block_on(token_or_dm(|| Ok(Some("   ".to_string())))),
            Err(DmFriendsError::MissingToken)
        );
        assert_eq!(
            runtime.block_on(token_or_dm(|| {
                Err(AuthError::Keyring {
                    message: "Secret Service unavailable".to_string(),
                })
            })),
            Err(DmFriendsError::Storage)
        );
    }
}
