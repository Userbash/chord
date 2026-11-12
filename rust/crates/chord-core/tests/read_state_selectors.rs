//! W17c / P2-07c: селекторы mentions и бейджей.
//!
//! Оракул — реальная фикстура `002-READY.json` (`jq` из
//! `waves/RW-17/RESEARCH-P2-07.md` §2) и замороженный контракт
//! `waves/RW-17/DESIGN-P2-07.md` §5. Синтетические записи проверяют clamp, fallback
//! ack-точки и сортировку независимо от порядка входа.
//!
//! Тесты падают до реализации селекторов — это compile-red.

mod support;

use chord_core::ready::ReadStateEntry;
use chord_core::state::{Event, EventPayload, State, apply_frame};

fn ready_state() -> State {
    let event = Event::from_frame(&support::state_fixture::load("002-READY.json"))
        .expect("READY 002 должен разбираться");
    let EventPayload::Ready(ready) = event.payload else {
        panic!("ожидался READY");
    };
    State::from_ready(&ready)
}

fn minimal_state() -> State {
    let frame = support::state_fixture::minimal_ready_frame();
    let event = Event::from_frame(&frame).expect("minimal READY");
    let EventPayload::Ready(ready) = event.payload else {
        panic!("ожидался READY");
    };
    State::from_ready(&ready)
}

fn entry(id: u64, mention_count: Option<i64>, badge_count: Option<i64>) -> ReadStateEntry {
    ReadStateEntry {
        id,
        mention_count,
        badge_count,
        ..ReadStateEntry::default()
    }
}

/// Записать synthetic mention в существующую запись (или добавить новую).
fn set_mention(state: &mut State, id: u64, count: i64) {
    if let Some(existing) = state
        .read_state
        .entries
        .iter_mut()
        .find(|entry| entry.id == id)
    {
        existing.mention_count = Some(count);
    } else {
        state.read_state.entries.push(entry(id, Some(count), None));
    }
}

/// s01: READY — mentions пусты, положительный badge только у `384299970632876032`.
///
/// TZ §W17c s01 перечисляет обе записи с полем `badge_count`, но у гильдии
/// `1014537851418980372` в фикстуре `badge_count=0`. DESIGN §5 задаёт
/// `badge_entries` как `badge_count > 0`, поэтому нулевая запись не входит; факт
/// фикстуры проверяется отдельным `badge_count`.
#[test]
fn s01_ready_mentions_and_badges() {
    let state = ready_state();

    assert!(state.mention_channels().is_empty());
    assert_eq!(state.total_mentions(), 0);
    assert!(!state.has_mentions(1_293_192_721_237_938_187));
    assert_eq!(state.mention_count(1_293_192_721_237_938_187), 0);
    assert_eq!(state.mention_count(1), 0);

    assert_eq!(
        state.badge_entries(),
        vec![384_299_970_632_876_032],
        "DESIGN §5: badge_entries — только badge_count > 0"
    );
    assert_eq!(state.badge_count(384_299_970_632_876_032), 16);
    assert_eq!(state.badge_count(1_014_537_851_418_980_372), 0);
    assert_eq!(state.total_badges(), 16);
}

/// s02: synthetic mentions 2, 0, -3 и отсутствие — clamp, `mention_channels`, сумма.
#[test]
fn s02_synthetic_mentions_clamp_and_filter() {
    let mut state = minimal_state();
    state.read_state.entries.push(entry(40, Some(2), None));
    state.read_state.entries.push(entry(30, Some(0), None));
    state.read_state.entries.push(entry(20, Some(-3), None));
    state.read_state.entries.push(entry(10, None, None));

    assert_eq!(state.mention_count(40), 2);
    assert_eq!(state.mention_count(30), 0);
    assert_eq!(state.mention_count(20), 0);
    assert_eq!(state.mention_count(10), 0);
    assert_eq!(state.mention_count(99), 0);

    assert!(state.has_mentions(40));
    assert!(!state.has_mentions(20));
    assert!(!state.has_mentions(99));

    assert_eq!(state.mention_channels(), vec![40]);
    assert_eq!(state.total_mentions(), 2);
}

/// s03: гильдия суммирует mention-каналы и треды; ЛС и неизвестный канал не входят.
#[test]
fn s03_guild_mentions_include_channels_and_threads_only() {
    let guild_id = 1_014_537_851_418_980_372;
    let channel_id = 1_163_408_591_315_812_392;
    let thread_id = 42;

    let frame = support::state_fixture::thread_sync_frame(
        3,
        "1014537851418980372",
        vec![support::state_fixture::thread(
            "42",
            "1014537851418980372",
            "1163408591315812392",
            "тред",
        )],
    );
    let (mut state, _) = apply_frame(ready_state(), &frame);
    assert!(state.thread(thread_id).is_some(), "тред должен примениться");

    set_mention(&mut state, channel_id, 2);
    set_mention(&mut state, thread_id, 3);

    let private_id = *state
        .private_channels
        .keys()
        .next()
        .expect("в READY есть приватные каналы");
    set_mention(&mut state, private_id, 5);
    set_mention(&mut state, 999_999_999, 7);

    assert_eq!(state.guild_mention_count(guild_id), 5);
    assert_eq!(state.guild_mention_count(274_093_052_673_392_641), 0);
    assert_eq!(state.guild_mention_count(999), 0);
}

/// s04: `private_mention_count` учитывает только id из `private_channels`.
#[test]
fn s04_private_mentions_only_private_channels() {
    let mut state = ready_state();
    let private_id = *state
        .private_channels
        .keys()
        .next()
        .expect("в READY есть приватные каналы");
    let guild_channel_id = 1_163_408_591_315_812_392;

    set_mention(&mut state, private_id, 4);
    set_mention(&mut state, guild_channel_id, 7);

    assert_eq!(state.private_mention_count(), 4);
    assert_eq!(minimal_state().private_mention_count(), 0);
}

/// s05: `badge_count` clamp в 0, `badge_entries` — только положительные по возрастанию id.
#[test]
fn s05_badge_count_clamps_and_entries_sort() {
    let mut state = minimal_state();
    state.read_state.entries.push(entry(900, None, Some(5)));
    state.read_state.entries.push(entry(100, None, Some(-2)));
    state.read_state.entries.push(entry(500, None, Some(1)));
    state.read_state.entries.push(entry(400, None, None));

    assert_eq!(state.badge_count(900), 5);
    assert_eq!(state.badge_count(100), 0);
    assert_eq!(state.badge_count(500), 1);
    assert_eq!(state.badge_count(400), 0);
    assert_eq!(state.badge_count(99), 0);

    assert_eq!(state.badge_entries(), vec![500, 900]);
    assert_eq!(state.total_badges(), 6);
}

/// s06: channel-запись — только `last_message_id`; non-channel — `last_acked_id`
/// с fallback на `last_message_id`.
#[test]
fn s06_acknowledged_message_id_uses_channel_or_non_channel_point() {
    let ready = ready_state();
    assert_eq!(
        ready.acknowledged_message_id(1_293_192_721_237_938_187),
        Some(1_561_348_877_573_947_515)
    );
    assert_eq!(
        ready.acknowledged_message_id(384_299_970_632_876_032),
        Some(1_294_268_550_826_950_656)
    );
    assert_eq!(
        ready.acknowledged_message_id(1_014_537_851_418_980_372),
        Some(1_237_379_809_395_605_509)
    );
    assert_eq!(ready.acknowledged_message_id(999), None);

    let mut state = minimal_state();
    state.read_state.entries.push(ReadStateEntry {
        id: 10,
        last_message_id: Some(100),
        last_acked_id: Some(200),
        ..ReadStateEntry::default()
    });
    state.read_state.entries.push(ReadStateEntry {
        id: 20,
        read_state_type: Some(0),
        last_acked_id: Some(300),
        ..ReadStateEntry::default()
    });
    state.read_state.entries.push(ReadStateEntry {
        id: 30,
        read_state_type: Some(2),
        last_message_id: Some(400),
        last_acked_id: Some(500),
        ..ReadStateEntry::default()
    });
    state.read_state.entries.push(ReadStateEntry {
        id: 40,
        read_state_type: Some(4),
        last_message_id: Some(600),
        ..ReadStateEntry::default()
    });
    state.read_state.entries.push(ReadStateEntry {
        id: 50,
        read_state_type: Some(2),
        ..ReadStateEntry::default()
    });

    assert_eq!(state.acknowledged_message_id(10), Some(100));
    // P2-1: channel без last_message_id не использует last_acked_id как ack-точку.
    assert_eq!(state.acknowledged_message_id(20), None);
    assert_eq!(state.acknowledged_message_id(30), Some(500));
    assert_eq!(state.acknowledged_message_id(40), Some(600));
    assert_eq!(state.acknowledged_message_id(50), None);
    assert_eq!(state.acknowledged_message_id(99), None);
}

/// s07: `is_message_read` — ack >= message_id; выше точки и неизвестные — false.
#[test]
fn s07_is_message_read_compares_against_ack_point() {
    let mut state = minimal_state();
    state.read_state.entries.push(ReadStateEntry {
        id: 10,
        last_message_id: Some(1_000),
        ..ReadStateEntry::default()
    });
    state.read_state.entries.push(ReadStateEntry {
        id: 20,
        read_state_type: Some(2),
        last_message_id: Some(900),
        last_acked_id: Some(500),
        ..ReadStateEntry::default()
    });

    assert!(state.is_message_read(10, 1_000));
    assert!(state.is_message_read(10, 999));
    assert!(!state.is_message_read(10, 1_001));
    assert!(!state.is_message_read(99, 1));

    assert!(state.is_message_read(20, 500));
    assert!(!state.is_message_read(20, 501));
}

/// s08: версия read-state READY и пустого состояния.
#[test]
fn s08_read_state_version() {
    assert_eq!(ready_state().read_state_version(), 128_949);
    assert_eq!(minimal_state().read_state_version(), 0);
}

/// s09: ACK `046` через `apply_frame` — mentions очищены, версия 128950, сообщение прочитано.
#[test]
fn s09_ack_fixture_updates_selectors() {
    let channel_id = 1_293_192_721_237_938_187;
    let message_id = 1_561_348_877_573_947_515;
    let state = ready_state();
    assert_eq!(state.mention_count(channel_id), 0);

    let (state, effects) =
        apply_frame(state, &support::state_fixture::load("046-MESSAGE_ACK.json"));
    assert!(effects.is_empty());

    assert_eq!(state.mention_count(channel_id), 0);
    assert!(!state.has_mentions(channel_id));
    assert_eq!(state.read_state_version(), 128_950);
    assert!(state.is_message_read(channel_id, message_id));
    assert_eq!(state.acknowledged_message_id(channel_id), Some(message_id));
}

/// s11: channel-запись без `last_message_id` не использует `last_acked_id` как ack-точку.
///
/// P2-1 независимого ревью: DESIGN §5/§6 разводит channel (`last_message_id`) и
/// non-channel (`last_acked_id`) семантику; fallback для канала давал ложное «прочитано».
#[test]
fn s11_channel_ack_point_does_not_use_last_acked_id() {
    let mut state = minimal_state();
    state.read_state.entries.push(ReadStateEntry {
        id: 60,
        read_state_type: Some(0),
        last_acked_id: Some(700),
        ..ReadStateEntry::default()
    });

    assert_eq!(state.acknowledged_message_id(60), None);
    assert!(!state.is_message_read(60, 700));
}

/// s10: порядок входа записей не влияет на `mention_channels`/`badge_entries` и суммы.
#[test]
fn s10_entry_order_does_not_affect_sorting() {
    let entries = vec![
        entry(900, Some(4), Some(1)),
        entry(100, Some(1), Some(5)),
        entry(500, Some(-2), Some(-3)),
        entry(300, None, None),
        entry(700, Some(0), Some(0)),
    ];

    let mut forward = minimal_state();
    for item in entries.clone() {
        forward.read_state.entries.push(item);
    }
    let mut reverse = minimal_state();
    for item in entries.into_iter().rev() {
        reverse.read_state.entries.push(item);
    }

    for state in [&forward, &reverse] {
        assert_eq!(state.mention_channels(), vec![100, 900]);
        assert_eq!(state.total_mentions(), 5);
        assert_eq!(state.badge_entries(), vec![100, 900]);
        assert_eq!(state.total_badges(), 6);
        assert_eq!(state.mention_count(500), 0);
        assert_eq!(state.badge_count(500), 0);
    }
}
