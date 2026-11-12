//! W12a / P2-01a: READY — гильдии, каналы, роли, эмодзи, стикеры (кейсы A1–A14).
//!
//! Оракул — `fixtures/gateway/by-type/002-READY.json`
//! (md5 `a1a8fd5662be88108dd25ac619a377f2`) и контракт
//! `waves/RW-12/DESIGN-P2-01.md` §2–§3, §5.1. Набор обязан быть зелёным при заглушках
//! W12b/W12c: `users`, `private_channels`, `relationships`, `sessions`, `read_state`,
//! `user_guild_settings` и `merged_members` здесь не проверяются — это B/C-кейсы.
//! Привязку `merged_members[i] → guilds[i].members` закрывает B6 `ready_social.rs`.

mod support;

use chord_codec::etf::EtfValue;
use chord_core::ready::{
    Channel, Emoji, Guild, Ready, ReadyDiagnostics, ReadyError, Role, Sticker, TOP_LEVEL_KEYS,
};

/// Полный READY-кадр из курированной фикстуры.
fn dispatch() -> EtfValue {
    support::ready_fixture::load_ready().expect("READY-фикстура читается")
}

/// Синтетический JSON → `EtfValue` (форма провода: `nil`/`true`/`false` — атомы).
fn payload(json: &str) -> EtfValue {
    support::ready_fixture::parse_json(json).expect("синтетический JSON разбирается")
}

fn parse(json: &str) -> Ready {
    Ready::parse(&payload(json)).expect("READY разбирается")
}

fn parse_error(json: &str) -> ReadyError {
    Ready::parse(&payload(json)).expect_err("ожидалась ошибка READY")
}

fn fixture_ready() -> Ready {
    Ready::from_dispatch(&dispatch()).expect("полный op0 READY разбирается")
}

fn guild(ready: &Ready, id: u64) -> &Guild {
    ready
        .guilds
        .iter()
        .find(|guild| guild.id == id)
        .unwrap_or_else(|| panic!("гильдия {id} не найдена"))
}

fn channel(guild: &Guild, id: u64) -> &Channel {
    guild
        .channels
        .iter()
        .find(|channel| channel.id == id)
        .unwrap_or_else(|| panic!("канал {id} не найден"))
}

fn role(guild: &Guild, id: u64) -> &Role {
    guild
        .roles
        .iter()
        .find(|role| role.id == id)
        .unwrap_or_else(|| panic!("роль {id} не найдена"))
}

fn emoji(guild: &Guild, id: u64) -> &Emoji {
    guild
        .emojis
        .iter()
        .find(|emoji| emoji.id == id)
        .unwrap_or_else(|| panic!("эмодзи {id} не найден"))
}

fn sticker(guild: &Guild, id: u64) -> &Sticker {
    guild
        .stickers
        .iter()
        .find(|sticker| sticker.id == id)
        .unwrap_or_else(|| panic!("стикер {id} не найден"))
}

/// A1: минимальный READY (синтетика) — 3 ключа, дефолты, пустая диагностика.
#[test]
fn a1_minimal_ready_defaults() {
    let ready = parse(r#"{"user":{"id":"1"},"guilds":[],"session_id":"s"}"#);

    assert_eq!(ready.top_level_keys, 3);
    assert_eq!(ready.v, None);
    assert_eq!(ready.session_id, "s");
    assert_eq!(ready.user.id, 1);
    assert!(ready.guilds.is_empty());
    assert!(ready.private_channels.is_empty());
    assert!(ready.users.is_empty());
    assert!(ready.relationships.is_empty());
    assert!(ready.read_state.entries.is_empty());
    assert!(ready.user_guild_settings.entries.is_empty());
    assert!(ready.sessions.is_empty());
    assert!(ready.geo_ordered_rtc_regions.is_empty());
    assert_eq!(ready.country_code, None);
    assert_eq!(ready.subdivision_code, None);
    assert_eq!(ready.friend_suggestion_count, None);
    assert_eq!(ready.api_code_version, None);
    assert_eq!(ready.explicit_content_scan_version, None);
    assert!(!ready.ad_personalization_toggles_disabled);
    assert_eq!(ready.user_settings_proto, None);
    assert_eq!(ready.diagnostics, ReadyDiagnostics::default());
}

/// A2: полная фикстура — 40 ключей и числа jq-оракула; диагностика пуста.
#[test]
fn a2_full_fixture_counts_match_jq_oracle() {
    assert_eq!(TOP_LEVEL_KEYS.len(), 40);

    let ready = fixture_ready();
    assert_eq!(ready.top_level_keys, 40);
    assert_eq!(ready.guilds.len(), 7);

    let channels: usize = ready.guilds.iter().map(|guild| guild.channels.len()).sum();
    let roles: usize = ready.guilds.iter().map(|guild| guild.roles.len()).sum();
    let emojis: usize = ready.guilds.iter().map(|guild| guild.emojis.len()).sum();
    let stickers: usize = ready.guilds.iter().map(|guild| guild.stickers.len()).sum();
    assert_eq!(
        (channels, roles, emojis, stickers),
        (272, 132, 363, 51),
        "jq: channels=272, roles=132, emojis=363, stickers=51"
    );

    assert_eq!(ready.user.id, 384_299_970_632_876_032);
    assert_eq!(
        ready.user_settings_proto.as_deref().map(str::len),
        Some(4400),
        "user_settings_proto — base64-строка 4400 символов"
    );
    assert_eq!(ready.diagnostics, ReadyDiagnostics::default());
}

/// A3: крупная гильдия ITSKIDON — числа jq-оракула (members — B6/W12b).
#[test]
fn a3_large_guild_matches_capture() {
    let ready = fixture_ready();
    let guild = guild(&ready, 1_014_537_851_418_980_372);

    assert_eq!(guild.name.as_deref(), Some("ITSKIDON PC BOOST"));
    assert_eq!(
        guild.icon.as_deref(),
        Some("b4b4c3850182f7e0dbbaa116df2ed3f2")
    );
    assert_eq!(guild.owner_id, Some(375_401_893_679_333_376));
    assert_eq!(guild.data_mode.as_deref(), Some("full"));
    assert!(guild.large);
    assert!(guild.lazy);
    assert!(!guild.unavailable);

    assert_eq!(guild.channels.len(), 83);
    let by_type = |kind: i64| {
        guild
            .channels
            .iter()
            .filter(|channel| channel.channel_type == kind)
            .count()
    };
    assert_eq!(
        (by_type(0), by_type(2), by_type(4), by_type(15)),
        (47, 20, 12, 4),
        "jq: type 0:47, 2:20, 4:12, 15:4"
    );
    assert_eq!(guild.roles.len(), 34);
    assert_eq!(guild.emojis.len(), 46);
    assert_eq!(guild.stickers.len(), 6);
    assert_eq!(guild.member_count, Some(24_228));
    assert_eq!(guild.premium_subscription_count, Some(5));
    assert_eq!(
        guild.joined_at.as_deref(),
        Some("2023-12-25T17:19:01.733000+00:00")
    );
    assert_eq!(guild.version, Some(1_792_391_252_875));

    // `Guild.members` наполняется из `merged_members` парсером W12b (на старте W12a — заглушка);
    // контракт members.len()=1 проверяется в B6 `ready_social.rs`, чтобы W12a был зелёным
    // независимо от соседа.
}

/// A4: `unavailable` (синтетика; в capture не встречался) — true, остальное пусто.
#[test]
fn a4_unavailable_guild_synthetic() {
    let ready = parse(
        r#"{"user":{"id":"1"},"session_id":"s","guilds":[{"id":"42","unavailable":"true"}]}"#,
    );

    assert_eq!(ready.guilds.len(), 1);
    let guild = &ready.guilds[0];
    assert!(guild.unavailable);
    assert_eq!(guild.name, None);
    assert!(guild.channels.is_empty());
    assert!(guild.roles.is_empty());
    assert!(guild.emojis.is_empty());
    assert!(guild.stickers.is_empty());
}

/// A5: неизвестные поля — top-level в диагностику, значения остаются в `raw`.
#[test]
fn a5_unknown_fields_are_kept_in_raw() {
    let ready = parse(
        r#"{"user":{"id":"1"},"session_id":"s","new_top":"x",
            "guilds":[{"id":"1","new_guild":"g",
                       "channels":[{"id":"2","type":0,"new_channel":"c"}]}]}"#,
    );

    assert_eq!(
        ready.diagnostics.unknown_top_level,
        vec!["new_top".to_string()]
    );
    assert!(matches!(
        ready.raw().get("new_top"),
        Some(EtfValue::Str(text)) if text == "x"
    ));

    let control = parse(
        r#"{"user":{"id":"1"},"session_id":"s",
            "guilds":[{"id":"1","channels":[{"id":"2","type":0}]}]}"#,
    );
    assert_eq!(ready.guilds.len(), control.guilds.len());
    assert_eq!(
        ready.guilds[0].channels.len(),
        control.guilds[0].channels.len()
    );
    assert_eq!(ready.guilds[0].channels[0], control.guilds[0].channels[0]);
}

/// A6: канал только `{id,type}`; гильдия без emojis/stickers — дефолты без диагностики.
#[test]
fn a6_minimal_channel_and_guild_defaults() {
    let ready = parse(
        r#"{"user":{"id":"1"},"session_id":"s",
            "guilds":[{"id":"42","channels":[{"id":"7","type":0}]}]}"#,
    );

    let guild = &ready.guilds[0];
    assert_eq!(guild.name, None);
    assert!(guild.emojis.is_empty());
    assert!(guild.stickers.is_empty());

    let channel = &guild.channels[0];
    assert_eq!(channel.name, None);
    assert_eq!(channel.position, 0);
    assert!(channel.permission_overwrites.is_empty());
    assert_eq!(ready.diagnostics, ReadyDiagnostics::default());
}

/// A7: битые якоря — `WrongType`/`Missing` с точными путями (регрессия P-6).
#[test]
fn a7_broken_anchors_are_fatal() {
    let error = parse_error(r#"{"user":{"id":"1"},"session_id":"s","guilds":"x"}"#);
    match &error {
        ReadyError::WrongType {
            path,
            expected,
            found,
        } => {
            assert_eq!(path, "d.guilds");
            assert_eq!(*expected, "list");
            assert_eq!(*found, "str");
        }
        other => panic!("ожидался WrongType(d.guilds), получено {other:?}"),
    }

    let error = parse_error(r#"{"user":5,"guilds":[],"session_id":"s"}"#);
    match &error {
        ReadyError::WrongType {
            path,
            expected,
            found,
        } => {
            assert_eq!(path, "d.user");
            assert_eq!(*expected, "map");
            assert_eq!(*found, "int");
        }
        other => panic!("ожидался WrongType(d.user), получено {other:?}"),
    }

    let error = parse_error(r#"{"user":{"id":"1"},"guilds":[],"session_id":5}"#);
    match &error {
        ReadyError::WrongType {
            path,
            expected,
            found,
        } => {
            assert_eq!(path, "d.session_id");
            assert_eq!(*expected, "str");
            assert_eq!(*found, "int");
        }
        other => panic!("ожидался WrongType(d.session_id), получено {other:?}"),
    }

    let error = parse_error(r#"{"guilds":[],"session_id":"s"}"#);
    match &error {
        ReadyError::Missing { path } => assert_eq!(path, "d.user"),
        other => panic!("ожидался Missing(d.user), получено {other:?}"),
    }
}

/// A8: битые элементы `guilds`/`channels` — skip + `malformed`; поле — дефолт + `ignored`.
#[test]
fn a8_broken_guild_and_channel_elements_are_skipped() {
    let ready = parse(
        r#"{"user":{"id":"1"},"session_id":"s",
            "guilds":[{"id":"abc"},
                      {"id":"2","channels":[{"id":"3","type":0,"position":"x"}]}]}"#,
    );

    assert_eq!(ready.guilds.len(), 1, "битая гильдия пропущена");
    assert_eq!(ready.guilds[0].id, 2);
    assert_eq!(ready.guilds[0].channels.len(), 1);
    assert_eq!(
        ready.guilds[0].channels[0].position, 0,
        "битый position → дефолт 0"
    );
    assert_eq!(ready.diagnostics.malformed, vec!["guilds[0]".to_string()]);
    assert!(
        ready
            .diagnostics
            .ignored
            .iter()
            .any(|entry| entry == "d.guilds[1].channels[0].position"),
        "ignored: {:?}",
        ready.diagnostics.ignored
    );
}

/// A9: дубликаты — last wins, позиция первого вхождения, запись в `duplicates`.
#[test]
fn a9_duplicates_last_wins_first_position() {
    let ready = parse(
        r#"{"user":{"id":"1"},"session_id":"s",
            "guilds":[{"id":"1","name":"first","channels":[]},
                      {"id":"1","name":"second","channels":[
                          {"id":"1","type":0,"name":"a"},
                          {"id":"2","type":0,"name":"b"},
                          {"id":"1","type":0,"name":"c"}]}]}"#,
    );

    assert_eq!(ready.guilds.len(), 1, "дубликат гильдии схлопнут");
    let guild = &ready.guilds[0];
    assert_eq!(
        guild.name.as_deref(),
        Some("second"),
        "last wins: значение последней гильдии"
    );

    assert_eq!(guild.channels.len(), 2);
    assert_eq!(
        (guild.channels[0].id, guild.channels[0].name.as_deref()),
        (1, Some("c")),
        "позиция первого вхождения, значение последнего"
    );
    assert_eq!(
        (guild.channels[1].id, guild.channels[1].name.as_deref()),
        (2, Some("b"))
    );

    assert!(
        ready
            .diagnostics
            .duplicates
            .iter()
            .any(|entry| entry == "guilds:1"),
        "duplicates: {:?}",
        ready.diagnostics.duplicates
    );
    assert!(
        ready
            .diagnostics
            .duplicates
            .iter()
            .any(|entry| entry == "guilds:1.channels:1"),
        "duplicates: {:?}",
        ready.diagnostics.duplicates
    );
    assert_eq!(ready.diagnostics.duplicates.len(), 2);
}

/// A10: пустые списки и `nil` вложенных коллекций гильдии — пустые `Vec`, без диагностики.
#[test]
fn a10_empty_and_nil_guild_collections() {
    let ready = parse(
        r#"{"user":{"id":"1"},"session_id":"s",
            "guilds":[{"id":"42","channels":[],"roles":"nil","emojis":null,"stickers":[]}]}"#,
    );

    let guild = &ready.guilds[0];
    assert!(guild.channels.is_empty());
    assert!(guild.roles.is_empty());
    assert!(guild.emojis.is_empty());
    assert!(guild.stickers.is_empty());
    assert_eq!(ready.diagnostics, ReadyDiagnostics::default());
}

/// A11: регрессия big-int — отрицательный id гильдии (Int/Str) пропускается; `user.id` — InvalidId.
#[test]
fn a11_negative_ids_are_rejected() {
    let ready = parse(
        r#"{"user":{"id":"1"},"session_id":"s",
            "guilds":[{"id":-1467272931325252608},{"id":"-1467272931325252608"}]}"#,
    );

    assert!(ready.guilds.is_empty());
    assert_eq!(
        ready.diagnostics.malformed,
        vec!["guilds[0]".to_string(), "guilds[1]".to_string()]
    );

    let error = parse_error(r#"{"user":{"id":-5},"guilds":[],"session_id":"s"}"#);
    assert!(
        matches!(&error, ReadyError::InvalidId { path, .. } if path == "d.user.id"),
        "user.id: {error:?}"
    );
}

/// A12: детали канала/роли/эмодзи/стикера по id (jq-оракул).
#[test]
fn a12_channel_role_emoji_sticker_details() {
    let ready = fixture_ready();
    let itskidon = guild(&ready, 1_014_537_851_418_980_372);

    let channel = channel(itskidon, 1_275_727_867_497_680_918);
    assert_eq!(channel.parent_id, Some(1_167_440_450_416_824_430));
    assert_eq!(channel.position, 29);
    assert_eq!(channel.permission_overwrites.len(), 2);
    assert_eq!(channel.permission_overwrites[0].allow, 412_317_240_385);
    assert_eq!(channel.permission_overwrites[0].deny, 16);

    let role = role(itskidon, 1_057_598_992_449_552_425);
    assert_eq!(role.color, 16_711_684);
    assert_eq!(role.position, 30);
    assert_eq!(role.permissions, 41_780_636_024_647);
    assert!(role.hoist);

    // Эмодзи и стикер лежат в гильдии MONOLIT (274093052673392641), не в ITSKIDON.
    let monolit = guild(&ready, 274_093_052_673_392_641);
    let emoji = emoji(monolit, 590_239_122_757_648_416);
    assert!(emoji.roles.is_empty(), "roles=null на проводе → пустой Vec");

    let sticker = sticker(monolit, 875_058_193_758_289_990);
    assert_eq!(sticker.tags.as_deref(), Some("dizzy_face"));
    assert_eq!(sticker.format_type, 1);
    assert_eq!(sticker.sticker_type, 2);
}

/// A13: `Debug` не печатает секреты (`session_id`, `user_settings_proto`, `raw`).
#[test]
fn a13_debug_redacts_secrets() {
    let ready = parse(
        r#"{"user":{"id":"1"},"session_id":"SECRET_SESSION","guilds":[],
            "user_settings_proto":"SECRET_TOKEN","qos_token":"SECRET_QOS"}"#,
    );

    let debug = format!("{ready:?}");
    for secret in ["SECRET_SESSION", "SECRET_TOKEN", "SECRET_QOS"] {
        assert!(
            !debug.contains(secret),
            "Debug не должен печатать {secret}: {debug}"
        );
    }
}

/// A14: `from_dispatch` — op10 / `t:"OTHER"` / нет `d` (регрессия).
#[test]
fn a14_from_dispatch_rejects_non_ready_frames() {
    let op10 =
        payload(r#"{"op":10,"t":"READY","d":{"user":{"id":"1"},"guilds":[],"session_id":"s"}}"#);
    assert_eq!(Ready::from_dispatch(&op10), Err(ReadyError::NotReady));

    let other =
        payload(r#"{"op":0,"t":"OTHER","d":{"user":{"id":"1"},"guilds":[],"session_id":"s"}}"#);
    assert_eq!(Ready::from_dispatch(&other), Err(ReadyError::NotReady));

    let no_payload = payload(r#"{"op":0,"t":"READY"}"#);
    match Ready::from_dispatch(&no_payload) {
        Err(ReadyError::Missing { path }) => assert_eq!(path, "d"),
        other => panic!("ожидался Missing(d), получено {other:?}"),
    }
}
