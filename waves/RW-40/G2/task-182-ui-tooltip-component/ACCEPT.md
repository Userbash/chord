# Приёмка PM: task-182 (FE-P2-01) — Доступный компонент `Tooltip` (`tooltip_c36707`) вместо `title`

- **PR:** PR-67 (`feature/task-182-ui-tooltip-component -> unstable`)
- **Статус:** `Ready for Technical Review` (передано в Review 96A)
- **Дата:** 2026-10-31

## Проверка DoD
1. `rust/gui-spike/ui/features/tooltip.js`: `TOOLTIP_SHOW_DELAY_MS = 400`, `ICON_TOOLTIPS` (13 подписей), `createTooltipController` (`role="tooltip"`, `aria-describedby`, миграция `title -> data-tooltip`, закрытие по `Escape`/`mouseleave`/`focusout`/`click`).
2. `rust/gui-spike/ui/components/primitives.css`: стили `.tooltip` / `.tooltip_c36707` (`pointer-events: none`).
3. `tests/unit/tooltip.test.mjs`: `20/20 PASS`, `tsc` без ошибок, `5/5` мутаций убито.

## Решение PM
**ПРИНЯТО (Accepted for Technical Review).**
