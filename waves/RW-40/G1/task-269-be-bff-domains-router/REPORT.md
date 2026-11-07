# Отчёт воркера — task-269: Unified BFF Domain Action Dispatcher (`bff_domains_router`)

## 1. Batch / карточка

- **Волна**: `RW-40`, отдел `G1` (Backend).
- **Карточка**: `task-269` (`BE-INT-12` / `bff_domains_router`) — единый типизированный роутер команд BFF для доменов `slash_commands`, `onboarding`, `soundboard` и `stage_instances` (`SUPPORTED_BFF_DOMAINS`, `BffDomainAction`, `UpdateGuildOnboardingRequest`, `is_supported_bff_domain`, `dispatch_bff_domain_action`).
- **Ветка / worktree**: `feature/task-269-be-bff-domains-router`, `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target`.
- **Write-Scope (единственные изменённые файлы)**:
  - `rust/gui-spike/src/bff_domains_router.rs`
  - `rust/gui-spike/src/lib.rs`
  - `rust/gui-spike/tests/bff_domains_router_contract.rs`
  - `/home/sanya/orca/project/waves/RW-40/G1/task-269-be-bff-domains-router/REPORT.md` (и `red.log`, `green.log`)

## 2. Согласование контракта и оракулов

- Сверены публичные сигнатуры четырёх профильных модулей BFF:
  - `rust/gui-spike/src/slash_commands.rs` (`InvokeSlashCommandRequest`, `build_interactions_route_and_body`, `POST /api/v9/interactions`);
  - `rust/gui-spike/src/onboarding.rs` (`UpdateWelcomeScreenRequest`, `validate_welcome_screen_request`, `build_guild_welcome_screen_route`, `PATCH /api/v9/guilds/{guild_id}/welcome-screen`);
  - `rust/gui-spike/src/soundboard.rs` (`SendSoundboardSoundRequest`, `build_send_soundboard_route_and_body`, `POST /api/v9/channels/{channel_id}/send-soundboard-sound`);
  - `rust/gui-spike/src/stage_instances.rs` (`CreateStageInstanceRequest`, `build_create_stage_instance_route_and_body`, `POST /api/v9/stage-instances`).
- Через `orca orchestration ask` у координатора уточнён контракт онбординга в `bff_domains_router.rs`: объявлен алиас `pub type UpdateGuildOnboardingRequest = crate::onboarding::UpdateWelcomeScreenRequest;`, а ветка `BffDomainAction::UpdateGuildOnboarding { guild_id, request }` вызывает `validate_welcome_screen_request(request)`, `build_guild_welcome_screen_route(guild_id)` и `serde_json::to_value(request)`.

## 3. Что сделано

1. **`rust/gui-spike/tests/bff_domains_router_contract.rs` (Red-first, 8 тестов)**:
   - Проверка `SUPPORTED_BFF_DOMAINS == &["slash_commands", "onboarding", "soundboard", "stage_instances"]`.
   - Проверка `is_supported_bff_domain(domain: &str) -> bool` на позитивных и негативных входах (пустая строка, пробелы, верхний регистр, дефисы, сторонние домены).
   - Проверка serde round-trip всех четырёх вариантов `BffDomainAction`.
   - Проверка успешной маршрутизации `dispatch_bff_domain_action` для каждого из 4 вариантов (`InvokeSlashCommand`, `UpdateGuildOnboarding`, `SendSoundboardSound`, `CreateStageInstance`), включая граничные Snowflake (`"1"` и 20 девяток).
   - Проверка проброса ошибок валидации (`Err(String)`) во всех четырёх доменах (`"0"`, пустые строки, 21 цифра, ведущий ноль, регистр имени команды, превышение числа опций, `privacy_level != 2`).
2. **`rust/gui-spike/src/bff_domains_router.rs`**:
   - Реализованы `SUPPORTED_BFF_DOMAINS`, алиас `UpdateGuildOnboardingRequest`, перечисление `BffDomainAction` (и алиас `BffDomainRequest`), `is_supported_bff_domain` и `dispatch_bff_domain_action`.
3. **`rust/gui-spike/src/lib.rs`**:
   - Добавлен экспорт `pub mod bff_domains_router;`.

## 4. RED-прогон

Сохранено в `/home/sanya/orca/project/waves/RW-40/G1/task-269-be-bff-domains-router/red.log`:

```text
error[E0432]: unresolved import `discord_gui_spike::bff_domains_router`
  --> tests/bff_domains_router_contract.rs:10:24
   |
10 | use discord_gui_spike::bff_domains_router::{
   |                        ^^^^^^^^^^^^^^^^^^ could not find `bff_domains_router` in `discord_gui_spike`

error: could not compile `discord-gui-spike` (test "bff_domains_router_contract") due to 1 previous error
EXIT_CODE=101
```

## 5. GREEN-прогон и проверки (Targeted + Clippy + Rustfmt)

Сохранено в `/home/sanya/orca/project/waves/RW-40/G1/task-269-be-bff-domains-router/green.log`:

```text
running 8 tests
test bff_domain_action_serde_roundtrip_all_variants ... ok
test dispatch_bff_domain_action_propagates_validation_errors_across_all_domains ... ok
test dispatch_create_stage_instance_builds_stage_instances_route_and_body ... ok
test dispatch_invoke_slash_command_builds_interactions_route_and_body ... ok
test dispatch_send_soundboard_sound_builds_channel_route_and_body ... ok
test dispatch_update_guild_onboarding_builds_welcome_screen_route_and_body ... ok
test is_supported_bff_domain_accepts_only_registered_domains ... ok
test supported_bff_domains_constant_matches_exact_contract ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Дополнительно проверено:
- `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo clippy --manifest-path rust/gui-spike/Cargo.toml -p discord-gui-spike --test bff_domains_router_contract --lib -- -D warnings` — `0 warnings`, `exit=0`.
- `rustfmt --edition 2021 --check rust/gui-spike/src/bff_domains_router.rs rust/gui-spike/tests/bff_domains_router_contract.rs` — `exit=0`.

## 6. Файлы + md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/bff_domains_router.rs` | `b59afec01f123e50b04e685276ae003d` |
| `rust/gui-spike/src/lib.rs` | `93d19a31d69ded1e448f330e79db6fd2` |
| `rust/gui-spike/tests/bff_domains_router_contract.rs` | `cf7dcc67c12e2e047a182814823cecd4` |
| `waves/RW-40/G1/task-269-be-bff-domains-router/red.log` | `ef8c45f8fbd59fa253a5b3fb4b6e8246` |
| `waves/RW-40/G1/task-269-be-bff-domains-router/green.log` | `3d77037153158b481f46d2354c1e02c9` |

## 7. Риски и замечания

- **Дрейф `rust/gui-spike/Cargo.lock`**: при запуске `cargo` перегенерировался `Cargo.lock` из-за `discord-codec` (`zstd`); откатан через `git checkout -- rust/gui-spike/Cargo.lock`, чтобы строго соблюсти write-scope.
- `package-lock.json` был модифицирован в рабочем дереве до старта задачи — не трогался.
- `git commit` не выполнялся.
