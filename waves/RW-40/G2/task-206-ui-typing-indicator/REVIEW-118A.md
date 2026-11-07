# REVIEW-118A — task-206, входящий индикатор набора

Вердикт: **APPROVED** (P0=0, P1=0, мутации 5/5 убиты).
Ревью файла в рабочем дереве `feature/task-206-ui-typing-indicator`:
`rust/gui-spike/ui/features/typing-indicator.js`, md5 `2c1eb022de4f5b647b77160176d31909` — совпал с REPORT.md до и после мутаций.

## Целевой прогон
`node --test tests/unit/typing-indicator.test.mjs` — exit 0 (11 тестов).

## Мутации (mutation-testing на новый код, 5 запусков)
| # | Мутация | Итог |
|---|---------|------|
| M1 | окно TTL `<` → `<=` (запись на границе жива) | KILLED |
| M2 | дедуп по `userId` оставляет старое `startedAtMs` | KILLED |
| M3 | убран отброс пустого `userId` в prune | KILLED |
| M4 | форматтер считает пустые/пробельные имена | KILLED |
| M5 | порог формы для 3 имён сдвинут на 4 | KILLED |

После каждой мутации файл восстановлен из копии, `diff -q` чист, md5 исходный.

## Замечания (не блокирующие)
- P2: в рабочем дереве нет trim `displayName` в prune — по REPORT это осознанно (обрезка в форматтере); строка `getTypingText`-пути тогда покажет имя с пробелами.
- P3: `Number(raw.startedAtMs)` принимает numeric string; тест это не фиксирует.
- P3: соседний `formatTypingText` даёт `…`, новый `formatTypingIndicatorRu` — ASCII `...`; расхождение задокументировано в REPORT, унификация отдельной карточкой.
- Копия в `/home/sanya/orca/project` (md5 `8c50cf9b...`) — другая ревизия без task-206; ревью относится к ветке воркера.

Тесты не ослаблялись, `git commit` не делал.
