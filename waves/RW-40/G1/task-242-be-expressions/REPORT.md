# Отчёт воркера — task-242: BFF-контракт кастомных эмодзи и стикеров гильдии (`expressions`)

## 1. Batch / карточка

- **Волна**: `RW-40`, отдел `G1` (Backend).
- **Карточка**: `task-242` (`BE-P1-26` / `expressions`) — BFF-контракт кастомных эмодзи и стикеров гильдии (`MIN_EMOJI_NAME_CHARS`, `MAX_EMOJI_NAME_CHARS`, `MAX_EMOJI_BYTES`, `MIN_STICKER_NAME_CHARS`, `MAX_STICKER_NAME_CHARS`, `MAX_STICKER_DESC_CHARS`, `STICKER_FORMAT_*`, `CreateGuildEmojiRequest`, `validate_emoji_name`, `validate_create_guild_emoji`, `build_guild_emojis_route`).
- **Ветка / worktree**: `feature/task-242-be-expressions`, `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`.
- **Write-Scope (единственные изменённые файлы)**:
  - `rust/gui-spike/src/expressions.rs`
  - `rust/gui-spike/src/lib.rs`
  - `rust/gui-spike/tests/expressions_contract.rs`
  - `/home/user/orca/project/waves/RW-40/G1/task-242-be-expressions/REPORT.md` (и `red.log`, `green.log`)

## 2. Оракул (read-only)

Сверено с `reference-oracle/rest-static.json`:

- `GUILD_EMOJIS` (`rest-static.json:1197-1208`): `GET|POST /guilds/{arg}/emojis`, `bodyKeys: ["{image,name,roles}"]`.
- `GUILD_EMOJI` (`rest-static.json:1225-1236`): `DELETE|PATCH /guilds/{arg}/emojis/{arg}`, `bodyKeys: ["{name,roles}"]`.
- Константы Discord API v9:
  - `MIN_EMOJI_NAME_CHARS = 2`, `MAX_EMOJI_NAME_CHARS = 32`, `MAX_EMOJI_BYTES = 256 * 1024` (`262_144` байт);
  - `MIN_STICKER_NAME_CHARS = 2`, `MAX_STICKER_NAME_CHARS = 30`, `MAX_STICKER_DESC_CHARS = 100`;
  - `STICKER_FORMAT_PNG = 1`, `STICKER_FORMAT_APNG = 2`, `STICKER_FORMAT_LOTTIE = 3`, `STICKER_FORMAT_GIF = 4`.

## 3. Что сделано

1. **`rust/gui-spike/tests/expressions_contract.rs` (Red-first, 7 тестов)**:
   - Проверка констант `MIN_EMOJI_NAME_CHARS = 2`, `MAX_EMOJI_NAME_CHARS = 32`, `MAX_EMOJI_BYTES = 256 * 1024`, `MIN_STICKER_NAME_CHARS = 2`, `MAX_STICKER_NAME_CHARS = 30`, `MAX_STICKER_DESC_CHARS = 100`, `STICKER_FORMAT_PNG = 1`, `STICKER_FORMAT_APNG = 2`, `STICKER_FORMAT_LOTTIE = 3`, `STICKER_FORMAT_GIF = 4`.
   - Проверка serde round-trip для `CreateGuildEmojiRequest`.
   - Проверка `validate_emoji_name(name: &str) -> Result<(), String>`:
     - длина `name.trim()` в диапазоне `2..=32` символов;
     - все символы после `trim()` удовлетворяют `c.is_ascii_alphanumeric() || c == '_'`.
   - Проверка `validate_create_guild_emoji(req: &CreateGuildEmojiRequest, byte_len: u64) -> Result<(), String>`:
     - делегирование валидации имени в `validate_emoji_name(&req.name)`;
     - проверка `byte_len` в диапазоне `1..=MAX_EMOJI_BYTES` (`1..=262_144`);
     - проверка префикса `req.image_data_uri` (`data:image/{png,gif,jpeg,webp};base64,`) и непустого base64-пейлоада после запятой;
     - проверка каждой роли в `req.roles` как положительного числового Snowflake (`1..=20` ASCII-цифр, без ведущего `'0'`).
   - Проверка `build_guild_emojis_route(guild_id: &str, emoji_id: Option<&str>) -> Result<String, String>`:
     - валидация `guild_id` (и `emoji_id`, если `Some`) как положительных числовых Snowflake (`1..=20` ASCII-цифр, без ведущего `'0'`);
     - возврат `"/api/v9/guilds/{guild_id}/emojis"` или `"/api/v9/guilds/{guild_id}/emojis/{emoji_id}"`.
2. **`rust/gui-spike/src/expressions.rs`**:
   - Реализованы все константы, структура `CreateGuildEmojiRequest` и функции `validate_emoji_name`, `validate_create_guild_emoji`, `build_guild_emojis_route`.
3. **`rust/gui-spike/src/lib.rs`**:
   - Добавлен экспорт `pub mod expressions;`.

## 4. RED-прогон

Сохранено в `/home/user/orca/project/waves/RW-40/G1/task-242-be-expressions/red.log`:

```text
error[E0432]: unresolved import `discord_gui_spike::expressions`
  --> tests/expressions_contract.rs:22:24
   |
22 | use discord_gui_spike::expressions::{
   |                        ^^^^^^^^^^^ could not find `expressions` in `discord_gui_spike`

error: could not compile `discord-gui-spike` (test "expressions_contract") due to 1 previous error
EXIT_CODE=101
```

## 5. GREEN-прогон и проверки (Targeted + Clippy + Rustfmt)

Сохранено в `/home/user/orca/project/waves/RW-40/G1/task-242-be-expressions/green.log`:

```text
running 7 tests
test build_guild_emojis_route_formats_collection_and_item_routes_and_validates_snowflakes ... ok
test create_guild_emoji_request_serde_roundtrip ... ok
test expressions_constants_match_discord_contract ... ok
test validate_create_guild_emoji_enforces_byte_len_and_supported_image_data_uris ... ok
test validate_create_guild_emoji_validates_name_and_role_snowflakes ... ok
test validate_emoji_name_accepts_valid_trimmed_ascii_alphanumeric_and_underscore_names ... ok
test validate_emoji_name_rejects_short_long_non_ascii_and_invalid_punctuation ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT_CODE=0
```

Дополнительно проверено:
- `cargo clippy --manifest-path rust/gui-spike/Cargo.toml -p discord-gui-spike --test expressions_contract --lib -- -D warnings` — `0 warnings`, `exit=0`.
- `rustfmt --edition 2021 --check rust/gui-spike/src/expressions.rs rust/gui-spike/tests/expressions_contract.rs` — `exit=0`.

## 6. Файлы + md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/expressions.rs` | `9c9aa605e2e4c50f395652ba49004c1d` |
| `rust/gui-spike/src/lib.rs` | `458eb47566cf6bd8b55dab9e633bee0b` |
| `rust/gui-spike/tests/expressions_contract.rs` | `5739d978a28ff4f14d7d88e40e2b7085` |
| `waves/RW-40/G1/task-242-be-expressions/red.log` | `91b9494f2ab69b393f18a3ee72537755` |
| `waves/RW-40/G1/task-242-be-expressions/green.log` | `0d94f50ee4e95e0fe69d1a68e17eef69` |

## 7. Риски и замечания

- **Дрейф `rust/gui-spike/Cargo.lock`**: при запуске `cargo` перегенерировался `Cargo.lock` из-за `discord-codec` (`zstd`); откатан через `git checkout -- rust/gui-spike/Cargo.lock`, чтобы строго соблюсти write-scope.
- `package-lock.json` был модифицирован в рабочем дереве до старта задачи — не трогался.
- `git commit` не выполнялся.
