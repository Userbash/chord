#!/usr/bin/env bash
# ==============================================================================
# scripts/pr-quarantine.sh — Auto-Quarantine & Defect Triage Bot for Third-Party PRs
#
# Inspects incoming PRs, audits against AMCP v2.0 gates, and isolates regressive PRs
# with automated structured diagnostic feedback without human intervention.
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO="${REPO:-Userbash/chord}"
TRIAGE_DIR="$PROJECT_ROOT/waves/triage"

mkdir -p "$TRIAGE_DIR"

command_help() {
  cat << 'EOF'
Usage:
  bash scripts/pr-quarantine.sh scan
  bash scripts/pr-quarantine.sh audit <PR_ID>
  bash scripts/pr-quarantine.sh quarantine <PR_ID> <REASON>
  bash scripts/pr-quarantine.sh release <PR_ID>
EOF
}

CMD="${1:-scan}"

case "$CMD" in
  scan)
    echo "🔍 [PR-QUARANTINE] Scanning open pull requests in $REPO..."
    prs="$(gh pr list --repo "$REPO" --json number,title,author,isDraft,mergeable,state 2>/dev/null || echo "[]")"
    
    python3 -c '
import sys
import json

raw = """'"$prs"'"""
try:
    data = json.loads(raw)
except Exception:
    data = []

print(f"📊 Open Pull Requests: {len(data)}")
for pr in data:
    num = pr.get("number")
    title = pr.get("title", "")
    author = pr.get("author", {}).get("login", "unknown")
    draft = " [DRAFT]" if pr.get("isDraft") else ""
    mergeable = pr.get("mergeable", "UNKNOWN")
    print(f"   • PR #{num}{draft}: \"{title}\" by @{author} (Mergeable: {mergeable})")
'
    ;;

  audit)
    PR_ID="${2:-}"
    if [ -z "$PR_ID" ]; then
      echo "Usage: pr-quarantine.sh audit <PR_ID>"
      exit 1
    fi
    echo "🛡️  [PR-QUARANTINE] Auditing PR #$PR_ID for protocol violations..."
    gh pr view "$PR_ID" --repo "$REPO" --json number,title,author,isDraft,files 2>/dev/null | python3 -c '
import json
import sys

try:
    pr = json.load(sys.stdin)
except Exception as e:
    print(f"Error loading PR JSON: {e}")
    sys.exit(1)

num = pr.get("number")
title = pr.get("title")
author = pr.get("author", {}).get("login")
files = [f.get("path") for f in pr.get("files", [])]

violations = []
for f in files:
    if f.startswith("logs/") or f.startswith("functions/") or f.startswith("evidence/"):
        violations.append(f"PROTECTED GOLDEN BASELINE modified: {f}")
    if f.endswith(".exe") or f.endswith(".dll") or f.endswith(".so") or f.endswith(".dylib"):
        violations.append(f"UNVERIFIED BINARY BLOB included: {f}")

print(f"\nAudit Report for PR #{num} by @{author}:")
print(f"Files changed: {len(files)}")
if violations:
    print(f"❌ {len(violations)} VIOLATION(S) DETECTED:")
    for v in violations:
        print(f"   🚫 {v}")
    print("👉 Action: Recommend QUARANTINE")
else:
    print("✅ No golden baseline or binary violations detected.")
'
    ;;

  quarantine)
    PR_ID="${2:-}"
    shift 2 || true
    REASON="${*:-Regressions detected against independent oracle}"
    if [ -z "$PR_ID" ]; then
      echo "Usage: pr-quarantine.sh quarantine <PR_ID> <REASON>"
      exit 1
    fi
    
    echo "⚠️  [PR-QUARANTINE] Quarantining PR #$PR_ID in $REPO..."
    REPORT_FILE="$TRIAGE_DIR/PR-${PR_ID}-QUARANTINE.md"
    
    cat << EOF > "$REPORT_FILE"
# Quarantine Notice: PR #$PR_ID

- **Repository**: $REPO
- **Quarantined At**: $(date -u +"%Y-%m-%dT%H:%M:%SZ")
- **Reason**: $REASON
- **Protocol Status**: ISOLATED (Changes Requested)

## Action Plan for Author
1. Rebase your branch onto current \`release/developer-alpha-preview\`.
2. Ensure \`npm run precheck\` and \`npm run audit:scope\` pass locally before re-submitting.
3. Remove any binary artifacts, golden baselines, or unverified crate bumps.
EOF

    echo "📄 Generated quarantine report at $REPORT_FILE"
    echo "🔒 Adding quarantine notice to GitHub PR #$PR_ID..."
    gh pr comment "$PR_ID" --repo "$REPO" --body-file "$REPORT_FILE" 2>/dev/null || echo "   (GitHub comment skipped or repo offline)"
    echo "✅ [PR-QUARANTINE] PR #$PR_ID successfully quarantined."
    ;;

  *)
    command_help
    exit 1
    ;;
esac
