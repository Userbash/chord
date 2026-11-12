//! P2-04b: read-only селекторы ленивого списка участников.

use crate::ready::{GuildMember, User};

use super::member_list::{
    MemberList, MemberListEntry, MemberListGroup, MemberListId, MemberListRange, MemberPresence,
};
use super::types::State;

impl State {
    /// Список по `(guild_id, list_id)`.
    #[must_use]
    pub fn member_list(&self, guild_id: u64, id: &MemberListId) -> Option<&MemberList> {
        self.member_lists.get(&(guild_id, id.clone()))
    }

    /// Плоские items; пустой срез, если списка ещё нет.
    #[must_use]
    pub fn member_list_items<'a>(
        &'a self,
        guild_id: u64,
        id: &MemberListId,
    ) -> &'a [MemberListEntry] {
        self.member_list(guild_id, id)
            .map_or(&[], |list| list.items.as_slice())
    }

    /// Участник по индексу плоского списка; group-элемент → `None`.
    #[must_use]
    pub fn member_list_member_at<'a>(
        &'a self,
        guild_id: u64,
        id: &MemberListId,
        index: usize,
    ) -> Option<(&'a GuildMember, &'a User, &'a MemberPresence)> {
        match self.member_list(guild_id, id)?.items.get(index)? {
            MemberListEntry::Member {
                member,
                user,
                presence,
            } => Some((member, user, presence)),
            MemberListEntry::Group { .. } => None,
        }
    }

    /// Первый индекс member с `user_id`.
    #[must_use]
    pub fn member_list_index_of(
        &self,
        guild_id: u64,
        id: &MemberListId,
        user_id: u64,
    ) -> Option<usize> {
        self.member_list(guild_id, id)?
            .items
            .iter()
            .position(|entry| {
                matches!(
                    entry,
                    MemberListEntry::Member { member, .. } if member.user_id == user_id
                )
            })
    }

    /// Группа по строковому id (`online`/role-id).
    #[must_use]
    pub fn member_list_group<'a>(
        &'a self,
        guild_id: u64,
        id: &MemberListId,
        group_id: &str,
    ) -> Option<&'a MemberListGroup> {
        self.member_list(guild_id, id)?
            .groups
            .iter()
            .find(|group| group.id == group_id)
    }

    /// Окно, установленное последним `SYNC`.
    #[must_use]
    pub fn member_list_range(&self, guild_id: u64, id: &MemberListId) -> Option<MemberListRange> {
        self.member_list(guild_id, id)?.range
    }

    /// Число плоских элементов.
    #[must_use]
    pub fn member_list_len(&self, guild_id: u64, id: &MemberListId) -> usize {
        self.member_list(guild_id, id)
            .map_or(0, |list| list.items.len())
    }

    /// Был ли `SYNC` (есть окно).
    #[must_use]
    pub fn member_list_is_loaded(&self, guild_id: u64, id: &MemberListId) -> bool {
        self.member_list(guild_id, id)
            .is_some_and(|list| list.range.is_some())
    }

    /// Число списков в состоянии.
    #[must_use]
    pub fn member_list_count(&self) -> usize {
        self.member_lists.len()
    }
}
