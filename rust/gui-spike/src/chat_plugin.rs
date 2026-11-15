//! Modular Tauri v2 plugin (`chat`) encapsulating Discord channel message, read-state ACK,
//! reaction, and typing indicator IPC commands and typed DTOs.
//!
//! Provides `messages_page`, `send_message`, `send_message_with_api`,
//! `send_message_with_attachments_with_api`, `edit_message`, `edit_message_with_api`,
//! `delete_message`, `delete_message_with_api`, `reply_message`, `reply_message_with_api`,
//! `ack_message`, `ack_message_with_api`, `ack_message_with_options_with_api`,
//! `ack_messages_bulk_with_api`, `toggle_reaction`, `toggle_reaction_with_api`,
//! `add_reaction`, `add_reaction_with_api`, `send_typing`, and `send_typing_with_api`
//! via `tauri::plugin::Builder::new("chat")`.

use std::sync::Arc;

use chord_client::attachments::UploadedAttachment;
use chord_client::auth::{AuthBackend, AuthError};
use chord_client::data::{self, DataError, MessagesPage, MessagesPageRequest, MessagesPosition};
use chord_client::messages::{
    self, AckBulkEntry, AckMessageOptions, DiscordMessageSender, DiscordReadState, MessageActions,
    MessageError, MessageReceipt, MessageSender, ReadStateAck,
};
use chord_client::reactions::{self, DiscordReactionApi, ReactionApi, ReactionError};
use chord_client::typing::{self, DiscordTyping, TypingApi, TypingCooldowns, TypingError};
use serde::{Deserialize, Serialize};

/// Typed request DTO for `messages_page` pagination (`cursor`, `before`, `position`, `limit`).
#[allow(dead_code)]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessagesPageQueryDto {
    /// Target channel Snowflake or fixture identifier (`1..=64` ASCII bytes).
    #[serde(alias = "channel_id")]
    pub channel_id: String,
    /// Optional forward pagination cursor (last message ID of the previous page).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    /// Optional backward pagination cursor (oldest message ID of the current window).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    /// Optional starting position (`"start"` or `"latest"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<String>,
    /// Optional page size (`1..=100`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

/// Typed request DTO for sending a channel message (`POST /channels/{channel_id}/messages`).
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendChatMessageDto {
    /// Target channel Snowflake ID (`1..=64` ASCII alphanumeric/`_`/`-` bytes).
    #[serde(alias = "channel_id")]
    pub channel_id: String,
    /// Message text content (`<= 2000` characters).
    pub content: String,
    /// Client idempotency nonce string.
    pub nonce: String,
    /// Optional pre-uploaded attachment descriptors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<UploadedAttachment>,
}

/// Typed request DTO for editing an existing channel message (`PATCH /channels/{channel_id}/messages/{message_id}`).
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditChatMessageDto {
    /// Target channel Snowflake ID.
    #[serde(alias = "channel_id")]
    pub channel_id: String,
    /// Target message Snowflake ID.
    #[serde(alias = "message_id")]
    pub message_id: String,
    /// Updated message text content (`1..=2000` characters).
    pub content: String,
}

/// Typed request DTO for deleting a channel message (`DELETE /channels/{channel_id}/messages/{message_id}`).
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteChatMessageDto {
    /// Target channel Snowflake ID.
    #[serde(alias = "channel_id")]
    pub channel_id: String,
    /// Target message Snowflake ID.
    #[serde(alias = "message_id")]
    pub message_id: String,
}

/// Typed request DTO for replying to a channel message (`POST /channels/{channel_id}/messages` with `message_reference`).
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplyChatMessageDto {
    /// Target channel Snowflake ID.
    #[serde(alias = "channel_id")]
    pub channel_id: String,
    /// Reply message text content (`1..=2000` characters).
    pub content: String,
    /// Referenced message Snowflake ID (`reply_to` / `replyTo`).
    #[serde(alias = "reply_to")]
    pub reply_to: String,
}

/// Typed request DTO for acknowledging a channel's read state (`POST /channels/{channel_id}/messages/{message_id}/ack`).
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AckChatMessageDto {
    /// Target channel Snowflake ID.
    #[serde(alias = "channel_id")]
    pub channel_id: String,
    /// Acknowledged message Snowflake ID.
    #[serde(alias = "message_id")]
    pub message_id: String,
    /// Read-state `last_viewed` day counter.
    #[serde(alias = "last_viewed")]
    pub last_viewed: u64,
    /// Optional read-state ack token (`null` when omitted, never the user Bearer token).
    #[serde(default)]
    pub token: Option<String>,
    /// Optional read-state flags bitmask.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<u32>,
    /// Optional manual mark-unread flag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual: Option<bool>,
    /// Optional unread mention count override.
    #[serde(
        default,
        alias = "mention_count",
        skip_serializing_if = "Option::is_none"
    )]
    pub mention_count: Option<u32>,
}

impl AckChatMessageDto {
    /// Converts this DTO into [`AckMessageOptions`] for `POST /channels/{id}/messages/{id}/ack`.
    #[allow(dead_code)]
    #[inline]
    #[must_use]
    pub fn to_ack_options(&self) -> AckMessageOptions {
        AckMessageOptions {
            token: self.token.clone(),
            last_viewed: Some(self.last_viewed),
            flags: self.flags,
            manual: self.manual,
            mention_count: self.mention_count,
        }
    }
}

/// Typed request DTO for bulk read-state acknowledgment (`POST /read-states/ack-bulk`).
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkAckChatMessagesDto {
    /// Batch of read-state entries (`1..=100`).
    #[serde(alias = "read_states")]
    pub read_states: Vec<AckBulkEntry>,
}

/// Typed request DTO for toggling or adding a reaction (`PUT` / `DELETE /channels/{channel_id}/messages/{message_id}/reactions/{emoji}/@me`).
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleReactionDto {
    /// Target channel Snowflake ID.
    #[serde(alias = "channel_id")]
    pub channel_id: String,
    /// Target message Snowflake ID.
    #[serde(alias = "message_id")]
    pub message_id: String,
    /// Unicode emoji or custom emoji descriptor (`name:id` or `a:name:id`).
    pub emoji: String,
    /// Desired active state (`true` to add `PUT`, `false` to remove `DELETE`).
    #[serde(default = "default_active_true")]
    pub active: bool,
}

const fn default_active_true() -> bool {
    true
}

/// Typed request DTO for sending a channel typing indicator (`POST /channels/{channel_id}/typing`).
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendTypingDto {
    /// Target channel Snowflake ID.
    #[serde(alias = "channel_id")]
    pub channel_id: String,
}

async fn token_or_chat<E, F>(work: F, storage: fn() -> E, missing: fn() -> E) -> Result<String, E>
where
    F: FnOnce() -> Result<Option<String>, AuthError> + Send + 'static,
    E: Send + 'static,
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
        Ok(Ok(_)) => Err(missing()),
        Ok(Err(_)) | Err(_) => Err(storage()),
    }
}

/// Loads a paginated window of channel messages from the local fixture/live store without blocking the async runtime.
///
/// # Errors
/// Returns [`DataError`] if the fixtures root cannot be resolved, parameters are invalid, or message parsing fails.
#[allow(dead_code)]
pub async fn messages_page(
    channel_id: String,
    cursor: Option<String>,
    before: Option<String>,
    position: Option<String>,
    limit: Option<usize>,
) -> Result<MessagesPage, DataError> {
    tokio::task::spawn_blocking(move || {
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
    .map_err(|err| DataError::invalid_arg(format!("blocking task failed: {err}")))?
}

/// Sends a text message to `channel_id` (`POST /channels/{channel_id}/messages`) using an initialized [`MessageSender`].
///
/// # Errors
/// Returns [`MessageError`] if `channel_id` or `content` fails validation or the REST request fails.
#[allow(dead_code)]
#[inline]
pub async fn send_message_with_api<S: MessageSender + ?Sized>(
    sender: &S,
    channel_id: &str,
    content: &str,
    nonce: &str,
) -> Result<MessageReceipt, MessageError> {
    messages::send_message(sender, channel_id, content, nonce).await
}

/// Sends a message with optional pre-uploaded attachments (`POST /channels/{channel_id}/messages`)
/// using an initialized [`DiscordMessageSender`].
///
/// # Errors
/// Returns [`MessageError`] if `channel_id`, `content`, or `attachments` fail validation or the REST request fails.
#[allow(dead_code)]
#[inline]
pub async fn send_message_with_attachments_with_api(
    sender: &DiscordMessageSender,
    channel_id: &str,
    content: &str,
    nonce: &str,
    attachments: Option<&[UploadedAttachment]>,
) -> Result<MessageReceipt, MessageError> {
    if let Some(att) = attachments.filter(|a| !a.is_empty()) {
        sender
            .send_with_attachments(channel_id, content, nonce, att)
            .await
    } else {
        messages::send_message(sender, channel_id, content, nonce).await
    }
}

/// Sends a new message to `channel_id` (`POST /channels/{channel_id}/messages`), reading the user token from [`AuthBackend`].
///
/// # Errors
/// Returns [`MessageError`] if the keyring token is missing/inaccessible, arguments are invalid, or the REST call fails.
#[allow(dead_code)]
pub async fn send_message(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    content: String,
    nonce: String,
    attachments: Option<Vec<UploadedAttachment>>,
) -> Result<MessageReceipt, MessageError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or_chat(
        move || backend.token(),
        || MessageError::Storage,
        || MessageError::Unauthorized,
    )
    .await?;
    let sender = DiscordMessageSender::new(&token)?;
    send_message_with_attachments_with_api(
        &sender,
        &channel_id,
        &content,
        &nonce,
        attachments.as_deref(),
    )
    .await
}

/// Edits an existing message (`PATCH /channels/{channel_id}/messages/{message_id}`) using an initialized [`MessageActions`] client.
///
/// # Errors
/// Returns [`MessageError`] if `channel_id`, `message_id`, or `content` is invalid, or the REST call fails.
#[allow(dead_code)]
#[inline]
pub async fn edit_message_with_api<A: MessageActions + ?Sized>(
    actions: &A,
    channel_id: &str,
    message_id: &str,
    content: &str,
) -> Result<(), MessageError> {
    messages::edit_message(actions, channel_id, message_id, content).await
}

/// Edits an existing message (`PATCH /channels/{channel_id}/messages/{message_id}`), reading the user token from [`AuthBackend`].
///
/// # Errors
/// Returns [`MessageError`] if the keyring token is missing/inaccessible, arguments are invalid, or the REST call fails.
#[allow(dead_code)]
pub async fn edit_message(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    message_id: String,
    content: String,
) -> Result<(), MessageError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or_chat(
        move || backend.token(),
        || MessageError::Storage,
        || MessageError::Unauthorized,
    )
    .await?;
    let api = DiscordMessageSender::new(&token)?;
    edit_message_with_api(&api, &channel_id, &message_id, &content).await
}

/// Deletes a message (`DELETE /channels/{channel_id}/messages/{message_id}`) using an initialized [`MessageActions`] client.
///
/// # Errors
/// Returns [`MessageError`] if `channel_id` or `message_id` is invalid, or the REST call fails.
#[allow(dead_code)]
#[inline]
pub async fn delete_message_with_api<A: MessageActions + ?Sized>(
    actions: &A,
    channel_id: &str,
    message_id: &str,
) -> Result<(), MessageError> {
    messages::delete_message(actions, channel_id, message_id).await
}

/// Deletes a message (`DELETE /channels/{channel_id}/messages/{message_id}`), reading the user token from [`AuthBackend`].
///
/// # Errors
/// Returns [`MessageError`] if the keyring token is missing/inaccessible, arguments are invalid, or the REST call fails.
#[allow(dead_code)]
pub async fn delete_message(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    message_id: String,
) -> Result<(), MessageError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or_chat(
        move || backend.token(),
        || MessageError::Storage,
        || MessageError::Unauthorized,
    )
    .await?;
    let api = DiscordMessageSender::new(&token)?;
    delete_message_with_api(&api, &channel_id, &message_id).await
}

/// Sends a reply to an existing message (`POST /channels/{channel_id}/messages` with `message_reference`)
/// using an initialized [`MessageActions`] client.
///
/// # Errors
/// Returns [`MessageError`] if `channel_id`, `reply_to`, or `content` is invalid, or the REST call fails.
#[allow(dead_code)]
#[inline]
pub async fn reply_message_with_api<A: MessageActions + ?Sized>(
    actions: &A,
    channel_id: &str,
    content: &str,
    reply_to: &str,
) -> Result<(), MessageError> {
    messages::reply_message(actions, channel_id, content, reply_to).await
}

/// Sends a reply to an existing message (`POST /channels/{channel_id}/messages` with `message_reference`),
/// reading the user token from [`AuthBackend`].
///
/// # Errors
/// Returns [`MessageError`] if the keyring token is missing/inaccessible, arguments are invalid, or the REST call fails.
#[allow(dead_code)]
pub async fn reply_message(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    content: String,
    reply_to: String,
) -> Result<(), MessageError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or_chat(
        move || backend.token(),
        || MessageError::Storage,
        || MessageError::Unauthorized,
    )
    .await?;
    let api = DiscordMessageSender::new(&token)?;
    reply_message_with_api(&api, &channel_id, &content, &reply_to).await
}

/// Marks a channel message as read (`POST /channels/{channel_id}/messages/{message_id}/ack`)
/// using an initialized [`ReadStateAck`] client.
///
/// # Errors
/// Returns [`MessageError`] if `channel_id` or `message_id` is invalid, or the REST call fails.
#[allow(dead_code)]
#[inline]
pub async fn ack_message_with_api<A: ReadStateAck + ?Sized>(
    api: &A,
    channel_id: &str,
    message_id: &str,
    last_viewed: u64,
    token: Option<String>,
) -> Result<(), MessageError> {
    messages::ack_message(api, channel_id, message_id, last_viewed, token).await
}

/// Marks a channel message as read with optional `flags`, `manual`, and `mention_count` fields
/// using an initialized [`DiscordReadState`] client.
///
/// # Errors
/// Returns [`MessageError`] if `channel_id` or `message_id` is invalid, or the REST call fails.
#[allow(dead_code)]
#[inline]
pub async fn ack_message_with_options_with_api(
    api: &DiscordReadState,
    channel_id: &str,
    message_id: &str,
    options: AckMessageOptions,
) -> Result<(), MessageError> {
    messages::ack_message_with_options(api, channel_id, message_id, options).await
}

/// Acknowledges a batch of channel read states (`POST /read-states/ack-bulk`, `1..=100` entries)
/// using an initialized [`DiscordReadState`] client.
///
/// # Errors
/// Returns [`MessageError`] if `entries` is empty, exceeds `100` items, contains invalid IDs, or the REST call fails.
#[allow(dead_code)]
#[inline]
pub async fn ack_messages_bulk_with_api(
    api: &DiscordReadState,
    entries: &[AckBulkEntry],
) -> Result<(), MessageError> {
    messages::ack_messages_bulk(api, entries).await
}

/// Marks a channel message as read (`POST /channels/{channel_id}/messages/{message_id}/ack`),
/// reading the user Bearer token from [`AuthBackend`].
///
/// # Errors
/// Returns [`MessageError`] if the keyring token is missing/inaccessible, arguments are invalid, or the REST call fails.
#[allow(dead_code)]
pub async fn ack_message(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    message_id: String,
    last_viewed: u64,
    token: Option<String>,
) -> Result<(), MessageError> {
    let backend = Arc::clone(backend.inner());
    let bearer = token_or_chat(
        move || backend.token(),
        || MessageError::Storage,
        || MessageError::Unauthorized,
    )
    .await?;
    let api = DiscordReadState::new(&bearer)?;
    ack_message_with_api(&api, &channel_id, &message_id, last_viewed, token).await
}

/// Toggles a reaction on a message (`PUT` or `DELETE /channels/{channel_id}/messages/{message_id}/reactions/{emoji}/@me`)
/// using an initialized [`ReactionApi`] client.
///
/// # Errors
/// Returns [`ReactionError`] if `channel_id`, `message_id`, or `emoji` is invalid, or the REST call fails.
#[allow(dead_code)]
#[inline]
pub async fn toggle_reaction_with_api<A: ReactionApi + ?Sized>(
    api: &A,
    channel_id: &str,
    message_id: &str,
    emoji: &str,
    active: bool,
) -> Result<(), ReactionError> {
    reactions::toggle_reaction(api, channel_id, message_id, emoji, active).await
}

/// Adds a reaction to a message (`PUT /channels/{channel_id}/messages/{message_id}/reactions/{emoji}/@me`)
/// using an initialized [`ReactionApi`] client.
///
/// # Errors
/// Returns [`ReactionError`] if `channel_id`, `message_id`, or `emoji` is invalid, or the REST call fails.
#[allow(dead_code)]
#[inline]
pub async fn add_reaction_with_api<A: ReactionApi + ?Sized>(
    api: &A,
    channel_id: &str,
    message_id: &str,
    emoji: &str,
) -> Result<(), ReactionError> {
    reactions::add_reaction(api, channel_id, message_id, emoji).await
}

/// Toggles a reaction on a message (`PUT` or `DELETE /channels/{channel_id}/messages/{message_id}/reactions/{emoji}/@me`),
/// reading the user token from [`AuthBackend`].
///
/// # Errors
/// Returns [`ReactionError`] if the keyring token is missing/inaccessible, arguments are invalid, or the REST call fails.
#[allow(dead_code)]
pub async fn toggle_reaction(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    message_id: String,
    emoji: String,
    active: bool,
) -> Result<(), ReactionError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or_chat(
        move || backend.token(),
        || ReactionError::Storage,
        || ReactionError::Unauthorized,
    )
    .await?;
    let api = DiscordReactionApi::new(&token)?;
    toggle_reaction_with_api(&api, &channel_id, &message_id, &emoji, active).await
}

/// Adds a reaction to a message (`PUT /channels/{channel_id}/messages/{message_id}/reactions/{emoji}/@me`),
/// reading the user token from [`AuthBackend`].
///
/// # Errors
/// Returns [`ReactionError`] if the keyring token is missing/inaccessible, arguments are invalid, or the REST call fails.
#[allow(dead_code)]
pub async fn add_reaction(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    message_id: String,
    emoji: String,
) -> Result<(), ReactionError> {
    toggle_reaction(backend, channel_id, message_id, emoji, true).await
}

/// Sends an outgoing typing indicator (`POST /channels/{channel_id}/typing`)
/// using an initialized [`TypingApi`] client.
///
/// # Errors
/// Returns [`TypingError`] if `channel_id` is not a valid Snowflake or the REST call fails.
#[allow(dead_code)]
#[inline]
pub async fn send_typing_with_api<A: TypingApi + ?Sized>(
    api: &A,
    channel_id: &str,
) -> Result<Option<TypingCooldowns>, TypingError> {
    typing::send_typing(api, channel_id).await
}

/// Sends an outgoing typing indicator (`POST /channels/{channel_id}/typing`),
/// reading the user token from [`AuthBackend`].
///
/// # Errors
/// Returns [`TypingError`] if the keyring token is missing/inaccessible, `channel_id` is invalid, or the REST call fails.
#[allow(dead_code)]
pub async fn send_typing(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
) -> Result<Option<TypingCooldowns>, TypingError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or_chat(
        move || backend.token(),
        || TypingError::Storage,
        || TypingError::Unauthorized,
    )
    .await?;
    let api = DiscordTyping::new(&token)?;
    send_typing_with_api(&api, &channel_id).await
}

/// Initializes the modular Tauri v2 `"chat"` plugin.
#[allow(dead_code)]
#[inline]
#[must_use]
pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("chat").build()
}

/// Alias for [`init`] (`plugin::chat()`).
#[allow(dead_code)]
#[inline]
#[must_use]
pub fn chat<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    init()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chord_client::messages::AckTransport;
    use chord_core::rest::transport::HttpResponse;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Mutex;

    #[derive(Clone)]
    struct FakeMessageClient {
        send_result: Result<MessageReceipt, MessageError>,
        action_result: Result<(), MessageError>,
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl FakeMessageClient {
        fn ok(receipt: MessageReceipt) -> Self {
            Self {
                send_result: Ok(receipt),
                action_result: Ok(()),
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().expect("calls lock").clone()
        }
    }

    impl MessageSender for FakeMessageClient {
        fn send_message<'a>(
            &'a self,
            channel_id: &'a str,
            content: &'a str,
            nonce: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<MessageReceipt, MessageError>> + Send + 'a>>
        {
            self.calls
                .lock()
                .expect("calls lock")
                .push(format!("SEND {channel_id} {nonce} {content}"));
            let res = self.send_result.clone();
            Box::pin(async move { res })
        }
    }

    impl MessageActions for FakeMessageClient {
        fn edit_message<'a>(
            &'a self,
            channel_id: &'a str,
            message_id: &'a str,
            content: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<(), MessageError>> + Send + 'a>> {
            self.calls
                .lock()
                .expect("calls lock")
                .push(format!("EDIT {channel_id}/{message_id} {content}"));
            let res = self.action_result.clone();
            Box::pin(async move { res })
        }

        fn delete_message<'a>(
            &'a self,
            channel_id: &'a str,
            message_id: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<(), MessageError>> + Send + 'a>> {
            self.calls
                .lock()
                .expect("calls lock")
                .push(format!("DELETE {channel_id}/{message_id}"));
            let res = self.action_result.clone();
            Box::pin(async move { res })
        }

        fn reply_message<'a>(
            &'a self,
            channel_id: &'a str,
            content: &'a str,
            reply_to: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<(), MessageError>> + Send + 'a>> {
            self.calls
                .lock()
                .expect("calls lock")
                .push(format!("REPLY {channel_id}->{reply_to} {content}"));
            let res = self.action_result.clone();
            Box::pin(async move { res })
        }
    }

    #[derive(Clone, Default)]
    struct FakeAckTransport {
        status: u16,
        posts: Arc<Mutex<Vec<(String, Vec<u8>)>>>,
    }

    impl FakeAckTransport {
        fn with_status(status: u16) -> Self {
            Self {
                status,
                posts: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn posts(&self) -> Vec<(String, Vec<u8>)> {
            self.posts.lock().expect("posts lock").clone()
        }
    }

    impl AckTransport for FakeAckTransport {
        fn post_ack<'a>(
            &'a self,
            target: &'a str,
            body: &'a [u8],
        ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, MessageError>> + Send + 'a>> {
            self.posts
                .lock()
                .expect("posts lock")
                .push((target.to_string(), body.to_vec()));
            let resp = HttpResponse {
                status: self.status,
                headers: Vec::new(),
                body: Vec::new(),
            };
            Box::pin(async move { Ok(resp) })
        }
    }

    struct FakeReactionClient {
        recorded: Mutex<Vec<(String, String, String, bool)>>,
        outcome: Result<(), ReactionError>,
    }

    impl FakeReactionClient {
        fn ok() -> Self {
            Self {
                recorded: Mutex::new(Vec::new()),
                outcome: Ok(()),
            }
        }
    }

    impl ReactionApi for FakeReactionClient {
        fn toggle_reaction<'a>(
            &'a self,
            channel_id: &'a str,
            message_id: &'a str,
            emoji: &'a str,
            active: bool,
        ) -> Pin<Box<dyn Future<Output = Result<(), ReactionError>> + Send + 'a>> {
            self.recorded.lock().expect("recorded lock").push((
                channel_id.to_string(),
                message_id.to_string(),
                emoji.to_string(),
                active,
            ));
            let outcome = self.outcome.clone();
            Box::pin(async move { outcome })
        }
    }

    struct FakeTypingClient {
        recorded: Mutex<Vec<String>>,
        outcome: Result<Option<TypingCooldowns>, TypingError>,
    }

    impl FakeTypingClient {
        fn ok(cooldowns: Option<TypingCooldowns>) -> Self {
            Self {
                recorded: Mutex::new(Vec::new()),
                outcome: Ok(cooldowns),
            }
        }
    }

    impl TypingApi for FakeTypingClient {
        fn send_typing<'a>(
            &'a self,
            channel_id: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<Option<TypingCooldowns>, TypingError>> + Send + 'a>>
        {
            self.recorded
                .lock()
                .expect("recorded lock")
                .push(channel_id.to_string());
            let outcome = self.outcome.clone();
            Box::pin(async move { outcome })
        }
    }

    #[test]
    fn chat_dtos_serialize_camel_case_and_deserialize_both_camel_and_snake_case() {
        let query: ChatMessagesPageQueryDto = serde_json::from_value(serde_json::json!({
            "channel_id": "1173958353802776576",
            "before": "1200000000000000000",
            "limit": 50
        }))
        .expect("deserialize ChatMessagesPageQueryDto from snake_case");
        assert_eq!(query.channel_id, "1173958353802776576");
        assert_eq!(query.before.as_deref(), Some("1200000000000000000"));
        assert_eq!(query.limit, Some(50));
        let query_json = serde_json::to_value(&query).expect("serialize ChatMessagesPageQueryDto");
        assert_eq!(
            query_json,
            serde_json::json!({
                "channelId": "1173958353802776576",
                "before": "1200000000000000000",
                "limit": 50
            })
        );

        let send_dto: SendChatMessageDto = serde_json::from_value(serde_json::json!({
            "channel_id": "1173958353802776576",
            "content": "Hello modular chat plugin!",
            "nonce": "9988776655"
        }))
        .expect("deserialize SendChatMessageDto");
        assert_eq!(send_dto.channel_id, "1173958353802776576");
        assert!(send_dto.attachments.is_empty());
        let send_json = serde_json::to_value(&send_dto).expect("serialize SendChatMessageDto");
        assert_eq!(
            send_json,
            serde_json::json!({
                "channelId": "1173958353802776576",
                "content": "Hello modular chat plugin!",
                "nonce": "9988776655"
            })
        );

        let edit_dto: EditChatMessageDto = serde_json::from_value(serde_json::json!({
            "channelId": "10",
            "message_id": "20",
            "content": "Edited text"
        }))
        .expect("deserialize EditChatMessageDto");
        assert_eq!(
            serde_json::to_value(&edit_dto).expect("serialize EditChatMessageDto"),
            serde_json::json!({
                "channelId": "10",
                "messageId": "20",
                "content": "Edited text"
            })
        );

        let delete_dto: DeleteChatMessageDto = serde_json::from_value(serde_json::json!({
            "channel_id": "10",
            "messageId": "20"
        }))
        .expect("deserialize DeleteChatMessageDto");
        assert_eq!(
            serde_json::to_value(&delete_dto).expect("serialize DeleteChatMessageDto"),
            serde_json::json!({
                "channelId": "10",
                "messageId": "20"
            })
        );

        let reply_dto: ReplyChatMessageDto = serde_json::from_value(serde_json::json!({
            "channel_id": "10",
            "content": "Replying!",
            "reply_to": "20"
        }))
        .expect("deserialize ReplyChatMessageDto");
        assert_eq!(
            serde_json::to_value(&reply_dto).expect("serialize ReplyChatMessageDto"),
            serde_json::json!({
                "channelId": "10",
                "content": "Replying!",
                "replyTo": "20"
            })
        );

        let ack_dto: AckChatMessageDto = serde_json::from_value(serde_json::json!({
            "channel_id": "10",
            "message_id": "20",
            "last_viewed": 4310,
            "token": null,
            "mention_count": 2
        }))
        .expect("deserialize AckChatMessageDto");
        assert_eq!(
            ack_dto.to_ack_options(),
            AckMessageOptions {
                token: None,
                last_viewed: Some(4310),
                flags: None,
                manual: None,
                mention_count: Some(2),
            }
        );
        assert_eq!(
            serde_json::to_value(&ack_dto).expect("serialize AckChatMessageDto"),
            serde_json::json!({
                "channelId": "10",
                "messageId": "20",
                "lastViewed": 4310,
                "token": null,
                "mentionCount": 2
            })
        );

        let bulk_dto: BulkAckChatMessagesDto = serde_json::from_value(serde_json::json!({
            "read_states": [
                {"channel_id": "10", "message_id": "20"},
                {"channel_id": "30", "message_id": "40", "read_state_type": 2}
            ]
        }))
        .expect("deserialize BulkAckChatMessagesDto");
        assert_eq!(bulk_dto.read_states.len(), 2);
        assert_eq!(bulk_dto.read_states[1].read_state_type, Some(2));

        let reaction_dto: ToggleReactionDto = serde_json::from_value(serde_json::json!({
            "channel_id": "10",
            "message_id": "20",
            "emoji": "👍"
        }))
        .expect("deserialize ToggleReactionDto with default active=true");
        assert!(reaction_dto.active);
        assert_eq!(
            serde_json::to_value(&reaction_dto).expect("serialize ToggleReactionDto"),
            serde_json::json!({
                "channelId": "10",
                "messageId": "20",
                "emoji": "👍",
                "active": true
            })
        );

        let typing_dto: SendTypingDto = serde_json::from_value(serde_json::json!({
            "channel_id": "1173958353802776576"
        }))
        .expect("deserialize SendTypingDto");
        assert_eq!(
            serde_json::to_value(&typing_dto).expect("serialize SendTypingDto"),
            serde_json::json!({
                "channelId": "1173958353802776576"
            })
        );
    }

    #[test]
    fn message_send_edit_delete_and_reply_delegate_and_validate() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime");

        let expected = MessageReceipt {
            id: "9001".to_string(),
            channel_id: "1173958353802776576".to_string(),
            content: "Hello".to_string(),
            nonce: "n-1".to_string(),
        };
        let client = FakeMessageClient::ok(expected.clone());

        let receipt = runtime
            .block_on(send_message_with_api(
                &client,
                "1173958353802776576",
                "Hello",
                "n-1",
            ))
            .expect("send_message_with_api");
        assert_eq!(receipt, expected);

        runtime
            .block_on(edit_message_with_api(
                &client,
                "1173958353802776576",
                "9001",
                "Updated",
            ))
            .expect("edit_message_with_api");
        runtime
            .block_on(delete_message_with_api(
                &client,
                "1173958353802776576",
                "9001",
            ))
            .expect("delete_message_with_api");
        runtime
            .block_on(reply_message_with_api(
                &client,
                "1173958353802776576",
                "Reply text",
                "9001",
            ))
            .expect("reply_message_with_api");

        assert_eq!(
            client.calls(),
            vec![
                "SEND 1173958353802776576 n-1 Hello".to_string(),
                "EDIT 1173958353802776576/9001 Updated".to_string(),
                "DELETE 1173958353802776576/9001".to_string(),
                "REPLY 1173958353802776576->9001 Reply text".to_string(),
            ]
        );

        assert_eq!(
            runtime.block_on(send_message_with_api(&client, "../invalid", "Hello", "n-2")),
            Err(MessageError::InvalidArg)
        );
        assert_eq!(client.calls().len(), 4);
    }

    #[test]
    fn ack_reactions_and_typing_commands_delegate_accurately() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime");

        let ack_transport = FakeAckTransport::with_status(204);
        let read_state = DiscordReadState::with_transport(Arc::new(ack_transport.clone()));

        runtime
            .block_on(ack_message_with_api(
                &read_state,
                "1173958353802776576",
                "1200000000000000001",
                4310,
                None,
            ))
            .expect("ack_message_with_api");
        runtime
            .block_on(ack_messages_bulk_with_api(
                &read_state,
                &[AckBulkEntry {
                    channel_id: "1173958353802776576".to_string(),
                    message_id: "1200000000000000001".to_string(),
                    read_state_type: None,
                }],
            ))
            .expect("ack_messages_bulk_with_api");

        let posts = ack_transport.posts();
        assert_eq!(posts.len(), 2);
        assert_eq!(
            posts[0].0,
            "/channels/1173958353802776576/messages/1200000000000000001/ack"
        );
        assert_eq!(posts[1].0, "/read-states/ack-bulk");

        let reaction_api = FakeReactionClient::ok();
        runtime
            .block_on(add_reaction_with_api(
                &reaction_api,
                "1173958353802776576",
                "1200000000000000001",
                "🔥",
            ))
            .expect("add_reaction_with_api");
        runtime
            .block_on(toggle_reaction_with_api(
                &reaction_api,
                "1173958353802776576",
                "1200000000000000001",
                "🔥",
                false,
            ))
            .expect("toggle_reaction_with_api");
        assert_eq!(
            *reaction_api.recorded.lock().expect("recorded lock"),
            vec![
                (
                    "1173958353802776576".to_string(),
                    "1200000000000000001".to_string(),
                    "🔥".to_string(),
                    true
                ),
                (
                    "1173958353802776576".to_string(),
                    "1200000000000000001".to_string(),
                    "🔥".to_string(),
                    false
                ),
            ]
        );

        let expected_cooldown = TypingCooldowns {
            message_send_cooldown_ms: Some(5000),
            thread_create_cooldown_ms: None,
        };
        let typing_api = FakeTypingClient::ok(Some(expected_cooldown.clone()));
        let actual_cooldown = runtime
            .block_on(send_typing_with_api(&typing_api, "1173958353802776576"))
            .expect("send_typing_with_api");
        assert_eq!(actual_cooldown, Some(expected_cooldown));
    }

    #[test]
    fn token_or_chat_maps_valid_missing_and_storage_errors() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime");

        assert_eq!(
            runtime.block_on(token_or_chat(
                || Ok(Some("valid-chat-token".to_string())),
                || MessageError::Storage,
                || MessageError::Unauthorized,
            )),
            Ok("valid-chat-token".to_string())
        );
        assert_eq!(
            runtime.block_on(token_or_chat(
                || Ok(None),
                || MessageError::Storage,
                || MessageError::Unauthorized,
            )),
            Err(MessageError::Unauthorized)
        );
        assert_eq!(
            runtime.block_on(token_or_chat(
                || Ok(Some("   ".to_string())),
                || MessageError::Storage,
                || MessageError::Unauthorized,
            )),
            Err(MessageError::Unauthorized)
        );
        assert_eq!(
            runtime.block_on(token_or_chat(
                || {
                    Err(AuthError::Keyring {
                        message: "Secret Service unavailable".to_string(),
                    })
                },
                || MessageError::Storage,
                || MessageError::Unauthorized,
            )),
            Err(MessageError::Storage)
        );
    }
}
