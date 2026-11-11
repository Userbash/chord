#!/bin/bash
LOG="/home/sanya/orca/project/waves/app-trace.log"
while true; do
  if ! pgrep -f discord-gui-spike > /dev/null 2>&1; then
    echo "=== APP CRASHED at $(date) ==="
    echo "--- LAST 20 LINES ---"
    tail -20 "$LOG"
    echo "--- ERRORS ---"
    grep -i 'error\|panic\|crash\|fatal\|BACKTRACE' "$LOG" | tail -10
    exit 1
  fi
  sleep 30
done
