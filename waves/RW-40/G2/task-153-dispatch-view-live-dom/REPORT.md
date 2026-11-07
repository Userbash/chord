# task-153 — Подключение живых проекций B18..B23 к DOM-отрисовке в `dispatch-view.js`

- **batch/карточка:** RW-40 / G2 Frontend / `task-153` (FE-P1-01: подключение живых проекций B18..B23 из `dispatch-model.js` к DOM-отрисовке в `rust/gui-spike/ui/features/dispatch-view.js`).
- **worktree:** `/home/sanya/orca/workspaces/project/feature-task-153-dispatch-view-live-dom`.
- **файлы (write-scope):**
  - `rust/gui-spike/ui/features/dispatch-view.js`;
  - `tests/unit/dispatch_ui.test.mjs`;
  - отчёт и логи в `/home/sanya/orca/project/waves/RW-40/G2/task-153-dispatch-view-live-dom/`.

## Сделано

1. **Синхронизация актуальной базы:** перед стартом скопированы актуальные версии файлов из `/home/sanya/orca/project` (PR-22 и PR-31: `dispatch-model.js`, `dispatch-view.js`, `dispatch_ui.test.mjs`).
2. **Red-тесты (Strict TDD):**
   - Написаны падающие тесты `C17–C22` в секции `L-02-UI: интеграция dispatch → DOM` файла `tests/unit/dispatch_ui.test.mjs`:
     - `C17`: живой `MESSAGE_CREATE` для активного канала добавляет новую строку сообщения в `#message-list` / `#chat-list` без перезагрузки истории (`callsOf("messages_page")` не растёт), а для неактивного канала увеличивает `unread`/`mention` индикатор в дереве каналов.
     - `C18`: живой `MESSAGE_UPDATE` обновляет текст сообщения и плашку `(изменено)` в `#message-list` / `#chat-list`.
     - `C19`: живой `MESSAGE_ACK` очищает `unread`-индикатор канала в `#channel-list`.
     - `C20`: живой `TYPING_START` показывает/обновляет индикатор печатающего пользователя (`#typing-indicator` и атрибут `data-typing-users` на `#composer` / `#chat-view`) и очищает его при приходе `MESSAGE_CREATE` от этого автора.
     - `C21`: живой `VOICE_STATE_UPDATE` / `VOICE_SERVER_UPDATE` обновляет счётчик/индикатор голосовых участников, статус стрима (`data-stream-status`, `data-self-stream`) и endpoint голосового сервера в DOM.
     - `C22`: живой `THREAD_LIST_SYNC` обновляет счётчик активных тредов гильдии/канала в DOM (`[data-action="threads"]`, `#channel-tree`, `.channel-item[data-thread-count]`).
   - Падение всех 6 новых тестов зафиксировано в `red.log` (51 pass, 6 fail, exit 1).
3. **Реализация DOM-отрисовки в `rust/gui-spike/ui/features/dispatch-view.js`:**
   - Реализована обработка вариантов изменений `message_create`, `message_update`, `message_ack`, `typing`, `voice_state`, `voice_server`, `threads` внутри `apply()`.
   - Подключена функция `renderChannelMessage` из `chat.js` для единообразного рендеринга строк живых сообщений.
   - Реализовано обновление индикатора `is-unread` и `data-mention-count` на плитках каналов в дереве при входящих сообщениях и их сброс по `MESSAGE_ACK`.
   - Реализовано управление индикатором тайпинга (`#typing-indicator`, `data-typing-users`) с автоматическим погашением при приходе `MESSAGE_CREATE`.
   - Реализовано отображение голосовых участников и статуса стрима/endpoint на `#voice-panel` и плитках каналов.
   - Реализовано обновление счётчиков активных тредов на кнопке `[data-action="threads"]`, дереве каналов и родительских каналах.
   - Сохранены все инварианты `C1–C16` и `B1–B23`.

## red (до реализации)

`node --test tests/unit/dispatch_ui.test.mjs` → exit 1 (`red.log`, 51 pass, 6 fail):
- `C17`: `новая строка сообщения не добавлена в #message-list / #chat-list` (1 !== 2);
- `C18`: `текст сообщения не обновился` ('строка для live-реакций' !== 'обновлённый текст сообщения');
- `C19`: `подготовка: канал не стал unread` (false !== true);
- `C20`: `индикатор печати не отобразился` (false !== true);
- `C21`: `счётчик голосовых участников не обновился до 1` (undefined !== '1');
- `C22`: `счётчик активных тредов не обновился до 1` (undefined !== '1').

## green

- `node --test tests/unit/dispatch_ui.test.mjs` → **57 passed, 0 failed, 0 skipped**, exit 0 (`green.log`).
- `node --test 'tests/unit/**/*.test.mjs'` → **716 passed, 0 failed, 0 skipped**, exit 0.
- `npx tsc -p tsconfig.json` → exit 0 (0 ошибок).
- `npx tsc -p rust/gui-spike/ui/tsconfig.json` → exit 0 (0 ошибок).

## Файлы + md5

| файл | md5 |
|---|---|
| `rust/gui-spike/ui/features/dispatch-view.js` | `17582e7a211b45d3b35866e64d46e0d0` |
| `tests/unit/dispatch_ui.test.mjs` | `b799592654fe99ef371d20a548b28681` |

Логи: `red.log`, `green.log`.

## Риски / заметки

1. **Write-scope:** затронуты только назначенные файлы `rust/gui-spike/ui/features/dispatch-view.js` и `tests/unit/dispatch_ui.test.mjs`.
2. **Изоляция и производительность:** добавление сообщений не пересобирает виртуализованный `#chat-list` и не вызывает повторных запросов IPC (`messages_page`).
3. **Безопасность:** секреты (токены/сессии) не попадают в DOM и отфильтрованы на уровне модели.
