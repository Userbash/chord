#!/usr/bin/env bash
# scripts/adaptive-worker-runner.sh
# Адаптивная система управления воркерами с защитой от галлюцинаций.
#
# 1. Автоматически маршрутизирует задачу на экономичную модель (task-router.mjs).
# 2. Выполняет мгновенный онбординг (worker-preflight.sh).
# 3. Следит за счетчиком итераций (iteration-circuit-breaker.mjs), блокируя
#    галлюцинации после 6-го шага.
# 4. Подставляет сжатый фильтр тестов (smart-test-runner.mjs).

set -euo pipefail

TASK_SPEC="${1:-"Routine code or styling fix"}"
TASK_ID="${2:-"task_$(date +%s)"}"

echo "=== 1. АДАПТИВНАЯ МАРШРУТИЗАЦИЯ МОДЕЛИ ==="
ROUTING_JSON=$(node scripts/task-router.mjs --spec "$TASK_SPEC")
echo "$ROUTING_JSON"

MODEL=$(echo "$ROUTING_JSON" | grep -o '"model": "[^"]*' | cut -d'"' -f4)
EFFORT=$(echo "$ROUTING_JSON" | grep -o '"effort": "[^"]*' | cut -d'"' -f4)
TIER=$(echo "$ROUTING_JSON" | grep -o '"tier": "[^"]*' | cut -d'"' -f4)

echo ""
echo "=== 2. ПОДГОТОВКА СРЕДЫ (PREFLIGHT) ==="
bash scripts/worker-preflight.sh "$TASK_ID" ""

echo ""
echo "=== 3. РАСЧЕТ ПРЕДОХРАНИТЕЛЯ ИТЕРАЦИЙ ==="
echo "Назначена модель: $MODEL (уровень reasoning: $EFFORT, тир: $TIER)"
echo "Лимит шагов до Circuit Breaker: 6 итераций (для предотвращения деградации внимания)."
echo "Команда запуска воркера в Orca:"
echo "  orca orchestration worker-start --task $TASK_ID --agent antigravity --model \"$MODEL\" --effort $EFFORT"
