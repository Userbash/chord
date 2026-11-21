#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/generate-ipc-bridges.mjs
 * 
 * Автоматический парсер Rust #[tauri::command] и генератор тонких JS/JSDoc обвязок.
 * Полностью автоматизирует рутину создания IPC-мостов без привлечения LLM.
 * 
 * Вход: rust/gui-spike/src/main.rs и rust/gui-spike/src/*_plugin.rs
 * Выход: rust/gui-spike/ui/features/auto-ipc-bridge.js
 */

import { readFileSync, writeFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

const SRC_DIR = join(process.cwd(), "rust/gui-spike/src");
const OUT_FILE = join(process.cwd(), "rust/gui-spike/ui/features/auto-ipc-bridge.js");
const DTS_FILE = join(process.cwd(), "rust/gui-spike/ui/features/auto-ipc-bridge.d.ts");

console.log("[IPC-Gen] Сканирование Rust исходников на наличие #[tauri::command]...");

const rustFiles = readdirSync(SRC_DIR).filter(f => f.endsWith(".rs"));
const commands = new Map();

// Регулярка для захвата #[tauri::command] и следующей за ним сигнатуры async fn / fn
const cmdRegex = /#\[tauri::command\]\s*(?:#\[[^\]]+\]\s*)*(?:pub\s+)?(?:async\s+)?fn\s+([a-zA-Z0-9_]+)\s*\(([^)]*)\)/g;

function splitRustParams(raw) {
  const result = [];
  let current = "";
  let depth = 0;
  for (let i = 0; i < raw.length; i++) {
    const ch = raw[i];
    if (ch === "<") depth++;
    else if (ch === ">") depth--;
    else if (ch === "," && depth === 0) {
      if (current.trim()) result.push(current.trim());
      current = "";
      continue;
    }
    current += ch;
  }
  if (current.trim()) result.push(current.trim());
  return result;
}

for (const file of rustFiles) {
  const content = readFileSync(join(SRC_DIR, file), "utf8");
  let match;
  while ((match = cmdRegex.exec(content)) !== null) {
    const fnName = match[1];
    const rawParams = match[2];
    
    // Фильтруем внутренние параметры Tauri: tauri::State, AppHandle, Window
    const params = splitRustParams(rawParams || "")
      .filter(p => !p.includes("State") && !p.includes("AppHandle") && !p.includes("Window") && !p.includes("State<"))
      .map(p => {
        const parts = p.split(":");
        const name = parts[0]?.trim();
        return /^[a-zA-Z_$][a-zA-Z0-9_$]*$/.test(name) ? name : null;
      })
      .filter(Boolean);

    if (!commands.has(fnName)) {
      commands.set(fnName, { fnName, params, file });
    }
  }
}

console.log(`[IPC-Gen] Найдено ${commands.size} уникальных Tauri IPC-команд.`);

const outLines = [
  "// @ts-check",
  "/**",
  " * Сгенерировано автоматически скриптом scripts/generate-ipc-bridges.mjs",
  " * Тонкий мост между Rust Tauri backend и JavaScript frontend.",
  " * Zero-token deterministic IPC binding generator.",
  " */",
  "",
  "/**",
  " * Базовый invoke-вызов Tauri",
  " * @param {string} cmd",
  " * @param {Record<string, any>} [args]",
  " * @returns {Promise<any>}",
  " */",
  "export async function invoke(cmd, args = {}) {",
  "  if (typeof window !== 'undefined' && /** @type {any} */ (window).__TAURI__?.core?.invoke) {",
  "    return /** @type {any} */ (window).__TAURI__.core.invoke(cmd, args);",
  "  }",
  "  console.warn(`[IPC-Stub] Mock call to ${cmd}`, args);",
  "  return null;",
  "}",
  "",
  "/** Экспортированные строго типизированные тонкие обвязки команд */"
];

for (const [fnName, info] of commands.entries()) {
  const camelName = fnName.replace(/_([a-z])/g, (/** @type {any} */ _, /** @type {string} */ g) => g.toUpperCase());
  const paramDocs = info.params.length > 0
    ? ` * @param {object} [args]\n` + info.params.map((/** @type {string} */ p) => ` * @param {any} [args.${p}]`).join("\n")
    : "";
  const jsParams = info.params.length > 0 ? "{ " + info.params.join(", ") + " } = {}" : "";
  const passArgs = info.params.length > 0 ? "{" + info.params.join(", ") + "}" : "{}";

  outLines.push("/**");
  outLines.push(` * Вызов команды ${fnName} (источник: ${info.file})`);
  if (paramDocs) outLines.push(paramDocs);
  outLines.push(" * @returns {Promise<any>}");
  outLines.push(" */");
  outLines.push(`export async function ${camelName}(${jsParams}) {`);
  outLines.push(`  return invoke("${fnName}", ${passArgs});`);
  outLines.push("}\n");
}

writeFileSync(OUT_FILE, outLines.join("\n"), "utf8");
console.log(`[IPC-Gen] Сгенерирован файл моста: ${OUT_FILE}`);

// Генерация TypeScript bindings.d.ts для строгой typecheck проверки
const dtsLines = [
  "/**",
  " * bindings.d.ts — Автоматически сгенерированные TypeScript типы для Tauri IPC команд.",
  " * Zero-Token генерация из Rust #[tauri::command] (STAGE-10.BFF.2).",
  " */",
  "",
  "export declare function invoke<T = any>(cmd: string, args?: Record<string, any>): Promise<T>;",
  ""
];

for (const [fnName, info] of commands.entries()) {
  const camelName = fnName.replace(/_([a-z])/g, (_, g) => g.toUpperCase());
  const paramTypes = info.params.length > 0
    ? "args: { " + info.params.map(p => `${p}?: any`).join("; ") + " }"
    : "args?: Record<string, never>";
  dtsLines.push(`/** ${fnName} (from ${info.file}) */`);
  dtsLines.push(`export declare function ${camelName}(${paramTypes}): Promise<any>;\n`);
}

writeFileSync(DTS_FILE, dtsLines.join("\n"), "utf8");
console.log(`[IPC-Gen] Сгенерирован TypeScript файл типов: ${DTS_FILE}`);
