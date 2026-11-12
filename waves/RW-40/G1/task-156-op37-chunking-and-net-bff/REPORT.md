# Отчёт воркера — task-156 (OP 37 Chunking & Net BFF)

## 1. Batch / карточка

- **Волна:** `RW-40`, отдел `G1` (Rust Core & Tauri BFF).
- **Карточка:** `task-156` (находка `F-GW-05` / `BE-P1-01` из `waves/RW-40/PM/BACKLOG-RE-BACKEND.md` и `waves/RW-40/RE-AUDIT/RE-GAP-BACKEND-PROTOCOL.md`):
  1. Чанкование `OP 37 GUILD_SUBSCRIPTIONS_BULK` по аппаратному лимиту ETF-кадра 15 360 байт (14.5 KiB по оракулу Discord 1.0.162).
  2. Проброс 8 расширенных событий удаления сообщений и жизненного цикла каналов/веток из PR-32 в `FORWARDED_DISPATCHES` / `map_dispatch_event`.
  3. BFF-хелперы построения и валидации клиентских команд шлюза (`OP 3` presence, `OP 4` voice state, `OP 8` request guild members, `OP 37` guild subscriptions bulk).
- **Файлы воркера (write-scope):**
  - `rust/crates/discord-core/src/gateway.rs`
  - `rust/gui-spike/src/net.rs`
  - `rust/gui-spike/tests/net_contract.rs`
- **Предварительная синхронизация upstream-файлов:**
  - `rust/crates/discord-core/src/gateway.rs`
  - `rust/crates/discord-core/src/ws.rs`
  - `rust/crates/discord-core/src/dispatch.rs`
  - `rust/gui-spike/src/net.rs`
  - `rust/gui-spike/tests/net_contract.rs`

## 2. Что сделано

1. **`rust/crates/discord-core/src/gateway.rs`:**
   - Добавлена константа `pub const MAX_GUILD_SUBSCRIPTIONS_FRAME_BYTES: usize = 15_360;` (14.5 KiB по клиенту Discord 1.0.162).
   - Объявлены публичные алиасы типов `pub type GuildSubscriptionsBulk = BTreeMap<String, OutboundGuildSubscription>;` и `pub type CodecError = OutboundError;`.
   - Реализована функция `encode_guild_subscriptions_bulk(&GuildSubscriptionsBulk) -> Result<Vec<u8>, OutboundError>` с полной обратной совместимостью с `encode_guild_subscriptions`.
   - Реализована функция `encode_guild_subscriptions_bulk_chunked(&GuildSubscriptionsBulk) -> Result<Vec<Vec<u8>>, CodecError>`, автоматически разбивающая большие словари подписок гильдий на последовательность ETF-кадров `OP 37`, каждый размером $\le$ 15 360 байт.
   - Добавлены 4 unit-теста (`max_guild_subscriptions_frame_bytes_constant_matches_oracle`, `encode_guild_subscriptions_bulk_backward_compatible_with_single_frame`, `encode_guild_subscriptions_bulk_chunked_empty_yields_empty_frames`, `encode_guild_subscriptions_bulk_chunked_splits_oversized_payloads_into_frames_le_15360`).

2. **`rust/gui-spike/src/net.rs`:**
   - Расширен список `FORWARDED_DISPATCHES` с 22 до 30 событий с включением 8 событий жизненного цикла и удалений из PR-32: `MESSAGE_DELETE`, `MESSAGE_DELETE_BULK`, `CHANNEL_CREATE`, `CHANNEL_UPDATE`, `CHANNEL_DELETE`, `THREAD_CREATE`, `THREAD_UPDATE`, `THREAD_DELETE`.
   - Добавлен алиас `pub fn map_dispatch_event(event: &DispatchEvent) -> Result<Option<MappedDispatch>, DispatchError>`.
   - Добавлена функция проверки снежинок `pub fn is_snowflake(id: &str) -> bool`.
   - Добавлен тип ошибки валидации/кодирования `GatewayCommandError` (`InvalidSnowflake`, `InvalidStatus`, `EmptyField`, `Encode`).
   - Реализованы типизированные структуры BFF-команд:
     - `PresenceCommand` (`OP 3`): валидация статусов `online`, `idle`, `dnd`, `invisible`, `offline`, сборка `OutboundPresenceUpdate`, кодирование в wetf-кадр `OP 3`.
     - `VoiceStateCommand` (`OP 4`): валидация `guild_id`/`channel_id` как snowflake, сборка `OutboundVoiceStateUpdate`, кодирование в кадр `OP 4`.
     - `RequestGuildMembersCommand` (`OP 8`): валидация `guild_id` и списка `user_ids`, сборка `OutboundGuildMembersRequest`, чанкованное кодирование кадров по 100 id.
     - `SubscriptionEntry` и `GuildSubscriptionsBulkCommand` (`OP 37`): валидация снежинок гильдий, каналов и участников, сборка `GuildSubscriptionsBulk`, кодирование с чанкованием по лимиту 15 360 байт через `encode_guild_subscriptions_bulk_chunked`.
   - Добавлен объединяющий enum `ClientGatewayCommand` (`PresenceUpdate`, `VoiceStateUpdate`, `RequestGuildMembers`, `GuildSubscriptionsBulk`) с serde-тегами (`op` / `d`).
   - Добавлены функции-хелперы `build_*_command` и `validate_*_command`.

3. **`rust/gui-spike/tests/net_contract.rs`:**
   - Добавлен тест `forwarded_dispatches_and_map_dispatch_event_cover_eight_pr32_events`, проверяющий присутствие всех 8 событий в `FORWARDED_DISPATCHES` и их проброс через `map_dispatch_event`.
   - Добавлены тесты `client_gateway_command_helpers_build_validate_op3_presence`, `client_gateway_command_helpers_build_validate_op4_voice_state`, `client_gateway_command_helpers_build_validate_op8_request_members`, `client_gateway_command_helpers_build_validate_op37_guild_subscriptions`.
   - Обновлены существующие проверки инвариантов проброса (`forwarded_dispatches_cover_every_core_kind_and_the_raw_reaction_remove`, `wire_names_outside_the_core_router_are_filtered`, `non_forwarded_kind_is_counted_as_filtered`) с учётом перехода 8 событий PR-32 в категорию форвардируемых.

## 3. RED-прогон

До реализации продуктового кода зафиксированы падения:

1. `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml -p discord-core --lib gateway`:
   ```
   error[E0425]: cannot find value `MAX_GUILD_SUBSCRIPTIONS_FRAME_BYTES` in module `super`
   error[E0425]: cannot find type `GuildSubscriptionsBulk` in module `super`
   error[E0425]: cannot find function `encode_guild_subscriptions_bulk` in module `super`
   error[E0425]: cannot find function `encode_guild_subscriptions_bulk_chunked` in module `super`
   error: could not compile `discord-core` (lib test) due to 11 previous errors
   ```

2. `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/gui-spike/Cargo.toml --test net_contract`:
   ```
   error[E0432]: unresolved imports `discord_gui_spike::net::map_dispatch_event`, `discord_gui_spike::net::GatewayCommandError`, `discord_gui_spike::net::GuildSubscriptionsBulkCommand`, `discord_gui_spike::net::PresenceCommand`, `discord_gui_spike::net::RequestGuildMembersCommand`, `discord_gui_spike::net::VoiceStateCommand`
   error[E0422]: cannot find struct, variant or union type `SubscriptionEntry` in module `discord_gui_spike::net`
   error: could not compile `discord-gui-spike` (test "net_contract") due to 3 previous errors
   ```

## 4. GREEN-прогон и проверки

1. **Unit-тесты `discord-core::gateway`:**
   ```sh
   CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml -p discord-core --lib gateway
   ```
   Вывод:
   ```
   running 12 tests
   test gateway::tests::encode_guild_subscriptions_bulk_chunked_empty_yields_empty_frames ... ok
   test gateway::tests::encode_guild_subscriptions_bulk_backward_compatible_with_single_frame ... ok
   test gateway::tests::op_codes_match_protocol_table ... ok
   test gateway::tests::max_guild_subscriptions_frame_bytes_constant_matches_oracle ... ok
   test gateway::tests::guild_subscriptions_frame_matches_client_shape ... ok
   test gateway::tests::guild_members_request_covers_query_and_user_ids_paths ... ok
   test gateway::tests::presence_update_frame_passes_activity_terms_and_afk ... ok
   test gateway::tests::presence_update_frame_bytes_match_wetf_layout ... ok
   test gateway::tests::voice_state_update_frame_matches_client_shape ... ok
   test gateway::tests::user_ids_chunk_at_one_hundred ... ok
   test tests::gateway_constants_match_capture ... ok
   test gateway::tests::encode_guild_subscriptions_bulk_chunked_splits_oversized_payloads_into_frames_le_15360 ... ok

   test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out; finished in 0.01s
   ```

2. **Контрактные тесты `discord-gui-spike::net_contract`:**
   ```sh
   CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/gui-spike/Cargo.toml --test net_contract
   ```
   Вывод:
   ```
   running 32 tests
   test client_gateway_command_helpers_build_validate_op37_guild_subscriptions ... ok
   test client_gateway_command_helpers_build_validate_op3_presence ... ok
   test broken_payload_is_counted_instead_of_panicking ... ok
   test close_policies_are_forwarded ... ok
   test connect_reads_token_and_reaches_ready ... ok
   test client_gateway_command_helpers_build_validate_op4_voice_state ... ok
   test disconnect_is_idempotent_and_reconnect_is_visible ... ok
   test dispatch_stats_report_core_source_available ... ok
   test core_supervisor_config_wires_lifecycle_and_dispatch ... ok
   test all_core_kinds_reach_the_ui_channel_in_order ... ok
   test etf_to_json_keeps_atoms_as_strings_and_handles_nil_and_numbers ... ok
   test exact_token_and_nested_auth_tokens_are_redacted_and_counted ... ok
   test dispatch_reaches_ui_channel_in_order_without_changing_status ... ok
   test filtered_and_unknown_kinds_do_not_panic_and_are_not_forwarded ... ok
   test duplicate_sequences_are_dropped_and_reconnect_does_not_replay ... ok
   test forwarded_dispatches_and_map_dispatch_event_cover_eight_pr32_events ... ok
   test forwarded_dispatches_cover_every_core_kind_and_the_raw_reaction_remove ... ok
   test missing_token_is_a_typed_error ... ok
   test every_core_dispatch_kind_reaches_the_kind_data_envelope ... ok
   test non_forwarded_kind_is_counted_as_filtered ... ok
   test production_driver_disconnect_is_idempotent_without_connecting ... ok
   test client_gateway_command_helpers_build_validate_op8_request_members ... ok
   test reaction_kinds_use_wire_name_and_forward_data_as_is ... ok
   test status_never_contains_token ... ok
   test supervisor_events_map_to_gateway_contract ... ok
   test presence_dispatch_is_forwarded_verbatim_in_kind_data_envelope ... ok
   test members_chunk_is_forwarded_verbatim ... ok
   test wire_names_outside_the_core_router_are_filtered ... ok
   test blocked_ui_channel_never_blocks_the_gateway_source ... ok
   test emit_error_is_counted_and_not_swallowed ... ok
   test members_list_update_keeps_counts_and_drops_nested_presence_session_ids ... ok
   test ready_payload_drops_session_secrets_and_keeps_data ... ok

   test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
   ```

3. **Clippy:**
   ```sh
   CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo clippy --manifest-path rust/gui-spike/Cargo.toml --test net_contract -- -D warnings
   CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo clippy --manifest-path rust/Cargo.toml -p discord-core -- -D warnings
   ```
   Оба вызова завершились с кодом 0 без единого предупреждения.

## 5. Файлы и контрольные суммы (MD5)

- `e95fce54083a93535019af1dc5ad0e78`  `rust/crates/discord-core/src/gateway.rs`
- `72cc90b5f9bf75a890a8538bd32dc7b8`  `rust/gui-spike/src/net.rs`
- `055bd28ce0af37d6eedb78887eed63dc`  `rust/gui-spike/tests/net_contract.rs`

## 6. Риски

- Изменения полностью изолированы в назначенных файлах `gateway.rs`, `net.rs`, `net_contract.rs`.
- Обратная совместимость одиночного кодирования `encode_guild_subscriptions` и `encode_guild_subscriptions_bulk` гарантирована.
- Чанкование `OP 37` строго держит размер каждого ETF-кадра $\le$ 15 360 байт (14.5 KiB), предотвращая разрыв соединения сервером Discord при большом числе подписок.
