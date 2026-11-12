# QA-BUILD-003-REPORT: Централизованная QA-верификация Build-003

- **Дата верификации:** 2026-10-31
- **Окружение:** Linux x86_64, Node v24, Cargo / Rust workspace
- **Ветка:** `testing`
- **Коммит:** `0bc83bf`
- **Дефекты:** `P0 = 0`, `P1 = 0`

## Результаты проверок

| № | Проверка / Команда | Результат | Метрики / Детали |
|---|--------------------|-----------|------------------|
| 1 | `git rev-parse --short HEAD` | `PASS` | `0bc83bf` |
| 2 | `npm run typecheck && npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` | `PASS` | TypeScript typecheck пройден без ошибок |
| 3 | `npm run test:meta` | `PASS` | 102 passed, 0 failed, 17 suites |
| 4 | `node --test tests/unit/test-impact.test.mjs tests/unit/dispatch_ui.test.mjs tests/unit/app-shell-wiring.test.mjs tests/unit/composer-popovers-bridge.test.mjs tests/unit/voice-diagnostics.test.mjs tests/unit/voice-ui.test.mjs tests/unit/reactions.test.mjs` | `PASS` | 261 passed, 0 failed, 38 suites |
| 5 | `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/gui-spike/Cargo.toml --test ipc_wiring_contract --test bff_domains_router_contract --test voice_stage_contract --test voice_diagnostics_contract` | `PASS` | 48 passed, 0 failed (`bff`: 10, `ipc`: 9, `voice_diag`: 21, `voice_stage`: 8) |
| 6 | `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml -p discord-core --test dispatch_types` | `PASS` | 4 passed, 0 failed |
| 7 | `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml -p discord-voice --lib` | `PASS` | 36 passed, 0 failed |
| 8 | `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml -p discord-platform --test safe_storage_contract` | `PASS` | 13 passed, 0 failed, 1 ignored (live-keyring) |

## Вердикт
**QA Passed — Ready for Beta**
