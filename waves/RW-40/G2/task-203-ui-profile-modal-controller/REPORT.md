# TASK-203 — UI контроллер поповера и модального окна профиля (`profile-modal.js`, G2)

- **Карточка / Ветка**: `task-203` (`feature/task-203-ui-profile-modal-controller`)
- **Отдел**: G2 Frontend (WebView UI & DOM/A11y)
- **Оракулы**: `/home/sanya/re-deep/ui/06-profile-popout.html`, `/home/sanya/re-deep/voice/ui7/15-popout.json`, `/home/sanya/re-deep/frontend-d03.md` (§5 Modal / Popover).

## Сделано
1. `tests/unit/profile-modal.test.mjs` (Red → Green, 12 тестов в 4 suites, `// @ts-check` + JSDoc):
   - проверка заморозки и русских подписей `STATUS_LABELS_RU` (`online: 'В сети'`, `idle: 'Не активен'`, `dnd: 'Не беспокоить'`, `offline: 'Не в сети'`);
   - проверка `openPopout(profileData)`: рендер карточки `data-profile-mode="popout"` с `displayName`, `username`, бейджем местоимений (`[data-profile-pronouns]`), русским статусом из `STATUS_LABELS_RU` (`[data-profile-status]`), текстом биографии/о себе (`[data-profile-bio]`), пилюлями ролей (`[data-role-id]`, `data-role-color` и `style.color`), инпутом личной заметки (`[data-profile-note-input]`, вызывающим `onSaveNote(userId, noteText)` на `change`) и инпутом быстрого ЛС (`placeholder="Написать @" + username`, вызывающим `onSendQuickMessage(userId, text)` только при непустом тексте);
   - проверка `openFullModal(profileData)` и `setTab(tabId)`: рендер диалога (`role="dialog"`, `aria-modal="true"`, `data-profile-mode="modal"`) с 3 вкладками (`Обо мне` / `about`, `Общие серверы` / `mutual_guilds` со счётчиком, `Общие друзья` / `mutual_friends` со счётчиком), переключение вкладок через `setTab(tabId)` и по клику, рендер списков и русских пустых состояний (`Нет общих серверов`, `Нет общих друзей`);
   - проверка `close()`, идемпотентности повторного закрытия и валидации `containerEl`.
2. `rust/gui-spike/ui/features/profile-modal.js` (`// @ts-check` + JSDoc):
   - экспортированы `STATUS_LABELS_RU`, `PROFILE_MODAL_TABS`, `PROFILE_TAB_LABELS_RU`, `PROFILE_EMPTY_STATES_RU` и `createProfileModalController(containerEl, options)`.

## Прогоны (Red / Green / Targeted / Gate)
- **Red**: `node --test tests/unit/profile-modal.test.mjs` → `tests 1, pass 0, fail 1` (`ERR_MODULE_NOT_FOUND`), лог сохранён в `/home/sanya/orca/project/waves/RW-40/G2/task-203-ui-profile-modal-controller/red.log`.
- **Green**: `node --test tests/unit/profile-modal.test.mjs` → `tests 12, suites 4, pass 12, fail 0`, лог сохранён в `/home/sanya/orca/project/waves/RW-40/G2/task-203-ui-profile-modal-controller/green.log`.
- **Typecheck (targeted)**: `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` и проверка `profile-modal.js` + `profile-modal.test.mjs` — `0 errors`.
- **Gate**: полный `gate:worker` не запускался (режим ТЗ карточки); чужие файлы не затронуты.

## Файлы и MD5
```
1518ab1b8972591a49746291b1e21400  rust/gui-spike/ui/features/profile-modal.js
cbf7f0437e4b523c9afff3305f93e375  tests/unit/profile-modal.test.mjs
66bb9159f57c485aec905b3a1b9efe1e  /home/sanya/orca/project/waves/RW-40/G2/task-203-ui-profile-modal-controller/red.log
c2694c27b13547a05e1b5754b024fd85  /home/sanya/orca/project/waves/RW-40/G2/task-203-ui-profile-modal-controller/green.log
```

## Риски
- Монтаж контроллера в общий shell (`app.js` / `screens.js`) находится вне write-scope данной карточки и выполняется в интеграционной задаче.
- Ветка не коммитилась и не пушилась согласно правилам воркера.
