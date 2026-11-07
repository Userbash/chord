# ТЗ — task-286 (`G2 Frontend L1 / P0`: `FE-FIX-08` — Исправление `FORWARDED_KINDS` в `dispatch.js` / `dispatch_ui.test.mjs:264`)

## Приоритет, Сложность и Экономия токенов
- **Приоритет:** `P0` | **Сложность:** `L1` (1 падающий unit-тест `A2: FORWARDED_KINDS` в `tests/unit/dispatch_ui.test.mjs:264`).
- **Модель:** `deepseek/deepseek-v4.1-flash` (`--agent opencode`).
- **Экономия токенов:** НЕ сканируй весь проект! Прочитай только `tests/unit/dispatch_ui.test.mjs` (строки 240–300) и импортируемый им модуль `rust/gui-spike/ui/features/dispatch.js` (или `rust/gui-spike/ui/app.js` — то, что тестирует `A2`).

## Разрешённые файлы для записи (Write-Scope)
- `rust/gui-spike/ui/features/dispatch.js` (или целевой UI-файл с `FORWARDED_KINDS`)
- `tests/unit/dispatch_ui.test.mjs`
- `/home/sanya/orca/project/waves/RW-40/G2/task-286-dispatch-ui-forwarded-kinds-fix/red.log`
- `/home/sanya/orca/project/waves/RW-40/G2/task-286-dispatch-ui-forwarded-kinds-fix/green.log`
- `/home/sanya/orca/project/waves/RW-40/G2/task-286-dispatch-ui-forwarded-kinds-fix/REPORT.md`

## Шаги выполнения (Strict TDD: Red -> Green)
1. **Red:** Запусти `node --test tests/unit/dispatch_ui.test.mjs` и сохрани падение теста `A2: FORWARDED_KINDS` (`AssertionError: CHANNEL_CREATE форвардится бэкендом, но отбрасывается UI`) в `red.log`.
2. **Green:** Приведи список `FORWARDED_KINDS` в продуктовом UI-модуле (и/или контракт синхронизации с `DispatchKind::as_str` из Rust, который парсит тест `tests/unit/dispatch_ui.test.mjs:264`) в полное соответствие с `DispatchKind::as_str`, не ослабляя проверку теста! Также убедись, что `npx tsc -p tsconfig.json --noEmit` на `tests/unit/dispatch_ui.test.mjs` даёт `0 errors`.
3. **Проверка:**
   - `node --test tests/unit/dispatch_ui.test.mjs` — **100% PASS** (`green.log`)!
   - `npx tsc -p tsconfig.json --noEmit` — **0 errors**!
4. Запиши краткий `REPORT.md` с `md5sum` изменённых файлов и вызови `orca orchestration worker-done`. Не делай `git commit`.
