# REPORT: RW-40 / INT-05-IPC-ALIASES — Выравнивание F-1 IPC-алиасов UI ↔ Rust `main.rs` (`attachments_upload_slots`, `create_invite`, `pins_list`, `threads_list`)

- **Batch / Волна**: `RW-40` (Rust Integration Lane G1, Wave 8)
- **Карточка**: `INT-05-IPC-ALIASES`
- **Воркер**: Worker G1 Rust Integration (`term_098eecb0-44c9-4da1-b926-f71bdf459362`)
- **HEAD**: `4e240481e59ff4d49732ac8bd5ba08a006363748`
- **Назначенные файлы**:
  - `rust/gui-spike/src/main.rs`
  - `rust/gui-spike/tests/ipc_wiring_contract.rs`
  - `waves/RW-40/PM/int05-ipc-aliases/REPORT.md`

## 1. Что сделано

1. **Уточнение объёма F-1 через `orca orchestration ask`**:
   - В дереве `features/guild-settings.js` и команды `guild_reorder_*` / `guild_update_role` отсутствуют; открытая находка **F-1 (P1)** из `waves/RW-40/PARITY/MERGE-READINESS-11.md` §6 и `waves/RW-40/PARITY/MERGE-READINESS-12.md` §6 фиксирует расхождение 4 оставшихся имён IPC-вызовов фронтенда (`rust/gui-spike/ui/`) и `#[tauri::command]` в `rust/gui-spike/src/main.rs` (5-я команда `attachments_upload_put` уже подключена в `INT-04`).
   - По подтверждению координатора в `INT-05-IPC-ALIASES` реализованы и зарегистрированы все 4 реальных F-1 IPC-алиаса.
2. **Контрактные и модульные тесты (`rust/gui-spike/tests/ipc_wiring_contract.rs`, `rust/gui-spike/src/main.rs::wiring_tests`)**:
   - В `rust/gui-spike/tests/ipc_wiring_contract.rs` добавлены:
     - `tauri_main_registers_and_wires_f1_ui_ipc_aliases_via_token_or`: проверяет регистрацию `attachments_upload_slots`, `create_invite`, `pins_list`, `threads_list` в `tauri::generate_handler![...]` ровно по одному разу, чтение токена исключительно через `token_or::<...>(move || backend.token(), ...)` с сохранением доменных кодов ошибок (`AttachmentError::Network` / `Unauthorized`, `InviteError::Storage` / `MissingToken`, `PinsThreadsError::Storage` / `MissingToken`), отсутствие приёма токена из аргументов UI и отсутствие логирования (`println!`/`eprintln!`/`log::`), а также делегирование в `discord_gui_spike::request_upload_slots`, `discord_gui_spike::create_channel_invite`, `discord_gui_spike::list_channel_pins`, `discord_gui_spike::list_channel_threads` с дефолтом `archived.unwrap_or(false)` и обёрткой `UploadSlotsEnvelope { attachments }`.
     - `every_ui_invoke_command_in_js_sources_is_registered_in_tauri_main`: независимый оракул по исходникам `rust/gui-spike/ui/**/*.js`, извлекающий все литеральные вызовы `invoke("...")` / `invokeFn("...")` и проверяющий, что каждая UI-команда объявлена как `fn` в `src/main.rs` и зарегистрирована в `tauri::generate_handler![...]`.
   - В `rust/gui-spike/src/main.rs` (`mod wiring_tests`):
     - массив `TOKEN_COMMANDS` расширен с 19 до 23 сетевых команд (`+ create_invite`, `attachments_upload_slots`, `pins_list`, `threads_list`);
     - добавлен тест `invoke_handler_registers_f1_ui_ipc_aliases_and_wires_payload_contracts`, проверяющий регистрацию, делегирование и поведенческий разбор плоских/вложенных и `camelCase`/`snake_case` payload-структур (`CreateInviteParamsInput::resolve`, `UploadSlotSpecInput`, `UploadSlotsEnvelope`).
3. **Реализация 4 `#[tauri::command]` алиасов в `rust/gui-spike/src/main.rs`**:
   - `attachments_upload_slots(backend, channel_id, files: Vec<UploadSlotSpecInput>) -> Result<UploadSlotsEnvelope, AttachmentError>`: принимает как `snake_case` (`file_size`, `is_clip` из `composer.js:725`), так и `camelCase` (`fileSize`, `isClip`), делегирует в `discord_gui_spike::request_upload_slots(&api, &channel_id, &specs)` и возвращает `{ attachments: Vec<AttachmentUploadSlot> }`, который читает `composer.js:737` (`res?.attachments`). Имя аргумента `files` соблюдает инвариант `PLAT-11` (`tests/meta/ipc-path.test.mjs`).
   - `create_invite(backend, channel_id, params: Option<CreateInviteParamsInput>, max_age, max_uses, temporary, flags) -> Result<CreatedInvite, InviteError>`: принимает как плоский payload `{ channelId, maxAge, maxUses, temporary, flags }` из `invite-modal.js:370`, так и вложенный `{ channelId, params }` (в `camelCase` и `snake_case`), резолвит параметры через `CreateInviteParamsInput::resolve` и делегирует в `discord_gui_spike::create_channel_invite(&api, &channel_id, &resolved)`.
   - `pins_list(backend, channel_id) -> Result<Vec<PinnedMessage>, PinsThreadsError>`: алиас для вызова `app.js:1372`, делегирующий в `discord_gui_spike::list_channel_pins(&api, &channel_id)`.
   - `threads_list(backend, channel_id, archived: Option<bool>) -> Result<Vec<ThreadSummary>, PinsThreadsError>`: алиас для вызова `app.js:1378` (без `archived` по умолчанию запрашивает активные ветки `archived = false`), делегирующий в `discord_gui_spike::list_channel_threads(&api, &channel_id, archived.unwrap_or(false))`.
   - Все 4 команды зарегистрированы в `tauri::generate_handler![...]`.

## 2. Red-прогон (до реализации команд в `main.rs`)

- **Команда**:
  ```bash
  (cd rust/gui-spike && cargo test --offline -p discord-gui-spike --test ipc_wiring_contract --lib --bins --no-fail-fast)
  ```
- **Код возврата**: `EXIT_CODE=101` (зафиксирован внутри лога)
- **Лог**: `waves/RW-40/PM/int05-ipc-aliases/red.log`
  - `src/lib.rs`: `8 passed; 0 failed`
  - `src/main.rs`: `FAILED. 9 passed; 2 failed` (`wiring_tests::every_network_command_reads_token_through_one_helper`, `wiring_tests::invoke_handler_registers_f1_ui_ipc_aliases_and_wires_payload_contracts`)
  - `tests/ipc_wiring_contract.rs`: `FAILED. 5 passed; 2 failed` (`tauri_main_registers_and_wires_f1_ui_ipc_aliases_via_token_or`, `every_ui_invoke_command_in_js_sources_is_registered_in_tauri_main`)

## 3. Green-прогон, Clippy, Rustfmt и Meta-тест

- **Лог**: `waves/RW-40/PM/int05-ipc-aliases/green.log` (`EXIT_CODE=0`)
- **Проверенные команды и результаты**:
  1. `(cd rust/gui-spike && cargo test --offline -p discord-gui-spike --test ipc_wiring_contract --lib --bins)` → `CARGO_TEST_EXIT_CODE=0`:
     - `src/lib.rs`: `8 passed; 0 failed`
     - `src/main.rs`: `11 passed; 0 failed`
     - `tests/ipc_wiring_contract.rs`: `7 passed; 0 failed`
  2. `(cd rust/gui-spike && cargo clippy --offline -p discord-gui-spike --lib --bins --test ipc_wiring_contract -- -D warnings)` → `CARGO_CLIPPY_EXIT_CODE=0` (0 предупреждений).
  3. `rustfmt --edition 2021 --check rust/gui-spike/src/main.rs rust/gui-spike/tests/ipc_wiring_contract.rs` → `RUSTFMT_EXIT_CODE=0`.
  4. `node --test tests/meta/ipc-path.test.mjs` → `IPC_PATH_META_EXIT_CODE=0` (`12 pass / 0 fail`).

## 3.1. Oracle (независимые источники контракта, PROCESS.md §4.2)

- **Исходники UI (`rust/gui-spike/ui/`) и аудит паритета (`waves/RW-40/PARITY/MERGE-READINESS-11.md` §6, `MERGE-READINESS-12.md` §6, находка F-1)**:
  - `rust/gui-spike/ui/features/composer.js:725-738`: вызывает `invokeFn("attachments_upload_slots", { channelId, files: [{ filename, file_size, id, is_clip }] })` и читает слот из `res?.attachments`.
  - `rust/gui-spike/ui/features/invite-modal.js:370-376`: вызывает `invoke("create_invite", { channelId, maxAge, maxUses, temporary, flags })`.
  - `rust/gui-spike/ui/app.js:1372,1378`: вызывает `invoke("pins_list", { channelId })` и `invoke("threads_list", { channelId })` (без поля `archived`).
- **Инвариант безопасности IPC (`tests/meta/ipc-path.test.mjs`, `PLAT-11`) и единого чтения токена (`G1-02`/`SEC-02`)**:
  - Сигнатуры `#[tauri::command]` не принимают файловых путей/дескрипторов и совпадают с `tauri::generate_handler![...]`; токен читается исключительно из `AuthBackend` через `token_or`.

## 4. Мутационное тестирование (5/5 убито, лог `waves/RW-40/PM/int05-ipc-aliases/mutations.log`)

| ID | Файл | Мутация | Baseline MD5 | Mutant MD5 | Exit | Убивший тест | Restored MD5 |
|---|---|---|---|---|---|---|---|
| MUT-1 | `rust/gui-spike/src/main.rs` | `CreateInviteParamsInput::resolve` игнорирует плоский `max_age` (`nested.max_age.unwrap_or(defaults.max_age)`) | `a9b2c9164e121deb3497e5cfac195ad6` | `6e7e6792d0ab4fde03fa35d0281c52ec` | `101` | `wiring_tests::invoke_handler_registers_f1_ui_ipc_aliases_and_wires_payload_contracts` | `a9b2c9164e121deb3497e5cfac195ad6` |
| MUT-2 | `rust/gui-spike/src/main.rs` | `From<UploadSlotSpecInput> for AttachmentFileSpec` обнуляет `file_size: 0` | `a9b2c9164e121deb3497e5cfac195ad6` | `3be1c5ef4713f67f8bf309a2671fc97e` | `101` | `wiring_tests::invoke_handler_registers_f1_ui_ipc_aliases_and_wires_payload_contracts` | `a9b2c9164e121deb3497e5cfac195ad6` |
| MUT-3 | `rust/gui-spike/src/main.rs` | `CreateInviteParamsInput::resolve` теряет `flags` (`flags: None`) | `a9b2c9164e121deb3497e5cfac195ad6` | `b42a59ab4a3d5e8d1fc5b490cb1612a5` | `101` | `wiring_tests::invoke_handler_registers_f1_ui_ipc_aliases_and_wires_payload_contracts` | `a9b2c9164e121deb3497e5cfac195ad6` |
| MUT-4 | `rust/gui-spike/src/main.rs` | `threads_list` подставляет `archived.unwrap_or(true)` вместо `false` | `a9b2c9164e121deb3497e5cfac195ad6` | `178a8c87e5a23a4c2bd19da187d50132` | `101` | `tauri_main_registers_and_wires_f1_ui_ipc_aliases_via_token_or`, `wiring_tests::invoke_handler_registers_f1_ui_ipc_aliases_and_wires_payload_contracts` | `a9b2c9164e121deb3497e5cfac195ad6` |
| MUT-5 | `rust/gui-spike/src/main.rs` | Удалена регистрация `attachments_upload_slots` из `tauri::generate_handler![...]` | `a9b2c9164e121deb3497e5cfac195ad6` | `490c35dee80a5df736f57b11cd6cc7d5` | `101` | `every_ui_invoke_command_in_js_sources_is_registered_in_tauri_main`, `tauri_main_registers_and_wires_f1_ui_ipc_aliases_via_token_or`, `wiring_tests::invoke_handler_registers_f1_ui_ipc_aliases_and_wires_payload_contracts` | `a9b2c9164e121deb3497e5cfac195ad6` |

После восстановления всех 5 мутаций финальный прогон в `mutations.log` завершился с `8 + 11 + 7 passed; 0 failed` (`final_green_exit=0`).

## 5. Файлы и MD5

| Файл | MD5 | Статус |
|---|---|---|
| `rust/gui-spike/src/main.rs` | `a9b2c9164e121deb3497e5cfac195ad6` | Добавлены 4 `#[tauri::command]` F-1 алиаса, регистрация в `generate_handler!`, `TOKEN_COMMANDS` (23) и unit-тест в `wiring_tests` |
| `rust/gui-spike/tests/ipc_wiring_contract.rs` | `2c6efc012048d839b26b95cffdc18edc` | Добавлены 2 контрактных теста F-1 алиасов и полного покрытия UI `invoke` (всего 7 тестов) |
| `waves/RW-40/PM/int05-ipc-aliases/red.log` | `dbbb671c86bd24f8a2ccda9555b23c50` | Лог RED-фазы (`EXIT_CODE=101`) |
| `waves/RW-40/PM/int05-ipc-aliases/green.log` | `d9c7ed611552f6b37e25b54dc148fdb3` | Лог GREEN-фазы (`cargo test`, `clippy`, `rustfmt`, `ipc-path.test.mjs`, `EXIT_CODE=0`) |
| `waves/RW-40/PM/int05-ipc-aliases/mutations.log` | `e751e9b279d82041e912615a7f63ea4a` | Лог 5 мутаций (`5/5 KILLED`, restore MD5, `final_green_exit=0`) |

## 6. Риски и замечания

- Канонические команды `request_upload_slots`, `create_channel_invite`, `list_channel_pins`, `list_channel_threads` сохранены без изменений для обратной совместимости с контрактом `INT-01-RUST-WIRING`; новые команды `attachments_upload_slots`, `create_invite`, `pins_list`, `threads_list` работают как точные Tauri IPC-алиасы над теми же функциями крейта.
- Поскольку `main.rs` изменён этой карточкой поверх `INT-01` и `B2b`, при финальной приёмке волны Team Lead может обновить контрольную сумму `main.rs` и `ipc_wiring_contract.rs` в сводном `ACCEPT.md` (замечание `P1-1` из `QA-AUDIT-WAVE8.md`).
