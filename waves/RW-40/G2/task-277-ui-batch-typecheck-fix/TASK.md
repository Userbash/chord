# ТЗ — task-277 (`G2 Frontend Pass 2`: `FE-FIX-02` — Устранение 10 ошибок `tsc` в 7 файлах батча `Build-003`)

## Приоритет и Экономия токенов
- **Приоритет:** `P0` (блокер `npm run typecheck` из `QA-BUILD-003-REPORT.md` §5).
- **Экономия токенов:** НЕ ищи файлы по всему проекту и НЕ читай лишние модули. Работай СТРОГО по 7 перечисленным ниже файлам.

## Разрешённые файлы для записи (Write-Scope)
- `rust/gui-spike/ui/features/thread-badge.js`
- `tests/unit/thread-badge.test.mjs`
- `tests/unit/chat-helpers-bridge.test.mjs`
- `tests/unit/slowmode.test.mjs`
- `tests/unit/event-banner.test.mjs`
- `tests/unit/guild-boost.test.mjs`
- `tests/unit/poll-card.test.mjs`
- `/home/user/orca/project/waves/RW-40/G2/task-277-ui-batch-typecheck-fix/red.log`
- `/home/user/orca/project/waves/RW-40/G2/task-277-ui-batch-typecheck-fix/green.log`
- `/home/user/orca/project/waves/RW-40/G2/task-277-ui-batch-typecheck-fix/REPORT.md`

## Шаги выполнения (Strict TDD: Red -> Green)
1. **Red:** Запусти `npx tsc -p tsconfig.json --noEmit 2>&1 | grep -E "thread-badge|chat-helpers-bridge|slowmode\.test|event-banner\.test|guild-boost\.test|poll-card\.test"` и сохрани вывод (10 ошибок TS) в `/home/user/orca/project/waves/RW-40/G2/task-277-ui-batch-typecheck-fix/red.log`.
2. **Green:** Исправь JSDoc-аннотации и вызовы в этих 7 файлах (без ослабления проверок тестов и без `@ts-ignore`/`@ts-expect-error`, где проверяется валидный тип; для намеренно невалидных входов в негативных тестах используй `/** @type {any} */ (...)`).
3. **Проверка:**
   - `npx tsc -p tsconfig.json --noEmit 2>&1 | grep -E "thread-badge|chat-helpers-bridge|slowmode\.test|event-banner\.test|guild-boost\.test|poll-card\.test"` — должно быть **0 строк**!
   - `node --test tests/unit/thread-badge.test.mjs tests/unit/chat-helpers-bridge.test.mjs tests/unit/slowmode.test.mjs tests/unit/event-banner.test.mjs tests/unit/guild-boost.test.mjs tests/unit/poll-card.test.mjs` — все тесты `PASS`!
4. Сохрани `green.log` и краткий `REPORT.md` с `md5sum` 7 файлов, затем вызови `orca orchestration worker-done`. Не делай `git commit`.
