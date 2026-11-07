# REVIEW-168A — Technical Review PR-139 (task-254 / FE-P2-32)

Ревьюер: Технический ревьюер (DeepSeek), token-lean режим.
Дата: 2026-10-31. Ревьюируемый отчёт: `REPORT.md` (эта же папка). HEAD: `3581166` (`feat(ui): merge PR-134 (task-250 slowmode)`).

## Вердикт

**APPROVED.** P0 = 0, P1 = 0, мутации 5/5 убиты. Файл продукта после всех мутаций побайтово восстановлен.

## 1. Входная сверка

| Проверка | Ожидание в `REPORT.md` | Факт | Итог |
| --- | --- | --- | --- |
| md5 `rust/gui-spike/ui/features/thread-badge.js` | `012aa64b65dfdf51dc58de5e2d4072a6` | `012aa64b65dfdf51dc58de5e2d4072a6` | совпало |
| md5 `tests/unit/thread-badge.test.mjs` | `43a60a790bdc6ed9ab56e5ec5db2c848` | `43a60a790bdc6ed9ab56e5ec5db2c848` | совпало |
| `node --test tests/unit/thread-badge.test.mjs` | `tests 13 / pass 13 / fail 0`, `exit=0` | `tests 13 / pass 13 / fail 0 / skipped 0`, `exit=0` | совпало |

Бэкап перед мутациями: `/tmp/opencode/thread-badge.js.bak`, md5 `012aa64b65dfdf51dc58de5e2d4072a6`.

## 2. Ручные мутации (по одной, с восстановлением)

| # | Мутация | md5 мутанта | Тест | Убита | Поймавший тест |
| --- | --- | --- | --- | --- | --- |
| M1 | `DEFAULT_ARCHIVE_DURATION` `1440` → `60` | `840224cf3357e1b66dd9230d59a6d4a8` | exit=1, pass 10 / fail 3 | да | `normalizeArchiveDurationMinutes: unknown/invalid defaults to 1440`; `formatThreadStatusSummaryRu: empty thread falls back to defaults`; `formatThreadStatusSummaryRu: archive label follows normalized duration` |
| M2 | порог бейджа `n > 99` → `n >= 99` | `c3f9d6d4fe2294d34c60fc211c434de0` | exit=1, pass 12 / fail 1 | да | `formatUnreadCountBadge: 1..99 renders the number` (99 → `99+`) |
| M3 | приоритет состояния: `archived` проверяется раньше `locked` | `0d8e2afc0ecdc4f07b7e1371dc60800c` | exit=1, pass 12 / fail 1 | да | `formatThreadStatusSummaryRu: locked wins over archived, active is the default` |
| M4 | из `ALLOWED_ARCHIVE_DURATIONS` убран `10080` | `47abe2a8a64b48121a1558f0debbb60f` | exit=1, pass 10 / fail 3 | да | `normalizeArchiveDurationMinutes: allowed values pass through as Number`; `formatThreadStatusSummaryRu: archive label follows normalized duration`; `formatThreadStatusSummaryRu: full example` |
| M5 | `Math.trunc` → `Math.round` в `messageCountLabel` | `f6dd6e2ed439fb94de5e87bf296eeb8c` | exit=1, pass 12 / fail 1 | да | `formatThreadStatusSummaryRu: message count label is sanitized` (3.9 → 4) |

После каждой мутации файл восстанавливался из бэкапа, `md5sum` сверялся: все пять раз `012aa64b65dfdf51dc58de5e2d4072a6`. Финальный прогон после восстановления: `tests 13 / pass 13 / fail 0`, `exit=0`.

## 3. Финальное состояние

| Файл | md5 (финал) |
| --- | --- |
| `rust/gui-spike/ui/features/thread-badge.js` | `012aa64b65dfdf51dc58de5e2d4072a6` |
| `tests/unit/thread-badge.test.mjs` | `43a60a790bdc6ed9ab56e5ec5db2c848` |

Оба файла в рабочем дереве остались `??` (untracked), тестовый файл не редактировался.

## 4. Замечания

P0/P1 нет. Наблюдения уровня P2/P3, не блокируют merge:

- P3: `formatUnreadCountBadge(true)` даёт `"1"` — приведение `Number()` принимает boolean; контракт ТЗ такой вход не описывает, поведение не хуже текущих соседних хелперов.
- P3: `memberCount` в `ThreadStatusInput` объявлен, но в сводку не попадает — соответствует записи «ТЗ не требует» в `REPORT.md`.

## 5. Границы ревью

Изменялся только файл продукта (временно, для мутаций) и создан этот отчёт. Тестовый файл, `gate:worker`, полный `cargo` и `git commit` не запускались.
