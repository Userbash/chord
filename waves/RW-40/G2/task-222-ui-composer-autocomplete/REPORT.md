# TASK-222 (FE-P1-21) — UI контроллер автодополнения композера: слэш-команды `/`, упоминания `@user/@role` и каналы `#channel` (`composer-autocomplete.js`, G2)

- **Карточка / Ветка**: `task-222` (`feature/task-222-ui-composer-autocomplete`)
- **Отдел**: G2 Frontend (WebView UI & DOM/A11y)
- **Оракулы**: `/home/sanya/re-deep/frontend-d03.md` (композер, всплывающий список подсказок `role="listbox"`, элементы `role="option"` с `aria-selected`), `/home/sanya/re-deep/ru-catalog.json` (`X2x0MF`: «Доступные команды», секции `КОМАНДЫ` / `УЧАСТНИКИ И РОЛИ` / `ТЕКСТОВЫЕ КАНАЛЫ`).

## Сделано
1. `tests/unit/composer-autocomplete.test.mjs` (Red → Green, 8 тестов в 2 suites, `// @ts-check` + JSDoc, минимальный DOM-шим):
   - проверка экспорта и заморозки констант `AUTOCOMPLETE_MAX_SUGGESTIONS = 10` и `AUTOCOMPLETE_KINDS = Object.freeze({ USER: "@", CHANNEL: "#", SLASH: "/" })`;
   - проверка `detectAutocompleteTrigger(text, cursorIndex)`:
     - распознавание триггера слэш-команды только в начале строки по `/^\/([a-z0-9_-]*)$/i` (`{ kind: "slash", query, startIndex: 0 }`) и возврат `null` при слэше не с начала строки или после пробела;
     - распознавание триггеров `@` (`kind: "user"`) и `#` (`kind: "channel"`) в начале строки и после пробельных символов (` `, `\n`) с вычислением точного `startIndex`;
     - возврат `null` для email (`user@example.com`), URL-якорей (`https://discord.com#top`), текста после пробела и пустого ввода;
   - проверка `createComposerAutocompleteController({ container, onInsertReplacement })`:
     - рендер выпадающего списка `[data-composer-autocomplete]` с `role="listbox"`, заголовком `[data-autocomplete-header]` (`"КОМАНДЫ"`, `"УЧАСТНИКИ И РОЛИ"`, `"ТЕКСТОВЫЕ КАНАЛЫ"`) и кнопками `[data-autocomplete-option]` (`role="option"`, `aria-selected="true" | "false"`);
     - ограничение количества отображаемых подсказок до `AUTOCOMPLETE_MAX_SUGGESTIONS` (`10`) и циклическая навигация `moveSelection(delta)` по `0..items.length - 1`;
     - замена триггер-токена через `confirmSelection()` и по клику мыши на `"<@id>"`, `"<#id>"` или `"/command "` с вызовом `onInsertReplacement({ newText, newCursorIndex, item })` и закрытием списка;
     - принудительное закрытие через `close()`.
2. `rust/gui-spike/ui/features/composer-autocomplete.js` (`// @ts-check` + JSDoc, ESM):
   - экспортированы `AUTOCOMPLETE_MAX_SUGGESTIONS`, `AUTOCOMPLETE_KINDS`, `AUTOCOMPLETE_HEADERS_RU`, `detectAutocompleteTrigger` и `createComposerAutocompleteController`.

## Прогоны (Red / Green / Targeted / Gate)
- **Red**: `node --test tests/unit/composer-autocomplete.test.mjs` → `tests 1, pass 0, fail 1` (`ERR_MODULE_NOT_FOUND`), лог сохранён в `/home/sanya/orca/project/waves/RW-40/G2/task-222-ui-composer-autocomplete/red.log`.
- **Green**: `node --test tests/unit/composer-autocomplete.test.mjs` → `tests 8, suites 2, pass 8, fail 0`, лог сохранён в `/home/sanya/orca/project/waves/RW-40/G2/task-222-ui-composer-autocomplete/green.log`.
- **Typecheck (targeted)**: `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` и целевая проверка `composer-autocomplete.js` + `composer-autocomplete.test.mjs` через `npx tsc --ignoreConfig` — `0 errors`.
- **Gate**: полный `gate:worker` не запускался (режим ТЗ карточки); чужие файлы не затронуты.

## Файлы и MD5
```
79eb7acc7351aa330549229624298e1d  rust/gui-spike/ui/features/composer-autocomplete.js
5453420db7fd309ff029a126dd471253  tests/unit/composer-autocomplete.test.mjs
95d83ef05f55e95b5bbdfda73c6dec90  /home/sanya/orca/project/waves/RW-40/G2/task-222-ui-composer-autocomplete/red.log
764a3df02e78ee33c7ed4607ada15a96  /home/sanya/orca/project/waves/RW-40/G2/task-222-ui-composer-autocomplete/green.log
```

## Риски
- Монтаж `createComposerAutocompleteController` в основной контроллер композера (`composer.js` / `app.js`) находится вне write-scope данной карточки и выполняется отдельной интеграционной задачей.
- `git commit` не выполнялся согласно правилам воркера.
