# Отчёт воркера — `TASK-617` (`feature/task-617-rest-search-bff`)

- **Карточка / волна:** `TASK-617` (RW-40, отдел `G1 Backend`)
- **Тема:** Полнотекстовый поиск по гильдии `GET /guilds/{id}/messages/search` (BFF для REST-поиска сообщений)
- **Ветка:** `feature/task-617-rest-search-bff` (от `unstable`)
- **Оракулы RE:**
  - `/home/sanya/re-deep/decoded/chunk-b929d27a8474c41c.js` (`SEARCH_MESSAGES: (e) => /guilds/${e}/messages/search`, `MAX_SEARCH_OFFSET = 9975`, `limit = 25`, 9 токенов `has`, 3 значения `author_type`)
  - `waves/RW-40/SEARCH-202-NSFW-RE.md` (§2–§6: query-параметры, `202 Accepted` индексация, `429 Too Many Requests`, `messages: Vec<Vec<Message>>`, `total_results`, `documents_indexed`, `doing_deep_historical_index`)
  - `docs/re/rest-d02.md` (§6.1: 6 обязательных заголовков Discord REST v9 с `X-Discord-Locale: ru`)

---

## 1. Сделано

1. **Ядро REST v9 (`rust/crates/discord-core/src/rest/search.rs`, экспортирован в `rust/crates/discord-core/src/rest/mod.rs`):**
   - Реализована структура [`SearchQueryRequest`](file:///home/sanya/orca/workspaces/project/task-617-rest-search-bff/rust/crates/discord-core/src/rest/search.rs#L118-L162) и парсер инлайн-фасетов `parse_and_normalize` / `apply_inline_query_tokens` для токенов `from:`, `mentions:`, `has:`, `in:`, `before:`, `after:`, `pinned:`, `author_type:`, `link_hostname:`, `attachment_extension:`, `attachment_filename:`.
   - Реализована строгая предсетевая валидация (`content <= 512` символов, `offset <= 9975`, `limit` `1..=25`, Snowflake ID, 9 подтверждённых токенов `has` и 3 токена `author_type` с поддержкой отрицания `-`, проверка `min_id <= max_id` и непустоты критериев).
   - Реализованы билдеры `build_guild_search_http_request` (`/api/v9/guilds/{guild_id}/messages/search?...`) и `build_channel_search_http_request` (`/api/v9/channels/{channel_id}/messages/search?...`) с полным набором из 6 обязательных заголовков Discord REST v9 (`Authorization`, `X-Super-Properties`, `X-Discord-Locale: ru`, `X-Discord-Timezone`, `X-Debug-Options: bugReporterEnabled`, `Accept-Language: ru`).
   - Реализован парсер `parse_search_http_response` на базе `discord_json` (без `serde`):
     - `200 OK` → [`GuildSearchRestResult`](file:///home/sanya/orca/workspaces/project/task-617-rest-search-bff/rust/crates/discord-core/src/rest/search.rs#L549-L564) с вложенными контекстными блоками [`SearchContextGroup`](file:///home/sanya/orca/workspaces/project/task-617-rest-search-bff/rust/crates/discord-core/src/rest/search.rs#L514-L546) (`hit_message()`, `context_before()`, `context_after()`), вложениями [`SearchedAttachment`](file:///home/sanya/orca/workspaces/project/task-617-rest-search-bff/rust/crates/discord-core/src/rest/search.rs#L477-L489), `embeds_count`, `analytics_id`, `total_results`, `doing_deep_historical_index`, `documents_indexed`;
     - `202 Accepted` → конверт индексации (`indexing: true`, `retry_after_ms`, `documents_indexed`);
     - `429 Too Many Requests` → `SearchRestError::RateLimited { retry_after_ms, is_global }` + расчёт экспоненциального бэкоффа `compute_429_exponential_backoff_ms` (с потолком `30_000` мс);
     - `401`, `403`, `404`, `4xx`, `5xx` → типизированные варианты [`SearchRestError`](file:///home/sanya/orca/workspaces/project/task-617-rest-search-bff/rust/crates/discord-core/src/rest/search.rs#L576-L640).
2. **BFF-слой (`rust/gui-spike/src/search.rs`):**
   - Расширен [`SearchParams`](file:///home/sanya/orca/workspaces/project/task-617-rest-search-bff/rust/gui-spike/src/search.rs#L142-L179) поддержкой фасетов `link_hostname`, `attachment_extension`, `attachment_filename` и нормализацией инлайн-токенов (`SearchParams::from_raw_query`, `SearchParams::normalized`, `SearchParams::canonical_cache_key`) с полным сохранением обратной совместимости существующих 30 контрактных тестов.
   - Добавлены структуры [`SearchHitMessage`](file:///home/sanya/orca/workspaces/project/task-617-rest-search-bff/rust/gui-spike/src/search.rs#L455-L467) и [`SearchHitGroup`](file:///home/sanya/orca/workspaces/project/task-617-rest-search-bff/rust/gui-spike/src/search.rs#L470-L486), а также методы `SearchResponse::hit_groups()` и `SearchResponse::primary_hits()` для извлечения контекстных групп `messages[][]`.
   - В `DiscordSearch::execute_with_retry` добавлен цикл повторов при `HTTP 429 Too Many Requests` с экспоненциальным бэкоффом (`MAX_SEARCH_429_RETRIES = 3`, `compute_429_exponential_backoff_ms`) наряду с существующим циклом повторов `HTTP 202 Accepted` (`MAX_SEARCH_202_RETRIES = 5`).
   - В `DiscordSearch` реализован потокобезопасный LRU-кэш недавних поисковых запросов (`search_cached`, `cache_len`, `invalidate_guild_cache`, `clear_cache`, `MAX_SEARCH_CACHE_ENTRIES = 64`), кэширующий только завершённые ответы `200 OK` (`!indexing && !doing_deep_historical_index`) по каноническому ключу `(guild_id, normalized_params_without_transient_ids)`.
3. **Контрактные тесты (`rust/gui-spike/tests/search_contract.rs`):**
   - Добавлены 5 контрактных тестов `task_617_*` (общее число тестов в `search_contract` выросло с 30 до 35, плюс 2 unit-теста в `discord-core` и 4 unit-теста в `discord-gui-spike`).

---

## 2. Red-прогон (Strict TDD)

Команда:
```sh
CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target flock /tmp/discord-lab-cargo.lock cargo test --manifest-path rust/gui-spike/Cargo.toml --test search_contract
```
Лог сохранён в [`red.log`](file:///home/sanya/orca/workspaces/project/task-617-rest-search-bff/waves/RW-40/G1/task-617-rest-search-bff/red.log) — падение компиляции (код возврата `101`, `unresolved import discord_core::rest::search`, отсутствуют методы `from_raw_query`, `hit_groups`, `primary_hits`, `search_cached`, `cache_len`, `invalidate_guild_cache`, `MAX_SEARCH_429_RETRIES`).

---

## 3. Green-прогон и Targeted-тесты (`T0`)

Команда:
```sh
CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target flock /tmp/discord-lab-cargo.lock bash -c '
  cargo test --manifest-path rust/Cargo.toml -p discord-core --lib search &&
  cargo test --manifest-path rust/gui-spike/Cargo.toml --test search_contract &&
  cargo test --manifest-path rust/gui-spike/Cargo.toml --lib search
'
```
Лог сохранён в [`green.log`](file:///home/sanya/orca/workspaces/project/task-617-rest-search-bff/waves/RW-40/G1/task-617-rest-search-bff/green.log):
- `discord-core --lib search`: `2 passed; 0 failed`
- `discord-gui-spike --test search_contract`: `35 passed; 0 failed`
- `discord-gui-spike --lib search`: `4 passed; 0 failed`
- `rustfmt` и `cargo clippy` по затронутым файлам — чисто (`0` предупреждений).

---

## 4. Мутационное тестирование (`5/5 Killed`)

Лог сохранён в [`mutations.log`](file:///home/sanya/orca/workspaces/project/task-617-rest-search-bff/waves/RW-40/G1/task-617-rest-search-bff/mutations.log):
1. **M1 (`discord-core/src/rest/search.rs`):** `MAX_SEARCH_OFFSET: 9975 -> 10000` — **KILLED** (`task_617_core_rest_search_normalizes_inline_facets_and_builds_v9_request_with_6_auth_headers`).
2. **M2 (`discord-core/src/rest/search.rs`):** `SearchContextGroup::hit_message` (`m.hit -> !m.hit`) — **KILLED** (`task_617_core_rest_search_parses_200_nested_context_groups_202_indexing_and_errors`).
3. **M3 (`discord-core/src/rest/search.rs`):** `compute_429_exponential_backoff_ms` (`1_u64 << shift -> 1_u64`) — **KILLED** (`task_617_core_rest_search_parses_200_nested_context_groups_202_indexing_and_errors`, `task_617_bff_retries_http_429_with_exponential_backoff_and_succeeds_or_fails_after_budget`).
4. **M4 (`gui-spike/src/search.rs`):** `MAX_SEARCH_429_RETRIES: 3 -> 1` — **KILLED** (`task_617_bff_retries_http_429_with_exponential_backoff_and_succeeds_or_fails_after_budget`).
5. **M5 (`gui-spike/src/search.rs`):** кэширование незаконченных `202 Accepted` ответов в `search_cached` — **KILLED** (`task_617_bff_caches_recent_200_search_results_and_bypasses_cache_on_202_or_invalidate`).

После проверки все 5 мутаций откачены бит-в-бит (`md5sum` совпадает).

---

## 5. Изменённые файлы и `md5sum`

| Файл | `md5sum` | Статус |
|---|---|---|
| [`rust/crates/discord-core/src/rest/search.rs`](file:///home/sanya/orca/workspaces/project/task-617-rest-search-bff/rust/crates/discord-core/src/rest/search.rs) | `1080c8583fd64c5af999e3927f2f1b81` | Новый модуль |
| [`rust/crates/discord-core/src/rest/mod.rs`](file:///home/sanya/orca/workspaces/project/task-617-rest-search-bff/rust/crates/discord-core/src/rest/mod.rs) | `a2a284ad5b9b7fc92650d4469c9b8240` | Изменён (`pub mod search;`) |
| [`rust/gui-spike/src/search.rs`](file:///home/sanya/orca/workspaces/project/task-617-rest-search-bff/rust/gui-spike/src/search.rs) | `7480942682dcb45680c352b98f662b72` | Изменён |
| [`rust/gui-spike/tests/search_contract.rs`](file:///home/sanya/orca/workspaces/project/task-617-rest-search-bff/rust/gui-spike/tests/search_contract.rs) | `fa945c92abb85caaaab6538c87af61ea` | Изменён (+5 контрактных тестов) |

---

## 6. Риски и совместимость

- **Обратная совместимость:** все 30 ранее существовавших контрактных тестов в `search_contract.rs` проходят без изменений; сериализация `SearchParams` и `SearchResponse` полностью обратно совместима с UI (`TASK-618`) и Tauri IPC (`search_remote`).
- **Кэширование `CARGO_TARGET_DIR`:** поскольку добавлен новый модуль `discord_core::rest::search`, при сборке в общем `CARGO_TARGET_DIR` после переключения веток рекомендуется пересобирать `-p discord-core` перед запуском тестов `discord-gui-spike`.
