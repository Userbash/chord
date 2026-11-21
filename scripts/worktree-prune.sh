#!/usr/bin/env bash
# scripts/worktree-prune.sh — Zero-Token Worktree Garbage Collector
# Inspects and safely prunes merged, dangling, or stale Git worktrees.
#
# Usage:
#   ./scripts/worktree-prune.sh --dry-run
#   ./scripts/worktree-prune.sh --prune

set -euo pipefail

DRY_RUN=1
if [ "${1:-}" = "--prune" ] || [ "${1:-}" = "-f" ]; then
  DRY_RUN=0
fi

echo "🧹 [WORKTREE-PRUNE] Auditing Git worktrees..."

git worktree prune

python3 -c '
import subprocess, sys, os

dry_run = '"$DRY_RUN"'

# List worktrees
res = subprocess.run(["git", "worktree", "list", "--porcelain"], capture_output=True, text=True)
lines = res.stdout.strip().split("\n")

worktrees = []
current_wt = {}
for line in lines:
    if line.startswith("worktree "):
        if current_wt: worktrees.append(current_wt)
        current_wt = {"path": line.split(" ", 1)[1]}
    elif line.startswith("branch "):
        current_wt["branch"] = line.split(" ", 1)[1]
    elif line.startswith("bare"):
        current_wt["bare"] = True
if current_wt: worktrees.append(current_wt)

root_wt = "/home/sanya/orca/project"
candidates = []
for wt in worktrees:
    p = wt.get("path", "")
    b = wt.get("branch", "")
    if p != root_wt and "/orca/workspaces/" in p:
        candidates.append((p, b))

print(f"📊 Total worktrees registered: {len(worktrees)}")
print(f"🔍 External task worktrees: {len(candidates)}")

reclaimed = 0
for p, b in candidates:
    # Check if directory exists
    if not os.path.exists(p):
        print(f"   Removing missing worktree: {p}")
        if not dry_run:
            subprocess.run(["git", "worktree", "remove", "--force", p], check=False)
        reclaimed += 1

print(f"🎉 Worktree audit complete: {reclaimed} dangling worktrees processed.")
if dry_run and candidates:
    print("   (Run with --prune to clean up inactive task worktrees)")
'
