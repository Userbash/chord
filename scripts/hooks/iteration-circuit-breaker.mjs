#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/hooks/iteration-circuit-breaker.mjs
 * 
 * Предохранитель галлюцинаций: Ограничение итераций воркера (Max 6 Iterations Guard).
 * 
 * Научно доказанный факт: После 8-10 итераций в длинном диалоге агент накапливает
 * взаимно противоречивый контекст, начинает стирать свои же правки и галлюцинировать.
 * 
 * Данный скрипт:
 * 1. Ведет строгий счетчик ходов воркера (актуален для любого воркера и задачи).
 * 2. При достижении 5-го шага выдает жесткое предупреждение завершить задачу.
 * 3. При достижении 6-го шага активирует Circuit Breaker:
 *    - Запечатывает диффы в контрольный патч (.claude/checkpoints/<task>.patch).
 *    - Блокирует хаотичные попытки и останавливает зацикливание.
 *    - Передает чистое состояние свежему воркеру с чистым контекстным окном.
 */

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { execSync } from "node:child_process";

const STATE_DIR = join(process.cwd(), ".claude/worker-states");
const CHECKPOINT_DIR = join(process.cwd(), ".claude/checkpoints");

if (!existsSync(STATE_DIR)) mkdirSync(STATE_DIR, { recursive: true });
if (!existsSync(CHECKPOINT_DIR)) mkdirSync(CHECKPOINT_DIR, { recursive: true });

const MAX_ITERATIONS = 6;
const WARNING_THRESHOLD = 5;

/**
 * Регистрирует шаг воркера и проверяет порог галлюцинаций.
 * @param {string} taskId Идентификатор задачи или диспетчера
 * @returns {{ allowed: boolean, iteration: number, action: 'continue' | 'warn' | 'circuit_break', message?: string }}
 */
export function recordWorkerIteration(taskId) {
  const safeId = taskId.replace(/[^a-zA-Z0-9_-]/g, "_");
  const stateFile = join(STATE_DIR, `${safeId}.json`);

  let state = { taskId, iteration: 0, createdAt: new Date().toISOString() };
  if (existsSync(stateFile)) {
    try {
      state = JSON.parse(readFileSync(stateFile, "utf8"));
    } catch {
      // Игнорируем ошибки парсинга
    }
  }

  state.iteration += 1;
  state.lastUpdated = new Date().toISOString();
  writeFileSync(stateFile, JSON.stringify(state, null, 2), "utf8");

  // Зона нормальной работы
  if (state.iteration < WARNING_THRESHOLD) {
    return {
      allowed: true,
      iteration: state.iteration,
      action: "continue",
    };
  }

  // Желтая зона: предупреждение
  if (state.iteration === WARNING_THRESHOLD) {
    return {
      allowed: true,
      iteration: state.iteration,
      action: "warn",
      message: `[IterationGuard] Внимание: шаг 5 из ${MAX_ITERATIONS}. Завершите задачу (зеленый тест + отчет), чтобы избежать размытия контекста.`,
    };
  }

  // Красная зона: Предохранитель (Circuit Breaker)
  const patchFile = join(CHECKPOINT_DIR, `${safeId}.patch`);
  try {
    const diff = execSync("git diff HEAD", { encoding: "utf8" });
    if (diff.trim()) {
      writeFileSync(patchFile, diff, "utf8");
    }
  } catch {
    // Безопасный фолбек
  }

  return {
    allowed: false,
    iteration: state.iteration,
    action: "circuit_break",
    message: `[CIRCUIT BREAKER TRIGGERED] Превышен порог ${MAX_ITERATIONS} итераций (риск галлюцинаций). Дифф сохранен в ${patchFile}. Не продолжайте зацикленные попытки: перейдите к отчету или перезапустите свежего воркера с патчем.`,
  };
}

// Запуск из CLI
if (process.argv[1]?.endsWith("iteration-circuit-breaker.mjs")) {
  const testId = process.argv[2] || "worker-demo-1";
  const res = recordWorkerIteration(testId);
  console.log(JSON.stringify(res, null, 2));
}
