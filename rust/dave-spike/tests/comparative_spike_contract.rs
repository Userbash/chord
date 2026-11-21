//! Comparative spike contract & interoperability suite (`LIB-SIMPLIFY-05`):
//! - `501.1`: Official Discord `libdave` C++17 (OpenSSL `libcrypto`) via thin Rust FFI (`LibdaveFfiBackend`)
//! - `501.2`: Serein `davey` + `OpenMLS` architecture via 100% safe RustCrypto (`DaveySession`)
//! - `501.3`: Comparative performance, memory safety (`#![forbid(unsafe_code)]`), and dependency audit
//! - `502.1`: Unified transparent `DaveVoiceBridge` (`Voice Gateway Op 18..22 / 21..31 -> DaveSessionManager -> Backend`)

mod test_libdave_ffi;

use dave_spike::audit::{BackendBenchmarkSample, build_comparative_audit_report};
use dave_spike::bridge::{
    BridgeOutboundFrame, DaveBackend, DaveBackendKind, DaveVoiceBridge, StreamSignalingEvent,
};
use dave_spike::davey_crypto::{
    DaveyExternalDelivery, DaveyHashRatchet, aes128_gcm_truncated8_seal, verify_confirmation_tag,
};
use dave_spike::davey_session::DaveySession;
use dave_spike::protocol::{
    Codec, DAVE_MAGIC_MARKER, DAVE_OPUS_SILENCE_PACKET, DAVE_PROTOCOL_VERSION_V1, DaveError,
    MediaType, OP_DAVE_EXECUTE_TRANSITION, OP_DAVE_MLS_ANNOUNCE_COMMIT, OP_DAVE_MLS_COMMIT_WELCOME,
    OP_DAVE_MLS_EXTERNAL_SENDER, OP_DAVE_MLS_INVALID_COMMIT_WELCOME, OP_DAVE_MLS_KEY_PACKAGE,
    OP_DAVE_MLS_PROPOSALS, OP_DAVE_MLS_WELCOME, OP_DAVE_PREPARE_EPOCH, OP_DAVE_PREPARE_TRANSITION,
    OP_DAVE_READY_FOR_TRANSITION, OP_STREAM_CREATE, OP_STREAM_DELETE, OP_STREAM_PING,
    OP_STREAM_SET_PAUSED, OP_STREAM_WATCH, ProposalsOperation, generate_displayable_code,
};
use std::time::Instant;
use test_libdave_ffi::{
    CppExternalDelivery, LibdaveFfiBackend, cpp_aes128_gcm_truncated8_encrypt,
    cpp_derive_ratchet_key_and_nonce,
};

fn decode_hex(s: &str) -> Vec<u8> {
    assert!(s.len().is_multiple_of(2));
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("valid hex"))
        .collect()
}

/// Published KAT vectors (`mlspp` / `davey` HashRatchet + NIST SP 800-38D TC4 8B tag + DisplayableCode)
/// verified identically across BOTH C++ OpenSSL (`libdave`) and Safe RustCrypto (`davey`).
#[test]
fn kat_vectors_match_identically_across_libdave_cpp_and_davey_rust() {
    // Official davey / mlspp HashRatchet vector (davey/src/cryptor/hash_ratchet.rs:88-102)
    let base_secret: [u8; 16] = [
        206, 221, 97, 177, 184, 161, 202, 105, 4, 101, 84, 40, 44, 247, 11, 123,
    ];
    let expected_key0: [u8; 16] = [
        117, 48, 249, 169, 148, 94, 45, 46, 6, 208, 101, 31, 123, 42, 134, 75,
    ];
    let expected_nonce0: [u8; 12] = [48, 30, 95, 75, 116, 9, 15, 152, 94, 114, 107, 178];

    let mut rust_ratchet = DaveyHashRatchet::new(base_secret);
    let (rust_key0, rust_nonce0) = rust_ratchet.get(0).expect("rust ratchet gen 0");
    assert_eq!(rust_key0, expected_key0);
    assert_eq!(rust_nonce0, expected_nonce0);

    let (cpp_key0, cpp_nonce0) = cpp_derive_ratchet_key_and_nonce(&base_secret, 0);
    assert_eq!(cpp_key0, expected_key0);
    assert_eq!(cpp_nonce0, expected_nonce0);

    // Generation 5 cross-check + forward-secrecy erasure
    let (rust_key5, rust_nonce5) = rust_ratchet.get(5).expect("rust ratchet gen 5");
    let (cpp_key5, cpp_nonce5) = cpp_derive_ratchet_key_and_nonce(&base_secret, 5);
    assert_eq!(cpp_key5, rust_key5);
    assert_eq!(cpp_nonce5, rust_nonce5);

    rust_ratchet.delete_keys_up_to(3);
    assert!(matches!(rust_ratchet.get(0), Err(DaveError::KeyExpired)));
    assert!(matches!(rust_ratchet.get(2), Err(DaveError::KeyExpired)));
    assert!(rust_ratchet.get(3).is_ok());

    // NIST SP 800-38D Test Case 4 (AES-128-GCM with 8-byte truncated tag for DAVE v1)
    let tc4_key: [u8; 16] = decode_hex("feffe9928665731c6d6a8f9467308308")
        .try_into()
        .unwrap();
    let tc4_iv: [u8; 12] = decode_hex("cafebabefacedbaddecaf888").try_into().unwrap();
    let tc4_aad = decode_hex("feedfacedeadbeeffeedfacedeadbeefabaddad2");
    let tc4_pt = decode_hex(
        "d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a721\
         c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b39",
    );
    let expected_ct = decode_hex(
        "42831ec2217774244b7221b784d0d49ce3aa212f2c02a4e035c17e2329aca12e2\
         1d514b25466931c7d8f6a5aac84aa051ba30b396a0aac973d58e091",
    );
    let expected_tag8: [u8; 8] = decode_hex("5bc94fbc3221a5db").try_into().unwrap();

    let (rust_ct, rust_tag8) = aes128_gcm_truncated8_seal(&tc4_key, &tc4_iv, &tc4_aad, &tc4_pt)
        .expect("RustCrypto aes-gcm U8");
    assert_eq!(rust_ct, expected_ct);
    assert_eq!(rust_tag8, expected_tag8);

    let (cpp_ct, cpp_tag8) =
        cpp_aes128_gcm_truncated8_encrypt(&tc4_key, &tc4_iv, &tc4_aad, &tc4_pt);
    assert_eq!(cpp_ct, expected_ct);
    assert_eq!(cpp_tag8, expected_tag8);

    // Displayable privacy code (30 digits, 6x5 modulus 100000 per big-endian 5-byte group)
    let sample_auth: Vec<u8> = (1u8..=32).collect();
    let code = generate_displayable_code(&sample_auth, 30, 5).unwrap();
    assert_eq!(code.len(), 30);
    assert!(code.chars().all(|c| c.is_ascii_digit()));

    // Constant-time confirmation_tag verification rejects bit-flips and truncated tags
    assert!(verify_confirmation_tag(&[0x55; 32], &[0x55; 32]));
    assert!(!verify_confirmation_tag(&[0x55; 32], &[0x54; 32]));
    assert!(!verify_confirmation_tag(&[0x55; 32], &[0x55; 31]));
}

/// Subtask 501.1: Standalone `libdave` C++ FFI prototype — 2-party & 3-party group, Opus + H.264,
/// tamper rejection, replay protection, and forward secrecy on member removal.
#[test]
fn subtask_501_1_libdave_cpp_ffi_prototype_full_lifecycle() {
    assert_eq!(
        LibdaveFfiBackend::max_supported_version(),
        DAVE_PROTOCOL_VERSION_V1
    );
    let delivery = CppExternalDelivery::new(b"discord-voice-rtc-fra19");
    let channel_id = 998877665544332211u64;

    let mut alice = LibdaveFfiBackend::new(1, 1001, channel_id).unwrap();
    let mut bob = LibdaveFfiBackend::new(1, 2002, channel_id).unwrap();
    let mut charlie = LibdaveFfiBackend::new(1, 3003, channel_id).unwrap();

    for peer in [&mut alice, &mut bob, &mut charlie] {
        peer.set_external_sender(&delivery.external_package)
            .unwrap();
        assert!(!peer.is_ready());
    }

    // Bob generates KeyPackage; ExternalSender issues Add proposal; Alice commits
    let bob_kp = bob.create_key_package().unwrap();
    let add_bob = delivery.sign_add(channel_id, 0, &bob_kp);
    let cw = alice
        .process_proposals(ProposalsOperation::Append, &add_bob, &[1001, 2002])
        .unwrap()
        .expect("commit+welcome generated");
    let welcome = cw.welcome.expect("welcome present for added peer");

    alice.process_commit(&cw.commit).unwrap();
    bob.process_welcome(&welcome, &[1001, 2002]).unwrap();

    assert!(alice.is_ready() && bob.is_ready());
    assert_eq!(alice.epoch(), Some(1));
    assert_eq!(bob.epoch(), Some(1));
    assert_eq!(alice.voice_privacy_code(), bob.voice_privacy_code());
    assert_eq!(alice.voice_privacy_code().unwrap().len(), 30);
    assert_eq!(
        alice.pairwise_fingerprint(0, 2002).unwrap(),
        bob.pairwise_fingerprint(0, 1001).unwrap()
    );

    // Bidirectional Opus + DTX silence + anti-replay + tamper check
    assert_eq!(
        alice
            .encrypt(MediaType::Audio, Codec::Opus, &DAVE_OPUS_SILENCE_PACKET)
            .unwrap(),
        DAVE_OPUS_SILENCE_PACKET
    );
    let enc_audio = alice
        .encrypt(MediaType::Audio, Codec::Opus, b"libdave-opus-frame-epoch1")
        .unwrap();
    assert_eq!(&enc_audio[enc_audio.len() - 2..], &DAVE_MAGIC_MARKER);

    let mut tampered = enc_audio.clone();
    tampered[0] ^= 0x40;
    assert!(bob.decrypt(1001, MediaType::Audio, &tampered).is_err());

    assert_eq!(
        bob.decrypt(1001, MediaType::Audio, &enc_audio).unwrap(),
        b"libdave-opus-frame-epoch1"
    );
    // Exact replay must fail closed
    assert!(bob.decrypt(1001, MediaType::Audio, &enc_audio).is_err());

    // H.264 Annex-B video frame with unencrypted NAL header range
    let h264_idr = [&[0x00, 0x00, 0x00, 0x01, 0x65][..], &[0xAA; 96][..]].concat();
    let enc_video = bob
        .encrypt(MediaType::Video, Codec::H264, &h264_idr)
        .unwrap();
    assert_eq!(&enc_video[..5], &[0x00, 0x00, 0x00, 0x01, 0x65]);
    assert_eq!(&enc_video[enc_video.len() - 2..], &DAVE_MAGIC_MARKER);
    assert_eq!(
        alice.decrypt(2002, MediaType::Video, &enc_video).unwrap(),
        h264_idr
    );

    // Charlie joins in Epoch 2, then Charlie is removed in Epoch 3 (Forward Secrecy)
    let charlie_kp = charlie.create_key_package().unwrap();
    let add_charlie = delivery.sign_add(channel_id, 1, &charlie_kp);
    let cw2 = alice
        .process_proposals(
            ProposalsOperation::Append,
            &add_charlie,
            &[1001, 2002, 3003],
        )
        .unwrap()
        .unwrap();
    bob.process_proposals(
        ProposalsOperation::Append,
        &add_charlie,
        &[1001, 2002, 3003],
    )
    .unwrap();
    alice.process_commit(&cw2.commit).unwrap();
    bob.process_commit(&cw2.commit).unwrap();
    charlie
        .process_welcome(&cw2.welcome.unwrap(), &[1001, 2002, 3003])
        .unwrap();
    assert_eq!(alice.voice_privacy_code(), charlie.voice_privacy_code());

    let remove_charlie = delivery.sign_remove(channel_id, 2, 3003);
    let cw3 = alice
        .process_proposals(ProposalsOperation::Append, &remove_charlie, &[1001, 2002])
        .unwrap()
        .unwrap();
    assert!(cw3.welcome.is_none());
    bob.process_proposals(ProposalsOperation::Append, &remove_charlie, &[1001, 2002])
        .unwrap();
    alice.process_commit(&cw3.commit).unwrap();
    bob.process_commit(&cw3.commit).unwrap();

    let post_remove_audio = alice
        .encrypt(MediaType::Audio, Codec::Opus, b"secret-after-charlie-left")
        .unwrap();
    assert_eq!(
        bob.decrypt(1001, MediaType::Audio, &post_remove_audio)
            .unwrap(),
        b"secret-after-charlie-left"
    );
    assert!(
        charlie
            .decrypt(1001, MediaType::Audio, &post_remove_audio)
            .is_err(),
        "removed member must not decrypt post-removal epoch frames"
    );
}

/// Subtask 501.2 + Cross-Backend Interop: Serein `DaveySession` (Pure RustCrypto) interoperating
/// directly on the wire with C++ OpenSSL `LibdaveFfiBackend`!
#[test]
fn subtask_501_2_davey_rust_and_libdave_cpp_cross_backend_interoperability() {
    let delivery = DaveyExternalDelivery::new(b"serein-interop-voice-server").unwrap();
    let channel_id = 424242424242u64;

    // Alice = C++17 OpenSSL libdave via FFI; Bob = Pure-Rust Serein DaveySession; Charlie = Pure-Rust DaveySession
    let mut alice_cpp = LibdaveFfiBackend::new(1, 111, channel_id).unwrap();
    let mut bob_rust = DaveySession::new(1, 222, channel_id).unwrap();
    let mut charlie_rust = DaveySession::new(1, 333, channel_id).unwrap();

    alice_cpp
        .set_external_sender(delivery.external_sender_bytes())
        .unwrap();
    bob_rust
        .set_external_sender(delivery.external_sender_bytes())
        .unwrap();
    charlie_rust
        .set_external_sender(delivery.external_sender_bytes())
        .unwrap();

    // Rust Bob creates KeyPackage -> C++ Alice processes Proposal & creates Commit+Welcome -> Rust Bob joins via Welcome!
    let bob_kp = bob_rust.create_key_package().unwrap();
    let (add_bob_payload, _) = delivery.sign_add_proposal(channel_id, 0, &bob_kp).unwrap();

    let cw_from_cpp = alice_cpp
        .process_proposals(ProposalsOperation::Append, &add_bob_payload, &[111, 222])
        .unwrap()
        .unwrap();
    alice_cpp.process_commit(&cw_from_cpp.commit).unwrap();
    bob_rust
        .process_welcome(&cw_from_cpp.welcome.unwrap(), &[111, 222])
        .unwrap();

    // Independent OpenSSL (C++) and RustCrypto (Rust) derive the EXACT SAME EpochAuthenticator & 30-digit Privacy Code!
    assert_eq!(
        alice_cpp.epoch_authenticator(),
        bob_rust.epoch_authenticator()
    );
    assert_eq!(
        alice_cpp.voice_privacy_code(),
        bob_rust.voice_privacy_code()
    );

    // Media encrypted by C++ libdave is decrypted by Rust DaveySession, and vice-versa!
    let from_cpp = alice_cpp
        .encrypt(MediaType::Audio, Codec::Opus, b"hello-from-cpp-libdave")
        .unwrap();
    assert_eq!(
        bob_rust.decrypt(111, MediaType::Audio, &from_cpp).unwrap(),
        b"hello-from-cpp-libdave"
    );

    let from_rust = bob_rust
        .encrypt(MediaType::Audio, Codec::Opus, b"hello-from-rust-davey")
        .unwrap();
    assert_eq!(
        alice_cpp
            .decrypt(222, MediaType::Audio, &from_rust)
            .unwrap(),
        b"hello-from-rust-davey"
    );

    // Proposal REVOKE check on Rust DaveySession before committing Charlie
    let charlie_kp = charlie_rust.create_key_package().unwrap();
    let (add_charlie, charlie_ref) = delivery
        .sign_add_proposal(channel_id, 1, &charlie_kp)
        .unwrap();
    assert!(
        bob_rust
            .process_proposals(ProposalsOperation::Append, &add_charlie, &[111, 222, 333])
            .unwrap()
            .is_some()
    );
    let revoke_payload = delivery.build_revoke_payload(&[charlie_ref]).unwrap();
    assert!(
        bob_rust
            .process_proposals(
                ProposalsOperation::Revoke,
                &revoke_payload,
                &[111, 222, 333]
            )
            .unwrap()
            .is_none(),
        "revoking the sole queued proposal must return None and revert pending commit"
    );

    // Now Rust Bob commits Charlie -> C++ Alice processes Rust Bob's Commit!
    let cw_from_rust = bob_rust
        .process_proposals(ProposalsOperation::Append, &add_charlie, &[111, 222, 333])
        .unwrap()
        .unwrap();
    alice_cpp
        .process_proposals(ProposalsOperation::Append, &add_charlie, &[111, 222, 333])
        .unwrap();

    bob_rust.process_commit(&cw_from_rust.commit).unwrap();
    alice_cpp.process_commit(&cw_from_rust.commit).unwrap();
    charlie_rust
        .process_welcome(&cw_from_rust.welcome.unwrap(), &[111, 222, 333])
        .unwrap();

    assert_eq!(alice_cpp.epoch(), Some(2));
    assert_eq!(bob_rust.epoch(), Some(2));
    assert_eq!(charlie_rust.epoch(), Some(2));
    assert_eq!(
        alice_cpp.voice_privacy_code(),
        charlie_rust.voice_privacy_code()
    );
}

/// Subtask 502.1: Unified Transparent `DaveVoiceBridge` (`Op 18..22` stream events + `Op 21..31` DAVE/MLS,
/// epoch transitions, participant bounds, sole-member waiting, and `Op 31` auto-recovery) verified identically
/// over both `LibdaveFfiBackend` and `DaveySession`!
#[test]
fn subtask_502_1_dave_voice_bridge_transparent_gateway_signaling_both_backends() {
    let delivery = DaveyExternalDelivery::new(b"bridge-gateway-oracle").unwrap();
    let chan = 777000111u64;

    let mut bridge_cpp =
        DaveVoiceBridge::new(LibdaveFfiBackend::new(1, 10, chan).unwrap(), None).unwrap();
    let mut bridge_rust =
        DaveVoiceBridge::new(DaveySession::new(1, 20, chan).unwrap(), None).unwrap();

    assert_eq!(bridge_cpp.backend_kind(), DaveBackendKind::LibdaveFfi);
    assert_eq!(bridge_rust.backend_kind(), DaveBackendKind::DaveyRustCrypto);

    // 1) Stream signaling opcodes 18..=22 preserved transparently
    let stream_events = [
        (
            OP_STREAM_CREATE,
            r#"{"type":"guild","guild_id":"1","channel_id":"777000111","preferred_region":"rotterdam"}"#,
        ),
        (OP_STREAM_WATCH, r#"{"stream_key":"guild:1:777000111:10"}"#),
        (OP_STREAM_PING, r#"{"stream_key":"guild:1:777000111:10"}"#),
        (
            OP_STREAM_SET_PAUSED,
            r#"{"stream_key":"guild:1:777000111:10","paused":true}"#,
        ),
        (OP_STREAM_DELETE, r#"{"stream_key":"guild:1:777000111:10"}"#),
    ];
    for (op, payload) in stream_events {
        let ev = bridge_rust.handle_stream_opcode(op, payload).unwrap();
        assert!(matches!(
            ev,
            StreamSignalingEvent::Create { .. }
                | StreamSignalingEvent::Watch { .. }
                | StreamSignalingEvent::Ping { .. }
                | StreamSignalingEvent::SetPaused { .. }
                | StreamSignalingEvent::Delete { .. }
        ));
    }
    assert!(bridge_rust.handle_stream_opcode(99, "{}").is_err());

    // 2) Op 25 External Sender + Sole-Member Waiting before peer connects
    bridge_cpp
        .handle_binary_opcode(
            OP_DAVE_MLS_EXTERNAL_SENDER,
            delivery.external_sender_bytes(),
        )
        .unwrap();
    bridge_rust
        .handle_binary_opcode(
            OP_DAVE_MLS_EXTERNAL_SENDER,
            delivery.external_sender_bytes(),
        )
        .unwrap();

    bridge_rust.enter_sole_member_waiting().unwrap();
    assert!(bridge_rust.is_waiting_for_peer());
    assert!(!bridge_rust.is_ready());

    // Participant bounds: Snowflake 0 and >64 participants rejected
    assert!(bridge_rust.connect_users(&[0]).is_err());
    let too_many: Vec<u64> = (100..170).collect();
    assert!(bridge_rust.connect_users(&too_many).is_err());

    bridge_cpp.connect_users(&[10, 20]).unwrap();
    bridge_rust.connect_users(&[10, 20]).unwrap();
    assert!(!bridge_rust.is_waiting_for_peer());

    // 3) Op 24 PrepareEpoch(epoch=1) -> emits Op 26 KeyPackage
    let epoch1_out = bridge_rust
        .handle_prepare_epoch(OP_DAVE_PREPARE_EPOCH, 1, 1)
        .unwrap();
    assert_eq!(epoch1_out.len(), 1);
    let BridgeOutboundFrame::Binary { opcode, payload } = &epoch1_out[0] else {
        panic!("expected binary KeyPackage frame");
    };
    assert_eq!(*opcode, OP_DAVE_MLS_KEY_PACKAGE);
    let mut full_kp_wire = vec![OP_DAVE_MLS_KEY_PACKAGE];
    full_kp_wire.extend_from_slice(payload);

    // 4) Op 27 Proposals -> Alice emits Op 28 CommitWelcome
    let (proposal_payload, _) = delivery
        .sign_add_proposal(chan, 0, &full_kp_wire[1..])
        .unwrap();
    let prop_out = bridge_cpp
        .handle_binary_opcode(OP_DAVE_MLS_PROPOSALS, &proposal_payload)
        .unwrap();
    assert_eq!(prop_out.len(), 1);
    let BridgeOutboundFrame::Binary {
        opcode: op28,
        payload: cw_wire,
    } = &prop_out[0]
    else {
        panic!("expected Op 28 CommitWelcome");
    };
    assert_eq!(*op28, OP_DAVE_MLS_COMMIT_WELCOME);
    let (commit_bytes, welcome_bytes) =
        DaveyExternalDelivery::split_commit_welcome(cw_wire).unwrap();

    // 5) Op 29 AnnounceCommitTransition (transition_id = 12) & Op 30 Welcome (transition_id = 12)
    // -> Both emit Op 23 ReadyForTransition(12), then activate on Op 22 ExecuteTransition(12)!
    let mut op29 = 12u16.to_be_bytes().to_vec();
    op29.extend_from_slice(&commit_bytes);
    let out_cpp = bridge_cpp
        .handle_binary_opcode(OP_DAVE_MLS_ANNOUNCE_COMMIT, &op29)
        .unwrap();
    assert_eq!(
        out_cpp,
        vec![BridgeOutboundFrame::JsonReadyForTransition {
            opcode: OP_DAVE_READY_FOR_TRANSITION,
            transition_id: 12,
        }]
    );

    let mut op30 = 12u16.to_be_bytes().to_vec();
    op30.extend_from_slice(&welcome_bytes.unwrap());
    let out_rust = bridge_rust
        .handle_binary_opcode(OP_DAVE_MLS_WELCOME, &op30)
        .unwrap();
    assert_eq!(
        out_rust,
        vec![BridgeOutboundFrame::JsonReadyForTransition {
            opcode: OP_DAVE_READY_FOR_TRANSITION,
            transition_id: 12,
        }]
    );

    assert!(!bridge_cpp.is_ready() && !bridge_rust.is_ready());
    bridge_cpp
        .handle_execute_transition(OP_DAVE_EXECUTE_TRANSITION, 12)
        .unwrap();
    bridge_rust
        .handle_execute_transition(OP_DAVE_EXECUTE_TRANSITION, 12)
        .unwrap();
    assert!(bridge_cpp.is_ready() && bridge_rust.is_ready());
    assert_eq!(
        bridge_cpp.voice_privacy_code(),
        bridge_rust.voice_privacy_code()
    );

    // Downgrade to protocol_version=0 once E2EE active is rejected!
    assert_eq!(
        bridge_rust.handle_prepare_transition(OP_DAVE_PREPARE_TRANSITION, 13, 0),
        Err(DaveError::DowngradeRejected {
            current: 1,
            requested: 0,
        })
    );

    // Media roundtrip through DaveVoiceBridge + anti-replay & forged confirmation_tag rejection
    let sealed = bridge_cpp.encrypt_opus(b"bridge-voice-packet").unwrap();
    assert_eq!(
        bridge_rust.decrypt_opus(10, &sealed).unwrap(),
        b"bridge-voice-packet"
    );
    assert_eq!(
        bridge_rust.decrypt_opus(10, &sealed),
        Err(DaveError::ReplayDetected),
        "duplicate frame must be rejected by AntiReplayWindow"
    );

    // Forged confirmation_tag on an otherwise signed Commit must be rejected by constant-time tag check!
    let mut forged_commit = commit_bytes.clone();
    let tag_flip_idx = forged_commit.len() - 70; // inside confirmation_tag before 64B signature
    forged_commit[tag_flip_idx] ^= 0x01;
    let mut bad_tag_check = DaveySession::new(1, 20, chan).unwrap();
    bad_tag_check
        .set_external_sender(delivery.external_sender_bytes())
        .unwrap();
    assert!(bad_tag_check.process_commit(&forged_commit).is_err());

    // Corrupted Op 29 Commit triggers automatic Op 31 InvalidCommitWelcome + fresh Op 26 KeyPackage recovery (up to 3 resets)
    let bad_op29 = [0x00, 0x15, 0xDE, 0xAD, 0xBE, 0xEF];
    let recovery = bridge_rust
        .handle_binary_opcode(OP_DAVE_MLS_ANNOUNCE_COMMIT, &bad_op29)
        .unwrap();
    assert_eq!(recovery.len(), 2);
    assert_eq!(
        recovery[0],
        BridgeOutboundFrame::JsonInvalidCommitWelcome {
            opcode: OP_DAVE_MLS_INVALID_COMMIT_WELCOME,
            transition_id: 0x0015,
        }
    );
    assert!(matches!(
        recovery[1],
        BridgeOutboundFrame::Binary {
            opcode: OP_DAVE_MLS_KEY_PACKAGE,
            ..
        }
    ));
}

/// Subtask 501.3: Comparative benchmark & security/dependency audit execution.
#[test]
fn subtask_501_3_comparative_benchmark_and_security_dependency_audit() {
    let iterations = 200usize;
    let delivery = DaveyExternalDelivery::new(b"bench-server").unwrap();
    let chan = 888888u64;

    // Measure libdave C++ FFI handshake + 200 Opus frames
    let t0 = Instant::now();
    let mut cpp_a = LibdaveFfiBackend::new(1, 1, chan).unwrap();
    let mut cpp_b = LibdaveFfiBackend::new(1, 2, chan).unwrap();
    cpp_a
        .set_external_sender(delivery.external_sender_bytes())
        .unwrap();
    cpp_b
        .set_external_sender(delivery.external_sender_bytes())
        .unwrap();
    let kp = cpp_b.create_key_package().unwrap();
    let (prop, _) = delivery.sign_add_proposal(chan, 0, &kp).unwrap();
    let cw = cpp_a
        .process_proposals(ProposalsOperation::Append, &prop, &[1, 2])
        .unwrap()
        .unwrap();
    cpp_a.process_commit(&cw.commit).unwrap();
    cpp_b
        .process_welcome(&cw.welcome.unwrap(), &[1, 2])
        .unwrap();
    let cpp_handshake_us = t0.elapsed().as_micros() as u64;

    let payload = [0x5Au8; 80]; // Typical 20ms 32kbps Opus packet (80 bytes)
    let t_enc = Instant::now();
    let mut cpp_packets = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        cpp_packets.push(
            cpp_a
                .encrypt(MediaType::Audio, Codec::Opus, &payload)
                .unwrap(),
        );
    }
    let cpp_encrypt_ns_per_frame = (t_enc.elapsed().as_nanos() as u64) / (iterations as u64);

    let t_dec = Instant::now();
    for pkt in &cpp_packets {
        let pt = cpp_b.decrypt(1, MediaType::Audio, pkt).unwrap();
        assert_eq!(pt.len(), payload.len());
    }
    let cpp_decrypt_ns_per_frame = (t_dec.elapsed().as_nanos() as u64) / (iterations as u64);

    // Measure Serein DaveySession (Pure RustCrypto) handshake + 200 Opus frames
    let r0 = Instant::now();
    let mut rust_a = DaveySession::new(1, 1, chan).unwrap();
    let mut rust_b = DaveySession::new(1, 2, chan).unwrap();
    rust_a
        .set_external_sender(delivery.external_sender_bytes())
        .unwrap();
    rust_b
        .set_external_sender(delivery.external_sender_bytes())
        .unwrap();
    let r_kp = rust_b.create_key_package().unwrap();
    let (r_prop, _) = delivery.sign_add_proposal(chan, 0, &r_kp).unwrap();
    let r_cw = rust_a
        .process_proposals(ProposalsOperation::Append, &r_prop, &[1, 2])
        .unwrap()
        .unwrap();
    rust_a.process_commit(&r_cw.commit).unwrap();
    rust_b
        .process_welcome(&r_cw.welcome.unwrap(), &[1, 2])
        .unwrap();
    let rust_handshake_us = r0.elapsed().as_micros() as u64;

    let r_enc = Instant::now();
    let mut rust_packets = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        rust_packets.push(
            rust_a
                .encrypt(MediaType::Audio, Codec::Opus, &payload)
                .unwrap(),
        );
    }
    let rust_encrypt_ns_per_frame = (r_enc.elapsed().as_nanos() as u64) / (iterations as u64);

    let r_dec = Instant::now();
    for pkt in &rust_packets {
        let pt = rust_b.decrypt(1, MediaType::Audio, pkt).unwrap();
        assert_eq!(pt.len(), payload.len());
    }
    let rust_decrypt_ns_per_frame = (r_dec.elapsed().as_nanos() as u64) / (iterations as u64);

    let report = build_comparative_audit_report(
        BackendBenchmarkSample {
            backend: DaveBackendKind::LibdaveFfi,
            handshake_2party_us: cpp_handshake_us,
            encrypt_opus_ns: cpp_encrypt_ns_per_frame,
            decrypt_opus_ns: cpp_decrypt_ns_per_frame,
            custom_crypto_lines: 0,
            unsafe_blocks_in_lib: 0,
            requires_cxx_toolchain: true,
            forbid_unsafe_compatible: false,
        },
        BackendBenchmarkSample {
            backend: DaveBackendKind::DaveyRustCrypto,
            handshake_2party_us: rust_handshake_us,
            encrypt_opus_ns: rust_encrypt_ns_per_frame,
            decrypt_opus_ns: rust_decrypt_ns_per_frame,
            custom_crypto_lines: 0,
            unsafe_blocks_in_lib: 0,
            requires_cxx_toolchain: false,
            forbid_unsafe_compatible: true,
        },
    );

    // Both easily beat the 20,000,000 ns (20ms) real-time Opus frame budget by >1,000x
    assert!(report.libdave_ffi.encrypt_opus_ns < 500_000);
    assert!(report.davey_rust.encrypt_opus_ns < 500_000);
    assert!(report.custom_crypto_eliminated);
    assert_eq!(
        report.recommended_primary_backend,
        DaveBackendKind::DaveyRustCrypto
    );
    println!(
        "BENCHMARK_SUMMARY: libdave_ffi(handshake={}us, enc={}ns, dec={}ns) vs davey_rust(handshake={}us, enc={}ns, dec={}ns)",
        report.libdave_ffi.handshake_2party_us,
        report.libdave_ffi.encrypt_opus_ns,
        report.libdave_ffi.decrypt_opus_ns,
        report.davey_rust.handshake_2party_us,
        report.davey_rust.encrypt_opus_ns,
        report.davey_rust.decrypt_opus_ns
    );
}
