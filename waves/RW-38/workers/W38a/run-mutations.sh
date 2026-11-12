#!/usr/bin/env bash
# RW-38 / W38a: red-first — мутации app.js в КОПИИ (сдаваемый ui/app.js не трогаем).
# Каждая мутация: копия ui/ в /tmp/opencode/rw38-mut/<id>/, точечная замена в копии,
# прогон тестов через GUI_DOM_APP, фиксация pass/fail. Оригинал сверяется по md5.
set -u

REPO=/home/user/orca/project
OUT="$REPO/waves/RW-38/workers/W38a"
WORK=/tmp/opencode/rw38-mut
APP="$REPO/rust/gui-spike/ui/app.js"
TEST="$REPO/tests/unit/gui-spike-dom.test.mjs"

mkdir -p "$OUT" "$WORK"
BEFORE=$(md5sum "$APP" | cut -d' ' -f1)

summarize() {
  local log=$1
  local pass fail
  pass=$(grep -Eo 'ℹ pass [0-9]+' "$log" | tail -1 | grep -Eo '[0-9]+')
  fail=$(grep -Eo 'ℹ fail [0-9]+' "$log" | tail -1 | grep -Eo '[0-9]+')
  echo "${pass:-?} ${fail:-?}"
}

run_case() {
  local id=$1 file=$2 from=$3 to=$4
  local dir="$WORK/$id"
  rm -rf "$dir"
  mkdir -p "$dir"
  cp -r "$REPO/rust/gui-spike/ui" "$dir/ui"
  node - "$dir/ui/app.js" "$from" "$to" <<'NODE'
import { readFileSync, writeFileSync } from "node:fs";
const [file, from, to] = process.argv.slice(2);
const source = readFileSync(file, "utf8");
if (!source.includes(from)) {
  console.error(`МУТАЦИЯ НЕ ПРИМЕНИЛАСЬ: нет фрагмента ${JSON.stringify(from)}`);
  process.exit(3);
}
writeFileSync(file, source.replace(from, to));
NODE
  if [ $? -ne 0 ]; then
    echo "$id: МУТАЦИЯ НЕ ПРИМЕНИЛАСЬ" | tee "$OUT/mut-$id.log"
    echo "$id ? ?" >> "$OUT/summary.txt"
    return
  fi
  GUI_DOM_APP="$dir/ui/app.js" node --test "$TEST" > "$OUT/mut-$id.log" 2>&1
  read -r pass fail < <(summarize "$OUT/mut-$id.log")
  echo "$id: pass=${pass:-?} fail=${fail:-?}"
  echo "$id $pass $fail" >> "$OUT/summary.txt"
}

: > "$OUT/summary.txt"
echo "app.js md5 до мутаций: $BEFORE"

node --test "$TEST" > "$OUT/green-baseline.log" 2>&1
read -r pass fail < <(summarize "$OUT/green-baseline.log")
echo "baseline: pass=$pass fail=$fail"
echo "baseline $pass $fail" >> "$OUT/summary.txt"

run_case "M1-chat-rows-500" "$APP" "const CHAT_ROWS = 100_000;" "const CHAT_ROWS = 500;"
run_case "M2-no-row-replaceChildren" "$APP" $'  row.className = "row";\n  row.replaceChildren();' '  row.className = "row";'
run_case "M3-no-emoji-replaceChildren" "$APP" "emojiGrid.replaceChildren(" "emojiGrid.append("
run_case "M4-no-resize-observe" "$APP" "      this.observer.observe(container);" "      void container;"
run_case "M5-no-row-remove" "$APP" "        el.remove();" "        void 0;"

AFTER=$(md5sum "$APP" | cut -d' ' -f1)
echo "app.js md5 после мутаций: $AFTER"
if [ "$BEFORE" != "$AFTER" ]; then
  echo "ОШИБКА: сдаваемый app.js изменился!" | tee -a "$OUT/summary.txt"
  exit 1
fi
echo "restore проверен: md5 $AFTER"
cat "$OUT/summary.txt"
