# MERGE-READINESS-13 — RW-40: 38 приёмок, beta/BUILD-004 = 38/38 IPC, unstable = 38/41

Дата: **2026-10-31, 00:55 +05**. Снимок сделан по рабочему дереву `/home/user/orca/project`
и по веткам `main` / `unstable` / `testing` / `beta`.
Воркер: **G6 Parity**, карточка **G6-PARITY-SYNC-13**, задача `task_17e8a0993772`.
Режим — **read-only по продуктовому коду и тестам**: `cargo` не запускался, `git commit` не
выполнялся, окно и `gui-window`-проба не спавнились. Изменён ровно один файл — этот отчёт.

> **Важно про время.** Пока шёл проход, состояние менялось: в 19:36 появился зелёный gate,
> в 19:38–19:39 — приёмки INT-05 и AT-03, в 19:58 — INT-06, в 21:39 — второй зелёный gate,
> в 21:52 — QA Passed для BUILD-004, в 23:22 — релизный коммит `beta`, в 00:32 — новые коммиты
> `unstable`/`testing`. Всё, что ниже, проверено на снимке 00:55; более ранние версии этого
> отчёта (если где-то сохранились) устарели.

---

## 0. Резюме и вердикт

**Вердикт двойной, потому что двойным стало состояние.**

1. **RW-40 как релизная сборка `BUILD-004` (`beta`, `e91acf6`, 23:22) — GO.**
   - Все 4 среза из задания приняты и вошли в `beta`: `#31 INT-04-ATTACH-PUT-BACKEND Pass 2`,
     `#32 STUB-16 / MB-03 Pass 3`, `INT-05-IPC-ALIASES` (`#34`), `AT-03-UI-RENDER`.
     Для AT-03 и STUB-16 байты в `beta` совпадают с принятыми точно (`chat.js`,
     `members.js`); `main.rs` после INT-05 ушёл вперёд (INT-06, task-109), но все четыре
     алиаса INT-05 в нём сохранены — это проверено по исходнику, а не по отчёту.
   - **IPC-паритет на `beta` — 38 из 38 (100 %)**: 49 команд объявлены и зарегистрированы,
     каждый вызов `invoke`/`invokeFn`/`invokeReaction` из UI находит команду. Пять имён из
     задания (`attachments_upload_slots`, `attachments_upload_put`, `create_invite`, `pins_list`,
     `threads_list`) закрыты.
   - QA `BUILD-004` после retest: **QA Passed**, P0=0, P1=0 (в объёме retest), unit `977/977`,
     meta `102/102`, типы чистые.

2. **Текущее дерево и `unstable`/`testing` (00:32) — BLOCK на слияние в `main`.**
   - **IPC-паритет упал до 38 из 41 (92,7 %)**: три вызова UI не имеют Tauri-команды —
     `add_friend`, `dm_create` (`user-popout.js:284,392`) и `get_reaction_users`
     (`reactions.js:1067`).
   - **Дерево красное:** `npm run test:unit` — 1176 тестов, **1 fail**; `node --test tests/meta/` —
     102 теста, **6 fail**.
   - **Последний зелёный gate (21:39) устарел:** после него изменено 86 файлов исходников и
     тестов, включая `chat.js` (23:27) и весь блок DM/Friends/Reactions/UserPopout.

3. **Приёмочные артефакты перестали описывать дерево.** `chat.js` в дереве —
   `5d938a86…`, а принят и лежит в `beta` — `0e3dc498…`; `members.js` — `6b4e06af…` против
   принятого `55356f6e…`. Это не подлог, а дрейф после приёмки, но означает: **приёмки RW-40
   привязаны к `beta`, а не к текущему `unstable`**.

---

## 1. Топология репозитория

| Ветка | HEAD | Дата | Комментарий |
|---|---|---|---|
| `main` | `4e24048` | 2026-10-24 16:22 | база, от которой считались все срезы волны |
| `testing` | `0314cff` | 2026-10-31 00:32 | QA-ветка, ушла вперёд после retest |
| `unstable` | `f331110` | 2026-10-31 00:32 | 43 коммита, PR-01…PR-63 |
| `beta` | `e91acf6` | 2026-10-30 23:22 | `release(beta): BUILD-004 (RW-40 PR-01..PR-20 QA Passed)` |
| Рабочее дерево | `4e24048` + **1077** грязных путей | — | содержит весь `unstable` плюс незакоммиченную работу |

| Параметр | Значение | Метка |
|---|---|---|
| Записей приёмки `ACCEPT*.md` | **38** (36 в `waves/RW-40/PM/`, `PM/g01b-contract`, `QUALITY/`) | [V] |
| Нумерация `Code Slice #` | проставлена только в 5 файлах: `#27`, `#31`, `#32`, `#33`, `#34`; остальные без номера | [V] |
| Последний gate-артефакт | `.gate/worker-2026-10-30T16-39-13-422Z.json` (21:39), `verdict: passed` | [V] |
| Файлов изменено после этого gate | **86** (`rust/`, `tests/`, `scripts/`) | [V] |
| `npm run test:unit` (мой прогон) | 1176 tests / **1175 pass / 1 fail** / 0 skip / 211 suites | [V] |
| `node --test tests/meta/` | 102 tests / **96 pass / 6 fail** | [V] |
| `npm run typecheck` | exit 0 | [V] |
| `npx tsc -p rust/gui-spike/ui/tsconfig.json` | exit 0 | [V] |
| `@ts-ignore` / `@ts-nocheck` / `.only` / `.skip` / `.todo` | 0 | [V] |

---

## 2. Реестр приёмок

38 записей. Целевые срезы задания и то, что закрылось по ходу прохода:

| Запись | Срез | Артефакт | Итог |
|---|---|---|---|
| **#31** | `INT-04-ATTACH-PUT-BACKEND` Pass 2 | `PM/b2b-attach-ui/ACCEPT-BE-PUT.md` | ACCEPTED (19:03) |
| **#32** | `STUB-16 / MB-03` Pass 3 (AT-SPI `list item`) | `PM/stub16-profile-ui/ACCEPT.md` | ACCEPTED (19:03) |
| **#33** | `N-05-NOTIFICATIONS-BACKEND` | `PM/n05-be/ACCEPT.md` | ACCEPTED (19:37) |
| **#34** | `INT-05-IPC-ALIASES` Pass 2 | `PM/int05-ipc-aliases/ACCEPT.md` | ACCEPTED (19:38) |
| — | `AT-03-UI-RENDER` Pass 2 | `PM/at03-render-ui/ACCEPT.md` | ACCEPTED (19:39) |
| — | `INT-06-NOTIFICATIONS-IPC-WIRING` | `PM/int06-notif-ipc/ACCEPT.md` | ACCEPTED (19:58), закрыл F-5 |
| — | `N-05-UI-NOTIFICATIONS` Pass 2 | `PM/n05-ui-notifications/ACCEPT.md` | ACCEPTED (20:00) |
| — | `M-13-SEARCH-202-NSFW-BACKEND` Pass 2 | `PM/m13-search-be/ACCEPT.md` | ACCEPTED (20:26) |
| — | `task-109` строгий IPC-оракул | `PM/task109-ipc-oracle/ACCEPT.md` | ACCEPTED (20:26), закрыл дыру оракула |
| — | `C-02-BE`, `L-02` (повторная приёмка) | `PM/c02-be`, `PM/l02` | ACCEPTED (21:34) |

Остальные 28 записей — из реестров `MERGE-READINESS-12.md` §2 и предыдущих снимков; их файлы
после 19:00 не менялись, кроме `chat.js`/`members.js` (см. §4.3).

---

## 3. IPC-паритет UI → Rust

### 3.1. Метод

Независимый оракул — исходники, а не отчёты. Для каждой ветки отдельно:

1. `git show <ref>:rust/gui-spike/src/main.rs` → все `#[tauri::command]` и тело
   `tauri::generate_handler![...]`.
2. Все `.js` из `rust/gui-spike/ui` этой же ревизии → вызовы любых идентификаторов вида
   `*invoke*(` с строковым литералом (`invoke`, `invokeFn`, `invokeReaction`, `invoker`).
3. Команды из констант: `AUTH_COMMANDS`, `NET_COMMANDS`, `JOIN_COMMANDS`, `READY_COMMAND`.

### 3.2. Результат по веткам

| Ревизия | Команд объявлено / зарегистрировано | Имён в UI | Не найдено | Паритет |
|---|---:|---:|---:|---:|
| `beta` (`e91acf6`, BUILD-004) | 49 / 49 | 38 | **0** | **100 %** |
| `unstable` (`f331110`) | 49 / 49 | 41 | 3 | 92,7 % |
| `testing` (`0314cff`) | 49 / 49 | 41 | 3 | 92,7 % |
| рабочее дерево | 50 / 49 | 41 | 3 | 92,7 % |

В рабочем дереве 50 объявлений против 49 регистраций — это `main.rs` в процессе правки
(`88e54b69…`), в `unstable`/`beta` такого расхождения нет.

### 3.3. Пять имён из задания — проверено построчно

| UI-вызов | Аргументы UI | Rust `fn` (`main.rs`) | В `generate_handler!` | Итог |
|---|---|---|---:|---|
| `attachments_upload_slots` (`composer.js:725`) | `channelId`, `files[{filename,file_size,id,is_clip}]` | 758 | 1093 | OK |
| `attachments_upload_put` (`composer.js:760`) | `channelId,id,uploadUrl,uploadFilename,filename,fileSize,contentType,file,bytes,data` | 531 | 1094 | OK |
| `create_invite` (`invite-modal.js:370`) | `channelId,maxAge,maxUses,temporary,flags` | 705 | 1091 | OK |
| `pins_list` (`app.js:1372`) | `channelId` | 778 | 1096 | OK |
| `threads_list` (`app.js:1378`) | `channelId` (без `archived`) | 796 | 1100 | OK |

Контракт аргументов сходится и по остальным вызовам: скриптовая сверка «обязательные параметры
команды ⊆ ключи объекта из UI» по 34 литеральным call-site дала **0 расхождений**. Токен ни в
одну из пяти команд не приходит из фронтенда — только `token_or(backend.token())`.

### 3.4. Три расхождения на `unstable`/`testing`/дереве (P1)

| Команда | Вызов | Состояние |
|---|---|---|
| `dm_create` | `ui/features/user-popout.js:284` | в `main.rs` нет ни объявления, ни регистрации |
| `add_friend` | `ui/features/user-popout.js:392` | то же |
| `get_reaction_users` | `ui/features/reactions.js:1067` | то же |

Оба модуля смонтированы в `app.js` (`createUserPopoutController`,
`createDmFriendsController`), то есть вызовы достижимы из UI. На `beta` их нет — там parity 38/38.

### 3.5. Почему это не поймал «строгий оракул 38/38» (P2)

`task-109` расширил `extract_ui_invoke_commands` (`tests/ipc_wiring_contract.rs:899`) до шести
префиксов: `invoke(`, `invokeFn(`, `invokeReaction(` и их варианты с одинарной кавычкой, плюс
разбор четырёх константных объектов. Но новые модули зовут **`invoker(`**:

- `user-popout.js:140` — `const invoker = options.invoker ?? options.invoke;`
- `reactions.js:295` — `const invoker = options.invoke !== undefined ? options.invoke : invokeReaction;`
- `reactions.js:1057` — `const invoker = resolveInvoke();`

Префикса `invoker("` в списке нет, поэтому тест остаётся зелёным при трёх несуществующих
командах. Это ровно тот же класс дефекта, что был у `invokeReaction` до task-109 — лечение
симптома вместо обхода по `ui/**/*.js` и поиска любого идентификатора с `invoke`.

### 3.6. Прочая поверхность

11 зарегистрированных команд UI не вызывает: `create_channel_invite`, `list_channel_pins`,
`list_channel_threads`, `pin_channel_message`, `unpin_channel_message`, `request_upload_slots`
(канонические имена, вытесненные алиасами), `members_page`, `search_page`, `settings`,
`net_disconnect`, `net_dispatch_stats`. Мёртвая поверхность, не дефект.

---

## 4. TDD и TEST-POLICY по целевым срезам

Требования: `TEST-POLICY.md` §2 (запреты), §3 (red → green → targeted → gate → review →
mutation), §4 (команда → exit code → артефакт), §9 (DoD).

| Срез | RED | GREEN | Мутации | Gate на финальных байтах | Итог |
|---|---|---|---|---|---|
| **#31** INT-04 Pass 2 | `red-be-put-pass2.log`: 3 стадии, `101` | `green-be-put-pass2.log`: 8/8 + 10/10 + 23/23 + 5/5, clippy/rustfmt/ipc-path 0 | 5/5, restore 4 MD5 | `.gate/…14-36-10` passed | **PASS** |
| **#32** STUB-16 Pass 3 | `red-pass3.log`: `actual 'button'` vs `'listitem'`, smoke `list item 3 < 10` | `green-pass3.log`: smoke 1/1, unit 166/166, ui-tsc 0 | 5/5, restore `55356f6e…` | тот же | **PASS** |
| **INT-05** | `red.log`: `101`, «`attachments_upload_slots` не зарегистрирован» | `green.log`: 8/8 + 11/11 + 7/7, clippy/rustfmt/ipc-path 12/12 | 5/5, restore `a9b2c916…` | тот же | **PASS** (REVIEW-PASS2, ACCEPT #34) |
| **AT-03** | `red.log`: 11 тестов, pass 0, fail 11 | `green.log`: 238/238, tsc 0/0 | 5/5, финальные MD5 = baseline | тот же | **PASS** (REVIEW-PASS2), см. §4.2 |

### 4.1. Проверки политики

| Проверка | Результат |
|---|---|
| RED падает по контракту, не `ERR_MODULE_NOT_FOUND` | да, во всех четырёх |
| GREEN с exit code | да, `EXIT_CODE=0` в каждом логе |
| Targeted и смежные наборы | 23/23, 166/166 + smoke, 26/26 rust, 238/238 |
| Мутации 5/5 caught с восстановлением MD5 | да, во всех четырёх |
| `.only` / `.skip` / `.todo` / `assert.ok(true)` | 0 во всём `tests/unit|smoke|e2e` |
| `@ts-ignore` / `@ts-nocheck` | 0 в `rust/gui-spike/ui` и `tests` |
| Секреты в логах | нет; `ack_message` передаёт `token: null` |

### 4.2. Дефект приёмки AT-03 (P2, документация)

`PM/at03-render-ui/ACCEPT.md` ссылается на артефакты, которых нет:

- `rust/gui-spike/ui/features/attachments-render.js` — **файла не существует**; фактический код
  лежит в `chat.js` (так и написано в `REPORT.md`). В `ui/features/` 34 модуля, среди них
  `attachments.js`, но не `attachments-render.js`.
- «23 unit-теста» — в `tests/unit/attachments-render.test.mjs` их **11**, все зелёные.
- `mutations/mutation-summary.json` — каталога `mutations/` в `PM/at03-render-ui/` нет.

Код при этом рабочий и покрыт; дефект чисто трассировочный, но при аудите приёмок он всплывёт
первым. Нужна правка `ACCEPT.md`, а не кода.

### 4.3. Дрейф принятых байтов (P2)

| Файл | Принято (и лежит в `beta`) | В дереве / `unstable` |
|---|---|---|
| `ui/features/chat.js` | `0e3dc4984bf7a5db8f400e3c288f873f` | `5d938a865356d284971ea25d60057ffb` |
| `ui/features/members.js` | `55356f6ee740d224c1339d70fc08ec1d` | `6b4e06afe0314b64d8302354fd702291` |
| `src/main.rs` | `ae573539a9b8a8cc6e355c3975edaff0` (`beta`) | `88e54b69…` (дерево), `24bb37a8…` (`unstable`) |

Вывод: **приёмки RW-40 описывают `beta`**, а не текущее дерево. Это нормально для движущейся
волны, но означает, что «принято» ≠ «в дереве»; в `MERGE-READINESS` это надо фиксировать явно.

MD5 логов целевых срезов (совпадают с отчётами): `at03/red` `545dcbe0…`, `at03/green` `0a38fa67…`,
`at03/mutations` `beec144a…`, `int05/red` `dbbb671c…`, `int05/green` `d9c7ed61…`,
`int05/mutations` `e751e9b2…`, `stub16/red-pass3` `7672e639…`, `stub16/green-pass3` `edbbcc1f…`,
`stub16/mutations-pass3` `cff5c073…`, `be-put/{red,green,mutations}-pass2`
`7090a232…`/`d49fc77a…`/`970e8596…`.

---

## 5. Текущее состояние дерева

### 5.1. Красные проверки (мой прогон, 00:50)

| Проверка | Результат | Что именно |
|---|---|---|
| `npm run test:unit` | 1176 tests, **1175 pass, 1 fail**, 0 skip, 211 suites | `dispatch_ui.test.mjs` → «A2: FORWARDED_KINDS — 21 имя DispatchKind::as_str + MESSAGE_REACTION_REMOVE»: UI-константа `FORWARDED_KINDS` разошлась с набором имён провода в `discord-core/src/dispatch.rs` |
| `node --test tests/meta/` | 102 tests, **96 pass, 6 fail** | PLAT-02 T1 (3 теста) и T3 (2 теста) — набор `live-keyring` `#[ignore]` не совпадает с ожидаемым (`12`); `plan-sync` — 7 новых тестовых файлов без строки в `TDD-PLAN.md §8`: `attachments-ui`, `guild-modals`, `members-subscriptions`, `notifications`, `pins-threads`, `reactions-ui`, `user-popout` |
| `npm run typecheck` | exit 0 | — |
| `ui-tsc` | exit 0 | — |

### 5.2. Gate

| Артефакт | Время | Вердикт | Покрытие |
|---|---|---|---|
| `.gate/worker-2026-10-30T13-37-49-761Z.json` | 18:37 | failed | meta 101/102, smoke 21/22 |
| `.gate/worker-2026-10-30T14-36-10-163Z.json` | 19:36 | **passed** | meta 102/102, unit 876/876, smoke 22/22, workspace 1142/1154 |
| `.gate/worker-2026-10-30T16-39-13-422Z.json` | 21:39 | **passed** | meta 102/102, unit 934/934, smoke 22/22, workspace 1146/1158 |
| Текущее дерево | 00:55 | **не покрыто** | 86 файлов изменены после 21:39 |

То есть: **на байтах `beta` зелёный гейт есть, на текущем `unstable` — нет.**

### 5.3. QA-контур

`BUILD-004` прошёл два круга: первый — **QA Failed** (P0=3: не подключены `dm_friends`/
`voice_devices`, fallback `discord_rest_request`, тест без §8), второй — **QA Passed**
(21:36–21:52, unit `977/977`, meta `102/102`, типы чистые). Процессный хвост P1-1
(«состав сборки не зафиксирован в git») на момент QA оставался открытым; релизный коммит `beta`
в 23:22 его закрывает.

---

## 6. Матрица паритета

Счётчики `PARITY-MATRIX.md` §14 (независимый `awk`-оракул из самого файла, снимок 20:59):

| App-статус | Кол-во |
|---|---:|
| implemented (+ live unverified) | 23 |
| partial | 41 |
| absent | 27 |
| unknown | 0 |
| **Итого** | **91** |

P0=5, P1=32, P2=40, P3=14.

Матрица по-прежнему **не отражает** INT-05/INT-06: в §14 текст «G-06/M-10/M-11/C-04 остались
partial из-за отсутствующих `create_invite`/`pins_list`/`threads_list`» устарел — команды есть,
приняты и лежат в `beta`. Целевой статус этих строк — `implemented (live unverified)`.
`GAP-BACKLOG.md` обновлён 20:59, счётчики 22/39/30 против 23/41/27 — расхождение остаётся.
`waves/STATE.md` — 2026-10-23, волна RW-40 в нём не отражена.

Это зона Team Lead; воркер эти файлы не правит.

---

## 7. Блокеры и находки

| ID | Уровень | Суть | Кто закрывает |
|---|---|---|---|
| **F-1** | — | IPC-имена UI ↔ Rust из снимка -12 | **закрыт**: `#34` INT-05 + `#35` INT-06, на `beta` 38/38 |
| **F-2** | — | дыра оракула `every_ui_invoke_command…` | закрыта частично: task-109 добавил `invokeReaction` и константы, но не `invoker` (§3.5) |
| **F-3** | P2 | оракул не видит `invoker(` — 3 команды прошли мимо | G1 + G4 |
| **F-4** | P1 | `dm_create`, `add_friend`, `get_reaction_users` не зарегистрированы в `main.rs` (`unstable`/`testing`/дерево) | G1 |
| **F-5** | P1 | unit красный: `FORWARDED_KINDS` UI не совпадает с проводом ядра | G2 + G1 |
| **F-6** | P1 | meta красный: `plan-sync` — 7 тестовых файлов без §8; PLAT-02 — ignored-набор keyring | G4 + G1 |
| **F-7** | P1 | зелёный gate отсутствует на текущих байтах (последний — 21:39, после него 86 файлов) | DevOps/Integration |
| **F-8** | P2 | `AT-03 ACCEPT.md` ссылается на несуществующий `attachments-render.js`, «23 теста» вместо 11, нет `mutations/mutation-summary.json` | PM |
| **F-9** | P2 | принятые MD5 (`chat.js`, `members.js`) не совпадают с деревом — приёмки привязаны к `beta` | G6/PM |
| **F-10** | P2 | матрица и бэклог не отражают INT-05/INT-06; `STATE.md` от 23 октября | Team Lead |
| **F-11** | P3 | 11 зарегистрированных, но не вызываемых команд — мёртвая поверхность | G1 |
| **F-12** | P3 | в рабочем дереве 50 `#[tauri::command]` против 49 регистраций (`main.rs` в правке) | G1 |

---

## 8. Чек-лист готовности к слиянию

`MERGE-POLICY.md` §2 (Definition of Ready) + `TEST-POLICY.md` §9 (DoD).

| Пункт | `beta` / BUILD-004 | Текущее дерево |
|---|---|---|
| Контракт IPC заморожен, аргументы сходятся | ✅ 38/38 | ❌ 38/41 (F-4) |
| RED-логи по всем срезам | ✅ | ✅ |
| GREEN + targeted | ✅ | ✅ по срезам, ❌ по дереву (F-5) |
| Мутации 5/5, MD5 восстановлены | ✅ | ✅ |
| Нет `.only/.skip/.todo/@ts-ignore` | ✅ 0 | ✅ 0 |
| Диф тестов проверен не автором | ✅ | ✅ |
| `gate:worker` passed на финальных байтах | ✅ (21:39, до релиза) | ❌ (F-7) |
| Unit | ✅ 977/977 (QA retest) | ❌ 1175/1176 |
| Meta | ✅ 102/102 | ❌ 96/102 |
| Typecheck / ui-tsc | ✅ 0 / 0 | ✅ 0 / 0 |
| Smoke | ✅ 22/22 (gate 21:39) | не прогонялся |
| QA-приёмка сборки | ✅ QA Passed (retest) | — |
| Границы файлов | ✅ нарушений не найдено | ⚠ незакоммиченная работа |
| Документы (матрица, бэклог, STATE) | ⚠ F-10 | ⚠ F-10 |
| Приёмочные артефакты соответствуют коду | ⚠ F-8 | ⚠ F-8/F-9 |

Итог: **`beta` — 12 ✅, 2 ⚠; дерево — 7 ✅, 5 ❌, 3 ⚠.**

---

## 9. Что делать по порядку

1. **G1 — закрыть F-4:** добавить `#[tauri::command]` для `dm_create`, `add_friend`,
   `get_reaction_users` (по образцу `pins_list`: `token_or`, typed errors, без токена из
   фронтенда), зарегистрировать в `generate_handler!`, покрыть `wiring_tests`.
2. **G1/G4 — закрыть F-3:** оракул должен обходить `ui/**/*.js` и ловить любой идентификатор
   `*invoke*(`, а не фиксированный список шести префиксов. Иначе следующая команда снова
   проскочит.
3. **G2/G1 — F-5:** синхронизировать `FORWARDED_KINDS` с проводом `dispatch.rs` (red-first).
4. **G4 — F-6:** добавить 7 тестовых файлов в `TDD-PLAN.md §8`; привести ignored-набор keyring
   к ожидаемым 12 или обновить контракт PLAT-02.
5. **DevOps — F-7:** после фиксов один `npm run gate:worker` на текущих байтах, артефакт в
   `.gate/`, затем коммит состава сборки (сейчас 1077 грязных путей).
6. **PM — F-8/F-9:** поправить `AT-03 ACCEPT.md` (путь `chat.js`, 11 тестов, убрать ссылку на
   `mutations/mutation-summary.json`), зафиксировать, что приёмки RW-40 привязаны к `beta`.
7. **Team Lead — F-10:** обновить `PARITY-MATRIX.md` §14 (M-10/M-11/C-04/G-06/AT-01/MB-03),
   `GAP-BACKLOG.md`, `waves/STATE.md`, `waves/db.jsonl`.

---

## 10. Границы прохода

- `cargo build/test/clippy` не запускались (запрет ТЗ). Rust-состояние подтверждается только
  сохранёнными логами и MD5; независимого прогона нет.
- `tests/smoke/gui-window.smoke.test.mjs` не запускался: требует дисплея и спавнит окно.
  Зелёный smoke подтверждён артефактом gate 21:39 (22/22) и логами QA retest.
- Живых сетевых вызовов (READY, отправка, вложения, реакции) не было — E3 по-прежнему ни у
  одной строки нет, `implemented` в матрице означает «offline + тесты».
- Ветки `unstable`/`testing`/`beta` читались через `git show`/`git ls-tree`, без checkout и без
  записи во временные каталоги.

---

## 11. Вердикт

**`beta` / BUILD-004 — MERGE: GO.** Паритет IPC **38/38 (100 %)**, пять имён из задания
закрыты и проверены по исходникам, TDD-цепочка по всем четырём срезам полная (red → green →
5/5 мутаций → gate), QA Passed. Замечания F-8…F-10 — трассировка и документы, не код.

**Текущее дерево / `unstable` / `testing` — MERGE: BLOCK.** Паритет **38/41**, три команды
отсутствуют; unit `1175/1176`, meta `96/102`; зелёного gate на текущих байтах нет. Причина —
новый блок DM/Friends/Reactions/UserPopout, который добавил UI-вызовы без Rust-обёрток и
проскочил мимо оракула из-за имени `invoker(`.

Формулировка «100 % UI → Rust IPC» верна для релизной сборки и неверна для верхушки `unstable`.
Это главный результат прохода.
