//! Внешний независимый тест RW-01: только публичное API `discord_codec::etf`.
//! Собственные значения ревьюера (не из тестов воркера).
use discord_codec::etf::{self, EtfValue};

fn hex(s: &str) -> Vec<u8> {
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    (0..compact.len() / 2)
        .map(|i| u8::from_str_radix(&compact[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

fn atom(s: &str) -> EtfValue {
    EtfValue::Atom(s.to_string())
}

/// Ошибочный разбор «знак после цифр» — независимая реализация (i128, свои данные).
fn wrong_order(buf: &[u8]) -> i128 {
    assert_eq!(buf[0], 0x6e, "SMALL_BIG_EXT");
    let n = buf[1] as usize;
    let mut mag: i128 = 0;
    for &b in buf[2..2 + n].iter().rev() {
        mag = (mag << 8) | i128::from(b);
    }
    if buf[2 + n] != 0 { -mag } else { mag }
}

#[test]
fn roundtrip_nested_own_values() {
    let v = EtfValue::Map(vec![
        (
            atom("l1"),
            EtfValue::List(vec![
                EtfValue::Int(42),
                EtfValue::Str("привет".into()),
                EtfValue::Tuple(vec![EtfValue::Float(1.5), EtfValue::Nil]),
                EtfValue::Map(vec![(EtfValue::Str("k".into()), EtfValue::Int(-7))]),
            ]),
        ),
        (atom("snowflake"), EtfValue::Int(1_176_785_373_989_445_703)),
    ]);
    let framed = etf::encode(&v);
    let (decoded, consumed) = etf::decode(&framed).expect("decode");
    assert_eq!(consumed, framed.len());
    assert_eq!(decoded, v);
    let payload = etf::encode_term(&v);
    let (d2, c2) = etf::decode_term(&payload).expect("decode_term");
    assert_eq!(c2, payload.len());
    assert_eq!(d2, v);
}

#[test]
fn own_small_big_ints_decode() {
    // свои значения: 4660 = 0x1234 (LE 34 12), -513 (sign=1, LE 01 02)
    assert_eq!(
        etf::decode_term(&hex("6e02003412")).unwrap(),
        (EtfValue::Int(4660), 5)
    );
    assert_eq!(
        etf::decode_term(&hex("6e02010102")).unwrap(),
        (EtfValue::Int(-513), 5)
    );
}

#[test]
fn big_int_sign_before_digits_with_negative_control() {
    // Вектор из §2.5 спеки, но проверка своим независимым разбором.
    let raw = hex("6e08004720841110c85410");
    let (value, consumed) = etf::decode_term(&raw).expect("decode");
    assert_eq!(consumed, 11, "0x83 не входит (decode_term)");
    assert_eq!(value, EtfValue::Int(1_176_785_373_989_445_703));
    assert_ne!(value, EtfValue::Int(-6_109_150_561_945_274_112));
    assert_eq!(wrong_order(&raw), -6_109_150_561_945_274_112);
    // и обратный энкод каноничен для > i32
    assert_eq!(etf::encode_term(&value), raw);
    // framed-вариант с 0x83
    let mut framed = vec![0x83];
    framed.extend_from_slice(&raw);
    let (v2, c2) = etf::decode(&framed).expect("decode");
    assert_eq!(c2, 12);
    assert_eq!(v2, value);
}

/// Наблюдение ревьюера (риск 6a): атом длиной > 65535 Б даёт усечённое u16-поле длины.
#[test]
fn reviewer_observation_atom_gt_65535_truncates_len() {    let big = "a".repeat(65_536); // 0x1_0000
    let enc = etf::encode_term(&EtfValue::Atom(big));
    assert_eq!(enc[0], 0x64, "ASCI-атом → ATOM_EXT");
    let len_field = u16::from_be_bytes([enc[1], enc[2]]);
    assert_eq!(len_field, 0, "наблюдение: 65536 as u16 == 0 (поле обрезано)");
    assert_eq!(enc.len(), 3 + 65_536, "байты записаны полностью");
    let (dec, consumed) = etf::decode_term(&enc).expect("decode");
    assert_eq!(consumed, 3, "декодер прочитал 0 байт атома");
    assert_eq!(dec, EtfValue::Atom(String::new()));
}

/// Сверка с независимым Node-энкодером: канонические теги unicode-атомов.
#[test]
fn unicode_atom_canonical_tags_match_reviewer() {
    let uni = "Проверка";
    let enc = etf::encode_term(&EtfValue::Atom(uni.into()));
    assert_eq!(enc, hex("7710d09fd180d0bed0b2d0b5d180d0bad0b0"));
    let long = uni.repeat(40);
    let enc2 = etf::encode_term(&EtfValue::Atom(long.clone()));
    assert_eq!(&enc2[..3], &hex("760280")[..], "длинный атом → 0x76 + u16 len");
    assert_eq!(enc2.len(), 3 + long.len());
    assert_eq!(&enc2[3..], long.as_bytes());
    assert_eq!(etf::decode_term(&enc).unwrap().0, EtfValue::Atom(uni.into()));
    assert_eq!(etf::decode_term(&enc2).unwrap().0, EtfValue::Atom(long));
}
