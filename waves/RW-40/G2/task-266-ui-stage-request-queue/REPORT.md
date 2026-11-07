# task-266 — Stage Channel Speaker Request Queue & Audience Hand-Raise Helper (G2/G5)

Batch: RW-40 / G2–G5. Ветка: `feature/task-266-ui-stage-request-queue` (без `git commit`).

## Сделано
- `tests/unit/stage-request-queue.test.mjs` — red-first контракт, 11 тестов: заморозка и точные подписи `STAGE_MEMBER_ROLES_RU`; приоритет `suppress: false` над заявкой; строка заявки с пробелами по краям; пустые/нестроковые/отсутствующие значения; битый вход; фильтрация очереди (докладчики и слушатели отсекаются); сортировка по времени поднятой руки; равенство времени — строковый `userId`; невалидная дата в конце; отсутствие мутации входа; пустой и не-массив вход.
- `rust/gui-spike/ui/features/stage-request-queue.js` (`// @ts-check` + JSDoc, ESM):
  - `STAGE_MEMBER_ROLES_RU` — `Object.freeze({ speaker: "Докладчик", requested: "Просит слово", listener: "Слушатель" })`.
  - `classifyStageVoiceState(voiceState)` — `suppress === false` → `speaker`; непустая (после `trim()`) строка `requestToSpeakTimestamp` → `requested`; всё остальное, включая `null`/не-объект, → `listener`.
  - `sortStageSpeakerRequests(members)` — оставляет только `requested`, сортирует по `Date.parse(requestToSpeakTimestamp)` по возрастанию, `NaN` трактует как `Infinity`, при равенстве сравнивает `String(userId)` побайтово; вход не мутирует, возвращает ссылки на исходные объекты.
  - Внутренний `asRecord` не даёт упасть на `null`/примитивах; не-массив вход даёт `[]`.

## Прогоны
- **Red:** `node --test tests/unit/stage-request-queue.test.mjs` → `tests 1, pass 0, fail 1`, `ERR_MODULE_NOT_FOUND` на ещё не созданном модуле — `red.log`. Лог снят на финальной версии теста (модуль временно убран, затем возвращён), exit `1`.
- **Green (targeted):** `node --test tests/unit/stage-request-queue.test.mjs` → `tests 11, pass 11, fail 0, skipped 0`, exit `0` — `green.log`.
- **Typecheck (контракт ТЗ):** `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` → exit `0` (0 ошибок).
- **Typecheck (targeted, обходит узкий `include: ["app.js"]`):** `npx tsc --ignoreConfig --noEmit --allowJs --checkJs --strict --noUncheckedIndexedAccess --target ES2023 --module NodeNext --moduleResolution NodeNext --types node rust/gui-spike/ui/features/stage-request-queue.js tests/unit/stage-request-queue.test.mjs` → exit `0`.
- Полный `gate:worker` (`T1`) в воркере не запускался: по регламенту он выполняется один раз на batch в integration-лейне.

## Файлы и md5
```
80e6c4caefa68d35499bd7dec39636fc  rust/gui-spike/ui/features/stage-request-queue.js
281c2367698b3995c9773e0b8aaa23b3  tests/unit/stage-request-queue.test.mjs
9e746855f86b13f02d5fe3a2479e882a  waves/RW-40/G2/task-266-ui-stage-request-queue/red.log
57159b87f5c9743b30915f9292940885  waves/RW-40/G2/task-266-ui-stage-request-queue/green.log
```

## Риски / осталось
- Модуль пока никем не импортируется: рендер очереди «Просит слово», кнопки «Поднять руку»/«Дать слово» и подписка на обновления `VOICE_STATE_UPDATE` подключаются отдельной интеграционной карточкой.
- Сортировка `userId` намеренно строковая (по контракту), а не числовая: для Snowflake-идентификаторов одинаковой длины это даёт тот же порядок, для разной длины — побайтовый (например `"10" < "100" < "9"`). Это покрыто тестом.
- `Date.parse` лоялен к не-ISO форматам (`"Oct 31 2026"` и `"2026-10-31"` дают валидное время), поэтому «невалидная дата» — это только заведомо неразбираемая строка (`"не дата"`, `"31.10.2026 10:00"` — обе дают `NaN`). Если контракт требует строгого ISO-разбора, это отдельное уточнение.
- `git commit` не выполнялся. `package-lock.json` и `node_modules` были изменены до начала работы и не трогались.
