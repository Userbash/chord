#!/usr/bin/env bash
# GREEN-прогон RW-07 (ТЗ §5): тесты, ×5, clippy, fmt. Полный лог — green.log, шаги с `# EXIT=`.
set -u
cd /home/user/orca/project/rust || exit 1
L=/tmp/opencode/rw-07/green.log
: > "$L"
step() {
  local title="$1"
  shift
  {
    echo "### $title"
    echo "\$ $*"
  } >> "$L"
  bash -c "$* 2>&1" | tee -a "$L"
  echo "# EXIT=${PIPESTATUS[0]}" | tee -a "$L"
}
# Инцидент залипшего rlib (TASKS.md:678): после правок продуктовых файлов — touch перед прогонами.
touch crates/discord-core/src/dispatch.rs crates/discord-core/src/ws.rs crates/discord-core/src/lib.rs crates/discord-core/tests/dispatch_router.rs
step "G1) dispatch_router 11/11" 'cargo test --offline -p discord-core --test dispatch_router'
step "G2) discord-core 70" 'cargo test --offline -p discord-core'
step "G3) workspace 112" 'cargo test --offline --workspace'
step "G4) dispatch_router x4 (стабильность)" 'for i in 2 3 4 5; do cargo test --offline -p discord-core --test dispatch_router || exit 1; done'
step "G5) clippy -D warnings" 'cargo clippy --offline --workspace --all-targets -- -D warnings'
step "G6) fmt --check" 'cargo fmt --check'
