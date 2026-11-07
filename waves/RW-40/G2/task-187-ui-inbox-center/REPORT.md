# TASK-187 — UI центра «Почта» (`inbox.js`)

## Сделано
- `tests/unit/inbox.test.mjs`: 19 тестов (4 suites) на контракт `INBOX_TABS`,
  `formatInboxBadge` (`""`/`"99+"`/`String(n)`) и `createInboxController`:
  каркас `#inbox-popover` (`role=dialog`, `aria-label="Почта"`, `hidden`/`aria-hidden`),
  вкладки `[data-inbox-tab]` с `aria-selected`, бейдж `[data-inbox-badge]`
  = `mentions + friendRequests`, `[data-inbox-item]`, пустое состояние
  «Всё прочитано» (`[data-inbox-empty]`), `inbox-mark-all-read`, `onSelectMessage`,
  `onAcceptFriend`, `markItemRead`, `markAllRead`, `destroy`.
- `rust/gui-spike/ui/features/inbox.js`: реализация с `// @ts-check`, состояние вне DOM,
  идемпотентный `destroy()`, невалидные id/вкладки возвращают `false`/`0`.
- DOM в тестах — локальный минимальный шим (стандартные `createElement`/
  `appendChild`/`replaceChildren`/`addEventListener`), без зависимостей.

## Прогоны
- red: `node --test tests/unit/inbox.test.mjs` → fail 1 (модуль отсутствовал), `red.log`.
- green: `node --test tests/unit/inbox.test.mjs` → tests 19, pass 19, fail 0, `green.log`.
- `gate:worker` не запускался (token-lean режим задания); чужие файлы не тронуты.

## Файлы и md5
```
6c706ccf38a79e01a50cca056dc31145  rust/gui-spike/ui/features/inbox.js
cc7037b14d9443d09b41cb4ecf9706b6  tests/unit/inbox.test.mjs
```

## Риски
- Интеграция попапа в общий shell/`screens.js` — вне scope карточки (общие файлы).
- `markAllRead` чистит и `unreads`; счётчик бейджа от этого не зависит.
- Ветка не коммитилась и не пушилась (запрет воркерам); PR оформляет координатор.
