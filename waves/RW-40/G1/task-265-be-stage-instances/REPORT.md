# Отчёт воркера — task-265: BFF-контракт трибун и запроса слова (`stage_instances`)

## 1. Batch / карточка

- **Волна**: `RW-40`, отдел `G1` (Backend).
- **Карточка**: `task-265` (`BE-P1-32` / `stage_instances`) — Rust Stage Channel Instances & Speaker Request BFF Contract (`MIN_STAGE_TOPIC_CHARS`, `MAX_STAGE_TOPIC_CHARS`, `STAGE_PRIVACY_GUILD_ONLY`, `CreateStageInstanceRequest`, `UpdateSelfVoiceStageRequest`, `validate_create_stage_instance`, `build_create_stage_instance_route_and_body`, `build_update_self_stage_voice_state_route_and_body`).
- **Ветка / worktree**: `feature/task-265-be-stage-instances`, `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target`.
- **Write-Scope (единственные изменённые файлы)**:
  - `rust/gui-spike/src/stage_instances.rs`
  - `rust/gui-spike/src/lib.rs`
  - `rust/gui-spike/tests/stage_instances_contract.rs`
  - `/home/sanya/orca/project/waves/RW-40/G1/task-265-be-stage-instances/REPORT.md` (и `red.log`, `green.log`)

## 2. Оракул (read-only)

Сверено с `/home/sanya/re-deep/rest-static.json`:

- `STAGE_INSTANCES` (`rest-static.json:8088-8099`): `POST /stage-instances` (`{channel_id,topic,privacy_level,guild_scheduled_event_id,send_start_notification}`).
- `STAGE_INSTANCE` (`rest-static.json:8114-8124`): `DELETE|PATCH /stage-instances/{arg}` (`{topic,privacy_level}`).
- `UPDATE_VOICE_STATE` (`rest-static.json:7915-7928`): `PATCH /guilds/{arg}/voice-states/{arg}` (`{suppress,request_to_speak_timestamp,channel_id}`).
- Константы Discord API v9:
  - `MIN_STAGE_TOPIC_CHARS = 1`, `MAX_STAGE_TOPIC_CHARS = 120`;
  - `STAGE_PRIVACY_GUILD_ONLY = 2` (`GUILD_ONLY`).

## 3. Что сделано

1. **`rust/gui-spike/tests/stage_instances_contract.rs` (Red-first, 7 тестов)**:
   - Проверка констант `MIN_STAGE_TOPIC_CHARS = 1`, `MAX_STAGE_TOPIC_CHARS = 120`, `STAGE_PRIVACY_GUILD_ONLY = 2`.
   - Проверка serde round-trip для `CreateStageInstanceRequest` и `UpdateSelfVoiceStageRequest`.
   - Проверка `validate_create_stage_instance(req: &CreateStageInstanceRequest) -> Result<(), String>`:
     - `req.channel_id` как положительный числовой Snowflake (`1..=20` ASCII-цифр, без ведущего `'0'`);
     - длина `req.topic.trim().chars().count()` в диапазоне `1..=120` символов Unicode;
     - `req.privacy_level == STAGE_PRIVACY_GUILD_ONLY` (`2`).
   - Проверка `build_create_stage_instance_route_and_body(req: &CreateStageInstanceRequest) -> Result<(String, serde_json::Value), String>`:
     - вызов `validate_create_stage_instance(req)` и возврат `("/api/v9/stage-instances", json!({ "channel_id": req.channel_id, "topic": req.topic.trim(), "privacy_level": req.privacy_level, "send_start_notification": req.send_start_notification }))`.
   - Проверка `build_update_self_stage_voice_state_route_and_body(guild_id: &str, req: &UpdateSelfVoiceStageRequest) -> Result<(String, serde_json::Value), String>`:
     - валидация `guild_id` и `req.channel_id` как положительных числовых Snowflake (`1..=20` ASCII-цифр, без ведущего `'0'`);
     - проверка непустого `request_to_speak_timestamp` (`!ts.trim().is_empty()`), если задан `Some(ts)`;
     - возврат `("/api/v9/guilds/{guild_id}/voice-states/@me", json!({ "channel_id": req.channel_id, "suppress": req.suppress, "request_to_speak_timestamp": req.request_to_speak_timestamp }))`.
2. **`rust/gui-spike/src/stage_instances.rs`**:
   - Реализованы константы, структуры `CreateStageInstanceRequest` и `UpdateSelfVoiceStageRequest`, а также функции `validate_create_stage_instance`, `build_create_stage_instance_route_and_body` и `build_update_self_stage_voice_state_route_and_body`.
3. **`rust/gui-spike/src/lib.rs`**:
   - Добавлен экспорт `pub mod stage_instances;`.

## 4. RED-прогон

Сохранено в `/home/sanya/orca/project/waves/RW-40/G1/task-265-be-stage-instances/red.log`:

```text
error[E0432]: unresolved import `discord_gui_spike::stage_instances`
  --> tests/stage_instances_contract.rs:20:24
   |
20 | use discord_gui_spike::stage_instances::{
   |                        ^^^^^^^^^^^^^^^ could not find `stage_instances` in `discord_gui_spike`

error: could not compile `discord-gui-spike` (test "stage_instances_contract") due to 1 previous error
EXIT_CODE=101
```

## 5. GREEN-прогон и проверки (Targeted + Clippy + Rustfmt)

Сохранено в `/home/sanya/orca/project/waves/RW-40/G1/task-265-be-stage-instances/green.log`:

```text
running 7 tests
test stage_instances_constants_match_discord_contract ... ok
test build_create_stage_instance_route_and_body_validates_and_trims_topic ... ok
test build_update_self_stage_voice_state_route_and_body_validates_snowflakes_and_timestamp ... ok
test validate_create_stage_instance_enforces_channel_id_snowflake ... ok
test validate_create_stage_instance_enforces_guild_only_privacy_level ... ok
test stage_instances_request_structs_serde_roundtrip ... ok
test validate_create_stage_instance_enforces_trimmed_unicode_topic_bounds ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

EXIT_CODE=0
```

Дополнительно проверено:
- `cargo clippy --manifest-path rust/gui-spike/Cargo.toml -p discord-gui-spike --test stage_instances_contract --lib -- -D warnings` — `0 warnings`, `exit=0`.
- `rustfmt --edition 2021 --check rust/gui-spike/src/stage_instances.rs rust/gui-spike/tests/stage_instances_contract.rs` — `exit=0`.

## 6. Файлы + md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/stage_instances.rs` | `9ffbfacca407637196ab0ffc9da44dfc` |
| `rust/gui-spike/src/lib.rs` | `dea23457b1361d1e821fe733fd7ebc2f` |
| `rust/gui-spike/tests/stage_instances_contract.rs` | `6bf3197d64d69726d79c0228e2391543` |
| `waves/RW-40/G1/task-265-be-stage-instances/red.log` | `0a67f103e508211cf2e54e59f9b1fe89` |
| `waves/RW-40/G1/task-265-be-stage-instances/green.log` | `7769613be875a5b1edef98f6d17dfdf9` |

## 7. Риски и замечания

- **Дрейф `rust/gui-spike/Cargo.lock`**: при запуске `cargo` перегенерировался `Cargo.lock` из-за `discord-codec` (`zstd`); откатан через `git checkout -- rust/gui-spike/Cargo.lock`, чтобы строго соблюсти write-scope.
- `package-lock.json` был модифицирован в рабочем дереве до старта задачи — не трогался.
- `git commit` не выполнялся.
