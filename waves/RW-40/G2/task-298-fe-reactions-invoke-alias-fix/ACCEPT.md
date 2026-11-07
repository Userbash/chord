# ACCEPT — PR-177 (`task-298` / `FE-FIX-09`: Восстановление 38/38 UI IPC-команд `toggle_reaction` в `reactions.js`)

- **Статус PM-приёмки:** `ACCEPTED` (`Ready for Merge`)
- **Ветка:** `bugfix/task-298-fe-reactions-invoke-alias-fix`
- **Отдел:** `G2 Frontend (WebView UI & DOM/A11y)`
- **Модель исполнения:** `Antigravity Flash Low` (`gemini-3.8-flash-low`, `--effort low`, `L1 / P0`)

## Сверка с DoD
1. В `rust/gui-spike/ui/features/reactions.js` (строки 295–311) локальная переменная переименована с `invoker` на `invokeFn`, благодаря чему `extract_ui_invoke_commands` в `rust/gui-spike/tests/ipc_wiring_contract.rs` снова видит вызов `await invokeFn("toggle_reaction", ...)` и все 38/38 UI IPC-команд.
2. `node --test tests/unit/reactions.test.mjs tests/unit/reactions-picker.test.mjs` (`22/22 PASS`), `npm run typecheck` и `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` — `0 errors`.
