# Отчёт воркера — task-163 (BE-P1-05: Поиск — HTTP 202 Indexing, search_session_id / search_query_id, include_nsfw)

## 1. Batch / карточка

- **Волна:** `RW-40`, отдел `G1` (Rust Core & Tauri BFF).
- **Карточка:** `task-163` (BE-P1-05 из `/home/user/orca/project/waves/RW-40/PM/BACKLOG-RE-BACKEND.md` и спецификация `/home/user/orca/project/waves/RW-40/DESIGN/SEARCH-202-NSFW-SPEC.md`):
  1. Генерация и передача обязательных параметров `search_session_id` и `search_query_id` (UUID v4 на сессию и поисковый запрос) в query-строке поиска по подтверждённому оракулу `capture-run2.jsonl`.
  2. Переиспользование `search_session_id` и `search_query_id` при пагинации с тем же запросом (`with_offset`, `next_page`) и обновление `search_query_id` при формировании нового запроса в рамках сессии (`new_query_in_session`).
  3. Поддержка параметра `include_nsfw: Option<bool>` в query-строке (`include_nsfw=true` при `Some(true)`, пропуск при `None` и `Some(false)`).
  4. Обработка `HTTP 202 Accepted` (индексация идёт): парсинг `retry_after` из JSON-тела ответа (`{ "retry_after": 2.0, "message": ... }`) или из заголовка `Retry-After` (с приоритетом заголовка при наличии), повтор запроса до `MAX_INDEXING_RETRIES` (5 раз) с возвратом типизированной ошибки `SearchError::Indexing { retry_after_ms, attempts: 5 }` после исчерпания попыток.
- **Файлы воркера (write-scope):**
  - `rust/gui-spike/src/search.rs`
  - `rust/gui-spike/tests/search_contract.rs`
  - Отчёт: `/home/user/orca/project/waves/RW-40/G1/task-163-search-202-session-uuid/REPORT.md`

## 2. Что сделано

1. **`rust/gui-spike/src/search.rs`:**
   - Добавлены константы:
     - `pub const MAX_INDEXING_RETRIES: u32 = MAX_SEARCH_202_RETRIES;` (5 повторов).
   - Реализованы функции генерации UUID v4:
     - `pub fn generate_uuid_v4() -> String` (генерация случайного RFC 4122 UUID v4 через `discord_core::rest::identity::uuid_v4`).
     - `pub fn deterministic_uuid_v4(seed: u64) -> String` (детерминированный генератор для воспроизводимых контрактных тестов).
   - Реализована структура сессии `SearchSession`:
     - `pub struct SearchSession { pub session_id: String, pub query_id: String }`
     - Методы `new()`, `with_ids()`, `apply_to(&self, &mut SearchParams)`, `refresh_query_id(&mut self) -> String`.
   - Расширен `SearchParams`:
     - Поля `pub search_session_id: Option<String>` и `pub search_query_id: Option<String>`.
     - Десериализация из JSON: поддержка camelCase (`searchSessionId`, `searchQueryId`), snake_case (`search_session_id`, `search_query_id`), а также вложенных объектов `query: { ... }`.
     - Методы:
       - `ensure_session_ids(&mut self)` — генерация UUID v4 при их отсутствии.
       - `with_generated_ids(self) -> Self` — билдер со сгенерированными ID.
       - `with_session_ids(self, session_id, query_id) -> Self`.
       - `with_offset(&self, offset: i64) -> Self` — пагинация с сохранением обоих идентификаторов.
       - `next_page(&self, step: i64) -> Self` — переход на следующую страницу с сохранением обоих ID.
       - `new_query_in_session(&self, new_content) -> Self` — сохранение `search_session_id`, сброс `offset` на 0 и генерация нового `search_query_id`.
   - Сериализация параметров и валидация в `execute()`:
     - Валидация `search_session_id` и `search_query_id`: отклонение пустых строк и спецсимволов (`{`, `}`, `\r`, `\n`) до отправки в сеть (`SearchError::InvalidArg`).
     - Сериализация в query-строку в порядке, подтверждённом live-дампом `capture-run2.jsonl`: `search_session_id=...&search_query_id=...` перед параметрами пагинации `limit`/`offset`.
     - Если `search_session_id` / `search_query_id` не заданы (`None`), они не добавляются в query-строку (100% обратная совместимость со всеми существующими тестами).
   - Обработка `HTTP 202 Accepted` и парсинг `retry_after`:
     - Функция `parse_retry_after_ms(&[(String, String)], &[u8]) -> u64` теперь проверяет заголовок `Retry-After`, а при его отсутствии разбирает JSON-тело ответа на наличие поля `retry_after` (число float/int или строка) и переводит секунды в миллисекунды.
     - Сохранена полная совместимость с fallback-логикой `DEFAULT_SEARCH_202_RETRY_AFTER_MS` (5000 мс) и приоритетом заголовка над телом.

2. **`rust/gui-spike/tests/search_contract.rs`:**
   - Набор тестов расширен с 23 до 30 тестов:
     1. `serializes_search_session_id_and_search_query_id_in_query_string` — проверка сериализации обоих параметров в URL и десериализации camelCase/snake_case/nested.
     2. `reuses_search_session_id_and_query_id_on_pagination_and_refreshes_query_id_on_new_query` — проверка неизменности session_id/query_id при пагинации (`with_offset`, `next_page`) и обновления query_id при новом запросе.
     3. `parses_retry_after_from_json_body_on_http_202_when_header_is_absent` — проверка извлечения `retry_after` из тела JSON (дробные, float, int) при отсутствии заголовка.
     4. `retries_http_202_using_retry_after_from_json_body_and_preserves_session_ids` — сквозной retry-цикл при 202 с задержкой из тела и проверка сохранения session/query ID во всех запросах ретрая.
     5. `returns_typed_indexing_error_after_5_retries_with_json_body_retry_after` — возврат `SearchError::Indexing { retry_after_ms, attempts: 5 }` после 5 повторов.
     6. `rejects_invalid_session_and_query_ids_before_transport` — валидация входных ID до сетевого вызова.
     7. `deterministic_uuid_v4_oracle_shape` — проверка формата UUID v4 (RFC 4122) и повторяемости детерминированного генератора.

## 3. RED-прогон

До добавления реализации в `search.rs` зафиксировано падение сборки тестов:

```
   Compiling discord-gui-spike v0.1.0 (/home/user/orca/workspaces/project/feature-task-163-search-202-session-uuid/rust/gui-spike)
error[E0432]: unresolved imports `discord_gui_spike::search::deterministic_uuid_v4`, `discord_gui_spike::search::generate_uuid_v4`, `discord_gui_spike::search::SearchSession`, `discord_gui_spike::search::MAX_INDEXING_RETRIES`
 --> tests/search_contract.rs:7:5
  |
7 |     deterministic_uuid_v4, generate_uuid_v4, DiscordSearch, RemoteSearch,...
  |     ^^^^^^^^^^^^^^^^^^^^^  ^^^^^^^^^^^^^^^^ no `generate_uuid_v4` in `search`
  |     |
  |     no `deterministic_uuid_v4` in `search`
8 |     SearchParams, SearchResponse, SearchSession, SearchTransport,
  |                                   ^^^^^^^^^^^^^ no `SearchSession` in `search`
9 |     DEFAULT_SEARCH_202_RETRY_AFTER_MS, MAX_INDEXING_RETRIES, MAX_SEARCH_2...
  |                                        ^^^^^^^^^^^^^^^^^^^^ no `MAX_INDEXING_RETRIES` in `search`

error[E0560]: struct `SearchParams` has no field named `search_session_id`
    --> tests/search_contract.rs:1079:9
     |
1079 |         search_session_id: Some(session_id.into()),
     |         ^^^^^^^^^^^^^^^^^ `SearchParams` does not have this field

error[E0560]: struct `SearchParams` has no field named `search_query_id`
    --> tests/search_contract.rs:1080:9
     |
1080 |         search_query_id: Some(query_id.into()),
     |         ^^^^^^^^^^^^^^^ `SearchParams` does not have this field

error[E0599]: no method named `with_offset` found for struct `SearchParams` in the current scope
    --> tests/search_contract.rs:1140:32
     |
1140 |     let page2 = initial_params.with_offset(25);
     |                                ^^^^^^^^^^^ method not found in `SearchParams`

error[E0599]: no method named `ensure_session_ids` found for struct `SearchParams` in the current scope
    --> tests/search_contract.rs:1165:17
     |
1165 |     auto_params.ensure_session_ids();
     |                 ^^^^^^^^^^^^^^^^^^ method not found in `SearchParams`

error: could not compile `discord-gui-spike` (test "search_contract") due to 18 previous errors
```

## 4. GREEN-прогон и проверки

1. **Контрактные тесты `search_contract`:**
   ```sh
   CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/gui-spike/Cargo.toml --test search_contract
   ```
   Результат:
   ```
   running 30 tests
   test decodes_hits_and_deep_index_flag ... ok
   test decodes_documents_indexed_from_200_ok_and_defaults_to_zero_when_omitted ... ok
   test no_token_is_typed_before_network ... ok
   test accepts_all_nine_confirmed_has_tokens_and_three_author_types ... ok
   test maps_network_error_without_exposing_details ... ok
   test maps_malformed_json_to_typed_error ... ok
   test debug_and_errors_do_not_expose_token ... ok
   test enforces_max_search_offset_9975_and_rejects_above_ceiling_before_transport ... ok
   test deterministic_uuid_v4_oracle_shape ... ok
   test maps_401_and_429_to_typed_errors ... ok
   test maps_other_http_errors_to_typed_errors_without_body_leak ... ok
   test rejects_empty_guild_id_before_transport ... ok
   test parses_retry_after_from_json_body_on_http_202_when_header_is_absent ... ok
   test rejects_invalid_session_and_query_ids_before_transport ... ok
   test rejects_negative_offset_before_transport ... ok
   test rejects_invalid_filter_values_before_transport ... ok
   test deserializes_ipc_camel_case_snake_case_and_nested_query_payload ... ok
   test includes_nsfw_and_attempts_query_params_and_validates_attempts_range ... ok
   test reuses_search_session_id_and_query_id_on_pagination_and_refreshes_query_id_on_new_query ... ok
   test storage_failure_is_typed_for_ui ... ok
   test retries_http_202_using_retry_after_from_json_body_and_preserves_session_ids ... ok
   test returns_immediate_indexing_envelope_on_http_202_without_sleeping_or_reading_body ... ok
   test returns_typed_indexing_error_when_all_5_retries_return_202 ... ok
   test sends_all_confirmed_re_filters_in_query ... ok
   test sends_confirmed_query_and_decodes_empty_result ... ok
   test returns_typed_indexing_error_after_5_retries_with_json_body_retry_after ... ok
   test retries_http_202_with_retry_after_header_ignoring_202_body_and_returns_200_results ... ok
   test tauri_command_reads_token_from_backend_and_is_registered ... ok
   test serializes_search_session_id_and_search_query_id_in_query_string ... ok
   test retries_http_202_against_local_mock_http_server_and_parses_200_ok ... ok

   test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.00s
   ```

2. **Unit-тесты библиотеки `search.rs`:**
   ```sh
   CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/gui-spike/Cargo.toml --lib search
   ```
   Результат:
   ```
   running 4 tests
   test search::tests::m13_constants_and_indexing_error_format_match_spec ... ok
   test search::tests::retry_after_header_parses_positive_integer_seconds_with_5000ms_fallback ... ok
   test search::tests::uuid_v4_generation_matches_rfc4122_shape ... ok
   test search::tests::retry_after_body_parses_float_seconds_when_header_absent ... ok

   test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out; finished in 0.00s
   ```

3. **Clippy:**
   ```sh
   CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo clippy --manifest-path rust/gui-spike/Cargo.toml --test search_contract -- -D warnings
   ```
   Результат: `Finished dev profile [unoptimized + debuginfo]`, 0 warnings, exit code 0.

4. **Rustfmt:**
   ```sh
   rustfmt --edition 2021 rust/gui-spike/src/search.rs rust/gui-spike/tests/search_contract.rs
   ```
   Результат: чистый форматированный код, exit code 0.

## 5. Файлы и контрольные суммы (md5)

```
bf5be02cc5522a2a27bf4e824cef0fc4  rust/gui-spike/src/search.rs
44bfb9f0236c9a8ac7fc7551d34ed0cb  rust/gui-spike/tests/search_contract.rs
```

## 6. Риски и безопасность

- **Безопасность токенов и тел:** все доменные ошибки `SearchError` и их представления в `Display` / `Debug` / `Serialize` очищены от чувствительных данных (токен, тело запроса, заголовки авторизации).
- **Валидация до сети:** `search_session_id`, `search_query_id`, `offset`, `limit`, `author_type`, `has`, `min_id`/`max_id` проверяются до обращения к сети и отклоняются с типизированной ошибкой `SearchError::InvalidArg`.
- **Обратная совместимость:** 100% совместимость со всеми существующими контрактами и тестами; если `search_session_id` / `search_query_id` не переданы, они не загрязняют URL запроса.
