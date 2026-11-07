# task-256 — Guild Custom Emoji & Sticker Upload Manager Controller (`expressions-manager.js`, FE-P0-15)

- **Карточка / Ветка**: `task-256` (`feature/task-256-ui-expressions-manager`)
- **Отдел**: G2 Frontend (WebView UI & DOM/A11y)
- **Оракулы**: `/home/sanya/re-deep/rest-static.md` (`GUILD_EMOJIS`, `GUILD_EMOJI`, `GUILD_STICKER_PACKS`, `GUILD_STICKER`, лимиты имени эмодзи `2..=32` символов `[A-Za-z0-9_]`).

## Сделано
1. `tests/unit/expressions-manager.test.mjs` (`// @ts-check`, Strict TDD: Red → Green, 9 unit-тестов в 4 suites):
   - проверка экспорта и заморозки `EXPRESSION_MANAGER_TABS_RU = Object.freeze([ { id: "emojis", label: "Эмодзи" }, { id: "stickers", label: "Стикеры" } ])`;
   - проверка `isValidCustomEmojiName(name)`:
     - возвращает `true`, если обрезанная по краям строка имеет длину `2..=32` символов и соответствует `/^[A-Za-z0-9_]+$/`;
     - возвращает `false` для пустых/пробельных строк, строк длиной `1` или `>= 33` символов после `trim()`, строк с дефисами/пробелами/двоеточиями/кириллицей и нестроковых значений;
   - проверка `filterCustomExpressions(items, query)`:
     - фильтрация массива `{ id, name, animated? }` по регистронезависимой подстроке `name` с сортировкой `name.localeCompare(..., "ru")` по возрастанию и tie-break по `id` по возрастанию;
     - сохранение неизменности входного массива и его элементов;
   - проверка `createExpressionsManagerController(options)`:
     - рендер региона `[data-expressions-manager]` с `role="region"` и `aria-label="Управление эмодзи и стикерами"`;
     - рендер вкладок `[data-expr-tab="emojis"|"stickers"]` с `role="tab"` и `aria-selected="true"|"false"` и переключение активной вкладки по клику;
     - рендер счётчика слотов `[data-expr-slots]` в формате `"Слоты: N / M"`;
     - рендер элементов активной вкладки `[data-expr-item-id="<id>"]` с именем и кнопкой удаления `[data-expr-delete-btn="<id>"]` (`onDeleteExpression(activeTab, id)`);
     - рендер формы загрузки с `[data-expr-name-input]`, `[data-expr-datauri-input]` и `[data-expr-upload-btn]`, вызывающей `onUploadEmoji({ name: trimmedName, imageDataUri: trimmedUri })` только при `isValidCustomEmojiName(name) === true` и `trimmedUri.startsWith("data:image/") === true`;
     - проверка методов `{ setState(patch), getState(), destroy() }` и неизменяемости возвращаемого снимка состояния.
2. `rust/gui-spike/ui/features/expressions-manager.js` (`// @ts-check` + JSDoc, ESM):
   - реализованы и экспортированы `EXPRESSION_MANAGER_TABS_RU`, `isValidCustomEmojiName`, `filterCustomExpressions` и `createExpressionsManagerController`.

## Прогоны (Red / Green / Targeted / Gate)
- **Red**: `node --test tests/unit/expressions-manager.test.mjs > /home/sanya/orca/project/waves/RW-40/G2/task-256-ui-expressions-manager/red.log 2>&1` → `exit=1` (`ERR_MODULE_NOT_FOUND`, `tests 1, pass 0, fail 1`).
- **Green**: `node --test tests/unit/expressions-manager.test.mjs > /home/sanya/orca/project/waves/RW-40/G2/task-256-ui-expressions-manager/green.log 2>&1` → `exit=0` (`tests 9, suites 4, pass 9, fail 0`).
- **Typecheck**:
  - `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` → `exit=0` (0 ошибок);
  - `npx tsc --ignoreConfig --noEmit --strict --allowJs --checkJs --noUncheckedIndexedAccess --skipLibCheck --target es2022 --module node20 --moduleResolution node16 --types node rust/gui-spike/ui/features/expressions-manager.js tests/unit/expressions-manager.test.mjs` → `exit=0` (0 ошибок).
- **Gate**: полный `gate:worker` не запускался (режим ТЗ воркера в batch); чужие файлы не затронуты.

## Файлы и md5
```
386ffc7915824c9ea2286aea2281b3e0  rust/gui-spike/ui/features/expressions-manager.js
b5841f715ff257616a2ce19055d74fe4  tests/unit/expressions-manager.test.mjs
df41251890c18c809d2936fa9cf46bbf  /home/sanya/orca/project/waves/RW-40/G2/task-256-ui-expressions-manager/red.log
9078175b7e00faf45b0fd29b0a64d3ba  /home/sanya/orca/project/waves/RW-40/G2/task-256-ui-expressions-manager/green.log
```

## Риски
- Контроллер не зависит от глобального `window.document`, если `options.container` предоставляет `ownerDocument.createElement` или `createElement`.
- `git commit` не выполнялся в соответствии с правилами воркера.
