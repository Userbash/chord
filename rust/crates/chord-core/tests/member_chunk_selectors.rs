//! W16c / P2-05: селекторы чанков и presence-effect для видимого диапазона.
//!
//! `008` — реальный member list (17 items, 15 members, 2 groups); `022` — реальный chunk.
//! Presence-effect — чистая декларация `Effect::RequestPresence`, без сети.

mod support;

use chord_core::state::{
    Effect, EventPayload, MemberListId, MemberListRange, State, apply_frame,
};
use support::state_fixture::{int, load, map, text};

fn event(name: &str) -> chord_core::codec::etf::EtfValue {
    load(name)
}

fn ready_state() -> State {
    let event = chord_core::Event::from_frame(&event("002-READY.json")).expect("READY");
    let EventPayload::Ready(ready) = event.payload else {
        panic!("ожидался READY");
    };
    State::from_ready(&ready)
}

fn apply_named(state: State, name: &str) -> State {
    let (state, effects) = apply_frame(state, &event(name));
    assert!(effects.is_empty());
    state
}

fn chunk_frame(
    seq: u64,
    guild_id: &str,
    index: u32,
    count: u32,
    nonce: Option<&str>,
    id: &str,
) -> chord_core::codec::etf::EtfValue {
    let mut pairs = vec![
        ("guild_id", text(guild_id)),
        ("chunk_index", int(i64::from(index))),
        ("chunk_count", int(i64::from(count))),
        (
            "members",
            chord_core::codec::etf::EtfValue::List(vec![map(vec![
                (
                    "roles",
                    chord_core::codec::etf::EtfValue::Atom("nil".to_string()),
                ),
                (
                    "user",
                    map(vec![("id", text(id)), ("username", text("user"))]),
                ),
            ])]),
        ),
    ];
    if let Some(nonce) = nonce {
        pairs.push(("nonce", text(nonce)));
    }
    support::state_fixture::frame("GUILD_MEMBERS_CHUNK", seq, map(pairs))
}

fn chunk_state() -> State {
    apply_named(ready_state(), "022-GUILD_MEMBERS_CHUNK.json")
}

const GUILD: u64 = 294_064_825_540_280_335;

#[test]
fn s01_member_chunks_are_filtered_by_guild() {
    let state = apply_named(chunk_state(), "022-GUILD_MEMBERS_CHUNK.json");
    assert_eq!(state.member_chunks(GUILD).count(), 1);
    assert_eq!(state.member_chunks(999).count(), 0);
}

#[test]
fn s02_member_chunk_selects_by_nonce_and_none() {
    let mut state = chunk_state();
    state = apply_frame(
        state,
        &chunk_frame(30, "294064825540280335", 0, 1, Some("a"), "100"),
    )
    .0;
    assert!(state.member_chunk(GUILD, None).is_some());
    assert!(state.member_chunk(GUILD, Some("a")).is_some());
    assert!(state.member_chunk(GUILD, Some("b")).is_none());
}

#[test]
fn s03_is_complete_false_until_all_indices_arrive() {
    let mut state = apply_named(ready_state(), "022-GUILD_MEMBERS_CHUNK.json");
    state = apply_frame(
        state,
        &chunk_frame(30, "294064825540280335", 0, 2, Some("n"), "100"),
    )
    .0;
    assert!(!state.member_chunk_is_complete(GUILD, Some("n")));
    state = apply_frame(
        state,
        &chunk_frame(31, "294064825540280335", 1, 2, Some("n"), "101"),
    )
    .0;
    assert!(state.member_chunk_is_complete(GUILD, Some("n")));
    assert_eq!(state.member_chunk_len(GUILD, Some("n")), 2);
}

#[test]
fn s04_members_iterator_orders_by_chunk_index() {
    let mut state = apply_named(ready_state(), "022-GUILD_MEMBERS_CHUNK.json");
    state = apply_frame(
        state,
        &chunk_frame(30, "294064825540280335", 1, 2, Some("n"), "201"),
    )
    .0;
    state = apply_frame(
        state,
        &chunk_frame(31, "294064825540280335", 0, 2, Some("n"), "200"),
    )
    .0;
    assert_eq!(
        state
            .member_chunk_members(GUILD, Some("n"))
            .map(|member| member.member.user_id)
            .collect::<Vec<_>>(),
        vec![200, 201]
    );
    assert_eq!(state.member_chunk_member_count(GUILD, Some("n")), 2);
}

#[test]
fn s05_member_chunk_member_lookup() {
    let state = chunk_state();
    let member = state
        .member_chunk_member(GUILD, None, 159_985_870_458_322_944)
        .expect("MEE6");
    assert_eq!(member.user.username.as_deref(), Some("MEE6"));
    assert!(
        state
            .member_chunk_member(GUILD, None, 707_997_415_642_365_992)
            .is_none()
    );
}

#[test]
fn s06_not_found_is_flattened_in_chunk_order() {
    let state = chunk_state();
    assert_eq!(
        state.member_chunk_not_found(GUILD, None),
        vec![707_997_415_642_365_992]
    );
    assert!(state.member_chunk_not_found(999, None).is_empty());
}

#[test]
fn s07_presence_effect_full_range_skips_groups() {
    let state = apply_named(ready_state(), "008-GUILD_MEMBER_LIST_UPDATE.json");
    let effect = state
        .member_list_presence_effect(
            1_014_537_851_418_980_372,
            &MemberListId::new("1659606647"),
            MemberListRange { start: 0, end: 99 },
        )
        .expect("effect");
    assert_eq!(
        effect,
        Effect::RequestPresence {
            guild_id: 1_014_537_851_418_980_372,
            user_ids: vec![
                95_101_520_944_570_368,
                702_520_807_447_855_184,
                531_356_125_954_834_463,
                310_848_622_642_069_504,
                793_141_031_326_973_962,
                634_717_597_551_689_728,
                388_346_693_021_728_778,
                282_859_044_593_598_464,
                538_823_101_409_132_545,
                198_286_902_099_443_712,
                536_642_357_102_247_937,
                275_258_435_199_827_968,
                1_286_988_213_654_917_154,
                344_286_347_697_324_043,
                449_859_319_861_870_592,
            ],
        }
    );
}

#[test]
fn s08_presence_effect_subrange_and_only_groups() {
    let state = apply_named(ready_state(), "008-GUILD_MEMBER_LIST_UPDATE.json");
    let list_id = MemberListId::new("1659606647");
    let guild_id = 1_014_537_851_418_980_372;
    assert_eq!(
        state
            .member_list_presence_effect(guild_id, &list_id, MemberListRange { start: 2, end: 4 },),
        Some(Effect::RequestPresence {
            guild_id,
            user_ids: vec![702_520_807_447_855_184, 531_356_125_954_834_463],
        })
    );
    assert_eq!(
        state
            .member_list_presence_effect(guild_id, &list_id, MemberListRange { start: 0, end: 0 },),
        None
    );
}

#[test]
fn s09_presence_effect_missing_list_or_bad_range_is_none() {
    let state = ready_state();
    let list_id = MemberListId::new("1659606647");
    assert_eq!(
        state.member_list_presence_effect(
            1_014_537_851_418_980_372,
            &list_id,
            MemberListRange { start: 0, end: 5 },
        ),
        None
    );
    let state = apply_named(ready_state(), "008-GUILD_MEMBER_LIST_UPDATE.json");
    assert_eq!(
        state.member_list_presence_effect(
            1_014_537_851_418_980_372,
            &list_id,
            MemberListRange { start: 5, end: 2 },
        ),
        None
    );
    assert_eq!(
        state.member_list_presence_effect(
            1_014_537_851_418_980_372,
            &MemberListId::new("missing"),
            MemberListRange { start: 0, end: 5 },
        ),
        None
    );
}

#[test]
fn s10_member_chunk_count() {
    let state = chunk_state();
    assert_eq!(state.member_chunk_count(), 1);
    let state = apply_named(state, "022-GUILD_MEMBERS_CHUNK.json");
    assert_eq!(state.member_chunk_count(), 1);
}

#[test]
fn s11_unknown_nonce_has_empty_selectors() {
    let state = chunk_state();
    assert!(!state.member_chunk_is_complete(GUILD, Some("missing")));
    assert_eq!(state.member_chunk_len(GUILD, Some("missing")), 0);
    assert_eq!(state.member_chunk_member_count(GUILD, Some("missing")), 0);
    assert!(
        state
            .member_chunk_members(GUILD, Some("missing"))
            .next()
            .is_none()
    );
}
