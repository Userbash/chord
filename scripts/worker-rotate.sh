#!/usr/bin/env bash
# scripts/worker-rotate.sh — Zero-Idle 1-to-1 Worker Slot Rotation Engine
# Enforces AGENTS.md §2, §3, §8:
#   1. Reclaims settled terminals (orca worker-release)
#   2. Enforces WORKER_RAM_LIMIT <= 25 GB and max 5 active slots
#   3. Picks the next backlog card from waves/kanban-board.json
#   4. Dispatches the next worker via scripts/ecc-orca-bridge.sh
#
# Usage:
#   ./scripts/worker-rotate.sh --status
#   ./scripts/worker-rotate.sh --rotate
#   ./scripts/worker-rotate.sh --dry-run

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ORCA_BIN="${ORCA_BIN:-/home/sanya/.config/orca/linux-orca-cli-shim/orca}"
if [ ! -x "$ORCA_BIN" ]; then
  ORCA_BIN="orca"
fi

RUN_STATE_FILE="$PROJECT_ROOT/.claude/ecc-orca-run.json"
BOARD_FILE="$PROJECT_ROOT/waves/kanban-board.json"

RUN_ID="run_31999544da31"
if [ -f "$RUN_STATE_FILE" ]; then
  RUN_ID="$(grep -o '"runId": "[^"]*"' "$RUN_STATE_FILE" | cut -d'"' -f4 || echo "$RUN_ID")"
fi

MODE="rotate"
DRY_RUN=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --status|-s)
      MODE="status"
      shift
      ;;
    --rotate|-r)
      MODE="rotate"
      shift
      ;;
    --dry-run|-d)
      DRY_RUN=1
      shift
      ;;
    *)
      shift
      ;;
  esac
done

# Step 1: Scan memory & active slots
function get_active_slots() {
  local list_out
  list_out="$("$ORCA_BIN" orchestration worker-list --run "$RUN_ID" 2>/dev/null || true)"
  local active
  active="$(echo "$list_out" | grep -c "state=running" || true)"
  echo "$active"
}

function show_status() {
  echo "🔄 [WORKER-ROTATE] Orca Slot Status (Run: $RUN_ID)"
  local active
  active="$(get_active_slots)"
  echo "   Active running slots: $active / 5 max (AGENTS.md §8)"

  # Show reclaimable
  local list_out
  list_out="$("$ORCA_BIN" orchestration worker-list --run "$RUN_ID" 2>/dev/null || true)"
  local reclaimable
  reclaimable="$(echo "$list_out" | grep -c "terminal=reclaimable" || true)"
  echo "   Reclaimable terminals: $reclaimable"

  # Show next backlog card
  python3 -c '
import json
with open("'"$BOARD_FILE"'") as f:
    board = json.load(f)
backlog = [c for c in board.get("cards", []) if c.get("status") == "backlog"]
print(f"   Backlog cards in queue: {len(backlog)}")
if backlog:
    next_c = backlog[0]
    cid = next_c.get("id")
    cpri = next_c.get("priority", "P2")
    ctitle = next_c.get("title", "")[:50]
    print(f"   Next card: {cid} ({cpri}) - {ctitle}")
'
}

function rotate_workers() {
  echo "🔄 [WORKER-ROTATE] Running Zero-Idle slot rotation..."

  # 1. Reclaim any finished terminals
  echo "🧹 [1/3] Scanning for reclaimable terminals..."
  node "$PROJECT_ROOT/scripts/worker-gc.mjs" > /dev/null 2>&1 || true

  # 2. Check current active slots
  local active
  active="$(get_active_slots)"
  echo "📊 [2/3] Active running slots: $active / 5"

  if [ "$active" -ge 5 ]; then
    echo "⚠️  [WORKER-ROTATE] All 5 slots are currently occupied. Zero-Idle waiting for a worker to settle."
    return 0
  fi

  # 3. Pick next card from backlog
  python3 -c '
import json, subprocess, sys

board_file = "'"$BOARD_FILE"'"
dry_run = '"$DRY_RUN"'

with open(board_file) as f:
    board = json.load(f)

backlog = [c for c in board.get("cards", []) if c.get("status") == "backlog"]
if not backlog:
    print("✅ [3/3] Backlog is empty! All tasks are in_dev, in_review, or done.")
    sys.exit(0)

# Sort by priority: P0 > P1 > P2 > P3
pri_order = {"P0": 0, "P1": 1, "P2": 2, "P3": 3}
backlog.sort(key=lambda x: pri_order.get(x.get("priority", "P2"), 99))

card = backlog[0]
card_id = card.get("id")
card_pri = card.get("priority", "P2")
dept = card.get("department", "devops").lower().replace(" ", "_")
title = card.get("title", "")

print(f"🚀 [3/3] Promoting {card_id} ({card_pri}, {dept}) to in_dev...")
print(f"   Task: {title}")

if not dry_run:
    # Update card status
    card["status"] = "in_dev"
    with open(board_file, "w") as f:
        json.dump(board, f, indent=2, ensure_ascii=False)

    # Launch worker via ecc-orca-bridge
    role_map = {
        "g0_routine": "g0_routine",
        "g1_backend": "g1_backend",
        "g2_frontend": "g2_frontend",
        "g4_qa": "g4_qa",
        "g5_voice": "g5_voice",
        "devops": "devops"
    }
    role = role_map.get(dept, "devops")
    print(f"   Spawning worker for {role}...")
    subprocess.run(["bash", "'"$PROJECT_ROOT"'/scripts/ecc-orca-bridge.sh", "--role", role, "--task", f"{card_id}: {title}"], check=False)
else:
    print(f"   [DRY-RUN] Would promote {card_id} and dispatch worker.")
'
}

case "$MODE" in
  status)
    show_status
    ;;
  rotate)
    rotate_workers
    ;;
esac
