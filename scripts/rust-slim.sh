#!/usr/bin/env bash
# ==============================================================================
# scripts/rust-slim.sh — Rust Compiler Error Condenser (-95% LLM Tokens)
#
# Runs cargo check/test with --message-format=json and distills compiler output
# down to only essential errors, primary spans, and actionable compiler hints.
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
RUST_DIR="$PROJECT_ROOT/rust"

CARGO_CMD="${1:-check}"
shift 1 || true

cd "$RUST_DIR"

python3 -c '
import sys
import subprocess
import json

args = sys.argv[1:]
cargo_cmd = ["cargo", "'"$CARGO_CMD"'", "--message-format=json"] + args

proc = subprocess.Popen(cargo_cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)

errors = []
warnings = []

for line in proc.stdout:
    line = line.strip()
    if not line:
        continue
    try:
        data = json.loads(line)
    except json.JSONDecodeError:
        continue

    if data.get("reason") == "compiler-message":
        msg = data.get("message", {})
        level = msg.get("level")
        spans = msg.get("spans", [])
        primary = next((s for s in spans if s.get("is_primary")), None)

        if level == "error":
            code = msg.get("code", {}).get("code", "ERROR") if msg.get("code") else "ERROR"
            text = msg.get("message", "")
            file_name = primary.get("file_name", "unknown") if primary else "unknown"
            line_start = primary.get("line_start", 0) if primary else 0
            col_start = primary.get("column_start", 0) if primary else 0

            helps = [c.get("message") for c in msg.get("children", []) if c.get("level") == "help"]

            errors.append({
                "code": code,
                "text": text,
                "file": file_name,
                "line": line_start,
                "col": col_start,
                "helps": helps
            })

_, stderr_out = proc.communicate()

if errors:
    print(f"\n❌ [RUST-SLIM] Found {len(errors)} compilation error(s):")
    for idx, err in enumerate(errors, 1):
        err_code = err["code"]
        err_file = err["file"]
        err_line = err["line"]
        err_col = err["col"]
        err_text = err["text"]
        print(f"\n[{idx}] {err_code} at {err_file}:{err_line}:{err_col}")
        print(f"    🚫 {err_text}")
        for h in err["helps"]:
            print(f"    💡 Help: {h}")
    print()
    sys.exit(1)
elif proc.returncode != 0:
    print(f"\n❌ [RUST-SLIM] Cargo command failed with exit code {proc.returncode}:")
    for l in stderr_out.splitlines()[-10:]:
        print(f"    {l}")
    print()
    sys.exit(proc.returncode)
else:
    print("✅ [RUST-SLIM] Cargo check passed with 0 errors!")
    sys.exit(0)
' "$@"
