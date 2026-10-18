#!/usr/bin/env bash
# Static per-function tables for every Discord native module.
#
#   scripts/extract-static-functions.sh [discord-app-dir]
#
# Writes, per module, into functions/static/:
#   <module>.csv      name,type,address,size  (from the ELF symbol table)
#   <module>.nm.txt   raw `nm` output kept as provenance
#
# Works without a native-analysis engine: the Discord modules ship readable
# symbol tables (`.symtab`), so `nm` is enough for a full function inventory.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
app="${1:-$(ls -d "$HOME"/.config/discord/app-* 2>/dev/null | sort -V | tail -1)}"
out="$root/functions/static"

if [[ -z "$app" || ! -d "$app" ]]; then
  echo "Каталог Discord не найден; передайте путь: scripts/extract-static-functions.sh ~/.config/discord/app-1.0.162" >&2
  exit 1
fi

mkdir -p "$out"
count=0

while IFS= read -r file; do
  name="$(basename "$file" .node)"
  nm --defined-only --format=posix "$file" 2>/dev/null >"$out/$name.nm.txt"
  if [[ ! -s "$out/$name.nm.txt" ]]; then
    # Stripped modules (e.g. updater.node) expose only dynamic symbols.
    nm -D --defined-only --format=posix "$file" 2>/dev/null >"$out/$name.nm.txt"
  fi
  awk -v module="$name" 'NF >= 3 { name=$1; type=$2; address=$3; size=($4 != "" ? $4 : ""); if (type ~ /^[TtWw]$/) printf "%s,%s,%s,%s\n", name, type, address, size }' \
    "$out/$name.nm.txt" >"$out/$name.csv"
  functions="$(wc -l <"$out/$name.csv")"
  echo "$name: функций $functions -> functions/static/$name.csv"
  count=$((count + 1))
done < <(find "$app" -maxdepth 4 -name '*.node' | sort)

echo "Модулей обработано: $count"
