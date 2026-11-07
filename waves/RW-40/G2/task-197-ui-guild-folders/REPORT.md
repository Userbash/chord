# T-197 — UI guild folders (G2)

Ветка: `feature/task-197-ui-guild-folders`. Только свои файлы, коммитов нет.

## Сделано
- `computeGuildFolderState(folderGuildIds, guildStatesById)` — `guildCount`, `hasUnread`
  (`unread === true` или `mentionCount > 0`), `mentionCount` (сумма только положительных
  целых), `badgeText` (`''` / `String(n)` / `'99+'` при `> 99`), `hasActiveVoice`.
- `toggleFolderCollapsed(collapsedFolderIds, folderId)` — новый массив: снимает id или
  добавляет в конец; вход не мутируется.
- `GUILD_FOLDER_LIMITS.MENTION_BADGE_CAP = 99`.
- Модуль без DOM, `// @ts-check` + JSDoc.

## Прогоны
- Red: `red.log` — падение на импорте отсутствующего модуля (31 строка).
- Green: `green.log` — `tests 14, pass 14, fail 0`.
- `npx tsc` по файлу не запускался: tsconfig требует проектный прогон, гейт не в scope.

## Итерация 108B (после REVIEW-108A)
- Добавлен кейс `does not mutate the input array on the append path` (append-путь
  `toggleFolderCollapsed(["a"], "b")`: вход равен `["a"]`, результат — новый массив).
- `node --test tests/unit/guild-folders.test.mjs` → `tests 15, pass 15, fail 0`.
- Мутации: 5/5 killed (M1–M5, включая M4 append-путь), md5 источника после прогона
  совпадает с исходным — файл восстановлен.

## Файлы (md5)
- `fbc40bd60c7fc28374da9c664ff70d49` — `rust/gui-spike/ui/features/guild-folders.js`
- `b965c633284594b38a3312394337e5c6` — `tests/unit/guild-folders.test.mjs`

## Осталось
Монтаж в guild-rail (G2 UI) и полный `gate:worker` в integration-лейне.
