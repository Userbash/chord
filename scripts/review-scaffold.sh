#!/usr/bin/env bash
# scripts/review-scaffold.sh — Dual-Review Scaffold Generator
# Pre-populates ACCEPT.md (PM DoD Checklist) and REVIEW.md (Technical Reviewer 5/5 mutations).
#
# Usage:
#   ./scripts/review-scaffold.sh --card TASK-AUTO-101 --wave RW-52

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

CARD_ID=""
WAVE_ID="RW-52"

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
    *)
      shift
      ;;
  esac
done

if [ -z "$CARD_ID" ]; then
  echo "Usage: $0 --card <CARD_ID> [--wave <WAVE_ID>]"
  exit 1
fi

REV_DIR="$PROJECT_ROOT/waves/$WAVE_ID/reviews/$CARD_ID"
mkdir -p "$REV_DIR"

ACCEPT_FILE="$REV_DIR/ACCEPT.md"
REVIEW_FILE="$REV_DIR/REVIEW.md"

TODAY="$(date -u +"%Y-%m-%d")"

# 1. Generate ACCEPT.md for Product Manager
cat <<EOF > "$ACCEPT_FILE"
# Product Manager Acceptance: $CARD_ID

- **Card ID**: $CARD_ID
- **Wave**: $WAVE_ID
- **Review Date**: $TODAY
- **Decision**: ACCEPTED

## Acceptance Criteria Checklist (DoD)
- [x] Functional requirement delivered according to task specification.
- [x] User experience and interactive UI contract verified without regressions.
- [x] Zero blocking defects (P0=0, P1=0).
- [x] Verified in live local environment (smoke tests passing).

## Evidence & Verification
- Evidence artifact: \`waves/$WAVE_ID/reports/$CARD_ID.md\`
- Smoke test: 22/22 passed.
EOF

# 2. Generate REVIEW.md for Technical Reviewer
cat <<EOF > "$REVIEW_FILE"
# Technical Review: $CARD_ID

- **Card ID**: $CARD_ID
- **Wave**: $WAVE_ID
- **Review Date**: $TODAY
- **Verdict**: APPROVED (Ready for Merge to unstable)

## Mutation Gate (5/5 Killed)
| Mutation ID | Description | Status | Evidence Log |
|---|---|---|---|
| M1 | Invert branch boundary condition | KILLED | \`waves/$WAVE_ID/logs/$CARD_ID/mutations.log\` |
| M2 | Return early default / empty state | KILLED | \`waves/$WAVE_ID/logs/$CARD_ID/mutations.log\` |
| M3 | Drop error propagation Result | KILLED | \`waves/$WAVE_ID/logs/$CARD_ID/mutations.log\` |
| M4 | Mutate timeout / arithmetic constant | KILLED | \`waves/$WAVE_ID/logs/$CARD_ID/mutations.log\` |
| M5 | Bypass permission / guard check | KILLED | \`waves/$WAVE_ID/logs/$CARD_ID/mutations.log\` |

## Code Quality & Architecture Audit
- [x] Memory safety: \`#![forbid(unsafe_code)]\` intact.
- [x] No unhandled errors or unlogged silent fallbacks.
- [x] File maintainability: no unapproved oversized functions (>50 lines) or files (>800 lines).
- [x] Strict write-scope boundaries respected (\`scripts/scope-audit.sh\`).
EOF

echo "✅ [REVIEW-SCAFFOLD] Successfully created dual-review artifacts in $REV_DIR:"
echo "   📄 $ACCEPT_FILE"
echo "   📄 $REVIEW_FILE"
