# Техническое ревью (Review 83A): PR-54 (task-168 / BE-P2-04)

- **Задача**: task-168 / BE-P2-04: REST Route Query Spec, RFC 3986 percent-encoding, and pins/threads/messages/search path builders in `discord-core::rest::routes`
- **Пулл-реквест**: PR-54 (`feature/task-168-rest-routes-query-layer -> unstable`)
- **Ревьюер**: Technical Reviewer (Review 83A)
- **Дата**: 2026-10-31
- **Вердикт**: **APPROVED (Ready for Merge)**

---

## 1. Проверка области изменений (Git Status & Diff Scope)

- **Разрешённые файлы**:
  - `rust/crates/discord-core/src/rest/routes.rs`
  - `rust/crates/discord-core/tests/rest_routes.rs`
- **Фактический git status**:
  ```text
  M rust/crates/discord-core/src/rest/routes.rs
  M rust/crates/discord-core/tests/rest_routes.rs
  ```
  Индекс и рабочая копия не содержат лишних файлов. Общие файлы (`lib.rs`, `types.rs`, `Cargo.lock`, `Cargo.toml`) не затронуты.
- **Git diff**: 2 файла изменены (+1009 строк, -7 строк). Все изменения локализованы строго в рамках ТЗ задачи task-168.

---

## 2. Сборка, тесты и статический анализ (Clippy)

Запуск выполнен в общем каталоге артефактов `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`:

1. **Компиляция и прогон тестов**:
   ```bash
   CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml -p discord-core --test rest_routes --lib
   ```
   **Результат**:
   - `discord-core` lib unittests: `33 passed; 0 failed; finished in 0.42s`.
   - `rest_routes` integration tests: `16 passed; 0 failed; finished in 0.00s`.
   - **Итого**: 49/49 passed.

2. **Статический анализ Clippy**:
   ```bash
   CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo clippy --manifest-path rust/Cargo.toml -p discord-core --test rest_routes --lib -- -D warnings
   ```
   **Результат**: `0 warnings`, чистый прогон с флагом `-D warnings`.

---

## 3. Верификация уничтожения 5/5 мутаций (Mutation Testing)

| № | Мутация | Описание мутации | Результат проверки | Тест-киллер |
|---|---|---|---|---|
| **M1** | `CAPTURED_QUERY_PARAMS count != 45` | Изменение количества зафиксированных query-параметров из оракула `rest-d02.json` (например, 44 или 46). | **KILLED** | `captured_query_params_catalog_matches_re_oracle_45` проверяет точную длину `assert_eq!(CAPTURED_QUERY_PARAMS.len(), 45)` и сверяет вхождение ключевых параметров (`archived`, `limit`, `sort_by` и др.). |
| **M2** | Failing percent-encoding in `encode_query_component` | Пропуск кодирования спецсимволов `+`, `#`, `&`, пробелов, кириллицы или эмодзи (RFC 3986 unreserved boundary). | **KILLED** | `rfc3986_percent_encoding_matches_capture_and_standards` строго сверяет кодирование `+` -> `%2B`, `#` -> `%23`, `&` -> `%26`, пробела -> `%20`, а также живые эталоны из `capture-run2.jsonl` (`#💥┃правилатест` и `ProBot#🔴┃поговорить-тут`). Любое отклонение ломает тест. |
| **M3** | Missing required `archived` check in `validate_spec` / `build_thread_search_path` | Допуск запроса к поиску тредов без обязательного параметра `archived` или игнорирование `required: true` в `validate_spec`. | **KILLED** | `query_builder_assembly_and_spec_validation` валидирует отказ с `QueryError::MissingRequiredParam("archived")` при отсутствии обязательного параметра; `thread_search_path_matches_oracle_and_validates` проверяет, что `ThreadSearchParams::empty()` возвращает `QueryError::MissingRequiredParam("archived")`. |
| **M4** | `DEFAULT_PINS_LIMIT != 25` | Изменение дефолтного значения лимита закреплённых сообщений (`DEFAULT_PINS_LIMIT` другое число вместо 25). | **KILLED** | `channel_pins_path_matches_oracle_and_validates` явно проверяет константу `assert_eq!(DEFAULT_PINS_LIMIT, 25)` и проверяет вызов `build_channel_pins_path(oracle_channel, None)` с генерацией `?limit=25`. |
| **M5** | Allowing multiple cursors in `build_channel_messages_path` | Разрешение комбинаций `before + after`, `before + around`, `after + around`. | **KILLED** | `channel_messages_path_matches_oracle_and_validates` тестирует все 3 пары конфликтов (`conflict1`, `conflict2`, `conflict3`) и ожидает `QueryError::MutuallyExclusiveParams`. |

**Итог по мутациям**: 5 из 5 мутаций полностью покрыты тестами и гарантированно уничтожаются (`5/5 KILLED`).

---

## 4. Контрольные суммы файлов (MD5)

- `e577cfcf731801a758f51004a9b598c8` `rust/crates/discord-core/src/rest/routes.rs`
- `c5f6674330a64d8640e04096d408570e` `rust/crates/discord-core/tests/rest_routes.rs`

---

## 5. Итоговое заключение

- Блокирующие замечания: `P0 = 0`, `P1 = 0`.
- Качество архитектуры и типизации: `QueryParamSpec`, `QueryParamType`, `QueryBuilder`, `QueryError`, RFC 3986 percent-encoding и типобезопасные конструкторы путей спроектированы в строгом соответствии с контрактами D-02 и живыми сетевыми трассами.
- Мутационное тестирование: `5/5` пройдено.
- **Статус**: PR-54 полностью одобрен и готов к слиянию (`Ready for Merge`).
