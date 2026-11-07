# REVIEW — RW-40 / M-13-SEARCH-202-NSFW-BACKEND

- **HEAD:** `4e240481e59ff4d49732ac8bd5ba08a006363748` (ветка `rw40/m13-search-202-be`)
- **Ревью:** независимое, read-only по коду и golden-каталогам; мутации не повторялись (роль ревьюера, `AGENTS.md`), оценивался `mutations.log`.
- **Объект:** `rust/gui-spike/src/search.rs`, `rust/gui-spike/tests/search_contract.rs` + `waves/RW-40/PM/m13-search-be/{REPORT,red,green,mutations}.{md,log}`
- **Оракулы:** `waves/RW-40/DESIGN/SEARCH-202-NSFW-SPEC.md`, `waves/RW-40/SEARCH-202-NSFW-RE.md`, `rust/gui-spike/ui/features/search-overlay.js` (замороженный потребитель IPC)
- **Вердикт:** **RETURN** — P0=0, P1=1, P2=5. Код соответствует ТЗ карточки и оракулу RE; блокирует не реализация пунктов ТЗ, а несогласованный контракт: карточка перенесла поллинг `202` из UI в Rust, но спека §3.1–§3.3 и замороженный консьюмер `search-overlay.js` остались на старой схеме (IPC-конверт `indexing`/`retry_after_ms` + retry-таймер в UI). Нужно решение Team Lead: обновить спеку и UI-карточку либо вернуть IPC-конверт.

## 1. Чек-лист ТЗ

| # | Требование | Итог | Где |
|---|---|---|---|
| 1 | `MAX_SEARCH_OFFSET = 9975`, `MAX_SEARCH_PAGES = 400`, `MAX_SEARCH_202_RETRIES = 5`, `DEFAULT_SEARCH_202_RETRY_AFTER_MS = 5000` | PASS | `src/search.rs:23,26,29,32`; проверено `m13_constants_and_indexing_error_format_match_spec` |
| 2 | Тело `202` не читается | PASS | `src/search.rs:439-452`: на `202` тело не передаётся ни в serde, ни в лог; тесты кладут в тело `{"retry_after":99,"documents_indexed":1420}` и `not-json-body-ignored` — значения не используются |
| 3 | `Retry-After` в целых секундах `*1000`; fallback `5000` при `0`/отрицательном/дробном/нечисловом/отсутствующем | PASS | `parse_retry_after_ms` (`src/search.rs:472-480`); покрыты `"2"`, `" 7 "`, `"0"`, `"-1"`, `"0.25"`, `"abc"`, пустой список (`--lib search` + `retries_...` тесты) |
| 4 | До 5 повторов, при исчерпании `SearchError::Indexing { retry_after_ms, attempts: 5 }` | PASS | `src/search.rs:441-447`: 5 sleep + 6-й запрос, затем ошибка; тесты фиксируют ровно 5 `sleep` и 6 `get`, `attempts = 5`, `retry_after_ms` из заголовка последнего `202` |
| 5 | Без утечки токена | PASS | `Indexing` несёт только числа; `debug_and_errors_do_not_expose_token` прогоняет variant через `Display`/`Debug`/JSON |
| 6 | `documents_indexed: Option<u64>` в `SearchResponse` | PASS (см. P2-1) | `src/search.rs:174-176`; `Some(590)` из `200`, `None` при отсутствии поля |
| 7 | `offset <= 9975` до транспорта, `9975` разрешён | PASS | `src/search.rs:350-355`; тест проверяет `9975` → запрос ушёл, `9976`/`10000`/`i64::MAX` → `InvalidArg` и `target.is_none()` |
| 8 | `include_nsfw=true` только при `Some(true)` | PASS | `src/search.rs:428-430`; тест сверяет точный URL для `Some(true)`/`Some(false)`/`None`, плюс десериализация `include_nsfw`/`includeNsfw`/вложенного `query` |
| 9 | md5 файлов и логов | PASS | все 5 значений совпали с `REPORT.md` (см. §4) |
| 10 | `cargo test --offline -p discord-gui-spike --test search_contract`, `node --test tests/meta/test-policy.test.mjs` | PASS | 22/22 и 17/17, см. §4 |

## 2. P1 — блокирует приёмку

### P1-1. Карточка перенесла поллинг 202 в Rust, но спека и замороженный UI остались на схеме «поллинг в UI»

Спека задаёт другой контракт:

- §3.1 (`SPEC:436-458`): Rust на **один** `202` возвращает `Ok(SearchResponse { indexing: true, retry_after_ms: Some(delay_ms), .. })`, а структура `SearchResponse` расширяется полями `indexing: bool` и `retry_after_ms: Option<u64>`. Никакого цикла в Rust нет.
- §3.2 (`SPEC:460-509`): таймер, счётчик `indexingAttempts` 1..5, `cancelIndexingPoll()` и состояния баннера живут в `search-overlay.js`.
- §3.3 и тесты `T-M13-202-01..04` (`SPEC:1143-1146`) требуют от UI `data-indexing-state="indexing"`, бейдж «Попытка 1 из 5», «2 с» и `attempts: 1` в повторном запросе.

Реализация (`src/search.rs:436-458`) делает всё наоборот: цикл 5 повторов внутри `execute`, полей `indexing`/`retry_after_ms` в `SearchResponse` нет, при исчерпании возвращается новый `SearchError::Indexing`. То есть ТЗ карточки выполнено, а нормативная спека в этой части инвалидирована.

Прямое следствие для замороженного потребителя — не теория:

1. `rust/gui-spike/ui/features/search-overlay.js:73-85` держит закрытый список `SEARCH_ERROR_KINDS`, и `"indexing"` в нём **нет** (12 вариантов `SearchError` против 11 в списке).
2. `search-overlay.js:146-151` (`isSearchError`) по этому списку решает, ошибка это бэкенда или «IPC недоступен».
3. `search-overlay.js:1041-1051`: неизвестный `kind` уводит в fallback — `remoteState = "unavailable"`, `mode = "local"`, `localReason = "ipc"`.

Итог: сервер в состоянии индексации молча ждёт до 25 с (5 × 5000 мс), затем UI показывает локальную выдачу и сообщает, что IPC-команда недоступна. До ветки `202` давал `InvalidResponse` — известный `kind`, честное сообщение «некорректный ответ сервера поиска». Это регрессия наблюдаемого поведения, а не только расхождение текста.

Что нужно (решение Team Lead, код карточки тут ни при чём):

- **Вариант A (принять новую схему):** обновить `SEARCH-202-NSFW-SPEC.md` §3.1–§3.3 (`SearchResponse` без `indexing`/`retry_after_ms`, retry в Rust, ошибка `Indexing`), добавить `"indexing"` в `SEARCH_ERROR_KINDS` и текст ошибки в `remoteErrorText` UI-карточкой, переписать `T-M13-202-01..04` под `SearchError::Indexing`. Тогда бейдж «Попытка N из 5» и «повтор через N с» из UI не реализуемы — их надо снять со спеки явно.
- **Вариант B (вернуть спеку):** Rust возвращает IPC-конверт `indexing: true, retry_after_ms`, поллинг остаётся в UI. Тогда переделывается `execute` и часть тестов этой карточки.

Пока варианта нет в DECISIONS/спеке, ветку в `unstable` пускать нельзя: следующая UI-карточка реализует §3.2–§3.3 по спеке и упрётся в бэкенд, который так себя не ведёт.

## 3. P2 — исправить, не блокирует

### P2-1. Тип `documents_indexed`: спека `u64`, код `Option<u64>`

`SPEC:446-448` объявляет `#[serde(default, rename = "documents_indexed")] pub documents_indexed: u64` (отсутствие поля → `0`), `RE §6:516-517` даёт дефолт `0` (`A.result.documents_indexed ?? 0`). Код отдаёт `null`, когда поля нет (`search.rs:175-176`, тест `decodes_documents_indexed_...`). Это осознанное требование ТЗ карточки и семантически точнее (не отличать «0 проиндексировано» от «поля нет»), но в `REPORT.md` отклонение от спеки не названо: пункт 3 отчёта подаёт поле как «обратно совместимое». Потребитель, который сделает `response.documents_indexed.toLocaleString()` без `?? 0`, упадёт на `null`. Синхронизировать §3.1 и зафиксировать решение в DECISIONS; в M-13 UI читать строго `?? 0`.

### P2-2. `attempts` не пробрасывается в повторные GET

`RE §5.4:480-485` (`[D] 82a5af57ee11dcd5_0@212`) и `SPEC:485`: повторные запросы уходят с `&attempts=1..5`; первый — без него. Rust повторяет один и тот же `target` (`search.rs:431-437`), поэтому параметр не появляется ни разу. Тест `retries_http_202_against_local_mock_http_server_and_parses_200_ok:870` прямо закрепляет это: `assert_eq!(seen[0], seen[1])`. Реакция сервера на `attempts` в RE помечена `[A]` (не проверена), так что риск низкий, но расхождение с оракулом должно быть либо реализовано, либо записано в SPEC/DECISIONS и в отчёт. Сейчас о нём молчат оба документа.

### P2-3. У Rust-цикла нет отмены и прогресса

`RE §5.2` и `SPEC:506-509` предусматривают `cancel()`/`cancelIndexingPoll()`: смена запроса, фильтра, гильдии или закрытие панели снимает таймер и прекращает поллинг. В реализации `execute` спит `tokio::time::sleep` до 25 с и не проверяет отмену; trait `SearchTransport` (`search.rs:269-278`) не имеет ни cancel-token, ни канала прогресса. `requestSeq` в UI лишь скрывает устаревший ответ — запрос уже ушёл и ещё до 5 раз уйдёт после закрытия панели. Для пользователя это 25 с «загрузки» без баннера индексации и до 6 GET на каждый поиск в неиндексированной гильдии. Если вариант A принят — нужен хотя бы `tokio::select!` с отменой и событие прогресса для UI; если нет — вернуться к варианту B.

### P2-4. Слабая проверка в unit-тесте

`search.rs:561-566`: `assert!(format!("{err}").contains("5"))` и `contains("2000")`. Строка «5» встречается и в «5000», так что тест не отличит `attempts: 5` от `retry_after_ms: 5000`. Заменить на сверку полной строки (`"после 5 попыток; повтор через 2000 мс"`). Контрактные тесты `search_contract` этот пробел закрывают, поэтому P2.

### P2-5. Мёртвый `skip_serializing_if` на `include_nsfw`

`search.rs:62-63`: `#[serde(default, skip_serializing_if = "Option::is_none")]` влияет только на сериализацию самой структуры `SearchParams`, а query-строка собирается вручную (`search.rs:428-430`) и это правило не использует. Ничего не ломает, но читателю подсказывает неверную модель: «опускается при `None`» — а `Some(false)` при сериализации структуры тоже опускается. Либо убрать атрибут, либо оставить комментарий, что он для отладочного JSON, а не для URL.

### Информационно (не дефект)

- `parse::<i64>()` строже `parseInt`: заголовок `"2.0"` или `"2abc"` даст fallback `5000`, тогда как оракул дал бы `2000`. Спека §3.1 предписывает именно строгий разбор, так что это соблюдение спеки; но live-захват `202` (RE §9, gap `[A]`) стоит проверить на формат заголовка.
- Идентичные `target` на всех повторах означают, что сервер видит 6 одинаковых GET подряд; при живом `202` это стоит сверить с рейт-лимитами (RE §5.6).

## 4. Доказательства

### md5 против `REPORT.md` — совпали все

| Файл | md5 (факт) | REPORT |
|---|---|---|
| `rust/gui-spike/src/search.rs` | `13675fe6b6065d5698c3eeb3ba899557` | совпал |
| `rust/gui-spike/tests/search_contract.rs` | `666fe46eebb3c5740b43a80d02cd7e23` | совпал |
| `waves/RW-40/PM/m13-search-be/red.log` | `24c14fe79bfed58f616ed4d57efb8307` | совпал |
| `.../green.log` | `30f36eff5fd7b0dd344609ad72ce1f02` | совпал |
| `.../mutations.log` | `80192b7d1f543a05b7d0a26d59dce9e7` | совпал |

### Прогоны (мои, 2026-10-30)

- `cargo test --offline -p discord-gui-spike --test search_contract` → `test result: ok. 22 passed; 0 failed; 0 ignored` (1.00 s), совпадает с `green.log`; новых тестов 6, старые не ослаблены (`git diff --numstat`: `622 0` — ни одной удалённой строки в тест-файле, 16 тестов до карточки + 6 M-13).
- `cargo test --offline -p discord-gui-spike --lib search` → `2 passed; 8 filtered out`, как в `green.log`.
- `node --test tests/meta/test-policy.test.mjs` → `tests 17 / pass 17 / fail 0`, `exit=0`.
- `rustfmt --edition 2021 --check search.rs search_contract.rs` → `exit=0`.
- `red.log` внутренне согласован: Pass 1 — 13 ошибок компиляции (`E0609`×7, `E0560`×3, `E0407`, `E0432`, `E0599`), Pass 2 — `FAILED. 16 passed; 6 failed`; обе цифры совпадают с `REPORT.md`.
- `mutations.log`: 5/5 `KILLED`, у каждого мутанта падают именно тесты из отчёта, после каждого — `restored ... (match=True)` с baseline md5. MUT-1 (off-by-one) даёт `attempts: 4`, MUT-2 — sleeps `[5000,5000,5000]`, MUT-3 — `None` вместо `Some(590)`, MUT-4 — `InvalidResponse` вместо `InvalidArg`, MUT-5 — лишний `include_nsfw=true`. Тесты ловят все пять.

### Что ещё проверено в коде

- `202` обрабатывается до классификатора (`src/search.rs:439`), поэтому `RestFailure::from_status(202) → None` не мешает; `429` по-прежнему несёт `RateLimited`.
- Границы: `retries >= MAX_SEARCH_202_RETRIES` проверяется до `sleep` → ровно 5 ожиданий и 6 запросов, без «лишнего» сна после финального `202` (тест это фиксирует).
- `MAX_SEARCH_PAGES` участвует только в документации/тесте; потолок задан через `MAX_SEARCH_OFFSET`, 400 = 9975/25 + 1. Расхождения нет.
- Серверный путь `DiscordSearch::with_base` + локальный TCP-мок подтверждает, что заголовок доходит из реального транспорта (`Retry-After: 1` → sleep 1 с) и что `include_nsfw=true` и `offset=9975` попадают в живой URL.
- `SearchParams` десериализуется из `camelCase`, `snake_case` и вложенного `query` без ослабления прежних проверок (`limit`, `has`, `author_type`, snowflake-диапазон).

## 5. Итог

Реализация пунктов ТЗ карточки — качественная: константы, поведение `202`, `Option<u64>`, потолок `offset`, `include_nsfw` и защита от утечки токена сделаны и покрыты тестами, red/green/mutations и md5 сходятся. Возврат — из-за P1-1: карточка сменила владельца retry-цикла, спека и `search-overlay.js` остались на прежнем контракте, и на текущем HEAD исчерпанная индексация превращается в молчаливый локальный fallback с ложной причиной «IPC недоступен». После решения Team Lead по варианту A/B (и синхронизации §3.1–§3.3 + `SEARCH_ERROR_KINDS`) карточку можно принимать; P2-1…P2-5 закрываются отдельной правкой, кроме P2-3, который входит в то же решение.
