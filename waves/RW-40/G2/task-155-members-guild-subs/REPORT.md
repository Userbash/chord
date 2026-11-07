# task-155 — Расчёт окон подписок гильдии OP 37 [0, 99] при прокрутке и группировка по ролям в `members.js`

- **batch/карточка:** RW-40 / G2 Frontend / `task-155` (FE-P1-03: расчёт окон подписок гильдии OP 37 `[0, 99]` при прокрутке и группировка по ролям в `rust/gui-spike/ui/features/members.js`).
- **worktree:** `/home/sanya/orca/workspaces/project/feature-task-155-members-guild-subs`.
- **файлы (write-scope):**
  - `rust/gui-spike/ui/features/members.js`;
  - `tests/unit/members-subscriptions.test.mjs` (новый файл unit-тестов);
  - синхронизация approved артефактов типизации: `tests/unit/dispatch_ui.test.mjs`, `rust/gui-spike/ui/features/dispatch-model.js`, `rust/gui-spike/ui/features/dispatch-view.js` из `/home/sanya/orca/project`;
  - отчёт и логи в `/home/sanya/orca/project/waves/RW-40/G2/task-155-members-guild-subs/`.

## Сделано

1. **Strict TDD:**
   - Создан новый тестовый сьют `tests/unit/members-subscriptions.test.mjs`, содержащий 14 контрактных тестов на все новые функции.
   - Зафиксирован Red-прогон в `red.log` с падением на этапе отсутствия экспортов.
2. **Функция `computeSubscriptionRanges(scrollTop, viewportHeight, rowHeight=44)`:**
   - Всегда включает базовый чанк `[0, 99]`.
   - Вычисляет видимые 100-элементные чанки `[start, start + 99]` (кратные 100) на основе вертикальной прокрутки.
   - Дедуплицирует совпадающие диапазоны.
   - Ограничивает количество подписок максимум 3 чанками на канал в каноническом порядке возрастания.
3. **Трекер подписок `createGuildSubscriptionsTracker({ onSubscribe, debounceMs, sidebar, guildId, channelId, rowHeight })`:**
   - Отслеживает переключение канала (`switchChannel` / `setChannel`) и прокрутку `#members-sidebar`.
   - Отправляет каноническую структуру исходящего опкода шлюза OP 37:
     `{ subscriptions: { [guildId]: { typing: true, threads: true, activities: true, channels: { [channelId]: ranges } } } }`.
   - Не спамит повторными вызовами `onSubscribe`, если вычисленные диапазоны не изменились.
   - Поддерживает конфигурируемый дебаунс скролла (`debounceMs`), ручной сброс (`flush`) и очистку ресурсов (`destroy` / `dispose`).
4. **Группировка и рендеринг `groupMembersByRoleOrStatus` и `renderMembersList`:**
   - Поддержка hoisted-групп из `GUILD_MEMBER_LIST_UPDATE` (`groups: [{ id, name, count, color }]`) с точным сохранением счётчика `count`, пришедшего из шлюза.
   - Поддержка цветов ролей (`roleColor` / `color`) в числовом и hex-форматах с нормализацией в `#rrggbb`.
   - Сохранение полной обратной совместимости со стандартными группами статусов (`online`, `idle`, `dnd`, `offline`) при отсутствии явных `groups`.
   - В `renderMembersList`: рендеринг заголовков групп (`.member-group-title`, `data-height="42"`, подпись вида `Название — N`) и строк участников (`.member`, окрашивание имени в `roleColor`, доступные атрибуты `aria-label`, статус-доты и аватары).

## red (до реализации)

Вывод сохранён в `red.log`:
```
SyntaxError: The requested module '../../rust/gui-spike/ui/features/members.js' does not provide an export named 'computeSubscriptionRanges'
✖ tests/unit/members-subscriptions.test.mjs (24.168879ms)
ℹ tests 1
ℹ pass 0
ℹ fail 1
```

## green (после реализации)

Вывод сохранён в `green.log`:
- `node --test tests/unit/members-subscriptions.test.mjs tests/unit/gui-spike-dom.test.mjs`:
  - `GUI spike: настоящий app.js в DOM-шиме`: 25 passed;
  - `чат-вью P3-02`: 4 passed;
  - `computeSubscriptionRanges`: 7 passed;
  - `createGuildSubscriptionsTracker`: 3 passed;
  - `groupMembersByRoleOrStatus & renderMembersList`: 4 passed;
  - **ИТОГО: 43 passed, 0 failed, 0 skipped**.
- `npx tsc -p tsconfig.json` → exit 0 (0 ошибок).
- `npx tsc -p rust/gui-spike/ui/tsconfig.json` → exit 0 (0 ошибок).
- `npm run typecheck` → exit 0.

## Файлы + md5

| Файл | MD5 |
|---|---|
| `rust/gui-spike/ui/features/members.js` | `6b4e06afe0314b64d8302354fd702291` |
| `tests/unit/members-subscriptions.test.mjs` | `2e04c232bb8d4ed643a027e342678215` |

Логи:
- `red.log`
- `green.log`

## Риски

1. **Связанность с `dispatch-view.js`:** В `dispatch-view.js` отрисовка списка участников сейчас адаптирована для плоского списка без заголовков групп (MB-01). `renderMembersList` и `groupMembersByRoleOrStatus` предоставляют готовую функциональность группировки и раскраски для монтажа в `dispatch-view` при переключении на полнофункциональный ролевой виджет.
2. **Дебаунс прокрутки:** Для высокочастотного скролла дефолт `debounceMs = 50` обеспечивает баланс между реактивностью обновления участников и защитой от спама шлюза кадрами OP 37.
