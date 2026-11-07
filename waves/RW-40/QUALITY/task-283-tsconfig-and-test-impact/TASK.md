# ТЗ — task-283 (`G4 QA L1 / P1`: `QA-FIX-04` — Включение `"features/**/*.js"` в `ui/tsconfig.json` и маппинг новых модулей в `scripts/test-impact.mjs`)

## Приоритет, Сложность и Экономия токенов
- **Приоритет:** `P1` | **Сложность:** `L1` (2 файла, точечная конфигурация).
- **Модель:** `deepseek/deepseek-v4.1-flash` (`--agent opencode`).
- **Экономия токенов:** НЕ сканируй весь проект! Работай СТРОГО только с `rust/gui-spike/ui/tsconfig.json` и `scripts/test-impact.mjs`.

## Разрешённые файлы для записи (Write-Scope)
- `rust/gui-spike/ui/tsconfig.json`
- `scripts/test-impact.mjs`
- `/home/sanya/orca/project/waves/RW-40/QUALITY/task-283-tsconfig-and-test-impact/REPORT.md`

## Шаги выполнения
1. В `rust/gui-spike/ui/tsconfig.json` замени массив `"include"` на:
   `["app.js", "features/**/*.js"]`
   и проверь `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` — должно быть **0 ошибок** (exit `0`).
2. Открой `scripts/test-impact.mjs`, найди таблицу соответствия UI/Rust модулей и добавь (если ещё не добавлены) маппинги для:
   - `rust/gui-spike/ui/features/shell-controllers-registry.js` -> `tests/unit/shell-controllers-registry.test.mjs`
   - `rust/gui-spike/ui/features/chat-helpers-bridge.js` -> `tests/unit/chat-helpers-bridge.test.mjs`
   - `rust/gui-spike/ui/features/composer-popovers-bridge.js` -> `tests/unit/composer-popovers-bridge.test.mjs`
   - `rust/gui-spike/ui/features/app-shell-wiring.js` -> `tests/unit/app-shell-wiring.test.mjs`
   - `rust/gui-spike/src/bff_domains_router.rs` -> `bff_domains_router_contract`
3. Проверь `npx tsc -p tsconfig.agent.json --noEmit` (или синтаксис `node --check scripts/test-impact.mjs`).
4. Запиши краткий `REPORT.md` с `md5sum` обоих файлов и вызови `orca orchestration worker-done`. Не делай `git commit`.
