# T-197 — Technical Review 108B (guild-folders.js, повторная)

Закрытие P1 из REVIEW-108A. Ветка `feature/task-197-ui-guild-folders`, коммитов нет.
Изменён только тест: `tests/unit/guild-folders.test.mjs` (md5 `b965c633284594b38a3312394337e5c6`).
Источник `rust/gui-spike/ui/features/guild-folders.js` не менялся — md5 `fbc40bd60c7fc28374da9c664ff70d49`.

## Прогон
`node --test tests/unit/guild-folders.test.mjs` → `tests 15, pass 15, fail 0`.
Новый кейс `does not mutate the input array on the append path`: проверяет `input` равен `["a"]`,
`next !== input`, `next` равен `["a", "b"]`.

## Мутации (5/5 killed)
| # | Мутация | Итог |
|---|---|---|
| M1 | `hasUnread` без условия `guildMentions > 0` | killed |
| M2 | `isPositiveInteger` без `Number.isInteger` | killed |
| M3 | `mentionCount > 99` → `>= 99` | killed |
| M4 | append-путь `return [...collapsed, folderId]` → `collapsed.push(...)` | killed |
| M5 | `mentionCount <= 0` → `< 0` | killed |

После прогона md5 источника совпадает с исходным — файл восстановлен.

## Вердикт
**APPROVED** — P0=0, P1=0, 5/5 мутаций killed. Можно к merge в `unstable` после CI.
