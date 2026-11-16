#!/usr/bin/env bash
# scripts/install-git-hooks.sh — Zero-Token Native Git Hooks Installer
# Installs zero-dependency pre-commit and commit-msg hooks into .git/hooks.
#
# Usage:
#   ./scripts/install-git-hooks.sh

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HOOKS_DIR="$PROJECT_ROOT/.git/hooks"

if [ ! -d "$HOOKS_DIR" ]; then
  echo "❌ .git/hooks not found."
  exit 1
fi

echo "⚓ [GIT-HOOKS] Installing native Git hooks into $HOOKS_DIR..."

# 1. Pre-commit hook: runs npm run precheck (< 1 sec)
cat <<'EOF' > "$HOOKS_DIR/pre-commit"
#!/usr/bin/env bash
set -e

# Run zero-token precheck
if command -v npm >/dev/null 2>&1; then
  echo "🔍 [GIT-HOOK] Running fast pre-commit sanity check..."
  npm run precheck --silent || {
    echo "❌ [GIT-HOOK] Pre-commit check failed. Fix syntax/type errors before committing."
    exit 1
  }
fi
EOF
chmod +x "$HOOKS_DIR/pre-commit"
echo "   ✅ Installed .git/hooks/pre-commit"

# 2. Commit-msg hook: validates Conventional Commits format
cat <<'EOF' > "$HOOKS_DIR/commit-msg"
#!/usr/bin/env bash
set -e

MSG_FILE="$1"
COMMIT_MSG="$(head -n 1 "$MSG_FILE")"

# Regex for Conventional Commits
PATTERN="^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\([a-zA-Z0-9_.-]+\))?: .+"

if [[ ! "$COMMIT_MSG" =~ $PATTERN ]]; then
  echo "❌ [GIT-HOOK] Invalid commit message format: '$COMMIT_MSG'"
  echo "   Must follow Conventional Commits (AGENTS.md §4.1):"
  echo "   <type>(<scope>): <description>"
  echo "   Example: feat(voice): add Krisp noise suppression toggle"
  exit 1
fi
EOF
chmod +x "$HOOKS_DIR/commit-msg"
echo "   ✅ Installed .git/hooks/commit-msg"

echo "🎉 [GIT-HOOKS] Git hooks installed successfully!"
