//! P2-05a: memory-only `GUILD_MEMBERS_CHUNK`.
//!
//! Подтверждённая capture часть: `guild_id`, `chunk_index`, `chunk_count`, `members`,
//! `not_found`. `nonce` сохраняется как optional, но в доступных фикстурах отсутствует.
//! P2-09: `presences[]` — best-effort, форма в capture не подтверждена; принимаются только
//! элементы формы `PRESENCE_UPDATE`, факт помечается inconsistency. Wire-запрос не
//! изобретается; см. `waves/RW-18/DESIGN-P2-09.md` §3.

use std::collections::BTreeMap;

use chord_codec::etf::EtfValue;

use crate::ready::social::{parse_member_fields, parse_user};
use crate::ready::{GuildMember, ReadyDiagnostics, User, value};

use super::presence::{PresenceUpdate, parse_presence_event};
use super::types::{EventError, State, StateDiagnostics};

/// Участник chunk-а: member-поля и пользователь из `member.user`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberChunkMember {
    /// Поля участника.
    pub member: GuildMember,
    /// Пользователь.
    pub user: User,
}

/// Один `GUILD_MEMBERS_CHUNK`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberChunk {
    /// Гильдия.
    pub guild_id: u64,
    /// Nonce корреляции; в capture отсутствует.
    pub nonce: Option<String>,
    /// Индекс chunk-а.
    pub chunk_index: u32,
    /// Всего chunk-ов в ответе.
    pub chunk_count: u32,
    /// Участники этого chunk-а.
    pub members: Vec<MemberChunkMember>,
    /// Id, которых сервер не нашёл; сохраняются как есть.
    pub not_found: Vec<u64>,
    /// Best-effort presence из `presences[]` (форма не подтверждена).
    pub presences: Vec<PresenceUpdate>,
}

/// Сборка chunk-ов по `(guild_id, nonce)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberChunkAssembly {
    /// Гильдия.
    pub guild_id: u64,
    /// Nonce; `None` — поле отсутствовало.
    pub nonce: Option<String>,
    /// Ожидаемое число chunk-ов (first wins).
    pub chunk_count: u32,
    /// Индекс → последний полученный chunk.
    pub chunks: BTreeMap<u32, MemberChunk>,
}

impl MemberChunkAssembly {
    /// Все индексы `0..chunk_count` получены.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.chunk_count > 0
            && self.chunks.len() == self.chunk_count as usize
            && (0..self.chunk_count).all(|index| self.chunks.contains_key(&index))
    }

    /// Число уникальных chunk-индексов.
    #[must_use]
    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    /// Нет ни одного chunk-а.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// Участники по возрастанию `chunk_index`, затем в порядке прихода внутри chunk-а.
    pub fn members(&self) -> impl Iterator<Item = &MemberChunkMember> {
        self.chunks.values().flat_map(|chunk| chunk.members.iter())
    }
}

/// Префикс диагностики chunk-а.
pub(crate) fn chunk_prefix(guild_id: u64, nonce: Option<&str>) -> String {
    format!("member_chunk:{guild_id}:{}", nonce.unwrap_or("none"))
}

/// Разобрать `d` события; фатален только заголовок.
pub(crate) fn parse_member_chunk(
    value: &EtfValue,
) -> Result<(MemberChunk, StateDiagnostics), EventError> {
    let mut diagnostics = StateDiagnostics::default();

    let guild_raw = value.get("guild_id").ok_or_else(|| EventError::Missing {
        path: "d.guild_id".to_string(),
    })?;
    let guild_id = value::unsigned(guild_raw).ok_or_else(|| EventError::InvalidId {
        path: "d.guild_id".to_string(),
        value: format!("{guild_raw:?}"),
    })?;

    let chunk_index = required_u32(value.get("chunk_index"), "d.chunk_index")?;
    let chunk_count = required_u32(value.get("chunk_count"), "d.chunk_count")?;
    if chunk_count == 0 {
        return Err(EventError::WrongType {
            path: "d.chunk_count".to_string(),
            expected: "positive integer",
            found: "0",
        });
    }
    if chunk_index >= chunk_count {
        return Err(EventError::WrongType {
            path: "d.chunk_index".to_string(),
            expected: "index < chunk_count",
            found: "out-of-range",
        });
    }

    let nonce = parse_nonce(value.get("nonce"), &mut diagnostics);
    let nonce_label = nonce.as_deref().unwrap_or("none");
    let members = parse_members(
        value.get("members"),
        guild_id,
        nonce_label,
        &mut diagnostics,
    );
    let not_found = parse_not_found(value.get("not_found"), &mut diagnostics);

    let mut presences = Vec::new();
    if let Some(raw) = value.get("presences")
        && !value::is_null(raw)
    {
        // Форма presences[] в capture не наблюдалась. Не теряем факт молча: принимаем
        // только PRESENCE_UPDATE-подобные элементы и явно маркируем неизвестность.
        diagnostics.inconsistencies.push(format!(
            "{}:presences:unconfirmed-shape",
            chunk_prefix(guild_id, Some(nonce_label))
        ));
        presences = parse_chunk_presences(Some(raw), guild_id, nonce_label, &mut diagnostics);
    }

    Ok((
        MemberChunk {
            guild_id,
            nonce,
            chunk_index,
            chunk_count,
            members,
            not_found,
            presences,
        },
        diagnostics,
    ))
}

fn required_u32(raw: Option<&EtfValue>, path: &str) -> Result<u32, EventError> {
    let raw = raw.ok_or_else(|| EventError::Missing {
        path: path.to_string(),
    })?;
    value::unsigned(raw)
        .and_then(|number| u32::try_from(number).ok())
        .ok_or_else(|| EventError::WrongType {
            path: path.to_string(),
            expected: "u32",
            found: value::kind(raw),
        })
}

fn parse_nonce(raw: Option<&EtfValue>, diagnostics: &mut StateDiagnostics) -> Option<String> {
    let raw = raw?;
    if value::is_null(raw) {
        return None;
    }
    match value::text(raw) {
        Some(text) if !text.is_empty() => Some(text.to_string()),
        _ => {
            diagnostics.ignored.push("d.nonce".to_string());
            None
        }
    }
}

fn parse_members(
    raw: Option<&EtfValue>,
    guild_id: u64,
    nonce_label: &str,
    diagnostics: &mut StateDiagnostics,
) -> Vec<MemberChunkMember> {
    let Some(raw) = raw else {
        return Vec::new();
    };
    if value::is_null(raw) {
        return Vec::new();
    }
    let Some(items) = value::items(raw) else {
        diagnostics.ignored.push("d.members".to_string());
        return Vec::new();
    };

    let mut members = Vec::with_capacity(items.len());
    let mut positions: BTreeMap<u64, usize> = BTreeMap::new();
    for (index, item) in items.iter().enumerate() {
        let path = format!("d.members[{index}]");
        if item.as_map().is_none() {
            diagnostics.malformed.push(path);
            continue;
        }
        let Some(user_raw) = item.get("user") else {
            diagnostics.malformed.push(path);
            continue;
        };
        let mut ready = ReadyDiagnostics::default();
        let Some(user) = parse_user(user_raw, &format!("{path}.user"), &mut ready) else {
            diagnostics.malformed.push(path);
            diagnostics.merge(&StateDiagnostics::from_ready(&ready));
            continue;
        };
        let member = parse_member_fields(item, user.id, &path, &mut ready);
        diagnostics.merge(&StateDiagnostics::from_ready(&ready));

        let entry = MemberChunkMember { member, user };
        let user_id = entry.user.id;
        if let Some(position) = positions.get(&user_id).copied() {
            members[position] = entry;
            diagnostics.duplicates.push(format!(
                "{}:duplicate-member:{user_id}",
                chunk_prefix(guild_id, Some(nonce_label))
            ));
        } else {
            positions.insert(user_id, members.len());
            members.push(entry);
        }
    }
    members
}

fn parse_not_found(raw: Option<&EtfValue>, diagnostics: &mut StateDiagnostics) -> Vec<u64> {
    let Some(raw) = raw else {
        return Vec::new();
    };
    if value::is_null(raw) {
        return Vec::new();
    }
    let Some(items) = value::items(raw) else {
        diagnostics.ignored.push("d.not_found".to_string());
        return Vec::new();
    };
    let mut not_found = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        match value::unsigned(item) {
            Some(id) => not_found.push(id),
            None => diagnostics.ignored.push(format!("d.not_found[{index}]")),
        }
    }
    not_found
}

fn parse_chunk_presences(
    raw: Option<&EtfValue>,
    guild_id: u64,
    nonce_label: &str,
    diagnostics: &mut StateDiagnostics,
) -> Vec<PresenceUpdate> {
    let Some(raw) = raw else {
        return Vec::new();
    };
    if value::is_null(raw) {
        return Vec::new();
    }
    let Some(items) = value::items(raw) else {
        diagnostics.ignored.push("d.presences".to_string());
        return Vec::new();
    };
    let prefix = chunk_prefix(guild_id, Some(nonce_label));
    let mut presences = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        match parse_presence_event(item) {
            Ok((mut update, parsed)) => {
                diagnostics.merge(&parsed);
                match update.presence.guild_id {
                    None => update.presence.guild_id = Some(guild_id),
                    Some(id) if id == guild_id => {}
                    Some(_) => {
                        diagnostics
                            .inconsistencies
                            .push(format!("{prefix}:presences[{index}]:guild-mismatch"));
                        update.presence.guild_id = Some(guild_id);
                    }
                }
                presences.push(update);
            }
            Err(error) => diagnostics
                .malformed
                .push(format!("d.presences[{index}]:{error}")),
        }
    }
    presences
}

/// Применить chunk: сохранить в сборке и аддитивно слить участников.
pub(crate) fn apply_chunk(state: &mut State, chunk: MemberChunk) {
    let prefix = chunk_prefix(chunk.guild_id, chunk.nonce.as_deref());
    if chunk.chunk_count == 0 || chunk.chunk_index >= chunk.chunk_count {
        state.diagnostics.inconsistencies.push(format!(
            "{prefix}:invalid-index:{}/{}",
            chunk.chunk_index, chunk.chunk_count
        ));
        return;
    }

    let key = (chunk.guild_id, chunk.nonce.clone());
    let duplicate_index;
    {
        let assembly = state
            .member_chunks
            .entry(key)
            .or_insert_with(|| MemberChunkAssembly {
                guild_id: chunk.guild_id,
                nonce: chunk.nonce.clone(),
                chunk_count: chunk.chunk_count,
                chunks: BTreeMap::new(),
            });
        if assembly.chunk_count != chunk.chunk_count {
            state.diagnostics.inconsistencies.push(format!(
                "{prefix}:chunk-count:{}->{}",
                assembly.chunk_count, chunk.chunk_count
            ));
            return;
        }
        duplicate_index = assembly
            .chunks
            .insert(chunk.chunk_index, chunk.clone())
            .is_some();
    }
    // Повтор `0/1` без nonce — обычный новый ответ на другой запрос (capture: s=78 и
    // s=84 для одной гильдии), поэтому только этот случай не диагностируется. Для
    // count>1 без nonce корреляция неизвестна: повтор индекса — потенциальный дубль
    // внутри batch-а, его нельзя молча подавлять.
    if duplicate_index && (chunk.nonce.is_some() || chunk.chunk_count > 1) {
        state
            .diagnostics
            .duplicates
            .push(format!("{prefix}:duplicate-index:{}", chunk.chunk_index));
    }

    for update in &chunk.presences {
        super::presence_reducer::apply_update(state, update.clone());
    }
    for entry in &chunk.members {
        state.users.insert(entry.user.id, entry.user.clone());
    }
    let Some(guild) = state.guilds.get_mut(&chunk.guild_id) else {
        state
            .diagnostics
            .inconsistencies
            .push(format!("guilds:{}:member-chunk", chunk.guild_id));
        return;
    };
    for entry in &chunk.members {
        if let Some(existing) = guild
            .members
            .iter_mut()
            .find(|member| member.user_id == entry.member.user_id)
        {
            *existing = entry.member.clone();
        } else {
            guild.members.push(entry.member.clone());
        }
    }
}
