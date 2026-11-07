# ACCEPT — task-287 (`G6 Docs L1 / P0`, `DOCS-FIX-01`: Синхронизация `TDD-PLAN.md` §8)

- **PR:** `PR-167` (`branch:docs/task-287-tdd-plan-sync`)
- **Статус PM:** `APPROVED` (`Ready for Merge`)
- **Проверка DoD:**
  1. В `TDD-PLAN.md` (§8) добавлены 74 недостающие строки для всех новых файлов `tests/unit/*.test.mjs` с реальными путями `rust/gui-spike/ui/features/*.js` (`rust/gui-spike/ui/i18n.js`) и буквальными токенами из соответствующих тестов.
  2. `node --test tests/meta/plan-sync.test.mjs` — `5/5 PASS` (exit `0`).
