# ТЗ Технического ревью — TR-187A (`task-279` / `PR-158`, Экономия токенов)

## Контекст (НЕ читай лишние файлы вне списка!)
Ветка: `/home/sanya/orca/workspaces/project/bugfix-task-279-ui-features-tsc-clean`.
Файлы:
- `rust/gui-spike/ui/features/attachment-validator.js` (`dec89713e4893572f6dba02f8dd2a4c9`)
- `rust/gui-spike/ui/features/inbox.js` (`e1d84a89972594d8ed7c1b7b383ac567`)
- `rust/gui-spike/ui/features/invite-parser.js` (`1835d4d322d9be4772998494390a7e7b`)
- `rust/gui-spike/ui/features/svg-icons.js` (`4790e8f94cda67280aa4b1befc095d24`)
- `rust/gui-spike/ui/features/token-map.js` (`1781779a33804706b07d550199549fc8`)

## Шаги (коротко, без лишних чтений)
1. Запусти:
   - `npx tsc -p tsconfig.json --noEmit 2>&1 | grep -E "attachment-validator\.js|inbox\.js|invite-parser\.js|svg-icons\.js|token-map\.js"` (ожидается 0 строк).
   - `node --test tests/unit/attachment-validator.test.mjs tests/unit/inbox.test.mjs tests/unit/invite-parser.test.mjs tests/unit/svg-icons.test.mjs tests/unit/token-map.test.mjs` (`83/83 PASS`).
2. Выполни **5 точечных мутаций** (по 1 мутации в каждом из 5 файлов), убедись что все `5/5` убиты соответствующим тестом, восстанови все 5 файлов и сверь `md5sum`.
3. Запиши краткий отчёт в `/home/sanya/orca/project/waves/RW-40/G2/task-279-ui-features-tsc-clean/REVIEW-187A.md` (вердикт `APPROVED`, `P0=0, P1=0`, `5/5` мутаций) и вызови `orca orchestration worker-done`. Не делай `git commit`.
