# Приёмка: task-109 — Строгий оракул 38/38 IPC-команд и закрытие P2-1..P2-3 из INT-06

- **Ветка:** `bugfix/task-109-ipc-oracle-strict` → `unstable`
- **Pull Request:** `waves/RW-40/PR/PR-05-task-109-ipc-oracle-strict.md`
- **Продуктовое ревью (PM):** **APPROVED**
  - Проверено закрытие всех трёх замечаний `P2-1..P2-3` из `waves/RW-40/PM/int06-notif-ipc/REVIEW.md`:
    - `P2-1`: Оракул `extract_ui_invoke_commands` в `rust/gui-spike/tests/ipc_wiring_contract.rs` разбирает `AUTH_COMMANDS`, `JOIN_COMMANDS`, `NET_COMMANDS`, `READY_COMMAND` и сверяет все `38/38` UI IPC-команд с `generate_handler![]` и `fn <cmd>(` в `main.rs`.
    - `P2-2`: Удалён тихий fallback `.into_values().next()` в `ChannelOverrideInput::into_patch`; при непустом конверте без запрошенного `channel_id` возвращается `Err(NotificationError::InvalidChannelId)`.
    - `P2-3`: Команда `update_channel_notification_override` и `ChannelOverrideInput` принимают поле `override` (`r#override` / `#[serde(rename = "override")]`).
- **Техническое ревью (DeepSeek Review 36A — `REVIEW.md`):** **PASS (P0=0, P1=0, P2=0, P3=4)**
  - `cargo test --offline -p discord-gui-spike --test ipc_wiring_contract` → `9 passed; 0 failed`
  - `cargo test --offline -p discord-gui-spike --bin discord-gui-spike` → `12 passed; 0 failed`
  - `cargo clippy --offline -p discord-gui-spike --lib --bins --test ipc_wiring_contract -- -D warnings` → `exit 0`
  - `node --test tests/meta/test-policy.test.mjs` → `17 pass / 0 fail`
  - Мутации: `5/5 KILLED` с восстановлением baseline MD5.

## Принятые файлы и MD5

| Файл | MD5 |
|---|---|
| `rust/gui-spike/src/main.rs` | `24bb37a8f233213ba7c59dc7b9fb1b22` |
| `rust/gui-spike/tests/ipc_wiring_contract.rs` | `01929cf0e635a2e4ef946e416a61801f` |
| `waves/RW-40/PM/task109-ipc-oracle/red.log` | `ce62fc6c62dcc996d69f7a53068269db` |
| `waves/RW-40/PM/task109-ipc-oracle/green.log` | `cf3ac9478a0b6b800dded041a1b6da8a` |
| `waves/RW-40/PM/task109-ipc-oracle/mutations.log` | `fcc1274d02d76fd0e5796ee0f3a0d45a` |

- **Статус задачи:** `Merged to Unstable` (ветка `bugfix/task-109-ipc-oracle-strict` удалена после слияния).
