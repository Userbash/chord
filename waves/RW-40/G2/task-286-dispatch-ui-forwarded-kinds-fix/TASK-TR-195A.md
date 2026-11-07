# TASK-TR-195A — Техническое ревью `PR-166` (`task-286` / `FE-FIX-08`: `dispatch-model.js` + `dispatch_ui.test.mjs`)

- **Отдел:** Technical Review (`DeepSeek` — `--agent opencode --model deepseek/deepseek-v4.1-flash`)
- **Уровень сложности / Приоритет:** `L2 / P1` (по `waves/RW-40/PM/MODEL-ROUTING-CRITERIA.md`)
- **Рабочее дерево:** `/home/sanya/orca/workspaces/project/bugfix-task-286-dispatch-ui-forwarded-kinds-fix` (`branch:bugfix/task-286-dispatch-ui-forwarded-kinds-fix`)
- **Проверяемые файлы:**
  1. `rust/gui-spike/ui/features/dispatch-model.js`
  2. `tests/unit/dispatch_ui.test.mjs`
- **Отчёт воркера и PM Accept:**
  - `/home/sanya/orca/project/waves/RW-40/G2/task-286-dispatch-ui-forwarded-kinds-fix/REPORT.md`
  - `/home/sanya/orca/project/waves/RW-40/G2/task-286-dispatch-ui-forwarded-kinds-fix/ACCEPT.md`

## Задание ревьюеру
1. Перейди в `/home/sanya/orca/workspaces/project/bugfix-task-286-dispatch-ui-forwarded-kinds-fix`.
2. Запусти:
   - `node --test tests/unit/dispatch_ui.test.mjs` (`58/58 PASS`)
   - `npx tsc -p tsconfig.json --noEmit` (`0 errors`)
3. Выполни **5 ручных мутаций** в `rust/gui-spike/ui/features/dispatch-model.js` (с обязательным откатом после каждой) и убедись, что `node --test tests/unit/dispatch_ui.test.mjs` убивает `5/5` мутаций (`100%`).
4. Проверь `git status --short` (чистый скоуп, без лишних правок).
5. Запиши `/home/sanya/orca/project/waves/RW-40/G2/task-286-dispatch-ui-forwarded-kinds-fix/REVIEW-195A.md` с вердиктом (`APPROVED` или `CHANGES REQUESTED`, `P0=0, P1=0`, таблица `5/5` мутаций).
6. Заверши задачу через `orca orchestration complete`. Не делай `git commit/push`.
