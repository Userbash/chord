//! W15a / P2-04: разбор `GUILD_MEMBER_LIST_UPDATE`.
//!
//! Оракулы — реальные курированные фикстуры `008/021/034/045/079` и `jq`-числа из
//! `waves/RW-15/RESEARCH.md` §2. `005/006` — legacy big-int: отрицательный `guild_id`
//! обязан быть отвергнут, а не превращён в состояние.

mod support;

use discord_core::codec::etf::EtfValue;
use discord_core::state::{
    Event, EventError, EventPayload, MemberListEntry, MemberListGroup, MemberListOp,
    MemberListRange,
};
use support::state_fixture::{atom, frame, int, list, load, map, text};

fn parse(name: &str) -> Event {
    Event::from_frame(&load(name)).unwrap_or_else(|error| panic!("{name}: {error}"))
}

fn into_update(event: Event) -> discord_core::state::MemberListUpdate {
    match event.payload {
        EventPayload::GuildMemberListUpdate(update) => update,
        other => panic!("ожидался member list, получено {other:?}"),
    }
}

fn member_item(id: &str) -> EtfValue {
    map(vec![(
        "member",
        map(vec![
            ("avatar", atom("nil")),
            ("roles", atom("nil")),
            (
                "user",
                map(vec![("id", text(id)), ("username", text("user"))]),
            ),
        ]),
    )])
}

fn group_item(id: &str) -> EtfValue {
    map(vec![("group", map(vec![("id", text(id))]))])
}

fn list_frame(seq: u64, ops: Vec<EtfValue>) -> EtfValue {
    frame(
        "GUILD_MEMBER_LIST_UPDATE",
        seq,
        map(vec![
            ("guild_id", text("294064825540280335")),
            ("id", text("everyone")),
            ("member_count", int(112)),
            ("online_count", int(13)),
            (
                "groups",
                list(vec![map(vec![("id", text("online")), ("count", int(6))])]),
            ),
            ("ops", list(ops)),
        ]),
    )
}

fn with_positive_guild(mut value: EtfValue, guild_id: &str) -> EtfValue {
    if let EtfValue::Map(pairs) = &mut value {
        for (key, item) in pairs {
            if matches!(key, EtfValue::Atom(name) | EtfValue::Str(name) if name == "d")
                && let EtfValue::Map(fields) = item
            {
                for (key, field) in fields {
                    if matches!(key, EtfValue::Atom(name) | EtfValue::Str(name) if name == "guild_id")
                    {
                        *field = text(guild_id);
                    }
                }
            }
        }
    }
    value
}

#[test]
fn m01_parse_008_shape_and_presence() {
    let update = into_update(parse("008-GUILD_MEMBER_LIST_UPDATE.json"));

    assert_eq!(update.guild_id, 1_014_537_851_418_980_372);
    assert_eq!(update.id.as_str(), "1659606647");
    assert_eq!(update.member_count, Some(24_228));
    assert_eq!(update.online_count, Some(681));
    assert_eq!(
        update.groups,
        vec![
            MemberListGroup {
                id: "1062745847512715385".to_string(),
                count: 2,
            },
            MemberListGroup {
                id: "online".to_string(),
                count: 13,
            },
        ]
    );
    assert_eq!(update.ops.len(), 1);
    let MemberListOp::Sync { items, range } = &update.ops[0] else {
        panic!("ожидался SYNC");
    };
    assert_eq!(*range, Some(MemberListRange { start: 0, end: 99 }));
    assert_eq!(items.len(), 17);
    assert!(matches!(&items[0], MemberListEntry::Group { id } if id == "1062745847512715385"));
    let MemberListEntry::Member {
        member,
        user,
        presence,
    } = &items[1]
    else {
        panic!("ожидался member на индексе 1");
    };
    assert_eq!(member.user_id, 95_101_520_944_570_368);
    assert_eq!(user.username.as_deref(), Some("romanpony"));
    assert_eq!(presence.status.as_deref(), Some("online"));
    assert_eq!(
        presence.client_status.get("desktop").map(String::as_str),
        Some("online")
    );
}

#[test]
fn m02_parse_021_counts_groups_and_flat_items() {
    let update = into_update(parse("021-GUILD_MEMBER_LIST_UPDATE.json"));

    assert_eq!(update.guild_id, 294_064_825_540_280_335);
    assert_eq!(update.id.as_str(), "everyone");
    assert_eq!(update.member_count, Some(112));
    assert_eq!(update.online_count, Some(13));
    assert_eq!(update.groups.len(), 4);
    assert_eq!(
        update
            .groups
            .iter()
            .find(|group| group.id == "online")
            .map(|group| group.count),
        Some(6)
    );
    let MemberListOp::Sync { items, range } = &update.ops[0] else {
        panic!("ожидался SYNC");
    };
    assert_eq!(*range, Some(MemberListRange { start: 0, end: 99 }));
    assert_eq!(items.len(), 100);
    assert!(matches!(&items[0], MemberListEntry::Group { .. }));
    let MemberListEntry::Member { member, .. } = &items[1] else {
        panic!("ожидался member");
    };
    assert_eq!(member.user_id, 275_258_435_199_827_968);
}

#[test]
fn m03_parse_034_insert_and_045_ops_in_order() {
    let insert = into_update(parse("034-GUILD_MEMBER_LIST_UPDATE.json"));
    assert_eq!(insert.id.as_str(), "1659606647");
    assert_eq!(insert.ops.len(), 1);
    let MemberListOp::Insert { index, item } = &insert.ops[0] else {
        panic!("ожидался INSERT");
    };
    assert_eq!(*index, 1);
    let MemberListEntry::Member { member, user, .. } = item else {
        panic!("ожидался member");
    };
    assert_eq!(member.user_id, 294_857_804_404_228_106);
    assert_eq!(user.username.as_deref(), Some("aqua_it"));

    let update = into_update(parse("045-GUILD_MEMBER_LIST_UPDATE.json"));
    assert_eq!(update.ops.len(), 2);
    assert!(matches!(update.ops[0], MemberListOp::Delete { index: 11 }));
    let MemberListOp::Insert { index, item } = &update.ops[1] else {
        panic!("ожидался INSERT");
    };
    assert_eq!(*index, 17);
    assert!(matches!(
        item,
        MemberListEntry::Member { member, .. } if member.user_id == 270_232_759_388_078_082
    ));
}

#[test]
fn m04_parse_079_group_insert() {
    let update = into_update(parse("079-GUILD_MEMBER_LIST_UPDATE.json"));
    assert_eq!(update.ops.len(), 4);
    assert!(matches!(update.ops[0], MemberListOp::Delete { index: 99 }));
    assert!(matches!(
        update.ops[1],
        MemberListOp::Insert { index: 0, .. }
    ));
    assert!(matches!(update.ops[2], MemberListOp::Delete { index: 99 }));
    let MemberListOp::Insert { index, item } = &update.ops[3] else {
        panic!("ожидался INSERT group");
    };
    assert_eq!(*index, 0);
    assert!(matches!(
        item,
        MemberListEntry::Group { id } if id == "294152150374940673"
    ));
}

#[test]
fn m05_legacy_005_006_are_rejected_on_negative_guild_id() {
    for name in [
        "005-GUILD_MEMBER_LIST_UPDATE.json",
        "006-GUILD_MEMBER_LIST_UPDATE.json",
    ] {
        let error = Event::from_frame(&load(name)).expect_err("legacy negative guild_id");
        assert!(
            matches!(
                &error,
                EventError::InvalidId { path, .. } if path == "d.guild_id"
            ),
            "{name}: {error:?}"
        );
    }
}

#[test]
fn m06_negative_member_id_is_diagnostic_and_does_not_poison_list() {
    let frame = list_frame(
        2,
        vec![map(vec![
            ("op", text("SYNC")),
            ("range", text("\0c")),
            (
                "items",
                list(vec![group_item("online"), member_item("-123")]),
            ),
        ])],
    );
    let event = Event::from_frame(&frame).expect("заголовок валиден");
    assert_eq!(event.diagnostics.malformed.len(), 1);
    assert!(event.diagnostics.malformed[0].contains("items[1]"));
    let update = into_update(event);
    let MemberListOp::Sync { items, .. } = &update.ops[0] else {
        panic!("ожидался SYNC");
    };
    assert_eq!(items.len(), 1);
    assert!(matches!(items[0], MemberListEntry::Group { .. }));
}

#[test]
fn m07_malformed_header_is_fatal_but_not_panic() {
    let missing_guild = frame(
        "GUILD_MEMBER_LIST_UPDATE",
        2,
        map(vec![("id", text("everyone")), ("ops", list(Vec::new()))]),
    );
    assert!(matches!(
        Event::from_frame(&missing_guild),
        Err(EventError::Missing { path }) if path == "d.guild_id"
    ));

    let bad_ops = frame(
        "GUILD_MEMBER_LIST_UPDATE",
        3,
        map(vec![
            ("guild_id", text("42")),
            ("id", text("everyone")),
            ("ops", text("not-a-list")),
        ]),
    );
    assert!(matches!(
        Event::from_frame(&bad_ops),
        Err(EventError::WrongType { path, .. }) if path == "d.ops"
    ));

    let missing_id = frame(
        "GUILD_MEMBER_LIST_UPDATE",
        4,
        map(vec![("guild_id", text("42")), ("ops", list(Vec::new()))]),
    );
    assert!(matches!(
        Event::from_frame(&missing_id),
        Err(EventError::Missing { path }) if path == "d.id"
    ));

    let bad_guild = frame(
        "GUILD_MEMBER_LIST_UPDATE",
        5,
        map(vec![
            ("guild_id", text("not-a-snowflake")),
            ("id", text("everyone")),
            ("ops", list(Vec::new())),
        ]),
    );
    assert!(matches!(
        Event::from_frame(&bad_guild),
        Err(EventError::InvalidId { path, .. }) if path == "d.guild_id"
    ));

    let empty_id = frame(
        "GUILD_MEMBER_LIST_UPDATE",
        6,
        map(vec![
            ("guild_id", text("42")),
            ("id", text("")),
            ("ops", list(Vec::new())),
        ]),
    );
    assert!(matches!(
        Event::from_frame(&empty_id),
        Err(EventError::WrongType { path, .. }) if path == "d.id"
    ));
}

#[test]
fn m08_range_forms_and_invalid_range() {
    let mut items = vec![group_item("online")];
    let as_bytes = list_frame(
        2,
        vec![map(vec![
            ("op", text("SYNC")),
            ("range", text("\0c")),
            ("items", list(std::mem::take(&mut items))),
        ])],
    );
    let update = into_update(Event::from_frame(&as_bytes).expect("bytes range"));
    let MemberListOp::Sync { range, .. } = &update.ops[0] else {
        panic!("SYNC");
    };
    assert_eq!(*range, Some(MemberListRange { start: 0, end: 99 }));

    let as_list = list_frame(
        3,
        vec![map(vec![
            ("op", text("SYNC")),
            ("range", list(vec![int(0), int(5)])),
            ("items", list(vec![group_item("online")])),
        ])],
    );
    let update = into_update(Event::from_frame(&as_list).expect("list range"));
    let MemberListOp::Sync { range, .. } = &update.ops[0] else {
        panic!("SYNC");
    };
    assert_eq!(*range, Some(MemberListRange { start: 0, end: 5 }));

    let bad = list_frame(
        4,
        vec![map(vec![
            ("op", text("SYNC")),
            ("range", text("broken")),
            ("items", list(vec![group_item("online")])),
        ])],
    );
    let event = Event::from_frame(&bad).expect("range не фатален");
    assert!(
        event
            .diagnostics
            .ignored
            .iter()
            .any(|entry| entry.contains("range"))
    );
    let update = into_update(event);
    let MemberListOp::Sync { range, .. } = &update.ops[0] else {
        panic!("SYNC");
    };
    assert_eq!(*range, None);
}

#[test]
fn m09_unknown_op_is_skipped_and_other_ops_apply() {
    let frame = list_frame(
        2,
        vec![
            map(vec![("op", text("FROBNICATE")), ("index", int(0))]),
            map(vec![
                ("op", text("INSERT")),
                ("index", int(0)),
                ("item", group_item("online")),
            ]),
        ],
    );
    let event = Event::from_frame(&frame).expect("unknown op не фатален");
    assert!(
        event.diagnostics.inconsistencies.iter().any(|entry| {
            entry.contains("member_list:294064825540280335:everyone:unknown-op:FROBNICATE")
        }),
        "диагностика: {:?}",
        event.diagnostics
    );
    let update = into_update(event);
    assert_eq!(update.ops.len(), 1);
    assert!(matches!(
        update.ops[0],
        MemberListOp::Insert { index: 0, .. }
    ));
}

#[test]
fn m10_presence_missing_and_roles_null_are_safe() {
    let frame = list_frame(
        2,
        vec![map(vec![
            ("op", text("SYNC")),
            (
                "items",
                list(vec![map(vec![(
                    "member",
                    map(vec![
                        ("roles", atom("nil")),
                        ("user", map(vec![("id", text("42"))])),
                    ]),
                )])]),
            ),
        ])],
    );
    let update = into_update(Event::from_frame(&frame).expect("member без presence"));
    let MemberListOp::Sync { items, .. } = &update.ops[0] else {
        panic!("SYNC");
    };
    let MemberListEntry::Member {
        member, presence, ..
    } = &items[0]
    else {
        panic!("member");
    };
    assert!(member.roles.is_empty());
    assert_eq!(presence.status, None);
    assert!(presence.client_status.is_empty());
}

#[test]
fn m13_bad_groups_and_empty_client_status_are_diagnostic() {
    let presence_frame = list_frame(
        2,
        vec![map(vec![
            ("op", text("SYNC")),
            (
                "items",
                list(vec![map(vec![(
                    "member",
                    map(vec![
                        ("roles", atom("nil")),
                        (
                            "presence",
                            map(vec![
                                ("status", text("online")),
                                (
                                    "client_status",
                                    map(vec![
                                        ("", text("online")),
                                        ("desktop", text("")),
                                        ("web", text("online")),
                                    ]),
                                ),
                            ]),
                        ),
                        ("user", map(vec![("id", text("42"))])),
                    ]),
                )])]),
            ),
        ])],
    );
    // groups в list_frame валидны; отдельно проверяем битый groups ниже.
    let event = Event::from_frame(&presence_frame).expect("member presence");
    assert_eq!(event.diagnostics.ignored.len(), 2);
    let update = into_update(event);
    let MemberListOp::Sync { items, .. } = &update.ops[0] else {
        panic!("SYNC");
    };
    let MemberListEntry::Member { presence, .. } = &items[0] else {
        panic!("member");
    };
    assert_eq!(presence.client_status.len(), 1);
    assert_eq!(
        presence.client_status.get("web").map(String::as_str),
        Some("online")
    );

    let bad_groups = frame(
        "GUILD_MEMBER_LIST_UPDATE",
        3,
        map(vec![
            ("guild_id", text("294064825540280335")),
            ("id", text("everyone")),
            (
                "groups",
                list(vec![
                    map(vec![("id", text("online"))]),
                    map(vec![("count", int(1))]),
                    map(vec![("id", text("offline")), ("count", int(-1))]),
                ]),
            ),
            ("ops", list(Vec::new())),
        ]),
    );
    let event = Event::from_frame(&bad_groups).expect("groups не фатальны");
    assert_eq!(event.diagnostics.malformed.len(), 1);
    assert!(
        event
            .diagnostics
            .ignored
            .iter()
            .any(|entry| entry.ends_with(".count"))
    );
    let update = into_update(event);
    assert_eq!(update.groups.len(), 2);
    assert_eq!(update.groups[0].count, 0);
    assert_eq!(update.groups[1].count, 0);
}

#[test]
fn m14_bad_insert_item_has_single_malformed_diagnostic() {
    let frame = list_frame(
        2,
        vec![map(vec![
            ("op", text("INSERT")),
            ("index", int(0)),
            ("item", map(vec![("member", atom("nil"))])),
        ])],
    );
    let event = Event::from_frame(&frame).expect("bad item не фатален");
    assert_eq!(
        event.diagnostics.malformed.len(),
        1,
        "{:?}",
        event.diagnostics
    );
    assert!(event.diagnostics.malformed[0].contains("item"));
    assert!(into_update(event).ops.is_empty());
}

#[test]
fn m12_legacy_005_006_shape_with_corrected_guild_id() {
    // 005/006 — старый big-int декод. Меняем только guild_id на подтверждённый
    // corrections-документом; отрицательные member-id остаются и обязаны быть
    // диагностикой, а не типизированными участниками. Это shape/ops-проверка,
    // а не утверждение, что исходные файлы валидны.
    let frame = with_positive_guild(
        load("005-GUILD_MEMBER_LIST_UPDATE.json"),
        "1014537851418980372",
    );
    let event = Event::from_frame(&frame).expect("005 shape после correction");
    assert_eq!(event.diagnostics.malformed.len(), 98);
    let update = into_update(event);
    assert_eq!(update.guild_id, 1_014_537_851_418_980_372);
    assert_eq!(update.id.as_str(), "everyone");
    assert_eq!(update.member_count, Some(24_227));
    assert_eq!(update.online_count, Some(663));
    assert_eq!(update.groups.len(), 2);
    let MemberListOp::Sync { items, range } = &update.ops[0] else {
        panic!("SYNC");
    };
    assert_eq!(*range, Some(MemberListRange { start: 0, end: 99 }));
    assert_eq!(items.len(), 2);
    assert!(
        items
            .iter()
            .all(|item| matches!(item, MemberListEntry::Group { .. }))
    );

    let frame = with_positive_guild(
        load("006-GUILD_MEMBER_LIST_UPDATE.json"),
        "1014537851418980372",
    );
    let event = Event::from_frame(&frame).expect("006 header после correction");
    assert!(!event.diagnostics.malformed.is_empty());
    assert!(into_update(event).ops.is_empty());
}

#[test]
fn m11_root_reexports_are_available() {
    use discord_core::{
        MemberList, MemberListEntry as RootEntry, MemberListGroup as RootGroup,
        MemberListId as RootId, MemberListOp as RootOp, MemberListRange as RootRange,
        MemberListUpdate as RootUpdate, MemberPresence as RootPresence,
    };

    let id = RootId::new("everyone");
    let range = RootRange { start: 0, end: 99 };
    let group = RootGroup {
        id: "online".to_string(),
        count: 1,
    };
    let presence = RootPresence::default();
    let entry: RootEntry = RootEntry::Group {
        id: group.id.clone(),
    };
    let update = RootUpdate {
        guild_id: 1,
        id,
        member_count: Some(1),
        online_count: Some(1),
        groups: vec![group],
        ops: vec![RootOp::Sync {
            items: vec![entry],
            range: Some(range),
        }],
    };
    let list = MemberList {
        guild_id: update.guild_id,
        id: update.id.clone(),
        member_count: update.member_count,
        online_count: update.online_count,
        groups: update.groups.clone(),
        items: Vec::new(),
        range: Some(range),
    };
    assert_eq!(list.guild_id, 1);
    assert_eq!(presence.client_status.len(), 0);
}
