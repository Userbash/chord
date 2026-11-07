# TASK-236 (FE-P0-09) — Реестр фич оболочки UI и единый мост контроллеров (`shell-bridge.js`, G2)

- **Карточка / Ветка**: `task-236` (`feature/task-236-ui-shell-bridge`)
- **Отдел**: G2 Frontend (WebView UI & DOM/A11y)
- **Write-Scope**:
  - `rust/gui-spike/ui/features/shell-bridge.js`
  - `tests/unit/shell-bridge.test.mjs`
  - `/home/sanya/orca/project/waves/RW-40/G2/task-236-ui-shell-bridge/REPORT.md` (`red.log`, `green.log`)

## 1. Сделано

1. **`tests/unit/shell-bridge.test.mjs`** (Strict TDD Red → Green, 7 тестов в 2 suites, `// @ts-check` + JSDoc):
   - проверка экспорта и заморозки `SHELL_MODAL_LAYERS` (`10` слоёв в каноническом порядке: `"quick-switcher"`, `"settings-modal"`, `"profile-modal"`, `"invite-modal"`, `"forward-modal"`, `"expression-picker"`, `"context-menu"`, `"threads-drawer"`, `"search-panel"`, `"inbox-popover"`);
   - проверка `registerController(layerId, controllerInstance)` и отклонения невалидных `layerId`;
   - проверка `openLayer(layerId, payload)` с передачей `payload` в `controller.open?.(payload)`, добавлением `layerId` в `activeOverlayStack` и взаимным исключением 5 модальных окон (`"quick-switcher"`, `"settings-modal"`, `"profile-modal"`, `"invite-modal"`, `"forward-modal"`) при сохранении открытых боковых панелей/поповеров;
   - проверка `closeTopLayer()` и `closeAllLayers()` в строгом порядке LIFO с вызовом `controller.close?.()`;
   - проверка `handleGlobalKeyDown(event)` через `normalizeKeyCombo(event)` из `./keybinds.js` (`"Ctrl+K"` → `"quick-switcher"`, `"Ctrl+F"` → `"search-panel"`, `"Escape"` → `closeTopLayer()`, возврат `boolean`);
   - проверка делегирования `dispatchIpc` (`onDispatchIpc`) и `navigate` (`onNavigate`).
2. **`rust/gui-spike/ui/features/shell-bridge.js`** (`// @ts-check` + JSDoc, ESM):
   - экспортированы `SHELL_MODAL_LAYERS`, `EXCLUSIVE_MODAL_LAYERS` и `createShellFeatureBridge({ onDispatchIpc, onNavigate })`.

## 2. Прогоны (Red / Green / Targeted / Gate)

- **Red**: `node --test tests/unit/shell-bridge.test.mjs` → exit 1 (`tests 1, pass 0, fail 1`, `ERR_MODULE_NOT_FOUND` до создания модуля), лог сохранён в `/home/sanya/orca/project/waves/RW-40/G2/task-236-ui-shell-bridge/red.log`.
- **Green**: `node --test tests/unit/shell-bridge.test.mjs` → exit 0 (`tests 7, suites 2, pass 7, fail 0`), лог сохранён в `/home/sanya/orca/project/waves/RW-40/G2/task-236-ui-shell-bridge/green.log`.
- **Targeted & Typecheck**: `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` (`0 errors`), `0 errors` в `shell-bridge.js` и `shell-bridge.test.mjs` при проверке `npx tsc -p tsconfig.json --noEmit`.
- **Gate**: полный `gate:worker` не запускался (режим воркера в batch); чужие файлы не затрагивались.

## 3. Файлы и MD5

```
3389f3aad9c5265135ba5876daea149d  rust/gui-spike/ui/features/shell-bridge.js
70f337e6960ec99be7d168d0029ecd82  tests/unit/shell-bridge.test.mjs
0e5af0972ac488b9abbe85df14309433  /home/sanya/orca/project/waves/RW-40/G2/task-236-ui-shell-bridge/red.log
af300dd1c3a70eabe26a2b6adcffa5f9  /home/sanya/orca/project/waves/RW-40/G2/task-236-ui-shell-bridge/green.log
```

## 4. Риски и примечания

- Подключение `createShellFeatureBridge` к корневому `app.js` / DOM-контейнерам оболочки выполняется отдельной интеграционной карточкой.
- `git commit` не выполнялся в соответствии с правилами воркера.
