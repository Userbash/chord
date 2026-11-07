# ТЗ Технического ревью — TR-183A (`task-270` / `PR-154`: `composer-popovers-bridge.js`)

## Контекст
Проверяется рабочая ветка `feature/task-270-ui-composer-popovers-bridge` (`/home/sanya/orca/workspaces/project/feature-task-270-ui-composer-popovers-bridge`).
Файлы:
- `rust/gui-spike/ui/features/composer-popovers-bridge.js` (MD5 `0d5ae58d106cc500335195b82e1e2e77`)
- `tests/unit/composer-popovers-bridge.test.mjs` (MD5 `7e3c3edefd86fd58c9b4b117d4d80be1`)

## Обязательные шаги проверки
1. Проверить `git status --short` и отсутствие посторонних изменений вне scope задачи.
2. Запустить контрактные тесты и `tsc`:
   - `node --test tests/unit/composer-popovers-bridge.test.mjs`
   - `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit`
3. Выполнить **5 точечных мутаций** в `rust/gui-spike/ui/features/composer-popovers-bridge.js` (с обязательным восстановлением исходного файла и проверкой MD5 `0d5ae58d106cc500335195b82e1e2e77` после всех мутаций). Все `5/5` мутаций должны быть убиты тестами (`tests/unit/composer-popovers-bridge.test.mjs`).
4. Записать полный отчёт в `/home/sanya/orca/project/waves/RW-40/G2/task-270-ui-composer-popovers-bridge/REVIEW-183A.md` (вердикт `APPROVED` при `P0=0, P1=0` и `5/5` убитых мутациях).
5. Завершить работу вызовом `orca orchestration worker-done`. Не делать `git commit`.
