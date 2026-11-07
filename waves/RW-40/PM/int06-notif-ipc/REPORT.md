# REPORT: RW-40 / INT-06-NOTIFICATIONS-IPC-WIRING — Подключение Tauri IPC-команд уведомлений (`update_guild_notification_settings`, `update_channel_notification_override`, F-5)

- **Batch / Волна**: `RW-40` (G1 Rust Backend, Wave 8 Integration)
- **Карточка**: `INT-06-NOTIFICATIONS-IPC-WIRING`
- **Воркер**: Worker G1 Rust Backend (`term_8a777b7a-09d7-4312-a839-2ef477d7de12`, задача `task_903207c50602`)
- **HEAD**: `4e240481e59ff4d49732ac8bd5ba08a006363748`
- **Назначенные файлы**:
  - `rust/gui-spike/src/main.rs`
  - `rust/gui-spike/tests/ipc_wiring_contract.rs`
  - `waves/RW-40/PM/int06-notif-ipc/REPORT.md`
  - `waves/RW-40/PM/int06-notif-ipc/red.log`
  - `waves/RW-40/PM/int06-notif-ipc/green.log`
  - `waves/RW-40/PM/int06-notif-ipc/mutations.log`

## 1. Что сделано

1. **Уточнение сигнатур через `orca orchestration ask`**:
   - Согласована адаптация `#[tauri::command]` обёрток в `rust/gui-spike/src/main.rs` и теста `notifications_ipc_commands_are_registered_and_delegate_to_lib()` в `rust/gui-spike/tests/ipc_wiring_contract.rs` под фактические сигнатуры `rust/gui-spike/src/lib.rs` (`backend: tauri::State<'_, Arc<AuthBackend>>`, `token_or::<NotificationError, _>(move || backend.token(), || NotificationError::Storage, || NotificationError::MissingToken)`, `DiscordNotifications::new(&token)?`, `discord_gui_spike::update_guild_notification_settings` и `discord_gui_spike::update_channel_notification_override`) и структуру аргументов `rust/gui-spike/ui/features/notifications.js:1335,1389`.
2. **Контрактные тесты (`rust/gui-spike/tests/ipc_wiring_contract.rs`, `rust/gui-spike/src/main.rs::wiring_tests`)**:
   - В `rust/gui-spike/tests/ipc_wiring_contract.rs` добавлен тест `notifications_ipc_commands_are_registered_and_delegate_to_lib()`, проверяющий:
     - регистрацию `update_guild_notification_settings` и `update_channel_notification_override` в `tauri::generate_handler![...]` ровно по одному разу;
     - сигнатуры обеих команд (`backend: tauri::State<'_, Arc<AuthBackend>>`, `guild_id: String`, `channel_id: String`, `patch`/`settings`/`override_patch`/`payload` и возврат `Result<serde_json::Value, NotificationError>`);
     - чтение токена исключительно через `token_or::<NotificationError, _>` с сохранением доменных кодов `NotificationError::Storage` и `NotificationError::MissingToken`, отсутствие приёма токена из фронтенда и отсутствие логирования (`println!`/`eprintln!`/`log::`);
     - делегирование в `discord_gui_spike::update_guild_notification_settings(&api, &token, &guild_id, &resolved).await` и `discord_gui_spike::update_channel_notification_override(&api, &token, &guild_id, &channel_id, &resolved).await`;
     - поведенческий контракт вызовов `update_guild_notification_settings` и `update_channel_notification_override` на `FakeNotificationTransport` (`PATCH /users/@me/guilds/{guild_id}/settings`, предсетевая валидация `guild_id`/`channel_id`, редактирование секрета в `Debug`).
   - Расширен тест `every_ui_invoke_command_in_js_sources_is_registered_in_tauri_main()`:
     - вместо фиксированного списка из 10 файлов рекурсивно обходит все файлы `rust/gui-spike/ui/**/*.js` (32 файла, включая `ui/features/notifications.js` и `ui/features/reactions.js`);
     - извлекает вызовы `invoke("...")`, `invoke('...')`, `invokeFn("...")`, `invokeFn('...')`, `invokeReaction("...")`, `invokeReaction('...')` и проверяет регистрацию каждого имени в `tauri::generate_handler![...]` и объявление `fn <cmd>(` в `src/main.rs`.
   - В `rust/gui-spike/src/main.rs` (`mod wiring_tests`) массив `TOKEN_COMMANDS` расширен с 23 до 25 команд и добавлен тест `invoke_handler_registers_notifications_ipc_commands_and_resolves_channel_override_input`.
3. **Реализация Tauri IPC-команд в `rust/gui-spike/src/main.rs` (закрытие F-5 из `MERGE-READINESS-13.md`)**:
   - `update_guild_notification_settings(backend, guild_id, patch, settings, payload) -> Result<serde_json::Value, NotificationError>`: читает токен через `token_or`, создаёт `DiscordNotifications::new(&token)?`, принимает `patch`/`settings` из `notifications.js:1335` или `payload` и делегирует в `discord_gui_spike::update_guild_notification_settings(&api, &token, &guild_id, &resolved).await`.
   - `update_channel_notification_override(backend, guild_id, channel_id, override_patch, patch, payload) -> Result<serde_json::Value, NotificationError>`: читает токен через `token_or`, создаёт `DiscordNotifications::new(&token)?`, разбирает как конверт `{ channel_overrides: { [channelId]: overridePatch } }` из `notifications.js:1389`, так и прямой `ChannelNotificationOverridePatch` (`override_patch`/`payload`) через `ChannelOverrideInput::resolve`, и делегирует в `discord_gui_spike::update_channel_notification_override(&api, &token, &guild_id, &channel_id, &resolved).await`.
   - Обе команды включены в `tauri::generate_handler![...]`.

## 2. Red-прогон (до подключения команд в `main.rs`)

- **Команда**:
  ```bash
  (cd rust/gui-spike && cargo test -p discord-gui-spike --test ipc_wiring_contract -- --nocapture)
  ```
- **Код возврата**: `EXIT_CODE=101`
- **Лог**: `waves/RW-40/PM/int06-notif-ipc/red.log`
  - `tests/ipc_wiring_contract.rs`: `FAILED. 6 passed; 2 failed` (`notifications_ipc_commands_are_registered_and_delegate_to_lib`, `every_ui_invoke_command_in_js_sources_is_registered_in_tauri_main`)

## 3. Green-прогон и Targeted-проверки

- **Лог**: `waves/RW-40/PM/int06-notif-ipc/green.log` (`EXIT_CODE=0`)
- **Проверенные команды и результаты**:
  1. `(cd rust/gui-spike && cargo test -p discord-gui-spike --test ipc_wiring_contract -- --nocapture)` → `8 passed; 0 failed` (`exit 0`).
  2. `(cd rust/gui-spike && cargo test -p discord-gui-spike --lib notifications -- --nocapture)` → `0 failed` (`exit 0`).
  3. `(cd rust/gui-spike && cargo test -p discord-gui-spike --lib --bins --test notifications_contract -- --nocapture)` → `src/lib.rs`: `8 passed`, `src/main.rs`: `12 passed`, `tests/notifications_contract.rs`: `4 passed` (`exit 0`).
  4. `(cd rust/gui-spike && cargo clippy --offline -p discord-gui-spike --lib --bins --test ipc_wiring_contract -- -D warnings)` → 0 предупреждений (`exit 0`).
  5. `rustfmt --edition 2021 --check rust/gui-spike/src/main.rs rust/gui-spike/tests/ipc_wiring_contract.rs` → чисто (`exit 0`).
  6. `node --test tests/meta/ipc-path.test.mjs` → `12 pass / 0 fail` (`exit 0`).

## 3.1. Независимый оракул (PROCESS.md §4.2)

- `waves/RW-40/PARITY/MERGE-READINESS-13.md` (§3.4, §3.5, §7 F-5): вызовы `update_guild_notification_settings` и `update_channel_notification_override` в `rust/gui-spike/ui/features/notifications.js:1335,1389`.
- `rust/gui-spike/src/notifications.rs` и `rust/gui-spike/src/lib.rs:232-261`: публичные обёртки крейта и доменный контракт `PATCH /users/@me/guilds/{guild_id}/settings`.
- `tests/meta/ipc-path.test.mjs` (`PLAT-11`): совпадение множества `#[tauri::command]` с `tauri::generate_handler![...]` и отсутствие файловых путей в аргументах IPC.

## 4. Мутационное тестирование (5/5 убито, лог `waves/RW-40/PM/int06-notif-ipc/mutations.log`)

| ID | Файл | Мутация | Baseline MD5 | Mutant MD5 | Exit | Убивший тест | Restored MD5 |
|---|---|---|---|---|---:|---|---|
| MUT-1 | `rust/gui-spike/src/main.rs` | Удаление регистрации `update_guild_notification_settings` из `tauri::generate_handler![...]` | `ae573539a9b8a8cc6e355c3975edaff0` | `9f0ceede9031e92ed53ff1529581f5f2` | `101` | `notifications_ipc_commands_are_registered_and_delegate_to_lib`, `every_ui_invoke_command_in_js_sources_is_registered_in_tauri_main` | `ae573539a9b8a8cc6e355c3975edaff0` |
| MUT-2 | `rust/gui-spike/src/main.rs` | Удаление регистрации `update_channel_notification_override` из `tauri::generate_handler![...]` | `ae573539a9b8a8cc6e355c3975edaff0` | `97d70510e94493b12a6abbe193ba9cee` | `101` | `notifications_ipc_commands_are_registered_and_delegate_to_lib`, `every_ui_invoke_command_in_js_sources_is_registered_in_tauri_main` | `ae573539a9b8a8cc6e355c3975edaff0` |
| MUT-3 | `rust/gui-spike/src/main.rs` | Удаление вызова `token_or` в `update_guild_notification_settings` (прямой `blocking(move || backend.token())`) | `ae573539a9b8a8cc6e355c3975edaff0` | `ecd31872d3c561ed60e6f7614264731e` | `101` | `notifications_ipc_commands_are_registered_and_delegate_to_lib` | `ae573539a9b8a8cc6e355c3975edaff0` |
| MUT-4 | `rust/gui-spike/src/main.rs` | Подмена делегата `discord_gui_spike::update_guild_notification_settings` на `Ok(serde_json::json!({}))` | `ae573539a9b8a8cc6e355c3975edaff0` | `98def869a74a2f6eebff8f1e38c2afc5` | `101` | `notifications_ipc_commands_are_registered_and_delegate_to_lib` | `ae573539a9b8a8cc6e355c3975edaff0` |
| MUT-5 | `rust/gui-spike/src/main.rs` | Подмена делегата `discord_gui_spike::update_channel_notification_override` на `Ok(serde_json::json!({}))` | `ae573539a9b8a8cc6e355c3975edaff0` | `59fe38656d478068f18a0c4c5a2c7cd7` | `101` | `notifications_ipc_commands_are_registered_and_delegate_to_lib` | `ae573539a9b8a8cc6e355c3975edaff0` |

После восстановления baseline все 8 тестов `ipc_wiring_contract` прошли (`FINAL_GREEN_EXIT_CODE=0`).

## 5. Файлы и MD5

| Файл | MD5 | Статус |
|---|---|---|
| `rust/gui-spike/src/main.rs` | `ae573539a9b8a8cc6e355c3975edaff0` | Добавлены `#[tauri::command]` `update_guild_notification_settings` и `update_channel_notification_override`, `ChannelOverrideInput`, регистрация в `generate_handler!`, `TOKEN_COMMANDS` (25) и тест в `wiring_tests` |
| `rust/gui-spike/tests/ipc_wiring_contract.rs` | `159053d11b700daba7a0ecdc863fb7fc` | Добавлен `notifications_ipc_commands_are_registered_and_delegate_to_lib`, обход всех `ui/**/*.js` в `every_ui_invoke_command_in_js_sources_is_registered_in_tauri_main` (8 тестов) |
| `waves/RW-40/PM/int06-notif-ipc/red.log` | `1876c2d4ec7cbb2d4c2a82433dea96ad` | Лог RED-фазы (`EXIT_CODE=101`) |
| `waves/RW-40/PM/int06-notif-ipc/green.log` | `375f8e323295df42ac1440f0d8477b85` | Лог GREEN-фазы (`cargo test`, `clippy`, `rustfmt`, `ipc-path.test.mjs`, `EXIT_CODE=0`) |
| `waves/RW-40/PM/int06-notif-ipc/mutations.log` | `5e06bb03d823bc184cdc5a046f8091d2` | Лог 5 мутаций (`5/5 KILLED`, restore MD5, `FINAL_GREEN_EXIT_CODE=0`) |

## 6. Риски и замечания

- Изменены исключительно разрешённые файлы карточки (`rust/gui-spike/src/main.rs`, `rust/gui-spike/tests/ipc_wiring_contract.rs` и артефакты в `waves/RW-40/PM/int06-notif-ipc/`).
- По `MERGE-READINESS-13.md` §3.4 и §7 с подключением `update_guild_notification_settings` и `update_channel_notification_override` паритет уникальных UI → Rust IPC-команд доведён с 36/38 до **38/38 (100 %)**, а оракул `every_ui_invoke_command_in_js_sources_is_registered_in_tauri_main` теперь сканирует все 32 файла `rust/gui-spike/ui/**/*.js`.
