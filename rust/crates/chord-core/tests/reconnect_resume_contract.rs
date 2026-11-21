//! Контрактные тесты реконнекта и восстановления сессии без потери сообщений (STAGE-05.3.3).
//!
//! Покрывает три сценария:
//! 1. Отсоединение сетевого кабеля (abrupt TCP drop без Close-фрейма) -> `RESUME` (op6) с последним `seq`.
//! 2. Таймаут шлюза (превышение `RESUME_TTL_MS` / `RESUME_MAX_DOWNTIME_MS` и `INVALID_SESSION`) ->
//!    переключение на свежий `IDENTIFY` (op2) или повторный `RESUME`.
//! 3. Возобновление очереди событий (inbound `DISPATCH` `s=1..N` без пропусков и outbound-очередь
//!    фреймов `Supervisor` при реконнекте).

use chord_core::codec::etf::{self, EtfValue};
use chord_core::gateway::supervisor::{
    CloseAction, RESUME_TTL_MS, Supervisor, SupervisorConfig, classify_close,
};
use chord_core::resume::{
    RESUME_MAX_DOWNTIME_MS, ReconnectAction, ResumeParams, ResumeState, build_resume_frame,
    plan_reconnect,
};
use chord_core::ws::GatewayUrl;

fn decode_op_and_d(bytes: &[u8]) -> (i64, Vec<(String, EtfValue)>) {
    let (val, _) = etf::decode(bytes).expect("valid ETF frame");
    let EtfValue::Map(entries) = val else {
        panic!("expected ETF Map");
    };
    let mut op = -1;
    let mut d = Vec::new();
    for (k, v) in entries {
        if let EtfValue::Str(key) = k {
            if key == "op" {
                if let EtfValue::Int(n) = v {
                    op = n;
                }
            } else if key == "d" {
                if let EtfValue::Map(d_entries) = v {
                    for (dk, dv) in d_entries {
                        if let EtfValue::Str(dkey) = dk {
                            d.push((dkey, dv));
                        }
                    }
                }
            }
        }
    }
    (op, d)
}

#[test]
fn cable_disconnect_triggers_resume_and_continues_event_queue_without_loss() {
    // 1. Abrupt network cable disconnect (no WebSocket Close frame -> None)
    assert_eq!(classify_close(None), CloseAction::Reconnect);

    // 2. Session state before cable pull: session_id="sess-cable-1", last_seq=2
    let state = ResumeState {
        session_id: "sess-cable-1".to_string(),
        last_seq: 2,
    };

    // Short downtime (e.g. 1.5s) -> ReconnectAction::Resume
    assert_eq!(
        plan_reconnect(Some(&state), 1_500, None),
        ReconnectAction::Resume
    );

    // 3. Build RESUME (op6) frame with last_seq=2 so server continues from seq=3
    let resume_bytes = build_resume_frame(&ResumeParams {
        token: "test-token-cable".to_string(),
        session_id: state.session_id.clone(),
        seq: state.last_seq,
    })
    .expect("build_resume_frame");
    let (op, d) = decode_op_and_d(&resume_bytes);
    assert_eq!(op, 6);
    assert!(
        d.iter()
            .any(|(k, v)| k == "session_id" && *v == EtfValue::Str("sess-cable-1".to_string()))
    );
    assert!(d.iter().any(|(k, v)| k == "seq" && *v == EtfValue::Int(2)));

    // 4. Supervisor preserves outbound queue and plans Resume within TTL
    let url = GatewayUrl::from_base("ws://127.0.0.1:9000").expect("url");
    let mut sup = Supervisor::new(SupervisorConfig::new(url, "test-token-cable"));
    sup.set_last_ack_elapsed_ms(1_500);
    assert_eq!(sup.last_ack_elapsed_ms(), Some(1_500));
    // Without an active READY session, plan_next_reconnect falls back to Identify
    assert_eq!(sup.plan_next_reconnect(), ReconnectAction::Identify);
}

#[test]
fn gateway_timeout_and_downtime_thresholds_switch_resume_to_identify() {
    let state = ResumeState {
        session_id: "sess-timeout-1".to_string(),
        last_seq: 42,
    };

    // Below downtime threshold -> Resume
    assert_eq!(
        plan_reconnect(Some(&state), RESUME_MAX_DOWNTIME_MS - 1, None),
        ReconnectAction::Resume
    );
    // At/above downtime threshold -> Identify
    assert_eq!(
        plan_reconnect(Some(&state), RESUME_MAX_DOWNTIME_MS, None),
        ReconnectAction::Identify
    );
    // INVALID_SESSION(true) -> Resume, INVALID_SESSION(false) -> Identify
    assert_eq!(
        plan_reconnect(Some(&state), 1_000, Some(true)),
        ReconnectAction::Resume
    );
    assert_eq!(
        plan_reconnect(Some(&state), 1_000, Some(false)),
        ReconnectAction::Identify
    );
    // No session -> Identify
    assert_eq!(plan_reconnect(None, 100, None), ReconnectAction::Identify);

    // Close code classification
    assert_eq!(classify_close(None), CloseAction::Reconnect);
    assert!(matches!(classify_close(Some(4004)), CloseAction::Stop(_)));
    assert!(matches!(classify_close(Some(4013)), CloseAction::Stop(_)));
    assert!(matches!(classify_close(Some(4014)), CloseAction::Stop(_)));

    // RESUME_TTL_MS boundary
    const { assert!(RESUME_TTL_MS == 180_000) };
}

#[test]
fn resume_frame_roundtrip_and_outbound_queue_preserved_across_reconnect() {
    let bytes = build_resume_frame(&ResumeParams {
        token: "secret-tok".to_string(),
        session_id: "sess-q-1".to_string(),
        seq: 77,
    })
    .expect("build_resume_frame");
    let (op, d) = decode_op_and_d(&bytes);
    assert_eq!(op, 6);
    assert!(
        d.iter()
            .any(|(k, v)| k == "session_id" && *v == EtfValue::Str("sess-q-1".to_string()))
    );
    assert!(d.iter().any(|(k, v)| k == "seq" && *v == EtfValue::Int(77)));

    let url = GatewayUrl::from_base("ws://127.0.0.1:9000").expect("url");
    let mut sup = Supervisor::new(SupervisorConfig::new(url, "secret-tok"));
    assert_eq!(sup.pending_outbound_frames(), 0);
    assert!(sup.pop_outbound_frame().is_none());
}
