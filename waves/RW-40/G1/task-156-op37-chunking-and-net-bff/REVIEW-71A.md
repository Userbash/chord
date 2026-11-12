# REVIEW-71A — task-156 (`BE-P1-01` / `F-GW-05`: OP 37 15 360 B Chunking & 30 Forwarded Dispatches / BFF Gateway Helpers)

Дата: 2026-10-30. Ревьюер: Технический ревьюер (Review 71A).
Ветка: `feature/task-156-op37-chunking-and-net-bff`, worktree `/home/user/orca/workspaces/project/feature-task-156-op37-chunking-and-net-bff`.
HEAD: `4e240481e59ff4d49732ac8bd5ba08a006363748`.
Файлы на ревью:
- `rust/crates/discord-core/src/gateway.rs`
- `rust/gui-spike/src/net.rs`
- `rust/gui-spike/tests/net_contract.rs`

Отчёт воркера: `/home/user/orca/project/waves/RW-40/G1/task-156-op37-chunking-and-net-bff/REPORT.md`.

---

## 1. Вердикт

**PASS / Ready for Merge.**
P0 = 0, P1 = 0, P2 = 0, P3 = 0.
Все требования ТЗ выполнены в полном объёме, соблюдён strict TDD, контракты с оракулом Discord 1.0.162 подтверждены тестами. Мутационное тестирование показало 100% эффективность покрытия (5/5 мутантов убиты).

---

## 2. Что сделано и проверено по коду

1. **`rust/crates/discord-core/src/gateway.rs`:**
   - Объявлена константа `pub const MAX_GUILD_SUBSCRIPTIONS_FRAME_BYTES: usize = 15_360;` (14.5 KiB по оракулу Discord 1.0.162).
   - Введены публичные алиасы типов:
     - `pub type GuildSubscriptionsBulk = BTreeMap<String, OutboundGuildSubscription>;`
     - `pub type CodecError = OutboundError;`
   - Реализована функция `encode_guild_subscriptions_bulk(&GuildSubscriptionsBulk) -> Result<Vec<u8>, OutboundError>` с полной обратной совместимостью с существующим `encode_guild_subscriptions`.
   - Реализована функция `encode_guild_subscriptions_bulk_chunked(&GuildSubscriptionsBulk) -> Result<Vec<Vec<u8>>, CodecError>`, автоматически разбивающая массив подписок гильдий на последовательность бинарных ETF-кадров `OP 37`, каждый размером $\le$ 15 360 байт (при пустом словаре возвращает пустой список кадров).
   - Модульные тесты `gateway::tests` покрывают обратную совместимость, константу оракула, пустой словарь и разбиение нагрузки >15 360 байт (120 гильдий).

2. **`rust/gui-spike/src/net.rs`:**
   - Список `FORWARDED_DISPATCHES` расширен с 22 до 30 событий. Включены 8 событий удаления и жизненного цикла каналов/веток из PR-32:
     `MESSAGE_DELETE`, `MESSAGE_DELETE_BULK`, `CHANNEL_CREATE`, `CHANNEL_UPDATE`, `CHANNEL_DELETE`, `THREAD_CREATE`, `THREAD_UPDATE`, `THREAD_DELETE`.
   - Добавлен публичный алиас роутера `map_dispatch_event`.
   - Реализована строгая проверка snowflake: `pub fn is_snowflake(id: &str) -> bool` (1..20 десятичных цифр, `u64`).
   - Реализован enum ошибок `GatewayCommandError` (`InvalidSnowflake`, `InvalidStatus`, `EmptyField`, `Encode`).
   - Реализованы BFF-команды с serde-сериализацией и валидацией:
     - `PresenceCommand` (`OP 3`): статусы `online`, `idle`, `dnd`, `invisible`, `offline`, сборка в `OutboundPresenceUpdate`.
     - `VoiceStateCommand` (`OP 4`): проверка `guild_id`/`channel_id` как snowflake, сборка в `OutboundVoiceStateUpdate`.
     - `RequestGuildMembersCommand` (`OP 8`): валидация `guild_id` и списка `user_ids`, чанкование по 100 id через `encode_request_guild_members_frames`.
     - `SubscriptionEntry` и `GuildSubscriptionsBulkCommand` (`OP 37`): валидация snowflake ключей гильдий, каналов и участников, сборка в `GuildSubscriptionsBulk`, чанкованное кодирование кадров через `encode_guild_subscriptions_bulk_chunked`.
   - Добавлен объединяющий enum `ClientGatewayCommand` и парные функции `build_*_command` / `validate_*_command`.

3. **`rust/gui-spike/tests/net_contract.rs`:**
   - Добавлен тест `forwarded_dispatches_and_map_dispatch_event_cover_eight_pr32_events` для 8 событий PR-32.
   - Добавлены контрактные тесты BFF-хелперов:
     - `client_gateway_command_helpers_build_validate_op3_presence`
     - `client_gateway_command_helpers_build_validate_op4_voice_state`
     - `client_gateway_command_helpers_build_validate_op8_request_members`
     - `client_gateway_command_helpers_build_validate_op37_guild_subscriptions`
   - Инварианты фильтрации и форвардинга обновлены под 30 форвардируемых событий ядра.

---

## 3. Контрольные суммы файлов

Проверено совпадение контрольных сумм до и после ревью:
- `e95fce54083a93535019af1dc5ad0e78  rust/crates/discord-core/src/gateway.rs`
- `72cc90b5f9bf75a890a8538bd32dc7b8  rust/gui-spike/src/net.rs`
- `055bd28ce0af37d6eedb78887eed63dc  rust/gui-spike/tests/net_contract.rs`

---

## 4. Прогоны тестов

Все команды выполнялись с `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`.

1. **`discord-core --lib gateway`:**
   ```sh
   CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml -p discord-core --lib gateway
   ```
   **Результат:** `12 passed; 0 failed; 0 ignored; finished in 0.01s` (EXIT=0).

2. **`discord-gui-spike --test net_contract`:**
   ```sh
   CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/gui-spike/Cargo.toml --test net_contract
   ```
   **Результат:** `32 passed; 0 failed; 0 ignored; finished in 0.07s` (EXIT=0).

---

## 5. Мутационное тестирование (5/5 убито)

Проведено 5 ручных мутаций кода в `gateway.rs` и `net.rs`. Все мутанты успешно пойманы соответствующими тестами.

| # | Мутация | Файл / Строка | Результат | Тест, поймавший мутацию |
|---|---|---|---|---|
| M1 | Изменение константы `MAX_GUILD_SUBSCRIPTIONS_FRAME_BYTES` с `15_360` на `16_000` | `gateway.rs:106` | **KILLED** (`assertion left == right failed: 16000 == 15360`) | `gateway::tests::max_guild_subscriptions_frame_bytes_constant_matches_oracle` |
| M2 | Изменение опкода `GUILD_SUBSCRIPTIONS_BULK_OP` с `37` на `36` | `gateway.rs:103` | **KILLED** (`assertion left == right failed: Some(36) == Some(37)`) | `gateway::tests::guild_subscriptions_frame_matches_client_shape`, `gateway::tests::encode_guild_subscriptions_bulk_chunked_splits_oversized_payloads_into_frames_le_15360` |
| M3 | Удаление события `"MESSAGE_DELETE"` из `FORWARDED_DISPATCHES` (с корректировкой размера массива до 29) | `net.rs:117,126` | **KILLED** (`расширенный тип PR-32 MESSAGE_DELETE не форвардится`) | `forwarded_dispatches_cover_every_core_kind_and_the_raw_reaction_remove`, `forwarded_dispatches_and_map_dispatch_event_cover_eight_pr32_events` |
| M4 | Инверсия условия в `is_snowflake`: `!id.is_empty()` -> `id.is_empty()` | `net.rs:1081` | **KILLED** (`valid voice state / request members / bulk subscriptions: InvalidSnowflake`) | `client_gateway_command_helpers_build_validate_op4_voice_state`, `client_gateway_command_helpers_build_validate_op8_request_members`, `client_gateway_command_helpers_build_validate_op37_guild_subscriptions` |
| M5 | Пропуск валидации статуса в `PresenceCommand::validate`: возврат `Ok(())` для любых строк | `net.rs:1136-1143` | **KILLED** (`invalid status must fail`) | `client_gateway_command_helpers_build_validate_op3_presence` |

После завершения мутационного тестирования файлы возвращены в исходное состояние, контрольные суммы проверены (`md5sum`), контрольные прогоны тестов подтвердили чистоту репозитория.

---

## 6. Замечания (Findings)

Замечаний уровней P0, P1, P2 нет. Код готов к вливанию.
