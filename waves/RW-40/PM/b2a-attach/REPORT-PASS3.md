# B2a-ATTACH-TIER-FIX-PASS2 — отчёт воркера (pass 3, закрытие P1 из REVIEW-PASS2.md)

- **batch/карточка:** RW-40 / Wave 7 / `B2a-ATTACH-TIER-FIX-PASS2`, отдел G1 Rust Backend.
- **основание:** `waves/RW-40/PM/b2a-attach/REVIEW-PASS2.md` (P1 — по независимому оракулу `waves/RW-40/reference-oracle-REA.md:122-125` формула лимита файла равна `Math.max(0x1400000, tierLimit)`, то есть базовый пол для `None` и `Tier1` равен 20 MiB (`0x1400000` = `20 * 1024 * 1024` = `20_971_520` байт), для `Tier2` — 50 MiB (`0x3200000` = `52_428_800` байт), для `Tier3` — 100 MiB (`0x6400000` = `104_857_600` байт)).
- **сделано:**
  1. В `rust/gui-spike/tests/attachments_contract.rs` обновлены контрактные тесты `oracle_constants_match_re`, `guild_boost_tier_limits_match_re_oracle` и `enforces_exact_upload_size_boundaries_for_each_guild_boost_tier` на базовый лимит 20 MiB (`0x140_0000` = `20_971_520`) для `MAX_UPLOAD_FILE_BYTES`, `GuildBoostTier::None` и `GuildBoostTier::Tier1`, а также добавлена явная проверка прохождения файлов в диапазоне `(10 MiB + 1 ..= 20 MiB)` (`10_485_761`, `15_728_640`, `20_971_520` байт) и отсечения `20 MiB + 1` (`20_971_521` байт) до обращения к сети.
  2. Зафиксирован RED-прогон до изменения реализации в `waves/RW-40/PM/b2a-attach/red-pass3.log` (`14 passed; 3 failed`, `EXIT_CODE=101`).
  3. В `rust/gui-spike/src/attachments.rs` базовый лимит `MAX_UPLOAD_FILE_BYTES` (и возврат `GuildBoostTier::{None, Tier1}::max_upload_bytes()`) приведён к `20 * 1024 * 1024` (`0x1400000` = `20_971_520` байт) согласно формуле `Math.max(0x1400000, tierLimit)`.
  4. Выполнен прогон 4 мутаций с проверкой восстановления `md5` (`waves/RW-40/PM/b2a-attach/mutations-pass3.log`, 4/4 KILLED) и финальный GREEN-прогон (`waves/RW-40/PM/b2a-attach/green-pass3.log`).

## Что реализовано

| Элемент | Контракт (RE) |
|---|---|
| `MAX_UPLOAD_FILE_BYTES = 20 * 1024 * 1024` (`0x1400000` = `20_971_520`) | `reference-oracle-REA.md:122-123` (`Math.max(0x1400000, tierLimit)` для `NONE` / `TIER_1`) |
| `MAX_UPLOAD_FILE_BYTES_TIER2 = 50 * 1024 * 1024` (`0x3200000` = `52_428_800`) | `reference-oracle-REA.md:124` (`TIER_2` = `0x3200000`) |
| `MAX_UPLOAD_FILE_BYTES_TIER3 = 100 * 1024 * 1024` (`0x6400000` = `104_857_600`) | `reference-oracle-REA.md:125` (`TIER_3` = `0x6400000`) |
| `GuildBoostTier::{None, Tier1, Tier2, Tier3}::max_upload_bytes()` | `None`/`Tier1` → 20 MiB (`20_971_520`), `Tier2` → 50 MiB (`52_428_800`), `Tier3` → 100 MiB (`104_857_600`) |

## red

```bash
(cd rust/gui-spike && CARGO_TARGET_DIR=/tmp/target-g1-b2a2 cargo test --offline -p discord-gui-spike --test attachments_contract) > waves/RW-40/PM/b2a-attach/red-pass3.log 2>&1
→ test result: FAILED. 14 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
→ EXIT_CODE=101
```

Падения в `waves/RW-40/PM/b2a-attach/red-pass3.log`:
- `oracle_constants_match_re`: `assertion left == right failed: left: 10485760, right: 20971520`
- `guild_boost_tier_limits_match_re_oracle`: `assertion left == right failed: left: 10485760, right: 20971520`
- `enforces_exact_upload_size_boundaries_for_each_guild_boost_tier`: `базовый вызов обязан пропускать размер 10485761 (<= 20 MiB)`

## green

```bash
CARGO_TARGET_DIR=/tmp/target-g1-b2a2 cargo test --offline -p discord-gui-spike --test attachments_contract → ok. 17 passed; 0 failed (exit=0)
CARGO_TARGET_DIR=/tmp/target-g1-b2a2 cargo test --offline -p discord-gui-spike --lib attachments           → ok. 2 passed; 0 failed  (exit=0)
rustfmt --edition 2021 --check src/attachments.rs tests/attachments_contract.rs                            → exit=0
CARGO_TARGET_DIR=/tmp/target-g1-b2a2 cargo clippy --offline -p discord-gui-spike --lib --test attachments_contract -- -D warnings → exit=0
```

Лог: `waves/RW-40/PM/b2a-attach/green-pass3.log` (снят после восстановления мутаций на финальных байтах).

## targeted

`rust/gui-spike/tests/attachments_contract.rs` — 17 контрактных тестов + 2 unit-теста в `src/attachments.rs`:

| Test ID | Утверждение |
|---|---|
| `oracle_constants_match_re` | `MAX_UPLOAD_FILE_BYTES == 20 * 1024 * 1024` (`20_971_520`), `MAX_ATTACHMENTS_PER_MESSAGE == 10`, шаблон пути и ключи JSON |
| `guild_boost_tier_limits_match_re_oracle` | `0x140_0000` (`20_971_520`), `0x320_0000` (`52_428_800`), `0x640_0000` (`104_857_600`), `Default` = `None`, `max_upload_bytes()` для `None`/`Tier1`/`Tier2`/`Tier3` |
| `enforces_exact_upload_size_boundaries_for_each_guild_boost_tier` | Размеры `10 MiB + 1` (`10_485_761`), `15 MiB` и `20 MiB` (`20_971_520`) проходят для базового вызова, `None` и `Tier1`; для каждого `None`/`Tier1`/`Tier2`/`Tier3` размер `== limit` проходит (1 POST), `limit + 1` возвращает `Err(AttachmentError::EntityTooLarge)` без вызовов транспорта (`calls().is_empty()`); `20 MiB + 1` отклоняется на базовом/Tier1 и проходит на `Tier2` |
| `rejects_oversized_file_locally_with_entity_too_large` | `MAX_UPLOAD_FILE_BYTES + 1` (`20_971_521`) → `EntityTooLarge` до сети, ровно `MAX_UPLOAD_FILE_BYTES` (`20_971_520`) → `Ok` |

## oracle

- `waves/RW-40/reference-oracle-REA.md:122-125` (`[D] 4d2a64cb5df5fc4e_0@6011428`, `@314301`, `@6804500`, `@6805978`):
  - Базовый лимит на файл: `Math.max(0x1400000, tierLimit)` = не ниже **20 МиБ** (`0x1400000` = `20_971_520` байт);
  - Для `NONE` / `TIER_1` (`tierLimit = 0xa00000` = 10 МиБ): `max(0x1400000, 0xa00000) = 0x1400000` (20 МиБ);
  - Для `TIER_2`: `0x3200000` = 50 МиБ (`52_428_800` байт);
  - Для `TIER_3`: `0x6400000` = 100 МиБ (`104_857_600` байт).

## мутации

4/4 KILLED с `CARGO_TARGET_DIR=/tmp/target-g1-b2a2 --offline` (`waves/RW-40/PM/b2a-attach/mutations-pass3.log`):

| Мутант | md5 мутанта | Результат | Убит тестами |
|---|---|---|---|
| M1 `None`/`Tier1` возвращает `10 * 1024 * 1024` (`0xa0_0000`) вместо базового пола 20 MiB (`Math.max(0x1400000, tierLimit)`) | `6c8aeb22e0d7530ddd8325ae74843674` | KILLED (14 passed; 3 failed, exit 101) | `enforces_exact_upload_size_boundaries_for_each_guild_boost_tier`, `guild_boost_tier_limits_match_re_oracle`, `rejects_oversized_file_locally_with_entity_too_large` |
| M2 `Tier2` возвращает базовый лимит 20 MiB вместо 50 MiB | `02cda1732100e40623002317ca8d05e6` | KILLED (15 passed; 2 failed, exit 101) | `enforces_exact_upload_size_boundaries_for_each_guild_boost_tier`, `guild_boost_tier_limits_match_re_oracle` |
| M3 `Tier3` возвращает лимит `Tier2` (50 MiB) вместо 100 MiB | `e176a6552549808677acb378c14ee34a` | KILLED (15 passed; 2 failed, exit 101) | `enforces_exact_upload_size_boundaries_for_each_guild_boost_tier`, `guild_boost_tier_limits_match_re_oracle` |
| M4 Граница размера в `validate_files`: `>=` вместо `>` (режет точный размер 20/50/100 MiB) | `ab153d43735e6d2a7f2e5298861d2c31` | KILLED (15 passed; 2 failed, exit 101) | `enforces_exact_upload_size_boundaries_for_each_guild_boost_tier`, `rejects_oversized_file_locally_with_entity_too_large` |

## baseline/restore

- baseline до мутаций:
  - `rust/gui-spike/src/attachments.rs` = `3377d6dcd0850da6b92dbaac3a6c7dac`
  - `rust/gui-spike/tests/attachments_contract.rs` = `45d107db41d4c1f443de8596b1196e49`
- после каждого мутанта выполнен restore и проверено точное совпадение `md5` (`3377d6dcd0850da6b92dbaac3a6c7dac`).

## файлы

| Файл | Старый md5 (pass 2) | Новый md5 (pass 3) |
|---|---|---|
| `rust/gui-spike/src/attachments.rs` | `e28b8cbc5dfb2db2e14aecabbd61caf2` | `3377d6dcd0850da6b92dbaac3a6c7dac` |
| `rust/gui-spike/tests/attachments_contract.rs` | `8563afd362e5e3ba7f66b4f2e3704c02` | `45d107db41d4c1f443de8596b1196e49` |
| `waves/RW-40/PM/b2a-attach/red-pass3.log` | — | `263ad3ab737c0c258a20e75ece4281fc` |
| `waves/RW-40/PM/b2a-attach/green-pass3.log` | — | `271d99f11d8c5ea052ded2c3f92837fc` |
| `waves/RW-40/PM/b2a-attach/mutations-pass3.log` | — | `d4a6ec635ed47ac0ef79479953618b7b` |

## риски/границы

- Чужие файлы не затрагивались; все запуски выполнялись с `CARGO_TARGET_DIR=/tmp/target-g1-b2a2 --offline`.
- `git commit/push` не выполнялся.
