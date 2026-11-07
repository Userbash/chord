# TASK — task-293 (`G1 Backend L1 / P1`, `BE-INT-16`: Регистрация `#[tauri::command] fn invoke_bff_domain` в `rust/gui-spike/src/main.rs`)

- **Отдел:** G1 Backend (Rust Core & Tauri BFF) (`DeepSeek` — `--agent opencode --model deepseek/deepseek-v4.1-flash`)
- **Уровень сложности / Приоритет:** `L1 / P1` (Фаза 0 из `RELEASE-NOTES-005.md` §4, по `waves/RW-40/PM/MODEL-ROUTING-CRITERIA.md`)
- **Рабочее дерево:** `/home/sanya/orca/workspaces/project/feature-task-293-be-bff-ipc-wiring` (`branch:feature/task-293-be-bff-ipc-wiring`)
- **Разрешённый к изменению файл (строго 1 файл):**
  - `rust/gui-spike/src/main.rs` (строки около 1164, 1252 и 2175)
- **Артефакты отчёта:**
  - `/home/sanya/orca/project/waves/RW-40/G1/task-293-be-bff-ipc-wiring/REPORT.md`
  - `/home/sanya/orca/project/waves/RW-40/G1/task-293-be-bff-ipc-wiring/red.log`
  - `/home/sanya/orca/project/waves/RW-40/G1/task-293-be-bff-ipc-wiring/green.log`

## Точное описание задачи (без поиска по репозиторию)

1. Перейди в `/home/sanya/orca/workspaces/project/feature-task-293-be-bff-ipc-wiring`.
2. **Strict TDD (сначала тест в `mod wiring_tests` в конце `rust/gui-spike/src/main.rs` -> `red.log`):**
   - В конец `mod wiring_tests` (около строки 2175 `rust/gui-spike/src/main.rs`) добавь тест `invoke_handler_registers_invoke_bff_domain_for_seven_domains`:
     - проверяет, что `lists_command(handler_block(head_source()), "invoke_bff_domain")` равно `true`;
     - вызывает `invoke_bff_domain` (напрямую как Rust-функцию в `wiring_tests`) на валидном действии `BffDomainAction::ListAutoModRules { guild_id: "123456789012345678".to_string() }` и проверяет `Ok(BffResolvedAction { method: "GET", route: "/guilds/123456789012345678/auto-moderation/rules".to_string(), body: None })`, а также на невалидном `guild_id: "0"` проверяет `Err(...)`.
   - Запусти `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --offline --manifest-path rust/gui-spike/Cargo.toml --bin discord-gui-spike wiring_tests::invoke_handler_registers_invoke_bff_domain_for_seven_domains` и сохрани вывод падения в `red.log`.
3. **Минимальная реализация в `rust/gui-spike/src/main.rs`:**
   - Сразу после `settings_save` (около строки 1164) добавь:
     ```rust
     /// Единый Tauri IPC-роутер для 7 BFF-доменов (`automod`, `expressions`, `webhooks`,
     /// `slash_commands`, `onboarding`, `soundboard`, `stage_instances`).
     #[tauri::command]
     fn invoke_bff_domain(
         action: discord_gui_spike::bff_domains_router::BffDomainAction,
     ) -> Result<discord_gui_spike::bff_domains_router::BffResolvedAction, String> {
         discord_gui_spike::bff_domains_router::resolve_bff_domain_action(&action)
     }
     ```
     *(Важно: имя аргумента строго `action` — не используй имена со словами `path`/`root`/`dir`, чтобы мета-тест `tests/meta/ipc-path.test.mjs` оставался зелёным).*
   - В `tauri::generate_handler![...]` (около строки 1252, после `settings_save`) добавь `invoke_bff_domain`.
4. **Green-прогон (с `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target`):**
   - `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --offline --manifest-path rust/gui-spike/Cargo.toml --bin discord-gui-spike`
   - `node --test tests/meta/ipc-path.test.mjs`
   Сохрани вывод в `green.log`.
5. Запиши `/home/sanya/orca/project/waves/RW-40/G1/task-293-be-bff-ipc-wiring/REPORT.md` (с `md5sum rust/gui-spike/src/main.rs`) и заверши через `orca orchestration complete`. Не делай `git commit/push`.
