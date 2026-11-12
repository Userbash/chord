# REPORT — TASK-618 (G2 Frontend): UI Поискового попапа с подсказками (`rust/gui-spike/ui/features/search.js` + `rust/gui-spike/ui/styles/search.css`)

- **batch/карточка:** `RW-40 / TASK-618` (G2 Frontend — UI Поискового попапа с подсказками)
- **Ветка:** `feature/task-618-search-popover-ui`
- **Рабочее дерево:** `/home/user/orca/workspaces/project/task-618-search-popover-ui`
- **Дата:** 2026-11-07
- **Коммитов нет:** `git commit` / `git push` не выполнялись (согласно правилам `AGENTS.md` §6).

---

## 1. Сделано (`PROCESS.md` §4.2)

- **batch/карточка:** `RW-40 / TASK-618`;
- **сделано:**
  1. В `rust/gui-spike/ui/features/search.js` (с полным сохранением обратной совместимости всех существующих экспортов `M-13` / `STUB-14`) реализованы:
     - `SEARCH_POPOVER_TOKEN_SUGGESTIONS` — замороженный справочник 7 выпадающих подсказок токенов (`'from:'`, `'mentions:'`, `'has:'`, `'in:'`, `'before:'`, `'after:'`, `'pinned:'`) с русскими подписями и примерами.
     - `getSearchTokenSuggestions(rawQuery)` — фильтрация выпадающих подсказок по вводимому префиксу токена или русской подписи (возвращает все 7 подсказок для пустой строки или после пробела).
     - `tokenizeSearchInputForHighlight(rawQuery)` — сегментация поисковой строки на свободный текст и активные токены (`from:`, `mentions:`, `has:`, `in:`, `before:`, `after:`, `pinned:` и их русские алиасы) для визуальной подсветки активных токенов в строке ввода (`.search-input-token`, `.search-chip-prefix`, `.search-chip-value`).
     - `splitTextBySearchQuery(content, rawQuery)` — регистронезависимая разбивка текста результата на сегменты `{ text, match }` по свободному тексту запроса (с экранированием спецсимволов регулярных выражений и исключением фильтр-токенов из текстовой подсветки).
     - `createSearchPopover(containerEl, options)` (и алиас `createSearchPopoverController`) — DOM-контроллер поискового попапа (`.search-popover[data-search-popover]`), рендерящий строку ввода с подсветкой активных токенов (`[data-search-input-highlight]`), выпадающий список подсказок токенов (`[data-search-token-suggestions]`), список результатов (`[data-search-popover-results]`, карточки `id="search-result-<id>"` с `<mark class="search-highlight" data-search-highlight>`), кнопку прыжка к сообщению (`[data-action="jump-to-message"]` → `onJumpToMessage(messageId, channelId, item)`) и клавиатурную навигацию (`ArrowDown`, `ArrowUp` с циклическим переходом и `aria-activedescendant`, `Enter` для прыжка к выбранному результату или подстановки подсказки, `Escape` для закрытия попапа).
  2. Создан файл стилей `rust/gui-spike/ui/styles/search.css`, оформленный на 100% через CSS-переменные из `rust/gui-spike/ui/tokens.css` (без сырых hex/rgb/hsl цветов и без `!important`).
  3. Создан модульный тест `tests/unit/search-popover.test.mjs` (9 тестов в 4 сьютах) и добавлена строка синхронизации в `TDD-PLAN.md` §8.
- **red:** `node --test tests/unit/search-popover.test.mjs` → `1 fail` (`SyntaxError: The requested module '../../rust/gui-spike/ui/features/search.js' does not provide an export named 'SEARCH_POPOVER_TOKEN_SUGGESTIONS'`), `exit code = 1` (лог: `waves/RW-40/G2/task-618-search-popover-ui/red.log`);
- **green:** `node --test tests/unit/search-popover.test.mjs` → `9/9 pass, 0 fail, 0 skipped`, `exit code = 0` (лог: `waves/RW-40/G2/task-618-search-popover-ui/green.log`);
- **targeted:**
  - `node --test tests/unit/search-popover.test.mjs tests/unit/search-remote-real-index.test.mjs tests/unit/gui-spike-dom.test.mjs tests/unit/gui-spike-features.test.mjs tests/meta/plan-sync.test.mjs` → `218/218 pass, 0 fail, 0 skipped` (`5106.56ms`, `exit code = 0`);
  - `npm run typecheck` → `0 errors` (`exit code = 0`);
  - `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` → `0 errors` (`exit code = 0`);
- **oracle:**
  - `reference-oracle/frontend-d03.md` §1.1–§1.3 и `docs/re/frontend-d03.md` §1.1–§1.3 (`searchResultsWrap_a98f3b`, `id="search-result-<id>"`, секции и фильтр-токены `from:`, `mentions:`, `has:`, `in:`, `before:`, `after:`, `pinned:`);
  - `reference-oracle/ui8/20-search-results.json`, `20a-searchbar.json`, `20b-search-filters.json`;
  - `waves/RW-40/DESIGN/SEARCH-202-NSFW-SPEC.md` §2, §5, §6 и `rust/gui-spike/ui/tokens.css`;
- **генерация:** не применимо (статические модули UI и таблица стилей);
- **мутации:** `5/5 KILLED` (`0 survivors`, `0 INCONCLUSIVE`, лог: `waves/RW-40/G2/task-618-search-popover-ui/mutations.log`):
  - `M1_token_suggestions_filter` (`item.token.startsWith(trailing)` → `!item.token.startsWith(trailing)`): **KILLED** (`exit=1`) — тест `getSearchTokenSuggestions фильтрует подсказки по вводимому префиксу`;
  - `M2_active_token_highlight` (`if (seg.type === "token")` → `if (false && seg.type === "token")`): **KILLED** (`exit=1`) — тест `подсвечивает активные токены в строке ввода и поисковый запрос в списке результатов`;
  - `M3_result_query_highlight` (`match: true` → `match: false`): **KILLED** (`exit=1`) — тест `splitTextBySearchQuery регистронезависимо выделяет совпадения поискового запроса`;
  - `M4_jump_to_message_callback` (удаление вызова `options.onJumpToMessage(...)`): **KILLED** (`exit=1`) — тест `вызывает onJumpToMessage по клику на кнопку прыжка и при навигации стрелками + Enter`;
  - `M5_keyboard_arrow_down` (`(activeResultIndex + 1) % results.length` → `0`): **KILLED** (`exit=1`) — тест `вызывает onJumpToMessage по клику на кнопку прыжка и при навигации стрелками + Enter`;
- **baseline/restore:**
  - До и после всех 5 мутаций проверены контрольные суммы:
    - `rust/gui-spike/ui/features/search.js`: `d96a0920012a6f2f0e2b643a7dcf7711` (`match: true`)
    - `tests/unit/search-popover.test.mjs`: `2765033f99c860848c129a4077edd058` (`match: true`)
- **стабильность:** не требовалось (детерминированные синхронные DOM unit-тесты без таймеров и сети);
- **gate:worker:** в соответствии с `PROCESS.md` §3.14 / §5 воркер сдаёт targeted red/green и typecheck; полный batch-gate запускается на integration-лейне;
- **файлы (`md5sum`):**
  ```text
  d96a0920012a6f2f0e2b643a7dcf7711  rust/gui-spike/ui/features/search.js
  18c47c9c27897394c5fe410e4373e43e  rust/gui-spike/ui/styles/search.css
  2765033f99c860848c129a4077edd058  tests/unit/search-popover.test.mjs
  e388187553759c6f592220b92de13d77  TDD-PLAN.md
  ```
- **риски/вопросы:** отсутствуют; все существующие экспорты `rust/gui-spike/ui/features/search.js` сохранены без изменений, регрессионный сьют `tests/unit/search-remote-real-index.test.mjs` (33 теста) полностью зелёный;
- **следующий шаг:** передать задачу `TASK-618` на Product Review (PM) и Technical Review (DeepSeek).
