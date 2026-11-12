#!/usr/bin/env bash
# 5 mandated mutations + exploratory probes for REVIEW-195A (task-286)
set -u
cd /home/user/orca/workspaces/project/bugfix-task-286-dispatch-ui-forwarded-kinds-fix || exit 1
SRC=rust/gui-spike/ui/features/dispatch-model.js
ORIG=/tmp/opencode/tr195a/dispatch-model.js.orig
OUT=/tmp/opencode/tr195a
TEST=tests/unit/dispatch_ui.test.mjs

restore() {
  cp "$ORIG" "$SRC"
  local md5
  md5=$(md5sum "$SRC" | cut -d' ' -f1)
  if [ "$md5" != "9c7439d1db386a89fbd2347a95a207a2" ]; then
    echo "RESTORE FAILED md5=$md5"; exit 3
  fi
}

run_mutation() { # $1 label, $2 applier
  local label=$1 applier=$2
  echo "================ $label ================"
  node "$OUT/apply-$applier.mjs" || { echo "APPLY FAILED"; restore; return; }
  echo "--- diff vs clean ---"
  diff -u "$ORIG" "$SRC" | sed -n '3,40p'
  node --test "$TEST" > "$OUT/$applier.log" 2>&1
  local fails
  fails=$(grep -E "^ℹ fail" "$OUT/$applier.log" | head -1 | tr -dc '0-9')
  grep -E "^ℹ (tests|pass|fail)" "$OUT/$applier.log" | sort -u
  echo "failing assertions:"
  grep -E "^✖ [A-Z]" "$OUT/$applier.log" | sort -u | sed 's/^/  /'
  grep -E "AssertionError" "$OUT/$applier.log" | sort -u | head -6 | sed 's/^/  /'
  if [ "${fails:-0}" -gt 0 ]; then echo "VERDICT: KILLED (fail=$fails)"; else echo "VERDICT: SURVIVED (fail=0)"; fi
  restore
  echo "restored md5=$(md5sum "$SRC" | cut -d' ' -f1)"
}

restore
run_mutation "M1 drop CHANNEL_CREATE (extended kind removed)" m1
run_mutation "M2 MESSAGE_DELETE -> MESSAGE_DELETE_EXTRA (false wire name)" m2
run_mutation "M3 drop MESSAGE_REACTION_REMOVE (raw kind removed)" m3
run_mutation "M4 duplicate TYPING_START (31 entries, 30 unique)" m4
run_mutation "M5 THREAD_CREATE -> THREAD_CREATED (typo)" m5
run_mutation "P1 probe: order change, same 30 names" p1
run_mutation "P2 probe: filter bypass (!FORWARDED.has -> false)" p2
echo "FINAL md5=$(md5sum $SRC | cut -d' ' -f1)"
