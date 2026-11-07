# REVIEW-119A — T-207 UI unread bar

Карточка: task-207-ui-unread-bar, G2 Frontend. Ревьюер: DeepSeek (token-lean).
Объект: `rust/gui-spike/ui/features/unread-bar.js` + `tests/unit/unread-bar.test.mjs`.

## Baseline
- `node --test tests/unit/unread-bar.test.mjs` → 11 tests, 3 suites, 11 pass, 0 fail.
- md5 модуля до ревью: `40dc98f1f3a829a6e150dae9eb9cb57f` — совпадает с REPORT.md.
- Контракт чистый: нет DOM/сетевых вызовов, `// @ts-check` + JSDoc, границы (`<= 0`, нечисловые) обработаны.

## Мутации (5/5 killed)
| # | Мутация | Результат |
|---|---------|-----------|
| M1 | порог 300 → 200 | KILLED (fail=2) |
| M2 | скрытие при 0 убрано (guard `< 0`) | KILLED (fail=2) |
| M3 | бейдж `99+` при `> 100` | KILLED (fail=1) |
| M4 | `visible` игнорирует новые сообщения | KILLED (fail=1) |
| M5 | `Math.trunc` → `Math.round` | KILLED (fail=2) |

После каждой мутации файл восстановлен; финальный md5 `40dc98f1f3a829a6e150dae9eb9cb57f`, diff с оригиналом пуст, повторный прогон 11/11 pass.

## Вердикт
**APPROVED.** P0=0, P1=0, мутации 5/5 (100%). `git commit` не делался.
Замечаний нет; интеграция в chat-панель (mount, a11y, CSS) — отдельная карточка.
