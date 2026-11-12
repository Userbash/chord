# ТЗ Технического ревью — TR-194A (`task-283` / `PR-165`, Экономия токенов)

## Контекст (НЕ читай лишние файлы вне списка!)
Ветка: `/home/user/orca/workspaces/project/qa-task-283-tsconfig-and-test-impact`.
Файлы:
- `rust/gui-spike/ui/tsconfig.json` (`d36fa8dd3043b17cd5b965de19e2ba28`)
- `scripts/test-impact.mjs` (`330caeeac8e6f553df0ecfd25eea84cc`)

## Шаги (коротко, без лишних чтений)
1. Запусти `node --test tests/unit/test-impact.test.mjs` (`53/53 PASS`) и `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` (`0 errors`).
2. Выполни **5 точечных мутаций** в `scripts/test-impact.mjs` (например, удаляя или портя новые маппинги из `RUST_MODULE_CONTRACTS` и `UI_MODULE_TESTS`), убедись что все `5/5` убиваются тестом `tests/unit/test-impact.test.mjs`, восстанови файл и сверь MD5 (`330caeeac8e6f553df0ecfd25eea84cc`).
3. Запиши краткий отчёт в `/home/user/orca/project/waves/RW-40/QUALITY/task-283-tsconfig-and-test-impact/REVIEW-194A.md` (вердикт `APPROVED`, `P0=0, P1=0`, `5/5` мутаций) и вызови `orca orchestration worker-done`. Не делай `git commit`.
