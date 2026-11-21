#!/usr/bin/env bash
# ==============================================================================
# scripts/test-all-rust.sh — Unified Zero-Token Rust Test Runner
#
# Автоматически и последовательно тестирует все Rust-модули и изолированные спайки:
# 1. chord-core (lib + REST bridge + Gateway supervisor + dispatch)
# 2. chord-store (worker + FTS5 + migrations v2 + store contract)
# 3. chord-platform (keyring + ashpd + secrets + tray + spellcheck)
# 4. rust/dave-spike (DAVE MLS v1 + AES-GCM + KAT vectors)
# 5. rust/audio-spike (cpal pipeline + ArrayQueue ring buffer + VAD)
#
# Соблюдает лимит памяти (WORKER_RAM_LIMIT <= 25GB) за счет последовательного
# запуска и флага --test-threads=2.
#
# Использование:
#   bash scripts/test-all-rust.sh [--fast]
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "🦀 ================================================================"
echo "🦀  UNIFIED RUST TEST RUNNER (ALL CRATES & SPIKES)"
echo "🦀 ================================================================"

# Проверка ветки
"$PROJECT_ROOT/scripts/assert-branch-tauri.sh"

# Контроль RAM
"$PROJECT_ROOT/scripts/worker-ram-guard.sh" >/dev/null 2>&1 || true

TOTAL_PASSED=0
FAILURES=0

run_crate_tests() {
  local name="$1"
  local cmd="$2"
  echo -n "🧪 [${name}] Testing... "
  
  local output
  if output=$(eval "$cmd" 2>&1); then
    local passed
    passed=$(echo "$output" | grep -oE '[0-9]+ passed' | awk '{s+=$1} END {print s}')
    passed=${passed:-0}
    TOTAL_PASSED=$((TOTAL_PASSED + passed))
    echo "✅ OK (${passed} passed)"
  else
    echo "❌ FAILED"
    echo "$output" | tail -n 25
    FAILURES=$((FAILURES + 1))
  fi
}

# 1. chord-core
run_crate_tests "chord-core" "cargo test --manifest-path $PROJECT_ROOT/rust/Cargo.toml -p chord-core --lib -- --test-threads=2"

# 2. chord-store
run_crate_tests "chord-store" "cargo test --manifest-path $PROJECT_ROOT/rust/Cargo.toml -p chord-store -- --test-threads=2"

# 3. chord-platform
run_crate_tests "chord-platform" "cargo test --manifest-path $PROJECT_ROOT/rust/Cargo.toml -p chord-platform --test autostart_contract --test dbus_contract --test rpc_ipc_contract --test screencast_contract --test spellcheck_contract --test tray_contract -- --test-threads=2"

# 4. dave-spike
run_crate_tests "dave-spike" "cargo test --manifest-path $PROJECT_ROOT/rust/dave-spike/Cargo.toml -- --test-threads=2"

# 5. audio-spike
run_crate_tests "audio-spike" "cargo test --manifest-path $PROJECT_ROOT/rust/audio-spike/Cargo.toml -- --test-threads=2"

echo "----------------------------------------------------------------"
if [ "$FAILURES" -eq 0 ]; then
  echo "🎉 All Rust suites passed! Total tests verified: $TOTAL_PASSED"
  echo "🦀 ================================================================"
  exit 0
else
  echo "❌ $FAILURES suite(s) failed. Fix errors above."
  echo "🦀 ================================================================"
  exit 1
fi
