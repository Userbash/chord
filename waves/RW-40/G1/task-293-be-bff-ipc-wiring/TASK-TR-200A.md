# TASK-TR-200A — Техническое ревью PR-173 (`task-293`: `rust/gui-spike/src/main.rs`)

- **Роль:** `Technical Reviewer (DeepSeek)` (`L1 / P1`)
- **Ветка для проверки:** `feature/task-293-be-bff-ipc-wiring` (рабочее дерево `/home/sanya/orca/workspaces/project/feature-task-293-be-bff-ipc-wiring`)
- **Проверяемый файл:** `rust/gui-spike/src/main.rs`

## Задание
1. Проверь `git diff unstable..HEAD` — убедись, что изменён ровно 1 файл (`rust/gui-spike/src/main.rs`).
2. Прогони целевые тесты (с `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target`):
   ```bash
   CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --manifest-path rust/gui-spike/Cargo.toml --bin discord-gui-spike
   node --test tests/meta/ipc-path.test.mjs
   ```
3. Выполни 5/5 мутаций в `rust/gui-spike/src/main.rs` (с обязательным откатом после каждой) и убедись, что они убиваются тестами `cargo test --manifest-path rust/gui-spike/Cargo.toml --bin discord-gui-spike`.
4. Запиши вердикт (`APPROVED` при `P0=0, P1=0` и `5/5` мутаций) в `/home/sanya/orca/project/waves/RW-40/G1/task-293-be-bff-ipc-wiring/REVIEW-200A.md`.
