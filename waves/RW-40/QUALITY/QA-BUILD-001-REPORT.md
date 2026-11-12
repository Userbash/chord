# Отчёт централизованного тестирования QA — Сборка `BUILD-001`

- **Задача QA:** `qa/task-116-qa-build-001` (`task_58e11f39a986`, `ctx_a6c3ea5e6bde`)
- **Роль:** Тестировщик QA (`G4 QA`)
- **Рабочая ветка:** `qa/task-116-qa-build-001` (от ветки централизованной сборки `testing`)
- **Номер сборки:** `BUILD-001` ([waves/RW-40/BUILDS/BUILD-001.md](file:///home/user/orca/workspaces/project/qa-task-116-qa-build-001/waves/RW-40/BUILDS/BUILD-001.md))
- **Дата и время тестирования:** 2026-10-30, 20:17–20:20 +05:00
- **Финальный вердикт по сборке `BUILD-001`:** **QA Passed** (`P0 = 0`, `P1 = 0`, `P2 = 3` — неблокирующие замечания, учтённые в бэклоге)

---

## 1. Окружение и тестовые оракулы

| Параметр | Значение |
|---|---|
| **ОС / Ядро** | `Linux 7.3.0-5-cachyos #1 SMP PREEMPT_DYNAMIC Fri, 23 Oct 2026 14:38:02 +0000 x86_64` |
| **Git HEAD** | `4e240481e59ff4d49732ac8bd5ba08a006363748` (`RW-40: integrate Discord GUI spike, dispatch, paging and QA evidence`) |
| **Ветка сборки / QA** | `testing` → `qa/task-116-qa-build-001` (побайтово идентична `testing`) |
| **Node.js** | `v26.13.0` |
| **TypeScript (`tsc`)** | `Version 7.0.2` |
| **Rust (`rustc` / `cargo`)** | `rustc 1.99.0 (b940084d7 2026-09-28)` / `cargo 1.99.0 (5f94df478 2026-08-27)` |
| **Оракул Gateway READY** | [fixtures/gateway/by-type/002-READY.json](file:///home/user/orca/workspaces/project/qa-task-116-qa-build-001/fixtures/gateway/by-type/002-READY.json) (`md5: a1a8fd5662be88108dd25ac619a377f2`, read-only: 58 `user_guild_settings`, 1328 `channel_overrides`, 17 `private_channels`, 60 `relationships`) |
| **Оракул Voice Stream** | [waves/RW-40/VOICE/stream-opcodes-fixtures.json](file:///home/user/orca/workspaces/project/qa-task-116-qa-build-001/waves/RW-40/VOICE/stream-opcodes-fixtures.json) (`md5: 1a44becfab3c6fd4170346635f80830b`, read-only) |

---

## 2. Список проверенных задач и PR в составе `BUILD-001`

| Задача / Пакет | PR / Артефакт | Содержание проверки в `BUILD-001` | Статус |
|---|---|---|---|
| **`task-101..103` (и базовый пакет `task-101..135`, 35 срезов RW-40)** | Базовый интеграционный пакет `unstable` | `L-02` Auth + Secure Storage, `STUB-27` Gateway Dispatch, `C-02` Older Messages Paging, `M-07` Unread Badges & Read State, `M-08` Markdown & Mentions AST, `P-03` Typing Indicator, `B2a/B2b` Attachments Upload + PUT (`attachments.rs`, `composer.js`), `AT-03` Attachments Render (`chat.js`, `chat.css`), `M-10/STUB-10` Pins & Threads, `STUB-05` Invites & Guild Join, `STUB-16/MB-03` Member Profile Popover & Role Grouping, `V-01/V-06/V-07` Voice Signaling & Audio/Video Devices (`voice-devices-settings.js`), `N-05-BE` Notifications REST (`notifications.rs`), `INT-05` IPC Aliases (`main.rs`). | **PASSED** |
| **`task-104` (`N-05-UI-NOTIFICATIONS` Pass 2)** | [PR-03](file:///home/user/orca/workspaces/project/qa-task-116-qa-build-001/waves/RW-40/PR/PR-03-task-104-n05-ui-notifications.md) | Контроллер UI уведомлений [notifications.js](file:///home/user/orca/workspaces/project/qa-task-116-qa-build-001/rust/gui-spike/ui/features/notifications.js) (`md5: 001ff70a7538378d16d52fd3b6b10117`) и unit-набор [notifications-ui.test.mjs](file:///home/user/orca/workspaces/project/qa-task-116-qa-build-001/tests/unit/notifications-ui.test.mjs) (`md5: 22238cac9b6e56b5ba9aa3f33c4484f7`): каскад правил, индикаторы `#guild-rail`/`#channel-tree`, модалка `#guild-notif-modal`, контекстное меню `#channel-notif-menu`, focus trap, таймер истечения mute, 429 cooldown. | **PASSED** (`17/17`) |
| **`task-105` (`SEARCH-202-NSFW-SPEC` Pass 2)** | [PR-01](file:///home/user/orca/workspaces/project/qa-task-116-qa-build-001/waves/RW-40/PR/PR-01-search-202-nsfw-spec.md) | Дизайн-спецификация [SEARCH-202-NSFW-SPEC.md](file:///home/user/orca/workspaces/project/qa-task-116-qa-build-001/waves/RW-40/DESIGN/SEARCH-202-NSFW-SPEC.md) (`md5: 4d07bc2fa8ecaa964e99f546096b6bd8`), привязка 60/60 CSS-токенов и согласованность с мета-политиками. | **PASSED** |
| **`task-106` (`INT-06-NOTIFICATIONS-IPC-WIRING` / `F-5`)** | [PR-02](file:///home/user/orca/workspaces/project/qa-task-116-qa-build-001/waves/RW-40/PR/PR-02-task-106-notif-ipc.md) | Подключение Tauri IPC-команд `update_guild_notification_settings` и `update_channel_notification_override` в [main.rs](file:///home/user/orca/workspaces/project/qa-task-116-qa-build-001/rust/gui-spike/src/main.rs) (`md5: ae573539a9b8a8cc6e355c3975edaff0`) и проверка в [ipc_wiring_contract.rs](file:///home/user/orca/workspaces/project/qa-task-116-qa-build-001/rust/gui-spike/tests/ipc_wiring_contract.rs) (`md5: 159053d11b700daba7a0ecdc863fb7fc`). | **PASSED** (`8/8`) |

---

## 3. Шаги воспроизведения, ожидаемый и фактический результат

### Шаг 1. Мета-политика и целостность тестового контура

- **Команда:**
  ```sh
  node --test tests/meta/*.test.mjs
  ```
- **Ожидаемый результат:** `102/102` тестов проходят успешно (`fail 0`, `skipped 0`), код возврата `0`.
- **Фактический результат:** `102/102` тестов прошли успешно (`17 suites`, `fail 0`, `skipped 0`, `1328.92 ms`), код возврата `0`.
- **Вывод лога:**
  ```text
  ✔ backlog-sync: бэклог переноса ↔ docs ↔ README/TASKS (21.89543ms)
  ✔ QA-M2-04: CI browser-job (pinned Chromium, без live) (16.966178ms)
  ✔ DP-01: breakpoints SPEC §5.1 (8.187674ms)
  ✔ docs-sync: README ↔ команды CLI (8.526644ms)
  ✔ M2 e2e: обязательный browser-набор (78.41ms)
  ✔ plan-sync: TDD-PLAN.md §8 ↔ тестовые и продуктовые файлы
  ✔ ipc-path: запрет передачи путей в IPC и файловых плагинов
  ✔ test-policy: запреты .only, .todo, пустых assert, @ts-ignore
  ℹ tests 102
  ℹ suites 17
  ℹ pass 102
  ℹ fail 0
  ℹ cancelled 0
  ℹ skipped 0
  ℹ todo 0
  ℹ duration_ms 1328.91624
  STEP1_EXIT=0
  ```

### Шаг 2. Проверка компиляции и типов TypeScript / JSDoc

- **Команда:**
  ```sh
  npx tsc -p tsconfig.json && npx tsc -p rust/gui-spike/ui/tsconfig.json
  ```
- **Ожидаемый результат:** Оба проекта проходят строгую проверку типов без ошибок и предупреждений, код возврата `0`.
- **Фактический результат:** Оба вызова `tsc` завершились с пустым выводом (0 ошибок) и кодом возврата `0`.
- **Вывод лога:**
  ```text
  === STEP 2A: TSC ROOT ===
  STEP2A_EXIT=0
  === STEP 2B: TSC UI ===
  STEP2B_EXIT=0
  ```

### Шаг 3. Функциональные и регрессионные unit-наборы UI

- **Команда:**
  ```sh
  node --test \
    tests/unit/notifications-ui.test.mjs \
    tests/unit/attachments-render.test.mjs \
    tests/unit/voice-devices-settings.test.mjs \
    tests/unit/gui-spike-dom.test.mjs \
    tests/unit/gui-spike-features.test.mjs
  ```
- **Ожидаемый результат:** Все функциональные и регрессионные unit-тесты проходят успешно (`fail 0`, `skipped 0`), код возврата `0`.
- **Фактический результат:** `183/183` тестов (`22 suites`) прошли успешно за `1734.24 ms`, `fail 0`, `skipped 0`, код возврата `0`.
- **Разбивка по наборам:**
  | Тестовый набор | Suites | Pass | Fail | Skip | Время | Код возврата |
  |---|---:|---:|---:|---:|---:|---:|
  | `tests/unit/notifications-ui.test.mjs` | 4 | 17 | 0 | 0 | 62.32 ms | `0` |
  | `tests/unit/attachments-render.test.mjs` | 1 | 11 | 0 | 0 | 56.79 ms | `0` |
  | `tests/unit/voice-devices-settings.test.mjs` | 1 | 17 | 0 | 0 | 50.63 ms | `0` |
  | `tests/unit/gui-spike-dom.test.mjs` | 3 | 38 | 0 | 0 | 563.46 ms | `0` |
  | `tests/unit/gui-spike-features.test.mjs` | 13 | 100 | 0 | 0 | 1321.54 ms | `0` |
  | **Итого (совместный прогон)** | **22** | **183** | **0** | **0** | **1734.24 ms** | **`0`** |
- **Вывод лога:**
  ```text
  ✔ N-05 UI Notifications: чистый резолвер resolveChannelNotificationState (каскад §1.4 и §1.5) (7.49ms)
  ✔ N-05 UI Notifications: мутаторы applyGuildSettingsPatch и applyChannelOverridePatch (2.01ms)
  ✔ N-05 UI Notifications: индикаторы непрочитанного и бейджей в #guild-rail и #channel-tree (2.69ms)
  ✔ N-05 UI Notifications: контроллер createNotificationSettingsController (#guild-notif-modal и #channel-notif-menu) (23.19ms)
  ✔ AT-03: рендер карточек вложений в ленте чата (ATTACHMENTS-RENDER-SPEC §1–§8) (56.79ms)
  ✔ V-07 UI: устройства и режим активации («Голос и видео») (16.64ms)
  ℹ tests 183
  ℹ suites 22
  ℹ pass 183
  ℹ fail 0
  ℹ cancelled 0
  ℹ skipped 0
  ℹ todo 0
  ℹ duration_ms 1734.239129
  STEP3_EXIT=0
  ```

### Шаг 4. Контрактные тесты Rust (`discord-gui-spike`)

- **Команда:**
  ```sh
  cd rust/gui-spike && cargo test --offline -p discord-gui-spike --test notifications_contract --test ipc_wiring_contract --test attachments_contract
  ```
- **Ожидаемый результат:** Все три контрактных таргета компилируются офлайн и проходят со 100% успешным результатом (`0 failed`, `0 ignored`), код возврата `0`.
- **Фактический результат:** `35/35` контрактных тестов прошли успешно (`attachments_contract`: `23/23`, `ipc_wiring_contract`: `8/8`, `notifications_contract`: `4/4`), код возврата `0`.
- **Вывод лога:**
  ```text
  Finished `test` profile [unoptimized + debuginfo] target(s) in 9.42s
  Running tests/attachments_contract.rs (target/debug/deps/attachments_contract-dec706dc624e87f3)
  running 23 tests
  ...
  test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

  Running tests/ipc_wiring_contract.rs (target/debug/deps/ipc_wiring_contract-fd68270b2b73aeea)
  running 8 tests
  ...
  test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

  Running tests/notifications_contract.rs (target/debug/deps/notifications_contract-54db853511b687fd)
  running 4 tests
  ...
  test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

  STEP4_EXIT=0
  ```

---

## 4. Обнаруженные дефекты и замечания (классификация P0 / P1 / P2)

Продуктовый код в ходе QA-тестирования не изменялся. Блокирующих дефектов (`P0` и `P1`) в сборке `BUILD-001` **не обнаружено**. Зафиксированы 3 неблокирующих замечания уровня `P2`:

| ID | Критичность | Компонент / Файл | Описание дефекта / несоответствия | Рекомендация / Задача на исправление |
|---|---|---|---|---|
| **QA-B001-P2-01** | **P2** | Инструкция сборки [BUILD-001.md](file:///home/user/orca/workspaces/project/qa-task-116-qa-build-001/waves/RW-40/BUILDS/BUILD-001.md#L43) | Команда шага 4 `cargo test --offline -p discord-gui-spike ...` указана без перехода в подкаталог `cd rust/gui-spike` (или флага `--manifest-path rust/gui-spike/Cargo.toml`). Поскольку `rust/gui-spike/Cargo.toml` объявляет собственный `[workspace]`, запуск из корня или из `rust/` возвращает `error: package ID specification discord-gui-spike did not match any packages` (`exit 101`). | Уточнить рабочую директорию `cd rust/gui-spike` (или `--manifest-path rust/gui-spike/Cargo.toml`) в шаблоне `BUILD-002.md`. |
| **QA-B001-P2-02** | **P2** | Документация сборки [BUILD-001.md](file:///home/user/orca/workspaces/project/qa-task-116-qa-build-001/waves/RW-40/BUILDS/BUILD-001.md#L19) и [PM/at03-render-ui/ACCEPT.md](file:///home/user/orca/workspaces/project/qa-task-116-qa-build-001/waves/RW-40/PM/at03-render-ui/ACCEPT.md#L10) | В описании среза `AT-03` указан несуществующий файл `attachments-render.js`, тогда как фактическая принятая реализация рендера вложений находится в [rust/gui-spike/ui/features/chat.js](file:///home/user/orca/workspaces/project/qa-task-116-qa-build-001/rust/gui-spike/ui/features/chat.js) (`md5: 0e3dc4984bf7a5db8f400e3c288f873f`) и [rust/gui-spike/ui/components/chat.css](file:///home/user/orca/workspaces/project/qa-task-116-qa-build-001/rust/gui-spike/ui/components/chat.css) (`md5: 96b789ab7b8459008efa1f429dfe3863`), что отражено в `PM/at03-render-ui/REPORT.md` и проверяется `tests/unit/attachments-render.test.mjs`. | Синхронизировать ссылку на `chat.js` / `chat.css` в описании следующих сборок (`BUILD-002`). |
| **QA-B001-P2-03** | **P2** | `rust/gui-spike/src/main.rs` и `rust/gui-spike/tests/ipc_wiring_contract.rs` (`PR-02`, `task-106`) | Известные неблокирующие замечания ревью `PR-02`: (1) статический тест `every_ui_invoke_command_in_js_sources_is_registered_in_tauri_main` проверяет 26 литеральных вызовов из 38 (вызовы через константы `AUTH_COMMANDS`/`JOIN_COMMANDS`/`NET_COMMANDS`/`READY_COMMAND` не разворачиваются); (2) `ChannelOverrideInput::into_patch` содержит fallback `.or_else(|| channel_overrides.into_values().next())` и игнорирует прямое поле `override`. | Уже вынесено в отдельную задачу `bugfix/task-109-ipc-oracle-strict` для включения в `BUILD-002`. |

---

## 5. Контрольные суммы проверенных артефактов (MD5)

```text
0b76263a7eab93af74618de5d384c188  waves/RW-40/BUILDS/BUILD-001.md
60f110602a27b9e2c58946a91826ed58  waves/RW-40/PR/PR-01-search-202-nsfw-spec.md
aac344124ca5bc247eab542cc3c739b7  waves/RW-40/PR/PR-02-task-106-notif-ipc.md
49c11a2524b849e9ca1625d942d3b63e  waves/RW-40/PR/PR-03-task-104-n05-ui-notifications.md
4d07bc2fa8ecaa964e99f546096b6bd8  waves/RW-40/DESIGN/SEARCH-202-NSFW-SPEC.md
ae573539a9b8a8cc6e355c3975edaff0  rust/gui-spike/src/main.rs
155ad039f672518b8298fb1b52789560  rust/gui-spike/src/lib.rs
2768dddc07f378bbed7884f5f21579bb  rust/gui-spike/src/notifications.rs
75f50c624ab5a6b21a67281235f4b289  rust/gui-spike/src/attachments.rs
9f047fd938b856037d1c1f5cbc3324a3  rust/gui-spike/tests/notifications_contract.rs
159053d11b700daba7a0ecdc863fb7fc  rust/gui-spike/tests/ipc_wiring_contract.rs
77157aadf4267ded8c225d8891c71a9f  rust/gui-spike/tests/attachments_contract.rs
001ff70a7538378d16d52fd3b6b10117  rust/gui-spike/ui/features/notifications.js
0e3dc4984bf7a5db8f400e3c288f873f  rust/gui-spike/ui/features/chat.js
96b789ab7b8459008efa1f429dfe3863  rust/gui-spike/ui/components/chat.css
5fda00c8c063f9726fc1ab31ac13ef99  rust/gui-spike/ui/features/voice-devices-settings.js
22238cac9b6e56b5ba9aa3f33c4484f7  tests/unit/notifications-ui.test.mjs
8b4678d19c6aa4ed9a6b7aa8ad787615  tests/unit/attachments-render.test.mjs
bf7544e0c25c18ca0e8eaba579bca7bc  tests/unit/voice-devices-settings.test.mjs
e39c8a68936367fef0f51fa27a594b31  tests/unit/gui-spike-dom.test.mjs
3c85261013fe463427ef59c1ea4bf6a8  tests/unit/gui-spike-features.test.mjs
a1a8fd5662be88108dd25ac619a377f2  fixtures/gateway/by-type/002-READY.json
1a44becfab3c6fd4170346635f80830b  waves/RW-40/VOICE/stream-opcodes-fixtures.json
```

---

## 6. Финальный вердикт QA

**Вердикт по сборке `BUILD-001`:** **QA Passed**

Все 4 обязательных проверочных контура (`meta` 102/102, `tsc` root + UI 0 ошибок, `unit` 183/183, `cargo test` контракты `discord-gui-spike` 35/35) пройдены со 100% успешным результатом (`exit 0`). Сборка `BUILD-001` рекомендована к продвижению в ветку предрелизной проверки `beta`.
