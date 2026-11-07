#!/usr/bin/env bash
# TR-197A: 5 обязательных мутаций voice-diagnostics.js + 2 дополнительные пробы.
# Каждая мутация: точечная замена одной конструкции → прогон targeted unit-тестов →
# побайтовый откат из бэкапа с проверкой MD5. Если MD5 после отката не сошёлся — exit 3.
set -uo pipefail

ROOT=/home/sanya/orca/workspaces/project/feature-task-290-fe-voice-diagnostics
SRC="$ROOT/rust/gui-spike/ui/features/voice-diagnostics.js"
TEST_REL="tests/unit/voice-diagnostics.test.mjs"
OUT="$(cd "$(dirname "$0")" && pwd)"
BAK=/tmp/opencode/tr197a/voice-diagnostics.js.orig
EXPECTED=75a2a965b69d65471377f8c0506895ac

mkdir -p "$(dirname "$BAK")"
cp "$SRC" "$BAK"
BASE_MD5=$(md5sum "$SRC" | cut -d' ' -f1)
if [ "$BASE_MD5" != "$EXPECTED" ]; then
  echo "БАЗА НЕ СОВПАЛА: $BASE_MD5 != $EXPECTED" >&2
  exit 4
fi
rm -f "$OUT"/M*.log "$OUT"/B*.log
: > "$OUT/run.log"

restore() {
  cp "$BAK" "$SRC"
  local m
  m=$(md5sum "$SRC" | cut -d' ' -f1)
  if [ "$m" != "$EXPECTED" ]; then
    echo "ОТКАТ НЕ СОШЁЛСЯ: $m != $EXPECTED" | tee -a "$OUT/run.log" >&2
    exit 3
  fi
  return 0
}

apply_one() {
  # $1 = old, $2 = new; ровно одна замена, иначе код 5
  OLD="$1" NEW="$2" python3 - "$SRC" <<'PY'
import os, sys
path = sys.argv[1]
src = open(path, encoding='utf-8').read()
old = os.environ['OLD']
new = os.environ['NEW']
n = src.count(old)
if n != 1:
    sys.stderr.write(f'ожидалось 1 вхождение, найдено {n}\n')
    sys.exit(5)
open(path, 'w', encoding='utf-8').write(src.replace(old, new, 1))
PY
}

run_case() {
  local id="$1" kind="$2" desc="$3" old="$4" new="$5"
  echo "=== $id [$kind] $desc" | tee -a "$OUT/run.log"
  if ! apply_one "$old" "$new"; then
    echo "$id: ПРИМЕНЕНИЕ НЕ УДАЛОСЬ" | tee -a "$OUT/run.log"
    return 1
  fi
  diff -u "$BAK" "$SRC" >> "$OUT/run.log"
  ( cd "$ROOT" && node --test "$TEST_REL" ) > "$OUT/$id.log" 2>&1
  local rc=$?
  local pass fail
  pass=$(awk '/^# pass /{print $3}' "$OUT/$id.log")
  fail=$(awk '/^# fail /{print $3}' "$OUT/$id.log")
  if [ -z "${fail:-}" ]; then
    # node >= 20 печатает «ℹ fail N»
    pass=$(grep -a '^ℹ pass ' "$OUT/$id.log" | awk '{print $3}')
    fail=$(grep -a '^ℹ fail ' "$OUT/$id.log" | awk '{print $3}')
  fi
  echo "$id: exit=$rc pass=${pass:-?} fail=${fail:-?}" | tee -a "$OUT/run.log"
  { grep -a '^✖\|^  ✖' "$OUT/$id.log" | head -12 || true; } | sed 's/^/  /' | tee -a "$OUT/run.log"
  restore || return 3
  echo "$id: откат OK (MD5 $EXPECTED)" | tee -a "$OUT/run.log"
  echo
}

# --- M1..M5: обязательные мутации (5/5) ---
run_case M1 "обязательная" \
  "evaluatePingQuality: граница bad по пингу > 500 → >= 500" \
  'if (pingMs > PING_AVERAGE_MAX_MS ||' \
  'if (pingMs >= PING_AVERAGE_MAX_MS ||'

run_case M2 "обязательная" \
  "formatPopoutMetricsRu: порог предупреждения по потерям 10% → 5%" \
  '(hasLoss && numericLoss > LOSS_AVERAGE_MAX_PCT)' \
  '(hasLoss && numericLoss > LOSS_FINE_MAX_PCT)'

run_case M3 "обязательная" \
  "extractShortHostname: отсечение порта по ':' убрано (regex [^.:] → [^.])" \
  "const match = /^[^.:]*/.exec(trimmed);" \
  "const match = /^[^.]*/.exec(trimmed);"

run_case M4 "обязательная" \
  "formatVoicePrivacyCodeChunks: длина кода 30 → >=29 цифр" \
  "if (!/^\\d{30}$/.test(digits)) {" \
  "if (!/^\\d{29,}$/.test(digits)) {"

run_case M5 "обязательная" \
  "computeSparklinePoints: окно истории 199 → 20 точек (PING_AVERAGE_WINDOW)" \
  "const window = values.slice(-PING_HISTORY_MAX_POINTS);" \
  "const window = values.slice(-PING_AVERAGE_WINDOW);"

# --- B1..B2: дополнительные пробы вне 5/5 ---
run_case B1 "проба" \
  "controller.open(): снят guard идемпотентности → повторный open дергает загрузку" \
  "  function open() {
    if (!state.isOpen) {
      state.isOpen = true;
      state.copiedPrivacyCode = false;
      if (typeof callbacks.onFetchDiagnostics === 'function') {
        callbacks.onFetchDiagnostics();
      }
    }
    return getState();
  }" \
  "  function open() {
    state.isOpen = true;
    state.copiedPrivacyCode = false;
    if (typeof callbacks.onFetchDiagnostics === 'function') {
      callbacks.onFetchDiagnostics();
    }
    return getState();
  }"

run_case B2 "проба" \
  "controller.handleKeyDown: Escape сначала закрывает попап, потом модалку (обратный порядок)" \
  "    if (state.isDebugModalOpen) {
      state.isDebugModalOpen = false;
    } else if (state.isOpen) {
      state.isOpen = false;
      state.copiedPrivacyCode = false;
    } else {
      return false;
    }" \
  "    if (state.isOpen) {
      state.isOpen = false;
      state.copiedPrivacyCode = false;
    } else if (state.isDebugModalOpen) {
      state.isDebugModalOpen = false;
    } else {
      return false;
    }"

FINAL=$(md5sum "$SRC" | cut -d' ' -f1)
echo "ИТОГ: исходник $FINAL (ожидался $EXPECTED)" | tee -a "$OUT/run.log"
[ "$FINAL" = "$EXPECTED" ] || exit 3
echo "OK: файл восстановлен побайтово, тесты не тронуты" | tee -a "$OUT/run.log"
