# RW-40 / B7b-SEARCH-FILTERS-BACKEND — подтверждённые фильтры поиска в `search.rs`

- **batch/карточка:** `RW-40 / B7b-SEARCH-FILTERS-BACKEND` (`waves/RW-40/reference-oracle-REA.md` §2.3–2.4);
- **роль:** Воркер G1 GUI-Spike Rust Backend (`CARGO_TARGET_DIR=/tmp/target-g1-spike`);
- **сделано:**
  1. В `rust/gui-spike/src/search.rs` структура `SearchParams` расширена 8 подтверждёнными фильтрами из `reference-oracle-REA.md` §2.3–2.4 (`author_id`, `mentions`, `has`, `pinned`, `author_type`, `min_id`, `max_id`, `limit`) в дополнение к базовым 5 полям (`content`, `channel_id`, `sort_by`, `sort_order`, `offset`).
  2. Экспортированы константы оракула `MAX_SEARCH_LIMIT = 25` (`iG`/`T_y`), `SEARCH_HAS_VALUES` (9 значений: `link, embed, poll, snapshot, file, video, image, sound, sticker`) и `SEARCH_AUTHOR_TYPES` (3 значения: `user, bot, webhook`).
  3. Десериализация `SearchParams` поддерживает как `camelCase` и `snake_case` поля на верхнем уровне (включая строковые и массивные представления `author_id`/`mentions`/`has`/`channel_id`), так и вложенный объект `query` (`params.query`, отправляемый `ui/features/search-overlay.js` по карточке STUB-14).
  4. В `DiscordSearch::execute` добавлена предсетевая валидация (`limit` в `1..=25`, `has` по 9 токенам с поддержкой `-`, `author_type` по 3 значениям с поддержкой `-`, непустые `author_id`/`mentions`, snowflake-проверка `min_id`/`max_id` и порядок `min_id <= max_id`) и сериализация параметров в query-string.
  5. В `rust/gui-spike/tests/search_contract.rs` добавлены 4 контрактных теста (`sends_all_confirmed_re_filters_in_query`, `accepts_all_nine_confirmed_has_tokens_and_three_author_types`, `deserializes_ipc_camel_case_snake_case_and_nested_query_payload`, `rejects_invalid_filter_values_before_transport`); существующие 12 тестов сохранены без ослаблений.
- **red:** `(cd rust/gui-spike && CARGO_TARGET_DIR=/tmp/target-g1-spike cargo test --offline -p discord-gui-spike --test net_contract --test search_contract)` → Pass 1 (нетронутый `src/search.rs`): `error[E0560]`/`error[E0609]` по 8 новым полям (`exit=101`); Pass 2 (пассивные поля структуры до реализации `execute()` и serde-нормализации): `FAILED. 12 passed; 4 failed`, `exit=101`; оба прогона записаны в `waves/RW-40/PM/b7b-search/red.log`;
- **green:** `(cd rust/gui-spike && CARGO_TARGET_DIR=/tmp/target-g1-spike cargo test --offline -p discord-gui-spike --test net_contract --test search_contract)` → `ok. 24 passed; 0 failed` (`net_contract`) + `ok. 16 passed; 0 failed` (`search_contract`), `exit=0`, лог `waves/RW-40/PM/b7b-search/green.log`;
- **targeted:** `waves/RW-40/PM/b7b-search/green.log` — 16 тестов `search_contract` (было 12, +4 новых);
- **oracle:** `waves/RW-40/reference-oracle-REA.md` §2.3–2.4 (`4d2a64cb5df5fc4e_0@7564745…7569200`, `@7550420`, `@7559957`, `@7542421`, `@318812`), формула snowflake `(timestamp_ms - 1420070400000) << 22`;
- **генерация:** не применимо;
- **мутации:** 6/6 `KILLED`, эквивалентных — 0, `INCONCLUSIVE` — 0, survivors — 0 (`waves/RW-40/PM/b7b-search/mutations.log`):
  - `MUT-1-drop-limit-ceiling` (md5 `23ff6d65a77ae6a96b8802b05e2af979`) → `KILLED` тестом `rejects_invalid_filter_values_before_transport` (`exit=101`);
  - `MUT-2-drop-has-validation` (md5 `87f67f30fbb928c428f6cce766a3c01a`) → `KILLED` тестом `rejects_invalid_filter_values_before_transport` (`exit=101`);
  - `MUT-3-drop-author-type-validation` (md5 `04460624986849387176242043ac2167`) → `KILLED` тестом `rejects_invalid_filter_values_before_transport` (`exit=101`);
  - `MUT-4-drop-min-max-order-check` (md5 `972cbe5107addaa2743c7ee33902d654`) → `KILLED` тестом `rejects_invalid_filter_values_before_transport` (`exit=101`);
  - `MUT-5-drop-pinned-param` (md5 `12376e3d3d987a887f2238b2f109ea26`) → `KILLED` тестами `sends_all_confirmed_re_filters_in_query`, `accepts_all_nine_confirmed_has_tokens_and_three_author_types`, `deserializes_ipc_camel_case_snake_case_and_nested_query_payload` (`exit=101`);
  - `MUT-6-drop-nested-query-fallback` (md5 `6a1679fa054ecfe9a5194503c5d43539`) → `KILLED` тестом `deserializes_ipc_camel_case_snake_case_and_nested_query_payload` (`exit=101`);
- **baseline/restore:** baseline `rust/gui-spike/src/search.rs` = `6bae3ea23dfe31d1929fe3f7eb926fd0`, `rust/gui-spike/tests/search_contract.rs` = `a02006481e7e97ab26667367be8ce902` (`exit=0`, 0 skip); после каждого мутанта выполнен restore и проверено совпадение md5 (`6bae3ea23dfe31d1929fe3f7eb926fd0`);
- **стабильность:** все 16 тестов `search_contract` полностью детерминированы (фейковый транспорт в памяти, без сети/таймеров);
- **gate:worker:** в режиме batch полный `gate:worker` запускается на integration-лейне; `rustfmt --edition 2021 --check` по изменённым файлам чист (`exit=0`);
- **файлы:**
  - `rust/gui-spike/src/search.rs` — `6bae3ea23dfe31d1929fe3f7eb926fd0`
  - `rust/gui-spike/tests/search_contract.rs` — `a02006481e7e97ab26667367be8ce902`
  - `waves/RW-40/PM/b7b-search/red.log` — `ddc4263e5099f54e7db4df1fec268e73`
  - `waves/RW-40/PM/b7b-search/green.log` — `8f076d771844f66805ac5041f3a60c28`
  - `waves/RW-40/PM/b7b-search/mutations.log` — `a52aac38101e2754741994ee4587e2d2`
- **риски/вопросы:**
  - UI в `ui/features/search-overlay.js` (карточка STUB-14) передаёт фильтры во вложенном `params.query`, который теперь автоматически разбирается `SearchParams` и уходит в REST-запрос; при этом `ui/**` в рамках этой карточки не изменялся.
  - Обработка `202 Accepted` с `retry-after` (≤5 попыток, reference-oracle-REA §2.2) и `include_nsfw` (§2.4) не входили в список 8 полей ТЗ и остаются предметом отдельной карточки.
- **следующий шаг:** передать `B7b-SEARCH-FILTERS-BACKEND` на ревью и приёмку Team Lead.
