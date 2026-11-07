#!/usr/bin/env bash
# L02-UI-MUTATION evidence runner (G2 Frontend, 2026-10-24).
# ВАЖНО: продукт и тесты репозитория не трогаются. Все мутации применяются
# только к копии в /tmp/opencode/l02-ui-mutation; после каждого мутанта файл
# восстанавливается из .pristine и md5 сверяется.
set -uo pipefail

WT=/tmp/opencode/l02-ui-mutation
LOG=/home/sanya/orca/project/waves/RW-40/PM/l02-ui/logs
MODEL="$WT/rust/gui-spike/ui/features/dispatch-model.js"
APP="$WT/rust/gui-spike/ui/app.js"
PRISTINE="$WT/.pristine"

restore() {
  cp -a "$PRISTINE/dispatch-model.js" "$MODEL"
  cp -a "$PRISTINE/app.js" "$APP"
}

check_restored() {
  local want_model want_app
  want_model=$(md5sum "$PRISTINE/dispatch-model.js" | cut -d' ' -f1)
  want_app=$(md5sum "$PRISTINE/app.js" | cut -d' ' -f1)
  local got_model got_app
  got_model=$(md5sum "$MODEL" | cut -d' ' -f1)
  got_app=$(md5sum "$APP" | cut -d' ' -f1)
  if [ "$want_model" != "$got_model" ] || [ "$want_app" != "$got_app" ]; then
    echo "RESTORE-MISMATCH model=$got_model app=$got_app"
    return 1
  fi
  echo "restore-ok model=$got_model app=$got_app"
  return 0
}

# apply_patch <file> <old> <new>  (anchor обязан встречаться ровно один раз)
apply_patch() {
  python3 - "$1" "$2" "$3" <<'PY'
import sys
path, old, new = sys.argv[1], sys.argv[2], sys.argv[3]
text = open(path, encoding="utf-8").read()
n = text.count(old)
if n != 1:
    sys.exit(f"ANCHOR-COUNT {n} in {path}: {old[:60]!r}")
open(path, "w", encoding="utf-8").write(text.replace(old, new))
PY
}

run_one() {
  local name="$1" expected="$2"; shift 2
  local out="$LOG/mutation-$name.log"
  restore
  local before
  before=$(md5sum "$MODEL" "$APP" | awk '{print $1}' | paste -sd,)
  {
    echo "### MUTANT $name"
    echo "expected-caught-by: $expected"
    echo "md5-before: $before"
  } > "$out"
  if ! "$@" >> "$out" 2>&1; then
    echo "patch-error" >> "$out"
    restore
    return 1
  fi
  echo "md5-mutated: $(md5sum "$MODEL" "$APP" | awk '{print $1}' | paste -sd,)" >> "$out"
  echo "command: node --test --test-reporter=spec tests/unit/dispatch_ui.test.mjs (cwd=$WT)" >> "$out"
  ( cd "$WT" && node --test --test-reporter=spec tests/unit/dispatch_ui.test.mjs ) >> "$out" 2>&1
  local code=$?
  echo "exit=$code" >> "$out"
  restore >> "$out" 2>&1
  check_restored >> "$out" 2>&1
  echo "---- killed? expected: $expected; exit=$code" >> "$out"
  grep -E "^ℹ (tests|pass|fail)|^✖ " "$out" | head -40
  echo "log: $out"
}

# ---------- M1: P2-1, ранний return на пустом READY (dispatch-model) ----------
m1() {
  apply_patch "$MODEL" \
'  const order = guilds.map((guild) => guild.id);
  const fallbackSelf' \
'  if (guilds.length === 0) return [];
  const order = guilds.map((guild) => guild.id);
  const fallbackSelf'
}

# ---------- M2: P2-2, asString для READY-fallback self ID ----------
m2() {
  apply_patch "$MODEL" \
'const fallbackSelf = asSnowflake(asRecord(data.user).id);' \
'const fallbackSelf = asString(asRecord(data.user).id);'
}

# ---------- M3: P2-2, asString в setDispatchSelfUserId ----------
m3() {
  apply_patch "$MODEL" \
'  const id = asSnowflake(value);
  if (id !== "") store.selfUserId = id;' \
'  const id = asString(value);
  if (id !== "") store.selfUserId = id;'
}

# ---------- M4: P2-1, ранний return в rail.applyGuilds (view) ----------
m4() {
  apply_patch "$APP" \
'      if (items.length === 0) {
        renderEmpty();
        return;
      }
      const previousSelected' \
'      if (items.length === 0) {
        return;
      }
      const previousSelected'
}

# ---------- M5: P2-2, asString на границе ready() IPC (app.js) ----------
m5() {
  apply_patch "$APP" \
'import { asSnowflake, createDispatchStore } from "./features/dispatch-model.js";' \
'import { asSnowflake, asString, createDispatchStore } from "./features/dispatch-model.js";'
  apply_patch "$APP" \
'      const userId = asSnowflake(result?.user?.id);' \
'      const userId = asString(result?.user?.id);'
}

run_one M1 "B16/B16b/C15/C15b (пустой READY)" m1
run_one M2 "B17 (READY fallback unsafe user.id)" m2
run_one M3 "B17 (setDispatchSelfUserId unsafe)" m3
run_one M4 "C15/C15b (рейл не очищается)" m4
run_one M5 "C16 (ready() IPC unsafe self ID)" m5
