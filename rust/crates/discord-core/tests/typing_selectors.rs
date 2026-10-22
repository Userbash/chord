//! W20c / P2-11c: селекторы typing-store.
//!
//! Оракул — 28 реальных `TYPING_START` (`tests/data/typing-run2.jsonl`) и
//! замороженный контракт `waves/RW-20/DESIGN-P2-11.md` §7. Селекторы только
//! читают: просроченные записи скрываются, но не удаляются.
//!
//! Тесты падают до объявления `state::typing` — compile-red.

mod support;

use discord_core::ready::User;
use discord_core::state::typing::parser::parse_typing_start;
use discord_core::state::typing::reducer::apply_start;
use discord_core::state::typing::types::TypingStart;
use discord_core::state::{Event, EventPayload, State};
use support::ready_fixture::parse_json;
use support::state_fixture::minimal_ready_frame;

const FIXTURE: &str = include_str!("data/typing-run2.jsonl");

const GUILD: u64 = 942_355_007_271_997_450;
const CHANNEL_A: u64 = 942_355_562_958_573_628;
const USER_A: u64 = 1_301_880_740_245_405_707;
const CHANNEL_B: u64 = 1_434_201_417_467_433_001;
const USER_B: u64 = 367_288_240_207_822_858;

fn minimal_state() -> State {
    let event = Event::from_frame(&minimal_ready_frame()).expect("minimal READY");
    let EventPayload::Ready(ready) = event.payload else {
        panic!("ожидался READY");
    };
    State::from_ready(&ready)
}

fn user(id: u64, username: &str) -> User {
    User {
        id,
        username: Some(username.to_string()),
        ..User::default()
    }
}

fn start(channel_id: u64, user_id: u64, received_at_ms: u64, guild_id: Option<u64>) -> TypingStart {
    TypingStart {
        channel_id,
        user_id,
        guild_id,
        wire_timestamp: None,
        member_user: None,
        received_at_ms: Some(received_at_ms),
    }
}

fn start_with_user(
    channel_id: u64,
    received_at_ms: u64,
    guild_id: Option<u64>,
    user: User,
) -> TypingStart {
    TypingStart {
        channel_id,
        user_id: user.id,
        guild_id,
        wire_timestamp: None,
        member_user: Some(user),
        received_at_ms: Some(received_at_ms),
    }
}

/// t11: пустое состояние; self entry; границы `expires_at`.
#[test]
fn t11_empty_and_self_boundary() {
    let state = minimal_state();
    assert!(state.typing_entry(500, 100, 1_000_000).is_none());
    assert!(!state.is_typing(500, 100, 1_000_000));
    assert!(state.typing_user_ids(500, 1_000_000).is_empty());
    assert!(state.typing_entries(500, 1_000_000).is_empty());
    assert_eq!(state.typing_entry_count(1_000_000), 0);
    assert_eq!(state.typing_channel_count(1_000_000), 0);
    assert_eq!(state.typing_remaining_ms(500, 100, 1_000_000), None);

    let mut state = minimal_state();
    apply_start(
        &mut state,
        start_with_user(500, 1_000_000, None, user(100, "self")),
    );

    assert!(state.typing_entry(500, 100, 1_009_999).is_some());
    assert!(state.is_typing(500, 100, 1_009_999));
    assert_eq!(state.typing_remaining_ms(500, 100, 1_009_999), Some(1));
    assert_eq!(
        state
            .typing_entry(500, 100, 1_009_999)
            .map(|entry| entry.remaining_ms(1_009_999)),
        Some(1)
    );

    // Граница: `now == expires_at_ms` — запись неактивна.
    assert!(state.typing_entry(500, 100, 1_010_000).is_none());
    assert!(!state.is_typing(500, 100, 1_010_000));
    assert_eq!(state.typing_remaining_ms(500, 100, 1_010_000), None);
    assert_eq!(
        state
            .typing
            .get(&(500, 100))
            .map(|entry| entry.remaining_ms(1_010_000)),
        Some(0)
    );
}

/// t12: несколько пользователей — только активные, ids/users по возрастанию.
#[test]
fn t12_users_sorted_and_expired_hidden() {
    let mut state = minimal_state();
    apply_start(
        &mut state,
        start_with_user(10, 1_000_000, None, user(300, "user-300")),
    );
    apply_start(
        &mut state,
        start_with_user(10, 1_005_000, None, user(200, "user-200")),
    );
    apply_start(
        &mut state,
        start_with_user(10, 990_000, None, user(400, "user-400")),
    );

    let now = 1_006_000;
    assert_eq!(state.typing_user_ids(10, now), vec![200, 300]);
    let users = state.typing_users(10, now);
    assert_eq!(
        users.iter().map(|user| user.id).collect::<Vec<_>>(),
        vec![200, 300]
    );
    assert_eq!(
        users
            .iter()
            .map(|user| user.username.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("user-200"), Some("user-300")]
    );
    assert_eq!(
        state
            .typing_entries(10, now)
            .iter()
            .map(|entry| entry.user_id)
            .collect::<Vec<_>>(),
        vec![200, 300]
    );
    assert_eq!(state.typing_entry_count(now), 2);
}

/// t13: разделение по гильдиям; private (`None`) не попадает ни в одну.
#[test]
fn t13_guild_split() {
    let mut state = minimal_state();
    apply_start(&mut state, start(10, 1, 1_000_000, Some(100)));
    apply_start(&mut state, start(10, 2, 1_000_000, Some(100)));
    apply_start(&mut state, start(11, 1, 1_000_000, Some(200)));
    apply_start(&mut state, start(12, 5, 1_000_000, None));

    let now = 1_001_000;
    assert_eq!(
        state
            .typing_entries_by_guild(100, now)
            .iter()
            .map(|entry| (entry.channel_id, entry.user_id))
            .collect::<Vec<_>>(),
        vec![(10, 1), (10, 2)]
    );
    assert_eq!(
        state
            .typing_entries_by_guild(200, now)
            .iter()
            .map(|entry| (entry.channel_id, entry.user_id))
            .collect::<Vec<_>>(),
        vec![(11, 1)]
    );
    assert!(state.typing_entries_by_guild(300, now).is_empty());
    assert!(
        state
            .typing_entries_by_guild(100, now)
            .iter()
            .all(|entry| entry.guild_id == Some(100))
    );
}

/// t14: активные счётчики, уникальные каналы, граница `now == expires`.
#[test]
fn t14_counts_and_expiry_boundary() {
    let mut state = minimal_state();
    apply_start(&mut state, start(10, 1, 1_000_000, None));
    apply_start(&mut state, start(10, 2, 1_000_000, None));
    apply_start(&mut state, start(11, 3, 1_005_000, None));
    apply_start(&mut state, start(11, 4, 990_000, None));

    let now = 1_006_000;
    assert_eq!(state.typing_entry_count(now), 3);
    assert_eq!(state.typing_channel_count(now), 2);
    assert_eq!(state.typing_entries(10, now).len(), 2);
    assert_eq!(state.typing_entries(11, now).len(), 1);

    // На границе истекают записи канала 10 (expires 1 010 000).
    assert!(!state.is_typing(10, 1, 1_010_000));
    assert!(!state.is_typing(10, 2, 1_010_000));
    assert!(state.is_typing(11, 3, 1_010_000));
    assert_eq!(state.typing_entry_count(1_010_000), 1);
    assert_eq!(state.typing_channel_count(1_010_000), 1);
    assert_eq!(state.typing_entries(10, 1_010_000).len(), 0);
    // Просроченная запись остаётся в store: селекторы её только скрывают.
    assert_eq!(state.typing.len(), 4);
}

/// t15: 28 fixture — активные селекторы и сохранённые stale-записи.
#[test]
fn t15_fixture_final_state() {
    let payloads: Vec<_> = FIXTURE
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| parse_json(line).expect("строка fixture — валидный JSON"))
        .collect();
    assert_eq!(payloads.len(), 28);

    let mut state = minimal_state();
    let mut last_now = 0;
    for payload in &payloads {
        let wire_timestamp = payload
            .get("timestamp")
            .and_then(discord_core::ready::value::integer)
            .expect("timestamp fixture");
        let now = u64::try_from(wire_timestamp * 1_000).expect("ms");
        let (parsed, _) = parse_typing_start(payload, Some(now)).expect("fixture START");
        apply_start(&mut state, parsed);
        last_now = now;
    }

    // Store: обе пары сохранены (старая не чистится лениво), 2 канала, 1 гильдия.
    assert_eq!(state.typing.len(), 2);
    assert_eq!(
        state
            .typing
            .keys()
            .map(|(channel_id, _)| *channel_id)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        2
    );
    assert!(
        state
            .typing
            .values()
            .all(|entry| entry.guild_id == Some(GUILD))
    );

    // Селекторы на финальном `now`: активна только последняя пара B.
    assert_eq!(state.typing_entries(CHANNEL_B, last_now).len(), 1);
    assert_eq!(state.typing_entry_count(last_now), 1);
    assert_eq!(state.typing_channel_count(last_now), 1);
    assert_eq!(
        state.typing_entries_by_guild(GUILD, last_now).len(),
        1,
        "DESIGN §7: просроченная пара A скрыта селекторами"
    );
    assert!(state.is_typing(CHANNEL_B, USER_B, last_now));
    assert!(!state.is_typing(CHANNEL_A, USER_A, last_now));

    // В `State.users` только self: неизвестные пользователи не выдумываются.
    assert!(state.typing_users(CHANNEL_B, last_now).is_empty());
    assert!(state.typing_users(CHANNEL_A, 1_792_392_992_000).is_empty());
}
