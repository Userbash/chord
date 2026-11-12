# ТЗ — task-285 (`G2 Frontend L1 / P1`: `FE-FIX-07` — Pass 2 `composer-popovers-bridge.js` по замечаниям `REVIEW-183A`)

## Приоритет, Сложность и Экономия токенов
- **Приоритет:** `P1` | **Сложность:** `L1` (1 модуль UI + 1 unit-тест).
- **Модель:** `deepseek/deepseek-v4.1-flash` (`--agent opencode`).
- **Экономия токенов:** Работай СТРОГО только с `rust/gui-spike/ui/features/composer-popovers-bridge.js` и `tests/unit/composer-popovers-bridge.test.mjs`.

## Разрешённые файлы для записи (Write-Scope)
- `rust/gui-spike/ui/features/composer-popovers-bridge.js`
- `tests/unit/composer-popovers-bridge.test.mjs`
- `/home/user/orca/project/waves/RW-40/G2/task-285-ui-composer-popovers-pass2/red.log`
- `/home/user/orca/project/waves/RW-40/G2/task-285-ui-composer-popovers-pass2/green.log`
- `/home/user/orca/project/waves/RW-40/G2/task-285-ui-composer-popovers-pass2/REPORT.md`

## Шаги выполнения (Strict TDD: Red -> Green)
1. **Red:** Добавь 2 теста в `tests/unit/composer-popovers-bridge.test.mjs`:
   - Мутация массива `options` внутри элементов, возвращённых `getFilteredSlashCommands()`, не должна менять внутреннее состояние моста (глубокое клонирование `options: Array.isArray(cmd.options) ? cmd.options.map((o) => ({ ...o })) : []`).
   - Вызов `selectSlashCommand("")` или `selectSlashCommand("   ")` с пустым именем команды возвращает `null` и не вызывает `onInsertSlashTemplate`.
   Запусти `node --test tests/unit/composer-popovers-bridge.test.mjs` и запиши падение в `red.log`.
2. **Green:** Внеси минимальные правки в `rust/gui-spike/ui/features/composer-popovers-bridge.js`.
3. Проверь `node --test tests/unit/composer-popovers-bridge.test.mjs` (`13/13 PASS`) и `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` (`0 errors`), запиши `green.log` и `REPORT.md` с `md5sum` и вызови `orca orchestration worker-done`. Не делай `git commit`.
