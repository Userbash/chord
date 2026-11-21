#!/usr/bin/env bash
# ==============================================================================
# scripts/quick-gate.sh — 3-Second Zero-Token Quality Gate
#
# Автоматически проверяет ветку, типы IPC, память и таргетированные тесты.
# Запускается автоматически перед коммитом или при приемке задачи.
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "🚀 [QUICK-GATE] Starting automated verification..."

# 1. Проверка ветки (только tauri)
"$PROJECT_ROOT/scripts/assert-branch-tauri.sh"

# 2. Проверка IPC моста
node "$PROJECT_ROOT/scripts/generate-ipc-bridges.mjs" >/dev/null

# 3. Контроль памяти
"$PROJECT_ROOT/scripts/worker-ram-guard.sh" >/dev/null

# 3.1. Архитектурный контроль мостов (<800 LOC, без самописных алгоритмов)
node "$PROJECT_ROOT/scripts/audit-thin-bridges.mjs" >/dev/null

# 4. Запуск таргетированных тестов
echo "🧪 Running smart test runner..."
node "$PROJECT_ROOT/scripts/test-runner-smart.mjs"

# 5. Статус канбана
bash "$PROJECT_ROOT/scripts/kanban-sync.sh" status

echo "✅ [QUICK-GATE] All checks passed successfully in < 3 seconds!"
