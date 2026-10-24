# QA-IMPACT — контракт селектора тестов `scripts/test-impact.mjs`

- Batch/карточка: RW-40 / QA-IMPACT (воркер G4 QA)
- Дата: 2026-10-24, Node v24.21.0
- Режим: offline, без live/сети/окна, без commit
- Код продукта не менялся; правка `scripts/test-impact.mjs` — только найденные баги
  выбора тестов (селектор — инструмент, не продуктовый модуль)

## Что сделано

`tests/unit/test-impact.test.mjs` (новый, 10 тестов) фиксирует правила `planFor` независимым
оракулом (имена тестовых файлов/команд, а не текущий вывод скрипта):

| Правило | Ожидание (проверено) |
| --- | --- |
| `docs`, `docs/map.md` | `npm run typecheck` + `node --test tests/meta/docs-sync.test.mjs`; tier T0 |
| `rust/gui-spike/ui/features/join-server.js` | typecheck + `join-server.test.mjs` + `gui-spike-features.test.mjs`; **3 команды**, без полного frontend и без geometry |
| `rust/crates/discord-core` (+ файл в нём) | `cargo test --offline -p discord-core` (не workspace); tier T1 |
| `rust/gui-spike/ui/app.js` | один полный frontend-прогон (dom/model/features/guild-rail/voice/channel-switch) + `geometry-check.mjs` |
| `src/cli`, `src/cli/run.mjs` | `cli-run.test.mjs` + `cli.smoke.test.mjs`, без wildcard и без `walk` |
| неизвестный файл (`LICENSE`) | fallback = typecheck + `tests/meta/plan-sync.test.mjs` (ровно 2 команды) |
| `--full` | tier `T2` + `gate:release` + `cargo test --offline --workspace` |
| `--changed … --json` | `--json` не попадает в список changed-файлов |
| анти-live | ни в одном плане нет `live-discord`, `GUI_SPIKE_E2E`, `gui-spike.smoke`, `curl/wget/URL`, `attach`, `--online` |

Дополнительно: чужой крейт (`rust/crates/discord-platform/…`) по-прежнему идёт workspace-прогоном.

## RED (до правки скрипта)

Прогон: `node --test tests/unit/test-impact.test.mjs` → exit 1, **tests 10 / pass 6 / fail 4**
(логи рядом: `red.log`, `green.log`, `typecheck.log`, `plan-sync.log`):

```
✖ docs-файл и каталог docs включают meta docs-sync  — «docs: нет docs-sync»
✖ каталог и файл крейта дают targeted cargo discord-core — «discord-core: … workspace»
✖ каталог src/cli и файл дают cli-run + smoke — «src/cli: нет cli-run»
✖ флаги после --changed не считаются изменёнными файлами — actual: [...,'--json']
```

Причина: ветки матчились только по `startsWith("dir/")`, поэтому путь-каталог без слэша
(`docs`, `rust/crates/discord-core`, `src/cli`) проваливался в более общую ветку
(`src/` → `tests/unit/*.test.mjs`, `rust/crates/` → workspace, fallback) либо раздувал прогон;
`--changed a --json` считал флаг файлом.

## GREEN (после минимальной правки)

Правка `scripts/test-impact.mjs` (без ослабления гейта):

1. helper `under(file, dir)` = `file === dir` без слэша **или** `file.startsWith(dir)`;
   применён ко всем каталоговым веткам (`waves/`, `docs/`, `tests/*`, `src/*`,
   `rust/gui-spike/*`, `rust/crates/*`) и к вычислению `sensitive`/tier;
2. `--changed` отбрасывает аргументы, начинающиеся с `--`.

```
node --test tests/unit/test-impact.test.mjs   → exit 0, tests 10 / pass 10 / fail 0
npm run typecheck                             → exit 0 (tsc project + agent)
node --test tests/meta/plan-sync.test.mjs     → exit 0, tests 5 / pass 5 / fail 0
```

Проверка объединённого плана (ручная):
`--changed docs src/cli rust/crates/discord-core rust/gui-spike/ui/app.js rust/gui-spike/ui/features/join-server.js LICENSE`
→ tier T1, 10 команд, все offline, без live; `src/cli` даёт cli-run+smoke, `docs` — docs-sync,
`discord-core` — targeted cargo, join-server — только 2 файла + typecheck.

## Файлы и md5

```
ad2f041367f2c193843b6b723144896b  tests/unit/test-impact.test.mjs   (new)
20808575903485873f81a52f291ef2de  scripts/test-impact.mjs           (fixed; untracked)
750adee39d64f803c6efa8166c0a85dd  TDD-PLAN.md                       (+1 строка §8)
```

Строка §8 (добавлена, мета-тест plan-sync зелёный):

```
| `tests/unit/test-impact.test.mjs` | `scripts/test-impact.mjs` | `planFor`, `docs-sync`, `--full` |
```

## Риски / замечания

- `scripts/test-impact.mjs` и тест — **untracked**; в git их добавляет координатор при коммите.
- `geometry-check.mjs` живёт в `waves/RW-39/PM/w39c/` — путь волновой, при переезде волн
  правило app.js/tokens.css сломается (fallback не предусмотрен, команда упадёт явно).
- Для `src/**` (кроме cdp/ui/cli/types) сохраняется широкий `tests/unit/*.test.mjs` — это
  осознанный fallback «неизвестный src-модуль», не ослабление: файл не входит в batch-правила.
- `--full` не запускает e2e/live — live остаётся за `gate:release` (opt-in), селектор его не
  дублирует; это соответствует запрету на live без окна.
- Пороги/составы suite не менялись (frontend/unit-наборы не тронуты), gate не ослаблен.
