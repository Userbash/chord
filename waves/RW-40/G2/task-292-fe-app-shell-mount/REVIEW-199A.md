# REVIEW-199A — Техническое ревью PR-172 (`task-292` / `FE-INT-15`: монтирование `createAppShellWiring` и `createComposerPopoversBridge` в `rust/gui-spike/ui/app.js`)

- **Ревьюер:** Технический ревьюер (DeepSeek), TR-199A
- **Проверяемая ветка:** `feature/task-292-fe-app-shell-mount`, HEAD `d7eff40` (Merge PR-168)
- **Рабочее дерево:** `/home/user/orca/workspaces/project/feature-task-292-fe-app-shell-mount`
- **Дата:** 2026-10-31
- **Вердикт:** **APPROVED** — `P0 = 0`, `P1 = 0`, мутации `5/5` убиты, `app.js` после каждой мутации восстановлен побайтно (`a97d997ba0578e184cc491fa4db4b3e9` до и после цикла)
- **Объект:** `rust/gui-spike/ui/app.js` (`+28 / −1`) и `tests/unit/app-shell-wiring.test.mjs` (`+176 / −0`)

## 1. Целостность файлов

| Файл | MD5 в `REPORT.md` | MD5 до ревью | MD5 после тестов, мутаций и cargo-прогонов |
| --- | --- | --- | --- |
| `rust/gui-spike/ui/app.js` | `a97d997ba0578e184cc491fa4db4b3e9` | совпал ✅ | `a97d997ba0578e184cc491fa4db4b3e9` ✅ |
| `tests/unit/app-shell-wiring.test.mjs` | `80ff1ccd1ac198ae4267cf985d5eabdd` | совпал ✅ | `80ff1ccd1ac198ae4267cf985d5eabdd` ✅ |

`git diff --numstat`: `28 1 rust/gui-spike/ui/app.js`, `176 0 tests/unit/app-shell-wiring.test.mjs`. Единственное удаление — прежняя однострочная строка `export { createUserPopoutController, createGuildModalsController };`, она переписана в многострочный экспорт; ни одна существующая функция `boot()` не изменена и не удалена. `git diff --check` — чисто.

## 2. Состав изменений и границы

`git status --porcelain`:

```
 D node_modules
 M package-lock.json
 M rust/gui-spike/ui/app.js
 M tests/unit/app-shell-wiring.test.mjs
```

- `app.js` — ровно три точки по ТЗ: два импорта (строки 53–54), создание `shellWiring`/`composerPopoversBridge` в `boot()` (3295–3309) и их наличие в возвращаемом объекте (3342–3343), расширенный финальный `export` (3456–3462).
- `app-shell-wiring.test.mjs` — 3 новых теста + 5 локальных хелперов; прежние 15 тестов task-271 не тронуты (`176` добавленных строк, `0` удалённых).
- `D node_modules`, `M package-lock.json` — шум дерева, к PR не относятся: симлинк `node_modules` (`120000`) отсутствует в рабочей копии, а lock отличается блоками `bin`/`engines` пакета `discord-lab` (не связаны с UI). **При merge брать только два пути, не `git add -A`** (см. `P2-5`).
- Побочный дрейф, который появился уже в ходе ревью: `rust/gui-spike/Cargo.lock` переписывается любым `cargo`-прогоном (`ruzstd` → `zstd`, добавлен `jobserver`). Я откатил его через `git checkout --`; в коммит он попасть не должен (`P2-6`).

**Ветка не содержит коммитов:** `HEAD = merge-base = d7eff40`, вся работа лежит в рабочем дереве. Поэтому `git diff unstable..HEAD` из задания сегодня показывает не PR, а отставание базы: `unstable` ушёл на 4 коммита (task-289/290/291) и diff рисует удаление `voice-diagnostics.js`, `VOICE-DIAGNOSTICS-SPEC.md`, `stats.rs` и тестов. Это не регрессия PR-172; предмет проверки — `git diff` рабочего дерева, что и сделано. После коммита diff составит ровно два файла.

## 3. Сверка с контрактом (оракул)

Источник истины — не сам тест, а объявления фасадов и реальное поведение модулей:

| Утверждение | Оракул | Результат |
| --- | --- | --- |
| `AppShellWiringOptions.onDispatchIpc?: (command, payload?) => unknown` | `features/app-shell-wiring.js:42` | ✅ прокинут в `createShellFeatureBridge` и в `onDispatchAction` роутера |
| Создание фасада не трогает DOM/IPC/таймеры | `grep document\|window\|addEventListener\|setTimeout\|setInterval` по `app-shell-wiring.js`, `shell-bridge.js`, `shell-controllers-registry.js`, `composer-popovers-bridge.js` — 0 совпадений | ✅ создание `shellWiring` на `boot()` безопасно, побочных эффектов и утечек нет |
| `createComposerPopoversBridge(options)` читает `commands`, `sounds`, `volume`, `onSelectSlashCommand`, `onPlaySoundboardSound` | `features/composer-popovers-bridge.js:117–139` | ⚠️ `onDispatchIpc` **не читается вообще** — см. `P2-1` |
| `boot()` возвращает объект типа `App` | `@typedef` `App` `app.js:2270–2300`, `currentApp()` `app.js:2309` | ⚠️ typedef не расширен — см. `P2-2` |

Поведенческий зонд (`logs/probe-199a-composer-bridge.mjs`, лог `logs/probe-199a-composer-bridge.log`): мост создан с шпионом `onDispatchIpc`, затем `updateDraft("/shrug")`, `selectSlashCommand({id,name})`, `playSoundboardSound("s1")`, `setSoundboardVolume(120)`. Результат:

```
ipcCalls: []
selected: ["shrug"]
played: [["s1",1]]
VERDICT: onDispatchIpc NEVER called (dead option)
```

То есть объявленные коллбэки работают, а `onDispatchIpc` не вызывается ни разу — опция декоративная.

## 4. Red-фаза

Red-first подтверждён независимо: я подменил `app.js` на версию из `HEAD` (без реализации), прогнал `node --test tests/unit/app-shell-wiring.test.mjs` и получил ровно то же, что в `red.log` воркера — `tests 18`, `pass 15`, `fail 3`, падения на трёх новых тестах. Лог: `logs/review-199a-red-verify.log`. После проверки файл восстановлен, MD5 `a97d997b…`.

`red.log` воркера — настоящий red: падения на `AssertionError` (`actual` — реальный текст `boot()`, `expected` — regex; `actual: ['createUserPopoutController','createGuildModalsController']` для экспорта), а не на импорте/парсинге.

## 5. Тесты и типы

| Проверка | Результат | Лог |
| --- | --- | --- |
| `node --test tests/unit/app-shell-wiring.test.mjs tests/unit/composer-popovers-bridge.test.mjs` | exit `0`: 31/31 (18 + 13) | `logs/review-199a-targeted.log`, `logs/review-199a-targeted-final.log` |
| `node --test tests/unit` | exit `0`: **1941/1941**, skip `0` | `logs/review-199a-unit-all.log` |
| `npm run typecheck` (`tsc -p tsconfig.json && tsc -p tsconfig.agent.json`) | exit `0`, 0 ошибок | `logs/review-199a-typecheck-root.log` |
| `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` | exit `0`, 0 ошибок | — |
| `npm run gate:worker` (с `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`) | verdict `failed`: `typecheck`, `ui-tsc` ✅, `meta` 102/102 ✅, `unit` 1941/1941 ✅, `smoke` 19 pass / 2 fail / 1 skip (2 fail — нет бинарника GUI), `spike-rust`/`workspace-rust` — красные на базе | `logs/review-199a-gate-worker.log`, `.gate/worker-2026-10-31T07-20-45-799Z.json` |

Что реально проверяют 3 новых теста (по исходнику `app.js` через `readFile`):

1. импорт `createAppShellWiring` из `./features/app-shell-wiring.js` и `createComposerPopoversBridge` из `./features/composer-popovers-bridge.js`;
2. внутри тела `boot()` (вырезается от `function boot(` до `try {\n  app = boot();`) созданы `shellWiring` и `composerPopoversBridge`, у обоих `onDispatchIpc: (command, payload) => invoke(command, /** @type … */ (payload))`, и оба имени есть в объекте, который возвращает `boot()`;
3. финальный `export { … }` содержит оба фасада рядом с `createUserPopoutController` и `createGuildModalsController` (`deepEqual` по порядку).

Проверки в `gate:worker` красные не из-за PR:

- `spike-rust`: `ipc_wiring_contract::every_ui_invoke_command_in_js_sources_is_registered_in_tauri_main` — `37 != 38`, не найдена `toggle_reaction` (вызывается как `invoker("toggle_reaction")` в `features/reactions.js:305`). Я воспроизвёл это на **версии `app.js` из `HEAD`** — падение идентично, к диффу отношения не имеет.
- `workspace-rust`: `cargo test -p discord-core --test dispatch_types` не компилируется — `E0432`: `discord_core::dispatch::{RoutedStreamDispatch, StreamDispatchKind, route_stream_dispatch, route_stream_dispatch_with_diagnostics}` не существуют (`logs/review-199a-baseline-rust-red.log`, exit `101`). Те же символы отсутствуют и в `unstable` (`git grep` пуст), то есть `cargo test --workspace` красный на всём батче, а не из-за PR-172.
- `smoke`: `tests/smoke/gui-spike.smoke.test.mjs:30` и `gui-window.smoke.test.mjs:35` требуют `rust/gui-spike/target/debug/discord-gui-spike`; бинарник в рабочем дереве не собран (`CARGO_TARGET_DIR` указывал на общий кэш). Окружение, не код.

## 6. Мутации

Методика: `python3`-замена с проверкой уникальности шаблона (иначе `ABORT`), прогон `node --test tests/unit/app-shell-wiring.test.mjs`, откат из снимка в памяти с контролем MD5. Харнесс: `mutations/mutate-199a.py`, логи `mutations/M1.log … M5.log`, сводка `mutations/run.log`.

| # | Мутация | Итог | Убивший тест |
| --- | --- | --- | --- |
| M1 | путь импорта `./features/app-shell-wiring.js` → `./features/shell-wiring.js` | убит | `app.js: импортирует createAppShellWiring и createComposerPopoversBridge из ./features` |
| M2 | `shellWiring.onDispatchIpc` → `() => undefined` (IPC не вызывается) | убит | `app.js: создаёт shellWiring и composerPopoversBridge внутри boot() с onDispatchIpc -> invoke` |
| M3 | `boot()` больше не возвращает `shellWiring` | убит | тот же тест (проверка ключей возврата) |
| M4 | `composerPopoversBridge.onDispatchIpc` → `(payload),` (вместо `invoke(...)`) | убит | тот же тест (regex требует `invoke(`) |
| M5 | финальный `export` без `createComposerPopoversBridge` | убит | `app.js: реэкспортирует createAppShellWiring и createComposerPopoversBridge вместе с прежними фабриками` |

**Итого `5/5` (100%).** После каждой мутации и в конце: `a97d997ba0578e184cc491fa4db4b3e9` — файл идентичен исходному, `FINAL … (base a97d997b…)`.

Мутации по `tests/unit/composer-popovers-bridge.test.mjs` не требовались: его поведение PR не меняет.

## 7. Замечания без блокировки (`P2`)

### P2-1. `onDispatchIpc` для `createComposerPopoversBridge` — no-op
`ComposerPopoversBridgeOptions` (`composer-popovers-bridge.js:53–59`) объявляет только `sounds`, `commands`, `volume`, `onSelectSlashCommand`, `onPlaySoundboardSound`; строки `onDispatchIpc` в модуле нет вообще (`grep` — 0 совпадений). Переданная опция молча игнорируется, что подтверждено поведенческим зондом (§3). Приведение типа в `app.js:3303–3308` (`NonNullable<Parameters<…>[0]> & { onDispatchIpc: … }`) нужно только чтобы пройти `tsc` — оно маскирует расхождение контрактов, а не решает его.

Причина — в ТЗ: `TASK.md` L33–36 прямо предписывает прокинуть `onDispatchIpc` в оба моста, а расширять `composer-popovers-bridge.js` карточка запрещает (файл вне двух разрешённых). Воркер это зафиксировал (`REPORT.md`, риск 1). Формально DoD выполнен, поведенчески мост IPC не получил.

**Рекомендация:** отдельная карточка «`composer-popovers-bridge`: принять `onDispatchIpc` или заменить его на реальные `onSelectSlashCommand`/`onPlaySoundboardSound` → IPC-команды». Если PM считает рабочий IPC-мост обязательным уже сейчас — это `P1`, и PR-172 надо вернуть; по тексту текущего DoD блокировки нет.

### P2-2. Typedef `App` не расширен — смонтированные фасады недоступны типизированно
`@typedef App` (`app.js:2270–2300`) перечисляет 24 поля и заканчивается на `userPopout`/`guildModals`; `shellWiring` и `composerPopoversBridge` в него не добавлены. `currentApp(): App | null` (`app.js:2309`) — единственная типизированная точка доступа к собранному приложению для диагностики и поведенческих тестов, поэтому обратиться к новым фасадам из тестов/отладки нельзя без правки typedef: `app.shellWiring` — ошибка типа. `tsc` этого не ловит, потому что присваивание `app = boot()` проверяется на совместимость, а лишние свойства не запрещены.

Файл `app.js` входит в разрешённые двум, так что правка была возможна в рамках карточки.

**Рекомендация:** добавить два поля в `App` (`shellWiring: ReturnType<typeof createAppShellWiring>`, `composerPopoversBridge: ReturnType<typeof createComposerPopoversBridge>`) — 2 строки, и моунт станет реально достижимым.

### P2-3. Тесты проверяют текст, а не поведение
Три новых теста читают `app.js` как строку и матчат regex/`slice` по балансу скобок. Следствия:

- они не поймали бы `P2-1` (no-op опция проходит все проверки — это и есть причина, почему зелёный прогон не доказывает работу IPC);
- они ломаются от безобидного рефакторинга: вынести колбэк в `const dispatchIpc = (command, payload) => invoke(...)` и передать `onDispatchIpc: dispatchIpc` — поведение то же, тесты красные; переформатировать `onDispatchIpc:` — тоже;
- `BOOT_END = "\ntry {\n  app = boot();"` и `lastIndexOf("return {")` привязаны к текущей разметке хвоста файла.

Подход задан карточкой (`TASK.md` L19 требует проверять именно исходник через `readFile`), поэтому это не дефект исполнения. Но как оракул эти тесты слабее поведенческих.

**Рекомендация:** добавить один поведенческий тест поверх `currentApp()` (после `P2-2`): поднять `boot()` на DOM-шиме, дернуть `openAdminPanel("moderation")`/`dispatchAdminAction(...)` и проверить, что фейковый `tauri.core.invoke` получил `guild_admin:<panel>:<action>`. Такой тест убивает no-op без привязки к форматированию.

### P2-4. База `d7eff40` красная для Rust-фаз гейта
`npm run gate:worker` не может стать зелёным ни на одном PR от этой базы: `spike-rust` падает на `toggle_reaction` (37/38), `workspace-rust` — на отсутствующих `discord_core::dispatch::route_stream_dispatch*` (`E0432`, 21 ошибка). Обе поломки воспроизводятся без изменений PR (для `spike-rust` — прогон на `app.js` из `HEAD`; для `workspace-rust` — символы отсутствуют и в `unstable`). Похоже, sync-коммит `7d59fbf` завёл тесты раньше реализации.

**Рекомендация:** интеграционному лейну/PM — отдельная карточка на достройку `discord_core::dispatch` (stream-routing) и на `toggle_reaction` в списке UI-команд; до этого `gate:worker` по Rust-фазам не является сигналом о качестве PR-172.

### P2-5. Merge только двух путей; ветка отстаёт от `unstable`
Координатору: коммитить `rust/gui-spike/ui/app.js` и `tests/unit/app-shell-wiring.test.mjs`, не `git add -A` (иначе уедет `M package-lock.json` и `D node_modules`). Перед PR ветку стоит перебазировать на актуальный `unstable` (`648ba83`): там уже лежат `voice-diagnostics.js` и тест task-290, и `test-impact.mjs` пересчитает затронутые модули по новой базе. Merge-конфликтов не ожидается — пересечений по файлам нет.

### P2-6. `rust/gui-spike/Cargo.lock` дрейфует от любого cargo-прогона
Любой `cargo test` в `rust/gui-spike` переписывает lock (`ruzstd` → `zstd`, добавляется `jobserver`/`libc`) — то есть lock в репозитории не соответствует `Cargo.toml`. Я откатил файл; в коммит он не входит.

**Рекомендация:** отдельная карточка DevOps на синхронизацию lock-файла, иначе каждый воркер будет тащить этот шум в `git status`.

## 8. Замечания (`P3`)

- **P3-1. `tests/unit/ui-boot.test.mjs` из `TASK-TR-199A.md` не существует** (ни в рабочем дереве, ни в `unstable`). `node --test` в Node 26 при списке из существующих и отсутствующего файла молча игнорирует отсутствующий и выходит с `0`; одиночный запуск даёт exit `1`. Поэтому проверка выполнена по двум реальным файлам плюс полный `tests/unit` (1941/1941). Команду в `TASK-TR-199A.md` стоит поправить.
- **P3-2. `REPORT.md` занижает номера строк:** указано «3292–3323» и «3435», фактически создание — 3295–3309, экспорт — 3456–3462. На содержание не влияет.
- **P3-3. Дублирование колбэка:** один и тот же `onDispatchIpc` написан дважды (и в третий раз — в типе). Локальная `const dispatchIpc = (command, payload) => invoke(command, payload)` убрала бы копипасту; тесты, впрочем, привязаны к инлайн-форме (`P2-3`), так что менять надо вместе с ними.
- **P3-4. `destroy()` не вызывается** ни для `shellWiring`, ни для `composerPopoversBridge` при выходе из аккаунта. Утечки нет: оба модуля не подписываются на DOM/таймеры (`grep` — 0 совпадений), а `shellWiring.destroy()` идемпотентен. Как только появятся подписки — teardown обязателен.
- **P3-5. `composerPopoversBridge` создан без `commands`/`sounds`/`volume`** — мост пустой: `getFilteredSlashCommands()` вернёт `[]`. Данные в него карточка не требует прокидывать, но до появления этой проводки поповеры работать не будут.
- **P3-6. Тест фиксирует текст type-каста** (`/** @type {Record<string, unknown> | undefined} */`) — смена аннотации на эквивалентную сломает тест без изменения поведения. Тот же корень, что `P2-3`.

## 9. Соответствие DoD

| Критерий | Статус |
| --- | --- |
| Изменены только `app.js` и `app-shell-wiring.test.mjs` | ✅ `+28/−1` и `+176/−0`; прочий `git status` — шум дерева |
| Импорты из `./features/app-shell-wiring.js` и `./features/composer-popovers-bridge.js` | ✅ строки 53–54 |
| Создание `shellWiring` и `composerPopoversBridge` внутри `boot()` с `onDispatchIpc → invoke` | ✅ строки 3295–3309 (для моста опция фактически no-op — `P2-1`) |
| Оба экземпляра в объекте, который возвращает `boot()` | ✅ строки 3342–3343 |
| Реэкспорт обоих фасадов в финальном `export` | ✅ строки 3456–3462 |
| Red-first настоящий | ✅ независимый replay на версии из `HEAD`: 15 pass / 3 fail |
| Targeted tests | ✅ 31/31 (18 + 13) |
| `npm run typecheck` и `ui/tsconfig.json --noEmit` | ✅ 0 ошибок |
| Полный `tests/unit` | ✅ 1941/1941 |
| `meta` (гейт) | ✅ 102/102 |
| Мутации `5/5` | ✅ 100%, откат побайтный (`a97d997b…` до и после) |
| `P0 = 0`, `P1 = 0` | ✅ (`P2-1` — к обсуждению PM, см. §7) |
| `git commit/push` не выполнялся | ✅ (HEAD `d7eff40`, коммитов в ветке нет) |

## 10. Артефакты

```
waves/RW-40/G2/task-292-fe-app-shell-mount/REVIEW-199A.md                       — этот отчёт
waves/RW-40/G2/task-292-fe-app-shell-mount/logs/review-199a-red-verify.log      — replay red на версии app.js из HEAD (15 pass / 3 fail)
waves/RW-40/G2/task-292-fe-app-shell-mount/logs/review-199a-targeted.log        — targeted по команде из TASK-TR-199A
waves/RW-40/G2/task-292-fe-app-shell-mount/logs/review-199a-targeted-final.log  — финальный targeted 31/31 после мутаций
waves/RW-40/G2/task-292-fe-app-shell-mount/logs/review-199a-unit-all.log        — node --test tests/unit, 1941/1941
waves/RW-40/G2/task-292-fe-app-shell-mount/logs/review-199a-typecheck-root.log  — npm run typecheck, exit 0
waves/RW-40/G2/task-292-fe-app-shell-mount/logs/review-199a-gate-worker.log     — npm run gate:worker (meta/unit зелёные, Rust/smoke — база)
waves/RW-40/G2/task-292-fe-app-shell-mount/logs/review-199a-baseline-rust-red.log — E0432 в discord-core на базе
waves/RW-40/G2/task-292-fe-app-shell-mount/logs/probe-199a-composer-bridge.mjs  — поведенческий зонд моста
waves/RW-40/G2/task-292-fe-app-shell-mount/logs/probe-199a-composer-bridge.log  — onDispatchIpc не вызывается ни разу
waves/RW-40/G2/task-292-fe-app-shell-mount/mutations/mutate-199a.py             — харнесс с откатом и MD5-контролем
waves/RW-40/G2/task-292-fe-app-shell-mount/mutations/run.log                    — сводка 5/5, MD5 до/после
waves/RW-40/G2/task-292-fe-app-shell-mount/mutations/M1.log … M5.log            — логи мутантов
```

**Вердикт: APPROVED.** `P0 = 0`, `P1 = 0`, мутации `5/5`, оба файла побайтно совпадают с `REPORT.md`. Следующий статус — `Ready for Merge` с оговорками координатору: коммитить только два пути, ветку перебазировать на `unstable`, `composerPopoversBridge.onDispatchIpc` завести отдельной карточкой (`P2-1`), typedef `App` расширить при следующей правке `app.js` (`P2-2`).
