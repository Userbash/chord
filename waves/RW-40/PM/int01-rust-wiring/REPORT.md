# REPORT: RW-40 / INT-01-RUST-WIRING — Rust IPC-обёртки и Tauri-регистрация (`attachments`, `invites`, `pins_threads`) + проверка экспорта `devices`

- **Batch / Волна**: `RW-40` (Rust Integration Lane G1)
- **Карточка**: `INT-01-RUST-WIRING`
- **Воркер**: Worker G1 Rust Integration Lane
- **HEAD**: `4e240481e59ff4d49732ac8bd5ba08a006363748`
- **Назначенные файлы**:
  - `rust/crates/discord-voice/src/lib.rs`
  - `rust/gui-spike/src/lib.rs`
  - `rust/gui-spike/src/main.rs`
  - `rust/gui-spike/tests/ipc_wiring_contract.rs`

## 1. Что сделано

1. **Проверка экспортов модулей**:
   - Проверен экспорт `pub mod devices;` в `rust/crates/discord-voice/src/lib.rs` (строка 13) и зелёный контракт `devices_contract` (`5 passed; 0 failed`).
   - Проверены экспорты `pub mod attachments;`, `pub mod invites;`, `pub mod pins_threads;` в `rust/gui-spike/src/lib.rs` (строки 36, 41, 45).
2. **Контрактный тест `rust/gui-spike/tests/ipc_wiring_contract.rs` (5 независимых тестов)**:
   - `ipc_wrapper_request_upload_slots_delegates_and_enforces_contract`: проверяет публичную обёртку `discord_gui_spike::request_upload_slots` (`POST /channels/{id}/attachments`, сохранение порядка файлов и всех RE-полей `id`, `upload_url`, `upload_filename`, pre-flight валидацию небезопасных `channel_id`, пустого списка и превышения `MAX_UPLOAD_BYTES_FREE`, маппинг `429 Retry-After` и `413`/`40005`).
   - `ipc_wrapper_create_channel_invite_delegates_and_enforces_re_tables`: проверяет публичную обёртку `discord_gui_spike::create_channel_invite` (`POST /channels/{id}/invites`, сериализацию `max_age`/`max_uses`/`temporary`, RE-таблицы `ALLOWED_MAX_AGE_SECS`/`ALLOWED_MAX_USES`, pre-flight отклонение небезопасных `channel_id`, маппинг `429 Retry-After` и `403`).
   - `ipc_wrapper_pins_and_threads_route_get_put_delete_and_archived_flag`: проверяет публичные обёртки `discord_gui_spike::list_channel_pins` (`GET /channels/{id}/pins`), `discord_gui_spike::pin_channel_message` (`PUT /channels/{id}/pins/{message_id}`), `discord_gui_spike::unpin_channel_message` (`DELETE /channels/{id}/pins/{message_id}`) и `discord_gui_spike::list_channel_threads` (`GET /channels/{id}/threads/active` при `archived=false` и `GET /channels/{id}/threads/archived/public` при `archived=true`), а также pre-flight валидацию идентификаторов и `429 Retry-After`.
   - `tauri_main_registers_and_wires_all_six_ipc_commands_via_token_or`: проверяет `IPC_WIRING_COMMANDS`, объявление всех 6 `#[tauri::command]` в `rust/gui-spike/src/main.rs`, регистрацию в `tauri::generate_handler![...]`, включение в `TOKEN_COMMANDS`, чтение Bearer исключительно через `token_or::<...>(move || backend.token(), ...)` и делегирование в `discord_gui_spike::<command>`.
   - `ipc_error_kinds_serialize_to_expected_frontend_tags_without_leaking_secrets`: проверяет сериализацию тегов `kind` для фронтенда (`AttachmentError`, `InviteError`, `PinsThreadsError`) и отсутствие утечки секретов в JSON/Display.
3. **Публичные IPC-обёртки и Tauri-команды (`rust/gui-spike/src/lib.rs`, `rust/gui-spike/src/main.rs`)**:
   - В `rust/gui-spike/src/lib.rs` добавлены константа `IPC_WIRING_COMMANDS: [&str; 6]` и публичные async-обёртки `request_upload_slots`, `create_channel_invite`, `list_channel_pins`, `pin_channel_message`, `unpin_channel_message`, `list_channel_threads`.
   - В `rust/gui-spike/src/main.rs` добавлены одноимённые `#[tauri::command]`, читающие токен через `token_or`, зарегистрированы в `tauri::generate_handler![...]` и добавлены в `TOKEN_COMMANDS` (`[&str; 18]`). Сигнатуры проверены мета-тестом `tests/meta/ipc-path.test.mjs` (`12 passed; 0 failed`).

## 2. Red-прогон (до реализации обёрток в `lib.rs` и `main.rs`)

- **Команда**:
  ```bash
  (cd rust/gui-spike && CARGO_TARGET_DIR=/tmp/target-int01 cargo test --offline -p discord-gui-spike --test ipc_wiring_contract) > waves/RW-40/PM/int01-rust-wiring/red.log 2>&1
  ```
- **Код возврата**: `101`
- **Лог**: `waves/RW-40/PM/int01-rust-wiring/red.log` (`error[E0432]: unresolved imports discord_gui_spike::create_channel_invite, discord_gui_spike::list_channel_pins, discord_gui_spike::list_channel_threads, discord_gui_spike::pin_channel_message, discord_gui_spike::request_upload_slots, discord_gui_spike::unpin_channel_message, discord_gui_spike::IPC_WIRING_COMMANDS`).

## 3. Green-прогон и Clippy

- **Green-команда**:
  ```bash
  (cd rust/gui-spike && CARGO_TARGET_DIR=/tmp/target-int01 cargo test --offline -p discord-gui-spike --test ipc_wiring_contract) > waves/RW-40/PM/int01-rust-wiring/green.log 2>&1
  ```
- **Результат**: `ok. 5 passed; 0 failed; 0 ignored` (exit `0`, лог `waves/RW-40/PM/int01-rust-wiring/green.log`).
- **Дополнительные проверки**:
  - `cargo test --offline -p discord-gui-spike --bin discord-gui-spike` → `9 passed; 0 failed` (`invoke_handler_registers_all_bff_commands`, `every_network_command_reads_token_through_one_helper` и др.).
  - `cargo test --offline -p discord-gui-spike --test attachments_contract --test invites_contract --test pins_threads_contract` → `17 + 20 + 6 passed; 0 failed`.
  - `cargo test --manifest-path rust/Cargo.toml --offline -p discord-voice --test devices_contract` → `5 passed; 0 failed`.
  - `node --test tests/meta/ipc-path.test.mjs` → `12 passed; 0 failed`.
- **Clippy-команда**:
  ```bash
  (cd rust/gui-spike && CARGO_TARGET_DIR=/tmp/target-int01 cargo clippy --offline -p discord-gui-spike --lib --bin discord-gui-spike --test ipc_wiring_contract -- -D warnings && cd /home/user/orca/project && CARGO_TARGET_DIR=/tmp/target-int01 cargo clippy --manifest-path rust/Cargo.toml --offline -p discord-voice --lib --test devices_contract -- -D warnings) > waves/RW-40/PM/int01-rust-wiring/clippy.log 2>&1
  ```
- **Результат Clippy**: exit `0`, 0 предупреждений (лог `waves/RW-40/PM/int01-rust-wiring/clippy.log`).

## 3.1. Oracle (независимые источники контракта, PROCESS.md §4.2)

- **Вложения (`request_upload_slots`, тест `ipc_wrapper_request_upload_slots_delegates_and_enforces_contract`)**:
  - Независимый RE-оракул `waves/RW-40/reference-oracle-REA.md` §1 (строки 115–138, дамп `reference-oracle/decoded/4d2a64cb5df5fc4e_0@6011428`, `@6804500`, `@6805978`) и `waves/RW-40/ATTACH-RE.md` §2:
  - Подтверждает проверяемые в тесте ожидания: маршрут `POST /channels/{channel_id}/attachments`, структуру тела `{"files": [{"filename", "file_size", "id", "is_clip"}]}`, структуру ответа `{"attachments": [{"id", "upload_url", "upload_filename"}]}`, базовый лимит `MAX_UPLOAD_BYTES_FREE = 20 MiB` (`Math.max(0x1400000, tierLimit)` = `20_971_520` байт) и маппинг `413`/`40005` → `AttachmentError::EntityTooLarge` и `429 Retry-After` → `AttachmentError::RateLimited`.
- **Приглашения (`create_channel_invite`, тест `ipc_wrapper_create_channel_invite_delegates_and_enforces_re_tables`)**:
  - Независимый RE-оракул `waves/RW-40/reference-oracle-REA.md` §3 (строки 318–355, таблицы TTL и `max_uses` из `reference-oracle/decoded/4d2a64cb5df5fc4e_0@632264` и `@1933819`) и §4.3 (строки 435–460, `INSTANT_INVITES: e => "/channels/${e}/invites"` из `4d2a64cb5df5fc4e_0@270160`):
  - Подтверждает проверяемые в тесте ожидания: маршрут `POST /channels/{channel_id}/invites`, поля `max_age`/`max_uses`/`temporary`, точные RE-таблицы `ALLOWED_MAX_AGE_SECS = [0, 1800, 3600, 21600, 43200, 86400, 604800]` и `ALLOWED_MAX_USES = [0, 1, 5, 10, 25, 50, 100]`, а также маппинг `429 Retry-After` и `403 Forbidden`.
- **Пины и ветки (`list_channel_pins`, `pin_channel_message`, `unpin_channel_message`, `list_channel_threads`, тест `ipc_wrapper_pins_and_threads_route_get_put_delete_and_archived_flag`)**:
  - Источник 1 (RE-дамп): `reference-oracle/decoded/4d2a64cb5df5fc4e_0@266996` (`ALL_ARCHIVED_THREADS:(e,t)=>"/channels/${e}/threads/archived/${t}"` при `t="public"` → `/channels/{id}/threads/archived/public`) и `@270063..270155` (семейство маршрутов пинов канала).
  - Источник 2 (независимый согласованный контракт `[A]` бэкенд-модуля, принятый в `waves/RW-40/PM/m10-pins-be/ACCEPT.md` и `waves/RW-40/PM/m10-pins-be/REVIEW-PASS2.md`): специфицирует канонические REST v9/v10 маршруты проектного контракта `discord-gui-spike::pins_threads`, делегирование в которые проверяет данный IPC-тест:
    - `list_channel_pins` → `GET /channels/{id}/pins` (`[A]` контракт `pins_threads.rs`);
    - `pin_channel_message` → `PUT /channels/{id}/pins/{message_id}` (`204 No Content`, `[A]` контракт `pins_threads.rs`);
    - `unpin_channel_message` → `DELETE /channels/{id}/pins/{message_id}` (`204 No Content`, `[A]` контракт `pins_threads.rs`);
    - `list_channel_threads(archived=false)` → `GET /channels/{id}/threads/active` (`[A]` контракт `pins_threads.rs`);
    - `list_channel_threads(archived=true)` → `GET /channels/{id}/threads/archived/public` (`[D]` `4d2a64cb5df5fc4e_0@266996` + `[A]` контракт `pins_threads.rs`).
- **Контракт безопасности Tauri IPC (тесты `tauri_main_registers_and_wires_all_six_ipc_commands_via_token_or` и `ipc_error_kinds_serialize_to_expected_frontend_tags_without_leaking_secrets`)**:
  - Независимый мета-тест `tests/meta/ipc-path.test.mjs` и контракт `rust/gui-spike/src/main.rs` (`TOKEN_COMMANDS`, `token_or::<E, _>(move || backend.token(), ...)` без передачи Bearer-токена из JS).

## 4. Мутационное тестирование (5/5 убито, лог `waves/RW-40/PM/int01-rust-wiring/mutations.log`)

| ID | Файл | Мутация | Baseline MD5 | Mutant MD5 | Exit | Убивший тест | Restored MD5 |
|---|---|---|---|---|---|---|---|
| MUT-1 | `rust/gui-spike/src/lib.rs` | `list_channel_threads` передаёт `false` вместо `archived` | `b2af030ca24749a6e4d278b5f1106a1f` | `194e631b634ab7ecbe488c617a37326a` | `101` | `ipc_wrapper_pins_and_threads_route_get_put_delete_and_archived_flag` | `b2af030ca24749a6e4d278b5f1106a1f` |
| MUT-2 | `rust/gui-spike/src/lib.rs` | `pin_channel_message` вызывает `unpin_channel_message` (`DELETE` вместо `PUT`) | `b2af030ca24749a6e4d278b5f1106a1f` | `c9d4f7a70c05ae1fb3214a95eee35c28` | `101` | `ipc_wrapper_pins_and_threads_route_get_put_delete_and_archived_flag` | `b2af030ca24749a6e4d278b5f1106a1f` |
| MUT-3 | `rust/gui-spike/src/lib.rs` | `request_upload_slots` передаёт `&[]` вместо `files` | `b2af030ca24749a6e4d278b5f1106a1f` | `fd5cf45a2c967d2766e613149be26153` | `101` | `ipc_wrapper_request_upload_slots_delegates_and_enforces_contract` | `b2af030ca24749a6e4d278b5f1106a1f` |
| MUT-4 | `rust/gui-spike/src/lib.rs` | `create_channel_invite` игнорирует `params` и передаёт `CreateInviteParams::default()` | `b2af030ca24749a6e4d278b5f1106a1f` | `6a8790be2eaf7785ac613a567647a793` | `101` | `ipc_wrapper_create_channel_invite_delegates_and_enforces_re_tables` | `b2af030ca24749a6e4d278b5f1106a1f` |
| MUT-5 | `rust/gui-spike/src/main.rs` | Удалена регистрация `list_channel_pins` из `tauri::generate_handler![...]` | `c192e36f7c3bc68637169f45cb53b203` | `47a0ce2957e625a36f06169703c54cc0` | `101` | `tauri_main_registers_and_wires_all_six_ipc_commands_via_token_or` | `c192e36f7c3bc68637169f45cb53b203` |

После восстановления всех мутаций контрольный прогон `ipc_wiring_contract` завершился с `5 passed; 0 failed` (`final_green_exit=0`).

## 5. Файлы и MD5

| Файл | MD5 | Статус |
|---|---|---|
| `rust/crates/discord-voice/src/lib.rs` | `f365261d5c7322594bb7370a8e213b38` | Проверен экспорт `pub mod devices;` (без изменений) |
| `rust/gui-spike/src/lib.rs` | `b2af030ca24749a6e4d278b5f1106a1f` | Добавлены `IPC_WIRING_COMMANDS` и 6 публичных IPC-обёрток |
| `rust/gui-spike/src/main.rs` | `c192e36f7c3bc68637169f45cb53b203` | Добавлены 6 `#[tauri::command]`, регистрация в `generate_handler!` и `TOKEN_COMMANDS` |
| `rust/gui-spike/tests/ipc_wiring_contract.rs` | `564bb6d670b3346a126c49d26ba1eb94` | Создан контрактный тест (5 тестов) |
| `waves/RW-40/PM/int01-rust-wiring/red.log` | `096bf0fe1af25f9b4a440e379a1c9abe` | Лог RED-фазы |
| `waves/RW-40/PM/int01-rust-wiring/green.log` | `5efd1ce690b423f319a76ae39562ce10` | Лог GREEN-фазы |
| `waves/RW-40/PM/int01-rust-wiring/clippy.log` | `08003686700b7232411823a26d46c43e` | Лог `cargo clippy -- -D warnings` |
| `waves/RW-40/PM/int01-rust-wiring/mutations.log` | `ab43cff1aaa1002bad5c8415c770894c` | Лог 5 мутаций и проверки восстановления MD5 |

## 6. Риски и замечания

- В `AttachmentError` (`rust/gui-spike/src/attachments.rs` — вне назначенных файлов карточки) отсутствуют отдельные варианты `Storage`/`MissingToken` (модуль использует `AttachmentError::Unauthorized` для пустого токена и `AttachmentError::Network` для сбоев окружения/сети). В `main.rs::request_upload_slots` вызов `token_or::<AttachmentError, _>` маппит сбой keyring в `AttachmentError::Network`, а отсутствие токена — в `AttachmentError::Unauthorized`.
- Аргумент списка файлов в `request_upload_slots` назван `files: Vec<AttachmentFileSpec>`, что строго удовлетворяет `tests/meta/ipc-path.test.mjs` (сегмент `files` не входит в запрещённый список имён путей, в отличие от `file`).
