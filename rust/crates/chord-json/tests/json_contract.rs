//! Контракт foundation-парсера JSON (RW-22).
//!
//! Оракулы: реальные `ru-catalog.json`/`en-catalog.json` и `voice/rtc*-frames.jsonl`
//! из `/home/sanya/re-deep` (если лаборатория доступна), плюс независимые грамматические
//! кейсы RFC 8259. Тесты не подстраиваются под реализацию.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use chord_json::{JsonErrorKind, JsonValue, MAX_DEPTH, parse};

fn lab_root() -> Option<PathBuf> {
    if let Some(root) = std::env::var_os("RE_LAB_ROOT") {
        let root = PathBuf::from(root);
        if root.is_dir() {
            return Some(root);
        }
    }
    let fallback = Path::new("/home/sanya/re-deep");
    fallback.is_dir().then(|| fallback.to_path_buf())
}

fn lab_file(rel: &str) -> Option<PathBuf> {
    let path = lab_root()?.join(rel);
    path.is_file().then_some(path)
}

#[test]
fn parses_scalars_arrays_and_objects() {
    assert_eq!(parse("null").unwrap(), JsonValue::Null);
    assert_eq!(parse("true").unwrap(), JsonValue::Bool(true));
    assert_eq!(parse("false").unwrap(), JsonValue::Bool(false));
    assert_eq!(
        parse("\"текст\"").unwrap(),
        JsonValue::String("текст".into())
    );

    let value = parse(r#"{"a":[1,-2,3.5,true,false,null,"x"],"b":{"c":"d"}}"#).unwrap();
    let object = value.as_object().expect("object");
    assert_eq!(object.len(), 2);
    let array = object
        .get("a")
        .and_then(JsonValue::as_array)
        .expect("array");
    assert_eq!(array.len(), 7);
    assert_eq!(array[0].as_i64(), Some(1));
    assert_eq!(array[1].as_i64(), Some(-2));
    assert_eq!(array[2].as_f64(), Some(3.5));
    assert_eq!(object["b"].get("c").and_then(JsonValue::as_str), Some("d"));
}

#[test]
fn numbers_preserve_raw_and_parse_integers() {
    let cases = [
        ("0", Some(0), Some(0)),
        ("-0", Some(0), None),
        (
            "9007199254740993",
            Some(9007199254740993),
            Some(9007199254740993),
        ),
        ("-42", Some(-42), None),
        ("1.0", None, None),
        ("1e3", None, None),
        ("18446744073709551615", None, Some(18446744073709551615)),
    ];
    for (raw, i64_expected, u64_expected) in cases {
        let value = parse(raw).unwrap();
        let number = value.as_number().expect("number");
        assert_eq!(number.as_i64(), i64_expected, "i64 raw={raw}");
        assert_eq!(number.as_u64(), u64_expected, "u64 raw={raw}");
        assert!(!number.as_str().is_empty());
    }
    assert_eq!(parse("1e3").unwrap().as_f64(), Some(1000.0));
}

#[test]
fn rejects_malformed_and_dangerous_input() {
    let cases: &[(&str, JsonErrorKind)] = &[
        ("", JsonErrorKind::UnexpectedEof),
        (" ", JsonErrorKind::UnexpectedEof),
        ("{", JsonErrorKind::UnexpectedEof),
        ("[1,]", JsonErrorKind::UnexpectedByte(b']')),
        ("{\"a\":1,}", JsonErrorKind::UnexpectedByte(b'}')),
        ("{\"a\" 1}", JsonErrorKind::UnexpectedByte(b'1')),
        ("01", JsonErrorKind::InvalidNumber),
        ("+1", JsonErrorKind::UnexpectedByte(b'+')),
        (".5", JsonErrorKind::UnexpectedByte(b'.')),
        ("1.", JsonErrorKind::InvalidNumber),
        ("1e", JsonErrorKind::InvalidNumber),
        ("NaN", JsonErrorKind::UnexpectedByte(b'N')),
        ("\"\\x\"", JsonErrorKind::InvalidEscape),
        ("\"\\u12\"", JsonErrorKind::InvalidUnicode),
        ("\"\\ud800\"", JsonErrorKind::InvalidUnicode),
        ("\"a\nb\"", JsonErrorKind::ControlCharacter),
        ("{\"a\":1,\"a\":2}", JsonErrorKind::DuplicateKey),
        ("{}x", JsonErrorKind::TrailingData),
        ("null null", JsonErrorKind::TrailingData),
    ];
    for (input, expected) in cases {
        let error = parse(input).expect_err(input);
        assert_eq!(
            std::mem::discriminant(&error.kind),
            std::mem::discriminant(expected),
            "input={input:?} got={error:?}"
        );
        assert!(error.offset <= input.len(), "offset for {input:?}");
    }
}

#[test]
fn supports_unicode_escapes_and_rejects_lone_low_surrogate() {
    assert_eq!(
        parse(r#""\uD83D\uDE00""#).unwrap(),
        JsonValue::String("😀".into())
    );
    assert_eq!(
        parse(r#""\u043F\u0440\u0438\u0432\u0435\u0442""#).unwrap(),
        JsonValue::String("привет".into())
    );
    let escaped = "\"\\/\\b\\f\\n\\r\\t\\\\\\\"\\\"\"";
    assert_eq!(
        parse(escaped).unwrap(),
        JsonValue::String("/\u{8}\u{c}\n\r\t\\\"\"".into())
    );
    assert!(parse(r#""\uDC00""#).is_err());
    assert!(parse(r#""\uD800\uD800""#).is_err());
}

#[test]
fn depth_limit_is_enforced_without_panic() {
    let mut input = String::new();
    for _ in 0..MAX_DEPTH {
        input.push('[');
    }
    input.push('0');
    for _ in 0..MAX_DEPTH {
        input.push(']');
    }
    // Ровно MAX_DEPTH вложенных массивов допустимо; следующий уровень — уже нет.
    assert!(parse(&input).is_ok());

    let mut too_deep = String::from("[");
    too_deep.push_str(&input);
    too_deep.push(']');
    assert!(matches!(
        parse(&too_deep).unwrap_err().kind,
        JsonErrorKind::DepthLimit
    ));
}

#[test]
fn truncated_inputs_never_panic() {
    let samples = [
        r#"{"op":8,"d":{"v":8,"heartbeat_interval":13750}}"#,
        r##"[1,"count",{"one":"# участник","few":"# участника","other":"# участников"}]"##,
        r#"{"a":["\uD83D\uDE00",-1.5e-3,true,null]}"#,
    ];
    for sample in samples {
        for end in 0..=sample.len() {
            if !sample.is_char_boundary(end) {
                continue;
            }
            let prefix = &sample[..end];
            let result = catch_unwind(AssertUnwindSafe(|| parse(prefix)));
            assert!(result.is_ok(), "panic on prefix {prefix:?}");
        }
    }
}

#[test]
fn parses_real_catalogs_when_lab_is_available() {
    let Some(path) = lab_file("ru-catalog.json") else {
        eprintln!("skip: нет /home/sanya/re-deep/ru-catalog.json");
        return;
    };
    let text = std::fs::read_to_string(path).unwrap();
    let root = parse(&text).expect("ru catalog JSON");
    let object = root.as_object().expect("catalog object");
    assert_eq!(object.len(), 23_836);
    assert_eq!(
        object["hPBScv"].as_array().unwrap()[0].as_str(),
        Some("Закрыть всплывающее окно")
    );

    let en_path = lab_file("en-catalog.json").expect("en catalog рядом с ru");
    let en = parse(&std::fs::read_to_string(en_path).unwrap()).expect("en catalog JSON");
    assert_eq!(en.as_object().unwrap().len(), 23_903);
}

#[test]
fn parses_real_voice_frames_when_lab_is_available() {
    let Some(path) = lab_file("voice/rtc1-frames.jsonl") else {
        eprintln!("skip: нет /home/sanya/re-deep/voice/rtc1-frames.jsonl");
        return;
    };
    let text = std::fs::read_to_string(path).unwrap();
    let mut seen = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let wrapper = parse(line).expect("voice wrapper JSON");
        let frame = parse(
            wrapper
                .get("payload")
                .and_then(JsonValue::as_str)
                .expect("payload"),
        )
        .expect("voice frame JSON");
        let op = frame.get("op").and_then(JsonValue::as_u64).expect("op") as u8;
        seen.push(op);
        if op == 2 {
            let payload = frame.get("d").expect("op2 d");
            assert_eq!(payload.get("ssrc").and_then(JsonValue::as_u64), Some(7142));
            assert_eq!(
                payload.get("port").and_then(JsonValue::as_u64),
                Some(19_334)
            );
            assert_eq!(
                payload.get("modes").and_then(JsonValue::as_array).unwrap()[0].as_str(),
                Some("aead_aes256_gcm_rtpsize")
            );
        }
        if op == 4 {
            let payload = frame.get("d").expect("op4 d");
            assert_eq!(
                payload.get("mode").and_then(JsonValue::as_str),
                Some("aead_aes256_gcm_rtpsize")
            );
            assert_eq!(
                payload.get("media_session_id").and_then(JsonValue::as_str),
                Some("REDACTED")
            );
        }
    }
    for expected in [0, 1, 2, 3, 4, 6, 8, 15, 16] {
        assert!(seen.contains(&expected), "нет op {expected} в кадрах");
    }
}

#[test]
fn writer_round_trips_unicode_escapes_and_numbers() {
    let value =
        parse(r#"{"text":"строка\n\"кавычки\"\u0001","n":-12.5e-2,"a":[true,null,"😀"]}"#).unwrap();
    let text = chord_json::to_json_string(&value);
    assert!(text.contains("\\n"));
    assert!(text.contains("\\u0001"));
    assert!(text.contains("строка"));
    assert_eq!(parse(&text).unwrap(), value);
    assert_eq!(
        chord_json::to_json_string(&JsonValue::String("a/b\"c".into())),
        r#""a/b\"c""#
    );
}

#[test]
fn writer_can_round_trip_a_real_voice_frame() {
    let Some(path) = lab_file("voice/rtc1-frames.jsonl") else {
        eprintln!("skip: нет voice fixture");
        return;
    };
    let line = std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .nth(1)
        .expect("second frame")
        .to_owned();
    let wrapper = parse(&line).unwrap();
    let payload = wrapper.get("payload").and_then(JsonValue::as_str).unwrap();
    let frame = parse(payload).unwrap();
    let rendered = chord_json::to_json_string(&frame);
    assert_eq!(parse(&rendered).unwrap(), frame);
}

#[test]
fn error_display_has_offset_but_not_payload() {
    let input = r#"{"secret":"super-secret-token","a":1,}"#;
    let error = parse(input).unwrap_err();
    let text = error.to_string();
    assert!(text.contains("offset"), "{text}");
    assert!(!text.contains("super-secret-token"), "{text}");
}
