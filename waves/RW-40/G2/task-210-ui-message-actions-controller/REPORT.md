# TASK-210 — UI контроллер панели действий сообщения и модального окна пересылки (`message-actions.js`, G2)

- **Карточка / Ветка**: `task-210` (`feature/task-210-ui-message-actions-controller`)
- **Отдел**: G2 Frontend (WebView UI & DOM/A11y)
- **Оракулы**: `reference-oracle/ui/01-msg-menu-items.json`, `reference-oracle/frontend-d03.md` (§5 Modal / Toolbar / Menu).

## Сделано
1. `tests/unit/message-actions.test.mjs` (Red → Green, 9 тестов в 3 suites, `// @ts-check` + JSDoc):
   - проверка заморозки и состава `QUICK_REACTION_EMOJIS` (`['👍', '❤️', '😂']`) и константы `MAX_FORWARD_DESTINATIONS = 5`;
   - проверка `renderActionBar(message, context = { currentUserId, canManageMessages })`:
     - рендер кнопок быстрых реакций из `QUICK_REACTION_EMOJIS` с вызовом `onReact(message.id, emoji)`, кнопки `Ответить` с вызовом `onReply(message.id)` и кнопки `Переслать`;
     - рендер кнопки `Изменить` ТОЛЬКО при `message.authorId === context.currentUserId` с вызовом `onEdit(message.id)`;
     - рендер кнопки `Закрепить сообщение` (или `Открепить сообщение` при `message.pinned === true`) ТОЛЬКО при `context.canManageMessages === true` с вызовом `onTogglePin(message.id, !Boolean(message.pinned))`;
     - рендер кнопки `Удалить сообщение` при `message.authorId === context.currentUserId || context.canManageMessages === true` с вызовом `onDelete(message.id)`;
   - проверка `openForwardModal(message, channels)`:
     - рендер диалога `Переслать сообщение` (`role="dialog"`, `aria-modal="true"`) с полем фильтрации каналов (`[data-forward-filter]`), чекбоксами каналов-получателей с потолком `MAX_FORWARD_DESTINATIONS` (`5`), полем необязательного комментария (`[data-forward-comment]`) и кнопкой отправки `Отправить` (`onForward(message.id, selectedChannelIds, comment)`);
     - проверка блокировки остальных чекбоксов при выборе 5 каналов, разблокировки при снятии выбора, регистронезависимой фильтрации каналов с сохранением уже выбранных получателей и защиты от пустой отправки (0 каналов).
2. `rust/gui-spike/ui/features/message-actions.js` (`// @ts-check` + JSDoc):
   - экспортированы `QUICK_REACTION_EMOJIS`, `MAX_FORWARD_DESTINATIONS` и `createMessageActionsController(containerEl, options)`.

## Прогоны (Red / Green / Targeted / Gate)
- **Red**: `node --test tests/unit/message-actions.test.mjs` → `tests 1, pass 0, fail 1` (`ERR_MODULE_NOT_FOUND`), лог сохранён в `/home/user/orca/project/waves/RW-40/G2/task-210-ui-message-actions-controller/red.log`.
- **Green**: `node --test tests/unit/message-actions.test.mjs` → `tests 9, suites 3, pass 9, fail 0`, лог сохранён в `/home/user/orca/project/waves/RW-40/G2/task-210-ui-message-actions-controller/green.log`.
- **Typecheck (targeted)**: проверка `rust/gui-spike/ui/features/message-actions.js` и `tests/unit/message-actions.test.mjs` через `npx tsc -p tsconfig.json --noEmit` — `0 errors`.
- **Gate**: полный `gate:worker` не запускался (режим ТЗ карточки); чужие файлы не затронуты.

## Файлы и MD5
```
ca5fac6fb925cc5b83df26801e7242de  rust/gui-spike/ui/features/message-actions.js
54b3f0d853b3cd2a63013f8a4ac33451  tests/unit/message-actions.test.mjs
64f3dc82f61a9cd6a0541d78fa1c670d  /home/user/orca/project/waves/RW-40/G2/task-210-ui-message-actions-controller/red.log
12f83be3abf7fc86b1b9b6a46368dc31  /home/user/orca/project/waves/RW-40/G2/task-210-ui-message-actions-controller/green.log
```

## Риски
- Монтаж контроллера в общий shell (`app.js` / `chat.js`) находится вне write-scope данной карточки и выполняется в интеграционной задаче.
- Ветка не коммитилась и не пушилась согласно правилам воркера.
