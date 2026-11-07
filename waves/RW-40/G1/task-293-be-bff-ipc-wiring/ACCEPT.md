# ACCEPT — PR-173 (`task-293` / `BE-INT-16`: Регистрация Tauri IPC-команды `invoke_bff_domain` в `rust/gui-spike/src/main.rs`)

- **Статус PM-приёмки:** `ACCEPTED` (`Ready for Technical Review`)
- **Ветка:** `feature/task-293-be-bff-ipc-wiring`
- **Отдел:** `G1 Backend (Rust Core & Tauri BFF)`
- **Модель исполнения:** `DeepSeek` (`deepseek/deepseek-v4.1-flash`, `L1 / P1`)

## Сверка с DoD
1. В `rust/gui-spike/src/main.rs` зарегистрирована команда `#[tauri::command] fn invoke_bff_domain(action: bff_domains_router::BffDomainAction) -> Result<bff_domains_router::BffResolvedAction, String>` и добавлена в `tauri::generate_handler![...]`.
2. В `mod wiring_tests` добавлен тест `invoke_handler_registers_invoke_bff_domain_for_seven_domains` (`13/13 PASS`).
3. Мета-тест `node --test tests/meta/ipc-path.test.mjs` проходит `12/12 PASS`.
4. Передано на техническое ревью `TR-200A` (5/5 мутаций).
