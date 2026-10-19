#!/usr/bin/env bash
# Закрытие нитов RW-07: восстановление продукта после red-мутаций и зелёный прогон.
set -u
cd /home/sanya/orca/project/rust
LOG=/tmp/opencode/rw-07/green-gap.log
: > "$LOG"

echo "== restore (gap-backup → repo), touch ==" | tee -a "$LOG"
cp /tmp/opencode/rw-07/gap-backup/dispatch.rs crates/discord-core/src/dispatch.rs
cp /tmp/opencode/rw-07/gap-backup/ws.rs crates/discord-core/src/ws.rs
touch crates/discord-core/src/dispatch.rs crates/discord-core/src/ws.rs crates/discord-core/tests/dispatch_router.rs
md5sum crates/discord-core/src/dispatch.rs crates/discord-core/src/ws.rs crates/discord-core/tests/dispatch_router.rs | tee -a "$LOG"

run_test() { # $1=label; $2=команда
  local label="$1"; shift
  bash -c "$1" 2>&1 | tee -a "$LOG"
  echo "# EXIT=${PIPESTATUS[0]} ($label)" | tee -a "$LOG"
}

echo "== 1. dispatch_router (green) ==" | tee -a "$LOG"
run_test "dispatch_router" 'cargo test --offline -p discord-core --test dispatch_router'

echo "== 2. ×4 повтора dispatch_router ==" | tee -a "$LOG"
for i in 1 2 3 4; do
  run_test "dispatch_router повтор $i" 'cargo test --offline -p discord-core --test dispatch_router 2>&1 | grep "test result"'
done

echo "== 3. discord-core (сумма) ==" | tee -a "$LOG"
run_test "discord-core" 'cargo test --offline -p discord-core 2>&1 | grep "test result: ok" | awk "{s+=\$4} END {print \"passed:\", s}"'

echo "== 4. workspace (сумма) ==" | tee -a "$LOG"
run_test "workspace" 'cargo test --offline --workspace 2>&1 | grep "test result: ok" | awk "{s+=\$4} END {print \"passed:\", s}"'

echo "== 5. clippy -D warnings ==" | tee -a "$LOG"
run_test "clippy" 'cargo clippy --offline --workspace --all-targets -- -D warnings 2>&1 | tail -2'

echo "== 6. fmt --check ==" | tee -a "$LOG"
run_test "fmt" 'cargo fmt --check 2>&1 | tail -2'

echo "== итоговые md5 ==" | tee -a "$LOG"
md5sum crates/discord-core/src/dispatch.rs crates/discord-core/src/ws.rs crates/discord-core/src/lib.rs crates/discord-core/tests/dispatch_router.rs | tee -a "$LOG"
