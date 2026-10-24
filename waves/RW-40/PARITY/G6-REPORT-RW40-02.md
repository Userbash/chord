# G6-REPORT-RW40-02 — decision-ready отчёт для пула задач

Дата: 2026-10-24. Автор: G6 Lead (RE Parity), subagent PM DeepSeek.
Режим: **read-only**. Не запускались: Discord, окно, CDP/AT-SPI, сеть, `cargo`, `npm`, commit.
Источники — только `app.asar` 1.0.166, dump `/home/sanya/re-deep/**`, `docs/re/**`,
`waves/RW-40/**` и текущий код `rust/gui-spike/**` (чтение).
Код и чужие документы не изменялись; выход — этот отчёт + `REA-SESSION.md` §11.

Связанные: `PIPELINE.md`, `G6-CHARTER.md`, `PARITY-MATRIX.md`, `GAP-BACKLOG.md`,
`STUB-MAP.md`, `QUEUE.md`, `PARALLEL-LANES.md`, `NETWORK-CONTRACTS.md`,
`DESIGN/SERVER-MENU.md`, `DESIGN/VOICE.md`.

---

## 0. Резюме для PM (что решать сейчас)

1. **Главная структурная находка:** BFF-слой уже готов, но UI к нему не подключён.
   `ui/**` вызывает только `send_message`, `edit_message`, `delete_message`, `reply_message`,
   `toggle_reaction`, `search_remote`, `settings_load/save`, `leave_guild`, `auth_*`,
   `process_metrics`, `report_ui_snapshot`, `spike_config`.
   **Ни разу** не вызываются: `ready`, `messages_page`, `members_page`, `settings`,
   `search_page`, `join_guild`, `net_connect/net_status`.
   Следствие: 5 из 12 функций — это «подключить», а не «написать backend».
2. **Дефект, блокирующий live-send (P0-02) и C-02:** в UI канал зашит строкой
   `"news"` (`ui/features/chat.js:432,495,509`, `ui/app.js:160`), а `messageId` для
   edit/delete — синтетический индекс строки (`String(index)`). Живой REST ждёт
   snowflake канала и сообщения → `404`/`400`. Это не «live unverified», это
   гарантированный отказ. Отдельная карточка — §7.1 (G6-D-01).
3. **ST-01 переоценён в матрице.** «63 строки» — это DOM-строки, а не 63 раздела:
   по артефакту `ui8/23-settings-categories.json` (63 записи, `count: 63`) это
   профиль (2) + 19 записей навигации × 3 обёртки (57) + 4 подпункта «Учётной записи» (4).
   Реальный gap: нет профиля, нет 4 подпунктов, нет «Выйти». Это задача размера S,
   а не «45 подпунктов/панелей» (§8, правка матрицы).
4. **G-03a UI нельзя дать отдельно от G-03.** Backend (`PUT …?lurker=true` → 204) принят,
   но у экрана нет источника `guildId` для preview: его даёт только `GET /invites/{code}`
   (G-03) или deep link (PL-03, отсутствует). Порядок: G-03 resolve → G-03a UI.
5. **G-03 (invite) кодируем**: `[S]` путь + тело, причём все три поля могут быть `null`
   (`session_id` = `getSessionId()`, в бандле `getSessionId():null`;
   `invite_instance_id` — клиентский `_U(baseCode, message_id)`, опционален;
   `received_installation_id` — `null` допустим). Live-коды/TTL — по-прежнему `[A]`.
6. **M-07 контракт закрыт полностью `[L]`:** тело ack — `{"token":null,"last_viewed":4310}`
   (200×5). `last_viewed` — **счётчик read_state, не id сообщения**. `token` нулевой,
   и его нельзя путать с bearer-токеном.
7. **Нельзя кодировать:** AT-01 (upload), S-02/Go Live, voice media/DAVE, N-03 (inbox),
   G-02 (create server), точные коды invite/leave. Причины — §6.

---

## 1. Как читать и границы

| Поле | Значение |
|---|---|
| level | `[L]` live-захват · `[S]` call-site/тело в коде эталона · `[D]` только Enum/путь · `[M]` измерение (DOM/скрин) · `[A]` не подтверждено/наше решение |
| version-gap | renderer UI — **1.0.162**; desktop/IPC — **1.0.166**; REST/gateway/voice live — **1.0.162**. Renderer 1.0.166 не снимался, поэтому «как в текущем Discord» не утверждается ни для одной UI-строки |
| app | `implemented` · `partial` · `absent` · `unknown`; `implemented ≠ live` (см. матрицу §15) |
| ready | `ready-to-code` (есть `[L]`/`[S]` + офлайн-оракул) · `partial-ready` (часть контракта) · `RE-gate` / `live-gate` |

Ограничения этого отчёта: не проверялись живьём коды ответов invite/leave, не снимался
renderer 1.0.166, не измерялась геометрия join/leave-модалок (`SERVER-MENU.md §0` —
`unmeasured`). Всё, что помечено `[A]`, в код без RE/live не идёт (устав §4.1).

---

## 2. Сводная таблица 12 функций

| # | ID | Функция | Evidence | App | P | WSJF | Owner | Ready |
|---:|---|---|---|---|---|---:|---|---|
| 1 | G-01 | Рейл серверов из READY | `[L]` READY.guilds · `[M]` UI6:01 | partial | P1 | 5.5 | G2+G1 | **ready-to-code** |
| 2 | G-03a UI | Вход в сервер из preview (lurker) | `[L]` 204×2 | partial (backend) / absent (UI) | P1 | 4.2 | G2+G1 | partial-ready — **после G-03** |
| 3 | G-03 | Join по инвайту | `[S]` путь+тело | absent | P1 | 4.0 | G1+G2 | **partial-ready** (happy path) |
| 4 | C-02 | Переключение канала + история | `[L]` GET messages; UI `[M]` | partial | P1 | 5.0 | G2+G1 | **ready-to-code** (офлайн) |
| 5 | M-07 | Read state / ack | `[L]` POST ack 200×5 | partial | P1 | 4.5 | G1+G2 | **ready-to-code** |
| 6 | P-03 | Typing: POST + индикатор | `[S]` POST; `[L]` TYPING_START | partial | P1 | 4.5 | G1+G2 | **ready-to-code** (TTL — параметр) |
| 7 | V-08 | Голосовой UI (панель/участники/ping) | `[M]` VUI:01 · `[D]` 12 состояний · design accepted | absent | P1 | 3.67 | G2+G3 | **ready на fake** |
| 8 | AT-01 | Загрузка вложения | `[D]` только путь | absent | P2 | 3.0 | G1+G2 | **RE-gate** ❌ |
| 9 | N-01 | Системные уведомления | `[S]` IPC 1.0.166 + D-Bus crate | partial (crate) | P2 | 3.0 | G5+G2 | partial-ready (policy+adapter) |
| 10 | N-03 | Inbox/упоминания | `[M]` только строки | absent | P2 | 3.0 | G2+G1 | **RE-gate** ❌ |
| 11 | ST-01 | Навигация настроек | `[M]` F162:63-70 + oracle `ui8/23-settings-categories.json` | partial | P2 | 4.0 → ~6.0 (пересчёт) | G2+G3 | **ready-to-code** (структура) |
| 12 | S-02 | Screen share / Go Live | `[D]` capture IPC; `[S]` STREAM_* | absent | P2 | 1.6 | G5+G1 | **RE+live-gate** ❌ |

---

## 3. Карточки

### 3.1 G-01 — Рейл серверов: список/выбор/переключение

**Что делает для пользователя.** Показывает все серверы, в которых он состоит, позволяет
переключаться; выбранный сервер задаёт дерево каналов. Без этого клиент выглядит как один
демо-сервер.

**Источник / evidence.**
- Источник списка — `READY.guilds[]` + `READY_SUPPLEMENTAL`: `[L]`, `gateway-protocol.md:150,196-208`,
  фикстура `fixtures/gateway/by-type/002-READY.json` (7 гильдий; в первой — 55 каналов; 17 ЛС).
  Отдельной REST-ручки списка в бандле **нет** (`[S]`: `GET /users/@me/guilds` отсутствует;
  `NETWORK-CONTRACTS §1.3`).
- Визуал рейла — `[M]`: `UI6:01`, `UI:00-app.png` (`FUNCTION-INVENTORY G-01`).
- version-gap: READY live снят на 1.0.162; renderer 1.0.166 не снимался.

**Текущее состояние: partial.** Backend готов:
`main.rs:87 ready()` → `data.rs:412 ReadyPayload { user, guilds, guildCount,
privateChannelCount, relationshipCount, readStateCount, session }`,
`data.rs:391 GuildSummary { id, name, icon, memberCount, channelCount }` — парсится из READY-фикстуры.
UI отсутствует: `ui/index.html:35-40` — три статичные кнопки `data-guild="1|2|3"`,
первая `is-selected`; `invoke("ready")` в `ui/**` нет.

**Gap.** Нет привязки данных, выбора, состояний (loading/ready/empty/error/offline), клавиатуры,
восстановления выбранного сервера; нет связи «сервер → дерево каналов».

**Контракт (точный).**
- IPC существующий: `ready()` → `{user, guilds[], guildCount, privateChannelCount,
  relationshipCount, readStateCount, session}`; ошибки `DataError {kind: Io|Parse|NotFound|InvalidArg}`.
- UI: `#guild-rail [role="navigation"]`, элемент `button.guild-item[data-guild-id]`,
  `aria-label="Сервер <name>"`, `aria-current="true"` на выбранном; пустой список →
  empty-state; ошибка → `.error-state` (`features/errors.js`).
- **Никакого нового endpoint не нужно.** Иконки: `icon` — хеш CDN, URL **не склеивать**
  (`SERVER-MENU.md §2.1`), пока — монограмма 2 буквы.
- Восстановление выбора в первом срезе **вне scope** (G-01b): `UiSettings` — типизированная
  структура без `lastGuildId` (`settings_store.rs:137`), добавление поля = изменение
  контракта настроек, отдельное решение PM.

**DoD / тест.** 7 гильдий из фикстуры в рейле; клик меняет выбранный (и с этого момента —
заголовок дерева/каналы при готовом C-01); `↑/↓/Home/End`; 0 гильдий → empty; битая
фикстура → error с retry; имена не выдуманы; `data-guild-id` = snowflake.
Тест: **новый** `tests/unit/guild-rail.test.mjs` (DOM-шим, `loadApp`) + существующий
`tests/data_contract.rs` (ready: 7 гильдий, поля). Live — окно N0.
**Owner** G2 (+G1 при изменении payload). **P1**, WSJF 5.5. **Зависимости:** нет (офлайн).
**Ready: yes.**

---

### 3.2 G-03a UI — Вход в сервер из preview/lurker

**Что делает.** Кнопка «Открыть сервер» на карточке приглашения: даёт доступ к просмотру
сервера до вступления. Не добавляет в гильдию.

**Источник / evidence.** `PUT /guilds/{id}/members/@me?lurker=true`, тело `{}` → **204 ×2**
(`[L]`, `D02:101,138,225`, requestId `ready|57992.6989`, `run2|95974.7409`).
Прочие коды — `[A]`. version-gap: REST live 1.0.162.

**Текущее состояние: backend — implemented, UI — absent.**
`src/guilds.rs:378 enter_guild` (алиас `join_guild:387`), типизированные ошибки
(`InvalidArg|MissingToken|Unauthorized|Forbidden|NotFound|RateLimited{retryAfterMs}|Network|Parse`),
ровно один Tauri-command `join_guild` (`main.rs:261`, alias `enter_guild` удалён по Q-01),
24 контрактных теста (`tests/guilds_contract.rs`, 696 строк).
В UI нет ни `enter-guild`, ни `join_guild` (grep = 0).

**Gap.** Нет точки входа: `guildId` для preview берётся только из `GET /invites/{code}`
(G-03) либо из deep link (PL-03 — absent). Отдельно «кнопка без карточки» была бы фикцией.

**Контракт.** `invoke("join_guild", { guildId })` → `void`; успех **только 204**
(иной 2xx → `Parse` — так в коде и это правильно); ошибки — `{kind:"forbidden"}` и т.п.
UI-состояния: `idle → entering → entered | error`, `aria-busy` на время, повтор только
упавшего шага. Плитка в рейле **не** добавляется (lurker ≠ членство).

**DoD / тест.** Preview-карточка: «Открыть сервер» → 204 → статус «Сервер открыт для
просмотра»; 403 → «Нет доступа», без повтора; 429 → `retryAfterMs`; повторный клик игнорируется.
Тест: `tests/unit/server-menu.test.mjs` (новый, по `SERVER-MENU.md §7.3/7.4`) + backend-тесты
уже есть. **Owner** G2+G1. **P1**, WSJF 4.2. **Зависимость: G-03 resolve (preview).**
**Ready: partial — после G-03, не раньше.**

---

### 3.3 G-03 — Вход по инвайту

**Что делает.** Пользователь вставляет `discord.gg/…` или код → видит превью → вступает
в сервер.

**Источник / evidence.**
- Accept: `POST /invites/{code}`, тело `{session_id, invite_instance_id, received_installation_id}`
  — `[S]`, anchor `_.Bo.post({url:ei.Rsh.INVITE(A)`; ответ — invite-объект
  (`guild_id`, `target_channel_id`, `target_message_id`, `guild_scheduled_event`)
  (`NETWORK-CONTRACTS §1.2/§1.4`).
- `context` — analytics-объект запроса, **не** поле тела (не переносить в body).
- `session_id` = `getSessionId()`; в бандле `getSessionId():null` → значение **nullable** (`[S]`).
- `invite_instance_id` — клиентский `_U(baseCode, message_id)`, для вставленного кода
  может отсутствовать → поле **опционально** (`[S]`).
- `received_installation_id` — `null` допустим (`[S]`, чистится после успеха).
- Resolve: `GET /invites/{code}` c `with_counts/with_expiration/…` — `[S]`.
- Live: **не наблюдался** ни разу (`D02`: 0 invite-запросов) → коды 200/204/400/403/404/429,
  тело ошибки, TTL `received_installation_id` — `[A]`.

**Текущее состояние: absent.** Backend: в `src/guilds.rs` только enter/leave; маршрута
`/invites/{code}` в каталоге `routes.rs` нет (каталог = 47 live-маршрутов D-02).
UI: пункт «Пригласить на сервер» честно disabled через `data-capability-invite="false"`
(`actions.js:76-77,374`); диалога join нет.

**Gap.** resolve, accept, нормализация кода, состояния и ошибки, обновление рейла.

**Контракт.**
- Нормализация: trim, снять хвостовой `/`, взять последний сегмент из `…/invite/<code>`,
  допускать `[A-Za-z0-9-]` длиной 2…32 (границы — `[A]`, наша валидация, не утверждение о Discord).
- `resolve_invite(code)` → `{code, guildId, guildName, icon?, memberCount?, presenceCount?, expiresAt?, channelId?}`;
  `accept_invite(code)` → `{guildId, targetChannelId?, targetMessageId?}`.
- Успех accept — только 2xx; `USER_BANNED` → отдельный текст; non-2xx → типизированная ошибка
  через `rest_failure.rs` (`Unauthorized|Forbidden|NotFound|RateLimited|ClientOther|Server|Network`).
- Обновление списка: `guild_id` из ответа + `READY`/локальный список; **не строить** путь
  только на `GUILD_CREATE` (в 7 живых сессиях не наблюдался). Нет данных — плитка `pending`.
- UI `#join-server-dialog`: `idle|resolving|preview_ready|accepting|accepted|error`,
  `#join-error[role="alert"]`, фокус в поле при открытии, повтор только упавшего шага.

**DoD / тест.** Вставка кода → skeleton → превью; accept 2xx → pending-плитка → обновление
из READY; 403/404/429 видны и не «зеленятся»; фальшивая ссылка не показывается.
Тест: расширение `tests/guilds_contract.rs` (fake 200/204/403/404/429, тело из 2–3 nullable полей)
+ `tests/unit/join-server.test.mjs`. **Owner** G1+G2. **P1**, WSJF 4.0.
**Зависимости:** политика расширения route-каталога (см. §7.3), окно RE-2 для кодов/TTL.
**Ready: partial-ready** — happy path да, таксономия ошибок нет.

---

### 3.4 C-02 — Переключение канала + история

**Что делает.** Клик по текстовому каналу открывает его сообщения; возврат к каналу
восстанавливает окно/позицию.

**Источник / evidence.** `GET /channels/{id}/messages` — `[L]`, 200×11
(`rest-d02.md:70`, таблица §4 #1); переключение в UI — `[M]` (`FUNCTION-INVENTORY C-02`,
ссылки `/channels/<g>/<c>` в `UI6:01`). version-gap: REST/UI 1.0.162.

**Текущее состояние: partial.**
- Backend: `main.rs:94 messages_page(channelId, cursor, limit)` → `data.rs:1006`
  (`MessagesPage`), фильтрация по каналу, `NotFound` на неизвестный канал.
- UI: чат синтетический — `chat.js:20 CHAT_ROWS = 100_000`; дерево статично
  (`index.html:50-73`); отправка/правка/удаление/ответ идут с `channelId: "news"`
  (`chat.js:432,495,509`, `app.js:160`), `messageId = String(index)`.

**Gap.** Нет выбора канала, нет реальных id, нет загрузки истории через IPC, нет
per-channel окна и позиции, нет типизации по типу канала.

**Контракт.**
- IPC `messages_page` не меняется: `{channelId, cursor?, limit?}` →
  `{channelId, items[], nextCursor, hasMore, total, limit, loadedRowsLimit}`;
  ошибки `DataError`.
- UI: `#channel-tree .channel-item[data-channel-id]`, `aria-current="page"`; заголовок
  `#chat-header .chat-title-text` = имя канала; список `#chat-list` перерисовывается из
  `messages_page`, не из синтетики.
- **Типы (из фикстуры READY, 272 канала):** `0` текстовый — 137, `2` голосовой — 82,
  `4` категория — 43, `15` форум — 9, `5` объявление — 1. Открывать чат: `0` и `5`;
  `2` → voice (V-08/blocked live), `4` → collapse, `15` → forum-вид (P3).
- `NotFound|InvalidArg` → видимая ошибка, **не** пустой список.
- Бенч-путь сохраняется: `CHAT_ROWS=100_000` и `UI_SNAPSHOT`-порог
  (`lib.rs SNAPSHOT_FLOORS chatRows=100_000`) не ослаблять; синтетика остаётся за
  режимом бенча/`SPIKE_BENCH`, реальные данные — отдельный путь.

**DoD / тест.** Клик по текстовому каналу → заголовок + строки из `messages_page`;
возврат → своё окно; голосовой/категория не открывают чат; неизвестный канал → ошибка;
бенч-порог зелёный. Тест: **новый** `tests/unit/channel-switch.test.mjs`; Rust —
`tests/data_contract.rs` (per-channel + NotFound уже есть) и `messages_contract.rs`.
**Owner** G2+G1. **P1**, WSJF 5.0. **Зависимости:** G-01 (реальный список), M-01 (live-история).
**Ready: yes (офлайн), live — после N0.**

---

### 3.5 M-07 — Read state / ack

**Что делает.** Помечает канал прочитанным, сбрасывает unread/mention при просмотре.

**Источник / evidence.** `[L]`: `POST /channels/{id}/messages/{id}/ack`, 200×5,
тело ровно `{token, last_viewed}`; пример `{"token":null,"last_viewed":4310}`
(`docs/re/rest-d02.md §4`, requestId `ready|57992.7577`). `MESSAGE_ACK` live-кадр
(`fixtures/gateway/by-type/046-MESSAGE_ACK.json`) несёт `last_viewed: 4310` → **`last_viewed`
— счётчик read_state, не id сообщения**. `READY.read_state.entries[]` = `{id, flags,
last_message_id, last_pin_timestamp, last_viewed, mention_count}` (`[L]`, 3 записи в фикстуре).

**Текущее состояние: partial.** Core: `read_state_reducer::apply_ack`, разбор `MESSAGE_ACK`;
маршрут есть в каталоге (`routes.rs:74`, `body_keys = ["token","last_viewed"]`).
Нет: REST-метода, Tauri-команды, UI-триггера. Unread-полоса статична (`chat.js UNREAD_COUNT=20`).

**Gap.** Отправка ack, курсор `last_viewed`, сброс unread/`mention_count`, троттлинг.

**Контракт.**
- IPC `ack_message { channelId, messageId, lastViewed, token? }` → `void`;
  тело запроса **дословно** `{token, last_viewed}`; `token` — nullable, при неизвестном
  значении отправлять `null`; **никогда** не подставлять bearer-токен и не логировать.
- `last_viewed` брать из `READY.read_state`/`MESSAGE_ACK` для канала (число, не snowflake).
- Ошибки: `Unauthorized|Forbidden|NotFound|RateLimited|ClientOther|Server|Network`.
- Политика триггера (когда именно ack’ить: открытие, скролл в низ, фокус окна) — `[A]`,
  зафиксировать в `CONTRACTS/read-state.md`; без live не выдавать за паритет.

**DoD / тест.** Открытие канала → один ack с корректным `last_viewed`; unread-полоса
исчезает; повторный вызов не дублируется (троттлинг); 401/429 видны; `token` в логах = 0.
Тест: **новый** `tests/rest_ack.rs` (fake transport, тело-оракул из `rest-d02`) +
`tests/unit/read-state-ui.test.mjs`. **Owner** G1+G2. **P1**, WSJF 4.5.
**Зависимости:** M-01 (реальная история) — для офлайна фикстуры достаточно.
**Ready: yes.**

---

### 3.6 P-03 — Индикатор набора + POST /typing

**Что делает.** Показывает «Печатает…» и сообщает серверу, что пользователь печатает
(в т.ч. slowmode-кулдаун).

**Источник / evidence.**
- `TYPING_START` — `[L]`, 28 событий (`gateway-protocol.md`); core парсит и держит TTL.
- `POST /channels/{id}/typing` — `[S]`, anchor `Rsh.TYPING(t)`; тело отсутствует; `200` →
  `{message_send_cooldown_ms?, thread_create_cooldown_ms?}` → `SLOWMODE_SET_COOLDOWN`.
- Клиентский троттлинг: отправка при `now > prev + 0.8*E`, `TYPING_STOP` через `E` мс,
  локальный guard ≤5 каналов — `[S]`; **константа `E` не восстановлена** → `[A]`
  (`NETWORK-CONTRACTS §1.6`).

**Текущее состояние: partial.** Core: `state/typing/*` (parse/reducer), `TYPING_TTL_MS = 10_000`
— это **наше** значение `[A]`. Маршрута `/typing` в каталоге `routes.rs` **нет**
(каталог = 47 маршрутов D-02). POST и UI-индикатора нет; `composer`-состояние `typing`
— про ввод текста, не про сеть.

**Gap.** REST-метод, IPC, троттлинг, индикатор, slowmode-сообщение.

**Контракт.**
- Маршрут `POST /channels/{id}/typing`, без тела, успех 200; поля ответа только два
  (см. выше), оба опциональны.
- IPC `send_typing { channelId }` → `void | {messageSendCooldownMs?, threadCreateCooldownMs?}`.
- Интервал повтора — **именованная конфигурация** (`TYPING_RESEND_MS`), дефолт = core TTL,
  помечен `[A]`; тест проверяет **правило** (`now > prev + interval`), а не «число Discord».
  Guard: не более 5 каналов в состоянии typing.
- UI: строка над композером, `aria-live="polite"`, «Печатает…» / «Несколько человек печатают…»,
  истечение по TTL, ничего не показывать для себя.

**DoD / тест.** Первое нажатие → один POST; повтор не чаще интервала; TYPING_START →
индикатор появился/исчез по TTL; slowmode → сообщение; 5-канальный guard.
Тест: **новый** `tests/rest_typing.rs` (fake, оракул — `[S]` call-site; см. §7.3) +
`tests/unit/typing-ui.test.mjs`; core-тесты typing уже есть.
**Owner** G1+G2. **P1**, WSJF 4.5. **Зависимости:** L-02 для live-доставки TYPING_START в UI.
**Ready: yes** (офлайн), при письменном решении по расширению каталога маршрутов.

---

### 3.7 V-08 — Голосовой UI: панель, участники, ping

**Что делает.** Показывает, что пользователь в голосовом канале, кто ещё там, кто говорит,
качество связи; позволяет выйти.

**Источник / evidence.** `[M]` панель/ping 93 ms/участники (`VUI:01`); `[D]` 12 состояний,
строки статусов (`VOICE.md §2.7`); дизайн принят — `DESIGN/VOICE.md` (991 строка),
машиночитаемо `DESIGN/voice-spec.json`. **Speaking-анимация не наблюдалась** (`[A]`, VG-01).

**Текущее состояние: absent.** В `index.html` нет `#voice-panel`; `mute`/`deafen`
(`actions.js:661`) — локальный `aria-pressed`, без voice-state.

**Gap.** Панель, 12 состояний, участники, speaking, ping, попап устройств, PTT-индикация.

**Контракт (на fake).** `#voice-panel[data-state]` ∈ `idle|loading|connecting|connected|
reconnecting|empty|error|no-permission|ptt-required|device-missing|server-mute|disabled`;
в текущей сборке реальны только `disabled` (voice-сессии нет) и `idle`; остальные —
управляются фейковым store. Ping — формат «93 ms»; speaking — кольцо `--positive`;
`aria-live="polite"` для смены статуса. **Не заявлять** паритет анимации/устройств/PTT
(`VOICE.md §9.3`).

**DoD / тест.** Все 12 состояний рендерятся из фейка; при отсутствии voice-сессии вход
заблокирован (`disabled`), панель не имитирует «подключено»; a11y-инварианты
`VOICE.md §8.5`; приватность §8.6. Тест: **новый** `tests/unit/voice-ui.test.mjs`
(по §8.1–8.4). **Owner** G2+G3. **P1**, WSJF 3.67. **Зависимости:** V-01…V-05 —
для реального звука; fake-срез можно сейчас (§9.2).
**Ready: yes на fake**, реальная функция — live-gate.

---

### 3.8 AT-01 — Загрузка вложения (multipart/прогресс)

**Что делает.** Прикрепить файл к сообщению.

**Источник / evidence.** Только `[D]`: путь `/channels/{id}/attachments`
(`MESSAGE_CREATE_ATTACHMENT_UPLOAD`, Enum), **call-site 0**. Метод, multipart-поля, лимиты,
413/429, refresh-urls — `[A]`. `attachments` в `MESSAGE_CREATE` в дампе всегда `null` →
схема объекта `[A]`. (**Не** выдавать `[M]`-строки «Перетащите…» за контракт.)

**Текущее состояние: absent.** Кнопка `＋` честно `aria-disabled` с title
(`actions.js:742-743`); `data.rs MessageAttachment` есть, но поля — гипотеза, используется
только для рендера, если вложение появится.

**Gap.** Всё. **Контракт: не определён** — кодировать нельзя (устав §4.1, E0 для метода).

**DoD (после RE-1).** Метод + multipart-поля + лимиты + 413/429 + refresh-urls подтверждены
живым захватом своей загрузки; затем upload → send → рендер. **Owner** G1+G2. **P2**, WSJF 3.0.
**Ready: НЕТ — RE-gate (RE-1).**

---

### 3.9 N-01 — Системные уведомления

**Что делает.** Показывает системное уведомление о новом сообщении; клик ведёт в канал.

**Источник / evidence.** `[S]` desktop 1.0.166: `DISCORD_NOTIFICATIONS_SEND_NOTIFICATION`
(`C166@63242`) → native `discord_notifications.sendNotification`; `M162:892-900`.
Наша Linux-реализация — freedesktop D-Bus, это **наш** выбор `[A]`, не wire-совместимость.
version-gap: IPC-цепочка 1.0.166, поведение UI уведомлений — 1.0.162.

**Текущее состояние: partial (crate-only).** `discord-platform/src/notifications.rs`:
`Notify` (сигнатура `susssasa{sv}i`), `CloseNotification`, `GetCapabilities`,
`AddMatch` + `ActionInvoked`/`NotificationClosed`, 8 hints; тесты `notifications_contract.rs`.
В приложении не подключено: `Notifier` не используется в `gui-spike`/`discord-app`;
`net.rs` отдаёт только статус шлюза, диспатчи в UI не идут (L-02).

**Gap.** Политика (своё сообщение, активный канал/фокус, mention-уровень, DND), адаптер,
дедупликация, клик → канал.

**Контракт.** `Notification { app_name, summary, body, actions, timeout_ms }`,
`urgency`/`category` hints; action id → маршрут канала (нужен роутер — PL-03/abs).
Политика — чистая функция с тестами; DND-подавление — после N-02 (`[A]`-правила).
Приватность: текст сообщения в summary/body виден другим приложениям session bus →
по умолчанию **без тела**, только автор + «новое сообщение» (наше решение `[A]`,
требует подтверждения PM).

**DoD / тест.** Политика: своё/активный канал/mention/DND → 0 вызовов; иначе 1;
fake-`Notifier` фиксирует поля; клик по `ActionInvoked` → событие маршрута.
Тест: `tests/unit/notify-policy.test.mjs` + Rust `notify_wiring.rs` (fake trait).
**Owner** G5+G2. **P2**, WSJF 3.0. **Зависимости:** L-02 (источник диспатчей) — для e2e.
**Ready: partial-ready** (policy + адаптер), e2e — после L-02.

---

### 3.10 N-03 — Inbox / упоминания

**Что делает.** Экран упоминаний и непрочитанного.

**Источник / evidence.** Только `[M]`: строки в `ru-catalog.json` (`CAT`), кнопка `🔔`
в шапке. Ни UI-структуры, ни счётчиков, ни запросов в дампе нет.

**Текущее состояние: absent** (`index.html:95` — `aria-disabled`, title «Phase 3»).

**Gap / контракт.** Не определён. Нельзя придумывать ни endpoint, ни модель списка.
**DoD:** сначала RE (найти экран/запрос в дампе или live-окне), затем реализация.
**Owner** G2+G1. **P2**, WSJF 3.0, conf 0.4. **Ready: НЕТ — RE-gate.**
Честная строка матрицы: `[M]` — не паритет, это только существование кнопки.

---

### 3.11 ST-01 — Навигация настроек

**Что делает.** Полный список разделов настроек с подпунктами; «Выйти» из аккаунта.

**Источник / evidence.** `[M]` `F162:63-70` («63 строки») + **независимый оракул**
`/home/sanya/re-deep/ui8/23-settings-categories.json`: `count: 63`, 63 записи с `text`,
`depth`, `cls`.
Разбор 63 записей: **2** строки профиля (`ShadowByte / Редактировать профиль`) +
**19 × 3** обёртки (`itemContainer`/`item`/`itemContent`) = 57 +
**4** подпункта «Учётной записи» (Информация об учётной записи, Пароль и безопасность,
Статус учётной записи, Семейный центр) = **63**.
Т.е. «63 строки» ≠ 63 раздела: навигация = профиль + 18 разделов + «Выйти».
version-gap: дамп 1.0.162.

**Текущее состояние: partial.** `ui/features/settings.js:16 SETTINGS_SECTIONS` — 18 разделов
в порядке дампа; 3 с контентом (`SETTINGS_ENABLED`: Уведомления, Голос и видео, Внешний вид);
остальные disabled с причиной. Нет: профиля, 4 подпунктов «Учётной записи», «Выйти».

**Gap (точно).** +1 запись «Выйти», +4 подпункта, +блок профиля. Подпункты остальных 17
разделов в захвате **свёрнуты** → их состав `[A]`, не выдумывать.

**Контракт.** `SETTINGS_SECTIONS` = 18 разделов + «Выйти»; подпункты рендерятся под
«Учётной записью» (`aria-expanded` у родителя, `aria-current="page"` у активного);
«Выйти» → существующая команда `auth_logout` (`main.rs:381`) — новых endpoint нет;
нереализованные панели остаются `disabled` c `title`-причиной.

**DoD / тест.** 63 строки воспроизводятся из оракула (фикстура-TSV, извлечённая из
`23-settings-categories.json`, **не** из нашего кода); «Выйти» → logout + состояние
`no-session`; у disabled-разделов есть причина; клавиатура/`aria-current`.
Тест: расширение `tests/unit/gui-spike-features.test.mjs` + новая фикстура-оракул.
**Owner** G2+G3. **P2**, WSJF: матрица 4.0 — **пересчитать**: после уточнения объём S
(3+2+1)/1 ≈ 6.0 (G6-оценка, PM решает).
**Ready: yes** (структура); контент панелей — отдельные P2/P3.

---

### 3.12 S-02 — Screen share / Go Live

**Что делает.** Показ своего экрана в голосовом канале.

**Источник / evidence.** `[D]` capture IPC 1.0.166: `DESKTOP_CAPTURER_GET_SOURCES`
(`C166@23953`), `ACTIVITY_CAPTURE_STREAM_*` (`C166@23840`), UI-строка «Продемонстрируйте
свой экран» (`VUI:01`).
`[S]` gateway-события `STREAM_CREATE / STREAM_SERVER_UPDATE / STREAM_UPDATE / STREAM_DELETE`
и stream key `guild:<guild>:<channel>:<user>` (`[L]` формат, `RS:7476`).
**Live:** ни одного `STREAM_*` в 1340 op0-событиях; viewer flow, rid/quality для зрителей,
`paused` — `[A]`. Linux-захват: `screen_previews`/`window_previews` — **win32-only**,
на Linux только Wayland picker (`native_screenshare_picker`) → путь PipeWire/X11 — решение G5 `[A]`.

**Текущее состояние: absent.** Кода захвата нет вообще (grep по `desktopCapturer|pipewire` = 0);
consent (PL-07) отсутствует.

**Gap.** consent → выбор источника → захват → энкод → RTP → STREAM-сигнализация → stop.
**Контракт: неполный** (только имена/поля событий по коду; поведение — `[A]`).
**DoD (после RE-3 и PL-07).** **Owner** G5+G1. **P2**, WSJF 1.6.
**Зависимости:** PL-07 (consent — **до**, не после), RE-3, зелёный voice MVP (V-03).
**Ready: НЕТ — RE + live-gate.**

---

## 4. Топ-5 задач для пула **сейчас**

Критерии: (а) контракт `[L]`/`[S]`, (б) офлайн-оракул (фикстура/дампированный DOM),
(c) нет live/RE, (d) не пересекается файлами с текущими слотами (G4 QA4-02, G1 SEC-02,
G2 G-05/G-04).

| Rank | Задача | Owner | P | WSJF | Почему сейчас | Файлы (не пересекаются) |
|---:|---|---|---|---:|---|---|
| 1 | **G-01** — READY → рейл серверов | G2 (+G1) | P1B | 5.5 | Backend `ready()` готов и парсит реальную фикстуру (7 гильдий); UI статичен. Даёт «серверы» и разблокирует C-01/C-02/G-08. Оракул — READY-фикстура, live не нужен | `ui/index.html`, `ui/app.js`, `ui/features/guild-rail.js` (new), `ui/components/rail.css` |
| 2 | **M-07** — ack из UI | G1 (+G2) | P1B | 4.5 | Контракт закрыт **полностью `[L]`**, включая пример тела из захвата (`{"token":null,"last_viewed":4310}`) — догадок не остаётся. Backend-срез (REST+IPC) разблокирует и C-03/G-08 | `src/read_state.rs` (new), `src/main.rs`, `tests/rest_ack.rs` (new) |
| 3 | **C-02** — переключение канала + история | G2+G1 | P1B | 5.0 | 80 % работы — подключить существующий `messages_page`; закрывает STUB-09 и попутно убивает дефект `channelId:"news"` (G6-D-01), без которого P0-02 всё равно не принять | `ui/features/chat.js`, `ui/app.js`, `ui/index.html`, `tests/unit/channel-switch.test.mjs` (new) |
| 4 | **P-03** — typing POST + индикатор | G1+G2 | P1B | 4.5 | `[S]` тело/успех, `[L]` входящий TYPING_START, core TTL уже есть; TTL не хардкодим — выносим в конфиг | `src/typing.rs` (new), `src/main.rs`, `tests/rest_typing.rs` (new), `ui/features/typing-indicator.js` (new) |
| 5 | **G-03** — resolve+accept инвайта | G1+G2 | P1B | 4.0 | `[S]` путь и тело, все 3 поля nullable; даёт вход в сервер и **открывает дорогу G-03a UI** (иначе у lurker-кнопки нет источника `guildId`) | `src/guilds.rs`, `src/main.rs`, `ui/features/join-server.js` (new), `tests/guilds_contract.rs` |

Порядок с учётом 2 слотов: **G2 → G-01**, **G1 → M-07**; как только G2 закрывает G-01 —
**C-02**; G1 после M-07 — **P-03**; затем **G-03** (G1) → **G-03a UI** (G2).

**Сверх пяти, если освободится третий слот (дёшево, закрывает строку матрицы):**
- **ST-01** — 4 подпункта + профиль + «Выйти» (≈S, оракул `ui8/23-settings-categories.json`,
  logout уже реализован `auth_logout`).
- **N-01a** — policy-модуль уведомлений на fake-`Notifier` (e2e — после L-02).

**Чего НЕ добавлять сейчас:** AT-01, S-02, N-03, G-02, S-01, V-10 — нет контракта (§6).

---

## 5. Что нельзя реализовывать без live/RE

| Область | Что именно нельзя | Гейт | Почему (факт, не мнение) |
|---|---|---|---|
| **Attachments (AT-01/02/03)** | метод, multipart-поля, лимиты, 413/429, refresh-urls, схема attachment | **RE-1** (live-загрузка своего файла) | `[D]` только путь, call-site 0; `attachments` в `MESSAGE_CREATE` всегда `null` |
| **Invite/leave — коды** | точные 200/204/4xx, тело ошибки, TTL `received_installation_id`, owner-leave | **RE-2** (live-окно N1) | accept-invite ни разу не наблюдался; `DELETE` — 0 запросов в 4 захватах |
| **Voice media (V-01…V-05, V-10)** | живой звук, расшифровка SRTP, DAVE/MLS, speaking | **live-окно Voice** (2 клиента) | ни одного входящего медиа-пакета; `secret_key` вырезан; `VOICE-CRYPTO.md` утерян; op5/11/13 не наблюдались |
| **Go Live (S-02/S-03)** | start/stop, viewer, rid/quality, Linux-захват | **RE-3** + **PL-07** (consent до) + V-03 | `STREAM_*` только по коду (0 live); превью экранов win32-only; consent-flow не снят |
| **Inbox (N-03)** | экран, счётчики, список упоминаний | **RE** (дамп/live) | evidence = `[M]`, только строки i18n |
| **Create server (G-02)** | `POST /guilds` | **RE-6** | Enum-запись есть, REST call-site = 0 (`B-17`) |
| **Settings sub-items 17 разделов** | состав подпунктов | **RE** (развернуть разделы в дампе) | в `23-settings-categories.json` развёрнут только «Учётная запись» |
| **N-02/N-05 (DND/override)** | правила подавления, уровни | **RE-5** | `[A]` правила; есть только строки |
| **Камера (S-01)** | превью/включение камеры | live | в живых сессиях `active:false`, устройства не перечислялись |

**Отдельно:** любой live-элемент (P0-01/02/03, A-01/A-06/A-07, M-02…M-06) требует
**окна PM и тестового токена** — сейчас `QUEUE.md §8` фиксирует их как blocked.
`E3` нет ни у одной строки — «работает» писать нельзя.

---

## 6. Карта изменений по файлам/модулям (для ready-срезов)

| Срез | Rust | UI JS | CSS/HTML | Тесты | Чужие файлы, которые **не** трогаем |
|---|---|---|---|---|---|
| **G-01** рейл | `src/data.rs` (только если нужен `position`/сортировка), иначе 0 | `ui/app.js` (boot: `ready()` до первого кадра), `ui/features/guild-rail.js` (new) | `ui/index.html` (`#guild-rail`), `ui/components/rail.css` | `tests/unit/guild-rail.test.mjs` (new) | `state/*`, `PLAN/STATUS/TASKS`, `waves/*` |
| **C-02** канал | `src/messages.rs` (если нужен `channelType` — новый селектор в `data.rs`) | `ui/features/chat.js` (источник строк), `ui/features/actions.js` (клик по `[data-channel-id]`), `ui/features/channel-tree.js` (new, если рендер из READY) | `ui/index.html` (`#channel-tree`) | `tests/unit/channel-switch.test.mjs` (new), `rust/gui-spike/tests/data_contract.rs` (аддитивно) | `ui/model.js` (высоты — DP-02), `virtual-list.js` |
| **M-07** ack | `src/read_state.rs` (new), `src/main.rs` (+command, `invoke_handler!`), `src/lib.rs` (+mod) | `ui/features/read-state.js` (new), `ui/app.js` (вызов при открытии канала) | — (unread-полоса уже в `chat.css`) | `rust/gui-spike/tests/rest_ack.rs` (new), `tests/unit/read-state-ui.test.mjs` (new) | `crates/discord-core/src/rest/routes.rs` (маршрут уже есть), `state/read_state_reducer.rs` |
| **P-03** typing | `src/typing.rs` (new), `src/main.rs`, `src/lib.rs`; **решение PM** по `crates/discord-core/src/rest/routes.rs` (расширение каталога) | `ui/features/typing-indicator.js` (new), `ui/features/composer.js` (хук «первое нажатие») | `ui/components/chat.css` или `composer.css` (строка индикатора) | `rust/gui-spike/tests/rest_typing.rs` (new), `tests/unit/typing-ui.test.mjs` (new) | `state/typing/*` (готово), `queued`-логика ядра |
| **G-03** invite | `src/guilds.rs` (resolve/accept + типы), `src/main.rs` (+2 commands) | `ui/features/join-server.js` (new), `ui/features/actions.js` (capability из backend, не из роли) | `ui/index.html` (`#join-server-dialog`), `ui/components/server-menu.css` (модалка) | `rust/gui-spike/tests/guilds_contract.rs` (аддитивно), `tests/unit/join-server.test.mjs` (new) | `DESIGN/SERVER-MENU.md` (read-only), `ui/components/*.css` кроме модалки |
| **G-03a UI** | 0 (backend принят) | `ui/features/join-server.js` (кнопка «Открыть сервер»), `ui/features/errors.js` (reuse) | `#invite-preview-card` в `index.html` | `tests/unit/server-menu.test.mjs` (new) | `src/guilds.rs` (не менять без причины) |
| **ST-01** nav | 0 (logout есть: `auth_logout`) | `ui/features/settings.js` (`SETTINGS_SECTIONS`, подпункты, «Выйти» → `invoke("auth_logout")`) | `ui/components/settings.css` (подпункты) | `tests/unit/gui-spike-features.test.mjs` (аддитивно) + фикстура-оракул `tests/fixtures/settings-nav-63.tsv` (из `ui8/23-settings-categories.json`) | 38 тумблеров (`[F]`), `settings_store.rs` |
| **V-08** панель | 0 на fake | `ui/features/voice-panel.js` (new), `ui/features/voice-store.fake.js` (new) | `ui/index.html` (`#voice-panel`), `ui/components/voice.css` (new) | `tests/unit/voice-ui.test.mjs` (new) | `discord-voice/**` (V-01…V-05 — live) |
| **N-01a** policy | `src/notify.rs` (new, trait `Notifier` + policy), `src/lib.rs` | 0 | — | `rust/gui-spike/tests/notify_wiring.rs` (new), `tests/unit/notify-policy.test.mjs` (new) | `crates/discord-platform/src/notifications.rs` (готов) |

Границы: `ui/**` — G2; `src/*.rs` — G1; `ui/components/*.css` и токены — G3 (согласование);
`state/types.rs`, `state/reducer.rs`, `state/mod.rs`, `src/lib.rs` регистрация модуля —
по правилу AGENTS.md общие файлы правит только Lead → **`src/lib.rs` в матрице среза
выделять как «shared: только add-mod, координация PM»**.

---

## 7. Риски, security, a11y, целостность тестов

### 7.1 Дефекты, найденные при сверке (оформить в `QUALITY/BUGS.md`)

| ID | Дефект | Файл:строка | Эффект | P |
|---|---|---|---|---|
| **G6-D-01** | `channelId: "news"` зашит в UI (не snowflake) | `ui/features/chat.js:432,495,509`, `ui/app.js:160` | live send/edit/delete/reply уйдут в несуществующий канал → 400/404; **P0-02 нельзя принять** | **P1** |
| **G6-D-02** | `messageId = String(index)` (синтетический номер строки) | `ui/features/chat.js:495,509` | live edit/delete → 404; приёмка M-03/M-04 невозможна | **P1** |
| **G6-D-03** | `ready()`/`messages_page`/`members_page`/`settings` не вызываются из UI | `ui/**` (grep = 0) | BFF существует, но «пустая» связка: `implemented` в матрице относится к BFF, не к экрану | P2 |
| **G6-D-04** | В матрице ST-01 «нет 45 подпунктов/панелей» — неверно | `PARITY-MATRIX.md` ST-01 | Ложный размер задачи (XL вместо S); влияет на WSJF и планирование | P2 |
| **G6-D-05** | В матрице G-01 только `[M]`, хотя источник списка — `[L]` READY | `PARITY-MATRIX.md` G-01 | Занижает готовность к коду | P3 |

### 7.2 Security

1. **`token` в теле ack** — это read-state токен (в захвате `null`), **не** bearer.
   Запрещено: подставлять auth-токен, логировать поле, класть в сообщения ошибок.
   Тест: `grep -c` по логам = 0 (как P0-03).
2. **Уведомления** уходят в session bus: summary/body читаемы другими приложениями.
   Решение по умолчанию — без текста сообщения (автор + «новое сообщение»), включается
   явно. Это `[A]`, нужно подтверждение PM.
3. **Preview инвайта** не должен рендерить `iconUrl`, склеенный на клиенте; только URL от
   backend, иначе SSRF/утечка через чужой CDN-путь (`SERVER-MENU.md §2.1`).
4. **Никаких секретов в evidence**: `secret_key`, RTC `token`, `media_session_id`, IP —
   в отчёте отсутствуют (в `NETWORK-CONTRACTS` уже `REDACTED`).
5. **Захват экрана** (S-02) — только после consent (PL-07); скрытый захват запрещён
   (`VOICE.md §5.1`). Сейчас кода нет — риска нет, но и функции нет.

### 7.3 Целостность тестов и оракулов

1. **Каталог маршрутов заморожен оракулом D-02** (47 маршрутов + POST send = 49 записей,
   `routes.rs`). `POST /channels/{id}/typing` и `POST /invites/{code}` в нём отсутствуют.
   Добавление = **письменное решение PM** + отдельный оракул-файл с цитатой `[S]`
   (anchor в бандле, `NETWORK-CONTRACTS §1.6/§1.2`), а **не** тихая правка TSV.
   Иначе тест начнёт проверять сам себя.
2. **Бенч-порог 100k строк** (`lib.rs SNAPSHOT_FLOORS`, `CHAT_ROWS=100_000`) — при C-02
   не ослаблять и не удалять; синтетика остаётся отдельным режимом.
3. **ST-01-оракул** извлекать из дампа (`ui8/23-settings-categories.json`), а не из
   `SETTINGS_SECTIONS` — иначе тест тавтологичен.
4. **`[M]` не превращать в паритет**: V-08 speaking-анимация, N-03, S-02, ST-01 подпункты.
5. Red-first и запрет `.only`/`skip` — без изменений (TEST-POLICY).

### 7.4 Доступность (a11y)

| Срез | Инвариант |
|---|---|
| G-01 рейл | Роли из дампа: `NAV aria="Боковая панель серверов"` + `group` + `treeitem` (`ui2/00-start.fingerprint.json`). Не смешивать `tree` без `treeitem`. `aria-current` на выбранном, roving tabindex, `↑/↓/Home/End` |
| G-03/G-03a | Диалог: `aria-busy` на запросе, `role="alert"` для ошибки, фокус возвращается на пункт меню; destructive-кнопка отделена; Esc не закрывает во время запроса |
| C-02 | `aria-current="page"` на активном канале; заголовок канала — `h1`; смена канала объявляется (`aria-live` на заголовке — `[A]`, наше решение) |
| M-07 | Сброс unread не должен «кричать» скринридеру: без `aria-live` на счётчике |
| P-03 | Индикатор — `aria-live="polite"`, не `assertive`; текст без имён-спама |
| V-08 | `#voice-panel[data-state]`, фокус-порядок muted/deafen/settings, `aria-pressed` синхронен с состоянием, контраст по `VOICE.md §4.7` |
| ST-01 | Навигация — `nav` + `aria-current`; disabled-раздел имеет причину в `title`/`aria-describedby`, а не «молчит» |

### 7.5 Прочие риски

| Риск | Проявление | Мера |
|---|---|---|
| version-gap 1.0.162 vs 1.0.166 | UI-«паритет» может быть устаревшим | в DoD писать «паритет 1.0.162»; renderer 1.0.166 снять в окне |
| `loadApp`-шим ≠ браузер | DOM-тесты проходят, живой webview падает | e2e/smoke после merge (G4), не отменять |
| 2 воркера в одних файлах | конфликт `main.rs`/`index.html` | файловая матрица до кода (PIPELINE §3.15) |
| «Пустой» успех lurker | 204 не означает членство | плитка не добавляется, отдельный статус |
| Обновление списка после join | `GUILD_CREATE` не наблюдался | опираться на READY/локальный список, pending-плитка |
| Slowmode/429 | typing и send имеют кулдауны | `retry_after_ms` в UI, без «тихого» ретрая |

---

## 8. Правки для `PARITY-MATRIX.md` / `GAP-BACKLOG.md` (применить PM, не G6)

| Строка | Сейчас | Предлагается |
|---|---|---|
| **ST-01** Gap | «нет 45 подпунктов/панелей» | «63 DOM-строки = профиль 2 + 18 разделов/Выйти ×3 + 4 подпункта «Учётной записи»; отсутствуют профиль, 4 подпункта, «Выйти»; подпункты 17 разделов не сняты (свёрнуты) → `[A]`» |
| **ST-01** WSJF | 4.0 | пересчитать (объём S); G6-оценка ≈ 6.0; P2 сохранить |
| **ST-01** Test | `gui-spike-features.test.mjs` | + фикстура-оракул `tests/fixtures/settings-nav-63.tsv` из `re-deep/ui8/23-settings-categories.json` |
| **G-01** Ref | `UI6:01` · `[M]` | добавить `[L] READY.guilds[]` (источник списка), `[M]` оставить за визуалом |
| **G-01** DoD | «restore» | «restore — G-01b (нужно поле в `UiSettings`); первый срез без persist» |
| **G-03** DoD | «RE-payload → join» | «тело `[S]`; `session_id`/`invite_instance_id`/`received_installation_id` nullable; live-коды RE-2» |
| **G-03a** Ready | «backend accepted, UI queued» | «UI зависит от G-03 resolve или deep link; порядок G-03 → G-03a» |
| **C-02** Gap | «нет switch, нет REST-истории» | + «дефект G6-D-01: `channelId:"news"`; + типы каналов 0/5 открывают чат, 2/4/15 — нет» |
| **M-07** DoD | «ack при просмотре» | + «`last_viewed` — счётчик read_state, `token` nullable и не bearer» |
| **N-01** | одна строка P2 | split: **N-01a** policy+adapter (ready, P2), **N-01b** dispatch wiring (blocked L-02) |
| **NEW** | — | G6-D-01…D-05 (§7.1) в `QUALITY/BUGS.md`; G6-D-01/D-02 имеют приоритет выше C-02 |

Матрица/бэклог после применения должны сходиться по счётчикам (§14 матрицы: 91 строка;
5/32/40/14) — при split N-01 число строк станет 92, пересчитать обе таблицы.

---

## 9. Проверка «не выдумано»

| Утверждение | Уровень | Источник |
|---|---|---|
| Список серверов = `READY.guilds[]`, REST-ручки нет | `[L]`+`[S]` | `NETWORK-CONTRACTS §1.3`; `002-READY.json` (7 гильдий) |
| Enter guild: `PUT …?lurker=true`, `{}`, 204 | `[L]` | `D02:101,138,225` |
| Invite accept: путь + 3 поля, все nullable | `[S]` | anchor `_.Bo.post({url:ei.Rsh.INVITE`; `getSessionId():null` |
| Коды/TTL invite и leave | `[A]` | не наблюдались; 0 запросов в 4 захватах |
| Ack: `{token,last_viewed}`, 200, `last_viewed=4310` | `[L]` | `rest-d02 §4`, `046-MESSAGE_ACK.json` |
| Typing: путь, без тела, 200 + 2 поля, `0.8*E`, ≤5 каналов | `[S]` | `NC §1.6` |
| TTL typing = 10 с | `[A]` (наше) | `state/typing/types.rs:10` |
| Каналы: типы 0/2/4/5/15, 272 шт. | `[L]` (фикстура READY) | `002-READY.json` |
| Settings: 63 строки = 2+57+4 | `[M]`+`[D]` | `ui8/23-settings-categories.json` (`count:63`) |
| Notifications: IPC 1.0.166 → native | `[S]` | `C166@63242`, `M162:892-900` |
| D-Bus Notify/Close/ActionInvoked — наша реализация | `[A]` (наш выбор) | `discord-platform/src/notifications.rs` |
| Attachments: только путь | `[D]` | `S162:817`, call-site 0 |
| Go Live: `STREAM_*` только по коду, 0 live | `[S]`/`[A]` | `NC §3.3`; `GP:150-155` |
| Linux: `screen_previews` win32-only | `[S]` | `NR:362` |
| Voice: нет входящего медиа, speaking не наблюдался | `[L]`/`[A]` | `NC §2.7`, `VOICE.md VG-01` |

**Чего в отчёте нет и не должно появиться:** придуманных query/полей (`limit`, `page_index`
у members-search), «подтверждённых» кодов invite, схемы attachment, порогов DND,
`quality` для `rid:50`, keybind PTT. Всё это — `[A]` и ждёт RE/live.

---

## 10. Evidence и воспроизводимость

| Артефакт | Значение |
|---|---|
| app.asar 1.0.166 sha256 | `0b29757f8b8194092e9ea57eb45182694a8c92fb5011e0cbc1eff0a8a690cdf1` |
| core.asar 1.0.166 sha256 | `e24a5b8393384dc885f5e1dabb48d8b6833731f01c40460c99815a9169a12770` |
| Discord ELF 1.0.166 sha256 | `37d7e3540277d878985c8f973e2313ada9601adb5bd8116e29f5bb4201ca2809` |
| READY-фикстура | `fixtures/gateway/by-type/002-READY.json` (7 гильдий, 272 канала, 17 ЛС, 3 read_state) |
| Settings-оракул | `/home/sanya/re-deep/ui8/23-settings-categories.json` (`count:63`, 5 455 Б) |
| Ack-оракул | `docs/re/rest-d02.md §4` (`{"token":null,"last_viewed":4310}`) |
| Typing/invite-оракул | `NETWORK-CONTRACTS.md §1.2, §1.6` (anchor + цитата) |

Команды проверки (read-only):

```bash
# 63 строки настроек
python3 -c "import json;d=json.load(open('/home/sanya/re-deep/ui8/23-settings-categories.json'));print(d['count'],len(d['rows']))"

# каналы/типы в READY
python3 - <<'PY'
import json,collections
d=json.load(open('fixtures/gateway/by-type/002-READY.json'))['d']
c=collections.Counter(ch['type'] for g in d['guilds'] for ch in g['channels'])
print(len(d['guilds']), sum(c.values()), dict(c))
PY

# UI не вызывает BFF
for c in ready messages_page members_page settings search_page join_guild; do
  printf '%s: ' "$c"; grep -rl "\"$c\"" rust/gui-spike/ui/ | wc -l
done
```

---

## 11. Что дальше по процессу

1. PM: применить §8 (правки матрицы/бэклога), завести G6-D-01…D-05, взять топ-5 в пул.
2. G6 Reviewer: проверить этот отчёт по уставу §11.7 (source существует, цитата точная, тег не завышен).
3. G6: после merge каждого среза — обновить строку матрицы (`parity-ok` / `deferred` с причиной).
4. Live-окна не открывались и не планировались этой сессией; окно N0/N1/Voice/Stream — решение PM.
