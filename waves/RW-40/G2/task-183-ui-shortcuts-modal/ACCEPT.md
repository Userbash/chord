# Приёмка PM: task-183 (FE-P2-05) — Горячие клавиши Discord и модалка «Горячие клавиши» (`Ctrl+/`)

- **PR:** PR-68 (`feature/task-183-ui-shortcuts-modal -> unstable`)
- **Статус:** `Ready for Technical Review` (передано в Review 97A)
- **Дата:** 2026-10-31

## Проверка DoD
1. `rust/gui-spike/ui/features/shortcuts.js`: `DISCORD_SHORTCUTS` (8 шорткатов), `matchShortcut` с защитой полей ввода, `createShortcutsController` (`#shortcuts-help-modal`, `role="dialog"`, `aria-modal="true"`, `Ctrl+/`, `Escape` с возвратом фокуса).
2. `tests/unit/shortcuts.test.mjs`: `24/24 PASS`, `tsc` без ошибок, `5/5` мутаций убито.

## Решение PM
**ПРИНЯТО (Accepted for Technical Review).**
