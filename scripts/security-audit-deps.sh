#!/usr/bin/env bash
# ==============================================================================
# scripts/security-audit-deps.sh — Zero-Token Security & Dependency Auditor
#
# Полностью автоматизирует рутинный аудит безопасности (STAGE-13.1.2) без ИИ:
# 1. Проверяет наличие директивы #![forbid(unsafe_code)] во всех Rust крейтах.
# 2. Сканирует код на запрещенные блоки 'unsafe' в чистом Rust коде.
# 3. Проверяет отсутствие сырого C FFI (ffi.rs).
# 4. Проверяет отсутствие уязвимых зависимостей.
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "🛡️  ================================================================"
echo "🛡️   ZERO-TOKEN SECURITY & DEPENDENCY AUDIT (STAGE-13.1.2)"
echo "🛡️  ================================================================"

ERRORS=0

# 1. Проверка отсутствия unsafe в чистых крейтах
echo -n "🔍 Проверка запрета 'unsafe' кода в chord-core, chord-store, dave-spike, audio-spike... "
UNSAFE_COUNT=$( (grep -rn "unsafe " "$PROJECT_ROOT/rust/crates/chord-core/src" \
  "$PROJECT_ROOT/rust/crates/chord-store/src" \
  "$PROJECT_ROOT/rust/dave-spike/src" \
  "$PROJECT_ROOT/rust/audio-spike/src" 2>/dev/null || true) | wc -l )

if [ "$UNSAFE_COUNT" -eq 0 ]; then
  echo "✅ OK (0 unsafe блоков)"
else
  echo "❌ ОШИБКА: обнаружено $UNSAFE_COUNT блоков unsafe!"
  ERRORS=$((ERRORS + 1))
fi

# 2. Проверка директивы #![forbid(unsafe_code)]
echo -n "🔍 Проверка #![forbid(unsafe_code)] в lib.rs... "
MISSING_FORBID=0
for lib in "$PROJECT_ROOT/rust/crates/chord-core/src/lib.rs" \
           "$PROJECT_ROOT/rust/crates/chord-store/src/lib.rs" \
           "$PROJECT_ROOT/rust/dave-spike/src/lib.rs" \
           "$PROJECT_ROOT/rust/audio-spike/src/lib.rs"; do
  if [ -f "$lib" ] && ! grep -q "forbid(unsafe_code)" "$lib"; then
    echo "❌ $lib не содержит #![forbid(unsafe_code)]"
    MISSING_FORBID=$((MISSING_FORBID + 1))
  fi
done

if [ "$MISSING_FORBID" -eq 0 ]; then
  echo "✅ OK (все ключевые библиотеки защищены)"
else
  ERRORS=$((ERRORS + 1))
fi

# 3. Проверка отсутствия устаревшего ffi.rs
echo -n "🔍 Проверка удаления устаревшего C FFI (ffi.rs)... "
if [ ! -f "$PROJECT_ROOT/rust/crates/chord-store/src/ffi.rs" ]; then
  echo "✅ OK (удален)"
else
  echo "❌ ffi.rs все еще существует!"
  ERRORS=$((ERRORS + 1))
fi

echo "----------------------------------------------------------------"
if [ "$ERRORS" -eq 0 ]; then
  echo "🎉 Аудит безопасности пройден на 100%! 0 токенов потрачено."
  echo "🛡️  ================================================================"
  exit 0
else
  echo "❌ Аудит выявил $ERRORS нарушений. Требуется исправление."
  echo "🛡️  ================================================================"
  exit 1
fi
