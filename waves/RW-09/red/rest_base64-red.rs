//! Base64 (RFC 4648) для `X-Super-Properties` — волна RW-09 / P1-08 (T-REST-04), воркер W9a.
//!
//! **Провенанс оракулов** (независимые артефакты и правила, не подгонка под реализацию):
//! - Живой снимок `/home/sanya/re-deep/d02-snapshot/capture-run2.snap.jsonl`,
//!   md5 `abfe14b76cec1e559c3b45522bc44b7a`; запрос `requestId = "95974.2323"`
//!   (GET `/api/v9/channels/{id}/messages?limit=12`), заголовок `X-Super-Properties`.
//! - `tests/data/rest-super-properties-2323.b64` — 1084 символа + `\n`, md5
//!   `b9e8f8430d8b496df50da9419e760a2d` (без хвостового `\n` —
//!   `7516c16078b0963c81e141565740cdfa`).
//! - `tests/data/rest-super-properties-2323.json` — 811 байт + `\n`, md5
//!   `90b06ded88a98dd05d3b821f6bedc52b`; 22 ключа, base64 оканчивается на `==`.
//! - `tests/data/rest-base64-vectors.tsv` — 10 векторов, md5 `8b5bb8c97841773bd13e0fb9f29e8b51`;
//!   base64-колонка снята python3: `base64.b64encode(bytes.fromhex(hex))`.
//! - Команды снятия выдержек (ТЗ §3.1, выполнены координатором):
//!   ```text
//!   jq -r 'select(.ev=="req" and .requestId=="95974.2323") | (.headers // {}) | to_entries[]
//!          | select(.key|ascii_downcase=="x-super-properties") | .value' "$SNAP" \
//!     > rest-super-properties-2323.b64
//!   base64 -d < rest-super-properties-2323.b64 > rest-super-properties-2323.json
//!   ```
//! - Счётчики: 10 векторов с входами 0, 1, 2, 3, 4, 5, 6, 64, 256 и 1 байт (выходы 0, 4, 4, 4,
//!   8, 8, 8, 88, 344, 4 символа); живой заголовок 1084 симв. → 811 байт.
//!
//! Тест-данные обязательны: файла нет — падение, а не skip (выдержки лежат в репозитории).

use discord_core::rest::base64::{Base64Error, decode, encode};
use std::fs;
use std::path::{Path, PathBuf};

const VECTORS_TSV: &str = "rest-base64-vectors.tsv";
const LIVE_B64: &str = "rest-super-properties-2323.b64";
const LIVE_JSON: &str = "rest-super-properties-2323.json";

/// Имена и порядок векторов — оракул ТЗ §3.2 (строки `rest-base64-vectors.tsv`).
const VECTOR_NAMES: [&str; 10] = [
    "empty",
    "f",
    "fo",
    "foo",
    "foob",
    "fooba",
    "foobar",
    "bytes-00-3f",
    "bytes-00-ff",
    "one-00",
];

/// Путь к выдержке в `tests/data` (от корня крейта, а не от текущего каталога прогона).
fn data_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

/// Текстовые тест-данные: файла нет — падение с путём (оракул обязателен).
fn read_data_text(name: &str) -> String {
    let path = data_path(name);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("нет тест-данных {}: {error}", path.display()))
}

/// Бинарные тест-данные: файла нет — падение с путём.
fn read_data_bytes(name: &str) -> Vec<u8> {
    let path = data_path(name);
    fs::read(&path).unwrap_or_else(|error| panic!("нет тест-данных {}: {error}", path.display()))
}

/// Строки TSV из `tests/data` (пустые строки отброшены, колонки — по `\t`).
fn data_rows(name: &str) -> Vec<Vec<String>> {
    read_data_text(name)
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| line.split('\t').map(str::to_owned).collect())
        .collect()
}

/// hex-цифра → значение (вход TSV: колонка 2 — hex байтов).
fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => panic!("не hex-цифра: {byte:#04x}"),
    }
}

/// hex-строка (нижний регистр) → байты; нечётная длина/чужой символ — падение теста.
fn hex_to_bytes(hex: &str) -> Vec<u8> {
    assert!(
        hex.len().is_multiple_of(2),
        "hex-строка нечётной длины: {}",
        hex.len()
    );
    let digits = hex.as_bytes();
    let mut bytes = Vec::with_capacity(digits.len() / 2);
    for pair in digits.chunks(2) {
        bytes.push((hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]));
    }
    bytes
}

/// B1: все 10 векторов (RFC 4648 §10 и синтетические) — decode и encode сходятся с python3.
#[test]
fn rfc_vectors() {
    let rows = data_rows(VECTORS_TSV);
    assert_eq!(rows.len(), 10, "10 векторов в {VECTORS_TSV}");

    let mut names = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row.len(), 3, "строка {index}: колонки name,hex,base64");
        let name = row[0].as_str();
        let bytes = hex_to_bytes(&row[1]);
        let expected = row[2].as_str();

        assert_eq!(
            decode(expected),
            Ok(bytes.clone()),
            "{name}: decode(base64) = hex-байты"
        );
        assert_eq!(
            encode(&bytes),
            expected,
            "{name}: encode(hex-байты) = base64"
        );
        assert!(
            expected.len().is_multiple_of(4),
            "{name}: длина канонического base64 кратна 4"
        );
        names.push(name);
    }
    assert_eq!(
        names.as_slice(),
        VECTOR_NAMES.as_slice(),
        "имена/порядок векторов — оракул §3.2"
    );
}

/// B2: живой заголовок `X-Super-Properties` (requestId 95974.2323) — побайтовый round-trip.
#[test]
fn live_header_roundtrip() {
    let b64_file = read_data_text(LIVE_B64);
    let b64 = b64_file
        .strip_suffix('\n')
        .expect("выдержка .b64 оканчивается одним переводом строки");
    assert!(
        !b64.bytes().any(|byte| byte.is_ascii_whitespace()),
        "в base64 нет пробелов/переводов строк"
    );
    assert_eq!(b64.len(), 1084, "длина живого заголовка — 1084 символа");
    assert!(b64.ends_with("=="), "живой base64 оканчивается на '=='");

    let json_file = read_data_bytes(LIVE_JSON);
    let json = json_file
        .strip_suffix(b"\n")
        .expect("выдержка .json оканчивается одним переводом строки");
    assert_eq!(json.len(), 811, "декодированный живой JSON — 811 байт");

    let decoded = decode(b64).expect("живой заголовок декодируется без ошибок");
    assert_eq!(
        decoded.as_slice(),
        json,
        "decode(b64) = байты .json без '\\n'"
    );

    assert_eq!(encode(json), b64, "encode(.json без '\\n') = живой base64");
    assert_eq!(encode(&decoded), b64);

    let text = std::str::from_utf8(&decoded).expect("живой JSON — валидный UTF-8");
    assert!(
        text.starts_with(r#"{"os":"Linux","browser":"#),
        "живой JSON начинается с os/browser"
    );
    assert!(
        text.ends_with('}'),
        "живой JSON закрывается фигурной скобкой"
    );
}

/// B3: ошибки типизированы и содержат позицию; пустая строка — валидный пустой вход.
#[test]
fn errors_are_typed() {
    assert_eq!(decode("AA"), Err(Base64Error::InvalidLength(2)));
    assert_eq!(
        decode("A==="),
        Err(Base64Error::InvalidPadding { index: 1 })
    );
    assert_eq!(
        decode("===="),
        Err(Base64Error::InvalidPadding { index: 0 })
    );
    assert_eq!(
        decode("=AAA"),
        Err(Base64Error::InvalidPadding { index: 0 })
    );
    // Нит N1/N6 ревью W9r (red-first): 1 символ данных + 1 паддинг недопустимы
    // (first_pad % 4 == 1) — ошибка обязана быть InvalidPadding, а не InvalidLength.
    assert_eq!(decode("A="), Err(Base64Error::InvalidPadding { index: 1 }));
    assert_eq!(
        decode("AAAAA="),
        Err(Base64Error::InvalidPadding { index: 5 })
    );
    assert_eq!(
        decode("AA=A"),
        Err(Base64Error::InvalidPadding { index: 2 })
    );
    assert_eq!(
        decode("AA!A"),
        Err(Base64Error::InvalidChar {
            byte: b'!',
            index: 2
        })
    );
    assert_eq!(
        decode("Zm9v\n"),
        Err(Base64Error::InvalidChar {
            byte: b'\n',
            index: 4
        })
    );
    assert_eq!(decode(""), Ok(Vec::new()));
}

/// B4: длины и `=`-паддинг — 0/1/2/3 байта данных, 256-байтный вектор и длина 344.
#[test]
fn lengths_and_padding() {
    assert_eq!(encode(&[]), "");
    assert_eq!(encode(b"f"), "Zg==");
    assert_eq!(encode(b"fo"), "Zm8=");
    assert_eq!(encode(b"foo"), "Zm9v");
    assert_eq!(decode(""), Ok(Vec::new()));

    let rows = data_rows(VECTORS_TSV);
    let row = rows
        .iter()
        .find(|row| row[0] == "bytes-00-ff")
        .expect("вектор bytes-00-ff (256 байт) в TSV");
    let bytes = hex_to_bytes(&row[1]);
    assert_eq!(bytes.len(), 256, "вход вектора bytes-00-ff — 256 байт");
    let encoded = encode(&bytes);
    assert_eq!(encoded, row[2], "encode 256 байт = python3 b64encode");
    assert_eq!(encoded.len(), 344, "256 байт → 344 символа base64");
    assert!(encoded.len().is_multiple_of(4), "344 кратно 4");
    assert_eq!(
        decode(&encoded),
        Ok(bytes),
        "decode 344 символов → 256 байт"
    );

    for (input, expected_pad) in [
        (b"f".as_slice(), 2_usize),
        (b"fo".as_slice(), 1),
        (b"foo".as_slice(), 0),
    ] {
        let encoded = encode(input);
        assert!(encoded.len().is_multiple_of(4), "{encoded}: длина кратна 4");
        let pad = encoded.chars().rev().take_while(|&ch| ch == '=').count();
        assert_eq!(pad, expected_pad, "{encoded}: число хвостовых '='");
    }
}

/// B5: ненулевые хвостовые биты последней группы отбрасываются (декодер не строгий),
/// а `encode` от результата даёт каноническую форму.
#[test]
fn noncanonical_trailing_bits() {
    let decoded = decode("Zh==").expect("неканонические хвостовые биты принимаются");
    assert_eq!(decoded.as_slice(), b"f".as_slice(), "Zh== → b\"f\"");
    assert_eq!(encode(&decoded), "Zg==", "повторный encode канонизирует");

    let decoded = decode("Zm9=").expect("вторая неканоническая форма принимается");
    assert_eq!(decoded.as_slice(), b"fo".as_slice(), "Zm9= → b\"fo\"");
    assert_eq!(encode(&decoded), "Zm8=", "повторный encode канонизирует");
}
