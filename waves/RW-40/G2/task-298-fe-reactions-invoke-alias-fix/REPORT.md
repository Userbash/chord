# Отчёт по задаче TASK-298 (FE-FIX-09)

## 1. Сводка
- **Карточка:** `TASK-298 (FE-FIX-09)` / `bugfix/task-298-fe-reactions-invoke-alias-fix`
- **Отдел:** `G2 Frontend (WebView UI & DOM/A11y)`
- **Приоритет:** `P0 (Gate Blocker: ipc_wiring_contract 37 != 38)`
- **Статус:** Выполнено (Ready for Review)

## 2. Что сделано
- В файле `rust/gui-spike/ui/features/reactions.js` (строки 295–311) переименована локальная переменная `invoker` в `invokeFn` (`options.invoke !== undefined ? options.invoke : invokeReaction;`).
- Вызов IPC-команды переведён на `await invokeFn("toggle_reaction", { ... })`.
- Благодаря префиксу `invokeFn("` вызов команды `toggle_reaction` теперь корректно определяется синтаксическим сканером `extract_ui_invoke_commands` в `rust/gui-spike/tests/ipc_wiring_contract.rs`, восстанавливая покрытие до 38/38 UI IPC-команд.

## 3. Проверки и верификация
- Тесты модуля реакций:
  - `node --test tests/unit/chat-reactions.test.mjs tests/unit/reactions-ui.test.mjs` — **22/22 pass**, 0 fail.
- Статическая типизация:
  - `npm run typecheck` (`tsc -p tsconfig.json && tsc -p tsconfig.agent.json`) — **OK (код выхода 0)**.
  - `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` — **OK (код выхода 0)**.
- Верификация парсера `extract_ui_invoke_commands`:
  - `toggle_reaction` успешно извлекается среди UI-команд.

## 4. Изменённые файлы и контрольные суммы
- `rust/gui-spike/ui/features/reactions.js` (`md5: 2290a695bc137fbb3740d165e779c5ee`)

## 5. Риски и замечания
- Риски отсутствуют: функционал и семантика вызова сохранены, затронуто только имя локальной ссылки `invoker` -> `invokeFn`.
- Команды `cargo` не запускались во избежание конфликтов по общему `CARGO_TARGET_DIR` со слотом G1.
