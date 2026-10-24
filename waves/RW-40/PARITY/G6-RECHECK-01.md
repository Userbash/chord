# G6-RECHECK-01 — предварительный parity-recheck (RW-40, после batch)

Дата: 2026-10-24, снимок 15:22 +05. Воркер: **G6 Parity** (read-only). PM: DeepSeek.
Режим: единственная запись — этот файл. Product/UI/Rust/tests, `TASKS.md`, `QUEUE.md`,
`STATE.md`, `db.jsonl`, `QUALITY/*` не менялись. Не запускались: `cargo`, `gate`, e2e,
браузер, CDP, сеть, Discord, live, мутации.

Обозначения evidence: **[V]** — проверено в этом проходе чтением/grep/md5/JSON гейта;
**[R]** — взято из матрицы/отчётов карточек. Ни один live-факт здесь не заявляется.

---

## 0. Снимок и границы

`HEAD = 0239543`, дерево **dirty** (RW-39/RW-40 не закоммичены). `rust/gui-spike/**` и
`rust/crates/discord-core/src/gateway/**` вне git-базы — диффом не проверяются, только md5.

**Дерево менялось под отчётом.** Писатель `L-02-CORE-FINAL` работал в этом же checkout:
`ws.rs`, `gateway/supervisor.rs`, `net.rs`, `main.rs` изменены в 15:17–15:21, уже **после**
приёмки L-02 (15:08) и после последнего гейта (15:10). `net.rs` менялся трижды:
`f21143f3…` (принятый L-02) → `f23aea4e…` (15:18) → `d07d16fe…` (в отчёте L-02-CORE)
→ **`e13d4e8c…`** (факт на 15:21:14). Отчёт `PM/l02-core/REPORT.md` появился в 15:21:17,
зелёный `net_contract` — в 15:19:57, т.е. **последняя правка `net.rs` новее зелёного
прогона**. md5 ниже — снимок, не фиксация.

| Файл | mtime | md5 на снимке | Комментарий |
|---|---|---|---|
| `rust/crates/discord-core/src/ws.rs` | 15:17:06 | `176791cc…` | `RunOptions.on_dispatch` + дренаж; совпал с отчётом |
| `rust/crates/discord-core/src/gateway/supervisor.rs` | 15:17:21 | `5180b637…` | `with_dispatch_handler`; совпал с отчётом |
| `rust/gui-spike/src/net.rs` | 15:21:14 | `e13d4e8c…` | `CORE_DISPATCH_AVAILABLE = true`; **не** совпал с отчётом (`d07d16fe…`) |
| `rust/gui-spike/src/main.rs` | 15:17:40 | `c003a9e5…` | эмиттер + `net_dispatch_stats`; совпал с отчётом |
| `rust/gui-spike/tests/net_contract.rs` | 15:19:46 | `19f24968…` | 22 passed (`green-net-contract.log`) |
| `rust/gui-spike/ui/features/chat.js` | 14:59 | `f4c80763…` | совпадает с C-02-PAGE |
| `rust/gui-spike/ui/app.js` | 14:33 | `78039af3…` | совпадает с P-03-UI |
| `rust/gui-spike/ui/features/typing-indicator.js` | 14:34 | `09f9f538…` | совпадает с P-03-UI |
| `tests/unit/channel-paging.test.mjs` | 15:09 | `54184e41…` | **не** совпадает с отчётом (`5d98e960…`) |

Приёмка L-02 фиксировала `net.rs f21143f3…`, `main.rs 0082fb4b…`; текущие md5 другие —
ожидаемо для следующего среза того же лейна, но означает: **принятый md5 больше не
описывает дерево**, и merge обязан принять не «L-02 по отчёту», а `L-02-CORE-FINAL` с
новым отчётом, приёмкой и гейтом. Отдельный разрыв: `net.rs e13d4e8c` (15:21:14) новее
`net_contract` 22/22 (15:19:57) — последняя правка не покрыта зелёным контрактным прогоном.

Параллельно в том же каталоге существует `PARITY/G6-RECHECK-01-VERIFY.md` (второй
G6-воркер, снимок 15:22): его числа (91/21/36/34, release 09:37, worker 10:10) совпали с
моими; дельта этого файла учтена ниже (в частности, `net_contract` к 15:19 уже зелёный,
а `PM/l02-core/REPORT.md` к 15:21 появился).

Гейты (прочитаны, не запускались):

| Гейт | Вердикт | Что покрыто | Чего нет |
|---|---|---|---|
| `release-2026-10-24T09-37Z` (14:37 local) | `passed_with_skips` | meta 65, unit 643, smoke 22, workspace 1103 pass + 12 ignored, **e2e 0 pass / 4 skip**, keyring-live 12/12, coverage 643 | нет P-03-UI, C-02-PAGE, L-02, L-02-CORE |
| `worker-2026-10-24T10-10Z` (15:10 local) | `passed_with_skips` | typecheck, ui-tsc, meta 65, unit 651, spike-rust, workspace 1103 + 12, smoke 22 | **нет e2e/coverage/keyring-live**; нет изменений 15:16+ |

Вывод: **release-гейт старше четырёх последних срезов**, worker-гейт старше L-02-CORE.
Ни один гейт не покрывает текущее дерево целиком.

---

## 1. Числа

Правило strict recheck: строка считается `implemented` только если её пользовательская
функция достижима end-to-end офлайн; `verified` — только при E3 (live). Класс матрицы
при этом **не переписывается** (владелец — PM), ниже он отделён от recheck-класса.

| Класс | Матрица (G6-SYNC-01, 14:34) | Strict recheck | Δ |
|---|---:|---:|---:|
| verified (E3) | 0 | **0** | 0 |
| implemented (offline, live unverified) | 21 | **15** | −6 |
| partial | 36 | **42** | +6 |
| absent | 34 | **34** | 0 |
| unknown | 0 | **0** | 0 |
| **Всего строк** | **91** | **91** | 0 |
| **parity-ok строк** | **0** | **0** | 0 |

Проверка сходимости матрицы (команда §14 матрицы, независимый оракул — сам markdown):
`implemented 21 / partial 36 / absent 34 / unknown 0 / total 91` **[V]**. Три P0-гейта
входят в 91 (P0-01/02 — `partial (offline)`, P0-03 — `implemented (offline)`).

Сдвиги после синка (не отражены в матрице):

| Срез | Время | Что даёт | Класс строки |
|---|---|---|---|
| P-03-UI | 14:35 | исходящий `send_typing`, индикатор «Печатает…», 20 тестов, offline/fake | P-03 остаётся `partial` (входящие — отдельный forward) |
| C-02-PAGE | 15:00 | `nextCursor/hasMore`, окно ≤500, дедуп, ошибки, 8 тестов | C-02: gap «только первая страница» устарел; функция всё ещё не полная |
| L-02 (backend) | 15:03 | канал dispatch, 6 kinds, redaction, дедуп, 14 тестов | L-02 остаётся `partial` (UI нет) |
| L-02-CORE | 15:16–15:21 | `RunOptions.on_dispatch`, дренаж пайпа, тест на реальном `Supervisor`; отчёт есть, targeted green | L-02 core готов **к приёмке**; гейта и UI нет |

---

## 2. Таблица 91 функции

Колонка «Recheck»: `impl*` — implemented offline, live не проверен; `impl*↓` — de facto
partial (см. §3); `partial`; `absent`. Для absent-строк тест в матрице — **план**, а не
существующий файл (реально существуют только `auth`, `channels`, `data`, `guilds`,
`invites`, `messages`, `net`, `reactions`, `read_state`, `rest_ack`, `search`, `settings`,
`typing` в `rust/gui-spike/tests/`, плюс 32 unit-файла в `tests/unit/`) **[V]**.

| ID | Функция | Матрица | Recheck | Evidence / граница |
|---|---|---|---|---|
| A-01 | Вход логин/пароль | impl* | impl* | `auth_contract.rs`; live-вход N0-01 не выполнен → E3 нет |
| A-02 | MFA TOTP/SMS/backup | impl* | impl* | `auth_contract.rs`; live 5 методов не прогнаны |
| A-03 | Captcha-гейт | impl (offline) | impl* | `auth_contract.rs`; live-капча/sitekey не сняты |
| A-04 | QR login | absent | absent | UI disabled; протокол QR не снят |
| A-05 | Passkey/WebAuthn | absent | absent | Linux-WebAuthn отсутствует |
| A-06 | Восстановление сессии | impl* | impl* | keyring `[A]`; live-restore не проверен |
| A-07 | Logout/инвалидация | impl* | impl* | контракт есть; live stop/401 не проверены |
| A-08 | Регистрация/сброс | absent | absent | web-flow вне desktop scope |
| A-09 | Сессии/logout others | absent | absent | нет экрана и вызова |
| G-01 | Рейл серверов | impl* | impl*↓ | READY 7 гильдий, `guild-rail` (8) **[V]**; restore `lastGuildId` нет (G-01b) |
| G-02 | Создать/добавить сервер | absent | absent | endpoint `GET/POST /guilds` не определён |
| G-03 | Вход по инвайту | impl* | impl* | `invites_contract` (14) + `join-server` (24); live-коды/TTL `[A]` |
| G-03a | Вход из preview/lurker | impl* | impl* | backend `[L]` 204×2; live 204 из UI не снят |
| G-04 | Выход с сервера | impl* | impl* | `guilds_contract` + features; live-коды `[A]` |
| G-05 | Меню сервера | partial | partial | **[V]** `actions.js` SPEC: активны leave + copy-id, invite по `capability=false` → 2/10 |
| G-06 | Инвайты CRUD | absent | absent | API/UI нет; коды `[A]` |
| G-07 | Роли/права сервера | absent | absent | UI не снят |
| G-08 | Unread/mention сервера | partial | partial | статичный `is-unread`; счётчиков/read_state нет |
| C-01 | Дерево каналов | partial | partial | READY-типы 0/2/4/5/15; нет open voice/forum, collapse |
| C-02 | Канал + история | impl* | impl*↓ | paging только вперёд по `nextCursor`; tail/older нет; live-история не снята |
| C-03 | Unread/mention канала | partial | partial | статичный класс; нет mention/ack-счётчика |
| C-04 | Шапка: pins/threads/members/search | partial | partial | поиск/участники есть; pins/threads stub |
| C-05 | Создание/редактирование канала | absent | absent | UI не снят |
| C-07 | QuickSwitcher `Ctrl+K` | impl (B6) | impl*↓ | **[V]** гильдии ищутся по `dataset.guild`, рейл отдаёт `data-guild-id` → выбор no-op; канал не грузит историю |
| M-01 | История/пагинация | partial | partial | fixtures + PageStore; live-история/курсоры нет |
| M-02 | Отправка | impl* | impl* | `messages_contract.rs`; live send/401/429 не выполнены |
| M-03 | Редактирование | impl* | impl* | тело/live `[A]` |
| M-04 | Удаление | impl* | impl* | live 403/tombstone не проверены |
| M-05 | Reply | impl* | impl* | `message_reference`; live-поле `[A]` |
| M-06 | Реакции | impl* | impl*↓ | pills на реальных `channel:<id>:<msg>` **[V]**; входящие `MESSAGE_REACTION_*` — L-02; live PUT/DELETE/429 нет |
| M-07 | Read state/ack | impl* | impl*↓ | ack open/focus/scroll, `token:null`; unread только `mentionCount`, badge нет; live ack нет |
| M-08 | Markdown/embeds/mentions | partial | partial | только embeds |
| M-09 | Контекстное меню | partial | partial | 4 команды, остальные stub |
| M-10 | Pins | absent | absent | stub |
| M-11 | Threads | absent | absent | stub |
| M-12 | Forward/bookmark/reminder/TTS | absent | absent | endpoints `[A]` |
| M-13 | Поиск по серверу | impl* (4/7) | impl*↓ | **[V]** `app.js:1206` берёт `dataset.guild` (пусто) → `search_remote` не включается; остальные фильтры/лимиты `[A]` |
| M-14 | Эмодзи-пикер | impl (static) | impl* | статичный набор; кастомные/стикеры нет |
| MB-01 | Участники: роли+статусы | partial | partial | synthetic 24k/6 групп; live-списка нет |
| MB-02 | Роли/цвета/права | partial | partial | synthetic |
| MB-03 | Профиль-поповер | partial | partial | stub aria |
| MB-04 | Полный поиск участников | absent | absent | пагинация/коды `[A]` |
| P-01 | Presence-статусы | partial | partial | synthetic + core; live нет |
| P-02 | PRESENCE_UPDATE | partial | partial | reducer есть; UI не связан (L-02) |
| P-03 | Typing send/indicator | partial | partial | backend `typing_contract` (13) + offline UI `typing-ui` (20); входящие `TYPING_START` — L-02; live нет |
| P-04 | Custom status/activity | absent | absent | нет UI/обновления |
| P-05 | Смена своего статуса | absent | absent | нет меню/UPDATE |
| V-01 | Voice join/leave | absent | absent | только crate |
| V-02 | RTC-сигнализация | partial | partial | парсер/тесты; live WS/сессии нет |
| V-03 | UDP discovery/keepalive | partial | partial | примитивы; live UDP нет |
| V-04 | SRTP | partial | partial | KDF/AEAD тесты; live encrypt/decrypt нет |
| V-05 | Opus/джиттер/DTX | partial | partial | jitter only, Opus нет |
| V-06 | Voice stats/reconnect | partial | partial | policy/stats; live метрик нет |
| V-07 | Устройства/mute/PTT | absent | absent | settings fake |
| V-08 | Voice UI | partial | partial | панель на fake store (D-115) |
| V-09 | Krisp | absent | absent | модели/интеграции нет |
| V-10 | DAVE/MLS | absent | absent | init evidence, обмена нет |
| V-11 | Voice messages/clips | absent | absent | контракт не снят |
| S-01 | Камера | absent | absent | capture нет |
| S-02 | Screen share/Go Live | absent | absent | capture/STREAM нет |
| S-03 | Зрители/simulcast | absent | absent | RTP-видео нет |
| S-04 | Activities | absent | absent | контракт запуска `[A]` |
| AT-01 | Загрузка вложения | absent | absent | method/fields `[A]`, кнопка disabled |
| AT-02 | Drag&drop/paste | absent | absent | только строка каталога |
| AT-03 | Рендер вложений | absent | absent | только embeds |
| ST-01 | Навигация настроек | partial | partial | 18 разделов/3 панели (D-118); 17 подпунктов `[A]` |
| ST-02 | Voice/video settings | partial | partial | 38 toggles без эффекта; устройств нет |
| ST-03 | Тема/плотность/zoom | partial | partial | dark/light/density; zoom нет |
| ST-04 | Язык/время/i18n | partial | partial | crate 16k строк не подключён |
| ST-05 | Уведомления-настройки | partial | partial | панель-заглушка |
| ST-06 | Горячие клавиши | absent | absent | редактора нет |
| ST-07 | Аккаунт/профиль | absent | absent | полей/действий нет |
| ST-08 | Персистентность/sync | partial | partial | локальный файл `[A]` |
| N-01 | Системные уведомления | partial | partial | D-Bus crate не подключён (L-02) |
| N-02 | DND/focus | partial | partial | статус-лейбл + idle policy `[A]` |
| N-03 | Inbox/упоминания | absent | absent | кнопка disabled |
| N-04 | Бейдж/прогресс | absent | absent | нет |
| N-05 | Переопределения уведомлений | absent | absent | нет |
| PL-01 | Трей/фон | absent | absent | нет |
| PL-02 | Автозапуск | partial | partial | модель `.desktop` |
| PL-03 | Deep links/single instance | partial | partial | парсер `discord://` |
| PL-04 | Апдейтер | partial | partial | manifest/plan |
| PL-05 | Spellcheck | partial | partial | hunspell FFI, интеграция неполна |
| PL-06 | Power/idle | partial | partial | crate only |
| PL-07 | Consent захвата | absent | absent | нет |
| PL-08 | Окно badge/flash/PiP | absent | absent | нет |
| L-01 | Reconnect/offline UI | impl* | impl* | `#net-status` + retry; live resume/close-коды нет |
| L-02 | Dispatch → UI | partial | partial | backend принят (6 kinds, очередь 256, дедуп); core hook готов к приёмке (тест на реальном `Supervisor`); **[V]** в UI 0 вхождений `gateway://dispatch`/`listen` → потребителя нет |
| P0-01 | Live READY | partial (offline) | partial | live READY/guild/channel не сняты; окно PM |
| P0-02 | Live send | partial (offline) | partial | offline-контракт не доказывает доставку |
| P0-03 | Секреты не в логах | impl (offline) | impl* | offline redaction; live `grep token/password/key = 0` не выполнен |

---

## 3. Какие `implemented` стали de facto partial

Понижены 6 строк (матрица не переписана — это флаг для PM). Три из них вскрыты срезами
L-02/C-02, три — сверкой G-01/C-02 в этом проходе.

| ID | Почему не end-to-end | Evidence | Что вернёт класс |
|---|---|---|---|
| C-02 | История листается только **вперёд**: `cursor_start` отдаёт строки после курсора, подгрузка на нижнем крае; открытие на последнем сообщении и «старее» не реализуемы без `before`-курсора | C-02-PAGE §6 (варианты a/b); `chat.js` `loadMoreHistory`; `data.rs::cursor_start` | решение PM по направлению; при «older» — G1 `before`-курсор + oracle двух страниц + tail-open |
| C-07 | **[V]** гильдии в свитчере адресуются `node.dataset.guild`, а рейл G-01 выставляет `data-guild-id` (`app.js:489,492`); target = `""`, выбор плитки не срабатывает. Канал только помечается `aria-current` + заголовок, `openChannel`/`messages_page` не вызывается (`quick-switcher.js:133-142`) | `quick-switcher.js:67,144,150`; `actions.js:367-369` уже делает fallback на `data-guild-id` | fallback на `data-guild-id` + тест выбора гильдии; вызов `openChannel(channelId)` из свитчера |
| M-13 | **[V]** `app.js:1206` передаёт `guildId` из `selectedGuild.dataset.guild` (пусто) → серверный `search_remote` не включается вовсе; работает локальный fallback | `app.js:1206`; `search-overlay.js:121` | `data-guild-id` + тест, что `search_remote` получает реальный guildId |
| M-06 | Свои реакции рисуются/шлются на реальных id, но чужие `MESSAGE_REACTION_ADD/REMOVE` не доходят до UI (нет L-02-подписчика) → в мультипользовательском канале функция неполна | `chat-reactions` (9); L-02 `FORWARDED_DISPATCHES`; **[V]** UI без `listen` | L-02-UI mapping ADD/REMOVE (REMOVE — `[A]`, 0 кадров: без вымысла) |
| M-07 | Ack работает, но unread выводится только по `mentionCount`; unread без упоминаний и badge-счётчики не проецируются | M-07-UI §; READY: 2 badge-записи; матрица: gap G-08/C-03 | G1 projection `flags`/`badgeCount` + G2 badge/clear-on-ack + `null ≠ 0` |
| G-01 | Список/выбор/переключение из READY есть, но `lastGuildId` не сохраняется — после рестарта выбор теряется | **[V]** `settings_store.rs::UiSettings` без поля; `app.js:329` «G-01b, вне scope» | G-01b: поле + миграция + round-trip + restore-тест |

Не понижались, хотя residual есть: M-14 (статичный набор), M-13 частично (см. выше),
A-03/A-06 (`[A]`-оговорки), C-07 (см. выше). `M-01` и `P-03` уже `partial`.
`P0-03` остаётся `implemented (offline)` — live-грепа нет, но и функции в UI нет.

---

## 4. Блокеры parity и минимальное evidence

| Область | Что именно блокирует | Минимальный evidence для снятия (без live / с live) |
|---|---|---|
| **L-02-CORE** | Хук и дренаж в дереве; `PM/l02-core/REPORT.md` (15:21) даёт red-first, targeted 12/12, core 575, gui 269, `net_contract` 22/22, clippy/fmt чисто. Но: приёмки PM нет; гейта после 15:21 нет; `net.rs` правился **после** зелёного `net_contract` (15:21:14 vs 15:19:57) и md5 в отчёте не совпал; mutation-proof `session_id` из `PM/l02/ACCEPT.md §3` не выполнен | **Offline:** приёмка `PM/l02-core/ACCEPT.md`; перепрогон `net_contract` на финальном md5; гейт на текущем дереве; QA-мутация redaction. **Live:** не требуется для core, но нужен для `sourceAvailable=true` в реальной сессии |
| **L-02-UI** | **[V]** в `ui/**` 0 вхождений `gateway://dispatch` и `listen(`; `net_dispatch_stats` не вызывается. Даже с готовым core события никто не потребляет. Нет состояния «нет источника» (`sourceAvailable=false`) — условие приёмки L-02 №2. Контракт G2 (`PM/l02-ui/PLAN.md`) существует, но код не начат | **Offline:** подписчик `gateway://dispatch` → mapper по 6 `kind` → состояние экранов без reload; DOM-тест на READY (рейл/дерево), CHUNK/LIST_UPDATE (участники), PRESENCE, REACTION_ADD; тест «`sourceAvailable=false` → видимое состояние, а не пустой чат»; QA-мутация: убрать redaction `session_id` → контрактный тест падает |
| **G6-D-03** | Формулировка устарела. **[V]** UI вызывает `ready`, `channels`, `messages_page`, `read_state`, `ack_message`, `send_typing`, `join_guild`, `resolve_invite`, `accept_invite`, `leave_guild`. **Не вызывает:** `members_page` (0), `search_page` (0), `settings` (0; есть только `settings_load/save`). `enter_guild` — не дефект: UI-глагол `join_guild` — alias той же операции (`guilds.rs:393-398`) | Закрыть D-03 как: (а) `enter_guild` — documented alias + тест; (б) `members_page` — вызов из экрана участников + DOM-тест на fixture; (в) `search_page` — решение PM (локальный поиск vs backend) + тест; (г) `settings` — решение PM (статические секции vs backend) + тест. Без этого D-03 остаётся P2 |
| **C-02 forward/older** | Backend листает вперёд; «предыдущая/старее» и открытие на последнем сообщении не реализуемы на текущем `data.rs`. Live-история не снята | **Offline:** письменное решение PM (a: forward-only приемлемо, b: `before`). При (b): G1 `before`-курсор + тест-оракул на двух страницах (старшие/младшие snowflake, без дублей, якорь), tail-open. **Live:** захват истории реального канала (M-01), сверка порядка/окна |
| **G-05 (2/10)** | **[V]** в коде активны ровно 2 действия (leave, copy-id) + invite при `capability=true`, но capability никто не выставляет (`actions.js:77-78` → `false`). Остальные 8 — `disabled`-заглушки. G3-спека подтверждает: у 8 пунктов нет подтверждённых route/capability (`[A]`) | **Offline:** таблица 10 пунктов с `action`/capability/disabled-reason; для local-only (#5 скрыть заглушённые — локальная настройка, #10 copy-id уже есть) — включить с тестом; для #1/#3/#4/#6/#7/#8 — RE-контракт capability/route. **Live:** меню по реальным правам. `10/10` без RE/live недостижимо — нужен либо RE-2, либо письменный scope-waiver |
| **M-07 unread/badge** | Нет проекции `flags`/badge; unread только по упоминаниям; `G-08`/`C-03` остаются `partial`; live ack не проверен; входящий `MESSAGE_ACK` L-02 **не форвардит** | **Offline:** G1 отдаёт badge-форму (`badge_count`/`read_state_type`) без вымысла (2 записи в READY), G2 агрегирует badge и снимает по ack; тесты `null ≠ 0`, unread без mention, очистка; обновить G-08/C-03; отдельная forward-карточка для `MESSAGE_ACK`, если нужны live-обновления. **Live:** один `POST ack` 200 + сброс полосы |
| **G-01b** | **[V]** `UiSettings` без `last_guild_id`; restore не реализован | **Offline:** поле (Option) + миграция/неизвестный id → fallback + round-trip save/reload + UI-restore из READY-рейла (не «в текущей сессии») |
| **P-03** | Исходящий путь готов offline (sender/guard/TTL/ошибки). Входящий `TYPING_START` не доходит: `typing.start()` не вызывается нигде **[V]**, и L-02 его **не форвардит** (`FORWARDED_DISPATCHES` — 6 имён без typing; G2-контракт прямо исключает) | **Offline:** отдельная G1-карточка: `TYPING_START` в `FORWARDED_DISPATCHES` с `[L]`-оракулом (кадр есть) + G2-маппинг в `typing.start`; DOM-тест индикатора из события. **Live:** опционально, окно |
| **M-06** | Входящие реакции и live-запись не проверены; REMOVE `[A]` (0 кадров) | **Offline:** ADD-fixture → mapper → строка (ADD/REMOVE уже в 6 форвардимых); optimistic rollback/429 уже есть. **Live:** PUT/DELETE/429 + второе клиентское событие. REMOVE не выдумывать до захвата |
| **e2e** | **[V]** `CHROME_BIN` не задан, `chromium/google-chrome` в `PATH` отсутствуют; release-гейт: e2e `pass 0 / skip 4`. Фаза e2e без `failOnSkips`, поэтому skip неотличим от pass по вердикту | **Offline:** браузер в образе + `CHROME_BIN`; прогон `cdp-browser` + `walk-fixture` с `pass 2 / skip 2 (live)`; приёмка читает `counts`, а не только verdict. **Live:** `DISCORD_LAB_LIVE_CDP` → 2 live-кейса |
| **P0 live** | READY/send/no-secrets не сняты; окна не было | Окно PM: READY + guild/channel, send виден, 401/429, `grep token/password/key = 0`. Ни один из фактов здесь не заявляется |

Отдельно по реестру: evidence `G6-D-03` в `QUALITY/BUGS.md:90` («`ready`/`members_page`/
`search_page`/`settings`/`enter_guild` = 0 invoke») **неверен** для `ready` (вызывается через
`READY_COMMAND`) и `join_guild` (UI-алиас `enter_guild`). D-03 нужно переписать по факту
(правка документа, не продуктовый дефект); иначе дефект закрывается «одним L-02»,
чего он не заслуживает.

### Новые кандидаты в `QUALITY/BUGS.md` (не зарегистрированы — файл вне моей зоны)

| ID | Дефект | Файл:строка | Эффект | P |
|---|---|---|---|---|
| R-01 | Рейл G-01 выставляет `data-guild-id`, а три потребителя читают `dataset.guild` | `ui/app.js:1206`; `ui/features/search-overlay.js:121`; `ui/features/quick-switcher.js:67,144,150` | серверный поиск не включается; выбор гильдии в свитчере — no-op | P2 |
| R-02 | Выбор канала в QuickSwitcher не открывает канал (только `aria-current` + заголовок) | `ui/features/quick-switcher.js:133-142` | «переключение» из свитчера не грузит историю | P2 |
| R-03 | md5-дрейф: `tests/unit/channel-paging.test.mjs` не совпадает с отчётом, отдельного red/green-лога на правку нет | `PM/c02-page/REPORT.md` §5 vs факт `54184e41…` | «не ослаблено» доказывается прогоном, а не диффом | P3 |

Прецедент исправления R-01 уже есть: `actions.js:367-369,499-500` читает оба атрибута.

---

## 5. Dependency graph

```text
ДО L-02-CORE (сейчас; один writer на checkout → только read-only):
  G2  L-02-UI-CONTRACT (design, DESIGN/*)        ─┐
  G3  G-05 spec/validate (DESIGN/G05-*)          ─┤ файлы не пересекаются
  G4  QA-E2E-PREP (QUALITY/*, план)              ─┤ product не трогают
  G5  VOICE-PREREQ (VOICE/*)                     ─┤
  G6  G6-RECHECK / MERGE-AUDIT (PARITY/*)        ─┘
  G1  L-02-CORE-FINAL — единственный writer: ws.rs, supervisor.rs, net.rs, main.rs

ПОСЛЕ приёмки L-02-CORE + гейта (разблокируется код):
  L-02-UI (G2: new ui/features/dispatch.js + app.js + index.html + dispatch-ui.test;
            контракт уже есть — PM/l02-ui/PLAN.md)
      ├─ READY → рейл/дерево/каналы (G-01, C-01, D-03)
      ├─ GUILD_MEMBERS_CHUNK / LIST_UPDATE → участники (MB-01; live-имена ролей — отдельно)
      ├─ PRESENCE_UPDATE → P-01/P-02
      └─ MESSAGE_REACTION_ADD/REMOVE [A] → M-06 входящие

  ОТДЕЛЬНЫЕ forward-карточки (L-02 их НЕ форвардит — 6 имён, net.rs:113-120):
      ├─ TYPING_START → P-03 входящий (кадр `[L]` есть; G1 + G2)
      ├─ MESSAGE_ACK → M-07 live-badge (после G1 badge-проекции)
      └─ MESSAGE_CREATE → N-01/M-01 live-чат (контракт не входит в RW-40 L-02)

  G1: C-02-BEFORE (data.rs) ──→ M-07 badge projection (data.rs; тот же файл — строго после)
  G1: PLAT-11a (main.rs; после L-02-CORE)
  G1: G-01b contract (settings_store.rs)
  G2: G-01b UI (app.js; после L-02-UI — тот же файл)
  G2: G-05 #5/#10 (actions.js; независимо от app.js)
  G2: M-07 unread UI (chat.js; после L-02-UI и G1-проекции)
  R-01/R-02 фикс (app.js/search-overlay/quick-switcher; после L-02-UI, тот же app.js)

ТРЕБУЕТ LIVE/RE (вне offline):
  P0-01 READY → P0-02 send → P0-03 no-secrets (одно окно)
    ├─ M-06 live PUT/DELETE/429 + второе клиентское событие
    ├─ P-03 live TYPING_START, M-07 live ack
    ├─ G-03 live коды/TTL, G-03a live 204
    ├─ G-05 реальные права (плюс RE-2 capability)
    └─ MB-01/MB-02/P-01 live-данные
  V-01…V-08 voice: live voice + libopus (D-113)
  e2e: fixture — только браузер/CI; live — CDP endpoint + окно
```

Правило файлов: `chat.js` (C-02-PAGE/M-06/M-07), `app.js` (P-03-UI/G-01/G-01b/L-02-UI),
`actions.js` (G-05/M-07), `data.rs` (C-02-BEFORE/M-07), `main.rs` (L-02-CORE/PLAT-11a) —
**одновременно только один владелец**. Read-only отчёты пишут только свои каталоги.

---

## 6. Что можно передать без конфликта файлов

Только как кандидаты; назначение и файловая матрица — PM/Tech Lead. До приёмки
L-02-CORE кодовые передачи в этом checkout не запускать (один writer).

| Кому | Задача | Файлы (монополия) | Предусловие | Конфликт, которого нельзя допустить |
|---|---|---|---|---|
| **G1** | C-02-BEFORE: `before`-курсор в `data.rs` | `rust/gui-spike/src/data.rs`, новый `tests/history_*`/`data_contract` | решение PM «older нужен» | не параллелить с M-07-проекцией (`data.rs`); UI уже готов (prepend + якорь) — `chat.js` не трогать |
| **G1** | M-07 badge projection | `data.rs` (после C-02-BEFORE), `read_state_contract.rs` | C-02-BEFORE принят | `data.rs` строго последовательно |
| **G1** | PLAT-11a channelId allowlist | `main.rs` (+ тест) | L-02-CORE принят и md5 зафиксирован | `main.rs` занят L-02-CORE |
| **G1** | G-01b contract | `settings_store.rs`, `settings_contract.rs` | — | не менять `app.js` (это G2) |
| **G1** | Forward-расширение: `TYPING_START` (и при необходимости `MESSAGE_ACK`) | `net.rs` (`FORWARDED_DISPATCHES` + маппинг), `net_contract.rs` | L-02-CORE принят, `net.rs` свободен | `net.rs` занят L-02-CORE; не параллелить с другими forward-правками |
| **G2** | L-02-UI: подписчик + projections | новый `ui/features/dispatch.js`, `ui/app.js`, `ui/index.html`, `tests/unit/dispatch-ui.test.mjs` | L-02-CORE принят; контракт `PM/l02-ui/PLAN.md` | `app.js` нельзя делить с G-01b-UI/P-03; `chat.js` не трогать |
| **G2** | G-01b UI restore | `ui/app.js`, `guild-rail.test.mjs` | G1-контракт поля | после L-02-UI (тот же `app.js`) |
| **G2** | G-05 #5 (hide muted, local) | `ui/features/actions.js`, `gui-spike-features.test.mjs` | PM подтверждает local-only | `actions.js` также нужен M-07-UI — не параллелить |
| **G2** | M-07 unread UI | `ui/features/chat.js`, `read-state-ui.test.mjs` | G1 badge projection | `chat.js` занят C-02-PAGE/M-06 — только после их заморозки |
| **G2** | R-01/R-02 фикс | `app.js`, `search-overlay.js`, `quick-switcher.js` | L-02-UI принят | те же файлы, что L-02-UI/G-01b — последовательно |
| **G3** | G-05 оставшиеся 6 пунктов: контракт/capability | `DESIGN/G05-REST-SPEC.md` и только design | PM/RE | product-файлы не трогать; продуктовый список должен совпасть с кодом G2 |
| **G4** | e2e: браузер в runner + `counts`-приёмка | `tests/e2e/**`, `tests/helpers/browser.mjs`, `scripts/gate.mjs` | TL разрешает правку гейта | не менять G2 unit-тесты; `gate.mjs` — зона TL |
| **G4** | L-02 QA-мутация redaction | отдельный лог/скрипт, без product | L-02-CORE принят | не «зеленить» тесты |
| **G5** | voice prereq / V-04-V-05 | `VOICE/*`, затем `media`/`devices` | libopus (D-113), live voice | не кодировать core/UI G1/G2 |
| **G6** | повторный recheck после L-02 | `PARITY/G6-RECHECK-02.md` | приёмка L-02-CORE+UI | только G6-доки |

---

## 7. Условия закрытия (порядок)

`parity-ok` возможен только при выполнении **всех** пунктов; сейчас не выполнен ни один
полностью.

1. **L-02-CORE**: приёмка `PM/l02-core/ACCEPT.md` по отчёту 15:21 (red-first, targeted
   12/12, core 575, gui 269, `net_contract` 22/22, clippy/fmt); перепрогон `net_contract`
   на финальном `net.rs` (последняя правка новее зелёного лога); свежий гейт; QA-мутация
   redaction (caught).
2. **L-02-UI**: подписчик `gateway://dispatch` + mapper 6 kinds + состояние «нет источника»;
   DOM-тесты без reload.
3. **Гейт**: release-scope прогон после merge всего accepted set; e2e с браузером
   (`pass 2 / skip 2` минимум; для parity-ok — live 2 тоже); вердикт читается по `counts`.
4. **Parity-остатки**: C-02 (решение + before/tail), G-05 (10/10 или письменный waiver),
   M-07 unread/badge, G-01b, P-03 входящий, M-06 live, R-01/R-02.
5. **Матрица/доки**: PM применяет §8 `G6-REPORT-RW40-02` + правки этого отчёта
   (P-03 UI, C-02 paging, D-03 narrowing), пересчитывает 91 строку; TL синхронизирует
   `TASKS/QUEUE/STATE/db.jsonl`. Сейчас `TASKS/QUEUE` противоречат `MERGE-MANIFEST`
   (P-03 UI, C-02-PAGE, L-02 отсутствуют как карточки).
6. **P0 live**: READY, send, no-secrets в одном окне; после — live e2e.
7. **Live-остатки**: G-03/G-03a коды/204, live permissions G-05, MB-01/P-01 presence.

---

## 8. Вердикт

### `deferred`

Не `parity-ok`:
- **0 из 91** строк подтверждены E3; `implemented (live unverified)` — не паритет;
- L-02 не end-to-end: core-хук готов к приёмке (отчёт 15:21, targeted green), но приёмки
  и гейта нет, а UI-подписчика нет вообще (`gateway://dispatch` = 0 вхождений), т.е.
  live-события не доходят до экранов; `TYPING_START`/`MESSAGE_ACK` L-02 не форвардит;
- 6 строк `implemented` de facto partial (C-02, C-07, M-06, M-07, M-13, G-01);
- G-05 — 2/10, остальные 8 без подтверждённых capability/route;
- e2e — 0 pass / 4 skip, браузера в среде нет; release-гейт старше четырёх срезов;
- P0-01/02/03 live не сняты.

Не `block`:
- P0-дефектов не найдено; вымысла endpoint/полей нет (G6-BATCH §3);
- offline-путь и тесты есть, задачи для движения перечислены и не требуют
  неразрешимых ресурсов, кроме live-окна и браузера;
- последний worker-гейт (15:10) зелёный; targeted-логи L-02-CORE (15:16–15:20) зелёные;
  текущее дерево после 15:21 не гейтилось, но признаков поломки нет.

Ближайшая точка, где вердикт может измениться: приёмка **L-02-CORE + L-02-UI** и
release-гейт на одном дереве. До этого любые заявления о parity L-02/M-06/M-07/P-03
будут преждевременными.

---

## 9. Ограничения recheck

Не запускались `cargo`, `gate`, e2e, браузер, CDP, сеть, Discord, live, мутации; тесты не
исполнялись. Все числа тестов взяты из `.gate/*.json` и отчётов карточек. Дерево менялось
во время прохода (L-02-CORE-FINAL), md5 — снимок 15:17–15:22. Отчёт не закрывает ни
карточки, ни волну, ни waiver; `QUALITY/BUGS.md`, `TASKS.md`, `QUEUE.md`, `STATE.md`,
`db.jsonl` не изменялись.
