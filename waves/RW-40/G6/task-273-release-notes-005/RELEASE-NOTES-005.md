# RELEASE-NOTES-005 — Build-003: инкремент `PR-125..PR-152`

- **Дата снимка:** `2026-10-31`.
- **Воркер:** G6 Tech Writer / Parity, задача `task-273` (`DOCS-P1-16`), dispatch `ctx_ca7024f63f67`.
- **Снимок git:** `unstable` = `b35c3e1`, `testing` = `11070f0`, `beta` = `e91acf6`, `main` = `4e24048`.
- **Начало окна:** `ebdb966` — последний коммит перед слиянием `PR-125`. В окне **76 коммитов** и **28 PR**.
- **Основание:** `waves/RW-40/G6/task-264-merge-readiness-005/MERGE-READINESS-005.md` (срез `PR-125..PR-144`) + git-история `unstable` по `PR-145..PR-152`.

## 0. Резюме

1. **28 PR (`PR-125..PR-152`) влиты в `unstable`** и синхронизированы в `testing`. Каждый прошёл PM acceptance и техническое ревью DeepSeek: 28/28 `APPROVED`, `P0=0`, `P1=0`, мутации `5/5` убиты в каждом ревью.
2. **Инкремент добавил 7 Rust BFF-модулей и 21 UI-модуль** (18 новых файлов + 3 расширенных): `expressions`, `webhooks`, `automod`, `slash_commands`, `onboarding`, `soundboard`, `stage_instances`; контроллеры панелей гильдии, пикеры, мастер онбординга, очередь запросов на сцену, помощники чата и композера, включая два интеграционных моста — `shell-controllers-registry` и `chat-helpers-bridge`.
3. **Тестовая база выросла**: Rust-тесты `35 -> 42` файла (39 контрактных), Node unit-тесты `94 -> 112` файлов, UI feature-модули `77 -> 95`.
4. **Разрыв интеграции не закрыт**: 0 из 7 новых BFF-доменов имеют `#[tauri::command]` (в `main.rs` по-прежнему 52 команды), 0 из 21 UI-модуля инкремента имеют потребителя в рантайме (ни `app.js`, ни `screens.js`, ни `chat.js`/`composer.js`/`voice.js` их не импортируют). Мосты `shell-controllers-registry` и `chat-helpers-bridge` тоже не подключены.
5. **`BUILD-003` остаётся в `In Testing`**: `waves/RW-40/QUALITY/QA-BUILD-003-REPORT.md` отсутствует на `unstable`, `testing`, `beta` и `main`; манифест `waves/RW-40/BUILDS/BUILD-003.md` описывает только `PR-08..PR-12` и под инкремент не обновлялся. `beta` (`e91acf6`, 19 `src/*.rs`, 31 UI feature) и `main` (`4e24048`, 14 `src/*.rs`, 25 UI feature) отстают на всю волну.

## 1. Что вошло в инкремент

### 1.1. Rust BFF (`rust/gui-spike/src/*.rs`) — 7 модулей

| Модуль | Задача / PR | Контрактный оракул | Назначение |
|---|---|---|---|
| `expressions.rs` | `task-242` / `PR-127` | `tests/expressions_contract.rs` | каталог и активация выражений гильдии |
| `webhooks.rs` | `task-245` / `PR-131` | `tests/webhooks_contract.rs` | CRUD вебхуков канала |
| `automod.rs` | `task-249` / `PR-135` | `tests/automod_contract.rs` | правила автомодерации |
| `slash_commands.rs` | `task-253` / `PR-138` | `tests/slash_commands_contract.rs` | реестр и вызов slash-команд |
| `onboarding.rs` | `task-258` / `PR-141` | `tests/onboarding_contract.rs` | шаги онбординга гильдии |
| `soundboard.rs` | `task-259` / `PR-144` | `tests/soundboard_contract.rs` | звуки soundboard, лимиты и громкость |
| `stage_instances.rs` | `task-265` / `PR-150` | `tests/stage_instances_contract.rs` | инстансы stage-каналов и запросы на выступление |

Все семь объявлены в `rust/gui-spike/src/lib.rs` (34 `pub mod`), но ни один не экспортирован в `main.rs` как Tauri-команда.

### 1.2. UI (`rust/gui-spike/ui/features/*.js`) — 21 модуль

Контроллеры (12): `forum-view` (расширен), `poll-card`, `events-modal`, `role-editor`, `automod-editor`, `webhooks-editor`, `expressions-manager`, `moderation-panel`, `soundboard-picker`, `slash-command-picker`, `onboarding-wizard`, `stage-request-queue`.
Помощники (7): `event-banner`, `attachment-helper`, `slowmode` (расширен), `voice-quality` (расширен), `thread-badge`, `guild-boost`, `voice-sound-effects`.
Мосты (2): `shell-controllers-registry` (`FE-INT-10`) — реестр 6 панелей администрирования и маршрутизатор действий; `chat-helpers-bridge` (`FE-INT-11`) — фасад помощников заголовка канала и композера над `thread-badge`/`guild-boost`.

### 1.3. Матрица `PR-125..PR-152`

Тип: `BE` — Rust BFF, `UI-C` — контроллер (`create*Controller`), `UI-H` — чистый помощник, `UI-B` — мост.
Подключение: `IPC-нет` — нет `#[tauri::command]`; `runtime-нет` — вне собственного файла и теста модуль никто не импортирует.

| PR | Карточка | Тип | Артефакт | Тест-оракул | Ревью | Подключение |
|---|---|---|---|---|---|---|
| `PR-125` | `task-243` `FE-P2-28` | UI-H | `ui/features/event-banner.js` | `tests/unit/event-banner.test.mjs` | `154A` | `runtime-нет` |
| `PR-126` | `task-244` `FE-P2-29` | UI-H | `ui/features/attachment-helper.js` | `tests/unit/attachment-helper.test.mjs` | `155A` | `runtime-нет` |
| `PR-127` | `task-242` `BE-P1-26` | BE | `src/expressions.rs` | `tests/expressions_contract.rs` | `156A` | `IPC-нет` |
| `PR-128` | `task-240` `FE-P0-10` | UI-C | `ui/features/forum-view.js` (расширен) | `tests/unit/forum-view.test.mjs` | `157A` | `runtime-нет` |
| `PR-129` | `task-241` `FE-P1-27` | UI-C | `ui/features/poll-card.js` | `tests/unit/poll-card.test.mjs` | `158A` | `runtime-нет` |
| `PR-130` | `task-246` `FE-P2-30` | UI-H | `ui/features/voice-quality.js` (расширен) | `tests/unit/voice-quality.test.mjs` | `159A` | `runtime-нет` |
| `PR-131` | `task-245` `BE-P1-27` | BE | `src/webhooks.rs` | `tests/webhooks_contract.rs` | `160A` | `IPC-нет` |
| `PR-132` | `task-247` `FE-P0-11` | UI-C | `ui/features/events-modal.js` | `tests/unit/events-modal.test.mjs` | `161A` | `runtime-нет` |
| `PR-133` | `task-248` `FE-P0-12` | UI-C | `ui/features/role-editor.js` | `tests/unit/role-editor.test.mjs` | `162A` | `runtime-нет` |
| `PR-134` | `task-250` `FE-P2-31` | UI-H | `ui/features/slowmode.js` (расширен) | `tests/unit/slowmode.test.mjs` | `163A` | `runtime-нет` |
| `PR-135` | `task-249` `BE-P1-28` | BE | `src/automod.rs` | `tests/automod_contract.rs` | `164A` | `IPC-нет` |
| `PR-136` | `task-251` `FE-P0-13` | UI-C | `ui/features/automod-editor.js` | `tests/unit/automod-editor.test.mjs` | `165A` | `runtime-нет` |
| `PR-137` | `task-252` `FE-P0-14` | UI-C | `ui/features/webhooks-editor.js` | `tests/unit/webhooks-editor.test.mjs` | `166A` | `runtime-нет` |
| `PR-138` | `task-253` `BE-P1-29` | BE | `src/slash_commands.rs` | `tests/slash_commands_contract.rs` | `167A` | `IPC-нет` |
| `PR-139` | `task-254` `FE-P2-32` | UI-H | `ui/features/thread-badge.js` | `tests/unit/thread-badge.test.mjs` | `168A` | `runtime-нет` |
| `PR-140` | `task-255` `FE-P2-33` | UI-H | `ui/features/guild-boost.js` | `tests/unit/guild-boost.test.mjs` | `169A` | `runtime-нет` |
| `PR-141` | `task-258` `BE-P1-30` | BE | `src/onboarding.rs` | `tests/onboarding_contract.rs` | `170A` | `IPC-нет` |
| `PR-142` | `task-256` `FE-P0-15` | UI-C | `ui/features/expressions-manager.js` | `tests/unit/expressions-manager.test.mjs` | `171A` | `runtime-нет` |
| `PR-143` | `task-257` `FE-P0-16` | UI-C | `ui/features/moderation-panel.js` | `tests/unit/moderation-panel.test.mjs` | `172A` | `runtime-нет` |
| `PR-144` | `task-259` `BE-P1-31` | BE | `src/soundboard.rs` | `tests/soundboard_contract.rs` | `173A` | `IPC-нет` |
| `PR-145` | `task-260` `FE-P0-17` | UI-C | `ui/features/soundboard-picker.js` | `tests/unit/soundboard-picker.test.mjs` | `174A` | `runtime-нет` |
| `PR-146` | `task-261` `FE-P0-18` | UI-C | `ui/features/slash-command-picker.js` | `tests/unit/slash-command-picker.test.mjs` | `175A` | `runtime-нет` |
| `PR-147` | `task-262` `FE-P0-19` | UI-C | `ui/features/onboarding-wizard.js` | `tests/unit/onboarding-wizard.test.mjs` | `176A` | `runtime-нет` |
| `PR-148` | `task-263` `FE-P1-34` | UI-H | `ui/features/voice-sound-effects.js` | `tests/unit/voice-sound-effects.test.mjs` | `177A` | `runtime-нет` |
| `PR-149` | `task-266` `FE-P1-35` | UI-C | `ui/features/stage-request-queue.js` | `tests/unit/stage-request-queue.test.mjs` | `178A` | `runtime-нет` |
| `PR-150` | `task-265` `BE-P1-32` | BE | `src/stage_instances.rs` | `tests/stage_instances_contract.rs` | `179A` | `IPC-нет` |
| `PR-151` | `task-267` `FE-INT-10` | UI-B | `ui/features/shell-controllers-registry.js` | `tests/unit/shell-controllers-registry.test.mjs` | `180A` | `runtime-нет` |
| `PR-152` | `task-268` `FE-INT-11` | UI-B | `ui/features/chat-helpers-bridge.js` | `tests/unit/chat-helpers-bridge.test.mjs` | `181A` | `runtime-нет` |

Итого: `BE` 7, `UI-C` 12, `UI-H` 7, `UI-B` 2. Ни один из 28 артефактов не имеет потребителя в рантайме — только тесты.

## 2. Показатели качества

| Метрика | Значение | Как проверено |
|---|---|---|
| Двухуровневое ревью | **28/28** PR: PM acceptance + DeepSeek Tech Review | `ACCEPT.md` и `REVIEW-{154..181}A.md` в `waves/RW-40/G{1,2}/task-*/` |
| Вердикты ревью | `APPROVED` 28/28 | поле «Вердикт» каждого `REVIEW-*A.md` |
| Блокирующие замечания | `P0 = 0`, `P1 = 0` в каждом ревью | те же документы |
| Мутации | `5/5` убиты на каждом срезе (у части срезов дополнительные, напр. `7/7` у `PR-145`) | раздел «Мутации» каждого `REVIEW-*A.md`, логи `mutations`/`MUTATION-*` |
| Целостность после мутаций | побайтовое восстановление, `md5` совпадает с `REPORT.md` | там же |
| Rust-тесты | 42 файла, из них 39 `*_contract.rs` | `git ls-tree unstable rust/gui-spike/tests/` |
| Node unit-тесты | 112 файлов (+18 новых, 3 расширены) | `git ls-tree unstable tests/unit/` |
| UI feature-модули | 95 (+18 новых, 3 расширены) | `git ls-tree unstable rust/gui-spike/ui/features/` |
| Rust BFF-модули | 35 без `lib.rs`/`main.rs` (+7) | `git ls-tree unstable rust/gui-spike/src/` |
| Tauri-команды | **52** (без изменений за окно) | `grep -c '#\[tauri::command\]' main.rs` |
| Тестовые файлы не правились под код | тест-оракулы независимы (контракт/`md5`/RE-дамп) | `REPORT.md`/`REVIEW-*A.md` по каждой карточке |

## 3. Известные разрывы интеграции (блокируют гейт)

1. **P1. 7 BFF-доменов недостижимы из UI.** `automod`, `expressions`, `webhooks`, `slash_commands`, `onboarding`, `soundboard`, `stage_instances` объявлены в `lib.rs`, но `main.rs` не содержит ни одной `#[tauri::command]` для них — число команд не изменилось (52 до и после волны). Вызов через `invoke` невозможен.
2. **P1. 21 UI-модуль не смонтирован.** `git grep` по `rust/gui-spike/ui/**` находит каждый новый модуль только в его собственном файле; `app.js` (29 `import`) импортирует `dispatch-bridge`, но не `shell-controllers-registry` и не `chat-helpers-bridge`. В `shell-bridge.js` `SHELL_MODAL_LAYERS` по-прежнему заморожен на 10 слоях и новых id панелей не содержит.
3. **P2. Манифест сборки устарел.** `waves/RW-40/BUILDS/BUILD-003.md` на `testing` перечисляет `PR-08..PR-12`, `BUILD-004.md` — `PR-13..PR-17`. Инкремент `PR-125..PR-152` в манифестах не отражён, поэтому QA не получал актуального списка задач и инструкций `T2`.
4. **P2. `QA-BUILD-003-REPORT.md` отсутствует** на всех четырёх ветках; ветка `qa/task-272-qa-build-003-report` стоит на `11070f0` (тот же коммит, что `testing`) и отчёта не содержит. Вердикта G4 QA нет.
5. **P2. `beta`/`main` отстают на всю волну.** `beta` = `e91acf6` (19 `src/*.rs`, 31 UI feature, 40 unit-тестов), `main` = `4e24048` (14 / 25 / 34). Перенос `testing -> beta` и репетиция отката не выполнялись.

## 4. План финального продвижения `Build-003`

### Фаза 0 — закрыть разрывы (G1 + G2), без неё сценарии QA недостижимы

- [ ] G1: экспортировать 7 BFF-доменов через `#[tauri::command]` в `main.rs`, расширить `tests/ipc_wiring_contract.rs` новыми командами как оракулом (`IPC-нет -> 0`).
- [ ] G2: импортировать `createShellFeatureBridge`/`shell-controllers-registry` в `ui/app.js`, зарегистрировать 12 контроллеров, расширить `SHELL_MODAL_LAYERS` **вместе** с `tests/unit/shell-bridge.test.mjs` (список слоёв + кейс открытия нового слоя) — правка теста «под код» без нового кейса запрещена (`TEST-POLICY.md`).
- [ ] G2: смонтировать 7 помощников: `event-banner`/`guild-boost`/`thread-badge` — в список каналов, `slowmode`/`attachment-helper`/`slash-command-picker` — в composer, `voice-quality`/`voice-sound-effects` — в `voice.js`; `chat-helpers-bridge` — в шапку канала.
- [ ] G6: `MERGE-READINESS-006.md` с нулевыми разрывами.

### Фаза 1 — QA-гейт `T0..T4` и вердикт G4

- [ ] Обновить `waves/RW-40/BUILDS/BUILD-003.md`: полный список `PR-125..PR-152`, инструкции `T2` по сценариям automod/webhooks/expressions/slash/onboarding/soundboard/stage.
- [ ] `T0`: `node scripts/test-impact.mjs` по затронутым модулям (18 новых unit-файлов + 7 контрактов).
- [ ] `T1`: `npm run typecheck`, `npm run test:meta` (102/102), `npm run test:unit` (112 файлов), `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --workspace` (42 файла `gui-spike`, из них 39 контрактных).
- [ ] `T2`: прогон сборки `testing` по сценариям волны на живом окне после Фазы 0.
- [ ] `T4`: `npm run gate:release` (`failOnSkips: true`, `flaky_rate = 0%`).
- [ ] Выпустить `waves/RW-40/QUALITY/QA-BUILD-003-REPORT.md` с вердиктом `P0..P3`; условие перехода — `P0 = 0`, `P1 = 0`, `Pass P0/P1 = 100%`, `lines >= 90%`, `branches >= 90%`, `functions >= 80%`, мутации `5/5`.

### Фаза 2 — `testing -> beta`

- [ ] Слить `testing` (`11070f0` + принятые фиксы) в `beta` (`e91acf6`) через PR, без прямых коммитов.
- [ ] Повторить на `beta`: `perf_contract`, `gate:release` с `failOnSkips: true`, проверка `SHA256SUMS`/SBOM под Build-003.
- [ ] Репетиция отката: возврат `beta` на `e91acf6` и подтверждение, что сборка поднимается без `unstable`.

### Фаза 3 — `beta -> main` и релиз

- [ ] PR `beta -> main` (`main` = `4e24048`) после `QA Passed` и чистого `beta`-прогона.
- [ ] Поднять версию, поставить git-тег, пересобрать `scripts/release.sh` + `scripts/sbom.mjs` + `SHA256SUMS`.
- [ ] Обратная синхронизация `main -> unstable -> testing -> beta`, удаление рабочих веток, запись `waves/db.jsonl` и `waves/STATE.md`.

## 5. Риски

- **P1.** Без Фазы 0 инкремент остаётся библиотекой модулей: 28 артефактов покрыты тестами, но пользовательских сценариев не добавляют. Unit-тесты этого не покажут.
- **P1.** `SHELL_MODAL_LAYERS` заморожен на 10 слоях; добавление 6 панелей требует парной правки кода и теста — иначе либо red, либо ослабление теста (`TEST-POLICY.md`, `P0`).
- **P2.** Устаревшие манифесты `BUILD-003`/`BUILD-004` дают QA неверный объём проверки.
- **P2.** `beta` и `main` отстают на 28 PR: перенос и откат не репетированы, конфликтов слияния не проверяли.
- **P3.** `PR-123` (`scheduled_events`, вне окна) связан с `event-banner`/`events-modal`; при следующем срезе паритета учитывать вместе.

## 6. Воспроизводимость

```sh
git rev-parse --short unstable testing beta main            # b35c3e1 / 11070f0 / e91acf6 / 4e24048
git rev-list --count ebdb966..unstable                      # 76
git ls-tree --name-only unstable rust/gui-spike/src/ | wc -l            # 37 файлов, 35 модулей без lib.rs/main.rs
git ls-tree --name-only unstable rust/gui-spike/ui/features/ | wc -l    # 95
git ls-tree --name-only unstable rust/gui-spike/tests/ | wc -l          # 42 (39 *_contract.rs)
git ls-tree --name-only unstable tests/unit/ | wc -l                    # 112
git show unstable:rust/gui-spike/src/main.rs | grep -c '#\[tauri::command\]'  # 52
git show unstable:rust/gui-spike/src/lib.rs | grep -c '^pub mod'        # 34
git diff --name-status ebdb966 unstable -- rust/gui-spike/src/ rust/gui-spike/ui/features/ tests/unit/
git grep -l "chat-helpers-bridge" unstable -- 'rust/gui-spike/ui/**'    # только сам модуль
git cat-file -e testing:waves/RW-40/QUALITY/QA-BUILD-003-REPORT.md      # отсутствует
```

## 7. Источники

`waves/RW-40/G6/task-264-merge-readiness-005/MERGE-READINESS-005.md`; `waves/RW-40/G1/task-{242,245,249,253,258,259,265}-*/{ACCEPT.md,REVIEW-*A.md}`; `waves/RW-40/G2/task-{240,241,243,244,246,247,248,250,251,252,254,255,256,257,260,261,262,263,266,267,268}-*/{ACCEPT.md,REVIEW-*A.md}`; `waves/RW-40/BUILDS/BUILD-003.md`, `BUILD-004.md` (ветка `testing`); `rust/gui-spike/src/lib.rs:285-288`; `rust/gui-spike/src/main.rs`; `rust/gui-spike/ui/app.js`; `rust/gui-spike/ui/features/shell-bridge.js`; `waves/STATE.md`.
