#!/usr/bin/env bash
# scripts/auto-commit-push.sh
#
# Скрипт полной автоматизации рутины фиксации изменений (Zero-Token Commit & Push Pipeline).
#
# Заменяет повторяющуюся цепочку:
#   npm run fix:routine -> AGENT_DEPT=tech_lead ./scripts/collab.sh verify -> git add -> git commit -> git push (origin + public)
#
# Использование:
#   bash scripts/auto-commit-push.sh "<type>(<scope>): <description>"
#   npm run auto:commit -- "fix(voice): restore Krisp NC separation"
#   ./scripts/collab.sh ship "feat(ui): add pins list"

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

MSG="${1:-}"

if [ -z "$MSG" ]; then
  echo "❌ Ошибка: Укажите сообщение коммита в формате Conventional Commits."
  echo "   Пример: bash scripts/auto-commit-push.sh 'fix(voice): restore Krisp NC separation'"
  exit 1
fi

echo "================================================================================"
echo "🚀 [AUTO-COMMIT-PUSH] Zero-Token Automated Commit & Push Pipeline"
echo "================================================================================"
echo "📝 Сообщение коммита: $MSG"
echo ""

# 1. Запуск быстрого автопилота рутины (починка плана, гигиена тестов, паритет, пречек)
echo "▶️  1. Запуск zero-token автопилота разработки..."
node "$PROJECT_ROOT/scripts/auto-fix-routine.mjs"

# 2. Полная предварительная верификация (scopes, DTO contracts, security shield, SocratiCode hubs)
echo "▶️  2. Полный аудит перед коммитом (collab verify)..."
AGENT_DEPT=tech_lead "$PROJECT_ROOT/scripts/collab.sh" verify

# 3. Добавление изменённых файлов
echo "▶️  3. Добавление изменений в git index..."
git add -A

# Проверка, есть ли что коммитить
if git diff --cached --quiet; then
  echo "ℹ️  Нет изменённых файлов для коммита."
  exit 0
fi

# 4. Создание коммита
echo "▶️  4. Фиксация коммита..."
git commit -m "$MSG"

# 5. Отправка в оба удалённых репозитория
CURRENT_BRANCH="$(git branch --show-current)"
echo "▶️  5. Отправка изменений в origin и public ($CURRENT_BRANCH)..."
git push origin "$CURRENT_BRANCH"
git push public "$CURRENT_BRANCH"

echo ""
echo "================================================================================"
echo "🎉 [AUTO-COMMIT-PUSH] Изменения успешно проверены, зафиксированы и отправлены!"
echo "   0 токенов ИИ потрачено на рутинные команды git/verify."
echo "================================================================================"
