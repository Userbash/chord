# STUB-14-UI-SEARCH-FIX — независимое ревью pass 2

**Вердикт: RETURN (P2, только по evidence).** Поведенческие P1/P2 дефекты выглядят исправленными, но карточка не удовлетворяет обязательному mutation gate.

## Поведенческая проверка

- `author_id`/`mentions` теперь сравниваются с числовыми и строковыми snowflake id, а не только с отображаемым именем (`rust/gui-spike/ui/features/search-overlay.js:225-262`, `:750-756`).
- Календарные даты проверяются обратной сверкой UTC-компонентов (`rust/gui-spike/ui/features/search.js:90-119`), а overlay использует эту проверку (`rust/gui-spike/ui/features/search-overlay.js:106-113`). Тесты содержат независимый `snowflakeAt` и проверки `2026-02-31`/UTC (`tests/unit/search-remote-real-index.test.mjs:345-435`).

## Evidence / TEST-POLICY

- RED: `waves/RW-40/PM/stub14-search-ui/red-fix.log` — exit 1, 3 failures.
- GREEN: `waves/RW-40/PM/stub14-search-ui/green-fix.log` — 108 passed / 0 failed, skipped 0, todo 0, exit 0; `tsc` exit 0.
- **P2 — mutation proof отсутствует.** В `REPORT-FIX.md` перечислены red/green и ограничения, но нет mutation log, mutant cases или KILLED/MISSED результатов. Добавлены новые инварианты snowflake-id и календарной UTC-валидности, поэтому TEST-POLICY.md §3 и §9 требуют доказать, что тесты ловят возврат старого поведения.

Возвратить карточку на mutation run и приложить неизменяемый артефакт; после этого повторное функциональное ревью не требуется, если код и тесты не меняются.

Cargo в рамках этого ревью не запускался согласно заданию.
