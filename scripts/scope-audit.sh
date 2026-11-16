#!/usr/bin/env bash
# scripts/scope-audit.sh — Automated Write-Scope Boundary Auditor
# Enforces AGENTS.md §2 & common-agents.md non-overlapping department write-scopes.
#
# Usage:
#   ./scripts/scope-audit.sh --dept g2_frontend
#   ./scripts/scope-audit.sh --dept g1_backend --base unstable

set -euo pipefail

DEPT="${AGENT_DEPT:-devops}"
BASE="HEAD~1"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dept|-d)
      DEPT="$2"
      shift 2
      ;;
    --base|-b)
      BASE="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done

CHANGED_FILES="$(git diff --name-only "$BASE"...HEAD 2>/dev/null || git status --porcelain | awk '{print $NF}')"

echo "🛡️  [SCOPE-AUDIT] Auditing file boundaries for department: $DEPT against $BASE..."

python3 -c '
import sys

dept = "'"$DEPT"'"
raw_files = """'"$CHANGED_FILES"'""".strip().split()

# Forbidden for all ordinary workers
golden_prefixes = ["logs/", "functions/", "evidence/"]
frozen_prefixes = ["rust/chord-gpui/"]
shared_core_files = ["state/types.rs", "state/reducer.rs", "state/mod.rs", "src/lib.rs", "PLAN.md", "STATUS.md", "ROADMAP.md", "TASKS.md"]

violations = []

for f in raw_files:
    # 0. Check frozen GPUI codebase (Tauri is the primary client)
    for fr in frozen_prefixes:
        if f.startswith(fr):
            violations.append(f"FROZEN GPUI: {f} (GPUI is frozen. All changes rejected; Tauri is the primary client.)")

    # 1. Check golden baselines
    for g in golden_prefixes:
        if f.startswith(g):
            violations.append(f"PROTECTED GOLDEN BASELINE: {f} (Read-only per AGENTS.md §6)")

    # 2. Check Tech Lead shared files
    for sc in shared_core_files:
        if f.endswith(sc) and dept != "tech_lead" and dept != "orchestrator_argon":
            violations.append(f"SHARED CORE FILE: {f} (Tech Lead / Architect only)")

    # 3. Department scope checks
    if dept == "g2_frontend":
        # Frontend allowed: rust/gui-spike/ui/**, tests/unit/*.test.mjs
        if not (f.startswith("rust/gui-spike/ui/") or f.startswith("tests/unit/") or f.startswith("waves/")):
            violations.append(f"OUT-OF-SCOPE FOR G2: {f}")
    elif dept == "g1_backend":
        # Backend allowed: rust/crates/*, rust/gui-spike/src/*.rs (except voice), rust/gui-spike/tests/
        if f.startswith("rust/gui-spike/ui/") or f.startswith("rust/crates/discord-voice"):
            violations.append(f"OUT-OF-SCOPE FOR G1: {f}")
    elif dept == "g0_routine":
        if f.endswith(".rs") or "Cargo.toml" in f:
            violations.append(f"OUT-OF-SCOPE FOR G0: {f} (Rust core restricted)")

if violations:
    print(f"❌ [SCOPE-AUDIT] FAILED: {len(violations)} boundary violation(s) detected:")
    for v in violations:
        print(f"   🚫 {v}")
    sys.exit(1)
else:
    print(f"✅ [SCOPE-AUDIT] PASSED: All {len(raw_files)} changed file(s) comply with {dept} write-scope.")
'
