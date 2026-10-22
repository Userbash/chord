//! W13a / P2-02a: типы и разбор событий состояния.
//!
//! Оракулы: `fixtures/gateway/by-type/*` (md5 в `waves/RW-13/RESEARCH.md` §1), числа `jq`
//! из `waves/RW-13/DESIGN-P2-02.md` §10. Санитайзенные отрицательные id в
//! `THREAD_LIST_SYNC`/`CHANNEL_INFO` проверяются как диагностика/отказ; валидный путь —
//! синтетика с теми же значениями полей.

mod support;

use discord_core::dispatch::DispatchKind;
use discord_core::ready::Ready;
use discord_core::state::{
    ChannelInfo, Event, EventError, EventPayload, ReadySupplemental, State, ThreadListSync,
};

use support::state_fixture::{atom, frame, int, list, load, map, text};

fn event(name: &str) -> Event {
    let frame = load(name);
    Event::from_frame(&frame).unwrap_or_else(|error| panic!("{name}: {error}"))
}

/// T1: READY — op0/s=1, типизированный payload.
#[test]
fn t01_ready_event_parses_real_fixture() {
    let event = event("002-READY.json");
    assert_eq!(event.seq, Some(1));
    let EventPayload::Ready(ready) = event.payload else {
        panic!("ожидался EventPayload::Ready");
    };
    assert_eq!(ready.guilds.len(), 7);
    assert_eq!(ready.session_id, "REDACTED");
    assert_eq!(ready.user.id, 384_299_970_632_876_032);
}

/// T2: READY_SUPPLEMENTAL — 7 групп, 138 сырых / 137 после dedup, первый участник из capture.
#[test]
fn t02_ready_supplemental_groups_match_oracle() {
    let event = event("003-READY_SUPPLEMENTAL.json");
    assert_eq!(event.seq, Some(2));
    let EventPayload::ReadySupplemental(supplemental) = event.payload else {
        panic!("ожидался EventPayload::ReadySupplemental");
    };
    assert_eq!(supplemental.guild_ids.len(), 7);
    assert!(supplemental.guild_ids.iter().all(Option::is_some));
    let counts: Vec<usize> = supplemental.merged_members.iter().map(Vec::len).collect();
    // 138 сырых записей; в первой группе дубликат user_id (P2-01 dedup last wins).
    assert_eq!(counts, vec![16, 14, 7, 2, 10, 87, 1]);
    assert_eq!(counts.iter().sum::<usize>(), 137);

    let first = &supplemental.merged_members[0][0];
    assert_eq!(first.user_id, 289_091_818_442_719_242);
    assert_eq!(first.nick.as_deref(), Some("𝓚𝓞𝓣_𝓑𝓐𝓨𝓤𝓝(Рома)"));
    assert_eq!(first.roles.len(), 8);
    assert!(event.diagnostics.malformed.is_empty());
}

/// T3: SESSIONS_REPLACE — `d` массив, замена списка сессий.
#[test]
fn t03_sessions_replace_parses_array() {
    let event = event("004-SESSIONS_REPLACE.json");
    assert_eq!(event.seq, Some(3));
    let EventPayload::SessionsReplace(sessions) = event.payload else {
        panic!("ожидался EventPayload::SessionsReplace");
    };
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id, "REDACTED");
    assert_eq!(sessions[0].status.as_deref(), Some("invisible"));
    assert_eq!(sessions[0].client_info.client.as_deref(), Some("desktop"));
    assert_eq!(sessions[0].client_info.os.as_deref(), Some("linux"));
    assert_eq!(sessions[0].client_info.version, 0);
    assert!(event.diagnostics.malformed.is_empty());
}

/// T4: реальный THREAD_LIST_SYNC санитайзен (отрицательный guild_id) — отказ.
#[test]
fn t04_thread_fixture_negative_guild_id_is_invalid() {
    let error = Event::from_frame(&load("004-THREAD_LIST_SYNC.json"))
        .expect_err("отрицательный guild_id невалиден");
    assert!(
        matches!(
            &error,
            EventError::InvalidId { path, .. } if path == "thread_list_sync.guild_id"
        ),
        "получено {error:?}"
    );
}

/// T4b: синтетический THREAD_LIST_SYNC с валидными id — поля из реальной фикстуры.
#[test]
fn t04b_thread_synthetic_fields_from_fixture() {
    let thread = support::state_fixture::thread(
        "531496018449321446",
        "146727293132525260",
        "530758416657182848",
        "Тред",
    );
    let frame = support::state_fixture::thread_sync_frame(3, "146727293132525260", vec![thread]);
    let event = Event::from_frame(&frame).expect("валидный THREAD_LIST_SYNC");
    let EventPayload::ThreadListSync(ThreadListSync { guild_id, threads }) = event.payload else {
        panic!("ожидался ThreadListSync");
    };
    assert_eq!(guild_id, 146_727_293_132_525_260);
    assert_eq!(threads.len(), 1);
    let thread = &threads[0];
    assert_eq!(thread.id, 531_496_018_449_321_446);
    assert_eq!(thread.thread_type, 11);
    assert_eq!(thread.message_count, 5);
    assert_eq!(thread.member_count, 0);
    assert_eq!(thread.total_message_sent, 10);
    assert_eq!(thread.flags, 2);
    assert!(!thread.archived);
    assert!(!thread.locked);
    assert!(thread.invitable);
    assert_eq!(thread.auto_archive_duration, Some(60));
    assert_eq!(
        thread.archive_timestamp.as_deref(),
        Some("2024-05-30T14:17:04.195000+00:00")
    );
}

/// T5: CHANNEL_INFO — поля канала из READY-оракула.
#[test]
fn t05_channel_info_synthetic_channel_fields() {
    let channel = map(vec![
        ("id", text("751425447614480414")),
        ("type", int(0)),
        ("name", text("___hidden___")),
        ("parent_id", atom("nil")),
        ("position", int(24)),
        ("flags", int(131_072)),
        ("nsfw", atom("false")),
        ("topic", atom("nil")),
        ("last_message_id", atom("nil")),
        (
            "permission_overwrites",
            list(vec![map(vec![
                ("id", text("274093052673392641")),
                ("type", int(0)),
                ("allow", text("0")),
                ("deny", text("1024")),
            ])]),
        ),
        ("rate_limit_per_user", int(0)),
        ("bitrate", atom("nil")),
        ("user_limit", atom("nil")),
        ("rtc_region", atom("nil")),
        ("video_quality_mode", atom("nil")),
        ("default_auto_archive_duration", atom("nil")),
    ]);
    let frame = support::state_fixture::channel_info_frame(10, "274093052673392641", vec![channel]);
    let event = Event::from_frame(&frame).expect("валидный CHANNEL_INFO");
    let EventPayload::ChannelInfo(ChannelInfo { guild_id, channels }) = event.payload else {
        panic!("ожидался ChannelInfo");
    };
    assert_eq!(guild_id, 274_093_052_673_392_641);
    assert_eq!(channels.len(), 1);
    let channel = &channels[0];
    assert_eq!(channel.id, 751_425_447_614_480_414);
    assert_eq!(channel.name.as_deref(), Some("___hidden___"));
    assert_eq!(channel.position, 24);
    assert_eq!(channel.flags, 131_072);
    assert!(!channel.nsfw);
    assert_eq!(channel.parent_id, None);
    assert_eq!(channel.permission_overwrites.len(), 1);
    assert_eq!(channel.permission_overwrites[0].deny, 1024);
}

/// T5b: реальный CHANNEL_INFO — 75 каналов с отрицательными id пропущены.
#[test]
fn t05b_channel_info_negative_ids_are_diagnostics() {
    let fixture = load("012-CHANNEL_INFO.json");
    let channels = fixture
        .get("d")
        .and_then(|d| d.get("channels"))
        .expect("d.channels")
        .clone();
    let frame = map(vec![
        ("op", int(0)),
        ("t", atom("CHANNEL_INFO")),
        ("s", int(10)),
        (
            "d",
            map(vec![("guild_id", text("1")), ("channels", channels)]),
        ),
    ]);
    let event = Event::from_frame(&frame).expect("конверт валиден");
    let EventPayload::ChannelInfo(info) = event.payload else {
        panic!("ожидался ChannelInfo");
    };
    assert!(info.channels.is_empty());
    assert_eq!(event.diagnostics.malformed.len(), 75);
}

/// T6: MESSAGE_CREATE — реальные id/content/author.
#[test]
fn t06_message_create_real_fixture() {
    let event = event("023-MESSAGE_CREATE.json");
    assert_eq!(event.seq, Some(18));
    let EventPayload::MessageCreate(message) = event.payload else {
        panic!("ожидался MessageCreate");
    };
    assert_eq!(message.id, 1_561_627_527_296_979_046);
    assert_eq!(message.channel_id, 1_163_408_591_315_812_392);
    assert_eq!(message.guild_id, Some(1_014_537_851_418_980_372));
    assert_eq!(message.content.chars().count(), 208);
    assert_eq!(
        message.timestamp.as_deref(),
        Some("2026-10-19T06:30:16.144000+00:00")
    );
    assert_eq!(message.edited_timestamp, None);
    assert_eq!(message.message_type, 0);
    assert_eq!(message.flags, 0);
    assert!(!message.pinned);
    assert!(!message.tts);
    assert!(!message.mention_everyone);
    assert!(message.mention_user_ids.is_empty());
    assert!(message.mention_role_ids.is_empty());
    assert_eq!(message.attachment_count, 0);
    assert_eq!(message.embed_count, 0);
    let author = message.author.expect("author");
    assert_eq!(author.id, 268_478_587_651_358_721);
    assert!(author.bot);
}

/// T7: MESSAGE_UPDATE — content длиной 208 и один embed.
#[test]
fn t07_message_update_real_fixture() {
    let event = event("026-MESSAGE_UPDATE.json");
    assert_eq!(event.seq, Some(21));
    let EventPayload::MessageUpdate(patch) = event.payload else {
        panic!("ожидался MessageUpdate");
    };
    assert_eq!(patch.id, 1_561_627_527_296_979_046);
    assert_eq!(patch.channel_id, Some(1_163_408_591_315_812_392));
    assert_eq!(
        patch
            .content
            .as_ref()
            .map(|content| content.chars().count()),
        Some(208)
    );
    assert_eq!(patch.embed_count, Some(1));
    assert_eq!(patch.flags, Some(0));
    assert_eq!(patch.pinned, Some(false));
    assert_eq!(patch.author.expect("author").id, 268_478_587_651_358_721);
}

/// T8: неизвестный `t` — не ошибка; известный не-поддержанный — `Unsupported`.
#[test]
fn t08_unknown_and_unsupported_events() {
    let unknown = frame("FUTURE_EVENT", 7, map(Vec::new()));
    let event = Event::from_frame(&unknown).expect("неизвестный тип не ошибка");
    assert_eq!(event.seq, Some(7));
    assert!(matches!(
        event.payload,
        EventPayload::Unknown { ref name } if name == "FUTURE_EVENT"
    ));

    let presence = frame("PRESENCE_UPDATE", 8, map(Vec::new()));
    let event = Event::from_frame(&presence).expect("PRESENCE_UPDATE не ошибка");
    assert!(matches!(
        event.payload,
        EventPayload::Unsupported(DispatchKind::PresenceUpdate)
    ));
}

/// T9: MESSAGE_CREATE без channel_id — `Missing("message.channel_id")`.
#[test]
fn t09_message_without_channel_is_error() {
    let frame = frame(
        "MESSAGE_CREATE",
        5,
        map(vec![("id", text("1")), ("content", text("x"))]),
    );
    let error = Event::from_frame(&frame).expect_err("channel_id обязателен");
    assert!(
        matches!(
            &error,
            EventError::Missing { path } if path == "message.channel_id"
        ),
        "получено {error:?}"
    );
}

/// T10: SESSIONS_REPLACE с объектом вместо массива — `WrongType`.
#[test]
fn t10_sessions_replace_wrong_type() {
    let frame = frame("SESSIONS_REPLACE", 3, map(Vec::new()));
    let error = Event::from_frame(&frame).expect_err("нужен массив");
    assert!(
        matches!(
            &error,
            EventError::WrongType { path, .. } if path == "sessions_replace"
        ),
        "получено {error:?}"
    );
}

/// T11: `State::Debug` редактирует session_id и user_settings_proto.
#[test]
fn t11_state_debug_redacts_secrets() {
    let ready = Ready::from_dispatch(&load("002-READY.json")).expect("READY");
    let session = ready.session_id.clone();
    let proto = ready.user_settings_proto.clone();
    let state = State::from_ready(&ready);
    let debug = format!("{state:?}");
    assert!(debug.contains("State"));
    assert!(!debug.contains(&session));
    if let Some(proto) = proto {
        assert!(!debug.contains(&proto));
    }
}

/// T12: расхождение длины merged_members/guilds — диагностика, не ошибка.
#[test]
fn t12_supplemental_length_mismatch_is_diagnostic() {
    let member = map(vec![("user_id", text("1"))]);
    let frame = frame(
        "READY_SUPPLEMENTAL",
        2,
        map(vec![
            ("guilds", list(vec![map(vec![("id", text("10"))])])),
            (
                "merged_members",
                list(vec![list(vec![member.clone()]), list(vec![member])]),
            ),
        ]),
    );
    let event = Event::from_frame(&frame).expect("конверт валиден");
    let EventPayload::ReadySupplemental(ReadySupplemental { merged_members, .. }) = event.payload
    else {
        panic!("ожидался ReadySupplemental");
    };
    assert_eq!(merged_members.len(), 2);
    assert!(
        event
            .diagnostics
            .malformed
            .iter()
            .any(|item| item == "merged_members:2 vs guilds:1"),
        "диагностика: {:?}",
        event.diagnostics
    );
}

/// T13: минимальный READY — State без секрета в Debug.
#[test]
fn t13_minimal_ready_state_is_redacted() {
    let frame = support::state_fixture::minimal_ready_frame();
    let ready = Ready::from_dispatch(&frame).expect("minimal READY");
    let state = State::from_ready(&ready);
    assert_eq!(state.user.id, 100);
    assert_eq!(state.guilds.len(), 0);
    let debug = format!("{state:?}");
    assert!(!debug.contains("session-secret"));
}
