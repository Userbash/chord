# task-252 — Webhooks & Channel Follow Settings Controller (`webhooks-editor.js`, FE-P0-14)

- **Карточка / Ветка**: `task-252` (`feature/task-252-ui-webhooks-editor`)
- **Отдел**: G2 Frontend (WebView UI & DOM/A11y)
- **Оракулы**: `/home/sanya/re-deep/rest-static.md` (`CHANNEL_WEBHOOKS`, `GUILD_WEBHOOKS`, `WEBHOOK`, `WEBHOOK_INTEGRATION`), `rust/gui-spike/src/webhooks.rs` (типы вебхуков `1..=3`, границы имени `1..=80` символов, запрет `"clyde"` и подстроки `"discord"`).

## Сделано
1. `tests/unit/webhooks-editor.test.mjs` (`// @ts-check`, Strict TDD: Red → Green, 9 unit-тестов в 4 suites):
   - проверка экспорта и заморозки `WEBHOOK_TYPE_LABELS_RU = Object.freeze({ 1: "Входящий вебхук", 2: "Подписка на канал", 3: "Вебхук приложения" })`;
   - проверка `isValidWebhookName(name)`:
     - возвращает `true`, если обрезанная по краям строка имеет длину `1..=80` символов, в нижнем регистре не равна `"clyde"` и не содержит подстроку `"discord"`;
     - возвращает `false` для пустых/пробельных строк, строк длиннее 80 символов, `"clyde"` (без учёта регистра), строк с подстрокой `"discord"` (без учёта регистра) и нестроковых значений;
   - проверка `maskWebhookTokenUrl(url)`:
     - для URL с `"/api/webhooks/<id>/<token>"` и `"/api/v9/webhooks/<id>/<token>"` заменяет `<token>` на `"***"`, сохраняя префикс вплоть до `"<id>/"`;
     - возвращает `""` для пустых и нестроковых значений;
   - проверка `createWebhooksEditorController(options)`:
     - рендер `[data-webhooks-editor]` с `role="region"` и `aria-label="Вебхуки"`;
     - рендер карточек вебхуков `[data-webhook-item-id="<id>"]` с именем, русской меткой типа из `WEBHOOK_TYPE_LABELS_RU`, замаскированным URL `[data-webhook-masked-url="<id>"]`, кнопкой копирования `[data-webhook-copy-url="<id>"]` (`onCopyWebhookUrl(id, url)`) и кнопкой удаления `[data-webhook-delete-btn="<id>"]` (`onDeleteWebhook(id)`);
     - рендер формы создания с `[data-webhook-new-name-input]`, `[data-webhook-create-btn]` и `[data-webhook-error]` (вызывает `onCreateWebhook(trimmedName)` только при `isValidWebhookName(name) === true`, иначе выставляет текст ошибки в `[data-webhook-error]`);
     - проверка методов `{ setState(patch), getState(), destroy() }` и неизменяемости возвращаемого снимка состояния.
2. `rust/gui-spike/ui/features/webhooks-editor.js` (`// @ts-check` + JSDoc, ESM):
   - реализованы и экспортированы `WEBHOOK_TYPE_LABELS_RU`, `isValidWebhookName`, `maskWebhookTokenUrl` и `createWebhooksEditorController`.

## Прогоны (Red / Green / Targeted / Gate)
- **Red**: `node --test tests/unit/webhooks-editor.test.mjs > /home/sanya/orca/project/waves/RW-40/G2/task-252-ui-webhooks-editor/red.log 2>&1` → `exit=1` (`ERR_MODULE_NOT_FOUND`, `tests 1, pass 0, fail 1`).
- **Green**: `node --test tests/unit/webhooks-editor.test.mjs > /home/sanya/orca/project/waves/RW-40/G2/task-252-ui-webhooks-editor/green.log 2>&1` → `exit=0` (`tests 9, suites 4, pass 9, fail 0`).
- **Typecheck**:
  - `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` → `exit=0` (0 ошибок);
  - `npx tsc --ignoreConfig --noEmit --strict --allowJs --checkJs --noUncheckedIndexedAccess --skipLibCheck --target es2022 --module node20 --moduleResolution node16 --types node rust/gui-spike/ui/features/webhooks-editor.js tests/unit/webhooks-editor.test.mjs` → `exit=0` (0 ошибок).
- **Gate**: полный `gate:worker` не запускался (режим ТЗ воркера в batch); чужие файлы не затронуты.

## Файлы и md5
```
d36b437e62317a54fd4b7f9c7cea64d5  rust/gui-spike/ui/features/webhooks-editor.js
d90df3860665f11accc26f29551317e3  tests/unit/webhooks-editor.test.mjs
dfbf9d99889a0609458a02e121bd488d  /home/sanya/orca/project/waves/RW-40/G2/task-252-ui-webhooks-editor/red.log
31bd032723eb8fddca1f68c1582a540e  /home/sanya/orca/project/waves/RW-40/G2/task-252-ui-webhooks-editor/green.log
```

## Риски
- Контроллер не зависит от глобального `window.document`, если `options.container` предоставляет `ownerDocument.createElement` или `createElement`.
- `git commit` не выполнялся в соответствии с правилами воркера.
