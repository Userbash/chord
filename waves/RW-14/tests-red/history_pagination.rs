//! W14b / P2-06b: курсоры, страницы, dedup, stale-токены и окно.
//!
//! Синтетические страницы: форма REST-ответа не захвачена (RESEARCH §3), поэтому оракул —
//! замороженный контракт DESIGN §4–§5, а не выдуманный JSON Discord.

mod support;

use discord_core::history::{
    Cursor, History, HistoryConfig, MessagePage, PageError, PageRequest,
};
use discord_core::state::{Effect, Message};

fn msg(channel_id: u64, id: u64, content: &str) -> Message {
    Message {
        id,
        channel_id,
        content: content.to_string(),
        ..Message::default()
    }
}

fn page(messages: Vec<Message>, has_older: Option<bool>, has_newer: Option<bool>) -> MessagePage {
    MessagePage {
        messages,
        has_older,
        has_newer,
    }
}

fn history(cap: usize) -> History {
    History::new(HistoryConfig {
        channel_window: cap,
        page_limit: 50,
    })
    .expect("валидный конфиг")
}

/// H07: маппинг эффекта, query-формы и валидация лимита.
#[test]
fn h07_effect_mapping_and_queries() {
    let latest = PageRequest::from_effect(
        &Effect::FetchMessages {
            channel_id: 7,
            before: None,
            limit: 12,
        },
        50,
    )
    .expect("валидный эффект")
    .expect("FetchMessages");
    assert_eq!(latest.cursor, Cursor::Latest);
    assert_eq!(latest.query(), "limit=12");

    let before = PageRequest::from_effect(
        &Effect::FetchMessages {
            channel_id: 7,
            before: Some(100),
            limit: 24,
        },
        50,
    )
    .unwrap()
    .unwrap();
    assert_eq!(before.cursor, Cursor::Before(100));
    assert_eq!(before.query(), "before=100&limit=24");

    assert_eq!(
        PageRequest::new(7, Cursor::Around(100), 30).query(),
        "limit=30&around=100"
    );
    assert_eq!(
        PageRequest::new(7, Cursor::After(100), 20).query(),
        "after=100&limit=20"
    );

    assert!(
        PageRequest::from_effect(
            &Effect::FetchMessages {
                channel_id: 7,
                before: None,
                limit: 0,
            },
            50
        )
        .is_err()
    );
    assert!(matches!(
        PageRequest::from_effect(
            &Effect::FetchMessages {
                channel_id: 7,
                before: None,
                limit: 51,
            },
            50
        ),
        Err(PageError::LimitTooLarge { limit: 51, max: 50 })
    ));
    assert!(
        PageRequest::from_effect(
            &Effect::AckMessage {
                channel_id: 7,
                message_id: 1,
            },
            50
        )
        .unwrap()
        .is_none()
    );
}

/// H08: Before-страница сортируется, границы сохраняются, чужой канал атомарен.
#[test]
fn h08_before_page_and_channel_mismatch() {
    let mut history = history(10);
    let token = history
        .begin_page(PageRequest::new(7, Cursor::Before(10), 10))
        .unwrap();
    let applied = history
        .apply_page(
            token,
            page(
                vec![msg(7, 3, "c"), msg(7, 1, "a"), msg(7, 2, "b")],
                Some(false),
                Some(true),
            ),
        )
        .unwrap();
    assert_eq!(applied.inserted, 3);
    assert_eq!(history.has_older(7), Some(false));
    assert_eq!(history.has_newer(7), Some(true));
    let ids: Vec<u64> = history.messages(7).iter().map(|m| m.id).collect();
    assert_eq!(ids, vec![1, 2, 3]);

    let token = history
        .begin_page(PageRequest::new(7, Cursor::Latest, 10))
        .unwrap();
    let error = history
        .apply_page(token, page(vec![msg(8, 9, "other")], None, None))
        .unwrap_err();
    assert!(matches!(
        error,
        PageError::ChannelMismatch {
            expected: 7,
            actual: 8,
            message_id: 9
        }
    ));
    assert_eq!(history.len(7), 3);
    assert_eq!(history.diagnostics().channel_mismatches, 1);
}

/// H09: After/Latest вытесняют старые, Around — ближайшие к anchor.
#[test]
fn h09_after_and_around_windows() {
    let mut history = history(5);
    let latest = history
        .begin_page(PageRequest::new(7, Cursor::Latest, 5))
        .unwrap();
    history
        .apply_page(
            latest,
            page(
                vec![msg(7, 1, "1"), msg(7, 2, "2"), msg(7, 3, "3")],
                None,
                None,
            ),
        )
        .unwrap();
    let after = history
        .begin_page(PageRequest::new(7, Cursor::After(3), 5))
        .unwrap();
    history
        .apply_page(after, page(vec![msg(7, 4, "4"), msg(7, 5, "5")], None, None))
        .unwrap();
    let ids: Vec<u64> = history.messages(7).iter().map(|m| m.id).collect();
    assert_eq!(ids, vec![1, 2, 3, 4, 5]);

    let mut around = history(4);
    let token = around
        .begin_page(PageRequest::new(7, Cursor::Around(4), 5))
        .unwrap();
    around
        .apply_page(
            token,
            page(
                vec![
                    msg(7, 2, "2"),
                    msg(7, 3, "3"),
                    msg(7, 4, "4"),
                    msg(7, 5, "5"),
                    msg(7, 6, "6"),
                ],
                None,
                None,
            ),
        )
        .unwrap();
    let ids: Vec<u64> = around.messages(7).iter().map(|m| m.id).collect();
    assert_eq!(ids, vec![2, 3, 4, 5]);
}

/// H10: Live важнее страницы; page/page — last-wins; live вытесняет oldest.
#[test]
fn h10_live_precedence_and_eviction() {
    let mut history = history(2);
    let first = history
        .begin_page(PageRequest::new(7, Cursor::Latest, 2))
        .unwrap();
    history
        .apply_page(first, page(vec![msg(7, 1, "one"), msg(7, 2, "old")], None, None))
        .unwrap();
    assert_eq!(
        history.apply_live(msg(7, 2, "edited")),
        discord_core::history::LiveOutcome::Replaced
    );

    let second = history
        .begin_page(PageRequest::new(7, Cursor::Latest, 2))
        .unwrap();
    history
        .apply_page(second, page(vec![msg(7, 2, "old")], None, None))
        .unwrap();
    let stored = history
        .messages(7)
        .into_iter()
        .find(|message| message.id == 2)
        .expect("id 2");
    assert_eq!(stored.content, "edited");

    let third = history
        .begin_page(PageRequest::new(7, Cursor::Latest, 2))
        .unwrap();
    history
        .apply_page(third, page(vec![msg(7, 3, "three")], None, None))
        .unwrap();
    assert_eq!(history.len(7), 2);
    assert_eq!(
        history.messages(7).iter().map(|m| m.id).collect::<Vec<_>>(),
        vec![2, 3]
    );
    assert_eq!(history.diagnostics().evicted, 1);

    let mut detached = history(3);
    let token = detached
        .begin_page(PageRequest::new(7, Cursor::Before(10), 3))
        .unwrap();
    detached
        .apply_page(
            token,
            page(
                vec![msg(7, 1, "1"), msg(7, 2, "2"), msg(7, 3, "3")],
                None,
                None,
            ),
        )
        .unwrap();
    assert_eq!(
        detached.apply_live(msg(7, 4, "deferred")),
        discord_core::history::LiveOutcome::Deferred
    );
    assert_eq!(detached.messages(7).len(), 3);
    assert_eq!(detached.has_newer(7), Some(true));
}

/// H11: пустая/короткая страница, повтор страницы и stale-токен.
#[test]
fn h11_empty_pages_duplicates_and_stale_tokens() {
    let mut history = history(10);
    let token = history
        .begin_page(PageRequest::new(7, Cursor::Latest, 10))
        .unwrap();
    let applied = history
        .apply_page(token, page(Vec::new(), Some(true), Some(false)))
        .unwrap();
    assert_eq!(applied.inserted, 0);
    assert_eq!(history.has_older(7), Some(true));
    assert_eq!(history.has_newer(7), Some(false));

    let token = history
        .begin_page(PageRequest::new(7, Cursor::Latest, 10))
        .unwrap();
    let applied = history
        .apply_page(
            token,
            page(
                vec![msg(7, 1, "a"), msg(7, 1, "b"), msg(7, 2, "c")],
                Some(true),
                Some(false),
            ),
        )
        .unwrap();
    assert_eq!(history.len(7), 2);
    assert_eq!(applied.duplicates, 1);
    assert_eq!(history.diagnostics().duplicates, vec![1]);

    let short = history
        .begin_page(PageRequest::new(7, Cursor::Latest, 10))
        .unwrap();
    history
        .apply_page(short, page(vec![msg(7, 3, "d")], None, None))
        .unwrap();
    assert_eq!(history.has_older(7), Some(true));

    let token1 = history
        .begin_page(PageRequest::new(7, Cursor::Latest, 10))
        .unwrap();
    let token2 = history
        .begin_page(PageRequest::new(7, Cursor::Latest, 10))
        .unwrap();
    let stale = history
        .apply_page(token1, page(vec![msg(7, 4, "stale")], None, None))
        .unwrap_err();
    assert!(matches!(stale, PageError::StaleToken { .. }));
    assert_eq!(history.len(7), 3);
    history
        .apply_page(token2, page(vec![msg(7, 5, "new")], None, None))
        .unwrap();
    assert_eq!(history.len(7), 4);
    assert_eq!(history.diagnostics().stale_pages, 1);
}
