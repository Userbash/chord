# QA-MATRIX-02 — актуальная QA-матрица M2/T2/T3 после L-02

Дата: 2026-10-24. Роль: **G4 QA**. Задача: QA-MATRIX-02 (после L-02 и `QA-E2E-PLAN.md`).
Режим: **read-only**. Браузер, Chrome, CDP, Discord, live, сеть, `cargo`, `gate:*` не
запускались. Тесты, продуктовый код, `scripts/**`, `package.json` не менялись. Единственная
запись — этот файл.

Вход: `QUALITY/QA-STRATEGY.md`, `QA-E2E-PLAN.md`, `QA-AUDIT.md`, `BUGS.md`,
`PARITY/G6-SYNC-01.md`, `G6-RECHECK-01.md`, `MERGE-POLICY.md`, `MASTER-PLAN.md`,
`tests/e2e/*`, `tests/unit/*` (выборочно), `tests/support/gui-dom-harness.mjs`,
`.gate/*.json`, `scripts/gate.mjs`, `scripts/test-impact.mjs`, `.github/workflows/ci.yml`,
`PM/l02/ACCEPT.md`, `PM/l02-core/REPORT.md`, `PM/l02-ui/PLAN.md`.

Обозначения: **[V]** — проверено в этом проходе (чтение/grep/JSON/прогон без браузера);
**[R]** — взято из отчёта карточки или `.gate`. `skip` **никогда** не считается pass.

---

## 0. Снимок и краткий вывод

- `HEAD 0239543`, дерево dirty; `rust/gui-spike/**` и `discord-core/src/gateway/**` вне git —
  контроль только по md5 **[V]**.
- Свежий гейт: `worker-2026-10-24T10-22-26-400Z` (15:22 local) — `passed_with_skips`,
  meta 65, unit **651**, workspace **1104 + 12 ignored**, smoke 22; **e2e/coverage/keyring
  в worker-скоупе отсутствуют** **[V]**. Стартовал 15:21:42 и покрывает `net.rs`
  `e13d4e8c` (mtime 15:21:14) — замечание `G6-RECHECK-01` §0 о «правке новее зелёного
  прогона» **снято для core** **[V]**.
- **Но гейт уже не описывает дерево:** после 15:22:26 изменены 5 файлов —
  `discord-core/src/ws.rs` (15:28:50), `gateway_supervisor.rs` (15:27:30),
  `gui-spike/src/main.rs` (**15:29:03**, md5 `3e1ef0ea` ≠ `c003a9e5` из G6),
  `gui-spike/src/data.rs` (**15:28:56**, `a99647c1`), новый
  `gui-spike/tests/history_before_contract.rs` (15:28:24) **[V]**. Это признак активного
  писателя в этом checkout (карточка C-02-BEFORE: в `data.rs` появились
  `before`/`position: latest`/`hasOlder`). Снимок матрицы — 15:29, не фиксация.
- Последний **release**-гейт — `release-…T09-37-37-616Z` (14:37 local): `passed_with_skips`,
  e2e `pass 0 / skipped 4`, keyring-live 12/12 **[V]**. Он **старше** L-02 (15:03),
  L-02-CORE (15:21), C-02-PAGE (`chat.js` 14:59, `data.rs` 15:28) **[V]**.
- `L-02`: backend **принят** (`PM/l02/ACCEPT.md`), core **готов к приёмке, но не принят**
  (нет `PM/l02-core/ACCEPT.md`), **UI-потребителя нет**: `gateway://dispatch` в
  `rust/gui-spike/ui/**` — **0 вхождений** **[V]**.
- **7 M2-браузерных сценариев не существуют**: файлов `tests/e2e/m2-*.e2e.test.mjs`,
  `tests/helpers/ui-fixture.mjs`, `tests/fixtures/ui/*`, `fake-tauri.mjs`,
  `tests/meta/e2e-required.test.mjs` в дереве нет **[V]**. `QA-E2E-PLAN` §4 — план, не факт.
- E2E сейчас: `tests 4 / suites 3 / pass 0 / fail 0 / skipped 4` (32,9 мс, ни одного
  процесса браузера) **[V]**. В release-гейте это даёт `passed_with_skips` и **exit 0** [V].
- Найден **false-green** сценария 3: unit-тест `search_remote` зелёный, потому что DOM-шим
  подставляет `data-guild`, а реальный рейл — `data-guild-id`; `app.js:1206` читает
  `dataset.guild` → в продукте `guildId === ""` и серверный поиск не включается **[V]**.

**Вердикт M2: НЕ принят.** Причина не в количестве unit-тестов, а в том, что приёмочный
контур (7 browser-сценариев, браузерный CI, `failOnSkips`) не существует, а один из
сценариев в реальном UI сломан. P0-QA не найдено; P1-QA — **7** (§3).

---

## 1. Матрица: функция / unit / fixture-browser / live T3 / статус / owner

Статусы: `offline-pass` — зелёный unit/contract в гейте; `browser-absent` — harness нет;
`unmet` — проверка не выполнялась (нет среды/окна); `absent` — функции/теста нет;
`false-green` — unit зелёный, реальный путь сломан; `partial` — функция не end-to-end.

### 1.1 Семь сценариев M2 (приёмка milestone M2)

| # | Функция / сценарий | unit (offline) | fixture-browser | live T3 | Статус | Owner |
|---|---|---|---|---|---|---|
| 1 | Login → READY → rail | ✅ `gui-spike-auth` (46), `guild-rail` (8) [V: grep] | ❌ нет файла | ⚪ unmet (P0-01) | `offline-pass / browser-absent` | G4 (browser) + G1/G2 (продукт) |
| 2 | Канал → send/edit/delete/reply | ✅ `channel-switch` (9), `messages_contract` [R] | ❌ нет | ⚪ unmet (P0-02) | `offline-pass / browser-absent` | G4 + G2 |
| 3 | Search remote error → no fallback | ⚠️ зелёный **на шиме** (9 видов ошибок) [V] | ❌ нет | n/a (не нужен) | **`false-green`** (§1.3, P1) | G2 (фикс) + G4 (red-first browser) |
| 4 | Join invite → preview → accept → rail | ✅ `join-server` (24), `invites_contract` (14) [R] | ❌ нет | ⚪ unmet (G-03-LIVE) | `offline-pass / browser-absent` | G4 + G2 |
| 5 | Settings density/theme → **reload** | ⚠️ гидратация + save-whitelist есть; **reload-оракула нет** [V] | ❌ нет | n/a | `partial` (нет persistence-проверки) | G4 + G2 |
| 6 | Reconnect/offline → retry | ✅ `features` §L-01 [V] | ❌ нет | ⚪ опционально | `offline-pass / browser-absent` | G4 + G2 |
| 7 | Voice panel disabled/idle | ✅ `voice-ui` (33) [V: grep] | ❌ нет | ⚪ unmet (вне M2) | `offline-pass / browser-absent` | G4 + G5 |

Итог по M2: **0 из 7** доказаны браузером; **0 из 7** имеют `test id` сценария.
`skip 4/4` в текущем e2e относится к **другим** кейсам (CLI/CDP harness на мини-странице,
live Discord), а не к этим семи **[V]**.

### 1.2 Инфраструктура и зависимости приёмки

| Функция / контур | unit | fixture-browser | live T3 | Статус | Owner |
|---|---|---|---|---|---|
| E2E harness (реальный `ui/index.html` + fake `__TAURI__`) | — | ❌ файлов нет [V] | — | `absent` | G4 |
| 7 e2e-файлов `m2-*.e2e.test.mjs` | — | ❌ 0 файлов [V] | — | `absent` | G4 |
| Fixture-транспорт `tests/fixtures/ui/*` + `MANIFEST.json` | — | ❌ каталога нет [V] | — | `absent` | G1/G2 |
| Мета-инвариант «7 сценариев есть и не skip» | ❌ `tests/meta/e2e-required.test.mjs` нет [V] | — | — | `absent` | G4 |
| Gate: `failOnSkips` для e2e + split live | — | — | — | `absent` (e2e `retries:1`, без `failOnSkips`) [V] | PM/TL |
| Gate: `--test-concurrency=1` для e2e | — | — | — | `absent` [V] | PM/TL |
| Gate: сохранение обоих retry-прогонов | — | — | — | `partial` (в JSON только `note`, output первой попытки теряется) [V] | PM/TL |
| Browser CI-job (`qa-browser`, pinned Chromium) | — | ❌ `ci.yml` — только Rust (`ci.sh --online --skip-release`) [V] | — | `absent` | G5 |
| Impact selector | ✅ `test-impact.test.mjs` есть | ⚠️ неполный список (§3, QA-M2-05) [V] | — | `partial` | G4 |
| L-02 backend (канал/redaction/очередь, `net.rs`) | ✅ `net_contract` 22 [R] | n/a | ⚪ нужен для `sourceAvailable=true` | `accepted(backend)` | G1 |
| L-02-CORE (хук кадра в ядре) | ✅ targeted green [R] | n/a | ⚪ | `ready-to-accept` (ACCEPT.md нет) | G1/PM |
| L-02-UI (подписчик + 6 проекций) | ❌ `dispatch_ui.test.mjs` нет [V] | ❌ | ❌ | `absent` (контракт `PM/l02-ui/PLAN.md` есть) | G2 |
| M-06 входящие реакции | ⚠️ свои — да, входящие — нет [R] | ❌ | ⚪ | `partial` (de facto) | G2+G1 |
| M-13 / R-01 серверный поиск | ⚠️ false-green [V] | ❌ | ⚪ | `partial` | G2 |
| P-03 входящий `TYPING_START` | ❌ не форвардится [R] | ❌ | ⚪ | `partial` | G1+G2 |
| C-02 paging (older/tail) | ⚠️ `channel-paging` (7) [V: grep]; **C-02-BEFORE в работе** (новый `history_before_contract.rs`, вне гейта) | ❌ | ⚪ | `partial` (forward есть, `before` не принят) | G1+G2 |
| G-01b restore `lastGuildId` | ❌ нет | ❌ | ⚪ | `absent` | G1+G2 |
| M-07 unread/badge | ⚠️ только mentions | ❌ | ⚪ | `partial` | G1+G2 |
| G-05 меню сервера | ⚠️ 2 из 10 | ❌ | ⚪ | `partial` | G2+G3 |
| Fidelity «шим ↔ реальный `index.html`» | ❌ мета-теста нет [V] | — | — | `absent` → причина §1.3 | G4 |

### 1.3 Почему сценарий 3 — `false-green` (цепочка [V])

1. `tests/support/gui-dom-harness.mjs:810-821` создаёт три статичные плитки с
   `button.dataset.guild = guild` (атрибут `data-guild`).
2. `rust/gui-spike/ui/app.js:1206`: `guildId: selectedGuild?.dataset.guild ?? ""`.
3. Реальный рейл G-01 строит плитки с `data-guild-id` (`app.js:489`) и
   `dataset.guildId` (`app.js:492`), атрибута `data-guild` в `index.html` нет
   (это отдельно фиксирует `guild-rail.test.mjs` §«нет статичной заглушки»).
4. В реальном boot рейл пуст (плитки появляются только после `ready()`), поэтому
   `selectedGuild === null` → `guildId = ""`.
5. `search-overlay.js:883`: `if (invoke === null || guildId === "") return;` —
   `search_remote` не вызывается. Ошибка remote-поиска на реальном экране недостижима.
6. Unit остаётся зелёным, т.к. `withRemoteApp` поднимает шим с `data-guild="1"` и
   проверяет `args.guildId === "1"` [V].

Следствие: **browser E2E сценария 3 обязан быть red до фикса G2**; текущий unit
не является доказательством сценария (оракул подменён шимом, а не `index.html`).

---

## 2. Семь M2-сценариев: есть/нет, skip/pass, оракул, что нужно для pass

| # | Есть сейчас | Прогон | Оракул (независимый) | Чего не хватает для `pass` |
|---|---|---|---|---|
| 1 | unit ✅, browser ❌ | unit pass; browser — файла нет (не skip, а отсутствие) | `fixtures/gateway/by-type/002-READY.json` — 7 гильдий, порядок, `properties.name`, `member_count` **[V: 7]** | harness (реальный `index.html`), fake `__TAURI__` с `auth_status`/`ready`/`channels`, assert: гейт скрыт, 7 `button.guild-item[data-guild-id]`, первая выбрана, `auth-token.value === ""`, `ready()` только после auth |
| 2 | unit ✅, browser ❌ | unit pass; browser — нет | READY (272 канала) + `*-MESSAGE_CREATE.json` (реальные `id`/`content`), `channel-switch.test.mjs` уже использует их как оракул **[V]** | browser: выбрать гильдию `1014537851418980372` → канал `1163408591315812392`, реальные `channelId`/`messageId` в `send/reply/edit/delete` (не `"news"`/индекс), цитата reply, видимая ошибка send |
| 3 | unit ⚠️ false-green, browser ❌ | unit pass (шим); browser — нет | таблица `kind`: `unauthorized/forbidden/notFound/client/server/rateLimited/network` + счётчик вызовов **[V: features §B7b, G1-01]** | **сначала фикс G2** (`data-guild-id`), затем browser: `.search-status[data-state=error]`, `matchesCount === 0`, **ноль** локальных результатов, **ровно один** `search_remote` с реальным `guildId` |
| 4 | unit ✅, browser ❌ | unit pass; browser — нет | `resolve_invite`/`accept_invite`/`join_guild` контракт (`invites_contract` 14) + preview-поля из fake-ответа | browser: preview (имя/участники/срок), после accept — success + новая плитка, ровно один вызов на шаг, 403/429/401 видимы, retry не дублирует accept |
| 5 | unit ⚠️ partial, browser ❌ | unit pass (гидратация/save), **reload не проверен** [V] | `settings_load` payload `{version:1,toggles,density,theme,locale}`; density-оракул `tokens.css:361` (`--message-gutter:80px`) [R] | browser: клик `compact`/`light` → `html[data-density]` + `html[data-theme]` и `aria-pressed`; `Page.reload` → fake-store вернул сохранённое; атрибуты восстановлены; в `settings_save` нет чужих ключей/секретов |
| 6 | unit ✅, browser ❌ | unit pass; browser — нет | `net_status` сценарий `ready → reconnecting → offline` + `net_connect` 200/401 (`features` §L-01) **[V]** | browser: баннер `.net-status` видим/скрыт по состояниям, 401-текст, ровно один `net_connect` на клик, нет retry-loop; live-разрыв — опционально |
| 7 | unit ✅, browser ❌ | unit pass; browser — нет | `voice-ui` контракт: `data-state="disabled"` без сервиса, `aria-disabled`, join-причина, ноль IPC/audio | browser: `data-state=disabled` на реальном `index.html`, клик join не создаёт сессию, ноль `send_typing`/network/audio; live-аудио — отдельный T3 |

Правило статуса для этой таблицы: «файла нет» ≠ skip. Skip — это «среда отсутствует,
но тест существует». Отсутствие теста — **невыполненная работа**, а не среда.

---

## 3. P0/P1 QA-дефекты

P0 (security/live) в этом проходе **не найдено**. Отсутствие live-подтверждения
(P0-01/02/03) — это **unmet-гейт**, а не дефект кода; так и помечено в §4.

### 3.1 P1

| ID | P | Область | Симптом / repro | Evidence | Owner | Что закроет |
|---|---|---|---|---|---|---|
| **QA-M2-01** | P1 | Gate/e2e | `skipped 4` при `pass 0` даёт `passed_with_skips` и **exit 0**; M2 можно «принять» без единого браузерного assert. Это прямо запрещено `QA-STRATEGY` §8 и `MERGE-POLICY` §2 | `.gate/release-…T09-37-37-616Z.json`: e2e `pass 0 / skip 4`, verdict `passed_with_skips`; `scripts/gate.mjs:430` (нет `failOnSkips`), `:463` (verdict) **[V]** | PM/TL (гейт) + G4 | `e2e.failOnSkips=true` для fixture-набора, отдельная live-фаза, приёмка по `counts`, а не по verdict |
| **QA-M2-02** | P1 | E2E scope | Приёмочных 7 browser-сценариев нет; существующие 4 e2e проверяют CLI на `tests/fixtures/app/index.html`, а не продукт | нет `tests/e2e/m2-*.e2e.test.mjs`, `tests/helpers/ui-fixture.mjs`, `tests/fixtures/ui/**`, `fake-tauri.mjs`, `tests/meta/e2e-required.test.mjs` **[V]** | G4 (+G1/G2 fixture) | harness + 7 файлов + meta-инвариант «7 существуют и не skip» |
| **QA-M2-03** | P1 | UI/search | `search_remote` не включается в реальном приложении: `dataset.guild` vs `data-guild-id`; unit зелёный (шим подменяет атрибут) → false-green | `app.js:1206`, `app.js:489/492`, `search-overlay.js:883`, `gui-dom-harness.mjs:810-821`, `features.test.mjs:2025-2054` **[V]** | G2 (фикс) + G4 (red-first browser) | фикс на `data-guild-id` (+fallback) + regression-тест на реальном `index.html`; запись в `BUGS.md` как R-01/M-13 |
| **QA-M2-04** | P1 | CI | В CI нет ни одного Node-теста и нет browser-job: unit/meta/smoke/e2e проверяются только локально | `.github/workflows/ci.yml` — 2 job'а, оба Rust (`ci.sh --online --skip-release`) **[V]** | G5 | `qa-browser` job: pinned Chromium, `CHROME_BIN`, preflight «нет браузера → fail», `--test-concurrency=1`, artifacts `.gate` |
| **QA-M2-05** | P1 | Test impact | Селектор занижает набор: `app.js` → 9 frontend-файлов, но **не** `chat-reactions`, `join-server`, `read-state-ui`, `typing-ui`, `channel-paging`; `chat.js` → не запускает `chat-reactions` (реакции!) | `scripts/test-impact.mjs:10-20,96-107`; прогон `--changed ui/app.js` и `--changed ui/features/chat.js` **[V]** | G4 | профильные ветки: `chat.js → chat-reactions/read-state-ui`, `search-overlay.js → features B7b`, `app.js → +join-server/chat-reactions/channel-paging`, новый `dispatch_ui` |
| **QA-M2-06** | P1 | L-02 end-to-end | L-02 не end-to-end: backend принят, core без ACCEPT, UI-потребителя нет. `POST`/события не доходят до экранов; M-06 входящие, P-02/P-03 входящий остаются partial | `ui/**`: `gateway://dispatch` = 0, `net_dispatch_stats` = 0 **[V]**; `PM/l02-core/` без `ACCEPT.md` **[V]** | G1/PM (accept) + G2 (UI) | приёмка core + `dispatch_ui.test.mjs` (A1-A3, B1-B15, C1-C14, D1-D5) + `data-dispatch-source=absent` при `sourceAvailable=false` + QA-мутация redaction |
| **QA-M2-07** | P1 | Test fidelity | Нет мета-инварианта «DOM-шим ↔ реальный `index.html`»: шим содержит id/атрибуты, которых в продукте нет (`data-guild`, статичные 3 плитки, `guild-item.is-selected` на boot) | `gui-dom-harness.mjs:810-821` **[V]**; последствие — QA-M2-03 | G4 | meta-тест: каждый используемый в шиме id/атрибут существует в `index.html` или создаётся кодом; расхождения — fail |

Отдельно: `BUGS.md` держит e2e-проблему как **Q-09 P2**. По `MERGE-POLICY` §2 e2e-skip
блокирует merge/приёмку, поэтому предлагаю **повысить Q-09 до P1** либо разделить:
«skip как pass в гейте» (P1) и «нет fixture-прогона» (P2). Файл `BUGS.md` — вне моей зоны
записи; это рекомендация PM.

### 3.2 P2/P3 (в моей зоне — только фиксация)

| ID | P | Симптом | Evidence |
|---|---|---|---|
| QA-M2-08 | P2 | Retry теряет первый прогон: `attempts[]` не пишется в JSON, остаётся `note: «первый прогон: fail»` | `scripts/gate.mjs:340-375` **[V]** |
| QA-M2-09 | P2 | `workspace-rust` skip 12 смешан с e2e-skip в одном verdict `passed_with_skips` — разные причины, одна строка | `.gate/worker-…T10-22` / `release-…T09-37` **[V]** |
| QA-M2-10 | P2 | `BUGS.md` содержит дубли ID с противоречивым статусом: `Q-01` (open+closed), `G2-03` (open+closed), `SEC-02` (open+closed), `PLAT-11` (две разные строки) | `BUGS.md:14/40, 49/58, 66/76, 69/71` **[V]** |
| QA-M2-11 | P2 | `QA4-01a` числится «in work», хотя `cdp eval/clicktext/list/shot` в CLI есть (блокер — браузер, а не код) | `BUGS.md:62` vs `src/cli/run.mjs` **[R]** |
| QA-M2-12 | P2 | `G6-D-03` evidence неверен для `ready`/`join_guild` (вызываются), `enter_guild` — alias | `G6-RECHECK-01` §4 **[R]** |
| QA-M2-13 | P3 | Журнал RW-40: `waves/db.jsonl` — 0 событий RW-40 | `grep -c "RW-40" waves/db.jsonl` = 0 **[V]** |

Открытые P1 из `BUGS.md`, которые напрямую влияют на M2/T3 (не мои, но в матрице
учитываются): `QA4-03` (source-scrape `token_or` вместо поведения), `PLAT-11/11a`
(мета-инвариант «IPC не принимает путь» **уже реализован** — `tests/meta/ipc-path.test.mjs`,
но строка в `BUGS.md` не закрыта; 11a — валидация `channelId` в `messages.rs`), `Q-02`/`Q-04`
(размеры модулей), `G-04` UI (backend closed, UI queued), `PLAT-10` (mute/deafen только aria).

---

## 4. T2 release и T3 live: чек-листы

### 4.1 T2 — release / M2 acceptance (fixture, без live)

Предусловия: Node ≥ 24 (`v24.21.0` **[V]**), `CHROME_BIN` указывает на исполняемый
Chromium, live-env **не задан**, один browser-job на runner, один `cargo` на target-dir.

```sh
# 0. Preflight: отсутствие браузера — infra fail, а не skip
test -x "$CHROME_BIN" || { echo "FAIL: CHROME_BIN не исполняется"; exit 1; }
node --version

# 1. Минимальный M2-набор: строго последовательно
CHROME_BIN=/usr/bin/chromium node --test --test-concurrency=1 \
  tests/e2e/cdp-browser.e2e.test.mjs \
  tests/e2e/walk-fixture.e2e.test.mjs \
  tests/e2e/m2-*.e2e.test.mjs

# 2. Полный release-гейт
CHROME_BIN=/usr/bin/chromium npm run gate:release

# 3. Машинная приёмка: pass, а не verdict
jq -e '.phases[] | select(.name=="e2e") |
  (.ok==true) and (.counts.fail==0) and (.counts.skipped==0)' .gate/release-*.json
jq -e '.verdict=="passed"' .gate/release-*.json
```

- [ ] `typecheck`, `ui-tsc`, `meta`, `unit`, `spike-rust`, `workspace-rust`, `smoke`
- [ ] `e2e`: `fail=0`, `skipped=0`, `pass ≥ 9` (2 harness + 7 M2) — **сейчас `pass 0 / skip 4` [V]**
- [ ] live-файл **исключён** из fixture-фазы e2e (иначе `live-discord` даст `skip 2` и «честный» `skipped=0` недостижим)
- [ ] `coverage` ≥ порогов (`lines 90 / functions 80 / branches 90`)
- [ ] `golden` — `md5sum -c .gate/golden.manifest` чисто
- [ ] `keyring-live` 12/12, `skipped=0` (Secret Service)
- [ ] `workspace-rust` skip 12 — **объяснён** (ignored-набор), не смешан с e2e-skip
- [ ] verdict строго `passed`; `passed_with_skips` — **не** приёмка
- [ ] гейт снят на **финальном** дереве (сравнить mtime/md5 с `net.rs`, `app.js`)

Блокеры T2 сейчас: нет браузера **[V]**, нет 7 сценариев/harness **[V]**, e2e без
`failOnSkips` **[V]**, CI не запускает Node **[V]**, release-гейт старше четырёх срезов **[V]**,
и **дерево меняется во время прохода** (`data.rs`/`main.rs`/`ws.rs` новее 15:22) — гейт нужно
снимать после остановки писателя, иначе он снова «про вчерашнее дерево» **[V]**.

### 4.2 T3 — live (только согласованное окно PM)

Предусловия: Discord запущен с `--remote-debugging-port=9223`, **тестовый** аккаунт,
владелец окна — PM, таймбокс и pre/post-проверка, токен не попадает в артефакты.

```sh
DISCORD_LAB_LIVE_CDP=http://127.0.0.1:9223 node bin/discord-lab.mjs cdp targets
DISCORD_LAB_LIVE_CDP=http://127.0.0.1:9223 node --test tests/e2e/live-discord.e2e.test.mjs
DISCORD_LAB_LIVE_CDP=http://127.0.0.1:9223 DISCORD_LAB_KEYRING_LIVE=1 npm run gate:qualify
```

| # | Пункт T3 | Evidence (команда, время, результат — без токена) | Владелец |
|---|---|---|---|
| 1 | Реальный login/READY, гильдии/каналы | `cdp targets` + UI-снимок; 7 гильдий READY vs экран | PM+G1+G4 |
| 2 | send/edit/delete/reply в **тестовом** канале | id сообщения и результат; 401/429 отдельно | PM+G4 |
| 3 | ack/unread, badge (M-07) | ack 200 + сброс полосы | G2+G4 |
| 4 | Входящие реакции из второго клиента (M-06), REMOVE — `[A]` | два клиента, ADD/REMOVE, счётчик | G2+G4 |
| 5 | invite: resolve/accept/TTL/USER_BANNED (G-03-LIVE) | одноразовый код, время истечения | G1+G2 |
| 6 | reconnect: resume/close-коды (L-01) | сеть/WS, баннер, отсутствие retry-loop | G1+G2 |
| 7 | voice join/leave (если в scope M2) | устройства/PipeWire, ноль утечек PCM/ключей | G5 |
| 8 | keyring lock/unlock + `gate:qualify` | 12/12, `skipped=0` | G4b |
| 9 | Redaction: `grep -R "token\|password\|key=" артефакты` = 0 | скан после окна | G4b |

Правила статуса T3: endpoint не задан → **unmet** (не pass и не skip); endpoint задан, но
недоступен → **fail**; один pass после одного fail → **flaky/inconclusive** (нужны оба лога).
`gate:release` не заменяет live: fixture доказывает wiring против стаба, но не доставку.

### 4.3 Что можно проверить **сейчас**, без Chrome и live

| Проверка | Как | Результат этого прохода |
|---|---|---|
| Учёт e2e (skip vs pass) | `node --test 'tests/e2e/**/*.test.mjs'` | `pass 0 / skip 4` **[V]** |
| Арифметика гейта | `jq` по `.gate/*.json` (e2e/keyring/workspace) | release 14:37: e2e 0/4; keyring 12/12 **[V]** |
| Наличие потребителя L-02 | `grep -rn "gateway://dispatch" rust/gui-spike/ui` | 0 **[V]** |
| Наличие M2-файлов | `ls tests/e2e tests/fixtures/ui tests/helpers` | m2/ui-fixture/fake-tauri нет **[V]** |
| False-green сценария 3 | сверка `app.js:1206` ↔ `data-guild-id` ↔ шим | подтверждён **[V]** |
| Полнота impact selector | `node scripts/test-impact.mjs --changed …` (dry-run) | app.js/chat.js занижены **[V]** |
| Секреты в артефактах | grep по `.gate`, `waves/RW-40/PM` | 0 «сырых» токенов; фикстуры `REDACTED` **[V]** |
| Целостность оракула READY | `jq '.d.guilds | length' fixtures/.../002-READY.json` | 7 гильдий **[V]** |
| Offline-слои | `npm run typecheck`, `node --test tests/meta`, `tests/unit` | не запускались здесь; зелёные в worker-гейте 15:22 **[V: JSON]** |
| Стабильность md5 | `md5sum` ключевых файлов | `app.js 78039af3`, `net.rs e13d4e8c` **[V]** |

---

## 5. Риски: flaky, параллельный Chrome, секреты в артефактах

### 5.1 Flaky

| Риск | Механика | Мера |
|---|---|---|
| Медленный старт браузера | `launchBrowser` ждёт `DevToolsActivePort` 15 с; CLI-вызов ограничен 20 с (`helpers/cli.mjs`) | ждать предикат (`waitFor`), а не sleep; диагностика (stderr браузера, порт-файл); таймауты harness конфигурируемы |
| Retry скрывает нестабильность | `attempts[]` не сохраняется, в JSON только `note` **[V]** | писать оба прогона и оба лога; первый fail помечать `flaky`, не «зелёным» |
| Нестабильные ожидания UI | реальный `index.html` грузит модули/шрифты, `reload` меняет timing | `waitFor` по атрибуту/DOM, а не по времени; фиксировать версию Chromium в отчёте |
| `file://` и storage | разные Chromium по-разному ведут себя с `sessionStorage`/`window.name` | fake-store с явной проверкой доступности; pinned-версия в CI |
| 12 ignored в Rust | смешиваются с e2e-skip в одном verdict | отдельная строка/warning с причиной; не считать «пропуском среды» |

Правило: **1 повтор**, оба результата сохраняются; повтор не превращает fail в pass.

### 5.2 Параллельный Chrome

| Риск | Механика | Мера |
|---|---|---|
| `node --test` запускает файлы параллельно | gate не задаёт concurrency для e2e **[V]** | `--test-concurrency=1` для всего e2e; max 1 Chrome на runner |
| Несколько профилей/портов | каждый тест создаёт `user-data-dir`, но при параллели запуске возможны гонки | уникальный `user-data-dir`, отдельный порт, чистка в `t.after` |
| Осиротевшие процессы | `stopBrowser` делает `SIGKILL` и не ждёт `exit` | ждать `close` с таймаутом, kill process group, проверять `/tmp` после прогона |
| Двойной запуск с `gate:release` | browser-suite + гейт одновременно грузят CPU/RAM | не запускать e2e параллельно с гейтом; по `AGENTS.md` — максимум 2 фоновых процесса, 1 `cargo` |
| Неоднозначный CDP-target | `CDP_TARGET=index.html`, `pickTarget` берёт первую подходящую страницу | отдельный браузер на fixture, уникальный title/URL, проверка `targets` до действий |

### 5.3 Секреты в артефактах

| Риск | Факт/механика | Мера |
|---|---|---|
| Токен в фикстурах | `002-READY.json`: `session_id`, `resume_gateway_url`, `auth_session_id_hash`, `analytics_token`, `qos_token` = `REDACTED` (len 8) **[V]** | фикстуры остаются redacted; любой новый дамп — через redaction-скрипт, не «как есть» |
| Токен в логах/`cdp eval` | `.gate/*.json` хранит `tail` stdout; live `eval` может вернуть токен | запрет `eval` произвольных выражений в T3; redaction в harness; скан после окна |
| Токен в артефактах отчётов | скан `.gate` + `waves/RW-40/PM` на `mfa.`/3-part token-паттерн → **0** **[V]** | сохранить правило: evidence = команда/время/результат, **без** значения токена |
| Keyring/Secret Service | `gate:qualify` поднимает `DISCORD_LAB_KEYRING_LIVE=1` | изолированный тестовый keyring-профиль; не логировать `secret-tool` вывод |
| `rust/gui-spike/**` вне git | дифф недоступен, только md5 | фиксировать md5 до/после в отчёте карточки; md5 не заменяет тест |
| Golden `logs/`, `functions/`, `evidence/` | реальные пользовательские данные, read-only | T3-артефакты писать **только** в `waves/<ID>/`/`tmp`, не в golden |

---

## 6. Evidence этого прохода (что именно запускалось)

```text
$ node --version
v24.21.0
$ env -u CHROME_BIN -u CHROME_PATH -u DISCORD_LAB_LIVE_CDP \
    node --test 'tests/e2e/**/*.test.mjs'
tests 4 | suites 3 | pass 0 | fail 0 | skipped 4 | duration_ms 32.9
$ command -v chromium/chrome/brave/edge   → ничего; CHROME_BIN/CHROME_PATH unset
$ grep -rl "gateway://dispatch" rust/gui-spike/ui → 0 файлов
$ jq '.phases[] | {name,counts,note}' .gate/release-…T09-37.json → e2e pass 0 skip 4
$ node scripts/test-impact.mjs --changed rust/gui-spike/ui/app.js       → 9 frontend-файлов
$ node scripts/test-impact.mjs --changed rust/gui-spike/ui/features/chat.js → без chat-reactions
$ jq '.d.guilds|length' fixtures/gateway/by-type/002-READY.json → 7
$ grep -rIoE "(mfa\.[A-Za-z0-9_-]{20,}|[A-Za-z0-9_-]{24}\.[A-Za-z0-9_-]{6}\.[A-Za-z0-9_-]{27,})" \
    .gate waves/RW-40/PM | wc -l → 0
$ jq -r '.d | {session_id, resume_gateway_url}' fixtures/gateway/by-type/002-READY.json
    → "REDACTED", "REDACTED"
$ find rust tests -newermt '2026-10-24 15:22:26' -type f | grep -v target
    → ws.rs, gateway_supervisor.rs, main.rs, data.rs, history_before_contract.rs
$ md5sum rust/gui-spike/src/net.rs rust/crates/discord-core/src/ws.rs \
           rust/gui-spike/src/main.rs rust/gui-spike/src/data.rs
    → net.rs e13d4e8c…, ws.rs 176791cc…, main.rs 3e1ef0ea…, data.rs a99647c1…
```

Ключевые md5 (снимок, не фиксация): `app.js 78039af36b09fa94e5d92b207f5ce977`,
`search-overlay.js 1b71bcc0ae3f2e2074e6d04794d7fac9`, `net.rs e13d4e8cb4fbb22af2ce6d96702f0e8a`,
`gui-dom-harness.mjs 8e26df0623a00504fc92e3ed7b01ead8`, `gate.mjs c8a4875ed7d7a64d9dad3a215cad9a8f`.

Не выполнялось: браузер, CDP, Discord, live, сеть, `cargo`, `gate:*`, мутации. Числа
unit/meta/smoke/workspace — из `.gate/*.json`, а не из собственного прогона.

---

## 7. Вердикт и следующая точка

- **M2 (QA handoff): не принят.** Нет 7 browser-сценариев, нет harness/CI, gate
  принимает `skip` как `passed_with_skips`, сценарий 3 false-green, L-02 не end-to-end.
- **P0-QA: 0. P1-QA: 7** (QA-M2-01…07). P1 из `BUGS.md`, влияющие на M2/T3, остаются
  открытыми (`QA4-03`, `PLAT-11*`, `Q-02`, `Q-04`, `G-04` UI, `PLAT-10`).
- Минимальная последовательность снятия блокеров:
  1. **G2**: фикс поиска `data-guild-id` (+ fallback) — red-first browser-тест.
  2. **G1/G2**: fixture-транспорт `tests/fixtures/ui/*` + `fake-tauri.mjs` с журналом IPC.
  3. **G4**: harness (`ui-fixture.mjs`) + 7 `m2-*.e2e.test.mjs` + meta «7 и не skip».
  4. **PM/TL**: gate-патч (`failOnSkips`, split live, `--test-concurrency=1`, оба retry-лога).
  5. **G5**: `qa-browser` CI-job с pinned Chromium и preflight «нет браузера → fail».
  6. **PM/TL**: свежий `gate:release` на финальном дереве → затем отдельное T3-окно.
- Точка пересмотра: после приёмки **L-02-CORE + L-02-UI** и release-гейта, где
  `e2e.pass ≥ 9, skipped = 0`, а `verdict === "passed"`.
- **Оговорка о снимке:** пока в checkout работает писатель (C-02-BEFORE: `data.rs`,
  `main.rs`, `ws.rs`, новый `history_before_contract.rs` новее 15:22), любые числа
  «покрыто гейтом» устаревают в пределах минут. Матрицу следует пересобрать после
  остановки писателя и свежего `gate:release` на одном дереве — иначе QA снова будет
  описывать не то дерево, которое пойдёт в merge.
