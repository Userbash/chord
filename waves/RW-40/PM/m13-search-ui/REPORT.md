# M-13-SEARCH-202-NSFW-UI — отчёт

Дата: 2026-10-30. Ветка: `rw40/m13-search-202-ui`. HEAD на момент работы:
`4e240481e59ff4d49732ac8bd5ba08a006363748` (`RW-40: integrate Discord GUI spike,
dispatch, paging and QA evidence`).

## Что сделано

Реализован UI-контракт `M-13-SEARCH-202-NSFW-UI` по принятой спецификации
`waves/RW-40/DESIGN/SEARCH-202-NSFW-SPEC.md` (`SEARCH-202-NSFW-SPEC-ACCEPT.md`)
и RE-досье `waves/RW-40/SEARCH-202-NSFW-RE.md`.

### 1. Чистый слой нормализации и контрактов (`rust/gui-spike/ui/features/search.js`)
- Экспортированы подтверждённые константы:
  - `MAX_SEARCH_OFFSET = 9975`, `MAX_SEARCH_PAGES = 400` (`4d2a64cb5df5fc4e_0@7556789`);
  - `SEARCH_MAX_INDEXING_ATTEMPTS = 5`, `SEARCH_DEFAULT_RETRY_AFTER_MS = 5000` (`4d2a64cb5df5fc4e_0@7548292`, `82a5af57ee11dcd5_0@212`);
  - `AGE_GATED_SPACES = 1`, `ADULT_VERIFICATION_STATUSES = [3, 4]`, `GUILD_NSFW_AGREE_STORAGE_KEY = "GuildNSFWAgreeStore"` (`4d2a64cb5df5fc4e_0@4174880`, `@1291038`).
- `computeIndexingRetryDelayMs(response)`:
  - полностью игнорирует тело ответа 202 (`body.retry_after`, `retry_after`, `documents_indexed`);
  - читает заголовок `Retry-After` / `retry-after` через `parseInt(raw, 10) * 1000` либо `retry_after_ms` Tauri IPC-конверта; при `0`, дробном `< 1` (`0.25` -> `0`) или отсутствии заголовка возвращает дефолт `5000` мс.
- `isAgeGatedSpacesRestricted`, `didAgreeNsfwGuild`, `applyIncludeNsfw`:
  - проверяют `AGE_GATED_SPACES = 1` и `age_verification_status ∈ {3, 4}` (`VERIFIED_ADULT=3`, `INFERRED_ADULT=4`) до проверки хранилища согласия;
  - при `!didAgree` ключ `include_nsfw` полностью опускается из `query` (не передаётся как `false`).
- `searchDateBounds(value)`:
  - сохраняет обратную совместимость со строковым аргументом `searchDateBounds("2026-10-20")`;
  - поддерживает объектную форму `{ date, on, before, after }` (`before: "2026-03-01"` -> `max_id: "1477455328051200000"`, `after: "2026-01-01"` -> `min_id: "1456436831842205696"`).
- `parseSearchQueryTokens(rawQuery)` и `normalizeHasFilter(value)`:
  - разбирают инлайн-токены (`from:`, `от:`, `in:`, `в:`, `has:`, `-has:`, `имеет:`, `before:`, `до:`, `after:`, `после:`, `on:`, `during:`, `день:`, `pinned:`, `закреплённые:`, `mentions:`, `с упоминанием:`, `authorType:`, `-authorType:`, `ТипАвтора:`);
  - поддерживают отрицание для `has` и `authorType` (`-file`, `-bot`) и русские алиасы (`ссылку` -> `link`, `audio` -> `sound`, `forward` -> `snapshot`).
- `formatHistoricalIndexedStatus(documentsIndexed)` (`4Y3O+O`) и `formatMaxPagesWarning(maxPages)` (`E+2azY`).

### 2. Проводка и состояния оверлея (`rust/gui-spike/ui/features/search-overlay.js`)
- **HTTP 202 / Indexing Retry & Historical Index**:
  - `.search-indexing-banner` (`role="status"`, `aria-live="polite"`, `aria-atomic="true"`, `data-indexing-state="idle|indexing|historical|exhausted"`) с `.search-indexing-progress[role="progressbar"]`;
  - до 5 автоматических повторов (`attempts: 1..5`), управляемых одиночным таймером `indexingPollId`, который немедленно очищается при смене запроса/фильтра/сортировки и при `close()`;
  - при исчерпании 5 повторов (6 ответов 202 подряд) — состояние `exhausted` (`aria-valuetext="5 из 5 попыток"`) и кнопка ручного повтора `.search-indexing-retry-btn`;
  - при `200 OK` с `doing_deep_historical_index: true` и `documents_indexed` — состояние `historical` с текстом `4Y3O+O` и `aria-valuetext="Архивная индексация: ${documents_indexed}"` одновременно с рендером карточек результатов.
- **NSFW Gate (`.search-nsfw-banner.search-nsfw-gate`)**:
  - режимы `prompt` (`Канал с возрастным ограничением`, кнопки `.search-nsfw-btn-continue` и `.search-nsfw-btn-back`, автофокус на `Продолжить`) и `underage` (`К сожалению, вы ещё слишком молоды…`, скрыта `Продолжить`, автофокус на `Вернуться`);
  - блокировка сетевого запроса в NSFW-канал до подтверждения согласия;
  - сохранение согласия в `GuildNSFWAgreeStore` (с fallback в память без `ExperimentalWarning` Node 24) и отправка `include_nsfw: true`;
  - бейдж `.search-result-nsfw-badge` (`18+`) на результатах из NSFW-канала;
  - клик по `Вернуться` очищает фильтр `in`, скрывает баннер и возвращает фокус в `.search-overlay-input`.
- **Чипы фильтров (`.search-filter-chips`)**:
  - синхронизация инлайн-токенов и контролов в `.search-chip` (`data-chip-key`, `data-chip-value`, `data-chip-negated`);
  - удаление отдельного чипа (`.search-chip-remove` кликом и `Backspace`/`Delete`) и полный сброс всех чипов (`.search-chips-clear`) с сохранением свободного текста запроса.
- **Потолок пагинации (`MAX_SEARCH_OFFSET = 9975`, `MAX_SEARCH_PAGES = 400`)**:
  - метод `loadAtOffset(offset)` и блокировка запросов с `offset > 9975`;
  - скрытие `.search-more` и вывод предупреждения `E+2azY` при достижении `offset >= 9975`.

### 3. Стили (`rust/gui-spike/ui/components/chat.css`)
- Добавлены стили для `.search-filter-chips`, `.search-chip`, `.search-indexing-banner`, `.search-nsfw-banner` / `.search-nsfw-gate`, `.search-result-nsfw-badge`, фокус-колец `:focus-visible` и `@media (prefers-reduced-motion: reduce)`.
- 100% token-only из `tokens.css`: `0` сырых `#hex`/`rgb`/`hsl`-цветов, `0` `!important`.

## Red

Лог: `waves/RW-40/PM/m13-search-ui/red.log` (запуск `node --test tests/unit/search-remote-real-index.test.mjs` до реализации продуктового кода, exit code `1`):

```
ℹ tests 18
ℹ suites 3
ℹ pass 8
ℹ fail 10
✖ T-M13-202-01: HTTP 202 игнорирует тело, берёт задержку из Retry-After / retry_after_ms и рендерит прогресс попытки 1 из 5
✖ T-M13-202-02: HTTP 202 при Retry-After '0', '0.25' или без заголовка (и при IPC retry_after_ms) использует контрактную задержку / fallback 5000 мс
✖ T-M13-202-03: лимит 5 автоматических повторов (6 запросов 202 подряд) переводит баннер в exhausted с кнопкой повтора
✖ T-M13-202-04: смена запроса и закрытие панели немедленно отменяют таймер поллинга 202
✖ T-M13-202-05: ответ 200 OK с doing_deep_historical_index=true и documents_indexed=8450 показывает результаты и баннер 4Y3O+O
✖ T-M13-NSFW-01..03: didAgreeNsfwGuild, пропуск include_nsfw при !didAgree и баннер .search-nsfw-gate (prompt, Continue, Go Back)
✖ T-M13-NSFW-04: при AGE_GATED_SPACES=1 и age_verification_status=2 (teen) didAgree=false даже при сохранённом согласии, режим underage
✖ T-M13-CHIPS-01..03: объектная форма searchDateBounds({ date, before, after }) и синхронизация чипов фильтров
✖ T-M13-CHIPS-02: отрицающие токены (-has:file -authorType:bot) и русские алиасы (от:777001 имеет:ссылку)
✖ T-M13-PAGE-01: потолок пагинации MAX_SEARCH_OFFSET = 9975 / MAX_SEARCH_PAGES = 400 блокирует переход далее 400 страницы с предупреждением E+2azY
```

Все 8 ранее существовавших тестов в `tests/unit/search-remote-real-index.test.mjs` сохранены без ослаблений.

## Green

Лог: `waves/RW-40/PM/m13-search-ui/green.log`:

```
node --test tests/unit/search-remote-real-index.test.mjs  -> pass 18 / fail 0
node --test tests/meta/test-policy.test.mjs              -> pass 17 / fail 0
npx tsc -p rust/gui-spike/ui/tsconfig.json               -> exit 0
npx tsc -p tsconfig.json                                 -> exit 0
```

Дополнительно проверен `node --test tests/unit/gui-spike-features.test.mjs` (`pass 100 / fail 0`).

## Мутации

Лог: `waves/RW-40/PM/m13-search-ui/mutations.log` — **5/5 KILLED**, baseline MD5 полностью восстановлен:
1. `M1` (`search.js` — `computeIndexingRetryDelayMs`: `parsedSec >= 0` вместо `parsedSec > 0`) -> **KILLED** (`T-M13-202-02`).
2. `M2` (`search.js` — `isAgeGatedSpacesRestricted`: пропуск проверки `ADULT_VERIFICATION_STATUSES [3, 4]`) -> **KILLED** (`T-M13-NSFW-04`).
3. `M3` (`search.js` — `searchDateBounds`: `before` использует конец дня `T23:59:59.999Z` вместо `T00:00:00.000Z`) -> **KILLED** (`T-M13-CHIPS-01..03`).
4. `M4` (`search-overlay.js` — `handleIndexingResponse`: лимит повторов `> 10` вместо `> SEARCH_MAX_INDEXING_ATTEMPTS (5)`) -> **KILLED** (`T-M13-202-03`).
5. `M5` (`search-overlay.js` — `fetchRemote`: снятие блокировки `requestOffset > MAX_SEARCH_OFFSET (9975)`) -> **KILLED** (`T-M13-PAGE-01`).

## Файлы и md5sum

```
d43971eba87ff56d59a70c9d7ed2a0e2  rust/gui-spike/ui/features/search.js
0bcea1b4a328b0794ee371f090f12530  rust/gui-spike/ui/features/search-overlay.js
fb419354a5f66bc4394b156f5fbb40f8  rust/gui-spike/ui/components/chat.css
bc7ee7fbd5f1b49417e71c2f94104f29  tests/unit/search-remote-real-index.test.mjs
d5fa2fbae82022a3289bd5ecde3576ed  waves/RW-40/PM/m13-search-ui/red.log
52565df50ec828b248a214d0758d0f04  waves/RW-40/PM/m13-search-ui/green.log
4530fd4b48c9d14201860cf1dc70b848  waves/RW-40/PM/m13-search-ui/mutations.log
```

## Риски и границы

- Изменены исключительно разрешённые файлы задачи `M-13-SEARCH-202-NSFW-UI`.
- Проброс HTTP 202 и заголовка `Retry-After` в Rust-команде `search_remote` (`rust/gui-spike/src/search.rs`) относится к отдельной backend-карточке (`SEARCH-202-NSFW-SPEC.md` §10); UI-слой поддерживает как прямой HTTP-подобный ответ (`status: 202`, `headers`), так и Tauri IPC-конверт (`indexing: true`, `retry_after_ms`).
