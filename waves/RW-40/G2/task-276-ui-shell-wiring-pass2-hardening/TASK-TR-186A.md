# ТЗ Технического ревью — TR-186A (`task-276` / `PR-157`, Экономия токенов)

## Контекст (НЕ читай лишние файлы вне списка!)
Ветка: `/home/sanya/orca/workspaces/project/bugfix-task-276-ui-shell-wiring-pass2-hardening`.
Файлы:
- `rust/gui-spike/ui/features/app-shell-wiring.js` (MD5 `a4c9cb5ea7ddca62eddbdca5ab1788fa`)
- `tests/unit/app-shell-wiring.test.mjs` (MD5 `5cca3ef80362b924786e92d9382d1baf`)
- `rust/gui-spike/ui/tsconfig.json` (MD5 `044b460c1852b37bee299b58ef12bc6e`)

## Шаги (коротко, без лишних чтений)
1. Запусти `node --test tests/unit/app-shell-wiring.test.mjs` (`15/15 PASS`).
2. Выполни **5 точечных мутаций** в `rust/gui-spike/ui/features/app-shell-wiring.js` (включая проверку новых guards: валидация до закрытия слоёв, `destroyed` guard, `Object.freeze` в `getState`). Все `5/5` должны быть убиты тестом `tests/unit/app-shell-wiring.test.mjs`. Восстанови файл и сверь MD5 (`a4c9cb5ea7ddca62eddbdca5ab1788fa`).
3. Запиши краткий отчёт в `/home/sanya/orca/project/waves/RW-40/G2/task-276-ui-shell-wiring-pass2-hardening/REVIEW-186A.md` (вердикт `APPROVED`, `P0=0, P1=0`, `5/5` мутаций) и вызови `orca orchestration worker-done`. Не делай `git commit`.
