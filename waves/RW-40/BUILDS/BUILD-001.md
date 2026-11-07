# Централизованная сборка QA — `BUILD-001` (ветка `testing`)

- **Номер сборки:** `BUILD-001`
- **Ветка сборки:** `testing` ([/home/sanya/orca/workspaces/project/testing](file:///home/sanya/orca/workspaces/project/testing))
- **Источник интеграции:** `unstable` ([/home/sanya/orca/workspaces/project/unstable](file:///home/sanya/orca/workspaces/project/unstable))
- **Дата сборки:** 2026-10-30
- **Статус сборки:** Передана в QA (`In Testing`)

## 1. Список включённых задач (прошедших Product Review + Technical Review DeepSeek + CI)

1. **`task-101..135` (Базовые 35 срезов RW-40):**
   - `L-02` Auth + Secure Storage (`0600`, DPAPI, Redacted debug)
   - `STUB-27` Gateway Dispatch (`MESSAGE_CREATE/UPDATE/DELETE`, `MESSAGE_REACTION_*`, `CHANNEL_PINS_UPDATE`, `THREAD_*`, `TYPING_START`)
   - `C-02` Older Messages Paging (`before` cursor, dedup, jump-to-message)
   - `M-07` Unread Badges & Read State (`POST /channels/{id}/messages/{id}/ack`, `MESSAGE_ACK`)
   - `M-08` Markdown & Mentions AST (`markdown.js`, XSS sanitizer, `<@id>`, `<#id>`, `<@&id>`)
   - `P-03` Typing Indicator (`typing-indicator.js`, 10s TTL, throttle)
   - `B2a/B2b` Attachments Upload + PUT (`attachments.rs`, `composer.js`, `INT-04-ATTACH-PUT-BACKEND`)
   - `AT-03` Attachments Render (`attachments-render.js`: Image/Video/Audio/File, `SPOILER_` toggle, Lightbox)
   - `M-10 / STUB-10` Pins & Threads (`pins_threads.rs`, `pins-threads.js`)
   - `STUB-05` Invites & Guild Join (`invites.rs`, `invite-modal.js`, `join-server.test.mjs`)
   - `STUB-16 / MB-03` Member Profile Popover & Role Grouping (`members.js`, AT-SPI `list item`)
   - `V-01 / V-06 / V-07` Voice Signaling, Stream Gateway Opcodes (`18..22`), Audio/Video Devices (`devices.rs`, `voice-devices-settings.js`)
   - `N-05-BE` Notifications REST Backend (`notifications.rs`, `notifications_contract.rs`)
   - `INT-05` IPC Aliases (`main.rs`, `ipc_wiring_contract.rs`)
2. **`task-105` ([PR-01](file:///home/sanya/orca/project/waves/RW-40/PR/PR-01-search-202-nsfw-spec.md)):**
   - Спецификация `SEARCH-202-NSFW-SPEC.md` (Pass 2, `PASS` от `DeepSeek 29B`).
3. **`task-106` ([PR-02](file:///home/sanya/orca/project/waves/RW-40/PR/PR-02-task-106-notif-ipc.md)):**
   - Tauri IPC-команды уведомлений `update_guild_notification_settings` и `update_channel_notification_override` в `rust/gui-spike/src/main.rs` (`PASS` от `DeepSeek 33A`).
4. **`task-104` ([PR-03](file:///home/sanya/orca/project/waves/RW-40/PR/PR-03-task-104-n05-ui-notifications.md)):**
   - UI-контроллер уведомлений `rust/gui-spike/ui/features/notifications.js` и `tests/unit/notifications-ui.test.mjs` (Pass 2, `17/17` тестов, `5/5` мутаций, `PASS` от `DeepSeek 31B`).

## 2. Инструкции по проверке для QA (в ветке `testing`)

1. Проверить мета-политику и целостность тестового контура:
   - `node --test tests/meta/*.test.mjs` (102/102)
2. Проверить компиляцию и типы TypeScript/JSDoc:
   - `npx tsc -p tsconfig.json`
   - `npx tsc -p rust/gui-spike/ui/tsconfig.json`
3. Проверить функциональные и регрессионные unit-наборы:
   - `node --test tests/unit/notifications-ui.test.mjs tests/unit/attachments-render.test.mjs tests/unit/voice-devices-settings.test.mjs tests/unit/gui-spike-dom.test.mjs tests/unit/gui-spike-features.test.mjs`
4. Проверить контрактные тесты Rust (`discord-gui-spike`, `discord-core`, `discord-voice`):
   - `cargo test --offline -p discord-gui-spike --test notifications_contract --test ipc_wiring_contract --test attachments_contract`

## 3. Тестовые данные и оракулы

- `fixtures/gateway/by-type/002-READY.json` (read-only: 58 записей `user_guild_settings`, 1328 `channel_overrides`, 17 `private_channels`, 60 `relationships`).
- `waves/RW-40/VOICE/stream-opcodes-fixtures.json` (read-only).

## 4. Известные ограничения сборки `BUILD-001`

- Задачи `task-107` (`INT-07-UI-APP-MOUNT`), `task-109` (`bugfix/task-109-ipc-oracle-strict`), `task-110` (`M-13-SEARCH-202-NSFW-BACKEND`), `task-111` (`M-13-SEARCH-202-NSFW-UI`), `task-112` (`DM-FRIENDS-BE`) и `task-113` (`DM-FRIENDS-UI`) находятся в разработке/ревью (`In Progress` / `Ready for Technical Review`) и войдут в `BUILD-002` после слияния их PR в `unstable`.
