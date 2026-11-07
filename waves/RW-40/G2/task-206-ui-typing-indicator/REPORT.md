# task-206 — входящий индикатор набора (TTL + русская строка)

Статус: green, к приёмке. Ветка feature/task-206-ui-typing-indicator (base e3ada49). Не коммитил.

## Сделано
- Red-first: `tests/unit/typing-indicator.test.mjs`, 11 тестов.
- В существующий `rust/gui-spike/ui/features/typing-indicator.js` добавлены (прочее не тронуто):
  - `pruneExpiredTypingUsers(entries, nowMs = Date.now())` — окно `0 <= age < TYPING_TTL_MS`,
    дедуп по `userId` (побеждает наибольший `startedAtMs`), сортировка по `startedAtMs`.
  - `formatTypingIndicatorRu(names)` — пустые/пробельные отброшены до подсчёта: `''` /
    `X печатает...` / `X и Y печатают...` / `X, Y и Z печатают...` / `Несколько человек печатают...`.
- `TYPING_TTL_MS = 10000` уже был, покрыт тестом.

## Прогоны
- red → `red.log`: 0 pass / 1 fail (нет экспортов), падение зафиксировано до кода.
- green → `green.log`: `tests 11 / pass 11 / fail 0`.
- `npx tsc -p tsconfig.json --noEmit`: в typing-indicator-файлах ошибок нет; остальные ошибки tsc — предсуществующие, в чужих тестах. `gate:worker` не запускал — запрещён режимом задачи.

## Файлы (md5)
- `rust/gui-spike/ui/features/typing-indicator.js` — `2c1eb022de4f5b647b77160176d31909`
- `tests/unit/typing-indicator.test.mjs` — `49a08fc331c12ea6c6b17201b530d62c`

## Риски
- Многоточие — ASCII `...` строго по ТЗ; старый `formatTypingText` даёт `…`. Унификация — одна строка + тест.
- `displayName` в prune не trim-ится (обрезка в форматтере); merge и `waves/db.jsonl` — за Team Lead.
