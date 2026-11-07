# Pull Request #05: `bugfix/task-109-ipc-oracle-strict` → `unstable`

- **ID задачи:** `task-109` (`BUGFIX-IPC-ORACLE-STRICT`)
- **Отдел / Воркер:** G1 Backend (`codex`)
- **Исходная ветка:** `bugfix/task-109-ipc-oracle-strict` (создана от `unstable`)
- **Целевая ветка:** `unstable`
- **Статус:** `Merged to Unstable` (ветка `bugfix/task-109-ipc-oracle-strict` удалена после слияния)

## 1. Описание изменений

- Закрыты все три замечания `P2-1..P2-3` из технического ревью `PR #02` (`INT-06-NOTIFICATIONS-IPC-WIRING`):
  1. `P2-1`: В `rust/gui-spike/tests/ipc_wiring_contract.rs` оракул `extract_ui_invoke_commands` расширен на объектные карты `AUTH_COMMANDS`, `JOIN_COMMANDS`, `NET_COMMANDS` и константу `READY_COMMAND`, подтверждая 100% покрытие всех `38/38` UI IPC-команд (`ALL_38_UI_IPC_COMMANDS`).
  2. `P2-2`: В `rust/gui-spike/src/main.rs` (`ChannelOverrideInput::into_patch`) удалён тихий fallback `.into_values().next()`; при передаче конверта `channel_overrides` с чужим ID канала возвращается `Err(NotificationError::InvalidChannelId)`.
  3. `P2-3`: В `ChannelOverrideInput` и `#[tauri::command] fn update_channel_notification_override` добавлена поддержка поля `override` (`r#override: Option<ChannelNotificationOverridePatch>` / `#[serde(rename = "override")]`).

## 2. Проверки и ревью

- **Локальные проверки воркера:** `9/9` `ipc_wiring_contract`, `12/12` `discord-gui-spike` bin tests, `clippy -D warnings` `0`, `rustfmt` `0`, `5/5` мутаций `KILLED`.
- **Продуктовое ревью (PM):** **APPROVED** (`waves/RW-40/PM/task109-ipc-oracle/ACCEPT.md`).
- **Техническое ревью (`deepseek/deepseek-v4.1-flash` — Review 36A):** **PASS (`P0=0, P1=0, P2=0`)** (`waves/RW-40/PM/task109-ipc-oracle/REVIEW.md`).
