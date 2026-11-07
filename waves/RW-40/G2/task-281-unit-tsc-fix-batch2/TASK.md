# ТЗ — task-281 (`G2 Frontend L1 / P0`: `FE-FIX-05` — Устранение ошибок `tsc` в `svg-icons.test.mjs` и `channel-tree.test.mjs`)

## Приоритет, Сложность и Экономия токенов
- **Приоритет:** `P0` | **Сложность:** `L1` (атомарная правка типов JSDoc в 2 тестовых файлах).
- **Модель:** `deepseek/deepseek-v4.1-flash` (`--agent opencode`).
- **Экономия токенов:** НЕ читай другие файлы проекта! Работай СТРОГО только с `tests/unit/svg-icons.test.mjs` и `tests/unit/channel-tree.test.mjs`.

## Разрешённые файлы для записи (Write-Scope)
- `tests/unit/svg-icons.test.mjs`
- `tests/unit/channel-tree.test.mjs`
- `/home/sanya/orca/project/waves/RW-40/G2/task-281-unit-tsc-fix-batch2/red.log`
- `/home/sanya/orca/project/waves/RW-40/G2/task-281-unit-tsc-fix-batch2/green.log`
- `/home/sanya/orca/project/waves/RW-40/G2/task-281-unit-tsc-fix-batch2/REPORT.md`

## Шаги выполнения (Strict TDD: Red -> Green)
1. **Red:** Запусти `npx tsc -p tsconfig.json --noEmit 2>&1 | grep -E "svg-icons\.test\.mjs|channel-tree\.test\.mjs"` и сохрани вывод (44 ошибки) в `red.log`.
2. **Green:** Добавь аккуратные JSDoc-приведения типов (`/** @type {any} */` для негативных входов и DOM-моков или проверку на `undefined`), не меняя логику `assert.*` и не добавляя `@ts-ignore`.
3. **Проверка:**
   - `npx tsc -p tsconfig.json --noEmit 2>&1 | grep -E "svg-icons\.test\.mjs|channel-tree\.test\.mjs"` — **0 строк** (`green.log`)!
   - `node --test tests/unit/svg-icons.test.mjs tests/unit/channel-tree.test.mjs` — все тесты `PASS`!
4. Запиши краткий `REPORT.md` с `md5sum` 2 файлов и вызови `orca orchestration worker-done`. Не делай `git commit`.
