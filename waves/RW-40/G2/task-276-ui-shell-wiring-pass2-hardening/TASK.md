# ТЗ — task-276 (`G2 Frontend Pass 2`: `FE-FIX-01` — Hardening `app-shell-wiring.js` по замечаниям `REVIEW-184A` и `tsconfig.json`)

## Роль и цель
Ты воркер отдела `G2 Frontend` (Pass 2 рефакторинг по результатам технического ревью `REVIEW-184A.md` и отчёта QA `QA-BUILD-003-REPORT.md`).

## Разрешённые файлы для записи (Write-Scope)
- `rust/gui-spike/ui/features/app-shell-wiring.js`
- `tests/unit/app-shell-wiring.test.mjs`
- `rust/gui-spike/ui/tsconfig.json`
- `/home/user/orca/project/waves/RW-40/G2/task-276-ui-shell-wiring-pass2-hardening/red.log`
- `/home/user/orca/project/waves/RW-40/G2/task-276-ui-shell-wiring-pass2-hardening/green.log`
- `/home/user/orca/project/waves/RW-40/G2/task-276-ui-shell-wiring-pass2-hardening/REPORT.md`

## Требования (Strict TDD: Red -> Green)
1. **Сначала добавь тесты (Red) в `tests/unit/app-shell-wiring.test.mjs` и запиши `red.log`:**
   - При вызове `openAdminPanel("invalid_panel")` с невалидным ID уже открытые модальные слои НЕ должны закрываться (валидация до побочных эффектов).
   - При вызове `openModalLayer("invalid_layer")` с невалидным ID активная админ-панель НЕ должна закрываться.
   - После вызова `destroy()` повторные вызовы `openAdminPanel(...)`, `openModalLayer(...)` и `dispatchAdminAction(...)` выбрасывают `Error` (или отклоняются без открытия слоёв).
   - Объект, возвращаемый `getState()`, и вложенный массив `activeModalStack` заморожены (`Object.isFrozen(state) === true` и `Object.isFrozen(state.activeModalStack) === true`).
2. **Затем внеси минимальный код (Green) в `rust/gui-spike/ui/features/app-shell-wiring.js` и обнови `rust/gui-spike/ui/tsconfig.json`:**
   - В `rust/gui-spike/ui/tsconfig.json` убедись, что `"include"` покрывает `"features/**/*.js"` (не ломая `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit`). Если какой-то файл в `features/*.js` даёт ошибку JSDoc-типизации при включении `features/**/*.js`, аккуратно добавь только `features/app-shell-wiring.js`, `features/shell-controllers-registry.js`, `features/chat-helpers-bridge.js` в массив `include`, чтобы `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` проходил с `0 errors`.
3. Запиши `green.log` и `REPORT.md` с `md5sum` всех изменённых файлов и заверши задачу через `orca orchestration worker-done`. Не делай `git commit`.
