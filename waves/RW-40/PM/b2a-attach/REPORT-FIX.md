# B2a-ATTACH-TIER-FIX — отчёт воркера (закрытие P1 из REVIEW.md)

- **batch/карточка:** RW-40 / B2a-ATTACH-TIER-FIX, отдел G1 GUI-Spike Rust Backend.
- **основание:** `waves/RW-40/PM/b2a-attach/REVIEW.md` (P1 — жёсткий лимит 10 MiB без учёта guild boost tier отвергает валидные файлы для Tier 2 = 50 MiB и Tier 3 = 100 MiB).
- **сделано:** в `rust/gui-spike/src/attachments.rs` добавлены тип `GuildBoostTier` (`None`, `Tier1`, `Tier2`, `Tier3`) с методом `max_upload_bytes()`, константы `MAX_UPLOAD_FILE_BYTES_TIER2` (50 MiB = `0x3200000`) и `MAX_UPLOAD_FILE_BYTES_TIER3` (100 MiB = `0x6400000`), свободная функция `request_upload_slots_with_tier(&transport, channel_id, tier, files)` и метод `DiscordAttachments::request_upload_slots_with_tier(&self, channel_id, tier, files)`. Существующий `request_upload_slots` делегирует в `request_upload_slots_with_tier(..., GuildBoostTier::None, ...)`, сохраняя обратную совместимость и все 15 исходных тестов зелёными без ослабления. В `rust/gui-spike/tests/attachments_contract.rs` добавлены 2 новых контрактных теста на константы и границы каждого tier.

## Что реализовано

| Элемент | Контракт (RE) |
|---|---|
| `MAX_UPLOAD_FILE_BYTES = 10 * 1024 * 1024` | reference-oracle-REA §1.5 (`NONE` / `TIER_1` = `0xa00000`) |
| `MAX_UPLOAD_FILE_BYTES_TIER2 = 50 * 1024 * 1024` | reference-oracle-REA §1.5 (`TIER_2` = `0x3200000`) |
| `MAX_UPLOAD_FILE_BYTES_TIER3 = 100 * 1024 * 1024` | reference-oracle-REA §1.5 (`TIER_3` = `0x6400000`) |
| `GuildBoostTier::{None, Tier1, Tier2, Tier3}::max_upload_bytes()` | `None`/`Tier1` → 10 MiB, `Tier2` → 50 MiB, `Tier3` → 100 MiB |
| `request_upload_slots_with_tier(&transport, channel_id, tier, files)` + метод `DiscordAttachments` | Предпроверка размера по `tier.max_upload_bytes()` до обращения к сети |
| `request_upload_slots(&transport, channel_id, files)` | Вызов `request_upload_slots_with_tier` с `GuildBoostTier::None` |

## red

```
(cd rust/gui-spike && CARGO_TARGET_DIR=/tmp/target-g1-attach cargo test --offline -p discord-gui-spike --test attachments_contract)
→ test result: FAILED. 15 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
→ EXIT_CODE=101
```

Первая ошибка в `waves/RW-40/PM/b2a-attach/red-fix.log`:
- `guild_boost_tier_limits_match_re_oracle`: `assertion left == right failed: left: 0, right: 10485760`
- `enforces_exact_upload_size_boundaries_for_each_guild_boost_tier`: `tier=None на границе 10485760 обязан проходить: InvalidResponse`
Все 15 ранее существовавших тестов оставались зелёными.

## green

```
CARGO_TARGET_DIR=/tmp/target-g1-attach cargo test --offline -p discord-gui-spike --test attachments_contract → ok. 17 passed; 0 failed (exit 0)
CARGO_TARGET_DIR=/tmp/target-g1-attach cargo test --offline -p discord-gui-spike --lib attachments           → ok. 2 passed; 0 failed  (exit 0)
cargo fmt --all -- --check                                                                                   → exit 0
CARGO_TARGET_DIR=/tmp/target-g1-attach cargo clippy --offline -p discord-gui-spike --lib --test attachments_contract -- -D warnings → exit 0
```

Лог: `waves/RW-40/PM/b2a-attach/green-fix.log` (снят после восстановления мутаций на финальных байтах).

## targeted

`rust/gui-spike/tests/attachments_contract.rs` — 17 тестов (15 исходных без изменений + 2 новых) и 2 unit-теста в `src/attachments.rs`:

| Test ID | Утверждение |
|---|---|
| `guild_boost_tier_limits_match_re_oracle` | `0xa00000` (10 MiB), `0x3200000` (50 MiB), `0x6400000` (100 MiB), `Default` = `None`, `max_upload_bytes()` по каждому tier |
| `enforces_exact_upload_size_boundaries_for_each_guild_boost_tier` | Для `None`, `Tier1`, `Tier2`, `Tier3`: размер `== limit` проходит (1 POST), `limit + 1` → `Err(AttachmentError::EntityTooLarge)` без вызовов транспорта (`calls().is_empty()`); проверка метода `DiscordAttachments::request_upload_slots_with_tier` на `10 MiB + 1` с `Tier2` |

## oracle

- `waves/RW-40/reference-oracle-REA.md` §1.5 (`[D] @314301`, `@6804500`, `@6805978`):
  - `NONE` / `TIER_1` = `10 МиБ` (`0xa00000` = `10_485_760`);
  - `TIER_2` = `50 МиБ` (`0x3200000` = `52_428_800`);
  - `TIER_3` = `100 МиБ` (`0x6400000` = `104_857_600`).

## мутации

4/4 KILLED на границах Tier 2 / Tier 3 (`waves/RW-40/PM/b2a-attach/mutations-fix.log`):

| Мутант | md5 мутанта | Убит тестами |
|---|---|---|
| M1 `Tier2` возвращает `MAX_UPLOAD_FILE_BYTES` (10 MiB) вместо 50 MiB | `d2786e371102aee16757be28a297183f` | `enforces_exact_upload_size_boundaries_for_each_guild_boost_tier`, `guild_boost_tier_limits_match_re_oracle` (exit 101) |
| M2 `Tier3` возвращает `MAX_UPLOAD_FILE_BYTES_TIER2` (50 MiB) вместо 100 MiB | `4a2ce9580a9f1258f374ff7590b2992b` | `enforces_exact_upload_size_boundaries_for_each_guild_boost_tier`, `guild_boost_tier_limits_match_re_oracle` (exit 101) |
| M3 Граница размера в `validate_files`: `>=` вместо `>` (режет точный размер Tier2/Tier3) | `94c5b7dd671cf1993e8cbd24e4536459` | `enforces_exact_upload_size_boundaries_for_each_guild_boost_tier`, `rejects_oversized_file_locally_with_entity_too_large` (exit 101) |
| M4 `validate_files` игнорирует `tier` и проверяет жёсткий `MAX_UPLOAD_FILE_BYTES` | `04b521c81580c31aa032f08f37592e30` | `enforces_exact_upload_size_boundaries_for_each_guild_boost_tier` (exit 101) |

## baseline/restore

- baseline до мутаций:
  - `rust/gui-spike/src/attachments.rs` = `e28b8cbc5dfb2db2e14aecabbd61caf2`
  - `rust/gui-spike/tests/attachments_contract.rs` = `8563afd362e5e3ba7f66b4f2e3704c02`
- после каждого мутанта выполнен restore и проверено совпадение md5 (`e28b8cbc5dfb2db2e14aecabbd61caf2`).

## файлы

| Файл | Старый md5 (REPORT.md) | Новый md5 |
|---|---|---|
| `rust/gui-spike/src/attachments.rs` | `c2e1062287d334a397901a7ebe90b3a3` | `e28b8cbc5dfb2db2e14aecabbd61caf2` |
| `rust/gui-spike/tests/attachments_contract.rs` | `cdc8cbd9dd91323ad97a84ef4acfa962` | `8563afd362e5e3ba7f66b4f2e3704c02` |
| `rust/gui-spike/src/lib.rs` (не тронут) | `f8b5af6132cba13213d3a79031b8f976` | `f8b5af6132cba13213d3a79031b8f976` |
| `waves/RW-40/PM/b2a-attach/red-fix.log` | — | `07e29db5fb0e12feb2e341dac1360fa9` |
| `waves/RW-40/PM/b2a-attach/green-fix.log` | — | `1bb3434fb2bb99cde675663a9d31b4c5` |
| `waves/RW-40/PM/b2a-attach/mutations-fix.log` | — | `6165c47a464399b8a01223af58dd0bd9` |

## риски/замечания

- Чужие файлы не затрагивались; все сборки и тесты выполнялись строго с `CARGO_TARGET_DIR=/tmp/target-g1-attach`.
- Примечание по `clippy --all-targets`: в чужом тесте `tests/net_contract.rs:782` присутствует предупреждение `clippy::type_complexity`; целевые таргеты карточки (`--lib --test attachments_contract`) проходят `clippy -D warnings` с `exit=0`.
