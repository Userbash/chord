//! Контрактные тесты этапов V1, V2, V3, V4 в `discord-voice`:
//! - V1: `VoiceCallStateMachine` (`Idle`, `Signaling`, `NegotiatingTransport`, `Connected`, `Departing`),
//!   монотонный `negotiation_revision`, барьер `VOICE_STATE_UPDATE` + `VOICE_SERVER_UPDATE`,
//!   `session_confirmed` только после `op:4 SessionDescription`, коалесценция `STATE_COALESCE = 120ms`,
//!   таймаут 15s с возвратом `VoiceCleanupAction::SendGatewayLeaveOp4`.
//! - V2: `SeqAckTracker` (`seq_ack: Option<u16>` из JSON `seq` и 2B BE заголовка бинарных DAVE-фреймов
//!   `op: 25/27/29/30`, генерация `op:3 Heartbeat` и `op:7 Resume`).
//! - V3: `DaveMlsEngine` в `srtp.rs` (инициализация на `op:4` с отправкой бинарного `op:26 KeyPackage`,
//!   учёт `recognized_users` из `op:11/13`, обработка бинарных `op:25 ExternalSender`,
//!   `op:27 Proposals` -> ответ бинарный `op:28 CommitWelcome`, `op:29/30 Commit/Welcome` ->
//!   ответ JSON `op:23 TransitionReady` или `op:31 InvalidCommitWelcome` + свежий `op:26`,
//!   обработка JSON `op:21/22` переключения эпох, двойное шифрование/дешифрование DAVE + SRTP
//!   со снятием RFC 5285 `0xBEDE`).
//! - V4: `UdpReliabilityTracker` (keepalive 5s, `UdpFailures = 10` ошибок подряд -> реконнект,
//!   ротация порта `+60.08s`).

use discord_voice::srtp::{
    DAVE_MARKER_BYTES, DaveMlsEngine, DaveTransitionOutcome, EncryptionMode, SrtpError,
};
use discord_voice::{
    KEEPALIVE_MAGIC, STATE_COALESCE, STATE_COALESCE_MS, SeqAckTracker, UDP_MAX_CONSECUTIVE_FAILURES,
    UdpFailures, UdpReliabilityStatus, UdpReliabilityTracker, VOICE_CALL_NEGOTIATION_TIMEOUT_MS,
    VoiceCallState, VoiceCallStateMachine, VoiceCleanupAction, VoiceServerUpdate, VoiceStateUpdate,
    parse_frame,
};
use std::time::Duration;

const SAMPLE_OP4_SECRET_KEY: [u8; 32] = [
    0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x01,
    0x12, 0x23, 0x34, 0x45, 0x56, 0x67, 0x78, 0x89, 0x9a, 0xab, 0xbc, 0xcd, 0xde, 0xef, 0xf0, 0x02,
];

fn sample_session_description() -> discord_voice::SessionDescription {
    discord_voice::SessionDescription {
        mode: "aead_xchacha20_poly1305_rtpsize".to_owned(),
        secret_key: SAMPLE_OP4_SECRET_KEY.to_vec(),
        media_session_id: "media-session-v1".to_owned(),
        dave_protocol_version: 1,
        secure_frames_version: 1,
        audio_codec: Some("opus".to_owned()),
        video_codec: Some("H264".to_owned()),
    }
}

#[test]
fn v1_voice_call_state_machine_barrier_monotonic_revision_and_confirm_only_after_op4() {
    assert_eq!(STATE_COALESCE_MS, 120);
    assert_eq!(STATE_COALESCE, Duration::from_millis(120));
    assert_eq!(VOICE_CALL_NEGOTIATION_TIMEOUT_MS, 15_000);

    let mut fsm = VoiceCallStateMachine::new("384299970632876032");
    assert_eq!(fsm.state(), VoiceCallState::Idle);
    assert_eq!(fsm.negotiation_revision(), 0);
    assert!(!fsm.session_confirmed());

    // 1. Вход в голосовой канал переводит автомат из Idle в Signaling и отправляет Gateway op:4
    let join_op4 = fsm
        .start_join(
            Some("1014537851418980372".to_owned()),
            "1176785373989445703".to_owned(),
            1,
            false,
            false,
            1_000,
        )
        .expect("start_join from Idle");
    assert_eq!(fsm.state(), VoiceCallState::Signaling);
    assert!(!fsm.session_confirmed());
    let join_val: serde_json::Value = serde_json::from_str(&join_op4).expect("valid join op4 json");
    assert_eq!(join_val["op"], 4);
    assert_eq!(join_val["d"]["guild_id"], "1014537851418980372");
    assert_eq!(join_val["d"]["channel_id"], "1176785373989445703");
    assert_eq!(join_val["d"]["self_mute"], false);
    assert_eq!(join_val["d"]["self_deaf"], false);

    // Попытка подтвердить сессию до прохождения барьера VOICE_STATE_UPDATE + VOICE_SERVER_UPDATE отклоняется
    let desc = sample_session_description();
    assert!(fsm.on_session_description_op4(1, &desc).is_err());
    assert!(!fsm.session_confirmed());
    assert_eq!(fsm.state(), VoiceCallState::Signaling);

    // 2. Приходит только VOICE_STATE_UPDATE: ревизия растёт (0 -> 1), но без VOICE_SERVER_UPDATE барьер не пройден
    let vsu = VoiceStateUpdate::parse(
        r#"{"guild_id":"1014537851418980372","channel_id":"1176785373989445703","user_id":"384299970632876032","session_id":"sess-alpha-01","self_mute":false,"self_deaf":false,"self_video":false}"#,
    )
    .expect("parse VSU");
    let barrier_after_vsu = fsm.on_voice_state_update(&vsu).expect("handle VSU");
    assert_eq!(barrier_after_vsu, None);
    assert_eq!(fsm.state(), VoiceCallState::Signaling);
    assert_eq!(fsm.negotiation_revision(), 1);
    assert!(!fsm.session_confirmed());

    // Повторный идентичный VOICE_STATE_UPDATE не раздувает negotiation_revision
    assert_eq!(fsm.on_voice_state_update(&vsu).expect("idempotent VSU"), None);
    assert_eq!(fsm.negotiation_revision(), 1);

    // 3. Приходит VOICE_SERVER_UPDATE: барьер закрыт, ревизия монотонно растёт (1 -> 2), переход в NegotiatingTransport
    let server = VoiceServerUpdate::parse(
        r#"{"token":"tok-secret-01","guild_id":"1014537851418980372","endpoint":"c-fra19-4da7fe5f.discord.media:2053"}"#,
    )
    .expect("parse VSerU");
    let barrier_ready_rev = fsm.on_voice_server_update(&server).expect("handle VSerU");
    assert_eq!(barrier_ready_rev, Some(2));
    assert_eq!(fsm.state(), VoiceCallState::NegotiatingTransport);
    assert_eq!(fsm.negotiation_revision(), 2);
    // Даже после прохождения барьера шлюза session_confirmed == false до получения RTC op:4 SessionDescription!
    assert!(!fsm.session_confirmed());

    // 4. Неверная (устаревшая) ревизия в op:4 отклоняется
    assert!(fsm.on_session_description_op4(1, &desc).is_err());
    assert!(!fsm.session_confirmed());
    assert_eq!(fsm.state(), VoiceCallState::NegotiatingTransport);

    // 5. Получение валидного RTC op:4 SessionDescription с актуальной ревизией 2 подтверждает сессию -> Connected
    fsm.on_session_description_op4(2, &desc)
        .expect("confirm session on RTC op:4");
    assert!(fsm.session_confirmed());
    assert_eq!(fsm.state(), VoiceCallState::Connected);

    // 6. Выход из звонка: Connected -> Departing -> Idle (после пустого VOICE_STATE_UPDATE от Discord)
    let leave_op4 = fsm.leave(5_000).expect("leave active call");
    assert_eq!(fsm.state(), VoiceCallState::Departing);
    assert!(!fsm.session_confirmed());
    let leave_val: serde_json::Value = serde_json::from_str(&leave_op4).expect("leave op4 json");
    assert_eq!(leave_val["op"], 4);
    assert!(leave_val["d"]["channel_id"].is_null());
    assert_eq!(leave_val["d"]["self_mute"], true);
    assert_eq!(leave_val["d"]["self_deaf"], true);

    let departed_vsu = VoiceStateUpdate {
        guild_id: Some("1014537851418980372".to_owned()),
        channel_id: None,
        user_id: "384299970632876032".to_owned(),
        session_id: "sess-alpha-01".to_owned(),
        self_mute: true,
        self_deaf: true,
        self_video: false,
        self_stream: false,
    };
    fsm.on_voice_state_update(&departed_vsu)
        .expect("departed ack");
    assert_eq!(fsm.state(), VoiceCallState::Idle);
    assert_eq!(fsm.negotiation_revision(), 2);
}

#[test]
fn v1_barrier_accepts_server_update_before_state_update_and_rotates_revision() {
    let mut fsm = VoiceCallStateMachine::new("384299970632876032");
    fsm.start_join(
        Some("1014537851418980372".to_owned()),
        "1176785373989445703".to_owned(),
        2,
        false,
        false,
        2_000,
    )
    .expect("start_join");

    // Сначала VOICE_SERVER_UPDATE -> ещё в Signaling
    let server_1 = VoiceServerUpdate::parse(
        r#"{"token":"tok-1","guild_id":"1014537851418980372","endpoint":"c-arn02.discord.media:2096"}"#,
    )
    .expect("server_1");
    assert_eq!(fsm.on_voice_server_update(&server_1).expect("server_1"), None);
    assert_eq!(fsm.state(), VoiceCallState::Signaling);
    assert_eq!(fsm.negotiation_revision(), 1);

    // Затем VOICE_STATE_UPDATE -> барьер пройден, NegotiatingTransport, ревизия 2
    let vsu = VoiceStateUpdate::parse(
        r#"{"guild_id":"1014537851418980372","channel_id":"1176785373989445703","user_id":"384299970632876032","session_id":"sess-beta-02"}"#,
    )
    .expect("vsu");
    assert_eq!(fsm.on_voice_state_update(&vsu).expect("vsu"), Some(2));
    assert_eq!(fsm.state(), VoiceCallState::NegotiatingTransport);

    // Ротация голосового сервера во время переговоров инкрементирует ревизию до 3
    let server_2 = VoiceServerUpdate::parse(
        r#"{"token":"tok-2","guild_id":"1014537851418980372","endpoint":"c-rot01.discord.media:2053"}"#,
    )
    .expect("server_2");
    assert_eq!(
        fsm.on_voice_server_update(&server_2).expect("server_2"),
        Some(3)
    );
    assert_eq!(fsm.negotiation_revision(), 3);
}

#[test]
fn v1_state_coalesce_120ms_collapses_rapid_mute_deaf_toggles_and_handles_send_failure() {
    let mut fsm = VoiceCallStateMachine::new("384299970632876032");
    let _ = fsm
        .start_join(
            Some("1014537851418980372".to_owned()),
            "1176785373989445703".to_owned(),
            1,
            false,
            false,
            1_000,
        )
        .expect("join sent at 1000ms");

    // Быстрая серия переключений внутри окна 120ms (1000..1120 мс) коалесцируется
    assert_eq!(fsm.set_mute_deaf(true, false, 1_020), None);
    assert_eq!(fsm.state_deadline_ms(), Some(1_120));
    assert_eq!(fsm.set_mute_deaf(false, true, 1_050), None);
    // Дедлайн остаётся привязанным к началу окна (1000 + 120 = 1120), а deaf=true включает muted=true
    assert_eq!(fsm.state_deadline_ms(), Some(1_120));
    assert!(fsm.muted());
    assert!(fsm.deafened());

    // До наступления 1120 мс flush_state ничего не отправляет
    assert_eq!(fsm.flush_state(1_119), None);

    // Ровно в 1120 мс уходит один агрегированный op:4 с последним состоянием (mute:true, deaf:true)
    let coalesced = fsm
        .flush_state(1_120)
        .expect("coalesced op4 must flush at 1120ms");
    let val: serde_json::Value = serde_json::from_str(&coalesced).expect("json");
    assert_eq!(val["op"], 4);
    assert_eq!(val["d"]["self_mute"], true);
    assert_eq!(val["d"]["self_deaf"], true);
    assert_eq!(fsm.state_deadline_ms(), None);

    // Если внутри нового окна 120ms (1120..1240 мс) переключить и вернуть обратно к (true, true),
    // дедлайн снимается и лишний пакет не уходит
    assert_eq!(fsm.set_mute_deaf(false, false, 1_150), None);
    assert_eq!(fsm.state_deadline_ms(), Some(1_240));
    assert_eq!(fsm.set_mute_deaf(true, true, 1_180), None);
    assert_eq!(fsm.state_deadline_ms(), None);
    assert_eq!(fsm.flush_state(1_250), None);

    // Спустя >= 120ms после последней отправки (1120 + 120 = 1240) изменение уходит немедленно
    let immediate = fsm
        .set_mute_deaf(false, false, 1_300)
        .expect("outside 120ms window sends immediately");
    let imm_val: serde_json::Value = serde_json::from_str(&immediate).expect("json");
    assert_eq!(imm_val["d"]["self_mute"], false);
    assert_eq!(imm_val["d"]["self_deaf"], false);

    // При сбое отправки сокета state_send_failed выставляет немедленный повтор для следующего flush
    fsm.state_send_failed(1_305);
    assert_eq!(fsm.state_deadline_ms(), Some(1_305));
    let retry = fsm.flush_state(1_305).expect("retry after state_send_failed");
    let retry_val: serde_json::Value = serde_json::from_str(&retry).expect("json");
    assert_eq!(retry_val["d"]["self_mute"], false);
}

#[test]
fn v1_negotiation_15s_timeout_returns_send_gateway_leave_op4_and_clears_phantom_session() {
    let mut fsm = VoiceCallStateMachine::new("384299970632876032");
    fsm.start_join(
        Some("1014537851418980372".to_owned()),
        "1176785373989445703".to_owned(),
        7,
        false,
        false,
        10_000,
    )
    .expect("start_join at 10_000ms");

    // На 24_999 мс (14.999s) таймаут ещё не наступил
    assert_eq!(fsm.check_timeout(24_999), VoiceCleanupAction::None);
    assert_eq!(fsm.state(), VoiceCallState::Signaling);

    // Ровно через 15s (25_000 мс) без op:4 SessionDescription возвращается SendGatewayLeaveOp4
    let action = fsm.check_timeout(25_000);
    assert_eq!(action, VoiceCleanupAction::SendGatewayLeaveOp4);
    assert_eq!(fsm.state(), VoiceCallState::Idle);
    assert!(!fsm.session_confirmed());

    let cleanup_json = fsm
        .take_last_cleanup_op4_json()
        .expect("cleanup op4 JSON must be prepared");
    let cleanup_val: serde_json::Value = serde_json::from_str(&cleanup_json).expect("json");
    assert_eq!(cleanup_val["op"], 4);
    assert_eq!(cleanup_val["d"]["guild_id"], "1014537851418980372");
    assert!(cleanup_val["d"]["channel_id"].is_null());
}

#[test]
fn v2_seq_ack_tracker_updates_from_json_and_binary_dave_headers_and_builds_op3_and_op7() {
    let mut tracker = SeqAckTracker::new();
    assert_eq!(tracker.seq_ack(), None);

    // До получения кадров с seq в op:3 и op:7 уходит seq_ack = -1
    let hb_initial = tracker.build_heartbeat_json(1_000);
    let hb_init_val: serde_json::Value = serde_json::from_str(&hb_initial).expect("hb json");
    assert_eq!(hb_init_val["op"], 3);
    assert_eq!(hb_init_val["d"]["t"], 1_000);
    assert_eq!(hb_init_val["d"]["seq_ack"], -1);

    // 1. JSON фрейм без seq (op:8 Hello) не меняет seq_ack
    assert_eq!(
        tracker
            .update_from_json(r#"{"op":8,"d":{"v":8,"heartbeat_interval":13750}}"#)
            .expect("op8 without seq"),
        None
    );
    assert_eq!(tracker.seq_ack(), None);

    // 2. JSON фрейм с seq (op:15 MediaSinkWants, seq=12) обновляет seq_ack до Some(12)
    assert_eq!(
        tracker
            .update_from_json(r#"{"seq":12,"op":15,"d":{"any":100}}"#)
            .expect("json with seq=12"),
        Some(12)
    );
    assert_eq!(tracker.seq_ack(), Some(12));

    // 3. Бинарные DAVE фреймы op 25, 27, 29, 30 обновляют seq_ack из 2-байтного BE заголовка
    for (seq, op, tail) in [
        (250u16, 25u8, &[0xaa, 0xbb][..]),
        (251u16, 27u8, &[0x00, 0x11, 0x22][..]),
        (252u16, 29u8, &[0x00, 0x01, 0xde, 0xad][..]),
        (65_535u16, 30u8, &[0x00, 0x02, 0xbe, 0xef][..]),
    ] {
        let mut frame = Vec::new();
        frame.extend_from_slice(&seq.to_be_bytes());
        frame.push(op);
        frame.extend_from_slice(tail);

        let (parsed_seq, parsed_op, payload) = tracker
            .update_from_binary(&frame)
            .expect("binary DAVE frame updates seq_ack");
        assert_eq!(parsed_seq, seq);
        assert_eq!(parsed_op, op);
        assert_eq!(payload, tail);
        assert_eq!(tracker.seq_ack(), Some(seq));
    }

    // Обрезанный бинарный фрейм (< 3 байт) отклоняется без порчи seq_ack
    assert!(tracker.update_from_binary(&[0x01, 0x02]).is_err());
    assert_eq!(tracker.seq_ack(), Some(65_535));

    // 4. Генерация op:3 Heartbeat и op:7 Resume использует актуальный seq_ack = 65535
    let hb_json = tracker.build_heartbeat_json(14_750);
    let hb_val: serde_json::Value = serde_json::from_str(&hb_json).expect("hb json");
    assert_eq!(hb_val["op"], 3);
    assert_eq!(hb_val["d"]["t"], 14_750);
    assert_eq!(hb_val["d"]["seq_ack"], 65_535);

    let resume_json = tracker.build_resume_json(
        "1014537851418980372",
        "1176785373989445703",
        "sess-alpha-01",
        "tok-secret-01",
    );
    let resume_val: serde_json::Value = serde_json::from_str(&resume_json).expect("resume json");
    assert_eq!(resume_val["op"], 7);
    assert_eq!(resume_val["d"]["server_id"], "1014537851418980372");
    assert_eq!(resume_val["d"]["channel_id"], "1176785373989445703");
    assert_eq!(resume_val["d"]["session_id"], "sess-alpha-01");
    assert_eq!(resume_val["d"]["token"], "tok-secret-01");
    assert_eq!(resume_val["d"]["seq_ack"], 65_535);
    assert!(parse_frame(&resume_json).is_ok());
}

#[test]
fn v3_dave_mls_engine_op4_init_op11_op13_roster_op25_to_31_and_two_layer_srtp_bede_strip() {
    let own_user_id = 384_299_970_632_876_032u64;
    let peer_user_id = 555_666_777_888_999_000u64;
    let channel_id = 1_176_785_373_989_445_703u64;

    // 1. Инициализация на op:4 SessionDescription немедленно возвращает бинарный op:26 KeyPackage
    let (mut alice, op26_alice) = DaveMlsEngine::init_on_op4(
        own_user_id,
        channel_id,
        1,
        &SAMPLE_OP4_SECRET_KEY,
        EncryptionMode::XChaCha20Poly1305RtpSize,
    )
    .expect("init alice on op:4");
    let (mut bob, op26_bob) = DaveMlsEngine::init_on_op4(
        peer_user_id,
        channel_id,
        1,
        &SAMPLE_OP4_SECRET_KEY,
        EncryptionMode::XChaCha20Poly1305RtpSize,
    )
    .expect("init bob on op:4");

    assert_eq!(op26_alice.first().copied(), Some(26));
    assert!(op26_alice.len() > 16);
    assert_eq!(op26_bob.first().copied(), Some(26));
    assert_eq!(alice.recognized_users(), &[own_user_id]);

    // Даунгрейд dave_protocol_version = 0 на op:4 отклоняется
    assert!(
        DaveMlsEngine::init_on_op4(
            own_user_id,
            channel_id,
            0,
            &SAMPLE_OP4_SECRET_KEY,
            EncryptionMode::XChaCha20Poly1305RtpSize,
        )
        .is_err()
    );

    // 2. Учёт recognized_users из op:11 ClientsConnect и op:13 ClientDisconnect
    assert!(
        alice
            .on_clients_connect_op11(&[peer_user_id, 777_888_999])
            .expect("op:11 connect peers")
    );
    assert!(alice.contains_user(peer_user_id));
    assert!(alice.contains_user(777_888_999));
    assert!(
        alice
            .on_client_disconnect_op13(777_888_999)
            .expect("op:13 disconnect transient peer")
    );
    assert!(!alice.contains_user(777_888_999));
    assert_eq!(alice.recognized_users(), &[own_user_id, peer_user_id]);
    bob.on_clients_connect_op11(&[own_user_id])
        .expect("bob op:11");

    // До получения op:25 ExternalSender пакет op:27 Proposals игнорируется (возвращает Ok(None))
    let early_proposals = [0x00, 0x01, 27, 0x00, 0x01, 0x02, 0x03];
    assert_eq!(
        alice
            .on_proposals_op27(&early_proposals)
            .expect("early proposal before op25"),
        None
    );

    // 3. Обработка бинарного op:25 ExternalSenderPackage
    let ext_sender_frame = [0x00, 0x10, 25, 0xde, 0xad, 0xbe, 0xef, 0x01, 0x02, 0x03, 0x04];
    alice
        .on_external_sender_op25(&ext_sender_frame)
        .expect("alice op25");
    bob.on_external_sender_op25(&ext_sender_frame)
        .expect("bob op25");

    // 4. Обработка бинарного op:27 Proposals -> ответ бинарный op:28 CommitWelcome
    let mut proposal_frame = vec![0x00, 0x11, 27, 0x00]; // seq=17, op=27, operation=0 (APPEND)
    proposal_frame.extend_from_slice(&peer_user_id.to_be_bytes());
    proposal_frame.extend_from_slice(&op26_bob[1..]);
    let op28_commit_welcome = alice
        .on_proposals_op27(&proposal_frame)
        .expect("alice processes op27 proposals")
        .expect("must emit op28 CommitWelcome");
    assert_eq!(op28_commit_welcome[0], 28);
    let mls_commit_body = &op28_commit_welcome[1..];
    assert!(!mls_commit_body.is_empty());

    // 5. Обработка бинарного op:29 PrepareCommitTransition и op:30 Welcome -> ответ JSON op:23 TransitionReady
    let transition_id = 4u16;
    let mut op29_frame = vec![0x00, 0x12, 29];
    op29_frame.extend_from_slice(&transition_id.to_be_bytes());
    op29_frame.extend_from_slice(mls_commit_body);

    let alice_outcome = alice
        .on_commit_or_welcome_op29_op30(&op29_frame)
        .expect("alice op29 commit");
    assert_eq!(
        alice_outcome,
        DaveTransitionOutcome::SendTransitionReadyOp23 {
            transition_id: 4,
            op23_json: r#"{"d":{"transition_id":4},"op":23}"#.to_owned(),
        }
    );

    let mut op30_frame = vec![0x00, 0x13, 30];
    op30_frame.extend_from_slice(&transition_id.to_be_bytes());
    op30_frame.extend_from_slice(mls_commit_body);
    let bob_outcome = bob
        .on_commit_or_welcome_op29_op30(&op30_frame)
        .expect("bob op30 welcome");
    assert_eq!(
        bob_outcome,
        DaveTransitionOutcome::SendTransitionReadyOp23 {
            transition_id: 4,
            op23_json: r#"{"d":{"transition_id":4},"op":23}"#.to_owned(),
        }
    );

    // 6. Обработка JSON op:21 PrepareTransition и op:22 ExecuteTransition (переключение эпохи)
    let op21_resp = alice
        .on_prepare_transition_op21(4, 1)
        .expect("op21 prepare transition");
    assert_eq!(
        op21_resp.as_deref(),
        Some(r#"{"d":{"transition_id":4},"op":23}"#)
    );
    let active_epoch_alice = alice
        .on_execute_transition_op22(4)
        .expect("alice op22 execute transition");
    let active_epoch_bob = bob
        .on_execute_transition_op22(4)
        .expect("bob op22 execute transition");
    assert_eq!(active_epoch_alice, 1);
    assert_eq!(active_epoch_bob, 1);
    assert!(alice.is_ready());
    assert!(bob.is_ready());

    // 7. Невалидный/повреждённый Commit/Welcome -> ответ JSON op:31 InvalidCommitWelcome + свежий бинарный op:26 KeyPackage
    let corrupt_op29 = [0x00, 0x14, 29, 0x00, 0x09, 0xff, 0x00, 0xff];
    let recovery = alice
        .on_commit_or_welcome_op29_op30(&corrupt_op29)
        .expect("corrupt op29 triggers recovery outcome");
    match recovery {
        DaveTransitionOutcome::InvalidCommitWelcome {
            transition_id: tid,
            op31_json,
            fresh_key_package_op26,
        } => {
            assert_eq!(tid, 9);
            assert_eq!(op31_json, r#"{"d":{"transition_id":9},"op":31}"#);
            assert_eq!(fresh_key_package_op26.first().copied(), Some(26));
            assert_ne!(
                fresh_key_package_op26, op26_alice,
                "свежий KeyPackage после сброса должен отличаться от исходного"
            );
        }
        other => panic!("expected InvalidCommitWelcome, got {other:?}"),
    }

    // Восстанавливаем эпоху 1 у Alice через transition_id = 0 (немедленное применение)
    alice
        .on_external_sender_op25(&ext_sender_frame)
        .expect("re-apply op25");
    let op28_re = alice
        .on_proposals_op27(&proposal_frame)
        .expect("re-apply op27")
        .expect("op28");
    let mut op29_imm = vec![0x00, 0x15, 29, 0x00, 0x00];
    op29_imm.extend_from_slice(&op28_re[1..]);
    assert_eq!(
        alice
            .on_commit_or_welcome_op29_op30(&op29_imm)
            .expect("op29 transition_id=0"),
        DaveTransitionOutcome::ExecutedImmediate { epoch: 1 }
    );

    // 8. Двойное шифрование/дешифрование DAVE + SRTP со снятием RFC 5285 0xBEDE
    // Формируем RTP-кадр с X=1 и расширением 0xBEDE (2 слова = 8 байт) + открытый Opus-фрейм
    let rtp_header_with_bede: [u8; 24] = [
        0x90, 0x78, 0x00, 0x2a, // V=2, P=0, X=1, CC=0, PT=120 (Opus), seq=42
        0x00, 0x00, 0x03, 0xc0, // timestamp=960
        0x00, 0x00, 0x23, 0xf8, // ssrc=9208
        0xbe, 0xde, 0x00, 0x02, // RFC 5285 0xBEDE, length=2 words (8 bytes)
        0x10, 0xff, 0x22, 0x00, 0x01, 0xab, 0x00, 0x00,
    ];
    let opus_plaintext = b"\xf8\xff\xfe\x41\x42\x43\x44\x45\x46";
    let mut clear_rtp = Vec::with_capacity(rtp_header_with_bede.len() + opus_plaintext.len());
    clear_rtp.extend_from_slice(&rtp_header_with_bede);
    clear_rtp.extend_from_slice(opus_plaintext);

    let wire_packet = alice
        .encrypt_rtp_packet(&clear_rtp)
        .expect("Alice two-layer DAVE + SRTP encrypt");
    assert_eq!(
        &wire_packet[..24],
        &rtp_header_with_bede,
        "RTP header + 0xBEDE extension stays clear AAD in _rtpsize mode"
    );

    // Дешифрование со снятием RFC 5285 0xBEDE возвращает чистый Opus-пэйлоад и метаданные RTP
    let opened = bob
        .decrypt_rtp_stripping_bede(&wire_packet)
        .expect("Bob two-layer SRTP + DAVE decrypt stripping 0xBEDE");
    assert_eq!(opened.ssrc, 0x23f8);
    assert_eq!(opened.sequence, 0x2a);
    assert_eq!(opened.timestamp, 960);
    assert_eq!(opened.payload_type, 120);
    assert_eq!(opened.extension_profile, Some(0xBEDE));
    assert_eq!(
        opened.extension_data.as_deref(),
        Some(&rtp_header_with_bede[16..24])
    );
    assert_eq!(opened.payload, opus_plaintext);

    // Также проверяем Serein soundshare формат (преамбула 0xBEDE в AAD, тело расширения зашифровано вместе с DAVE-кадром)
    let soundshare_wire = alice
        .seal_soundshare_rtp(
            &rtp_header_with_bede[..12].try_into().unwrap(),
            opus_plaintext,
        )
        .expect("seal_soundshare_rtp");
    let opened_soundshare = bob
        .decrypt_rtp_stripping_bede(&soundshare_wire)
        .expect("decrypt soundshare stripping 0xBEDE");
    assert_eq!(opened_soundshare.payload, opus_plaintext);
    assert_eq!(opened_soundshare.extension_profile, Some(0xBEDE));

    // Повторная отправка того же пакета блокируется защитой от повтора нонса (NonceReuse)
    assert!(matches!(
        bob.decrypt_rtp_stripping_bede(&wire_packet),
        Err(SrtpError::NonceReuse { .. })
    ));

    // Подделка любого байта шифротекста нового пакета отклоняется аутентификацией
    let mut tampered = alice
        .encrypt_rtp_packet(&clear_rtp)
        .expect("encrypt second packet for tamper check");
    tampered[25] ^= 0x80;
    assert_eq!(
        bob.decrypt_rtp_stripping_bede(&tampered),
        Err(SrtpError::AuthenticationFailed)
    );
    assert_eq!(DAVE_MARKER_BYTES, [0xFA, 0xFA]);
}

#[test]
fn v4_udp_reliability_tracker_keepalive_5s_ten_failures_reconnect_and_60_08s_port_rotation() {
    assert_eq!(UDP_MAX_CONSECUTIVE_FAILURES, 10);

    let mut tracker = UdpReliabilityTracker::new(1_000);
    assert_eq!(tracker.consecutive_failures(), 0);
    assert!(!tracker.should_reconnect());

    // 1. Keepalive каждые 5s (5_000 мс)
    assert_eq!(tracker.poll_keepalive(5_999), None);
    let ka_1 = tracker
        .poll_keepalive(6_000)
        .expect("keepalive due at +5000ms");
    assert_eq!(&ka_1[..4], &KEEPALIVE_MAGIC.to_be_bytes());
    assert_eq!(u32::from_le_bytes([ka_1[4], ka_1[5], ka_1[6], ka_1[7]]), 1);
    assert_eq!(tracker.poll_keepalive(10_999), None);
    let ka_2 = tracker
        .poll_keepalive(11_000)
        .expect("second keepalive due at +10000ms");
    assert_eq!(u32::from_le_bytes([ka_2[4], ka_2[5], ka_2[6], ka_2[7]]), 2);

    // 2. UdpFailures = 10 ошибок подряд -> реконнект; успешный пакет сбрасывает счётчик
    let mut failures = UdpFailures::new();
    for attempt in 1..=9 {
        assert!(!failures.record_failure());
        assert_eq!(failures.consecutive_failures(), attempt);
    }
    failures.record_success();
    assert_eq!(failures.consecutive_failures(), 0);

    for expected in 1..=9 {
        assert_eq!(
            tracker.record_send_result(false),
            UdpReliabilityStatus::TransientFailure {
                consecutive_failures: expected
            }
        );
        assert!(!tracker.should_reconnect());
    }
    // 10-я ошибка подряд переводит трекер в ReconnectRequired
    assert_eq!(
        tracker.record_send_result(false),
        UdpReliabilityStatus::ReconnectRequired {
            consecutive_failures: 10
        }
    );
    assert!(tracker.should_reconnect());

    // После успешной отправки/реконнекта счётчик сбрасывается
    assert_eq!(
        tracker.record_send_result(true),
        UdpReliabilityStatus::Healthy
    );
    assert_eq!(tracker.consecutive_failures(), 0);
    assert!(!tracker.should_reconnect());

    // 3. Ротация UDP-порта через +60.08s (60_080 мс)
    assert_eq!(tracker.poll_port_rotation(61_079, 9208), None);
    let rotation = tracker
        .poll_port_rotation(61_080, 9208)
        .expect("port rotation must trigger at +60.08s (60_080ms)");
    assert_eq!(&rotation.discovery[0..4], &[0x00, 0x01, 0x00, 0x46]);
    assert_eq!(&rotation.discovery[4..8], &9208u32.to_be_bytes());
    assert_eq!(&rotation.empty_rr[0..4], &[0x80, 0xc9, 0x00, 0x01]);
    assert_eq!(&rotation.empty_rr[4..8], &9208u32.to_be_bytes());
    assert_eq!(tracker.rotations_completed(), 1);
    assert_eq!(tracker.poll_port_rotation(121_159, 9208), None);
    assert!(tracker.poll_port_rotation(121_160, 9208).is_some());
    assert_eq!(tracker.rotations_completed(), 2);
}
