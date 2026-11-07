# task-257 — Guild Moderation & Ban/Audit Log Manager Controller (`moderation-panel.js`, FE-P0-16)

- **Карточка / Ветка**: `task-257` (`feature/task-257-ui-moderation-panel`)
- **Отдел**: G2 Frontend (WebView UI & DOM/A11y)
- **Оракулы**: `/home/sanya/re-deep/rest-static.json:871-877` (`Endpoints.GUILD_BAN`), `/home/sanya/re-deep/rest-static.json:1240-1246` (`Endpoints.GUILD_AUDIT_LOG`), `rust/gui-spike/src/moderation.rs` (`MAX_AUDIT_REASON_CHARS = 512`, `MAX_BAN_DELETE_MESSAGE_SECONDS = 604_800`, положительный числовой Snowflake `1..20` цифр).

## Сделано
1. `tests/unit/moderation-panel.test.mjs` (`// @ts-check`, Strict TDD: Red → Green, 8 unit-тестов в 3 suites):
   - `T-257-01`: проверка экспорта и глубокой заморозки `BAN_DELETE_OPTIONS_RU = Object.freeze([{ seconds: 0, label: "Не удалять сообщения" }, { seconds: 3600, label: "За последний час" }, { seconds: 86400, label: "За последние 24 часа" }, { seconds: 604800, label: "За последние 7 дней" }])`;
   - `T-257-02`, `T-257-03`: проверка чистой функции `filterAuditEntries(entries, options)`:
     - фильтрация по `options.actionType` (если `> 0`) и возврат всех записей при `actionType <= 0` или без фильтра;
     - регистронезависимый поиск подстроки `options.query` по `actorName`, `targetName` и `reason`;
     - отсутствие мутации входного массива и его объектов;
   - `T-257-04`..`T-257-08`: проверка `createModerationPanelController(options)`:
     - рендер региона `[data-moderation-panel]` с `role="region"` и `aria-label="Модерация сервера"`;
     - рендер формы бана `[data-mod-ban-form]` с полями `[data-mod-target-input]`, `[data-mod-reason-input]`, `[data-mod-delete-select]` и кнопкой `[data-mod-ban-submit]` (валидация положительного числового Snowflake `1..20` цифр и длины `reason <= 512` символов с вызовом `onBanMember({ targetUserId, deleteMessageSeconds, reason })`);
     - рендер списка активных банов `[data-mod-ban-id="<userId>"]` с кнопками разбана `[data-mod-unban-btn="<userId>"]`, вызывающими `onUnbanMember(userId)`;
     - рендер списка записей журнала аудита `[data-mod-audit-entry-id="<id>"]` с динамической фильтрацией по полю поиска `[data-mod-audit-search]`;
     - проверка методов `{ setState(patch), getState(), destroy() }` и неизменяемости снимка состояния.
2. `rust/gui-spike/ui/features/moderation-panel.js` (`// @ts-check` + JSDoc, ESM):
   - реализованы и экспортированы `BAN_DELETE_OPTIONS_RU`, `filterAuditEntries` и `createModerationPanelController`.

## Прогоны (Red / Green / Targeted / Gate)
- **Red**: `node --test tests/unit/moderation-panel.test.mjs > /home/sanya/orca/project/waves/RW-40/G2/task-257-ui-moderation-panel/red.log 2>&1` → `exit=1` (`ERR_MODULE_NOT_FOUND`, `tests 1, pass 0, fail 1`).
- **Green**: `node --test tests/unit/moderation-panel.test.mjs > /home/sanya/orca/project/waves/RW-40/G2/task-257-ui-moderation-panel/green.log 2>&1` → `exit=0` (`tests 8, suites 3, pass 8, fail 0`).
- **Typecheck**:
  - `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` → `exit=0` (0 ошибок);
  - `./node_modules/.bin/tsc --ignoreConfig --noEmit --strict --allowJs --checkJs --noUncheckedIndexedAccess --skipLibCheck --target es2022 --module node20 --moduleResolution node16 --types node rust/gui-spike/ui/features/moderation-panel.js tests/unit/moderation-panel.test.mjs` → `exit=0` (0 ошибок).
- **Gate**: полный `gate:worker` не запускался (режим ТЗ воркера в batch); чужие файлы не затронуты.

## Файлы и md5
```
25231dfa4b572e1cf8ae89a261d73d9b  rust/gui-spike/ui/features/moderation-panel.js
c97150c308b5dcecaa26ff1937888373  tests/unit/moderation-panel.test.mjs
f65a02ae08125665f45c00b3813979ec  /home/sanya/orca/project/waves/RW-40/G2/task-257-ui-moderation-panel/red.log
58f2486b097a9edc58814d524561f0c4  /home/sanya/orca/project/waves/RW-40/G2/task-257-ui-moderation-panel/green.log
```

## Риски
- Контроллер не зависит от глобального `window.document`, если `options.container` предоставляет `ownerDocument.createElement` или `createElement`.
- `git commit` не выполнялся в соответствии с правилами воркера.
