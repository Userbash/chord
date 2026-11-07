# Отчёт воркера — task-249: BFF-контракт правил автомодерации гильдии (`automod`)

## 1. Batch / карточка

- **Волна**: `RW-40`, отдел `G1` (Backend).
- **Карточка**: `task-249` (`BE-P1-28` / `automod`) — BFF-контракт правил автомодерации гильдии (`AUTOMOD_EVENT_MESSAGE_SEND`, `AUTOMOD_TRIGGER_*`, `AUTOMOD_ACTION_*`, `MAX_AUTOMOD_NAME_CHARS`, `MAX_KEYWORD_FILTER_ITEMS`, `MAX_MENTION_TOTAL_LIMIT`, `MAX_TIMEOUT_DURATION_SECS`, `UpsertAutoModRuleRequest`, `validate_automod_rule_request`, `build_guild_automod_route`).
- **Ветка / worktree**: `feature/task-249-be-automod`, `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target`.
- **Write-Scope (единственные изменённые файлы)**:
  - `rust/gui-spike/src/automod.rs`
  - `rust/gui-spike/src/lib.rs`
  - `rust/gui-spike/tests/automod_contract.rs`
  - `/home/sanya/orca/project/waves/RW-40/G1/task-249-be-automod/REPORT.md` (и `red.log`, `green.log`)

## 2. Оракул (read-only)

Сверено с `/home/sanya/re-deep/rest-static.json`:

- `GUILD_AUTOMOD_RULES` (`rest-static.json:8353-8366`): `GET|POST /guilds/{arg}/auto-moderation/rules`.
- `GUILD_AUTOMOD_RULE` (`rest-static.json:8367-8380`): `DELETE|PATCH /guilds/{arg}/auto-moderation/rules/{arg}`.
- Константы Discord API v9:
  - `AUTOMOD_EVENT_MESSAGE_SEND = 1`;
  - `AUTOMOD_TRIGGER_KEYWORD = 1`, `AUTOMOD_TRIGGER_SPAM = 3`, `AUTOMOD_TRIGGER_KEYWORD_PRESET = 4`, `AUTOMOD_TRIGGER_MENTION_SPAM = 5`;
  - `AUTOMOD_ACTION_BLOCK_MESSAGE = 1`, `AUTOMOD_ACTION_SEND_ALERT = 2`, `AUTOMOD_ACTION_TIMEOUT = 3`;
  - `MAX_AUTOMOD_NAME_CHARS = 100`, `MAX_KEYWORD_FILTER_ITEMS = 1000` (каждый элемент `1..=60` символов Unicode после `trim()`), `MAX_MENTION_TOTAL_LIMIT = 50`, `MAX_TIMEOUT_DURATION_SECS = 2_419_200` (28 суток).

## 3. Что сделано

1. **`rust/gui-spike/tests/automod_contract.rs` (Red-first, 7 тестов)**:
   - Проверка констант `AUTOMOD_EVENT_MESSAGE_SEND = 1`, `AUTOMOD_TRIGGER_KEYWORD = 1`, `AUTOMOD_TRIGGER_SPAM = 3`, `AUTOMOD_TRIGGER_KEYWORD_PRESET = 4`, `AUTOMOD_TRIGGER_MENTION_SPAM = 5`, `AUTOMOD_ACTION_BLOCK_MESSAGE = 1`, `AUTOMOD_ACTION_SEND_ALERT = 2`, `AUTOMOD_ACTION_TIMEOUT = 3`, `MAX_AUTOMOD_NAME_CHARS = 100`, `MAX_KEYWORD_FILTER_ITEMS = 1000`, `MAX_MENTION_TOTAL_LIMIT = 50`, `MAX_TIMEOUT_DURATION_SECS = 2_419_200`.
   - Проверка serde round-trip для `UpsertAutoModRuleRequest`.
   - Проверка `validate_automod_rule_request(req: &UpsertAutoModRuleRequest) -> Result<(), String>`:
     - длина `req.name.trim()` в диапазоне `1..=100` символов Unicode;
     - `req.event_type` равен `1` (`MESSAGE_SEND`);
     - при `trigger_type == 1` (`KEYWORD`): `keyword_filter` содержит `1..=1000` элементов, каждый после `trim()` длиной `1..=60` символов Unicode;
     - при `trigger_type == 5` (`MENTION_SPAM`): `mention_total_limit` равен `Some(1..=50)`;
     - `trigger_type == 3` (`SPAM`) и `4` (`KEYWORD_PRESET`) допустимы, прочие возвращают `Err`;
     - `action_type` равен `1`, `2` или `3`; при `action_type == 3` (`TIMEOUT`) `timeout_duration_secs` равен `Some(1..=2_419_200)`.
   - Проверка `build_guild_automod_route(guild_id: &str, rule_id: Option<&str>) -> Result<String, String>`:
     - валидация `guild_id` (и `rule_id`, если `Some`) как положительных числовых Snowflake (`1..=20` ASCII-цифр, без ведущего `'0'`);
     - возврат `"/api/v9/guilds/{guild_id}/auto-moderation/rules"` или `"/api/v9/guilds/{guild_id}/auto-moderation/rules/{rule_id}"`.
2. **`rust/gui-spike/src/automod.rs`**:
   - Реализованы все константы, структура `UpsertAutoModRuleRequest` и функции `validate_automod_rule_request`, `build_guild_automod_route`.
3. **`rust/gui-spike/src/lib.rs`**:
   - Добавлен экспорт `pub mod automod;`.

## 4. RED-прогон

Сохранено в `/home/sanya/orca/project/waves/RW-40/G1/task-249-be-automod/red.log`:

```text
error[E0432]: unresolved import `discord_gui_spike::automod`
  --> tests/automod_contract.rs:29:24
   |
29 | use discord_gui_spike::automod::{
   |                        ^^^^^^^ could not find `automod` in `discord_gui_spike`

error: could not compile `discord-gui-spike` (test "automod_contract") due to 1 previous error
EXIT_CODE=101
```

## 5. GREEN-прогон и проверки (Targeted + Clippy + Rustfmt)

Сохранено в `/home/sanya/orca/project/waves/RW-40/G1/task-249-be-automod/green.log`:

```text
running 7 tests
test automod_constants_match_discord_contract ... ok
test build_guild_automod_route_formats_collection_and_item_routes_and_validates_snowflakes ... ok
test validate_automod_rule_request_enforces_name_bounds_and_event_type ... ok
test validate_automod_rule_request_validates_action_types_and_timeout_duration ... ok
test validate_automod_rule_request_validates_mention_spam_spam_preset_and_unknown_triggers ... ok
test upsert_automod_rule_request_serde_roundtrip ... ok
test validate_automod_rule_request_validates_keyword_trigger_bounds ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT_CODE=0
```

Дополнительно проверено:
- `cargo clippy --manifest-path rust/gui-spike/Cargo.toml -p discord-gui-spike --test automod_contract --lib -- -D warnings` — `0 warnings`, `exit=0`.
- `rustfmt --edition 2021 --check rust/gui-spike/src/automod.rs rust/gui-spike/tests/automod_contract.rs` — `exit=0`.

## 6. Файлы + md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/automod.rs` | `f789253558955d8a123fd2b6adf99d48` |
| `rust/gui-spike/src/lib.rs` | `18526d4c514924c86d6b328c5390e112` |
| `rust/gui-spike/tests/automod_contract.rs` | `d439a60fb588560edc577315a2b45d98` |
| `waves/RW-40/G1/task-249-be-automod/red.log` | `1d6534a926035e52555d5c3ec2f3625d` |
| `waves/RW-40/G1/task-249-be-automod/green.log` | `3e801272550eca55c9a10519e7463f22` |

## 7. Риски и замечания

- **Дрейф `rust/gui-spike/Cargo.lock`**: при запуске `cargo` перегенерировался `Cargo.lock` из-за `discord-codec` (`zstd`); откатан через `git checkout -- rust/gui-spike/Cargo.lock`, чтобы строго соблюсти write-scope.
- `package-lock.json` был модифицирован в рабочем дереве до старта задачи — не трогался.
- `git commit` не выполнялся.
