# TASK-TR-196A — Техническое ревью `PR-168` (`task-288` / `PLAT-FIX-01`: `keyring_contract.rs` + `safe_storage_contract.rs`)

- **Отдел:** Technical Review (`DeepSeek` — `--agent opencode --model deepseek/deepseek-v4.1-flash`)
- **Уровень сложности / Приоритет:** `L2 / P1` (по `waves/RW-40/PM/MODEL-ROUTING-CRITERIA.md`)
- **Рабочее дерево:** `/home/user/orca/workspaces/project/bugfix-task-288-keyring-live-meta-fix` (`branch:bugfix/task-288-keyring-live-meta-fix`)
- **Проверяемые файлы:**
  1. `rust/crates/discord-platform/tests/keyring_contract.rs`
  2. `rust/crates/discord-platform/tests/safe_storage_contract.rs`
- **Отчёт воркера и PM Accept:**
  - `/home/user/orca/project/waves/RW-40/G5/task-288-keyring-live-meta-fix/REPORT.md`
  - `/home/user/orca/project/waves/RW-40/G5/task-288-keyring-live-meta-fix/ACCEPT.md`

## Задание ревьюеру
1. Перейди в `/home/user/orca/workspaces/project/bugfix-task-288-keyring-live-meta-fix`.
2. Запусти (с `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`):
   - `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --offline --manifest-path rust/Cargo.toml -p discord-platform --test keyring_contract --test safe_storage_contract`
   - `CARGO_TARGET_DIR=/home/user/orca/project/rust/target node --test tests/meta/keyring-live.test.mjs` (`13/13 PASS`)
   - `CARGO_TARGET_DIR=/home/user/orca/project/rust/target npm run test:meta` (`102/102 PASS`)
3. Выполни **5 ручных мутаций** в `rust/crates/discord-platform/src/safe_storage.rs` (для `safe_storage_contract`) и/или в `rust/crates/discord-platform/tests/keyring_contract.rs` (для `tests/meta/keyring-live.test.mjs`) с обязательным побайтовым откатом после каждой, подтвердив убийство `5/5` мутаций (`100%`).
4. Проверь, что `tests/meta/keyring-live.test.mjs` не изменялся и все 26 кейсов `k01..k26` сохранены без ослаблений.
5. Запиши `/home/user/orca/project/waves/RW-40/G5/task-288-keyring-live-meta-fix/REVIEW-196A.md` с вердиктом (`APPROVED` или `CHANGES REQUESTED`, `P0=0, P1=0`, таблица `5/5` мутаций).
6. Заверши через `orca orchestration complete`. Не делай `git commit/push`.
