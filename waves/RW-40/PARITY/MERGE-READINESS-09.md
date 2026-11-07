# MERGE-READINESS-09 — 15 ACCEPT-срезов, Wave 6 завершена (Batch 13A/13B на ревью), сверка md5

Дата: **2026-10-30**, снимок **17:45–17:50 +05**. Ветка `main`, HEAD
`4e240481e59ff4d49732ac8bd5ba08a006363748` (integration commit RW-40 от 2026-10-24 16:22).
Воркер: **G6 Parity**, отдел G6, карточка **G6-PARITY-SYNC-09**.
Режим — **read-only по продуктовому коду и тестам**.

> **Вход прохода.** `MERGE-READINESS-08.md`, `PARITY-MATRIX.md`, `GAP-BACKLOG.md`.
> **15 ACCEPTED срезов:**
> 1. `G-01b-CONTRACT` (`PM/g01b-contract/ACCEPT.md`)
> 2. `L-02-CORE` (`waves/RW-40/PM/l02-core/ACCEPT.md`)
> 3. `L-02-UI` (`waves/RW-40/PM/l02-ui/ACCEPT.md`)
> 4. `C-02-UI-OLDER-FIX` (`waves/RW-40/PM/c02-ui-older/ACCEPT.md`)
> 5. `PLAT-11a` (`waves/RW-40/PM/plat-11a/ACCEPT.md`)
> 6. `M-07-BE3` (`waves/RW-40/PM/m07-be3/ACCEPT.md`)
> 7. `QA-M2-04` (`QUALITY/QA-M2-04-ACCEPT.md`)
> 8. `SEC-L02-01-FIX` (`waves/RW-40/PM/sec-l02-01/ACCEPT.md`)
> 9. `B7b-SEARCH-FILTERS-BACKEND` (`waves/RW-40/PM/b7b-search/ACCEPT.md`)
> 10. `M-07-UI-BADGE` (`waves/RW-40/PM/m07-ui-badge/ACCEPT.md`)
> 11. `G3-STREAM-FIX-PASS2` / `STREAM-REVIEW-PASS3` (`waves/RW-40/DESIGN/STREAM-REVIEW-PASS3.md`, PASS F-02..F-04)
> 12. `STUB-27-CORE-ORACLE-FIX` (`waves/RW-40/PM/stub27-core/ACCEPT.md`)
> 13. `V-01-RTC-FIX` (`waves/RW-40/PM/v01-rtc/ACCEPT.md`)
> 14. `STUB-05-INVITES-BACKEND` (`waves/RW-40/PM/stub05-invites-be/ACCEPT.md`)
> 15. `STUB-14-UI-SEARCH-FIX` (`waves/RW-40/PM/stub14-search-ui/ACCEPT.md`)
>
> **Кодовые пакеты Wave 6 (переданы на независимое ревью Codex Batch 13A / 13B):**
> - `P-03-UI-TYPING-INDICATOR` (`typing-indicator.js`, 38/38 pass, 5/5 мутаций, `waves/RW-40/PM/p03-typing-ui/REPORT.md`)
> - `STUB-05-INVITES-MODAL-UI` (`invite-modal.js`, 35/35 pass, 5/5 мутаций, `waves/RW-40/PM/stub05-invites-ui/REPORT.md`)
> - `M-08-MARKDOWN-MENTIONS-UI` (`markdown.js`, 48/48 pass, 5/5 мутаций, `waves/RW-40/PM/m08-markdown/REPORT.md`)
> - `M-10-M-11-PINS-THREADS-BACKEND` (`pins_threads.rs`, 6/6 pass, 5/5 мутаций, `waves/RW-40/PM/m10-pins-be/REPORT.md`)
> - `V-07-VOICE-DEVICES-CONTRACT` (`devices.rs`, 5/5 pass, 5/5 мутаций, `waves/RW-40/PM/v07-devices/REPORT.md`)
> - `TYPING-MARKDOWN-SPEC.md` (`waves/RW-40/DESIGN/TYPING-MARKDOWN-SPEC.md`)

Запреты ТЗ соблюдены: продуктовый код и тесты не изменялись, `cargo` не запускался, `git commit`
не производился. Сверка md5 проведена утилитой `md5sum` напрямую по рабочему дереву.

---

## 0. Резюме

1. **Вердикт: merge — `block`. Parity — `in-progress / review-pending`.**
   - Блокер слияния: отсутствие финального integration-коммита и gate-артефакта (`.gate/worker-*.json`) на текущем снимке дерева (последний gate — `0239543` от 2026-10-24).
   - Пакеты Wave 6 полностью реализованы и покрыты тестами воркерами, но ожидают подписания независимым ревью Codex (Batch 13A и Batch 13B).
2. **Accepted set расширен до 15 срезов** (включая 14 официальных `ACCEPT.md` + формально принятый ревью-пасс дизайн-спеки `STREAM-REVIEW-PASS3`).
   - Новые принятые срезы после прохода 08:
     - `STUB-27-CORE-ORACLE-FIX`: byte-level RE-оракул Go Live (`stream.rs`, `stream_gateway_contract.rs`, 11/11 pass, 5/5 мутаций убито).
     - `V-01-RTC-FIX`: сессия RTC и реконнект `discord-voice` (`session.rs`, `session_contract.rs`, 147/147 pass, 7/7 мутаций убито).
     - `STUB-05-INVITES-BACKEND`: бэкенд создания инвайтов канала (`invites.rs`, `invites_contract.rs`, 20/20 pass, 5/5 мутаций убито).
     - `STUB-14-UI-SEARCH-FIX`: UI фильтров поиска (`search-overlay.js`, `search.js`, `search-remote-real-index.test.mjs`, 108/108 pass, 5/5 мутаций убито).
     - `G3-STREAM-FIX-PASS2` / `STREAM-REVIEW-PASS3`: закрыты F-02..F-04 в `STREAM.md` (P0=0, P1=0).
3. **Статус Wave 6 (Batch 13A / Batch 13B):**
   - Все 5 карточек кода завершены в строгом TDD (red -> green), с полными мутационными протоколами (5/5 убитых мутантов в каждой), отчёты и артефакты сложены в `waves/RW-40/PM/` и переданы Codex:
     - Lane A: `P-03-UI-TYPING-INDICATOR` (`typing-indicator.js`, 38/38 pass, 5/5 мутаций);
     - Lane B: `STUB-05-INVITES-MODAL-UI` (`invite-modal.js`, 35/35 pass, 5/5 мутаций);
     - Lane C: `M-08-MARKDOWN-MENTIONS-UI` (`markdown.js`, 48/48 pass, 5/5 мутаций);
     - Lane D: `M-10-M-11-PINS-THREADS-BACKEND` (`pins_threads.rs`, 6/6 pass, 5/5 мутаций);
     - Lane E: `V-07-VOICE-DEVICES-CONTRACT` (`devices.rs`, 5/5 pass, 5/5 мутаций);
     - Lane F: дизайн-спецификация `TYPING-MARKDOWN-SPEC.md`.
4. **Сходимость матрицы паритета и бэклога:**
   - 91 функция эталона: **22 implemented (live unverified) / 39 partial / 30 absent / 0 unknown**.
   - Переведены в `partial`:
     - `V-01` (`absent → partial`) благодаря приёмке `session.rs` (V-01-RTC-FIX ACCEPT);
     - `G-06` (`absent → partial`) благодаря приёмке `invites.rs` (STUB-05-INVITES-BACKEND ACCEPT).
   - Строки `M-10`, `M-11`, `V-07`, `P-03`, `M-08` обновлены описанием текущего состояния (код Wave 6 на ревью Codex).

---

## 1. Снимок репозитория на момент синхронизации

| Параметр | Значение | Метка |
|---|---|---|
| HEAD | `4e240481e59ff4d49732ac8bd5ba08a006363748` (2026-10-24 16:22:06 +0500) | [V] |
| Неотслеживаемые / изменённые файлы (`git status --porcelain -uall`) | **272** пути на 17:47 | [V] |
| Завершённые волны в журнале | `waves/db.jsonl` — RW-40 ожидает финальной фиксации при закрытии волны | [V] |
| Активные кодовые пакеты Wave 6 | 5 продуктовых модулей + 1 дизайн-спека переданы на ревью Codex | [V] |

---

## 2. Реестр 15 принятых срезов (ACCEPTED Set)

| # | Срез | Файл приёмки / ревью | Ключевые файлы кода / тестов | Статус проверок |
|---:|---|---|---|---|
| 1 | `G-01b-CONTRACT` | `PM/g01b-contract/ACCEPT.md` | `settings_store.rs`, `settings_contract.rs` | 2/2 совпали, PASS |
| 2 | `L-02-CORE` | `waves/RW-40/PM/l02-core/ACCEPT.md` | `discord-core/src/ws.rs`, `gateway_supervisor.rs` | 2/2 совпали, PASS |
| 3 | `L-02-UI` | `waves/RW-40/PM/l02-ui/ACCEPT.md` | `dispatch-model.js`, `dispatch_ui.test.mjs` | совпали (app.js superseding) |
| 4 | `C-02-UI-OLDER-FIX` | `waves/RW-40/PM/c02-ui-older/ACCEPT.md` | `chat.js`, `channel-paging.test.mjs` | chat.js superseding |
| 5 | `PLAT-11a` | `waves/RW-40/PM/plat-11a/ACCEPT.md` | `ci.yml`, `ci-browser.test.mjs` | 2/2 совпали, PASS |
| 6 | `M-07-BE3` | `waves/RW-40/PM/m07-be3/ACCEPT.md` | `data.rs`, `read_state_contract.rs` | 2/2 совпали, PASS |
| 7 | `QA-M2-04` | `QUALITY/QA-M2-04-ACCEPT.md` | `messages.rs`, `messages_contract.rs` | 2/2 совпали, PASS |
| 8 | `SEC-L02-01-FIX` | `waves/RW-40/PM/sec-l02-01/ACCEPT.md` | `net.rs`, `net_contract.rs` | 2/2 совпали, 2/2 мутации |
| 9 | `B7b-SEARCH-FILTERS-BACKEND` | `waves/RW-40/PM/b7b-search/ACCEPT.md` | `search.rs`, `search_contract.rs` | 2/2 совпали, 6/6 мутаций |
| 10 | `M-07-UI-BADGE` | `waves/RW-40/PM/m07-ui-badge/ACCEPT.md` | `chat.js`, `read-state-ui.test.mjs` | 2/2 совпали, supersedes c02 |
| 11 | `G3-STREAM-FIX-PASS2` | `waves/RW-40/DESIGN/STREAM-REVIEW-PASS3.md` | `waves/RW-40/DESIGN/STREAM.md` | PASS F-02..F-04 (P0=0, P1=0) |
| 12 | `STUB-27-CORE-ORACLE-FIX` | `waves/RW-40/PM/stub27-core/ACCEPT.md` | `stream.rs`, `stream_gateway_contract.rs` | 11/11 pass, 5/5 мутаций |
| 13 | `V-01-RTC-FIX` | `waves/RW-40/PM/v01-rtc/ACCEPT.md` | `session.rs`, `session_contract.rs` | 147/147 pass, 7/7 мутаций |
| 14 | `STUB-05-INVITES-BACKEND` | `waves/RW-40/PM/stub05-invites-be/ACCEPT.md` | `invites.rs`, `invites_contract.rs` | 20/20 pass, 5/5 мутаций |
| 15 | `STUB-14-UI-SEARCH-FIX` | `waves/RW-40/PM/stub14-search-ui/ACCEPT.md` | `search-overlay.js`, `search.js` | 108/108 pass, 5/5 мутаций |

---

## 3. Сверка контрольных сумм (md5) принятых файлов кода

| Файл | Фактический md5 в дереве | Источник / ACCEPT | Статус |
|---|---|---|---|
| `rust/crates/discord-core/src/stream.rs` | `e4e0450559bcf947261ca7bc91aea91f` | `stub27-core/ACCEPT.md` | **Совпал** [V] |
| `rust/crates/discord-core/tests/stream_gateway_contract.rs` | `99a5577644f436a467bac50a676dff3d` | `stub27-core/ACCEPT.md` | **Совпал** [V] |
| `rust/crates/discord-voice/src/session.rs` | `395df9d9408f50435a966b881870265e` | `v01-rtc/ACCEPT.md` | **Совпал** [V] |
| `rust/crates/discord-voice/tests/session_contract.rs` | `c59ae2dedeb10eb0c9cce55c87cb6244` | `v01-rtc/ACCEPT.md` | **Совпал** [V] |
| `rust/gui-spike/src/invites.rs` | `951bb4bc5f79b8e96ac3d96f8f5bafca` | `stub05-invites-be/ACCEPT.md` | **Совпал** [V] |
| `rust/gui-spike/tests/invites_contract.rs` | `f6a8d252207558d7cae0aa58a67368e4` | `stub05-invites-be/ACCEPT.md` | **Совпал** [V] |
| `rust/gui-spike/ui/features/search-overlay.js` | `860e9237a00e4eacd66e343bbeb5ab45` | `stub14-search-ui/ACCEPT.md` | **Совпал** [V] |
| `rust/gui-spike/ui/features/search.js` | `b5c5c3e6538fa0f6c24c45c3ac1c36a0` | `stub14-search-ui/ACCEPT.md` | **Совпал** [V] |
| `tests/unit/search-remote-real-index.test.mjs` | `b2692ff1bb6b0fb107e998ac2f5b2c2d` | `stub14-search-ui/ACCEPT.md` | **Совпал** [V] |
| `rust/gui-spike/src/net.rs` | `b286d1779fc89f1dd6c829dc3a32dba7` | `sec-l02-01/ACCEPT.md` | **Совпал** [V] |
| `rust/gui-spike/tests/net_contract.rs` | `573d271204f3c1db601e2df73684f129` | `sec-l02-01/ACCEPT.md` | **Совпал** [V] |
| `rust/gui-spike/ui/features/chat.js` | `1e73c60219792da3c332b8cbc73c7607` | `m07-ui-badge/ACCEPT.md` | **Совпал** [V] |
| `tests/unit/read-state-ui.test.mjs` | `fe363afe9abcc82bc99e1e59b0af58c5` | `m07-ui-badge/ACCEPT.md` | **Совпал** [V] |
| `rust/crates/discord-core/src/dispatch.rs` | `528597e9f5aa4b894982cbd3f5cbda38` | `stub27-dispatch/REPORT.md` | Сохранён [V] |

---

## 4. Пакет Wave 6: передано на Codex-ревью (Batch 13A / Batch 13B)

Все файлы проверены: тесты зелёные, мутации убиты, md5 зафиксированы для сверки ревьюером:

| Карточка | Пакет файлов | Тесты | Мутации | Контрольная сумма md5 |
|---|---|---|---|---|
| `P-03-UI-TYPING-INDICATOR` | `rust/gui-spike/ui/features/typing-indicator.js` | 38/38 pass | 5/5 killed | `8c50cf9be15519b2fdc6906769d784ae` |
| `STUB-05-INVITES-MODAL-UI` | `rust/gui-spike/ui/features/invite-modal.js` | 35/35 pass | 5/5 killed | `d3d96c622325715a437334af431f91f7` |
| `M-08-MARKDOWN-MENTIONS-UI` | `rust/gui-spike/ui/features/markdown.js` | 48/48 pass | 5/5 killed | `85f1cbd92bec6e9d5fc157bd2e8fdeb8` |
| `M-10-M-11-PINS-THREADS-BACKEND` | `rust/gui-spike/src/pins_threads.rs` | 6/6 pass | 5/5 killed | `839e78e1a62e585483076a329c9c0dbc` |
| `V-07-VOICE-DEVICES-CONTRACT` | `rust/crates/discord-voice/src/devices.rs` | 5/5 pass | 5/5 killed | `ceaa10efd2729455e76ba05656f2fc98` |
| Спецификация | `waves/RW-40/DESIGN/TYPING-MARKDOWN-SPEC.md` | — | — | `08f016cd4d018584e98b9abd70317d5d` |

---

## 5. Паритет функций (PARITY-MATRIX и GAP-BACKLOG)

В результате синхронизации после Review Batch 11/12A/12B:
- `V-01` (Голосовая сессия): переведён из `absent` в **`partial`** (бэкенд `session.rs` и контракт `session_contract.rs` полностью приняты с 7/7 мутациями);
- `G-06` (Инвайты): переведён из `absent` в **`partial`** (бэкенд создания инвайтов `invites.rs` принят с 5/5 мутациями, UI-модалка реализована в 13A);
- `S-02` (Screen share / Go Live): подтверждён статус **`partial`** (принят byte-level оракул шлюза `stream.rs` с 5/5 мутациями, ревью дизайн-спеки pass 3 PASS);
- `M-13` (Поиск по серверу): подтверждён статус **`implemented (live unverified)`** (бэкенд и UI фильтров поиска приняты с 5/5 мутациями).

Счётчики функций эталона (§14):
- **implemented (live unverified): 22**
- **partial: 39** (+2: V-01, G-06)
- **absent: 30** (−2: V-01, G-06)
- **unknown: 0**
- **Всего: 91**

Сходимость по приоритетам: P0: 5, P1: 32, P2: 40, P3: 14 (сумма 91). Полная независимая проверка `awk` подтверждена.

---

## 6. Шаги до разблокировки слияния (Merge Checklist)

1. **Завершение независимого ревью Codex Batch 13A и Batch 13B:**
   - Получение `REVIEW.md` и подписанных `ACCEPT.md` для P-03, STUB-05-UI, M-08, M-10/M-11, V-07.
2. **Integration-патч Team Lead:**
   - Подключение UI-модулей в `app.js` и контейнеры представлений.
3. **Единый прогон интеграционного гейта:**
   - Запуск `cargo test --workspace` и `npm run gate:worker` для фиксации `.gate/worker-*.json` на актуальном дереве.
4. **Фиксация в журнале волн:**
   - Append события RW-40 в `waves/db.jsonl` и обновление `waves/STATE.md`.
5. **Создание интеграционного коммита координатором (L1).**
