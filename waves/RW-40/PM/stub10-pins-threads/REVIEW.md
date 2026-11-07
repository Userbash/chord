# Независимое ревью — STUB-10-PINS-THREADS-PANELS-UI

Дата: 2026-10-30. HEAD: `4e24048`. Продуктовый код и тесты read-only.

## Вердикт

**RETURN. P0=0, P1=1.**

### P1 — панели не подключены к приложению и не оформлены как поповеры

`createPinsThreadsPanels` существует только в новом модуле: поиск по `rust/gui-spike/ui`
не находит его вызов или импорт из `app.js`. Следовательно, реальные кнопки хедера не
создают панели и пользователь не может открыть их в приложении. Даже при ручном создании
`ensurePinsPanel`/`ensureThreadsPanel` добавляют секции в `.content`, а для классов
`header-dropdown-panel`, `pins-panel`, `threads-panel` и элементов панелей нет CSS-правил.
Это расходится с PINS-THREADS-PROFILE-SPEC §1–3: панели должны быть позиционированными
поповерами с шириной, ограничением высоты и слоем `--z-popover`. Текущие тесты создают
контроллер напрямую в DOM-шиме и не проверяют интеграцию или геометрию.

## Evidence

- MD5 совпадают с отчётом: `pins-threads.js` `f3e2708a3e8082dc60a999387850659a`,
  `channel-paging.test.mjs` `e6974ee4a62ea5eef57ac128239e5dce`.
- `node --test tests/unit/channel-paging.test.mjs` → exit 0, 19 passed, 0 failed.
- 5 описанных мутаций caught; локальные поведение/ARIA/состояния контроллера покрыты.
- `cargo` не запускался.
