#!/usr/bin/env bash
# scripts/sync-push.sh — Zero-Token Synchronous Multi-Remote Git Pusher
# Usage:
#   ./scripts/sync-push.sh
#   ./scripts/sync-push.sh --force

set -euo pipefail

CURRENT_BRANCH="$(git symbolic-ref --short HEAD 2>/dev/null || git rev-parse --short HEAD)"
echo "🔄 [SYNC-PUSH] Target branch: $CURRENT_BRANCH"

# Check remotes
REMOTES=()
for r in origin public; do
  if git remote | grep -q "^$r$"; then
    REMOTES+=("$r")
  fi
done

if [ ${#REMOTES[@]} -eq 0 ]; then
  echo "❌ No configured remotes (origin/public) found."
  exit 1
fi

echo "🚀 Pushing to remotes: ${REMOTES[*]}..."
for r in "${REMOTES[@]}"; do
  echo "   Pushing to $r/$CURRENT_BRANCH..."
  git push "$@" "$r" "$CURRENT_BRANCH"
done

echo "✅ [SYNC-PUSH] Successfully synchronized all remotes!"
