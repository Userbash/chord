//! P2-02b: чистый reducer `(State, Event) -> (State, Effects)` (RW-13 / W13b).
//!
//! Контракт заморожен `waves/RW-13/DESIGN-P2-02.md` §4–§6. Никакого I/O: эффекты только
//! декларируются, `State` и `Effects` возвращаются по владению. Ошибки разбора и неизвестные
//! события не меняют данные, но оставляют диагностику; `READY` сбрасывает последовательность.

use crate::dispatch::{DispatchEvent, DispatchKind, route_dispatch};
use crate::ready::{Guild, User};

use super::types::{
    ChannelInfo, Effect, Effects, Event, EventPayload, MemberRange, Message, MessagePatch,
    ReadySupplemental, State, ThreadListSync,
};

/// Применить типизированное событие.
///
/// `READY` всегда применяется и сбрасывает `last_seq` (новая последовательность). Для
/// остальных событий `seq <= last_seq` — stale: данные не меняются, пишется диагностика.
pub fn apply(mut state: State, event: Event) -> (State, Effects) {
    let is_ready = matches!(&event.payload, EventPayload::Ready(_));
    if !is_ready
        && let (Some(sequence), Some(last)) = (event.seq, state.last_seq)
        && sequence <= last
    {
        state.diagnostics.stale_sequences.push(sequence);
        return (state, Effects::new());
    }

    state.diagnostics.merge(&event.diagnostics);
    let sequence = event.seq;
    let mut effects = Effects::new();

    match event.payload {
        EventPayload::Ready(ready) => {
            let mut next = State::from_ready(&ready);
            next.diagnostics.merge(&state.diagnostics);
            next.last_seq = sequence;
            effects = subscribe_effects(&next);
            state = next;
        }
        EventPayload::ReadySupplemental(supplemental) => {
            merge_supplemental(&mut state, supplemental);
        }
        EventPayload::Resumed => {}
        EventPayload::SessionsReplace(sessions) => {
            state.replace_sessions(sessions);
        }
        EventPayload::ThreadListSync(sync) => {
            apply_thread_list_sync(&mut state, sync);
        }
        EventPayload::ChannelInfo(info) => {
            apply_channel_info(&mut state, info);
        }
        EventPayload::MessageCreate(message) => {
            let channel_id = message.channel_id;
            let author_id = message.author.as_ref().map(|author| author.id);
            insert_message(&mut state, message);
            if let Some(author_id) = author_id {
                let _ =
                    super::typing::reducer::remove_for_message(&mut state, channel_id, author_id);
            }
        }
        EventPayload::MessageUpdate(patch) => {
            apply_message_patch(&mut state, patch);
        }
        EventPayload::GuildMemberListUpdate(update) => {
            super::member_list::apply_update(&mut state, update);
        }
        EventPayload::GuildMembersChunk(chunk) => {
            super::member_chunk::apply_chunk(&mut state, chunk);
        }
        EventPayload::MessageAck(ack) => {
            super::read_state_reducer::apply_ack(&mut state, ack);
        }
        EventPayload::PresenceUpdate(update) => {
            super::presence_reducer::apply_update(&mut state, update);
        }
        EventPayload::VoiceStateUpdate(update) => {
            super::voice::reducer::apply_update(&mut state, update);
        }
        EventPayload::VoiceChannelStartTime(update) => {
            super::voice::reducer::apply_start_time(&mut state, update);
        }
        EventPayload::VoiceChannelStatus(update) => {
            super::voice::reducer::apply_status(&mut state, update);
        }
        EventPayload::TypingStart(start) => {
            super::typing::reducer::apply_start(&mut state, start);
        }
        EventPayload::MessageDelete(ev) => {
            state.messages.remove(&ev.id);
        }
        EventPayload::MessageDeleteBulk(ev) => {
            for id in &ev.ids {
                state.messages.remove(id);
            }
        }
        EventPayload::ChannelCreate(ev) | EventPayload::ChannelUpdate(ev) => {
            apply_channel_upsert(&mut state, ev);
        }
        EventPayload::ChannelDelete(ev) => {
            apply_channel_delete(&mut state, &ev);
        }
        EventPayload::ThreadCreate(ev) | EventPayload::ThreadUpdate(ev) => {
            apply_thread_upsert(&mut state, ev);
        }
        EventPayload::ThreadDelete(ev) => {
            apply_thread_delete(&mut state, &ev);
        }
        EventPayload::Unsupported(kind) => {
            state
                .diagnostics
                .unsupported_events
                .push(kind.as_str().to_string());
        }
        EventPayload::Unknown { name } => {
            state.diagnostics.unknown_events.push(name);
        }
    }

    if !is_ready {
        state.last_seq = sequence.or(state.last_seq);
    }
    (state, effects)
}

/// Применить уже маршрутизированный dispatch; ошибка разбора — диагностика, не паника.
pub fn apply_dispatch(mut state: State, dispatch: &DispatchEvent) -> (State, Effects) {
    // Stale-политика действует и до разбора: битый старый кадр не должен откатывать
    // последовательность. `READY` — единственное исключение (новая сессия).
    let is_ready = matches!(dispatch.kind, Some(DispatchKind::Ready));
    if !is_ready
        && let (Some(sequence), Some(last)) = (dispatch.seq, state.last_seq)
        && sequence <= last
    {
        state.diagnostics.stale_sequences.push(sequence);
        return (state, Effects::new());
    }

    match Event::from_routed(dispatch) {
        Ok(event) => apply(state, event),
        Err(error) => {
            state.diagnostics.malformed.push(format!("event:{error}"));
            // Монотонность сохраняется и для ошибки разбора: назад `last_seq` не идёт.
            state.last_seq = state.last_seq.max(dispatch.seq);
            (state, Effects::new())
        }
    }
}

/// Применить полный op0-кадр; не-dispatch — диагностика, не паника.
pub fn apply_frame(mut state: State, frame: &discord_codec::etf::EtfValue) -> (State, Effects) {
    let Some(dispatch) = route_dispatch(frame) else {
        state
            .diagnostics
            .malformed
            .push("dispatch:not-op0".to_string());
        return (state, Effects::new());
    };
    apply_dispatch(state, &dispatch)
}

/// Применить полный op0-кадр с детерминированными часами.
pub fn apply_frame_at(
    mut state: State,
    frame: &discord_codec::etf::EtfValue,
    now_ms: u64,
) -> (State, Effects) {
    let Some(dispatch) = route_dispatch(frame) else {
        state
            .diagnostics
            .malformed
            .push("dispatch:not-op0".to_string());
        return (state, Effects::new());
    };
    let is_ready = matches!(dispatch.kind, Some(DispatchKind::Ready));
    if !is_ready
        && let (Some(sequence), Some(last)) = (dispatch.seq, state.last_seq)
        && sequence <= last
    {
        state.diagnostics.stale_sequences.push(sequence);
        return (state, Effects::new());
    }
    match Event::from_frame_at(frame, now_ms) {
        Ok(event) => apply(state, event),
        Err(error) => {
            state.diagnostics.malformed.push(format!("event:{error}"));
            state.last_seq = state.last_seq.max(dispatch.seq);
            (state, Effects::new())
        }
    }
}

fn subscribe_effects(state: &State) -> Effects {
    let mut effects = Effects::new();
    for guild in state.guilds.values() {
        if guild.lazy && guild.member_count.unwrap_or(0) > 0 {
            effects.push(Effect::SubscribeMembers {
                guild_id: guild.id,
                ranges: vec![MemberRange { start: 0, end: 99 }],
            });
        }
    }
    effects
}

fn merge_supplemental(state: &mut State, supplemental: ReadySupplemental) {
    let ReadySupplemental {
        guild_ids,
        merged_members,
        merged_presences,
        voice_states,
    } = supplemental;
    for (index, members) in merged_members.into_iter().enumerate() {
        match guild_ids.get(index).copied().flatten() {
            Some(guild_id) => {
                if let Some(guild) = state.guilds.get_mut(&guild_id) {
                    guild.members = members;
                } else {
                    state
                        .diagnostics
                        .inconsistencies
                        .push(format!("guilds:{guild_id}:supplemental"));
                }
            }
            None if !members.is_empty() => {
                state
                    .diagnostics
                    .inconsistencies
                    .push(format!("merged_members[{index}]"));
            }
            None => {}
        }
    }
    super::presence_reducer::apply_supplemental(state, &guild_ids, merged_presences);
    super::voice::reducer::apply_snapshot(state, voice_states);
}

fn ensure_guild(state: &mut State, guild_id: u64) -> &mut Guild {
    if let std::collections::btree_map::Entry::Vacant(entry) = state.guilds.entry(guild_id) {
        entry.insert(Guild {
            id: guild_id,
            unavailable: true,
            ..Guild::default()
        });
        state
            .diagnostics
            .inconsistencies
            .push(format!("guilds:{guild_id}:missing"));
    }
    state
        .guilds
        .get_mut(&guild_id)
        .expect("гильдия вставлена выше")
}

fn insert_channel(state: &mut State, guild_id: u64, channel: crate::ready::Channel) {
    let channel_id = channel.id;
    if let Some(previous) = state.channel_guild.get(&channel_id).copied()
        && previous != guild_id
    {
        if let Some(old_guild) = state.guilds.get_mut(&previous) {
            old_guild
                .channels
                .retain(|existing| existing.id != channel_id);
        }
        state.diagnostics.inconsistencies.push(format!(
            "channels:{channel_id}:guilds:{previous}->{guild_id}"
        ));
    }

    let guild = ensure_guild(state, guild_id);
    if let Some(existing) = guild
        .channels
        .iter_mut()
        .find(|existing| existing.id == channel_id)
    {
        *existing = channel;
    } else {
        guild.channels.push(channel);
    }
    state.channel_guild.insert(channel_id, guild_id);
}

fn apply_channel_info(state: &mut State, info: ChannelInfo) {
    ensure_guild(state, info.guild_id);
    for channel in info.channels {
        insert_channel(state, info.guild_id, channel);
    }
}

fn apply_thread_list_sync(state: &mut State, sync: ThreadListSync) {
    ensure_guild(state, sync.guild_id);
    for mut thread in sync.threads {
        if let Some(previous) = state.thread_guild.get(&thread.id).copied()
            && previous != sync.guild_id
        {
            state.diagnostics.inconsistencies.push(format!(
                "threads:{}:guilds:{previous}->{}",
                thread.id, sync.guild_id
            ));
        }
        thread.guild_id = sync.guild_id;
        state.thread_guild.insert(thread.id, sync.guild_id);
        state.threads.insert(thread.id, thread);
    }
}

fn merge_user(existing: &mut User, incoming: User) {
    if incoming.username.is_some() {
        existing.username = incoming.username;
    }
    if incoming.global_name.is_some() {
        existing.global_name = incoming.global_name;
    }
    if incoming.display_name.is_some() {
        existing.display_name = incoming.display_name;
    }
    if incoming.discriminator.is_some() {
        existing.discriminator = incoming.discriminator;
    }
    if incoming.avatar.is_some() {
        existing.avatar = incoming.avatar;
    }
    if incoming.public_flags.is_some() {
        existing.public_flags = incoming.public_flags;
    }
    existing.bot |= incoming.bot;
}

pub(crate) fn upsert_user(state: &mut State, user: User) {
    if user.id == state.user.id {
        merge_user(&mut state.user, user.clone());
    }
    if let Some(existing) = state.users.get_mut(&user.id) {
        merge_user(existing, user);
    } else {
        state.users.insert(user.id, user);
    }
}

fn insert_message(state: &mut State, message: Message) {
    if state.messages.contains_key(&message.id) {
        state
            .diagnostics
            .duplicates
            .push(format!("messages:{}", message.id));
    }
    if let Some(author) = message.author.clone() {
        upsert_user(state, author);
    }
    if !state.channel_guild.contains_key(&message.channel_id)
        && !state.thread_guild.contains_key(&message.channel_id)
        && !state.private_channels.contains_key(&message.channel_id)
    {
        state.diagnostics.inconsistencies.push(format!(
            "messages:{}:channel:{}",
            message.id, message.channel_id
        ));
    }
    state.messages.insert(message.id, message);
}

fn apply_patch_fields(message: &mut Message, patch: &MessagePatch) {
    if let Some(author) = patch.author.clone() {
        message.author = Some(author);
    }
    if let Some(content) = patch.content.clone() {
        message.content = content;
    }
    if let Some(edited_timestamp) = patch.edited_timestamp.clone() {
        message.edited_timestamp = Some(edited_timestamp);
    }
    if let Some(message_type) = patch.message_type {
        message.message_type = message_type;
    }
    if let Some(flags) = patch.flags {
        message.flags = flags;
    }
    if let Some(pinned) = patch.pinned {
        message.pinned = pinned;
    }
    if let Some(tts) = patch.tts {
        message.tts = tts;
    }
    if let Some(mention_everyone) = patch.mention_everyone {
        message.mention_everyone = mention_everyone;
    }
    if let Some(ids) = &patch.mention_user_ids {
        message.mention_user_ids = ids.clone();
    }
    if let Some(ids) = &patch.mention_role_ids {
        message.mention_role_ids = ids.clone();
    }
    if let Some(count) = patch.attachment_count {
        message.attachment_count = count;
    }
    if let Some(count) = patch.embed_count {
        message.embed_count = count;
    }
}

fn apply_message_patch(state: &mut State, patch: MessagePatch) {
    if state.messages.contains_key(&patch.id) {
        // Автор обновления относится к уже известному сообщению; для неизвестного без
        // канала патч отклоняется ниже и не должен оставлять «фантомного» пользователя.
        if let Some(author) = patch.author.clone() {
            upsert_user(state, author);
        }
        let message = state
            .messages
            .get_mut(&patch.id)
            .expect("сообщение проверено выше");
        if let Some(channel_id) = patch.channel_id
            && channel_id != message.channel_id
        {
            state.diagnostics.inconsistencies.push(format!(
                "messages:{}:channel-change:{}->{channel_id}",
                patch.id, message.channel_id
            ));
        }
        if let Some(guild_id) = patch.guild_id {
            match message.guild_id {
                Some(existing) if existing != guild_id => {
                    state.diagnostics.inconsistencies.push(format!(
                        "messages:{}:guild-change:{existing}->{guild_id}",
                        patch.id
                    ));
                }
                None => message.guild_id = Some(guild_id),
                Some(_) => {}
            }
        }
        apply_patch_fields(message, &patch);
        return;
    }

    let Some(channel_id) = patch.channel_id else {
        state
            .diagnostics
            .malformed
            .push(format!("message_update:{}:channel_id", patch.id));
        return;
    };
    let mut message = Message {
        id: patch.id,
        channel_id,
        guild_id: patch.guild_id,
        ..Message::default()
    };
    apply_patch_fields(&mut message, &patch);
    state
        .diagnostics
        .inconsistencies
        .push(format!("messages:{}:updated-before-create", patch.id));
    insert_message(state, message);
}

fn apply_channel_upsert(state: &mut State, ev: crate::dispatch::ChannelLifecycleDispatch) {
    let channel_id = ev.id;
    let Some(guild_id) = ev.guild_id else {
        state
            .diagnostics
            .inconsistencies
            .push(format!("channels:{channel_id}:missing_guild"));
        return;
    };

    if !state.guilds.contains_key(&guild_id) {
        state
            .diagnostics
            .inconsistencies
            .push(format!("guilds:{guild_id}:missing"));
        return;
    }

    if let Some(previous) = state.channel_guild.get(&channel_id).copied()
        && previous != guild_id
    {
        if let Some(old_guild) = state.guilds.get_mut(&previous) {
            old_guild
                .channels
                .retain(|existing| existing.id != channel_id);
        }
        state.diagnostics.inconsistencies.push(format!(
            "channels:{channel_id}:guilds:{previous}->{guild_id}"
        ));
    }

    let guild = state
        .guilds
        .get_mut(&guild_id)
        .expect("наличие проверено выше");
    if let Some(existing) = guild.channels.iter_mut().find(|c| c.id == channel_id) {
        if let Some(name) = ev.name {
            existing.name = Some(name);
        }
        if let Some(kind) = ev.kind {
            existing.channel_type = kind;
        }
        if ev.parent_id.is_some() {
            existing.parent_id = ev.parent_id;
        }
    } else {
        guild.channels.push(crate::ready::Channel {
            id: channel_id,
            channel_type: ev.kind.unwrap_or(0),
            name: ev.name,
            parent_id: ev.parent_id,
            ..crate::ready::Channel::default()
        });
    }
    state.channel_guild.insert(channel_id, guild_id);
}

fn apply_channel_delete(state: &mut State, ev: &crate::dispatch::ChannelLifecycleDispatch) {
    let channel_id = ev.id;
    let target_guild = state.channel_guild.remove(&channel_id).or(ev.guild_id);
    if let Some(guild_id) = target_guild {
        if let Some(guild) = state.guilds.get_mut(&guild_id) {
            guild.channels.retain(|c| c.id != channel_id);
        }
    } else {
        for guild in state.guilds.values_mut() {
            guild.channels.retain(|c| c.id != channel_id);
        }
    }
    state.messages.retain(|_, msg| msg.channel_id != channel_id);
    let child_threads: Vec<u64> = state
        .threads
        .iter()
        .filter(|(_, th)| th.parent_id == Some(channel_id))
        .map(|(&id, _)| id)
        .collect();
    for thread_id in child_threads {
        state.threads.remove(&thread_id);
        state.thread_guild.remove(&thread_id);
        state.messages.retain(|_, msg| msg.channel_id != thread_id);
    }
}

fn apply_thread_upsert(state: &mut State, ev: crate::dispatch::ThreadLifecycleDispatch) {
    let thread_id = ev.id;
    let guild_id = ev
        .guild_id
        .or_else(|| state.thread_guild.get(&thread_id).copied());

    if let Some(gid) = ev.guild_id {
        if let Some(previous) = state.thread_guild.get(&thread_id).copied()
            && previous != gid
        {
            state
                .diagnostics
                .inconsistencies
                .push(format!("threads:{thread_id}:guilds:{previous}->{gid}"));
        }
        state.thread_guild.insert(thread_id, gid);
    }

    if let Some(existing) = state.threads.get_mut(&thread_id) {
        if let Some(gid) = guild_id {
            existing.guild_id = gid;
        }
        if let Some(name) = ev.name {
            existing.name = Some(name);
        }
        if let Some(kind) = ev.kind {
            existing.thread_type = kind;
        }
        if ev.parent_id.is_some() {
            existing.parent_id = ev.parent_id;
        }
    } else {
        let thread = super::types::Thread {
            id: thread_id,
            guild_id: guild_id.unwrap_or(0),
            parent_id: ev.parent_id,
            name: ev.name,
            thread_type: ev.kind.unwrap_or(11),
            ..super::types::Thread::default()
        };
        state.threads.insert(thread_id, thread);
    }
}

fn apply_thread_delete(state: &mut State, ev: &crate::dispatch::ThreadLifecycleDispatch) {
    state.threads.remove(&ev.id);
    state.thread_guild.remove(&ev.id);
    state.messages.retain(|_, msg| msg.channel_id != ev.id);
}
