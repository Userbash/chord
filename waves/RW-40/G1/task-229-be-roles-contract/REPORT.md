# Отчёт воркера — task-229: BFF-контракт управления ролями и правами гильдии (`roles`)

## 1. Batch / карточка

- **Волна**: `RW-40`, отдел `G1` (Backend).
- **Карточка**: `task-229` (`BE-P1-22` / `roles`) — BFF-контракт управления ролями и правами гильдии (`UpsertRoleRequest`, `validate_upsert_role_request`, `compute_effective_permissions`, `build_member_role_route`).
- **Ветка / worktree**: `feature/task-229-be-roles-contract`, `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target`.
- **Write-Scope (единственные изменённые файлы)**:
  - `rust/gui-spike/src/roles.rs`
  - `rust/gui-spike/src/lib.rs`
  - `rust/gui-spike/tests/roles_contract.rs`
  - `/home/sanya/orca/project/waves/RW-40/G1/task-229-be-roles-contract/REPORT.md` (и `red.log`, `green.log`)

## 2. Оракул (read-only)

Сверено с `/home/sanya/re-deep/rest-static.json`:

- `GUILD_ROLES` (`rest-static.json:886-896`): `POST|PATCH /guilds/{guild_id}/roles`.
- `GUILD_ROLE` (`rest-static.json:936-948`): `PATCH|DELETE /guilds/{guild_id}/roles/{role_id}`.
- Назначение/снятие роли участника гильдии: `PUT|DELETE /guilds/{guild_id}/members/{user_id}/roles/{role_id}`.
- Константы Discord API v9: `MAX_ROLE_NAME_CHARS = 100` (Unicode-символы после `trim()`), `MAX_RGB_COLOR = 0x00FF_FFFF` (24-битный RGB без альфа-канала), `PERM_ADMINISTRATOR = 1 << 3` (`8`).

## 3. Что сделано

1. **`rust/gui-spike/tests/roles_contract.rs` (Red-first, 8 тестов)**:
   - Проверка констант `MAX_ROLE_NAME_CHARS = 100`, `MAX_RGB_COLOR = 0x00FF_FFFF`, `PERM_ADMINISTRATOR = 1 << 3`.
   - Проверка serde round-trip для `UpsertRoleRequest`.
   - Проверка `validate_upsert_role_request(req: &UpsertRoleRequest)`:
     - проверка `guild_id` как канонической положительной `u64` Snowflake строки (`"1"`, `u64::MAX` — `Ok`; `""`, `"0"`, `"00"`, ведущие нули `"0123456789"`, `"01"`, не-цифры, пробелы, `u64::MAX + 1` — `Err`);
     - проверка `req.name.trim()` по длине `1..=100` символов Unicode (отклонение пустой и пробельной строки, допуск `1` и `100` символов кириллицы и с внешними пробелами, отклонение `101` символа);
     - проверка `req.color <= MAX_RGB_COLOR` (`0`, `1`, `0x0058_65F2`, `0x00FF_FFFF` — `Ok`; `0x0100_0000`, `u32::MAX` — `Err`);
     - проверка `req.permissions` как канонической десятичной строки `u64` (`"0"`, `"1"`, `"8"`, `"1071698660929"`, `u64::MAX` — `Ok`; `""`, `"00"`, `"01"`, `"08"`, отрицательные, hex, `u64::MAX + 1` — `Err`).
   - Проверка `compute_effective_permissions(everyone_perms, role_perms, is_owner)`:
     - при `is_owner == true` возвращает `u64::MAX`;
     - побитовое объединение (`|`) `everyone_perms` и элементов `role_perms`;
     - если в аккумуляторе установлен бит `PERM_ADMINISTRATOR` (`1 << 3`), возвращает `u64::MAX`, иначе аккумулятор.
   - Проверка `build_member_role_route(guild_id, user_id, role_id)`:
     - построение канонического маршрута `"/guilds/{guild_id}/members/{user_id}/roles/{role_id}"`;
     - валидация всех трёх параметров (`guild_id`, `user_id`, `role_id`) как канонических положительных `u64` Snowflake ID.
2. **`rust/gui-spike/src/roles.rs`**:
   - Реализованы константы `MAX_ROLE_NAME_CHARS = 100`, `MAX_RGB_COLOR = 0x00FF_FFFF`, `PERM_ADMINISTRATOR = 1 << 3`.
   - Реализована структура `UpsertRoleRequest`.
   - Реализованы функции `validate_upsert_role_request`, `compute_effective_permissions`, `build_member_role_route`.
3. **`rust/gui-spike/src/lib.rs`**:
   - Экспортирован модуль `pub mod roles;`.

## 4. RED-прогон

Сохранено в `/home/sanya/orca/project/waves/RW-40/G1/task-229-be-roles-contract/red.log`:

```text
error[E0432]: unresolved import `discord_gui_spike::roles`
  --> tests/roles_contract.rs:21:24
   |
21 | use discord_gui_spike::roles::{
   |                        ^^^^^ could not find `roles` in `discord_gui_spike`

error: could not compile `discord-gui-spike` (test "roles_contract") due to 1 previous error
EXIT_CODE=101
```

## 5. GREEN-прогон и проверки (Targeted + Clippy + Rustfmt)

Сохранено в `/home/sanya/orca/project/waves/RW-40/G1/task-229-be-roles-contract/green.log`:

```text
running 8 tests
test build_member_role_route_returns_canonical_path_and_validates_all_three_snowflakes ... ok
test compute_effective_permissions_handles_owner_admin_and_bitwise_or ... ok
test roles_constants_match_discord_contract ... ok
test upsert_role_request_serde_roundtrip ... ok
test validate_upsert_role_request_checks_canonical_guild_id_snowflake ... ok
test validate_upsert_role_request_enforces_canonical_u64_permissions ... ok
test validate_upsert_role_request_enforces_rgb_color_bounds ... ok
test validate_upsert_role_request_enforces_trimmed_unicode_name_bounds ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT_CODE=0
```

Дополнительно проверено:
- `cargo clippy --manifest-path rust/gui-spike/Cargo.toml -p discord-gui-spike --test roles_contract --lib -- -D warnings` — `0 warnings`, `exit=0`.
- `rustfmt --edition 2021 --check rust/gui-spike/src/roles.rs rust/gui-spike/tests/roles_contract.rs` — `exit=0`.
- `rustfmt --edition 2021 --check --config skip_children=true rust/gui-spike/src/lib.rs` — `exit=0`.

## 6. Файлы + md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/roles.rs` | `08f60934a7c87fa8f2da75cd314cd95a` |
| `rust/gui-spike/src/lib.rs` | `a13ef3f9983b2841831bcdaadceb595c` |
| `rust/gui-spike/tests/roles_contract.rs` | `b5d79ad08dbf943a934f5ee6c46b2f88` |
| `waves/RW-40/G1/task-229-be-roles-contract/red.log` | `161b755553dd8eb8d2d294db3b280f54` |
| `waves/RW-40/G1/task-229-be-roles-contract/green.log` | `6b34a4c0a2cc034004921f7ae9856f45` |

## 7. Риски и замечания

- **Дрейф `rust/gui-spike/Cargo.lock`**: при запуске `cargo` перегенерировался `Cargo.lock` из-за `discord-codec` (`zstd`); откатан через `git checkout -- rust/gui-spike/Cargo.lock`, чтобы строго соблюсти write-scope.
- `package-lock.json` был модифицирован в рабочем дереве до старта задачи — не трогался.
- `git commit` не выполнялся.
