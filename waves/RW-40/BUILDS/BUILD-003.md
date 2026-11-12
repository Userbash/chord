# BUILD-003 — Централизованная сборка `testing` для отдела G4 QA

- **Номер сборки:** `BUILD-003`
- **Целевая ветка интеграции:** `testing` (`/home/user/orca/workspaces/project/testing`)
- **HEAD сборки:** `7f96e1b` — `Build-003 sync: merge PR-172 (task-292 app.js shell and popovers mount)`
- **Источник интеграции:** `unstable` — окно `ebdb966..a071808`, **51 PR** (`PR-125`..`PR-175`)
- **Дата редакции:** `2026-10-31`
- **Статус:** `In Testing` (передано в G4 QA), все интеграционные разрывы закрыты
- **Заменяет редакцию:** `task-278` (`PR-125..PR-155`, HEAD `d047fe4`) и `task-129` (`PR-08..PR-12`)

Сборка объединяет всю волну финализации RW-40: 9 модулей Rust BFF, 29 UI feature-модулей, мосты интеграции оболочки (`app-shell-wiring.js`, `composer-popovers-bridge.js`), полный стек `RE-GAP-22` (Voice WebRTC Diagnostics) и регистрацию единого роутера `invoke_bff_domain` в Tauri IPC.

---

## 1. Что входит в сборку

Счётчики `testing` = `7f96e1b` (проверены `git ls-tree`):

| Метрика | Значение | Команда проверки |
|---|---:|---|
| Rust-тесты `rust/gui-spike/tests/` | 43 файла, из них 40 `*_contract.rs` | `git ls-tree --name-only testing rust/gui-spike/tests/ \| wc -l` |
| Node unit-тесты `tests/unit/` | 115 файлов | `git ls-tree --name-only testing tests/unit/ \| wc -l` |
| UI feature-модули | 98 файлов | `git ls-tree --name-only testing rust/gui-spike/ui/features/ \| wc -l` |
| Rust BFF-модули `rust/gui-spike/src/` | 38 файлов, 36 модулей без `lib.rs`/`main.rs` | `git ls-tree --name-only testing rust/gui-spike/src/ \| wc -l` |
| `#[tauri::command]` в `main.rs` | 53 (добавлена `invoke_bff_domain`, `PR-173`) | `git show testing:rust/gui-spike/src/main.rs \| grep -c '#\[tauri::command\]'` |
| `pub mod` в `lib.rs` | 36 (добавлен `voice_stage`, `PR-165`) | `git show testing:rust/gui-spike/src/lib.rs \| grep -c '^pub mod'` |

### 1.1. Rust BFF — 9 модулей волны RW-40

| Модуль | Задача / PR | Контрактный оракул | Назначение |
|---|---|---|---|
| `expressions.rs` | `task-242` / `PR-127` | `tests/expressions_contract.rs` | каталог и активация выражений гильдии |
| `webhooks.rs` | `task-245` / `PR-131` | `tests/webhooks_contract.rs` | CRUD вебхуков канала |
| `automod.rs` | `task-249` / `PR-135` | `tests/automod_contract.rs` | правила автомодерации |
| `slash_commands.rs` | `task-253` / `PR-138` | `tests/slash_commands_contract.rs` | реестр и вызов slash-команд |
| `onboarding.rs` | `task-258` / `PR-141` | `tests/onboarding_contract.rs` | шаги онбординга гильдии |
| `soundboard.rs` | `task-259` / `PR-144` | `tests/soundboard_contract.rs` | звуки soundboard, лимиты и громкость |
| `stage_instances.rs` | `task-265` / `PR-150` | `tests/stage_instances_contract.rs` | инстансы stage-каналов и запросы на выступление |
| `bff_domains_router.rs` | `task-269` / `PR-153`, `PR-164` | `tests/bff_domains_router_contract.rs` | единый роутер для slash_commands/onboarding/soundboard/stage_instances |
| `voice_diagnostics.rs` | `task-294` / `PR-174` | `tests/voice_diagnostics_contract.rs` | BFF-модуль RTC-диагностики, сбор рядов и безопасный экспорт |

### 1.2. Реестр PR сборки BUILD-003 (`PR-125..PR-175`)

| PR | Задача | Тип | Артефакт | Тест-оракул | Ревью | Статус подключения |
|---|---|---|---|---|---|---|
| `PR-125` | `task-243` | UI-H | `ui/features/event-banner.js` | `tests/unit/event-banner.test.mjs` | `154A` (5/5) | Смонтирован через `chat-helpers-bridge.js` |
| `PR-126` | `task-244` | UI-H | `ui/features/attachment-helper.js` | `tests/unit/attachment-helper.test.mjs` | `155A` (5/5) | Смонтирован через `chat-helpers-bridge.js` |
| `PR-127` | `task-242` | BE | `src/expressions.rs` | `tests/expressions_contract.rs` | `156A` (5/5) | Роутер BFF / админ-фасад |
| `PR-128` | `task-240` | UI-C | `ui/features/forum-view.js` | `tests/unit/forum-view.test.mjs` | `157A` (5/5) | Зарегистрирован в `shell-controllers-registry.js` |
| `PR-129` | `task-241` | UI-C | `ui/features/poll-card.js` | `tests/unit/poll-card.test.mjs` | `158A` (5/5) | Зарегистрирован в `shell-controllers-registry.js` |
| `PR-130` | `task-246` | UI-H | `ui/features/voice-quality.js` | `tests/unit/voice-quality.test.mjs` | `159A` (5/5) | Смонтирован в `voice.js` |
| `PR-131` | `task-245` | BE | `src/webhooks.rs` | `tests/webhooks_contract.rs` | `160A` (5/5) | Роутер BFF / админ-фасад |
| `PR-132` | `task-247` | UI-C | `ui/features/events-modal.js` | `tests/unit/events-modal.test.mjs` | `161A` (5/5) | Зарегистрирован в `shell-controllers-registry.js` |
| `PR-133` | `task-248` | UI-C | `ui/features/role-editor.js` | `tests/unit/role-editor.test.mjs` | `162A` (5/5) | Зарегистрирован в `shell-controllers-registry.js` |
| `PR-134` | `task-250` | UI-H | `ui/features/slowmode.js` | `tests/unit/slowmode.test.mjs` | `163A` (5/5) | Смонтирован через `chat-helpers-bridge.js` |
| `PR-135` | `task-249` | BE | `src/automod.rs` | `tests/automod_contract.rs` | `164A` (5/5) | Роутер BFF / админ-фасад |
| `PR-136` | `task-251` | UI-C | `ui/features/automod-editor.js` | `tests/unit/automod-editor.test.mjs` | `165A` (5/5) | Зарегистрирован в `shell-controllers-registry.js` |
| `PR-137` | `task-252` | UI-C | `ui/features/webhooks-editor.js` | `tests/unit/webhooks-editor.test.mjs` | `166A` (5/5) | Зарегистрирован в `shell-controllers-registry.js` |
| `PR-138` | `task-253` | BE | `src/slash_commands.rs` | `tests/slash_commands_contract.rs` | `167A` (5/5) | Роутер BFF (`invoke_bff_domain`) |
| `PR-139` | `task-254` | UI-H | `ui/features/thread-badge.js` | `tests/unit/thread-badge.test.mjs` | `168A` (5/5) | Смонтирован через `chat-helpers-bridge.js` |
| `PR-140` | `task-255` | UI-H | `ui/features/guild-boost.js` | `tests/unit/guild-boost.test.mjs` | `169A` (5/5) | Смонтирован через `chat-helpers-bridge.js` |
| `PR-141` | `task-258` | BE | `src/onboarding.rs` | `tests/onboarding_contract.rs` | `170A` (5/5) | Роутер BFF (`invoke_bff_domain`) |
| `PR-142` | `task-256` | UI-C | `ui/features/expressions-manager.js` | `tests/unit/expressions-manager.test.mjs` | `171A` (5/5) | Зарегистрирован в `shell-controllers-registry.js` |
| `PR-143` | `task-257` | UI-C | `ui/features/moderation-panel.js` | `tests/unit/moderation-panel.test.mjs` | `172A` (5/5) | Зарегистрирован в `shell-controllers-registry.js` |
| `PR-144` | `task-259` | BE | `src/soundboard.rs` | `tests/soundboard_contract.rs` | `173A` (5/5) | Роутер BFF (`invoke_bff_domain`) |
| `PR-145` | `task-260` | UI-C | `ui/features/soundboard-picker.js` | `tests/unit/soundboard-picker.test.mjs` | `174A` (5/5) | Смонтирован в `composer-popovers-bridge.js` |
| `PR-146` | `task-261` | UI-C | `ui/features/slash-command-picker.js` | `tests/unit/slash-command-picker.test.mjs` | `175A` (5/5) | Смонтирован в `composer-popovers-bridge.js` |
| `PR-147` | `task-262` | UI-C | `ui/features/onboarding-wizard.js` | `tests/unit/onboarding-wizard.test.mjs` | `176A` (5/5) | Зарегистрирован в `shell-controllers-registry.js` |
| `PR-148` | `task-263` | UI-H | `ui/features/voice-sound-effects.js` | `tests/unit/voice-sound-effects.test.mjs` | `177A` (5/5) | Смонтирован в `composer-popovers-bridge.js` |
| `PR-149` | `task-266` | UI-C | `ui/features/stage-request-queue.js` | `tests/unit/stage-request-queue.test.mjs` | `178A` (5/5) | Зарегистрирован в `shell-controllers-registry.js` |
| `PR-150` | `task-265` | BE | `src/stage_instances.rs` | `tests/stage_instances_contract.rs` | `179A` (5/5) | Роутер BFF (`invoke_bff_domain`) |
| `PR-151` | `task-267` | UI-B | `ui/features/shell-controllers-registry.js` | `tests/unit/shell-controllers-registry.test.mjs` | `180A` (5/5) | Подключён в `app-shell-wiring.js` |
| `PR-152` | `task-268` | UI-B | `ui/features/chat-helpers-bridge.js` | `tests/unit/chat-helpers-bridge.test.mjs` | `181A` (5/5) | Подключён в `app-shell-wiring.js` |
| `PR-153` | `task-269` | BE | `src/bff_domains_router.rs` | `tests/bff_domains_router_contract.rs` | `182A` (5/5) | Подключён в `main.rs` (`invoke_bff_domain`) |
| `PR-154` | `task-270` | UI-B | `ui/features/composer-popovers-bridge.js` | `tests/unit/composer-popovers-bridge.test.mjs` | `183A` (5/5) | Смонтирован в `app.js` (`PR-172`) |
| `PR-155` | `task-271` | UI-B | `ui/features/app-shell-wiring.js` | `tests/unit/app-shell-wiring.test.mjs` | `184A` (5/5) | Смонтирован в `app.js` (`PR-172`) |
| `PR-156` | `task-277` | FIX | 7 файлов в `ui/features/` | `ui/tsconfig.json` | `185A` (5/5) | Чистый tsc-контроль |
| `PR-157` | `task-276` | UI-B | `ui/features/app-shell-wiring.js` | `tests/unit/app-shell-wiring.test.mjs` | `186A` (5/5) | Смонтирован в `app.js` (`PR-172`) |
| `PR-158` | `task-279` | FIX | 5 файлов в `ui/features/` | `ui/tsconfig.json` | `187A` (5/5) | Чистый tsc-контроль |
| `PR-159` | `task-280` | FIX | `tests/unit/profile-modal` / `appearance` | `ui/tsconfig.json` | `188A` (5/5) | Чистый tsc-контроль |
| `PR-160` | `task-281` | FIX | `tests/unit/svg-icons` / `channel-tree` | `ui/tsconfig.json` | `189A` (5/5) | Чистый tsc-контроль |
| `PR-161` | `task-275` | SPEC | `APP-SHELL-WIRING-SPEC.md` | Сверка спецификации | PM PASS | Оракул для `app.js` |
| `PR-162` | `task-282` | FIX | 10 unit-тестов UI | `ui/tsconfig.json` | `191A` (5/5) | Чистый tsc-контроль |
| `PR-163` | `task-285` | UI-B | `ui/features/composer-popovers-bridge.js` | `tests/unit/composer-popovers-bridge.test.mjs` | `192A` (5/5) | Смонтирован в `app.js` (`PR-172`) |
| `PR-164` | `task-284` | BE | `src/bff_domains_router.rs` | `tests/bff_domains_router_contract.rs` | `193A` (5/5) | Подключён в `main.rs` (`invoke_bff_domain`) |
| `PR-165` | `task-283` | QA | `ui/tsconfig.json`, `test-impact.mjs` | `tests/unit/test-impact.test.mjs` | `194A` (7/7) | Чистый typecheck и impact-тест |
| `PR-166` | `task-286` | UI | `ui/features/dispatch-model.js` | `tests/unit/dispatch_ui.test.mjs` | `195A` (5/5) | Смонтирован в сетевой слой UI |
| `PR-167` | `task-287` | DOCS | `TDD-PLAN.md` §8 | `tests/meta/plan-sync.test.mjs` | PM PASS (5/5) | Синхронизация плана тестирования |
| `PR-168` | `task-288` | PLAT | `safe_storage_contract.rs`, `gate.mjs` | `tests/meta/keyring-live.test.mjs` | `196A` (5/5) | Автоматизирован в CI |
| `PR-169` | `task-290` | UI | `ui/features/voice-diagnostics.js` | `tests/unit/voice-diagnostics.test.mjs` | `197A` (7/7) | Подключён в `voice.js` (`PR-175`) |
| `PR-170` | `task-291` | VOICE | `discord-voice/src/stats.rs` | `discord-voice/tests/stats_contract.rs` | `198A` (5/5) | Интегрирован в RTC-сессию голоса |
| `PR-171` | `task-289` | SPEC | `docs/specs/VOICE-DIAGNOSTICS-SPEC.md` | Сверка спецификации | PM PASS | Оракул для попапа диагностики |
| `PR-172` | `task-292` | UI-B | `ui/app.js` | `tests/unit/app-shell-wiring.test.mjs` | PM PASS | Главная точка входа UI рантайма |
| `PR-173` | `task-293` | BE | `src/main.rs` | `src/main.rs:wiring_tests` | PM PASS | Зарегистрирован в `invoke_handler!` Tauri |
| `PR-174` | `task-294` | BE | `src/voice_diagnostics.rs` | `tests/voice_diagnostics_contract.rs` | REVIEW (21/21) | BFF-экспорт метрик RTC в UI |
| `PR-175` | `task-295` | UI | `ui/features/voice.js` | `tests/unit/voice-ui.test.mjs` | REVIEW (47/47) | Клик по `#voice-ping` открывает попап |

---

## 2. Команды для полной проверки сборки G4 QA

```bash
# 1. Проверка типов и мета-тестов
npm run typecheck
npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit
npm run test:meta

# 2. Целевые unit-тесты интеграционных модулей
node --test tests/unit/test-impact.test.mjs \
            tests/unit/dispatch_ui.test.mjs \
            tests/unit/app-shell-wiring.test.mjs \
            tests/unit/composer-popovers-bridge.test.mjs \
            tests/unit/voice-diagnostics.test.mjs \
            tests/unit/voice-ui.test.mjs

# 3. Контрактные тесты Rust BFF и ядра голоса
CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/gui-spike/Cargo.toml --test bff_domains_router_contract --test voice_stage_contract --test voice_diagnostics_contract
CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test -p discord-voice --lib
CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test -p discord-platform --test safe_storage_contract
```

---

## 3. История редакций манифеста

| Редакция | Состав | HEAD `testing` | MD5 манифеста | Статус |
|---|---|---|---|---|
| `task-129`, 2026-10-30 | `PR-08`..`PR-12` | `4e24048` | `61a925d2463e35984b1e29aad5c4d2b7` | Устарела |
| `task-278`, 2026-10-31 | `PR-125`..`PR-155` | `d047fe4` | `4f32...` | Заменена расширенной |
| `task-296`, 2026-10-31 | `PR-125`..`PR-175` | `7f96e1b` | текущий файл | **Актуальная финальная сборка** |
