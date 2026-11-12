//! Контрактные тесты для task-157 (BE-P1-02 / F-GW-06):
//! Обработка MESSAGE_DELETE, MESSAGE_DELETE_BULK, CHANNEL_CREATE/UPDATE/DELETE
//! и THREAD_CREATE/UPDATE/DELETE в State reducer.

mod support;

use chord_core::dispatch::{
    ChannelLifecycleDispatch, MessageDeleteBulkDispatch, MessageDeleteDispatch,
    ThreadLifecycleDispatch,
};
use chord_core::ready::Guild;
use chord_core::state::{Event, EventPayload, State, apply, apply_frame};
use support::state_fixture::{frame, int, list, map, text};

fn minimal_state_with_guild(guild_id: u64) -> State {
    let frame = support::state_fixture::minimal_ready_frame();
    let event = Event::from_frame(&frame).expect("minimal READY");
    let EventPayload::Ready(ready) = event.payload else {
        panic!("ожидался READY");
    };
    let mut state = State::from_ready(&ready);
    state.guilds.insert(
        guild_id,
        Guild {
            id: guild_id,
            ..Guild::default()
        },
    );
    state
}

fn message_frame(
    seq: u64,
    id: u64,
    channel_id: u64,
    content: &str,
) -> chord_codec::etf::EtfValue {
    frame(
        "MESSAGE_CREATE",
        seq,
        map(vec![
            ("id", text(&id.to_string())),
            ("channel_id", text(&channel_id.to_string())),
            ("content", text(content)),
        ]),
    )
}

#[test]
fn c01_message_delete_removes_message_and_is_idempotent() {
    let guild_id = 100;
    let channel_id = 200;
    let msg_id = 501;

    let mut state = minimal_state_with_guild(guild_id);
    let (s, _) = apply(
        state,
        Event {
            seq: Some(10),
            payload: EventPayload::ChannelCreate(ChannelLifecycleDispatch {
                id: channel_id,
                guild_id: Some(guild_id),
                kind: Some(0),
                name: Some("general".to_string()),
                parent_id: None,
            }),
            diagnostics: Default::default(),
        },
    );
    state = s;

    let (s, _) = apply_frame(state, &message_frame(11, msg_id, channel_id, "hello"));
    state = s;
    assert!(state.messages.contains_key(&msg_id));
    assert_eq!(
        state.message(msg_id).map(|m| m.content.as_str()),
        Some("hello")
    );

    // 1. Применяем MessageDelete
    let (s, _) = apply(
        state,
        Event {
            seq: Some(12),
            payload: EventPayload::MessageDelete(MessageDeleteDispatch {
                id: msg_id,
                channel_id,
                guild_id: Some(guild_id),
            }),
            diagnostics: Default::default(),
        },
    );
    state = s;
    assert!(!state.messages.contains_key(&msg_id));
    assert_eq!(state.message(msg_id), None);

    // 2. Идемпотентность — повторное удаление не вызывает паники
    let (s, _) = apply(
        state,
        Event {
            seq: Some(13),
            payload: EventPayload::MessageDelete(MessageDeleteDispatch {
                id: msg_id,
                channel_id,
                guild_id: Some(guild_id),
            }),
            diagnostics: Default::default(),
        },
    );
    assert!(!s.messages.contains_key(&msg_id));
}

#[test]
fn c02_message_delete_bulk_removes_multiple_messages() {
    let guild_id = 100;
    let channel_id = 200;

    let mut state = minimal_state_with_guild(guild_id);
    let (s, _) = apply(
        state,
        Event {
            seq: Some(10),
            payload: EventPayload::ChannelCreate(ChannelLifecycleDispatch {
                id: channel_id,
                guild_id: Some(guild_id),
                kind: Some(0),
                name: Some("general".to_string()),
                parent_id: None,
            }),
            diagnostics: Default::default(),
        },
    );
    state = s;

    // Вставляем 4 сообщения
    for i in 1..=4 {
        let (s, _) = apply_frame(state, &message_frame(10 + i, 500 + i, channel_id, "msg"));
        state = s;
    }
    assert_eq!(state.messages.len(), 4);

    // Удаляем сообщения 501, 503 и несуществующее 999
    let (s, _) = apply(
        state,
        Event {
            seq: Some(20),
            payload: EventPayload::MessageDeleteBulk(MessageDeleteBulkDispatch {
                ids: vec![501, 503, 999],
                channel_id,
                guild_id: Some(guild_id),
            }),
            diagnostics: Default::default(),
        },
    );
    state = s;
    assert_eq!(state.messages.len(), 2);
    assert!(!state.messages.contains_key(&501));
    assert!(state.messages.contains_key(&502));
    assert!(!state.messages.contains_key(&503));
    assert!(state.messages.contains_key(&504));
}

#[test]
fn c03_channel_create_and_update_in_existing_guild() {
    let guild_id = 100;
    let channel_id = 200;

    let state = minimal_state_with_guild(guild_id);

    // 1. ChannelCreate
    let (state, _) = apply(
        state,
        Event {
            seq: Some(10),
            payload: EventPayload::ChannelCreate(ChannelLifecycleDispatch {
                id: channel_id,
                guild_id: Some(guild_id),
                kind: Some(0),
                name: Some("general".to_string()),
                parent_id: None,
            }),
            diagnostics: Default::default(),
        },
    );

    assert_eq!(state.channel_guild_id(channel_id), Some(guild_id));
    let ch = state
        .channel(channel_id)
        .expect("канал должен существовать");
    assert_eq!(ch.name.as_deref(), Some("general"));
    assert_eq!(ch.channel_type, 0);

    // 2. ChannelUpdate
    let (state, _) = apply(
        state,
        Event {
            seq: Some(11),
            payload: EventPayload::ChannelUpdate(ChannelLifecycleDispatch {
                id: channel_id,
                guild_id: Some(guild_id),
                kind: Some(0),
                name: Some("general-renamed".to_string()),
                parent_id: Some(150),
            }),
            diagnostics: Default::default(),
        },
    );

    let ch = state
        .channel(channel_id)
        .expect("канал должен существовать");
    assert_eq!(ch.name.as_deref(), Some("general-renamed"));
    assert_eq!(ch.parent_id, Some(150));
}

#[test]
fn c04_channel_create_in_unknown_guild_records_inconsistency() {
    let state = minimal_state_with_guild(100);
    let unknown_guild_id = 999;
    let channel_id = 300;

    let (state, _) = apply(
        state,
        Event {
            seq: Some(10),
            payload: EventPayload::ChannelCreate(ChannelLifecycleDispatch {
                id: channel_id,
                guild_id: Some(unknown_guild_id),
                kind: Some(0),
                name: Some("orphan".to_string()),
                parent_id: None,
            }),
            diagnostics: Default::default(),
        },
    );

    assert_eq!(state.channel(channel_id), None);
    assert!(
        state
            .diagnostics
            .inconsistencies
            .iter()
            .any(|inc| inc.contains("999")),
        "должна быть зафиксирована несогласованность с гильдией 999"
    );
}

#[test]
fn c05_channel_delete_removes_channel_and_messages() {
    let guild_id = 100;
    let channel_id = 200;
    let other_channel_id = 201;

    let mut state = minimal_state_with_guild(guild_id);

    // Создаём два канала
    for cid in [channel_id, other_channel_id] {
        let (s, _) = apply(
            state,
            Event {
                seq: None,
                payload: EventPayload::ChannelCreate(ChannelLifecycleDispatch {
                    id: cid,
                    guild_id: Some(guild_id),
                    kind: Some(0),
                    name: Some(format!("chan-{cid}")),
                    parent_id: None,
                }),
                diagnostics: Default::default(),
            },
        );
        state = s;
    }

    // Сообщения в channel_id
    let (s, _) = apply_frame(state, &message_frame(10, 501, channel_id, "c1 msg 1"));
    let (s, _) = apply_frame(s, &message_frame(11, 502, channel_id, "c1 msg 2"));
    // Сообщение в other_channel_id
    let (s, _) = apply_frame(s, &message_frame(12, 503, other_channel_id, "c2 msg 1"));
    state = s;

    assert_eq!(state.messages.len(), 3);

    // Удаляем channel_id
    let (state, _) = apply(
        state,
        Event {
            seq: Some(20),
            payload: EventPayload::ChannelDelete(ChannelLifecycleDispatch {
                id: channel_id,
                guild_id: Some(guild_id),
                kind: Some(0),
                name: Some(format!("chan-{channel_id}")),
                parent_id: None,
            }),
            diagnostics: Default::default(),
        },
    );

    assert_eq!(state.channel(channel_id), None);
    assert_eq!(state.channel_guild_id(channel_id), None);
    assert!(state.channel(other_channel_id).is_some());

    // Сообщения удалённого канала очищены, чужие сохранены
    assert_eq!(state.messages.len(), 1);
    assert!(!state.messages.contains_key(&501));
    assert!(!state.messages.contains_key(&502));
    assert!(state.messages.contains_key(&503));
}

#[test]
fn c06_thread_create_and_update() {
    let guild_id = 100;
    let parent_id = 200;
    let thread_id = 400;

    let state = minimal_state_with_guild(guild_id);

    // ThreadCreate
    let (state, _) = apply(
        state,
        Event {
            seq: Some(10),
            payload: EventPayload::ThreadCreate(ThreadLifecycleDispatch {
                id: thread_id,
                guild_id: Some(guild_id),
                kind: Some(11),
                name: Some("thread-test".to_string()),
                parent_id: Some(parent_id),
                newly_created: Some(true),
            }),
            diagnostics: Default::default(),
        },
    );

    let th = state
        .thread(thread_id)
        .expect("тред должен быть в состоянии");
    assert_eq!(th.name.as_deref(), Some("thread-test"));
    assert_eq!(th.guild_id, guild_id);
    assert_eq!(th.parent_id, Some(parent_id));

    // ThreadUpdate
    let (state, _) = apply(
        state,
        Event {
            seq: Some(11),
            payload: EventPayload::ThreadUpdate(ThreadLifecycleDispatch {
                id: thread_id,
                guild_id: Some(guild_id),
                kind: Some(11),
                name: Some("thread-test-renamed".to_string()),
                parent_id: Some(parent_id),
                newly_created: None,
            }),
            diagnostics: Default::default(),
        },
    );

    let th = state
        .thread(thread_id)
        .expect("тред должен быть в состоянии");
    assert_eq!(th.name.as_deref(), Some("thread-test-renamed"));
}

#[test]
fn c07_thread_delete_removes_thread_and_messages() {
    let guild_id = 100;
    let thread_id = 400;

    let mut state = minimal_state_with_guild(guild_id);

    // Создаём тред
    let (s, _) = apply(
        state,
        Event {
            seq: Some(10),
            payload: EventPayload::ThreadCreate(ThreadLifecycleDispatch {
                id: thread_id,
                guild_id: Some(guild_id),
                kind: Some(11),
                name: Some("thread-to-delete".to_string()),
                parent_id: Some(200),
                newly_created: Some(true),
            }),
            diagnostics: Default::default(),
        },
    );
    state = s;

    // Вставляем сообщение в тред
    let (s, _) = apply_frame(state, &message_frame(11, 601, thread_id, "msg in thread"));
    state = s;
    assert!(state.messages.contains_key(&601));

    // Удаляем тред
    let (state, _) = apply(
        state,
        Event {
            seq: Some(12),
            payload: EventPayload::ThreadDelete(ThreadLifecycleDispatch {
                id: thread_id,
                guild_id: Some(guild_id),
                kind: Some(11),
                name: None,
                parent_id: Some(200),
                newly_created: None,
            }),
            diagnostics: Default::default(),
        },
    );

    assert_eq!(state.thread(thread_id), None);
    assert!(!state.messages.contains_key(&601));
}

#[test]
fn c08_wire_etf_frames_route_and_apply_all_eight_events() {
    let guild_id = 100;
    let channel_id = 200;
    let thread_id = 300;
    let msg_id = 400;

    let mut state = minimal_state_with_guild(guild_id);

    // 1. CHANNEL_CREATE через ETF op0
    let f = frame(
        "CHANNEL_CREATE",
        10,
        map(vec![
            ("id", text(&channel_id.to_string())),
            ("guild_id", text(&guild_id.to_string())),
            ("type", int(0)),
            ("name", text("wire-channel")),
        ]),
    );
    let (s, _) = apply_frame(state, &f);
    state = s;
    assert!(state.channel(channel_id).is_some());
    assert_eq!(
        state.channel(channel_id).unwrap().name.as_deref(),
        Some("wire-channel")
    );

    // 2. CHANNEL_UPDATE через ETF op0
    let f = frame(
        "CHANNEL_UPDATE",
        11,
        map(vec![
            ("id", text(&channel_id.to_string())),
            ("guild_id", text(&guild_id.to_string())),
            ("name", text("wire-channel-updated")),
        ]),
    );
    let (s, _) = apply_frame(state, &f);
    state = s;
    assert_eq!(
        state.channel(channel_id).unwrap().name.as_deref(),
        Some("wire-channel-updated")
    );

    // 3. THREAD_CREATE через ETF op0
    let f = frame(
        "THREAD_CREATE",
        12,
        map(vec![
            ("id", text(&thread_id.to_string())),
            ("guild_id", text(&guild_id.to_string())),
            ("type", int(11)),
            ("name", text("wire-thread")),
            ("parent_id", text(&channel_id.to_string())),
        ]),
    );
    let (s, _) = apply_frame(state, &f);
    state = s;
    assert!(state.thread(thread_id).is_some());

    // 4. THREAD_UPDATE через ETF op0
    let f = frame(
        "THREAD_UPDATE",
        13,
        map(vec![
            ("id", text(&thread_id.to_string())),
            ("guild_id", text(&guild_id.to_string())),
            ("name", text("wire-thread-renamed")),
        ]),
    );
    let (s, _) = apply_frame(state, &f);
    state = s;
    assert_eq!(
        state.thread(thread_id).unwrap().name.as_deref(),
        Some("wire-thread-renamed")
    );

    // Добавим сообщения
    let (s, _) = apply_frame(state, &message_frame(14, msg_id, channel_id, "hello"));
    let (s, _) = apply_frame(s, &message_frame(15, msg_id + 1, channel_id, "hello 2"));
    state = s;
    assert_eq!(state.messages.len(), 2);

    // 5. MESSAGE_DELETE через ETF op0
    let f = frame(
        "MESSAGE_DELETE",
        16,
        map(vec![
            ("id", text(&msg_id.to_string())),
            ("channel_id", text(&channel_id.to_string())),
            ("guild_id", text(&guild_id.to_string())),
        ]),
    );
    let (s, _) = apply_frame(state, &f);
    state = s;
    assert!(!state.messages.contains_key(&msg_id));
    assert!(state.messages.contains_key(&(msg_id + 1)));

    // 6. MESSAGE_DELETE_BULK через ETF op0
    let f = frame(
        "MESSAGE_DELETE_BULK",
        17,
        map(vec![
            ("ids", list(vec![text(&(msg_id + 1).to_string())])),
            ("channel_id", text(&channel_id.to_string())),
            ("guild_id", text(&guild_id.to_string())),
        ]),
    );
    let (s, _) = apply_frame(state, &f);
    state = s;
    assert_eq!(state.messages.len(), 0);

    // 7. THREAD_DELETE через ETF op0
    let f = frame(
        "THREAD_DELETE",
        18,
        map(vec![
            ("id", text(&thread_id.to_string())),
            ("guild_id", text(&guild_id.to_string())),
        ]),
    );
    let (s, _) = apply_frame(state, &f);
    state = s;
    assert_eq!(state.thread(thread_id), None);

    // 8. CHANNEL_DELETE через ETF op0
    let f = frame(
        "CHANNEL_DELETE",
        19,
        map(vec![
            ("id", text(&channel_id.to_string())),
            ("guild_id", text(&guild_id.to_string())),
        ]),
    );
    let (s, _) = apply_frame(state, &f);
    state = s;
    assert_eq!(state.channel(channel_id), None);
}

#[test]
fn c09_stale_lifecycle_events_are_ignored() {
    let guild_id = 100;
    let channel_id = 200;
    let mut state = minimal_state_with_guild(guild_id);
    state.last_seq = Some(50);

    // Событие с seq = 45 (старее last_seq) должно быть отброшено
    let f = frame(
        "CHANNEL_CREATE",
        45,
        map(vec![
            ("id", text(&channel_id.to_string())),
            ("guild_id", text(&guild_id.to_string())),
            ("name", text("stale-channel")),
        ]),
    );
    let (state, _) = apply_frame(state, &f);
    assert_eq!(state.channel(channel_id), None);
    assert_eq!(state.diagnostics.stale_sequences, vec![45]);
}

#[test]
fn c10_channel_migration_between_guilds_records_inconsistency() {
    let mut state = minimal_state_with_guild(100);
    state.guilds.insert(
        101,
        Guild {
            id: 101,
            ..Guild::default()
        },
    );

    let channel_id = 200;
    // 1. Создаём в гильдии 100
    let (s, _) = apply(
        state,
        Event {
            seq: Some(1),
            payload: EventPayload::ChannelCreate(ChannelLifecycleDispatch {
                id: channel_id,
                guild_id: Some(100),
                kind: Some(0),
                name: Some("chan".to_string()),
                parent_id: None,
            }),
            diagnostics: Default::default(),
        },
    );
    state = s;
    assert_eq!(state.channel_guild_id(channel_id), Some(100));
    assert!(
        state.guilds[&100]
            .channels
            .iter()
            .any(|c| c.id == channel_id)
    );

    // 2. Обновляем с указанием гильдии 101 (миграция канала)
    let (s, _) = apply(
        state,
        Event {
            seq: Some(2),
            payload: EventPayload::ChannelUpdate(ChannelLifecycleDispatch {
                id: channel_id,
                guild_id: Some(101),
                kind: Some(0),
                name: Some("chan-moved".to_string()),
                parent_id: None,
            }),
            diagnostics: Default::default(),
        },
    );
    state = s;

    assert_eq!(state.channel_guild_id(channel_id), Some(101));
    assert!(
        !state.guilds[&100]
            .channels
            .iter()
            .any(|c| c.id == channel_id)
    );
    assert!(
        state.guilds[&101]
            .channels
            .iter()
            .any(|c| c.id == channel_id)
    );
    assert!(
        state
            .diagnostics
            .inconsistencies
            .contains(&"channels:200:guilds:100->101".to_string())
    );
}

#[test]
fn c11_thread_migration_between_guilds_records_inconsistency() {
    let mut state = minimal_state_with_guild(100);
    state.guilds.insert(
        101,
        Guild {
            id: 101,
            ..Guild::default()
        },
    );

    let thread_id = 400;
    let (s, _) = apply(
        state,
        Event {
            seq: Some(1),
            payload: EventPayload::ThreadCreate(ThreadLifecycleDispatch {
                id: thread_id,
                guild_id: Some(100),
                kind: Some(11),
                name: Some("th".to_string()),
                parent_id: Some(200),
                newly_created: None,
            }),
            diagnostics: Default::default(),
        },
    );
    state = s;
    assert_eq!(state.thread(thread_id).unwrap().guild_id, 100);

    // Миграция в гильдию 101
    let (s, _) = apply(
        state,
        Event {
            seq: Some(2),
            payload: EventPayload::ThreadUpdate(ThreadLifecycleDispatch {
                id: thread_id,
                guild_id: Some(101),
                kind: Some(11),
                name: Some("th-moved".to_string()),
                parent_id: Some(200),
                newly_created: None,
            }),
            diagnostics: Default::default(),
        },
    );
    state = s;

    assert_eq!(state.thread(thread_id).unwrap().guild_id, 101);
    assert!(
        state
            .diagnostics
            .inconsistencies
            .contains(&"threads:400:guilds:100->101".to_string())
    );
}

#[test]
fn c12_channel_delete_cleans_child_threads_and_messages() {
    let guild_id = 100;
    let channel_id = 200;
    let thread_id = 300;

    let mut state = minimal_state_with_guild(guild_id);
    let (s, _) = apply(
        state,
        Event {
            seq: None,
            payload: EventPayload::ChannelCreate(ChannelLifecycleDispatch {
                id: channel_id,
                guild_id: Some(guild_id),
                kind: Some(0),
                name: Some("main-ch".to_string()),
                parent_id: None,
            }),
            diagnostics: Default::default(),
        },
    );
    let (s, _) = apply(
        s,
        Event {
            seq: None,
            payload: EventPayload::ThreadCreate(ThreadLifecycleDispatch {
                id: thread_id,
                guild_id: Some(guild_id),
                kind: Some(11),
                name: Some("child-thread".to_string()),
                parent_id: Some(channel_id),
                newly_created: None,
            }),
            diagnostics: Default::default(),
        },
    );
    let (s, _) = apply_frame(s, &message_frame(1, 801, thread_id, "thread msg"));
    state = s;

    assert!(state.thread(thread_id).is_some());
    assert!(state.messages.contains_key(&801));

    // Удаляем родительский канал
    let (state, _) = apply(
        state,
        Event {
            seq: None,
            payload: EventPayload::ChannelDelete(ChannelLifecycleDispatch {
                id: channel_id,
                guild_id: Some(guild_id),
                kind: Some(0),
                name: None,
                parent_id: None,
            }),
            diagnostics: Default::default(),
        },
    );

    assert_eq!(state.channel(channel_id), None);
    assert_eq!(state.thread(thread_id), None);
    assert!(!state.messages.contains_key(&801));
}

#[test]
fn c13_type_aliases_and_api_compatibility() {
    use chord_core::state::types::{
        ChannelEvent, MessageDeleteBulkEvent, MessageDeleteEvent, ThreadEvent,
    };

    let msg_del: MessageDeleteEvent = MessageDeleteDispatch {
        id: 1,
        channel_id: 2,
        guild_id: Some(3),
    };
    let bulk_del: MessageDeleteBulkEvent = MessageDeleteBulkDispatch {
        ids: vec![1, 2],
        channel_id: 2,
        guild_id: Some(3),
    };
    let chan_ev: ChannelEvent = ChannelLifecycleDispatch {
        id: 10,
        guild_id: Some(3),
        kind: Some(0),
        name: Some("c".to_string()),
        parent_id: None,
    };
    let thr_ev: ThreadEvent = ThreadLifecycleDispatch {
        id: 20,
        guild_id: Some(3),
        kind: Some(11),
        name: Some("t".to_string()),
        parent_id: Some(10),
        newly_created: None,
    };

    assert_eq!(msg_del.id, 1);
    assert_eq!(bulk_del.ids.len(), 2);
    assert_eq!(chan_ev.name.as_deref(), Some("c"));
    assert_eq!(thr_ev.thread_type(), Some(11));
}

#[test]
fn c14_malformed_wire_payload_records_diagnostic() {
    let state = minimal_state_with_guild(100);
    // MESSAGE_DELETE без обязательного channel_id
    let bad_frame = frame("MESSAGE_DELETE", 1, map(vec![("id", text("123"))]));
    let (state, _) = apply_frame(state, &bad_frame);
    assert!(
        state
            .diagnostics
            .malformed
            .iter()
            .any(|m| m.contains("channel_id")),
        "должна быть зафиксирована ошибка malformed: {:?}",
        state.diagnostics.malformed
    );
}

#[test]
fn c15_channel_create_without_guild_id_records_inconsistency() {
    let state = minimal_state_with_guild(100);
    let (state, _) = apply(
        state,
        Event {
            seq: Some(1),
            payload: EventPayload::ChannelCreate(ChannelLifecycleDispatch {
                id: 999,
                guild_id: None,
                kind: Some(0),
                name: Some("no-guild".to_string()),
                parent_id: None,
            }),
            diagnostics: Default::default(),
        },
    );
    assert!(
        state
            .diagnostics
            .inconsistencies
            .contains(&"channels:999:missing_guild".to_string())
    );
}
