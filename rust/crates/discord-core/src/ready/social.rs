//! READY: пользователи, ЛС, связи, сессии, участники (RW-12 / P2-01b).
//!
//! Типы и сигнатуры заморожены `waves/RW-12/DESIGN-P2-01.md` §3.3–3.4. Политика разбора —
//! §2.3: битый элемент коллекции пропускается с `diagnostics.malformed`, неверный тип
//! необязательной коллекции даёт пустой `Vec` + `diagnostics.ignored`, `nil`/отсутствие —
//! пусто без диагностики. Дубликаты — last wins, позиция первого, запись в
//! `diagnostics.duplicates`. `Session` печатается с redacted `session_id`.

use std::fmt;

use discord_codec::etf::EtfValue;

use super::ReadyDiagnostics;
use super::value;

/// Необязательный текст: отсутствие/`nil` → `None`; неверный тип → `None` + `ignored`.
fn text_at(
    item: &EtfValue,
    key: &str,
    path: &str,
    diagnostics: &mut ReadyDiagnostics,
) -> Option<String> {
    let raw = item.get(key)?;
    if value::is_null(raw) {
        return None;
    }
    match value::text(raw) {
        Some(text) => Some(text.to_string()),
        None => {
            diagnostics.ignored.push(format!("{path}.{key}"));
            None
        }
    }
}

/// Необязательное целое; неверный тип → `None` + `ignored`.
fn int_at(
    item: &EtfValue,
    key: &str,
    path: &str,
    diagnostics: &mut ReadyDiagnostics,
) -> Option<i64> {
    let raw = item.get(key)?;
    if value::is_null(raw) {
        return None;
    }
    match value::integer(raw) {
        Some(number) => Some(number),
        None => {
            diagnostics.ignored.push(format!("{path}.{key}"));
            None
        }
    }
}

/// Необязательный bool; неверный тип → `false` + `ignored`.
fn bool_at(item: &EtfValue, key: &str, path: &str, diagnostics: &mut ReadyDiagnostics) -> bool {
    let Some(raw) = item.get(key) else {
        return false;
    };
    if value::is_null(raw) {
        return false;
    }
    match value::boolean(raw) {
        Some(flag) => flag,
        None => {
            diagnostics.ignored.push(format!("{path}.{key}"));
            false
        }
    }
}

/// Необязательная снежинка; неверный тип → `None` + `ignored`.
fn uint_at(
    item: &EtfValue,
    key: &str,
    path: &str,
    diagnostics: &mut ReadyDiagnostics,
) -> Option<u64> {
    let raw = item.get(key)?;
    if value::is_null(raw) {
        return None;
    }
    match value::unsigned(raw) {
        Some(id) => Some(id),
        None => {
            diagnostics.ignored.push(format!("{path}.{key}"));
            None
        }
    }
}

/// Список снежинок: неверный контейнер/элемент → `ignored`.
fn ids_at(item: &EtfValue, key: &str, path: &str, diagnostics: &mut ReadyDiagnostics) -> Vec<u64> {
    let Some(raw) = item.get(key) else {
        return Vec::new();
    };
    if value::is_null(raw) {
        return Vec::new();
    }
    let Some(items) = value::items(raw) else {
        diagnostics.ignored.push(format!("{path}.{key}"));
        return Vec::new();
    };
    let mut ids = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        match value::unsigned(item) {
            Some(id) => ids.push(id),
            None => diagnostics.ignored.push(format!("{path}.{key}[{index}]")),
        }
    }
    ids
}

/// Пользователь.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct User {
    /// Снежинка.
    pub id: u64,
    /// Имя.
    pub username: Option<String>,
    /// Глобальное имя.
    pub global_name: Option<String>,
    /// display_name (только `users[]`).
    pub display_name: Option<String>,
    /// Дискриминатор.
    pub discriminator: Option<String>,
    /// Аватар.
    pub avatar: Option<String>,
    /// Бот; отсутствие в `users[]` → false (наша договорённость).
    pub bot: bool,
    /// Публичные флаги (только `users[]`).
    pub public_flags: Option<i64>,
}

/// Приватный канал (ЛС/группа).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PrivateChannel {
    /// Снежинка.
    pub id: u64,
    /// `type`.
    pub channel_type: i64,
    /// Флаги.
    pub flags: i64,
    /// Получатели.
    pub recipient_ids: Vec<u64>,
    /// Последнее сообщение.
    pub last_message_id: Option<u64>,
    /// Последний пин.
    pub last_pin_timestamp: Option<String>,
    /// Запрос сообщения.
    pub is_message_request: bool,
    /// Время запроса.
    pub is_message_request_timestamp: Option<String>,
    /// Спам.
    pub is_spam: bool,
    /// Флаги получателя.
    pub recipient_flags: i64,
}

/// Связь.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Relationship {
    /// Снежинка связи.
    pub id: u64,
    /// Снежинка пользователя.
    pub user_id: u64,
    /// `type` (1 — друг, 2 — блок).
    pub relation_type: i64,
    /// Ник.
    pub nickname: Option<String>,
    /// С какого времени.
    pub since: Option<String>,
    /// Спам-запрос.
    pub is_spam_request: bool,
    /// Пользователь игнорируется.
    pub user_ignored: bool,
}

/// Участник гильдии из `merged_members`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GuildMember {
    /// Снежинка.
    pub user_id: u64,
    /// Ник.
    pub nick: Option<String>,
    /// Аватар.
    pub avatar: Option<String>,
    /// Баннер.
    pub banner: Option<String>,
    /// Вступление.
    pub joined_at: Option<String>,
    /// Premium since.
    pub premium_since: Option<String>,
    /// Communication disabled until.
    pub communication_disabled_until: Option<String>,
    /// Deaf.
    pub deaf: bool,
    /// Mute.
    pub mute: bool,
    /// Pending.
    pub pending: bool,
    /// Флаги.
    pub flags: i64,
    /// Роли.
    pub roles: Vec<u64>,
}

/// Клиент сессии.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClientInfo {
    /// Клиент.
    pub client: Option<String>,
    /// ОС.
    pub os: Option<String>,
    /// Версия.
    pub version: i64,
}

/// Сессия; `Debug` не печатает `session_id`.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Session {
    /// ID сессии.
    pub session_id: String,
    /// Статус.
    pub status: Option<String>,
    /// Клиент.
    pub client_info: ClientInfo,
    /// Время обработки.
    pub processed_at_timestamp: i64,
}

impl fmt::Debug for Session {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Session")
            .field("session_id", &"<redacted>")
            .field("status", &self.status)
            .field("client_info", &self.client_info)
            .field("processed_at_timestamp", &self.processed_at_timestamp)
            .finish()
    }
}

/// Разобрать пользователя.
///
/// Контракт: `None` — для отсутствующего/битого `id` (отрицательное число, не-число,
/// переполнение); иначе `User` с дефолтами для отсутствующих полей. `bot` отсутствует
/// в 41/146 записей `users[]` — договорённость «не бот/неизвестно» → `false`.
pub(crate) fn parse_user(
    value: &EtfValue,
    path: &str,
    diagnostics: &mut ReadyDiagnostics,
) -> Option<User> {
    let id = value::unsigned(value.get("id")?)?;
    Some(User {
        id,
        username: text_at(value, "username", path, diagnostics),
        global_name: text_at(value, "global_name", path, diagnostics),
        display_name: text_at(value, "display_name", path, diagnostics),
        discriminator: text_at(value, "discriminator", path, diagnostics),
        avatar: text_at(value, "avatar", path, diagnostics),
        bot: bool_at(value, "bot", path, diagnostics),
        public_flags: int_at(value, "public_flags", path, diagnostics),
    })
}

/// `d.users`: битые элементы → `malformed` (`users[i]`), дубликаты id → last wins.
pub(crate) fn parse_users(
    value: Option<&EtfValue>,
    diagnostics: &mut ReadyDiagnostics,
) -> Vec<User> {
    let Some(value) = value else {
        return Vec::new();
    };
    if value::is_null(value) {
        return Vec::new();
    }
    let Some(items) = value::items(value) else {
        diagnostics.ignored.push("d.users".to_string());
        return Vec::new();
    };

    let mut users = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        match parse_user(item, &format!("d.users[{index}]"), diagnostics) {
            Some(user) => users.push(user),
            None => diagnostics.malformed.push(format!("users[{index}]")),
        }
    }
    value::dedup_last_by(
        users,
        |user| user.id,
        |user| user.id.to_string(),
        "users",
        diagnostics,
    )
}

/// Разобрать приватный канал; обязателен только `id` (DESIGN §2.3).
fn parse_private_channel(
    value: &EtfValue,
    path: &str,
    diagnostics: &mut ReadyDiagnostics,
) -> Option<PrivateChannel> {
    let id = value::unsigned(value.get("id")?)?;
    Some(PrivateChannel {
        id,
        channel_type: int_at(value, "type", path, diagnostics).unwrap_or(0),
        flags: int_at(value, "flags", path, diagnostics).unwrap_or(0),
        recipient_ids: ids_at(value, "recipient_ids", path, diagnostics),
        last_message_id: uint_at(value, "last_message_id", path, diagnostics),
        last_pin_timestamp: text_at(value, "last_pin_timestamp", path, diagnostics),
        is_message_request: bool_at(value, "is_message_request", path, diagnostics),
        is_message_request_timestamp: text_at(
            value,
            "is_message_request_timestamp",
            path,
            diagnostics,
        ),
        is_spam: bool_at(value, "is_spam", path, diagnostics),
        recipient_flags: int_at(value, "recipient_flags", path, diagnostics).unwrap_or(0),
    })
}

/// `d.private_channels`: битые элементы → `malformed` (`private_channels[i]`), дубликаты — last wins.
pub(crate) fn parse_private_channels(
    value: Option<&EtfValue>,
    diagnostics: &mut ReadyDiagnostics,
) -> Vec<PrivateChannel> {
    let Some(value) = value else {
        return Vec::new();
    };
    if value::is_null(value) {
        return Vec::new();
    }
    let Some(items) = value::items(value) else {
        diagnostics.ignored.push("d.private_channels".to_string());
        return Vec::new();
    };

    let mut channels = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        match parse_private_channel(item, &format!("d.private_channels[{index}]"), diagnostics) {
            Some(channel) => channels.push(channel),
            None => diagnostics
                .malformed
                .push(format!("private_channels[{index}]")),
        }
    }
    value::dedup_last_by(
        channels,
        |channel| channel.id,
        |channel| channel.id.to_string(),
        "private_channels",
        diagnostics,
    )
}

/// Разобрать связь; обязательны `id`, `user_id`, `type` (DESIGN §2.3).
fn parse_relationship(
    value: &EtfValue,
    path: &str,
    diagnostics: &mut ReadyDiagnostics,
) -> Option<Relationship> {
    let id = value::unsigned(value.get("id")?)?;
    let user_id = value::unsigned(value.get("user_id")?)?;
    let relation_type = value::integer(value.get("type")?)?;
    Some(Relationship {
        id,
        user_id,
        relation_type,
        nickname: text_at(value, "nickname", path, diagnostics),
        since: text_at(value, "since", path, diagnostics),
        is_spam_request: bool_at(value, "is_spam_request", path, diagnostics),
        user_ignored: bool_at(value, "user_ignored", path, diagnostics),
    })
}

/// `d.relationships`: битые элементы → `malformed` (`relationships[i]`), дубликаты — last wins.
pub(crate) fn parse_relationships(
    value: Option<&EtfValue>,
    diagnostics: &mut ReadyDiagnostics,
) -> Vec<Relationship> {
    let Some(value) = value else {
        return Vec::new();
    };
    if value::is_null(value) {
        return Vec::new();
    }
    let Some(items) = value::items(value) else {
        diagnostics.ignored.push("d.relationships".to_string());
        return Vec::new();
    };

    let mut relationships = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        match parse_relationship(item, &format!("d.relationships[{index}]"), diagnostics) {
            Some(relationship) => relationships.push(relationship),
            None => diagnostics
                .malformed
                .push(format!("relationships[{index}]")),
        }
    }
    value::dedup_last_by(
        relationships,
        |relationship| relationship.id,
        |relationship| relationship.id.to_string(),
        "relationships",
        diagnostics,
    )
}

/// `client_info` сессии; отсутствие/не-map → дефолт (+ `ignored` для не-map).
fn parse_client_info(
    value: Option<&EtfValue>,
    path: &str,
    diagnostics: &mut ReadyDiagnostics,
) -> ClientInfo {
    let Some(info) = value else {
        return ClientInfo::default();
    };
    if value::is_null(info) {
        return ClientInfo::default();
    }
    if value::object(info).is_none() {
        diagnostics.ignored.push(path.to_string());
        return ClientInfo::default();
    }
    ClientInfo {
        client: text_at(info, "client", path, diagnostics),
        os: text_at(info, "os", path, diagnostics),
        version: int_at(info, "version", path, diagnostics).unwrap_or(0),
    }
}

/// Разобрать сессию; обязателен `session_id` (DESIGN §2.3).
fn parse_session(
    value: &EtfValue,
    path: &str,
    diagnostics: &mut ReadyDiagnostics,
) -> Option<Session> {
    let session_id = value::text(value.get("session_id")?)?;
    Some(Session {
        session_id: session_id.to_string(),
        status: text_at(value, "status", path, diagnostics),
        client_info: parse_client_info(
            value.get("client_info"),
            &format!("{path}.client_info"),
            diagnostics,
        ),
        processed_at_timestamp: int_at(value, "processed_at_timestamp", path, diagnostics)
            .unwrap_or(0),
    })
}

/// `d.sessions`: битые элементы → `malformed` (`sessions[i]`).
///
/// Дубликаты сессий в контракте §2.3 не перечислены — dedup не выполняется.
pub(crate) fn parse_sessions(
    value: Option<&EtfValue>,
    diagnostics: &mut ReadyDiagnostics,
) -> Vec<Session> {
    let Some(value) = value else {
        return Vec::new();
    };
    if value::is_null(value) {
        return Vec::new();
    }
    let Some(items) = value::items(value) else {
        diagnostics.ignored.push("d.sessions".to_string());
        return Vec::new();
    };

    let mut sessions = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        match parse_session(item, &format!("d.sessions[{index}]"), diagnostics) {
            Some(session) => sessions.push(session),
            None => diagnostics.malformed.push(format!("sessions[{index}]")),
        }
    }
    sessions
}

/// Разобрать участника; обязателен `user_id` (DESIGN §2.3).
fn parse_member(
    value: &EtfValue,
    path: &str,
    diagnostics: &mut ReadyDiagnostics,
) -> Option<GuildMember> {
    let user_id = value::unsigned(value.get("user_id")?)?;
    Some(parse_member_fields(value, user_id, path, diagnostics))
}

/// Поля участника при уже известном `user_id`.
///
/// Используется P2-01 (`merged_members`, где id лежит в `user_id`) и P2-04
/// (`GUILD_MEMBER_LIST_UPDATE`, где id лежит во вложенном `user.id`). Отдельный helper
/// не меняет политику P2-01: обязательность id остаётся у вызывающего.
pub(crate) fn parse_member_fields(
    value: &EtfValue,
    user_id: u64,
    path: &str,
    diagnostics: &mut ReadyDiagnostics,
) -> GuildMember {
    GuildMember {
        user_id,
        nick: text_at(value, "nick", path, diagnostics),
        avatar: text_at(value, "avatar", path, diagnostics),
        banner: text_at(value, "banner", path, diagnostics),
        joined_at: text_at(value, "joined_at", path, diagnostics),
        premium_since: text_at(value, "premium_since", path, diagnostics),
        communication_disabled_until: text_at(
            value,
            "communication_disabled_until",
            path,
            diagnostics,
        ),
        deaf: bool_at(value, "deaf", path, diagnostics),
        mute: bool_at(value, "mute", path, diagnostics),
        pending: bool_at(value, "pending", path, diagnostics),
        flags: int_at(value, "flags", path, diagnostics).unwrap_or(0),
        roles: ids_at(value, "roles", path, diagnostics),
    }
}

/// `d.merged_members`: группа `i` прикрепляется к `guilds[i].members` композицией
/// `Ready::parse`; здесь группы только разбираются, порядок и длина сохраняются.
///
/// - битая группа (не список и не `nil`) → `malformed` (`merged_members[i]`) и пустая
///   группа-заглушка, чтобы выравнивание `i ↔ guilds[i]` не сдвинулось;
/// - битый участник → `malformed` (`merged_members[i][j]`);
/// - дубликат `user_id` в группе → last wins, `duplicates` (`merged_members[i].members:<id>`);
/// - неверный тип всей коллекции → пусто + `ignored` (`d.merged_members`).
pub(crate) fn parse_merged_members(
    value: Option<&EtfValue>,
    diagnostics: &mut ReadyDiagnostics,
) -> Vec<Vec<GuildMember>> {
    let Some(value) = value else {
        return Vec::new();
    };
    if value::is_null(value) {
        return Vec::new();
    }
    let Some(groups) = value::items(value) else {
        diagnostics.ignored.push("d.merged_members".to_string());
        return Vec::new();
    };

    let mut merged = Vec::with_capacity(groups.len());
    for (group_index, group) in groups.iter().enumerate() {
        if value::is_null(group) {
            merged.push(Vec::new());
            continue;
        }
        let Some(members) = value::items(group) else {
            diagnostics
                .malformed
                .push(format!("merged_members[{group_index}]"));
            merged.push(Vec::new());
            continue;
        };

        let mut parsed = Vec::with_capacity(members.len());
        for (member_index, member) in members.iter().enumerate() {
            match parse_member(
                member,
                &format!("d.merged_members[{group_index}][{member_index}]"),
                diagnostics,
            ) {
                Some(member) => parsed.push(member),
                None => diagnostics
                    .malformed
                    .push(format!("merged_members[{group_index}][{member_index}]")),
            }
        }
        let collection = format!("merged_members[{group_index}].members");
        merged.push(value::dedup_last_by(
            parsed,
            |member| member.user_id,
            |member| member.user_id.to_string(),
            &collection,
            diagnostics,
        ));
    }
    merged
}
