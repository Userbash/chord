//! P2-01: находки независимого ревью (`waves/reviews/review-p2-01.md`).
//!
//! Контракт: атом-строки (`full`, `normal`, `invisible`, `desktop`, `linux`) — это текст;
//! `from_dispatch` с не-map `d` — `NotReady`; неверные типы вложенных полей — `ignored`;
//! `merged_members` привязывается по сырому индексу гильдии, а не по индексу после фильтрации;
//! пути `ignored` начинаются с `d.`.
//!
//! Все проверки идут через публичный `Ready::parse`/`from_dispatch` (P-2).

use discord_codec::etf::EtfValue;
use discord_core::ready::{Ready, ReadyError};

fn atom(value: &str) -> EtfValue {
    EtfValue::Atom(value.to_string())
}

fn text(value: &str) -> EtfValue {
    EtfValue::Str(value.to_string())
}

fn int(value: i64) -> EtfValue {
    EtfValue::Int(value)
}

fn map(pairs: Vec<(&str, EtfValue)>) -> EtfValue {
    EtfValue::Map(
        pairs
            .into_iter()
            .map(|(key, value)| (atom(key), value))
            .collect(),
    )
}

fn list(values: Vec<EtfValue>) -> EtfValue {
    EtfValue::List(values)
}

fn minimal_d(extra: Vec<(&str, EtfValue)>) -> EtfValue {
    let mut pairs = vec![
        ("user", map(vec![("id", int(1))])),
        ("session_id", text("s")),
        ("guilds", list(Vec::new())),
    ];
    for (key, value) in extra {
        if let Some(slot) = pairs.iter_mut().find(|(name, _)| *name == key) {
            slot.1 = value;
        } else {
            pairs.push((key, value));
        }
    }
    map(pairs)
}

fn frame(d: EtfValue) -> EtfValue {
    map(vec![("op", int(0)), ("t", atom("READY")), ("d", d)])
}

/// P1-1: строки, закодированные атомами, читаются как текст.
#[test]
fn r1_atom_strings_are_text() {
    let guild = map(vec![
        ("id", int(10)),
        ("data_mode", atom("full")),
        ("channels", list(Vec::new())),
        ("roles", list(Vec::new())),
        ("emojis", list(Vec::new())),
        ("stickers", list(Vec::new())),
    ]);
    let session = map(vec![
        ("session_id", text("sess")),
        ("status", atom("invisible")),
        (
            "client_info",
            map(vec![
                ("client", atom("desktop")),
                ("os", atom("linux")),
                ("version", int(0)),
            ]),
        ),
        ("processed_at_timestamp", int(0)),
    ]);
    let d = minimal_d(vec![
        ("session_type", atom("normal")),
        ("country_code", atom("US")),
        ("subdivision_code", atom("CA")),
        ("guilds", list(vec![guild])),
        ("sessions", list(vec![session])),
    ]);

    let ready = Ready::parse(&d).expect("атом-строки не должны теряться");
    assert_eq!(ready.session_type.as_deref(), Some("normal"));
    assert_eq!(ready.country_code.as_deref(), Some("US"));
    assert_eq!(ready.subdivision_code.as_deref(), Some("CA"));
    assert_eq!(ready.guilds[0].data_mode.as_deref(), Some("full"));
    assert_eq!(ready.sessions[0].status.as_deref(), Some("invisible"));
    assert_eq!(
        ready.sessions[0].client_info.client.as_deref(),
        Some("desktop")
    );
    assert_eq!(ready.sessions[0].client_info.os.as_deref(), Some("linux"));
    assert_eq!(ready.diagnostics, Default::default());
}

/// P2-1/P2-4: `from_dispatch` с не-map `d` — `NotReady`; payload-парсер — `WrongType`.
#[test]
fn r2_non_map_d_is_not_ready() {
    assert_eq!(
        Ready::from_dispatch(&frame(int(5))),
        Err(ReadyError::NotReady)
    );
    assert_eq!(
        Ready::parse(&int(5)),
        Err(ReadyError::WrongType {
            path: "d".to_string(),
            expected: "map",
            found: "int",
        })
    );
}

/// P2-2: неверные типы необязательных полей вложенных объектов попадают в `ignored`.
#[test]
fn r3_nested_wrong_types_are_ignored() {
    let channel = map(vec![
        ("id", int(20)),
        ("type", int(1)),
        ("flags", text("oops")),
        ("recipient_ids", list(vec![text("bad-id")])),
    ]);
    let member = map(vec![("user_id", int(100)), ("flags", text("oops"))]);
    let d = minimal_d(vec![
        ("private_channels", list(vec![channel])),
        ("merged_members", list(vec![list(vec![member])])),
        ("guilds", list(vec![map(vec![("id", int(1))])])),
    ]);

    let ready = Ready::parse(&d).expect("неверные необязательные поля — не фатальны");
    let ignored = &ready.diagnostics.ignored;
    assert!(
        ignored.contains(&"d.private_channels[0].flags".to_string()),
        "ignored: {ignored:?}"
    );
    assert!(
        ignored.contains(&"d.private_channels[0].recipient_ids[0]".to_string()),
        "ignored: {ignored:?}"
    );
    assert!(
        ignored.contains(&"d.merged_members[0][0].flags".to_string()),
        "ignored: {ignored:?}"
    );
}

/// P2-3: группа `merged_members` привязывается к сырому индексу гильдии.
#[test]
fn r4_merged_members_keep_raw_guild_index() {
    let d = minimal_d(vec![
        ("guilds", list(vec![int(5), map(vec![("id", int(2))])])),
        (
            "merged_members",
            list(vec![
                list(vec![map(vec![("user_id", int(100))])]),
                list(vec![map(vec![("user_id", int(200))])]),
            ]),
        ),
    ]);

    let ready = Ready::parse(&d).expect("битая гильдия не фатальна");
    assert_eq!(ready.guilds.len(), 1);
    assert_eq!(ready.guilds[0].id, 2);
    assert_eq!(ready.guilds[0].members.len(), 1);
    assert_eq!(
        ready.guilds[0].members[0].user_id, 200,
        "группа пропущенной гильдии не должна приклеиваться к следующей"
    );
}

/// P3-1: явный пустой `merged_members` — расхождение длин, а не отсутствие.
#[test]
fn r5_explicit_empty_merged_members_is_malformed() {
    let d = minimal_d(vec![
        (
            "guilds",
            list(vec![map(vec![("id", int(1))]), map(vec![("id", int(2))])]),
        ),
        ("merged_members", list(Vec::new())),
    ]);

    let ready = Ready::parse(&d).expect("пустой список не фатален");
    assert!(
        ready
            .diagnostics
            .malformed
            .iter()
            .any(|path| path.starts_with("merged_members:0 vs guilds:2")),
        "malformed: {:?}",
        ready.diagnostics.malformed
    );
}

/// P3-4: для дубликата гильдии побеждает группа последнего сырого вхождения,
/// группа предыдущего — `malformed`.
#[test]
fn r7_duplicate_guild_keeps_last_member_group() {
    let d = minimal_d(vec![
        (
            "guilds",
            list(vec![
                map(vec![("id", int(1)), ("name", text("first"))]),
                map(vec![("id", int(1)), ("name", text("second"))]),
            ]),
        ),
        (
            "merged_members",
            list(vec![
                list(vec![map(vec![("user_id", int(100))])]),
                list(vec![map(vec![("user_id", int(200))])]),
            ]),
        ),
    ]);

    let ready = Ready::parse(&d).expect("дубликат гильдии не фатален");
    assert_eq!(ready.guilds.len(), 1);
    assert_eq!(ready.guilds[0].name.as_deref(), Some("second"));
    assert_eq!(ready.guilds[0].members.len(), 1);
    assert_eq!(ready.guilds[0].members[0].user_id, 200);
    assert!(
        ready
            .diagnostics
            .duplicates
            .contains(&"guilds:1".to_string()),
        "duplicates: {:?}",
        ready.diagnostics.duplicates
    );
    assert!(
        ready
            .diagnostics
            .malformed
            .contains(&"merged_members[0]".to_string()),
        "malformed: {:?}",
        ready.diagnostics.malformed
    );
}

/// P3-2: все `ignored`-пути вложенных полей начинаются с `d.`.
#[test]
fn r6_ignored_paths_use_d_prefix() {
    let d = minimal_d(vec![(
        "guilds",
        list(vec![map(vec![
            ("id", int(1)),
            (
                "channels",
                list(vec![map(vec![
                    ("id", int(3)),
                    ("type", int(0)),
                    ("position", text("x")),
                ])]),
            ),
        ])]),
    )]);

    let ready = Ready::parse(&d).expect("битое поле канала не фатально");
    assert!(
        ready
            .diagnostics
            .ignored
            .contains(&"d.guilds[0].channels[0].position".to_string()),
        "ignored: {:?}",
        ready.diagnostics.ignored
    );
}
