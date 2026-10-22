//! READY: типизированный разбор (P2-01, T-DATA-01…03).
//!
//! **Оракул**: `docs/re/gateway-schema.md` §3 — реальный READY 40 ключей, 7 гильдий,
//! 272 канала, 132 роли, 157 read_state, 60 relationships; семантика полей — там же.
//! Синтетические термы повторяют **форму захвата** (atom-ключи, `Atom("nil")`, `Atom("false")`,
//! bignum-id), а не текущую реализацию. Реального сокета нет.

use discord_codec::etf::EtfValue;
use discord_core::ready_data::{
    ReadyData, ReadyParseError, ReadyPrivateChannel, ReadyReadStateEntry, ReadyUser,
};

fn atom(name: &str) -> EtfValue {
    EtfValue::Atom(name.to_string())
}

fn text(value: &str) -> EtfValue {
    EtfValue::Str(value.to_string())
}

fn int(value: i64) -> EtfValue {
    EtfValue::Int(value)
}

fn list(items: Vec<EtfValue>) -> EtfValue {
    EtfValue::List(items)
}

fn map(pairs: Vec<(&str, EtfValue)>) -> EtfValue {
    EtfValue::Map(
        pairs
            .into_iter()
            .map(|(key, value)| (atom(key), value))
            .collect(),
    )
}

fn map_owned(pairs: Vec<(String, EtfValue)>) -> EtfValue {
    EtfValue::Map(
        pairs
            .into_iter()
            .map(|(key, value)| (atom(&key), value))
            .collect(),
    )
}

/// Полный READY-`d`: 40 ключей, вложенные гильдия/канал/роль/эмодзи/стикер, ЛС,
/// relationships, read_state, sessions, merged_members.
fn full_data() -> EtfValue {
    let channel = map(vec![
        ("id", text("10")),
        ("name", text("general")),
        ("type", int(0)),
        ("parent_id", text("1")),
        ("position", int(2)),
        ("topic", text("hello")),
    ]);
    let role = map(vec![
        ("id", text("20")),
        ("name", text("moderator")),
        ("color", int(255)),
        ("position", int(1)),
        ("permissions", text("1024")),
        ("hoist", atom("true")),
        ("managed", atom("false")),
        ("mentionable", atom("true")),
    ]);
    let emoji = map(vec![
        ("id", text("30")),
        ("name", text("cat")),
        ("animated", atom("false")),
        ("available", atom("true")),
    ]);
    let sticker = map(vec![
        ("id", text("40")),
        ("name", text("wave")),
        ("description", text("wave sticker")),
        ("available", atom("true")),
        ("format_type", int(1)),
    ]);
    let guild = map(vec![
        ("id", text("-1")),
        ("name", text("Test Guild")),
        ("unavailable", atom("false")),
        ("member_count", int(157)),
        ("channels", list(vec![channel])),
        ("roles", list(vec![role])),
        ("emojis", list(vec![emoji])),
        ("stickers", list(vec![sticker])),
    ]);
    let own_user = map(vec![
        ("id", int(-6_147_072_113_468_506_112)),
        ("username", text("self")),
        ("discriminator", text("0")),
        ("global_name", text("Self")),
        ("avatar", atom("nil")),
        ("bot", atom("false")),
    ]);
    let friend = map(vec![
        ("id", text("200")),
        ("username", text("friend")),
        ("bot", atom("true")),
    ]);
    let private = map(vec![
        ("id", text("300")),
        ("type", int(1)),
        ("recipient_ids", list(vec![text("200")])),
        ("last_message_id", text("400")),
        ("flags", int(0)),
    ]);
    let relationship = map(vec![
        ("id", text("200")),
        ("type", int(1)),
        ("nickname", text("buddy")),
    ]);
    let read_entry = |id: &str, mention_count: i64| {
        map(vec![
            ("id", text(id)),
            ("last_message_id", text("500")),
            ("mention_count", int(mention_count)),
            ("flags", int(0)),
            ("last_viewed", int(4274)),
        ])
    };
    let session = map(vec![
        ("session_id", text("session-1")),
        ("status", text("invisible")),
        (
            "client_info",
            map(vec![
                ("client", text("desktop")),
                ("os", text("linux")),
                ("version", int(0)),
            ]),
        ),
    ]);
    let member = map(vec![
        ("user_id", text("200")),
        ("nick", atom("nil")),
        ("roles", list(vec![text("20")])),
        ("joined_at", text("2026-09-11T12:29:43.951000+00:00")),
        ("deaf", atom("false")),
        ("mute", atom("false")),
        ("pending", atom("false")),
        ("premium_since", atom("nil")),
    ]);

    let mut pairs = vec![
        ("guilds".to_string(), list(vec![guild])),
        ("user".to_string(), own_user),
        ("users".to_string(), list(vec![friend])),
        ("private_channels".to_string(), list(vec![private])),
        ("relationships".to_string(), list(vec![relationship])),
        (
            "read_state".to_string(),
            map(vec![
                (
                    "entries",
                    list(vec![read_entry("10", 3), read_entry("300", 0)]),
                ),
                ("partial", atom("false")),
                ("version", int(7)),
            ]),
        ),
        ("sessions".to_string(), list(vec![session])),
        ("merged_members".to_string(), list(vec![list(vec![member])])),
    ];
    // Добить до 40 верхнеуровневых ключей, как в захвате; неизвестные не должны ломать разбор.
    let mut index = 0;
    while pairs.len() < 40 {
        pairs.push((format!("unknown_{index}"), EtfValue::Nil));
        index += 1;
    }
    map_owned(pairs)
}

fn full_dispatch() -> EtfValue {
    map(vec![
        ("op", int(0)),
        ("t", atom("READY")),
        ("s", int(1)),
        ("d", full_data()),
    ])
}

#[test]
fn full_ready_parses_40_keys_and_nested() {
    let ready = ReadyData::from_dispatch(&full_dispatch()).expect("READY разбирается");
    assert_eq!(ready.top_level_keys.len(), 40);
    assert!(ready.top_level_keys.iter().any(|key| key == "guilds"));
    assert!(ready.top_level_keys.iter().any(|key| key == "unknown_0"));

    assert_eq!(ready.guilds.len(), 1);
    let guild = &ready.guilds[0];
    assert_eq!(guild.id, "-1");
    assert_eq!(guild.name.as_deref(), Some("Test Guild"));
    assert!(!guild.unavailable);
    assert_eq!(guild.member_count, Some(157));
    assert_eq!(guild.channels.len(), 1);
    assert_eq!(guild.channels[0].kind, Some(0));
    assert_eq!(guild.channels[0].parent_id.as_deref(), Some("1"));
    assert_eq!(guild.channels[0].topic.as_deref(), Some("hello"));
    assert_eq!(guild.roles.len(), 1);
    assert_eq!(guild.roles[0].permissions.as_deref(), Some("1024"));
    assert!(guild.roles[0].hoist);
    assert!(!guild.roles[0].managed);
    assert_eq!(guild.emojis.len(), 1);
    assert!(guild.emojis[0].available);
    assert_eq!(guild.stickers[0].format_type, Some(1));

    assert_eq!(
        ready.user.as_ref().map(|user| user.id.as_str()),
        Some("-6147072113468506112")
    );
    assert_eq!(
        ready.user.as_ref().and_then(|user| user.avatar.as_deref()),
        None
    );
    assert_eq!(ready.users.len(), 1);
    assert!(ready.users[0].bot);

    assert_eq!(ready.private_channels.len(), 1);
    let private: &ReadyPrivateChannel = &ready.private_channels[0];
    assert_eq!(private.kind, 1);
    assert_eq!(private.recipient_ids, vec!["200".to_string()]);
    assert_eq!(private.last_message_id.as_deref(), Some("400"));

    assert_eq!(ready.relationships.len(), 1);
    assert_eq!(ready.relationships[0].kind, 1);
    assert_eq!(ready.relationships[0].nickname.as_deref(), Some("buddy"));

    assert_eq!(ready.read_state.entries.len(), 2);
    let entry: &ReadyReadStateEntry = &ready.read_state.entries[0];
    assert_eq!(entry.id, "10");
    assert_eq!(entry.mention_count, 3);
    assert!(!ready.read_state.partial);
    assert_eq!(ready.read_state.version, 7);

    assert_eq!(ready.sessions.len(), 1);
    assert_eq!(ready.sessions[0].client.as_deref(), Some("desktop"));
    assert_eq!(ready.sessions[0].os.as_deref(), Some("linux"));
    assert_eq!(ready.sessions[0].version, Some(0));

    assert_eq!(ready.merged_members.len(), 1);
    assert_eq!(ready.merged_members[0][0].user_id, "200");
    assert_eq!(ready.merged_members[0][0].roles, vec!["20".to_string()]);
}

#[test]
fn minimal_ready_has_defaults_and_only_guilds_required() {
    let data = map(vec![("guilds", list(Vec::new()))]);
    let ready = ReadyData::from_data(&data).expect("минимальный READY");
    assert_eq!(ready.top_level_keys, vec!["guilds".to_string()]);
    assert!(ready.user.is_none());
    assert!(ready.users.is_empty());
    assert!(ready.guilds.is_empty());
    assert!(ready.private_channels.is_empty());
    assert!(ready.relationships.is_empty());
    assert!(ready.read_state.entries.is_empty());
    assert!(!ready.read_state.partial);
    assert_eq!(ready.read_state.version, 0);
    assert!(ready.sessions.is_empty());
    assert!(ready.merged_members.is_empty());
}

#[test]
fn from_dispatch_rejects_non_ready() {
    for value in [
        map(vec![
            ("op", int(1)),
            ("t", atom("READY")),
            ("d", map(Vec::new())),
        ]),
        map(vec![
            ("op", int(0)),
            ("t", atom("MESSAGE_CREATE")),
            ("d", map(Vec::new())),
        ]),
        map(vec![("op", int(0)), ("t", atom("READY"))]),
    ] {
        assert_eq!(
            ReadyData::from_dispatch(&value),
            Err(ReadyParseError::NotReady)
        );
    }
}

#[test]
fn missing_guilds_is_error() {
    let error =
        ReadyData::from_data(&map(vec![("users", list(Vec::new()))])).expect_err("нет guilds");
    assert_eq!(
        error,
        ReadyParseError::Missing {
            path: "guilds".to_string()
        }
    );
}

#[test]
fn wrong_guilds_type_is_error() {
    let error = ReadyData::from_data(&map(vec![("guilds", int(1))])).expect_err("guilds не массив");
    assert_eq!(
        error,
        ReadyParseError::WrongType {
            path: "guilds".to_string(),
            expected: "array"
        }
    );
}

#[test]
fn missing_guild_id_reports_element_path() {
    let guild = map(vec![("name", text("without id"))]);
    let error =
        ReadyData::from_data(&map(vec![("guilds", list(vec![guild]))])).expect_err("id обязателен");
    assert_eq!(
        error,
        ReadyParseError::Missing {
            path: "guilds[0].id".to_string()
        }
    );
}

#[test]
fn atom_nil_is_absent_and_int_bool_is_accepted() {
    let user = map(vec![
        ("id", text("1")),
        ("avatar", atom("nil")),
        ("bot", int(0)),
    ]);
    let ready = ReadyData::from_data(&map(vec![("guilds", list(Vec::new())), ("user", user)]))
        .expect("user");
    let user: &ReadyUser = ready.user.as_ref().expect("user");
    assert_eq!(user.avatar, None);
    assert!(!user.bot);

    let data = map(vec![
        ("guilds", list(Vec::new())),
        (
            "read_state",
            map(vec![("partial", int(1)), ("version", int(9))]),
        ),
    ]);
    let ready = ReadyData::from_data(&data).expect("read_state");
    assert!(ready.read_state.partial);
    assert_eq!(ready.read_state.version, 9);
}

#[test]
fn unknown_fields_are_ignored_but_counted() {
    let data = map(vec![
        ("guilds", list(Vec::new())),
        ("future_field", map(vec![("nested", list(vec![int(1)]))])),
        ("another", atom("nil")),
    ]);
    let ready = ReadyData::from_data(&data).expect("forward compat");
    assert_eq!(ready.top_level_keys.len(), 3);
    assert!(ready.top_level_keys.contains(&"future_field".to_string()));
}

#[test]
fn wrong_user_type_reports_path() {
    let error = ReadyData::from_data(&map(vec![
        ("guilds", list(Vec::new())),
        ("users", list(vec![int(1)])),
    ]))
    .expect_err("users[0] не map");
    assert_eq!(
        error,
        ReadyParseError::Missing {
            path: "users[0].id".to_string()
        }
    );
}
