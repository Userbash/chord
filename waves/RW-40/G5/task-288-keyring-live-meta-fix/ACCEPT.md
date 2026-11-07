# ACCEPT — task-288 (`G5 Platform L1 / P0`, `PLAT-FIX-01`: Вынос `k13..k26` из `keyring_contract.rs` в `safe_storage_contract.rs`)

- **PR:** `PR-168` (`branch:bugfix/task-288-keyring-live-meta-fix`)
- **Статус PM:** `APPROVED` (`Ready for Technical Review` -> `TR-196A`)
- **Проверка DoD:**
  1. `rust/crates/discord-platform/tests/keyring_contract.rs` содержит ровно 12 live-кейсов `k01..k12` (`0 passed; 0 failed; 12 ignored`), не ослаблен ни один порог (`RSS_LIMIT_KILOBYTES`, `LEAK_ITERATIONS`, `LEAK_SECRET_BYTES`).
  2. `rust/crates/discord-platform/tests/safe_storage_contract.rs` содержит все 14 кейсов `k13..k26` (`13 passed; 0 failed; 1 ignored`).
  3. `node --test tests/meta/keyring-live.test.mjs` (`13/13 PASS`) и `npm run test:meta` (`102/102 PASS`, `EXIT=0`) проходят без изменений `tests/meta/keyring-live.test.mjs`.
