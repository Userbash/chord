//! P2-07b: применение `MESSAGE_ACK` к `State.read_state` (RW-17 / W17b).
//!
//! Контракт заморожен `waves/RW-17/DESIGN-P2-07.md` §4. Reducer только памяти: запись
//! ищется по каналу, при отсутствии добавляется в конец; non-channel и stale ACK не меняют
//! данные, но оставляют диагностику. Effects не создаются.

use crate::ready::ReadStateEntry;

use super::read_state::MessageAck;
use super::types::State;

/// Применить подтверждение прочтения к read-state.
///
/// Новые записи ACK добавляются в конец, порядок существующих не меняется. `last_message_id`
/// обновляется только вперёд (равенство — норма), `version` — глобальный watermark без
/// max-guard. `partial`, бейджи и не-channel поля не трогаются.
pub(crate) fn apply_ack(state: &mut State, ack: MessageAck) {
    let Some(index) = state
        .read_state
        .entries
        .iter()
        .position(|entry| entry.id == ack.channel_id)
    else {
        // Ленивое создание: неизвестный канал получает запись в конце, остальные записи
        // сохраняют порядок провода.
        let mut entry = ReadStateEntry {
            id: ack.channel_id,
            ..ReadStateEntry::default()
        };
        apply_fields(&mut entry, &ack);
        state.read_state.entries.push(entry);
        if let Some(version) = ack.version {
            state.read_state.version = version;
        }
        return;
    };

    // Non-channel записи READY (2 — user, 4 — guild) не являются каналами: ACK для них
    // не применяется, чтобы не перепутать ack-точку канала с `last_acked_id`.
    if let Some(read_state_type) = state.read_state.entries[index].read_state_type
        && read_state_type != 0
    {
        state.diagnostics.inconsistencies.push(format!(
            "read_state.ack:{}:non-channel:{read_state_type}",
            ack.channel_id
        ));
        return;
    }

    // Локальная защита от регрессии ack-точки; это не заявленная семантика протокола.
    if let Some(current) = state.read_state.entries[index].last_message_id
        && ack.message_id < current
    {
        state.diagnostics.inconsistencies.push(format!(
            "read_state.ack:{}:stale:{}",
            ack.channel_id, ack.message_id
        ));
        return;
    }

    apply_fields(&mut state.read_state.entries[index], &ack);
    if let Some(version) = ack.version {
        state.read_state.version = version;
    }
}

/// Обновить поля записи, которые меняет ACK; остальные поля не трогаются.
fn apply_fields(entry: &mut ReadStateEntry, ack: &MessageAck) {
    if entry
        .last_message_id
        .is_none_or(|current| ack.message_id > current)
    {
        entry.last_message_id = Some(ack.message_id);
    }
    if let Some(flags) = ack.flags {
        entry.flags = Some(flags);
    }
    if let Some(last_viewed) = ack.last_viewed {
        entry.last_viewed = Some(last_viewed);
    }
    // Non-manual ACK официального клиента очищает mentions; `mention_count` имеет смысл
    // только при manual, отрицательное значение семантически равно 0.
    entry.mention_count = Some(if ack.manual {
        ack.mention_count.unwrap_or(0).max(0)
    } else {
        0
    });
}
