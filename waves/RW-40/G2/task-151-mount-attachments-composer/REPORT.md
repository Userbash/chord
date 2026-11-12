# Отчёт воркера: task-151 (`FE-P0-05` Mount Attachments Queue in Composer)

- **Batch / Волна**: `RW-40` (Отдел G2 Frontend)
- **Карточка**: `task-151` / `FE-P0-05` (`/home/user/orca/project/waves/RW-40/PM/BACKLOG-RE-FRONTEND.md`, строки 158–192)
- **Спецификации и оракулы**:
  - `docs/specs/M-11-ATTACHMENTS.md` / `/home/user/orca/project/waves/RW-40/DESIGN/ATTACHMENTS-SPEC.md` (лимит 10 файлов, `<= 25 МБ` (`26_214_400` байт) на файл, `SPOILER_`, `description` / `alt text`, `#attachment-shelf` над `#composer-input`, вызов `uploadAll()` -> `upload_attachments`, отправка сообщения с пустым текстом при `>= 1` вложении, очистка после отправки и при `destroy()`)
  - `/home/user/orca/project/waves/RW-40/ATTACH-RE.md` (`n4 = 10`, `SPOILER_`, `AttachmentFlags.IS_SPOILER = 1 << 3 = 8`)
- **Ворктри**: `/home/user/orca/workspaces/project/feature-task-151-mount-attachments-composer` (`HEAD = 4e24048`)

---

## 1. Что сделано

1. **Red-first тесты (`tests/unit/attachments-ui.test.mjs`)**:
   - Написаны 2 тестовых suite (11 тестов суммарно):
     - `M-11 UI: контроллер очереди вложений createAttachmentsController` (4 теста): проверка констант лимитов (`MAX_ATTACHMENT_COUNT = 10`, `MAX_ATTACHMENT_SIZE_BYTES = 26_214_400`), `SPOILER_` хелперов, рендеринга `#attachment-shelf` над `#composer-input`, переключения спойлера, ввода описания (`alt text`), удаления карточки и вызова `uploadAll()` -> `upload_attachments`.
     - `FE-P0-05 / task-151: интеграция createAttachmentsController в composer.js` (7 тестов):
       - экспорт `createMessageComposer` и `mountComposerAttachments` из `composer.js`;
       - добавление файлов через `#composer-attach-btn` (`#composer-file-input`), drag-and-drop и `paste` на зону композера с проверкой лимитов 10 файлов и `<= 25 МБ` на файл;
       - рендеринг полки `#attachment-shelf` над `#composer-input` с карточками `.attachment-card`, переключателем спойлера (`SPOILER_`), полем ввода описания (`.attachment-alt-input`) и кнопкой удаления (`.attachment-remove-btn`);
       - вызов `uploadAll()` -> `upload_attachments` при отправке сообщения и передача массива готовых метаданных `attachments` вместе с текстом;
       - разрешение отправки сообщения с пустым текстом (`""`), если в очереди есть хотя бы 1 вложение, и блокировка пустой отправки без вложений;
       - очистка полки вложений и отписка обработчиков при `destroy()`;
       - отправка по `Enter` (без `Shift`) и сохранение очереди вложений и текста при ошибке `upload_attachments`.
2. **Контроллер очереди вложений (`rust/gui-spike/ui/features/attachments.js`)**:
   - Реализован `createAttachmentsController` с поддержкой лимитов `MAX_ATTACHMENT_COUNT = 10` и `MAX_ATTACHMENT_SIZE_BYTES = 25 * 1024 * 1024` (`26_214_400`), полки `#attachment-shelf` над полем ввода композера, карточек файлов с переключением `SPOILER_`, вводом `description` (`alt text`), удалением из очереди, пакетной выгрузкой `uploadAll()` через `upload_attachments`, `clear()` и `destroy()`.
3. **Интеграция в композер (`rust/gui-spike/ui/features/composer.js`)**:
   - В `composer.js` реэкспортированы публичные символы `attachments.js`, добавлены `mountComposerAttachments(options)` и `createMessageComposer(options)`, а также метод `destroy()` в `createComposer(options)` с полным сохранением обратной совместимости со всеми 29 существующими тестами `composer.test.mjs` и `gui-spike-dom.test.mjs`.

---

## 2. Red-прогон (до интеграции в `composer.js`)

- **Команда**: `node --test tests/unit/attachments-ui.test.mjs`
- **Код выхода**: `1` (`tests 11, pass 4, fail 7`)
- **Лог**: `/home/user/orca/project/waves/RW-40/G2/task-151-mount-attachments-composer/red.log`

Выдержка из `red.log`:
```text
✔ M-11 UI: контроллер очереди вложений createAttachmentsController (4.702685ms)
▶ FE-P0-05 / task-151: интеграция createAttachmentsController в composer.js
  ✖ composer.js экспортирует createMessageComposer и mountComposerAttachments
  ✖ кнопка #composer-attach-btn (file input), drag-and-drop и paste добавляют файлы в createAttachmentsController (лимит 10 файлов, <= 25 МБ)
  ✖ рендерит полку #attachment-shelf над #composer-input с карточками, переключателем SPOILER_, alt text и кнопкой удаления
  ✖ при отправке сообщения вызывает uploadAll() -> upload_attachments и передаёт метаданные attachments вместе с текстом
  ✖ позволяет отправить сообщение с пустым текстом, если в очереди есть хотя бы 1 вложение, и блокирует пустую отправку без вложений
  ✖ очищает полку вложений и отписывает обработчики при вызове destroy()
  ✖ отправка по клавише Enter (без Shift) и сохранение очереди при ошибке upload_attachments
ℹ tests 11 | pass 4 | fail 7
```

---

## 3. Green-прогон (после реализации)

- **Команда**: `node --test tests/unit/attachments-ui.test.mjs tests/unit/gui-spike-dom.test.mjs /home/user/orca/project/tests/unit/composer.test.mjs tests/meta/ipc-path.test.mjs tests/meta/test-policy.test.mjs && npx tsc -p rust/gui-spike/ui/tsconfig.json`
- **Код выхода**: `0` (`tests 98, pass 98, fail 0`)
- **Лог**: `/home/user/orca/project/waves/RW-40/G2/task-151-mount-attachments-composer/green.log`

Выдержка из `green.log`:
```text
✔ M-11 UI: контроллер очереди вложений createAttachmentsController (4.518065ms)
▶ FE-P0-05 / task-151: интеграция createAttachmentsController в composer.js
  ✔ composer.js экспортирует createMessageComposer и mountComposerAttachments (0.583191ms)
  ✔ кнопка #composer-attach-btn (file input), drag-and-drop и paste добавляют файлы в createAttachmentsController (лимит 10 файлов, <= 25 МБ) (1.280801ms)
  ✔ рендерит полку #attachment-shelf над #composer-input с карточками, переключателем SPOILER_, alt text и кнопкой удаления (0.871281ms)
  ✔ при отправке сообщения вызывает uploadAll() -> upload_attachments и передаёт метаданные attachments вместе с текстом (0.779821ms)
  ✔ позволяет отправить сообщение с пустым текстом, если в очереди есть хотя бы 1 вложение, и блокирует пустую отправку без вложений (0.41502ms)
  ✔ очищает полку вложений и отписывает обработчики при вызове destroy() (0.31251ms)
  ✔ отправка по клавише Enter (без Shift) и сохранение очереди при ошибке upload_attachments (10.752461ms)
✔ FE-P0-05 / task-151: интеграция createAttachmentsController в composer.js (15.469455ms)
```

---

## 4. Targeted & TypeScript проверки

- `npx tsc -p rust/gui-spike/ui/tsconfig.json` → **exit 0** (0 ошибок)
- `npx tsc -p tsconfig.json` → **0 ошибок** во всех 3 назначенных файлах (`composer.js`, `attachments.js`, `attachments-ui.test.mjs`; в базовом коммите `4e24048` присутствуют только до-базовые предупреждения в чужом `tests/unit/dispatch_ui.test.mjs`, зафиксированные в `REVIEW-59A.md`)
- `node --test tests/unit/gui-spike-dom.test.mjs` → **29/29 pass** (`exit 0`)
- `node --test /home/user/orca/project/tests/unit/composer.test.mjs` → **29/29 pass** (`exit 0`)
- `node --test tests/unit/gui-spike-features.test.mjs` → **100/100 pass** (`exit 0`)
- `node --test tests/meta/ipc-path.test.mjs tests/meta/test-policy.test.mjs` → **29/29 pass** (`exit 0`)

---

## 5. Проверка мутациями (TEST-POLICY §5)

Выполнены 5 целевых мутаций с сохранением лога в `/home/user/orca/project/waves/RW-40/G2/task-151-mount-attachments-composer/mutations.log`:
1. **M1 (`attachments.js`)**: `MAX_ATTACHMENT_COUNT = 10 -> 9` → **KILLED** (`exit=1`, `fail 3`).
2. **M2 (`attachments.js`)**: `SPOILER_PREFIX = "SPOILER_" -> "HIDDEN_"` → **KILLED** (`exit=1`, `fail 5`).
3. **M3 (`composer.js`)**: блокировка пустого текста даже при `queuedCount > 0` → **KILLED** (`exit=1`, `fail 1`).
4. **M4 (`composer.js`)**: пропуск `attachments.uploadAll(channelId)` при отправке → **KILLED** (`exit=1`, `fail 3`).
5. **M5 (`composer.js`)**: пропуск `attachments.clear()` после успешной отправки → **KILLED** (`exit=1`, `fail 2`).

После мутаций файлы восстановлены побайтно, MD5 сверены.

---

## 6. Изменённые файлы и MD5

| Файл | Статус | MD5 |
|---|---|---|
| `rust/gui-spike/ui/features/composer.js` | modified | `db914a85335b56913710ac513be37c2e` |
| `rust/gui-spike/ui/features/attachments.js` | added | `324ded5e39847f4e99532dc861d1269a` |
| `tests/unit/attachments-ui.test.mjs` | added | `9380e9230098f0aabf9fca1423b26b5d` |
| `/home/user/orca/project/waves/RW-40/G2/task-151-mount-attachments-composer/red.log` | log | `5817643a681e65d19413d6528cebe4ee` |
| `/home/user/orca/project/waves/RW-40/G2/task-151-mount-attachments-composer/green.log` | log | `d228f1fba90151b10f7927de29ee23a5` |
| `/home/user/orca/project/waves/RW-40/G2/task-151-mount-attachments-composer/mutations.log` | log | `727f9cb8a3fcde051b14efb1a2a76ae7` |
| `/home/user/orca/project/waves/RW-40/G2/task-151-mount-attachments-composer/REPORT.md` | report | — |

Файлы `app.js`, `index.html`, `chat.js` **не затрагивались**.

---

## 7. Риски и открытые вопросы

- Нет блокеров. Статический оракул 38 IPC-команд (`ipc_wiring_contract.rs`) сохранён (вызов `upload_attachments` выполняется через константу `UPLOAD_ATTACHMENTS_COMMAND`), существующий `createComposer` (20 МиБ chip-очередь) и новый `createMessageComposer` / `mountComposerAttachments` (25 МБ полка `#attachment-shelf` с `SPOILER_` и `alt text`) работают без конфликтов селекторов.
