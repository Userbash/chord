# TASK-232 (FE-P1-25) — UI контроллер домашнего экрана личных сообщений и друзей: В сети, Все, Ожидание, Заблокированные и Добавить в друзья (`friends-home.js`, G2)

- **Карточка / Ветка**: `task-232` (`feature/task-232-ui-friends-home`)
- **Отдел**: G2 Frontend (WebView UI & DOM/A11y)
- **Оракулы**: `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0@258005` (`RelationshipType`: `FRIEND = 1`, `BLOCKED = 2`, `PENDING_INCOMING = 3`, `PENDING_OUTGOING = 4`), `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0@243700` (`FRIENDS_SECTIONS`: `ONLINE`, `ALL`, `PENDING`, `BLOCKED`, `ADD_FRIEND`), `waves/RW-40/DM-FRIENDS-RE.md` (§4–§6), `waves/RW-40/DESIGN/DM-FRIENDS-SPEC.md` (§2.3, §4).

## Сделано
1. `tests/unit/friends-home.test.mjs` (Red → Green, 7 тестов в 2 suites, `// @ts-check` + JSDoc, минимальный DOM-шим):
   - проверка экспорта и заморозки `RELATIONSHIP_TYPES = Object.freeze({ FRIEND: 1, BLOCKED: 2, PENDING_INCOMING: 3, PENDING_OUTGOING: 4 })` и `FRIENDS_TABS_RU = Object.freeze({ online: "В сети", all: "Все", pending: "Ожидание", blocked: "Заблокированные", add: "Добавить в друзья" })`;
   - проверка `filterRelationshipsByTab(relationships, tab, searchQuery)`:
     - для `tab === "online"`: оставляет только `type === 1` (`FRIEND`) и `status !== "offline"` (`"online" | "idle" | "dnd"`);
     - для `tab === "all"`: оставляет все записи с `type === 1`;
     - для `tab === "pending"`: оставляет `type === 3` (`PENDING_INCOMING`) и `type === 4` (`PENDING_OUTGOING`);
     - для `tab === "blocked"`: оставляет `type === 2` (`BLOCKED`);
     - регистронезависимая фильтрация по `searchQuery` на `(displayName || username)`, алфавитная сортировка (`localeCompare("ru")`) и отсутствие мутации входного массива;
   - проверка `createFriendsHomeController({ container, onOpenDm, onAcceptRequest, onRemoveOrBlock, onSendFriendRequest })`:
     - рендер корня `[data-friends-home]`, панели вкладок `[data-friends-tab]` с `aria-selected="true"` на активной вкладке, бейджа входящих заявок `[data-friends-pending-badge]` (считающего только `type === 3`), поля поиска `[data-friends-search]` и списка `[data-friends-list]` со строками `[data-friend-row]` (отображающими `displayName || username`, `status` и кнопки `[data-friend-dm-btn]`, `[data-friend-accept-btn]`, `[data-friend-remove-btn]`);
     - рендер формы `[data-add-friend-form]` с полем `[data-add-friend-input]` и кнопкой `[data-add-friend-submit]` на вкладке `tab === "add"`, вызывающей `onSendFriendRequest(username.trim())` для непустого ввода.
2. `rust/gui-spike/ui/features/friends-home.js` (`// @ts-check` + JSDoc, ESM):
   - экспортированы `RELATIONSHIP_TYPES`, `FRIENDS_TABS_RU`, `filterRelationshipsByTab` и `createFriendsHomeController`.

## Прогоны (Red / Green / Targeted / Gate)
- **Red**: `node --test tests/unit/friends-home.test.mjs` → `tests 1, pass 0, fail 1` (`ERR_MODULE_NOT_FOUND`), лог сохранён в `/home/sanya/orca/project/waves/RW-40/G2/task-232-ui-friends-home/red.log`.
- **Green**: `node --test tests/unit/friends-home.test.mjs` → `tests 7, suites 2, pass 7, fail 0`, лог сохранён в `/home/sanya/orca/project/waves/RW-40/G2/task-232-ui-friends-home/green.log`.
- **Typecheck (targeted)**:
  - `npx tsc --ignoreConfig --noEmit --strict --allowJs --checkJs --target es2022 --module node20 --moduleResolution node16 --types node rust/gui-spike/ui/features/friends-home.js tests/unit/friends-home.test.mjs` — `0 errors`;
  - `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` — `0 errors`.
- **Gate**: полный `gate:worker` не запускался (режим ТЗ карточки); чужие файлы не затронуты.

## Файлы и MD5
```
859219bd3678198921343ce71e26dd79  rust/gui-spike/ui/features/friends-home.js
34e858b2feb44ceea94d1f712e5c7231  tests/unit/friends-home.test.mjs
fb76c4d4f56b18712c3e0196f53d4916  /home/sanya/orca/project/waves/RW-40/G2/task-232-ui-friends-home/red.log
5466e83f37d5d899760f3b15ab7dd411  /home/sanya/orca/project/waves/RW-40/G2/task-232-ui-friends-home/green.log
```

## Риски
- Монтаж `createFriendsHomeController` в общий shell (`app.js`) выполняется отдельной интеграционной задачей вне write-scope данной карточки.
- `git commit` не выполнялся согласно правилам воркера.
