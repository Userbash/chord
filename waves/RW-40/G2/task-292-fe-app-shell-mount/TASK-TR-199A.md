# TASK-TR-199A — Техническое ревью PR-172 (`task-292`: `rust/gui-spike/ui/app.js` + `tests/unit/app-shell-wiring.test.mjs`)

- **Роль:** `Technical Reviewer (DeepSeek)` (`L1 / P1`)
- **Ветка для проверки:** `feature/task-292-fe-app-shell-mount` (рабочее дерево `/home/user/orca/workspaces/project/feature-task-292-fe-app-shell-mount`)
- **Проверяемые файлы:**
  - `rust/gui-spike/ui/app.js`
  - `tests/unit/app-shell-wiring.test.mjs`

## Задание
1. Проверь diff `git diff unstable..HEAD` — убедись, что изменены только `rust/gui-spike/ui/app.js` и `tests/unit/app-shell-wiring.test.mjs`.
2. Прогони целевые тесты и `typecheck`:
   ```bash
   node --test tests/unit/app-shell-wiring.test.mjs tests/unit/composer-popovers-bridge.test.mjs tests/unit/ui-boot.test.mjs
   npm run typecheck
   npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit
   ```
3. Выполни 5/5 точечных мутаций в `rust/gui-spike/ui/app.js` (откат после каждой мутации обязателен) и убедись, что все 5/5 убиваются тестами `tests/unit/app-shell-wiring.test.mjs`.
4. Запиши вердикт (`APPROVED` при `P0=0, P1=0` и `5/5` мутаций) в `/home/user/orca/project/waves/RW-40/G2/task-292-fe-app-shell-mount/REVIEW-199A.md`.
