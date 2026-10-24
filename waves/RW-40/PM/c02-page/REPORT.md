# C-02-PAGE — пагинация истории канала (`nextCursor`/`hasMore`)

- **Воркер:** G2 Frontend, batch RW-40/PM, карточка **C-02-PAGE** (P1B).
- **Дата:** 2026-10-24. Офлайн: без live/окна/сети/cargo/gate/commit.
- **Источник:** `waves/RW-40/PARITY/G6-REPORT-RW40-02.md` §3.4;
  `G6-BATCH-MERGE-01.md` §«Можно после merge» п.3; `QUEUE.md` P1B.
  Контракт backend — `rust/gui-spike/src/data.rs::messages_page` (READ-ONLY).

## 1. Что сделано

`rust/gui-spike/ui/features/chat.js`

- **Первая страница** (`openChannel`): `messages_page({channelId, limit:100})` —
  `cursor` в аргументах **отсутствует**, `token` не передаётся. Из ответа
  сохраняется окно канала: `items`, `nextCursor`, `hasMore`, `total`, `pages`,
  `loading` (typedef `ChannelWindow`), `scroll` — по-прежнему per-channel.
- **Подгрузка** (`loadMoreHistory`): `messages_page({channelId, cursor: nextCursor,
  limit:100})` у нижнего края списка (`CHANNEL_LOAD_EDGE_PX = 96`). Guards:
  только реальный канал, `hasMore`, нет параллельного запроса (single-flight),
  курсор непустой, окно ≤ 500 строк (`CHANNEL_WINDOW_ROWS`, `data.rs
  UI_WINDOW_ROWS`). При `items.length >= 500` подгрузка останавливается и
  показывает `data-page-state="limit"`.
- **Склейка страниц** (`mergeChannelItems`, экспорт): дедуп по `id`
  (существующая строка побеждает — в ней может быть optimistic-правка), порядок
  backend (`timestamp`, затем `id` — как `collect_messages`). Дубли не
  создаются, счётчик `aria-setsize` обновляется у всех строк.
- **Позиция/якорь**: в обычном пути строки **дописываются** (`appendChannelRows`),
  уже показанные узлы не пересоздаются; `scrollTop` не сбрасывается. Для
  защитного пути «страница встала перед началом окна» (`prepended > 0`) —
  полный рендер + компенсация на `scrollHeight`-delta (в браузере сохраняет
  строку под курсором; в DOM-шиме layout не считается, delta = 0).
- **Остановка**: `hasMore=false` → подвал `end`, повторные скроллы не шлют
  запросы. Битый ответ «тот же `nextCursor` без новых строк» (`stalled`) тоже
  останавливает пагинацию — без цикла запросов.
- **Ошибки 404/401/429/прочее**: подвал `role="alert"` + текст
  `channelFailureText` (404 → «Канал не найден или недоступен», 401 → «Сессия
  истекла…»), кнопка «Повторить» повторяет **тот же** шаг; строки первой
  страницы остаются, список не превращается в пустой.
- **Подвал истории** — последний ребёнок `#chat-list` (не строка): в подсчёте
  строк/`aria-setsize`/геометрии 26/48/+22 не участвует. Состояние — в
  `data-page-state` (`idle|loading|error|end|limit`), а **не** в `data-state`:
  `#chat-list [data-state]` — контракт состояний канала C-02-UI (loading/
  empty/error), существующие тесты его считают.
- **Бенч-путь не тронут**: `CHAT_ROWS=100_000`, `SNAPSHOT_FLOORS`, виртуализация
  и `PageStore` не менялись; в бенч-режиме `onScrollHistory` выходит по
  `isChannelMode()`.

`rust/gui-spike/ui/components/chat.css` — блок `.chat-history-more`
(`idle/loading/error/limit`, `min-height` из токенов, retry-кнопка); строки
`.row`/токены/геометрия не менялись.

`rust/gui-spike/ui/app.js` — **не менялся** (пагинация живёт в chat-экране,
дерево каналов и `openChannel` уже связаны C-02-UI).

## 2. Red (red-first)

Тест `tests/unit/channel-paging.test.mjs` (new, 8 тестов) написан до кода.
Первый прогон — против **нереализованной** пагинации в рабочем дереве:

```
node --test tests/unit/channel-paging.test.mjs
exit=1
ℹ tests 8   ℹ pass 2   ℹ fail 6
  «нет #chat-list .chat-history-more» (первая страница/ошибки/лимит)
  «подгрузка не сделала второй запрос истории»
  «подгрузка без новых строк не остановилась»
```

Повторяемый red (копия UI без C-02-PAGE-обвязки, сдаваемые файлы не
мутировались: `GUI_DOM_APP=/tmp/opencode/c02p-red/ui/app.js`) — 6/8 fail,
лог `red.log`.

## 3. Green

```
node --test tests/unit/channel-paging.test.mjs
exit=0   ℹ tests 8   ℹ pass 8   ℹ fail 0        (green.log)

node --test <12 файлов фронтенда> tests/unit/channel-paging.test.mjs
exit=0   ℹ tests 368   ℹ pass 368   ℹ fail 0     (suite.log)

node --test "tests/unit/*.test.mjs"
exit=0   ℹ tests 651   ℹ pass 651   ℹ fail 0     (units.log)
```

Было 360 фронтенд-тестов (baseline выше 351), +8 новых; существующие не
менялись и не ослаблялись.

Покрытие (оракул — READY + `MESSAGE_CREATE` кадра `1163408591315812392` +
`data.rs::messages_page`, не реализация):

1. Первая страница: один запрос, аргументы ровно `{channelId, limit:100}`,
   без `cursor`/`token`, строки из оракула, `hasMore=true` → подвал `idle`,
   `snapshot.chatRows=100_000`.
2. Вторая страница по `nextCursor`: аргументы `{channelId, cursor, limit:100}`;
   порядок строк = оракул; `hasMore=false` → `end`; повторные скроллы не шлют
   запросов.
3. Дедуп: перекрывающаяся страница (повтор id) не создаёт вторую строку.
4. Якорь/позиция: `scrollTop` не сбрасывается, якорный узел — тот же DOM-узел,
   id не сместился.
5. Битый курсор (`hasMore=true`, тот же `nextCursor`, только дубли) — ровно
   один повторный запрос, дальше тишина.
6. Ошибки 404 и 401 второй страницы: подвал `role="alert"`, видимый текст,
   строки первой страницы целы, `aria-busy=false`, retry догружает страницу.
7. Бенч: скролл синтетического окна не вызывает `messages_page`, `chatRows`
   100 000.
8. Окно: 5 страниц/500 строк — предел, `data-page-state="limit"`, шестой запрос
   не уходит.

## 4. Проверки

```
npx tsc -p rust/gui-spike/ui/tsconfig.json   → exit=0 (0 ошибок, tsc.log)
node waves/RW-39/PM/w39c/geometry-check.mjs  → exit=0, 31/31 в допуске (geometry.log)
```

`cargo`/`gate`/live/окно/сеть/commit — не запускались (запрещены ТЗ).

## 5. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/ui/features/chat.js` | `f4c8076386755a76efce9166dba71e50` |
| `rust/gui-spike/ui/components/chat.css` | `4de23215973553fa53470eebf2542842` |
| `rust/gui-spike/ui/app.js` (не менялся) | `78039af36b09fa94e5d92b207f5ce977` |
| `tests/unit/channel-paging.test.mjs` (new) | `5d98e96080c82fb2e8b8588ea10582c7` |

`src/**`, `rust/gui-spike/src/**`, `index.html`, `actions.js`, `reactions.js`,
`typing-indicator.js`, `model.js`, `virtual-list.js`, `tests/support/gui-dom-harness.mjs`,
`logs/`, `functions/`, `evidence/`, `waves/db.jsonl`, `STATE.md`, `TASKS.md` —
не трогались.

## 6. Остаток / риски

- **Направление курсора — расхождение с формулировкой ТЗ (нужно решение PM).**
  Backend (`data.rs::cursor_start`) отдаёт строки **строго после** курсора, т.е.
  `messages_page` листает историю **вперёд по времени**, а `nextCursor` — id
  последней строки страницы. Поэтому «ещё» — это следующая (более новая)
  страница, и подгрузка висит на **нижнем** крае. Требование «скролл вверх /
  `nextCursor` грузит предыдущую» на этом контракте невыполнимо: более старых
  строк, чем первая страница, backend не отдаёт (нет `before`-курсора).
  Варианты: (a) оставить как есть (UI читает историю от начала к концу);
  (b) карточка G1 на `before`-курсор в `data.rs` (вне матрицы G2, `src/**`
  READ-ONLY), тогда «вверх» включится одной веткой — компенсация prepend уже
  реализована и покрыта якорным тестом (в шиме delta = 0).
- **Открытие на последнем сообщении** (пункт G6 backlog) не сделано: первая
  страница — начало истории. Для tail-режима нужно либо `before`-курсор, либо
  проход по всем страницам при открытии (до 5 запросов) — отдельная карточка.
- **Предел 500 строк** останавливает подгрузку, а не вытесняет старые строки:
  вытеснение сверху сдвигает позицию и требует отдельной политики.
- **Live не проверялся** (окно/live/сеть запрещены): формы `MessagesPage` и
  поведение курсора взяты из `data.rs` и фикстур.
- Подвал истории намеренно не использует `data-state` и не является строкой —
  иначе ломались бы контракт состояний C-02-UI и подсчёт `aria-setsize`.
- Мутации/приёмка/`waves/db.jsonl`/`STATE.md` — за Team Lead.
