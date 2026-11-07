# task-251 — AutoModeration Rules & Keyword Filter Editor Controller (`automod-editor.js`, FE-P0-13)

- **Карточка / Ветка**: `task-251` (`feature/task-251-ui-automod-editor`)
- **Отдел**: G2 Frontend (WebView UI & DOM/A11y)

## Сделано
1. `tests/unit/automod-editor.test.mjs` (`// @ts-check`, Strict TDD: Red → Green, 9 unit-тестов в 3 suites):
   - проверка экспорта и заморозки `AUTOMOD_TRIGGER_LABELS_RU = Object.freeze({ 1: "Фильтр ключевых слов", 3: "Защита от спама", 4: "Часто упоминаемые слова", 5: "Спам упоминаниями" })` и `AUTOMOD_ACTION_LABELS_RU = Object.freeze({ 1: "Блокировать сообщение", 2: "Отправить предупреждение", 3: "Тайм-аут участника" })`;
   - проверка `parseKeywordListInput(rawText)`:
     - разбиение по запятым и переводам строк (`/[,\r\n]+/`), обрезка пробелов (`trim()`);
     - отбрасывание пустых токенов и токенов длиной `> 60` символов (с проверкой границы `60` vs `61`);
     - регистронезависимая дедупликация с сохранением первого встреченного регистра;
     - ограничение результата максимум `1000` элементами;
   - проверка `createAutoModEditorController(options)`:
     - рендер региона `[data-automod-editor]` с `role="region"` и `aria-label="Автомодерация"`;
     - рендер карточек правил `[data-automod-rule-id="<id>"]` с именем правила, русской меткой триггера, русской меткой действия и переключателем `[data-automod-rule-toggle="<id>"]` (`role="switch"`, `aria-checked="true"|"false"`; клик инвертирует `enabled` и вызывает `onToggleRule(id, nextEnabled)`);
     - рендер формы создания/редактирования правила `[data-automod-form]` с полями `[data-automod-name-input]`, `[data-automod-trigger-select]`, `[data-automod-action-select]`, `[data-automod-keywords-input]` и кнопкой `[data-automod-save-btn]`, которая вызывает `onSaveRule({ name, triggerType, actionType, keywordFilter })` при непустом обрезанном `name` и игнорирует пустое/пробельное имя;
     - проверка методов `{ setState(patch), getState(), destroy() }` и защиты состояния от внешней мутации.
2. `rust/gui-spike/ui/features/automod-editor.js` (`// @ts-check` + JSDoc, ESM):
   - реализованы и экспортированы `AUTOMOD_TRIGGER_LABELS_RU`, `AUTOMOD_ACTION_LABELS_RU`, `parseKeywordListInput` и `createAutoModEditorController`.

## Прогоны (Red / Green / Targeted / Gate)
- **Red**: `node --test tests/unit/automod-editor.test.mjs` → `exit=1` (`ERR_MODULE_NOT_FOUND`, `tests 1, pass 0, fail 1`), лог сохранён в `/home/sanya/orca/project/waves/RW-40/G2/task-251-ui-automod-editor/red.log`.
- **Green**: `node --test tests/unit/automod-editor.test.mjs` → `exit=0` (`tests 9, suites 3, pass 9, fail 0`), лог сохранён в `/home/sanya/orca/project/waves/RW-40/G2/task-251-ui-automod-editor/green.log`.
- **Typecheck**:
  - `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` → `exit=0` (0 ошибок);
  - `./node_modules/.bin/tsc --ignoreConfig --noEmit --strict --allowJs --checkJs --noUncheckedIndexedAccess --target es2022 --module node20 --moduleResolution node16 --types node rust/gui-spike/ui/features/automod-editor.js tests/unit/automod-editor.test.mjs` → `exit=0` (0 ошибок).
- **Gate**: полный `gate:worker` не запускался (режим ТЗ воркера в batch); чужие файлы не затронуты.

## Файлы и md5
```
a0cb1574ba8df4ef99ebc4fa9a8cdd92  rust/gui-spike/ui/features/automod-editor.js
512b7deeb0e0ee03a2a072a90f19222b  tests/unit/automod-editor.test.mjs
86c518a70661774e3328d0917c94788d  /home/sanya/orca/project/waves/RW-40/G2/task-251-ui-automod-editor/red.log
dfbefee946579e29e52f1453931f1890  /home/sanya/orca/project/waves/RW-40/G2/task-251-ui-automod-editor/green.log
```

## Риски
- Контроллер не зависит от глобального `window.document`, если `options.container` предоставляет `ownerDocument.createElement` или `createElement`.
- `git commit` не выполнялся в соответствии с правилами воркера.
