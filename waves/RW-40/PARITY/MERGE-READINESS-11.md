# MERGE-READINESS-11 — 24 принятых среза, Wave 8: INT-02 принят, INT-03 и G05 на возврате, unit 833/833

Дата: **2026-10-30**, снимок **18:12–18:14 +05**. Ветка `main`, HEAD
`4e240481e59ff4d49732ac8bd5ba08a006363748` (integration commit RW-40 от 2026-10-24 16:22).
Воркер: **G6 Parity**, отдел G6, карточка **G6-PARITY-SYNC-11**.
Режим — **read-only по продуктовому коду и тестам**.

> **Вход прохода.** `MERGE-READINESS-10.md`, `PARITY-MATRIX.md`, `GAP-BACKLOG.md`,
> 24 файла приёмки/ревью (`ACCEPT.md`, `REVIEW*.md`), red/green/mutation-логи
> `waves/RW-40/PM/*` и `waves/RW-40/PM/g05-rest/unit-green-833.log`.
> **Новые срезы после снимка -10:**
> 1. `INT-02-UI-CHAT-MARKDOWN-TYPING` (`PM/int02-chat-md/ACCEPT.md`);
> 2. `B2a-ATTACH-TIER-FIX-PASS2` (`PM/b2a-attach/ACCEPT.md`);
> 3. `STUB-16-MEMBER-PROFILE-POPOVER-FIX` (`PM/stub16-profile-ui/ACCEPT.md`);
> 4. `B2b-ATTACH-UI-20MIB-SYNC` (`PM/b2b-attach-ui/REVIEW-PASS4.md`, PASS, P0=0, P1=0 в объёме карточки).

Запреты ТЗ соблюдены: продуктовый код и тесты не изменялись, `cargo` не запускался,
`git commit` не производился. Сверка md5 — `md5sum` по рабочему дереву; счётчики
матрицы — независимый `awk`-оракул из `PARITY-MATRIX.md §14`; unit-набор — собственный
прогон `npm run test:unit`.

---

## 0. Резюме

1. **Вердикт: merge — `block`. Parity — `in-progress / wave-8-integration` (открыты P1).**
   - Принято 24 кодовых среза, `npm run test:unit` — **833/833 pass, 0 fail, 0 skipped**
     (167 suites) на снимке 18:12, `npm run typecheck` — exit 0, `@ts-ignore` в UI — 0.
     В 18:17 дерево ушло вперёд: 834/834, `app.js` и `guild-rail.test.mjs` заменены новой
     pass `INT-03` (см. §1.1) — её отчёт в этот снимок не входит.
   - Слияние блокируют не тесты, а процесс и интеграция: незакрытый P1 `INT-03` (гонка
     `settings_save`), отсутствие `gate:worker` для `G05-REST-MUTATIONS-FIX`, расхождение
     имён IPC (п. 2), устаревший gate-артефакт на коммите `0239543` и отсутствие
     интеграционного коммита.
2. **Новая находка G6 (P1, не покрыта ревью Wave 8):** фронтенд вызывает пять IPC-команд,
   которых нет в `rust/gui-spike/src/main.rs`:
   `attachments_upload_slots`, `attachments_upload_put`, `create_invite`, `pins_list`,
   `threads_list`. Реальные имена — `request_upload_slots`, `create_channel_invite`,
   `list_channel_pins`, `list_channel_threads`; PUT-команды нет вовсе.
   Unit-тесты этого не ловят: они подменяют `invoke`, а мета-тест `tests/meta/ipc-path.test.mjs`
   сверяет только Rust-объявления с `generate_handler!`, не с JS.
3. **Accepted set = 24 среза** (23 официальных `ACCEPT.md` + `B2b` по PASS-ревью pass 4).
   Дизайн-спека `STREAM-REVIEW-PASS3` учитывается отдельно как архитектурная позиция.
   `G05-REST-MUTATIONS-FIX` **не принят**: `REVIEW-PASS4` — RETURN, P1 (нет evidence gate).
4. **Сходимость матрицы:** 91 функция — **23 implemented (live unverified) / 41 partial /
   27 absent / 0 unknown**; приоритеты 5/32/40/14. Единственный перевод статуса за проход —
   `M-08` (`partial → implemented (live unverified)`).
5. **Рассинхрон с бэклогом:** `GAP-BACKLOG.md` остаётся на 22/39/30 (файл вне назначенных
   воркеру и требует правки Team Lead).

---

## 1. Снимок репозитория

| Параметр | Значение | Метка |
|---|---|---|
| HEAD | `4e240481e59ff4d49732ac8bd5ba08a006363748` | [V] |
| Изменённые / неотслеживаемые пути (`git status --porcelain -uall`) | **345** | [V] |
| Последний gate-артефакт | `.gate/worker-2026-10-24T10-22-26-400Z.json`, коммит `0239543`, dirty | [V] |
| Принятые кодовые срезы | **24** | [V] |
| `npm run test:unit` | 833 tests / 833 pass / 0 fail / 0 skipped, 167 suites, 2.75 с | [V] |
| `npm run typecheck` | exit 0 (`tsc -p tsconfig.json && tsc -p tsconfig.agent.json`) | [V] |
| `@ts-ignore` в `rust/gui-spike/ui/**/*.js` | 0 | [V] |

### 1.1 Дерево двигалось во время прохода

Снимок отчёта — 18:12–18:14. В 18:17 другой лейн записал новую версию
`tests/unit/guild-rail.test.mjs` (`76a0acca…` вместо `a1907d0e…`) и `app.js`
(`8f5d84c1…` вместо `4d46b018…`): это новая pass по `INT-03`, её отчёта и ревью
на снимке ещё нет.

| Момент | `app.js` | `guild-rail.test.mjs` | `npm run test:unit` |
|---|---|---|---|
| 18:12–18:14 (снимок отчёта) | `4d46b018…` | `a1907d0e…` | 833/833 |
| 18:17 (addendum) | `8f5d84c1…` | `76a0acca…` | **834/834**, 0 fail |

Вывод по слиянию не меняется: пока новая pass не принята, действует `RETURN` из
`REVIEW-PASS3.md`. Но evidence 833 и 834 относятся к разным деревьям, и смешивать их нельзя.

---

## 2. Реестр принятых срезов (24 кодовых + 1 дизайн-спека)

| # | Срез | Приёмка / ревью | Ключевые файлы | Проверки |
|---:|---|---|---|---|
| 1 | `G-01b-CONTRACT` | `PM/g01b-contract/ACCEPT.md` | `settings_store.rs`, `settings_contract.rs` | 2/2 совпали, PASS |
| 2 | `L-02-CORE` | `PM/l02-core/ACCEPT.md` | `discord-core/src/ws.rs`, `gateway_supervisor.rs` | 2/2 совпали, PASS |
| 3 | `L-02-UI` | `PM/l02-ui/ACCEPT.md` | `dispatch-model.js`, `dispatch_ui.test.mjs` | совпали (app.js superseding) |
| 4 | `C-02-UI-OLDER-FIX` | `PM/c02-ui-older/ACCEPT.md` | `chat.js`, `channel-paging.test.mjs` | chat.js superseding |
| 5 | `PLAT-11a` | `PM/plat-11a/ACCEPT.md` | `messages.rs`, `messages_contract.rs` | 2/2 совпали, PASS |
| 6 | `M-07-BE3` | `PM/m07-be3/ACCEPT.md` | `data.rs`, `read_state_contract.rs` | 2/2 совпали, PASS |
| 7 | `QA-M2-04` | `QUALITY/QA-M2-04-ACCEPT.md` | `ci.yml`, `ci-browser.test.mjs` | 2/2 совпали, PASS |
| 8 | `SEC-L02-01-FIX` | `PM/sec-l02-01/ACCEPT.md` | `net.rs`, `net_contract.rs` | 2/2 совпали, 2/2 мутации |
| 9 | `B7b-SEARCH-FILTERS-BACKEND` | `PM/b7b-search/ACCEPT.md` | `search.rs`, `search_contract.rs` | 2/2 совпали, 6/6 мутаций |
| 10 | `M-07-UI-BADGE` | `PM/m07-ui-badge/ACCEPT.md` | `chat.js`, `read-state-ui.test.mjs` | 2/2 совпали, supersedes c02 |
| 11 | `G3-STREAM-FIX-PASS2` | `DESIGN/STREAM-REVIEW-PASS3.md` | `DESIGN/STREAM.md` | PASS F-02..F-04 (P0=0, P1=0) |
| 12 | `STUB-27-CORE-ORACLE-FIX` | `PM/stub27-core/ACCEPT.md` | `stream.rs`, `stream_gateway_contract.rs` | 11/11 pass, 5/5 мутаций |
| 13 | `V-01-RTC-FIX` | `PM/v01-rtc/ACCEPT.md` | `session.rs`, `session_contract.rs` | 147/147 pass, 7/7 мутаций |
| 14 | `STUB-05-INVITES-BACKEND` | `PM/stub05-invites-be/ACCEPT.md` | `invites.rs`, `invites_contract.rs` | 20/20 pass, 5/5 мутаций |
| 15 | `STUB-14-UI-SEARCH-FIX` | `PM/stub14-search-ui/ACCEPT.md` | `search-overlay.js`, `search.js` | 108/108 pass, 5/5 мутаций |
| 16 | `P-03-UI-TYPING-INDICATOR` | `PM/p03-typing-ui/ACCEPT.md` | `typing-indicator.js`, `gui-spike-dom.test.mjs` | 58/58 pass, 5/5 мутаций |
| 17 | `M-10-M-11-PINS-THREADS-BACKEND` | `PM/m10-pins-be/ACCEPT.md` | `pins_threads.rs`, `pins_threads_contract.rs` | 6/6 pass, 5/5 мутаций |
| 18 | `STUB-27-CORE-DISPATCH-INTEGRATION` | `PM/stub27-dispatch/ACCEPT.md` | `dispatch.rs`, `dispatch_types.rs` | 583/583 pass, 5/5 мутаций |
| 19 | `STUB-05-INVITES-MODAL-UI` | `PM/stub05-invites-ui/ACCEPT.md` | `invite-modal.js`, `join-server.test.mjs` | 35/35 pass, 5/5 мутаций |
| 20 | `M-08-MARKDOWN-MENTIONS-UI` | `PM/m08-markdown/ACCEPT.md` | `markdown.js`, `gui-spike-model.test.mjs` | 48/48 pass, 5/5 мутаций |
| 21 | `V-07-VOICE-DEVICES-CONTRACT` | `PM/v07-devices/ACCEPT.md` | `devices.rs`, `devices_contract.rs` | 5/5 pass, 5/5 мутаций |
| 22 | `INT-02-UI-CHAT-MARKDOWN-TYPING` | `PM/int02-chat-md/ACCEPT.md` | `chat.js`, `composer.css` | 53/53 + 272/272 регрессий, 5/5 мутаций, P0=P1=P2=0 |
| 23 | `B2a-ATTACH-TIER-FIX-PASS2` | `PM/b2a-attach/ACCEPT.md` | `attachments.rs`, `attachments_contract.rs` | 17/17 pass, 4/4 мутаций, P0=P1=P2=0 |
| 24 | `STUB-16-MEMBER-PROFILE-POPOVER-FIX` | `PM/stub16-profile-ui/ACCEPT.md` | `members.js`, `members.css` | 13/13 pass, 5/5 мутаций, P0=P1=P2=0 |
| 25 | `B2b-ATTACH-UI-20MIB-SYNC` | `PM/b2b-attach-ui/REVIEW-PASS4.md` | `composer.js`, `composer.test.mjs` | 29/29 pass, 5/5 caught, P0=P1=P2=0 в объёме карточки |

> Позиция 11 — дизайн-спека, не кодовый срез. Поэтому «24 среза» = 23 `ACCEPT.md` + `B2b` по PASS-ревью.

**Не принято, хотя код сдан:**

| Карточка | Артефакт | Вердикт | Причина |
|---|---|---|---|
| `INT-03-UI-APP-PANELS-MODALS` | `PM/g01b-ui/REPORT-PASS3.md`, `REVIEW-PASS3.md` | **RETURN, P0=0, P1=1** | гонка `settings_save` A/B не закрыта тестом и кодом |
| `G05-REST-MUTATIONS-FIX` | `PM/g05-rest/REPORT-PASS3.md`, `REVIEW-PASS4.md` | **RETURN, P0=0, P1=1** | нет лога `npm run gate:worker` с exit code |
| `INT-01-RUST-WIRING` | `PM/int01-rust-wiring/REPORT.md` | отчёт сдан, ревью нет | 5/5 тестов, clippy exit 0, 5/5 мутаций; ждёт Codex 15A |

---

## 3. Сверка md5 по новым срезам

| Файл | MD5 в дереве | Источник | Статус |
|---|---|---|---|
| `rust/gui-spike/ui/features/chat.js` | `d2e3bb0958fbafb44dc14fe72cee0426` | `int02-chat-md/ACCEPT.md` | Совпал [V] |
| `rust/gui-spike/ui/components/composer.css` | `d62aa5c55b033f3fe7da1708e151d717` | `int02-chat-md/ACCEPT.md` | Совпал [V] |
| `tests/unit/gui-spike-model.test.mjs` | `0eee4284af263c3d412b0afe4910bd58` | `int02-chat-md/ACCEPT.md` | Совпал [V] |
| `rust/gui-spike/src/attachments.rs` | `3377d6dcd0850da6b92dbaac3a6c7dac` | `b2a-attach/ACCEPT.md` | Совпал [V] |
| `rust/gui-spike/tests/attachments_contract.rs` | `45d107db41d4c1f443de8596b1196e49` | `b2a-attach/ACCEPT.md` | Совпал [V] |
| `rust/gui-spike/ui/features/members.js` | `316bc1acb8ab609bd7814f4f2ffff1bd` | `stub16-profile-ui/ACCEPT.md` | Совпал [V] |
| `rust/gui-spike/ui/components/members.css` | `9ff3955f5c7b9941b0564b639e40e991` | `stub16-profile-ui/ACCEPT.md` | Совпал [V] |
| `tests/unit/gui-spike-regressions.test.mjs` | `5dbe993ecf515a9e7622ac2258903c03` | `stub16-profile-ui/ACCEPT.md` | Совпал [V] |
| `rust/gui-spike/ui/features/composer.js` | `223e7641d973ea68c93fde90969f3053` | `b2b-attach-ui/REVIEW-PASS4.md` | Совпал [V] |
| `tests/unit/composer.test.mjs` | `fd4211394ef4bb7659477fd5904ab584` | `b2b-attach-ui/REVIEW-PASS4.md` | Совпал [V] |
| `rust/gui-spike/ui/app.js` | `4d46b018bbca23fe08be08bae1ac1a32` | `g01b-ui/REPORT-PASS3.md` | Совпал на 18:14; к 18:17 заменён (см. §1.1) |
| `rust/gui-spike/ui/components/server-menu.css` | `08c5c3cb71d59df18e90a72787f0fe8a` | `g01b-ui/REPORT-PASS3.md` | Не пересчитывался |
| `tests/unit/guild-rail.test.mjs` | `a1907d0e8820f346743b56b63fdfaaa6` | `g01b-ui/REPORT-PASS3.md` | Не пересчитывался |
| `rust/gui-spike/src/lib.rs` | `b2af030ca24749a6e4d278b5f1106a1f` | `int01-rust-wiring/REPORT.md` | Совпал [V] |
| `rust/gui-spike/src/main.rs` | `c192e36f7c3bc68637169f45cb53b203` | `int01-rust-wiring/REPORT.md` | Совпал [V] |
| `rust/gui-spike/tests/ipc_wiring_contract.rs` | `564bb6d670b3346a126c49d26ba1eb94` | `int01-rust-wiring/REPORT.md` | Совпал [V] |

`server-menu.css` и `guild-rail.test.mjs` в отчёте INT-03 указаны, но отдельного файла приёмки
у INT-03 нет (RETURN), поэтому md5 сверены с отчётом, а не с `ACCEPT.md`.

---

## 4. Независимые проверки этого прохода

| Проверка | Команда | Результат |
|---|---|---|
| Полный unit-набор | `npm run test:unit` | 833 tests, 833 pass, 0 fail, 0 cancelled, 0 skipped, 0 todo, 167 suites на 18:12 [V]; на 18:17 — 834/834 (§1.1) |
| Typecheck | `npm run typecheck` | exit 0 [V] |
| `@ts-ignore` в UI | `grep -rn "@ts-ignore" rust/gui-spike/ui/` | 0 [V] |
| Модель чата (INT-02) | `node --test tests/unit/gui-spike-model.test.mjs` | 53/53 [V] |
| Рейл/гильдии (INT-03) | `node --test tests/unit/guild-rail.test.mjs` | 15/15 [V] |
| Композер (B2b) | `node --test tests/unit/composer.test.mjs` | 29/29 [V] |
| Поповер профиля (STUB-16) | `node --test tests/unit/gui-spike-regressions.test.mjs` | 13/13 [V] |
| Панели пинов/веток (STUB-10) | `node --test tests/unit/channel-paging.test.mjs` | 19/19 [V] |
| Typing | `node --test tests/unit/typing-ui.test.mjs` + `gui-spike-dom.test.mjs` | 20/20 + 38/38 [V] |
| Счётчики матрицы | `awk`-оракул §14 | 23/41/27, 91, P0..P3 = 5/32/40/14 [V] |

**Не запускалось этим проходом:** `cargo test`, `cargo clippy`, `npm run gate:worker`,
e2e и live-сценарии. Для Rust-срезов цитируются логи воркеров (`green.log`, `clippy.log`,
`mutations.log`), а не собственный прогон.

---

## 5. Паритет: что изменилось

| Строка | Было | Стало | Основание |
|---|---|---|---|
| `M-08` Markdown/mentions | partial | **implemented (live unverified)** | `markdown.js` встроен в `chat.js`, INT-02 ACCEPT, 53/53, 5/5 |
| `AT-01` Загрузка файлов | partial | partial (уточнено) | пол 20 МиБ, Tier2/3 = 50/100 МиБ, UI-лимит выровнен |
| `MB-03` Профиль-поповер | partial | partial (уточнено) | `#user-profile-popover`, 13/13, 5/5 |
| `P-03` Индикатор набора | partial | partial (уточнено) | исходящий `send_typing` подключён; входящий `TYPING_START` — нет |
| `G-06`, `M-10`, `M-11`, `C-04` | partial | partial (уточнено) | UI смонтирован, IPC-имена не совпадают |

Счётчики (`PARITY-MATRIX.md §14`):

| App-статус | Кол-во | Δ к -10 |
|---|---:|---:|
| implemented / verified | 23 | +1 |
| partial | 41 | −1 |
| absent | 27 | 0 |
| unknown | 0 | 0 |
| **Итого** | **91** | 0 |

`implemented (live unverified)` по-прежнему не значит «паритет подтверждён»: E3 нет ни по одной строке.

---

## 6. Находки, блокирующие слияние

### F-1 (P1, новая). Пять IPC-команд, которых нет в Rust-слое

Сверка всех вызовов `invoke(...)`/`invokeFn(...)` в `rust/gui-spike/ui/**/*.js` с
`#[tauri::command]` в `rust/gui-spike/src/main.rs`:

| Вызов из UI | Файл:строка | Команда в `main.rs` | Итог |
|---|---|---|---|
| `attachments_upload_slots` | `composer.js:725` | `request_upload_slots` (:511) | имя не совпадает |
| `attachments_upload_put` | `composer.js:760` | **отсутствует** | PUT-пути нет |
| `create_invite` | `invite-modal.js:370` | `create_channel_invite` (:487) | имя не совпадает |
| `pins_list` | `app.js:1359` | `list_channel_pins` (:534) | имя не совпадает |
| `threads_list` | `app.js:1365` | `list_channel_threads` (:587) | имя не совпадает, нет `archived` |

Следствия: модалка инвайтов, панели пинов/веток и загрузка вложений в собранном приложении
не доходят до бэкенда. Тесты зелёные, потому что подменяют `invoke`; мета-тест
`tests/meta/ipc-path.test.mjs` проверяет только пары «объявление ↔ `generate_handler!`»
и не читает JS. Это же объясняет открытый P1 `B2b REVIEW-PASS4` про реальную доставку файла.

Рекомендация: карточка на выравнивание имён (UI → существующие команды), плюс новый
мета-тест «каждый `invoke("...")` из `ui/` существует в `main.rs`», иначе класс ошибки
вернётся. Правка вне назначенных файлов G6 — не выполнялась.

### F-2 (P1, из ревью). INT-03: гонка `settings_save`

`PM/g01b-ui/REVIEW-PASS3.md`: если сохранение A стартует, пользователь выбирает B, B
сохраняется раньше, а A завершается последним — в store остаётся A. Тест задерживает
`settings_load`, а не `settings_save`, поэтому обратный порядок не моделируется.
Карточка не принята, хотя панели и модалка смонтированы.

### F-3 (P1, из ревью). G05: нет evidence `gate:worker`

`PM/g05-rest/REVIEW-PASS4.md`: `unit-green-833.log` подтверждает 833/833, но не заменяет
gate — нет typecheck/meta/smoke-фаз и явного exit code. Карточка не принята.

### F-4 (P1, процесс). Gate-артефакт устарел

Последний `.gate/worker-*.json` — `2026-10-24T10-22-26-400Z`, коммит `0239543`.
Текущий HEAD `4e24048`, дерево dirty (345 путей). Интеграционного коммита нет.

---

## 7. Чек-лист до разблокировки слияния

1. **Выровнять IPC-имена (F-1).** UI → `request_upload_slots` / `create_channel_invite` /
   `list_channel_pins` / `list_channel_threads`; решить судьбу PUT: добавить Tauri-команду
   или явно оставить B2b вне scope. Добавить мета-тест «UI `invoke` ⊆ Rust-команды».
2. **Закрыть P1 INT-03** red-first сценарием с задержанным `settings_save` A и повторным ревью.
3. **Приложить `npm run gate:worker`** с exit code 0 для `G05-REST-MUTATIONS-FIX`.
4. **Прогнать единый gate на актуальном дереве** (`gate:worker`; для выпуска — `gate:release`),
   сохранить `.gate/worker-*.json` с HEAD `4e24048`.
5. **Дождаться ревью Codex 15A/15B** по `INT-01`, `INT-02`, `INT-03`, `G05`.
6. **Синхронизировать `GAP-BACKLOG.md`** под 23/41/27 (файл Team Lead).
7. **Зафиксировать волну:** append в `waves/db.jsonl`, обновить `waves/STATE.md`,
   интеграционный коммит — координатор L1.

---

## 8. Ограничения прохода

1. `cargo test`/`clippy` не запускались: Rust-доказательства взяты из логов воркеров и
   отчёта INT-01, повторно не воспроизводились.
2. Live/E3 отсутствует: `implemented (live unverified)` — offline-путь, не паритет.
3. `GAP-BACKLOG.md` не сверен построчно; зафиксирован только рассинхрон счётчиков.
4. F-1 получена статической сверкой имён; поведение в собранном Tauri-приложении не
   воспроизводилось (нет окна и live-сессии).
5. Число 24 — реестровое: 23 `ACCEPT.md` + `B2b` по PASS-ревью; дизайн-спека учтена отдельно.
