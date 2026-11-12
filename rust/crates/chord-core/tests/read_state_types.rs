//! W17a / P2-07a: типы и разбор `MESSAGE_ACK`.
//!
//! Оракул — реальная фикстура `046-MESSAGE_ACK.json` и замороженный контракт
//! `waves/RW-17/DESIGN-P2-07.md` §2–§3. Синтетические кейсы проверяют optional-поля,
//! диагностику `ignored` и фатальные ошибки обязательных полей.
//!
//! Тесты падают до реализации `MessageAck` — это compile-red.

mod support;

use chord_core::codec::etf::EtfValue;
use chord_core::dispatch::route_dispatch;
use chord_core::ready::Ready;
use chord_core::state::{
    Event, EventError, EventPayload, MessageAck, State, StateDiagnostics, apply_dispatch,
};
use support::state_fixture::{atom, frame, int, list, load, map, text};

fn ack_frame(seq: u64, pairs: Vec<(&str, EtfValue)>) -> EtfValue {
    frame("MESSAGE_ACK", seq, map(pairs))
}

fn parse_ack(frame: &EtfValue) -> (MessageAck, StateDiagnostics) {
    let event = Event::from_frame(frame).expect("MESSAGE_ACK должен разбираться");
    match event.payload {
        EventPayload::MessageAck(ack) => (ack, event.diagnostics),
        other => panic!("ожидался MessageAck, получено {other:?}"),
    }
}

/// a01: фикстура `046` — все подтверждённые поля, `seq=38`, диагностика пуста.
#[test]
fn a01_fixture_046_fields_and_empty_diagnostics() {
    let event = Event::from_frame(&load("046-MESSAGE_ACK.json"))
        .expect("MESSAGE_ACK 046 должен разбираться");
    let Event {
        seq,
        payload,
        diagnostics,
    } = event;
    assert_eq!(seq, Some(38));
    let EventPayload::MessageAck(ack) = payload else {
        panic!("ожидался EventPayload::MessageAck");
    };
    assert_eq!(ack.channel_id, 1_293_192_721_237_938_187);
    assert_eq!(ack.message_id, 1_561_348_877_573_947_515);
    assert_eq!(ack.flags, Some(0));
    assert_eq!(ack.last_viewed, Some(4310));
    assert_eq!(ack.mention_count, None);
    assert!(!ack.manual);
    assert_eq!(ack.version, Some(128_950));
    assert_eq!(diagnostics, StateDiagnostics::default());

    // Root re-export `chord_core::MessageAck` доступен потребителям.
    let _: Option<chord_core::MessageAck> = None;
}

/// a02: минимальные обязательные поля — optional `None`, `manual=false`.
#[test]
fn a02_minimal_ack_defaults_optional_fields() {
    let data = map(vec![
        ("channel_id", text("1293192721237938187")),
        ("message_id", text("1561348877573947515")),
    ]);
    let (ack, diagnostics) = MessageAck::parse(&data).expect("минимальный ACK валиден");
    assert_eq!(ack.channel_id, 1_293_192_721_237_938_187);
    assert_eq!(ack.message_id, 1_561_348_877_573_947_515);
    assert_eq!(ack.flags, None);
    assert_eq!(ack.last_viewed, None);
    assert_eq!(ack.mention_count, None);
    assert!(!ack.manual);
    assert_eq!(ack.version, None);
    assert_eq!(diagnostics, StateDiagnostics::default());
}

/// a03: `nil` в optional-полях — дефолты без диагностики.
#[test]
fn a03_nil_optional_fields_are_defaults_without_diagnostics() {
    let frame = ack_frame(
        12,
        vec![
            ("channel_id", text("1")),
            ("message_id", text("2")),
            ("flags", atom("nil")),
            ("last_viewed", atom("nil")),
            ("mention_count", atom("nil")),
            ("manual", atom("nil")),
            ("version", atom("nil")),
        ],
    );
    let (ack, diagnostics) = parse_ack(&frame);
    assert_eq!(ack.flags, None);
    assert_eq!(ack.last_viewed, None);
    assert_eq!(ack.mention_count, None);
    assert!(!ack.manual);
    assert_eq!(ack.version, None);
    assert_eq!(diagnostics, StateDiagnostics::default());
}

/// a04: неверные типы optional — дефолты и ровно пять записей `ignored`.
#[test]
fn a04_wrong_optional_types_are_ignored() {
    let frame = ack_frame(
        13,
        vec![
            ("channel_id", text("1")),
            ("message_id", text("2")),
            ("flags", text("not-an-int")),
            ("last_viewed", text("not-an-int")),
            ("mention_count", text("not-an-int")),
            ("manual", text("maybe")),
            ("version", text("not-a-u64")),
        ],
    );
    let (ack, diagnostics) = parse_ack(&frame);
    assert_eq!(ack.flags, None);
    assert_eq!(ack.last_viewed, None);
    assert_eq!(ack.mention_count, None);
    assert!(!ack.manual);
    assert_eq!(ack.version, None);
    assert_eq!(
        diagnostics.ignored,
        vec![
            "d.flags",
            "d.last_viewed",
            "d.mention_count",
            "d.manual",
            "d.version",
        ]
    );
    assert!(diagnostics.malformed.is_empty());
}

/// a05: `manual=true`, `mention_count=5`, `version=128954` разбираются.
#[test]
fn a05_manual_mention_and_version_parse() {
    let frame = ack_frame(
        14,
        vec![
            ("channel_id", text("1014537851867775060")),
            ("message_id", text("1561632596763353163")),
            ("manual", atom("true")),
            ("mention_count", int(5)),
            ("version", int(128_954)),
        ],
    );
    let (ack, diagnostics) = parse_ack(&frame);
    assert!(ack.manual);
    assert_eq!(ack.mention_count, Some(5));
    assert_eq!(ack.version, Some(128_954));
    assert_eq!(diagnostics, StateDiagnostics::default());
}

/// a06: отсутствие `channel_id`/`message_id` — фатальный `Missing` с путём поля.
#[test]
fn a06_missing_required_fields_are_fatal() {
    let without_channel = ack_frame(15, vec![("message_id", text("2"))]);
    let error = Event::from_frame(&without_channel).expect_err("channel_id обязателен");
    assert!(
        matches!(
            &error,
            EventError::Missing { path } if path == "message_ack.channel_id"
        ),
        "получено {error:?}"
    );

    let without_message = ack_frame(16, vec![("channel_id", text("1"))]);
    let error = Event::from_frame(&without_message).expect_err("message_id обязателен");
    assert!(
        matches!(
            &error,
            EventError::Missing { path } if path == "message_ack.message_id"
        ),
        "получено {error:?}"
    );
}

/// a07: отрицательный/дробный id — фатальный `InvalidId` с путём поля.
#[test]
fn a07_invalid_required_ids_are_fatal() {
    let negative_channel = ack_frame(17, vec![("channel_id", int(-1)), ("message_id", text("2"))]);
    let error = Event::from_frame(&negative_channel).expect_err("отрицательный channel_id");
    assert!(
        matches!(
            &error,
            EventError::InvalidId { path, .. } if path == "message_ack.channel_id"
        ),
        "получено {error:?}"
    );

    let fractional_message = ack_frame(
        18,
        vec![
            ("channel_id", text("1")),
            ("message_id", EtfValue::Float(1.5)),
        ],
    );
    let error = Event::from_frame(&fractional_message).expect_err("дробный message_id");
    assert!(
        matches!(
            &error,
            EventError::InvalidId { path, .. } if path == "message_ack.message_id"
        ),
        "получено {error:?}"
    );
}

/// a08: `d` не map — фатальный `WrongType` с путём `d`.
#[test]
fn a08_non_map_data_is_fatal() {
    let frame = frame("MESSAGE_ACK", 39, list(Vec::new()));
    let error = Event::from_frame(&frame).expect_err("d должен быть map");
    assert!(
        matches!(
            &error,
            EventError::WrongType { path, expected, found }
                if path == "d" && *expected == "map" && *found == "list"
        ),
        "получено {error:?}"
    );
}

/// a09: `Event::from_frame` маршрутизирует `MESSAGE_ACK` в `EventPayload::MessageAck`.
#[test]
fn a09_event_from_frame_routes_message_ack() {
    let frame = ack_frame(
        40,
        vec![("channel_id", text("1")), ("message_id", text("2"))],
    );
    let event = Event::from_frame(&frame).expect("MESSAGE_ACK не Unsupported");
    assert_eq!(event.seq, Some(40));
    assert!(matches!(event.payload, EventPayload::MessageAck(_)));
}

/// a10: битый ACK через `apply_dispatch` — `malformed`, read-state и версия не меняются,
/// `last_seq` монотонен.
#[test]
fn a10_broken_ack_is_malformed_without_state_change() {
    let ready = Ready::from_dispatch(&load("002-READY.json")).expect("READY 002");
    let mut state = State::from_ready(&ready);
    let entries = state.read_state.entries.len();
    let version = state.read_state.version;
    assert_eq!(entries, 157);
    assert_eq!(version, 128_949);
    state.last_seq = Some(30);

    let broken = ack_frame(38, vec![("message_id", text("1561348877573947515"))]);
    let dispatch = route_dispatch(&broken).expect("op0 MESSAGE_ACK маршрутизируется");
    let (state, effects) = apply_dispatch(state, &dispatch);

    assert!(effects.is_empty());
    assert_eq!(state.read_state.entries.len(), entries);
    assert_eq!(state.read_state.version, version);
    assert_eq!(state.last_seq, Some(38));
    assert!(
        state
            .diagnostics
            .malformed
            .iter()
            .any(|entry| entry.starts_with("event:")),
        "ожидался malformed с event:, получено {:?}",
        state.diagnostics.malformed
    );
}

/// a11: повторный ACK того же канала/сообщения с другой version — не duplicate.
#[test]
fn a11_repeated_ack_with_new_version_is_not_duplicate() {
    let ack = |seq: u64, version: i64| {
        ack_frame(
            seq,
            vec![
                ("channel_id", text("1176785373989445703")),
                ("message_id", text("1561379697990959244")),
                ("version", int(version)),
            ],
        )
    };

    let (first, first_diagnostics) = parse_ack(&ack(41, 128_951));
    let (second, second_diagnostics) = parse_ack(&ack(42, 128_953));
    assert_eq!(first.channel_id, second.channel_id);
    assert_eq!(first.message_id, second.message_id);
    assert_eq!(first.version, Some(128_951));
    assert_eq!(second.version, Some(128_953));
    assert!(first_diagnostics.duplicates.is_empty());
    assert!(second_diagnostics.duplicates.is_empty());
    assert_eq!(first_diagnostics, StateDiagnostics::default());
    assert_eq!(second_diagnostics, StateDiagnostics::default());
}

/// a12: отрицательные `flags`/`last_viewed` допустимы как i64, отрицательный `version`
/// игнорируется.
#[test]
fn a12_negative_fields_have_typed_behavior() {
    let frame = ack_frame(
        43,
        vec![
            ("channel_id", text("1")),
            ("message_id", text("2")),
            ("flags", int(-1)),
            ("last_viewed", int(-5)),
            ("version", int(-1)),
        ],
    );
    let (ack, diagnostics) = parse_ack(&frame);
    assert_eq!(ack.flags, Some(-1));
    assert_eq!(ack.last_viewed, Some(-5));
    assert_eq!(ack.version, None);
    assert_eq!(diagnostics.ignored, vec!["d.version"]);
}
