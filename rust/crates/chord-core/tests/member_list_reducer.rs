//! W15b / P2-04: reducer member list ops на реальных фикстурах и синтетике.
//!
//! Ops и counts — `jq`-оракулы `waves/RW-15/RESEARCH.md` §2; 005/006 сюда не попадают:
//! у них legacy-отрицательный `guild_id`, политика зафиксирована в `member_list_types`.

mod support;

use chord_core::codec::etf::EtfValue;
use chord_core::state::{
    EventPayload, MemberList, MemberListEntry, MemberListId, MemberListRange, State, apply_frame,
};
use support::state_fixture::{atom, frame, int, list, load, map, text};

fn event(name: &str) -> EtfValue {
    load(name)
}

fn apply_named(state: State, name: &str) -> State {
    let (state, effects) = apply_frame(state, &event(name));
    assert!(effects.is_empty(), "{name}: member list не создаёт effects");
    state
}

fn minimal_state() -> State {
    let frame = support::state_fixture::minimal_ready_frame();
    let event = chord_core::Event::from_frame(&frame).expect("minimal READY");
    let EventPayload::Ready(ready) = event.payload else {
        panic!("ожидался READY");
    };
    State::from_ready(&ready)
}

fn ready_state() -> State {
    let event = chord_core::Event::from_frame(&event("002-READY.json")).expect("READY");
    let EventPayload::Ready(ready) = event.payload else {
        panic!("ожидался READY");
    };
    State::from_ready(&ready)
}

fn mlist<'a>(state: &'a State, guild_id: u64, id: &str) -> &'a MemberList {
    state
        .member_lists
        .get(&(guild_id, MemberListId::new(id)))
        .unwrap_or_else(|| panic!("нет списка {guild_id}/{id}"))
}

fn member_id(entry: &MemberListEntry) -> Option<u64> {
    match entry {
        MemberListEntry::Member { member, .. } => Some(member.user_id),
        MemberListEntry::Group { .. } => None,
    }
}

fn group(id: &str, count: i64) -> EtfValue {
    map(vec![("id", text(id)), ("count", int(count))])
}

fn group_item(id: &str) -> EtfValue {
    map(vec![("group", map(vec![("id", text(id))]))])
}

fn member_item(id: &str) -> EtfValue {
    map(vec![(
        "member",
        map(vec![
            ("roles", atom("nil")),
            ("presence", atom("nil")),
            (
                "user",
                map(vec![("id", text(id)), ("username", text("user"))]),
            ),
        ]),
    )])
}

fn member_list_frame(
    seq: u64,
    guild_id: &str,
    list_id: &str,
    groups: Vec<EtfValue>,
    ops: Vec<EtfValue>,
    member_count: i64,
    online_count: i64,
) -> EtfValue {
    frame(
        "GUILD_MEMBER_LIST_UPDATE",
        seq,
        map(vec![
            ("guild_id", text(guild_id)),
            ("id", text(list_id)),
            ("member_count", int(member_count)),
            ("online_count", int(online_count)),
            ("groups", list(groups)),
            ("ops", list(ops)),
        ]),
    )
}

fn sync(items: Vec<EtfValue>, range: Option<&str>) -> EtfValue {
    let mut pairs = vec![("op", text("SYNC")), ("items", list(items))];
    if let Some(range) = range {
        pairs.push(("range", text(range)));
    }
    map(pairs)
}

fn insert(index: i64, item: EtfValue) -> EtfValue {
    map(vec![
        ("op", text("INSERT")),
        ("index", int(index)),
        ("item", item),
    ])
}

fn update_op(index: i64, item: EtfValue) -> EtfValue {
    map(vec![
        ("op", text("UPDATE")),
        ("index", int(index)),
        ("item", item),
    ])
}

fn delete(index: i64) -> EtfValue {
    map(vec![("op", text("DELETE")), ("index", int(index))])
}

/// R1: SYNC 008 создаёт список `(guild, 1659606647)` с 17 items и range.
#[test]
fn m01_sync_008_creates_member_list() {
    let state = apply_named(minimal_state(), "008-GUILD_MEMBER_LIST_UPDATE.json");
    assert_eq!(state.member_lists.len(), 1);
    let list = mlist(&state, 1_014_537_851_418_980_372, "1659606647");
    assert_eq!(list.items.len(), 17);
    assert_eq!(list.groups.len(), 2);
    assert_eq!(list.member_count, Some(24_228));
    assert_eq!(list.online_count, Some(681));
    assert_eq!(list.range, Some(MemberListRange { start: 0, end: 99 }));
    assert_eq!(
        list.items[0],
        MemberListEntry::Group {
            id: "1062745847512715385".to_string()
        }
    );
    assert_eq!(member_id(&list.items[1]), Some(95_101_520_944_570_368));
}

/// R2: два list_id разных гильдий живут независимо (same-guild — m16).
#[test]
fn m02_two_lists_across_guilds_do_not_overwrite_each_other() {
    let state = apply_named(minimal_state(), "008-GUILD_MEMBER_LIST_UPDATE.json");
    let state = apply_named(state, "021-GUILD_MEMBER_LIST_UPDATE.json");
    assert_eq!(state.member_lists.len(), 2);
    assert_eq!(
        mlist(&state, 1_014_537_851_418_980_372, "1659606647")
            .items
            .len(),
        17
    );
    assert_eq!(
        mlist(&state, 294_064_825_540_280_335, "everyone")
            .items
            .len(),
        100
    );
}

/// R3: SYNC 021 — 100 items, 4 groups.
#[test]
fn m03_sync_021_groups_and_counts() {
    let state = apply_named(minimal_state(), "021-GUILD_MEMBER_LIST_UPDATE.json");
    let list = mlist(&state, 294_064_825_540_280_335, "everyone");
    assert_eq!(list.items.len(), 100);
    assert_eq!(list.groups.len(), 4);
    assert_eq!(list.member_count, Some(112));
    assert_eq!(list.online_count, Some(13));
}

/// R4: INSERT 034 после 008 сдвигает хвост.
#[test]
fn m04_insert_034_after_sync() {
    let state = apply_named(minimal_state(), "008-GUILD_MEMBER_LIST_UPDATE.json");
    let state = apply_named(state, "034-GUILD_MEMBER_LIST_UPDATE.json");
    let list = mlist(&state, 1_014_537_851_418_980_372, "1659606647");
    assert_eq!(list.items.len(), 18);
    assert_eq!(member_id(&list.items[1]), Some(294_857_804_404_228_106));
    assert_eq!(member_id(&list.items[2]), Some(95_101_520_944_570_368));
    assert_eq!(member_id(&list.items[3]), Some(702_520_807_447_855_184));
}

/// R5: DELETE 075 после 008+034 удаляет index 11 и сдвигает хвост.
#[test]
fn m05_delete_075_after_sync_and_insert() {
    let state = apply_named(minimal_state(), "008-GUILD_MEMBER_LIST_UPDATE.json");
    let state = apply_named(state, "034-GUILD_MEMBER_LIST_UPDATE.json");
    let before = mlist(&state, 1_014_537_851_418_980_372, "1659606647");
    assert_eq!(before.items.len(), 18);
    let removed = member_id(&before.items[11]).expect("на index 11 member");
    let state = apply_named(state, "075-GUILD_MEMBER_LIST_UPDATE.json");
    let after = mlist(&state, 1_014_537_851_418_980_372, "1659606647");
    assert_eq!(after.items.len(), 17);
    assert_ne!(member_id(&after.items[11]), Some(removed));
    assert_eq!(after.range, Some(MemberListRange { start: 0, end: 99 }));
}

/// R6: DELETE+INSERT 045 после 021 — обе ops по порядку, range не меняется.
#[test]
fn m06_delete_insert_045_after_sync() {
    let state = apply_named(minimal_state(), "021-GUILD_MEMBER_LIST_UPDATE.json");
    let state = apply_named(state, "045-GUILD_MEMBER_LIST_UPDATE.json");
    let list = mlist(&state, 294_064_825_540_280_335, "everyone");
    assert_eq!(list.items.len(), 100);
    assert_eq!(member_id(&list.items[17]), Some(270_232_759_388_078_082));
    assert_eq!(list.range, Some(MemberListRange { start: 0, end: 99 }));
}

/// R7: group INSERT 079 — 5 groups, group на index 0, длина 100.
#[test]
fn m07_group_insert_079() {
    let state = apply_named(minimal_state(), "021-GUILD_MEMBER_LIST_UPDATE.json");
    let state = apply_named(state, "079-GUILD_MEMBER_LIST_UPDATE.json");
    let list = mlist(&state, 294_064_825_540_280_335, "everyone");
    assert_eq!(list.groups.len(), 5);
    assert_eq!(list.items.len(), 100);
    assert_eq!(
        list.items[0],
        MemberListEntry::Group {
            id: "294152150374940673".to_string()
        }
    );
}

/// R8: второй SYNC полностью заменяет items/groups/range.
#[test]
fn m08_second_sync_resets_list() {
    let state = apply_named(minimal_state(), "021-GUILD_MEMBER_LIST_UPDATE.json");
    let reset = member_list_frame(
        100,
        "294064825540280335",
        "everyone",
        vec![group("offline", 1)],
        vec![sync(vec![group_item("offline")], Some("\0\u{1}"))],
        1,
        0,
    );
    let (state, effects) = apply_frame(state, &reset);
    assert!(effects.is_empty());
    let list = mlist(&state, 294_064_825_540_280_335, "everyone");
    assert_eq!(list.items.len(), 1);
    assert_eq!(list.groups.len(), 1);
    assert_eq!(list.range, Some(MemberListRange { start: 0, end: 1 }));
    assert_eq!(list.member_count, Some(1));
}

/// R9: UPDATE 074 без базы — список создаётся, out-of-range диагностируется.
#[test]
fn m09_update_out_of_range_creates_stub() {
    let frame = member_list_frame(
        2,
        "294064825540280335",
        "everyone",
        vec![group("online", 1)],
        vec![update_op(74, group_item("online"))],
        112,
        13,
    );
    let (state, effects) = apply_frame(minimal_state(), &frame);
    assert!(effects.is_empty());
    let list = mlist(&state, 294_064_825_540_280_335, "everyone");
    assert!(list.items.is_empty());
    assert!(
        state
            .diagnostics
            .inconsistencies
            .iter()
            .any(|entry| entry.contains("out-of-range"))
    );
}

/// R10: duplicate member-id применяется буквально и диагностируется.
#[test]
fn m10_duplicate_member_is_diagnostic() {
    let first = member_list_frame(
        2,
        "294064825540280335",
        "everyone",
        vec![group("online", 1)],
        vec![sync(vec![member_item("42")], None)],
        1,
        1,
    );
    let state = apply_named_frame(minimal_state(), first);
    let duplicate = member_list_frame(
        3,
        "294064825540280335",
        "everyone",
        vec![group("online", 2)],
        vec![insert(1, member_item("42"))],
        2,
        2,
    );
    let (state, _) = apply_frame(state, &duplicate);
    let list = mlist(&state, 294_064_825_540_280_335, "everyone");
    assert_eq!(list.items.len(), 2);
    assert!(
        state
            .diagnostics
            .duplicates
            .iter()
            .any(|entry| entry.contains("duplicate:42"))
    );
}

fn apply_named_frame(state: State, frame: EtfValue) -> State {
    let (state, effects) = apply_frame(state, &frame);
    assert!(effects.is_empty());
    state
}

/// R15 (P2-1): INSERT в позицию существующего member-id сдвигает его и создаёт дубликат.
#[test]
fn m15_insert_duplicate_at_same_position_is_diagnostic() {
    let first = member_list_frame(
        2,
        "294064825540280335",
        "everyone",
        vec![group("online", 1)],
        vec![sync(vec![member_item("42")], None)],
        1,
        1,
    );
    let state = apply_named_frame(minimal_state(), first);
    let duplicate = member_list_frame(
        3,
        "294064825540280335",
        "everyone",
        vec![group("online", 2)],
        vec![insert(0, member_item("42"))],
        2,
        2,
    );
    let (state, _) = apply_frame(state, &duplicate);
    let list = mlist(&state, 294_064_825_540_280_335, "everyone");
    assert_eq!(list.items.len(), 2);
    assert!(
        state
            .diagnostics
            .duplicates
            .iter()
            .any(|entry| entry.contains("duplicate:42")),
        "диагностика: {:?}",
        state.diagnostics
    );
}

/// R16: два list_id одной и той же гильдии не смешиваются.
#[test]
fn m16_two_lists_of_same_guild_are_independent() {
    let state = apply_named(minimal_state(), "008-GUILD_MEMBER_LIST_UPDATE.json");
    let second = member_list_frame(
        30,
        "1014537851418980372",
        "other",
        vec![group("online", 1)],
        vec![sync(vec![group_item("online")], Some("\0\u{1}"))],
        1,
        1,
    );
    let state = apply_named_frame(state, second);
    assert_eq!(state.member_lists.len(), 2);
    assert_eq!(
        mlist(&state, 1_014_537_851_418_980_372, "1659606647")
            .items
            .len(),
        17
    );
    let other = mlist(&state, 1_014_537_851_418_980_372, "other");
    assert_eq!(other.items.len(), 1);
    assert_eq!(other.range, Some(MemberListRange { start: 0, end: 1 }));
}

/// R17: успешный UPDATE заменяет элемент без сдвига.
#[test]
fn m17_update_in_range_replaces_without_shift() {
    let state = apply_named(minimal_state(), "008-GUILD_MEMBER_LIST_UPDATE.json");
    let update = member_list_frame(
        30,
        "1014537851418980372",
        "1659606647",
        vec![group("online", 13), group("offline", 4)],
        vec![update_op(1, member_item("42"))],
        24_228,
        681,
    );
    let state = apply_named_frame(state, update);
    let list = mlist(&state, 1_014_537_851_418_980_372, "1659606647");
    assert_eq!(list.items.len(), 17);
    assert_eq!(member_id(&list.items[1]), Some(42));
    assert_eq!(member_id(&list.items[2]), Some(702_520_807_447_855_184));
    assert_eq!(list.range, Some(MemberListRange { start: 0, end: 99 }));
    assert!(state.diagnostics.duplicates.is_empty());
}

/// R11: out-of-range DELETE/UPDATE не меняют данные; INSERT 0 в пустой допустим.
#[test]
fn m11_out_of_range_ops_and_insert_into_empty() {
    let frame = member_list_frame(
        2,
        "294064825540280335",
        "everyone",
        Vec::new(),
        vec![
            delete(5),
            update_op(5, group_item("online")),
            insert(5, group_item("offline")),
            insert(0, group_item("online")),
        ],
        0,
        0,
    );
    let (state, _) = apply_frame(minimal_state(), &frame);
    let list = mlist(&state, 294_064_825_540_280_335, "everyone");
    assert_eq!(list.items.len(), 1);
    assert_eq!(
        state
            .diagnostics
            .inconsistencies
            .iter()
            .filter(|entry| entry.contains("out-of-range"))
            .count(),
        3
    );
}

/// R12: stale `s` — no-op и диагностика, list не меняется.
#[test]
fn m12_stale_sequence_is_ignored() {
    let state = apply_named(minimal_state(), "008-GUILD_MEMBER_LIST_UPDATE.json");
    let (state, effects) = apply_frame(state, &event("008-GUILD_MEMBER_LIST_UPDATE.json"));
    assert!(effects.is_empty());
    assert_eq!(
        mlist(&state, 1_014_537_851_418_980_372, "1659606647")
            .items
            .len(),
        17
    );
    assert!(state.diagnostics.stale_sequences.contains(&6));
}

/// R13: неизвестная гильдия — inconsistency, список всё равно создаётся.
#[test]
fn m13_unknown_guild_is_inconsistency() {
    let frame = member_list_frame(
        2,
        "42",
        "everyone",
        vec![group("online", 1)],
        vec![sync(vec![group_item("online")], None)],
        1,
        1,
    );
    let (state, _) = apply_frame(minimal_state(), &frame);
    assert!(
        state
            .member_lists
            .contains_key(&(42, MemberListId::new("everyone")))
    );
    assert!(
        state
            .diagnostics
            .inconsistencies
            .iter()
            .any(|entry| entry.contains("guilds:42:member-list"))
    );
}

/// R14: member list не трогает messages/users/Guild.member_count.
#[test]
fn m14_member_list_has_no_cross_state_side_effects() {
    let state = ready_state();
    let before_users = state.users.len();
    let before_messages = state.messages.len();
    let before_count = state.guilds[&294_064_825_540_280_335].member_count;
    let state = apply_named(state, "021-GUILD_MEMBER_LIST_UPDATE.json");
    assert_eq!(state.users.len(), before_users);
    assert_eq!(state.messages.len(), before_messages);
    assert_eq!(
        state.guilds[&294_064_825_540_280_335].member_count,
        before_count
    );
    assert_eq!(state.member_lists.len(), 1);
}
