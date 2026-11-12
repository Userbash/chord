#!/usr/bin/env bash
# 5/5 manual mutation check for task-291 stats.rs (TR-198A)
set -u
WT=/home/user/orca/workspaces/project/feature-task-291-voice-stats-re22
F="$WT/rust/crates/discord-voice/src/stats.rs"
BAK=/tmp/opencode/stats.rs.orig
OUT=/tmp/opencode/mutations-198a.log
export CARGO_TARGET_DIR=/home/user/orca/project/rust/target

BASELINE_MD5=d5debf4cf1831c5a6e473fbedabfe447
cp "$F" "$BAK"
BASE=$(md5sum "$BAK" | awk '{print $1}')
echo "baseline md5: $BASE" | tee "$OUT"
if [ "$BASE" != "$BASELINE_MD5" ]; then echo "FATAL: baseline md5 mismatch"; exit 2; fi

run_mutant() {
  local id="$1" old="$2" new="$3" fn="$4"
  echo "=== MUTANT $id ($fn) ===" | tee -a "$OUT"
  cp "$BAK" "$F"
  python3 - "$F" "$old" "$new" <<'PY'
import sys
p, old, new = sys.argv[1], sys.argv[2], sys.argv[3]
s = open(p, encoding='utf-8').read()
n = s.count(old)
if n != 1:
    print(f"FATAL: pattern count {n}")
    sys.exit(3)
open(p, 'w', encoding='utf-8').write(s.replace(old, new))
PY
  if [ $? -ne 0 ]; then echo "FATAL: patch failed for $id" | tee -a "$OUT"; exit 3; fi
  echo "--- diff ---" | tee -a "$OUT"
  diff "$BAK" "$F" | tee -a "$OUT"
  cargo test --offline --manifest-path "$WT/rust/Cargo.toml" -p discord-voice --lib > /tmp/opencode/mut-$id.log 2>&1
  local rc=$?
  echo "--- test rc=$rc ---" | tee -a "$OUT"
  grep -E '^(test result|failures:|error\[|error:)' /tmp/opencode/mut-$id.log | head -20 | tee -a "$OUT"
  grep -E '^    stats::tests::' /tmp/opencode/mut-$id.log | sort -u | tee -a "$OUT"
  cp "$BAK" "$F"
  local after
  after=$(md5sum "$F" | awk '{print $1}')
  echo "--- rollback md5=$after ($([ "$after" = "$BASELINE_MD5" ] && echo OK || echo MISMATCH)) ---" | tee -a "$OUT"
  if [ "$rc" -eq 0 ]; then echo "MUTANT $id: SURVIVED" | tee -a "$OUT"; else echo "MUTANT $id: KILLED" | tee -a "$OUT"; fi
}

run_mutant M1 \
  '} else if ping_ms > PING_FINE_MAX_MS' \
  '} else if ping_ms >= PING_FINE_MAX_MS' \
  'ConnectionQuality::evaluate'

run_mutant M2 \
  'while self.samples.len() > MAX_PING_HISTORY_LEN {' \
  'while self.samples.len() >= MAX_PING_HISTORY_LEN {' \
  'PingHistory::push_ping'

run_mutant M3 \
  'if b == a + 1 && c == b + 1 {' \
  'if b == a && c == b {' \
  'PingHistory::note_ping_timeout'

run_mutant M4 \
  'let count = self.samples.len().min(PING_AVERAGE_WINDOW);' \
  'let count = self.samples.len();' \
  'PingHistory::average_ping_ms'

run_mutant M5 \
  'let total = delta_sent.saturating_add(delta_lost);' \
  'let total = delta_sent;' \
  'compute_outbound_loss_rate_pct'

echo "=== FINAL ===" | tee -a "$OUT"
FINAL=$(md5sum "$F" | awk '{print $1}')
echo "final md5=$FINAL ($([ "$FINAL" = "$BASELINE_MD5" ] && echo IDENTICAL || echo DIFFERS))" | tee -a "$OUT"
