# task-240 — Forum Channel & Post Creation View Controller (`forum-view.js`, FE-P0-10)

- **Карточка / Ветка**: `task-240` (`feature/task-240-ui-forum-view`)
- **Отдел**: G2 Frontend (WebView UI & DOM/A11y)
- **Оракулы**: `/home/sanya/re-deep/gateway-schema.md` (`default_sort_order`, `default_forum_layout`), `rust/gui-spike/src/forum.rs` (`MAX_FORUM_APPLIED_TAGS = 5`, `ForumSortOrder`).

## Сделано
1. `tests/unit/forum-view.test.mjs` (`// @ts-check`, Strict TDD: Red → Green, 7 unit-тестов в 2 suites):
   - проверка экспорта и заморозки `FORUM_SORT_ORDERS = Object.freeze({ LATEST_ACTIVITY: 0, CREATION_DATE: 1 })`, `FORUM_LAYOUT_VIEWS = Object.freeze({ LIST: 1, GALLERY: 2 })` и `MAX_FORUM_POST_TAGS = 5`;
   - проверка `sortAndFilterForumPosts(posts, options)`:
     - фильтрация по всем выбранным тегам (`selectedTagIds` — `post.appliedTags` должен содержать все указанные теги) и регистронезависимому подстрочному поиску по `post.title` (`searchQuery`);
     - сортировка закреплённых публикаций (`post.pinned === true`) первыми, затем по убыванию `lastActivityTimestamp` (`sortOrder === 0` или не указан) либо `createdAtTimestamp` (`sortOrder === 1`) с tie-break по `String(post.id)` `localeCompare` desc;
     - гарантия отсутствия мутации входного массива и объектов публикаций;
   - проверка `createForumViewController(options)`:
     - рендер корня `[data-forum-view]` с `role="region"` и `aria-label="Форум"`;
     - рендер панели фильтра тегов `[data-forum-tag-filter="<tagId>"]` с `aria-pressed="true" | "false"` и переключением фильтра по клику;
     - рендер карточек публикаций `[data-forum-post-id="<id>"]` с заголовком `title`, счётчиком `messageCount`, бейджем `[data-pinned="true"]` для закреплённых и вызовом `onSelectPost(id)` по клику;
     - рендер формы создания новой публикации `[data-forum-new-post-form]` с полями `[data-forum-title-input]`, `[data-forum-body-input]`, чекбоксами тегов (блокировка и запрет выбора более `MAX_FORUM_POST_TAGS = 5` тегов) и кнопкой `[data-forum-submit-btn]`, вызывающей `onCreatePost({ title, content, appliedTags })` с обрезанными строками при непустых заголовке и тексте;
     - поддержка методов `{ setState(patch), getState(), destroy() }`.
2. `rust/gui-spike/ui/features/forum-view.js` (`// @ts-check` + JSDoc, ESM):
   - реализованы и экспортированы `FORUM_SORT_ORDERS`, `FORUM_LAYOUT_VIEWS`, `MAX_FORUM_POST_TAGS`, `sortAndFilterForumPosts` и `createForumViewController`.

## Прогоны (Red / Green / Targeted / Gate)
- **Red**: `node --test tests/unit/forum-view.test.mjs` → `exit=1` (`SyntaxError: The requested module '../../rust/gui-spike/ui/features/forum-view.js' does not provide an export named 'FORUM_LAYOUT_VIEWS'`, `tests 1, pass 0, fail 1`), лог сохранён в `/home/sanya/orca/project/waves/RW-40/G2/task-240-ui-forum-view/red.log`.
- **Green**: `node --test tests/unit/forum-view.test.mjs` → `exit=0` (`tests 7, suites 2, pass 7, fail 0`), лог сохранён в `/home/sanya/orca/project/waves/RW-40/G2/task-240-ui-forum-view/green.log`.
- **Typecheck**:
  - `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` → `exit=0` (0 ошибок);
  - `tsc --ignoreConfig --noEmit --strict --allowJs --checkJs --target es2022 --module node20 --moduleResolution node16 --types node rust/gui-spike/ui/features/forum-view.js tests/unit/forum-view.test.mjs` → `exit=0` (0 ошибок).
- **Gate**: полный `gate:worker` не запускался (режим ТЗ воркера в batch); чужие файлы не затронуты.

## Файлы и md5
```
9c076101ba72900431651e4620af3e66  rust/gui-spike/ui/features/forum-view.js
0dc4cf045ad4dbb6fd31d42d1a17c26b  tests/unit/forum-view.test.mjs
29f6bae140d468f8f0c525f932ba73ca  /home/sanya/orca/project/waves/RW-40/G2/task-240-ui-forum-view/red.log
46a7a1d506c0b167a7ca43bd98d809ee  /home/sanya/orca/project/waves/RW-40/G2/task-240-ui-forum-view/green.log
```

## Риски
- Контроллер не зависит от глобального `window.document`, если `options.container` предоставляет `ownerDocument.createElement` или `createElement`.
- `git commit` не выполнялся в соответствии с правилами воркера.
