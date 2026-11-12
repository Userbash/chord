//! P2-10: memory-only voice states (RW-19 / W19).
//!
//! Контракт заморожен `waves/RW-19/DESIGN-P2-10.md` §3–§6. Модуль не выполняет I/O:
//! публичные типы, разбор уже декодированных ETF, чистый reducer и селекторы от
//! `now_ms`. `session_id` не сохраняется, `Guild.members` не меняется, voice
//! transport/DM-звонки — вне карточки.

pub mod parser;
pub mod reducer;
pub mod selectors;
pub mod types;

pub use parser::{
    parse_voice_channel_start_time, parse_voice_channel_status, parse_voice_state_update,
    parse_voice_states,
};
pub use reducer::{apply_snapshot, apply_start_time, apply_status, apply_update};
pub use types::{
    VoiceChannelKey, VoiceChannelStartTime, VoiceChannelStatus, VoiceKey, VoiceState,
    VoiceStateUpdate,
};
