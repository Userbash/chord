# TASK-220 (FE-P1-20) — UI контроллер списка участников гильдии, ролевых групп и сайдбара присутствия (`member-list.js`, G2)

- **Карточка / Ветка**: `task-220` (`feature/task-220-ui-member-list`)
- **Отдел**: G2 Frontend (WebView UI & DOM/A11y)
- **Оракулы**: `/home/sanya/re-deep/frontend-d03.md` (§3 «Участники (правый сайдбар)»: `aside[class*=membersWrap]`, группы `NAME — COUNT`, строки участников), `/home/sanya/re-deep/ui8/22-members.json`.

## Сделано
1. `tests/unit/member-list.test.mjs` (Red → Green, 7 тестов в 2 suites, `// @ts-check` + JSDoc, минимальный DOM-шим):
   - проверка заморозки и состава `DEFAULT_GROUP_LABELS_RU = Object.freeze({ online: 'В сети', offline: 'Не в сети' })`;
   - проверка `groupMembersByHoistedRole(members, roles)`:
     - распределение участников со статусами `'online' | 'idle' | 'dnd'` в корзину самой старшей по `position` hoisted-роли (`role.hoist === true`, сортировка групп по `role.position` по убыванию) либо в корзину `'online'` (`'В сети'`), если hoisted-ролей у участника нет;
     - распределение участников со `status === 'offline'` или отсутствующим статусом в корзину `'offline'` (`'Не в сети'`) независимо от наличия hoisted-ролей;
     - алфавитная сортировка внутри каждой корзины по `(displayName || username)` с использованием `localeCompare('ru')`;
     - фильтрация пустых групп и отсутствие мутации входных массивов `members` и `roles`;
   - проверка `createMemberListController({ container, onSelectMember })`:
     - рендер сайдбара `aside[data-member-list]` с заголовками ролевых групп `[data-member-group-header]` (`'NAME — COUNT'`) и элементами участников `[data-member-item]` (отображаемое имя, HEX-цвет старшей цветной роли `#rrggbb` с ведущими нулями, точка статуса `[data-status]`, бейдж короны владельца `[data-owner-badge]` только при `isOwner === true`, текст `customStatus`);
     - динамическое обновление списка через `controller.render({ members, roles })`;
     - вызов `onSelectMember({ userId, anchorElement })` по клику на элемент участника `[data-member-item]`;
     - переключение видимости через `toggleVisibility()` с синхронизацией атрибута `hidden` и возвратом булева состояния видимости.
2. `rust/gui-spike/ui/features/member-list.js` (`// @ts-check` + JSDoc, ESM):
   - экспортированы `DEFAULT_GROUP_LABELS_RU`, `groupMembersByHoistedRole` и `createMemberListController`.

## Прогоны (Red / Green / Targeted / Gate)
- **Red**: `node --test tests/unit/member-list.test.mjs` → `tests 1, pass 0, fail 1` (`ERR_MODULE_NOT_FOUND`), лог сохранён в `/home/sanya/orca/project/waves/RW-40/G2/task-220-ui-member-list/red.log`.
- **Green**: `node --test tests/unit/member-list.test.mjs` → `tests 7, suites 2, pass 7, fail 0`, лог сохранён в `/home/sanya/orca/project/waves/RW-40/G2/task-220-ui-member-list/green.log`.
- **Typecheck (targeted)**: `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` (`0 errors`) и `0 errors` в `member-list.js` / `member-list.test.mjs` при проверке `npx tsc -p tsconfig.json --noEmit`.
- **Gate**: полный `gate:worker` не запускался (режим ТЗ карточки); чужие файлы не затронуты.

## Файлы и MD5
```
e91643fa1484861b4f60d1dee992b026  rust/gui-spike/ui/features/member-list.js
60cf19aaf0219e548c98b7646dcb5558  tests/unit/member-list.test.mjs
56bf942f6ce3da61a584e5b2f272fa88  /home/sanya/orca/project/waves/RW-40/G2/task-220-ui-member-list/red.log
08e4edb6648105f21b28ead4bce491cf  /home/sanya/orca/project/waves/RW-40/G2/task-220-ui-member-list/green.log
```

## Риски
- Монтаж `createMemberListController` в основной shell чата (`app.js` / `screens.js`) находится вне write-scope данной карточки и выполняется отдельной интеграционной задачей.
- `git commit` не выполнялся согласно правилам воркера.
