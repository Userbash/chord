# Отчёт по задаче task-168 (BE-P2-04: Query-слой маршрутов в discord-core::rest::routes — описание обязательных/опциональных query-параметров, percent-encoding спецсимволов и кириллицы, конструкторы URL для pins/threads/messages/search)

- **Карточка**: task-168 / BE-P2-04
- **Исполнитель**: G1 Backend (Rust Core & Tauri BFF)
- **Ветка**: `feature/task-168-rest-routes-query-layer`
- **Дата**: 2026-10-31

---

## 1. Что сделано

1. **Типизированное описание query-параметров и спецификация в `RouteSpec`**:
   - В `rust/crates/discord-core/src/rest/routes.rs` добавлены типы `QueryParamType` (`String`, `Integer`, `Boolean`, `Snowflake`, `Uuid`) и `QueryParamSpec` (`name`, `required`, `param_type`) с конструкторами `QueryParamSpec::required` и `QueryParamSpec::optional`.
   - В `RouteSpec` добавлено поле `pub query_params: &'static [QueryParamSpec]`.
   - Для сохранения 100% обратной совместимости конструктор `route` инициализирует `query_params: &[]`, а маршруты с параметрами используют `route_with_query`.
   - Определены спецификации параметров для ключевых маршрутов:
     - `MESSAGES_QUERY_PARAMS`: `limit` (Integer), `before` (Snowflake), `after` (Snowflake), `around` (Snowflake).
     - `PINS_QUERY_PARAMS`: `limit` (Integer).
     - `THREADS_SEARCH_QUERY_PARAMS`: `archived` (Boolean, **обязательный**), `sort_by` (String), `sort_order` (String), `limit` (Integer), `tag_setting` (String), `offset` (Integer).
     - `MESSAGES_SEARCH_QUERY_PARAMS`: `content` (String), `sort_by` (String), `sort_order` (String), `search_session_id` (Uuid), `search_query_id` (Uuid), `offset` (Integer).
   - В каталог занесён полный список из 45 подтверждённых захватом параметров `CAPTURED_QUERY_PARAMS` из `rest-d02.json` (queryParams).

2. **RFC 3986 percent-encoding и `QueryBuilder`**:
   - Реализована функция `encode_query_component(input: &str) -> String`, выполняющая percent-encoding по стандарту RFC 3986: unreserved-символы (`A-Z`, `a-z`, `0-9`, `-`, `_`, `.`, `~`) остаются без изменений, а спецсимволы (`+`, `#`, `&`, пробелы), кириллица и UTF-8 эмодзи кодируются в верхнем регистре (`%XX`).
   - Сверено с живым захватом `capture-run2.jsonl`: `#💥┃правилатест` → `%23%F0%9F%92%A5%E2%94%83%D0%BF%D1%80%D0%B0%D0%B2%D0%B8%D0%BB%D0%B0%D1%82%D0%B5%D1%81%D1%82`, `ProBot#🔴┃поговорить-тут` → `ProBot%23%F0%9F%94%B4%E2%94%83%D0%BF%D0%BE%D0%B3%D0%BE%D0%B2%D0%BE%D1%80%D0%B8%D1%82%D1%8C-%D1%82%D1%83%D1%82`.
   - Реализован `QueryBuilder` с методами `param`, `param_opt`, `param_display`, `param_display_opt`, `param_bool`, `build()`, `build_with_path(base)` и `validate_spec(spec)`.
   - Типизированы ошибки в enum `QueryError` (`MissingRequiredParam`, `InvalidParamValue`, `MutuallyExclusiveParams`, `Path(PathRenderError)`).

3. **Типизированные конструкторы URL-путей с валидацией до сети**:
   - `build_channel_pins_path(channel_id, limit)`: по умолчанию `DEFAULT_PINS_LIMIT = 25`, проверяет границы `1..=MAX_PINS_LIMIT (50)`. Сверен с эталоном `PINS.cacheEvidence` (`https://discord.com/api/v9/channels/1058348665342808104/messages/pins?limit=25`).
   - `build_thread_search_path(channel_id, params)`: требует обязательный `archived: bool`, валидирует `limit` (1..=100). Сверен с эталоном `THREAD_SEARCH.cacheEvidence` (`https://discord.com/api/v9/channels/1058348665342808104/threads/search?archived=true&sort_by=last_message_time&sort_order=desc&limit=25&tag_setting=match_some&offset=0`).
   - `build_channel_messages_path(channel_id, params)`: валидирует `limit` (1..=100), проверяет взаимную исключаемость курсоров `before`, `after`, `around`, валидирует Snowflake-курсоры (отвергает нечисловые/отрицательные/пустые). Сверен с эталоном `MESSAGES.cacheEvidence` (`https://discord.com/api/v9/channels/1056563701764857896/messages?limit=12`).
   - `build_guild_search_path(guild_id, params)` и `build_channel_search_path(channel_id, params)`: собирает параметры поиска (content, sort_by, sort_order, search_session_id, search_query_id, offset, channel_id, author_id, has, min_id, max_id) с экранированием. Сверен с живой трассой `capture-run2.jsonl`.

---

## 2. Red-прогон

Перед написанием реализации в `rust/crates/discord-core/tests/rest_routes.rs` добавлены тесты:
- `captured_query_params_catalog_matches_re_oracle_45`
- `rfc3986_percent_encoding_matches_capture_and_standards`
- `query_builder_assembly_and_spec_validation`
- `channel_pins_path_matches_oracle_and_validates`
- `thread_search_path_matches_oracle_and_validates`
- `channel_messages_path_matches_oracle_and_validates`
- `messages_search_path_matches_live_capture`

Лог падения на Red-фазе:
```text
error[E0432]: unresolved imports `discord_core::rest::routes::build_channel_messages_path`, `discord_core::rest::routes::build_channel_pins_path`, `discord_core::rest::routes::build_channel_search_path`, `discord_core::rest::routes::build_guild_search_path`, `discord_core::rest::routes::build_thread_search_path`, `discord_core::rest::routes::encode_query_component`, `discord_core::rest::routes::ChannelMessagesParams`, `discord_core::rest::routes::MessageSearchParams`, `discord_core::rest::routes::QueryBuilder`, `discord_core::rest::routes::QueryError`, `discord_core::rest::routes::QueryParamSpec`, `discord_core::rest::routes::QueryParamType`, `discord_core::rest::routes::ThreadSearchParams`, `discord_core::rest::routes::CAPTURED_QUERY_PARAMS`, `discord_core::rest::routes::DEFAULT_PINS_LIMIT`, `discord_core::rest::routes::MAX_PINS_LIMIT`
error[E0609]: no field `query_params` on type `&RouteSpec`
error: could not compile `discord-core` (test "rest_routes") due to 7 previous errors
```

---

## 3. Green-прогон

После реализации все тесты успешно пройдены:
```text
running 33 tests in src/lib.rs
test dispatch::tests::channel_create_update_delete_etf_and_json_typed_parsing ... ok
test dispatch::tests::thread_create_update_delete_etf_and_json_typed_parsing ... ok
test dispatch::tests::rejects_invalid_snowflakes_missing_fields_and_non_dispatch_envelopes ... ok
test dispatch::tests::message_delete_and_bulk_etf_and_json_preserve_64bit_snowflakes ... ok
test gateway::tests::encode_guild_subscriptions_bulk_chunked_empty_yields_empty_frames ... ok
test gateway::tests::encode_guild_subscriptions_bulk_backward_compatible_with_single_frame ... ok
test gateway::tests::guild_members_request_covers_query_and_user_ids_paths ... ok
test gateway::tests::max_guild_subscriptions_frame_bytes_constant_matches_oracle ... ok
test gateway::tests::guild_subscriptions_frame_matches_client_shape ... ok
test gateway::tests::op_codes_match_protocol_table ... ok
test gateway::tests::presence_update_frame_bytes_match_wetf_layout ... ok
test gateway::tests::presence_update_frame_passes_activity_terms_and_afk ... ok
test identify::tests::client_identity_drives_identify_properties ... ok
test gateway::tests::voice_state_update_frame_matches_client_shape ... ok
test gateway::tests::user_ids_chunk_at_one_hundred ... ok
test state::presence::tests::status_and_custom_activity_helpers ... ok
test rest::identity::tests::build_stays_a_wrapper_over_client_identity ... ok
test identify::tests::identify_wire_matches_super_properties_byte_for_byte ... ok
test rest::identity::tests::client_identity_builds_rest_and_identify_from_one_set_of_uuids ... ok
test tests::gateway_constants_match_capture ... ok
test identify::tests::rw_client_identity_matches_identify_defaults ... ok
test gateway::supervisor::tests::outbound_opcodes_3_4_8_37_reject_before_ready_and_encode_in_active_session ... ok
test ws::tests::send_presence_update_sends_binary_etf_op3_frame ... ok
test ws::tests::send_guild_subscriptions_bulk_sends_binary_etf_op37_frame ... ok
test ws::tests::send_request_guild_members_chunks_user_ids_by_100_and_supports_query ... ok
test ws::tests::send_voice_state_update_sends_binary_etf_op4_join_and_leave ... ok
test gateway::supervisor::tests::resume_ttl_preserves_session_up_to_180s_and_resets_to_identify_after_180s ... ok
test gateway::tests::encode_guild_subscriptions_bulk_chunked_splits_oversized_payloads_into_frames_le_15360 ... ok
test gateway::supervisor::tests::resets_session_after_four_failed_resumes_and_switches_to_fresh_identify ... ok
test gateway::supervisor::tests::successful_resumed_resets_failure_streak_and_backoff ... ok
test gateway::supervisor::tests::unlimited_reconnect_mode_holds_60s_ceiling_and_stops_immediately_on_4004 ... ok
test ws::tests::outbound_guards_dead_session_and_flushes_queued_frames_after_hello ... ok
test dispatch::tests::canonical_21_dispatch_kinds_preserved_and_disjoint_from_8_extended_kinds ... ok

test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.42s

     Running tests/rest_routes.rs
running 16 tests
test body_keys_only_on_six_routes ... ok
test captured_query_params_catalog_matches_re_oracle_45 ... ok
test channel_messages_path_matches_oracle_and_validates ... ok
test channel_pins_path_matches_oracle_and_validates ... ok
test guild_search_route_is_catalogued ... ok
test find_roundtrip_and_rejects ... ok
test catalog_shape_48_unique ... ok
test messages_search_path_matches_live_capture ... ok
test method_names_cover_enum ... ok
test query_builder_assembly_and_spec_validation ... ok
test render_path_substitutes_and_rejects ... ok
test rfc3986_percent_encoding_matches_capture_and_standards ... ok
test method_distribution_matches_capture ... ok
test thread_search_path_matches_oracle_and_validates ... ok
test catalog_matches_live_capture ... ok
test crossref_static_confirms_catalog ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

- Clippy (`cargo clippy --manifest-path rust/Cargo.toml -p discord-core --test rest_routes --lib -- -D warnings`): 0 warnings.
- Typecheck (`npm run typecheck`): 0 errors.
- `cargo check --manifest-path rust/gui-spike/Cargo.toml`: Finished dev profile cleanly (0 errors).

---

## 4. Файлы и контрольные суммы (MD5)

- `e577cfcf731801a758f51004a9b598c8` `rust/crates/discord-core/src/rest/routes.rs`
- `c5f6674330a64d8640e04096d408570e` `rust/crates/discord-core/tests/rest_routes.rs`

---

## 5. Риски и совместимость

- **Риски**: отсутствуют.
- **Совместимость**: 100% обратная совместимость со всеми 49 маршрутами оракула D-02 и существующими тестами.
