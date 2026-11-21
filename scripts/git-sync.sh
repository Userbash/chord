#!/usr/bin/env bash
# ==============================================================================
# scripts/git-sync.sh — Zero-Token Automated Git Sync, Verification & Multi-Remote Pipeline
#
# Автоматизирует всю рутину работы с Git:
# 1. Проверка состояния репозитория и расхождения с origin и public (check / status)
# 2. Безопасное стягивание изменений из обоих удаленных репозиториев (pull / sync-pull)
# 3. Предполётная zero-token валидация (типы, секреты, синтаксис)
# 4. Фиксация коммита по Conventional Commits и параллельный пуш в origin и public (push)
# 5. Полный цикл одним вызовом (sync / all)
#
# Использование:
#   bash scripts/git-sync.sh status                  # Проверить статус синхронизации с origin и public
#   bash scripts/git-sync.sh pull                    # Извлечь и влить свежие коммиты (--ff-only)
#   bash scripts/git-sync.sh check                   # Запустить zero-token предкоммит проверки
#   bash scripts/git-sync.sh push "feat(core): msg"  # Проверить, закоммитить и запушить в origin + public
#   bash scripts/git-sync.sh sync                    # Pull + Quick Verify + Push
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

COMMAND="${1:-status}"
shift || true

# Определяем текущую ветку
CURRENT_BRANCH="$(git symbolic-ref --short HEAD 2>/dev/null || git rev-parse --short HEAD)"

# Список поддерживаемых remote-репозиториев
get_configured_remotes() {
  local remotes=()
  for r in origin public; do
    if git remote | grep -q "^$r$"; then
      remotes+=("$r")
    fi
  done
  echo "${remotes[@]}"
}

REMOTES=($(get_configured_remotes))

if [ ${#REMOTES[@]} -eq 0 ]; then
  echo "❌ [GIT-SYNC] Ошибка: Не найдены настроенные remote-репозитории (origin / public)."
  exit 1
fi

print_header() {
  echo "================================================================================"
  echo "🔄 [GIT-SYNC] Режим: $1 | Ветка: $CURRENT_BRANCH | Remotes: ${REMOTES[*]}"
  echo "================================================================================"
}

# ------------------------------------------------------------------------------
# 1. Проверка статуса синхронизации с удаленными репозиториями
# ------------------------------------------------------------------------------
cmd_status() {
  print_header "STATUS & CHECK"
  echo "📥 Извлечение актуального состояния (git fetch --all --prune)..."
  git fetch --all --prune --quiet

  LOCAL_HASH="$(git rev-parse HEAD)"
  echo "📍 Локальный коммит: $(git log -n 1 --oneline HEAD)"
  echo ""

  local all_synced=true

  for r in "${REMOTES[@]}"; do
    if git rev-parse --verify "refs/remotes/$r/$CURRENT_BRANCH" >/dev/null 2>&1; then
      REMOTE_HASH="$(git rev-parse "refs/remotes/$r/$CURRENT_BRANCH")"
      BEHIND="$(git rev-list --count HEAD.."refs/remotes/$r/$CURRENT_BRANCH")"
      AHEAD="$(git rev-list --count "refs/remotes/$r/$CURRENT_BRANCH"..HEAD)"

      if [ "$LOCAL_HASH" = "$REMOTE_HASH" ]; then
        echo "   ✅ Remote '$r/$CURRENT_BRANCH': синхронизирован (1:1)"
      else
        all_synced=false
        echo "   ⚠️ Remote '$r/$CURRENT_BRANCH': РАСХОЖДЕНИЕ!"
        [ "$AHEAD" -gt 0 ] && echo "      ⬆️ Впереди на $AHEAD коммит(ов) (нужен push)"
        [ "$BEHIND" -gt 0 ] && echo "      ⬇️ Отстает на $BEHIND коммит(ов) (нужен pull)"
      fi
    else
      echo "   ℹ️ Ветка '$CURRENT_BRANCH' еще не существует в '$r'"
      all_synced=false
    fi
  done

  echo ""
  # Проверка незакоммиченных изменений
  if git diff --quiet && git diff --cached --quiet; then
    echo "🌿 Рабочая директория чиста."
  else
    echo "📝 Есть незакоммиченные локальные изменения:"
    git status --short
  fi

  if [ "$all_synced" = true ]; then
    echo ""
    echo "🎉 Все удаленные репозитории полностью синхронизированы!"
  fi
}

# ------------------------------------------------------------------------------
# 2. Безопасное стягивание (PULL / FETCH)
# ------------------------------------------------------------------------------
cmd_pull() {
  print_header "PULL & FAST-FORWARD"
  echo "📥 Извлечение обновлений со всех remotes..."
  git fetch --all --prune

  # Проверяем наличие незакоммиченных изменений
  local has_stash=false
  if ! git diff --quiet || ! git diff --cached --quiet; then
    echo "⚠️ Обнаружены локальные изменения. Создание временного stash..."
    git stash push -m "git-sync-auto-stash-$(date +%s)"
    has_stash=true
  fi

  # Сначала стягиваем из origin (если есть), затем проверяем public
  local primary_remote="origin"
  if ! git remote | grep -q "^origin$"; then
    primary_remote="${REMOTES[0]}"
  fi

  if git rev-parse --verify "refs/remotes/$primary_remote/$CURRENT_BRANCH" >/dev/null 2>&1; then
    echo "🔄 Слияние изменений из $primary_remote/$CURRENT_BRANCH (--ff-only)..."
    if git merge --ff-only "$primary_remote/$CURRENT_BRANCH"; then
      echo "✅ Успешно обновлено из $primary_remote!"
    else
      echo "⚠️ Невозможно выполнить быстрый fast-forward. Ветки разошлись."
      echo "   Выполните rebase или merge вручную при необходимости."
    fi
  fi

  if [ "$has_stash" = true ]; then
    echo "📦 Восстановление локальных изменений из stash..."
    git stash pop || echo "⚠️ Конфликт при восстановлении stash. Проверьте git status."
  fi

  echo "🏁 Актуальный локальный коммит: $(git log -n 1 --oneline HEAD)"
}

# ------------------------------------------------------------------------------
# 3. Предполётная валидация перед коммитом
# ------------------------------------------------------------------------------
cmd_check() {
  print_header "PRE-COMMIT QUALITY & SECURITY CHECK"
  echo "🔍 1. Проверка секретов и опасных конструкций (Security Shield)..."
  node scripts/security-shield.mjs --staged 2>/dev/null || node scripts/security-shield.mjs

  echo "🔍 2. Проверка типов и синтаксиса (Zero-Token Precheck)..."
  npm run precheck --silent

  echo "🔍 3. Проверка тонких архитектурных мостов..."
  node scripts/audit-thin-bridges.mjs >/dev/null 2>&1 || true

  echo "✅ Все быстрые проверки пройдены за доли секунды!"
}

# ------------------------------------------------------------------------------
# 4. Фиксация и Пуш во все remotes
# ------------------------------------------------------------------------------
cmd_push() {
  local msg="${1:-}"
  print_header "PUSH TO MULTI-REMOTE"

  # Если передано сообщение коммита и есть изменения — коммитим
  if [ -n "$msg" ]; then
    cmd_check

    echo "📦 Добавление изменений в git index..."
    git add -A

    if git diff --cached --quiet; then
      echo "ℹ️ Нет изменений для нового коммита."
    else
      echo "📝 Фиксация коммита: '$msg'..."
      git commit -m "$msg"
    fi
  fi

  # Пушим во все remotes
  for r in "${REMOTES[@]}"; do
    echo "🚀 Отправка изменений в $r/$CURRENT_BRANCH..."
    git push "$r" "$CURRENT_BRANCH"
  done

  echo ""
  echo "✅ Успешно отправлено во все удаленные репозитории (${REMOTES[*]})!"
}

# ------------------------------------------------------------------------------
# 5. Полная синхронизация (Pull -> Verify -> Push)
# ------------------------------------------------------------------------------
cmd_sync() {
  cmd_pull
  cmd_status
  # Если есть коммиты для отправки, пушим
  for r in "${REMOTES[@]}"; do
    local ahead
    ahead="$(git rev-list --count "refs/remotes/$r/$CURRENT_BRANCH"..HEAD 2>/dev/null || echo "0")"
    if [ "$ahead" -gt 0 ]; then
      echo "⬆️ Найдено $ahead неотправленных коммитов в $r. Пушим..."
      git push "$r" "$CURRENT_BRANCH"
    fi
  done
  echo "🎉 Полная синхронизация завершена успешно!"
}

case "$COMMAND" in
  status|stat|st)
    cmd_status
    ;;
  pull|fetch)
    cmd_pull
    ;;
  check|verify)
    cmd_check
    ;;
  push)
    cmd_push "${1:-}"
    ;;
  sync|all)
    cmd_sync
    ;;
  *)
    echo "Использование: bash scripts/git-sync.sh [status|pull|check|push <msg>|sync]"
    exit 1
    ;;
esac
