# MERGE-READINESS-07 — md5-сверка accepted set, Wave 2 и свежие логи meta/tsc

Дата: **2026-10-30**, снимок **16:31–16:40 +05**. Ветка `main`, HEAD
`4e240481e59ff4d49732ac8bd5ba08a006363748` (integration commit RW-40 от 2026-10-24 16:22).
Воркер: **G6 Parity**, отдел G6, карточка **G6-PARITY-SYNC-07**. Режим — **read-only по коду**.

> **Путь.** Отчёт лежит в `waves/RW-40/PARITY/`, как MERGE-READINESS-05/06. Вход прохода:
> `reference-oracle-REA.md`, `VOICE/STREAM-GATEWAY-CONTRACT.md`, отчёты Wave 2
> (`PM/b2a-attach`, `PM/m07-ui-badge`, `PM/g01b-ui`, `PM/stub14-search-ui`),
> `PM/m07-be3/{ACCEPT,REVIEW-PASS2}.md`, `QUALITY/QA-M2-04-ACCEPT.md`,
> `QUALITY/{SEC-L02,QA-M2-04-md5.txt}`, `MERGE-POLICY.md`, `TDD-PLAN.md §8`.

Запреты ТЗ соблюдены: продуктовый код не менялся, `cargo` не запускался, `git commit` не делался.
Дополнительно не запускались `node --test`, `npx tsc`, `npm run gate:*`, e2e, браузер, CDP, сеть,
Discord, live. Запускались только `md5sum`, `git status/diff`, `stat`, `grep`/`awk` и один
статический пересчёт §14 матрицы. Метки: **[V]** — измерено в этом проходе, **[R]** — из
отчёта/лога карточки.

---

## 0. Резюме

1. **Вердикт: merge — `block`. Parity — `deferred`.** Блокируют два пункта, оба вне кода Wave 2:
   **P1 `SEC-L02-01`** (дедуп `s` переживает новый `connect`) и **отсутствие gate-артефакта**
   на текущих байтах. Последний `.gate/worker-*.json` — commit `0239543`, 2026-10-24 10:22,
   он старше всего окна **[V]**.
2. **Оба прежних «бумажных» блокера закрыты evidence, а не обещанием.** `plan-sync` теперь
   подтверждён **сохранённым логом**: `QUALITY/meta-102-pass.log` — `tests 102 / pass 102 /
   fail 0 / skipped 0`, внутри `✔ plan-sync: §8 ↔ тестовые файлы ↔ модули` **[V]**. `tsc` —
   `QUALITY/tsc-after-wave2.log` пуст (0 Б), то есть 0 ошибок, но **exit-код в файле не записан**,
   а появившийся в 16:36 `tests/unit/server-menu.test.mjs` лог уже не покрывает (см. §3.2) **[V]**.
3. **Accepted set: 9 ACCEPT-файлов вместо 5.** Четыре новых: `PLAT-11a` (2 пина),
   `C-02-UI-OLDER` (2), `M-07-BE3` (2), `QA-M2-04` (2). Из 9 файлов md5-таблицу имеют 7 — это
   **16 пинов, 14 совпали**; `PM/l02` вытеснен L-02-CORE/UI, `PM/c02-be` принят не по md5.
   Два расхождения — `app.js` и `chat.js` — объяснены более поздними карточками в тех же файлах
   (§2.3) **[V]**.
4. **Wave 2 на ревью: 4 карточки / 12 пинов, все 12 md5 совпали с отчётами** **[V]**:
   B2a-ATTACH-BACKEND (3), M-07-UI-BADGE (2), G-01b-UI + C-07-FIX (4), STUB-14-UI (3).
   Ни у одной ещё нет `ACCEPT.md` — приёмка не выдана, «готово к приёмке» ≠ accepted.
5. **Дерево не заморожено — и это видно в этом проходе.** Продуктовый код стоял 9 минут
   (последняя запись окна — `search-overlay.js` 16:27:28), а в 16:36:22–16:37:03 пошли новые
   правки: `tests/unit/server-menu.test.mjs`, `ui/features/actions.js`. За время прохода
   `git status` вырос **91 → 102** пути за ~6 минут. Все md5 ниже — точки, не фиксация **[V]**.
6. **Матрица синхронизирована** (`PARITY-MATRIX.md`): AT-01 `absent → partial`, M-13 `4/7 → 7/7`
   UI-фильтров, S-02 — RE подтверждён и контракт заморожен (статус `absent`), G-01/C-07/M-07 —
   уточнены Gap/DoD/Test. §14 пересчитан: **21 implemented / 37 partial / 33 absent / 0 unknown,
   всего 91**; независимый пересчёт `awk` по самому markdown даёт те же числа **[V]**.

---

## 1. Снимок дерева

| Параметр | Значение | Метка |
|---|---|---|
| HEAD | `4e240481e59ff4d49732ac8bd5ba08a006363748` (2026-10-24 16:22) | [V] |
| Dirty, `git status --porcelain -uall` | **98** путей (25 `M` + 73 `??`) на 16:37; на 16:34 — 91, на 16:36 — 93; к 16:40 — **102** | [V] |
| Дельта tracked | `25 files changed, 1771 insertions(+), 272 deletions(-)` (растёт во время прохода) | [V] |
| Последняя запись во «втором окне» | `ui/features/actions.js` 16:37:03, `tests/unit/server-menu.test.mjs` 16:36:22 | [V] |
| Последняя запись «первого окна» | `ui/features/search-overlay.js` 16:27:28 | [V] |
| Журнал | `grep -c '"wave": "RW-40"' waves/db.jsonl` → **0** (exit 1) | [V] |
| Gate-артефакт | `.gate/worker-2026-10-24T10-22-26-400Z.json`, commit `0239543`, `passed_with_skips` | [V] |

Порядок правок «первого окна» 16:09–16:27 (по `stat`) **[V]**: `TDD-PLAN.md` 16:09 →
`messages.rs` 16:12 → `channel-paging.test.mjs` 16:13 → `guild-rail.test.mjs` 16:24:16 →
`gui-spike-regressions` 16:24:35 → `read-state-ui.test.mjs` 16:24:41 → `src/lib.rs` 16:25:18 →
`quick-switcher.js` 16:25:25 → `chat.js` 16:25:51 → `app.js` 16:26:16 →
`CONTRACTS/read-state.md` 16:26:36 → `search.js` 16:26:58 → `search-overlay.js` 16:27:28.

Вывод: после 16:27:28 дерево стояло ~9 минут, но freeze объявлен не был — в 16:36:22–16:37:03
началась следующая карточка (`server-menu.test.mjs`, `actions.js`), плюс появились
`STUB-TRACKER.md` (16:28), `ORCA-WORKERS.md` (16:34), `PM/v01-rtc/baseline-test.log` и этот
отчёт. Для merge нужен явный freeze с двумя совпадающими `md5sum`, а не «вроде перестали писать».

---

## 2. Сверка md5

### 2.1 Accepted set — 16 пинов, 14 совпали

| Срез | Файл (md5 из ACCEPT) | md5 в дереве 16:31 | Итог |
|---|---|---|---|
| **G-01b-CONTRACT** | `settings_store.rs` `ac58e876a6553e0d00cd881f30622b0b` | `ac58e876…` | совпал [V] |
| | `settings_contract.rs` `0e943bb9a487d77718cb37b3338e4ea6` | `0e943bb9…` | совпал [V] |
| **L-02-CORE** | `discord-core/src/ws.rs` `8703c1d695cd0af26458393eb3c1e3f9` | `8703c1d6…` | совпал [V] |
| | `gateway_supervisor.rs` `7f23d85a4d3b0e54c58d765b8b879c58` | `7f23d85a…` | совпал [V] |
| | `gui-spike/tests/net_contract.rs` `fe8aa92146e9dcf8b943335dfad652c8` | `fe8aa921…` | совпал [V] |
| **L-02-UI** | `ui/features/dispatch-model.js` `37dacc61d7daf74c144681637dded1f5` | `37dacc61…` | совпал [V] |
| | `ui/app.js` `1ac5c36976a064120529c132e61cfb54` | `5f081fdd…` | **разошёлся** [V] — G-01b-UI |
| | `tests/unit/dispatch_ui.test.mjs` `d30771d65b6373ecbbac6de1f9e61a3d` | `d30771d6…` | совпал [V] |
| **PLAT-11a** | `src/messages.rs` `c449a06c783c795effb33c7fc763614e` | `c449a06c…` | совпал [V] |
| | `tests/messages_contract.rs` `0f92348a5e3e15563603faec0917c4b5` | `0f92348a…` | совпал [V] |
| **C-02-UI-OLDER** | `ui/features/chat.js` `6444e9d94a6daa9e1d7b036525295398` | `1e73c602…` | **разошёлся** [V] — M-07-UI-BADGE |
| | `tests/unit/channel-paging.test.mjs` `4ef50982a5de2847765bd1bc64d8dedf` | `4ef50982…` | совпал [V] |
| **M-07-BE3** | `src/data.rs` `8013271de7a6444b2b9dba3301a165d4` | `8013271d…` | совпал [V] |
| | `tests/read_state_contract.rs` `fde688f33614636c2699c55ebba945e9` | `fde688f3…` | совпал [V] |
| **QA-M2-04** | `.github/workflows/ci.yml` `608e26837c10996aae11d2c29fc6d320` | `608e2683…` | совпал [V]/[R] |
| | `tests/meta/ci-browser.test.mjs` `06ab44eee0c919777cbaec91756adaff` | `06ab44ee…` | совпал [V]/[R] |

`QA-M2-04-ACCEPT.md` md5 в таблице не содержит — значения взяты из
`QUALITY/QA-M2-04-md5.txt` (там же `scripts/ci.sh` `fb5ad676…`, логи) **[R]**; в дереве
оба файла совпали **[V]**. `M-07-BE3` дополнительно фиксирует `tests/data_contract.rs`
`e31c9127…` в отчёте (не в ACCEPT) — в дереве совпал **[V]**.

### 2.2 Ожидают ревью (Wave 2) — 12 пинов, 12 совпали

| Карточка | Файл (md5 из отчёта) | md5 в дереве | Итог |
|---|---|---|---|
| **B2a-ATTACH-BACKEND** | `src/attachments.rs` `c2e1062287d334a397901a7ebe90b3a3` | `c2e10622…` | совпал [V] |
| | `src/lib.rs` `f8b5af6132cba13213d3a79031b8f976` | `f8b5af61…` | совпал [V] |
| | `tests/attachments_contract.rs` `cdc8cbd9dd91323ad97a84ef4acfa962` | `cdc8cbd9…` | совпал [V] |
| **M-07-UI-BADGE** | `ui/features/chat.js` `1e73c60219792da3c332b8cbc73c7607` | `1e73c602…` | совпал [V] |
| | `tests/unit/read-state-ui.test.mjs` `fe363afe9abcc82bc99e1e59b0af58c5` | `fe363afe…` | совпал [V] |
| **G-01b-UI + C-07-FIX** | `ui/app.js` `5f081fdd05224d68ac4e7d8b9a803c8c` | `5f081fdd…` | совпал [V] |
| | `ui/features/quick-switcher.js` `ba8aff457122cbd67b8edefec2df7479` | `ba8aff45…` | совпал [V] |
| | `tests/unit/guild-rail.test.mjs` `939ab3ab60297a4d132f12a997053b0e` | `939ab3ab…` | совпал [V] |
| | `tests/unit/gui-spike-regressions.test.mjs` `b0d21cbca0f3695087d0b5cabb1e6b4b` | `b0d21cbc…` | совпал [V] |
| **STUB-14-UI** | `ui/features/search-overlay.js` `b6f1226b7c4b9496256bbc18c5b79e6f` | `b6f1226b…` | совпал [V] |
| | `ui/features/search.js` `87379a7b6a315bb6fe114e99fa42d13d` | `87379a7b…` | совпал [V] |
| | `tests/unit/search-remote-real-index.test.mjs` `9b099543e8bbb465288bc27ac05eb4e2` | `9b099543…` | совпал [V] |

Ни в одном из четырёх каталогов нет `ACCEPT.md` **[V]** — это не «принято», это «сдано и
ждёт». `PM/b2b-attach-ui/` создан 16:32 и пуст: следующая карточка стартовала, артефактов нет **[V]**.

### 2.3 Два расхождения — и почему это не дефект кода

| Файл | Пин | Кто переписал | Правильное действие |
|---|---|---|---|
| `ui/app.js` | L-02-UI `1ac5c369…` | G-01b-UI `5f081fdd…` (16:26:16) | ACCEPT G-01b-UI с md5 или строка «supersedes pin L-02-UI» |
| `ui/features/chat.js` | C-02-UI-OLDER `6444e9d9…` | M-07-UI-BADGE `1e73c602…` (16:25:51) | ACCEPT M-07-UI-BADGE + строка «supersedes pin C-02-UI-OLDER» |

Это ровно тот системный эффект, который MERGE-READINESS-06 назвал в §2.1: составные файлы
(`app.js`, `chat.js`) несут по 3–4 карточки, поэтому md5-пин ломается следующей карточкой
в том же файле. Ни один из двух дрейфов не отменяет прежнюю приёмку по существу — но формально
приёмка больше не покрывает текущие байты, пока не появится ACCEPT на переписавшую карточку.
Первым делом это касается `app.js`: он уже дважды менялся после выдачи ACCEPT.

### 2.4 Прежние ACCEPT — статус не изменился

| ACCEPT | Статус |
|---|---|
| `PM/l02/ACCEPT.md` (L-02 backend) | вытеснен L-02-CORE/L-02-UI; строки «superseded by» по-прежнему нет [V] |
| `PM/c02-be/ACCEPT.md` | приёмка сознательно не по md5 (сам ACCEPT §3) [R] |

---

## 3. Свежие логи Wave 2

### 3.1 `QUALITY/meta-102-pass.log` — meta закрыт, evidence на месте

| Факт | Значение | Метка |
|---|---|---|
| Файл | 13 500 Б, 144 строки, mtime 16:31:28 | [V] |
| Итог | `tests 102 · suites 17 · pass 102 · fail 0 · cancelled 0 · skipped 0 · todo 0`, 161.9 мс | [V] |
| `plan-sync` | `✔ plan-sync: §8 ↔ тестовые файлы ↔ модули` (строка 94) | [V] |
| `waves-sync` | `✔ волны R4/R5/R6/D … журнал восстановим` | [V] |
| Форма | нет строки команды и нет `exit=`; вердикт восстановим по `fail 0` | [V] |

Это закрывает пробел MERGE-READINESS-06 §3.2: раньше 102/102 существовало только как ссылка на
`ORCA-WORKERS.md`, теперь лог лежит в `waves/RW-40/QUALITY/`. `plan-sync` проверен **прогоном**,
а не статическим разбором §8, как в 06.

### 3.2 `QUALITY/tsc-after-wave2.log` — 0 ошибок, но без exit-кода

| Факт | Значение | Метка |
|---|---|---|
| Размер | **0 байт**, mtime 16:31:28 (тот же прогон, что meta) | [V] |
| Интерпретация | пустой stdout+stderr у `tsc` = 0 ошибок | [V] |
| Покрытие | все правки `tests/**` до 16:26:37 попали в прогон 16:31:28: +378 строк от 16:13–16:24 (`channel-paging` 154, `guild-rail` 122, `gui-spike-regressions` 80, `read-state-ui` 22) и +140 в `search-remote-real-index`; но в 16:36:22 появился новый `tests/unit/server-menu.test.mjs`, который лог уже **не** покрывает | [V] |
| Пробел | нет строки команды и нет `exit=`; 0-байтовый файл неотличим от «команду не запускали» | [V] |

Формально это evidence слабее meta-лога: содержимое согласуется с «чисто», но проверить
exit-код нечем, а новый тестовый файл от 16:36:22 входит в `include` корневого `tsconfig.json`
и тоже не покрыт. Лечение — одна строка:
`npx tsc -p tsconfig.json > waves/RW-40/QUALITY/tsc-after-wave2.log 2>&1; echo "exit=$?" >> …`
на замороженном дереве. До этого блокер `tsc` считаю **условно закрытым**: ошибок не видно,
подтверждения кода возврата и полного охвата нет.

### 3.3 Гейт по-прежнему не покрывает принимаемые байты

| Артефакт | Commit | Вердикт | Покрывает |
|---|---|---|---|
| `.gate/worker-2026-10-24T10-22-26-400Z.json` (последний) | `0239543` | `passed_with_skips` | **ничего** из окна: старше L-02, G-01b, M-07, B2a, STUB-14 и всех ACCEPT |
| `.gate/release-2026-10-24T09-37-37-616Z.json` (последний) | `0239543` | `passed_with_skips`, e2e 4 skipped | live-фазы нет, `acceptance`/`attempts[]` нет |

`gate:worker`/`gate:release` на текущем дереве не запускались (запрет ТЗ: `cargo`).
Условие «gate на замороженных байтах» из `MERGE-POLICY §2` не выполнено.

---

## 4. Что изменилось с MERGE-READINESS-06

| Пункт 06 | Состояние в 07 | Метка |
|---|---|---|
| `tsc` 16 → 0 только по логу от 10-24 | свежий лог 16:31:28 на тестах окна; 0 Б, exit не записан; новый `server-menu.test.mjs` (16:36) не покрыт | [V] |
| meta 102/102 без лога | лог `meta-102-pass.log` сохранён, 102/102, `plan-sync` ✔ | [V] |
| Accepted set = 5 ACCEPT-файлов, md5 — по 8 пинам окна | **9 ACCEPT-файлов**, 7 с md5-таблицами / **16 пинов**, 14/16 совпали | [V] |
| PLAT-11a, C-02-UI-OLDER без ACCEPT | оба `ACCEPTED` 2026-10-30, md5 совпали | [V] |
| M-07-BE3 — RETURN (P2) | `REVIEW-PASS2` ACCEPT + `ACCEPT.md`, P2 закрыт | [V] |
| QA-M2-04 без приёмки | `ACCEPTED`, P0/P1/P2 = 0, `ci-browser` 10/10 | [V]/[R] |
| Матрица: AT-01/M-13/S-02 устарели | строки переписаны, §14 пересчитан (21/37/33/0, 91) | [V] |
| P1 `SEC-L02-01` open | **open**: `net.rs` `e13d4e8c…` не менялся | [V] |
| Gate старше окна | **без изменений**: последний артефакт `0239543` | [V] |
| `waves/db.jsonl` RW-40 = 0 | **без изменений**: 0 | [V] |

---

## 5. Что сделано с матрицей (G6-PARITY-SYNC-07)

| Строка | Было | Стало | Основание |
|---|---|---|---|
| **AT-01** | `absent (кнопка disabled)`, gap «**blocked**: метод/поля/лимиты», conf 0.45 | `partial: backend слотов (B2a, на ревью)`; RE снят; gap — нет `PUT`/докачки 308/прогресса/UI, лимиты tier не параметризованы; conf 0.8 | `reference-oracle-REA §1.1–1.8`, `ATTACH-RE §2`, `PM/b2a-attach/REPORT.md` |
| **M-13** | `implemented (4/7 фильтров)`, gap «6 фильтров/лимиты не подтверждены» | `implemented (7/7 UI-фильтров)`; gap — backend `search.rs` читает 5 полей, фильтры применяются локально, нет `202`-ретрая/`include_nsfw`; conf 0.9 | `reference-oracle-REA §2.1–2.4`, `PM/stub14-search-ui/REPORT.md`, `src/search.rs` |
| **S-02** | `STREAM [A]`, gap «нет capture/сигнализации», conf 0.45 | статус `absent`, но RE-STUB-27 подтверждён и контракт шлюза заморожен (op 18…22, поля, `stream_key`, 6 фаз); живых событий 0; conf 0.75 | `reference-oracle-REA §6`, `VOICE/STREAM-GATEWAY-CONTRACT.md` |
| **G-01** | gap «restore `lastGuildId` — G-01b», test `(8)` | restore реализован (G-01b-UI, на ревью), добавлен риск «`settings.js` теряет поле при `persist()`»; test `(11 = 8 + 3)` | `PM/g01b-ui/REPORT.md` §6.1 |
| **C-07** | `implemented (B6)`, gap одна строка | `implemented (B6, live unverified)`; C-07-FIX: `data-guild-id`, legacy `data-guild` не читается; выбор не пишет `lastGuildId` | `PM/g01b-ui/REPORT.md` |
| **M-07** | `read_state_contract.rs (12)`, `read-state-ui.test.mjs (8)`, conf 0.85 | 9 полей проекции, UI выводит `badgeCount`/`flags` канала; тесты `(15)` и `(13)`; conf 0.9 | `PM/m07-be3/ACCEPT.md`, `PM/m07-ui-badge/REPORT.md` |

§14 пересчитан независимым оракулом (тот же `awk`-скрипт, что в самом файле) **[V]**:

```
app absent 33
app implemented 21
app partial 37
prio P0 5 · P1 32 · P2 40 · P3 14
total 91
```

`AT-01` — единственный сдвиг статуса (`absent → partial`), остальные пять строк — уточнение
Gap/DoD/Test/Conf без смены класса. Строки `G-08`/`C-03` намеренно не тронуты: канальный бейдж
появился, бейджи рейла и сервера — нет.

**Остаток, который матрица не закрывает** (владельцы — PM/TL):
`GAP-BACKLOG.md` всё ещё говорит «M-13: поиск live не сверен, 3 фильтра» и «AT-01: **BLOCKED**:
upload-контракт» — расхождение с обновлённой матрицей **[V]**. Параграф-«следствие» в
`STUB-TRACKER.md` («строки AT-01/M-13/S-02 устарели и требуют синхронизации») теперь выполнен
и подлежит снятию.

---

## 6. Оставшиеся шаги до merge

| # | Шаг | Owner | DoD |
|---:|---|---|---|
| 1 | **Freeze**: писатели остановлены, `git status -uall` зафиксирован | TL | 0 изменений 15 мин, `md5sum` совпадает дважды (сейчас 98 путей и рост) |
| 2 | **`tsc` с фиксацией exit-кода** + targeted (5 изменённых тестовых файлов + новый `server-menu.test.mjs`) | G4 | `exit=0` в логе; targeted 43/43 + channel-paging/guild-rail/gui-spike-regressions/read-state-ui/search-remote-real-index/server-menu, 0 skip |
| 3 | **`gate:worker` + `gate:release`** с `CHROME_BIN` | G4/TL | `acceptance.accepted === true`, `e2e.fixture === "met"`, `skipped === 0`, `attempts[]` |
| 4 | **Закрыть `SEC-L02-01`** red-first | G1 | тест «connect → dispatch → connect → dispatch(s=1)» красный до, зелёный после; `net.rs` новый md5 |
| 5 | **ACCEPT на Wave 2**: B2a, M-07-UI-BADGE, G-01b-UI + C-07-FIX, STUB-14-UI | PM (+Codex) | `ACCEPT.md` с md5 = дерево у каждой из 4 карточек |
| 6 | **Перевыпуск пинов**: `app.js` (L-02-UI → G-01b-UI), `chat.js` (C-02-UI-OLDER → M-07-UI-BADGE), `PM/l02/ACCEPT.md` | PM | строка «superseded by …» + md5-лист на freeze |
| 7 | **Решение по P2 G-01b**: `settings.js` затирает `lastGuildId` при `persist()` | PM/TL | карточка владельцу `settings.js` либо письменный waiver |
| 8 | **`MERGE-MANIFEST`** на freeze (включая `rust/gui-spike/**`, `CONTRACTS/`, `tests/e2e/m2-*`, `scripts/gate.mjs`, `ci.yml`) | PM/TL | каждый файл integration имеет md5 |
| 9 | **`GAP-BACKLOG`**: M-13/AT-01 строки, снять «следствие» в `STUB-TRACKER` | PM/TL | матрица и бэклог сходятся |
| 10 | **Реестры**: `TASKS.md`, `QUEUE.md`, `STATE.md`, `waves/db.jsonl` | PM/TL | `grep -c RW-40 waves/db.jsonl` > 0 |
| 11 | **Commit + merge** (L1): все dirty-пути на freeze + ACCEPT/логи | координатор | `git status` пуст, HEAD содержит ACCEPT-файлы |

Порядок обязателен: freeze → прогоны → приёмка → merge. Шаги 5–9 бумажные, но без них код
мержится без md5-актуального ACCEPT: за один проход 06 два пина успели разойтись, за этот —
разошлись ещё два (`app.js`, `chat.js`), и оба раза «виноват» не дефект, а составной файл.

**Что этим не достигается:** `parity-ok` и `qualify`. Нужен live N0 (READY + send + отсутствие
секретов), live-история/реакции/ack, `e2e.live = met`. E3 = 0, live-окна не было **[V]**.

---

## 7. Что НЕ является блокером

1. **Продуктового P0 нет:** P0 = 0 в `SEC-L02`; выдуманных endpoint/полей G6 не нашёл.
2. **Wave 2 сдана чисто по границам:** B2a тронул 3 файла (`attachments.rs`, `lib.rs`,
   `attachments_contract.rs`), M-07-UI — 2, G-01b-UI — 4, STUB-14-UI — 3; чужие модули,
   `fixtures/`, `logs/`, `evidence/`, `functions/` не затронуты **[R]**.
3. **Тесты не ослаблены:** B2a 6/6 мутантов убиты, `lib.rs` — одна строка экспорта; STUB-14-UI
   в тесте +140/−0 строк; M-07-UI добавил 5 тестов при 8 прежних зелёных; G-01b-UI — 3 + 1 тест
   **[R]**.
4. **`waves-sync` не требует событий RW-40** — 0 в журнале процессный долг, не красный тест.
5. **meta 102/102 зелёный на дереве 16:31** — §8, CI-workflow, TEST-POLICY и e2e-реестр
   согласованы прямо сейчас, а не «когда-то» **[V]**. Новый `server-menu.test.mjs` (16:36)
   в этот прогон не входил — его добавит следующий meta-прогон.

---

## 8. Ограничения отчёта

1. Снимок **16:31–16:40**; дерево менялось во время прохода (91 → 93 → 98 → 102 dirty-пути,
   в 16:36:22–16:37:03 начались правки следующей карточки). Все md5 — точки; freeze не объявлен.
2. `cargo`, `node --test`, `npx tsc`, `gate:*`, e2e, браузер, live — **не запускались** (запрет ТЗ).
   Все числа прогонов — из логов карточек **[R]**, кроме содержимого двух свежих логов Wave 2,
   которые я прочитал **[V]**.
3. `QA-M2-04` md5 взяты из `QUALITY/QA-M2-04-md5.txt`, потому что в `ACCEPT.md` их нет —
   это дефект формы ACCEPT, а не сверки.
4. Матрицу я обновил (это ТЗ карточки). Карточки не закрываю, waiver не выдаю, `GAP-BACKLOG`,
   `STUB-TRACKER`, `MERGE-MANIFEST` и реестры не правлю — владельцы PM/TL.
5. Правки этого прохода — только `waves/RW-40/PARITY/PARITY-MATRIX.md` и
   `waves/RW-40/PARITY/MERGE-READINESS-07.md`.

---

## 9. Как перепроверить (read-only, ~1 мин)

```bash
cd /home/user/orca/project
git log --oneline -1 && git status --porcelain -uall | wc -l
md5sum rust/gui-spike/ui/app.js rust/gui-spike/ui/features/chat.js \
       rust/gui-spike/src/attachments.rs rust/gui-spike/src/lib.rs \
       rust/gui-spike/tests/attachments_contract.rs \
       rust/gui-spike/ui/features/search.js rust/gui-spike/ui/features/search-overlay.js \
       tests/unit/search-remote-real-index.test.mjs \
       tests/unit/guild-rail.test.mjs tests/unit/gui-spike-regressions.test.mjs   # §2.1–2.2
grep -E 'tests 102|pass 102|fail 0' waves/RW-40/QUALITY/meta-102-pass.log         # §3.1
wc -c waves/RW-40/QUALITY/tsc-after-wave2.log                                     # 0 Б, exit не записан
ls -t .gate/*.json | head -1                                                      # старше окна
grep -n "SEC-L02-01" QUALITY/SEC-L02.md && md5sum rust/gui-spike/src/net.rs       # P1 всё ещё open
cd waves/RW-40/PARITY && sed 's/\\|/~/g' PARITY-MATRIX.md | awk -F'|' '
/^\| [A-Z][A-Z0-9]*-[0-9]/ { n=NF; app=$6; prio=$(n-3); gsub(/^ +| +$/,"",app);
  gsub(/^ +| +$/,"",prio); sub(/[ (:].*/,"",app); a[app]++; p[prio]++; t++ }
END { for (k in a) print "app",k,a[k]; for (k in p) print "prio",k,p[k]; print "total",t }'
```

Красный флаг для приёмки: **любой** md5 из §2.1, отличающийся от ACCEPT, при отсутствии строки
«superseded by» — значит, наливаются непринятые байты.
