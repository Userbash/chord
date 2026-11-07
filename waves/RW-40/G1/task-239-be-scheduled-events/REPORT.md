# Отчёт воркера — task-239: BFF-контракт запланированных событий гильдии (`scheduled_events`)

## 1. Batch / карточка

- **Волна**: `RW-40`, отдел `G1` (Backend).
- **Карточка**: `task-239` (`BE-P1-25` / `scheduled_events`) — BFF-контракт запланированных событий гильдии (`EVENT_ENTITY_*`, `EVENT_STATUS_*`, `MAX_EVENT_*_CHARS`, `CreateScheduledEventRequest`, `validate_scheduled_event_request`, `build_guild_events_route`, `build_event_rsvp_route`).
- **Ветка / worktree**: `feature/task-239-be-scheduled-events`, `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target`.
- **Write-Scope (единственные изменённые файлы)**:
  - `rust/gui-spike/src/scheduled_events.rs`
  - `rust/gui-spike/src/lib.rs`
  - `rust/gui-spike/tests/scheduled_events_contract.rs`
  - `/home/sanya/orca/project/waves/RW-40/G1/task-239-be-scheduled-events/REPORT.md` (и `red.log`, `green.log`)

## 2. Оракул (read-only)

Сверено с `/home/sanya/re-deep/rest-static.json`:

- `GUILD_EVENTS_FOR_GUILD` (`rest-static.json:8232-8243`): `/guilds/{guild_id}/scheduled-events`.
- `GUILD_EVENT` / `USER_GUILD_EVENT` (`rest-static.json:8205-8284`): `/guilds/{guild_id}/scheduled-events/{event_id}` и `/users/@me` RSVP.
- Константы Discord API v9:
  - `EVENT_ENTITY_STAGE = 1`, `EVENT_ENTITY_VOICE = 2`, `EVENT_ENTITY_EXTERNAL = 3`;
  - `EVENT_STATUS_SCHEDULED = 1`, `EVENT_STATUS_ACTIVE = 2`, `EVENT_STATUS_COMPLETED = 3`, `EVENT_STATUS_CANCELED = 4`;
  - `MAX_EVENT_NAME_CHARS = 100`, `MAX_EVENT_DESC_CHARS = 1000`, `MAX_EVENT_LOCATION_CHARS = 100`, `privacy_level = 2` (`GUILD_ONLY`).

## 3. Что сделано

1. **`rust/gui-spike/tests/scheduled_events_contract.rs` (Red-first, 8 тестов)**:
   - Проверка констант `EVENT_ENTITY_STAGE = 1`, `EVENT_ENTITY_VOICE = 2`, `EVENT_ENTITY_EXTERNAL = 3`, `EVENT_STATUS_SCHEDULED = 1`, `EVENT_STATUS_ACTIVE = 2`, `EVENT_STATUS_COMPLETED = 3`, `EVENT_STATUS_CANCELED = 4`, `MAX_EVENT_NAME_CHARS = 100`, `MAX_EVENT_DESC_CHARS = 1000`, `MAX_EVENT_LOCATION_CHARS = 100`.
   - Проверка serde round-trip для `CreateScheduledEventRequest` (для `VOICE` и `EXTERNAL` вариантов).
   - Проверка `validate_scheduled_event_request(req: &CreateScheduledEventRequest) -> Result<(), String>`:
     - длина `name.trim()` в диапазоне `1..=100` символов Unicode;
     - длина `description` при `Some` не более `1000` символов Unicode;
     - `privacy_level == 2` (`GUILD_ONLY`);
     - `scheduled_start_time.trim()` непустой;
     - для `entity_type == 1` (`STAGE`) или `2` (`VOICE`): обязательный `channel_id` (`Some` валидная положительная числовая снежинка `1..=20` цифр без ведущего `'0'`);
     - для `entity_type == 3` (`EXTERNAL`): обязательная `location` (`1..=100` символов Unicode после `trim()`) и обязательное непустое `scheduled_end_time` после `trim()`;
     - отклонение иных значений `entity_type` (`0`, `4`, `5`, `255`).
   - Проверка `build_guild_events_route(guild_id: &str, with_user_count: bool) -> Result<String, String>`:
     - валидация `guild_id` как положительной числовой снежинки (`1..=20` цифр без ведущего `'0'`);
     - возврат `"/api/v9/guilds/{guild_id}/scheduled-events?with_user_count=true"` (и `false`).
   - Проверка `build_event_rsvp_route(guild_id: &str, event_id: &str) -> Result<String, String>`:
     - валидация обоих снежинок (`guild_id`, `event_id`);
     - возврат `"/api/v9/guilds/{guild_id}/scheduled-events/{event_id}/users/@me"`.
2. **`rust/gui-spike/src/scheduled_events.rs`**:
   - Реализованы константы, структура `CreateScheduledEventRequest` и функции `validate_scheduled_event_request`, `build_guild_events_route`, `build_event_rsvp_route`.
3. **`rust/gui-spike/src/lib.rs`**:
   - Добавлен экспорт `pub mod scheduled_events;`.

## 4. RED-прогон

Сохранено в `/home/sanya/orca/project/waves/RW-40/G1/task-239-be-scheduled-events/red.log`:

```text
error[E0432]: unresolved import `discord_gui_spike::scheduled_events`
  --> tests/scheduled_events_contract.rs:24:24
   |
24 | use discord_gui_spike::scheduled_events::{
   |                        ^^^^^^^^^^^^^^^^ could not find `scheduled_events` in `discord_gui_spike`

error: could not compile `discord-gui-spike` (test "scheduled_events_contract") due to 1 previous error
EXIT_CODE=101
```

## 5. GREEN-прогон и проверки (Targeted + Clippy + Rustfmt)

Сохранено в `/home/sanya/orca/project/waves/RW-40/G1/task-239-be-scheduled-events/green.log`:

```text
running 8 tests
test build_event_rsvp_route_validates_both_snowflakes_and_formats_path ... ok
test build_guild_events_route_validates_guild_id_and_formats_query ... ok
test scheduled_events_constants_match_discord_contract ... ok
test validate_scheduled_event_request_enforces_name_and_description_bounds ... ok
test create_scheduled_event_request_serde_roundtrip ... ok
test validate_scheduled_event_request_validates_external_location_and_end_time_and_rejects_unknown_entity ... ok
test validate_scheduled_event_request_enforces_privacy_level_and_start_time ... ok
test validate_scheduled_event_request_validates_stage_and_voice_channel_snowflake ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT_CODE=0
```

Дополнительно проверено:
- `cargo clippy --manifest-path rust/gui-spike/Cargo.toml -p discord-gui-spike --test scheduled_events_contract --lib -- -D warnings` — `0 warnings`, `exit=0`.
- `rustfmt --edition 2021 --check rust/gui-spike/src/scheduled_events.rs rust/gui-spike/tests/scheduled_events_contract.rs` — `exit=0`.

## 6. Файлы + md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/scheduled_events.rs` | `b4c3e2052468f296dad69f0cbe4c0188` |
| `rust/gui-spike/src/lib.rs` | `c3ead90bb6bcabb99d6687bbd1c42343` |
| `rust/gui-spike/tests/scheduled_events_contract.rs` | `2529b0ad5e21883960effd7b0d21c505` |
| `waves/RW-40/G1/task-239-be-scheduled-events/red.log` | `253df1ff5dd42ed8f61bcfc2390ea1d2` |
| `waves/RW-40/G1/task-239-be-scheduled-events/green.log` | `e5c1e2bea8195f2687c672eb3fcd3389` |

## 7. Риски и замечания

- **Дрейф `rust/gui-spike/Cargo.lock`**: при запуске `cargo` перегенерировался `Cargo.lock` из-за `discord-codec` (`zstd`); откатан через `git checkout -- rust/gui-spike/Cargo.lock`, чтобы строго соблюсти write-scope.
- `package-lock.json` был модифицирован в рабочем дереве до старта задачи — не трогался.
- `git commit` не выполнялся.
