#![forbid(unsafe_code)]
//! Contract tests for `STAGE-10.BFF.6`:
//! Wiring `DaveVoiceBridge<DaveySession>` (`dave-spike`) with Tauri commands
//! `voice_connect`, `voice_state`, and `voice_e2ee_status` for real-time DAVE E2EE
//! status indication in the user's voice interface.

#[allow(dead_code)]
#[path = "../src/voice_plugin.rs"]
pub mod voice_plugin;

use chord_client::dave_bridge::{
    self, DaveE2eeStatusDto, DaveGatewayBinaryOp, VoiceStateSnapshotDto,
};
use dave_spike::davey_crypto::DaveyExternalDelivery;
use dave_spike::davey_session::DaveySession;
use dave_spike::protocol::{
    DAVE_MAGIC_MARKER, DAVE_PROTOCOL_VERSION_V1, OP_DAVE_EXECUTE_TRANSITION,
    OP_DAVE_MLS_COMMIT_WELCOME, OP_DAVE_MLS_INVALID_COMMIT_WELCOME, OP_DAVE_MLS_KEY_PACKAGE,
    OP_DAVE_PREPARE_EPOCH, OP_DAVE_PREPARE_TRANSITION, OP_STREAM_CREATE,
};
use serde_json::json;
use std::future::Future;
use std::sync::{Mutex, OnceLock};

const GUILD_ID: &str = "111222333444555666";
const CHANNEL_ID: &str = "222333444555666777";
const OTHER_CHANNEL_ID: &str = "999888777666555444";
const PEER_UID_STR: &str = "333444555666777888";
const PEER_UID: u64 = 333_444_555_666_777_888;

fn serial_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn run_async<F: Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio current_thread runtime");
    rt.block_on(fut)
}

#[test]
fn main_rs_and_voice_plugin_register_dave_bridge_commands() {
    let main_src = include_str!("../src/main.rs");
    let plugin_src = include_str!("../src/voice_plugin.rs");

    assert_eq!(
        voice_plugin::DAVE_VOICE_BRIDGE_COMMANDS,
        ["voice_connect", "voice_state", "voice_e2ee_status"]
    );

    let handler_start = main_src
        .find("generate_handler![")
        .expect("generate_handler! in main.rs");
    let handler_tail = &main_src[handler_start..];
    let handler_end = handler_tail.find(']').expect("closing bracket");
    let handler_block = &handler_tail[..handler_end];

    for cmd in ["voice_connect", "voice_state", "voice_e2ee_status"] {
        assert!(
            main_src.contains(&format!("async fn {cmd}(")),
            "main.rs must declare #[tauri::command] async fn {cmd}"
        );
        assert!(
            handler_block.contains(cmd),
            "main.rs generate_handler! must register {cmd}"
        );
    }

    let has_cyrillic = plugin_src
        .chars()
        .any(|ch| ('\u{0400}'..='\u{04FF}').contains(&ch));
    assert!(
        !has_cyrillic,
        "voice_plugin.rs must remain strictly English"
    );
}

#[test]
fn voice_connect_initializes_dave_bridge_and_exposes_waiting_e2ee_status() {
    let _guard = serial_lock().lock().unwrap_or_else(|p| p.into_inner());
    let _ = run_async(voice_plugin::voice_disconnect(None));

    // Disconnected baseline
    let disconnected: DaveE2eeStatusDto =
        run_async(voice_plugin::voice_e2ee_status(None)).expect("disconnected e2ee status");
    assert_eq!(disconnected.state, "disconnected");
    assert!(!disconnected.is_end_to_end_encrypted);
    assert!(!disconnected.dave_verified);
    assert!(disconnected.privacy_code.is_none());
    assert!(disconnected.privacy_code_chunks.is_empty());

    // Connect to guild voice channel
    let connect_res = run_async(voice_plugin::voice_connect(
        Some(GUILD_ID.to_string()),
        Some(CHANNEL_ID.to_string()),
        Some(false),
        Some(false),
        Some(false),
        Some(false),
    ))
    .expect("voice_connect should succeed");

    let e2ee_embedded = connect_res
        .get("e2ee")
        .expect("voice_connect response must embed `e2ee` status object");
    assert_eq!(e2ee_embedded["enabled"], json!(true));
    assert_eq!(e2ee_embedded["backend"], json!("davey_rust_crypto"));
    assert_eq!(e2ee_embedded["configuredVersion"], json!(1));
    assert_eq!(e2ee_embedded["state"], json!("waiting_for_peer"));
    assert_eq!(e2ee_embedded["waitingForPeer"], json!(true));
    assert_eq!(e2ee_embedded["isEndToEndEncrypted"], json!(false));
    assert_eq!(e2ee_embedded["channelId"], json!(CHANNEL_ID));

    // Query dedicated voice_e2ee_status command
    let status = run_async(voice_plugin::voice_e2ee_status(Some(
        CHANNEL_ID.to_string(),
    )))
    .expect("voice_e2ee_status for active channel");
    assert_eq!(status.state, "waiting_for_peer");
    assert!(status.waiting_for_peer);
    assert!(!status.is_end_to_end_encrypted);
    assert_eq!(status.configured_version, DAVE_PROTOCOL_VERSION_V1);
    assert_eq!(status.channel_id.as_deref(), Some(CHANNEL_ID));
    assert_eq!(status.guild_id.as_deref(), Some(GUILD_ID));
    assert_eq!(status.participant_count, 1);

    // Querying a different channel_id does not leak active call status
    let other_status = run_async(voice_plugin::voice_e2ee_status(Some(
        OTHER_CHANNEL_ID.to_string(),
    )))
    .expect("voice_e2ee_status for non-active channel");
    assert_eq!(other_status.state, "disconnected");
    assert!(!other_status.is_end_to_end_encrypted);

    // voice_rtc_status also embeds live `e2ee`
    let rtc = run_async(voice_plugin::voice_rtc_status()).expect("voice_rtc_status");
    assert_eq!(rtc["e2ee"]["state"], json!("waiting_for_peer"));

    let _ = run_async(voice_plugin::voice_disconnect(Some(GUILD_ID.to_string())));
}

#[test]
fn voice_state_tracks_mute_deaf_and_peer_roster_transitions_in_dave_bridge() {
    let _guard = serial_lock().lock().unwrap_or_else(|p| p.into_inner());
    let _ = run_async(voice_plugin::voice_disconnect(None));

    let _ = run_async(voice_plugin::voice_connect(
        Some(GUILD_ID.to_string()),
        Some(CHANNEL_ID.to_string()),
        Some(false),
        Some(false),
        Some(false),
        Some(false),
    ))
    .expect("voice_connect");

    // 1. Read-only query via voice_state with all None options
    let initial: VoiceStateSnapshotDto = run_async(voice_plugin::voice_state(
        None, None, None, None, None, None, None,
    ))
    .expect("voice_state query");
    assert_eq!(initial.channel_id.as_deref(), Some(CHANNEL_ID));
    assert_eq!(initial.guild_id.as_deref(), Some(GUILD_ID));
    assert!(!initial.self_mute);
    assert!(!initial.self_deaf);
    assert!(!initial.effective_mute);
    assert_eq!(initial.e2ee.state, "waiting_for_peer");

    // 2. Peer joins via voice_state -> transitions waiting_for_peer -> handshaking
    let with_peer: VoiceStateSnapshotDto = run_async(voice_plugin::voice_state(
        Some(GUILD_ID.to_string()),
        Some(CHANNEL_ID.to_string()),
        Some(false),
        Some(true),
        Some(false),
        Some(false),
        Some(vec![PEER_UID_STR.to_string()]),
    ))
    .expect("voice_state update with peer and deafen");
    assert!(with_peer.self_deaf);
    assert!(
        with_peer.effective_mute,
        "self_deaf=true must enforce effective_mute=true"
    );
    assert_eq!(with_peer.speaking_flags, 0);
    assert_eq!(with_peer.e2ee.participant_count, 2);
    assert!(!with_peer.e2ee.waiting_for_peer);
    assert_eq!(with_peer.e2ee.state, "handshaking");

    // 3. Peer leaves (empty peer_user_ids) -> returns to sole-member waiting_for_peer
    let peer_left: VoiceStateSnapshotDto = run_async(voice_plugin::voice_state(
        Some(GUILD_ID.to_string()),
        Some(CHANNEL_ID.to_string()),
        None,
        None,
        None,
        None,
        Some(vec![]),
    ))
    .expect("voice_state peer left");
    assert_eq!(peer_left.e2ee.participant_count, 1);
    assert!(peer_left.e2ee.waiting_for_peer);
    assert_eq!(peer_left.e2ee.state, "waiting_for_peer");

    // 4. Boundary validation: Snowflake "1" and u64::MAX accepted; "0", non-numeric, or >64 participants rejected
    let boundary_ok: VoiceStateSnapshotDto = run_async(voice_plugin::voice_state(
        Some(GUILD_ID.to_string()),
        Some(CHANNEL_ID.to_string()),
        None,
        None,
        None,
        None,
        Some(vec!["1".to_string(), u64::MAX.to_string()]),
    ))
    .expect("Snowflake 1 and u64::MAX must be accepted");
    assert_eq!(boundary_ok.e2ee.participant_count, 3);
    assert_eq!(boundary_ok.e2ee.state, "handshaking");

    let too_many_peers: Vec<String> = (2..=65).map(|n| n.to_string()).collect();
    let overflow_roster = run_async(voice_plugin::voice_state(
        Some(GUILD_ID.to_string()),
        Some(CHANNEL_ID.to_string()),
        None,
        None,
        None,
        None,
        Some(too_many_peers),
    ));
    assert!(
        overflow_roster.is_err(),
        "Exceeding MAX_VOICE_PARTICIPANTS (64) must be rejected"
    );

    let zero_channel = run_async(voice_plugin::voice_connect(
        Some(GUILD_ID.to_string()),
        Some("0".to_string()),
        Some(false),
        Some(false),
        Some(false),
        Some(false),
    ));
    assert!(
        zero_channel.is_err(),
        "voice_connect with channel_id=0 must be rejected"
    );

    let bad_peer = run_async(voice_plugin::voice_state(
        Some(GUILD_ID.to_string()),
        Some(CHANNEL_ID.to_string()),
        None,
        None,
        None,
        None,
        Some(vec!["0".to_string()]),
    ));
    assert!(bad_peer.is_err(), "Snowflake 0 peer must be rejected");

    let non_numeric_peer = run_async(voice_plugin::voice_state(
        Some(GUILD_ID.to_string()),
        Some(CHANNEL_ID.to_string()),
        None,
        None,
        None,
        None,
        Some(vec!["invalid_snowflake".to_string()]),
    ));
    assert!(
        non_numeric_peer.is_err(),
        "non-numeric peer ID must be rejected"
    );

    let _ = run_async(voice_plugin::voice_disconnect(Some(GUILD_ID.to_string())));
}

#[test]
fn full_dave_mls_handshake_establishes_e2ee_privacy_code_opus_crypto_and_downgrade_guard() {
    let _guard = serial_lock().lock().unwrap_or_else(|p| p.into_inner());
    let _ = run_async(voice_plugin::voice_disconnect(None));

    let chan_u64: u64 = CHANNEL_ID.parse().expect("valid u64 channel");

    let _ = run_async(voice_plugin::voice_connect(
        Some(GUILD_ID.to_string()),
        Some(CHANNEL_ID.to_string()),
        Some(false),
        Some(false),
        Some(false),
        Some(false),
    ))
    .expect("voice_connect");

    // Register peer in active DaveVoiceBridge via voice_state
    let state_snap = run_async(voice_plugin::voice_state(
        Some(GUILD_ID.to_string()),
        Some(CHANNEL_ID.to_string()),
        None,
        None,
        None,
        None,
        Some(vec![PEER_UID_STR.to_string()]),
    ))
    .expect("register peer via voice_state");
    assert_eq!(state_snap.e2ee.state, "handshaking");

    let local_uid = dave_bridge::active_local_user_id().expect("active local uid");

    // Independent clean-room peer oracle (`DaveySession` + `DaveyExternalDelivery`)
    let delivery = DaveyExternalDelivery::new(b"discord-dave-mls").expect("external delivery");
    let ext_pkg = delivery.external_sender_bytes();
    let mut peer_oracle =
        DaveySession::new(DAVE_PROTOCOL_VERSION_V1, PEER_UID, chan_u64).expect("peer DaveySession");
    use dave_spike::bridge::DaveBackend;
    peer_oracle
        .set_external_sender(ext_pkg)
        .expect("peer set_external_sender");
    let peer_kp = peer_oracle.create_key_package().expect("peer key package");

    // Step 1: Op 25 (MLS External Sender) + Op 24 (Prepare Epoch 1 -> emits Op 26 KeyPackage)
    let out_ext = dave_bridge::handle_gateway_binary(DaveGatewayBinaryOp::ExternalSender, ext_pkg)
        .expect("Op 25 ExternalSender");
    assert!(out_ext.is_empty());

    let out_epoch = dave_bridge::handle_prepare_epoch(OP_DAVE_PREPARE_EPOCH, 1, 1)
        .expect("Op 24 PrepareEpoch(1, 1)");
    assert_eq!(out_epoch.len(), 1);
    let local_kp = match &out_epoch[0] {
        dave_spike::bridge::BridgeOutboundFrame::Binary { opcode, payload } => {
            assert_eq!(*opcode, OP_DAVE_MLS_KEY_PACKAGE);
            payload.clone()
        }
        other => panic!("expected Op 26 Binary KeyPackage, got {other:?}"),
    };
    let _ = local_kp;

    // Step 2: Delivery server issues Op 27 Add proposal for PEER_UID; local bridge emits Op 28 Commit+Welcome
    let (proposals, _) = delivery
        .sign_add_proposal(chan_u64, 0, &peer_kp)
        .expect("build Op 27 proposals");
    let out_prop = dave_bridge::handle_gateway_binary(DaveGatewayBinaryOp::Proposals, &proposals)
        .expect("Op 27 Proposals");
    assert_eq!(out_prop.len(), 1);
    let commit_welcome_wire = match &out_prop[0] {
        dave_spike::bridge::BridgeOutboundFrame::Binary { opcode, payload } => {
            assert_eq!(*opcode, OP_DAVE_MLS_COMMIT_WELCOME);
            payload.clone()
        }
        other => panic!("expected Op 28 CommitWelcome, got {other:?}"),
    };

    let (commit_bytes, welcome_opt) =
        DaveyExternalDelivery::split_commit_welcome(&commit_welcome_wire)
            .expect("split CommitWelcome");
    let welcome_bytes = welcome_opt.expect("Welcome message for newly added peer");

    // Peer oracle processes Welcome
    peer_oracle
        .process_welcome(&welcome_bytes, &[local_uid, PEER_UID])
        .expect("peer process_welcome");

    // Step 3: Voice Gateway announces Commit (Op 29, transition_id = 7) -> local emits Op 23 -> Op 22 ExecuteTransition(7)
    let mut op29_payload = Vec::with_capacity(2 + commit_bytes.len());
    op29_payload.extend_from_slice(&7u16.to_be_bytes());
    op29_payload.extend_from_slice(&commit_bytes);
    let out_commit =
        dave_bridge::handle_gateway_binary(DaveGatewayBinaryOp::AnnounceCommit, &op29_payload)
            .expect("Op 29 AnnounceCommit");
    assert_eq!(out_commit.len(), 1);

    dave_bridge::handle_execute_transition(OP_DAVE_EXECUTE_TRANSITION, 7)
        .expect("Op 22 ExecuteTransition(7)");

    // Step 4: Verify `voice_e2ee_status` now reports verified E2EE + 30-digit privacy code matching peer oracle!
    let e2ee = run_async(voice_plugin::voice_e2ee_status(None))
        .expect("voice_e2ee_status after MLS transition");
    assert_eq!(e2ee.state, "encrypted");
    assert!(e2ee.is_end_to_end_encrypted);
    assert!(e2ee.dave_verified);
    assert_eq!(e2ee.protocol_version, 1);
    assert_eq!(e2ee.epoch, Some(1));

    let oracle_code = peer_oracle
        .voice_privacy_code()
        .expect("peer oracle privacy code");
    assert_eq!(
        e2ee.privacy_code.as_deref(),
        Some(oracle_code.as_str()),
        "DaveVoiceBridge 30-digit Voice Privacy Code must match independent peer MLS oracle byte-for-byte"
    );
    assert_eq!(e2ee.privacy_code_chunks.len(), 6);
    assert!(e2ee
        .privacy_code_chunks
        .iter()
        .all(|chunk| chunk.len() == 5 && chunk.chars().all(|c| c.is_ascii_digit())));

    // Step 5: Real Opus frame E2EE roundtrip through active DaveVoiceBridge
    let pcm_opus = b"\x48\x65\x6c\x6c\x6f\x2d\x44\x41\x56\x45\x2d\x4f\x70\x75\x73";
    let encrypted_frame =
        dave_bridge::encrypt_active_opus(pcm_opus).expect("encrypt Opus via DaveVoiceBridge");
    assert_ne!(encrypted_frame.as_slice(), pcm_opus);
    assert_eq!(
        &encrypted_frame[encrypted_frame.len() - 2..],
        &DAVE_MAGIC_MARKER,
        "DAVE encrypted frame must end with 0xFAFA marker"
    );
    let peer_decrypted = peer_oracle
        .decrypt(
            local_uid,
            dave_spike::protocol::MediaType::Audio,
            &encrypted_frame,
        )
        .expect("peer decrypts local DAVE Opus frame");
    assert_eq!(peer_decrypted.as_slice(), pcm_opus);

    // Reverse direction: peer encrypts -> local DaveVoiceBridge decrypts
    let peer_ct = peer_oracle
        .encrypt(
            dave_spike::protocol::MediaType::Audio,
            dave_spike::protocol::Codec::Opus,
            pcm_opus,
        )
        .expect("peer encrypts Opus");
    let local_pt =
        dave_bridge::decrypt_active_opus(PEER_UID, &peer_ct).expect("local decrypts peer Opus");
    assert_eq!(local_pt.as_slice(), pcm_opus);

    // Step 5b: Same-channel voice_connect / voice_mute mid-call preserves active MLS epoch & privacy code
    let mute_mid_call = run_async(voice_plugin::voice_mute(true)).expect("voice_mute mid-call");
    assert_eq!(mute_mid_call["e2ee"]["state"], json!("encrypted"));
    assert_eq!(mute_mid_call["e2ee"]["epoch"], json!(1));
    assert_eq!(
        mute_mid_call["e2ee"]["privacyCode"],
        json!(oracle_code.as_str())
    );
    let reconnect_same = run_async(voice_plugin::voice_connect(
        Some(GUILD_ID.to_string()),
        Some(CHANNEL_ID.to_string()),
        Some(false),
        Some(false),
        Some(false),
        Some(false),
    ))
    .expect("same-channel voice_connect update");
    assert_eq!(reconnect_same["e2ee"]["state"], json!("encrypted"));
    assert_eq!(reconnect_same["e2ee"]["epoch"], json!(1));

    // Step 6: Protocol downgrade attack (Op 21 protocol_version = 0 while active == 1) MUST be rejected!
    let downgrade_err = dave_bridge::handle_prepare_transition(OP_DAVE_PREPARE_TRANSITION, 9, 0);
    assert!(
        downgrade_err.is_err(),
        "DAVE downgrade to protocol_version=0 must be rejected"
    );
    let post_downgrade =
        run_async(voice_plugin::voice_e2ee_status(None)).expect("status after blocked downgrade");
    assert!(post_downgrade.downgrade_blocked);
    assert!(post_downgrade.is_end_to_end_encrypted);

    // Step 7: Stream signaling passthrough (Op 18..22) works transparently
    let stream_ev = dave_bridge::handle_stream_signaling(
        OP_STREAM_CREATE,
        r#"{"type":"guild","channel_id":"1"}"#,
    )
    .expect("Op 18 StreamCreate passthrough");
    assert!(matches!(
        stream_ev,
        dave_spike::bridge::StreamSignalingEvent::Create { .. }
    ));

    // Step 8: Malformed Op 30 Welcome triggers Op 31 auto-recovery (`recovering` state, resets == 1)
    let bad_welcome = [0u8, 12u8, 0xde, 0xad, 0xbe, 0xef];
    let recovery_frames =
        dave_bridge::handle_gateway_binary(DaveGatewayBinaryOp::Welcome, &bad_welcome)
            .expect("Op 30 malformed welcome triggers Op 31 recovery");
    assert_eq!(recovery_frames.len(), 2);
    assert!(matches!(
        recovery_frames[0],
        dave_spike::bridge::BridgeOutboundFrame::JsonInvalidCommitWelcome {
            opcode: OP_DAVE_MLS_INVALID_COMMIT_WELCOME,
            transition_id: 12
        }
    ));
    let recovering_status =
        run_async(voice_plugin::voice_e2ee_status(None)).expect("status during recovery");
    assert_eq!(recovering_status.state, "recovering");
    assert_eq!(recovering_status.resets, 1);
    assert!(!recovering_status.is_end_to_end_encrypted);

    // Step 9: Disconnect tears down DaveVoiceBridge cleanly
    let _ = run_async(voice_plugin::voice_disconnect(Some(GUILD_ID.to_string())))
        .expect("voice_disconnect");
    let final_status =
        run_async(voice_plugin::voice_e2ee_status(None)).expect("status after disconnect");
    assert_eq!(final_status.state, "disconnected");
    assert!(!final_status.is_end_to_end_encrypted);
    assert!(final_status.privacy_code.is_none());
}

#[test]
fn voice_state_while_disconnected_does_not_leak_session_and_downgrade_beats_recovery() {
    let _guard = serial_lock().lock().unwrap_or_else(|p| p.into_inner());
    let _ = run_async(voice_plugin::voice_disconnect(None));

    // Stale `voice_state` poll arriving after `voice_disconnect` MUST NOT resurrect ActiveDaveSession
    let stale_snap = run_async(voice_plugin::voice_state(
        Some(GUILD_ID.to_string()),
        Some(CHANNEL_ID.to_string()),
        None,
        None,
        None,
        None,
        Some(vec![PEER_UID_STR.to_string()]),
    ))
    .expect("voice_state while disconnected");
    assert!(
        !stale_snap.connected,
        "disconnected voice_state must report connected=false"
    );
    assert_eq!(stale_snap.state, "disconnected");
    assert_eq!(stale_snap.e2ee.state, "disconnected");
    assert_eq!(
        dave_bridge::active_local_user_id(),
        None,
        "stale voice_state while disconnected must not allocate an active DaveVoiceBridge session"
    );

    // Connect, activate protocol v1 (transition_id=0), trigger Op 30 reset, then trigger Op 21 downgrade:
    // `downgrade_blocked` security alert must take precedence over `recovering`!
    let _ = run_async(voice_plugin::voice_connect(
        Some(GUILD_ID.to_string()),
        Some(CHANNEL_ID.to_string()),
        Some(false),
        Some(false),
        Some(false),
        Some(false),
    ))
    .expect("voice_connect");

    dave_bridge::handle_prepare_transition(OP_DAVE_PREPARE_TRANSITION, 0, 1)
        .expect("activate protocol v1 via transition_id=0");
    let bad_welcome = [0u8, 5u8, 0xca, 0xfe, 0xba, 0xbe];
    let _ = dave_bridge::handle_gateway_binary(DaveGatewayBinaryOp::Welcome, &bad_welcome)
        .expect("Op 30 malformed welcome triggers reset");
    let rec_status = run_async(voice_plugin::voice_e2ee_status(None)).expect("recovering status");
    assert_eq!(rec_status.state, "recovering");
    assert_eq!(rec_status.resets, 1);

    let downgrade_res = dave_bridge::handle_prepare_transition(OP_DAVE_PREPARE_TRANSITION, 9, 0);
    assert!(downgrade_res.is_err(), "downgrade to v0 must fail");

    let blocked_status =
        run_async(voice_plugin::voice_e2ee_status(None)).expect("downgrade_blocked status");
    assert!(blocked_status.downgrade_blocked);
    assert_eq!(
        blocked_status.state, "downgrade_blocked",
        "security state `downgrade_blocked` must take priority over `recovering` when unencrypted"
    );

    let _ = run_async(voice_plugin::voice_disconnect(Some(GUILD_ID.to_string())));
}
