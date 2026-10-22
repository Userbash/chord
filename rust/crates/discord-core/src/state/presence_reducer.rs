//! P2-09b: reducer presence (RW-18 / W18b).
//!
//! Контракт заморожен `waves/RW-18/DESIGN-P2-09.md` §5. Presence хранится по
//! `(user_id, scope)`; агрегат выбирается селекторами. Offline без hidden activities
//! удаляет пользователя, когда все его scopes offline. Никакого I/O.

use std::collections::BTreeSet;

use super::member_list::MemberPresence;
use super::presence::{
    MergedPresences, Presence, PresenceScope, PresenceStatus, normalize_activities,
};
use super::types::State;

/// Применить live `PRESENCE_UPDATE`.
pub(crate) fn apply_update(state: &mut State, update: super::presence::PresenceUpdate) {
    if let Some(user) = update.user {
        super::reducer::upsert_user(state, user);
    }
    upsert_presence(state, update.presence);
}

/// Применить начальный снимок `READY_SUPPLEMENTAL`.
pub(crate) fn apply_supplemental(
    state: &mut State,
    guild_ids: &[Option<u64>],
    merged: MergedPresences,
) {
    let mut missing = BTreeSet::new();

    for presence in merged.friends {
        note_missing_user(state, &presence, &mut missing);
        upsert_presence(state, presence);
    }

    for (index, group) in merged.guilds.into_iter().enumerate() {
        let Some(guild_id) = guild_ids.get(index).copied().flatten() else {
            if !group.is_empty() {
                state
                    .diagnostics
                    .inconsistencies
                    .push(format!("merged_presences.guilds[{index}]"));
            }
            continue;
        };
        for mut presence in group {
            presence.guild_id = Some(guild_id);
            note_missing_user(state, &presence, &mut missing);
            upsert_presence(state, presence);
        }
    }

    for user_id in missing {
        state
            .diagnostics
            .inconsistencies
            .push(format!("presences:user:{user_id}:missing"));
    }
}

/// Применить presence из member-list; `State.users` не трогается (P2-04 frozen).
pub(crate) fn apply_member_presence(
    state: &mut State,
    guild_id: u64,
    user_id: u64,
    member: &MemberPresence,
) {
    let Some(status) = member.status.as_deref().filter(|status| !status.is_empty()) else {
        return;
    };
    let activities = normalize_activities(member.activities.clone());
    let hidden_activities = member.hidden_activities.clone();
    let presence = Presence {
        user_id,
        guild_id: Some(guild_id),
        status: PresenceStatus::from_wire(status),
        client_status: member
            .client_status
            .iter()
            .map(|(client, status)| (client.clone(), PresenceStatus::from_wire(status)))
            .collect(),
        activities,
        hidden_activities,
        processed_at_timestamp: member.processed_at_timestamp,
        since: None,
        broadcast: None,
    };
    upsert_presence(state, presence);
}

fn note_missing_user(state: &State, presence: &Presence, missing: &mut BTreeSet<u64>) {
    if presence.user_id != state.user.id && !state.users.contains_key(&presence.user_id) {
        missing.insert(presence.user_id);
    }
}

fn upsert_presence(state: &mut State, mut presence: Presence) {
    if presence.status.is_offline() && presence.hidden_activities.is_empty() {
        presence.activities.clear();
    }

    let user_id = presence.user_id;
    let scope = presence.scope();

    // Ключ store — `(user_id, scope)`, `PresenceScope::Guild(..) < PresenceScope::Private`
    // по `Ord`, поэтому диапазон `[(user_id, Guild(0)) ..= (user_id, Private)]` покрывает
    // ровно записи этого пользователя. Диапазон вместо полного скана: P7-02/I1 — на
    // 24 229 участниках прежний вариант давал O(n²) (см. тест `r19` в
    // `tests/presence_reducer.rs`).
    let first = (user_id, PresenceScope::Guild(0));
    let last = (user_id, PresenceScope::Private);

    let has_existing = state.presences.range(first..=last).next().is_some();
    if !presence.is_visible() && !has_existing {
        // Официальный клиент игнорирует offline для неизвестного пользователя.
        return;
    }

    state.presences.insert((user_id, scope), presence);

    let mut total = 0usize;
    let mut invisible = Vec::new();
    for (key, presence) in state.presences.range(first..=last) {
        total += 1;
        if !presence.is_visible() {
            invisible.push(*key);
        }
    }
    if total > 0 && invisible.len() == total {
        for key in invisible {
            state.presences.remove(&key);
        }
    }
}
