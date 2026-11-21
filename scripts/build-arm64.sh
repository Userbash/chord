#!/usr/bin/env bash
# Real Chord ARM64 builds and official Type 2 AppImage packaging. No downloads.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if ! command -v python3 >/dev/null 2>&1; then
  echo "build-arm64: EnvFail: Python 3.11 or newer is required" >&2
  exit 2
fi
exec python3 "$ROOT/scripts/arm64/build.py" "$@"
