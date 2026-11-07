# RW-40 / M-13-SEARCH-202-NSFW-BACKEND — HTTP 202 Retry-After IPC-конверт, `execute_with_retry`, `documents_indexed: u64`, `attempts`, `MAX_SEARCH_OFFSET=9975`, `include_nsfw` в `search.rs` (Pass 2)

- **batch/карточка:** `RW-40 / M-13-SEARCH-202-NSFW-BACKEND` (Pass 2, закрытие P1-1 и P2-1..P2-5 из `waves/RW-40/PM/m13-search-be/REVIEW.md` по Варианту B — строгое соответствие `waves/RW-40/DESIGN/SEARCH-202-NSFW-SPEC.md` §3.1);
- **роль:** Воркер G1 GUI-Spike Rust Backend (ветка `rw40/m13-search-202-be`);
- **сделано:**
  1. **P1-1 и P2-3 (Контракт HTTP 202 в `SearchResponse`, `execute` и `execute_with_retry`):**
     - В `SearchResponse` (`rust/gui-spike/src/search.rs`) добавлены обратно совместимые поля строго по `SEARCH-202-NSFW-SPEC.md` §3.1 (строки 436–458):
       - `#[serde(default, rename = "documents_indexed")] pub documents_indexed: u64`,
       - `#[serde(default)] pub indexing: bool`,
       - `#[serde(default, rename = "retry_after_ms", skip_serializing_if = "Option::is_none")] pub retry_after_ms: Option<u64>`.
     - При получении `HTTP 202 Accepted` метод `DiscordSearch::execute` (и `RemoteSearch::search`) **не блокирует поток**, полностью игнорирует тело ответа `202`, разбирает заголовок `Retry-After` / `retry-after` через `parse_retry_after_ms` (в целых положительных секундах `* 1000` мс с fallback `DEFAULT_SEARCH_202_RETRY_AFTER_MS = 5000`) и немедленно возвращает Tauri IPC конверт:
       `Ok(SearchResponse { messages: Vec::new(), total_results: 0, doing_deep_historical_index: false, documents_indexed: 0, indexing: true, retry_after_ms: Some(retry_after_ms) })`.
     - Автоматический retry-цикл на стороне Rust (до `MAX_SEARCH_202_RETRIES = 5` повторов с делегированием ожидания в `SearchTransport::sleep`, пробросом `attempts = Some(1..=5)` и возвратом `Err(SearchError::Indexing { retry_after_ms, attempts: 5 })` при исчерпании) выделен в отдельный публичный метод `DiscordSearch::execute_with_retry`.
  2. **P2-1 (Тип `documents_indexed: u64`):**
     - Поле `SearchResponse::documents_indexed` имеет тип `u64` с `#[serde(default, rename = "documents_indexed")]` (`SEARCH-202-NSFW-SPEC.md:446-448`, `SEARCH-202-NSFW-RE.md` §6:516-517): при отсутствии поля в JSON `200 OK` десериализуется в `0`, при наличии `590` — в `590`.
  3. **P2-2 (Параметр `attempts` в повторных GET):**
     - В `SearchParams` и `RawSearchFields` добавлено поле `pub attempts: Option<u32>` (с поддержкой верхнеуровневого `attempts` и вложенного `query.attempts`).
     - При `params.attempts == Some(n)` с `n > MAX_SEARCH_202_RETRIES` (`5`) до обращения к транспорту возвращается `Err(SearchError::InvalidArg)`; при `1..=5` в query-строку URL добавляется `attempts={n}` (`SEARCH-202-NSFW-RE.md` §5.4, `SPEC:485`); при `None` или `Some(0)` параметр `attempts` в URL не добавляется.
  4. **P2-4 (Точная проверка `Display` для `SearchError::Indexing`):**
     - В unit-тесте `search::tests::m13_constants_and_indexing_error_format_match_spec` проверяется полная подстрока `format!("{err}").contains("после 5 попыток; повтор через 2000 мс")`.
  5. **P2-5 (Атрибут и комментарий на `SearchParams::include_nsfw`):**
     - Убран вводящий в заблуждение `#[serde(default, skip_serializing_if = "Option::is_none")]` на `SearchParams::include_nsfw` и добавлен явный doc-комментарий о ручной сборке query-строки в `execute()` (`include_nsfw=true` только при `Some(true)`).
  6. **Контрактные тесты (`rust/gui-spike/tests/search_contract.rs`):**
     - Обновлены и расширены до 23 интеграционных тестов (16 базовых + 7 тестов контракта M-13): проверка мгновенного конверта `indexing: true` + `retry_after_ms` в `execute()` без вызова `sleep` и без чтения тела `202`, проверка `execute_with_retry()` с пробросом `&attempts=1..5` в URL и `SearchError::Indexing` после 5 повторов (включая реальный локальный TCP HTTP/1.1 mock-сервер через `DiscordSearch::with_base`), проверка `documents_indexed: u64 = 0` по умолчанию и `590` при наличии, проверка потолка `MAX_SEARCH_OFFSET = 9975`, `include_nsfw` и валидации `attempts`.
- **red:** `(cd rust/gui-spike && cargo test --offline -p discord-gui-spike --test search_contract -- --nocapture)` → Pass 1 (против исходного `src/search.rs` из Pass 1): 22 ошибки компиляции (`E0308`, `E0560`, `E0599`, `E0609`, `E0624`), `exit=101`; Pass 2 (пассивные типы/поля до реализации логики Pass 2 в `execute()`/`execute_with_retry()`/`Deserialize`): `FAILED. 17 passed; 6 failed`, `exit=101`; оба прогона записаны в `waves/RW-40/PM/m13-search-be/red.log`;
- **green:** `(cd rust/gui-spike && cargo test --offline -p discord-gui-spike --test search_contract -- --nocapture && cargo test --offline -p discord-gui-spike --lib search -- --nocapture)` → `ok. 23 passed; 0 failed` (`search_contract`) + `ok. 2 passed; 0 failed` (`--lib search`), `exit=0`, лог `waves/RW-40/PM/m13-search-be/green.log`;
- **targeted:** `waves/RW-40/PM/m13-search-be/green.log` — 23 интеграционных теста `search_contract` + 2 unit-теста `--lib search`; `node --test tests/meta/test-policy.test.mjs` — `pass 17 / fail 0`;
- **oracle:** `waves/RW-40/SEARCH-202-NSFW-RE.md` (§4.1, §5.1–§5.4, §6, §7.2: `82a5af57ee11dcd5_0@212`, `b9e153f9dfaafc9f_0@29479,31367`, `4d2a64cb5df5fc4e_0@7543688,7550064`), `waves/RW-40/DESIGN/SEARCH-202-NSFW-SPEC.md` (§1.1, §3.1–§3.2, §4.1) и `waves/RW-40/PM/m13-search-be/REVIEW.md`;
- **генерация:** не применимо;
- **мутации:** 5/5 `KILLED`, эквивалентных — 0, `INCONCLUSIVE` — 0, survivors — 0 (`waves/RW-40/PM/m13-search-be/mutations.log`):
  - `MUT-1-off-by-one-202-retry-limit-and-indexing-envelope` (`if retries >= MAX_SEARCH_202_RETRIES` → `if retries + 1 >= MAX_SEARCH_202_RETRIES`, md5 `d4273ecdabeaa10e2034a236beba4d0b`) → `KILLED` тестами `returns_typed_indexing_error_when_all_5_retries_return_202`, `retries_http_202_with_retry_after_header_ignoring_202_body_and_returns_200_results` (`exit=101`);
  - `MUT-2-ignore-retry-after-header` (`.filter(|&secs| secs > 0)` → `.filter(|_| false)`, md5 `a6d9d21e95ec90b8a73df544cf3b79f8`) → `KILLED` тестами `returns_immediate_indexing_envelope_on_http_202_without_sleeping_or_reading_body`, `returns_typed_indexing_error_when_all_5_retries_return_202`, `retries_http_202_with_retry_after_header_ignoring_202_body_and_returns_200_results`, `retries_http_202_against_local_mock_http_server_and_parses_200_ok` (`exit=101`);
  - `MUT-3-drop-documents-indexed-deserialization` (`#[serde(default, rename = "documents_indexed")]` → `#[serde(skip)]`, md5 `c92af851faf772a42dbf6dec3ba09112`) → `KILLED` тестами `decodes_documents_indexed_from_200_ok_and_defaults_to_zero_when_omitted`, `returns_immediate_indexing_envelope_on_http_202_without_sleeping_or_reading_body`, `retries_http_202_with_retry_after_header_ignoring_202_body_and_returns_200_results`, `retries_http_202_against_local_mock_http_server_and_parses_200_ok` (`exit=101`);
  - `MUT-4-omit-attempts-query-parameter` (`params.attempts.filter(|&n| n > 0)` → `params.attempts.filter(|_| false)`, md5 `c16601a4f9c0533a979336cd97b73c44`) → `KILLED` тестами `includes_nsfw_and_attempts_query_params_and_validates_attempts_range`, `returns_typed_indexing_error_when_all_5_retries_return_202`, `retries_http_202_with_retry_after_header_ignoring_202_body_and_returns_200_results`, `retries_http_202_against_local_mock_http_server_and_parses_200_ok` (`exit=101`);
  - `MUT-5-send-include-nsfw-on-some-false-and-drop-offset-ceiling` (`if params.include_nsfw == Some(true)` → `if params.include_nsfw.is_some()`, md5 `c855037e2c94b89f2e47af087aebef33`) → `KILLED` тестом `includes_nsfw_and_attempts_query_params_and_validates_attempts_range` (`exit=101`);
- **baseline/restore:** baseline `rust/gui-spike/src/search.rs` = `3ec29ae3219be411465ce3b2cc749287`, `rust/gui-spike/tests/search_contract.rs` = `f113b75de5727b82fe1ca5dc0f57eb3b` (`exit=0`, 0 skip); после каждого мутанта выполнен restore и проверено совпадение md5 (`3ec29ae3219be411465ce3b2cc749287`);
- **стабильность:** тесты с `QueuedTransport`/`FakeTransport` полностью детерминированы в памяти (0 мс ожидания), тест `retries_http_202_against_local_mock_http_server_and_parses_200_ok` использует локальный `TcpListener` на `127.0.0.1:0` с `Retry-After: 1` и проверяет передачу `&attempts=1` по проводу;
- **gate:worker:** в режиме batch полный `gate:worker` запускается на integration-лейне; `rustfmt --edition 2021 --check` по обоим файлам чист (`exit=0`), `node --test tests/meta/test-policy.test.mjs` — `17/17 pass`;
- **файлы:**
  - `rust/gui-spike/src/search.rs` — `3ec29ae3219be411465ce3b2cc749287`
  - `rust/gui-spike/tests/search_contract.rs` — `f113b75de5727b82fe1ca5dc0f57eb3b`
  - `waves/RW-40/PM/m13-search-be/red.log` — `e9e16189cb373e7460ffa3950aa18152`
  - `waves/RW-40/PM/m13-search-be/green.log` — `334a7fd4ace05320700309cac8035ca8`
  - `waves/RW-40/PM/m13-search-be/mutations.log` — `98cca7b2ff60094cf2cf172dfdc796d3`
- **риски/вопросы:**
  - Все замечания ревью (`P1-1`, `P2-1`..`P2-5`) закрыты: `search_remote` через `RemoteSearch::search` / `DiscordSearch::execute` возвращает немедленный IPC-конверт `indexing: true, retry_after_ms: Some(delay_ms), documents_indexed: 0` при `HTTP 202`, согласованный с `SEARCH-202-NSFW-SPEC.md` §3.1–§3.2 и `ui/features/search-overlay.js`.
- **следующий шаг:** передать `M-13-SEARCH-202-NSFW-BACKEND` (Pass 2) на повторное ревью и приёмку координатору / Team Lead.
