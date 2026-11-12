//! P2-05c: селекторы chunk-ов и presence-effect для видимого диапазона.
//!
//! Все функции только читают состояние. `member_list_presence_effect` возвращает
//! декларацию `Effect::RequestPresence`; сеть/async — вне `discord-core`.

use std::collections::BTreeSet;

use super::member_chunk::{MemberChunkAssembly, MemberChunkMember};
use super::member_list::{MemberListEntry, MemberListId, MemberListRange};
use super::types::{Effect, State};

impl State {
    /// Все сборки гильдии; порядок — `(nonce)` как в `BTreeMap`.
    pub fn member_chunks(&self, guild_id: u64) -> impl Iterator<Item = &MemberChunkAssembly> {
        self.member_chunks
            .iter()
            .filter(move |((guild, _), _)| *guild == guild_id)
            .map(|(_, assembly)| assembly)
    }

    /// Сборка по `(guild_id, nonce)`; `None` — nonce отсутствовал.
    #[must_use]
    pub fn member_chunk(&self, guild_id: u64, nonce: Option<&str>) -> Option<&MemberChunkAssembly> {
        self.member_chunks
            .get(&(guild_id, nonce.map(str::to_string)))
    }

    /// Все индексы chunk-ов получены.
    #[must_use]
    pub fn member_chunk_is_complete(&self, guild_id: u64, nonce: Option<&str>) -> bool {
        self.member_chunk(guild_id, nonce)
            .is_some_and(MemberChunkAssembly::is_complete)
    }

    /// Число уникальных chunk-индексов.
    #[must_use]
    pub fn member_chunk_len(&self, guild_id: u64, nonce: Option<&str>) -> usize {
        self.member_chunk(guild_id, nonce)
            .map_or(0, MemberChunkAssembly::len)
    }

    /// Число участников во всех полученных chunk-ах.
    #[must_use]
    pub fn member_chunk_member_count(&self, guild_id: u64, nonce: Option<&str>) -> usize {
        self.member_chunk(guild_id, nonce).map_or(0, |assembly| {
            assembly
                .chunks
                .values()
                .map(|chunk| chunk.members.len())
                .sum()
        })
    }

    /// Участники по возрастанию `chunk_index`.
    pub fn member_chunk_members(
        &self,
        guild_id: u64,
        nonce: Option<&str>,
    ) -> impl Iterator<Item = &MemberChunkMember> {
        self.member_chunk(guild_id, nonce)
            .into_iter()
            .flat_map(MemberChunkAssembly::members)
    }

    /// Первый участник с `user_id` в порядке chunk-ов.
    #[must_use]
    pub fn member_chunk_member(
        &self,
        guild_id: u64,
        nonce: Option<&str>,
        user_id: u64,
    ) -> Option<&MemberChunkMember> {
        self.member_chunk_members(guild_id, nonce)
            .find(|member| member.member.user_id == user_id)
    }

    /// `not_found` всех chunk-ов в порядке индексов.
    #[must_use]
    pub fn member_chunk_not_found(&self, guild_id: u64, nonce: Option<&str>) -> Vec<u64> {
        self.member_chunk(guild_id, nonce)
            .map_or_else(Vec::new, |assembly| {
                assembly
                    .chunks
                    .values()
                    .flat_map(|chunk| chunk.not_found.iter().copied())
                    .collect()
            })
    }

    /// Число сборок chunk-ов в состоянии.
    #[must_use]
    pub fn member_chunk_count(&self) -> usize {
        self.member_chunks.len()
    }

    /// Декларация запроса presence для видимого диапазона member list P2-04.
    ///
    /// Группы пропускаются, member-id дедуплицируются с сохранением первого порядка.
    /// Пустой диапазон/список/только группы → `None`; сеть не вызывается.
    #[must_use]
    pub fn member_list_presence_effect(
        &self,
        guild_id: u64,
        list_id: &MemberListId,
        range: MemberListRange,
    ) -> Option<Effect> {
        if range.start > range.end {
            return None;
        }
        let items = self.member_list_items(guild_id, list_id);
        let start = range.start as usize;
        if start >= items.len() {
            return None;
        }
        let end = (range.end as usize).min(items.len() - 1);
        let mut seen = BTreeSet::new();
        let mut user_ids = Vec::new();
        for entry in &items[start..=end] {
            let MemberListEntry::Member { member, .. } = entry else {
                continue;
            };
            if seen.insert(member.user_id) {
                user_ids.push(member.user_id);
            }
        }
        if user_ids.is_empty() {
            None
        } else {
            Some(Effect::RequestPresence { guild_id, user_ids })
        }
    }
}
