# TASK-TR-201A — Техническое ревью PR-175 (`task-295`: `rust/gui-spike/ui/features/voice.js` + `tests/unit/voice-ui.test.mjs`)

- **Роль:** `Technical Reviewer` (`L1 / P1`)
- **Ветка для проверки:** `feature/task-295-fe-voice-diagnostics-wiring` (`/home/sanya/orca/workspaces/project/feature-task-295-fe-voice-diagnostics-wiring`)
- **Проверяемые файлы:**
  - `rust/gui-spike/ui/features/voice.js`
  - `tests/unit/voice-ui.test.mjs`

## Задание
1. Проверь diff `git status -s` / `git diff` — убедись, что изменены только `rust/gui-spike/ui/features/voice.js` и `tests/unit/voice-ui.test.mjs`.
2. Прогони целевые тесты и `typecheck`:
   ```bash
   node --test tests/unit/voice-ui.test.mjs tests/unit/voice-diagnostics.test.mjs
   npm run typecheck
   npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit
   ```
3. Выполни 5/5 точечных мутаций в добавленном коде `rust/gui-spike/ui/features/voice.js` (с обязательным побайтным откатом после каждой) и убедись, что все 5/5 убиваются `tests/unit/voice-ui.test.mjs`.
4. Запиши вердикт (`APPROVED` при `P0=0, P1=0` и `5/5` мутаций) в `/home/sanya/orca/project/waves/RW-40/G2/task-295-fe-voice-diagnostics-wiring/REVIEW-201A.md`.
