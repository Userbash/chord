//! READY: гильдии, каналы, роли, эмодзи, стикеры (RW-12 / P2-01a).
//!
//! Типы и сигнатуры заморожены `waves/RW-12/DESIGN-P2-01.md` §3.3–3.4. Парсер наполняет
//! W12a; до него заглушка возвращает пустой список, чтобы W12b/W12c могли тестироваться
//! независимо. Битые элементы коллекции пропускаются с диагностикой `malformed`.

use std::collections::HashMap;

use chord_codec::etf::EtfValue;

use super::ReadyDiagnostics;
use super::social::GuildMember;
use super::value;

/// Гильдия READY.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Guild {
    /// Снежинка.
    pub id: u64,
    /// Недоступна (lazy-загрузка); в capture не встречалась.
    pub unavailable: bool,
    /// Имя: top-level `name` → `properties.name`.
    pub name: Option<String>,
    /// Иконка: `properties.icon`.
    pub icon: Option<String>,
    /// Владелец: `properties.owner_id`.
    pub owner_id: Option<u64>,
    /// `data_mode` (`full`).
    pub data_mode: Option<String>,
    /// `joined_at`.
    pub joined_at: Option<String>,
    /// `large`.
    pub large: bool,
    /// `lazy`.
    pub lazy: bool,
    /// `member_count` (в фикстуре крупной гильдии — 24 228).
    pub member_count: Option<i64>,
    /// `premium_subscription_count`.
    pub premium_subscription_count: Option<i64>,
    /// `version` (целое-строка).
    pub version: Option<u64>,
    /// Каналы.
    pub channels: Vec<Channel>,
    /// Роли.
    pub roles: Vec<Role>,
    /// Эмодзи.
    pub emojis: Vec<Emoji>,
    /// Стикеры.
    pub stickers: Vec<Sticker>,
    /// Участники из `merged_members[i]`.
    pub members: Vec<GuildMember>,
}

/// Канал гильдии.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Channel {
    /// Снежинка.
    pub id: u64,
    /// `type` (wire).
    pub channel_type: i64,
    /// Имя.
    pub name: Option<String>,
    /// Родительская категория.
    pub parent_id: Option<u64>,
    /// Позиция.
    pub position: i64,
    /// Флаги.
    pub flags: i64,
    /// NSFW.
    pub nsfw: bool,
    /// Тема.
    pub topic: Option<String>,
    /// Последнее сообщение.
    pub last_message_id: Option<u64>,
    /// Последний пин.
    pub last_pin_timestamp: Option<String>,
    /// Перезаписи прав.
    pub permission_overwrites: Vec<PermissionOverwrite>,
    /// Rate limit.
    pub rate_limit_per_user: Option<i64>,
    /// Битрейт.
    pub bitrate: Option<i64>,
    /// Лимит пользователей.
    pub user_limit: Option<i64>,
    /// RTC-регион.
    pub rtc_region: Option<String>,
    /// Качество видео.
    pub video_quality_mode: Option<i64>,
    /// Автоархивация.
    pub default_auto_archive_duration: Option<i64>,
}

/// Перезапись прав канала.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PermissionOverwrite {
    /// Снежинка роли/пользователя.
    pub id: u64,
    /// 0 — роль, 1 — участник.
    pub target_type: i64,
    /// Разрешённые биты (decimal-строка).
    pub allow: u64,
    /// Запрещённые биты.
    pub deny: u64,
}

/// Роль.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Role {
    /// Снежинка.
    pub id: u64,
    /// Имя.
    pub name: Option<String>,
    /// Цвет.
    pub color: i64,
    /// Показывать отдельно.
    pub hoist: bool,
    /// Управляемая.
    pub managed: bool,
    /// Упоминаемая.
    pub mentionable: bool,
    /// Права (decimal-строка).
    pub permissions: u64,
    /// Позиция.
    pub position: i64,
    /// Флаги.
    pub flags: i64,
    /// Unicode-эмодзи роли.
    pub unicode_emoji: Option<String>,
}

/// Эмодзи.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Emoji {
    /// Снежинка.
    pub id: u64,
    /// Имя.
    pub name: Option<String>,
    /// Анимированный.
    pub animated: bool,
    /// Доступный.
    pub available: bool,
    /// Управляемый.
    pub managed: bool,
    /// Требует двоеточий.
    pub require_colons: bool,
    /// Роли, которым доступен.
    pub roles: Vec<u64>,
}

/// Стикер.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sticker {
    /// Снежинка.
    pub id: u64,
    /// Имя.
    pub name: Option<String>,
    /// Описание.
    pub description: Option<String>,
    /// Теги.
    pub tags: Option<String>,
    /// Asset.
    pub asset: Option<String>,
    /// Формат.
    pub format_type: i64,
    /// Доступный.
    pub available: bool,
    /// Гильдия.
    pub guild_id: Option<u64>,
    /// Тип стикера.
    pub sticker_type: i64,
}

/// Результат разбора `d.guilds`: гильдии + карта «сырой индекс → итоговый индекс».
///
/// Карта нужна для `merged_members[i]`: группы привязаны к сырым индексам, а после
/// пропуска битых гильдий и last-wins-дедупликации индексы сдвигаются.
pub(crate) struct ParsedGuilds {
    /// Гильдии в порядке первого вхождения.
    pub(crate) guilds: Vec<Guild>,
    /// Для каждого сырого элемента `guilds[i]` — индекс в `guilds` или `None`.
    pub(crate) raw_positions: Vec<Option<usize>>,
}

/// Полный разбор `d.guilds`: битые элементы пропускаются (`malformed`), дубликаты id —
/// last wins с позицией первого вхождения (`duplicates`).
pub(crate) fn parse_guilds(value: &EtfValue, diagnostics: &mut ReadyDiagnostics) -> ParsedGuilds {
    let items = value::items(value).unwrap_or(&[]);
    let mut parsed = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        if let Some(guild) = parse_guild(item, index, diagnostics) {
            parsed.push((index, guild));
        }
    }

    let mut guilds: Vec<Guild> = Vec::with_capacity(parsed.len());
    let mut raw_positions = vec![None; items.len()];
    let mut positions: HashMap<u64, usize> = HashMap::new();
    for (raw_index, guild) in parsed {
        if let Some(&position) = positions.get(&guild.id) {
            // last wins: значение последней гильдии, позиция первого вхождения.
            guilds[position] = guild;
            for slot in &mut raw_positions {
                if *slot == Some(position) {
                    *slot = None;
                }
            }
            raw_positions[raw_index] = Some(position);
            diagnostics
                .duplicates
                .push(format!("guilds:{}", guilds[position].id));
        } else {
            positions.insert(guild.id, guilds.len());
            raw_positions[raw_index] = Some(guilds.len());
            guilds.push(guild);
        }
    }

    ParsedGuilds {
        guilds,
        raw_positions,
    }
}

/// Одна гильдия; `None` — битый элемент (не map или невалидный `id`).
fn parse_guild(item: &EtfValue, index: usize, diagnostics: &mut ReadyDiagnostics) -> Option<Guild> {
    let path = format!("guilds[{index}]");
    if value::object(item).is_none() {
        diagnostics.malformed.push(path);
        return None;
    }
    let Some(id) = item.get("id").and_then(value::unsigned) else {
        diagnostics.malformed.push(path);
        return None;
    };

    let properties = match item.get("properties") {
        None => None,
        Some(properties) if value::is_null(properties) => None,
        Some(properties) if value::object(properties).is_some() => Some(properties),
        Some(_) => {
            diagnostics.ignored.push(format!("d.{path}.properties"));
            None
        }
    };
    // §7.5: top-level name/icon, затем properties; в capture только properties.
    let name = text_field(item, "name", &path, diagnostics).or_else(|| {
        properties.and_then(|properties| {
            text_field(
                properties,
                "name",
                &format!("{path}.properties"),
                diagnostics,
            )
        })
    });
    let icon = text_field(item, "icon", &path, diagnostics).or_else(|| {
        properties.and_then(|properties| {
            text_field(
                properties,
                "icon",
                &format!("{path}.properties"),
                diagnostics,
            )
        })
    });
    let owner_id = properties.and_then(|properties| {
        uint_field(
            properties,
            "owner_id",
            &format!("{path}.properties"),
            diagnostics,
        )
    });

    let mut channels = Vec::new();
    for (channel_index, channel) in list_field(item, "channels", &path, diagnostics)
        .iter()
        .enumerate()
    {
        if let Some(channel) = parse_channel(channel, index, channel_index, diagnostics) {
            channels.push(channel);
        }
    }
    let channels = value::dedup_last_by(
        channels,
        |channel| channel.id,
        |channel| channel.id.to_string(),
        &format!("guilds:{id}.channels"),
        diagnostics,
    );

    let mut roles = Vec::new();
    for (role_index, role) in list_field(item, "roles", &path, diagnostics)
        .iter()
        .enumerate()
    {
        if let Some(role) = parse_role(role, index, role_index, diagnostics) {
            roles.push(role);
        }
    }
    let roles = value::dedup_last_by(
        roles,
        |role| role.id,
        |role| role.id.to_string(),
        &format!("guilds:{id}.roles"),
        diagnostics,
    );

    let mut emojis = Vec::new();
    for (emoji_index, emoji) in list_field(item, "emojis", &path, diagnostics)
        .iter()
        .enumerate()
    {
        if let Some(emoji) = parse_emoji(emoji, index, emoji_index, diagnostics) {
            emojis.push(emoji);
        }
    }
    let emojis = value::dedup_last_by(
        emojis,
        |emoji| emoji.id,
        |emoji| emoji.id.to_string(),
        &format!("guilds:{id}.emojis"),
        diagnostics,
    );

    let mut stickers = Vec::new();
    for (sticker_index, sticker) in list_field(item, "stickers", &path, diagnostics)
        .iter()
        .enumerate()
    {
        if let Some(sticker) = parse_sticker(sticker, index, sticker_index, diagnostics) {
            stickers.push(sticker);
        }
    }
    let stickers = value::dedup_last_by(
        stickers,
        |sticker| sticker.id,
        |sticker| sticker.id.to_string(),
        &format!("guilds:{id}.stickers"),
        diagnostics,
    );

    Some(Guild {
        id,
        unavailable: bool_field(item, "unavailable", &path, diagnostics),
        name,
        icon,
        owner_id,
        data_mode: text_field(item, "data_mode", &path, diagnostics),
        joined_at: text_field(item, "joined_at", &path, diagnostics),
        large: bool_field(item, "large", &path, diagnostics),
        lazy: bool_field(item, "lazy", &path, diagnostics),
        member_count: int_field(item, "member_count", &path, diagnostics),
        premium_subscription_count: int_field(
            item,
            "premium_subscription_count",
            &path,
            diagnostics,
        ),
        version: uint_field(item, "version", &path, diagnostics),
        channels,
        roles,
        emojis,
        stickers,
        members: Vec::new(),
    })
}

/// Канал; `None` — битый элемент (не map или нет `id`/`type`).
fn parse_channel(
    item: &EtfValue,
    guild_index: usize,
    index: usize,
    diagnostics: &mut ReadyDiagnostics,
) -> Option<Channel> {
    let path = format!("guilds[{guild_index}].channels[{index}]");
    parse_channel_value(item, &path, diagnostics)
}

/// Разобрать канал по произвольному пути (P2-02 `CHANNEL_INFO`; P2-01 `guilds[..]`).
///
/// Поведение идентично прежнему `parse_channel`: `None` — не map или нет `id`/`type`.
pub(crate) fn parse_channel_value(
    item: &EtfValue,
    path: &str,
    diagnostics: &mut ReadyDiagnostics,
) -> Option<Channel> {
    if value::object(item).is_none() {
        diagnostics.malformed.push(path.to_string());
        return None;
    }
    let Some(id) = item.get("id").and_then(value::unsigned) else {
        diagnostics.malformed.push(path.to_string());
        return None;
    };
    let Some(channel_type) = item.get("type").and_then(value::integer) else {
        diagnostics.malformed.push(path.to_string());
        return None;
    };

    Some(Channel {
        id,
        channel_type,
        name: text_field(item, "name", path, diagnostics),
        parent_id: uint_field(item, "parent_id", path, diagnostics),
        position: int_field(item, "position", path, diagnostics).unwrap_or(0),
        flags: int_field(item, "flags", path, diagnostics).unwrap_or(0),
        nsfw: bool_field(item, "nsfw", path, diagnostics),
        topic: text_field(item, "topic", path, diagnostics),
        last_message_id: uint_field(item, "last_message_id", path, diagnostics),
        last_pin_timestamp: text_field(item, "last_pin_timestamp", path, diagnostics),
        permission_overwrites: parse_overwrites(
            item.get("permission_overwrites"),
            path,
            diagnostics,
        ),
        rate_limit_per_user: int_field(item, "rate_limit_per_user", path, diagnostics),
        bitrate: int_field(item, "bitrate", path, diagnostics),
        user_limit: int_field(item, "user_limit", path, diagnostics),
        rtc_region: text_field(item, "rtc_region", path, diagnostics),
        video_quality_mode: int_field(item, "video_quality_mode", path, diagnostics),
        default_auto_archive_duration: int_field(
            item,
            "default_auto_archive_duration",
            path,
            diagnostics,
        ),
    })
}

/// Перезаписи прав канала; элемент без валидного `id` пропускается.
fn parse_overwrites(
    value: Option<&EtfValue>,
    path: &str,
    diagnostics: &mut ReadyDiagnostics,
) -> Vec<PermissionOverwrite> {
    let Some(value) = value else {
        return Vec::new();
    };
    if value::is_null(value) {
        return Vec::new();
    }
    let Some(items) = value::items(value) else {
        diagnostics
            .ignored
            .push(format!("d.{path}.permission_overwrites"));
        return Vec::new();
    };

    let mut overwrites = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let item_path = format!("{path}.permission_overwrites[{index}]");
        if value::object(item).is_none() {
            diagnostics.malformed.push(item_path);
            continue;
        }
        let Some(id) = item.get("id").and_then(value::unsigned) else {
            diagnostics.malformed.push(item_path);
            continue;
        };
        overwrites.push(PermissionOverwrite {
            id,
            target_type: int_field(item, "type", &item_path, diagnostics).unwrap_or(0),
            allow: uint_field(item, "allow", &item_path, diagnostics).unwrap_or(0),
            deny: uint_field(item, "deny", &item_path, diagnostics).unwrap_or(0),
        });
    }
    overwrites
}

/// Роль; `None` — битый элемент (не map или невалидный `id`).
fn parse_role(
    item: &EtfValue,
    guild_index: usize,
    index: usize,
    diagnostics: &mut ReadyDiagnostics,
) -> Option<Role> {
    let path = format!("guilds[{guild_index}].roles[{index}]");
    if value::object(item).is_none() {
        diagnostics.malformed.push(path);
        return None;
    }
    let Some(id) = item.get("id").and_then(value::unsigned) else {
        diagnostics.malformed.push(path);
        return None;
    };

    Some(Role {
        id,
        name: text_field(item, "name", &path, diagnostics),
        color: int_field(item, "color", &path, diagnostics).unwrap_or(0),
        hoist: bool_field(item, "hoist", &path, diagnostics),
        managed: bool_field(item, "managed", &path, diagnostics),
        mentionable: bool_field(item, "mentionable", &path, diagnostics),
        permissions: uint_field(item, "permissions", &path, diagnostics).unwrap_or(0),
        position: int_field(item, "position", &path, diagnostics).unwrap_or(0),
        flags: int_field(item, "flags", &path, diagnostics).unwrap_or(0),
        unicode_emoji: text_field(item, "unicode_emoji", &path, diagnostics),
    })
}

/// Эмодзи; `None` — битый элемент (не map или невалидный `id`).
fn parse_emoji(
    item: &EtfValue,
    guild_index: usize,
    index: usize,
    diagnostics: &mut ReadyDiagnostics,
) -> Option<Emoji> {
    let path = format!("guilds[{guild_index}].emojis[{index}]");
    if value::object(item).is_none() {
        diagnostics.malformed.push(path);
        return None;
    }
    let Some(id) = item.get("id").and_then(value::unsigned) else {
        diagnostics.malformed.push(path);
        return None;
    };

    Some(Emoji {
        id,
        name: text_field(item, "name", &path, diagnostics),
        animated: bool_field(item, "animated", &path, diagnostics),
        available: bool_field(item, "available", &path, diagnostics),
        managed: bool_field(item, "managed", &path, diagnostics),
        require_colons: bool_field(item, "require_colons", &path, diagnostics),
        roles: id_list_field(item, "roles", &path, diagnostics),
    })
}

/// Стикер; `None` — битый элемент (не map или невалидный `id`).
fn parse_sticker(
    item: &EtfValue,
    guild_index: usize,
    index: usize,
    diagnostics: &mut ReadyDiagnostics,
) -> Option<Sticker> {
    let path = format!("guilds[{guild_index}].stickers[{index}]");
    if value::object(item).is_none() {
        diagnostics.malformed.push(path);
        return None;
    }
    let Some(id) = item.get("id").and_then(value::unsigned) else {
        diagnostics.malformed.push(path);
        return None;
    };

    Some(Sticker {
        id,
        name: text_field(item, "name", &path, diagnostics),
        description: text_field(item, "description", &path, diagnostics),
        tags: text_field(item, "tags", &path, diagnostics),
        asset: text_field(item, "asset", &path, diagnostics),
        format_type: int_field(item, "format_type", &path, diagnostics).unwrap_or(0),
        available: bool_field(item, "available", &path, diagnostics),
        guild_id: uint_field(item, "guild_id", &path, diagnostics),
        sticker_type: int_field(item, "type", &path, diagnostics).unwrap_or(0),
    })
}

/// Текстовое поле: отсутствие/`nil` → `None`; неверный тип → `None` + `ignored`.
fn text_field(
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
            diagnostics.ignored.push(format!("d.{path}.{key}"));
            None
        }
    }
}

/// Целое поле (`i64`): отсутствие/`nil` → `None`; неверный тип → `None` + `ignored`.
fn int_field(
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
            diagnostics.ignored.push(format!("d.{path}.{key}"));
            None
        }
    }
}

/// Беззнаковое поле (ID/права/версия): отсутствие/`nil` → `None`; неверный тип → `ignored`.
fn uint_field(
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
        Some(number) => Some(number),
        None => {
            diagnostics.ignored.push(format!("d.{path}.{key}"));
            None
        }
    }
}

/// Bool-поле: отсутствие/`nil` → `false`; неверный тип → `false` + `ignored`.
fn bool_field(item: &EtfValue, key: &str, path: &str, diagnostics: &mut ReadyDiagnostics) -> bool {
    let Some(raw) = item.get(key) else {
        return false;
    };
    if value::is_null(raw) {
        return false;
    }
    match value::boolean(raw) {
        Some(flag) => flag,
        None => {
            diagnostics.ignored.push(format!("d.{path}.{key}"));
            false
        }
    }
}

/// Список: отсутствие/`nil` → пусто; неверный тип → пусто + `ignored`.
fn list_field<'a>(
    item: &'a EtfValue,
    key: &str,
    path: &str,
    diagnostics: &mut ReadyDiagnostics,
) -> &'a [EtfValue] {
    let Some(raw) = item.get(key) else {
        return &[];
    };
    if value::is_null(raw) {
        return &[];
    }
    match value::items(raw) {
        Some(items) => items,
        None => {
            diagnostics.ignored.push(format!("d.{path}.{key}"));
            &[]
        }
    }
}

/// Список снежинок (`emoji.roles`): `nil` → пусто; битый элемент — `ignored`.
fn id_list_field(
    item: &EtfValue,
    key: &str,
    path: &str,
    diagnostics: &mut ReadyDiagnostics,
) -> Vec<u64> {
    let mut ids = Vec::new();
    for (index, raw) in list_field(item, key, path, diagnostics).iter().enumerate() {
        match value::unsigned(raw) {
            Some(id) => ids.push(id),
            None => diagnostics.ignored.push(format!("d.{path}.{key}[{index}]")),
        }
    }
    ids
}
