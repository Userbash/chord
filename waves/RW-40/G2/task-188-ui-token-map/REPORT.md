# T-188 — UI token map (G2)

Ветка/worktree: `feature/task-188-ui-token-map`, база HEAD `d151389`.

## Сделано
- `tests/unit/token-map.test.mjs` — 15 тестов, 4 сьюта: заморозка карт, точные 7
  коллизий, полнота `CANONICAL_TOKEN_MAP` (25 записей), `resolveClientToken`
  (включая null-защиту), `auditStylesheetTokens` (mapped/unmapped/collisions, дедуп,
  пустой и нестроковый вход).
- `rust/gui-spike/ui/features/token-map.js` — `RESOLVED_COLLISIONS` (7),
  `CANONICAL_TOKEN_MAP` (25: коллизии + `--bg-0..4`, `--surface-1..4`, `--text-0..2`,
  `--text-strong`, `--accent`, `--online`, `--idle`, `--dnd`, `--offline`),
  `resolveClientToken`, `auditStylesheetTokens` (дедуп + сортировка, пустой вход — три
  пустых массива).

## Прогоны
- red: `node --test tests/unit/token-map.test.mjs` → падение на импорте отсутствующего
  модуля; лог `waves/RW-40/G2/task-188-ui-token-map/red.log`.
- green: `tests 15 / pass 15 / fail 0`; лог `waves/RW-40/G2/task-188-ui-token-map/green.log`.

## Файлы и md5
```
353647380c0f4748b52c6c2565bdfaf0  rust/gui-spike/ui/features/token-map.js
972fed0d2fd7462e7b4a1165162efd32  tests/unit/token-map.test.mjs
```

## Границы и риски
- Изменены только два файла из write-scope; `gate:worker` не запускался по режиму T0.
- Значения токенов Discord (`--brand-500`, `--status-dnd`, `--background-secondary-alt`)
  сверены с именами из контракта карточки; расширенная сверка с `reference-oracle/`
  не выполнялась в token-lean режиме — при интеграции стоит подтвердить по дампу.
- Коммитов нет.
