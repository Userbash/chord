# Отчёт воркера — task-259: BFF-контракт голосового саундборда и кастомных звуков (`soundboard`)

## 1. Batch / карточка

- **Волна**: `RW-40`, отдел `G1` (Backend).
- **Карточка**: `task-259` (`BE-P1-31` / `soundboard`) — BFF-контракт голосового саундборда и кастомных звуков гильдии (`MIN_SOUND_NAME_CHARS`, `MAX_SOUND_NAME_CHARS`, `MAX_SOUND_BYTES`, `MIN_SOUND_VOLUME`, `MAX_SOUND_VOLUME`, `SendSoundboardSoundRequest`, `CreateGuildSoundRequest`, `validate_create_guild_sound`, `build_send_soundboard_route_and_body`).
- **Ветка / worktree**: `feature/task-259-be-soundboard`, `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`.
- **Write-Scope (единственные изменённые файлы)**:
  - `rust/gui-spike/src/soundboard.rs`
  - `rust/gui-spike/src/lib.rs`
  - `rust/gui-spike/tests/soundboard_contract.rs`
  - `/home/user/orca/project/waves/RW-40/G1/task-259-be-soundboard/REPORT.md` (и `red.log`, `green.log`)

## 2. Оракул (read-only)

Сверено с `reference-oracle/rest-static.json`:

- `GUILD_SOUNDBOARD_SOUNDS` (`rest-static.json:9268-9281`): `POST /guilds/{arg}/soundboard-sounds`.
- `SEND_SOUNDBOARD_SOUND` (`rest-static.json:9348-9358`): `POST /channels/{arg}/send-soundboard-sound`.
- Константы Discord API v9:
  - `MIN_SOUND_NAME_CHARS = 2`, `MAX_SOUND_NAME_CHARS = 32`, `MAX_SOUND_BYTES = 512 * 1024` (`524_288` байт);
  - `MIN_SOUND_VOLUME = 0.0`, `MAX_SOUND_VOLUME = 1.0`.

## 3. Что сделано

1. **`rust/gui-spike/tests/soundboard_contract.rs` (Red-first, 6 тестов)**:
   - Проверка констант `MIN_SOUND_NAME_CHARS = 2`, `MAX_SOUND_NAME_CHARS = 32`, `MAX_SOUND_BYTES = 512 * 1024` (`524_288`), `MIN_SOUND_VOLUME = 0.0`, `MAX_SOUND_VOLUME = 1.0`.
   - Проверка serde round-trip для `SendSoundboardSoundRequest` и `CreateGuildSoundRequest`.
   - Проверка `validate_create_guild_sound(req: &CreateGuildSoundRequest, byte_len: u64) -> Result<(), String>`:
     - длина `req.name.trim()` в диапазоне `2..=32` символов Unicode;
     - `byte_len` в диапазоне `1..=MAX_SOUND_BYTES` (`1..=524_288`);
     - префикс `req.sound_data_uri` (`data:audio/{ogg,mpeg,mp3,wav};base64,`) и непустой base64-пейлоад после запятой;
     - проверка опциональной громкости `req.volume` (`v.is_finite()` и `(0.0..=1.0).contains(&v)`);
     - проверка опционального `req.emoji_id` как положительного числового Snowflake (`1..=20` ASCII-цифр, без ведущего `'0'`).
   - Проверка `build_send_soundboard_route_and_body(channel_id: &str, req: &SendSoundboardSoundRequest) -> Result<(String, serde_json::Value), String>`:
     - валидация `channel_id`, `req.sound_id` и `req.source_guild_id` (если `Some`) как положительных числовых Snowflake;
     - возврат `("/api/v9/channels/{channel_id}/send-soundboard-sound", json!({ "sound_id": req.sound_id, "source_guild_id": req.source_guild_id }))`.
2. **`rust/gui-spike/src/soundboard.rs`**:
   - Реализованы константы, структуры `SendSoundboardSoundRequest` и `CreateGuildSoundRequest`, а также функции `validate_create_guild_sound` и `build_send_soundboard_route_and_body`.
3. **`rust/gui-spike/src/lib.rs`**:
   - Добавлен экспорт `pub mod soundboard;`.

## 4. RED-прогон

Сохранено в `/home/user/orca/project/waves/RW-40/G1/task-259-be-soundboard/red.log`:

```text
error[E0432]: unresolved import `discord_gui_spike::soundboard`
  --> tests/soundboard_contract.rs:20:24
   |
20 | use discord_gui_spike::soundboard::{
   |                        ^^^^^^^^^^ could not find `soundboard` in `discord_gui_spike`

error: could not compile `discord-gui-spike` (test "soundboard_contract") due to 1 previous error
EXIT_CODE=101
```

## 5. GREEN-прогон и проверки (Targeted + Clippy + Rustfmt)

Сохранено в `/home/user/orca/project/waves/RW-40/G1/task-259-be-soundboard/green.log`:

```text
running 6 tests
test soundboard_constants_match_discord_contract ... ok
test build_send_soundboard_route_and_body_formats_route_and_validates_snowflakes ... ok
test soundboard_request_structs_serde_roundtrip ... ok
test validate_create_guild_sound_enforces_trimmed_unicode_name_bounds ... ok
test validate_create_guild_sound_enforces_byte_len_and_audio_data_uri_prefixes ... ok
test validate_create_guild_sound_validates_volume_and_optional_emoji_id ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

EXIT_CODE=0
```

Дополнительно проверено:
- `cargo clippy --manifest-path rust/gui-spike/Cargo.toml -p discord-gui-spike --test soundboard_contract --lib -- -D warnings` — `0 warnings`, `exit=0`.
- `rustfmt --edition 2021 --check rust/gui-spike/src/soundboard.rs rust/gui-spike/tests/soundboard_contract.rs` — `exit=0`.

## 6. Файлы + md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/soundboard.rs` | `8ba4d45ed8953f59f424843be1a7ad20` |
| `rust/gui-spike/src/lib.rs` | `2640d0d09b224b82eca3cd43d006592f` |
| `rust/gui-spike/tests/soundboard_contract.rs` | `d96debb8e28c4f70e265caef8095b665` |
| `waves/RW-40/G1/task-259-be-soundboard/red.log` | `c698c9466fc9ad3aac46790094315cf7` |
| `waves/RW-40/G1/task-259-be-soundboard/green.log` | `b5a5b3d2e487d249585b27257db913c4` |

## 7. Риски и замечания

- **Дрейф `rust/gui-spike/Cargo.lock`**: при запуске `cargo` перегенерировался `Cargo.lock` из-за `discord-codec` (`zstd`); откатан через `git checkout -- rust/gui-spike/Cargo.lock`, чтобы строго соблюсти write-scope.
- `package-lock.json` был модифицирован в рабочем дереве до старта задачи — не трогался.
- `git commit` не выполнялся.
