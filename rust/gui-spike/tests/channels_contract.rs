//! C-02-BE: READY-проекция каналов для переключения канала (G6-D-01).
//!
//! Оракул — сам `fixtures/gateway/by-type/002-READY.json`: список гильдейских
//! каналов (272 записи, типы 0/2/4/5/15) и 17 ЛС. Тесты проверяют поля, которых
//! в READY нет (имя/позиция у ЛС), как честный `null`, а не выдуманное значение.
//!
//! Skip-aware: без `fixtures/` тесты печатают причину и возвращаются; при
//! `DL_STRICT=1` — падают (CI-режим).

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use chord_client::data::{self, DataError};
use serde_json::Value;

/// Корень фикстур: `DL_FIXTURES` или `fixtures/` на два уровня выше crate.
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
                    "SKIP channels_contract: нет фикстур в {} (DL_STRICT≠1)",
                    candidate_root().display()
                );
                return;
            }
        }
    };
}

fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("не читается {}: {err}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|err| panic!("не парсится {}: {err}", path.display()))
}

fn temp_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("dl-c02be-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("gateway/by-type")).expect("временный каталог создаётся");
    root
}

fn cleanup(root: &Path) {
    let _ = fs::remove_dir_all(root);
}

fn write_ready(root: &Path, body: &Value) {
    fs::write(
        root.join("gateway/by-type/002-READY.json"),
        body.to_string(),
    )
    .expect("пишем READY");
}

fn kind(err: &DataError) -> &'static str {
    err.kind()
}

const READY_PATH: &str = "gateway/by-type/002-READY.json";

// ---------------------------------------------------------------------------
// Проекция READY
// ---------------------------------------------------------------------------

#[test]
fn channels_projection_matches_ready_oracle() {
    let root = fixtures_or_skip!();
    let ready = read_json(&root.join(READY_PATH));
    let rows = data::channels(&root, None).expect("каналы READY");

    let guilds = ready["d"]["guilds"].as_array().expect("guilds");
    let oracle_guild_channels: usize = guilds
        .iter()
        .map(|guild| guild["channels"].as_array().map_or(0, Vec::len))
        .sum();
    let oracle_private = ready["d"]["private_channels"]
        .as_array()
        .expect("private_channels")
        .len();
    // Независимые числа из отчёта G6 §3.4 / READY: 272 канала в гильдиях, 17 ЛС.
    assert_eq!(oracle_guild_channels, 272, "оракул READY: 272 канала");
    assert_eq!(oracle_private, 17, "оракул READY: 17 ЛС");
    assert_eq!(rows.len(), 272 + 17, "гильдейские каналы + ЛС");

    // Числа по типам (0/2/4/5/15) — из отчёта G6 §3.4, только гильдейские.
    let mut counts: BTreeMap<u64, usize> = BTreeMap::new();
    for row in rows.iter().filter(|row| row.guild_id.is_some()) {
        *counts.entry(row.kind).or_insert(0) += 1;
    }
    assert_eq!(
        counts,
        BTreeMap::from([(0_u64, 137_usize), (2, 82), (4, 43), (5, 1), (15, 9)]),
        "типы сохраняются числом, без переименования"
    );

    // Каждое поле гильдейского канала сверяется с READY, не с реализацией.
    let by_id: HashMap<&str, &data::ChannelRow> =
        rows.iter().map(|row| (row.id.as_str(), row)).collect();
    for guild in guilds {
        let guild_id = guild["id"].as_str().expect("guild.id");
        for channel in guild["channels"].as_array().expect("guild.channels") {
            let id = channel["id"].as_str().expect("channel.id");
            let row = by_id
                .get(id)
                .unwrap_or_else(|| panic!("канал {id} пропал из проекции"));
            assert_eq!(row.guild_id.as_deref(), Some(guild_id), "{id}");
            assert_eq!(
                row.name.as_deref(),
                channel["name"].as_str(),
                "{id}: имя из READY"
            );
            assert_eq!(row.kind, channel["type"].as_u64().expect("type"), "{id}");
            assert_eq!(
                row.parent_id.as_deref(),
                channel["parent_id"]
                    .as_str()
                    .filter(|value| *value != "nil"),
                "{id}: parent_id"
            );
            assert_eq!(row.position, channel["position"].as_i64(), "{id}");
        }
    }

    // ЛС: гильдии/имени/позиции в READY нет — честный null, не выдумка.
    for channel in ready["d"]["private_channels"]
        .as_array()
        .expect("private_channels")
    {
        let id = channel["id"].as_str().expect("private.id");
        let row = by_id
            .get(id)
            .unwrap_or_else(|| panic!("ЛС {id} пропал из проекции"));
        assert_eq!(row.guild_id, None, "{id}: у ЛС нет гильдии");
        assert_eq!(row.name, None, "{id}: имени ЛС в READY нет");
        assert_eq!(row.position, None, "{id}: позиции у ЛС нет");
        assert_eq!(row.kind, channel["type"].as_u64().expect("type"), "{id}");
        assert_eq!(row.kind, 1, "{id}: тип ЛС из READY");
        assert_eq!(row.parent_id, None, "{id}");
    }

    // READY-проекция, а не «все каналы, где были сообщения»: канал из
    // MESSAGE_CREATE, которого нет в READY, в список не попадает.
    assert!(
        !by_id.contains_key("-2684694197495146496"),
        "канал вне READY не должен появляться в дереве"
    );

    // IPC-контракт: camelCase, `type` — числом, без переименования в `kind`.
    let json = serde_json::to_value(&rows[0]).expect("канал сериализуется");
    for key in ["id", "guildId", "name", "type", "parentId", "position"] {
        assert!(json.get(key).is_some(), "нет поля {key}: {json}");
    }
    assert!(json["type"].is_u64(), "тип числом: {json}");
    assert!(json.get("kind").is_none(), "поле называется type: {json}");
}

#[test]
fn channels_are_sorted_by_guild_position_and_id() {
    let root = fixtures_or_skip!();
    let ready = read_json(&root.join(READY_PATH));
    let guilds = ready["d"]["guilds"].as_array().expect("guilds");
    let rank: HashMap<&str, usize> = guilds
        .iter()
        .enumerate()
        .map(|(index, guild)| (guild["id"].as_str().expect("guild.id"), index))
        .collect();
    let rows = data::channels(&root, None).expect("каналы READY");

    // Порядок гильдий — как в READY (рейл), ЛС — после всех гильдий;
    // внутри гильдии — position asc, при равенстве — id asc.
    let key = |row: &data::ChannelRow| {
        (
            row.guild_id
                .as_deref()
                .and_then(|id| rank.get(id).copied())
                .unwrap_or(usize::MAX),
            row.position.unwrap_or(i64::MAX),
            row.id.clone(),
        )
    };
    for pair in rows.windows(2) {
        let left = key(&pair[0]);
        let right = key(&pair[1]);
        assert!(
            left <= right,
            "порядок нарушен: {:?} / {:?}",
            pair[0],
            pair[1]
        );
    }
    assert!(
        rows.last().is_some_and(|row| row.guild_id.is_none()),
        "ЛС идут последними"
    );

    // Повтор детерминирован: без скрытого состояния и случайного порядка.
    let repeated = data::channels(&root, None).expect("повтор чтения");
    assert_eq!(repeated, rows);
}

#[test]
fn channels_filter_by_guild_returns_only_that_guild() {
    let root = fixtures_or_skip!();
    let ready = read_json(&root.join(READY_PATH));
    let guild = &ready["d"]["guilds"].as_array().expect("guilds")[0];
    let guild_id = guild["id"].as_str().expect("guild.id");
    let expected = guild["channels"].as_array().expect("channels").len();
    assert_eq!(expected, 55, "оракул: у первой гильдии 55 каналов");

    let rows = data::channels(&root, Some(guild_id)).expect("фильтр по гильдии");
    assert_eq!(rows.len(), expected);
    assert!(
        rows.iter()
            .all(|row| row.guild_id.as_deref() == Some(guild_id)),
        "чужая гильдия в фильтре"
    );

    // Обрезка пробелов: команда не должна требовать «чистый» id.
    let padded = format!("  {guild_id}  ");
    assert_eq!(
        data::channels(&root, Some(&padded)).expect("id с пробелами"),
        rows
    );

    let err = data::channels(&root, Some("нет-такой-гильдии")).expect_err("неизвестная гильдия");
    assert_eq!(kind(&err), "NotFound", "{err}");

    let err = data::channels(&root, Some("   ")).expect_err("пустой guildId");
    assert_eq!(kind(&err), "InvalidArg", "{err}");

    let long = "7".repeat(10_000);
    let err = data::channels(&root, Some(&long)).expect_err("длинный guildId");
    assert_eq!(kind(&err), "InvalidArg", "{err}");
    assert!(
        !err.to_string().contains(&long),
        "значение не копируется целиком: {err}"
    );
    assert!(err.to_string().contains('…'), "видно усечение: {err}");

    // Граница включительно: 256 символов — валидная длина, дальше NotFound.
    let allowed = "9".repeat(256);
    let err = data::channels(&root, Some(&allowed)).expect_err("такой гильдии нет");
    assert_eq!(kind(&err), "NotFound", "256 символов допустимы: {err}");
}

// ---------------------------------------------------------------------------
// Негативные кейсы: нет READY/битый READY/битая запись канала
// ---------------------------------------------------------------------------

#[test]
fn channels_without_ready_or_broken_ready_are_typed_errors() {
    let root = temp_root("channels-missing");
    let err = data::channels(&root, None).expect_err("READY отсутствует");
    assert_eq!(kind(&err), "NotFound", "{err}");
    cleanup(&root);

    let root = temp_root("channels-broken");
    fs::write(root.join(READY_PATH), "{ это не json").expect("пишем битый READY");
    let err = data::channels(&root, None).expect_err("битый READY");
    assert_eq!(kind(&err), "Parse", "{err}");
    assert!(err.to_string().contains("002-READY.json"), "{err}");
    cleanup(&root);

    let root = temp_root("channels-no-data");
    write_ready(&root, &serde_json::json!({"t": "READY"}));
    let err = data::channels(&root, None).expect_err("READY без d");
    assert_eq!(kind(&err), "Parse", "{err}");
    cleanup(&root);

    let root = temp_root("channels-guilds-not-array");
    write_ready(
        &root,
        &serde_json::json!({"t": "READY", "d": {"guilds": {}}}),
    );
    let err = data::channels(&root, None).expect_err("guilds не массив");
    assert_eq!(kind(&err), "Parse", "{err}");
    cleanup(&root);

    let root = temp_root("channels-private-not-array");
    write_ready(
        &root,
        &serde_json::json!({"t": "READY", "d": {"guilds": [], "private_channels": 7}}),
    );
    let err = data::channels(&root, None).expect_err("private_channels не массив");
    assert_eq!(kind(&err), "Parse", "{err}");
    cleanup(&root);
}

#[test]
fn channels_reject_records_without_id_or_type() {
    for (name, channel) in [
        ("no-id", serde_json::json!({"name": "x", "type": 0})),
        (
            "no-type",
            serde_json::json!({"id": "42", "name": "x", "position": 1}),
        ),
        (
            "type-not-number",
            serde_json::json!({"id": "42", "name": "x", "type": "text"}),
        ),
    ] {
        let root = temp_root(&format!("channels-bad-{name}"));
        write_ready(
            &root,
            &serde_json::json!({
                "t": "READY",
                "d": {
                    "guilds": [{"id": "1", "channels": [channel]}],
                    "private_channels": [],
                },
            }),
        );
        let err = match data::channels(&root, None) {
            Ok(rows) => panic!("{name}: битая запись принята, строк: {}", rows.len()),
            Err(error) => error,
        };
        assert_eq!(kind(&err), "Parse", "{name}: {err}");
        assert!(err.to_string().contains("002-READY.json"), "{name}: {err}");
        cleanup(&root);
    }
}

#[test]
fn channels_keep_absent_fields_as_null_and_order_private_last() {
    let root = temp_root("channels-sanitized");
    write_ready(
        &root,
        &serde_json::json!({
            "t": "READY",
            "d": {
                "guilds": [{
                    "id": "g1",
                    "channels": [
                        {"id": "c2", "name": "beta", "type": 0, "parent_id": "cat", "position": 7},
                        {"id": "c1", "name": "nil", "type": 5, "parent_id": "nil", "position": 2},
                    ],
                }],
                "private_channels": [{"id": "d1", "type": 1}],
            },
        }),
    );

    let rows = data::channels(&root, None).expect("минимальный READY");
    assert_eq!(rows.len(), 3);
    // Внутри гильдии — по position; «nil»-строки — отсутствие значения.
    assert_eq!(rows[0].id, "c1");
    assert_eq!(
        rows[0].name, None,
        "name=nil — нет значения, а не строка nil"
    );
    assert_eq!(rows[0].parent_id, None, "parent_id=nil — нет категории");
    assert_eq!(rows[0].position, Some(2));
    assert_eq!(rows[0].kind, 5);
    assert_eq!(rows[0].guild_id.as_deref(), Some("g1"));

    assert_eq!(rows[1].id, "c2");
    assert_eq!(rows[1].name.as_deref(), Some("beta"));
    assert_eq!(rows[1].parent_id.as_deref(), Some("cat"));

    // ЛС: гильдии, имени, позиции и категории в READY нет.
    assert_eq!(rows[2].id, "d1");
    assert_eq!(rows[2].guild_id, None);
    assert_eq!(rows[2].name, None);
    assert_eq!(rows[2].parent_id, None);
    assert_eq!(rows[2].position, None);
    assert_eq!(rows[2].kind, 1);

    cleanup(&root);
}

#[test]
fn channels_mark_afk_channel_when_matching_guild_afk_channel_id() {
    let root = temp_root("channels-afk");
    write_ready(
        &root,
        &serde_json::json!({
            "t": "READY",
            "d": {
                "guilds": [{
                    "id": "g1",
                    "afk_channel_id": "c_afk",
                    "channels": [
                        {"id": "c_voice", "name": "General Voice", "type": 2, "position": 1},
                        {"id": "c_afk", "name": "AFK Room", "type": 2, "position": 2},
                    ],
                }],
                "private_channels": [],
            },
        }),
    );

    let rows = data::channels(&root, Some("g1")).expect("каналы с AFK");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, "c_voice");
    assert_eq!(
        rows[0].is_afk, None,
        "обычный голосовой канал не является AFK"
    );

    assert_eq!(rows[1].id, "c_afk");
    assert_eq!(
        rows[1].is_afk,
        Some(true),
        "AFK-канал помечается is_afk=Some(true)"
    );

    // Сериализация в camelCase должна давать isAfk: true
    let json_afk = serde_json::to_value(&rows[1]).expect("serde json");
    assert_eq!(json_afk["isAfk"], serde_json::json!(true));

    let json_normal = serde_json::to_value(&rows[0]).expect("serde json");
    assert!(
        json_normal.get("isAfk").is_none(),
        "для обычных каналов isAfk опускается"
    );

    cleanup(&root);
}
