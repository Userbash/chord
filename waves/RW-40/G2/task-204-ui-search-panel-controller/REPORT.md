# task-204 — UI search panel controller (`search-panel.js`)

Batch: RW-40 / G2. Ветка: `feature/task-204-ui-search-panel-controller`.

## Сделано
- `tests/unit/search-panel.test.mjs` — red-first unit-тесты (10 тестов, 3 сьюта) на `parseSearchQueryTokens`, `SEARCH_FILTER_SUGGESTIONS_RU`, `clampSearchOffset` и `createSearchPanelController`.
- `rust/gui-spike/ui/features/search-panel.js` — реализация парсера поисковых токенов, замороженного справочника русских подсказок фильтрации и контроллера панели поиска (`// @ts-check` + JSDoc, ESM).

## Контракт и RE-оракул
- Оракулы `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0@7564745` (`SearchStore`/`queryKey`), `@318812` (`T_y = 25`, `MAX_SEARCH_OFFSET = 9975`) и `/home/sanya/re-deep/frontend-d03.md` §1.1–§1.3:
  - `parseSearchQueryTokens(rawQuery)` разбирает токены `from:<user>`, `in:<channel>`, `has:<link|embed|file|image|video>`, `mentions:<user>`, `pinned:<true|false>` и оставшийся свободный текст, возвращая `{ content: string, author: string[], channel: string[], has: string[], mentions: string[], pinned: boolean | null }`.
  - `SEARCH_FILTER_SUGGESTIONS_RU` заморожен (`Object.freeze`) и содержит 6 русских подсказок (`from:`, `in:`, `has:link`, `has:file`, `has:image`, `pinned:true`).
  - `createSearchPanelController(containerEl, options = { onSearch, onJumpToMessage })`:
    - рендерит поле поиска (`[data-search-input]`), чипы подсказок (`[data-suggestion-token]`), русские вкладки сортировки (`'Новые' -> 'newest'`, `'Старые' -> 'oldest'`, `'Релевантные' -> 'relevant'`), заголовок общего числа результатов (`'Найдено результатов: N'` при `N > 0` или `'Ничего не найдено'` при `0`), карточки результатов (`[data-search-hit]`) с кнопкой `'Перейти'` (`onJumpToMessage(hit.id, hit.channelId)`) и элементы пагинации (`[data-page-prev]` / `[data-page-next]`, шаг `25`, зажим `offset` в `0..=9975`).

## Прогоны
- Red: `node --test tests/unit/search-panel.test.mjs` → `fail 1` (`ERR_MODULE_NOT_FOUND`), сохранён в `red.log`.
- Green: `node --test tests/unit/search-panel.test.mjs` → `tests 10, pass 10, fail 0`, сохранён в `green.log`.
- Typecheck: `npx tsc -p tsconfig.json` → 0 ошибок в `rust/gui-spike/ui/features/search-panel.js` и `tests/unit/search-panel.test.mjs`.

## Файлы и md5
```
8dffe69f992cfe42f73f7ff7df1e5485  rust/gui-spike/ui/features/search-panel.js
edf727e6646da8a37500fbae9135c02b  tests/unit/search-panel.test.mjs
```

## Риски и остаток
- Монтаж `createSearchPanelController` в общий shell `app.js` выполняется отдельной интеграционной карточкой.
- `git commit` не выполнялся.
