# QA-GATE-SKIPS — skip ≠ pass: e2e-набор и live-фаза в гейте

Дата: 2026-10-24. Роль: **G4 QA writer**. Карточка: **QA-GATE-SKIPS (P1A, QA-M2-01)**.
PM: DeepSeek. `HEAD 0239543`, дерево dirty (правки других воркеров не трогались).
Режим: только Node-мета-тесты; браузер/Chrome/CDP, Discord/live, полный `gate:release`,
`gate:qualify` и product/UI/Rust-код не запускались и не менялись.

Основание: `waves/RW-40/QUALITY/QA-MATRIX-02.md` §3 QA-M2-01 — «`skipped 4` при `pass 0`
даёт `passed_with_skips` и exit 0; M2 можно принять без единого браузерного assert».

## 1. Что сделано

| # | Требование | Реализация |
|---|---|---|
| 1 | fixture e2e: `skipped > 0` → fail, не `passed_with_skips` | `e2eFixturePhase()` (`scripts/gate.mjs:354`): `failOnSkips: true`, `minPass: 1`, `--test-concurrency=1`, набор = все `tests/e2e/*.e2e.test.mjs`, **кроме** `live-*` (`fixtureE2eFiles()` `:340`). Skip или `pass 0` → фаза failed (`e2e_unmet`) |
| 1a | live-фаза отдельно, с явным `unmet` | `e2eLivePhase` (`:380`): только `tests/e2e/live-discord.e2e.test.mjs`, `requiresEnv: "DISCORD_LAB_LIVE_CDP"`, `failOnSkips: true`. Без env фаза не запускается: `unmet: true`, вердикт `passed_with_unmet`, `acceptance.accepted=false`. Фаза включена только в `qualify` (T3); `release` (T2) — fixture-only |
| 2 | сохранить `attempts[]` / оба результата повтора | `executePhase` (`:520`) пишет каждый прогон: `{attempt, ok, status, ms, counts, reasons, tail}`; в JSON гейта — `phases[].attempts` (оба лога, не только `note`) |
| 2a | не менять смысл Rust `ignored` (keyring) | `cargoTestCounts` не тронут: `ignored → skipped`. Для `workspace-rust`/`spike-rust` добавлен `skipKind: "ignored"`: такие skip видны в `warnings` и отдельном поле `ignored[]`, но **не** превращают вердикт в e2e-skip. `keyring-live` по-прежнему `failOnSkips` |
| 3 | meta-тест «skip ≠ pass», `failOnSkips` на примере | `tests/meta/test-policy.test.mjs:137` — 11 кейсов: `phaseOutcome` на синтетических счётчиках, `computeVerdict`, `e2eStatus`, `computeAcceptance`, структура фаз и `attempts[]` |
| 4 | приёмка по счётчикам, а не по вердикту | В JSON: `verdict`, `e2e: {fixture, live}` (`met/unmet/absent`), `acceptance: {accepted, reason}`, `ignored[]`, `totalSkipped`. Exit 0 только если прогон принят; `--verify-qualification` больше не принимает `passed_with_*`/`unmet` (`:469`) |

Вердикты: `passed | passed_with_skips | passed_with_unmet | failed`.
`passed_with_skips` остаётся только для непройденных **не-e2e** фаз (skip-aware unit/smoke);
e2e/live-skip даёт `failed`, отсутствие live-среды — `passed_with_unmet`.

## 2. Red → Green

| Шаг | Команда | Exit | Результат | Лог |
|---|---|---|---|---|
| RED | `node --test tests/meta/test-policy.test.mjs` | **1** | tests 17, pass 5, **fail 12** — функций `phaseOutcome/computeVerdict/e2eStatus/computeAcceptance` нет, `e2e` без `failOnSkips`, live-фазы нет, `attempts[]` нет | `waves/RW-40/QUALITY/QA-GATE-SKIPS-red.log` |
| GREEN | `node --test tests/meta/test-policy.test.mjs` | **0** | tests 17, pass 17, fail 0, skipped 0, 47.9 мс | `waves/RW-40/QUALITY/QA-GATE-SKIPS-green.log` |
| META | `node --test --test-concurrency=1 tests/meta/*.test.mjs` | **1** | tests 77, pass 76, fail 1 — только **pre-existing** `plan-sync` (§8 не описывает чужие новые файлы `tests/unit/dispatch_ui.test.mjs`, `tests/unit/search-remote-real-index.test.mjs`); мои файлы и PLAT-02 (`PLAT-02 T1/T2/T3`) зелёные | `waves/RW-40/QUALITY/QA-GATE-SKIPS-meta.log` |
| E2E-вход | `env -u CHROME_BIN -u CHROME_PATH -u DISCORD_LAB_LIVE_CDP node --test --test-concurrency=1 tests/e2e/cdp-browser.e2e.test.mjs tests/e2e/walk-fixture.e2e.test.mjs` | 0 | tests 2, **pass 0, skip 2** — ровно тот вход, который раньше давал «зелёный» e2e | `waves/RW-40/QUALITY/QA-GATE-SKIPS-e2e-counts.log` |

Про `E2E-вход`: node возвращает 0 при полном skip — это и есть дыра. Теперь
`phaseOutcome({failOnSkips:true, minPass:1}, {tests:2, pass:0, fail:0, skipped:2}, true)`
возвращает `ok:false, skipFailure:true` (закреплено в green-тесте на примере `skip 4` из
`.gate/release-…T09-37-37-616Z.json`).

`npx tsc -p tsconfig.json`: в моих файлах ошибок 0. Полный tsc красный **не из-за карточки**:
16 ошибок в untracked `tests/unit/dispatch_ui.test.mjs` (L-02-UI, чужая зона).

## 3. Диф

Файлы: `scripts/gate.mjs` (+374/−26 к `HEAD`, при этом в диффе уже были незакоммиченные
правки PLAT-02 — live-keyring; они не откатывались), `tests/meta/test-policy.test.mjs`
(untracked, добавлен блок QA-M2-01: хелперы 25–54, тесты 137–260).

| Место | Было | Стало |
|---|---|---|
| `e2e`-фаза | `{name:"e2e", args:["--test","tests/e2e/**/*.test.mjs"], retries:1}` — live внутри, skip не валит | `e2eFixturePhase()`: без `live-*`, `--test-concurrency=1`, `failOnSkips:true`, `minPass:1`, preflight браузера («нет браузера → skip → failed») |
| live | жил внутри `e2e`, при отсутствии env → `skip` | `e2eLivePhase`: `requiresEnv`, без env → `unmet`, только в `qualify` |
| исходы фазы | `ok` + `counts` | `ok`, `unmet`, `skipKind`, `attempts[]` |
| вердикт | `!allOk ? failed : totalSkipped>0 ? passed_with_skips : passed` | `computeVerdict`: failed → passed_with_unmet → passed_with_skips (только не-ignored skip) → passed |
| запись | вердикт + warnings | + `e2e.fixture/live`, `acceptance`, `ignored[]`, `totalSkipped`, `phases[].attempts` |
| выход | `allOk ? 0 : 1` | `прогон принят ? 0 : 1` (unmet в qualify — 1) |
| `--verify-qualification` | `verdict.startsWith("passed")` | `acceptance.accepted === true` либо строго `verdict === "passed"` |

Ключевые точки: `phaseOutcome:156`, `computeVerdict:173`, `e2eStatus:189`,
`computeAcceptance:208`, `browserPreflight:312`, `fixtureE2eFiles:340`,
`e2eFixturePhase:354`, `e2eLivePhase:380`, `verifyQualification:469`,
`executePhase:520`, `main:623`.

## 4. Evidence (команда → exit → артефакт)

```text
$ node --check scripts/gate.mjs                                            → 0
$ node --test tests/meta/test-policy.test.mjs (RED)                        → 1  (5 pass / 12 fail)
  waves/RW-40/QUALITY/QA-GATE-SKIPS-red.log
$ node --test tests/meta/test-policy.test.mjs (GREEN)                      → 0  (17/17)
  waves/RW-40/QUALITY/QA-GATE-SKIPS-green.log
$ node --test --test-concurrency=1 tests/meta/*.test.mjs                   → 1  (76/77; plan-sync — чужое)
  waves/RW-40/QUALITY/QA-GATE-SKIPS-meta.log
$ node scripts/gate.mjs --help                                             → 0
$ env -u CHROME_BIN ... node --test tests/e2e/{cdp-browser,walk-fixture}…  → 0  (pass 0 / skip 2)
  waves/RW-40/QUALITY/QA-GATE-SKIPS-e2e-counts.log
$ npx tsc -p tsconfig.json | grep -E '^(scripts/gate|tests/meta/test-policy)' → 0 строк
$ md5sum scripts/gate.mjs tests/meta/test-policy.test.mjs
  89c67ad0a8f087b87626090b2b5899d9  scripts/gate.mjs
  62d7c60b80bcf11cc18294c9a0350dc4  tests/meta/test-policy.test.mjs
```

До правки (снимок QA-MATRIX-02 §7): `gate.mjs c8a4875ed7d7a64d9dad3a215cad9a8f`.

## 5. Что это меняет для приёмки

| Прогон | Было | Стало |
|---|---|---|
| `release`, нет браузера | e2e `pass 0/skip 4` → `passed_with_skips`, exit **0** | e2e failed (`failOnSkips: skip N`), `verdict=failed`, `acceptance.accepted=false`, exit **1** |
| `release`, браузер есть, fixture зелёный | `passed_with_skips` (из-за Rust ignored) | `verdict=passed`, `e2e.fixture=met`, `acceptance.accepted=true`, exit 0; ignored-набор — в `ignored[]` |
| `qualify` без `DISCORD_LAB_LIVE_CDP` | live-тесты уходили в `skip`, вердикт «зелёный» | `e2e.live=unmet`, `passed_with_unmet`, `acceptance=false`, exit **1** |
| `worker` | `passed_with_skips` из-за 12 Rust ignored | `passed`; `ignored[]` объясняет 12, e2e в worker не входит |

Проверка приёмки в CI/PM — по полям, а не по вердикту:

```sh
jq -e '.e2e.fixture=="met" and .acceptance.accepted==true' .gate/release-*.json
jq -e '.e2e.fixture=="met" and .e2e.live=="met"' .gate/qualification-*.json   # T3
```

## 6. Остаток / риски

- **Гейт теперь честно красный без браузера.** Это цель карточки, но это значит: пока
  `CHROME_BIN` не задан и нет 7 браузерных сценариев (QA-M2-02), `gate:release` не даёт
  `passed`. Красный нельзя «лечить» снятием `failOnSkips` — meta-тест упадёт.
- **`minPass: 1`, а не 9.** Порог фиксирует «хотя бы один выполненный тест»; целевой
  `pass ≥ 9` (2 harness + 7 M2) — предмет QA-M2-02 и meta-инварианта «7 сценариев есть и
  не skip». Повышать порог до появления `m2-*.e2e.test.mjs` значило бы заранее красить
  релиз по чужой карточке.
- **`e2e-live` только в `qualify`.** Если PM захочет live и в `release`, это одна строка,
  но тогда T2 перестанет быть fixture-only (против QA-MATRIX §4.1).
- **`attempts[].tail` — последние 20 строк попытки**, полный stdout не сохраняется (размер
  JSON). Первый прогон больше не теряется, но и не является полным логом: для разбора
  запускать команду фазы вручную.
- **`--verify-qualification` стал строже**: старые записи с `passed_with_skips` больше не
  проходят проверку. Это осознанно (skip ≠ pass); действующие qualification-файлы нужно
  перевыпустить после зелёного `gate:qualify` с live.
- **Не запускалось** (по ТЗ): Chrome/браузер, live Discord, `cargo`-гейт, `gate:release`,
  `gate:qualify`, мутации гейта, coverage. Вывод о release/qualify-поведении — из кода и
  green-тестов на чистых функциях, не из полного прогона.
- **Чужое красное, не моя зона:** `plan-sync` (2 untracked unit-файла без строк в §8
  `TDD-PLAN.md`), 16 ошибок tsc в `tests/unit/dispatch_ui.test.mjs`, diff `AGENTS.md`/
  `PROCESS.md`/etc. — правки других воркеров; не откатывались и не правились.
- **Оговорки по прогонам (полная прозрачность):**
  - в green-итерации один regex моего нового теста был уточнён с `attempts.push({ attempt`
    на `attempts.push({\s*attempt,` — это формат многострочного литерала, ожидание не
    менялось (red-лог снят до реализации, требование то же);
  - `node --test tests/meta/*` включает `keyring-live` T3, который сам запускает
    `cargo test --offline -p discord-platform --test keyring_contract` (2 прогона, live-env
    не выставлялся): `PLAT-02 T3` зелёный, `no cargo` — до запуска (проверено `pgrep`),
    параллельного cargo не было;
  - `gate:release`/`gate:qualify` не запускались, поэтому нового `.gate/*.json` эта карточка
    не создаёт; утверждения о поведении release/qualify — из кода и green-тестов.
- Журнал `waves/db.jsonl` и `waves/STATE.md` не трогались — по процессу их обновляет
  Team Lead после приёмки.
