//! Роутер диспатчей шлюза: op0 → 21 типизированный вид; неизвестные → raw (волна RW-07,
//! P1-06 / T-GW-08).
//!
//! **Провенанс оракулов** (независимые артефакты захвата, не подгонка под реализацию):
//! - union семи снимков `gw-proto/msgs-*.jsonl` (run1 `.5112` + run2 `.1389/.5532/.8269/.8608/
//!   `.10029/.13072`): **1340** op0, **21** тип; потиповые числа — `LAB_TYPE_COUNTS` (§3.1 ТЗ);
//! - снимок потока `gw-proto/zstd-frames-95974_1389.hexl`: 137 кадров, 104 066 сжатых Б;
//!   распаковка без удержания окна — **824 816 Б, 137 полных термов** (113 op0, s = 1..113;
//!   24 не-диспатча: 23 op11 + 1 op10); потиповые числа сокета — `LAB_STREAM_COUNTS` (§3.2 ТЗ,
//!   независимый источник — таймлайн `gw-proto/msgs-run2-95974_1389.jsonl`). До task-137
//!   (F-GW-01) ruzstd удерживал последние 64 КиБ, и в тест попадал префикс из 44 термов.
//!
//! Lab-файлы опциональны: нет файла — skip с причиной, а не падение.

use discord_core::codec::ZstdStream;
use discord_core::codec::etf::{self, EtfError, EtfValue};
use discord_core::gateway::Session;
use discord_core::ws::GatewayPipe;
use discord_core::{
    DISPATCH_COUNT_R7, DISPATCH_TYPE_COUNT_R7, DispatchEvent, DispatchKind, route_dispatch,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Оракульные числа захвата (§3.1/§3.2 ТЗ; перепроверены координатором 2026-10-19)
// ---------------------------------------------------------------------------

/// Снимки run1/run2, дающие union всех наблюдаемых типов.
const LAB_FILES: [&str; 7] = [
    "msgs-run1-57992_5112.jsonl",
    "msgs-run2-95974_1389.jsonl",
    "msgs-run2-95974_5532.jsonl",
    "msgs-run2-95974_8269.jsonl",
    "msgs-run2-95974_8608.jsonl",
    "msgs-run2-95974_10029.jsonl",
    "msgs-run2-95974_13072.jsonl",
];

/// 21 имя провода op0 (порядок — как в таблице протокола).
const LAB_TYPE_NAMES: [&str; 21] = [
    "CHANNEL_INFO",
    "GUILD_MEMBERS_CHUNK",
    "GUILD_MEMBER_LIST_UPDATE",
    "MESSAGE_ACK",
    "MESSAGE_CREATE",
    "MESSAGE_POLL_VOTE_ADD",
    "MESSAGE_REACTION_ADD",
    "MESSAGE_UPDATE",
    "PRESENCE_UPDATE",
    "READY",
    "READY_SUPPLEMENTAL",
    "RESUMED",
    "SESSIONS_REPLACE",
    "SOUNDBOARD_SOUNDS",
    "THREAD_LIST_SYNC",
    "TYPING_START",
    "USER_SETTINGS_PROTO_UPDATE",
    "VOICE_CHANNEL_START_TIME_UPDATE",
    "VOICE_CHANNEL_STATUS_UPDATE",
    "VOICE_SERVER_UPDATE",
    "VOICE_STATE_UPDATE",
];

/// Потиповые числа union (сумма — 1340).
const LAB_TYPE_COUNTS: [(&str, u64); 21] = [
    ("GUILD_MEMBER_LIST_UPDATE", 1048),
    ("PRESENCE_UPDATE", 107),
    ("MESSAGE_CREATE", 31),
    ("TYPING_START", 28),
    ("MESSAGE_UPDATE", 24),
    ("SOUNDBOARD_SOUNDS", 21),
    ("GUILD_MEMBERS_CHUNK", 11),
    ("SESSIONS_REPLACE", 8),
    ("VOICE_STATE_UPDATE", 8),
    ("MESSAGE_ACK", 8),
    ("THREAD_LIST_SYNC", 7),
    ("CHANNEL_INFO", 7),
    ("VOICE_CHANNEL_START_TIME_UPDATE", 7),
    ("READY", 4),
    ("READY_SUPPLEMENTAL", 4),
    ("MESSAGE_POLL_VOTE_ADD", 4),
    ("VOICE_SERVER_UPDATE", 3),
    ("VOICE_CHANNEL_STATUS_UPDATE", 3),
    ("RESUMED", 3),
    ("USER_SETTINGS_PROTO_UPDATE", 3),
    ("MESSAGE_REACTION_ADD", 1),
];

/// Потиповые числа реального сокета `.1389` целиком (113 op0, s = 1..113; сумма — 113).
const LAB_STREAM_COUNTS: [(&str, u64); 11] = [
    ("GUILD_MEMBER_LIST_UPDATE", 51),
    ("PRESENCE_UPDATE", 40),
    ("MESSAGE_UPDATE", 6),
    ("GUILD_MEMBERS_CHUNK", 4),
    ("MESSAGE_CREATE", 3),
    ("SESSIONS_REPLACE", 2),
    ("THREAD_LIST_SYNC", 2),
    ("MESSAGE_ACK", 2),
    ("READY", 1),
    ("READY_SUPPLEMENTAL", 1),
    ("CHANNEL_INFO", 1),
];

// ---------------------------------------------------------------------------
// ETF-помощники (формы как в соседних тестах RW-05/RW-06)
// ---------------------------------------------------------------------------

/// Ключ/значение-атом map (формы сервера: `op`/`t`/`d`).
fn atom(name: &str) -> EtfValue {
    EtfValue::Atom(name.to_string())
}

/// Map из пар «ключ-атом → значение».
fn map(pairs: Vec<(&str, EtfValue)>) -> EtfValue {
    EtfValue::Map(
        pairs
            .into_iter()
            .map(|(key, value)| (atom(key), value))
            .collect(),
    )
}

/// Список (`LIST_EXT` с хвостовым `NIL_EXT` — форма массивов в захвате).
fn list(items: Vec<EtfValue>) -> EtfValue {
    EtfValue::List(items)
}

/// Форма провода: `0x83` + терм.
fn wire(value: &EtfValue) -> Vec<u8> {
    etf::encode(value)
}

/// Диспатч op0 с типом-атомом, `s` и payload `d`.
fn dispatch_value(t: &str, s: i64, d: EtfValue) -> EtfValue {
    map(vec![
        ("op", EtfValue::Int(0)),
        ("t", atom(t)),
        ("s", EtfValue::Int(s)),
        ("d", d),
    ])
}

/// HELLO с интервалом `interval`.
fn hello_bytes(interval: i64) -> Vec<u8> {
    wire(&map(vec![
        ("op", EtfValue::Int(10)),
        (
            "d",
            map(vec![("heartbeat_interval", EtfValue::Int(interval))]),
        ),
    ]))
}

/// Один zstd-фрагмент с данными термами — **завершённый** кадр (пайп отдаёт все термы целиком).
fn zstd_message(terms: &[Vec<u8>]) -> Vec<u8> {
    let mut encoder = zstd::stream::write::Encoder::new(Vec::new(), 1).expect("zstd-энкодер");
    encoder
        .set_parameter(zstd::zstd_safe::CParameter::WindowLog(10))
        .expect("окно zstd 1 КиБ");
    for term in terms {
        encoder.write_all(term).expect("терм в zstd");
    }
    encoder.finish().expect("завершение zstd-кадра")
}

/// Питание пайпа для P1/P2: HELLO + READY(s=1) + MESSAGE_CREATE(s=2) + неизвестный(s=3).
fn sample_feed_terms() -> Vec<Vec<u8>> {
    vec![
        hello_bytes(41_250),
        wire(&dispatch_value(
            "READY",
            1,
            map(vec![("guilds", list(Vec::new()))]),
        )),
        wire(&dispatch_value(
            "MESSAGE_CREATE",
            2,
            map(vec![("content", EtfValue::Str("привет".to_string()))]),
        )),
        wire(&dispatch_value(
            "FUTURE_EVENT_X",
            3,
            map(vec![("k", EtfValue::Int(1))]),
        )),
    ]
}

// ---------------------------------------------------------------------------
// Lab-помощники (skip-aware)
// ---------------------------------------------------------------------------

/// Lab-корень: `RE_LAB_ROOT` или `/home/sanya/re-deep` (как в тестах RW-02/RW-04/RW-05).
fn lab_root() -> PathBuf {
    PathBuf::from(
        std::env::var("RE_LAB_ROOT").unwrap_or_else(|_| "/home/sanya/re-deep".to_string()),
    )
}

/// Текст lab-файла; нет файла → `None` (skip-aware), прочие ошибки чтения не глотаются.
fn read_opt(path: &Path) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => panic!("lab-файл {}: {error}", path.display()),
    }
}

/// Числовое поле верхнего уровня из jsonl-строки захвата (`"key":123`).
///
/// Формат артефактов фиксирован (`{"idx":N,"op":N,"t":"…","s":N|"nil",…}`) — первый `"key":`
/// в строке всегда верхнего уровня, поэтому мини-парсер честен для этих файлов.
fn json_int_field(line: &str, key: &str) -> Option<i64> {
    let needle = format!("\"{key}\":");
    let start = line.find(&needle)? + needle.len();
    let rest = &line[start..];
    if rest.starts_with('"') {
        return None; // "nil" или строка
    }
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '-'))
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// Строковое поле верхнего уровня из jsonl-строки захвата (`"key":"value"`).
fn json_str_field(line: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\":\"");
    let start = line.find(&needle)? + needle.len();
    let end = line[start..].find('"')? + start;
    Some(line[start..end].to_string())
}

/// Строка hex → байты (пробелы игнорируются).
fn unhex(s: &str) -> Vec<u8> {
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    assert_eq!(compact.len() % 2, 0, "hex нечётной длины: {s}");
    (0..compact.len() / 2)
        .map(|i| u8::from_str_radix(&compact[2 * i..2 * i + 2], 16).expect("hex"))
        .collect()
}

/// Сверка потиповых счётчиков с оракульной таблицей (ни одного типа мимо таблицы).
fn assert_type_counts(actual: &BTreeMap<String, u64>, oracle: &[(&str, u64)], label: &str) {
    assert_eq!(
        oracle.len(),
        actual.len(),
        "{label}: число типов не совпало с оракулом"
    );
    for (name, expected) in oracle {
        assert_eq!(
            actual.get(*name).copied().unwrap_or(0),
            *expected,
            "{label}: потиповое число {name}"
        );
    }
}

// ---------------------------------------------------------------------------
// Юнит-кейсы роутера (без сокетов)
// ---------------------------------------------------------------------------

#[test]
fn kind_table_covers_21_observed_types() {
    // Кейс U1: таблица типов — ровно 21 имя провода, дублей нет, константа кейса сходится.
    assert_eq!(DispatchKind::ALL.len(), 21, "все 21 наблюдаемый тип");
    let names: BTreeSet<&str> = DispatchKind::ALL.iter().map(|kind| kind.as_str()).collect();
    assert_eq!(names.len(), 21, "дублей имён в ALL нет");
    let oracle: BTreeSet<&str> = LAB_TYPE_NAMES.into_iter().collect();
    assert_eq!(oracle.len(), 21, "оракул без дублей");
    assert_eq!(names, oracle, "ALL ⇄ 21 имя провода оракула");
    assert_eq!(DISPATCH_TYPE_COUNT_R7, 21, "константа числа типов");
}

#[test]
fn kind_from_name_exact_roundtrip() {
    // Кейс U2: from_name(as_str()) — тождество; регистр и мусор — None.
    for kind in DispatchKind::ALL {
        assert_eq!(
            DispatchKind::from_name(kind.as_str()),
            Some(kind),
            "roundtrip {}",
            kind.as_str()
        );
    }
    assert_eq!(
        DispatchKind::from_name("guild_member_list_update"),
        None,
        "регистрозависимо"
    );
    assert_eq!(DispatchKind::from_name(""), None, "пустое имя — не тип");
    assert_eq!(DispatchKind::from_name("nil"), None, "nil — не тип");
}

#[test]
fn route_known_keeps_seq_and_payload() {
    // Кейс U3: известный тип — событие с `s` и нетронутым payload.
    let d = list(vec![EtfValue::Int(7), EtfValue::Str("x".to_string())]);
    let value = dispatch_value("MESSAGE_CREATE", 42, d.clone());
    let event = route_dispatch(&value).expect("известный тип — событие");
    assert_eq!(event.kind, Some(DispatchKind::MessageCreate), "вид события");
    assert_eq!(event.name, "MESSAGE_CREATE", "имя провода");
    assert_eq!(event.seq, Some(42), "`s` сохранён");
    assert_eq!(event.data, d, "payload не разбирается и не теряется");

    // Оба представления `t` допустимы: атом и строка.
    let as_str_value = map(vec![
        ("op", EtfValue::Int(0)),
        ("t", EtfValue::Str("READY".to_string())),
        ("s", EtfValue::Int(1)),
        ("d", map(vec![("guilds", list(Vec::new()))])),
    ]);
    let event = route_dispatch(&as_str_value).expect("`t`-строка — тоже диспатч");
    assert_eq!(event.kind, Some(DispatchKind::Ready), "READY из строки");
    assert_eq!(event.name, "READY", "имя из строки");
}

#[test]
fn route_unknown_is_raw_with_name() {
    // Кейс U4: неизвестный тип — не ошибка, а raw-событие с сохранённым именем.
    let d = map(vec![("k", EtfValue::Int(1))]);
    for t in [
        atom("FUTURE_EVENT_X"),
        EtfValue::Str("FUTURE_EVENT_X".to_string()),
    ] {
        let value = map(vec![
            ("op", EtfValue::Int(0)),
            ("t", t),
            ("s", EtfValue::Int(5)),
            ("d", d.clone()),
        ]);
        let event = route_dispatch(&value).expect("неизвестный тип — raw, не ошибка");
        assert_eq!(event.kind, None, "raw-событие");
        assert_eq!(event.name, "FUTURE_EVENT_X", "имя не теряется");
        assert_eq!(event.seq, Some(5), "`s` сохранён");
        assert_eq!(event.data, d, "payload сохранён целиком");
    }
}

#[test]
fn route_lowercase_known_name_is_raw() {
    // Кейс N1 (нит ревью): регистр значим — «ready» не READY, а неизвестное имя (raw).
    let value = dispatch_value("ready", 4, map(vec![("guilds", list(Vec::new()))]));
    let event = route_dispatch(&value).expect("op0 с именем другого регистра — raw, не None");
    assert_eq!(event.kind, None, "«ready» ≠ READY");
    assert_eq!(event.name, "ready", "имя как пришло");
    assert_eq!(event.seq, Some(4), "`s` сохранён");
}

#[test]
fn route_ignores_non_dispatches() {
    // Кейс U5 (негативный контроль): не-op0 и кривые `t` — не диспатчи.
    for op in [10, 11, 7, 9] {
        assert_eq!(
            route_dispatch(&map(vec![("op", EtfValue::Int(op)), ("t", atom("READY"))])),
            None,
            "op{op} — не диспатч"
        );
    }
    assert_eq!(
        route_dispatch(&map(vec![("op", EtfValue::Int(0))])),
        None,
        "op0 без `t`"
    );
    assert_eq!(
        route_dispatch(&map(vec![
            ("op", EtfValue::Int(0)),
            ("t", EtfValue::Int(5))
        ])),
        None,
        "`t` не атом и не строка"
    );
    assert_eq!(
        route_dispatch(&map(vec![("t", atom("READY"))])),
        None,
        "без `op`"
    );
}

#[test]
fn route_seq_and_data_optional() {
    // Кейс U6: `s` и `d` необязательны; отрицательный `s` не влезает в u64.
    let no_seq = map(vec![
        ("op", EtfValue::Int(0)),
        ("t", atom("TYPING_START")),
        ("d", map(Vec::new())),
    ]);
    let event = route_dispatch(&no_seq).expect("op0 с известным типом");
    assert_eq!(event.seq, None, "нет `s` — нет seq");

    let no_data = map(vec![
        ("op", EtfValue::Int(0)),
        ("t", atom("TYPING_START")),
        ("s", EtfValue::Int(1)),
    ]);
    let event = route_dispatch(&no_data).expect("op0 без `d`");
    assert_eq!(event.data, EtfValue::Nil, "нет `d` — Nil");

    let negative = map(vec![
        ("op", EtfValue::Int(0)),
        ("t", atom("TYPING_START")),
        ("s", EtfValue::Int(-1)),
        ("d", map(Vec::new())),
    ]);
    let event = route_dispatch(&negative).expect("op0 с отрицательным `s`");
    assert_eq!(event.seq, None, "отрицательный `s` — не seq");

    // Нит N3 (ревью): `"s": "nil"` — «нет последовательности», а не число.
    let nil_seq = map(vec![
        ("op", EtfValue::Int(0)),
        ("t", atom("TYPING_START")),
        ("s", atom("nil")),
        ("d", map(Vec::new())),
    ]);
    let event = route_dispatch(&nil_seq).expect("op0 с `s`=\"nil\"");
    assert_eq!(event.seq, None, "`\"nil\"` в `s` — не seq");
}

// ---------------------------------------------------------------------------
// Пайп: офлайн, завершённый zstd-кадр через `push_binary` (без сокетов)
// ---------------------------------------------------------------------------

#[test]
fn pipe_routes_dispatches_in_order() {
    // Кейс P1: один кадр — три диспатча в порядке потока, raw со счётчиком.
    let mut pipe = GatewayPipe::new(Session::new(0));
    let effects = pipe
        .push_binary(&zstd_message(&sample_feed_terms()), 0)
        .expect("кадр принят");
    assert!(effects.is_empty(), "диспатчи не дают эффектов");

    let events: &[DispatchEvent] = pipe.events();
    assert_eq!(events.len(), 3, "три диспатча: READY, MESSAGE_CREATE, raw");
    let kinds: Vec<Option<DispatchKind>> = events.iter().map(|event| event.kind).collect();
    assert_eq!(
        kinds,
        [
            Some(DispatchKind::Ready),
            Some(DispatchKind::MessageCreate),
            None
        ],
        "порядок потока: READY → MESSAGE_CREATE → raw"
    );
    let names: Vec<&str> = events.iter().map(|event| event.name.as_str()).collect();
    assert_eq!(
        names,
        ["READY", "MESSAGE_CREATE", "FUTURE_EVENT_X"],
        "имена в порядке потока"
    );
    let seqs: Vec<Option<u64>> = events.iter().map(|event| event.seq).collect();
    assert_eq!(seqs, [Some(1), Some(2), Some(3)], "`s` сохранены");
    assert_eq!(events[2].kind, None, "raw виден вызывающему как kind: None");
    assert_eq!(pipe.unknown_dispatches(), 1, "raw посчитан");
    assert_eq!(pipe.session().last_seq(), Some(3), "сессия дочитала серию");
    assert_eq!(pipe.terms_parsed(), 4, "HELLO + три диспатча");
    assert!(pipe.last_ready().is_some(), "READY прошёл конвейер");
}

#[test]
fn pipe_take_events_drains_once() {
    // Кейс P2: дренаж отдаёт буфер один раз; счётчик raw накопительный.
    let mut pipe = GatewayPipe::new(Session::new(0));
    pipe.push_binary(&zstd_message(&sample_feed_terms()), 0)
        .expect("кадр принят");
    let taken = pipe.take_events();
    assert_eq!(taken.len(), 3, "дренаж отдаёт накопленное");
    // Нит N2 (ревью): дренаж сохраняет порядок потока (а не только состав).
    let names: Vec<&str> = taken.iter().map(|event| event.name.as_str()).collect();
    assert_eq!(
        names,
        ["READY", "MESSAGE_CREATE", "FUTURE_EVENT_X"],
        "порядок потока сохраняется и после дренажа"
    );
    assert!(pipe.events().is_empty(), "буфер пуст после дренажа");
    assert!(pipe.take_events().is_empty(), "повторный дренаж пуст");
    assert_eq!(
        pipe.unknown_dispatches(),
        1,
        "счётчик raw не сбрасывается дренажом"
    );
}

#[test]
fn pipe_hello_only_yields_no_events() {
    // Кейс P3 (негативный контроль): HELLO — не диспатч.
    let mut pipe = GatewayPipe::new(Session::new(0));
    pipe.push_binary(&zstd_message(&[hello_bytes(41_250)]), 0)
        .expect("HELLO принят");
    assert!(pipe.events().is_empty(), "HELLO не даёт событий");
    assert_eq!(pipe.unknown_dispatches(), 0, "неизвестных типов не было");
}

// ---------------------------------------------------------------------------
// Lab-кейсы по реальным артефактам (skip-aware)
// ---------------------------------------------------------------------------

#[test]
fn lab_all_dispatch_types_route_to_kinds() {
    // Кейс L1 (T-GW-08, union): все op0 семи снимков роутятся в известные виды.
    let mut texts = Vec::new();
    for name in LAB_FILES {
        let path = lab_root().join("gw-proto").join(name);
        let Some(text) = read_opt(&path) else {
            eprintln!("skip: нет lab-файла {}", path.display());
            return;
        };
        texts.push(text);
    }

    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut dispatches: Vec<(String, i64)> = Vec::new();
    for text in &texts {
        for line in text.lines() {
            if json_int_field(line, "op") != Some(0) {
                continue;
            }
            let Some(t) = json_str_field(line, "t") else {
                panic!("op0 без `t` в артефакте: {line}");
            };
            let Some(s) = json_int_field(line, "s") else {
                panic!("op0 без `s` в артефакте: {line}");
            };
            *counts.entry(t.clone()).or_insert(0) += 1;
            dispatches.push((t, s));
        }
    }
    assert_eq!(dispatches.len(), 1340, "op0 в union семи снимков");
    assert_eq!(
        dispatches.len() as u32,
        DISPATCH_COUNT_R7,
        "константа всех диспатчей захвата"
    );
    assert_eq!(counts.len(), 21, "уникальных типов");
    assert_eq!(
        counts.len(),
        DISPATCH_TYPE_COUNT_R7,
        "константа числа типов"
    );
    let union_oracle: BTreeMap<&str, u64> = LAB_TYPE_COUNTS.into_iter().collect();
    assert_eq!(union_oracle.len(), 21, "оракул union без дублей");
    assert_type_counts(&counts, &LAB_TYPE_COUNTS, "union");

    // Каждый наблюдаемый (t, s) → синтетический терм → типизированное событие; raw — 0.
    let mut routed: BTreeMap<String, u64> = BTreeMap::new();
    let mut raw = 0u64;
    for (t, s) in &dispatches {
        let value = map(vec![
            ("op", EtfValue::Int(0)),
            ("t", atom(t)),
            ("s", EtfValue::Int(*s)),
            ("d", map(Vec::new())),
        ]);
        let event =
            route_dispatch(&value).unwrap_or_else(|| panic!("диспатч {t} s={s} не роутится"));
        let expected =
            DispatchKind::from_name(t).unwrap_or_else(|| panic!("тип {t} отсутствует в таблице"));
        assert_eq!(event.kind, Some(expected), "вид для {t}");
        assert_eq!(event.name.as_str(), t.as_str(), "имя {t} не искажено");
        assert_eq!(
            event.seq,
            Some(u64::try_from(*s).expect("`s` захвата неотрицателен")),
            "seq для {t} s={s}"
        );
        match event.kind {
            Some(_) => *routed.entry(event.name.clone()).or_insert(0) += 1,
            None => raw += 1,
        }
    }
    assert_eq!(raw, 0, "у наблюдаемых типов raw-событий нет");
    assert_type_counts(&routed, &LAB_TYPE_COUNTS, "union/replay");
}

#[test]
fn lab_real_stream_routes_known_kinds() {
    // Кейс L2: реальные кадры `.1389` — весь поток (137 термов) роутится без raw.
    let path = lab_root().join("gw-proto/zstd-frames-95974_1389.hexl");
    let Some(text) = read_opt(&path) else {
        eprintln!("skip: нет lab-файла {}", path.display());
        return;
    };
    let frames: Vec<Vec<u8>> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(unhex)
        .collect();
    assert_eq!(frames.len(), 137, "кадров в снимке потока");
    let compressed: u64 = frames.iter().map(|frame| frame.len() as u64).sum();
    assert_eq!(compressed, 104_066, "сжатых байт в снимке");

    // F-GW-01: незавершённый кадр больше не удерживает окно — 824 816 Б отдаются целиком.
    let mut stream = ZstdStream::new();
    let mut out = Vec::new();
    for (index, frame) in frames.iter().enumerate() {
        out.extend(
            stream
                .push(frame)
                .unwrap_or_else(|error| panic!("кадр {index}: {error}")),
        );
    }
    assert_eq!(out.len(), 824_816, "распакован весь поток");
    assert_eq!(stream.pending_bytes(), 0, "хвоста в накопителе нет");

    // Разбор термов: 137 полных, остатка нет (раньше хвост обрезался окном на 44-м терме).
    let mut offset = 0usize;
    let mut terms = 0usize;
    let mut routed: Vec<DispatchEvent> = Vec::new();
    let mut non_dispatch_ops: Vec<Option<i64>> = Vec::new();
    while offset < out.len() {
        match etf::decode(&out[offset..]) {
            Ok((value, consumed)) => {
                assert!(consumed > 0, "нулевой consumed у терма {terms}");
                offset += consumed;
                terms += 1;
                if value.get("op").and_then(EtfValue::as_int) == Some(0) {
                    let event = route_dispatch(&value)
                        .unwrap_or_else(|| panic!("реальный op0 #{} не роутится", routed.len()));
                    routed.push(event);
                } else {
                    non_dispatch_ops.push(value.get("op").and_then(EtfValue::as_int));
                }
            }
            Err(EtfError::UnexpectedEof { .. }) => break,
            Err(error) => panic!("терм на смещении {offset}: {error}"),
        }
    }
    assert_eq!(terms, 137, "полных термов в потоке");
    assert_eq!(routed.len(), 113, "из них op0");
    assert_eq!(non_dispatch_ops.len(), 24, "не-диспатчей (op10/op11)");
    assert!(
        non_dispatch_ops
            .iter()
            .all(|op| matches!(op, Some(10) | Some(11))),
        "не-диспатчи — только op10/op11: {non_dispatch_ops:?}"
    );
    assert_eq!(
        routed.len() + non_dispatch_ops.len(),
        terms,
        "баланс термов"
    );
    assert_eq!(out.len() - offset, 0, "обрезанного хвоста нет");

    // Виды и серии: все известные, raw нет, s = 1..113.
    let seqs: Vec<u64> = routed
        .iter()
        .map(|event| event.seq.expect("`s` реального диспатча"))
        .collect();
    assert_eq!(
        seqs,
        (1..=113).collect::<Vec<u64>>(),
        "s = 1..113 без разрывов"
    );
    let raw = routed.iter().filter(|event| event.kind.is_none()).count();
    assert_eq!(raw, 0, "в потоке нет неизвестных типов");
    assert_eq!(
        routed.first().and_then(|event| event.kind),
        Some(DispatchKind::Ready),
        "первый routed — READY"
    );
    assert_eq!(
        routed.first().and_then(|event| event.seq),
        Some(1),
        "первый routed — s=1"
    );
    assert_eq!(
        routed.last().and_then(|event| event.seq),
        Some(113),
        "последний routed — s=113"
    );

    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    for event in &routed {
        *counts.entry(event.name.clone()).or_insert(0) += 1;
    }
    let stream_oracle: BTreeMap<&str, u64> = LAB_STREAM_COUNTS.into_iter().collect();
    assert_eq!(stream_oracle.len(), 11, "оракул потока без дублей");
    assert_type_counts(&counts, &LAB_STREAM_COUNTS, "поток .1389");
}
