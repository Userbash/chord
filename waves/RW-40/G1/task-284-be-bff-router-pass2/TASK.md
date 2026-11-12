# ТЗ — task-284 (`G1 Backend L2 / P1`: `BE-FIX-01` — Pass 2 `bff_domains_router.rs`: метод HTTP и сериализуемый `BffResolvedRoute` по `REVIEW-182A`)

## Приоритет, Сложность и Экономия токенов
- **Приоритет:** `P1` | **Сложность:** `L2` (1 модуль Rust + 1 контрактный тест).
- **Модель:** `deepseek/deepseek-v4.1-flash` (`--agent opencode`).
- **Экономия токенов:** НЕ читай лишние крейты! Открой только `rust/gui-spike/src/bff_domains_router.rs` и `rust/gui-spike/tests/bff_domains_router_contract.rs`.

## Разрешённые файлы для записи (Write-Scope)
- `rust/gui-spike/src/bff_domains_router.rs`
- `rust/gui-spike/tests/bff_domains_router_contract.rs`
- `/home/user/orca/project/waves/RW-40/G1/task-284-be-bff-router-pass2/red.log`
- `/home/user/orca/project/waves/RW-40/G1/task-284-be-bff-router-pass2/green.log`
- `/home/user/orca/project/waves/RW-40/G1/task-284-be-bff-router-pass2/REPORT.md`

## Шаги выполнения (Strict TDD: Red -> Green)
1. **Red:** В `rust/gui-spike/tests/bff_domains_router_contract.rs` добавь 2 теста (сохранив существующие 8 тестов):
   - Тест `resolve_bff_domain_action_returns_http_method_route_and_body`: проверяет новую публичную структуру `BffResolvedAction { pub method: &'static str, pub route: String, pub body: serde_json::Value }` и функцию `resolve_bff_domain_action(req: BffDomainRequest) -> Result<BffResolvedAction, String>`, которая возвращает `"POST"` для `TriggerSlashCommand` / `SendSoundboardSound` / `CreateStageInstance` и `"PATCH"` для `UpdateGuildOnboarding`.
   - Тест `bff_domain_request_wire_format_matches_tagged_domain_action`: проверяет десериализацию из буквального JSON `{"domain":"soundboard","action":{"guild_id":"111","sound_id":"222"}}` (или текущего wire-формата `BffDomainRequest`).
   Запусти `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/gui-spike/Cargo.toml --test bff_domains_router_contract` и сохрани падение компиляции в `red.log`.
2. **Green:** Добавь `BffResolvedAction` и `resolve_bff_domain_action` в `rust/gui-spike/src/bff_domains_router.rs` (сохранив обратную совместимость `build_bff_domain_route_and_body`).
3. Проверь `cargo test` (`10/10 PASS`) и `cargo clippy --manifest-path rust/gui-spike/Cargo.toml --test bff_domains_router_contract -- -D warnings` (`0 warnings`), откати `Cargo.lock` если изменился, запиши `green.log` и `REPORT.md` с `md5sum` и вызови `orca orchestration worker-done`. Не делай `git commit`.
