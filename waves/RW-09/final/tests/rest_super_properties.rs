//! X-Super-Properties: 22 поля, JSON и base64 (волна RW-09, P1-08 / T-REST-04).
//!
//! **Провенанс оракулов** (живой снимок, не подгонка под реализацию):
//! - снимок `/home/user/clean-room-oracle/d02-snapshot/capture-run2.snap.jsonl`, md5
//!   `abfe14b76cec1e559c3b45522bc44b7a`; запрос `requestId = "95974.2323"`
//!   (GET `/api/v9/channels/{id}/messages?limit=12`), заголовок `X-Super-Properties` —
//!   1084 символа base64 → 811 байт компактного JSON (22 ключа, порядок из захвата);
//! - выдержки в репозитории (read-only): `tests/data/rest-super-properties-2323.b64` —
//!   1084 симв. + `\n`, md5 `b9e8f8430d8b496df50da9419e760a2d`;
//!   `tests/data/rest-super-properties-2323.json` — 811 байт + `\n`, md5
//!   `90b06ded88a98dd05d3b821f6bedc52b`; `tests/data/rest-super-properties-22.tsv` — 22 строки,
//!   md5 `95e2f8616043041431fcef2d38d6fac5`;
//! - разбор полей: `docs/re/rest-d02.md` §6.2 (md5 `c7e61929677efab6bdac86a05a778fce`).
//! - команды снятия (выполнены координатором, см. `waves/RW-09/TZ.md` §3.1):
//!   `jq -r 'select(.ev=="req" and .requestId=="95974.2323") | (.headers // {}) | to_entries[]
//!   | select(.key|ascii_downcase=="x-super-properties") | .value' "$SNAP"`, затем
//!   `base64 -d` и `jq -r 'to_entries[] | [.key, (.value|type), (.value|tostring)] | @tsv'`.
//! - контрольные счётчики: 22 поля, из них 18 строк, 2 числа (`u32`: 641004, 94186),
//!   1 bool (`false`), 1 null (`client_event_source`); base64 оканчивается на `==`.
//!
//! Тест-данные обязательны: нет файла — падение, а не skip.

use discord_core::rest::base64::{self, Base64Error};
use discord_core::rest::super_properties::{
    FIELD_COUNT, SuperProperties, SuperPropertiesError, field_names,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const LIVE_B64: &str = "rest-super-properties-2323.b64";
const LIVE_JSON: &str = "rest-super-properties-2323.json";
const FIELDS_TSV: &str = "rest-super-properties-22.tsv";

/// Оракул `docs/re/rest-d02.md` §6.2: канонический порядок 22 полей (как в захвате).
const OBSERVED_FIELDS: [&str; FIELD_COUNT] = [
    "os",
    "browser",
    "release_channel",
    "client_version",
    "os_version",
    "os_arch",
    "app_arch",
    "system_locale",
    "has_client_mods",
    "client_launch_id",
    "browser_user_agent",
    "browser_version",
    "window_manager",
    "distro",
    "runtime_environment",
    "display_server",
    "client_build_number",
    "native_build_number",
    "client_event_source",
    "launch_signature",
    "client_heartbeat_session_id",
    "client_app_state",
];

/// Путь к выдержке в `tests/data` (отсутствие файла — падение на чтении).
fn data_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

/// Сырые байты выдержки; нет файла — паника с путём (не skip).
fn read_data(name: &str) -> Vec<u8> {
    let path = data_path(name);
    fs::read(&path).unwrap_or_else(|error| panic!("нет тест-данных {}: {error}", path.display()))
}

/// Побайтовый живой JSON (811 байт, хвостовой `\n` файла отброшен).
fn live_json_bytes() -> Vec<u8> {
    let raw = read_data(LIVE_JSON);
    assert_eq!(
        raw.last().copied(),
        Some(b'\n'),
        "{LIVE_JSON}: хвостовой \\n"
    );
    assert_eq!(raw.len(), 812, "{LIVE_JSON}: 811 байт JSON + \\n");
    let body = raw[..raw.len() - 1].to_vec();
    assert!(
        !body.contains(&b'\n'),
        "{LIVE_JSON}: компактный JSON без переводов строк"
    );
    body
}

/// Живой JSON как `String` (UTF-8 — часть контракта).
fn live_json() -> String {
    String::from_utf8(live_json_bytes())
        .unwrap_or_else(|error| panic!("{LIVE_JSON}: не UTF-8: {error}"))
}

/// Живой заголовок base64: 1084 символа, хвостовой `\n` файла отброшен.
fn live_b64() -> String {
    let raw = read_data(LIVE_B64);
    assert_eq!(
        raw.last().copied(),
        Some(b'\n'),
        "{LIVE_B64}: хвостовой \\n"
    );
    let text = String::from_utf8(raw[..raw.len() - 1].to_vec())
        .unwrap_or_else(|error| panic!("{LIVE_B64}: не ASCII/UTF-8: {error}"));
    assert_eq!(text.len(), 1084, "{LIVE_B64}: длина живого заголовка");
    assert!(text.ends_with("=="), "{LIVE_B64}: канонический `=`-паддинг");
    text
}

/// Строки TSV выдержки (пустые строки отброшены; колонки — по `\t`).
fn tsv_rows() -> Vec<Vec<String>> {
    let text = String::from_utf8(read_data(FIELDS_TSV))
        .unwrap_or_else(|error| panic!("{FIELDS_TSV}: не UTF-8: {error}"));
    text.lines()
        .filter(|line| !line.is_empty())
        .map(|line| line.split('\t').map(str::to_owned).collect())
        .collect()
}

/// Значение поля в виде строки — ровно как колонка 3 TSV (`null` для отсутствующего
/// `client_event_source`).
fn field_text(sp: &SuperProperties, name: &str) -> String {
    match name {
        "os" => sp.os.clone(),
        "browser" => sp.browser.clone(),
        "release_channel" => sp.release_channel.clone(),
        "client_version" => sp.client_version.clone(),
        "os_version" => sp.os_version.clone(),
        "os_arch" => sp.os_arch.clone(),
        "app_arch" => sp.app_arch.clone(),
        "system_locale" => sp.system_locale.clone(),
        "has_client_mods" => sp.has_client_mods.to_string(),
        "client_launch_id" => sp.client_launch_id.clone(),
        "browser_user_agent" => sp.browser_user_agent.clone(),
        "browser_version" => sp.browser_version.clone(),
        "window_manager" => sp.window_manager.clone(),
        "distro" => sp.distro.clone(),
        "runtime_environment" => sp.runtime_environment.clone(),
        "display_server" => sp.display_server.clone(),
        "client_build_number" => sp.client_build_number.to_string(),
        "native_build_number" => sp.native_build_number.to_string(),
        "client_event_source" => sp
            .client_event_source
            .clone()
            .unwrap_or_else(|| "null".to_string()),
        "launch_signature" => sp.launch_signature.clone(),
        "client_heartbeat_session_id" => sp.client_heartbeat_session_id.clone(),
        "client_app_state" => sp.client_app_state.clone(),
        other => panic!("{FIELDS_TSV}: неизвестное поле {other:?}"),
    }
}

/// Тип поля в терминах TSV (`string`/`number`/`boolean`/`null`).
fn field_type(sp: &SuperProperties, name: &str) -> &'static str {
    match name {
        "has_client_mods" => "boolean",
        "client_build_number" | "native_build_number" => "number",
        "client_event_source" => {
            if sp.client_event_source.is_none() {
                "null"
            } else {
                "string"
            }
        }
        "os"
        | "browser"
        | "release_channel"
        | "client_version"
        | "os_version"
        | "os_arch"
        | "app_arch"
        | "system_locale"
        | "client_launch_id"
        | "browser_user_agent"
        | "browser_version"
        | "window_manager"
        | "distro"
        | "runtime_environment"
        | "display_server"
        | "launch_signature"
        | "client_heartbeat_session_id"
        | "client_app_state" => "string",
        other => panic!("{FIELDS_TSV}: неизвестное поле {other:?}"),
    }
}

/// Замена ровно одного вхождения (нет вхождения — паника теста, а не тихая подмена).
fn mutate(text: &str, from: &str, to: &str) -> String {
    assert!(text.contains(from), "в живом JSON нет {from:?}");
    text.replacen(from, to, 1)
}

/// Ожидание `Err(SuperPropertiesError::Json(..))` с непустым сообщением и позицией.
fn assert_json_error(result: Result<SuperProperties, SuperPropertiesError>, label: &str) {
    match result {
        Err(SuperPropertiesError::Json(message)) => {
            assert!(!message.trim().is_empty(), "{label}: пустое сообщение");
            assert!(
                message.chars().any(|ch| ch.is_ascii_digit()),
                "{label}: в сообщении нет позиции: {message:?}"
            );
        }
        other => panic!("{label}: ожидалась Json-ошибка, получено {other:?}"),
    }
}

/// P1: `FIELD_COUNT == 22`; `field_names()` построчно равен колонке 1 TSV и оракулу §6.2.
#[test]
fn p1_fields_are_22_in_order() {
    assert_eq!(FIELD_COUNT, 22, "число полей супер-свойств");
    let names = field_names();
    assert_eq!(names.len(), FIELD_COUNT, "field_names() — 22 имени");
    assert_eq!(
        names, OBSERVED_FIELDS,
        "порядок имён из docs/re/rest-d02.md §6.2"
    );

    let rows = tsv_rows();
    assert_eq!(rows.len(), FIELD_COUNT, "строк в {FIELDS_TSV}");
    let mut tsv_names = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row.len(), 3, "строка {index}: колонки field,type,value");
        tsv_names.push(row[0].as_str());
    }
    assert_eq!(
        names.as_slice(),
        tsv_names.as_slice(),
        "колонка 1 {FIELDS_TSV} = field_names()"
    );

    let unique: BTreeSet<&str> = names.iter().copied().collect();
    assert_eq!(unique.len(), FIELD_COUNT, "имена полей уникальны");
}

/// P2: `from_json(live)` даёт каждое поле по TSV — значения дословно и типы.
#[test]
fn p2_parses_live_json_fields() {
    let live = live_json();
    let sp = SuperProperties::from_json(&live).expect("живой JSON должен разбираться");

    let rows = tsv_rows();
    assert_eq!(rows.len(), FIELD_COUNT, "строк в {FIELDS_TSV}");
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row.len(), 3, "строка {index}: колонки field,type,value");
        let name = row[0].as_str();
        assert_eq!(
            field_type(&sp, name),
            row[1],
            "строка {index}: тип поля {name}"
        );
        assert_eq!(
            field_text(&sp, name),
            row[2],
            "строка {index}: значение поля {name}"
        );
        *kinds.entry(row[1].clone()).or_insert(0) += 1;
    }
    assert_eq!(kinds.get("string"), Some(&18), "18 строк в живых данных");
    assert_eq!(kinds.get("number"), Some(&2), "2 числа (u32)");
    assert_eq!(kinds.get("boolean"), Some(&1), "1 bool");
    assert_eq!(kinds.get("null"), Some(&1), "1 null");

    // Ключевые значения — независимо от TSV (оракул §6.2).
    assert_eq!(sp.os, "Linux");
    assert_eq!(sp.browser, "Discord Client");
    assert_eq!(sp.client_version, "1.0.162");
    assert_eq!(sp.distro, "\"Arch Linux\"", "кавычки — часть значения");
    assert_eq!(sp.window_manager, "KDE,unknown");
    assert_eq!(sp.client_build_number, 641_004);
    assert_eq!(sp.native_build_number, 94_186);
    assert!(!sp.has_client_mods, "has_client_mods = false");
    assert_eq!(sp.client_event_source, None, "client_event_source = null");
    assert_eq!(sp.client_app_state, "focused");
}

/// P3: `to_json(from_json(live))` побайтово равен выдержке `.json` (без хвостового `\n`).
#[test]
fn p3_live_json_roundtrip_bytewise() {
    let live_bytes = live_json_bytes();
    let live = live_json();
    let sp = SuperProperties::from_json(&live).expect("живой JSON должен разбираться");

    let produced = sp.to_json();
    assert_eq!(
        produced.as_bytes(),
        live_bytes.as_slice(),
        "канонический JSON — побайтово как живой заголовок"
    );
    assert_eq!(produced.len(), 811, "длина живого JSON");
    assert!(!produced.contains('\n'), "канонический JSON компактный");

    // Идемпотентность: повторный разбор и повторная запись совпадают.
    let reparsed = SuperProperties::from_json(&produced).expect("канонический JSON разбирается");
    assert_eq!(reparsed, sp, "round-trip структуры");
    assert_eq!(reparsed.to_json(), produced, "round-trip JSON");

    // Пробелы/переводы строк между токенами допустимы (RFC 8259 whitespace).
    let spaced = mutate(&live, "{\"os\"", "{ \n\t\"os\"");
    let spaced = mutate(&spaced, "\"os\":", "\"os\" \t : ");
    let spaced = mutate(
        &spaced,
        "\"client_app_state\":\"focused\"}",
        "\"client_app_state\" : \"focused\" } \n",
    );
    let from_spaced = SuperProperties::from_json(&spaced).expect("whitespace между токенами");
    assert_eq!(from_spaced, sp, "whitespace не влияет на поля");
    assert_eq!(
        from_spaced.to_json(),
        produced,
        "whitespace не влияет на канонический вывод"
    );
}

/// P4: base64 живого заголовка — `to_base64(parsed)` и `from_base64(live)`.
#[test]
fn p4_live_base64_roundtrip() {
    let sp = SuperProperties::from_json(&live_json()).expect("живой JSON должен разбираться");
    let b64 = live_b64();

    assert_eq!(sp.to_base64(), b64, "to_base64(parsed) = живой заголовок");
    assert_eq!(
        SuperProperties::from_base64(&b64).expect("живой base64 разбирается"),
        sp,
        "from_base64(live) = структура"
    );

    // Слои согласованы: base64 — ровно encode(to_json bytes) и decode(..) = JSON-байты.
    assert_eq!(sp.to_base64(), base64::encode(sp.to_json().as_bytes()));
    assert_eq!(
        base64::decode(&b64).expect("живой base64 декодируется"),
        live_json_bytes(),
        "decode(live b64) = побайтовый JSON"
    );
    assert!(
        !sp.to_base64().contains('\n'),
        "to_base64 без переводов строк"
    );
}

/// P5: экранирование RFC 8259 и round-trip юникода/управляющих символов.
#[test]
fn p5_escaping_roundtrip() {
    let value = SuperProperties {
        os: "Linux \"quoted\"".to_string(),
        browser: "discord-lab\\client".to_string(),
        release_channel: "stable\nnightly".to_string(),
        client_version: "0.1.0\tbeta".to_string(),
        os_version: "ядро ё: \u{0001}\u{0008}\u{000c}\u{000d}\u{001f}".to_string(),
        os_arch: "x64".to_string(),
        app_arch: "x64".to_string(),
        system_locale: "ru".to_string(),
        has_client_mods: true,
        client_launch_id: "id-ё".to_string(),
        browser_user_agent: "Mozilla/5.0 (X11; Linux x86_64) 🦀".to_string(),
        browser_version: "42.11.12".to_string(),
        window_manager: "KDE,\"unknown\"".to_string(),
        distro: "\"Arch Linux\"".to_string(),
        runtime_environment: "native".to_string(),
        display_server: "wayland".to_string(),
        client_build_number: u32::MAX,
        native_build_number: 0,
        client_event_source: Some("science:ё🦀".to_string()),
        launch_signature: "sig\\path/ё".to_string(),
        client_heartbeat_session_id: "hb-\u{0007}".to_string(),
        client_app_state: "focused".to_string(),
    };

    let json = value.to_json();
    assert!(json.contains(r#"\""#), "кавычка экранирована: {json}");
    assert!(json.contains(r#"\\"#), "обратный слэш экранирован: {json}");
    assert!(json.contains(r#"\n"#), "перевод строки экранирован: {json}");
    assert!(json.contains(r#"\t"#), "таб экранирован: {json}");
    assert!(json.contains(r#"\b"#), "backspace экранирован: {json}");
    assert!(json.contains(r#"\f"#), "form feed экранирован: {json}");
    assert!(
        json.contains(r#"\r"#),
        "carriage return экранирован: {json}"
    );
    assert!(
        !json.chars().any(|ch| (ch as u32) < 0x20),
        "в JSON нет сырых управляющих символов: {json:?}"
    );
    // Прочие управляющие — \u00XX (регистр hex-цифр контрактом не зафиксирован).
    let lowered = json.to_ascii_lowercase();
    assert!(lowered.contains(r#"\u0001"#), "\\u0001: {json}");
    assert!(lowered.contains(r#"\u001f"#), "\\u001f: {json}");
    // Юникод не деградирует (не \\u-эскейпится).
    assert!(json.contains('ё'), "unicode ё как есть");
    assert!(json.contains('🦀'), "эмодзи как есть");

    let parsed = SuperProperties::from_json(&json).expect("свой JSON разбирается обратно");
    assert_eq!(parsed, value, "from_json(to_json(x)) == x");

    // Входные \\u-эскейпы разбираются (в т.ч. суррогатная пара эмодзи).
    let live = live_json();
    let cyrillic = mutate(
        &live,
        "\"system_locale\":\"ru\"",
        "\"system_locale\":\"\\u0440\\u0443\"",
    );
    assert_eq!(
        SuperProperties::from_json(&cyrillic)
            .expect("\\u-escape разбирается")
            .system_locale,
        "ру"
    );
    let emoji = mutate(
        &live,
        "\"system_locale\":\"ru\"",
        "\"system_locale\":\"\\ud83e\\udd80\"",
    );
    assert_eq!(
        SuperProperties::from_json(&emoji)
            .expect("суррогатная пара разбирается")
            .system_locale,
        "🦀"
    );
}

/// P6: строгий парсер отвергает пропуски/дубликаты/лишние ключи/типы/мусор; валидный — принимает.
#[test]
fn p6_parser_rejects_bad_input() {
    let live = live_json();
    let cases: Vec<(&str, String)> = vec![
        ("пропущен os", mutate(&live, "\"os\":\"Linux\",", "")),
        ("лишний ключ", mutate(&live, "{", "{\"extra\":1,")),
        ("дубликат os", mutate(&live, "{", "{\"os\":\"Linux\",")),
        (
            "число как строка",
            mutate(
                &live,
                "\"client_build_number\":641004",
                "\"client_build_number\":\"641004\"",
            ),
        ),
        (
            "отрицательное число",
            mutate(
                &live,
                "\"client_build_number\":641004",
                "\"client_build_number\":-1",
            ),
        ),
        (
            "дробное число",
            mutate(
                &live,
                "\"client_build_number\":641004",
                "\"client_build_number\":1.5",
            ),
        ),
        (
            "null вместо строки",
            mutate(&live, "\"browser\":\"Discord Client\"", "\"browser\":null"),
        ),
        (
            "число вместо строки",
            mutate(&live, "\"os\":\"Linux\"", "\"os\":123"),
        ),
        (
            "bool вместо числа",
            mutate(
                &live,
                "\"client_build_number\":641004",
                "\"client_build_number\":true",
            ),
        ),
        (
            "строка вместо bool",
            mutate(
                &live,
                "\"has_client_mods\":false",
                "\"has_client_mods\":\"false\"",
            ),
        ),
        (
            "null вместо числа",
            mutate(
                &live,
                "\"native_build_number\":94186",
                "\"native_build_number\":null",
            ),
        ),
        (
            "запятая в конце",
            mutate(
                &live,
                "\"client_app_state\":\"focused\"}",
                "\"client_app_state\":\"focused\",}",
            ),
        ),
        (
            "двойная запятая",
            mutate(&live, "\"Linux\",", "\"Linux\",,"),
        ),
        ("мусор после объекта", format!("{live}{{}}")),
        ("мусор перед объектом", format!("x{live}")),
        ("пустой объект", "{}".to_string()),
        ("незакрытый объект", live[..live.len() - 1].to_string()),
        (
            "незакрытая строка",
            mutate(
                &live,
                "\"client_app_state\":\"focused\"}",
                "\"client_app_state\":\"focused}",
            ),
        ),
        (
            "сырой перевод строки в строке",
            mutate(&live, "\"os\":\"Linux\"", "\"os\":\"Li\nnux\""),
        ),
        (
            "негодный escape",
            mutate(&live, "\"os\":\"Linux\"", "\"os\":\"Li\\qux\""),
        ),
    ];

    // Положительный контроль: парсер не «отвергает всё».
    assert!(
        SuperProperties::from_json(&live).is_ok(),
        "валидный живой JSON принимается"
    );

    for (label, text) in &cases {
        assert_json_error(SuperProperties::from_json(text), label);
    }
}

/// P7: границы `u32` — `0` и `4294967295` принимаются, переполнение и нецелые — нет.
#[test]
fn p7_numbers_bounds() {
    let live = live_json();

    let zeros = mutate(
        &mutate(
            &live,
            "\"client_build_number\":641004",
            "\"client_build_number\":0",
        ),
        "\"native_build_number\":94186",
        "\"native_build_number\":0",
    );
    let sp_zero = SuperProperties::from_json(&zeros).expect("нули — валидные u32");
    assert_eq!(sp_zero.client_build_number, 0);
    assert_eq!(sp_zero.native_build_number, 0);

    let max = mutate(
        &mutate(
            &live,
            "\"client_build_number\":641004",
            "\"client_build_number\":4294967295",
        ),
        "\"native_build_number\":94186",
        "\"native_build_number\":4294967295",
    );
    let sp_max = SuperProperties::from_json(&max).expect("u32::MAX — валидный");
    assert_eq!(sp_max.client_build_number, u32::MAX);
    assert_eq!(sp_max.native_build_number, u32::MAX);
    let written = sp_max.to_json();
    assert!(
        written.contains("\"client_build_number\":4294967295"),
        "u32::MAX без потерь: {written}"
    );
    assert!(
        written.contains("\"native_build_number\":4294967295"),
        "u32::MAX без потерь: {written}"
    );

    for bad in [
        "4294967296",
        "18446744073709551616",
        "-1",
        "1.5",
        "1e3",
        "0641004",
    ] {
        let text = mutate(
            &live,
            "\"client_build_number\":641004",
            &format!("\"client_build_number\":{bad}"),
        );
        assert_json_error(SuperProperties::from_json(&text), bad);
    }
}

/// P8: ошибки base64-слоя — типизированы и не подменяются JSON-ошибкой (и наоборот).
#[test]
fn p8_base64_layer_errors() {
    // «!!!» — недопустимый символ (W9a проверяет символы раньше общей длины, кейс B3/P8).
    match SuperProperties::from_base64("!!!") {
        Err(SuperPropertiesError::Base64(Base64Error::InvalidChar { byte, index })) => {
            assert_eq!(byte, b'!', "байт недопустимого символа");
            assert_eq!(index, 0, "позиция недопустимого символа");
        }
        other => panic!("from_base64(\"!!!\"): ожидалась Base64(InvalidChar), получено {other:?}"),
    }

    // 11 символов алфавита без паддинга — некратная длина (не % 4).
    match SuperProperties::from_base64("AAAAAAAAAAA") {
        Err(SuperPropertiesError::Base64(Base64Error::InvalidLength(11))) => {}
        other => {
            panic!(
                "from_base64(11 символов): ожидалась Base64(InvalidLength(11)), получено {other:?}"
            )
        }
    }

    // Ошибка паддинга проходит через обёртку без подмены варианта.
    match SuperProperties::from_base64("A===") {
        Err(SuperPropertiesError::Base64(Base64Error::InvalidPadding { index: 1 })) => {}
        other => {
            panic!(
                "from_base64(\"A===\"): ожидалась Base64(InvalidPadding{{1}}), получено {other:?}"
            )
        }
    }

    // Валидный base64 без нужного JSON: «[]» и «{}» — ошибка JSON-слоя с позицией.
    assert_json_error(SuperProperties::from_base64("W10="), "base64 от []");
    assert_json_error(SuperProperties::from_base64("e30="), "base64 от {}");

    // Валидный base64, но не UTF-8 — тоже Json-ошибка (слой from_utf8).
    assert_json_error(
        SuperProperties::from_base64("/w=="),
        "base64 от не-UTF-8 [0xff]",
    );
}
