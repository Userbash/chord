//! W19 / P2-10: reducer voice states.
//!
//! Оракул — `003-READY_SUPPLEMENTAL.json`, live-события
//! `waves/RW-19/RESEARCH-P2-10.md` §2 и замороженный контракт
//! `waves/RW-19/DESIGN-P2-10.md` §5. Проверяются вход/выход/перемещение,
//! member upsert, снимок, таймер/статус и stale-политика.
//!
//! До интеграции `state/voice` и `EventPayload` — ожидаемый compile-red.

mod support;

use chord_core::codec::etf::EtfValue;
use chord_core::state::voice::{
    VoiceChannelStartTime, VoiceChannelStatus, VoiceState, apply_snapshot, apply_start_time,
    apply_status,
};
use chord_core::state::{Event, EventPayload, State, apply_frame};
use support::state_fixture::{
    atom, channel, channel_info_frame, frame, int, list, load, map, minimal_ready_frame, text,
};

const GUILD: u64 = 1_014_537_851_418_980_372;
const USER: u64 = 384_299_970_632_876_032;
const CHANNEL: u64 = 1_176_785_373_989_445_703;

fn minimal_state() -> State {
    let event = Event::from_frame(&minimal_ready_frame()).expect("minimal READY");
    let EventPayload::Ready(ready) = event.payload else {
        panic!("ожидался READY");
    };
    State::from_ready(&ready)
}

fn voice_frame(
    seq: u64,
    guild_id: u64,
    user_id: u64,
    channel_id: Option<u64>,
    connected_at: i64,
    member: Option<&str>,
) -> EtfValue {
    let guild = guild_id.to_string();
    let user = user_id.to_string();
    let channel_id = channel_id.map_or_else(|| atom("nil"), |id| text(&id.to_string()));
    let mut pairs = vec![
        ("guild_id", text(&guild)),
        ("user_id", text(&user)),
        ("channel_id", channel_id),
        ("connected_at", int(connected_at)),
        ("mute", atom("false")),
        ("deaf", atom("false")),
        ("self_mute", atom("false")),
        ("self_deaf", atom("false")),
        ("self_video", atom("false")),
        ("suppress", atom("false")),
        ("request_to_speak_timestamp", atom("nil")),
        ("session_id", text("REDACTED")),
    ];
    if let Some(username) = member {
        pairs.push((
            "member",
            map(vec![(
                "user",
                map(vec![("id", text(&user)), ("username", text(username))]),
            )]),
        ));
    }
    frame("VOICE_STATE_UPDATE", seq, map(pairs))
}

fn voice_frame_without_user(seq: u64, guild_id: u64, channel_id: u64) -> EtfValue {
    frame(
        "VOICE_STATE_UPDATE",
        seq,
        map(vec![
            ("guild_id", text(&guild_id.to_string())),
            ("channel_id", text(&channel_id.to_string())),
        ]),
    )
}

fn snapshot_voice(user_id: u64, channel_id: Option<u64>, connected_at: i64) -> EtfValue {
    let channel_id = channel_id.map_or_else(|| atom("nil"), |id| text(&id.to_string()));
    map(vec![
        ("user_id", text(&user_id.to_string())),
        ("channel_id", channel_id),
        ("connected_at", int(connected_at)),
        ("mute", atom("false")),
        ("deaf", atom("false")),
        ("self_mute", atom("false")),
        ("self_deaf", atom("false")),
        ("self_video", atom("false")),
        ("self_stream", atom("false")),
        ("suppress", atom("false")),
        ("discoverable", atom("true")),
        ("request_to_speak_timestamp", atom("nil")),
    ])
}

fn supplemental_frame(seq: u64, guild_id: u64, voices: Vec<EtfValue>) -> EtfValue {
    frame(
        "READY_SUPPLEMENTAL",
        seq,
        map(vec![(
            "guilds",
            list(vec![map(vec![
                ("id", text(&guild_id.to_string())),
                ("voice_states", list(voices)),
            ])]),
        )]),
    )
}

/// v07: join — запись в карте, member в `State.users`, `Guild.members` не изменён.
#[test]
fn v07_join_inserts_voice_and_member_user() {
    let state = apply_frame(
        minimal_state(),
        &channel_info_frame(
            1,
            "1014537851418980372",
            vec![channel("1176785373989445703", "voice")],
        ),
    )
    .0;
    let guilds_before = state.guilds.clone();

    let (state, effects) = apply_frame(
        state,
        &voice_frame(
            111,
            GUILD,
            USER,
            Some(CHANNEL),
            1_792_391_032,
            Some("bg-capture"),
        ),
    );

    assert_eq!(state.voice_state_count(), 1);
    assert_eq!(state.voice_channel_of(GUILD, USER), Some(CHANNEL));
    let member_user = state.users.get(&USER).expect("member upsert в State.users");
    assert_eq!(member_user.id, USER);
    assert_eq!(member_user.username.as_deref(), Some("bg-capture"));
    assert_eq!(state.guilds, guilds_before);
    assert!(effects.is_empty());
}

/// v08: join → move → leave; last wins, повторный leave — no-op, duplicates пуст.
#[test]
fn v08_move_and_leave_last_wins() {
    let (state, _) = apply_frame(
        minimal_state(),
        &voice_frame(1, GUILD, USER, Some(CHANNEL), 100, None),
    );
    let (state, _) = apply_frame(
        state,
        &voice_frame(2, GUILD, USER, Some(CHANNEL + 1), 200, None),
    );
    let voice = state.voice_state(GUILD, USER).expect("запись после move");
    assert_eq!(voice.channel_id, Some(CHANNEL + 1));
    assert_eq!(voice.connected_at, Some(200));

    let (state, _) = apply_frame(state, &voice_frame(3, GUILD, USER, None, 200, None));
    assert_eq!(state.voice_state_count(), 0);

    let before = state.voice_state(GUILD, USER).cloned();
    let (state, _) = apply_frame(state, &voice_frame(4, GUILD, USER, None, 200, None));
    assert_eq!(state.voice_state(GUILD, USER).cloned(), before);
    assert!(state.diagnostics.duplicates.is_empty());
}

/// v09: снимок `003` аддитивен, затем `READY` сбрасывает карты и `last_seq`.
#[test]
fn v09_supplemental_snapshot_then_ready_reset() {
    let (mut state, _) = apply_frame(minimal_state(), &load("003-READY_SUPPLEMENTAL.json"));
    assert_eq!(state.voice_state_count(), 1);
    let voice = state
        .voice_state(274_093_052_673_392_641, 289_091_818_442_719_242)
        .expect("запись из снимка 003");
    assert_eq!(voice.channel_id, Some(351_403_515_723_120_651));
    assert_eq!(voice.connected_at, Some(1_792_367_828));

    apply_start_time(
        &mut state,
        VoiceChannelStartTime {
            guild_id: GUILD,
            channel_id: CHANNEL,
            voice_start_time: Some(1_792_391_032),
        },
    );
    apply_status(
        &mut state,
        VoiceChannelStatus {
            guild_id: GUILD,
            channel_id: CHANNEL,
            status: Some("Music".to_string()),
        },
    );

    let (state, _) = apply_frame(state, &minimal_ready_frame());
    assert_eq!(state.voice_state_count(), 0);
    assert_eq!(state.voice_channel_start_time(GUILD, CHANNEL), None);
    assert_eq!(state.voice_channel_status(GUILD, CHANNEL), None);
    assert_eq!(state.last_seq, Some(1));
}

/// v09b: снимок `003` дополняет непустой store, не заменяя существующую запись.
#[test]
fn v09b_snapshot_merges_into_existing_store() {
    const OTHER_GUILD: u64 = 1_014_537_851_418_980_373;
    const OTHER_USER: u64 = 384_299_970_632_876_033;
    const OTHER_CHANNEL: u64 = 1_176_785_373_989_445_704;

    let (state, _) = apply_frame(
        minimal_state(),
        &voice_frame(1, OTHER_GUILD, OTHER_USER, Some(OTHER_CHANNEL), 200, None),
    );
    let (state, _) = apply_frame(state, &load("003-READY_SUPPLEMENTAL.json"));

    assert_eq!(state.voice_state_count(), 2);
    assert_eq!(
        state.voice_channel_of(OTHER_GUILD, OTHER_USER),
        Some(OTHER_CHANNEL)
    );
    assert_eq!(
        state.voice_channel_of(274_093_052_673_392_641, 289_091_818_442_719_242),
        Some(351_403_515_723_120_651)
    );
}

/// v09c: дубликаты `(guild,user)` в снимке — last wins, без diagnostics.duplicates.
#[test]
fn v09c_snapshot_duplicate_last_wins() {
    let first = snapshot_voice(USER, Some(CHANNEL), 100);
    let second = snapshot_voice(USER, Some(CHANNEL + 1), 200);
    let (state, _) = apply_frame(
        minimal_state(),
        &supplemental_frame(2, GUILD, vec![first, second]),
    );

    let voice = state
        .voice_state(GUILD, USER)
        .expect("запись после duplicate");
    assert_eq!(voice.channel_id, Some(CHANNEL + 1));
    assert_eq!(voice.connected_at, Some(200));
    assert_eq!(state.voice_state_count(), 1);
    assert!(state.diagnostics.duplicates.is_empty());
}

/// v09d: прямой `apply_snapshot` с `channel_id: None` удаляет существующую запись.
#[test]
fn v09d_snapshot_remove_branch() {
    let voice = VoiceState {
        guild_id: GUILD,
        user_id: USER,
        channel_id: Some(CHANNEL),
        connected_at: Some(100),
        mute: false,
        deaf: false,
        self_mute: false,
        self_deaf: false,
        self_video: false,
        self_stream: false,
        suppress: false,
        discoverable: true,
        request_to_speak_timestamp: None,
    };
    let mut state = minimal_state();
    apply_snapshot(&mut state, vec![voice.clone()]);
    assert_eq!(state.voice_channel_of(GUILD, USER), Some(CHANNEL));

    let leaving = VoiceState {
        channel_id: None,
        connected_at: Some(200),
        ..voice
    };
    apply_snapshot(&mut state, vec![leaving]);
    assert_eq!(state.voice_state_count(), 0);
}

/// v10: timer/status set/clear и границы (nil, пустая строка, отрицательный timer).
#[test]
fn v10_timer_and_status_boundaries() {
    let mut state = minimal_state();

    apply_start_time(
        &mut state,
        VoiceChannelStartTime {
            guild_id: GUILD,
            channel_id: CHANNEL,
            voice_start_time: Some(1_792_391_032),
        },
    );
    assert_eq!(
        state.voice_channel_start_time(GUILD, CHANNEL),
        Some(1_792_391_032)
    );

    apply_start_time(
        &mut state,
        VoiceChannelStartTime {
            guild_id: GUILD,
            channel_id: CHANNEL,
            voice_start_time: None,
        },
    );
    assert_eq!(state.voice_channel_start_time(GUILD, CHANNEL), None);

    apply_start_time(
        &mut state,
        VoiceChannelStartTime {
            guild_id: GUILD,
            channel_id: CHANNEL,
            voice_start_time: Some(100),
        },
    );
    // Ноль — валидное стартовое время (epoch), а не очистка.
    apply_start_time(
        &mut state,
        VoiceChannelStartTime {
            guild_id: GUILD,
            channel_id: CHANNEL,
            voice_start_time: Some(0),
        },
    );
    assert_eq!(state.voice_channel_start_time(GUILD, CHANNEL), Some(0));
    assert_eq!(
        state.voice_channel_elapsed_ms(GUILD, CHANNEL, 1_500),
        Some(1_500)
    );
    // Парсер не пропускает отрицательный timer; reducer обязан не паниковать и убрать запись.
    apply_start_time(
        &mut state,
        VoiceChannelStartTime {
            guild_id: GUILD,
            channel_id: CHANNEL,
            voice_start_time: Some(-1),
        },
    );
    assert_eq!(state.voice_channel_start_time(GUILD, CHANNEL), None);

    apply_status(
        &mut state,
        VoiceChannelStatus {
            guild_id: GUILD,
            channel_id: CHANNEL,
            status: Some("Music".to_string()),
        },
    );
    assert_eq!(state.voice_channel_status(GUILD, CHANNEL), Some("Music"));

    apply_status(
        &mut state,
        VoiceChannelStatus {
            guild_id: GUILD,
            channel_id: CHANNEL,
            status: Some(String::new()),
        },
    );
    assert_eq!(state.voice_channel_status(GUILD, CHANNEL), None);

    apply_status(
        &mut state,
        VoiceChannelStatus {
            guild_id: GUILD,
            channel_id: CHANNEL,
            status: Some("Music".to_string()),
        },
    );
    apply_status(
        &mut state,
        VoiceChannelStatus {
            guild_id: GUILD,
            channel_id: CHANNEL,
            status: None,
        },
    );
    assert_eq!(state.voice_channel_status(GUILD, CHANNEL), None);
}

/// v11: stale `s=51` не откатывает карту; malformed даёт `event:` и не откатывает seq.
#[test]
fn v11_stale_and_malformed_events() {
    let (state, _) = apply_frame(
        minimal_state(),
        &voice_frame(100, GUILD, USER, Some(CHANNEL), 100, None),
    );
    let joined = state
        .voice_state(GUILD, USER)
        .cloned()
        .expect("запись после join");

    let (state, _) = apply_frame(state, &voice_frame(51, GUILD, USER, None, 100, None));
    assert_eq!(state.voice_state(GUILD, USER), Some(&joined));
    assert!(state.diagnostics.stale_sequences.contains(&51));
    assert_eq!(state.last_seq, Some(100));

    let (state, _) = apply_frame(state, &voice_frame_without_user(101, GUILD, CHANNEL));
    assert_eq!(state.voice_state(GUILD, USER), Some(&joined));
    assert!(
        state
            .diagnostics
            .malformed
            .iter()
            .any(|entry| entry.starts_with("event:"))
    );
    assert_eq!(state.last_seq, Some(101));
}
