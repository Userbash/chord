# ТЗ Технического ревью — TR-191A (`task-282` / `PR-162`, Экономия токенов)

## Контекст (НЕ читай лишние файлы вне списка!)
Ветка: `/home/user/orca/workspaces/project/bugfix-task-282-unit-tsc-fix-batch3`.
10 тестовых файлов `tests/unit/*.test.mjs` из `ACCEPT.md`.

## Шаги (коротко, без лишних чтений)
1. Запусти:
   - `npx tsc -p tsconfig.json --noEmit 2>&1 | grep -E "locale-switcher\.test|inbox\.test|embed-formatter\.test|voice-input-mode\.test|poll-progress\.test|markdown-helper\.test|invite-parser\.test|forum-filter\.test|audit-log\.test|attachment-validator\.test"` (ожидается 0 строк).
   - `node --test tests/unit/locale-switcher.test.mjs tests/unit/inbox.test.mjs tests/unit/embed-formatter.test.mjs tests/unit/voice-input-mode.test.mjs tests/unit/poll-progress.test.mjs tests/unit/markdown-helper.test.mjs tests/unit/invite-parser.test.mjs tests/unit/forum-filter.test.mjs tests/unit/audit-log.test.mjs tests/unit/attachment-validator.test.mjs` (`150/150 PASS`).
2. Выполни **5 точечных мутаций** в продуктовых модулях `rust/gui-spike/ui/features/locale-switcher.js` (2 мутации), `embed-formatter.js` (2 мутации) и `poll-progress.js` (1 мутация), убедись что все `5/5` убиваются тестами, и восстанови продуктовые файлы (`git checkout -- rust/gui-spike/ui/features/`).
3. Запиши краткий отчёт в `/home/user/orca/project/waves/RW-40/G2/task-282-unit-tsc-fix-batch3/REVIEW-191A.md` (вердикт `APPROVED`, `P0=0, P1=0`, `5/5` мутаций) и вызови `orca orchestration worker-done`. Не делай `git commit`.
