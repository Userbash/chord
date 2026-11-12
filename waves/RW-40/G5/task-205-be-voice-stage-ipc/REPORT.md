# Отчёт воркера — task-205: BFF голосовой сцены и запуска демонстрации экрана (`voice_stage`)

## 1. Batch / карточка

- **Волна**: `RW-40`, отдел `G5` (Voice / Platform).
- **Карточка**: `task-205` — BFF голосовой сцены и IPC-контракт демонстрации экрана (`MAX_STAGE_VISIBLE_TILES`, `StreamResolution`, `StreamFps`, `ScreenShareSpec`, `build_gateway_stream_create_op18`, `build_gateway_stream_delete_op19`).
- **Ветка / worktree**: `feature/task-205-be-voice-stage-ipc`, `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`.
- **Write-Scope (единственные изменённые файлы)**:
  - `rust/gui-spike/src/voice_stage.rs`
  - `rust/gui-spike/src/lib.rs`
  - `rust/gui-spike/tests/voice_stage_contract.rs`
  - `/home/user/orca/project/waves/RW-40/G5/task-205-be-voice-stage-ipc/REPORT.md` (а также `red.log` и `green.log`)

## 2. Оракул (read-only)

Сверено с `reference-oracle/decoded/4d2a64cb5df5fc4e_0`, `waves/RW-40/VOICE/STREAM-GATEWAY-CONTRACT.md` (`A-OPCODES`, `A-SENDERS`, `A-KEY-PARSE`, `A-KEY-BUILD`, `D-27-2`), `waves/RW-40/VOICE/stream-opcodes-fixtures.json` и `rust/gui-spike/ui/features/voice-grid.js`:

- **`MAX_STAGE_VISIBLE_TILES`**: `25` (сетка участников голосовой сцены до `5x5`).
- **`A-KEY-BUILD` (`@5089954`) и `A-KEY-PARSE` (`@5089698`)**:
  - гильдия: `guild:{guild_id}:{channel_id}:{user_id}` (4 сегмента);
  - звонок/ЛС: `call:{channel_id}:{user_id}` (3 сегмента);
  - все Snowflake-сегменты — непустые положительные десятичные `u64` без ведущих нулей (`1..=u64::MAX`).
- **`op 18 STREAM_CREATE` (`A-OPCODES @4996751`, `A-SENDERS @4998992`, `D-27-2`)**:
  - для гильдии: `{"op": 18, "d": {"type": "guild", "guild_id": "<gid>", "channel_id": "<cid>", "preferred_region": null}}`;
  - для звонка: `{"op": 18, "d": {"type": "call", "guild_id": null, "channel_id": "<cid>", "preferred_region": null}}`.
- **`op 19 STREAM_DELETE` (`A-OPCODES @4996751`, `A-SENDERS @4999270`)**:
  - `{"op": 19, "d": {"stream_key": "<stream_key>"}}`.

## 3. Что сделано

1. **`rust/gui-spike/tests/voice_stage_contract.rs` (Red-first, 8 контрактных тестов)**:
   - Проверка `MAX_STAGE_VISIBLE_TILES == 25`, хелперов `StreamResolution` (`P720` -> `1280x720`, `P1080` -> `1920x1080`, `P1440` -> `2560x1440`, `width()`, `height()`, `dimensions()`, `label()`) и `StreamFps` (`Fps30` -> `30`, `Fps60` -> `60`, `fps()`) + serde round-trip.
   - Позитивные, граничные (`"1"`, `u64::MAX`) и негативные (`""`, `"0"`, `"0000"`, `"012345"`, не-цифры, `u64::MAX + 1`, пустой/пробельный `source_id`) сценарии `ScreenShareSpec::validate(&self)`.
   - Проверка `ScreenShareSpec::to_stream_Key(&self, user_id)` и `to_stream_key(&self, user_id)` для гильдии и звонка.
   - Проверка `build_gateway_stream_create_op18` и `build_gateway_stream_delete_op19` на точное соответствие JSON-кадрам оракула и отказ на невалидных спецификациях / `stream_key`.
2. **`rust/gui-spike/src/voice_stage.rs`**:
   - Реализованы `MAX_STAGE_VISIBLE_TILES`, `StreamResolution`, `StreamFps`, `ScreenShareSpec` (`validate`, `to_stream_key`, `to_stream_Key`), `build_gateway_stream_create_op18` и `build_gateway_stream_delete_op19`.
3. **`rust/gui-spike/src/lib.rs`**:
   - Экспортирован модуль `pub mod voice_stage;`.

## 4. RED-прогон

Сохранено в `/home/user/orca/project/waves/RW-40/G5/task-205-be-voice-stage-ipc/red.log`:

```text
error[E0432]: unresolved import `discord_gui_spike::voice_stage`
  --> tests/voice_stage_contract.rs:18:24
   |
18 | use discord_gui_spike::voice_stage::{
   |                        ^^^^^^^^^^^ could not find `voice_stage` in `discord_gui_spike`

error: could not compile `discord-gui-spike` (test "voice_stage_contract") due to 1 previous error
EXIT_CODE=101
```

## 5. GREEN-прогон и проверки (Targeted + Clippy + Rustfmt)

Сохранено в `/home/user/orca/project/waves/RW-40/G5/task-205-be-voice-stage-ipc/green.log`:

```text
running 8 tests
test build_gateway_stream_create_op18_rejects_invalid_spec ... ok
test build_gateway_stream_create_op18_produces_exact_oracle_payloads_for_guild_and_call ... ok
test max_stage_visible_tiles_and_resolution_fps_helpers_match_spec ... ok
test to_stream_key_builds_canonical_guild_and_call_keys ... ok
test screen_share_spec_validate_rejects_invalid_snowflakes_and_empty_source_id ... ok
test screen_share_spec_validate_accepts_valid_guild_and_call_specs_including_boundaries ... ok
test build_gateway_stream_delete_op19_produces_exact_oracle_payloads_and_validates_stream_key ... ok
test resolution_and_fps_serde_roundtrip ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

EXIT_CODE=0
```

Дополнительно проверено:
- `cargo clippy -p discord-gui-spike --test voice_stage_contract --lib -- -D warnings` — `0 warnings`, `exit=0`.
- `rustfmt --edition 2021 --check rust/gui-spike/src/voice_stage.rs rust/gui-spike/tests/voice_stage_contract.rs` — `exit=0`.
- `rustfmt --edition 2021 --check --config skip_children=true rust/gui-spike/src/lib.rs` — `exit=0`.

## 6. Файлы + md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/voice_stage.rs` | `9e141b3332ebcc2551eb824e8d905bf5` |
| `rust/gui-spike/src/lib.rs` | `3e71e98879c1eebbd6563a97c1db699e` |
| `rust/gui-spike/tests/voice_stage_contract.rs` | `a7c025f9dc95b0a89c63c0149158bd09` |
| `waves/RW-40/G5/task-205-be-voice-stage-ipc/red.log` | `f373c88c56ad8a2a5b864073853ca26d` |
| `waves/RW-40/G5/task-205-be-voice-stage-ipc/green.log` | `bf61c35ce70ef81191c99d76cefc0b34` |

## 7. Риски и замечания

- Для полной совместимости с регистром имени метода в ТЗ (`to_stream_Key`) в `ScreenShareSpec` предоставлены оба метода: `to_stream_Key(&self, user_id: &str) -> String` (`#[allow(non_snake_case)]`) и канонический `to_stream_key(&self, user_id: &str) -> String`.
- Дрейф `rust/gui-spike/Cargo.lock` при запуске `cargo` откатан через `git checkout -- rust/gui-spike/Cargo.lock`.
- `package-lock.json` был модифицирован в рабочем дереве до старта задачи — не трогался.
- `git commit` не выполнялся.
