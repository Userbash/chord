#!/usr/bin/env bash
# ==============================================================================
# scripts/rootless-test-runner.sh — Unprivileged Rootless Sandbox Test Runner
#
# Runs tests in a hardened, non-root sandbox:
# 1. Local Rootless Mode (Default): Uses bubblewrap (bwrap) with isolated /tmp tmpfs,
#    private PID namespace, and read-only host mounts (/usr, /etc, /lib).
# 2. Remote Node2 Mode (--remote-node2): Runs in rootless Podman on node2 under user 'doom'.
#
# Usage:
#   bash scripts/rootless-test-runner.sh node --test tests/unit/some.test.mjs
#   bash scripts/rootless-test-runner.sh --remote-node2 "podman run --rm alpine uname -a"
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

REMOTE_NODE2=false
if [ "${1:-}" = "--remote-node2" ]; then
  REMOTE_NODE2=true
  shift
fi

COMMAND=("$@")
if [ ${#COMMAND[@]} -eq 0 ]; then
  COMMAND=("node" "--test" "tests/unit/stage10-bff6-dave-voice-ui.test.mjs")
fi

echo "🛡️  [ROOTLESS-RUNNER] Initializing unprivileged execution sandbox..."

if [ "$REMOTE_NODE2" = true ]; then
  echo "🌐 [ROOTLESS-RUNNER] Dispatching remote execution to node2 (user: doom, rootless podman)..."
  ssh -o ConnectTimeout=5 -o BatchMode=yes node2 "sudo -u doom -i ${COMMAND[*]}"
  exit $?
fi

# Local Bubblewrap Rootless Sandbox
if command -v bwrap >/dev/null 2>&1; then
  echo "🔒 [ROOTLESS-RUNNER] Enforcing local Bubblewrap (bwrap) user sandbox..."
  
  BWRAP_ARGS=(
    --ro-bind /usr /usr
    --ro-bind /bin /bin
    --ro-bind /etc /etc
    --dev /dev
    --proc /proc
    --tmpfs /tmp
    --unshare-pid
    --die-with-parent
  )

  # Lib directories
  if [ -d /lib ]; then BWRAP_ARGS+=(--ro-bind /lib /lib); fi
  if [ -d /lib64 ]; then BWRAP_ARGS+=(--ro-bind /lib64 /lib64); fi
  if [ -d /opt ]; then BWRAP_ARGS+=(--ro-bind /opt /opt); fi

  # Environment paths (nvm / cargo / rustup)
  if [ -d "$HOME/.nvm" ]; then BWRAP_ARGS+=(--ro-bind "$HOME/.nvm" "$HOME/.nvm"); fi
  if [ -d "$HOME/.cargo" ]; then BWRAP_ARGS+=(--ro-bind "$HOME/.cargo" "$HOME/.cargo"); fi
  if [ -d "$HOME/.rustup" ]; then BWRAP_ARGS+=(--ro-bind "$HOME/.rustup" "$HOME/.rustup"); fi

  # Mount project workspace
  BWRAP_ARGS+=(
    --bind "$PROJECT_ROOT" "$PROJECT_ROOT"
    --chdir "$PROJECT_ROOT"
  )

  bwrap "${BWRAP_ARGS[@]}" "${COMMAND[@]}"
  exit $?
else
  echo "⚠️  [ROOTLESS-RUNNER] bwrap not found, falling back to standard execution."
  cd "$PROJECT_ROOT"
  exec "${COMMAND[@]}"
fi
