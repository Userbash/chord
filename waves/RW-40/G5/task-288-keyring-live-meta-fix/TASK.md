# TASK — task-288 (`G5 Platform L1 / P0`, `PLAT-FIX-01`: Вынос `k13..k26` из `keyring_contract.rs` в `safe_storage_contract.rs` для `tests/meta/keyring-live.test.mjs`)

- **Отдел:** G5 Voice / Platform (`DeepSeek` — `--agent opencode --model deepseek/deepseek-v4.1-flash`)
- **Уровень сложности / Приоритет:** `L1 / P0` (блокер `tests/meta/keyring-live.test.mjs` в `npm run test:meta`, по `waves/RW-40/PM/MODEL-ROUTING-CRITERIA.md`)
- **Рабочее дерево:** `/home/sanya/orca/workspaces/project/bugfix-task-288-keyring-live-meta-fix` (`branch:bugfix/task-288-keyring-live-meta-fix`)
- **Разрешённые к изменению файлы (строго 2 файла в worktree, `tests/meta/keyring-live.test.mjs` НЕ трогать):**
  1. `rust/crates/discord-platform/tests/keyring_contract.rs`
  2. `rust/crates/discord-platform/tests/safe_storage_contract.rs` (новый файл)
- **Артефакты отчёта:**
  - `/home/sanya/orca/project/waves/RW-40/G5/task-288-keyring-live-meta-fix/REPORT.md`
  - `/home/sanya/orca/project/waves/RW-40/G5/task-288-keyring-live-meta-fix/red.log`
  - `/home/sanya/orca/project/waves/RW-40/G5/task-288-keyring-live-meta-fix/green.log`

## Точное описание задачи (без поиска по репозиторию)

1. Перейди в `/home/sanya/orca/workspaces/project/bugfix-task-288-keyring-live-meta-fix`.
2. **Red-прогон:** запусти `node --test tests/meta/keyring-live.test.mjs` и сохрани вывод в `/home/sanya/orca/project/waves/RW-40/G5/task-288-keyring-live-meta-fix/red.log` (5 падений из-за того, что в `rust/crates/discord-platform/tests/keyring_contract.rs` на строках 377–731 были дописаны тесты `k13..k26`, тогда как `tests/meta/keyring-live.test.mjs` требует, чтобы в `keyring_contract.rs` было ровно 12 live-кейсов `k01..k12` со статусом `0 passed; 0 failed; 12 ignored`).
3. **Правка (без потери и без ослабления ни одного теста):**
   - Вынеси тесты `k13_safe_storage_prefix_and_detection` .. `k26_migration_failure_paths_and_state_do_not_leak_secrets` (строки 377–731 из `rust/crates/discord-platform/tests/keyring_contract.rs`) вместе с нужными импортами (`SafeStorageMigrator`, `SafeStorageMigrationError`, `ENCRYPTED_TOKEN_PREFIX`, `extract_safe_storage_ciphertext`, `is_safe_storage_token`, `KeyringError` и хелперами `live_keyring`/`unique_key` для `k25_migrator_with_live_keyring`) в отдельный контрактный тест `rust/crates/discord-platform/tests/safe_storage_contract.rs`.
   - Из `rust/crates/discord-platform/tests/keyring_contract.rs` убери `k13..k26` и неиспользуемые импорты `safe_storage`, оставив ровно исходные `k01..k12`.
   - **Важно:** `tests/meta/keyring-live.test.mjs` НЕ изменять!
4. **Green-прогон (используй общий `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target`):**
   - `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --offline --manifest-path rust/Cargo.toml -p discord-platform --test keyring_contract --test safe_storage_contract`
   - `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target node --test tests/meta/keyring-live.test.mjs` (`13/13 PASS`)
   - `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target npm run test:meta` (`102/102 PASS`)
   Сохрани вывод в `/home/sanya/orca/project/waves/RW-40/G5/task-288-keyring-live-meta-fix/green.log`.
5. Запиши `/home/sanya/orca/project/waves/RW-40/G5/task-288-keyring-live-meta-fix/REPORT.md` (с `md5sum` обоих файлов) и заверши задачу через `orca orchestration complete`. Не делай `git commit/push`.
