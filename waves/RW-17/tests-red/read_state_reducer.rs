//! W17b / P2-07b: применение `MESSAGE_ACK` к `State.read_state`.
//!
//! Оракул — реальные READY `002-READY.json`, ACK `046-MESSAGE_ACK.json` и замороженный
//! контракт `waves/RW-17/DESIGN-P2-07.md` §4. Синтетические ACK проверяют manual/mention,
//! stale/non-channel guard и неизменяемые поля записи.

mod support;

use discord_core::codec::etf::EtfValue;
use discord_core::dispatch::route_dispatch;
use discord_core::ready::{ReadStateEntry, Ready};
use discord_core::state::{State, apply_dispatch, apply_frame};
use support::state_fixture::{atom, frame, int, load, map, text};

const CHANNEL: u64 = 1_293_192_721_237_938_187;
const MESSAGE: u64 = 1_561_348_877_573_947_515;
const NON_CHANNEL: u64 = 384_299_970_632_876_032;
const REPEAT_CHANNEL: u64 = 1_176_785_373_989_445_703;
const REPEAT_MESSAGE: u64 = 1_561_379_697_990_959_244;

fn ready_state() -> State {
    let ready = Ready::from_dispatch(&load("002-READY.json")).expect("READY 002");
    State::from_ready(&ready)
}

fn id(value: u64) -> EtfValue {
    text(&value.to_string())
}

fn ack_frame(seq: u64, channel_id: u64, message_id: u64, extra: Vec<(&str, EtfValue)>) -> EtfValue {
    let mut pairs = vec![
        ("channel_id", id(channel_id)),
        ("message_id", id(message_id)),
    ];
    pairs.extend(extra);
    frame("MESSAGE_ACK", seq, map(pairs))
}

fn entry(state: &State, channel_id: u64) -> &ReadStateEntry {
    state
        .read_state_entry(channel_id)
        .unwrap_or_else(|| panic!("нет записи read_state {channel_id}"))
}

fn set_mention_count(state: &mut State, channel_id: u64, count: Option<i64>) {
    let entry = state
        .read_state
        .entries
        .iter_mut()
        .find(|entry| entry.id == channel_id)
        .unwrap_or_else(|| panic!("нет записи read_state {channel_id}"));
    entry.mention_count = count;
}

/// r01: ACK `046` применяется к записи канала READY, порядок и число записей стабильны.
#[test]
fn r01_fixture_ack_updates_channel_entry() {
    let before = ready_state();
    let index = before
        .read_state
        .entries
        .iter()
        .position(|entry| entry.id == CHANNEL)
        .expect("канал 046 есть в READY");
    let first_id = before.read_state.entries.first().map(|entry| entry.id);
    let last_id = before.read_state.entries.last().map(|entry| entry.id);
    let partial = before.read_state.partial;
    assert_eq!(before.read_state.entries.len(), 157);
    assert_eq!(before.read_state.version, 128_949);

    let (state, effects) = apply_frame(before, &load("046-MESSAGE_ACK.json"));

    assert!(effects.is_empty());
    assert_eq!(state.read_state.entries.len(), 157);
    assert_eq!(state.read_state.entries[index].id, CHANNEL);
    assert_eq!(
        state.read_state.entries.first().map(|entry| entry.id),
        first_id
    );
    assert_eq!(
        state.read_state.entries.last().map(|entry| entry.id),
        last_id
    );
    assert_eq!(state.read_state.partial, partial);
    assert_eq!(state.read_state.version, 128_950);

    let entry = entry(&state, CHANNEL);
    assert_eq!(entry.last_message_id, Some(MESSAGE));
    assert_eq!(entry.flags, Some(0));
    assert_eq!(entry.last_viewed, Some(4310));
    assert_eq!(entry.mention_count, Some(0));
    assert_eq!(state.last_seq, Some(38));
}

/// r02: неизвестный канал — ленивая запись в конце с полями ACK и mentions 0.
#[test]
fn r02_unknown_channel_creates_entry_at_end() {
    let unknown = 1_234_567_890_123_456_789;
    let ack_message = 1_566_000_000_000_000_001;
    let state = ready_state();
    let before_ids: Vec<u64> = state
        .read_state
        .entries
        .iter()
        .map(|entry| entry.id)
        .collect();
    let partial = state.read_state.partial;
    let ack = ack_frame(
        39,
        unknown,
        ack_message,
        vec![
            ("flags", int(7)),
            ("last_viewed", int(99)),
            ("version", int(128_999)),
            ("mention_count", int(9)),
        ],
    );

    let (state, effects) = apply_frame(state, &ack);

    assert!(effects.is_empty());
    assert_eq!(state.read_state.entries.len(), 158);
    assert_eq!(
        state
            .read_state
            .entries
            .iter()
            .take(157)
            .map(|entry| entry.id)
            .collect::<Vec<_>>(),
        before_ids
    );
    let entry = state.read_state.entries.last().expect("запись добавлена");
    assert_eq!(entry.id, unknown);
    assert_eq!(entry.last_message_id, Some(ack_message));
    assert_eq!(entry.flags, Some(7));
    assert_eq!(entry.last_viewed, Some(99));
    assert_eq!(entry.mention_count, Some(0));
    assert_eq!(entry.read_state_type, None);
    assert_eq!(state.read_state.version, 128_999);
    assert_eq!(state.read_state.partial, partial);
}

/// r03: `manual=true, mention_count=5` → 5.
#[test]
fn r03_manual_ack_sets_mention_count() {
    let mut state = ready_state();
    set_mention_count(&mut state, CHANNEL, Some(7));
    let ack = ack_frame(
        39,
        CHANNEL,
        MESSAGE,
        vec![("manual", atom("true")), ("mention_count", int(5))],
    );

    let (state, _) = apply_frame(state, &ack);

    assert_eq!(entry(&state, CHANNEL).mention_count, Some(5));
    assert_eq!(state.read_state.entries.len(), 157);
}

/// r04: `manual=true` без `mention_count` → 0.
#[test]
fn r04_manual_ack_without_mention_count_clears_mentions() {
    let mut state = ready_state();
    set_mention_count(&mut state, CHANNEL, Some(7));
    let ack = ack_frame(39, CHANNEL, MESSAGE, vec![("manual", atom("true"))]);

    let (state, _) = apply_frame(state, &ack);

    assert_eq!(entry(&state, CHANNEL).mention_count, Some(0));
}

/// r05: `manual=true, mention_count=-3` → 0 (clamp).
#[test]
fn r05_negative_manual_mention_count_is_clamped() {
    let mut state = ready_state();
    set_mention_count(&mut state, CHANNEL, Some(7));
    let ack = ack_frame(
        39,
        CHANNEL,
        MESSAGE,
        vec![("manual", atom("true")), ("mention_count", int(-3))],
    );

    let (state, _) = apply_frame(state, &ack);

    assert_eq!(entry(&state, CHANNEL).mention_count, Some(0));
}

/// r06: `manual=false, mention_count=5` → 0 (non-manual ACK очищает mentions).
#[test]
fn r06_non_manual_ack_ignores_mention_count() {
    let mut state = ready_state();
    set_mention_count(&mut state, CHANNEL, Some(7));
    let ack = ack_frame(
        39,
        CHANNEL,
        MESSAGE,
        vec![("manual", atom("false")), ("mention_count", int(5))],
    );

    let (state, _) = apply_frame(state, &ack);

    assert_eq!(entry(&state, CHANNEL).mention_count, Some(0));
}

/// r07: stale message-id — запись и version без изменений, ровно одна диагностика.
#[test]
fn r07_stale_message_id_is_rejected_with_diagnostic() {
    let state = ready_state();
    let before_entry = entry(&state, CHANNEL).clone();
    let before_version = state.read_state.version;
    let before_len = state.read_state.entries.len();
    let ack = ack_frame(
        39,
        CHANNEL,
        1,
        vec![
            ("flags", int(1)),
            ("last_viewed", int(9999)),
            ("version", int(128_999)),
            ("manual", atom("true")),
            ("mention_count", int(5)),
        ],
    );

    let (state, _) = apply_frame(state, &ack);

    assert_eq!(state.read_state.entries.len(), before_len);
    assert_eq!(entry(&state, CHANNEL), &before_entry);
    assert_eq!(state.read_state.version, before_version);
    assert_eq!(
        state.diagnostics.inconsistencies,
        vec![format!("read_state.ack:{CHANNEL}:stale:1")]
    );
    assert_eq!(state.last_seq, Some(39));
}

/// r08: равный message-id — норма: metadata/version обновляются, mentions очищаются,
/// duplicate-диагностики нет.
#[test]
fn r08_equal_message_id_updates_metadata_without_duplicate() {
    let mut state = ready_state();
    set_mention_count(&mut state, CHANNEL, Some(7));
    let ack = ack_frame(
        39,
        CHANNEL,
        MESSAGE,
        vec![
            ("flags", int(1)),
            ("last_viewed", int(4321)),
            ("version", int(128_960)),
            ("mention_count", int(9)),
        ],
    );

    let (state, _) = apply_frame(state, &ack);

    assert_eq!(state.read_state.entries.len(), 157);
    let entry = entry(&state, CHANNEL);
    assert_eq!(entry.last_message_id, Some(MESSAGE));
    assert_eq!(entry.flags, Some(1));
    assert_eq!(entry.last_viewed, Some(4321));
    assert_eq!(entry.mention_count, Some(0));
    assert_eq!(state.read_state.version, 128_960);
    assert!(state.diagnostics.duplicates.is_empty());
    assert!(state.diagnostics.inconsistencies.is_empty());
}

/// r09: `flags`/`last_viewed`/`version = nil` сохраняют прежние значения.
#[test]
fn r09_missing_optional_fields_preserve_previous_values() {
    let first = ack_frame(
        39,
        CHANNEL,
        MESSAGE,
        vec![
            ("flags", int(3)),
            ("last_viewed", int(4400)),
            ("version", int(128_955)),
        ],
    );
    let (state, _) = apply_frame(ready_state(), &first);
    assert_eq!(state.read_state.version, 128_955);

    let second = ack_frame(
        40,
        CHANNEL,
        MESSAGE,
        vec![
            ("flags", atom("nil")),
            ("last_viewed", atom("nil")),
            ("version", atom("nil")),
            ("mention_count", atom("nil")),
        ],
    );
    let (state, _) = apply_frame(state, &second);

    let entry = entry(&state, CHANNEL);
    assert_eq!(entry.last_message_id, Some(MESSAGE));
    assert_eq!(entry.flags, Some(3));
    assert_eq!(entry.last_viewed, Some(4400));
    assert_eq!(state.read_state.version, 128_955);
}

/// r10: non-channel (`read_state_type=2`) не меняется, диагностика `non-channel`.
#[test]
fn r10_non_channel_entry_is_guarded() {
    let state = ready_state();
    let before_entry = entry(&state, NON_CHANNEL).clone();
    assert_eq!(before_entry.read_state_type, Some(2));
    let before_version = state.read_state.version;
    let ack = ack_frame(
        39,
        NON_CHANNEL,
        1_294_268_550_826_950_656,
        vec![
            ("flags", int(1)),
            ("last_viewed", int(1)),
            ("version", int(128_999)),
        ],
    );

    let (state, _) = apply_frame(state, &ack);

    assert_eq!(state.read_state.entries.len(), 157);
    assert_eq!(entry(&state, NON_CHANNEL), &before_entry);
    assert_eq!(state.read_state.version, before_version);
    assert_eq!(
        state.diagnostics.inconsistencies,
        vec![format!("read_state.ack:{NON_CHANNEL}:non-channel:2")]
    );
    assert_eq!(state.last_seq, Some(39));
}

/// r11: повторные ACK того же канала/сообщения — не duplicate, версия растёт.
#[test]
fn r11_repeated_acks_are_idempotent_with_growing_version() {
    let first = ack_frame(
        39,
        REPEAT_CHANNEL,
        REPEAT_MESSAGE,
        vec![("version", int(128_951))],
    );
    let (state, _) = apply_frame(ready_state(), &first);
    assert_eq!(state.read_state.version, 128_951);
    assert_eq!(state.read_state.entries.len(), 157);

    let second = ack_frame(
        40,
        REPEAT_CHANNEL,
        REPEAT_MESSAGE,
        vec![("version", int(128_953))],
    );
    let (state, _) = apply_frame(state, &second);

    assert_eq!(state.read_state.version, 128_953);
    assert_eq!(state.read_state.entries.len(), 157);
    assert_eq!(
        entry(&state, REPEAT_CHANNEL).last_message_id,
        Some(REPEAT_MESSAGE)
    );
    assert!(state.diagnostics.duplicates.is_empty());
    assert!(state.diagnostics.inconsistencies.is_empty());
}

/// r12: stale `s` (seq <= last_seq) отбрасывается до применения ACK.
#[test]
fn r12_stale_sequence_does_not_touch_read_state() {
    let ack = ack_frame(51, CHANNEL, MESSAGE, vec![("version", int(128_950))]);
    let (fresh, _) = apply_frame(ready_state(), &ack);
    assert_eq!(fresh.read_state.version, 128_950);

    let mut state = ready_state();
    state.last_seq = Some(51);
    let before = state.read_state.clone();
    let (state, effects) = apply_frame(state, &ack);

    assert!(effects.is_empty());
    assert_eq!(state.last_seq, Some(51));
    assert_eq!(state.diagnostics.stale_sequences, vec![51]);
    assert_eq!(state.read_state, before);
}

/// r13: битый ACK — `malformed`, без новых записей, `last_seq` монотонен.
#[test]
fn r13_broken_ack_is_malformed_and_keeps_monotonic_seq() {
    let mut state = ready_state();
    state.last_seq = Some(30);
    let before_version = state.read_state.version;
    let broken = frame("MESSAGE_ACK", 38, map(vec![("message_id", id(MESSAGE))]));
    let dispatch = route_dispatch(&broken).expect("op0 MESSAGE_ACK маршрутизируется");

    let (state, effects) = apply_dispatch(state, &dispatch);

    assert!(effects.is_empty());
    assert_eq!(state.read_state.entries.len(), 157);
    assert_eq!(state.read_state.version, before_version);
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

    let valid = ack_frame(39, CHANNEL, MESSAGE, vec![("version", int(128_950))]);
    let (state, _) = apply_frame(state, &valid);
    assert_eq!(state.read_state.version, 128_950);
    assert_eq!(state.read_state.entries.len(), 157);
    assert_eq!(state.last_seq, Some(39));
}

/// r14: `badge_count`/`last_acked_id`/`last_pin_timestamp`/`read_state_type` не меняются.
#[test]
fn r14_ack_does_not_touch_unrelated_entry_fields() {
    let (state, _) = apply_frame(ready_state(), &load("046-MESSAGE_ACK.json"));

    let channel = entry(&state, CHANNEL);
    // ACK действительно применён: ack-точка и last_viewed обновились.
    assert_eq!(channel.last_message_id, Some(MESSAGE));
    assert_eq!(channel.last_viewed, Some(4310));
    assert_eq!(
        channel.last_pin_timestamp.as_deref(),
        Some("2026-04-09T09:21:10+00:00")
    );
    assert_eq!(channel.badge_count, None);
    assert_eq!(channel.last_acked_id, None);
    assert_eq!(channel.read_state_type, None);

    let non_channel = entry(&state, NON_CHANNEL);
    assert_eq!(non_channel.badge_count, Some(16));
    assert_eq!(non_channel.last_acked_id, Some(1_294_268_550_826_950_656));
    assert_eq!(non_channel.read_state_type, Some(2));
}
