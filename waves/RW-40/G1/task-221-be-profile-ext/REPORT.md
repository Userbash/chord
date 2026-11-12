# Отчёт воркера — task-221: BFF-контракт профиля пользователя, bio, pronouns, mutuals и заметок (`profile_ext`)

## 1. Batch / карточка

- **Волна**: `RW-40`, отдел `G1` (Backend).
- **Карточка**: `task-221` (`BE-P1-19` / `profile_ext`) — контракт профиля пользователя, bio, pronouns, mutuals и заметок (`UserProfileQuery`, `UpdateProfilePatch`, `build_user_profile_route`, `validate_profile_patch`, `validate_user_note`).
- **Ветка / worktree**: `feature/task-221-be-profile-ext`, `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`.
- **Write-Scope (единственные изменённые файлы)**:
  - `rust/gui-spike/src/profile_ext.rs`
  - `rust/gui-spike/src/lib.rs`
  - `rust/gui-spike/tests/profile_ext_contract.rs`
  - `/home/user/orca/project/waves/RW-40/G1/task-221-be-profile-ext/REPORT.md` (и `red.log`, `green.log`)

## 2. Оракул (read-only)

Сверено с `reference-oracle/rest-static.json` и модулями `profile.rs`, `dm_friends.rs`, `ipc_ext.rs`:

- `USER_PROFILE` (`rest-static.json:77`): `GET /users/{user_id}/profile?with_mutual_guilds=...&with_mutual_friends=...[&guild_id=...]`.
- `NOTE` (`rest-static.json:3518`): `PUT /users/@me/notes/{target_user_id}`.
- Лимиты Discord: `MAX_BIO_CHARS = 190`, `MAX_PRONOUNS_CHARS = 40`, `MAX_NOTE_CHARS = 256`, `accent_color <= 0xFF_FFFF` (24-битный RGB).

## 3. Что сделано

1. **`rust/gui-spike/tests/profile_ext_contract.rs` (Red-first, 8 тестов)**:
   - Проверка констант `MAX_BIO_CHARS = 190`, `MAX_PRONOUNS_CHARS = 40`, `MAX_NOTE_CHARS = 256`.
   - Проверка serde round-trip для `UserProfileQuery` и `UpdateProfilePatch`.
   - Позитивные, граничные и негативные сценарии `build_user_profile_route`:
     - построение маршрута `/users/{user_id}/profile?with_mutual_guilds={true|false}&with_mutual_friends={true|false}[&guild_id={gid}]`;
     - `user_id` и `guild_id`: `"1"` и `u64::MAX` (`Ok`), `""`, `"0"`, `"00"`, ведущие нули `"01"`, `"0123456789"`, не-цифры, `u64::MAX + 1` (`Err`).
   - Проверка `validate_profile_patch(patch)`:
     - `bio`: `None`, `""`, `190` символов Unicode (`Ok`), `191` символ (`Err`);
     - `pronouns`: `None`, `""`, `40` символов Unicode (`Ok`), `41` символ (`Err`);
     - `accent_color`: `None`, `0`, `1`, `0x58_65F2`, `0xFF_FFFF` (`Ok`), `0x0100_0000`, `u32::MAX` (`Err`).
   - Проверка `validate_user_note(target_user_id, note)`:
     - валидация канонического Snowflake `target_user_id` (`"1"`, `u64::MAX` -> `Ok`; `""`, `"0"`, ведущие нули, не-цифры, переполнение -> `Err`);
     - валидация `note.chars().count() <= 256` (включая многобайтовые символы кириллицы) и возврат `Ok("/users/@me/notes/{target_user_id}")`.
2. **`rust/gui-spike/src/profile_ext.rs`**:
   - Реализованы константы `MAX_BIO_CHARS`, `MAX_PRONOUNS_CHARS`, `MAX_NOTE_CHARS`.
   - Реализованы структуры `UserProfileQuery`, `UpdateProfilePatch`.
   - Реализованы функции `build_user_profile_route`, `validate_profile_patch`, `validate_user_note`.
3. **`rust/gui-spike/src/lib.rs`**:
   - Экспортирован модуль `pub mod profile_ext;`.

## 4. RED-прогон

Сохранено в `/home/user/orca/project/waves/RW-40/G1/task-221-be-profile-ext/red.log`:

```text
error[E0432]: unresolved import `discord_gui_spike::profile_ext`
  --> tests/profile_ext_contract.rs:19:24
   |
19 | use discord_gui_spike::profile_ext::{
   |                        ^^^^^^^^^^^ could not find `profile_ext` in `discord_gui_spike`

error: could not compile `discord-gui-spike` (test "profile_ext_contract") due to 1 previous error
EXIT_CODE=101
```

## 5. GREEN-прогон и проверки (Targeted + Clippy + Rustfmt)

Сохранено в `/home/user/orca/project/waves/RW-40/G1/task-221-be-profile-ext/green.log`:

```text
running 8 tests
test build_user_profile_route_formats_query_with_and_without_guild_id ... ok
test build_user_profile_route_validates_canonical_snowflake_bounds ... ok
test profile_ext_constants_match_discord_contract ... ok
test profile_ext_structs_serde_roundtrip ... ok
test validate_profile_patch_enforces_bio_char_limit ... ok
test validate_profile_patch_enforces_24bit_accent_color_bounds ... ok
test validate_profile_patch_enforces_pronouns_char_limit ... ok
test validate_user_note_builds_route_and_enforces_snowflake_and_char_limits ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT_CODE=0
```

Дополнительно проверено:
- `cargo clippy --manifest-path rust/gui-spike/Cargo.toml -p discord-gui-spike --test profile_ext_contract --lib -- -D warnings` — `0 warnings`, `exit=0`.
- `rustfmt --edition 2021 --check rust/gui-spike/src/profile_ext.rs rust/gui-spike/tests/profile_ext_contract.rs` — `exit=0`.
- `rustfmt --edition 2021 --check --config skip_children=true rust/gui-spike/src/lib.rs` — `exit=0`.

## 6. Файлы + md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/profile_ext.rs` | `ebd1724dfa9f513a9ef4d3759c6cccdd` |
| `rust/gui-spike/src/lib.rs` | `0ac6d016e1e0ae30a54d20556e91c973` |
| `rust/gui-spike/tests/profile_ext_contract.rs` | `9a0ac1708db14644ecb37896a1321a6c` |
| `waves/RW-40/G1/task-221-be-profile-ext/red.log` | `f45469038d52ac48f0471036fe108536` |
| `waves/RW-40/G1/task-221-be-profile-ext/green.log` | `b2c764cdd10fabe1a9446e74d581bae1` |

## 7. Риски и замечания

- **Дрейф `rust/gui-spike/Cargo.lock`**: при запуске `cargo` перегенерировался `Cargo.lock` из-за `discord-codec` (`zstd`); откатан через `git checkout -- rust/gui-spike/Cargo.lock`, чтобы строго соблюсти write-scope.
- `package-lock.json` был модифицирован в рабочем дереве до старта задачи — не трогался.
- `git commit` не выполнялся.
