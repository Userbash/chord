#![forbid(unsafe_code)]
#![allow(dead_code)]
//! P1-3.1 + P1-3.3: Clean-Room Discord Remote Auth Desktop v2 (QR Login) Protocol State Machine.
//!
//! Implements the client-side WebSocket state machine (`wss://remote-auth-gateway.discord.gg/?v=2`)
//! and REST v9 token exchange (`POST /api/v9/users/@me/remote-auth/login`):
//! 1. `hello` -> stores `heartbeat_interval_ms` & `qr_timeout_ms`, transitions to [`RemoteAuthState::Handshaking`],
//!    and emits `{"op":"init","encoded_public_key":"<base64 SPKI>"}`.
//! 2. `nonce_proof` -> decrypts `encrypted_nonce` via [`RemoteAuthCipher::decrypt_oaep_sha256`],
//!    computes SHA-256 of the plaintext nonce, encodes the 32-byte digest as URL-safe unpadded
//!    base64 (`base64url`), and emits `{"op":"nonce_proof","proof":"<base64url_sha256>"}`.
//! 3. `pending_remote_init` -> verifies `fingerprint` equals the local SPKI DER SHA-256 `base64url`
//!    digest (preventing MITM key substitution with [`RemoteAuthError::FingerprintMismatch`]),
//!    then transitions to [`RemoteAuthState::AwaitingScan`] with `https://discord.com/ra/{fingerprint}`.
//! 4. `pending_ticket` -> decrypts `"<user_id>:<discriminator>:<avatar_hash>:<username>"` via
//!    `splitn(4, ':')` so usernames containing `:` are preserved intact, transitioning to
//!    [`RemoteAuthState::AwaitingConfirmation`].
//! 5. `pending_login` -> transitions to [`RemoteAuthState::TicketReady`], builds the REST v9
//!    [`HttpRequest`], and decrypts `encrypted_token` into a redacted [`RemoteAuthToken`].
//! 6. `cancel` -> transitions to [`RemoteAuthState::Cancelled`].
//! 7. `tick(now_ms)` -> enforces QR expiration (`started_ms + timeout_ms` -> [`RemoteAuthError::QrExpired`])
//!    and heartbeat cadence (`heartbeat_interval_ms` -> `{"op":"heartbeat"}`, requiring
//!    `{"op":"heartbeat_ack"}` before the next heartbeat or failing with [`RemoteAuthError::HeartbeatTimeout`]).

use std::fmt;

use chord_core::rest::base64;
use chord_core::rest::routes::HttpMethod;
use chord_core::rest::transport::HttpRequest;
use serde::{Deserialize, Serialize};

/// Discord Remote Auth Desktop v2 WebSocket endpoint.
pub const REMOTE_AUTH_GATEWAY_URL: &str = "wss://remote-auth-gateway.discord.gg/?v=2";

/// REST v9 endpoint for exchanging a Remote Auth ticket for an encrypted token.
pub const REMOTE_AUTH_LOGIN_PATH: &str = "/api/v9/users/@me/remote-auth/login";

/// Prefix for the QR code verification URL scanned by the Discord mobile app.
pub const QR_URL_PREFIX: &str = "https://discord.com/ra/";

/// Default QR code lifetime reported by Discord (`120_000` ms = 2 minutes).
pub const DEFAULT_TIMEOUT_MS: u64 = 120_000;

/// Default heartbeat interval reported by Discord (`41_250` ms).
pub const DEFAULT_HEARTBEAT_INTERVAL_MS: u64 = 41_250;

const SHA256_H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// Clean-room NIST FIPS 180-4 SHA-256 implementation in pure safe Rust.
#[must_use]
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut state = SHA256_H0;
    let bit_len = (data.len() as u64).wrapping_mul(8);

    // Pad message: data || 0x80 || 0x00... || bit_len (64-bit BE) to a multiple of 64 bytes.
    let rem = (data.len() + 9) % 64;
    let zero_pad = if rem == 0 { 0 } else { 64 - rem };
    let total_len = data.len() + 1 + zero_pad + 8;

    let mut padded = Vec::with_capacity(total_len);
    padded.extend_from_slice(data);
    padded.push(0x80);
    padded.resize(data.len() + 1 + zero_pad, 0);
    padded.extend_from_slice(&bit_len.to_be_bytes());

    let mut w = [0_u32; 64];
    for chunk in padded.chunks_exact(64) {
        for (i, word_bytes) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word_bytes[0], word_bytes[1], word_bytes[2], word_bytes[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = state[0];
        let mut b = state[1];
        let mut c = state[2];
        let mut d = state[3];
        let mut e = state[4];
        let mut f = state[5];
        let mut g = state[6];
        let mut h = state[7];

        for i in 0..64 {
            let big_s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(big_s1)
                .wrapping_add(ch)
                .wrapping_add(SHA256_K[i])
                .wrapping_add(w[i]);
            let big_s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = big_s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g);
        state[7] = state[7].wrapping_add(h);
    }

    let mut digest = [0_u8; 32];
    for (slot, word) in digest.chunks_exact_mut(4).zip(state.iter()) {
        slot.copy_from_slice(&word.to_be_bytes());
    }
    digest
}

/// Encodes bytes as standard RFC 4648 §4 base64 with `=` padding.
#[must_use]
pub fn encode_base64_standard(data: &[u8]) -> String {
    base64::encode(data)
}

/// Decodes standard RFC 4648 §4 base64 (rejecting empty inputs).
pub fn decode_base64_standard(input: &str) -> Result<Vec<u8>, RemoteAuthError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(RemoteAuthError::InvalidBase64);
    }
    base64::decode(trimmed).map_err(|_| RemoteAuthError::InvalidBase64)
}

/// Encodes bytes as URL-safe RFC 4648 §5 base64 (`base64url`) without `=` padding.
#[must_use]
pub fn encode_base64url_unpadded(data: &[u8]) -> String {
    base64::encode(data)
        .trim_end_matches('=')
        .chars()
        .map(|c| match c {
            '+' => '-',
            '/' => '_',
            other => other,
        })
        .collect()
}

/// Decodes URL-safe RFC 4648 §5 unpadded base64 (`base64url`).
pub fn decode_base64url_unpadded(input: &str) -> Result<Vec<u8>, RemoteAuthError> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.contains(['=', '+', '/']) || trimmed.len() % 4 == 1 {
        return Err(RemoteAuthError::InvalidBase64);
    }
    let mut std_b64: String = trimmed
        .chars()
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            other => other,
        })
        .collect();
    let pad = (4 - (std_b64.len() % 4)) % 4;
    for _ in 0..pad {
        std_b64.push('=');
    }
    base64::decode(&std_b64).map_err(|_| RemoteAuthError::InvalidBase64)
}

/// Computes the `nonce_proof` response digest: `base64url_unpadded(SHA-256(plaintext_nonce))`.
#[must_use]
pub fn compute_nonce_proof(plaintext_nonce: &[u8]) -> String {
    encode_base64url_unpadded(&sha256(plaintext_nonce))
}

/// Computes the local SPKI public key fingerprint: `base64url_unpadded(SHA-256(spki_der))`.
#[must_use]
pub fn compute_spki_fingerprint(spki_der: &[u8]) -> String {
    encode_base64url_unpadded(&sha256(spki_der))
}

/// Trait abstracting RSA-2048 OAEP-SHA256 private-key decryption for Remote Auth v2.
pub trait RemoteAuthCipher {
    /// Decrypts RSA-OAEP-SHA256 `ciphertext` into raw plaintext bytes.
    fn decrypt_oaep_sha256(&self, ciphertext: &[u8]) -> Result<Vec<u8>, RemoteAuthError>;
}

/// Errors produced by the Remote Auth v2 protocol state machine and token exchange.
///
/// Security invariant: no variant ever embeds decrypted token bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteAuthError {
    /// Local DER SPKI public key is empty.
    InvalidPublicKey,
    /// Incoming WebSocket or REST frame is not valid JSON or lacks required fields.
    MalformedJson(String),
    /// Base64 or base64url field could not be decoded.
    InvalidBase64,
    /// RSA-OAEP-SHA256 decryption failed.
    DecryptionFailed(String),
    /// Decrypted nonce plaintext was empty.
    InvalidNonce,
    /// Server `pending_remote_init` fingerprint did not match the local SPKI SHA-256 digest (MITM guard).
    FingerprintMismatch { expected: String, actual: String },
    /// Decrypted `pending_ticket` user payload was malformed or had an invalid snowflake.
    InvalidUserPayload,
    /// Login ticket in `pending_login` or exchange request was empty.
    InvalidTicket,
    /// Decrypted auth token was empty, non-UTF-8, or contained control characters (`\0`, `\r`, `\n`).
    InvalidToken,
    /// Opcode arrived in an invalid protocol state.
    UnexpectedOpcode { op: String, state: &'static str },
    /// Unknown `op` value in incoming WebSocket frame.
    UnknownOpcode(String),
    /// `hello` frame reported zero `timeout_ms` or `heartbeat_interval`.
    InvalidHelloTiming,
    /// Server failed to send `heartbeat_ack` before the next heartbeat interval elapsed.
    HeartbeatTimeout,
    /// QR code expired at `started_ms + timeout_ms`.
    QrExpired,
}

impl fmt::Display for RemoteAuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPublicKey => f.write_str("remote auth SPKI public key must not be empty"),
            Self::MalformedJson(reason) => write!(f, "malformed remote auth JSON: {reason}"),
            Self::InvalidBase64 => f.write_str("invalid base64 payload in remote auth frame"),
            Self::DecryptionFailed(reason) => {
                write!(f, "RSA-OAEP-SHA256 decryption failed: {reason}")
            }
            Self::InvalidNonce => f.write_str("decrypted remote auth nonce is empty"),
            Self::FingerprintMismatch { expected, actual } => write!(
                f,
                "remote auth SPKI fingerprint mismatch (expected {expected}, got {actual})"
            ),
            Self::InvalidUserPayload => {
                f.write_str("invalid decrypted user payload in pending_ticket")
            }
            Self::InvalidTicket => f.write_str("remote auth login ticket must not be empty"),
            Self::InvalidToken => f.write_str(
                "decrypted remote auth token is invalid (empty or contains control chars)",
            ),
            Self::UnexpectedOpcode { op, state } => {
                write!(f, "unexpected opcode {op:?} while in state {state}")
            }
            Self::UnknownOpcode(op) => write!(f, "unknown remote auth opcode {op:?}"),
            Self::InvalidHelloTiming => {
                f.write_str("invalid hello timing (timeout_ms and heartbeat_interval must be > 0)")
            }
            Self::HeartbeatTimeout => {
                f.write_str("remote auth heartbeat timed out (missed heartbeat_ack)")
            }
            Self::QrExpired => f.write_str("remote auth QR code expired"),
        }
    }
}

impl std::error::Error for RemoteAuthError {}

/// Decrypted user preview received in `pending_ticket` after scanning the QR code on mobile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingUserPreview {
    pub user_id: String,
    pub discriminator: String,
    pub avatar_hash: String,
    pub username: String,
}

impl PendingUserPreview {
    /// Parses `"<user_id>:<discriminator>:<avatar_hash>:<username>"` using `splitn(4, ':')`
    /// so usernames containing `:` are preserved intact.
    pub fn parse(raw_utf8: &str) -> Result<Self, RemoteAuthError> {
        let mut parts = raw_utf8.splitn(4, ':');
        let user_id = parts.next().ok_or(RemoteAuthError::InvalidUserPayload)?;
        let discriminator = parts.next().ok_or(RemoteAuthError::InvalidUserPayload)?;
        let avatar_hash = parts.next().ok_or(RemoteAuthError::InvalidUserPayload)?;
        let username = parts.next().ok_or(RemoteAuthError::InvalidUserPayload)?;

        if user_id.is_empty()
            || !user_id.bytes().all(|b| b.is_ascii_digit())
            || user_id.parse::<u64>().unwrap_or(0) == 0
        {
            return Err(RemoteAuthError::InvalidUserPayload);
        }
        if discriminator.is_empty() || username.is_empty() || username.contains(['\0', '\r', '\n'])
        {
            return Err(RemoteAuthError::InvalidUserPayload);
        }

        Ok(Self {
            user_id: user_id.to_owned(),
            discriminator: discriminator.to_owned(),
            avatar_hash: avatar_hash.to_owned(),
            username: username.to_owned(),
        })
    }
}

/// Redacted token returned after decrypting `POST /api/v9/users/@me/remote-auth/login`.
#[derive(Clone, PartialEq, Eq)]
pub struct RemoteAuthToken {
    inner: String,
}

impl RemoteAuthToken {
    /// Validates and constructs a [`RemoteAuthToken`], rejecting empty strings or strings
    /// containing `\0`, `\r`, or `\n`.
    pub fn new(raw: impl AsRef<str>) -> Result<Self, RemoteAuthError> {
        let trimmed = raw.as_ref().trim();
        if trimmed.is_empty() || trimmed.contains(['\0', '\r', '\n']) {
            return Err(RemoteAuthError::InvalidToken);
        }
        Ok(Self {
            inner: trimmed.to_owned(),
        })
    }

    /// Explicitly exposes the raw token string for authorized storage in `TokenStore`.
    #[must_use]
    pub fn expose_secret(&self) -> &str {
        &self.inner
    }
}

impl fmt::Debug for RemoteAuthToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RemoteAuthToken([REDACTED])")
    }
}

impl fmt::Display for RemoteAuthToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

/// State of a Discord Remote Auth v2 QR login session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteAuthState {
    /// Initial state before receiving `hello`.
    Connecting,
    /// Received `hello`, sent `init` (and `nonce_proof`), awaiting `pending_remote_init`.
    Handshaking,
    /// Verified local SPKI fingerprint against `pending_remote_init`; displaying QR code.
    AwaitingScan {
        qr_url: String,
        fingerprint: String,
        expires_at_ms: u64,
    },
    /// User scanned QR code on mobile (`pending_ticket`); awaiting confirmation on mobile.
    AwaitingConfirmation(PendingUserPreview),
    /// User approved login on mobile (`pending_login`); ticket is ready for REST exchange.
    TicketReady { ticket: String },
    /// Token exchange completed and decrypted.
    Completed,
    /// User cancelled QR login on mobile (`cancel`).
    Cancelled,
    /// Terminal error state (MITM mismatch, QR expiration, or heartbeat timeout).
    Failed(RemoteAuthError),
}

impl RemoteAuthState {
    /// Returns the static state name for diagnostics.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Connecting => "Connecting",
            Self::Handshaking => "Handshaking",
            Self::AwaitingScan { .. } => "AwaitingScan",
            Self::AwaitingConfirmation(_) => "AwaitingConfirmation",
            Self::TicketReady { .. } => "TicketReady",
            Self::Completed => "Completed",
            Self::Cancelled => "Cancelled",
            Self::Failed(_) => "Failed",
        }
    }
}

#[derive(Deserialize)]
struct RawInboundEnvelope {
    op: String,
    timeout_ms: Option<u64>,
    heartbeat_interval: Option<u64>,
    encrypted_nonce: Option<String>,
    fingerprint: Option<String>,
    encrypted_user_payload: Option<String>,
    ticket: Option<String>,
}

#[derive(Serialize)]
struct OutboundInit<'a> {
    op: &'static str,
    encoded_public_key: &'a str,
}

#[derive(Serialize)]
struct OutboundNonceProof<'a> {
    op: &'static str,
    proof: &'a str,
}

#[derive(Serialize)]
struct OutboundHeartbeat {
    op: &'static str,
}

#[derive(Serialize)]
struct LoginExchangeRequestBody<'a> {
    ticket: &'a str,
}

#[derive(Deserialize)]
struct LoginExchangeResponseBody {
    encrypted_token: String,
}

/// Builds the REST v9 `POST /api/v9/users/@me/remote-auth/login` [`HttpRequest`] for `ticket`.
pub fn build_login_exchange_request(ticket: &str) -> Result<HttpRequest, RemoteAuthError> {
    let trimmed = ticket.trim();
    if trimmed.is_empty() {
        return Err(RemoteAuthError::InvalidTicket);
    }
    let body = serde_json::to_vec(&LoginExchangeRequestBody { ticket: trimmed })
        .map_err(|e| RemoteAuthError::MalformedJson(e.to_string()))?;
    Ok(HttpRequest::new(HttpMethod::Post, REMOTE_AUTH_LOGIN_PATH)
        .header("content-type", "application/json")
        .body(body))
}

/// Parses `{"encrypted_token": "<base64>"}` and decrypts it via [`RemoteAuthCipher`] into a
/// redacted [`RemoteAuthToken`].
pub fn decrypt_login_exchange_response(
    response_json: &str,
    cipher: &dyn RemoteAuthCipher,
) -> Result<RemoteAuthToken, RemoteAuthError> {
    let parsed: LoginExchangeResponseBody = serde_json::from_str(response_json)
        .map_err(|e| RemoteAuthError::MalformedJson(e.to_string()))?;
    let ciphertext = decode_base64_standard(&parsed.encrypted_token)?;
    let plaintext = cipher.decrypt_oaep_sha256(&ciphertext)?;
    let raw_token = String::from_utf8(plaintext).map_err(|_| RemoteAuthError::InvalidToken)?;
    RemoteAuthToken::new(raw_token)
}

/// Client-side state machine for Discord Remote Auth Desktop v2 (`wss://remote-auth-gateway.discord.gg/?v=2`).
#[derive(Debug, Clone)]
pub struct RemoteAuthSession {
    spki_der: Vec<u8>,
    expected_fingerprint: String,
    started_ms: u64,
    heartbeat_interval_ms: Option<u64>,
    qr_timeout_ms: Option<u64>,
    expires_at_ms: Option<u64>,
    last_heartbeat_sent_ms: Option<u64>,
    awaiting_heartbeat_ack: bool,
    nonce_verified: bool,
    state: RemoteAuthState,
}

impl RemoteAuthSession {
    /// Creates a new Remote Auth v2 session with the client's DER-encoded SPKI RSA-2048 public key.
    pub fn new(spki_der: Vec<u8>, started_ms: u64) -> Result<Self, RemoteAuthError> {
        if spki_der.is_empty() {
            return Err(RemoteAuthError::InvalidPublicKey);
        }
        let expected_fingerprint = compute_spki_fingerprint(&spki_der);
        Ok(Self {
            spki_der,
            expected_fingerprint,
            started_ms,
            heartbeat_interval_ms: None,
            qr_timeout_ms: None,
            expires_at_ms: None,
            last_heartbeat_sent_ms: None,
            awaiting_heartbeat_ack: false,
            nonce_verified: false,
            state: RemoteAuthState::Connecting,
        })
    }

    /// Returns the current [`RemoteAuthState`].
    #[must_use]
    pub const fn state(&self) -> &RemoteAuthState {
        &self.state
    }

    /// Returns the local SPKI public key fingerprint (`base64url_unpadded(SHA-256(spki_der))`).
    #[must_use]
    pub fn expected_fingerprint(&self) -> &str {
        &self.expected_fingerprint
    }

    /// Returns the negotiated heartbeat interval in milliseconds after `hello`.
    #[must_use]
    pub const fn heartbeat_interval_ms(&self) -> Option<u64> {
        self.heartbeat_interval_ms
    }

    /// Returns the negotiated QR timeout in milliseconds after `hello`.
    #[must_use]
    pub const fn qr_timeout_ms(&self) -> Option<u64> {
        self.qr_timeout_ms
    }

    /// Processes an incoming WebSocket JSON frame and returns an optional outbound JSON frame.
    pub fn handle_frame(
        &mut self,
        raw_json: &str,
        cipher: &dyn RemoteAuthCipher,
        now_ms: u64,
    ) -> Result<Option<String>, RemoteAuthError> {
        let env: RawInboundEnvelope = serde_json::from_str(raw_json)
            .map_err(|e| RemoteAuthError::MalformedJson(e.to_string()))?;

        match env.op.as_str() {
            "hello" => {
                if !matches!(self.state, RemoteAuthState::Connecting) {
                    return Err(RemoteAuthError::UnexpectedOpcode {
                        op: env.op,
                        state: self.state.name(),
                    });
                }
                let timeout_ms = env.timeout_ms.ok_or_else(|| {
                    RemoteAuthError::MalformedJson("missing timeout_ms".to_owned())
                })?;
                let heartbeat_interval = env.heartbeat_interval.ok_or_else(|| {
                    RemoteAuthError::MalformedJson("missing heartbeat_interval".to_owned())
                })?;
                if timeout_ms == 0 || heartbeat_interval == 0 {
                    return Err(RemoteAuthError::InvalidHelloTiming);
                }

                self.qr_timeout_ms = Some(timeout_ms);
                self.heartbeat_interval_ms = Some(heartbeat_interval);
                self.expires_at_ms = Some(self.started_ms.saturating_add(timeout_ms));
                self.last_heartbeat_sent_ms = Some(now_ms);
                self.awaiting_heartbeat_ack = false;
                self.state = RemoteAuthState::Handshaking;

                let encoded_pk = encode_base64_standard(&self.spki_der);
                let reply = serde_json::to_string(&OutboundInit {
                    op: "init",
                    encoded_public_key: &encoded_pk,
                })
                .map_err(|e| RemoteAuthError::MalformedJson(e.to_string()))?;
                Ok(Some(reply))
            }
            "nonce_proof" => {
                if !matches!(self.state, RemoteAuthState::Handshaking) {
                    return Err(RemoteAuthError::UnexpectedOpcode {
                        op: env.op,
                        state: self.state.name(),
                    });
                }
                let enc_nonce_b64 = env.encrypted_nonce.ok_or_else(|| {
                    RemoteAuthError::MalformedJson("missing encrypted_nonce".to_owned())
                })?;
                let ciphertext = decode_base64_standard(&enc_nonce_b64)?;
                let plaintext_nonce = cipher.decrypt_oaep_sha256(&ciphertext)?;
                if plaintext_nonce.is_empty() {
                    return Err(RemoteAuthError::InvalidNonce);
                }
                let proof = compute_nonce_proof(&plaintext_nonce);
                self.nonce_verified = true;

                let reply = serde_json::to_string(&OutboundNonceProof {
                    op: "nonce_proof",
                    proof: &proof,
                })
                .map_err(|e| RemoteAuthError::MalformedJson(e.to_string()))?;
                Ok(Some(reply))
            }
            "pending_remote_init" => {
                if !matches!(self.state, RemoteAuthState::Handshaking) || !self.nonce_verified {
                    return Err(RemoteAuthError::UnexpectedOpcode {
                        op: env.op,
                        state: self.state.name(),
                    });
                }
                let server_fp = env.fingerprint.ok_or_else(|| {
                    RemoteAuthError::MalformedJson("missing fingerprint".to_owned())
                })?;
                if server_fp != self.expected_fingerprint {
                    let err = RemoteAuthError::FingerprintMismatch {
                        expected: self.expected_fingerprint.clone(),
                        actual: server_fp,
                    };
                    self.state = RemoteAuthState::Failed(err.clone());
                    return Err(err);
                }

                let expires_at_ms = self
                    .expires_at_ms
                    .unwrap_or_else(|| self.started_ms.saturating_add(DEFAULT_TIMEOUT_MS));
                let qr_url = format!("{QR_URL_PREFIX}{server_fp}");
                self.state = RemoteAuthState::AwaitingScan {
                    qr_url,
                    fingerprint: server_fp,
                    expires_at_ms,
                };
                Ok(None)
            }
            "pending_ticket" => {
                if !matches!(self.state, RemoteAuthState::AwaitingScan { .. }) {
                    return Err(RemoteAuthError::UnexpectedOpcode {
                        op: env.op,
                        state: self.state.name(),
                    });
                }
                let enc_user_b64 = env.encrypted_user_payload.ok_or_else(|| {
                    RemoteAuthError::MalformedJson("missing encrypted_user_payload".to_owned())
                })?;
                let ciphertext = decode_base64_standard(&enc_user_b64)?;
                let plaintext = cipher.decrypt_oaep_sha256(&ciphertext)?;
                let raw_utf8 = String::from_utf8(plaintext)
                    .map_err(|_| RemoteAuthError::InvalidUserPayload)?;
                let preview = PendingUserPreview::parse(&raw_utf8)?;
                self.state = RemoteAuthState::AwaitingConfirmation(preview);
                Ok(None)
            }
            "pending_login" => {
                if !matches!(self.state, RemoteAuthState::AwaitingConfirmation(_)) {
                    return Err(RemoteAuthError::UnexpectedOpcode {
                        op: env.op,
                        state: self.state.name(),
                    });
                }
                let raw_ticket = env
                    .ticket
                    .ok_or_else(|| RemoteAuthError::MalformedJson("missing ticket".to_owned()))?;
                let trimmed = raw_ticket.trim();
                if trimmed.is_empty() {
                    return Err(RemoteAuthError::InvalidTicket);
                }
                self.state = RemoteAuthState::TicketReady {
                    ticket: trimmed.to_owned(),
                };
                Ok(None)
            }
            "cancel" => {
                if !matches!(
                    self.state,
                    RemoteAuthState::Handshaking
                        | RemoteAuthState::AwaitingScan { .. }
                        | RemoteAuthState::AwaitingConfirmation(_)
                ) {
                    return Err(RemoteAuthError::UnexpectedOpcode {
                        op: env.op,
                        state: self.state.name(),
                    });
                }
                self.state = RemoteAuthState::Cancelled;
                Ok(None)
            }
            "heartbeat_ack" => {
                if !matches!(
                    self.state,
                    RemoteAuthState::Handshaking
                        | RemoteAuthState::AwaitingScan { .. }
                        | RemoteAuthState::AwaitingConfirmation(_)
                        | RemoteAuthState::TicketReady { .. }
                ) {
                    return Err(RemoteAuthError::UnexpectedOpcode {
                        op: env.op,
                        state: self.state.name(),
                    });
                }
                self.awaiting_heartbeat_ack = false;
                Ok(None)
            }
            other => Err(RemoteAuthError::UnknownOpcode(other.to_owned())),
        }
    }

    /// Advances timers to `now_ms`, checking QR expiration (`started_ms + timeout_ms`) and
    /// heartbeat cadence (`heartbeat_interval_ms`).
    pub fn tick(&mut self, now_ms: u64) -> Result<Option<String>, RemoteAuthError> {
        match &self.state {
            RemoteAuthState::Connecting
            | RemoteAuthState::Completed
            | RemoteAuthState::Cancelled => return Ok(None),
            RemoteAuthState::Failed(err) => return Err(err.clone()),
            RemoteAuthState::Handshaking
            | RemoteAuthState::AwaitingScan { .. }
            | RemoteAuthState::AwaitingConfirmation(_)
            | RemoteAuthState::TicketReady { .. } => {}
        }

        if let Some(expires_at_ms) = self.expires_at_ms {
            if now_ms >= expires_at_ms {
                self.state = RemoteAuthState::Failed(RemoteAuthError::QrExpired);
                return Err(RemoteAuthError::QrExpired);
            }
        }

        let Some(interval_ms) = self.heartbeat_interval_ms else {
            return Ok(None);
        };
        let last_sent = self.last_heartbeat_sent_ms.unwrap_or(self.started_ms);
        if now_ms.saturating_sub(last_sent) >= interval_ms {
            if self.awaiting_heartbeat_ack {
                self.state = RemoteAuthState::Failed(RemoteAuthError::HeartbeatTimeout);
                return Err(RemoteAuthError::HeartbeatTimeout);
            }
            self.awaiting_heartbeat_ack = true;
            self.last_heartbeat_sent_ms = Some(now_ms);
            let hb = serde_json::to_string(&OutboundHeartbeat { op: "heartbeat" })
                .map_err(|e| RemoteAuthError::MalformedJson(e.to_string()))?;
            return Ok(Some(hb));
        }

        Ok(None)
    }

    /// Builds the REST v9 `POST /api/v9/users/@me/remote-auth/login` [`HttpRequest`] when the
    /// session is in [`RemoteAuthState::TicketReady`].
    pub fn build_login_request(&self) -> Result<HttpRequest, RemoteAuthError> {
        match &self.state {
            RemoteAuthState::TicketReady { ticket } => build_login_exchange_request(ticket),
            _ => Err(RemoteAuthError::UnexpectedOpcode {
                op: "build_login_request".to_owned(),
                state: self.state.name(),
            }),
        }
    }

    /// Decrypts the REST v9 `{"encrypted_token": "<base64>"}` response via [`RemoteAuthCipher`]
    /// and transitions the session from [`RemoteAuthState::TicketReady`] to [`RemoteAuthState::Completed`].
    pub fn complete_login(
        &mut self,
        response_json: &str,
        cipher: &dyn RemoteAuthCipher,
    ) -> Result<RemoteAuthToken, RemoteAuthError> {
        if !matches!(self.state, RemoteAuthState::TicketReady { .. }) {
            return Err(RemoteAuthError::UnexpectedOpcode {
                op: "complete_login".to_owned(),
                state: self.state.name(),
            });
        }
        let token = decrypt_login_exchange_response(response_json, cipher)?;
        self.state = RemoteAuthState::Completed;
        Ok(token)
    }
}
