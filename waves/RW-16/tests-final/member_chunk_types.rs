//! W16a / P2-05: разбор `GUILD_MEMBERS_CHUNK` на реальных фикстурах и синтетике.
//!
//! Оракул — `waves/RW-16/RESEARCH-P2-05.md` §1/§4: `022` — 4 участника,
//! `chunk_count/index=1/0`, `not_found` длины 1; `009` — legacy отрицательные id.
//! Тесты падают до реализации (`member_chunk` ещё нет) — это compile-red.

mod support;

use discord_core::codec::etf::EtfValue;
use discord_core::state::{
    Event, EventError, EventPayload, MemberChunk, MemberChunkMember, StateDiagnostics,
};
use support::state_fixture::{atom, frame, int, list, load, map, text};

fn parse(frame: &EtfValue) -> (MemberChunk, StateDiagnostics) {
    let event = Event::from_frame(frame).expect("GUILD_MEMBERS_CHUNK должен разбираться");
    match event.payload {
        EventPayload::GuildMembersChunk(chunk) => (chunk, event.diagnostics),
        other => panic!("ожидался GuildMembersChunk, получено {other:?}"),
    }
}

fn parse_err(frame: &EtfValue) -> EventError {
    Event::from_frame(frame).expect_err("кадр должен быть отвергнут")
}

fn user(id: &str) -> EtfValue {
    map(vec![("id", text(id)), ("username", text("user"))])
}

fn member(id: &str) -> EtfValue {
    map(vec![
        ("roles", atom("nil")),
        ("nick", atom("nil")),
        ("deaf", atom("false")),
        ("mute", atom("false")),
        ("user", user(id)),
    ])
}

fn member_with_nick(id: &str, nick: &str) -> EtfValue {
    map(vec![
        ("roles", atom("nil")),
        ("nick", text(nick)),
        ("user", user(id)),
    ])
}

// Тестовый конструктор покрывает все поля кадра; продуктовая сигнатура не раздувается.
#[allow(clippy::too_many_arguments)]
fn chunk_frame(
    seq: u64,
    guild_id: &str,
    chunk_index: Option<i64>,
    chunk_count: Option<i64>,
    members: Option<EtfValue>,
    not_found: Option<EtfValue>,
    nonce: Option<EtfValue>,
    presences: Option<EtfValue>,
) -> EtfValue {
    let mut pairs: Vec<(&str, EtfValue)> = vec![("guild_id", text(guild_id))];
    if let Some(index) = chunk_index {
        pairs.push(("chunk_index", int(index)));
    }
    if let Some(count) = chunk_count {
        pairs.push(("chunk_count", int(count)));
    }
    if let Some(members) = members {
        pairs.push(("members", members));
    }
    if let Some(not_found) = not_found {
        pairs.push(("not_found", not_found));
    }
    if let Some(nonce) = nonce {
        pairs.push(("nonce", nonce));
    }
    if let Some(presences) = presences {
        pairs.push(("presences", presences));
    }
    frame("GUILD_MEMBERS_CHUNK", seq, map(pairs))
}

fn corrected_legacy_frame() -> EtfValue {
    let mut frame = load("009-GUILD_MEMBERS_CHUNK.json");
    let EtfValue::Map(pairs) = &mut frame else {
        panic!("кадр — map");
    };
    for (key, value) in pairs.iter_mut() {
        if key.as_atom() == Some("d") {
            let EtfValue::Map(data) = value else {
                panic!("d — map");
            };
            for (data_key, data_value) in data.iter_mut() {
                if data_key.as_atom() == Some("guild_id") {
                    *data_value = text("123456789012345678");
                }
            }
        }
    }
    frame
}

#[test]
fn c01_parse_022_shape_matches_jq_oracle() {
    let (chunk, diagnostics) = parse(&load("022-GUILD_MEMBERS_CHUNK.json"));
    assert_eq!(chunk.guild_id, 294_064_825_540_280_335);
    assert_eq!(chunk.chunk_index, 0);
    assert_eq!(chunk.chunk_count, 1);
    assert_eq!(chunk.members.len(), 4);
    assert_eq!(
        chunk
            .members
            .iter()
            .map(|member| member.member.user_id)
            .collect::<Vec<_>>(),
        vec![
            159_985_870_458_322_944,
            465_601_078_914_711_572,
            566_215_798_759_096_322,
            875_330_208_104_853_504,
        ]
    );
    assert_eq!(chunk.members[0].user.username.as_deref(), Some("MEE6"));
    assert_eq!(chunk.members[0].member.roles.len(), 2);
    assert!(chunk.members[1].member.roles.is_empty());
    assert_eq!(chunk.not_found, vec![707_997_415_642_365_992]);
    assert_eq!(chunk.nonce, None);
    assert!(diagnostics.malformed.is_empty());
    assert!(diagnostics.ignored.is_empty());
}

#[test]
fn c02_legacy_009_original_is_invalid_id() {
    let error = parse_err(&load("009-GUILD_MEMBERS_CHUNK.json"));
    assert!(matches!(
        error,
        EventError::InvalidId { ref path, .. } if path == "d.guild_id"
    ));
}

#[test]
fn c03_legacy_009_corrected_guild_keeps_negative_member_malformed() {
    let (chunk, diagnostics) = parse(&corrected_legacy_frame());
    assert_eq!(chunk.guild_id, 123_456_789_012_345_678);
    assert!(chunk.members.is_empty());
    assert!(
        diagnostics
            .malformed
            .iter()
            .any(|entry| entry == "d.members[0]"),
        "ожидался malformed d.members[0], получено {:?}",
        diagnostics.malformed
    );
}

#[test]
fn c04_nonce_absent_is_none_and_preserved_when_present() {
    let (chunk, _) = parse(&load("022-GUILD_MEMBERS_CHUNK.json"));
    assert_eq!(chunk.nonce, None);

    let synthetic = chunk_frame(
        30,
        "294064825540280335",
        Some(0),
        Some(1),
        Some(list(vec![member("100")])),
        None,
        Some(text("nonce-1")),
        None,
    );
    let (chunk, diagnostics) = parse(&synthetic);
    assert_eq!(chunk.nonce.as_deref(), Some("nonce-1"));
    assert!(diagnostics.ignored.is_empty());
}

#[test]
fn c05_members_null_is_empty_without_diagnostics() {
    let synthetic = chunk_frame(
        30,
        "294064825540280335",
        Some(0),
        Some(1),
        Some(atom("nil")),
        None,
        None,
        None,
    );
    let (chunk, diagnostics) = parse(&synthetic);
    assert!(chunk.members.is_empty());
    assert!(diagnostics.ignored.is_empty());
    assert!(diagnostics.malformed.is_empty());
}

#[test]
fn c06_members_wrong_type_is_ignored() {
    let synthetic = chunk_frame(
        30,
        "294064825540280335",
        Some(0),
        Some(1),
        Some(text("not-a-list")),
        None,
        None,
        None,
    );
    let (chunk, diagnostics) = parse(&synthetic);
    assert!(chunk.members.is_empty());
    assert!(diagnostics.ignored.iter().any(|entry| entry == "d.members"));
}

#[test]
fn c07_bad_member_is_malformed_and_others_kept() {
    let synthetic = chunk_frame(
        30,
        "294064825540280335",
        Some(0),
        Some(1),
        Some(list(vec![
            member("100"),
            map(vec![("roles", atom("nil"))]),
            member("200"),
        ])),
        None,
        None,
        None,
    );
    let (chunk, diagnostics) = parse(&synthetic);
    assert_eq!(
        chunk
            .members
            .iter()
            .map(|member| member.member.user_id)
            .collect::<Vec<_>>(),
        vec![100, 200]
    );
    assert!(
        diagnostics
            .malformed
            .iter()
            .any(|entry| entry == "d.members[1]"),
        "ожидался malformed d.members[1], получено {:?}",
        diagnostics.malformed
    );
}

#[test]
fn c08_not_found_skips_bad_ids_and_keeps_good() {
    let synthetic = chunk_frame(
        30,
        "294064825540280335",
        Some(0),
        Some(1),
        Some(list(vec![member("100")])),
        Some(list(vec![
            int(7),
            text("8"),
            int(-1),
            text("bad"),
            atom("nil"),
        ])),
        None,
        None,
    );
    let (chunk, diagnostics) = parse(&synthetic);
    assert_eq!(chunk.not_found, vec![7, 8]);
    assert!(
        diagnostics
            .ignored
            .iter()
            .any(|entry| entry == "d.not_found[2]")
    );
    assert!(
        diagnostics
            .ignored
            .iter()
            .any(|entry| entry == "d.not_found[3]")
    );
    assert!(
        diagnostics
            .ignored
            .iter()
            .any(|entry| entry == "d.not_found[4]")
    );
}

#[test]
fn c09_missing_chunk_index_is_fatal() {
    let synthetic = chunk_frame(
        30,
        "294064825540280335",
        None,
        Some(1),
        Some(list(Vec::new())),
        None,
        None,
        None,
    );
    let error = parse_err(&synthetic);
    assert!(matches!(
        error,
        EventError::Missing { ref path } if path == "d.chunk_index"
    ));
}

#[test]
fn c10_missing_chunk_count_is_fatal() {
    let synthetic = chunk_frame(
        30,
        "294064825540280335",
        Some(0),
        None,
        Some(list(Vec::new())),
        None,
        None,
        None,
    );
    let error = parse_err(&synthetic);
    assert!(matches!(
        error,
        EventError::Missing { ref path } if path == "d.chunk_count"
    ));
}

#[test]
fn c11_zero_chunk_count_is_fatal() {
    let synthetic = chunk_frame(
        30,
        "294064825540280335",
        Some(0),
        Some(0),
        Some(list(Vec::new())),
        None,
        None,
        None,
    );
    let error = parse_err(&synthetic);
    assert!(matches!(
        error,
        EventError::WrongType { ref path, .. } if path == "d.chunk_count"
    ));
}

#[test]
fn c12_chunk_index_out_of_range_is_fatal() {
    let synthetic = chunk_frame(
        30,
        "294064825540280335",
        Some(1),
        Some(1),
        Some(list(Vec::new())),
        None,
        None,
        None,
    );
    let error = parse_err(&synthetic);
    assert!(matches!(
        error,
        EventError::WrongType { ref path, .. } if path == "d.chunk_index"
    ));
}

#[test]
fn c13_presences_present_is_ignored_not_invented() {
    let synthetic = chunk_frame(
        30,
        "294064825540280335",
        Some(0),
        Some(1),
        Some(list(vec![member("100")])),
        None,
        None,
        Some(list(vec![map(vec![
            ("user_id", text("100")),
            ("status", text("online")),
        ])])),
    );
    let (chunk, diagnostics) = parse(&synthetic);
    assert_eq!(chunk.members.len(), 1);
    assert!(
        diagnostics
            .ignored
            .iter()
            .any(|entry| entry == "d.presences")
    );
}

#[test]
fn c14_duplicate_member_last_wins_with_diagnostic() {
    let synthetic = chunk_frame(
        30,
        "294064825540280335",
        Some(0),
        Some(1),
        Some(list(vec![
            member_with_nick("100", "old"),
            member_with_nick("100", "new"),
        ])),
        None,
        None,
        None,
    );
    let (chunk, diagnostics) = parse(&synthetic);
    assert_eq!(chunk.members.len(), 1);
    assert_eq!(chunk.members[0].member.nick.as_deref(), Some("new"));
    assert!(
        diagnostics
            .duplicates
            .iter()
            .any(|entry| entry.contains("duplicate-member:100")),
        "ожидался duplicate-member:100, получено {:?}",
        diagnostics.duplicates
    );
}

#[test]
fn c17_empty_and_non_text_nonce_are_ignored() {
    for nonce in [text(""), int(5)] {
        let synthetic = chunk_frame(
            30,
            "294064825540280335",
            Some(0),
            Some(1),
            Some(list(vec![member("100")])),
            None,
            Some(nonce),
            None,
        );
        let (chunk, diagnostics) = parse(&synthetic);
        assert_eq!(chunk.nonce, None);
        assert!(diagnostics.ignored.iter().any(|entry| entry == "d.nonce"));
    }
}

#[test]
fn c18_not_found_wrong_container_is_ignored() {
    let synthetic = chunk_frame(
        30,
        "294064825540280335",
        Some(0),
        Some(1),
        Some(list(vec![member("100")])),
        Some(text("not-a-list")),
        None,
        None,
    );
    let (chunk, diagnostics) = parse(&synthetic);
    assert!(chunk.not_found.is_empty());
    assert!(
        diagnostics
            .ignored
            .iter()
            .any(|entry| entry == "d.not_found")
    );
}

#[test]
fn c19_missing_members_is_empty_without_diagnostics() {
    let synthetic = chunk_frame(
        30,
        "294064825540280335",
        Some(0),
        Some(1),
        None,
        None,
        None,
        None,
    );
    let (chunk, diagnostics) = parse(&synthetic);
    assert!(chunk.members.is_empty());
    assert!(diagnostics.ignored.is_empty());
    assert!(diagnostics.malformed.is_empty());
}

#[test]
fn c15_non_numeric_guild_id_is_invalid() {
    let synthetic = chunk_frame(
        30,
        "not-a-snowflake",
        Some(0),
        Some(1),
        Some(list(Vec::new())),
        None,
        None,
        None,
    );
    let error = parse_err(&synthetic);
    assert!(matches!(
        error,
        EventError::InvalidId { ref path, .. } if path == "d.guild_id"
    ));
}

#[test]
fn c16_root_reexports_are_available() {
    let _: Option<MemberChunkMember> = None;
    let _: Option<MemberChunk> = None;
}
