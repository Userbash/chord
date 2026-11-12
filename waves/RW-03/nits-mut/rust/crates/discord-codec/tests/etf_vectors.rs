//! Эталонные векторы ETF: реальные термы шлюза Discord (волна RW-1, P0-02, крейт `discord-codec`).
//!
//! **Провенанс** (независимый оракул, все значения сверены с артефактами):
//! - сырые байты термов — `capture-run2.jsonl`, сокет `requestId 95974.1389`, вырезаны
//!   `gw-slice-terms.mjs` (копия проверенного `gw-decode.mjs`);
//! - ожидаемые значения — `/home/user/clean-room-oracle/gw-proto/msgs-run2-95974_1389.jsonl`
//!   (поле `term`, индекс `idx`); длины термов совпадают с полем `bytes` (длина без `0x83`):
//!   138→137, 43→42, 210→209, 148→147, 257→256;
//! - спека: `docs/re/gateway-protocol.md` §2 (11 фактических тегов, big-int — знак ДО цифр,
//!   «nil»-атом ≠ `NIL_EXT`, хвост `LIST_EXT` = `NIL_EXT`).
//!
//! Тесты фиксированы ТЗ RW-01; hex-константы менять нельзя (иначе потеряется оракул).

use discord_codec::etf::{self, EtfError, EtfValue};

/// Строка hex → байты (пробелы игнорируются).
fn unhex(s: &str) -> Vec<u8> {
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    assert_eq!(compact.len() % 2, 0, "hex нечётной длины: {s}");
    (0..compact.len() / 2)
        .map(|i| u8::from_str_radix(&compact[2 * i..2 * i + 2], 16).expect("hex"))
        .collect()
}

/// Байты → строка hex (для читаемых расхождений round-trip).
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

/// Map с атомными ключами (порядок как в векторах).
fn map(pairs: Vec<(&str, EtfValue)>) -> EtfValue {
    EtfValue::Map(pairs.into_iter().map(|(k, v)| (atom(k), v)).collect())
}

/// HELLO, терм #1, 138 Б (полный поток провода, с ведущим `0x83`).
const HELLO_HEX: &str = "8374000000046400016474000000026400065f74726163656c000000016d000000325b22676174657761792d7072642d61726d2d75732d65617374312d642d30346d35222c7b226d6963726f73223a302e307d5d6a6400126865617274626561745f696e74657276616c620000a1226400026f70610a640001736400036e696c640001746400036e696c";

/// HEARTBEAT ACK, терм #6, 43 Б.
const ACK_HEX: &str =
    "837400000004640001646400036e696c6400026f70610b640001736400036e696c640001746400036e696c";

/// PRESENCE_UPDATE, терм #18, 210 Б.
const PRESENCE_HEX: &str = "83740000000464000164740000000664000a616374697669746965736a64000d636c69656e745f73746174757374000000026400066d6f62696c656400066f6e6c696e65640002767264000469646c656400086775696c645f69646e0800143004f5cc5c140e64001670726f6365737365645f61745f74696d657374616d7061006400067374617475736400066f6e6c696e6564000475736572740000000164000269646e0800500040c7f3dabf096400026f70610064000173610d6400017464000f50524553454e43455f555044415445";

/// MESSAGE_ACK, терм #46, 148 Б (ключи внутри `d` — бинарники `0x6d`).
const MESSAGE_ACK_HEX: &str = "8374000000046400016474000000056d0000000a6368616e6e656c5f69646e08000b1082a7ed57f2116d00000005666c61677361006d0000000b6c6173745f76696577656462000010d66d0000000a6d6573736167655f69646e08007b0040228806ab156d0000000776657273696f6e620001f7b66400026f7061006400017361266400017464000b4d4553534147455f41434b";

/// GUILD_MEMBER_LIST_UPDATE, терм #75, 257 Б.
const MEMBER_LIST_HEX: &str = "83740000000464000164740000000664000667726f7570736c000000027400000002640005636f756e74610364000269646e0800795084b7b7a1bf0e7400000002640005636f756e74610c64000269646400066f6e6c696e656a6400086775696c645f69646e0800143004f5cc5c140e64000269646d0000000a3136353936303636343764000c6d656d6265725f636f756e746200005ea464000c6f6e6c696e655f636f756e7462000002c46400036f70736c000000017400000002640005696e646578610b6400026f7064000644454c4554456a6400026f70610064000173613a640001746400184755494c445f4d454d4245525f4c4953545f555044415445";

/// Разбор вектора с проверкой `consumed` и byte-exact канона.
fn decode_framed(name: &str, hex_src: &str) -> EtfValue {
    let bytes = unhex(hex_src);
    let (value, consumed) = etf::decode(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(consumed, bytes.len(), "{name}: consumed");
    let canonical = etf::encode(&value);
    assert_eq!(
        hex(&canonical),
        hex(&bytes),
        "{name}: канон не совпал с проводом"
    );
    value
}

#[test]
fn vector_lengths_match_capture_bytes_field() {
    // Поле `bytes` в `gw-proto/msgs-run2-95974_1389.jsonl` — длина терма БЕЗ ведущего 0x83.
    for (name, source, bytes_field) in [
        ("#1 HELLO", HELLO_HEX, 137),
        ("#6 HEARTBEAT ACK", ACK_HEX, 42),
        ("#18 PRESENCE_UPDATE", PRESENCE_HEX, 209),
        ("#46 MESSAGE_ACK", MESSAGE_ACK_HEX, 147),
        ("#75 GUILD_MEMBER_LIST_UPDATE", MEMBER_LIST_HEX, 256),
    ] {
        let raw = unhex(source);
        assert_eq!(raw.len(), bytes_field + 1, "{name}: длина с 0x83");
        assert_eq!(raw[0], 0x83, "{name}: ведущий маркер терма");
    }
}

#[test]
fn eleven_wire_tags_decode_with_exact_consumed_len() {
    // Таблица §2 спеки: 11 тегов, реально встречающихся в захвате (без 0x83 — decode_term).
    let cases: Vec<(&str, &str, EtfValue)> = vec![
        (
            "0x46 NEW_FLOAT_EXT",
            "463ff8000000000000",
            EtfValue::Float(1.5),
        ),
        ("0x61 SMALL_INTEGER_EXT", "610a", int(10)),
        ("0x62 INTEGER_EXT", "620000a122", int(41250)),
        ("0x64 ATOM_EXT", "6400026f70", atom("op")),
        (
            "0x68 SMALL_TUPLE_EXT",
            "680261016102",
            EtfValue::Tuple(vec![int(1), int(2)]),
        ),
        ("0x6a NIL_EXT", "6a", EtfValue::Nil),
        ("0x6b STRING_EXT (latin1)", "6b0003616263", text("abc")),
        (
            "0x6c LIST_EXT (хвост NIL)",
            "6c00000002610161026a",
            EtfValue::List(vec![int(1), int(2)]),
        ),
        ("0x6d BINARY_EXT (UTF-8)", "6d00000003616263", text("abc")),
        ("0x6e SMALL_BIG_EXT", "6e0100ff", int(255)),
        (
            "0x74 MAP_EXT",
            "7400000001640001616101",
            EtfValue::Map(vec![(atom("a"), int(1))]),
        ),
    ];
    assert_eq!(
        cases.len(),
        11,
        "в таблице обязаны быть все 11 тегов захвата"
    );
    for (name, src, expected) in cases {
        let bytes = unhex(src);
        let (value, consumed) = etf::decode_term(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(value, expected, "{name}");
        assert_eq!(consumed, bytes.len(), "{name}: consumed");
    }
}

#[test]
fn supported_but_unseen_tags_decode_per_contract() {
    // Теги, поддержанные §2 спеки, но в захвате не встреченные (latin1, LARGE_*, UTF-8-атомы).
    let float_ext = format!("63312e35{}", "20".repeat(28)); // "1.5" + 28 пробелов = 31 Б
    let cases: Vec<(&str, &str, EtfValue)> = vec![
        (
            "0x63 FLOAT_EXT (ASCII + пробелы)",
            float_ext.as_str(),
            EtfValue::Float(1.5),
        ),
        (
            "0x69 LARGE_TUPLE_EXT",
            "690000000261016102",
            EtfValue::Tuple(vec![int(1), int(2)]),
        ),
        (
            "0x6f LARGE_BIG_EXT (guild_id)",
            "6f0000000800143004f5cc5c140e",
            int(1_014_537_851_418_980_372),
        ),
        ("0x73 SMALL_ATOM_EXT (latin1)", "73026f70", atom("op")),
        ("0x76 ATOM_UTF8_EXT", "7600026f70", atom("op")),
        ("0x77 SMALL_ATOM_UTF8_EXT", "77026f70", atom("op")),
        ("0x64 ATOM_EXT latin1 0xff", "640001ff", atom("\u{ff}")),
        ("0x73 SMALL_ATOM_EXT latin1 0xe8", "7301e8", atom("\u{e8}")),
        ("0x6b STRING_EXT latin1 0xe8", "6b0001e8", text("\u{e8}")),
        (
            "0x6d BINARY_EXT UTF-8 «я»",
            "6d00000002d18f",
            text("\u{44f}"),
        ),
    ];
    for (name, src, expected) in cases {
        let bytes = unhex(src);
        let (value, consumed) = etf::decode_term(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(value, expected, "{name}");
        assert_eq!(consumed, bytes.len(), "{name}: consumed");
    }
}

#[test]
fn decode_skips_only_the_leading_version_byte() {
    let hello = unhex(HELLO_HEX);
    let (framed, consumed) = etf::decode(&hello).expect("decode framed");
    assert_eq!(consumed, 138, "0x83 входит в consumed");
    let payload = &hello[1..];
    let (unframed, payload_consumed) = etf::decode_term(payload).expect("decode_term payload");
    assert_eq!(unframed, framed, "значение без 0x83 совпадает с framed");
    assert_eq!(payload_consumed, 137);
    assert_eq!(payload_consumed + 1, consumed);
    // С 0x83 в начале decode_term обязан ругаться, а не «проглатывать» байт.
    assert_eq!(
        etf::decode_term(&hello),
        Err(EtfError::UnknownTag { tag: 0x83, at: 0 })
    );
}

#[test]
fn hello_term_full_tree_matches_capture() {
    let value = decode_framed("HELLO #1", HELLO_HEX);
    let expected = map(vec![
        (
            "d",
            map(vec![
                (
                    "_trace",
                    EtfValue::List(vec![text(
                        r#"["gateway-prd-arm-us-east1-d-04m5",{"micros":0.0}]"#,
                    )]),
                ),
                ("heartbeat_interval", int(41_250)),
            ]),
        ),
        ("op", int(10)),
        ("s", atom("nil")),
        ("t", atom("nil")),
    ]);
    assert_eq!(value, expected);
    // Точечно: `_trace` — список из одной строки, сердцебиение — 41 250 мс.
    assert_eq!(
        value.get("d").and_then(|d| d.get("heartbeat_interval")),
        Some(&int(41_250))
    );
    assert_eq!(
        value
            .get("d")
            .and_then(|d| d.get("_trace"))
            .and_then(EtfValue::as_list)
            .map(<[EtfValue]>::len),
        Some(1)
    );
}

#[test]
fn heartbeat_ack_keeps_atom_nil_and_int_op() {
    let value = decode_framed("HEARTBEAT ACK #6", ACK_HEX);
    // Полное дерево: `d` — атом-строка "nil", НЕ NIL_EXT.
    let expected = map(vec![
        ("d", atom("nil")),
        ("op", int(11)),
        ("s", atom("nil")),
        ("t", atom("nil")),
    ]);
    assert_eq!(value, expected);
    assert_eq!(value.get("d"), Some(&atom("nil")));
    assert_ne!(value.get("d"), Some(&EtfValue::Nil), "атом nil ≠ NIL_EXT");
    assert_eq!(value.get("d").and_then(EtfValue::as_atom), Some("nil"));
    assert_eq!(
        value.get("d").and_then(EtfValue::as_str),
        None,
        "это атом, не бинарник"
    );
    assert_eq!(value.get("op").and_then(EtfValue::as_int), Some(11));
}

#[test]
fn presence_update_snowflakes_and_real_nil_activities() {
    let value = decode_framed("PRESENCE_UPDATE #18", PRESENCE_HEX);
    let d = value.get("d").expect("d");
    assert_eq!(value.get("op").and_then(EtfValue::as_int), Some(0));
    assert_eq!(value.get("s").and_then(EtfValue::as_int), Some(13));
    assert_eq!(value.get("t"), Some(&atom("PRESENCE_UPDATE")));
    assert_eq!(
        d.get("guild_id").and_then(EtfValue::as_int),
        Some(1_014_537_851_418_980_372)
    );
    assert_eq!(
        d.get("user")
            .and_then(|u| u.get("id"))
            .and_then(EtfValue::as_int),
        Some(702_520_807_447_855_184)
    );
    // activities пришли НАСТОЯЩИМ NIL_EXT (0x6a), а не атомом "nil".
    assert_eq!(d.get("activities"), Some(&EtfValue::Nil));
    assert_eq!(d.get("status"), Some(&atom("online")));
    assert_eq!(
        d.get("processed_at_timestamp").and_then(EtfValue::as_int),
        Some(0)
    );
    let client_status = d.get("client_status").expect("client_status");
    assert_eq!(client_status.get("mobile"), Some(&atom("online")));
    assert_eq!(client_status.get("vr"), Some(&atom("idle")));
}

#[test]
fn message_ack_binary_keys_are_found_by_get() {
    let value = decode_framed("MESSAGE_ACK #46", MESSAGE_ACK_HEX);
    let d = value.get("d").expect("d");
    // Ключи внутри `d` — бинарники (0x6d), поэтому `get` обязан искать и по Str.
    for key in [
        "channel_id",
        "flags",
        "last_viewed",
        "message_id",
        "version",
    ] {
        let (k, _) = d
            .as_map()
            .expect("map")
            .iter()
            .find(|(k, _)| k.as_str() == Some(key))
            .unwrap_or_else(|| panic!("ключ {key} не найден"));
        assert_eq!(k.as_atom(), None, "ключ {key} — бинарник, не атом");
    }
    assert_eq!(
        d.get("channel_id").and_then(EtfValue::as_int),
        Some(1_293_192_721_237_938_187)
    );
    assert_eq!(d.get("flags").and_then(EtfValue::as_int), Some(0));
    assert_eq!(d.get("last_viewed").and_then(EtfValue::as_int), Some(4_310));
    assert_eq!(
        d.get("message_id").and_then(EtfValue::as_int),
        Some(1_561_348_877_573_947_515)
    );
    assert_eq!(d.get("version").and_then(EtfValue::as_int), Some(128_950));
    assert_eq!(value.get("op").and_then(EtfValue::as_int), Some(0));
    assert_eq!(value.get("s").and_then(EtfValue::as_int), Some(38));
    assert_eq!(value.get("t"), Some(&atom("MESSAGE_ACK")));
}

#[test]
fn guild_member_list_update_groups_ops_and_string_id() {
    let value = decode_framed("GUILD_MEMBER_LIST_UPDATE #75", MEMBER_LIST_HEX);
    let d = value.get("d").expect("d");
    assert_eq!(
        d.get("id"),
        Some(&text("1659606647")),
        "id пришёл бинарником"
    );
    assert_eq!(
        d.get("guild_id").and_then(EtfValue::as_int),
        Some(1_014_537_851_418_980_372)
    );
    assert_eq!(
        d.get("member_count").and_then(EtfValue::as_int),
        Some(24_228)
    );
    assert_eq!(d.get("online_count").and_then(EtfValue::as_int), Some(708));
    let groups = d.get("groups").and_then(EtfValue::as_list).expect("groups");
    assert_eq!(groups.len(), 2);
    assert_eq!(
        groups[0].get("id").and_then(EtfValue::as_int),
        Some(1_062_745_847_512_715_385)
    );
    assert_eq!(groups[0].get("count").and_then(EtfValue::as_int), Some(3));
    assert_eq!(
        groups[1].get("id"),
        Some(&atom("online")),
        "второй id — атом"
    );
    assert_eq!(groups[1].get("count").and_then(EtfValue::as_int), Some(12));
    let ops = d.get("ops").and_then(EtfValue::as_list).expect("ops");
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].get("index").and_then(EtfValue::as_int), Some(11));
    assert_eq!(ops[0].get("op"), Some(&atom("DELETE")));
    assert_eq!(value.get("op").and_then(EtfValue::as_int), Some(0));
    assert_eq!(value.get("s").and_then(EtfValue::as_int), Some(58));
    assert_eq!(value.get("t"), Some(&atom("GUILD_MEMBER_LIST_UPDATE")));
}

#[test]
fn two_terms_back_to_back_split_by_consumed() {
    let mut stream = unhex(HELLO_HEX);
    stream.extend(unhex(ACK_HEX));
    let (hello, first) = etf::decode(&stream).expect("терм 1");
    assert_eq!(first, 138);
    let (ack, second) = etf::decode(&stream[first..]).expect("терм 2");
    assert_eq!(second, 43);
    assert_eq!(hello, decode_framed("HELLO #1", HELLO_HEX));
    assert_eq!(ack, decode_framed("HEARTBEAT ACK #6", ACK_HEX));
    assert_eq!(first + second, stream.len());
}

#[test]
fn small_big_int_reads_sign_before_digits() {
    // §2.5 спеки: `[110][n:u8][sign:u8][n байт LE]`; знак — ДО цифр.
    let cases: [(i64, &str, i64); 3] = [
        (
            1_014_537_851_418_980_372,
            "6e0800143004f5cc5c140e",
            -1_467_272_931_325_252_608,
        ),
        (
            384_299_970_632_876_032,
            "6e08000000c484c94e5505",
            -6_147_072_113_468_506_112,
        ),
        (
            1_176_785_373_989_445_703,
            "6e08004720841110c85410",
            -6_109_150_561_945_274_112,
        ),
    ];
    for (expected, src, wrong) in cases {
        let (value, consumed) = etf::decode_term(&unhex(src)).expect(src);
        assert_eq!(consumed, unhex(src).len());
        assert_eq!(value, int(expected), "{src}");
        assert_ne!(
            value,
            int(wrong),
            "{src}: разбор «знак после цифр» дал бы {wrong}"
        );
        assert!(expected > 0, "{src}: snowflake положительный");
    }
    // Тот же big-int в формате LARGE_BIG_EXT (111) — поддержан, в захвате не встречался.
    let large = unhex("6f0000000800143004f5cc5c140e");
    assert_eq!(
        etf::decode_term(&large),
        Ok((int(1_014_537_851_418_980_372), 14))
    );
}

#[test]
fn atom_nil_is_a_string_and_not_nil_ext() {
    let (from_atom, atom_len) = etf::decode_term(&unhex("6400036e696c")).expect("атом nil");
    let (from_nil, nil_len) = etf::decode_term(&unhex("6a")).expect("NIL_EXT");
    assert_eq!(from_atom, atom("nil"));
    assert_eq!(atom_len, 6);
    assert_eq!(from_nil, EtfValue::Nil);
    assert_eq!(nil_len, 1);
    assert_ne!(from_atom, from_nil, "атом nil — не null");
    assert_eq!(from_atom.as_atom(), Some("nil"));
    assert_eq!(from_atom.as_str(), None);
    assert_eq!(from_nil.as_atom(), None);
}

#[test]
fn list_tail_is_nil_ext_only() {
    // Реальный хвост LIST_EXT — NIL_EXT; Int на месте хвоста обязан ломать разбор.
    let ok = etf::decode_term(&unhex("6c0000000161016a")).expect("хвост NIL");
    assert_eq!(ok, (EtfValue::List(vec![int(1)]), 8));
    match etf::decode_term(&unhex("6c000000016101 6102")) {
        Err(EtfError::ListTailNotNil { tail_tag, .. }) => assert_eq!(tail_tag, 0x61),
        other => panic!("ожидался ListTailNotNil, получено {other:?}"),
    }
}
