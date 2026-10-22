//! W18a / P2-09a: типы и разбор presence.
//!
//! Оракул — реальные `PRESENCE_UPDATE`/`READY_SUPPLEMENTAL` фикстуры и официальный
//! веб-клиент (`waves/RW-18/RESEARCH-P2-09.md` §1–§2). Синтетика проверяет
//! optional-поля, диагностику и границы.
//!
//! Тесты падают до реализации presence-типов — это compile-red.

mod support;

use std::collections::BTreeSet;

use discord_core::codec::etf::EtfValue;
use discord_core::state::{EventError, EventPayload, StateDiagnostics};
use discord_core::{
    Activity, ActivityType, Event, MergedPresences, Presence, PresenceKey, PresenceScope,
    PresenceStatus, PresenceUpdate, State,
};
use support::state_fixture::{atom, frame, int, list, load, map, minimal_ready_frame, text};

fn parse(frame: &EtfValue) -> (PresenceUpdate, StateDiagnostics) {
    let event = Event::from_frame(frame).expect("PRESENCE_UPDATE должен разбираться");
    match event.payload {
        EventPayload::PresenceUpdate(update) => (update, event.diagnostics),
        other => panic!("ожидался PresenceUpdate, получено {other:?}"),
    }
}

fn parse_err(frame: &EtfValue) -> EventError {
    Event::from_frame(frame).expect_err("кадр должен быть отвергнут")
}

fn user(id: &str) -> EtfValue {
    map(vec![("id", text(id)), ("username", text("user"))])
}

fn update_frame(seq: u64, pairs: Vec<(&str, EtfValue)>) -> EtfValue {
    frame("PRESENCE_UPDATE", seq, map(pairs))
}

fn activity(name: &str, activity_type: i64) -> EtfValue {
    map(vec![("name", text(name)), ("type", int(activity_type))])
}

/// a01: фикстура `010` — все подтверждённые поля, диагностика пуста.
#[test]
fn a01_fixture_010_fields_and_empty_diagnostics() {
    let (update, diagnostics) = parse(&load("010-PRESENCE_UPDATE.json"));
    let presence = &update.presence;
    assert_eq!(presence.user_id, 95_101_520_944_570_368);
    assert_eq!(presence.guild_id, Some(1_014_537_851_418_980_372));
    assert_eq!(
        presence.scope(),
        PresenceScope::Guild(1_014_537_851_418_980_372)
    );
    assert_eq!(presence.status, PresenceStatus::Online);
    assert_eq!(
        presence.client_status.get("desktop"),
        Some(&PresenceStatus::Online)
    );
    assert_eq!(presence.activities.len(), 1);
    let activity = &presence.activities[0];
    assert_eq!(activity.name, "bg-capture");
    assert_eq!(activity.activity_type, ActivityType::Playing);
    assert_eq!(
        activity
            .timestamps
            .as_ref()
            .and_then(|timestamps| timestamps.start),
        Some(1_792_391_357_883)
    );
    assert_eq!(presence.processed_at_timestamp, Some(1_792_391_357_994));
    assert!(diagnostics.malformed.is_empty());
    assert!(diagnostics.ignored.is_empty());
}

/// a02: фикстура `043` — custom status, dnd и emoji.
#[test]
fn a02_fixture_043_custom_status() {
    let (update, diagnostics) = parse(&load("043-PRESENCE_UPDATE.json"));
    let presence = &update.presence;
    assert_eq!(presence.status, PresenceStatus::Dnd);
    assert_eq!(presence.activities.len(), 1);
    let activity = &presence.activities[0];
    assert_eq!(activity.activity_type, ActivityType::Custom);
    assert_eq!(activity.name, "Custom Status");
    assert_eq!(activity.state.as_deref(), Some("f1an#9271 | f_1an"));
    assert_eq!(
        activity
            .emoji
            .as_ref()
            .and_then(|emoji| emoji.name.as_deref()),
        Some("🇷🇺")
    );
    assert!(diagnostics.malformed.is_empty());
}

/// a03: фикстура `012` — приватный scope и полный user.
#[test]
fn a03_fixture_012_private_scope_and_full_user() {
    let (update, diagnostics) = parse(&load("012-PRESENCE_UPDATE.json"));
    assert_eq!(update.presence.guild_id, None);
    assert_eq!(update.presence.scope(), PresenceScope::Private);
    assert_eq!(update.presence.status, PresenceStatus::Idle);
    assert!(update.presence.activities.is_empty());
    let user = update.user.expect("user должен сохраняться");
    assert_eq!(user.id, 275_258_435_199_827_968);
    assert_eq!(user.username.as_deref(), Some("dpakc"));
    assert_eq!(user.global_name.as_deref(), Some("ДраКс-Иван"));
    assert!(diagnostics.malformed.is_empty());
}

/// a04: фикстура `018` — mobile online + vr idle, processed timestamp 0.
#[test]
fn a04_fixture_018_client_platforms() {
    let (update, _) = parse(&load("018-PRESENCE_UPDATE.json"));
    assert_eq!(update.presence.status, PresenceStatus::Online);
    assert_eq!(
        update.presence.client_status.get("mobile"),
        Some(&PresenceStatus::Online)
    );
    assert_eq!(
        update.presence.client_status.get("vr"),
        Some(&PresenceStatus::Idle)
    );
    assert_eq!(update.presence.processed_at_timestamp, Some(0));
}

/// a05: минимальный synthetic — optional-поля дают дефолты без диагностики.
#[test]
fn a05_minimal_presence_defaults() {
    let frame = update_frame(1, vec![("user", user("100")), ("status", text("online"))]);
    let (update, diagnostics) = parse(&frame);
    assert_eq!(update.presence.user_id, 100);
    assert_eq!(update.presence.guild_id, None);
    assert!(update.presence.client_status.is_empty());
    assert!(update.presence.activities.is_empty());
    assert_eq!(update.presence.processed_at_timestamp, None);
    assert_eq!(update.presence.since, None);
    assert_eq!(update.presence.broadcast, None);
    assert!(diagnostics.ignored.is_empty());
}

/// a06: неверные optional-типы не фатальны: дефолт + ignored.
#[test]
fn a06_wrong_optional_types_are_ignored() {
    let frame = update_frame(
        1,
        vec![
            ("user", user("100")),
            ("status", text("online")),
            ("client_status", text("bad")),
            ("activities", text("bad")),
            ("processed_at_timestamp", text("not-a-number")),
            ("broadcast", text("bad")),
        ],
    );
    let (update, diagnostics) = parse(&frame);
    assert!(update.presence.client_status.is_empty());
    assert!(update.presence.activities.is_empty());
    assert_eq!(update.presence.processed_at_timestamp, None);
    assert_eq!(update.presence.broadcast, None);
    for path in [
        "presence.client_status",
        "presence.activities",
        "presence.processed_at_timestamp",
        "presence.broadcast",
    ] {
        assert!(
            diagnostics.ignored.iter().any(|entry| entry == path),
            "нет {path} в {:?}",
            diagnostics.ignored
        );
    }
}

/// a07: без `d.user` — Missing.
#[test]
fn a07_missing_user_is_fatal() {
    let frame = update_frame(1, vec![("status", text("online"))]);
    assert!(matches!(
        parse_err(&frame),
        EventError::Missing { ref path } if path == "presence.user"
    ));
}

/// a08: user без валидного id — InvalidId.
#[test]
fn a08_invalid_user_id_is_fatal() {
    let frame = update_frame(
        1,
        vec![
            ("user", map(vec![("id", int(-1))])),
            ("status", text("online")),
        ],
    );
    match parse_err(&frame) {
        EventError::InvalidId { path, value } => {
            assert_eq!(path, "presence.user.id");
            assert_eq!(value, "-1");
        }
        other => panic!("ожидался InvalidId, получено {other:?}"),
    }
}

/// a18: нулевой guild_id не является валидной снежинкой.
#[test]
fn a18_zero_guild_id_is_ignored() {
    let frame = update_frame(
        1,
        vec![
            ("user", user("100")),
            ("guild_id", text("0")),
            ("status", text("online")),
        ],
    );
    let (update, diagnostics) = parse(&frame);
    assert_eq!(update.presence.guild_id, None);
    assert_eq!(update.presence.scope(), PresenceScope::Private);
    assert!(
        diagnostics
            .ignored
            .iter()
            .any(|entry| entry == "presence.guild_id"),
        "нет диагностики guild_id=0: {:?}",
        diagnostics.ignored
    );
}

/// a09: `d` не map — WrongType.
#[test]
fn a09_data_not_map_is_fatal() {
    let frame = frame("PRESENCE_UPDATE", 1, list(Vec::new()));
    assert!(matches!(
        parse_err(&frame),
        EventError::WrongType { ref path, .. } if path == "d"
    ));
}

/// a10: activities null/валидные/битые элементы.
#[test]
fn a10_activities_null_and_bad_items() {
    let mut pairs = vec![("user", user("100")), ("status", text("online"))];
    pairs.push(("activities", atom("nil")));
    let (update, diagnostics) = parse(&update_frame(1, pairs));
    assert!(update.presence.activities.is_empty());
    assert!(diagnostics.ignored.is_empty());

    let frame = update_frame(
        2,
        vec![
            ("user", user("100")),
            ("status", text("online")),
            (
                "activities",
                list(vec![
                    activity("game", 0),
                    text("not-an-object"),
                    map(vec![("name", text("no-type"))]),
                    map(vec![("type", int(0))]),
                ]),
            ),
        ],
    );
    let (update, diagnostics) = parse(&frame);
    assert_eq!(update.presence.activities.len(), 1);
    assert_eq!(update.presence.activities[0].name, "game");
    assert!(
        diagnostics
            .ignored
            .iter()
            .any(|entry| entry == "presence.activities[1]")
    );
    assert!(
        diagnostics
            .ignored
            .iter()
            .any(|entry| entry == "presence.activities[2].type")
    );
    assert!(
        diagnostics
            .ignored
            .iter()
            .any(|entry| entry == "presence.activities[3].name")
    );
}

/// a11: client_status — валидные пары, пустые/неверные игнорируются.
#[test]
fn a11_client_status_diagnostics() {
    let frame = update_frame(
        1,
        vec![
            ("user", user("100")),
            ("status", text("online")),
            (
                "client_status",
                map(vec![
                    ("desktop", text("online")),
                    ("", text("idle")),
                    ("web", text("")),
                    ("mobile", int(1)),
                ]),
            ),
        ],
    );
    let (update, diagnostics) = parse(&frame);
    assert_eq!(update.presence.client_status.len(), 1);
    assert_eq!(
        update.presence.client_status.get("desktop"),
        Some(&PresenceStatus::Online)
    );
    assert!(
        diagnostics
            .ignored
            .iter()
            .any(|entry| entry.contains("empty"))
    );
    assert!(
        diagnostics
            .ignored
            .iter()
            .any(|entry| entry == "presence.client_status.web")
    );
    assert!(
        diagnostics
            .ignored
            .iter()
            .any(|entry| entry == "presence.client_status.mobile")
    );
}

/// a12: timestamp-поля принимают int/десятичную строку, в т.ч. отрицательную.
#[test]
fn a12_timestamps_and_broadcast() {
    let frame = update_frame(
        1,
        vec![
            ("user", user("100")),
            ("status", text("online")),
            ("processed_at_timestamp", text("1792391486569")),
            ("since", int(-5)),
            ("broadcast", atom("true")),
        ],
    );
    let (update, _) = parse(&frame);
    assert_eq!(
        update.presence.processed_at_timestamp,
        Some(1_792_391_486_569)
    );
    assert_eq!(update.presence.since, Some(-5));
    assert_eq!(update.presence.broadcast, Some(true));

    let frame = update_frame(
        2,
        vec![
            ("user", user("100")),
            ("status", text("online")),
            ("processed_at_timestamp", text("-42")),
        ],
    );
    let (update, _) = parse(&frame);
    assert_eq!(update.presence.processed_at_timestamp, Some(-42));
}

/// a13: неизвестный status сохраняется, а не теряется.
#[test]
fn a13_unknown_status_is_preserved() {
    let frame = update_frame(1, vec![("user", user("100")), ("status", text("away"))]);
    let (update, _) = parse(&frame);
    assert_eq!(
        update.presence.status,
        PresenceStatus::Unknown("away".to_string())
    );
    assert_eq!(update.presence.status.as_str(), "away");
    assert!(!update.presence.status.is_online());
}

/// a14: rich activity — отрицательный application_id, party.size байтами, assets/emoji.
#[test]
fn a14_rich_activity_fields() {
    let rich = map(vec![
        ("type", int(0)),
        ("name", text("ARC Raiders")),
        ("details", text("В Сперанце")),
        ("state", text("state")),
        ("url", text("https://example.test")),
        ("id", text("activity-id")),
        ("platform", text("desktop")),
        ("created_at", text("1792391826510")),
        ("application_id", text("-1236281867793082368")),
        ("parent_application_id", text("-9510227338491611136")),
        (
            "timestamps",
            map(vec![("start", text("100")), ("end", int(200))]),
        ),
        (
            "assets",
            map(vec![
                ("large_image", text("large")),
                ("large_text", text("Large")),
                ("small_image", text("small")),
                ("small_text", text("Small")),
            ]),
        ),
        (
            "party",
            map(vec![("id", text("party")), ("size", text("\u{1}\u{3}"))]),
        ),
        (
            "emoji",
            map(vec![
                ("name", text("☕")),
                ("id", text("123")),
                ("animated", atom("true")),
            ]),
        ),
        ("buttons", list(vec![text("View game"), text("Join")])),
        ("supported_platforms", list(vec![text("desktop")])),
        ("flags", int(1026)),
        ("status_display_type", int(2)),
    ]);
    let frame = update_frame(
        1,
        vec![
            ("user", user("100")),
            ("status", text("online")),
            ("activities", list(vec![rich])),
        ],
    );
    let (update, diagnostics) = parse(&frame);
    assert!(diagnostics.ignored.is_empty(), "{:?}", diagnostics.ignored);
    let activity = &update.presence.activities[0];
    assert_eq!(
        activity.application_id.as_deref(),
        Some("-1236281867793082368")
    );
    assert_eq!(
        activity.parent_application_id.as_deref(),
        Some("-9510227338491611136")
    );
    assert_eq!(activity.created_at, Some(1_792_391_826_510));
    let timestamps = activity.timestamps.as_ref().expect("timestamps");
    assert_eq!(timestamps.start, Some(100));
    assert_eq!(timestamps.end, Some(200));
    let assets = activity.assets.as_ref().expect("assets");
    assert_eq!(assets.large_image.as_deref(), Some("large"));
    assert_eq!(assets.small_text.as_deref(), Some("Small"));
    let party = activity.party.as_ref().expect("party");
    assert_eq!(party.id.as_deref(), Some("party"));
    assert_eq!(party.current_size, Some(1));
    assert_eq!(party.max_size, Some(3));
    let emoji = activity.emoji.as_ref().expect("emoji");
    assert_eq!(emoji.name.as_deref(), Some("☕"));
    assert_eq!(emoji.id.as_deref(), Some("123"));
    assert_eq!(emoji.animated, Some(true));
    assert_eq!(activity.buttons, vec!["View game", "Join"]);
    assert_eq!(activity.supported_platforms, vec!["desktop"]);
    assert_eq!(activity.flags, Some(1026));
    assert_eq!(activity.status_display_type, Some(2));
}

/// a17: party.size принимает list-форму [current, max].
#[test]
fn a17_party_size_list_fallback() {
    let rich = map(vec![
        ("name", text("list-party")),
        ("type", int(0)),
        (
            "party",
            map(vec![
                ("id", text("party")),
                ("size", list(vec![int(1), int(3)])),
            ]),
        ),
    ]);
    let frame = update_frame(
        1,
        vec![
            ("user", user("101")),
            ("status", text("online")),
            ("activities", list(vec![rich])),
        ],
    );
    let (update, diagnostics) = parse(&frame);
    assert!(
        diagnostics.ignored.is_empty(),
        "неожиданная диагностика: {:?}",
        diagnostics.ignored
    );
    let party = update.presence.activities[0].party.as_ref().expect("party");
    assert_eq!(party.current_size, Some(1));
    assert_eq!(party.max_size, Some(3));
}

/// a15: READY_SUPPLEMENTAL — 2 friends, 7 guild-групп, 28 scope, 18 users.
#[test]
fn a15_supplemental_merged_presences_fixture() {
    let event = Event::from_frame(&load("003-READY_SUPPLEMENTAL.json")).expect("supplemental");
    let EventPayload::ReadySupplemental(supplemental) = event.payload else {
        panic!("ожидался ReadySupplemental");
    };
    let guild_ids = supplemental.guild_ids.clone();
    let merged = supplemental.merged_presences;
    assert_eq!(merged.friends.len(), 2);
    assert_eq!(merged.guilds.len(), 7);
    assert_eq!(
        merged.guilds.iter().map(Vec::len).collect::<Vec<_>>(),
        vec![4, 4, 3, 0, 1, 14, 0]
    );
    for (index, group) in merged.guilds.iter().enumerate() {
        assert!(
            group
                .iter()
                .all(|presence| presence.guild_id == guild_ids[index]),
            "группа {index} не выровнена по guild_ids: {:?} / {:?}",
            guild_ids[index],
            group
                .iter()
                .map(|presence| presence.guild_id)
                .collect::<Vec<_>>()
        );
    }
    assert!(
        merged
            .friends
            .iter()
            .all(|presence| presence.scope() == PresenceScope::Private)
    );
    let guild_presences = merged.guilds.iter().flatten().collect::<Vec<_>>();
    assert_eq!(guild_presences.len(), 26);
    assert_eq!(guild_presences.len() + merged.friends.len(), 28);
    let users = guild_presences
        .into_iter()
        .chain(merged.friends.iter())
        .map(|presence| presence.user_id)
        .collect::<BTreeSet<_>>();
    assert_eq!(users.len(), 18);
    assert!(event.diagnostics.malformed.is_empty());
}

/// a16: битый merged-снимок диагностируется, State::from_ready пуст, root re-exports.
#[test]
fn a16_merged_diagnostics_empty_state_and_root_exports() {
    let ready = Event::from_frame(&minimal_ready_frame()).expect("READY");
    let EventPayload::Ready(ready) = ready.payload else {
        panic!("ожидался Ready");
    };
    let state = State::from_ready(&ready);
    assert!(state.presences.is_empty());

    let merged = map(vec![
        ("friends", list(vec![map(vec![("status", text("online"))])])),
        (
            "guilds",
            list(vec![
                list(vec![map(vec![
                    ("user_id", text("100")),
                    ("status", text("online")),
                ])]),
                list(vec![map(vec![
                    ("user_id", text("200")),
                    ("status", text("idle")),
                ])]),
            ]),
        ),
    ]);
    let frame = frame(
        "READY_SUPPLEMENTAL",
        2,
        map(vec![
            ("guilds", list(vec![map(vec![("id", text("1"))])])),
            ("merged_members", list(vec![list(Vec::new())])),
            ("merged_presences", merged),
        ]),
    );
    let event = Event::from_frame(&frame).expect("supplemental");
    let EventPayload::ReadySupplemental(supplemental) = event.payload else {
        panic!("ожидался ReadySupplemental");
    };
    assert_eq!(supplemental.merged_presences.friends.len(), 0);
    assert_eq!(supplemental.merged_presences.guilds.len(), 1);
    assert!(
        event
            .diagnostics
            .malformed
            .iter()
            .any(|entry| entry.contains("merged_presences"))
    );

    let _status = PresenceStatus::Online;
    let _activity_type = ActivityType::Playing;
    let _activity = Activity::default();
    let _presence = Presence::default();
    let _update = PresenceUpdate {
        presence: Presence::default(),
        user: None,
    };
    let _merged = MergedPresences::default();
    let _key: PresenceKey = (1, PresenceScope::Private);
}
