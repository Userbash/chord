//! W16b / P2-05: сборка chunk-ов и аддитивный merge в состояние.
//!
//! `022` — реальный оракул (4 участника); out-of-order/duplicate/nonce — синтетика,
//! потому что в доступном capture только один chunk. Тесты падают до реализации.

mod support;

use discord_core::codec::etf::EtfValue;
use discord_core::ready::GuildMember;
use discord_core::state::{EventPayload, MemberChunkAssembly, State, apply_frame};
use support::state_fixture::{atom, frame, int, list, load, map, text};

fn event(name: &str) -> EtfValue {
    load(name)
}

fn ready_state() -> State {
    let event = discord_core::Event::from_frame(&event("002-READY.json")).expect("READY");
    let EventPayload::Ready(ready) = event.payload else {
        panic!("ожидался READY");
    };
    State::from_ready(&ready)
}

fn apply(state: State, frame: &EtfValue) -> State {
    let (state, effects) = apply_frame(state, frame);
    assert!(effects.is_empty(), "chunk не создаёт effects");
    state
}

fn user(id: &str) -> EtfValue {
    map(vec![("id", text(id)), ("username", text("user"))])
}

fn member(id: &str) -> EtfValue {
    map(vec![
        ("roles", atom("nil")),
        ("nick", atom("nil")),
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

fn chunk_frame(
    seq: u64,
    guild_id: &str,
    chunk_index: u32,
    chunk_count: u32,
    nonce: Option<&str>,
    members: Vec<EtfValue>,
) -> EtfValue {
    let mut pairs = vec![
        ("guild_id", text(guild_id)),
        ("chunk_index", int(i64::from(chunk_index))),
        ("chunk_count", int(i64::from(chunk_count))),
        ("members", list(members)),
    ];
    if let Some(nonce) = nonce {
        pairs.push(("nonce", text(nonce)));
    }
    frame("GUILD_MEMBERS_CHUNK", seq, map(pairs))
}

fn assembly<'a>(state: &'a State, guild_id: u64, nonce: Option<&str>) -> &'a MemberChunkAssembly {
    state
        .member_chunks
        .get(&(guild_id, nonce.map(str::to_string)))
        .unwrap_or_else(|| panic!("нет сборки {guild_id}/{nonce:?}"))
}

#[test]
fn r01_apply_022_stores_assembly_and_merges_members() {
    let mut state = ready_state();
    let guild_id = 294_064_825_540_280_335;
    let before_count = state.guilds[&guild_id].member_count;
    state = apply(state, &event("022-GUILD_MEMBERS_CHUNK.json"));

    let assembly = assembly(&state, guild_id, None);
    assert_eq!(assembly.chunk_count, 1);
    assert!(assembly.is_complete());
    assert_eq!(assembly.len(), 1);
    assert!(!assembly.is_empty());
    assert_eq!(assembly.members().count(), 4);
    assert!(format!("{state:?}").contains("member_chunks: 1"));

    let guild = &state.guilds[&guild_id];
    assert_eq!(guild.member_count, before_count);
    for id in [
        159_985_870_458_322_944_u64,
        465_601_078_914_711_572,
        566_215_798_759_096_322,
        875_330_208_104_853_504,
    ] {
        assert!(
            guild.members.iter().any(|member| member.user_id == id),
            "нет участника {id} в Guild.members"
        );
        assert!(state.users.contains_key(&id), "нет пользователя {id}");
    }
}

#[test]
fn r02_unknown_guild_is_diagnostic_but_assembly_is_stored() {
    let state = apply(
        ready_state(),
        &chunk_frame(30, "999", 0, 1, None, vec![member("100")]),
    );
    assert!(state.member_chunks.contains_key(&(999, None)));
    assert!(
        state
            .diagnostics
            .inconsistencies
            .iter()
            .any(|entry| entry == "guilds:999:member-chunk"),
        "ожидалась диагностика unknown guild, получено {:?}",
        state.diagnostics.inconsistencies
    );
}

#[test]
fn r03_out_of_order_chunks_are_complete_and_ordered_by_index() {
    let guild_id = 294_064_825_540_280_335;
    let mut state = apply(
        ready_state(),
        &chunk_frame(
            30,
            "294064825540280335",
            1,
            2,
            Some("n"),
            vec![member("201")],
        ),
    );
    assert!(!assembly(&state, guild_id, Some("n")).is_complete());
    state = apply(
        state,
        &chunk_frame(
            31,
            "294064825540280335",
            0,
            2,
            Some("n"),
            vec![member("200")],
        ),
    );
    let assembly = assembly(&state, guild_id, Some("n"));
    assert!(assembly.is_complete());
    assert_eq!(
        assembly
            .members()
            .map(|member| member.member.user_id)
            .collect::<Vec<_>>(),
        vec![200, 201]
    );
}

#[test]
fn r04_duplicate_index_last_wins_with_diagnostic() {
    let guild_id = 294_064_825_540_280_335;
    let mut state = apply(
        ready_state(),
        &chunk_frame(
            30,
            "294064825540280335",
            0,
            1,
            Some("n"),
            vec![member_with_nick("100", "old")],
        ),
    );
    state = apply(
        state,
        &chunk_frame(
            31,
            "294064825540280335",
            0,
            1,
            Some("n"),
            vec![member_with_nick("100", "new")],
        ),
    );
    let assembly = assembly(&state, guild_id, Some("n"));
    assert_eq!(assembly.len(), 1);
    assert_eq!(
        assembly.chunks[&0].members[0].member.nick.as_deref(),
        Some("new")
    );
    assert!(
        state
            .diagnostics
            .duplicates
            .iter()
            .any(|entry| entry.contains("duplicate-index:0")),
        "ожидался duplicate-index:0, получено {:?}",
        state.diagnostics.duplicates
    );
}

#[test]
fn r05_chunk_count_mismatch_is_diagnostic_and_first_wins() {
    let guild_id = 294_064_825_540_280_335;
    let mut state = apply(
        ready_state(),
        &chunk_frame(
            30,
            "294064825540280335",
            0,
            2,
            Some("n"),
            vec![member("100")],
        ),
    );
    state = apply(
        state,
        &chunk_frame(
            31,
            "294064825540280335",
            1,
            3,
            Some("n"),
            vec![member("101")],
        ),
    );
    let assembly = assembly(&state, guild_id, Some("n"));
    assert_eq!(assembly.chunk_count, 2);
    assert_eq!(assembly.len(), 1);
    assert!(
        state
            .diagnostics
            .inconsistencies
            .iter()
            .any(|entry| entry.contains("chunk-count:2->3")),
        "ожидался chunk-count conflict, получено {:?}",
        state.diagnostics.inconsistencies
    );
}

#[test]
fn r06_nonce_correlates_two_assemblies() {
    let guild_id = 294_064_825_540_280_335;
    let mut state = ready_state();
    state = apply(
        state,
        &chunk_frame(
            30,
            "294064825540280335",
            0,
            1,
            Some("a"),
            vec![member("100")],
        ),
    );
    state = apply(
        state,
        &chunk_frame(
            31,
            "294064825540280335",
            0,
            1,
            Some("b"),
            vec![member("200")],
        ),
    );
    assert_eq!(state.member_chunks.len(), 2);
    assert!(assembly(&state, guild_id, Some("a")).is_complete());
    assert!(assembly(&state, guild_id, Some("b")).is_complete());
}

#[test]
fn r07_not_found_is_stored_without_removing_members() {
    let guild_id = 294_064_825_540_280_335;
    let state = apply(ready_state(), &event("022-GUILD_MEMBERS_CHUNK.json"));
    let assembly = assembly(&state, guild_id, None);
    assert_eq!(assembly.chunks[&0].not_found, vec![707_997_415_642_365_992]);
    assert_eq!(assembly.members().count(), 4);
    assert!(
        !state.guilds[&guild_id]
            .members
            .iter()
            .any(|member| member.user_id == 707_997_415_642_365_992)
    );
}

#[test]
fn r08_stale_sequence_is_noop() {
    let mut state = apply(ready_state(), &event("022-GUILD_MEMBERS_CHUNK.json"));
    let before = state.member_chunks.len();
    state = apply(
        state,
        &chunk_frame(16, "294064825540280335", 0, 1, None, vec![member("100")]),
    );
    assert_eq!(state.member_chunks.len(), before);
    assert!(state.diagnostics.stale_sequences.contains(&16));
}

#[test]
fn r09_invalid_index_is_malformed_and_does_not_change_state() {
    let state = apply(
        ready_state(),
        &chunk_frame(30, "294064825540280335", 1, 1, None, vec![member("100")]),
    );
    assert!(state.member_chunks.is_empty());
    assert!(
        state
            .diagnostics
            .malformed
            .iter()
            .any(|entry| entry.starts_with("event:")),
        "ожидался malformed event, получено {:?}",
        state.diagnostics.malformed
    );
}

#[test]
fn r10_existing_guild_member_is_updated_not_duplicated() {
    let guild_id = 294_064_825_540_280_335;
    let mut state = ready_state();
    let guild = state.guilds.get_mut(&guild_id).expect("гильдия");
    guild.members.retain(|member| member.user_id != 100);
    guild.members.push(GuildMember {
        user_id: 100,
        nick: Some("old".to_string()),
        ..GuildMember::default()
    });

    state = apply(
        state,
        &chunk_frame(
            30,
            "294064825540280335",
            0,
            1,
            None,
            vec![member_with_nick("100", "new")],
        ),
    );
    let matching: Vec<&GuildMember> = state.guilds[&guild_id]
        .members
        .iter()
        .filter(|member| member.user_id == 100)
        .collect();
    assert_eq!(matching.len(), 1);
    assert_eq!(matching[0].nick.as_deref(), Some("new"));
}

#[test]
fn r12_single_chunk_without_nonce_is_new_response_without_duplicate() {
    let guild_id = 294_064_825_540_280_335;
    let mut state = apply(
        ready_state(),
        &chunk_frame(30, "294064825540280335", 0, 1, None, vec![member("100")]),
    );
    state = apply(
        state,
        &chunk_frame(31, "294064825540280335", 0, 1, None, vec![member("200")]),
    );
    let assembly = assembly(&state, guild_id, None);
    assert_eq!(assembly.len(), 1);
    assert_eq!(assembly.chunks[&0].members[0].member.user_id, 200);
    assert!(
        !state
            .diagnostics
            .duplicates
            .iter()
            .any(|entry| entry.contains("duplicate-index")),
        "повтор 0/1 без nonce — новый ответ, не duplicate: {:?}",
        state.diagnostics.duplicates
    );
    assert!(state.users.contains_key(&100));
    assert!(state.users.contains_key(&200));
}

#[test]
fn r13_uncorrelated_multi_chunk_duplicate_index_is_diagnostic() {
    let guild_id = 294_064_825_540_280_335;
    let mut state = apply(
        ready_state(),
        &chunk_frame(30, "294064825540280335", 0, 2, None, vec![member("100")]),
    );
    state = apply(
        state,
        &chunk_frame(31, "294064825540280335", 0, 2, None, vec![member("200")]),
    );
    let assembly = assembly(&state, guild_id, None);
    assert_eq!(assembly.len(), 1);
    assert_eq!(assembly.chunks[&0].members[0].member.user_id, 200);
    assert!(
        state
            .diagnostics
            .duplicates
            .iter()
            .any(|entry| entry.contains("none:duplicate-index:0")),
        "nonce=None/count>1: повтор индекса обязан быть диагностирован: {:?}",
        state.diagnostics.duplicates
    );
}

#[test]
fn r11_two_guilds_have_independent_assemblies() {
    let state = apply(
        ready_state(),
        &chunk_frame(30, "999", 0, 1, None, vec![member("100")]),
    );
    let state = apply(
        state,
        &chunk_frame(31, "294064825540280335", 0, 1, None, vec![member("200")]),
    );
    assert_eq!(state.member_chunks.len(), 2);
    assert!(state.member_chunks.contains_key(&(999, None)));
    assert!(
        state
            .member_chunks
            .contains_key(&(294_064_825_540_280_335, None))
    );
}
