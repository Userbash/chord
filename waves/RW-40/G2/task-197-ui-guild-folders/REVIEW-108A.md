# T-197 — Technical Review 108A (guild-folders.js)

Ветка `feature/task-197-ui-guild-folders`, коммитов нет. Проверено: `rust/gui-spike/ui/features/guild-folders.js` (103 строки) + `tests/unit/guild-folders.test.mjs`.

## Прогон
`node --test tests/unit/guild-folders.test.mjs` → `tests 14, pass 14, fail 0`.
md5 до и после мутаций: `fbc40bd60c7fc28374da9c664ff70d49` — совпадает с REPORT.md, файл восстановлен.

## Мутации (4/5 killed)
| # | Мутация | Итог |
|---|---|---|
| M1 | `hasUnread` без условия `guildMentions > 0` | killed |
| M2 | `isPositiveInteger` без `Number.isInteger` (пропускает дроби) | killed |
| M3 | `mentionCount > 99` → `>= 99` | killed |
| M4 | append-путь `return [...collapsed, folderId]` → `collapsed.push(...)` | **SURVIVED** |
| M5 | `mentionCount <= 0` → `< 0` (ноль даёт badge `"0"`) | killed |

## P1 (блокирует merge)
M4 выжила: тест `does not mutate the input array` (строки 103–108) проверяет только removal-путь — вход `["a"]`, id `"a"`. Append-путь не покрыт, поэтому нарушение контракта «вход не мутируется» (шапка модуля) тестами не ловится. Нужен кейс:
```js
const input = ["a"];
const next = toggleFolderCollapsed(input, "b");
assert.deepEqual(input, ["a"]);
assert.notEqual(next, input);
```
Правка теста — зона воркера G2, не ревьюера.

## Прочее
P0: 0. Замечаний по читаемости, JSDoc, `@ts-check`, отсутствию DOM — нет.

## Вердикт
**CHANGES REQUESTED** — P1=1 (4/5 мутаций). APPROVED невозможен до добавления append-кейса на иммутабельность и повторного прогона 5/5.
