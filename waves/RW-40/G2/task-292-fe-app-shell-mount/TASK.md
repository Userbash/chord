# TASK — task-292 (`G2 Frontend L1 / P1`, `FE-INT-15`: Монтирование `createAppShellWiring` и `createComposerPopoversBridge` в `rust/gui-spike/ui/app.js`)

- **Отдел:** G2 Frontend (WebView UI & DOM/A11y) (`DeepSeek` — `--agent opencode --model deepseek/deepseek-v4.1-flash`)
- **Уровень сложности / Приоритет:** `L1 / P1` (Фаза 0 из `RELEASE-NOTES-005.md`, по `waves/RW-40/PM/MODEL-ROUTING-CRITERIA.md`)
- **Рабочее дерево:** `/home/user/orca/workspaces/project/feature-task-292-fe-app-shell-mount` (`branch:feature/task-292-fe-app-shell-mount`)
- **Разрешённые к изменению файлы (строго 2 файла):**
  1. `rust/gui-spike/ui/app.js` (импорт на строках 52–53, инициализация в `boot()` на строках 3292–3323, реэкспорт на строке 3435)
  2. `tests/unit/app-shell-wiring.test.mjs` (добавить тест, проверяющий, что `rust/gui-spike/ui/app.js` импортирует и монтирует `createAppShellWiring` и `createComposerPopoversBridge` в возвращаемом объекте `boot()`)
- **Артефакты отчёта:**
  - `/home/user/orca/project/waves/RW-40/G2/task-292-fe-app-shell-mount/REPORT.md`
  - `/home/user/orca/project/waves/RW-40/G2/task-292-fe-app-shell-mount/red.log`
  - `/home/user/orca/project/waves/RW-40/G2/task-292-fe-app-shell-mount/green.log`

## Точное описание задачи (без поиска по репозиторию)

1. Перейди в `/home/user/orca/workspaces/project/feature-task-292-fe-app-shell-mount`.
2. **Strict TDD (сначала тест в `tests/unit/app-shell-wiring.test.mjs` -> `red.log`):**
   - В `tests/unit/app-shell-wiring.test.mjs` добавь тест:
     - проверяющий по исходнику `rust/gui-spike/ui/app.js` (через `node:fs/promises` `readFile`), что `app.js` импортирует `createAppShellWiring` из `./features/app-shell-wiring.js` и `createComposerPopoversBridge` из `./features/composer-popovers-bridge.js`, создаёт экземпляры `shellWiring` и `composerPopoversBridge` внутри `boot()` (прокидывая `onDispatchIpc: (command, payload) => invoke(command, /** @type {Record<string, unknown>} */ (payload))` в оба моста), возвращает `shellWiring` и `composerPopoversBridge` в объекте `boot()`, и реэкспортирует `createAppShellWiring` и `createComposerPopoversBridge` в `export { ... }` в конце `app.js`.
   - Запусти `node --test tests/unit/app-shell-wiring.test.mjs` и сохрани падение в `/home/user/orca/project/waves/RW-40/G2/task-292-fe-app-shell-mount/red.log`.
3. **Минимальная реализация в `rust/gui-spike/ui/app.js`:**
   - На строке 53 (после `import { createGuildModalsController } from "./features/guild-modals.js";`) добавь:
     ```js
     import { createAppShellWiring } from "./features/app-shell-wiring.js";
     import { createComposerPopoversBridge } from "./features/composer-popovers-bridge.js";
     ```
   - В функции `boot()` перед `return {` (около строки 3292) создай:
     ```js
     const shellWiring = createAppShellWiring({
       onDispatchIpc: (command, payload) =>
         invoke(command, /** @type {Record<string, unknown> | undefined} */ (payload)),
     });
     const composerPopoversBridge = createComposerPopoversBridge({
       onDispatchIpc: (command, payload) =>
         invoke(command, /** @type {Record<string, unknown> | undefined} */ (payload)),
     });
     ```
     и добавь `shellWiring,` и `composerPopoversBridge,` в возвращаемый объект `boot()` (после `guildModals,`).
   - В последней строке `rust/gui-spike/ui/app.js` обнови `export`:
     ```js
     export {
       createUserPopoutController,
       createGuildModalsController,
       createAppShellWiring,
       createComposerPopoversBridge,
     };
     ```
4. **Green-прогон:**
   - `node --test tests/unit/app-shell-wiring.test.mjs tests/unit/gui-spike-dom.test.mjs tests/unit/gui-spike-features.test.mjs`
   - `npx tsc -p tsconfig.json --noEmit` (`0 errors`)
   - `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` (`0 errors`)
   Сохрани вывод в `/home/user/orca/project/waves/RW-40/G2/task-292-fe-app-shell-mount/green.log`.
5. Запиши `/home/user/orca/project/waves/RW-40/G2/task-292-fe-app-shell-mount/REPORT.md` (с `md5sum` обоих файлов) и заверши через `orca orchestration complete`. Не делай `git commit/push`.
