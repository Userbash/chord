#!/usr/bin/env bash
# Static REA analysis of the installed Discord client.
#
#   scripts/analyze.sh [path/to/app.asar]
#
# Writes:
#   evidence/js-app-graph.json   REA JavaScript application graph + Electron boundaries
#   evidence/asar-inventory.json artifact inventory (when inspectable)
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
asar="${1:-}"

if [[ -z "$asar" ]]; then
  asar="$(ls -d "$HOME"/.config/discord/app-*/resources/app.asar /usr/share/discord/resources/app.asar 2>/dev/null | head -1 || true)"
fi
if [[ ! -f "$asar" ]]; then
  echo "app.asar не найден; передайте путь аргументом: scripts/analyze.sh /path/app.asar" >&2
  exit 1
fi

mkdir -p "$root/evidence"
echo "Анализирую: $asar"
rea analyze-javascript-application "$asar" --format json >"$root/evidence/js-app-graph.json"
rea inspect-artifact "$asar" --format json >"$root/evidence/asar-inventory.json" 2>/dev/null || true
echo "Готово: $root/evidence/"
