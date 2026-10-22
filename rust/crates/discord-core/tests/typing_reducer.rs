//! W20b / P2-11b: reducer typing-store.
//!
//! Оракул — 28 реальных `TYPING_START` (`tests/data/typing-run2.jsonl`), TTL
//! `10 000` мс из официального `TypingStore` (`waves/RW-20/RESEARCH-P2-11.md` §2.3)
//! и замороженный контракт `waves/RW-20/DESIGN-P2-11.md` §5. Время приходит только
//! из `received_at_ms`/`now_ms`; системные часы не читаются.
//!
//! Тесты падают до объявления `state::typing` и `apply_frame_at` — compile-red.

mod support;

use discord_core::codec::etf::EtfValue;
use discord_core::ready::value;
use discord_core::state::reducer::apply_frame_at;
use discord_core::state::typing::reducer::{
    apply_start, prune_expired, remove_entry, remove_for_message,
};
use discord_core::state::typing::types::{TYPING_TTL_MS, TypingStart};
use discord_core::state::{Event, EventPayload, State, apply_frame};
use support::ready_fixture::parse_json;
use support::state_fixture::{atom, frame, int, map, minimal_ready_frame, text};

const FIXTURE: &str = include_str!("data/typing-run2.jsonl");

const CHANNEL_A: u64 = 942_355_562_958_573_628;
const USER_A: u64 = 1_301_880_740_245_405_707;
const CHANNEL_B: u64 = 1_434_201_417_467_433_001;
const USER_B: u64 = 367_288_240_207_822_858;

fn minimal_state() -> State {
    let event = Event::from_frame(&minimal_ready_frame()).expect("minimal READY");
    let EventPayload::Ready(ready) = event.payload else {
        panic!("ожидался READY");
    };
    State::from_ready(&ready)
}

fn start(channel_id: u64, user_id: u64, received_at_ms: u64) -> TypingStart {
    TypingStart {
        channel_id,
        user_id,
        guild_id: None,
        wire_timestamp: None,
        member_user: None,
        received_at_ms: Some(received_at_ms),
    }
}

fn start_frame(seq: u64, channel: &str, user: &str, timestamp: i64) -> EtfValue {
    frame(
        "TYPING_START",
        seq,
        map(vec![
            ("channel_id", text(channel)),
            ("user_id", text(user)),
            ("member", member(user, "typing-user")),
            ("timestamp", int(timestamp)),
        ]),
    )
}

fn member(id: &str, username: &str) -> EtfValue {
    map(vec![(
        "user",
        map(vec![("id", text(id)), ("username", text(username))]),
    )])
}

fn message_frame(seq: u64, id: &str, channel: &str, author: &str) -> EtfValue {
    frame(
        "MESSAGE_CREATE",
        seq,
        map(vec![
            ("id", text(id)),
            ("channel_id", text(channel)),
            ("content", text("")),
            ("type", int(0)),
            ("flags", int(0)),
            ("pinned", atom("false")),
            ("tts", atom("false")),
            ("mention_everyone", atom("false")),
            (
                "author",
                map(vec![("id", text(author)), ("username", text("author"))]),
            ),
        ]),
    )
}

fn fixture_payloads() -> Vec<EtfValue> {
    FIXTURE
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| parse_json(line).expect("строка fixture — валидный JSON"))
        .collect()
}

/// t06: READY + START `(200,300)` at 1 000 000 — запись, TTL, member upsert.
#[test]
fn t06_start_creates_entry_and_upserts_member() {
    let state = minimal_state();
    let (state, _) = apply_frame_at(state, &minimal_ready_frame(), 999_000);
    let (state, effects) = apply_frame_at(state, &start_frame(2, "200", "300", 1), 1_000_000);

    let entry = state.typing.get(&(200, 300)).expect("запись typing");
    assert_eq!(entry.channel_id, 200);
    assert_eq!(entry.user_id, 300);
    assert_eq!(entry.guild_id, None);
    assert_eq!(entry.wire_timestamp, Some(1));
    assert_eq!(entry.started_at_ms, 1_000_000);
    assert_eq!(entry.expires_at_ms, 1_000_000 + TYPING_TTL_MS);
    assert_eq!(entry.expires_at_ms, 1_010_000);
    assert_eq!(state.typing.len(), 1);
    assert_eq!(
        state
            .users
            .get(&300)
            .and_then(|user| user.username.as_deref()),
        Some("typing-user")
    );
    assert!(effects.is_empty());
    assert!(state.diagnostics.ignored.is_empty());
    assert!(state.diagnostics.malformed.is_empty());
    assert!(state.diagnostics.duplicates.is_empty());
}

/// t07: повтор пары через 8 000 мс перезапускает TTL, duplicates не пишется.
#[test]
fn t07_repeat_restarts_ttl() {
    let mut state = minimal_state();
    apply_start(&mut state, start(200, 300, 1_000_000));
    apply_start(&mut state, start(200, 300, 1_008_000));

    let entry = state.typing.get(&(200, 300)).expect("запись typing");
    assert_eq!(state.typing.len(), 1);
    assert_eq!(entry.started_at_ms, 1_008_000);
    assert_eq!(entry.expires_at_ms, 1_018_000);
    assert!(entry.is_active(1_017_999));
    assert!(!entry.is_active(1_018_000));
    assert!(state.diagnostics.duplicates.is_empty());
}

/// t08: 28 реальных START, `now=timestamp*1000` — store содержит обе пары.
#[test]
fn t08_fixture_28_starts_two_pairs() {
    let payloads = fixture_payloads();
    assert_eq!(payloads.len(), 28);

    let mut state = minimal_state();
    let mut final_now = 0;
    for (index, payload) in payloads.iter().enumerate() {
        let timestamp = payload
            .get("timestamp")
            .and_then(value::integer)
            .expect("timestamp fixture");
        let now = u64::try_from(timestamp * 1_000).expect("ms");
        let seq = u64::try_from(index + 1).expect("seq");
        let event = frame("TYPING_START", seq, payload.clone());
        let (next, _) = apply_frame_at(state, &event, now);
        state = next;
        final_now = now;
    }

    assert_eq!(state.typing.len(), 2);
    assert!(state.typing.contains_key(&(CHANNEL_A, USER_A)));
    assert!(state.typing.contains_key(&(CHANNEL_B, USER_B)));
    assert!(state.diagnostics.duplicates.is_empty());
    // DESIGN §7: счётчик учитывает только активные записи. На финальном `now`
    // пара A уже истекла (см. `waves/RW-20/w20-report.md`, C1).
    assert_eq!(state.typing_entry_count(final_now), 1);
    assert!(state.is_typing(CHANNEL_B, USER_B, final_now));
    assert!(!state.is_typing(CHANNEL_A, USER_A, final_now));
}

/// t09: MESSAGE_CREATE автора удаляет только свою пару; helper bool-контракт.
#[test]
fn t09_message_create_removes_author_pair() {
    let mut state = minimal_state();
    apply_start(&mut state, start(200, 1, 1_000_000));
    apply_start(&mut state, start(200, 2, 1_000_000));

    let (state, effects) = apply_frame_at(state, &message_frame(1, "9001", "200", "1"), 1_001_000);
    assert!(!state.typing.contains_key(&(200, 1)));
    assert!(state.typing.contains_key(&(200, 2)));
    assert!(state.messages.contains_key(&9001));
    assert!(effects.is_empty());

    let mut state = state;
    assert!(remove_for_message(&mut state, 200, 2));
    assert!(!remove_for_message(&mut state, 200, 2));
    // Удаление по user без канала не должно срабатывать.
    apply_start(&mut state, start(201, 2, 1_001_000));
    assert!(!remove_for_message(&mut state, 202, 2));
    assert!(state.typing.contains_key(&(201, 2)));

    // MESSAGE_CREATE другого автора не трогает чужую пару.
    let (state, _) = apply_frame_at(state, &message_frame(2, "9002", "201", "3"), 1_002_000);
    assert!(state.typing.contains_key(&(201, 2)));
}

/// t10: READY reset, no-clock ignored, stale/malformed диагностика, prune.
#[test]
fn t10_reset_clock_stale_malformed_prune() {
    let mut state = minimal_state();
    apply_start(&mut state, start(200, 300, 1_000_000));

    // READY создаёт пустую карту и сбрасывает последовательность.
    let (reloaded, effects) = apply_frame(state, &minimal_ready_frame());
    assert!(reloaded.typing.is_empty());
    assert_eq!(reloaded.last_seq, Some(1));
    assert!(effects.is_empty());

    // START без часов: запись не создаётся, диагностика ignored.
    let (no_clock, _) = apply_frame(reloaded, &start_frame(2, "200", "300", 1));
    assert!(no_clock.typing.is_empty());
    assert_eq!(
        no_clock.diagnostics.ignored,
        vec!["typing.received_at_ms".to_string()]
    );
    assert_eq!(no_clock.last_seq, Some(2));

    // Первый START с часами создаёт запись.
    let (applied, _) = apply_frame_at(no_clock, &start_frame(3, "200", "300", 1), 1_000_000);
    let expiry = applied
        .typing
        .get(&(200, 300))
        .expect("запись typing")
        .expires_at_ms;
    assert_eq!(expiry, 1_010_000);

    // Повтор с той же `s` — stale: TTL не продлевается.
    let (stale, _) = apply_frame_at(applied, &start_frame(3, "200", "300", 1), 1_005_000);
    assert!(stale.diagnostics.stale_sequences.contains(&3));
    assert_eq!(
        stale
            .typing
            .get(&(200, 300))
            .expect("запись typing")
            .expires_at_ms,
        1_010_000
    );

    // Битый `d`: нет channel_id — malformed `event:...`, store не меняется.
    let (broken, _) = apply_frame_at(
        stale,
        &frame("TYPING_START", 4, map(vec![("user_id", text("300"))])),
        1_006_000,
    );
    assert_eq!(broken.typing.len(), 1);
    assert!(
        broken
            .diagnostics
            .malformed
            .iter()
            .any(|entry| { entry.starts_with("event:") && entry.contains("typing.channel_id") })
    );
    assert_eq!(broken.last_seq, Some(4));

    // `d` не map — тоже malformed, без паники и без изменения store.
    let (broken, _) = apply_frame_at(broken, &frame("TYPING_START", 5, text("bad")), 1_006_000);
    assert_eq!(broken.typing.len(), 1);
    assert!(
        broken
            .diagnostics
            .malformed
            .iter()
            .any(|entry| entry.starts_with("event:") && entry.contains("typing"))
    );

    // prune удаляет только истёкшие; диагностика не меняется.
    let mut broken = broken;
    apply_start(&mut broken, start(201, 1, 1_000_000));
    apply_start(&mut broken, start(202, 2, 1_005_000));
    let diagnostics_before = broken.diagnostics.clone();
    assert_eq!(broken.typing.len(), 3);
    assert_eq!(prune_expired(&mut broken, 1_010_000), 2);
    assert_eq!(broken.typing.len(), 1);
    assert!(broken.typing.contains_key(&(202, 2)));
    assert_eq!(prune_expired(&mut broken, 1_010_000), 0);
    assert_eq!(broken.diagnostics, diagnostics_before);

    assert!(remove_entry(&mut broken, 202, 2));
    assert!(!remove_entry(&mut broken, 202, 2));
}
