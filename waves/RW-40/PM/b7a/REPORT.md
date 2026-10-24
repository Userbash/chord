# B7a — guild-search backend: отчёт G1

Дата: 2026-10-24. Воркер: G1 Backend/Network. PM: DeepSeek.
Карточка: B7a (`waves/RW-40/TASKS.md:63`). Контракт: `waves/RW-40/ATTACH-RE.md` §1.

## 1. Статус

**done (backend, offline).** Маршрут `GET /api/v9/guilds/{id}/messages/search` реализован и
покрыт fake-тестами; live-проверка на реальном Discord не выполнялась (окно/сеть запрещены).
Существующие тесты не ослаблялись и не переписывались — только добавлены новые.

## 2. Сделано

| Пункт ТЗ | Где | Состояние |
|---|---|---|
| маршрут в каталоге | `crates/discord-core/src/rest/routes.rs:120` | было, проверено тестом `guild_search_route_is_catalogued` |
| query: `content`, `channel_id`, `sort_by`, `sort_order`, `offset` | `gui-spike/src/search.rs` (`execute`) | было, проверено точным assert на target |
| ответ `messages` / `total_results` / `doing_deep_historical_index` | `SearchResponse` | было, добавлен тест на непустой ответ + `true` |
| 401 / 429 / сеть / битый JSON | `execute` + `map_api_error` | было, добавлены тесты на прочие 4xx/5xx |
| `MissingToken` | `DiscordSearch::new/with_base`, `search_remote` | было, добавлены тесты |
| токен не в логах/Debug/ошибках | `Debug` через `finish_non_exhaustive`, `SearchError` без полей токена | проверено тестом |
| команда Tauri `search_remote` | `gui-spike/src/main.rs:141-155`, `invoke_handler!` | было, добавлен контрактный тест |

Изменения в этой сессии:

1. `gui-spike/src/main.rs` — только форматирование списка команд в `wiring_tests`
   (отступ `search_remote`, `cargo fmt --check` был красным).
2. `gui-spike/tests/search_contract.rs` — **добавлены** 5 тестов (6 → 11), ни один
   существующий тест не изменён:
   - `rejects_empty_guild_id_before_transport` — пустой/пробельный guild_id → `InvalidArg`,
     транспорт не вызывается;
   - `maps_other_http_errors_to_network_without_body_leak` — 403/404/500/503 → `Network`,
     тело ответа не попадает в ошибку;
   - `decodes_hits_and_deep_index_flag` — непустой `messages`, `total_results=2`,
     `doing_deep_historical_index=true`;
   - `debug_and_errors_do_not_expose_token` — `Debug` `DiscordSearch` и все варианты
     `SearchError` (Display/Debug/JSON) не содержат токен;
   - `tauri_command_reads_token_from_backend_and_is_registered` — `search_remote` берёт
     токен из `AuthBackend`, отдаёт `SearchError::MissingToken` и зарегистрирован в
     `invoke_handler!`.

Честная оговорка: это guard-тесты на уже реализованный контракт, они прошли сразу
(red-фаза отсутствует, т.к. продуктовый код по существу не менялся). Продуктовый код
`search.rs` и маршрут перепроверены чтением и не потребовали правок.

## 3. Прогоны

| Проверка | Результат | Лог |
|---|---|---|
| `cargo test --offline -p discord-core` | **574 passed / 0 failed** (база 573 + 1 маршрутный тест B7a) | `core-test.log` |
| `cargo test --offline --manifest-path gui-spike/Cargo.toml` | **132 passed / 0 failed** (база 121 + 11 search) | `gui-test.log` |
| `cargo clippy --offline -p discord-core --all-targets -- -D warnings` | 0 | `gate.log` |
| `cargo clippy --offline --manifest-path gui-spike/Cargo.toml --all-targets -- -D warnings` | 0 | `gate.log` |
| `cargo fmt -p discord-core -- --check` | 0 | `gate.log` |
| `cargo fmt --manifest-path gui-spike/Cargo.toml -- --check` | 0 | `gate.log` |
| `--test search_contract` | **11 passed / 0 failed** | `gui-test.log` |

## 4. md5 файлов

```
d50e933a3102fdd89fc4e18e861ed3e1  rust/gui-spike/src/search.rs
d6db06da6f6111afc2dc4561bcea293d  rust/gui-spike/tests/search_contract.rs
036b49b6619efe1ade8914df6ab25370  rust/gui-spike/src/lib.rs
974cc7e7e6058939d21810a92222ca5b  rust/gui-spike/src/main.rs
695a8732c275ef6797c6f8745639c63d  rust/crates/discord-core/src/rest/routes.rs
a1767cc9499fbb72d3aa4740f9cd7eaf  rust/crates/discord-core/tests/rest_routes.rs
```

`ui/**` и `fixtures/**` не изменялись (проверка: нет файлов новее правки теста).

## 5. Остаток / открытые вопросы

1. **202 Accepted** (индекс ещё строится) в дампе не подтверждён; сейчас не-2xx, кроме
   401/429 → `Network`. Нужен capture, иначе поведение по догадке.
2. `retry_after_ms` в ветке «сырой статус 429» всегда 0 (заголовки `Retry-After` не
   разбираются) — так же, как в `messages.rs`/`reactions.rs`. Решение стоит принимать
   единым слоем ядра, а не только в поиске.
3. `search_session_id` / `search_query_id` есть в дампе `[D]`, но вне scope B7a
   (ATTACH-RE §1, вывод). Добавлять вместе с UI-пагинацией B7.
4. Прочие 6 фильтров (автор, упоминания, дата, тип сообщения, тип автора, закрепление),
   лимит страницы, max offset, `/channels/{id}/messages/search` — **gap**, только UI/локально.
5. UI-путь B7 (`ui/features/search-overlay.js`, сетевой fallback) — G2, не эта карточка.
6. Live-проверка маршрута на реальном Discord не выполнялась (окно/сеть запрещены).
