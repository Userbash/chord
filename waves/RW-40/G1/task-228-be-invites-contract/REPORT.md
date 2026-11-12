# Отчёт воркера — task-228: BFF-контракт приглашений гильдии и vanity-кодов (`BE-P1-21` / `invites`)

## 1. Batch / карточка

- **Волна**: `RW-40`, отдел `G1` (Backend).
- **Карточка**: `task-228` (`BE-P1-21`) — Rust Guild Invites & Vanity Code BFF Contract (`ALLOWED_INVITE_MAX_AGES`, `ALLOWED_INVITE_MAX_USES`, `CreateChannelInviteRequest`, `validate_invite_code`, `build_invite_resolve_route`, `build_create_channel_invite_route_and_body`).
- **Ветка / worktree**: `feature/task-228-be-invites-contract`, `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`.
- **Write-Scope (единственные изменённые файлы)**:
  - `rust/gui-spike/src/invites.rs`
  - `rust/gui-spike/src/lib.rs`
  - `rust/gui-spike/tests/invites_contract.rs`
  - `/home/user/orca/project/waves/RW-40/G1/task-228-be-invites-contract/REPORT.md` (и `red.log`, `green.log`)

## 2. Что сделано

1. **`rust/gui-spike/tests/invites_contract.rs` (Red-first, +5 контрактных тестов `BE-P1-21`, итого 25 тестов)**:
   - `be_p1_21_allowed_invite_constants_match_contract`: проверка `ALLOWED_INVITE_MAX_AGES = [0, 1800, 3600, 21600, 43200, 86400, 604800, 2592000]` и `ALLOWED_INVITE_MAX_USES = [0, 1, 5, 10, 25, 100]`.
   - `be_p1_21_create_channel_invite_request_serde_roundtrip`: проверка сериализации/десериализации `CreateChannelInviteRequest`.
   - `be_p1_21_validate_invite_code_trims_and_enforces_2_to_32_ascii_alnum_dash_underscore`: проверка `validate_invite_code(code)` (`trim()`, длина `2..=32`, символы `[A-Za-z0-9_-]`, отклонение `<2`, `>32`, пробелов внутри, `/`, `?`, `#`, `.`, не-ASCII).
   - `be_p1_21_build_invite_resolve_route_formats_query_and_propagates_code_validation`: проверка `build_invite_resolve_route(code, with_counts, with_expiration)` -> `"/invites/{c}?with_counts={with_counts}&with_expiration={with_expiration}"` и проброса ошибок `validate_invite_code`.
   - `be_p1_21_build_create_channel_invite_route_and_body_validates_snowflake_and_tables`: проверка `build_create_channel_invite_route_and_body(req)` на валидацию `channel_id` (канонический положительный `u64` Snowflake `1..=u64::MAX` без ведущих нулей), `max_age` в `ALLOWED_INVITE_MAX_AGES`, `max_uses` в `ALLOWED_INVITE_MAX_USES` и формирование маршрута `"/channels/{channel_id}/invites"` и JSON-тела `{"max_age", "max_uses", "temporary", "unique"}`.
2. **`rust/gui-spike/src/invites.rs`**:
   - Добавлены константы `ALLOWED_INVITE_MAX_AGES: [u32; 8]` и `ALLOWED_INVITE_MAX_USES: [u32; 6]`.
   - Добавлена структура `CreateChannelInviteRequest`.
   - Реализованы функции `validate_invite_code`, `build_invite_resolve_route`, `build_create_channel_invite_route_and_body`.
   - Существующие API и тесты `G-03` / `STUB-05` полностью сохранены без регрессий.
3. **`rust/gui-spike/src/lib.rs`**:
   - Экспортирован модуль `pub mod invites;` с обновлённым doc-комментарием (`task-228` / `BE-P1-21`).

## 3. RED-прогон

Сохранено в `/home/user/orca/project/waves/RW-40/G1/task-228-be-invites-contract/red.log`:

```text
error[E0432]: unresolved imports `discord_gui_spike::invites::build_create_channel_invite_route_and_body`, `discord_gui_spike::invites::build_invite_resolve_route`, `discord_gui_spike::invites::validate_invite_code`, `discord_gui_spike::invites::CreateChannelInviteRequest`, `discord_gui_spike::invites::ALLOWED_INVITE_MAX_AGES`, `discord_gui_spike::invites::ALLOWED_INVITE_MAX_USES`
    --> tests/invites_contract.rs:1261:5
error: could not compile `discord-gui-spike` (test "invites_contract") due to 1 previous error
EXIT_CODE=101
```

## 4. GREEN-прогон и проверки (Targeted + Clippy + Rustfmt)

Сохранено в `/home/user/orca/project/waves/RW-40/G1/task-228-be-invites-contract/green.log`:

```text
running 25 tests
test be_p1_21_allowed_invite_constants_match_contract ... ok
test be_p1_21_build_invite_resolve_route_formats_query_and_propagates_code_validation ... ok
test be_p1_21_create_channel_invite_request_serde_roundtrip ... ok
test be_p1_21_validate_invite_code_trims_and_enforces_2_to_32_ascii_alnum_dash_underscore ... ok
test be_p1_21_build_create_channel_invite_route_and_body_validates_snowflake_and_tables ... ok
...
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT_CODE=0
```

Дополнительно проверено:
- `cargo clippy --manifest-path rust/gui-spike/Cargo.toml -p discord-gui-spike --test invites_contract --lib -- -D warnings` — `0 warnings`, `exit=0`.
- `rustfmt --edition 2021 --check rust/gui-spike/src/invites.rs rust/gui-spike/tests/invites_contract.rs` — `exit=0`.
- `rustfmt --edition 2021 --check --config skip_children=true rust/gui-spike/src/lib.rs` — `exit=0`.

## 5. Файлы + md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/invites.rs` | `dcf41477b5f5842b51ff389d992e74c4` |
| `rust/gui-spike/src/lib.rs` | `69e0dd1e4fd86332e62df577be982b62` |
| `rust/gui-spike/tests/invites_contract.rs` | `1b97e688dafd39d9c5a4463cf7541ed8` |
| `waves/RW-40/G1/task-228-be-invites-contract/red.log` | `25cae1f9d5c5fbed967ce686ff8ad291` |
| `waves/RW-40/G1/task-228-be-invites-contract/green.log` | `e768800d9c1868e67386386202539533` |

## 6. Риски и замечания

- **Дрейф `rust/gui-spike/Cargo.lock`**: при запуске `cargo` перегенерировался `Cargo.lock` из-за `discord-codec` (`zstd`); откатан через `git checkout -- rust/gui-spike/Cargo.lock`, чтобы строго соблюсти write-scope.
- `package-lock.json` был модифицирован в рабочем дереве до старта задачи — не трогался.
- `git commit` не выполнялся.
