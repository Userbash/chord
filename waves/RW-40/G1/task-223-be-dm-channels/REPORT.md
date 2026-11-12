# Отчёт воркера — task-223: BFF-контракт создания DM / Group DM каналов и управления участниками Group DM (`dm_channels`)

## 1. Batch / карточка

- **Волна**: `RW-40`, отдел `G1` (Backend).
- **Карточка**: `task-223` (`BE-P1-20` / `dm_channels`) — BFF-контракт создания одиночных и групповых ЛС (`CreateDmRequest`, `validate_create_dm_request`, `build_create_dm_body`) и маршрутов управления участниками Group DM (`build_group_dm_recipient_route`).
- **Ветка / worktree**: `feature/task-223-be-dm-channels`, `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`.
- **Write-Scope (единственные изменённые файлы)**:
  - `rust/gui-spike/src/dm_channels.rs`
  - `rust/gui-spike/src/lib.rs`
  - `rust/gui-spike/tests/dm_channels_contract.rs`
  - `/home/user/orca/project/waves/RW-40/G1/task-223-be-dm-channels/REPORT.md` (и `red.log`, `green.log`)

## 2. Оракул (read-only)

Сверено с `reference-oracle/rest-static.json` и `rust/gui-spike/src/dm_friends.rs`:

- `USER_CHANNELS` (`rest-static.json:255-261`): `POST /users/@me/channels`, при создании одиночного DM передаётся `{"recipient_id": "..."}`, при создании Group DM (`2..=9` получателей) — `{"recipients": [...]}`.
- `CHANNEL_RECIPIENT` (`rest-static.json:2594-2600`): `PUT/DELETE /channels/{channel_id}/recipients/{user_id}`.
- Лимиты Group DM: `MAX_GROUP_DM_RECIPIENTS = 9` (до 10 участников вместе с автором), `MAX_GROUP_DM_NAME_CHARS = 100`.

## 3. Что сделано

1. **`rust/gui-spike/tests/dm_channels_contract.rs` (Red-first, 8 тестов)**:
   - Проверка констант `MAX_GROUP_DM_RECIPIENTS = 9` и `MAX_GROUP_DM_NAME_CHARS = 100`.
   - Проверка serde round-trip для `CreateDmRequest`.
   - Проверка `validate_create_dm_request(req: &CreateDmRequest)`:
     - отклонение пустого списка `recipient_ids` и списка длиной `> 9` (`10` элементов);
     - допуск `1` и `9` элементов (`MAX_GROUP_DM_RECIPIENTS`);
     - проверка каждого `recipient_id` как канонической положительной `u64` Snowflake строки (`"1"`, `u64::MAX` — `Ok`; `""`, `"0"`, `"000"`, ведущие нули `"0123456789"`, `"01"`, не-цифры, пробелы, `u64::MAX + 1` — `Err`);
     - отклонение дублирующихся `recipient_id` (смежных и несмежных).
   - Проверка `build_create_dm_body(req: &CreateDmRequest)`:
     - возврат `{"recipient_id": req.recipient_ids[0]}` при `recipient_ids.len() == 1`;
     - возврат `{"recipients": req.recipient_ids}` при `recipient_ids.len() in 2..=9`;
     - проброс ошибок валидации.
   - Проверка `build_group_dm_recipient_route(channel_id, user_id)`:
     - построение канонического маршрута `"/channels/{channel_id}/recipients/{user_id}"`;
     - валидация `channel_id` и `user_id` как канонических положительных `u64` Snowflake ID.
2. **`rust/gui-spike/src/dm_channels.rs`**:
   - Реализованы константы `MAX_GROUP_DM_RECIPIENTS = 9`, `MAX_GROUP_DM_NAME_CHARS = 100`.
   - Реализована структура `CreateDmRequest`.
   - Реализованы функции `validate_create_dm_request`, `build_create_dm_body`, `build_group_dm_recipient_route`.
3. **`rust/gui-spike/src/lib.rs`**:
   - Экспортирован модуль `pub mod dm_channels;`.

## 4. RED-прогон

Сохранено в `/home/user/orca/project/waves/RW-40/G1/task-223-be-dm-channels/red.log`:

```text
error[E0432]: unresolved import `discord_gui_spike::dm_channels`
  --> tests/dm_channels_contract.rs:21:24
   |
21 | use discord_gui_spike::dm_channels::{
   |                        ^^^^^^^^^^^ could not find `dm_channels` in `discord_gui_spike`

error: could not compile `discord-gui-spike` (test "dm_channels_contract") due to 1 previous error
EXIT_CODE=101
```

## 5. GREEN-прогон и проверки (Targeted + Clippy + Rustfmt)

Сохранено в `/home/user/orca/project/waves/RW-40/G1/task-223-be-dm-channels/green.log`:

```text
running 8 tests
test build_create_dm_body_propagates_validation_errors ... ok
test build_create_dm_body_returns_recipient_id_for_single_and_recipients_for_group ... ok
test build_group_dm_recipient_route_returns_canonical_path_and_validates_inputs ... ok
test create_dm_request_serde_roundtrip ... ok
test dm_channels_constants_match_discord_contract ... ok
test validate_create_dm_request_checks_canonical_positive_u64_snowflakes ... ok
test validate_create_dm_request_enforces_recipient_count_bounds ... ok
test validate_create_dm_request_rejects_duplicate_recipients ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT_CODE=0
```

Дополнительно проверено:
- `cargo clippy --manifest-path rust/gui-spike/Cargo.toml -p discord-gui-spike --test dm_channels_contract --lib -- -D warnings` — `0 warnings`, `exit=0`.
- `rustfmt --edition 2021 --check rust/gui-spike/src/dm_channels.rs rust/gui-spike/tests/dm_channels_contract.rs` — `exit=0`.
- `rustfmt --edition 2021 --check --config skip_children=true rust/gui-spike/src/lib.rs` — `exit=0`.

## 6. Файлы + md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/dm_channels.rs` | `f66582c3a8cd321ba88207f3d74cc9d0` |
| `rust/gui-spike/src/lib.rs` | `4fe562628d5e0aa400ff684368d08580` |
| `rust/gui-spike/tests/dm_channels_contract.rs` | `265727fd8085f43126fac015bd423d8a` |
| `waves/RW-40/G1/task-223-be-dm-channels/red.log` | `aa8ee5c3b4452c3d25180e026eb7d5ad` |
| `waves/RW-40/G1/task-223-be-dm-channels/green.log` | `a53a4b95ff9fbf72fdbc56922abb6a37` |

## 7. Риски и замечания

- **Дрейф `rust/gui-spike/Cargo.lock`**: при запуске `cargo` перегенерировался `Cargo.lock` из-за `discord-codec` (`zstd`); откатан через `git checkout -- rust/gui-spike/Cargo.lock`, чтобы строго соблюсти write-scope.
- `package-lock.json` был модифицирован в рабочем дереве до старта задачи — не трогался.
- `git commit` не выполнялся.
