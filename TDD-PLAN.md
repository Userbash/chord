# План: `discord-lab` — полноценное CLI-приложение на строгом TDD

Дата плана: 2026-10-18. Обновлено: 2026-10-19 (M1–M2 приняты, red-базлайн закрыт, CI,
сверка тестов и статусов; ранее — 2026-10-18: декомпозиция, мета-контроль, углубление).
Процесс (роли, волны, гейты, шаблоны ТЗ/отчётов) — `PROCESS.md`; правила агентам — `AGENTS.md`.

Статус (2026-10-19): **M1 (ядро) и M2 (Frida-слой) приняты; красного нет** — полный прогон
`npm test`: 200 тестов — 196 pass / 0 fail / 4 skip (e2e без браузера, skip-aware). Впереди
M3–M6 (walk-движок, отчёты, жёсткость, паритет). Микро-декомпозиция работ и маршрут для
воркеров — `TASKS.md`. Реализация идёт строго по тестам (red → green → refactor).

## 1. Продукт и принципы

Продукт — CLI-приложение **`discord-lab`** (пакет `discord-function-lab`):
лаборатория разбора Discord-клиента превращается из набора разрозненных скриптов
в один инструмент с библиотечным ядром.

Принципы:

1. **Без сборки.** Всё запускается обычным `node` напрямую: `.mjs` + JSDoc-типы,
   `tsc --noEmit` только проверяет типы (уже работает в проекте).
2. **Строгий TDD.** Ни строки прод-логики без красного теста на это поведение.
   Порядок: тест → красный → минимальная реализация → зелёный → рефактор.
3. **Три уровня тестов** (unit / smoke / e2e) на встроенном раннере `node:test`
   (0 зависимостей). Уровни описаны в §4.
4. **Тесты — контракт.** Публичные сигнатуры и форматы данных зафиксированы тестами;
   тесты нельзя править, чтобы «позеленело» — правится код (или отдельным коммитом
   исправляется сам тест, если он описывал неверный контракт).
5. **Паритет до M6.** Существующие `scripts/*` продолжают работать; `src/` заменяет
   их постепенно, по милестоунам, без одновременного переключения поведения.

## 2. Целевая структура

```
bin/discord-lab.mjs        точка входа (#!/usr/bin/env node), тонкая: run(argv) -> код выхода

src/core/                  доменное ядро: чистые функции, ноль I/O
  types.d.ts               общие типы (CDP, записи, инвентарь) — только типы
  args.mjs                 parseArgs: --key value / --key=value / --flag / --module (повторяемый)
  csv.mjs                  safeName, csv (экранирование как в scripts/logger.mjs)
  paths.mjs                корень проекта, logs/ functions/ evidence/screens/, имена скриншотов (slug)
  processes.mjs            isDiscordProcess / filterDiscordProcesses
  inventory.mjs            строки CSV и JSON инвентаря (--max-symbols-логика формата)
  calls.mjs                записи вызовов: isCallPayload, callLine, parseCallLog, summarizeCallLines
  hook-io.mjs              openCallLog, waitForStop — потоковая запись и ожидание hook-лога
  report/nm.mjs            parseNmPosix, staticFunctionsCsv (паритет extract-static-functions.sh)
  report/domains.mjs       classifyName по таблице доменов, domainCsv, formatDomainSummary
  cdp/client.mjs           CDP: resolveEndpoint/resolveTargetHint/listTargets/pickTarget/
                           createCdpClient (WebSocket инжектится) / evaluate
  ui/profile.mjs           topFunctions: топ функций из CPU-профиля (+NOISE)
  ui/blacklist.mjs         isBlacklisted: запрещённые пункты UI-прогонов (объединение всех версий)
  ui/walk-log.mjs          parseVisited, buildWalkRecord, consoleIssues, shouldBail, pickNextItem
  ui/engine.mjs (M3)       шаг walk: клик → профиль → скриншот → запись (адаптеры discord/fixture)

src/cli/                   команды: оркестрация + вывод (M1–M2: pids/inventory/hook/summarize/cdp; M3–M4: walk/domains/static)
  run.mjs                  маршрутизация команд, help/version, коды выхода

src/adapters/              границы с внешним миром
  cxxfilt.mjs              demangle(names) через c++filt (spawn)
  frida.mjs                Frida-слой: loadDevice/loadAgentSource/listProcesses/attach/
                           collectInventory/parseHookMessage/startHook/stopHook (устройство инжектится)

tests/
  unit/                    чистые контракты (быстро, без процессов и сети)
  meta/                    мета-тесты: «план §8 ↔ тесты ↔ экспорты», «фикстуры ↔ git», «README ↔ команды CLI»
  smoke/                   реальный CLI-процесс; Frida только информационно (skip-aware)
  e2e/                     реальный CDP-браузер; live-Discord — opt-in (skip-aware)
  helpers/                 FakeWebSocket, запуск CLI, tmp-каталоги, поиск/запуск браузера
  fixtures/                sample/golden-фикстуры (провенанс: артефакт + md5 + оракул)

TDD-PLAN.md                этот план
TASKS.md                   микро-декомпозиция задач, волны, протокол воркеров
```

Слои и правила зависимостей:

- `src/core/**` — **чистые функции и структуры**: не читают файлы, не ходят в сеть,
  не спавнят процессы, не трогают `process.env` напрямую (env передаётся аргументом,
  например `resolveEndpoint(env)`). Это делает unit-тесты мгновенными.
- `src/adapters/**` и `src/cli/**` — **грязный слой**: процессы, сеть, диск, Frida.
  Всё внешнее инжектится: `WebSocket` (CDP), `fetch`, Frida-устройство, часы.
- Типы: новые модули используют `src/core/types.d.ts`; старые скрипты остаются
  на `scripts/cdp-types.d.ts` до милестоуна M6 (унификация типов — шаг M6).

## 3. Как это запускается

```bash
npm test              # все уровни: unit + meta + smoke + e2e (e2e сами пропускаются без среды)
npm run test:unit     # только unit
npm run test:meta     # мета-тесты: план §8 ↔ файлы тестов ↔ экспорты модулей
npm run test:smoke    # CLI + Frida (skip, если Frida недоступна)
npm run test:e2e      # CDP против headless-браузера (нужен CHROME_BIN или chromium в PATH)
npm run test:watch    # unit в режиме наблюдения
npm run test:coverage # покрытие src/** с порогами: строки ≥ 90, функции ≥ 80, ветки ≥ 90
                      # (M1: 99.30/90.45/97.56 — зелёный; расширение на cli/adapters — M5)
npm run typecheck     # tsc: scripts/ + src/ + bin/ + tests/
npm run check         # typecheck + все тесты — «одна кнопка» перед приёмкой шага
npm run gate:worker   # гейт воркера: типы → мета → unit → smoke (PROCESS.md §3)
npm run gate:release  # гейт выпуска: worker + e2e (повтор при падении) + покрытие + golden-сверка
npm run gate:qualify  # квалификация: release + tarball + изолированный прогон пакета
```

Переменные окружения тестов:

- `CHROME_BIN` — путь к chromium/chrome/brave/edge для e2e (иначе e2e пропускается);
- `DISCORD_LAB_LIVE_CDP` — endpoint запущенного Discord с remote-debugging
  (например `http://127.0.0.1:9223`); включает live-проверки e2e (только чтение);
- `CDP_ENDPOINT` / `CDP_TARGET` — обычные переменные CDP-клиента.

## 4. Тестовая стратегия

| Уровень | Что проверяет | Среда | Скорость | Где живёт |
| --- | --- | --- | --- | --- |
| unit | чистые функции `src/core`: разбор, форматирование, фильтры, маршрутизация CDP-сообщений; golden-данные из реальных артефактов | без процессов и сети (чтение фикстур/бандлов разрешено) | ~секунды | `tests/unit` |
| meta | соответствие «план §8 ↔ тесты ↔ экспорты»: ничего не пропущено и не придумано | чтение файлов репозитория, без процессов | мгновенно | `tests/meta` |
| smoke | реальный процесс `bin/discord-lab.mjs`: help/version/ошибки, `summarize` на фикстурах и реальных логах, `pids` через живой Frida | процессы, локальный ФС; Frida — skip-aware | секунды | `tests/smoke` |
| e2e | CLI против реального CDP: headless-браузер на fixture-странице; live-Discord (opt-in) — только чтение | браузер/живой Discord | десятки секунд | `tests/e2e` |

Правила «жёсткого» TDD, действующие в проекте:

1. Один цикл = один новый/изменённый тест. Красный → минимальный зелёный → рефактор.
2. Никакой «логики на будущее»: реализуется только то, что требует тест.
3. Unit-тесты не запускают процессы и не ходят в сеть; входные данные — чтение фикстур
   или сборка в памяти; запись — только в `mkdtemp` (`tests/helpers/tmp.mjs`).
   Время, WebSocket, fetch, Frida — инжектируемые зависимости (см. `FakeWebSocket`).
4. Тесты детерминированы: фиксированные данные, явные сортировки; `skip` разрешён
   только из-за отсутствия среды (нет браузера / Frida / live-endpoint), не из-за багов.
5. Ничего не пишется в рабочие каталоги проекта из тестов — только в tmp.
6. Каждый зелёный шаг сохраняется (см. §5 про git); сообщения коммитов: `test: …`,
   `feat: …`, `fix: …`.
7. Покрытие — следствие TDD, а не самоцель; тем не менее гейт: `src/core` ≥ 90% строк
   (M1), мутационная проверка — M5.
8. Мета-контроль: §8 плана и тесты не должны разъезжаться —
   `npm run test:meta` падает на любом расхождении (см. «Методы углубления тестов»).

### Методы углубления тестов (волна W1 и далее)

1. **Golden из реальных артефактов.** Ожидания строятся независимыми оракулами
   (`jq`, `sort | uniq`, `awk`, `c++filt`) по реальным логам и таблицам (`logs/**`,
   `functions/**`), а не из кода под тестом. В тесте фиксируются: имя артефакта,
   его md5 и пайплайн-оракул (в комментарии). Компактные копии — в
   `tests/fixtures/**` (≤ 64 КиБ); полные артефакты — только в smoke-проверках
   (skip, если файла нет).
2. **Инварианты, границы, стресс.** Пустой ввод; битые и неполные строки; дубликаты;
   кириллица, кавычки, запятые, переносы; очень длинные имена; стресс-объёмы
   (10⁵+ записей) — с проверкой корректности, а не только «не упало».
3. **Поведенческие матрицы.** Один кейс = одна строка таблицы кейсов (например,
   чёрный список UI-пунктов: блокировка/пропуск/обычные пункты), чтобы при
   провале было видно, какое именно правило сломалось.
4. **Мета-тесты.** `tests/meta/plan-sync.test.mjs` связывает: тестовый файл ↔ строка
   §8 ↔ модуль (существует, импортируется) ↔ экспорты (каждый упомянут и протестирован);
   `fixtures-tracked.test.mjs` — фикстуры отслеживаются git; `docs-sync.test.mjs` —
   команды CLI из `HELP` упомянуты в `README.md`.
5. **Проверка кода.** `npm run check` (typecheck + все уровни) — обязательный прогон
   после каждого шага; пороги покрытия `test:coverage` — гейт M1; повторный прогон
   после интеграции воркеров — часть приёмки.
6. **Приёмочный аудит координатора и фикс-волны.** Аудит W2 сверяет область правок
   (`find -newer`, diff против pristine-копий), провенанс (md5 источников) и golden
   своим независимым оракулом. Дефект спеки не правится молча — возвращается владельцу
   файла отдельным ТЗ (пример: W1-fix закрыл 4 дефекта — paths-кейс шага 24,
   недостижимый assert в profile, отсутствующие header-only фикстуры dtls/ice/stun,
   blacklist-кейс против подстрочного правила).

## 5. Инфраструктура и качество

- **Раннер**: `node:test` (Node 24). Ноль зависимостей и конфигов.
- **Покрытие**: `npm run test:coverage` (`--experimental-test-coverage`,
  include только `src/**`).
- **git** (сделан 2026-10-18): baseline `4188bd0`, тег `v0.1.0`, ветка `main`, remote
  `origin` = `github.com/Userbash/discord-lab`; серверная защита `main` недоступна
  (free-план) — аккуратность коммитов на коммиттере.
- **CI** (сделан 2026-10-18, досрочно к M5): `.github/workflows/ci.yml` на push в `main`
  и pull request — `npm ci && npm run gate:worker` (типы → мета → unit → smoke). Остаётся
  на M5: e2e в CI (chromium пакетом), отчёт покрытия, мутации. Live-Discord в CI
  не запускается.
- **Мутации (M5, опционально)**: Stryker на `src/core` с порогом выживаемости ≤ 20%;
  цель — доказать, что тесты «настоящие», а не тавтологичные.

## 6. Матрица паритета: что чем заменяется

| Сейчас | Станет | Тесты | Milestone |
| --- | --- | --- | --- |
| `scripts/logger.mjs pids` | `discord-lab pids [--all]` | `processes.test`, `pids.smoke` | M2 |
| `scripts/logger.mjs inventory` | `discord-lab inventory` | `inventory.test`, smoke (M2) | M1/M2 |
| `scripts/logger.mjs hook` | `discord-lab hook` | fake-device unit (M2) | M2 |
| `scripts/cdp.mjs <sub>` | `discord-lab cdp <sub>` | `cdp.test`, `cdp-browser.e2e` | M1/M3 |
| `scripts/ui-walk-settings.mjs` | `discord-lab walk settings` | `walk-log.test`, `profile.test`, `blacklist.test`, `walk-fixture.e2e` | M3 |
| `scripts/ui-walk-settings2.mjs` | то же (режим «добить пропущенные» внутри walk) | `parseVisited` unit | M3 |
| `scripts/ui-walk-menus.mjs`, `ui-menu-extra.mjs` | `discord-lab walk menus` | те же + fixture e2e | M3 |
| `scripts/summarize-calls.sh` | `discord-lab summarize [файлы]` | `calls.test`, `summarize.smoke` | M1/M4 |
| `scripts/extract-domains.sh` + `domain-report.sh` | `discord-lab domains` | `domains.test`, `cxxfilt.smoke` | M4 |
| `scripts/extract-static-functions.sh` | `discord-lab static` | `nm.test` | M4 |
| `scripts/analyze.sh` (REA) | остаётся тонкой обёрткой (внешний `rea`) | — | вне scope |

Строки M1–M2 закрыты (команды `pids`/`inventory`/`hook`/`summarize`/`cdp targets`
работают и покрыты тестами); открыты строки M3–M4: `walk`, `domains`, `static`.

## 7. Дорожная карта

Статусы на 2026-10-19: M0–M2 — сделаны, M3–M6 — впереди; DoD выполненных милстоунов
ниже сохранены как хроника приёмок.

### M0 — red-базлайн (сделан)

- [x] Приняты решения: CLI `discord-lab`, `node:test`, каркас без логики.
- [x] Каркас `src/` + `bin/` — только сигнатуры, тела `notImplemented`.
- [x] Тесты unit (12 файлов) / smoke (4) / e2e (3), helpers, фикстуры.
- [x] `package.json`: тест-скрипты; `tsconfig.json` покрывает `src/`, `bin/`, `tests/`.
- DoD: `npm test` — красный, каждый провал называет функцию («Не реализовано: …»),
  `npm run typecheck` — зелёный.

### M0.5 — декомпозиция, мета-контроль, углубление тестов (сделан)

- [x] Микро-декомпозиция: `TASKS.md` — волны W0–W8, ID микрозадач, матрица
  непересекающихся файлов, протокол воркеров и приёмки.
- [x] `tests/meta/plan-sync.test.mjs`: §8 ↔ тестовые файлы ↔ модули ↔ экспорты.
- [x] `package.json`: `test:meta`, `check`; пороги покрытия в `test:coverage`.
- [x] Углубление unit/smoke golden-тестами из реальных артефактов (4 воркера, W1 +
  приёмка W2 + фикс-волна W1-fix): calls (voice/стресс-лог),
  walk-log/profile/blacklist (реальный walk-журнал), nm/domains/cxxfilt (таблицы
  символов + REPORT.md), args/csv/paths/inventory/processes (края: кириллические
  метки скриншотов, реальные CSV/JSON инвентаря). Итог: 153 теста — 7 pass /
  142 fail / 4 skip; typecheck и meta зелёные.
- [x] Аудит W2: область правок, провенанс (md5), независимые оракулы; 4 дефекта
  спеки закрыты владельцами (W1-fix, см. `TASKS.md`).
- DoD: `npm run check` — typecheck зелёный, meta зелёный, красные — только
  «Не реализовано: …» (ни одной ошибки загрузки модулей); у каждого добавленного
  golden-теста есть провенанс (артефакт + md5 + оракул). По факту приёмки 140 из
  142 падений буквально называют заглушку; два косвенных (5 CLI-smoke кейсов —
  код выхода 1 вместо 0/2, т.к. бросает `run`; `createCdpClient` «ошибка
  соединения до open» — заглушка бросает до создания fake-socket) снимаются
  реализацией M1.

### M1 — ядро без I/O (сделан, волна W3)

- [x] Все unit-модули §2 от `args.mjs` до `report/domains.mjs` реализованы воркерами
  W3a–W3c (микрозадачи M1-* — `TASKS.md`); CLI-хаб (`run.mjs`) и `summarize` — W3e.
- DoD: `npm run check` зелёный; `npm run test:coverage` проходит пороги
  (строки ≥ 90, функции ≥ 80, ветки ≥ 90).
  Факт приёмки: 153 теста — 148 pass / 0 fail / 5 skip (pids — честный skip до M2,
  4 e2e — skip-aware); покрытие 100 / 100 / 90.48 (строки/функции/ветки).
  Два дефекта спеки, найденные воркерами при реализации, исправлены координатором
  (W3-specfix в `TASKS.md`). Примечание: отчёт покрытия считает только модули,
  загруженные unit-тестами (`cli`/`adapters` в таблицу не попадают) — расширение
  гейта в M5.

### M2 — Frida-слой (сделан, волна W4)

- [x] `src/adapters/frida.mjs` (device/session/script — инжектируемое устройство),
  команды `pids` / `inventory` / `hook` в `src/cli/run.mjs`.
- [x] Unit с fake-device (`tests/unit/frida.test.mjs`, 10/10); smoke `pids` — реальная Frida
  (выполняется, не skip), `inventory`/`hook --pid 99999999` → код 2 (+ анти-заглушка).
- [x] DoD: живой CLI-прогон `pids` (реальные процессы Discord) и коды 2 у inventory/hook.
  Сверка записанных `functions/pid-*.json/.csv` с golden W1d на живом attach — ручной
  чек-лист: на машине `kernel.yama.ptrace_scope=1` (нужен `sysctl kernel.yama.ptrace_scope=0`,
  иначе attach к Discord вне демона невозможен).
- Замер при приёмке M1: полный `npm test` — 200 тестов (196 pass / 0 fail / 4 e2e-skip).
  Актуально (после R7/RC): `npm run check` — **215 тестов (211 pass / 0 fail / 4 e2e-skip)**;
  из robustness-backlog открыт только W3R-HARD-06 (P3).

### M3 — UI-walk

- `src/core/ui/engine.mjs`: шаг = клик → профиль → скриншот → запись; адаптеры
  `discord` / `fixture` (селекторы и подписи).
- DoD: `walk-fixture.e2e` зелёный на headless-браузере (клики, чёрный список,
  скриншоты, JSONL); ручной прогон `walk settings` на живом Discord — совпадает
  по формату логов с прогоном 2026-10-18.

### M4 — отчёты и пайплайны

- `summarize`, `domains`, `static` в CLI; паритет CSV/REPORT.md.
- DoD: на текущих `functions/static/*.nm.txt` вывод совпадает с `REPORT.md` по числам.

### M5 — жёсткость

- CI — **сделан досрочно** (2026-10-18: `6497a4b`, починка `90554c1`); покрытие-гейты — с M1.
- Осталось: мутации (Stryker, опц.), e2e в CI (chromium) и ×5 повторов (стабильность).
- DoD: CI зелёный на push; мутанты ≥ 80% убиты.

### M6 — паритет и уборка

- `scripts/ui-*.mjs`, `logger.mjs`, `cdp.mjs` → тонкие обёртки или удаление;
  унификация типов (`cdp-types.d.ts` → `src/core/types.d.ts`); README.
- DoD: таблица §6 полностью закрыта; README описывает только `discord-lab`.

### R1 — разведка протокола (закрыта 2026-10-19, вне кода CLI)

- Шлюз (WS/ETF/zstd, опкоды), REST-инвентарь, крипто медиа; артефакты — `/tmp/opencode/r2-net/`,
  карточки и статусы — `TASKS.md` §R1. Результаты служат входом для внешнего Rust-проекта
  (UX-клон Discord) и не изменяют код `src/`.

### R4–R6 и D — перенос недостающих функций (2026-10-19, вне кода CLI)

- Полный инвентарь функций, которых нет в нашем проекте (оболочка ОС, стриминг/аудио, платформа,
  данные), — `docs/feature-backlog.md`: 41 карточка с описанием, источником и критерием готовности;
  сравнения с референсами — `docs/vesktop-analysis.md` и `docs/discord-qt-analysis.md`; карта —
  `docs/map.md` (Mermaid). Реализация — внешний Rust-проект (сценарий A, READINESS: GO); в этом
  репозитории волны ведутся как спеки/карточки (`TASKS.md` §R4/R5/R6/D) плюс волна добора данных D.

### R7 — глубокий RE: транспорт, медиа, бэкенд, UI, i18n (закрыта 2026-10-19, вне кода CLI)

- Нормативные документы — `docs/re/` (индекс `docs/re/README.md`, графы процессов/вызовов/функций —
  `docs/re/architecture.md`); сырые доказательства — лаборатория `/home/sanya/re-deep/`
  (в git не попадают).
- Закрыто: шлюз (ETF/zstd/опкоды, D-01 — READY/GUILD_CREATE), REST D-02, UI D-03, голос
  (сигнализация + UDP-структуры + ротация сокета), бэкенд (`main`-процесс, 168 IPC-каналов,
  9 нативных модулей), i18n. Карточки — `TASKS.md` §R7.
- Открыто (по запросу): пересоздание `VOICE-CRYPTO.md` (режимы 0–6), вложенный DAVE/MLS,
  расшифровка talkspurt, GUILD_CREATE через приглашение, spec-pack `messenger-client`.

### RC — Rust-клиент: концепт, архитектура, план, тесты (документы, 2026-10-19, вне кода CLI)

- Пакет `docs/rust-client/`: `CONCEPT.md` (что/зачем, стек, сценарии, критерии), `FEATURES.md`
  (14 групп функций), `ARCHITECTURE.md` (процессы/крейты/шина/хранилище, 8 графов), `PLAN.md`
  (волны P0–P8, 60+ карточек), `TEST-PLAN.md` (L1–L5, ~130 тестов на RE-фикстурах); индекс —
  `README.md`. Назначение: спека-первый вход Rust-проекта — полноценный нативный клиент
  Discord без Electron/Chromium.
- Единый проект (RC-07/RC-08): курированные фикстуры `fixtures/` (12 записей, md5-манифест),
  каркас `rust/` (cargo-воркспейс, 10 крейтов, офлайн), оракул `docs/rust-client/verify/verify-re.mjs`
  (20 сверок, коды 0/1/2), журнал `VERIFICATION.md`, строение `PROJECT-STRUCTURE.md`, трассировка
  `TRACEABILITY.md`.
- В этом репозитории — документы, `tests/meta/rust-client-sync.test.mjs`, фикстуры (`fixtures/`)
  и каркас (`rust/`); полные артефакты — лаборатория `/home/sanya/re-deep/` (`TEST-PLAN.md` §2).
  Карточки — `TASKS.md` §RC.
- Голосовой ввод и шумоподавление (`TASK-633`, `discord-voice`): аппаратный обработчик Push-to-Talk,
  приоритетный голос (`SpeakingFlags::PRIORITY`), задержка отпускания `0..=2000` мс и автомат
  состояния микрофона (`rust/crates/discord-voice/src/ptt.rs`, `rust/crates/discord-voice/tests/ptt_contract.rs`),
  а также конвейер шумоподавления Krisp / RNNoise / WebRTC NS и VAD-гейтинг в дБ
  (`rust/crates/discord-voice/src/noise.rs`, `rust/crates/discord-voice/tests/noise_contract.rs`).

## 8. Матрица тестов: тест ↔ модуль ↔ экспорты

Историческая справка: каркас `src/` создавался **из тестов** — каждая функция описывалась
сигнатурой с телом `Не реализовано: <имя>`; red-прогон показывал очередь работ, а `tsc`
проверял типы самих тестов. После приёмки M1–M2 заглушек не осталось — таблица ниже
стала зелёной матрицей контрактов.

Таблица ниже — **машиночитаемая**: её разбирает `tests/meta/plan-sync.test.mjs`
(строка = один тестовый файл; модули и имена — в косых кавычках; формат — ровно три
колонки). Меняя таблицу, держитесь формата; меняя формат — обновите мета-тест.

| Тест | Модуль | Экспорты / контракт |
| --- | --- | --- |
| `tests/unit/args.test.mjs` | `src/core/args.mjs` | `parseArgs`, `parseNumericArg`, `parseRegexArg` |
| `tests/unit/csv.test.mjs` | `src/core/csv.mjs` | `safeName`, `csv` |
| `tests/unit/paths.test.mjs` | `src/core/paths.mjs` | `projectRoot`, `logsDir`, `functionsDir`, `screensDir`, `screenshotName`, `relativeToRoot` |
| `tests/unit/processes.test.mjs` | `src/core/processes.mjs` | `isDiscordProcess`, `filterDiscordProcesses` |
| `tests/unit/inventory.test.mjs` | `src/core/inventory.mjs` | `inventoryCounts`, `inventoryCsvRows`, `inventoryJson` |
| `tests/unit/profile.test.mjs` | `src/core/ui/profile.mjs` | `topFunctions` |
| `tests/unit/blacklist.test.mjs` | `src/core/ui/blacklist.mjs` | `isBlacklisted` |
| `tests/unit/walk-log.test.mjs` | `src/core/ui/walk-log.mjs` | `parseVisited`, `buildWalkRecord`, `consoleIssues`, `shouldBail`, `pickNextItem` |
| `tests/unit/walk.test.mjs` | `src/core/ui/engine.mjs`, `src/core/ui/adapters.mjs` | `runWalk`, `resolveAdapter`, `clickNavItemExpression` |
| `tests/unit/cdp.test.mjs` | `src/core/cdp/client.mjs` | `resolveEndpoint`, `resolveTargetHint`, `listTargets`, `pickTarget`, `createCdpClient`, `evaluate`, `captureScreenshot`, `clickText`, `clickTextExpression`, `listElements`, `listElementsExpression`, `openPage` |
| `tests/unit/cli-run.test.mjs` | `src/cli/run.mjs` | `run`, `cdp targets`, `pids`, `inventory`, `hook`, `summarize`, `domains`, `static` |
| `tests/unit/calls.test.mjs` | `src/core/calls.mjs` | `isCallPayload`, `callLine`, `parseCallLog`, `summarizeCallLines` |
| `tests/unit/nm.test.mjs` | `src/core/report/nm.mjs` | `parseNmPosix`, `staticFunctionsCsv` |
| `tests/unit/domains.test.mjs` | `src/core/report/domains.mjs` | `classifyName`, `domainCsv`, `formatDomainSummary` |
| `tests/unit/frida.test.mjs` | `src/adapters/frida.mjs` | `loadDevice`, `loadAgentSource`, `listProcesses`, `attach`, `collectInventory`, `parseHookMessage`, `startHook`, `stopHook` |
| `tests/meta/plan-sync.test.mjs` | — | мета: §8 ↔ тесты ↔ модули ↔ экспорты |
| `tests/meta/fixtures-tracked.test.mjs` | — | мета: фикстуры `tests/fixtures` отслеживаются `git` |
| `tests/smoke/cli.smoke.test.mjs` | `bin/discord-lab.mjs`, `src/cli/run.mjs` | `--help`, `--version` |
| `tests/smoke/pids.smoke.test.mjs` | — | `pids` (Frida, skip-aware) |
| `tests/smoke/inventory.smoke.test.mjs` | — | `inventory` (недоступный pid → код 2) |
| `tests/smoke/hook.smoke.test.mjs` | — | `hook` (недоступный pid → код 2) |
| `tests/smoke/summarize.smoke.test.mjs` | — | `summarize` (паритет summarize-calls.sh) |
| `tests/smoke/cxxfilt.smoke.test.mjs` | `src/adapters/cxxfilt.mjs` | `demangle` |
| `tests/e2e/cdp-browser.e2e.test.mjs` | — | `targets`, `eval`, `clicktext`, `list`, `shot` |
| `tests/e2e/walk-fixture.e2e.test.mjs` | — | `walk` |
| `tests/e2e/live-discord.e2e.test.mjs` | — | `targets`, `eval` (только чтение, opt-in) |
| `tests/unit/hook-io.test.mjs` | `src/core/hook-io.mjs` | `openCallLog`, `waitForStop` |
| `tests/unit/frida-agent.test.mjs` | — | `inventory`, `hook`, `unhook` |
| `tests/unit/gui-spike-bench.test.mjs` | `scripts/gui-spike-bench.mjs` | `summarize`, `percentile95`, `parseStatus`, `parseCliArgs` |
| `tests/unit/test-impact.test.mjs` | `scripts/test-impact.mjs` | `planFor`, `docs-sync`, `--full` |
| `tests/unit/gui-spike-model.test.mjs` | `rust/gui-spike/ui/model.js` | `PageStore`, `getRow`, `ensureRange`, `stats` |
| `tests/unit/channel-paging.test.mjs` | `rust/gui-spike/src/data.rs` | `messages_page`, `nextCursor`, `hasMore`, `loaded_rows_limit` |
| `tests/unit/gui-spike-dom.test.mjs` | `rust/gui-spike/ui/index.html`, `rust/gui-spike/ui/app.js` | DOM-контракт: 4 экрана, обязательные id, константы |
| `tests/unit/gui-spike-features.test.mjs` | `rust/gui-spike/ui/app.js`, `rust/gui-spike/ui/features/settings.js`, `rust/gui-spike/ui/features/emoji.js`, `rust/gui-spike/ui/features/search-overlay.js` | `loadApp`, `data-action`, `settings`, `emoji`, `search` |
| `tests/unit/dm-friends-ui.test.mjs` | `rust/gui-spike/ui/features/dm-friends.js` | `createDmFriendsController`, `DM_FRIENDS_ENDPOINTS`, `FRIENDS_TABS`, `applyDmFriendsDispatch`, `sortPrivateChannels` |
| `tests/meta/test-policy.test.mjs` | — | `TEST-POLICY`, `--no-verify`, `gate.mjs` |
| `tests/meta/design-breakpoints.test.mjs` | — | мета: адаптив shell 1600/1200/960/720 (`--layout-tree-w`, `--layout-tree-min-w`, `#member-panel`, `channel-tree`) |
| `tests/meta/design-tokens-contrast.test.mjs` | — | мета: Design Tokens V2 и контраст WCAG 2.2 (`WCAG 2.2`, `design-tokens-v2.md`, `DESIGN-TOKENS-V2.md`, `contrastRatio`, `relativeLuminance`, `Midnight`) |
| `tests/unit/gui-spike-regressions.test.mjs` | — | `quick-switcher`, `quick-switcher-empty`, `msg-toolbar`, `delete_message` |
| `tests/unit/channel-switch.test.mjs` | — | `channels`, `messages_page`, `channel-item`, `chat-title-text`, `G6-D-01` |
| `tests/unit/guild-rail.test.mjs` | — | `ready`, `guild-rail`, `data-guild-id` |
| `tests/unit/voice-ui.test.mjs` | — | `voice-panel`, `voice-spec`, `data-state` |
| `tests/unit/voice-devices-settings.test.mjs` | — | `createVoiceDevicesSettings`, `DeviceFallback`, `NoDeviceAvailable` |
| `tests/unit/attachments-render.test.mjs` | — | `renderChannelMessage`, `decodeWaveform`, `IS_VOICE_MESSAGE` |
| `tests/unit/notifications-ui.test.mjs` | — | `MUTE_TIMER_PRESETS`, `MESSAGE_NOTIFICATION_LEVELS`, `NOTIFY_HIGHLIGHTS` |
| `tests/unit/join-server.test.mjs` | — | `join-server`, `resolve_invite`, `accept_invite` |
| `tests/unit/create-guild.test.mjs` | `rust/gui-spike/ui/features/create-guild.js` | STUB-03, `create_guild`, `create-guild-dialog` |
| `tests/unit/create-guild-wizard.test.mjs` | `rust/gui-spike/ui/features/create-guild.js` | `CREATE_GUILD_TEMPLATES`, `validateGuildIconDataUri`, `createCreateGuildWizard` |
| `tests/unit/read-state-ui.test.mjs` | — | `read_state`, `ack_message`, `chat-unread-divider` |
| `tests/unit/chat-reactions.test.mjs` | — | `toggle_reaction`, `messageId`, `reaction` |
| `tests/unit/typing-ui.test.mjs` | — | `typing-indicator`, `send_typing`, `TYPING_RESEND_MS` |
| `tests/unit/gui-spike-auth.test.mjs` | `rust/gui-spike/ui/features/auth.js`, `rust/gui-spike/ui/app.js` | `auth-gate`, `auth_login_token`, `auth_status` |
| `tests/smoke/gui-spike.smoke.test.mjs` | — | live smoke: `window-ready`, `UI_SNAPSHOT` (`GUI_SPIKE_E2E=1`) |
| `tests/smoke/gui-window.smoke.test.mjs` | `scripts/gui-window-probe.py` | AT-SPI/X11: окно, элементы, PNG, GBM |
| `tests/meta/docs-sync.test.mjs` | — | мета: `HELP` ↔ `README.md` (команды CLI задокументированы) |
| `tests/meta/backlog-sync.test.mjs` | — | мета: карточки `docs/feature-backlog.md` ↔ README/TASKS/карта |
| `tests/meta/rust-client-sync.test.mjs` | — | мета: план Rust-клиента (docs/rust-client) ↔ README/TASKS/карта |
| `tests/meta/waves-sync.test.mjs` | — | мета: `waves/db.jsonl` (схема v1) ↔ `STATE.md` и `evidence` |
| `tests/meta/keyring-live.test.mjs` | — | мета: live keyring `#[ignore]` ↔ `--ignored` ↔ `failOnSkips` |
| `tests/meta/ipc-path.test.mjs` | — | мета: IPC не принимает путь — `scan` сигнатур `#[tauri::command]` (`PathBuf`, `File`, `path`/`root`/`dir`), `tauri-plugin-fs`/`tauri-plugin-dialog`, 0 violations |
| `tests/meta/ci-browser.test.mjs` | — | мета: CI `qa-browser` — pinned Chromium, `CHROME_BIN`, preflight, `--test-concurrency=1`, `.gate` |
| `tests/meta/e2e-required.test.mjs` | — | мета: `m2-scenarios.json`, `failOnSkips`, `requireBrowser`, `InfraError` |
| `tests/meta/mutations.test.mjs` | — | `15/15`, `mutations`, `killed`, `LatestFrameSlot`, `DAVE`, `hCaptcha` |
| `tests/meta/parity-matrix.test.mjs` | — | `MERGE-READINESS-V0.5.0.md`, `RELEASE-NOTES-V0.5.0.md`, `parseParityRows`, `verifyDomainSummary` |
| `tests/unit/dispatch_ui.test.mjs` | — | `gateway://dispatch`, `FORWARDED_DISPATCHES`, `createDispatchBridge`, `READY`, `PRESENCE_UPDATE` |
| `tests/unit/search-remote-real-index.test.mjs` | — | `search_remote`, `data-guild-id`, `ready`, `002-READY.json` |
| `tests/e2e/m2-login-ready-rail.e2e.test.mjs` | — | `auth_status`, `auth_login_token`, `ready`, `002-READY.json` |
| `tests/e2e/m2-channel-send.e2e.test.mjs` | — | `M2_IDS`, `m2Channels`, `MESSAGE_CREATE`, `openApp` |
| `tests/e2e/m2-search-error.e2e.test.mjs` | — | `search_remote`, `data-guild-id`, `openApp` |
| `tests/e2e/m2-invite.e2e.test.mjs` | — | `resolve_invite`, `accept_invite`, `JOIN_TEXT`, `openApp` |
| `tests/e2e/m2-settings-reload.e2e.test.mjs` | — | `openApp`, `m2FixtureData` |
| `tests/e2e/m2-reconnect.e2e.test.mjs` | — | `NET_STATUSES`, `NET_TEXT`, `net_connect`, `openApp` |
| `tests/e2e/m2-voice-idle.e2e.test.mjs` | — | `VOICE_TEXT`, `disabled`, `openApp` |
| `tests/e2e/e2e-build006.test.mjs` | — | `openApp`, `captcha-modal`, `create-guild-dialog`, `stream-viewer`, `search-popover`, `context-menu`, `advanced-settings`, `resolve_gift_code` |
| `tests/unit/composer.test.mjs` | `rust/gui-spike/ui/features/composer.js` | `ATTACH_LIMITS`, `ATTACH_TEXT`, `GIFT_TEXT`, `createComposer`, `formatBytes` |
| `tests/unit/server-menu.test.mjs` | `rust/gui-spike/ui/features/actions.js` | `MENU_LABELS`, `REASONS`, `loadApp` |
| `tests/unit/account-dynamic-sync.test.mjs` | — | — |
| `tests/unit/advanced-settings.test.mjs` | `rust/gui-spike/ui/features/advanced-settings.js` | `createAdvancedSettingsController`, `detectKeybindConflicts`, `resolveEntranceSoundForGuild`, `normalizeClipLengthSeconds` |
| `tests/unit/appearance-settings.test.mjs` | — | — |
| `tests/unit/attachment-helper.test.mjs` | — | — |
| `tests/unit/attachment-validator.test.mjs` | — | — |
| `tests/unit/attachments-ui.test.mjs` | — | — |
| `tests/unit/audit-log.test.mjs` | — | — |
| `tests/unit/automod-editor.test.mjs` | — | — |
| `tests/unit/bindings-permissions.test.mjs` | — | — |
| `tests/unit/captcha-modal.test.mjs` | `rust/gui-spike/ui/features/captcha-modal.js` | `createCaptchaModal`, `extractCaptchaChallenge`, `captcha-modal` |
| `tests/unit/call_stream_dispatch.test.mjs` | — | — |
| `tests/unit/call_stream_lifecycle.test.mjs` | — | — |
| `tests/unit/stream_restart_lifecycle.test.mjs` | — | — |
| `tests/unit/screen_color_and_window_target.test.mjs` | — | — |
| `tests/unit/stream_broadcaster_handshake_and_source.test.mjs` | — | — |
| `tests/unit/stream_viewer_close_and_source_switch.test.mjs` | — | — |
| `tests/unit/channel-gate.test.mjs` | — | — |
| `tests/unit/channel-tree.test.mjs` | — | — |
| `tests/unit/composer-autocomplete.test.mjs` | — | — |
| `tests/unit/context-menu.test.mjs` | — | — |
| `tests/unit/embed-formatter.test.mjs` | — | — |
| `tests/unit/event-banner.test.mjs` | — | — |
| `tests/unit/events-modal.test.mjs` | — | — |
| `tests/unit/expression-picker.test.mjs` | — | — |
| `tests/unit/forum-filter.test.mjs` | — | — |
| `tests/unit/forum-view.test.mjs` | — | — |
| `tests/unit/friends-home.test.mjs` | — | — |
| `tests/unit/gift-modal.test.mjs` | `rust/gui-spike/ui/features/gift-modal.js` | `createGiftModal`, `resolveGiftStyle`, `extractGiftCodesFromText`, `classifyGiftError` |
| `tests/unit/guild-folders.test.mjs` | — | — |
| `tests/unit/guild-rail-incremental.test.mjs` | — | — |
| `tests/unit/guild-modals.test.mjs` | — | — |
| `tests/unit/i18n.test.mjs` | — | — |
| `tests/unit/inbox.test.mjs` | — | — |
| `tests/unit/inbox-panel.test.mjs` | — | — |
| `tests/unit/invite-modal.test.mjs` | — | — |
| `tests/unit/invite-parser.test.mjs` | — | — |
| `tests/unit/keybinds.test.mjs` | — | — |
| `tests/unit/locale-switcher.test.mjs` | — | — |
| `tests/unit/markdown-helper.test.mjs` | — | — |
| `tests/unit/member-list.test.mjs` | — | — |
| `tests/unit/members-subscriptions.test.mjs` | — | — |
| `tests/unit/message-actions.test.mjs` | — | — |
| `tests/unit/message-context-actions.test.mjs` | — | — |
| `tests/unit/message-media-polls.test.mjs` | — | — |
| `tests/unit/message-delete-voice-exit-ui.test.mjs` | — | — |
| `tests/unit/notifications.test.mjs` | — | — |
| `tests/unit/path1-frameworks.test.mjs` | — | — |
| `tests/unit/pins-threads.test.mjs` | — | — |
| `tests/unit/plural-format.test.mjs` | — | — |
| `tests/unit/poll-card.test.mjs` | — | — |
| `tests/unit/poll-progress.test.mjs` | — | — |
| `tests/unit/presence-activity.test.mjs` | — | — |
| `tests/unit/profile-modal.test.mjs` | — | — |
| `tests/unit/qr-auth-modal.test.mjs` | `rust/gui-spike/ui/features/qr-auth-modal.js` | `createQrAuthModal`, `parseQrAuthUrl`, `evaluateQrSecurityIndicator`, `classifyQrAuthError` |
| `tests/unit/quick-switcher.test.mjs` | — | — |
| `tests/unit/reactions-ui.test.mjs` | — | — |
| `tests/unit/reactive-store.test.mjs` | — | — |
| `tests/unit/role-editor.test.mjs` | — | — |
| `tests/unit/role-permissions.test.mjs` | — | — |
| `tests/unit/search-panel.test.mjs` | — | — |
| `tests/unit/search-popover.test.mjs` | — | — |
| `tests/unit/settings-modal.test.mjs` | — | — |
| `tests/unit/shell-bridge.test.mjs` | — | — |
| `tests/unit/shortcuts.test.mjs` | — | — |
| `tests/unit/slowmode.test.mjs` | — | — |
| `tests/unit/stage-channel.test.mjs` | — | — |
| `tests/unit/stream-viewer.test.mjs` | — | — |
| `tests/unit/stream-viewer-ui.test.mjs` | — | — |
| `tests/unit/stream-signaling-parity.test.mjs` | — | — |
| `tests/unit/svg-icons.test.mjs` | — | — |
| `tests/unit/threads-drawer.test.mjs` | — | — |
| `tests/unit/token-map.test.mjs` | — | — |
| `tests/unit/tooltip.test.mjs` | — | — |
| `tests/unit/typing-indicator.test.mjs` | — | — |
| `tests/unit/unread-bar.test.mjs` | — | — |
| `tests/unit/user-popout.test.mjs` | — | — |
| `tests/unit/voice-call-phases-and-server-access.test.mjs` | — | — |
| `tests/unit/voice-grid.test.mjs` | — | — |
| `tests/unit/voice-input-mode.test.mjs` | — | — |
| `tests/unit/voice-quality.test.mjs` | — | — |
| `tests/unit/voice-stage-ui.test.mjs` | — | — |
| `tests/unit/voice-volume.test.mjs` | — | — |

Цикл для новой функциональности (рецепт):

```bash
npm run test:unit 2>&1 | head            # смотрим первый красный
# 1) открываем тест → 2) минимально реализуем в src/core → 3) повторяем
npm run check                            # typecheck + все тесты (unit/meta/smoke/e2e)
```

## 9. Риски и ограничения

- **Нет браузера в системе** — e2e CDP пропускаются (skip с подсказкой про `CHROME_BIN`);
  в CI ставится chromium пакетом. Это осознанная деградация, не «зелёный самообман»:
  skip виден в отчёте раннера.
- **Live-Discord** зависит от запуска с `--remote-debugging-port` и включается только
  явным `DISCORD_LAB_LIVE_CDP`; тесты в этом режиме — строго read-only.
- **Frida attach** требует `kernel.yama.ptrace_scope=0`; smoke-тест `pids` attach
  не делает (перечисление процессов работает всегда), поэтому безопасен.
- **walk на живом Discord** не автоматизируется (аккаунт, необратимые пункты) —
  регрессии ловятся fixture-e2e + ручным прогоном по чек-листу из README.
- **Дублирование поведения** `scripts/*` ↔ `src/*` живёт до M6; расхождения допустимы
  только как исправления в `src/`, зафиксированные тестами.
- **Headless Chromium ≠ Electron Discord** — CDP-поверхность близка, но не идентична;
  поэтому e2e проверяет протокол и движок, а не «дизайн Discord».
- **Покрытие `node:test`** помечено экспериментальным (флаг `--experimental-test-coverage`);
  если флаг изменится — правится один npm-скрипт.
- **Golden-фикстуры привязаны к реальным артефактам** (md5 в провенансе): фикстура
  остаётся валидной копией артефакта; smoke-проверки полного артефакта пропускаются,
  если файл отсутствует.
- **Robustness-находки ревью W3** (W3R-HARD-01…06, `TASKS.md`): HARD-01…05 закрыты волной
  W3R (2026-10-18); HARD-06 (P3) — частично: `engines` добавлен, `packageVersion` под try;
  открыто: счётчики битых строк в `parseCallLog`/`parseVisited` — backlog до M4.

## 10. Открытые вопросы

1. `git init` — **сделано (2026-10-18)**: репозиторий, baseline `4188bd0`, тег `v0.1.0`.
2. Перенести инструменты прогона №2 из `/tmp/opencode/discord-ui-walkthrough/` в `tools/`?
3. В M6 `scripts/*` удаляем или оставляем тонкими обёртками?
4. Логи `logs/ui-clicks-*.jsonl` (события кликов `{at,event,target}`) пока без
   потребителя в `src/` — нужна ли отдельная команда/модуль (backlog)?
5. **Решено (W4-fix, 2026-10-18)**: истинный граф (caller→callee) не строим. Добавлен
   опциональный `--backtrace` (по умолчанию off): агент пишет `stack` (до 8 фреймов
   `module+0xoffset`) в запись вызова; подтверждено живым прогоном на Discord
   (11 582/11 582 записей со `stack`). Дефолтный плоский формат сохранён.
   Детали — `TASKS.md`, W4f-08/W4f-12.
6. **R1 (19.10.2026)**: опции по данным — свежий READY/GUILD_CREATE (нужен полный
   перезапуск копии) и source maps веб-бандла (нужен согласованный сетевой доступ);
   контекст — `/tmp/opencode/r2-net/READINESS.md`, карточки — `TASKS.md` §R1.
   Продолжение — **R7 (закрыта 19.10.2026)**: READY снят (4 логина, D-01 — `docs/re/gateway-protocol.md`),
   REST и UI закрыты (D-02/D-03), бэкенд разобран (`docs/re/main-arch.md`, `backend-ipc.md`,
   `native-re.md`); source maps и GUILD_CREATE-через-приглашение остаются открытыми.
7. **RC (19.10.2026)**: план Rust-клиента (нативный клиент без Electron/Chromium) — пакет
   `docs/rust-client/` (концепт, функции, архитектура, план P0–P8, тест-план); карточки —
   `TASKS.md` §RC; синхронность — `tests/meta/rust-client-sync.test.mjs`. Единый проект:
   фикстуры `fixtures/` (12 записей, md5-манифест), каркас `rust/` (cargo, 10 крейтов,
   офлайн `cargo test`), верификация — `docs/rust-client/verify/verify-re.mjs` (20/20,
   журнал — `VERIFICATION.md`); неизвестные RE (DAVE/MLS-детали, talkspurt, утраченный
   `VOICE-CRYPTO.md`) учтены как риски фаз P4/P4.5.

## 11. Микро-декомпозиция и воркеры

- Источник истины по задачам — `TASKS.md`: волны W0–W8 и R1, ID микрозадач, зависимости,
  владельцы (C — координатор, W* — воркеры), DoD и матрица непересекающихся файлов.
- Ядро M1 декомпозировано до «функция / кластер поведения = микрозадача»; M2–M6 —
  до задач волн, разворачиваемых в микрозадачи при старте волны.
- Репозиторий — git (`main`, baseline `4188bd0`, тег `v0.1.0`); воркеры работают в общем
  рабочем каталоге с непересекающимися наборами файлов (один писатель на файл);
  копии `project-w1…w4` не используются.
- Интеграцию копий, финальные `npm run check` и обновление документов ведёт
  координатор — это неделимая часть.
- Приёмка волны: `npm run gate:worker` (`gate:release` — на выпуске, `gate:qualify` — на
  квалификации; см. `PROCESS.md`); мета-тесты зелёные; у новых тестов есть red-прогон
  (или skip с причиной отсутствия среды); отчёт воркера с провенансом golden-данных;
  обновление статусов в `TASKS.md`.
