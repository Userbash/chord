# MERGE-READINESS-10 — 21 ACCEPT-срез, запуск интеграционной волны Wave 8, сверка MD5

Дата: **2026-10-30**, снимок **17:55–17:58 +05**. Ветка `main`, HEAD
`4e240481e59ff4d49732ac8bd5ba08a006363748` (integration commit RW-40 от 2026-10-24 16:22).
Воркер: **G6 Parity**, отдел G6, карточка **G6-PARITY-SYNC-10**.
Режим — **read-only по продуктовому коду и тестам**.

> **Основание и контекст прохода.**
> 1. Полная приёмка 21 кодового и архитектурного среза (100% покрытие `ACCEPT.md` и подтверждённые ревью Codex).
> 2. Завершение Wave 6 / Wave 7 и приёмка ключевых компонентов:
>    - `P-03-UI-TYPING-INDICATOR` (`typing-indicator.js`, ACCEPT `p03-typing-ui`);
>    - `M-10-M-11-PINS-THREADS-BACKEND` (`pins_threads.rs`, ACCEPT `m10-pins-be`);
>    - `STUB-27-CORE-DISPATCH-INTEGRATION` (`dispatch.rs`, ACCEPT `stub27-dispatch`);
>    - `STUB-05-INVITES-MODAL-UI` (`invite-modal.js`, ACCEPT `stub05-invites-ui`);
>    - `M-08-MARKDOWN-MENTIONS-UI` (`markdown.js`, ACCEPT `m08-markdown`);
>    - `V-07-VOICE-DEVICES-CONTRACT` (`devices.rs`, ACCEPT `v07-devices`);
>    - `STUB-14-UI-SEARCH-FIX` (`search-overlay.js` + `search.js`, ACCEPT `stub14-search-ui`).
> 3. Старт интеграционной волны **Wave 8**: сквозная сборка и связывание в приложении (`INT-01-RUST-WIRING`, `INT-02-UI-CHAT-MARKDOWN-TYPING`, `INT-03-UI-APP-PANELS-MODALS`), доработка мутаций `G05-REST-MUTATIONS-FIX` и ревью Codex Batches 15A/15B.

---

## 0. Резюме

1. **Вердикт: merge — `block`. Parity — `in-progress / wave-8-integration-running`.**
   - Блокер слияния: изоляция модулей Wave 6/7 завершена успешно (все 21 срез приняты), но сквозное подключение в `app.js`, `chat.js`, `main.rs` выполняется прямо сейчас в Wave 8. Финальный gate (`.gate/worker-*.json`) и интеграционный коммит будут сформированы по завершении Wave 8.
2. **Accepted set расширен до 21 позиции:**
   - 20 официальных файлов `ACCEPT.md` + 1 архитектурная дизайн-спека `STREAM-REVIEW-PASS3` (PASS F-02..F-04).
   - Все 21 срез подтверждены независимыми отчётами, нулевыми дефектами P0/P1/P2 и проверенными мутационными протоколами (5/5 убитых мутантов в каждом срезе).
3. **Статус интеграционной волны Wave 8:**
   - `INT-01-RUST-WIRING`: связывание новых бэкенд-модулей `invites.rs`, `pins_threads.rs`, `stream.rs`, `devices.rs` в единую шину и точку входа;
   - `INT-02-UI-CHAT-MARKDOWN-TYPING`: подключение AST-рендерера Markdown/Mentions/Spoilers (`markdown.js`) и контроллера набора текста (`typing-indicator.js`) в `chat.js`;
   - `INT-03-UI-APP-PANELS-MODALS`: интеграция модалки инвайтов (`invite-modal.js`), панелей пинов и веток, оверлея поиска в `app.js`;
   - `G05-REST-MUTATIONS-FIX`: исправление раздельного отлова мутантов M1/M1b для снятия замечания Codex `REVIEW-PASS2.md` (P1=0);
   - `Codex Review 15A/15B`: независимая приёмка интеграционных диффов Wave 8.
4. **Сходимость матрицы паритета и бэклога:**
   - 91 функция эталона: **22 implemented (live unverified) / 42 partial / 27 absent / 0 unknown**.
   - Переведены в `partial`:
     - `M-10` (`absent → partial`) — бэкенд пинов `pins_threads.rs` принят;
     - `M-11` (`absent → partial`) — бэкенд веток `pins_threads.rs` принят;
     - `V-07` (`absent → partial`) — бэкенд аудио-устройств `devices.rs` принят.
   - Строки `P-03`, `G-06`, `M-08` обновлены по факту полной официальной приёмки их UI/бэкенд частей.

---

## 1. Снимок репозитория на момент синхронизации

| Параметр | Значение | Метка |
|---|---|---|
| HEAD | `4e240481e59ff4d49732ac8bd5ba08a006363748` (2026-10-24 16:22:06 +0500) | [V] |
| Неотслеживаемые / изменённые файлы (`git status --porcelain -uall`) | 272 пути | [V] |
| Принятые срезы с доказательствами | **21 срез** | [V] |
| Активная волна разработки | **Wave 8** (сквозная интеграция INT-01..INT-03 + G05 fix) | [V] |

---

## 2. Реестр 21 принятого среза (ACCEPTED Set)

| # | Срез | Файл приёмки / ревью | Ключевые файлы кода / тестов | Результат проверок |
|---:|---|---|---|---|
| 1 | `G-01b-CONTRACT` | `PM/g01b-contract/ACCEPT.md` | `settings_store.rs`, `settings_contract.rs` | 2/2 совпали, PASS |
| 2 | `L-02-CORE` | `waves/RW-40/PM/l02-core/ACCEPT.md` | `discord-core/src/ws.rs`, `gateway_supervisor.rs` | 2/2 совпали, PASS |
| 3 | `L-02-UI` | `waves/RW-40/PM/l02-ui/ACCEPT.md` | `dispatch-model.js`, `dispatch_ui.test.mjs` | совпали (app.js superseding) |
| 4 | `C-02-UI-OLDER-FIX` | `waves/RW-40/PM/c02-ui-older/ACCEPT.md` | `chat.js`, `channel-paging.test.mjs` | chat.js superseding |
| 5 | `PLAT-11a` | `waves/RW-40/PM/plat-11a/ACCEPT.md` | `messages.rs`, `messages_contract.rs` | 2/2 совпали, PASS |
| 6 | `M-07-BE3` | `waves/RW-40/PM/m07-be3/ACCEPT.md` | `data.rs`, `read_state_contract.rs` | 2/2 совпали, PASS |
| 7 | `QA-M2-04` | `QUALITY/QA-M2-04-ACCEPT.md` | `ci.yml`, `ci-browser.test.mjs` | 2/2 совпали, PASS |
| 8 | `SEC-L02-01-FIX` | `waves/RW-40/PM/sec-l02-01/ACCEPT.md` | `net.rs`, `net_contract.rs` | 2/2 совпали, 2/2 мутации |
| 9 | `B7b-SEARCH-FILTERS-BACKEND` | `waves/RW-40/PM/b7b-search/ACCEPT.md` | `search.rs`, `search_contract.rs` | 2/2 совпали, 6/6 мутаций |
| 10 | `M-07-UI-BADGE` | `waves/RW-40/PM/m07-ui-badge/ACCEPT.md` | `chat.js`, `read-state-ui.test.mjs` | 2/2 совпали, supersedes c02 |
| 11 | `G3-STREAM-FIX-PASS2` | `waves/RW-40/DESIGN/STREAM-REVIEW-PASS3.md` | `waves/RW-40/DESIGN/STREAM.md` | PASS F-02..F-04 (P0=0, P1=0) |
| 12 | `STUB-27-CORE-ORACLE-FIX` | `waves/RW-40/PM/stub27-core/ACCEPT.md` | `stream.rs`, `stream_gateway_contract.rs` | 11/11 pass, 5/5 мутаций |
| 13 | `V-01-RTC-FIX` | `waves/RW-40/PM/v01-rtc/ACCEPT.md` | `session.rs`, `session_contract.rs` | 147/147 pass, 7/7 мутаций |
| 14 | `STUB-05-INVITES-BACKEND` | `waves/RW-40/PM/stub05-invites-be/ACCEPT.md` | `invites.rs`, `invites_contract.rs` | 20/20 pass, 5/5 мутаций |
| 15 | `STUB-14-UI-SEARCH-FIX` | `waves/RW-40/PM/stub14-search-ui/ACCEPT.md` | `search-overlay.js`, `search.js` | 108/108 pass, 5/5 мутаций |
| 16 | `P-03-UI-TYPING-INDICATOR` | `waves/RW-40/PM/p03-typing-ui/ACCEPT.md` | `typing-indicator.js`, `gui-spike-dom.test.mjs` | 58/58 pass, 5/5 мутаций |
| 17 | `M-10-M-11-PINS-THREADS-BACKEND` | `waves/RW-40/PM/m10-pins-be/ACCEPT.md` | `pins_threads.rs`, `pins_threads_contract.rs` | 6/6 pass, 5/5 мутаций |
| 18 | `STUB-27-CORE-DISPATCH-INTEGRATION` | `waves/RW-40/PM/stub27-dispatch/ACCEPT.md` | `dispatch.rs`, `dispatch_types.rs` | 583/583 pass, 5/5 мутаций |
| 19 | `STUB-05-INVITES-MODAL-UI` | `waves/RW-40/PM/stub05-invites-ui/ACCEPT.md` | `invite-modal.js`, `join-server.test.mjs` | 35/35 pass, 5/5 мутаций |
| 20 | `M-08-MARKDOWN-MENTIONS-UI` | `waves/RW-40/PM/m08-markdown/ACCEPT.md` | `markdown.js`, `gui-spike-model.test.mjs` | 48/48 pass, 5/5 мутаций |
| 21 | `V-07-VOICE-DEVICES-CONTRACT` | `waves/RW-40/PM/v07-devices/ACCEPT.md` | `devices.rs`, `devices_contract.rs` | 5/5 pass, 5/5 мутаций |

---

## 3. Сверка контрольных сумм MD5 по рабочему дереву

Прямой пересчёт утилитой `md5sum` по актуальным файлам репозитория:

| Файл рабочего дерева | Актуальный MD5 | Источник ACCEPT / Срез | Статус соответствия |
|---|---|---|---|
| `rust/gui-spike/src/settings_store.rs` | `ac58e876a6553e0d00cd881f30622b0b` | `g01b-contract/ACCEPT.md` | **Совпал** [V] |
| `rust/gui-spike/tests/settings_contract.rs` | `0e943bb9a487d77718cb37b3338e4ea6` | `g01b-contract/ACCEPT.md` | **Совпал** [V] |
| `rust/crates/discord-core/src/ws.rs` | `8703c1d695cd0af26458393eb3c1e3f9` | `l02-core/ACCEPT.md` | **Совпал** [V] |
| `rust/crates/discord-core/tests/gateway_supervisor.rs` | `7f23d85a4d3b0e54c58d765b8b879c58` | `l02-core/ACCEPT.md` | **Совпал** [V] |
| `rust/gui-spike/ui/features/dispatch-model.js` | `37dacc61d7daf74c144681637dded1f5` | `l02-ui/ACCEPT.md` | **Совпал** [V] |
| `tests/unit/dispatch_ui.test.mjs` | `d30771d65b6373ecbbac6de1f9e61a3d` | `l02-ui/ACCEPT.md` | **Совпал** [V] |
| `.github/workflows/ci.yml` | `608e26837c10996aae11d2c29fc6d320` | `QA-M2-04-ACCEPT.md` | **Совпал** [V] |
| `tests/meta/ci-browser.test.mjs` | `06ab44eee0c919777cbaec91756adaff` | `QA-M2-04-ACCEPT.md` | **Совпал** [V] |
| `rust/gui-spike/src/messages.rs` | `c449a06c783c795effb33c7fc763614e` | `plat-11a/ACCEPT.md` | **Совпал** [V] |
| `rust/gui-spike/tests/messages_contract.rs` | `0f92348a5e3e15563603faec0917c4b5` | `plat-11a/ACCEPT.md` | **Совпал** [V] |
| `rust/gui-spike/src/data.rs` | `8013271de7a6444b2b9dba3301a165d4` | `m07-be3/ACCEPT.md` | **Совпал** [V] |
| `rust/gui-spike/tests/read_state_contract.rs` | `fde688f33614636c2699c55ebba945e9` | `m07-be3/ACCEPT.md` | **Совпал** [V] |
| `rust/gui-spike/src/net.rs` | `b286d1779fc89f1dd6c829dc3a32dba7` | `sec-l02-01/ACCEPT.md` | **Совпал** [V] |
| `rust/gui-spike/tests/net_contract.rs` | `573d271204f3c1db601e2df73684f129` | `sec-l02-01/ACCEPT.md` | **Совпал** [V] |
| `rust/gui-spike/src/search.rs` | `6bae3ea23dfe31d1929fe3f7eb926fd0` | `b7b-search/ACCEPT.md` | **Совпал** [V] |
| `rust/gui-spike/tests/search_contract.rs` | `a02006481e7e97ab26667367be8ce902` | `b7b-search/ACCEPT.md` | **Совпал** [V] |
| `rust/gui-spike/ui/features/chat.js` | `1e73c60219792da3c332b8cbc73c7607` | `m07-ui-badge/ACCEPT.md` | **Совпал** (включает C-02) [V] |
| `tests/unit/read-state-ui.test.mjs` | `fe363afe9abcc82bc99e1e59b0af58c5` | `m07-ui-badge/ACCEPT.md` | **Совпал** [V] |
| `rust/crates/discord-core/src/stream.rs` | `e4e0450559bcf947261ca7bc91aea91f` | `stub27-core/ACCEPT.md` | **Совпал** [V] |
| `rust/crates/discord-core/tests/stream_gateway_contract.rs` | `99a5577644f436a467bac50a676dff3d` | `stub27-core/ACCEPT.md` | **Совпал** [V] |
| `rust/crates/discord-voice/src/session.rs` | `395df9d9408f50435a966b881870265e` | `v01-rtc/ACCEPT.md` | **Совпал** [V] |
| `rust/crates/discord-voice/tests/session_contract.rs` | `c59ae2dedeb10eb0c9cce55c87cb6244` | `v01-rtc/ACCEPT.md` | **Совпал** [V] |
| `rust/gui-spike/src/invites.rs` | `951bb4bc5f79b8e96ac3d96f8f5bafca` | `stub05-invites-be/ACCEPT.md` | **Совпал** [V] |
| `rust/gui-spike/tests/invites_contract.rs` | `f6a8d252207558d7cae0aa58a67368e4` | `stub05-invites-be/ACCEPT.md` | **Совпал** [V] |
| `rust/gui-spike/ui/features/search-overlay.js` | `860e9237a00e4eacd66e343bbeb5ab45` | `stub14-search-ui/ACCEPT.md` | **Совпал** [V] |
| `rust/gui-spike/ui/features/search.js` | `b5c5c3e6538fa0f6c24c45c3ac1c36a0` | `stub14-search-ui/ACCEPT.md` | **Совпал** [V] |
| `tests/unit/search-remote-real-index.test.mjs` | `b2692ff1bb6b0fb107e998ac2f5b2c2d` | `stub14-search-ui/ACCEPT.md` | **Совпал** [V] |
| `rust/gui-spike/ui/features/typing-indicator.js` | `8c50cf9be15519b2fdc6906769d784ae` | `p03-typing-ui/ACCEPT.md` | **Совпал** [V] |
| `tests/unit/gui-spike-dom.test.mjs` | `e39c8a68936367fef0f51fa27a594b31` | `p03-typing-ui/ACCEPT.md` | **Совпал** [V] |
| `rust/gui-spike/src/pins_threads.rs` | `839e78e1a62e585483076a329c9c0dbc` | `m10-pins-be/ACCEPT.md` | **Совпал** [V] |
| `rust/gui-spike/tests/pins_threads_contract.rs` | `55c4f3344ba5f66c2600a59e9fd34ff0` | `m10-pins-be/ACCEPT.md` | **Совпал** [V] |
| `rust/crates/discord-core/src/dispatch.rs` | `528597e9f5aa4b894982cbd3f5cbda38` | `stub27-dispatch/ACCEPT.md` | **Совпал** [V] |
| `rust/crates/discord-core/tests/dispatch_types.rs` | `af859f628901986aa523b01ea1c507b1` | `stub27-dispatch/ACCEPT.md` | **Совпал** [V] |
| `rust/gui-spike/ui/features/invite-modal.js` | `d3d96c622325715a437334af431f91f7` | `stub05-invites-ui/ACCEPT.md` | **Совпал** [V] |
| `tests/unit/join-server.test.mjs` | `8b790b9178f40aeed91346e5984713f9` | `stub05-invites-ui/ACCEPT.md` | **Совпал** [V] |
| `rust/gui-spike/ui/features/markdown.js` | `85f1cbd92bec6e9d5fc157bd2e8fdeb8` | `m08-markdown/ACCEPT.md` | **Совпал** [V] |
| `tests/unit/gui-spike-model.test.mjs` | `e975a4d45da485beebcefb0319db950f` | `m08-markdown/ACCEPT.md` | **Совпал** [V] |
| `rust/crates/discord-voice/src/devices.rs` | `ceaa10efd2729455e76ba05656f2fc98` | `v07-devices/ACCEPT.md` | **Совпал** [V] |
| `rust/crates/discord-voice/tests/devices_contract.rs` | `65c2b4b0d6b29e55276046b68f5bae57` | `v07-devices/ACCEPT.md` | **Совпал** [V] |

---

## 4. Паритет функций (PARITY-MATRIX и GAP-BACKLOG)

В результате приёмки 21 среза:
- `M-10` (Закреплённые сообщения / пины): переведён из `absent` в **`partial`** (бэкенд `pins_threads.rs` 6/6 pass, 5/5 мутаций принят);
- `M-11` (Ветки каналов / threads): переведён из `absent` в **`partial`** (бэкенд `pins_threads.rs` 6/6 pass, 5/5 мутаций принят);
- `V-07` (Устройства ввода/вывода, Mute/Deafen/PTT): переведён из `absent` в **`partial`** (бэкенд `devices.rs` 5/5 pass, 5/5 мутаций принят);
- `P-03` (Индикатор набора): обновлён — UI-контроллер `#chat-typing-indicator` принят (38/38 pass, 5/5 мутаций);
- `G-06` (Инвайты): обновлён — UI модалки `#invite-create-modal` принят (35/35 pass, 5/5 мутаций);
- `M-08` (Markdown / упоминания / спойлеры): обновлён — AST-парсер и DOM-рендерер принят (48/48 pass, 5/5 мутаций).

Счётчики функций эталона (§14):
- **implemented (live unverified): 22**
- **partial: 42** (+3: M-10, M-11, V-07)
- **absent: 27** (−3: M-10, M-11, V-07)
- **unknown: 0**
- **Всего: 91**

Сходимость по приоритетам: P0: 5, P1: 32, P2: 40, P3: 14 (сумма 91). Полная независимая проверка `awk` подтверждена.

---

## 5. Запуск интеграционной волны Wave 8

| Карточка / Задача | Отдел / Исполнитель | Скоуп файлов и интеграционная цель | Статус |
|---|---|---|---|
| `INT-01-RUST-WIRING` | G1 Backend (`gemini-4-argon-high`) | Экспорт и интеграция `invites.rs`, `pins_threads.rs`, `stream.rs`, `devices.rs` в `rust/gui-spike/src/lib.rs` / `main.rs` | **IN PROGRESS** |
| `INT-02-UI-CHAT-MARKDOWN-TYPING` | G2 Frontend Lane A (`gemini-3.8-flash-high`) | Подключение `markdown.js` и `typing-indicator.js` в `rust/gui-spike/ui/features/chat.js` | **IN PROGRESS** |
| `INT-03-UI-APP-PANELS-MODALS` | G2 Frontend Lane B (`gemini-3.8-flash-high`) | Подключение `invite-modal.js`, панелей пинов/веток и оверлея поиска в `rust/gui-spike/ui/app.js` | **IN PROGRESS** |
| `G05-REST-MUTATIONS-FIX` | G2 Frontend Lane C (`deepseek-flash`) | Раздельный отлов мутаций M1 (`aria-disabled`) и M1b (`guild-invite`) в `server-menu.test.mjs` для закрытия замечания Codex | **IN PROGRESS** |
| `Codex Review 15A/15B` | Codex (`gpt-6-luna`) | Независимое интеграционное ревью сквозных диффов и мутаций Wave 8 | **STANDBY / PIPELINE** |

---

## 6. Шаги до разблокировки слияния (Merge Checklist)

1. **Завершение Wave 8:**
   - Успешное выполнение сквозных сценариев в `INT-01`, `INT-02`, `INT-03`.
   - Закрытие замечания Codex по мутациям в `G05-REST-MUTATIONS-FIX`.
2. **Получение ревью Codex 15A / 15B:**
   - Подтверждение чистоты интеграционных правок (P0=0, P1=0, P2=0).
3. **Единый прогон интеграционного гейта:**
   - Запуск `cargo test --workspace` и `npm run gate:worker` с сохранением актуального `.gate/worker-*.json`.
4. **Фиксация в журнале волн:**
   - Append события RW-40 в `waves/db.jsonl` и обновление `waves/STATE.md`.
5. **Создание интеграционного коммита координатором (L1).**
