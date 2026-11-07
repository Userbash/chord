# REPORT: RW-40 / bugfix/task-109-ipc-oracle-strict — Закрытие 3 замечаний P2 из ревью `waves/RW-40/PM/int06-notif-ipc/REVIEW.md`

- **Batch / Волна**: `RW-40` (G1 Rust Backend, Wave 8 Integration)
- **Карточка / Ветка**: `bugfix/task-109-ipc-oracle-strict` (от `unstable`)
- **Воркер**: Worker G1 Rust Backend (`term_5995562f-d4cc-4b57-adb7-46f28a5f1a93`, задача `task_b582da944ba5`, dispatch `ctx_21e97383655c`)
- **HEAD**: `4e240481e59ff4d49732ac8bd5ba08a006363748`
- **Назначенные файлы**:
  - `rust/gui-spike/src/main.rs`
  - `rust/gui-spike/tests/ipc_wiring_contract.rs`
  - `waves/RW-40/PM/task109-ipc-oracle/REPORT.md`
  - `waves/RW-40/PM/task109-ipc-oracle/red.log`
  - `waves/RW-40/PM/task109-ipc-oracle/green.log`
  - `waves/RW-40/PM/task109-ipc-oracle/mutations.log`

## 1. Что сделано (закрытие P2-1, P2-2, P2-3)

1. **P2-1 (расширение оракула `every_ui_invoke_command_in_js_sources_is_registered_in_tauri_main` до полных 38/38 UI IPC-команд)**:
   - В `rust/gui-spike/tests/ipc_wiring_contract.rs` функция `extract_ui_invoke_commands` расширена так, что помимо литеральных вызовов `invoke("...")`, `invoke('...')`, `invokeFn("...")`, `invokeFn('...')`, `invokeReaction("...")`, `invokeReaction('...')` она разбирает строковые литералы из констант `AUTH_COMMANDS` (`ui/features/auth.js`), `JOIN_COMMANDS` (`ui/features/join-server.js`), `NET_COMMANDS` и `READY_COMMAND` (`ui/app.js`).
   - В `every_ui_invoke_command_in_js_sources_is_registered_in_tauri_main` добавлена строгая проверка `assert_eq!(seen_ui_commands.len(), 38)` и точное совпадение отсортированного множества найденных в `ui/**/*.js` команд с каноническим списком `ALL_38_UI_IPC_COMMANDS` (38 команд), при этом для каждой из 38 команд проверяется регистрация в `tauri::generate_handler![...]` и наличие объявления `fn <cmd>(` в `rust/gui-spike/src/main.rs`.
2. **P2-2 (устранение тихого fallback на чужой канал в `ChannelOverrideInput::into_patch`)**:
   - В `rust/gui-spike/src/main.rs` удалён тихий fallback `.or_else(|| channel_overrides.into_values().next())` в `ChannelOverrideInput::into_patch`.
   - Сигнатура `ChannelOverrideInput::into_patch` изменена на `fn into_patch(self, channel_id: &str) -> Result<ChannelNotificationOverridePatch, NotificationError>`:
     - если конверт `channel_overrides` содержит запрошенный `channel_id`, возвращается `Ok(patch)`;
     - если конверт `channel_overrides` пуст (`channel_overrides.is_empty()`), возвращается `Ok(ChannelNotificationOverridePatch::default())`;
     - если непустой конверт `channel_overrides` не содержит запрошенный `channel_id` (содержит чужой канал), возвращается `Err(NotificationError::InvalidChannelId)`.
   - `ChannelOverrideInput::resolve` возвращает `Result<ChannelNotificationOverridePatch, NotificationError>` и пробрасывает ошибку через `?` в `update_channel_notification_override`.
   - Добавлены тесты в `rust/gui-spike/tests/ipc_wiring_contract.rs` (`channel_override_input_into_patch_rejects_foreign_channel_and_supports_override_field`) и поведенческий unit-тест в `rust/gui-spike/src/main.rs::wiring_tests` (`invoke_handler_registers_notifications_ipc_commands_and_resolves_channel_override_input`).
3. **P2-3 (поддержка поля `override` / `override_field` без дублирования `patch` в `update_channel_notification_override`)**:
   - В `ChannelOverrideInput` (`rust/gui-spike/src/main.rs`) добавлен вариант `OverrideField { #[serde(rename = "override", alias = "override_field", alias = "overrideField")] override_field: ChannelNotificationOverridePatch }`.
   - В параметры `#[tauri::command] async fn update_channel_notification_override` добавлены `r#override: Option<ChannelNotificationOverridePatch>` (который `tauri-macros` через `ident.unraw().to_lower_camel_case()` маппит на ключ `"override"` из `notifications.js:1610`) и `override_field: Option<ChannelNotificationOverridePatch>`, передаваемые в `ChannelOverrideInput::resolve`. Вызов команды только с полем `override` (без дублирования `patch`) теперь корректно применяет патч переопределения канала.

## 2. Red-прогон (до внесения правок в `src/main.rs`)

- **Команда**:
  ```bash
  (cd rust/gui-spike && cargo test -p discord-gui-spike --test ipc_wiring_contract -- --nocapture)
  ```
- **Код возврата**: `EXIT_CODE=101`
- **Лог**: `waves/RW-40/PM/task109-ipc-oracle/red.log`
  - `tests/ipc_wiring_contract.rs`: `FAILED. 7 passed; 2 failed` (`channel_override_input_into_patch_rejects_foreign_channel_and_supports_override_field` на строке `1456:5` и `notifications_ipc_commands_are_registered_and_delegate_to_lib` на строке `1311:5`).

## 3. Green-прогон и Targeted-проверки

- **Лог**: `waves/RW-40/PM/task109-ipc-oracle/green.log` (`EXIT_CODE=0`)
- **Проверенные команды и результаты**:
  1. `(cd rust/gui-spike && cargo test -p discord-gui-spike --test ipc_wiring_contract -- --nocapture)` → `9 passed; 0 failed` (`exit 0`).
  2. `(cd rust/gui-spike && cargo test -p discord-gui-spike --lib --bins --test notifications_contract -- --nocapture)` → `src/lib.rs`: `8 passed`, `src/main.rs`: `12 passed`, `tests/notifications_contract.rs`: `4 passed` (`exit 0`).
  3. `(cd rust/gui-spike && cargo clippy --offline -p discord-gui-spike --lib --bins --test ipc_wiring_contract -- -D warnings)` → 0 предупреждений (`exit 0`).
  4. `rustfmt --edition 2021 --check rust/gui-spike/src/main.rs rust/gui-spike/tests/ipc_wiring_contract.rs` → чисто (`exit 0`).
  5. `node --test tests/meta/ipc-path.test.mjs` → `12 pass / 0 fail` (`exit 0`).

## 3.1. Независимый оракул (PROCESS.md §4.2)

- `waves/RW-40/PM/int06-notif-ipc/REVIEW.md` (замечания P2-1, P2-2, P2-3) и `waves/RW-40/PARITY/MERGE-READINESS-13.md` (§3.4, §3.5: полный перечень 38 уникальных UI IPC-команд в `rust/gui-spike/ui/**/*.js`).
- Исходники `rust/gui-spike/ui/features/auth.js:15` (`AUTH_COMMANDS`), `rust/gui-spike/ui/features/join-server.js:27` (`JOIN_COMMANDS`), `rust/gui-spike/ui/app.js:93,300` (`NET_COMMANDS`, `READY_COMMAND`), `rust/gui-spike/ui/features/notifications.js:1547,1606`.
- `tests/meta/ipc-path.test.mjs` (`PLAT-11`): инвариант совпадения всех `#[tauri::command]` с `tauri::generate_handler![...]` и отсутствия файловых путей в аргументах IPC.

## 4. Мутационное тестирование (5/5 убито, лог `waves/RW-40/PM/task109-ipc-oracle/mutations.log`)

| ID | Файл | Мутация | Baseline MD5 | Mutant MD5 | Exit | Убивший тест | Restored MD5 |
|---|---|---|---|---|---:|---|---|
| MUT-1 | `rust/gui-spike/src/main.rs` | Удаление регистрации константной UI-команды `auth_status` (`AUTH_COMMANDS.status`, P2-1) из `tauri::generate_handler![...]` | `24bb37a8f233213ba7c59dc7b9fb1b22` | `82adf8a5b2c74cacf73efafbe74f2f7e` | `101` | `every_ui_invoke_command_in_js_sources_is_registered_in_tauri_main` | `24bb37a8f233213ba7c59dc7b9fb1b22` |
| MUT-2 | `rust/gui-spike/src/main.rs` | Удаление регистрации константной UI-команды `resolve_invite` (`JOIN_COMMANDS.resolve`, P2-1) из `tauri::generate_handler![...]` | `24bb37a8f233213ba7c59dc7b9fb1b22` | `613fff086e213f8aada06a1db5d43e2c` | `101` | `every_ui_invoke_command_in_js_sources_is_registered_in_tauri_main` | `24bb37a8f233213ba7c59dc7b9fb1b22` |
| MUT-3 | `rust/gui-spike/src/main.rs` | Возврат тихого fallback `.into_values().next()` в `ChannelOverrideInput::into_patch` вместо `Err(NotificationError::InvalidChannelId)` при чужом `channel_id` (P2-2) | `24bb37a8f233213ba7c59dc7b9fb1b22` | `5d011a2bd0eb1da7471dd53b2caae4df` | `101` | `wiring_tests::invoke_handler_registers_notifications_ipc_commands_and_resolves_channel_override_input`, `channel_override_input_into_patch_rejects_foreign_channel_and_supports_override_field` | `24bb37a8f233213ba7c59dc7b9fb1b22` |
| MUT-4 | `rust/gui-spike/src/main.rs` | Игнорирование полей `r#override` и `override_field` в `ChannelOverrideInput::resolve` (P2-3) | `24bb37a8f233213ba7c59dc7b9fb1b22` | `4d8f526bb63d3c4c8e302d2ba6841a0f` | `101` | `wiring_tests::invoke_handler_registers_notifications_ipc_commands_and_resolves_channel_override_input`, `channel_override_input_into_patch_rejects_foreign_channel_and_supports_override_field` | `24bb37a8f233213ba7c59dc7b9fb1b22` |
| MUT-5 | `rust/gui-spike/src/main.rs` | Подмена делегата `discord_gui_spike::update_channel_notification_override` на `Ok(serde_json::json!({}))` | `24bb37a8f233213ba7c59dc7b9fb1b22` | `1f5cd60bdbb25f58d258ffa0a1cde967` | `101` | `notifications_ipc_commands_are_registered_and_delegate_to_lib` | `24bb37a8f233213ba7c59dc7b9fb1b22` |

После восстановления baseline все 12 unit-тестов `src/main.rs::wiring_tests` и все 9 контрактных тестов `tests/ipc_wiring_contract.rs` прошли (`FINAL_GREEN_EXIT_CODE=0`).

## 5. Файлы и MD5

| Файл | MD5 | Статус |
|---|---|---|
| `rust/gui-spike/src/main.rs` | `24bb37a8f233213ba7c59dc7b9fb1b22` | Закрыты P2-2 (`into_patch` возвращает `Err(NotificationError::InvalidChannelId)` при чужом канале в конверте) и P2-3 (`r#override` / `override_field` / `#[serde(rename = "override")]`), расширен unit-тест в `wiring_tests` |
| `rust/gui-spike/tests/ipc_wiring_contract.rs` | `01929cf0e635a2e4ef946e416a61801f` | Закрыт P2-1 (разбор `AUTH_COMMANDS`, `JOIN_COMMANDS`, `NET_COMMANDS`, `READY_COMMAND`, проверка `38/38` UI IPC-команд) и добавлен контрактный тест `channel_override_input_into_patch_rejects_foreign_channel_and_supports_override_field` (9 тестов) |
| `waves/RW-40/PM/task109-ipc-oracle/red.log` | `ce62fc6c62dcc996d69f7a53068269db` | Лог RED-фазы (`EXIT_CODE=101`, 7 passed / 2 failed) |
| `waves/RW-40/PM/task109-ipc-oracle/green.log` | `cf3ac9478a0b6b800dded041a1b6da8a` | Лог GREEN-фазы (`ipc_wiring_contract` 9/9, `main.rs` 12/12, `clippy`, `rustfmt`, `ipc-path.test.mjs` 12/12, `EXIT_CODE=0`) |
| `waves/RW-40/PM/task109-ipc-oracle/mutations.log` | `fcc1274d02d76fd0e5796ee0f3a0d45a` | Лог 5 мутаций (`5/5 KILLED`, restore MD5, `FINAL_GREEN_EXIT_CODE=0`) |

## 6. Готовность к PR в `unstable` и риски

- Изменены исключительно разрешённые файлы карточки (`rust/gui-spike/src/main.rs`, `rust/gui-spike/tests/ipc_wiring_contract.rs` и артефакты в `waves/RW-40/PM/task109-ipc-oracle/`).
- Все три замечания P2-1, P2-2, P2-3 из `waves/RW-40/PM/int06-notif-ipc/REVIEW.md` полностью устранены; паритет `38/38` уникальных UI → Rust IPC-команд теперь проверяется автоматически тестом `every_ui_invoke_command_in_js_sources_is_registered_in_tauri_main`.
