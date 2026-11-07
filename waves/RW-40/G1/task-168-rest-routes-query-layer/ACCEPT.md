# Приёмка PM: task-168 (BE-P2-04) — Типизированный query-слой маршрутов в discord-core::rest::routes, RFC 3986 percent-encoding и конструкторы путей pins/threads/messages/search

- **Статус:** `Ready for Technical Review` (Этап 1 PM Приёмки — **ОДОБРЕНО**)
- **Ветка:** `feature/task-168-rest-routes-query-layer`
- **Пулл-реквест:** `PR-54` (`feature/task-168-rest-routes-query-layer -> unstable`)

## Сверка с продуктовым ТЗ и DoD
1. **Типизированная спецификация query-параметров в `RouteSpec`**:
   - В `routes.rs` добавлены `QueryParamType`, `QueryParamSpec`, поле `query_params: &'static [QueryParamSpec]` в `RouteSpec` и полный каталог `CAPTURED_QUERY_PARAMS` (45 имён из `rest-d02.json`).
2. **RFC 3986 percent-encoding и `QueryBuilder`**:
   - Реализованы `encode_query_component` (экранирование `+`, `#`, `&`, пробелов, кириллицы и эмодзи в точном соответствии с `capture-run2.jsonl`) и `QueryBuilder` с валидацией обязательных параметров (`QueryError`).
3. **Типизированные конструкторы URL-путей**:
   - Реализованы `build_channel_pins_path`, `build_thread_search_path`, `build_channel_messages_path`, `build_guild_search_path`, `build_channel_search_path` (сверены с `rest-static.json` и `capture-run2.jsonl`).
   - Все 16 тестов в `rest_routes.rs` и 33 в `lib.rs` проходят (`49/49 PASS`, `clippy 0 warnings`).

Передано на **Техническое ревью (Review 83A)** (`5/5` мутаций).
