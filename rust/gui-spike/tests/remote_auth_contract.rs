//! RW-40 / TASK-629 (`G1 Backend`): контрактные тесты модуля Remote Auth v2
//! (`rust/gui-spike/src/remote_auth.rs` и Tauri IPC-команды в `rust/gui-spike/src/main.rs`).
//!
//! Оракулы (`/home/sanya/re-deep/` и `waves/RW-39/NET/`):
//! - `/home/sanya/re-deep/decoded/f0cac74b6ccdca5e_0` (`QR_CHUNK` 52423):
//!   - `generateRsaKeyPair`: RSA-OAEP-2048 (`modulusLength: 2048`, `publicExponent: [1, 0, 1]` = 65537,
//!     `hash: "SHA-256"`), экспорт публичного ключа в ASN.1 DER `spki` (294 байта) и Base64;
//!   - `publicKeyFingerprint`: `base64url_nopad(SHA-256(spki_der))`;
//!   - `decryptNonce`: RSA-OAEP-SHA256 расшифровка `encrypted_nonce` (Base64) и кодирование
//!     сырых байтов nonce в `base64url_nopad` (`O(await P(e, R(t)))`);
//!   - `decryptEncodedCiphertext`: RSA-OAEP-SHA256 расшифровка Base64-шифротекста в UTF-8;
//!   - `D(e, t)`: разбор расшифрованного `encrypted_user_payload` по регулярному выражению
//!     `^(\d+):(\d{1,4}):([a-zA-Z0-9_]+):(.*)$` (`avatar == "0"` -> `None`);
//!   - WebSocket-протокол `/?v=2`:
//!     - исходящие кадры: `init {encoded_public_key}`, `nonce_proof {nonce}`, `heartbeat`;
//!     - входящие кадры: `hello {timeout_ms, heartbeat_interval}`, `nonce_proof {encrypted_nonce}`,
//!       `pending_remote_init {fingerprint}`, `pending_ticket {encrypted_user_payload}`,
//!       `pending_login {ticket}`, `cancel`, `heartbeat_ack`;
//!     - конечный автомат `RemoteAuthStep`: `Initializing (0)` -> `PendingRemoteInit (1)` ->
//!       `PendingTicket (2)` -> `PendingLogin (3)` -> `Finish (4)`;
//!     - экспоненциальный backoff переподключения `new Backoff(1500, 30000)` и закрытие с кодом `1000`;
//! - `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0` (`@275654..@275802`, `@11800708`):
//!   - `REMOTE_AUTH_INITIALIZE = "/users/@me/remote-auth"`,
//!   - `REMOTE_AUTH_CANCEL = "/users/@me/remote-auth/cancel"`,
//!   - `REMOTE_AUTH_LOGIN = "/users/@me/remote-auth/login"`,
//!   - `REMOTE_AUTH_FINISH = "/users/@me/remote-auth/finish"`,
//!   - `REMOTE_AUTH = "/ra/:remoteAuthFingerprint"`.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use discord_core::rest::transport::HttpResponse;
pub use discord_gui_spike::rest_failure;

#[path = "../src/remote_auth.rs"]
pub mod remote_auth;

use remote_auth::{
    compute_backoff_delay_ms, compute_initial_heartbeat_delay_ms, compute_spki_fingerprint,
    decode_base64_flexible, decrypt_and_prove_nonce, encode_base64_standard,
    encode_base64url_nopad, exchange_remote_auth_ticket, format_remote_auth_qr_url,
    parse_nonce_proof_frame, parse_remote_auth_server_frame, parse_remote_auth_user_payload,
    remote_auth_cancel, remote_auth_finish, remote_auth_initialize, resolve_remote_auth_ws_url,
    run_remote_auth_ws_handshake, sha256_bytes, DiscordRemoteAuthRest, RemoteAuthClientFrame,
    RemoteAuthError, RemoteAuthHandshakeOutcome, RemoteAuthRestTransport, RemoteAuthServerFrame,
    RemoteAuthSession, RemoteAuthSessionAction, RemoteAuthStep, RemoteAuthUser,
    RemoteAuthWsTransport, Rsa2048KeyPair, Rsa2048PublicKey, REMOTE_AUTH_BACKOFF_MAX_MS,
    REMOTE_AUTH_BACKOFF_MIN_MS, REMOTE_AUTH_CANCEL_PATH, REMOTE_AUTH_FINISH_PATH,
    REMOTE_AUTH_GATEWAY_ENDPOINT, REMOTE_AUTH_GATEWAY_URL, REMOTE_AUTH_INITIALIZE_PATH,
    REMOTE_AUTH_IPC_COMMANDS, REMOTE_AUTH_LOGIN_PATH, REMOTE_AUTH_PROTOCOL_VERSION,
    REMOTE_AUTH_QR_URL_PREFIX, REMOTE_AUTH_ROUTE_PATTERN, REMOTE_AUTH_WS_ENDPOINT,
    REMOTE_AUTH_WS_NORMAL_CLOSE_CODE, REMOTE_AUTH_WS_URL, RSA_MODULUS_BITS, RSA_MODULUS_BYTES,
    RSA_OAEP_HASH_LEN, RSA_OAEP_MAX_PAYLOAD_BYTES, RSA_PUBLIC_EXPONENT, RSA_SPKI_DER_LEN,
};

// Независимый оракул WebCrypto (`crypto.subtle` RSA-OAEP-2048 SHA-256):
const ORACLE_P0_HEX: &str = "f183bee4207e92c932ccc91f6cdbcc30bb4fcffb2ef07cecc13d9d1e5b08b45a0dbabc7ff93158f780721713646f3b09dbe6749b2a2c1cad76682745451bcc5139ad4584116ff8721bf30e527cdd6531bd1b031fac0941867b2d4f5f6e3e5ab41b81aced5134e4d5af18bf8c4f9801f13df9a67c89b16a1d036865bafd52f559";
const ORACLE_P1_HEX: &str = "e1434fa659c35569589c38099101e77c45043a7e2f76015d7ee4144ee31835aab658230c3ef42cb1dc3c9be3f5f3d25dc0c7ffd664ce8b54460fc68a9b3b2b89fa16d534fd0202053209b60f126db962f063b7f04996d3d45e07462be74804da2ddfe2de814f8a45965c88077b382c1b622eda8a396f7ecdca4cb4964193456d";
const ORACLE_SPKI_B64: &str = "MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA1IRLZg5JA2M5wgQTF7QHACiTBsswZNavW39EEDHQmDIaVIRhHTEV4/2ZJOc6iUXaaGLS53BfgV5kVq/YTfPi7W3lEVR9tASqOK8yJgGzRW/5UOGRXpauTImWMU2DaEVvqn5B5nqq269t9jh+vWkfDD+V/4tlGtUZGkrO2fAlay/4LrBjs4IXjdFccO7dyJTo3ej6t1C5mWwOvux0weK9yrqVKNuf9dVEnZinEEBN65uhSeWsLRWAk/zv08Y+iNT5TzooJsO7i4Eimz88Xg4n11V9gX7hHbWP2b4JR+/YF1U46wwzLBF7XOejQ2fU+U7L7k1inMeLfTlPGpP5s45z5QIDAQAB";
const ORACLE_FP: &str = "0uv6U28HWAbeOqpRLBRM5dXoiwNX7Qb16u7OpPLeREQ";
const ORACLE_ENC_NONCE: &str = "gPKYtFQZUa+Zl4LT9L0ABlCjKtLFv9Rvw/zeOMtdN732DqZm6Q21ImLM9TLNBhjMcd+vheywl9GX6SlAd6MbHbZjXi39fiJuH7gGCL8CDXqLjC4GrXf2fe2JE8lzzThREtGZEij1GGuD2V7TabRNLY4LQfFDEQrzCUuFpyg2Lec1B5Xrvp/ry90A/pGeVwryYLTa3lv7x7ppESet92ldk+HkLhTlkJXdWEqwF4DRcW3EOHDq4sMpnUvhvu72Kv3AFf6JVuTSJHV7/cXaRZvCls9wBd3HGK0ysxhKeG+ipFnnvVFpOZIBlzLm6y6AIWYukTS55pJHiSYfdoTP1wG3bQ==";
const ORACLE_NONCE_PROOF: &str = "ZGlzY29yZC1yZW1vdGUtYXV0aC12Mi1ub25jZS0zMmI";
const ORACLE_ENC_USER: &str = "DVSaUnpHZbfHebeKRJduBO1TlxLpz1umCw1Z2CU3PjJQ9A9QRoIYZ6cmSJGJ7KYL/IYNa7N9YJtwk0/YNUeNCIfeMT4u3qpTP/j8sMXy0d4QPa5iUiQlUUNipSliwTw7O99YZTBMd3m83On+emfoKAAj2vBWJShzgjNudSRmO75W2sDb6fy6IlA3GwwyqTy701NBMZNJbema+k93m4pC128AbdVU2tb+CmbkYlTC6c/7Cxzl3u0ICyfrqeAecctys0ol4U2izVjQFJYqOvVy7NKvt2yStiCM0uJqdGk1oV6QDvJKaE1kMKxovG5fcUn9x7EpjOhYzJ3Yc/4fu/xZYw==";
const ORACLE_ENC_TOKEN: &str = "vP8BC1XIw3azPqThsJ4WyTLDAshi4IiDTfTRbfnHkz6jv+i7WKtvokAOdjUqIwo30+X1OQag72COjBcGetVdDsuTJXiUX+7hh3TWhyh7y9T/Q5adZVfmSYEWFFEdwoGjKa8zVtN8pZggGXb545uo1R6JFpPLj3y803RM+OrzPTctA+TJMhnC+mSi0iK3uppzkexKGfb+Y1fdYwGErZNHLrFtzluO99VjoCI9/ilfG13f9W3CblW9oFap1kU+MInobEVr5TBfJGJmnWbSgGx0Gq3wtAID6nOGJz+99DQKMaMLQ7NtxzWr6WEb+5ZE12f6XIyq00rVrNXndo/eMJ9OyA==";

fn hex_128(hex: &str) -> [u8; 128] {
    assert_eq!(hex.len(), 256);
    let mut out = [0u8; 128];
    for i in 0..128 {
        out[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).expect("valid hex byte");
    }
    out
}

fn oracle_keypair() -> Rsa2048KeyPair {
    Rsa2048KeyPair::from_primes(&hex_128(ORACLE_P0_HEX), &hex_128(ORACLE_P1_HEX))
        .expect("oracle RSA-2048 keypair")
}

fn run<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("tokio runtime")
        .block_on(future)
}

#[test]
fn t01_constants_and_webcrypto_oracle_spki_fingerprint_and_oaep_decryption() {
    assert_eq!(REMOTE_AUTH_WS_ENDPOINT, "wss://remote-auth.discord.gg");
    assert_eq!(REMOTE_AUTH_WS_URL, "wss://remote-auth.discord.gg/?v=2");
    assert_eq!(
        REMOTE_AUTH_GATEWAY_ENDPOINT,
        "wss://remote-auth-gateway.discord.gg"
    );
    assert_eq!(
        REMOTE_AUTH_GATEWAY_URL,
        "wss://remote-auth-gateway.discord.gg/?v=2"
    );
    assert_eq!(REMOTE_AUTH_PROTOCOL_VERSION, 2);
    assert_eq!(REMOTE_AUTH_QR_URL_PREFIX, "https://discord.com/ra/");
    assert_eq!(REMOTE_AUTH_ROUTE_PATTERN, "/ra/:remoteAuthFingerprint");
    assert_eq!(REMOTE_AUTH_INITIALIZE_PATH, "/users/@me/remote-auth");
    assert_eq!(REMOTE_AUTH_CANCEL_PATH, "/users/@me/remote-auth/cancel");
    assert_eq!(REMOTE_AUTH_LOGIN_PATH, "/users/@me/remote-auth/login");
    assert_eq!(REMOTE_AUTH_FINISH_PATH, "/users/@me/remote-auth/finish");
    assert_eq!(REMOTE_AUTH_BACKOFF_MIN_MS, 1_500);
    assert_eq!(REMOTE_AUTH_BACKOFF_MAX_MS, 30_000);
    assert_eq!(REMOTE_AUTH_WS_NORMAL_CLOSE_CODE, 1_000);
    assert_eq!(RSA_MODULUS_BITS, 2_048);
    assert_eq!(RSA_MODULUS_BYTES, 256);
    assert_eq!(RSA_PUBLIC_EXPONENT, 65_537);
    assert_eq!(RSA_SPKI_DER_LEN, 294);
    assert_eq!(RSA_OAEP_HASH_LEN, 32);
    assert_eq!(RSA_OAEP_MAX_PAYLOAD_BYTES, 190);
    assert_eq!(REMOTE_AUTH_IPC_COMMANDS.len(), 7);

    let kp = oracle_keypair();
    let pub_key = kp.public_key();
    let spki_der = pub_key.to_spki_der();
    assert_eq!(spki_der.len(), RSA_SPKI_DER_LEN);
    assert_eq!(pub_key.serialize_public_key(), ORACLE_SPKI_B64);
    assert_eq!(pub_key.fingerprint(), ORACLE_FP);
    assert_eq!(compute_spki_fingerprint(&spki_der), ORACLE_FP);
    assert_eq!(
        pub_key.qr_url(),
        format!("https://discord.com/ra/{ORACLE_FP}")
    );

    // Расшифровка реальных шифротекстов из WebCrypto (`window.crypto.subtle.encrypt`):
    let nonce_proof = kp
        .decrypt_nonce(ORACLE_ENC_NONCE)
        .expect("decrypt WebCrypto nonce");
    assert_eq!(nonce_proof, ORACLE_NONCE_PROOF);

    let user = kp
        .decrypt_user_payload(ORACLE_ENC_USER)
        .expect("decrypt WebCrypto user payload");
    assert_eq!(
        user,
        RemoteAuthUser {
            id: "1301880740245405707".to_string(),
            discriminator: "0".to_string(),
            avatar: Some("a_1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d".to_string()),
            username: "sanya_discord".to_string(),
        }
    );

    let token = kp
        .decrypt_encoded_ciphertext(ORACLE_ENC_TOKEN)
        .expect("decrypt WebCrypto token");
    assert_eq!(
        token,
        "MTMwMTg4MDc0MDI0NTQwNTcwNw.Gh7K9x.test_remote_auth_decrypted_token_v2"
    );
}

#[test]
fn t02_rsa_2048_generation_spki_roundtrip_and_oaep_boundaries() {
    let kp1 = Rsa2048KeyPair::generate().expect("generate kp1");
    let kp2 = Rsa2048KeyPair::generate().expect("generate kp2");
    assert_ne!(
        kp1.public_key().fingerprint(),
        kp2.public_key().fingerprint(),
        "каждый вызов generate() обязан выдавать уникальную пару ключей RSA-2048"
    );
    assert_eq!(kp1.public_key().modulus_bytes().len(), 256);
    assert_ne!(kp1.public_key().modulus_bytes()[0] & 0x80, 0);
    assert_eq!(kp1.public_key().modulus_bytes()[255] & 1, 1);
    assert_eq!(kp1.public_key().exponent(), 65_537);

    // SPKI DER & Base64 roundtrip:
    let b64 = kp1.public_key().serialize_public_key();
    let parsed = Rsa2048PublicKey::from_encoded_public_key(&b64).expect("parse encoded public key");
    assert_eq!(&parsed, kp1.public_key());

    // Невалидный SPKI DER отвергается:
    let mut bad_der = kp1.public_key().to_spki_der();
    bad_der[8] ^= 0xff; // повреждён OID rsaEncryption
    assert!(matches!(
        Rsa2048PublicKey::from_spki_der(&bad_der),
        Err(RemoteAuthError::InvalidPublicKey { .. })
    ));
    assert!(matches!(
        Rsa2048PublicKey::from_spki_der(&bad_der[..100]),
        Err(RemoteAuthError::InvalidPublicKey { .. })
    ));

    // Границы длины открытого текста RSA-OAEP-SHA256: 1 байт, 32 байта и ровно 190 байт:
    let seed = sha256_bytes(b"deterministic-oaep-seed-for-contract-test");
    for len in [1usize, 32, RSA_OAEP_MAX_PAYLOAD_BYTES] {
        let payload: Vec<u8> = (0..len).map(|i| (i as u8).wrapping_mul(37)).collect();
        let ct = kp1
            .public_key()
            .encrypt_oaep_sha256(&payload, &seed)
            .expect("encrypt OAEP");
        assert_eq!(ct.len(), RSA_MODULUS_BYTES);
        let pt = kp1.decrypt_oaep_sha256(&ct).expect("decrypt OAEP");
        assert_eq!(pt, payload, "OAEP roundtrip mismatch for len={len}");
    }

    // 191 байт (> 190) — превышение лимита RFC 8017 §7.1.1:
    let oversized = vec![0x42u8; RSA_OAEP_MAX_PAYLOAD_BYTES + 1];
    assert_eq!(
        kp1.public_key().encrypt_oaep_sha256(&oversized, &seed),
        Err(RemoteAuthError::PayloadTooLarge {
            len: 191,
            max: RSA_OAEP_MAX_PAYLOAD_BYTES,
        })
    );

    // Модификация шифротекста или неверная длина обязаны отвергаться:
    let mut ct = kp1
        .public_key()
        .encrypt_oaep_sha256(b"integrity-check", &seed)
        .expect("encrypt");
    ct[120] ^= 0x01;
    assert!(matches!(
        kp1.decrypt_oaep_sha256(&ct),
        Err(RemoteAuthError::Crypto { .. })
    ));
    assert!(matches!(
        kp1.decrypt_oaep_sha256(&ct[..255]),
        Err(RemoteAuthError::Crypto { .. })
    ));
}

#[test]
fn t03_nonce_parsing_and_nonce_proof_computation() {
    let kp = oracle_keypair();
    let frame_json = serde_json::json!({
        "op": "nonce_proof",
        "encrypted_nonce": ORACLE_ENC_NONCE,
    })
    .to_string();

    let parsed = parse_nonce_proof_frame(&frame_json).expect("parse nonce_proof frame");
    assert_eq!(parsed.ciphertext.len(), RSA_MODULUS_BYTES);
    assert_eq!(parsed.encrypted_nonce_b64, ORACLE_ENC_NONCE);

    let proof_from_frame =
        decrypt_and_prove_nonce(&kp, &frame_json).expect("prove from JSON frame");
    assert_eq!(proof_from_frame.nonce, ORACLE_NONCE_PROOF);
    assert_eq!(
        proof_from_frame.decrypted_bytes,
        b"discord-remote-auth-v2-nonce-32b"
    );
    assert_eq!(
        proof_from_frame.sha256_proof,
        encode_base64url_nopad(&sha256_bytes(b"discord-remote-auth-v2-nonce-32b"))
    );
    assert_eq!(
        proof_from_frame.outbound_frame_json(),
        r#"{"op":"nonce_proof","nonce":"ZGlzY29yZC1yZW1vdGUtYXV0aC12Mi1ub25jZS0zMmI"}"#
    );

    // Прямая передача Base64 `encrypted_nonce` без JSON-обёртки также поддерживается:
    let proof_from_b64 =
        decrypt_and_prove_nonce(&kp, ORACLE_ENC_NONCE).expect("prove from raw Base64");
    assert_eq!(proof_from_b64.nonce, ORACLE_NONCE_PROOF);

    // Негативные сценарии парсинга nonce:
    for bad in [
        r#"{"op":"hello","encrypted_nonce":"AAAA"}"#,
        r#"{"op":"nonce_proof","encrypted_nonce":""}"#,
        r#"{"op":"nonce_proof","encrypted_nonce":"not-valid-b64!!!"}"#,
        r#"{"op":"nonce_proof","encrypted_nonce":"YWJj"}"#,
        "not-json-and-short-b64",
    ] {
        assert!(
            parse_nonce_proof_frame(bad).is_err(),
            "ожидался отказ на некорректном кадре nonce: {bad}"
        );
    }

    // Расшифровка пустого nonce запрещена:
    let empty_seed = sha256_bytes(b"empty-nonce-seed");
    let empty_ct_b64 = kp
        .public_key()
        .encrypt_oaep_sha256_base64(b"", &empty_seed)
        .expect("encrypt empty");
    assert!(matches!(
        decrypt_and_prove_nonce(&kp, &empty_ct_b64),
        Err(RemoteAuthError::InvalidNonce { .. })
    ));
}

#[test]
fn t04_ws_url_resolution_and_protocol_version_validation() {
    assert_eq!(
        resolve_remote_auth_ws_url(None).expect("default url"),
        "wss://remote-auth.discord.gg/?v=2"
    );
    assert_eq!(
        resolve_remote_auth_ws_url(Some("wss://remote-auth.discord.gg")).expect("bare host"),
        "wss://remote-auth.discord.gg/?v=2"
    );
    assert_eq!(
        resolve_remote_auth_ws_url(Some("wss://remote-auth.discord.gg/")).expect("trailing slash"),
        "wss://remote-auth.discord.gg/?v=2"
    );
    assert_eq!(
        resolve_remote_auth_ws_url(Some("//remote-auth-gateway.discord.gg"))
            .expect("protocol-relative"),
        "wss://remote-auth-gateway.discord.gg/?v=2"
    );
    assert_eq!(
        resolve_remote_auth_ws_url(Some("wss://remote-auth-gateway.discord.gg/?v=2"))
            .expect("explicit v2"),
        "wss://remote-auth-gateway.discord.gg/?v=2"
    );

    // Небезопасные схемы, чужие хосты и неподдерживаемые версии протокола отвергаются:
    assert!(matches!(
        resolve_remote_auth_ws_url(Some("ws://remote-auth.discord.gg/?v=2")),
        Err(RemoteAuthError::InvalidEndpoint { .. })
    ));
    assert!(matches!(
        resolve_remote_auth_ws_url(Some("https://remote-auth.discord.gg/?v=2")),
        Err(RemoteAuthError::InvalidEndpoint { .. })
    ));
    assert!(matches!(
        resolve_remote_auth_ws_url(Some("wss://attacker.example.com/?v=2")),
        Err(RemoteAuthError::InvalidEndpoint { .. })
    ));
    assert!(matches!(
        resolve_remote_auth_ws_url(Some("wss://remote-auth.discord.gg/?v=1")),
        Err(RemoteAuthError::UnsupportedProtocolVersion { version: 1 })
    ));
    assert!(matches!(
        resolve_remote_auth_ws_url(Some("wss://remote-auth.discord.gg/?v=3")),
        Err(RemoteAuthError::UnsupportedProtocolVersion { version: 3 })
    ));
}

#[test]
fn t05_session_state_machine_and_user_payload_parsing() {
    let kp = oracle_keypair();
    let mut session =
        RemoteAuthSession::with_keypair(kp, None).expect("session with oracle keypair");
    assert_eq!(session.step(), RemoteAuthStep::Initializing);
    assert_eq!(session.ws_url(), "wss://remote-auth.discord.gg/?v=2");
    assert_eq!(session.fingerprint(), ORACLE_FP);

    // 1. Открытие WS -> отправка `init`:
    let init_frame = session.on_open();
    assert_eq!(
        init_frame,
        RemoteAuthClientFrame::Init {
            encoded_public_key: ORACLE_SPKI_B64.to_string(),
        }
    );

    // 2. Входящий `hello`:
    let hello_action = session
        .handle_server_frame(r#"{"op":"hello","timeout_ms":120000,"heartbeat_interval":41250}"#)
        .expect("handle hello");
    assert_eq!(
        hello_action,
        RemoteAuthSessionAction::Hello {
            timeout_ms: 120_000,
            heartbeat_interval_ms: 41_250,
            outbound: None,
        }
    );
    assert_eq!(session.heartbeat_interval_ms(), Some(41_250));
    assert_eq!(session.timeout_ms(), Some(120_000));
    assert_eq!(compute_initial_heartbeat_delay_ms(41_250, 500), 20_625);

    // 3. Входящий `nonce_proof`:
    let nonce_frame = serde_json::json!({
        "op": "nonce_proof",
        "encrypted_nonce": ORACLE_ENC_NONCE,
    })
    .to_string();
    let nonce_action = session
        .handle_server_frame(&nonce_frame)
        .expect("handle nonce_proof");
    assert_eq!(
        nonce_action,
        RemoteAuthSessionAction::SendNonceProof {
            nonce: ORACLE_NONCE_PROOF.to_string(),
            frame: RemoteAuthClientFrame::NonceProof {
                nonce: ORACLE_NONCE_PROOF.to_string(),
            },
        }
    );

    // 4. Входящий `pending_remote_init`:
    let remote_init_frame = serde_json::json!({
        "op": "pending_remote_init",
        "fingerprint": ORACLE_FP,
    })
    .to_string();
    let init_action = session
        .handle_server_frame(&remote_init_frame)
        .expect("handle pending_remote_init");
    assert_eq!(session.step(), RemoteAuthStep::PendingRemoteInit);
    assert_eq!(
        init_action,
        RemoteAuthSessionAction::ShowQr {
            fingerprint: ORACLE_FP.to_string(),
            qr_url: format!("https://discord.com/ra/{ORACLE_FP}"),
        }
    );

    // 5. Heartbeat + HeartbeatAck:
    let hb_frame = session.on_heartbeat_tick().expect("first heartbeat tick");
    assert_eq!(hb_frame, RemoteAuthClientFrame::Heartbeat);
    assert!(!session.heartbeat_acked());
    let ack_action = session
        .handle_server_frame(r#"{"op":"heartbeat_ack"}"#)
        .expect("handle heartbeat_ack");
    assert_eq!(ack_action, RemoteAuthSessionAction::HeartbeatAcked);
    assert!(session.heartbeat_acked());

    // 6. Входящий `pending_ticket` (сканирование QR телефоном):
    let ticket_frame = serde_json::json!({
        "op": "pending_ticket",
        "encrypted_user_payload": ORACLE_ENC_USER,
    })
    .to_string();
    let ticket_action = session
        .handle_server_frame(&ticket_frame)
        .expect("handle pending_ticket");
    assert_eq!(session.step(), RemoteAuthStep::PendingTicket);
    let expected_user = RemoteAuthUser {
        id: "1301880740245405707".to_string(),
        discriminator: "0".to_string(),
        avatar: Some("a_1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d".to_string()),
        username: "sanya_discord".to_string(),
    };
    assert_eq!(
        ticket_action,
        RemoteAuthSessionAction::AwaitingTicketConfirmation {
            user: expected_user.clone()
        }
    );
    assert_eq!(session.user(), Some(&expected_user));

    // 7. Входящий `pending_login` (подтверждение на телефоне):
    let login_action = session
        .handle_server_frame(r#"{"op":"pending_login","ticket":"ra-ticket-998877"}"#)
        .expect("handle pending_login");
    assert_eq!(session.step(), RemoteAuthStep::PendingLogin);
    assert_eq!(
        login_action,
        RemoteAuthSessionAction::ExchangeTicket {
            ticket: "ra-ticket-998877".to_string(),
            endpoint: REMOTE_AUTH_LOGIN_PATH.to_string(),
        }
    );

    // Проверка `avatar == "0"` -> `None` и двоеточий внутри `username`:
    let no_avatar = parse_remote_auth_user_payload("942355007271997450:0001:0:User:With:Colons")
        .expect("parse avatar=0");
    assert_eq!(no_avatar.avatar, None);
    assert_eq!(no_avatar.username, "User:With:Colons");
}

#[test]
fn t06_security_fingerprint_mismatch_heartbeat_timeout_cancel_and_backoff() {
    let mut session =
        RemoteAuthSession::with_keypair(oracle_keypair(), None).expect("oracle session");
    let _ = session.on_open();
    session
        .handle_server_frame(r#"{"op":"hello","timeout_ms":120000,"heartbeat_interval":41250}"#)
        .expect("hello");
    let nonce_frame = serde_json::json!({
        "op": "nonce_proof",
        "encrypted_nonce": ORACLE_ENC_NONCE,
    })
    .to_string();
    session
        .handle_server_frame(&nonce_frame)
        .expect("nonce_proof");

    // Подмена fingerprint сервером (MITM) обязана немедленно прерывать рукопожатие:
    let mitm_init = r#"{"op":"pending_remote_init","fingerprint":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}"#;
    let err = session
        .handle_server_frame(mitm_init)
        .expect_err("fingerprint mismatch must fail");
    assert_eq!(
        err,
        RemoteAuthError::FingerprintMismatch {
            expected: ORACLE_FP.to_string(),
            actual: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
        }
    );

    // Heartbeat timeout при отсутствии `heartbeat_ack` (`E == false` в `QR_CHUNK` 52423):
    let _ = session.on_heartbeat_tick().expect("first heartbeat");
    let hb_err = session
        .on_heartbeat_tick()
        .expect_err("second heartbeat without ack must fail");
    assert_eq!(hb_err, RemoteAuthError::HeartbeatTimeout);
    assert_eq!(session.backoff_attempt(), 1);

    // Проверка экспоненциального backoff в границах `1500..=30000` мс:
    assert_eq!(compute_backoff_delay_ms(0), 1_500);
    assert_eq!(compute_backoff_delay_ms(1), 3_000);
    assert_eq!(compute_backoff_delay_ms(2), 6_000);
    assert_eq!(compute_backoff_delay_ms(3), 12_000);
    assert_eq!(compute_backoff_delay_ms(4), 24_000);
    assert_eq!(compute_backoff_delay_ms(5), 30_000);
    assert_eq!(compute_backoff_delay_ms(50), 30_000);

    // Кадр `cancel` сбрасывает состояние в `Initializing` и ротирует пару ключей RSA-2048:
    let cancel_action = session
        .handle_server_frame(r#"{"op":"cancel"}"#)
        .expect("handle cancel");
    assert_eq!(session.step(), RemoteAuthStep::Initializing);
    assert_ne!(
        session.fingerprint(),
        ORACLE_FP,
        "после cancel должен быть сгенерирован новый ключ RSA-2048"
    );
    assert!(matches!(
        cancel_action,
        RemoteAuthSessionAction::Cancelled { .. }
    ));

    // Неизвестный опкод возвращает типизированную ошибку:
    let unknown_err = session
        .handle_server_frame(r#"{"op":"unknown_future_op"}"#)
        .expect_err("unknown op");
    assert_eq!(
        unknown_err,
        RemoteAuthError::UnsupportedOpcode {
            op: "unknown_future_op".to_string()
        }
    );
}

#[derive(Default)]
struct FakeWsTransport {
    inbound: VecDeque<String>,
    sent: Vec<String>,
    closed_with: Option<u16>,
}

impl FakeWsTransport {
    fn with_frames(frames: Vec<String>) -> Self {
        Self {
            inbound: VecDeque::from(frames),
            sent: Vec::new(),
            closed_with: None,
        }
    }
}

impl RemoteAuthWsTransport for FakeWsTransport {
    fn send<'a>(
        &'a mut self,
        frame: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), RemoteAuthError>> + Send + 'a>> {
        self.sent.push(frame.to_string());
        Box::pin(async { Ok(()) })
    }

    fn recv<'a>(
        &'a mut self,
    ) -> Pin<Box<dyn Future<Output = Result<Option<String>, RemoteAuthError>> + Send + 'a>> {
        let next = self.inbound.pop_front();
        Box::pin(async move { Ok(next) })
    }

    fn close<'a>(
        &'a mut self,
        code: u16,
    ) -> Pin<Box<dyn Future<Output = Result<(), RemoteAuthError>> + Send + 'a>> {
        self.closed_with = Some(code);
        Box::pin(async { Ok(()) })
    }
}

#[test]
fn t07_async_ws_handshake_driver_with_remote_auth_discord_gg() {
    let mut session =
        RemoteAuthSession::with_keypair(oracle_keypair(), Some(REMOTE_AUTH_WS_ENDPOINT))
            .expect("session");
    let frames = vec![
        r#"{"op":"hello","timeout_ms":120000,"heartbeat_interval":41250}"#.to_string(),
        serde_json::json!({
            "op": "nonce_proof",
            "encrypted_nonce": ORACLE_ENC_NONCE,
        })
        .to_string(),
        serde_json::json!({
            "op": "pending_remote_init",
            "fingerprint": ORACLE_FP,
        })
        .to_string(),
        serde_json::json!({
            "op": "pending_ticket",
            "encrypted_user_payload": ORACLE_ENC_USER,
        })
        .to_string(),
        r#"{"op":"pending_login","ticket":"ticket-live-flow-42"}"#.to_string(),
    ];
    let mut ws = FakeWsTransport::with_frames(frames);
    let outcome = run(run_remote_auth_ws_handshake(&mut ws, &mut session))
        .expect("ws handshake must succeed");

    assert_eq!(
        outcome,
        RemoteAuthHandshakeOutcome::PendingLogin {
            fingerprint: ORACLE_FP.to_string(),
            qr_url: format!("https://discord.com/ra/{ORACLE_FP}"),
            user: Some(RemoteAuthUser {
                id: "1301880740245405707".to_string(),
                discriminator: "0".to_string(),
                avatar: Some("a_1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d".to_string()),
                username: "sanya_discord".to_string(),
            }),
            ticket: "ticket-live-flow-42".to_string(),
        }
    );
    assert_eq!(ws.sent.len(), 2);
    let sent_init: serde_json::Value = serde_json::from_str(&ws.sent[0]).expect("init json");
    assert_eq!(sent_init["op"], "init");
    assert_eq!(sent_init["encoded_public_key"], ORACLE_SPKI_B64);
    let sent_proof: serde_json::Value = serde_json::from_str(&ws.sent[1]).expect("proof json");
    assert_eq!(sent_proof["op"], "nonce_proof");
    assert_eq!(sent_proof["nonce"], ORACLE_NONCE_PROOF);
    assert_eq!(ws.closed_with, Some(REMOTE_AUTH_WS_NORMAL_CLOSE_CODE));
}

type RestCallLog = Arc<Mutex<Vec<(String, String, Vec<u8>)>>>;

#[derive(Clone, Default)]
struct FakeRestTransport {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    calls: RestCallLog,
}

impl FakeRestTransport {
    fn answering(status: u16, body: &[u8]) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.to_vec(),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn with_headers(mut self, headers: &[(&str, &str)]) -> Self {
        self.headers = headers
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self
    }

    fn recorded(&self) -> Vec<(String, String, Vec<u8>)> {
        self.calls.lock().expect("calls lock").clone()
    }
}

impl RemoteAuthRestTransport for FakeRestTransport {
    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, RemoteAuthError>> + Send + 'a>> {
        self.calls.lock().expect("calls lock").push((
            "POST".to_string(),
            target.to_string(),
            body.to_vec(),
        ));
        let response = HttpResponse {
            status: self.status,
            headers: self.headers.clone(),
            body: self.body.clone(),
        };
        Box::pin(async move { Ok(response) })
    }
}

#[test]
fn t08_rest_endpoints_ticket_exchange_mobile_scanner_and_redaction() {
    let mut session =
        RemoteAuthSession::with_keypair(oracle_keypair(), None).expect("oracle session");
    let response_body = serde_json::json!({
        "encrypted_token": ORACLE_ENC_TOKEN,
    })
    .to_string();
    let transport = FakeRestTransport::answering(200, response_body.as_bytes());

    let login_res = run(exchange_remote_auth_ticket(
        &transport,
        &mut session,
        "ra-ticket-998877",
    ))
    .expect("exchange ticket");
    assert_eq!(session.step(), RemoteAuthStep::Finish);
    assert_eq!(login_res.fingerprint, ORACLE_FP);
    assert_eq!(
        login_res.decrypted_token(),
        "MTMwMTg4MDc0MDI0NTQwNTcwNw.Gh7K9x.test_remote_auth_decrypted_token_v2"
    );

    let calls = transport.recorded();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "POST");
    assert_eq!(calls[0].1, "/users/@me/remote-auth/login");
    let sent_json: serde_json::Value = serde_json::from_slice(&calls[0].2).expect("body json");
    assert_eq!(
        sent_json,
        serde_json::json!({ "ticket": "ra-ticket-998877" })
    );

    // Мобильные/десктопные REST-маршруты подтверждения чужого QR (`@275654`):
    let init_transport =
        FakeRestTransport::answering(200, br#"{"handshake_token":"hs-tok-12345"}"#);
    let hs = run(remote_auth_initialize(&init_transport, ORACLE_FP)).expect("initialize");
    assert_eq!(hs.handshake_token, "hs-tok-12345");
    assert_eq!(init_transport.recorded()[0].1, "/users/@me/remote-auth");

    let finish_transport = FakeRestTransport::answering(204, b"");
    run(remote_auth_finish(&finish_transport, "hs-tok-12345", false)).expect("finish");
    assert_eq!(
        finish_transport.recorded()[0].1,
        "/users/@me/remote-auth/finish"
    );

    let cancel_transport = FakeRestTransport::answering(204, b"");
    run(remote_auth_cancel(&cancel_transport, "hs-tok-12345")).expect("cancel");
    assert_eq!(
        cancel_transport.recorded()[0].1,
        "/users/@me/remote-auth/cancel"
    );

    // Обработка 429 RateLimited:
    let rl_transport = FakeRestTransport::answering(429, br#"{"retry_after": 2.5}"#)
        .with_headers(&[("retry-after", "2.5")]);
    assert_eq!(
        run(remote_auth_cancel(&rl_transport, "hs-tok-12345")),
        Err(RemoteAuthError::RateLimited {
            retry_after_ms: 2_500
        })
    );

    // Проверка отсутствия утечки секретов в Debug:
    let debug_str = format!("{:?} {:?} {:?}", oracle_keypair(), session, login_res);
    assert!(
        !debug_str.contains("test_remote_auth_decrypted_token_v2"),
        "Debug не должен раскрывать расшифрованный токен: {debug_str}"
    );
    assert!(
        !debug_str.contains(&ORACLE_P0_HEX[..32]),
        "Debug не должен раскрывать простые множители приватного ключа: {debug_str}"
    );
    let rest_client = DiscordRemoteAuthRest::new("secret_bearer_token_do_not_leak")
        .expect("DiscordRemoteAuthRest");
    assert!(
        !format!("{rest_client:?}").contains("secret_bearer_token_do_not_leak"),
        "Debug клиента REST не должен содержать токен"
    );
}

#[test]
fn t09_main_rs_registers_remote_auth_ipc_commands() {
    let main_src = include_str!("../src/main.rs");
    assert!(
        main_src.contains("pub mod remote_auth;"),
        "main.rs должен подключать модуль remote_auth"
    );
    for cmd in REMOTE_AUTH_IPC_COMMANDS {
        let fn_decl = format!("fn {cmd}(");
        assert!(
            main_src.contains(&fn_decl),
            "main.rs должен объявлять #[tauri::command] {cmd}"
        );
        assert!(
            main_src.contains(cmd),
            "main.rs должен регистрировать {cmd} в generate_handler!"
        );
    }

    // Проверка Base64 и QR URL хелперов:
    let raw = b"remote-auth-binary-test\x00\xff";
    let b64 = encode_base64_standard(raw);
    assert_eq!(decode_base64_flexible(&b64).expect("decode std b64"), raw);
    let b64url = encode_base64url_nopad(raw);
    assert_eq!(decode_base64_flexible(&b64url).expect("decode b64url"), raw);
    assert_eq!(
        format_remote_auth_qr_url(ORACLE_FP).expect("qr url"),
        format!("https://discord.com/ra/{ORACLE_FP}")
    );
    assert_eq!(
        parse_remote_auth_server_frame(r#"{"op":"cancel"}"#).expect("parse cancel"),
        RemoteAuthServerFrame::Cancel
    );
}
