//! Свойства кодека ETF (волна RW-1, P0-02, крейт `discord-codec`): round-trip, канонический
//! энкод, ошибки, лимиты глубины и fuzz-lite.
//!
//! Оракулы: фиксированные канонические байты сверены с реальными термами захвата
//! (`capture-run2.jsonl`, сокет `95974.1389`; `gw-proto/msgs-run2-95974_1389.jsonl`) и со спекой
//! `docs/re/gateway-protocol.md` §2. Правила канона — контракт ТЗ RW-01.
//!
//! Проверяемое свойство: `decode_term(encode_term(v)) == v` **побайтово** обратимо
//! (`encode_term(decode_term(b).0) == b`) для канонических байт.

use chord_codec::etf::{self, EtfError, EtfValue, MAX_DEPTH};

/// Строка hex → байты (пробелы игнорируются).
fn unhex(s: &str) -> Vec<u8> {
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    assert_eq!(compact.len() % 2, 0, "hex нечётной длины: {s}");
    (0..compact.len() / 2)
        .map(|i| u8::from_str_radix(&compact[2 * i..2 * i + 2], 16).expect("hex"))
        .collect()
}

/// Байты → строка hex.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn atom(s: &str) -> EtfValue {
    EtfValue::Atom(s.to_string())
}

fn int(v: i64) -> EtfValue {
    EtfValue::Int(v)
}

fn text(s: &str) -> EtfValue {
    EtfValue::Str(s.to_string())
}

/// Рекурсивная сверка битов `f64` (чтобы `-0.0` не «уравнивался» с `0.0` по `PartialEq`).
fn assert_float_bits_equal(expected: &EtfValue, actual: &EtfValue, path: &str) {
    match (expected, actual) {
        (EtfValue::Float(x), EtfValue::Float(y)) => {
            assert_eq!(x.to_bits(), y.to_bits(), "{path}: биты f64");
        }
        (EtfValue::Tuple(x), EtfValue::Tuple(y)) | (EtfValue::List(x), EtfValue::List(y)) => {
            assert_eq!(x.len(), y.len(), "{path}: длина");
            for (i, (xe, xa)) in x.iter().zip(y.iter()).enumerate() {
                assert_float_bits_equal(xe, xa, &format!("{path}[{i}]"));
            }
        }
        (EtfValue::Map(x), EtfValue::Map(y)) => {
            assert_eq!(x.len(), y.len(), "{path}: число пар");
            for (i, ((ke, ve), (ka, va))) in x.iter().zip(y.iter()).enumerate() {
                assert_float_bits_equal(ke, ka, &format!("{path}[{i}].key"));
                assert_float_bits_equal(ve, va, &format!("{path}[{i}].value"));
            }
        }
        _ => {}
    }
}

/// Round-trip значения: `encode_term → decode_term` и `encode → decode`.
fn roundtrip(value: &EtfValue) {
    let payload = etf::encode_term(value);
    let (decoded, consumed) = etf::decode_term(&payload).expect("decode_term");
    assert_eq!(consumed, payload.len(), "consumed != длины терма");
    assert_eq!(&decoded, value, "значение исказилось");
    assert_float_bits_equal(value, &decoded, "$");
    assert_eq!(
        etf::encode_term(&decoded),
        payload,
        "повторный энкод не идемпотентен"
    );

    let framed = etf::encode(value);
    assert_eq!(
        framed.first().copied(),
        Some(0x83),
        "encode обязан начинаться с 0x83"
    );
    assert_eq!(&framed[1..], payload.as_slice(), "0x83 + терм");
    let (decoded_framed, framed_consumed) = etf::decode(&framed).expect("decode");
    assert_eq!(framed_consumed, framed.len());
    assert_eq!(decoded_framed, *value);
}

/// Кейсы round-trip: все варианты `EtfValue`, границы целых, спец-значения float.
fn roundtrip_cases() -> Vec<EtfValue> {
    vec![
        EtfValue::Nil,
        atom("op"),
        atom("nil"),
        atom("Проверка"),
        int(0),
        int(255),
        int(256),
        int(-1),
        int(i32::MIN as i64),
        int(i32::MAX as i64),
        int(2_147_483_648),
        int(i64::MIN),
        int(i64::MAX),
        EtfValue::Float(1.5),
        EtfValue::Float(-0.0),
        EtfValue::Float(41_250.0),
        text(""),
        text("привет"),
        text(r#"["gateway-prd-arm-us-east1-d-04m5",{"micros":0.0}]"#),
        EtfValue::Tuple(vec![]),
        EtfValue::Tuple(vec![int(1), atom("a"), text("b")]),
        EtfValue::List(vec![]),
        EtfValue::List(vec![int(1), EtfValue::Nil]),
        EtfValue::Map(vec![]),
        EtfValue::Map(vec![(atom("a"), int(1)), (text("b"), text("x"))]),
        // Вложенность трёх уровней: map → list → map.
        EtfValue::Map(vec![(
            atom("l1"),
            EtfValue::List(vec![EtfValue::Map(vec![(atom("l3"), int(3))])]),
        )]),
    ]
}

#[test]
fn roundtrip_covers_every_variant_and_border_values() {
    for value in roundtrip_cases() {
        roundtrip(&value);
    }
}

#[test]
fn canonical_encoder_matches_fixed_bytes() {
    // Контракт канона ТЗ RW-01; big-int — реальные байты захвата (SESSIONS_REPLACE #5,
    // `processed_at_timestamp = 1792391313326`).
    let cases: Vec<(EtfValue, &str)> = vec![
        (int(10), "610a"),
        (int(41250), "620000a122"),
        (int(255), "61ff"),
        (int(-1), "62ffffffff"),
        (int(2_147_483_648), "6e040000000080"),
        (int(1_792_391_313_326), "6e0600aef7d852a101"),
        (int(i64::MAX), "6e0800ffffffffffffff7f"),
        (int(i64::MIN), "6e08010000000000000080"),
        (EtfValue::Float(1.5), "463ff8000000000000"),
        (atom("op"), "6400026f70"),
        (text("abc"), "6d00000003616263"),
        (EtfValue::List(vec![int(1), int(2)]), "6c00000002610161026a"),
        (EtfValue::Tuple(vec![int(1)]), "68016101"),
        (
            EtfValue::Map(vec![(atom("a"), int(1))]),
            "7400000001640001616101",
        ),
        (EtfValue::Nil, "6a"),
        (text(""), "6d00000000"),
        (EtfValue::List(vec![]), "6c000000006a"),
        (EtfValue::Tuple(vec![]), "6800"),
        (EtfValue::Map(vec![]), "7400000000"),
    ];
    for (value, expected) in cases {
        assert_eq!(hex(&etf::encode_term(&value)), expected, "канон {value:?}");
    }
    assert_eq!(hex(&etf::encode(&int(10))), "83610a", "0x83 перед термом");
    assert_eq!(
        hex(&etf::encode_term(&int(1_014_537_851_418_980_372))),
        "6e0800143004f5cc5c140e",
        "guild_id из PRESENCE_UPDATE #18"
    );
}

#[test]
fn canonical_encoder_uses_expected_tags_for_atoms_and_big_containers() {
    // Атом: чистый ASCII → 0x64; unicode ≤255 Б → 0x77; >255 Б → 0x76.
    let unicode_atom = "Проверка";
    assert_eq!(
        hex(&etf::encode_term(&atom(unicode_atom))),
        format!(
            "77{:02x}{}",
            unicode_atom.len(),
            hex(unicode_atom.as_bytes())
        ),
        "unicode-атом ≤255 Б идёт тегом 0x77"
    );
    assert_eq!(
        hex(&etf::encode_term(&atom("op"))),
        "6400026f70",
        "ASCII-атом идёт тегом 0x64"
    );
    let long_atom = "Проверка".repeat(40); // 640 Б (> 255)
    let encoded = hex(&etf::encode_term(&atom(&long_atom)));
    assert!(
        encoded.starts_with(&format!("76{:04x}", long_atom.len())),
        "длинный атом: {encoded}"
    );
    // Кортеж: ≤255 элементов → 0x68 (u8 count), 256 → 0x69 (u32 count).
    let tuple255 = EtfValue::Tuple(vec![EtfValue::Nil; 255]);
    let tuple256 = EtfValue::Tuple(vec![EtfValue::Nil; 256]);
    assert!(hex(&etf::encode_term(&tuple255)).starts_with("68ff"));
    assert!(hex(&etf::encode_term(&tuple256)).starts_with("6900000100"));
    // Список длиной 256 — u32 count.
    let list256 = EtfValue::List(vec![EtfValue::Nil; 256]);
    assert!(hex(&etf::encode_term(&list256)).starts_with("6c00000100"));
    for value in [tuple255, tuple256, list256] {
        roundtrip(&value);
    }
}

/// Ожидание ошибки `decode_term` без привязки к конкретным `at`/`need`.
fn expect_err(name: &str, bytes: &[u8]) -> EtfError {
    match etf::decode_term(bytes) {
        Err(e) => e,
        Ok((value, consumed)) => {
            panic!("{name}: ожидалась ошибка, получено {value:?} ({consumed} Б)")
        }
    }
}

#[test]
fn decoder_reports_specific_errors() {
    // Пустой буфер и обрыв фиксированного поля.
    assert!(
        matches!(expect_err("пусто", &[]), EtfError::UnexpectedEof { .. }),
        "пустой буфер — UnexpectedEof"
    );
    assert!(matches!(
        expect_err("0x62 без 4 байт", &[0x62, 0x00]),
        EtfError::UnexpectedEof { .. }
    ));
    assert!(matches!(
        expect_err("0x46 без 8 байт", &[0x46, 0x00, 0x00]),
        EtfError::UnexpectedEof { .. }
    ));
    // Неизвестный тег (в т.ч. 0x83 — маркер кадра, в терме недопустим).
    assert_eq!(
        expect_err("0x7f", &[0x7f]),
        EtfError::UnknownTag { tag: 0x7f, at: 0 }
    );
    assert_eq!(
        expect_err("0x83", &[0x83]),
        EtfError::UnknownTag { tag: 0x83, at: 0 }
    );
    match expect_err("тег внутри map", &unhex("7400000001 7f 6101")) {
        EtfError::UnknownTag { tag, .. } => assert_eq!(tag, 0x7f),
        other => panic!("ожидался UnknownTag, получено {other:?}"),
    }
    // Строгий UTF-8 у 0x6d.
    match expect_err("0x6d ff", &unhex("6d00000001ff")) {
        EtfError::InvalidUtf8 { .. } => {}
        other => panic!("ожидался InvalidUtf8, получено {other:?}"),
    }
    match expect_err("0x76 невалидный utf8", &unhex("760002 ff80")) {
        EtfError::InvalidUtf8 { .. } => {}
        other => panic!("ожидался InvalidUtf8, получено {other:?}"),
    }
    // Хвост LIST_EXT — только NIL_EXT.
    match expect_err("хвост-Int", &unhex("6c0000000161016101")) {
        EtfError::ListTailNotNil { tail_tag, .. } => assert_eq!(tail_tag, 0x61),
        other => panic!("ожидался ListTailNotNil, получено {other:?}"),
    }
    match expect_err("хвост-0x83", &unhex("6c00000000 83")) {
        EtfError::ListTailNotNil { tail_tag, .. } => assert_eq!(tail_tag, 0x83),
        other => panic!("ожидался ListTailNotNil, получено {other:?}"),
    }
    match expect_err("хвост оборван", &unhex("6c000000016101")) {
        EtfError::UnexpectedEof { .. } => {}
        other => panic!("ожидался UnexpectedEof, получено {other:?}"),
    }
    // FLOAT_EXT: 31 байт мусора не парсится как f64.
    let mut spaces = vec![0x63];
    spaces.extend([0x20u8; 31]);
    match expect_err("float из пробелов", &spaces) {
        EtfError::MalformedFloat { .. } => {}
        other => panic!("ожидался MalformedFloat, получено {other:?}"),
    }
    assert!(
        matches!(
            expect_err("0x63 без 31 Б", &[0x63, 0x31]),
            EtfError::UnexpectedEof { .. }
        ),
        "0x63 короткий — UnexpectedEof"
    );
    // Заявленная длина 0xffffffff: ошибка ДО какой-либо аллокации (не 4 ГиБ).
    match expect_err("0x6d ffffffff", &unhex("6dffffffff")) {
        EtfError::UnexpectedEof { .. } => {}
        other => panic!("ожидался UnexpectedEof, получено {other:?}"),
    }
    for src in ["6bffff03 616263", "69000000", "6cffffffff 6a", "74ffffffff"] {
        assert!(
            matches!(expect_err(src, &unhex(src)), EtfError::UnexpectedEof { .. }),
            "{src}: ожидался UnexpectedEof"
        );
    }
    // decode с ведущим 0x83, но без терма.
    assert!(matches!(
        etf::decode(&[0x83]),
        Err(EtfError::UnexpectedEof { .. })
    ));
}

#[test]
fn big_int_rejects_overflow_and_keeps_signed_borders() {
    // n > 16 → BigIntOverflow (0x6e: u8 n, 0x6f: u32 n).
    let mut small17 = vec![0x6e, 17, 0];
    small17.extend([0u8; 17]);
    assert!(matches!(
        expect_err("0x6e n=17", &small17),
        EtfError::BigIntOverflow { .. }
    ));
    let mut large17 = vec![0x6f, 0, 0, 0, 17, 0];
    large17.extend([0u8; 17]);
    assert!(matches!(
        expect_err("0x6f n=17", &large17),
        EtfError::BigIntOverflow { .. }
    ));
    // sign=0 и magnitude > i64::MAX.
    assert!(matches!(
        expect_err("u64::MAX положительный", &unhex("6e0800ffffffffffffffff")),
        EtfError::BigIntOverflow { .. }
    ));
    // sign=1 и magnitude > 2^63.
    assert!(matches!(
        expect_err("2^63+1 отрицательный", &unhex("6e08 01 0100000000000080")),
        EtfError::BigIntOverflow { .. }
    ));
    // Границы i64 разбираются: i64::MAX (sign=0) и i64::MIN (sign=1, magnitude 2^63).
    assert_eq!(
        etf::decode_term(&unhex("6e0800ffffffffffffff7f")),
        Ok((int(i64::MAX), 11))
    );
    assert_eq!(
        etf::decode_term(&unhex("6e08 01 0000000000000080")),
        Ok((int(i64::MIN), 11))
    );
    assert_eq!(
        etf::decode_term(&unhex("6f00000008010000000000000080")),
        Ok((int(i64::MIN), 14))
    );
    // Ноль байт цифр: 6e 00 sign → 0.
    assert_eq!(etf::decode_term(&unhex("6e0000")), Ok((int(0), 3)));
}

/// Ровно `depth` вложенных LIST (внутри — Int(0)), по контракту глубины.
fn nested_lists(depth: usize) -> Vec<u8> {
    let mut buf = Vec::new();
    for _ in 0..depth {
        buf.extend_from_slice(&[0x6c, 0x00, 0x00, 0x00, 0x01]);
    }
    buf.extend_from_slice(&[0x61, 0x00]);
    buf.extend(std::iter::repeat_n(0x6a_u8, depth));
    buf
}

#[test]
fn nesting_depth_limit_is_enforced() {
    let ok = nested_lists(MAX_DEPTH);
    let (value, consumed) = etf::decode_term(&ok).expect("ровно MAX_DEPTH — допустимо");
    assert_eq!(consumed, ok.len());
    let mut cursor = &value;
    let mut depth = 0;
    while let Some(items) = cursor.as_list() {
        depth += 1;
        cursor = &items[0];
    }
    assert_eq!(depth, MAX_DEPTH, "вложенность разобралась не полностью");
    assert_eq!(cursor, &int(0));
    roundtrip(&value);

    match etf::decode_term(&nested_lists(MAX_DEPTH + 1)) {
        Err(EtfError::DepthLimit { .. }) => {}
        other => panic!("MAX_DEPTH+1: ожидался DepthLimit, получено {other:?}"),
    }
    // Глубина режется и внутри кортежей/map: 65 кортежей подряд — предел.
    let mut deep_tuple = Vec::new();
    for _ in 0..=MAX_DEPTH {
        deep_tuple.extend_from_slice(&[0x68, 0x01]);
    }
    deep_tuple.extend_from_slice(&[0x61, 0x00]);
    assert!(matches!(
        etf::decode_term(&deep_tuple),
        Err(EtfError::DepthLimit { .. })
    ));
}

#[test]
fn truncated_hello_prefixes_are_errors_not_panics() {
    let hello = unhex(HELLO_HEX);
    for len in 0..138 {
        match etf::decode(&hello[..len]) {
            Err(_) => {}
            Ok(other) => panic!("усечение {len} Б разобрано как целый терм: {other:?}"),
        }
    }
    assert!(etf::decode(&hello).is_ok(), "полный HELLO — ок");
}

#[test]
fn fuzz_lite_never_panics_on_garbage() {
    // Детерминированный LCG (без внешних крейтов): x = x*6364136223846793005 + 1442695040888963407.
    let mut state: u64 = 0x0123_4567_89ab_cdef;
    let mut next = || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        state
    };
    for _ in 0..20_000 {
        let len = (next() >> 33) as usize % 65;
        let buf: Vec<u8> = (0..len).map(|_| (next() >> 24) as u8).collect();
        let _ = etf::decode_term(&buf);
        let _ = etf::decode(&buf);
    }
    // Исчерпывающе: все однобайтовые буферы.
    for byte in 0u8..=255 {
        let _ = etf::decode_term(&[byte]);
    }
    // Одиночные инверсии (xor 0xff) первых ≤64 байт пяти реальных векторов.
    for src in [
        HELLO_HEX,
        ACK_HEX,
        PRESENCE_HEX,
        MESSAGE_ACK_HEX,
        MEMBER_LIST_HEX,
    ] {
        let bytes = unhex(src);
        let head = bytes.len().min(64);
        for i in 0..head {
            let mut mutated = bytes[..head].to_vec();
            mutated[i] ^= 0xff;
            let _ = etf::decode_term(&mutated);
            let _ = etf::decode(&mutated);
        }
    }
}

#[test]
fn accessors_keep_types_strict() {
    let a = atom("nil");
    let s = text("nil");
    let n = EtfValue::Nil;
    assert_eq!(a.as_atom(), Some("nil"));
    assert_eq!(a.as_str(), None, "атом — не бинарник");
    assert_eq!(s.as_str(), Some("nil"));
    assert_eq!(s.as_atom(), None, "бинарник — не атом");
    assert_eq!(n.as_atom(), None);
    assert_eq!(n.as_str(), None);
    assert_eq!(n.as_int(), None);
    assert_eq!(n.as_float(), None);
    assert_eq!(int(3).as_int(), Some(3));
    assert_eq!(int(3).as_float(), None, "int — не float");
    assert_eq!(EtfValue::Float(3.5).as_float(), Some(3.5));
    assert_eq!(EtfValue::Float(3.5).as_int(), None, "float — не int");
    assert_eq!(EtfValue::Tuple(vec![int(1)]).as_list(), None);
    assert_eq!(EtfValue::List(vec![int(1)]).as_list(), Some(&[int(1)][..]));
    assert_eq!(EtfValue::List(vec![int(1)]).as_map(), None);
    assert_eq!(
        EtfValue::Map(vec![(atom("k"), int(9))])
            .as_map()
            .map(<[(EtfValue, EtfValue)]>::len),
        Some(1)
    );
    // get: только Map; ключ — атом ИЛИ бинарник.
    let m = EtfValue::Map(vec![(atom("a"), int(1)), (text("b"), int(2))]);
    assert_eq!(m.get("a"), Some(&int(1)));
    assert_eq!(m.get("b"), Some(&int(2)));
    assert_eq!(m.get("c"), None);
    assert_eq!(n.get("a"), None);
    assert_eq!(EtfValue::Map(vec![(int(1), text("x"))]).get("1"), None);
    // Бинарные ключи HELLO-подобной map с дубликатом атом/бинарник: находится первый.
    let mixed = EtfValue::Map(vec![(atom("k"), int(1)), (text("k"), int(2))]);
    assert_eq!(mixed.get("k"), Some(&int(1)));
}

/// Сырые термы провода (те же, что в `etf_vectors.rs`): HELLO #1 и ACK #6, сокет 95974.1389.
const HELLO_HEX: &str = "8374000000046400016474000000026400065f74726163656c000000016d000000325b22676174657761792d7072642d61726d2d75732d65617374312d642d30346d35222c7b226d6963726f73223a302e307d5d6a6400126865617274626561745f696e74657276616c620000a1226400026f70610a640001736400036e696c640001746400036e696c";
const ACK_HEX: &str =
    "837400000004640001646400036e696c6400026f70610b640001736400036e696c640001746400036e696c";
const PRESENCE_HEX: &str = "83740000000464000164740000000664000a616374697669746965736a64000d636c69656e745f73746174757374000000026400066d6f62696c656400066f6e6c696e65640002767264000469646c656400086775696c645f69646e0800143004f5cc5c140e64001670726f6365737365645f61745f74696d657374616d7061006400067374617475736400066f6e6c696e6564000475736572740000000164000269646e0800500040c7f3dabf096400026f70610064000173610d6400017464000f50524553454e43455f555044415445";
const MESSAGE_ACK_HEX: &str = "8374000000046400016474000000056d0000000a6368616e6e656c5f69646e08000b1082a7ed57f2116d00000005666c61677361006d0000000b6c6173745f76696577656462000010d66d0000000a6d6573736167655f69646e08007b0040228806ab156d0000000776657273696f6e620001f7b66400026f7061006400017361266400017464000b4d4553534147455f41434b";
const MEMBER_LIST_HEX: &str = "83740000000464000164740000000664000667726f7570736c000000027400000002640005636f756e74610364000269646e0800795084b7b7a1bf0e7400000002640005636f756e74610c64000269646400066f6e6c696e656a6400086775696c645f69646e0800143004f5cc5c140e64000269646d0000000a3136353936303636343764000c6d656d6265725f636f756e746200005ea464000c6f6e6c696e655f636f756e7462000002c46400036f70736c000000017400000002640005696e646578610b6400026f7064000644454c4554456a6400026f70610064000173613a640001746400184755494c445f4d454d4245525f4c4953545f555044415445";
