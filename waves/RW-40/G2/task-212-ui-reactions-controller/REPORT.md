# TASK-212 — UI контроллер панели пилюль реакций и модалки реакций (`reactions-ui.js`, G2)

- **Карточка / Ветка**: `task-212` (`feature/task-212-ui-reactions-controller`)
- **Отдел**: G2 Frontend (WebView UI & DOM/A11y)
- **Оракулы**: `waves/RW-40/DESIGN/REACTIONS-EXTENDED-SPEC.md`, `waves/RW-40/PM/BACKLOG-RE-FRONTEND.md` (FE-P0-03, FE-P0-04), `rust/discord-client/src/http/reactions.rs` (`type=0` NORMAL, `type=1` BURST).

## Сделано
1. `tests/unit/reactions-ui.test.mjs` (Red → Green, 13 тестов в 4 suites, `// @ts-check` + JSDoc, с сохранением существующих тестов `FE-P0-04`):
   - проверка заморозки `REACTION_TYPES = Object.freeze({ NORMAL: 0, BURST: 1 })`;
   - проверка `renderReactionBar(reactions)`: рендер кнопок-пилюль с атрибутами `data-emoji`, `data-reaction-burst` (`'true'` при `burstCount > 0`, иначе `'false'`), `aria-pressed` (`'true'` при `me || meBurst`, иначе `'false'`), общим счётчиком `count + (burstCount || 0)`, цветами `burstColors` и вызовом `onToggleReaction(emoji, meBurst ? REACTION_TYPES.BURST : REACTION_TYPES.NORMAL, !(me || meBurst))` при клике;
   - проверка `openReactorsModal(messageId, reactionsWithUsers)`: рендер русского диалога `'Реакции'` (`role="dialog"`, `aria-modal="true"`) с вкладками фильтрации эмодзи (`role="tab"`, показывающими `emoji` + `count`), списком отреагировавших пользователей (`{ id, displayName, username, reactionType }`) с бейджем `'Суперреакция'` при `reactionType === 1`, переключением вкладок по клику и русским пустым состоянием `'Нет реакций'`.
2. `rust/gui-spike/ui/features/reactions-ui.js` (`// @ts-check` + JSDoc, ESM):
   - экспортированы `REACTION_TYPES`, `REACTORS_DIALOG_TITLE_RU`, `BURST_BADGE_LABEL_RU`, `REACTIONS_EMPTY_STATE_RU` и `createReactionsUiController(containerEl, options)`.

## Прогоны (Red / Green / Targeted / Gate)
- **Red**: `node --test tests/unit/reactions-ui.test.mjs` → `tests 1, pass 0, fail 1` (`ERR_MODULE_NOT_FOUND`), лог сохранён в `/home/sanya/orca/project/waves/RW-40/G2/task-212-ui-reactions-controller/red.log`.
- **Green**: `node --test tests/unit/reactions-ui.test.mjs` → `tests 13, suites 4, pass 13, fail 0`, лог сохранён в `/home/sanya/orca/project/waves/RW-40/G2/task-212-ui-reactions-controller/green.log`.
- **Typecheck (targeted)**: `npx tsc -p tsconfig.json` и `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` — `0 errors` в `rust/gui-spike/ui/features/reactions-ui.js` и `tests/unit/reactions-ui.test.mjs`.
- **Gate**: полный `gate:worker` не запускался (режим ТЗ карточки); чужие файлы не затронуты.

## Файлы и MD5
```
c2bd5e9c1469eb6747c8dffc117e97f6  rust/gui-spike/ui/features/reactions-ui.js
551b7dd2ddf2f200117bcef63db55c59  tests/unit/reactions-ui.test.mjs
b1ea10b8e065e27717221eafee0616c7  /home/sanya/orca/project/waves/RW-40/G2/task-212-ui-reactions-controller/red.log
8aa4d844bf19b37d9fde0021fa937b3d  /home/sanya/orca/project/waves/RW-40/G2/task-212-ui-reactions-controller/green.log
```

## Риски
- Монтаж `createReactionsUiController` в общий shell (`app.js`) выполняется отдельной интеграционной карточкой.
- `git commit` не выполнялся согласно правилам воркера.
