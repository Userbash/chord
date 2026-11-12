# Отчёт по карточке task-149 (FE-P0-04): Монтаж `createReactionsController` в `chat.js` и интерактивные реакции

- **Волна / Отдел / Карточка**: `RW-40` / `G2 Frontend (WebView UI & DOM/A11y)` / `task-149` (`FE-P0-04` из `waves/RW-40/PM/BACKLOG-RE-FRONTEND.md`, спецификация `waves/RW-40/DESIGN/REACTIONS-EXTENDED-SPEC.md`)
- **Ворктри**: `/home/user/orca/workspaces/project/feature-task-149-mount-reactions-chat`
- **Статус**: Выполнено (Strict TDD: Red → Green → Mutation Kill 5/5)

## 1. Сделано

1. **Контроллер реакций (`rust/gui-spike/ui/features/reactions.js`)**:
   - Экспортированы `createReactionsController(options)`, константы типов реакций Discord API (`REACTION_TYPE_NORMAL = 0`, `REACTION_TYPE_BURST = 1`) и замороженный каталог `REACTION_CATEGORIES` из 9 категорий (`Часто используемые`, `Люди`, `Природа`, `Еда`, `Активности`, `Путешествия`, `Предметы`, `Символы`, `Флаги`).
   - Сохранены без изменений все существующие экспорты и инварианты (`configureReactionTransport`, `channelReactionKey`, `reactionsFor`, `toggleReaction`, `requestReaction`, `adjustReactionOverride`, `resetReactions`).
   - Расширен `requestReaction` и реализован `bindMessageRow(row, meta)`:
     - клик по пилюле `.reaction` переключает реакцию (`toggle_reaction` / `onToggleReaction` → `add_reaction` / `remove_reaction` с поддержкой `burst = false | true` и `type = 0 | 1`), оптимистично обновляет счётчик `.reaction-count`, атрибуты `data-me`, `aria-pressed`, `data-burst` и класс `.active`, а при отказе IPC (`401`/`403`/`429`/сеть) откатывает состояние;
     - повторная привязка `bindMessageRow` при перерисовке строки предварительно очищает старые обработчики, исключая дублирование вызовов.
   - Интегрирована всплывающая панель действий над сообщением (`.message-action-toolbar` / `.msg-hover-toolbar`):
     - расширяет существующий `.msg-toolbar` в `.chat-main` (сохраняя инвариант единственного `.msg-toolbar` из 15 кнопок R-16) либо создаёт панель в standalone-режиме;
     - поддерживает быстрые реакции (`[data-command="react"]`), кнопку открытия 9-категорийного пикера (`[data-command="add-reaction"]` с `aria-haspopup="dialog"`, `aria-expanded="false|true"`, `aria-controls="reaction-picker-popover"`) и кнопку ответа (`[data-command="reply"]`).
   - Реализован 9-категорийный поповер выбора реакции `#reaction-picker-popover` (`role="dialog"`, `aria-modal="false"`, lazy mount / lazy destroy):
     - префиксные `id` (`#reaction-picker-search`, `#reaction-picker-tabs`, `#reaction-picker-tab-0..8`, `#reaction-picker-grid`), чтобы не конфликтовать со стационарным экраном `#screen-emoji` и сохранять `document.querySelectorAll(".tab").length === 10` в закрытом состоянии;
     - переключение между 9 категориями, глобальный поиск по категориям и эмодзи, переключатель супер-реакции (`[data-action="toggle-burst"]`), закрытие по выбору эмодзи, кнопке `[data-action="reaction-picker-close"]` и `Escape` с возвратом фокуса.
   - Реализована модалка «Кто поставил реакцию» (`#reaction-users-modal`, `role="dialog"`, `aria-modal="true"`), открываемая по `contextmenu` на пилюле `.reaction` или через `openReactionUsersModal`:
     - вкладки `Обычные` (`role="tab"`, `data-reaction-type="0"`, `type=0`) и `Супер-реакции` (`role="tab"`, `data-reaction-type="1"`, `type=1`);
     - загрузка и рендер списка пользователей (`#reaction-users-list`, `role="list"`, элементы `role="listitem"`) через `get_reaction_users` / `onFetchReactionUsers`, закрытие по `[data-action="reaction-users-close"]` и `Escape`.
   - Реализована полная очистка (`destroy()` / `dispose()`): снятие всех обработчиков строк и кнопок и удаление `#reaction-picker-popover` / `#reaction-users-modal` из DOM.

2. **Монтирование в `rust/gui-spike/ui/features/chat.js`**:
   - В `reactionPill` и `paintPill` добавлена синхронизация класса `.active` (`reaction.me ? "reaction active" : "reaction"`, `pill.classList.toggle("active", next.me)`).
   - В `renderChatRow` и `renderChannelMessage` подключены поддержка `Shift+Click` (`burst=true`) и открытие `#reaction-users-modal` по `contextmenu` на пилюле `.reaction` при сохранении инварианта `row.children.length === 2` (`.author` + `.text`).
   - В `createChatScreen` смонтирован `createReactionsController`, экспортированы `reactionsController` и `destroyReactions()`.
   - Файлы `rust/gui-spike/ui/app.js` и `rust/gui-spike/ui/index.html` не затрагивались (соседний слот `task-148`).

3. **Тесты (`tests/unit/reactions-ui.test.mjs`, `tests/unit/gui-spike-dom.test.mjs`)**:
   - Создан `tests/unit/reactions-ui.test.mjs` (5 поведенческих тестов) и добавлен интеграционный тест в `tests/unit/gui-spike-dom.test.mjs`.

## 2. Red-прогон

- Команда: `node --test tests/unit/reactions-ui.test.mjs tests/unit/gui-spike-dom.test.mjs`
- Лог: `/home/user/orca/project/waves/RW-40/G2/task-149-mount-reactions-chat/red.log`
- Результат: **EXIT_CODE=1** (6 падений в новых тестах до реализации `createReactionsController` и его монтажа в `chat.js`).

## 3. Green-прогон и Typecheck

- Команда: `node --test tests/unit/reactions-ui.test.mjs tests/unit/gui-spike-dom.test.mjs && npx tsc -p tsconfig.json`
- Лог: `/home/user/orca/project/waves/RW-40/G2/task-149-mount-reactions-chat/green.log`
- Результат:
  - `tests/unit/reactions-ui.test.mjs` + `tests/unit/gui-spike-dom.test.mjs`: **44/44 pass, 0 fail, 0 skip**.
  - `npx tsc -p tsconfig.json` и `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit`: **0 ошибок (exit 0)**.
  - Смежные наборы (`tests/unit/chat-reactions.test.mjs`, `tests/unit/gui-spike-features.test.mjs`, `tests/unit/dispatch_ui.test.mjs`, `tests/meta/ipc-path.test.mjs`, `tests/meta/test-policy.test.mjs`): **233/233 pass, 0 fail**.

## 4. Мутационное тестирование

- Лог: `/home/user/orca/project/waves/RW-40/G2/task-149-mount-reactions-chat/mutations.log`
- Проверено 5 целевых мутантов (`M1` — снятие переключения `.active` на пилюле; `M2` — подмена `type=1` на `type=0` при `burst=true`; `M3` — отключение lazy-destroy `#reaction-picker-popover` в `closePicker`; `M4` — поломка переключения вкладки `Супер-реакции` `type=1` в `#reaction-users-modal`; `M5` — отключение монтирования `createReactionsController` в `chat.js`).
- Результат: **5/5 мутантов убиты (KILLED)**, контрольные суммы MD5 файлов полностью восстановлены (`match=true`).

## 5. Файлы и MD5

| Файл | Статус | MD5 |
|---|---|---|
| `rust/gui-spike/ui/features/chat.js` | modified | `55b467842ef33af9d8bfeb8547ce6c9f` |
| `rust/gui-spike/ui/features/reactions.js` | modified | `aa45fe323a0844ed4f675ffe42b0307d` |
| `tests/unit/reactions-ui.test.mjs` | created | `8b814efb01b8305c0b35523e4608734e` |
| `tests/unit/gui-spike-dom.test.mjs` | modified | `018c247adbce6eec1f903c966833b905` |
| `/home/user/orca/project/waves/RW-40/G2/task-149-mount-reactions-chat/red.log` | created | `cdb1a5f27900ac4918a0b71bf12682d1` |
| `/home/user/orca/project/waves/RW-40/G2/task-149-mount-reactions-chat/green.log` | created | `9876b1fbc9a3a11f397144c39ca85c93` |
| `/home/user/orca/project/waves/RW-40/G2/task-149-mount-reactions-chat/mutations.log` | created | `5d041d58c176cc42cb9d18dc14b2a942` |

## 6. Риски и заметки

- Все инварианты DOM (`row.children.length === 2`, единственный `.msg-toolbar` в `.chat-main`, 10 вкладок `.tab` на `#screen-emoji` благодаря lazy destroy `#reaction-picker-popover`) полностью соблюдены.
- `innerHTML` / `outerHTML` / `insertAdjacentHTML` не используются; все узлы создаются через `document.createElement` и `textContent`.
