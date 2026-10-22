//! P1-01: полный контракт ETF-энкодера (checked-путь, границы, глубина) и усиленный fuzz.
//!
//! Оракул: `docs/re/gateway-protocol.md` §2, канонические байты RW-01 и `MAX_DEPTH=64`
//! (тот же предел у декодера). Тесты не подгоняют реализацию: для обычных значений
//! `encode_checked` обязан давать ровно те же байты, что принятый `encode`.

use discord_codec::etf::{self, EtfEncodeError, EtfError, EtfValue, MAX_DEPTH};

fn atom(s: &str) -> EtfValue {
    EtfValue::Atom(s.to_string())
}

fn int(v: i64) -> EtfValue {
    EtfValue::Int(v)
}

fn text(s: &str) -> EtfValue {
    EtfValue::Str(s.to_string())
}

fn nested_lists(depth: usize) -> EtfValue {
    let mut value = EtfValue::Nil;
    for _ in 0..depth {
        value = EtfValue::List(vec![value]);
    }
    value
}

#[test]
fn checked_encoder_is_byte_exact_with_infallible_encoder() {
    let cases = vec![
        EtfValue::Nil,
        atom("op"),
        atom("Проверка"),
        int(0),
        int(-1),
        int(i32::MIN.into()),
        int(i64::MIN),
        int(i64::MAX),
        EtfValue::Float(-0.0),
        text(""),
        text("привет"),
        EtfValue::Tuple(vec![int(1), atom("a")]),
        EtfValue::List(vec![int(1), EtfValue::Nil]),
        EtfValue::Map(vec![(atom("a"), int(1)), (text("b"), text("x"))]),
        nested_lists(MAX_DEPTH),
    ];
    for value in cases {
        assert_eq!(
            etf::encode_checked(&value),
            Ok(etf::encode(&value)),
            "framed checked != infallible для {value:?}"
        );
        assert_eq!(
            etf::encode_term_checked(&value),
            Ok(etf::encode_term(&value)),
            "term checked != infallible для {value:?}"
        );
    }
}

#[test]
fn depth_limit_is_the_same_for_encoder_and_decoder() {
    let at_limit = nested_lists(MAX_DEPTH);
    let encoded = etf::encode_checked(&at_limit).expect("глубина 64 допустима");
    let (decoded, consumed) = etf::decode(&encoded).expect("декодер принимает глубину 64");
    assert_eq!(consumed, encoded.len());
    assert_eq!(decoded, at_limit);

    let too_deep = nested_lists(MAX_DEPTH + 1);
    assert_eq!(
        etf::encode_checked(&too_deep),
        Err(EtfEncodeError::DepthLimit { limit: MAX_DEPTH })
    );

    // Сырой поток из 65 вложенных LIST_EXT: декодер обязан остановиться на 65-м
    // контейнере, не пытаясь рекурсивно читать хвост.
    let mut raw = Vec::new();
    for _ in 0..=MAX_DEPTH {
        raw.push(0x6c); // LIST_EXT
        raw.extend_from_slice(&1u32.to_be_bytes());
    }
    raw.push(discord_codec::TAG_NIL_EXT);
    assert!(matches!(
        etf::decode_term(&raw),
        Err(EtfError::DepthLimit { .. })
    ));
}

#[test]
fn checked_encoder_rejects_atom_that_does_not_fit_u16_without_truncation() {
    // 65 536 ASCII-байт: `u16` длины не хватает. Обычный `encode` в этом случае паникует
    // по документированному контракту, checked-путь обязан вернуть ошибку.
    let value = atom(&"x".repeat(usize::from(u16::MAX) + 1));
    assert_eq!(
        etf::encode_checked(&value),
        Err(EtfEncodeError::AtomTooLong {
            len: usize::from(u16::MAX) + 1
        })
    );
    assert_eq!(
        etf::encode_term_checked(&value),
        Err(EtfEncodeError::AtomTooLong {
            len: usize::from(u16::MAX) + 1
        })
    );
}

#[test]
fn big_int_borders_roundtrip_through_checked_path() {
    for value in [
        int(i64::MIN),
        int(i64::MIN + 1),
        int(-(1 << 32)),
        int(-1),
        int(1),
        int(1 << 32),
        int(i64::MAX - 1),
        int(i64::MAX),
    ] {
        let encoded = etf::encode_term_checked(&value).expect("big-int в границах i64");
        let (decoded, consumed) = etf::decode_term(&encoded).expect("big-int декодируется");
        assert_eq!(consumed, encoded.len());
        assert_eq!(decoded, value);
    }
}

#[test]
fn fuzz_50k_random_and_all_single_bytes_never_panic() {
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = || {
        state ^= state << 7;
        state ^= state >> 9;
        state ^= state << 8;
        state
    };
    for _ in 0..50_000 {
        let len = (next() as usize) % 129;
        let mut bytes = vec![0u8; len];
        for byte in &mut bytes {
            *byte = next() as u8;
        }
        if let Ok((_, consumed)) = etf::decode(&bytes) {
            assert!(consumed <= bytes.len(), "consumed вышел за буфер");
        }
    }
    for byte in 0u8..=u8::MAX {
        let _ = etf::decode(&[byte]);
        let _ = etf::decode_term(&[byte]);
    }
}

#[test]
fn prefix_truncation_is_bounded_and_never_reports_overread() {
    let value = EtfValue::Map(vec![
        (atom("op"), int(10)),
        (
            atom("d"),
            EtfValue::List(vec![text("alpha"), int(1_792_391_313_326)]),
        ),
    ]);
    let encoded = etf::encode(&value);
    for end in 0..encoded.len() {
        if let Ok((_, consumed)) = etf::decode(&encoded[..end]) {
            assert!(
                consumed <= end,
                "потреблено {consumed} Б из усечённых {end}"
            );
        }
    }
    let (decoded, consumed) = etf::decode(&encoded).expect("полный буфер");
    assert_eq!(consumed, encoded.len());
    assert_eq!(decoded, value);
}
