# ACCEPT — PR-172 (`task-292` / `FE-INT-15`: Подключение `createAppShellWiring` и `createComposerPopoversBridge` в `rust/gui-spike/ui/app.js`)

- **Статус PM-приёмки:** `ACCEPTED` (`Ready for Technical Review`)
- **Ветка:** `feature/task-292-fe-app-shell-mount`
- **Отдел:** `G2 Frontend (WebView UI & DOM/A11y)`
- **Модель исполнения:** `DeepSeek` (`deepseek/deepseek-v4.1-flash`, `L1 / P1`)

## Сверка с DoD
1. В `rust/gui-spike/ui/app.js` импортированы, смонтированы в `boot()` и реэкспортированы `createAppShellWiring` (`./features/app-shell-wiring.js`) и `createComposerPopoversBridge` (`./features/composer-popovers-bridge.js`) с прокидыванием `onDispatchIpc` → `invoke`.
2. В `tests/unit/app-shell-wiring.test.mjs` добавлены 3 теста, проверяющие подключение в `app.js` (`189/189 PASS` по целевым сьютам).
3. `npm run typecheck` и `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` — `0 errors`.
4. Передано на техническое ревью `TR-199A` (5/5 мутаций).
