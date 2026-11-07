# ТЗ Технического ревью — TR-185A (`task-277` / `PR-156`, Экономия токенов)

## Контекст (НЕ читай лишние файлы вне списка!)
Ветка: `/home/sanya/orca/workspaces/project/bugfix-task-277-ui-batch-typecheck-fix`.
Целевой продуктовый файл для мутаций: `rust/gui-spike/ui/features/thread-badge.js` (MD5 `8a8dbe830a80e378e3956ad614d29fc6`).

## Шаги (коротко, без лишних чтений)
1. Запусти:
   - `npx tsc -p tsconfig.json --noEmit 2>&1 | grep -E "thread-badge|chat-helpers-bridge|slowmode\.test|event-banner\.test|guild-boost\.test|poll-card\.test"` (ожидается 0 строк).
   - `node --test tests/unit/thread-badge.test.mjs tests/unit/chat-helpers-bridge.test.mjs tests/unit/slowmode.test.mjs tests/unit/event-banner.test.mjs tests/unit/guild-boost.test.mjs tests/unit/poll-card.test.mjs` (`68/68 PASS`).
2. Выполни **5 точечных мутаций** в `rust/gui-spike/ui/features/thread-badge.js`, прогоняя `node --test tests/unit/thread-badge.test.mjs`. Убедись, что `5/5` убиты, восстанови файл и проверь `md5sum` (`8a8dbe830a80e378e3956ad614d29fc6`).
3. Запиши краткий отчёт в `/home/sanya/orca/project/waves/RW-40/G2/task-277-ui-batch-typecheck-fix/REVIEW-185A.md` (вердикт `APPROVED`, `P0=0, P1=0`, `5/5` мутаций) и вызови `orca orchestration worker-done`. Не делай `git commit`.
