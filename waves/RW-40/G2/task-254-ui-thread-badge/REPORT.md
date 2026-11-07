# TASK-254 — UI thread badge (G2), token-lean batch

Карточка: `task-254-ui-thread-badge`. Режим: token-lean, 2 файла, без чтения доков и RE-дампа.

## Что сделано

1. Red: сначала написан `tests/unit/thread-badge.test.mjs` (13 тестов) на контракт из ТЗ; прогон упал на `ERR_MODULE_NOT_FOUND` (модуля ещё нет) — `exit=1`.
2. Green: реализован `rust/gui-spike/ui/features/thread-badge.js` с `// @ts-check` и JSDoc: `THREAD_ARCHIVE_DURATIONS_RU` (frozen), `normalizeArchiveDurationMinutes`, `formatUnreadCountBadge`, `formatThreadStatusSummaryRu`.
3. Green-прогон: `13/13 pass`, `fail 0`, `exit=0`.

## Артефакты и md5

| Файл | md5 |
| --- | --- |
| `rust/gui-spike/ui/features/thread-badge.js` | `012aa64b65dfdf51dc58de5e2d4072a6` |
| `tests/unit/thread-badge.test.mjs` | `43a60a790bdc6ed9ab56e5ec5db2c848` |
| `waves/RW-40/G2/task-254-ui-thread-badge/red.log` | см. файл (Node 26.13.0, `ERR_MODULE_NOT_FOUND`, fail 1) |
| `waves/RW-40/G2/task-254-ui-thread-badge/green.log` | см. файл (tests 13, pass 13, fail 0) |

## Покрытый контракт

- Реестр сроков: ровно `60/1440/4320/10080` → `1 час / 24 часа / 3 дня / 1 неделя`, объект заморожен.
- `normalizeArchiveDurationMinutes`: допустимые значения (включая строку `"4320"`) проходят как `Number`; `0, 1, 90, 999, -60, null, undefined, "", "abc", NaN, {}` → `1440`.
- `formatUnreadCountBadge`: `0/null/NaN/"abc"` → `""`; `1..99` → число строкой; `>99` → `"99+"`; `5.9` → `"5"`; `-3` → `""`.
- `formatThreadStatusSummaryRu`: имя обрезается, пустое/нестроковое → `"Ветка без названия"`; метка архива из нормализованного срока; `locked` важнее `archived`, иначе `"Активна"`; счётчик сообщений неотрицательный и усечённый.

## Границы

Правились только два разрешённых файла. `gate:worker`, полный `cargo`, `git commit` не запускались — по условиям token-lean режима. `memberCount` принимается в типе, но в сводку не входит (ТЗ не требует).
