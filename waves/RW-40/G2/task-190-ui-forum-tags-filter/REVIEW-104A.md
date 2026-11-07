# REVIEW-104A — T-190 forum tags filter (technical review)

Вердикт: **APPROVED** (P0 = 0, P1 = 0, мутации 5/5 killed).

## Проверено
- Targeted-прогон: `node --test tests/unit/forum-filter.test.mjs` → 20/20 pass, 0 fail, 0 skipped (44 ms).
- Код `rust/gui-spike/ui/features/forum-filter.js`: чистые функции без DOM, `// @ts-check` + JSDoc, вход не мутируется (`filter` создаёт копию перед `sort`).
- Контракт совпадает с REPORT.md: закреплённые первыми, `any`/`all`, `trim()`+регистронезависимая подстрока, `lastActivityMs ?? createdAtMs ?? 0` / `createdAtMs ?? 0` по убыванию, тай-брейкер `String(b.id).localeCompare(String(a.id))`, дедупликация тегов в `countThreadsByTag`.

## Мутации (5/5 killed)
1. `every` → `some` в режиме `all` — fail 2.
2. Инверсия порядка закреплённых (`aPinned - bPinned`) — fail 2.
3. Инверсия направления сортировки — fail 9.
4. Снятие `toLowerCase()` в `matchesQuery` — fail 2.
5. Снятие `seen.has(tagId)` в `countThreadsByTag` — fail 1.

Оригинал восстановлен после каждой мутации; `md5 = acb667344ee3d1662ed8551c59667e56` совпадает с REPORT.md.

## Замечания вне блокировки
- `countThreadsByTag` на ключе `__proto__` теряет счётчик (обычный объект). Реальные ID тегов — snowflake, риск P3, контрактом не покрыт.
- `gate:worker`/`tsc` не запускались по режиму token-lean; UI-подключение панели вне scope T-190.

Коммитов не делал.
