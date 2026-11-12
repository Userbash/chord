# Отчёт воркера — task-237: BFF-контракт журнала аудита, кика, бана и таймаута модерации гильдии (`moderation`)

## 1. Batch / карточка

- **Волна**: `RW-40`, отдел `G1` (Backend).
- **Карточка**: `task-237` (`BE-P1-24` / `moderation`) — Rust Guild Audit Log, Kick, Ban & Timeout Moderation BFF Contract (`BanMemberRequest`, `validate_audit_reason`, `build_ban_member_route_and_body`, `build_audit_log_query_route`).
- **Ветка / worktree**: `feature/task-237-be-moderation`, `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`.
- **Write-Scope (единственные изменённые файлы)**:
  - `rust/gui-spike/src/moderation.rs`
  - `rust/gui-spike/src/lib.rs`
  - `rust/gui-spike/tests/moderation_contract.rs`
  - `/home/user/orca/project/waves/RW-40/G1/task-237-be-moderation/REPORT.md` (и `red.log`, `green.log`)

## 2. Оракул (read-only)

Сверено с `reference-oracle/rest-static.json`:

- `GUILD_BAN` (`rest-static.json:871-877`): `PUT|DELETE /guilds/{guild_id}/bans/{user_id}`.
- `GUILD_AUDIT_LOG` (`rest-static.json:1240-1246`): `GET /guilds/{guild_id}/audit-logs`.
- Константы Discord API v9:
  - `MAX_AUDIT_REASON_CHARS = 512` (символы Unicode в заголовке `X-Audit-Log-Reason`);
  - `MAX_BAN_DELETE_MESSAGE_SECONDS = 604_800` (7 суток в секундах);
  - `MAX_TIMEOUT_SECONDS = 2_419_200` (28 суток в секундах);
  - `MAX_AUDIT_LOG_LIMIT = 100` (верхняя граница параметра `limit` `1..=100`).

## 3. Что сделано

1. **`rust/gui-spike/tests/moderation_contract.rs` (Red-first, 8 тестов)**:
   - Проверка констант `MAX_AUDIT_REASON_CHARS = 512`, `MAX_BAN_DELETE_MESSAGE_SECONDS = 604_800`, `MAX_TIMEOUT_SECONDS = 2_419_200`, `MAX_AUDIT_LOG_LIMIT = 100`.
   - Проверка serde round-trip для `BanMemberRequest`.
   - Проверка `validate_audit_reason(reason: Option<&str>)`:
     - `None` и пробельные/пустые строки `Some("")`, `Some("   ")`, `Some("\t\n\r ")` нормализуются в `Ok(None)`;
     - непустые строки триммятся (`Ok(Some(trimmed.to_string()))`);
     - граница `512` символов Unicode (ASCII и кириллица `512` символов — `Ok`, `513` символов — `Err`).
   - Проверка `build_ban_member_route_and_body(req: &BanMemberRequest)`:
     - валидация `guild_id` и `user_id` как канонических положительных `u64` Snowflake ID (`1..=u64::MAX`, без ведущих нулей);
     - проверка `delete_message_seconds <= 604_800` (`0`, `86_400`, `604_800` — `Ok`; `604_801`, `1_000_000`, `u32::MAX` — `Err`);
     - валидация `reason` через `validate_audit_reason`;
     - возврат `("/guilds/{guild_id}/bans/{user_id}", json!({"delete_message_seconds": ...}), clean_reason)`.
   - Проверка `build_audit_log_query_route(guild_id, action_type, limit)`:
     - валидация `guild_id` как канонического положительного `u64` Snowflake ID;
     - проверка `1 <= limit && limit <= 100` (`0`, `101`, `u8::MAX` — `Err`);
     - построение `"/guilds/{guild_id}/audit-logs?limit={limit}"` и `"/guilds/{guild_id}/audit-logs?limit={limit}&action_type={at}"`.
2. **`rust/gui-spike/src/moderation.rs`**:
   - Реализованы константы `MAX_AUDIT_REASON_CHARS`, `MAX_BAN_DELETE_MESSAGE_SECONDS`, `MAX_TIMEOUT_SECONDS`, `MAX_AUDIT_LOG_LIMIT`.
   - Реализована структура `BanMemberRequest`.
   - Реализованы функции `validate_audit_reason`, `build_ban_member_route_and_body`, `build_audit_log_query_route`.
3. **`rust/gui-spike/src/lib.rs`**:
   - Экспортирован модуль `pub mod moderation;`.

## 4. RED-прогон

Сохранено в `/home/user/orca/project/waves/RW-40/G1/task-237-be-moderation/red.log`:

```text
error[E0432]: unresolved import `discord_gui_spike::moderation`
  --> tests/moderation_contract.rs:27:24
   |
27 | use discord_gui_spike::moderation::{
   |                        ^^^^^^^^^^ could not find `moderation` in `discord_gui_spike`

error: could not compile `discord-gui-spike` (test "moderation_contract") due to 1 previous error
EXIT_CODE=101
```

## 5. GREEN-прогон и проверки (Targeted + Clippy + Rustfmt)

Сохранено в `/home/user/orca/project/waves/RW-40/G1/task-237-be-moderation/green.log`:

```text
running 8 tests
test build_audit_log_query_route_formats_query_with_and_without_action_type ... ok
test ban_member_request_serde_roundtrip ... ok
test build_audit_log_query_route_rejects_invalid_guild_id_and_out_of_range_limit ... ok
test validate_audit_reason_enforces_512_unicode_char_limit ... ok
test validate_audit_reason_handles_none_blank_and_trims_valid_strings ... ok
test moderation_constants_match_discord_contract ... ok
test build_ban_member_route_and_body_rejects_invalid_snowflakes_seconds_and_reason ... ok
test build_ban_member_route_and_body_returns_canonical_route_json_and_clean_reason ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT_CODE=0
```

Дополнительно проверено:
- `cargo clippy --manifest-path rust/gui-spike/Cargo.toml -p discord-gui-spike --test moderation_contract --lib -- -D warnings` — `0 warnings`, `exit=0`.
- `rustfmt --edition 2021 --check rust/gui-spike/src/moderation.rs rust/gui-spike/tests/moderation_contract.rs` — `exit=0`.

## 6. Файлы + md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/moderation.rs` | `b9a7b9c21dcbe962b99b7bad25535c66` |
| `rust/gui-spike/src/lib.rs` | `7d289effe294723afc0f5ca2819cd39e` |
| `rust/gui-spike/tests/moderation_contract.rs` | `42d556f60abca54306614c8cc1543cde` |
| `waves/RW-40/G1/task-237-be-moderation/red.log` | `c0e5892251dc2072bf928c0b74b36f5f` |
| `waves/RW-40/G1/task-237-be-moderation/green.log` | `9bea405e74ff95b4aa47c8e418e881f2` |

## 7. Риски и замечания

- **Дрейф `rust/gui-spike/Cargo.lock`**: откатан через `git checkout -- rust/gui-spike/Cargo.lock`, чтобы строго соблюсти write-scope.
- `package-lock.json` был модифицирован в рабочем дереве до старта задачи — не трогался.
- `git commit` не выполнялся.
