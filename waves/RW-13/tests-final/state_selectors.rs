//! W13c / P2-02c: селекторы состояния.
//!
//! READY-оракул — `fixtures/gateway/by-type/002-READY.json` и `jq` из
//! `waves/RW-13/RESEARCH.md` §2. Сортировки проверяются по ключу, а не по порядку реализации.

mod support;

use discord_core::state::{Event, EventPayload, State, apply, apply_frame};

use support::state_fixture::{load, message_create_frame};

fn event(name: &str) -> Event {
    Event::from_frame(&load(name)).unwrap_or_else(|error| panic!("{name}: {error}"))
}

fn ready_state() -> State {
    let event = event("002-READY.json");
    let EventPayload::Ready(ready) = event.payload else {
        panic!("ожидался READY");
    };
    State::from_ready(&ready)
}

fn minimal_state() -> State {
    let frame = support::state_fixture::minimal_ready_frame();
    let event = Event::from_frame(&frame).expect("minimal READY");
    let EventPayload::Ready(ready) = event.payload else {
        panic!("ожидался READY");
    };
    State::from_ready(&ready)
}

/// S1: базовые выборки READY.
#[test]
fn s01_ready_lookups() {
    let state = ready_state();

    let guild = state.guild(1_014_537_851_418_980_372).expect("ITSKIDON");
    assert_eq!(guild.name.as_deref(), Some("ITSKIDON PC BOOST"));
    assert_eq!(guild.member_count, Some(24_228));

    let channel = state.channel(1_163_408_591_315_812_392).expect("канал");
    assert_eq!(channel.name.as_deref(), Some("📃┃новости"));
    assert_eq!(
        state.channel_guild_id(1_163_408_591_315_812_392),
        Some(1_014_537_851_418_980_372)
    );

    let private = state.private_channel(696_738_350_706_458_685).expect("ЛС");
    assert_eq!(private.channel_type, 1);

    let user = state.user(384_299_970_632_876_032).expect("self");
    assert_eq!(user.username.as_deref(), Some("pharos12510"));

    let relationship = state
        .relationship(194_374_444_821_643_266)
        .expect("relationship");
    assert_eq!(relationship.relation_type, 1);

    let read = state
        .read_state_entry(274_093_052_673_392_641)
        .expect("read_state");
    assert_eq!(read.mention_count, Some(0));

    let settings = state
        .user_guild_settings_entry(1_014_537_851_418_980_372)
        .expect("UGS");
    assert_eq!(settings.version, 734);
    assert!(settings.muted);
}

/// S2: каналы гильдии — 83, порядок `(position, id)`.
#[test]
fn s02_channels_in_guild_sorted() {
    let state = ready_state();
    let channels = state.channels_in_guild(1_014_537_851_418_980_372);
    assert_eq!(channels.len(), 83);

    let keys: Vec<(i64, u64)> = channels
        .iter()
        .map(|channel| (channel.position, channel.id))
        .collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(keys, sorted);
    assert_eq!(channels[0].name.as_deref(), Some("📞┃Пекарня"));
}

/// S3: сообщения канала — по возрастанию id, независимо от порядка применения.
#[test]
fn s03_messages_in_channel_sorted_by_id() {
    let state = ready_state();
    let channel = "1163408591315812392";
    let (state, _) = apply_frame(state, &message_create_frame(18, "300", channel, "third"));
    let (state, _) = apply_frame(state, &message_create_frame(19, "200", channel, "second"));

    let messages = state.messages_in_channel(1_163_408_591_315_812_392);
    assert_eq!(
        messages
            .iter()
            .map(|message| message.id)
            .collect::<Vec<_>>(),
        vec![200, 300]
    );
    assert_eq!(messages[0].content, "second");
    assert!(state.messages_in_channel(999).is_empty());
}

/// S4: участник после READY_SUPPLEMENTAL.
#[test]
fn s04_guild_member_lookup() {
    let (state, _) = apply(ready_state(), event("003-READY_SUPPLEMENTAL.json"));
    let member = state
        .guild_member(274_093_052_673_392_641, 289_091_818_442_719_242)
        .expect("участник");
    assert_eq!(member.nick.as_deref(), Some("𝓚𝓞𝓣_𝓑𝓐𝓨𝓤𝓝(Рома)"));
    assert!(state.guild_member(274_093_052_673_392_641, 1).is_none());
}

/// S5: треды — выборка и сортировка по id.
#[test]
fn s05_threads_in_guild_sorted() {
    let guild = "274093052673392641";
    let parent = "751425447614480414";
    let frame = support::state_fixture::thread_sync_frame(
        3,
        guild,
        vec![
            support::state_fixture::thread("43", guild, parent, "b"),
            support::state_fixture::thread("42", guild, parent, "a"),
        ],
    );
    let (state, _) = apply_frame(ready_state(), &frame);

    assert_eq!(state.threads_in_guild(274_093_052_673_392_641).len(), 2);
    assert_eq!(
        state
            .threads_in_guild(274_093_052_673_392_641)
            .iter()
            .map(|thread| thread.id)
            .collect::<Vec<_>>(),
        vec![42, 43]
    );
    assert_eq!(state.thread(43).expect("тред").name.as_deref(), Some("b"));
    assert!(state.threads_in_guild(1).is_empty());
}

/// S6: отсутствующие id — `None`/пусто, без паники.
#[test]
fn s06_missing_lookups() {
    let state = minimal_state();
    assert!(state.guild(1).is_none());
    assert!(state.channel(1).is_none());
    assert!(state.channel_guild_id(1).is_none());
    assert!(state.private_channel(1).is_none());
    assert!(state.user(1).is_none());
    assert!(state.relationship(1).is_none());
    assert!(state.message(1).is_none());
    assert!(state.thread(1).is_none());
    assert!(state.read_state_entry(1).is_none());
    assert!(state.user_guild_settings_entry(1).is_none());
    assert!(state.channels_in_guild(1).is_empty());
    assert!(state.messages_in_channel(1).is_empty());
    assert!(state.threads_in_guild(1).is_empty());
}

/// S7: счётчики READY.
#[test]
fn s07_counts() {
    let state = ready_state();
    assert_eq!(state.guild_count(), 7);
    assert_eq!(state.channel_count(), 272);
    assert_eq!(state.private_channel_count(), 17);
    assert_eq!(state.user_count(), 146);
    assert_eq!(state.message_count(), 0);
    assert_eq!(state.thread_count(), 0);
}

/// S8: индекс канала обновляется после CHANNEL_INFO.
#[test]
fn s08_channel_index_after_channel_info() {
    let frame = support::state_fixture::channel_info_frame(
        2,
        "42",
        vec![support::state_fixture::channel("43", "general")],
    );
    let (state, _) = apply_frame(minimal_state(), &frame);
    assert_eq!(state.channel_guild_id(43), Some(42));
    assert_eq!(
        state.channel(43).expect("канал").name.as_deref(),
        Some("general")
    );
    assert_eq!(state.channels_in_guild(42).len(), 1);
    assert_eq!(state.channel_count(), 1);
}
