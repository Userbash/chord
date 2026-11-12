# Отчёт воркера — task-209: контракт публикаций в форум-канале (`forum`)

## 1. Batch / карточка

- **Волна**: `RW-40`, отдел `G1` (Backend).
- **Карточка**: `task-209` (`BE-P1-07` / `forum`) — контракт создания публикации в форум-канале (`CreateForumPostRequest`, `ForumSortOrder`, `validate_forum_post_request`, `build_forum_post_route_and_body`).
- **Ветка / worktree**: `feature/task-209-be-forum-contract`, `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`.
- **Write-Scope (единственные изменённые файлы)**:
  - `rust/gui-spike/src/forum.rs`
  - `rust/gui-spike/src/lib.rs`
  - `rust/gui-spike/tests/forum_contract.rs`
  - `/home/user/orca/project/waves/RW-40/G1/task-209-be-forum-contract/REPORT.md` (и `red.log`, `green.log`)

## 2. Оракул (read-only)

Сверено с `reference-oracle/rest-static.json`, `reference-oracle/gateway-schema.md` и модулями `ipc_ext.rs`, `pins_threads.rs`, `messages.rs`:

- `CHANNEL_THREADS` (`rest-static.json:7863`): `POST /channels/{channel_id}/threads`, тело `{"name": ..., "auto_archive_duration": ..., "applied_tags": [...], "message": {"content": ...}}` (и опциональный `rate_limit_per_user`).
- `FORUM_TAG` (`rest-static.json:2820`): `applied_tags` (`<= 5` Snowflake ID).
- `gateway-schema.md:326,353`: `default_sort_order` (`0 = LatestActivity`, `1 = CreationDate`), `default_auto_archive_duration` (`[60, 1440, 4320, 10080]`).

## 3. Что сделано

1. **`rust/gui-spike/tests/forum_contract.rs` (Red-first, 10 тестов)**:
   - Проверка констант `MAX_FORUM_APPLIED_TAGS = 5`, `MAX_FORUM_TITLE_CHARS = 100`, `ALLOWED_AUTO_ARCHIVE_MINUTES = [60, 1440, 4320, 10080]`.
   - Проверка `ForumSortOrder` (`LatestActivity = 0`, `CreationDate = 1`) — дискриминанты и serde serialize/deserialize.
   - Позитивные, граничные и негативные сценарии `validate_forum_post_request`:
     - `channel_id`: `"1"` и `u64::MAX` (`Ok`), `""`, `"0"`, `"0000"`, не-цифры, `u64::MAX + 1` (`Err`);
     - `name.trim()`: `1` и `100` символов Unicode (`Ok`), пустые/пробельные и `101` символ (`Err`);
     - `auto_archive_duration`: `60`, `1440`, `4320`, `10080` (`Ok`), прочие значения (`Err`);
     - `content.trim()`: `1` и `2000` символов Unicode (`Ok`), пустые/пробельные и `2001` символ (`Err`);
     - `applied_tags`: `0..=5` валидных Snowflake ID (`Ok`), `6` тегов или невалидный Snowflake в списке (`Err`).
   - Проверка `build_forum_post_route_and_body(req)` — возврат маршрута `"/channels/{channel_id}/threads"` и JSON-тела `{"name": ..., "auto_archive_duration": ..., "applied_tags": ..., "message": {"content": ...}}` (а также проброс ошибок валидации и `rate_limit_per_user`).
2. **`rust/gui-spike/src/forum.rs`**:
   - Реализованы константы `MAX_FORUM_APPLIED_TAGS`, `MAX_FORUM_TITLE_CHARS`, `MAX_FORUM_CONTENT_CHARS`, `ALLOWED_AUTO_ARCHIVE_MINUTES`.
   - Реализованы `ForumSortOrder`, `CreateForumPostRequest`, `validate_forum_post_request` и `build_forum_post_route_and_body`.
3. **`rust/gui-spike/src/lib.rs`**:
   - Экспортирован модуль `pub mod forum;`.

## 4. RED-прогон

Сохранено в `/home/user/orca/project/waves/RW-40/G1/task-209-be-forum-contract/red.log`:

```text
error[E0432]: unresolved import `discord_gui_spike::forum`
  --> tests/forum_contract.rs:16:24
   |
16 | use discord_gui_spike::forum::{
   |                        ^^^^^ could not find `forum` in `discord_gui_spike`

error: could not compile `discord-gui-spike` (test "forum_contract") due to 1 previous error
EXIT_CODE=101
```

## 5. GREEN-прогон и проверки (Targeted + Clippy + Rustfmt)

Сохранено в `/home/user/orca/project/waves/RW-40/G1/task-209-be-forum-contract/green.log`:

```text
running 10 tests
test build_forum_post_route_and_body_propagates_validation_errors ... ok
test create_forum_post_request_serde_roundtrip ... ok
test build_forum_post_route_and_body_returns_threads_route_and_json_payload ... ok
test forum_sort_order_discriminants_and_serde_roundtrip ... ok
test validate_forum_post_request_checks_channel_id_snowflake_bounds ... ok
test forum_constants_match_discord_contract ... ok
test validate_forum_post_request_enforces_allowed_auto_archive_minutes ... ok
test validate_forum_post_request_enforces_applied_tags_max_5_and_snowflakes ... ok
test validate_forum_post_request_enforces_trimmed_content_1_to_2000_chars ... ok
test validate_forum_post_request_enforces_trimmed_title_1_to_100_chars ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT_CODE=0
```

Дополнительно проверено:
- `cargo clippy --manifest-path rust/gui-spike/Cargo.toml -p discord-gui-spike --test forum_contract --lib -- -D warnings` — `0 warnings`, `exit=0`.
- `rustfmt --edition 2021 --check rust/gui-spike/src/forum.rs rust/gui-spike/tests/forum_contract.rs` — `exit=0`.
- `rustfmt --edition 2021 --check --config skip_children=true rust/gui-spike/src/lib.rs` — `exit=0`.

## 6. Файлы + md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/forum.rs` | `3b5be56436eca056649ba8533172a5f2` |
| `rust/gui-spike/src/lib.rs` | `95bbf8db66a9efa9fc6d37f45e165c35` |
| `rust/gui-spike/tests/forum_contract.rs` | `47a3d82fbdc7c1036ff7365285800dae` |
| `waves/RW-40/G1/task-209-be-forum-contract/red.log` | `c3625152ffc3af9faf55bfc41772ef62` |
| `waves/RW-40/G1/task-209-be-forum-contract/green.log` | `e2b7a9b6d6f44b0e570bcfef2bb7e159` |

## 7. Риски и замечания

- **Дрейф `rust/gui-spike/Cargo.lock`**: при запуске `cargo` перегенерировался `Cargo.lock` из-за `discord-codec` (`zstd`); откатан через `git checkout -- rust/gui-spike/Cargo.lock`, чтобы строго соблюсти write-scope.
- `package-lock.json` был модифицирован в рабочем дереве до старта задачи — не трогался.
- `git commit` не выполнялся.
