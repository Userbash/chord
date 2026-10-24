# MERGE-READINESS-05 — accepted set и блокеры merge после L-02

Дата: 2026-10-24, снимок **16:10–16:14 +05**. HEAD `0239543`, ветка `main`, дерево **dirty**
(`git status --porcelain -uall` → **1099** путей; свёрнутый вид — 117: 47 `M` + 70 `??`).
Воркер: **G6 Parity** (read-only). PM: DeepSeek.

> **Путь.** Корневых `PARITY/`, `PM/`, `QUALITY/` (в смысле ТЗ) нет: файлы карточек лежат в
> `waves/RW-40/{PARITY,PM,QUALITY}`, отчёт записан туда — как `G6-REPARITY-04`.
> Корневой `/QUALITY/` содержит только артефакты QA-M2-02/04 и SEC-L02 (12 файлов на 16:14).

Режим: единственная запись — этот файл. `rust/**`, `ui/**`, `tests/**`, `scripts/**`,
`.github/**`, гейт, e2e, браузер, CDP, сеть, Discord, live, `cargo`, `npm run gate:*` —
**не запускались и не менялись**. Запускались только read-only проверки: `md5sum`,
`npx tsc -p tsconfig.json` (noEmit), `node --test` на двух файлах (см. §1) — записей в
product/tests они не делают. `PARITY-MATRIX.md`, `GAP-BACKLOG.md`, `MERGE-MANIFEST.md`,
`MERGE-POLICY.md`, `TASKS.md`, `QUEUE.md`, `STATE.md`, `db.jsonl`, `TDD-PLAN.md` — не менялись.

Метки: **[V]** — проверено в этом проходе; **[R]** — из отчёта/лога карточки или гейта.
**E3 (live) = 0; ни один live-факт здесь не заявляется.**

---

## 0. Резюме

1. **Вердикт: merge — `block`; parity — `deferred`.** Не «merge-ready»: гейт на текущем
   дереве **красный** (проверено собственным прогоном, не по логам), accepted set не привязан
   к байтам, e2e не выполнен ни разу, live = 0.
2. **«2 accepted среза» подтверждены — и оба формально стухли.**
   `find waves/RW-40/PM -name 'ACCEPT*.md'` → ровно **2** файла: `PM/l02/ACCEPT.md`
   (L-02 backend, «ACCEPTED с условиями») и `PM/c02-be/ACCEPT.md` (C-02-BE). Но у обоих
   **md5 в ACCEPT ≠ md5 в дереве: 4/4 расхождения** (§2.1). По критерию merge (условие 2)
   это значит: **md5-актуальных accepted-срезов — 0 из 2**.
3. **Второй ACCEPT-контур неполон:** 10 срезов окна из `G6-REPARITY-04 §0.8` без
   `ACCEPT.md`, включая **L-02-CORE** (Codex `REVIEW-FIX` = ACCEPT, но ACCEPT.md PM нет) и
   **L-02-UI** (`REVIEW` = **Return**, `FIX-REPORT` 15:54 готов к повторному review,
   повторного review нет); плюс ещё 5 карточек вне этого списка — §2.
4. **Красный гейт подтверждён независимо:** `npx tsc -p tsconfig.json` → **exit 1, 16 ошибок,
   все в `tests/unit/dispatch_ui.test.mjs`** (файл самой карточки L-02-UI);
   `node --test tests/meta/plan-sync.test.mjs` → **exit 1**, в `TDD-PLAN.md §8` нет
   **10 тестовых файлов** (7 × `m2-*` + `e2e-required` + `dispatch_ui` + `search-remote-real-index`).
5. **e2e: файлы появились (15:58–16:04), прогонов 0.** 7 `tests/e2e/m2-*.e2e.test.mjs` +
   `fake-tauri.mjs`/`ui-fixture.mjs`/`e2e-required.test.mjs` есть **[V]**; браузера в среде
   **нет** (`CHROME_BIN` не задан, `chromium/chrome/brave` в PATH отсутствуют) **[V]**;
   новый гейт при отсутствии браузера даёт **failed**, а не skip **[V]** по `gate.mjs:354-378`.
6. **Дерево живёт прямо сейчас:** `settings_store.rs` 16:12:14 (`G-01b`), `chat.js` 16:12:19
   (`C-02-UI-OLDER`), новый `PM/g01b-contract/logs/{red,green}.log` 16:11–16:12; правились
   и доки: `QUALITY/QA-M2-04.md` 16:11, `QA-M2-02.md` 16:12, `SEC-L02.md` 16:13 **[V]**.
   Любой md5/ACCEPT в этом окне — движущаяся мишень; **сначала freeze, потом приёмка**.
7. **Integration set не определён файлово:** `MERGE-MANIFEST.md` (14:20) не упоминает ни
   L-02, ни SEARCH-FIX, ни QA-GATE-SKIPS, ни QA-M2-02/04, ни `CONTRACTS/`, и не содержит
   списка файлов/md5 — только таблицу функций. `rust/gui-spike/**` (**84 файла**) не в git
   вообще (`git ls-files rust/gui-spike` → 0). `.gate/**` в `.gitignore` → доказательства
   гейта не переносимы.
8. **Реестры расходятся:** в `waves/db.jsonl` **0 событий RW-40**; `TASKS.md` (15:04) не
   содержит ни L-01, ни L-02; `MERGE-POLICY §7` (12:45) — снимок до L-02; `MERGE-MANIFEST`
   ссылается на «окно D-128», тогда как актуальное решение — **D-150** (`POOL-04`).

---

## 1. Что проверено независимо (не «на слово»)

| Проверка | Команда/способ | Результат |
|---|---|---|
| md5 22 ключевых файлов vs `G6-REPARITY-04 §7` | `md5sum` | 20/22 совпали; **дрейф**: `settings_store.rs` `561a4fa1…`→**`ac58e876…`**, `chat.js` `2e918f3c…`→**`f38e4264…`** (16:12, писали writer'ы) **[V]** |
| typecheck (фаза гейта) | `npx tsc -p tsconfig.json` | **exit 1**, 16 × TS7006/TS2339/TS2345, все в `tests/unit/dispatch_ui.test.mjs` **[V]** |
| meta `plan-sync` (фаза гейта) | `node --test tests/meta/plan-sync.test.mjs` | **exit 1**, `actual` = 10 файлов не описаны в §8 **[V]** |
| L-02-UI unit | `node --test tests/unit/dispatch_ui.test.mjs` | **43 passed / 0 failed / 0 skipped** **[V]** (md5 `f2806f3f…` = `FIX-REPORT §4`) |
| ACCEPT-файлы | `find waves/RW-40/PM -name 'ACCEPT*.md'` | ровно **2** **[V]** |
| md5 в ACCEPT vs дерево | `md5sum` + `grep` | **4/4 ≠** в обоих ACCEPT **[V]** |
| браузер/live-среда | `command -v chromium…`, env | браузера нет; `CHROME_BIN`, `DISCORD_LAB_LIVE_CDP` — не заданы **[V]** |
| гейт-артефакты | `ls -t .gate/*.json`, JSON | worker `10:22:26Z` (15:22), release `09:37:37Z` (14:37); полей `e2e`/`acceptance`/`attempts[]` у worker **нет**; release e2e `4 / pass 0 / skipped 4` **[V]** |
| P-03 | `grep FORWARDED_DISPATCHES` | `[&str; 6]`, `TYPING` в `net.rs` = 0 **[V]** |
| M-07 | `grep badgeCount\|flags ui/**` | 0 вхождений **[V]** |
| C-07 | `grep dataset.guild` | `quick-switcher.js:67,144,150` — старый атрибут **[V]** |
| L-02 UI-подписка | `grep gateway://dispatch ui/**` | есть (`dispatch-model.js:16`, `app.js:1173`) **[V]** — вывод QA-MATRIX-02 «0 вхождений» устарел |
| integration set | `git status --porcelain -uall`, `git ls-files` | 1099 путей; `rust/gui-spike` — **0** в индексе **[V]** |

---

## 2. Срезы: accepted / returned / no-ACCEPT

Обозначения: **ACCEPTED** — есть `ACCEPT.md`; **REVIEW-OK** — независимый review сказал
accept, но `ACCEPT.md` нет; **RETURNED** — review вернул; **NO-ACCEPT** — есть код/логи, нет
ни review-accept, ни `ACCEPT.md`; **IN WORK** — writer ещё пишет.

| # | Срез | ACCEPT.md | Review | md5 = дерево | Статус |
|---:|---|---|---|---|---|
| 1 | **L-02 backend** (`PM/l02`) | **есть** (`ACCEPTED с условиями`) | PM-проверка по логам/md5 | **нет, 0/4** | **ACCEPTED (md5 устарел)** |
| 2 | **C-02-BE** (`PM/c02-be`) | **есть** (`ACCEPTED`) | PM-проверка по прогону | **нет, 0/4** (сам ACCEPT признаёт) | **ACCEPTED (md5 устарел)** |
| 3 | L-02-CORE (`PM/l02-core`) | нет | `REVIEW-FIX`: **ACCEPT**, P0/P1/P2=0 | н/д | **REVIEW-OK / PM-pending** |
| 4 | L-02-UI (`PM/l02-ui`) | нет | `REVIEW`: **Return** (2×P2); `FIX-REPORT` 15:54 = готово к re-review | н/д | **RETURNED (fix ждёт review)** |
| 5 | M-07-BE3 (`PM/m07-be3`) | нет | нет | н/д | **NO-ACCEPT** («готово к ревью») |
| 6 | SEARCH-FIX / QA-M2-03 (`PM/search-fix`) | нет | нет | н/д | **NO-ACCEPT** |
| 7 | QA-GATE-SKIPS / QA-M2-01 | нет | нет (meta 17/17 **[R]**) | н/д | **NO-ACCEPT** |
| 8 | QA-M2-02 (harness + 7 e2e) | нет | нет; отчёт сам: «M2 не принят» | н/д | **NO-ACCEPT (нужен браузер)** |
| 9 | QA-M2-04 (CI `qa-browser`) | нет | нет; self-verify 16:10–16:11 (`QA-M2-04-VERIFY`), Actions не запускался | н/д | **NO-ACCEPT** |
| 10 | C-02-BEFORE (`PM/c02-before`) | нет | нет | н/д | **NO-ACCEPT** |
| 11 | C-02-PAGE / C-02-UI | нет | нет | н/д | **IN WORK** (`chat.js` 16:12) |
| 12 | P-03 backend (`PM/p03`) | нет | нет (в отчёте «готово к приёмке») | н/д | **NO-ACCEPT** (D-accepted) |
| 13 | P-03-UI (`PM/p03-ui`) | нет | нет | н/д | **NO-ACCEPT** (D-accepted) |
| 14 | M-07-BE2 / M-07-UI | нет | нет | н/д | **NO-ACCEPT** (D-accepted) |
| 15 | G-05-UI / G-04-UI (`PM/g05-ui`) | нет | нет | н/д | **NO-ACCEPT** (D-accepted) |
| 16 | L-01 (`PM/l01`) | нет | нет (в отчёте нет строки статуса) | н/д | **NO-ACCEPT** (D-accepted), окна нет → **конфликт реестров** |

> **D-accepted** = `MERGE-MANIFEST §1` числит срез принятым по решению (D-xxx), но
> `ACCEPT.md` у него нет. Строки 5–16 — ровно тот разрыв, из-за которого «accepted set»
> читается по-разному: 2 файла ACCEPT против ~17 «accepted» в манифесте.

**Отдельно — baseline, который `MERGE-MANIFEST §1/§2` называет accepted** (G-01, C-02, G-03,
G-03a, G-04, G-05, M-07, P-03, PLAT-01/02, QA4-01a/b, QA4-02/03, DP-02, L-01, SEC-01/02,
Q-TOKEN/G1-01/Q-G2-P1/Q-G4-REG, V-08 UI, ST-01, PLAT-11, QA-IMPACT): приняты **решением
(D-xxx)**, `ACCEPT.md` у них нет, md5-пинов нет. Это третье, самое слабое определение
«accepted» — и именно оно даёт в манифесте «много принятого».

### 2.1 Два ACCEPT подтверждены — и оба не связаны с текущими байтами

| ACCEPT | Что зафиксировано | Сейчас | Расхождение |
|---|---|---|---|
| `PM/l02/ACCEPT.md` | `net.rs` `f21143f3…`, `main.rs` `0082fb4b…`, `net_contract.rs` `0d0ff80d…`, `dispatch.md` `58f678f9…` | `e13d4e8c…`, `336863b5…`, `fe8aa921…`, `4bfeef6c…` | **4/4** |
| `PM/c02-be/ACCEPT.md` (md5.txt) | `data.rs` `5d9119ba…`, `lib.rs` `f4c9c451…`, `data_contract.rs` `ffa9e6cb…`, `main.rs` `cbb5409a…` | `8013271d…`, `44a22d10…`, `e31c9127…`, `336863b5…` | **4/4** |

Дрейф объясним (файлы правили L-02-CORE, M-07-BE3, C-02-BEFORE и др.), но по
`MERGE-POLICY §2` («Evidence: md5 файлов») и условию 2 из `G6-REPARITY-04 §5.2`
(«md5 в ACCEPT = md5 в дереве») **ни один ACCEPT не покрывает текущие байты**: новые правки
в тех же файлах не имеют своего `ACCEPT.md`.

Отдельно: условие 3 из `PM/l02/ACCEPT.md` — «QA-мутация (обязательна до merge): убрать
вырезание `session_id` → тест обязан упасть». Есть лог мутанта по **`token`**
(`fix-red-oracle-token-mutant.log`: `left 2 / right 4`, caught) **[V]**, но лога мутации
именно по **`session_id`** нет (`grep` по `PM/*/*.log`) — условие выполнено **частично**.

### 2.2 Второй ревьюер L-02 (SEC-L02, снимок 16:12) — P1/P2 открыты

`QUALITY/SEC-L02.md` переписан в 16:13 (уже **после** `G6-REPARITY-04`): **P0 = 0, P1 = 1,
P2 = 7, P3 = 10** **[V]**. Для merge важно:

- `SEC-L02-01` (P1) — дедуп `s` не сбрасывается на новом `connect`; тот же вывод, что в §4;
  сброса в `net.rs` нет (`md5 e13d4e8c…` не менялся) **[V]**.
- `SEC-L02-17` (P2, новый) — на logout `dispatch.stop()` не очищает store и оставляет
  `data-dispatch-source="live"`; в связке с `SEC-L02-01` (потеря READY) UI может показывать
  прошлую сессию **[R]**.
- Это независимое подтверждение, что **L-02-UI не готов к ACCEPT** не только из-за
  типизации: по нему есть открытые P2 второго ревьюера, а `REVIEW-L02-UI` был Return.

Следствие: «L-02 закрыт» нельзя утверждать ни по backend (md5), ни по core (нет PM-ACCEPT),
ни по UI (Return + открытые P2). От L-02 зависят M-06/P-01/P-02/P-03/MB-01.

---

## 3. Integration set: что входит и чего не хватает

### 3.1 Размер и состав (по `git status -uall`, 1099 путей)

| Область | Файлов | Входит в integration | Комментарий |
|---|---:|---|---|
| `rust/gui-spike/**` | 84 | **да, обязательно** | **0 файлов в git** (`?? rust/gui-spike/`, не ignored) — весь продукт GUI; единственный слепок — md5 |
| `rust/crates/discord-core` | 19 | да | `ws.rs`, `gateway/`, `gateway_supervisor.rs`, `identify*`, `rest/*`, `Cargo.toml` |
| `rust/crates/discord-platform` | 3 | да | `keyring_offline_contract.rs` + тесты |
| `tests/**` | 45 | да | 20 unit, 8 e2e, 7 meta, 7 helpers, 2 smoke, 1 support |
| `src/**`, `scripts/**` | 9 | да | `core/ui/*`, `cli/run.mjs`, `gate.mjs`, `test-impact.mjs`, bench/probe |
| `.github/workflows/ci.yml` | 1 | да | job `qa-browser`, **не закоммичен** → Actions его не видит |
| `CONTRACTS/` | 2 | да | `dispatch.md`, `read-state.md` — контракт L-02/M-07 |
| `QUALITY/`, `QA/` | 13 | **да** (evidence) | `QA-M2-02/04`, `SEC-L02` — без них приёмка не воспроизводится |
| `TDD-PLAN.md`, `TEST-POLICY.md` | 2 | **да** | meta `plan-sync`/`test-policy` читают их |
| `docs/rust-client/*`, `docs/map.md` | 12 | да (доки) | STATUS/ROADMAP/PLAN/TEST-PLAN |
| `waves/RW-36…RW-40`, `waves/{STATE,db.jsonl}`, `TASKS.md` | ~900 | **да** (журнал) | RW-36…39 и весь RW-40 не закоммичены |

**Ловушка:** `git status --porcelain` без `-uall` показывает 117 строк — **982 файла
скрыты** внутри `??`-каталогов. Список integration, собранный свёрнутым `status`, потеряет
`rust/gui-spike/**` и почти все e2e/helpers. Проверять только `-uall`.

### 3.2 Чего физически нет / что не переносится

| Пробел | Факт | Последствие |
|---|---|---|
| `rust/gui-spike/**` не в git | 0 файлов в индексе **[V]** | merge «по git» не содержит продукт; нужен явный `git add` 84 файлов + md5-лист |
| `.gate/**` в `.gitignore` | `check-ignore` подтверждает **[V]** | JSON гейта — локальные; на другой машине/в CI их нет, `golden.manifest` тоже |
| `TDD-PLAN.md §8` | нет 10 файлов **[V]** | meta-фаза гейта красная навсегда, пока PM/TL не допишет строки |
| `QUALITY/` и `QA/` в корне | untracked | evidence-цепочка приёмки не входит в поставку по умолчанию |
| `ci.yml` | `M` (не закоммичен) | `qa-browser` не запустится до commit/push координатора |

### 3.3 Чего не хватает в `MERGE-MANIFEST.md` (14:20)

1. Нет **L-02** (ни backend, ни core, ни UI) — главного среза окна.
2. Нет `SEARCH-FIX` (QA-M2-03/M-13), `QA-GATE-SKIPS`, `QA-M2-02/04`, `M-07-BE3`,
   `C-02-BEFORE`, `CONTRACTS/dispatch.md`, `CONTRACTS/read-state.md`.
3. Нет **списка файлов и md5** — только таблица функций; координатор не может собрать
   integration и не может проверить, что налил именно принятые байты.
4. §4 устарел: «P-03 typing UI отсутствует» — модуль `typing-indicator.js` есть
   (`09f9f538…`), но входящий путь не форвардится (`net.rs`, 0 `TYPING`) → правильная
   формулировка «UI есть, live-путь отсутствует»; «C-02 — только первая страница» —
   backend `before`/`position` уже есть (`c02-before`), UI older — нет.
5. Окно описано как **D-128**, актуальное решение — **D-150** (`POOL-04`, 16:09);
   `DECISIONS.md` = `waves/RW-39/PM/DECISIONS.md`.

---

## 4. Условия merge: факт vs требование

| # | Условие (`MERGE-POLICY §2`, `G6-REPARITY-04 §5.2`) | Факт 16:12 | Статус |
|---:|---|---|---|
| **1** | **Gate-green на замороженном дереве**: `gate:worker` + `gate:release`, `acceptance.accepted === true`, `e2e.fixture === "met"`, `skipped === 0`, `typecheck`/`meta` зелёные, в JSON `attempts[]`/`ignored[]` | Последний worker 15:22, release 14:37 — **старше** L-02-CORE/FIX/UI-FIX/QA-M2-02. На текущем дереве **typecheck красный** (16 ошибок, прогнал сам) и **meta красная** (`plan-sync`, 10 файлов, прогнал сам). `acceptance`/`attempts` в JSON **нет**. Дерево **не заморожено** (2 writer'а активны) | **FAIL** |
| **2** | **Accepted set полон и привязан к md5**: у каждой карточки окна `ACCEPT.md`, md5 = дерево | 2 ACCEPT, оба 0/4; 10 срезов окна без ACCEPT; L-02-UI = Return; L-02-CORE = review-ok без PM-ACCEPT | **FAIL** |
| **3** | **E2E**: fixture-набор доказан браузером, skip ≠ pass | 7 сценариев + harness **есть** (15:58–16:04), прогонов **0**; браузера в среде нет → гейт даст **failed**; CI-job написан, но не закоммичен и ни разу не запускался | **FAIL** |
| **4** | **Parity**: матрица синхронизирована, нет `implemented` без offline-пути | 91 строка: **21 implemented / 36 partial / 34 absent**, `implemented`-де-факто-partial = 6 (C-02, C-07, M-06, M-07, M-13, G-01), `parity-ok` = 0, **E3 = 0** | **deferred** |
| **5** | **Live**: окно N0 (READY + send + секреты) | Окна не было; `DISCORD_LAB_LIVE_CDP` не задан; `SEC-L02-01` (P1, дедуп `s` не сбрасывается на `connect`) — **OPEN**, `net.rs` md5 не менялся | **FAIL (для qualify)** |
| — | **Реестры честные** (условие 3 из §5.2) | `SEC-L02` (снимок 16:12): **P0=0, P1=1** (`SEC-L02-01`), **P2=7** — в т.ч. новый `SEC-L02-17` (logout не очищает dispatch-store, UI может жить прошлой сессией); waiver'а нет, `net.rs` md5 не менялся. `BUGS.md:90` (G6-D-03) устарел; `MERGE-MANIFEST §4` устарел; `TASKS.md`/`QUEUE.md`/`STATE.md`/`db.jsonl` не содержат RW-40 | **FAIL** |

**Итог по условиям:** 0 из 4 обязательных для merge (1–3, реестры) выполнены;
parity — `deferred`; live — вне merge, но блокирует `qualify`.

---

## 5. Вердикт и три действия

### 5.1 Вердикт

| Вопрос | Ответ |
|---|---|
| **merge-ready?** | **Нет. `block`.** |
| parity | **`deferred`** (E3 = 0, L-02/M-07 partial, 6 `implemented` де-факто partial) |
| продуктовый блокер (P0/регресс/вымысел) | **Нет.** P0 = 0; обе красные фазы — тестовая типизация и книжная матрица тестов, не поведение `rust/**`/`ui/**`; L-02-UI unit **43/43** (проверено) |
| можно ли «принять по отчётам» | **Нет** — `MERGE-POLICY §2/§4`: приёмка по `counts` на замороженном дереве, не по verdict/отчёту |

Точная формулировка для координатора: **код окна в целом работоспособен offline, но
приёмочный контур (gate + ACCEPT + e2e) не замкнут; лить в integration сейчас — значит
налить 84+ файла GUI без единого md5-актуального ACCEPT.**

### 5.2 Три конкретных действия (в этом порядке)

**Действие 1 — «Freeze + green gate» (owner TL/PM, исполняет G4).**
Остановить code-writer'ов (сейчас активны G2 `chat.js` и G1 `settings_store.rs`), после
остановки — один прогон `gate:worker` и `gate:release` на неизменяемом дереве.
До прогона закрыть две красные фазы **в их собственных файлах**:
(a) `tests/unit/dispatch_ui.test.mjs` — 16 ошибок типизации: JSDoc-типы без изменения
assert'ов (ошибка на строке 522 — внутри **нового** теста B17, т.е. долг внесён фиксом
L-02-UI; `FIX-REPORT §5.5` «новых строк нет» — неверно);
(b) `TDD-PLAN.md §8` — 10 строк (7 × `m2-*`, `e2e-required`, `dispatch_ui`,
`search-remote-real-index`) — зона TL/PM, не воркера.
DoD: `acceptance.accepted === true`, `e2e.fixture === "met"`, `skipped === 0`, в JSON есть
`attempts[]`/`ignored[]`; md5 дерева до и после прогона совпадают.
Для e2e нужен браузер: либо `CHROME_BIN` на машине гейта, либо расширить `qa-browser`
до `gate:release` с выгрузкой acceptance-JSON (сейчас CI гоняет только файлы e2e и **не**
формирует приёмку release — это разрыв, а не «зелёный гейт»).

**Действие 2 — «ACCEPT-пакет на текущих байтах» (owner PM + Codex).**
Выпустить `ACCEPT.md` для: `l02-core` (после `REVIEW-FIX`, md5 `8703c1d6/7f23d85a/fe8aa921`),
`l02-ui` (после повторного review `FIX-REPORT` + свежий green 43/43 + полный `tsc`;
плюс решение по `SEC-L02-17`), `search-fix`, `gate-skips`, `m07-be3`; **перевыпустить**
привязку md5 в `PM/l02/ACCEPT.md` и `PM/c02-be/ACCEPT.md` под текущие байты (или письменно
зафиксировать «superseded by …»).
DoD: у каждого среза окна ACCEPT с md5 = дерево; ни один срез не входит «по отчёту»;
закрыто условие 3 из `PM/l02/ACCEPT.md` (мутация по `session_id`, не только `token`).

**Действие 3 — «Честные реестры и границы merge» (owner PM/TL).**
(a) `MERGE-MANIFEST` → добавить файловый список с md5 (включая `rust/gui-spike/**`,
`CONTRACTS/`, `tests/e2e/m2-*`, `scripts/gate.mjs`, `ci.yml`), обновить §4 и ссылку D-128 → D-150;
(b) закрыть или waiver'нуть **два** открытых пункта L-02: `SEC-L02-01` (P1, фикс red-first
в `net.rs` — сброс дедупа `s` на `connect`) и `SEC-L02-17` (P2, очистка dispatch-store на
logout) — P1 нельзя переносить в P2 без письменного waiver PM (`MERGE-POLICY §4.6`);
(c) переписать `BUGS.md` G6-D-03; синхронизировать `TASKS.md`/`QUEUE.md`/`STATE.md`/`db.jsonl`
(RW-40 = 0 событий) и закоммитить `ci.yml` (иначе Actions не увидит `qa-browser`).
DoD: `grep -c` по RW-40 в журнале > 0; в манифесте каждый файл integration имеет md5;
`SEC-L02-01` = closed|waiver; `SEC-L02-17` = closed|ticket.

### 5.3 Минимальный путь до `merge-ready` (оценка)

| Шаг | Объём | Что снимает |
|---|---|---|
| tsc 16 ошибок + 10 строк §8 | ~30–60 мин (2 файла, 2 владельца) | 2 красные фазы гейта |
| e2e-прогон в браузере (CI или локальный Chromium) | 1 прогон | `e2e.fixture = met` |
| 5 новых ACCEPT + 2 перевыпуска md5 | ~5 карточек | accepted set |
| `SEC-L02-01` фикс/waiver + `SEC-L02-17` решение + реестры | 1 карточка + доки | условие 3/реестры |
| freeze + `gate:worker` + `gate:release` | 2 прогона | условие 1 |

`parity-ok` этим **не** достигается: нужен ещё live N0 (READY+send+секреты), история/реакции/ack
live, G-05 10/10 или waiver, M-07/C-02/G-01b и `gate:qualify` с `e2e.live = met`.

### 5.4 Как перепроверить (read-only, ~2 мин)

```bash
cd /home/sanya/orca/project
git status --porcelain -uall | wc -l                    # 1099 (не 117: -uall обязателен)
find waves/RW-40/PM -name 'ACCEPT*.md'                  # ровно 2
npx tsc -p tsconfig.json; echo "tsc=$?"                 # 16 ошибок, exit 1
node --test tests/meta/plan-sync.test.mjs; echo "ps=$?" # 10 файлов не в §8, exit 1
node --test tests/unit/dispatch_ui.test.mjs             # 43/43
grep -c '"wave": "RW-40"' waves/db.jsonl                # 0
python3 - <<'PY'
import json,glob
for f in sorted(glob.glob(".gate/release-*.json"))[-1:]:
    d=json.load(open(f)); print(f, d["verdict"], sorted(d))
PY
```

---

## 6. Что НЕ является блокером (чтобы не путать)

1. **Продуктового P0 нет** — `SEC-L02.md §0`, вымышленных endpoint/полей нет
   (`G6-BATCH-MERGE-01 §3` PASS).
2. **L-02-UI фикс работает** на unit-уровне: 43/43, 0 skip, md5 совпадает с `FIX-REPORT §4`
   **[V]** — блокер в приёмке/типизации, не в логике пустого READY/self-ID.
3. **QA-GATE-SKIPS реализован по коду**: `failOnSkips`/`minPass`/`e2eLivePhase`/`attempts[]`/
   `computeAcceptance` в `gate.mjs` присутствуют **[V]**; meta-тест на них есть.
4. **7 e2e-сценариев написаны** и покрывают M2-01…M2-07; не хватает только исполнения.
5. **`waves-sync` не требует** событий RW-40 — отсутствие журнала это процессный долг,
   а не красный тест.

## 7. Ограничения

1. Снимок **16:10–16:14**, дерево менялось во время прохода (16:12 — `settings_store.rs`,
   `chat.js`; 16:11–16:13 — `QA-M2-02/04`, `SEC-L02`). Все md5 — точка, не фиксация; приёмка
   только после freeze. Числа `SEC-L02` (P0=0/P1=1/P2=7) — его собственный снимок 16:12.
2. `cargo`, `gate:*`, e2e, браузер, CDP, сеть, Discord, live — **не запускались**.
   Числа тестов Rust/гейта — из `.gate/*.json`, отчётов и логов writer'ов.
3. `tsc`/`plan-sync`/`dispatch_ui` прогнаны **сейчас** — это и есть единственные
   собственные прогоны; 43/43 относится к unit-уровню, не к end-to-end.
4. Отчёт **не** закрывает карточки, не выдаёт waiver и не меняет матрицу: владельцы —
   PM/TL. Вердикт `block` — следствие красного гейта и md5-пустого accepted set, а не
   продуктового дефекта.
