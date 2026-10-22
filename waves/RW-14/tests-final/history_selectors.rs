//! W14c / P2-06c: read-only селекторы истории.
//!
//! Оракул: замороженный контракт DESIGN §6 и реальные `023/024/025` (md5 в
//! `waves/RW-14/RESEARCH.md`) для порядка снежинок. Селекторы не мутируют историю
//! и не создают канал.

mod support;

use discord_core::history::{Cursor, History, HistoryConfig, MessagePage, PageRequest};
use discord_core::state::{Event, EventPayload, Message};
use support::state_fixture::load;

const CHANNEL_A: u64 = 7;
const CHANNEL_B: u64 = 8;
const UNKNOWN: u64 = 999;

fn message(channel_id: u64, id: u64, content: &str) -> Message {
    Message {
        id,
        channel_id,
        content: content.to_string(),
        ..Message::default()
    }
}

fn live(history: &mut History, channel_id: u64, id: u64) {
    history.apply_live(message(channel_id, id, "live"));
}

fn ids(messages: &[&Message]) -> Vec<u64> {
    messages.iter().map(|message| message.id).collect()
}

fn history(cap: usize) -> History {
    History::new(HistoryConfig {
        channel_window: cap,
        page_limit: 50,
    })
    .expect("валидный конфиг")
}

fn apply_page(history: &mut History, channel_id: u64, page: MessagePage) {
    let token = history
        .begin_page(PageRequest::new(channel_id, Cursor::Latest, 10))
        .expect("валидный лимит");
    history
        .apply_page(token, page)
        .expect("страница своего канала");
}

/// S01: два канала независимы; `message` не протекает между каналами.
#[test]
fn s01_channels_do_not_leak() {
    let mut history = history(10);
    for id in [30, 10, 20] {
        live(&mut history, CHANNEL_A, id);
    }
    live(&mut history, CHANNEL_B, 100);

    assert_eq!(history.channel_count(), 2);
    assert_eq!(history.total_len(), 4);
    assert_eq!(history.len(CHANNEL_A), 3);
    assert_eq!(history.len(CHANNEL_B), 1);
    assert!(!history.is_empty(CHANNEL_A));

    assert_eq!(ids(&history.messages(CHANNEL_A)), vec![10, 20, 30]);
    assert_eq!(ids(&history.messages(CHANNEL_B)), vec![100]);

    assert_eq!(
        history.message(CHANNEL_A, 20).map(|m| m.content.as_str()),
        Some("live")
    );
    assert!(history.message(CHANNEL_A, 100).is_none());
    assert!(history.message(CHANNEL_B, 20).is_none());

    assert!(history.is_empty(UNKNOWN));
    assert_eq!(history.len(UNKNOWN), 0);
    assert!(history.messages(UNKNOWN).is_empty());
    assert!(history.message(UNKNOWN, 10).is_none());
}

/// S02: границы три-состояния; `None` не превращается в `false`.
#[test]
fn s02_bounds_tri_state() {
    let mut history = history(10);
    assert_eq!(history.has_older(UNKNOWN), None);
    assert_eq!(history.has_newer(UNKNOWN), None);

    // Пустая страница без границ создаёт канал, но не выдумывает `false`.
    apply_page(
        &mut history,
        CHANNEL_A,
        MessagePage {
            messages: Vec::new(),
            has_older: None,
            has_newer: None,
        },
    );
    assert_eq!(history.has_older(CHANNEL_A), None);
    assert_eq!(history.has_newer(CHANNEL_A), None);

    apply_page(
        &mut history,
        CHANNEL_A,
        MessagePage {
            messages: Vec::new(),
            has_older: Some(false),
            has_newer: None,
        },
    );
    assert_eq!(history.has_older(CHANNEL_A), Some(false));
    assert_eq!(history.has_newer(CHANNEL_A), None);

    apply_page(
        &mut history,
        CHANNEL_A,
        MessagePage {
            messages: Vec::new(),
            has_older: None,
            has_newer: Some(true),
        },
    );
    assert_eq!(history.has_older(CHANNEL_A), Some(false));
    assert_eq!(history.has_newer(CHANNEL_A), Some(true));

    apply_page(
        &mut history,
        CHANNEL_A,
        MessagePage {
            messages: Vec::new(),
            has_older: None,
            has_newer: None,
        },
    );
    assert_eq!(history.has_older(CHANNEL_A), Some(false));
    assert_eq!(history.has_newer(CHANNEL_A), Some(true));
}

/// S03: `recent` — newest-first; `oldest`/`newest`; `limit=0` пуст.
#[test]
fn s03_recent_oldest_newest() {
    let mut history = history(10);
    for id in [40, 10, 30, 20] {
        live(&mut history, CHANNEL_A, id);
    }

    assert_eq!(ids(&history.recent(CHANNEL_A, 3)), vec![40, 30, 20]);
    assert_eq!(ids(&history.recent(CHANNEL_A, 99)), vec![40, 30, 20, 10]);
    assert!(history.recent(CHANNEL_A, 0).is_empty());
    assert!(history.recent(UNKNOWN, 3).is_empty());

    assert_eq!(history.oldest(CHANNEL_A).map(|m| m.id), Some(10));
    assert_eq!(history.newest(CHANNEL_A).map(|m| m.id), Some(40));
    assert!(history.oldest(UNKNOWN).is_none());
    assert!(history.newest(UNKNOWN).is_none());
}

/// S04: `range` включителен с обеих сторон; `from > to` и чужие id пусты.
#[test]
fn s04_range_is_inclusive() {
    let mut history = history(10);
    for id in [10, 20, 30, 40] {
        live(&mut history, CHANNEL_A, id);
    }

    assert_eq!(ids(&history.range(CHANNEL_A, 10, 30)), vec![10, 20, 30]);
    assert_eq!(ids(&history.range(CHANNEL_A, 20, 20)), vec![20]);
    assert_eq!(ids(&history.range(CHANNEL_A, 15, 25)), vec![20]);
    assert_eq!(
        ids(&history.range(CHANNEL_A, 0, u64::MAX)),
        vec![10, 20, 30, 40]
    );
    assert!(history.range(CHANNEL_A, 30, 10).is_empty());
    assert!(history.range(CHANNEL_A, 50, 60).is_empty());
    assert!(history.range(UNKNOWN, 0, 40).is_empty());
}

/// S05: `window_around` — ближайшие к anchor, tie меньшим id, результат ascending.
#[test]
fn s05_window_around_tie_and_ascending() {
    let mut history = history(10);
    for id in [2, 3, 5, 6] {
        live(&mut history, CHANNEL_A, id);
    }

    // anchor=4 отсутствует; расстояния 3 и 5 равны — остаётся меньший id 3.
    assert_eq!(ids(&history.window_around(CHANNEL_A, 4, 2)), vec![3, 5]);
    assert!(history.window_around(CHANNEL_A, 4, 0).is_empty());
    assert_eq!(
        ids(&history.window_around(CHANNEL_A, 4, 4)),
        vec![2, 3, 5, 6]
    );
    assert!(history.window_around(UNKNOWN, 4, 3).is_empty());

    live(&mut history, CHANNEL_A, 4);
    assert_eq!(ids(&history.window_around(CHANNEL_A, 4, 3)), vec![3, 4, 5]);
    // Четвёртое место делят 2 и 6 — tie оставляет 2.
    assert_eq!(
        ids(&history.window_around(CHANNEL_A, 4, 4)),
        vec![2, 3, 4, 5]
    );
}

/// S06: селекторы не мутируют и не создают канал; повтор детерминирован.
#[test]
fn s06_read_only_and_deterministic() {
    fn build() -> History {
        let mut history = history(10);
        for id in [10, 20, 30] {
            live(&mut history, CHANNEL_A, id);
        }
        live(&mut history, CHANNEL_B, 5);
        history
    }

    let history = build();
    let channels = history.channel_count();
    let total = history.total_len();

    let probe = |history: &History| {
        vec![
            ids(&history.messages(CHANNEL_A)),
            ids(&history.recent(CHANNEL_A, 2)),
            ids(&history.range(CHANNEL_A, 10, 30)),
            ids(&history.window_around(CHANNEL_A, 20, 2)),
            ids(&history.messages(UNKNOWN)),
            ids(&history.recent(UNKNOWN, 2)),
            ids(&history.range(UNKNOWN, 0, 10)),
            ids(&history.window_around(UNKNOWN, 1, 2)),
        ]
    };
    let first = probe(&history);
    let _ = (
        history.message(CHANNEL_A, 20),
        history.oldest(CHANNEL_A),
        history.newest(CHANNEL_A),
        history.has_older(CHANNEL_A),
        history.has_newer(CHANNEL_A),
    );
    let second = probe(&history);
    assert_eq!(first, second);

    assert_eq!(history.channel_count(), channels);
    assert_eq!(history.total_len(), total);
    assert_eq!(history.len(UNKNOWN), 0);
    assert_eq!(history.channel_count(), 2);

    let other = build();
    assert_eq!(
        ids(&history.messages(CHANNEL_A)),
        ids(&other.messages(CHANNEL_A))
    );
    assert_eq!(
        ids(&history.recent(CHANNEL_A, 2)),
        ids(&other.recent(CHANNEL_A, 2))
    );
    assert_eq!(
        ids(&history.window_around(CHANNEL_A, 20, 3)),
        ids(&other.window_around(CHANNEL_A, 20, 3))
    );
}

/// S07: `Default` пуст; `Debug` истории не печатает content.
#[test]
fn s07_default_empty_and_debug_hides_content() {
    let empty = History::default();
    assert_eq!(empty.channel_count(), 0);
    assert_eq!(empty.total_len(), 0);
    assert!(empty.is_empty(1));
    assert!(empty.messages(1).is_empty());
    assert!(empty.recent(1, 5).is_empty());
    assert!(empty.oldest(1).is_none());
    assert!(empty.newest(1).is_none());
    assert_eq!(empty.channel_count(), 0);

    let mut history = History::default();
    history.apply_live(message(CHANNEL_A, 1, "super-secret-content"));
    let debug = format!("{history:?}");
    assert!(!debug.contains("super-secret-content"));
    assert!(debug.contains("channels"));
    assert_eq!(history.channel_count(), 1);
}

fn real_message(name: &str) -> Message {
    let frame = load(name);
    let event = Event::from_frame(&frame).unwrap_or_else(|error| panic!("{name}: {error}"));
    match event.payload {
        EventPayload::MessageCreate(message) => message,
        _ => panic!("{name}: ожидался MESSAGE_CREATE"),
    }
}

/// S08: реальные `023/024/025` — порядок селекторов совпадает со снежинками.
#[test]
fn s08_real_fixtures_order() {
    let mut history = History::default();
    for name in [
        "023-MESSAGE_CREATE.json",
        "024-MESSAGE_CREATE.json",
        "025-MESSAGE_CREATE.json",
    ] {
        history.apply_live(real_message(name));
    }
    let channel = 1_163_408_591_315_812_392;
    let first = 1_561_627_527_296_979_046;
    let second = 1_561_627_527_833_849_898;
    let third = 1_561_627_528_685_166_655;

    assert_eq!(ids(&history.messages(channel)), vec![first, second, third]);
    assert_eq!(ids(&history.recent(channel, 2)), vec![third, second]);
    assert_eq!(history.oldest(channel).map(|m| m.id), Some(first));
    assert_eq!(history.newest(channel).map(|m| m.id), Some(third));
    assert_eq!(
        ids(&history.range(channel, first, second)),
        vec![first, second]
    );
    assert_eq!(history.len(channel), 3);
    assert_eq!(history.total_len(), 3);
}
