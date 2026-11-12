//! W14a / P2-06a: окно истории, конфиг и live-слияние.
//!
//! Оракулы: реальные `023/024/025/026` (md5 в `waves/RW-14/RESEARCH.md`), reducer P2-02;
//! синтетические сообщения — только для границ окна/диагностики.

mod support;

use chord_core::history::{History, HistoryConfig, HistoryError, LiveOutcome};
use chord_core::state::{Event, EventPayload, Message, State, apply};
use chord_core::{
    Cursor, HistoryDiagnostics, MessagePage, PageApply, PageError, PageRequest, PageToken,
};
use support::state_fixture::{load, minimal_ready_frame};

fn event(name: &str) -> Event {
    let frame = load(name);
    Event::from_frame(&frame).unwrap_or_else(|error| panic!("{name}: {error}"))
}

fn message(name: &str) -> Message {
    match event(name).payload {
        EventPayload::MessageCreate(message) => message,
        _ => panic!("{name}: ожидался MESSAGE_CREATE"),
    }
}

fn empty_state() -> State {
    let frame = minimal_ready_frame();
    let event = Event::from_frame(&frame).expect("минимальный READY");
    match event.payload {
        EventPayload::Ready(ready) => State::from_ready(&ready),
        _ => panic!("ожидался READY"),
    }
}

fn state_with(names: &[&str]) -> State {
    let mut state = empty_state();
    for name in names {
        let (next, _) = apply(state, event(name));
        state = next;
    }
    state
}

fn synthetic(channel_id: u64, id: u64, content: &str) -> Message {
    Message {
        id,
        channel_id,
        content: content.to_string(),
        ..Message::default()
    }
}

/// H01: конфиг по умолчанию и отказ на нулевые размеры.
#[test]
fn h01_config_and_validation() {
    let config = HistoryConfig::default();
    assert_eq!(config.channel_window, 100);
    assert_eq!(config.page_limit, 50);
    assert_eq!(History::default().config(), config);

    assert_eq!(
        History::new(HistoryConfig {
            channel_window: 0,
            page_limit: 50
        })
        .unwrap_err(),
        HistoryError::ZeroWindow
    );
    assert_eq!(
        History::new(HistoryConfig {
            channel_window: 100,
            page_limit: 0
        })
        .unwrap_err(),
        HistoryError::ZeroPageLimit
    );
}

/// H02: три реальных create — порядок, content и автор из фикстур.
#[test]
fn h02_real_create_fixtures() {
    let mut history = History::default();
    for name in [
        "023-MESSAGE_CREATE.json",
        "024-MESSAGE_CREATE.json",
        "025-MESSAGE_CREATE.json",
    ] {
        history.apply_live(message(name));
    }
    let channel = 1_163_408_591_315_812_392;
    let messages = history.messages(channel);
    assert_eq!(messages.len(), 3);
    let ids: Vec<u64> = messages.iter().map(|message| message.id).collect();
    assert_eq!(
        ids,
        vec![
            1_561_627_527_296_979_046,
            1_561_627_527_833_849_898,
            1_561_627_528_685_166_655
        ]
    );
    let lengths: Vec<usize> = messages
        .iter()
        .map(|message| message.content.chars().count())
        .collect();
    assert_eq!(lengths, vec![208, 168, 173]);
    let author = messages[0].author.as_ref().expect("author");
    assert_eq!(author.id, 268_478_587_651_358_721);
    assert!(author.bot);
    assert_eq!(messages[0].guild_id, Some(1_014_537_851_418_980_372));
    assert!(history.messages(999).is_empty());
}

/// H03: повтор id — last-wins без роста окна и без duplicate-диагностики.
#[test]
fn h03_live_replacement_is_last_wins() {
    let mut history = History::default();
    assert_eq!(
        history.apply_live(synthetic(7, 1, "v1")),
        LiveOutcome::Inserted
    );
    assert_eq!(
        history.apply_live(synthetic(7, 1, "v2")),
        LiveOutcome::Replaced
    );
    assert_eq!(history.len(7), 1);
    assert_eq!(history.messages(7)[0].content, "v2");
    assert!(history.diagnostics().duplicates.is_empty());
    assert_eq!(history.diagnostics().evicted, 0);
}

/// H04: UPDATE после CREATE даёт одно сообщение с embed=1 и content 208.
#[test]
fn h04_update_upserts_full_message() {
    let state = state_with(&["023-MESSAGE_CREATE.json", "026-MESSAGE_UPDATE.json"]);
    let message = state
        .message(1_561_627_527_296_979_046)
        .expect("сообщение после update")
        .clone();
    let mut history = History::default();
    history.apply_live(message);
    assert_eq!(history.len(1_163_408_591_315_812_392), 1);
    let stored = &history.messages(1_163_408_591_315_812_392)[0];
    assert_eq!(stored.content.chars().count(), 208);
    assert_eq!(stored.embed_count, 1);
    assert_eq!(stored.id, 1_561_627_527_296_979_046);
}

/// H05: окно Latest вытесняет старые; каналы независимы.
#[test]
fn h05_latest_window_and_independent_channels() {
    let config = HistoryConfig {
        channel_window: 3,
        page_limit: 10,
    };
    let mut history = History::new(config).expect("валидный конфиг");
    for id in 1..=5 {
        history.apply_live(synthetic(10, id, "x"));
    }
    let ids: Vec<u64> = history.messages(10).iter().map(|m| m.id).collect();
    assert_eq!(ids, vec![3, 4, 5]);
    assert_eq!(history.diagnostics().evicted, 2);

    history.apply_live(synthetic(11, 100, "other"));
    assert_eq!(history.len(10), 3);
    assert_eq!(history.len(11), 1);
    assert_eq!(history.messages(11)[0].id, 100);
}

/// H06: Debug истории не печатает content.
#[test]
fn h06_debug_does_not_print_content() {
    let mut history = History::default();
    history.apply_live(synthetic(1, 1, "super-secret-content"));
    let debug = format!("{history:?}");
    assert!(!debug.contains("super-secret-content"));
    assert!(debug.contains("channels"));
}

/// H07: обещанные DESIGN §2 корневые re-exports компилируются и работают.
#[test]
fn h07_crate_root_reexports() {
    let mut history = History::new(HistoryConfig::default()).expect("конфиг");
    assert_eq!(HistoryDiagnostics::default().evicted, 0);
    assert_eq!(
        HistoryError::ZeroWindow.to_string(),
        "history: channel_window must be > 0"
    );
    let request = PageRequest::new(7, Cursor::Latest, 1);
    assert_eq!(request.query(), "limit=1");
    let page = MessagePage::default();
    assert!(page.messages.is_empty());
    assert_eq!(PageApply::default().inserted, 0);
    assert_eq!(
        PageError::ZeroLimit.to_string(),
        "history: page limit must be > 0"
    );
    assert_eq!(LiveOutcome::Inserted, LiveOutcome::Inserted);
    let token: PageToken = history.begin_page(request).expect("токен");
    assert_eq!(token.channel_id(), 7);
}
