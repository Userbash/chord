# Отчёт воркера: task-253 (`BE-P1-29` — Rust Application Slash Commands & Interaction BFF Contract)

- **Batch / Карточка:** `RW-40` / `G1` / `task-253` (`BE-P1-29`)
- **Ветка:** `feature/task-253-be-slash-commands`

## 1. Что сделано
1. **Strict TDD (Red):** написан контрактный тест `rust/gui-spike/tests/slash_commands_contract.rs` (8 тестовых сценариев: константы типов взаимодействий и команд, лимиты длины имени и количества опций, serde round-trip `InvokeSlashCommandRequest`, валидация `validate_slash_command_name` на диапазон `1..=32` символов `[-_a-z0-9]`, валидация `validate_invoke_slash_command` для положительных числовых Snowflake `1..=20` цифр без ведущих нулей, `session_id.trim()`, `options.len() <= 25`, а также построение канонического маршрута `/api/v9/interactions` и JSON-тела `build_interactions_route_and_body`). Зафиксировано падение компиляции (`E0432: unresolved import discord_gui_spike::slash_commands`) в `red.log`.
2. **Strict TDD (Green):** реализован модуль `rust/gui-spike/src/slash_commands.rs` и экспортирован `pub mod slash_commands;` в `rust/gui-spike/src/lib.rs`:
   - Константы: `INTERACTION_TYPE_APPLICATION_COMMAND = 2`, `INTERACTION_TYPE_MESSAGE_COMPONENT = 3`, `INTERACTION_TYPE_AUTOCOMPLETE = 4`, `INTERACTION_TYPE_MODAL_SUBMIT = 5`, `COMMAND_TYPE_CHAT_INPUT = 1`, `COMMAND_TYPE_USER = 2`, `COMMAND_TYPE_MESSAGE = 3`, `MAX_COMMAND_NAME_CHARS = 32`, `MAX_COMMAND_OPTIONS = 25`.
   - Структура `InvokeSlashCommandRequest { pub application_id: String, pub guild_id: Option<String>, pub channel_id: String, pub session_id: String, pub command_id: String, pub command_version: String, pub command_name: String, pub options: Vec<serde_json::Value>, pub nonce: String }`.
   - Функции `validate_slash_command_name`, `validate_invoke_slash_command`, `build_interactions_route_and_body` в соответствии с RE-оракулом `reference-oracle/rest-static.json:7720-7731` (`Endpoints.INTERACTIONS`).

## 2. Red-прогон
Команда:
```sh
CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/gui-spike/Cargo.toml --test slash_commands_contract > /home/user/orca/project/waves/RW-40/G1/task-253-be-slash-commands/red.log 2>&1
```
Результат: ожидаемое падение компиляции (`error[E0432]: unresolved import discord_gui_spike::slash_commands`). Лог сохранён в `/home/user/orca/project/waves/RW-40/G1/task-253-be-slash-commands/red.log`.

## 3. Green-прогон (Targeted T0)
Команда:
```sh
CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/gui-spike/Cargo.toml --test slash_commands_contract > /home/user/orca/project/waves/RW-40/G1/task-253-be-slash-commands/green.log 2>&1
```
Результат: `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`. Лог сохранён в `/home/user/orca/project/waves/RW-40/G1/task-253-be-slash-commands/green.log`.

## 4. Изменённые и созданные файлы (`md5sum`)
| Файл | MD5 |
|---|---|
| `rust/gui-spike/src/slash_commands.rs` | `3e03aa0be158350afb64ce317a17f15d` |
| `rust/gui-spike/src/lib.rs` | `a68d036467f31d38022e323346a835f6` |
| `rust/gui-spike/tests/slash_commands_contract.rs` | `c081c7664dce6ebf97f778e9da6efcfc` |
| `/home/user/orca/project/waves/RW-40/G1/task-253-be-slash-commands/red.log` | `30234d2ef02d38d235a9f93c4cb03b86` |
| `/home/user/orca/project/waves/RW-40/G1/task-253-be-slash-commands/green.log` | `caa79083f5888b50f67e88951ba7a227` |

## 5. Риски и замечания
- Затронуты исключительно разрешённые в ТЗ файлы.
- `git commit` не выполнялся в соответствии с правилами воркера.
