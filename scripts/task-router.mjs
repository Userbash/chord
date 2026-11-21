#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/task-router.mjs
 * 
 * Адаптивный маршрутизатор задач по сложности (Complexity-First Model Routing).
 * Определяет четкую границу:
 * 1. TIER_0_SCRIPT: Детерминированные скрипты (0 токенов, запуск локально без ИИ).
 * 2. TIER_S: Минимальный ИИ (gemini-4-hydrogen-low / gemini-3.8-flash-low, effort: low).
 * 3. TIER_M: Средний ИИ (gemini-4-hydrogen-medium / flash-medium, effort: medium).
 * 4. TIER_L: Сложное ядро и протоколы (gemini-4-argon-medium, effort: medium).
 * 5. TIER_XL: Критическая криптография и zero-copy concurrency (gemini-4-carbon-high, effort: high).
 * 
 * Использование:
 *   node scripts/task-router.mjs --spec "Авто-генерация файла bindings.d.ts"
 *   node scripts/task-router.mjs --spec "Реализовать DAVE MLS v1"
 */

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

let tier = "TIER_M";
let model = "gemini-3.8-flash-medium";
let effort = "medium";
let maxIterations = 5;
let scriptCommand = null;
let rationale = "";

// -----------------------------------------------------------------------------
// 1. ПРОВЕРКА НА TIER 0 (ЧИСТЫЕ ДЕТЕРМИНИРОВАННЫЕ СКРИПТЫ — 0 ТОКЕНОВ ИИ)
// -----------------------------------------------------------------------------
if (
  lower.includes("bindings.d.ts") ||
  lower.includes("specta") ||
  lower.includes("генерация ipc") ||
  lower.includes("generate:ipc")
) {
  tier = "TIER_0_SCRIPT";
  scriptCommand = "node scripts/generate-ipc-bridges.mjs";
  model = "none";
  effort = "none";
  maxIterations = 1;
  rationale = "Детерминированная генерация типов TypeScript из Rust команд через AST. 0 токенов ИИ.";
} else if (
  lower.includes("стресс-тест sqlite fts5") ||
  lower.includes("stress-fts5")
) {
  tier = "TIER_0_SCRIPT";
  scriptCommand = "node scripts/stress-fts5.mjs --messages 10000 --threads 2";
  model = "none";
  effort = "none";
  maxIterations = 1;
  rationale = "Готовый детерминированный стресс-бенчмарк FTS5. Выполняется скриптом за 2 секунды без ИИ.";
} else if (
  lower.includes("стресс-тест кольцевого буфера") ||
  lower.includes("джиттер 100 мс") ||
  lower.includes("bench-voice-cpu")
) {
  tier = "TIER_0_SCRIPT";
  scriptCommand = "node scripts/bench-voice-cpu.mjs";
  model = "none";
  effort = "none";
  maxIterations = 1;
  rationale = "Готовый стресс-тест аудио-буфера с имитацией джиттера. Запуск через Node.js без ИИ.";
} else if (
  lower.includes("моделирование нагрузки 50") ||
  lower.includes("50 активных участников") ||
  lower.includes("stress-test-50-peers")
) {
  tier = "TIER_0_SCRIPT";
  scriptCommand = "node scripts/stress-test-50-peers.mjs";
  model = "none";
  effort = "none";
  maxIterations = 1;
  rationale = "Готовый бенчмарк 50 голосовых пиров с замером CPU/памяти. 0 токенов ИИ.";
} else if (
  lower.includes("удаление закомментированного") ||
  lower.includes("чистка мертвого кода") ||
  lower.includes("clean-dead-code") ||
  lower.includes("audit:deadcode")
) {
  tier = "TIER_0_SCRIPT";
  scriptCommand = "node scripts/clean-dead-code.mjs";
  model = "none";
  effort = "none";
  maxIterations = 1;
  rationale = "Автоматический AST-сканер забытых отладочных инструкций и рудиментов. 0 токенов ИИ.";
} else if (
  lower.includes("cargo audit") ||
  lower.includes("дерево зависимостей без уязвимостей") ||
  lower.includes("security-audit-deps")
) {
  tier = "TIER_0_SCRIPT";
  scriptCommand = "npm run audit:security";
  model = "none";
  effort = "none";
  maxIterations = 1;
  rationale = "Автоматический аудит безопасности (forbid(unsafe), C FFI, сканирование секретов). 0 токенов.";
} else if (
  lower.includes("синхронизация дорожной карты") ||
  lower.includes("roadmap-audit") ||
  lower.includes("roadmap:sync")
) {
  tier = "TIER_0_SCRIPT";
  scriptCommand = "npm run roadmap:sync";
  model = "none";
  effort = "none";
  maxIterations = 1;
  rationale = "Детерминированный аудит тонких мостов и подсчет прогресса roadmap. 0 токенов.";
} else if (
  lower.includes("синхронизация git") ||
  lower.includes("git pull") ||
  lower.includes("git push") ||
  lower.includes("git-sync")
) {
  tier = "TIER_0_SCRIPT";
  scriptCommand = "npm run git:sync";
  model = "none";
  effort = "none";
  maxIterations = 1;
  rationale = "Автоматизированная синхронизация origin + public репозиториев через bash scripts/git-sync.sh.";
}
// -----------------------------------------------------------------------------
// 2. TIER XL (СЛОЖНЕЙШАЯ КРИПТОГРАФИЯ, ДЕСКРИПТОРЫ И ZERO-COPY)
// -----------------------------------------------------------------------------
else if (
  lower.includes("crypto") ||
  lower.includes("srtp") ||
  lower.includes("dave") ||
  lower.includes("mls") ||
  lower.includes("ring-buffer") ||
  lower.includes("lock-free") ||
  lower.includes("pipewire") ||
  lower.includes("ownedfd") ||
  lower.includes("deadlock")
) {
  tier = "TIER_XL";
  model = "gemini-4-carbon-high";
  effort = "high";
  maxIterations = 8;
  rationale = "Критическая криптография / Zero-Copy / Дескрипторы PipeWire: требует максимальной мощности Carbon.";
}
// -----------------------------------------------------------------------------
// 3. TIER S (МЕХАНИЧЕСКИЙ МИНИМАЛЬНЫЙ ИИ — LOW EFFORT)
// -----------------------------------------------------------------------------
else if (
  lower.includes("css") ||
  lower.includes("token") ||
  lower.includes("стил") ||
  lower.includes("док") ||
  lower.includes("readme") ||
  lower.includes("format") ||
  lower.includes("переименов") ||
  lower.includes("one-line") ||
  lower.includes("конфиг") ||
  lower.includes("markdown") ||
  lower.includes("спецификац")
) {
  tier = "TIER_S";
  model = "gemini-4-hydrogen-low";
  effort = "low";
  maxIterations = 3;
  rationale = "Механическая текстовая задача: используется быстрая экономичная модель с минимальным effort.";
}
// -----------------------------------------------------------------------------
// 4. TIER L (ПРОТОКОЛЫ СЕТИ, ШЛЮЗ, ПАРСЕРЫ)
// -----------------------------------------------------------------------------
else if (
  lower.includes("gateway") ||
  lower.includes("websocket") ||
  lower.includes("tungstenite") ||
  lower.includes("zlib") ||
  lower.includes("heartbeat") ||
  lower.includes("identify") ||
  lower.includes("resume") ||
  lower.includes("permissions") ||
  lower.includes("auditlog") ||
  lower.includes("mutation")
) {
  tier = "TIER_L";
  model = "gemini-4-argon-medium";
  effort = "medium";
  maxIterations = 6;
  rationale = "Сетевой протокол Gateway / стейт-машина Heartbeat / валидатор прав: рабочая лошадка Argon Medium.";
}
// -----------------------------------------------------------------------------
// 5. TIER M (СТАНДАРТНЫЙ UI / СВЯЗКИ REST И ДАННЫХ — ПО УМОЛЧАНИЮ)
// -----------------------------------------------------------------------------
else {
  tier = "TIER_M";
  model = "gemini-4-hydrogen-medium";
  effort = "low";
  maxIterations = 4;
  rationale = "Стандартная продуктовая задача (UI/BFF обвязка, команды): Hydrogen Medium с низким effort.";
}

const plan = {
  tier,
  isScript: tier === "TIER_0_SCRIPT",
  scriptCommand,
  model,
  effort,
  maxIterations,
  rationale,
  circuitBreakerThreshold: maxIterations,
  recommendedFlags: tier === "TIER_0_SCRIPT" ? "--script-only" : `--model "${model}" --effort ${effort}`,
};

console.log(JSON.stringify(plan, null, 2));
