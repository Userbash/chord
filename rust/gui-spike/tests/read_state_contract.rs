//! RW-40/M-07-BE3: READY-проекция `read_state` для UI (курсор + badge-форма).
//!
//! Контракт (G6-REPORT §3.5 / G6-REPARITY-03 M-07, `docs/re/gateway-schema.md`
//! `READY.d.read_state.entries[]`): обычная запись несёт
//! `{id, flags, last_message_id, last_pin_timestamp, last_viewed, mention_count}`,
//! badge-форма (`read_state_type`, 2/157 в READY) — ещё
//! `{badge_count, last_acked_id, read_state_type}`. `last_viewed` — счётчик
//! read-state (не snowflake), пример `4310` из `MESSAGE_ACK`/`rest-d02 §4`.
//! IPC-ответ — массив ровно из девяти полей, имена — точный camelCase READY:
//! `{channelId, flags, lastMessageId, lastPinTimestamp, lastViewed, mentionCount,
//! badgeCount, lastAckedId, readStateType}`: числа — JSON-числа,
//! отсутствующее/`nil` — `null`, лишних (не из READY) полей нет.
//!
//! Оракул здесь — не реализация, а руками собранная фикстура из трёх записей
//! (одна с `last_viewed = 4310`, одна без `last_viewed`, одна с `nil`), реальная
//! badge-форма из READY и сам файл READY (в `data_contract.rs`). Ошибки —
//! `DataError` `NotFound`/`Parse`; при ошибке возвращается `Err`, а не пустой
//! список.
//!
//! Skip-aware здесь не нужен: все кейсы, кроме source-проверки wiring,
//! используют временный каталог фикстур под `temp_dir`.

use std::fs;
use std::path::{Path, PathBuf};

use chord_client::data::{self, DataError, ReadStateEntry};
use serde_json::{json, Value};

/// Временный корень фикстур: уникален для теста (pid + имя), чистится заранее.
fn temp_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("dl-m07be2-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("gateway/by-type")).expect("временный каталог создаётся");
    root
}

/// Best-effort уборка: ошибки удаления не влияют на проверку.
fn cleanup(root: &Path) {
    let _ = fs::remove_dir_all(root);
}

/// Пишет READY с телом `d` (остальное — минимальный каркас, чтобы ошибка
/// возникала именно в `read_state`, а не в `user`/`guilds`).
fn write_ready(root: &Path, data: Value) -> PathBuf {
    let ready = json!({"t": "READY", "d": data});
    let path = root.join("gateway/by-type/002-READY.json");
    fs::write(&path, ready.to_string()).expect("пишем READY");
    path
}

fn kind(err: &DataError) -> &'static str {
    err.kind()
}

/// Три записи: полная с `last_viewed = 4310`, без `last_viewed`/`last_message_id`
/// и с санитизированными `nil`-заглушками. Значения взяты из реального READY
/// (`002-READY.json`) и `046-MESSAGE_ACK.json`, чтобы оракул был внешним.
fn three_entries() -> Value {
    json!([
        {
            "id": "274093052673392641",
            "flags": 0,
            "last_message_id": "1547947251035406357",
            "last_pin_timestamp": "1970-01-01T00:00:00+00:00",
            "last_viewed": 4310,
            "mention_count": 7
        },
        {
            "id": "275270042508066816",
            "flags": 1,
            "last_message_id": "1486711666774114445",
            "last_pin_timestamp": "1970-01-01T00:00:00+00:00",
            "mention_count": 0
        },
        {
            "id": "276391685506990080",
            "flags": 3,
            "last_message_id": "nil",
            "last_pin_timestamp": "nil",
            "last_viewed": "nil",
            "mention_count": 2
        }
    ])
}

#[test]
fn projects_three_entries_with_last_viewed_4310_and_nulls() {
    let root = temp_root("three");
    write_ready(
        &root,
        json!({
            "user": {"id": "1", "username": "tester"},
            "guilds": [],
            "private_channels": [],
            "read_state": {"entries": three_entries()},
        }),
    );

    let entries = data::read_state(&root).expect("READY с read_state читается");
    assert_eq!(entries.len(), 3, "в READY ровно три записи read_state");

    assert_eq!(
        entries[0],
        ReadStateEntry {
            channel_id: "274093052673392641".to_string(),
            flags: Some(0),
            last_message_id: Some("1547947251035406357".to_string()),
            last_pin_timestamp: Some("1970-01-01T00:00:00+00:00".to_string()),
            last_viewed: Some(4310),
            mention_count: Some(7),
            badge_count: None,
            last_acked_id: None,
            read_state_type: None,
        }
    );
    assert_eq!(
        entries[0].last_viewed,
        Some(4310),
        "last_viewed — счётчик read-state из оракула 046-MESSAGE_ACK"
    );
    assert_eq!(entries[1].last_viewed, None, "нет last_viewed → null");
    assert_eq!(entries[2].last_viewed, None, "санитизированный nil → null");
    assert_eq!(entries[2].last_message_id, None, "nil id → null");
    assert_eq!(entries[2].last_pin_timestamp, None, "nil timestamp → null");
    assert_eq!(entries[2].flags, Some(3), "flags передаются как есть");
    assert_eq!(
        entries[2].mention_count,
        Some(2),
        "mention_count передаётся как есть"
    );
    assert_eq!(
        entries[0].badge_count, None,
        "у обычной записи badge_count отсутствует в READY → null, а не 0"
    );
    assert_eq!(
        entries[0].read_state_type, None,
        "у обычной записи нет read_state_type → null"
    );
    assert_eq!(
        entries[0].last_acked_id, None,
        "last_acked_id — только non-channel badge, иначе null"
    );

    cleanup(&root);
}

#[test]
fn serializes_exactly_nine_camel_case_fields_typed_like_ready() {
    let root = temp_root("json");
    write_ready(
        &root,
        json!({
            "user": {"id": "1", "username": "tester"},
            "read_state": {"entries": three_entries()},
        }),
    );
    let entries = data::read_state(&root).expect("READY читается");
    let value = serde_json::to_value(&entries).expect("ответ сериализуется");
    let array = value.as_array().expect("ответ — массив");
    assert_eq!(array.len(), 3);

    let expected_keys = [
        "channelId",
        "flags",
        "lastMessageId",
        "lastPinTimestamp",
        "lastViewed",
        "mentionCount",
        "badgeCount",
        "lastAckedId",
        "readStateType",
    ];
    for (index, entry) in array.iter().enumerate() {
        let object = entry.as_object().expect("запись — объект");
        let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
        keys.sort_unstable();
        let mut expected = expected_keys.to_vec();
        expected.sort_unstable();
        assert_eq!(
            keys, expected,
            "запись {index}: ровно девять полей контракта (шесть общих + три badge READY)"
        );
    }

    let first = &array[0];
    assert!(
        first["lastViewed"].is_u64(),
        "lastViewed — число, не строка"
    );
    assert_eq!(first["lastViewed"].as_u64(), Some(4310));
    assert!(first["flags"].is_u64());
    assert!(first["mentionCount"].is_u64());
    assert_eq!(first["mentionCount"].as_u64(), Some(7));
    assert_eq!(first["channelId"].as_str(), Some("274093052673392641"));
    for key in ["badgeCount", "lastAckedId", "readStateType"] {
        assert_eq!(
            first[key],
            Value::Null,
            "{key}: у обычной записи READY поля нет → null"
        );
    }

    let second = &array[1];
    assert_eq!(second["lastViewed"], Value::Null, "nil → null, не 0");
    let third = &array[2];
    assert_eq!(third["lastMessageId"], Value::Null);
    assert_eq!(third["lastPinTimestamp"], Value::Null);
}

#[test]
fn empty_entries_is_a_valid_empty_projection() {
    let root = temp_root("empty");
    write_ready(
        &root,
        json!({
            "user": {"id": "1", "username": "tester"},
            "read_state": {"entries": []},
        }),
    );
    let entries = data::read_state(&root).expect("пустой read_state — валидный ответ");
    assert!(
        entries.is_empty(),
        "нет записей — пустой список, а не ошибка"
    );
    cleanup(&root);
}

#[test]
fn missing_ready_file_is_not_found() {
    let root = temp_root("missing");
    let error = data::read_state(&root).expect_err("без READY — ошибка, не пустой список");
    assert_eq!(kind(&error), "NotFound", "ошибка: {error}");
    cleanup(&root);
}

#[test]
fn broken_ready_json_is_parse() {
    let root = temp_root("broken");
    let path = root.join("gateway/by-type/002-READY.json");
    fs::write(&path, "{not json").expect("пишем битый READY");
    let error = data::read_state(&root).expect_err("битый JSON — ошибка");
    assert_eq!(kind(&error), "Parse", "ошибка: {error}");
    cleanup(&root);
}

#[test]
fn ready_without_read_state_is_parse() {
    let root = temp_root("no-read-state");
    write_ready(&root, json!({"user": {"id": "1", "username": "tester"}}));
    let error = data::read_state(&root).expect_err("нет read_state — ошибка");
    assert_eq!(kind(&error), "Parse", "ошибка: {error}");
    cleanup(&root);
}

#[test]
fn entries_of_wrong_type_is_parse() {
    let root = temp_root("wrong-entries");
    write_ready(
        &root,
        json!({"user": {"id": "1", "username": "tester"}, "read_state": {"entries": {}}}),
    );
    let error = data::read_state(&root).expect_err("entries не массив — ошибка");
    assert_eq!(kind(&error), "Parse", "ошибка: {error}");
    cleanup(&root);
}

#[test]
fn entry_without_channel_id_is_parse() {
    let root = temp_root("no-id");
    write_ready(
        &root,
        json!({
            "user": {"id": "1", "username": "tester"},
            "read_state": {"entries": [
                {"id": "1", "flags": 0},
                {"flags": 1, "last_viewed": 4310},
            ]},
        }),
    );
    let error = data::read_state(&root).expect_err("запись без id — ошибка, не строка-пустышка");
    assert_eq!(kind(&error), "Parse", "ошибка: {error}");
    assert!(
        error.to_string().contains("entries[1]"),
        "в ошибке должен быть индекс записи: {error}"
    );
    cleanup(&root);
}

#[test]
fn entry_without_flags_is_null_not_invented_zero() {
    let root = temp_root("no-flags");
    write_ready(
        &root,
        json!({
            "user": {"id": "1", "username": "tester"},
            "read_state": {"entries": [{"id": "1", "last_viewed": 4310}]},
        }),
    );
    let entries = data::read_state(&root).expect("запись без flags — всё ещё запись READY");
    assert_eq!(
        entries[0].flags, None,
        "нет flags → null, а не выдуманный 0"
    );
    assert_eq!(entries[0].mention_count, None, "нет mention_count → null");
    assert_eq!(entries[0].last_viewed, Some(4310));
    cleanup(&root);
}

/// Реальная badge-форма записи READY (`read_state_type`/`badge_count`/
/// `last_acked_id` — 2/157 в `docs/re/gateway-schema.md`):
/// `{badge_count, id, last_acked_id, read_state_type}` — гильдейский badge, без
/// `flags`/`last_viewed`/`mention_count`. Поля badge проецируются как есть
/// (имена — точный camelCase READY), отсутствующие общие поля — `null`, а не 0.
/// `lastAckedId` остаётся десятичной строкой: bignum READY не помещается в
/// JS-safe integer, как и `lastMessageId`.
#[test]
fn badge_form_projects_badge_fields_and_missing_common_fields_as_null() {
    let root = temp_root("badge");
    write_ready(
        &root,
        json!({
            "user": {"id": "1", "username": "tester"},
            "read_state": {"entries": [{
                "badge_count": 16,
                "id": "384299970632876032",
                "last_acked_id": "1294268550826950656",
                "read_state_type": 2
            }]},
        }),
    );
    let entries = data::read_state(&root).expect("badge-запись не ломает проекцию");
    assert_eq!(
        entries,
        vec![ReadStateEntry {
            channel_id: "384299970632876032".to_string(),
            flags: None,
            last_message_id: None,
            last_pin_timestamp: None,
            last_viewed: None,
            mention_count: None,
            badge_count: Some(16),
            last_acked_id: Some("1294268550826950656".to_string()),
            read_state_type: Some(2),
        }]
    );

    let value = serde_json::to_value(&entries).expect("ответ сериализуется");
    let first = &value[0];
    assert!(first["badgeCount"].is_i64(), "badgeCount — число READY");
    assert_eq!(first["badgeCount"].as_i64(), Some(16));
    assert!(
        first["readStateType"].is_i64(),
        "readStateType — число READY"
    );
    assert_eq!(first["readStateType"].as_i64(), Some(2));
    assert_eq!(
        first["lastAckedId"].as_str(),
        Some("1294268550826950656"),
        "lastAckedId — десятичная строка, snowflake не теряет точность"
    );
    assert_eq!(first["flags"], Value::Null, "нет flags → null, а не 0");
    assert_eq!(
        first["mentionCount"],
        Value::Null,
        "нет mention_count → null, а не 0"
    );
    cleanup(&root);
}

/// Второй badge-вид READY: `badge_count = 0` — валидное значение поля, а не
/// «нет бейджа» и не отсутствие поля (`CODE-STANDARD` RW-17: наличие поля не
/// равно предикату). Проекция обязана отдать `Some(0)`, предикат `> 0` — дело UI.
#[test]
fn badge_count_zero_is_a_value_not_null() {
    let root = temp_root("badge-zero");
    write_ready(
        &root,
        json!({
            "user": {"id": "1", "username": "tester"},
            "read_state": {"entries": [{
                "badge_count": 0,
                "id": "1014537851418980372",
                "last_acked_id": "1237379809395605509",
                "read_state_type": 4
            }]},
        }),
    );
    let entries = data::read_state(&root).expect("badge_count = 0 читается");
    assert_eq!(
        entries[0].badge_count,
        Some(0),
        "badge_count = 0 — значение поля, не null"
    );
    assert_eq!(entries[0].read_state_type, Some(4));
    assert_eq!(
        entries[0].last_acked_id.as_deref(),
        Some("1237379809395605509")
    );
    cleanup(&root);
}

/// Ошибочные типы badge-полей — `Parse` с индексом записи, не тихий `null`/0:
/// иначе испорченный READY дал бы ложный «нет бейджей».
#[test]
fn malformed_badge_fields_are_parse_with_entry_index() {
    for (name, entry, field) in [
        (
            "badge-count",
            json!({"id": "1", "badge_count": "много", "read_state_type": 2}),
            "badge_count",
        ),
        (
            "read-state-type",
            json!({"id": "2", "badge_count": 16, "read_state_type": true}),
            "read_state_type",
        ),
        (
            "last-acked-id",
            json!({"id": "3", "last_acked_id": 12, "read_state_type": 2}),
            "last_acked_id",
        ),
    ] {
        let root = temp_root(name);
        write_ready(
            &root,
            json!({
                "user": {"id": "1", "username": "tester"},
                "read_state": {"entries": [
                    {"id": "0", "flags": 0},
                    entry,
                ]},
            }),
        );
        let error = data::read_state(&root).expect_err("битое badge-поле — Parse, не null");
        assert_eq!(kind(&error), "Parse", "{field}: {error}");
        assert!(
            error.to_string().contains("entries[1]"),
            "{field}: в ошибке индекс записи: {error}"
        );
        cleanup(&root);
    }
}

/// Явный `null` в badge-полях — отсутствие значения (`None`), а не 0.
#[test]
fn explicit_null_badge_fields_are_none_not_zero() {
    let root = temp_root("badge-null");
    write_ready(
        &root,
        json!({
            "user": {"id": "1", "username": "tester"},
            "read_state": {"entries": [{
                "id": "42",
                "badge_count": null,
                "last_acked_id": null,
                "read_state_type": null,
            }]},
        }),
    );
    let entries = data::read_state(&root).expect("null badge-поля — валидная запись");
    assert_eq!(entries[0].badge_count, None);
    assert_eq!(entries[0].last_acked_id, None);
    assert_eq!(entries[0].read_state_type, None);
    cleanup(&root);
}

#[test]
fn malformed_last_viewed_is_parse_not_silent_null() {
    let root = temp_root("bad-lv");
    write_ready(
        &root,
        json!({
            "user": {"id": "1", "username": "tester"},
            "read_state": {"entries": [
                {"id": "1", "flags": 0, "last_viewed": "не число"},
            ]},
        }),
    );
    let error = data::read_state(&root).expect_err("нечитаемый last_viewed — Parse, не null");
    assert_eq!(kind(&error), "Parse", "ошибка: {error}");
    cleanup(&root);
}

/// Source-проверка wiring: тело команды в `main.rs` (окно не нужно).
fn command_body(name: &str) -> &'static str {
    let source: &'static str = include_str!("../src/main.rs");
    let marker = format!("fn {name}(");
    let start = source
        .find(&marker)
        .unwrap_or_else(|| panic!("main.rs должен объявлять команду {name}"));
    let rest = &source[start..];
    let end = rest.find("\n}\n").map_or(rest.len(), |index| index + 2);
    &rest[..end]
}

#[test]
fn tauri_read_state_command_is_local_registered_and_silent() {
    let source: &'static str = include_str!("../src/main.rs");
    let open = "generate_handler![";
    let start = source
        .find(open)
        .expect("main.rs должен объявлять invoke_handler!")
        + open.len();
    let block = &source[start..];
    let block = &block[..block.find(']').expect("invoke_handler! без закрывающей ]")];
    let lists = |command: &str| {
        block
            .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .any(|name| name == command)
    };
    assert!(
        lists("read_state"),
        "read_state не зарегистрирован в invoke_handler!"
    );

    let body = command_body("read_state");
    assert!(
        body.contains("data::read_state(&root)"),
        "команда обязана делегировать в data::read_state: {body}"
    );
    for forbidden in [
        "token_or",
        "backend.token()",
        "AuthBackend",
        "keyring",
        "println!",
        "eprintln!",
        "log::",
    ] {
        assert!(
            !body.contains(forbidden),
            "read_state не должен содержать {forbidden}: {body}"
        );
    }
}
