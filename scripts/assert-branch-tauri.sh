#!/usr/bin/env bash
# ==============================================================================
# scripts/assert-branch-tauri.sh — Strict Branch Guard for Discord Lab
#
# Гарантирует, что все изменения, коммиты и воркеры работают ИСКЛЮЧИТЕЛЬНО
# в ветке tauri, предотвращая случайную модификацию main, beta, testing, unstable.
# ==============================================================================
set -euo pipefail

CURRENT_BRANCH="$(git branch --show-current)"

if [ "$CURRENT_BRANCH" != "tauri" ]; then
  echo "❌ [BRANCH GUARD ERROR] Current branch is '$CURRENT_BRANCH', but strictly expected 'tauri'!"
  echo "All changes must be confined exclusively to branch 'tauri'."
  exit 1
fi

echo "✅ [BRANCH GUARD OK] Verified: Current branch is 'tauri'."
