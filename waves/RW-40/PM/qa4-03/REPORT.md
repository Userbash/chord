# QA4-03 — coverage gate: unit-тесты CLI `src/cli/run.mjs`

- batch/карточка: RW-40 / QA4-03 (G-7), PM — DeepSeek.
- дата: 2026-10-24. Live Discord / окно / сеть / commit не использовались.
- CDP — только fake `fetch`/`WebSocket`; Frida — подменённый адаптер через `module.registerHooks`
  (resolve только для `import("../adapters/frida.mjs")` из `src/cli/run.mjs`). Продукт не менялся.

## сделано

1. `tests/unit/cli-run.test.mjs` (new, 38 тестов) — поведенческие тесты `run()`:
   - help (`[]`, `-h`, `--help`, флаги без команды), `--version`, неизвестная команда;
   - `summarize`: явный лог (CSV + сортировка), авто-выбор `calls-*.jsonl` из cwd, пустой cwd (1),
     нечитаемый cwd (2), нечитаемый лог (2);
   - `cdp targets` (+ HTTP 500), без подкоманды (1), `eval` (expr со пробелами, закрытие сокета),
     `eval` с исключением страницы (2), `list`, `clicktext`, `shot` (вложенный каталог, PNG),
     `shot` с несоздаваемым каталогом (2), недоступная страница (2);
   - `pids`: фильтр Discord, `--all`, пустой список, Frida недоступна (2);
   - `inventory`: неверные `--pid`/`--max-symbols` (1), Frida недоступна (2), нет процессов (0),
     ошибка attach по `--pid` (2 + счётчик пропусков);
   - `hook`: неверные `--pid/--limit/--arg-count/--duration-ms/--pattern` (1), запись вызовов
     в `--out` (tmp) и подсчёт, `hooked=0`, нет процессов, Frida недоступна, ошибка attach,
     несоздаваемый каталог лога, лог-каталог, ошибка `startHook` (2) с освобождением агента;
   - `walk`/`domains`/`static` — заглушки (2).
2. `tests/helpers/cli-capture.mjs` (new, аддитивно) — перехват stdout/stderr `run()` без процесса.
3. `tests/helpers/fake-cdp.mjs` (new, аддитивно) — глобальные fake `fetch`/`WebSocket`:
   `/json/list`, ответы на CDP-методы, запись вызовов и сокетов; реальная сеть не используется.
4. `tests/helpers/fake-frida.mjs` (new, аддитивно) — resolve-хук `node:module` + фейковый адаптер
   (`loadDevice/listProcesses/attach/collectInventory/startHook/stopHook`), поведение задаёт тест.
5. `TDD-PLAN.md` §8 — **одна строка** для нового тестового файла (требование `plan-sync`).
   Это общий файл: правка только добавочная, строка зарегистрирована; нужно подтверждение TL/PM.

`tests/unit/cdp.test.mjs` не изменялся (его текущий diff — параллельная линия), `scripts/gate.mjs`
и `src/cli/run.mjs` не изменялись (md5 совпадают с принятыми QA4-02/QA4-01a).

## red

```
npm run test:coverage → exit 1
ℹ Error: 79.96% line coverage does not meet threshold of 90%.
ℹ Error: 87.93% branch coverage does not meet threshold of 90%.
ℹ   run.mjs | 24.94 | 60.61 | 15.38
```
Лог: `red-coverage.log` (md5 `ad526b0076fa4eb463b5c274e9e1ec76`).

## green

```
node --test tests/unit/cli-run.test.mjs → 38 tests, 38 pass, 0 fail, 0 skip (exit 0)
npm run test:coverage → exit 0, 448 tests, 448 pass, 0 fail, 0 skip
```
Логи: `green-targeted.log` (`26bc9b67f98bc02167b33ecf3b2bff8f`),
`green-coverage.log` (`d6c1bc5939f6b25dbc44283ce3a2e4d9`).

| метрика | было | стало | порог |
|---|---|---|---|
| line (all files) | 79.96% | **99.74%** | 90% |
| branch (all files) | 87.93% | **94.50%** | 90% |
| functions (all files) | 88.33% | **93.33%** | 80% (факт ≥90%) |
| `src/cli/run.mjs` line | 24.94% | **100.00%** | — |
| `src/cli/run.mjs` branch | 60.61% | **98.47%** | — |

## targeted

- `npx tsc -p tsconfig.json` → чисто (exit 0), `typecheck.log` (`9638c5dccefbd8652188ceed072c13d1`).
- `npm run typecheck` (оба tsconfig) → exit 0.
- `node --test tests/unit/cli-run.test.mjs` → 38/38, 0 skip.
- Полный unit в coverage-прогоне: 448/448 pass, 0 fail, 0 skip (218+ не ослаблены, только добавлено).

## gate:release

`npm run gate:release` → **passed_with_skips** (exit 0), лог `gate-release.log`
(`e3b2417a9e7b49432382dab8120e481c`), отчёт `.gate/release-2026-10-24T08-08-32-643Z.json`.

| фаза | ok | counts |
|---|---|---|
| typecheck | ✔ | — |
| ui-tsc | ✔ | — |
| meta | ✔ | 40: 40 pass / 0 fail |
| unit | ✔ | 448: 448 pass / 0 fail / 0 skip |
| spike-rust | ✔ | — |
| workspace-rust | ✔ | 1112: 1112 pass / 0 fail |
| smoke | ✔ | 22: 22 pass |
| e2e | ✔ | 4: 0 pass / **4 skip** (нет браузера) |
| coverage | ✔ | 448 pass, пороги пройдены |
| golden | ✔ | — |

Вердикт `passed_with_skips` — только из-за 4 e2e-skip (осознанная деградация §9 TDD-PLAN);
они отражены warning-ом, в pass не переведены. Coverage/meta — зелёные.

## покрытие: что осознанно не покрыто (skip, не pass)

1. `hook --spawn` (`src/cli/run.mjs:309-312`): вызов `device.spawn(["discord"])` — это live-спавн
   Discord, запрещён карточкой; ветка не тестируется.
2. `inventory` успешная запись (`245-257`): пишет в golden `functions/` (`functionsDir()` без
   override) либо требует live-attach. Покрыт только путь attach-ошибки (2 + счётчик), записи
   в golden нет.
3. `--version` при нечитаемом `package.json` (`83-85`) и сбой динамического `import` адаптера
   (`180-182`, `213-216`, `298-302`): недостижимы без поломки модульного резолва/продукта.
4. В `run.mjs` functions 61.54% — не покрыты только error-swallow колбэки
   (`() => {}` в cleanup), `setTimeout`-колбэк `--spawn` и колбэки catch в inventory-finally;
   глобальный functions 93.33% ≥ 90%.

## файлы + md5

| файл | md5 | статус |
|---|---|---|
| `tests/unit/cli-run.test.mjs` | `272254f95517a50e857874d7c1008a46` | new (38 тестов) |
| `tests/helpers/cli-capture.mjs` | `d14167f2d3db7fe6691d87dec2991717` | new |
| `tests/helpers/fake-cdp.mjs` | `89d8921497e91637eb144f330865bfe5` | new |
| `tests/helpers/fake-frida.mjs` | `c66d6e786e595ff52d413e5df7e99279` | new |
| `TDD-PLAN.md` | `632c61834f6ee48c18f20c34265ec7c9` | +1 строка §8 (общий файл) |
| `tests/unit/cdp.test.mjs` | `376c2bff2400c88a49f9e3601c26c8f2` | не менялся этой карточкой |
| `src/cli/run.mjs` | `3b3b5554fb2690486a0bac1358881e85` | не менялся |
| `scripts/gate.mjs` | `27d7d39e7d54ec53bdfe5ca71e0408c1` | не менялся |

## остаток / риски

1. `TDD-PLAN.md` §8 — общий файл: добавлена одна регистрационная строка для нового теста
   (иначе `meta/plan-sync` красный). Требуется подтверждение TL/PM; откат — удалить строку.
2. `fake-frida.mjs` использует экспериментальный `node:module.registerHooks` (Node 24.21):
   стабилен в рамках файла теста, но при обновлении Node API стоит перепроверить.
   `mock.module` в этой версии Node недоступен без флага, поэтому выбран `registerHooks`.
3. 4 e2e-skip остаются (нет браузера) — вердикт гейта `passed_with_skips`, как и раньше.
4. Coverage-пороги не менялись, тесты не ослаблялись, `skip` в новых тестах — 0.
