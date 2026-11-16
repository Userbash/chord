#!/usr/bin/env bash
# scripts/ci-pause.sh — Dynamic CI/CD Pause & Resume Manager
# Toggles pause status across all GitHub Action workflows without manual YAML edits.
#
# Usage:
#   ./scripts/ci-pause.sh --status
#   ./scripts/ci-pause.sh --pause 2026-12-01
#   ./scripts/ci-pause.sh --resume

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORKFLOWS_DIR="$PROJECT_ROOT/.github/workflows"

if [ ! -d "$WORKFLOWS_DIR" ]; then
  echo "❌ Directory $WORKFLOWS_DIR not found."
  exit 1
fi

ACTION="${1:-status}"
shift || true

function show_status() {
  local total
  total="$(find "$WORKFLOWS_DIR" -name "*.yml" | wc -l)"
  local paused
  paused="$(grep -rl "override_pause == 'run'" "$WORKFLOWS_DIR" | wc -l)"

  echo "📋 [CI-PAUSE] Total workflows: $total"
  if [ "$paused" -gt 0 ]; then
    echo "⏸️  [CI-PAUSE] Status: PAUSED ($paused/$total workflows paused with manual dispatch guard)"
    local sample_until
    sample_until="$(grep -ro "paused until [0-9.-]*" "$WORKFLOWS_DIR" | head -n 1 | awk '{print $NF}' || echo "active")"
    if [ -n "$sample_until" ]; then
      echo "   Scheduled resume date: $sample_until"
    fi
  else
    echo "▶️  [CI-PAUSE] Status: ACTIVE (Normal push/PR execution enabled)"
  fi
}

function pause_ci() {
  local until_date="${1:-2026-12-01}"
  echo "⏸️  [CI-PAUSE] Pausing CI workflows until $until_date..."

  python3 -c '
import glob, re

files = glob.glob("'"$WORKFLOWS_DIR"'/*.yml")
count = 0
guard_condition = "    if: github.event_name == '\''workflow_dispatch'\'' && github.event.inputs.override_pause == '\''run'\''"

for fpath in files:
    with open(fpath, "r", encoding="utf-8") as f:
        content = f.read()

    # If already guarded, update until date comment
    if "override_pause" in content:
        content = re.sub(r"# CI/CD paused until [0-9.-]+", "# CI/CD paused until '"$until_date"'", content)
        with open(fpath, "w", encoding="utf-8") as f:
            f.write(content)
        count += 1
        continue

    # Add guard to primary jobs
    # Look for first job definition
    modified = re.sub(r"(\n  [a-zA-Z0-9_-]+:\s*\n    runs-on:[^\n]+\n)", r"\1" + guard_condition + " # CI/CD paused until '"$until_date"'\n", content, count=1)
    if modified != content:
        with open(fpath, "w", encoding="utf-8") as f:
            f.write(modified)
        count += 1

print(f"✅ Successfully updated {count} workflows with pause condition.")
'
}

function resume_ci() {
  echo "▶️  [CI-PAUSE] Resuming CI workflows (removing pause guards)..."

  python3 -c '
import glob, re

files = glob.glob("'"$WORKFLOWS_DIR"'/*.yml")
count = 0

for fpath in files:
    with open(fpath, "r", encoding="utf-8") as f:
        content = f.read()

    if "override_pause" in content:
        # Remove guard condition
        cleaned = re.sub(r"[ \t]*if:[^\n]*override_pause[^\n]*\n", "", content)
        cleaned = re.sub(r"[ \t]*# CI/CD paused until [^\n]*\n", "", cleaned)
        with open(fpath, "w", encoding="utf-8") as f:
            f.write(cleaned)
        count += 1

print(f"✅ Successfully resumed {count} workflows.")
'
}

case "$ACTION" in
  --status|status)
    show_status
    ;;
  --pause|pause)
    pause_ci "${1:-2026-12-01}"
    ;;
  --resume|resume)
    resume_ci
    ;;
  *)
    echo "Usage: $0 {--status|--pause <YYYY-MM-DD>|--resume}"
    exit 1
    ;;
esac
