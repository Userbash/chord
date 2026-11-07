# task-125 — test-impact v2 (RW-40, G4 QA)

- **Ветка:** `qa/task-125-test-impact-v2` (от `unstable`, HEAD `4e24048`)
- **Дата:** 2026-10-30, проход 1 — 20:40–20:55, проход 2 (после ревью 44A) — 21:10–21:40,
  проход 3 (после ревью 44B) — 21:45–22:20
- **Файлы карточки:** `scripts/test-impact.mjs`, `tests/unit/test-impact.test.mjs`
- **Оракул:** `TEST-STRATEGY-V2.md` §5.2.1–§5.2.3, §8.1; реальный layout репозитория
  (`git ls-files`, `rust/gui-spike/tests/*.rs`, `rust/gui-spike/src/lib.rs`), `BOARD.md`
  (`rust/gui-spike/ui/features/<mod>.js`, task-113).

## Проход 3 (после ревью 44B): что исправлено

| Замечание | Было (проход 2) | Стало (проход 3) |
|---|---|---|
| P1-1 тест зашивал снимок ветки | `assert.deepEqual(onDisk, [три файла])`; на дереве с шестью `features/*.js` — красный | инвариант `layoutProblems`: модуль либо на диске, либо помечен запланированным; пометки не ссылаются на ключи вне таблицы; «ровно N файлов» не проверяется |
| P2-1 неизвестный путь | `typecheck` + `plan-sync` (T0), тест требовал ровно 2 команды | режим `all: true`: `FULL_RUN` + tier `T2 (release)` |
| P2-2 `ui/app.js` вне safety net | `frontend` + геометрия, T1 | safety net §5.2.3: `FULL_RUN` + геометрия (в `npm test` её нет), T2 |
| P2-3 команда на отсутствующий тест | `features/chat.js` всегда давал `node --test tests/unit/attachments-render.test.mjs` | профильный тест попадает в план только при наличии файла (`testOnDisk`); отсутствующие помечены в тесте как запланированные |
| P3-2 baseline red | «tests 45, pass 18, fail 27» без указания селектора | указано: селектор прохода 1 с плоскими ключами `ui/<mod>.js`; отсюда расхождение с независимым прогоном ревьюера (19/26) |

P3-1 (сверка whitelist с внешним источником) не закрыт: `BOARD.md` и `TEST-STRATEGY-V2.md`
лежат вне отслеживаемого дерева (`waves/` не в git рабочего дерева), поэтому тест не может
читать их как оракул. Вместо этого инвариант ловит опечатку в ключе таблицы (тест
«фантомный ключ и плоский путь ловятся инвариантом»), а имена запланированных файлов
сверяются с §5.2.1 вручную.

## Граф UI: `features/<mod>.js` → профильный тест + общие контракты

| Модуль | Тесты таблицы |
|---|---|
| `ui/features/notifications.js` | `notifications-ui`*, `gui-spike-dom`, `gui-spike-features` |
| `ui/features/voice-devices-settings.js` | `voice-devices-settings`*, `gui-spike-dom`, `gui-spike-features` |
| `ui/features/dm-friends.js` | `dm-friends-ui`*, `gui-spike-dom`, `gui-spike-features` |
| `ui/features/search.js` | `search-remote-real-index`, `gui-spike-dom`, `gui-spike-features` |
| `ui/features/search-overlay.js` | `search-remote-real-index`, `gui-spike-dom`, `gui-spike-features` |
| `ui/features/reactions.js` | `chat-reactions`, `gui-spike-dom`, `gui-spike-features` |
| `ui/features/chat.js` | `attachments-render`*, `channel-switch`, `gui-spike-features`, `gui-spike-model` |

`*` — файла в этой ветке ещё нет (смежные карточки). Команда на него в план не попадает:
`testOnDisk` проверяет `existsSync` перед выдачей строки. Как только файл появится, он
автоматически войдёт в план.

Tier — `T0 (targeted)`: ни геометрия, ни полный `FRONTEND` в план не попадают. Профильные
`chat-reactions.test.mjs` (импорт `features/reactions.js:31`) и
`search-remote-real-index.test.mjs` существуют и проходят (40/40 вместе с `gui-spike-dom`).

## Граф Rust BFF: `src/<mod>.rs` → контрактные таргеты

`cargo test --offline --manifest-path rust/gui-spike/Cargo.toml --test <mod>_contract --test ipc_contract`,
полный прогон — только для `lib.rs`/`main.rs`/`Cargo.toml` и для модуля без правила.

| Модуль | Контракт |
|---|---|
| `auth`, `data`, `guilds`, `invites`, `messages`, `net`, `reactions`, `rest_failure`, `search`, `typing` | `<mod>_contract` |
| `settings_store` | `settings_contract` (имя файла `tests/settings_contract.rs`) |
| `core_bridge` | `core_bridge` (файл `tests/core_bridge.rs`, не `_contract`) |

Все 12 контрактных таргетов существуют в `rust/gui-spike/tests/`; тест сверяет таблицу с
файловой системой и с `pub mod` из `lib.rs`.

**Отклонение от буквы стратегии:** §5.2.2 называет wiring-таргет `ipc_wiring_contract`, но
такого файла в крейте нет — wiring-контракт лежит в `tests/ipc_contract.rs`. Использован
реальный таргет `ipc_contract`, чтобы команда из плана не падала.

## Safety net §5.2.3

`package.json`, `package-lock.json`, `scripts/*`, `rust/gui-spike/Cargo.toml`, `rust/Cargo.toml`,
`rust/gui-spike/src/lib.rs`, `rust/gui-spike/src/main.rs`, `rust/gui-spike/ui/app.js` → tier
`T2 (release)`:

```
npm run typecheck
node --test tests/unit/test-impact.test.mjs
npm test
cargo test --offline --manifest-path rust/gui-spike/Cargo.toml
cargo test --offline --manifest-path rust/Cargo.toml --workspace
```

Для `ui/app.js` дополнительно `node waves/RW-39/PM/w39c/geometry-check.mjs`: `npm test`
не содержит этот скрипт, поэтому иначе правка общего UI-файла потеряла бы проверку
геометрии.

Неизвестный путь (`LICENSE`) с прохода 3 тоже даёт полный прогон (`all: true`), как требует
§5.2.3. Раньше он уходил в `typecheck` + `plan-sync`, то есть без unit- и Rust-прогона.

## Baseline красного прогона (P3-2)

`red2.log` прохода 2 снимался не с `HEAD`, а с селектора прохода 1: 45 tests, 18 pass,
27 fail. Независимый прогон ревьюера на `HEAD` дал 19/26. Разница — ровно один кейс
«фантомный плоский путь»: в red прохода 2 он падал из-за плоских ключей `ui/<mod>.js`,
которых на `HEAD` уже не было. Это не расхождение цифр, а разные baseline.

В проходе 3 сняты два красных прогона:

- `red3-pass2selector.log` — новый тест против селектора прохода 2 (четыре правки откатаны):
  exit 1, 48 tests, **41 pass / 7 fail**. Падают ровно кейсы P2-1 (`LICENSE`), P2-2
  (`app.js`), P2-3 (`notifications`, `voice-devices-settings`, `dm-friends`, `chat.js`) и
  дубль `app.js` из safety net.
- `red3-snapshot-p11.log` — снимок `deepEqual(onDisk, [три файла])` из прохода 2 на дереве
  с шестью `features/*.js`: exit 1, в диффе три лишних модуля. Это и был P1-1.

Дополнительно `red3-headselector.log` — новый тест против селектора `HEAD`: exit 1,
48 tests, 20 pass / 28 fail (в `HEAD` ещё нет таблиц прохода 2, поэтому красных больше).

## Green (проход 3)

- `node --test tests/unit/test-impact.test.mjs` → exit 0, **48/48**, лог `green3.log`.
- `node --test tests/unit/test-impact.test.mjs tests/meta/test-policy.test.mjs` → exit 0,
  **65/65** (48 + 17), лог `targeted3.log`.
- `integration-layout3.log` — симуляция дерева после merge: добавлены шесть
  `ui/features/*.js` и четыре профильных теста из таблицы, прогон остался **48/48**. Это
  прямое закрытие P1-1: зелёный больше не держится на отсутствии файлов.
- `profile-trio3.log` — `chat-reactions` + `search-remote-real-index` + `gui-spike-dom`
  → 40/40.
- CLI: `LICENSE` → `T2` + 4 строки полного прогона; `ui/app.js` → `T2` + полный прогон +
  геометрия; `features/chat.js` → `T0`, три существующих теста без `attachments-render`;
  `features/notifications.js` → `T0`, только `gui-spike-dom` + `gui-spike-features`.

## Гейты

- `npx tsc -p tsconfig.json` → **exit 1**, 16 ошибок TS, все в `tests/unit/dispatch_ui.test.mjs`
  (файл из `HEAD`, карточкой не тронут, лог `typecheck3.log`). В файлах карточки ошибок 0.
- `npx tsc -p tsconfig.agent.json` → exit 0 (`typecheck-agent3.log`).
- `node --test tests/meta/plan-sync.test.mjs` → exit 1, 4/5: «тестовые файлы без строки в §8» —
  внешний блокер `TDD-PLAN.md`, не дефект карточки (`plan-sync3.log`).
- `npm run gate:worker` в проходе 3 не запускался: `typecheck` и `plan-sync` красные по
  внешним причинам (чужой файл и чужой §8), полный прогон даст тот же результат. Лог
  прохода 1 (`gate-worker.log`): unit 711/711, smoke 21+1 skip, spike-rust, workspace-rust
  1106+12 ignored, ui-tsc — зелёные.

## Файлы и md5 (проход 3)

| Файл | Строк | md5 |
|---|---|---|
| `scripts/test-impact.mjs` | 311 | `bff340a33607ed596569e06ade3fd90a` |
| `tests/unit/test-impact.test.mjs` | 596 | `3beaf9f181e17ac2bd2ebb0d6ab9255f` |

`package-lock.json` изменён до начала работы (`M` в исходном `git status`) — не трогал.
Логи прохода 3 рядом с отчётом: `red3-headselector.log`, `red3-pass2selector.log`,
`red3-snapshot-p11.log`, `green3.log`, `targeted3.log`, `integration-layout3.log`,
`typecheck3.log`, `typecheck-agent3.log`, `profile-trio3.log`, `plan-sync3.log`.

## Риски

1. План теперь зависит от диска: тест попадает в него только при наличии файла. Это
   осознанно (P2-3), но означает, что опечатка в имени теста не даёт красной команды, а
   просто молча выпадает из плана. Частично ловится тестом: имя должно быть либо на диске,
   либо в `PLANNED_TESTS`, а пометки вне таблицы ломают инвариант. Полную сверку с
   `BOARD.md`/`TEST-STRATEGY-V2.md` тест не делает (файлы вне git-дерева) — P3-1 открыт.
2. Модули `ui/features/notifications.js`, `voice-devices-settings.js`, `dm-friends.js` и тесты
   `notifications-ui`, `voice-devices-settings`, `dm-friends-ui`, `attachments-render` в этой
   ветке ещё не существуют — их создают смежные карточки (task-113 и др.). До появления
   файлов правило остаётся мёртвым кодом, но план команд на них не выдаёт.
3. Зелёный `gate:worker` невозможен без чужих правок: JSDoc-типы `dispatch_ui.test.mjs` и
   строки §8 в `TDD-PLAN.md`. Нужны карточки/разрешение Team Lead.
4. Safety net `T2` тяжёлый: правка `package.json`, селектора или `ui/app.js` запускает полный
   JS- и Rust-прогон. Это требование §5.2.3, а не регрессия скорости.
