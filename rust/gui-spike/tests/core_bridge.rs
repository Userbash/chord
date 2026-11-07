//! W39j: контракт моста BFF → `discord-core` на реальных фикстурах.
//!
//! Оракул — публичный BFF (`data::messages_page`), а не реализация моста:
//! страница ядра обязана отдавать те же id в том же порядке, тот же курсор и
//! те же `hasMore`/`total`. Проверяется, что пилот «BFF через ядро» не меняет
//! наблюдаемый контракт страницы.
//!
//! Skip-aware: без `fixtures/` тесты печатают причину и возвращаются;
//! при `DL_STRICT=1` — падают (CI-режим).

use std::path::PathBuf;

use discord_gui_spike::core_bridge;
use discord_gui_spike::data::{self, MessageAuthor, MessageRow, MAX_PAGE_LIMIT};

/// Канал READY с реальными (положительными) snowflake — единственный, который
/// ядро представляет без потерь: 3 сообщения, CREATE/UPDATE дедуплицируются.
const NEWS_CHANNEL: &str = "1163408591315812392";
const NEWS_AUTHOR: &str = "268478587651358721";

fn candidate_root() -> PathBuf {
    std::env::var("DL_FIXTURES")
        .ok()
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(|path| path.parent())
                .expect("crate лежит в rust/gui-spike")
                .join("fixtures")
        })
}

fn fixtures_root() -> Option<PathBuf> {
    let root = candidate_root();
    root.join("gateway/by-type/002-READY.json")
        .is_file()
        .then_some(root)
}

macro_rules! fixtures_or_skip {
    () => {
        match fixtures_root() {
            Some(root) => root,
            None => {
                assert!(
                    std::env::var("DL_STRICT").ok().as_deref() != Some("1"),
                    "DL_STRICT=1: фикстуры обязательны, но {} не найден",
                    candidate_root().display()
                );
                eprintln!(
                    "SKIP core_bridge: нет фикстур в {} (DL_STRICT≠1)",
                    candidate_root().display()
                );
                return;
            }
        }
    };
}

fn ids_of(page: &[MessageRow]) -> Vec<String> {
    page.iter().map(|row| row.id.clone()).collect()
}

#[test]
fn first_page_ids_and_order_match_bff() {
    let root = fixtures_or_skip!();

    let bff = data::messages_page(&root, NEWS_CHANNEL, None, Some(MAX_PAGE_LIMIT))
        .expect("BFF-страница канала");
    let core = core_bridge::core_messages_page(&root, NEWS_CHANNEL, None, Some(MAX_PAGE_LIMIT))
        .expect("страница ядра");

    assert_eq!(core.channel_id, bff.channel_id);
    assert_eq!(core.total, bff.total, "total = все сообщения канала");
    assert_eq!(core.limit, bff.limit);
    assert_eq!(core.has_more, bff.has_more);
    assert_eq!(core.next_cursor, bff.next_cursor);
    assert_eq!(
        core.items.iter().map(|m| m.id).collect::<Vec<_>>(),
        bff.items
            .iter()
            .map(|row| row.id.parse::<u64>().expect("id фикстуры числовой"))
            .collect::<Vec<_>>(),
        "ids и порядок ядра обязаны совпасть с BFF"
    );
    assert_eq!(ids_of(&bff.items).len(), 3, "CREATE/UPDATE дедуплицируются");
}

#[test]
fn pagination_is_stable_forward_and_on_repeat() {
    let root = fixtures_or_skip!();

    let mut bff_ids = Vec::new();
    let mut core_ids = Vec::new();
    let mut bff_cursor: Option<String> = None;
    let mut core_cursor: Option<String> = None;

    // Вперёд по одной строке: последовательности id совпадают, пересечений нет.
    for _ in 0..=MAX_PAGE_LIMIT {
        let bff = data::messages_page(&root, NEWS_CHANNEL, bff_cursor.as_deref(), Some(1))
            .expect("шаг BFF");
        let core =
            core_bridge::core_messages_page(&root, NEWS_CHANNEL, core_cursor.as_deref(), Some(1))
                .expect("шаг ядра");

        assert_eq!(
            core.items
                .iter()
                .map(|m| m.id.to_string())
                .collect::<Vec<_>>(),
            ids_of(&bff.items),
            "страница ядра = страница BFF на каждом шаге"
        );
        assert_eq!(core.next_cursor, bff.next_cursor);
        assert_eq!(core.has_more, bff.has_more);
        assert_eq!(core.total, bff.total);

        // Повтор с тем же курсором — тот же результат (состояния нет).
        let core_repeat =
            core_bridge::core_messages_page(&root, NEWS_CHANNEL, core_cursor.as_deref(), Some(1))
                .expect("повтор шага ядра");
        assert_eq!(core_repeat.items, core.items, "повтор стабилен");

        bff_ids.extend(ids_of(&bff.items));
        core_ids.extend(core.items.iter().map(|m| m.id.to_string()));

        match (bff.next_cursor, core.next_cursor) {
            (Some(next_bff), Some(next_core)) => {
                assert_eq!(next_bff, next_core);
                bff_cursor = Some(next_bff);
                core_cursor = Some(next_core);
            }
            (None, None) => break,
            (left, right) => panic!("курсоры разошлись: BFF {left:?}, ядро {right:?}"),
        }
    }

    assert_eq!(core_ids, bff_ids);
    assert_eq!(
        core_ids,
        ids_of(
            &data::messages_page(&root, NEWS_CHANNEL, None, None)
                .expect("все")
                .items
        )
    );
    let unique: std::collections::HashSet<_> = core_ids.iter().collect();
    assert_eq!(unique.len(), core_ids.len(), "страницы не пересекаются");

    // Хвост за последней строкой — пустая валидная страница, а не ошибка.
    let tail = core_bridge::core_messages_page(
        &root,
        NEWS_CHANNEL,
        core_ids.last().map(String::as_str),
        Some(MAX_PAGE_LIMIT),
    )
    .expect("хвост ядра");
    assert!(tail.items.is_empty());
    assert!(!tail.has_more);
    assert_eq!(tail.next_cursor, None);
    assert_eq!(tail.total, core_ids.len());
}

#[test]
fn empty_channel_page_is_valid() {
    let root = fixtures_or_skip!();

    // Оракул молчащего канала — READY: канал есть, сообщений в by-type нет.
    let ready = std::fs::read_to_string(root.join("gateway/by-type/002-READY.json"))
        .expect("READY читается");
    let value: serde_json::Value = serde_json::from_str(&ready).expect("READY — JSON");
    let mut with_messages = std::collections::HashSet::new();
    for entry in std::fs::read_dir(root.join("gateway/by-type")).expect("by-type читается")
    {
        let path = entry.expect("запись каталога").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("кадр читается");
        let frame: serde_json::Value = serde_json::from_str(&text).expect("кадр — JSON");
        if frame["t"]
            .as_str()
            .is_some_and(|kind| kind.starts_with("MESSAGE_"))
        {
            if let Some(id) = frame["d"]["channel_id"].as_str() {
                with_messages.insert(id.to_string());
            }
        }
    }
    let silent = value["d"]["guilds"]
        .as_array()
        .expect("guilds")
        .iter()
        .flat_map(|guild| guild["channels"].as_array().into_iter().flatten())
        .filter_map(|channel| channel["id"].as_str())
        .find(|id| !with_messages.contains(*id))
        .expect("в READY есть канал без сообщений");

    let core = core_bridge::core_messages_page(&root, silent, None, Some(10))
        .expect("известный тихий канал — пустая страница");
    assert!(core.items.is_empty());
    assert_eq!(core.total, 0);
    assert!(!core.has_more);
    assert_eq!(core.next_cursor, None);

    let bff = data::messages_page(&root, silent, None, Some(10)).expect("BFF-пустышка");
    assert_eq!(core.items.len(), bff.items.len());
    assert_eq!(core.has_more, bff.has_more);
    assert_eq!(core.total, bff.total);
}

#[test]
fn row_conversion_maps_contract_fields() {
    let root = fixtures_or_skip!();
    let page = data::messages_page(&root, NEWS_CHANNEL, None, Some(1)).expect("первая строка BFF");
    let row = page.items.first().expect("строка есть");

    let message = core_bridge::message_from_row(row).expect("конверсия реальной строки");
    assert_eq!(message.id.to_string(), row.id);
    assert_eq!(message.channel_id.to_string(), row.channel_id);
    assert_eq!(
        message.author.as_ref().map(|user| user.id.to_string()),
        Some(row.author.id.clone())
    );
    assert_eq!(
        message
            .author
            .as_ref()
            .and_then(|user| user.username.clone()),
        Some(row.author.username.clone())
    );
    assert_eq!(message.content, row.content);
    assert_eq!(message.timestamp, row.timestamp);
    assert_eq!(message.pinned, row.pinned);
    assert_eq!(message.message_type, 0, "type 0 — обычное сообщение");
    assert_eq!(message.edited_timestamp, row.edited_at);
    assert_eq!(message.embed_count, row.embeds.len());
    assert_eq!(message.attachment_count, row.attachments.len());
    assert_eq!(message.mention_user_ids.len(), row.mentions.len());
    assert_eq!(message.mention_everyone, row.mention_everyone);
}

#[test]
fn non_numeric_id_is_parse_error_not_panic() {
    let row = MessageRow {
        id: "not-a-snowflake".to_string(),
        channel_id: NEWS_CHANNEL.to_string(),
        guild_id: None,
        author: MessageAuthor {
            id: NEWS_AUTHOR.to_string(),
            username: "tester".to_string(),
            global_name: None,
            avatar: None,
            bot: false,
        },
        content: "текст".to_string(),
        timestamp: Some("2026-10-19T06:30:16.144000+00:00".to_string()),
        edited: false,
        edited_at: None,
        day: Some("2026-10-19".to_string()),
        group: "start".to_string(),
        pinned: false,
        message_type: 0,
        embeds: Vec::new(),
        attachments: Vec::new(),
        reactions: Vec::new(),
        reply_to: None,
        mentions: Vec::new(),
        mention_everyone: false,
        flags: None,
        call: None,
    };
    let err = core_bridge::message_from_row(&row).expect_err("нечисловой id — ошибка");
    assert_eq!(err.kind(), "Parse", "{err}");
}

#[test]
fn sanitized_negative_ids_are_rejected_as_parse() {
    let root = fixtures_or_skip!();
    // Санитизированные id дампа отрицательны: ядро (u64) их не представляет.
    // Мост обязан вернуть типизированный `Parse`, а не паниковать/терять строку.
    let err = core_bridge::core_messages_page(&root, "-1467273046216692736", None, Some(10))
        .expect_err("отрицательный id — Parse");
    assert_eq!(err.kind(), "Parse", "{err}");
}
