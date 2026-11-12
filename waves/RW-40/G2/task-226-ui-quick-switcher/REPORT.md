# TASK-226 (FE-P1-22) — UI контроллер модального окна быстрого перехода Quick Switcher (`Ctrl+K`) с префиксной и fuzzy-фильтрацией каналов, ЛС и гильдий (`quick-switcher.js`, G2)

- **Карточка / Ветка**: `task-226` (`feature/task-226-ui-quick-switcher`)
- **Отдел**: G2 Frontend (WebView UI & DOM/A11y)
- **Оракулы**: `reference-oracle/frontend-d03.md` (§1 `mod+k` — модуль `675704` QuickSwitcher), `reference-oracle/ui/03-quickswitcher.html`, `reference-oracle/ui/03-quickswitcher-input.html` (`placeholder="Куда отправимся?"`, `autocompleteQuerySymbol_ac6cb0`: `@` пользователи, `#` текстовые каналы, `!` голосовые каналы, `*` серверы).

## Сделано
1. `tests/unit/quick-switcher.test.mjs` (Red → Green, 8 тестов в 2 suites, `// @ts-check` + JSDoc, минимальный DOM-шим):
   - проверка экспорта и заморозки `QUICK_SWITCHER_PREFIXES = Object.freeze({ USER: "@", TEXT_CHANNEL: "#", VOICE_CHANNEL: "!", GUILD: "*" })` и `MAX_QUICK_SWITCHER_RESULTS = 10`;
   - проверка `scoreAndFilterQuickSwitcherItems(items, rawQuery)`:
     - префиксная фильтрация по `"@"` (`"user"`), `"#"` (`"text"`), `"!"` (`"voice"`), `"*"` (`"guild"`) с отбрасыванием символа префикса и возвратом всех элементов соответствующего типа при пустом остатке запроса;
     - регистронезависимый поиск по подстроке и нечёткой подпоследовательности (`fuzzy subsequence`) в `item.name`;
     - ранжирование: точный префикс `item.name` → элементы с `mentionCount > 0` → элементы с `unreadCount > 0` → по алфавиту `name`, с обрезкой до `MAX_QUICK_SWITCHER_RESULTS` (`10`) и без мутации входного массива;
   - проверка `createQuickSwitcherController({ container, onSelectTarget })`:
     - рендер диалога `[data-quick-switcher]` (`role="dialog"`, `aria-modal="true"`), поля ввода `[data-quick-switcher-input]` (`placeholder="Куда отправимся?"`), списка `[data-quick-switcher-results]` (`role="listbox"`), опций `[data-quick-switcher-option]` (`role="option"`, `aria-selected="true"` на `activeIndex`) и бейджа непрочитанных/упоминаний `[data-quick-switcher-badge]`;
     - фильтрация через `setQuery(q)` со сбросом `activeIndex` на `0`;
     - циклическая навигация `moveSelection(delta)` (wrapping `0..N-1`);
     - подтверждение `confirmSelection()` с вызовом `onSelectTarget(item)` и закрытием модалки, а также закрытие `close()` без вызова callback.
2. `rust/gui-spike/ui/features/quick-switcher.js` (`// @ts-check` + JSDoc, ESM):
   - экспортированы `QUICK_SWITCHER_PREFIXES`, `MAX_QUICK_SWITCHER_RESULTS`, `QUICK_SWITCHER_PLACEHOLDER_RU`, `scoreAndFilterQuickSwitcherItems`, `createQuickSwitcherController` при полном сохранении совместимости с существующим `createQuickSwitcher({ router })`.

## Прогоны (Red / Green / Targeted / Gate)
- **Red**: `node --test tests/unit/quick-switcher.test.mjs` → `tests 1, pass 0, fail 1` (`SyntaxError: ... does not provide an export named 'MAX_QUICK_SWITCHER_RESULTS'`), лог сохранён в `/home/user/orca/project/waves/RW-40/G2/task-226-ui-quick-switcher/red.log`.
- **Green**: `node --test tests/unit/quick-switcher.test.mjs` → `tests 8, suites 2, pass 8, fail 0`, лог сохранён в `/home/user/orca/project/waves/RW-40/G2/task-226-ui-quick-switcher/green.log`.
- **Targeted & Typecheck**: `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` (`0 errors`), `0 errors` в `quick-switcher.js` / `quick-switcher.test.mjs` при проверке `tsconfig.json`, `node --test tests/unit/gui-spike-regressions.test.mjs` (`13/13 pass`).
- **Gate**: полный `gate:worker` не запускался (режим ТЗ карточки); чужие файлы не затронуты.

## Файлы и MD5
```
9fa592ae1447d5c6c7615d313b12c503  rust/gui-spike/ui/features/quick-switcher.js
e7e57f435ac6fe5172c791773b516198  tests/unit/quick-switcher.test.mjs
f7952108e90bd6c1d35e83c33d017d90  /home/user/orca/project/waves/RW-40/G2/task-226-ui-quick-switcher/red.log
6d2e466bf6cc8587161971211ec4ded1  /home/user/orca/project/waves/RW-40/G2/task-226-ui-quick-switcher/green.log
```

## Риски
- Интеграция `createQuickSwitcherController` с данными каналов/ЛС/гильдий из живого состояния диспетчера выполняется отдельной задачей монтажа.
- `git commit` не выполнялся согласно правилам воркера.
