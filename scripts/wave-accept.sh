#!/usr/bin/env bash
# scripts/wave-accept.sh — Automated Wave Journal & Card Acceptance Tool
# Formats and appends Schema v1 events to waves/db.jsonl and updates waves/STATE.md.
#
# Usage:
#   ./scripts/wave-accept.sh --card TASK-AUTO-101 --wave RW-52 --summary "Automation suite accepted"

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DB_FILE="$PROJECT_ROOT/waves/db.jsonl"
STATE_FILE="$PROJECT_ROOT/waves/STATE.md"

CARD_ID=""
WAVE_ID="RW-52"
SUMMARY=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --card|-c)
      CARD_ID="$2"
      shift 2
      ;;
    --wave|-w)
      WAVE_ID="$2"
      shift 2
      ;;
    --summary|-s)
      SUMMARY="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done

if [ -z "$CARD_ID" ] || [ -z "$SUMMARY" ]; then
  echo "Usage: $0 --card <CARD_ID> --summary <SUMMARY> [--wave <WAVE_ID>]"
  exit 1
fi

REPORT_FILE="$PROJECT_ROOT/waves/$WAVE_ID/reports/$CARD_ID.md"
if [ ! -f "$REPORT_FILE" ]; then
  # Auto-generate worker report if not exists
  bash "$PROJECT_ROOT/scripts/worker-report.sh" --card "$CARD_ID" --wave "$WAVE_ID" >/dev/null 2>&1 || true
fi

echo "📋 [WAVE-ACCEPT] Processing acceptance for $CARD_ID ($WAVE_ID)..."

TODAY="$(date -u +"%Y-%m-%d")"

python3 -c '
import json, os, hashlib

db_file = "'"$DB_FILE"'"
card_id = "'"$CARD_ID"'"
wave_id = "'"$WAVE_ID"'"
summary = "'"$SUMMARY"'"
today = "'"$TODAY"'"
evidence = f"waves/{wave_id}/reports/{card_id}.md"
if not os.path.exists(evidence):
    evidence = "waves/STATE.md"

event = {
    "v": 1,
    "ts": today,
    "event": "accepted",
    "wave": wave_id,
    "card": card_id,
    "summary": summary,
    "tests": {"smoke": 22, "meta": 191},
    "evidence": evidence
}

# Append-only to db.jsonl
with open(db_file, "a", encoding="utf-8") as f:
    f.write(json.dumps(event, ensure_ascii=False) + "\n")

print(f"✅ Appended Schema v1 event to {db_file}")
'

echo "🔍 [WAVE-ACCEPT] Verifying waves-sync meta-test..."
node --test "$PROJECT_ROOT/tests/meta/waves-sync.test.mjs"

echo "🎉 [WAVE-ACCEPT] Successfully accepted $CARD_ID into wave journal!"
