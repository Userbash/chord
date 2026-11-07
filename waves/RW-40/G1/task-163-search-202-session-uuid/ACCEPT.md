# PM Acceptance — task-163 (`BE-P1-05`: Search 202 Indexing Retry, `search_session_id` / `search_query_id` & `include_nsfw`)

- **Статус PM:** `Accepted for Technical Review` (`Ready for Technical Review`)
- **Проверенные файлы:**
  - `rust/gui-spike/src/search.rs`
  - `rust/gui-spike/tests/search_contract.rs`
- **Соответствие ТЗ и оракулу:**
  1. Реализованы `search_session_id` и `search_query_id` (UUID v4, переиспользование при пагинации `with_offset` / `next_page`, смена `search_query_id` при `new_query_in_session`).
  2. Обработка `HTTP 202 Accepted` с извлечением `retry_after` из JSON-тела или заголовка `Retry-After`, повтор до `MAX_INDEXING_RETRIES = 5` и ошибка `SearchError::Indexing { retry_after_ms, attempts: 5 }`.
  3. 30/30 контрактных тестов `search_contract` проходят (`0 fail`, `clippy 0`).
- **Следующий шаг:** Техническое ревью (`Review 77A`) с проверкой `5/5` мутаций.
