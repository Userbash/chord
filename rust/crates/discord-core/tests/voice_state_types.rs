//! W19 / P2-10: типы и разбор voice states.
//!
//! Оракул — реальный снимок `003-READY_SUPPLEMENTAL.json` (md5
//! `55983830547abaf8d1b122798fdcf568`) и live-значения
//! `waves/RW-19/RESEARCH-P2-10.md` §2. Синтетические кадры повторяют форму провода:
//! атомы `nil`/`true`/`false`, строки-id, map-ключи-атомы.
//!
//! До интеграции `state/voice` в `state/mod.rs` и `EventPayload` — ожидаемый compile-red.

mod support;

use discord_core::codec::etf::EtfValue;
use discord_core::state::voice::{VoiceChannelStartTime, VoiceChannelStatus, VoiceStateUpdate};
use discord_core::state::{Event, EventError, EventPayload, StateDiagnostics};
use support::state_fixture::{atom, frame, int, list, load, map, text};

fn event(frame: &EtfValue) -> (EventPayload, StateDiagnostics) {
    let Event {
        payload,
        diagnostics,
        ..
    } = Event::from_frame(frame).expect("кадр должен разбираться");
    (payload, diagnostics)
}

fn voice_update(frame: &EtfValue) -> (VoiceStateUpdate, StateDiagnostics) {
    let (payload, diagnostics) = event(frame);
    match payload {
        EventPayload::VoiceStateUpdate(update) => (update, diagnostics),
        other => panic!("ожидался VoiceStateUpdate, получено {other:?}"),
    }
}

fn start_time(frame: &EtfValue) -> (VoiceChannelStartTime, StateDiagnostics) {
    let (payload, diagnostics) = event(frame);
    match payload {
        EventPayload::VoiceChannelStartTime(update) => (update, diagnostics),
        other => panic!("ожидался VoiceChannelStartTime, получено {other:?}"),
    }
}

fn status(frame: &EtfValue) -> (VoiceChannelStatus, StateDiagnostics) {
    let (payload, diagnostics) = event(frame);
    match payload {
        EventPayload::VoiceChannelStatus(update) => (update, diagnostics),
        other => panic!("ожидался VoiceChannelStatus, получено {other:?}"),
    }
}

fn parse_err(frame: &EtfValue) -> EventError {
    Event::from_frame(frame).expect_err("кадр должен быть отвергнут")
}

fn voice_frame(seq: u64, pairs: Vec<(&str, EtfValue)>) -> EtfValue {
    frame("VOICE_STATE_UPDATE", seq, map(pairs))
}

fn member(user_id: &str, username: &str) -> EtfValue {
    map(vec![(
        "user",
        map(vec![("id", text(user_id)), ("username", text(username))]),
    )])
}

fn start_time_frame(seq: u64, value: EtfValue) -> EtfValue {
    frame(
        "VOICE_CHANNEL_START_TIME_UPDATE",
        seq,
        map(vec![
            ("guild_id", text("1014537851418980372")),
            ("id", text("1176785373989445703")),
            ("voice_start_time", value),
        ]),
    )
}

fn status_frame(seq: u64, value: EtfValue) -> EtfValue {
    frame(
        "VOICE_CHANNEL_STATUS_UPDATE",
        seq,
        map(vec![
            ("guild_id", text("1014537851418980372")),
            ("id", text("1176785373989445703")),
            ("status", value),
        ]),
    )
}

/// v01: реальный снимок `003` — одна запись, точные id/флаги/`connected_at`.
#[test]
fn v01_snapshot_003_exact_fields() {
    let (payload, diagnostics) = event(&load("003-READY_SUPPLEMENTAL.json"));
    // `003` содержит дубликат в `merged_members[0]`; это диагностика P2-01, не voice.
    // Поэтому проверяем отсутствие именно voice-диагностики.
    assert!(diagnostics.malformed.is_empty());
    assert!(diagnostics.ignored.is_empty());
    assert!(diagnostics.inconsistencies.is_empty());
    assert!(
        diagnostics
            .duplicates
            .iter()
            .all(|entry| !entry.starts_with("voice")),
        "voice diagnostics должны быть пусты: {:?}",
        diagnostics.duplicates
    );
    let EventPayload::ReadySupplemental(supplemental) = payload else {
        panic!("ожидался ReadySupplemental");
    };
    assert_eq!(supplemental.voice_states.len(), 1);
    let voice = &supplemental.voice_states[0];
    assert_eq!(voice.guild_id, 274_093_052_673_392_641);
    assert_eq!(voice.channel_id, Some(351_403_515_723_120_651));
    assert_eq!(voice.user_id, 289_091_818_442_719_242);
    assert_eq!(voice.connected_at, Some(1_792_367_828));
    assert_eq!(voice.connected_at_ms(), Some(1_792_367_828_000));
    assert!(voice.is_connected());
    assert!(!voice.mute);
    assert!(!voice.deaf);
    assert!(!voice.self_mute);
    assert!(!voice.self_deaf);
    assert!(!voice.self_video);
    assert!(!voice.self_stream);
    assert!(!voice.suppress);
    assert!(voice.discoverable);
    assert_eq!(voice.request_to_speak_timestamp, None);
}

/// v02: live join `s=111` — member.user, `self_mute=true`, остальные флаги false.
#[test]
fn v02_live_join_111() {
    let frame = voice_frame(
        111,
        vec![
            ("guild_id", text("1014537851418980372")),
            ("user_id", text("384299970632876032")),
            ("channel_id", text("1176785373989445703")),
            ("connected_at", int(1_792_391_032)),
            ("mute", atom("false")),
            ("deaf", atom("false")),
            ("self_mute", atom("true")),
            ("self_deaf", atom("false")),
            ("self_video", atom("false")),
            ("suppress", atom("false")),
            ("request_to_speak_timestamp", atom("nil")),
            ("session_id", text("REDACTED")),
            ("member", member("384299970632876032", "bg-capture")),
        ],
    );
    let (update, diagnostics) = voice_update(&frame);
    assert_eq!(diagnostics, StateDiagnostics::default());
    let voice = &update.state;
    assert_eq!(voice.guild_id, 1_014_537_851_418_980_372);
    assert_eq!(voice.user_id, 384_299_970_632_876_032);
    assert_eq!(voice.channel_id, Some(1_176_785_373_989_445_703));
    assert_eq!(voice.connected_at, Some(1_792_391_032));
    assert!(!voice.mute);
    assert!(!voice.deaf);
    assert!(voice.self_mute);
    assert!(!voice.self_deaf);
    assert!(!voice.self_video);
    assert!(!voice.self_stream);
    assert!(!voice.suppress);
    assert!(voice.discoverable);
    assert_eq!(voice.request_to_speak_timestamp, None);
    let member_user = update.member_user.expect("member.user сохраняется");
    assert_eq!(member_user.id, voice.user_id);
    assert!(
        member_user
            .username
            .as_deref()
            .is_some_and(|username| !username.is_empty())
    );
}

/// v03: live leave `s=51` — `channel_id=None`, `connected_at` сохранён.
#[test]
fn v03_live_leave_51() {
    let frame = voice_frame(
        51,
        vec![
            ("guild_id", text("1014537851418980372")),
            ("user_id", text("384299970632876032")),
            ("channel_id", atom("nil")),
            ("connected_at", int(1_792_395_566)),
            ("mute", atom("false")),
            ("deaf", atom("false")),
            ("self_mute", atom("false")),
            ("self_deaf", atom("false")),
            ("self_video", atom("false")),
            ("suppress", atom("false")),
            ("request_to_speak_timestamp", atom("nil")),
            ("session_id", text("REDACTED")),
            ("member", member("384299970632876032", "bg-capture")),
        ],
    );
    let (update, diagnostics) = voice_update(&frame);
    assert_eq!(diagnostics, StateDiagnostics::default());
    let voice = &update.state;
    assert_eq!(voice.channel_id, None);
    assert_eq!(voice.connected_at, Some(1_792_395_566));
    assert!(!voice.is_connected());
    assert!(!voice.mute);
    assert!(!voice.deaf);
    assert!(!voice.self_mute);
    assert!(!voice.self_deaf);
    assert!(!voice.self_video);
    assert!(!voice.self_stream);
    assert!(!voice.suppress);
    assert!(voice.discoverable);
    assert!(update.member_user.is_some());
}

/// v04: timer numeric/nil и status nil/synthetic `"Music"`.
#[test]
fn v04_timer_and_status_values() {
    let (timer, diagnostics) = start_time(&start_time_frame(112, int(1_792_391_032)));
    assert_eq!(diagnostics, StateDiagnostics::default());
    assert_eq!(timer.guild_id, 1_014_537_851_418_980_372);
    assert_eq!(timer.channel_id, 1_176_785_373_989_445_703);
    assert_eq!(timer.voice_start_time, Some(1_792_391_032));

    let (timer, diagnostics) = start_time(&start_time_frame(52, atom("nil")));
    assert_eq!(diagnostics, StateDiagnostics::default());
    assert_eq!(timer.voice_start_time, None);

    let (clear, diagnostics) = status(&status_frame(53, atom("nil")));
    assert_eq!(diagnostics, StateDiagnostics::default());
    assert_eq!(clear.status, None);

    let (music, diagnostics) = status(&status_frame(54, text("Music")));
    assert_eq!(diagnostics, StateDiagnostics::default());
    assert_eq!(music.status.as_deref(), Some("Music"));
}

/// v05: таблица обязательных/optional полей — точные диагностические пути.
#[test]
fn v05_optional_and_broken_table() {
    let missing_guild = voice_frame(1, vec![("user_id", text("42")), ("channel_id", text("7"))]);
    assert!(matches!(
        parse_err(&missing_guild),
        EventError::Missing { ref path } if path == "voice.guild_id"
    ));

    let invalid_guild = voice_frame(
        2,
        vec![
            ("guild_id", text("not-a-snowflake")),
            ("user_id", text("42")),
            ("channel_id", text("7")),
        ],
    );
    assert!(matches!(
        parse_err(&invalid_guild),
        EventError::InvalidId { ref path, .. } if path == "voice.guild_id"
    ));

    let missing_user = voice_frame(3, vec![("guild_id", text("1")), ("channel_id", text("7"))]);
    assert!(matches!(
        parse_err(&missing_user),
        EventError::Missing { ref path } if path == "voice.user_id"
    ));

    let invalid_user = voice_frame(
        4,
        vec![
            ("guild_id", text("1")),
            ("user_id", text("bad")),
            ("channel_id", text("7")),
        ],
    );
    assert!(matches!(
        parse_err(&invalid_user),
        EventError::InvalidId { ref path, .. } if path == "voice.user_id"
    ));

    // `channel_id` неверного типа — ошибка, а не выход.
    let bad_channel = voice_frame(
        5,
        vec![
            ("guild_id", text("1")),
            ("user_id", text("2")),
            ("channel_id", atom("true")),
        ],
    );
    assert!(matches!(
        parse_err(&bad_channel),
        EventError::InvalidId { ref path, .. } if path == "voice.channel_id"
    ));

    let bad_flags = voice_frame(
        6,
        vec![
            ("guild_id", text("1")),
            ("user_id", text("2")),
            ("channel_id", text("3")),
            ("mute", text("bad")),
            ("deaf", int(2)),
            ("self_mute", atom("nil")),
            ("self_deaf", list(Vec::new())),
            ("self_video", text("true")),
            ("self_stream", text("bad")),
            ("suppress", map(Vec::new())),
            ("connected_at", text("1792391032")),
            ("request_to_speak_timestamp", text("1792391032")),
        ],
    );
    let (update, diagnostics) = voice_update(&bad_flags);
    let voice = &update.state;
    assert!(!voice.mute);
    assert!(!voice.deaf);
    assert!(!voice.self_mute);
    assert!(!voice.self_deaf);
    assert!(voice.self_video);
    assert!(!voice.self_stream);
    assert!(!voice.suppress);
    assert!(voice.discoverable);
    assert_eq!(voice.connected_at, Some(1_792_391_032));
    assert_eq!(voice.request_to_speak_timestamp, Some(1_792_391_032));
    for path in [
        "voice.mute",
        "voice.deaf",
        "voice.self_deaf",
        "voice.self_stream",
        "voice.suppress",
    ] {
        assert!(
            diagnostics.ignored.iter().any(|entry| entry == path),
            "нет {path} в {:?}",
            diagnostics.ignored
        );
    }
    assert!(
        !diagnostics
            .ignored
            .iter()
            .any(|entry| entry == "voice.self_mute")
    );
    assert!(
        !diagnostics
            .ignored
            .iter()
            .any(|entry| entry == "voice.self_video")
    );

    let bad_discoverable = voice_frame(
        7,
        vec![
            ("guild_id", text("1")),
            ("user_id", text("2")),
            ("channel_id", text("3")),
            ("discoverable", text("bad")),
        ],
    );
    let (update, diagnostics) = voice_update(&bad_discoverable);
    assert!(update.state.discoverable);
    assert!(
        diagnostics
            .ignored
            .iter()
            .any(|entry| entry == "voice.discoverable")
    );
}

/// v06: member string/без user/чужой id и `d` не map — без паник, с диагностикой.
#[test]
fn v06_member_mismatch_and_malformed() {
    let guild = "1014537851418980372";
    let user = "384299970632876032";

    let member_string = voice_frame(
        1,
        vec![
            ("guild_id", text(guild)),
            ("user_id", text(user)),
            ("channel_id", text("7")),
            ("member", text("broken")),
        ],
    );
    let (update, diagnostics) = voice_update(&member_string);
    assert!(update.member_user.is_none());
    assert!(
        diagnostics
            .ignored
            .iter()
            .any(|entry| entry == "voice.member")
    );

    let member_without_user = voice_frame(
        2,
        vec![
            ("guild_id", text(guild)),
            ("user_id", text(user)),
            ("channel_id", text("7")),
            ("member", map(vec![("nick", text("nick"))])),
        ],
    );
    let (update, diagnostics) = voice_update(&member_without_user);
    assert!(update.member_user.is_none());
    assert!(
        diagnostics
            .malformed
            .iter()
            .any(|entry| entry == "voice.member.user")
    );

    let mismatched_member = voice_frame(
        3,
        vec![
            ("guild_id", text(guild)),
            ("user_id", text(user)),
            ("channel_id", text("7")),
            ("member", member("999", "other")),
        ],
    );
    let (update, diagnostics) = voice_update(&mismatched_member);
    assert!(update.member_user.is_none());
    assert!(
        diagnostics
            .inconsistencies
            .iter()
            .any(|entry| entry == "voice:1014537851418980372:384299970632876032:member-user:999"),
        "{:?}",
        diagnostics.inconsistencies
    );

    let not_map = frame("VOICE_STATE_UPDATE", 4, list(Vec::new()));
    assert!(matches!(
        parse_err(&not_map),
        EventError::WrongType {
            ref path,
            expected: "map",
            ..
        } if path == "voice"
    ));
}
