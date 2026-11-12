# Отчёт воркера — task-258: BFF-контракт онбординга и экрана приветствия гильдии (`onboarding`)

## 1. Batch / карточка

- **Волна**: `RW-40`, отдел `G1` (Backend).
- **Карточка**: `task-258` (`BE-P1-30` / `onboarding`) — BFF-контракт онбординга и экрана приветствия гильдии (`ONBOARDING_MODE_DEFAULT`, `ONBOARDING_MODE_ADVANCED`, `MAX_WELCOME_DESCRIPTION_CHARS`, `MAX_WELCOME_CHANNELS`, `MAX_WELCOME_CHANNEL_DESC_CHARS`, `WelcomeScreenChannelEntry`, `UpdateWelcomeScreenRequest`, `validate_welcome_screen_request`, `build_guild_welcome_screen_route`, `build_guild_onboarding_route`).
- **Ветка / worktree**: `feature/task-258-be-onboarding`, `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`.
- **Write-Scope (единственные изменённые файлы)**:
  - `rust/gui-spike/src/onboarding.rs`
  - `rust/gui-spike/src/lib.rs`
  - `rust/gui-spike/tests/onboarding_contract.rs`
  - `/home/user/orca/project/waves/RW-40/G1/task-258-be-onboarding/REPORT.md` (и `red.log`, `green.log`)

## 2. Оракул (read-only)

Сверено с `reference-oracle/rest-static.json`:

- `GUILD_ONBOARDING` (`rest-static.json:1001-1015`): `GET|PUT /guilds/{arg}/onboarding` (`https://discord.com/api/v9/guilds/{guild_id}/onboarding`).
- `GUILD_WELCOME_SCREEN` (`rest-static.json:1542-1558`): `GET|PATCH /guilds/{arg}/welcome-screen` (`https://discord.com/api/v9/guilds/{guild_id}/welcome-screen`).
- Константы Discord API v9:
  - `ONBOARDING_MODE_DEFAULT = 0`, `ONBOARDING_MODE_ADVANCED = 1`;
  - `MAX_WELCOME_DESCRIPTION_CHARS = 140` (`1..=140` символов Unicode после `trim()`);
  - `MAX_WELCOME_CHANNELS = 5`;
  - `MAX_WELCOME_CHANNEL_DESC_CHARS = 50` (`1..=50` символов Unicode после `trim()`).

## 3. Что сделано

1. **`rust/gui-spike/tests/onboarding_contract.rs` (Red-first, 5 тестов)**:
   - Проверка констант `ONBOARDING_MODE_DEFAULT = 0`, `ONBOARDING_MODE_ADVANCED = 1`, `MAX_WELCOME_DESCRIPTION_CHARS = 140`, `MAX_WELCOME_CHANNELS = 5`, `MAX_WELCOME_CHANNEL_DESC_CHARS = 50`.
   - Проверка serde round-trip для `WelcomeScreenChannelEntry` и `UpdateWelcomeScreenRequest`.
   - Проверка `validate_welcome_screen_request(req: &UpdateWelcomeScreenRequest) -> Result<(), String>`:
     - при `req.description == Some(ref d)` длина `d.trim()` в диапазоне `1..=140` символов Unicode;
     - `req.welcome_channels.len() <= 5`;
     - для каждого канала в `welcome_channels`: `channel_id` — валидный положительный числовой Snowflake (`1..=20` ASCII-цифр, без ведущего `'0'`), `description.trim()` в диапазоне `1..=50` символов Unicode, и при `emoji_id == Some(ref eid)` `eid` — валидный положительный числовой Snowflake.
   - Проверка `build_guild_welcome_screen_route(guild_id: &str) -> Result<String, String>` и `build_guild_onboarding_route(guild_id: &str) -> Result<String, String>` (валидация Snowflake и форматирование `/api/v9/guilds/{guild_id}/welcome-screen` и `/api/v9/guilds/{guild_id}/onboarding`).
2. **`rust/gui-spike/src/onboarding.rs`**:
   - Реализованы константы, структуры `WelcomeScreenChannelEntry`, `UpdateWelcomeScreenRequest` и функции `validate_welcome_screen_request`, `build_guild_welcome_screen_route`, `build_guild_onboarding_route`.
3. **`rust/gui-spike/src/lib.rs`**:
   - Добавлен экспорт `pub mod onboarding;`.

## 4. RED-прогон

Сохранено в `/home/user/orca/project/waves/RW-40/G1/task-258-be-onboarding/red.log`:

```text
error[E0432]: unresolved import `discord_gui_spike::onboarding`
  --> tests/onboarding_contract.rs:25:24
   |
25 | use discord_gui_spike::onboarding::{
   |                        ^^^^^^^^^^ could not find `onboarding` in `discord_gui_spike`

error: could not compile `discord-gui-spike` (test "onboarding_contract") due to 1 previous error
EXIT_CODE=101
```

## 5. GREEN-прогон и проверки (Targeted + Clippy + Rustfmt)

Сохранено в `/home/user/orca/project/waves/RW-40/G1/task-258-be-onboarding/green.log`:

```text
running 5 tests
test onboarding_constants_match_discord_contract ... ok
test build_guild_welcome_screen_and_onboarding_routes_validate_snowflake_and_format_paths ... ok
test validate_welcome_screen_request_enforces_description_and_channels_count_bounds ... ok
test validate_welcome_screen_request_validates_channel_entries ... ok
test welcome_screen_structs_serde_roundtrip ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Дополнительно проверено:
- `cargo clippy --manifest-path rust/gui-spike/Cargo.toml -p discord-gui-spike --test onboarding_contract --lib -- -D warnings` — `0 warnings`, `exit=0`.
- `rustfmt --edition 2021 --check rust/gui-spike/src/onboarding.rs rust/gui-spike/tests/onboarding_contract.rs` — `exit=0`.

## 6. Файлы + md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/onboarding.rs` | `41aac1af42000e1485b60d168dee0757` |
| `rust/gui-spike/src/lib.rs` | `12e7a931384fc53bfaf9b70e87f67803` |
| `rust/gui-spike/tests/onboarding_contract.rs` | `5c856c3207f76f3a3ea3e9f348919417` |
| `waves/RW-40/G1/task-258-be-onboarding/red.log` | `3a9203c0535126653271bdf41e315af5` |
| `waves/RW-40/G1/task-258-be-onboarding/green.log` | `dfbfa74c3e17154f18f0d70f7d8cbcfb` |

## 7. Риски и замечания

- **Дрейф `rust/gui-spike/Cargo.lock`**: при запуске `cargo` перегенерировался `Cargo.lock` из-за `discord-codec` (`zstd`); откатан через `git checkout -- rust/gui-spike/Cargo.lock`, чтобы строго соблюсти write-scope.
- `package-lock.json` был модифицирован в рабочем дереве до старта задачи — не трогался.
- `git commit` не выполнялся.
