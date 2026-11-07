# task-202 — UI forum view controller (`forum-view.js`)

Batch: RW-40 / G2. Ветка: `feature/task-202-ui-forum-view-controller`.

## Сделано
- `tests/unit/forum-view.test.mjs` — red-first unit-тесты (8 тестов, 3 сьюта) на `createForumViewController`: шапка с поиском, сортировкой и чипами тегов, фильтрация/сортировка карточек, закреплённые треды и счётчик сообщений, пустое состояние и валидация/отправка новой публикации.
- `rust/gui-spike/ui/features/forum-view.js` — реализация `createForumViewController(containerEl, initialData, options)` с переиспользованием `./forum-filter.js` (`FORUM_SORT_MODES`, `filterAndSortForumThreads`, `countThreadsByTag`); `// @ts-check` + JSDoc, ESM.

## Контракт
- Шапка форума рендерит:
  - поле поиска с `placeholder="Поиск публикаций..."`;
  - селектор сортировки (`<select>`) с опциями `Недавняя активность` (`latest_activity`) и `Дата создания` (`creation_date`);
  - чипы тегов со счётчиками из `countThreadsByTag`, переключающие `aria-pressed="true" | "false"` по клику и фильтрующие список тредов.
- Список тредов рендерится через `filterAndSortForumThreads`:
  - закреплённые треды отмечаются `data-pinned="true"` и бейджем `Закреплено` (`[data-pinned-badge]`);
  - у каждой карточки отображается счётчик сообщений (`[data-message-count]`);
  - клик по карточке вызывает `onSelectThread(thread.id)`.
- При пустом результате фильтрации рендерится пустое состояние `Публикации не найдены` (`[data-forum-empty]`).
- `submitNewThread({ title, content, tagIds })` проверяет непустые `title.trim()` и `content.trim()`, при ошибке возвращает `{ ok: false, errorRu: "Введите заголовок и текст публикации" }`, а при валидных данных вызывает `onCreateThread({ title, content, tagIds })` и возвращает `{ ok: true }`.

## Прогоны
- Red: `node --test tests/unit/forum-view.test.mjs` → fail 1 (`ERR_MODULE_NOT_FOUND`), сохранён в `red.log`.
- Green: `node --test tests/unit/forum-view.test.mjs` → `tests 8, pass 8, fail 0`, сохранён в `green.log`.
- Typecheck: `npx tsc -p tsconfig.json` → 0 ошибок в `rust/gui-spike/ui/features/forum-view.js` и `tests/unit/forum-view.test.mjs`.

## Файлы и md5
```
a7f98f2cdb7ea34f1adc20d51677e5c4  rust/gui-spike/ui/features/forum-view.js
3208cd7edaab3b370cb6e2bdf31a1e27  tests/unit/forum-view.test.mjs
```

## Риски и остаток
- Монтаж `createForumViewController` в переключение каналов типа `15` (`GUILD_FORUM`) в `app.js` выполняется отдельной интеграционной карточкой.
- `git commit` не выполнялся.
