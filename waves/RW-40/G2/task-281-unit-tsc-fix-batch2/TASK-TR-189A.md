# ТЗ Технического ревью — TR-189A (`task-281` / `PR-160`, Экономия токенов)

## Контекст (НЕ читай лишние файлы вне списка!)
Ветка: `/home/sanya/orca/workspaces/project/bugfix-task-281-unit-tsc-fix-batch2`.
Файлы:
- `tests/unit/svg-icons.test.mjs` (`fee2fa69b0ed19c9a572f18361c6d30f`)
- `tests/unit/channel-tree.test.mjs` (`7aa8a39f23f9dd2dcf73d5a707450404`)

## Шаги (коротко, без лишних чтений)
1. Запусти:
   - `npx tsc -p tsconfig.json --noEmit 2>&1 | grep -E "svg-icons\.test\.mjs|channel-tree\.test\.mjs"` (ожидается 0 строк).
   - `node --test tests/unit/svg-icons.test.mjs tests/unit/channel-tree.test.mjs` (`23/23 PASS`).
2. Выполни **5 точечных мутаций** в продуктовых модулях `rust/gui-spike/ui/features/svg-icons.js` (3 мутации) и `rust/gui-spike/ui/features/channel-tree.js` (2 мутации), убедись что все `5/5` убиваются тестами, и восстанови оба продуктовых файла (`git checkout -- rust/gui-spike/ui/features/svg-icons.js rust/gui-spike/ui/features/channel-tree.js`).
3. Запиши краткий отчёт в `/home/sanya/orca/project/waves/RW-40/G2/task-281-unit-tsc-fix-batch2/REVIEW-189A.md` (вердикт `APPROVED`, `P0=0, P1=0`, `5/5` мутаций) и вызови `orca orchestration worker-done`. Не делай `git commit`.
