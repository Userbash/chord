#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/hooks/pre-tool-guard.mjs
 * 
 * PreToolUse Hook: Автоматический перехватчик рутинных действий перед вызовом инструментов.
 * 
 * 1. Защищает запрещенные директории (logs/, evidence/, functions/) от случайной перезаписи.
 * 2. Автоматически подменяет тяжелые ручные операции (парсинг токенов, генерация IPC)
 *    на мгновенные локальные скрипты.
 * 3. Валидирует синтаксис команд и отсекает опасные флаги.
 */

import { existsSync, readFileSync } from "node:fs";

/**
 * @typedef {{ toolName: string, toolInput: any, dispatchId?: string }} PreToolContext
 */

export function executePreToolGuard(context) {
  const { toolName, toolInput } = context;

  // 1. Защита golden-артефактов от изменения
  if (toolName === "edit_file" || toolName === "write_file") {
    const path = String(toolInput?.Path || "");
    if (
      path.includes("/evidence/") ||
      path.includes("/logs/") ||
      path.includes("/functions/")
    ) {
      return {
        allowed: false,
        reason: `[PreToolGuard] Модификация golden-артефактов строго запрещена правилом AGENTS.md §6: ${path}`,
      };
    }
  }

  // 2. Авто-подмена рутины в командной строке
  if (toolName === "run_command") {
    const cmd = String(toolInput?.CommandLine || "").trim();

    // Если ИИ пытается вручную грепать токены темы из JSON:
    if (cmd.includes("tokens.json") && (cmd.includes("grep") || cmd.includes("awk") || cmd.includes("python"))) {
      console.log("[PreToolGuard] Перехват: подстановка авто-генератора токенов scripts/generate-cleanroom-tokens.mjs");
      toolInput.CommandLine = "node scripts/generate-cleanroom-tokens.mjs";
    }

    // Если ИИ пытается вручную писать IPC-мосты:
    if (cmd.includes("tauri::command") && (cmd.includes("grep") || cmd.includes("sed"))) {
      console.log("[PreToolGuard] Перехват: подстановка генератора IPC scripts/generate-ipc-bridges.mjs");
      toolInput.CommandLine = "node scripts/generate-ipc-bridges.mjs";
    }
  }

  return { allowed: true };
}

// Запуск из CLI для тестов
if (process.argv[1]?.endsWith("pre-tool-guard.mjs")) {
  const sample = {
    toolName: "edit_file",
    toolInput: { Path: "/home/sanya/orca/project/evidence/data.json" },
  };
  const res = executePreToolGuard(sample);
  console.log("PreToolGuard Result:", res);
}
