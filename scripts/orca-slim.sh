#!/usr/bin/env bash
# scripts/orca-slim.sh — Zero-Token Orca Orchestration & Worktree Summarizer
# Compresses 5,000-line JSON outputs into a 3-line status string.
# Usage:
#   ./scripts/orca-slim.sh
#   ./scripts/orca-slim.sh --worktrees
#   ./scripts/orca-slim.sh --run <run_id>

set -euo pipefail

ORCA_BIN="${ORCA_BIN:-/home/sanya/.config/orca/linux-orca-cli-shim/orca}"
if [ ! -x "$ORCA_BIN" ]; then
  ORCA_BIN="orca"
fi

RUN_STATE_FILE="/home/sanya/orca/project/.claude/ecc-orca-run.json"
DEFAULT_RUN="run_31999544da31"
if [ -f "$RUN_STATE_FILE" ]; then
  RUN_ID="$(grep -o '"runId": "[^"]*"' "$RUN_STATE_FILE" | cut -d'"' -f4 || echo "$DEFAULT_RUN")"
else
  RUN_ID="$DEFAULT_RUN"
fi

MODE="check"
if [ "${1:-}" = "--worktrees" ] || [ "${1:-}" = "-w" ]; then
  MODE="worktrees"
elif [ "${1:-}" = "--run" ]; then
  RUN_ID="${2:-$RUN_ID}"
fi

if [ "$MODE" = "worktrees" ]; then
  "$ORCA_BIN" worktree ps --json 2>/dev/null | python3 -c '
import json, sys
try:
    content = sys.stdin.read().strip()
    data = json.loads(content) if content else {}
    res = data.get("result", {})
    wts = res.get("worktrees", []) if isinstance(res, dict) else (data if isinstance(data, list) else [])
    total = res.get("totalCount", len(wts)) if isinstance(res, dict) else len(wts)
    dirty = sum(1 for w in wts if isinstance(w, dict) and w.get("dirty", False))
    branches = [w.get("branch", w.get("name", "unknown")) for w in wts[:4] if isinstance(w, dict)]
    branch_str = ", ".join(branches)
    print(f"📊 [ORCA-WT] Total worktrees: {total} | Dirty: {dirty}")
    if branch_str:
        print(f"   Active branches: {branch_str}")
except Exception as e:
    print(f"⚠️  [ORCA-WT] Could not parse worktrees: {e}")
'
  exit 0
fi

# Default: summarize orchestration run
"$ORCA_BIN" orchestration check --run "$RUN_ID" 2>&1 | python3 -c '
import sys

raw = sys.stdin.read()
lines = raw.strip().split("\n")
running = [l for l in lines if "state=running" in l]
ready = [l for l in lines if "state=ready" in l]
succeeded = [l for l in lines if "state=succeeded" in l]
failed = [l for l in lines if "state=failed" in l]
stopped = [l for l in lines if "state=stopped" in l]

print(f"🤖 [ORCA-SLIM] Run: '"$RUN_ID"' | Active: {len(running)} | Ready: {len(ready)} | Done: {len(succeeded)} | Failed: {len(failed)}")
if failed:
    for f in failed[:2]:
        print(f"   ❌ Failed: {f[:90]}")
if running:
    for r in running[:3]:
        parts = r.split()
        disp = parts[0] if parts else ""
        print(f"   ⚡ Running: {disp}")
if not running and not ready and not failed:
    print("   ✅ All workers settled / queue idle.")
'
