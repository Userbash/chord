#!/usr/bin/env bash
# ==============================================================================
# scripts/git-sync.sh — Zero-Token Automated Git Sync, Verification & Multi-Remote Pipeline
#
# Delegates directly to scripts/git-sync.mjs for full zero-token automation,
# branch protection, pre-commit quality checks, and dual remote sync.
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

exec node "$SCRIPT_DIR/git-sync.mjs" "$@"
