// @ts-check
/**
 * scripts/gui-watch.mjs
 *
 * Автоматический фоновый наблюдатель (Auto-Test & Parity Watcher).
 * Автоматически отслеживает изменения в `rust/gui-spike/ui/` и мгновенно
 * запускает аудит целостности UI <-> BFF и санитарную проверку тестов.
 * Разработчику не нужно вручную ничего запускать или звать ИИ —
 * любая опечатка или битый селектор высвечиваются мгновенно при сохранении файла.
 *
 * Запуск: npm run gui:watch
 */

import { watch } from "node:fs";
import { spawnSync } from "node:child_process";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../", import.meta.url));
const UI_DIR = join(ROOT, "rust", "gui-spike", "ui");

console.log("👀 [GUI-WATCH] Starting Zero-Touch Autonomous Watcher...");
console.log(`📁 Watching directory: ${UI_DIR}`);
console.log("⚡ Auto-running Parity Audit & Sanity Precheck on file save...\n");

/** @type {ReturnType<typeof setTimeout> | null} */
let timeoutId = null;

/**
 * @param {string} filename
 */
function runVerification(filename) {
  console.log(`\n🔄 [CHANGE DETECTED] Modified: ${filename}`);
  console.log("⏱️  Running instant Parity & Sanity verification...");

  const start = Date.now();
  const audit = spawnSync("node", ["scripts/gui-parity-audit.mjs"], { cwd: ROOT, encoding: "utf8" });
  const duration = ((Date.now() - start) / 1000).toFixed(2);

  if (audit.status === 0) {
    console.log(`✅ [GUI-PARITY] Clean! Verified in ${duration}s.`);
  } else {
    console.error(`❌ [GUI-PARITY] Regression detected!`);
    console.error(audit.stdout || audit.stderr);
  }
}

watch(UI_DIR, { recursive: true }, (eventType, filename) => {
  if (!filename || (!filename.endsWith(".js") && !filename.endsWith(".html") && !filename.endsWith(".css"))) {
    return;
  }

  // Debounce 300ms для групповых сохранений
  if (timeoutId) clearTimeout(timeoutId);
  timeoutId = setTimeout(() => {
    runVerification(filename);
  }, 300);
});

// Первоначальный прогон при старте
runVerification("initial-check");
