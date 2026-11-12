//! P2-02: модель состояния и чистый reducer (RW-13).
//!
//! `(State, Event) -> (State, Effects)` без I/O, SQLite, UI и сети. Модель собрана из
//! принятого READY P2-01; эффекты только декларируются. Контракт заморожен
//! `waves/RW-13/DESIGN-P2-02.md`; тесты — `tests/state_types.rs`, `state_reducer.rs`,
//! `state_selectors.rs`.
//!
//! P2-07: read-state `MESSAGE_ACK` и селекторы mentions/бейджей — `read_state.rs`,
//! `read_state_reducer.rs`, `read_state_selectors.rs`; тесты `read_state_*`.
//!
//! `State::Debug` редактирует `session_id` и `user_settings_proto`.

pub mod member_chunk;
mod member_chunk_selectors;
pub mod member_list;
mod member_list_selectors;
mod presence;
mod presence_reducer;
mod presence_selectors;
mod read_state;
mod read_state_reducer;
mod read_state_selectors;
pub mod reducer;
pub mod selectors;
pub mod types;
pub mod typing;
pub mod voice;

pub use member_chunk::{MemberChunk, MemberChunkAssembly, MemberChunkMember};
pub use member_list::{
    MemberList, MemberListEntry, MemberListGroup, MemberListId, MemberListOp, MemberListRange,
    MemberListUpdate, MemberPresence,
};
pub use presence::{
    Activity, ActivityAssets, ActivityEmoji, ActivityParty, ActivityTimestamps, ActivityType,
    MergedPresences, Presence, PresenceKey, PresenceScope, PresenceStatus, PresenceUpdate,
};
pub use read_state::MessageAck;
pub use reducer::{apply, apply_dispatch, apply_frame, apply_frame_at};
pub use types::{
    ChannelInfo, Effect, Effects, Event, EventError, EventPayload, MemberRange, Message,
    MessagePatch, ReadySupplemental, State, StateDiagnostics, Thread, ThreadListSync,
};
pub use typing::{TYPING_TTL_MS, TypingEntry, TypingKey, TypingStart};
pub use voice::{
    VoiceChannelKey, VoiceChannelStartTime, VoiceChannelStatus, VoiceKey, VoiceState,
    VoiceStateUpdate, apply_snapshot, apply_start_time, apply_status, apply_update,
    parse_voice_channel_start_time, parse_voice_channel_status, parse_voice_state_update,
    parse_voice_states,
};
