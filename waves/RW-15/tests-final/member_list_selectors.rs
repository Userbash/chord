//! W15c / P2-04: read-only селекторы member list.

mod support;

use discord_core::codec::etf::EtfValue;
use discord_core::state::{EventPayload, MemberListId, MemberListRange, State, apply_frame};
use support::state_fixture::{frame, int, list, load, map, text};

fn state() -> State {
    let frame = support::state_fixture::minimal_ready_frame();
    let event = discord_core::Event::from_frame(&frame).expect("minimal READY");
    let EventPayload::Ready(ready) = event.payload else {
        panic!("ожидался READY");
    };
    State::from_ready(&ready)
}

fn apply_named(state: State, name: &str) -> State {
    apply(state, &load(name))
}

fn apply(state: State, frame: &EtfValue) -> State {
    let (next, effects) = apply_frame(state, frame);
    assert!(effects.is_empty());
    next
}

fn other_list_frame() -> EtfValue {
    frame(
        "GUILD_MEMBER_LIST_UPDATE",
        30,
        map(vec![
            ("guild_id", text("1014537851418980372")),
            ("id", text("other")),
            ("member_count", int(1)),
            ("online_count", int(1)),
            (
                "groups",
                list(vec![map(vec![("id", text("online")), ("count", int(1))])]),
            ),
            (
                "ops",
                list(vec![map(vec![
                    ("op", text("SYNC")),
                    ("range", text("\0c")),
                    (
                        "items",
                        list(vec![map(vec![(
                            "group",
                            map(vec![("id", text("online"))]),
                        )])]),
                    ),
                ])]),
            ),
        ]),
    )
}

fn everyone() -> MemberListId {
    MemberListId::new("everyone")
}

#[test]
fn s01_list_and_items_are_visible() {
    let state = apply_named(state(), "008-GUILD_MEMBER_LIST_UPDATE.json");
    let list = state
        .member_list(1_014_537_851_418_980_372, &MemberListId::new("1659606647"))
        .expect("список 008");
    assert_eq!(list.items.len(), 17);
    assert_eq!(
        state
            .member_list_items(1_014_537_851_418_980_372, &MemberListId::new("1659606647"))
            .len(),
        17
    );
    assert_eq!(
        state.member_list_len(1_014_537_851_418_980_372, &MemberListId::new("1659606647")),
        17
    );
}

#[test]
fn s02_member_at_and_index_of() {
    let state = apply_named(state(), "008-GUILD_MEMBER_LIST_UPDATE.json");
    let list_id = MemberListId::new("1659606647");
    let (member, user, presence) = state
        .member_list_member_at(1_014_537_851_418_980_372, &list_id, 1)
        .expect("member index 1");
    assert_eq!(member.user_id, 95_101_520_944_570_368);
    assert_eq!(user.username.as_deref(), Some("romanpony"));
    assert_eq!(presence.status.as_deref(), Some("online"));
    assert_eq!(
        state.member_list_index_of(1_014_537_851_418_980_372, &list_id, 702_520_807_447_855_184),
        Some(2)
    );
}

#[test]
fn s03_group_and_range() {
    let state = apply_named(state(), "008-GUILD_MEMBER_LIST_UPDATE.json");
    let list_id = MemberListId::new("1659606647");
    let group = state
        .member_list_group(1_014_537_851_418_980_372, &list_id, "1062745847512715385")
        .expect("role group");
    assert_eq!(group.count, 2);
    assert_eq!(
        state.member_list_range(1_014_537_851_418_980_372, &list_id),
        Some(MemberListRange { start: 0, end: 99 })
    );
}

#[test]
fn s04_is_loaded_before_and_after_sync() {
    let state = state();
    assert!(!state.member_list_is_loaded(294_064_825_540_280_335, &everyone()));
    let state = apply_named(state, "021-GUILD_MEMBER_LIST_UPDATE.json");
    assert!(state.member_list_is_loaded(294_064_825_540_280_335, &everyone()));
}

#[test]
fn s05_missing_list_is_empty_and_safe() {
    let state = state();
    assert!(state.member_list(1, &everyone()).is_none());
    assert!(state.member_list_items(1, &everyone()).is_empty());
    assert!(state.member_list_member_at(1, &everyone(), 0).is_none());
    assert!(state.member_list_index_of(1, &everyone(), 42).is_none());
    assert!(state.member_list_group(1, &everyone(), "online").is_none());
    assert_eq!(state.member_list_range(1, &everyone()), None);
    assert_eq!(state.member_list_len(1, &everyone()), 0);
    assert_eq!(state.member_list_count(), 0);
}

#[test]
fn s06_two_lists_across_guilds_are_independent() {
    let state = apply_named(state(), "008-GUILD_MEMBER_LIST_UPDATE.json");
    let state = apply_named(state, "021-GUILD_MEMBER_LIST_UPDATE.json");
    let a = MemberListId::new("1659606647");
    let b = everyone();
    assert_eq!(
        state.member_list_items(1_014_537_851_418_980_372, &a).len(),
        17
    );
    assert_eq!(
        state.member_list_items(294_064_825_540_280_335, &b).len(),
        100
    );
    assert_eq!(state.member_list_count(), 2);
}

#[test]
fn s07_group_entry_is_not_a_member() {
    let state = apply_named(state(), "008-GUILD_MEMBER_LIST_UPDATE.json");
    let list_id = MemberListId::new("1659606647");
    assert!(
        state
            .member_list_member_at(1_014_537_851_418_980_372, &list_id, 0)
            .is_none()
    );
    assert!(
        state
            .member_list_index_of(
                1_014_537_851_418_980_372,
                &list_id,
                1_062_745_847_512_715_385
            )
            .is_none()
    );
}

#[test]
fn s09_two_lists_of_same_guild_are_independent() {
    let state = apply_named(state(), "008-GUILD_MEMBER_LIST_UPDATE.json");
    let state = apply(state, &other_list_frame());
    let first = MemberListId::new("1659606647");
    let second = MemberListId::new("other");
    assert_eq!(
        state
            .member_list_items(1_014_537_851_418_980_372, &first)
            .len(),
        17
    );
    assert_eq!(
        state
            .member_list_items(1_014_537_851_418_980_372, &second)
            .len(),
        1
    );
    assert_eq!(state.member_list_count(), 2);
}

#[test]
fn s08_member_list_count_counts_lists() {
    let state = state();
    assert_eq!(state.member_list_count(), 0);
    let state = apply_named(state, "008-GUILD_MEMBER_LIST_UPDATE.json");
    assert_eq!(state.member_list_count(), 1);
    let state = apply_named(state, "021-GUILD_MEMBER_LIST_UPDATE.json");
    assert_eq!(state.member_list_count(), 2);
}
