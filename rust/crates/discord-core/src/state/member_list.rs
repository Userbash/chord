//! P2-04a: memory-only ленивый список участников.
//!
//! Подтверждённая capture часть `GUILD_MEMBER_LIST_UPDATE`: плоский список `group|member`,
//! ops `SYNC`/`INSERT`/`UPDATE`/`DELETE`, counts/groups как сырые значения. Wire-формат
//! подписки op37/op14 **не подтверждён** и здесь не изобретается: интерфейсом остаётся
//! `state::Effect::SubscribeMembers` (P2-02). Контракт — `waves/RW-15/DESIGN-P2-04.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use discord_codec::etf::EtfValue;

use crate::ready::social::{parse_member_fields, parse_user};
use crate::ready::{GuildMember, ReadyDiagnostics, User, value};

use super::presence::{Activity, normalize_activities, parse_activities, parse_i64_field};
use super::types::{EventError, State, StateDiagnostics};

/// Идентификатор списка участников как он пришёл в проводе.
///
/// Это **не** снежинка: в capture встречаются `"everyone"` и строковый `"1659606647"`.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MemberListId(String);

impl MemberListId {
    /// Новый raw-идентификатор.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Срез raw-значения.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MemberListId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl From<&str> for MemberListId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for MemberListId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Включительный диапазон плоского списка.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemberListRange {
    /// Начало.
    pub start: u32,
    /// Конец (включительно).
    pub end: u32,
}

/// Заголовок группы списка; `count` — сырое число из события.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberListGroup {
    /// `online`/`offline`/role-id строкой.
    pub id: String,
    /// Сырой счётчик; не выводится из `items`.
    pub count: u32,
}

/// Presence-снимок member-list; P2-09 добавляет activities/timestamps.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemberPresence {
    /// `online`/`idle`/`dnd`/`offline`.
    pub status: Option<String>,
    /// Клиенты (`desktop`/`mobile`/`web`/…).
    pub client_status: BTreeMap<String, String>,
    /// Нормализованные activities.
    pub activities: Vec<Activity>,
    /// Скрытые activities.
    pub hidden_activities: Vec<Activity>,
    /// Время обработки сервером.
    pub processed_at_timestamp: Option<i64>,
}

/// Элемент плоского списка: заголовок группы или участник.
///
/// Размер варианта `Member` больше `Group`, но список ограничен окном порядка 100
/// элементов; box на каждый member дал бы лишние аллокации без выигрыша для UI.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberListEntry {
    /// Заголовок группы.
    Group {
        /// Идентификатор группы (строка).
        id: String,
    },
    /// Участник с list-local снимком пользователя и presence.
    Member {
        /// Поля участника.
        member: GuildMember,
        /// Пользователь из `item.member.user`.
        user: User,
        /// Минимальный presence.
        presence: MemberPresence,
    },
}

/// Одна операция списка.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberListOp {
    /// Полная замена плоского списка.
    Sync {
        /// Новые элементы.
        items: Vec<MemberListEntry>,
        /// Новое окно.
        range: Option<MemberListRange>,
    },
    /// Вставка со сдвигом.
    Insert {
        /// Позиция в плоском списке.
        index: u32,
        /// Элемент.
        item: MemberListEntry,
    },
    /// Замена без сдвига.
    Update {
        /// Позиция.
        index: u32,
        /// Новый элемент.
        item: MemberListEntry,
    },
    /// Удаление со сдвигом.
    Delete {
        /// Позиция.
        index: u32,
    },
}

/// Разобранное событие `GUILD_MEMBER_LIST_UPDATE`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberListUpdate {
    /// Гильдия.
    pub guild_id: u64,
    /// Raw id списка.
    pub id: MemberListId,
    /// Сырой `member_count`.
    pub member_count: Option<i64>,
    /// Сырой `online_count`.
    pub online_count: Option<i64>,
    /// Снимок заголовков групп.
    pub groups: Vec<MemberListGroup>,
    /// Ops в порядке провода.
    pub ops: Vec<MemberListOp>,
}

/// Накопленный список участников.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberList {
    /// Гильдия.
    pub guild_id: u64,
    /// Raw id.
    pub id: MemberListId,
    /// Сырой `member_count`.
    pub member_count: Option<i64>,
    /// Сырой `online_count`.
    pub online_count: Option<i64>,
    /// Снимок групп.
    pub groups: Vec<MemberListGroup>,
    /// Плоский список `group|member`.
    pub items: Vec<MemberListEntry>,
    /// Окно, установленное последним `SYNC`.
    pub range: Option<MemberListRange>,
}

impl MemberList {
    fn new(guild_id: u64, id: MemberListId) -> Self {
        Self {
            guild_id,
            id,
            member_count: None,
            online_count: None,
            groups: Vec::new(),
            items: Vec::new(),
            range: None,
        }
    }
}

/// Разобрать `d` события; фатален только заголовок.
pub(crate) fn parse_member_list_update(
    value: &EtfValue,
) -> Result<(MemberListUpdate, StateDiagnostics), EventError> {
    let mut diagnostics = StateDiagnostics::default();

    let guild_raw = value.get("guild_id").ok_or_else(|| EventError::Missing {
        path: "d.guild_id".to_string(),
    })?;
    let guild_id = value::unsigned(guild_raw).ok_or_else(|| EventError::InvalidId {
        path: "d.guild_id".to_string(),
        value: format!("{guild_raw:?}"),
    })?;

    let id_raw = value.get("id").ok_or_else(|| EventError::Missing {
        path: "d.id".to_string(),
    })?;
    let id_text = value::text(id_raw)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| EventError::WrongType {
            path: "d.id".to_string(),
            expected: "non-empty string",
            found: value::kind(id_raw),
        })?;

    let ops_raw = value.get("ops").ok_or_else(|| EventError::Missing {
        path: "d.ops".to_string(),
    })?;
    let ops_values = value::items(ops_raw).ok_or_else(|| EventError::WrongType {
        path: "d.ops".to_string(),
        expected: "list",
        found: value::kind(ops_raw),
    })?;

    let groups = parse_groups(value.get("groups"), &mut diagnostics);
    let member_count = optional_int(
        value.get("member_count"),
        "d.member_count",
        &mut diagnostics,
    );
    let online_count = optional_int(
        value.get("online_count"),
        "d.online_count",
        &mut diagnostics,
    );
    let list_id = MemberListId::new(id_text);

    let mut ops = Vec::with_capacity(ops_values.len());
    for (index, raw_op) in ops_values.iter().enumerate() {
        let path = format!("d.ops[{index}]");
        if let Some(op) = parse_op(raw_op, &path, guild_id, &list_id, &mut diagnostics) {
            ops.push(op);
        }
    }

    Ok((
        MemberListUpdate {
            guild_id,
            id: list_id,
            member_count,
            online_count,
            groups,
            ops,
        },
        diagnostics,
    ))
}

fn optional_int(
    raw: Option<&EtfValue>,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Option<i64> {
    let raw = raw?;
    if value::is_null(raw) {
        return None;
    }
    match value::integer(raw) {
        Some(number) => Some(number),
        None => {
            diagnostics.ignored.push(path.to_string());
            None
        }
    }
}

fn parse_groups(
    raw: Option<&EtfValue>,
    diagnostics: &mut StateDiagnostics,
) -> Vec<MemberListGroup> {
    let Some(raw) = raw else {
        return Vec::new();
    };
    if value::is_null(raw) {
        return Vec::new();
    }
    let Some(items) = value::items(raw) else {
        diagnostics.ignored.push("d.groups".to_string());
        return Vec::new();
    };

    let mut groups = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let path = format!("d.groups[{index}]");
        let Some(id) = item.get("id").and_then(value::text) else {
            diagnostics.malformed.push(path);
            continue;
        };
        if id.is_empty() {
            diagnostics.malformed.push(path);
            continue;
        }
        let count = match item.get("count") {
            Some(raw) if !value::is_null(raw) => {
                match value::integer(raw).and_then(|number| u32::try_from(number).ok()) {
                    Some(count) => count,
                    None => {
                        diagnostics.ignored.push(format!("{path}.count"));
                        0
                    }
                }
            }
            _ => 0,
        };
        groups.push(MemberListGroup {
            id: id.to_string(),
            count,
        });
    }
    groups
}

fn parse_index(raw: Option<&EtfValue>) -> Option<u32> {
    let raw = raw?;
    let number = value::unsigned(raw)?;
    u32::try_from(number).ok()
}

fn parse_range(
    raw: Option<&EtfValue>,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Option<MemberListRange> {
    let raw = raw?;
    if value::is_null(raw) {
        return None;
    }
    if let Some(text) = value::text(raw) {
        let bytes = text.as_bytes();
        if bytes.len() == 2 {
            return Some(MemberListRange {
                start: u32::from(bytes[0]),
                end: u32::from(bytes[1]),
            });
        }
        diagnostics.ignored.push(path.to_string());
        return None;
    }
    if let Some(items) = value::items(raw)
        && items.len() == 2
    {
        let start = value::unsigned(&items[0]).and_then(|value| u32::try_from(value).ok());
        let end = value::unsigned(&items[1]).and_then(|value| u32::try_from(value).ok());
        if let (Some(start), Some(end)) = (start, end) {
            return Some(MemberListRange { start, end });
        }
    }
    diagnostics.ignored.push(path.to_string());
    None
}

fn parse_op(
    raw: &EtfValue,
    path: &str,
    guild_id: u64,
    list_id: &MemberListId,
    diagnostics: &mut StateDiagnostics,
) -> Option<MemberListOp> {
    let Some(name) = raw.get("op").and_then(value::text) else {
        diagnostics.malformed.push(path.to_string());
        return None;
    };
    match name {
        "SYNC" => parse_sync(raw, path, diagnostics),
        "INSERT" => parse_insert_update(raw, path, diagnostics, true),
        "UPDATE" => parse_insert_update(raw, path, diagnostics, false),
        "DELETE" => {
            let index = parse_index(raw.get("index")).or_else(|| {
                diagnostics.malformed.push(format!("{path}.index"));
                None
            })?;
            Some(MemberListOp::Delete { index })
        }
        other => {
            diagnostics.inconsistencies.push(format!(
                "member_list:{guild_id}:{list_id}:unknown-op:{other}"
            ));
            None
        }
    }
}

fn parse_sync(
    raw: &EtfValue,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Option<MemberListOp> {
    let Some(items_raw) = raw.get("items") else {
        diagnostics.malformed.push(format!("{path}.items"));
        return None;
    };
    let Some(items_values) = value::items(items_raw) else {
        diagnostics.malformed.push(format!("{path}.items"));
        return None;
    };
    let mut items = Vec::with_capacity(items_values.len());
    for (index, item) in items_values.iter().enumerate() {
        if let Some(entry) = parse_entry(item, &format!("{path}.items[{index}]"), diagnostics) {
            items.push(entry);
        }
    }
    let range = parse_range(raw.get("range"), &format!("{path}.range"), diagnostics);
    Some(MemberListOp::Sync { items, range })
}

fn parse_insert_update(
    raw: &EtfValue,
    path: &str,
    diagnostics: &mut StateDiagnostics,
    insert: bool,
) -> Option<MemberListOp> {
    let Some(index) = parse_index(raw.get("index")) else {
        diagnostics.malformed.push(format!("{path}.index"));
        return None;
    };
    let Some(item_raw) = raw.get("item") else {
        diagnostics.malformed.push(format!("{path}.item"));
        return None;
    };
    let item = parse_entry(item_raw, &format!("{path}.item"), diagnostics)?;
    Some(if insert {
        MemberListOp::Insert { index, item }
    } else {
        MemberListOp::Update { index, item }
    })
}

fn parse_entry(
    raw: &EtfValue,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Option<MemberListEntry> {
    if raw.get("member").is_some() {
        let parsed = parse_member_entry(raw, path, diagnostics);
        if parsed.is_none() {
            diagnostics.malformed.push(path.to_string());
        }
        return parsed;
    }
    if let Some(group_raw) = raw.get("group") {
        let Some(id) = group_raw
            .get("id")
            .and_then(value::text)
            .filter(|id| !id.is_empty())
        else {
            diagnostics.malformed.push(path.to_string());
            return None;
        };
        return Some(MemberListEntry::Group { id: id.to_string() });
    }
    diagnostics.malformed.push(path.to_string());
    None
}

fn parse_member_entry(
    raw: &EtfValue,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Option<MemberListEntry> {
    let member_raw = raw.get("member")?;
    if value::is_null(member_raw) {
        return None;
    }
    let user_raw = member_raw.get("user")?;
    if value::is_null(user_raw) {
        return None;
    }
    let mut ready_diagnostics = ReadyDiagnostics::default();
    let user = parse_user(
        user_raw,
        &format!("{path}.member.user"),
        &mut ready_diagnostics,
    );
    let Some(user) = user else {
        diagnostics.merge(&StateDiagnostics::from_ready(&ready_diagnostics));
        return None;
    };
    let member = parse_member_fields(
        member_raw,
        user.id,
        &format!("{path}.member"),
        &mut ready_diagnostics,
    );
    diagnostics.merge(&StateDiagnostics::from_ready(&ready_diagnostics));
    let presence = parse_presence(
        member_raw.get("presence"),
        &format!("{path}.member.presence"),
        diagnostics,
    );
    Some(MemberListEntry::Member {
        member,
        user,
        presence,
    })
}

fn parse_presence(
    raw: Option<&EtfValue>,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> MemberPresence {
    let Some(raw) = raw else {
        return MemberPresence::default();
    };
    if value::is_null(raw) {
        return MemberPresence::default();
    }
    if value::object(raw).is_none() {
        diagnostics.ignored.push(path.to_string());
        return MemberPresence::default();
    }
    let status = match raw.get("status") {
        Some(status) if !value::is_null(status) => match value::text(status) {
            Some(status) if !status.is_empty() => Some(status.to_string()),
            Some(_) | None => {
                diagnostics.ignored.push(format!("{path}.status"));
                None
            }
        },
        _ => None,
    };
    let mut client_status = BTreeMap::new();
    match raw.get("client_status") {
        Some(raw) if !value::is_null(raw) => match value::object(raw) {
            Some(entries) => {
                for (key, value) in entries {
                    let Some(key) = value::text(key) else {
                        diagnostics
                            .ignored
                            .push(format!("{path}.client_status.key"));
                        continue;
                    };
                    let Some(value) = value::text(value) else {
                        diagnostics
                            .ignored
                            .push(format!("{path}.client_status.{key}"));
                        continue;
                    };
                    if key.is_empty() {
                        diagnostics
                            .ignored
                            .push(format!("{path}.client_status.empty-key"));
                        continue;
                    }
                    if value.is_empty() {
                        diagnostics
                            .ignored
                            .push(format!("{path}.client_status.{key}.empty"));
                        continue;
                    }
                    client_status.insert(key.to_string(), value.to_string());
                }
            }
            None => diagnostics.ignored.push(format!("{path}.client_status")),
        },
        Some(_) => {}
        None => {}
    }
    let activities = normalize_activities(parse_activities(
        raw.get("activities"),
        &format!("{path}.activities"),
        diagnostics,
    ));
    let hidden_activities = parse_activities(
        raw.get("hidden_activities"),
        &format!("{path}.hidden_activities"),
        diagnostics,
    );
    let processed_at_timestamp = parse_i64_field(
        raw.get("processed_at_timestamp"),
        &format!("{path}.processed_at_timestamp"),
        diagnostics,
    );
    MemberPresence {
        status,
        client_status,
        activities,
        hidden_activities,
        processed_at_timestamp,
    }
}

/// Применить событие к `State`; ошибки ops не отменяют уже применённые.
pub(crate) fn apply_update(state: &mut State, update: MemberListUpdate) {
    let MemberListUpdate {
        guild_id,
        id,
        member_count,
        online_count,
        groups,
        ops,
    } = update;
    if !state.guilds.contains_key(&guild_id) {
        state
            .diagnostics
            .inconsistencies
            .push(format!("guilds:{guild_id}:member-list"));
    }

    // Presence применяется до list-op и независимо от успеха вставки/замены. DELETE
    // presence не удаляет: его жизненным циклом управляет presence-store.
    for op in &ops {
        let entries: &[MemberListEntry] = match op {
            MemberListOp::Sync { items, .. } => items,
            MemberListOp::Insert { item, .. } | MemberListOp::Update { item, .. } => {
                std::slice::from_ref(item)
            }
            MemberListOp::Delete { .. } => &[],
        };
        for entry in entries {
            if let MemberListEntry::Member {
                member, presence, ..
            } = entry
            {
                super::presence_reducer::apply_member_presence(
                    state,
                    guild_id,
                    member.user_id,
                    presence,
                );
            }
        }
    }

    let list = state
        .member_lists
        .entry((guild_id, id.clone()))
        .or_insert_with(|| MemberList::new(guild_id, id.clone()));
    if member_count.is_some() {
        list.member_count = member_count;
    }
    if online_count.is_some() {
        list.online_count = online_count;
    }
    list.groups = groups;

    let diagnostics = &mut state.diagnostics;
    for op in ops {
        apply_op(list, op, diagnostics, guild_id, &id);
    }
}

fn apply_op(
    list: &mut MemberList,
    op: MemberListOp,
    diagnostics: &mut StateDiagnostics,
    guild_id: u64,
    list_id: &MemberListId,
) {
    match op {
        MemberListOp::Sync { items, range } => {
            list.items = items;
            list.range = range;
            diagnose_duplicates(list, diagnostics, guild_id, list_id);
        }
        MemberListOp::Insert { index, item } => {
            let index = index as usize;
            if index > list.items.len() {
                out_of_range(diagnostics, guild_id, list_id, "INSERT", index);
                return;
            }
            // Перед вставкой проверяем все существующие элементы: элемент с позицией
            // `index` после вставки сдвинется на `index + 1` и останется в списке.
            if let Some(user_id) = member_id(&item)
                && list
                    .items
                    .iter()
                    .any(|entry| member_id(entry) == Some(user_id))
            {
                duplicate(diagnostics, guild_id, list_id, user_id);
            }
            list.items.insert(index, item);
        }
        MemberListOp::Update { index, item } => {
            let index = index as usize;
            if index >= list.items.len() {
                out_of_range(diagnostics, guild_id, list_id, "UPDATE", index);
                return;
            }
            if let Some(user_id) = member_id(&item)
                && list
                    .items
                    .iter()
                    .enumerate()
                    .any(|(position, entry)| position != index && member_id(entry) == Some(user_id))
            {
                duplicate(diagnostics, guild_id, list_id, user_id);
            }
            list.items[index] = item;
        }
        MemberListOp::Delete { index } => {
            let index = index as usize;
            if index >= list.items.len() {
                out_of_range(diagnostics, guild_id, list_id, "DELETE", index);
                return;
            }
            list.items.remove(index);
        }
    }
}

fn member_id(entry: &MemberListEntry) -> Option<u64> {
    match entry {
        MemberListEntry::Member { member, .. } => Some(member.user_id),
        MemberListEntry::Group { .. } => None,
    }
}

fn diagnose_duplicates(
    list: &MemberList,
    diagnostics: &mut StateDiagnostics,
    guild_id: u64,
    list_id: &MemberListId,
) {
    let mut seen = BTreeSet::new();
    for entry in &list.items {
        if let Some(user_id) = member_id(entry)
            && !seen.insert(user_id)
        {
            duplicate(diagnostics, guild_id, list_id, user_id);
        }
    }
}

fn out_of_range(
    diagnostics: &mut StateDiagnostics,
    guild_id: u64,
    list_id: &MemberListId,
    op: &str,
    index: usize,
) {
    diagnostics.inconsistencies.push(format!(
        "member_list:{guild_id}:{list_id}:out-of-range:{op}:{index}"
    ));
}

fn duplicate(
    diagnostics: &mut StateDiagnostics,
    guild_id: u64,
    list_id: &MemberListId,
    user_id: u64,
) {
    diagnostics.duplicates.push(format!(
        "member_list:{guild_id}:{list_id}:duplicate:{user_id}"
    ));
}
