#!/usr/bin/env bash
# Прогон мутаций RW-07: apply → cargo test (адресный файл) → log → restore+touch.
set -u
cd /home/sanya/orca/project/rust
S=/tmp/opencode/review-rw-07/scripts
L=/tmp/opencode/review-rw-07/logs
mkdir -p "$L"
for name in "$@"; do
  echo "=== $name ==="
  if ! python3 "$S/mutate.py" "$name"; then
    echo "APPLY-FAILED $name"
    "$S/restore.sh" > /dev/null
    exit 1
  fi
  cargo test --offline -p discord-core --test dispatch_router > "$L/$name.log" 2>&1
  rc=$?
  echo "# EXIT=$rc" >> "$L/$name.log"
  echo "rc=$rc"
  grep -E "^(test [a-z_]+ (ok|FAILED)|test result:|error(\[|:))" "$L/$name.log" | head -30
  "$S/restore.sh" > /dev/null
done
echo "all done"
