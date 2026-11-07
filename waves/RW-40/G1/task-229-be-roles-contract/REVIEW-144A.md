# Technical Review 144A — PR-115 / task-229 (`BE-P1-22`, `roles`)

**Verdict: APPROVED**

- Ревьюер: Technical Reviewer (DeepSeek), диспатч `ctx_452c6c2076e7`.
- Worktree: `/home/sanya/orca/workspaces/project/feature-task-229-be-roles-contract`, HEAD `ae89c2f`.
- Write-scope соблюдён: изменены только `rust/gui-spike/src/roles.rs`, `rust/gui-spike/tests/roles_contract.rs`, `rust/gui-spike/src/lib.rs` (+ артефакты `waves/RW-40/G1/task-229-be-roles-contract/`). `git commit` не выполнялся.

## 1. Targeted-прогон (baseline)

```
CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target \
cargo test --manifest-path rust/gui-spike/Cargo.toml --test roles_contract
```

`test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` (exit 0).

## 2. Мутации (5/5 killed)

Каждая мутация вносилась в `rust/gui-spike/src/roles.rs`, прогонялся тот же targeted-тест, затем файл восстанавливался из побайтовой копии.

| # | Мутация | Ожидание | Результат | Убившие тесты |
|---|---|---|---|---|
| M1 | `MAX_ROLE_NAME_CHARS: 100 -> 101` | killed | **killed**, exit 101 | `roles_constants_match_discord_contract`, `validate_upsert_role_request_enforces_trimmed_unicode_name_bounds` |
| M2 | `MAX_RGB_COLOR: 0x00FF_FFFF -> 0x00FF_FFFE` | killed | **killed**, exit 101 | `roles_constants_match_discord_contract` |
| M3 | `PERM_ADMINISTRATOR: 1 << 3 -> 1 << 4` | killed | **killed**, exit 101 | `roles_constants_match_discord_contract`, `compute_effective_permissions_handles_owner_admin_and_bitwise_or` |
| M4 | Удалён short-circuit владельца (`if is_owner { return u64::MAX }` заменён на `let _ = is_owner;`) | killed | **killed**, exit 101 | `compute_effective_permissions_handles_owner_admin_and_bitwise_or` |
| M5 | Снята проверка ведущих нулей в `validate_snowflake` | killed | **killed**, exit 101 | `build_member_role_route_returns_canonical_path_and_validates_all_three_snowflakes`, `validate_upsert_role_request_checks_canonical_guild_id_snowflake` |

**Итог: 5/5 (100%).** Выживших мутаций нет.

## 3. Восстановление и целостность

| Файл | md5 после восстановления | md5 в REPORT.md | Совпадает |
|---|---|---|---|
| `rust/gui-spike/src/roles.rs` | `08f60934a7c87fa8f2da75cd314cd95a` | `08f60934a7c87fa8f2da75cd314cd95a` | да |
| `rust/gui-spike/src/lib.rs` | `a13ef3f9983b2841831bcdaadceb595c` | `a13ef3f9983b2841831bcdaadceb595c` | да |
| `rust/gui-spike/tests/roles_contract.rs` | `b5d79ad08dbf943a934f5ee6c46b2f88` | `b5d79ad08dbf943a934f5ee6c46b2f88` | да |

`diff -q` восстановленного `roles.rs` с исходной копией — идентичны (`RESTORE-IDENTICAL`).

## 4. Замечания по коду

1. Тесты сверяют значения с независимыми константами предметной области (Discord API v9: `100`, `0x00FF_FFFF`, `1 << 3`), а не с реализацией, — оракул корректен.
2. `validate_snowflake` отвергает `""`, `"0"`, ведущие нули, не-цифры, пробелы и переполнение `u64`; граница `1..=u64::MAX` покрыта с обеих сторон.
3. `validate_u64_decimal` корректно допускает единственный `"0"` и отклоняет `"00"`, `"01"`, hex, знаки и `u64::MAX + 1`.
4. Длина имени проверяется по `chars()` после `trim()`, что даёт ровно 100 Unicode-символов независимо от UTF-8-длины; тест покрывает 100 и 101 символ для ASCII и кириллицы.
5. `compute_effective_permissions` возвращает `u64::MAX` и для владельца, и при установленном `ADMINISTRATOR`; проверено для `everyone_perms` и для отдельной роли.
6. Замечание P3 (не блокирует): проверка `n > 0` в `validate_snowflake` избыточна после отсечения `starts_with('0')`, но безвредна и служит явным контрактом. Мутация M5 подтверждает, что обе ветки защиты проверяются тестами.

Блокирующих замечаний (`P0`/`P1`) нет.
