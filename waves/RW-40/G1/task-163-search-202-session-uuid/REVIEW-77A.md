# Technical Review: task-163 (BE-P1-05: Поиск — HTTP 202 Indexing, search_session_id / search_query_id, include_nsfw)

- **Дата и время:** 2026-10-30 23:53 UTC+5
- **Ревьюер:** Technical Reviewer (Orca Worker `term_9c00720d-ade6-4e6c-96dc-54e6fb957fdc`)
- **Статус карточки:** Ready for Technical Review -> **Ready for Merge**
- **Вердикт:** **PASS / Ready for Merge**

---

## 1. Проверяемые артефакты и контрольные суммы

| Файл | Описание | MD5 до ревью | MD5 после ревью | Статус совпадения |
|---|---|---|---|---|
| `rust/gui-spike/src/search.rs` | Реализация логики поиска, сессий и HTTP 202 | `bf5be02cc5522a2a27bf4e824cef0fc4` | `bf5be02cc5522a2a27bf4e824cef0fc4` | 100% Match |
| `rust/gui-spike/tests/search_contract.rs` | Контрактные тесты поиска (30 тестов) | `44bfb9f0236c9a8ac7fc7551d34ed0cb` | `44bfb9f0236c9a8ac7fc7551d34ed0cb` | 100% Match |

---

## 2. Анализ реализации и соответствия архитектурным требованиям

1. **RFC 4122 UUID v4 генераторы (`generate_uuid_v4`, `deterministic_uuid_v4`):**
   - Корректная генерация случайного UUID через `discord_core::rest::identity::uuid_v4(&SystemSources)`.
   - Наличие детерминированного генератора `deterministic_uuid_v4(seed)` с воспроизводимым LCG для надёжных контрактных тестов без флаков.
2. **Управление идентификаторами сессии поиска (`SearchSession`, `SearchParams`):**
   - Добавлены поля `search_session_id` и `search_query_id`.
   - Сериализация в query-строку в порядке, подтверждённом оракулом `capture-run2.jsonl`.
   - Поддержка десериализации camelCase, snake_case и вложенного объекта `query: { ... }`.
   - Реализована строгая валидация ID до сетевого вызова (`trim().is_empty()` и отсутствие запрещённых символов).
   - Методы пагинации `with_offset` и `next_page` сохраняют оба идентификатора.
   - Метод `new_query_in_session` сохраняет `search_session_id`, сбрасывает `offset` и генерирует новый `search_query_id`.
3. **Обработка HTTP 202 Accepted и `retry_after`:**
   - Функция `parse_retry_after_ms` проверяет заголовок `Retry-After` (секунды -> мс), а при его отсутствии корректно парсит `retry_after` из тела JSON (поддержка float/int/str) с дефолтным fallback `5000` мс.
   - Заголовок имеет приоритет над телом.
   - Цикл `execute_with_retry` выполняет до `MAX_SEARCH_202_RETRIES` (5 повторов) с передачей `attempts = 1..=5` и возвращает типизированную ошибку `SearchError::Indexing { retry_after_ms, attempts: 5 }` при исчерпании лимита.
4. **Параметр `include_nsfw`:**
   - Передаётся в query-строку только при `Some(true)`. При `None` и `Some(false)` не добавляется в URL.
5. **Безопасность и маскирование:**
   - Ни один токен или тело запроса не логируются и не утекают в ошибки `SearchError`.

---

## 3. Результаты запуска тестовых наборов

### 3.1. Контрактные тесты (`search_contract.rs`)
```sh
CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/gui-spike/Cargo.toml --test search_contract
```
- **Результат:** 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.00s.

### 3.2. Unit-тесты библиотеки (`src/search.rs`)
```sh
CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/gui-spike/Cargo.toml --lib search
```
- **Результат:** 4 passed; 0 failed; 0 ignored; 0 measured; finished in 0.00s.

### 3.3. Clippy
```sh
CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo clippy --manifest-path rust/gui-spike/Cargo.toml --test search_contract -- -D warnings
```
- **Результат:** 0 warnings, exit code 0.

---

## 4. Верификация мутационным тестированием (5/5 ручных мутаций)

Для подтверждения полноты покрытия контракта были внесены 5 ручных мутаций в продуктовый код `rust/gui-spike/src/search.rs`. Все мутанты были успешно обнаружены и убиты тестами.

| № | Мутация | Описание мутации в `rust/gui-spike/src/search.rs` | Убивающий тест | Результат |
|---|---|---|---|---|
| **M1** | Инверсия условия `include_nsfw` | Замена `params.include_nsfw == Some(true)` на `Some(false)` (строка ~665) | `includes_nsfw_and_attempts_query_params_and_validates_attempts_range`, `retries_http_202_against_local_mock_http_server_and_parses_200_ok` | **KILLED** (2 failed) |
| **M2** | Отключение парсинга `retry_after` из тела JSON | Игнорирование `body` в `parse_retry_after_ms` и возврат константы 5000 мс (строка ~718) | `parses_retry_after_from_json_body_on_http_202_when_header_is_absent`, `retries_http_202_using_retry_after_from_json_body_and_preserves_session_ids`, `returns_typed_indexing_error_after_5_retries_with_json_body_retry_after` | **KILLED** (3 failed) |
| **M3** | Искажение имени query-параметра сессии | Замена `search_session_id={}` на `session_id={}` (строка ~649) | `retries_http_202_using_retry_after_from_json_body_and_preserves_session_ids`, `serializes_search_session_id_and_search_query_id_in_query_string` | **KILLED** (2 failed) |
| **M4** | Нарушение сохранения `query_id` при пагинации | Принудительная генерация нового `search_query_id` внутри `with_offset` (строка ~198) | `reuses_search_session_id_and_query_id_on_pagination_and_refreshes_query_id_on_new_query` | **KILLED** (1 failed) |
| **M5** | Ослабление валидации идентификаторов | Пропуск проверки `trim().is_empty()` для `search_session_id` и `search_query_id` (строки ~594, ~600) | `rejects_invalid_session_and_query_ids_before_transport` | **KILLED** (1 failed) |

**Итог мутационного тестирования:** 5/5 мутантов убито (100% kill rate). Исходные файлы полностью восстановлены, хеши `md5sum` сверены.

---

## 5. Итоговое заключение

- Код полностью соответствует спецификации `SEARCH-202-NSFW-SPEC.md` и оракулам `reference-oracle`.
- Все 34 теста (30 контрактных + 4 unit) зелёные.
- Clippy проходит без замечаний.
- Мутационный тест 5/5 пройден.
- Контрольные суммы файлов совпадают с отчётом воркера.

**Вердикт:** **PASS / Ready for Merge** в `unstable`.
