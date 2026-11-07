# MERGE-READINESS-005 — Build-003增量: паритет волны `PR-125..PR-144`

Дата снимка: **2026-10-31**. Воркер: **G6 Tech Writer / Parity**, задача `task-264` (`DOCS-P1-15`), dispatch `ctx_25f441a5e252`.
Замер по git-дереву, без правок продуктового кода: `unstable` = `36dc676`, `testing` = `87dc4cc`, `main` = `4e24048`, `beta` = `e91acf6`.
Начало окна: `ebdb966` (последний коммит перед слиянием `PR-125`/`PR-126`).

## 0. Резюме

1. **20 PR (`PR-125..PR-144`) слиты в `unstable` и перенесены в `testing`.** Каждый прошёл PM acceptance и техническое ревью `DeepSeek`: 20/20 `APPROVED`, `P0=0`, `P1=0`, мутации `5/5` убиты в каждом ревью.
2. **Волна добавила 6 Rust BFF-модулей и 14 UI-модулей** (11 новых файлов + 3 расширенных). Итог на `36dc676`: **34 модуля BFF** (`rust/gui-spike/src/*.rs` без `lib.rs`/`main.rs`) и **88 UI feature-модулей** (`rust/gui-spike/ui/features/*.js`).
3. **Интеграционный разрыв не закрыт:** `shell-bridge.js` из `PR-122` не импортируется ни одним файлом `rust/gui-spike/ui/**`; 8 новых контроллеров не регистрируются; 6 новых BFF-модулей не имеют `#[tauri::command]` (в `main.rs` по-прежнему 52 команды). Функциональность существует как модули и тесты, но в рантайме недостижима.
4. **`BUILD-003` в статусе `In Testing`:** файла `waves/RW-40/QUALITY/QA-BUILD-003-REPORT.md` нет ни на `unstable`, ни на `testing`; `beta` и `main` отстают на всю волну.
5. **Гейт `testing -> beta -> main` не может быть закрыт до трёх работ:** подключение `shell-bridge.js` в `app.js`, экспорт 6 BFF-доменов как IPC-команд, монтаж 6 helper-модулей в экраны.

## 1. Матрица паритета `PR-125..PR-144`

Тип: `BE` — Rust BFF, `UI-C` — UI-контроллер (экспортирует `create*Controller`), `UI-H` — UI-helper (чистые функции без DOM).
Подключение: `IPC-нет` — модуль есть, `#[tauri::command]` нет; `shell-нет` — контроллер не зарегистрирован в `shell-bridge.js`; `mount-нет` — helper не импортируется экраном.

| PR | Задача | Тип | Артефакт | Тест-оракул | Ревью | Подключение |
|---|---|---|---|---|---|---|
| `PR-125` | `task-243` `FE-P2-28` | UI-H | `ui/features/event-banner.js` | `tests/unit/event-banner.test.mjs` | `154A` | `mount-нет` |
| `PR-126` | `task-244` `FE-P2-29` | UI-H | `ui/features/attachment-helper.js` | `tests/unit/attachment-helper.test.mjs` | `155A` | `mount-нет` |
| `PR-127` | `task-242` `BE-P1-26` | BE | `src/expressions.rs` | `tests/expressions_contract.rs` | `156A` | `IPC-нет` |
| `PR-128` | `task-240` `FE-P0-10` | UI-C | `ui/features/forum-view.js` (расширен) | `tests/unit/forum-view.test.mjs` | `157A` | `shell-нет` |
| `PR-129` | `task-241` `FE-P1-27` | UI-C | `ui/features/poll-card.js` | `tests/unit/poll-card.test.mjs` | `158A` | `shell-нет` |
| `PR-130` | `task-246` `FE-P2-30` | UI-H | `ui/features/voice-quality.js` (расширен) | `tests/unit/voice-quality.test.mjs` | `159A` | `mount-нет` |
| `PR-131` | `task-245` `BE-P1-27` | BE | `src/webhooks.rs` | `tests/webhooks_contract.rs` | `160A` | `IPC-нет` |
| `PR-132` | `task-247` `FE-P0-11` | UI-C | `ui/features/events-modal.js` | `tests/unit/events-modal.test.mjs` | `161A` | `shell-нет` |
| `PR-133` | `task-248` `FE-P0-12` | UI-C | `ui/features/role-editor.js` | `tests/unit/role-editor.test.mjs` | `162A` | `shell-нет` |
| `PR-134` | `task-250` `FE-P2-31` | UI-H | `ui/features/slowmode.js` (расширен) | `tests/unit/slowmode.test.mjs` | `163A` | `mount-нет` |
| `PR-135` | `task-249` `BE-P1-28` | BE | `src/automod.rs` | `tests/automod_contract.rs` | `164A` | `IPC-нет` |
| `PR-136` | `task-251` `FE-P0-13` | UI-C | `ui/features/automod-editor.js` | `tests/unit/automod-editor.test.mjs` | `165A` | `shell-нет` |
| `PR-137` | `task-252` `FE-P0-14` | UI-C | `ui/features/webhooks-editor.js` | `tests/unit/webhooks-editor.test.mjs` | `166A` | `shell-нет` |
| `PR-138` | `task-253` `BE-P1-29` | BE | `src/slash_commands.rs` | `tests/slash_commands_contract.rs` | `167A` | `IPC-нет` |
| `PR-139` | `task-254` `FE-P2-32` | UI-H | `ui/features/thread-badge.js` | `tests/unit/thread-badge.test.mjs` | `168A` | `mount-нет` |
| `PR-140` | `task-255` `FE-P2-33` | UI-H | `ui/features/guild-boost.js` | `tests/unit/guild-boost.test.mjs` | `169A` | `mount-нет` |
| `PR-141` | `task-258` `BE-P1-30` | BE | `src/onboarding.rs` | `tests/onboarding_contract.rs` | `170A` | `IPC-нет` |
| `PR-142` | `task-256` `FE-P0-15` | UI-C | `ui/features/expressions-manager.js` | `tests/unit/expressions-manager.test.mjs` | `171A` | `shell-нет` |
| `PR-143` | `task-257` `FE-P0-16` | UI-C | `ui/features/moderation-panel.js` | `tests/unit/moderation-panel.test.mjs` | `172A` | `shell-нет` |
| `PR-144` | `task-259` `BE-P1-31` | BE | `src/soundboard.rs` | `tests/soundboard_contract.rs` | `173A` | `IPC-нет` |

Итого: `BE` 6, `UI-C` 8, `UI-H` 6. Ни один из 20 артефактов не имеет потребителя в рантайме — только тесты.

## 2. Числа на `36dc676` (и дельта окна)

| Метрика | Начало окна `ebdb966` | Сейчас `36dc676` | Дельта |
|---|---|---|---|
| Rust BFF: файлы `rust/gui-spike/src/*.rs` | 30 | **36** | +6 |
| Rust BFF: модули без `lib.rs`/`main.rs` | 28 | **34** | +6 |
| `#[tauri::command]` в `main.rs` | 52 | **52** | 0 |
| UI feature-модули `ui/features/*.js` | 77 | **88** | +11 новых, 3 расширены |
| UI-модули всего (+`app.js`, `i18n.js`, `model.js`) | — | **91** | — |
| Rust-тесты `rust/gui-spike/tests/*` | 35 | **41** | +6 |
| из них `*_contract.rs` | — | **38** | — |
| Node unit-тесты `tests/unit/*.test.mjs` | 94 | **105** | +11 |

Состав модулей: 33 модуля объявлены в `src/lib.rs` (`pub mod`), `voice_stage.rs` объявляется в `main.rs`. Шесть новых BFF-модулей добавлены в `lib.rs` (`slash_commands`, `onboarding` — `:285-286`, `soundboard` — `:287`).
`PR-123` (`task-239`, `src/scheduled_events.rs`) лежит **вне окна** и служит основой для `event-banner`/`events-modal`; в матрицу следующего среза его нужно включить явно.

## 3. Где именно рвётся интеграция

1. **`shell-bridge.js` — мёртвый модуль.** `PR-122` (`task-236`) слит, но `git grep -l "shell-bridge" unstable -- 'rust/gui-spike/ui/**'` возвращает единственный файл — сам `shell-bridge.js`. В `ui/app.js` (27 `import`) и `ui/features/screens.js` вызовов `createShellFeatureBridge` нет. Тест `tests/unit/shell-bridge.test.mjs` покрывает мост в изоляции, поэтому гейт зелёный при нерабочей фиче.
2. **8 контроллеров не зарегистрированы.** `automod-editor`, `webhooks-editor`, `expressions-manager`, `moderation-panel`, `poll-card`, `forum-view`, `events-modal`, `role-editor` экспортируют `create*Controller`, но `registerController` вызывается только внутри тестов. `SHELL_MODAL_LAYERS` заморожен на **10 слоях** и новых id не содержит.
3. **Тест закрепляет текущий разрыв.** `tests/unit/shell-bridge.test.mjs:54-68` сверяет точный список из 10 слоёв через `assert.deepEqual`. Добавление слоёв в код без правки теста даст red, а правка «под код» без новых сценариев — нарушение `TEST-POLICY.md`. Обновлять надо парой: код + тест + новый кейс открытия слоя.
4. **6 BFF-доменов недостижимы из UI.** `automod`, `expressions`, `webhooks`, `slash_commands`, `onboarding`, `soundboard` — сервисный слой с контрактными тестами, но число `#[tauri::command]` в `main.rs` не изменилось (52 до и после волны). UI-вызов через `invoke` невозможен.
5. **6 helper-модулей не смонтированы.** `event-banner`, `attachment-helper`, `slowmode`, `thread-badge`, `guild-boost`, `voice-quality` — чистые функции без DOM-обвязки; их не импортирует ни `chat.js`, ни `composer.js`, ни `voice.js`, ни `settings.js`.

## 4. Чеклист готовности к финальному QA-гейту `BUILD-003`

Слияние и ревью:
- [x] 20/20 PR (`PR-125..PR-144`) влиты в `unstable` (`36dc676`), рабочие ветки удалены после merge.
- [x] 20/20 технических ревью `APPROVED` (`P0=0`, `P1=0`, мутации `5/5`), PM acceptance по каждой карточке (`waves/RW-40/G{1,2}/task-*/ACCEPT.md`).
- [x] `testing` синхронизирован (`87dc4cc`) и содержит `waves/RW-40/BUILDS/BUILD-003.md`.

QA-гейт:
- [ ] `QA-BUILD-003-REPORT.md` отсутствует — сборка числится `In Testing` без вердикта G4 QA.
- [ ] `T0`: `node scripts/test-impact.mjs` по затронутым модулям (11 новых unit-файлов + 6 контрактов).
- [ ] `T1`: `npm run typecheck`, `npm run test:meta` (102/102), `npm run test:unit`, `cargo test --workspace` на общем `CARGO_TARGET_DIR`.
- [ ] `T2`: прогон сборки `testing` по сценариям волны (automod/webhooks/expressions/events/polls/forum/roles/moderation/soundboard) — без подключения из §5 п.1-3 сценарии недостижимы.
- [ ] `T4`: `npm run gate:release` (`failOnSkips: true`).

Продвижение сборки:
- [ ] `beta` на `e91acf6` (19 `src/*.rs`) — отстаёт на всю волну; перенос `testing -> beta` не выполнялся.
- [ ] `main` на `4e24048` — примет только PR из `beta` после `QA Passed`.
- [ ] Релизные артефакты (`scripts/release.sh`, `scripts/sbom.mjs`, `SHA256SUMS`) не пересобирались под Build-003.

## 5. Оставшиеся шаги (в порядке разблокировки)

1. **G2 Frontend:** импортировать `createShellFeatureBridge` в `ui/app.js`, зарегистрировать 8 контроллеров, расширить `SHELL_MODAL_LAYERS` и одновременно переписать `tests/unit/shell-bridge.test.mjs` (список слоёв + кейс открытия нового слоя).
2. **G1 Backend:** экспортировать 6 BFF-доменов через `#[tauri::command]` в `main.rs`, расширить `tests/ipc_wiring_contract.rs` (сейчас 9 тестов, реестр IPC — 38 команд) и зафиксировать новые команды как оракул.
3. **G2 Frontend:** смонтировать 6 helper-модулей в экраны — `event-banner`/`guild-boost`/`thread-badge` в список каналов, `slowmode`/`attachment-helper` в composer, `voice-quality` в `voice.js`.
4. **G4 QA:** прогнать `BUILD-003` и выпустить `waves/RW-40/QUALITY/QA-BUILD-003-REPORT.md` с вердиктом `P0–P3`.
5. **DevOps:** перенести сборку `testing -> beta`, затем `beta -> main` с версией, тегом и обратной синхронизацией в `unstable`.
6. **G6:** после приёмки — `MERGE-READINESS-006.md` с закрытым разрывом и обновление `waves/STATE.md`.

Следующий инкремент (вне окна): `task-260` (`soundboard-picker`), `task-261` (`slash-command-picker`), `task-262` (`onboarding-wizard`), `task-263` (`voice-sound-effects`) — `ACCEPT.md` у всех четырёх отсутствует, в матрицу не входят.

## 6. Риски

- **P1.** `shell-bridge.js` не подключён: 8 контроллеров волны не попадут в рантайм, а unit-тесты этого не покажут.
- **P1.** 6 BFF-доменов без IPC-команд: UI не может вызвать automod/webhooks/expressions/slash/onboarding/soundboard.
- **P2.** `shell-bridge.test.mjs:54-68` фиксирует 10 слоёв; правка списка «под код» без нового кейса = ослабление теста (`TEST-POLICY.md`, `P0`).
- **P2.** `beta` отстаёт на всю волну — перенос сборки и откат не репетированы.
- **P3.** `PR-123` (`scheduled_events`) вне окна, но связан с `event-banner`/`events-modal`: при срезе паритета учитывать вместе.

## 7. Воспроизводимость

```sh
git rev-parse --short unstable testing main beta            # 36dc676 / 87dc4cc / 4e24048 / e91acf6
git ls-tree --name-only unstable rust/gui-spike/src/ | wc -l          # 36
git ls-tree --name-only unstable rust/gui-spike/ui/features/ | wc -l  # 88
git ls-tree --name-only unstable rust/gui-spike/tests/ | wc -l        # 41
git ls-tree --name-only unstable tests/unit/ | wc -l                  # 105
git show unstable:rust/gui-spike/src/main.rs | grep -c '#\[tauri::command\]'   # 52
git grep -l "shell-bridge" unstable -- 'rust/gui-spike/ui/**'         # только сам модуль
git diff --name-status ebdb966 unstable -- rust/gui-spike/src/ rust/gui-spike/ui/features/ tests/unit/
```

Источники: `waves/RW-40/G1/task-{242,245,249,253,258,259}-*/{ACCEPT.md,REVIEW-*A.md}`, `waves/RW-40/G2/task-{240,241,243,244,246,247,248,250,251,252,254,255,256,257}-*/{ACCEPT.md,REVIEW-*A.md}`, `waves/RW-40/BUILDS/BUILD-003.md` (ветка `testing`), `rust/gui-spike/src/lib.rs:285-287`, `rust/gui-spike/ui/features/shell-bridge.js`, `tests/unit/shell-bridge.test.mjs:54-68`, `waves/STATE.md`.
