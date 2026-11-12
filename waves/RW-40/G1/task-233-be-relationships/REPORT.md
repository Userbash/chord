# Отчёт воркера — task-233: BFF-контракт связей и заявок в друзья (`relationships`)

## 1. Batch / карточка

- **Волна**: `RW-40`, отдел `G1` (Backend).
- **Карточка**: `task-233` (`BE-P1-23` / `relationships`) — BFF-контракт связей и заявок в друзья (`RELATIONSHIP_FRIEND`, `RELATIONSHIP_BLOCKED`, `RELATIONSHIP_PENDING_INCOMING`, `RELATIONSHIP_PENDING_OUTGOING`, `MAX_USERNAME_CHARS`, `SendFriendRequest`, `validate_friend_request_username`, `build_send_friend_request_body`, `build_relationship_user_route`, `build_block_user_body`).
- **Ветка / worktree**: `feature/task-233-be-relationships`, `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`.
- **Write-Scope (единственные изменённые файлы)**:
  - `rust/gui-spike/src/relationships.rs`
  - `rust/gui-spike/src/lib.rs`
  - `rust/gui-spike/tests/relationships_contract.rs`
  - `/home/user/orca/project/waves/RW-40/G1/task-233-be-relationships/REPORT.md` (и `red.log`, `green.log`)

## 2. Оракул (read-only)

Сверено с `reference-oracle/rest-static.json` и `waves/RW-40/DM-FRIENDS-RE.md`:

- `USER_RELATIONSHIPS` (`rest-static.json:19-33`, `DM-FRIENDS-RE.md` §4.2): `POST /users/@me/relationships` с JSON-телом `{"username": trimmed, "discriminator": null}` для Pomelo-аккаунтов.
- `USER_RELATIONSHIP` (`rest-static.json:34-44`, `DM-FRIENDS-RE.md` §4.3): `PUT/DELETE /users/@me/relationships/{user_id}`; блокировка пользователя отправляет `{"type": 2}`.
- `RelationshipType` (`DM-FRIENDS-RE.md` §4.7): `1 = FRIEND`, `2 = BLOCKED`, `3 = PENDING_INCOMING`, `4 = PENDING_OUTGOING`.

## 3. Что сделано

1. **`rust/gui-spike/tests/relationships_contract.rs` (Red-first, 7 тестов)**:
   - Проверка констант `RELATIONSHIP_FRIEND = 1`, `RELATIONSHIP_BLOCKED = 2`, `RELATIONSHIP_PENDING_INCOMING = 3`, `RELATIONSHIP_PENDING_OUTGOING = 4`, `MAX_USERNAME_CHARS = 32`.
   - Проверка serde round-trip для `SendFriendRequest`.
   - Проверка `validate_friend_request_username(username: &str) -> Result<&str, String>`:
     - обрезка внешних пробелов (`trim()`);
     - границы длины `2..=32` символов Unicode (`chars().count()`), включая кириллицу (`2` и `32` символа `Ok`, `< 2` и `33` символа `Err`);
     - отклонение строк, содержащих запрещённые символы `"@"`, `":"`, `"#"`.
   - Проверка `build_send_friend_request_body(req: &SendFriendRequest)`:
     - валидация через `validate_friend_request_username` и возврат `{"username": trimmed, "discriminator": null}`;
     - проброс ошибок валидации.
   - Проверка `build_relationship_user_route(target_user_id: &str)`:
     - валидация канонического положительного `u64` Snowflake ID (`1..=u64::MAX`, только ASCII-цифры, без ведущих нулей);
     - формирование маршрута `"/users/@me/relationships/{target_user_id}"`.
   - Проверка `build_block_user_body()`:
     - возврат `{"type": 2}` (`RELATIONSHIP_BLOCKED`).
2. **`rust/gui-spike/src/relationships.rs`**:
   - Реализованы константы `RELATIONSHIP_FRIEND`, `RELATIONSHIP_BLOCKED`, `RELATIONSHIP_PENDING_INCOMING`, `RELATIONSHIP_PENDING_OUTGOING`, `MAX_USERNAME_CHARS`.
   - Реализована структура `SendFriendRequest`.
   - Реализованы функции `validate_friend_request_username`, `build_send_friend_request_body`, `build_relationship_user_route`, `build_block_user_body`.
3. **`rust/gui-spike/src/lib.rs`**:
   - Экспортирован модуль `pub mod relationships;`.

## 4. RED-прогон

Сохранено в `/home/user/orca/project/waves/RW-40/G1/task-233-be-relationships/red.log`:

```text
error[E0432]: unresolved import `discord_gui_spike::relationships`
  --> tests/relationships_contract.rs:28:24
   |
28 | use discord_gui_spike::relationships::{
   |                        ^^^^^^^^^^^^^ could not find `relationships` in `discord_gui_spike`

error: could not compile `discord-gui-spike` (test "relationships_contract") due to 1 previous error
EXIT_CODE=101
```

## 5. GREEN-прогон и проверки (Targeted + Clippy + Rustfmt)

Сохранено в `/home/user/orca/project/waves/RW-40/G1/task-233-be-relationships/green.log`:

```text
running 7 tests
test build_block_user_body_returns_blocked_relationship_type ... ok
test build_relationship_user_route_validates_canonical_snowflake_and_formats_path ... ok
test build_send_friend_request_body_returns_trimmed_username_and_null_discriminator ... ok
test relationships_constants_match_discord_contract ... ok
test send_friend_request_serde_roundtrip ... ok
test validate_friend_request_username_rejects_forbidden_characters ... ok
test validate_friend_request_username_trims_and_enforces_2_to_32_unicode_char_bounds ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT_CODE=0
```

Дополнительно проверено:
- `cargo clippy --manifest-path rust/gui-spike/Cargo.toml -p discord-gui-spike --test relationships_contract --lib -- -D warnings` — `0 warnings`, `exit=0`.
- `rustfmt --edition 2021 --check rust/gui-spike/src/relationships.rs rust/gui-spike/tests/relationships_contract.rs` — `exit=0`.

## 6. Файлы + md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/relationships.rs` | `17d84aa2bee7b81297f8b313b2a9a953` |
| `rust/gui-spike/src/lib.rs` | `d60aa48c0daf799de67a16388a7d9f04` |
| `rust/gui-spike/tests/relationships_contract.rs` | `b8387f1244df8c7c5608c48a11a03ba7` |
| `waves/RW-40/G1/task-233-be-relationships/red.log` | `34de2cdf79b0dc01425797b1cbf89bc2` |
| `waves/RW-40/G1/task-233-be-relationships/green.log` | `acc9f8cfd7ba9a19253f695e85874b87` |

## 7. Риски и замечания

- **Дрейф `rust/gui-spike/Cargo.lock`**: при запуске `cargo` перегенерировался `Cargo.lock` из-за `discord-codec` (`zstd`); откатан через `git checkout -- rust/gui-spike/Cargo.lock`, чтобы строго соблюсти write-scope.
- `package-lock.json` был модифицирован в рабочем дереве до старта задачи — не трогался.
- `git commit` не выполнялся.
