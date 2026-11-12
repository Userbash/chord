# ТЗ — task-282 (`G2 Frontend L1 / P0`: `FE-FIX-06` — Устранение оставшихся 34 ошибок `tsc` в 10 файлах `tests/unit/*.test.mjs`)

## Приоритет, Сложность и Экономия токенов
- **Приоритет:** `P0` | **Сложность:** `L1` (атомарная правка типов JSDoc в 10 тестовых файлах).
- **Модель:** `deepseek/deepseek-v4.1-flash` (`--agent opencode`).
- **Экономия токенов:** НЕ сканируй репозиторий и НЕ читай посторонние файлы! Работай СТРОГО только с 10 перечисленными ниже тестовыми файлами.

## Разрешённые файлы для записи (Write-Scope)
- `tests/unit/locale-switcher.test.mjs` (9 ошибок)
- `tests/unit/inbox.test.mjs` (6 ошибок)
- `tests/unit/embed-formatter.test.mjs` (6 ошибок)
- `tests/unit/voice-input-mode.test.mjs` (3 ошибки)
- `tests/unit/poll-progress.test.mjs` (3 ошибки)
- `tests/unit/markdown-helper.test.mjs` (3 ошибки)
- `tests/unit/invite-parser.test.mjs` (1 ошибка)
- `tests/unit/forum-filter.test.mjs` (1 ошибка)
- `tests/unit/audit-log.test.mjs` (1 ошибка)
- `tests/unit/attachment-validator.test.mjs` (1 ошибка)
- `/home/user/orca/project/waves/RW-40/G2/task-282-unit-tsc-fix-batch3/red.log`
- `/home/user/orca/project/waves/RW-40/G2/task-282-unit-tsc-fix-batch3/green.log`
- `/home/user/orca/project/waves/RW-40/G2/task-282-unit-tsc-fix-batch3/REPORT.md`

## Шаги выполнения (Strict TDD: Red -> Green)
1. **Red:** Запусти `npx tsc -p tsconfig.json --noEmit 2>&1 | grep -E "locale-switcher\.test|inbox\.test|embed-formatter\.test|voice-input-mode\.test|poll-progress\.test|markdown-helper\.test|invite-parser\.test|forum-filter\.test|audit-log\.test|attachment-validator\.test"` и сохрани вывод (34 ошибки) в `red.log`.
2. **Green:** Добавь аккуратные JSDoc-приведения типов (`/** @type {any} */` для негативных входов и моков или проверку на `undefined`), не меняя логику `assert.*` и не добавляя `@ts-ignore`.
3. **Проверка:**
   - Повтори команду `grep -E` из п.1 — должно быть **0 строк** (`green.log`)!
   - Запусти `node --test` по этим 10 файлам — все тесты `PASS`!
4. Запиши краткий `REPORT.md` с `md5sum` 10 файлов и вызови `orca orchestration worker-done`. Не делай `git commit`.
