//! C-02-BEFORE: контракт older-history курсора (`before`/`position`) BFF.
//!
//! Оракул — сами санитизированные кадры `fixtures/gateway/by-type/*.json`,
//! а не текущая реализация: id строк канала, порядок `timestamp`, затем `id`,
//! дедуп по id. Канал `-2684694197495146496` выбран потому, что в фикстурах
//! у него 8 уникальных `MESSAGE_CREATE` — достаточно для трёх страниц по 2–3.
//!
//! Контракт, который проверяется:
//! - `cursor` — forward-курсор: строки строго **после** id (не меняется);
//! - `before` — older-курсор: не более `limit` строк строго **до** id, в том же
//!   возрастающем порядке, что у forward-страниц (UI делает prepend);
//! - `position: "latest"` — стартовая страница с конца (хвост), работает без
//!   знания id последнего сообщения;
//! - ответ добавляет `hasOlder`/`olderCursor`, не меняя `nextCursor`/`hasMore`;
//! - `olderCursor` — id первой строки страницы: следующий `before`;
//! - `cursor` + `before` и `position: "latest"` с любым курсором — `InvalidArg`;
//! - неизвестный/слишком длинный `before` — `InvalidArg`, `limit` — `1..=100`;
//! - `total`/`limit`/`loadedRowsLimit` — как раньше, без выдуманных полей.
//!
//! Skip-aware: без `fixtures/` тесты печатают причину и возвращаются,
//! при `DL_STRICT=1` — падают (CI-режим).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use discord_gui_spike::data::{
    self, DataError, MessagesPage, MessagesPageRequest, MessagesPosition, MAX_PAGE_LIMIT,
    UI_WINDOW_ROWS,
};
use serde_json::Value;

/// Канал с 8 уникальными сообщениями в фикстурах.
const CHANNEL: &str = "-2684694197495146496";

fn repo_fixtures_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo = manifest
        .parent()
        .and_then(Path::parent)
        .expect("crate лежит в rust/gui-spike");
    repo.join("fixtures")
}

fn candidate_root() -> PathBuf {
    std::env::var("DL_FIXTURES")
        .ok()
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(repo_fixtures_root)
}

fn fixtures_root() -> Option<PathBuf> {
    let root = candidate_root();
    if root.join("gateway/by-type/002-READY.json").is_file() {
        Some(root)
    } else {
        None
    }
}

/// `None` — фикстур нет: печатаем причину и выходим; при `DL_STRICT=1` — паника.
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
                    "SKIP history_before_contract: нет фикстур в {} (DL_STRICT≠1)",
                    candidate_root().display()
                );
                return;
            }
        }
    };
}

/// id+timestamp всех сообщений канала из сырых кадров (дедуп по id, last wins).
fn oracle_rows(root: &Path) -> Vec<(String, Option<String>)> {
    let dir = root.join("gateway/by-type");
    let mut frames: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("by-type читается")
        .map(|entry| entry.expect("запись by-type").path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect();
    frames.sort();

    let mut seen: HashMap<String, Option<String>> = HashMap::new();
    for path in frames {
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("не читается {}: {err}", path.display()));
        let value: Value = serde_json::from_str(&text)
            .unwrap_or_else(|err| panic!("не парсится {}: {err}", path.display()));
        let Some(kind) = value["t"].as_str() else {
            continue;
        };
        if !kind.starts_with("MESSAGE_") || kind == "MESSAGE_ACK" {
            continue;
        }
        let body = &value["d"];
        if body["channel_id"].as_str() != Some(CHANNEL) {
            continue;
        }
        let Some(id) = body["id"].as_str() else {
            continue;
        };
        seen.insert(
            id.to_string(),
            body["timestamp"].as_str().map(str::to_string),
        );
    }

    let mut rows: Vec<(String, Option<String>)> = seen.into_iter().collect();
    rows.sort_by(|left, right| left.1.cmp(&right.1).then_with(|| left.0.cmp(&right.0)));
    rows
}

/// Оракул id канала: ровно то, что обязан отдать backend (порядок сохраняется).
fn oracle_ids(root: &Path) -> Vec<String> {
    let ids: Vec<String> = oracle_rows(root).into_iter().map(|(id, _)| id).collect();
    assert_eq!(
        ids.len(),
        8,
        "в канале {CHANNEL} ожидаются 8 уникальных строк"
    );
    ids
}

/// Каналы, у которых в кадрах есть хотя бы одно сообщение (для тихого канала).
fn channels_with_messages(root: &Path) -> HashSet<String> {
    let dir = root.join("gateway/by-type");
    let mut channels = HashSet::new();
    for entry in std::fs::read_dir(&dir).expect("by-type читается") {
        let path = entry.expect("запись by-type").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("не читается {}: {err}", path.display()));
        let value: Value = serde_json::from_str(&text)
            .unwrap_or_else(|err| panic!("не парсится {}: {err}", path.display()));
        let Some(kind) = value["t"].as_str() else {
            continue;
        };
        if !kind.starts_with("MESSAGE_") || kind == "MESSAGE_ACK" {
            continue;
        }
        if let Some(channel) = value["d"]["channel_id"].as_str() {
            channels.insert(channel.to_string());
        }
    }
    channels
}

/// Первый канал READY без сообщений — тихий канал фикстур.
fn silent_channel(root: &Path) -> String {
    let text = std::fs::read_to_string(root.join("gateway/by-type/002-READY.json"))
        .expect("READY читается");
    let ready: Value = serde_json::from_str(&text).expect("READY парсится");
    let with_messages = channels_with_messages(root);
    ready["d"]["guilds"]
        .as_array()
        .expect("READY.guilds")
        .iter()
        .flat_map(|guild| guild["channels"].as_array().into_iter().flatten())
        .filter_map(|channel| channel["id"].as_str())
        .find(|id| !with_messages.contains(*id))
        .expect("в READY есть канал без сообщений")
        .to_string()
}

fn request<'a>(
    cursor: Option<&'a str>,
    before: Option<&'a str>,
    position: MessagesPosition,
    limit: Option<usize>,
) -> MessagesPageRequest<'a> {
    MessagesPageRequest {
        cursor,
        before,
        position,
        limit,
    }
}

fn page(root: &Path, req: MessagesPageRequest<'_>) -> MessagesPage {
    data::messages_page_request(root, CHANNEL, req).expect("страница канала")
}

fn ids_of(page: &MessagesPage) -> Vec<String> {
    page.items.iter().map(|row| row.id.clone()).collect()
}

fn kind(err: &DataError) -> &'static str {
    err.kind()
}

// ---------------------------------------------------------------------------
// before: страница строго старше якоря
// ---------------------------------------------------------------------------

#[test]
fn before_anchor_returns_immediately_older_rows_ascending() {
    let root = fixtures_or_skip!();
    let ids = oracle_ids(&root);

    // Якорь — 5-я строка; страница из 2 строк обязана быть ровно [2..4].
    let older = page(
        &root,
        request(None, Some(&ids[4]), MessagesPosition::Start, Some(2)),
    );
    assert_eq!(ids_of(&older), ids[2..4].to_vec(), "строки строго до якоря");
    assert_eq!(older.channel_id, CHANNEL);
    assert_eq!(older.total, 8, "total — весь канал, не страница");
    assert_eq!(older.limit, 2);
    assert_eq!(older.loaded_rows_limit, UI_WINDOW_ROWS);
    assert!(older.has_older, "до страницы есть ещё строки");
    assert_eq!(
        older.older_cursor.as_deref(),
        Some(ids[2].as_str()),
        "olderCursor — id первой строки: следующий before"
    );
    assert!(older.has_more, "после страницы есть строки новее");
    assert_eq!(
        older.next_cursor.as_deref(),
        Some(ids[3].as_str()),
        "nextCursor остаётся id последней строки страницы"
    );
}

#[test]
fn before_paging_walks_back_to_start_without_overlap() {
    let root = fixtures_or_skip!();
    let ids = oracle_ids(&root);

    let mut before: Option<String> = Some(ids[4].clone());
    let mut collected: Vec<String> = Vec::new();
    let mut pages = 0;
    while let Some(anchor) = before {
        let current = page(
            &root,
            request(None, Some(&anchor), MessagesPosition::Start, Some(2)),
        );
        pages += 1;
        assert!(pages <= 8, "обход не сходится к началу: цикл");
        let page_ids = ids_of(&current);
        assert!(
            !page_ids.is_empty(),
            "непустой якорь с индексом > 0 даёт непустую страницу"
        );
        // Страница всегда целиком старше якоря и в возрастающем порядке.
        let anchor_index = ids.iter().position(|id| id == &anchor).expect("якорь");
        let expected_start = anchor_index.saturating_sub(2);
        assert_eq!(page_ids, ids[expected_start..anchor_index].to_vec());
        collected.splice(0..0, page_ids.clone());
        before = current
            .has_older
            .then(|| current.older_cursor.clone())
            .flatten();
        if before.is_none() {
            assert!(!current.has_older);
            assert_eq!(current.older_cursor, None);
        }
    }

    assert_eq!(
        collected,
        ids[..4].to_vec(),
        "страницы покрывают начало без дублей"
    );
    let unique: HashSet<&String> = collected.iter().collect();
    assert_eq!(unique.len(), collected.len(), "дубли между страницами");

    // Самая старая строка: старше неё ничего нет — пустая страница, не ошибка.
    let start = page(
        &root,
        request(None, Some(&ids[0]), MessagesPosition::Start, Some(2)),
    );
    assert!(start.items.is_empty());
    assert!(!start.has_older, "старше первой строки данных нет");
    assert_eq!(start.older_cursor, None, "нечего листать дальше");
    assert!(!start.has_more, "пустая страница не обещает nextCursor");
    assert_eq!(start.next_cursor, None);
}

#[test]
fn before_page_rows_match_forward_rows_exactly() {
    let root = fixtures_or_skip!();
    let ids = oracle_ids(&root);

    // Полный forward-обход (существующий контракт) — эталон строк.
    let mut forward_rows = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let current = page(
            &root,
            request(cursor.as_deref(), None, MessagesPosition::Start, Some(3)),
        );
        forward_rows.extend(current.items.iter().cloned());
        match current.next_cursor {
            Some(next) if current.has_more => cursor = Some(next),
            _ => break,
        }
    }
    assert_eq!(forward_rows.len(), ids.len());

    let older = page(
        &root,
        request(None, Some(&ids[6]), MessagesPosition::Start, Some(3)),
    );
    assert_eq!(older.items, forward_rows[3..6].to_vec());
}

// ---------------------------------------------------------------------------
// position: latest — старт с конца без id последнего сообщения
// ---------------------------------------------------------------------------

#[test]
fn latest_position_starts_at_newest_rows_and_pages_backwards() {
    let root = fixtures_or_skip!();
    let ids = oracle_ids(&root);

    let latest = page(
        &root,
        request(None, None, MessagesPosition::Latest, Some(3)),
    );
    assert_eq!(
        ids_of(&latest),
        ids[5..8].to_vec(),
        "хвост канала, порядок backend"
    );
    assert!(!latest.has_more, "после хвоста строк нет");
    assert_eq!(latest.next_cursor, None);
    assert!(latest.has_older, "до хвоста есть строки");
    assert_eq!(latest.older_cursor.as_deref(), Some(ids[5].as_str()));

    let middle = page(
        &root,
        request(None, Some(&ids[5]), MessagesPosition::Start, Some(3)),
    );
    assert_eq!(ids_of(&middle), ids[2..5].to_vec());
    assert!(middle.has_older);

    let start = page(
        &root,
        request(None, Some(&ids[2]), MessagesPosition::Start, Some(3)),
    );
    assert_eq!(
        ids_of(&start),
        ids[0..2].to_vec(),
        "первая страница начинается с начала"
    );
    assert!(!start.has_older);
    assert_eq!(start.older_cursor, None);
}

#[test]
fn latest_position_on_channel_larger_than_limit_is_a_single_tail_page() {
    let root = fixtures_or_skip!();
    let ids = oracle_ids(&root);

    let latest = page(
        &root,
        request(None, None, MessagesPosition::Latest, Some(MAX_PAGE_LIMIT)),
    );
    assert_eq!(
        ids_of(&latest),
        ids.clone(),
        "8 строк < limit — отдаются все"
    );
    assert!(!latest.has_older, "больше нечего листать вверх");
    assert_eq!(latest.older_cursor, None);
    assert!(!latest.has_more);
    assert_eq!(latest.next_cursor, None);
}

#[test]
fn latest_position_on_silent_channel_has_no_cursors() {
    let root = fixtures_or_skip!();
    let channel = silent_channel(&root);

    let latest = data::messages_page_request(
        &root,
        &channel,
        request(None, None, MessagesPosition::Latest, Some(3)),
    )
    .expect("тихий канал — пустая страница, не ошибка");
    assert!(latest.items.is_empty());
    assert_eq!(latest.total, 0);
    assert!(!latest.has_more);
    assert_eq!(latest.next_cursor, None);
    assert!(!latest.has_older, "в пустом канале нет строк старше");
    assert_eq!(latest.older_cursor, None, "курсор не выдумывается");
}

// ---------------------------------------------------------------------------
// forward-контракт не сломан
// ---------------------------------------------------------------------------

#[test]
fn forward_paging_is_unchanged_and_gains_older_metadata() {
    let root = fixtures_or_skip!();
    let ids = oracle_ids(&root);

    let first = page(&root, request(None, None, MessagesPosition::Start, Some(3)));
    assert_eq!(ids_of(&first), ids[0..3].to_vec());
    assert!(first.has_more);
    assert_eq!(first.next_cursor.as_deref(), Some(ids[2].as_str()));
    assert!(!first.has_older, "перед началом истории строк нет");
    assert_eq!(first.older_cursor, None);

    let second = page(
        &root,
        request(
            first.next_cursor.as_deref(),
            None,
            MessagesPosition::Start,
            Some(3),
        ),
    );
    assert_eq!(ids_of(&second), ids[3..6].to_vec());
    assert!(second.has_more);
    assert_eq!(second.next_cursor.as_deref(), Some(ids[5].as_str()));
    assert!(second.has_older);
    assert_eq!(second.older_cursor.as_deref(), Some(ids[3].as_str()));

    // Старый вход `data::messages_page` — ровно тот же ответ, что и request-форма.
    let legacy = data::messages_page(&root, CHANNEL, None, Some(3)).expect("forward");
    assert_eq!(legacy, first, "старая сигнатура не изменила ответ");
}

// ---------------------------------------------------------------------------
// ошибки и границы
// ---------------------------------------------------------------------------

#[test]
fn mutually_exclusive_anchors_are_invalid_arg() {
    let root = fixtures_or_skip!();
    let ids = oracle_ids(&root);

    let err = data::messages_page_request(
        &root,
        CHANNEL,
        request(
            Some(&ids[1]),
            Some(&ids[4]),
            MessagesPosition::Start,
            Some(2),
        ),
    )
    .expect_err("cursor+before");
    assert_eq!(kind(&err), "InvalidArg", "{err}");
    assert!(err.to_string().contains("взаимоисключающие"), "{err}");

    for req in [
        request(Some(&ids[1]), None, MessagesPosition::Latest, Some(2)),
        request(None, Some(&ids[4]), MessagesPosition::Latest, Some(2)),
    ] {
        let err = data::messages_page_request(&root, CHANNEL, req).expect_err("latest + курсор");
        assert_eq!(kind(&err), "InvalidArg", "{err}");
    }
}

#[test]
fn unknown_position_and_anchor_are_typed_errors() {
    // Позиция разбирается до чтения фикстур: парсер — часть контракта.
    let err = MessagesPosition::parse(Some("middle")).expect_err("неизвестная позиция");
    assert_eq!(kind(&err), "InvalidArg", "{err}");
    assert!(err.to_string().contains("start"), "{err}");
    assert!(err.to_string().contains("latest"), "{err}");
    assert_eq!(
        MessagesPosition::parse(Some(" start ")).expect("start"),
        MessagesPosition::Start
    );
    assert_eq!(
        MessagesPosition::parse(None).expect("по умолчанию start"),
        MessagesPosition::Start
    );

    let root = fixtures_or_skip!();
    let err = data::messages_page_request(
        &root,
        CHANNEL,
        request(
            None,
            Some("нет-такого-id"),
            MessagesPosition::Start,
            Some(2),
        ),
    )
    .expect_err("чужой before");
    assert_eq!(kind(&err), "InvalidArg", "{err}");
    assert!(err.to_string().contains("before"), "{err}");

    let long = "x".repeat(257);
    let err = data::messages_page_request(
        &root,
        CHANNEL,
        request(None, Some(&long), MessagesPosition::Start, Some(2)),
    )
    .expect_err("длинный before");
    assert_eq!(kind(&err), "InvalidArg", "{err}");

    // Пустой/пробельный before — «якоря нет», как пустой cursor: это не ошибка.
    let blank = page(
        &root,
        request(None, Some("   "), MessagesPosition::Start, Some(3)),
    );
    assert_eq!(ids_of(&blank), oracle_ids(&root)[0..3].to_vec());
}

#[test]
fn older_page_limit_bounds_and_window_invariant() {
    let root = fixtures_or_skip!();
    let ids = oracle_ids(&root);

    for limit in [0, MAX_PAGE_LIMIT + 1] {
        let err = data::messages_page_request(
            &root,
            CHANNEL,
            request(None, Some(&ids[4]), MessagesPosition::Start, Some(limit)),
        )
        .expect_err("limit вне 1..=MAX_PAGE_LIMIT");
        assert_eq!(kind(&err), "InvalidArg", "limit={limit}: {err}");
    }

    let all = page(
        &root,
        request(
            None,
            Some(&ids[7]),
            MessagesPosition::Start,
            Some(MAX_PAGE_LIMIT),
        ),
    );
    assert_eq!(ids_of(&all), ids[0..7].to_vec());
    assert_eq!(all.limit, MAX_PAGE_LIMIT);
    assert_eq!(
        all.loaded_rows_limit, UI_WINDOW_ROWS,
        "окно UI ≤ 500 — инвариант"
    );
}

// ---------------------------------------------------------------------------
// dedup/merge на older-странице (временные кадры, без зависимости от fixtures)
// ---------------------------------------------------------------------------

/// Временный корень: минимальный READY + кадры сообщений.
fn temp_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("dl-c02-before-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("gateway/by-type")).expect("временный каталог");
    let ready = serde_json::json!({
        "t": "READY",
        "d": {
            "user": {"id": "1", "username": "tester"},
            "guilds": [{
                "id": "10",
                "properties": {"name": "Тестовая гильдия"},
                "channels": [{"id": "77", "name": "history"}],
            }],
            "private_channels": [],
        },
    });
    std::fs::write(
        root.join("gateway/by-type/002-READY.json"),
        ready.to_string(),
    )
    .expect("READY записан");
    root
}

fn write_message(root: &Path, file: &str, kind: &str, id: &str, timestamp: &str, content: &str) {
    let frame = serde_json::json!({
        "t": kind,
        "d": {
            "id": id,
            "channel_id": "77",
            "author": {"id": "5", "username": "author"},
            "content": content,
            "timestamp": timestamp,
            "pinned": false,
            "type": 0,
        },
    });
    std::fs::write(root.join("gateway/by-type").join(file), frame.to_string())
        .expect("кадр записан");
}

#[test]
fn older_page_dedups_by_id_and_applies_update_without_duplicate_rows() {
    let root = temp_root("dedup");
    write_message(
        &root,
        "005-MESSAGE_CREATE.json",
        "MESSAGE_CREATE",
        "3",
        "2026-10-19T03:00:00+00:00",
        "третье",
    );
    write_message(
        &root,
        "006-MESSAGE_CREATE.json",
        "MESSAGE_CREATE",
        "1",
        "2026-10-19T01:00:00+00:00",
        "первое",
    );
    write_message(
        &root,
        "007-MESSAGE_CREATE.json",
        "MESSAGE_CREATE",
        "2",
        "2026-10-19T02:00:00+00:00",
        "второе",
    );
    write_message(
        &root,
        "008-MESSAGE_UPDATE.json",
        "MESSAGE_UPDATE",
        "2",
        "2026-10-19T02:00:00+00:00",
        "второе (правка)",
    );

    let requested = request(None, Some("3"), MessagesPosition::Start, Some(1));
    let older = data::messages_page_request(&root, "77", requested).expect("older-страница");
    assert_eq!(
        ids_of(&older),
        ["2".to_string()],
        "дубль UPDATE не создаёт строку"
    );
    assert_eq!(older.total, 3, "дедуп по id: 4 кадра → 3 строки");
    assert_eq!(
        older.items[0].content, "второе (правка)",
        "UPDATE уточняет строку"
    );
    assert!(older.items[0].edited, "edited из UPDATE");
    assert!(older.has_older, "строка id=1 старше страницы");
    assert_eq!(older.older_cursor.as_deref(), Some("2"));
    assert!(older.has_more, "новее страницы есть строки");

    let next = data::messages_page_request(
        &root,
        "77",
        request(None, Some("2"), MessagesPosition::Start, Some(1)),
    )
    .expect("вторая older-страница");
    assert_eq!(ids_of(&next), ["1".to_string()]);
    assert!(
        !next.has_older,
        "страница начинается с первой строки канала"
    );
    assert_eq!(next.older_cursor, None);

    let tail = data::messages_page_request(
        &root,
        "77",
        request(None, Some("1"), MessagesPosition::Start, Some(1)),
    )
    .expect("последняя older-страница");
    assert!(tail.items.is_empty());
    assert!(!tail.has_older);
    assert_eq!(tail.older_cursor, None);

    let _ = std::fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// сериализация: ровно документированные поля, camelCase
// ---------------------------------------------------------------------------

#[test]
fn older_page_serializes_documented_fields_only() {
    let root = fixtures_or_skip!();
    let ids = oracle_ids(&root);

    let older = page(
        &root,
        request(None, Some(&ids[4]), MessagesPosition::Start, Some(2)),
    );
    let value = serde_json::to_value(&older).expect("MessagesPage → JSON");
    let mut keys: Vec<&str> = value
        .as_object()
        .expect("объект")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "channelId",
            "hasMore",
            "hasOlder",
            "items",
            "limit",
            "loadedRowsLimit",
            "nextCursor",
            "olderCursor",
            "total",
        ],
        "новые поля только hasOlder/olderCursor; выдуманных нет"
    );
    assert_eq!(value["hasOlder"], true);
    assert_eq!(value["olderCursor"], ids[2].as_str());
    assert_eq!(value["nextCursor"], ids[3].as_str());
    assert_eq!(value["items"][0]["id"], ids[2].as_str());
}
