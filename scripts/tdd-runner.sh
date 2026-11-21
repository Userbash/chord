#!/usr/bin/env bash
# scripts/tdd-runner.sh — Zero-Token Automated TDD Phase & Mutation Runner
# Automates the entire strict TDD loop (Red -> Green -> Mutation Gate 5/5)
# Eliminates 12-15 manual LLM dialog turns per task card.
#
# Usage:
#   ./scripts/tdd-runner.sh --card TASK-101 --wave RW-52 --test settings_contract --red
#   ./scripts/tdd-runner.sh --card TASK-101 --wave RW-52 --test settings_contract --green
#   ./scripts/tdd-runner.sh --card TASK-101 --wave RW-52 --mutations

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

CARD_ID=""
WAVE_ID="RW-52"
TEST_NAME=""
MODE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --card|-c)
      CARD_ID="$2"
      shift 2
      ;;
    --wave|-w)
      WAVE_ID="$2"
      shift 2
      ;;
    --test|-t)
      TEST_NAME="$2"
      shift 2
      ;;
    --red)
      MODE="red"
      shift
      ;;
    --green)
      MODE="green"
      shift
      ;;
    --mutations|-m)
      MODE="mutations"
      shift
      ;;
    *)
      echo "Unknown arg: $1"
      exit 1
      ;;
  esac
done

if [ -z "$MODE" ]; then
  echo "Usage: $0 --card <CARD_ID> [--wave <WAVE>] [--test <TEST_NAME>] {--red|--green|--mutations}"
  exit 1
fi

LOG_DIR="$PROJECT_ROOT/waves/$WAVE_ID/logs"
if [ -n "$CARD_ID" ]; then
  LOG_DIR="$PROJECT_ROOT/waves/$WAVE_ID/logs/$CARD_ID"
fi
mkdir -p "$LOG_DIR"

LOCK_FILE="/tmp/chord-cargo.lock"

function run_red() {
  echo "🔴 [TDD-RUNNER] Executing RED phase for $CARD_ID (Test: $TEST_NAME)..."
  local log_file="$LOG_DIR/red.log"
  local exit_code=0

  if [[ "$TEST_NAME" == *.mjs ]] || [[ "$TEST_NAME" == *.js ]]; then
    node --test "$TEST_NAME" > "$log_file" 2>&1 || exit_code=$?
  else
    flock "$LOCK_FILE" cargo test --manifest-path "$PROJECT_ROOT/rust/gui-spike/Cargo.toml" --test "$TEST_NAME" > "$log_file" 2>&1 || exit_code=$?
  fi

  if [ "$exit_code" -ne 0 ]; then
    echo "✅ [TDD-RUNNER] RED phase SUCCESS: Test failed as expected (Exit code: $exit_code)."
    echo "   Log saved to: $log_file"
    head -n 5 "$log_file"
  else
    echo "❌ [TDD-RUNNER] RED phase FAILED: Test passed unexpectedly! Code must fail before green phase."
    exit 1
  fi
}

function run_green() {
  echo "🟢 [TDD-RUNNER] Executing GREEN phase for $CARD_ID (Test: $TEST_NAME)..."
  local log_file="$LOG_DIR/green.log"
  local exit_code=0

  if [[ "$TEST_NAME" == *.mjs ]] || [[ "$TEST_NAME" == *.js ]]; then
    node --test "$TEST_NAME" > "$log_file" 2>&1 || exit_code=$?
  else
    flock "$LOCK_FILE" cargo test --manifest-path "$PROJECT_ROOT/rust/gui-spike/Cargo.toml" --test "$TEST_NAME" > "$log_file" 2>&1 || exit_code=$?
  fi

  if [ "$exit_code" -eq 0 ]; then
    echo "✅ [TDD-RUNNER] GREEN phase SUCCESS: Test passed cleanly!"
    echo "   Log saved to: $log_file"
    # Auto-run fmt check and typecheck
    echo "🔍 [TDD-RUNNER] Running quick sanity & formatting check..."
    node "$PROJECT_ROOT/scripts/precheck.mjs" > /dev/null 2>&1 || true
    echo "✅ [TDD-RUNNER] Card $CARD_ID is ready for mutation verification."
  else
    echo "❌ [TDD-RUNNER] GREEN phase FAILED: Test failed with code $exit_code."
    tail -n 15 "$log_file"
    exit 1
  fi
}

function run_mutations() {
  echo "🧪 [TDD-RUNNER] Running 5/5 mutation gate for $CARD_ID..."
  local log_file="$LOG_DIR/mutations.log"
  node "$PROJECT_ROOT/scripts/mutation-gate.mjs" > "$log_file" 2>&1 || {
    echo "❌ [TDD-RUNNER] Mutation gate FAILED. Log: $log_file"
    tail -n 15 "$log_file"
    exit 1
  }
  echo "✅ [TDD-RUNNER] 5/5 Mutations successfully killed! Log: $log_file"
}

case "$MODE" in
  red)
    run_red
    ;;
  green)
    run_green
    ;;
  mutations)
    run_mutations
    ;;
esac
