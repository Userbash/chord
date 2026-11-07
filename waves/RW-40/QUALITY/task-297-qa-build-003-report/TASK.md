# TASK-297 — Централизованная QA-верификация `Build-003` (`L1 / P0`, сжатый контекст)

- **Ветка / Дерево:** `/home/sanya/orca/workspaces/project/testing` (ветка `testing`). Никакие другие файлы вне отчёта не читать и не менять.
- **Выходной файл (ровно 1):** `/home/sanya/orca/project/waves/RW-40/QUALITY/QA-BUILD-003-REPORT.md`.
- **Команды проверки (выполнить один раз в `/home/sanya/orca/workspaces/project/testing`):**
  ```bash
  git rev-parse --short HEAD
  npm run typecheck && npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit
  npm run test:meta
  node --test tests/unit/test-impact.test.mjs tests/unit/dispatch_ui.test.mjs tests/unit/app-shell-wiring.test.mjs tests/unit/composer-popovers-bridge.test.mjs tests/unit/voice-diagnostics.test.mjs tests/unit/voice-ui.test.mjs tests/unit/reactions.test.mjs
  CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --manifest-path rust/gui-spike/Cargo.toml --test ipc_wiring_contract --test bff_domains_router_contract --test voice_stage_contract --test voice_diagnostics_contract
  CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test -p discord-core --test dispatch_types
  CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test -p discord-voice --lib
  CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test -p discord-platform --test safe_storage_contract
  ```
- **Формат отчёта:** компактная таблица (коммит `testing`, статус каждой команды `PASS`, `P0=0, P1=0`, вердикт `QA Passed — Ready for Beta`).
