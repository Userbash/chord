# REPORT — QA4-01b: реализация `walk` (fixture)

- Волна: `RW-40`; карточка: **QA4-01b** (P1, G4 QA/Tooling); PM — DeepSeek.
- Вход: `waves/RW-40/QUALITY/WALK-PLAN.md` (D-104), `TDD-PLAN.md` §M3/§8,
  `waves/RW-40/QUALITY/TEST-AUDIT.md` §5, `waves/RW-40/STUB-TRACKER.md` STUB-31/33.
- Ограничения соблюдены: без live Discord, без окна, без Frida/spawn, без сети
  (кроме loopback stub-CDP на `127.0.0.1`), без коммитов; `tests/e2e/**` — только
  аддитивные assert; `rust/**`, `logs/`, `functions/`, `evidence/` не менялись.

## Сделано

1. `src/core/ui/adapters.mjs` (новый, 55 строк): реестр `ADAPTERS` (только `fixture`),
   `resolveAdapter(app)` (ошибка со списком доступных) и `clickNavItemExpression(selector, label)` —
   точное совпадение нормализованного текста, последний кандидат, `scrollIntoView` + `el.click()`;
   не найдено → `{clicked:false, reason:"label not found"}`.
2. `src/core/ui/engine.mjs` (новый, 289 строк): `runWalk({cdp, adapter, outPath, screensDir,
   settleMs, profile, maxSteps, fresh, log, warn, sleep})` → `{code, records, skipped, steps}`.
   - `Runtime.enable` → (`Profiler.enable`) → `Page.enable` до первого клика;
   - шаг: список `navSelector` → blacklist/visited до клика → `Profiler.start` → `mark()` →
     `Runtime.evaluate` (клик) → `sleep(settleMs)` → `Profiler.stop` → `Page.captureScreenshot` →
     PNG + append JSONL → строка stdout;
   - `mark`/`consoleIssues` для ошибок консоли, `--no-profile` без `Profiler.*`, `topFunctions: []`;
   - resume по журналу (`parseVisited`), `--fresh` усекает журнал, шаг = `maxStep + 1`;
   - `ПРОПУСК (запрещено)` / `ПРОПУСК (уже пройдено)` — ровно одна строка на подпись за прогон;
   - `shouldBail` (таймаут/закрыто) прерывает цикл с записью шага; `clicked:false` → запись,
     продолжение, итог 2; `--max-steps` → стоп с предупреждением, код 0;
   - PNG проверяется на сигнатуру `89 50 4E 47`; ошибка скриншота/записи → код 2.
3. `src/cli/run.mjs`: ветка `walk` вместо заглушки — `<app>`, `--url`, `--out`, `--screens`,
   `--settle-ms` (0…10000, 250), `--max-steps` (1…1000, 200), `--fresh`, `--no-profile`;
   дефолты — `logs/ui-walk-<app>.jsonl` и `evidence/screens`; `openPage` + `finally cdp.close()`;
   коды 0/1/2, `Готово: N шагов -> <out>`.
4. `tests/unit/walk.test.mjs` (новый, 33 теста): адаптер, аргументы CLI (1), engine на fake-CDP
   (запрещённые не кликаются, порядок CDP, JSONL/PNG, consoleErrors, resume/fresh, bail,
   clicked:false, ошибки Profiler/записи, `--no-profile`, `--max-steps`), CLI через
   `installFakeCdp` (0 и 2, fresh/no-profile, нет страницы).
5. `tests/helpers/fake-cdp.mjs`: аддитивно `respond(method, params, socket)` — инъекция
   console-события без реального сокета.
6. `tests/unit/cli-run.test.mjs`: `walk` убран из кейса «заглушки» (остались `domains`/`static`);
   поведение `walk` покрыто отдельным файлом.
7. `tests/e2e/walk-fixture.e2e.test.mjs`: только аддитивные assert (step 1..4, 4 PNG,
   `window.__clickLog` = 4 обычные подписи, текст ошибки `/уведомлен/i`); существующие не менялись.
8. Доки: `README.md` — строка `walk` (аргументы, дефолты, коды) + пример; `TDD-PLAN.md` §8 —
   строка `tests/unit/walk.test.mjs`; из строки `cli-run` убран токен `walk` (теперь отдельный файл).

Запрещённые вызовы не используются: `Page.navigate`, `Target.*`, `Input.*`, `Network.*`,
`Emulation.*`, `Runtime.terminateExecution`, `device.spawn`.

## Red (strict TDD)

`node --test tests/unit/walk.test.mjs` → **exit 1** (`red.log`):

```
Error [ERR_MODULE_NOT_FOUND]: Cannot find module '.../src/core/ui/adapters.mjs'
ℹ tests 1 / pass 0 / fail 1
exit=1
```

`tests/e2e/walk-fixture.e2e.test.mjs` в этой среде — **skip** (`e2e-skip.log`: pass 0 / skip 1,
exit 0, причина «нет браузера: задайте CHROME_BIN»); e2e-red не наблюдаем — зафиксировано явно.
До реализации `discord-lab walk fixture` → `Команда walk появится в M3.`, exit 2
(`walk-probe.log`).

## Green

- `node --test tests/unit/walk.test.mjs` → **pass 33 / fail 0**, exit 0 (`green-unit.log`).
- `node --test tests/unit/cli-run.test.mjs tests/unit/walk-log.test.mjs tests/unit/cdp.test.mjs`
  → **pass 93 / fail 0**, exit 0 (`targeted.log`).
- `node --test 'tests/unit/**/*.test.mjs'` → **pass 588 / fail 0 / skip 0**, exit 0 (`unit-all.log`);
  было 539 unit до карточки (не ослаблено, +33 новых).
- `node --test 'tests/meta/**/*.test.mjs'` → 52/53: красный только `plan-sync` из-за чужого
  `tests/unit/join-server.test.mjs` (лейн G-03-UI, файл вне моей карточки) — см. «Gate».
- Покрытие нового кода (`coverage.log`): `engine.mjs` 100.00% строк / 93.83% ветвей,
  `adapters.mjs` 100/100 (порог ≥90/90 выполнен).
- T3 evidence (loopback stub-CDP, вне гейта): `node waves/RW-40/PM/qa4-01b/EVIDENCE-stub-cdp-walk.mjs`
  → `OK stub-CDP walk: 4 шага, 4 PNG, consoleErrors 1, запрещённые вызовы отсутствуют`, exit 0
  (`stub-smoke.log`): реальный WS-хендшейк, реальные PNG-байты, subprocess CLI.

## Oracle

- `waves/RW-40/QUALITY/WALK-PLAN.md` §1 (нормативный контракт: CDP-последовательность,
  поля `WalkRecord`, коды, resume/blacklist).
- `tests/e2e/walk-fixture.e2e.test.mjs` + `tests/fixtures/app/index.html` (независимый DOM-оракул,
  не менялся).
- `src/core/ui/walk-log.mjs`, `blacklist.mjs`, `profile.mjs`, `paths.mjs` (принятые контракты,
  тесты не менялись).
- T3: реальные байты PNG и WS-кадры от loopback stub (не DOM-шим, не vm).

## Gate

`npm run gate:worker` → **failed** (`gate-worker.log`), но не из-за карточки:

- `unit` — **pass 588 / fail 0 / skip 0**; `smoke` — pass 22; `spike-rust` — pass;
  `workspace-rust` — 1103 pass / 12 skip (live keyring);
- `typecheck` — **красный из-за чужого untracked `rust/gui-spike/ui/features/join-server.js`**
  (`TS2322 Timeout`); в выводе tsc нет ни одного файла этой карточки;
- `meta` — `plan-sync` красный из-за `tests/unit/join-server.test.mjs` без строки в §8
  (лейн G-03-UI); `docs-sync`/`test-policy` зелёные.

То есть блокер внешний: параллельный лейн не завершил §8-строку/тип. После его завершения
gate:worker нужно перезапустить (Integration/QA).

## Файлы + md5 (после)

| Файл | md5 |
| --- | --- |
| `src/core/ui/engine.mjs` (new) | `dcd7cec03794962592e67e100a7fe390` |
| `src/core/ui/adapters.mjs` (new) | `f5f52aef35eb98ad9e14ff17e3c6095b` |
| `src/cli/run.mjs` | `f622b4608bca2d31edabeb7c928383b4` |
| `tests/unit/walk.test.mjs` (new) | `03e15f6bd9c595d1a90aee4736742932` |
| `tests/unit/cli-run.test.mjs` | `43290e0706a3ba9e46223daff9b7701d` |
| `tests/helpers/fake-cdp.mjs` | `80e3f18bdd73fde4b01ff974d86c7254` |
| `tests/e2e/walk-fixture.e2e.test.mjs` | `75e4524eaad0494fd4492c0e6f6ab1d5` |
| `TDD-PLAN.md` | `66daec5bd9c66b732e42a828a3e2e160` |
| `README.md` | `e098377735a43a6f495b8640ab997312` |

До карточки: `run.mjs` `3b3b5554…`, `cli-run.test.mjs` `272254f9…`, `fake-cdp.mjs` `89d89214…`,
`walk-fixture.e2e` `faeb5264…`, `TDD-PLAN.md` `bf6b4e2a…`, `README.md` `b2bb127b…`
(`md5-before.txt` / `md5-after.txt`).

Логи: `walk-probe.log`, `e2e-skip.log`, `red.log`, `green-unit.log`, `targeted.log`,
`unit-all.log`, `coverage.log`, `meta.log`, `typecheck.log`, `stub-smoke.log`, `gate-worker.log`,
`md5-before.txt`, `md5-after.txt`, `EVIDENCE-stub-cdp-walk.mjs`.

## Риски / остаток

| # | Риск/остаток | Статус |
| --- | --- | --- |
| 1 | e2e fixture не прогнан (нет браузера): DOM/консоль/`__clickLog` подтверждены только unit-оракулом + T3 | **долг**: нужен прогон `CHROME_BIN=… node --test tests/e2e/walk-fixture.e2e.test.mjs` в integration/CI до приёмки |
| 2 | `gate:worker` красный из-за чужого `join-server` (typecheck + plan-sync) | внешний блокер, перезапустить после G-03-UI |
| 3 | `maxSteps` при исчерпании даёт код 0 (по WALK-PLAN §1.2/§1.7) | решение PM, реализации соответствует |
| 4 | Адаптеры только `fixture`; `walk settings/menus` не реализованы | отдельная карточка (README/HELP не обещают больше) |
| 5 | Дефолтные `--out/--screens` — `logs/`/`evidence/` (golden) | тесты/агент всегда передают tmp; golden не тронут |

## Следующий шаг

Integration/QA: перезапустить `npm run gate:worker` после завершения лейна G-03-UI; затем прогон
e2e fixture в среде с `CHROME_BIN` (критерии WALK-PLAN §5). PM — приёмка карточки.
