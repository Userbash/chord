# MERGE-READINESS-08 — 12 ACCEPT-файлов, закрытый P1 SEC-L02-01, сверка md5

Дата: **2026-10-30**, снимок **17:05–17:12 +05**. Ветка `main`, HEAD
`4e240481e59ff4d49732ac8bd5ba08a006363748` (integration commit RW-40 от 2026-10-24 16:22).
Воркер: **G6 Parity**, отдел G6, карточки **G6-PARITY-SYNC-08** + **GAP-BACKLOG-SYNC**.
Режим — **read-only по коду и тестам**.

> **Вход прохода.** `MERGE-READINESS-07.md`, `PARITY-MATRIX.md`, `GAP-BACKLOG.md`; ACCEPT Wave 3–4:
> `PM/sec-l02-01/ACCEPT.md`, `PM/b7b-search/ACCEPT.md`, `PM/m07-ui-badge/ACCEPT.md`; отчёты фиксов:
> `PM/b2a-attach/REPORT-FIX.md`, `PM/g01b-ui/REPORT-FIX.md`, `PM/stub14-search-ui/REPORT-FIX.md`;
> отчёты `PM/stub16-profile-ui/REPORT.md`, `PM/stub27-dispatch/REPORT.md`; `DESIGN/INVITES-SPEC.md`;
> `QUALITY/TEST-INTEGRITY-WAVE3.md`. Логи: `QUALITY/meta-after-wave{3,4}.log`,
> `QUALITY/tsc-after-wave{3,4}.log`.

Запреты ТЗ соблюдены: продуктовый код и тесты не менялись, `cargo` не запускался, `git commit`
не делался. Дополнительно не запускались `node --test`, `npx tsc`, `npm run gate:*`, e2e, браузер,
CDP, сеть, Discord, live. Запускались `md5sum`, `git status/diff/log`, `stat`, `grep`/`awk`.
Метки: **[V]** — измерено в этом проходе, **[R]** — из отчёта/лога карточки.

---

## 0. Резюме

1. **Вердикт: merge — `block`. Parity — `deferred`.** Из двух блокеров MERGE-READINESS-07
   остался один: **нет gate-артефакта на текущих байтах** (последний `.gate/worker-*.json` —
   commit `0239543`, 2026-10-24 10:22) и **нет freeze**. Второй — **P1 `SEC-L02-01`** —
   **закрыт** (§3.3) **[V]**.
2. **Accepted set: 12 ACCEPT-файлов** вместо 9. Три новых: `SEC-L02-01`, `B7b-SEARCH-FILTERS-BACKEND`,
   `M-07-UI-BADGE`. Два из двенадцати лежат **в корне репозитория**, а не в `waves/RW-40/`:
   `PM/g01b-contract/ACCEPT.md` и `QUALITY/QA-M2-04-ACCEPT.md` (§2.1) **[V]**.
3. **22 md5-пина в ACCEPT, 19 совпали.** Три расхождения — `net_contract.rs`, `app.js`, `chat.js`
   (§2.2). `chat.js` официально перекрыт строкой «supersedes» в ACCEPT M-07-UI-BADGE; `net_contract.rs`
   перекрыт более поздним ACCEPT SEC-L02-01 (но без «superseded by» в L-02-CORE); `app.js`
   по-прежнему **не покрыт** приёмкой (§2.2). Шесть log-пинов новых ACCEPT совпали **[V]**.
4. **Wave 4 сдана, но не принята:** b2a TIER-FIX, G-01b-UI SETTINGS-FIX, STUB-14 FIX, STUB-16,
   STUB-27-DISPATCH — `ACCEPT.md` нет ни у одной. Сверх ТЗ-списка то же у `b2b-attach-ui`,
   `g05-rest`, `v01-rtc`, `stub27-core`. «Сдано» ≠ «принято» **[V]**.
5. **Дерево активно двигается во время прохода:** dirty `196 → 206` за 3 минуты, `composer.test.mjs`
   переписан в 17:10:31, `session.rs` — 17:12:06, `composer.js` — 17:12:28. У трёх карточек без
   ACCEPT байты уже **разошлись** со своими отчётами (§2.4) **[V]**.
6. **meta зелёный:** `QUALITY/meta-after-wave4.log` — 102/102, `plan-sync` ✔ (17:05:40).
   Это закрывает F1 аудита `TEST-INTEGRITY-WAVE3`. `tsc` — 0 Б, exit-код не записан (§3.2) **[V]**.
7. **Матрица и бэклог синхронизированы одним проходом:** 7 строк, §14 = **22 implemented /
   37 partial / 32 absent / 0 unknown, всего 91**; независимый `awk` по markdown даёт те же
   числа, бэклог содержит те же 91 ID ровно по одному разу (§5) **[V]**.

---

## 1. Снимок дерева

| Параметр | Значение | Метка |
|---|---|---|
| HEAD | `4e240481e59ff4d49732ac8bd5ba08a006363748` (2026-10-24 16:22) | [V] |
| Dirty, `git status --porcelain -uall` | **206** путей на 17:12:26; 196 на 17:09, 200 на 17:10, 201 на 17:10:24 | [V] |
| Дельта tracked | `41 files changed, 5334 insertions(+), 400 deletions(-)` | [V] |
| Последние записи кода | `composer.test.mjs` 17:10:31, `session.rs` 17:12:06, `composer.js` 17:12:28 | [V] |
| Предыдущее окно | `app.js` 16:59:07, `dispatch.rs` 17:00:35 | [V] |
| Свежие логи | `meta-after-wave4.log` и `tsc-after-wave4.log` — 17:05:40 | [V] |
| Журнал | `grep -c '"wave": "RW-40"' waves/db.jsonl` → **0** (exit 1) | [V] |
| Gate-артефакт | `.gate/worker-2026-10-24T10-22-26-400Z.json`, commit `0239543`, `passed_with_skips` | [V] |
| Release-артефакт | `.gate/release-2026-10-24T09-37-37-616Z.json`, commit `0239543`, e2e 4 skipped | [V] |

Рост dirty на 10 путей за 3 минуты — не «шум отчётов»: менялись продуктовые файлы
(`tests/unit/composer.test.mjs`, `rust/crates/discord-voice/src/session.rs`,
`rust/gui-spike/ui/features/composer.js`). Пока писатели не остановлены, любой md5 в §2 — точка,
а не фиксация **[V]**.

---

## 2. Сверка md5

### 2.1 Accepted set — 12 ACCEPT-файлов, 22 пина кода (+6 логов), 19 совпали

| # | ACCEPT (путь) | Срез | Пины | Совпало |
|---:|---|---|---:|---:|
| 1 | `PM/g01b-contract/ACCEPT.md` (корень) | G-01b-CONTRACT | 2 | 2 [V] |
| 2 | `waves/RW-40/PM/l02/ACCEPT.md` | L-02 (backend, 2026-10-24) | md5 в прозе, вытеснены | — [R] |
| 3 | `waves/RW-40/PM/l02-core/ACCEPT.md` | L-02-CORE | 3 | 2 [V] |
| 4 | `waves/RW-40/PM/l02-ui/ACCEPT.md` | L-02-UI + TSC-FIX | 3 | 2 [V] |
| 5 | `waves/RW-40/PM/plat-11a/ACCEPT.md` | PLAT-11a | 2 | 2 [V] |
| 6 | `waves/RW-40/PM/c02-ui-older/ACCEPT.md` | C-02-UI-OLDER | 2 | 1 [V] |
| 7 | `waves/RW-40/PM/c02-be/ACCEPT.md` | C-02-BE | 0 (принят прогоном) [R] | — |
| 8 | `waves/RW-40/PM/m07-be3/ACCEPT.md` | M-07-BE3 | 2 | 2 [V] |
| 9 | `waves/RW-40/PM/m07-ui-badge/ACCEPT.md` | M-07-UI-BADGE | 2 | 2 [V] |
| 10 | `waves/RW-40/PM/sec-l02-01/ACCEPT.md` | SEC-L02-01 | 2 кода + 3 лога | 5 [V] |
| 11 | `waves/RW-40/PM/b7b-search/ACCEPT.md` | B7b-SEARCH-FILTERS-BACKEND | 2 кода + 3 лога | 5 [V] |
| 12 | `QUALITY/QA-M2-04-ACCEPT.md` (корень) | QA-M2-04 | 2 (из `QA-M2-04-md5.txt`) | 2 [V] |

Детально по файлам (17:12:32) **[V]**:

| Файл (md5 из ACCEPT) | md5 в дереве | Итог |
|---|---|---|
| `settings_store.rs` `ac58e876…` | `ac58e876a6553e0d00cd881f30622b0b` | совпал |
| `settings_contract.rs` `0e943bb9…` | `0e943bb9a487d77718cb37b3338e4ea6` | совпал |
| `discord-core/src/ws.rs` `8703c1d6…` | `8703c1d695cd0af26458393eb3c1e3f9` | совпал |
| `discord-core/tests/gateway_supervisor.rs` `7f23d85a…` | `7f23d85a4d3b0e54c58d765b8b879c58` | совпал |
| `net_contract.rs` (L-02-CORE) `fe8aa921…` | `573d271204f3c1db601e2df73684f129` | **разошёлся** — SEC-L02-01 |
| `dispatch-model.js` `37dacc61…` | `37dacc61d7daf74c144681637dded1f5` | совпал |
| `ui/app.js` (L-02-UI) `1ac5c369…` | `70bcf5e7edca4b2c788508d226fe25df` | **разошёлся** — G-01b-UI-SETTINGS-FIX |
| `dispatch_ui.test.mjs` `d30771d6…` | `d30771d65b6373ecbbac6de1f9e61a3d` | совпал |
| `src/messages.rs` `c449a06c…` | `c449a06c783c795effb33c7fc763614e` | совпал |
| `tests/messages_contract.rs` `0f92348a…` | `0f92348a5e3e15563603faec0917c4b5` | совпал |
| `ui/features/chat.js` (C-02-UI-OLDER) `6444e9d9…` | `1e73c60219792da3c332b8cbc73c7607` | **разошёлся** — M-07-UI-BADGE |
| `channel-paging.test.mjs` `4ef50982…` | `4ef50982a5de2847765bd1bc64d8dedf` | совпал |
| `src/data.rs` `8013271d…` | `8013271de7a6444b2b9dba3301a165d4` | совпал |
| `tests/read_state_contract.rs` `fde688f3…` | `fde688f33614636c2699c55ebba945e9` | совпал |
| `.github/workflows/ci.yml` `608e2683…` | `608e26837c10996aae11d2c29fc6d320` | совпал |
| `tests/meta/ci-browser.test.mjs` `06ab44ee…` | `06ab44eee0c919777cbaec91756adaff` | совпал |
| `src/net.rs` `b286d177…` | `b286d1779fc89f1dd6c829dc3a32dba7` | совпал |
| `tests/net_contract.rs` (SEC-L02-01) `573d2712…` | `573d271204f3c1db601e2df73684f129` | совпал |
| `src/search.rs` `6bae3ea2…` | `6bae3ea23dfe31d1929fe3f7eb926fd0` | совпал |
| `tests/search_contract.rs` `a0200648…` | `a02006481e7e97ab26667367be8ce902` | совпал |
| `tests/read-state-ui.test.mjs` `fe363afe…` | `fe363afe9abcc82bc99e1e59b0af58c5` | совпал |
| `ui/features/chat.js` (M-07-UI-BADGE) `1e73c602…` | `1e73c60219792da3c332b8cbc73c7607` | совпал |

Логи новых ACCEPT: `sec-l02-01/red.log` `c0ff41c7…`, `green.log` `8f076d77…`, `mutations.log`
`47b949cc…`; `b7b-search/red.log` `ddc4263e…`, `green.log` `8f076d77…`, `mutations.log`
`a52aac38…` — все шесть совпали **[V]**. Совпадение md5 у двух `green.log` не ошибка: оба ACCEPT
ссылаются на один и тот же прогон двух таргетов `net_contract` + `search_contract` **[V]**.

`PM/l02/ACCEPT.md` (2026-10-24) держит md5 в прозе (`net.rs f21143f3…`, `main.rs 0082fb4b…`,
`net_contract.rs 0d0ff80d…`, `dispatch.md 58f678f9…`); все четыре вытеснены L-02-CORE/UI и
SEC-L02-01, строки «superseded by» в файле нет **[V]**. `PM/c02-be/ACCEPT.md` принимает не по md5 —
это записано в самом ACCEPT (§3) **[R]**.

### 2.2 Три расхождения и кто их закрывает

| Файл | Пин | Текущие байты | Кто переписал | Статус покрытия |
|---|---|---|---|---|
| `rust/gui-spike/tests/net_contract.rs` | L-02-CORE `fe8aa921…` | `573d2712…` | SEC-L02-01 (ACCEPT есть, `573d2712…`) | **покрыт** новым ACCEPT; в L-02-CORE строки «superseded by» нет |
| `rust/gui-spike/ui/features/chat.js` | C-02-UI-OLDER `6444e9d9…` | `1e73c602…` | M-07-UI-BADGE | **покрыт**: ACCEPT прямо пишет «supersedes `chat.js` md5 pin from `c02-ui-older/ACCEPT.md`» |
| `rust/gui-spike/ui/app.js` | L-02-UI `1ac5c369…` | `70bcf5e7…` | G-01b-UI-SETTINGS-FIX (REPORT-FIX, 16:59) | **не покрыт**: ACCEPT у карточки нет |

`app.js` менялся после выдачи ACCEPT уже дважды (G-01b-UI → G-01b-UI-SETTINGS-FIX). Это тот же
системный эффект, что описан в 07 §2.3: составной файл несёт 3–4 карточки, поэтому пин ломается
следующей карточкой в том же файле. Лечение одно — ACCEPT на G-01b-UI-SETTINGS-FIX с md5
`70bcf5e7…` **или** строка «supersedes pin L-02-UI»; до этого текущие байты `app.js` не приняты **[V]**.

### 2.3 Wave 4: сдано, ACCEPT нет — 14 пинов, все совпали на 17:12

| Карточка | Файл (md5 из отчёта) | md5 в дереве | Итог |
|---|---|---|---|
| **B2a TIER-FIX** | `src/attachments.rs` `e28b8cbc…` | `e28b8cbc5dfb2db2e14aecabbd61caf2` | совпал [V] |
| | `tests/attachments_contract.rs` `8563afd3…` | `8563afd362e5e3ba7f66b4f2e3704c02` | совпал [V] |
| | `src/lib.rs` `f8b5af61…` (не тронут) | `f8b5af6132cba13213d3a79031b8f976` | совпал [V] |
| **G-01b-UI SETTINGS-FIX** | `ui/features/settings.js` `040013d5…` | `040013d5212a3a0b8e50c045270144ec` | совпал [V] |
| | `ui/app.js` `70bcf5e7…` | `70bcf5e7edca4b2c788508d226fe25df` | совпал [V] |
| | `tests/unit/guild-rail.test.mjs` `8e44df12…` | `8e44df12e01f2fd8a831ec615e30a360` | совпал [V] |
| **STUB-14 FIX** | `ui/features/search.js` `b5c5c3e6…` | `b5c5c3e6538fa0f6c24c45c3ac1c36a0` | совпал [V] |
| | `ui/features/search-overlay.js` `860e9237…` | `860e9237a00e4eacd66e343bbeb5ab45` | совпал [V] |
| | `tests/unit/search-remote-real-index.test.mjs` `b2692ff1…` | `b2692ff1bb6b0fb107e998ac2f5b2c2d` | совпал [V] |
| **STUB-16 (MB-03)** | `ui/features/members.js` `3dea788f…` | `3dea788f3fedb22f8ad14a9306d56912` | совпал [V] |
| | `ui/components/members.css` `77e6b5e9…` | `77e6b5e9ee6ac23eeec6afff8370e8f2` | совпал [V] |
| | `tests/unit/gui-spike-regressions.test.mjs` `ca041626…` | `ca0416266f00b06208c6a2814c2aa1ca` | совпал [V] |
| **STUB-27-DISPATCH** | `discord-core/src/dispatch.rs` `528597e9…` | `528597e9f5aa4b894982cbd3f5cbda38` | совпал [V] |
| | `discord-core/tests/dispatch_types.rs` `af859f62…` | `af859f628901986aa523b01ea1c507b1` | совпал [V] |

Пять карточек из ТЗ-списка сданы чисто по md5, но приёмки нет — в приёмке считать их нельзя.
Остальные карточки волны (`b2b-attach-ui`, `g05-rest`, `v01-rtc`, `stub27-core`) тоже без ACCEPT;
`REVIEW.md` есть у всех четырёх, а также у `sec-l02-01`/`b7b-search` (F7 аудита закрыт) **[V]**.

### 2.4 Три пина без ACCEPT уже разошлись с отчётами

| Файл | В отчёте | В дереве (17:12) | Mtime |
|---|---|---|---|
| `tests/unit/composer.test.mjs` (b2b-attach-ui) | `03ae4054…` **[R]** | `a1739159…` **[V]** | 17:10:31 |
| `rust/crates/discord-voice/src/session.rs` (v01-rtc) | `030303ad…` **[R]** | `395df9d9…` **[V]** | 17:12:06 |
| `rust/crates/discord-voice/tests/session_contract.rs` (v01-rtc) | `1d6b0e2d…` **[R]** | `c59ae2de…` **[V]** | 17:12 |
| `rust/gui-spike/ui/features/composer.js` (b2b-attach-ui) | `911b3639…` **[R]** | `2b1d80a5…` **[V]** | 17:12:28 |

Это не дефект кода, а незакрытая работа: карточки правятся прямо во время прохода (у `composer.test.mjs`
число `@ts-ignore` уменьшилось с 5 до 3 — попытка лечить F3 аудита). Пока ACCEPT нет, md5 отчёта
к байтам не обязывает; после приёмки их надо перемерить **[V]**.

---

## 3. Свежие логи, `tsc` и закрытие P1

### 3.1 `QUALITY/meta-after-wave4.log` — 102/102, `plan-sync` зелёный

| Факт | Значение | Метка |
|---|---|---|
| Файл | 13 500 Б, mtime 17:05:40 | [V] |
| Итог | `tests 102 · suites 17 · pass 102 · fail 0 · cancelled 0 · skipped 0 · todo 0` | [V] |
| `plan-sync` | `✔ plan-sync: §8 ↔ тестовые файлы ↔ модули` (строка 94) | [V] |
| `waves-sync` | `✔` (журнал восстановим) | [V] |
| Предыдущий прогон | `meta-after-wave3.log` — 102/101/1, красный `plan-sync` (токены §8) | [V] |

F1 из `TEST-INTEGRITY-WAVE3` (красная фаза `meta`) закрыт правкой `TDD-PLAN.md §8`, а не тестов:
четыре токена (`initComposer`, `DISABLED_REASON_TEXT`, `initActions`, `leaveFailureText`) заменены
на фактические **[V]**/ **[R]**. Форма лога прежняя: нет строки команды и нет `exit=`; вердикт
восстановим по `fail 0` **[V]**.

### 3.2 `QUALITY/tsc-after-wave4.log` — 0 ошибок, exit-кода по-прежнему нет

| Факт | Значение | Метка |
|---|---|---|
| Размер | **0 байт**, mtime 17:05:40 (тот же прогон, что meta) | [V] |
| Интерпретация | пустой stdout+stderr у `tsc` = 0 ошибок | [V] |
| Пробел | нет строки команды и `exit=`; 0-байтовый файл неотличим от «не запускали» | [V] |

`TEST-INTEGRITY-WAVE3` F11 (две диагностики в `gui-spike-regressions.test.mjs` на 16:57) снят:
в 17:00 и 17:05 `tsc` чист **[V]**/ **[R]**. Но без `exit=` это evidence слабее meta-лога:
лечение то же, что в 07 §3.2 — одна строка `… ; echo "exit=$?" >> …` на замороженном дереве.

### 3.3 P1 `SEC-L02-01` — **закрыт**

| Что | Значение | Метка |
|---|---|---|
| ACCEPT | `PM/sec-l02-01/ACCEPT.md`, 2026-10-30, `ACCEPTED`, P0=0/P1=0 | [V] |
| Ревью | `PM/sec-l02-01/REVIEW.md` — **PASS**, независимое (Codex) | [V] |
| Код | `net.rs`: `DispatchSink::reset()` вызывается на `connect()` (до `Ready`), на `Ready`, `Reconnecting`, `disconnect()` и на всех ветках `Closed` (`4004`, `4013/4014`, прочие); внутри `last_seq = None` | [V] |
| Тест | `net_contract.rs::connect_and_disconnect_reset_sequence_dedup_before_ready` (отложенный `Ready`), 24 теста таргета | [V] |
| Мутации | `mutations.log` — 2/2 KILLED (снятие reset на `connect` и на `disconnect`) | [R] |
| md5 | `net.rs` `b286d177…`, `net_contract.rs` `573d2712…` — совпали | [V] |
| Остаток в аудите | `QUALITY/SEC-L02.md` всё ещё держит «P1 = 1» и §3 «SEC-L02-01 open», `QUALITY/SEC-L02-REVIEW.md` — исторический RETURN. Закрытие живёт в `PM/sec-l02-01/`, аудит QA-владельца не обновлён | [V] |

Закрытие подтверждено кодом и тестом, а не только ACCEPT: сброс стоит на всех границах соединения,
включая ветки `Closed`. Это снимает сценарий «logout → login → первое событие новой сессии
молча отброшено» **[V]**.

### 3.4 Статус находок `TEST-INTEGRITY-WAVE3`

| Находка | Статус на 17:12 | Метка |
|---|---|---|
| F1 `plan-sync` красный | **закрыт**: `meta-after-wave4.log` 102/102, `plan-sync` ✔ | [V] |
| F2 `b2a` md5 ≠ отчёт | **закрыт**: REPORT-FIX + `green-fix.log` + мутации 4/4; дерево = отчёт | [V] |
| F3 `@ts-ignore` без обоснования | **частично**: 3 директивы (было 5) в `composer.test.mjs`, письменного обоснования в файле нет → по §2.4 формально P0 | [V] |
| F4 нет записи в `DECISIONS.md` | **открыт**: файла `DECISIONS.md` в дереве нет; согласование PM живёт комментарием в `gui-spike-features.test.mjs:3087` | [V] |
| F5 устаревший md5 в 07 | **закрыт этим отчётом**: `search-remote-real-index.test.mjs` = `b2692ff1…` (§2.1) | [V] |
| F6 имя теста ≠ утверждение | открыт (P3), не блокер | [R] |
| F7 шесть карточек без ревью | **закрыт**: `REVIEW.md` есть у всех шести | [V] |
| F8 четыре RETURN | **закрыт по факту**: `m07-ui-badge` получил ACCEPT, у `b2a`, G-01b-UI и STUB-14 есть REPORT-FIX; ACCEPT этих трёх ещё нет | [V] |
| F9 снимок устарел | подтверждён: за проход сдвинулись ещё 4 файла (§1, §2.4) | [V] |
| F10 `v01-rtc` «0 passed» | без изменений, к приёмке не относится | [R] |
| F11 `tsc` красный | **закрыт**: `tsc-after-wave4.log` 0 Б | [V] |

---

## 4. Что изменилось с MERGE-READINESS-07

| Пункт 07 | Состояние в 08 | Метка |
|---|---|---|
| P1 `SEC-L02-01` open | **закрыт**: ACCEPT + PASS + reset на всех границах, `net.rs` `b286d177…` | [V] |
| Accepted set = 9 ACCEPT, 16 пинов, 14/16 | **12 ACCEPT** (2 в корне), **22 пина**, 19/22 | [V] |
| `app.js` не покрыт приёмкой | **без изменений**: `70bcf5e7…`, ACCEPT G-01b-UI-SETTINGS-FIX нет | [V] |
| `net_contract.rs` не упомянут в дрейфах | **новый дрейф**, покрыт ACCEPT SEC-L02-01 | [V] |
| M-07-UI-BADGE на ревью | **ACCEPTED**, `chat.js` pin официально перекрыт | [V] |
| B2a на ревью (P1) | **REPORT-FIX**, tier 10/50/100 МиБ, мутации 4/4; ACCEPT нет | [V] |
| meta 102/102 без свежего прогона на fix-байтах | `meta-after-wave4.log` 17:05:40 — 102/102, `plan-sync` ✔ | [V] |
| `tsc` без exit-кода | без изменений: 0 Б, exit не записан | [V] |
| Gate старше окна | без изменений: `0239543`, 2026-10-24 10:22 | [V] |
| `waves/db.jsonl` RW-40 = 0 | без изменений: 0 | [V] |
| Матрица: AT-01/M-13/S-02 устарели | обновлены 7 строк, §14 = 22/37/32/0 | [V] |
| `GAP-BACKLOG` расходился с матрицей | **синхронизирован** (§5) | [V] |

---

## 5. Синхронизация матрицы и бэклога (G6-PARITY-SYNC-08 + GAP-BACKLOG-SYNC)

| Строка | Было | Стало | Основание |
|---|---|---|---|
| **M-13** | backend `search.rs` читает 5 полей, фильтры применяются локально | backend принят: 8 подтверждённых фильтров + `params.query`, валидация до сети; gap — `202`-ретрай, `include_nsfw`, live | `PM/b7b-search/ACCEPT.md`, `REPORT.md` |
| **MB-03** | `partial: stub aria`, нет поповера | `partial`: поповер с 4 состояниями, ARIA, Escape/outside-click, токены; gap — synthetic-данные, действия, live | `PM/stub16-profile-ui/REPORT.md` |
| **S-02** | `absent`, RE закрыт, кода нет | **`partial`**: редьюсер `stream.rs` + маршрутизация 4 серверных `STREAM_*` в `dispatch.rs`; нет capture/consent/op 18-19/UI | `PM/stub27-*/REPORT*.md`, `VOICE/STREAM-GATEWAY-CONTRACT.md` |
| **AT-01** | лимит 10 МиБ без tier | tier-лимиты 10/50/100 МиБ (`GuildBoostTier`), тест 17; gap — `PUT` upload/308/прогресс/UI, сумма 500 МБ/1 ГиБ | `PM/b2a-attach/REPORT-FIX.md` |
| **G-01** | restore на ревью, `settings.js` теряет поле (P2) | restore закрыт полностью: поле переживает `settings_save` и перезапуск; тест 12 | `PM/g01b-ui/REPORT-FIX.md` |
| **M-07** | UI-бейджи «на ревью» | `badgeCount`/`flags` канала приняты, контракт на 9 полей | `PM/m07-ui-badge/ACCEPT.md` |
| **L-02** | `partial`: reducer/core, UI на fixtures | **`implemented (live unverified)`**: L-02-CORE/UI ACCEPT + `STREAM_*` мост; gap — E3, P-02/P-03 | `PM/l02-core/ACCEPT.md`, `PM/l02-ui/ACCEPT.md`, `PM/stub27-dispatch/REPORT.md` |

`GAP-BACKLOG.md` приведён к матрице: §2.1 «Топ-9» (L-02 выведен в §2.4), §2.2 M-13, §2.4 — 9 строк
с обновлёнными остатками G-01/M-07 и новой строкой L-02, §3 — AT-01/MB-03/S-02, §5 — RE-1 и RE-3
сняты с остатками, §6 и §7 пересчитаны **[V]**.

§14 пересчитан независимым оракулом (тот же `awk`, что в самом файле) **[V]**:

```
app absent 32
app implemented 22
app partial 37
prio P0 5 · P1 32 · P2 40 · P3 14
total 91
```

Сходимость бэклога с матрицей: 91 уникальный ID, каждый — ровно один раз
(P0 5 + P1 9+14+9 + P2 40 + P3 14); повторные вхождения `G-02`/`MB-04` — только ссылки в таблице
RE-гейтов §5, не строки очереди **[V]**.

---

## 6. Оставшиеся шаги до merge

| # | Шаг | Owner | DoD |
|---:|---|---|---|
| 1 | **Freeze**: остановить писателей (`composer.test.mjs`, `session.rs`, `composer.js` менялись в 17:10–17:12) | TL | 0 изменений 15 мин, `md5sum` совпадает дважды (сейчас 206 путей и рост) |
| 2 | **ACCEPT на G-01b-UI SETTINGS-FIX** | PM | md5 `70bcf5e7…` + строка «supersedes pin L-02-UI» |
| 3 | **ACCEPT на b2a TIER-FIX, STUB-14 FIX, STUB-16, STUB-27-DISPATCH** | PM | `ACCEPT.md` с md5 = дерево у каждой (§2.3) |
| 4 | **Перевыпуск пинов**: `net_contract.rs` (L-02-CORE → SEC-L02-01), `PM/l02/ACCEPT.md` | PM | строка «superseded by …» в вытесненных ACCEPT |
| 5 | **`tsc` с фиксацией exit-кода** + targeted по 5 изменённым тестовым файлам | G4 | `exit=0` в логе, 0 skip |
| 6 | **`gate:worker` + `gate:release`** с `CHROME_BIN` | G4/TL | `acceptance.accepted === true`, `e2e.fixture === "met"`, `skipped === 0`, `attempts[]` |
| 7 | **`DECISIONS.md`**: запись о согласовании 429 и обоснование 3 `@ts-ignore` | TL + G4 | F3/F4 аудита закрыты письменно |
| 8 | **`MERGE-MANIFEST`** на freeze (включая `rust/gui-spike/**`, `CONTRACTS/`, `tests/e2e/m2-*`, `scripts/gate.mjs`, `ci.yml`) | PM/TL | каждый файл integration имеет md5 |
| 9 | **Реестры**: `TASKS.md`, `QUEUE.md`, `STATE.md`, `waves/db.jsonl`; снять «следствие» в `STUB-TRACKER.md` (строки AT-01/M-13/S-02 синхронизированы) | PM/TL | `grep -c RW-40 waves/db.jsonl` > 0 |
| 10 | **Commit + merge** (L1) | координатор | `git status` пуст, HEAD содержит ACCEPT-файлы |

Порядок обязателен: freeze → прогоны → приёмка → merge. Шаги 2–4 бумажные, но без них мержится
код, у которого текущие байты `app.js` не приняты, а `net_contract.rs`/`chat.js` покрыты только
более поздними ACCEPT без обратной ссылки.

**Что этим не достигается:** `parity-ok` и `qualify`. Нужен live N0 (READY + send + отсутствие
секретов), live-история/реакции/ack, `e2e.live = met`. E3 = 0, live-окна не было **[V]**.

---

## 7. Что НЕ является блокером

1. **Продуктового P0 нет:** P0 = 0 в `QUALITY/SEC-L02.md` §0 (P1 был 1 — теперь закрыт, §3.3);
   выдуманных endpoint/полей G6 не нашёл.
2. **SEC-L02-01 закрыт по существу, а не по бумаге:** `reset()` на `connect`, `Ready`, `Reconnecting`,
   `disconnect` и всех ветках `Closed`; тест с отложенным `Ready`; мутации 2/2 **[V]**.
3. **Границы карточек соблюдены:** B2a TIER-FIX — 2 файла, G-01b SETTINGS-FIX — 3, STUB-14 FIX — 3,
   STUB-16 — 3, STUB-27 — 2; `fixtures/`, `logs/`, `evidence/`, `functions/` не затронуты **[R]**.
4. **Тесты не ослаблены:** B2a 15 прежних + 2 новых при 4/4 мутантах, B7b 12 + 4 при 6/6,
   SEC-L02-01 — аддитивный тест при 2/2, STUB-14 +3 `it` и −0 строк, G-01b +1 тест при 112/112,
   STUB-16 +9 `it` при 5 стабильных прогонах **[R]**/**[V]**.
5. **`waves-sync` не требует событий RW-40** — 0 в журнале остаётся процессным долгом, не красным тестом **[V]**.
6. **`meta` 102/102 и `plan-sync` ✔** — §8, CI-workflow, TEST-POLICY и e2e-реестр согласованы
   на 17:05:40; после 17:12 добавились правки вне тестовых реестров, их покроет следующий прогон **[V]**.

---

## 8. Ограничения отчёта

1. Снимок **17:05–17:12**; дерево менялось во время прохода (196 → 206 dirty-путей, правки
   `composer.test.mjs`/`session.rs`/`composer.js` в 17:10–17:12). Все md5 — точки **[V]**.
2. `cargo`, `node --test`, `npx tsc`, `gate:*`, e2e, браузер, live — **не запускались** (запрет ТЗ).
   Числа прогонов — из логов и ACCEPT **[R]**, кроме содержимого `meta-after-wave{3,4}.log`,
   `tsc-after-wave*`, которые прочитаны **[V]**.
3. `QA-M2-04` md5 взяты из `QUALITY/QA-M2-04-md5.txt`: в `ACCEPT.md` их нет — дефект формы ACCEPT **[R]**.
4. Правки этого прохода — только `waves/RW-40/PARITY/PARITY-MATRIX.md`,
   `waves/RW-40/PARITY/GAP-BACKLOG.md` и этот отчёт. Карточки не закрываю, waiver не выдаю,
   `STUB-TRACKER`, `MERGE-MANIFEST`, `DECISIONS.md` и реестры не правлю — владельцы PM/TL.
5. `INVITES-SPEC.md` прочитан: это дизайн-контракт STUB-05/STUB-07, кода нет — на паритет
   G-06/G-03 не влияет, строки матрицы не менял **[V]**.
6. Файлы этого прохода: `PARITY-MATRIX.md` `988e98ac26d9d474e81c6995bcb6abde`,
   `GAP-BACKLOG.md` `b9e56241f53b76da39932a36783fc072`, `MERGE-READINESS-08.md` — этот отчёт
   (md5 фиксирует принимающий, после правки он неизбежно меняется) **[V]**.

---

## 9. Как перепроверить (read-only, ~1 мин)

```bash
cd /home/sanya/orca/project
git log --oneline -1 && git status --porcelain -uall | wc -l
find waves/RW-40 -name '*ACCEPT*.md' | wc -l; ls PM/g01b-contract/ACCEPT.md QUALITY/QA-M2-04-ACCEPT.md

md5sum rust/gui-spike/src/net.rs rust/gui-spike/tests/net_contract.rs \
       rust/gui-spike/src/search.rs rust/gui-spike/tests/search_contract.rs \
       rust/gui-spike/ui/features/chat.js tests/unit/read-state-ui.test.mjs \
       rust/gui-spike/src/attachments.rs rust/gui-spike/tests/attachments_contract.rs \
       rust/gui-spike/ui/app.js rust/gui-spike/ui/features/settings.js \
       rust/crates/discord-core/src/dispatch.rs                            # §2.1–2.4

grep -E 'tests 102|pass 102|fail 0' waves/RW-40/QUALITY/meta-after-wave4.log   # §3.1
grep -n 'plan-sync' waves/RW-40/QUALITY/meta-after-wave4.log                   # ✔ на строке 94
wc -c waves/RW-40/QUALITY/tsc-after-wave4.log                                  # 0 Б, exit не записан
ls -t .gate/*.json | head -1                                                   # старше окна
grep -n 'SEC-L02-01' QUALITY/SEC-L02.md                                        # аудит QA ещё держит P1

cd waves/RW-40/PARITY && sed 's/\\|/~/g' PARITY-MATRIX.md | awk -F'|' '
/^\| [A-Z][A-Z0-9]*-[0-9]/ { n=NF; app=$6; prio=$(n-3); gsub(/^ +| +$/,"",app);
  gsub(/^ +| +$/,"",prio); sub(/[ (:].*/,"",app); a[app]++; p[prio]++; t++ }
END { for (k in a) print "app",k,a[k]; for (k in p) print "prio",k,p[k]; print "total",t }'
```

Красный флаг для приёмки: **любой** md5 из §2.1, отличающийся от ACCEPT, при отсутствии строки
«superseded by» — значит, наливаются непринятые байты. Сейчас такой один: `ui/app.js`.
