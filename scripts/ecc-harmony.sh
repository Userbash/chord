#!/usr/bin/env bash
# scripts/ecc-harmony.sh — DSH-Superagent CLI helper for ECC 2.0 & Discord-Lab
set -euo pipefail

PROJECT_ROOT="/home/sanya/orca/project"
ROUTER="$PROJECT_ROOT/scripts/harmony-router.mjs"

CMD="${1:-matrix}"
shift || true

case "$CMD" in
  matrix|admission-matrix)
    node "$ROUTER" matrix
    ;;
  mandate)
    TASK_ID="${1:-TASK-702}"
    node "$ROUTER" mandate "$TASK_ID"
    ;;
  verify|settle)
    TASK_ID="${1:-TASK-702}"
    node "$ROUTER" verify "$TASK_ID"
    ;;
  admission)
    TASK_ID="${1:-TASK-702}"
    node "$ROUTER" admission "$TASK_ID"
    ;;
  locks)
    node "$ROUTER" locks
    ;;
  outbox)
    tail -n 25 "$PROJECT_ROOT/waves/RW-41-ECC-INTEGRATION/dsh-outbox.jsonl" 2>/dev/null || echo "Outbox empty."
    ;;
  status)
    node "$ROUTER" status
    ;;
  help|--help|-h)
    echo "Usage: scripts/ecc-harmony.sh <command> [args]"
    echo ""
    echo "Commands:"
    echo "  matrix             Show DAG Admission Matrix (Ready vs Blocked)"
    echo "  mandate <task_id>  Generate Immutable Worker Mandate"
    echo "  verify <task_id>   Run DSH Arbiter to verify tests & settle task"
    echo "  admission <id>     Evaluate admission for specific task"
    echo "  locks              Show active file mutex locks"
    echo "  outbox             Show latest DSH Transaction Outbox records"
    echo "  status             Show Kanban board card statistics"
    ;;
  *)
    node "$ROUTER" "$CMD" "$@"
    ;;
esac
