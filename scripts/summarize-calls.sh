#!/usr/bin/env bash
# Сводка логов вызовов: сколько раз вызывалась каждая функция.
#
#   scripts/summarize-calls.sh [logs/calls-*.jsonl ...]
#
# Для каждого лога пишет рядом <лог>.by-function.csv:
#   calls,module,name,address

set -euo pipefail

files=("$@")
if [[ ${#files[@]} -eq 0 ]]; then
  mapfile -t files < <(ls logs/calls-*.jsonl 2>/dev/null || true)
fi
if [[ ${#files[@]} -eq 0 ]]; then
  echo "Логов не найдено: сначала запустите scripts/logger.mjs hook" >&2
  exit 1
fi

for file in "${files[@]}"; do
  [[ -f "$file" ]] || continue
  jq -r '[.module, .name, .address] | @tsv' "$file" |
    sort |
    uniq -c |
    sed -E 's/^ *([0-9]+) /\1\t/' |
    awk 'BEGIN { FS = "\t"; OFS = ","; print "calls,module,name,address" }
         { gsub(/"/, "\"\"", $2); gsub(/"/, "\"\"", $3); gsub(/"/, "\"\"", $4);
           print $1, $2, $3, $4 }' \
      >"$file.by-function.csv"
  total="$(wc -l <"$file")"
  unique="$(($(wc -l <"$file.by-function.csv") - 1))"
  echo "$file: вызовов $total, уникальных функций $unique -> $file.by-function.csv"
done
