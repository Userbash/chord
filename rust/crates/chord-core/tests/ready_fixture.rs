//! Проверка JSON→`EtfValue` адаптера на реальной READY-фикстуре (RW-12, precondition P-1).
//!
//! Адаптер — собственность координатора (`tests/support/ready_fixture.rs`); воркеры W12a/W12b/W12c
//! используют его в `tests/ready_core.rs` / `ready_social.rs` / `ready_read_state.rs`.
//! Оракул — числа jq из `waves/RW-12/DESIGN-P2-01.md` §2 и форма захвата
//! (`docs/re/gateway-protocol.md` §2): `nil`/`true`/`false` — атомы, `null` — `NIL_EXT`.

mod support;

use chord_codec::etf::EtfValue;
use chord_core::ready::{TOP_LEVEL_KEYS, value};

fn ready() -> EtfValue {
    support::ready_fixture::load_ready().expect("READY-фикстура читается и парсится")
}

fn d(ready: &EtfValue) -> &EtfValue {
    ready.get("d").expect("d")
}

#[test]
fn adapter_reads_ready_fixture_and_matches_oracle_counts() {
    let ready = ready();
    let d = d(&ready);

    let map = d.as_map().expect("d — map");
    assert_eq!(map.len(), 40);
    let mut keys: Vec<&str> = map
        .iter()
        .filter_map(|(key, _)| match key {
            EtfValue::Atom(name) | EtfValue::Str(name) => Some(name.as_str()),
            _ => None,
        })
        .collect();
    keys.sort_unstable();
    let mut expected = TOP_LEVEL_KEYS.to_vec();
    expected.sort_unstable();
    assert_eq!(keys, expected);

    assert_eq!(value::items_or_empty(d.get("guilds")).len(), 7);
    assert_eq!(value::items_or_empty(d.get("private_channels")).len(), 17);
    assert_eq!(value::items_or_empty(d.get("users")).len(), 146);
    assert_eq!(value::items_or_empty(d.get("relationships")).len(), 60);

    let (mut channels, mut roles, mut emojis, mut stickers) = (0, 0, 0, 0);
    for guild in value::items_or_empty(d.get("guilds")) {
        channels += value::items_or_empty(guild.get("channels")).len();
        roles += value::items_or_empty(guild.get("roles")).len();
        emojis += value::items_or_empty(guild.get("emojis")).len();
        stickers += value::items_or_empty(guild.get("stickers")).len();
    }
    assert_eq!(channels, 272);
    assert_eq!(roles, 132);
    assert_eq!(emojis, 363);
    assert_eq!(stickers, 51);

    let merged = value::items_or_empty(d.get("merged_members"));
    assert_eq!(merged.len(), 7);
    assert!(
        merged
            .iter()
            .all(|group| value::items_or_empty(Some(group)).len() == 1)
    );
}

#[test]
fn adapter_maps_capture_shape_nil_bool_ids_and_numbers() {
    let ready = ready();
    let d = d(&ready);

    // JSON `"nil"` — атом 0x64, а не строка; `value::is_null` его видит.
    let timestamp = value::items_or_empty(d.get("private_channels"))
        .first()
        .and_then(|channel| channel.get("is_message_request_timestamp"))
        .expect("is_message_request_timestamp");
    assert!(matches!(timestamp, EtfValue::Atom(name) if name == "nil"));
    assert!(value::is_null(timestamp));

    // JSON `"false"` — атом; `value::boolean` даёт false.
    let bot = d.get("user").and_then(|user| user.get("bot")).expect("bot");
    assert!(matches!(bot, EtfValue::Atom(name) if name == "false"));
    assert_eq!(value::boolean(bot), Some(false));

    // Десятичная строка ID остаётся Str и разбирается `unsigned`.
    let user_id = d
        .get("user")
        .and_then(|user| user.get("id"))
        .expect("user.id");
    assert!(matches!(user_id, EtfValue::Str(_)));
    assert_eq!(value::unsigned(user_id), Some(384_299_970_632_876_032));

    // Число — Int; крупная гильдия ITSKIDON.
    let guild = value::items_or_empty(d.get("guilds"))
        .iter()
        .find(|guild| {
            value::unsigned(guild.get("id").expect("guild.id")) == Some(1_014_537_851_418_980_372)
        })
        .expect("ITSKIDON");
    assert_eq!(
        value::integer(guild.get("member_count").expect("member_count")),
        Some(24_228)
    );
}

/// Якорь `d.user.id` (precondition P-6): битый id фатален ещё до наполнения полей user.
#[test]
fn ready_parse_rejects_top_level_user_with_bad_id() {
    let payload =
        support::ready_fixture::parse_json(r#"{"user":{"id":"abc"},"guilds":[],"session_id":"s"}"#)
            .expect("синтетика парсится");

    let error = chord_core::ready::Ready::parse(&payload)
        .expect_err("битый user.id обязан быть фатальным");
    assert!(
        matches!(
            &error,
            chord_core::ready::ReadyError::InvalidId { path, .. } if path == "d.user.id"
        ),
        "ожидался InvalidId(d.user.id), получено {error:?}"
    );
}
