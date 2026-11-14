#![forbid(unsafe_code)]

#[path = "../src/auth/remote_auth.rs"]
mod remote_auth;

use chord_core::rest::routes::HttpMethod;
use remote_auth::{
    PendingUserPreview, QR_URL_PREFIX, REMOTE_AUTH_GATEWAY_URL, REMOTE_AUTH_LOGIN_PATH,
    RemoteAuthCipher, RemoteAuthError, RemoteAuthSession, RemoteAuthState, RemoteAuthToken,
    build_login_exchange_request, compute_nonce_proof, compute_spki_fingerprint,
    decode_base64_standard, decode_base64url_unpadded, decrypt_login_exchange_response,
    encode_base64_standard, encode_base64url_unpadded, sha256,
};

/// Deterministic test cipher simulating RSA-OAEP-SHA256 via a keyed XOR prefix tag
/// so tests can verify both successful decryption and decryption failures deterministically.
struct MockOaepCipher {
    key_tag: u8,
    fail_decrypt: bool,
}

impl MockOaepCipher {
    const fn new(key_tag: u8) -> Self {
        Self {
            key_tag,
            fail_decrypt: false,
        }
    }

    fn encrypt(&self, plaintext: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(plaintext.len() + 1);
        out.push(self.key_tag);
        for &b in plaintext {
            out.push(b ^ self.key_tag);
        }
        out
    }

    fn encrypt_b64(&self, plaintext: &[u8]) -> String {
        encode_base64_standard(&self.encrypt(plaintext))
    }
}

impl RemoteAuthCipher for MockOaepCipher {
    fn decrypt_oaep_sha256(&self, ciphertext: &[u8]) -> Result<Vec<u8>, RemoteAuthError> {
        if self.fail_decrypt {
            return Err(RemoteAuthError::DecryptionFailed(
                "simulated OAEP failure".to_owned(),
            ));
        }
        let Some((&tag, rest)) = ciphertext.split_first() else {
            return Err(RemoteAuthError::DecryptionFailed(
                "empty ciphertext".to_owned(),
            ));
        };
        if tag != self.key_tag {
            return Err(RemoteAuthError::DecryptionFailed(
                "OAEP key tag mismatch".to_owned(),
            ));
        }
        Ok(rest.iter().map(|&b| b ^ self.key_tag).collect())
    }
}

fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        use std::fmt::Write;
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

#[test]
fn t01_sha256_nist_fips_180_4_vectors_and_base64_encodings() {
    // NIST FIPS 180-4 vector 1: empty string ""
    let digest_empty = sha256(b"");
    assert_eq!(
        to_hex(&digest_empty),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );

    // NIST FIPS 180-4 vector 2: "abc"
    let digest_abc = sha256(b"abc");
    assert_eq!(
        to_hex(&digest_abc),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );

    // URL-safe unpadded base64 of SHA-256("abc") must use '-' and '_' and have no '=' padding
    let b64url_abc = encode_base64url_unpadded(&digest_abc);
    assert_eq!(b64url_abc, "ungWv48Bz-pBQUDeXa4iI7ADYaOWF3qctBD_YfIAFa0");
    assert!(!b64url_abc.contains('='));
    assert!(!b64url_abc.contains('+'));
    assert!(!b64url_abc.contains('/'));
    assert_eq!(compute_nonce_proof(b"abc"), b64url_abc);
    assert_eq!(compute_spki_fingerprint(b"abc"), b64url_abc);

    // Roundtrip decode of base64url unpadded
    let decoded_abc = decode_base64url_unpadded(&b64url_abc).expect("valid base64url");
    assert_eq!(decoded_abc, digest_abc);

    // NIST FIPS 180-4 vector 3: 448-bit multi-block message
    let digest_multi = sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq");
    assert_eq!(
        to_hex(&digest_multi),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );

    // Standard base64 roundtrip
    let b64_std = encode_base64_standard(&digest_abc);
    assert_eq!(b64_std, "ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0=");
    assert_eq!(
        decode_base64_standard(&b64_std).expect("valid standard b64"),
        digest_abc
    );
}

#[test]
fn t02_full_happy_path_remote_auth_v2_state_machine_and_token_exchange() {
    assert_eq!(
        REMOTE_AUTH_GATEWAY_URL,
        "wss://remote-auth-gateway.discord.gg/?v=2"
    );
    assert_eq!(
        REMOTE_AUTH_LOGIN_PATH,
        "/api/v9/users/@me/remote-auth/login"
    );
    assert_eq!(QR_URL_PREFIX, "https://discord.com/ra/");

    let spki_der = b"MOCK_SPKI_PUBLIC_KEY_DER_2048_BIT_TEST_VECTOR".to_vec();
    let expected_spki_b64 = encode_base64_standard(&spki_der);
    let expected_fingerprint = compute_spki_fingerprint(&spki_der);

    let cipher = MockOaepCipher::new(0x5A);
    let t0 = 1_000_000_u64;
    let mut session = RemoteAuthSession::new(spki_der, t0).expect("valid session init");
    assert_eq!(session.state(), &RemoteAuthState::Connecting);

    // Step 1: Server -> Client `hello`
    let hello_json = r#"{"op":"hello","timeout_ms":120000,"heartbeat_interval":41250}"#;
    let init_reply = session
        .handle_frame(hello_json, &cipher, t0 + 50)
        .expect("hello handled")
        .expect("must emit init frame");

    assert_eq!(session.state(), &RemoteAuthState::Handshaking);
    assert_eq!(session.heartbeat_interval_ms(), Some(41_250));
    assert_eq!(session.qr_timeout_ms(), Some(120_000));

    let init_val: serde_json::Value = serde_json::from_str(&init_reply).expect("valid init JSON");
    assert_eq!(init_val["op"], "init");
    assert_eq!(init_val["encoded_public_key"], expected_spki_b64);

    // Step 2: Server -> Client `nonce_proof`
    let raw_nonce = b"server-random-nonce-32-bytes-xyz!";
    let encrypted_nonce_b64 = cipher.encrypt_b64(raw_nonce);
    let nonce_frame = serde_json::json!({
        "op": "nonce_proof",
        "encrypted_nonce": encrypted_nonce_b64,
    })
    .to_string();

    let proof_reply = session
        .handle_frame(&nonce_frame, &cipher, t0 + 100)
        .expect("nonce_proof handled")
        .expect("must emit nonce_proof response");

    let proof_val: serde_json::Value =
        serde_json::from_str(&proof_reply).expect("valid nonce_proof JSON");
    assert_eq!(proof_val["op"], "nonce_proof");
    assert_eq!(proof_val["proof"], compute_nonce_proof(raw_nonce));

    // Step 3: Server -> Client `pending_remote_init`
    let remote_init_frame = serde_json::json!({
        "op": "pending_remote_init",
        "fingerprint": expected_fingerprint,
    })
    .to_string();

    let out_remote_init = session
        .handle_frame(&remote_init_frame, &cipher, t0 + 150)
        .expect("pending_remote_init handled");
    assert_eq!(out_remote_init, None);
    assert_eq!(
        session.state(),
        &RemoteAuthState::AwaitingScan {
            qr_url: format!("https://discord.com/ra/{expected_fingerprint}"),
            fingerprint: expected_fingerprint,
            expires_at_ms: t0 + 120_000,
        }
    );

    // Step 4: Server -> Client `pending_ticket` (username contains ':' characters!)
    let user_payload = b"80351110224678912:0:a_f1e2d3c4b5a69788:cyber:ninja:2077";
    let encrypted_user_b64 = cipher.encrypt_b64(user_payload);
    let ticket_frame = serde_json::json!({
        "op": "pending_ticket",
        "encrypted_user_payload": encrypted_user_b64,
    })
    .to_string();

    let out_ticket = session
        .handle_frame(&ticket_frame, &cipher, t0 + 5_000)
        .expect("pending_ticket handled");
    assert_eq!(out_ticket, None);
    assert_eq!(
        session.state(),
        &RemoteAuthState::AwaitingConfirmation(PendingUserPreview {
            user_id: "80351110224678912".to_owned(),
            discriminator: "0".to_owned(),
            avatar_hash: "a_f1e2d3c4b5a69788".to_owned(),
            username: "cyber:ninja:2077".to_owned(),
        })
    );

    // Step 5: Server -> Client `pending_login`
    let login_frame = r#"{"op":"pending_login","ticket":"ra_ticket_live_v2_abcdef123456"}"#;
    let out_login = session
        .handle_frame(login_frame, &cipher, t0 + 8_000)
        .expect("pending_login handled");
    assert_eq!(out_login, None);
    assert_eq!(
        session.state(),
        &RemoteAuthState::TicketReady {
            ticket: "ra_ticket_live_v2_abcdef123456".to_owned(),
        }
    );

    // Build REST v9 exchange HttpRequest
    let req = session
        .build_login_request()
        .expect("build_login_request succeeds");
    assert_eq!(req.method, HttpMethod::Post);
    assert_eq!(req.target, "/api/v9/users/@me/remote-auth/login");
    let req_json: serde_json::Value =
        serde_json::from_slice(&req.body).expect("request body is valid JSON");
    assert_eq!(
        req_json,
        serde_json::json!({"ticket": "ra_ticket_live_v2_abcdef123456"})
    );

    // Exchange encrypted_token response and verify redaction
    let raw_token = "mfa.REMOTE_AUTH_SECRET_TOKEN_NEVER_LEAK_777";
    let encrypted_token_b64 = cipher.encrypt_b64(raw_token.as_bytes());
    let rest_response_json = serde_json::json!({
        "encrypted_token": encrypted_token_b64,
    })
    .to_string();

    let token: RemoteAuthToken = session
        .complete_login(&rest_response_json, &cipher)
        .expect("complete_login decrypts token");
    assert_eq!(token.expose_secret(), raw_token);

    let debug_str = format!("{token:?}");
    let display_str = format!("{token}");
    assert_eq!(debug_str, "RemoteAuthToken([REDACTED])");
    assert_eq!(display_str, "[REDACTED]");
    assert!(!debug_str.contains(raw_token));
    assert!(!display_str.contains(raw_token));
    assert_eq!(session.state(), &RemoteAuthState::Completed);
}

#[test]
fn t03_mitm_tampered_fingerprint_rejected_with_fingerprint_mismatch() {
    let spki_der = b"LEGITIMATE_CLIENT_SPKI_PUBLIC_KEY_DER".to_vec();
    let legitimate_fp = compute_spki_fingerprint(&spki_der);
    let attacker_fp = compute_spki_fingerprint(b"MITM_ATTACKER_SUBSTITUTED_SPKI_KEY");
    assert_ne!(legitimate_fp, attacker_fp);

    let cipher = MockOaepCipher::new(0x33);
    let mut session = RemoteAuthSession::new(spki_der, 0).expect("valid session");

    session
        .handle_frame(
            r#"{"op":"hello","timeout_ms":120000,"heartbeat_interval":41250}"#,
            &cipher,
            10,
        )
        .expect("hello");

    let nonce_frame = serde_json::json!({
        "op": "nonce_proof",
        "encrypted_nonce": cipher.encrypt_b64(b"nonce-bytes-1234"),
    })
    .to_string();
    session
        .handle_frame(&nonce_frame, &cipher, 20)
        .expect("nonce_proof");

    // Server returns attacker's fingerprint instead of client's SPKI SHA-256 base64url
    let mitm_frame = serde_json::json!({
        "op": "pending_remote_init",
        "fingerprint": attacker_fp,
    })
    .to_string();

    let err = session
        .handle_frame(&mitm_frame, &cipher, 30)
        .expect_err("MITM fingerprint must be rejected");

    assert_eq!(
        err,
        RemoteAuthError::FingerprintMismatch {
            expected: legitimate_fp,
            actual: attacker_fp,
        }
    );
    assert_eq!(session.state(), &RemoteAuthState::Failed(err));
}

#[test]
fn t04_cancel_opcode_transitions_to_cancelled() {
    let spki_der = b"SPKI_KEY_FOR_CANCEL_TEST".to_vec();
    let fp = compute_spki_fingerprint(&spki_der);
    let cipher = MockOaepCipher::new(0x11);
    let mut session = RemoteAuthSession::new(spki_der, 500).expect("session");

    session
        .handle_frame(
            r#"{"op":"hello","timeout_ms":120000,"heartbeat_interval":41250}"#,
            &cipher,
            510,
        )
        .expect("hello");
    session
        .handle_frame(
            &serde_json::json!({
                "op": "nonce_proof",
                "encrypted_nonce": cipher.encrypt_b64(b"nonce"),
            })
            .to_string(),
            &cipher,
            520,
        )
        .expect("nonce_proof");
    session
        .handle_frame(
            &serde_json::json!({
                "op": "pending_remote_init",
                "fingerprint": fp,
            })
            .to_string(),
            &cipher,
            530,
        )
        .expect("pending_remote_init");

    let out = session
        .handle_frame(r#"{"op":"cancel"}"#, &cipher, 600)
        .expect("cancel frame handled");
    assert_eq!(out, None);
    assert_eq!(session.state(), &RemoteAuthState::Cancelled);

    // Any frame after cancel is rejected
    let after_cancel = session.handle_frame(
        r#"{"op":"pending_login","ticket":"late_ticket"}"#,
        &cipher,
        700,
    );
    assert!(matches!(
        after_cancel,
        Err(RemoteAuthError::UnexpectedOpcode { .. })
    ));
}

#[test]
fn t05_heartbeat_emission_ack_and_missed_ack_timeout() {
    let spki_der = b"SPKI_KEY_FOR_HEARTBEAT_TEST".to_vec();
    let cipher = MockOaepCipher::new(0x22);
    let t0 = 10_000_u64;
    let mut session = RemoteAuthSession::new(spki_der, t0).expect("session");

    // Before hello, tick emits nothing
    assert_eq!(session.tick(t0 + 500), Ok(None));

    session
        .handle_frame(
            r#"{"op":"hello","timeout_ms":300000,"heartbeat_interval":41250}"#,
            &cipher,
            t0,
        )
        .expect("hello");

    // Before interval (41249 ms elapsed), no heartbeat yet
    assert_eq!(session.tick(t0 + 41_249), Ok(None));

    // At exact interval (41250 ms elapsed), emits heartbeat
    let hb1 = session
        .tick(t0 + 41_250)
        .expect("tick ok")
        .expect("heartbeat emitted");
    let hb1_val: serde_json::Value = serde_json::from_str(&hb1).expect("valid heartbeat JSON");
    assert_eq!(hb1_val, serde_json::json!({"op": "heartbeat"}));

    // Receive heartbeat_ack before next interval
    let ack_out = session
        .handle_frame(r#"{"op":"heartbeat_ack"}"#, &cipher, t0 + 42_000)
        .expect("heartbeat_ack ok");
    assert_eq!(ack_out, None);

    // Next interval (t0 + 82500) emits second heartbeat
    let hb2 = session
        .tick(t0 + 82_500)
        .expect("second tick ok")
        .expect("second heartbeat emitted");
    let hb2_val: serde_json::Value = serde_json::from_str(&hb2).expect("valid heartbeat JSON");
    assert_eq!(hb2_val, serde_json::json!({"op": "heartbeat"}));

    // Do NOT send heartbeat_ack; at next interval (t0 + 123750), tick fails with HeartbeatTimeout
    let err = session
        .tick(t0 + 123_750)
        .expect_err("missed heartbeat_ack must time out");
    assert_eq!(err, RemoteAuthError::HeartbeatTimeout);
    assert_eq!(
        session.state(),
        &RemoteAuthState::Failed(RemoteAuthError::HeartbeatTimeout)
    );
}

#[test]
fn t06_qr_expiration_at_timeout_ms() {
    let spki_der = b"SPKI_KEY_FOR_EXPIRATION_TEST".to_vec();
    let fp = compute_spki_fingerprint(&spki_der);
    let cipher = MockOaepCipher::new(0x44);
    let t0 = 5_000_u64;
    let mut session = RemoteAuthSession::new(spki_der, t0).expect("session");

    session
        .handle_frame(
            r#"{"op":"hello","timeout_ms":120000,"heartbeat_interval":41250}"#,
            &cipher,
            t0,
        )
        .expect("hello");
    session
        .handle_frame(
            &serde_json::json!({
                "op": "nonce_proof",
                "encrypted_nonce": cipher.encrypt_b64(b"nonce"),
            })
            .to_string(),
            &cipher,
            t0 + 10,
        )
        .expect("nonce_proof");
    session
        .handle_frame(
            &serde_json::json!({
                "op": "pending_remote_init",
                "fingerprint": fp,
            })
            .to_string(),
            &cipher,
            t0 + 20,
        )
        .expect("pending_remote_init");

    // 1 ms before expiration (t0 + 119_999) with acked heartbeats -> still valid
    assert!(session.tick(t0 + 41_250).expect("hb1").is_some());
    session
        .handle_frame(r#"{"op":"heartbeat_ack"}"#, &cipher, t0 + 41_300)
        .expect("ack1");
    assert!(session.tick(t0 + 82_500).expect("hb2").is_some());
    session
        .handle_frame(r#"{"op":"heartbeat_ack"}"#, &cipher, t0 + 82_600)
        .expect("ack2");
    assert_eq!(session.tick(t0 + 119_999), Ok(None));

    // At exact expiration timestamp (t0 + 120_000) -> QrExpired
    let err = session
        .tick(t0 + 120_000)
        .expect_err("QR code must expire at started_ms + timeout_ms");
    assert_eq!(err, RemoteAuthError::QrExpired);
    assert_eq!(
        session.state(),
        &RemoteAuthState::Failed(RemoteAuthError::QrExpired)
    );
}

#[test]
fn t07_out_of_order_opcodes_and_malformed_inputs_rejected() {
    let spki_der = b"SPKI_KEY_FOR_NEGATIVE_TESTS".to_vec();
    let cipher = MockOaepCipher::new(0x77);

    // Empty SPKI key rejected on construction
    assert_eq!(
        RemoteAuthSession::new(Vec::new(), 0).unwrap_err(),
        RemoteAuthError::InvalidPublicKey
    );

    // Out-of-order opcodes before `hello`
    let mut session = RemoteAuthSession::new(spki_der.clone(), 0).expect("session");
    assert!(matches!(
        session.handle_frame(r#"{"op":"pending_login","ticket":"t1"}"#, &cipher, 10),
        Err(RemoteAuthError::UnexpectedOpcode { .. })
    ));
    assert!(matches!(
        session.handle_frame(
            r#"{"op":"nonce_proof","encrypted_nonce":"AAAA"}"#,
            &cipher,
            10
        ),
        Err(RemoteAuthError::UnexpectedOpcode { .. })
    ));
    assert!(matches!(
        session.handle_frame(
            r#"{"op":"pending_remote_init","fingerprint":"abc"}"#,
            &cipher,
            10
        ),
        Err(RemoteAuthError::UnexpectedOpcode { .. })
    ));

    // Malformed JSON
    assert!(matches!(
        session.handle_frame("{not-valid-json", &cipher, 10),
        Err(RemoteAuthError::MalformedJson(_))
    ));

    // Zero timeout_ms or heartbeat_interval in hello rejected
    assert_eq!(
        session.handle_frame(
            r#"{"op":"hello","timeout_ms":0,"heartbeat_interval":41250}"#,
            &cipher,
            10
        ),
        Err(RemoteAuthError::InvalidHelloTiming)
    );

    // Advance to Handshaking
    session
        .handle_frame(
            r#"{"op":"hello","timeout_ms":120000,"heartbeat_interval":41250}"#,
            &cipher,
            10,
        )
        .expect("hello");

    // Duplicate hello rejected
    assert!(matches!(
        session.handle_frame(
            r#"{"op":"hello","timeout_ms":120000,"heartbeat_interval":41250}"#,
            &cipher,
            20
        ),
        Err(RemoteAuthError::UnexpectedOpcode { .. })
    ));

    // pending_remote_init before nonce_proof rejected
    let fp = compute_spki_fingerprint(&spki_der);
    assert!(matches!(
        session.handle_frame(
            &serde_json::json!({"op": "pending_remote_init", "fingerprint": fp}).to_string(),
            &cipher,
            20
        ),
        Err(RemoteAuthError::UnexpectedOpcode { .. })
    ));

    // Invalid base64 in nonce_proof rejected
    assert_eq!(
        session.handle_frame(
            r#"{"op":"nonce_proof","encrypted_nonce":"!!!not_base64!!!"}"#,
            &cipher,
            20
        ),
        Err(RemoteAuthError::InvalidBase64)
    );

    // Complete nonce_proof and pending_remote_init
    session
        .handle_frame(
            &serde_json::json!({
                "op": "nonce_proof",
                "encrypted_nonce": cipher.encrypt_b64(b"valid-nonce"),
            })
            .to_string(),
            &cipher,
            30,
        )
        .expect("nonce_proof");
    session
        .handle_frame(
            &serde_json::json!({
                "op": "pending_remote_init",
                "fingerprint": fp,
            })
            .to_string(),
            &cipher,
            40,
        )
        .expect("pending_remote_init");

    // Malformed user payload (only 3 fields instead of 4)
    let bad_user_3_fields = cipher.encrypt_b64(b"80351110224678912:0:only_three_parts");
    assert_eq!(
        session.handle_frame(
            &serde_json::json!({
                "op": "pending_ticket",
                "encrypted_user_payload": bad_user_3_fields,
            })
            .to_string(),
            &cipher,
            50,
        ),
        Err(RemoteAuthError::InvalidUserPayload)
    );

    // Malformed user payload (invalid snowflake 0)
    let bad_user_zero_id = cipher.encrypt_b64(b"0:0:avatar:username");
    assert_eq!(
        session.handle_frame(
            &serde_json::json!({
                "op": "pending_ticket",
                "encrypted_user_payload": bad_user_zero_id,
            })
            .to_string(),
            &cipher,
            55,
        ),
        Err(RemoteAuthError::InvalidUserPayload)
    );

    // Valid pending_ticket followed by pending_login
    session
        .handle_frame(
            &serde_json::json!({
                "op": "pending_ticket",
                "encrypted_user_payload": cipher.encrypt_b64(b"80351110224678912:0::valid_user"),
            })
            .to_string(),
            &cipher,
            60,
        )
        .expect("valid pending_ticket");

    // Empty ticket in pending_login rejected
    assert_eq!(
        session.handle_frame(r#"{"op":"pending_login","ticket":"   "}"#, &cipher, 65),
        Err(RemoteAuthError::InvalidTicket)
    );

    session
        .handle_frame(
            r#"{"op":"pending_login","ticket":"ticket_ok"}"#,
            &cipher,
            70,
        )
        .expect("pending_login");

    // Decrypted token containing newline is rejected and never leaks in error strings
    let bad_secret = "mfa.SECRET_WITH_NEWLINE\nDO_NOT_LEAK";
    let bad_token_resp = serde_json::json!({
        "encrypted_token": cipher.encrypt_b64(bad_secret.as_bytes()),
    })
    .to_string();
    let tok_err = session
        .complete_login(&bad_token_resp, &cipher)
        .expect_err("token with newline must fail validation");
    assert_eq!(tok_err, RemoteAuthError::InvalidToken);
    assert!(!format!("{tok_err:?}").contains("SECRET_WITH_NEWLINE"));
    assert!(!format!("{tok_err}").contains("SECRET_WITH_NEWLINE"));
}

#[test]
fn t08_oaep_decryption_failures_and_error_display_coverage() {
    let spki_der = b"SPKI_KEY_FOR_OAEP_AND_COVERAGE".to_vec();
    let fp = compute_spki_fingerprint(&spki_der);
    let cipher = MockOaepCipher::new(0x55);
    let mut failing_cipher = MockOaepCipher::new(0x55);
    failing_cipher.fail_decrypt = true;

    let mut session = RemoteAuthSession::new(spki_der, 100).expect("session");
    assert_eq!(session.expected_fingerprint(), fp);

    // Calling build_login_request or complete_login before TicketReady fails
    assert!(matches!(
        session.build_login_request(),
        Err(RemoteAuthError::UnexpectedOpcode { .. })
    ));
    assert!(matches!(
        session.complete_login(r#"{"encrypted_token":"AAAA"}"#, &cipher),
        Err(RemoteAuthError::UnexpectedOpcode { .. })
    ));
    assert_eq!(
        build_login_exchange_request("   "),
        Err(RemoteAuthError::InvalidTicket)
    );

    // Advance to Handshaking
    session
        .handle_frame(
            r#"{"op":"hello","timeout_ms":120000,"heartbeat_interval":41250}"#,
            &cipher,
            100,
        )
        .expect("hello");

    // Unknown opcode rejected
    assert_eq!(
        session.handle_frame(r#"{"op":"unknown_future_op"}"#, &cipher, 110),
        Err(RemoteAuthError::UnknownOpcode(
            "unknown_future_op".to_owned()
        ))
    );

    // OAEP decryption failure on nonce_proof
    let valid_nonce_frame = serde_json::json!({
        "op": "nonce_proof",
        "encrypted_nonce": cipher.encrypt_b64(b"nonce-1"),
    })
    .to_string();
    assert!(matches!(
        session.handle_frame(&valid_nonce_frame, &failing_cipher, 120),
        Err(RemoteAuthError::DecryptionFailed(_))
    ));

    // Empty decrypted nonce rejected with InvalidNonce
    let empty_nonce_frame = serde_json::json!({
        "op": "nonce_proof",
        "encrypted_nonce": cipher.encrypt_b64(b""),
    })
    .to_string();
    assert_eq!(
        session.handle_frame(&empty_nonce_frame, &cipher, 125),
        Err(RemoteAuthError::InvalidNonce)
    );

    // Base64url invalid inputs rejected
    for bad_b64url in ["", "   ", "abc=", "abc+", "abc/", "a", "abcde"] {
        assert_eq!(
            decode_base64url_unpadded(bad_b64url),
            Err(RemoteAuthError::InvalidBase64),
            "expected InvalidBase64 for {bad_b64url:?}"
        );
    }
    assert_eq!(
        decode_base64_standard("   "),
        Err(RemoteAuthError::InvalidBase64)
    );

    // Standalone decrypt_login_exchange_response failure with failing cipher
    let resp_json = serde_json::json!({
        "encrypted_token": cipher.encrypt_b64(b"mfa.valid_token"),
    })
    .to_string();
    assert!(matches!(
        decrypt_login_exchange_response(&resp_json, &failing_cipher),
        Err(RemoteAuthError::DecryptionFailed(_))
    ));

    // Verify all RemoteAuthError variants implement std::error::Error and non-empty Display
    let all_errors = [
        RemoteAuthError::InvalidPublicKey,
        RemoteAuthError::MalformedJson("eof".to_owned()),
        RemoteAuthError::InvalidBase64,
        RemoteAuthError::DecryptionFailed("bad padding".to_owned()),
        RemoteAuthError::InvalidNonce,
        RemoteAuthError::FingerprintMismatch {
            expected: "exp".to_owned(),
            actual: "act".to_owned(),
        },
        RemoteAuthError::InvalidUserPayload,
        RemoteAuthError::InvalidTicket,
        RemoteAuthError::InvalidToken,
        RemoteAuthError::UnexpectedOpcode {
            op: "pending_login".to_owned(),
            state: "Connecting",
        },
        RemoteAuthError::UnknownOpcode("foo".to_owned()),
        RemoteAuthError::InvalidHelloTiming,
        RemoteAuthError::HeartbeatTimeout,
        RemoteAuthError::QrExpired,
    ];
    for err in &all_errors {
        let _: &dyn std::error::Error = err;
        assert!(!err.to_string().is_empty());
    }
}
