# task-247 — Scheduled Guild Events Modal & Event Creation Controller (`events-modal.js`, FE-P0-11)

- **Карточка / Ветка**: `task-247` (`feature/task-247-ui-events-modal`)
- **Отдел**: G2 Frontend (WebView UI & DOM/A11y)
- **Оракулы**: `reference-oracle/rest-static.json:8205-8284` (`GUILD_EVENTS_FOR_GUILD`), `rust/gui-spike/src/scheduled_events.rs` (статусы событий `1..=4`, типы сущностей `1..=3`).

## Сделано
1. `tests/unit/events-modal.test.mjs` (`// @ts-check`, Strict TDD: Red → Green, 7 unit-тестов в 3 suites):
   - проверка экспорта и заморозки `EVENT_FILTER_TABS_RU = Object.freeze([{ id: "upcoming", label: "Предстоящие" }, { id: "active", label: "Активные" }, { id: "all", label: "Все события" }])`;
   - проверка `filterEventsByTab(events, tabId)`:
     - `"active"`: только события со `status === 2`;
     - `"upcoming"`: только события со `status === 1` (также по умолчанию при неизвестном или опущенном `tabId`);
     - `"all"`: все события кроме отменённых (`status !== 4`);
     - сортировка по возрастанию `String(scheduledStartTime || "")` с tie-break по возрастанию `String(id)` без мутации входного массива;
   - проверка `createEventsModalController(options)`:
     - рендер `[data-events-modal]` с `role="dialog"`, `aria-modal="true"`, `aria-label="События сервера"`;
     - рендер кнопок вкладок `[data-event-tab="<id>"]` с `role="tab"` и `aria-selected="true" | "false"`;
     - рендер карточек событий `[data-event-card-id="<id>"]` с заголовком и кнопкой RSVP `[data-event-rsvp-btn="<id>"]` (`aria-pressed="true"` при `event.interested === true`, клик переключает состояние и вызывает `onToggleRsvp(id, !event.interested)`);
     - рендер формы создания события `[data-event-create-form]` при `canManageEvents === true` с полями `[data-event-name-input]`, `[data-event-entity-select]`, `[data-event-location-input]`, `[data-event-start-input]` и кнопкой `[data-event-create-submit]` (вызывает `onCreateEvent({ name, entityType, location, scheduledStartTime })`, когда обрезанные `name` и `scheduledStartTime` непустые);
     - рендер кнопки закрытия `[data-events-close-btn]`, вызывающей `onClose()`, и поддержка методов `{ setState(patch), getState(), close(), destroy() }`.
2. `rust/gui-spike/ui/features/events-modal.js` (`// @ts-check` + JSDoc, ESM):
   - реализованы и экспортированы `EVENT_FILTER_TABS_RU`, `filterEventsByTab` и `createEventsModalController`.

## Прогоны (Red / Green / Targeted / Gate)
- **Red**: `node --test tests/unit/events-modal.test.mjs` → `exit=1` (`ERR_MODULE_NOT_FOUND`, `tests 1, pass 0, fail 1`), лог сохранён в `/home/user/orca/project/waves/RW-40/G2/task-247-ui-events-modal/red.log`.
- **Green**: `node --test tests/unit/events-modal.test.mjs` → `exit=0` (`tests 7, suites 3, pass 7, fail 0`), лог сохранён в `/home/user/orca/project/waves/RW-40/G2/task-247-ui-events-modal/green.log`.
- **Typecheck**:
   - `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` → `exit=0` (0 ошибок);
   - `./node_modules/.bin/tsc --ignoreConfig --noEmit --strict --allowJs --checkJs --noUncheckedIndexedAccess --target es2022 --module node20 --moduleResolution node16 --types node rust/gui-spike/ui/features/events-modal.js tests/unit/events-modal.test.mjs` → `exit=0` (0 ошибок).
- **Gate**: полный `gate:worker` не запускался (режим ТЗ воркера в batch); чужие файлы не затронуты.

## Файлы и md5
```
3e14e44521110a282863f7f855a0358b  rust/gui-spike/ui/features/events-modal.js
3dd0638bb74b2c5507e9722a07d5f36c  tests/unit/events-modal.test.mjs
1315123c4a8744f152ff79f7c5d43b87  /home/user/orca/project/waves/RW-40/G2/task-247-ui-events-modal/red.log
d4a21a9bd6d39db421340808d669581e  /home/user/orca/project/waves/RW-40/G2/task-247-ui-events-modal/green.log
```

## Риски
- Контроллер не зависит от глобального `window.document`, если `options.container` предоставляет `ownerDocument.createElement` или `createElement`.
- `git commit` не выполнялся в соответствии с правилами воркера.
