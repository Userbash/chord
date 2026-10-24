# G6-BATCH-MERGE-01 — приёмка batch перед общим слиянием

Дата: 2026-10-24. Автор: G6 Lead (RE Parity), subagent PM DeepSeek.
Режим: **read-only**. Не запускались: Discord/окно, live, сеть, CDP/AT-SPI, `cargo`, `npm`,
commit. Продуктовый код (`ui/**`, `gui-spike/src/**`, `rust/crates/**`) и чужие документы
не изменялись. Выход: этот отчёт + `REA-SESSION.md` §12.

Источники: `G6-REPORT-RW40-02.md`, `PARITY-MATRIX.md`, `GAP-BACKLOG.md`, `MERGE-POLICY.md`,
`MERGE-MANIFEST.md`, `TASKS.md`, `QUALITY/BUGS.md`, `STUB-TRACKER.md`, отчёты
`PM/{g01,c02-be,c02-ui,g03,g03-ui,g03a-ui,g04,g05-ui,m07,m07-be2,m07-ui,p03,dp02,l01,
sec-01,sec-02,plat-01,plat-02,qa4-01a,qa4-01b,qa4-02,qa4-03,plat-11-meta}/REPORT.md`,
текущий код и `.gate/release-2026-10-24T09-19-36-946Z.json`.

---

## 0. Резюме

1. **Вердикт: `deferred`** (не `block`). Вымысла нет, P0=0, гейт `passed_with_skips`;
   merge возможен только после 5 условий §5.
2. Все 25 срезов batch (BE/UI-части считаются отдельно) имеют red-артефакт (полный или
   inline в отчёте) и зелёный targeted; новых «придуманных» endpoint/полей не найдено. Расширения каталога
   `/typing` и `/invites/{code}` — отдельные `[S]`-оракулы, frozen D-02 не размыт
   (`ROUTES` = 49, TSV md5 `2db6b39b…` не менялся).
3. Реально закрыты офлайн: **G-01, C-02, G-03, G-03a, G-04, M-07, L-01** (7 строк
   матрицы можно перевести в `implemented (live unverified)`).
   **P-03 — только backend** (UI индикатора нет), **G-05 — 2 активных пункта из 10**,
   **M-06 (реакции) в реальном канале отсутствует** — это открытые P1/P2 среза.
4. e2e: **4 skip ≠ pass** (нет браузера/CDP). Гейт-вердикт `passed_with_skips`, не
   `passed`. 12 `ignored` workspace — live-keyring, отдельно закрыты фазой
   `keyring-live` 12/12.
5. Найден **P2 evidence drift**: md5 шести тестовых файлов не совпадают с отчётными,
   отдельной карточки/лога на этот дрейф нет. Счётчики кейсов новых файлов совпадают,
   гейт зелёный, но «не ослаблено» доказывается прогоном, а не диффом (baseline
   `rust/gui-spike/**` вне git).
6. Журнал волн по RW-40 **пуст**: `waves/db.jsonl` — 0 событий `RW-40`, `waves/STATE.md`
   обновлён по 2026-10-23. Это блокер не кода, а закрытия batch (см. §5, п.4).

---

## 1. Batch: функция → evidence → состояние → тест → accepted

Уровни: `[L]` live · `[S]` call-site эталона · `[D]` только Enum/путь · `[M]` измерение ·
`[A]` не подтверждено. `implemented` ниже = **офлайн-путь в приложении + тесты**, не E3.

| # | Карточка | Функция | Evidence | Состояние в коде (проверено) | Тест / оракул | Accepted |
|---:|---|---|---|---|---|---|
| 1 | G-01 | Рейл серверов из READY | `[L]` READY.guilds, `[M]` рейл | **implemented (offline)**: `ready()` после auth ready; 7 плиток `data-guild-id`, loading/empty/error+retry, ↑/↓/Home/End; статичных `data-guild="1|2|3"` нет | `tests/unit/guild-rail.test.mjs` (8; оракул `fixtures/.../002-READY.json`) | accepted; D-* в отчёте не присвоен |
| 2 | C-02-BE | Проекция каналов READY | `[L]` READY.channels (272 + 17 ЛС, типы 0/2/4/5/15) | **implemented**: `data::channels()` + Tauri `channels`; порядок гильдий READY, `position/id`; ошибки типизированы | `channels_contract.rs` (6) + `data_contract.rs`; оракул — READY | accepted |
| 3 | C-02-UI | Переключение канала + история | `[L]` GET messages (`D2:70`), `[M]` switch | **implemented (offline)**: дерево из `channels()`, `messages_page` реальным id, per-channel окно/scroll, `data-message-id` из `item.id`; **G6-D-01 закрыт (D-120)**; G6-D-02 (`messageId=index`) фактически закрыт тем же срезом, но в `BUGS.md` не заведён | `channel-switch.test.mjs` (10 дин.); реальные id из `MESSAGE_*` | accepted |
| 4 | G-03-BE | Resolve/accept инвайта | `[S]` path + 3 nullable поля; live-коды `[A]` | **implemented**: `invites.rs`; accept тело ровно `{session_id, invite_instance_id, received_installation_id}`, `context` не в теле; 2xx без объекта = `InvalidResponse`; bearer только backend | `invites_contract.rs` (14) + отдельный `[S]`-оракул (D-02 не расширяется) | **D-116** |
| 5 | G-03-UI | Модалка join | `[S]` контракт + `DESIGN/SERVER-MENU.md` | **implemented (offline)**: состояния idle/resolving/preview/accepting/accepted/error, 429-cooldown, Esc-guard, accept `{code}` без nullable-полей | `join-server.test.mjs` (16) | **D-122** |
| 6 | G-03a-BE | Enter guild/lurker | `[L]` `PUT …?lurker=true` `{}` → 204×2 | **implemented** (принят ранее) | `guilds_contract.rs` | **D-76** |
| 7 | G-03a-UI | «Открыть сервер» в preview | `[L]` backend + design | **implemented (offline)**: только при `guildId` из resolve; плитка в рейл не добавляется; 403/429/401/сеть видны; 1 клик = 1 IPC | `join-server.test.mjs` (+8, всего 24) | accepted (STUB-04, **D-128**) |
| 8 | G-04-BE | Leave server | `[S]` `DELETE /users/@me/guilds/{id}` + `{lurking}`; коды `[A]` | **implemented**: `guilds.rs`, тело `{lurking: bool}`, 2xx без чтения тела, `delete` через `ApiClient::execute` (ядро не менялось) | `guilds_contract.rs` (13+11) | **D-89** |
| 9 | G-04-UI | Confirm выхода | `[S]` backend + `[M]` меню | **implemented**: `alertdialog`, ровно один `leave_guild({guildId, lurking})`, плитка удаляется после 2xx, owner-disabled | `gui-spike-features.test.mjs` (+10) | **D-89/D-92** |
| 10 | G-05-UI | Меню сервера | `[M]` `UI2:D1` (10 пунктов) | **partial**: поповер/ARIA/клавиатура есть; активны **2 из 10** (leave, copy-id), invite disabled по capability, остальные — статичные заглушки | `gui-spike-features.test.mjs` | **D-92** |
| 11 | M-07-BE | Ack REST | `[L]` тело `{token,last_viewed}`, 200×5, `last_viewed=4310` | **implemented**: `messages.rs`, тело дословно, `token: Option` (не bearer), id валидируются до сети | `rest_ack.rs` (9); оракул `rest-d02 §4` | **D-126** |
| 12 | M-07-BE2 | READY `read_state` | `[L]` READY.read_state (157 записей) | **implemented**: ровно 6 camelCase-полей; `null`/badge не превращаются в 0 | `read_state_contract.rs` (12) + READY-оракул | accepted |
| 13 | M-07-UI | Ack из UI | `[L]` ack + политика триггеров `[A]` | **implemented (offline)**: open/focus/scroll-to-bottom, дедуп + 3 с, `token: null`, бенч `news` не ack'ает; unread только по `mentionCount` | `read-state-ui.test.mjs` (8) + `CONTRACTS/read-state.md` | accepted |
| 14 | P-03-BE | `POST /channels/{id}/typing` | `[S]` path/без тела/200 + 2 поля; TTL `[A]` | **backend implemented; UI отсутствует** (в `ui/**` нет `send_typing`/индикатора) | `typing_contract.rs` (13) + отдельный `[S]`-оракул | **D-108** |
| 15 | DP-02 | Высоты 26/48/+22, member 42 | `[M]`/`[L]` `RW-37/RE/layout.json` | **implemented** (геометрия): `messageHeight` — единственный источник, prefix-sum/top/indexAt, member 42; legacy 44/40 оставлены только для geometry-check | `model/features/bench` (+16), `geometry-check` 31/31 | **D-102**; D-04 |
| 16 | L-01 | Reconnect/offline в UI | `[M]` UI + `[L]` `GatewayStatus` | **implemented (offline)**: `#net-status` (role=status) + poll 2 с + retry; семантика resume/4013/4014 не дублируется | `gui-spike-features.test.mjs` (+8) | **D-106** |
| 17 | PLAT-01 | Auth: не держать Mutex через сеть/keyring | аудит кода (`PLATFORM-AUDIT`) | **implemented**: `inner`/`store`/`commit` + `generation`; сеть и keyring вне `inner` | `auth_contract.rs` (44) | accepted |
| 18 | SEC-01 | Keyring timeout | аудит кода | **implemented**: `recv_timeout(15 s)`, текст без секретов; `Drop::join` без предела — известный остаток | `auth_contract.rs` (+1) | **D-96** |
| 19 | SEC-02 | keyring-сбой → `kind:"storage"` | контракт ошибок | **implemented**: `Storage` в 5 доменах; `RestFailure` не переиспользует Storage; UI search-overlay различает | `*_contract.rs` + `gui-spike-features` | **D-107** |
| 20 | PLAT-02 | Live-keyring фаза гейта | аудит/план `D-110` | **implemented**: 12 `#[ignore]` + `live_guard` + `keyring-live` только release/qualify + offline 3 | `keyring_contract` (12 ignored) / `keyring_offline_contract` (3) / meta 13 | **D-110** |
| 21 | QA4-01a | `cdp eval/list/clicktext/shot` | — (tooling) | **implemented** | `cdp.test.mjs` (36) + stub-CDP smoke | accepted |
| 22 | QA4-01b | `walk` (fixture) | — (tooling) | **implemented** | `walk.test.mjs` (33); e2e fixture — skip | **D-123** |
| 23 | QA4-02 | Gate → Rust workspace | — (tooling) | **implemented**: фаза `workspace-rust` в commonPhases | `test-policy.test.mjs` | accepted |
| 24 | QA4-03 | Coverage CLI | — (tooling) | **implemented**: `cli-run.test.mjs` 38, coverage 99,74/94,50 | `test-policy`/`coverage` | accepted |
| 25 | PLAT-11-meta | IPC не принимает путь | аудит `PLAT-11` | **implemented**: 35 команд, 0 path-аргументов, `tauri-plugin-fs/dialog` отсутствуют | `ipc-path.test.mjs` (12) + red-proof мутациями | **D-127** |

Итог batch: **P0-дефектов нет**. `G6-D-01/D-02` (канал `"news"`/индекс) закрыты в live-пути
(`chat.js: openChannel`, `item.id`); в бенч-режиме `"news"` и `String(index)` сохранены
осознанно (`BENCH_CHANNEL_ID`) и live-запросов не порождают.

---

## 2. Parity: что реально закрыто, что partial/deferred

### 2.1 Закрыто (офлайн-паритет, live unverified)

| Строка | Было | Стало | Что осталось |
|---|---|---|---|
| G-01 | partial (3 статичные кнопки) | implemented | live READY (P0-01); restore `lastGuildId` — G-01b |
| C-02 | partial (`news`) | implemented | пагинация истории (первая страница `limit=100`); live-история M-01 |
| G-03 | absent | implemented | live-коды/тело ошибки/TTL — `[A]` (RE-2) |
| G-03a | partial (backend) | implemented | live 204 из UI не проверен |
| G-04 | absent | implemented | live-коды (403 owner, 404, 429 retry_after) — `[A]` |
| M-07 | partial (ядро) | implemented | unread без mentions; badge-записи; live ack |
| L-01 | partial (нет UI) | implemented | live resume/close-коды — сторона G1 |

### 2.2 Partial (не выдаётся за паритет)

| Строка | Состояние | Причина |
|---|---|---|
| **P-03** | **partial** | backend есть (`typing.rs`, `[S]`-оракул), UI-индикатора и `send_typing` нет; `TYPING_RESEND_MS`/≤5 каналов не реализованы |
| **G-05** | **partial** | 10 пунктов отрисованы, но 8 — статичные заглушки; invite disabled до G-06/capability |
| **M-06** | **partial (регресс среза)** | в реальном канале реакции не рендерятся (`renderChannelMessage` без pills, кнопки скрыты); `reactions.js:109` остаётся на bench `"news"` |
| **M-07 unread** | partial | только `mentionCount > 0`; unread без упоминаний и badge-счётчики не выводятся (нужна проекция `badgeCount`/`flags`) |
| **DP-02** | measurement | `[M]`-геометрия, не функция; legacy-токены 44/40 держит `geometry-check` |
| **G-01 restore** | deferred | нужен `lastGuildId` в `UiSettings` (изменение контракта настроек) |

### 2.3 Deferred / block (вне паритета, гейты)

Live-паритет: **P0-01 READY, P0-02 send, P0-03 no-secrets-live, A-01, M-02** — окно PM.
RE-гейты: invite/leave live-коды (RE-2), upload (RE-1), Go Live (RE-3), members-search (RE-4),
DND (RE-5), create server (RE-6). Не кодировать: AT-01, S-02/S-03, V-10, N-03, G-02.
`[M]`-строки (V-08 fake, N-03, G-05 заглушки, ST-01 подпункты) **не** считать паритетом.

### 2.4 Открытые P0/P1 (влияют на merge)

**P0 (все — live, не код):** P0-01, P0-02, P0-03.

**P1, связанные с batch:**
1. **P-03 UI** — индикатор «Печатает…» + отправка typing + троттлинг/guard (backend готов).
2. **M-06 реакции в реальном канале** — сейчас нет ни pills, ни быстрых реакций.
3. **M-01/C-02 пагинация** — только первая страница, `hasMore/nextCursor` не догружаются.
4. **L-02 dispatch → UI** — без него L-01 показывает только статус, live-данные в экраны не идут.
5. **G-08/C-03 unread** — статичный/частичный, зависит от M-07-проекции счётчиков.

Не входят в batch, но остаются P1: MB-01/P-01/P-02 (live-данные), V-01…V-08 (voice),
ST-01 (если считать accepted — статус в `TASKS.md` противоречив: `queued` vs
`MERGE-MANIFEST` accepted).

---

## 3. Проверка «нет вымысла» (endpoint/поля/route extensions)

| Расширение | Evidence | Отдельный оракул | Что доказывает тест | Вердикт |
|---|---|---|---|---|
| `POST /channels/{id}/typing` | `[S]` `NETWORK-CONTRACTS §1.6` (anchor `Rsh.TYPING`), тело пустое, 200 + 2 поля; TTL/0.8·E — `[A]` | **да**: `typing_contract.rs`, `D02_TSV` (`/typing` отсутствует) + `ROUTES` не расширен | path/method/body пусто, `find()` резолвит только через `EXTENSIONS`, `render_path` с `{id}`, 201/204 → `InvalidResponse` | **PASS** |
| `GET|POST /invites/{code}` | `[S]` anchor `_.Bo.post({url:ei.Rsh.INVITE…`), 3 nullable поля, `getSessionId():null`; live-коды `[A]` | **да**: `invites_contract.rs`, тот же `D02_TSV` + `ROUTES.len()==49` | accept тело ровно 3 ключа, `context` не в теле, нормализация кода до транспорта, 2xx без объекта → ошибка | **PASS** |
| `channels(guildId?)` (IPC, не REST) | `[L]` READY.guilds[].channels + private_channels | **да**: `channels_contract.rs` сверяет поле-в-поле с READY (272 + 17, `{0:137,2:82,4:43,5:1,15:9}`), чужой канал из `MESSAGE_*` отсутствует | не endpoint; проекция только из READY, `nil` → `null`, ошибки не маскируются пустым списком | **PASS** |
| `read_state()` (IPC, не REST) | `[L]` READY.read_state.entries; `last_viewed` — счётчик | **да**: `read_state_contract.rs` + `data_contract` (порядок/поля из READY) | ровно 6 полей, `badge_count/last_acked_id/read_state_type` **не** отдаются, `null` ≠ 0 | **PASS** |
| `ack_message` (REST уже в D-02) | `[L]` тело `{token,last_viewed}`, 200×5 | **да**: `rest_ack.rs` JSON-равенство `{"token":null,"last_viewed":4310}` | `token` — не bearer, id до сети, 429 без выдуманного `retry_after` | **PASS** |
| `leave_guild` (REST в D-02) | `[S]` `DELETE` + `{lurking: bool}` | **да**: `guilds_contract.rs` | ровно одно поле `lurking`, `POST /guilds/{id}/delete` не вызывается | **PASS** |

Frozen-оракулы целы: `rust/crates/discord-core/tests/data/rest-routes-47.tsv` — 47 строк,
md5 `2db6b39bcba96bc3a2d480fea9461a33`, `/typing` и `/invites` = 0 вхождений;
`routes::ROUTES` = 49 (D-02 + POST send), расширения — `EXTENSIONS: [RouteSpec; 3]`.
Придуманных query/полей (`inputValue`, `with_games`, `badge_count`, attachment-схема)
в коде нет.

**Итог §3: PASS.** Ни один `[A]`-элемент не выдан за паритет: TTL typing, коды invite/leave,
нормализация кода помечены как наша политика/неподтверждённое.

---

## 4. Тесты: целостность и skip

| Проверка | Факт | Источник |
|---|---|---|
| red-first | red-логи есть у 22/25 срезов; у G-03a-UI, PLAT-01, PLAT-02 — только inline-вывод в отчёте (`PM/*/red*.log` отсутствует) | листинг `waves/RW-40/PM/*` |
| `.only` | 0 во всём `tests/**` | grep |
| unit-`skip` | только platform-guard в bench (`platform !== "linux"`), не ослабление | grep |
| e2e | **4 skip**: 2× «нет браузера: CHROME_BIN» (`cdp-browser`, `walk-fixture`) + 2 live (`CDP_ENDPOINT`) — **не pass** | `.gate/release-…09-19-36`, `tests/e2e/*` |
| workspace | 1115 тестов: **1103 pass / 0 fail / 12 skipped**; 12 = live-keyring (`#[ignore]`), отдельно `keyring-live` 12/12 | gate JSON, PLAT-02 |
| gate | `passed_with_skips`, exit 0: meta 65, unit 614, smoke 22, coverage 614, e2e 4 skip | `.gate/release-2026-10-24T09-19-36-946Z.json` |
| бенч-порог | `CHAT_ROWS=100_000`, `SNAPSHOT_FLOORS chatRows=100_000` не тронуты; тест C-02 проверяет `snapshot.chatRows` | `chat.js`, `channel-switch.test.mjs` |
| оракулы | D-02 TSV md5 стабилен; geometry 31/31; READY-фикстура не менялась | md5, `geometry-check` |

**Найденные отклонения (P2, не блок):**
1. **md5 drift.** Текущие `tests/unit/{guild-rail,channel-switch,read-state-ui,gui-spike-features,
   gui-spike-model}.test.mjs` и `join-server.test.mjs` не совпадают с md5 в отчётах
   (совпадает только `join-server`). Счётчики кейсов новых файлов совпадают (8/10/24/8),
   `.only`/`.skip` не добавлены, гейт зелёный. Причина по свидетельствам других отчётов —
   поздние аддитивные правки/типы (PLAT-02/PLAT-11 фиксировали tsc-ошибки в
   `channel-switch`/`read-state-ui`), но **отдельного red/green-лога на дрейф нет**;
   baseline `rust/gui-spike/**` вне git → диффом не проверить.
2. **Правка формулировок теста в G-03-UI** (429: наличие `disabled`, а не отсутствие) —
   задокументирована как ошибка оракула, не ослабление. Требует явной подписи PM.
3. **M-07-UI** менял тест «медленный сервер» для наблюдения полосы до ответа — контракт
   не менялся; задокументировано.
4. **DP-02 red** получен обратным патчем продукта (файлы восстановлены) — воспроизводимо,
   но это не «red до первой реализации».

---

## 5. Merge verdict

### Вердикт: **`deferred`**

- **`block` нет:** P0-дефектов не найдено; вымысла нет (§3 PASS); гейт зелёный
  (`passed_with_skips`); новых секретов/логов не добавлено (ack `token` — nullable,
  bearer только backend; `Debug` не печатает токен).
- **`parity-ok` пока нельзя:** P-03 без UI, реакции в реальном канале отсутствуют,
  матрица и BUGS не синхронизированы, журнал RW-40 пуст.
- Merge разрешён после пунктов 1–5; п.6–7 — сразу после merge, до объявления accepted.

### Обязательно до merge (5)

1. **Integration берёт весь accepted set**, а не только D-128: обязательны DP-02, L-01,
   SEC-01/02, Q-TOKEN/G1-01/Q-G2-P1/Q-G4-REG, V-08 UI, ST-01, PLAT-11 meta, QA-IMPACT
   (`MERGE-MANIFEST §5`). Иначе теряются файлы и гейт не воспроизводится.
2. **Письменное решение PM по P-03:** `P-03-BE` принят, строка матрицы остаётся
   `partial` (UI нет). Либо waiver на «backend-only», либо карточка P-03-UI **до**
   заявления паритета P-03. По `MERGE-POLICY §4` перенос P1 в deferred — только письменно.
3. **Закрыть или письменно отложить M-06 в реальном канале** (реакции): сейчас в
   `renderChannelMessage` pills нет, быстрые реакции скрыты, `reactions.js` живёт на
   bench-id. Это функциональный регресс при переходе на реальные каналы.
4. **Синхронизировать G6 перед accept:** применить §8 `G6-REPORT-RW40-02` (G-01 `[L]`,
   ST-01 gap/WSJF, C-02/M-07/G-03 формулировки), завести `G6-D-02…D-05` в
   `QUALITY/BUGS.md` (сейчас есть только `G6-D-01 closed D-120`), обновить строки
   G-01/C-02/G-03/G-03a/G-04/G-05/M-07/P-03/L-01 в `PARITY-MATRIX.md`.
5. **Зафиксировать e2e как skip, а не pass:** merge-вердикт `passed_with_skips`; в
   integration прогон с `CHROME_BIN`/`CDP_ENDPOINT` (4 e2e) обязателен до `gate:qualify`.
   Одновременно сохранить frozen-оракулы (D-02 TSV md5, `ROUTES=49`, geometry 31/31).

### Можно после merge (5)

1. **P-03 UI**: `send_typing` + индикатор `aria-live="polite"`, `TYPING_RESEND_MS`
   (именованная `[A]`-конфигурация), guard ≤5 каналов, slowmode-сообщение.
2. **M-06**: реакции в реальных строках (pills + быстрые реакции на `item.id`), убрать
   bench-only путь `reactions.js`.
3. **M-01/C-02 paging**: `hasMore/nextCursor`, открытие на последнем сообщении,
   сохранение позиции на длинных каналах.
4. **L-02 dispatch → UI**, затем live-окна: P0-01 READY, P0-02 send, P0-03 live-логи;
   RE-2 (live-коды invite/leave, `USER_BANNED`, TTL), G-03a live 204.
5. **G6-D-04/D-05 + G-01b restore + G-08/C-03 unread-счётчики + ST-01** (если accepted —
   подтвердить статус: `TASKS.md` говорит `queued`); синхронизировать `TASKS.md`/`STATE.md`
   (там G-04 и G-05/G-04 UI до сих пор «in work», DP-02 — «queued»).

### Процесс (P2, закрыть до объявления batch accepted)

- `waves/db.jsonl` — **0 событий RW-40**; `waves/STATE.md` обновлён 2026-10-23. Нужны
  события `red/gate/accepted` по batch и обновление STATE — делает Team Lead, не G6.
- Red-логи G-03a-UI/PLAT-01/PLAT-02 — только inline; положить файлы в `waves/RW-40/PM/*`.

---

## 6. Handoff для QA

### 6.1 Офлайн (можно запускать сейчас, read-only для кода)

| Сценарий | Команда/файл | Ожидание |
|---|---|---|
| Рейл из READY | `node --test tests/unit/guild-rail.test.mjs` | 8/8; 7 гильдий, `data-guild-id`, empty/error+retry, клавиатура |
| Каналы/история | `node --test tests/unit/channel-switch.test.mjs` | 10/10; 55 каналов первой гильдии, типы 0/5 открывают, 2/4/15 — нет; send/edit/delete/reply без `"news"`/индекса |
| Join/enter | `node --test tests/unit/join-server.test.mjs` | 24/24; accept `{code}`, enter `{guildId}`, 401/403/404/429, нет подстановки nullable |
| Ack/read-state | `node --test tests/unit/read-state-ui.test.mjs` | 8/8; один ack на курсор, `token:null`, badge → 0 ack, 401/429 видны |
| Backend-контракты | `cargo test --offline --manifest-path rust/gui-spike/Cargo.toml --test {channels,invites,typing,read_state,guilds,rest_ack}` | все зелёные; D-02 TSV не расширен |
| Ядро/каталог | `cargo test --offline -p discord-core --test rest_routes` | `ROUTES=49`, `/typing`/`/invites` резолвятся только через EXTENSIONS |
| Гейт | `npm run gate:worker` | meta/unit/smoke/spike/workspace зелёные; e2e 4 skip = warning |

### 6.2 Live (только окно PM, тестовый аккаунт)

1. **N0-01 READY:** реальные гильдии/каналы в рейле и дереве (не фикстура); счётчики
   совпадают с READY; токен/секреты в логах = 0.
2. **N0-02 send:** отправить в реальный канал → сообщение видно; edit/delete/reply по
   реальному snowflake; 401/429 показываются один раз, без «тихого» ретрая.
3. **M-07 ack:** открытие канала → ровно один `POST …/ack` 200, `last_viewed` из живого
   READY; `token` в теле/логах не bearer; unread-полоса сбрасывается.
4. **G-03/G-03a:** resolve валидного/просроченного/забаненного инвайта; accept 2xx → плитка;
   `PUT …?lurker=true` → 204 и «Сервер открыт для просмотра» (плитки нет).
5. **G-04:** выход участника → 2xx и плитка исчезла; выход владельца → 403 видим.
6. **G-05:** меню по реальным правам (disabled-пункты не активны), leave-confirm.
7. **L-01:** разрыв сети → «Нет соединения» + «Повторить»; reconnect → баннер скрыт;
   resume/4013/4014 не ломают UI.
8. **Keyring live:** `gate:release` с живой шиной → `keyring-live 12/12`, ignored=0.
9. **P-03 (после UI):** первое нажатие → один POST, повтор не чаще интервала, TTL.

### 6.3 Skip / не считать pass

- `tests/e2e/cdp-browser.e2e.test.mjs` — skip без `CHROME_BIN`.
- `tests/e2e/walk-fixture.e2e.test.mjs` — skip без `CHROME_BIN`; DOM/`__clickLog` не проверены.
- `tests/e2e/live-discord.e2e.test.mjs` (2) — skip без `CDP_ENDPOINT` (live-окно).
- 12 `keyring_contract` — `#[ignore]`, но в release покрыты фазой `keyring-live`.
- **Не проверять и не заявлять:** voice (fake), Go Live, attachments, inbox, typing-индикатор
  (его нет), unread без упоминаний, геометрию dialogs (`unmeasured`).

---

## 7. Приложение: текущий merge-baseline (md5)

| Файл | md5 |
|---|---|
| `rust/crates/discord-core/src/rest/routes.rs` | `923ccb1f65dd5daeb4d7bfad96908b80` |
| `rust/crates/discord-core/tests/data/rest-routes-47.tsv` (frozen D-02) | `2db6b39bcba96bc3a2d480fea9461a33` |
| `rust/gui-spike/src/invites.rs` | `4ce58a583a2384d5f6b0b952e24f054b` |
| `rust/gui-spike/src/typing.rs` | `e62d22fedf30877efc9e96ac791879c2` |
| `rust/gui-spike/src/messages.rs` | `8da412cf1a629ac03636d61c5e58be3d` |
| `rust/gui-spike/src/data.rs` | `6de4f5726fbf2589c2cb7218f8097208` |
| `rust/gui-spike/src/guilds.rs` | `45cc55f727863bdf8780b044d3bc881f` |
| `rust/gui-spike/src/main.rs` | `6e5b0adb6113383b2b010d1be823ff82` |
| `rust/gui-spike/tests/typing_contract.rs` | `4ab8e1b5494f730b0026361dd3047193` |
| `rust/gui-spike/tests/invites_contract.rs` | `cbfef8f6be091893081f0b1ffc20c125` |
| `rust/gui-spike/tests/read_state_contract.rs` | `8718a639750412301d4eaff170fec077` |
| `rust/gui-spike/tests/channels_contract.rs` | `58d375894162069e34aa3c1680d33e65` |
| `rust/gui-spike/tests/rest_ack.rs` | `fe95d3c0bb66047226898ae648c6c16b` |
| `rust/gui-spike/ui/app.js` | `4591bdf4c2ea793ecbb35221ddcaf2ef` |
| `rust/gui-spike/ui/index.html` | `417bd6687f54a2063232390e579ffe39` |
| `rust/gui-spike/ui/features/chat.js` | `fc56080139437146857bbf551b61db80` |
| `rust/gui-spike/ui/features/join-server.js` | `940612793f1f82ba8625da4dfd33d438` |
| `rust/gui-spike/ui/features/actions.js` | `908bfe2dbf4202cdf28c389d44b2095f` |
| `rust/gui-spike/ui/model.js` | `642e1d8f9a35ca0bc6e0dced394ebd77` |
| `rust/gui-spike/ui/tokens.css` | `21d570a6bf9c4b5d77b00d3d9b3d1d08` |
| `CONTRACTS/read-state.md` | `fb7ed47afc3494e667ad21d873a04ed7` |

Гигиена (P3): `ui/index.html:119` содержит статический `data-row-height="44"`; `chat.js`
перезаписывает его из `ROW_HEIGHTS` при инициализации, но плейсхолдер противоречит D-04.

---

## 8. Проверки, которые НЕ выполнялись

Окно/live, сеть, CDP/AT-SPI, `cargo`/`npm`/`node --test`, мутации, commit. Все числа
тестов взяты из `.gate/release-2026-10-24T09-19-36-946Z.json` и отчётов карточек;
повторный прогон — на integration/QA. Геометрия join/leave/voice-модалок остаётся
`unmeasured` до первого живого окна.
