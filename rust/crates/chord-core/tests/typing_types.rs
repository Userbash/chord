//! W20a / P2-11a: типы и разбор `TYPING_START`.
//!
//! Оракул — 28 реальных `TYPING_START` из `gw-proto/msgs-run2-95974_8269.jsonl`
//! (`waves/RW-20/RESEARCH-P2-11.md` §1–§2), минимизированная копия
//! `tests/data/typing-run2.jsonl` и замороженный контракт
//! `waves/RW-20/DESIGN-P2-11.md` §3–§4. Синтетика проверяет optional-поля,
//! диагностику и форму member.
//!
//! Тесты падают до объявления `state::typing` (compile-red), это ожидаемо.

mod support;

use std::collections::BTreeMap;

use chord_core::codec::etf::EtfValue;
use chord_core::ready::value;
use chord_core::state::typing::parser::parse_typing_start;
use chord_core::state::typing::types::{TYPING_TTL_MS, TypingStart};
use chord_core::state::{Event, EventError, EventPayload, StateDiagnostics};
use support::ready_fixture::parse_json;
use support::state_fixture::{atom, frame, int, list, map, text};

const FIXTURE: &str = include_str!("data/typing-run2.jsonl");

const GUILD: u64 = 942_355_007_271_997_450;
const CHANNEL_A: u64 = 942_355_562_958_573_628;
const USER_A: u64 = 1_301_880_740_245_405_707;
const CHANNEL_B: u64 = 1_434_201_417_467_433_001;
const USER_B: u64 = 367_288_240_207_822_858;

fn member_user(id: &str, username: &str) -> EtfValue {
    map(vec![("id", text(id)), ("username", text(username))])
}

fn member(id: &str, username: &str) -> EtfValue {
    map(vec![("user", member_user(id, username))])
}

fn typing_frame(seq: u64, pairs: Vec<(&str, EtfValue)>) -> EtfValue {
    frame("TYPING_START", seq, map(pairs))
}

fn parse(data: &EtfValue, received_at_ms: Option<u64>) -> (TypingStart, StateDiagnostics) {
    parse_typing_start(data, received_at_ms).expect("TYPING_START должен разбираться")
}

fn parse_err(data: &EtfValue) -> EventError {
    parse_typing_start(data, Some(0)).expect_err("кадр должен быть отвергнут")
}

fn fixture_payloads() -> Vec<EtfValue> {
    FIXTURE
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| parse_json(line).expect("строка fixture — валидный JSON"))
        .collect()
}

/// t01: реальный idx 117 / `s=192` — точные поля первого START.
#[test]
fn t01_capture_117_fields() {
    let data = map(vec![
        ("guild_id", text("942355007271997450")),
        ("channel_id", text("942355562958573628")),
        ("member", member("1301880740245405707", "user")),
        ("timestamp", int(1_792_392_992)),
        ("user_id", text("1301880740245405707")),
    ]);

    // `s=192` — метка первого кадра capture; `idx 117` — позиция в дампе.
    let frame = frame("TYPING_START", 192, data);
    let event = Event::from_frame_at(&frame, 1_792_392_992_000).expect("from_frame_at");
    assert_eq!(event.seq, Some(192));
    let EventPayload::TypingStart(start) = event.payload else {
        panic!("ожидался TypingStart");
    };
    let diagnostics = event.diagnostics;

    assert_eq!(TYPING_TTL_MS, 10_000);
    assert_eq!(start.channel_id, CHANNEL_A);
    assert_eq!(start.user_id, USER_A);
    assert_eq!(start.guild_id, Some(GUILD));
    assert_eq!(start.wire_timestamp, Some(1_792_392_992));
    assert_eq!(start.received_at_ms, Some(1_792_392_992_000));
    let user = start.member_user.as_ref().expect("member.user");
    assert_eq!(user.id, USER_A);
    assert_eq!(user.username.as_deref(), Some("user"));
    assert!(
        diagnostics
            .ignored
            .iter()
            .all(|path| path != "typing.typing_indicator_style")
    );
    assert!(diagnostics.malformed.is_empty());
    assert!(diagnostics.inconsistencies.is_empty());
}

/// t02: все 28 строк fixture — 2 пары, 27+1, timestamp-диапазон, одна гильдия.
#[test]
fn t02_fixture_28_events_pairs_and_timestamps() {
    let payloads = fixture_payloads();
    assert_eq!(payloads.len(), 28);

    let mut pairs: BTreeMap<(u64, u64), usize> = BTreeMap::new();
    let mut timestamps = Vec::new();
    for payload in &payloads {
        let wire_timestamp = payload
            .get("timestamp")
            .and_then(value::integer)
            .expect("timestamp fixture");
        let (start, diagnostics) = parse(
            payload,
            Some(u64::try_from(wire_timestamp * 1_000).expect("ms")),
        );
        assert!(diagnostics.ignored.is_empty(), "{:?}", diagnostics.ignored);
        assert!(
            diagnostics.malformed.is_empty(),
            "{:?}",
            diagnostics.malformed
        );
        assert_eq!(start.guild_id, Some(GUILD));
        assert_eq!(start.wire_timestamp, Some(wire_timestamp));
        assert_eq!(
            start.received_at_ms,
            Some(u64::try_from(wire_timestamp * 1_000).expect("ms"))
        );
        assert!(start.member_user.is_none());
        *pairs.entry((start.channel_id, start.user_id)).or_insert(0) += 1;
        timestamps.push(wire_timestamp);
    }

    assert_eq!(pairs.len(), 2);
    assert_eq!(pairs.get(&(CHANNEL_B, USER_B)), Some(&27));
    assert_eq!(pairs.get(&(CHANNEL_A, USER_A)), Some(&1));
    assert_eq!(timestamps.iter().min(), Some(&1_792_392_992));
    assert_eq!(timestamps.iter().max(), Some(&1_792_394_710));
}

/// t03: минимальный DM-подобный `d` — optional-поля отсутствуют без диагностики.
#[test]
fn t03_minimal_dm_shape() {
    let data = map(vec![
        ("channel_id", text("942355562958573628")),
        ("user_id", text("1301880740245405707")),
    ]);
    let (start, diagnostics) = parse(&data, Some(5));

    assert_eq!(start.channel_id, CHANNEL_A);
    assert_eq!(start.user_id, USER_A);
    assert_eq!(start.guild_id, None);
    assert_eq!(start.wire_timestamp, None);
    assert_eq!(start.member_user, None);
    assert_eq!(start.received_at_ms, Some(5));
    assert!(diagnostics.ignored.is_empty());
    assert!(diagnostics.malformed.is_empty());
    assert!(diagnostics.inconsistencies.is_empty());
}

/// t04: обязательные/optional поля неверного вида → Missing/InvalidId/ignored.
#[test]
fn t04_required_and_optional_field_table() {
    // Обязательный channel_id отсутствует.
    assert_eq!(
        parse_err(&map(vec![("user_id", text("300"))])),
        EventError::Missing {
            path: "typing.channel_id".to_string()
        }
    );
    // Обязательный user_id отсутствует.
    assert_eq!(
        parse_err(&map(vec![("channel_id", text("200"))])),
        EventError::Missing {
            path: "typing.user_id".to_string()
        }
    );
    // Нечисловой и нулевой id — InvalidId.
    assert_eq!(
        parse_err(&map(vec![
            ("channel_id", text("abc")),
            ("user_id", text("300"))
        ])),
        EventError::InvalidId {
            path: "typing.channel_id".to_string(),
            value: "abc".to_string()
        }
    );
    assert_eq!(
        parse_err(&map(vec![("channel_id", int(0)), ("user_id", text("300"))])),
        EventError::InvalidId {
            path: "typing.channel_id".to_string(),
            value: "0".to_string()
        }
    );
    assert_eq!(
        parse_err(&map(vec![
            ("channel_id", text("200")),
            ("user_id", text("-1"))
        ])),
        EventError::InvalidId {
            path: "typing.user_id".to_string(),
            value: "-1".to_string()
        }
    );
    // `d` не map — фатально, но без паники.
    assert!(matches!(
        parse_err(&atom("nil")),
        EventError::WrongType { ref path, expected: "map", .. } if path == "typing"
    ));
    assert!(matches!(
        parse_err(&text("bad")),
        EventError::WrongType { ref path, expected: "map", .. } if path == "typing"
    ));

    // Optional мусор не фатален: значение отбрасывается, путь — в ignored.
    let data = map(vec![
        ("channel_id", text("200")),
        ("user_id", text("300")),
        ("guild_id", list(vec![int(1)])),
        ("timestamp", atom("true")),
        ("member", text("bad")),
        ("typing_indicator_style", int(4)),
    ]);
    let (start, diagnostics) = parse(&data, Some(0));
    assert_eq!(start.guild_id, None);
    assert_eq!(start.wire_timestamp, None);
    assert_eq!(start.member_user, None);
    for path in [
        "typing.guild_id",
        "typing.timestamp",
        "typing.member",
        "typing.typing_indicator_style",
    ] {
        assert!(
            diagnostics.ignored.iter().any(|entry| entry == path),
            "нет {path} в {:?}",
            diagnostics.ignored
        );
    }
    // Строка-int — валидный timestamp; nil — отсутствие без диагностики.
    let (with_text, text_diagnostics) = parse(
        &map(vec![
            ("channel_id", text("200")),
            ("user_id", text("300")),
            ("timestamp", text("1792392992")),
            ("guild_id", atom("nil")),
        ]),
        Some(0),
    );
    assert_eq!(with_text.wire_timestamp, Some(1_792_392_992));
    assert_eq!(with_text.guild_id, None);
    assert!(text_diagnostics.ignored.is_empty());
    // Поле style не сохраняется в модели.
    assert!(diagnostics.malformed.is_empty());
}

/// t05: member без user / mismatch; `from_frame` без часов vs `from_frame_at`.
#[test]
fn t05_member_errors_and_event_clock() {
    let missing_user = map(vec![
        ("channel_id", text("200")),
        ("user_id", text("300")),
        ("member", map(vec![("nick", text("nick"))])),
    ]);
    let (start, diagnostics) = parse(&missing_user, Some(0));
    assert_eq!(start.member_user, None);
    assert_eq!(
        diagnostics.malformed,
        vec!["typing.member.user".to_string()]
    );

    let bad_user = map(vec![
        ("channel_id", text("200")),
        ("user_id", text("300")),
        ("member", map(vec![("user", text("bad"))])),
    ]);
    let (start, diagnostics) = parse(&bad_user, Some(0));
    assert_eq!(start.member_user, None);
    assert_eq!(
        diagnostics.malformed,
        vec!["typing.member.user".to_string()]
    );

    let mismatch = map(vec![
        ("channel_id", text("200")),
        ("user_id", text("300")),
        ("member", member("301", "other")),
    ]);
    let (start, diagnostics) = parse(&mismatch, Some(0));
    assert_eq!(start.user_id, 300);
    assert_eq!(start.member_user, None);
    assert_eq!(
        diagnostics.inconsistencies,
        vec!["typing:200:300:member-user:301".to_string()]
    );

    let frame = typing_frame(
        192,
        vec![
            ("channel_id", text("200")),
            ("user_id", text("300")),
            ("member", member("300", "user")),
        ],
    );
    let event = Event::from_frame(&frame).expect("from_frame");
    let EventPayload::TypingStart(start) = event.payload else {
        panic!("ожидался TypingStart");
    };
    assert_eq!(start.received_at_ms, None);

    let event = Event::from_frame_at(&frame, 1_000_000).expect("from_frame_at");
    let EventPayload::TypingStart(start) = event.payload else {
        panic!("ожидался TypingStart");
    };
    assert_eq!(start.received_at_ms, Some(1_000_000));
    assert_eq!(start.member_user.map(|user| user.id), Some(300));
}
