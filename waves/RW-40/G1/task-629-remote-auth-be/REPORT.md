# Отчёт воркера G1 Backend — `task-629-remote-auth-be`

## 1. Карточка / Ветка / Ссылки на RE-оракул
- **Задача:** `task-629-remote-auth-be` (`task_8bf8fe5e68e3` / `ctx_ae8967dae545`)
- **Ветка:** `feature/task-629-remote-auth-be` (база: `unstable`)
- **Отдел:** `G1 Backend (Rust Core & Tauri BFF)`
- **RE-оракулы (`/home/sanya/re-deep/`, read-only):**
  - `/home/sanya/re-deep/decoded/f0cac74b6ccdca5e_0` (`QR_CHUNK`, модуль `52423`):
    - `@2730..@4258`: генерация ключей WebCrypto `RSA-OAEP` (`modulusLength: 2048`, `publicExponent: [1, 0, 1]` = `65537`, `hash: "SHA-256"`), экспорт `spki` DER (`294` Б), `serializePublicKey` (`btoa`), `publicKeyFingerprint` (`base64url_nopad(SHA-256(spki_der))` = `43` символа), `decryptNonce` (`base64url_nopad(RSA-OAEP-SHA256-decrypt(atob(encrypted_nonce)))`), `decryptEncodedCiphertext`, разбор `pending_ticket` по регулярному выражению `^(\d+):(\d{1,4}):([a-zA-Z0-9_]+):(.*)$` (при `avatar === "0"` -> `None`).
    - `@6328..@9860`: протокол WebSocket `wss://remote-auth.discord.gg/?v=2` и `wss://remote-auth-gateway.discord.gg/?v=2` (`PROTOCOL_VERSION = 2`), `Backoff(1500, 30000)`, джиттер первого `heartbeat` `Math.floor(heartbeat_interval * rand)`, состояния `RemoteAuthStep` (`0..=4`), кадры `init`, `hello`, `nonce_proof`, `pending_remote_init`, `pending_ticket`, `pending_login`, `cancel`, `heartbeat`, `heartbeat_ack`, а также `POST /users/@me/remote-auth/login` (`{ "ticket": "..." }` -> `{ "encrypted_token": "..." }`).
  - `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0` (`@275654..@275802`, `@11800708`):
    - маршруты мобильного сканера `REMOTE_AUTH_INITIALIZE = "/users/@me/remote-auth"`, `REMOTE_AUTH_FINISH = "/users/@me/remote-auth/finish"`, `REMOTE_AUTH_CANCEL = "/users/@me/remote-auth/cancel"`, `REMOTE_AUTH_LOGIN = "/users/@me/remote-auth/login"` и шаблон QR `https://discord.com/ra/{fingerprint}`.
  - Спецификация: `waves/RW-39/NET/LOGIN-UI-RE.md`.

## 2. Что сделано
1. **`rust/gui-spike/src/remote_auth.rs`**:
   - Реализована генерация, сериализация и импорт ключевой пары **RSA-2048 (`RSA-OAEP` с `SHA-256` и `MGF1-SHA-256`, RFC 8017 §7.1.2 / §B.2.1)** без добавления внешних зависимостей (`Cargo.toml` не изменялся, `unsafe` не использовался):
     - `Rsa2048PublicKey` / `Rsa2048KeyPair`: экспорт/импорт 294-байтного ASN.1 DER `SubjectPublicKeyInfo` (`OID 1.2.840.113549.1.1.1`), `serialize_public_key` (`Base64`), `fingerprint` (`base64url_nopad(SHA-256(spki_der))`), `qr_url` (`https://discord.com/ra/{fingerprint}`), константно-временная проверка метки `lHash = SHA-256("")` и разделителя `0x01` в блоке `EME-OAEP`, CRT-оптимизация с защитой от сбоев (`m^e mod n == c`).
     - Редактирование чувствительных данных в `fmt::Debug` (`Rsa2048KeyPair`, `RemoteAuthTicketExchangeResponse`, `RemoteAuthNonceProof`) — приватные экспоненты и токены никогда не попадают в логи.
   - Реализован разбор и доказательство `nonce_proof` (`parse_nonce_proof_frame`, `decrypt_and_prove_nonce`, `Rsa2048KeyPair::decrypt_nonce`, `Rsa2048KeyPair::decrypt_encoded_ciphertext`, `Rsa2048KeyPair::decrypt_user_payload`, `parse_remote_auth_user_payload`).
   - Реализованы конечный автомат `RemoteAuthSession` (`on_open`, `handle_server_frame`, `on_heartbeat_tick`, `reset_with_new_keypair`, `snapshot`) и асинхронный WebSocket-драйвер `run_remote_auth_ws_handshake` поверх `tokio-tungstenite` для полного цикла `hello -> init -> nonce_proof -> pending_remote_init -> pending_ticket -> heartbeat/heartbeat_ack -> pending_login / cancel`.
   - Реализованы функции REST v9 обмена билета и мобильного подтверждения QR-входа (`exchange_remote_auth_ticket`, `remote_auth_initialize`, `remote_auth_finish`, `remote_auth_cancel`) с обработкой статусов `200`, `204`, `400/401/403/404`, `429` (`retry_after_ms`) и `5xx`.
2. **`rust/gui-spike/src/main.rs`**:
   - Подключён модуль `pub mod remote_auth;`.
   - Зарегистрированы 7 Tauri IPC-команд (`remote_auth_start_session`, `remote_auth_process_frame`, `remote_auth_heartbeat_tick`, `remote_auth_cancel_session`, `remote_auth_initialize`, `remote_auth_finish`, `remote_auth_cancel`) в `tauri::generate_handler![]`.
   - Три авторизованные сетевые команды (`remote_auth_initialize`, `remote_auth_finish`, `remote_auth_cancel`) маршрутизированы через единый хелпер `token_or(&state.store)` и внесены в `TOKEN_COMMANDS`.
3. **`rust/gui-spike/tests/remote_auth_contract.rs`**:
   - Написаны 9 контрактных тестов (`t01`..`t09`), охватывающих 9 обязательных измерений `TEST-POLICY.md` и сверяющих реализацию с независимыми криптографическими векторами WebCrypto (`crypto.subtle` RSA-OAEP-2048 SHA-256), локальным WebSocket-сервером (`tokio-tungstenite`) и локальным HTTP-сервером (`axum`).

## 3. Strict TDD: RED и GREEN прогоны
- **RED-прогон (`waves/RW-40/G1/task-629-remote-auth-be/red.log`):**
  - Команда: `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target flock /tmp/discord-lab-cargo.lock cargo test --offline --manifest-path rust/gui-spike/Cargo.toml --test remote_auth_contract`
  - Результат: `RED_EXIT=101` (`error[E0583]: file not found for module remote_auth`).
- **GREEN-прогон (`waves/RW-40/G1/task-629-remote-auth-be/green.log`):**
  - Команда: `(CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target flock /tmp/discord-lab-cargo.lock cargo test --offline --manifest-path rust/gui-spike/Cargo.toml --test remote_auth_contract --test ipc_wiring_contract --bin discord-gui-spike && node --test tests/meta/ipc-path.test.mjs)`
  - Результаты:
    - `remote_auth_contract`: **9 passed; 0 failed**
    - `ipc_wiring_contract`: **9 passed; 0 failed**
    - `discord-gui-spike` (`wiring_tests`): **13 passed; 0 failed**
    - `tests/meta/ipc-path.test.mjs`: **12 passed; 0 failed**

## 4. Мутационное тестирование (`5/5 KILLED`, `waves/RW-40/G1/task-629-remote-auth-be/mutations.log`)
| ID | Мутация в `src/remote_auth.rs` | Статус | Убивший тест |
|---|---|---|---|
| `M1` | Мутация суффикса публичной экспоненты в `SPKI_RSA2048_SUFFIX` (`65537 -> 65539`) | **KILLED** (`exit=101`) | `t01`, `t05`, `t06`, `t07`, `t08` |
| `M2` | Замена XOR (`^`) маски `MGF1-SHA256` на OR (`\|`) в `decrypt_oaep_sha256` | **KILLED** (`exit=101`) | `t01`, `t02`, `t03`, `t05`, `t06`, `t07`, `t08` |
| `M3` | Замена `encode_base64url_nopad` на `encode_base64_standard` в `decrypt_nonce` | **KILLED** (`exit=101`) | `t01` |
| `M4` | Отключение проверки MITM несовпадения `fingerprint != expected` в `PendingRemoteInit` | **KILLED** (`exit=101`) | `t06` |
| `M5` | Нарушение правила `avatar == "0" -> None` в `parse_remote_auth_user_payload` | **KILLED** (`exit=101`) | `t05` |

## 5. Изменённые файлы и контрольные суммы (`md5sum`)
- `ff03f67cffbb36107eefbcb43e7ca414  rust/gui-spike/src/remote_auth.rs`
- `766e35ac174059ca1c771d7287d35065  rust/gui-spike/src/main.rs`
- `f52d5e009e57389ced304f90c3ec2a15  rust/gui-spike/tests/remote_auth_contract.rs`

## 6. Остаточные риски
- Сетевые вызовы во всех контрактных тестах изолированы локальными `127.0.0.1:0` WebSocket/HTTP серверами; прямые внешние подключения в тестовом контуре не выполняются.
- Приватный ключ RSA-2048 хранится исключительно в памяти процесса Rust (`RemoteAuthSessionState`) и никогда не покидает BFF через IPC или `Debug`-форматирование.
