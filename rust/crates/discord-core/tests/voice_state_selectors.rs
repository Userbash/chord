//! W19 / P2-10: селекторы voice states.
//!
//! Оракул — замороженный контракт `waves/RW-19/DESIGN-P2-10.md` §6 и границы
//! `waves/RW-19/TEST-MATRIX-P2-10.md` (сортировка, сатурация, несколько гильдий).
//! Все методы только читают карты и не паникуют на отсутствующих ключах.
//!
//! До интеграции `state/voice` и полей `State` — ожидаемый compile-red.

mod support;

use discord_core::codec::etf::EtfValue;
use discord_core::state::voice::{
    VoiceChannelStartTime, VoiceChannelStatus, VoiceState, VoiceStateUpdate, apply_start_time,
    apply_status, apply_update,
};
use discord_core::state::{Event, EventPayload, State, apply_frame};
use support::state_fixture::{atom, frame, int, map, minimal_ready_frame, text};

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
    channel_id: u64,
    connected_at: i64,
) -> EtfValue {
    frame(
        "VOICE_STATE_UPDATE",
        seq,
        map(vec![
            ("guild_id", text(&guild_id.to_string())),
            ("user_id", text(&user_id.to_string())),
            ("channel_id", text(&channel_id.to_string())),
            ("connected_at", int(connected_at)),
            ("mute", atom("false")),
            ("deaf", atom("false")),
            ("self_mute", atom("false")),
            ("self_deaf", atom("false")),
            ("self_video", atom("false")),
            ("suppress", atom("false")),
            ("request_to_speak_timestamp", atom("nil")),
        ]),
    )
}

fn voice(
    guild_id: u64,
    user_id: u64,
    channel_id: Option<u64>,
    connected_at: Option<i64>,
) -> VoiceState {
    VoiceState {
        guild_id,
        user_id,
        channel_id,
        connected_at,
        mute: false,
        deaf: false,
        self_mute: false,
        self_deaf: false,
        self_video: false,
        self_stream: false,
        suppress: false,
        discoverable: true,
        request_to_speak_timestamp: None,
    }
}

/// v12: пустой store и собственный voice; после join — Some/true.
#[test]
fn v12_self_voice_state() {
    let state = minimal_state();
    assert!(state.self_voice_state().is_none());
    assert!(!state.is_self_in_voice());

    let (state, _) = apply_frame(state, &voice_frame(1, 1, 100, 10, 100));
    let voice = state.self_voice_state().expect("self voice после join");
    assert_eq!(voice.user_id, 100);
    assert_eq!(voice.channel_id, Some(10));
    assert!(state.is_self_in_voice());
}

/// v13: пользователи одного канала по возрастанию id, счётчики и число гильдий.
#[test]
fn v13_channel_users_sorted_and_counts() {
    let mut state = minimal_state();
    for (seq, user_id) in [(1_u64, 300_u64), (2, 100), (3, 200)] {
        let (next, _) = apply_frame(state, &voice_frame(seq, 1, user_id, 10, 100));
        state = next;
    }
    // Тот же channel_id в другой гильдии не должен попасть в выборку guild 1.
    let (next, _) = apply_frame(state, &voice_frame(4, 2, 50, 10, 100));
    state = next;

    let user_ids: Vec<u64> = state
        .voice_users_in_channel(1, 10)
        .into_iter()
        .map(|voice| voice.user_id)
        .collect();
    assert_eq!(user_ids, vec![100, 200, 300]);
    assert_eq!(state.voice_channel_user_count(1, 10), 3);
    assert_eq!(state.voice_channel_user_count(2, 10), 1);
    assert_eq!(state.voice_channel_user_count(1, 20), 0);
    assert_eq!(state.voice_state_count(), 4);
    assert_eq!(state.voice_guild_count(), 2);
}

/// v14: `connected_at` в секундах, сатурация и границы (negative/overflow).
#[test]
fn v14_connected_duration_boundaries() {
    let mut state = minimal_state();
    for (user_id, connected_at) in [
        (1, Some(100)),
        (2, Some(-1)),
        (3, Some(i64::MAX)),
        (4, None),
    ] {
        apply_update(
            &mut state,
            VoiceStateUpdate {
                state: voice(1, user_id, Some(10), connected_at),
                member_user: None,
            },
        );
    }

    assert_eq!(state.voice_connected_for_ms(1, 1, 100_000), Some(0));
    assert_eq!(state.voice_connected_for_ms(1, 1, 199_900), Some(99_900));
    assert_eq!(state.voice_connected_for_ms(1, 1, 99_000), Some(0));
    assert_eq!(
        state.voice_connected_for_ms(1, 1, u64::MAX),
        Some(u64::MAX - 100_000)
    );
    assert_eq!(state.voice_connected_for_ms(1, 2, 100_000), None);
    assert_eq!(state.voice_connected_for_ms(1, 3, 100_000), None);
    assert_eq!(state.voice_connected_for_ms(1, 4, 100_000), None);
    assert_eq!(state.voice_connected_for_ms(1, 99, 100_000), None);

    assert_eq!(
        state.voice_state(1, 1).expect("запись").connected_at_ms(),
        Some(100_000)
    );
    assert_eq!(
        state.voice_state(1, 2).expect("запись").connected_at_ms(),
        None
    );
    assert_eq!(
        state.voice_state(1, 3).expect("запись").connected_at_ms(),
        None
    );
    assert_eq!(
        state.voice_state(1, 4).expect("запись").connected_at_ms(),
        None
    );
}

/// v15: timer/status по каналу, elapsed, пустые значения и независимость гильдий.
#[test]
fn v15_channel_timer_and_status() {
    let mut state = minimal_state();
    apply_start_time(
        &mut state,
        VoiceChannelStartTime {
            guild_id: 1,
            channel_id: 10,
            voice_start_time: Some(100),
        },
    );
    apply_status(
        &mut state,
        VoiceChannelStatus {
            guild_id: 1,
            channel_id: 10,
            status: Some("Music".to_string()),
        },
    );
    apply_start_time(
        &mut state,
        VoiceChannelStartTime {
            guild_id: 2,
            channel_id: 10,
            voice_start_time: Some(200),
        },
    );
    apply_status(
        &mut state,
        VoiceChannelStatus {
            guild_id: 2,
            channel_id: 10,
            status: Some("Other".to_string()),
        },
    );

    assert_eq!(state.voice_channel_start_time(1, 10), Some(100));
    assert_eq!(state.voice_channel_elapsed_ms(1, 10, 199_900), Some(99_900));
    assert_eq!(state.voice_channel_status(1, 10), Some("Music"));

    assert_eq!(state.voice_channel_start_time(2, 10), Some(200));
    assert_eq!(state.voice_channel_elapsed_ms(2, 10, 200_500), Some(500));
    assert_eq!(state.voice_channel_status(2, 10), Some("Other"));

    assert_eq!(state.voice_channel_start_time(1, 11), None);
    assert_eq!(state.voice_channel_elapsed_ms(1, 11, 199_900), None);
    assert_eq!(state.voice_channel_status(1, 11), None);

    apply_status(
        &mut state,
        VoiceChannelStatus {
            guild_id: 1,
            channel_id: 10,
            status: Some(String::new()),
        },
    );
    assert_eq!(state.voice_channel_status(1, 10), None);
    apply_start_time(
        &mut state,
        VoiceChannelStartTime {
            guild_id: 1,
            channel_id: 10,
            voice_start_time: Some(-1),
        },
    );
    assert_eq!(state.voice_channel_start_time(1, 10), None);
    assert_eq!(state.voice_channel_elapsed_ms(1, 10, 199_900), None);
    // Соседняя гильдия не затронута очисткой.
    assert_eq!(state.voice_channel_start_time(2, 10), Some(200));
    assert_eq!(state.voice_channel_status(2, 10), Some("Other"));
}
