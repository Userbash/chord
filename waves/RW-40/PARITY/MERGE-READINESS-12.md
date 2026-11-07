# MERGE-READINESS-12 — 26 принятых срезов, Wave 8: завершение INT-03 (Pass 4), unit 834/834

Дата: **2026-10-30**, снимок **18:24–18:26 +05**. Ветка `main`, HEAD
`4e240481e59ff4d49732ac8bd5ba08a006363748` (integration commit RW-40 от 2026-10-24 16:22).
Воркер: **G6 Parity**, отдел G6, карточка **G6-PARITY-SYNC-12**.
Режим — **read-only по продуктовому коду и тестам**.

> **Вход прохода.** `MERGE-READINESS-11.md`, `PARITY-MATRIX.md`, `GAP-BACKLOG.md`,
> 26 файлов приёмки (`ACCEPT.md`), ревью дизайн-спек (`STREAM-REVIEW-PASS3.md`, `TYPING-MARKDOWN-SPEC-REVIEW-PASS4.md`),
> артефакты `INT-03-SAVE-QUEUE-FIX` (`PM/g01b-ui/REPORT-PASS4.md`, `red-pass4.log`, `green-pass4.log`, `mutations-pass4.log`, `targeted-pass4.log`).
> **Новые принятые срезы и завершённые работы после снимка -11:**
> 1. `STUB-10-PINS-THREADS-PANELS-UI` (`PM/stub10-pins-threads/ACCEPT.md` — 19/19 pass, 5/5 мутаций, поповер-стили в `server-menu.css`, панели в `pins-threads.js`);
> 2. `G3-TYPING-MARKDOWN-SPEC` (`DESIGN/TYPING-MARKDOWN-SPEC-REVIEW-PASS4.md` — PASS, токены выровнены по `tokens.css`, P0=0, P1=0);
> 3. `INT-03-SAVE-QUEUE-FIX` (`PM/g01b-ui/REPORT-PASS4.md` — Pass 4, сериализация `saveQueue` в `app.js`, 98/98 targeted exit 0, 5/5 мутаций killed, `tests/unit/guild-rail.test.mjs` 16/16).

Запреты ТЗ соблюдены: продуктовый код и тесты не изменялись, `cargo` не запускался,
`git commit` не производился. Сверка md5 — `md5sum` по рабочему дереву; счётчики
матрицы — независимый `awk`-оракул из `PARITY-MATRIX.md §14`; unit-набор — собственный
прогон `npm run test:unit`.

---

## 0. Резюме

1. **Вердикт: merge — `block`. Parity — `in-progress / wave-8-integration` (открыты P1 по IPC и G05).**
   - На снимке 18:24 учтено **26 принятых срезов** (24 кодовых среза в `waves/RW-40/PM/*/ACCEPT.md` + 2 в корне `PM/` и `QUALITY/`), дизайн-спеки `G3-STREAM` и `G3-TYPING-MARKDOWN` имеют вердикт PASS.
   - Завершён `INT-03-SAVE-QUEUE-FIX` (Pass 4): закрыт блокер гонки `settings_save` через `saveQueue`, unit-тесты показывают **834/834 pass, 0 fail, 0 skipped** (167 suites), `npm run typecheck` — exit 0, `@ts-ignore` в UI — 0.
   - Слияние по-прежнему блокируют:
     - Несоответствие 5 имён IPC-команд между UI и Rust `main.rs` (F-1);
     - Отсутствие лога `npm run gate:worker` для `G05-REST-MUTATIONS-FIX` (F-3);
     - Устаревший gate-артефакт на коммите `0239543` и отсутствие интеграционного коммита (F-4).
2. **Сходимость матрицы:** 91 функция — **23 implemented (live unverified) / 41 partial / 27 absent / 0 unknown**; приоритеты 5/32/40/14. Счётчики стабильны, выполнена сверка независимым `awk`-оракулом.
3. **Рассинхрон с бэклогом:** `GAP-BACKLOG.md` остаётся на 22/39/30 (файл вне назначенных воркеру, требует обновления Team Lead).

---

## 1. Снимок репозитория

| Параметр | Значение | Метка |
|---|---|---|
| HEAD | `4e240481e59ff4d49732ac8bd5ba08a006363748` | [V] |
| Изменённые / неотслеживаемые пути (`git status --porcelain -uall`) | **345** | [V] |
| Последний gate-артефакт | `.gate/worker-2026-10-24T10-22-26-400Z.json`, коммит `0239543`, dirty | [V] |
| Принятые срезы с `ACCEPT.md` | **26** | [V] |
| Принятые дизайн-спецификации (PASS-ревью) | **2** (`STREAM.md`, `TYPING-MARKDOWN-SPEC.md`) | [V] |
| `npm run test:unit` | 834 tests / 834 pass / 0 fail / 0 skipped, 167 suites, 2.73 с | [V] |
| `npm run typecheck` | exit 0 (`tsc -p tsconfig.json && tsc -p tsconfig.agent.json`) | [V] |
| `@ts-ignore` в `rust/gui-spike/ui/**/*.js` | 0 | [V] |

---

## 2. Реестр принятых срезов (26 кодовых + 2 дизайн-спеки)

| # | Срез | Приёмка / ревью | Ключевые файлы | Проверки |
|---:|---|---|---|---|
| 1 | `G-01b-CONTRACT` | `PM/g01b-contract/ACCEPT.md` | `settings_store.rs`, `settings_contract.rs` | 2/2 совпали, PASS |
| 2 | `QA-M2-04` | `QUALITY/QA-M2-04-ACCEPT.md` | `ci.yml`, `ci-browser.test.mjs` | 2/2 совпали, PASS |
| 3 | `L-02` (интеграция) | `PM/l02/ACCEPT.md` | `dispatch.rs`, `gateway_supervisor.rs` | PASS |
| 4 | `L-02-CORE` | `PM/l02-core/ACCEPT.md` | `discord-core/src/ws.rs`, `gateway_supervisor.rs` | 2/2 совпали, PASS |
| 5 | `L-02-UI` | `PM/l02-ui/ACCEPT.md` | `dispatch-model.js`, `dispatch_ui.test.mjs` | 5/5 мутаций killed |
| 6 | `C-02-BE` | `PM/c02-be/ACCEPT.md` | `messages.rs`, `messages_contract.rs` | PASS |
| 7 | `C-02-UI-OLDER-FIX` | `PM/c02-ui-older/ACCEPT.md` | `chat.js`, `channel-paging.test.mjs` | PASS |
| 8 | `PLAT-11a` | `PM/plat-11a/ACCEPT.md` | `messages.rs`, `messages_contract.rs` | 15/15 contract, 5/5 мутаций |
| 9 | `M-07-BE3` | `PM/m07-be3/ACCEPT.md` | `data.rs`, `read_state_contract.rs` | 2/2 совпали, PASS |
| 10 | `SEC-L02-01-FIX` | `PM/sec-l02-01/ACCEPT.md` | `net.rs`, `net_contract.rs` | 2/2 совпали, 2/2 мутации |
| 11 | `B7b-SEARCH-FILTERS-BACKEND` | `PM/b7b-search/ACCEPT.md` | `search.rs`, `search_contract.rs` | 16/16 pass, 6/6 мутаций |
| 12 | `M-07-UI-BADGE` | `PM/m07-ui-badge/ACCEPT.md` | `chat.js`, `read-state-ui.test.mjs` | 29/29 pass |
| 13 | `STUB-27-CORE-ORACLE-FIX` | `PM/stub27-core/ACCEPT.md` | `stream.rs`, `stream_gateway_contract.rs` | 11/11 pass, 5/5 мутаций |
| 14 | `V-01-RTC-FIX` | `PM/v01-rtc/ACCEPT.md` | `session.rs`, `session_contract.rs` | 147/147 pass, 7/7 мутаций |
| 15 | `STUB-05-INVITES-BACKEND` | `PM/stub05-invites-be/ACCEPT.md` | `invites.rs`, `invites_contract.rs` | 20/20 pass, 5/5 мутаций |
| 16 | `STUB-14-UI-SEARCH-FIX` | `PM/stub14-search-ui/ACCEPT.md` | `search-overlay.js`, `search.js` | 108/108 pass, 5/5 мутаций |
| 17 | `P-03-UI-TYPING-INDICATOR` | `PM/p03-typing-ui/ACCEPT.md` | `typing-indicator.js`, `gui-spike-dom.test.mjs` | 58/58 pass, 5/5 мутаций |
| 18 | `M-10-M-11-PINS-THREADS-BACKEND` | `PM/m10-pins-be/ACCEPT.md` | `pins_threads.rs`, `pins_threads_contract.rs` | 6/6 pass, 5/5 мутаций |
| 19 | `STUB-27-CORE-DISPATCH-INTEGRATION`| `PM/stub27-dispatch/ACCEPT.md` | `dispatch.rs`, `dispatch_types.rs` | 583/583 pass, 5/5 мутаций |
| 20 | `STUB-05-INVITES-MODAL-UI` | `PM/stub05-invites-ui/ACCEPT.md` | `invite-modal.js`, `join-server.test.mjs` | 35/35 pass, 5/5 мутаций |
| 21 | `M-08-MARKDOWN-MENTIONS-UI` | `PM/m08-markdown/ACCEPT.md` | `markdown.js`, `gui-spike-model.test.mjs` | 48/48 pass, 5/5 мутаций |
| 22 | `V-07-VOICE-DEVICES-CONTRACT` | `PM/v07-devices/ACCEPT.md` | `devices.rs`, `devices_contract.rs` | 5/5 pass, 5/5 мутаций |
| 23 | `INT-02-UI-CHAT-MARKDOWN-TYPING` | `PM/int02-chat-md/ACCEPT.md` | `chat.js`, `composer.css` | 53/53 + 272/272 регрессий, 5/5 мутаций |
| 24 | `B2a-ATTACH-TIER-FIX-PASS2` | `PM/b2a-attach/ACCEPT.md` | `attachments.rs`, `attachments_contract.rs` | 17/17 pass, 4/4 мутаций |
| 25 | `STUB-16-MEMBER-PROFILE-POPOVER-FIX`| `PM/stub16-profile-ui/ACCEPT.md` | `members.js`, `members.css` | 13/13 pass, 5/5 мутаций |
| 26 | `STUB-10-PINS-THREADS-PANELS-UI` | `PM/stub10-pins-threads/ACCEPT.md` | `pins-threads.js`, `server-menu.css`, `channel-paging.test.mjs` | 19/19 pass, 5/5 мутаций |

Дизайн-спецификации:
- `G3-STREAM-FIX-PASS2` (`DESIGN/STREAM-REVIEW-PASS3.md`): PASS F-02..F-04.
- `G3-TYPING-MARKDOWN-SPEC` (`DESIGN/TYPING-MARKDOWN-SPEC-REVIEW-PASS4.md`): PASS, токены согласованы с `tokens.css`.

Статус связанных работ:
- `INT-03-SAVE-QUEUE-FIX` (`PM/g01b-ui/REPORT-PASS4.md`): Pass 4 завершена, 98/98 targeted exit 0, 5/5 мутаций killed, P1 закрыт в коде/тестах; ожидает формального ACCEPT/ревью.
- `B2b-ATTACH-UI-20MIB-SYNC` (`PM/b2b-attach-ui/REVIEW-PASS4.md`): 29/29 pass, 5/5 caught (PASS-ревью в объёме карточки).
- `G05-REST-MUTATIONS-FIX` (`PM/g05-rest/REVIEW-PASS4.md`): RETURN (нет gate:worker).

---

## 3. Сверка md5 по ключевым файлам свежих срезов

| Файл | MD5 в дереве | Источник | Статус |
|---|---|---|---|
| `rust/gui-spike/ui/features/pins-threads.js` | `f3e2708a3e8082dc60a999387850659a` | `stub10-pins-threads/ACCEPT.md` | Совпал [V] |
| `rust/gui-spike/ui/components/server-menu.css` | `08c5c3cb71d59df18e90a72787f0fe8a` | `stub10-pins-threads/ACCEPT.md` | Совпал [V] |
| `tests/unit/channel-paging.test.mjs` | `e6974ee4a62ea5eef57ac128239e5dce` | `stub10-pins-threads/ACCEPT.md` | Совпал [V] |
| `waves/RW-40/DESIGN/TYPING-MARKDOWN-SPEC.md` | `38651e2b4d740cc434b805d941703d5f` | `TYPING-MARKDOWN-SPEC-REVIEW-PASS4.md` | Совпал [V] |
| `rust/gui-spike/ui/app.js` | `8f5d84c14dc6e9aea749a202ecb3a96b` | `g01b-ui/REPORT-PASS4.md` | Совпал [V] |
| `tests/unit/guild-rail.test.mjs` | `76a0acca4dcc95ac21ec1d7f90370a32` | `g01b-ui/REPORT-PASS4.md` | Совпал [V] |

---

## 4. Независимые проверки этого прохода

| Проверка | Команда | Результат |
|---|---|---|
| Полный unit-набор | `npm run test:unit` | 834 tests, 834 pass, 0 fail, 0 cancelled, 0 skipped, 0 todo, 167 suites (2.73 с) [V] |
| Typecheck | `npm run typecheck` | exit 0 (`tsc -p tsconfig.json && tsc -p tsconfig.agent.json`) [V] |
| `@ts-ignore` в UI | `grep -rn "@ts-ignore" rust/gui-spike/ui/` | 0 [V] |
| Счётчики матрицы | `awk`-оракул §14 | 23 implemented / 41 partial / 27 absent, total 91, P0..P3 = 5/32/40/14 [V] |

---

## 5. Паритет и текущий статус

| Строка | Текущий статус | Изменения и детали |
|---|---|---|
| `G-01` | implemented (live unverified) | Введена очередь `saveQueue` в `app.js` (`INT-03 Pass 4`), исключена перезапись выбора серверов при запаздывании `settings_save`. 16/16 тестов в `guild-rail.test.mjs` green. |
| `M-10` | partial | Backend (`pins_threads.rs`) + UI (`pins-threads.js`) приняты (`STUB-10` ACCEPT). Блокирует F-1 (UI зовёт `pins_list` вместо `list_channel_pins`). |
| `M-11` | partial | Backend + UI панель активных/архивных веток приняты (`STUB-10` ACCEPT). Блокирует F-1 (UI зовёт `threads_list` без `archived` вместо `list_channel_threads`). |
| `M-08` | implemented (live unverified) | Спецификация `TYPING-MARKDOWN-SPEC` успешно прошла PASS 4 (токены CSS выровнены). AST-парсер и DOM-рендерер встроены в `chat.js`. |
| `C-04` | partial | Панели пинов и веток подключены в шапку через `app.js`. Требуется исправление IPC-маршрутизации. |

---

## 6. Открытые блокеры слияния

### F-1 (P1). Несоответствие IPC-команд (UI ↔ Rust main.rs)
UI вызывает:
1. `attachments_upload_slots` (`composer.js:725`) → в `main.rs` зарегистрирована `request_upload_slots`.
2. `attachments_upload_put` (`composer.js:760`) → отсутствует в `main.rs`.
3. `create_invite` (`invite-modal.js:370`) → зарегистрирована `create_channel_invite`.
4. `pins_list` (`app.js:1359`) → зарегистрирована `list_channel_pins`.
5. `threads_list` (`app.js:1365`) → зарегистрирована `list_channel_threads`.

### F-3 (P1). G05: отсутствие лога gate:worker
`PM/g05-rest/REVIEW-PASS4.md` ожидает полного артефакта `npm run gate:worker` с `exit code 0`.

### F-4 (P1). Устаревший gate-артефакт
Артефакт `.gate/worker-*.json` относится к коммиту `0239543`. Необходим интеграционный коммит и единый прогон гейта.

---

## 7. Чек-лист до разблокировки слияния

1. **Выровнять IPC-имена (F-1):** переключить вызовы в UI на команды `main.rs` либо зарегистрировать соответствующие алиасы в Rust.
2. **Формализовать приёмку INT-03 (Pass 4):** зафиксировать `ACCEPT.md` после успешного Pass 4 (98/98 targeted, 5/5 мутаций).
3. **Предоставить лог `npm run gate:worker` для G05.**
4. **Выполнить интеграционный коммит и запустить полный гейт.**
5. **Обновить `waves/STATE.md`, `waves/db.jsonl` и `GAP-BACKLOG.md` (Team Lead).**
