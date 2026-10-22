# TASKS.md — микро-декомпозиция работ `discord-lab`

Обновлено: 2026-10-18 (волны W1–W4 закрыты и приняты: M1 и M2 сделаны — 176 тестов,
172 pass / 0 fail / 4 e2e-skip; покрытие 99.3/90.5/97.6; пост-W4 аудит — в backlog ниже).
Обновлено: 2026-10-19 (волны R7/RC: спека `docs/re/`, план Rust-клиента `docs/rust-client/`,
фикстуры `fixtures/`, каркас `rust/`; полный `npm run check` — 215 тестов, 211 pass / 0 fail /
4 e2e-skip; верификация сырья — 20/20).
Введён производственный процесс `PROCESS.md` (роли, гейты `gate:worker/release/qualify`,
шаблоны ТЗ/отчётов) и `AGENTS.md` — краткие правила агентам-воркерам. Контекст и
правила — `TDD-PLAN.md` (§8 — машиночитаемая таблица «тест ↔ модуль ↔ экспорты»;
мета-контроль — `tests/meta/plan-sync.test.mjs`).
Дополнено 2026-10-19: раздел «R1 — разведка gateway» (карточки; согласовано владельцем).
Итог 2026-10-19: волна R1 закрыта — статусы, каталог опкодов и артефакты в разделе «R1».
Дополнено 2026-10-19 (2): раздел «R4/R5/R6/D — перенос недостающих функций» по референсам
(Vesktop v1.6.7, discord-qt v0.5.1); полные описания и критерии — `docs/feature-backlog.md`.
Дополнено 2026-10-19 (3): раздел «R7 — глубокий RE: транспорт, медиа, бэкенд, UI, i18n»
(закрыт; нормативные доки — `docs/re/`). Волна R7 закрыла карточки D-01–D-03.
Дополнено 2026-10-19 (4): раздел «RC — Rust-клиент: концепт, архитектура, план, тесты»
(документы готовы; пакет — `docs/rust-client/`).
Дополнено 2026-10-20: batch-конвейер (3–6 карточек на волну, один gate, review после gate) —
`docs/rust-client/TEAM.md` и `docs/rust-client/WAVE-PLAN.md`; RW-18 (P2-09) принят,
RW-19/RW-20 (P2-10/P2-11) — активная пара B01.
Дополнено 2026-10-21: B01 принят (P2-10/P2-11); B02/RW-21 принят (P2-08, P1-01,
P1-02, P0-04, P0-07); B03/RW-22 принят (P3-15, P4-01, P7-04, P8-03); B04/RW-23 принят
(P4-02, P8-04); B05/RW-24 принят (P7-01, полный i18n).
Дополнено 2026-10-22: B06/RW-25 принят (P4-03 SRTP-крипта, P7-02 перф-харнесс,
P8-01 updater) — первая headless-волна, офлайн, без новых внешних зависимостей;
Дополнено 2026-10-22 (2): RW-26 принят (P4-07 RTCP/RTP-статистика, P4-11 регионы и
latency) и RW-27 принят (P7-01b ListFormat, P6-04 deep links, P6-03 автозапуск/бейдж) —
headless, без новых зависимостей; принято 39/90; следующий batch — **B07**
(GUI/keyring/SQLite) после решения владельца.

Дополнено 2026-10-22 (3): RW-28 принят (P7-01c RelativeTimeFormat — ICU-паритет ±2,
точные i64 >2^53) и RW-29 принят (P4-04 RTP-медиа по pcap-оракулу, P4-12 reconnect,
P7-01e `<t:…>`) — headless, без новых зависимостей; принято **43/90**; следующий batch —
**B07** (GUI/keyring/SQLite) после решения владельца.

Дополнено 2026-10-22 (4): RW-30 принят (P7-01d DurationFormat, P6-02 D-Bus-уведомления) —
headless, без новых зависимостей; принято **42/90** (хвосты P7-01b/c/d/e в 90 не входят,
исправлен прежний счётчик). Следующий batch — B07 (GUI/keyring/SQLite) после решения владельца;
headless-остатки — P6-06 (power/idle) и P2-03 (SQLite через FFI, см. `waves/RW-31/`).

Дополнено 2026-10-22 (5): RW-31 принят (P6-06 power/idle, P2-03 SQLite-store через FFI к
системному libsqlite3 — решение владельца A) — headless; принято **44/90**, осталось 46.
Следующий batch — B07 (GUI/keyring) после решения владельца; SQLite-часть B07 закрыта.

Дополнено 2026-10-22 (6): RW-32 принят (P6-07 spellcheck через FFI libhunspell, P6-02b hints
a{sv}) — headless; принято **45/90**, осталось 45. Следующий крупный headless-блок —
P1-10 keyring через FFI libsecret (libsecret 0.21 + ksecretd + secret-tool доступны).

Дополнено 2026-10-22 (7): RW-33 принят (P1-10 keyring через FFI libsecret) — headless;
принято **46/90**, P1-фаза закрыта, осталось 44. B07 заблокирован только GUI-крейтами
(keyring и SQLite закрыты); нужен выбор GUI-тулкита/окно live-smoke.

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

R1 (разведка gateway: WS/ETF/zstd, опкоды, голос) ──► закрыт 2026-10-19 (артефакты /tmp/opencode/r2-net/)

R2-NET/R3 (медиа/кэш-ассеты/UI-трассы/крипто) ──► закрыты 2026-10-19 (артефакты /tmp/opencode/)
R7 (глубокий RE: шлюз, голос, бэкенд/IPC, REST, UI, i18n) ──► закрыт 2026-10-19 (доки — docs/re/)
RC (Rust-клиент: концепт, функции, архитектура, план P0–P8, тест-план) ──► документы готовы 2026-10-19 (docs/rust-client/)
D (добор данных: READY, REST-методы, UI-трассы II — D-01…D-03 закрыты R7) ⊕ R4 (оболочка ОС, P0) ──► R5 (стриминг/аудио, P1) ──► R6 (платформа, P2)
бэклог переноса: docs/feature-backlog.md (41 функция; карточки ниже)
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

## R1 — разведка gateway (карточки; согласовано 2026-10-19)

Цель: эмпирически снять и задокументировать протокол шлюза событий (`gateway.discord.gg`,
WS/TCP/443) и REST-поверхность, работая только с изолированной копией (CDP 9223); живой клиент
(PID 220743) не трогается. Входные находки: в веб-бандле найден адрес
`wss://gateway.discord.gg/?encoding=etf&v=9&compress=zstd-stream` (ETF + потоковый zstd);
`evidence/network/endpoints.md` (хосты, fake-IP); `/tmp/opencode/ux-research/RESEARCH-REPORT.md`
(индекс 14 379 модулей); `/tmp/opencode/voice-trace-rtp/REPORT.md` (голосовой RTP/RTCP — рядом).
Артефакты волны — только в `/tmp/opencode/` (фактически `/tmp/opencode/r2-net/`; каталог `r1-gateway/`
не создавался); `logs/`, `functions/`, `evidence/` не пишутся.

| ID | Микрозадача | Владелец | Статус |
| --- | --- | --- | --- |
| R1-GW-01 | Захват WS-фреймов gateway на копии: CDP `Network.enable` + `webSocketFrameSent/Received` → `frames-*.jsonl` (dir, opcode, payload base64, ts); фиксировать URL handshake и первый фрейм (ожидается HELLO) | C | [x] |
| R1-GW-02 | Декодер `zstd-stream`: потоковый контекст на соединение (Node ≥ 24 `node:zlib` zstd или системный `zstd` — выбрать на шаге 01); тест — декод первого фрейма до валидного ETF | C | [x] |
| R1-GW-03 | Декодер ETF (подмножество тегов: int/large-int, atoms, binary, list, map, nil) + roundtrip-тест на живых фреймах; таблица тегов в отчёте | C | [x] |
| R1-GW-04 | Каталог opcodes и dispatch-событий из живого дампа: HELLO(10) → IDENTIFY(2) → READY; HEARTBEAT(1)/ACK(11); диспатчи (MESSAGE_CREATE, GUILD_CREATE, …) — таблица «событие → ETF-ключи» | C | [x] |
| R1-GW-05 | REST-инвентарь без сети: `api/v10`-эндпоинты из модулей веб-бандла (`/tmp/opencode/ux-research/scripts/export-1`, grep) → список с ролями | C | [x] |
| R1-GW-06 | Голосовая сигнализация: VOICE_STATE_UPDATE/VOICE_SERVER_UPDATE → voice-WS (handshake, heartbeat, op-коды); связать с `/tmp/opencode/voice-trace-rtp/REPORT.md`. **Закрыт волной R7**: полный голосовой WS и каденция — `docs/re/voice-deep.md` §1 | C | [x] |
| R1-GW-07 | Сводный отчёт `R1-GATEWAY.md` (метод, структуры, ограничения) + правка §5 `GAP-ANALYSIS.md`; решение «идём ли в протокол» — за владельцем | C | [x] |

Ограничения: интерактив — чтение (CDP eval по согласованным сценариям); gdb — только по правилам
волны (SIGINT → detach, без SIGKILL); любые сетевые шаги (повторное подключение, replay) — отдельное
согласие; IDENTIFY-пейлоады содержат токен сессии — в отчётах и артефактах маскировать (только
структуры и ключи, без значений авторизации).

**Итог R1 (2026-10-19): волна закрыта.** Фактический каталог артефактов — `/tmp/opencode/r2-net/`
(`r1-gateway/` не создавался); статусы — в таблице выше.

- Шлюз: точечный форс-реконнект только основного сокета (`ss -K -t dst 198.18.0.109`; голосовой
  сокет не тронут) → чистый поток с первого кадра (`28 b5 2f fd`), 70 recv-термов + 11 sent
  (RESUME прошёл → RESUMED; свежий IDENTIFY/READY не снимался — сессия резюмировалась).
- Каталог опкодов: recv — 10 HELLO / 11 ACK; sent — 6 RESUME, 3 PRESENCE (invisible), 4 VOICE_STATE,
  37 SUBSCRIPTIONS (диапазоны `[[0,99]]`), 40 QoS (`ver:31`), 43 REQUEST_CHANNEL_INFO; диспатчи —
  RESUMED, SESSIONS_REPLACE, CHANNEL_INFO, GUILD_MEMBER_LIST_UPDATE, PRESENCE_UPDATE,
  VOICE_STATE_UPDATE, MESSAGE_POLL_VOTE_ADD/REMOVE, USER_SETTINGS_PROTO_UPDATE.
- ETF: 16 тегов, таблица — `REPORT.md` §11; особенности: nil — строка «nil», булевы — «true»/«false»,
  снежинки — строки с суффиксом `n`.
- REST: 231 URL / 88 эндпоинтов `api/v9` из кэша — `rest-endpoints.md` (в карточке ожидался v10 —
  отклонение зафиксировано).
- Голос: формат шифрования снят RE (`PacketCrypter`, режимы 0–6; AAD = заголовок; nonce — 4-байтовый
  LE-счётчик в хвосте) — `VOICE-CRYPTO.md`; сигнализация — частично (R1-GW-06: VOICE_STATE_UPDATE и
  каденция/роли сокетов подтверждены, полный дамп голосового WS — нет).
- Дополнительно: кэш-ассеты (4640 записей: 15 mp3, 22 woff2, 137 svg, 206 png, 429 webp, 13 видео),
  UI-трассы (DM/эмодзи/поповер), оценка готовности к Rust-клону — `READINESS.md` (A: GO, C: NO-GO).
- Инструменты волны: `ws-capture.mjs`, `gw-zstd.mjs`, `cache-entries.mjs`, `ui-traces.mjs`,
  `map-project.py` (все — в `/tmp/opencode/r2-net/`).

## R7 — глубокий RE: транспорт, медиа, бэкенд, UI, i18n (закрыт 2026-10-19)

Цель: закрыть слабые места таблицы покрытия (транспорт/шлюз, голос/крипто, REST, UI/i18n) и
впервые разобрать бэкенд (`main`-процесс, IPC-мост, нативные модули), не трогая живой клиент
(изолированная копия, CDP 9223). Нормативные документы — `docs/re/` (индекс — `docs/re/README.md`,
графы процессов/вызовов/функций — `docs/re/architecture.md`); сырые доказательства (pcap, strace,
декоды, скриншоты, санитизированный capture; всего 1,7 ГБ) — лаборатория `/home/sanya/re-deep/`.
Секреты вырезаны санитайзером, публичные IP — `<PUB_IP>`, исходящие WS-кадры — длина + sha256;
golden-каталоги (`logs/`, `functions/`, `evidence/`) не трогались.

| ID | Направление | Результат | Артефакт |
| --- | --- | --- | --- |
| R7-01 | Шлюз: ETF/zstd/опкоды/READY/GUILD_CREATE | **D-01 закрыт**: 4 полных READY (40 ключей, 7 гильдий, 17 ЛС, 146 users, 157 read_state); 11 фактических ETF-тегов; исправлен big-int (знак до цифр); op7 → RESUME → RESUMED без разрывов `s` | `docs/re/gateway-protocol.md`, `gateway-schema.md` |
| R7-02 | REST: маршруты, тела, auth | **D-02 закрыт**: 47 маршрутов, 133 запроса v9; auth — 221 запрос / 6 заголовков, супер-свойства 22 поля; независимая сверка — exit 0 | `docs/re/rest-d02.md` (+`.json`, `.stats.json`, `.crossref.json`, `rest-d02-verify.sh`) |
| R7-03 | UI-трассы II и контракты компонент | **D-03 закрыт**: поиск (+7 секций фильтров), эмодзи-пикер, участники, настройки, поповер; 12 контрактов | `docs/re/frontend-d03.md` |
| R7-04 | Голос: сигнализация и UDP | ops 8/0/1/2/4/15/16/3/6; байтовые структуры discovery/keepalive/RTP/SRTCP; ротация медиа-сокета +60,08 с (подтверждена strace); DTX и comfort-noise | `docs/re/voice-deep.md` |
| R7-05 | main-процесс и IPC-цепочки | Bootstrap, окна/`webPreferences`, апдейтер; 5 полных цепочек `DISCORD_*`; 121/121 цитат по смещениям | `docs/re/main-arch.md` |
| R7-06 | Карта IPC и мост `DiscordNative` | 168 каналов (152 main / 129 renderer / 115 обе стороны), whitelist 43, мост 35 записей, 47 call-sites | `docs/re/backend-ipc.md` |
| R7-07 | Нативные модули | 9 модулей (`discord_voice.node` 126,1 МБ, `discord_krisp.node` 62,1 МБ), NAPI 94/94, Frida-инвентарь 110 имён, dlopen-список | `docs/re/native-re.md`, `native-modules.md` |
| R7-08 | i18n | Собственный рантайм; ключ = xxh64 LE → base64[0..6] (36 бит); плурализация ru + `=N`; 23 правила разметки; 31/36 локалей; 23 836 ключей | `docs/re/i18n-deep.md`, `i18n-stats.json` |
| R7-09 | Методы и санитизация | strace → полные байты UDP; CDP-драйверы с доверенными `Input.*`; snap-pcap (2,6 МБ); санитайзер JSONL (утечек нет) | `docs/re/README.md` §4, лаборатория |

Ограничения/unknowns: первые 43-байтные пакеты talkspurt; тела RTCP шифрованы (потери/джиттер
снаружи недоступны); закрытие старых fd при ротации в strace не видно; вложенный E2EE DAVE/MLS не
разбирался; `VOICE-CRYPTO.md` прошлой волны потерян вместе с `/tmp` — крипто-поля восстановлены
наблюдательно (`docs/re/voice-deep.md` §2.3–2.4).

Продолжение (по запросу): пересоздать `VOICE-CRYPTO.md`; DAVE/MLS-разбор; расшифровка talkspurt;
GUILD_CREATE через приглашение; spec-pack `messenger-client`.

## RC — Rust-клиент: концепт, архитектура, план, тесты (RC-01…RC-08, 2026-10-19)

Цель: подготовить вход для внешнего Rust-проекта — **полноценного клиента Discord (нативный
бэкенд, оболочка Tauri; без Electron и встроенного Chromium)**. Пакет — `docs/rust-client/` (индекс —
`docs/rust-client/README.md`); опирается на R7 (`docs/re/`) и карточки R4/R5/R6/D
(`docs/feature-backlog.md`).

| ID | Направление | Результат | Артефакт |
| --- | --- | --- | --- |
| RC-01 | Концепт | Цель и рамки, 10 групп возможностей, стек (tokio/tungstenite/zstd/SQLite/keyring…), сценарии, критерии MVP/полного, риски | `docs/rust-client/CONCEPT.md` |
| RC-02 | Каталог функций | 14 групп (GW/ENC/AUTH/API/DATA/MSG/SRCH/PRES/VOICE/MEDIA/UI/I18N/SYS/SEC): что делает, зачем, как работает по RE, источник, фаза | `docs/rust-client/FEATURES.md` |
| RC-03 | Архитектура и графы | Процессы (основной + медиа), 10 крейтов, шина/редьюсер, конвейер шлюза, стек голоса, SQLite-схема, замена 168 IPC-каналов, ADR-1 (Tauri; запас — iced/Slint), бюджеты | `docs/rust-client/ARCHITECTURE.md` (8 графов Mermaid) |
| RC-04 | План | Волны P0–P8: 90 карточек с DoD, тестами, зависимостями и размерами; гейты, риск-план, соответствие R4/R5/R6/D | `docs/rust-client/PLAN.md` |
| RC-05 | Тесты | Уровни L1–L5; манифест RE-фикстур; каталог ~130 тестов (T-ENC/GW/REST/DATA/MSG/SRCH/VOICE/MEDIA/UI/I18N/SYS/SEC/HARN); протокол red-first | `docs/rust-client/TEST-PLAN.md` |
| RC-06 | Мета-контроль | Синхронность документов плана: файлы на месте, графы есть, ID функций/тестов уникальны, ссылки из README/TASKS/map живы | `tests/meta/rust-client-sync.test.mjs` |
| RC-07 | Единая структура | Объединение RE и Rust-клиента: курированные фикстуры `fixtures/` (12 записей, 1,59 МБ, `MANIFEST.json` с md5), cargo-воркспейс `rust/` (10 крейтов, офлайн `cargo test` — на 2026-10-20 **337** тестов, ядро **295**), строение и трассировка (141 F → спека → фикстуры → 133 T) | `fixtures/`, `rust/`, `docs/rust-client/PROJECT-STRUCTURE.md`, `TRACEABILITY.md` |
| RC-08 | Верификация данных | 20 автосверок сырьё↔доки: md5 снапшотов D-02, READY-числа (7/17/146/60/157), кадры gw-proto (1544/1340/21), 11 ETF-тегов, REST (47/133/221), i18n (23 836/31), IPC (168/43/35), модули (9), RTC-опы, санитизация (вырезан реальный `analytics_token` из декода READY), целостность фикстур — все 20/20 | `docs/rust-client/verify/verify-re.mjs`, `verify/report.json`, `VERIFICATION.md` |

Фикстуры конформанс-тестов — лаборатория `/home/sanya/re-deep/` (манифест — `TEST-PLAN.md` §2;
курированный срез в git — `fixtures/`, 12 записей с md5); секреты вырезаны. Реализация (код на
Rust) — по `docs/rust-client/PROJECT-STRUCTURE.md`: каркас уже в репозитории (`rust/`, 10 крейтов),
развитие — волнами P1–P8. Верификация данных RC-08 прошла 20/20 (`VERIFICATION.md`).
Продолжение R7 «spec-pack `messenger-client`» закрыто пакетом RC-01…RC-08.

## R4/R5/R6/D — перенос недостающих функций (карточки; согласовано 2026-10-19)

Цель: перенести в наш продукт (сценарий A: свой мессенджер с UX по мотивам Discord) функции,
которых у нас нет по сравнению с внешними референсами. Полный инвентарь — `docs/feature-backlog.md`
(41 карточка: что делает / откуда / куда переносим / критерий готовности); анализы —
`docs/vesktop-analysis.md` и `docs/discord-qt-analysis.md`. Код референсов не копируется (GPL):
перенимаются идеи и контракты; venmic (MPL-2.0) и arrpc (MIT) — с соблюдением лицензий.

| Волна | Содержание | Карточек | Приоритет | Статус |
| --- | --- | --- | --- | --- |
| R4 | Оболочка ОС: окно, трей, бейджи непрочитанного, автозапуск, deep links, уведомления, апдейтер, упаковка, splash, спеллчек, настройки | 12 | P0 | [ ] |
| R5 | Стриминг и аудио: PipeWire-захват, фильтры узлов, пикер источника, качество, подмена media-API, события стрима, RPC игр | 11 | P1 | [ ] |
| R6 | Платформа и расширяемость: Steam Deck, D-Bus, Win/mac-специфика, debug-окна, reset/repair, доставка модулей, ассеты, буфер | 10 | P2 | [ ] |
| D | Данные и контракты: READY/GUILD_CREATE, REST-методы/тела, UI-трассы II, getUserMedia, stream-контракты, дефолты уведомлений, devtools/токен, приватность | 8 | P0 (кормит R4–R6) | [~] |

Статус D (2026-10-19): **D-01–D-03 закрыты волной R7** — артефакты `docs/re/gateway-protocol.md`,
`docs/re/rest-d02.md`, `docs/re/frontend-d03.md`; D-04–D-08 открыты (нужны веб-модули).

Правила: статусы карточек ведутся в `docs/feature-backlog.md` (ID `R4-01`…`D-08`); при старте
волны координатор разворачивает её в микрозадачи по образцу R1 (свои файлы, гейт, приёмка).
Порядок: D и R4 можно вести параллельно; R5 — после D-04/D-05 (контракты медиа/стрима);
R6 — после стабилизации R4–R5. Внешний Rust-проект (сценарий A) — потребитель волн. Синхронность
документов — `tests/meta/backlog-sync.test.mjs`.

## RW — реализация Rust-клиента (волны плана `docs/rust-client/PLAN.md`)

Старт 2026-10-19. Регламент — `PROCESS.md` §3–§4: координатор ведёт карточки/ТЗ/приёмку; воркер
(субагент) пишет тесты и код только в своих файлах; ревьюер (второй субагент, независимая
проверка) подтверждает red-first и оракулы. С 2026-10-20 действует batch-конвейер: 3–6 карточек,
общий integration-патч, targeted у воркеров, один gate, review после gate (`docs/rust-client/TEAM.md`,
`docs/rust-client/WAVE-PLAN.md`). Журнал волн (артефакты, red/green-логи, точка восстановления) —
`waves/` (`db.jsonl`, `STATE.md`, `RW-01`…`RW-18`; RW-01…RW-09 и журнал в git, RW-10…RW-18 — в
рабочем дереве до приёмки); рабочие черновики — `/tmp/opencode/rw-*` (вне git, tmpfs — исчезнут
при перезагрузке).

| ID | Карточка | План | DoD | Тесты | Владелец | Статус |
|---|---|---|---|---|---|---|
| RW-01 | ETF-кодек (прототип): `EtfValue`/`decode`/`encode`, fuzz-lite | P0-02 | 11 тегов + big-int «знак до цифр» + 5 реальных термов из `capture-run2.jsonl` (сверка длин с `gw-proto/msgs-*.jsonl`); round-trip байт-в-байт; ошибки/границы без паник | T-ENC-01…03 (+fuzz-lite) | воркер: субагент; приёмка: координатор | принято 2026-10-19 |
| RW-02 | zstd-stream декодер (прототип): `ZstdStream::push` на ruzstd 0.9.1 | P0-03 | кадр 0 с магией, кадры 2+ без (реальный сокет `95974.1389`: 137 кадров, 104 066 → 824 816 Б); непрерывный контекст без `end()`; удержание окна 64 КиБ задокументировано; ошибки `MissingMagic`/`Decode`/`TrailingData` без паник | T-ENC-04 | воркер: субагент; приёмка: координатор | принято 2026-10-19 |
| RW-03 | Сессия шлюза (прототип без транспорта): HELLO/heartbeat 41 250/ACK/RTT/zombie в `discord-core::gateway` | P1-03 | автомат на виртуальных часах: первый удар — `seed % 41250`, следующий — `ACK + 41250`; RTT по op11; зомби → `Reconnect`; `s`-последовательность; оракул — таймлайн `95974.1389` (23 ACK; дельты ≤ 41 263; симуляция: 11–12 ударов, без зомби) | T-GW-01…03 (без стаб-сервера) | воркер: субагент; приёмка: координатор | принято 2026-10-19 |
| RW-04 | Транспорт шлюза: WS-клиент (tokio-tungstenite + rustls/ring), пайп zstd→термы→сессия, стаб-сервер в тестах | P1-03 | connect (`ws://`/`wss://`), чтение кадров, heartbeat по эффектам сессии, таймеры/зомби, исходы `Reconnect`/`Closed`; T-GW-01/03 на локальном стабе (`127.0.0.1`); lab-реплей 137 кадров; новые зависимости разрешены владельцем 2026-10-19 | T-GW-01, T-GW-03 | воркер: субагент; приёмка: координатор | принято 2026-10-19 |
| RW-05 | Идентификация: IDENTIFY (op2) по структуре веб-клиента + READY-сводка; отправка IDENTIFY после HELLO | P1-04 | поля op2 `{token, capabilities, properties, presence, compress, client_state, qos_token}` (оракул — бандл веб-клиента `out/bundles/discord.com_assets_web.*.js`, факты `rest-static.md` §4); `capabilities` 1734653/1767421; READY — сводка по run2-оракулу (40 ключей, 7/17/146/60, read_state 157/128949, user_guild_settings 58/734); T-GW-02 на стабе, T-GW-04 лаб-тест | T-GW-02, T-GW-04 | воркер: субагент; приёмка: координатор | принято 2026-10-19 (независимое ревью; ниты закрыты) |
| RW-06 | Resume/реконнект: op6 `{token,session_id,seq}`, политика resume (3 с/30 с), флаг op9 `d`, непрерывный `s` | P1-05 | `resume.rs` (`build_resume_frame`, `ResumeState`, `plan_reconnect`, порог 30 000 мс); `ws.rs` (`connect_and_run_resume`, op6 после HELLO, `resume_state()`, `last_invalid_session_resumable()`); T-GW-05 швы 663→664 и 331→332, T-GW-06 стаб-сценарий «op7 → RESUME → RESUMED без потерь», T-GW-07 политика разрыва | T-GW-05…07 | воркер: субагент; приёмка: координатор | принято 2026-10-19 (независимое ревью; дыры R4/R10/R11 закрыты) |
| RW-07 | Роутер диспатчей: op0 → типизированные события 21 типа; неизвестные → raw (`kind: None` + счётчик); payload без разбора | P1-06 | `dispatch.rs` (`DispatchKind` 21 тип, `DispatchEvent`, `route_dispatch`); `ws.rs` (`events`/`take_events`/`unknown_dispatches` в пайпе); оракулы: union `msgs-*.jsonl` — 21 тип/1340 событий; сокет `.1389` — 44 терма/36 op0 (окно ruzstd, 704 Б хвоста) | T-GW-08 | воркер: субагент; приёмка: координатор | принято 2026-10-19 (независимое ревью; ниты N1–N3 закрыты red-first, N4 — в backlog) |
| RW-08 | REST-клиент: каталог 47 маршрутов, 6 auth-заголовков, коды, ретраи/таймауты (3 параллельных воркера + рефакторинг W8d + ревью W8r) | P1-07 | `routes.rs` — 47 маршрутов/`find`/`render_path` (T-REST-01); `headers.rs` — 6 заголовков + классы кодов (T-REST-02); `retry.rs` — таймауты/ретраи идемпотентных, POST/PATCH без ретраев, локальный стаб (T-REST-03); без новых зависимостей; реальный TLS-транспорт — открытый вопрос | T-REST-01…03 | воркеры: субагенты W8a–W8c; рефакторинг: W8d; ревью: W8r | принято 2026-10-20 (ревью W8r: блокеров нет, 14/14 мутаций; нит покрытия backoff закрыт red-first — кейс N6) |
| RW-09 | X-Super-Properties: генератор 22 полей (base64 JSON), конфигурируемо, политика «честные значения» (3 параллельных воркера + рефакторинг W9d + ревью W9r) | P1-08 | W9a `base64.rs` — RFC 4648 вручную; W9b `super_properties.rs` — JSON-запись/разбор 22 полей и base64 (T-REST-04); W9c `identity.rs` — честные значения из системы (ОС/ядро/локаль/дистрибутив/дисплей/оконный менеджер), свои поля идентичности, UUID v4 вручную; оракул — живой заголовок `95974.2323` (снимок D-02) + §6.2 `rest-d02.md`; без новых зависимостей | T-REST-04 | воркеры: субагенты W9a–W9c; рефакторинг: W9d; ревью: W9r | принято 2026-10-20 (ревью W9r: блокеров нет, 20/21 мутаций; ниты N1/N6 закрыты red-first, N4/N5 исправлены, N2 аннотирован, N3 — в backlog) |
| RW-10 | HTTP/1.1-транспорт REST (вариант A: tokio + tokio-rustls, ручной фрейминг) — 2 параллельных воркера W10a/W10b | P1-07 (инфраструктура) | `http1.rs` — запрос/ответ/CL/chunked/лимиты/инъекции; `transport.rs` — Endpoint, TCP/TLS, SAN, таймауты, чтение по CL/chunked/until-close; фикстуры CA→leaf; зависимости `tokio-rustls 0.26`, `httparse 1` (офлайн) | 30 (16+14) | воркеры: W10a/W10b; ревью: отдельный агент | ревью 2026-10-20: «принято с нитами» (повторное ревью — «принято с нитами», P0/P1/P2 нет); P1/P2-ниты закрыты red/green (`tests` 16+14; полный red/green `nits-red-final`/`nits-green-final`), P3 N1–N9 — очередь (`waves/reviews/review-rw-10-11-round2.md`); принято 2026-10-20 |
| RW-11 | Rate-limit/ошибки, телеметрия-off, ack/read-state — 3 параллельных воркера W11a/W11b/W11c | P1-09, P1-11, P1-12 | `rate_limit.rs` — retry-after/JSON-поля/429/5xx/401/бэкофф; `telemetry.rs` — блок science/beaker/track; `ack.rs` — target/body с токеном | 23 (10+4+9) | воркеры: W11a–W11c; ревью: отдельный агент | ревью 2026-10-20: «принято с нитами» (повторное ревью — «принято с нитами», P0/P1/P2 нет); P3 (JSON-сканер, нормализация telemetry, ссылки, httparse, тесты Host/TE) — очередь; принято 2026-10-20 |
| RW-12 | P2-01 READY-парсер + I1 REST-фасад (независимые дорожки) | P2-01 / инфраструктура | I1 `client.rs` — auth+транспорт+429/5xx+телеметрия; P2-01 — A1: 40 top-level, guilds/channels/roles/emojis/stickers, private, users, relationships, read_state, UGS, sessions, merged_members, диагностика | I1 16; P2-01 14+13+11+7 | I1: координатор; P2-01: W12a/W12b/W12c + review | **Принято 2026-10-20.** I1 — независимое ревью «Принято» (`waves/reviews/review-i1.md`), P3-7 backlog. P2-01 — review round 1 «не принято» (P1/P2/P3), все P1/P2 закрыты red-first; round 3 «принято», P0/P1/P2=0 (`waves/reviews/review-p2-01-round3.md`). Red-final 0/7, green 7/7; core **233**, workspace **275**, fmt/clippy чисто; legacy `ready_data` удалён (P-4); гейт passed `.gate/worker-2026-10-20T09-56-10-288Z.json`; артефакты `waves/RW-12/reports/` |
| RW-13 | P2-02 state/reducer/selectors (3 параллельных воркера W13a/W13b/W13c) | P2-02 | `state/types.rs` — State/Event/парсеры, диагностика, redaction; `state/reducer.rs` — `apply`/`apply_dispatch`/`apply_frame`, stale-политика, слияние, `SubscribeMembers`; `state/selectors.rs` — выборки; корневые re-exports | 42 (15+19+8) | W13a/W13b/W13c; review: отдельный агент | **Принято 2026-10-20.** Review round 1 — «не принято»: P1 stale malformed + 5 P2 (author upsert, private DM, supplemental index, root exports, property-test); все закрыты red-first, повторное ревью — «принято», P0/P1/P2=0 (`waves/reviews/review-p2-02.md`). Red фиксов: 4 runtime + compile-r19, exit 101; green 42; core **275**, workspace **317**, fmt/clippy чисто; gate passed `.gate/worker-2026-10-20T10-58-32-180Z.json`; артефакты `waves/RW-13/`. |
| RW-14 | P2-06 memory history/pagination (W14a/W14b/W14c) | P2-06 | `history/types.rs` — bounded window, Live > Page, trim Latest/Before/After/Around; `pagination.rs` — Cursor/PageRequest/MessagePage/PageToken, generation/stale, atomic ChannelMismatch, dedup; `selectors.rs` — recent/range/window_around и др. | 20 (7+5+8) | W14a/W14b/W14c; review: отдельный агент | **Принято 2026-10-20.** Review round 1 — P2-1 (root re-exports), P0/P1=0; закрыт red-first, P3-2/cap/expect исправлены; повторное ревью — «принято», P0/P1/P2=0 (`waves/reviews/review-p2-06.md`). Red: compile W14a/b + behavior W14c + root/counts, exit 101; green 20; core **295**, workspace **337**, fmt/clippy чисто; gate passed `.gate/worker-2026-10-20T11-45-30-914Z.json`; scope memory-only (REST response/JSON/SQLite/перезапуск вне волны); артефакты `waves/RW-14/`. |
| RW-15 | P2-04 member list: GATEWAY upsert/delta, op37-подписки, range/ranges, удаление | P2-04 | `state/member_list.rs` — типы и парсеры; `member_list_reducer.rs` — upsert/delta/range/ranges, stale/duplicate; `member_list_selectors.rs` — выборки; `Effect::SubscribeMembers`; без живого op37 | 40 (14+17+9) | W15a/W15b/W15c; review: отдельный агент | **Принято 2026-10-20.** Review round 1 — P2-1 (duplicate при INSERT в позицию существующего id), P0/P1=0; закрыт red-first, P3 закрыты; повторное ревью — «принято», P0/P1/P2=0 (`waves/reviews/review-p2-04.md`). Green targeted 40/40, core **335**, workspace **377**, fmt/clippy чисто; gate passed `.gate/worker-2026-10-20T12-43-25-346Z.json`; артефакты `waves/RW-15/`. |
| RW-16 | P2-05 memory-only chunks/догрузка: `GUILD_MEMBERS_CHUNK`, merge members, `not_found`, `Effect::RequestPresence` | P2-05 | `state/member_chunk.rs` — типы/парсер; `member_chunk_reducer.rs` — additive merge по guild, chunk_index/count, not_found; `member_chunk_selectors.rs` — диапазоны/прогресс; `presences[]` не разбирается до P2-09 | 43 (19+13+11) | W16a/W16b/W16c; review: отдельный агент | **Принято 2026-10-20.** Review round 1 — P2-1 duplicate-index, закрыт red-first; round 2 — «принято», P0/P1/P2=0 (`waves/reviews/review-p2-05.md`). Green targeted 43/43, core **378**, workspace **420**, fmt/clippy чисто; gate passed `.gate/worker-2026-10-20T13-21-59-680Z.json`; mutation 3/3; артефакты `waves/RW-16/`. |
| RW-17 | P2-07 memory-only read-state/бейджи: `MESSAGE_ACK`, mentions, badge, ack-point | P2-07 | `state/read_state.rs` — `MessageAck`/parse; `read_state_reducer.rs` — lazy upsert, non-channel/stale guard, manual/non-manual mentions, version watermark; `read_state_selectors.rs` — mentions/guild/private/badge/ack-point/is_message_read; без unread-count, REST ACK, SQLite/UI/сети | 40 (12+17+11) | W17a/W17b/W17c; review: отдельный агент | **Принято 2026-10-20.** Review round 1 — P2-1 (channel fallback в `last_acked_id` давал ложный read), P0/P1=0; закрыт red-first s11, s06 исправлен; round 2 — «принято», P0/P1/P2=0 (`waves/reviews/review-p2-07.md`). Green targeted 40/40, core **418**, workspace **460**, fmt/clippy чисто; gate passed `.gate/worker-2026-10-20T15-10-13-521Z.json`; mutation 7/7; артефакты `waves/RW-17/`. |
| RW-18 | P2-09 memory-only presence: `PRESENCE_UPDATE`, scopes, activities, member-list/chunk presences | P2-09 | `state/presence.rs` — типы/парсер; `presence_reducer.rs` — upsert/offline+hidden/supplemental/member-list/chunk; `presence_selectors.rs` — статусы/scopes/activities/online ids; без сети/UI/SQLite/исходящих запросов | 54 (18+18+18) | W18a/W18b/W18c; review: отдельный агент | **Принято 2026-10-21.** Round 1 — P0/P1=0, P2=2 + P3=14, все закрыты red-first; round 2 — «принято», P0/P1/P2=0, P3=8 backlog (`waves/reviews/review-p2-09-round2.md`); mutation 13/13, md5 15/15; gate passed `waves/RW-18/gate-worker.json`; core **473**, workspace **515**; артефакты `waves/RW-18/`. |
| RW-19 | P2-10 memory-only voice states: snapshot, `VOICE_STATE_UPDATE`, timers, channel status | P2-10 | `state/voice/{types,parser,reducer,selectors}.rs`; ключ `(guild,user)`, таймеры `(guild,channel)`, `channel_id=nil` — выход; без live/сети/исходящих voice-запросов | 18 (types 6, reducer 8, selectors 4) | CORE-1/W19; TL-2 I0/I1; review: отдельный агент | **Принято 2026-10-21.** Review round 1 — P2-1 (snapshot additivity/last-wins/remove), закрыт red-first v09b/v09c/v09d + мутанты clear-before-loop/first-wins; round 2 — P0/P1/P2=0, P3=11 backlog (`waves/reviews/review-p2-10-round2.md`); B01 mutations 18/18; gate passed; core 506, workspace 548. |
| RW-20 | P2-11 memory-only typing TTL: `TYPING_START`, 10 000 мс, сброс/удаление | P2-11 | `state/typing/{types,parser,reducer,selectors}.rs`; детерминированное время через `received_at_ms`; удаление по `MESSAGE_CREATE`/READY; без wire `TYPING_STOP`, unread-count, UI/сети | 15 (types 5, reducer 5, selectors 5) | CORE-2/W20; TL-2 I0/I1; review: отдельный агент | **Принято 2026-10-21.** Review — P0/P1/P2=0, P3=10 backlog (`waves/reviews/review-p2-11.md`); typing mutations 8/8; fixture `RW-20/tests-data/typing-run2.jsonl` (28 START); t01 — capture `idx 117`, `s=192`. |
| RW-21 | B02: P2-08 settings-proto + P1-01 ETF full + P1-02 zstd strict + P0-04 conformance + P0-07 legal | P2-08, P1-01, P1-02, P0-04, P0-07 | `settings/{mod,wire,preloaded,frecency}.rs`; checked ETF `encode_checked`/границы; zstd `dead`/`Poisoned`/`is_finished`; `discord-conformance` repo→lab loader; `LEGAL.md`; без state/REST/UI/сети/новых зависимостей | settings 18; codec 43; conformance 6; core 524; workspace 580 | CORE-3/W21, CORE-1/2, TEST-1, OPS-1; review: отдельные агенты | **Принято 2026-10-21.** P2-08 review round 2 P0/P1/P2=0, P3 (MAX_FIELDS) закрыт t18+M13; P1-01/P1-02 accepted with P3 (doc fixed); P0-04/P0-07 round 2 accepted with P3 (RE_REQUIRE_LAB=1) fixed. Мутации 11/11 codec + 13/13 settings; gate `waves/RW-21/gate-worker-b02.json`; артефакты `waves/RW-21/`. |
| RW-22 | B03: P3-15 i18n runtime + P4-01 voice signaling + P7-04 read-only API + P8-03 CI/release | P3-15, P4-01, P7-04, P8-03 | `discord-json`; `discord-i18n` hash/AST/plural/markdown; `discord-voice` signaling; `rest::resources`; `scripts/{ci,release}.sh` | JSON 11; i18n 8; voice 12; REST 6; CI 4; core 530; workspace 618 | CLIENT-1, VOICE-1, CORE-1, OPS-1; review A/B/C | **Принято 2026-10-21.** Review A/B/C: P1 fence не воспроизвёлся (regression добавлен), P2 plural/link/Debug/endpoint закрыты red-first; round 2 P0/P1/P2=0; мутации 18/18; gate `waves/RW-22/gate-worker-b03.json`. |
| RW-23 | B04: P4-02 UDP media transport + P8-04 reproducible SBOM/licenses | P4-02, P8-04 | `discord-voice/src/udp.rs` + `tests/udp.rs`; `scripts/sbom.mjs`; `ci.sh`/`release.sh`; `sbom_contract.rs` | UDP 15; SBOM 5; core 530; workspace 638 | VOICE-1/W24a, OPS-1/W24b; review A/B | **Принято 2026-10-21.** Discovery/keepalive/rotation/RTCP+SRTCP, fallible encode, peer/SSRC, rotate_if_due; SBOM 85 lock packages, strict SPDX, CI/release. Review A 4 P2 + B 2 P1/3 P2 закрыты red-first; мутации 21 caught + 1 inconclusive; gate `waves/RW-23/gate-worker-b04.log`; workspace 638. |
| RW-24 | B05: P7-01 полный i18n — locale registry/CLDR cardinal+ordinal, числа/валюты, даты/время, 23 markdown-правила | P7-01 | `discord-i18n/src/{locale,plural,format,format_data,markdown,lib}.rs`; `tests/{locale,plural,format,i18n,markdown}_contract.rs`; `tests/data/cldr-oracle.json`; генератор `waves/RW-24/gen-format-data.mjs` | i18n 33 (8 lib + 25 новых); core 530; workspace 663 | CLIENT-1/W25a, W25b; integration TL; review A/B | **Принято 2026-10-21.** 36 локалей/31 включена, CLDR cardinal/ordinal, exact `=N` до offset, числа/валюты, 4 стиля даты/времени UTC, `th` +543, localized names/timezone, skeleton, 23 markdown-правила с safe URL/bounded depth, fallback current→default→en-US. Review A/B: P0/P1/P2=0 после red-first; мутации 20/20 caught; gate `waves/RW-24/gate-worker-b05-final.log` / `.gate/worker-2026-10-21T08-58-08-129Z.json`; артефакты `waves/RW-24/`. |
| RW-25 | B06 (headless): P4-03 SRTP-крипта + P7-02 перф-харнесс + P8-01 updater | P4-03, P7-02, P8-01 | `discord-voice/src/crypto/{mod,aes,gcm,chacha,kdf}.rs` + `src/srtp.rs` + `tests/{crypto_vectors,srtp_contract}.rs`; новый крейт `discord-perf` (`src/workload.rs`, `tests/perf_contract.rs`); новый крейт `discord-updater` (`src/{manifest,plan}.rs`, `tests/updater_contract.rs`); без новых внешних зависимостей (кроме офлайн-кэша: `ring`, `sha2`, `serde_json`) | voice 65 (38 новых); perf 17; updater 34 + 1 doctest; core 531; workspace 785 | VOICE-1/W26a, OPS-1/W26b, OPS-1/W26c; интеграция TL-2 (I1: O(n²) presence); review: codex + TL-1 | **Принято 2026-10-22.** P4-03: AES-256/GCM, ChaCha20/XChaCha20-Poly1305, HKDF-SHA1 без зависимостей; S-box алгебраический (таблицы убраны — side-channel через кэш); раскладка `[clear][ciphertext][tag 16][counter 4 LE]` по capture (`waves/RW-25/research/RE-SRTP-LAYOUT.md`) и по функциям клиента (`PacketCrypter::HeaderLength`/`SpaceRequiredToEncrypt`, `RE-NATIVE-SRTP.md`); исходящий счётчик `0x80000000|n`, общий RTP+SRTCP; AAD = открытая часть в `*_rtpsize`; совместимость с wire Discord **не заявлена** (нет ключевого материала). P7-02: 24 229 участников + 100 000 сообщений на реальном редьюсере, холодный старт ~47 мс при бюджете 1500 мс, гейт регрессии >10 %. P8-01: манифест + Ed25519 над исходными байтами тела, SHA-256 до перемещений, staged-замена, откат с проверкой backup, запрет downgrade, защита от path traversal. Ревью: 2×P0 + 4×P1 (codex + TL-1) закрыты red-first; мутации **18/18**; gate passed `.gate/worker-2026-10-22T05-52-40-267Z.json`; артефакты `waves/RW-25/`.

| RW-26 | B09 (headless): P4-07 RTCP/RTP-статистика + P4-11 регионы/latency | P4-07, P4-11 | `discord-voice/src/{rtcp,reception,stats,regions}.rs` + `tests/{rtcp,reception,stats,regions}_contract.rs`; фикстуры `fixtures/rtcp-voice-udp4-server.{bin,json}`, `fixtures/regions-rtc.json` | voice 114; workspace 869 | CORE-V1/W26a, CORE-V2/W26b; review TL-1 | **Принято 2026-10-22.** P4-07: compound-парсер RTCP (SR/RR/SDES, SRTCP-трейлер 16/20, лимит 32), RFC 3550 loss/jitter по открытым RTP-заголовкам, `RttEstimator`/`StatsWindow`; test-only ветка и `#[path]`-копия устранены. P4-11: парсер `regions-rtc.json` 5×5, ранжирование измеренные→geo→лексика, `plan_probes`; wire-формат проб не изобретался. Мутации 19/19; gate `.gate/worker-2026-10-22T11-40-00-985Z.json`; артефакты `waves/RW-26/`. |
| RW-27 | B09 (headless): P7-01b ListFormat + P6-04 deep links + P6-03 автозапуск/бейдж | P7-01b, P6-04, P6-03 | `discord-i18n/src/{list,list_data}.rs` + `tests/list_contract.rs`; `discord-platform/src/{deep_link,autostart}.rs` + `tests/{deep_link,autostart}_contract.rs`; оракул `waves/RW-27/fixtures/list-oracle.json` | targeted 32; workspace 902 | CLIENT-1/W27a, PLATFORM-1/W27b; integration TL-2; review REV-2 | **Принято 2026-10-22.** P7-01b: 31 локаль × 3 стиля × 3 типа, 279 строк, 1674 кейса ICU. P6-04: `discord://` разбор, RE-маршруты, `extract_deep_link`, redaction `raw`/`query`/`fragment` в `Debug`. P6-03: XDG Desktop Entry с Exec-quoting (reserved-набор 1.5), Unity badge, `autostart_path`. Ревью: 2×P2 закрыты red-first (Exec-quoting, доказательная база); мутации list 6/6 + 2 эквивалентных, platform 14/14; gate `.gate/worker-2026-10-22T13-58-02-813Z.json`; артефакты `waves/RW-27/`. |
| RW-28 | B10 (headless): P7-01c RelativeTimeFormat | P7-01c | `discord-i18n/src/{relative,relative_data}.rs` + `tests/relative_contract.rs`; оракул `tests/data/relative-oracle.json`; генератор `waves/RW-28/gen-relative-data.mjs`; точные пути `plural_category_integer`/`format_integer_unsigned` | 12 (i18n 70; workspace 953) | CLIENT-1/W28a; review 2 прохода (независимый агент) | **Принято 2026-10-22.** 31 локаль × 8 единиц × 3 стиля (744 строки, 3392 кейса ICU), ICU-паритет `Auto` включая локальные ±2 (TL-решение `waves/RW-28/DECISIONS.md`), точные цифры/категория для `i64` > 2^53, оракул внутри крейта. Ревью round 1 — P0 Auto ±2 + P1 точность, закрыты; round 2 — P0/P1=0, P2 закрыты правкой отчёта. Мутации 8/8; gate `.gate/worker-2026-10-22T17-12-46-556Z.json`. |
| RW-29 | B10 (headless): P4-04 RTP-медиа + P4-12 reconnect + P7-01e `<t:…>` | P4-04, P4-12, P7-01e | `discord-voice/src/media/{mod,rtp,jitter}.rs` + `tests/media_contract.rs`; `discord-voice/src/reconnect.rs` + `tests/reconnect_contract.rs`; `discord-i18n/src/{timestamp,timestamp_data}.rs` + `tests/timestamp_contract.rs`; фикстура `fixtures/voice/rtp-pt120-oracle.json`; генераторы `waves/RW-29/gen-{rtp-oracle.py,timestamp-data.mjs}` | media 13; reconnect 10+2; timestamp 14 (workspace 953) | VOICE-1/W29b, W29c; CLIENT-1/W29a; review voice — независимый агент | **Принято 2026-10-22.** P4-04: RTP PT120/0xBEDE, marker/seq/ts+960, DTX-CN по длине шифротекста, джиттер-буфер mod 2¹⁶ с реордером/дублями/потерями — оракул из golden-pcap (10 пакетов, 2 аномалии, opaque-профиль зафиксирован в TZ §6). P4-12: детерминированный бэкофф 1–30 с, Resume/Identify/GiveUp, op9, сброс по READY/RESUMED. P7-01e: 7 стилей, парсер `<t:…>`, UTC, 31 локаль × 6 стилей × 3 момента (558) + R-оракул. Ревью voice — принято, P0/P1/P2=0, 8 P3 в бэклог. Мутации 8/8, 9/9, 6/6. |
| RW-30 | B11 (headless): P7-01d DurationFormat + P6-02 D-Bus-уведомления | P7-01d, P6-02 | `discord-i18n/src/{duration,duration_data}.rs` + `tests/duration_contract.rs`; `discord-platform/src/dbus/{mod,auth,message}.rs` + `notifications.rs` + `tests/{dbus_contract,notifications_contract}.rs`; оракулы `tests/data/duration-oracle.json`, `fixtures/dbus/*`; генераторы `waves/RW-30/{gen-duration-data.mjs,dbus/*.py}` | duration 13; dbus 7 + notifications 11 (workspace 984) | CLIENT-1/W30a, PLATFORM-1/W30b; review: независимый агент + codex | **Принято 2026-10-22.** DurationFormat: 31 локаль × 3 стиля × 8 единиц (4464 паттерна, 4092 кейса ICU), точные категории/группировка. D-Bus: собственный std-клиент (auth EXTERNAL, Hello, вызовы, сигналы), Notify/Close/GetCapabilities/GetServerInformation, события ActionInvoked/NotificationClosed; без unsafe/новых крейтов; оракул — приватный dbus-daemon + libdbus-мок. Ревью codex: P1 md5/P2 фикстура/P2 SKIP в мутациях — закрыты red-first (10/10, SKIP=0). Мутации 8/8 и 10/10; gate `.gate/worker-2026-10-22T17-54-26-464Z.json`. |
| RW-31 | B12 (headless): P6-06 power/idle + P2-03 SQLite-store (FFI) | P6-06, P2-03 | `discord-platform/src/power.rs` + `tests/power_contract.rs`; `discord-store/src/{lib.rs,ffi.rs}` + `tests/store_contract.rs`; мок `waves/RW-31/power/mock_power.py`; оракул `waves/RW-31/sqlite/oracle.py`; фикстура `fixtures/dbus/power.json` | power 12; store 14 (workspace 1010) | PLATFORM-1/W31a, STORE-1/W31b; review: независимый агент + codex | **Принято 2026-10-22.** P6-06: ScreenSaver inhibit/uninhibit/is_active/idle_seconds (UnknownMethod→Ok(None)), PowerMonitor PrepareForSleep с общим дедлайном, IdlePolicy (пороги — конфиг). P2-03: SQLite через FFI к системному libsqlite3 3.54 без rusqlite; миграции user_version=1, upsert, keyset-пагинация, soft delete, kv; unsafe только в ffi.rs (26 SAFETY), unsafe_code=deny. Ревью: P1/P2 закрыты red-first (якоря мутаций после fmt, md5, --nocapture/SKIP-детекция, дедлайн); мутации 7/7 и 7/7 (SKIP=0); gate `.gate/worker-2026-10-22T18-08-42-326Z.json`. |
| RW-32 | B13 (headless): P6-07 spellcheck + P6-02b hints | P6-07, P6-02b | `discord-platform/src/spellcheck.rs`, `ffi_hunspell.rs`, `notifications.rs` + тесты; `fixtures/hunspell/*`, `fixtures/dbus/hints.json`; мок `waves/RW-32/dbus/mock_hints.py` | spellcheck 13; notifications 18; dbus 8 (workspace 1031) | PLATFORM-2/W32a, PLATFORM-1/W32b; review: codex + независимый агент | **Принято 2026-10-22.** P6-07: FFI libhunspell 1.7.5, SpellChecker check/suggest/add/remove, токенайзер с байтовыми смещениями (URL/e-mail/code/mentions/emoji пропускаются), усиленная валидация .aff/.dic. P6-02b: hints `a{sv}` (urgency y, category/desktop-entry s, resident b), сигнатура Notify не менялась. Ревью: P1 валидации словаря и P3 утечки daemon закрыты red-first; мутации 8/8 и 6/6 (SKIP=0); gate `.gate/worker-2026-10-22T18-33-30-356Z.json`. |
| RW-33 | B14 (headless): P1-10 keyring через libsecret FFI | P1-10 | `discord-platform/src/{keyring,ffi_secret}.rs`, `src/lib.rs` (+2 строки), `tests/keyring_contract.rs`; oracle `secret-tool` + ksecretd | keyring 12 (workspace 1043) | SEC-1/W33a; review: codex + независимый агент | **Принято 2026-10-22.** `Keyring::{connect,store,lookup,clear}` через `secret_service_*_sync` + `secret_schema_newv`; strict `xdg:schema` (без DONT_MATCH_NAME); значение копируется по `length` (бинарные записи не портятся); `GError`/`g_free` без утечек, `put_string` освобождает key при OOM; `!Send+!Sync`, `Debug` без секрета. Ревью: P1 обрезки на NUL, P1 коллизии чужих записей, P2 утечки — закрыты red-first; live 12 passed SKIP=0 (ksecretd+secret-tool), мутации 7/7; gate `.gate/worker-2026-10-22T18-57-30-507Z.json`. |

Приёмка RW-01 (2026-10-19): 24 теста `discord-codec` (13 векторов + 9 свойств + 2 либ),
воркспейс 34/34 зелёные (`cargo test --offline`); red-first — падения заглушек (9+12) и 21/21 на
финальных тестах (логи `/tmp/opencode/rw-01/red*.txt`); `cargo clippy -D warnings` и `cargo
fmt --check` чисты; `gate:worker` passed (`.gate/worker-2026-10-19T10-49-38-957Z.json`); независимое
ревью (субагент): 95/95 машинных сверок hex/значений с сырьём и `gw-proto/msgs-*.jsonl`, две
мутации пойманы тестами, внешний тест на публичном API 5/5; ниты закрыты док-комментариями
(ограничения энкодера >64 КиБ атом / >4 ГиБ строки — вне данных Discord). Снимок приёмки:
`rust/crates/discord-codec/src/etf.rs` md5 `a636e9cd…`, `lib.rs` md5 `602968dd…`.

Приёмка RW-02 (2026-10-19): 9 тестов zstd-стрима (кодек 33/33, воркспейс 43/43;
`cargo test --offline`); red-first — 9/9 падений на заглушке (`/tmp/opencode/rw-02/red.txt`; нит:
лог снят до финальной правки тестов — номера строк разошлись, содержательно red воспроизведён
ревьюером на финальной редакции); `clippy -D warnings` / `fmt --check` чисты; `gate:worker`
passed (`.gate/worker-2026-10-19T11-27-32-889Z.json`; meta 22 / unit 169 / smoke 20; ревьюер
перезапустил гейт в полной копии — фазы совпали). Независимое ревью (субагент): оракулы
перевыведены своими инструментами (137 кадров / 104 066 → 824 816 / 759 280, окно 65 536 =
window descriptor `0x30`; Σbytes 824 679 + 137×`0x83`; 44 полных терма / смещение 758 576 /
остаток 704; синтетика 67 → 139 264, md5 payload `4dbd601d…`), внешний тест публичного API —
8 тестов / 390 проверок, 4/4 мутации пойманы. Снимок приёмки: `zstd.rs` md5 `0b4b9abb…`,
`lib.rs` `95e12674…`, `zstd_stream.rs` `4dd800a4…`, `tests/data/synth-zstd-frame.hexl`
`0a7742ae…`. Известная семантика: ruzstd удерживает хвост размером окна кадра (64 КиБ) до его
завершения — задокументировано; пересмотр (при необходимости) — P1-02.

Приёмка RW-03 (2026-10-19): 15 интеграционных тестов сессии (`discord-core` — 17, воркспейс
59/59; `cargo test --offline`); red-first — 12/12 падений на заглушке (`/tmp/opencode/rw-03/red.txt`;
у прерванной попытки был невалидный red-лог с ошибками компиляции — сохранён как
`red-prev-attempt.txt`); `clippy -D warnings` / `fmt --check` чисты; `gate:worker` passed
(`.gate/worker-2026-10-19T12-29-11-179Z.json`; meta 22 / unit 169 / smoke 20). Независимое ревью
(субагент): оракулы перевыведены (таймлайн 137 / op10=1 / op11=23 / op0=113, s 1..113; first_ack
1214, last_ack 658 927, max дельта 41 263, 9 дельт в [41 050; 41 300]); HELLO-терм (138 Б) сверен
mini-парсером ETF и независимой распаковкой (`zstd` CLI + scratch `ruzstd --offline`); симуляция
всех 41 250 сидов — 0 зомби, удары 11–12 (seed 1214 → 12); 11 мутаций — 8 пойманы, 3 дыры покрытия
закрыты доп. тестами (мутации снова падают целевыми тестами), внешние крайние тесты 6/6. Снимок
приёмки: `gateway.rs` md5 `f6fdb06b…`, `lib.rs` `21aea9a7…`, `gateway_session.rs` `c974da4d…`.
Граница волны: транспорт (сокет/TLS/WS) не реализован — RW-04 (нужны крейты tokio/tungstenite и
разрешение владельца на сеть); T-GW-01/03 покрыты логикой без стаб-сервера.

Приёмка RW-04 (2026-10-19): транспорт шлюза — `discord-core::ws` (пайп zstd→термы→сессия,
`connect_and_run`, стаб-сервер в тестах; 13 тестов, `discord-core` — 30, воркспейс 72/72;
`cargo test --offline`); red-first — 10/10 падений (первый прогон `red-first.txt` с сетевой
сборкой и воспроизведённый `red.txt` на заглушке `ws-stub.rs`; ревьюер независимо воспроизвёл
red байт-в-байт); `clippy -D warnings` / `fmt --check` чисты; `gate:worker` passed
(`.gate/worker-2026-10-19T13-17-20-623Z.json`; meta 22 / unit 169 / smoke 20). Независимое ревью
(субагент): оракулы перевыведены (полная распаковка 824 816 Б; проверено, что пайп получает
44 полных терма = первые 44 полного потока, `last_seq == 36`, остаток 704 Б — удержание окна
ruzstd, не потеря; HELLO 41 250; 10/10 прогонов без флаков), 9 мутаций — 6 пойманы сразу, 3 дыры
покрытия закрыты доп. тестами (Ping→Pong с payload, пунктуальность зомби ≤500 мс при интервале
300 мс, desync-guard, счётчик Text; мутации снова падают целевыми тестами), lab-кейс зафиксирован
на фактическом контракте (см. backlog). Снимок приёмки: `ws.rs` md5 `ca5791e3…`, `lib.rs`
`05ed2e5a…`, `ws_transport.rs` `1d091ee2…`, `Cargo.toml` `900f1e7b…`, `Cargo.lock` `d067aaa1…`.
Известные ослабления/открытые вопросы: `wss://` без рантайм-кейса (нужен CA/живой хост),
`Reconnect` не шлёт close-кадр, ветка `Text/Pong/Frame` — счётчик без точных ассертов формы.
`s`-непрерывность resume-сессий: lab окно `.8608` начинается с s=664 — покрытие resume/op7/op9
впереди (P1-05/RW-06).

Сдача RW-05 (2026-10-19): IDENTIFY (op2) + READY-сводка — `discord-core::identify` (`build_identify`
= payload, `build_identify_frame` = провод `{op:2, d:…}`), `discord-core::ready` (`ReadySummary`),
точечные правки `ws.rs` (`RunOptions`, `connect_and_run_with`, IDENTIFY один раз после HELLO,
`GatewayPipe::last_ready`); 16 тестов `identify_ready` (`discord-core` — 46, воркспейс 88/88;
`cargo test --offline`). Ход волны: воркер-субагент прерван рейт-лимитом после green-core (не дописал
воркспейс/clippy/гейт/отчёт); координатор нашёл и исправил дефект ТЗ — IDENTIFY уходил плоским
payload вместо `{op:2, d:{…}}` (red `red-contract-fix.txt`), догнал волну до конца. red-first:
`red.txt` — 12 failed / 1 passed (пройденный — негативный контроль старого пути RW-04);
`clippy -D warnings` / `fmt --check` чисты; `gate:worker` passed дважды
(`.gate/worker-2026-10-19T13-58-37-508Z.json`, `…T14-12-35-509Z.json`; meta 22 / unit 169 / smoke 20).
Self-review (мутационное; независимый ревьюер-субагент 1× — рейт-лимит): 11 мутаций — 8 пойманы,
3 дыры закрыты тестами red-first при живых мутантах (`ready_accepts_string_user_id`,
`ready_accepts_int_partial`, `ws_identify_only_after_hello`; логи `/tmp/opencode/rw-05/mut/`).
Отчёт: `/tmp/opencode/rw-05/report.md`; независимое ревью — первым делом после снятия рейт-лимита,
приёмка — после него. Нит: проводные формы сверять по `pack({op, d})` до фиксации тестов.
Снимок: `identify.rs` md5 `17f2f310…`, `ready.rs` `c7c98489…`, `ws.rs` `659ee47e…`, `lib.rs`
`8986fa10…`, `identify_ready.rs` `ea4c7f18…`.

Сдача RW-06 (2026-10-19): RESUME/реконнект — `discord-core::resume` (`ResumeParams` с редакцией
токена, `build_resume_frame` `{op:6,d:{token,session_id,seq}}`, `ResumeState`,
`plan_reconnect`/`ReconnectAction`, порог `RESUME_MAX_DOWNTIME_MS = 30 000`), правки `ws.rs`
(`connect_and_run_resume` — op6 вместо op2 после первого HELLO, `resume_state()`,
`last_invalid_session_resumable()` из op9 `d`, `TransportError::Resume`); 12 тестов
`resume_reconnect` (`discord-core` — 58, воркспейс 100/100; `cargo test --offline`). Контракт
`RunOptions` не расширялся — существующие тесты RW-05 не правились. red-first на финальной
редакции: 11/12 падений (`red.txt`, `# EXIT=101`, md5 теста `60bf79f4…`; прошёл только
Debug-редакция), затем green: 58/58 `discord-core`, 100/100 воркспейс, `resume_reconnect` ×5
стабильно; `clippy -D warnings`/`fmt --check` — после 3 нитов теста чисто; `gate:worker` passed
(`.gate/worker-2026-10-19T14-33-22-949Z.json`). Оракулы T-GW-05: run2-артефакты `.8269` (s 95..663,
569 op0), `.8608` (664..722, 59), `.10029` (1..331, op7), `.13072` (RESUMED 332..371, 40), швы
663+1=664 и 331+1=332; lab-кейс skip-aware. Отчёт: `/tmp/opencode/rw-06/report.md`. Нит: порог
30 с — контракт плана (в захвате разрывы ~2.9 с); при новом захвате пересмотреть. Независимое
ревью RW-05/RW-06 — после снятия рейт-лимита субагентов; приёмка — после него.

Приёмка RW-05 и RW-06 (2026-10-19): независимое ревью (субагент;
`/tmp/opencode/review-rw-0506.md`) — **RW-05 «принято с нитами»** (red-first подтверждён логом и
собственной репродукцией заглушками, md5 и lab-оракулы сошлись, мутации 10/10; ниты N1 сводка
`mut/results.txt`, N2 строки файлов, N3 промежуточные md5 — закрыты координатором) и **RW-06
«принято с нитами»** (репродукция red 1:1, md5/оракулы сошлись, 18/21 мутаций поймано; выжившие
R4 порог-константа, R10 RESUME до HELLO, R11 варианты ошибок — дыры покрытия, закрыты red-first:
`red-gap-tests.txt` `# EXIT=101`, 10 passed/3 failed ровно по целевым тестам, md5 теста
`9852c2ca…`, копия red-версии сохранена). Инцидент: после мутаций ревьюера restore без `touch`
оставил залипший rlib (тесты линковались с мутированной сборкой) — вылечен `cargo clean` + полный
повтор прогонов; процедура восстановления теперь с `touch`. Финальные числа на чистой сборке:
`discord-core` **59** (resume_reconnect **13**), воркспейс **101/101**, `resume_reconnect` ×5
стабильно; clippy/fmt чисто; `gate:worker` passed (`.gate/worker-2026-10-19T15-05-50-384Z.json`).
Снимок принятых: `resume.rs` `211448f7…`, `ws.rs` `b822d6a6…`, `lib.rs` `69e299f2…`,
`resume_reconnect.rs` `9852c2ca…`; RW-05 без изменений (`identify.rs` `17f2f310…`, `ready.rs`
`c7c98489…`, `identify_ready.rs` `ea4c7f18…`). Следующая волна — RW-07 (P1-06, роутер диспатчей,
T-GW-08).

Сдача RW-07 (2026-10-19): роутер диспатчей — `discord-core::dispatch` (`DispatchKind`:
21 тип, `ALL`/`from_name`/`as_str`, `DispatchEvent`, `route_dispatch`), правки `ws.rs`
(очередь `events`, `events()`/`take_events()`, накопительный `unknown_dispatches()`; роутер
в цикле `push_binary` после op9, до `drain`) и `lib.rs` (модуль + ре-экспорты); 11 тестов
`dispatch_router` при сдаче (`discord-core` — 70, воркспейс 112). Red-first: 3 passed /
8 failed (`# EXIT=101`, md5 `4fdfe8a8…`; прошли негативные контроли U2/U5/P3), затем green.
Оракулы T-GW-08: union 7 снимков `msgs-*.jsonl` — 21 тип / 1340 op0; сокет `.1389` —
44 терма (36 op0, s = 1..36; удержание окна ruzstd, 704 Б хвоста; таймлайн 113 — хвост не
потерян). Отчёт: `/tmp/opencode/rw-07/report.md`.

Приёмка RW-07 (2026-10-19): независимое ревью (субагент; `/tmp/opencode/review-rw-07.md`) —
**«принято с нитами»**, блокеров нет: границы (только 4 файла ТЗ; соседи RW-06 по md5 целы,
`Cargo.toml/lock` не тронуты), red-репродукция 1:1, оракулы пересчитаны независимо (python:
union / префикс / hexl / таймлайн) 1:1, 18 мутаций — 16 поймано. Выжившие m (регистр имени:
`route_dispatch` с именем «ready») и q (порядок `take_events`) + опц. нит `s:"nil"` — дыры покрытия,
закрыты координатором red-first (`/tmp/opencode/rw-07/red-gap.log`: 9 passed / 3 failed ровно
по целевым, `# EXIT=101`, md5 теста `fbb4e1bf…`, копия `dispatch_router-gap-red.rs`); продукт
при закрытии не менялся (`dispatch.rs` `d929b022…`, `ws.rs` `fa6e051a…`, `lib.rs`
`fa07ceb6…`). Финальные числа: `discord-core` **71** (`dispatch_router` **12**), воркспейс
**113/113**, ×5 стабильно; clippy `-D warnings`/fmt чисто; `gate:worker` passed
(`.gate/worker-2026-10-19T16-18-34-369Z.json`). Тест `dispatch_router.rs` `fbb4e1bf…`.
T-DATA-01 (разбор READY-крупняка) в волну не входит — роутер отдаёт payload без разбора
(это P2-01). Следующая волна — RW-08 (P1-07, REST-клиент, T-REST-01…03).

Старт RW-08 (2026-10-19): P1-07, REST-клиент — первый параллельный запуск (PROCESS.md §3.13,
пожелания владельца). Три воркера одновременно: W8a — каталог 47 маршрутов (`routes.rs`,
T-REST-01), W8b — 6 auth-заголовков и классы кодов (`headers.rs`, T-REST-02), W8c — таймауты/
ретраи идемпотентных (`retry.rs`, T-REST-03, локальный стаб `127.0.0.1`); после зелёного —
рефакторинг-воркер W8d и независимое ревью W8r. Контракты и тест-данные заморожены в
`waves/RW-08/TZ.md`; новые зависимости не добавляются (решение координатора в рамках вариантов
владельца: ручной клиент/стаб; реальный TLS-транспорт — открытый вопрос). Тест-данные —
`rust/crates/discord-core/tests/data/rest-*.tsv` (выдержки из `docs/re/rest-d02.json`, md5
`87d7a05d…`).

Приёмка RW-08 (2026-10-20): параллельная волна — W8a/W8b/W8c запущены одновременно, затем
рефакторинг W8d и независимое ревью W8r. Red-first: 1/7, 2/3, 0/10 (`# EXIT=101`); после fmt
теста W8a и добавления кейса N6 red переснят (см. `waves/RW-08/red/`). Итоги: `discord-core`
95, воркспейс 137/137, 24 новых кейса (T-REST-01…03), clippy/fmt чисто, гейт passed; ревью —
«принято с нитами», 14/14 мутаций, выжившая мутация `sleep(backoff)` закрыта кейсом N6
(red 0/11 → green 11/11, `# EXIT=101`). Отчёт: `waves/RW-08/report.md`; ревью:
`waves/reviews/review-rw-08.md`. Следующая волна — RW-09 (P1-08, X-Super-Properties).

Старт RW-09 (2026-10-20): P1-08, X-Super-Properties — вторая параллельная волна (PROCESS.md §3.13).
Три воркера одновременно: W9a — base64 RFC 4648 (`rest/base64.rs`), W9b — JSON-запись/разбор 22
полей и base64 (`rest/super_properties.rs`, T-REST-04), W9c — честная идентичность и сборка
(`rest/identity.rs`); затем рефакторинг W9d и независимое ревью W9r. Контракты заморожены
(каркас-заглушки в репозитории, md5 в `waves/RW-09/TZ.md`), выдержки-оракулы — из живого снимка
`/home/sanya/re-deep/d02-snapshot` (requestId `95974.2323`; md5 выдержек в ТЗ). Новые зависимости
не добавляются (base64/JSON/UUID — вручную); сеть не используется. Отдельно проработан вопрос
HTTP-транспорта: `waves/TRANSPORT.md` (рекомендация — ручной клиент на tokio+tokio-rustls;
решение владельца ожидается).

Приёмка RW-09 (2026-10-20): параллельная волна — W9a/W9b/W9c одновременно, затем W9d и
независимое ревью W9r. Red-first: 0/5, 0/8, 0/8 (`# EXIT=101`); после fmt теста W9a и нита N1
red переснят (`waves/RW-09/red/`; mutation-red — `w9a-red-mutation-n1.log`). Итоги: `discord-core`
**116**, воркспейс **158/158**, 21 новый кейс (T-REST-04), clippy/fmt чисто, гейт passed; ревью —
«принято с нитами», 20/21 мутаций; ниты N1/N6 закрыты red-first, N4/N5 исправлены, N2 аннотирован,
N3 — в backlog. Отчёт: `waves/RW-09/report.md`; ревью: `waves/reviews/review-rw-09.md`.
Очередь: транспортная волна (решение A), затем P1-09 (rate-limit/ошибки).

## Backlog / открытые вопросы

- RW-09/N3 (политика идентичности): `IdentityConfig` с `Some("")` возвращает пустую строку
  дословно (не «отсутствие»/`unknown`) — решить при живом подключении генератора к REST/шлюзу.

- HTTP-транспорт: **решено 2026-10-20 — вариант A** (ручной HTTP/1.1 на tokio+tokio-rustls;
  разбор — `waves/TRANSPORT.md` §6); разрешены прямые `tokio-rustls` (+ опц. `httparse`/`bytes`)
  в `Cargo.toml` без сетевых загрузок. Реализация — волной после RW-09 (до P1-09/P2-06).

- RW-07/P1-06 (нит N4 ревью, дизайн): буфер `GatewayPipe::events` растёт до дренажа — дренаж
  на вызывающем (контракт ТЗ §1.3, документирован); при цикле приложения/супервайзере
  (после P1-07) определить точку дренажа, иначе рост памяти в долгих сессиях.
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
- RW-04/P1-02: lab-кейс `lab_capture_stream_feeds_pipe` фиксирует `last_seq == Some(36)` вместо 113 —
  удержание окна ruzstd (64 КиБ) до завершения кадра; удержанное не потеряно. Пересмотреть декодер
  в P1-02 (стриминговое завершение/иной декодер) и вернуть строгую проверку `s == 113`.
- RW-04 (P4): `start + Duration::from_millis(ms)` в цикле транспорта теоретически переполняем при
  враждебном `heartbeat_interval` (недостижимо на Linux; безопаснее `checked_add` с насыщением) —
  пометить при следующем касании.
- Процесс (нит ревью RW-04): в red/green-логи волн включать `# EXIT=…` и md5 тест-файла — связь
  «red снят на финальном тесте» должна доказываться содержимым лога, а не mtime.
