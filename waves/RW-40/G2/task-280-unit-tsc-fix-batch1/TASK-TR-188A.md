# ТЗ Технического ревью — TR-188A (`task-280` / `PR-159`, Экономия токенов)

## Контекст (НЕ читай лишние файлы вне списка!)
Ветка: `/home/user/orca/workspaces/project/bugfix-task-280-unit-tsc-fix-batch1`.
Файлы:
- `tests/unit/profile-modal.test.mjs` (`33330e8cd38d2aea4f7a7bf2fa64fd9e`)
- `tests/unit/appearance-settings.test.mjs` (`0daf0811e1883af8b07c279c84b50af9`)

## Шаги (коротко, без лишних чтений)
1. Запусти:
   - `npx tsc -p tsconfig.json --noEmit 2>&1 | grep -E "profile-modal\.test\.mjs|appearance-settings\.test\.mjs"` (ожидается 0 строк).
   - `node --test tests/unit/profile-modal.test.mjs tests/unit/appearance-settings.test.mjs` (`27/27 PASS`).
2. Выполни **5 точечных мутаций** в продуктовых модулях `rust/gui-spike/ui/features/profile-modal.js` (3 мутации) и `rust/gui-spike/ui/features/appearance-settings.js` (2 мутации), убедись что все `5/5` убиваются тестами `profile-modal.test.mjs` и `appearance-settings.test.mjs`, и восстанови оба продуктовых файла (`git checkout -- rust/gui-spike/ui/features/profile-modal.js rust/gui-spike/ui/features/appearance-settings.js`).
3. Запиши краткий отчёт в `/home/user/orca/project/waves/RW-40/G2/task-280-unit-tsc-fix-batch1/REVIEW-188A.md` (вердикт `APPROVED`, `P0=0, P1=0`, `5/5` мутаций) и вызови `orca orchestration worker-done`. Не делай `git commit`.
