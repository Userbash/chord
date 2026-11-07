# TASK-TR-202A — Техническое ревью PR-174 (`task-294`: `rust/gui-spike/src/voice_diagnostics.rs` + `voice_diagnostics_contract.rs`)

- **Роль:** `Technical Reviewer` (`L2 / P1`)
- **Ветка для проверки:** `feature/task-294-be-voice-diagnostics` (`/home/sanya/orca/workspaces/project/feature-task-294-be-voice-diagnostics`)
- **Проверяемые файлы:**
  - `rust/gui-spike/src/voice_diagnostics.rs`
  - `rust/gui-spike/src/lib.rs`
  - `rust/gui-spike/tests/voice_diagnostics_contract.rs`
  - `scripts/test-impact.mjs`
  - `tests/unit/test-impact.test.mjs`

## Задание
1. Проверь `git status -s` / `git diff` — убедись, что затронуты только указанные файлы.
2. Прогони целевые тесты (с `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target`):
   ```bash
   CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --manifest-path rust/gui-spike/Cargo.toml --test voice_diagnostics_contract
   CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo clippy --manifest-path rust/gui-spike/Cargo.toml --test voice_diagnostics_contract -- -D warnings
   node --test tests/unit/test-impact.test.mjs
   ```
3. Выполни 5/5 точечных мутаций в `rust/gui-spike/src/voice_diagnostics.rs` (с обязательным побайтным откатом после каждой) и убедись, что они убиваются `voice_diagnostics_contract`.
4. Запиши вердикт (`APPROVED` при `P0=0, P1=0` и `5/5` мутаций) в `/home/sanya/orca/project/waves/RW-40/G1/task-294-be-voice-diagnostics/REVIEW-202A.md`.
