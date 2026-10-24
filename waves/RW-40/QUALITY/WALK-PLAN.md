# WALK-PLAN — QA4-01b: реализация `walk` (fixture), read-only план

- Волна: `RW-40`; карточка **QA4-01b** (`walk` fixture, P1, G4 QA/Tooling); PM — DeepSeek.
- Режим: **только чтение**. Код, тесты, фикстуры, доки и golden-каталоги не менялись;
  `git commit` не делался; live Discord/окно/Frida/сеть не запускались. Запускались только
  локальные read-only проверки: `node bin/discord-lab.mjs walk …` (заглушка, код 2) и
  `node --test tests/e2e/walk-fixture.e2e.test.mjs` (skip, exit 0).
- Основание: `waves/RW-40/QUALITY/TEST-AUDIT.md` §5 (P1: e2e fixture skip = нереализованная
  функция), `TDD-PLAN.md` §M3/§8, `QUEUE.md` строка 5, `STUB-TRACKER.md` STUB-31/33.

## 0. Снимок

| Что | Значение |
|---|---|
| `HEAD` | `0239543` (дерево грязное: идут лейны RW-40; `src/cli/run.mjs`, `src/core/cdp/client.mjs`, `tests/unit/cli-run.test.mjs` — в работе других карточек) |
| `src/cli/run.mjs` | `3b3b5554fb2690486a0bac1358881e85` (397 строк; **после** QA4-01a, аудит §5 устарел в части `cdp`) |
| `src/core/cdp/client.mjs` | `c8173e05512a7672d6fcb5b2ea399520` (318) |
| `src/core/ui/walk-log.mjs` | `7e79f811328b8520291ec2b491fec1fb` (103) |
| `src/core/ui/profile.mjs` | `85a2e4ea7dc81ce9a0b69341daac9b7f` |
| `src/core/ui/blacklist.mjs` | `409475cc151bdfbaf93cd7c5bbc313da` |
| `src/core/paths.mjs` | `3266d8e2e957d7db8672f6ed996b3491` |
| `src/core/types.d.ts` | `87319cb309db656a9032c816982e4a86` |
| `tests/e2e/walk-fixture.e2e.test.mjs` | `faeb5264c6e5600f3e1ca3f696bc3b61` |
| `tests/helpers/browser.mjs` | `01be4c2c09fd2c958e69e6436d0d48f6` |
| `tests/helpers/fake-cdp.mjs` | `89d8921497e91637eb144f330865bfe5` |
| `tests/helpers/cli.mjs` | `0d8a2bef781df0d61c1dbb81d5059b06` |
| `tests/unit/cli-run.test.mjs` | `272254f95517a50e857874d7c1008a46` (562) |
| `tests/fixtures/app/index.html` | `b7abdfbc9d6055a28895d0cb0cf9290a` |
| `scripts/gate.mjs` | `27d7d39e7d54ec53bdfe5ca71e0408c1` |
| `waves/RW-40/QUALITY/TEST-AUDIT.md` | `f2359dbaffbb99e879ba69cf8f5abe7b` |

Факт текущего состояния: `discord-lab walk fixture …` → `Команда walk появится в M3.`, **exit 2**;
`node --test tests/e2e/walk-fixture.e2e.test.mjs` → **pass 0 / skip 1**, exit 0 (причина skip —
`findBrowser()` вернул null: `CHROME_BIN`/`CHROME_PATH` не заданы, chromium/chrome/brave нет в
`PATH`, `/usr/bin`, `/opt`, `/snap`).

## 1. Контракт `walk` (нормативный)

### 1.1 CLI и аргументы

`discord-lab walk <app> [--url <hint>] [--out <path>] [--screens <dir>] [--settle-ms <n>]
[--max-steps <n>] [--fresh] [--no-profile]`

| Аргумент | Обяз. | По умолчанию | Проверка | Код при ошибке |
|---|---|---|---|---|
| `<app>` | да | — | `resolveAdapter(app)` | 1: `walk: укажите приложение (доступно: fixture)` / `walk: неизвестное приложение «x» (доступно: fixture)` |
| `--url <hint>` | нет | `CDP_TARGET` → `discord.com` | непустая строка; **это только подсказка выбора страницы**, не навигация | 1 при `--url=` |
| `--out <path>` | нет | `logs/ui-walk-<app>.jsonl` | `resolve(cwd)`; каталог создаётся | 2 при ошибке записи |
| `--screens <dir>` | нет | `evidence/screens` | `resolve(cwd)`; `mkdir -p` | 2 при ошибке записи |
| `--settle-ms <n>` | нет | `250` | `parseNumericArg`, 0…10000 | 1 |
| `--max-steps <n>` | нет | `200` | `parseNumericArg`, 1…1000 | 1 |
| `--fresh` | нет | false | boolean (игнорирует существующий журнал, усекает out) | — |
| `--no-profile` | нет | false | boolean (без `Profiler.*`, `topFunctions: []`) | — |

Вызов из e2e (не менять): `walk fixture --url file:///…/tests/fixtures/app/index.html
--out <tmp>/ui-walk-fixture.jsonl --screens <tmp>/screens`, env `CDP_ENDPOINT=<browser.endpoint>`.

Открытие страницы: `openPage({endpoint: resolveEndpoint(), hint: parsed.url ?? resolveTargetHint()})`.
`pickTarget` выбирает `type === "page"` c непустым `webSocketDebuggerUrl`: совпадение `url`
по подстроке hint → `discord` → первая страница. `--url` существует именно для fixture-прогона
(в e2e не задан `CDP_TARGET`).

### 1.2 Выбор пунктов и запрещённые

- Адаптер `fixture`: `navSelector = "#nav button"`; подпись = `innerText`, схлопнутые пробелы,
  `trim()` (клиент уже обрезает до 100 символов); пустые подписи отбрасываются; порядок — DOM.
- `visible` не фильтруется (в fixture все видимы; фильтр скрыл бы пункты живого Discord —
  осознанно, паритет `ui-walk-settings2`).
- Запрещённые: `isBlacklisted(label)` **до любого клика**; подстрока без регистра.
  Для запрещённого печатается ровно одна строка в stdout на пункт за прогон:
  `ПРОПУСК (запрещено): <label>` (e2e проверяет `stdout`, не stderr).
- Уже посещённые (из журнала): `ПРОПУСК (уже пройдено): <label>`, в журнал не пишутся.
- Выбор следующего: `pickNextItem(items, visited, isBlacklisted)`; после каждого шага список
  перезапрашивается (динамические подменю). Нет пункта → штатное завершение (0).
- Клик — **точное** совпадение нормализованного текста внутри `navSelector`, последний
  кандидат (паритет `ui-walk-settings2`), `scrollIntoView({block:"center"})` + `el.click()`;
  не найдено → `{clicked:false, reason:"label not found"}`. Это отличается от
  `cdp clicktext` (contains) — adapter-scoped выражение, см. §2.
- `--max-steps` — предохранитель от бесконечных динамических меню; при достижении — стоп,
  предупреждение в stderr, код 0 (журнал полон для пройденного).

### 1.3 CDP-вызовы (точная последовательность)

| # | Когда | Метод | Параметры | Зачем |
|---|---|---|---|---|
| 0 | до WS | `GET <endpoint>/json/list` | — | `listTargets`/`pickTarget` |
| 1 | старт | WS open | `target.webSocketDebuggerUrl` | `createCdpClient` |
| 2 | один раз | `Runtime.enable` | — | события `consoleAPICalled`/`exceptionThrown` |
| 3 | один раз (если profile) | `Profiler.enable` | — | профиль шага |
| 4 | один раз | `Page.enable` | — | паритет старых прогонов |
| 5 | старт и после шага | `Runtime.evaluate` | `listElementsExpression(navSelector)`, `returnByValue:true`, `awaitPromise:true` | список пунктов |
| 6 | шаг | `Profiler.start` | — | начало профиля |
| 7 | шаг | `Runtime.evaluate` | `clickNavItemExpression(navSelector,label)` | клик (без `Input.*`) |
| 8 | шаг | `sleep(settleMs)` | — | дождаться консольных событий/рендера |
| 9 | шаг | `Profiler.stop` → `topFunctions(profile)` | — | `topFunctions` (≤8) |
| 10 | шаг | `Page.captureScreenshot` | `{format:"png"}` | PNG шага |
| 11 | шаг | fs: `writeFile(png)`, `appendFile(jsonl)` | — | артефакты |
| 12 | `finally` | `cdp.close()` | — | закрыть сокет на всех путях |

Запрещено: `Page.navigate`, `Target.*`, `Input.*`, `Network.*`, `Emulation.*`,
`Runtime.terminateExecution`, `device.spawn`, attach к живому Discord. Никаких сетевых
вызовов, кроме CDP-endpoint (в тестах — вообще без сокетов, см. §3).

### 1.4 JSONL-журнал

Одна строка на шаг, `JSON.stringify(record) + "\n"`, **append после каждого шага** (падение
посреди прогона не теряет предыдущие). Поля — ровно `WalkRecord` из `buildWalkRecord`:

| Поле | Тип | Значение |
|---|---|---|
| `step` | number | 1..N; при resume — `maxStep + 1` |
| `label` | string | подпись пункта |
| `clicked` | boolean | результат выражения клика |
| `screenshot` | string \| null | `relativeToRoot(abs)`; вне корня — абсолютный путь (нужно e2e с `/tmp`) |
| `consoleErrors` | number | `consoleIssues(events.slice(mark)).count` |
| `consoleErrorTexts` | string[] | ≤3, `JSON.stringify(params).slice(0,220)` |
| `topFunctions` | TopFunction[] | из `Profiler.stop`; `[]` при `--no-profile` |
| `error` | string \| null | текст ошибки шага |

Resume: если `--out` существует и не `--fresh` → `parseVisited(text)` (битые строки
пропускаются), посещённые подписи не кликаются, `step` продолжается с `maxStep`. `--fresh` —
усечь журнал перед прогоном. Записи с `error != null` тоже попадают в журнал.

### 1.5 Скриншоты

- Каталог `--screens` (`mkdir -p`), имя — `screenshotName(prefix, step, label)` из `paths.mjs`,
  `prefix = "ui-fixture"` (например `ui-fixture-01-Мой-аккаунт.png`).
- PNG пишется из `Buffer` `captureScreenshot`; перед записью проверяется сигнатура
  `89 50 4E 47` и непустой размер (дешёвая защита от «пустого» CDP-ответа).
- Ошибка `Page.captureScreenshot`/записи → `record.error`, `screenshot: null`, прогон
  прекращается (это среда, не данные), код 2.
- Тесты **всегда** передают `--screens`/`--out` в `/tmp`; дефолтные `evidence/screens` и
  `logs/` — golden, агентские прогоны туда не пишут (см. §6).

### 1.6 Ошибки консоли

`mark = cdp.events.length` **до** клика; после `settleMs` —
`consoleIssues(cdp.events.slice(mark))`. `Runtime.enable` строго до первого клика (иначе
`consoleAPICalled` не приходит). `console.error` в fixture синхронный внутри обработчика клика,
порядок «событие/ответ» на одном WS не гарантирован → `settleMs` (250 мс) и есть контракт;
`consoleErrors` — **данные**, а не признак падения (exit 0).

### 1.7 Коды выхода

| Код | Когда | Сообщение (пример) |
|---|---|---|
| 0 | все не-запрещённые пункты пройдены (или уже посещены); шагов с `error` нет | `Готово: 4 шагов -> <out>` |
| 1 | нет/неизвестно `<app>`, пустой `--url`, неверные `--settle-ms`/`--max-steps` | `walk: неизвестное приложение «x» (доступно: fixture)` |
| 2 | CDP недоступен, нет подходящей страницы, 0 пунктов навигации, ошибка шага (`error != null`), потеря соединения (`shouldBail`), ошибка записи PNG/JSONL | `Ошибка CDP endpoint <e>: …` / `Шагов с ошибкой: 1` |

`shouldBail(error)` (`/таймаут|закрыто/`) → прервать цикл, дописать текущую запись, код 2.
Шаг с `clicked:false` → запись + продолжение до конца списка, итог 2 (не «зеленить» провал шага).

## 2. Что уже есть и что добавить

| Компонент | Файл | Что даёт | Статус |
|---|---|---|---|
| CDP-клиент | `src/core/cdp/client.mjs` | `resolveEndpoint`, `resolveTargetHint`, `listTargets`, `pickTarget`, `openPage`, `createCdpClient` (+`events`/`mark`/`eventsSince`), `evaluate`, `listElements`/`listElementsExpression`, `clickText`/`clickTextExpression`, `captureScreenshot` | есть, 36 unit (QA4-01a accepted) |
| Журнал | `src/core/ui/walk-log.mjs` | `parseVisited`, `buildWalkRecord`, `consoleIssues`, `shouldBail`, `pickNextItem` | есть, `tests/unit/walk-log.test.mjs` |
| Профиль | `src/core/ui/profile.mjs` | `topFunctions(profile, 8)` с `NOISE` | есть |
| Чёрный список | `src/core/ui/blacklist.mjs` | `isBlacklisted` | есть |
| Пути/имена | `src/core/paths.mjs` | `screensDir`, `logsDir`, `screenshotName`, `relativeToRoot` | есть |
| CLI `cdp eval/list/clicktext/shot` | `src/cli/run.mjs:120-174` | рабочие подкоманды | есть (аудит §5 в этой части устарел) |
| e2e-контракт walk | `tests/e2e/walk-fixture.e2e.test.mjs` | полный оракул (JSONL/PNG/console/blacklist) | есть, skip |
| Stub-CDP helper | `tests/helpers/fake-cdp.mjs` | подмена fetch/WebSocket, `calls`, `sockets`, `respond` | есть, нужен аддитивный 3-й аргумент `respond(method, params, socket)` для инъекции console-события |
| Фикстура | `tests/fixtures/app/index.html` | 6 `#nav button`, `console.error` на «Уведомления», `window.__clickLog` | есть, менять нельзя |

**Чего нет (объём карточки):**

1. `src/core/ui/engine.mjs` — оркестрация шага (нет файла).
   `runWalk({cdp, adapter, outPath, screensDir, settleMs, profile, maxSteps, fresh, log, warn, sleep})`
   → `{code, records, skipped, steps}`; использует только существующие примитивы.
2. `src/core/ui/adapters.mjs` — реестр `{fixture}`: `navSelector`, `screenshotPrefix`,
   `resolveAdapter(app)`, `clickNavItemExpression(navSelector, label)` (точное совпадение).
3. `src/cli/run.mjs` — ветка `walk` вместо заглушки: разбор флагов, `openPage`, `runWalk`,
   `finally close`, маппинг кодов; формат stdout (`ПРОПУСК (запрещено): …`, `[NN] label | клик=…`).
4. Тесты: `tests/unit/walk.test.mjs` (engine + CLI через stub); правка
   `tests/unit/cli-run.test.mjs` (убрать `walk` из кейса «заглушки», оставить `domains`/`static`).
5. e2e: аддитивные усиления (см. §4, шаг 1) — без ослабления существующих assert.
6. Доки: `README.md` строка `walk` (сейчас «в разработке: M3»), `TDD-PLAN.md` §8 — новая
   строка тест-файла (обязательна: `plan-sync` требует, чтобы **каждый** тест-файл был в §8).

## 3. Почему e2e skip и как проверить без live

### 3.1 Факты

- `findBrowser()` → `null`: `CHROME_BIN`/`CHROME_PATH` не заданы; `chromium`, `google-chrome`,
  `brave` отсутствуют в `PATH`, `/usr/bin`, `/opt`, `/snap`. `launchBrowser` физически не может
  стартовать.
- Поэтому e2e skip — **среда**, но сейчас он маскирует **вторую** причину: заглушку CLI.
  Если браузер появится, тест упадёт на `assert.equal(result.code, 0)` (сейчас код 2).
- После реализации карточки останется только честная причина «нет браузера»; критерии
  превращения в `pass` — §5. Пока `gate:release` даёт `passed_with_skips` — это долг,
  а не приёмка (TEST-POLICY §3.1, аудит §8.2 G-2/G-3 → STUB-33).

### 3.2 Четыре уровня проверки без live

| Уровень | Как | Что доказывает | Что не доказывает |
|---|---|---|---|
| T1 unit (engine) | ручной fake-cdp объект: `send` пишет вызовы, `events` массив, `mark`/`eventsSince`; скриптованные ответы nav/click/profile/screenshot; `sleep: async()=>{}` | порядок CDP-вызовов, blacklist до клика, resume/`--fresh`, `shouldBail`, запись JSONL/PNG в tmp, коды | DOM-семантику, реальный WS |
| T2 unit (CLI) | `installFakeCdp` (глобальные fetch/WebSocket подменены, сокетов нет) + `captureCli(() => run([...]))` | диспетчеризацию, разбор флагов, stdout (`ПРОПУСК …`), файлы, коды 0/1/2 | то же |
| T3 evidence (loopback stub-CDP) | одноразовый скрипт по образцу `waves/RW-40/PM/qa4-01a/EVIDENCE-stub-cdp-smoke.mjs`: HTTP `/json/list` + WS handshake на `127.0.0.1:0`, ответы на `Runtime.evaluate`/`Profiler`/`Page.captureScreenshot`; реальный subprocess CLI | реальный WS-хендшейк, реальные PNG-байты, склейку CLI↔клиент | DOM-семантику, Chrome-поведение |
| T4 e2e | реальный headless Chromium на fixture | **единственный** оракул DOM/геометрии/кликов/консоли | — |

T3 — evidence, не часть гейта (как в QA4-01a; loopback без интернета). Подменять T4 DOM-шимом
(`tests/support/gui-dom-harness.mjs`, `node:vm`) **нельзя**: это второй, более слабый оракул
(шим не исполняет скрипт фикстуры, не считает геометрию, не эмулирует console-порядок) и
прямая дорога к «зелёному» без браузера.

## 4. Red-first план (5 шагов)

Шаг 0 (подготовка, без кода): зафиксировать md5 §0, сохранить `walk-probe.log`
(`walk … → exit 2`) и `e2e-skip.log` (pass 0/skip 1) в `waves/RW-40/PM/qa4-01b/`.

**Шаг 1 — RED (тесты раньше кода).** Новый `tests/unit/walk.test.mjs`:

- args: нет `<app>` → 1; `walk nope` → 1; `--settle-ms abc|−1|10001` → 1; `--max-steps 0` → 1;
  `--url=` → 1;
- engine fixture-сценарий на fake-cdp: 6 пунктов (4 обычных + 2 запрещённых) → ровно 4 клика,
  4 записи в порядке DOM, у запрещённых **нет** `Runtime.evaluate` с их подписью; `ПРОПУСК
  (запрещено)` в stdout; PNG-сигнатура; `consoleErrors: 1` у «Уведомления» и `0` у «Профиля»;
  `step` 1..4; поля ровно `WalkRecord`;
- resume: журнал с 2 подписями → кликаются только оставшиеся, `step` с 3; `--fresh` → всё заново;
- ошибки: пустой nav → 2; `clicked:false` → запись с `error`, продолжение, итог 2;
  `error:"… таймаут …"` → `shouldBail`, цикл прерван, 2; ошибка записи PNG → 2;
  `--no-profile` → `topFunctions: []` и нет `Profiler.*`; `--max-steps 1` → один шаг + warning;
- CLI через `installFakeCdp`: `run(["walk","fixture","--out",tmp,…])` → 0, файл+PNG, stdout;
  `openPage`-ошибка (targets: []) → 2.

Плюс аддитивные assert в `tests/e2e/walk-fixture.e2e.test.mjs` (не заменяя существующие):
`records.map(r=>r.step)` = `[1,2,3,4]`; `window.__clickLog.map(c=>c.text)` = 4 обычные подписи
(через `cdp eval` с `CDP_TARGET=index.html`); файлов PNG в `--screens` ровно 4; текст ошибки
«Уведомлений» содержит `/уведомлен/i`.

Ожидаемый red-лог: `ERR_MODULE_NOT_FOUND … src/core/ui/engine.mjs` (unit), e2e — skip
(без браузера; red не наблюдаем — зафиксировать это в отчёте явно).

**Шаг 2 — GREEN (engine + adapters).** `adapters.mjs`, `engine.mjs` по §2; targeted
`node --test tests/unit/walk.test.mjs` → pass. Покрытие нового кода ≥90% строк/ветвей
(ветки ошибок из шага 1 обязательны).

**Шаг 3 — GREEN (CLI).** Ветка `walk` в `src/cli/run.mjs`; правка `tests/unit/cli-run.test.mjs`
(убрать `walk` из заглушек); targeted `walk.test.mjs` + `cli-run.test.mjs` + `walk-log.test.mjs`
+ `cdp.test.mjs` → pass 0 fail.

**Шаг 4 — проверка без live.** T3 loopback stub-CDP (evidence, скрипт вне `tests/**`), чтобы
получить реальный subprocess-прогон `walk` с WS-хендшейком и PNG; `npm run typecheck`;
meta `docs-sync`, `plan-sync`, `test-policy`, `fixtures-tracked`; `git diff --stat` по своим
файлам. Полный `gate:worker` — один раз в integration-лейне (AGENTS §3).

**Шаг 5 — доки и отчёт.** `README.md` строка `walk` → рабочий пример/коды; `TDD-PLAN.md` §8
строка `tests/unit/walk.test.mjs | src/core/ui/engine.mjs, src/core/ui/adapters.mjs | runWalk,
resolveAdapter, clickNavItemExpression` (**разрешение Team Lead**, общий документ); отчёт по
PROCESS §4.2 с red/green-логами и md5; передать integration-лейну прогон e2e с `CHROME_BIN`.

### Файлы и владельцы

| Файл | Действие | Владелец | Риск |
|---|---|---|---|
| `src/core/ui/engine.mjs` | новый | G4 (автор) | средний: порядок CDP/консоль |
| `src/core/ui/adapters.mjs` | новый | G4 | низкий |
| `src/cli/run.mjs` | правка (ветка `walk`) | G4 | низкий; файл уже менялся QA4-01a |
| `tests/unit/walk.test.mjs` | новый | G4 | средний: объём/покрытие |
| `tests/unit/cli-run.test.mjs` | правка (убрать `walk` из заглушек) | **согласовать** (создан QA4-03, md5 `272254f9…`) | процессный |
| `tests/e2e/walk-fixture.e2e.test.mjs` | аддитивные assert | G4 (после разрешения на правку e2e) | низкий; e2e read-only до карточки |
| `tests/helpers/fake-cdp.mjs` | аддитивно: `respond(m,w,socket)` | G4 | низкий |
| `README.md`, `TDD-PLAN.md` | 2 строки | Team Lead/по разрешению | процессный (общие доки) |

### Риски

| # | Риск | P | Митигация |
|---|---|---|---|
| R1 | e2e не исполняется локально → «зелено на stub, красно на Chrome» | **P1** | точный контракт §1, T3-evidence, обязательный прогон в integration с `CHROME_BIN` до приёмки |
| R2 | flaky console-счётчик (порядок событий) | P1 | `Runtime.enable` до клика, `mark` до клика, `settleMs=250`, e2e-assert не менять |
| R3 | выбор не той страницы (about:blank + fixture) | P2 | hint из `--url`; unit на `pickTarget`; e2e всегда одна fixture-страница |
| R4 | конфликт правок `cli-run.test.mjs` | P2 | согласование с Team Lead/владельцем лейна до кода |
| R5 | `plan-sync` красный из-за нового тест-файла без §8 | P1 (гейт) | строка §8 в том же batch, разрешение Lead; fallback — тесты в `walk-log.test.mjs` (строка §8 уже есть) |
| R6 | дефолтные `--out/--screens` = golden (`logs/`, `evidence/`) | P2 | тесты всегда tmp; в отчёте явно; golden не трогать |
| R7 | покрытие новых файлов <90% | P2 | ветки ошибок из шага 1 обязательны для green |
| R8 | `Profiler.stop` в headless даёт пустой профиль | P3 | `topFunctions` не пустой не требуется e2e; unit скриптует; `--no-profile` как выход |
| R9 | спор о коде 2 при ошибке шага (старый скрипт — 0) | P3 | решение PM (§6, вопрос 1) |

Оценка: 3–4 ч воркера (engine ~170 строк, adapters ~50, CLI ~65, тесты ~350, доки 2 строки).

## 5. Критерии, когда e2e fixture станет pass (не skip), и что нельзя ослаблять

**Pass-критерии (все обязательны):**

1. В среде есть исполняемый браузер: `CHROME_BIN=<path>` (существующий `X_OK`) либо
   chromium/chrome/brave в `PATH`; `findBrowser() !== null`; `launchBrowser` отдаёт
   `DevToolsActivePort` ≤15 с.
2. `node --test tests/e2e/walk-fixture.e2e.test.mjs` → `tests 1 / pass 1 / fail 0 / skip 0`,
   exit 0. В том же окружении `cdp-browser.e2e.test.mjs` → pass 1, skip 0.
3. Существующие assert не изменены и зелёные: код 0; два `ПРОПУСК (запрещено)` в stdout;
   подписи ровно `["Мой аккаунт","Профиль","Уведомления","Голос и видео"]`; у каждой записи
   `clicked:true` и читаемый PNG (`89 50 4E 47`); `consoleErrors` 1 у «Уведомлений» / 0 у
   «Профиля»; `consoleErrorTexts` непуст.
4. Аддитивные проверки §4 зелёные: `step` 1..4, page-side `__clickLog` = 4 обычные подписи,
   4 PNG-файла, текст ошибки про уведомления.
5. Стабильность: 5 последовательных прогонов без flake (M5), gate-повтор (retries: 1) не
   маскирует падение.
6. Долг skip в отчёте виден: `walk-fixture` и `cdp-browser` больше не в `skipped`; `skip`
   остаётся только у live-тестов (нужен живой Discord) — это фиксируется в отчёте явно.

**Нельзя ослаблять:**

- заменять `t.skip` на `return`/тихий pass, ставить `E2E_REQUIRE=0`, добавлять `--allow-skip`
  для fixture-тестов; `skip` допустим **только** при отсутствии браузера, с причиной;
- удалять/ослаблять любой существующий assert (порядок подписей, PNG-сигнатура, счётчики
  консоли, коды), переносить проверки с журнала на stdout;
- править `tests/fixtures/app/index.html` под реализацию (фикстура — независимый оракул;
  изменения только отдельной карточкой с ревью);
- подменять реальный браузер DOM-шимом/vm в e2e; эмулировать клики через `Input.*` вместо
  страничного `el.click()` (сломает `__clickLog`-оракул);
- кликать запрещённые пункты «для проверки»; отключать `isBlacklisted` флагом;
- ослаблять `TEST-POLICY`: без `.only`/`.todo`/`assert.ok(true)`, без `skip` без причины;
- трогать `logs/`, `functions/`, `evidence/` в тестах и агентских прогонах (golden).

## 6. Вопросы PM (блокеры/решения до кода)

1. **Код при ошибке шага.** Предлагаю 2 (сейчас план §1.7); старые скрипты давали 0.
   Если нужен паритет 0 — меняется одна ветка `engine.mjs` + 2 unit-assert.
2. **Новый тест-файл vs §8.** `plan-sync` требует строку в `TDD-PLAN.md` §8 для каждого
   тест-файла. Нужно разрешение Lead на правку общего документа; альтернатива — разместить
   engine+CLI тесты в `tests/unit/walk-log.test.mjs` (строка §8 уже есть).
3. **`tests/unit/cli-run.test.mjs`** — правка «убрать `walk` из заглушек»; файл создан карточкой
   QA4-03 (38 тестов, md5 `272254f9…`) и уже принят/на приёмке. Нужен явный владелец правки на
   время batch (автор QA4-03 или G4 по согласованию), чтобы не сломать его кейс «заглушки».
4. **Дефолтные каталоги** `--out`/`--screens` — `logs/`/`evidence/` (golden). Подтвердить, что
   продуктовый дефолт остаётся, а тесты/агенты всегда передают tmp.
5. **Объём адаптеров:** в этой карточке только `fixture`; `walk settings`/`walk menus`
   (паритет 56/52) — отдельная карточка. `README`/HELP не должны обещать больше.
6. **`README.md`** уже грязный от другого лейна — строку `walk` правит владелец файла или G4
   после согласования.

## 7. Что этот план НЕ делает

- Не реализует и не меняет код/тесты/фикстуры; не запускает браузер, Discord, Frida, сеть.
- Не отменяет skip и не «зеленит» e2e: честная приёмка QA4-01b возможна только с прогоном
  T4 в среде с `CHROME_BIN` (integration/CI); T1–T3 — доказательство исполнимости, не замена.
