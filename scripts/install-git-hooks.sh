#!/usr/bin/env bash
# scripts/install-git-hooks.sh — Zero-Token Native Git Hooks Installer
# Installs zero-dependency pre-commit, commit-msg, and pre-push hooks into .git/hooks.
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

# 1. Pre-commit hook: runs zero-token precheck, auto-syncs TDD plan, checks branch safety
cat <<'EOF' > "$HOOKS_DIR/pre-commit"
#!/usr/bin/env bash
set -e

# Block changes to GPUI (Tauri is the primary client)
CURRENT_BRANCH="$(git symbolic-ref --short HEAD 2>/dev/null || echo 'tauri')"
if [ "$CURRENT_BRANCH" = "tauri" ]; then
  if git diff --cached --name-only | grep -E '^rust/chord-gpui/' >/dev/null 2>&1; then
    echo "❌ [GIT-HOOK] Изменения в GPUI (rust/chord-gpui/) ЗАПРЕЩЕНЫ на ветке tauri. Основной клиент — Tauri."
    exit 1
  fi
fi

# Run zero-token precheck & auto-sync TDD plan
if command -v node >/dev/null 2>&1; then
  node scripts/sync-tdd-plan.mjs --fix >/dev/null 2>&1 || true
  git add TDD-PLAN.md 2>/dev/null || true
  
  echo "🔍 [GIT-HOOK] Running fast pre-commit sanity check..."
  npm run precheck --silent || {
    echo "❌ [GIT-HOOK] Pre-commit check failed. Fix syntax/type errors before committing."
    exit 1
  }
  node scripts/security-shield.mjs --staged || {
    echo "❌ [GIT-HOOK] Security shield failed: secrets or forbidden unsafe blocks found."
    exit 1
  }
  node scripts/socrati-hub-check.mjs --staged || true
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
  echo "   Must follow Conventional Commits: <type>(<scope>): <subject>"
  echo "   Types: feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert"
  exit 1
fi
EOF
chmod +x "$HOOKS_DIR/commit-msg"
echo "   ✅ Installed .git/hooks/commit-msg"

# 3. Pre-push hook: ensures precheck passes before push
cat <<'EOF' > "$HOOKS_DIR/pre-push"
#!/usr/bin/env bash
set -e

echo "🚀 [GIT-HOOK] Running pre-push zero-token check..."
if command -v npm >/dev/null 2>&1; then
  npm run precheck --silent || {
    echo "❌ [GIT-HOOK] Pre-push check failed. Cannot push broken build."
    exit 1
  }
fi
EOF
chmod +x "$HOOKS_DIR/pre-push"
echo "   ✅ Installed .git/hooks/pre-push"

echo "🎉 [GIT-HOOKS] All Git hooks successfully installed and active!"
