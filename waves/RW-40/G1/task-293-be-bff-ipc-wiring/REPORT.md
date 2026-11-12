# REPORT — task-293 (`G1 Backend L1 / P1`, `BE-INT-16`): регистрация `invoke_bff_domain` в `rust/gui-spike/src/main.rs`

- **Карточка:** `task-293` / `BE-INT-16` — единый Tauri IPC-роутер BFF `#[tauri::command] fn invoke_bff_domain`.
- **Ветка / дерево:** `feature/task-293-be-bff-ipc-wiring`, `/home/user/orca/workspaces/project/feature-task-293-be-bff-ipc-wiring`, база `unstable` = `648ba83`.
- **Статус:** green, готово к приёмке. `git commit/push` не выполнялись.

## Изменённый файл (ровно один)

| Файл | md5sum | Что сделано |
| --- | --- | --- |
| `rust/gui-spike/src/main.rs` | `91d37d09759e87f2cd6e73ca1b55f8aa` | команда `invoke_bff_domain` (стр. 1165–1174), регистрация в `tauri::generate_handler![...]` после `settings_save` (стр. 1264), тест в `mod wiring_tests` (стр. 2192–2240) |

`git status` показывает ещё `M package-lock.json` и `D node_modules` — это состояние дерева до начала работы, не мои правки.
`rust/gui-spike/Cargo.lock` прогон `cargo test --offline` перезаписал сам (в базе дрейф: `discord-codec` ждёт `zstd`, lock держал `ruzstd`); файл откачен к HEAD, чтобы не выходить за разрешённый список.

## Strict TDD

### Red (`red.log`)

Тест добавлен первым, до реализации. Прогон:

```
CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --offline \
  --manifest-path rust/gui-spike/Cargo.toml --bin discord-gui-spike \
  wiring_tests::invoke_handler_registers_invoke_bff_domain_for_seven_domains
```

Падение по делу — только отсутствие функции, других ошибок нет:

```
error[E0425]: cannot find function `invoke_bff_domain` in this scope
    --> src/main.rs:2196:13
error[E0425]: cannot find function `invoke_bff_domain` in this scope
    --> src/main.rs:2216:23
error: could not compile `discord-gui-spike` (bin "discord-gui-spike" test) due to 2 previous errors
exit=101
```

### Green (`green.log`)

```
CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --offline \
  --manifest-path rust/gui-spike/Cargo.toml --bin discord-gui-spike
test result: ok. 13 passed; 0 failed; 0 ignored
```

в том числе `wiring_tests::invoke_handler_registers_invoke_bff_domain_for_seven_domains ... ok`
(остальные 12 — ранее существовавшие `wiring_tests`, все зелёные).

```
node --test tests/meta/ipc-path.test.mjs
tests 12, pass 12, fail 0
```

Мета-тест PLAT-11 зелёный: аргумент команды назван `action`, путь/`PathBuf` в IPC не попал, набор `#[tauri::command]` совпадает с `generate_handler!`.

## Что именно реализовано

```rust
/// Единый Tauri IPC-роутер для доменов BFF (`bff_domains_router`):
/// `slash_commands`, `onboarding`, `soundboard`, `stage_instances` и их
/// последующие расширения. Валидирует действие и отдаёт метод, маршрут и тело
/// запроса; токен и сеть сюда не попадают — отправку делает сетевой слой.
#[tauri::command]
fn invoke_bff_domain(
    action: discord_gui_spike::bff_domains_router::BffDomainAction,
) -> Result<discord_gui_spike::bff_domains_router::BffResolvedAction, String> {
    discord_gui_spike::bff_domains_router::resolve_bff_domain_action(action)
}
```

Плюс `invoke_bff_domain` в `tauri::generate_handler![...]` сразу после `settings_save`.

Тест `invoke_handler_registers_invoke_bff_domain_for_seven_domains` проверяет три вещи:

1. `lists_command(handler_block(head_source()), "invoke_bff_domain")` — команда реально зарегистрирована в `invoke_handler!` (сверяется голова файла, чтобы литералы теста не давали ложное «нашлось»);
2. прямой вызов функции на валидном `BffDomainAction::UpdateGuildOnboarding { guild_id: "123456789012345678" }` даёт ровно `Ok(BffResolvedAction { method: "PATCH", route: "/api/v9/guilds/123456789012345678/welcome-screen", body: {"enabled": true, "description": null, "welcome_channels": []} })`;
3. на невалидном `guild_id: "0"` — `Err(...)`, без паники.

## Отклонение от ТЗ (согласовано)

ТЗ требовало `BffDomainAction::ListAutoModRules`, `method: "GET"`, `body: None` и «семь доменов». В базе `unstable 648ba83` этого контракта нет: `bff_domains_router` объявляет 4 домена (`slash_commands`, `onboarding`, `soundboard`, `stage_instances`), `BffResolvedAction.method: &'static str`, `body: serde_json::Value`, варианта `ListAutoModRules` нет ни в одном `rust/`-файле. Литтест из ТЗ не скомпилировался бы, а расширять роутер нельзя — разрешён ровно один файл.

Вопрос ушёл координатору через `orca orchestration ask`; ответ: «адаптируй тест под реальный контракт bff_domains_router… регистрация `invoke_bff_domain` — как в ТЗ». Имя теста оставил из карточки (`..._for_seven_domains` — целевой список при расширении роутера), в doc-комментарии теста зафиксировано фактическое состояние (4 домена, проверка на `UpdateGuildOnboarding`). Когда роутер дорастёт до `automod`/`expressions`/`webhooks`, тест расширяется без правки команды.

## Дополнительные проверки

- `rustfmt --check --edition 2021 rust/gui-spike/src/main.rs` — чисто (exit 0).
- `cargo clippy --bin discord-gui-spike --all-targets` — одно предупреждение, не моё: `clippy::type_complexity` на `src/main.rs:1718` в тестовом хелпере вложений (было до задачи).
- `cargo fmt --check` по крейту целиком краснеет на чужих файлах (`src/lib.rs` — порядок `pub mod`, `src/net.rs`, `tests/net_contract.rs`); это состояние базы, `src/main.rs` в диффе отсутствует.

## Риски

- `BffResolvedAction.body` — `serde_json::Value`, поэтому «нет тела» выражается `null`, а не `None`. Когда появится домен-чтение (`GET`, без тела), контракт `BffResolvedAction` придётся менять на `Option<Value>` — это отдельная карточка в `bff_domains_router`, не в `main.rs`.
- Shared `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`: прогоны делались последовательно, без параллельного `cargo`; lock-дрейф базы (см. выше) следующий прогон снова перезапишет.
