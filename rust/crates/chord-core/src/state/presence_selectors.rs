//! P2-09c: селекторы presence (RW-18 / W18c).
//!
//! Контракт заморожен `waves/RW-18/DESIGN-P2-09.md` §6. Только чтение, без паник.
//! Агрегат выбирается среди видимых scopes по `(processed_at_timestamp, activities.len())`;
//! при полном равенстве — по `PresenceKey`. Guild-селекторы возвращают точный scope.

use std::collections::{BTreeMap, BTreeSet};

use super::presence::{Activity, ActivityType, Presence, PresenceScope, PresenceStatus};
use super::types::State;

impl State {
    /// Выбранный видимый presence пользователя.
    ///
    /// Если есть живой online/idle/dnd scope, offline+hidden не понижает агрегат
    /// (oracle PresenceStore); иначе выбирается лучший видимый scope, включая
    /// offline+hidden.
    #[must_use]
    pub fn presence(&self, user_id: u64) -> Option<&Presence> {
        self.select_presence(user_id, true)
            .or_else(|| self.select_presence(user_id, false))
    }

    fn select_presence(&self, user_id: u64, online_only: bool) -> Option<&Presence> {
        self.presences
            .iter()
            .filter(|((user, _), presence)| {
                *user == user_id
                    && presence.is_visible()
                    && (!online_only || presence.status.is_online())
            })
            .max_by(
                |((left_user, left_scope), left), ((right_user, right_scope), right)| {
                    left.processed_at_timestamp
                        .cmp(&right.processed_at_timestamp)
                        .then_with(|| left.activities.len().cmp(&right.activities.len()))
                        .then_with(|| (*left_user, *left_scope).cmp(&(*right_user, *right_scope)))
                },
            )
            .map(|(_, presence)| presence)
    }

    /// Presence в конкретной гильдии.
    #[must_use]
    pub fn presence_in_guild(&self, user_id: u64, guild_id: u64) -> Option<&Presence> {
        self.presence_scope(user_id, PresenceScope::Guild(guild_id))
    }

    /// Presence в конкретном scope.
    #[must_use]
    pub fn presence_scope(&self, user_id: u64, scope: PresenceScope) -> Option<&Presence> {
        self.presences.get(&(user_id, scope))
    }

    /// Все scopes пользователя (включая offline, если запись ещё хранится).
    pub fn presence_scopes(&self, user_id: u64) -> impl Iterator<Item = &Presence> + '_ {
        self.presences
            .iter()
            .filter(move |((user, _), _)| *user == user_id)
            .map(|(_, presence)| presence)
    }

    /// Число scope-записей пользователя.
    #[must_use]
    pub fn presence_scope_count(&self, user_id: u64) -> usize {
        self.presences
            .keys()
            .filter(|(user, _)| *user == user_id)
            .count()
    }

    /// Число уникальных пользователей в presence-store.
    #[must_use]
    pub fn presence_user_count(&self) -> usize {
        self.presences
            .keys()
            .map(|(user, _)| *user)
            .collect::<BTreeSet<_>>()
            .len()
    }

    /// Уникальные user id по возрастанию.
    #[must_use]
    pub fn presence_user_ids(&self) -> Vec<u64> {
        self.presences
            .keys()
            .map(|(user, _)| *user)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Есть ли запись пользователя.
    #[must_use]
    pub fn has_presence(&self, user_id: u64) -> bool {
        self.presences.keys().any(|(user, _)| *user == user_id)
    }

    /// Агрегированный статус; отсутствие — `offline`.
    #[must_use]
    pub fn status(&self, user_id: u64) -> PresenceStatus {
        self.presence(user_id)
            .map_or(PresenceStatus::Offline, |presence| presence.status.clone())
    }

    /// Статус в конкретной гильдии; отсутствие — `offline`.
    #[must_use]
    pub fn status_in_guild(&self, user_id: u64, guild_id: u64) -> PresenceStatus {
        self.presence_in_guild(user_id, guild_id)
            .map_or(PresenceStatus::Offline, |presence| presence.status.clone())
    }

    /// Клиенты выбранного presence.
    #[must_use]
    pub fn client_status(&self, user_id: u64) -> Option<&BTreeMap<String, PresenceStatus>> {
        self.presence(user_id)
            .map(|presence| &presence.client_status)
    }

    /// Клиенты presence в конкретной гильдии.
    #[must_use]
    pub fn client_status_in_guild(
        &self,
        user_id: u64,
        guild_id: u64,
    ) -> Option<&BTreeMap<String, PresenceStatus>> {
        self.presence_in_guild(user_id, guild_id)
            .map(|presence| &presence.client_status)
    }

    /// Activities выбранного presence.
    #[must_use]
    pub fn activities(&self, user_id: u64) -> &[Activity] {
        self.presence(user_id)
            .map_or(&[], |presence| presence.activities.as_slice())
    }

    /// Activities presence в конкретной гильдии.
    #[must_use]
    pub fn activities_in_guild(&self, user_id: u64, guild_id: u64) -> &[Activity] {
        self.presence_in_guild(user_id, guild_id)
            .map_or(&[], |presence| presence.activities.as_slice())
    }

    /// Скрытые activities выбранного presence.
    #[must_use]
    pub fn hidden_activities(&self, user_id: u64) -> &[Activity] {
        self.presence(user_id)
            .map_or(&[], |presence| presence.hidden_activities.as_slice())
    }

    /// Первая не-custom activity.
    #[must_use]
    pub fn primary_activity(&self, user_id: u64) -> Option<&Activity> {
        self.activities(user_id)
            .iter()
            .find(|activity| !activity.activity_type.is_custom())
    }

    /// Custom status.
    #[must_use]
    pub fn custom_status(&self, user_id: u64) -> Option<&Activity> {
        self.activities(user_id)
            .iter()
            .find(|activity| activity.activity_type.is_custom())
    }

    /// Activity заданного типа.
    #[must_use]
    pub fn activity_by_type(&self, user_id: u64, activity_type: ActivityType) -> Option<&Activity> {
        self.activities(user_id)
            .iter()
            .find(|activity| activity.activity_type == activity_type)
    }

    /// Есть ли выбранный online/idle/dnd статус.
    #[must_use]
    pub fn is_online(&self, user_id: u64) -> bool {
        self.presence(user_id)
            .is_some_and(|presence| presence.status.is_online())
    }

    /// Отсутствие записи или статус `Offline`; `Invisible`/`Unknown` — отдельные статусы.
    #[must_use]
    pub fn is_offline(&self, user_id: u64) -> bool {
        self.presence(user_id)
            .is_none_or(|presence| presence.status.is_offline())
    }

    /// Mobile online: mobile online, desktop/vr не online.
    #[must_use]
    pub fn is_mobile_online(&self, user_id: u64) -> bool {
        self.presence(user_id).is_some_and(|presence| {
            client_is_online(presence, "mobile")
                && !client_is_online(presence, "desktop")
                && !client_is_online(presence, "vr")
        })
    }

    /// VR online.
    #[must_use]
    pub fn is_vr_online(&self, user_id: u64) -> bool {
        self.presence(user_id)
            .is_some_and(|presence| client_is_online(presence, "vr"))
    }

    /// Пользователи с выбранным online/idle/dnd статусом, по возрастанию id.
    #[must_use]
    pub fn online_user_ids(&self) -> Vec<u64> {
        self.presence_user_ids()
            .into_iter()
            .filter(|user_id| self.is_online(*user_id))
            .collect()
    }

    /// Число online-записей в гильдии (idle/dnd считаются online).
    #[must_use]
    pub fn guild_online_count(&self, guild_id: u64) -> usize {
        self.presences
            .iter()
            .filter(|((_, scope), presence)| {
                *scope == PresenceScope::Guild(guild_id) && presence.status.is_online()
            })
            .count()
    }
}

fn client_is_online(presence: &Presence, client: &str) -> bool {
    presence
        .client_status
        .get(client)
        .is_some_and(|status| matches!(status, PresenceStatus::Online))
}
