# Отчёт воркера — task-196: расширенный IPC-диспетчер (`ipc_ext`)

## 1. Batch / карточка

- **Волна**: `RW-40`, отдел `G1` (Backend).
- **Карточка**: `task-196` — расширенный IPC-диспетчер (`ExtendedIpcCommand`, `ExtendedIpcPlan`, `validate_and_build_plan`).
- **Ветка / worktree**: `feature/task-196-be-ipc-ext-dispatcher`, `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target`.
- **Write-Scope (единственные изменённые файлы)**:
  - `rust/gui-spike/src/ipc_ext.rs`
  - `rust/gui-spike/src/lib.rs`
  - `rust/gui-spike/tests/ipc_ext_contract.rs`
  - `/home/sanya/orca/project/waves/RW-40/G1/task-196-be-ipc-ext-dispatcher/REPORT.md` (и `red.log`, `green.log`)

## 2. Оракул (read-only)

Сверено с `/home/sanya/re-deep/rest-static.json`, `/home/sanya/re-deep/rest-d02.json` и модулями `profile.rs`, `messages.rs`, `reactions.rs`, `search.rs`:

- `USER_PROFILE`: `GET /users/{id}/profile?with_mutual_guilds={bool}&with_mutual_friends_count={bool}[&guild_id={id}]`, тело `None`.
- `BULK_ACK`: `POST /read-states/ack-bulk`, тело `{"read_states": [...]}` (`1..=100` элементов `BulkReadStateEntry` = `crate::messages::AckBulkEntry`).
- `REACTIONS` (`endpoints[38]`): `PUT /channels/{channel_id}/messages/{message_id}/reactions/{emoji}/@me`, тело `None` (0 байт).
- `POLL_ANSWERS`: `PUT /channels/{channel_id}/polls/{message_id}/answers/@me`, тело `{"answer_ids": [...]}` (`1..=10` ответов).
- `GUILD_SEARCH`: `GET /guilds/{guild_id}/messages/search?[content={content}&]offset={offset}&limit={limit}`, `offset <= 9975`, `limit` в `1..=25`, тело `None`.

## 3. Что сделано

1. **`rust/gui-spike/tests/ipc_ext_contract.rs` (Red-first, 12 тестов)**:
   - Проверка serde-контракта `ExtendedIpcCommand` (`#[serde(tag = "cmd", content = "args", rename_all = "snake_case")]`) для всех 5 вариантов (`get_user_profile`, `ack_messages_bulk`, `add_reaction_empty_body`, `vote_poll`, `search_guild_messages`) и отказа на неизвестном теге `cmd`.
   - Позитивные, граничные (`"1"`, `u64::MAX`, `read_states` = 1 и 100, `answer_ids` = 1 и 10, `offset` = 0 и 9975, `limit` = 1 и 25) и негативные (`""`, `"0"`, `"0000"`, не-цифры, `u64::MAX + 1`, `read_states` = 0 и 101, `answer_ids` = 0 и 11, `offset` = 9976, `limit` = 0 и 26, пустой `emoji`) сценарии `validate_and_build_plan`.
2. **`rust/gui-spike/src/ipc_ext.rs`**:
   - Тип-алиас `pub type BulkReadStateEntry = crate::messages::AckBulkEntry;` и константы лимитов (`MAX_BULK_READ_STATES = 100`, `MAX_POLL_ANSWERS = 10`, `MAX_SEARCH_OFFSET = 9975`, `MAX_SEARCH_LIMIT = 25`).
   - Перечисление `ExtendedIpcCommand` с `#[serde(tag = "cmd", content = "args", rename_all = "snake_case")]`.
   - Структура `ExtendedIpcPlan { pub method: &'static str, pub path: String, pub body: Option<serde_json::Value> }`.
   - Функция `validate_and_build_plan(cmd: &ExtendedIpcCommand) -> Result<ExtendedIpcPlan, String>` с валидацией Snowflake ID (непустые ASCII-цифры, парсятся в `u64 > 0`) и границ всех параметров.
3. **`rust/gui-spike/src/lib.rs`**:
   - Экспортирован модуль `pub mod ipc_ext;`.

## 4. RED-прогон

Сохранено в `/home/sanya/orca/project/waves/RW-40/G1/task-196-be-ipc-ext-dispatcher/red.log`:

```text
error[E0432]: unresolved import `discord_gui_spike::ipc_ext`
  --> tests/ipc_ext_contract.rs:15:24
   |
15 | use discord_gui_spike::ipc_ext::{
   |                        ^^^^^^^ could not find `ipc_ext` in `discord_gui_spike`

error: could not compile `discord-gui-spike` (test "ipc_ext_contract") due to 1 previous error
EXIT_CODE=101
```

## 5. GREEN-прогон и проверки (Targeted + Clippy + Rustfmt)

Сохранено в `/home/sanya/orca/project/waves/RW-40/G1/task-196-be-ipc-ext-dispatcher/green.log`:

```text
running 12 tests
test ack_messages_bulk_builds_post_read_states_ack_bulk_plan ... ok
test ack_messages_bulk_enforces_1_to_100_bounds_and_snowflake_ids ... ok
test add_reaction_empty_body_builds_put_plan_with_none_body ... ok
test add_reaction_empty_body_rejects_invalid_snowflakes_and_empty_emoji ... ok
test get_user_profile_accepts_min_positive_and_u64_max_snowflakes_and_rejects_invalid ... ok
test get_user_profile_builds_exact_route_with_and_without_guild_id ... ok
test search_guild_messages_builds_get_plan_with_and_without_content ... ok
test search_guild_messages_enforces_offset_limit_and_guild_id_bounds ... ok
test serde_rejects_unknown_command_tag ... ok
test serde_roundtrip_all_five_commands_matches_tagged_snake_case_wire_format ... ok
test vote_poll_builds_put_answers_me_with_answer_ids_body ... ok
test vote_poll_enforces_answer_ids_1_to_10_and_snowflake_ids ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT_CODE=0
```

Дополнительно проверено:
- `cargo clippy -p discord-gui-spike --test ipc_ext_contract --lib -- -D warnings` — `0 warnings`, `exit=0`.
- `rustfmt --edition 2021 --check rust/gui-spike/src/ipc_ext.rs rust/gui-spike/tests/ipc_ext_contract.rs` — `exit=0`.
- `rustfmt --edition 2021 --check --config skip_children=true rust/gui-spike/src/lib.rs` — `exit=0`.

## 6. Файлы + md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/ipc_ext.rs` | `9938b029541a73a27882c5859226021b` |
| `rust/gui-spike/src/lib.rs` | `0f3d34caeb0f8e868321f097978197a6` |
| `rust/gui-spike/tests/ipc_ext_contract.rs` | `1eff591f30207d4d6259a40487400c32` |
| `waves/RW-40/G1/task-196-be-ipc-ext-dispatcher/red.log` | `568d0a7400b7aa51d3a427c515768a1b` |
| `waves/RW-40/G1/task-196-be-ipc-ext-dispatcher/green.log` | `faf377924a120f9aa5eba095bd8dba59` |

## 7. Риски и замечания

- **Алиас `BulkReadStateEntry`**: в `crate::messages` структура названа `AckBulkEntry`; по согласованию с координатором в `ipc_ext.rs` экспортирован `pub type BulkReadStateEntry = crate::messages::AckBulkEntry;`.
- **Дрейф `rust/gui-spike/Cargo.lock`**: при запуске `cargo` перегенерируется `Cargo.lock` из-за `discord-codec` (`zstd`); откатан через `git checkout -- rust/gui-spike/Cargo.lock`, чтобы не выходить за write-scope.
- `package-lock.json` был модифицирован в рабочем дереве до старта задачи — не трогался.
- `git commit` не выполнялся.
