# TASK-217 (FE-P1-19) — UI контроллер вкладочного пикера выражений: Эмодзи, GIF и Стикеры (`expression-picker.js`, G2)

- **Карточка / Ветка**: `task-217` (`feature/task-217-ui-expression-picker`)
- **Отдел**: G2 Frontend (WebView UI & DOM/A11y)
- **Оракулы**: `reference-oracle/frontend-d03.md` (§2.1–§2.2 `expressionPickerPositionLayer__74017`, вкладки `[role="tab"]`, поиск, селектор тона кожи Фицпатрика), `reference-oracle/ui8/21-emoji-picker.json`, `21d-emoji-categories.json`.

## Сделано
1. `tests/unit/expression-picker.test.mjs` (Red → Green, 8 тестов в 2 suites, `// @ts-check` + JSDoc, минимальный DOM-шим):
   - проверка заморозки и состава `EXPRESSION_TABS_RU = Object.freeze({ emoji: 'Эмодзи', gif: 'GIF', stickers: 'Стикеры' })`, `SKIN_TONE_MODIFIERS = Object.freeze(['', '\u{1F3FB}', '\u{1F3FC}', '\u{1F3FD}', '\u{1F3FE}', '\u{1F3FF}'])` и `MAX_RECENT_EMOJIS = 18`;
   - проверка `applySkinTone(baseEmoji, toneIndex)`: добавление Fitzpatrick-модификатора только для валидных индексов `1..5` и сохранение `baseEmoji` при `toneIndex <= 0`, `>= SKIN_TONE_MODIFIERS.length` или пустом `baseEmoji`;
   - проверка `recordRecentEmoji(recentList, emojiItem)`: дедупликация по `emojiItem.id || emojiItem.name`, постановка нового элемента в начало без мутации исходного списка и обрезка до `MAX_RECENT_EMOJIS` (`18`);
   - проверка `createExpressionPickerController({ container, onSelectEmoji, onSelectSticker, onSelectGif })`:
     - рендер диалога `[data-expression-picker]` (`role="dialog"`) с 3 вкладками (`[data-picker-tab="emoji"|"gif"|"stickers"]`, `aria-selected="true"|"false"`), полем поиска `[data-picker-search]` (`placeholder="Найти идеальный эмодзи..."`), селектором тона кожи `[data-skin-tone-select]` и сеткой `[data-picker-grid]`;
     - переключение вкладок `emoji` / `gif` / `stickers`, регистронезависимая фильтрация эмодзи/стикеров/GIF по имени и тегам, применение тона кожи к эмодзи, вызов соответствующего callback при выборе элемента, закрытие пикера по клику без `shiftKey`, удержание пикера открытым при `shiftKey: true` и закрытие по клавише `Escape`.
2. `rust/gui-spike/ui/features/expression-picker.js` (`// @ts-check` + JSDoc, ESM):
   - экспортированы `EXPRESSION_TABS_RU`, `SKIN_TONE_MODIFIERS`, `MAX_RECENT_EMOJIS`, `applySkinTone`, `recordRecentEmoji` и `createExpressionPickerController`.

## Прогоны (Red / Green / Targeted / Gate)
- **Red**: `node --test tests/unit/expression-picker.test.mjs` → `tests 1, pass 0, fail 1` (`ERR_MODULE_NOT_FOUND`), лог сохранён в `/home/user/orca/project/waves/RW-40/G2/task-217-ui-expression-picker/red.log`.
- **Green**: `node --test tests/unit/expression-picker.test.mjs` → `tests 8, suites 2, pass 8, fail 0`, лог сохранён в `/home/user/orca/project/waves/RW-40/G2/task-217-ui-expression-picker/green.log`.
- **Typecheck (targeted)**: `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` и проверка `expression-picker` через `npx tsc -p tsconfig.json --noEmit` — `0 errors`.
- **Gate**: полный `gate:worker` не запускался (режим ТЗ карточки); чужие файлы не затронуты.

## Файлы и MD5
```
d6bbcca8609833bced40b09cd8817e66  rust/gui-spike/ui/features/expression-picker.js
5e4f7a2fef0224cf0135f5900bc0d885  tests/unit/expression-picker.test.mjs
399eaf73b55ff0deca231a970edd2d99  /home/user/orca/project/waves/RW-40/G2/task-217-ui-expression-picker/red.log
634cffa40b0e7146b9120a2f492190be  /home/user/orca/project/waves/RW-40/G2/task-217-ui-expression-picker/green.log
```

## Риски
- Монтаж `createExpressionPickerController` в общий shell (`app.js` / `composer.js`) находится вне write-scope данной карточки и выполняется отдельной интеграционной задачей.
- `git commit` не выполнялся согласно правилам воркера.
