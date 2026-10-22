//! Контракт P2-03 (RW-31, W31b): SQLite-store через FFI к системному `libsqlite3`.
//!
//! Кейсы s01–s13 из `waves/RW-31/TZ-W31b.md` §3 (плюс s14 — курсор неизвестного сообщения).
//! Независимый оракул — Python 3 `sqlite3` (stdlib, `/usr/lib/.../libsqlite3` 3.54): тест s12
//! открывает созданный Rust-кодом файл и сверяет `user_version`, `journal_mode`, схему,
//! индексы, данные и отсутствие дублей. Сверка идёт с файлом, а не с Rust-API.
//!
//! Тесты SKIP-aware: если в среде нет `python3` с модулем `sqlite3`, s12 печатает
//! `SKIP: <причина>` и завершается успешно, а не падает.

use discord_store::{ChannelRow, MessageRow, Store, StoreError};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const CHANNEL: &str = "ch-1";

// --- фикстуры -------------------------------------------------------------------

fn channel_row(id: &str) -> ChannelRow {
    ChannelRow {
        id: id.to_owned(),
        guild_id: Some("g-1".to_owned()),
        name: Some("общий канал".to_owned()),
        kind: 0,
        position: 0,
    }
}

fn message_row(id: &str, channel_id: &str, timestamp: i64) -> MessageRow {
    MessageRow {
        id: id.to_owned(),
        channel_id: channel_id.to_owned(),
        author_id: "u-1".to_owned(),
        content: format!("сообщение {id}"),
        timestamp,
        edited_timestamp: None,
        flags: 0,
        deleted: false,
    }
}

/// Готовое хранилище с миграцией и одним каналом.
fn store_with_channel() -> Store {
    let mut store = Store::open_in_memory().expect("open_in_memory");
    store.migrate().expect("migrate");
    store
        .put_channel(&channel_row(CHANNEL))
        .expect("put_channel");
    store
}

// --- временный каталог без внешних крейтов --------------------------------------

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(tag: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let serial = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "discord-store-w31b-{}-{tag}-{serial}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("временный каталог");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn db_path(&self) -> PathBuf {
        self.path.join("store.db")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // best-effort: уборка временного каталога не должна ломать тест
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

// --- s01 ------------------------------------------------------------------------

#[test]
fn s01_migrate_sets_schema_version_and_is_idempotent() {
    let mut store = Store::open_in_memory().expect("open_in_memory");
    assert_eq!(
        store.schema_version().expect("schema_version"),
        0,
        "до migrate схема пуста"
    );

    store.migrate().expect("первый migrate");
    assert_eq!(store.schema_version().expect("schema_version"), 1);

    store.migrate().expect("повторный migrate — no-op");
    assert_eq!(store.schema_version().expect("schema_version"), 1);

    store.migrate().expect("третий migrate");
    assert_eq!(store.schema_version().expect("schema_version"), 1);
}

// --- s02 ------------------------------------------------------------------------

#[test]
fn s02_channel_upsert_roundtrip() {
    let mut store = Store::open_in_memory().expect("open_in_memory");
    store.migrate().expect("migrate");
    assert!(store.channel("ch-1").expect("channel").is_none());

    let original = ChannelRow {
        id: "ch-1".to_owned(),
        guild_id: Some("g-1".to_owned()),
        name: Some("общий «канал» ✓ 名称".to_owned()),
        kind: 0,
        position: 7,
    };
    store.put_channel(&original).expect("put_channel");
    assert_eq!(
        store.channel("ch-1").expect("channel"),
        Some(original.clone()),
        "данные и Unicode читаются дословно"
    );

    let updated = ChannelRow {
        id: "ch-1".to_owned(),
        guild_id: None,
        name: None,
        kind: 2,
        position: 3,
    };
    store.put_channel(&updated).expect("upsert channel");
    assert_eq!(
        store.channel("ch-1").expect("channel"),
        Some(updated),
        "upsert обновляет строку, а не дублирует её"
    );
}

// --- s03 ------------------------------------------------------------------------

#[test]
fn s03_message_upsert_without_duplicates() {
    let mut store = store_with_channel();
    let mut row = message_row("m-1", CHANNEL, 1_000);
    row.content = "первая редакция".to_owned();
    store.put_message(&row).expect("put_message");

    let mut edited = row.clone();
    edited.content = "вторая редакция ' ✓".to_owned();
    edited.edited_timestamp = Some(1_001);
    edited.flags = 3;
    store.put_message(&edited).expect("upsert message");
    store.put_message(&edited).expect("повторный upsert");

    let rows = store.messages(CHANNEL, None, 10).expect("messages");
    assert_eq!(rows.len(), 1, "upsert по id не создаёт дублей");
    assert_eq!(rows[0], edited);
}

// --- s04 ------------------------------------------------------------------------

#[test]
fn s04_messages_order_limit_zero_and_clamp() {
    let mut store = store_with_channel();
    for index in 0..5 {
        let id = format!("m-{index}");
        store
            .put_message(&message_row(&id, CHANNEL, 100 + index))
            .expect("put_message");
    }

    let rows = store.messages(CHANNEL, None, 10).expect("messages");
    let ids: Vec<&str> = rows.iter().map(|row| row.id.as_str()).collect();
    assert_eq!(ids, ["m-4", "m-3", "m-2", "m-1", "m-0"], "порядок DESC");
    let timestamps: Vec<i64> = rows.iter().map(|row| row.timestamp).collect();
    assert_eq!(timestamps, [104, 103, 102, 101, 100]);

    assert!(
        store
            .messages(CHANNEL, None, 0)
            .expect("limit=0")
            .is_empty(),
        "limit == 0 → пустой вектор"
    );

    for index in 5..1_005 {
        let id = format!("m-{index:04}");
        store
            .put_message(&message_row(&id, CHANNEL, 100 + index))
            .expect("put_message");
    }
    let clamped = store.messages(CHANNEL, None, 5_000).expect("clamp");
    assert_eq!(clamped.len(), 1_000, "limit > 1000 клампится до 1000");
    assert_eq!(clamped[0].id, "m-1004", "первое — самое новое сообщение");
    assert_eq!(
        store.messages(CHANNEL, None, 1_001).expect("clamp").len(),
        1_000
    );
}

// --- s05 ------------------------------------------------------------------------

#[test]
fn s05_keyset_pagination_is_strict_and_gapless() {
    let mut store = store_with_channel();
    // Пары одинаковых timestamp: проверяем второй ключ (id) в курсоре.
    let plan: [(i64, &str); 10] = [
        (1_000, "m-00"),
        (999, "m-01"),
        (999, "m-02"),
        (999, "m-03"),
        (998, "m-04"),
        (998, "m-05"),
        (997, "m-06"),
        (996, "m-07"),
        (995, "m-08"),
        (994, "m-09"),
    ];
    for (timestamp, id) in plan {
        store
            .put_message(&message_row(id, CHANNEL, timestamp))
            .expect("put_message");
    }
    // ORDER BY timestamp DESC, id DESC
    let expected = [
        "m-00", "m-03", "m-02", "m-01", "m-05", "m-04", "m-06", "m-07", "m-08", "m-09",
    ];

    let mut seen: Vec<String> = Vec::new();
    let mut before: Option<String> = None;
    let mut pages = 0;
    loop {
        let page = store
            .messages(CHANNEL, before.as_deref(), 3)
            .expect("messages");
        if page.is_empty() {
            break;
        }
        pages += 1;
        assert!(page.len() <= 3, "limit соблюдается");
        assert!(pages <= 4, "пагинация не должна зацикливаться");
        seen.extend(page.iter().map(|row| row.id.clone()));
        before = page.last().map(|row| row.id.clone());
    }

    assert_eq!(pages, 4, "10 сообщений по 3 = 4 страницы");
    let seen_refs: Vec<&str> = seen.iter().map(String::as_str).collect();
    assert_eq!(seen_refs, expected, "страницы без пропусков и дублей");

    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        seen.len(),
        "одно и то же сообщение не повторяется"
    );
}

// --- s06 ------------------------------------------------------------------------

#[test]
fn s06_message_without_channel_is_constraint() {
    let mut store = Store::open_in_memory().expect("open_in_memory");
    store.migrate().expect("migrate");

    let error = store
        .put_message(&message_row("m-1", "missing-channel", 1))
        .expect_err("FK должен отвергнуть сообщение без канала");
    assert!(
        matches!(error, StoreError::Constraint(_)),
        "ожидали StoreError::Constraint, получили {error:?}"
    );

    // Хранилище остаётся работоспособным после ошибки.
    store
        .put_channel(&channel_row("missing-channel"))
        .expect("put_channel");
    store
        .put_message(&message_row("m-1", "missing-channel", 1))
        .expect("после появления канала сообщение сохраняется");
    assert_eq!(
        store
            .messages("missing-channel", None, 10)
            .expect("messages")
            .len(),
        1
    );
}

// --- s07 ------------------------------------------------------------------------

#[test]
fn s07_soft_delete_hides_message() {
    let mut store = store_with_channel();
    store
        .put_message(&message_row("m-1", CHANNEL, 1))
        .expect("put_message");
    store
        .put_message(&message_row("m-2", CHANNEL, 2))
        .expect("put_message");

    assert!(store.delete_message("m-1").expect("delete_message"));
    let rows = store.messages(CHANNEL, None, 10).expect("messages");
    assert_eq!(
        rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
        ["m-2"],
        "soft-deleted сообщение не отдаётся"
    );
    assert!(
        store.delete_message("m-1").expect("повторное удаление"),
        "строка существует (уже помечена) → true"
    );
    assert!(
        !store
            .delete_message("no-such-message")
            .expect("отсутствующая"),
        "отсутствующая строка → false"
    );
    assert_eq!(
        store.messages(CHANNEL, None, 10).expect("messages").len(),
        1,
        "повторное удаление не задевает другие строки"
    );
}

// --- s08 ------------------------------------------------------------------------

#[test]
fn s08_kv_set_get_overwrite() {
    let mut store = Store::open_in_memory().expect("open_in_memory");
    store.migrate().expect("migrate");
    assert_eq!(store.kv_get("k").expect("kv_get"), None);

    store.kv_set("k", "значение ✓").expect("kv_set");
    assert_eq!(
        store.kv_get("k").expect("kv_get"),
        Some("значение ✓".to_owned())
    );

    store.kv_set("k", "перезаписано '); --").expect("kv_set");
    assert_eq!(
        store.kv_get("k").expect("kv_get"),
        Some("перезаписано '); --".to_owned()),
        "значение перезаписано"
    );
    assert_eq!(store.kv_get("другой").expect("kv_get"), None);
}

// --- s09 ------------------------------------------------------------------------

#[test]
fn s09_reopen_file_keeps_data_and_version() {
    let dir = TempDir::new("reopen");
    let path = dir.db_path();

    let channel = channel_row(CHANNEL);
    let message = message_row("m-1", CHANNEL, 42);
    {
        let mut store = Store::open(&path).expect("open");
        store.migrate().expect("migrate");
        store.put_channel(&channel).expect("put_channel");
        store.put_message(&message).expect("put_message");
        store.kv_set("k", "v").expect("kv_set");
        assert_eq!(store.schema_version().expect("schema_version"), 1);
    }

    let mut store = Store::open(&path).expect("reopen");
    assert_eq!(
        store.schema_version().expect("schema_version"),
        1,
        "user_version переживает закрытие"
    );
    store.migrate().expect("migrate после reopen");
    assert_eq!(store.channel(CHANNEL).expect("channel"), Some(channel));
    assert_eq!(
        store.messages(CHANNEL, None, 10).expect("messages"),
        [message],
        "данные переживают закрытие"
    );
    assert_eq!(store.kv_get("k").expect("kv_get"), Some("v".to_owned()));
}

// --- s10 ------------------------------------------------------------------------

#[test]
fn s10_sql_injection_is_stored_literally() {
    let mut store = Store::open_in_memory().expect("open_in_memory");
    store.migrate().expect("migrate");

    let hostile_channel = "ch'; DROP TABLE messages; --";
    let first_id = "m-1'; DELETE FROM messages; --";
    let second_id = "m-2 ✓";
    let content = "текст '); DROP TABLE kv; -- ; ✓";

    store
        .put_channel(&channel_row(hostile_channel))
        .expect("put_channel");
    let mut first = message_row(first_id, hostile_channel, 10);
    first.content = content.to_owned();
    store.put_message(&first).expect("put_message");

    let again = store.messages(hostile_channel, None, 10).expect("messages");
    assert_eq!(again.len(), 1);
    assert_eq!(again[0].id, first_id, "id сохранён дословно");
    assert_eq!(again[0].content, content, "content сохранён дословно");

    store
        .put_message(&message_row(second_id, hostile_channel, 20))
        .expect("put_message");
    // Ветка с курсором: сюда попадает конкатенация SQL со значением, если её внести.
    let older = store
        .messages(hostile_channel, Some(second_id), 10)
        .expect("messages before");
    assert_eq!(
        older.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
        [first_id],
        "keyset-пагинация работает с враждебным channel_id"
    );

    assert_eq!(store.schema_version().expect("schema_version"), 1);
    store.kv_set("k", "v").expect("kv_set");
    assert_eq!(store.kv_get("k").expect("kv_get"), Some("v".to_owned()));
    assert_eq!(
        store
            .messages(hostile_channel, None, 10)
            .expect("messages")
            .len(),
        2,
        "таблица messages цела"
    );
}

// --- s11 ------------------------------------------------------------------------

#[test]
fn s11_missing_directory_is_open_error() {
    let dir = TempDir::new("missing");
    let path = dir.path().join("нет-такого-каталога").join("store.db");
    let error = Store::open(&path).expect_err("несуществующий каталог → ошибка, не паника");
    assert!(
        matches!(error, StoreError::Open(_)),
        "ожидали StoreError::Open, получили {error:?}"
    );
}

// --- s12 ------------------------------------------------------------------------

#[test]
fn s12_python_oracle_matches_file() {
    let Some(version) = python_sqlite_version() else {
        println!("SKIP: python3 с модулем sqlite3 недоступен — оракул s12 не проверен");
        return;
    };
    println!("оракул: python3 sqlite3 {version}");

    let dir = TempDir::new("oracle");
    let path = dir.db_path();

    let channel = ChannelRow {
        id: CHANNEL.to_owned(),
        guild_id: Some("g-1".to_owned()),
        name: Some("общий канал".to_owned()),
        kind: 0,
        position: 2,
    };
    let first = {
        let mut row = message_row("m-1", CHANNEL, 10);
        row.content = "первое ✓".to_owned();
        row
    };
    let mut second = message_row("m-2", CHANNEL, 20);
    second.content = "второе '); --".to_owned();
    second.edited_timestamp = Some(21);
    second.flags = 64;
    let mut third = message_row("m-3", CHANNEL, 30);
    third.content = "третье\nс переводом строки".to_owned();

    {
        let mut store = Store::open(&path).expect("open");
        store.migrate().expect("migrate");
        store.put_channel(&channel).expect("put_channel");
        store.put_message(&first).expect("put_message");
        store.put_message(&second).expect("put_message");
        store.put_message(&third).expect("put_message");
        store.kv_set("greeting", "привет мир").expect("kv_set");
    }

    let script = oracle_script();
    assert!(script.is_file(), "нет оракула {}", script.display());
    let output = Command::new("python3")
        .arg(&script)
        .arg(&path)
        .output()
        .expect("запуск python3-оракула");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    println!("ответ оракула:\n{stdout}");
    assert!(
        output.status.success(),
        "оракул упал ({}): {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let report = parse_oracle(&stdout);
    assert_eq!(oracle_value(&report, "user_version"), "1");
    assert_eq!(oracle_value(&report, "journal_mode"), "wal");
    assert_eq!(oracle_value(&report, "integrity_check"), "ok");
    assert_eq!(oracle_value(&report, "foreign_key_violations"), "0");
    assert_eq!(oracle_value(&report, "duplicate_ids"), "0");
    assert_eq!(
        oracle_value(&report, "order_matches"),
        "1",
        "SQL ORDER BY совпадает с сортировкой кортежей в Python"
    );
    assert_eq!(
        oracle_value(&report, "keyset_matches"),
        "1",
        "SQL keyset-пагинация совпадает с фильтром Python"
    );
    assert_eq!(oracle_value(&report, "keyset_size"), "2");
    assert_eq!(
        oracle_lines(&report, "table"),
        ["channels", "kv", "messages"],
        "схема v1: только прикладные таблицы"
    );
    assert!(
        oracle_lines(&report, "index")
            .contains(&"messages|idx_messages_channel_timestamp".to_owned()),
        "индекс (channel_id, timestamp DESC): {:?}",
        oracle_lines(&report, "index")
    );
    assert!(
        oracle_lines(&report, "index").contains(&"messages|idx_messages_channel_id".to_owned()),
        "индекс (channel_id, id DESC): {:?}",
        oracle_lines(&report, "index")
    );
    assert!(
        oracle_lines(&report, "index_column")
            .contains(&"idx_messages_channel_timestamp|0|channel_id".to_owned())
            && oracle_lines(&report, "index_column")
                .contains(&"idx_messages_channel_timestamp|1|timestamp".to_owned()),
        "колонки индекса по времени: {:?}",
        oracle_lines(&report, "index_column")
    );
    assert!(
        oracle_lines(&report, "column").contains(&"channels|guild_id|TEXT|0|0".to_owned()),
        "guild_id — nullable TEXT: {:?}",
        oracle_lines(&report, "column")
    );
    assert!(
        oracle_lines(&report, "column").contains(&"messages|timestamp|INTEGER|1|0".to_owned()),
        "timestamp — NOT NULL INTEGER: {:?}",
        oracle_lines(&report, "column")
    );

    let messages = oracle_lines(&report, "message");
    assert_eq!(
        messages.len(),
        3,
        "все три сообщения на месте: {messages:?}"
    );
    let fields: Vec<Vec<&str>> = messages
        .iter()
        .map(|line| line.split('|').collect())
        .collect();
    assert_eq!(
        fields.iter().map(|parts| parts[0]).collect::<Vec<_>>(),
        ["m-3", "m-2", "m-1"],
        "оракул видит порядок timestamp DESC, id DESC"
    );
    assert_eq!(fields[0][3], "30");
    assert_eq!(fields[0][4], "-", "edited_timestamp = NULL");
    assert_eq!(fields[1][4], "21");
    assert_eq!(fields[1][5], "64", "flags = u64 64");
    assert_eq!(fields[1][6], "0", "deleted = 0");
    assert_eq!(
        String::from_utf8(hex_decode(fields[0][7])).expect("UTF-8"),
        third.content
    );
    assert_eq!(
        String::from_utf8(hex_decode(fields[1][7])).expect("UTF-8"),
        second.content
    );

    assert_eq!(
        oracle_lines(&report, "kv"),
        ["greeting|привет мир"],
        "kv читается оракулом"
    );
    assert_eq!(
        oracle_lines(&report, "channel"),
        [format!(
            "ch-1|g-1|{}|0|2",
            hex_encode(channel.name.as_deref().unwrap_or(""))
        )],
        "канал (включая Unicode-имя) виден оракулу"
    );
}

// --- s13 ------------------------------------------------------------------------

#[test]
fn s13_nul_rejected_and_large_string_kept() {
    let mut store = store_with_channel();

    let mut with_nul = message_row("m-nul", CHANNEL, 1);
    with_nul.content = "до\0после".to_owned();
    let error = store
        .put_message(&with_nul)
        .expect_err("NUL в C-строке недопустим");
    assert!(
        matches!(error, StoreError::Sqlite { .. }),
        "ожидали StoreError::Sqlite, получили {error:?}"
    );

    let mut nul_id = message_row("m\0id", CHANNEL, 2);
    nul_id.content = "ok".to_owned();
    let error = store.put_message(&nul_id).expect_err("NUL в id");
    assert!(
        matches!(error, StoreError::Sqlite { .. }),
        "ожидали StoreError::Sqlite для id, получили {error:?}"
    );

    // Хранилище не сломано ошибкой: обычная строка сохраняется.
    let huge = "ж✓'".repeat(300_000);
    let mut large = message_row("m-large", CHANNEL, 3);
    large.content = huge.clone();
    store.put_message(&large).expect("огромная строка");
    let rows = store.messages(CHANNEL, None, 10).expect("messages");
    assert_eq!(rows.len(), 1, "NUL-строки не попали в таблицу");
    assert_eq!(rows[0].content.len(), huge.len());
    assert_eq!(rows[0].content, huge);
}

// --- s14 ------------------------------------------------------------------------

#[test]
fn s14_unknown_cursor_yields_empty_page() {
    let mut store = store_with_channel();
    assert!(
        store
            .messages(CHANNEL, None, 10)
            .expect("messages")
            .is_empty()
    );
    store
        .put_message(&message_row("m-1", CHANNEL, 1))
        .expect("put_message");
    assert!(
        store
            .messages(CHANNEL, Some("no-such-id"), 10)
            .expect("messages")
            .is_empty(),
        "неизвестный курсор → пустая страница (а не вся история заново)"
    );
    assert!(
        store
            .messages("no-such-channel", Some("m-1"), 10)
            .expect("messages")
            .is_empty(),
        "чужой канал → пусто"
    );
}

// --- помощники оракула ----------------------------------------------------------

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("корень репозитория")
        .to_path_buf()
}

fn oracle_script() -> PathBuf {
    repo_root().join("waves/RW-31/sqlite/oracle.py")
}

fn python_sqlite_version() -> Option<String> {
    let output = Command::new("python3")
        .arg("-c")
        .arg("import sqlite3; print(sqlite3.sqlite_version)")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let version = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if version.is_empty() {
        None
    } else {
        Some(version)
    }
}

fn parse_oracle(text: &str) -> BTreeMap<String, Vec<String>> {
    let mut report: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            panic!("строка оракула без '=': {line}");
        };
        report
            .entry(key.to_owned())
            .or_default()
            .push(value.to_owned());
    }
    report
}

fn oracle_lines<'a>(report: &'a BTreeMap<String, Vec<String>>, key: &str) -> &'a [String] {
    report.get(key).map(Vec::as_slice).unwrap_or(&[])
}

fn oracle_value<'a>(report: &'a BTreeMap<String, Vec<String>>, key: &str) -> &'a str {
    let values = oracle_lines(report, key);
    assert_eq!(
        values.len(),
        1,
        "ожидали ровно одно значение {key}: {values:?}"
    );
    &values[0]
}

fn hex_decode(text: &str) -> Vec<u8> {
    assert!(
        text.len().is_multiple_of(2) && text.is_ascii(),
        "не hex: {text}"
    );
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).expect("hex-байт"))
        .collect()
}

fn hex_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for byte in text.as_bytes() {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
