# REPORT — TASK-619 (G1 Backend): Permissions Bitfield Engine v9

- **Ветка:** `feature/task-619-permissions-engine`
- **Рабочее дерево:** `/home/sanya/orca/workspaces/project/task-619-permissions-engine`
- **Отдел:** `G1 Backend (Rust Core & Tauri BFF)`
- **Затронутые карточки:** `TASK-619` (`Permissions Bitfield Engine v9`)
- **Дата:** 2026-11-07
- **Коммитов нет:** `git commit` / `git push` не выполнялись (правила пайплайна `AGENTS.md` §6).

---

## 1. Сделано

1. **Сверка с эталонным RE-дампом `/home/sanya/re-deep/` и захватом `002-READY.json`:**
   - В `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0` (`@9770000..9774000`, модуль `136722` `BigFlagUtils` и таблица констант `eC = Object.freeze({...})`) сверены все 41 битовый флаг Discord API v9 (`0..=40`, от `CREATE_INSTANT_INVITE = 1 << 0` до `MODERATE_MEMBERS = 1 << 40`) и функции расчёта прав `U(guildId, member, basePerms, overwrites)` / `w(e)`:
     - базовые права: `everyone_role_perms | (OR всех ролей участника)`;
     - обход для владельца гильдии (`user_id == owner_id`) и наличия бита `ADMINISTRATOR` (`(base_perms & ADMINISTRATOR) != 0`) с возвратом `ALL_PERMISSIONS` (`u64::MAX`);
     - иерархия канальных перезаписей независимо от порядка элементов в массиве `permission_overwrites`:
       1) `@everyone` (`overwrite_type == 0 && id == guild_id`): `perms = (perms & !deny) | allow`;
       2) роли участника (`overwrite_type == 0 && id != guild_id && member_role_ids.contains(&id)`): побитовое накопление `role_allow |= allow` и `role_deny |= deny`, затем `perms = (perms & !role_deny) | role_allow`;
       3) персональная перезапись участника (`overwrite_type == 1 && id == user_id`): `perms = (perms & !member_deny) | member_allow`;
     - шаг 4: если итоговая маска содержит `ADMINISTRATOR` (`(perms & ADMINISTRATOR) != 0`), возвращается `ALL_PERMISSIONS`.
   - По реальной фикстуре `fixtures/gateway/by-type/002-READY.json` (`md5 a1a8fd5662be88108dd25ac619a377f2`, гильдия `ITSKIDON` `1014537851418980372`, канал `1275727867497680918`) проверен расчёт базовых прав (`1097311798689607`) и канальных перезаписей (`463046060607303` / `463320938565447`), где в массиве `permission_overwrites` сначала идёт перезапись роли `1275727508259606598`, а вторым элементом — `@everyone` (`1014537851418980372`).
2. **Реализован модуль `rust/crates/discord-core/src/permissions.rs` (ре-экспортирован в `rust/crates/discord-core/src/lib.rs`):**
   - Тип `Permissions(pub u64)` и модульные константы для всех 41 битовых флагов Discord API v9 (`0..=40`), константы `ALL_PERMISSIONS`, `ALL_V9_DEFINED_PERMISSIONS`, `OVERWRITE_TYPE_ROLE = 0`, `OVERWRITE_TYPE_MEMBER = 1`, побитовые операторы (`BitOr`, `BitAnd`, `BitXor`, `Not`) и методы `from_bits`, `bits`, `has`, `contains`.
   - Структура `PermissionOverwrite { pub id: u64, pub overwrite_type: u8, pub allow: u64, pub deny: u64 }` и `From<crate::ready::PermissionOverwrite>` / `From<&crate::ready::PermissionOverwrite>`. (Крейт `discord-core` намеренно не содержит `serde` в `Cargo.toml:10`, который вне `Scope`).
   - Публичные функции:
     - `pub fn compute_base_permissions(owner_id: u64, user_id: u64, everyone_role_perms: u64, member_roles_perms: &[u64]) -> u64`
     - `pub fn compute_channel_permissions(base_perms: u64, guild_id: u64, user_id: u64, member_role_ids: &[u64], overwrites: &[PermissionOverwrite]) -> u64`
     - `pub const fn has_permission(computed_perms: u64, required_perm: u64) -> bool`
3. **Написан контрактный тест `rust/crates/discord-core/tests/permissions_resolution_contract.rs` (6 тестов):**
   - Проверка всех 41 битовых флагов `0..=40`, попарной ортогональности и суммарной маски `(1u64 << 41) - 1`.
   - Проверка `compute_base_permissions` (пустые роли, объединение нескольких ролей, обход владельца гильдии `owner_id == user_id`, обход `ADMINISTRATOR`).
   - Проверка `compute_channel_permissions` по трёхуровневой иерархии `@everyone -> roles -> member` на массиве перезаписей в обратном порядке.
   - Проверка иммунитета `ADMINISTRATOR` к канальным `deny`, выдачи `ADMINISTRATOR` канальной перезаписью и игнорирования чужих/невалидных `overwrite_type` (`0`, `1`, `2`).
   - Проверка граничных снежинок (`0`, `u64::MAX`), приоритета `allow` над `deny` на одном уровне и составных масок в `has_permission`.
   - Сквозная проверка по реальному захвату `fixtures/gateway/by-type/002-READY.json`.

---

## 2. Строгий TDD (Red → Green)

### Red-прогон (до реализации `permissions.rs`):
- **Команда:**
  `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target flock /tmp/discord-lab-cargo.lock cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --test permissions_resolution_contract`
- **Лог:** `waves/RW-40/G1/task-619-permissions-engine/red.log` (`md5 392cc9f5f4c497a9ec95fbb4d8ed3496`, `exit 101`):
  ```text
  error[E0432]: unresolved import `discord_core::permissions`
    --> crates/discord-core/tests/permissions_resolution_contract.rs:17:19
     |
  17 | use discord_core::permissions::{
     |                   ^^^^^^^^^^^ could not find `permissions` in `discord_core`
  ```

### Green-прогон (после реализации, 5 прогонов подряд):
- **Команда:**
  `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target flock /tmp/discord-lab-cargo.lock cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --test permissions_resolution_contract`
- **Лог:** `waves/RW-40/G1/task-619-permissions-engine/green.log` (`md5 f8febb88a18707554285cf158684d203`, `exit 0`):
  `6 passed; 0 failed; 0 ignored` × 5 прогонов подряд (`flaky_rate = 0%`).
- **Форматирование и `clippy`:**
  - `rustfmt --edition 2024 --check rust/crates/discord-core/src/permissions.rs rust/crates/discord-core/src/lib.rs rust/crates/discord-core/tests/permissions_resolution_contract.rs` → `exit 0` (чисто).
  - `cargo clippy` по `permissions.rs`, `lib.rs` и `permissions_resolution_contract.rs` → `0 warnings`.

---

## 3. Мутационная проверка (5/5 убито)

Лог: `waves/RW-40/G1/task-619-permissions-engine/mutations.log` (`md5 ad011dd9df07de0231870496d476c13a`):
- `M1_owner_bypass` (`if user_id == owner_id` → `if false` в `compute_base_permissions`): **KILLED** (`exit=101`, `mut_md5=68c798ea535d01cc95cc489445454739`)
- `M2_base_admin_bypass` (`if (base_perms & ADMINISTRATOR) != 0` → `if false` в `compute_base_permissions`): **KILLED** (`exit=101`, `mut_md5=7f2b50b73fd38673714a69b5ade6a658`)
- `M3_everyone_overwrite_precedence` (`(perms & !ov.deny) | ov.allow` → `(perms | ov.allow) & !ov.deny` на уровне `@everyone`): **KILLED** (`exit=101`, `mut_md5=243e3d55c898c343b34f0fcac8d4736d`)
- `M4_role_allow_over_deny` (`(perms & !role_deny) | role_allow` → `(perms | role_allow) & !role_deny` на уровне ролей): **KILLED** (`exit=101`, `mut_md5=b3219efb3e2c146169ae7f0b55ef0a9c`)
- `M5_has_permission_composite` (`(computed_perms & required_perm) == required_perm` → `!= 0` в `has_permission`): **KILLED** (`exit=101`, `mut_md5=b2fa51242997e639e67203ed25d73ca5`)
- Исходный файл `rust/crates/discord-core/src/permissions.rs` восстановлен бит-в-бит (`md5 2565ee3ace58151e0a3ff9a784442a3b`).

---

## 4. Затронутые файлы и `md5sum`

```text
2565ee3ace58151e0a3ff9a784442a3b  rust/crates/discord-core/src/permissions.rs
aeac7f0ac6d4575ee249507f2b99aacc  rust/crates/discord-core/src/lib.rs
f7c96d432c5941c14e1715d39f46e47b  rust/crates/discord-core/tests/permissions_resolution_contract.rs
392cc9f5f4c497a9ec95fbb4d8ed3496  waves/RW-40/G1/task-619-permissions-engine/red.log
f8febb88a18707554285cf158684d203  waves/RW-40/G1/task-619-permissions-engine/green.log
ad011dd9df07de0231870496d476c13a  waves/RW-40/G1/task-619-permissions-engine/mutations.log
```

---

## 5. Риски и замечания

- Крейт `discord-core` не зависит от `serde` (согласно `rust/crates/discord-core/Cargo.toml:10`, где JSON-парсинг выполняется через `discord-json`, а `Cargo.toml` вне `Scope` задачи), поэтому `PermissionOverwrite` реализует стандартные типажи `Clone, Copy, Debug, Default, PartialEq, Eq, Hash` и конверсию `From<&crate::ready::PermissionOverwrite>`.
- Все вычисления выполняются детерминированно за `O(R + W)` без динамических аллокаций и паник.
