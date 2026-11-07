# REVIEW-121A — task-204 UI search panel controller

**Verdict: APPROVED.** P0 = 0, P1 = 0. Мутации: 5/5 убиты.

## Проверено
- Файл: `rust/gui-spike/ui/features/search-panel.js` (565 строк, `// @ts-check` + JSDoc, ESM).
- Целевой прогон `node --test tests/unit/search-panel.test.mjs`: `tests 10, pass 10, fail 0` (3 сьюта).
- Контракт совпадает с отчётом: `parseSearchQueryTokens` (`from`/`in`/`has`/`mentions`/`pinned` + свободный текст), `clampSearchOffset` в `0..=9975`, `SEARCH_PAGE_STEP = 25`, замороженные русские подсказки, `NEW`/`OLD`/`RELEVANT`-вкладки, `Найдено результатов: N` / `Ничего не найдено`, `onJumpToMessage`, `aria-disabled` на границах offset.

## Мутации (single-pass, файл восстановлен после каждой)
| # | Мутация | Итог |
|---|---------|------|
| M1 | `MAX_SEARCH_OFFSET` 9975 → 10000 | KILLED |
| M2 | `SEARCH_PAGE_STEP` 25 → 20 | KILLED |
| M3 | `has:` без фильтра `ALLOWED_HAS_TOKENS` | KILLED |
| M4 | `pinned:false` → `pinned = true` | KILLED |
| M5 | `from:` пишется в `channel` вместо `author` | KILLED |

## Восстановление
- `md5sum rust/gui-spike/ui/features/search-panel.js` → `8dffe69f992cfe42f73f7ff7df1e5485`, совпадает с REPORT.md.
- Повторный прогон после восстановления: `tests 10, pass 10, fail 0`.
- `git commit` не выполнялся; изменён только этот файл ревью.
