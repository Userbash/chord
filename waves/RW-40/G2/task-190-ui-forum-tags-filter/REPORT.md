# T-190 — UI forum tags filter (G2)

## Сделано
- `tests/unit/forum-filter.test.mjs` — 20 тестов (Red-first), контракт фильтра/сортировки тредов форума.
- `rust/gui-spike/ui/features/forum-filter.js` — `FORUM_SORT_MODES`, `filterAndSortForumThreads`, `countThreadsByTag`; `// @ts-check` + JSDoc, без DOM.

## Контракт (проверен тестами)
- `FORUM_SORT_MODES` заморожен: `latest_activity`, `creation_date`.
- Закреплённые треды всегда первые, порядок внутри групп — из сортировки.
- Теги: `any` (по умолчанию) / `all`; пустой `selectedTagIds` не фильтрует.
- `query` — `trim()` + регистронезависимая подстрока по `title`.
- Сортировка: `lastActivityMs ?? createdAtMs ?? 0` / `createdAtMs ?? 0` по убыванию, тай-брейкер `String(b.id).localeCompare(String(a.id))`.
- `countThreadsByTag` — обычный объект, дедупликация тега внутри треда.
- Входной массив не мутируется.

## Прогоны
- Red: `red.log` — модуль отсутствует, 1 файл-тест падает.
- Green: `green.log` — `node --test tests/unit/forum-filter.test.mjs`: 20/20 pass, 0 fail.

## Файлы и md5
- `rust/gui-spike/ui/features/forum-filter.js` — `acb667344ee3d1662ed8551c59667e56`
- `tests/unit/forum-filter.test.mjs` — `bfc7fb77f8a7988cbb76b78c8f107635`

## Риски / остаток
- Полный `gate:worker` и `tsc` не запускались (token-lean режим); UI-подключение панели фильтров вне scope T-190.
- Git commit не делался.
