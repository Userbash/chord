# TASKS.md — микро-декомпозиция работ `discord-lab`

Обновлено: 2026-10-18 (волны W1–W4 закрыты и приняты: M1 и M2 сделаны — 176 тестов,
172 pass / 0 fail / 4 e2e-skip; покрытие 99.3/90.5/97.6; пост-W4 аудит — в backlog ниже).
Введён производственный процесс `PROCESS.md` (роли, гейты `gate:worker/release/qualify`,
шаблоны ТЗ/отчётов) и `AGENTS.md` — краткие правила агентам-воркерам. Контекст и
правила — `TDD-PLAN.md` (§8 — машиночитаемая таблица «тест ↔ модуль ↔ экспорты»;
мета-контроль — `tests/meta/plan-sync.test.mjs`).

## Как читать

- **ID**: `W<волна>-<группа>-<номер>` — инфраструктура/тесты/делегирование;
  `M<милестоун>-<МОДУЛЬ>-<номер>` — реализация по тестам.
- **Статусы**: `[ ]` todo · `[~]` in progress · `[x]` done · `[!]` blocked · `[s]` skip.
- **Владельцы**: `C` — координатор (неделимая часть: план, мета-тесты, инфраструктура,
  интеграция и приёмка), `W1a…W1d` и `W3a…W3e` — воркеры соответствующих волн.
- Полный список кейсов живёт в тестовых файлах; здесь — кластеры поведения и ссылки.
  Добавили кейс — добавьте строку микрозадачи в соответствующую таблицу.
- Репозиторий: git (ветка `master`); параллельные писатели работают в ОДНОЙ папке
  `/home/sanya/orca/project` с непересекающимися файлами — уровень L1 процесса
  (`PROCESS.md` §2); orca-worktree + ветка `w/<task>` — уровень L2, включается
  при появлении remote/CI. Коммиты делает координатор.

## Волновая карта

```
W0 (C) ──► W1 (W1a…W1d) ──► W1-fix ──► W2 (C) ──► W3 (M1: W3a…W3e)
план,      углубление       4 дефекта   приёмка      реализация ядра,
мета,      тестов golden-   спеки       и проверки   CLI-хаб, паритет
инфра      данными

W3R (hardening: cxxfilt/CDP/args) ⊕ W4 (M2: frida → CLI) ──► W5 (walk-движок, M3) ──► W6 (отчёты, M4)
                          ──► W7 (CI/мутации, M5) ──► W8 (паритет, M6)
```

Критерий делегирования: задача попадает воркеру, если её можно выполнить в общей
папке с непересекающимся множеством файлов. Всё, что связывает волны (план,
метатесты, интеграция, финальные прогоны), остаётся у `C`.

## W0 — план, мета-контроль, инфраструктура (координатор)

| ID | Действие | Артефакт | Статус |
| --- | --- | --- | --- |
| W0-01 | Аудит и обновление плана: §4 методы, §7 M0.5, §8 машиночитаемость, §11 воркеры | `TDD-PLAN.md` | [x] |
| W0-02 | Микро-декомпозиция (этот файл): волны, ID, матрица файлов, протокол | `TASKS.md` | [x] |
| W0-03 | Мета-тест «§8 ↔ тесты ↔ модули ↔ экспорты» | `tests/meta/plan-sync.test.mjs` | [x] |
| W0-04 | Скрипты `test:meta`, `check`; пороги покрытия 90/80/90 | `package.json` | [x] |
| W0-05 | Режим параллельных воркеров: Orca Run, диспетчеризация, общая папка вместо копий | Orca Run `run_949f9e36ec2b` | [x] |
| W0-06 | Интеграция результатов волн, финальные `npm run check`, обновление документов | все | [~] |

## W1 — углубление тестов golden-данными (4 воркера параллельно)

Общая приёмка W1: `node --test <свои файлы>` — все падения только «Не реализовано: …»;
`npx tsc -p tsconfig.json --noEmit` — 0; отчёт с провенансом (артефакт + md5 + оракул)
и командами; изменены только назначенные файлы.

| ID | Владелец | Цель | Владеет файлами | Статус |
| --- | --- | --- | --- | --- |
| W1a-01 | W1a | Golden `calls`: фикстура из `logs/calls-voice-28170.jsonl` (6 функций, 4297 вызовов) + стресс-кейс | `tests/unit/calls.test.mjs`, `tests/fixtures/logs/calls-voice*.jsonl` | [x] |
| W1a-02 | W1a | Smoke `summarize` на полном реальном логе: CSV построчно равен оракулу `jq \| sort \| uniq -c` | `tests/smoke/summarize.smoke.test.mjs` | [x] |
| W1b-01 | W1b | Golden `parseVisited`/`buildWalkRecord` из `logs/ui-walk-settings.jsonl` (56 записей) и `logs/ui-walk-menus.jsonl` (52) | `tests/unit/walk-log.test.mjs`, `tests/fixtures/logs/walk-*.jsonl` | [x] |
| W1b-02 | W1b | `consoleIssues` на реальных `topFunctions`/ошибках журналов; границы обрезки 220 | `tests/unit/walk-log.test.mjs` | [x] |
| W1b-03 | W1b | `topFunctions` и `isBlacklisted` на реальных подписях из журналов и `scripts/ui-walk-settings.mjs` | `tests/unit/profile.test.mjs`, `tests/unit/blacklist.test.mjs` | [x] |
| W1c-01 | W1c | Golden `parseNmPosix`/`staticFunctionsCsv` на выдержках из `functions/static/*.nm.txt` (T/t/W/w, размеры, дубликаты) | `tests/unit/nm.test.mjs`, `tests/fixtures/nm/*` | [x] |
| W1c-02 | W1c | `classifyName` golden по `functions/domains/<domain>.csv` (демангленные имена → ожидаемый домен), `domainCsv`/`formatDomainSummary` | `tests/unit/domains.test.mjs`, `tests/fixtures/domains/*` | [x] |
| W1c-03 | W1c | Smoke `cxxfilt`: реальные манагленные имена из `functions/static/*.nm.txt`; порядок и неманагленные | `tests/smoke/cxxfilt.smoke.test.mjs` | [x] |
| W1d-01 | W1d | Golden `inventory` по `functions/pid-23548-Discord.csv/.json` (заголовок, kind/type, offset, safeName) | `tests/unit/inventory.test.mjs`, `tests/fixtures/inventory/*` | [x] |
| W1d-02 | W1d | `screenshotName`/`relativeToRoot` по реальным именам `evidence/screens/*.png` (кириллица, пунктуация, длина) | `tests/unit/paths.test.mjs` | [x] |
| W1d-03 | W1d | Края `parseArgs`/`csv`/`isDiscordProcess` (повторы ключей, `--`-подобные токены, пустые значения, регистр процессов) | `tests/unit/args.test.mjs`, `tests/unit/csv.test.mjs`, `tests/unit/processes.test.mjs` | [x] |

## W1-fix — устранение дефектов спеки, найденных приёмкой (владельцы файлов)

| ID | Владелец | Дефект | Правка | Статус |
| --- | --- | --- | --- | --- |
| W1-fix-01 | W1d | `paths`: кейс шага 24 ссылался на несуществующие подпись/файл (`ui-menu-24-…-Утилиты.png`); prefix завязан на номера шагов | реальные тройки 23/24/35 из журнала; prefix задаётся явно в кейсе | [x] |
| W1-fix-02 | W1b | `profile`: assert `getBoundingClientRect hits === 5` недостижим в top-8; `blacklist`: «Сначала выйти» → false против подстрочного правила; `walk-log`: вакуумный кейс shouldBail | golden top-8 из журналов (771/697-оракул); подстрока → true; явный ноль error-записей | [x] |
| W1-fix-03 | W1c | `domains`: нет header-only фикстур dtls/ice/stun → ENOENT после реализации | добавлены 3 файла (md5 заголовка `97f6689a…`) | [x] |

## W2 — приёмка волны W1 (координатор) — закрыт

| ID | Действие | Статус |
| --- | --- | --- |
| W2-01 | Область правок: `find -newer` + diff против pristine-копий — только назначенные файлы | [x] |
| W2-02 | Независимая перепроверка golden своим оракулом (jq/awk/c++filt/nm, md5, байтовые сверки) | [x] |
| W2-03 | Финальный прогон в основной копии (`npm run check`: typecheck+meta зелёные; 153 теста — 7 pass / 142 fail / 4 skip) | [x] |
| W2-04 | Детерминизм (повторные прогоны), обновление §8/счётчиков/статусов TASKS | [x] |

## W3 — реализация M1 воркерами (микрозадачи M1-* ниже)

| ID | Воркер | Файлы реализации | Покрывает | Статус |
| --- | --- | --- | --- | --- |
| W3a | W1d-терминал | `src/core/{args,csv,paths,processes,inventory}.mjs` | M1-ARGS…M1-INV | [x] |
| W3b | W1c-терминал | `src/core/calls.mjs`, `src/core/report/{nm,domains}.mjs`, `src/adapters/cxxfilt.mjs` | M1-CALLS…M1-CXXFILT | [x] |
| W3c | W1b-терминал | `src/core/ui/{profile,blacklist,walk-log}.mjs` | M1-PROFILE/BLACK/WALK | [x] |
| W3d | новый воркер | `src/core/cdp/client.mjs` | M1-CDP-01…07 | [x] |
| W3e | W1c-терминал (после W3a–W3d) | `src/cli/run.mjs` (bin не менялся) | M1-CLI, M1-SUM | [x] |

Правило волны: код пишется только под тесты; тесты не меняются (дефект спеки —
вопрос координатору); после каждой пары микрозадач — прогон своих тестов; W3e
запускается, когда зависимости (`calls`, `cdp`) уже реализованы.

Приёмка W3 (координатор): область правок — ровно 14 `src`-файлов (13 из W3a–W3d +
`run.mjs`, плюс doc-правка координатора в `run.mjs` после приёмки); `bin/discord-lab.mjs`
и `src/core/not-implemented.mjs` не тронуты; тесты воркеры не меняли. `npm run check`:
typecheck + meta + 153 теста — 148 pass / 0 fail / 5 skip (pids — честный skip до M2
через код 2 + «Frida недоступна»; 4 e2e — skip-aware без браузера/live-CDP).
`npm run test:coverage`: пороги 90/80/90 пройдены (факт 100 / 100 / 90.48; отчёт
считает только модули, загруженные unit-тестами — расширение гейта в M5-COV-01).

## W3-specfix — дефекты спеки, найденные воркерами при реализации (правки координатора)

Оба дефекта воркеры не «подгоняли» кодом, а подняли через `ask` — протокол сработал;
правки внесены координатором с двойным оракулом и зафиксированы здесь.

| ID | Дефект | Правка | Статус |
| --- | --- | --- | --- |
| W3-specfix-01 | `blacklist`: кейс «не блокирует подписи из реальных журналов» требовал `false` для всех 108 записей, но строгое объединённое правило обязано блокировать 3 реальные подписи | кейс заменён на «блокирует ровно три подписи-исключения…»: «Статус учётной записи» (settings-4), «Игнорировать и заблокировать» (settings-26), «Меню сервера2 → Скрыть заглушенные каналы» (menus-38); оракул — union-regex по всем 108 записям обеих фикстур; остальные 105 записей обязаны быть разрешены | [x] |
| W3-specfix-02 | `domains`: в ожидаемой строке `latency` было 13 пробелов против 12 у printf-формата и реального `functions/domains/summary.txt` | в `tests/unit/domains.test.mjs` исправлен ровно один пробел; после правки 12/12 зелёные; оракулы: `printf '%-12s %8d'` + байты `summary.txt` | [x] |

## W3-review — находки ревью кода (Node/память/безопасность), волна хардненинга

Ревью 2026-10-18 (полный отчёт: `/tmp/opencode/review-w3/security-review.md`; выполнено
координатором — неделимая аналитика). Графы: SocratiCode — 55 файлов / 44 ребра / 0 циклов;
реальные call-логи плоские (caller не пишется) — истинный граф вызовов невозможен без
backtrace (вопрос в TDD-PLAN §10). Уязвимостей (инъекции/eval/ReDoS) не найдено: `spawn` без
shell, `JSON.parse` недоверенного ввода в try, пути санитизированы; найденное — устойчивость.
Правило TDD: на каждый пункт red-кейс → минимальная правка.

| ID | Находка | Файл | Кейс (red) | Приоритет | Статус |
| --- | --- | --- | --- | --- | --- |
| W3R-HARD-01 | `demangle`: нет таймаута (подтверждено прогоном: висяк); при раннем выходе `c++filt` (EPIPE, ввод > буфера канала) запись в `child.stdin` без 'error'-обработчика → uncaught-исключение (подтверждено) | `src/adapters/cxxfilt.mjs` | `tests/smoke/cxxfilt.smoke.test.mjs` (fake c++filt через PATH) | P1 | [x] W4h1 |
| W3R-HARD-02 | `listTargets`: fetch без таймаута; `response.json()` без проверки массива | `src/core/cdp/client.mjs` | `tests/unit/cdp.test.mjs` | P1 | [x] W4h2 |
| W3R-HARD-03 | CDP `events`: неограниченный рост + нет окна событий на шаг (M3: `consoleIssues` считает всю сессию) | `src/core/cdp/client.mjs` | `tests/unit/cdp.test.mjs` | P1 (дизайн M3) | [x] W4h2 |
| W3R-HARD-04 | `send()`: синхронный throw `ws.send` после close оставляет pending+timer до таймаута | `src/core/cdp/client.mjs` | `tests/unit/cdp.test.mjs` | P2 | [x] W4h2 |
| W3R-HARD-05 | `parseArgs`: reserved keys (`_`, `modules`, `__proto__`, `constructor`) перезаписывают структуру результата | `src/core/args.mjs` | `tests/unit/args.test.mjs` | P2 | [x] W4h3 |
| W3R-HARD-06 | `packageVersion` без try; `parseCallLog`/`parseVisited` без счётчика битых строк; нет `engines` в `package.json` | `src/cli/run.mjs`, `src/core/calls.mjs`, `src/core/ui/walk-log.mjs`, `package.json` | свои unit | P3 | [ ] |
| W3R-PLAN-01 | TDD-PLAN §11: воркеры «в копиях проекта» — расхождение с фактическим протоколом (общая папка) | `TDD-PLAN.md` | — (док, координатор) | — | [x] |

Итог волны W4 (2026-10-18, приёмка координатора): HARD-01…05 закрыты — cxxfilt
(`timeoutMs=30000` + SIGKILL при таймауте + обработка stdin `'error'`; 6/6 smoke), CDP
(таймауты `listTargets`/`send`, проверка массива целей, окно событий `mark`/`eventsSince`,
`drainEvents` очищает на месте, отказ `send` после `close`; 23/23 unit), args (RESERVED-ключи;
18/18). Полная приёмка: `npm run check` — 176 тестов (172 pass / 0 fail / 4 e2e-skip);
покрытие 99.30/90.45/97.56; tsc 0; md5-аудит — правки только в назначенных файлах. HARD-06
(P3) остаётся в backlog. Вопрос W4b о заглушке-контракте легитимен: контракты усилены
`assert.doesNotMatch(/появится в M2/)` (`tests/smoke/hook.smoke.test.mjs`, `inventory.smoke.test.mjs`).
Отклонение по терминалам: все четыре codex-терминала волны — retained (живые агенты под M3).

## M1 — ядро без I/O: микрозадачи реализации

Формат: `M1-<МОДУЛЬ>-<номер>` — один кластер поведения (кейсы — в тестовом файле).
Правило TDD: задача закрывается только зелёным тестом; красный прогон до кода обязателен.

### M1-ARGS — `src/core/args.mjs` (`parseArgs`)

| ID | Микрозадача | Кейсы в `tests/unit/args.test.mjs` |
| --- | --- | --- |
| M1-ARGS-01 | База: пустой argv → `_`/`modules` пусты; позиционные в `_` по порядку | «пустой argv», «позиционные токены» |
| M1-ARGS-02 | Пары `--key value` и `--key=value` | «--key value съедает…», «--key=value…» |
| M1-ARGS-03 | Флаги: `--flag` → `true`; `--key=` → `""`; флаг перед флагом не съедает | «--flag без значения», «--key=», «флаг перед другим флагом» |
| M1-ARGS-04 | `--module` повторяется и копится в `modules` (ключа `module` нет) | «--module повторяется…» |
| M1-ARGS-05 | Числа строками; значения с regex/слешами; продолжение после пары; повторное вхождение ключа (W1d) | «числовые значения…», «значение может содержать…», «разбор продолжается…», «повтор обычного ключа…» |

### M1-CSV — `src/core/csv.mjs`

| ID | Микрозадача | Кейсы |
| --- | --- | --- |
| M1-CSV-01 | `safeName`: буквы/цифры/`.`/`-`/`_` сохраняются; прочее → `_`; кириллица → `_` | 3 кейса `safeName` (+W1d real-имена) |
| M1-CSV-02 | `csv`: все ячейки в кавычках, числа → строки, удвоение кавычек | 2 кейса `csv` (+W1d заголовок инвентаря) |
| M1-CSV-03 | `csv`: запятые и переносы строк не ломают ячейку; пустая таблица → `""` | 2 кейса `csv` |

### M1-PATHS — `src/core/paths.mjs`

| ID | Микрозадача | Кейсы |
| --- | --- | --- |
| M1-PATHS-01 | `projectRoot` + `logsDir`/`functionsDir`/`screensDir` внутри корня | 2 кейса |
| M1-PATHS-02 | `screenshotName`: нумерация с ведущим нулём, кириллица сохраняется, 3-значный номер | 2 кейса (+W1d real-имена) |
| M1-PATHS-03 | `screenshotName`: пунктуация → один дефис, обрезка slug до 40, пустая подпись → `item` | 3 кейса |
| M1-PATHS-04 | `relativeToRoot`: внутри корня → относительный; вне → как есть | 2 кейса |

### M1-PROC — `src/core/processes.mjs`

| ID | Микрозадача | Кейсы |
| --- | --- | --- |
| M1-PROC-01 | `isDiscordProcess`: регистронезависимо; `vesktop` исключён; чужие процессы — `false` | 3 кейса (+W1d варианты имён) |
| M1-PROC-02 | `filterDiscordProcesses`: сохраняет порядок; пустой список → пустой | 2 кейса |

### M1-INV — `src/core/inventory.mjs`

| ID | Микрозадача | Кейсы |
| --- | --- | --- |
| M1-INV-01 | `inventoryCounts`: суммарные символы/экспорты; пусто → нули | 2 кейса (+W1d фрагмент реального JSON) |
| M1-INV-02 | `inventoryCsvRows`: заголовок; символы `kind=symbol/type=function`; экспорты; `safeName`; без модулей | 3 кейса (+W1d jq-оракул) |
| M1-INV-03 | `inventoryJson`: `{pid, process, at, modules}`, имя процесса как есть | 1 кейс (+W1d round-trip реального JSON) |

### M1-PROFILE — `src/core/ui/profile.mjs`

| ID | Микрозадача | Кейсы |
| --- | --- | --- |
| M1-PROFILE-01 | `topFunctions`: сортировка по `hits` ↓; `url` — последний сегмент; стабильность при равенстве | 2 кейса |
| M1-PROFILE-02 | Отброс узлов без `hitCount` и шума `(idle)/(program)/(garbage collector)/(root)`/пустых | 2 кейса |
| M1-PROFILE-03 | Обрезка `url` до 60; пустой `url` допустим; `limit` (по умолчанию 8) | 2 кейса (+W1b golden top-8 реальных журналов) |

### M1-BLACK — `src/core/ui/blacklist.mjs`

| ID | Микрозадача | Кейсы |
| --- | --- | --- |
| M1-BLACK-01 | Матрица блокировок: выход/покидание; удаление/блокировка; mute/deafen/скрытие; сброс/очистка/забыть/отписка/статус | 4 кейса |
| M1-BLACK-02 | Обычные пункты настроек и меню не блокируются; подстрока срабатывает не только в начале (W1-fix) | 2 кейса (+W1b реальные подписи журналов) |

### M1-WALK — `src/core/ui/walk-log.mjs`

| ID | Микрозадача | Кейсы |
| --- | --- | --- |
| M1-WALK-01 | `parseVisited`: labels+maxStep; битые строки пропускаются; пусто; запись без `label` игнорируется | 3 кейса (+W1b golden 56/52) |
| M1-WALK-02 | `buildWalkRecord`: значения по умолчанию | 1 кейс (+W1b кириллица) |
| M1-WALK-03 | `consoleIssues`: `exceptionThrown` + `consoleAPICalled type=error`; ≤3 текста; обрезка 220 | 3 кейса |
| M1-WALK-04 | `shouldBail`: `/таймаут|закрыто/`; обычная ошибка — нет; `null` → false | 2 кейса (+W1-fix явный ноль error-записей) |
| M1-WALK-05 | `pickNextItem`: посещённые/запрещённые; порядок; всё пройдено → `undefined`; свой blacklist | 3 кейса (+W1b реальный журнал) |

### M1-CDP — `src/core/cdp/client.mjs`

| ID | Микрозадача | Кейсы |
| --- | --- | --- |
| M1-CDP-01 | `resolveEndpoint`/`resolveTargetHint`: значения по умолчанию и env-переопределение (env инжектится) | 3 кейса |
| M1-CDP-02 | `listTargets`: `<endpoint>/json/list`; ошибка HTTP → `CDP list failed: <status>` | 2 кейса |
| M1-CDP-03 | `pickTarget`: по hint; «discord» в url; пропуск целей без `webSocketDebuggerUrl`; fallback; `undefined` | 4 кейса |
| M1-CDP-04 | `createCdpClient`: отправка `{id, method, params}` → result; ответ с ошибкой → reject с методом | 2 кейса |
| M1-CDP-05 | События копятся в `events`; чужие `id` игнорируются; битый JSON не роняет | 2 кейса |
| M1-CDP-06 | Таймаут ответа; `close()` отклоняет незавершённые; ошибка соединения до open | 3 кейса |
| M1-CDP-07 | `evaluate`: `returnByValue/awaitPromise` → `value`; исключение → `Error` с `description`, fallback `text` | 3 кейса |

### M1-CALLS — `src/core/calls.mjs`

| ID | Микрозадача | Кейсы |
| --- | --- | --- |
| M1-CALLS-01 | `isCallPayload` (`type === "call"`) и `callLine` (JSON+`\n`; не-вызов → `null`) | 3 кейса |
| M1-CALLS-02 | `parseCallLog`: фикстура; пустые/битые строки и не-вызовы пропускаются | 1 кейс (+W1a golden) |
| M1-CALLS-03 | `summarizeCallLines`: группировка `(module,name,address)`, сортировка, экранирование как в shell | 1 кейс (+W1a golden CSV) |
| M1-CALLS-04 | `summarizeCallLines`: удвоение кавычек без внешних кавычек; пустой список → только заголовок | 2 кейса (+W1a стресс 50k/5k) |

### M1-NM — `src/core/report/nm.mjs`

| ID | Микрозадача | Кейсы |
| --- | --- | --- |
| M1-NM-01 | `parseNmPosix`: только `T/t/W/w`; адрес и size; короткие строки пропускаются; пусто | 2 кейса (+W1c golden) |
| M1-NM-02 | `staticFunctionsCsv`: `name,type,address,size` без финального `\n` | 1 кейс (+W1c awk-оракул) |

### M1-DOMAINS — `src/core/report/domains.mjs`

| ID | Микрозадача | Кейсы |
| --- | --- | --- |
| M1-DOMAINS-01 | `classifyName`: правила-подстроки; мультидомены и фиксированный порядок; «чистое» имя | 5 кейсов (+W1c golden по таблицам доменов) |
| M1-DOMAINS-02 | `domainCsv`: заголовок; `demangled` в кавычках с удвоением; пусто | 2 кейса |
| M1-DOMAINS-03 | `formatDomainSummary`: `%-12s %8d`, строки через `\n` | 1 кейс (+W1c REPORT.md-числа) |

### M1-CXXFILT — `src/adapters/cxxfilt.mjs`

| ID | Микрозадача | Кейсы |
| --- | --- | --- |
| M1-CXXFILT-01 | `demangle`: порядок сохраняется; неманагленные — как есть | 1 smoke-кейс (+W1c реальные имена) |
| M1-CXXFILT-02 | Пустой список → пустой результат без запуска процесса | 1 smoke-кейс |

### M1-CLI — `src/cli/run.mjs` + `bin/discord-lab.mjs`

| ID | Микрозадача | Кейсы |
| --- | --- | --- |
| M1-CLI-01 | `--help` и пустой argv: справка, код 0, список команд | 2 smoke-кейса |
| M1-CLI-02 | `--version`: версия из `package.json` | 1 smoke-кейс |
| M1-CLI-03 | Неизвестная команда → код 1 + сообщение в stderr | 1 smoke-кейс |
| M1-CLI-04 | Недоступная внешняя среда → код 2 (CDP; далее Frida) | 1 smoke-кейс |
| M1-CLI-05 | Маршрутизация команд §6 (pids/inventory/cdp/walk/summarize/domains/static) | покрывается M2–M4 |

### M1-SUM — команда `summarize` (поверх `calls`)

| ID | Микрозадача | Кейсы |
| --- | --- | --- |
| M1-SUM-01 | Пишет `<лог>.by-function.csv` и печатает сводку (паритет `summarize-calls.sh`) | smoke fs |
| M1-SUM-02 | Без аргументов и без `logs/calls-*.jsonl` → код 1, «Логов не найдено» | smoke fs |
| M1-SUM-03 | Полный реальный лог: CSV построчно равен оракулу (W1a) | smoke real |

## W4-fix — мини-волна робостности (P2/P3 пост-W4 аудита, §10)

Источник: `/tmp/opencode/review-w3/security-review-w4.md` (§4.1–4.8, §8, §10).
Решение владельца: W4-fix — делаем; §10 (backtrace) — делаем опционально (`--backtrace`).
Правила волны — `PROCESS.md` (ТЗ §4.1, отчёт §4.2, gate:worker). ТЗ: `/tmp/opencode/spec-w4fix-*.txt`.

| ID | Микрозадача | Файлы | Владелец | Статус |
| --- | --- | --- | --- | --- |
| W4f-01 | `captureMessages` (default false) — дренаж сообщений без накопления (P2-1) | `src/adapters/frida.mjs`, `tests/unit/frida.test.mjs` | W4f-a | [x] |
| W4f-02 | `src/core/hook-io.mjs`: openCallLog (стрим, счётчик ошибок, close-flush), waitForStop (SIGINT/SIGTERM в обеих ветках) (P2-2, P3-7) | `src/core/hook-io.mjs`, `tests/unit/hook-io.test.mjs`, §8 | W4f-b1 | [x] |
| W4f-03 | run.mjs hook: стрим-запись вместо appendFile, mkdir/open в try, счётчик ошибок в итоге (P2-2) | `src/cli/run.mjs`, smoke | W4f-b2 | [x] |
| W4f-04 | Числовая валидация `--pid`, `--limit`, `--max-symbols`, `--arg-count`, `--duration-ms` → код 1 до Frida (P2-3) | `src/core/args.mjs`, `src/cli/run.mjs`, tests | W4f-b2 | [x] |
| W4f-05 | packageVersion в try → код 2 при битом package.json (P3-6) | `src/cli/run.mjs`, smoke | W4f-b2 | [x] |
| W4f-06 | Валидация `--pattern` (regex + кап 512) → код 1 (P3-8) | `src/core/args.mjs`, `src/cli/run.mjs`, smoke | W4f-b2 | [x] |
| W4f-07 | Агент: `moduleEntry` в try (P2-4); сброс хуков при повторном `hook()` (P3-5) | `scripts/frida-log-calls.js`, `tests/unit/frida-agent.test.mjs` | W4f-c | [x] |
| W4f-08 | §10 backtrace: `--backtrace` → опция hook → `stack` (до 8 фреймов) в сообщениях | агент (W4f-c) + `src/cli/run.mjs` (W4f-b2) | W4f-c + W4f-b2 | [x] |
| W4f-09 | Контракт calls: запись со `stack` переживает callLine→parseCallLog | `tests/unit/calls.test.mjs` | W4f-c | [x] |
| W4f-10 | Финишёр: `tests/unit/frida-agent.test.mjs` по проверенному рецепту (7 правок) + типы `args.mjs` (7×TS2345 из зоны b2) | `tests/unit/frida-agent.test.mjs`, `src/core/args.mjs` | W4f-c-fin | [x] |
| W4f-11 | Branch coverage до порога: числовые входы `parseNumericArg`, `undefined`-паттерн, дефолтный `agentSource` (89.93%→91.88%) | `tests/unit/args.test.mjs`, `tests/unit/frida.test.mjs` | W4f-cov | [x] |
| W4f-12 | Живая находка: `startHook` нормализует RegExp→`.source` на границе RPC (frida RPC терял RegExp → `new RegExp({})` → `attach(0x0)`) | `src/adapters/frida.mjs`, `tests/unit/frida.test.mjs` | W4f-live | [x] |

Приёмка (координатор, 2026-10-18): gate:worker passed (typecheck; meta 5; unit 169; smoke 20);
gate:release passed_with_skips (4 e2e-скипа: 2×нет `CHROME_BIN`, 2×нет `DISCORD_LAB_LIVE_CDP` —
записаны в warnings); coverage 99.40/91.88/97.03; golden md5 192/192.
Live-чеклист (ptrace_scope=0 → возвращён 1): `inventory` в копии проекта (auto: 6 процессов;
ключи JSON/CSV = golden; 2 zygote пропущены `auxv-not-found`); `hook` 3 варианта — целей
7/установлено 7: 7732 вызова (8 с), 11582 (6 с, `--backtrace`: stack у 11582/11582), 11459 (SIGINT,
код 0); ошибок записи 0. Запись побайтово совпадает с легаси-логгером (`callLine` =
`JSON.stringify` + `\n`); `summarizeCallLines` читает живые логи.
Диспатчи (2026-10-18): a `ctx_0fa64c95c67c`, b1 `ctx_68b958b71c28`, b2 `ctx_4fe8244b2c19`,
c `ctx_3492203a6dd4` (закрыты вне оркестрации, релизнуты; прежние старты падали на
`agent-trust-workspace`); финишёр `ctx_bd636c98598f` (codex → INSUFFICIENT_BALANCE 403; остановлен)
→ перевыпуск `ctx_60eb6d71f1dc` (opencode, released); cov `ctx_d7dff3391098`, live `ctx_74fa2fb51624`
(released).
Инцидент golden (2026-10-18): красный кейс b2 (`inventory --pid abc` на pre-change CLI) создал
`functions/pid-NaN-pid-abc.{json,csv}`; воркер не тронул golden, эскалировал; координатор удалил
(копия — `/tmp/opencode/golden-incident-2026-10-18/`), манифест 192/192 цел; воркер изолирует кейс
временной копией. Бэклог: `gate:release` должен детектировать и ЛИШНИЕ файлы в golden-каталогах
(сейчас ловит только изменения по манифесту).
Инциденты инфраструктуры (2026-10-18): (1) 15:14Z — 8 агентских терминалов закрыты вне
оркестрации («Terminal closed by operator request»; в orchestration.db close-мутаций нет) —
работа продолжена с новыми терминалами; (2) codex-релей `codex.sale` — INSUFFICIENT_BALANCE 403;
все дальнейшие воркеры — на opencode; (3) живой чек-лист вскрыл RPC-баг `pattern` (RegExp не
сериализуется frida RPC → `new RegExp({})` = `/[object Object]/` → кандидаты с адресом `0x0` →
`attach(0x0)` падает → «установлено=0»): unit/smoke не ловили, исправлено W4f-12, подтверждено
живым прогоном (7/7, 11582 записи со stack).
(4) первый CI-прогон упал (17 с; 14 unit + 1 smoke — ENOENT `tests/fixtures/logs/*`):
паттерн `logs/` в `.gitignore` без якоря матчил и `tests/fixtures/logs/` — переносимые
фикстуры не попадали в коммит (локально были на диске). Исправлено: якорные
`/logs/`, `/functions/`, `/evidence/`; фикстуры в git; мета-стража
`tests/meta/fixtures-tracked.test.mjs` (red→green); прогон подтверждён в чистом клоне.

## M2–M6 — задачи волн (разворачиваются в микрозадачи при старте)

### M2 — Frida-слой (W4: воркеры; завершено 2026-10-18)

Ход: W4a (адаптер) → приёмка → W4b (CLI на терминале W4a); W3R-хардненинг — параллельно.

| ID | Микрозадача | Владелец | Статус |
| --- | --- | --- | --- |
| M2-FRIDA-01 | `src/adapters/frida.mjs` — `loadDevice` (ленивый динамический import "frida"), `loadAgentSource`, `listProcesses`, `attach` (паритет logger: message.connect до load), `collectInventory`, `parseHookMessage`, `startHook`, `stopHook`; unit `tests/unit/frida.test.mjs` с fake-device; строка §8 | W4a | [x] |
| M2-PIDS-01 | `pids [--all]` в `src/cli/run.mjs` (ленивый импорт адаптера; формат `<pid>\t<name>`; Frida недоступна → код 2 + «Frida недоступна»); существующий smoke skip-aware обязан позеленеть | W4b | [x] |
| M2-INV-01 | `inventory [--pid N] [--max-symbols N]` — форматы `functions/pid-*.json/.csv` через core/inventory + core/csv (паритет logger writeInventory); все цели упали → код 2; smoke `inventory --pid 99999999` → код 2 | W4b | [x] |
| M2-HOOK-01 | `hook [--pid\|--spawn] [--module]… [--kind] [--pattern] [--limit] [--arg-count] [--duration-ms] [--out]` — запись `calls-*.jsonl` через `callLine`; smoke `hook --pid 99999999` → код 2; живой прогон — в W4-fix (7/7 целей, записи с `--backtrace`; см. W4f-12) | W4b | [x] |

Приёмка (2026-10-18): живой `pids` — реальные Discord-процессы; `inventory`/`hook --pid 99999999`
→ код 2 (сообщения без заглушки); Frida 17.23.2 — device API асинхронный (`await getLocalDevice()`);
smoke-контракты усилены против текста заглушки. Всего 11/11 smoke M2-скоупа; подробности — в
«Итог волны W4» выше. Живой attach-прогон inventory/hook выполнен 2026-10-18 в live-чеклисте
W4-fix (ptrace_scope=0; там же найден и исправлен RPC-баг `pattern` — W4f-12).

Решение координатора (M2): команды CLI лениво импортируют `src/adapters/frida.mjs` (help/--version/
summarize не грузят нативный модуль); сообщения об ошибках среды — «Frida недоступна: <msg>» (код 2);
inventory при падении всех целей — код 2 (осознанное отклонение от logger.mjs, закреплено smoke-тестом).

### M3 — UI-walk (W5)

- M3-ENG-01: `src/core/ui/engine.mjs` — шаг: клик → профиль → скриншот → запись; интерфейс адаптера.
- M3-ADP-01/02: адаптеры `discord` и `fixture` (селекторы/подписи).
- M3-E2E-01: `walk-fixture.e2e` зелёный (клики, чёрный список, скриншоты, JSONL).
- M3-RUN-01: ручной прогон `walk settings` на живом Discord — паритет формата с `logs/ui-walk-settings.jsonl` (W1b golden).

### M4 — отчёты (W6)

- M4-SUM-01: `summarize` в CLI (закрывает M1-SUM, паритет CSV).
- M4-DOM-01: `domains` — полный пайплайн (nm → c++filt → classify → CSV/REPORT).
- M4-STAT-01: `static` — `functions/static/*.csv` (паритет `extract-static-functions.sh`).
- M4-GOLD-01: числа REPORT.md совпадают с пересчётом (115 150 функций; домены).

### M5 — жёсткость (W7)

- M5-COV-01: покрытие-гейты включены и зелёные.
- M5-CI-01: GitHub Actions (typecheck, unit/meta/smoke, e2e chromium, отчёт).
- M5-MUT-01: мутации Stryker на `src/core` (порог выживаемости ≤ 20%, опционально).
- M5-E2E-01: e2e ×5 повторов — стабильность.

### M6 — паритет и уборка (W8)

- M6-WRAP-01: `scripts/*` → тонкие обёртки или удаление (решение — владелец проекта).
- M6-TYPES-01: `scripts/cdp-types.d.ts` → `src/core/types.d.ts` (унификация).
- M6-DOC-01: README описывает только `discord-lab`; §6 закрыт полностью.

## Матрица непересекающихся файлов (волны W1 / W1-fix)

Все воркеры работают в общей папке `/home/sanya/orca/project` (один чек-аут;
копии `project-w1…w4` (эталонный базлайн W1) удалены после приёмки). Владение файлами на время волны:

| Воркер | Владеет | Запрещено |
| --- | --- | --- |
| W1a | `tests/unit/calls.test.mjs`, `tests/smoke/summarize.smoke.test.mjs`, `tests/fixtures/logs/calls-*` | всё остальное |
| W1b | `tests/unit/walk-log.test.mjs`, `tests/unit/profile.test.mjs`, `tests/unit/blacklist.test.mjs`, `tests/fixtures/logs/walk-*` | всё остальное |
| W1c | `tests/unit/nm.test.mjs`, `tests/unit/domains.test.mjs`, `tests/smoke/cxxfilt.smoke.test.mjs`, `tests/fixtures/nm/*`, `tests/fixtures/domains/*` | всё остальное |
| W1d | `tests/unit/args.test.mjs`, `tests/unit/csv.test.mjs`, `tests/unit/paths.test.mjs`, `tests/unit/inventory.test.mjs`, `tests/unit/processes.test.mjs`, `tests/fixtures/inventory/*` | всё остальное |

Никто из воркеров не трогает: `src/**`, `bin/**`, `scripts/**`, `package.json`,
`TDD-PLAN.md`, `TASKS.md`, `README.md`, чужие тестовые файлы. Новые `*.test.mjs`
не создаются — только правка назначенных (мета-тест требует строку в §8).

## Протокол воркера (кратко; полный — `PROCESS.md` §3.4 + `AGENTS.md`)

1. Работать только в общей папке и строго со своими файлами (множества не пересекаются).
2. Тесты — спецификация: ожидания из независимого оракула (jq/awk/c++filt), в
   комментарии — провенанс: артефакт, md5, команда. Фикстуры ≤ 64 КиБ.
3. Прогон: `node --test <свои файлы>` (red — падение до реализации), затем
   `npm run gate:worker` (typecheck + meta + unit + smoke) — зелёный. Никаких правок
   кода ради «позеленения».
4. Отчёт по шаблону `PROCESS.md` §4.2: файлы, кейсы, провенанс, команды, риски.
   Вопросы — через `ask`.
5. Git-коммиты делает координатор (уровень L1) — воркер не коммитит.
6. Финал — `worker_done` с `--outcome succeeded|failed` и кратким резюме.

## Чек-лист координатора (W2) — выполнен

- [x] Область правок: `find -newer` + diff против pristine-копий — только назначенные файлы (W1 и W1-fix).
- [x] Провенанс: md5 в комментариях совпадают с реальными артефактами (voice `f4e09451…`, walk `bf5b40e3…`/`de63e05e…`, nm `872c8da3…`/`bd7bd35a…`, domains, inventory; CSV-оракул `69a2ad56…`).
- [x] Независимая перепроверка golden своим оракулом (jq/awk/c++filt/nm): c++filt 7/7, CSV байт-в-байт, REPORT-счётчики, фикстуры byte-equal.
- [x] `npm run check` в основной копии: typecheck+meta зелёные; 153 теста (7 pass / 142 fail / 4 skip); падения только от заглушек (2 косвенных: CLI-smoke exit-код, CDP fake-socket).
- [x] Повторные прогоны (детерминизм) и отсутствие записей в рабочие каталоги.
- [x] Обновление §8/счётчиков/статусов: §8 не менялся (набор тестовых файлов тот же); статусы — этот файл.

## Backlog / открытые вопросы

- W1c-находка: в реальных `functions/static/*.nm.txt` нет символов типа `w`; реальная
  выборка покрывает T/t/W, синтетический кейс `w` остаётся в `tests/unit/nm.test.mjs`
  (комментарий в тесте) — при появлении таких файлов кейс начнёт матчиться данными.
- W3e-решение: `summarize` без аргумента при нескольких `calls-*.jsonl` в cwd берёт
  лексикографически первый файл — сверить с паритетом `summarize-calls.sh` на M4.
- `src/core/not-implemented.mjs` — удалён 2026-10-18 (Frida-слой им не воспользовался).
- Аудит пост-W4 (2026-10-18; отчёт: `/tmp/opencode/review-w3/security-review-w4.md`):
  P2 — (1) `frida.mjs attach()` копит `messages` без дренажа (на hook-пути — рост памяти);
  (2) hook пишет `appendFile` на каждый вызов (нет backpressure, ошибки записи глотаются — нужен
  write-stream + счётчик потерянных строк); (3) нет числовой валидации `--pid/--limit/--max-symbols/
  --arg-count/--duration-ms` (NaN доезжает до агента: `limit=NaN` → безлимитный Interceptor.attach);
  (4) агент `moduleEntry`: `Process.getModuleByName` вне try → опечатка в `--modules` роняет весь inventory.
  P3 — повторный `hook()` в агенте без сброса `activeHooks`; `packageVersion` без try (HARD-06);
  SIGINT не перехватывается при `--duration-ms`; `--pattern` компилируется в цели (валидировать в CLI).
  Проверено-не-воспроизводимо: stdout-EPIPE на Node 24 (прогон 1 МБ в `head` — код 0).
- `git init` — сделан 2026-10-18: ветка `master`, baseline-коммит; приватные каталоги
  (`logs/`, `functions/`, `evidence/`) и `.gate/`, `node_modules/` — в `.gitignore`.
  Ветки волн — `wave/<N>`, теги — `v<X.Y.Z>`; remote (GitHub) и push — по решению владельца.
- Перенос `/tmp/opencode/discord-ui-walkthrough/` → `tools/`?
- `logs/ui-clicks-*.jsonl` — формат событий кликов без потребителя: нужна ли команда?
- `screenshotName`: реальные имена в `evidence/screens` могут содержать `%`/`?`
  (проверить при W1d и зафиксировать поведение).
