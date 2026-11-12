# task-261 — Application Slash Command Autocomplete & Invocation Picker Controller (`slash-command-picker.js`, FE-P0-18)

- **Карточка / Ветка**: `task-261` (`feature/task-261-ui-slash-command-picker`)
- **Отдел**: G2 Frontend (WebView UI & DOM/A11y)
- **Оракулы**: `reference-oracle/rest-static.json:7720-7731` (`INTERACTIONS POST /interactions`), `reference-oracle/rest-static.json:9399-9421` (`APPLICATION_COMMAND_INDEX_CHANNEL`, `APPLICATION_COMMAND_INDEX_GUILD`), `rust/gui-spike/src/slash_commands.rs` (имя слэш-команды `1..=32` символов `[a-z0-9_-]`, до `25` опций).

## Сделано
1. `tests/unit/slash-command-picker.test.mjs` (`// @ts-check`, Strict TDD: Red → Green, 10 unit-тестов в 4 suites):
   - проверка экспорта констант `MAX_SLASH_COMMAND_NAME_CHARS = 32` и `MAX_SLASH_COMMAND_OPTIONS = 25`;
   - проверка `isValidSlashCommandName(name)`:
     - возвращает `true`, если `typeof name === "string"` и `name.trim()` имеет длину `1..=32` символов и соответствует `/^[a-z0-9_-]+$/`;
     - возвращает `false` для пустых/пробельных строк, длины `> 32`, заглавных букв, спецсимволов и не-строк;
   - проверка `parseSlashDraftInput(input)`:
     - возвращает `{ commandName, rawArgs }` (с `commandName` в нижнем регистре) для строк, начинающихся с `'/'` и непробельного токена команды;
     - возвращает `null`, если строка не начинается с `'/'` или сразу после `'/'` идёт пробел/пусто;
   - проверка `filterSlashCommands(commands, query)`:
     - фильтрация массива `{ id, name, description, options?: Array }` по префиксу или регистронезависимой подстроке `name`/`description` с приоритетом префикса `name`, сортировкой по `name.localeCompare(..., "en")` по возрастанию и tie-break по `String(id)` по возрастанию без мутации входного массива;
   - проверка `createSlashCommandPickerController(options)`:
     - рендер корневого узла `[data-slash-picker]` с `role="listbox"` и `aria-label="Команды приложений"`;
     - рендер элементов `[data-slash-cmd-id="<id>"]` с `role="option"`, `aria-selected="true"|"false"`, именем `"/<name>"` и описанием, а также вызов `onSelectCommand(command)` по клику;
     - проверка методов `{ setState(patch), getState(), moveSelection(delta), selectActive(), destroy() }`.
2. `rust/gui-spike/ui/features/slash-command-picker.js` (`// @ts-check` + JSDoc, ESM):
   - реализованы и экспортированы `MAX_SLASH_COMMAND_NAME_CHARS`, `MAX_SLASH_COMMAND_OPTIONS`, `isValidSlashCommandName`, `parseSlashDraftInput`, `filterSlashCommands` и `createSlashCommandPickerController`.

## Прогоны (Red / Green / Targeted / Gate)
- **Red**: `node --test tests/unit/slash-command-picker.test.mjs > /home/user/orca/project/waves/RW-40/G2/task-261-ui-slash-command-picker/red.log 2>&1` → `exit=1` (`ERR_MODULE_NOT_FOUND`, `tests 1, pass 0, fail 1`).
- **Green**: `node --test tests/unit/slash-command-picker.test.mjs > /home/user/orca/project/waves/RW-40/G2/task-261-ui-slash-command-picker/green.log 2>&1` → `exit=0` (`tests 10, suites 4, pass 10, fail 0`).
- **Typecheck**:
  - `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` → `exit=0` (0 ошибок);
  - `npx tsc --ignoreConfig --noEmit --strict --allowJs --checkJs --noUncheckedIndexedAccess --skipLibCheck --target es2022 --module node20 --moduleResolution node16 --types node rust/gui-spike/ui/features/slash-command-picker.js tests/unit/slash-command-picker.test.mjs` → `exit=0` (0 ошибок).
- **Gate**: полный `gate:worker` не запускался (режим ТЗ воркера в batch); чужие файлы не затронуты.

## Файлы и md5
```
88da4074bc8fc73d8f6eea8a87fda62c  rust/gui-spike/ui/features/slash-command-picker.js
b618ce0a27babd187469a34e5abdcb75  tests/unit/slash-command-picker.test.mjs
449b7f8e18dcff40929943ec776560fb  /home/user/orca/project/waves/RW-40/G2/task-261-ui-slash-command-picker/red.log
5e90ca12646ed6332a4480fac2576030  /home/user/orca/project/waves/RW-40/G2/task-261-ui-slash-command-picker/green.log
```

## Риски
- Контроллер не зависит от глобального `window.document`, если `options.container` предоставляет `ownerDocument.createElement` или `createElement`.
- `git commit` не выполнялся в соответствии с правилами воркера.
