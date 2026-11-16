#!/usr/bin/env bash
# ==============================================================================
# scripts/collab.sh — Multi-Human & Multi-AI Collaboration Protocol CLI Facade
#
# Unified command-line interface for the Agent Multi-Collaborator Protocol (AMCP v2.0):
#   - Manages distributed file locking (.collab/bus/locks.json)
#   - Handles inter-agent event publication & message routing
#   - Enforces pre-flight quality gates, write-scope boundaries, and zero-leak RAM rules
#   - Coordinates sync between git remotes and active AI worker slots
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BUS_DIR="$PROJECT_ROOT/.collab/bus"

mkdir -p "$BUS_DIR"

command_help() {
  cat << 'EOF'
================================================================================
🤝 CHORD / DISCORD-LAB MULTI-AGENT COLLABORATION CLI (AMCP v2.0)
================================================================================
Usage:
  bash scripts/collab.sh <subcommand> [args...]
  npm run collab <subcommand> [args...]

Subcommands:
  status                Show full collaboration dashboard (locks, events, workers, RAM)
  lock <file> [task]    Acquire exclusive lock on a file before editing
  unlock <file>         Release lock on an edited file
  locks                 List all currently active file locks
  publish <type> <msg>  Broadcast structured event to the inter-agent bus
  events [limit]        Display recent events from the bus (default: 10)
  send <to> <subj> <msg> Send peer-to-peer message to another worker or human
  inbox [worker_id]     Read worker's direct message inbox
  precheck              Execute fast sub-second precheck (Node syntax + TS + Meta)
  audit                 Validate write-scope boundaries against assigned department
  verify                Run full pre-flight verification before committing/pushing
  report                Generate standard PROCESS.md §4.2 Completion Report
  sync                  Synchronize code & locks with git remotes (origin + public)
  clean                 Run garbage collection on stale incremental targets

Examples:
  bash scripts/collab.sh status
  bash scripts/collab.sh lock rust/crates/discord-core/src/lib.rs TASK-123
  bash scripts/collab.sh unlock rust/crates/discord-core/src/lib.rs
  bash scripts/collab.sh publish CONTRACT_UPDATE "Updated VoiceState DTO schema"
  bash scripts/collab.sh verify
================================================================================
EOF
}

CURRENT_AGENT="${AGENT_ID:-${USER:-worker_local}}"

case "${1:-status}" in
  status)
    echo "================================================================================"
    echo "📊 COLLABORATION & MULTI-AGENT STATUS DASHBOARD"
    echo "================================================================================"
    echo "👤 Current Context Agent : @$CURRENT_AGENT"
    echo "🌿 Current Git Branch    : $(git branch --show-current)"
    echo "🔍 Working Copy Status   : $(git status -s | wc -l) modified/untracked files"
    echo ""
    node "$SCRIPT_DIR/agent-bus.mjs" status
    echo "--------------------------------------------------------------------------------"
    echo "🖥️  Worker Pools & Worktree Slots:"
    bash "$SCRIPT_DIR/orca-slim.sh" 2>/dev/null || echo "   (Orca daemon offline or in standalone mode)"
    echo "--------------------------------------------------------------------------------"
    echo "🧠 Host RAM & Worker Ceiling:"
    ps -eo rss,comm | awk '/opencode|agy|cargo|rustc|node/ {s+=$1} END {printf "   Active AI/Build RSS: %.2f GiB (Ceiling: 25.00 GiB)\n", s/1024/1024}'
    echo "================================================================================"
    ;;

  lock)
    FILE="${2:-}"
    TASK="${3:-TASK_UNASSIGNED}"
    if [ -z "$FILE" ]; then
      echo "❌ Error: Specify file to lock. Example: bash scripts/collab.sh lock src/lib.rs TASK-101"
      exit 1
    fi
    node "$SCRIPT_DIR/agent-bus.mjs" lock "$FILE" "$CURRENT_AGENT" "$TASK"
    ;;

  unlock)
    FILE="${2:-}"
    if [ -z "$FILE" ]; then
      echo "❌ Error: Specify file to unlock. Example: bash scripts/collab.sh unlock src/lib.rs"
      exit 1
    fi
    node "$SCRIPT_DIR/agent-bus.mjs" unlock "$FILE" "$CURRENT_AGENT"
    ;;

  locks)
    node "$SCRIPT_DIR/agent-bus.mjs" status | grep -A 10 "Active File Locks"
    ;;

  publish)
    TYPE="${2:-NOTICE}"
    shift 2 || true
    SUMMARY="${*:-No summary provided}"
    node "$SCRIPT_DIR/agent-bus.mjs" publish --from "$CURRENT_AGENT" --type "$TYPE" --summary "$SUMMARY"
    ;;

  events)
    LIMIT="${2:-10}"
    node "$SCRIPT_DIR/agent-bus.mjs" read --limit "$LIMIT"
    ;;

  send)
    TO="${2:-}"
    SUBJ="${3:-Collaboration message}"
    shift 3 || true
    BODY="${*:-No body}"
    if [ -z "$TO" ]; then
      echo "❌ Error: Specify recipient. Example: bash scripts/collab.sh send g1_backend 'DTO sync' 'Please re-export VoiceState'"
      exit 1
    fi
    node "$SCRIPT_DIR/agent-bus.mjs" send --from "$CURRENT_AGENT" --to "$TO" --subject "$SUBJ" --body "$BODY"
    ;;

  inbox)
    TARGET="${2:-$CURRENT_AGENT}"
    node "$SCRIPT_DIR/agent-bus.mjs" inbox "$TARGET"
    ;;

  precheck)
    node "$SCRIPT_DIR/precheck.mjs"
    ;;

  audit)
    bash "$SCRIPT_DIR/scope-audit.sh"
    ;;

  verify)
    echo "🚀 Starting Full Pre-Flight Collaboration Verification..."
    echo "1. Checking write-scope boundaries..."
    bash "$SCRIPT_DIR/scope-audit.sh"
    echo ""
    echo "2. Running fast sub-second precheck..."
    node "$SCRIPT_DIR/precheck.mjs"
    echo ""
    echo "3. Checking git status..."
    git status -s
    echo ""
    echo "✅ [COLLAB-VERIFY] All pre-flight checks PASSED cleanly."
    ;;

  report)
    bash "$SCRIPT_DIR/worker-report.sh" "$@"
    ;;

  sync)
    echo "🔄 Synchronizing with remotes and agent bus..."
    bash "$SCRIPT_DIR/sync-push.sh"
    ;;

  clean)
    node "$SCRIPT_DIR/worker-gc.mjs"
    ;;

  help|--help|-h)
    command_help
    ;;

  *)
    echo "Unknown subcommand: $1"
    command_help
    exit 1
    ;;
esac
