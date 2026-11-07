# MERGE-READINESS-06 — accepted set, закрытие tsc/plan-sync и остаток до merge

Дата: **2026-10-30**, снимок **16:21–16:27 +05**. Ветка `main`, HEAD
`4e240481e59ff4d49732ac8bd5ba08a006363748` (integration commit RW-40 от 2026-10-24 16:22).
Воркер: **G6 Parity**, отдел G6, карточка MERGE-READINESS-06. Режим — **read-only по коду**.

> **Путь.** Отчёт лежит в `waves/RW-40/PARITY/`, как MERGE-READINESS-05: корневых
> `PARITY/`/`PM/`/`QUALITY/` в смысле ТЗ нет. Вход этого прохода: `PM/g01b-contract/ACCEPT.md`,
> `waves/RW-40/PM/l02-core/ACCEPT.md`, `waves/RW-40/PM/l02-ui/ACCEPT.md`,
> `waves/RW-40/RE-DEEP-REA.md`, `waves/RW-40/ATTACH-RE.md`,
> `waves/RW-40/QUALITY/TSC-FIX.md`, `QUALITY/{SEC-L02,QA-M2-04,QA-M2-02}.md`, `TDD-PLAN.md §8`.

Что **не** запускалось (запрет ТЗ): `cargo`, `node --test`, `npx tsc`, `npm run gate:*`, e2e,
браузер, CDP, сеть, Discord, live. Запускались только `md5sum`, `git status/diff`,
`stat` и один статический разбор §8 скриптом на чтение. Метки: **[V]** — измерено в этом
проходе, **[R]** — из отчёта/лога карточки.

---

## 0. Резюме

1. **Вердикт: merge — `block`. Parity — `deferred`.** Продуктового P0 нет; блокирует не
   поведение кода, а **незамороженное дерево** и **приёмочный контур**: пока шёл проход,
   `ui/app.js` изменили в 16:26:16, и пин L-02-UI разошёлся (**7/8**
   ACCEPT-md5 вместо 8/8).
2. **Два прежних блокера гейта закрыты карточками.** `tsc`: 16 ошибок → **0** (`TSC-FIX`,
   red/green-логи) **[R]**. `plan-sync`: в `TDD-PLAN.md §8` добавлены **10 строк**, статическая
   сверка — **64 строки на 64 тестовых файла, 0 unlisted, 0 unknown, 0 отсутствующих токенов**
   **[V]**; прогон meta **102/102** заявлен PM (`ORCA-WORKERS.md`) **[R]**, отдельного лога
   «102/102» в `waves/RW-40/QUALITY/` нет.
3. **Accepted set вырос с 2 до 5 ACCEPT-файлов**, из них три — текущего окна:
   G-01b-CONTRACT (2 файла), L-02-CORE (3), L-02-UI (3). На 16:23 md5 совпадали **8/8** **[V]**;
   к 16:27 — **7/8** (дрейф `app.js` `1ac5c369…` → `5f081fdd…`, правка G-01b UI) **[V]**.
4. **P1 не закрыт:** `SEC-L02-01` (дедуп `s` переживает новый `connect`). `rust/gui-spike/src/net.rs`
   по-прежнему `e13d4e8c…` — тот же байт, что в снимке SEC-L02. По `MERGE-POLICY §2/§4.6`
   P1 без фикса или письменного waiver запрещает merge.
5. **Гейт на текущем дереве не подтверждён:** последний артефакт `.gate/worker-2026-10-24T10-22-26-400Z.json`
   — commit `0239543`, `verdict: passed_with_skips`, meta 65 тестов; он **старше** L-02-CORE/UI,
   TSC-FIX, `§8` и всех трёх ACCEPT. Ни один gate-артефакт не покрывает принимаемые байты.
6. **Заглушки разблокированы оракулом:** `STUB-13`, `STUB-12`, `STUB-14`, `STUB-27` переведены
   `blocked/unknown → ready` со ссылкой на `RE-DEEP-REA.md`; `blocked` в трекере стало **0**
   (см. `waves/RW-40/STUB-TRACKER.md`).

---

## 1. Снимок дерева и метод

| Параметр | Значение |
|---|---|
| HEAD | `4e240481e59ff4d49732ac8bd5ba08a006363748` (2026-10-24 16:22 +05) |
| Ветка | `main` |
| Dirty, `git status --porcelain -uall` | **79** путей (23 `M` + 56 `??`) на 16:27; в начале прохода — 48 **[V]** |
| Дельта tracked-файлов | `23 files changed, 1531 insertions(+), 172 deletions(-)` на 16:27 **[V]** |
| Продукт в git | `rust/gui-spike` — **84 файла в индексе** (в integration commit) **[V]** |
| Журнал | `grep -c '"wave": "RW-40"' waves/db.jsonl` → **0** **[V]** |
| Активные записи во время прохода | `ui/app.js` 16:26:16, `chat.js` 16:25:51, `src/lib.rs` 16:25:18, `ui/features/quick-switcher.js` 16:25:25 **[V]** |

Дерево **не заморожено**: за проход число dirty-путей выросло с 48 до 79, а
`app.js` (файл принятого среза L-02-UI) правили уже **после** выдачи ACCEPT. Любой md5
ниже — точка, а не фиксация. Это не придирка к процессу: именно из-за этого ACCEPT
L-02-UI перестал быть md5-актуальным за время прохода.

---

## 2. Сверка ACCEPT-файлов

### 2.1 Срезы текущего окна (все три — 2026-10-30, HEAD `4e24048`)

| Срез | Файл (md5 из ACCEPT) | md5 в дереве 16:23 | md5 в дереве 16:26–16:27 | Итог |
|---|---|---|---|---|
| **G-01b-CONTRACT** | `settings_store.rs` `ac58e876a6553e0d00cd881f30622b0b` | `ac58e876…` | `ac58e876…` | совпал **[V]** |
| | `settings_contract.rs` `0e943bb9a487d77718cb37b3338e4ea6` | `0e943bb9…` | `0e943bb9…` | совпал **[V]** |
| **L-02-CORE** | `discord-core/src/ws.rs` `8703c1d695cd0af26458393eb3c1e3f9` | `8703c1d6…` | `8703c1d6…` | совпал **[V]** |
| | `discord-core/tests/gateway_supervisor.rs` `7f23d85a4d3b0e54c58d765b8b879c58` | `7f23d85a…` | `7f23d85a…` | совпал **[V]** |
| | `gui-spike/tests/net_contract.rs` `fe8aa92146e9dcf8b943335dfad652c8` | `fe8aa921…` | `fe8aa921…` | совпал **[V]** |
| **L-02-UI** | `ui/features/dispatch-model.js` `37dacc61d7daf74c144681637dded1f5` | `37dacc61…` | `37dacc61…` | совпал **[V]** |
| | `ui/app.js` `1ac5c36976a064120529c132e61cfb54` | `1ac5c369…` | `3d65cb77…` (16:26:01) → **`5f081fdd…`** (16:27:17) | **разошёлся** **[V]** (правка G-01b UI; файл читался в момент записи) |
| | `tests/unit/dispatch_ui.test.mjs` `d30771d65b6373ecbbac6de1f9e61a3d` | `d30771d6…` | `d30771d6…` | совпал **[V]** |

Счёт: **8/8 → 7/8**. Содержательно дрейф — это G-01b-UI (`readLastGuildId`,
`resolvePreferred`, `onChoose`), но формально ACCEPT L-02-UI больше не покрывает текущие байты
файла. Два разных md5 за полторы минуты (`3d65cb77…` в 16:26:01, `5f081fdd…` в 16:27:17 при
mtime 16:26:16) означают, что чтение попало в момент записи — то есть в `app.js` писали прямо
во время сверки. Лечение — не «подкрутить md5», а один из двух вариантов: (a) отдельный ACCEPT
на G-01b-UI с md5 на freeze и явной записью «supersedes app.js-пин L-02-UI», либо (b) приёмка
по **дельте** (diff-хунки L-02-UI) плюс свежий md5-лист после freeze.

Это системная проблема составных файлов: `app.js` и `chat.js` несут по 3–4 карточки, поэтому
md5-пин любого ACCEPT ломается следующей карточкой в том же файле. Для merge нужен
md5-манифест **на момент freeze**, а ACCEPT-пины читать как «было принято здесь».

### 2.2 Прежние ACCEPT (до окна) — статус

| ACCEPT | Дата | md5 = дерево | Вывод |
|---|---|---|---|
| `waves/RW-40/PM/l02/ACCEPT.md` (L-02 backend) | 2026-10-24 | **0/4** (`net.rs` `f21143f3…`→`e13d4e8c…`, `main.rs` `0082fb4b…`→`336863b5…`, `net_contract.rs` `0d0ff80d…`→`fe8aa921…`, `dispatch.md` `58f678f9…`→`4bfeef6c…`) **[V]** | вытеснен L-02-CORE/L-02-UI; нужна строка «superseded by» |
| `waves/RW-40/PM/c02-be/ACCEPT.md` | 2026-10-24 | приёмка **сознательно не по md5** (сам ACCEPT §3 это фиксирует) | решение по прогону, не по байтам; при merge всё равно нужен md5-лист |

### 2.3 Review-accept без `ACCEPT.md` (не входят в accepted set)

| Срез | Review | md5 = дерево | Чего не хватает |
|---|---|---|---|
| PLAT-11a | `PM/plat-11a/REVIEW.md`: **ACCEPT** P0/P1/P2 = 0 | **2/2** (`messages.rs` `c449a06c…`, `messages_contract.rs` `0f92348a…`) **[V]** | PM-ACCEPT.md |
| C-02-UI-OLDER-FIX | `PM/c02-ui-older/REVIEW-PASS2.md`: **ACCEPT** | **2/2** (`chat.js` `6444e9d9…`, `channel-paging.test.mjs` `4ef50982…`) **[V]** | PM-ACCEPT.md |
| M-07-BE3 | `PM/m07-be3/REVIEW.md`: **RETURN**, P2=1 (шестиполевое описание `read_state`) | н/д | фикс P2 → повторное ревью → ACCEPT |
| TSC-FIX | `PM/l02-ui/REVIEW-PASS2.md` покрывает TSC-FIX | тест `d30771d6…` = ACCEPT L-02-UI **[V]** | закрыт в составе L-02-UI ACCEPT |
| QA-M2-02 / QA-M2-04 | нет | н/д | браузерный прогон (02), первый прогон Actions (04) |

---

## 3. Блокеры `tsc` и `plan-sync`: что именно закрыто

### 3.1 `tsc` — 16 → 0

| Факт | Значение | Метка |
|---|---|---|
| RED | `npx tsc -p tsconfig.json` → exit 1, **16 ошибок**, все в `tests/unit/dispatch_ui.test.mjs` (8×TS7006, 5×TS2339, 2×TS2345, 1×TS7006) | `waves/RW-40/QUALITY/TSC-FIX-red.log` **[R]** |
| GREEN | exit 0, **0 ошибок** | `waves/RW-40/QUALITY/TSC-FIX-green.log` (`exit=0`) **[R]** |
| Тесты не ослаблены | assert 262 → 262, `it(` 43 → 43, `.only`/`.skip` = 0 | `TSC-FIX.md §1/§3` **[R]** |
| Unit L-02-UI | 43 passed / 0 failed / 0 skipped | `waves/RW-40/QUALITY/TSC-FIX-test-green.log` (`skipped 0`, `exit=0`) **[R]** |
| md5 теста | `f2806f3f…` → `d30771d6…`, совпадает с ACCEPT L-02-UI | **[V]** |

**Оговорка, которую нельзя проглотить.** Зелёный `tsc` снят 2026-10-24 16:26. После него
правили **четыре** файла из `tests/**` (все в `include` корневого `tsconfig.json`):

| Файл | mtime | Дельта |
|---|---|---|
| `tests/unit/channel-paging.test.mjs` | 2026-10-30 16:13 | +154 строки |
| `tests/unit/guild-rail.test.mjs` | 2026-10-30 16:24 | +122 |
| `tests/unit/gui-spike-regressions.test.mjs` | 2026-10-30 16:24 | +80 |
| `tests/unit/read-state-ui.test.mjs` | 2026-10-30 16:24 | +22 |

То есть блокер **закрыт карточкой и её логом**, но **на текущих байтах не подтверждён**:
свежего `tsc` после 16:24 нет. По запрету ТЗ я его не запускал. До merge нужен один прогон
`npx tsc -p tsconfig.json` на замороженном дереве; это ~3 с, и он либо подтвердит 0, либо
даст новый список — оба исхода полезны.

### 3.2 `plan-sync` — §8 дополнен, файловая сверка сходится

`git diff TDD-PLAN.md` — **+10 строк** в §8 (`tests/meta/e2e-required.test.mjs`,
`tests/unit/dispatch_ui.test.mjs`, `tests/unit/search-remote-real-index.test.mjs`,
7 × `tests/e2e/m2-*.e2e.test.mjs`) **[V]**. Формат соблюдён (три колонки, имена в косых
кавычках).

Статическая сверка по алгоритму `tests/meta/plan-sync.test.mjs` (чтение файлов, без запуска
тестов) **[V]**:

```
rows 64, unique 64, dup []
unlisted (тест без строки §8) []
unknown  (строка §8 без файла) []
missing tokens []
```

Дополнительно: заявленный прогон meta — **102/102 pass** (`waves/RW-40/ORCA-WORKERS.md`,
строка PLAN-SYNC, «DONE (TL)») **[R]**. Лог именно этого прогона в репозитории отсутствует;
ближайший сохранённый — `QUALITY/QA-M2-04-verify-suite.log` с `101 pass / 1 fail` (красным был
`plan-sync` **до** патча §8). Рекомендация TL: сохранить лог 102/102 в
`waves/RW-40/QUALITY/`, иначе восстановление evidence — по памяти.

### 3.3 Гейт: артефакта на принимаемых байтах нет

| Артефакт | Commit | Вердикт | Покрывает |
|---|---|---|---|
| `.gate/worker-2026-10-24T10-22-26-400Z.json` (последний) | `0239543` | `passed_with_skips` (meta 65) | **ничего** из окна: старше L-02-CORE/UI, TSC-FIX, §8 и ACCEPT |
| `.gate/release-2026-10-24T09-37-37-616Z.json` (последний) | `0239543` | `passed_with_skips`: e2e 4 / pass 0 / skipped 4, workspace 12 skip | live-фазы нет, `acceptance`/`attempts` нет |

`acceptance.accepted`, `attempts[]`, `e2e.fixture` в worker-JSON отсутствуют — это
`gate:worker`, не `gate:release`. Условие 1 из `MERGE-POLICY §2` не выполнено и после
закрытия tsc/plan-sync: нужен прогон на замороженном дереве, с браузером для e2e
(`CHROME_BIN` в среде отсутствует).

---

## 4. Открытые пункты SEC-L02 (блокируют merge, а не паритет)

`QUALITY/SEC-L02.md` (снимок 2026-10-24 16:12): **P0 = 0, P1 = 1, P2 = 7, P3 = 10**.

| ID | Уровень | Суть | Состояние на 16:26 |
|---|---|---|---|
| SEC-L02-01 | **P1** | дедуп `s` не сбрасывается на новом `connect`; после logout→login READY теряется молча | `net.rs` `e13d4e8c…` **не менялся** → **open** **[V]** |
| SEC-L02-02 | P2 | `#[derive(Debug)]` печатает сырой `d` (`session_id`, `qos_token`) | open, deferred с тикетом |
| SEC-L02-03 | P2 | READY `d` (579 КБ, PII) уходит в webview целиком | open: нужно решение в `CONTRACTS/dispatch.md §5` |
| SEC-L02-04 | P2 | backpressure дропает любое событие, без приоритета READY | open |
| SEC-L02-05 | P2 | реальный `session_id` в тестах (`gateway_supervisor.rs:43`) | open (класс унаследован из HEAD) |
| SEC-L02-06 | P2 | redaction-evidence на пред-редактированных фикстурах | open |
| SEC-L02-07 | P2 | `connect` после терминального Stop — тихий no-op | open |
| SEC-L02-08 | P2 | evidence-цепочка не привязана к байтам | **частично снят**: ACCEPT L-02-CORE/UI пересняли md5 **[V]**; gate-артефакта по-прежнему нет |

Для merge важно ровно одно: **P1 нельзя тащить дальше** (`MERGE-POLICY §2`, «есть P0/P1 без
фикса → не готов к merge»; §4.6 — перенос P1→P2 только письменным waiver PM). P2 допустимо
`deferred` с тикетом, но SEC-L02-03/04 связаны с поведением UI при переполнении и потере
READY — их стоит решать вместе с SEC-L02-01, одним фиксом на `net.rs`.

---

## 5. Оставшиеся шаги до merge

Порядок обязателен: сначала freeze, потом прогоны, потом приёмка. Оценки — по объёму, не по
календарю.

| # | Шаг | Owner | Почему без него нельзя | DoD |
|---:|---|---|---|---|
| 1 | **Freeze**: остановить писателей (`app.js`, `lib.rs`, `quick-switcher.js`, `chat.js`, тесты), зафиксировать `git status -uall` | TL | ACCEPT L-02-UI уже разошёлся за 5 минут; любой прогон до freeze — по движущимся байтам | 0 изменений 15 мин, `md5sum` совпадает дважды |
| 2 | **Свежий `tsc` + targeted** на замороженном дереве (5 изменённых тестовых файлов) | G4 | зелёный лог от 10-24 не покрывает +378 строк тестов от 10-30 | `npx tsc` exit 0; targeted 43/43 + channel-paging/guild-rail/gui-spike-regressions/read-state-ui, 0 skip |
| 3 | **`gate:worker` + `gate:release`** с `CHROME_BIN` | G4/TL | условие 1 merge; последний артефакт старше окна | `acceptance.accepted === true`, `e2e.fixture === "met"`, `skipped === 0`, `attempts[]` в JSON |
| 4 | **Закрыть `SEC-L02-01`** (сброс дедупа на `connect`) red-first | G1 | P1 запрещает merge | тест «connect → dispatch → connect → dispatch(s=1)» красный до, зелёный после; `net.rs` новый md5 |
| 5 | Решения по SEC-L02-02/03/04/07 (+ `SEC-L02-17`, P3 — в тот же тикет) | PM+G1 | P2 допустимо deferred, но нужен тикет/waiver | тикет в `BUGS.md` или waiver PM, `CONTRACTS/dispatch.md §5` обновлён |
| 6 | **ACCEPT на остаток окна**: PLAT-11a, C-02-UI-OLDER, M-07-BE3 (после фикса P2), QA-M2-02 (после браузера), QA-M2-04 (после Actions) | PM (+Codex) | accepted set без них неполон | у каждого среза ACCEPT.md с md5 = дерево |
| 7 | **Перевыпуск/суперсид** пинов: `app.js` (G-01b UI), `PM/l02/ACCEPT.md` | PM | 1 файл разошёлся, старый ACCEPT 0/4 | строка «superseded by …» + md5-лист на freeze |
| 8 | **`MERGE-MANIFEST`**: файловый список с md5 (включая `rust/gui-spike/**`, `CONTRACTS/`, `tests/e2e/m2-*`, `scripts/gate.mjs`, `ci.yml`), ссылка D-128 → D-150 | PM/TL | манифест от 14:20 не знает ни L-02, ни G-01b (0 упоминаний) **[V]** | каждый файл integration имеет md5; §4 актуален |
| 9 | **`PARITY-MATRIX`**: `AT-01` («blocked»), `M-13` («4/7 фильтров»), `S-02` («STREAM [A]») — синхронизировать с `RE-DEEP-REA` | G6 Lead | матрица противоречит оракулу и трекеру | 3 строки переписаны, счётчики §14 пересчитаны |
| 10 | **Реестры**: `TASKS.md`, `QUEUE.md`, `STATE.md`, `waves/db.jsonl` (RW-40 = 0 событий), `BUGS.md` G6-D-03 | PM/TL | честность журнала — условие приёмки | `grep -c RW-40 waves/db.jsonl` > 0 |
| 11 | **Commit + merge** (L1): все dirty-пути на момент freeze (79+) + ACCEPT/логи | координатор | в git нет ни ACCEPT, ни `RE-DEEP-REA`, ни `TSC-FIX` | `git status` пуст, HEAD содержит ACCEPT-файлы |

После 1–5 снимается блок «гейт + P1». Шаги 6–11 — бумажная часть, но без неё merge нальёт
код, у которого нет md5-актуального ACCEPT: в этом проходе 8/8 успело стать 7/8 за три минуты.

**Что этим не достигается:** `parity-ok` и `qualify`. Для них нужен live N0 (READY + send +
отсутствие секретов), live-история/реакции/ack, `e2e.live = met`. E3 = 0, live-окна не было,
`DISCORD_LAB_LIVE_CDP` не задан **[V]**.

---

## 6. Что НЕ является блокером (чтобы не чинить лишнее)

1. **Продуктового P0 нет.** P0 = 0 в SEC-L02; выдуманных endpoint/полей G6 не нашёл.
2. **L-02-UI логика работает offline:** 43/43, 0 skip, 5/5 мутантов убиты
   (`PM/l02-ui/MUTATION.md`) **[R]**. Расхождение по `app.js` — привязка байтов, не дефект
   dispatch-канала.
3. **e2e-файлы на месте:** 10 `tests/e2e/*.e2e.test.mjs` — 7 `m2-*`, `cdp-browser`,
   `walk-fixture`, `live-discord`; fixture-набор собирается `find … ! -name 'live-*'`
   (9 файлов), meta-инвариант `e2e-required` зелёный, отсутствие браузера даёт infra fail,
   а не skip **[R]**.
4. **`waves-sync` не требует событий RW-40** — отсутствие журнала процессный долг, не красный тест.
5. **Заглушки STUB-12/13/14/27 разблокированы оракулом** — это снимает `blocked` в трекере, но
   не требует кода в этом окне.

---

## 7. Ограничения отчёта

1. Снимок **16:21–16:27**, дерево менялось во время прохода (48 → 79 dirty-путей; `app.js`
   правили в 16:26:16, чтение 16:26:01 попало в момент записи). Все md5 — точки; в таблице
   §2.1 намеренно две колонки времени.
2. `cargo`, `node --test`, `npx tsc`, `gate:*`, e2e, браузер, live — **не запускались**
   (запрет ТЗ). Числа тестов Rust/гейта — из ACCEPT, логов карточек и `.gate/*.json` **[R]**.
3. `plan-sync` подтверждён **статическим** разбором §8 (файлы+токены), а не прогоном теста;
   заявленные 102/102 — со ссылкой на `ORCA-WORKERS.md`, без сохранённого лога.
4. Карточки я не закрываю, waiver не выдаю, матрицу и манифест не правлю: владельцы — PM/TL.
   Правки этого прохода — только `waves/RW-40/PARITY/MERGE-READINESS-06.md` и
   `waves/RW-40/STUB-TRACKER.md`.

---

## 8. Как перепроверить (read-only, ~1 мин)

```bash
cd /home/sanya/orca/project
git log --oneline -1 && git status --porcelain -uall | wc -l
md5sum rust/gui-spike/src/settings_store.rs rust/gui-spike/tests/settings_contract.rs \
       rust/crates/discord-core/src/ws.rs rust/crates/discord-core/tests/gateway_supervisor.rs \
       rust/gui-spike/tests/net_contract.rs rust/gui-spike/ui/features/dispatch-model.js \
       rust/gui-spike/ui/app.js tests/unit/dispatch_ui.test.mjs     # сверить с §2.1
grep -c '^| `tests/' TDD-PLAN.md                                  # 64 строки §8
grep -c '"wave": "RW-40"' waves/db.jsonl                          # 0 — журнал не ведётся
ls -t .gate/*.json | head -1                                      # старше окна
grep -n "SEC-L02-01" QUALITY/SEC-L02.md                           # P1 всё ещё open
```

Красный флаг для приёмки: **любой** md5 из §2.1, отличающийся от ACCEPT, при отсутствии
строки «superseded by» — значит, наливаются непринятые байты.
