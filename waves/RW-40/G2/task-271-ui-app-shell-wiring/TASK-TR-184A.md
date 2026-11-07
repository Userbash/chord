# ТЗ Технического ревью — TR-184A (`task-271` / `PR-155`: `app-shell-wiring.js`)

## Контекст
Проверяется рабочая ветка `feature/task-271-ui-app-shell-wiring` (`/home/sanya/orca/workspaces/project/feature-task-271-ui-app-shell-wiring`).
Файлы:
- `rust/gui-spike/ui/features/app-shell-wiring.js` (MD5 `928cbb029bee1ae75ce4e80336a7a356`)
- `tests/unit/app-shell-wiring.test.mjs` (MD5 `c179f8be3bde983e3f058c827249f39c`)

## Обязательные шаги проверки
1. Проверить `git status --short` и отсутствие посторонних изменений вне scope задачи.
2. Запустить контрактные тесты и `tsc`:
   - `node --test tests/unit/app-shell-wiring.test.mjs`
   - `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit`
3. Выполнить **5 точечных мутаций** в `rust/gui-spike/ui/features/app-shell-wiring.js` (с обязательным восстановлением исходного файла и проверкой MD5 `928cbb029bee1ae75ce4e80336a7a356` после всех мутаций). Все `5/5` мутаций должны быть убиты тестами (`tests/unit/app-shell-wiring.test.mjs`).
4. Записать полный отчёт в `/home/sanya/orca/project/waves/RW-40/G2/task-271-ui-app-shell-wiring/REVIEW-184A.md` (вердикт `APPROVED` при `P0=0, P1=0` и `5/5` убитых мутациях).
5. Завершить работу вызовом `orca orchestration worker-done`. Не делать `git commit`.
