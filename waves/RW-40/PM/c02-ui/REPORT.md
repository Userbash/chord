# C-02-UI — реальные каналы из `channels(guildId)`, переключение + история

- **Воркер:** G2 Frontend, batch RW-40/PM, карточка **C-02-UI** (P0).
- **Дата:** 2026-10-24. Офлайн: без live/окна/сети/cargo/gate/commit.
- **Источник:** `waves/RW-40/PARITY/G6-REPORT-RW40-02.md` §3.4, дефект
  **G6-D-01** (`channelId:"news"`/`messageId=index` в live send/edit/delete/reply);
  backend `channels(guildId?)` принят в C-02-BE (`waves/RW-40/PM/c02-be/REPORT.md`),
  `messages_page({channelId,cursor?,limit?})` уже был.

## 1. Что сделано

`rust/gui-spike/ui/app.js`
- `createChannelTree`: `#channel-tree` → `invoke("channels", { guildId })` при
  выборе гильдии (rail `onSelect`, тот же порядок, что READY). Дерево — группы
  **по типам** (0 текст, 5 объявление, 15 форум, 2 голос, 4 категория; неизвестный
  тип — отдельная группа).
- Плитка `.channel-item[data-channel-id][data-channel-type][data-openable]`;
  `0/5` открывают чат, `2/4/15` — `aria-disabled="true"`, видимая причина
  (`.channel-reason` + live-регион `.channel-note`), `title`; клик не трогает чат.
- Состояния дерева: `data-state=loading|ready|empty|error`, `aria-busy`;
  ошибка — `role="alert"` + `.channel-tree-retry` (повтор `channels()`).
- Клавиатура: roving tabindex, ↑/↓/Home/End, Enter/Space открывает канал;
  `aria-current="page"` ровно на одном канале.
- Reply из композера: `channelId` берётся из `chat.channelId` (не `"news"`).

`rust/gui-spike/ui/features/chat.js`
- `openChannel({id,name})` → `messages_page({channelId: realId, limit: 100})`;
  `#chat-list` перерисовывается **реальной** страницей (`data-mode="channel"`,
  `data-channel-id`), синтетический sizer снимается (бенч-путь не ослаблен и
  остаётся режимом до первого реального канала).
- `#chat-header .chat-title-text` = имя канала, `#chat-header[data-channel-id]`.
- Per-channel окно/позиция: `Map<channelId,{items,scroll}>`; возврат в канал не
  перезапрашивает историю и восстанавливает `scrollTop`; пустой канал тоже
  кешируется.
- Видимые состояния `#chat-list`: `loading` (`role=status`, `aria-busy`),
  `empty`, `error` (`role="alert"` + повтор). Тексты 401 → «Сессия истекла…»,
  404/NotFound → «Канал не найден или недоступен», 429 → «Слишком много запросов».
- Строки реального канала: `data-message-id` = snowflake `items[].id`,
  `author/globalName`, `timestamp`, `group`, `(изменено)`, day-divider;
  структура `.author` + `.text` (`.text-body` первым) как у §7.1.
- **G6-D-01:** send/edit/delete/reply/reaction-панель используют реальный
  `channelId` и реальный `messageId` (`messageIdOf`, `onEdit`, `onDelete`), а не
  индекс. Бенч-режим сохраняет `BENCH_CHANNEL_ID="news"` (нужен существующим
  тестам синтетического окна).
- В реальном канале кнопки быстрых реакций скрыты: `reactions.js` вне матрицы
  файлов и привязан к bench-id; ложный запрос в чужой канал не отправляется.

`rust/gui-spike/ui/index.html` — `#channel-tree[data-state="idle"][aria-busy]` +
комментарий: статичная разметка — плейсхолдер до `channels()`, дерево собирает JS.
`rust/gui-spike/ui/components/chat.css` — только tree/header-часть C-02:
`.channel-note`, `.channel-reason`, состояние/ошибка дерева (стили `tree.css` и
`header.css` заморожены; строки истории используют уже существующие `.row`/`.text`).
`rust/gui-spike/ui/features/actions.js` — не менялся (клавиатура дерева живёт в
`app.js`, один код-путь с DOM-шимом).

Бенч/порог не ослаблены: `CHAT_ROWS=100_000`, `SNAPSHOT_FLOORS` не трогались;
новый тест дополнительно проверяет `snapshot.chatRows === 100_000`.

## 2. Red (red-first)

Тест `tests/unit/channel-switch.test.mjs` написан до кода. Red-прогон против копии
UI с отключённой C-02-обвязкой (`GUI_DOM_APP`, сдаваемые файлы не мутировались):

```
GUI_DOM_APP=/tmp/opencode/c02-red-ui/app.js node --test tests/unit/channel-switch.test.mjs
exit=1
ℹ tests 10   ℹ pass 0   ℹ fail 10
  «при выборе гильдии channels() не вызван ровно один раз»
  «в дереве нет канала 1163408591315812392»
  «смена гильдии не вызвала channels() второй раз»
  «ошибка channels() не видна в дереве»
```

Лог: `red.log` (10/10 fail). Первый red-прогон (до реализации) — те же 10/10 fail.

## 3. Green

```
node --test <8 файлов фронтенда> tests/unit/channel-switch.test.mjs
exit=0
ℹ tests 319   ℹ pass 319   ℹ fail 0
```

- 309 существующих фронтенд-тестов не менялись и не ослаблялись;
- +10 новых (`channel-switch.test.mjs`).

Targeted:

```
node --test tests/unit/channel-switch.test.mjs
ℹ tests 10   ℹ pass 10   ℹ fail 0
```

Покрытие нового теста (оракул — READY + `MESSAGE_CREATE`, не реализация):
1. `channels({guildId})` при выборе гильдии; 55 каналов = READY; порядок групп и
   `position/id`; типы/имена; `0/5` `data-openable=true`, `2/4/15` — false с
   причиной; клики 2/4/15 не вызывают `messages_page`; клик 0 → заголовок +
   реальные 3 строки (`data-message-id` из кадров); `CHAT_ROWS=100_000`.
2. Смена гильдии → второй `channels({guildId})`, дерево новой гильдии.
3. Per-channel окно/позиция: A(scroll=120) → B(empty) → A (без повторного IPC,
   `scrollTop=120`), повторный вход в B тоже без IPC.
4. **G6-D-01:** `send_message{channelId:real}`, `edit_message{channelId,messageId:
   real,content}`, `delete_message{channelId,messageId:real}`, `reply_message
   {channelId,content,replyTo:real}`; ни один вызов не `"news"`/индекс.
5. `loading` (`aria-busy=true`, `role=status`) → реальные строки.
6. `empty`-канал → видимое empty-состояние, без чужих строк.
7. 404/401 → `role="alert"`, видимый текст, строк нет, `aria-busy=false`,
   заголовок сохранён.
8. Клавиатура: ↓ переносит фокус, Enter открывает канал, `aria-current` один.
9. Ошибка `channels()` → alert + retry → дерево ready.

## 4. Проверки

```
npx tsc -p rust/gui-spike/ui/tsconfig.json   → exit=0 (0 ошибок)
node waves/RW-39/PM/w39c/geometry-check.mjs  → exit=0, все 31 в допуске
```

Логи: `tsc.log`, `geometry.log`, `green.log`.

## 5. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/ui/app.js` | `407cd3d92201d330e2e465075d8ea517` |
| `rust/gui-spike/ui/features/chat.js` | `a1bc4cb55a72411e77fd60e1f96c6eec` |
| `rust/gui-spike/ui/index.html` | `7ba16d9a9f129a472dd5ab9518e5d597` |
| `rust/gui-spike/ui/components/chat.css` | `f6577730e745fce3bcebeb38a6a88356` |
| `rust/gui-spike/ui/features/actions.js` (не менялся) | `1d792ba63d8c75344fe20e2c7e9ffd2f` |
| `tests/unit/channel-switch.test.mjs` (new) | `0fc9b44a20f0dbe3df626b2e8f8f8cf4` |

`src/**`, `search-overlay.js`, `voice.js`, `settings.js`, `tree.css`, `header.css`,
`tests/support/gui-dom-harness.mjs`, `logs/`, `functions/`, `evidence/`,
`waves/db.jsonl`, `STATE.md`, `TASKS.md` — не трогались.

## 6. Остаток / риски

- **Live не проверялся** (окно/live/сеть запрещены): реальные id взяты из
  `MESSAGE_CREATE` фикстур, IPC-форма — по `data.rs`.
- **История — первая страница** (`limit=100`, `hasMore/nextCursor` не
  догружаются; открытие на «последнем сообщении» для длинных каналов — P1-карточка).
- **QuickSwitcher** (`search-overlay`/`quick-switcher` — read-only для этой
  карточки) по-прежнему выбирает `.channel-item`/заголовок, но не вызывает
  `openChannel` → `messages_page`; канал откроется только кликом в дереве.
- **Реакции** (`reactions.js:109`) остаются на bench-id `"news"`; в реальном
  канале кнопки скрыты, пилюль в реальных строках нет — отдельная карточка
  (файл вне матрицы C-02-UI).
- Возврат в синтетический бенч-режим после открытия реального канала не
  предусмотрен: синтетика остаётся стартовым/бенч-путём и в UI_SNAPSHOT.
- Категории показаны отдельной группой; вложенность по `parentId` в дереве не
  строится (порядок и `data-parent-id` сохранены).
- Ошибка `channels()` видна в дереве и не уходит в общий `.error-state`
  (чтобы не перекрывать ошибки действий); повтор — кнопкой дерева.
