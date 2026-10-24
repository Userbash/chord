# G6-REPARITY-03 — пересчёт parity после L-02-UI, C-02-BEFORE и QA-findings

Дата: 2026-10-24, снимок **15:42–15:52 +05**. HEAD `0239543`, дерево **dirty**.
Воркер: **G6 Parity** (read-only). PM: DeepSeek.
Режим: единственная запись — этот файл. Продукт (`rust/**`, `ui/**`), тесты, `scripts/**`,
гейт, e2e, браузер, CDP, сеть, Discord, live, `cargo`, `npm`, `node --test`, мутации —
**не запускались и не менялись**. `PARITY-MATRIX.md`, `GAP-BACKLOG.md`,
`QUALITY/BUGS.md`, `QUALITY/QA-MATRIX-02.md`, `MERGE-MANIFEST.md`, `TASKS.md`, `QUEUE.md`,
`STATE.md`, `waves/db.jsonl`, `TDD-PLAN.md` — **не** менялись.

> **Путь.** В ТЗ указаны корневые `PARITY/`, `PM/`, `QUALITY/`. Фактическое дерево —
> `waves/RW-40/{PARITY,PM,QUALITY}`; отчёт записан туда. `QUALITY/SEC-L02.md` лежит
> в корне (`/QUALITY/SEC-L02.md`), в `waves/RW-40/QUALITY/` его нет.

Метки evidence: **[V]** — проверено в этом проходе (чтение/grep/md5/jq/awk);
**[R]** — из отчёта карточки, лога writer'а, гейта или матрицы. Ни один live-факт
здесь не заявляется.

> **Снимок двухслойный — дерево двигалось во время прохода.** На 15:42 в нём были
> L-02-UI и C-02-BEFORE, но не было фиксов Codex. В 15:42–15:49 три writer'а
> приземлили: `FIX-L02-CORE` (дренаж op0 перед ошибкой кадра + оракул `token`,
> red/green-логи), `SEARCH-FIX` (`data-guild-id`, regression + report),
> `QA-GATE-SKIPS` (`failOnSkips`, 17 meta-тестов) и Codex написал
> `PM/l02-ui/REVIEW.md` = **Return**. Всё изложенное ниже — состояние **на 15:52**;
> md5 в §8 — снимок, не фиксация.

---

## 0. Резюме

1. **Ни одна строка матрицы не сдвинулась.** Появились два среза (L-02-UI — 37 тестов,
   C-02-BEFORE — 12 тестов) и три фикса, но **ни один не принят**, а гейт
   `10:22:26Z` (15:22 local) старше всего этого. Счётчики: **91 строка,
   implemented 21 / partial 36 / absent 34 / unknown 0**; P0 5 / P1 32 / P2 40 / P3 14 —
   пересчёт независимым awk-оракулом по самому markdown (§3.15).
2. **Strict-recheck: 15 / 42 / 34** (те же 6 понижений: `C-02`, `C-07`, `M-06`, `M-07`,
   `M-13`, `G-01`). Пограничная трассировка (`G-01`/`C-02` = `implemented`, остатки —
   отдельные `G-01b`/`M-01`) даёт **17 / 40 / 34**. Обе сходятся к 91.
3. **L-02-UI: `RETURN`.** Codex `REVIEW-L02-UI` (15:42) вернул с двумя P2:
   (а) пустой READY оставляет устаревшие плитки в рейле (`projectReady` возвращает `[]`,
   `rail.applyGuilds([])` не вызывается); (б) числовой `user.id` теряет точность до
   `String()` (unsafe integer на IPC-границе). 37/37 и 347/347 подтверждены ревьюером.
   Дополнительно: `tests/unit/dispatch_ui.test.mjs` даёт **16 ошибок** проекта `tsc`
   (`npm run typecheck` включает `tests/**`) — узкий прогон `ui/tsconfig.json`
   (`include: ["app.js"]`) этот файл не покрывает (`QA-GATE-SKIPS.md` §2, 15:46) [R].
4. **L-02-CORE: фикс приземлился и задокументирован отдельно, приёмки нет.**
   P1(a) «потеря op0 при ошибке в том же фрагменте» **исправлен кодом** (`ws.rs` дренирует
   пайп до `parsed?`), P2 «оракул `token`» **закрыт тестом+мутантом**. `FIX-REPORT.md`
   (15:45) фиксирует актуальные md5 (`8703c1d6…`, `7f23d85a…`, `fe8aa921…`) и честную
   границу (Zstd-ошибка невозможна после разобранного op0; покрыт `Frame`). Нет:
   `ACCEPT.md`, гейта, повторного review.
5. **C-02: backend `before`/`latest` готов (не принят), UI older — нет.**
   `data.rs` `a99647c1…` = отчёт, 12 тестов, gui-spike 281. Но `chat.js` **не передаёт**
   `before`/`position:"latest"`/`olderCursor` (grep 0) → «старее» и открытие на последнем
   сообщении в приложении недостижимы.
6. **P-03: UI готов offline, входного пути нет** — `FORWARDED_DISPATCHES` = 6,
   `TYPING_START` отсутствует (`net.rs:113-120`).
7. **G-05 — 2/10**; **M-07** — `flags`/badge не проецируются; **G-01b** — `absent`
   (`UiSettings` ровно 5 полей).
8. **M-13 (search) — фикс приземлился**: `search-overlay.js` читает `data-guild-id`
   лениво, regression на реальном `index.html` (2 теста, red 0/2 → green 2/2),
   targeted 139/139; приёмки нет.
9. **Гейт на текущем дереве красный по двум фазам** (по логам writer'ов, не по прогону G6):
   (а) `typecheck` — 16 ошибок `tsc` в `tests/unit/dispatch_ui.test.mjs` [R];
   (б) `meta` — `plan-sync.test.mjs`: `TDD-PLAN.md §8` не описывает `dispatch_ui.test.mjs`
   и `search-remote-real-index.test.mjs` (`QA-GATE-SKIPS-meta.log`, 15:45) [R].
   Обе правки книжные/тестовые, продукт не затронут, но **merge без них невозможен**.
10. **Вердикт: `deferred`** для parity; **отдельный статус `gate-red`** для merge (§6).
    Не `parity-ok`, не `block` по продукту. Блокеры — §5.

---

## 1. Точный статус карточек

Классы: `returned` — ревью вернуло; `ready-for-review` — код+тесты, ревью идёт;
`ready-for-accept` — код+тесты+логи, нет ACCEPT; `partial`/`absent` — как в матрице.

### 1.1 Семь карточек ТЗ

| ID | Класс | Что доказано (evidence) | Чего нет | Что переводит дальше |
|---|---|---|---|---|
| **L-02-CORE** | **`ready-for-accept` (после фикса)** | дренаж **до** распространения ошибки: `let parsed = pipe.push_binary(...); flush_dispatches(...); let effects = parsed?` (`ws.rs:653-655`) **[V]**; новый тест `dispatch_handler_gets_op0_parsed_before_frame_error_in_same_fragment`; red `fix-red-core-flush.log` (FAILED 0/1) → green `fix-green-core-targeted.log` (14 passed) + `fix-green-net-contract.log` (23 passed), fmt/clippy чисто [R]; оракул `token`/`access_token`/`refresh_token` + вложенный `token`, мутант краснеет [R]; `FIX-REPORT.md` (15:45) с актуальными md5 и границей (`Zstd` до разбора невозможен, покрыт `Frame`) **[V]** | `PM/l02-core/ACCEPT.md`; гейт на дереве; повторный review после правки; `REPORT.md §6.4` остаётся снимком приёмки L-02 (см. §5.1) | ACCEPT со ссылкой на `FIX-REPORT §4` → gate |
| **L-02-UI** | **`returned`** | 3 модуля, подписка `gateway://dispatch`, 6 kind, `data-dispatch-source`, 15 счётчиков без payload; **37/37** новых + **347/347** targeted [R]; md5 8/8 = дерево **[V]**; red-first подтверждён ревьюером | **P2-1**: пустой READY → `rail.applyGuilds([])` не вызывается, старые плитки живут; **P2-2**: `user.id` как `number` → `String()` без проверки safe range; **`tsc` проекта**: 16 ошибок в `tests/unit/dispatch_ui.test.mjs` [R] | `L-02-UI-FIX`: 2 red-first теста («непустой → пустой READY», unsafe integer) + типизация теста → re-review |
| **C-02-BE (older)** | **`ready-for-accept`** | `messages_page(channelId, cursor?, before?, position?, limit?)`; `hasOlder`/`olderCursor`; `position:"latest"`; взаимоисключения → `InvalidArg`; 12 тестов на фикстурах; gui-spike **281**; md5 `data.rs`/`main.rs`/теста = отчёт **[V]** | `ACCEPT.md`; live-REST history (`?before=`) не реализован и не снят | приёмка G1; затем `data.rs`-очередь (M-07 проекция) |
| **C-02-UI (older)** | **`absent`** | forward-пагинация C-02-PAGE (8 тестов) жива; prepend/якорь есть | `chat.js` не вызывает `messages_page` с `before`/`latest`, `olderCursor` не читается **[V]** | `C-02-UI-OLDER` (S): `latest` при open, `before` на верхнем крае |
| **P-03** | **`partial`** | исходящий `send_typing`, «Печатает…», TTL 10 с, троттлинг 8 с `[A]`, guard ≤5, 401/429/slowmode; **20** тестов | входной `TYPING_START` не форвардится (`FORWARDED_DISPATCHES` = 6) **[V]**; `ACCEPT.md`; live | `TYPING_START-FORWARD` (G1 `net.rs` + `net_contract`) → G2-маппинг → приёмка |
| **G-05** | **`partial` (2/10)** | поповер/ARIA/клавиатура/leave-confirm; активны `guild-leave`, `guild-copy-id`; invite disabled по capability | 8 пунктов — заглушки; спека rev.3 не accepted (9 дефектов); capability/`ownerId` из READY; RE-2 | G3 добивает спеку → G2 фиксы; 10/10 offline недостижимо без waiver |
| **M-07** | **`partial`** | ack на open/focus/scroll, дедуп + 3 с, `last_viewed`, `token:null`; `rest_ack` (9) + `read_state_contract` (12) + `read-state-ui` (8) | `data.rs` отдаёт ровно 6 полей и **исключает** `badge_count`/`last_acked_id`/`read_state_type` (`:514-528`) **[V]**; unread без упоминаний, badge, G-08/C-03 не работают | G1 badge-проекция (аддитивно, оракул READY) → G2 UI |
| **G-01b** | **`absent`** | — | `UiSettings` = `version/toggles/density/theme/locale` (ровно 5) **[V]**; `lastGuildId` нет ни в Rust, ни в UI (только комментарий) **[V]** | G1 поле+миграция+round-trip → G2 restore |

### 1.2 Два фикса, приземлившиеся в этом проходе (вне ТЗ-списка)

| ID | Класс | Что доказано | Чего нет |
|---|---|---|---|
| **SEARCH-FIX** (QA-M2-03 / M-13) | **`ready-for-accept`** | `selectedGuildId()` читает `data-guild-id`, legacy `dataset.guild` — только если он есть; `getGuildId` ленивый (boot с пустым рейлом не замораживает id); regression на **реальном** `index.html`+`app.js` (шим-плитки удалены): red 0/2 → green 2/2, targeted **139/139**, tsc по своим файлам чист [R] | ACCEPT; браузерный e2e сценария 3 (QA-M2-02); запись в `BUGS.md` (R-01/M-13) |
| **QA-GATE-SKIPS** (QA-M2-01) | **`ready-for-accept`, но meta/typecheck красные** | `failOnSkips` в `scripts/gate.mjs` (+`minPass`, `--test-concurrency=1`, fixture-набор без `live-*`), e2e = unmet при `pass 0`; live-фаза отдельная (`passed_with_unmet`); `attempts[]` сохраняет оба прогона; **17** новых meta-тестов зелёные (red 5/17 → green 17/17); `QA-GATE-SKIPS.md` (15:46) с диффом и таблицей «было/стало» [R] | ACCEPT; **`TDD-PLAN.md §8` не содержит 2 новых unit-файлов** → `plan-sync` падает; 16 ошибок `tsc` в чужом `dispatch_ui.test.mjs`; гейт на дереве |

**Сводка:** закрытых (accepted) карточек — **0 из 9**. L-02-UI — `returned` (2 P2 + tsc),
L-02-CORE — `ready-for-accept` после фикса, L-02-UI/G-05/M-07/P-03 — `partial`,
C-02-UI/G-01b — `absent`; два фикса (SEARCH-FIX, QA-GATE-SKIPS) ждут ACCEPT.

---

## 2. Что фактически изменилось после `G6-REPARITY-02` (15:29)

| # | Факт | Evidence | Влияние на матрицу |
|---:|---|---|---|
| 1 | **L-02-UI записан** (15:35–15:37): 3 модуля, подписка, 6 проекций, 37 тестов | md5 8/8 = отчёт **[V]** | строки **не двигает**; меняет *содержание* gap у L-02, M-06, P-01/P-02, MB-01 |
| 2 | **C-02-BEFORE записан** (15:28–15:31): `before`/`position`/`hasOlder`, 12 тестов, 281 gui | md5 3/3 = отчёт **[V]** | C-02 остаётся `partial`: нет UI-ветки |
| 3 | **Codex REVIEW-L02-CORE = RETURN** (15:29) → **фикс** (15:42–15:45) | `REVIEW.md`, `fix-red-core-flush.log`, `fix-green-*` [V/R] | P1(a)/P2 закрыты кодом; ACCEPT нет |
| 4 | **Codex REVIEW-L02-UI = RETURN** (15:42), 2 P2 | `PM/l02-ui/REVIEW.md` **[V]** | L-02-UI из `ready-for-review` → `returned`; L-02 `partial` |
| 5 | **SEARCH-FIX** (15:44–15:45): `data-guild-id`, regression 2 теста | `PM/search-fix/REPORT.md` **[V]** | M-13 false-green закрыт offline; строка ждёт приёмки |
| 6 | **QA-GATE-SKIPS** (15:43–15:45): `failOnSkips`, split live, оба retry-лога | `scripts/gate.mjs`, 17 meta **[V]** | e2e-skip больше не «pass» в новом гейте; старый JSON не переснят |
| 7 | **SEC-L02 опубликован** (корень `QUALITY/`): P0 0 / **P1 1** / P2 7 / P3 3 | **[V]** | новый независимый P1: дедуп `s` на reconnect (`net.rs` не менялся) |
| 8 | **QA-MATRIX-02**: M2 **не принят**, 7 P1 (QA-M2-01…07) | **[V]** | из них QA-M2-01 и QA-M2-03 закрыты кодом в этом проходе |
| 9 | **POOL-02/QUEUE/ORCA-WORKERS** (15:41): 3 writer'а | **[V]** | процессный риск: приёмка по md5 «плывёт» |
| 10 | **`typecheck` красный на дереве**: 16 ошибок `tsc` в `tests/unit/dispatch_ui.test.mjs` (root `tsconfig` включает `tests/**`) | `QA-GATE-SKIPS.md` §2 [R] | следующий `gate:worker` падает на `typecheck` (L-02-UI-зона) |
| 11 | **`meta` красный на дереве**: `plan-sync` не видит 2 новых тест-файла | `QA-GATE-SKIPS-meta.log` [R] | следующий `gate:worker` падает и на `meta` (правка `TDD-PLAN §8`, TL) |
| 12 | **`FIX-REPORT.md` (15:45)** документирует правки L-02-CORE с актуальными md5 | `PM/l02-core/FIX-REPORT.md` **[V]** | P1/P2 доказаны; ACCEPT по-прежнему нет |
| 13 | **`QA-GATE-SKIPS.md` (15:46)** — отчёт с диффом и таблицей было/стало | `QUALITY/QA-GATE-SKIPS.md` **[V]** | карточка `ready-for-accept`; чужие красные честно помечены |
| 14 | **Дрейф md5 тестов**: `typing-ui` `896ae41c…`→`1a5f4de2…`; `read-state-ui` `80f4db2b…`→`f9d6e653…`; `channel-paging` `5d98e960…`→`54184e41…`; `actions.js` `908bfe2d…` не совпадает ни с `g05-ui`, ни с `m07-ui` | md5 **[V]** | P2: правки без своего red/green-лога |

**Вывод по §2:** три блокера из четырёх (Codex P1, e2e-skip, M-13 false-green) **сдвинуты
кодом**, но **ни один не закрыт приёмкой**. Одновременно дерево стало **непроходным для
гейта**: `typecheck` (16 ошибок в чужом тест-файле) и `meta` (`plan-sync`). Матрица обязана
остаться на 21/36/34; любая запись `implemented` для L-02/C-02/M-13 сейчас была бы выдачей
offline за паритет.

---

## 3. Матрица 91 функции (снимок 15:52)

Колонки: **Матрица** — состояние `G6-SYNC-01` (14:34), владелец PM;
**Recheck-03** — strict-класс этого прохода; **Δ** — изменение класса относительно
`G6-REPARITY-02`. `impl*` = implemented offline, E3 нет; `impl*↓` = de facto partial (§4);
`↓` — понижение strict-класса. У строк без Δ изменилось только **содержание остатка**.

### 3.1 Auth / session (9)

| ID | Функция | P | Матрица | Recheck-03 | Δ | Evidence / остаток |
|---|---|---|---|---|---|---|
| A-01 | Вход по логину/паролю | P0 | implemented (live unverified) | impl* | — | live-вход N0-01 не выполнен → E3 нет |
| A-02 | MFA TOTP/SMS/backup/password | P1 | implemented (live unverified) | impl* | — | 5 методов offline; live-прогона нет |
| A-03 | Captcha-гейт | P1 | implemented (offline) | impl* | — | live sitekey/flow `[A]` |
| A-04 | QR / one-time login | P2 | absent (UI disabled) | absent | — | протокол/поллинг не сняты |
| A-05 | Passkey / WebAuthn | P2 | absent (UI disabled) | absent | — | Linux-WebAuthn нет |
| A-06 | Восстановление сессии | P1 | implemented (keyring `[A]`, live unverified) | impl* | — | live restore/битый keyring не проверены |
| A-07 | Logout / инвалидация | P1 | implemented (live unverified) | impl* | — | live stop/401 после logout нет |
| A-08 | Регистрация/сброс пароля | P3 | absent | absent | — | решение PM: вне desktop scope |
| A-09 | Сессии / logout others | P2 | absent | absent | — | нет экрана и вызова |

### 3.2 Guilds (9)

| ID | Функция | P | Матрица | Recheck-03 | Δ | Evidence / остаток |
|---|---|---|---|---|---|---|
| G-01 | Рейл: список/выбор/переключение | P1 | implemented (live unverified) | **partial↓** | — | READY 7 гильдий + `guild-rail` (8); `UiSettings` без `last_guild_id` → **G-01b absent [V]**; L-02-UI P2-1 (пустой READY) касается этого же пути |
| G-02 | Добавить/создать сервер | P3 | absent | absent | — | endpoint `GET\|POST /guilds` `[A]` |
| G-03 | Вход по инвайту | P1 | implemented (live unverified) | impl* | — | `invites_contract` (14) + `join-server` (24); live-коды/TTL `[A]` |
| G-03a | Войти из preview/lurker | P1 | implemented (live unverified) | impl* | — | backend `[L]` 204×2; live 204 из UI не снят |
| G-04 | Выход с сервера | P1 | implemented (live unverified) | impl* | — | confirm + `{guildId, lurking}`; live-коды `[A]` |
| G-05 | Меню сервера | P1 | partial: **активны 2 из 10** | partial | — | спека rev.3 (не accepted) = 9 дефектов; invite `capability=false` **[V]** |
| G-06 | Инвайты: создать/список/отозвать | P2 | absent | absent | — | API/UI нет; коды `[A]` |
| G-07 | Роли/каналы/права сервера | P3 | absent | absent | — | design-first |
| G-08 | Unread/mention сервера | P2 | partial: статичный `is-unread` | partial | — | зависит от M-07 `flags`/badge — не проецируются **[V]** |

### 3.3 Channels / navigation (6)

| ID | Функция | P | Матрица | Recheck-03 | Δ | Evidence / остаток |
|---|---|---|---|---|---|---|
| C-01 | Дерево: категории, text/voice/forum | P1 | partial: дерево из READY | partial | — | типы 0/2/4/5/15; нет open voice/forum, collapse |
| C-02 | Переключение канала + история | P1 | implemented (live unverified) | **partial↓** | — | **backend older/latest готов** (`data.rs` `a99647c1…`, 12 тестов) **[V]**; **UI-ветки нет**: `chat.js` не шлёт `before`/`latest` **[V]**; live-истории нет |
| C-03 | Unread/mention канала | P2 | partial: статичный класс | partial | — | нет mention/ack-счётчика (M-07) |
| C-04 | Шапка: пины/ветки/участники/поиск | P2 | partial | partial | — | поиск/участники есть; pins/threads stub |
| C-05 | Создание/редактирование канала | P3 | absent | absent | — | UI не снят |
| C-07 | QuickSwitcher `Ctrl+K` | P2 | implemented (B6) | **partial↓** | — | свитчер адресует плитки, но не вызывает `openChannel`/`messages_page`; live-каналы не связаны |

### 3.4 Messages (14)

| ID | Функция | P | Матрица | Recheck-03 | Δ | Evidence / остаток |
|---|---|---|---|---|---|---|
| M-01 | История/пагинация | P1 | partial: fixtures + PageStore | partial | — | forward-paging есть; **older/tail UI нет**; live-истории/курсоров нет |
| M-02 | Отправка | P0 | implemented (live unverified) | impl* | — | live send/401/429 → окно N0-02 |
| M-03 | Редактирование | P1 | implemented (live unverified) | impl* | — | тело edit `[A]` |
| M-04 | Удаление | P1 | implemented (live unverified) | impl* | — | live 403/tombstone не проверены |
| M-05 | Ответ (reply) | P1 | implemented (live unverified) | impl* | — | `message_reference`; live-поле `[A]` |
| M-06 | Реакции | P1 | implemented (live unverified) | **partial↓** | — | pills/исходящие на реальных id; **входящий путь есть offline** (L-02-UI `applyReaction`; ADD/REMOVE в `FORWARDED_DISPATCHES`) **[V]**, но L-02-UI возвращён; live PUT/DELETE/429 нет; REMOVE `[A]` |
| M-07 | Read state / ack | P1 | implemented (live unverified) | **partial↓** | — | ack/`lastViewed` есть; `data.rs` **исключает** `badge_count`/`read_state_type` **[V]** |
| M-08 | Markdown/embeds/mentions | P1 | partial: только embeds | partial | — | нет markdown/mentions/emoji |
| M-09 | Контекстное меню (15) | P2 | partial: 4 команды | partial | — | forward/bookmark/reminder/TTS/report stub |
| M-10 | Пины | P2 | absent (stub) | absent | — | нет панели/загрузки |
| M-11 | Ветки | P2 | absent (stub) | absent | — | нет списка/создания |
| M-12 | Forward/bookmark/reminder/TTS/report | P3 | absent | absent | — | endpoints `[A]` |
| M-13 | Поиск по серверу | P1 | implemented (4/7, live unverified) | **partial↓** | — | **false-green закрыт offline** (`SEARCH-FIX`, 2 regression на реальном `index.html`, 139 targeted) **[V]**; приёмки нет; live-поиска нет |
| M-14 | Эмодзи-пикер | P2 | implemented (static) | impl* | — | статичный набор; кастомные/стикеры нет |

### 3.5 Members (4)

| ID | Функция | P | Матрица | Recheck-03 | Δ | Evidence / остаток |
|---|---|---|---|---|---|---|
| MB-01 | Список: роли + статусы | P1 | partial: 24k synthetic | partial | — | L-02-UI даёт live-строки панели (`data-user-id`), но **групп 6 нет**, `#member-list` не live [R]; live-членов/сортировки нет |
| MB-02 | Роли/цвета/права | P2 | partial: synthetic | partial | — | live-ролей/цветов нет |
| MB-03 | Профиль-поповер | P2 | partial: stub aria | partial | — | нет поповера/полей |
| MB-04 | Полный поиск участников | P2 | absent (deferred) | absent | — | RE-4 request/response |

### 3.6 Presence / typing (5)

| ID | Функция | P | Матрица | Recheck-03 | Δ | Evidence / остаток |
|---|---|---|---|---|---|---|
| P-01 | Статусы online/idle/dnd/offline | P1 | partial: synthetic + core | partial | — | `PRESENCE_UPDATE` проекция в панель есть offline (L-02-UI, LWW по `ts`); live-статусов нет |
| P-02 | Обновления PRESENCE_UPDATE | P1 | partial: reducer, UI не связан | partial | — | **UI-подписчик появился offline**, но L-02-UI возвращён; live нет |
| P-03 | Индикатор набора + POST /typing | P1 | partial: backend есть, UI отсутствует | partial | — | **UI есть offline** (20 тестов, `send_typing`); **`TYPING_START` не форвардится** → live-индикатор невозможен |
| P-04 | Кастомный статус/активность | P2 | absent | absent | — | нет редактора/отображения |
| P-05 | Смена своего статуса | P2 | absent | absent | — | нет меню/IDENTIFY/UPDATE |

### 3.7 Voice (11)

| ID | Функция | P | Матрица | Recheck-03 | Δ | Evidence / остаток |
|---|---|---|---|---|---|---|
| V-01 | Вход/выход из voice | P1 | absent (crate only) | absent | — | нет session manager/UI; G5 пишет контракт |
| V-02 | RTC-сигнализация | P1 | partial: парсер/тесты | partial | — | нет live-WS/сессии |
| V-03 | UDP discovery/keepalive | P1 | partial: примитивы | partial | — | нет live-UDP |
| V-04 | SRTP | P1 | partial: KDF/AEAD тесты | partial | — | нет live encrypt/decrypt |
| V-05 | Opus/джиттер/DTX | P1 | partial: jitter, нет Opus | partial | — | нет encode/decode |
| V-06 | Статистика/reconnect/регионы | P2 | partial: policy/stats | partial | — | нет live RTT/потерь |
| V-07 | Устройства/mute/deafen/PTT | P1 | absent (settings fake) | absent | — | нет устройств/эффекта |
| V-08 | Голосовой UI | P1 | partial: панель на fake store | partial | — | реального voice-backend нет (V-01…V-05) |
| V-09 | Шумоподавление Krisp | P2 | absent | absent | — | нет модели/интеграции |
| V-10 | DAVE/MLS E2EE | P2 | absent | absent | — | нет MLS |
| V-11 | Voice messages/clips/soundboard | P3 | absent | absent | — | контракт не снят |

### 3.8 Video / Go Live (4)

| ID | Функция | P | Матрица | Recheck-03 | Δ | Evidence / остаток |
|---|---|---|---|---|---|---|
| S-01 | Камера: preview/enable | P2 | absent | absent | — | нет захвата/превью |
| S-02 | Screen share / Go Live | P2 | absent | absent | — | нет capture/сигнализации (RE-3) |
| S-03 | Зрители/качество/simulcast | P2 | absent | absent | — | нет RTP-видео/декода |
| S-04 | Activities | P3 | absent | absent | — | контракт запуска `[A]` |

### 3.9 Attachments (3)

| ID | Функция | P | Матрица | Recheck-03 | Δ | Evidence / остаток |
|---|---|---|---|---|---|---|
| AT-01 | Загрузка файла | P2 | absent (кнопка disabled) | absent | — | **blocked**: метод/поля/лимиты (RE-1) |
| AT-02 | Drag&drop/paste | P2 | absent | absent | — | нет триггеров |
| AT-03 | Рендер вложений | P2 | absent (только embeds) | absent | — | нет схемы attachment |

### 3.10 Settings (8)

| ID | Функция | P | Матрица | Recheck-03 | Δ | Evidence / остаток |
|---|---|---|---|---|---|---|
| ST-01 | Навигация настроек | P2 | partial: 18 разделов, 3 панели | partial | — | 63 DOM-строки; подпункты 17 разделов `[A]` |
| ST-02 | Голос/видео: 38 тумблеров | P2 | partial: нет эффекта | partial | — | устройств/аудио нет |
| ST-03 | Внешний вид: тема/плотность/zoom | P2 | partial | partial | — | zoom/полного light нет |
| ST-04 | Язык/время + i18n-рантайм | P2 | partial: crate не подключён | partial | — | нет `t()`/каталогов в UI |
| ST-05 | Уведомления: настройки | P2 | partial: панель-заглушка | partial | — | нет полей/эффекта |
| ST-06 | Горячие клавиши | P3 | absent | absent | — | нет редактора |
| ST-07 | Аккаунт/профиль | P3 | absent | absent | — | поля не сняты |
| ST-08 | Персистентность/синхронизация | P2 | partial: локальный файл `[A]` | partial | — | нет settings-proto sync; **5 полей, `version:1`** **[V]** |

### 3.11 Notifications / DND (5)

| ID | Функция | P | Матрица | Recheck-03 | Δ | Evidence / остаток |
|---|---|---|---|---|---|---|
| N-01 | Системные уведомления | P2 | partial: D-Bus crate не подключён | partial | — | нет вызова из UI |
| N-02 | DND/режим концентрации | P2 | partial | partial | — | нет подавления |
| N-03 | Inbox/упоминания | P2 | absent (кнопка disabled) | absent | — | UI/счётчики не сняты |
| N-04 | Бейдж/прогресс | P3 | absent | absent | — | нет Unity badge |
| N-05 | Переопределения сервер/канал | P2 | absent | absent | — | нет меню/уровней |

### 3.12 Platform (8)

| ID | Функция | P | Матрица | Recheck-03 | Δ | Evidence / остаток |
|---|---|---|---|---|---|---|
| PL-01 | Трей/фон | P2 | absent | absent | — | нет трея |
| PL-02 | Автозапуск/старт свёрнутым | P2 | partial: модель `.desktop` | partial | — | нет записи/UI |
| PL-03 | Deep links + single instance | P2 | partial: парсер `discord://` | partial | — | нет wiring/second-instance |
| PL-04 | Апдейтер | P3 | partial: manifest/plan | partial | — | нет скачивания/замены |
| PL-05 | Spellcheck | P3 | partial: hunspell FFI | partial | — | нет UI/словарей |
| PL-06 | Питание/idle | P2 | partial: power/idle crate | partial | — | порог idle `[A]` |
| PL-07 | Consent/разрешения захвата | P3 | absent | absent | — | consent-flow не снят |
| PL-08 | Окно: badge/progress/flash/PiP | P3 | absent | absent | — | нет вызовов |

### 3.13 Live / connectivity (5)

| ID | Функция | P | Матрица | Recheck-03 | Δ | Evidence / остаток |
|---|---|---|---|---|---|---|
| L-01 | Reconnect/offline state в UI | P1 | implemented (live unverified) | impl* | — | `#net-status` + retry; live resume/4013/4014 нет |
| L-02 | Dispatch → UI | P1 | partial: reducer/core, UI на fixtures | partial | — | **core: фикс приземлился, ACCEPT нет; UI: `returned`** (§1) → `partial` |
| P0-01 | Live READY | P0 | partial (offline) | partial | — | нет E3 |
| P0-02 | Live send | P0 | partial (offline) | partial | — | нет E3 |
| P0-03 | Секреты не в логах | P0 | implemented (offline) | impl* | — | live-грепа token/password/key нет; SEC-L02 подтверждает redaction по ключам **[V]** |

### 3.14 Сходимость

| Класс | Матрица | Strict recheck-03 | Δ |
|---|---:|---:|---:|
| verified (E3) | 0 | **0** | 0 |
| implemented (offline) | 21 | **15** | −6 |
| partial | 36 | **42** | +6 |
| absent | 34 | **34** | 0 |
| unknown | 0 | **0** | 0 |
| **Итого** | **91** | **91** | 0 |

Пограничные: `G-01` и `C-02` — если остатки трассируются отдельными карточками
(`G-01b`, `M-01`), класс остаётся `implemented`, раскладка **17 / 40 / 34**. Обе сходятся к 91.

### 3.15 Независимый оракул (markdown, не реализация)

```bash
cd waves/RW-40/PARITY
sed 's/\\|/~/g' PARITY-MATRIX.md | awk -F'|' '
/^\| [A-Z][A-Z0-9]*-[0-9]/ { n=NF; app=$6; prio=$(n-3);
  gsub(/^ +| +$/,"",app); gsub(/^ +| +$/,"",prio); sub(/[ (:].*/,"",app);
  a[app]++; p[prio]++; t++ }
END { for (k in a) print "app",k,a[k]; for (k in p) print "prio",k,p[k]; print "total",t }'
```

Фактический вывод (15:44, этот проход):

```text
app absent 34
app partial 36
app implemented 21
prio P0 5 / P1 32 / P2 40 / P3 14
total 91
```

Сверка ID: 91/91, без пропусков и дублей; домены:
`A 9 · G 9 · C 6 · M 14 · MB 4 · P 5 · V 11 · S 4 · AT 3 · ST 8 · N 5 · PL 8 · L 2 · P0 3`.

---

## 4. Шесть `implemented`, которые не end-to-end (без изменений)

Понижение не меняет матрицу (владелец — PM); это флаг «функция + тесты» ≠ end-to-end.
Новых «зелёных» строк в этом проходе **нет**; у M-13 и M-06 закрыт offline-путь, но
приёмки нет — класс держится.

| ID | Почему не end-to-end | Evidence | Что вернёт `implemented` |
|---|---|---|---|
| **C-02** | backend older/latest готов, но UI листает **только вперёд**; открытия на последнем сообщении нет | `data.rs` `a99647c1…` **[V]**; `chat.js` без `before`/`latest` **[V]** | `C-02-UI-OLDER` + приёмка + live |
| **C-07** | свитчер не вызывает `openChannel` → канал не грузит историю; live-каналы не связаны | `quick-switcher.js`, `app.js` **[V]** | `openChannel` из свитчера + тест |
| **M-06** | входящий путь появился offline, но L-02-UI **возвращён**; live PUT/DELETE/429 нет; REMOVE `[A]` | ADD/REMOVE в `FORWARDED_DISPATCHES` **[V]**; Return **[V]** | L-02-UI-FIX + приёмка + live-реакции |
| **M-07** | `data.rs` отдаёт `flags`, UI читает только `mentionCount`; badge-поля исключены намеренно | `data.rs:514-528` **[V]** | G1 badge-проекция + G2 UI |
| **M-13** | false-green закрыт offline-фиксом, но не принят и не подтверждён браузером | `SEARCH-FIX` 2/2 regression, red 0/2 **[V]** | ACCEPT + browser-e2e сценария 3 |
| **G-01** | `lastGuildId` не сохраняется — после рестарта выбор теряется | `settings_store.rs` 5 полей **[V]** | `G-01b` (поле + миграция + restore-тест) |

Не понижались: `M-14` (статичный пикер — это и есть функция), `A-03`/`A-06` (`[A]`-оговорки),
`P0-03` (offline-redaction — сама функция), `L-01` (UI реализован, live только подтверждение).

---

## 5. Блокеры (обновление на 15:52)

### 5.1 Codex P1 — L-02-CORE

| # | Finding | Статус на 15:52 | Evidence |
|---|---|---|---|
| P1-a | Уже разобранные op0 теряются при ошибке в том же WS-фрагменте | **ИСПРАВЛЕН КОДОМ, не принят** | `ws.rs:653-655`: `let parsed = pipe.push_binary(...); flush_dispatches(...); let effects = parsed?;` **[V]**; тест `dispatch_handler_gets_op0_parsed_before_frame_error_in_same_fragment`; red `fix-red-core-flush.log` (0 passed / 1 failed) → green `fix-green-core-targeted.log` (14 passed) [R] |
| P1-b | md5 теста в отчёте не соответствовал проверяемому артефакту | **ЗАКРЫТ (документ + отдельный отчёт)** | `FIX-REPORT.md §4` (15:45): `ws.rs` `8703c1d6…`, `gateway_supervisor.rs` `7f23d85a…`, `net_contract.rs` `fe8aa921…` = дерево **[V]**; `REPORT.md §6.4` явно помечен как снимок приёмки L-02 |
| P2 | Оракул redaction не проверял точный `token` | **ИСПРАВЛЕН КОДОМ, не принят** | `net_contract.rs:358-371` — `token` + `access_token` + `refresh_token`; вложенный `token` в фикстуре; мутант `fix-red-oracle-token-mutant.log` (FAILED) → `fix-green-net-contract.log` (23 passed) [R/V] |
| — | `ACCEPT.md` отсутствует | **ОТКРЫТ** | `find waves/RW-40/PM -name ACCEPT.md` → только `c02-be`, `l02` **[V]** |
| — | Гейт на финальном дереве | **ОТКРЫТ** | worker `10:22:26Z` старше `ws.rs` 15:43, `gateway_supervisor.rs` 15:42, `net_contract.rs` 15:42 |

**Итог по §5.1:** содержательно Codex P1/P2 **закрыты** и задокументированы в
`FIX-REPORT.md` (актуальные md5 + честная граница по `Zstd`). Формально карточка не принята:
нет `ACCEPT.md`, нет review после правки, нет гейта на дереве.

**Дополнительно (не Codex, SEC-L02):**

| ID | Finding | Статус | Почему важно |
|---|---|---|---|
| SEC-L02-01 | **P1**: дедуп `s` не сбрасывается на новом `connect` → диспатчи новой сессии уходят в `duplicates` | **ОТКРЫТ** | `net.rs` md5 `e13d4e8c…` не менялся **[V]**; после relogin/reconnect UI не получает READY/списки |
| SEC-L02-02/03 | P2: сырой `d` в `Debug`; READY `d` в webview целиком (PII, ~579 КБ) | **ОТКРЫТ** | тип-барьер и проекция не вводились |
| SEC-L02-04/05 | P2: backpressure дропает READY; op0 до ошибки кадра без счётчика | **частично**: дренаж сделан (P1-a), счётчик потерь — нет | закрывать одним срезом с SEC-L02-01 |

### 5.2 e2e skip — `skip` больше не pass, но гейт на дереве красный

| Проверка | Факт 15:52 | Статус |
|---|---|---|
| `failOnSkips` в гейте | **реализован**: `failOnSkips` + `minPass`, `skipped > 0 ⇒ phase failed`; fixture-набор без `live-*`, live — только `qualify` с `passed_with_unmet` [R] | **код есть, ACCEPT нет** |
| e2e-счётчики | `pass 0 / skip 2` (fixture) → **unmet**, не pass [R] | **код есть** |
| **`typecheck`** | **16 ошибок `tsc`** в `tests/unit/dispatch_ui.test.mjs`; `npm run typecheck` = `tsc -p tsconfig.json` (включает `tests/**`) **[V]** | **КРАСНЫЙ на дереве** |
| **`meta`** | `plan-sync`: `TDD-PLAN.md §8` не описывает `dispatch_ui.test.mjs` и `search-remote-real-index.test.mjs`; 76/77 [R] | **КРАСНЫЙ на дереве** |
| Браузер | `CHROME_BIN`/`CHROME_PATH` unset, `chromium/chrome/brave` в `PATH` нет [R] | **ОТКРЫТ** |
| 7 M2-сценариев | `tests/e2e/m2-*.e2e.test.mjs`, `helpers/ui-fixture.mjs`, `fixtures/ui/**`, `meta/e2e-required.test.mjs` нет **[V]** | **ОТКРЫТ** |
| CI | `.github/workflows/ci.yml` — 2 Rust-job, ни одного Node/browser [R] | **ОТКРЫТ** |
| Impact selector | FRONTEND не содержит `dispatch_ui`, `typing-ui`, `read-state-ui`, `join-server`, `channel-paging`, `chat-reactions` **[V]** | **ОТКРЫТ** (QA-M2-05) |
| Старый release-гейт | `09:37:37Z`: e2e `0 pass / 4 skip`, verdict `passed_with_skips` | **историческое**; новый гейт не снимался |

> Гейт `unit` запускает `tests/unit/**/*.test.mjs` целиком (`gate.mjs:242`), поэтому
> `dispatch_ui` и `search-remote-real-index` попадут в **следующий** прогон. Дыра —
> в impact-селекторе (targeted-набор), не в полном unit.
> **Важно:** `ui/tsconfig.json` содержит `include: ["app.js"]` — узкий прогон L-02-UI
> не покрывал `tests/unit/dispatch_ui.test.mjs`; «tsc чисто» в отчёте карточки
> относилось к другому набору файлов **[V]**.

### 5.3 live E3 — `E3 = 0 из 91`

| Блок | Строки | Минимум | Статус |
|---|---|---|---|
| N0 (1 окно) | P0-01, P0-02, P0-03 | READY + guild/channel в логе; сообщение видно в канале; 401/429; `grep token/password/key = 0` | **не выполнено** |
| N0 (то же окно) | A-01…A-07, M-02…M-07, M-13, C-02/M-01, G-01, L-01, L-02 | вход/MFA/капча, send/edit/delete/reply, реакции, ack, поиск, история, READY-рейл, resume, «событие дошло до окна» | **не выполнено** |
| N1 (после RE-2) | G-03, G-03a, G-04, G-06 | live-коды/TTL, 204 из UI, owner-leave, invite CRUD | **не выполнено** |
| Voice / Stream / меню / presence | V-01…V-08, S-02/S-03, G-05, MB-01/02, P-01/02 | 2 клиента, свой микрофон, реальные права | **не выполнено** |

`implemented (live unverified)` без окна — это **не** приёмка PM и **не** `parity-ok`.

### 5.4 G6-D-03 — переформулирован, evidence в `BUGS.md` не исправлен

| Пункт | Факт 15:52 | Статус |
|---|---|---|
| `ready` | **вызывается**: `READY_COMMAND = "ready"`, `invoke` в `app.js:600` **[V]** | **из дефекта убрать** |
| `enter_guild` | UI-глагол `join_guild` — алиас той же команды (`Q-01 closed D-82`) **[V]** | **из дефекта убрать** |
| `members_page` | **0 вызовов** из UI **[V]** | **ОТКРЫТ** (вызов экрана участников + DOM-тест) |
| `search_page` | **0 вызовов** (поиск локальный/`search_remote`) **[V]** | **ОТКРЫТ** (решение PM: локальный vs backend) |
| `settings` (как команда) | **0**; используются `settings_load`/`settings_save` **[V]** | **ОТКРЫТ** (решение PM: статические секции vs backend-команда) |
| `net_dispatch_stats` | **0 `invoke`**, но статистика **отдаётся** через `report_ui_snapshot.dispatch` (`app.js:1481`) **[V]** | **evidence исправить**: ноль invoke ≠ нет наблюдаемости |
| evidence в `QUALITY/BUGS.md` | строка G6-D-03 всё ещё перечисляет `ready`/`enter_guild` как «не вызываются» **[V]** | **ОТКРЫТ (документ)** — правка PM/TL |

**DoD D-03:** (а) `enter_guild` — documented alias + тест, из дефекта убрать;
(б) `members_page` — вызов + DOM-тест; (в) `search_page` — решение PM + тест;
(г) `settings` — решение PM + тест; (д) evidence переписать по факту.
Закрытие **не** «одним L-02»: L-02 закрывает только `ready`/часть данных.

### 5.5 Процессные блокеры (обострились)

| # | Блокер | Evidence | Владелец |
|---:|---|---|---|
| 1 | **`typecheck` красный на дереве**: 16 ошибок `tsc` в `tests/unit/dispatch_ui.test.mjs` | `QA-GATE-SKIPS.md` §2 [R] | G2 (`L-02-UI-FIX`) |
| 2 | **`meta` красный на дереве**: `plan-sync` не находит 2 новых тест-файла в `TDD-PLAN.md §8` | `QA-GATE-SKIPS-meta.log` 15:45: `unlisted = [dispatch_ui, search-remote-real-index]` [R] | TL (`TDD-PLAN.md`) |
| 3 | **Нет ACCEPT ни у одной из восьми карточек** (l02-core, l02-ui, c02-before, p03-ui, g05-ui, m07-ui, search-fix, gate-skips) | `find … ACCEPT.md` → 2 файла из 10 каталогов **[V]** | PM |
| 4 | **Гейт не описывает дерево**: `10:22:26Z` старше `ws.rs` 15:43, `data.rs` 15:28, `main.rs` 15:29, `chat.js` 15:35, `dispatch_*` 15:35–15:37, `app.js` 15:44, `gate.mjs` 15:44 | mtime/gate JSON **[V]** | TL |
| 5 | **Три writer'а одновременно** + дерево dirty → приёмка по md5 «плывёт» | `ORCA-WORKERS.md` 15:41 **[V]** | PM |
| 6 | **Дрейф md5 тестов без своего red/green** (4 файла, §2.14) | md5 **[V]** | G4 |
| 7 | `waves/db.jsonl` — **0 событий RW-40**; `STATE.md` от 2026-10-23 | grep **[V]** | TL |
| 8 | `MERGE-MANIFEST` §4 устарел: «P-03 typing UI отсутствует», «C-02 — только первая страница», L-02/search-fix нет | `MERGE-MANIFEST.md:56-67` **[V]** | PM |

---

## 6. Вердикт

### `deferred` для parity + отдельный статус **`gate-red`** для merge

| Вопрос | Ответ |
|---|---|
| parity-вердикт | **`deferred`** — ни одна строка не `parity-ok`, ни одна не ухудшилась |
| можно ли merge сейчас | **нет: `gate-red`** — на текущем дереве красные `typecheck` и `meta` |
| продуктовый block | **нет** — P0-дефектов и вымысла не найдено |

**Почему не `parity-ok`:**
1. **E3 = 0 из 91.** Ни одна строка не подтверждена live; окна N0 не было.
2. **L-02 не end-to-end:** core ждёт ACCEPT после фикса, UI **возвращён** ревьюером
   (2 P2 + tsc). Это строка, от которой зависят M-06/P-01/P-02/P-03/MB-01.
3. **Восемь срезов лежат непринятыми** (L-02-CORE, L-02-UI, C-02-BEFORE, P-03-UI,
   G-05-UI, M-07-UI, SEARCH-FIX, QA-GATE-SKIPS); статус кода опережает приёмку на 4–6 карточек.
4. **6 `implemented` de facto partial** (C-02, C-07, M-06, M-07, M-13, G-01).
5. **G-05 — 2/10**, спека не accepted, 9 дефектов; 10/10 offline недостижимо без waiver.
6. **e2e — 0 pass / 4 skip** в последнем release, браузера нет, 7 M2-сценариев нет;
   `failOnSkips` появился, но не принят и не проверен на полном гейте.
7. **P0-01/02/03 live не сняты.**

**Почему не `block` (по продукту):**
- **P0-дефектов в коде нет**: SEC-L02 — **P0 0**, redaction по ключам корректен;
  вымышленных endpoint/полей нет (G6-BATCH-MERGE §3 PASS), `ROUTES=49`, frozen TSV цел.
- **Обе красные фазы — не продукт**: `typecheck` падает на типизации нового теста
  (не на `ui/**`-коде), `meta` — на отсутствии двух строк в `TDD-PLAN.md §8`.
  Правки локальны, поведение не меняют, 17 новых meta-тестов зелёные.
- Offline-путь и тесты есть: unit 651, workspace 1104 pass + 12 skip, smoke 22 (гейт 15:22);
  новые срезы: 37 + 12 + 20 + 2 + 17 тестов с red-логами.

> **Честная оговорка о критерии.** В `G6-REPARITY-02` §5 `block` определён в том числе как
> «красный гейт на текущем дереве». Если PM применяет это буквально, статус merge —
> **`block`**, пока `typecheck`/`meta` не зелёные. Здесь он вынесен в отдельный `gate-red`,
> потому что parity-картина (91 строка, E3, продуктовые дефекты) от этих двух фаз
> не меняется. Решение о ярлыке — за PM; факт один: **на этом дереве `gate:worker`
> вернёт `failed`, а не `passed`**.

### Что переводит вердикт

| Цель | Условия (все обязательны) |
|---|---|
| → `gate-green` | `L-02-UI-FIX` (2 P2 + tsc) **и** `TDD-PLAN §8` (+2 файла) **и** свежий `gate:worker` на финальном дереве |
| → `parity-ok` | `gate-green` **и** L-02-CORE accepted **и** L-02-UI accepted **и** `gate:release` с `verdict === "passed"`, `e2e.pass ≥ 9 / skipped = 0` **и** E3 по N0 **и** G-05 10/10 или письменный waiver **и** M-07/C-02/G-01b закрыты |
| → `block` | **продуктовый** красный гейт (unit/rust/smoke), регресс P0, утечка секретов, вымышленный endpoint/поле, расхождение матрицы и кода больше чем на класс |

**Оценка до `parity-ok`:** 3 fix/accept-карточки trust-контура (§7 п.1–3) + 2 parity-карточки
(§7 п.4–5) + `G-01b`/`M-07-UNREAD` + **1 release-гейт** + **1 live-окно** + **1 browser-фаза**.

---

## 7. Следующие 5 задач по WSJF для пула

Формула устава §5.2: `(BV + TC + RR) / JS`, шкала 1–5. Правило очереди (`QUEUE.md` §1):
**P1A (доверие) раньше P1B (паритет)**; внутри класса — WSJF. Поэтому «порядок запуска»
ниже — с учётом зависимостей, а не только числа.

| # | Карточка | Класс | BV | TC | RR | JS | **WSJF** | Owner / размер | Файлы (монополия) | Зависит | DoD (что **не** pass) |
|---:|---|---|---:|---:|---:|---:|---:|---|---|---|---|
| 1 | **L-02-UI-FIX** (2 P2 из REVIEW + `tsc` теста) | P1A | 4 | 5 | 3 | 2 | **6.0** | G2, S | `ui/features/dispatch-model.js`, `ui/features/dispatch-view.js`, `ui/app.js`, `tests/unit/dispatch_ui.test.mjs` | REVIEW-L02-UI 15:42; `tsc` 16 ошибок | red: «непустой READY → пустой READY очищает рейл» и «unsafe integer id не принимается» падают; green: `applyGuilds([])` на пустом snapshot, id только строкой/safe-range; `npm run typecheck` по файлу = 0 ошибок; затем re-review. Не pass: правка теста под текущее поведение |
| 2 | **ACCEPT-L02-CORE** (ACCEPT + повторный review + gate) | P1A | 4 | 5 | 3 | 1 | **12.0** | G1+Codex+PM, S | `PM/l02-core/ACCEPT.md`, без правок кода | `FIX-REPORT.md` 15:45 | ACCEPT со ссылкой на `FIX-REPORT §4` (md5 `8703c1d6…`/`7f23d85a…`/`fe8aa921…`); Codex подтверждает P1/P2; `gate:worker` после 15:45. Не pass: ACCEPT по md5 из `REPORT §6.4` |
| 3 | **META-PLAN-SYNC** (`TDD-PLAN.md §8`: +2 файла) | P1A | 3 | 5 | 4 | 1 | **12.0** | TL, S | `TDD-PLAN.md` (shared, TL) | meta-лог 15:45 | `plan-sync` зелёный: `dispatch_ui.test.mjs` и `search-remote-real-index.test.mjs` описаны ровно по разу, модули/токены существуют. Не pass: «гейт зелёный, потому что meta не запускали» |
| 4 | **TYPING_START-FORWARD** (P-03 end-to-end) | P1B | 4 | 3 | 2 | 1 | **9.0** | G1+G2, S | `net.rs` (`FORWARDED_DISPATCHES`), `net_contract.rs`; затем `app.js`/`typing-indicator.js` | п.2 (net.rs свободен); `app.js` — после п.1 | `[L]`-кадр как оракул → `typing.start` → индикатор зажигается из события; P-03 → `implemented (live unverified)`. Не pass: fake-событие вместо форварда |
| 5 | **C-02-UI-OLDER** (tail-open + `before`-скролл) | P1B | 4 | 4 | 2 | 2 | **5.0** | G2, S | `ui/features/chat.js`, `tests/unit/channel-paging.test.mjs` | C-02-BEFORE принят; `chat.js` свободен | open → `{position:"latest"}`, верхний край → `{before: olderCursor}`, prepend сохраняет `scrollTop`, `hasOlder=false` останавливает. Не pass: `nextCursor` вместо `olderCursor` |

**Следующие за пятёркой:**
`ACCEPT-SEARCH-FIX` (S, 12.0 по той же арифметике — PM/Codex, идёт параллельно) →
`G-01b` (S, 6.0: BV3/TC2/RR1/JS1) → `M-07-UNREAD` (M, 3.0) → `QA-M2-02` browser harness (L, 3.0) →
`G-05-REST` (M, 5.0, ждёт спеку G3) → `PLAT-11a` (S, P2, ready).

**Оговорки к §7:**
1. Пункты 2–3 — **не код** (отчёт/план): не занимают продуктовый слот и не конфликтуют
   с writer'ами. Их можно запускать немедленно.
2. `app.js` — общая точка: `SEARCH-FIX` (записан, ждёт приёмки), `L-02-UI-FIX` (п.1),
   `TYPING_START` (п.4, вторая половина), `G-01b-UI`, `C-07`-фикс. Только последовательно.
3. `dispatch-model.js`/`dispatch-view.js` — монополия `L-02-UI-FIX`; `chat.js` — только
   `C-02-UI-OLDER` (M-07-UI и L-02-UI уже записали свои версии).
4. `data.rs` — после C-02-BEFORE свободен; следующая карточка на нём — `M-07` проекция
   `flags`/badge (G1), не параллелить с `PLAT-11a` (`main.rs`).
5. `scripts/gate.mjs` — зона TL/G4; правка гейта не должна менять пороги тестов.
6. Любая карточка стартует с red-first и **не** считается pass по `verdict`; только по `counts`.

---

## 8. Снимок md5 (15:52, только чтение)

| Файл | md5 | Сверка |
|---|---|---|
| `rust/crates/discord-core/src/ws.rs` | `8703c1d695cd0af26458393eb3c1e3f9` | = `FIX-REPORT §4`; ≠ `REPORT §6.4` (`176791cc…`) — снимок приёмки L-02 |
| `rust/crates/discord-core/src/gateway/supervisor.rs` | `5180b63795b2eee3d017cb30324760e6` | = `REPORT`/`FIX-REPORT` |
| `rust/crates/discord-core/tests/gateway_supervisor.rs` | `7f23d85a4d3b0e54c58d765b8b879c58` | = `FIX-REPORT §4` (не `84f5fcc1…` из `REPORT §6.4`) |
| `rust/gui-spike/src/net.rs` | `e13d4e8cb4fbb22af2ce6d96702f0e8a` | = отчёт; SEC-L02-01 открыт |
| `rust/gui-spike/tests/net_contract.rs` | `fe8aa92146e9dcf8b943335dfad652c8` | = `FIX-REPORT §4` (фикс P2) |
| `rust/gui-spike/src/data.rs` | `a99647c19a4cfbdd7e1640506ea2e650` | = C-02-BEFORE |
| `rust/gui-spike/src/main.rs` | `336863b5b1ffca169b851a2095798f01` | = C-02-BEFORE (не `c003a9e5…` из l02-core) |
| `rust/gui-spike/tests/history_before_contract.rs` | `796293ea6ae985dcb17dfd96a61bf1a1` | = C-02-BEFORE |
| `rust/gui-spike/ui/app.js` | `c79d741461f24fbf9c768d1463c96795` | = SEARCH-FIX (не `16411a31…` из L-02-UI) → **дрейф** |
| `rust/gui-spike/ui/features/chat.js` | `2e918f3cff5dc968ca3117480e306ce4` | = L-02-UI |
| `rust/gui-spike/ui/features/actions.js` | `908bfe2dbf4202cdf28c389d44b2095f` | ≠ `g05-ui`/`m07-ui` → **дрейф** |
| `rust/gui-spike/ui/features/search-overlay.js` | `44b34c0f9642188f54fd96c29afdf7a4` | = SEARCH-FIX |
| `dispatch-model.js` | `154eec1272f9f320c0fb79ce21feb63d` | = L-02-UI |
| `dispatch-view.js` | `fd1842ac442dae108f80b3872f163f05` | = L-02-UI |
| `dispatch-bridge.js` | `1cad6f1324b2fe508866d29097792443` | = L-02-UI |
| `typing-indicator.js` | `09f9f53870b669a382d52ecadcabb799` | = P-03-UI |
| `tests/unit/dispatch_ui.test.mjs` | `611876033058344cc6b5a48303b91eae` | = L-02-UI |
| `tests/unit/search-remote-real-index.test.mjs` | `182f6199561e8c6334db68a904526788` | = SEARCH-FIX |
| `tests/unit/typing-ui.test.mjs` | `1a5f4de2b3ae8a4d040786840e172e69` | ≠ `896ae41c…` → **дрейф** |
| `tests/unit/read-state-ui.test.mjs` | `f9d6e653c31e5c8c1ab60ce04352f7d2` | ≠ `80f4db2b…` → **дрейф** |
| `tests/unit/channel-paging.test.mjs` | `54184e41ce632d3fdb47cec9672095e0` | ≠ `5d98e960…` → **дрейф** |
| `scripts/gate.mjs` | `89c67ad0a8f087b87626090b2b5899d9` | = QA-GATE-SKIPS (не `c8a4875e…` из QA-MATRIX) |

Гейты: worker `10:22:26Z` (meta 65 / unit 651 / workspace 1104+12 / smoke 22, **без** e2e,
coverage, keyring); release `09:37:37Z` (e2e **0 pass / 4 skip**, keyring-live 12/12, unit 643).
Ни один не покрывает дерево 15:28–15:52. **Следующий `gate:worker` на этом дереве даст
`failed` минимум на двух фазах: `typecheck` (16 ошибок `tsc` в `dispatch_ui.test.mjs`)
и `meta` (`plan-sync` без 2 строк в `TDD-PLAN §8`)** [R]. Обе — не продукт (§6).

---

## 9. Ограничения и что этот отчёт НЕ доказывает

1. **Не запускались** `cargo`, `npm`, `node --test`, гейт, e2e, браузер, CDP, сеть,
   Discord, live, мутации. Все числа тестов — из `.gate/*.json`, отчётов и логов writer'ов.
2. **Снимок нестабилен:** три writer'а (§0), дерево dirty, `rust/gui-spike/**` и
   `discord-core/src/gateway/**` вне git — контроль только по md5. Файлы менялись
   в 15:42–15:49, т.е. **во время** прохода; §8 — состояние на 15:52.
3. `implemented` здесь **не** live-паритет: E3 = 0.
4. `[M]`-строки (V-08 speaking, G-05-заглушки, ST-01 подпункты, N-03) паритетом не считаются;
   `[A]`-строки (TTL typing, live-коды invite/leave, порог idle, consent) — авторские
   решения, не RE-факты.
5. Отчёт **не** закрывает карточки, волну или waiver; `PARITY-MATRIX.md`,
   `GAP-BACKLOG.md`, `QUALITY/BUGS.md`, `TASKS.md`, `QUEUE.md`, `STATE.md`, `db.jsonl`,
   `MERGE-MANIFEST.md`, `TDD-PLAN.md` не изменялись (зона PM/TL).
6. Успешность фиксов L-02-CORE/SEARCH-FIX/QA-GATE-SKIPS принята **по логам writer'ов**,
   не по собственному прогону; red-логи проверены на факт падения, не на «правильную
   причину» в каждом assert.
7. Вердикт `REVIEW-L02-UI` — из файла 15:42; если ревьюер обновит его после фикса,
   класс L-02-UI меняется без изменения матрицы (строка остаётся `partial` до live).
8. Статус **`gate-red`** — вывод из двух чужих логов (`QA-GATE-SKIPS-meta.log`,
   `QA-GATE-SKIPS.md §2`), а **не** из прогона гейта: G6 гейт не запускает. Точный
   вердикт следующего `gate:worker` будет известен только после его выполнения на
   остановленном дереве.
9. `C-02-BE` и `SEARCH-FIX` md5 совпали с отчётами, но их тесты G6 не запускал:
   «12/12» и «2/2» — по логам карточек. Это приёмка evidence, не воспроизведение.
