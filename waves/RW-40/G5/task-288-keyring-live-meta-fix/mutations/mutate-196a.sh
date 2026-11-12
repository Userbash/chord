#!/usr/bin/env bash
# Пять ручных мутаций task-288 с побайтовым откатом.
set -uo pipefail
cd /home/user/orca/workspaces/project/bugfix-task-288-keyring-live-meta-fix || exit 2

export CARGO_TARGET_DIR=/home/user/orca/project/rust/target
SRC=rust/crates/discord-platform/src/keyring.rs
TEST=rust/crates/discord-platform/tests/keyring_contract.rs
LOG=/tmp/opencode/mutations.log
: > "$LOG"

BACKUP=/tmp/opencode/mut-backup
mkdir -p "$BACKUP"
cp "$SRC" "$BACKUP/keyring.rs.orig"
cp "$TEST" "$BACKUP/keyring_contract.rs.orig"
MD5_SRC_ORIG=$(md5sum "$SRC" | cut -d' ' -f1)
MD5_TEST_ORIG=$(md5sum "$TEST" | cut -d' ' -f1)
echo "orig md5 src=$MD5_SRC_ORIG test=$MD5_TEST_ORIG" >> "$LOG"

apply_replace() { # file, from, to
  python3 - "$1" "$2" "$3" <<'PY'
import sys
path, old, new = sys.argv[1], sys.argv[2], sys.argv[3]
text = open(path, encoding="utf-8").read()
n = text.count(old)
if n != 1:
    print(f"ABORT: pattern occurs {n} times in {path}")
    sys.exit(3)
open(path, "w", encoding="utf-8").write(text.replace(old, new))
PY
}

restore() {
  cp "$BACKUP/keyring.rs.orig" "$SRC"
  cp "$BACKUP/keyring_contract.rs.orig" "$TEST"
  local s t
  s=$(md5sum "$SRC" | cut -d' ' -f1)
  t=$(md5sum "$TEST" | cut -d' ' -f1)
  if [ "$s" != "$MD5_SRC_ORIG" ] || [ "$t" != "$MD5_TEST_ORIG" ]; then
    echo "RESTORE-MISMATCH src=$s test=$t" >> "$LOG"
    exit 4
  fi
  echo "restore-ok src=$s test=$t" >> "$LOG"
}

run_mutation() { # id file from to cmd
  local id="$1" file="$2" from="$3" to="$4" cmd="$5"
  echo "=== $id file=$file" >> "$LOG"
  if ! apply_replace "$file" "$from" "$to"; then
    echo "$id APPLY-FAILED" >> "$LOG"
    restore
    return 1
  fi
  echo "diff:" >> "$LOG"
  diff -u "$BACKUP/$(basename "$file").orig" "$file" >> "$LOG" 2>&1
  local out rc
  out=$(eval "$cmd" 2>&1)
  rc=$?
  echo "$out" | tail -12 >> "$LOG"
  echo "$id EXIT=$rc" >> "$LOG"
  restore
  if [ "$rc" -eq 0 ]; then
    echo "$id SURVIVED" >> "$LOG"
    return 1
  fi
  echo "$id KILLED" >> "$LOG"
  return 0
}

CARGO_TEST='cargo test --offline --manifest-path rust/Cargo.toml -p discord-platform --test safe_storage_contract'
META_TEST='node --test tests/meta/keyring-live.test.mjs'

run_mutation M1 "$SRC" \
  'pub const ENCRYPTED_TOKEN_PREFIX: &str = "dQw4w9WgXcQ:";' \
  'pub const ENCRYPTED_TOKEN_PREFIX: &str = "dQw4w9WgXcQ";' \
  "$CARGO_TEST"

run_mutation M2 "$SRC" \
  'raw.trim().starts_with(ENCRYPTED_TOKEN_PREFIX)' \
  'raw.starts_with(ENCRYPTED_TOKEN_PREFIX)' \
  "$CARGO_TEST"

run_mutation M3 "$SRC" \
  '        None => return Err(SafeStorageMigrationError::InvalidBase64),' \
  '        None => return Err(SafeStorageMigrationError::EmptyPayload),' \
  "$CARGO_TEST"

run_mutation M4 "$SRC" \
  '        if self.migrated {
            return Ok(false);
        }
        if !is_safe_storage_token(raw_entry) {' \
  '        if !is_safe_storage_token(raw_entry) {' \
  "$CARGO_TEST"

run_mutation M5 "$TEST" \
  'const RSS_LIMIT_KILOBYTES: u64 = 4 * 1024;' \
  'const RSS_LIMIT_KILOBYTES: u64 = 64 * 1024;' \
  "$META_TEST"

echo "=== final md5:" >> "$LOG"
md5sum "$SRC" "$TEST" >> "$LOG"
grep -E 'KILLED|SURVIVED|APPLY-FAILED|MISMATCH|restore-ok' "$LOG"
