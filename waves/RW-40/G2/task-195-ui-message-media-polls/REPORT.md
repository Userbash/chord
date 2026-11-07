# task-195 — UI message media & polls

Batch: RW-40 / G2. Ветка: feature/task-195-ui-message-media-polls.

## Сделано
- `tests/unit/message-media-polls.test.mjs` — red-first unit-тесты (11 тестов, 3 сьюта) на `isSafeAttachmentUrl`, `renderMessageAttachments` и `createPollWidget`.
- `rust/gui-spike/ui/features/message-media-polls.js` — реализация `renderMessageAttachments` и `createPollWidget` с переиспользованием `./attachment-validator.js` (`classifyAttachmentKind`, `formatFileSizeRu`, `isSpoilerFilename`) и `./poll-progress.js` (`computePollResults`, `formatPollExpiryRu`, `togglePollAnswerSelection`); `// @ts-check` + JSDoc, ESM-экспорты.

## Поведение
- `renderMessageAttachments(containerEl, attachments, options)`:
  - рендерит карточки с `data-attachment-id`, `data-attachment-kind` (`'image'|'video'|'audio'|'file'`) через `classifyAttachmentKind` и русским размером через `formatFileSizeRu`;
  - валидирует URL через `isSafeAttachmentUrl` (разрешены только `https://`, `blob:`, `asset:`; отвергаются `javascript:`, `data:`, `vbscript:`, `http://` и пустые строки);
  - для `attachment.spoiler === true` или `isSpoilerFilename(attachment.filename)` выставляет `data-spoiler="hidden"`, `aria-expanded="false"` и бейдж `СПОЙЛЕР`, переключая по клику в `data-spoiler="revealed"` и `aria-expanded="true"`.
- `createPollWidget(containerEl, pollData, options)`:
  - рендерит вопрос опроса, варианты ответов с количеством голосов и процентами через `computePollResults`, подсветку лидирующих вариантов (`data-leading="true"`), футер истечения через `formatPollExpiryRu`;
  - поддерживает одиночный и множественный выбор через `togglePollAnswerSelection`;
  - кнопка `Голосовать` вызывает `onVote(selectedAnswerIds)` и блокируется (`disabled = true`), когда опрос завершён или выбор пуст.

## Прогоны
- Red: `node --test tests/unit/message-media-polls.test.mjs` → fail 1 (`ERR_MODULE_NOT_FOUND`) — `red.log`.
- Green: `node --test tests/unit/message-media-polls.test.mjs` → tests 11, pass 11, fail 0 — `green.log`.
- Typecheck: `npx tsc -p tsconfig.json` → 0 ошибок в `message-media-polls.js` и `message-media-polls.test.mjs`.

## Файлы и md5
```
58f73ca76625e33382267d2f819d46ea  rust/gui-spike/ui/features/message-media-polls.js
7e386894ee5cdeda014258748a0af4c1  tests/unit/message-media-polls.test.mjs
```

## Риски / осталось
- Подключение `renderMessageAttachments` и `createPollWidget` в основной цикл рендера ленты `chat.js` выполняется отдельной интеграционной карточкой.
- `git commit` не выполнялся.
