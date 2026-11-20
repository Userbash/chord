#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/task-router.mjs
 * 
 * Адаптивный маршрутизатор задач по сложности (Complexity-First Model Routing).
 * Анализирует описание задачи, ключевые слова и целевые файлы, и автоматически
 * подбирает наиболее экономичную модель, уровень reasoning effort и тайм-аут,
 * предотвращая трату дорогих моделей на рутинные механические задачи.
 * 
 * Использование:
 *   node scripts/task-router.mjs --spec "Создать CSS-стили для меню"
 *   node scripts/task-router.mjs --spec "Реализовать SRTP AES-GCM криптографию"
 */

import { readFileSync } from "node:fs";

const args = process.argv.slice(2);
let spec = "";
for (let i = 0; i < args.length; i++) {
  if (args[i] === "--spec" && args[i + 1]) {
    spec = args[i + 1];
    break;
  }
}

if (!spec) {
  spec = args.join(" ") || "Routine task";
}

const lower = spec.toLowerCase();

/**
 * Классификация сложности:
 * - TIER_S: Документация, CSS, токены, простые флаги, переименование, статические моки.
 * - TIER_M: UI-компоненты, модалки, DTO/serde, простые адаптеры, юнит-тесты.
 * - TIER_L: REST BFF, Gateway-парсеры, IPC-мосты, QA-сьюты, валидация прав.
 * - TIER_XL: Lock-free буферы, MLS v1, SRTP, AES-GCM, Tokio <-> GPUI мост, Deadlock arbitration.
 */
let tier = "TIER_M";
let model = "gemini-3.8-flash-medium";
let effort = "medium";
let maxIterations = 6;
let rationale = "";

// Проверка на Tier XL (Сложнейшие задачи)
if (
  lower.includes("crypto") ||
  lower.includes("srtp") ||
  lower.includes("dave") ||
  lower.includes("mls") ||
  lower.includes("ring-buffer") ||
  lower.includes("lock-free") ||
  lower.includes("concurrency") ||
  lower.includes("deadlock")
) {
  tier = "TIER_XL";
  model = "gemini-4-carbon-high";
  effort = "high";
  maxIterations = 8;
  rationale = "Критическая криптография / многопоточность: требует максимального логического рассуждения.";
}
// Проверка на Tier S (Рутинные механические задачи)
else if (
  lower.includes("css") ||
  lower.includes("token") ||
  lower.includes("стил") ||
  lower.includes("док") ||
  lower.includes("readme") ||
  lower.includes("format") ||
  lower.includes("переименов") ||
  lower.includes("one-line") ||
  lower.includes("конфиг")
) {
  tier = "TIER_S";
  model = "gemini-4-hydrogen-low";
  effort = "low";
  maxIterations = 4;
  rationale = "Механическая задача (верстка/токены/документация): достаточно быстрой экономичной модели без тяжелого reasoning.";
}
// Проверка на Tier L (Стандартный бэкенд и тесты ядра)
else if (
  lower.includes("gateway") ||
  lower.includes("permissions") ||
  lower.includes("права") ||
  lower.includes("парсер") ||
  lower.includes("rust core") ||
  lower.includes("chord-store") ||
  lower.includes("sqlite") ||
  lower.includes("mutation")
) {
  tier = "TIER_L";
  model = "gemini-4-argon-medium";
  effort = "medium";
  maxIterations = 6;
  rationale = "Бэкенд ядра / протокольные парсеры: оптимальна сбалансированная модель среднего уровня.";
}
// Tier M (UI и обвязки по умолчанию)
else {
  tier = "TIER_M";
  model = "gemini-3.8-flash-medium";
  effort = "medium";
  maxIterations = 5;
  rationale = "Стандартная продуктовая задача (UI/BFF обвязка): скоростная модель Flash с умеренным расходом токенов.";
}

const plan = {
  tier,
  model,
  effort,
  maxIterations,
  rationale,
  circuitBreakerThreshold: maxIterations,
  recommendedFlags: `--model "${model}" --effort ${effort}`,
};

console.log(JSON.stringify(plan, null, 2));
