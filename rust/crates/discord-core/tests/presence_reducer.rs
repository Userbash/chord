//! W18b / P2-09b: reducer presence, supplemental, member list и chunk.
//!
//! Оракул — фикстуры `010`, `012`, `003`, `008`, `022` и замороженный контракт
//! `waves/RW-18/DESIGN-P2-09.md` §5. Синтетика проверяет offline/scope правила.
//!
//! Тесты падают до реализации presence-reducer — compile-red.

mod support;

use std::collections::BTreeSet;

use discord_core::codec::etf::EtfValue;
use discord_core::state::{
    Event, EventPayload, PresenceScope, PresenceStatus, State, StateDiagnostics, apply, apply_frame,
};
use support::state_fixture::{atom, frame, int, list, load, map, minimal_ready_frame, text};

fn minimal_state() -> State {
    let event = Event::from_frame(&minimal_ready_frame()).expect("minimal READY");
    let EventPayload::Ready(ready) = event.payload else {
        panic!("ожидался READY");
    };
    State::from_ready(&ready)
}

fn apply_named(state: State, name: &str) -> State {
    let (state, _) = apply_frame(state, &load(name));
    state
}

fn scope_count(state: &State, user_id: u64) -> usize {
    state
        .presences
        .keys()
        .filter(|(user, _)| *user == user_id)
        .count()
}

fn presence_for(
    state: &State,
    user_id: u64,
    scope: PresenceScope,
) -> Option<&discord_core::Presence> {
    state.presences.get(&(user_id, scope))
}

fn presence_frame(
    seq: u64,
    user_id: &str,
    guild_id: Option<&str>,
    status: &str,
    processed: i64,
    activities: Option<EtfValue>,
    hidden_activities: Option<EtfValue>,
) -> EtfValue {
    let mut pairs = vec![
        (
            "user",
            map(vec![("id", text(user_id)), ("username", text("user"))]),
        ),
        ("status", text(status)),
        ("processed_at_timestamp", int(processed)),
    ];
    if let Some(guild_id) = guild_id {
        pairs.push(("guild_id", text(guild_id)));
    }
    if let Some(activities) = activities {
        pairs.push(("activities", activities));
    }
    if let Some(hidden_activities) = hidden_activities {
        pairs.push(("hidden_activities", hidden_activities));
    }
    frame("PRESENCE_UPDATE", seq, map(pairs))
}

fn note_activity(name: &str) -> EtfValue {
    map(vec![
        ("name", text(name)),
        ("type", int(0)),
        ("created_at", text("100")),
    ])
}

fn member_list_frame(guild_id: &str, ops: Vec<EtfValue>) -> EtfValue {
    frame(
        "GUILD_MEMBER_LIST_UPDATE",
        50,
        map(vec![
            ("guild_id", text(guild_id)),
            ("id", text("everyone")),
            ("member_count", int(1)),
            ("online_count", int(1)),
            (
                "groups",
                list(vec![map(vec![("id", text("online")), ("count", int(1))])]),
            ),
            ("ops", list(ops)),
        ]),
    )
}

fn member_entry(user_id: &str, status: &str) -> EtfValue {
    map(vec![(
        "member",
        map(vec![
            ("roles", atom("nil")),
            (
                "presence",
                map(vec![
                    ("status", text(status)),
                    ("client_status", map(vec![("desktop", text(status))])),
                    ("activities", list(vec![note_activity("game")])),
                    ("processed_at_timestamp", int(1)),
                ]),
            ),
            (
                "user",
                map(vec![("id", text(user_id)), ("username", text("user"))]),
            ),
        ]),
    )])
}

fn chunk_frame(guild_id: &str, presences: Option<EtfValue>) -> EtfValue {
    let mut pairs = vec![
        ("guild_id", text(guild_id)),
        ("chunk_index", int(0)),
        ("chunk_count", int(1)),
        ("members", list(Vec::new())),
        ("not_found", atom("nil")),
    ];
    if let Some(presences) = presences {
        pairs.push(("presences", presences));
    }
    frame("GUILD_MEMBERS_CHUNK", 60, map(pairs))
}

/// r01: `010` — presence и user применяются, effects пусты.
#[test]
fn r01_live_update_applies_presence_and_user() {
    let (state, effects) = apply_frame(minimal_state(), &load("010-PRESENCE_UPDATE.json"));
    let presence = presence_for(
        &state,
        95_101_520_944_570_368,
        PresenceScope::Guild(1_014_537_851_418_980_372),
    )
    .expect("presence");
    assert_eq!(presence.status, PresenceStatus::Online);
    assert_eq!(presence.activities[0].name, "bg-capture");
    assert!(state.users.contains_key(&95_101_520_944_570_368));
    assert!(effects.is_empty());
}

/// r02: `010` → `062` — last wins для одного scope.
#[test]
fn r02_live_update_last_wins() {
    let state = apply_named(minimal_state(), "010-PRESENCE_UPDATE.json");
    let state = apply_named(state, "062-PRESENCE_UPDATE.json");
    assert_eq!(scope_count(&state, 95_101_520_944_570_368), 1);
    let presence = presence_for(
        &state,
        95_101_520_944_570_368,
        PresenceScope::Guild(1_014_537_851_418_980_372),
    )
    .expect("presence");
    assert_eq!(
        presence.activities[0]
            .timestamps
            .as_ref()
            .and_then(|timestamps| timestamps.start),
        Some(1_792_391_677_882)
    );
}

/// r03: `012` — приватный scope.
#[test]
fn r03_private_scope() {
    let state = apply_named(minimal_state(), "012-PRESENCE_UPDATE.json");
    let presence = presence_for(&state, 275_258_435_199_827_968, PresenceScope::Private)
        .expect("private presence");
    assert_eq!(presence.status, PresenceStatus::Idle);
    assert!(presence.activities.is_empty());
    assert_eq!(
        state.users[&275_258_435_199_827_968].username.as_deref(),
        Some("dpakc")
    );
}

/// r04: READY+`003` — 28 scope, 18 users, null-группы пусты.
#[test]
fn r04_supplemental_initial_snapshot() {
    let state = apply_named(minimal_state(), "002-READY.json");
    let state = apply_named(state, "003-READY_SUPPLEMENTAL.json");
    assert_eq!(state.presences.len(), 28);
    let users = state
        .presences
        .keys()
        .map(|(user, _)| *user)
        .collect::<BTreeSet<_>>();
    assert_eq!(users.len(), 18);
    assert!(
        state
            .presences
            .keys()
            .any(|(user, scope)| *user == 275_258_435_199_827_968
                && *scope == PresenceScope::Private)
    );
    assert!(
        state
            .presences
            .keys()
            .any(|(_, scope)| *scope == PresenceScope::Guild(1_014_537_851_418_980_372))
    );
    assert!(
        !state
            .presences
            .keys()
            .any(|(_, scope)| *scope == PresenceScope::Guild(700_482_297_027_231_765))
    );
}

/// r05: supplemental с отсутствующим user — inconsistency один раз.
#[test]
fn r05_supplemental_missing_user_inconsistency() {
    let frame = frame(
        "READY_SUPPLEMENTAL",
        2,
        map(vec![
            ("guilds", list(vec![map(vec![("id", text("1"))])])),
            ("merged_members", list(vec![list(Vec::new())])),
            (
                "merged_presences",
                map(vec![
                    ("friends", list(Vec::new())),
                    (
                        "guilds",
                        list(vec![list(vec![map(vec![
                            ("user_id", text("999")),
                            ("status", text("online")),
                        ])])]),
                    ),
                ]),
            ),
        ]),
    );
    let (state, _) = apply_frame(minimal_state(), &frame);
    assert_eq!(scope_count(&state, 999), 1);
    assert_eq!(
        state
            .diagnostics
            .inconsistencies
            .iter()
            .filter(|entry| entry.as_str() == "presences:user:999:missing")
            .count(),
        1
    );
}

/// r06: offline неизвестного пользователя — no-op.
#[test]
fn r06_offline_unknown_is_noop() {
    let frame = presence_frame(3, "999", Some("1"), "offline", 10, None, None);
    let (state, _) = apply_frame(minimal_state(), &frame);
    assert!(state.presences.is_empty());
    assert!(state.diagnostics.ignored.is_empty());
}

/// r07: online → offline единственного scope удаляет пользователя.
#[test]
fn r07_offline_removes_single_scope() {
    let state = apply_named(minimal_state(), "010-PRESENCE_UPDATE.json");
    let frame = presence_frame(
        8,
        "95101520944570368",
        Some("1014537851418980372"),
        "offline",
        1_792_391_400_000,
        None,
        None,
    );
    let (state, _) = apply_frame(state, &frame);
    assert_eq!(scope_count(&state, 95_101_520_944_570_368), 0);
    assert!(
        presence_for(
            &state,
            95_101_520_944_570_368,
            PresenceScope::Guild(1_014_537_851_418_980_372)
        )
        .is_none()
    );
}

/// r08: offline другого guild не удаляет online; offline всех — удаляет.
#[test]
fn r08_offline_other_guild_keeps_online_until_all_offline() {
    let state = apply_frame(
        minimal_state(),
        &presence_frame(3, "500", Some("10"), "online", 100, None, None),
    )
    .0;
    let (state, _) = apply_frame(
        state,
        &presence_frame(4, "500", Some("20"), "offline", 200, None, None),
    );
    assert_eq!(scope_count(&state, 500), 2);
    assert!(presence_for(&state, 500, PresenceScope::Guild(10)).is_some());
    assert!(presence_for(&state, 500, PresenceScope::Guild(20)).is_some());

    let (state, _) = apply_frame(
        state,
        &presence_frame(5, "500", Some("10"), "offline", 300, None, None),
    );
    assert_eq!(scope_count(&state, 500), 0);
}

/// r09: offline + hidden activity остаётся видимым, activities сохраняются.
#[test]
fn r09_offline_with_hidden_stays_visible() {
    let frame = presence_frame(
        3,
        "501",
        Some("10"),
        "offline",
        100,
        Some(list(vec![note_activity("visible")])),
        Some(list(vec![note_activity("hidden")])),
    );
    let (state, _) = apply_frame(minimal_state(), &frame);
    let presence = presence_for(&state, 501, PresenceScope::Guild(10)).expect("presence");
    assert!(presence.is_visible());
    // Официальный клиент очищает activities только для offline без hidden. При
    // hidden-фильтре activities сохраняются, но статус остаётся offline.
    assert_eq!(presence.activities.len(), 1);
    assert_eq!(presence.activities[0].name, "visible");
    assert_eq!(presence.hidden_activities.len(), 1);
    assert_eq!(presence.hidden_activities[0].name, "hidden");
}

/// r10: READY+`003`+`008` — member list добавляет presence и activities.
#[test]
fn r10_member_list_presence_integration() {
    let state = apply_named(minimal_state(), "002-READY.json");
    let state = apply_named(state, "003-READY_SUPPLEMENTAL.json");
    let state = apply_named(state, "008-GUILD_MEMBER_LIST_UPDATE.json");
    assert_eq!(state.presences.len(), 34);
    let users = state
        .presences
        .keys()
        .map(|(user, _)| *user)
        .collect::<BTreeSet<_>>();
    assert_eq!(users.len(), 24);
    let presence = presence_for(
        &state,
        95_101_520_944_570_368,
        PresenceScope::Guild(1_014_537_851_418_980_372),
    )
    .expect("member list presence");
    assert_eq!(presence.activities[0].name, "Wife's Bedroom");
    assert_eq!(
        presence.activities[0]
            .timestamps
            .as_ref()
            .and_then(|timestamps| timestamps.start),
        Some(1_792_390_727_877)
    );
}

/// r11: offline member без существующей записи не создаёт presence.
#[test]
fn r11_member_list_offline_unknown_is_noop() {
    let frame = member_list_frame(
        "10",
        vec![map(vec![
            ("op", text("SYNC")),
            ("items", list(vec![member_entry("999", "offline")])),
            ("range", list(vec![int(0), int(0)])),
        ])],
    );
    let (state, _) = apply_frame(minimal_state(), &frame);
    assert!(state.presences.is_empty());
}

/// r12: member list не меняет State.users (P2-04 frozen).
#[test]
fn r12_member_list_does_not_touch_users() {
    let state = apply_named(minimal_state(), "002-READY.json");
    let state = apply_named(state, "003-READY_SUPPLEMENTAL.json");
    let before = state.users.len();
    let state = apply_named(state, "008-GUILD_MEMBER_LIST_UPDATE.json");
    assert_eq!(state.users.len(), before);
}

/// r13: chunk presences формы PRESENCE_UPDATE применяются, shape помечен.
#[test]
fn r13_chunk_presences_are_applied() {
    let frame = chunk_frame(
        "10",
        Some(list(vec![map(vec![
            (
                "user",
                map(vec![("id", text("700")), ("username", text("chunk-user"))]),
            ),
            ("status", text("online")),
        ])])),
    );
    let (state, _) = apply_frame(minimal_state(), &frame);
    let presence = presence_for(&state, 700, PresenceScope::Guild(10)).expect("chunk presence");
    assert_eq!(presence.status, PresenceStatus::Online);
    assert!(
        state
            .diagnostics
            .inconsistencies
            .iter()
            .any(|entry| entry.contains("presences:unconfirmed-shape"))
    );
}

/// r14: chunk без presences — пустой список и без новых ignored.
#[test]
fn r14_chunk_without_presences() {
    let event = Event::from_frame(&load("022-GUILD_MEMBERS_CHUNK.json")).expect("chunk");
    let EventPayload::GuildMembersChunk(chunk) = event.payload else {
        panic!("ожидался chunk");
    };
    assert!(chunk.presences.is_empty());
    assert!(event.diagnostics.ignored.is_empty());
}

/// r15: stale seq не применяет presence.
#[test]
fn r15_stale_sequence_does_not_change_presence() {
    let state = apply_named(minimal_state(), "010-PRESENCE_UPDATE.json");
    let stale = presence_frame(
        7,
        "95101520944570368",
        Some("1014537851418980372"),
        "dnd",
        300,
        Some(list(vec![note_activity("stale")])),
        None,
    );
    let (state, _) = apply_frame(state, &stale);
    let presence = presence_for(
        &state,
        95_101_520_944_570_368,
        PresenceScope::Guild(1_014_537_851_418_980_372),
    )
    .expect("presence");
    assert_eq!(presence.status, PresenceStatus::Online);
    assert_eq!(presence.activities[0].name, "bg-capture");
    assert!(state.diagnostics.stale_sequences.contains(&7));
}

/// r16: READY сбрасывает presence и last_seq.
#[test]
fn r16_ready_resets_presence() {
    let state = apply_named(minimal_state(), "010-PRESENCE_UPDATE.json");
    assert!(!state.presences.is_empty());
    let (state, effects) = apply_frame(state, &minimal_ready_frame());
    assert!(state.presences.is_empty());
    assert_eq!(state.last_seq, Some(1));
    assert!(effects.is_empty());
}

/// r18: пустой status member-list диагностируется, а не теряется молча.
#[test]
fn r18_member_list_empty_status_is_diagnosed() {
    let frame = member_list_frame(
        "10",
        vec![map(vec![
            ("op", text("SYNC")),
            ("items", list(vec![member_entry("999", "")])),
            ("range", list(vec![int(0), int(0)])),
        ])],
    );
    let (state, _) = apply_frame(minimal_state(), &frame);
    assert!(state.presences.is_empty());
    assert!(
        state
            .diagnostics
            .ignored
            .iter()
            .any(|entry| entry.ends_with("member.presence.status")),
        "нет диагностики пустого status: {:?}",
        state.diagnostics.ignored
    );
}

/// r17: DELETE member list не удаляет presence.
#[test]
fn r17_member_list_delete_keeps_presence() {
    let state = apply_named(minimal_state(), "008-GUILD_MEMBER_LIST_UPDATE.json");
    let user_id = 95_101_520_944_570_368;
    assert!(scope_count(&state, user_id) > 0);
    let frame = member_list_frame(
        "1014537851418980372",
        vec![map(vec![("op", text("DELETE")), ("index", int(0))])],
    );
    let (state, _) = apply_frame(state, &frame);
    assert!(scope_count(&state, user_id) > 0);
}

/// r19 (P7-02 / I1, волна B06): `upsert_presence` не должен линейно сканировать весь
/// presence-store. До фикса поиск «есть ли у пользователя хоть один scope» и проверка
/// «все ли scope невидимы» обходили весь `BTreeMap`, а `retain` пересобирал его целиком —
/// на 24 229 участниках это O(n²). Оракул — карточка P7-02 (T-HARN-04): холодный старт
/// с 24k участниками должен укладываться в бюджет.
#[test]
fn r19_presence_upsert_scales_without_full_store_scan() {
    const USERS: u64 = 10_000;
    let mut state = minimal_state();
    let started = std::time::Instant::now();
    for index in 0..USERS {
        let user_id = 900_000_000_000_000_000 + index;
        let event = Event {
            seq: Some(index + 1),
            payload: EventPayload::PresenceUpdate(discord_core::state::PresenceUpdate {
                presence: discord_core::state::Presence {
                    user_id,
                    guild_id: Some(42),
                    status: PresenceStatus::Online,
                    ..discord_core::state::Presence::default()
                },
                user: None,
            }),
            diagnostics: StateDiagnostics::default(),
        };
        let (next, _) = apply(state, event);
        state = next;
    }
    let elapsed = started.elapsed();
    assert_eq!(
        state.presences.len(),
        USERS as usize,
        "все presence должны попасть в store"
    );
    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "upsert presence на {USERS} пользователей занял {elapsed:?} — ожидается менее 2 c \
         (полное сканирование store даёт O(n²))"
    );
}
