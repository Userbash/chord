//! W13b / P2-02b: чистый reducer `(State, Event) -> (State, Effects)`.
//!
//! T-DATA-04: одинаковый вход → одинаковое состояние. Числа READY/SUPPLEMENTAL и сообщений —
//! `jq`-оракулы из `waves/RW-13/RESEARCH.md` §2–§7.

mod support;

use discord_core::ready::GuildMember;
use discord_core::state::{
    Effect, Effects, Event, EventPayload, MemberRange, ReadySupplemental, State, apply, apply_frame,
};

use support::state_fixture::{atom, frame, load, map, message_create_frame, text};

fn event(name: &str) -> Event {
    let frame = load(name);
    Event::from_frame(&frame).unwrap_or_else(|error| panic!("{name}: {error}"))
}

fn state_from_event(event: Event) -> State {
    let EventPayload::Ready(ready) = event.payload else {
        panic!("ожидался READY");
    };
    State::from_ready(&ready)
}

fn ready_state() -> State {
    state_from_event(event("002-READY.json"))
}

fn minimal_state() -> State {
    let frame = support::state_fixture::minimal_ready_frame();
    let event = Event::from_frame(&frame).expect("minimal READY");
    state_from_event(event)
}

fn subscribe_ids(effects: &Effects) -> Vec<u64> {
    effects
        .as_slice()
        .iter()
        .filter_map(|effect| match effect {
            Effect::SubscribeMembers { guild_id, .. } => Some(*guild_id),
            _ => None,
        })
        .collect()
}

/// R1: `State::from_ready` — числа READY из jq.
#[test]
fn r01_state_from_real_ready_matches_jq() {
    let state = ready_state();
    assert_eq!(state.user.id, 384_299_970_632_876_032);
    assert_eq!(state.guilds.len(), 7);
    let channels: usize = state
        .guilds
        .values()
        .map(|guild| guild.channels.len())
        .sum();
    assert_eq!(channels, 272);
    assert_eq!(state.private_channels.len(), 17);
    assert_eq!(state.users.len(), 146);
    assert_eq!(state.relationships.len(), 60);
    assert_eq!(state.read_state.entries.len(), 157);
    assert_eq!(state.read_state.version, 128_949);
    assert_eq!(state.user_guild_settings.entries.len(), 58);
    assert_eq!(state.user_guild_settings.version, 734);
    assert_eq!(state.sessions.len(), 1);
    assert_eq!(state.messages.len(), 0);
    assert_eq!(state.threads.len(), 0);
    assert_eq!(state.last_seq, None);
    assert!(state.guilds.values().all(|guild| guild.members.len() == 1));
    assert_eq!(state.guilds[&1_014_537_851_418_980_372].channels.len(), 83);
    assert!(state.diagnostics.malformed.is_empty());
}

/// R2: READY детерминирован; 7 эффектов `SubscribeMembers` по возрастанию id.
#[test]
fn r02_ready_is_deterministic_and_emits_sorted_effects() {
    let base = ready_state();
    let ready = event("002-READY.json");

    let (a, effects_a) = apply(base.clone(), ready.clone());
    let (b, effects_b) = apply(base, ready);

    assert_eq!(a, b);
    assert_eq!(effects_a, effects_b);
    assert_eq!(a.last_seq, Some(1));
    assert_eq!(a.messages.len(), 0);
    assert_eq!(effects_a.len(), 7);
    assert_eq!(
        subscribe_ids(&effects_a),
        vec![
            274_093_052_673_392_641,
            294_064_825_540_280_335,
            338_001_249_918_255_106,
            700_482_297_027_231_765,
            942_355_007_271_997_450,
            1_014_537_851_418_980_372,
            1_340_572_775_974_305_803,
        ]
    );
    assert!(matches!(
        &effects_a.as_slice()[0],
        Effect::SubscribeMembers {
            ranges,
            ..
        } if ranges == &vec![MemberRange { start: 0, end: 99 }]
    ));
}

/// R3: READY + SUPPLEMENTAL — 138 сырых / 137 после dedup, группы по сырому индексу.
#[test]
fn r03_supplemental_merges_members_by_raw_guild_index() {
    let (state, _) = apply(minimal_state(), event("002-READY.json"));
    let (state, effects) = apply(state, event("003-READY_SUPPLEMENTAL.json"));

    assert!(effects.is_empty());
    assert_eq!(state.last_seq, Some(2));
    let counts: Vec<usize> = [
        274_093_052_673_392_641,
        294_064_825_540_280_335,
        338_001_249_918_255_106,
        700_482_297_027_231_765,
        942_355_007_271_997_450,
        1_014_537_851_418_980_372,
        1_340_572_775_974_305_803,
    ]
    .iter()
    .map(|id| state.guilds[id].members.len())
    .collect();
    // 138 сырых; первый дубликат user_id схлопнут P2-01 (last wins).
    assert_eq!(counts, vec![16, 14, 7, 2, 10, 87, 1]);

    let first = &state.guilds[&274_093_052_673_392_641].members[0];
    assert_eq!(first.user_id, 289_091_818_442_719_242);
    assert_eq!(first.nick.as_deref(), Some("𝓚𝓞𝓣_𝓑𝓐𝓨𝓤𝓝(Рома)"));
    assert!(state.diagnostics.malformed.is_empty());
}

/// R4: SESSIONS_REPLACE заменяет список целиком.
#[test]
fn r04_sessions_replace_replaces_sessions() {
    let (state, effects) = apply(ready_state(), event("004-SESSIONS_REPLACE.json"));
    assert!(effects.is_empty());
    assert_eq!(state.sessions.len(), 1);
    assert_eq!(state.sessions[0].session_id, "REDACTED");
    assert_eq!(state.sessions[0].status.as_deref(), Some("invisible"));
    assert_eq!(state.last_seq, Some(3));
}

/// R5: THREAD_LIST_SYNC — upsert без дублей.
#[test]
fn r05_thread_list_sync_upserts() {
    let guild = "274093052673392641";
    let parent = "751425447614480414";
    let first = support::state_fixture::thread_sync_frame(
        3,
        guild,
        vec![support::state_fixture::thread("43", guild, parent, "old")],
    );
    let second = support::state_fixture::thread_sync_frame(
        4,
        guild,
        vec![support::state_fixture::thread("43", guild, parent, "new")],
    );

    let (state, _) = apply_frame(ready_state(), &first);
    assert_eq!(state.threads.len(), 1);
    assert_eq!(state.threads[&43].name.as_deref(), Some("old"));
    assert_eq!(state.threads[&43].guild_id, 274_093_052_673_392_641);

    let (state, effects) = apply_frame(state, &second);
    assert!(effects.is_empty());
    assert_eq!(state.threads.len(), 1);
    assert_eq!(state.threads[&43].name.as_deref(), Some("new"));
    assert_eq!(state.last_seq, Some(4));
}

/// R6: CHANNEL_INFO — upsert канала; неизвестная гильдия даёт заглушку.
#[test]
fn r06_channel_info_upserts_and_creates_stub_guild() {
    let first = support::state_fixture::channel_info_frame(
        2,
        "42",
        vec![support::state_fixture::channel("43", "general")],
    );
    let second = support::state_fixture::channel_info_frame(
        3,
        "42",
        vec![support::state_fixture::channel("43", "renamed")],
    );

    let (state, _) = apply_frame(minimal_state(), &first);
    assert_eq!(state.guilds.len(), 1);
    let guild = &state.guilds[&42];
    assert!(guild.unavailable);
    assert_eq!(guild.channels.len(), 1);
    assert_eq!(guild.channels[0].name.as_deref(), Some("general"));
    assert!(
        state
            .diagnostics
            .inconsistencies
            .iter()
            .any(|entry| entry.contains("42"))
    );

    let (state, _) = apply_frame(state, &second);
    assert_eq!(state.guilds.len(), 1);
    assert_eq!(state.guilds[&42].channels.len(), 1);
    assert_eq!(
        state.guilds[&42].channels[0].name.as_deref(),
        Some("renamed")
    );
    assert_eq!(state.last_seq, Some(3));
}

/// R7: MESSAGE_CREATE + MESSAGE_UPDATE — один id, контент/embeds/автор.
#[test]
fn r07_message_create_and_update() {
    let (state, _) = apply(ready_state(), event("023-MESSAGE_CREATE.json"));
    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.users.len(), 147);
    let id = 1_561_627_527_296_979_046;
    assert_eq!(state.messages[&id].content.chars().count(), 208);
    assert!(
        state.messages[&id]
            .author
            .as_ref()
            .is_some_and(|author| author.bot)
    );

    let (state, effects) = apply(state, event("026-MESSAGE_UPDATE.json"));
    assert!(effects.is_empty());
    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.messages[&id].content.chars().count(), 208);
    assert_eq!(state.messages[&id].embed_count, 1);
    assert_eq!(state.last_seq, Some(21));
}

/// R8: stale `s` не меняет данные и попадает в диагностику.
#[test]
fn r08_stale_sequence_is_ignored() {
    let create = event("023-MESSAGE_CREATE.json");
    let (state, _) = apply(ready_state(), create.clone());
    let (state, effects) = apply(state, create);

    assert!(effects.is_empty());
    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.last_seq, Some(18));
    assert_eq!(state.diagnostics.stale_sequences, vec![18]);
}

/// R9: READY с меньшим `s` сбрасывает состояние, а не отбрасывается как stale.
#[test]
fn r09_ready_resets_sequence_and_state() {
    let (state, _) = apply(ready_state(), event("023-MESSAGE_CREATE.json"));
    assert_eq!(state.messages.len(), 1);

    let (state, effects) = apply(state, event("002-READY.json"));
    assert_eq!(state.messages.len(), 0);
    assert_eq!(state.guilds.len(), 7);
    assert_eq!(state.last_seq, Some(1));
    assert!(state.diagnostics.stale_sequences.is_empty());
    assert_eq!(effects.len(), 7);
}

/// R10: unknown/unsupported — диагностика, данные не меняются, seq обновляется.
#[test]
fn r10_unknown_and_unsupported_are_non_fatal() {
    let unknown = frame("FUTURE_EVENT", 2, map(Vec::new()));
    let (state, effects) = apply_frame(minimal_state(), &unknown);
    assert!(effects.is_empty());
    assert_eq!(state.diagnostics.unknown_events, vec!["FUTURE_EVENT"]);
    assert_eq!(state.last_seq, Some(2));
    assert_eq!(state.guilds.len(), 0);

    // P2-09: PRESENCE_UPDATE типизирован; для Unsupported берём другой известный тип.
    let typing = frame("TYPING_START", 3, map(Vec::new()));
    let (state, effects) = apply_frame(state, &typing);
    assert!(effects.is_empty());
    assert_eq!(state.diagnostics.unsupported_events, vec!["TYPING_START"]);
    assert_eq!(state.last_seq, Some(3));
}

/// R11: битый dispatch — диагностика, без паники, данные не меняются.
#[test]
fn r11_malformed_dispatch_is_diagnostic() {
    let broken = frame("MESSAGE_CREATE", 5, map(vec![("id", text("1"))]));
    let (state, effects) = apply_frame(minimal_state(), &broken);
    assert!(effects.is_empty());
    assert!(state.messages.is_empty());
    assert!(
        state
            .diagnostics
            .malformed
            .iter()
            .any(|entry| entry.starts_with("event:")),
        "диагностика: {:?}",
        state.diagnostics
    );
    assert_eq!(state.last_seq, Some(5));
}

/// R12: UPDATE неизвестного сообщения: с каналом — заготовка, без канала — malformed.
#[test]
fn r12_message_update_unknown_message() {
    let with_channel = frame(
        "MESSAGE_UPDATE",
        2,
        map(vec![
            ("id", text("900")),
            ("channel_id", text("501")),
            ("content", text("hi")),
        ]),
    );
    let (state, _) = apply_frame(minimal_state(), &with_channel);
    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.messages[&900].content, "hi");
    assert!(
        state
            .diagnostics
            .inconsistencies
            .iter()
            .any(|entry| entry.contains("900"))
    );

    let without_channel = frame(
        "MESSAGE_UPDATE",
        3,
        map(vec![("id", text("901")), ("content", text("x"))]),
    );
    let (state, _) = apply_frame(state, &without_channel);
    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.diagnostics.malformed.len(), 1);
    assert!(state.diagnostics.malformed[0].contains("901"));
    assert!(state.diagnostics.malformed[0].contains("channel_id"));
    assert_eq!(state.last_seq, Some(3));
}

/// R13: API эффектов готово для следующих карточек.
#[test]
fn r13_effects_api_declares_all_four_kinds() {
    let mut effects = Effects::new();
    assert!(effects.is_empty());
    effects.push(Effect::FetchMessages {
        channel_id: 1,
        before: Some(2),
        limit: 50,
    });
    effects.push(Effect::SubscribeMembers {
        guild_id: 3,
        ranges: vec![MemberRange { start: 0, end: 99 }],
    });
    effects.push(Effect::AckMessage {
        channel_id: 4,
        message_id: 5,
    });
    effects.push(Effect::RequestPresence {
        guild_id: 6,
        user_ids: vec![7, 8],
    });
    assert_eq!(effects.len(), 4);
    assert!(matches!(
        &effects.as_slice()[0],
        Effect::FetchMessages { limit: 50, .. }
    ));
}

/// R14: stale malformed dispatch не откатывает `last_seq` и не пропускает stale.
#[test]
fn r14_stale_malformed_dispatch_does_not_rewind_sequence() {
    let (state, _) = apply(ready_state(), event("023-MESSAGE_CREATE.json"));
    assert_eq!(state.last_seq, Some(18));

    let malformed = frame("MESSAGE_CREATE", 2, map(vec![("id", text("999"))]));
    let (state, _) = apply_frame(state, &malformed);
    assert_eq!(state.last_seq, Some(18));
    assert_eq!(state.messages.len(), 1);

    let old = message_create_frame(5, "400", "1163408591315812392", "old");
    let (state, effects) = apply_frame(state, &old);
    assert!(effects.is_empty());
    assert_eq!(state.messages.len(), 1);
    assert!(state.diagnostics.stale_sequences.contains(&5));
}

/// R15: malformed update без канала не меняет `users` автором.
#[test]
fn r15_malformed_update_does_not_upsert_author() {
    let update = frame(
        "MESSAGE_UPDATE",
        2,
        map(vec![
            ("id", text("901")),
            (
                "author",
                map(vec![("id", text("777")), ("username", text("ghost"))]),
            ),
        ]),
    );
    let (state, _) = apply_frame(minimal_state(), &update);
    assert_eq!(state.users.len(), 1);
    assert_eq!(state.messages.len(), 0);
    assert_eq!(state.diagnostics.malformed.len(), 1);
    assert!(state.diagnostics.malformed[0].contains("channel_id"));
}

/// R16: сообщение в приватном канале не даёт ложной inconsistency.
#[test]
fn r16_message_in_private_channel_is_known() {
    let private = "696738350706458685";
    let create = message_create_frame(18, "900", private, "dm");
    let (state, _) = apply_frame(ready_state(), &create);
    assert_eq!(state.messages.len(), 1);
    assert!(
        !state
            .diagnostics
            .inconsistencies
            .iter()
            .any(|entry| entry.contains("messages:900"))
    );
}

/// R17: невалидный raw-индекс supplemental — inconsistency, не malformed.
#[test]
fn r17_supplemental_invalid_raw_index_is_inconsistency() {
    let member = GuildMember {
        user_id: 1,
        ..GuildMember::default()
    };
    let event = Event {
        seq: Some(2),
        payload: EventPayload::ReadySupplemental(ReadySupplemental {
            guild_ids: vec![Some(42), None],
            merged_members: vec![vec![member.clone()], vec![member]],
            merged_presences: Default::default(),
        }),
        diagnostics: Default::default(),
    };
    let (state, _) = apply(minimal_state(), event);
    assert!(
        state
            .diagnostics
            .inconsistencies
            .iter()
            .any(|entry| entry.contains("merged_members[1]"))
    );
    assert!(
        !state
            .diagnostics
            .malformed
            .iter()
            .any(|entry| entry.contains("merged_members[1]"))
    );
}

/// R18: RESUMED — no-op, обновляет последовательность.
#[test]
fn r18_resumed_is_noop() {
    let (state, _) = apply(ready_state(), event("023-MESSAGE_CREATE.json"));
    let resumed = frame("RESUMED", 19, atom("nil"));
    let (state, effects) = apply_frame(state, &resumed);
    assert!(effects.is_empty());
    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.last_seq, Some(19));
}

/// R19: crate-root re-exports, обещанные ТЗ.
#[test]
fn r19_crate_root_reexports() {
    let frame = support::state_fixture::minimal_ready_frame();
    let event = discord_core::Event::from_frame(&frame).expect("minimal READY");
    let state = match &event.payload {
        EventPayload::Ready(ready) => discord_core::State::from_ready(ready),
        _ => panic!("ожидался READY"),
    };
    let (state, _) = discord_core::apply(state, event);
    assert_eq!(state.user.id, 100);
}
