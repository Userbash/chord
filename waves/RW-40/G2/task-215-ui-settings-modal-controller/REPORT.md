# TASK-215 — UI контроллер полноэкранного модального окна настроек (`settings-modal.js`, G2)

- **Карточка / Ветка**: `task-215` (`feature/task-215-ui-settings-modal-controller`)
- **Отдел**: G2 Frontend (WebView UI & DOM/A11y)
- **Оракулы**: `reference-oracle/ui8/23-settings-categories.json`, `reference-oracle/ui8/23-settings-voice.html`, `reference-oracle/frontend-d03.md` (§4.1–§4.2, §5 Modal).

## Сделано
1. `tests/unit/settings-modal.test.mjs` (Red → Green, 10 тестов в 5 suites, `// @ts-check` + JSDoc):
   - проверка заморозки и русских подписей `SETTINGS_TABS_RU`:
     - `{ id: 'appearance', labelRu: 'Внешний вид' }`
     - `{ id: 'voice', labelRu: 'Голос и видео' }`
     - `{ id: 'locale', labelRu: 'Язык' }`
     - `{ id: 'keybinds', labelRu: 'Горячие клавиши' }`
     - `{ id: 'notifications', labelRu: 'Уведомления' }`
   - проверка `open(initialTab = 'appearance')`: рендер полноэкранного диалога настроек (`role="dialog"`, `aria-modal="true"`, `data-settings-open="true"`) с элементами боковой навигации из `SETTINGS_TABS_RU`, активной панелью контента (`[role="tabpanel"]`, `data-settings-pane`), кнопкой `'Закрыть (ESC)'` (`[data-settings-close]`) и кнопкой `'Выйти из аккаунта'` (`[data-settings-logout]`);
   - проверка `setTab(tabId)`: переключение активной вкладки по всем 5 разделам и по клику в сайдбаре, обновление `aria-selected="true" | "false"`, игнорирование невалидных `tabId`;
   - проверка `updateSetting(key, value)`: обновление внутреннего состояния (`theme`, `fontScale`, `messageGroupSpacing`, `inputMode`, `vadThresholdDb`, `locale`, `desktopNotifications`), синхронизация DOM-контролов активной панели и вызов `onUpdateConfig({ ...state })` с защитной копией;
   - проверка `confirmLogout()` и `close()`: открытие диалога подтверждения `'Вы уверены, что хотите выйти?'` (`role="alertdialog"`, `data-logout-confirm="true"`), отмена без вызова `onLogout()`, подтверждение с вызовом `onLogout()`, удаление модального окна и вызов `onClose()` по `close()`, кнопке `'Закрыть (ESC)'` и клавише `Escape`.
2. `rust/gui-spike/ui/features/settings-modal.js` (`// @ts-check` + JSDoc):
   - экспортированы `SETTINGS_TABS_RU`, `DEFAULT_SETTINGS_CONFIG`, `CLOSE_SETTINGS_LABEL_RU`, `LOGOUT_BUTTON_LABEL_RU`, `LOGOUT_CONFIRM_PROMPT_RU` и `createSettingsModalController(containerEl, initialConfig, options)`.

## Прогоны (Red / Green / Targeted / Gate)
- **Red**: `node --test tests/unit/settings-modal.test.mjs` → `tests 1, pass 0, fail 1` (`ERR_MODULE_NOT_FOUND`), лог сохранён в `/home/user/orca/project/waves/RW-40/G2/task-215-ui-settings-modal-controller/red.log`.
- **Green**: `node --test tests/unit/settings-modal.test.mjs` → `tests 10, suites 5, pass 10, fail 0`, лог сохранён в `/home/user/orca/project/waves/RW-40/G2/task-215-ui-settings-modal-controller/green.log`.
- **Typecheck (targeted)**: проверка `rust/gui-spike/ui/features/settings-modal.js` и `tests/unit/settings-modal.test.mjs` через `npx tsc -p tsconfig.json --noEmit` — `0 errors`.
- **Gate**: полный `gate:worker` не запускался (режим ТЗ карточки); чужие файлы не затронуты.

## Файлы и MD5
```
dd3f783716ae3fc15f9ac2522d7564bf  rust/gui-spike/ui/features/settings-modal.js
deb64673bd4fdf2ea8d2cb53b58affe5  tests/unit/settings-modal.test.mjs
634fe077c2991e41508e9a82516844c1  /home/user/orca/project/waves/RW-40/G2/task-215-ui-settings-modal-controller/red.log
629d8e730232c78c5a584d5c70c41567  /home/user/orca/project/waves/RW-40/G2/task-215-ui-settings-modal-controller/green.log
```

## Риски
- Монтаж контроллера в общий shell (`app.js` / `screens.js`) находится вне write-scope данной карточки и выполняется в интеграционной задаче.
- Ветка не коммитилась и не пушилась согласно правилам воркера.
