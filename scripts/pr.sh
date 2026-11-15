#!/usr/bin/env bash
# pr.sh — Zero-Token CLI Wrapper for PR Inspection & Compliance Auditing

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PR_NUMBER="${2:-16}"
ACTION="${1:---audit}"

case "$ACTION" in
  --audit|audit)
    node "$SCRIPT_DIR/pr-ops.mjs" audit "$PR_NUMBER"
    ;;
  --inspect|--view|inspect|view)
    node "$SCRIPT_DIR/pr-ops.mjs" inspect "$PR_NUMBER"
    ;;
  --template|review-template|template)
    node "$SCRIPT_DIR/pr-ops.mjs" review-template "$PR_NUMBER"
    ;;
  --comment|comment)
    node "$SCRIPT_DIR/pr-ops.mjs" comment "$PR_NUMBER"
    ;;
  *)
    node "$SCRIPT_DIR/pr-ops.mjs" audit "$PR_NUMBER"
    ;;
esac
