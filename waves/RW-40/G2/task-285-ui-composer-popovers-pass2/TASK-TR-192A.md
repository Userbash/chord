# ТЗ Технического ревью — TR-192A (`task-285` / `PR-163`, Экономия токенов)

## Контекст (НЕ читай лишние файлы вне списка!)
Ветка: `/home/user/orca/workspaces/project/bugfix-task-285-ui-composer-popovers-pass2`.
Файлы:
- `rust/gui-spike/ui/features/composer-popovers-bridge.js` (`7f0ab60ad52f504e1f6ec8194e304a8a`)
- `tests/unit/composer-popovers-bridge.test.mjs` (`5c28324d2dd832960b186f6f667cb449`)

## Шаги (коротко, без лишних чтений)
1. Запусти `node --test tests/unit/composer-popovers-bridge.test.mjs` (`13/13 PASS`).
2. Выполни **5 точечных мутаций** в `rust/gui-spike/ui/features/composer-popovers-bridge.js` (включая мутации на новые проверки клонирования `options` и пустого имени в `selectSlashCommand`). Все `5/5` должны быть убиты тестом. Восстанови файл и сверь MD5 (`7f0ab60ad52f504e1f6ec8194e304a8a`).
3. Запиши краткий отчёт в `/home/user/orca/project/waves/RW-40/G2/task-285-ui-composer-popovers-pass2/REVIEW-192A.md` (вердикт `APPROVED`, `P0=0, P1=0`, `5/5` мутаций) и вызови `orca orchestration worker-done`. Не делай `git commit`.
