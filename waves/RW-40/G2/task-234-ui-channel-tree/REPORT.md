# task-234 — Guild Channel Category & Position Tree Sorter (FE-P2-26)

## Сделано
- `rust/gui-spike/ui/features/channel-tree.js`: `CHANNEL_TYPES` (frozen), `isVoiceLikeChannel`,
  `buildGuildChannelTree(channels, collapsedCategoryIds)`.
- `tests/unit/channel-tree.test.mjs`: 9 тестов, strict TDD (red -> green).

## Контракт
- Категории (`type === 4`) сортируются по `position`, затем по `id`.
- Внутри бакета (null = без категории, иначе `category.id`) сначала не-голосовые,
  потом voice-like (`2`, `13`), затем `position` по возрастанию, затем `id.localeCompare`.
- Свёрнутая категория: `collapsed: true`, `visibleChildren` — только `unread === true`,
  `allChildren` не фильтруется. Некатегорийные каналы не фильтруются.
- Канал с отсутствующей в payload категорией попадает в `uncategorized`, а не теряется.
- Входной массив не мутируется.

## Прогоны
- red: `node --test tests/unit/channel-tree.test.mjs` -> `exit=1`, `pass 0 / fail 1` (модуль отсутствовал) — `red.log`.
- green: тот же прогон -> `exit=0`, `tests 9 / pass 9 / fail 0` — `green.log`.
- `gate:worker` и полный набор тестов не запускались: token-lean режим ТЗ.

## Файлы и md5
- `rust/gui-spike/ui/features/channel-tree.js` — `04c1e353683249095d2c3c80e26326d0`
- `tests/unit/channel-tree.test.mjs` — `867f93dac8f055365a6df8f12c93456e`

## Риски
- Бакеты по `parentId` вложенности не поддерживают (Discord такого не отдаёт для guild-каналов).
- Коммит не делался, изменения лежат в рабочем дереве на ревью.
