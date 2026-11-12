//! P2-10: чистый reducer voice states (RW-19 / W19).
//!
//! Контракт заморожен `waves/RW-19/DESIGN-P2-10.md` §5. Store — memory-only:
//! `channel_id: None` удаляет запись, `Some` вставляет/заменяет (last wins).
//! `Guild.members` не трогается; `State.users` обновляется только когда
//! `member.user.id` совпал с `user_id` самого voice-события.

use crate::state::types::State;

use super::types::{VoiceChannelStartTime, VoiceChannelStatus, VoiceState, VoiceStateUpdate};

/// Применить live `VOICE_STATE_UPDATE`.
pub fn apply_update(state: &mut State, update: VoiceStateUpdate) {
    let VoiceStateUpdate {
        state: voice,
        member_user,
    } = update;
    if let Some(user) = member_user
        && user.id == voice.user_id
    {
        crate::state::reducer::upsert_user(state, user);
    }
    apply_voice_state(state, voice);
}

/// Применить снимок `READY_SUPPLEMENTAL`; карта не очищается целиком.
pub fn apply_snapshot(state: &mut State, voice_states: Vec<VoiceState>) {
    for voice in voice_states {
        apply_voice_state(state, voice);
    }
}

/// Применить `VOICE_CHANNEL_START_TIME_UPDATE`.
///
/// `None` и отрицательное значение удаляют запись: парсер не пропускает отрицательные,
/// но reducer не должен паниковать, если такой вызов пришёл напрямую.
pub fn apply_start_time(state: &mut State, update: VoiceChannelStartTime) {
    let key = (update.guild_id, update.channel_id);
    match update.voice_start_time {
        Some(value) if value >= 0 => {
            state.voice_channel_start_times.insert(key, value);
        }
        Some(_) | None => {
            state.voice_channel_start_times.remove(&key);
        }
    }
}

/// Применить `VOICE_CHANNEL_STATUS_UPDATE`; `None`/пустая строка удаляют запись.
pub fn apply_status(state: &mut State, update: VoiceChannelStatus) {
    let key = (update.guild_id, update.channel_id);
    match update.status {
        Some(status) if !status.is_empty() => {
            state.voice_channel_statuses.insert(key, status);
        }
        Some(_) | None => {
            state.voice_channel_statuses.remove(&key);
        }
    }
}

fn apply_voice_state(state: &mut State, voice: VoiceState) {
    let key = (voice.guild_id, voice.user_id);
    if voice.channel_id.is_some() {
        state.voice_states.insert(key, voice);
    } else {
        state.voice_states.remove(&key);
    }
}
