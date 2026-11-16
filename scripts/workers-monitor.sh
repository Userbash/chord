#!/usr/bin/env bash
# scripts/workers-monitor.sh
#
# Скрипт автоматического отслеживания воркеров Orca Orchestration и контроля ресурсов.
# Полностью автоматизирует повторяющуюся рутину координатора:
# 1. Замер Aggregate RSS (память воркеров) против лимита WORKER_RAM_LIMIT (25 GiB).
# 2. Проверка активных воркеров в Run.
# 3. Автоматическое освобождение settled/завершённых терминалов (worker-release).
# 4. Проверка входящих сообщений (check).
#
# Запуск: ./scripts/workers-monitor.sh [run_id]

set -euo pipefail

RUN_ID="${1:-run_51e28348eb16}"
LIMIT_GIB=25

echo "================================================================================"
echo "🤖 [WORKERS-MONITOR] Orca Orchestration Autonomous Resource & Status Inspector"
echo "================================================================================"
echo "🎯 Active Run : ${RUN_ID}"

# 1. Aggregate RSS Memory Check
echo "🔍 1. Inspecting System Memory & Aggregate Worker RSS..."
MEM_STATS=$(ps -eo rss,comm | awk '/opencode|agy|cargo|rustc|orca/ {s+=$1} END {printf "%.2f", s/1024/1024}')
echo "   📊 Aggregate Worker RSS: ${MEM_STATS} GiB / ${LIMIT_GIB} GiB limit"

# 2. List Supervised Workers
echo "👥 2. Active Supervised Workers Status:"
orca orchestration worker-list --run "${RUN_ID}" || true

# 3. Auto-Reclaim Settled Terminals
echo "🧹 3. Auto-Releasing Settled Terminals..."
RECLAIMED=0
for ctx in $(orca orchestration worker-list --run "${RUN_ID}" 2>/dev/null | awk '/terminalState=reclaimable/ || /state=succeeded.*terminalState=active/ {print $1}' | sed 's/dispatchId=//'); do
  if [ -n "$ctx" ]; then
    echo "   ♻️ Releasing settled terminal: $ctx"
    orca orchestration worker-release --dispatch "$ctx" 2>/dev/null || true
    RECLAIMED=$((RECLAIMED + 1))
  fi
done
if [ "$RECLAIMED" -eq 0 ]; then
  echo "   ✅ All terminals cleanly managed (0 settled terminals pending release)."
fi

# 4. Check Messages
echo "📬 4. Orchestration Mailbox Check:"
orca orchestration check --run "${RUN_ID}" || true
echo "================================================================================"
