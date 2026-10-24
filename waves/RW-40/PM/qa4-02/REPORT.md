# QA4-02 — gate запускает Rust workspace (не только gui-spike)

- batch/карточка: RW-40 / QA4-02 (QA4-02 §8.1 п.1, G-1 `rust-workspace`), PM — DeepSeek.
- дата: 2026-10-24. Live/сеть/окно/commit не использовались.

## сделано

1. `scripts/gate.mjs`: в `commonPhases` (значит и worker, и release/qualify) добавлена фаза
   `workspace-rust`:
   `cargo test --offline --workspace`, `cwd: "rust"`, `timeoutMs: 900000`, `countsKind: "cargo"`.
   gui-spike-фаза `spike-rust` осталась отдельной и не изменена.
2. Парсер счётчиков cargo (`cargoTestCounts`): суммирует
   `test result: ... N passed; M failed; K ignored` по всем бинарникам workspace;
   `tests = pass + fail + ignored`. Для `workspace-rust` счётчики попадают в отчёт гейта.
   `spike-rust` оставлен как был (`counts: null`), чтобы не менять его поведение.
3. `tests/meta/test-policy.test.mjs` (аддитивно): новый `it` «гейт запускает Rust workspace
   целиком, а не только gui-spike». Тест разбирает объекты фаз (а не ищет подстроку «где-то»)
   и требует: `workspace-rust` с `cargo`, `--offline`, `--workspace`, `timeoutMs: 900000`,
   `cwd: "rust"`/`--manifest-path rust/Cargo.toml`, без `gui-spike` внутри; `spike-rust`
   по-прежнему с `gui-spike/Cargo.toml`; `workspace-rust` находится в `commonPhases`.
4. `README.md` не менялся: docs-sync проверяет только команды CLI из HELP, раздел про фазы
   гейта в README отсутствует — синхронизация не требовалась (`docs-sync` зелёный).

Дедупликации нет: `rust/gui-spike/Cargo.toml` объявляет собственный `[workspace]`,
в `rust/Cargo.toml` members = `crates/*`.

## red

```
node --test tests/meta/test-policy.test.mjs
→ ✖ гейт запускает Rust workspace целиком, а не только gui-spike
  AssertionError: в gate.mjs нет фазы workspace-rust   (exit 1)
```
Лог: `waves/RW-40/PM/qa4-02/red.log` (md5 acbb4e9886465ba51d742e7b470d5010).

## green

```
node --test tests/meta/test-policy.test.mjs → 5 tests, 5 pass, 0 fail, 0 skip (exit 0)
```
Лог: `green-test-policy.log` (b7bf5ec7aee164abe93ee0c43f91758b).

## targeted

- `npm run typecheck` → чисто (exit 0), `typecheck.log` (9638c5dccefbd8652188ceed072c13d1).
- `node --test tests/meta/test-policy.test.mjs tests/meta/plan-sync.test.mjs` →
  `test-policy` 5/5 pass; `plan-sync` падает по **чужой** причине (см. ниже), `targeted.log`
  (ce55a6ecbe3a661c0c249b98ab85b2f7).
- Полный meta (`node --test 'tests/meta/**/*.test.mjs'`) → 39 pass / 1 fail, падает только
  `plan-sync`, `test-policy` зелёный: `meta-all.log` (a39a4a444b4d06eb984d40ce32fe116e).

## gate:release (2026-10-24T07:33:03Z)

`npm run gate:release` → **failed** (exit 1), лог `gate-release.log`
(48083decdd007e429d37df9a067ada2f), отчёт `.gate/release-2026-10-24T07-33-03-206Z.json`.

| фаза | ok | counts |
|---|---|---|
| typecheck | ✔ | — |
| ui-tsc | ✔ | — |
| meta | ✖ | 40: 39 pass / 1 fail (plan-sync) |
| unit | ✔ | 400: 400 pass / 0 fail / 0 skip |
| spike-rust | ✔ | — (как раньше) |
| **workspace-rust** | ✔ | **1112: 1112 pass / 0 fail / 0 skip** |
| smoke | ✔ | 22: 22 pass |
| e2e | ✔ | 4: 0 pass / 0 fail / **4 skip** |
| coverage | ✖ | 400 pass, но line 79.96% < 90%, branch 87.93% < 90% |
| golden | ✔ | — |

- `workspace-rust` даёт 1112 тестов — ровно baseline из `TEST-AUDIT §1` (core 574 / workspace 1112).
  Ядро `discord-core` теперь в гейте; ослаблений нет.
- e2e-поведение не менялось: 4 skip остаются `ok: true` + warning
  «e2e: пропущено 4 тестов — пропуск прохождением не считается», в pass не переведены.
  При зелёных meta/coverage вердикт был бы `passed_with_skips`.
- Оба ✖ — **не от этой карточки**: между зелёным `gate:release` в 07:22:19Z
  (`passed_with_skips`, все 9 фаз ✔) и моим прогоном в 07:33:03Z параллельная линия
  изменила CDP-файлы:
  - `tests/unit/cdp.test.mjs` (+191 строка, mtime 12:23:56) добавил
    `import { run } from "../../src/cli/run.mjs"` — из-за этого `run.mjs` впервые попал в
    coverage (line 24.94%) и обвалил общий порог до 79.96/87.93;
  - тот же файл и `src/core/cdp/client.mjs` (+118 строк, mtime 12:24:05) разошлись с §8
    `TDD-PLAN.md` по токенам — падает `plan-sync`
    (`tests/unit/cdp.test.mjs ↔ src/core/cdp/client.mjs: лишние [], не покрыты
    [captureScreenshot, clickText, ...]`).
  Мои файлы (`scripts/gate.mjs`, мета-тест) в unit/coverage не участвуют; `plan-sync`
  их не читает. Ничего не ослаблялось и не подгонялось.

## файлы + md5

| файл | md5 | статус |
|---|---|---|
| `scripts/gate.mjs` | `27d7d39e7d54ec53bdfe5ca71e0408c1` | M (+45/−4) |
| `tests/meta/test-policy.test.mjs` | `4aeb81c3c22569ab1b1a06498df04a59` | untracked, правка аддитивная (+37) |
| `README.md` | `b2bb127ba86e3f5744483425f3d5ba5b` | не менялся |

`test-policy.test.mjs` — untracked-файл параллельной линии (создан вне git до этой карточки);
правка только добавочная, но риск одновременной правки того же файла другим агентом есть.

## остаток / риски

1. **Coverage CLI — вне карточки** (QA4-03 / G-7): `src/cli/run.mjs` теперь в отчёте
   (24.94% line) из-за импорта в `tests/unit/cdp.test.mjs`; порог 90% не проходит. Нужен
   `tests/unit/cli-run.test.mjs` отдельной карточкой. В этой карточке coverage не трогался.
2. **plan-sync** красный по CDP-линии: нужна строка §8 и синхронизация токенов с
   `src/core/cdp/client.mjs` — карточка владельца этой линии, не QA4-02.
3. `gate:release` станет зелёным (`passed_with_skips`, e2e 4 skip как warning) сразу после
   закрытия п.1–2; фазы `workspace-rust`/`spike-rust`/e2e уже ведут себя корректно.
4. В `--scope worker` `workspace-rust` добавляет ~10 с (кэш тёплый) на компиляцию/прогон ядра —
   принято осознанно: PROCESS §10 требует workspace на batch-гейте.

Следующий шаг: приёмка PM/Team Lead; п.1–2 передать владельцу CDP/coverage-линии.
