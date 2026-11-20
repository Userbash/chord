#!/usr/bin/env bash
# scripts/worker-preflight.sh
# Автоматический скрипт онбординга воркера.
# Выдает компактную сводку окружения, файлов и правил без расхода токенов LLM.

set -euo pipefail

TASK_ID="${1:-""}"
COMPONENT="${2:-""}"

echo "=== WORKER PREFLIGHT SUMMARY (Zero-Token Setup) ==="
echo "Branch: $(git branch --show-current)"
echo "Head Commit: $(git rev-parse --short HEAD)"
echo "Host Memory Free: $(free -h | awk '/Mem:/ {print $4}')"
echo "Active Workers Count: $(orca orchestration worker-list 2>/dev/null | grep -c "state=ready" || echo 0)"

if [ -n "$COMPONENT" ]; then
  echo ""
  echo "=== RELEVANT COMPONENT FILES ==="
  find rust/gui-spike/ui -name "*${COMPONENT}*" 2>/dev/null | head -n 10 || true
  find rust/gui-spike/src -name "*${COMPONENT}*" 2>/dev/null | head -n 10 || true
  find tests/unit -name "*${COMPONENT}*" 2>/dev/null | head -n 5 || true
fi

echo ""
echo "=== FAST VERIFICATION COMMANDS ==="
echo "Impact tests: node scripts/test-impact.mjs"
echo "Generate tokens: node scripts/generate-cleanroom-tokens.mjs"
echo "Generate IPC: node scripts/generate-ipc-bridges.mjs"
echo "Generate report: bash scripts/generate-worker-report.sh"
echo "==================================================="
