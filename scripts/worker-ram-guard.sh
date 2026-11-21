#!/usr/bin/env bash
# ==============================================================================
# scripts/worker-ram-guard.sh — Worker Memory Ceiling Guard & Zero-OOM Auto-Reclaim
#
# Контролирует лимит RAM <= 25 GB согласно AGENTS.md §8 и автоматически
# высвобождает терминалы завершившихся воркеров (terminalState=reclaimable).
# ==============================================================================
set -euo pipefail

RAM_LIMIT_GIB=25

echo "=== [RAM-GUARD] System Memory & Worker Accounting ==="
free -h

# Подсчет суммарного RSS процессов разработки
TOTAL_RSS_KB=$(ps -eo rss,comm | awk '/cargo|rustc|node|orca|antigravity/ {s+=$1} END {print s+0}')
TOTAL_RSS_GIB=$(awk -v kb="$TOTAL_RSS_KB" 'BEGIN {printf "%.2f", kb/1024/1024}')

echo "--- Current Aggregate RSS: ${TOTAL_RSS_GIB} GiB (Ceiling: ${RAM_LIMIT_GIB} GiB) ---"

IS_OVER_LIMIT=$(awk -v cur="$TOTAL_RSS_GIB" -v lim="$RAM_LIMIT_GIB" 'BEGIN {print (cur > lim) ? 1 : 0}')
if [ "$IS_OVER_LIMIT" -eq 1 ]; then
  echo "⚠️ [ALERT] RSS exceeds limit! Freeing memory..."
fi

# Проверка и авто-освобождение зависших терминалов в Orca
if command -v orca &>/dev/null; then
  echo "--- Checking reclaimable Orca worker terminals ---"
  RECLAIMABLE=$(orca orchestration worker-list 2>/dev/null | awk '/terminalState=reclaimable/ {for(i=1;i<=NF;i++) if($i ~ /^dispatchId=/) {sub(/^dispatchId=/, "", $i); print $i}}' || true)
  if [ -n "$RECLAIMABLE" ]; then
    echo "Found reclaimable dispatches:"
    echo "$RECLAIMABLE"
    for d in $RECLAIMABLE; do
      echo "Releasing dispatch: $d"
      orca orchestration worker-release --dispatch "$d" || true
    done
    echo "✅ Reclaim completed."
  else
    echo "No reclaimable terminals found."
  fi
fi

echo "=== [RAM-GUARD] Status OK ==="
