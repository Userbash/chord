# task-208 — UI threads drawer controller (`threads-drawer.js`)

Batch: RW-40 / G2. Ветка: `feature/task-208-ui-threads-drawer-controller`.

## Сделано
- `tests/unit/threads-drawer.test.mjs` — red-first unit-тесты (11 тестов, 4 сьюта) на `AUTO_ARCHIVE_DURATIONS_RU` и `createThreadsDrawerController`: вкладки `Активные ветки` / `Архивные ветки`, поле поиска с фильтрацией, пустое состояние `Ветки не найдены`, карточка ветки с подписью автоархивации и кнопкой вступления/выхода, открытие split-pane ветки и валидация/создание новой ветки.
- `rust/gui-spike/ui/features/threads-drawer.js` — реализация `AUTO_ARCHIVE_DURATIONS_RU` и `createThreadsDrawerController(containerEl, initialData, options)` (`// @ts-check` + JSDoc, ESM).

## Контракт
- `AUTO_ARCHIVE_DURATIONS_RU` заморожен (`Object.freeze`) и содержит 4 срока автоархивации:
  - `{ minutes: 60, labelRu: "1 ч." }`
  - `{ minutes: 1440, labelRu: "24 ч." }`
  - `{ minutes: 4320, labelRu: "3 дн." }`
  - `{ minutes: 10080, labelRu: "1 нед." }`
- `createThreadsDrawerController(containerEl, initialData, options)` рендерит:
  - вкладки `Активные ветки` (`data-tab="active"`) и `Архивные ветки` (`data-tab="archived"`) с переключением `aria-selected="true" | "false"`;
  - поле поиска с `placeholder="Поиск веток..."`;
  - список подходящих веток или пустое состояние `Ветки не найдены` (`[data-threads-empty]`);
  - для каждой ветки: `name` (`[data-thread-name]`), `messageCount` (`[data-message-count]`), метку автоархивации из `AUTO_ARCHIVE_DURATIONS_RU` (`[data-auto-archive-label]`), кнопку `Покинуть ветку` (`joined === true`) / `Вступить в ветку` (`joined === false`) с вызовом `onToggleJoin(thread.id, !thread.joined)`, а также открытие split-pane ветки по клику с вызовом `onOpenThread(thread.id)`.
- `createThread({ name, autoArchiveDuration = 1440, starterMessage })` проверяет непустое `name.trim()` и допустимый срок автоархивации (`60 | 1440 | 4320 | 10080`), возвращает `{ ok: false, errorRu: "Введите название ветки" }` или `{ ok: false, errorRu: "Некорректный срок автоархивации" }` при ошибке, иначе вызывает `onCreateThread({ name, autoArchiveDuration, starterMessage })` и возвращает `{ ok: true }`.

## Прогоны
- Red: `node --test tests/unit/threads-drawer.test.mjs` → fail 1 (`ERR_MODULE_NOT_FOUND`), сохранён в `red.log`.
- Green: `node --test tests/unit/threads-drawer.test.mjs` → `tests 11, pass 11, fail 0`, сохранён в `green.log`.
- Typecheck: `npx tsc -p tsconfig.json` → 0 ошибок в `rust/gui-spike/ui/features/threads-drawer.js` и `tests/unit/threads-drawer.test.mjs`.

## Файлы и md5
```
8db3f33af1bb95eda621dc1b28d71a71  rust/gui-spike/ui/features/threads-drawer.js
a5462452053516d5dddc5b697d660be2  tests/unit/threads-drawer.test.mjs
```

## Риски и остаток
- Монтаж `createThreadsDrawerController` в основной UI-роутер чата (`app.js`) выполняется отдельной интеграционной карточкой.
- `git commit` не выполнялся.
