# Session Resilience & Security (D-07) — Session Token Protection, OS Keyring Storage, Opcode 6 `RESUME` Recovery, and Rate Limit Backoff

Normative specification for **Task D-07** (`Session Resilience & Security`) targeting **Auth & Storage** (`chord_core::session_resilience`, `chord_core::resume`, `chord_core::gateway::supervisor`, `chord_core::logging`, `chord_core::rest::rate_limit`, `chord_platform::keyring`, and `chord_store::keyring_crypto`). Synthesized from clean-room empirical captures (`docs/re/gateway-protocol.md`, `docs/re/gateway-protocol.json`, `docs/re/rest-d02.md`, `docs/re/backend-ipc.md`, `docs/re/main-arch.md`) and static bundle analysis (`waves/RW-39/NET/AUTH-RE.md`, `waves/RW-39/NET/auth-re.json`).

---

## 0. Quantitative Oracle Summary

| Metric / Constant | Value | Source Oracle |
|---|---:|---|
| Gateway `RESUME` Opcode (`RESUME_OP`) | **`6`** | `docs/re/gateway-protocol.md` §4, `web.js @5017590` |
| Captured Gateway `RESUME -> RESUMED` transitions | **`3`** (`run2:95974.8269`, `run2:95974.8608`, `run2:95974.13072`) | `docs/re/gateway-protocol.json` (`heartbeat.resume.events`) |
| Captured `RESUME` sequence ranges (`firstS..lastS`) | `95..663` (`569`), `664..722` (`59`), `332..371` (`40`) | `docs/re/gateway-protocol.json` (`gaps = 0` across all 3 seams) |
| Captured outbound `Opcode 6 RESUME` ETF frame sizes | **`173` B** (`.8269`) / **`176` B** (`.8608`, `.13072`) | `docs/re/gateway-protocol.json` (`opcodes.sentGateway`) |
| Gateway `HELLO.d.heartbeat_interval` (`DEFAULT_HEARTBEAT_INTERVAL_MS`) | **`41,250` ms** | `docs/re/gateway-protocol.md` §4 (all 7 captured sockets) |
| Gateway `RESUME` exponential backoff range (`RESUME_BACKOFF_MIN_MS..MAX_MS`) | **`1,000..=60,000` ms** (`1s -> 2s -> 4s -> 8s -> 16s -> 32s -> 60s`) | `web.js @5014326` (`new Backoff(1e3, 6e4)`) |
| Consecutive failed `RESUME` threshold (`RESUME_RESET_AFTER_FAILURES`) | **`4`** (5th attempt falls back to `Opcode 2 IDENTIFY`) | `web.js @5014326` (`gatewayBackoff.fails > 4 -> _reset()`) |
| Short-drop session retention threshold (`RESUME_MAX_DOWNTIME_MS`) | **`30,000` ms** (`< 30s -> RESUME`, `>= 30s -> IDENTIFY`) | `chord_core::resume::RESUME_MAX_DOWNTIME_MS` (`T-GW-07`) |
| Heartbeat ACK session TTL (`RESUME_TTL_MS`) | **`180,000` ms** (`3 * 60 * 1000` ms from `lastHeartbeatAckTime`) | `web.js @5019509` (`_doResumeOrIdentify`, `tI = 3 min`) |
| Fatal non-retryable Gateway close codes | **`4004`** (logout + keyring wipe), **`4013`**, **`4014`** (stop) | `web.js @5014409`, `waves/RW-39/NET/ADR-2-auth.md` |
| OS Keyring schema (`D07_KEYRING_SCHEMA_NAME` / `SCHEMA_NAME`) | **`"com.discordlab.Token"`** (`label = "discord-lab token"`) | `chord_platform::keyring::SCHEMA_NAME` (`libsecret 0.21`) |
| Electron `safeStorage` ciphertext prefix (`D07_SAFE_STORAGE_PREFIX`) | **`"dQw4w9WgXcQ:"`** (`+ base64(ciphertext)`) | `core/bundle.js @185907`, `web.js @11844262` (`T-02`) |
| Legacy `SAFE_STORAGE_*` IPC channels | **`3`** (`IS_ENCRYPTION_AVAILABLE`, `ENCRYPT_STRING`, `DECRYPT_STRING`) | `docs/re/backend-ipc.md:65` (`core/#5454`, `preload#5261`) |
| Canonical log & `Debug` redaction marker (`REDACTED_MARKER`) | **`"[REDACTED]"`** (`"REDACTED"` / `"<redacted>"` on wire structs) | `chord_core::logging::REDACTED_MARKER` |
| Default `429` / `503` retry delay (`DEFAULT_RETRY_AFTER`) | **`5,000` ms** (`5` s) | `web.js @10693500` (`f(t.headers, t.body) ?? 5`) |
| Maximum `429` / `503` / `5xx` retry delay (`MAX_RETRY_AFTER`) | **`60,000` ms** (`60` s) | `web.js @10693500` (`new a.A(1e3, 6e4)`) |
| Retryable HTTP `5xx` server statuses (`RETRYABLE_SERVER_STATUSES`) | **`8`** (`[502, 504, 507, 522, 523, 524, 598, 599]`) | `web.js @10692340` (`E = new Set([...])`) |

---

## 1. Session Token Protection & Automatic Secret Masking

### 1.1 Empirical Token Structure & Wire Usage (`AUTH-RE.md` §3–§4)

1. **Canonical Token Formats (`is_valid_discord_token_structure`)**:
   - **Three-Segment User/Bot Token (`core/bundle.js @186263`)**: matches `[\w-]{24,}\.[\w-]{6,}\.[\w-]{27,}` (base64url user Snowflake ID `.` timestamp `.` HMAC signature, `>= 59` chars total).
   - **MFA Token**: matches `mfa.[\w-]{8,}`.
2. **Authorization Header Invariant (`H-01`, `H-02`, `A-03`)**:
   - User session requests transmit the raw token in `Authorization: <raw_token>` without a `Bearer ` prefix (`AuthScheme::User`, `web.js @1002375`).
   - `Bearer <token>` is restricted to OAuth2 application endpoints (`AuthScheme::Bearer`, `web.js @2023976`).
   - During multi-account login (`POST /auth/login`), the client explicitly sets `authorization: ""` so the existing session token is not inherited (`web.js @551990`).
3. **DevTools Token Isolation (`T-11`, `web.js @1016683-1016969`)**:
   - Opening DevTools invokes `hideToken()` (`D07SessionResilienceEngine::hide_token_for_devtools`), removing the active token from primary storage so inspector panels cannot exfiltrate it; closing DevTools invokes `showToken()` (`restore_token_from_devtools`).

### 1.2 Automatic Secret Masking (`D07MaskedToken` & `chord_core::logging`)

1. **Zeroizing Masked Container (`D07MaskedToken`)**:
   - Both `fmt::Debug` and `fmt::Display` unconditionally render `"[REDACTED]"` (`REDACTED_MARKER`).
   - `mask_token_preview` always returns `"[REDACTED]"` without leaking prefix or suffix bytes.
   - `zeroize_now()` and `Drop` overwrite every byte in the token buffer with `0x00` followed by `compiler_fence(Ordering::SeqCst)`.
2. **Struct `Debug` Redaction Across Protocol Boundaries**:
   - `ResumeParams` and `ResumeHandshakeFrame` redact `token` as `"REDACTED"`.
   - `ApiAuth` redacts `authorization` as `"<redacted>"`.
   - `Ready` and `Session` redact `session_id` and `user_settings_proto` as `"<redacted>"`.
   - `DerivedKey` redacts derived Argon2id key material as `DerivedKey("[REDACTED]")`.
3. **Automatic Log Stream Filtering (`redact_secrets`, `RedactedWriter`, `SecretRedactionLayer`)**:
   - Automatically intercepts and masks `Bot <token>`, `Bearer <token>`, 3-part Discord tokens, `mfa.*` tokens, `ghp_*`/`gho_*`/`ghu_*`/`ghs_*`/`ghr_*` tokens, 64-character hex keys (SRTP/DAVE MLS master keys), key-value assignments (`token=`, `password=`, `secret_key=`, `session_id=`, `api_key=`, `key=`), and JSON string fields (`"token": "..."`, `"session_id": "..."`, `"password": "..."`) with `"[REDACTED]"`.

---

## 2. Secure OS Keyring Storage, Electron `safeStorage` Migration, and Argon2id Master Key Derivation

### 2.1 Native OS Keyring Contract (`com.discordlab.Token`)

Unlike the official Electron client on Linux — which falls back to storing plaintext tokens in `Local Storage/leveldb` when `safeStorage.isEncryptionAvailable()` is false or prior to Windows/macOS-only `encryptAndStoreTokens` on `CONNECTION_OPEN` (`AUTH-RE.md` `T-04`, `T-05`, `web.js @2222146`) — Chord strictly stores session tokens only in the OS keyring (`chord_platform::keyring::Keyring` / `D07KeyringTokenVault`):

- **Schema Name**: `SCHEMA_NAME = D07_KEYRING_SCHEMA_NAME = "com.discordlab.Token"`.
- **Attribute Key**: `"key"` (account Snowflake ID or profile key; empty keys and keys containing `\0` are rejected with `InvalidKey` / `InvalidKeyringKey`).
- **Entry Label**: `LABEL = D07_KEYRING_LABEL = "discord-lab token"`.
- **Memory & FFI Safety**: `chord_platform::keyring::Keyring` links `libsecret-1` (`secret_service_get_sync`, `secret_service_store_sync`, `secret_service_lookup_sync`, `secret_service_clear_sync`), wraps lookup buffers in `Zeroizing<Vec<u8>>`, zeroizes invalid UTF-8 bytes on error, and enforces `!Send + !Sync`.

### 2.2 One-Time Electron `safeStorage` Token Migration (`dQw4w9WgXcQ:`)

1. **Wire Format (`core/bundle.js @185907`, `web.js @11844262`)**:
   - Electron `safeStorage` prefixes encrypted base64 tokens with `ENCRYPTED_TOKEN_PREFIX = "dQw4w9WgXcQ:"`.
2. **Idempotent Migration State Machine (`SafeStorageMigrator` & `D07KeyringTokenVault::migrate_safe_storage_once`)**:
   - Strips surrounding whitespace, verifies the `"dQw4w9WgXcQ:"` prefix, validates strict RFC 4648 base64 padding, decrypts the ciphertext inside zeroizing buffers, validates that the plaintext token is non-empty UTF-8 without `\0`, `\n`, or `\r`, and stores it once in the OS keyring.
   - Subsequent migration calls return `Ok(false)` without re-invoking the decryption provider or overwriting an existing keyring entry.

### 2.3 Argon2id Database Master Key Derivation (`chord_store::keyring_crypto`)

- `MasterKeyManager` retrieves or generates a 256-bit CSPRNG secret in the OS keyring (`SecretServiceProvider`) and derives a 32-byte database encryption key (`DerivedKey`) via pure-Rust RFC 9106 **Argon2id** (`argon2id_derive`, `Blake2b-512` `H0` + hybrid Argon2i/Argon2d indexing) bound to the host `hardware_salt`.
- Intermediate 1024-byte memory blocks and raw keyring secrets are zeroized immediately after derivation (`compiler_fence(Ordering::SeqCst)`).

---

## 3. Session Recovery Protocol: Opcode 6 `RESUME` & Abrupt Connection Dropout Handling

### 3.1 Opcode 6 `RESUME` Wire Contract (`docs/re/gateway-protocol.md` §4)

When recovering an interrupted Gateway connection, the client sends **Opcode 6 (`RESUME_OP = 6`)** encoded as an Erlang External Term Format (`ETF`, leading version byte `0x83`) or JSON frame:

```json
{
  "op": 6,
  "d": {
    "token": "<raw_user_token>",
    "session_id": "<ready_session_id>",
    "seq": 663
  }
}
```

### 3.2 Empirically Captured `RESUME -> RESUMED` Transitions (`docs/re/gateway-protocol.json`)

Across the 7 captured Gateway sockets in Wave R7, 3 connections were Opcode 6 `RESUME` handshakes (`173`–`176` B first outbound frame) that replayed missed events and resumed the sequence `s` with zero gaps:

| Socket ID | Previous Socket & Final `s` | Outbound Op 6 Frame | Replayed `s` Range | Dispatches (`op=0`) | Sequence Gaps | First Dispatch Event |
|---|---|---:|---|---:|---:|---|
| `run2:95974.8269` | `run2:95974.5532` (`s = 94`) | `173` B | `95..663` | `569` | `0` | `GUILD_MEMBER_LIST_UPDATE` (+ `RESUMED`) |
| `run2:95974.8608` | `run2:95974.8269` (`s = 663`) | `176` B | `664..722` | `59` | `0` | `GUILD_MEMBER_LIST_UPDATE` (+ `RESUMED`) |
| `run2:95974.13072` | `run2:95974.10029` (`s = 331`, `op=7 RECONNECT`) | `176` B | `332..371` | `40` | `0` | `RESUMED` (`s = 332`) |

### 3.3 Dropout Detection & Recovery State Machine (`D07SessionResilienceEngine`, `ResumeSessionManager`, `Supervisor`)

1. **Dropout Triggers (`D07DropoutCause`)**:
   - **Abrupt Network Drop**: TCP reset / cable unplug (`CloseFrame == None -> classify_close(None) == CloseAction::Reconnect`).
   - **Zombie Socket Detection (`ZombieSocketDetector`)**: no `OP 11 HEARTBEAT_ACK` received within `HELLO.d.heartbeat_interval = 41,250` ms of an outbound heartbeat (`OP 40 QOS_HEARTBEAT` or `OP 1 HEARTBEAT`). Emits `ZombieReconnectSignal { close_immediately: true, reconnect_immediately: true }`.
   - **Server `OP 7 RECONNECT`**: server requests immediate socket migration to `resume_gateway_url` (trailing `/` stripped) followed by Opcode 6 `RESUME`.
   - **Server `OP 9 INVALID_SESSION`**:
     - `d == true`: session remains resumable (`ReconnectAction::Resume`);
     - `d == false`: session is invalidated; `session_id` and `seq` are cleared and the client falls back to a fresh `Opcode 2 IDENTIFY` (`ReconnectAction::Identify`).
2. **Eligibility Guards (`plan_reconnect` & `plan_next_reconnect`)**:
   - **Short-Drop Window (`RESUME_MAX_DOWNTIME_MS = 30,000` ms)**: if `downtime_ms < 30,000` ms, `session_id` is present, and `invalid_session != Some(false)`, plan `ReconnectAction::Resume`; if `downtime_ms >= 30,000` ms, plan `ReconnectAction::Identify`.
   - **3-Minute Heartbeat ACK TTL (`RESUME_TTL_MS = 180,000` ms, `web.js @5019509`)**: if `elapsed_since_last_ack <= 180,000` ms, plan `ReconnectAction::Resume`; if `elapsed_since_last_ack > 180,000` ms, reset `session_id`/`seq` and plan `ReconnectAction::Identify`.
   - **Exponential Backoff & Failure Cap (`1,000..=60,000` ms, `RESUME_RESET_AFTER_FAILURES = 4`)**: retry delays progress deterministically as `1,000 -> 2,000 -> 4,000 -> 8,000 -> 16,000 -> 32,000 -> 60,000` ms. After `4` consecutive failed `RESUME` attempts (`gatewayBackoff.fails > 4`), the session is reset to `ReconnectAction::Identify`; upon `RESUMED` or `READY`, backoff resets to `1,000` ms and `consecutive_failures` resets to `0`.
3. **Cache Preservation Invariant**:
   - Across `RESUME -> RESUMED`, `preserve_guild_cache == true` and `skip_channel_reload == true`: existing `InMemoryCache` guild and channel `Arc` allocations are preserved in-place without clearing or re-fetching channels.
4. **Fatal Close Code Policy (`classify_close` & `handle_close_code`)**:
   - `4004` (`Authentication Failed`): returns `CloseAction::Stop(StopReason::Unauthorized)`, halts reconnection, clears session state, and automatically wipes the invalid token from the OS keyring vault.
   - `4013` / `4014` (`Invalid Intents` / `Disallowed Intents`): returns `CloseAction::Stop(StopReason::InvalidClient { code })`.

---

## 4. Rate Limit Backoff (`429 Too Many Requests`, `503`, and Retryable `5xx`)

Synthesized from `web.js @10692340–10695593` and empirical capture `docs/re/rest-d02.md` §2 (`GET /users/@me/survey` -> `429`):

1. **`429 Too Many Requests` Resolution Order (`rate_limit_info` & `parse_retry_after_or_default`)**:
   - Priority 1: HTTP `Retry-After` header (integer or fractional seconds).
   - Priority 2: JSON response body `"retry_after"` field (outside string literals).
   - Priority 3: HTTP `x-ratelimit-reset-after` header.
   - Default & Ceiling: defaults to `DEFAULT_RETRY_AFTER = 5` s when absent in client-parity mode, capped at `MAX_RETRY_AFTER = 60` s. Malformed, negative, `NaN`, or infinite values never panic.
   - Bucket Metadata: tracks `x-ratelimit-global` / JSON `"global"`, `x-ratelimit-remaining`, `x-ratelimit-limit`, `x-ratelimit-reset-after`, `x-ratelimit-bucket`, and `x-ratelimit-scope` in `RateLimitBucketState`.
2. **`503 Service Unavailable`**:
   - Always retryable across all HTTP methods after `Retry-After` (or `DEFAULT_RETRY_AFTER = 5` s, capped at `60` s).
3. **Retryable `5xx` Statuses (`RETRYABLE_SERVER_STATUSES = [502, 504, 507, 522, 523, 524, 598, 599]`)**:
   - Retried automatically for idempotent HTTP methods (`GET`, `PUT`, `DELETE`) or when `explicit_retries > 0` (e.g. `POST /auth/login` and `POST /auth/mfa/*` with `retries: 2`), using exponential backoff `1s * 2^(attempt - 1)` capped at `60` s.
   - Non-retryable statuses (`401 Unauthorized`, `403`, `404`, `500`, `501`, `505`) return `should_retry = false` and `delay = None`.

---

## 5. Active Session Management & Remote Session Revocation

- **Endpoints (`docs/re/rest-d02.md`, `AUTH-RE.md` `A-17`)**:
  - `GET /users/@me/auth/sessions` / `GET /auth/sessions`: enumerates active device sessions (`id_hash`, `os`, `browser`, `ip`, `location`, `approx_last_used_time`, `is_current`).
  - Current device session displays badge `"Current Device"` (`CURRENT_DEVICE_BADGE_LABEL`) with color `"#23a55a"` (`CURRENT_DEVICE_BADGE_COLOR`).
  - `SessionManager::revoke_session(id_hash, authorized)` validates non-empty `id_hash` and authorization before emitting `DELETE /users/@me/auth/sessions` and removing the revoked session.
