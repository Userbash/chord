# TASK-227 (FE-P1-23) — UI контроллер контекстного меню и клавиатурной навигации для сообщений, каналов и пользователей (`context-menu.js`, G2)

- **Карточка / Ветка**: `task-227` (`feature/task-227-ui-context-menu`)
- **Отдел**: G2 Frontend (WebView UI & DOM/A11y)
- **Оракулы**: `reference-oracle/frontend-d03.md` (§5 п.3–4 Menu / ContextMenu: `menu_c1e9c4`, `role="menu"`, `role="menuitem"`, `colorDanger`, `CONTEXT_MENU_OPEN/CLOSE`), `reference-oracle/ui/01-msg-menu-items.json`, `01-msg-menu.html`.

## Сделано
1. `tests/unit/context-menu.test.mjs` (Red → Green, 10 тестов в 3 suites, `// @ts-check` + JSDoc, минимальный DOM-шим):
   - проверка `buildContextMenuItems({ targetType, isOwnMessage, isPinned, isMuted, canManageMessages })`:
     - для `targetType === "message"`: базовый набор (`reply`, `copy-text`, `copy-id`), условное включение `pin` («Открепить сообщение» / «Закрепить сообщение») при `canManageMessages`, `edit` («Изменить сообщение») при `isOwnMessage`, `delete` («Удалить сообщение», `danger: true`) при `isOwnMessage || canManageMessages`;
     - для `targetType === "channel"`: `mark-read` («Отметить как прочитанное»), `mute-channel` («Включить уведомления канала» при `isMuted: true` и «Заглушить канал» при `isMuted: false`), `copy-id` («Скопировать ID канала»);
     - для `targetType === "user"`: `profile` («Профиль»), `message-user` («Написать сообщение»), `copy-id` («Скопировать ID пользователя»);
     - возврат пустого массива `[]` для неизвестного `targetType`;
   - проверка `clampMenuCoordinates({ x, y, menuWidth, menuHeight, viewportWidth, viewportHeight })`:
     - зажатие `x` в диапазон `[0, Math.max(0, viewportWidth - menuWidth - 8)]` и `y` в `[0, Math.max(0, viewportHeight - menuHeight - 8)]`, включая отрицательные координаты и узкий viewport;
   - проверка `createContextMenuController({ container, onSelectAction })`:
     - рендер контейнера `[data-context-menu]` с `role="menu"` и пунктов `[data-context-item]` с `role="menuitem"` (и `data-danger="true"` для опасных пунктов);
     - открытие через `open({ x, y, targetType, targetId, ...flags })`, циклическая навигация `moveFocus(delta)` и клавишами `ArrowDown` / `ArrowUp` / `Home` / `End`, активация через `activateFocusedItem()`, `Enter` и клик мыши с вызовом `onSelectAction`, закрытие по `Escape` и `close()`.
2. `rust/gui-spike/ui/features/context-menu.js` (`// @ts-check` + JSDoc, ESM):
   - экспортированы `buildContextMenuItems`, `clampMenuCoordinates` и `createContextMenuController`.

## Прогоны (Red / Green / Targeted / Gate)
- **Red**: `node --test tests/unit/context-menu.test.mjs` → `tests 1, pass 0, fail 1` (`ERR_MODULE_NOT_FOUND`), лог сохранён в `/home/user/orca/project/waves/RW-40/G2/task-227-ui-context-menu/red.log`.
- **Green**: `node --test tests/unit/context-menu.test.mjs` → `tests 10, suites 3, pass 10, fail 0`, лог сохранён в `/home/user/orca/project/waves/RW-40/G2/task-227-ui-context-menu/green.log`.
- **Typecheck (targeted)**: `npx tsc --ignoreConfig --noEmit --strict --allowJs --checkJs --target es2022 --module node20 --moduleResolution node16 --types node rust/gui-spike/ui/features/context-menu.js tests/unit/context-menu.test.mjs` — `0 errors`.
- **Gate**: полный `gate:worker` не запускался (режим ТЗ карточки); чужие файлы не затронуты.

## Файлы и MD5
```
0b0dac971c189f3cb1892f30fd2f23ce  rust/gui-spike/ui/features/context-menu.js
b7b59792189270ef46d19b69746a9a38  tests/unit/context-menu.test.mjs
e2dd10f21d3e129c163decfd2eb6d0cb  /home/user/orca/project/waves/RW-40/G2/task-227-ui-context-menu/red.log
0dc93127ebb7683028dbde8634aaf831  /home/user/orca/project/waves/RW-40/G2/task-227-ui-context-menu/green.log
```

## Риски
- Монтаж `createContextMenuController` в общий shell (`app.js` / `chat.js`) находится вне write-scope данной карточки и выполняется отдельной интеграционной задачей.
- `git commit` не выполнялся согласно правилам воркера.
