#!/usr/bin/env bash
# auto-loop.sh — Zero-Token CLI Wrapper for Autonomous Orchestration Pipeline
# Eliminates repetitive manual LLM operations: status, check, harvest, commit, push, dispatch.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

export ORCA_BIN="${ORCA_BIN:-/home/sanya/.config/orca/linux-orca-cli-shim/orca}"
export WORKER_RAM_LIMIT_MB="${WORKER_RAM_LIMIT_MB:-25600}"
export MAX_ACTIVE_WORKERS="${MAX_ACTIVE_WORKERS:-4}"

ACTION="${1:---status}"
shift || true

case "$ACTION" in
  --status|-s|status)
    node "$SCRIPT_DIR/auto-pipeline.mjs" --status
    ;;
  --step|-1|step)
    node "$SCRIPT_DIR/auto-pipeline.mjs" --step
    ;;
  --harvest|harvest)
    node "$SCRIPT_DIR/auto-pipeline.mjs" --harvest
    ;;
  --dispatch|dispatch)
    node "$SCRIPT_DIR/auto-pipeline.mjs" --dispatch
    ;;
  --watch|-w|watch)
    INTERVAL="${1:-15}"
    node "$SCRIPT_DIR/auto-pipeline.mjs" --watch "$INTERVAL"
    ;;
  --kanban|kanban)
    node "$SCRIPT_DIR/kanban-tool.mjs" "${@:-status}"
    ;;
  --test|test)
    node "$SCRIPT_DIR/test-runner-smart.mjs" "${@:-}"
    ;;
  *)
    echo "Usage: $0 [--status | --step | --watch [interval] | --harvest | --dispatch | --kanban | --test]"
    exit 1
    ;;
esac
