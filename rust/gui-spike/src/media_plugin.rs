//! Modular Tauri v2 plugin (`media`) for attachments, pins, and threads.
//!
//! Provides the `"media"` plugin builder and encapsulates file upload slot requests,
//! attachment PUT payloads, and channel pins/threads retrieval.

use chord_client::attachments::{
    AttachmentError, AttachmentFileSpec, AttachmentUploadSlot, DiscordAttachments,
};
use chord_client::pins_threads::{
    DiscordPinsThreads, PinnedMessage, PinsThreadsError, ThreadSummary,
};
use serde::{Deserialize, Serialize};

/// Payload envelope for `attachments_upload_slots` expected by `composer.js`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadSlotsEnvelope {
    pub attachments: Vec<AttachmentUploadSlot>,
}

/// Input specification for a file upload slot.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct UploadSlotSpecInput {
    pub filename: String,
    #[serde(alias = "fileSize")]
    pub file_size: u64,
    pub id: String,
    #[serde(default, alias = "isClip")]
    pub is_clip: bool,
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

/// Requests attachment upload slots from Discord REST API using authenticated token.
pub async fn request_upload_slots(
    token: &str,
    channel_id: &str,
    specs: &[AttachmentFileSpec],
) -> Result<Vec<AttachmentUploadSlot>, AttachmentError> {
    let api = DiscordAttachments::new(token)?;
    chord_client::request_upload_slots(&api, channel_id, specs).await
}

/// Lists pinned messages for a channel using authenticated token.
pub async fn list_channel_pins(
    token: &str,
    channel_id: &str,
) -> Result<Vec<PinnedMessage>, PinsThreadsError> {
    let api = DiscordPinsThreads::new(token)?;
    chord_client::list_channel_pins(&api, channel_id).await
}

/// Lists threads for a channel (active or archived) using authenticated token.
pub async fn list_channel_threads(
    token: &str,
    channel_id: &str,
    archived: bool,
) -> Result<Vec<ThreadSummary>, PinsThreadsError> {
    let api = DiscordPinsThreads::new(token)?;
    chord_client::list_channel_threads(&api, channel_id, archived).await
}

/// Initializes the modular Tauri v2 `"media"` plugin.
#[must_use]
pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("media").build()
}
